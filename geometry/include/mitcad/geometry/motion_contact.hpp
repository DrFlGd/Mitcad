// SPDX-License-Identifier: MIT
#pragma once

// Contact sets: bodies that stop each other while joints move (mitcad#454).
// The model samples a motion and asks, at each sample, how far the bodies
// of a contact set pass through each other where the motion puts them.

#include <vector>

#include <gp_Trsf.hxx>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// Two bodies, each placed by a rigid transform into common coordinates.
struct PlacedPair {
  const Shape* a = nullptr;
  gp_Trsf a_placement;
  const Shape* b = nullptr;
  gp_Trsf b_placement;
};

// For each pair, the volume (mm^3) the placed bodies have in common: 0 when
// they are apart or only touch. Boxes more than `tolerance` (mm) apart cost
// no boolean; the common of the others is OCCT's boolean with `tolerance`
// as its fuzzy value, so that faces that lie on each other within it give
// no volume. The inputs are placed by locations, never modified. Throws
// std::runtime_error when a boolean fails.
std::vector<double> overlap_volumes(const std::vector<PlacedPair>& pairs, double tolerance);

} // namespace mitcad::geometry
