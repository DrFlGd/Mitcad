// SPDX-License-Identifier: MIT
#pragma once

// The plane geometry of a placement on a face (mitcad#656; PlaceInputs.hpp):
// the point a click snaps to, the two lines its distances are measured
// from, and the point at two distances. Plain OCCT geometry types, no
// model: the unit tests (app/tests/place_geometry.cpp) run it alone.

#include <optional>
#include <utility>
#include <vector>

#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

namespace mitcad::cmd::place {

// A line: a straight edge (a segment from `point` along `direction` for
// `length`), or an axis without an end (`length` negative).
struct Line {
  gp_Pnt point;
  gp_Dir direction = gp_Dir(1, 0, 0);
  double length = -1.0;
};

// The line put into the plane through `origin` square to `normal` (its
// projection); none when it runs along the normal.
std::optional<Line> inPlane(const Line& line, const gp_Pnt& origin, const gp_Dir& normal);

// The distance from a point to the line in the plane: to the segment of a
// straight edge, to the whole line of an axis.
double distanceTo(const Line& line, const gp_Pnt& point);

// The way across the line in the plane (normal x direction).
gp_Dir across(const Line& line, const gp_Dir& normal);

// How far the point lies from the line along `side` (a unit vector in the
// plane across the line): negative on the other side.
double signedDistance(const Line& line, const gp_Dir& side, const gp_Pnt& point);

// The point of the plane at `first` from line `a` along `sideA` and
// `second` from line `b` along `sideB`; none when the lines are parallel.
std::optional<gp_Pnt> pointAt(const Line& a, const gp_Dir& sideA, double first, const Line& b,
                              const gp_Dir& sideB, double second, const gp_Dir& normal);

// The candidate nearest the point within `tolerance`; none when none is.
std::optional<gp_Pnt> snap(const gp_Pnt& point, const std::vector<gp_Pnt>& candidates, double tolerance);

// Two lines to measure from: the one nearest the point, then the nearest
// of those that cross it at `minAngle` (radians) or more; their indices,
// none when no two lines cross so.
std::optional<std::pair<int, int>> nearestPair(const std::vector<Line>& lines, const gp_Pnt& point,
                                               double minAngle);

// Which of two reference lines a newly picked line takes the place of:
// 0 for `a`, 1 for `b`, the one more nearly parallel to it.
int replaced(const Line& a, const Line& b, const Line& next);

// A line's direction turned so that `side` (a unit vector in the plane
// across it) lies on its left about `normal`: the x axis of a part placed
// from the line grows into the face (mitcad#656, the model's align_side).
gp_Dir intoFace(const gp_Dir& line, const gp_Dir& side, const gp_Dir& normal);

// The x axis a part placed on a face takes before its Angle (mitcad#656):
// Align's direction when one is given (with `alignSide`, turned so that
// it lies on the left), else on an origin plane measured from its own
// axes (`planeAxes`) the plane's x, else Reference 1's line into the face.
gp_Dir zeroAxis(const std::optional<gp_Dir>& align, const std::optional<gp_Dir>& alignSide, bool planeAxes,
                const gp_Dir& planeX, const Line& reference1, const gp_Dir& side1, const gp_Dir& normal);

// Whether a reference picked again must pin the plane's x as Align: the
// references were an origin plane's own axes and are no longer, and Align
// is empty (else the part would turn from the plane's x to Reference 1).
bool keepsPlaneX(bool wasPlaneAxes, bool nowPlaneAxes, bool alignEmpty);

} // namespace mitcad::cmd::place
