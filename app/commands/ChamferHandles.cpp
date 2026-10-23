// SPDX-License-Identifier: MIT
#include "ChamferHandles.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRep_Tool.hxx>
#include <Geom2d_Curve.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Vec.hxx>

namespace mitcad::cmd {

std::optional<EdgeSides> edgeSides(const TopoDS_Shape& solid, const TopoDS_Edge& edge, const TopoDS_Face& first,
                                   const TopoDS_Face& second) {
  const BRepAdaptor_Curve curve(edge);
  const double t = (curve.FirstParameter() + curve.LastParameter()) / 2;
  gp_Pnt middle;
  gp_Vec tangent;
  curve.D1(t, middle, tangent);
  if (tangent.Magnitude() < 1.0e-9) {
    return std::nullopt;
  }
  gp_Vec normals[2];
  const TopoDS_Face* faces[2] = {&first, &second};
  for (std::size_t k = 0; k < 2; ++k) {
    const TopoDS_Face& face = *faces[k];
    double from = 0.0;
    double to = 0.0;
    const occ::handle<Geom2d_Curve> pcurve = BRep_Tool::CurveOnSurface(edge, face, from, to);
    if (pcurve.IsNull()) {
      return std::nullopt;
    }
    const gp_Pnt2d uv = pcurve->Value(t);
    const BRepAdaptor_Surface surface(face);
    BRepLProp_SLProps props(surface, uv.X(), uv.Y(), 1, 1.0e-9);
    if (!props.IsNormalDefined()) {
      return std::nullopt;
    }
    normals[k] = gp_Vec(props.Normal());
    if (face.Orientation() == TopAbs_REVERSED) {
      normals[k].Reverse();
    }
  }
  EdgeSides sides;
  sides.middle = middle;
  sides.tangent = gp_Dir(tangent);
  sides.normals = {gp_Dir(normals[0]), gp_Dir(normals[1])};
  // Locally a convex edge's material is where both faces' planes have it
  // (x.n0 < 0 and x.n1 < 0), a concave edge's where either has it. Along
  // n0 - n1, x.n0 = 1 - cos > 0 and x.n1 = cos - 1 < 0: outside the first
  // face's half and inside the second's, so out of a convex edge's
  // material and into a concave one's. Along the bisector n0 + n1 both are
  // positive, outside at either kind of edge: that probe took every edge
  // for convex and turned the concave arrows into the body (mitcad#571).
  const gp_Vec across = normals[0] - normals[1];
  if (across.Magnitude() > 1.0e-9) {
    // Off the edge by more than its tolerance, so the point is not on it.
    const double reach = std::max(1.0e-3, 10.0 * BRep_Tool::Tolerance(edge));
    const gp_Pnt probe = middle.Translated(across.Normalized() * reach);
    BRepClass3d_SolidClassifier inside(solid, probe, 1.0e-7);
    sides.convex = inside.State() != TopAbs_IN;
  }
  return sides;
}

EdgeSides placed(const EdgeSides& sides, const gp_Trsf& placement) {
  EdgeSides moved = sides;
  moved.middle = sides.middle.Transformed(placement);
  moved.tangent = sides.tangent.Transformed(placement);
  moved.normals = {sides.normals[0].Transformed(placement), sides.normals[1].Transformed(placement)};
  return moved;
}

double bendAngle(const EdgeSides& sides) { return sides.normals[0].Angle(sides.normals[1]); }

std::optional<gp_Dir> bendDirection(const EdgeSides& sides) {
  const gp_Vec sum = gp_Vec(sides.normals[0]) + gp_Vec(sides.normals[1]);
  if (sum.Magnitude() < 1.0e-9) {
    return std::nullopt;
  }
  const gp_Dir bisector(sum);
  return sides.convex ? bisector.Reversed() : bisector;
}

std::optional<EdgeArrow> filletArrow(const EdgeSides& sides) {
  const std::optional<gp_Dir> direction = bendDirection(sides);
  if (!direction) {
    return std::nullopt;
  }
  // The rounding's axis is radius / cos(angle / 2) from the edge on the
  // bisector, its middle a radius nearer.
  return EdgeArrow{sides.middle, *direction, 1.0 / std::cos(bendAngle(sides) / 2.0) - 1.0};
}

std::optional<EdgeArrow> equalChamferArrow(const EdgeSides& sides) {
  const std::optional<gp_Dir> direction = bendDirection(sides);
  // The bevel's face joins the points `distance` out on each face; its
  // middle lies on the bisector, distance * sin(angle / 2) from the edge.
  const double factor = std::sin(bendAngle(sides) / 2.0);
  if (!direction || factor < 1.0e-6) {
    return std::nullopt;
  }
  return EdgeArrow{sides.middle, *direction, factor};
}

std::optional<EdgeArrow> faceDistanceArrow(const EdgeSides& sides, int side) {
  if (side != 0 && side != 1) {
    return std::nullopt;
  }
  const std::size_t mine = side == 0 ? 0U : 1U;
  const gp_Vec normal(sides.normals[mine]);
  const gp_Vec other(sides.normals[1U - mine]);
  gp_Vec along = normal.Crossed(gp_Vec(sides.tangent));
  if (along.Magnitude() < 1.0e-9) {
    return std::nullopt;
  }
  along.Normalize();
  // At a convex edge a face runs away from the outside of the other face,
  // at a concave one towards it.
  const double lean = along.Dot(other);
  if (std::abs(lean) < 1.0e-6) {
    return std::nullopt;
  }
  if ((lean > 0.0) == sides.convex) {
    along.Reverse();
  }
  return EdgeArrow{sides.middle, gp_Dir(along), 1.0};
}

std::optional<int> measuredSide(bool reference, bool onFirst, bool onSecond, bool flip) {
  if (!reference) {
    return flip ? 1 : 0;
  }
  if (onFirst == onSecond) {
    return std::nullopt;
  }
  return (onFirst != flip) ? 0 : 1;
}

} // namespace mitcad::cmd
