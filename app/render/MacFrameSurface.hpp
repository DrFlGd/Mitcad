// SPDX-License-Identifier: MIT
#pragma once

#ifdef __APPLE__
#include <array>
#include <atomic>
#include <cstdint>
#include <memory>
#include <mutex>
#include <string>
#include <IOSurface/IOSurfaceRef.h>
#include "render/RenderProtocol.hpp"

namespace mitcad::render {
class FrameMemory;

class MacFrameSurfaces {
public:
  ~MacFrameSurfaces();
  bool create(std::uint32_t width, std::uint32_t height, std::string& error);
  // Takes every passed CF reference, also on failure. Imports must describe
  // distinct packed RGhA planes of exactly the negotiated dimensions.
  bool adopt(std::array<IOSurfaceRef, kFrameSlots * kFramePlanes> surfaces,
             std::uint32_t width, std::uint32_t height, std::string& error);
  std::uint16_t* pixels(std::uint32_t slot, std::uint32_t plane = 0) const;
  IOSurfaceRef surface(std::uint32_t slot, std::uint32_t plane = 0) const;
  bool lock(std::uint32_t slot, std::string& error);
  bool unlock(std::uint32_t slot, std::string& error);
  // Local reader counts coalesce duplicate leases into shared-control bits.
  std::uint32_t readers(std::uint32_t slot) const { return m_readers[slot].load(); }
private:
  friend class MacFrameLease;
  std::array<IOSurfaceRef, kFrameSlots * kFramePlanes> m_surfaces{};
  std::array<std::atomic<std::uint32_t>, kFrameSlots> m_readers{};
  std::mutex m_readerMutex;
  std::shared_ptr<FrameMemory> m_readerControl;
};

// Keeps the surface allocation alive when its original owner replaces it.
// Independent leases of one slot can coexist in the local display process.
class MacFrameLease {
public:
  MacFrameLease(std::shared_ptr<MacFrameSurfaces> surfaces, std::uint32_t slot,
                std::shared_ptr<FrameMemory> control = {});
  ~MacFrameLease();
  MacFrameLease(const MacFrameLease&) = delete;
  MacFrameLease& operator=(const MacFrameLease&) = delete;
  std::uint16_t* pixels(std::uint32_t plane = 0) const;
  IOSurfaceRef surface(std::uint32_t plane = 0) const;
  std::uint64_t publication() const;
private:
  std::shared_ptr<MacFrameSurfaces> m_surfaces;
  std::shared_ptr<FrameMemory> m_control;
  std::uint32_t m_slot;
};

// Claims a stable published slot and copies its bounded metadata. The lease
// retains both the old mapping and surface allocation through resize/crash.
std::shared_ptr<MacFrameLease> claimMacFrame(const std::shared_ptr<MacFrameSurfaces>& surfaces,
                                           const std::shared_ptr<FrameMemory>& control, FrameInfo& info);

} // namespace mitcad::render
#endif
