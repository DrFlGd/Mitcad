// SPDX-License-Identifier: MIT
#pragma once
#ifdef __APPLE__
#include <IOSurface/IOSurfaceRef.h>
#include <cstddef>
#include <memory>
#include <string>

namespace mitcad::render {
// A Metal view of the same IOSurface storage, for the default unified-memory
// device. The display driver owns this handle until Cycles has replaced its
// external buffer reference and completed all GPU writes.
class MacFrameMetalBuffer {
public:
  MacFrameMetalBuffer();
  ~MacFrameMetalBuffer();
  MacFrameMetalBuffer(const MacFrameMetalBuffer&) = delete;
  MacFrameMetalBuffer& operator=(const MacFrameMetalBuffer&) = delete;
  bool wrap(IOSurfaceRef surface, std::string& error);
  void* handle() const;
  std::size_t size() const;
  static bool supported();
private:
  struct Impl;
  std::unique_ptr<Impl> m_impl;
};
} // namespace mitcad::render
#endif
