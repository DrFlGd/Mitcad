// SPDX-License-Identifier: MIT
// Corners where a rounding ends, built here with OCCT's fillet alone on
// synthetic shapes: the cases behind the patches 0130 and later of the OCCT
// port (third_party/vcpkg-ports, mitcad#236).

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <utility>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepTools.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Ax2.hxx>
#include <gp_Pln.hxx>

#include "check.hpp"

namespace test {
namespace {

// The volume, integrated to a relative error of 1e-9 (OCCT's default
// integration errs by more than the differences checked here on the
// roundings' spline faces).
double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props, 1e-9, true);
  return props.Mass();
}

int face_count(const TopoDS_Shape& shape) {
  int n = 0;
  for (TopExp_Explorer e(shape, TopAbs_FACE); e.More(); e.Next()) {
    ++n;
  }
  return n;
}

// Builds the operation; false when OCCT throws or does not finish.
bool build(BRepFilletAPI_MakeFillet& op) {
  try {
    OCC_CATCH_SIGNALS
    op.Build();
  } catch (const Standard_Failure& failure) {
    std::fprintf(stderr, "fillet corner test: %s\n", failure.what());
    return false;
  }
  return op.IsDone();
}

// A block 40 x 20 x 25 whose four vertical edges are rounded with a radius
// of 2, cut to a vault: below z = 10 the block, above it a cylinder of
// radius r along x whose axis lies at y = 10, z = 10. With r = 10 the vault
// springs tangentially from the walls y = 0 and y = 20; a larger r makes it
// cross them at an angle of sqrt(2 (r - 10) / 10).
TopoDS_Shape vaulted_block(double r) {
  const TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(0, 0, 0), gp_Pnt(40, 20, 25)).Shape();
  BRepFilletAPI_MakeFillet corners(block);
  for (TopExp_Explorer e(block, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if (std::abs(a.X() - b.X()) < 1e-9 && std::abs(a.Y() - b.Y()) < 1e-9) {
      corners.Add(2.0, edge);
    }
  }
  corners.Build();
  const TopoDS_Shape lower = BRepPrimAPI_MakeBox(gp_Pnt(-1, -1, -1), gp_Pnt(41, 21, 10)).Shape();
  const TopoDS_Shape vault =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(-1, 10, 10), gp_Dir(1, 0, 0)), r, 42).Shape();
  const TopoDS_Shape tool = BRepAlgoAPI_Fuse(lower, vault).Shape();
  ShapeUpgrade_UnifySameDomain unify(BRepAlgoAPI_Common(corners.Shape(), tool).Shape(), true, true, true);
  unify.Build();
  return unify.Shape();
}

// The edges of the end x = 0 above the vault's springing: the arc on the end
// face and the two curves where the vault meets the corner roundings.
void add_vault_end(BRepFilletAPI_MakeFillet& fillet, const TopoDS_Shape& body, double radius) {
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    const BRepAdaptor_Curve curve(edge);
    const gp_Pnt m = curve.Value(0.5 * (curve.FirstParameter() + curve.LastParameter()));
    const bool vertical = std::abs(a.X() - b.X()) < 1e-9 && std::abs(a.Y() - b.Y()) < 1e-9;
    if (!vertical && m.X() < 2 + 1e-6 && m.Z() > 10 + 1e-6 && a.Z() > 10 - 1e-6 && b.Z() > 10 - 1e-6 &&
        fillet.Contour(edge) == 0) {
      fillet.Add(radius, edge);
    }
  }
}

// The vault's end rounded with a radius of 1 (patch 0130): the curves where
// the vault meets the corner roundings end where both come tangent to the
// wall, and the rounding vanishes there. With a vault that misses tangency by
// a micrometre (radius 10.000001, faces 0.00045 rad apart at the vertex)
// OCCT stopped with an end section of micrometres off the vertex and failed
// at the corner; with radius 10.00001 it built a cap of micrometres whose
// wire intersects itself. Now both end in a point at the vertex, as the
// tangent vault (radius 10) does, and remove the same volume.
void test_rounding_vanishing_where_its_faces_come_tangent() {
  double tangent_removed = 0.0;
  for (const double r : {10.0, 10.000001, 10.00001}) {
    const TopoDS_Shape body = vaulted_block(r);
    BRepFilletAPI_MakeFillet fillet(body);
    add_vault_end(fillet, body, 1.0);
    CHECK(fillet.NbContours() == 1);
    CHECK(build(fillet));
    if (!fillet.IsDone()) {
      continue;
    }
    CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
    CHECK(face_count(fillet.Shape()) == face_count(body) + 5);
    const double removed = volume_of(body) - volume_of(fillet.Shape());
    if (r == 10.0) {
      tangent_removed = removed;
      CHECK(removed > 4.8 && removed < 4.9);
    } else {
      CHECK_NEAR_TOL(removed, tangent_removed, 1e-4);
    }
  }
}

