// SPDX-License-Identifier: MIT
// Operations on bodies of thousands of faces (mitcad#121): a boolean whose
// targets OCCT's checker found valid checks only what it changed
// (util.hpp, is_valid_where_changed) and gives the checker's answer on the
// whole shape; a shape keeps its faces' kinds and areas and its edges'
// middles and curves (mitcad#119); the optimal box
// is its faces' boxes, found on all cores; and OCCT's face merge keeps a
// chain of B-spline edges it cannot concatenate (the port's patch 0110);
// the checker's statuses on faces of many wires and edges stay OCCT's
// (patches 0120 and 0121, mitcad#119).

#include <algorithm>
#include <exception>
#include <memory>
#include <string>
#include <vector>

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepCheck_Face.hxx>
#include <BRepCheck_Shell.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepTools.hxx>
#include <BRepTopAdaptor_FClass2d.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Line.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_List.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <Standard_Failure.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Dir2d.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt2d.hxx>

#include "../src/util.hpp"
#include "check.hpp"
#include "mitcad/geometry/datum.hpp"
#include "mitcad/geometry/query.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

int count(const TopoDS_Shape& shape, TopAbs_ShapeEnum type) {
  int n = 0;
  for (TopExp_Explorer it(shape, type); it.More(); it.Next()) {
    ++n;
  }
  return n;
}

// OCCT's fuse of two shapes, leaving them as they are.
TopoDS_Shape fused(const TopoDS_Shape& a, const TopoDS_Shape& b) {
  BRepAlgoAPI_Fuse fuse;
  NCollection_List<TopoDS_Shape> arguments;
  arguments.Append(a);
  NCollection_List<TopoDS_Shape> tools;
  tools.Append(b);
  fuse.SetArguments(arguments);
  fuse.SetTools(tools);
  fuse.SetNonDestructive(true);
  fuse.Build();
  CHECK(fuse.IsDone() && !fuse.HasErrors());
  return fuse.Shape();
}

// A plate of 100 x 100 x 5 with 8 x 8 holes of radius 2, cut by a boolean:
// its result is checked, so it is known valid.
ShapePtr perforated_plate() {
  const ShapePtr plate = extrude("F2", {rectangle(1, 0, 0, 100, 100)}, 0, 5);
  std::vector<Region> circles;
  for (int i = 0; i < 8; ++i) {
    for (int j = 0; j < 8; ++j) {
      circles.push_back(circle(10 + 8 * i + j, 10.0 + 11.0 * i, 10.0 + 11.0 * j, 2.0));
    }
  }
  const ShapePtr holes = extrude("F3", circles, -1, 6);
  const BooleanResult cut = boolean(BooleanOp::Cut, {plate.get()}, *holes);
  CHECK(cut.pieces.size() == 1);
  return cut.pieces.at(0).shape;
}

void test_a_checked_result_is_known_valid() {
  const ShapePtr plate = perforated_plate();
  CHECK(plate->checked_valid().value_or(false));
  CHECK(plate->face_count() == 6 + 64);
  // A boss joined at a corner: checked where it changed the plate.
  const ShapePtr boss = extrude("F4", {circle(90, 3, 3, 2)}, 4, 10);
  const BooleanResult joined = boolean(BooleanOp::Join, {plate.get()}, *boss);
  CHECK(joined.pieces.size() == 1);
  CHECK(joined.pieces.at(0).shape->checked_valid().value_or(false));
  CHECK(detail::is_valid(joined.pieces.at(0).shape->occt()));
}

