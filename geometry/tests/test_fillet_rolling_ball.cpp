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
//
// With 0500 the rounding between a cylinder and a cone, or two cones, about
// one axis is the torus its ball sweeps about the axis (OCCT walked it, a
// B-spline); with 0501 such a rounding whose contact passes below a ledge
// over flats cut into a post under an overhanging rim is cut round each
// flat (its side, the ledge, its side again), on a circle of one edge or
// two, the torus one face: the post rounded without flats, the flats cut
// after, is the reference.
//
// With 0502 the rounding of a thin rim's outer edge, wider than the rim, is
// cut where the face at its end (the round cap of the rim) meets it before
// its contact on the top comes back from over the pocket: the outer wall
// rounded alone, the ring it loses taken off the rim outside the caps, is
// the reference.
//
// With 0503 the rounding of a slot's chamfered rim along its straight wall
// (a cylinder) and round a boss (0500's torus) meet at the corners along
// their intersection where their ends lie on the chamfer's crease within its
// tolerance, as in a stored body.

#include <algorithm>
#include <cmath>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_IntSS.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_List.hxx>
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
#include <gp.hxx>
#include <gp_Torus.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec2d.hxx>

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


// A solid revolved about the z axis from the profile `points` (in the xz
// plane), by `angle` degrees (360: whole).
TopoDS_Shape revolved(const std::vector<gp_Pnt>& points, double angle = 360.0) {
  BRepBuilderAPI_MakePolygon polygon;
  for (const gp_Pnt& p : points) {
    polygon.Add(p);
  }
  polygon.Close();
  const TopoDS_Face profile = BRepBuilderAPI_MakeFace(polygon.Wire(), true).Face();
  const gp_Ax1 axis(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1));
  return angle >= 360.0 ? BRepPrimAPI_MakeRevol(profile, axis).Shape()
                        : BRepPrimAPI_MakeRevol(profile, axis, angle * kPi / 180.0).Shape();
}

double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props);
  return props.Mass();
}

// Rounds every edge of `body` that is an arc of the circle of radius `r`
// about the z axis at height `z`; a null shape where that fails.
TopoDS_Shape round_circle(const TopoDS_Shape& body, double r, double z, double radius) {
  BRepFilletAPI_MakeFillet rounding(body);
  int edges = 0;
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    BRepAdaptor_Curve c(edge);
    if (c.GetType() == GeomAbs_Circle && std::abs(c.Circle().Radius() - r) < 1e-9 &&
        std::abs(c.Circle().Location().Z() - z) < 1e-9 && rounding.Contour(edge) == 0) {
      rounding.Add(radius, edge);
      ++edges;
    }
  }
  CHECK(edges > 0);
  rounding.Build();
  return rounding.IsDone() ? rounding.Shape() : TopoDS_Shape();
}

// The torus faces of `shape`.
std::vector<TopoDS_Face> tori(const TopoDS_Shape& shape) {
  std::vector<TopoDS_Face> found;
  for (TopExp_Explorer f(shape, TopAbs_FACE); f.More(); f.Next()) {
    if (BRepAdaptor_Surface(TopoDS::Face(f.Current())).GetType() == GeomAbs_Torus) {
      found.push_back(TopoDS::Face(f.Current()));
    }
  }
  return found;
}

