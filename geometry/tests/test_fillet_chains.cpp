// SPDX-License-Identifier: MIT
// Fillet contours that stop where their edges turn although the faces go on
// tangentially across a shallow crease, built here with OCCT's fillet alone
// on synthetic shapes: the case behind the patch 0180 of the OCCT port
// (third_party/vcpkg-ports, mitcad#241); and Mitcad's fillets, which use it.

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <string>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRep_Tool.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GProp_GProps.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_Map.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <gp_Ax1.hxx>
#include <gp_Ax2.hxx>
#include <gp_Trsf.hxx>

#include "../src/util.hpp"
#include "check.hpp"
#include "mitcad/geometry/dressup.hpp"
#include "mitcad/geometry/import.hpp"
#include "mitcad/geometry/tangent_chain.hpp"

namespace test {
namespace {

using mitcad::geometry::FilletSet;
using mitcad::geometry::import_body;

// The volume, integrated to a relative error of 1e-9.
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
    std::fprintf(stderr, "fillet chain test: %s\n", failure.what());
    return false;
  }
  return op.IsDone();
}

TopoDS_Wire square(double half, double z) {
  BRepBuilderAPI_MakePolygon p(gp_Pnt(-half, -half, z), gp_Pnt(half, -half, z), gp_Pnt(half, half, z),
                               gp_Pnt(-half, half, z), true);
  return p.Wire();
}

constexpr double kHalf = 5.5;
const double kDraft = std::tan(3.0 * kPi / 180.0);

// The planar face through the points, in order.
TopoDS_Face polygon(const std::vector<gp_Pnt>& points) {
  BRepBuilderAPI_MakePolygon p;
  for (const gp_Pnt& point : points) {
    p.Add(point);
  }
  p.Close();
  return BRepBuilderAPI_MakeFace(p.Wire(), true).Face();
}

// A square post 11 wide whose walls are vertical from z = -7 to z = -4 and
// drafted outwards by 3 degrees from there to z = 0, all faces planes. OCCT
// takes the faces across the 3-degree creases at z = -4 for tangent, so the
// post's corner edges below and above a crease, which turn there by 4.2
// degrees, make one contour.
TopoDS_Shape drafted_post() {
  const double top = kHalf + 4.0 * kDraft;
  const double sx[4] = {-1, 1, 1, -1};
  const double sy[4] = {-1, -1, 1, 1};
  std::vector<gp_Pnt> low, mid, high;
  for (int i = 0; i < 4; ++i) {
    low.emplace_back(sx[i] * kHalf, sy[i] * kHalf, -7.0);
    mid.emplace_back(sx[i] * kHalf, sy[i] * kHalf, -4.0);
    high.emplace_back(sx[i] * top, sy[i] * top, 0.0);
  }
  BRepBuilderAPI_Sewing sewing;
  sewing.Add(polygon({low[3], low[2], low[1], low[0]}));
  sewing.Add(polygon(high));
  for (int i = 0; i < 4; ++i) {
    const int j = (i + 1) % 4;
    sewing.Add(polygon({low[i], low[j], mid[j], mid[i]}));
    sewing.Add(polygon({mid[i], mid[j], high[j], high[i]}));
  }
  sewing.Perform();
  BRepBuilderAPI_MakeSolid solid(TopoDS::Shell(sewing.SewedShape()));
  TopoDS_Solid post = solid.Solid();
  BRepLib::OrientClosedSolid(post);
  return post;
}

// The edge of a shape from (x, y, z0) to (x, y, z1), either way round.
bool is_vertical_edge(const TopoDS_Edge& edge, double x, double y, double z0, double z1) {
  const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
  const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
  const auto at = [&](const gp_Pnt& p, double z) {
    return std::abs(p.X() - x) < 1e-9 && std::abs(p.Y() - y) < 1e-9 && std::abs(p.Z() - z) < 1e-9;
  };
  return (at(a, z0) && at(b, z1)) || (at(a, z1) && at(b, z0));
}

