// SPDX-License-Identifier: MIT
// The faces a run of features made (face_copy.hpp): a pin on a plate with a
// rounding at its foot, where the fillet leaves the plate tangentially, and
// a rounding on its top; a hole with a chamfer on its rim. The material
// the features added or removed, moved to other places and joined to or cut
// from the body there, gives the body the features' faces give: the foot
// roundings stay tangent to the plate, no rounding is worked out again.

#include <cmath>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Ax2.hxx>

#include "check.hpp"
#include "mitcad/geometry/boolean.hpp"
#include "mitcad/geometry/face_copy.hpp"
#include "mitcad/geometry/query.hpp"
#include "mitcad/geometry/transform.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

// The circular edges at height z of a shape.
std::vector<TopoDS_Edge> circles_at(const TopoDS_Shape& shape, double z) {
  std::vector<TopoDS_Edge> found;
  for (TopExp_Explorer it(shape, TopAbs_EDGE); it.More(); it.Next()) {
    const TopoDS_Edge edge = TopoDS::Edge(it.Current());
    const BRepAdaptor_Curve curve(edge);
    if (curve.GetType() == GeomAbs_Circle && std::abs(curve.Circle().Location().Z() - z) < 1e-6) {
      found.push_back(edge);
    }
  }
  return found;
}

// A copy of the shape's B-rep, for OCCT's own operations (they write into
// their inputs' sub-shapes, which the library's shapes must not see).
TopoDS_Shape copy_of(const Shape& shape) { return BRepBuilderAPI_Copy(shape.occt()).Shape(); }

// A 60 x 40 x 5 plate.
Shape plate() { return Shape(BRepPrimAPI_MakeBox(gp_Pnt(0, 0, 0), gp_Pnt(60, 40, 5)).Shape()); }

// The plate with a pin of radius 4 and height 10 at (10, 10): a rounding of
// radius 1 at its foot and of 0.5 on its top.
Shape pinned(const Shape& base) {
  const TopoDS_Shape pin = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(10, 10, 5), gp_Dir(0, 0, 1)), 4, 10).Shape();
  const TopoDS_Shape joined = BRepAlgoAPI_Fuse(copy_of(base), pin).Shape();
  BRepFilletAPI_MakeFillet fillet(joined);
  for (const TopoDS_Edge& edge : circles_at(joined, 5)) {
    fillet.Add(1.0, edge);
  }
  for (const TopoDS_Edge& edge : circles_at(joined, 15)) {
    fillet.Add(0.5, edge);
  }
  fillet.Build();
  CHECK(fillet.IsDone());
  return Shape(fillet.Shape());
}

// The plate with a hole of radius 3 through it at (10, 10) and a chamfer of
// 1 mm on its rim.
Shape drilled(const Shape& base) {
  const TopoDS_Shape tool = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(10, 10, -1), gp_Dir(0, 0, 1)), 3, 7).Shape();
  const TopoDS_Shape cut = BRepAlgoAPI_Cut(copy_of(base), tool).Shape();
  BRepFilletAPI_MakeChamfer chamfer(cut);
  for (const TopoDS_Edge& edge : circles_at(cut, 5)) {
    chamfer.Add(1.0, edge);
  }
  chamfer.Build();
  CHECK(chamfer.IsDone());
  return Shape(chamfer.Shape());
}

ShapePtr moved(const Shape& shape, double dx, double dy, const char* name) {
  Affine map;
  map.translation = {dx, dy, 0};
  return transform_shape(shape, map, name);
}

bool valid(const Shape& shape) { return BRepCheck_Analyzer(shape.occt()).IsValid(); }

void test_added_material_is_the_pin_with_its_roundings() {
  const Shape before = plate();
  const Shape after = pinned(before);
  const FeatureChanges changes = feature_changes(before, after);
  CHECK(changes.removed.empty());
  CHECK(changes.added.size() == 1);
  if (changes.added.size() != 1) {
    return;
  }
  const Shape& pin = *changes.added.front().solid;
  CHECK_NEAR(volume(pin), volume(after) - volume(before));
  CHECK(valid(pin));
  const BoundingBox box = bounding_box(pin);
  CHECK(near(box.min.Z(), 5.0, 1e-6) && near(box.max.Z(), 15.0, 1e-6));
  // The foot's rounding runs out 1 mm beyond the pin's side.
  CHECK(near(box.min.X(), 10.0 - 5.0, 1e-6) && near(box.max.X(), 10.0 + 5.0, 1e-6));

  // The pin and its roundings at two more places, joined to the body that
  // has the first: the body's faces gain the pin's four, twice.
  const ShapePtr second = moved(pin, 20, 0, "F9:inst1");
  const ShapePtr third = moved(pin, 40, 0, "F9:inst2");
  const ShapePtr both = unite({second.get(), third.get()});
  const BooleanResult joined = boolean(BooleanOp::Join, {&after}, *both);
  CHECK(joined.touched == std::vector<bool>{true});
  CHECK(joined.pieces.size() == 1);
  if (joined.pieces.size() != 1) {
    return;
  }
  const Shape& result = *joined.pieces.front().shape;
  CHECK(valid(result));
  CHECK_NEAR(volume(result), volume(after) + 2.0 * volume(pin));
  CHECK(result.face_count() == after.face_count() + 8);
}

// The plate with a groove along y over x 28..34, 3 mm deep: nothing of the
// plate's top lies there.
Shape grooved(const Shape& base) {
  const TopoDS_Shape groove = BRepPrimAPI_MakeBox(gp_Pnt(28, -1, 2), gp_Pnt(34, 41, 6)).Shape();
  return Shape(BRepAlgoAPI_Cut(copy_of(base), groove).Shape());
}

