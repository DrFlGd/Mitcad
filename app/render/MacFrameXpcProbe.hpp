// SPDX-License-Identifier: MIT
#pragma once

// Test-only protocol and public code-signing validation shared by both endpoints.
#include "render/MacXpcPeer.hpp"
#include <xpc/xpc.h>
#include <cstdint>
#include <string>
#include <vector>

namespace mitcad::render::xpcprobe {
constexpr const char* kService = "org.mitcad.renderer.probe";
constexpr std::uint32_t kVersion = 1;
constexpr std::uint32_t kWidth = 16;
constexpr std::uint32_t kHeight = 8;
constexpr std::size_t kSurfaces = 6;

inline std::uint16_t marker(std::uint64_t generation, std::size_t plane, std::size_t pixel) {
  return static_cast<std::uint16_t>((generation * 257 + plane * 31 + pixel) % 65535);
}

using mitcad::render::requirePeer;
} // namespace mitcad::render::xpcprobe
