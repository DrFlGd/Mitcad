// SPDX-License-Identifier: MIT
// Real embedded-renderer sessions: transport ownership, resize and cancellation.
#include "render/MacRenderClient.hpp"
#include "render/MacRenderProtocol.hpp"
#include "render/MacCommandPayload.hpp"
#include "render/MacFrameControl.hpp"
#include "render/MacXpcPeer.hpp"
#include "render/FrameMemory.hpp"
#include "render/RenderClient.hpp"

#include <QCoreApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QJsonArray>
#include <QJsonDocument>
#include <QTimer>
#include <IOSurface/IOSurfaceAPI.h>
#include <atomic>
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <functional>
#include <memory>
#include <mutex>
#include <new>
#include <utility>
#include <vector>

namespace {
using namespace mitcad::render;
constexpr int kSamples = 4;
constexpr qint64 kDeadlineMilliseconds = 120000;

xpc_object_t rawMessage(const char* operation) {
  auto message = xpc_dictionary_create_empty();
  xpc_dictionary_set_string(message, "op", operation);
  xpc_dictionary_set_uint64(message, "version", kMacRenderProtocol);
  return message;
}

struct RawReply {
  std::atomic<bool> done{false};
  xpc_object_t value = nullptr;
  ~RawReply() { if (value) xpc_release(value); }
  bool accepted() const {
    return done.load(std::memory_order_acquire) && xpc_get_type(value) == XPC_TYPE_DICTIONARY &&
        xpc_dictionary_get_bool(value, "ok");
  }
};

class RawConnection {
public:
  using Reply = std::shared_ptr<RawReply>;
  RawConnection() {
    m_heartbeat.setInterval(5000);
    QObject::connect(&m_heartbeat, &QTimer::timeout, &m_heartbeat, [this] { send(rawMessage("heartbeat")); });
  }
  ~RawConnection() {
    m_heartbeat.stop();
    if (m_connection) { xpc_connection_cancel(m_connection); xpc_release(m_connection); }
  }
  bool start(std::string& error) {
    m_connection = xpc_connection_create(kMacRenderService, nullptr);
    if (!m_connection) { error = "could not create a raw renderer connection"; return false; }
    const auto bundle = QDir::cleanPath(QCoreApplication::applicationDirPath() +
        QStringLiteral("/../XPCServices/org.mitcad.renderer.xpc")).toUtf8();
    const bool valid = requirePeer(m_connection, bundle.constData(), error);
    const auto inbox = m_inbox;
    xpc_connection_set_event_handler(m_connection, ^(xpc_object_t incoming) {
      std::lock_guard<std::mutex> guard(inbox->mutex);
      if (xpc_get_type(incoming) == XPC_TYPE_ERROR) { inbox->closed = true; return; }
      std::size_t bytes = 0;
      const auto* data = xpc_get_type(incoming) == XPC_TYPE_DICTIONARY
          ? static_cast<const char*>(xpc_dictionary_get_data(incoming, "event", &bytes)) : nullptr;
      const auto document = data && bytes <= kMacRenderMessageBytes
          ? QJsonDocument::fromJson(QByteArray(data, static_cast<int>(bytes))) : QJsonDocument();
      if (!document.isObject() || inbox->events.size() >= 32) inbox->invalid = true;
      else inbox->events.push_back(document.object());
    });
    xpc_connection_activate(m_connection);
    if (valid) m_heartbeat.start();
    return valid;
  }
  Reply send(xpc_object_t message) {
    auto reply = std::make_shared<RawReply>();
    xpc_connection_send_message_with_reply(m_connection, message, dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0),
        ^(xpc_object_t response) {
          reply->value = xpc_retain(response);
          reply->done.store(true, std::memory_order_release);
        });
    xpc_release(message);
    return reply;
  }
  Reply open() {
    auto message = rawMessage("open");
    constexpr char configuration[] = "{\"denoiser\":\"cpu\",\"native_pass_compare\":true}";
    xpc_dictionary_set_data(message, "configuration", configuration, sizeof(configuration) - 1);
    return send(message);
  }
  Reply command(const QJsonObject& command) {
    const auto json = QJsonDocument(command).toJson(QJsonDocument::Compact);
    auto message = rawMessage("command");
    if (std::uint64_t(json.size()) > kMacRenderMessageBytes) {
      MacCommandPayload payload;
      std::string error;
      if (!payload.create(json, error)) {
        std::fprintf(stderr, "raw command payload: %s\n", error.c_str());
        xpc_release(message);
        auto failed = std::make_shared<RawReply>();
        failed->value = xpc_dictionary_create_empty();
        failed->done.store(true, std::memory_order_release);
        return failed;
      }
      xpc_dictionary_set_fd(message, "command_fd", payload.descriptor());
      xpc_dictionary_set_uint64(message, "command_bytes", payload.size());
    } else xpc_dictionary_set_data(message, "command", json.constData(), static_cast<std::size_t>(json.size()));
    return send(message);
  }
  std::vector<QJsonObject> takeEvents() {
    std::lock_guard<std::mutex> guard(m_inbox->mutex);
    return std::exchange(m_inbox->events, {});
  }
  bool invalid() const { std::lock_guard<std::mutex> guard(m_inbox->mutex); return m_inbox->invalid; }
  bool closed() const { std::lock_guard<std::mutex> guard(m_inbox->mutex); return m_inbox->closed; }
private:
  struct Inbox {
    std::mutex mutex;
    std::vector<QJsonObject> events;
    bool invalid = false;
    bool closed = false;
  };
  xpc_connection_t m_connection = nullptr;
  QTimer m_heartbeat;
  std::shared_ptr<Inbox> m_inbox = std::make_shared<Inbox>();
};

