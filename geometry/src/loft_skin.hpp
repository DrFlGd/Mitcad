// SPDX-License-Identifier: MIT
#pragma once

// Lofts with end conditions or rails (see loft.hpp). The sections are made
// compatible edge for edge; each side face is a surface whose rows
// interpolate the sections' poles together with the derivatives the end
// conditions ask for, then bent through the rails; the faces are sewn with
// the caps into a solid. Internal to the geometry library.

#include <optional>
#include <vector>

#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Dir.hxx>

#include "mitcad/geometry/loft.hpp"
#include "mitcad/geometry/shape.hpp"
#include "spine.hpp"

namespace mitcad::geometry::detail {

// A section to skin: its outer wire, or a vertex.
struct SkinSection {
  TopoDS_Shape shape;
  bool point = false;
  std::optional<gp_Dir> normal; // the plane of a sketch region
  ShapePtr body;                // a face of a body: the body and the face
  TopoDS_Face face;
};

// The solid through the sections (two or more, points only first or last),
// side faces named after the edges of `named` (one of the sections), caps
// `first` and `last` (empty for none). Names are not numbered yet. `closed`:
// from the last section back to the first, periodic and tangent-continuous,
// without caps; `solid` false: the side faces only, a shell. Without end
// conditions and rails the surfaces are the piecewise cubic Hermite ones
// (HermiteInterpolation).
ShapePtr skinned_loft(const std::vector<SkinSection>& sections, const LoftEnd& start, const LoftEnd& end,
                      const std::vector<Path>& rails, const NamedWire& named, const NameList& first,
                      const NameList& last, bool closed = false, bool solid = true);

} // namespace mitcad::geometry::detail
