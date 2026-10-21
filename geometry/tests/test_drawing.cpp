// SPDX-License-Identifier: MIT
// Drawings (mitcad#471): views of a block with a counterbored through hole
// with hidden lines removed, its section, and the round holes a drawing
// tabulates. The block is 40 x 30 x 20 from the origin; the hole of
// diameter 6.6 at (20, 15) runs along Z, counterbored 11 wide and 6.8 deep
// from the top.

#include <cmath>
#include <cstdio>
#include <memory>
#include <vector>

#include <BRepAlgoAPI_Cut.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <gp_Ax2.hxx>

#include "check.hpp"
#include "mitcad/geometry/drawing.hpp"

namespace {

using namespace mitcad::geometry;

TopoDS_Shape cylinder(double x, double y, double z0, double z1, double radius) {
  return BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(x, y, z0), gp_Dir(0.0, 0.0, 1.0)), radius, z1 - z0).Shape();
}

ShapePtr counterbored_block() {
  TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(0.0, 0.0, 0.0), 40.0, 30.0, 20.0).Shape();
  block = BRepAlgoAPI_Cut(block, cylinder(20.0, 15.0, -1.0, 21.0, 3.3)).Shape();
  block = BRepAlgoAPI_Cut(block, cylinder(20.0, 15.0, 13.2, 21.0, 5.5)).Shape();
  return std::make_shared<Shape>(block);
}

ShapePtr blind_hole_block() {
  TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(0.0, 0.0, 0.0), 40.0, 30.0, 20.0).Shape();
  block = BRepAlgoAPI_Cut(block, cylinder(10.0, 10.0, 12.0, 21.0, 2.5)).Shape();
  return std::make_shared<Shape>(block);
}

DrawingViewSpec view(const gp_Dir& toward_eye, const gp_Dir& x_axis, bool hidden) {
  DrawingViewSpec spec;
  spec.toward_eye = toward_eye;
  spec.x_axis = x_axis;
  spec.hidden = hidden;
  return spec;
}

bool near_value(double a, double b) { return std::abs(a - b) <= 1e-6; }

// Every visible line lies on the sides of the rectangle [0, w] x [0, h],
// and together they cover each side.
void check_outline(const DrawnView& drawn, double w, double h, const char* name) {
  double covered[4] = {0.0, 0.0, 0.0, 0.0}; // bottom, right, top, left
  int lines = 0;
  for (const DrawnCurve& c : drawn.curves) {
    if (!c.visible || c.type != DrawnCurve::Type::Line) {
      continue;
    }
    ++lines;
    const Point2 a = c.points[0];
    const Point2 b = c.points[1];
    const double length = std::hypot(b[0] - a[0], b[1] - a[1]);
    if (near_value(a[1], 0.0) && near_value(b[1], 0.0)) {
      covered[0] += length;
    } else if (near_value(a[0], w) && near_value(b[0], w)) {
      covered[1] += length;
    } else if (near_value(a[1], h) && near_value(b[1], h)) {
      covered[2] += length;
    } else if (near_value(a[0], 0.0) && near_value(b[0], 0.0)) {
      covered[3] += length;
    } else {
      std::fprintf(stderr, "%s: a visible line off the outline: (%.9g, %.9g) - (%.9g, %.9g)\n", name, a[0], a[1],
                   b[0], b[1]);
      ++test::failures;
    }
  }
  CHECK(lines >= 4);
  // Overlapping pieces (the counterbore's rim on the top edge) may add.
  CHECK(covered[0] >= w - 1e-6);
  CHECK(covered[1] >= h - 1e-6);
  CHECK(covered[2] >= w - 1e-6);
  CHECK(covered[3] >= h - 1e-6);
}

bool has_circle(const DrawnView& drawn, double cx, double cy, double radius, bool visible) {
  for (const DrawnCurve& c : drawn.curves) {
    if (c.type == DrawnCurve::Type::Arc && c.visible == visible && near_value(c.center[0], cx) &&
        near_value(c.center[1], cy) && near_value(c.radius, radius) && c.end - c.start > 2.0 * test::kPi - 1e-9) {
      return true;
    }
  }
  return false;
}

// A line along x = `x` from y0 to y1, hidden or seen.
bool has_line(const DrawnView& drawn, double x, double y0, double y1, bool visible) {
  for (const DrawnCurve& c : drawn.curves) {
    if (c.visible != visible || c.type != DrawnCurve::Type::Line) {
      continue;
    }
    const Point2 a = c.points[0];
    const Point2 b = c.points[1];
    if (near_value(a[0], x) && near_value(b[0], x) && near_value(std::min(a[1], b[1]), y0) &&
        near_value(std::max(a[1], b[1]), y1)) {
      return true;
    }
  }
  return false;
}

int hidden_count(const DrawnView& drawn) {
  int n = 0;
  for (const DrawnCurve& c : drawn.curves) {
    n += c.visible ? 0 : 1;
  }
  return n;
}