xpc_object_t rawAllocation(const std::shared_ptr<FrameMemory>& memory, const std::shared_ptr<MacFrameSurfaces>& surfaces,
                           std::uint64_t generation) {
  auto message = rawMessage("allocate");
  xpc_dictionary_set_uint64(message, "generation", generation);
  xpc_dictionary_set_fd(message, "control", memory->descriptor());
  auto array = xpc_array_create_empty();
  for (std::uint32_t slot = 0; slot < kFrameSlots; ++slot) for (std::uint32_t plane = 0; plane < kFramePlanes; ++plane) {
    auto surface = IOSurfaceCreateXPCObject(surfaces->surface(slot, plane));
    xpc_array_append_value(array, surface);
    xpc_release(surface);
  }
  xpc_dictionary_set_value(message, "surfaces", array);
  xpc_release(array);
  return message;
}

QJsonObject viewCommand(std::uint64_t sequence, int width, int height, double offset) {
  return {{QStringLiteral("cmd"), QStringLiteral("view")},
          {QStringLiteral("sequence"), static_cast<qint64>(sequence)},
          {QStringLiteral("projection"), QStringLiteral("orthographic")},
          {QStringLiteral("eye"), QJsonArray{180.0 + offset, -220.0, 160.0}},
          {QStringLiteral("target"), QJsonArray{0.0, 0.0, 10.0}},
          {QStringLiteral("up"), QJsonArray{0.0, 0.0, 1.0}},
          {QStringLiteral("half_height"), 50.0},
          {QStringLiteral("size"), QJsonArray{width, height}}};
}

std::uint64_t checksum(const Frame& frame) {
  const auto* pixels = frame.lightPixels();
  const auto count = std::size_t(frame.info.width) * frame.info.height * 4;
  std::uint64_t result = 14695981039346656037ull;
  for (std::size_t index = 0; index < count; ++index) {
    result = (result ^ pixels[index]) * 1099511628211ull;
  }
  return result;
}

bool hasRenderedPixels(const Frame& frame) {
  const auto* pixels = frame.lightPixels();
  if (!pixels || frame.info.width == 0 || frame.info.height == 0) return false;
  const auto count = std::size_t(frame.info.width) * frame.info.height;
  bool lit = false, covered = false;
  for (std::size_t pixel = 0; pixel < count; ++pixel) {
    for (std::size_t channel = 0; channel < 4; ++channel) {
      if ((pixels[pixel * 4 + channel] & 0x7c00u) == 0x7c00u) return false;
    }
    lit = lit || (pixels[pixel * 4] & 0x7fffu) != 0 || (pixels[pixel * 4 + 1] & 0x7fffu) != 0;
    covered = covered || (pixels[pixel * 4 + 3] & 0x7fffu) != 0;
  }
  return lit && covered;
}

bool sameInfo(const FrameInfo& left, const FrameInfo& right) {
  return left.width == right.width && left.height == right.height && left.fullWidth == right.fullWidth &&
      left.fullHeight == right.fullHeight && left.samples == right.samples && left.denoised == right.denoised &&
      left.view == right.view && left.catcher == right.catcher && left.reserved == right.reserved;
}

class Regression {
public:
  explicit Regression(QString device) : m_device(std::move(device)), first(*this), second(*this) { m_elapsed.start(); }

