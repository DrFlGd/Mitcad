// SPDX-License-Identifier: MIT
// Walked roundings that are true rolling balls (mitcad#121): with the port's
// patch 0075 (third_party/vcpkg-ports/opencascade) every section of a
// walked constant-radius blend is the arc of the ball touching both faces,
// in the plane through the ball's centre that holds both contact normals.
// OCCT's own sections lay in the plane normal to the guide, which is that
// plane only where the contact normals are square to the edge.
//
// The check is independent of the blend's construction: the ball's centre
// runs along the intersection of the two faces' offsets by the radius, so
// every point of a rolling-ball rounding lies at the radius from that
// curve. Between a plate and a tilted rod or cone OCCT's sections missed it
// by up to 0.014 (radius 2); the rolling ball's by the approximation only.

#include <algorithm>
#include <cmath>
#include <vector>

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_IntSS.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>

#include "check.hpp"

namespace test {
namespace {

constexpr double kTilt = 40.0 * kPi / 180.0;

const gp_Dir& tilted_axis() {
  static const gp_Dir axis(std::sin(kTilt), 0.0, std::cos(kTilt));
  return axis;
}

TopoDS_Shape unify(const TopoDS_Shape& shape) {
  ShapeUpgrade_UnifySameDomain unify(shape);
  unify.Build();
  return unify.Shape();
}

// The edge of `shape` (not degenerated) whose middle lies in the plane z = 0
// within the box |x|, |y| < 15.
TopoDS_Edge edge_on_top(const TopoDS_Shape& shape) {
  TopoDS_Edge found;
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    double f = 0.0, l = 0.0;
    const occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, f, l);
    if (curve.IsNull()) {
      continue;
    }
    const gp_Pnt m = curve->Value((f + l) / 2);
    if (std::abs(m.Z()) < 1e-6 && std::abs(m.X()) < 15 && std::abs(m.Y()) < 15) {
      found = edge;
    }
  }
  return found;
}

