// SPDX-License-Identifier: MIT
// Mass properties, bounding boxes and face and edge descriptions.

#include <thread>

#include <BRepBuilderAPI_Copy.hxx>
#include <BRep_Builder.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>

#include "../src/face_select.hpp"
#include "check.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

void test_mass_properties_and_box() {
  const ShapePtr block = extrude("F2", {rectangle(1, -10, 5, 60, 40)}, 0, 20);
  const MassProperties props = mass_properties(*block);
  CHECK(near(props.volume, 48000.0));
  CHECK(near(props.area, 2 * (60.0 * 40.0 + 60.0 * 20.0 + 40.0 * 20.0)));
  CHECK(near(props.center.X(), 20) && near(props.center.Y(), 25) && near(props.center.Z(), 10));
  const BoundingBox box = bounding_box(*block);
  CHECK(!box.empty);
  CHECK(near(box.min.X(), -10) && near(box.min.Y(), 5) && near(box.min.Z(), 0));
  CHECK(near(box.max.X(), 50) && near(box.max.Y(), 45) && near(box.max.Z(), 20));
  // A cylinder's box is tight, not enlarged by tolerances or control points.
  const BoundingBox round = bounding_box(*extrude("F3", {circle(5, 0, 0, 5)}, 0, 10));
  CHECK(near(round.min.X(), -5, 1e-4) && near(round.max.Y(), 5, 1e-4));
  CHECK(bounding_box(Shape(TopoDS_Shape())).empty);
}

void test_enclosing_box_and_edge_faces() {
  // The enclosing box holds the tight one (a cylinder's own box is exact).
  const ShapePtr round = extrude("F3", {circle(5, 0, 0, 5)}, 0, 10);
  const BoundingBox enclosing = enclosing_box(*round);
  CHECK(!enclosing.empty);
  CHECK(enclosing.min.X() <= -5 + 1e-6 && enclosing.max.X() >= 5 - 1e-6);
  CHECK(enclosing.min.Z() <= 1e-6 && enclosing.max.Z() >= 10 - 1e-6);
  CHECK(enclosing_box(Shape(TopoDS_Shape())).empty);
  // Once the tight box was measured, it is the one given.
  const BoundingBox tight = bounding_box(*round);
  const BoundingBox again = enclosing_box(*round);
  CHECK(near(again.min.X(), tight.min.X(), 1e-9) && near(again.max.Z(), tight.max.Z(), 1e-9));
  // The faces of every edge, found once, are those faces_at_edge finds.
  const mitcad::geometry::detail::EdgeFaces faces(*round);
  for (int e = 0; e < round->edge_count(); ++e) {
    CHECK(faces.at(e) == mitcad::geometry::detail::faces_at_edge(*round, e));
  }
}

void test_face_and_edge_infos() {
  const ShapePtr cylinder = extrude("F3", {circle(5, 0, 0, 5)}, 0, 10);
  const std::vector<FaceInfo> faces = face_infos(*cylinder);
  CHECK(faces.size() == 3);
  int planes = 0;
  for (const FaceInfo& face : faces) {
    if (face.surface == "plane") {
      ++planes;
      CHECK(near(face.area, kPi * 25.0));
    } else {
      CHECK(face.surface == "cylinder");
      CHECK(face.names == NameList{"F3:side(c5)"});
      CHECK(near(face.area, 2 * kPi * 5.0 * 10.0));
    }
  }
  CHECK(planes == 2);
  const std::vector<EdgeInfo> edges = edge_infos(*cylinder);
  CHECK(edges.size() == 3);
  int circles = 0;
  for (const EdgeInfo& e : edges) {
    CHECK(!e.name.empty());
    if (e.curve == "circle") {
      ++circles;
      CHECK(near(e.length, 2 * kPi * 5.0));
    } else {
      CHECK(e.curve == "line" && near(e.length, 10.0)); // the seam
    }
  }
  CHECK(circles == 2);
}

