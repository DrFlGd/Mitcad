// SPDX-License-Identifier: MIT
// The plane geometry of a placement on a face (mitcad#656; PlaceGeometry.hpp).
#include "PlaceGeometry.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <limits>

#include <gp_Vec.hxx>

namespace mitcad::cmd::place {

std::optional<Line> inPlane(const Line& line, const gp_Pnt& origin, const gp_Dir& normal) {
  const gp_Vec n(normal);
  const gp_Vec along = gp_Vec(line.direction) - n * gp_Vec(line.direction).Dot(n);
  if (along.Magnitude() < 1e-9) {
    return std::nullopt;
  }
  Line projected;
  projected.point = line.point.Translated(-n * gp_Vec(origin, line.point).Dot(n));
  projected.direction = gp_Dir(along);
  // A segment keeps its ends: its length is the projected one.
  projected.length = line.length < 0.0 ? -1.0 : line.length * along.Magnitude();
  return projected;
}

double distanceTo(const Line& line, const gp_Pnt& point) {
  const gp_Vec offset(line.point, point);
  double t = offset.Dot(gp_Vec(line.direction));
  if (line.length >= 0.0) {
    t = std::clamp(t, 0.0, line.length);
  }
  return point.Distance(line.point.Translated(gp_Vec(line.direction) * t));
}

gp_Dir across(const Line& line, const gp_Dir& normal) { return normal.Crossed(line.direction); }

double signedDistance(const Line& line, const gp_Dir& side, const gp_Pnt& point) {
  return gp_Vec(line.point, point).Dot(gp_Vec(side));
}

std::optional<gp_Pnt> pointAt(const Line& a, const gp_Dir& sideA, double first, const Line& b,
                              const gp_Dir& sideB, double second, const gp_Dir& normal) {
  // The two lines moved by their distances, intersected in the plane.
  const gp_Pnt pa = a.point.Translated(gp_Vec(sideA) * first);
  const gp_Pnt pb = b.point.Translated(gp_Vec(sideB) * second);
  const gp_Vec da(a.direction);
  const gp_Vec db(b.direction);
  const double det = da.Crossed(db).Dot(gp_Vec(normal));
  if (std::abs(det) < 1e-9) {
    return std::nullopt;
  }
  // pa + s da = pb + t db: s = ((pb - pa) x db) . n / det.
  const double s = gp_Vec(pa, pb).Crossed(db).Dot(gp_Vec(normal)) / det;
  return pa.Translated(da * s);
}

std::optional<gp_Pnt> snap(const gp_Pnt& point, const std::vector<gp_Pnt>& candidates, double tolerance) {
  std::optional<gp_Pnt> best;
  double nearest = tolerance;
  for (const gp_Pnt& candidate : candidates) {
    const double distance = candidate.Distance(point);
    if (distance <= nearest) {
      nearest = distance;
      best = candidate;
    }
  }
  return best;
}

std::optional<std::pair<int, int>> nearestPair(const std::vector<Line>& lines, const gp_Pnt& point,
                                               double minAngle) {
  int first = -1;
  double nearest = std::numeric_limits<double>::infinity();
  for (std::size_t i = 0; i < lines.size(); ++i) {
    const double distance = distanceTo(lines[i], point);
    if (distance < nearest) {
      nearest = distance;
      first = static_cast<int>(i);
    }
  }
  if (first < 0) {
    return std::nullopt;
  }
  const double sine = std::sin(minAngle);
  int second = -1;
  nearest = std::numeric_limits<double>::infinity();
  for (std::size_t i = 0; i < lines.size(); ++i) {
    const gp_Vec cross = gp_Vec(lines[i].direction).Crossed(gp_Vec(lines[static_cast<std::size_t>(first)].direction));
    if (static_cast<int>(i) == first || cross.Magnitude() < sine) {
      continue;
    }
    const double distance = distanceTo(lines[i], point);
    if (distance < nearest) {
      nearest = distance;
      second = static_cast<int>(i);
    }
  }
  if (second < 0) {
    return std::nullopt;
  }
  return std::make_pair(first, second);
}

int replaced(const Line& a, const Line& b, const Line& next) {
  const double withA = std::abs(gp_Vec(a.direction).Dot(gp_Vec(next.direction)));
  const double withB = std::abs(gp_Vec(b.direction).Dot(gp_Vec(next.direction)));
  return withB > withA ? 1 : 0;
}

gp_Dir intoFace(const gp_Dir& line, const gp_Dir& side, const gp_Dir& normal) {
  return gp_Vec(normal).Crossed(gp_Vec(line)).Dot(gp_Vec(side)) < 0.0 ? line.Reversed() : line;
}

gp_Dir zeroAxis(const std::optional<gp_Dir>& align, const std::optional<gp_Dir>& alignSide, bool planeAxes,
                const gp_Dir& planeX, const Line& reference1, const gp_Dir& side1, const gp_Dir& normal) {
  if (align) {
    return alignSide ? intoFace(*align, *alignSide, normal) : *align;
  }
  if (planeAxes) {
    return planeX;
  }
  return intoFace(reference1.direction, side1, normal);
}

bool keepsPlaneX(bool wasPlaneAxes, bool nowPlaneAxes, bool alignEmpty) {
  return wasPlaneAxes && !nowPlaneAxes && alignEmpty;
}

} // namespace mitcad::cmd::place
