// SPDX-License-Identifier: MIT
#include "bridge/face_extrude.hpp"

#include <string>

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
  return geometry::extrude_face(body, spec);
}

} // namespace mitcad::bridge
