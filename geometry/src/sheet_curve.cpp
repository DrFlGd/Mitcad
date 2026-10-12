// SPDX-License-Identifier: MIT
#include "sheet_curve.hpp"

#include <algorithm>
#include <cmath>
#include <stdexcept>

#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI.hxx>

#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Circle.hxx>
#include <Geom2d_Ellipse.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <NCollection_Array1.hxx>
#include <ShapeFix_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Ax2d.hxx>
#include <gp_Circ2d.hxx>
#include <gp_Elips2d.hxx>
#include <gp_Trsf2d.hxx>
#include <gp_Pln.hxx>

#include "util.hpp"

namespace mitcad::geometry::detail {
namespace {

constexpr double kTwoPi = 6.28318530717958647692;

void finite(double value) {
  if (!std::isfinite(value)) {
    throw std::invalid_argument("sheet curve values must be finite");
  }
}

void point(const gp_Pnt2d& value) {
  finite(value.X());
  finite(value.Y());
}

void span(ParamSpan value) {
  finite(value.first);
  finite(value.last);
  if (value.first == value.last) {
    throw std::invalid_argument("sheet curve range must be nonzero");
  }
}

void positive(double value) {
  finite(value);
  if (value <= 0.0) {
    throw std::invalid_argument("sheet curve radius and weights must be positive");
  }
}

occ::handle<Geom2d_BSplineCurve> spline_of(const Segment& s) {
  if (s.poles.size() < 2 || s.knots.size() < 2 || s.knots.size() != s.multiplicities.size() ||
      s.degree < 1 || (!s.weights.empty() && s.weights.size() != s.poles.size())) {
    throw std::invalid_argument("sheet curve has inconsistent B-spline data");
  }
  NCollection_Array1<gp_Pnt2d> poles(1, static_cast<int>(s.poles.size()));
  for (std::size_t i = 0; i < s.poles.size(); ++i) {
    point(s.poles[i]);
    poles.SetValue(static_cast<int>(i) + 1, s.poles[i]);
  }
  NCollection_Array1<double> knots(1, static_cast<int>(s.knots.size()));
  NCollection_Array1<int> multiplicities(1, static_cast<int>(s.knots.size()));
  for (std::size_t i = 0; i < s.knots.size(); ++i) {
    finite(s.knots[i]);
    if ((i > 0 && s.knots[i] <= s.knots[i - 1]) || s.multiplicities[i] <= 0) {
      throw std::invalid_argument("sheet B-spline knots must increase with positive multiplicity");
    }
    knots.SetValue(static_cast<int>(i) + 1, s.knots[i]);
    multiplicities.SetValue(static_cast<int>(i) + 1, s.multiplicities[i]);
  }
  if (s.weights.empty()) {
    return new Geom2d_BSplineCurve(poles, knots, multiplicities, s.degree, s.periodic);
  }
  NCollection_Array1<double> weights(1, static_cast<int>(s.weights.size()));
  for (std::size_t i = 0; i < s.weights.size(); ++i) {
    positive(s.weights[i]);
    weights.SetValue(static_cast<int>(i) + 1, s.weights[i]);
  }
  return new Geom2d_BSplineCurve(poles, weights, knots, multiplicities, s.degree, s.periodic);
}

} // namespace

Curve2 make_sheet_curve(const BoundedCurve2& input) {
  span(input.range);
  const Segment& s = input.geometry;
  Curve2 out;
  switch (s.kind) {
  case SegmentKind::Line: {
    point(s.start);
    point(s.end);
    const double length = s.start.Distance(s.end);
    if (length <= 0.0) {
      throw std::invalid_argument("sheet line has zero length");
    }
    if (std::min(input.range.first, input.range.last) < 0.0 ||
        std::max(input.range.first, input.range.last) > 1.0) {
      throw std::invalid_argument("sheet line range lies outside its bounded support");
    }
    out.curve = new Geom2d_Line(s.start, gp_Dir2d(gp_Vec2d(s.start, s.end)));
    out.parameter_scale = length;
    break;
  }
  case SegmentKind::Circle:
  case SegmentKind::Arc:
    point(s.center);
    positive(s.radius);
    out.curve = new Geom2d_Circle(gp_Circ2d(gp_Ax2d(s.center, gp_Dir2d(1, 0)), s.radius));
    break;
  case SegmentKind::Ellipse:
  case SegmentKind::EllipseArc:
    point(s.center);
    positive(s.radius);
    positive(s.minor_radius);
    finite(s.rotation);
    if (s.radius < s.minor_radius) {
      throw std::invalid_argument("sheet ellipse major radius is smaller than its minor radius");
    }
    out.curve = new Geom2d_Ellipse(gp_Elips2d(
        gp_Ax2d(s.center, gp_Dir2d(std::cos(s.rotation), std::sin(s.rotation))),
        s.radius, s.minor_radius));
    break;
  case SegmentKind::BSpline:
    out.curve = spline_of(s);
    if (!out.curve->IsPeriodic() &&
        (std::min(input.range.first, input.range.last) < out.curve->FirstParameter() ||
         std::max(input.range.first, input.range.last) > out.curve->LastParameter())) {
      throw std::invalid_argument("sheet spline range lies outside its support");
    }
    break;
  }
  if (out.curve.IsNull()) {
    throw std::invalid_argument("unsupported sheet curve support");
  }
  out.native_range = {out.native_parameter(input.range.first), out.native_parameter(input.range.last)};
  if (out.curve->IsPeriodic() &&
      std::abs(out.native_range.last - out.native_range.first) > out.curve->Period() + 1.0e-12) {
    throw std::invalid_argument("sheet periodic edge spans more than one period");
  }
  return out;
}

BoundedCurve2 export_sheet_curve(const occ::handle<Geom2d_Curve>& input, ParamSpan range) {
  span(range);
  if (input.IsNull()) {
    throw std::invalid_argument("sheet edge has no planar support");
  }
  occ::handle<Geom2d_Curve> curve = input;
  while (const auto trimmed = occ::handle<Geom2d_TrimmedCurve>::DownCast(curve)) {
    curve = trimmed->BasisCurve();
  }
  BoundedCurve2 out;
  out.range = range;
  Segment& s = out.geometry;
  if (const auto line = occ::handle<Geom2d_Line>::DownCast(curve)) {
    s.kind = SegmentKind::Line;
    s.start = line->Value(range.first);
    s.end = line->Value(range.last);
    out.range = {0, 1};
  } else if (const auto circle = occ::handle<Geom2d_Circle>::DownCast(curve)) {
    const gp_Circ2d support = circle->Circ2d();
    s.kind = std::abs(std::abs(range.last - range.first) - kTwoPi) < 1.0e-12
                 ? SegmentKind::Circle : SegmentKind::Arc;
    s.center = support.Location();
    s.radius = support.Radius();
    const gp_Dir2d x = support.XAxis().Direction();
    const double rotation = std::atan2(x.Y(), x.X());
    const double sign = support.IsDirect() ? 1.0 : -1.0;
    out.range = {rotation + sign * range.first, rotation + sign * range.last};
    s.start_angle = out.range.first;
    s.end_angle = out.range.last;
  } else if (const auto ellipse = occ::handle<Geom2d_Ellipse>::DownCast(curve)) {
    const gp_Elips2d support = ellipse->Elips2d();
    s.kind = std::abs(std::abs(range.last - range.first) - kTwoPi) < 1.0e-12
                 ? SegmentKind::Ellipse : SegmentKind::EllipseArc;
    s.center = support.Location();
    s.radius = support.MajorRadius();
    s.minor_radius = support.MinorRadius();
    const gp_Dir2d x = support.XAxis().Direction();
    s.rotation = std::atan2(x.Y(), x.X());
    const double sign = support.IsDirect() ? 1.0 : -1.0;
    out.range = {sign * range.first, sign * range.last};
    s.start_angle = out.range.first;
    s.end_angle = out.range.last;
  } else if (auto spline = occ::handle<Geom2d_BSplineCurve>::DownCast(curve)) {
    if (spline->IsPeriodic()) {
      // The value representation must own a bounded support: downstream
      // nonperiodic evaluators cannot interpret a partial periodic domain.
      // Segment preserves the rational curve and its native parameter,
      // including intervals crossing the original periodic origin.
      spline = occ::handle<Geom2d_BSplineCurve>::DownCast(spline->Copy());
      spline->Segment(std::min(range.first, range.last), std::max(range.first, range.last));
    }
    s.kind = SegmentKind::BSpline;
    s.degree = spline->Degree();
    s.periodic = spline->IsPeriodic();
    for (int i = 1; i <= spline->NbPoles(); ++i) {
      s.poles.push_back(spline->Pole(i));
      if (spline->IsRational()) {
        s.weights.push_back(spline->Weight(i));
      }
    }
    for (int i = 1; i <= spline->NbKnots(); ++i) {
      s.knots.push_back(spline->Knot(i));
      s.multiplicities.push_back(spline->Multiplicity(i));
    }
  } else {
    throw std::invalid_argument("unsupported exact planar sheet edge support");
  }
  return out;
}

void validate_sheet_isometry(const Isometry2& t) {
  point(t.origin);
  finite(t.x_axis.X());
  finite(t.x_axis.Y());
  finite(t.y_axis.X());
  finite(t.y_axis.Y());
  if (std::abs(t.x_axis.SquareMagnitude() - 1.0) > 1.0e-10 ||
      std::abs(t.y_axis.SquareMagnitude() - 1.0) > 1.0e-10 ||
      std::abs(t.x_axis.Dot(t.y_axis)) > 1.0e-10) {
    throw std::invalid_argument("sheet chart transform must be an isometry");
  }
}

occ::handle<Geom2d_Curve> transform_sheet_curve(const occ::handle<Geom2d_Curve>& curve,
                                              const Isometry2& input, bool inverse) {
  validate_sheet_isometry(input);
  gp_Trsf2d t;
  t.SetValues(input.x_axis.X(), input.y_axis.X(), input.origin.X(),
              input.x_axis.Y(), input.y_axis.Y(), input.origin.Y());
  if (inverse) {
    t.Invert();
  }
  return occ::handle<Geom2d_Curve>::DownCast(curve->Transformed(t));
}

SheetFace make_sheet_face(const MaterialRegion& region, const Isometry2& transform, double tolerance) {
  if (!std::isfinite(tolerance) || tolerance <= 0.0 || region.loops.empty()) {
    throw std::invalid_argument("sheet material region requires an outer loop and positive tolerance");
  }
  validate_sheet_isometry(transform);
  const gp_Pln plane(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1));
  std::vector<std::pair<gp_Pnt, TopoDS_Vertex>> vertices;
  auto vertex = [&](const gp_Pnt2d& p) {
    const gp_Pnt point(p.X(), p.Y(), 0);
    const TopoDS_Vertex* found = nullptr;
    for (const auto& item : vertices) {
      if (item.first.Distance(point) <= tolerance) {
        if (found) {
          throw std::invalid_argument("sheet material endpoint has ambiguous vertex correspondence");
        }
        found = &item.second;
      }
    }
    if (found) {
      return *found;
    }
    TopoDS_Vertex created = BRepBuilderAPI_MakeVertex(point).Vertex();
    vertices.emplace_back(point, created);
    return created;
  };
  SheetFace result;
  std::vector<TopoDS_Wire> wires;
  for (const auto& loop : region.loops) {
    if (loop.edges.empty()) {
      throw std::invalid_argument("sheet material loop is empty");
    }
    BRepBuilderAPI_MakeWire wire;
    std::vector<TopoDS_Edge> edges;
    gp_Pnt2d start;
    gp_Pnt2d previous;
    bool first = true;
    for (const auto& input : loop.edges) {
      throw_if_cancelled();
      if (input.piece.curve.empty()) {
        throw std::invalid_argument("sheet boundary has no source identity");
      }
      auto curve = make_sheet_curve(input.curve);
      curve.curve = transform_sheet_curve(curve.curve, transform);
      const gp_Pnt2d a = curve.curve->Value(curve.native_range.first);
      const gp_Pnt2d b = curve.curve->Value(curve.native_range.last);
      if (first) {
        start = a;
        first = false;
      } else if (a.Distance(previous) > tolerance) {
        throw std::invalid_argument("sheet material loop has a gap beyond tolerance");
      }
      previous = b;
      const double low = std::min(curve.native_range.first, curve.native_range.last);
      const double high = std::max(curve.native_range.first, curve.native_range.last);
      const TopoDS_Vertex v0 = vertex(curve.curve->Value(low));
      const TopoDS_Vertex v1 = vertex(curve.curve->Value(high));
      BRepBuilderAPI_MakeEdge maker(GeomAPI::To3d(curve.curve, plane), v0, v1, low, high);
      if (!maker.IsDone()) {
        throw std::runtime_error("sheet boundary edge could not be built");
      }
      TopoDS_Edge edge = maker.Edge();
      if (curve.native_range.first > curve.native_range.last) {
        edge.Reverse();
      }
      wire.Add(edge);
      if (!wire.IsDone()) {
        throw std::invalid_argument("sheet material edges do not form an ordered wire");
      }
      edges.push_back(wire.Edge());
    }
    if (start.Distance(previous) > tolerance) {
      throw std::invalid_argument("sheet material loop is open beyond tolerance");
    }
    wires.push_back(wire.Wire());
    result.edges.push_back(std::move(edges));
  }
  BRepBuilderAPI_MakeFace maker(plane, wires.front(), true);
  for (std::size_t i = 1; i < wires.size(); ++i) {
    maker.Add(wires[i]);
  }
  if (!maker.IsDone()) {
    throw std::invalid_argument("sheet material face could not be built");
  }
  ShapeFix_Face orient(maker.Face());
  orient.FixOrientation();
  result.face = orient.Face();
  require_valid(result.face, "sheet material region");
  GProp_GProps props;
  BRepGProp::SurfaceProperties(result.face, props);
  if (props.Mass() <= tolerance * tolerance) {
    throw std::invalid_argument("sheet material region has no bounded area");
  }
  return result;
}

} // namespace mitcad::geometry::detail