void test_the_check_where_changed_gives_the_whole_checks_answer() {
  const ShapePtr plate = perforated_plate();
  ShapeMap kept;
  for (TopExp_Explorer it(plate->occt(), TopAbs_FACE); it.More(); it.Next()) {
    kept.Add(it.Current());
  }
  // A cylinder fused on top near a corner, without a copy of the plate: the
  // result keeps most of the plate's faces as they are.
  const TopoDS_Shape boss = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(3, 3, 4), gp::DZ()), 2.0, 6.0).Shape();
  const TopoDS_Shape result = fused(plate->occt(), boss);
  int shared = 0;
  for (TopExp_Explorer it(result, TopAbs_FACE); it.More(); it.Next()) {
    shared += kept.Contains(it.Current()) ? 1 : 0;
  }
  CHECK(shared > count(result, TopAbs_FACE) / 2);
  CHECK(BRepCheck_Analyzer(result).IsValid());
  const detail::ChecksWhereChanged before = detail::checks_where_changed();
  CHECK(detail::is_valid_where_changed(result, kept));
  const detail::ChecksWhereChanged after = detail::checks_where_changed();
  // The checker saw the faces the boss changed and those meeting them (the
  // top's holes too), not the bottom.
  CHECK(after.checks == before.checks + 1);
  CHECK(after.faces - before.faces >= 3);
  CHECK(static_cast<int>(after.faces - before.faces) < count(result, TopAbs_FACE));
  bool threw = false;
  try {
    detail::require_valid_where_changed(result, kept, "the test");
  } catch (const std::exception&) {
    threw = true;
  }
  CHECK(!threw);

  // The cylinder's top edge (of faces the plate does not have) loses its
  // same-parameter flag: the check where it changed finds it, as the
  // whole check does.
  TopoDS_Edge top;
  for (TopExp_Explorer it(result, TopAbs_EDGE); it.More() && top.IsNull(); it.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(it.Current());
    bool high = !BRep_Tool::Degenerated(edge);
    for (TopExp_Explorer v(edge, TopAbs_VERTEX); v.More(); v.Next()) {
      high = high && BRep_Tool::Pnt(TopoDS::Vertex(v.Current())).Z() > 9.99;
    }
    if (high) {
      top = edge;
    }
  }
  CHECK(!top.IsNull());
  if (!top.IsNull()) {
    BRep_Builder().SameParameter(top, false);
    CHECK(!BRepCheck_Analyzer(result).IsValid());
    CHECK(!detail::is_valid_where_changed(result, kept));
    CHECK(throws_with([&] { detail::require_valid_where_changed(result, kept, "the test"); },
                      "the test produced an invalid shape"));
  }

  // A shell with a face taken out is open: the whole check decides.
  BRep_Builder builder;
  TopoDS_Shell open;
  builder.MakeShell(open);
  bool skipped = false;
  for (TopExp_Explorer it(plate->occt(), TopAbs_FACE); it.More(); it.Next()) {
    if (!skipped) {
      skipped = true;
      continue;
    }
    builder.Add(open, it.Current());
  }
  TopoDS_Solid solid;
  builder.MakeSolid(solid);
  builder.Add(solid, open);
  CHECK(!detail::is_valid_where_changed(solid, kept));
}

void test_faces_measured_once() {
  const ShapePtr plate = perforated_plate();
  CHECK(plate->face_measures() == nullptr);
  const std::vector<FaceInfo> first = face_infos(*plate);
  CHECK(plate->face_measures() != nullptr);
  const std::vector<FaceInfo> again = face_infos(*plate);
  CHECK(first.size() == again.size());
  double area = 0.0;
  for (std::size_t i = 0; i < first.size(); ++i) {
    CHECK(first[i].surface == again[i].surface && first[i].area == again[i].area);
    CHECK(first[i].names == again[i].names);
    area += first[i].area;
  }
  // 2 x (100 x 100 - 64 holes) + 4 sides of 100 x 5 + 64 walls of 2 pi 2 x 5.
  CHECK_NEAR(area, 2.0 * (10000.0 - 64.0 * kPi * 4.0) + 2000.0 + 64.0 * 2.0 * kPi * 2.0 * 5.0);
}

