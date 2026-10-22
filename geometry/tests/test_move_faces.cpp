// SPDX-License-Identifier: MIT
// Faces moved by a rigid motion, their neighbours following
// (move_faces.hpp, mitcad#343: the .f3d import's moved faces).

#include <cmath>
#include <stdexcept>

#include <BRepBndLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <gp_Ax1.hxx>
#include <gp_Trsf.hxx>

#include "check.hpp"
#include "mitcad/geometry/boolean.hpp"
#include "mitcad/geometry/move_faces.hpp"
#include "mitcad/geometry/naming.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

double volume_of(const Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape.occt(), props);
  return props.Mass();
}

Bnd_Box box_of(const TopoDS_Shape& shape) {
  Bnd_Box box;
  BRepBndLib::AddOptimal(shape, box, false, false);
  return box;
}

gp_Trsf translation(double x, double y, double z) {
  gp_Trsf t;
  t.SetTranslation(gp_Vec(x, y, z));
  return t;
}

// A rod's side moved square to its axis: its ends, planes square to the
// axis, stay where they are and follow it, so the rod moves whole.
void test_rod_side_moved_sideways() {
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 9)}, -32.5, 32.5);
  const std::string sideName = face_name("F2", "side", "c1");
  const ShapePtr moved = move_faces_rigidly("F3", *rod, {sideName}, translation(-1.8559, 0.0, -0.3614));
  CHECK(BRepCheck_Analyzer(moved->occt()).IsValid());
  CHECK_NEAR(volume_of(*moved), volume_of(*rod));
  CHECK(moved->face_count() == rod->face_count());
  CHECK(faces_named(*moved, sideName) == 1);
  double x0, y0, z0, x1, y1, z1;
  box_of(moved->occt()).Get(x0, y0, z0, x1, y1, z1);
  CHECK_NEAR_TOL(x0, -9.0 - 1.8559, 1e-7);
  CHECK_NEAR_TOL(x1, 9.0 - 1.8559, 1e-7);
  CHECK_NEAR_TOL(y0, -9.0, 1e-7);
  CHECK_NEAR_TOL(y1, 9.0, 1e-7);
  CHECK_NEAR_TOL(z0, -32.5, 1e-7);
  CHECK_NEAR_TOL(z1, 32.5, 1e-7);
}

// A block's end face pushed out along its normal, and its top tilted about
// the top's edge along x: the sides follow along their planes.
void test_block_faces_moved() {
  const ShapePtr block = extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20);
  // The side at x = 60.
  std::string endName;
  for (int i = 0; i < 4; ++i) {
    for (int f = 0; f < block->face_count(); ++f) {
      if (has_name(block->face_names(f), side("F2", 1, i))) {
        double x0, y0, z0, x1, y1, z1;
        box_of(block->face(f)).Get(x0, y0, z0, x1, y1, z1);
        if (std::abs(x0 - 60.0) < 1e-6 && std::abs(x1 - 60.0) < 1e-6) {
          endName = side("F2", 1, i);
        }
      }
    }
  }
  CHECK(!endName.empty());
  const ShapePtr longer = move_faces_rigidly("F3", *block, {endName}, translation(5.0, 0.0, 0.0));
  CHECK(BRepCheck_Analyzer(longer->occt()).IsValid());
  CHECK_NEAR(volume_of(*longer), 65.0 * 40.0 * 20.0);
  // (Moved along itself, a plane stays: still the same body.)
  CHECK(throws_with([&] { move_faces_rigidly("F3", *block, {endName}, gp_Trsf()); }, "moves nothing"));

  const double angle = 5.0 * kPi / 180.0;
  gp_Trsf tilt;
  tilt.SetRotation(gp_Ax1(gp_Pnt(0, 0, 20), gp_Dir(1, 0, 0)), angle);
  const ShapePtr tilted = move_faces_rigidly("F3", *block, {end_cap("F2", 1)}, tilt);
  CHECK(BRepCheck_Analyzer(tilted->occt()).IsValid());
  // The top rises by y tan(angle) towards y = 40.
  CHECK_NEAR(volume_of(*tilted), 60.0 * (20.0 * 40.0 + 0.5 * 40.0 * 40.0 * std::tan(angle)));
  CHECK(faces_named(*tilted, end_cap("F2", 1)) == 1);
}

// A boss's side moved on the block it stands on: the boss's foot moves on
// the block's top, which keeps its plane.
void test_boss_side_moved_on_a_block() {
  const ShapePtr block = extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 10);
  const ShapePtr boss = extrude("F4", {circle(5, 20, 20, 5)}, 10, 20);
  const BooleanResult joined = boolean(BooleanOp::Join, {block.get()}, *boss);
  CHECK(joined.pieces.size() == 1);
  if (joined.pieces.size() != 1) {
    return;
  }
  const ShapePtr body = joined.pieces.front().shape;
  const ShapePtr moved =
      move_faces_rigidly("F5", *body, {face_name("F4", "side", "c5")}, translation(4.0, -3.0, 0.0));
  CHECK(BRepCheck_Analyzer(moved->occt()).IsValid());
  CHECK_NEAR(volume_of(*moved), volume_of(*body));
  double x0, y0, z0, x1, y1, z1;
  box_of(moved->occt()).Get(x0, y0, z0, x1, y1, z1);
  CHECK_NEAR(z1, 20.0);
  // The boss's top moved with its side.
  for (int f = 0; f < moved->face_count(); ++f) {
    if (has_name(moved->face_names(f), face_name("F4", "end", "r{c5}"))) {
      box_of(moved->face(f)).Get(x0, y0, z0, x1, y1, z1);
      CHECK_NEAR_TOL(0.5 * (x0 + x1), 24.0, 1e-7);
      CHECK_NEAR_TOL(0.5 * (y0 + y1), 17.0, 1e-7);
    }
  }
}

// A motion that would change the body's faces is refused.
void test_moves_that_change_the_faces() {
  const ShapePtr block = extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20);
  // (The top pushed down past the bottom.)
  CHECK(throws_with([&] { move_faces_rigidly("F3", *block, {end_cap("F2", 1)}, translation(0, 0, -30)); }, ""));
  gp_Trsf scale;
  scale.SetScale(gp_Pnt(0, 0, 0), 2.0);
  CHECK(throws_with([&] { move_faces_rigidly("F3", *block, {end_cap("F2", 1)}, scale); }, "rigid"));
}

} // namespace

void move_faces_tests() {
  guarded("test_rod_side_moved_sideways", test_rod_side_moved_sideways);
  guarded("test_block_faces_moved", test_block_faces_moved);
  guarded("test_boss_side_moved_on_a_block", test_boss_side_moved_on_a_block);
  guarded("test_moves_that_change_the_faces", test_moves_that_change_the_faces);
}

} // namespace test
