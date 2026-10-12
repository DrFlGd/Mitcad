// SPDX-License-Identifier: MIT
#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>

#include "render/Renderer.hpp"

namespace mitcad::render {

// Where the render camera of an orthographic view stands (mitcad#215): on
// the view's axis, behind everything in the scene's bounds (low, high).
// The view's eye may stand inside the scene, as the 3D view's depth range
// reaches behind it, but the renderer's orthographic rays start at the
// camera's plane and go forward only, so what lies behind the eye would be
// cut off. The image does not depend on the distance along the axis, so the
// camera steps back to the bounds' sphere (and a little more); an eye that
// is behind it already stays where it is.
inline std::array<double, 3> orthographicEye(const ViewData& view, const std::array<double, 3>& low,
                                             const std::array<double, 3>& high) {
  std::array<double, 3> forward{};
  double length = 0.0;
  double diagonal = 0.0;
  for (std::size_t i = 0; i < 3; ++i) {
    forward[i] = view.target[i] - view.eye[i];
    length += forward[i] * forward[i];
    diagonal += (high[i] - low[i]) * (high[i] - low[i]);
  }
  length = std::sqrt(length);
  if (!(length > 0.0) || !(diagonal >= 0.0)) {
    return view.eye;
  }
  const double radius = std::sqrt(diagonal) / 2.0;
  // The bounds' centre along the axis, from the target.
  double along = 0.0;
  for (std::size_t i = 0; i < 3; ++i) {
    forward[i] /= length;
    along += ((low[i] + high[i]) / 2.0 - view.target[i]) * forward[i];
  }
  const double margin = std::max(radius * 0.01, 1.0);
  const double from = std::min(-length, along - radius - margin);
  std::array<double, 3> eye{};
  for (std::size_t i = 0; i < 3; ++i) {
    eye[i] = view.target[i] + forward[i] * from;
  }
  return eye;
}

} // namespace mitcad::render
