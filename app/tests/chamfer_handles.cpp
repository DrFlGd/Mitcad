// SPDX-License-Identifier: MIT
// Where Fillet's and Chamfer's handles stand on an edge (mitcad#534;
// commands/ChamferHandles.hpp): the bevel's middle of an equal distance,
// the arrows along each face for a distance measured on it, at convex,
// concave and obtuse edges, as a placement moves them, and which face the
// model measures the first distance on.
#include "commands/ChamferHandles.hpp"

#include <cmath>
#include <cstdio>

#include <gp_Ax1.hxx>
#include <gp_Vec.hxx>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "chamfer_handles.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using namespace mitcad::cmd;

constexpr double kTolerance = 1.0e-9;
constexpr double kPi = 3.14159265358979323846;

bool same(const gp_Dir& a, const gp_Dir& b) { return a.IsEqual(b, kTolerance); }
bool same(const gp_Pnt& a, const gp_Pnt& b) { return a.Distance(b) < kTolerance; }
bool same(double a, double b) { return std::abs(a - b) < kTolerance; }

// The knob of an arrow at a value (Manipulator: origin + value * factor).
gp_Pnt knob(const EdgeArrow& arrow, double value) {
  return arrow.origin.Translated(gp_Vec(arrow.direction) * (value * arrow.factor));
}

// The top front edge of a block: the top (+Z) its first face, the front
// (-Y) its second, along X at (30, 0, 20).
EdgeSides topFront() {
  EdgeSides sides;
  sides.middle = gp_Pnt(30, 0, 20);
  sides.tangent = gp_Dir(1, 0, 0);
  sides.normals = {gp_Dir(0, 0, 1), gp_Dir(0, -1, 0)};
  sides.convex = true;
  return sides;
}

void convexTests() {
  const EdgeSides sides = topFront();
  CHECK(same(bendAngle(sides), kPi / 2));
  const auto toward = bendDirection(sides);
  CHECK(toward && same(*toward, gp_Dir(0, 1, -1)));
  // Equal distance 2: the bevel joins (30, 2, 20) and (30, 0, 18); its
  // middle is (30, 1, 19).
  const auto equal = equalChamferArrow(sides);
  CHECK(equal && same(equal->factor, std::sqrt(0.5)));
  CHECK(equal && same(knob(*equal, 2.0), gp_Pnt(30, 1, 19)));
  // On the top: back into it (+Y); on the front: down it (-Z).
  const auto top = faceDistanceArrow(sides, 0);
  CHECK(top && same(top->direction, gp_Dir(0, 1, 0)) && same(top->factor, 1.0));
  CHECK(top && same(knob(*top, 2.0), gp_Pnt(30, 2, 20)));
  const auto front = faceDistanceArrow(sides, 1);
  CHECK(front && same(front->direction, gp_Dir(0, 0, -1)));
  CHECK(!faceDistanceArrow(sides, 2));
  // The edge's direction either way: the same arrows.
  EdgeSides reversed = sides;
  reversed.tangent = gp_Dir(-1, 0, 0);
  const auto top2 = faceDistanceArrow(reversed, 0);
  const auto front2 = faceDistanceArrow(reversed, 1);
  CHECK(top2 && same(top2->direction, gp_Dir(0, 1, 0)));
  CHECK(front2 && same(front2->direction, gp_Dir(0, 0, -1)));
}

void concaveTests() {
  // An inside corner: a floor (+Z, running to +Y) meets a wall (+Y,
  // running up) along X; the material is under the floor and behind the
  // wall.
  EdgeSides sides;
  sides.middle = gp_Pnt(0, 0, 0);
  sides.tangent = gp_Dir(1, 0, 0);
  sides.normals = {gp_Dir(0, 0, 1), gp_Dir(0, 1, 0)};
  sides.convex = false;
  const auto toward = bendDirection(sides);
  CHECK(toward && same(*toward, gp_Dir(0, 1, 1)));
  const auto onFloor = faceDistanceArrow(sides, 0);
  const auto onWall = faceDistanceArrow(sides, 1);
  CHECK(onFloor && same(onFloor->direction, gp_Dir(0, 1, 0)));
  CHECK(onWall && same(onWall->direction, gp_Dir(0, 0, 1)));
  const auto equal = equalChamferArrow(sides);
  CHECK(equal && same(knob(*equal, 2.0), gp_Pnt(0, 1, 1)));
}