void test_rounding_between_coaxial_cylinder_and_cone_is_a_torus() {
  struct Case {
    std::vector<gp_Pnt> profile;
    double r, z, radius;
    // The ball's centre in the meridian plane.
    double centre_r, centre_z;
  };
  const double s = std::sqrt(2.0);
  const std::vector<Case> cases = {
      // A post with a conical top (convex): the centre inside, 1 from the
      // wall and from the top's line z = 2 + (10 - r).
      {{{0, 0, 0}, {10, 0, 0}, {10, 0, 2}, {8, 0, 4}, {0, 0, 4}}, 10, 2, 1, 9, 3 - s},
      // A post under a flaring overhang (concave): outside, below the flare.
      {{{0, 0, 0}, {10, 0, 0}, {10, 0, 2}, {11, 0, 3}, {13, 0, 3}, {13, 0, 5}, {0, 0, 5}}, 10, 2, 1, 11, 3 - s},
      // Two cones meeting at a ridge (convex).
      {{{0, 0, 0}, {10, 0, 0}, {12, 0, 2}, {10, 0, 4}, {0, 0, 4}}, 12, 2, 1, 12 - s, 2},
  };
  for (const double angle : {360.0, 270.0}) {
    for (const Case& c : cases) {
      const TopoDS_Shape body = revolved(c.profile, angle);
      const TopoDS_Shape rounded = round_circle(body, c.r, c.z, c.radius);
      CHECK(!rounded.IsNull());
      if (rounded.IsNull()) {
        continue;
      }
      CHECK(BRepCheck_Analyzer(rounded).IsValid());
      const std::vector<TopoDS_Face> faces = tori(rounded);
      CHECK(faces.size() == 1);
      if (faces.size() != 1) {
        continue;
      }
      const gp_Torus torus = BRepAdaptor_Surface(faces.front()).Torus();
      CHECK(torus.Axis().Direction().IsParallel(gp::DZ(), 1e-12));
      CHECK(std::abs(torus.Location().X()) < 1e-9 && std::abs(torus.Location().Y()) < 1e-9);
      CHECK(std::abs(torus.Location().Z() - c.centre_z) < 1e-9);
      CHECK(std::abs(torus.MajorRadius() - c.centre_r) < 1e-9);
      CHECK(std::abs(torus.MinorRadius() - c.radius) < 1e-12);
    }
  }
}

// A post (radius 10) under an overhang that flares out above it as a cone,
// rounded where they meet with radius 0.9: its contact on the post lies
// 0.0728 below a ledge at 0.8 under the post's rim, where four flats (each
// 35 degrees either side of its middle, the middles `offset` degrees round
// from the x axis) are cut into the post. With `two_edges` the post and the
// flare are separate solids fused, their seams at a flat's corner (145
// degrees) and mid-flat (0): the rounded circle is two edges.
TopoDS_Shape post_under_overhang(bool flats, double offset, bool two_edges) {
  const double R = 10.0;
  TopoDS_Shape body;
  if (two_edges) {
    const double a = 145.0 * kPi / 180.0;
    const TopoDS_Shape post =
        BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1), gp_Dir(std::cos(a), std::sin(a), 0)), R, 1.1)
            .Shape();
    const TopoDS_Shape flare =
        BRepPrimAPI_MakeCone(gp_Ax2(gp_Pnt(0, 0, 1.1), gp_Dir(0, 0, 1), gp_Dir(1, 0, 0)), R, R + 0.5, 0.5).Shape();
    const TopoDS_Shape cap = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 1.6), gp_Dir(0, 0, 1)), R + 2, 1.4).Shape();
    body = BRepAlgoAPI_Fuse(BRepAlgoAPI_Fuse(post, flare).Shape(), cap).Shape();
  } else {
    body = revolved({{0, 0, 0}, {R, 0, 0}, {R, 0, 1.1}, {R + 0.5, 0, 1.6}, {R + 2, 0, 1.6}, {R + 2, 0, 3}, {0, 0, 3}});
  }
  if (!flats) {
    return body;
  }
  for (int i = 0; i < 4; ++i) {
    TopoDS_Shape cut = BRepPrimAPI_MakeBox(gp_Pnt(R * std::cos(35.0 * kPi / 180.0), -20, -1), 20, 40, 1.8).Shape();
    gp_Trsf turn;
    turn.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), (offset + 90.0 * i) * kPi / 180.0);
    body = BRepAlgoAPI_Cut(body, BRepBuilderAPI_Transform(cut, turn, true).Shape()).Shape();
  }
  return body;
}

