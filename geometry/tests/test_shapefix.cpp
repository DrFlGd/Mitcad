// SPDX-License-Identifier: MIT
// OCCT's repair of intersecting edges and wires (ShapeFix) on faces with
// many holes, with the port's patch 0060 (mitcad#60): the repair of a
// wire's intersecting edges applied the context to the whole face and went
// through all its vertices once per wire, and the repair of intersecting
// wires listed the edges of the second wire of every pair of wires. The
// results are the ones OCCT gave before the patch (the expected values were
// taken from OCCT 8.0.1 without it), on synthetic faces with OCCT alone.
// The last test is the port's patch 0942 (mitcad#495): without it, the
// repair of a face's intersecting wires never returns there.

#include <algorithm>
#include <cstdio>
#include <string>
#include <vector>

#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Circle.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <ShapeExtend.hxx>
#include <ShapeFix_IntersectionTool.hxx>
#include <ShapeFix_Shape.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <NCollection_IndexedMap.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Circ.hxx>
#include <gp_Ax22d.hxx>
#include <gp_Ax3.hxx>
#include <gp_Pln.hxx>

#include "check.hpp"

namespace test {
namespace {

using ShapeSet = NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher>;

// A check of a count that prints both values when it fails.
void expect_count(const char* what, int actual, int expected) {
  if (actual != expected) {
    std::fprintf(stderr, "shapefix test: %s: %d, expected %d\n", what, actual, expected);
    ++failures;
  }
}

// A check of a measure (relative 1e-9) that prints both values when it fails.
void expect_value(const char* what, double actual, double expected) {
  if (!near(actual, expected, 1e-9)) {
    std::fprintf(stderr, "shapefix test: %s: %.17g, expected %.17g\n", what, actual, expected);
    ++failures;
  }
}

int count(const TopoDS_Shape& shape, TopAbs_ShapeEnum type) {
  ShapeSet set;
  TopExp::MapShapes(shape, type, set);
  return set.Extent();
}

TopoDS_Wire circle_wire(double x, double y, double radius) {
  const gp_Circ circle(gp_Ax2(gp_Pnt(x, y, 0), gp::DZ()), radius);
  return BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(circle).Edge()).Wire();
}

TopoDS_Wire polygon_wire(double x, double y, const std::vector<gp_Pnt2d>& points) {
  BRepBuilderAPI_MakePolygon polygon;
  for (const gp_Pnt2d& p : points) {
    polygon.Add(gp_Pnt(x + p.X(), y + p.Y(), 0));
  }
  polygon.Close();
  return polygon.Wire();
}

// A plate in the XY plane with rows x rows round holes (radius 1 on a pitch
// of 4) and, above them, holes whose wires need the repairs: a quadrangle
// whose opposite edges cross in their middles, one whose opposite edges
// cross 0.005 from a vertex, and two circles that overlap. With
// `wide_vertex`, the vertex of the first round hole has a tolerance of
// 0.02, so that the second crossing lies within the largest tolerance of
// the face's vertices (which the repair of each wire looks up).
TopoDS_Face perforated_plate(int rows, bool wide_vertex) {
  const double top = 4.0 * rows;
  const TopoDS_Wire outer =
      polygon_wire(0, 0, {{-1, -1}, {top + 1, -1}, {top + 1, top + 10}, {-1, top + 10}});
  BRepBuilderAPI_MakeFace maker(gp_Pln(), outer);
  for (int i = 0; i < rows; ++i) {
    for (int j = 0; j < rows; ++j) {
      const TopoDS_Wire hole = circle_wire(2.0 + 4.0 * i, 2.0 + 4.0 * j, 1.0);
      if (wide_vertex && i == 0 && j == 0) {
        TopoDS_Iterator edge(hole);
        BRep_Builder().UpdateVertex(TopExp::FirstVertex(TopoDS::Edge(edge.Value())), 0.02);
      }
      maker.Add(TopoDS::Wire(hole.Reversed()));
    }
  }
  maker.Add(polygon_wire(2, top + 3, {{0, 0}, {2, 2}, {2, 0}, {0, 2}}));
  maker.Add(polygon_wire(6, top + 4, {{0, 0}, {1, 0}, {0.995, 0.5}, {0.995, -0.5}}));
  maker.Add(TopoDS::Wire(circle_wire(12, top + 4, 1.0).Reversed()));
  maker.Add(TopoDS::Wire(circle_wire(13, top + 4, 1.0).Reversed()));
  return maker.Face();
}

struct Healed {
  int faces = 0;
  int wires = 0;
  int edges = 0;
  int vertices = 0;
  double area = 0;
  double vertex_tolerance = 0;
  double edge_tolerance = 0;
  bool valid = false;
  int status = 0; // ShapeExtend_DONE1 .. DONE8 as bits 0 .. 7
};

// ShapeFix_Shape's healing of the face, as the import heals its bodies.
Healed heal(const TopoDS_Face& face) {
  Handle(ShapeFix_Shape) fix = new ShapeFix_Shape(face);
  fix->SetPrecision(1e-7);
  fix->SetMaxTolerance(1.0);
  fix->Perform();
  const TopoDS_Shape shape = fix->Shape();
  Healed healed;
  healed.faces = count(shape, TopAbs_FACE);
  healed.wires = count(shape, TopAbs_WIRE);
  healed.edges = count(shape, TopAbs_EDGE);
  healed.vertices = count(shape, TopAbs_VERTEX);
  GProp_GProps props;
  BRepGProp::SurfaceProperties(shape, props);
  healed.area = props.Mass();
  for (TopExp_Explorer v(shape, TopAbs_VERTEX); v.More(); v.Next()) {
    healed.vertex_tolerance = std::max(healed.vertex_tolerance, BRep_Tool::Tolerance(TopoDS::Vertex(v.Current())));
  }
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More(); e.Next()) {
    healed.edge_tolerance = std::max(healed.edge_tolerance, BRep_Tool::Tolerance(TopoDS::Edge(e.Current())));
  }
  healed.valid = BRepCheck_Analyzer(shape).IsValid();
  const ShapeExtend_Status done[] = {ShapeExtend_DONE1, ShapeExtend_DONE2, ShapeExtend_DONE3,
                                     ShapeExtend_DONE4, ShapeExtend_DONE5, ShapeExtend_DONE6,
                                     ShapeExtend_DONE7, ShapeExtend_DONE8};
  for (int i = 0; i < 8; ++i) {
    if (fix->Status(done[i])) {
      healed.status |= 1 << i;
    }
  }
  return healed;
}