// A prism 20 long along x whose end x = 0 is an octagon: the square
// y, z in [0, 10] with its corners cut at 45 degrees, legs of 2 (the top
// edge 6 long, from y = 2 to y = 8).
TopoDS_Shape octagonal_prism() {
  BRepBuilderAPI_MakePolygon outline;
  const std::pair<double, double> corners[] = {{2, 0}, {8, 0}, {10, 2}, {10, 8}, {8, 10}, {2, 10}, {0, 8}, {0, 2}};
  for (const auto& yz : corners) {
    outline.Add(gp_Pnt(0, yz.first, yz.second));
  }
  outline.Close();
  const TopoDS_Face end = BRepBuilderAPI_MakeFace(outline.Wire(), true).Face();
  return BRepPrimAPI_MakePrism(end, gp_Vec(20, 0, 0)).Shape();
}

// The edge of shape between points a and b.
TopoDS_Edge edge_between(const TopoDS_Shape& shape, const gp_Pnt& a, const gp_Pnt& b) {
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt p = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt q = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if ((p.Distance(a) < 1e-9 && q.Distance(b) < 1e-9) || (p.Distance(b) < 1e-9 && q.Distance(a) < 1e-9)) {
      return edge;
    }
  }
  return TopoDS_Edge();
}

// What a rounding of radius r removes along a straight edge from a to b
// between two perpendicular faces whose inward normals are n1 and n2: the
// square r x r beside the edge less the ball's quarter disc, swept along
// the edge 30 long both ways (cut by the body, it ends where the faces at
// its ends cut it).
TopoDS_Shape rounded_away(const gp_Pnt& a, const gp_Pnt& b, const gp_Dir& n1, const gp_Dir& n2, double r) {
  gp_Dir along(gp_Vec(a, b));
  if (along.Crossed(n1).Dot(n2) < 0) {
    along.Reverse();
  }
  const gp_Pnt start = gp_Pnt(0.5 * (a.XYZ() + b.XYZ())).Translated(-15.0 * gp_Vec(along));
  const TopoDS_Shape square = BRepPrimAPI_MakeBox(gp_Ax2(start, along, n1), r, r, 30).Shape();
  const gp_Pnt axis = start.Translated(r * gp_Vec(n1) + r * gp_Vec(n2));
  const TopoDS_Shape ball = BRepPrimAPI_MakeCylinder(gp_Ax2(axis, along), r, 30).Shape();
  return BRepAlgoAPI_Cut(square, ball).Shape();
}

// The two upper corners of the octagonal end rounded with a radius of 3,
// each a rounding of its own (patch 0210): on the top edge each rounding's
// setback is 3 sqrt(2) = 4.24, more than half its 6, so the two roundings'
// cuts by the top face crossed and OCCT failed ("fillets have too big
// radiuses"). Now the two blends are mitred: each is trimmed by the other
// and by the top face, the top edge vanishes, and the body is the octagonal
// prism less what each rounding removes along its edge.
void test_two_roundings_mitred_at_a_short_edge() {
  const TopoDS_Shape body = octagonal_prism();
  BRepFilletAPI_MakeFillet fillet(body);
  const gp_Pnt left_low(0, 0, 8), left_high(0, 2, 10), right_high(0, 8, 10), right_low(0, 10, 8);
  const TopoDS_Edge left = edge_between(body, left_low, left_high);
  const TopoDS_Edge right = edge_between(body, right_high, right_low);
  CHECK(!left.IsNull() && !right.IsNull());
  if (left.IsNull() || right.IsNull()) {
    return;
  }
  fillet.Add(3.0, left);
  fillet.Add(3.0, right);
  CHECK(fillet.NbContours() == 2);
  CHECK(build(fillet));
  if (!fillet.IsDone()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
  CHECK(face_count(fillet.Shape()) == face_count(body) + 2);
  CHECK(edge_between(fillet.Shape(), left_high, right_high).IsNull());
  const double s = std::sqrt(0.5);
  // (The inward normals of the end face and of the two upper bevels.)
  const gp_Dir into(1, 0, 0), below_left(0, s, -s), below_right(0, -s, -s);
  TopoDS_Shape expected = BRepAlgoAPI_Cut(body, rounded_away(left_low, left_high, into, below_left, 3.0)).Shape();
  expected = BRepAlgoAPI_Cut(expected, rounded_away(right_high, right_low, into, below_right, 3.0)).Shape();
  const double removed = volume_of(body) - volume_of(fillet.Shape());
  CHECK(removed > 1.0);
  // (Within the faces' boundaries' approximation: a rounding of one of the
  // edges alone, with no mitre, differs from its swept square by 1.7e-5.)
  CHECK_NEAR_TOL(removed, volume_of(body) - volume_of(expected), 5e-5);
}

// A cylinder of radius 10 along z whose seam runs at the angle a from x,
// cut by a gable roof: two planes of slope 0.5 meeting in a ridge along x
// at z = 20, which ends on the cylinder at (10, 0, 20) and (-10, 0, 20).
TopoDS_Shape gabled_cylinder(double a) {
  const TopoDS_Shape cylinder =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1), gp_Dir(std::cos(a), std::sin(a), 0)), 10, 30)
      .Shape();
  BRepBuilderAPI_MakePolygon gable;
  for (const auto& yz : {std::pair<double, double>{-20, -1}, {20, -1}, {20, 10}, {0, 20}, {-20, 10}}) {
    gable.Add(gp_Pnt(-20, yz.first, yz.second));
  }
  gable.Close();
  const TopoDS_Shape roof =
    BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(gable.Wire(), true).Face(), gp_Vec(40, 0, 0)).Shape();
  return BRepAlgoAPI_Common(cylinder, roof).Shape();
}