void test_rim_rounding_below_a_ledge_over_flats_is_cut_round_them() {
  struct Case {
    double offset;
    bool two_edges;
  };
  // The circle's seam mid-tooth, mid-flat; two edges.
  for (const Case c : {Case{45.0, false}, Case{0.0, false}, Case{0.0, true}}) {
    const TopoDS_Shape plain = post_under_overhang(false, c.offset, c.two_edges);
    const TopoDS_Shape body = post_under_overhang(true, c.offset, c.two_edges);
    const TopoDS_Shape plain_rounded = round_circle(plain, 10, 1.1, 0.9);
    const TopoDS_Shape rounded = round_circle(body, 10, 1.1, 0.9);
    CHECK(!plain_rounded.IsNull());
    CHECK(!rounded.IsNull());
    if (plain_rounded.IsNull() || rounded.IsNull()) {
      continue;
    }
    CHECK(BRepCheck_Analyzer(rounded).IsValid());
    // The rounding over the flats, cut by their faces: the plain post's,
    // the flats cut after (the room under the ledge, outside each flat).
    TopoDS_Shape reference = plain_rounded;
    for (int i = 0; i < 4; ++i) {
      TopoDS_Shape cut = BRepPrimAPI_MakeBox(gp_Pnt(10 * std::cos(35.0 * kPi / 180.0), -20, -1), 20, 40, 1.8).Shape();
      gp_Trsf turn;
      turn.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), (c.offset + 90.0 * i) * kPi / 180.0);
      reference = BRepAlgoAPI_Cut(reference, BRepBuilderAPI_Transform(cut, turn, true).Shape()).Shape();
    }
    const double v = volume_of(rounded), expected = volume_of(reference);
    if (std::abs(v - expected) > 1e-4) {
      std::fprintf(stderr, "rounding over flats: volume %.7f, expected %.7f\n", v, expected);
    }
    CHECK(std::abs(v - expected) < 1e-4);
    // The cut takes the slivers of the rounding under the ledges.
    CHECK(volume_of(plain_rounded) - volume_of(plain) - (v - volume_of(body)) > 2e-3);
    CHECK(tori(rounded).size() == 1);
  }
}

// A thin rim: an eccentric ring between an outer wall (radius 15.357 about
// (0, -0.3)) and a pocket (radius 14.4 about the axis), 15 high, whose ends
// are round caps of radius 1 centred on the pocket's corners at y = -2.2;
// the caps stand out of the outer wall by 3e-4 between their crossings with
// it. Spoke walls tangent to the caps bound the rim inside.
namespace thin_rim {
const gp_Pnt2d kCentre(0, -0.3);
const double   kOuter = 15.357, kHeight = 15.0, kPocket = 14.4;

TopoDS_Shape prism(const std::vector<gp_Pnt2d>& points, double z0, double z1) {
  BRepBuilderAPI_MakePolygon polygon;
  for (const gp_Pnt2d& p : points) {
    polygon.Add(gp_Pnt(p.X(), p.Y(), z0));
  }
  polygon.Close();
  return BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(polygon.Wire(), true).Face(), gp_Vec(0, 0, z1 - z0)).Shape();
}

TopoDS_Shape cylinder(const gp_Pnt2d& c, double r, double z0, double z1) {
  return BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(c.X(), c.Y(), z0), gp_Dir(0, 0, 1)), r, z1 - z0).Shape();
}

gp_Pnt2d corner(bool right) { return gp_Pnt2d((right ? 1 : -1) * std::sqrt(kPocket * kPocket - 2.2 * 2.2), -2.2); }

// The cap's lower crossing with the outer circle.
gp_Pnt2d crossing(const gp_Pnt2d& v) {
  const double   d = v.Distance(kCentre);
  const double   a = (kOuter * kOuter - 1.0 + d * d) / (2 * d);
  const double   h = std::sqrt(std::max(0.0, kOuter * kOuter - a * a));
  const gp_Vec2d e = gp_Vec2d(kCentre, v) / d;
  const gp_Vec2d n(-e.Y(), e.X());
  const gp_Pnt2d p = kCentre.Translated(e * a + n * h), q = kCentre.Translated(e * a - n * h);
  return p.Y() < q.Y() ? p : q;
}

