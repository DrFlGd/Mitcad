// SPDX-License-Identifier: MIT
// Roundings whose result OCCT's topological build made invalid, or whose
// computation failed, rebuilt with the port's patches 0150 and later
// (third_party/vcpkg-ports/opencascade, mitcad#239). OCCT alone on plain
// primitives; the volumes come from exact cross-sections.

#include <cmath>
#include <utility>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_NurbsConvert.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Ax2.hxx>
#include <gp_Vec.hxx>

#include "check.hpp"

namespace test {
namespace {

double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props);
  return props.Mass();
}

// The volume integrated adaptively (OCCT's default integration errs on
// the spline faces of walked roundings).
double exact_volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props, 1e-9, true);
  return props.Mass();
}

int faces_of(const TopoDS_Shape& shape) {
  int n = 0;
  for (TopExp_Explorer e(shape, TopAbs_FACE); e.More(); e.Next()) {
    ++n;
  }
  return n;
}

// The edge of a shape whose middle lies nearest to p.
TopoDS_Edge edge_near(const TopoDS_Shape& shape, const gp_Pnt& p) {
  TopoDS_Edge best;
  double nearest = 1e100;
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    if (BRep_Tool::Degenerated(edge)) {
      continue;
    }
    BRepAdaptor_Curve curve(edge);
    const double d = curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2).Distance(p);
    if (d < nearest) {
      nearest = d;
      best = edge;
    }
  }
  return best;
}

// OCCT's fillet of the edges nearest to the points, one radius; the result,
// valid, or a null shape.
TopoDS_Shape occt_fillet(const TopoDS_Shape& shape, const std::vector<gp_Pnt>& near_edges, double radius) {
  BRepFilletAPI_MakeFillet maker(shape);
  for (const gp_Pnt& p : near_edges) {
    maker.Add(radius, edge_near(shape, p));
  }
  maker.Build();
  if (!maker.IsDone() || !BRepCheck_Analyzer(maker.Shape()).IsValid()) {
    return TopoDS_Shape();
  }
  return maker.Shape();
}

// The cross-section a rounding of radius r removes at a straight edge where
// a plane meets a cylinder of radius R, the cylinder's axis parallel to the
// edge at distance h from the plane, the material inside the cylinder on
// the far side of the plane from the axis (a flat cut off a round bar): the
// region between the plane, the circle and the ball.
double flat_on_round_loss(double R, double h, double r) {
  // In the section: the axis at the origin, the plane at z = h, the
  // material above it. The ball's centre at z = h + r, R - r from the axis.
  const double zc = h + r;
  const double xc = std::sqrt((R - r) * (R - r) - zc * zc);
  const double xe = std::sqrt(R * R - h * h); // the edge
  const double s = R / (R - r);               // the contact on the circle
  const double t2x = xc * s, t2z = zc * s;
  // The polygon centre - contact on the plane - edge - contact on the circle.
  const double px[4] = {xc, xc, xe, t2x}, pz[4] = {zc, h, h, t2z};
  double twice = 0.0;
  for (int i = 0; i < 4; ++i) {
    twice += px[i] * pz[(i + 1) % 4] - px[(i + 1) % 4] * pz[i];
  }
  const double polygon = 0.5 * std::abs(twice);
  // The circle's segment between the edge and the contact on it.
  const double theta = std::atan2(t2z, t2x) - std::atan2(h, xe);
  const double segment = 0.5 * R * R * (theta - std::sin(theta));
  // The ball's sector between its two contacts.
  const double phi = std::acos((h - zc) * (t2z - zc) / (r * r));
  return polygon + segment - 0.5 * r * r * phi;
}

