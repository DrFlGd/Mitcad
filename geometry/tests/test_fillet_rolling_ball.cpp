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
//
// With patches 0170 to 0172 (mitcad#237) the same holds for a ball that
// rolls on an edge, where the face is narrower than the radius: its centre
// lies at the radius from the other face and from the edge, and where the
// edge ends the ball turns about its end, its centre on the sphere round it.
// With 0390 the ball that comes to an edge turning towards it touches that
// edge before the vertex and rolls on it from there, a piece of the ball
// between; with 0391 a ball resting on the edges of both faces is the ball
// tangent to both.

#include <algorithm>
#include <cmath>
#include <vector>

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_IntSS.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <Geom_Circle.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_IndexedMap.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
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

// Rounds `edge` of `body` with `radius` and checks the rounding against the
// centre curves `centre`.
void check_rounding(const TopoDS_Shape& body, const TopoDS_Edge& edge, double radius,
                    const std::vector<occ::handle<Geom_Curve>>& centre) {
  CHECK(!edge.IsNull());
  CHECK(!centre.empty());
  if (edge.IsNull() || centre.empty()) {
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
  int samples = 0;
  const double miss = largest_miss(rounding.Shape(), centre, radius, samples);
  CHECK(samples > 100);
  if (miss > 2e-5) {
    std::fprintf(stderr, "rolling ball missed by %.3g\n", miss);
  }
  CHECK(miss < 2e-5);
}

// Rounds the top edge of `body` with `radius` and checks the rounding
// against the centre curve where `offset_plane` meets `offset_side`.
void check_rolling_ball(const TopoDS_Shape& body, double radius, const occ::handle<Geom_Surface>& offset_plane,
                        const occ::handle<Geom_Surface>& offset_side) {
  check_rounding(body, edge_on_top(body), radius, meeting(offset_plane, offset_side));
}

// The edge of `shape` (not degenerated) whose middle `pick` accepts; null
// unless there is exactly one.
template <class Pick>
TopoDS_Edge the_edge(const TopoDS_Shape& shape, Pick pick) {
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges;
  TopExp::MapShapes(shape, TopAbs_EDGE, edges);
  TopoDS_Edge found;
  int count = 0;
  for (int i = 1; i <= edges.Extent(); ++i) {
    const TopoDS_Edge& edge = TopoDS::Edge(edges(i));
    double f = 0.0, l = 0.0;
    const occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, f, l);
    if (!curve.IsNull() && pick(curve->Value((f + l) / 2))) {
      found = edge;
      ++count;
    }
  }
  return count == 1 ? found : TopoDS_Edge();
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

void test_rounding_rolling_on_a_skew_edge_is_a_rolling_ball() {
  // A rib on a plate whose far side tapers (0.5 to 2.5 thick over 40); its
  // near top edge rounded with 3. The rib is thinner than the radius: the
  // ball touches the near side and rolls on the far top edge, which is skew
  // to the rounded edge (patch 0170). Its centre runs where the near side's
  // offset (the plane y = 12.5) meets the tube of radius 3 round the far
  // top edge. Where that edge ends, the ball turns about its end (0171), its
  // centre on the sphere of radius 3 round it, until the end face cuts it.
  // OCCT's sections, in the guide's normal plane, missed it by 6.5e-4.
  const double radius = 3.0;
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(0, 0, 0), gp_Pnt(40, 20, 5)).Shape();
  BRepBuilderAPI_MakePolygon outline(gp_Pnt(0, 9.5, 5), gp_Pnt(40, 9.5, 5), gp_Pnt(40, 12, 5), gp_Pnt(0, 10, 5),
                                     true);
  const TopoDS_Face foot = BRepBuilderAPI_MakeFace(outline.Wire());
  const TopoDS_Shape rib = BRepPrimAPI_MakePrism(foot, gp_Vec(0, 0, 10)).Shape();
  const TopoDS_Shape body = unify(BRepAlgoAPI_Fuse(plate, rib).Shape());
  const TopoDS_Edge edge = the_edge(body, [](const gp_Pnt& m) {
    return std::abs(m.Y() - 9.5) < 1e-6 && std::abs(m.Z() - 15) < 1e-6;
  });
  const occ::handle<Geom_Surface> side = new Geom_Plane(gp_Ax3(gp_Pnt(0, 9.5 + radius, 0), gp::DY()));
  const occ::handle<Geom_Surface> tube =
    new Geom_CylindricalSurface(gp_Ax3(gp_Pnt(0, 10, 15), gp_Dir(40, 2, 0)), radius);
  std::vector<occ::handle<Geom_Curve>> centre = meeting(side, tube);
  // The turn: the circle where the sphere round (40, 12, 15) meets that
  // plane, below the top and before the end face (x from 39.9 to 40).
  const double r = std::sqrt(radius * radius - 0.5 * 0.5);
  const occ::handle<Geom_Circle> turn =
    new Geom_Circle(gp_Circ(gp_Ax2(gp_Pnt(40, 12.5, 15), gp::DY(), gp::DX()), r));
  centre.push_back(new Geom_TrimmedCurve(turn, kPi / 2, std::acos(-0.1 / r)));
  check_rounding(body, edge, radius, centre);
}