// The gabled cylinder's ridge rounded with a radius of 2; the volume it
// removes, or a negative value when the rounding fails or is invalid.
double ridge_rounding_removes(const TopoDS_Shape& body) {
  BRepFilletAPI_MakeFillet fillet(body);
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if (std::abs(a.Y()) < 1e-7 && std::abs(b.Y()) < 1e-7 && a.Z() > 19.9 && b.Z() > 19.9 &&
        BRepAdaptor_Curve(edge).GetType() == GeomAbs_Line) {
      fillet.Add(2.0, edge);
    }
  }
  if (fillet.NbContours() != 1 || !build(fillet) || !BRepCheck_Analyzer(fillet.Shape()).IsValid()) {
    return -1.0;
  }
  return volume_of(body) - volume_of(fillet.Shape());
}

// The ridge's end at (10, 0, 20) is cut by the cylinder, whose seam crosses
// that cut where it runs 0.02 rad from the ridge (patch 0211): the arcs that
// hold the rounding's contacts there have their curves on the cylinder on
// the seam's two sides, a period apart, and the cut was sought round the
// whole cylinder (OCCT built an invalid body without its floor). Now it
// crosses the seam, and the rounding removes what it removes with the seam a
// quarter turn away.
void test_rounding_cut_by_a_cylinder_across_its_seam() {
  const double away = ridge_rounding_removes(gabled_cylinder(2.0 * std::atan(1.0)));
  CHECK(away > 2.8 && away < 3.0);
  CHECK_NEAR_TOL(ridge_rounding_removes(gabled_cylinder(0.02)), away, 1e-6);
}

// A ring of outer radius 15.3612 (z in [-2, 0]) whose top edge is rounded
// with 1.2 (its top plane turns tangentially into the rim's torus at radius
// 14.1612) on a core of radius 13 (z in [-5, -2]), and a pillar of radius 1
// whose axis lies at 14.361 from the ring's (z in [-5, 0]), fused, the tops
// one plane: the pillar's wall stands above the rim's torus, highest at its
// outer side. Its seam lies at the angle a from x.
TopoDS_Shape pillar_on_rimmed_ring(double a) {
  const TopoDS_Shape disc = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -5), gp_Dir(0, 0, 1)), 15.3612, 5).Shape();
  BRepFilletAPI_MakeFillet rim(disc);
  for (TopExp_Explorer e(disc, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    if (BRep_Tool::Pnt(TopExp::FirstVertex(edge)).Z() > -1e-9 && BRepAdaptor_Curve(edge).GetType() == GeomAbs_Circle) {
      rim.Add(1.2, edge);
    }
  }
  rim.Build();
  const TopoDS_Shape below = BRepAlgoAPI_Cut(BRepPrimAPI_MakeBox(gp_Pnt(-30, -30, -6), gp_Pnt(30, 30, -2)).Shape(),
                                             BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -7), gp_Dir(0, 0, 1)), 13, 6).Shape())
                               .Shape();
  const TopoDS_Shape ring = BRepAlgoAPI_Cut(rim.Shape(), below).Shape();
  const TopoDS_Shape pillar =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(14.361, 0, -5), gp_Dir(0, 0, 1), gp_Dir(std::cos(a), std::sin(a), 0)), 1, 5)
      .Shape();
  ShapeUpgrade_UnifySameDomain unify(BRepAlgoAPI_Fuse(ring, pillar).Shape(), true, true, true);
  unify.Build();
  return unify.Shape();
}