void test_slot_end_beside_a_split_side() {
  // A bar of radius 18.75 along y (65 long) whose side is two faces, split
  // at y = -15: a slot 24 high cuts its end y in [-15, 0], so the slot's
  // back wall lies in the plane of the split. The slot's top and bottom
  // edges on the side rounded with 0.5: each rounding ends against the
  // back wall, its contact on the side on the split edge. The removed
  // section is constant along the 15 of each edge.
  const TopoDS_Shape far =
      BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, -65, 0), gp_Dir(0, 1, 0)), 18.75, 50).Shape();
  const TopoDS_Shape near =
      BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, -15, 0), gp_Dir(0, 1, 0)), 18.75, 15).Shape();
  const TopoDS_Shape bar = BRepAlgoAPI_Fuse(far, near).Shape();
  const TopoDS_Shape slot = BRepPrimAPI_MakeBox(gp_Pnt(-40, -15, -12), gp_Pnt(40, 1, 12)).Shape();
  const TopoDS_Shape body = BRepAlgoAPI_Cut(bar, slot).Shape();
  CHECK(BRepCheck_Analyzer(body).IsValid());
  const double x = std::sqrt(18.75 * 18.75 - 12.0 * 12.0);
  const double loss = flat_on_round_loss(18.75, 12.0, 0.5) * 15.0;
  // One edge, the two top edges, all four.
  const std::vector<std::vector<gp_Pnt>> sets = {
      {gp_Pnt(-x, -7.5, 12)},
      {gp_Pnt(-x, -7.5, 12), gp_Pnt(x, -7.5, 12)},
      {gp_Pnt(-x, -7.5, 12), gp_Pnt(x, -7.5, 12), gp_Pnt(-x, -7.5, -12), gp_Pnt(x, -7.5, -12)}};
  for (const std::vector<gp_Pnt>& edges : sets) {
    const TopoDS_Shape rounded = occt_fillet(body, edges, 0.5);
    CHECK(!rounded.IsNull());
    if (!rounded.IsNull()) {
      CHECK_NEAR_TOL(volume_of(rounded), volume_of(body) - loss * static_cast<double>(edges.size()), 1e-9);
      CHECK(faces_of(rounded) == faces_of(body) + static_cast<int>(edges.size()));
    }
  }
}

// A prism of a closed polygon in the (x, y) plane, z from 0 to h.
TopoDS_Shape xy_prism(const std::vector<std::pair<double, double>>& xy, double h) {
  BRepBuilderAPI_MakePolygon polygon;
  for (const auto& [x, y] : xy) {
    polygon.Add(gp_Pnt(x, y, 0));
  }
  polygon.Close();
  const TopoDS_Face face = BRepBuilderAPI_MakeFace(polygon.Wire(), true).Face();
  return BRepPrimAPI_MakePrism(face, gp_Vec(0, 0, h)).Shape();
}

void test_rounding_ends_on_a_pads_split_foot() {
  // A block (x in [0, 20], 15 high) with a pad 0.5 thick on its side
  // x = 0, y in [-15, 15], its ends bevelled at 45 degrees, fused to it, so
  // that the pad's foot and top are faces of their own beside the block's
  // bottom and top. The concave edges where the bevels meet the side are
  // rounded with 30: the ball touches the side and rolls on the pad's front
  // edge (centre 30 in front of the side, sqrt(29.75) beyond the edge).
  // The rounding ends on the block's bottom edge and at the pad's bottom
  // vertex, faces of one plane split in two. The section filled is the
  // triangle side - bevel - contact less the circle's segment over the
  // chord from the contact to the pad's edge.
  const TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(0, -40, 0), gp_Pnt(20, 40, 15)).Shape();
  const TopoDS_Shape pad = xy_prism({{0, -15}, {0, 15}, {-0.5, 14.5}, {-0.5, -14.5}}, 15);
  const TopoDS_Shape body = BRepAlgoAPI_Fuse(block, pad).Shape();
  CHECK(BRepCheck_Analyzer(body).IsValid());
  const double c = std::sqrt(29.75);
  const double triangle = 0.5 * 0.5 * (c - 0.5);
  const double theta = 2.0 * std::asin(std::sqrt(0.25 + 29.75) / 60.0);
  const double fill = (triangle - 0.5 * 900.0 * (theta - std::sin(theta))) * 15.0;
  const std::vector<std::vector<gp_Pnt>> sets = {{gp_Pnt(0, 15, 7.5)}, {gp_Pnt(0, 15, 7.5), gp_Pnt(0, -15, 7.5)}};
  for (const std::vector<gp_Pnt>& edges : sets) {
    const TopoDS_Shape rounded = occt_fillet(body, edges, 30.0);
    CHECK(!rounded.IsNull());
    if (!rounded.IsNull()) {
      CHECK_NEAR_TOL(volume_of(rounded), volume_of(body) + fill * static_cast<double>(edges.size()), 1e-9);
      // Each bevel and the edge it shares with the pad's front gone, a
      // rounding added.
      CHECK(faces_of(rounded) == faces_of(body));
    }
  }
}