// What a rounding of radius r of the post's lower corner edge at
// (-5.5, -5.5) removes when it stops at the crease and the drafted walls
// cut it: the corner outside the rounding's cylinder, less what lies within
// the drafted walls extended downwards.
double drafted_corner_removal(double r) {
  BRepOffsetAPI_ThruSections extended(true, true);
  extended.AddWire(square(kHalf - 4.0 * kDraft, -8.0));
  extended.AddWire(square(kHalf + 4.0 * kDraft, 0.0));
  extended.Build();
  const TopoDS_Shape corner =
    BRepPrimAPI_MakeBox(gp_Pnt(-kHalf, -kHalf, -7.0), gp_Pnt(-kHalf + r, -kHalf + r, -4.0)).Shape();
  const TopoDS_Shape cylinder =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(-kHalf + r, -kHalf + r, -8.0), gp::DZ()), r, 10.0).Shape();
  const TopoDS_Shape outside = BRepAlgoAPI_Cut(corner, cylinder).Shape();
  return volume_of(BRepAlgoAPI_Cut(outside, extended.Shape()).Shape());
}

// The post's lower corner edge rounded with radius 0.4. By default OCCT's
// contour runs on over the drafted corner edge above the crease. With
// StopChainsAtTurns it stops at the crease, and the drafted walls, extended
// downwards, cut the rounding there: as the rounding of exactly that edge
// in the .f3d files.
void test_contour_stops_at_a_drafted_crease() {
  const TopoDS_Shape post = drafted_post();
  const double r = 0.4;
  for (const bool stop : {false, true}) {
    BRepFilletAPI_MakeFillet fillet(post);
    if (stop) {
      fillet.StopChainsAtTurns(NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher>());
    }
    for (TopExp_Explorer e(post, TopAbs_EDGE); e.More(); e.Next()) {
      if (is_vertical_edge(TopoDS::Edge(e.Current()), -kHalf, -kHalf, -7.0, -4.0)) {
        fillet.Add(r, TopoDS::Edge(e.Current()));
      }
    }
    CHECK(fillet.NbContours() == 1);
    CHECK(fillet.NbEdges(1) == (stop ? 1 : 2));
    if (!stop) {
      continue;
    }
    CHECK(fillet.ChainTurnStops().Extent() == 1); // at the crease, not at the post's foot
    CHECK(build(fillet));
    if (!fillet.IsDone()) {
      continue;
    }
    CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
    CHECK(face_count(fillet.Shape()) == face_count(post) + 1);
    CHECK_NEAR_TOL(volume_of(post) - volume_of(fillet.Shape()), drafted_corner_removal(r), 1e-5);
  }
}