void test_rounding_rolling_on_a_rim_past_its_face_is_a_rolling_ball() {
  // A post of radius 8 with a round pocket of radius 5.5 centred at
  // (0, 12) cut down to z = 5: the pocket's floor is a lens between its
  // wall and the post's rim, at most 1.5 wide. The floor's edge at the wall
  // rounded with 2: the ball touches the wall and rolls on the rim, its
  // centre where the wall's offset (radius 3.5) meets the tube of radius 2
  // round the rim (0170). Near the lens's tips the ball's contact leaves
  // the wall across the edge where wall and post meet, while the ball still
  // rolls on the rim until it reaches the post's side: there the rounding
  // is cut by the post's side and ends in a point on the rim (0172). Before,
  // the rounding was invalid; OCCT's sections missed the ball by 0.29.
  const double radius = 2.0;
  const TopoDS_Shape post = BRepPrimAPI_MakeCylinder(gp_Ax2(gp::Origin(), gp::DZ()), 8.0, 30.0).Shape();
  const TopoDS_Shape pocket = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 12, 5), gp::DZ()), 5.5, 30.0).Shape();
  const TopoDS_Shape body = unify(BRepAlgoAPI_Cut(post, pocket).Shape());
  const TopoDS_Edge edge = the_edge(body, [](const gp_Pnt& m) {
    return std::abs(m.Z() - 5) < 1e-6 && std::abs(m.Distance(gp_Pnt(0, 12, 5)) - 5.5) < 1e-6;
  });
  const occ::handle<Geom_Surface> wall =
    new Geom_CylindricalSurface(gp_Ax3(gp_Pnt(0, 12, 0), gp::DZ()), 5.5 - radius);
  const occ::handle<Geom_Surface> tube = new Geom_ToroidalSurface(gp_Ax3(gp_Pnt(0, 0, 5), gp::DZ()), 8.0, radius);
  check_rounding(body, edge, radius, meeting(wall, tube));
}

// The largest |distance - radius| of sampled points of the rounding's faces
// that are not planes from the nearest of the centre curves and points.
double largest_miss_from(const TopoDS_Shape& rounded, const std::vector<occ::handle<Geom_Curve>>& centre,
                         const std::vector<gp_Pnt>& points, double radius, int& samples) {
  double worst = 0.0;
  samples = 0;
  for (TopExp_Explorer f(rounded, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    const BRepAdaptor_Surface s(face);
    if (s.GetType() == GeomAbs_Plane) {
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
          // (A trimmed curve's ends, where the projection falls beyond them.)
          nearest = std::min(nearest, p.Distance(c->Value(c->FirstParameter())));
          nearest = std::min(nearest, p.Distance(c->Value(c->LastParameter())));
        }
        for (const gp_Pnt& q : points) {
          nearest = std::min(nearest, p.Distance(q));
        }
        worst = std::max(worst, std::abs(nearest - radius));
        ++samples;
      }
    }
  }
  return worst;
}