void test_three_views() {
  const ShapePtr block = counterbored_block();
  // From above: the outline and both circles of the hole, seen.
  const DrawnView top = project_view({block}, view(gp_Dir(0, 0, 1), gp_Dir(1, 0, 0), false));
  CHECK(!top.polygonal);
  check_outline(top, 40.0, 30.0, "top");
  CHECK(has_circle(top, 20.0, 15.0, 5.5, true));
  CHECK(has_circle(top, 20.0, 15.0, 3.3, true));
  CHECK(hidden_count(top) == 0);

  // From the front (-Y, x along X, up along Z): the outline; the hole's
  // walls hidden, drawn only when asked.
  const DrawnView front = project_view({block}, view(gp_Dir(0, -1, 0), gp_Dir(1, 0, 0), false));
  check_outline(front, 40.0, 20.0, "front");
  CHECK(hidden_count(front) == 0);
  const DrawnView dashed = project_view({block}, view(gp_Dir(0, -1, 0), gp_Dir(1, 0, 0), true));
  check_outline(dashed, 40.0, 20.0, "front with hidden lines");
  CHECK(has_line(dashed, 16.7, 0.0, 13.2, false));
  CHECK(has_line(dashed, 23.3, 0.0, 13.2, false));
  CHECK(has_line(dashed, 14.5, 13.2, 20.0, false));
  CHECK(has_line(dashed, 25.5, 13.2, 20.0, false));

  // From the right (+X, x along Y, up along Z).
  const DrawnView right = project_view({block}, view(gp_Dir(1, 0, 0), gp_Dir(0, 1, 0), false));
  check_outline(right, 30.0, 20.0, "right");
  CHECK(hidden_count(right) == 0);

  // The polygonal algorithm: the same outline within its deflection.
  DrawingViewSpec polygonal = view(gp_Dir(0, -1, 0), gp_Dir(1, 0, 0), false);
  polygonal.exact = false;
  const DrawnView poly = project_view({block}, polygonal);
  CHECK(poly.polygonal);
  double xmin = 1e9, xmax = -1e9, ymin = 1e9, ymax = -1e9;
  for (const DrawnCurve& c : poly.curves) {
    for (const Point2& p : c.points) {
      xmin = std::min(xmin, p[0]);
      xmax = std::max(xmax, p[0]);
      ymin = std::min(ymin, p[1]);
      ymax = std::max(ymax, p[1]);
    }
  }
  CHECK(std::abs(xmin) < 1e-3 && std::abs(xmax - 40.0) < 1e-3);
  CHECK(std::abs(ymin) < 1e-3 && std::abs(ymax - 20.0) < 1e-3);
}

double area(const std::vector<Point2>& loop) {
  double a = 0.0;
  for (std::size_t i = 0; i < loop.size(); ++i) {
    const Point2& p = loop[i];
    const Point2& q = loop[(i + 1) % loop.size()];
    a += p[0] * q[1] - q[0] * p[1];
  }
  return a / 2.0;
}

void test_section_view() {
  // Through the hole's axis, looking from the front at what is behind.
  DrawingViewSpec spec = view(gp_Dir(0, -1, 0), gp_Dir(1, 0, 0), false);
  spec.section = DrawingSection{gp_Pnt(20.0, 15.0, 0.0), gp_Dir(0, -1, 0)};
  const DrawnView cut = project_view({counterbored_block()}, spec);
  CHECK(cut.cuts.size() == 2);
  double total = 0.0;
  for (const CutFace& face : cut.cuts) {
    double face_area = 0.0;
    for (const std::vector<Point2>& loop : face) {
      face_area += area(loop);
    }
    total += std::abs(face_area);
  }
  CHECK_NEAR(total, 40.0 * 20.0 - 6.6 * 13.2 - 11.0 * 6.8);
  // The hole's walls in the cut, seen.
  CHECK(has_line(cut, 16.7, 0.0, 13.2, true));
  CHECK(has_line(cut, 25.5, 13.2, 20.0, true));
}

void test_round_holes() {
  const std::vector<RoundHole> holes = round_holes(*counterbored_block());
  CHECK(holes.size() == 1);
  if (holes.size() == 1) {
    const RoundHole& h = holes.front();
    CHECK_NEAR(h.diameter, 6.6);
    CHECK(h.through);
    CHECK_NEAR(h.cbore_diameter, 11.0);
    CHECK_NEAR(h.cbore_depth, 6.8);
    CHECK(h.csink_diameter == 0.0);
    CHECK(h.opening.Distance(gp_Pnt(20.0, 15.0, 20.0)) < 1e-6);
    CHECK(h.direction.IsEqual(gp_Dir(0, 0, -1), 1e-9));
    CHECK_NEAR(h.depth, 20.0);
  }
  const std::vector<RoundHole> blind = round_holes(*blind_hole_block());
  CHECK(blind.size() == 1);
  if (blind.size() == 1) {
    const RoundHole& h = blind.front();
    CHECK_NEAR(h.diameter, 5.0);
    CHECK(!h.through);
    CHECK_NEAR(h.depth, 8.0);
    CHECK(h.opening.Distance(gp_Pnt(10.0, 10.0, 20.0)) < 1e-6);
    CHECK(h.direction.IsEqual(gp_Dir(0, 0, -1), 1e-9));
    CHECK(h.cbore_diameter == 0.0);
  }
  // A boss is no hole.
  TopoDS_Shape boss = BRepPrimAPI_MakeBox(gp_Pnt(0.0, 0.0, 0.0), 10.0, 10.0, 10.0).Shape();
  CHECK(round_holes(Shape(cylinder(0.0, 0.0, 0.0, 10.0, 3.0))).empty());
  CHECK(round_holes(Shape(boss)).empty());
}

} // namespace

int main() {
  test::guarded("three views", test_three_views);
  test::guarded("a section view", test_section_view);
  test::guarded("round holes", test_round_holes);
  if (test::failures == 0) {
    std::puts("test_drawing: all checks passed");
  }
  return test::failures == 0 ? 0 : 1;
}
