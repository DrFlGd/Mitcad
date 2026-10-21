// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/gear.rs (mitcad#472). The profile types
// are the ones the profile bridge generates.

#include <memory>

#include "mitcad/geometry/shape.hpp"
#include "mitcad_bridge/kernel/profile.h"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/gear.h.
struct GearStep;

// geometry::gear_sweep.
std::shared_ptr<geometry::Shape> gear_sweep(rust::Str feature, const Frame& frame, const Region& region, double turn,
                                            double start, rust::Slice<const GearStep> steps, rust::Str start_name,
                                            rust::Str end_name);

// geometry::gear_cone.
std::shared_ptr<geometry::Shape> gear_cone(rust::Str feature, const Frame& frame, const Region& region, double apex,
                                           double scale, const Loop& blank);

} // namespace mitcad::bridge