void test_closed_rounding_rolling_from_a_seam_vertex() {
  // A disc of radius 6.95, 1 high, under a 45-degree bevel to radius 5.95
  // and a neck, fused from a cylinder, a cone and a cylinder (their seams at
  // one angle). The disc's bottom edge rounded with 2: the side and the
  // bevel are too narrow, the ball rolls on the bevel's top edge (centre at
  // radius 3.95, 2 up) all round; the start of the closed rounding lies at
  // that edge's vertex, where the neck's seam starts too. The section
  // removed is the polygon foot - side - bevel less the circle's segment
  // over the quarter arc, revolved (Pappus).
  const gp_Dir up(0, 0, 1);
  const TopoDS_Shape disc = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 0), up), 6.95, 1).Shape();
  const TopoDS_Shape bevel = BRepPrimAPI_MakeCone(gp_Ax2(gp_Pnt(0, 0, 1), up), 6.95, 5.95, 1).Shape();
  const TopoDS_Shape neck = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 2), up), 5.95, 3).Shape();
  const TopoDS_Shape body = BRepAlgoAPI_Fuse(BRepAlgoAPI_Fuse(disc, bevel).Shape(), neck).Shape();
  CHECK(BRepCheck_Analyzer(body).IsValid());
  // The polygon (3.95, 0) (6.95, 0) (6.95, 1) (5.95, 2): area and moment
  // about the axis.
  const double pr[4] = {3.95, 6.95, 6.95, 5.95}, pz[4] = {0, 0, 1, 2};
  double twice = 0.0, moment6 = 0.0;
  for (int i = 0; i < 4; ++i) {
    const int j = (i + 1) % 4;
    const double cross = pr[i] * pz[j] - pr[j] * pz[i];
    twice += cross;
    moment6 += (pr[i] + pr[j]) * cross;
  }
  const double polygon_moment = moment6 / 6.0;
  // The segment of the circle of radius 2 round (3.95, 2) over the quarter
  // arc from (3.95, 0) to (5.95, 2).
  const double theta = kPi / 2.0;
  const double segment = 2.0 * (theta - std::sin(theta));
  const double d = 4.0 * 2.0 * std::pow(std::sin(theta / 2.0), 3) / (3.0 * (theta - std::sin(theta)));
  const double segment_moment = segment * (3.95 + d * std::sqrt(0.5));
  const double removed = 2.0 * kPi * (polygon_moment - segment_moment);
  CHECK(twice > 0.0);
  const TopoDS_Shape rounded = occt_fillet(body, {gp_Pnt(-6.95, 0, 0)}, 2.0);
  CHECK(!rounded.IsNull());
  if (!rounded.IsNull()) {
    CHECK_NEAR_TOL(exact_volume_of(rounded), exact_volume_of(body) - removed, 1e-7);
    // The side and the bevel gone, the rounding added.
    CHECK(faces_of(rounded) == faces_of(body) - 1);
  }
}

void test_plate_edges_rounded_to_a_full_round() {
  // A plate 1.5 thick, its two top edges rounded with 0.75: the contact
  // lines meet in the middle of the top, which vanishes; the two quarter
  // rounds make a half round. Each removes its quarter of the corner
  // square.
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(0, 0, 0), gp_Pnt(20, 1.5, 10)).Shape();
  const TopoDS_Shape rounded = occt_fillet(plate, {gp_Pnt(10, 0, 10), gp_Pnt(10, 1.5, 10)}, 0.75);
  CHECK(!rounded.IsNull());
  if (!rounded.IsNull()) {
    CHECK_NEAR_TOL(volume_of(rounded), 300.0 - 2.0 * fillet_loss(0.75) * 20.0, 1e-9);
    // The top gone, two roundings added.
    CHECK(faces_of(rounded) == faces_of(plate) + 1);
  }
}

