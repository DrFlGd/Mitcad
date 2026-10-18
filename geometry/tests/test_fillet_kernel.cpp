// SPDX-License-Identifier: MIT
// Roundings that OCCT's fillet (TKFillet) failed on or broke, built here
// with OCCT alone on synthetic shapes: the cases behind the patches 0020 and
// later of the OCCT port (third_party/vcpkg-ports, mitcad#122), and where a
// fillet that is not done stopped (patch 0320, mitcad#322), an open stripe
// ending at the vertex of the edge its contact runs along (patch 0322,
// mitcad#331), which corner a fillet failed at (0330), a rounding rolled
// on a straight edge (0331, mitcad#323) and a boss's rim rounded on a
// cylinder whose seam runs in steps (patch 0420, mitcad#337).

#include <algorithm>
#include <cmath>
#include <string>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRep_Tool.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GProp_GProps.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Lin.hxx>
#include <gp_Ax1.hxx>
#include <gp_Ax2.hxx>
#include <gp_Trsf.hxx>

#include "check.hpp"

namespace test {
namespace {

double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props);
  return props.Mass();
}

// Builds the operation; false when OCCT throws or does not finish.
bool build(BRepFilletAPI_LocalOperation& op) {
  try {
    OCC_CATCH_SIGNALS
    op.Build();
  } catch (const Standard_Failure& failure) {
    std::fprintf(stderr, "fillet kernel test: %s\n", failure.what());
    return false;
  }
  return op.IsDone();
}

// True when both ends of the edge lie at height z.
bool at_height(const TopoDS_Edge& edge, double z) {
  return std::abs(BRep_Tool::Pnt(TopExp::FirstVertex(edge)).Z() - z) < 1e-6 &&
         std::abs(BRep_Tool::Pnt(TopExp::LastVertex(edge)).Z() - z) < 1e-6;
}

// A teardrop prism 20 high: two lines from the tip at the origin, tangent
// to a circle of radius 10 about (20, 0), and the circle's far arc.
TopoDS_Shape teardrop_prism() {
  const double s = 10 * std::sqrt(3.0) / 2;
  const gp_Pnt tip(0, 0, 0), upper(15, s, 0), lower(15, -s, 0), far(30, 0, 0);
  BRepBuilderAPI_MakeWire wire;
  wire.Add(BRepBuilderAPI_MakeEdge(tip, upper).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(upper, far, lower).Value()).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(lower, tip).Edge());
  return BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(wire.Wire()).Face(), gp_Vec(0, 0, 20)).Shape();
}