// An edge selected with the one it turns into stays in one contour.
void test_selected_edges_past_a_turn_stay_one_contour() {
  const TopoDS_Shape post = drafted_post();
  NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> selected;
  TopoDS_Edge lower;
  for (TopExp_Explorer e(post, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    if (is_vertical_edge(edge, -kHalf, -kHalf, -7.0, -4.0)) {
      lower = edge;
      selected.Add(edge);
    }
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if (std::abs(a.X() - a.Y()) < 1e-9 && std::abs(b.X() - b.Y()) < 1e-9 && a.X() < 0 && b.X() < 0 &&
        std::abs(a.Z() - b.Z()) > 3.9 && std::max(a.Z(), b.Z()) > -1e-9) {
      selected.Add(edge); // the drafted corner edge above it
    }
  }
  CHECK(selected.Extent() == 2);
  BRepFilletAPI_MakeFillet fillet(post);
  fillet.StopChainsAtTurns(selected);
  fillet.Add(0.4, lower);
  CHECK(fillet.NbContours() == 1);
  CHECK(fillet.NbEdges(1) == 2);
}

// The post with a vertical slot of radius 2.75 cut into its -y wall (the
// slot's axis 1.5 outside the wall), and the slot's edge up the vertical
// wall at x > 0 rounded with radius 0.4: the wall ends at the crease, the
// slot's face runs on past it, and the contour stops there. OCCT's end is a
// corner patch between the rounding, the slot and the drafted wall, built
// valid (the analytic blend's end is only taken as ending on the crease in
// SplitKPart; a crease that ended every walk there turned this solid
// inside out). It removes about the corner below the crease, not the
// default chain's rounding up the drafted wall too.
void test_contour_stops_where_one_face_runs_on() {
  const double radius = 0.4;
  const double slot = 2.75;
  const double offset = 1.5;
  const TopoDS_Shape cutter =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, -kHalf - offset, -8.0), gp::DZ()), slot, 10.0).Shape();
  const TopoDS_Shape body = BRepAlgoAPI_Cut(drafted_post(), cutter).Shape();
  const double x = std::sqrt(slot * slot - offset * offset);
  // The corner below the crease that the rounding takes away: the region
  // between the wall, the slot and the rounding's cylinder.
  const double yf = -kHalf + radius;
  const double xf = std::sqrt((slot + radius) * (slot + radius) - (yf + kHalf + offset) * (yf + kHalf + offset));
  const double k = slot / (slot + radius);
  const TopoDS_Face quad = polygon({gp_Pnt(x, -kHalf, -7.0), gp_Pnt(xf, -kHalf, -7.0), gp_Pnt(xf, yf, -7.0),
                                    gp_Pnt(xf * k, -kHalf - offset + (yf + kHalf + offset) * k, -7.0)});
  TopoDS_Shape corner = BRepPrimAPI_MakePrism(quad, gp_Vec(0, 0, 3.0)).Shape();
  corner = BRepAlgoAPI_Cut(corner,
                           BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(xf, yf, -8.0), gp::DZ()), radius, 10.0).Shape())
             .Shape();
  const double below = volume_of(BRepAlgoAPI_Cut(corner, cutter).Shape());
  for (const bool stop : {false, true}) {
    BRepFilletAPI_MakeFillet fillet(body);
    if (stop) {
      fillet.StopChainsAtTurns(NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher>());
    }
    for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
      if (is_vertical_edge(TopoDS::Edge(e.Current()), x, -kHalf, -7.0, -4.0)) {
        fillet.Add(radius, TopoDS::Edge(e.Current()));
      }
    }
    CHECK(fillet.NbContours() == 1);
    CHECK(fillet.NbEdges(1) == (stop ? 1 : 2));
    CHECK(build(fillet));
    if (!fillet.IsDone()) {
      continue;
    }
    CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
    const double removed = volume_of(body) - volume_of(fillet.Shape());
    if (stop) {
      CHECK_NEAR_TOL(removed, below, 0.05 * below); // within 5 % (near() is absolute below 1)
    } else {
      CHECK(removed > 2 * below);
    }
  }
}

constexpr double kRing = 15.0;
const double kCrease = 20.0 * kPi / 180.0;
constexpr double kPlate = 5.0;

// A plate 5 thick whose side is an arc of radius 15 about the z axis from
// 20 to 160 degrees, joined at each end by an arc of radius 1 that turns
// from it by `turn` radians: outwards (a concave crease, the plate's side
// bulging just past the crease before it rounds off) for a positive turn.
// Beyond the small arcs the side runs straight down to y = -5. OCCT takes
// the big and the small arcs' faces for tangent across the vertical creases.
struct Ring {
  TopoDS_Shape body;
  gp_Pnt small_centre; // of the small arc at 20 degrees; the other mirrors it
};

