// SPDX-License-Identifier: MIT
// The renderer itself is the embedded service; there is no sibling broker.
#include "render/MacFrameXpcProbe.hpp"
#include "render/RenderProtocol.hpp"

#include <Foundation/Foundation.h>
#include <IOSurface/IOSurfaceAPI.h>
#include <Metal/Metal.h>
#include <QCoreApplication>
#include <array>
#include <atomic>
#include <cstdio>
#include <cstring>
#include <memory>
#include <unistd.h>

namespace {
using namespace mitcad::render;
using namespace mitcad::render::xpcprobe;
std::atomic<unsigned> g_connections{0};

struct Session {
  xpc_connection_t connection = nullptr; // Owned by XPC while its handler retains this session.
  dispatch_source_t deadline = nullptr;
  std::array<IOSurfaceRef, kSurfaces> surfaces{};
  std::uint64_t generation = 0;
  ~Session() {
    clear();
    if (deadline) { dispatch_source_cancel(deadline); dispatch_release(deadline); }
    --g_connections;
  }
  void clear() {
    for (auto& surface : surfaces) { if (surface) CFRelease(surface); surface = nullptr; }
  }
  bool allocate(xpc_object_t message) {
    const auto next = xpc_dictionary_get_uint64(message, "generation");
    xpc_object_t array = xpc_dictionary_get_value(message, "surfaces");
    if (next <= generation || next == 0 || !array || xpc_get_type(array) != XPC_TYPE_ARRAY ||
        xpc_array_get_count(array) != kSurfaces || xpc_dictionary_get_uint64(message, "width") != kWidth ||
        xpc_dictionary_get_uint64(message, "height") != kHeight) return false;
    std::array<IOSurfaceRef, kSurfaces> imported{};
    bool valid = true;
    for (std::size_t index = 0; index < kSurfaces; ++index) {
      imported[index] = IOSurfaceLookupFromXPCObject(xpc_array_get_value(array, index));
      const auto surface = imported[index];
      valid = valid && surface && IOSurfaceGetWidth(surface) == kWidth && IOSurfaceGetHeight(surface) == kHeight &&
          IOSurfaceGetBytesPerElement(surface) == kBytesPerPixel &&
          IOSurfaceGetBytesPerRow(surface) == kWidth * kBytesPerPixel &&
          IOSurfaceGetPixelFormat(surface) == 0x52476841 && IOSurfaceGetAllocSize(surface) >= planeBytes(kWidth, kHeight);
      for (std::size_t previous = 0; previous < index && surface; ++previous) {
        if (imported[previous] && IOSurfaceGetID(imported[previous]) == IOSurfaceGetID(surface)) valid = false;
      }
    }
    if (!valid) { for (const auto surface : imported) { if (surface) CFRelease(surface); } return false; }
    clear();
    surfaces = imported;
    generation = next;
    return true;
  }
  bool write(std::uint64_t requested) {
    if (generation == 0 || requested != generation || !surfaces[0]) return false;
    for (std::size_t index = 0; index < kSurfaces; ++index) {
      const auto surface = surfaces[index];
      if (IOSurfaceLock(surface, 0, nullptr) != KERN_SUCCESS) return false;
      auto* pixels = static_cast<std::uint16_t*>(IOSurfaceGetBaseAddress(surface));
      if (pixels) {
        for (std::size_t pixel = 0; pixel < kWidth * kHeight * 4; ++pixel) pixels[pixel] = marker(generation, index, pixel);
      }
      const bool unlocked = IOSurfaceUnlock(surface, 0, nullptr) == KERN_SUCCESS;
      if (!pixels || !unlocked) return false;
    }
    return true;
  }
  void receive(xpc_object_t message) {
    if (xpc_get_type(message) == XPC_TYPE_ERROR) {
      clear();
      dispatch_source_cancel(deadline);
      return;
    }
    if (xpc_get_type(message) != XPC_TYPE_DICTIONARY) { xpc_connection_cancel(connection); return; }
    // One resettable timer per connection bounds retained allocations and idle clients.
    dispatch_source_set_timer(deadline, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC), DISPATCH_TIME_FOREVER, 0);
    xpc_object_t reply = xpc_dictionary_create_reply(message);
    if (!reply) { xpc_connection_cancel(connection); return; }
    bool ok = false;
    const char* operation = xpc_dictionary_get_string(message, "op");
    if (xpc_dictionary_get_uint64(message, "version") == kVersion && operation) {
      if (std::strcmp(operation, "allocate") == 0) ok = allocate(message);
      else if (std::strcmp(operation, "write") == 0) ok = write(xpc_dictionary_get_uint64(message, "generation"));
      else if (std::strcmp(operation, "close") == 0) { clear(); ok = true; }
    }
    xpc_dictionary_set_bool(reply, "ok", ok);
    xpc_dictionary_set_uint64(reply, "generation", generation);
    xpc_dictionary_set_uint64(reply, "retained", surfaces[0] ? kSurfaces : 0);
    xpc_dictionary_set_int64(reply, "renderer_pid", getpid());
    xpc_dictionary_set_int64(reply, "client_pid", xpc_connection_get_pid(connection));
    xpc_connection_send_message(connection, reply);
    xpc_release(reply);
  }
};

// xpc_main accepts a C function pointer, unlike connection event handlers.
void acceptConnection(xpc_connection_t connection) {
    @autoreleasepool {
      if (xpc_connection_get_euid(connection) != geteuid()) { xpc_connection_cancel(connection); return; }
      if (g_connections.fetch_add(1) >= 8) {
        --g_connections;
        xpc_connection_cancel(connection);
        return;
      }
      auto session = std::make_shared<Session>();
      session->connection = connection;
      NSString* app = NSBundle.mainBundle.bundlePath;
      for (int index = 0; index < 3; ++index) app = app.stringByDeletingLastPathComponent;
      std::string error;
      if (!requirePeer(connection, app.fileSystemRepresentation, error)) {
        std::fprintf(stderr, "%s\n", error.c_str());
        xpc_connection_cancel(connection);
        return;
      }
      dispatch_queue_t queue = dispatch_queue_create("org.mitcad.renderer.probe.session", DISPATCH_QUEUE_SERIAL);
      xpc_connection_set_target_queue(connection, queue);
      session->deadline = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, queue);
      std::weak_ptr<Session> weak = session;
      dispatch_source_set_event_handler(session->deadline, ^{
        if (auto active = weak.lock()) { active->clear(); xpc_connection_cancel(active->connection); }
      });
      dispatch_source_set_timer(session->deadline, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC), DISPATCH_TIME_FOREVER, 0);
      dispatch_resume(session->deadline);
      xpc_connection_set_event_handler(connection, ^(xpc_object_t message) { session->receive(message); });
      xpc_connection_activate(connection);
      dispatch_release(queue);
    }
}
} // namespace

int main(int argc, char** argv) {
  QCoreApplication application(argc, argv);
  @autoreleasepool {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (device == nil) return 1;
    std::fprintf(stderr, "embedded actual renderer initialized Qt/Metal: pid=%ld device=%s\n",
                 long(getpid()), [[device name] UTF8String]);
    [device release];
  }
  xpc_main(acceptConnection);
}
