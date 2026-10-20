// SPDX-License-Identifier: MIT
#pragma once

// Emboss: profiles wrapped onto cylinders (mitcad#458). The planar emboss
// is an extrusion (extrude.hpp); on a cylinder the profiles are laid out by
// arc length around it, not projected, and grown radially.

#include <string>
#include <vector>

#include <gp_Ax1.hxx>

#include "mitcad/geometry/profile.hpp"
#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// The regions of a sketch whose plane is parallel to `axis`, wrapped onto
// the cylinder of `axis` and `radius` and grown radially to `to_radius`
// (larger: out from the axis; smaller: in towards it).
//
// The sketch side's radial direction points from the axis towards the
// sketch plane (the sketch normal for a plane through the axis). A sketch
// point at distance s across the axis (along axis × radial, from the line
// where the axis lies under the sketch) and z along it goes to the angle
// s / radius about the axis from the radial direction, at z: lengths
// around the cylinder of `radius` are the sketch's. The profiles must stay
// within one turn.
//
// Faces are named after `feature`, as an extrusion's:
//   <feature>:side(<segment>)  grown from a profile segment, radially
//   <feature>:start(<region>)  on the cylinder of `radius`
//   <feature>:end(<region>)    on the cylinder of `to_radius`
// Several regions are fused; the result may hold several solids.
struct EmbossWrapSpec {
  std::string feature;
  Frame frame;
  std::vector<Region> regions;
  gp_Ax1 axis;
  double radius = 0.0;
  double to_radius = 0.0;
};

// Throws std::invalid_argument for a sketch plane across the axis, sizes
// that are not positive and profiles that reach around the cylinder, and
// std::runtime_error when the solid cannot be built.
ShapePtr wrapped_emboss(const EmbossWrapSpec& spec);

} // namespace mitcad::geometry
