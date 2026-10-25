// SPDX-License-Identifier: MIT
#include "blend_surfaces.hpp"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <stdexcept>
#include <string>

#include <Adaptor3d_CurveOnSurface.hxx>
#include <Adaptor3d_TopolTool.hxx>
#include <Approx_CurveOnSurface.hxx>
#include <BRepBlend_AppFunc.hxx>
#include <BRepBlend_AppFuncRst.hxx>
#include <BRepBlend_AppSurface.hxx>
#include <BRepBlend_ConstRadInv.hxx>
#include <BRepBlend_Line.hxx>
#include <BRepBlend_SurfCurvConstRadInv.hxx>
#include <BRepBlend_SurfPointConstRadInv.hxx>
#include <BRepBlend_SurfRstConstRad.hxx>
#include <BRepBlend_SurfRstLineBuilder.hxx>
#include <BRepBlend_Walking.hxx>
#include <BSplCLib.hxx>
#include <BlendFunc_ConstRad.hxx>
#include <BlendFunc_ConstRadInv.hxx>
#include <Blend_Point.hxx>
#include <ChFiDS_ElSpine.hxx>
#include <GeomAPI.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <Geom2dAPI_ProjectPointOnCurve.hxx>
#include <Geom2dAdaptor_Curve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Circle.hxx>
#include <Geom_Conic.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <ElSLib.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_Array2.hxx>
#include <Standard_Failure.hxx>
#include <gp.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Lin.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Vec.hxx>
#include <math_Vector.hxx>