void obtuseTests() {
  // A top (+Z) and a face sloping down at 45 degrees towards -Y: the
  // equal distance's knob is the middle of the knobs on the two faces.
  EdgeSides sides;
  sides.middle = gp_Pnt(0, 0, 0);
  sides.tangent = gp_Dir(1, 0, 0);
  sides.normals = {gp_Dir(0, 0, 1), gp_Dir(0, -1, 1)};
  sides.convex = true;
  CHECK(same(bendAngle(sides), kPi / 4));
  const auto top = faceDistanceArrow(sides, 0);
  const auto slope = faceDistanceArrow(sides, 1);
  const auto equal = equalChamferArrow(sides);
  CHECK(top && same(top->direction, gp_Dir(0, 1, 0)));
  CHECK(slope && same(slope->direction, gp_Dir(0, -1, -1)));
  CHECK(equal && same(equal->factor, std::sin(kPi / 8)));
  if (top && slope && equal) {
    const gp_Pnt a = knob(*top, 3.0);
    const gp_Pnt b = knob(*slope, 3.0);
    CHECK(same(knob(*equal, 3.0), gp_Pnt((a.XYZ() + b.XYZ()) / 2)));
  }
}

void degenerateTests() {
  // Tangent faces: no bevel and no side to go to.
  EdgeSides tangent = topFront();
  tangent.normals = {gp_Dir(0, 0, 1), gp_Dir(0, 0, 1)};
  CHECK(!equalChamferArrow(tangent));
  CHECK(!faceDistanceArrow(tangent, 0));
  // Opposite normals (a knife edge): no bisector.
  EdgeSides knife = topFront();
  knife.normals = {gp_Dir(0, 0, 1), gp_Dir(0, 0, -1)};
  CHECK(!bendDirection(knife));
  CHECK(!equalChamferArrow(knife));
}

void placementTests() {
  // A component turned a quarter about Z and moved: the arrows turn and
  // move with it.
  gp_Trsf turn;
  turn.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), -kPi / 2);
  gp_Trsf move;
  move.SetTranslation(gp_Vec(100, 0, 0));
  const gp_Trsf placement = move * turn;
  const EdgeSides sides = placed(topFront(), placement);
  CHECK(same(sides.middle, gp_Pnt(100, -30, 20)));
  CHECK(sides.convex);
  const auto top = faceDistanceArrow(sides, 0);
  const auto front = faceDistanceArrow(sides, 1);
  const auto equal = equalChamferArrow(sides);
  CHECK(top && same(top->direction, gp_Dir(1, 0, 0)));
  CHECK(front && same(front->direction, gp_Dir(0, 0, -1)));
  CHECK(equal && same(knob(*equal, 2.0), gp_Pnt(101, -30, 19)));
}

void measuredSideTests() {
  // Without a reference: the edge's first face, Flip the second.
  CHECK(measuredSide(false, false, false, false) == 0);
  CHECK(measuredSide(false, false, false, true) == 1);
  // The reference face's side, Flip the other.
  CHECK(measuredSide(true, true, false, false) == 0);
  CHECK(measuredSide(true, false, true, false) == 1);
  CHECK(measuredSide(true, true, false, true) == 1);
  CHECK(measuredSide(true, false, true, true) == 0);
  // A reference that does not border the edge, or on both sides.
  CHECK(!measuredSide(true, false, false, false));
  CHECK(!measuredSide(true, true, true, false));
}

} // namespace

int chamferHandleTests() {
  convexTests();
  concaveTests();
  obtuseTests();
  degenerateTests();
  placementTests();
  measuredSideTests();
  return failures;
}