TopoDS_Shape caps(double z0, double z1) {
  return BRepAlgoAPI_Fuse(cylinder(corner(true), 1, z0, z1), cylinder(corner(false), 1, z0, z1)).Shape();
}

TopoDS_Shape outer() { return cylinder(kCentre, kOuter, 0, kHeight); }

TopoDS_Shape body() {
  const gp_Pnt2d v1 = corner(true), v2 = corner(false);
  const gp_Pnt2d e1 = v1.Translated(gp_Vec2d(v1, crossing(v1)) * 3.0);
  const gp_Pnt2d e2 = v2.Translated(gp_Vec2d(v2, crossing(v2)) * 3.0);
  const TopoDS_Shape inside = prism({{13.716, -1.343}, {6.1315, -5.9}, {-6.1315, -5.9}, {-13.716, -1.343}, v2, e2,
                                     {-40, -40}, {40, -40}, e1, v1},
                                    -1, kHeight + 1);
  TopoDS_Shape rim = BRepAlgoAPI_Fuse(BRepAlgoAPI_Common(outer(), inside).Shape(), caps(0, kHeight)).Shape();
  const TopoDS_Shape pocket = BRepAlgoAPI_Common(
      cylinder(gp_Pnt2d(0, 0), kPocket, -1, kHeight + 1),
      prism({v1, {6.4088, -6.9}, {-6.4088, -6.9}, v2, {-20, -40}, {20, -40}}, -1, kHeight + 1))
                                  .Shape();
  rim = BRepAlgoAPI_Cut(rim, pocket).Shape();
  return unify(rim);
}

TopoDS_Edge outer_top_edge(const TopoDS_Shape& shape) {
  TopoDS_Edge found;
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More(); e.Next()) {
    BRepAdaptor_Curve c(TopoDS::Edge(e.Current()));
    if (c.GetType() == GeomAbs_Circle && std::abs(c.Circle().Radius() - kOuter) < 1e-6 &&
        std::abs(c.Circle().Location().Z() - kHeight) < 1e-6) {
      found = TopoDS::Edge(e.Current());
    }
  }
  return found;
}
} // namespace thin_rim

void test_thin_rim_rounding_wider_than_the_rim_is_cut_by_the_pocket_and_the_caps() {
  using namespace thin_rim;
  const TopoDS_Shape rim = body();
  const TopoDS_Edge edge = outer_top_edge(rim);
  CHECK(!edge.IsNull());
  if (edge.IsNull()) {
    return;
  }
  // Radius 1.2: wider than the rim (0.96 to 1.26), the contact on the top
  // runs over the pocket towards the ends; the caps stay whole.
  BRepFilletAPI_MakeFillet rounding(rim);
  NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> selected;
  selected.Add(edge);
  rounding.StopChainsAtTurns(selected);
  rounding.Add(1.2, edge);
  rounding.Build();
  CHECK(rounding.IsDone());
  if (!rounding.IsDone()) {
    return;
  }
  const TopoDS_Shape rounded = rounding.Shape();
  CHECK(BRepCheck_Analyzer(rounded).IsValid());
  // The outer wall's rounding takes a ring off; outside the caps that ring
  // comes off the rim.
  const TopoDS_Shape wall = outer();
  BRepFilletAPI_MakeFillet whole(wall);
  whole.Add(1.2, outer_top_edge(wall));
  whole.Build();
  CHECK(whole.IsDone());
  const TopoDS_Shape ring = BRepAlgoAPI_Cut(wall, whole.Shape()).Shape();
  const TopoDS_Shape tall = caps(-1, kHeight + 1);
  const TopoDS_Shape expected = BRepAlgoAPI_Cut(rim, BRepAlgoAPI_Cut(ring, tall).Shape()).Shape();
  GProp_GProps got, want;
  BRepGProp::VolumeProperties(rounded, got, 1e-9);
  BRepGProp::VolumeProperties(expected, want, 1e-9);
  if (std::abs(got.Mass() - want.Mass()) > 1e-4) {
    std::fprintf(stderr, "thin rim rounding: volume %.7f, expected %.7f\n", got.Mass(), want.Mass());
  }
  CHECK(std::abs(got.Mass() - want.Mass()) < 1e-4);
  CHECK(tori(rounded).size() == 1);
}