// The rim of a teardrop prism chamfered 2, then the lower edges of the
// chamfer rounded: one contour of tangent edges closed by its sharp corner
// at the tip, where four sharp edges meet (patch 0021). OCCT matched only
// the contour's first end there and threw
// "TopOpeBRepDS_DataStructure::Point".
void test_contour_closed_at_a_sharp_corner() {
  const TopoDS_Shape prism = teardrop_prism();
  BRepFilletAPI_MakeChamfer chamfer(prism);
  for (TopExp_Explorer e(prism, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    if (at_height(edge, 20) && chamfer.Contour(edge) == 0) {
      chamfer.Add(2.0, edge);
    }
  }
  CHECK(build(chamfer));
  const TopoDS_Shape chamfered = chamfer.Shape();
  BRepFilletAPI_MakeFillet fillet(chamfered);
  for (TopExp_Explorer e(chamfered, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    if (at_height(edge, 18) && fillet.Contour(edge) == 0) {
      fillet.Add(1.0, edge);
    }
  }
  CHECK(fillet.NbContours() == 1);
  CHECK(build(fillet));
  if (!fillet.IsDone()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
  // A rolling ball of radius 1 between faces whose normals are 45 degrees
  // apart takes (tan 22.5 deg - pi/8) r^2 of the section along the 76.5 mm
  // of the contour, a little less at the tip.
  const double length = 2 * std::sqrt(300.0) + 10 * 4 * kPi / 3;
  const double loss = (std::tan(kPi / 8) - kPi / 8) * length;
  const double removed = volume_of(chamfered) - volume_of(fillet.Shape());
  CHECK(removed > 0.98 * loss && removed < loss);
}

// A prism whose top is two planes meeting at 3 degrees along a ridge 20 long,
// the ridge rounded with a radius of 50 (patch 0022). OCCT took the faces
// for tangent (within 0.1 rad) and refused the edge.
void test_ridge_between_faces_at_a_small_angle() {
  const double alpha = 3 * kPi / 180;
  BRepBuilderAPI_MakePolygon outline;
  outline.Add(gp_Pnt(0, 0, 0));
  outline.Add(gp_Pnt(40, 0, 0));
  outline.Add(gp_Pnt(40, 10, 0));
  outline.Add(gp_Pnt(20, 10 + 20 * std::tan(alpha / 2), 0));
  outline.Add(gp_Pnt(0, 10, 0));
  outline.Close();
  const TopoDS_Shape prism =
      BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(outline.Wire()).Face(), gp_Vec(0, 0, 20)).Shape();
  BRepFilletAPI_MakeFillet fillet(prism);
  for (TopExp_Explorer e(prism, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    if (std::abs(BRep_Tool::Pnt(TopExp::FirstVertex(edge)).X() - 20) < 1e-9 &&
        std::abs(BRep_Tool::Pnt(TopExp::LastVertex(edge)).X() - 20) < 1e-9) {
      fillet.Add(50, edge);
    }
  }
  CHECK(fillet.NbContours() == 1);
  CHECK(build(fillet));
  if (!fillet.IsDone()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
  const double loss = 50 * 50 * (std::tan(alpha / 2) - alpha / 2) * 20;
  CHECK_NEAR_TOL(volume_of(prism) - volume_of(fillet.Shape()), loss, 1e-4);

  // Faces within the angular tolerance stay tangent: nothing to round.
  BRepFilletAPI_MakeFillet flat(prism);
  flat.SetParams(0.06, 1e-4, 1e-5, 1e-4, 1e-5, 1e-3);
  for (TopExp_Explorer e(prism, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    if (std::abs(BRep_Tool::Pnt(TopExp::FirstVertex(edge)).X() - 20) < 1e-9 &&
        std::abs(BRep_Tool::Pnt(TopExp::LastVertex(edge)).X() - 20) < 1e-9) {
      flat.Add(50, edge);
    }
  }
  CHECK(flat.NbContours() == 0);
}

// The edge of a shape between the points a and b.
TopoDS_Edge box_edge(const TopoDS_Shape& box, const gp_Pnt& a, const gp_Pnt& b) {
  for (TopExp_Explorer e(box, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt p = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt q = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if ((p.Distance(a) < 1e-9 && q.Distance(b) < 1e-9) || (p.Distance(b) < 1e-9 && q.Distance(a) < 1e-9)) {
      return edge;
    }
  }
  return TopoDS_Edge();
}

// Where a fillet that is not done stopped (patch 0320): a box edge rounded
// with a radius larger than its faces finds no start for the rolling ball;
// the contour it stopped at is that edge's, also after a contour that
// builds. A fillet that is done reports none.
void test_where_a_fillet_stops() {
  const TopoDS_Shape box = BRepPrimAPI_MakeBox(10, 10, 10).Shape();
  const TopoDS_Edge front = box_edge(box, gp_Pnt(0, 0, 10), gp_Pnt(10, 0, 10));
  const TopoDS_Edge back = box_edge(box, gp_Pnt(0, 10, 10), gp_Pnt(10, 10, 10));
  CHECK(!front.IsNull() && !back.IsNull());

  BRepFilletAPI_MakeFillet done(box);
  done.Add(1.0, front);
  CHECK(build(done));
  CHECK(done.Failure() == ChFi3d_Builder::Failure_None);
  CHECK(done.FailedContour() == 0);

  BRepFilletAPI_MakeFillet too_large(box);
  too_large.Add(1.0, back);
  too_large.Add(12.0, front);
  CHECK(!build(too_large));
  CHECK(too_large.NbFaultyVertices() == 0);
  CHECK(too_large.Failure() == ChFi3d_Builder::Failure_NoStart);
  CHECK(too_large.FailedContour() == 2);
  CHECK(too_large.FailedContour() > 0 && too_large.Edge(too_large.FailedContour(), 1).IsSame(front));
}

// A stadium (half circles of radius r about (0, 0) and (0, len), joined by
// lines) in the plane z = 0, as a face.
TopoDS_Face stadium(double r, double len) {
  const gp_Pnt a(-r, 0, 0), b(-r, len, 0), c(r, len, 0), d(r, 0, 0);
  BRepBuilderAPI_MakeWire wire;
  wire.Add(BRepBuilderAPI_MakeEdge(a, b).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(b, gp_Pnt(0, len + r, 0), c).Value()).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(c, d).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(d, gp_Pnt(0, -r, 0), a).Value()).Edge());
  return BRepBuilderAPI_MakeFace(wire.Wire()).Face();
}

// A block with a slot (half circles of radius 7.5, 20 apart) through it
// and a counterbore of the same slot's shape 0.5 wider, 5 deep, so that the
// counterbore's floor is a ledge 0.5 wide all round the slot's rim; the rim
// rounded with 0.5 (or chamfered 0.5), exactly as wide as the ledge (patch
// 0321). The rounding's contact on the ledge runs along the ledge's outer
// edges all round: one closed contour of two lines and two arcs whose blend
// consumes the ledge. OCCT made a point of its own for the closed stripe's
// start beside the ledge's vertex there, and the topological build failed
// (TopoDS_Builder::Add); a round hole (one closed edge) built.
void test_rim_rounded_as_wide_as_its_ledge() {
  const double ri = 7.5, len = 20.0, rr = 0.5;
  const TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(-30, -30, 0), gp_Pnt(30, 50, 20)).Shape();
  TopoDS_Shape counterbore = BRepPrimAPI_MakePrism(stadium(ri + rr, len), gp_Vec(0, 0, 10)).Shape();
  TopoDS_Shape slot = BRepPrimAPI_MakePrism(stadium(ri, len), gp_Vec(0, 0, 30)).Shape();
  gp_Trsf up, down;
  up.SetTranslation(gp_Vec(0, 0, 15));
  down.SetTranslation(gp_Vec(0, 0, -5));
  counterbore.Move(TopLoc_Location(up));
  slot.Move(TopLoc_Location(down));
  const TopoDS_Shape body = BRepAlgoAPI_Cut(BRepAlgoAPI_Cut(block, counterbore).Shape(), slot).Shape();
  // The slot's rim on the ledge (z = 15).
  std::vector<TopoDS_Edge> rim;
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    BRepAdaptor_Curve c(edge);
    const gp_Pnt m = c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
    const double along = std::max(0.0, std::min(len, m.Y()));
    if (std::abs(m.Z() - 15) < 1e-7 && std::abs(std::hypot(m.X(), m.Y() - along) - ri) < 1e-6) {
      rim.push_back(edge);
    }
  }
  CHECK(rim.size() >= 4);
  // The section a rounding of radius r takes off a right-angled convex
  // edge, (1 - pi/4) r^2, its centroid 0.2234 r (r (10 - 3 pi) / (3 (4 - pi)))
  // from the edge on each side: along the lines, and about the slot's axis
  // (Pappus) along the half circles.
  const double section = (1 - kPi / 4) * rr * rr;
  const double centroid = rr * (10 - 3 * kPi) / (3 * (4 - kPi));
  const double rounded = section * (2 * len + 2 * kPi * (ri + centroid));
  // A chamfer of 0.5 takes off rr^2 / 2, its centroid rr / 3 from the edge.
  const double bevelled = rr * rr / 2 * (2 * len + 2 * kPi * (ri + rr / 3));
  for (const bool chamfer : {false, true}) {
    TopoDS_Shape result;
    if (chamfer) {
      BRepFilletAPI_MakeChamfer maker(body);
      for (const TopoDS_Edge& edge : rim) {
        if (maker.Contour(edge) == 0) {
          maker.Add(rr, edge);
        }
      }
      CHECK(maker.NbContours() == 1);
      CHECK(build(maker));
      if (!maker.IsDone()) {
        continue;
      }
      result = maker.Shape();
    } else {
      BRepFilletAPI_MakeFillet maker(body);
      for (const TopoDS_Edge& edge : rim) {
        maker.Add(rr, edge);
      }
      CHECK(maker.NbContours() == 1);
      CHECK(build(maker));
      if (!maker.IsDone()) {
        continue;
      }
      result = maker.Shape();
    }
    CHECK(BRepCheck_Analyzer(result).IsValid());
    CHECK_NEAR_TOL(volume_of(body) - volume_of(result), chamfer ? bevelled : rounded, 1e-7);
    // The ledge is gone: no plane face at z = 15 is left.
    int ledges = 0;
    for (TopExp_Explorer f(result, TopAbs_FACE); f.More(); f.Next()) {
      BRepAdaptor_Surface s(TopoDS::Face(f.Current()));
      if (s.GetType() == GeomAbs_Plane && std::abs(s.Plane().Location().Z() - 15) < 1e-7 &&
          std::abs(std::abs(s.Plane().Axis().Direction().Z()) - 1) < 1e-9) {
        ++ledges;
      }
    }
    CHECK(ledges == 0);
  }
}

// Which corner a fillet failed at, and why (patch 0330): a fin 0.5 thick
// standing on a block, its upright front edge rounded with 2, more than the
// fin is thick. The rounding's end at the fin's foot, where three sharp
// edges meet, is a rounding's end alone (PerformOneCorner), and fails
// with a message; no other vertex is faulty.
void test_which_corner_a_fillet_fails_at() {
  const TopoDS_Shape block = BRepPrimAPI_MakeBox(20, 10, 10).Shape();
  const TopoDS_Shape fin = BRepPrimAPI_MakeBox(gp_Pnt(10, 0, 10), gp_Pnt(10.5, 10, 15)).Shape();
  const TopoDS_Shape body = BRepAlgoAPI_Fuse(block, fin).Shape();
  const TopoDS_Edge front = box_edge(body, gp_Pnt(10, 0, 15), gp_Pnt(10, 0, 10));
  CHECK(!front.IsNull());
  if (front.IsNull()) {
    return;
  }
  BRepFilletAPI_MakeFillet maker(body);
  maker.Add(2.0, front);
  CHECK(!build(maker));
  CHECK(maker.Failure() == ChFi3d_Builder::Failure_Corner);
  CHECK(maker.NbFaultyVertices() == 1);
  if (maker.NbFaultyVertices() != 1) {
    return;
  }
  CHECK(BRep_Tool::Pnt(maker.FaultyVertex(1)).Distance(gp_Pnt(10, 0, 10)) < 1e-9);
  CHECK(maker.FailedCorner(1) == ChFi3d_Builder::Corner_End);
  CHECK(maker.FailedCornerBlends(1) == 1);
  CHECK(maker.FailedCornerEdges(1) == 3);
  CHECK(maker.FailedCornerMessage(1).Length() > 0);
  CHECK(maker.FailedCorner(2) == ChFi3d_Builder::Corner_None);
}

// The edges of the face of a shape in the plane y = 0 whose middles lie
// above z and right of x.
std::vector<TopoDS_Edge> front_edges(const TopoDS_Shape& shape, double x, double z) {
  std::vector<TopoDS_Edge> out;
  for (TopExp_Explorer f(shape, TopAbs_FACE); f.More(); f.Next()) {
    BRepAdaptor_Surface s(TopoDS::Face(f.Current()));
    if (s.GetType() != GeomAbs_Plane || std::abs(std::abs(s.Plane().Axis().Direction().Y()) - 1) > 1e-9 ||
        std::abs(s.Plane().Location().Y()) > 1e-9) {
      continue;
    }
    for (TopExp_Explorer e(f.Current(), TopAbs_EDGE); e.More(); e.Next()) {
      BRepAdaptor_Curve c(TopoDS::Edge(e.Current()));
      const gp_Pnt m = c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
      if (m.Z() > z && m.X() > x) {
        out.push_back(TopoDS::Edge(e.Current()));
      }
    }
  }
  return out;
}

// A block 10 x 20 x 3 on a plate 30 x 20 x 2, its top edge along the
// plate's middle rounded with 5, more than the block's side (3) is high
// (patch 0331): the ball touches the top and rolls on the side's edge with
// the plate, a line parallel to the top. Its centres lie on a line, the
// rounding is a cylinder of radius 5 about it, which OCCT built as a
// walked B-spline approximation: a rounding of the block's front edges
// after it (the top's and the arc's, tangent to each other) failed. The
// rounding is now the cylinder (its edge with the plate not tangent, 0350);
// the front edges round as on the same body made exactly by a cut.
void test_rounding_rolled_on_a_straight_edge() {
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(0, 0, 0), gp_Pnt(30, 20, 2)).Shape();
  const TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(0, 0, 2), gp_Pnt(10, 20, 5)).Shape();
  BRepAlgoAPI_Fuse fuse(plate, block);
  fuse.SimplifyResult();
  const TopoDS_Shape body = fuse.Shape();
  const TopoDS_Edge top = box_edge(body, gp_Pnt(10, 0, 5), gp_Pnt(10, 20, 5));
  CHECK(!top.IsNull());
  if (top.IsNull()) {
    return;
  }
  const double r = 5;
  BRepFilletAPI_MakeFillet maker(body);
  maker.Add(r, top);
  CHECK(build(maker));
  if (!maker.IsDone()) {
    return;
  }
  const TopoDS_Shape rounded = maker.Shape();
  CHECK(BRepCheck_Analyzer(rounded).IsValid());
  // The section: centre (cx, cz), from the top (angle pi/2) to the edge
  // (10, 2) (angle a).
  const double cx = 10 - std::sqrt(r * r - 4), cz = 5 - r;
  const double a = std::atan2(2 - cz, 10 - cx);
  // The region the rounding takes away: the polygon (cx, 5), (10, 5),
  // (10, 2), (cx, cz) less the sector of the circle between its contacts.
  const double px[] = {cx, 10, 10, cx}, pz[] = {5, 5, 2, cz};
  double polygon = 0;
  for (int i = 0; i < 4; i++) {
    polygon += px[i] * pz[(i + 1) % 4] - px[(i + 1) % 4] * pz[i];
  }
  polygon = std::abs(polygon) / 2;
  const double section = polygon - 0.5 * r * r * (kPi / 2 - a);
  CHECK_NEAR_TOL(volume_of(body) - volume_of(rounded), section * 20, 1e-7);
  int cylinders = 0, others = 0;
  for (TopExp_Explorer f(rounded, TopAbs_FACE); f.More(); f.Next()) {
    BRepAdaptor_Surface s(TopoDS::Face(f.Current()));
    if (s.GetType() == GeomAbs_Cylinder) {
      ++cylinders;
      CHECK_NEAR_TOL(s.Cylinder().Radius(), r, 1e-12);
      CHECK(s.Cylinder().Axis().Direction().IsParallel(gp::DY(), 1e-12) &&
            gp_Lin(s.Cylinder().Axis()).Distance(gp_Pnt(cx, 0, cz)) < 1e-9);
    } else if (s.GetType() != GeomAbs_Plane) {
      ++others;
    }
  }
  CHECK(cylinders == 1 && others == 0);
  // The same body made exactly: the section taken away by a prism.
  BRepBuilderAPI_MakeWire wire;
  wire.Add(BRepBuilderAPI_MakeEdge(gp_Pnt(cx, -1, 5), gp_Pnt(10, -1, 5)).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(gp_Pnt(10, -1, 5), gp_Pnt(10, -1, 2)).Edge());
  const double am = 0.5 * (kPi / 2 + a);
  wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(gp_Pnt(10, -1, 2),
                                                      gp_Pnt(cx + r * std::cos(am), -1, cz + r * std::sin(am)),
                                                      gp_Pnt(cx, -1, 5))
                                       .Value())
               .Edge());
  const TopoDS_Shape tool =
      BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(wire.Wire()).Face(), gp_Vec(0, 22, 0)).Shape();
  BRepAlgoAPI_Cut cut(body, tool);
  cut.SimplifyResult();
  const TopoDS_Shape exact = cut.Shape();
  CHECK_NEAR_TOL(volume_of(exact), volume_of(rounded), 1e-7);
  // The front edges of both rounded with 1: one contour of the two edges,
  // the same volume.
  double taken[2] = {0, 0};
  for (int k = 0; k < 2; k++) {
    const TopoDS_Shape& shape = k == 0 ? rounded : exact;
    BRepFilletAPI_MakeFillet front(shape);
    const std::vector<TopoDS_Edge> edges = front_edges(shape, 0.5, 2.5);
    CHECK(edges.size() == 2);
    for (const TopoDS_Edge& e : edges) {
      front.Add(1.0, e);
    }
    CHECK(front.NbContours() == 1 && front.NbEdges(1) == 2);
    CHECK(build(front));
    if (!front.IsDone()) {
      return;
    }
    CHECK(BRepCheck_Analyzer(front.Shape()).IsValid());
    taken[k] = volume_of(shape) - volume_of(front.Shape());
  }
  CHECK_NEAR_TOL(taken[0], taken[1], 1e-5);
}