Ring ring_with_creases(double turn) {
  const gp_Pnt p(kRing * std::cos(kCrease), kRing * std::sin(kCrease), 0.0);
  const double a = kCrease + turn;
  const gp_Pnt c(p.X() - std::cos(a), p.Y() - std::sin(a), 0.0);
  const gp_Pnt q(c.X() + 1.0, c.Y(), 0.0);
  const double half_way = 0.5 * std::atan2(p.Y() - c.Y(), p.X() - c.X());
  const gp_Pnt m(c.X() + std::cos(half_way), c.Y() + std::sin(half_way), 0.0);
  const auto mirrored = [](const gp_Pnt& x) { return gp_Pnt(-x.X(), x.Y(), x.Z()); };
  BRepBuilderAPI_MakeWire wire;
  wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(mirrored(p), gp_Pnt(0, kRing, 0), p).Value()).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(p, m, q).Value()).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(q, gp_Pnt(q.X(), -5.0, 0.0)).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(gp_Pnt(q.X(), -5.0, 0.0), gp_Pnt(-q.X(), -5.0, 0.0)).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(gp_Pnt(-q.X(), -5.0, 0.0), mirrored(q)).Edge());
  wire.Add(
    BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(mirrored(q), mirrored(m), mirrored(p)).Value()).Edge());
  const TopoDS_Face face = BRepBuilderAPI_MakeFace(wire.Wire(), true).Face();
  return {BRepPrimAPI_MakePrism(face, gp_Vec(0, 0, kPlate)).Shape(), c};
}

// What a rounding of radius r of the big arc's top edge removes when it
// stops at the creases and the small arcs' cylinders cut it: the edge's
// corner (a square less a quarter disc) turned about the axis over the big
// arc's angles, less what lies within the small arcs' cylinders.
double ring_corner_removal(const Ring& ring, double r) {
  BRepBuilderAPI_MakeWire profile;
  const gp_Pnt a(kRing - r, 0.0, kPlate);
  const gp_Pnt b(kRing, 0.0, kPlate);
  const gp_Pnt c(kRing, 0.0, kPlate - r);
  const gp_Pnt mid(kRing - r + r * std::cos(kPi / 4), 0.0, kPlate - r + r * std::sin(kPi / 4));
  profile.Add(BRepBuilderAPI_MakeEdge(a, b).Edge());
  profile.Add(BRepBuilderAPI_MakeEdge(b, c).Edge());
  profile.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(c, mid, a).Value()).Edge());
  const TopoDS_Face section = BRepBuilderAPI_MakeFace(profile.Wire(), true).Face();
  const TopoDS_Shape turned =
    BRepPrimAPI_MakeRevol(section, gp_Ax1(gp::Origin(), gp::DZ()), kPi - 2 * kCrease).Shape();
  gp_Trsf rotation;
  rotation.SetRotation(gp_Ax1(gp::Origin(), gp::DZ()), kCrease);
  const TopoDS_Shape corner = BRepBuilderAPI_Transform(turned, rotation).Shape();
  const gp_Pnt& s = ring.small_centre;
  const TopoDS_Shape small1 =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(s.X(), s.Y(), -1.0), gp::DZ()), 1.0, kPlate + 2).Shape();
  const TopoDS_Shape small2 =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(-s.X(), s.Y(), -1.0), gp::DZ()), 1.0, kPlate + 2).Shape();
  const TopoDS_Shape removed = BRepAlgoAPI_Cut(BRepAlgoAPI_Cut(corner, small1).Shape(), small2).Shape();
  return volume_of(BRepAlgoAPI_Common(removed, ring.body).Shape());
}

// The big arc's top edge rounded with radius 1, the creases concave by 0.74
// and by 2.9 degrees. By default OCCT's contour runs on round the small arcs
// and down the straight sides' top edges. With StopChainsAtTurns it stops at
// the creases, and the small arcs' cylinders, extended over the big arc's
// side, cut the rounding (a torus, kept analytic) there.
void test_ring_contour_stops_at_a_concave_crease() {
  const double r = 1.0;
  for (const double turn : {0.0129, 0.05}) {
    const Ring ring = ring_with_creases(turn);
    for (const bool stop : {false, true}) {
      BRepFilletAPI_MakeFillet fillet(ring.body);
      if (stop) {
        fillet.StopChainsAtTurns(NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher>());
      }
      for (TopExp_Explorer e(ring.body, TopAbs_EDGE); e.More(); e.Next()) {
        const BRepAdaptor_Curve curve(TopoDS::Edge(e.Current()));
        const gp_Pnt m = curve.Value(0.5 * (curve.FirstParameter() + curve.LastParameter()));
        if (std::abs(m.Z() - kPlate) < 1e-9 && std::abs(m.X()) < 1e-6 && std::abs(m.Y() - kRing) < 1e-6) {
          fillet.Add(r, TopoDS::Edge(e.Current()));
        }
      }
      CHECK(fillet.NbContours() == 1);
      CHECK(fillet.NbEdges(1) == (stop ? 1 : 5));
      if (!stop) {
        continue;
      }
      CHECK(build(fillet));
      if (!fillet.IsDone()) {
        continue;
      }
      CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
      CHECK(face_count(fillet.Shape()) == face_count(ring.body) + 1);
      CHECK_NEAR_TOL(volume_of(ring.body) - volume_of(fillet.Shape()), ring_corner_removal(ring, r), 1e-5);
      int tori = 0;
      for (TopExp_Explorer f(fillet.Shape(), TopAbs_FACE); f.More(); f.Next()) {
        tori += BRepAdaptor_Surface(TopoDS::Face(f.Current())).GetType() == GeomAbs_Torus ? 1 : 0;
      }
      CHECK(tori == 1);
    }
  }
}