// A slot 3 wide through a round boss (radius 6, 10 high), its top rim
// chamfered with 1, the chamfer's lower edges rounded with 1: along the
// slot's walls a cylinder between two planes, round the boss the torus
// between the chamfer's cone and the boss's cylinder (0500). Where they meet,
// at the four corners of four sharp edges, both end on the crease between
// the chamfer's plane and cone and on the walls' crease at the same points,
// and the corner is their intersection. With `shift` the creases' curves on
// the cones are turned by that angle, as a body stored within a tolerance
// has them (1e-5 here); the ends then lie apart by its effect (1e-6), and
// the roundings still meet along their intersection (0503), not across a
// plate of no width.
TopoDS_Shape chamfered_slot_through_a_boss(double shift) {
  const double h = 10.0;
  const TopoDS_Shape boss = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 0), gp::DZ(), gp::DX()), 6, h).Shape();
  const TopoDS_Shape bar = BRepPrimAPI_MakeBox(gp_Pnt(-1.5, -15, 0), 3, 30, h).Shape();
  const TopoDS_Shape body = unify(BRepAlgoAPI_Fuse(boss, bar).Shape());
  BRepFilletAPI_MakeChamfer chamfer(body);
  for (TopExp_Explorer f(body, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    BRepAdaptor_Surface s(face);
    if (s.GetType() == GeomAbs_Plane && std::abs(s.Plane().Location().Z() - h) < 1e-9 &&
        s.Plane().Axis().Direction().IsParallel(gp::DZ(), 1e-12)) {
      for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
        chamfer.Add(1, 1, TopoDS::Edge(e.Current()), face);
      }
    }
  }
  chamfer.Build();
  CHECK(chamfer.IsDone());
  if (!chamfer.IsDone()) {
    return TopoDS_Shape();
  }
  const TopoDS_Shape chamfered = chamfer.Shape();
  if (shift != 0.0) {
    NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<TopoDS_Shape>, TopTools_ShapeMapHasher> faces;
    TopExp::MapShapesAndAncestors(chamfered, TopAbs_EDGE, TopAbs_FACE, faces);
    BRep_Builder builder;
    int moved = 0;
    for (int i = 1; i <= faces.Extent(); ++i) {
      if (faces(i).Extent() != 2) {
        continue;
      }
      const TopoDS_Face& a = TopoDS::Face(faces(i).First());
      const TopoDS_Face& b = TopoDS::Face(faces(i).Last());
      const GeomAbs_SurfaceType ta = BRepAdaptor_Surface(a).GetType(), tb = BRepAdaptor_Surface(b).GetType();
      if (!((ta == GeomAbs_Plane && tb == GeomAbs_Cone) || (ta == GeomAbs_Cone && tb == GeomAbs_Plane))) {
        continue;
      }
      const TopoDS_Edge& crease = TopoDS::Edge(faces.FindKey(i));
      const TopoDS_Face& cone = ta == GeomAbs_Cone ? a : b;
      double first = 0, last = 0;
      occ::handle<Geom2d_Curve> curve =
          occ::down_cast<Geom2d_Curve>(BRep_Tool::CurveOnSurface(crease, cone, first, last)->Copy());
      curve->Translate(gp_Vec2d(shift, 0));
      builder.UpdateEdge(crease, curve, cone, 1e-5);
      ++moved;
    }
    CHECK(moved >= 4);
    CHECK(BRepCheck_Analyzer(chamfered).IsValid());
  }
  return chamfered;
}

