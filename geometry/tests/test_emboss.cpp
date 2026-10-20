// SPDX-License-Identifier: MIT
// Emboss: profiles wrapped onto cylinders (mitcad#458). Volumes are
// analytic: a region of area A in the sketch, wrapped onto the cylinder of
// radius r and grown radially to R, is A |R^2 - r^2| / (2 r).

#include <cmath>
#include <cstdio>
#include <string>
#include <vector>

#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <GProp_GProps.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_Ax2.hxx>

#include "check.hpp"
#include "mitcad/geometry/emboss.hpp"

namespace {

using namespace mitcad::geometry;

double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props);
  return props.Mass();
}

int solids(const TopoDS_Shape& shape) {
  int n = 0;
  for (TopExp_Explorer e(shape, TopAbs_SOLID); e.More(); e.Next()) {
    ++n;
  }
  return n;
}

// A rectangle with named sides c1..c4 and the region name r1.
Region rectangle(double x, double y, double width, double height, const std::string& name = "r1") {
  Region region = rectangle_region(x, y, width, height);
  region.name = name;
  int i = 0;
  for (Segment& s : region.loops.front().segments) {
    s.name = name + "c" + std::to_string(++i);
  }
  return region;
}

double wrapped_volume(double area, double r, double to) { return area * std::abs(to * to - r * r) / (2.0 * r); }

// The sketch plane tangent to the cylinder of radius 10 about Z at +Y, its
// x along -X (so that s runs along it) and y along Z.
EmbossWrapSpec spec_on_y(const std::vector<Region>& regions, double to) {
  EmbossWrapSpec spec;
  spec.feature = "F5";
  spec.frame.origin = gp_Pnt(0.0, 10.0, 0.0);
  spec.frame.x_axis = gp_Dir(-1.0, 0.0, 0.0);
  spec.frame.y_axis = gp_Dir(0.0, 0.0, 1.0);
  spec.regions = regions;
  spec.axis = gp_Ax1(gp_Pnt(0.0, 0.0, 0.0), gp_Dir(0.0, 0.0, 1.0));
  spec.radius = 10.0;
  spec.to_radius = to;
  return spec;
}

void test_a_rectangle_raised_and_sunk() {
  for (const double to : {11.0, 9.0}) {
    const ShapePtr tool = wrapped_emboss(spec_on_y({rectangle(-2.0, 5.0, 4.0, 5.0)}, to));
    CHECK(BRepCheck_Analyzer(tool->occt()).IsValid());
    CHECK(solids(tool->occt()) == 1);
    CHECK_NEAR(volume_of(tool->occt()), wrapped_volume(20.0, 10.0, to));
    CHECK_NEAR(mass_properties(*tool).volume, wrapped_volume(20.0, 10.0, to));
    CHECK(tool->find_faces("F5:start(r1)").size() == 1);
    CHECK(tool->find_faces("F5:end(r1)").size() == 1);
    for (int i = 1; i <= 4; ++i) {
      CHECK(tool->find_faces("F5:side(r1c" + std::to_string(i) + ")").size() == 1);
    }
    // Joined to or cut from the rod (radius 10, 20 long): its volume grows
    // or shrinks by the tool's.
    const TopoDS_Shape rod = BRepPrimAPI_MakeCylinder(10.0, 20.0).Shape();
    const double before = volume_of(rod);
    if (to > 10.0) {
      BRepAlgoAPI_Fuse fuse(rod, tool->occt());
      CHECK(fuse.IsDone() && BRepCheck_Analyzer(fuse.Shape()).IsValid());
      CHECK_NEAR(volume_of(fuse.Shape()) - before, 21.0);
    } else {
      BRepAlgoAPI_Cut cut(rod, tool->occt());
      CHECK(cut.IsDone() && BRepCheck_Analyzer(cut.Shape()).IsValid());
      CHECK_NEAR(before - volume_of(cut.Shape()), 19.0);
    }
    // The tool lies about +Y (s = 0 under the sketch's middle), within the
    // profile's heights.
    Bnd_Box box;
    BRepBndLib::Add(tool->occt(), box);
    double xmin = 0, ymin = 0, zmin = 0, xmax = 0, ymax = 0, zmax = 0;
    box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
    CHECK(std::abs(xmin + xmax) < 1e-6 && ymin > 8.5 && zmin > 4.9 && zmax < 10.1);
  }
}

