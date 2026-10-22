// SPDX-License-Identifier: MIT
// A planar face of a body extruded as a profile (face_extrude.hpp,
// mitcad#535): a block's top out of it, into it, to two sides and from an
// offset; an annular face with its hole; an oblique face along its own
// normal; faces that are not planar or not there refused.

#include <cmath>
#include <string>
#include <vector>

#include <BRepBndLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Vec.hxx>

#include "check.hpp"
#include "mitcad/geometry/extrude.hpp"
#include "mitcad/geometry/face_extrude.hpp"
#include "mitcad/geometry/naming.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

double volume_of(const Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape.occt(), props);
  return props.Mass();
}

gp_Pnt centre_of(const Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape.occt(), props);
  return props.CentreOfMass();
}

// The box of the shape, exact to its geometry (no tolerance added).
void bounds(const TopoDS_Shape& shape, double& x0, double& y0, double& z0, double& x1, double& y1,
            double& z1) {
  Bnd_Box box;
  BRepBndLib::AddOptimal(shape, box, false, false);
  box.Get(x0, y0, z0, x1, y1, z1);
}

// The middle of the face named `name` (the centre of its area).
gp_Pnt face_middle(const Shape& shape, const std::string& name) {
  for (int f = 0; f < shape.face_count(); ++f) {
    if (has_name(shape.face_names(f), name)) {
      GProp_GProps props;
      BRepGProp::SurfaceProperties(shape.face(f), props);
      return props.CentreOfMass();
    }
  }
  return gp_Pnt(1e9, 1e9, 1e9);
}

FaceExtrudeSpec spec_of(const std::string& face, double distance1) {
  FaceExtrudeSpec spec;
  spec.feature = "F3";
  spec.face = face;
  spec.key = face;
  spec.distance1 = distance1;
  return spec;
}

// A 60 x 40 x 20 block's top extruded 5 out of it, 5 into it, 5 and 3 to
// two sides and 5 from 2 above it; its edges name the sides, the top the
// caps; the block stays as it was.
void test_block_top_extruded() {
  const ShapePtr block = extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20);
  const std::string top = end_cap("F2", 1);
  const std::string start = face_name("F3", "start", top);
  const std::string end = face_name("F3", "end", top);
  double x0, y0, z0, x1, y1, z1;

  const ShapePtr out = extrude_face(*block, spec_of(top, 5.0));
  CHECK(BRepCheck_Analyzer(out->occt()).IsValid());
  CHECK_NEAR(volume_of(*out), 60.0 * 40.0 * 5.0);
  CHECK(out->face_count() == 6);
  bounds(out->occt(), x0, y0, z0, x1, y1, z1);
  CHECK_NEAR_TOL(x0, 0.0, 1e-7);
  CHECK_NEAR_TOL(x1, 60.0, 1e-7);
  CHECK_NEAR_TOL(y0, 0.0, 1e-7);
  CHECK_NEAR_TOL(y1, 40.0, 1e-7);
  CHECK_NEAR_TOL(z0, 20.0, 1e-7);
  CHECK_NEAR_TOL(z1, 25.0, 1e-7);
  CHECK(faces_named(*out, start) == 1);
  CHECK(faces_named(*out, end) == 1);
  CHECK_NEAR_TOL(face_middle(*out, start).Z(), 20.0, 1e-7);
  CHECK_NEAR_TOL(face_middle(*out, end).Z(), 25.0, 1e-7);
  for (int i = 0; i < 4; ++i) {
    CHECK(faces_named(*out, face_name("F3", "side", edge(top, side("F2", 1, i)))) == 1);
  }
  CHECK(edge_names_unique(*out));

  // Into the block (a cut's tool).
  FaceExtrudeSpec into = spec_of(top, 5.0);
  into.reversed = true;
  const ShapePtr in = extrude_face(*block, into);
  CHECK(BRepCheck_Analyzer(in->occt()).IsValid());
  CHECK_NEAR(volume_of(*in), 60.0 * 40.0 * 5.0);
  bounds(in->occt(), x0, y0, z0, x1, y1, z1);
  CHECK_NEAR_TOL(z0, 15.0, 1e-7);
  CHECK_NEAR_TOL(z1, 20.0, 1e-7);
  CHECK_NEAR_TOL(face_middle(*in, start).Z(), 20.0, 1e-7);
  CHECK_NEAR_TOL(face_middle(*in, end).Z(), 15.0, 1e-7);

  // Two sides: the far cap of side one is the start, of side two the end.
  FaceExtrudeSpec two = spec_of(top, 5.0);
  two.distance2 = 3.0;
  const ShapePtr both = extrude_face(*block, two);
  CHECK(BRepCheck_Analyzer(both->occt()).IsValid());
  CHECK_NEAR(volume_of(*both), 60.0 * 40.0 * 8.0);
  bounds(both->occt(), x0, y0, z0, x1, y1, z1);
  CHECK_NEAR_TOL(z0, 17.0, 1e-7);
  CHECK_NEAR_TOL(z1, 25.0, 1e-7);
  CHECK_NEAR_TOL(face_middle(*both, start).Z(), 25.0, 1e-7);
  CHECK_NEAR_TOL(face_middle(*both, end).Z(), 17.0, 1e-7);

  // From 2 above the face.
  FaceExtrudeSpec offset = spec_of(top, 5.0);
  offset.start_offset = 2.0;
  const ShapePtr above = extrude_face(*block, offset);
  bounds(above->occt(), x0, y0, z0, x1, y1, z1);
  CHECK_NEAR_TOL(z0, 22.0, 1e-7);
  CHECK_NEAR_TOL(z1, 27.0, 1e-7);

  CHECK_NEAR(volume_of(*block), 60.0 * 40.0 * 20.0);
  CHECK(block->face_count() == 6);
}

