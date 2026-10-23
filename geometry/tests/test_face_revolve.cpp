// SPDX-License-Identifier: MIT
// A planar face of a body revolved as a profile (face_revolve.hpp,
// mitcad#539): a block's side turned a quarter about the edge it has on the
// X axis, to two sides and a full turn; a ring's top turned about a line
// in its plane outside it, its hole kept; faces that are not planar, not
// there, crossed by the axis or pierced by it refused.

#include <cmath>
#include <string>

#include <BRepBndLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Ax1.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

#include "check.hpp"
#include "mitcad/geometry/face_revolve.hpp"
#include "mitcad/geometry/naming.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

double volume_of(const Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape.occt(), props);
  return props.Mass();
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

const gp_Ax1 kXAxis(gp_Pnt(0.0, 0.0, 0.0), gp_Dir(1.0, 0.0, 0.0));

FaceRevolveSpec spec_of(const std::string& face, const gp_Ax1& axis, double angle1) {
  FaceRevolveSpec spec;
  spec.feature = "F3";
  spec.face = face;
  spec.key = face;
  spec.axis = axis;
  spec.angle1 = angle1;
  return spec;
}

// A 60 x 40 x 20 block's side at y = 0 (its edge at z = 0 on the X axis)
// turned a quarter about the X axis: a quarter of a cylinder of radius 20,
// 60 long, below y = 0; the side's edges off the axis name the sides, the
// side the caps; the block stays as it was. Then a quarter and an eighth
// to two sides, and a full turn without caps.
void test_block_side_turned() {
  const ShapePtr block = extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20);
  const std::string face = side("F2", 1, 0);
  const std::string start = face_name("F3", "start", face);
  const std::string end = face_name("F3", "end", face);
  double x0, y0, z0, x1, y1, z1;

  const ShapePtr quarter = revolve_face(*block, spec_of(face, kXAxis, kPi / 2.0));
  CHECK(BRepCheck_Analyzer(quarter->occt()).IsValid());
  CHECK_NEAR(volume_of(*quarter), kPi / 4.0 * 20.0 * 20.0 * 60.0);
  bounds(quarter->occt(), x0, y0, z0, x1, y1, z1);
  CHECK_NEAR_TOL(x0, 0.0, 1e-7);
  CHECK_NEAR_TOL(x1, 60.0, 1e-7);
  CHECK_NEAR_TOL(y0, -20.0, 1e-7);
  CHECK_NEAR_TOL(y1, 0.0, 1e-7);
  CHECK_NEAR_TOL(z0, 0.0, 1e-7);
  CHECK_NEAR_TOL(z1, 20.0, 1e-7);
  CHECK(faces_named(*quarter, start) == 1);
  CHECK(faces_named(*quarter, end) == 1);
  CHECK(face_middle(*quarter, start).Distance(gp_Pnt(30.0, 0.0, 10.0)) < 1e-7);
  CHECK(face_middle(*quarter, end).Distance(gp_Pnt(30.0, -10.0, 0.0)) < 1e-7);
  // The top edge sweeps the cylinder, the ends quarter discs; the edge on
  // the axis sweeps nothing.
  CHECK(faces_named(*quarter, face_name("F3", "side", edge(face, end_cap("F2", 1)))) == 1);
  CHECK(faces_named(*quarter, face_name("F3", "side", edge(face, side("F2", 1, 1)))) == 1);
  CHECK(faces_named(*quarter, face_name("F3", "side", edge(face, side("F2", 1, 3)))) == 1);
  CHECK(quarter->face_count() == 5);
  CHECK(edge_names_unique(*quarter));

  // To two sides: the end of side one is the start, of side two the end.
  FaceRevolveSpec two = spec_of(face, kXAxis, kPi / 2.0);
  two.angle2 = kPi / 4.0;
  const ShapePtr both = revolve_face(*block, two);
  CHECK(BRepCheck_Analyzer(both->occt()).IsValid());
  CHECK_NEAR(volume_of(*both), 3.0 * kPi / 8.0 * 20.0 * 20.0 * 60.0);
  CHECK(face_middle(*both, start).Distance(gp_Pnt(30.0, -10.0, 0.0)) < 1e-7);
  const double c = 10.0 * std::cos(kPi / 4.0);
  CHECK(face_middle(*both, end).Distance(gp_Pnt(30.0, c, c)) < 1e-7);

  // A full turn: a cylinder, no caps.
  const ShapePtr full = revolve_face(*block, spec_of(face, kXAxis, 2.0 * kPi));
  CHECK(BRepCheck_Analyzer(full->occt()).IsValid());
  CHECK_NEAR(volume_of(*full), kPi * 20.0 * 20.0 * 60.0);
  CHECK(faces_named(*full, start) == 0);
  CHECK(faces_named(*full, end) == 0);

  CHECK_NEAR(volume_of(*block), 60.0 * 40.0 * 20.0);
  CHECK(block->face_count() == 6);
}