// The edges' middles and curves are worked out once per shape (mitcad#119)
// and the kept ones are the same as those worked out.
void test_edges_measured_once() {
  const ShapePtr plate = perforated_plate();
  CHECK(plate->kept_edge_middles() == nullptr && plate->kept_edge_curves() == nullptr);
  const std::vector<EdgeMiddle> middles = edge_middles(*plate);
  const std::vector<CurveDescription> curves = edge_geometries(*plate);
  CHECK(plate->kept_edge_middles() != nullptr && plate->kept_edge_curves() != nullptr);
  CHECK(static_cast<int>(curves.size()) == plate->edge_count());
  CHECK(!middles.empty());
  const std::vector<EdgeMiddle> middles_again = edge_middles(*plate);
  const std::vector<CurveDescription> curves_again = edge_geometries(*plate);
  CHECK(middles.size() == middles_again.size() && curves.size() == curves_again.size());
  for (std::size_t i = 0; i < middles.size() && i < middles_again.size(); ++i) {
    CHECK(middles[i].name == middles_again[i].name && middles[i].length == middles_again[i].length);
    CHECK(middles[i].point.Distance(middles_again[i].point) == 0.0);
  }
  for (std::size_t i = 0; i < curves.size() && i < curves_again.size(); ++i) {
    CHECK(curves[i].type == curves_again[i].type && curves[i].radius == curves_again[i].radius);
    CHECK(curves[i].start.Distance(curves_again[i].start) == 0.0);
  }
  // A copy of the shape keeps them; another shape works out its own.
  const Shape copy = *plate;
  CHECK(copy.kept_edge_middles() == plate->kept_edge_middles());
  const ShapePtr other = perforated_plate();
  CHECK(other->kept_edge_middles() == nullptr);
}

// Each face's box holds the face (its vertices and the points of a grid over
// it), and is kept by the shape (mitcad#119).
void test_face_boxes_hold_their_faces() {
  const ShapePtr plate = perforated_plate();
  const TopoDS_Shape ball = BRepPrimAPI_MakeSphere(gp_Pnt(50, 50, 5), 7.0).Shape();
  const auto shape = std::make_shared<Shape>(fused(plate->occt(), ball));
  CHECK(shape->kept_face_boxes() == nullptr);
  const std::vector<std::array<double, 6>> boxes = face_boxes(*shape);
  CHECK(shape->kept_face_boxes() != nullptr);
  CHECK(static_cast<int>(boxes.size()) == shape->face_count());
  const auto inside = [](const std::array<double, 6>& box, const gp_Pnt& p) {
    return p.X() >= box[0] && p.Y() >= box[1] && p.Z() >= box[2] && p.X() <= box[3] && p.Y() <= box[4] &&
           p.Z() <= box[5];
  };
  int bounded = 0;
  for (int i = 0; i < shape->face_count() && i < static_cast<int>(boxes.size()); ++i) {
    const std::array<double, 6>& box = boxes[static_cast<std::size_t>(i)];
    bounded += box[3] - box[0] < 1e6 ? 1 : 0;
    for (TopExp_Explorer it(shape->face(i), TopAbs_VERTEX); it.More(); it.Next()) {
      CHECK(inside(box, BRep_Tool::Pnt(TopoDS::Vertex(it.Current()))));
    }
    const TopoDS_Face& face = shape->face(i);
    double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    const BRepAdaptor_Surface surface(face);
    const BRepTopAdaptor_FClass2d classifier(face, 1e-9);
    for (int a = 0; a <= 12; ++a) {
      for (int b = 0; b <= 12; ++b) {
        const gp_Pnt2d uv(u0 + (u1 - u0) * a / 12.0, v0 + (v1 - v0) * b / 12.0);
        if (classifier.Perform(uv) != TopAbs_OUT) {
          CHECK(inside(box, surface.Value(uv.X(), uv.Y())));
        }
      }
    }
  }
  // Planes, cylinders and the sphere all have boxes of their own.
  CHECK(bounded == shape->face_count());
}

// A plane face of a 100 x 100 square with holes of radius 2 at the given
// centres, each hole two arcs, the second one's range ending a rounding
// error past 2 pi as edges of stored bodies do (the port's patch 0120).
TopoDS_Face face_with_holes(const std::vector<gp_Pnt>& centres) {
  BRepBuilderAPI_MakePolygon square;
  square.Add(gp_Pnt(0, 0, 0));
  square.Add(gp_Pnt(100, 0, 0));
  square.Add(gp_Pnt(100, 100, 0));
  square.Add(gp_Pnt(0, 100, 0));
  square.Close();
  BRepBuilderAPI_MakeFace face(gp_Pln(gp::XOY()), square.Wire());
  BRep_Builder builder;
  for (const gp_Pnt& centre : centres) {
    const occ::handle<Geom_Circle> circle = new Geom_Circle(gp_Ax2(centre, gp::DZ()), 2.0);
    const TopoDS_Edge first = BRepBuilderAPI_MakeEdge(circle, 0.0, kPi);
    TopoDS_Edge second = BRepBuilderAPI_MakeEdge(circle, kPi, 2.0 * kPi);
    builder.Range(second, kPi, 2.0 * kPi + 4e-15);
    BRepBuilderAPI_MakeWire wire(first, second);
    CHECK(wire.IsDone());
    face.Add(TopoDS::Wire(wire.Wire().Reversed()));
  }
  CHECK(face.IsDone());
  return face.Face();
}

