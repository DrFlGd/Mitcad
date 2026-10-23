// SPDX-License-Identifier: MIT
#include "bridge/face_revolve.hpp"

#include <cmath>
#include <stdexcept>
#include <string>

#include <gp_Ax1.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

#include "mitcad/geometry/face_revolve.hpp"
#include "mitcad_bridge/kernel/face_revolve.h"

namespace mitcad::bridge {

std::shared_ptr<geometry::Shape> revolve_face(rust::Str feature, const geometry::Shape& body, rust::Str face,
                                              rust::Str key, const FaceRevolution& revolution) {
  const auto& d = revolution.axis_direction;
  if (!(std::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) > 1.0e-12)) {
    throw std::invalid_argument("the revolve axis has no direction");
  }
  geometry::FaceRevolveSpec spec;
  spec.feature = std::string(feature);
  spec.face = std::string(face);
  spec.key = std::string(key);
  const auto& o = revolution.axis_origin;
  spec.axis = gp_Ax1(gp_Pnt(o[0], o[1], o[2]), gp_Dir(d[0], d[1], d[2]));
  spec.angle1 = revolution.angle1;
  if (revolution.two_sides) {
    spec.angle2 = revolution.angle2;
  }
  return geometry::revolve_face(body, spec);
}

} // namespace mitcad::bridge
