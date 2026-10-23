// SPDX-License-Identifier: MIT
#pragma once

// Where the handles of Fillet and Chamfer stand on an edge (mitcad#534;
// app/COMMANDS.md, "Fillet and Chamfer handles"): from the edge's middle,
// its direction there and the outward normals of its two faces. The edge
// is read from the solid once (`edgeSides`); the rest is plain OCCT math
// (gp).

#include <array>
#include <optional>

#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
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

// The edge `edge` of `solid` between its faces `first` and `second`, at the
// middle of the edge's curve. Convex where a point just off the edge
// along first normal - second normal is outside the solid: that way leaves
// a convex edge's wedge of material and enters a concave edge's material
// (mitcad#571; along the normals' bisector it is outside at both). None
// where a face has no normal there or the edge no direction.
std::optional<EdgeSides> edgeSides(const TopoDS_Shape& solid, const TopoDS_Edge& edge, const TopoDS_Face& first,
                                   const TopoDS_Face& second);

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

// A constant radius fillet's arrow: along bendDirection, its knob at the
// middle of the rounding, radius * (1 / cos(bendAngle / 2) - 1) from the
// edge. None where the normals are opposite.
std::optional<EdgeArrow> filletArrow(const EdgeSides& sides);

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
