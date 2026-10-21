// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/drawing.rs.

#include "bridge/shape.hpp"
#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/drawing.h.
struct DrawingView;
struct DrawingProjection;
struct DrawingHole;

// geometry::project_view of the shapes.
DrawingProjection project_view(const ShapeList& shapes, const DrawingView& view);

// geometry::round_holes of the shape.
rust::Vec<DrawingHole> round_holes(const geometry::Shape& shape);

} // namespace mitcad::bridge