// The edge of a shape whose middle lies nearest to p.
TopoDS_Edge edge_nearest(const TopoDS_Shape& shape, const gp_Pnt& p) {
  TopoDS_Edge best;
  double nearest = 1e100;
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    BRepAdaptor_Curve c(edge);
    const double d = c.Value(0.5 * (c.FirstParameter() + c.LastParameter())).Distance(p);
    if (d < nearest) {
      nearest = d;
      best = edge;
    }
  }
  return best;
}

// A fin 1.5 thick standing 5 out from a block's side, its top and bottom
// flush with the block's, its outer top and bottom edges rounded with 2;
// the edges round one of its sides rounded with 0.75 (a tangent chain from
// the block over the top, down the outer end and back along the bottom),
// then those round the other side with 0.75, rounding the fin off (patch
// 0322). The second rounding's contacts on the top, the end and the bottom
// run along the whole edges the first one left there, up to the block.
// OCCT made the stripe's ends there the edges' vertices but kept the
// indices of its own points at them, then read the DS's shapes of those
// indices (the solid and a face) as vertices: with its exceptions off it
// wrote past their ends, and failed with TopoDS_Builder::Add (mitcad#331).
void test_fin_sides_rounded_one_after_the_other() {
  const double t = 1.5, r = 0.75, R = 2.0;
  const TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(-20, -20, 0), gp_Pnt(20, 0, 40)).Shape();
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(-t / 2, 0, 0), gp_Pnt(t / 2, 5, 40)).Shape();
  const TopoDS_Shape body = BRepAlgoAPI_Fuse(block, plate).Shape();
  BRepFilletAPI_MakeFillet ends(body);
  ends.Add(R, edge_nearest(body, gp_Pnt(0, 5, 40)));
  ends.Add(R, edge_nearest(body, gp_Pnt(0, 5, 0)));
  CHECK(build(ends));
  if (!ends.IsDone()) {
    return;
  }
  // The section a rounding takes off a right-angled convex edge and its
  // centroid's distance from each face; along the outer arcs (Pappus) it
  // turns about their axes at R less that distance.
  const double section = (1 - kPi / 4) * r * r;
  const double centroid = r * (10 - 3 * kPi) / (3 * (4 - kPi));
  const double each = section * (3 + 36 + 3 + kPi * (R - centroid));
  TopoDS_Shape shape = ends.Shape();
  for (const double x : {t / 2, -t / 2}) {
    BRepFilletAPI_MakeFillet side(shape);
    side.Add(r, edge_nearest(shape, gp_Pnt(x, 1.5, 40)));
    CHECK(side.NbContours() == 1);
    CHECK(side.NbEdges(1) == 5);
    CHECK(build(side));
    if (!side.IsDone()) {
      return;
    }
    CHECK(BRepCheck_Analyzer(side.Shape()).IsValid());
    CHECK_NEAR_TOL(volume_of(shape) - volume_of(side.Shape()), each, 1e-6);
    shape = side.Shape();
  }
  // The fin's top is gone: no plane face at z = 40 is left on it.
  int tops = 0;
  for (TopExp_Explorer f(shape, TopAbs_FACE); f.More(); f.Next()) {
    BRepAdaptor_Surface s(TopoDS::Face(f.Current()));
    GProp_GProps props;
    BRepGProp::SurfaceProperties(f.Current(), props);
    if (s.GetType() == GeomAbs_Plane && std::abs(props.CentreOfMass().Z() - 40) < 1e-7 &&
        props.CentreOfMass().Y() > 0) {
      ++tops;
    }
  }
  CHECK(tops == 0);
}

