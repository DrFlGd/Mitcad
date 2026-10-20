// SPDX-License-Identifier: MIT
#include "bridge/extrude_curves.hpp"

#include <stdexcept>
#include <string>
#include <utility>

#include "bridge/profile_feature.hpp"
#include "bridge/sweeps.hpp"
#include "mitcad/geometry/extrude.hpp"
#include "mitcad_bridge/kernel/extrude_curves.h"

namespace mitcad::bridge {

std::shared_ptr<geometry::Shape> extrude_curves(rust::Str feature, const Frame& frame,
                                                const ExtrudeFeatureInput& input,
                                                rust::Slice<const SweepCurve> curves,
                                                rust::Slice<const std::size_t> chains,
                                                rust::Slice<const rust::String> keys,
                                                const ShapeList& bodies) {
  geometry::ExtrudeFeatureSpec spec = extrude_spec(feature, frame, {}, input, bodies);
  std::size_t next = 0;
  for (const std::size_t count : chains) {
    if (count > curves.size() - next) {
      throw std::invalid_argument("a chain of curves runs past the curves given");
    }
    geometry::Path chain;
    for (std::size_t i = 0; i < count; ++i, ++next) {
      chain.push_back(path_curve_of(curves[next]));
    }
    spec.chains.push_back(std::move(chain));
  }
  for (const rust::String& key : keys) {
    spec.chain_keys.push_back(std::string(key));
  }
  return geometry::extrude_feature(spec);
}

} // namespace mitcad::bridge
