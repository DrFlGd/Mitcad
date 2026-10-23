// SPDX-License-Identifier: MIT
#pragma once

// A planar face of a body revolved as a profile (mitcad#539): Revolve with
// a body face instead of sketch regions, as face_extrude.hpp is for
// Extrude.

#include <optional>
#include <string>

#include <gp_Ax1.hxx>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// The face `face` of the body (one face, planar, with a surface), its
// bounded area with its holes, turned about `axis`: side one by `angle1`
// (radians, > 0, the right hand turn about the axis direction) and, with
// `angle2`, side two that much the other way (>= 0); together at most a
// full turn. The face must not cross the axis (it may touch it), nor may
// the axis pass through it. The body is left as it is: the solid shares
// none of its sub-shapes.
//
// Faces are named after `feature`, as a revolution's of sketch profiles:
//   side(<edge>)   swept from an edge of the face off the axis, by the
//                  edge's name in the body; an edge without a name (a
//                  surface body's free edge) by its index among the face's
//                  edges
//   start(<key>)   one side: the face where it is; two sides: the end of
//                  side one
//   end(<key>)     the far end (of side two with two sides)
// A full turn has no caps. `key` is the face reference the feature stores
// (`face` is that reference as found in the body now).
struct FaceRevolveSpec {
  std::string feature;
  std::string face;
  std::string key;
  gp_Ax1 axis;
  double angle1 = 0.0;
  std::optional<double> angle2;
};

ShapePtr revolve_face(const Shape& body, const FaceRevolveSpec& spec);

} // namespace mitcad::geometry
