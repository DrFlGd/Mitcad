// SPDX-License-Identifier: MIT
#include "ChamferHandles.hpp"

#include <cmath>
#include <cstddef>

#include <gp_Vec.hxx>

namespace mitcad::cmd {

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