// Rounds the edges of `body` that lie at height 9 (the chamfer's lower
// edges) with radius 1; a null shape where that fails.
TopoDS_Shape round_chamfers_lower_edges(const TopoDS_Shape& body) {
  BRepFilletAPI_MakeFillet rounding(body);
  int edges = 0;
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    BRepAdaptor_Curve c(edge);
    const double t0 = c.FirstParameter(), t1 = c.LastParameter();
    if (std::abs(c.Value(t0).Z() - 9) < 1e-9 && std::abs(c.Value(t1).Z() - 9) < 1e-9 &&
        std::abs(c.Value(0.5 * (t0 + t1)).Z() - 9) < 1e-9 && rounding.Contour(edge) == 0) {
      rounding.Add(1, edge);
      ++edges;
    }
  }
  CHECK(edges == 8);
  rounding.Build();
  return rounding.IsDone() ? rounding.Shape() : TopoDS_Shape();
}

void test_slot_and_boss_roundings_meet_along_their_intersection() {
  const TopoDS_Shape exact = round_chamfers_lower_edges(chamfered_slot_through_a_boss(0.0));
  CHECK(!exact.IsNull());
  if (exact.IsNull()) {
    return;
  }
  const double expected = volume_of(exact);
  // 2e-7 of a turn: the ends 1.1e-6 apart on the cones (radius 5.7).
  const TopoDS_Shape body = chamfered_slot_through_a_boss(2e-7);
  const TopoDS_Shape rounded = round_chamfers_lower_edges(body);
  CHECK(!rounded.IsNull());
  if (rounded.IsNull()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(rounded).IsValid());
  CHECK(tori(rounded).size() == 2);
  // No patch at the corners: the roundings' faces are cylinders and tori,
  // and their edges keep the input's tolerance.
  int patches = 0;
  for (TopExp_Explorer f(rounded, TopAbs_FACE); f.More(); f.Next()) {
    const GeomAbs_SurfaceType t = BRepAdaptor_Surface(TopoDS::Face(f.Current())).GetType();
    patches += t == GeomAbs_BSplineSurface || t == GeomAbs_BezierSurface ? 1 : 0;
  }
  double tolerance = 0;
  for (TopExp_Explorer e(rounded, TopAbs_EDGE); e.More(); e.Next()) {
    tolerance = std::max(tolerance, BRep_Tool::Tolerance(TopoDS::Edge(e.Current())));
  }
  if (patches != 0 || tolerance > 1e-4) {
    std::fprintf(stderr, "slot and boss roundings: %d patches, edge tolerance up to %g\n", patches, tolerance);
  }
  CHECK(patches == 0);
  CHECK(tolerance <= 1e-4);
  const double v = volume_of(rounded) - volume_of(body);
  const double want = expected - volume_of(chamfered_slot_through_a_boss(0.0));
  if (std::abs(v - want) > 1e-6) {
    std::fprintf(stderr, "slot and boss roundings: change %.8f, expected %.8f\n", v, want);
  }
  CHECK(std::abs(v - want) < 1e-6);
}
} // namespace

void fillet_rolling_ball_tests() {
  guarded("test_rounding_between_coaxial_cylinder_and_cone_is_a_torus",
          test_rounding_between_coaxial_cylinder_and_cone_is_a_torus);
  guarded("test_rim_rounding_below_a_ledge_over_flats_is_cut_round_them",
          test_rim_rounding_below_a_ledge_over_flats_is_cut_round_them);
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
  guarded("test_thin_rim_rounding_wider_than_the_rim_is_cut_by_the_pocket_and_the_caps",
          test_thin_rim_rounding_wider_than_the_rim_is_cut_by_the_pocket_and_the_caps);
  guarded("test_slot_and_boss_roundings_meet_along_their_intersection",
          test_slot_and_boss_roundings_meet_along_their_intersection);
}

} // namespace test