  bool run() {
    if (!first.start(101) || !second.start(201) ||
        !wait([this] { return first.complete() && second.complete(); }, "two independent initial frames")) return false;
    if (first.current.surface->surface() == second.current.surface->surface()) return fail("sessions share one surface allocation");
    if (!malformedControl() || !ambiguousCommandPayload()) return false;
    first.resize(111, 96, 64);
    second.resize(211, 96, 64);
    if (!wait([this] { return first.complete() && second.complete(); }, "both streams after malformed-header rejection")) return false;

    // Keep the displayed old allocation while the first client replaces both
    // surfaces and its control mapping. Read ownership must survive each ACK.
    const Frame old = first.current;
    const auto* const oldPixels = old.lightPixels();
    const auto oldHash = checksum(old);
    first.forbiddenFirst = 112;
    first.forbiddenLast = 113;
    // No event-loop turn occurs between these requests: the first allocation is
    // still in flight, and only the latest requested view may be delivered.
    first.resize(112, 160, 112);
    first.resize(113, 896, 640);
    first.resize(114, 256, 176);
    if (!wait([this] { return first.complete(); }, "coalesced replacement frame")) return false;
    if (first.current.lightPixels() == oldPixels || old.lightPixels() != oldPixels || checksum(old) != oldHash)
      return fail("replacing an allocation lost or overwrote the retained old frame");
    if (!second.complete()) return fail("resizing the first session changed the second session's frame");

    first.client.stop();
    if (first.client.isRunning()) return fail("a stopped client still reports a live session");
    second.resize(212, 128, 96);
    if (!wait([this] { return second.complete(); }, "second session after the first stops")) return false;
    if (checksum(old) != oldHash) return fail("stopping the worker invalidated a displayed old allocation");

    // Reusing the same client exercises epoch isolation: cancellation replies
    // and queued callbacks from its old connection cannot end the new session.
    if (!first.start(301) || !wait([this] { return first.complete(); }, "restarted first session")) return false;
    if (checksum(old) != oldHash || !second.complete()) return fail("restart crossed retained-frame or session ownership");
    if (!bulkSceneOrdering() || checksum(old) != oldHash)
      return fail("bulk scene transport failed or invalidated a retained allocation");
    if (!sameInfoPublication()) return false;
    first.client.stop();
    second.client.stop();
    std::printf("embedded renderer %s: two sessions, direct frames, resize coalescing, retained allocation, cancellation and restart passed\n",
                m_device.toUtf8().constData());
    return true;
  }

