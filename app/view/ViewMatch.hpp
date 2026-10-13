// SPDX-License-Identifier: MIT
#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>

namespace mitcad {

// What a view shows: its camera and its size in device pixels.
struct ViewPose {
  std::array<double, 3> eye{};
  std::array<double, 3> center{};
  std::array<double, 3> up{};
  // The height of the view in model units (an orthographic camera's scale,
  // or what a perspective camera sees at its target).
  double scale = 0.0;
  bool orthographic = false;
  int width = 0;
  int height = 0;
};

// Whether two views show the same thing (mitcad#216): the rendered image
// made for one may be drawn over the other. Positions may differ by a
// billionth of the view's height, far below a pixel, so that rounding noise
// in a copy of the camera does not count as a move. (OCCT's
// gp_XYZ::IsEqual compares with a strict `<`, so with a tolerance of 0 it
// never finds two points equal: it cannot be used for an exact comparison.)
inline bool sameView(const ViewPose& a, const ViewPose& b) {
  if (a.orthographic != b.orthographic || a.width != b.width || a.height != b.height) {
    return false;
  }
  const double height = std::max(std::abs(a.scale), std::abs(b.scale));
  if (!(std::abs(a.scale - b.scale) <= 1e-9 * height)) {
    return false;
  }
  const double length = 1e-9 * height;
  for (std::size_t i = 0; i < 3; ++i) {
    if (!(std::abs(a.eye[i] - b.eye[i]) <= length) || !(std::abs(a.center[i] - b.center[i]) <= length) ||
        !(std::abs(a.up[i] - b.up[i]) <= 1e-12)) {
      return false;
    }
  }
  return true;
}

} // namespace mitcad
