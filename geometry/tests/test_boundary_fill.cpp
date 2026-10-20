// SPDX-License-Identifier: MIT
// Boundary Fill (mitcad#457): the cells that tool bodies enclose, picked by
// points, with the tools' face names; the tools are not changed.

#include <cstddef>
#include <cstdio>
#include <string>
#include <vector>

#include <TopAbs_ShapeEnum.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_Pnt.hxx>

#include "check.hpp"
#include "mitcad/geometry/boundary_fill.hpp"
#include "mitcad/geometry/input_check.hpp"
#include "mitcad/geometry/primitive.hpp"
#include "mitcad/geometry/query.hpp"

namespace test {
namespace {

using mitcad::geometry::BoundaryCell;
using mitcad::geometry::Shape;

// A box with its lowest corner at `corner` and sides along the axes.
ShapePtr box(const std::string& feature, const gp_Pnt& corner, double length, double width,
             double height) {
  mitcad::geometry::PrimitiveSpec spec;
  spec.feature = feature;
  spec.frame.origin = corner;
  spec.kind = mitcad::geometry::PrimitiveKind::Box;
  spec.a = length;
  spec.b = width;
  spec.c = height;
  return mitcad::geometry::primitive(spec);
}

int solid_count(const Shape& shape) {
  int count = 0;
  for (TopExp_Explorer it(shape.occt(), TopAbs_SOLID); it.More(); it.Next()) {
    ++count;
  }
  return count;
}

bool has_face(const Shape& shape, const std::string& name) { return !shape.find_faces(name).empty(); }

bool all_faces_named(const Shape& shape) {
  for (int i = 0; i < shape.face_count(); ++i) {
    if (shape.face_names(i).empty()) {
      return false;
    }
  }
  return shape.face_count() > 0;
}

void check_box(const Shape& shape, const gp_Pnt& min, const gp_Pnt& max) {
  const mitcad::geometry::BoundingBox b = mitcad::geometry::bounding_box(shape);
  CHECK(!b.empty);
  CHECK_NEAR_TOL(b.min.X(), min.X(), 1e-6);
  CHECK_NEAR_TOL(b.min.Y(), min.Y(), 1e-6);
  CHECK_NEAR_TOL(b.min.Z(), min.Z(), 1e-6);
  CHECK_NEAR_TOL(b.max.X(), max.X(), 1e-6);
  CHECK_NEAR_TOL(b.max.Y(), max.Y(), 1e-6);
  CHECK_NEAR_TOL(b.max.Z(), max.Z(), 1e-6);
}

// Two 20 mm cubes, A at the origin and B at (10, 10, 10): the cube from
// 10 to 20 is in both, an L-shaped block of 7000 mm3 in each alone.
void test_cells_of_two_overlapping_boxes() {
  const ShapePtr a = box("F1", gp_Pnt(0, 0, 0), 20, 20, 20);
  const ShapePtr b = box("F2", gp_Pnt(10, 10, 10), 20, 20, 20);
  const std::vector<BoundaryCell> cells = mitcad::geometry::boundary_cells(
      {a.get(), b.get()},
      {gp_Pnt(15, 15, 15), gp_Pnt(5, 5, 5), gp_Pnt(25, 25, 25), gp_Pnt(12, 18, 11)});
  CHECK(cells.size() == 4);
  if (cells.size() != 4) {
    return;
  }

  const Shape& common = *cells[0].shape;
  CHECK((cells[0].holders == std::vector<std::size_t>{0, 1}));
  CHECK_NEAR(mitcad::geometry::volume(common), 1000.0);
  check_box(common, gp_Pnt(10, 10, 10), gp_Pnt(20, 20, 20));
  CHECK(solid_count(common) == 1);
  CHECK(common.face_count() == 6);
  // Bounded by A's top and far sides and B's bottom and near sides.
  CHECK(has_face(common, "F1:top"));
  CHECK(has_face(common, "F1:side1"));
  CHECK(has_face(common, "F2:bottom"));
  CHECK(has_face(common, "F2:side3"));
  CHECK(!has_face(common, "F1:bottom"));
  CHECK(!has_face(common, "F2:top"));
  CHECK(all_faces_named(common));

  const Shape& only_a = *cells[1].shape;
  CHECK((cells[1].holders == std::vector<std::size_t>{0}));
  CHECK_NEAR(mitcad::geometry::volume(only_a), 7000.0);
  check_box(only_a, gp_Pnt(0, 0, 0), gp_Pnt(20, 20, 20));
  CHECK(solid_count(only_a) == 1);
  // A's six faces and the three faces of B inside A.
  CHECK(only_a.face_count() == 9);
  CHECK(has_face(only_a, "F1:bottom"));
  CHECK(has_face(only_a, "F2:bottom"));
  CHECK(all_faces_named(only_a));

  const Shape& only_b = *cells[2].shape;
  CHECK((cells[2].holders == std::vector<std::size_t>{1}));
  CHECK_NEAR(mitcad::geometry::volume(only_b), 7000.0);
  check_box(only_b, gp_Pnt(10, 10, 10), gp_Pnt(30, 30, 30));
  CHECK(has_face(only_b, "F2:top"));
  CHECK(has_face(only_b, "F1:top"));
  CHECK(all_faces_named(only_b));

  // A second point in the common cell picks the same cell.
  CHECK(cells[3].holders == cells[0].holders);
  CHECK(cells[3].shape == cells[0].shape);

  // The tools are as they were.
  CHECK_NEAR(mitcad::geometry::volume(*a), 8000.0);
  CHECK_NEAR(mitcad::geometry::volume(*b), 8000.0);
}

// A bar crossed by a wall: the part of the bar outside the wall falls apart
// into two blocks, and one point picks both.
void test_a_cell_of_two_solids() {
  const ShapePtr bar = box("F3", gp_Pnt(0, -10, 0), 10, 40, 10);
  const ShapePtr wall = box("F4", gp_Pnt(-5, 8, -5), 20, 4, 20);
  const std::vector<BoundaryCell> cells =
      mitcad::geometry::boundary_cells({bar.get(), wall.get()}, {gp_Pnt(5, 0, 5)});
  CHECK(cells.size() == 1);
  if (cells.size() != 1) {
    return;
  }
  CHECK((cells[0].holders == std::vector<std::size_t>{0}));
  CHECK(solid_count(*cells[0].shape) == 2);
  CHECK_NEAR(mitcad::geometry::volume(*cells[0].shape), 3600.0);
  check_box(*cells[0].shape, gp_Pnt(0, -10, 0), gp_Pnt(10, 30, 10));
  CHECK(has_face(*cells[0].shape, "F4:side0"));
  CHECK(has_face(*cells[0].shape, "F4:side2"));
  CHECK(all_faces_named(*cells[0].shape));
}

// One tool: its cell is all of it.
void test_one_tool() {
  const ShapePtr a = box("F1", gp_Pnt(0, 0, 0), 20, 10, 5);
  const std::vector<BoundaryCell> cells = mitcad::geometry::boundary_cells({a.get()}, {gp_Pnt(1, 1, 1)});
  CHECK(cells.size() == 1);
  if (cells.size() == 1) {
    CHECK_NEAR(mitcad::geometry::volume(*cells[0].shape), 1000.0);
    CHECK(cells[0].shape->face_count() == 6);
    CHECK(has_face(*cells[0].shape, "F1:top"));
  }
}

void test_points_outside_or_on_a_boundary_fail() {
  const ShapePtr a = box("F1", gp_Pnt(0, 0, 0), 20, 20, 20);
  const ShapePtr b = box("F2", gp_Pnt(10, 10, 10), 20, 20, 20);
  CHECK(throws_with([&] { mitcad::geometry::boundary_cells({a.get(), b.get()}, {gp_Pnt(50, 50, 50)}); },
                    "is inside no tool body"));
  CHECK(throws_with([&] { mitcad::geometry::boundary_cells({a.get(), b.get()}, {gp_Pnt(20, 5, 5)}); },
                    "lies on the boundary of tool 1"));
}

} // namespace
} // namespace test

int main() {
  // The tools are read, never changed (T0e).
  mitcad::geometry::enable_input_checks(true);
  test::guarded("test_cells_of_two_overlapping_boxes", test::test_cells_of_two_overlapping_boxes);
  test::guarded("test_a_cell_of_two_solids", test::test_a_cell_of_two_solids);
  test::guarded("test_one_tool", test::test_one_tool);
  test::guarded("test_points_outside_or_on_a_boundary_fail", test::test_points_outside_or_on_a_boundary_fail);
  const std::vector<std::string> changes = mitcad::geometry::input_changes();
  for (const std::string& change : changes) {
    std::fprintf(stderr, "input changed: %s\n", change.c_str());
  }
  CHECK(changes.empty());
  if (test::failures == 0) {
    std::puts("test_boundary_fill: all checks passed");
  }
  return test::failures == 0 ? 0 : 1;
}