// The model's worker and the application measure the same shapes at once
// (P7): each thread gets the same answer, and copies made meanwhile keep
// what was measured.
void test_measured_from_threads() {
  const ShapePtr block = extrude("F2", {rectangle(1, -10, 5, 60, 40)}, 0, 20);
  constexpr std::size_t kThreads = 8;
  std::vector<double> volumes(kThreads, 0.0);
  std::vector<double> widths(kThreads, 0.0);
  std::vector<std::thread> threads;
  for (std::size_t i = 0; i < kThreads; ++i) {
    threads.emplace_back([&, i] {
      for (int round = 0; round < 20; ++round) {
        const Shape copy = *block;
        volumes[i] = (round % 2 == 0 ? mass_properties(*block) : mass_properties(copy)).volume;
        const BoundingBox box = bounding_box(round % 3 == 0 ? copy : *block);
        widths[i] = box.max.X() - box.min.X();
      }
    });
  }
  for (std::thread& thread : threads) {
    thread.join();
  }
  for (std::size_t i = 0; i < kThreads; ++i) {
    CHECK_NEAR(volumes[i], 48000.0);
    CHECK_NEAR(widths[i], 60.0);
  }
  const std::optional<std::array<double, 5>> measured = block->measured();
  CHECK(measured && near((*measured)[0], 48000.0));
  const Shape copy(*block);
  CHECK(copy.measured() == measured && copy.bounds() == block->bounds());
}

// The faces an operation made or changed (the .f3d import's geometric
// check, mitcad#138): a rounding of a block's top front edge makes its own
// face and trims the four faces it meets; the back and the bottom stay. A
// copy rebuilt from the same data has none.
void test_new_faces() {
  const ShapePtr block = extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20);
  const double r = 5;
  const ShapePtr rounded = fillet("F5", *block, {edge(end_cap("F2", 1), side("F2", 1, 0))}, r);
  const std::vector<NewFace> faces = new_faces({block->occt()}, rounded->occt(), 9);
  CHECK(faces.size() == 5);
  double area = 0;
  int round = 0;
  for (const NewFace& face : faces) {
    area += face.area;
    CHECK(face.points.size() == 9);
    for (const gp_Pnt& p : face.points) {
      BRepExtrema_DistShapeShape on(BRepBuilderAPI_MakeVertex(p).Vertex(), rounded->occt());
      CHECK(on.IsDone() && on.Value() < 1e-7);
    }
    // The rounding's face: its points r from the edge's axis.
    if (near(face.area, kPi / 2 * r * 60, 1e-4)) {
      ++round;
      for (const gp_Pnt& p : face.points) {
        CHECK_NEAR(std::hypot(p.Y() - r, p.Z() - (20 - r)), r);
      }
    }
  }
  CHECK(round == 1);
  // All of the block's faces but the back and the bottom, less what the
  // rounding took, and the rounding.
  const double kept = 60.0 * 20.0 + 60.0 * 40.0;
  const double total = 2 * (60.0 * 40.0 + 60.0 * 20.0 + 40.0 * 20.0);
  const double taken = 2 * r * 60 + 2 * (r * r - kPi * r * r / 4);
  CHECK_NEAR(area, total - kept - taken + kPi / 2 * r * 60);
  // Nothing new against itself or a copy of itself.
  CHECK(new_faces({rounded->occt()}, rounded->occt(), 9).empty());
  const TopoDS_Shape copy = BRepBuilderAPI_Copy(rounded->occt()).Shape();
  CHECK(new_faces({rounded->occt()}, copy, 9).empty());
  // Without bodies before, every face is new.
  CHECK(new_faces({}, rounded->occt(), 4).size() == 7);
  // Of Shapes, where only the faces whose boxes hold a centre of a face of
  // the result are measured (mitcad#658), the same; also with a body far
  // away before it, whose faces no box test lets through, and against a
  // copy rebuilt from the same data.
  const ShapePtr far_block = extrude("F7", {rectangle(1, 500, 500, 60, 40)}, 0, 20);
  const std::vector<NewFace> of_shapes = new_faces({block.get(), far_block.get()}, *rounded, 9);
  CHECK(of_shapes.size() == faces.size());
  double shapes_area = 0;
  for (const NewFace& face : of_shapes) {
    shapes_area += face.area;
  }
  CHECK_NEAR(shapes_area, area);
  const Shape rebuilt(copy);
  CHECK(new_faces({rounded.get()}, rebuilt, 9).empty());
  CHECK(new_faces({far_block.get(), rounded.get()}, rebuilt, 9).empty());
  CHECK(new_faces({far_block.get()}, rebuilt, 9).size() == 7);
  CHECK(new_faces(std::vector<const Shape*>{}, *rounded, 4).size() == 7);
}

