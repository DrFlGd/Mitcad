// SPDX-License-Identifier: MIT
// Emboss: profiles wrapped onto cylinders (mitcad#458). Volumes are
// analytic: a region of area A in the sketch, wrapped onto the cylinder of
// radius r and grown radially to R, is A |R^2 - r^2| / (2 r).

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <utility>
#include <string>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <GProp_GProps.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Ax2.hxx>

#include "check.hpp"
#include "mitcad/geometry/emboss.hpp"
#include "mitcad/geometry/text.hpp"

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

// The glyph outlines of a text (the bundled font, `height` mm high from a
// baseline at y) as regions, one per letter, its counters as holes, named
// r_<letter>; `left` the letters' flat extents along x.
std::vector<Region> text_regions(const std::string& text, double height, double x, double y,
                                 std::vector<std::pair<double, double>>& extents) {
  const TextOutlines outlines = text_outlines("", false, false, text);
  std::vector<Region> regions;
  double pen = 0.0;
  for (std::size_t k = 0; k < outlines.glyphs.size(); ++k) {
    const GlyphOutline& glyph = outlines.glyphs[k];
    std::vector<Loop> loops;
    std::vector<double> areas;
    std::vector<std::vector<gp_Pnt2d>> samples;
    double low = 1e300;
    double high = -1e300;
    int piece_number = 0;
    for (const auto& contour : glyph.contours) {
      Loop loop;
      std::vector<gp_Pnt2d> points;
      for (const auto& piece : contour) {
        std::vector<gp_Pnt2d> placed;
        for (const gp_Pnt2d& p : piece) {
          placed.emplace_back(x + (pen + p.X()) * height, y + p.Y() * height);
        }
        Segment s;
        s.name = text.substr(k, 1) + std::to_string(++piece_number);
        if (placed.size() == 2) {
          s.kind = SegmentKind::Line;
          s.start = placed[0];
          s.end = placed[1];
        } else {
          s.kind = SegmentKind::BSpline;
          s.degree = static_cast<int>(placed.size()) - 1;
          s.poles = placed;
          s.knots = {0.0, 1.0};
          s.multiplicities = {s.degree + 1, s.degree + 1};
        }
        loop.segments.push_back(s);
        // Points along the piece (de Casteljau) for its extent and area.
        for (int i = 0; i < 64; ++i) {
          std::vector<gp_Pnt2d> q = placed;
          const double t = i / 64.0;
          for (std::size_t n = q.size() - 1; n > 0; --n) {
            for (std::size_t j = 0; j < n; ++j) {
              q[j] = gp_Pnt2d(q[j].X() + t * (q[j + 1].X() - q[j].X()), q[j].Y() + t * (q[j + 1].Y() - q[j].Y()));
            }
          }
          points.push_back(q[0]);
          low = std::min(low, q[0].X());
          high = std::max(high, q[0].X());
        }
      }
      double area = 0.0;
      for (std::size_t i = 0; i < points.size(); ++i) {
        const gp_Pnt2d& p = points[i];
        const gp_Pnt2d& q = points[(i + 1) % points.size()];
        area += p.X() * q.Y() - q.X() * p.Y();
      }
      loops.push_back(loop);
      areas.push_back(area / 2.0);
      samples.push_back(points);
    }
    pen += glyph.advance;
    if (loops.empty()) {
      continue;
    }
    // The largest contour bounds the letter; the others are its counters.
    std::size_t outer = 0;
    for (std::size_t i = 1; i < areas.size(); ++i) {
      if (std::abs(areas[i]) > std::abs(areas[outer])) {
        outer = i;
      }
    }
    Region region;
    region.name = "r_" + text.substr(k, 1);
    region.loops.push_back(loops[outer]);
    for (std::size_t i = 0; i < loops.size(); ++i) {
      if (i != outer) {
        region.loops.push_back(loops[i]);
      }
    }
    regions.push_back(region);
    extents.emplace_back(low, high);
  }
  return regions;
}