void expect_healed(const char* name, const Healed& h, const Healed& expected) {
  const std::string prefix = name;
  expect_count((prefix + " faces").c_str(), h.faces, expected.faces);
  expect_count((prefix + " wires").c_str(), h.wires, expected.wires);
  expect_count((prefix + " edges").c_str(), h.edges, expected.edges);
  expect_count((prefix + " vertices").c_str(), h.vertices, expected.vertices);
  expect_value((prefix + " area").c_str(), h.area, expected.area);
  expect_value((prefix + " vertex tolerance").c_str(), h.vertex_tolerance, expected.vertex_tolerance);
  expect_value((prefix + " edge tolerance").c_str(), h.edge_tolerance, expected.edge_tolerance);
  expect_count((prefix + " valid").c_str(), h.valid ? 1 : 0, expected.valid ? 1 : 0);
  expect_count((prefix + " status").c_str(), h.status, expected.status);
}

// The crossing near a vertex is cut back to the vertex where it lies within
// the largest tolerance of the face's vertices, here that of another hole's
// vertex; the other wires are repaired as before.
void test_holes_repaired_as_before() {
  // OCCT leaves both faces invalid (pieces of the crossed wires stay), as
  // before the patch; the wide vertex saves one split.
  Healed wide;
  wide.faces = 1;
  wide.wires = 407;
  wide.edges = 421;
  wide.vertices = 417;
  wide.area = 6196.8297532567722;
  wide.vertex_tolerance = 0.02;
  wide.edge_tolerance = 1e-7;
  wide.valid = false;
  wide.status = 1 << 2;
  expect_healed("wide vertex", heal(perforated_plate(20, true)), wide);
  Healed narrow = wide;
  narrow.edges = 422;
  narrow.vertices = 418;
  narrow.vertex_tolerance = 1e-7;
  expect_healed("narrow vertices", heal(perforated_plate(20, false)), narrow);
}