// The faces of a shape that lie on the sphere of radius r about c (sampled
// 10 x 10 within their bounds, within tol), and their total area.
int faces_on_sphere(const TopoDS_Shape& shape, const gp_Pnt& c, double r, double tol, double& area) {
  int n = 0;
  area = 0.0;
  for (TopExp_Explorer e(shape, TopAbs_FACE); e.More(); e.Next()) {
    const TopoDS_Face& face = TopoDS::Face(e.Current());
    BRepAdaptor_Surface s(face);
    double u1 = 0.0, u2 = 0.0, v1 = 0.0, v2 = 0.0;
    BRepTools::UVBounds(face, u1, u2, v1, v2);
    bool on = true;
    for (int i = 0; i <= 10 && on; ++i) {
      for (int j = 0; j <= 10 && on; ++j) {
        on = std::abs(s.Value(u1 + (u2 - u1) * i / 10, v1 + (v2 - v1) * j / 10).Distance(c) - r) <= tol;
      }
    }
    if (on) {
      GProp_GProps props;
      BRepGProp::SurfaceProperties(face, props);
      area += props.Mass();
      ++n;
    }
  }
  return n;
}

// The material a rounding of radius r takes off (or adds) where it runs
// round a corner rounded with r: a quarter column r high less the ball's
// eighth.
double ball_corner_loss(double r) { return kPi * r * r * r / 4.0 - kPi * r * r * r / 6.0; }

void test_top_rounded_past_a_corner_of_the_balls_radius() {
  // A box 20 x 20 x 10 whose vertical edge at (20, 20) is rounded with 2,
  // stored as splines (so that the rounding is walked, not analytic); then
  // the top edges at that corner (one tangent chain: an edge, the arc, an
  // edge) rounded with 2. Round the arc the ball's centre stays on the
  // corner's axis, 2 below the top: its contact on the top stands still
  // while the other sweeps round the arc, and the blend is an eighth of the
  // ball's sphere. The two straight roundings take their sections along
  // 18 each, the corner a quarter column less the ball's eighth.
  const double r = 2.0;
  const TopoDS_Shape box = BRepPrimAPI_MakeBox(20, 20, 10).Shape();
  BRepFilletAPI_MakeFillet corner(box);
  corner.Add(r, edge_near(box, gp_Pnt(20, 20, 5)));
  corner.Build();
  CHECK(corner.IsDone());
  if (!corner.IsDone()) {
    return;
  }
  const TopoDS_Shape body = BRepBuilderAPI_NurbsConvert(corner.Shape()).Shape();
  const double before = 4000.0 - 10.0 * fillet_loss(r);
  CHECK_NEAR_TOL(exact_volume_of(body), before, 1e-8);
  const TopoDS_Shape rounded = occt_fillet(body, {gp_Pnt(10, 20, 10)}, r);
  CHECK(!rounded.IsNull());
  if (!rounded.IsNull()) {
    CHECK_NEAR_TOL(exact_volume_of(rounded), before - 36.0 * fillet_loss(r) - ball_corner_loss(r), 1e-8);
    double area = 0.0;
    CHECK(faces_on_sphere(rounded, gp_Pnt(18, 18, 8), r, 1e-4, area) == 1);
    CHECK_NEAR_TOL(area, kPi * r * r / 2.0, 1e-5);
  }
}