// The volume a change makes from the faces it changed (mitcad#235): the
// rounding takes r^2 (1 - pi / 4) along the 60 mm edge; against a copy of
// the block (its faces rebuilt from the same data) the same, as the faces
// the copies share cancel; nothing against itself; without edges looser
// than 1e-7 mm little slack, and with every edge loosened to 0.01 mm
// a slack of a third of their length times that times their distance from
// the box's centre.
void test_changed_volume() {
  const ShapePtr block = extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20);
  const double r = 5;
  const ShapePtr rounded = fillet("F5", *block, {edge(end_cap("F2", 1), side("F2", 1, 0))}, r);
  const double taken = r * r * (1.0 - kPi / 4.0) * 60.0;
  const ChangedVolume changed = changed_volume({block->occt()}, {rounded->occt()});
  CHECK_NEAR(changed.volume, -taken);
  CHECK(changed.slack < 1e-2);
  const TopoDS_Shape copy = BRepBuilderAPI_Copy(block->occt()).Shape();
  CHECK_NEAR(changed_volume({copy}, {rounded->occt()}).volume, -taken);
  CHECK_NEAR(changed_volume({rounded->occt()}, {rounded->occt()}).volume, 0.0);
  CHECK_NEAR(changed_volume({rounded->occt()}, {block->occt()}).volume, taken);
  const TopoDS_Shape loose = BRepBuilderAPI_Copy(rounded->occt()).Shape();
  BRep_Builder builder;
  for (TopExp_Explorer e(loose, TopAbs_EDGE); e.More(); e.Next()) {
    builder.UpdateEdge(TopoDS::Edge(e.Current()), 0.01);
  }
  const ChangedVolume slack = changed_volume({block->occt()}, {loose});
  CHECK_NEAR(slack.volume, -taken);
  CHECK(slack.slack > 1.0 && slack.slack > 100.0 * changed.slack);
  // Of Shapes (their face boxes kept with them, mitcad#658), the same,
  // also with a body far away on both sides and against a copy.
  const ShapePtr far_block = extrude("F7", {rectangle(1, 500, 500, 60, 40)}, 0, 20);
  const std::vector<const Shape*> before{block.get(), far_block.get()};
  const std::vector<const Shape*> after{rounded.get(), far_block.get()};
  CHECK_NEAR(changed_volume(before, after).volume, -taken);
  const Shape copied(copy);
  CHECK_NEAR(changed_volume({&copied}, {rounded.get()}).volume, -taken);
  CHECK_NEAR(changed_volume({rounded.get()}, {rounded.get()}).volume, 0.0);
  CHECK_NEAR(changed_volume({rounded.get()}, {block.get()}).volume, taken);
  const Shape loosened(loose);
  CHECK_NEAR(changed_volume({block.get()}, {&loosened}).slack, slack.slack);
}

} // namespace

void query_tests() {
  test_mass_properties_and_box();
  test_enclosing_box_and_edge_faces();
  test_face_and_edge_infos();
  test_measured_from_threads();
  test_new_faces();
  test_changed_volume();
}

} // namespace test