// mitcad#473: "ABC" wrapped onto a cylinder 30 mm across and raised 1 mm.
// The letters' outer faces lie on the cylinder of radius 16, and each
// letter spans the angle of its flat width over the radius 15.
void test_text_wrapped_on_a_cylinder() {
  std::vector<std::pair<double, double>> extents;
  const std::vector<Region> regions = text_regions("ABC", 10.0, -10.0, 5.0, extents);
  CHECK(regions.size() == 3);
  EmbossWrapSpec spec;
  spec.feature = "F5";
  spec.frame.origin = gp_Pnt(0.0, 15.0, 0.0);
  spec.frame.x_axis = gp_Dir(-1.0, 0.0, 0.0);
  spec.frame.y_axis = gp_Dir(0.0, 0.0, 1.0);
  spec.regions = regions;
  spec.axis = gp_Ax1(gp_Pnt(0.0, 0.0, 0.0), gp_Dir(0.0, 0.0, 1.0));
  spec.radius = 15.0;
  spec.to_radius = 16.0;
  const ShapePtr tool = wrapped_emboss(spec);
  CHECK(BRepCheck_Analyzer(tool->occt()).IsValid());
  CHECK(solids(tool->occt()) == 3);
  for (std::size_t k = 0; k < regions.size(); ++k) {
    const std::string& name = regions[k].name;
    const std::vector<int> tops = tool->find_faces("F5:end(" + name + ")");
    CHECK(tops.size() == 1);
    if (tops.size() != 1) {
      continue;
    }
    const TopoDS_Face top = tool->face(tops.front());
    const BRepAdaptor_Surface surface(top);
    CHECK(surface.GetType() == GeomAbs_Cylinder);
    // Points of the face's edges: on the cylinder of radius 16 within
    // 1e-4 mm, and their angles from +Y (s along -X) span the letter.
    double low = 1e300;
    double high = -1e300;
    double worst = 0.0;
    for (TopExp_Explorer e(top, TopAbs_EDGE); e.More(); e.Next()) {
      const BRepAdaptor_Curve curve(TopoDS::Edge(e.Current()));
      for (int i = 0; i <= 256; ++i) {
        const gp_Pnt p = curve.Value(curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * i / 256.0);
        worst = std::max(worst, std::abs(std::hypot(p.X(), p.Y()) - 16.0));
        const double angle = std::atan2(-p.X(), p.Y());
        low = std::min(low, angle);
        high = std::max(high, angle);
      }
    }
    CHECK(worst < 1e-4);
    const double flat = (extents[k].second - extents[k].first) / 15.0;
    if (std::abs((high - low) - flat) > 1e-4) {
      std::printf("letter %s: angular width %.7f, flat width over the radius %.7f\n", name.c_str(), high - low, flat);
    }
    CHECK(std::abs((high - low) - flat) < 1e-4);
    // And where: the letter's middle at its flat middle's angle.
    const double middle = (extents[k].first + extents[k].second) / 2.0 / 15.0;
    CHECK(std::abs((high + low) / 2.0 - middle) < 1e-4);
  }
}

// mitcad#473: a rectangle developed onto a cone and raised along its
// normals. The cone of half angle 30 degrees about Z, its apex at the
// origin, opening upwards; the sketch beside it at the height 20 (radius
// 20 tan 30). Raised by 1: every point of the top face lies 1 off the
// cone; the face's area is the rectangle's scaled by the offset cone's
// spread at that distance, and the volume about the rectangle's area by
// the depth.
void test_a_rectangle_developed_on_a_cone() {
  const double angle = test::kPi / 6.0;
  const double r0 = 20.0 * std::tan(angle);
  EmbossWrapSpec spec;
  spec.feature = "F6";
  spec.frame.origin = gp_Pnt(0.0, r0, 20.0);
  spec.frame.x_axis = gp_Dir(-1.0, 0.0, 0.0);
  spec.frame.y_axis = gp_Dir(0.0, 0.0, 1.0);
  spec.regions = {rectangle(-3.0, -2.0, 6.0, 4.0)};
  spec.axis = gp_Ax1(gp_Pnt(0.0, 0.0, 0.0), gp_Dir(0.0, 0.0, 1.0));
  spec.radius = r0;
  spec.to_radius = r0 + 1.0;
  spec.half_angle = angle;
  for (const double offset : {1.0, -1.0}) {
    spec.to_radius = r0 + offset;
    const ShapePtr tool = wrapped_emboss(spec);
    CHECK(BRepCheck_Analyzer(tool->occt()).IsValid());
    CHECK(solids(tool->occt()) == 1);
    CHECK(tool->find_faces("F6:end(r1)").size() == 1);
    CHECK(tool->find_faces("F6:side(r1c1)").size() == 1);
    // The bottom face lies on the cone: area 24 (a development keeps
    // areas), the volume about 24 by the depth.
    const std::vector<int> bottom = tool->find_faces("F6:start(r1)");
    CHECK(bottom.size() == 1);
    if (!bottom.empty()) {
      GProp_GProps props;
      BRepGProp::SurfaceProperties(tool->face(bottom.front()), props);
      CHECK_NEAR_TOL(props.Mass(), 24.0, 1e-6);
    }
    const double volume = volume_of(tool->occt());
    CHECK(std::abs(volume - 24.0) < 24.0 * 0.15);
    // Points of the top face's edges: 1 from the cone.
    const std::vector<int> top = tool->find_faces("F6:end(r1)");
    double worst = 0.0;
    if (!top.empty()) {
      for (TopExp_Explorer e(tool->face(top.front()), TopAbs_EDGE); e.More(); e.Next()) {
        const BRepAdaptor_Curve curve(TopoDS::Edge(e.Current()));
        for (int i = 0; i <= 64; ++i) {
          const gp_Pnt p = curve.Value(curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * i / 64.0);
          // Distance from the cone z = r / tan(angle): (r cos - z sin).
          const double distance = std::hypot(p.X(), p.Y()) * std::cos(angle) - p.Z() * std::sin(angle);
          worst = std::max(worst, std::abs(distance - offset));
        }
      }
    }
    CHECK(worst < 1e-6);
  }
  EmbossWrapSpec past = spec;
  past.regions = {rectangle(-3.0, -60.0, 6.0, 4.0)};
  past.to_radius = r0 + 1.0;
  CHECK(test::throws_with([&] { wrapped_emboss(past); }, "apex"));
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
  test::guarded("text wrapped on a cylinder", test_text_wrapped_on_a_cylinder);
  test::guarded("a rectangle developed on a cone", test_a_rectangle_developed_on_a_cone);
  if (test::failures == 0) {
    std::puts("test_emboss: all checks passed");
  }
  return test::failures == 0 ? 0 : 1;
}
