// SPDX-License-Identifier: MIT
#pragma once

// Where the handles of Fillet and Chamfer stand on an edge (mitcad#534;
// app/COMMANDS.md, "Fillet and Chamfer handles"): from the edge's middle,
// its direction there and the outward normals of its two faces. Plain
// OCCT math (gp); FilletCommands.cpp reads these from the body.

#include <array>
#include <optional>

#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>
#include <gp_Trsf.hxx>

namespace mitcad::cmd {

// An edge between two faces where a fillet or a chamfer takes it: its
// middle, its direction there (either way), the outward normals of its
// first and second face there, and whether the material lies between the
// faces (a convex edge) or around them (a concave one).
struct EdgeSides {
  gp_Pnt middle;
  gp_Dir tangent;
  std::array<gp_Dir, 2> normals{gp_Dir(0, 0, 1), gp_Dir(0, 0, 1)};
  bool convex = true;
};

// The edge as a placement shows it (a component's occurrence).
EdgeSides placed(const EdgeSides& sides, const gp_Trsf& placement);

// The angle between the faces' normals: 0 where the faces are tangent.
double bendAngle(const EdgeSides& sides);

// The way from the edge to the middle of a rounding or a bevel: the
// bisector of the normals, into the material at a convex edge, out of it
// at a concave one. None where the normals are opposite.
std::optional<gp_Dir> bendDirection(const EdgeSides& sides);

// An arrow handle: from `origin` along `direction`, its knob `factor` mm
// out per mm of the value (Manipulator's origin, direction and factor).
struct EdgeArrow {
  gp_Pnt origin;
  gp_Dir direction;
  double factor = 1.0;
};

// An equal distance chamfer's arrow: from the edge's middle to the middle
// of the bevel's face, distance * sin(bendAngle / 2) along bendDirection.
// None where the faces are tangent or the normals opposite.
std::optional<EdgeArrow> equalChamferArrow(const EdgeSides& sides);

// The arrow of a distance measured on face `side` (0: the edge's first
// face, 1: its second): along that face, square to the edge and away from
// it, the knob where the bevel meets the face. None where the faces are
// tangent (no side to go to).
std::optional<EdgeArrow> faceDistanceArrow(const EdgeSides& sides, int side);

// The face (0: the edge's first, 1: its second) that a chamfer's first
// distance and its angle are measured on, as the model picks it: the side
// the reference face is on (`onFirst`, `onSecond`: whether it borders the
// edge there), without a reference the first face; `flip` takes the other
// one. None when a reference borders the edge on neither side or on both.
std::optional<int> measuredSide(bool reference, bool onFirst, bool onSecond, bool flip);

} // namespace mitcad::cmd