// The pillar's top edge rounded with 0.79, the result, or a null shape when
// the rounding fails.
TopoDS_Shape round_pillar_top(const TopoDS_Shape& body) {
  BRepFilletAPI_MakeFillet fillet(body);
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const BRepAdaptor_Curve curve(edge);
    if (curve.GetType() == GeomAbs_Circle && std::abs(curve.Circle().Radius() - 1) < 1e-7 &&
        curve.Circle().Location().Distance(gp_Pnt(14.361, 0, 0)) < 1e-7) {
      fillet.Add(0.79, edge);
    }
  }
  CHECK(fillet.NbContours() == 1);
  if (fillet.NbContours() != 1 || !build(fillet)) {
    return TopoDS_Shape();
  }
  return fillet.Shape();
}

// Whether the shape has a vertex within tol of p.
bool has_vertex_at(const TopoDS_Shape& shape, const gp_Pnt& p, double tol) {
  for (TopExp_Explorer e(shape, TopAbs_VERTEX); e.More(); e.Next()) {
    if (BRep_Tool::Pnt(TopoDS::Vertex(e.Current())).Distance(p) < tol) {
      return true;
    }
  }
  return false;
}

// The pillar's top edge rounded with 0.79 (patch 0280): at both ends the
// wall's contact reaches the wall's edge with the rim's torus before the
// edge's vertex on the torus' tangent circle, and the top's contact (a
// circle of radius 0.21 round the pillar's axis) crosses that circle past
// the vertex. The blend is cut by the torus up to that crossing, to which
// the tangent circle runs on. OCCT failed at the corner: the crossing lies
// on the contact circle's prolongation past its parameter origin (rejected
// as out of range), and the circle's missing end was sought as the
// intersection of the top and the torus, which touch along it. Now the
// rounding is one face more, its corners end at the crossings, and it
// removes the same with the pillar's seam turned elsewhere.
void test_rounding_ending_where_the_top_turns_into_a_rim() {
  // (The crossings of the contact circle with the tangent circle.)
  const double d = 14.361, r1 = 0.21, r2 = 14.1612;
  const double x = (d * d + r2 * r2 - r1 * r1) / (2 * d), y = std::sqrt(r2 * r2 - x * x);
  double first = -1.0;
  for (const double a : {3.0, 2.5}) {
    const TopoDS_Shape body = pillar_on_rimmed_ring(a);
    const TopoDS_Shape result = round_pillar_top(body);
    CHECK(!result.IsNull());
    if (result.IsNull()) {
      continue;
    }
    CHECK(BRepCheck_Analyzer(result).IsValid());
    CHECK(face_count(result) == face_count(body) + 1);
    // (The circles cross at an angle of 0.3 rad, and the contact meets the
    // torus tangentially: the crossing found lies a few 1e-4 off.)
    CHECK(has_vertex_at(result, gp_Pnt(x, y, 0), 1e-3));
    CHECK(has_vertex_at(result, gp_Pnt(x, -y, 0), 1e-3));
    const double removed = volume_of(body) - volume_of(result);
    CHECK(removed > 0.2 && removed < 0.3);
    if (first < 0) {
      first = removed;
    } else {
      CHECK_NEAR_TOL(removed, first, 1e-7);
    }
  }
}

