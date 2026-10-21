// SPDX-License-Identifier: MIT
#pragma once

// Gears (mitcad#472): the toothed profiles the model works out
// (core/model/src/features/gear_profile.rs) swept along a gear's axis,
// straight or turned about it (helical teeth), or ruled to a bevel gear's
// apex. The segments' names are the faces' roles ("tip(3)"): a face swept
// from a segment is named "<feature>:<segment name>".

#include <string>
#include <vector>

#include "mitcad/geometry/profile.hpp"
#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// One stretch of a gear sweep: `length` along the axis (the frame's
// normal through its origin) while the profile turns `twist` radians about
// it (right-handed about the normal) and moves `shift` along the frame's x
// axis.
struct GearStep {
  double length = 0.0;
  double twist = 0.0;
  double shift = 0.0;
};

// The region's loops (the first the outer boundary, the others holes),
// turned by `turn` about the axis and moved `start` along it, swept by the
// steps one after another into one solid: a prism for one straight step,
// else faces whose curves follow each point's screw motion (within about
// 1e-7 mm), sewn. Faces are named "<feature>:<segment name>" (pieces of
// several steps "#k"), the caps "<feature>:<start_name>" and
// "<feature>:<end_name>".
struct GearSweepSpec {
  std::string feature;
  Frame frame;
  Region region;
  double turn = 0.0;
  double start = 0.0;
  std::vector<GearStep> steps;
  std::string start_name = "start";
  std::string end_name = "end";
};

// Throws std::invalid_argument for steps that are not positive and
// std::runtime_error when the solid cannot be built.
ShapePtr gear_sweep(const GearSweepSpec& spec);

// A straight bevel gear: the region's outer loop in the frame's plane (the
// heel section) ruled to its copy scaled by `scale` about the apex (on the
// axis, `apex` along the frame's normal), the solid's common part with the
// blank: the closed loop `blank` in (radius along the frame's x axis,
// height along the normal) coordinates turned about the axis. Faces are
// named after the segments of both; the section's caps "<feature>:back"
// (the heel's plane) and "<feature>:front".
struct GearConeSpec {
  std::string feature;
  Frame frame;
  Region region;
  double apex = 0.0;
  double scale = 0.0;
  Loop blank;
};

ShapePtr gear_cone(const GearConeSpec& spec);

} // namespace mitcad::geometry