// ApplyAgain gives Apply's result and status, also after requests and after
// a call that recorded the rebuilt face.
void test_apply_again_as_apply() {
  const TopoDS_Face face = perforated_plate(3, false);
  Handle(ShapeBuild_ReShape) again = new ShapeBuild_ReShape;
  Handle(ShapeBuild_ReShape) plain = new ShapeBuild_ReShape;
  const auto same_status = [&]() {
    for (const ShapeExtend_Status s : {ShapeExtend_OK, ShapeExtend_DONE1, ShapeExtend_DONE2, ShapeExtend_DONE3,
                                       ShapeExtend_DONE4, ShapeExtend_FAIL1}) {
      if (again->Status(s) != plain->Status(s)) {
        return false;
      }
    }
    return true;
  };
  // Nothing requested: the face itself.
  CHECK(again->ApplyAgain(face).IsEqual(face));
  CHECK(again->ApplyAgain(face).IsEqual(plain->Apply(face)));
  CHECK(same_status());

  // A vertex of the first hole replaced: the face is rebuilt and recorded.
  const TopoDS_Vertex vertex = TopoDS::Vertex(TopExp_Explorer(face, TopAbs_VERTEX).Current());
  const TopoDS_Vertex copy = TopoDS::Vertex(vertex.EmptyCopied());
  again->Replace(vertex, copy);
  plain->Replace(vertex, copy);
  const TopoDS_Shape rebuilt = again->ApplyAgain(face);
  const TopoDS_Shape rebuilt_plain = plain->Apply(face);
  CHECK(!rebuilt.IsSame(face));
  CHECK(again->Status(ShapeExtend_DONE3));
  CHECK(same_status());
  ShapeSet vertices;
  TopExp::MapShapes(rebuilt, TopAbs_VERTEX, vertices);
  CHECK(vertices.Contains(copy) && !vertices.Contains(vertex));
  CHECK(count(rebuilt, TopAbs_WIRE) == count(rebuilt_plain, TopAbs_WIRE));
  CHECK(count(rebuilt, TopAbs_EDGE) == count(rebuilt_plain, TopAbs_EDGE));

  // Applied again: the recorded face, given back without exploring it once
  // the call before recorded nothing, also after another shape's Apply.
  for (int i = 0; i < 3; ++i) {
    CHECK(again->ApplyAgain(face).IsEqual(rebuilt));
    CHECK(plain->Apply(face).IsEqual(rebuilt_plain));
    CHECK(again->Status(ShapeExtend_DONE1));
    CHECK(same_status());
    again->Apply(copy);
    plain->Apply(copy);
  }

  // Cleared: the face itself again.
  again->Clear();
  plain->Clear();
  CHECK(again->ApplyAgain(face).IsEqual(face));
  CHECK(plain->Apply(face).IsEqual(face));
  CHECK(same_status());
}

