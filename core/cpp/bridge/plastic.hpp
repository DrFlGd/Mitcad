// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/plastic.rs.

#include <memory>

#include "bridge/shape.hpp"
#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/plastic.h.
struct RimBand;

// geometry::rim_band on the first shape of `body`.
std::shared_ptr<geometry::Shape> rim_band(const ShapeList& body, const RimBand& spec);

} // namespace mitcad::bridge