// The octant x, y, z in [-10, 0] whose vertical edge x = y = 0 is cut below
// (0, 0, -0.1) by the plane x + y = z + 0.1: the corner (0, 0, 0) keeps a
// short edge 0.1 long down to the cut. A notch (a box 1 wide, 1 high) at a
// depth of 4 on the cut's line on the face x = 0 splits that line in two
// edges, both between that face and the cut; the face x = 0 is built with
// its wire starting at the lower one (an order the booleans may give as
// well).
TopoDS_Shape octant_with_cut_corner() {
  TopoDS_Shape body = BRepPrimAPI_MakeBox(gp_Pnt(-10, -10, -10), gp_Pnt(0, 0, 0)).Shape();
  const gp_Pln cut(gp_Pnt(0, 0, -0.1), gp_Dir(gp_Vec(1, 1, -1).Normalized()));
  const TopoDS_Face plane = BRepBuilderAPI_MakeFace(cut, -50, 50, -50, 50).Face();
  body = BRepAlgoAPI_Cut(body, BRepPrimAPI_MakePrism(plane, gp_Vec(1, 1, -1).Normalized() * 30.0).Shape()).Shape();
  body = BRepAlgoAPI_Cut(body, BRepPrimAPI_MakeBox(gp_Pnt(-0.5, -4.4, -4.5), gp_Pnt(0.5, -3.4, -3.5)).Shape()).Shape();
  for (TopExp_Explorer f(body, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    const BRepAdaptor_Surface surface(face);
    if (surface.GetType() != GeomAbs_Plane || std::abs(std::abs(surface.Plane().Axis().Direction().X()) - 1) > 1e-9 ||
        !has_vertex_at(face, gp_Pnt(0, 0, -0.1), 1e-7)) {
      continue;
    }
    std::vector<TopoDS_Edge> edges;
    for (BRepTools_WireExplorer w(BRepTools::OuterWire(face), face); w.More(); w.Next()) {
      edges.push_back(w.Current());
    }
    std::size_t start = 0;
    for (std::size_t i = 0; i < edges.size(); ++i) {
      const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edges[i]));
      const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edges[i]));
      if (std::abs(a.Y() - a.Z() - 0.1) < 1e-7 && std::abs(b.Y() - b.Z() - 0.1) < 1e-7 && std::min(a.Z(), b.Z()) < -4.6) {
        start = i;
      }
    }
    BRepBuilderAPI_MakeWire wire;
    for (std::size_t k = 0; k < edges.size(); ++k) {
      wire.Add(edges[(start + k) % edges.size()]);
    }
    TopoDS_Face rebuilt = BRepBuilderAPI_MakeFace(BRep_Tool::Surface(face), wire.Wire(), true).Face();
    if (rebuilt.Orientation() != face.Orientation()) {
      rebuilt.Reverse();
    }
    occ::handle<BRepTools_ReShape> reshape = new BRepTools_ReShape();
    reshape->Replace(face, rebuilt);
    return reshape->Apply(body);
  }
  return body;
}

// What a rounding of radius r of the octant's edge y = z = 0 removes from
// x = -10 up to the face x = 0 and the cut x + y = z + 0.1: the square r x r
// beside the edge less the quarter disc, swept to where the cut or that
// face ends it (integrated on a fine grid).
double rounded_up_to_cut(double r) {
  const int n = 1000;
  const double h = r / n;
  double sum = 0.0;
  for (int i = 0; i < n; ++i) {
    for (int j = 0; j < n; ++j) {
      const double y = -r + (i + 0.5) * h, z = -r + (j + 0.5) * h;
      if ((y + r) * (y + r) + (z + r) * (z + r) >= r * r) {
        sum += (std::min(0.0, z + 0.1 - y) + 10.0) * h * h;
      }
    }
  }
  return sum;
}

// The octant's edge y = z = 0 rounded with 1 up to the corner (patch 0281):
// the contact on y = 0 meets the cut before the corner, and the walk round
// the corner goes from there through the short edge to the face x = 0; the
// common edge of the cut and that face at the short edge's end is the upper
// of their two edges, OCCT took the first found, the lower one, and failed
// at the corner. Now the rounding removes what it removes up to the cut.
void test_walk_round_the_corner_past_a_short_edge() {
  const TopoDS_Shape body = octant_with_cut_corner();
  BRepFilletAPI_MakeFillet fillet(body);
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if (std::abs(a.Y()) < 1e-9 && std::abs(b.Y()) < 1e-9 && std::abs(a.Z()) < 1e-9 && std::abs(b.Z()) < 1e-9) {
      fillet.Add(1.0, edge);
    }
  }
  CHECK(fillet.NbContours() == 1);
  CHECK(build(fillet));
  if (!fillet.IsDone()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
  CHECK(face_count(fillet.Shape()) == face_count(body) + 1);
  CHECK_NEAR_TOL(volume_of(body) - volume_of(fillet.Shape()), rounded_up_to_cut(1.0), 1e-4);
}

