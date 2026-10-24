// SPDX-License-Identifier: MIT
#include "render/MacFrameXpcProbe.hpp"
#include "render/MacFrameSurface.hpp"

#include <Foundation/Foundation.h>
#include <IOSurface/IOSurfaceAPI.h>
#include <QCoreApplication>
#include <QEventLoop>
#include <QTimer>
#include <atomic>
#include <cstdio>
#include <memory>
#include <unistd.h>

namespace {
using namespace mitcad::render;
using namespace mitcad::render::xpcprobe;
struct Reply {
  std::atomic<bool> done{false};
  xpc_object_t value = nullptr;
  ~Reply() { if (value) xpc_release(value); }
};
using Pending = std::shared_ptr<Reply>;
Pending request(xpc_connection_t connection, xpc_object_t message) {
  auto pending = std::make_shared<Reply>();
  xpc_connection_send_message_with_reply(connection, message, dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0),
      ^(xpc_object_t reply) {
        pending->value = xpc_retain(reply);
        pending->done.store(true, std::memory_order_release);
      });
  xpc_release(message);
  return pending;
}
bool wait(const Pending& first, const Pending& second = {}) {
  const auto complete = [&] { return first->done.load(std::memory_order_acquire) &&
      (!second || second->done.load(std::memory_order_acquire)); };
  if (!complete()) {
    QEventLoop loop;
    QTimer poll;
    poll.setInterval(5);
    QObject::connect(&poll, &QTimer::timeout, &loop, [&] { if (complete()) loop.quit(); });
    QTimer::singleShot(5000, &loop, &QEventLoop::quit);
    poll.start();
    loop.exec();
  }
  if (!complete()) { std::fprintf(stderr, "bounded embedded-renderer reply timed out\n"); return false; }
  return true;
}
xpc_object_t command(const char* operation, std::uint64_t generation) {
  xpc_object_t message = xpc_dictionary_create_empty();
  xpc_dictionary_set_string(message, "op", operation);
  xpc_dictionary_set_uint64(message, "version", kVersion);
  xpc_dictionary_set_uint64(message, "generation", generation);
  return message;
}
struct Client {
  xpc_connection_t connection = nullptr;
  std::shared_ptr<MacFrameSurfaces> surfaces;
  std::uint64_t generation = 0;
  ~Client() { if (connection) { xpc_connection_cancel(connection); xpc_release(connection); } }
  bool open(bool requireWrongPeer = false) {
    connection = xpc_connection_create(kService, nullptr);
    if (!connection) return false;
    NSString* bundle = [NSBundle.mainBundle.bundlePath stringByAppendingPathComponent:
        @"Contents/XPCServices/org.mitcad.renderer.probe.xpc"];
    std::string error;
    const bool required = requireWrongPeer
        ? xpc_connection_set_peer_code_signing_requirement(connection, "identifier \"org.mitcad.invalid.renderer\"") == 0
        : requirePeer(connection, bundle.fileSystemRepresentation, error);
    xpc_connection_set_event_handler(connection, ^(xpc_object_t event) {
      if (xpc_get_type(event) == XPC_TYPE_ERROR) {
        const char* reason = xpc_dictionary_get_string(event, XPC_ERROR_KEY_DESCRIPTION);
        std::fprintf(stderr, "embedded-renderer connection: %s\n", reason ? reason : "invalid");
      }
    });
    xpc_connection_activate(connection); // Also required before disposal on failed validation.
    if (!required) std::fprintf(stderr, "%s\n", error.c_str());
    return required;
  }
  xpc_object_t allocation(std::uint64_t next, std::size_t count = kSurfaces) {
    auto message = command("allocate", next);
    xpc_dictionary_set_uint64(message, "width", kWidth);
    xpc_dictionary_set_uint64(message, "height", kHeight);
    xpc_object_t array = xpc_array_create_empty();
    for (std::size_t index = 0; index < count; ++index) {
      auto surface = IOSurfaceCreateXPCObject(surfaces->surface(static_cast<std::uint32_t>(index / kFramePlanes),
                                                               static_cast<std::uint32_t>(index % kFramePlanes)));
      xpc_array_append_value(array, surface);
      xpc_release(surface);
    }
    xpc_dictionary_set_value(message, "surfaces", array);
    xpc_release(array);
    return message;
  }
  bool create(std::uint64_t next) {
    auto allocation = std::make_shared<MacFrameSurfaces>();
    std::string error;
    if (!allocation->create(kWidth, kHeight, error)) return false;
    surfaces = std::move(allocation);
    generation = next;
    return true;
  }
  bool verify() const {
    std::string error;
    for (std::uint32_t slot = 0; slot < kFrameSlots; ++slot) {
      if (!surfaces->lock(slot, error)) return false;
      bool matches = true;
      for (std::uint32_t plane = 0; plane < kFramePlanes; ++plane) {
        const auto* pixels = surfaces->pixels(slot, plane);
        for (std::size_t pixel = 0; pixel < kWidth * kHeight * 4; ++pixel) {
          matches = matches && pixels[pixel] == marker(generation, slot * kFramePlanes + plane, pixel);
        }
      }
      if (!surfaces->unlock(slot, error) || !matches) return false;
    }
    return true;
  }
};
bool accepted(const Pending& reply, bool expected, std::uint64_t retained = kSurfaces) {
  if (!wait(reply) || xpc_get_type(reply->value) != XPC_TYPE_DICTIONARY ||
      xpc_dictionary_get_bool(reply->value, "ok") != expected ||
      xpc_dictionary_get_uint64(reply->value, "retained") != retained ||
      xpc_dictionary_get_int64(reply->value, "client_pid") != getpid() ||
      xpc_dictionary_get_int64(reply->value, "renderer_pid") <= 0 ||
      xpc_dictionary_get_int64(reply->value, "renderer_pid") == getpid()) {
    std::fprintf(stderr, "embedded renderer reply rejected or retained allocation count is wrong\n");
    return false;
  }
  return true;
}
} // namespace

