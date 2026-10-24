// SPDX-License-Identifier: MIT
#pragma once
#include <cstddef>
#include <cstdint>
namespace mitcad::render {
constexpr const char* kMacRenderService = "org.mitcad.renderer";
constexpr std::uint64_t kMacRenderProtocol = 1;
constexpr std::size_t kMacRenderMessageBytes = 16 * 1024 * 1024;
constexpr std::size_t kMacRenderPendingBytes = 32 * 1024 * 1024;
// Bulk JSON travels as an owned read-only file, with one service read in flight.
// Preserve a full int-sized Qt JSON document while bounding queued disk storage.
constexpr std::uint64_t kMacRenderPendingBulkBytes = 2ull * 2147483647ull;
constexpr unsigned kMacRenderConnections = 8;
} // namespace mitcad::render