// The arc of the closed curve c from its point nearest a to that nearest b,
// through the side where z < zmax.
occ::handle<Geom_Curve> arc_below(const occ::handle<Geom_Curve>& c, const gp_Pnt& a, const gp_Pnt& b, double zmax) {
  GeomAPI_ProjectPointOnCurve pa(a, c), pb(b, c);
  const double ta = pa.LowerDistanceParameter(), tb = pb.LowerDistanceParameter();
  const double lo = std::min(ta, tb), hi = std::max(ta, tb);
  if (c->Value(0.5 * (lo + hi)).Z() < zmax) {
    return new Geom_TrimmedCurve(c, lo, hi);
  }
  return new Geom_TrimmedCurve(c, hi, lo + c->Period());
}

void test_rounding_rolling_on_an_edge_into_a_concave_turn() {
  // A rib on a plate, 0.5 thick up to x = 20, whose far side then turns
  // towards the ball (it thickens to 2.5 at x = 40); its near top edge
  // rounded with 3. The ball touches the near side (its centre on the plane
  // y = 12.5) and rolls on the far top edge (y = 10, z = 15), its centre at
  // z = 15 - sqrt(2.75). The far top edge beyond the turn comes towards the
  // ball: the ball touches it at x = 19.8753, before the vertex, and rolls
  // on it from there (patch 0390); between the two, where the ball stands
  // touching both edges, the rounding is a piece of that ball. At x = 40 the
  // ball turns about the far edge's end (0171). Before, the ball rolled on
  // through the turn's edge into the rib, and the next blend started at the
  // vertex: 2.6e-3 off.
  const double radius = 3.0;
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(0, 0, 0), gp_Pnt(40, 20, 5)).Shape();
  BRepBuilderAPI_MakePolygon outline;
  outline.Add(gp_Pnt(0, 9.5, 5));
  outline.Add(gp_Pnt(40, 9.5, 5));
  outline.Add(gp_Pnt(40, 12, 5));
  outline.Add(gp_Pnt(20, 10, 5));
  outline.Add(gp_Pnt(0, 10, 5));
  outline.Close();
  const TopoDS_Face foot = BRepBuilderAPI_MakeFace(outline.Wire());
  const TopoDS_Shape rib = BRepPrimAPI_MakePrism(foot, gp_Vec(0, 0, 10)).Shape();
  const TopoDS_Shape body = unify(BRepAlgoAPI_Fuse(plate, rib).Shape());
  const TopoDS_Edge edge = the_edge(body, [](const gp_Pnt& m) {
    return std::abs(m.Y() - 9.5) < 1e-6 && std::abs(m.Z() - 15) < 1e-6;
  });
  // The centre: on the straight far edge, the line y = 12.5, z = zc up to
  // the turn at x = xt (where the slanted edge is 2.5 from it in plan);
  // beyond it, where the plane y = 12.5 meets the tube of radius 3 round
  // the slanted edge, up to x = 40; then the ball's turn about the edge's end
  // (40, 12, 15), its centre on the circle where the sphere round it meets
  // that plane. The turn's ball stands at the end of the first line.
  const gp_Dir slant(20, 2, 0);
  const gp_Vec normal(-slant.Y(), slant.X(), 0);
  const double zc = 15 - std::sqrt(radius * radius - 2.5 * 2.5);
  const double xt = 20 + (2.5 - 2.5 * normal.Y()) / normal.X();
  const gp_Pnt stand(xt, 12.5, zc);
  std::vector<occ::handle<Geom_Curve>> centre;
  centre.push_back(new Geom_TrimmedCurve(new Geom_Line(gp_Pnt(0, 12.5, zc), gp::DX()), -1, xt));
  const occ::handle<Geom_Surface> side = new Geom_Plane(gp_Ax3(gp_Pnt(0, 12.5, 0), gp::DY()));
  const occ::handle<Geom_Surface> tube = new Geom_CylindricalSurface(gp_Ax3(gp_Pnt(20, 10, 15), slant), radius);
  const gp_Pnt last(40, 12.5, 15 - std::sqrt(radius * radius - 0.5 * 0.5));
  for (const occ::handle<Geom_Curve>& c : meeting(side, tube)) {
    centre.push_back(c->IsPeriodic() ? arc_below(c, stand, last, 15) : c);
  }
  const double r = std::sqrt(radius * radius - 0.5 * 0.5);
  centre.push_back(new Geom_TrimmedCurve(
    new Geom_Circle(gp_Circ(gp_Ax2(gp_Pnt(40, 12.5, 15), gp::DY(), gp::DX()), r)), kPi / 2, kPi / 2 + 0.05));
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
  int samples = 0;
  const double miss = largest_miss_from(rounding.Shape(), centre, {stand}, radius, samples);
  CHECK(samples > 100);
  if (miss > 2e-5) {
    std::fprintf(stderr, "concave turn: rolling ball missed by %.3g\n", miss);
  }
  CHECK(miss < 2e-5);
}