// The end circle of a cylinder of radius 12, built as a boss 5 high joined
// with one 3 high on it whose seam lies `angle` from the first one's
// (patch 0420, mitcad#337). The face merge leaves one cylindrical face
// whose seam runs in steps (up the first seam, along the circle between
// the parts, up the second); `circle_seam` tells whether it does.
TopoDS_Shape stepped_boss(double angle, bool& circle_seam) {
  const gp_Ax2 lower(gp_Pnt(0, 0, 0), gp::DZ(), gp::DX());
  const gp_Ax2 upper(gp_Pnt(0, 0, 5), gp::DZ(), gp_Dir(std::cos(angle), std::sin(angle), 0));
  BRepAlgoAPI_Fuse fuse(BRepPrimAPI_MakeCylinder(lower, 12, 5).Shape(),
                        BRepPrimAPI_MakeCylinder(upper, 12, 3).Shape());
  fuse.SimplifyResult();
  circle_seam = false;
  int cylinders = 0;
  for (TopExp_Explorer f(fuse.Shape(), TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    if (BRepAdaptor_Surface(face).GetType() != GeomAbs_Cylinder) {
      continue;
    }
    ++cylinders;
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
      const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
      circle_seam = circle_seam || (BRep_Tool::IsClosed(edge, face) &&
                                    BRepAdaptor_Curve(edge).GetType() == GeomAbs_Circle);
    }
  }
  circle_seam = circle_seam && cylinders == 1;
  return fuse.Shape();
}