// A ring's top (a circle of radius 10 with a hole of radius 4) extruded 5:
// the hole goes through the prism, which has a side from each circle.
void test_annular_face_keeps_its_hole() {
  Region ring = circle(1, 0, 0, 10);
  ring.loops.push_back(circle(2, 0, 0, 4).loops.front());
  const ShapePtr body = extrude("F2", {ring}, 0, 10);
  const std::string top = face_name("F2", "end", "r{c1}");
  CHECK(faces_named(*body, top) == 1);
  const ShapePtr prism = extrude_face(*body, spec_of(top, 5.0));
  CHECK(BRepCheck_Analyzer(prism->occt()).IsValid());
  CHECK_NEAR(volume_of(*prism), kPi * (10.0 * 10.0 - 4.0 * 4.0) * 5.0);
  CHECK(prism->face_count() == 4);
  CHECK(faces_named(*prism, face_name("F3", "side", edge(top, face_name("F2", "side", "c1")))) == 1);
  CHECK(faces_named(*prism, face_name("F3", "side", edge(top, face_name("F2", "side", "c2")))) == 1);
  double x0, y0, z0, x1, y1, z1;
  bounds(prism->occt(), x0, y0, z0, x1, y1, z1);
  CHECK_NEAR_TOL(x0, -10.0, 1e-7);
  CHECK_NEAR_TOL(x1, 10.0, 1e-7);
  CHECK_NEAR_TOL(z0, 10.0, 1e-7);
  CHECK_NEAR_TOL(z1, 15.0, 1e-7);
  // The centre of mass of a ring lies in its hole.
  const gp_Pnt centre = centre_of(*prism);
  CHECK_NEAR_TOL(centre.X(), 0.0, 1e-7);
  CHECK_NEAR_TOL(centre.Y(), 0.0, 1e-7);
  CHECK_NEAR_TOL(centre.Z(), 12.5, 1e-7);
}

// A 20 x 10 plate on a plane tilted 30 degrees about X, 8 thick along its
// normal: its far face extruded 5 goes along that face's own normal.
void test_oblique_face_uses_its_own_normal() {
  const double a = 30.0 * kPi / 180.0;
  ExtrudeSpec plate;
  plate.feature = "F2";
  plate.frame.x_axis = gp_Dir(1.0, 0.0, 0.0);
  plate.frame.y_axis = gp_Dir(0.0, std::cos(a), std::sin(a));
  plate.regions = {rectangle(1, 0, 0, 20, 10)};
  plate.direction = plate.frame.normal();
  plate.start = 0.0;
  plate.end = 8.0;
  const ShapePtr body = mitcad::geometry::extrude(plate);
  const gp_Dir normal = plate.frame.normal();
  const std::string farCap = end_cap("F2", 1);
  // The far face's middle: the rectangle's middle moved 8 along the normal.
  const gp_Pnt middle = plate.frame.point(gp_Pnt2d(10.0, 5.0)).Translated(gp_Vec(normal) * 8.0);
  CHECK(face_middle(*body, farCap).Distance(middle) < 1e-7);

  const ShapePtr prism = extrude_face(*body, spec_of(farCap, 5.0));
  CHECK(BRepCheck_Analyzer(prism->occt()).IsValid());
  CHECK_NEAR(volume_of(*prism), 20.0 * 10.0 * 5.0);
  CHECK(prism->face_count() == 6);
  CHECK(centre_of(*prism).Distance(middle.Translated(gp_Vec(normal) * 2.5)) < 1e-7);
  CHECK(face_middle(*prism, face_name("F3", "start", farCap)).Distance(middle) < 1e-7);
  CHECK(face_middle(*prism, face_name("F3", "end", farCap)).Distance(middle.Translated(gp_Vec(normal) * 5.0)) <
        1e-7);
}

// A rod's side is no plane; a name the body has no face of, a distance of
// zero: errors, nothing made.
void test_refusals() {
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 5)}, 0, 20);
  CHECK(throws_with([&] { extrude_face(*rod, spec_of(face_name("F2", "side", "c1"), 5.0)); },
                    "is not planar"));
  CHECK(throws_with([&] { extrude_face(*rod, spec_of(face_name("F2", "top"), 5.0)); }, "has no face"));
  CHECK(throws_with([&] { extrude_face(*rod, spec_of(face_name("F2", "end", "r{c1}"), 0.0)); },
                    "must be greater than zero"));
}

} // namespace

void face_extrude_tests() {
  guarded("test_block_top_extruded", test_block_top_extruded);
  guarded("test_annular_face_keeps_its_hole", test_annular_face_keeps_its_hole);
  guarded("test_oblique_face_uses_its_own_normal", test_oblique_face_uses_its_own_normal);
  guarded("test_refusals", test_refusals);
}

} // namespace test