// The largest miss of the rounding's blend faces (B-splines, tori, spheres)
// from a ball resting on both curves a and b: at each sampled point the
// ball's centre, one radius along the face's normal, must lie at the radius
// from both.
double edge_ball_miss(const TopoDS_Shape& rounded, const occ::handle<Geom_Curve>& a, const occ::handle<Geom_Curve>& b,
                      double radius, int& samples) {
  double worst = 0.0;
  samples = 0;
  for (TopExp_Explorer f(rounded, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    const BRepAdaptor_Surface s(face);
    if (s.GetType() != GeomAbs_BSplineSurface && s.GetType() != GeomAbs_Torus && s.GetType() != GeomAbs_Sphere) {
      continue;
    }
    double u0 = 0.0, u1 = 0.0, v0 = 0.0, v1 = 0.0;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    for (int i = 0; i <= 40; ++i) {
      for (int j = 1; j < 10; ++j) {
        const gp_Pnt2d uv(u0 + (u1 - u0) * i / 40.0, v0 + (v1 - v0) * j / 10.0);
        if (BRepClass_FaceClassifier(face, uv, 1e-7).State() != TopAbs_IN) {
          continue;
        }
        gp_Pnt p;
        gp_Vec du, dv;
        s.D1(uv.X(), uv.Y(), p, du, dv);
        gp_Vec n = du.Crossed(dv);
        if (n.Magnitude() < 1e-12) {
          continue;
        }
        n.Normalize();
        double best = 1e100;
        for (int side = -1; side <= 1; side += 2) {
          const gp_Pnt c = p.Translated(side * radius * n);
          GeomAPI_ProjectPointOnCurve pa(c, a), pb(c, b);
          if (pa.NbPoints() > 0 && pb.NbPoints() > 0) {
            best = std::min(best,
                            std::max(std::abs(pa.LowerDistance() - radius), std::abs(pb.LowerDistance() - radius)));
          }
        }
        worst = std::max(worst, best);
        ++samples;
      }
    }
  }
  return worst;
}

void test_rounding_resting_on_the_edges_of_both_faces_is_a_rolling_ball() {
  // A ring round the z axis whose cross-section is a house: an outer wall
  // of radius 21 up to its eave at z = 9, roof faces at 45 degrees meeting
  // at the ridge (radius 20, z = 10); the inner wall a cylinder of radius
  // 19.2 round (0.3, 0, 0), off the axis, so that the inner eave is no
  // circle and the inner roof 0.5 to 1.1 wide. The ridge rounded with 3:
  // both roof faces are narrower than the ball's contacts (3 tan 45), the
  // ball rests on both eaves (patch 0391, closed contours only), each
  // tangent to it. Before, that start was refused and the rounding built
  // missed the eaves by 1.2.
  const double radius = 3.0;
  BRepBuilderAPI_MakePolygon profile;
  profile.Add(gp_Pnt(17, 0, 0));
  profile.Add(gp_Pnt(21, 0, 0));
  profile.Add(gp_Pnt(21, 0, 9));
  profile.Add(gp_Pnt(20, 0, 10));
  profile.Add(gp_Pnt(17, 0, 7));
  profile.Close();
  const TopoDS_Face section = BRepBuilderAPI_MakeFace(profile.Wire());
  const TopoDS_Shape ring = BRepPrimAPI_MakeRevol(section, gp_Ax1(gp::Origin(), gp::DZ())).Shape();
  const TopoDS_Shape hole = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0.3, 0, -1), gp::DZ()), 19.2, 20).Shape();
  const TopoDS_Shape body = unify(BRepAlgoAPI_Cut(ring, hole).Shape());
  const TopoDS_Edge edge = the_edge(body, [](const gp_Pnt& m) {
    return std::abs(std::hypot(m.X(), m.Y()) - 20) < 1e-6 && std::abs(m.Z() - 10) < 1e-6;
  });
  // The eaves: the outer circle, and the inner roof's edge on the hole.
  const occ::handle<Geom_Curve> outer = new Geom_Circle(gp_Circ(gp_Ax2(gp_Pnt(0, 0, 9), gp::DZ()), 21));
  occ::handle<Geom_Curve> inner;
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    double f = 0.0, l = 0.0;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(TopoDS::Edge(e.Current()), f, l);
    if (!c.IsNull()) {
      const gp_Pnt m = c->Value(0.5 * (f + l));
      if (m.Z() > 7.5 && m.Z() < 9.5 && std::hypot(m.X(), m.Y()) < 19.8) {
        inner = new Geom_TrimmedCurve(c, f, l);
      }
    }
  }
  CHECK(!edge.IsNull());
  CHECK(!inner.IsNull());
  if (edge.IsNull() || inner.IsNull()) {
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
  int samples = 0;
  const double miss = edge_ball_miss(rounding.Shape(), outer, inner, radius, samples);
  CHECK(samples > 100);
  if (miss > 5e-5) {
    std::fprintf(stderr, "ball on both edges missed by %.3g\n", miss);
  }
  CHECK(miss < 5e-5);
}

} // namespace