// The check of how a face's wires lie inside one another gives OCCT's
// statuses with the boxes of arcs whose ranges pass their circle's
// parameters (patch 0120): holes inside the outline are fine, a hole outside
// it is not.
void test_wires_of_arcs_past_their_range_are_classified() {
  std::vector<gp_Pnt> inside;
  for (int i = 0; i < 9; ++i) {
    for (int j = 0; j < 9; ++j) {
      inside.emplace_back(10.0 + 10.0 * i, 10.0 + 10.0 * j, 0.0);
    }
  }
  const occ::handle<BRepCheck_Face> good = new BRepCheck_Face(face_with_holes(inside));
  CHECK(good->ClassifyWires() == BRepCheck_NoError);
  std::vector<gp_Pnt> outside = inside;
  outside.emplace_back(150.0, 50.0, 0.0);
  const occ::handle<BRepCheck_Face> bad = new BRepCheck_Face(face_with_holes(outside));
  CHECK(bad->ClassifyWires() == BRepCheck_InvalidImbricationOfWires);
}

// The shell's orientation check gives OCCT's statuses on a face of many
// edges, whose edges' orientations it lists once (patch 0121): the
// perforated plate's shell is well oriented; with a hole's wall or the top
// face turned over, it is not.
void test_the_orientation_of_a_shell_with_a_face_of_many_edges() {
  const ShapePtr plate = perforated_plate();
  TopoDS_Shape shell;
  for (TopExp_Explorer it(plate->occt(), TopAbs_SHELL); it.More(); it.Next()) {
    shell = it.Current();
  }
  CHECK(!shell.IsNull());
  int most = 0;
  for (TopExp_Explorer it(shell, TopAbs_FACE); it.More(); it.Next()) {
    most = std::max(most, count(it.Current(), TopAbs_EDGE));
  }
  CHECK(most >= 64);
  const occ::handle<BRepCheck_Shell> whole = new BRepCheck_Shell(TopoDS::Shell(shell));
  CHECK(whole->Orientation() == BRepCheck_NoError);
  // The same faces with one of them turned over: a hole's wall (few edges)
  // or the face of most edges.
  for (const bool large : {false, true}) {
    BRep_Builder builder;
    TopoDS_Shell turned;
    builder.MakeShell(turned);
    bool done = false;
    for (TopExp_Explorer it(shell, TopAbs_FACE); it.More(); it.Next()) {
      const int edges = count(it.Current(), TopAbs_EDGE);
      const bool turn = !done && (large ? edges == most : edges < 8);
      builder.Add(turned, turn ? it.Current().Reversed() : it.Current());
      done = done || turn;
    }
    CHECK(done);
    const occ::handle<BRepCheck_Shell> check = new BRepCheck_Shell(turned);
    CHECK(check->Orientation() == BRepCheck_BadOrientationOfSubshape);
  }
}

void test_optimal_box_is_occts() {
  // Curved faces, where the optimal box is a search over each surface.
  const ShapePtr plate = perforated_plate();
  const TopoDS_Shape ball = BRepPrimAPI_MakeSphere(gp_Pnt(50, 50, 5), 7.0).Shape();
  const auto shape = std::make_shared<Shape>(fused(plate->occt(), ball));
  Bnd_Box serial;
  BRepBndLib::AddOptimal(shape->occt(), serial, false, false);
  double x0 = 0, y0 = 0, z0 = 0, x1 = 0, y1 = 0, z1 = 0;
  serial.Get(x0, y0, z0, x1, y1, z1);
  const BoundingBox box = bounding_box(*shape);
  CHECK(!box.empty);
  CHECK(box.min.X() == x0 && box.min.Y() == y0 && box.min.Z() == z0);
  CHECK(box.max.X() == x1 && box.max.Y() == y1 && box.max.Z() == z1);
}

