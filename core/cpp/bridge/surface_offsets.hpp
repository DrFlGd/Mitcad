// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/surface_offsets.rs.

#include <cstdint>
#include <memory>

#include "bridge/shape.hpp"
#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/surface_offsets.h.
struct RuledData;
struct OpeningData;

// geometry::offset_surface.
std::shared_ptr<geometry::Shape> offset_surface(rust::Str feature, const geometry::Shape& body,
                                                rust::Slice<const rust::String> faces, double distance,
                                                bool chain);
// geometry::ruled_surface.
std::shared_ptr<geometry::Shape> ruled_surface(rust::Str feature, const geometry::Shape& body,
                                               const RuledData& spec);
// geometry::extend_surface.
std::shared_ptr<geometry::Shape> extend_surface(const geometry::Shape& body, rust::Slice<const rust::String> edges,
                                                rust::Slice<const OpeningData> openings, double distance,
                                                std::uint8_t mode);

} // namespace mitcad::bridge