void fillet_rolling_ball_tests() {
  guarded("test_rounding_rolling_on_an_edge_into_a_concave_turn",
          test_rounding_rolling_on_an_edge_into_a_concave_turn);
  guarded("test_rounding_resting_on_the_edges_of_both_faces_is_a_rolling_ball",
          test_rounding_resting_on_the_edges_of_both_faces_is_a_rolling_ball);
  guarded("test_rounding_between_a_plate_and_a_tilted_rod_is_a_rolling_ball",
          test_rounding_between_a_plate_and_a_tilted_rod_is_a_rolling_ball);
  guarded("test_rounding_of_a_tilted_rods_top_is_a_rolling_ball", test_rounding_of_a_tilted_rods_top_is_a_rolling_ball);
  guarded("test_rounding_between_a_plate_and_a_tilted_cone_is_a_rolling_ball",
          test_rounding_between_a_plate_and_a_tilted_cone_is_a_rolling_ball);
  guarded("test_rounding_rolling_on_a_skew_edge_is_a_rolling_ball",
          test_rounding_rolling_on_a_skew_edge_is_a_rolling_ball);
  guarded("test_rounding_rolling_on_a_rim_past_its_face_is_a_rolling_ball",
          test_rounding_rolling_on_a_rim_past_its_face_is_a_rolling_ball);
}

} // namespace test