// A floor z <= 0 and a wall x <= 0 along y in [-20, 20], built as two
// halves split at y = 7 (their faces merged, the edge between floor and
// wall keeping the split), with a hole 10 deep down from the floor beside
// the wall: its outline from the wall at y = 6 by a 45 degree chamfer to
// its side y = 7, to x = 8, and back the same way to the wall at y = -6.
TopoDS_Shape floor_wall_with_hole() {
  const auto half = [](double y0, double y1) {
    return BRepAlgoAPI_Fuse(BRepPrimAPI_MakeBox(gp_Pnt(-5, y0, -15), gp_Pnt(20, y1, 0)).Shape(),
                            BRepPrimAPI_MakeBox(gp_Pnt(-5, y0, -15), gp_Pnt(0, y1, 10)).Shape())
      .Shape();
  };
  const TopoDS_Shape halves = BRepAlgoAPI_Fuse(half(-20, 7), half(7, 20)).Shape();
  BRepBuilderAPI_MakePolygon outline;
  for (const auto& xy : {std::pair<double, double>{0, 6}, {1, 7}, {8, 7}, {8, -7}, {1, -7}, {0, -6}}) {
    outline.Add(gp_Pnt(xy.first, xy.second, -10));
  }
  outline.Close();
  const TopoDS_Shape hole =
    BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(outline.Wire(), true).Face(), gp_Vec(0, 0, 20)).Shape();
  ShapeUpgrade_UnifySameDomain unify(BRepAlgoAPI_Cut(halves, hole).Shape(), false, true, true);
  unify.Build();
  return unify.Shape();
}

// The edge between floor and wall from y = 20 to the hole, rounded with 2;
// the result, or a null shape when the rounding fails.
TopoDS_Shape round_wall_foot(const TopoDS_Shape& body) {
  BRepFilletAPI_MakeFillet fillet(body);
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if (std::abs(a.X()) < 1e-9 && std::abs(b.X()) < 1e-9 && std::abs(a.Z()) < 1e-9 && std::abs(b.Z()) < 1e-9 &&
        a.Y() > 5.9 && b.Y() > 5.9 && fillet.Contour(edge) == 0) {
      fillet.Add(2.0, edge);
    }
  }
  CHECK(fillet.NbContours() == 1);
  if (fillet.NbContours() != 1 || !build(fillet)) {
    return TopoDS_Shape();
  }
  return fillet.Shape();
}

// What the rounding of the wall's foot adds: the square 2 x 2 in the corner
// less the quarter disc, from y = 20 down to the hole's side (y = 7, x >= 1)
// and its chamfer (y = 6 + x).
double added_up_to_hole() {
  const double r = 2.0;
  const int n = 1000;
  const double h = r / n;
  double sum = 0.0;
  for (int i = 0; i < n; ++i) {
    for (int j = 0; j < n; ++j) {
      const double x = (i + 0.5) * h, z = (j + 0.5) * h;
      if ((x - r) * (x - r) + (z - r) * (z - r) >= r * r) {
        sum += (20.0 - (x >= 1.0 ? 7.0 : 6.0 + x)) * h * h;
      }
    }
  }
  return sum;
}

// The wall's foot rounded up to the hole (patch 0282): the floor's contact
// meets the hole's side, the wall's runs on along the wall; the blend is
// cut by the hole's side and chamfer continued up through it, the wall's
// contact ending where the chamfer's plane meets it, above the chamfer's
// face. OCCT sought that end on the chamfer's face only and failed. Now the
// rounding adds what it adds up to the hole.
void test_rounding_cut_by_faces_past_their_bounds() {
  const TopoDS_Shape body = floor_wall_with_hole();
  const TopoDS_Shape result = round_wall_foot(body);
  CHECK(!result.IsNull());
  if (result.IsNull()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(result).IsValid());
  CHECK_NEAR_TOL(volume_of(result) - volume_of(body), added_up_to_hole(), 1e-4);
}

// The same rounding is one face (patch 0283): its piece along the edge from
// the split at y = 7 to the hole, whose floor contact stands at the hole's
// side, was a face of its own, cut off at y = 7.
void test_rounding_one_face_up_to_the_hole() {
  const TopoDS_Shape body = floor_wall_with_hole();
  const TopoDS_Shape result = round_wall_foot(body);
  CHECK(!result.IsNull());
  if (!result.IsNull()) {
    CHECK(face_count(result) == face_count(body) + 1);
  }
}

