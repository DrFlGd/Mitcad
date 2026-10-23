// SPDX-License-Identifier: MIT
#include "bridge/face_extrude.hpp"

#include <cmath>
#include <stdexcept>
#include <string>

#include <gp_Dir.hxx>

#include "mitcad/geometry/face_extrude.hpp"
#include "mitcad_bridge/kernel/face_extrude.h"

namespace mitcad::bridge {

std::shared_ptr<geometry::Shape> extrude_face(rust::Str feature, const geometry::Shape& body, rust::Str face,
                                              rust::Str key, const FaceExtrusion& extrusion) {
  geometry::FaceExtrudeSpec spec;
  spec.feature = std::string(feature);
  spec.face = std::string(face);
  spec.key = std::string(key);
  spec.reversed = extrusion.reversed;
  spec.start_offset = extrusion.start;
  spec.distance1 = extrusion.side1;
  if (extrusion.two_sides) {
    spec.distance2 = extrusion.side2;
  }
  // A direction of its own and the sides' tapers (mitcad#541).
  if (extrusion.has_direction) {
    const auto& d = extrusion.direction;
    if (!(std::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) > 1.0e-12)) {
      throw std::invalid_argument("the extrusion direction has no length");
    }
    spec.direction = gp_Dir(d[0], d[1], d[2]);
  }
  spec.taper1 = extrusion.taper1;
  spec.taper2 = extrusion.taper2;
  return geometry::extrude_face(body, spec);
}

} // namespace mitcad::bridge