void test_curves_holes_and_a_rotated_axis() {
  // A circle (a B-spline on the cylinder) and a rectangle with a hole, on
  // a cylinder of radius 10 about X through (123, -456, 789).
  const gp_Pnt origin(123.0, -456.0, 789.0);
  Region holed = rectangle(-2.0, 5.0, 4.0, 5.0, "r2");
  Region hole = rectangle(-1.0, 6.0, 2.0, 3.0, "h");
  holed.loops.push_back(hole.loops.front());
  Region circle = circle_region(6.0, 7.5, 1.5);
  circle.name = "r3";
  circle.loops.front().segments.front().name = "c9";
  for (const double to : {11.0, 9.0}) {
    EmbossWrapSpec spec;
    spec.feature = "F7";
    spec.frame.origin = origin.Translated(gp_Vec(0.0, 10.0, 0.0));
    spec.frame.x_axis = gp_Dir(0.0, 0.0, 1.0);
    spec.frame.y_axis = gp_Dir(1.0, 0.0, 0.0);
    spec.regions = {holed, circle};
    spec.axis = gp_Ax1(origin, gp_Dir(1.0, 0.0, 0.0));
    spec.radius = 10.0;
    spec.to_radius = to;
    const ShapePtr tool = wrapped_emboss(spec);
    CHECK(BRepCheck_Analyzer(tool->occt()).IsValid());
    CHECK(solids(tool->occt()) == 2);
    const double area = 20.0 - 6.0 + test::kPi * 1.5 * 1.5;
    CHECK_NEAR_TOL(mass_properties(*tool).volume, wrapped_volume(area, 10.0, to), 1e-6);
    CHECK(tool->find_faces("F7:side(c9)").size() == 1);
    CHECK(tool->find_faces("F7:end(r3)").size() == 1);
    CHECK(tool->find_faces("F7:side(hc1)").size() == 1);
  }
}

void test_a_bore_and_a_sketch_off_the_cylinder() {
  // Raised into a bore of radius 10 (towards the axis) from a sketch plane
  // 30 away from it: lengths are the cylinder's, wherever the sketch lies.
  EmbossWrapSpec spec = spec_on_y({rectangle(-2.0, 5.0, 4.0, 5.0)}, 9.5);
  spec.frame.origin = gp_Pnt(0.0, 30.0, 0.0);
  const ShapePtr tool = wrapped_emboss(spec);
  CHECK(BRepCheck_Analyzer(tool->occt()).IsValid());
  CHECK_NEAR(mass_properties(*tool).volume, wrapped_volume(20.0, 10.0, 9.5));
  // Every point of the tool lies on the +Y side.
  Bnd_Box box;
  BRepBndLib::Add(tool->occt(), box);
  double xmin = 0, ymin = 0, zmin = 0, xmax = 0, ymax = 0, zmax = 0;
  box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
  CHECK(ymin > 9.0);
  CHECK(zmin > 4.9 && zmax < 10.1);
}

void test_refusals() {
  EmbossWrapSpec across = spec_on_y({rectangle(-2.0, 5.0, 4.0, 5.0)}, 11.0);
  across.axis = gp_Ax1(gp_Pnt(0.0, 0.0, 0.0), gp_Dir(0.0, 1.0, 0.0));
  CHECK(test::throws_with([&] { wrapped_emboss(across); }, "parallel to its axis"));
  const EmbossWrapSpec wide = spec_on_y({rectangle(-40.0, 5.0, 80.0, 5.0)}, 11.0);
  CHECK(test::throws_with([&] { wrapped_emboss(wide); }, "wider than its circumference"));
  const EmbossWrapSpec flat = spec_on_y({rectangle(-2.0, 5.0, 4.0, 5.0)}, 10.0);
  CHECK(test::throws_with([&] { wrapped_emboss(flat); }, "no depth"));
}

} // namespace

int main() {
  test::guarded("a rectangle raised and sunk", test_a_rectangle_raised_and_sunk);
  test::guarded("curves, holes and a rotated axis", test_curves_holes_and_a_rotated_axis);
  test::guarded("a bore and a sketch off the cylinder", test_a_bore_and_a_sketch_off_the_cylinder);
  test::guarded("refusals", test_refusals);
  if (test::failures == 0) {
    std::puts("test_emboss: all checks passed");
  }
  return test::failures == 0 ? 0 : 1;
}