// A ceiling z = 0 over a cavity y < 0 against a wall y = 0 (the solid an L
// in the section, x in [-10, 10]), with a groove along their junction: the
// cavity runs on above the ceiling between the wall and the plane through
// y = -0.3, z = 0 and y = 0, z = 1. A rib x in [-2, 2], y in [-3, 0] stands
// against the wall up to the ceiling; under the groove its top is a face of
// its own, beside the ceiling in the same plane, the two meeting at the
// points where the groove's side crosses the rib's sides.
TopoDS_Shape ceiling_groove_rib() {
  TopoDS_Shape body = BRepAlgoAPI_Cut(BRepPrimAPI_MakeBox(gp_Pnt(-10, -10, -10), gp_Pnt(10, 10, 10)).Shape(),
                                      BRepPrimAPI_MakeBox(gp_Pnt(-11, -11, -11), gp_Pnt(11, 0, 0)).Shape())
                        .Shape();
  body = BRepAlgoAPI_Fuse(body, BRepPrimAPI_MakeBox(gp_Pnt(-2, -3, -10), gp_Pnt(2, 0, 0)).Shape()).Shape();
  BRepBuilderAPI_MakePolygon section;
  section.Add(gp_Pnt(-11, -0.3, 0));
  section.Add(gp_Pnt(-11, 0, 0));
  section.Add(gp_Pnt(-11, 0, 1));
  section.Close();
  const TopoDS_Shape groove =
    BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(section.Wire(), true).Face(), gp_Vec(22, 0, 0)).Shape();
  ShapeUpgrade_UnifySameDomain unify(BRepAlgoAPI_Cut(body, groove).Shape(), false, true, true);
  unify.Build();
  return unify.Shape();
}

// The rib's feet against the wall rounded with 1 up to the ceiling (patch
// 0290): at each foot's top the contact on the rib's side meets the ceiling
// past the point where the ceiling, the groove's side and the rib's top
// meet it; the walk round the corner from there found no edge between the
// ceiling and the rib's top, which meet at that point only. The fillet's
// end passes from the ceiling to the rib's top, grown under the groove,
// through the groove's side at a point. Each foot adds the square 1 x 1
// less the quarter disc over the height of 10.
void test_walk_round_the_corner_through_a_face_met_at_a_point() {
  const TopoDS_Shape body = ceiling_groove_rib();
  BRepFilletAPI_MakeFillet fillet(body);
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if (std::abs(std::abs(a.X()) - 2) < 1e-9 && std::abs(b.X() - a.X()) < 1e-9 && std::abs(a.Y()) < 1e-9 &&
        std::abs(b.Y()) < 1e-9) {
      fillet.Add(1.0, edge);
    }
  }
  CHECK(fillet.NbContours() == 2);
  CHECK(build(fillet));
  if (!fillet.IsDone()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
  CHECK(face_count(fillet.Shape()) == face_count(body) + 2);
  CHECK_NEAR_TOL(volume_of(fillet.Shape()) - volume_of(body), 2 * (1 - kPi / 4) * 10, 1e-6);
}

// A point at angle phi round a circle of radius r about the z axis in the
// plane z = 0, the angle from -y towards +x.
gp_Pnt round_axis(double r, double phi) {
  return gp_Pnt(r * std::sin(phi), -r * std::cos(phi), 0);
}

// A block x in [20 sin(-0.5), 12], y in [-30, -5], z in [-10, 0] outside
// a wall: the plane y = -20 for x <= 0, then the cylinder of radius 20
// about the z axis (tangent to it), up to angle 0.36 or, with `turn`, from
// there on the cylinder's tangent plane. Its top is cut by the plane
// through the cylinder's points at angles 0.3 and 0.7 on the top, falling
// away from the wall at a slope of 1/2: the top a strip along the wall
// narrowing to nothing at angle 0.3, the wall beyond it under the cut.
TopoDS_Shape tapering_strip_on_a_wall(bool turn) {
  const double x0   = 20 * std::sin(-0.5);
  TopoDS_Shape body = BRepPrimAPI_MakeBox(gp_Pnt(x0, -30, -10), gp_Pnt(12, -5, 0)).Shape();
  body = BRepAlgoAPI_Cut(body, BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -20), gp_Dir(0, 0, 1)), 20, 40).Shape())
           .Shape();
  body = BRepAlgoAPI_Cut(body, BRepPrimAPI_MakeBox(gp_Pnt(x0 - 1, -20, -11), gp_Pnt(0, 0, 1)).Shape()).Shape();
  if (turn) {
    const gp_Pnt p = round_axis(20, 0.36);
    const gp_Dir t(std::cos(0.36), std::sin(0.36), 0);
    body = BRepAlgoAPI_Cut(body, BRepPrimAPI_MakeBox(gp_Ax2(gp_Pnt(p.X(), p.Y(), -11), gp_Dir(0, 0, 1), t), 30, 30, 22).Shape())
             .Shape();
  }
  const gp_Pnt a = round_axis(20, 0.7), b = round_axis(20, 0.3);
  const gp_Vec along(a, b);
  gp_Vec       away(-along.Y(), along.X(), 0);
  if (away.X() * (a.X() + b.X()) + away.Y() * (a.Y() + b.Y()) < 0) {
    away.Reverse();
  }
  gp_Dir normal(along.Crossed(away.Normalized() + gp_Vec(0, 0, -0.5)));
  if (normal.Z() < 0) {
    normal.Reverse();
  }
  const TopoDS_Face cut = BRepBuilderAPI_MakeFace(gp_Pln(a, normal), -80, 80, -80, 80).Face();
  ShapeUpgrade_UnifySameDomain unify(
    BRepAlgoAPI_Cut(body, BRepPrimAPI_MakePrism(cut, gp_Vec(normal) * 60.0).Shape()).Shape(), false, true, true);
  unify.Build();
  return unify.Shape();
}

