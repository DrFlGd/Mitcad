// SPDX-License-Identifier: MIT
#pragma once

// Plastic part features (mitcad#459): the band of a lip or a groove along a
// rim face. Lengths are millimetres.

#include <string>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// The face `face` of `body` must be planar, with an outer loop and at least
// one inner loop that is an opening (the material does not go on over the
// face inside it: the cavity under a shelled body's rim, not the foot of a
// boss standing on the face). The band follows the outer loop (`outside`)
// or the largest opening; a face with two openings alike in size (the
// second more than half the first's area) has no rim and is refused. In
// the face's plane the band runs from the loop `width` into the face to
// `free` beyond the loop, away from the face, the offset loops' edges
// meeting in sharp corners; along the face's outward normal from `start`
// to `end` (offsets from its plane). Faces: band_side(<edge>) along the
// offset into the face, band_edge(<edge>) along the loop (<edge> the
// face's edge they follow, or its number in the loop when it has no name),
// band_top at `end`, band_bottom at `start`; a wall the offset made of no
// edge of the loop is band_wall(<k>). The body is not modified.
struct RimBandSpec {
  std::string feature;
  ShapePtr body;
  std::string face;
  bool outside = false;
  double width = 0.0;
  double free = 0.0;
  double start = 0.0;
  double end = 0.0;
};

ShapePtr rim_band(const RimBandSpec& spec);

} // namespace mitcad::geometry