void test_closed_top_rounded_past_four_corners() {
  // The same box with all four vertical edges rounded with 2: the top's
  // outline is one closed tangent chain, rounded with 2, the ball turning
  // about a standing contact at each corner.
  const double r = 2.0;
  const TopoDS_Shape box = BRepPrimAPI_MakeBox(20, 20, 10).Shape();
  BRepFilletAPI_MakeFillet corners(box);
  for (const gp_Pnt& p : {gp_Pnt(20, 20, 5), gp_Pnt(0, 20, 5), gp_Pnt(0, 0, 5), gp_Pnt(20, 0, 5)}) {
    corners.Add(r, edge_near(box, p));
  }
  corners.Build();
  CHECK(corners.IsDone());
  if (!corners.IsDone()) {
    return;
  }
  const TopoDS_Shape body = BRepBuilderAPI_NurbsConvert(corners.Shape()).Shape();
  const double before = 4000.0 - 40.0 * fillet_loss(r);
  const TopoDS_Shape rounded = occt_fillet(body, {gp_Pnt(10, 20, 10)}, r);
  CHECK(!rounded.IsNull());
  if (!rounded.IsNull()) {
    CHECK_NEAR_TOL(exact_volume_of(rounded), before - 64.0 * fillet_loss(r) - 4.0 * ball_corner_loss(r), 1e-8);
    for (const gp_Pnt& c : {gp_Pnt(18, 18, 8), gp_Pnt(2, 18, 8), gp_Pnt(2, 2, 8), gp_Pnt(18, 2, 8)}) {
      double area = 0.0;
      CHECK(faces_on_sphere(rounded, c, r, 1e-4, area) == 1);
      CHECK_NEAR_TOL(area, kPi * r * r / 2.0, 1e-5);
    }
  }
}

void test_pocket_floor_rounded_past_its_corners() {
  // A pocket 20 x 20, 10 deep in a block 40 x 40 x 20, its inner vertical
  // edges rounded with 2, stored as splines; then the floor's outline
  // rounded with 2 (concave): the ball turns about a standing contact on
  // the floor at each corner. Each rounding adds what it would take off a
  // convex edge.
  const double r = 2.0;
  const TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(-20, -20, 0), gp_Pnt(20, 20, 20)).Shape();
  const TopoDS_Shape cut = BRepPrimAPI_MakeBox(gp_Pnt(-10, -10, 10), gp_Pnt(10, 10, 30)).Shape();
  const TopoDS_Shape pocket = BRepAlgoAPI_Cut(block, cut).Shape();
  BRepFilletAPI_MakeFillet corners(pocket);
  for (const gp_Pnt& p : {gp_Pnt(10, 10, 15), gp_Pnt(-10, 10, 15), gp_Pnt(-10, -10, 15), gp_Pnt(10, -10, 15)}) {
    corners.Add(r, edge_near(pocket, p));
  }
  corners.Build();
  CHECK(corners.IsDone());
  if (!corners.IsDone()) {
    return;
  }
  const TopoDS_Shape body = BRepBuilderAPI_NurbsConvert(corners.Shape()).Shape();
  const double before = 28000.0 + 40.0 * fillet_loss(r);
  const TopoDS_Shape rounded = occt_fillet(body, {gp_Pnt(0, 10, 10)}, r);
  CHECK(!rounded.IsNull());
  if (!rounded.IsNull()) {
    CHECK_NEAR_TOL(exact_volume_of(rounded), before + 64.0 * fillet_loss(r) + 4.0 * ball_corner_loss(r), 1e-8);
    for (const gp_Pnt& c : {gp_Pnt(8, 8, 12), gp_Pnt(-8, 8, 12), gp_Pnt(-8, -8, 12), gp_Pnt(8, -8, 12)}) {
      double area = 0.0;
      CHECK(faces_on_sphere(rounded, c, r, 1e-4, area) == 1);
      CHECK_NEAR_TOL(area, kPi * r * r / 2.0, 1e-5);
    }
  }
}

} // namespace

void fillet_result_tests() {
  test_slot_end_beside_a_split_side();
  test_rounding_ends_on_a_pads_split_foot();
  test_closed_rounding_rolling_from_a_seam_vertex();
  test_plate_edges_rounded_to_a_full_round();
  test_top_rounded_past_a_corner_of_the_balls_radius();
  test_closed_top_rounded_past_four_corners();
  test_pocket_floor_rounded_past_its_corners();
}

} // namespace test
