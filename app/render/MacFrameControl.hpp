// SPDX-License-Identifier: MIT
#pragma once

// The private Mac transport's control mapping. The POSIX/CLI pixel protocol
// remains RenderProtocol v8; this header never contains pixel data.
#include "render/RenderProtocol.hpp"

namespace mitcad::render {
constexpr std::uint32_t kMacFrameMagic = 0x4d534631; // "MSF1"
constexpr std::uint32_t kMacFrameVersion = 1;
constexpr std::uint32_t kMacFrameReaderMask = (1u << kFrameSlots) - 1u;
constexpr std::uint32_t kMacFrameMaxDimension = 16384;
constexpr std::size_t kMacFrameMaxPlaneBytes = std::size_t(256) * 1024 * 1024;

struct MacFrameControl {
  std::uint32_t magic = kMacFrameMagic;
  std::uint32_t version = kMacFrameVersion;
  std::uint32_t capacityWidth = 0;
  std::uint32_t capacityHeight = 0;
  std::uint64_t generation = 0;
  std::atomic<std::uint32_t> published{kNoSlot};
  std::atomic<std::uint32_t> readingMask{0};
  // One producer advances the counter. Slot sequences stay immutable while
  // published or retained; zero means that slot has never been published.
  std::uint64_t publicationCounter = 0;
  std::uint64_t publications[kFrameSlots]{};
  FrameInfo frames[kFrameSlots];
};

// Darwin may report shm_open's file size rounded to a VM page even after a
// smaller ftruncate. The wire header is fixed; permit at most that one rounded
// allocation, never arbitrary extra mapped pages.
constexpr std::size_t macFrameControlStorageBytes(std::size_t page) {
  if (page == 0) return 0;
  const auto remainder = sizeof(MacFrameControl) % page;
  return sizeof(MacFrameControl) + (remainder == 0 ? 0 : page - remainder);
}
constexpr bool validMacFrameStorage(std::size_t bytes, std::size_t page) {
  return page != 0 && bytes >= sizeof(MacFrameControl) && bytes <= macFrameControlStorageBytes(page);
}

inline bool validMacFrameDimensions(std::uint32_t width, std::uint32_t height) {
  return width > 0 && height > 0 && width <= kMacFrameMaxDimension && height <= kMacFrameMaxDimension &&
      width % 8 == 0 && planeBytes(width, height) <= kMacFrameMaxPlaneBytes;
}

inline bool validMacFrameControl(const MacFrameControl& control, std::size_t bytes) {
  if (bytes < sizeof(MacFrameControl)) return false;
  const auto published = control.published.load();
  return control.magic == kMacFrameMagic &&
      control.version == kMacFrameVersion && control.generation != 0 &&
      validMacFrameDimensions(control.capacityWidth, control.capacityHeight) &&
      (control.readingMask.load() & ~kMacFrameReaderMask) == 0 &&
      (published == kNoSlot || published < kFrameSlots);
}

// One writer per allocation. Readers publish their bit before rechecking the
// published slot; the writer never recycles either a published or retained slot.
inline std::uint32_t macWritableSlot(const MacFrameControl& control) {
  const auto published = control.published.load();
  const auto readers = control.readingMask.load();
  for (std::uint32_t slot = 0; slot < kFrameSlots; ++slot) {
    if (slot != published && (readers & (1u << slot)) == 0) return slot;
  }
  return kNoSlot;
}
} // namespace mitcad::render