// A B-spline edge from a to b (degree 2, middle pole in between).
TopoDS_Edge spline(const gp_Pnt& a, const gp_Pnt& b, const TopoDS_Vertex& va, const TopoDS_Vertex& vb) {
  NCollection_Array1<gp_Pnt> poles(1, 3);
  poles(1) = a;
  poles(2) = gp_Pnt((a.XYZ() + b.XYZ()) / 2.0);
  poles(3) = b;
  NCollection_Array1<double> knots(1, 2);
  knots(1) = 0.0;
  knots(2) = 1.0;
  NCollection_Array1<int> mults(1, 2);
  mults(1) = 3;
  mults(2) = 3;
  const occ::handle<Geom_BSplineCurve> curve = new Geom_BSplineCurve(poles, knots, mults, 2);
  // Built as given: OCCT's edge maker would refuse a vertex off the curve.
  BRep_Builder builder;
  TopoDS_Edge edge;
  builder.MakeEdge(edge, curve, 1e-7);
  builder.Add(edge, va.Oriented(TopAbs_FORWARD));
  builder.Add(edge, vb.Oriented(TopAbs_REVERSED));
  builder.Range(edge, 0.0, 1.0);
  return edge;
}

void test_merge_keeps_a_chain_it_cannot_concatenate() {
  // A planar face whose bottom is two B-spline edges in a line. The second
  // starts 2e-6 off the end of the first; the vertex between them has a
  // tolerance of 1e-7. Concatenating them throws in OCCT; with the port's
  // patch 0110 the merge keeps them and finishes.
  BRep_Builder builder;
  const auto vertex = [&](double x, double y, double tolerance) {
    TopoDS_Vertex v;
    builder.MakeVertex(v, gp_Pnt(x, y, 0), tolerance);
    return v;
  };
  const TopoDS_Vertex v0 = vertex(0, 0, 1e-7);
  const TopoDS_Vertex v1 = vertex(1, 0, 1e-7);
  const TopoDS_Vertex v2 = vertex(2, 0, 1e-7);
  const TopoDS_Vertex v3 = vertex(2, 1, 1e-7);
  const TopoDS_Vertex v4 = vertex(0, 1, 1e-7);
  const TopoDS_Edge a = spline(gp_Pnt(0, 0, 0), gp_Pnt(1, 0, 0), v0, v1);
  const TopoDS_Edge b = spline(gp_Pnt(1, 2e-6, 0), gp_Pnt(2, 0, 0), v1, v2);
  const TopoDS_Edge c = BRepBuilderAPI_MakeEdge(v2, v3).Edge();
  const TopoDS_Edge d = BRepBuilderAPI_MakeEdge(v3, v4).Edge();
  const TopoDS_Edge e = BRepBuilderAPI_MakeEdge(v4, v0).Edge();
  TopoDS_Wire wire;
  builder.MakeWire(wire);
  for (const TopoDS_Edge& edge : {a, b, c, d, e}) {
    builder.Add(wire, edge);
  }
  const TopoDS_Face face = BRepBuilderAPI_MakeFace(gp_Pln(gp::XOY()), wire).Face();
  ShapeUpgrade_UnifySameDomain unify(face, true, false, true);
  bool failed = false;
  try {
    unify.Build();
  } catch (const Standard_Failure&) {
    failed = true;
  }
  CHECK(!failed);
  if (!failed) {
    CHECK(count(unify.Shape(), TopAbs_EDGE) == 5);
  }
}