// A ring's top (a circle of radius 10 with a hole of radius 4, at z = 10)
// turned a full turn about the line y = 20 in its plane: a solid ring of
// the annulus's area times the way its centre goes (Pappus).
void test_annular_face_keeps_its_hole() {
  Region ring = circle(1, 0, 0, 10);
  ring.loops.push_back(circle(2, 0, 0, 4).loops.front());
  const ShapePtr body = extrude("F2", {ring}, 0, 10);
  const std::string top = face_name("F2", "end", "r{c1}");
  CHECK(faces_named(*body, top) == 1);
  const gp_Ax1 axis(gp_Pnt(0.0, 20.0, 10.0), gp_Dir(1.0, 0.0, 0.0));
  const ShapePtr turned = revolve_face(*body, spec_of(top, axis, 2.0 * kPi));
  CHECK(BRepCheck_Analyzer(turned->occt()).IsValid());
  CHECK_NEAR(volume_of(*turned), kPi * (10.0 * 10.0 - 4.0 * 4.0) * 2.0 * kPi * 20.0);
  CHECK(faces_named(*turned, face_name("F3", "side", edge(top, face_name("F2", "side", "c1")))) == 1);
  CHECK(faces_named(*turned, face_name("F3", "side", edge(top, face_name("F2", "side", "c2")))) == 1);
}

// A rod's side is no plane; a name the body has no face of; the axis across
// the block's side or through its top; an angle of zero: errors.
void test_refusals() {
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 5)}, 0, 20);
  CHECK(throws_with([&] { revolve_face(*rod, spec_of(face_name("F2", "side", "c1"), kXAxis, 1.0)); },
                    "is not planar"));
  CHECK(throws_with([&] { revolve_face(*rod, spec_of(face_name("F2", "top"), kXAxis, 1.0)); }, "has no face"));
  const ShapePtr block = extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20);
  const gp_Ax1 upright(gp_Pnt(30.0, 0.0, 0.0), gp_Dir(0.0, 0.0, 1.0));
  CHECK(throws_with([&] { revolve_face(*block, spec_of(side("F2", 1, 0), upright, 1.0)); },
                    "crosses the revolve axis"));
  const gp_Ax1 through(gp_Pnt(30.0, 20.0, 0.0), gp_Dir(0.0, 0.0, 1.0));
  CHECK(throws_with([&] { revolve_face(*block, spec_of(end_cap("F2", 1), through, 1.0)); },
                    "passes through face"));
  CHECK(throws_with([&] { revolve_face(*block, spec_of(side("F2", 1, 0), kXAxis, 0.0)); },
                    "must be greater than zero"));
}

} // namespace

void face_revolve_tests() {
  guarded("test_block_side_turned", test_block_side_turned);
  guarded("test_annular_face_keeps_its_hole", test_annular_face_keeps_its_hole);
  guarded("test_refusals", test_refusals);
}

} // namespace test