// The boss's top circle rounded with 7: the torus's contact on the
// cylinder lies 7 below the top, across the step of the seam. Where the
// second seam lies at a smaller angle than the first, the part the circle
// lies on extends before the first seam in the face's parameters, and
// OCCT's split of the analytic blend into the face's domain put its piece
// a period off the blend's parameters: the stripe failed in the walk.
void test_boss_rim_rounded_across_a_stepped_seam() {
  bool straight_steps = false;
  const TopoDS_Shape straight = stepped_boss(0.0, straight_steps);
  CHECK(!straight_steps);
  BRepFilletAPI_MakeFillet reference(straight);
  reference.Add(7, edge_nearest(straight, gp_Pnt(-12, 0, 8)));
  CHECK(build(reference));
  if (!reference.IsDone()) {
    return;
  }
  const double removed = volume_of(straight) - volume_of(reference.Shape());
  for (const double angle : {-0.7, -2.5, 4.0, 0.7, 2.5}) {
    bool steps = false;
    const TopoDS_Shape body = stepped_boss(angle, steps);
    CHECK(steps);
    BRepFilletAPI_MakeFillet maker(body);
    maker.Add(7, edge_nearest(body, gp_Pnt(-12 * std::cos(angle), -12 * std::sin(angle), 8)));
    CHECK(maker.NbContours() == 1);
    CHECK(build(maker));
    if (!maker.IsDone()) {
      std::fprintf(stderr, "  (seams %g apart)\n", angle);
      continue;
    }
    CHECK(BRepCheck_Analyzer(maker.Shape()).IsValid());
    CHECK_NEAR_TOL(volume_of(body) - volume_of(maker.Shape()), removed, 1e-6);
  }
}

