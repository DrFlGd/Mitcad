// SPDX-License-Identifier: MIT
// Section curves of sketches (mitcad#389): where a plane cuts a shape's
// faces, a piece per face, B-spline pieces that meet end to end joined.

#include <cmath>
#include <string>
#include <vector>

#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <GeomAPI_PointsToBSpline.hxx>
#include <Geom_BSplineCurve.hxx>
#include <NCollection_Array1.hxx>
#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>

#include "check.hpp"
#include "mitcad/geometry/shape.hpp"
#include "mitcad/geometry/sketch_ref.hpp"

namespace test {
namespace {

using mitcad::geometry::ModelCurveKind;
using mitcad::geometry::SectionCurve;
using mitcad::geometry::Shape;

// A B-spline through points of the plane z = 0.
occ::handle<Geom_BSplineCurve> spline(const std::vector<gp_Pnt>& points) {
  NCollection_Array1<gp_Pnt> array(1, static_cast<int>(points.size()));
  for (std::size_t i = 0; i < points.size(); ++i) {
    array(static_cast<int>(i) + 1) = points[i];
  }
  return GeomAPI_PointsToBSpline(array, 3, 3).Curve();
}

const gp_Pnt& first_pole(const SectionCurve& c) { return c.curve.poles.front(); }

const gp_Pnt& last_pole(const SectionCurve& c) { return c.curve.poles.back(); }

void test_a_box_cut_through_its_middle() {
  // The plane y = 5 through a 10 mm cube: a line on each of the four faces
  // it crosses, none on the two faces parallel to it.
  const Shape box(BRepPrimAPI_MakeBox(10.0, 10.0, 10.0).Shape());
  const std::vector<SectionCurve> curves =
      mitcad::geometry::section_curves(box, "", gp_Pnt(0.0, 5.0, 0.0), gp_Dir(0.0, 1.0, 0.0));
  CHECK(curves.size() == 4);
  double length = 0.0;
  for (const SectionCurve& c : curves) {
    CHECK(c.curve.kind == ModelCurveKind::Line);
    CHECK(near(c.curve.start.Y(), 5.0) && near(c.curve.end.Y(), 5.0));
    length += c.curve.start.Distance(c.curve.end);
  }
  CHECK_NEAR_TOL(length, 40.0, 1e-9);
  // A plane that misses it cuts nothing.
  CHECK(mitcad::geometry::section_curves(box, "", gp_Pnt(0.0, 20.0, 0.0), gp_Dir(0.0, 1.0, 0.0)).empty());
}

void test_spline_pieces_end_to_end_are_one_curve() {
  // A prism on a profile of two splines end to end and a line closing them:
  // cut across, the splines' faces give pieces that meet where only they
  // end, one curve; the line's face gives a line.
  const gp_Pnt a(0.0, 0.0, 0.0);
  const gp_Pnt b(10.0, 0.0, 0.0);
  const gp_Pnt c(20.0, 0.0, 0.0);
  const occ::handle<Geom_BSplineCurve> s1 = spline({a, gp_Pnt(3.0, 4.0, 0.0), gp_Pnt(7.0, 5.0, 0.0), b});
  const occ::handle<Geom_BSplineCurve> s2 = spline({b, gp_Pnt(13.0, -3.0, 0.0), gp_Pnt(17.0, 6.0, 0.0), c});
  BRepBuilderAPI_MakeWire wire;
  wire.Add(BRepBuilderAPI_MakeEdge(s1).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(s2).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(c, a).Edge());
  const TopoDS_Face profile = BRepBuilderAPI_MakeFace(wire.Wire(), true).Face();
  const Shape prism(BRepPrimAPI_MakePrism(profile, gp_Vec(0.0, 0.0, 8.0)).Shape());
  const std::vector<SectionCurve> curves =
      mitcad::geometry::section_curves(prism, "", gp_Pnt(0.0, 0.0, 3.0), gp_Dir(0.0, 0.0, 1.0));
  CHECK(curves.size() == 2);
  int splines = 0;
  for (const SectionCurve& curve : curves) {
    if (curve.curve.kind != ModelCurveKind::BSpline) {
      CHECK(curve.curve.kind == ModelCurveKind::Line);
      continue;
    }
    ++splines;
    // From one end of the profile's splines to the other, at z = 3.
    const gp_Pnt p0 = first_pole(curve);
    const gp_Pnt p1 = last_pole(curve);
    CHECK((p0.Distance(gp_Pnt(0, 0, 3)) < 1e-6 && p1.Distance(gp_Pnt(20, 0, 3)) < 1e-6) ||
          (p1.Distance(gp_Pnt(0, 0, 3)) < 1e-6 && p0.Distance(gp_Pnt(20, 0, 3)) < 1e-6));
    for (const gp_Pnt& pole : curve.curve.poles) {
      CHECK(std::abs(pole.Z() - 3.0) < 1e-6);
    }
  }
  CHECK(splines == 1);
}

} // namespace

void section_curve_tests() {
  test_a_box_cut_through_its_middle();
  test_spline_pieces_end_to_end_are_one_curve();
}

} // namespace test
