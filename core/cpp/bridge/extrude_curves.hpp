// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/extrude_curves.rs: thin extrusions along
// chains of sketch curves (mitcad#463).

#include <cstddef>
#include <memory>

#include "bridge/shape.hpp"
#include "mitcad/geometry/shape.hpp"
#include "mitcad_bridge/kernel/profile.h"
#include "mitcad_bridge/kernel/profile_feature.h"
#include "mitcad_bridge/kernel/sweeps.h"
#include "rust/cxx.h"

namespace mitcad::bridge {

// geometry::extrude_feature of the walls along the chains: `chains` holds
// the number of curves of each chain, in the order of `curves`, and `keys`
// each chain's key.
std::shared_ptr<geometry::Shape> extrude_curves(rust::Str feature, const Frame& frame,
                                                const ExtrudeFeatureInput& input,
                                                rust::Slice<const SweepCurve> curves,
                                                rust::Slice<const std::size_t> chains,
                                                rust::Slice<const rust::String> keys,
                                                const ShapeList& bodies);

} // namespace mitcad::bridge