  QString error() const { return m_error; }

private:
  bool ambiguousCommandPayload() {
    RawConnection raw;
    std::string error;
    if (!raw.start(error)) return fail(QString::fromStdString(error));
    const auto opened = raw.open();
    if (!wait([&] { return opened->done.load(std::memory_order_acquire); }, "raw ambiguous-payload connection")) return false;
    if (!opened->accepted()) return fail("the ambiguous-payload subscriber could not open the service");
    // This would be an accepted first command if the service silently ignored
    // the wrong-typed inline field and selected its otherwise valid FD payload.
    const auto json = QJsonDocument(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("device")},
                                               {QStringLiteral("id"), m_device}}).toJson(QJsonDocument::Compact);
    MacCommandPayload payload;
    if (!payload.create(json, error)) return fail(QString::fromStdString(error));
    auto message = rawMessage("command");
    xpc_dictionary_set_fd(message, "command_fd", payload.descriptor());
    xpc_dictionary_set_uint64(message, "command_bytes", payload.size());
    xpc_dictionary_set_int64(message, "command", 1); // Present, but deliberately not XPC data.
    const auto rejected = raw.send(message);
    if (!wait([&] { return rejected->done.load(std::memory_order_acquire); },
              "exclusive FD command payload rejects wrong-typed inline alternative")) return false;
    return !rejected->accepted() || fail("the service accepted an FD command with an ambiguous wrong-typed inline payload");
  }

  bool bulkSceneOrdering() {
    // These are actual scene fields, not ignored padding. Scene A processes
    // the release list normally; scene B restores the fixed synthetic block
    // without introducing an expensive model corpus or many Cycles objects.
    QJsonArray released;
    constexpr int keys = 270000;
    for (int index = 0; index < keys; ++index)
      released.append(QString::fromLatin1(QByteArray::number(index, 16).rightJustified(64, '0')));
    const QJsonObject base{{QStringLiteral("cmd"), QStringLiteral("scene")},
                           {QStringLiteral("release"), released}};
    QJsonObject cleared = base;
    cleared.insert(QStringLiteral("sequence"), 901);
    cleared.insert(QStringLiteral("ground"), false);
    QJsonObject restored = base;
    restored.insert(QStringLiteral("sequence"), 902);
    restored.insert(QStringLiteral("test"), true);
    restored.insert(QStringLiteral("ground"), true);
    if (std::uint64_t(QJsonDocument(cleared).toJson(QJsonDocument::Compact).size()) <= kMacRenderMessageBytes ||
        std::uint64_t(QJsonDocument(restored).toJson(QJsonDocument::Compact).size()) <= kMacRenderMessageBytes)
      return fail("the scene ordering fixtures do not exceed the inline XPC capacity");

    first.sceneSequences.clear();
    first.commandTrace.clear();
    first.expected = 9012;
    first.current = {};
    first.requiredSceneSequence = 902;
    first.width = 160;
    first.height = 112;
    first.client.reserve(first.width, first.height);
    // No Qt event-loop turn: the first FD remains in flight locally while
    // the next FD and small controls are queued. Every scene boundary/ACK
    // must survive, including the view on each side of the second scene.
    // The pending allocation also exercises the previous deferred-view path,
    // which could incorrectly drop view9011 across the scene902 boundary.
    first.client.send(cleared);
    first.client.send(viewCommand(9011, first.width, first.height, 0));
    first.client.send(restored);
    first.client.send({{QStringLiteral("cmd"), QStringLiteral("environment")},
        {QStringLiteral("environment"), QJsonObject{{QStringLiteral("preset"), QStringLiteral("studio")},
                                                    {QStringLiteral("strength"), 0.5}}},
        {QStringLiteral("background"), QJsonObject{{QStringLiteral("mode"), QStringLiteral("environment")}}}});
    first.client.send(viewCommand(first.expected, first.width, first.height, 0));
    const std::vector<std::pair<QString, qint64>> expectedTrace{
        {QStringLiteral("scene"), 901}, {QStringLiteral("view"), 9011},
        {QStringLiteral("scene"), 902}, {QStringLiteral("view"), 9012}};
    if (!wait([this, &expectedTrace] {
                return first.complete() && first.sceneSequences.size() == 2 && first.commandTrace.size() >= expectedTrace.size();
              },
              "large scenes and small controls preserve production FIFO ordering")) return false;
    first.requiredSceneSequence = 0;
    if (first.sceneSequences != std::vector<int>{901, 902})
      return fail("a bulk scene was dropped, coalesced or applied out of order");
    if (first.commandTrace != expectedTrace)
      return fail("allocation-pending views crossed a scene boundary or were dropped from the applied-command trace");
    if (first.current.info.catcher != 0 || !second.complete())
      return fail("a small environment command overtook its bulk scene or affected another session");
    // Confirm the other live stream still renders after both large payloads.
    second.resize(213, second.width, second.height);
    return wait([this] { return second.complete(); }, "second stream after bulk scene commands");
  }

  bool malformedControl() {
    RawConnection raw;
    std::string error;
    if (!raw.start(error)) return fail(QString::fromStdString(error));
    const auto opened = raw.open();
    if (!wait([&] { return opened->done.load(std::memory_order_acquire); }, "raw malformed-control connection")) return false;
    if (!opened->accepted()) return fail("the raw connection could not open the authenticated service");
    auto memory = std::make_shared<FrameMemory>();
    auto surfaces = std::make_shared<MacFrameSurfaces>();
    if (!memory->create(sizeof(MacFrameControl), error) || !surfaces->create(16, 8, error)) return fail(QString::fromStdString(error));
    auto* control = new (memory->data()) MacFrameControl();
    control->capacityWidth = 16;
    control->capacityHeight = 8;
    control->generation = 1;
    control->published.store(kFrameSlots); // The only malformed field; all six planes and the FD are valid.
    const auto allocated = raw.send(rawAllocation(memory, surfaces, control->generation));
    if (!wait([&] { return allocated->done.load(std::memory_order_acquire); }, "out-of-range published-slot rejection")) return false;
    if (allocated->accepted()) return fail("the service accepted an out-of-range published slot in its allocation header");
    return true;
  }

  bool sameInfoPublication() {
    // Use the real service/renderer with a subscriber that deliberately holds
    // the final notification's ACK. An environment restart then changes pixels
    // while view, size, sample count and denoising metadata remain identical.
    RawConnection raw;
    std::string error;
    if (!raw.start(error)) return fail(QString::fromStdString(error));
    const auto opened = raw.open();
    if (!wait([&] { return opened->done.load(std::memory_order_acquire); }, "raw publication subscriber")) return false;
    if (!opened->accepted()) return fail("the publication subscriber could not open the authenticated service");

    bool ready = false, metalInteropUsed = false;
    bool nativeCatcherUsed = false, nativeCompositeUsed = false, nativeGraphicsUsed = false;
    bool catcherReferenceUsed = false, compositeReferenceUsed = false;
    qint64 maximumHalfUlps = 0;
    unsigned completed = 0;
    std::uint64_t heldPublication = 0, deliveredPublication = 0, lastAutoAck = 0;
    bool released = false;
    std::vector<RawConnection::Reply> acknowledgements;
    const auto acknowledge = [&](std::uint64_t publication) {
      auto message = rawMessage("consumed");
      xpc_dictionary_set_uint64(message, "generation", 1);
      xpc_dictionary_set_uint64(message, "publication", publication);
      acknowledgements.push_back(raw.send(message));
    };
    const auto pump = [&]() {
      if (raw.invalid() || raw.closed()) return fail("the publication subscriber received an invalid event or disconnected");
      for (const auto& event : raw.takeEvents()) {
        const auto name = event.value(QStringLiteral("event")).toString();
        if (name == QLatin1String("ready")) {
          const auto type = event.value(QStringLiteral("device")).toObject().value(QStringLiteral("type")).toString();
          const auto required = m_device == QLatin1String("metal") ? QStringLiteral("METAL") : QStringLiteral("CPU");
          if (type != required) return fail("the publication subscriber did not use the requested device");
          ready = true;
        } else if (name == QLatin1String("transport")) {
          if (event.value(QStringLiteral("kind")).toString() == QLatin1String("metal-buffer")) metalInteropUsed = true;
          if (event.value(QStringLiteral("kind")).toString() == QLatin1String("native-auxiliary")) {
            if (event.value(QStringLiteral("composite")).toBool()) nativeCompositeUsed = true;
            else nativeCatcherUsed = true;
            nativeGraphicsUsed = nativeGraphicsUsed || event.value(QStringLiteral("graphics")).toBool();
          }
          if (event.value(QStringLiteral("kind")).toString() == QLatin1String("native-auxiliary-reference")) {
            const auto difference = event.value(QStringLiteral("maxHalfUlps")).toInteger(-1);
            if (difference < 0) return fail("the native auxiliary reference omitted its half-precision comparison");
            maximumHalfUlps = std::max(maximumHalfUlps, difference);
            if (event.value(QStringLiteral("composite")).toBool()) compositeReferenceUsed = true;
            else catcherReferenceUsed = true;
          }
        } else if (name == QLatin1String("frame")) {
          const auto publication = static_cast<std::uint64_t>(event.value(QStringLiteral("publication")).toInteger());
          if (publication == 0) return fail("a frame notification omitted its publication identity");
          if (heldPublication == 0) {
            if (event.value(QStringLiteral("samples")).toInt() >= kSamples &&
                event.value(QStringLiteral("denoised")).toBool()) heldPublication = publication;
            else if (publication > lastAutoAck) { lastAutoAck = publication; acknowledge(publication); }
          } else if (publication > heldPublication) {
            if (!released) return fail("a second frame notification escaped without the held final ACK");
            deliveredPublication = std::max(deliveredPublication, publication);
          }
        } else if (name == QLatin1String("done")) {
          if (event.value(QStringLiteral("view")).toInteger() == 777 &&
              event.value(QStringLiteral("samples")).toInt() >= kSamples) ++completed;
        } else if (name == QLatin1String("error") || name == QLatin1String("device")) {
          return fail(event.value(QStringLiteral("message")).toString(QStringLiteral("publication render failed")));
        }
      }
      for (const auto& reply : acknowledgements) {
        if (reply->done.load(std::memory_order_acquire) && !reply->accepted()) return fail("the service rejected an exact publication ACK");
      }
      acknowledgements.erase(std::remove_if(acknowledgements.begin(), acknowledgements.end(),
          [](const auto& reply) { return reply->done.load(std::memory_order_acquire); }), acknowledgements.end());
      return true;
    };

    const auto device = raw.command({{QStringLiteral("cmd"), QStringLiteral("device")}, {QStringLiteral("id"), m_device}});
    if (!wait([&] { return pump() && ready && device->done.load(std::memory_order_acquire); }, "publication renderer ready")) return false;
    if (!device->accepted()) return fail("the publication renderer could not initialize");
    auto memory = std::make_shared<FrameMemory>();
    auto surfaces = std::make_shared<MacFrameSurfaces>();
    if (!memory->create(sizeof(MacFrameControl), error) || !surfaces->create(96, 64, error)) return fail(QString::fromStdString(error));
    auto* control = new (memory->data()) MacFrameControl();
    control->capacityWidth = 96;
    control->capacityHeight = 64;
    control->generation = 1;
    const auto allocation = raw.send(rawAllocation(memory, surfaces, 1));
    if (!wait([&] { return pump() && allocation->done.load(std::memory_order_acquire); }, "publication allocation ready")) return false;
    if (!allocation->accepted()) return fail("the publication renderer rejected a valid allocation");
    const auto samples = raw.command({{QStringLiteral("cmd"), QStringLiteral("samples")}, {QStringLiteral("count"), kSamples},
                                     {QStringLiteral("previews"), QJsonArray{kSamples}}});
    const auto scene = raw.command({{QStringLiteral("cmd"), QStringLiteral("scene")}, {QStringLiteral("test"), true},
                                   {QStringLiteral("ground"), true}, {QStringLiteral("sequence"), 1}});
    const auto view = raw.command(viewCommand(777, 96, 64, 0));
    if (!wait([&] { return pump() && heldPublication != 0 && completed >= 1 && acknowledgements.empty() &&
                          samples->done.load(std::memory_order_acquire) && scene->done.load(std::memory_order_acquire) &&
                          view->done.load(std::memory_order_acquire); },
              "held final publication before environment restart")) return false;
    if (!samples->accepted() || !scene->accepted() || !view->accepted()) return fail("the publication render rejected its commands");
    if (!nativeCatcherUsed || !catcherReferenceUsed)
      return fail("the publication renderer bypassed native catcher conversion or its float reference");
    if (m_device == QLatin1String("metal") && !metalInteropUsed) return fail("the publication renderer bypassed Metal buffer interop");
    Frame original;
    original.surface = claimMacFrame(surfaces, memory, original.info);
    if (!original.surface || original.info.view != 777 || original.info.width != 96 || original.info.height != 64 ||
        original.info.samples < kSamples || original.info.denoised == 0) return fail("the held publication was not a complete frame");
    const auto originalHash = checksum(original);
    const auto originalPublication = original.surface->publication();

    const auto environment = raw.command({{QStringLiteral("cmd"), QStringLiteral("environment")},
        {QStringLiteral("environment"), QJsonObject{{QStringLiteral("preset"), QStringLiteral("studio")},
                                                    {QStringLiteral("strength"), 0.25}}}});
    if (!wait([&] { return pump() && completed >= 2 && environment->done.load(std::memory_order_acquire); },
              "changed pixels while the first final notice stays in flight")) return false;
    if (!environment->accepted()) return fail("the publication renderer rejected its environment restart");
    Frame changed;
    changed.surface = claimMacFrame(surfaces, memory, changed.info);
    if (!changed.surface || !sameInfo(original.info, changed.info) || checksum(changed) == originalHash ||
        changed.surface->publication() <= originalPublication || checksum(original) != originalHash)
      return fail("the environment restart did not produce distinct pixels with identical frame metadata and retained ownership");
    const auto changedPublication = changed.surface->publication();
    released = true;
    acknowledge(heldPublication);
    if (!wait([&] { return pump() && deliveredPublication == changedPublication && acknowledgements.empty(); },
              "new publication after ACK with unchanged frame metadata", 5000)) return false;
    // A background-mode change needs the actual in-place environment composite,
    // unlike the preceding strength change, which deliberately kept FrameInfo
    // identical. Release the second lease so progressive batches have a slot.
    changed = {};
    const auto composite = raw.command({{QStringLiteral("cmd"), QStringLiteral("environment")},
        {QStringLiteral("environment"), QJsonObject{{QStringLiteral("preset"), QStringLiteral("studio")},
                                                    {QStringLiteral("strength"), 0.25}}},
        {QStringLiteral("background"), QJsonObject{{QStringLiteral("mode"), QStringLiteral("environment")}}}});
    if (!wait([&] { return pump() && completed >= 3 && nativeCompositeUsed && compositeReferenceUsed &&
                          composite->done.load(std::memory_order_acquire); },
              "native auxiliary environment composite")) return false;
    if (!composite->accepted()) return fail("the publication renderer rejected environment compositing");
    Frame composited;
    composited.surface = claimMacFrame(surfaces, memory, composited.info);
    if (!composited.surface || composited.info.catcher != 0 || composited.info.samples < kSamples ||
        composited.info.denoised == 0 || !hasRenderedPixels(composited) || checksum(original) != originalHash)
      return fail("native compositing produced an invalid frame or overwrote the retained original");
    std::printf("native auxiliary %s: catcher and environment composite used; graphics=%s, reference maxHalfUlps=%lld\n",
                m_device.toUtf8().constData(), nativeGraphicsUsed ? "true" : "false",
                static_cast<long long>(maximumHalfUlps));
    if (!first.complete() || !second.complete()) return fail("publication backpressure crossed normal session ownership");
    return true;
  }

  struct Session {
    Regression& test;
    MacRenderClient client{nullptr};
    Frame current;
    std::uint64_t expected = 0;
    std::uint64_t forbiddenFirst = 0, forbiddenLast = 0;
    int requiredSceneSequence = 0;
    std::vector<int> sceneSequences;
    std::vector<std::pair<QString, qint64>> commandTrace;
    int width = 96, height = 64;
    bool ready = false;
    bool metalInteropUsed = false;
    bool nativeCatcherUsed = false;
    bool catcherReferenceUsed = false;
    explicit Session(Regression& owner) : test(owner) {}
    ~Session() { client.stop(); }

    bool start(std::uint64_t sequence) {
      expected = sequence;
      width = 96;
      height = 64;
      ready = false;
      metalInteropUsed = false;
      nativeCatcherUsed = false;
      catcherReferenceUsed = false;
      forbiddenFirst = forbiddenLast = 0;
      requiredSceneSequence = 0;
      sceneSequences.clear();
      commandTrace.clear();
      current = {};
      if (!client.start([this](const QJsonObject& event) { receive(event); },
                        [this](const QString& reason) { test.fail(reason); })) return test.fail("could not start a native render client");
      return true;
    }
    void resize(std::uint64_t sequence, int requestedWidth, int requestedHeight) {
      expected = sequence;
      width = requestedWidth;
      height = requestedHeight;
      client.reserve(width, height);
      client.send(viewCommand(sequence, width, height, static_cast<double>(sequence % 7)));
    }
    bool complete() const {
      return ready && nativeCatcherUsed && catcherReferenceUsed &&
          (test.m_device != QLatin1String("metal") || metalInteropUsed) && current.surface &&
          current.info.view == expected && current.info.width == std::uint32_t(width) &&
          current.info.height == std::uint32_t(height) && current.info.fullWidth == std::uint32_t(width) &&
          current.info.fullHeight == std::uint32_t(height) && current.info.samples >= kSamples && current.info.denoised != 0;
    }
    void receive(const QJsonObject& event) {
      const auto name = event.value(QStringLiteral("event")).toString();
      if (name == QLatin1String("hello")) {
        if (event.value(QStringLiteral("transport")).toString() != QLatin1String("iosurface")) {
          test.fail("the service did not negotiate its private surface transport");
          return;
        }
        client.send({{QStringLiteral("cmd"), QStringLiteral("device")}, {QStringLiteral("id"), test.m_device}});
      } else if (name == QLatin1String("ready")) {
        const auto device = event.value(QStringLiteral("device")).toObject().value(QStringLiteral("type")).toString();
        const auto required = test.m_device == QLatin1String("metal") ? QStringLiteral("METAL") : QStringLiteral("CPU");
        if (device != required) { test.fail("the service did not execute on the requested device"); return; }
        ready = true;
        client.reserve(width, height);
        client.send({{QStringLiteral("cmd"), QStringLiteral("samples")}, {QStringLiteral("count"), kSamples},
                     {QStringLiteral("previews"), QJsonArray{kSamples}}});
        client.send({{QStringLiteral("cmd"), QStringLiteral("scene")}, {QStringLiteral("test"), true},
                     {QStringLiteral("ground"), true}, {QStringLiteral("sequence"), 1}});
        client.send(viewCommand(expected, width, height, static_cast<double>(expected % 7)));
      } else if (name == QLatin1String("transport")) {
        if (event.value(QStringLiteral("kind")).toString() == QLatin1String("metal-buffer")) metalInteropUsed = true;
        if (event.value(QStringLiteral("kind")).toString() == QLatin1String("native-auxiliary") &&
            !event.value(QStringLiteral("composite")).toBool()) nativeCatcherUsed = true;
        if (event.value(QStringLiteral("kind")).toString() == QLatin1String("native-auxiliary-reference") &&
            !event.value(QStringLiteral("composite")).toBool()) {
          const auto difference = event.value(QStringLiteral("maxHalfUlps")).toInteger(-1);
          if (difference < 0) { test.fail("the native catcher reference omitted its half-precision comparison"); return; }
          catcherReferenceUsed = true;
        }
      } else if (name == QLatin1String("command_applied")) {
        const auto command = event.value(QStringLiteral("cmd")).toString();
        const auto sequence = event.value(QStringLiteral("sequence")).toInteger();
        if ((command == QLatin1String("scene") && (sequence == 901 || sequence == 902)) ||
            (command == QLatin1String("view") && (sequence == 9011 || sequence == 9012))) {
          if (commandTrace.size() >= 8) { test.fail("too many commands in the bounded bulk ordering trace"); return; }
          commandTrace.emplace_back(command, sequence);
        }
      } else if (name == QLatin1String("scene")) {
        if (sceneSequences.size() >= 32) { test.fail("too many scene acknowledgements in the bounded test session"); return; }
        sceneSequences.push_back(event.value(QStringLiteral("sequence")).toInt());
      } else if (name == QLatin1String("frame")) {
        Frame incoming;
        if (!client.takeFrame(incoming)) return;
        if (!incoming.surface || !incoming.pixels.empty() || !incoming.catcher.empty()) {
          test.fail("the surface transport copied a frame into application vectors");
          return;
        }
        if (incoming.info.view == expected && requiredSceneSequence != 0 &&
            (sceneSequences.empty() || sceneSequences.back() != requiredSceneSequence)) {
          test.fail("a final view escaped before its preceding bulk scene acknowledgement");
          return;
        }
        if (forbiddenFirst != 0 && incoming.info.view >= forbiddenFirst && incoming.info.view <= forbiddenLast) {
          test.fail("an intermediate view escaped in-flight resize coalescing");
          return;
        }
        if (incoming.info.view == expected && incoming.info.width == std::uint32_t(width) &&
            incoming.info.height == std::uint32_t(height) && !hasRenderedPixels(incoming)) {
          test.fail("the real renderer produced an empty or non-finite surface frame");
          return;
        }
        if (test.m_device == QLatin1String("metal") && incoming.info.samples >= kSamples &&
            incoming.info.denoised != 0 && !metalInteropUsed) {
          test.fail("the Metal renderer copied a display frame without using buffer interop");
          return;
        }
        current = std::move(incoming);
      } else if (name == QLatin1String("error") || name == QLatin1String("device")) {
        test.fail(event.value(QStringLiteral("message")).toString(QStringLiteral("unexpected render device fallback")));
      }
    }
  };

  bool fail(const QString& reason) {
    if (m_error.isEmpty()) m_error = reason;
    return false;
  }
  bool wait(const std::function<bool()>& complete, const char* stage, qint64 maximum = kDeadlineMilliseconds) {
    std::fprintf(stderr, "START: %s\n", stage);
    if (!m_error.isEmpty()) return false;
    if (complete()) return true;
    const qint64 remaining = std::min(maximum, kDeadlineMilliseconds - m_elapsed.elapsed());
    if (remaining <= 0) return fail(QStringLiteral("deadline exceeded at %1").arg(QLatin1String(stage)));
    QEventLoop loop;
    QTimer poll;
    poll.setInterval(5);
    QObject::connect(&poll, &QTimer::timeout, &loop, [&] { if (!m_error.isEmpty() || complete()) loop.quit(); });
    QTimer::singleShot(static_cast<int>(remaining), &loop, &QEventLoop::quit);
    poll.start();
    loop.exec();
    if (!m_error.isEmpty()) return false;
    if (!complete()) return fail(QStringLiteral("deadline exceeded at %1").arg(QLatin1String(stage)));
    std::printf("PASS: %s\n", stage);
    return true;
  }

  QString m_device;
  QString m_error;
  QElapsedTimer m_elapsed;
  Session first;
  Session second;
};
} // namespace

