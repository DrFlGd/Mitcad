// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/motion_contact.rs.

#include "bridge/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/motion_contact.h.
struct OverlapPair;

// geometry::overlap_volumes of the pairs of shapes in the list.
rust::Vec<double> overlap_volumes(const ShapeList& shapes, rust::Slice<const OverlapPair> pairs,
                                  double tolerance);

} // namespace mitcad::bridge