void test_merge_keeps_lines_whose_pcurves_do_not_meet() {
  // A face of a cylinder (radius 10, u 0..1, v 0..2) whose left side is two
  // lines along the axis. Their p-curves on the cylinder do not meet: the
  // lower one runs 1e-6 to the side, more than the tolerance of the vertex
  // between them (1e-7). Merging the lines concatenates the p-curves, which
  // throws in OCCT; with the port's patch 0110 the lines stay.
  const double r = 10.0;
  const occ::handle<Geom_CylindricalSurface> cylinder = new Geom_CylindricalSurface(gp_Ax3(gp::XOY()), r);
  BRep_Builder builder;
  const auto vertex = [&](double u, double v) {
    TopoDS_Vertex vx;
    builder.MakeVertex(vx, cylinder->Value(u, v), 1e-7);
    return vx;
  };
  const TopoDS_Vertex v00 = vertex(0, 0);
  const TopoDS_Vertex v10 = vertex(1, 0);
  const TopoDS_Vertex v12 = vertex(1, 2);
  const TopoDS_Vertex v02 = vertex(0, 2);
  const TopoDS_Vertex v01 = vertex(0, 1);
  TopoDS_Face face;
  builder.MakeFace(face, cylinder, 1e-7);
  // An edge from `a` to `b` along a 3D curve over [0, length], with its
  // p-curve on the face: a 2D line from `start` along `along`.
  const auto edge = [&](const occ::handle<Geom_Curve>& curve, double length, const gp_Pnt2d& start,
                        const gp_Dir2d& along, const TopoDS_Vertex& a, const TopoDS_Vertex& b) {
    TopoDS_Edge e;
    builder.MakeEdge(e, curve, 1e-7);
    builder.UpdateEdge(e, new Geom2d_Line(start, along), face, 1e-7);
    builder.Add(e, a.Oriented(TopAbs_FORWARD));
    builder.Add(e, b.Oriented(TopAbs_REVERSED));
    builder.Range(e, 0.0, length);
    return e;
  };
  const auto line = [&](double u, double v) {
    return occ::handle<Geom_Curve>(new Geom_Line(cylinder->Value(u, v), gp::DZ()));
  };
  const auto arc = [&](double v) {
    return occ::handle<Geom_Curve>(new Geom_Circle(gp_Ax2(gp_Pnt(0, 0, v), gp::DZ(), gp::DX()), r));
  };
  // (Circle parameters: u is the angle; a 2D line of direction (1, 0)
  // runs at the same parameter.)
  const TopoDS_Edge bottom = edge(arc(0), 1.0, gp_Pnt2d(0, 0), gp_Dir2d(1, 0), v00, v10);
  const TopoDS_Edge right = edge(line(1, 0), 2.0, gp_Pnt2d(1, 0), gp_Dir2d(0, 1), v10, v12);
  const TopoDS_Edge top = edge(arc(2), 1.0, gp_Pnt2d(0, 2), gp_Dir2d(1, 0), v02, v12);
  const TopoDS_Edge upper = edge(line(0, 1), 1.0, gp_Pnt2d(0, 1), gp_Dir2d(0, 1), v01, v02);
  const TopoDS_Edge lower = edge(line(0, 0), 1.0, gp_Pnt2d(1e-6, 0), gp_Dir2d(0, 1), v00, v01);
  TopoDS_Wire wire;
  builder.MakeWire(wire);
  builder.Add(wire, bottom);
  builder.Add(wire, right);
  builder.Add(wire, top.Reversed());
  builder.Add(wire, upper.Reversed());
  builder.Add(wire, lower.Reversed());
  builder.Add(face, wire);
  ShapeUpgrade_UnifySameDomain unify(face, true, false, true);
  bool failed = false;
  try {
    unify.Build();
  } catch (const Standard_Failure&) {
    failed = true;
  }
  CHECK(!failed);
  if (!failed) {
    CHECK(count(unify.Shape(), TopAbs_EDGE) == 5);
  }
}

} // namespace

void large_body_tests() {
  guarded("test_a_checked_result_is_known_valid", test_a_checked_result_is_known_valid);
  guarded("test_the_check_where_changed_gives_the_whole_checks_answer",
          test_the_check_where_changed_gives_the_whole_checks_answer);
  guarded("test_faces_measured_once", test_faces_measured_once);
  guarded("test_optimal_box_is_occts", test_optimal_box_is_occts);
  guarded("test_edges_measured_once", test_edges_measured_once);
  guarded("test_face_boxes_hold_their_faces", test_face_boxes_hold_their_faces);
  guarded("test_wires_of_arcs_past_their_range_are_classified",
          test_wires_of_arcs_past_their_range_are_classified);
  guarded("test_the_orientation_of_a_shell_with_a_face_of_many_edges",
          test_the_orientation_of_a_shell_with_a_face_of_many_edges);
  guarded("test_merge_keeps_a_chain_it_cannot_concatenate", test_merge_keeps_a_chain_it_cannot_concatenate);
  guarded("test_merge_keeps_lines_whose_pcurves_do_not_meet", test_merge_keeps_lines_whose_pcurves_do_not_meet);
}

} // namespace test
