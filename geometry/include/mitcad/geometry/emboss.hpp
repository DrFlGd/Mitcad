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
//
// On a cone (mitcad#473: `half_angle` not zero), the sketch is the cone's
// development laid against it along the generator under the sketch's
// origin: `axis` runs from the apex where the radius grows, `radius` is the
// cone's radius at the sketch origin's height along the axis (ρ0 = radius
// / sin(half_angle) along the generator from the apex), and `to_radius -
// radius` is how far the profiles grow along the cone's normal (outwards
// when positive). A sketch point at s across the axis and t along it (from
// the origin's height) goes to the point of the cone at the distance
// hypot(s, t + ρ0) from the apex along a generator turned by
// atan2(s, t + ρ0) / sin(half_angle) about the axis: lengths and angles on
// the cone are the sketch's. The walls stand along the cone's normals.
struct EmbossWrapSpec {
  std::string feature;
  Frame frame;
  std::vector<Region> regions;
  gp_Ax1 axis;
  double radius = 0.0;
  double to_radius = 0.0;
  double half_angle = 0.0; // radians: 0 for a cylinder
};

// Throws std::invalid_argument for a sketch plane across the axis, sizes
// that are not positive and profiles that reach around the cylinder, and
// std::runtime_error when the solid cannot be built.
ShapePtr wrapped_emboss(const EmbossWrapSpec& spec);

} // namespace mitcad::geometry