// Mitcad's fillet of the post's lower corner edge: with its tangent chain it
// rounds that edge alone (the drafted edge above turns from it), and so it
// does without; before, the first rounded the drafted edge too and the
// second was unsupported.
void test_mitcad_fillet_stops_at_a_drafted_crease() {
  const ShapePtr post = import_body(drafted_post(), "F1", 0);
  std::string lower;
  int lower_index = -1;
  for (int i = 0; i < post->edge_count(); ++i) {
    if (is_vertical_edge(post->edge(i), -kHalf, -kHalf, -7.0, -4.0)) {
      lower = post->edge_name(i);
      lower_index = i;
    }
  }
  CHECK(!lower.empty());
  // The tangent chain the hover shows (mitcad#290) stops at the crease too:
  // the edge is in no chain.
  for (const std::vector<int>& chain : mitcad::geometry::edge_tangent_chains(*post)) {
    CHECK(std::find(chain.begin(), chain.end(), lower_index) == chain.end());
  }
  const double expected = volume_of(post->occt()) - drafted_corner_removal(0.4);
  for (const bool chain : {true, false}) {
    FilletSet set;
    set.edges = {lower};
    set.radius = 0.4;
    set.tangent_chain = chain;
    const ShapePtr rounded = mitcad::geometry::fillet("F2", *post, std::vector<FilletSet>{set});
    CHECK(mitcad::geometry::detail::is_valid(rounded->occt()));
    CHECK(rounded->notes().empty());
    CHECK(faces_named(*rounded, "F2:fillet(" + lower + ")") == 1);
    CHECK(rounded->face_count() == post->face_count() + 1);
    CHECK_NEAR_TOL(volume_of(rounded->occt()), expected, 1e-7);
    // The drafted corner edge above the crease, which the drafted walls'
    // growth prolongs down to the rounding, is one edge: no vertex is left
    // where the contour stopped.
    bool vertex_left = false;
    for (TopExp_Explorer v(rounded->occt(), TopAbs_VERTEX); v.More(); v.Next()) {
      const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(v.Current()));
      vertex_left = vertex_left || p.Distance(gp_Pnt(-kHalf, -kHalf, -4.0)) < 1e-6;
    }
    CHECK(!vertex_left);
    CHECK(edge_names_unique(*rounded));
  }
}

} // namespace

void fillet_chain_tests() {
  guarded("test_contour_stops_at_a_drafted_crease", test_contour_stops_at_a_drafted_crease);
  guarded("test_selected_edges_past_a_turn_stay_one_contour", test_selected_edges_past_a_turn_stay_one_contour);
  guarded("test_contour_stops_where_one_face_runs_on", test_contour_stops_where_one_face_runs_on);
  guarded("test_ring_contour_stops_at_a_concave_crease", test_ring_contour_stops_at_a_concave_crease);
  guarded("test_mitcad_fillet_stops_at_a_drafted_crease", test_mitcad_fillet_stops_at_a_drafted_crease);
}

} // namespace test
