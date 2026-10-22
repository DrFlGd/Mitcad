// SPDX-License-Identifier: MIT
#include "bridge/move_faces.hpp"

#include <string>
#include <vector>

#include <gp_Trsf.hxx>

#include "mitcad/geometry/move_faces.hpp"
#include "mitcad_bridge/kernel/move_faces.h"

namespace mitcad::bridge {

std::shared_ptr<geometry::Shape> move_faces(rust::Str feature, const geometry::Shape& body,
                                            rust::Slice<const rust::String> faces, const Affine& motion) {
  std::vector<std::string> names;
  for (const rust::String& face : faces) {
    names.emplace_back(face);
  }
  const auto& m = motion.linear;
  const auto& t = motion.translation;
  gp_Trsf trsf;
  // (SetValues checks that the linear part is a rotation, at its own
  // tolerance; the model checked the motion is rigid.)
  trsf.SetValues(m[0], m[1], m[2], t[0], m[3], m[4], m[5], t[1], m[6], m[7], m[8], t[2]);
  return geometry::move_faces_rigidly(std::string(feature), body, names, trsf);
}

} // namespace mitcad::bridge
