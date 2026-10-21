// SPDX-License-Identifier: MIT
#include "bridge/gear.hpp"

#include <string>
#include <vector>

#include "bridge/profile.hpp"
#include "mitcad/geometry/gear.hpp"
#include "mitcad_bridge/kernel/gear.h"

namespace mitcad::bridge {
namespace {

geometry::Region region_of(const Region& region) {
  geometry::Region out;
  out.name = std::string(region.name);
  for (const Loop& loop : region.loops) {
    geometry::Loop converted;
    for (const Segment& segment : loop.segments) {
      converted.segments.push_back(to_geometry(segment));
    }
    out.loops.push_back(std::move(converted));
  }
  return out;
}

} // namespace

std::shared_ptr<geometry::Shape> gear_sweep(rust::Str feature, const Frame& frame, const Region& region, double turn,
                                            double start, rust::Slice<const GearStep> steps, rust::Str start_name,
                                            rust::Str end_name) {
  geometry::GearSweepSpec spec;
  spec.feature = std::string(feature);
  spec.frame = to_geometry(frame);
  spec.region = region_of(region);
  spec.turn = turn;
  spec.start = start;
  for (const GearStep& step : steps) {
    spec.steps.push_back({step.length, step.twist, step.shift});
  }
  spec.start_name = std::string(start_name);
  spec.end_name = std::string(end_name);
  return geometry::gear_sweep(spec);
}

std::shared_ptr<geometry::Shape> gear_cone(rust::Str feature, const Frame& frame, const Region& region, double apex,
                                           double scale, const Loop& blank) {
  geometry::GearConeSpec spec;
  spec.feature = std::string(feature);
  spec.frame = to_geometry(frame);
  spec.region = region_of(region);
  spec.apex = apex;
  spec.scale = scale;
  for (const Segment& segment : blank.segments) {
    spec.blank.segments.push_back(to_geometry(segment));
  }
  return geometry::gear_cone(spec);
}

} // namespace mitcad::bridge
