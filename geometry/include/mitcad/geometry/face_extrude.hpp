// SPDX-License-Identifier: MIT
#pragma once

// A planar face of a body extruded as a profile (mitcad#535): Extrude
// with a body face instead of sketch regions.

#include <optional>
#include <string>

#include <gp_Dir.hxx>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// The face `face` of the body (one face, planar, with a surface) swept
// along its outward normal (against it when `reversed`): its bounded area
// with its holes, from the face moved `start_offset` along the outward
// normal, `distance1` along the sweep and, with `distance2`, that much
// against it (both > 0). The body is left as it is: the prism shares none
// of its sub-shapes.
//
// mitcad#541: with `direction` the sweep goes along it (against it when
// `reversed`) instead of the normal, the distances measured along it; it
// may not lie in the face's plane. `taper1` and `taper2` (radians, each
// side's) lean the sides as an extrusion's taper does: positive widens the
// face away from it (its outside grows, its holes shrink), along a
// direction by t tan(taper) in the face's plane at distance t. A face that
// is not planar (a curved face) needs a direction, and takes no taper and
// no start offset: its prism along the direction.
//
// Faces are named after `feature`, as an extrusion's:
//   side(<edge>)   swept from an edge of the face, by the edge's name in
//                  the body; an edge without a name (a surface body's free
//                  edge) by its index among the face's edges
//   start(<key>)   one side: the cap at the start; two sides: the far cap
//                  of side one
//   end(<key>)     the far cap (of side two with two sides)
// `key` is the face reference the feature stores (`face` is that reference
// as found in the body now).
struct FaceExtrudeSpec {
  std::string feature;
  std::string face;
  std::string key;
  bool reversed = false;
  double start_offset = 0.0;
  double distance1 = 0.0;
  std::optional<double> distance2;
  std::optional<gp_Dir> direction;
  double taper1 = 0.0;
  double taper2 = 0.0;
};

ShapePtr extrude_face(const Shape& body, const FaceExtrudeSpec& spec);

} // namespace mitcad::geometry