// The wall's top edge up to angle 0.3 rounded with 1, or a null shape.
TopoDS_Shape round_wall_top(const TopoDS_Shape& body) {
  BRepFilletAPI_MakeFillet fillet(body);
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge&      edge = TopoDS::Edge(e.Current());
    const BRepAdaptor_Curve curve(edge);
    const gp_Pnt            m = curve.Value(0.5 * (curve.FirstParameter() + curve.LastParameter()));
    const bool on_cylinder    = curve.GetType() == GeomAbs_Circle && std::abs(std::hypot(m.X(), m.Y()) - 20) < 1e-9;
    const bool on_plane       = curve.GetType() == GeomAbs_Line && std::abs(m.Y() + 20) < 1e-9;
    if (std::abs(m.Z()) < 1e-9 && m.X() < 6 && (on_cylinder || on_plane)) {
      fillet.Add(1.0, edge);
    }
  }
  CHECK(fillet.NbContours() == 1);
  if (fillet.NbContours() != 1 || !build(fillet)) {
    return TopoDS_Shape();
  }
  return fillet.Shape();
}

// The strip's edge with the wall rounded up to where the strip ends, the
// wall running on there as its tangent plane (patch 0291): the top's
// contact runs off the strip across the cut before the end, and the piece
// past it, walked on the top's plane, was dropped at the corner, which
// then failed. The rounding runs on as the blend before it, cut by the
// sloped face and by the wall's tangent plane: it removes what the same
// rounding removes from the block whose wall stays the cylinder, less what
// of that lies past the wall's turn.
void test_rounding_past_the_end_of_a_tapering_strip() {
  const TopoDS_Shape body   = tapering_strip_on_a_wall(true);
  const TopoDS_Shape result = round_wall_top(body);
  CHECK(!result.IsNull());
  if (result.IsNull()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(result).IsValid());
  const TopoDS_Shape plain         = tapering_strip_on_a_wall(false);
  const TopoDS_Shape plain_rounded = round_wall_top(plain);
  CHECK(!plain_rounded.IsNull());
  if (plain_rounded.IsNull()) {
    return;
  }
  const gp_Pnt       p = round_axis(20, 0.36);
  const gp_Dir       t(std::cos(0.36), std::sin(0.36), 0);
  const TopoDS_Shape past =
    BRepPrimAPI_MakeBox(gp_Ax2(gp_Pnt(p.X(), p.Y(), -11), gp_Dir(0, 0, 1), t), 30, 30, 22).Shape();
  const TopoDS_Shape removed = BRepAlgoAPI_Cut(BRepAlgoAPI_Cut(plain, plain_rounded).Shape(), past).Shape();
  CHECK_NEAR_TOL(volume_of(body) - volume_of(result), volume_of(removed), 1e-6);
}

} // namespace

void fillet_corner_tests() {
  guarded("test_rounding_vanishing_where_its_faces_come_tangent",
          test_rounding_vanishing_where_its_faces_come_tangent);
  guarded("test_two_roundings_mitred_at_a_short_edge", test_two_roundings_mitred_at_a_short_edge);
  guarded("test_rounding_cut_by_a_cylinder_across_its_seam", test_rounding_cut_by_a_cylinder_across_its_seam);
  guarded("test_rounding_ending_where_the_top_turns_into_a_rim", test_rounding_ending_where_the_top_turns_into_a_rim);
  guarded("test_walk_round_the_corner_past_a_short_edge", test_walk_round_the_corner_past_a_short_edge);
  guarded("test_rounding_cut_by_faces_past_their_bounds", test_rounding_cut_by_faces_past_their_bounds);
  guarded("test_rounding_one_face_up_to_the_hole", test_rounding_one_face_up_to_the_hole);
  guarded("test_walk_round_the_corner_through_a_face_met_at_a_point",
          test_walk_round_the_corner_through_a_face_met_at_a_point);
  guarded("test_rounding_past_the_end_of_a_tapering_strip", test_rounding_past_the_end_of_a_tapering_strip);
}

} // namespace test
