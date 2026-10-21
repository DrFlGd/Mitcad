// SPDX-License-Identifier: MIT
// Roundings cut by a face of the solid they pass through, built here with
// OCCT's fillet alone on synthetic shapes: the cases behind the patch 0700
// of the OCCT port (third_party/vcpkg-ports, mitcad#343). A ball rolling in
// a corner passes over a cap that stands out of the corner: the cap cuts the
// rounding, a notch in its side where the ball's contact runs over the cap's
// foot, a hole where the cap only pokes through it; of the cap the part
// outside the rounding is left. Two stripes whose ends meet at a cap that
// only pokes through the ball's path are one blend there (patch 0933).

#include <cmath>
#include <cstdio>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <GProp_GProps.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <NCollection_IndexedMap.hxx>
#include <TopExp.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Ax2.hxx>

#include "check.hpp"

namespace test {
namespace {

double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props, 1e-9, true);
  return props.Mass();
}

// The area of the faces of a shape on a sphere, and their number.
double sphere_area(const TopoDS_Shape& shape, int& count) {
  double area = 0.0;
  count       = 0;
  for (TopExp_Explorer e(shape, TopAbs_FACE); e.More(); e.Next()) {
    if (BRepAdaptor_Surface(TopoDS::Face(e.Current())).GetType() != GeomAbs_Sphere) {
      continue;
    }
    GProp_GProps props;
    BRepGProp::SurfaceProperties(e.Current(), props);
    area += props.Mass();
    ++count;
  }
  return area;
}

// An L-shaped block: a wall x < 0 and a floor z < 0 meeting in a concave
// edge along y (12 long), 6 thick.
TopoDS_Shape corner_block() {
  const TopoDS_Shape wall  = BRepPrimAPI_MakeBox(gp_Pnt(-6, -6, -6), gp_Pnt(0, 6, 6)).Shape();
  const TopoDS_Shape floor = BRepPrimAPI_MakeBox(gp_Pnt(-6, -6, -6), gp_Pnt(6, 6, 0)).Shape();
  return BRepAlgoAPI_Fuse(wall, floor).Shape();
}

// A ball of radius r about c, its poles along y and its seam behind the
// wall (inside the block).
TopoDS_Shape cap_ball(const gp_Pnt& c, double r) {
  return BRepPrimAPI_MakeSphere(gp_Ax2(c, gp_Dir(0, 1, 0), gp_Dir(-1, 0, 0)), r).Shape();
}

// The edges of a shape whose middles keep(m).
template <class Keep>
std::vector<TopoDS_Edge> edges_where(const TopoDS_Shape& shape, Keep keep) {
  std::vector<TopoDS_Edge> edges;
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> map;
  TopExp::MapShapes(shape, TopAbs_EDGE, map);
  for (int i = 1; i <= map.Extent(); ++i) {
    const TopoDS_Edge& e = TopoDS::Edge(map(i));
    BRepAdaptor_Curve  c(e);
    if (keep(c.Value(0.5 * (c.FirstParameter() + c.LastParameter())))) {
      edges.push_back(e);
    }
  }
  return edges;
}

bool on_corner(const gp_Pnt& m) {
  return std::abs(m.X()) < 1e-7 && std::abs(m.Z()) < 1e-7;
}

// The shape with the given edges rounded with radius 1; null where OCCT
// fails.
TopoDS_Shape rounded(const TopoDS_Shape& body, const std::vector<TopoDS_Edge>& edges) {
  BRepFilletAPI_MakeFillet fillet(body);
  for (const TopoDS_Edge& e : edges) {
    fillet.Add(1.0, e);
  }
  try {
    OCC_CATCH_SIGNALS
    fillet.Build();
  } catch (const Standard_Failure& failure) {
    std::fprintf(stderr, "pierced rounding test: %s\n", failure.what());
    return TopoDS_Shape();
  }
  return fillet.IsDone() ? fillet.Shape() : TopoDS_Shape();
}