// The volume of a shape inside the box.
double volume_within(const Shape& shape, double x0, double x1) {
  const TopoDS_Shape box = BRepPrimAPI_MakeBox(gp_Pnt(x0, -10, -10), gp_Pnt(x1, 50, 30)).Shape();
  return volume(Shape(BRepAlgoAPI_Common(copy_of(shape), box).Shape()));
}

void test_a_copy_stands_on_the_faces_under_its_seam() {
  const Shape before = plate();
  const Shape after = pinned(before);
  const FeatureChanges changes = feature_changes(before, after);
  CHECK(changes.added.size() == 1);
  if (changes.added.size() != 1) {
    return;
  }
  const AddedMaterial& pin = changes.added.front();
  // The seam is the disc where the foot's rounding ends on the plate.
  CHECK(pin.seam != nullptr);
  if (!pin.seam) {
    return;
  }
  CHECK(pin.seam->face_count() == 1);
  CHECK_NEAR_TOL(mass_properties(*pin.seam).area, kPi * 25.0, 1e-6);

  const Shape body = grooved(plate());
  // The pin at x = 10 stands on the plate; at x = 30 its seam (x 25..35)
  // is over the groove (x 28..34); at x = 31 (x 26..36) it lies over the
  // groove too, and at x = 80 off the plate.
  const auto copy = [&](double dx) { return moved(*pin.solid, dx, 0, ""); };
  const auto seam = [&](double dx) { return moved(*pin.seam, dx, 0, ""); };
  const std::vector<ShapePtr> solids = {copy(0), copy(20), copy(40), copy(70)};
  const std::vector<ShapePtr> seams = {seam(0), seam(20), seam(40), seam(70)};
  const std::vector<ShapePtr> stands = supported_copies(body, solids, seams);
  CHECK(stands.size() == 4);
  if (stands.size() != 4) {
    return;
  }
  // x = 10: all of it.
  CHECK(stands[0] != nullptr);
  if (stands[0]) {
    CHECK_NEAR(volume(*stands[0]), volume(*pin.solid));
  }
  // x = 30: only what is over the plate's top, x 25..28 and 34..35.
  CHECK(stands[1] != nullptr);
  if (stands[1]) {
    const double whole = volume(*solids[1]);
    const double over_plate = volume_within(*solids[1], 0, 28) + volume_within(*solids[1], 34, 100);
    CHECK(over_plate < whole);
    CHECK_NEAR_TOL(volume(*stands[1]), over_plate, 1e-6);
    CHECK(valid(*stands[1]));
    const BoundingBox box = bounding_box(*stands[1]);
    CHECK(near(box.min.X(), 25.0, 1e-6) && near(box.max.X(), 35.0, 1e-6));
  }
  // x = 50: wholly on the plate again.
  CHECK(stands[2] != nullptr);
  if (stands[2]) {
    CHECK_NEAR(volume(*stands[2]), volume(*pin.solid));
  }
  // x = 80: nothing under it.
  CHECK(stands[3] == nullptr);
}

void test_removed_material_is_the_hole_with_its_chamfer() {
  const Shape before = plate();
  const Shape after = drilled(before);
  const FeatureChanges changes = feature_changes(before, after);
  CHECK(changes.added.empty());
  CHECK(changes.removed.size() == 1);
  if (changes.removed.size() != 1) {
    return;
  }
  const Shape& hole = *changes.removed.front();
  CHECK_NEAR(volume(hole), volume(before) - volume(after));
  CHECK(valid(hole));

  const ShapePtr second = moved(hole, 20, 0, "F9:inst1");
  const ShapePtr third = moved(hole, 40, 0, "F9:inst2");
  const ShapePtr both = unite({second.get(), third.get()});
  const BooleanResult cut = boolean(BooleanOp::Cut, {&after}, *both);
  CHECK(cut.touched == std::vector<bool>{true});
  CHECK(cut.pieces.size() == 1);
  if (cut.pieces.size() != 1) {
    return;
  }
  const Shape& result = *cut.pieces.front().shape;
  CHECK(valid(result));
  CHECK_NEAR(volume(result), volume(after) - 2.0 * volume(hole));
  // Three holes with their chamfers: the plate's six faces, a wall and a
  // bevel each.
  CHECK(result.face_count() == 6 + 3 * 2);
}

void test_nothing_changed() {
  const Shape before = plate();
  const Shape after = pinned(before);
  const FeatureChanges same = feature_changes(after, after);
  CHECK(same.added.empty() && same.removed.empty());
}

void test_a_hole_filled_adds_material() {
  // The bodies the other way round: the hole's material is what `after`
  // has more, nothing is removed.
  const Shape before = drilled(plate());
  const Shape after = plate();
  const FeatureChanges changes = feature_changes(before, after);
  CHECK(changes.removed.empty());
  CHECK(changes.added.size() == 1);
}

} // namespace

void face_copy_tests() {
  guarded("face copy: added", test_added_material_is_the_pin_with_its_roundings);
  guarded("face copy: standing on faces", test_a_copy_stands_on_the_faces_under_its_seam);
  guarded("face copy: removed", test_removed_material_is_the_hole_with_its_chamfer);
  guarded("face copy: nothing changed", test_nothing_changed);
  guarded("face copy: a hole filled", test_a_hole_filled_adds_material);
}

} // namespace test
