// SPDX-License-Identifier: MIT
#pragma once

#include <Geom2d_Curve.hxx>

#include "mitcad/geometry/sheet_domain.hpp"

namespace mitcad::geometry::detail {

// OCCT lines use distance; the public line support uses [0,1]. Other supported
// curves retain their native parameter. No sampled or approximate conversion.
struct Curve2 {
  occ::handle<Geom2d_Curve> curve;
  ParamSpan native_range;
  double parameter_scale = 1.0;
  double parameter_offset = 0.0;

  double native_parameter(double value) const {
    return parameter_offset + parameter_scale * value;
  }
  double public_parameter(double value) const {
    return (value - parameter_offset) / parameter_scale;
  }
};

Curve2 make_sheet_curve(const BoundedCurve2& input);
// Curve must be in the required chart. Range includes the traversal direction.
// A trimmed support is unwrapped without changing its native parameter.
BoundedCurve2 export_sheet_curve(const occ::handle<Geom2d_Curve>& curve, ParamSpan range);

// An exact plane isometry. Reflection is allowed; scaling/shearing is rejected.
occ::handle<Geom2d_Curve> transform_sheet_curve(const occ::handle<Geom2d_Curve>& curve,
                                              const Isometry2& transform, bool inverse = false);
void validate_sheet_isometry(const Isometry2& transform);

struct SheetFace {
  TopoDS_Face face;
  // The actual edge for each input loop/edge, preserving authoritative history.
  std::vector<std::vector<TopoDS_Edge>> edges;
};

// Builds the value-only material domain with shared endpoint vertices across
// all its wires. A hole tangent to a semantic port shares that exact vertex;
// independent MakeWire calls would otherwise create an invalid touching face.
// Only wire orientation is normalized: supports are never healed or sampled.
SheetFace make_sheet_face(const MaterialRegion& region, const Isometry2& transform,
                           double tolerance);

} // namespace mitcad::geometry::detail