// A cylinder of radius 10 (its seam at the angle given) cut on top by two
// planes, z = 15 + 0.3 y for y > 0 and z = 15 - 0.1 y for y < 0: the top's
// edge with the cylinder is two arcs meeting where y = 0.
TopoDS_Shape ridged_cylinder(double seam) {
  const TopoDS_Shape cylinder =
      BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1), gp_Dir(std::cos(seam), std::sin(seam), 0)),
                               10, 30)
          .Shape();
  const auto above = [](double slope, bool positive) {
    gp_Trsf tilt;
    tilt.SetRotation(gp_Ax1(gp_Pnt(0, 0, 15), gp_Dir(1, 0, 0)), std::atan(slope));
    return BRepBuilderAPI_Transform(
               BRepPrimAPI_MakeBox(gp_Pnt(-20, positive ? 0 : -20, 15), gp_Pnt(20, positive ? 20 : 0, 40)).Shape(),
               tilt, true)
        .Shape();
  };
  const TopoDS_Shape once = BRepAlgoAPI_Cut(cylinder, above(0.3, true)).Shape();
  BRepAlgoAPI_Cut twice(once, above(-0.1, false));
  twice.SimplifyResult();
  return twice.Shape();
}

// The arc for y < 0 rounded with 1 (patch 0651): its end at y = 0, x = 10
// is where the cylinder's seam lies, and the rounding's walk past that end
// (for the corner there) starts on the seam, the cylinder's contact leaving
// the face across it at the first step. It now starts on the seam's other
// side, a period on, and the stripe is computed; before, the walk could not
// take a step and the stripe failed. (The corner at that vertex, where the
// seam passes, stays OCCT's limitation, as it is where the seam lies a
// little way past the vertex; with the seam elsewhere the rounding is made.)
void test_rounding_walked_past_its_end_across_a_seam() {
  const TopoDS_Shape body = ridged_cylinder(0.0);
  BRepFilletAPI_MakeFillet maker(body);
  maker.Add(1.0, edge_nearest(body, gp_Pnt(0, -10, 16)));
  CHECK(maker.NbContours() == 1);
  build(maker);
  CHECK(maker.NbFaultyContours() == 0);
  // The same rounding with the seam away from the end.
  const TopoDS_Shape turned = ridged_cylinder(kPi / 6);
  BRepFilletAPI_MakeFillet reference(turned);
  reference.Add(1.0, edge_nearest(turned, gp_Pnt(0, -10, 16)));
  CHECK(build(reference));
  CHECK(reference.IsDone() && BRepCheck_Analyzer(reference.Shape()).IsValid());
}

} // namespace

void fillet_kernel_tests() {
  guarded("test_contour_closed_at_a_sharp_corner", test_contour_closed_at_a_sharp_corner);
  guarded("test_ridge_between_faces_at_a_small_angle", test_ridge_between_faces_at_a_small_angle);
  guarded("test_where_a_fillet_stops", test_where_a_fillet_stops);
  guarded("test_rim_rounded_as_wide_as_its_ledge", test_rim_rounded_as_wide_as_its_ledge);
  guarded("test_fin_sides_rounded_one_after_the_other", test_fin_sides_rounded_one_after_the_other);
  guarded("test_which_corner_a_fillet_fails_at", test_which_corner_a_fillet_fails_at);
  guarded("test_rounding_rolled_on_a_straight_edge", test_rounding_rolled_on_a_straight_edge);
  guarded("test_boss_rim_rounded_across_a_stepped_seam", test_boss_rim_rounded_across_a_stepped_seam);
  guarded("test_rounding_walked_past_its_end_across_a_seam", test_rounding_walked_past_its_end_across_a_seam);
}

} // namespace test