// The corner block with a ball fused into it, its corner (and the edges
// with_also picks) rounded with 1, against the block's corner rounded and
// the ball fused to that: the ball touches neither of the rounding's
// faces, so the two are one solid. The sphere's faces left are compared
// too: the part of the cap outside the rounding.
void check_rounded_over_a_cap(const gp_Pnt& centre, double radius, bool with_cap_edges, int sphere_faces) {
  const TopoDS_Shape ball = cap_ball(centre, radius);
  const TopoDS_Shape body = BRepAlgoAPI_Fuse(corner_block(), ball).Shape();
  CHECK(BRepCheck_Analyzer(body).IsValid());
  std::vector<TopoDS_Edge> edges = edges_where(body, on_corner);
  if (with_cap_edges) {
    // The cap's edges with the wall.
    for (const TopoDS_Edge& e : edges_where(body, [](const gp_Pnt& m) {
           return std::abs(m.X()) < 1e-7 && m.Z() > 1e-7 && m.Z() < 2.0 && std::abs(m.Y()) < 3.0;
         })) {
      edges.push_back(e);
    }
  }
  const TopoDS_Shape block = corner_block();
  const TopoDS_Shape plain = rounded(block, edges_where(block, on_corner));
  CHECK(!plain.IsNull());
  const TopoDS_Shape result = rounded(body, edges);
  CHECK(!result.IsNull());
  if (result.IsNull() || plain.IsNull()) {
    return;
  }
  const TopoDS_Shape expected = BRepAlgoAPI_Fuse(plain, ball).Shape();
  CHECK(BRepCheck_Analyzer(result).IsValid());
  CHECK_NEAR_TOL(volume_of(result), volume_of(expected), 1e-8);
  int          n = 0, n_expected = 0;
  const double a = sphere_area(result, n), a_expected = sphere_area(expected, n_expected);
  CHECK(n == sphere_faces);
  CHECK(n_expected == sphere_faces);
  // (The cut is an approximated intersection, here as in the boolean.)
  CHECK_NEAR_TOL(a, a_expected, 1e-4);
}

// The cap stands out of the wall and the floor (a ball of 2.8 below the
// floor behind the wall), its foot on the floor reaching past the rounding's
// contact there (x 1.46 against 1): the corner's edge, broken by the cap,
// is two edges, two stripes. One ball rolls on over the cap from one to the
// other: its contact on the floor leaves the floor across the cap's foot and
// comes back across it 2.5 on. The cap cuts the rounding from there to
// there, the cut reaching back along the rounding past both points (a notch
// in its side); the cap keeps its part outside the rounding, between the
// cut and its foot. Before, each stripe ended at its vertex of the cap with
// a corner, which built an invalid shape.
void test_rounding_over_a_cap_notched_by_it() {
  check_rounded_over_a_cap(gp_Pnt(-0.5, 0.0, -2.0), 2.8, false, 1);
}

// As above, the cap's edges with the wall rounded too (the whole chain round
// the cap): their stripes lie under the ball rolling over the cap and are
// dropped.
void test_rounding_over_a_cap_notched_by_it_with_the_caps_edges() {
  check_rounded_over_a_cap(gp_Pnt(-0.5, 0.0, -2.0), 2.8, true, 1);
}

// A small cap stands out of the wall only, inside the corner's rounding, and
// pokes through it: the rounding (one stripe along the whole corner) has a
// hole where the cap cuts it, the cap's edge with the wall lies under the
// rounding, and the cap keeps its part outside it, bounded by the cut alone.
// Before, the rounding ran through the cap, which was left out.
void test_rounding_pierced_by_a_cap() {
  check_rounded_over_a_cap(gp_Pnt(-0.25, 0.0, 0.45), 0.5, false, 1);
}

// A post of radius post_radius standing on a plate (its seam away from the
// caps below), the edge round its foot broken by two caps, balls of radius
// cap_radius on either side of the post whose centres lie 0.3 inside the
// post and 0.3 below the plate.
TopoDS_Shape post_with_caps(double post_radius, double cap_radius, TopoDS_Shape& caps) {
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(-8, -8, -6), gp_Pnt(8, 8, 0)).Shape();
  const TopoDS_Shape post =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -1), gp_Dir(0, 0, 1), gp_Dir(0, -1, 0)), post_radius, 7).Shape();
  const double x = post_radius - 0.3;
  caps           = BRepAlgoAPI_Fuse(cap_ball(gp_Pnt(x, 0, -0.3), cap_radius), cap_ball(gp_Pnt(-x, 0, -0.3), cap_radius))
           .Shape();
  return BRepAlgoAPI_Fuse(plate, post).Shape();
}