// The surface of the face of `shape` of the given type.
occ::handle<Geom_Surface> surface_of(const TopoDS_Shape& shape, GeomAbs_SurfaceType type) {
  for (TopExp_Explorer f(shape, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    if (BRepAdaptor_Surface(face).GetType() == type) {
      return BRep_Tool::Surface(face);
    }
  }
  return {};
}

// The largest |distance - radius| of sampled points of the rounding's faces
// (those that are not planes, cylinders or cones) from the centre's curves.
double largest_miss(const TopoDS_Shape& rounded, const std::vector<occ::handle<Geom_Curve>>& centre, double radius,
                    int& samples) {
  double worst = 0.0;
  samples = 0;
  for (TopExp_Explorer f(rounded, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    const BRepAdaptor_Surface s(face);
    if (s.GetType() == GeomAbs_Plane || s.GetType() == GeomAbs_Cylinder || s.GetType() == GeomAbs_Cone) {
      continue;
    }
    double u0 = 0.0, u1 = 0.0, v0 = 0.0, v1 = 0.0;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    for (int i = 0; i <= 40; ++i) {
      for (int j = 0; j <= 10; ++j) {
        const gp_Pnt2d uv(u0 + (u1 - u0) * i / 40.0, v0 + (v1 - v0) * j / 10.0);
        if (BRepClass_FaceClassifier(face, uv, 1e-7).State() == TopAbs_OUT) {
          continue;
        }
        const gp_Pnt p = s.Value(uv.X(), uv.Y());
        double nearest = 1e100;
        for (const occ::handle<Geom_Curve>& c : centre) {
          GeomAPI_ProjectPointOnCurve projection(p, c);
          if (projection.NbPoints() > 0) {
            nearest = std::min(nearest, projection.LowerDistance());
          }
        }
        worst = std::max(worst, std::abs(nearest - radius));
        ++samples;
      }
    }
  }
  return worst;
}

// The curves where two offset surfaces meet.
std::vector<occ::handle<Geom_Curve>> meeting(const occ::handle<Geom_Surface>& a, const occ::handle<Geom_Surface>& b) {
  std::vector<occ::handle<Geom_Curve>> curves;
  GeomAPI_IntSS intersection(a, b, 1e-9);
  if (intersection.IsDone()) {
    for (int i = 1; i <= intersection.NbLines(); ++i) {
      curves.push_back(intersection.Line(i));
    }
  }
  return curves;
}

// Rounds the top edge of `body` with `radius` and checks the rounding
// against the centre curve where `offset_plane` meets `offset_side`.
void check_rolling_ball(const TopoDS_Shape& body, double radius, const occ::handle<Geom_Surface>& offset_plane,
                        const occ::handle<Geom_Surface>& offset_side) {
  const TopoDS_Edge edge = edge_on_top(body);
  CHECK(!edge.IsNull());
  if (edge.IsNull()) {
    return;
  }
  BRepFilletAPI_MakeFillet rounding(body);
  rounding.Add(radius, edge);
  rounding.Build();
  CHECK(rounding.IsDone());
  if (!rounding.IsDone()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(rounding.Shape()).IsValid());
  const std::vector<occ::handle<Geom_Curve>> centre = meeting(offset_plane, offset_side);
  CHECK(!centre.empty());
  if (centre.empty()) {
    return;
  }
  int samples = 0;
  const double miss = largest_miss(rounding.Shape(), centre, radius, samples);
  CHECK(samples > 100);
  if (miss > 2e-5) {
    std::fprintf(stderr, "rolling ball missed by %.3g\n", miss);
  }
  CHECK(miss < 2e-5);
}

void test_rounding_between_a_plate_and_a_tilted_rod_is_a_rolling_ball() {
  // A rod of radius 5 tilted by 40 degrees standing on a plate; the foot
  // rounded with radius 2. The centre runs where the plane z = 2 meets the
  // cylinder of radius 7 round the rod's axis.
  const double r = 5.0, radius = 2.0;
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(-30, -30, -10), gp_Pnt(30, 30, 0)).Shape();
  const gp_Pnt base(-8.0 * std::tan(kTilt), 0.0, -8.0);
  const TopoDS_Shape rod = BRepPrimAPI_MakeCylinder(gp_Ax2(base, tilted_axis()), r, 25.0).Shape();
  const TopoDS_Shape body = unify(BRepAlgoAPI_Fuse(plate, rod).Shape());
  const occ::handle<Geom_Surface> plane = new Geom_Plane(gp_Ax3(gp_Pnt(0, 0, radius), gp::DZ()));
  const occ::handle<Geom_Surface> side =
    new Geom_CylindricalSurface(gp_Ax3(gp::Origin(), tilted_axis()), r + radius);
  check_rolling_ball(body, radius, plane, side);
}

void test_rounding_of_a_tilted_rods_top_is_a_rolling_ball() {
  // The same rod cut by the plane z = 0: its top edge rounded with radius
  // 1.5, the ball inside, its centre where the plane z = -1.5 meets the
  // cylinder of radius 3.5.
  const double r = 5.0, radius = 1.5;
  const gp_Pnt base(-20.0 * std::tan(kTilt), 0.0, -20.0);
  const TopoDS_Shape rod = BRepPrimAPI_MakeCylinder(gp_Ax2(base, tilted_axis()), r, 40.0).Shape();
  const TopoDS_Shape below = BRepPrimAPI_MakeBox(gp_Pnt(-40, -40, -30), gp_Pnt(40, 40, 0)).Shape();
  const TopoDS_Shape body = unify(BRepAlgoAPI_Common(rod, below).Shape());
  const occ::handle<Geom_Surface> plane = new Geom_Plane(gp_Ax3(gp_Pnt(0, 0, -radius), gp::DZ()));
  const occ::handle<Geom_Surface> side =
    new Geom_CylindricalSurface(gp_Ax3(gp::Origin(), tilted_axis()), r - radius);
  check_rolling_ball(body, radius, plane, side);
}

void test_rounding_between_a_plate_and_a_tilted_cone_is_a_rolling_ball() {
  // A cone (radius 6 at its base, 3 at its top, 20 high) tilted by 40
  // degrees standing on a plate; the foot rounded with radius 2. The
  // centre runs where the plane z = 2 meets the cone's offset by 2: the
  // same axis and angle, its radius larger by 2 / cos(angle).
  const double radius = 2.0;
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(-30, -30, -10), gp_Pnt(30, 30, 0)).Shape();
  const gp_Pnt base(-6.0 * std::tan(kTilt), 0.0, -6.0);
  const TopoDS_Shape cone = BRepPrimAPI_MakeCone(gp_Ax2(base, tilted_axis()), 6.0, 3.0, 20.0).Shape();
  const TopoDS_Shape body = unify(BRepAlgoAPI_Fuse(plate, cone).Shape());
  const occ::handle<Geom_ConicalSurface> surface =
    occ::down_cast<Geom_ConicalSurface>(surface_of(body, GeomAbs_Cone));
  CHECK(!surface.IsNull());
  if (surface.IsNull()) {
    return;
  }
  const gp_Cone c = surface->Cone();
  const occ::handle<Geom_Surface> side =
    new Geom_ConicalSurface(c.Position(), c.SemiAngle(), c.RefRadius() + radius / std::cos(c.SemiAngle()));
  const occ::handle<Geom_Surface> plane = new Geom_Plane(gp_Ax3(gp_Pnt(0, 0, radius), gp::DZ()));
  check_rolling_ball(body, radius, plane, side);
}

} // namespace

void fillet_rolling_ball_tests() {
  guarded("test_rounding_between_a_plate_and_a_tilted_rod_is_a_rolling_ball",
          test_rounding_between_a_plate_and_a_tilted_rod_is_a_rolling_ball);
  guarded("test_rounding_of_a_tilted_rods_top_is_a_rolling_ball", test_rounding_of_a_tilted_rods_top_is_a_rolling_ball);
  guarded("test_rounding_between_a_plate_and_a_tilted_cone_is_a_rolling_ball",
          test_rounding_between_a_plate_and_a_tilted_cone_is_a_rolling_ball);
}

} // namespace test