// Port patch 0942 (mitcad#495): a planar face of two wires, each an arc of
// one circle (radius 7.5) from `a` to `b` and from `b` to `c` radians closed
// by a chord, the arcs meeting end to end at a vertex the wires share; the
// arcs' edges are not same parameter (as an invalid chamfer's result left a
// boss's bottom face), their p-curves trimmed circles. The intersection of
// the two arcs is a segment of no length at the common vertex; the repair
// of intersecting wires took both edges as cut there, which a trimmed
// circle never is, and intersected them again for ever. It returns now,
// with nothing to repair.
void test_arcs_meeting_in_two_wires(double a, double b, double c) {
  const double r = 7.5;
  Handle(Geom_Plane) plane = new Geom_Plane(gp_Ax3(gp_Pnt(0, 0, 0), gp::DZ(), gp::DX()));
  Handle(Geom2d_Circle) circle2d = new Geom2d_Circle(gp_Ax22d(gp_Pnt2d(0, 0), gp_Dir2d(1, 0)), r);
  Handle(Geom_Circle) circle = new Geom_Circle(gp_Ax2(gp_Pnt(0, 0, 0), gp::DZ(), gp::DX()), r);
  BRep_Builder builder;
  TopoDS_Face face;
  builder.MakeFace(face, plane, 1e-7);
  const auto vertex_at = [&](double t) {
    TopoDS_Vertex v;
    builder.MakeVertex(v, circle->Value(t), 1e-7);
    return v;
  };
  const auto bounded = [&](TopoDS_Edge& e, const TopoDS_Vertex& v1, const TopoDS_Vertex& v2) {
    TopoDS_Vertex first = v1, last = v2;
    first.Orientation(TopAbs_FORWARD);
    last.Orientation(TopAbs_REVERSED);
    builder.Add(e, first);
    builder.Add(e, last);
  };
  const auto arc = [&](double t1, double t2, const TopoDS_Vertex& v1, const TopoDS_Vertex& v2) {
    TopoDS_Edge e;
    builder.MakeEdge(e, circle, 1e-7);
    bounded(e, v1, v2);
    builder.UpdateEdge(e, new Geom2d_TrimmedCurve(circle2d, t1, t2), face, 1e-7);
    builder.Range(e, t1, t2);
    builder.SameParameter(e, false);
    builder.SameRange(e, false);
    return e;
  };
  const auto chord = [&](const TopoDS_Vertex& v1, const TopoDS_Vertex& v2) {
    const gp_Pnt p1 = BRep_Tool::Pnt(v1), p2 = BRep_Tool::Pnt(v2);
    TopoDS_Edge e;
    builder.MakeEdge(e, new Geom_Line(p1, gp_Dir(gp_Vec(p1, p2))), 1e-7);
    bounded(e, v1, v2);
    builder.Range(e, 0.0, p1.Distance(p2));
    builder.UpdateEdge(e, new Geom2d_Line(gp_Pnt2d(p1.X(), p1.Y()), gp_Dir2d(p2.X() - p1.X(), p2.Y() - p1.Y())), face,
                       1e-7);
    return e;
  };
  const TopoDS_Vertex va = vertex_at(a), vb = vertex_at(b), vc = vertex_at(c);
  TopoDS_Wire first, second;
  builder.MakeWire(first);
  builder.Add(first, arc(a, b, va, vb));
  builder.Add(first, chord(vb, va));
  builder.MakeWire(second);
  builder.Add(second, arc(b, c, vb, vc));
  builder.Add(second, chord(vc, vb));
  builder.Add(face, first);
  builder.Add(face, second);
  ShapeFix_IntersectionTool tool(new ShapeBuild_ReShape, 1e-7, 1e-7);
  CHECK(!tool.FixIntersectingWires(face));
  expect_count("wires", count(face, TopAbs_WIRE), 2);
  expect_count("edges", count(face, TopAbs_EDGE), 4);
}

} // namespace

void shapefix_tests() {
  guarded("holes repaired as before", test_holes_repaired_as_before);
  guarded("apply again as apply", test_apply_again_as_apply);
  guarded("arcs meeting in two wires", [] {
    test_arcs_meeting_in_two_wires(0.5, 1.0, 1.5);
    test_arcs_meeting_in_two_wires(0.0, 1.0, 2.0);
    test_arcs_meeting_in_two_wires(1.0, 2.0, 3.0);
  });
}

} // namespace test