namespace mitcad::brep {
namespace {

// Accuracy of the walked sections' contacts (mm).
constexpr double kWalkTolerance = 1e-6;
// The approximation's tolerances: 3D (mm), parameter space, angular.
constexpr double kApproxTolerance = 1e-5;
constexpr double kTolerance2d = 1e-7;
constexpr double kAngularTolerance = 1e-2;
// Largest error of the approximation the import accepts (mm, its geometric
// tolerance).
constexpr double kMaxApproxError = 0.01;
// How far (mm) the spine's start may be from the ball's centre there (its
// distance from a support from the radius). The files store the spine, the
// path of the centre, as an approximation (an offset intersection curve's
// B-spline), some starting a few micrometres off (mitcad#639: up to 0.008
// mm); the walk solves the true ball in each plane normal to the spine all
// the same, so only the surface's parametrization follows the spine. Like
// a stored approximation of the surface, it is taken within the import's
// tolerance; farther off, the spine is not this blend's.
constexpr double kMaxSpineOff = kMaxApproxError;

std::string text(double v) {
  char buf[64];
  std::snprintf(buf, sizeof buf, "%.6g", v);
  return buf;
}

// The point of the surface nearest p (the foot of the perpendicular from
// p) and the surface's parameters there.
bool nearest_on_surface(const Handle(Geom_Surface)& surface, const gp_Pnt& p, double& u, double& v,
                        gp_Pnt& at) {
  GeomAPI_ProjectPointOnSurf projection(p, surface);
  if (!projection.IsDone() || projection.NbPoints() == 0) {
    return false;
  }
  projection.LowerDistanceParameters(u, v);
  at = surface->Value(u, v);
  return true;
}

// Whether p lies on the side of the surface's natural normal (du x dv) at
// (u, v).
bool on_normal_side(const Handle(Geom_Surface)& surface, double u, double v, const gp_Pnt& p) {
  gp_Pnt at;
  gp_Vec du;
  gp_Vec dv;
  surface->D1(u, v, at, du, dv);
  return du.Crossed(dv).Dot(gp_Vec(at, p)) > 0.0;
}

// The surface for the walk, its parameters about (u, v): a periodic
// direction more than a period either side, so that the contact crossing
// the seam is not taken for leaving the surface's domain, nor the contact
// of a ball rolled once round (whose parameter there runs a period, give or
// take the rounding of a spine that is an approximation, mitcad#639).
Handle(GeomAdaptor_Surface) around(const Handle(Geom_Surface)& surface, double u, double v) {
  double u0 = 0.0;
  double u1 = 0.0;
  double v0 = 0.0;
  double v1 = 0.0;
  surface->Bounds(u0, u1, v0, v1);
  if (surface->IsUPeriodic()) {
    u0 = u - 1.25 * surface->UPeriod();
    u1 = u + 1.25 * surface->UPeriod();
  }
  if (surface->IsVPeriodic()) {
    v0 = v - 1.25 * surface->VPeriod();
    v1 = v + 1.25 * surface->VPeriod();
  }
  return new GeomAdaptor_Surface(surface, u0, u1, v0, v1);
}

// The spine's guide: the ball's centre runs along it, the mid-point of its
// contacts in the plane normal to it.
Handle(ChFiDS_ElSpine) guide_of(const Handle(Geom_Curve)& spine, double v0, double v1) {
  Handle(ChFiDS_ElSpine) guide = new ChFiDS_ElSpine();
  guide->SetCurve(spine);
  guide->FirstParameter(v0);
  guide->LastParameter(v1);
  return guide;
}

// The parity of OCCT's choice that makes the sections the minor arcs from
// the contact a to the contact b about the centre c of the ball at the
// spine's parameter t: odd turns the section plane's normal (the spine's
// tangent) round, so the arc runs from a to b positively about it (OCCT
// takes the angle in ]-pi/2, 3pi/2] about the plane's normal; a negative
// one past a quarter turn becomes the major arc).
int parity(const Handle(Geom_Curve)& spine, double t, const gp_Pnt& c, const gp_Pnt& a, const gp_Pnt& b) {
  gp_Pnt at;
  gp_Vec tangent;
  spine->D1(t, at, tangent);
  return tangent.Dot(gp_Vec(c, a).Crossed(gp_Vec(c, b))) > 0.0 ? 1 : 0;
}

// Whether the section of a walked point is the minor arc between the
// contacts: its middle lies towards the contacts from the ball's centre.
template <class Func>
bool minor_arc(Func& func, const Blend_Point& point, const gp_Pnt& centre, const gp_Pnt& a, const gp_Pnt& b,
               double radius) {
  int nb_poles = 0;
  int nb_knots = 0;
  int degree = 0;
  int nb_poles_2d = 0;
  func.GetShape(nb_poles, nb_knots, degree, nb_poles_2d);
  NCollection_Array1<gp_Pnt> poles(1, nb_poles);
  NCollection_Array1<gp_Pnt2d> poles_2d(1, std::max(nb_poles_2d, 2));
  NCollection_Array1<double> weights(1, nb_poles);
  NCollection_Array1<double> knots(1, nb_knots);
  NCollection_Array1<int> mults(1, nb_knots);
  func.Knots(knots);
  func.Mults(mults);
  func.Section(point, poles, poles_2d, weights);
  const Handle(Geom_BSplineCurve) section = new Geom_BSplineCurve(poles, weights, knots, mults, degree);
  const gp_Pnt middle = section->Value(0.5 * (section->FirstParameter() + section->LastParameter()));
  gp_Vec towards = gp_Vec(centre, a).Normalized() + gp_Vec(centre, b).Normalized();
  if (towards.Magnitude() <= gp::Resolution()) {
    return true; // a half circle: either sense
  }
  const gp_Pnt expected = centre.Translated(towards.Normalized() * radius);
  return middle.Distance(expected) < 0.5 * radius;
}

// The approximation's surface, u reparametrized to [0, 1].
Handle(Geom_BSplineSurface) approximated(const Handle(Approx_SweepFunction)& function, double first, double last) {
  BRepBlend_AppSurface approx(function, first, last, kApproxTolerance, kTolerance2d, kAngularTolerance, GeomAbs_C1,
                              11, 1000);
  if (!approx.IsDone()) {
    throw std::runtime_error("rolling-ball blend: the approximation of its sections failed");
  }
  if (approx.MaxErrorOnSurf() > kMaxApproxError) {
    throw std::runtime_error("rolling-ball blend: the approximation is " + text(approx.MaxErrorOnSurf()) +
                             " mm off its sections");
  }
  NCollection_Array1<double> uknots = approx.SurfUKnots();
  BSplCLib::Reparametrize(0.0, 1.0, uknots);
  return new Geom_BSplineSurface(approx.SurfPoles(), approx.SurfWeights(), uknots, approx.SurfVKnots(),
                                 approx.SurfUMults(), approx.SurfVMults(), approx.UDegree(), approx.VDegree());
}

// Every walked section must be the minor arc between its contacts.
template <class Func, class Contacts>
void check_sections(Func& func, const Handle(BRepBlend_Line)& line, const Handle(Geom_Curve)& spine, double radius,
                    Contacts contacts) {
  for (int i = 1; i <= line->NbPoints(); ++i) {
    const Blend_Point& q = line->Point(i);
    gp_Pnt a;
    gp_Pnt b;
    contacts(q, a, b);
    if (!minor_arc(func, q, spine->Value(q.Parameter()), a, b, radius)) {
      throw std::runtime_error("rolling-ball blend: its section at " + text(q.Parameter()) +
                               " is the major arc");
    }
  }
}

void check_walk(const Handle(BRepBlend_Line)& line, double v0, double v1) {
  if (line.IsNull() || line->NbPoints() < 2) {
    throw std::runtime_error("rolling-ball blend: OCCT's walking found no sections");
  }
  const double first = line->Point(1).Parameter();
  const double last = line->Point(line->NbPoints()).Parameter();
  const double slack = 1e-6 * std::max(1.0, v1 - v0);
  if (first > v0 + slack || last < v1 - slack) {
    throw std::runtime_error("rolling-ball blend: the walk covers [" + text(first) + ", " + text(last) +
                             "] of the needed [" + text(v0) + ", " + text(v1) + "]");
  }
}

// Between two surfaces: BlendFunc_ConstRad walked by BRepBlend_Walking.
Handle(Geom_Surface) between_surfaces(const Handle(Geom_Surface)& s1, const Handle(Geom_Surface)& s2,
                                      const Handle(Geom_Curve)& spine, double radius, double v0, double v1, double n0,
                                      double n1) {
  // The start: the contacts of the ball centred on the spine's start.
  const gp_Pnt centre = spine->Value(v0);
  math_Vector start(1, 4);
  gp_Pnt p1;
  gp_Pnt p2;
  if (!nearest_on_surface(s1, centre, start(1), start(2), p1) ||
      !nearest_on_surface(s2, centre, start(3), start(4), p2)) {
    throw std::runtime_error("rolling-ball blend: no contact of the ball at the spine's start");
  }
  const Handle(GeomAdaptor_Surface) a1 = around(s1, start(1), start(2));
  const Handle(GeomAdaptor_Surface) a2 = around(s2, start(3), start(4));
  const Handle(Adaptor3d_TopolTool) d1 = new Adaptor3d_TopolTool(a1);
  const Handle(Adaptor3d_TopolTool) d2 = new Adaptor3d_TopolTool(a2);
  const Handle(ChFiDS_ElSpine) guide = guide_of(spine, v0, v1);
  for (const gp_Pnt& p : {p1, p2}) {
    if (std::abs(p.Distance(centre) - radius) > kMaxSpineOff) {
      throw std::runtime_error("rolling-ball blend: the spine's start is " + text(p.Distance(centre)) +
                               " mm from a support, not the radius " + text(radius));
    }
  }

  // OCCT's choice of the sides the centre lies on: 1 on the normal's side
  // of both (its radii negative), 3 the second's only, 5 neither, 7 the
  // first's only; one more for the minor arcs about the spine's tangent.
  const bool front1 = on_normal_side(s1, start(1), start(2), centre);
  const bool front2 = on_normal_side(s2, start(3), start(4), centre);
  const int choice = (front1 ? (front2 ? 1 : 7) : (front2 ? 3 : 5)) + parity(spine, v0, centre, p1, p2);
  BlendFunc_ConstRad func(a1, a2, guide);
  func.Set(BlendFunc_Rational);
  func.Set(radius, choice);
  BlendFunc_ConstRadInv inverse(a1, a2, guide);
  inverse.Set(radius, choice);

  BRepBlend_Walking walk(a1, a2, d1, d2, guide);
  walk.Check2d(false);
  const double span = v1 - v0;
  walk.Perform(func, inverse, v0, v1, 0.05 * span, kWalkTolerance, 1e-7 * span, start, 1e-4 * std::max(1.0, radius),
               true);
  if (!walk.IsDone()) {
    throw std::runtime_error("rolling-ball blend: OCCT's walking along the spine failed");
  }
  Handle(BRepBlend_Line) line = walk.Line();
  check_walk(line, n0, n1);
  check_sections(func, line, spine, radius, [](const Blend_Point& q, gp_Pnt& a, gp_Pnt& b) {
    a = q.PointOnS1();
    b = q.PointOnS2();
  });
  const Handle(BRepBlend_AppFunc) sections = new BRepBlend_AppFunc(line, func, kApproxTolerance, kTolerance2d);
  return approximated(sections, line->Point(1).Parameter(), line->Point(line->NbPoints()).Parameter());
}

// A curve as a restriction of a surface containing it: a line or a conic on
// a plane through it.
bool as_restriction(Handle(Geom_Curve) curve, const gp_Pnt& centre, const gp_Pnt& contact,
                    Handle(Geom_Plane)& plane, Handle(Geom2d_Curve)& restriction) {
  gp_Ax3 frame;
  if (const Handle(Geom_Circle) circle = Handle(Geom_Circle)::DownCast(curve); !circle.IsNull()) {
    // Its parameter's seam opposite the contact, so the walk does not
    // meet the restriction's end.
    const gp_Circ c = circle->Circ();
    gp_Vec out(c.Location(), contact);
    out -= gp_Vec(c.Axis().Direction()) * out.Dot(gp_Vec(c.Axis().Direction()));
    if (out.Magnitude() > gp::Resolution()) {
      curve = new Geom_Circle(gp_Ax2(c.Location(), c.Axis().Direction(), gp_Dir(out.Reversed())), c.Radius());
    }
    frame = gp_Ax3(c.Position());
  } else if (const Handle(Geom_Conic) conic = Handle(Geom_Conic)::DownCast(curve); !conic.IsNull()) {
    frame = gp_Ax3(conic->Position());
  } else if (const Handle(Geom_Line) line = Handle(Geom_Line)::DownCast(curve); !line.IsNull()) {
    // The plane through the line and the ball's centre.
    const gp_Lin l = line->Lin();
    gp_Vec across(l.Location(), centre);
    across -= gp_Vec(l.Direction()) * across.Dot(gp_Vec(l.Direction()));
    if (across.Magnitude() <= gp::Resolution()) {
      return false;
    }
    frame = gp_Ax3(l.Location(), gp_Dir(gp_Vec(l.Direction()).Crossed(across)), l.Direction());
  } else {
    return false;
  }
  plane = new Geom_Plane(frame);
  restriction = GeomAPI::To2d(curve, plane->Pln());
  return !restriction.IsNull();
}

// The side (OCCT's SetRefSide: 1 the left of the restriction's direction in
// the plane's parameters, -1 the right) of the restriction at its parameter
// w away from the point c's foot in the plane.
int away_from_centre(const Handle(Geom2dAdaptor_Curve)& rst, const Handle(GeomAdaptor_Surface)& plane, double w,
                     const gp_Pnt& c) {
  gp_Pnt2d p;
  gp_Vec2d t;
  rst->D1(w, p, t);
  gp_Pnt at;
  gp_Vec du;
  gp_Vec dv;
  plane->D1(p.X(), p.Y(), at, du, dv);
  const gp_Vec left = du * -t.Y() + dv * t.X();
  return left.Dot(gp_Vec(at, c)) > 0.0 ? -1 : 1;
}

// Between a surface and a curve: the rolling ball of
// BRepBlend_SurfRstConstRad walked by BRepBlend_SurfRstLineBuilder, the
// curve a restriction of a plane through it. u runs from the surface's
// contact to the curve's.
Handle(Geom_Surface) surface_and_curve(const Handle(Geom_Surface)& s, const Handle(Geom_Curve)& c,
                                       const Handle(Geom_Curve)& spine, double radius, double v0, double v1,
                                       double n0, double n1) {
  const gp_Pnt centre = spine->Value(v0);
  math_Vector start(1, 3);
  gp_Pnt on_surface;
  if (!nearest_on_surface(s, centre, start(1), start(2), on_surface)) {
    throw std::runtime_error("rolling-ball blend: no contact of the ball at the spine's start");
  }
  GeomAPI_ProjectPointOnCurve on_curve(centre, c);
  if (on_curve.NbPoints() == 0) {
    throw std::runtime_error("rolling-ball blend: no contact of the ball on its curve at the spine's start");
  }
  const gp_Pnt contact = on_curve.NearestPoint();
  Handle(Geom_Plane) plane;
  Handle(Geom2d_Curve) c2d;
  if (!as_restriction(c, centre, contact, plane, c2d)) {
    throw std::runtime_error("rolling-ball blend on a curve that is not a line or a conic");
  }
  const Handle(GeomAdaptor_Surface) as = around(s, start(1), start(2));
  const Handle(GeomAdaptor_Surface) ap = new GeomAdaptor_Surface(plane);
  double pu = 0.0;
  double pv = 0.0;
  ElSLib::Parameters(plane->Pln(), contact, pu, pv);
  Geom2dAPI_ProjectPointOnCurve on_restriction(gp_Pnt2d(pu, pv), c2d);
  if (on_restriction.NbPoints() == 0) {
    throw std::runtime_error("rolling-ball blend: the contact is not on its curve");
  }
  start(3) = on_restriction.LowerDistanceParameter();
  // On a closed curve (a circle) the contact may go once round it, either
  // way (a ball rolling round the whole circle, mitcad#622): its parameter
  // then runs a period either side of the start, not to the curve's ends
  // (where OCCT's walk would look for a vertex of the restriction).
  const Handle(Geom2dAdaptor_Curve) rst =
      c2d->IsPeriodic() ? new Geom2dAdaptor_Curve(c2d, start(3) - 1.25 * c2d->Period(), start(3) + 1.25 * c2d->Period())
                        : new Geom2dAdaptor_Curve(c2d);
  const Handle(Adaptor3d_TopolTool) ds = new Adaptor3d_TopolTool(as);
  const Handle(Adaptor3d_TopolTool) dp = new Adaptor3d_TopolTool(ap);
  const Handle(ChFiDS_ElSpine) guide = guide_of(spine, v0, v1);
  for (const gp_Pnt& p : {on_surface, contact}) {
    if (std::abs(p.Distance(centre) - radius) > kMaxSpineOff) {
      throw std::runtime_error("rolling-ball blend: the spine's start is " + text(p.Distance(centre)) +
                               " mm from a support, not the radius " + text(radius));
    }
  }

  BRepBlend_SurfRstConstRad func(as, ap, rst, guide);
  func.Set(ap, rst);
  // The plane is only the curve's carrier, no face of the body: the ball
  // must not be taken for leaving the curve onto it. Without a side OCCT
  // decides that from the plane's normal against the section's tangent at
  // the curve, which is a matter of rounding where the ball's centre lies in
  // the plane (a ball on a rim level with its centre, mitcad#639; the plane
  // through a line and the centre): the walk then stopped, at its start or on
  // the way. The plane's face is said to lie on the curve's side away from
  // the centre, so the ball never reaches into it.
  func.SetRefSide(away_from_centre(rst, ap, start(3), centre));
  func.Set(BlendFunc_Rational);
  // OCCT's choice: 1 with the centre on the surface's normal side, 3
  // without; one more for the minor arcs about the spine's tangent.
  const int choice =
      (on_normal_side(s, start(1), start(2), centre) ? 1 : 3) + parity(spine, v0, centre, on_surface, contact);
  func.Set(radius, choice);
  const Handle(Adaptor3d_CurveOnSurface) curve_on_plane = new Adaptor3d_CurveOnSurface(rst, ap);
  BRepBlend_SurfCurvConstRadInv inverse_on_curve(as, curve_on_plane, guide);
  inverse_on_curve.SetRollingBall(true);
  inverse_on_curve.Set(radius, choice);
  BRepBlend_SurfPointConstRadInv inverse_at_point(as, guide);
  inverse_at_point.Set(radius, choice);
  BRepBlend_ConstRadInv inverse(as, ap, guide);
  inverse.Set(false, rst);
  inverse.Set(radius, choice);

  BRepBlend_SurfRstLineBuilder walk(as, ds, ap, rst, dp);
  const double span = v1 - v0;
  walk.Perform(func, inverse, inverse_at_point, inverse_on_curve, v0, v1, 0.05 * span, kWalkTolerance, kTolerance2d,
               1e-7 * span, start, 1e-4 * std::max(1.0, radius), true);
  if (!walk.IsDone()) {
    throw std::runtime_error("rolling-ball blend: OCCT's walking along the spine failed");
  }
  Handle(BRepBlend_Line) line = walk.Line();
  check_walk(line, n0, n1);
  check_sections(func, line, spine, radius, [](const Blend_Point& q, gp_Pnt& a, gp_Pnt& b) {
    a = q.PointOnS();
    b = q.PointOnC();
  });
  const Handle(BRepBlend_AppFuncRst) sections =
      new BRepBlend_AppFuncRst(line, func, kApproxTolerance, kTolerance2d);
  return approximated(sections, line->Point(1).Parameter(), line->Point(line->NbPoints()).Parameter());
}

// Whether [v0, v1] is one whole period of a closed spine.
bool round_whole_spine(const Handle(Geom_Curve)& spine, double v0, double v1) {
  if (!spine->IsPeriodic()) {
    return false;
  }
  const double period = spine->Period();
  return std::abs((v1 - v0) - period) <= 1e-9 * period;
}

// A blend walked round a whole closed spine, closed: its two end sections
// are one (the walk's ends, within its tolerance), made the same and the
// surface periodic in v, so that a face across its seam has its seam edge
// on both ends (mitcad#622).
Handle(Geom_Surface) closed_round(const Handle(Geom_Surface)& surface) {
  const Handle(Geom_BSplineSurface) b = Handle(Geom_BSplineSurface)::DownCast(surface);
  if (b.IsNull()) {
    throw std::runtime_error("rolling-ball blend round a closed spine: not a B-spline");
  }
  const int nv = b->NbVPoles();
  double gap = 0.0;
  for (int i = 1; i <= b->NbUPoles(); ++i) {
    const gp_Pnt first = b->Pole(i, 1);
    const gp_Pnt last = b->Pole(i, nv);
    gap = std::max(gap, first.Distance(last));
    const gp_Pnt middle(0.5 * (first.XYZ() + last.XYZ()));
    const double weight = 0.5 * (b->Weight(i, 1) + b->Weight(i, nv));
    b->SetPole(i, 1, middle, weight);
    b->SetPole(i, nv, middle, weight);
  }
  if (gap > kMaxApproxError) {
    throw std::runtime_error("rolling-ball blend round a closed spine: its end sections are " + text(gap) +
                             " mm apart");
  }
  b->SetVPeriodic();
  return b;
}

} // namespace

Handle(Geom_Surface) rolling_ball_surface(const BlendSupport& first, const BlendSupport& second,
                                          const Handle(Geom_Curve)& spine, double radius, double v0, double v1,
                                          double n0, double n1) {
  if (spine.IsNull() || !(radius > 0.0) || !(v1 > v0) || !(n0 >= v0) || !(n1 <= v1) || !(n1 > n0)) {
    throw std::runtime_error("rolling-ball blend: no spine, radius or range");
  }
  // What failed is said with what was blended.
  auto kind = [](const BlendSupport& s) {
    return std::string(s.surface.IsNull() ? s.curve->DynamicType()->Name() : s.surface->DynamicType()->Name());
  };
  const std::string context = " (radius " + text(radius) + " between a " + kind(first) + " and a " +
                              kind(second) + ", spine a " + spine->DynamicType()->Name() + " over [" +
                              text(v0) + ", " + text(v1) + "])";
  try {
    Handle(Geom_Surface) out;
    if (!first.surface.IsNull() && !second.surface.IsNull()) {
      out = between_surfaces(first.surface, second.surface, spine, radius, v0, v1, n0, n1);
    } else if (!first.surface.IsNull() && !second.curve.IsNull()) {
      out = surface_and_curve(first.surface, second.curve, spine, radius, v0, v1, n0, n1);
    } else if (!first.curve.IsNull() && !second.surface.IsNull()) {
      // Walked from the surface: u then runs from the second support.
      const Handle(Geom_Surface) s = surface_and_curve(second.surface, first.curve, spine, radius, v0, v1, n0, n1);
      const Handle(Geom_BSplineSurface) b = Handle(Geom_BSplineSurface)::DownCast(s);
      b->UReverse();
      out = b;
    }
    if (!out.IsNull()) {
      return round_whole_spine(spine, v0, v1) ? closed_round(out) : out;
    }
  } catch (const Standard_Failure& e) {
    throw std::runtime_error(std::string("rolling-ball blend: OCCT: ") + e.what() + context);
  } catch (const std::exception& e) {
    throw std::runtime_error(e.what() + context);
  }
  throw std::runtime_error("rolling-ball blend between two curves");
}

Handle(Geom_Curve) curve_on_surface(const Handle(Geom_Surface)& surface, const Handle(Geom2d_Curve)& pcurve) {
  const Handle(Geom2dAdaptor_Curve) c = new Geom2dAdaptor_Curve(pcurve);
  const Handle(GeomAdaptor_Surface) s = new GeomAdaptor_Surface(surface);
  Approx_CurveOnSurface approx(c, s, pcurve->FirstParameter(), pcurve->LastParameter(), 1e-6);
  approx.Perform(1000, 14, GeomAbs_C2, true, false);
  if (!approx.IsDone() || !approx.HasResult()) {
    throw std::runtime_error("curve on a surface: its approximation failed");
  }
  return approx.Curve3d();
}

} // namespace mitcad::brep