// The foot of the post rounded with 0.5, two stripes between the caps: the
// caps' feet lie inside the rounding's contacts, which stay on the plate
// and the post, and the caps poke through the rounding. Each pair of ends
// at a cap is one blend walked over the cap (port patch 0933), holed where
// the cap pokes through; the result is the post's rounded foot with the
// caps fused to it. Before, the corners at the caps' vertices cut each
// stripe's end by its cap and built an invalid solid.
void check_post_foot_rounded_over_caps(double post_radius, double cap_radius) {
  TopoDS_Shape       caps;
  const TopoDS_Shape bare = post_with_caps(post_radius, cap_radius, caps);
  const TopoDS_Shape body = BRepAlgoAPI_Fuse(bare, caps).Shape();
  CHECK(BRepCheck_Analyzer(body).IsValid());
  const auto foot = [post_radius](const gp_Pnt& m) {
    return std::abs(std::hypot(m.X(), m.Y()) - post_radius) < 1e-7 && std::abs(m.Z()) < 1e-7;
  };
  BRepFilletAPI_MakeFillet plain_fillet(bare), fillet(body);
  for (const TopoDS_Edge& e : edges_where(bare, foot)) {
    plain_fillet.Add(0.5, e);
  }
  const std::vector<TopoDS_Edge> edges = edges_where(body, foot);
  CHECK(edges.size() == 3); // (one arc split at the post's seam)
  for (const TopoDS_Edge& e : edges) {
    fillet.Add(0.5, e);
  }
  try {
    OCC_CATCH_SIGNALS
    plain_fillet.Build();
    fillet.Build();
  } catch (const Standard_Failure& failure) {
    std::fprintf(stderr, "post foot rounding test: %s\n", failure.what());
  }
  CHECK(plain_fillet.IsDone());
  CHECK(fillet.IsDone());
  if (!plain_fillet.IsDone() || !fillet.IsDone()) {
    return;
  }
  const TopoDS_Shape result   = fillet.Shape();
  const TopoDS_Shape expected = BRepAlgoAPI_Fuse(plain_fillet.Shape(), caps).Shape();
  CHECK(BRepCheck_Analyzer(result).IsValid());
  CHECK_NEAR_TOL(volume_of(result), volume_of(expected), 2e-5);
  int          n = 0, n_expected = 0;
  const double a = sphere_area(result, n), a_expected = sphere_area(expected, n_expected);
  CHECK(n == 2);
  CHECK(n_expected == 2);
  // (The holes' curves are approximated intersections.)
  CHECK_NEAR_TOL(a, a_expected, 2e-4);
}

void test_post_foot_rounded_over_two_caps() {
  check_post_foot_rounded_over_caps(1.0, 0.7);
}

// As above, a thinner post and larger caps: each cap reaches round the
// post to the walks' start sections, at the stripes' vertices.
void test_thin_post_foot_rounded_over_two_caps() {
  check_post_foot_rounded_over_caps(0.8, 0.75);
}

} // namespace

void fillet_pierced_tests() {
  guarded("test_rounding_over_a_cap_notched_by_it", test_rounding_over_a_cap_notched_by_it);
  guarded("test_rounding_over_a_cap_notched_by_it_with_the_caps_edges",
          test_rounding_over_a_cap_notched_by_it_with_the_caps_edges);
  guarded("test_rounding_pierced_by_a_cap", test_rounding_pierced_by_a_cap);
  guarded("test_post_foot_rounded_over_two_caps", test_post_foot_rounded_over_two_caps);
  guarded("test_thin_post_foot_rounded_over_two_caps", test_thin_post_foot_rounded_over_two_caps);
}

} // namespace test