int main(int argc, char** argv) {
  QCoreApplication application(argc, argv);
  @autoreleasepool {
    Client first, second;
    if (!first.open() || !second.open() || !first.create(1) || !second.create(2)) return 1;
    auto one = request(first.connection, first.allocation(first.generation));
    auto two = request(second.connection, second.allocation(second.generation));
    if (!wait(one, two) || !accepted(one, true) || !accepted(two, true)) return 2;
    one = request(first.connection, command("write", first.generation));
    two = request(second.connection, command("write", second.generation));
    if (!wait(one, two) || !accepted(one, true) || !accepted(two, true) || !first.verify() || !second.verify()) return 3;
    // Failed generations and descriptor counts preserve each connection's current allocation.
    if (!accepted(request(first.connection, first.allocation(0)), false) ||
        !accepted(request(first.connection, first.allocation(1)), false) ||
        !accepted(request(first.connection, first.allocation(3, 5)), false) ||
        !accepted(request(first.connection, command("write", second.generation)), false) ||
        !accepted(request(first.connection, command("write", first.generation)), true) || !first.verify()) return 4;
    // A replacement is scoped to one connection; retain its old allocation locally.
    const auto old = first.surfaces;
    if (!first.create(3) || !accepted(request(first.connection, first.allocation(first.generation)), true) ||
        !accepted(request(first.connection, command("write", first.generation)), true) || !first.verify() ||
        !second.verify() || old->pixels(0)[0] != marker(1, 0, 0)) return 5;
    if (!accepted(request(first.connection, command("close", first.generation)), true, 0) ||
        !accepted(request(first.connection, command("write", first.generation)), false, 0) ||
        !accepted(request(second.connection, command("write", second.generation)), true) || !second.verify() ||
        !accepted(request(second.connection, command("close", second.generation)), true, 0)) return 6;
    Client wrongSignature;
    if (!wrongSignature.open(true)) return 7;
    const auto refused = request(wrongSignature.connection, command("write", 1));
    if (!wait(refused) || refused->value != XPC_ERROR_PEER_CODE_SIGNING_REQUIREMENT) {
      std::fprintf(stderr, "XPC did not reject a mismatched renderer signing requirement\n");
      return 8;
    }
    std::puts("embedded actual-renderer IOSurface sessions passed; production transport unchanged");
    return 0;
  }
}
