// SPDX-License-Identifier: MIT
#include "bridge/motion_contact.hpp"

#include <array>
#include <cstddef>
#include <vector>

#include <gp_Trsf.hxx>

#include "mitcad/geometry/guard.hpp"
#include "mitcad/geometry/motion_contact.hpp"
#include "mitcad_bridge/kernel/motion_contact.h"

namespace mitcad::bridge {

namespace {

gp_Trsf placement(const std::array<double, 12>& r) {
  gp_Trsf trsf;
  trsf.SetValues(r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8], r[9], r[10], r[11]);
  return trsf;
}

} // namespace

rust::Vec<double> overlap_volumes(const ShapeList& shapes, rust::Slice<const OverlapPair> pairs,
                                  double tolerance) {
  std::vector<geometry::PlacedPair> placed;
  placed.reserve(pairs.size());
  for (const OverlapPair& pair : pairs) {
    geometry::PlacedPair p;
    p.a = shapes.at(pair.a).get();
    p.a_placement = placement(pair.a_placement);
    p.b = shapes.at(pair.b).get();
    p.b_placement = placement(pair.b_placement);
    placed.push_back(p);
  }
  std::vector<double> volumes;
  geometry::run_operation("contact", [&] { volumes = geometry::overlap_volumes(placed, tolerance); });
  rust::Vec<double> out;
  out.reserve(volumes.size());
  for (const double v : volumes) {
    out.push_back(v);
  }
  return out;
}

} // namespace mitcad::bridge
