// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/surface_edits.rs. The profile, tool and
// curve types are the ones their bridges generate.

#include <memory>

#include "bridge/shape.hpp"
#include "mitcad/geometry/shape.hpp"
#include "mitcad_bridge/kernel/faceops.h"
#include "mitcad_bridge/kernel/profile.h"
#include "mitcad_bridge/kernel/sweeps.h"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/surface_edits.h.
struct TrimData;

// geometry::trim_surface.
std::shared_ptr<geometry::Shape> trim_surface(const geometry::Shape& body, const ToolSpec& tool,
                                              const ShapeList& tool_shapes, const Frame& frame,
                                              rust::Slice<const Region> regions,
                                              rust::Slice<const SweepCurve> curves, const TrimData& data);
// geometry::untrim_surface.
std::shared_ptr<geometry::Shape> untrim_surface(rust::Str feature, const geometry::Shape& body, rust::Str face,
                                                bool holes, bool outer, bool in_place);
// geometry::unstitch: what is left first (an empty pointer when nothing
// is), then each face taken out.
std::unique_ptr<ShapeList> unstitch(const geometry::Shape& body, rust::Slice<const rust::String> faces);
// geometry::reverse_faces.
std::shared_ptr<geometry::Shape> reverse_faces(const geometry::Shape& body, rust::Slice<const rust::String> faces);

} // namespace mitcad::bridge