int main(int argc, char** argv) {
  // Keep completed phases visible even if a blocking cleanup requires CTest
  // to terminate the owned helper before normal stdio flushing.
  std::setvbuf(stdout, nullptr, _IONBF, 0);
  QCoreApplication application(argc, argv);
  QString device = QStringLiteral("cpu");
  if (argc == 3 && std::strcmp(argv[1], "--device") == 0) device = QString::fromLocal8Bit(argv[2]);
  else if (argc != 1) { std::fprintf(stderr, "usage: test_mac_render_service [--device cpu|metal]\n"); return 2; }
  if (device != QLatin1String("cpu") && device != QLatin1String("metal")) return 2;
  // Keep denoising deterministic and session-local when the service is reused.
  qputenv("MITCAD_RENDER_TEST_DENOISER", "cpu");
  qputenv("MITCAD_RENDER_TEST_NATIVE_PASS_COMPARE", "1");
  qputenv("MITCAD_RENDER_TEST_COMMAND_TRACE", "1");
  qunsetenv("MITCAD_RENDER_TEST_DENOISER_FAILURE");
  qunsetenv("MITCAD_RENDER_TEST_DEVICE_FAILURE");
  Regression regression(device);
  if (regression.run()) return 0;
  std::fprintf(stderr, "embedded renderer regression failed: %s\n", regression.error().toUtf8().constData());
  return 1;
}
