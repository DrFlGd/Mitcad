// SPDX-License-Identifier: MIT
#include "mitcad/geometry/emboss.hpp"

#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <BRepBndLib.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GeomAPI.hxx>
#include <GeomConvert.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_DataMap.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Lin.hxx>
#include <gp_Pln.hxx>

#include "history.hpp"
#include "sweep.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

constexpr double kTwoPi = 2.0 * 3.14159265358979323846;

// The cylinder the profiles wrap onto and the radii they grow between.
struct Wrap {
  gp_Ax3 frame; // on the axis: Z the axis, X the sketch side's radial direction
  double radius = 0.0;
  double to = 0.0;
  double tolerance = 0.0; // of what is built on it
  occ::handle<Geom_Surface> base;
  occ::handle<Geom_Surface> top;
};

// The sketch's frame in flat coordinates: s across the axis (along the
// frame's Y direction, axis × radial), z along it, 0.
Frame flat_frame(const Frame& frame, const gp_Ax3& cylinder) {
  const gp_Vec across(cylinder.YDirection());
  const gp_Vec along(cylinder.Direction());
  const gp_Vec origin(cylinder.Location(), frame.origin);
  const auto flat = [&](const gp_Dir& d) {
    return gp_Dir(gp_Vec(d).Dot(across), gp_Vec(d).Dot(along), 0.0);
  };
  Frame result;
  result.origin = gp_Pnt(origin.Dot(across), origin.Dot(along), 0.0);
  result.x_axis = flat(frame.x_axis);
  result.y_axis = flat(frame.y_axis);
  return result;
}

// A curve in the cylinders' (angle, height) parameters over its range.
struct UvCurve {
  occ::handle<Geom2d_Curve> curve;
  double first = 0.0;
  double last = 0.0;
};

// A flat point (s, z) as the cylinders' parameters (s / radius, z).
gp_Pnt2d uv_of(const gp_Pnt2d& p, double radius) { return gp_Pnt2d(p.X() / radius, p.Y()); }

// The edge of a flat profile face (in the plane z = 0) wrapped: its curve
// in the cylinders' parameters. Lines stay lines; other curves are their
// B-splines with the poles moved, which an affine map of a (rational)
// B-spline is exactly.
UvCurve uv_curve(const TopoDS_Edge& edge, double radius) {
  double first = 0.0;
  double last = 0.0;
  TopLoc_Location location;
  occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, location, first, last);
  if (curve.IsNull()) {
    throw std::runtime_error("a profile edge has no curve");
  }
  if (!location.IsIdentity()) {
    curve = occ::down_cast<Geom_Curve>(curve->Transformed(location.Transformation()));
  }
  const occ::handle<Geom2d_Curve> flat = GeomAPI::To2d(curve, gp_Pln(gp::XOY()));
  occ::handle<Geom2d_Curve> basis = flat;
  for (occ::handle<Geom2d_TrimmedCurve> trimmed = occ::down_cast<Geom2d_TrimmedCurve>(basis); !trimmed.IsNull();
       trimmed = occ::down_cast<Geom2d_TrimmedCurve>(basis)) {
    basis = trimmed->BasisCurve();
  }
  if (basis->IsKind(STANDARD_TYPE(Geom2d_Line))) {
    const gp_Pnt2d a = uv_of(flat->Value(first), radius);
    const gp_Pnt2d b = uv_of(flat->Value(last), radius);
    const double length = a.Distance(b);
    if (!(length > gp::Resolution())) {
      throw std::invalid_argument("a profile edge has no length");
    }
    return {new Geom2d_Line(a, gp_Dir2d(gp_Vec2d(a, b))), 0.0, length};
  }
  const occ::handle<Geom2d_BSplineCurve> spline =
      Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(flat, first, last));
  for (int i = 1; i <= spline->NbPoles(); ++i) {
    spline->SetPole(i, uv_of(spline->Pole(i), radius));
  }
  return {spline, spline->FirstParameter(), spline->LastParameter()};
}

// The point moved radially from the axis by the factor k.
gp_Pnt scaled(const gp_Ax3& frame, double k, const gp_Pnt& p) {
  const gp_Vec a(frame.Direction());
  const gp_Vec d(frame.Location(), p);
  const gp_Vec along = a * d.Dot(a);
  return frame.Location().Translated(along + (d - along) * k);
}

// A B-spline with its knots moved linearly onto [first, last].
occ::handle<Geom_BSplineCurve> reparametrized(const occ::handle<Geom_BSplineCurve>& spline, double first,
                                              double last) {
  NCollection_Array1<double> knots(spline->Knots());
  const double a = spline->FirstParameter();
  const double b = spline->LastParameter();
  for (int i = knots.Lower(); i <= knots.Upper(); ++i) {
    knots(i) = first + (knots(i) - a) * (last - first) / (b - a);
  }
  const NCollection_Array1<gp_Pnt>& poles = spline->Poles();
  const NCollection_Array1<int>& multiplicities = spline->Multiplicities();
  const NCollection_Array1<double>* weights = spline->Weights();
  if (spline->IsRational() && weights != nullptr) {
    return new Geom_BSplineCurve(poles, *weights, knots, multiplicities, spline->Degree(), spline->IsPeriodic());
  }
  return new Geom_BSplineCurve(poles, knots, multiplicities, spline->Degree(), spline->IsPeriodic());
}

// The edge on the base cylinder and its radial image on the top cylinder,
// both with the same parameters: the walls between them are ruled along
// the radii.
struct WrappedEdge {
  TopoDS_Edge base;
  TopoDS_Edge top;
  UvCurve uv;
};

WrappedEdge wrapped_edge(const Wrap& wrap, const UvCurve& uv, const TopoDS_Vertex (&base)[2],
                         const TopoDS_Vertex (&top)[2]) {
  BRep_Builder builder;
  WrappedEdge result;
  result.uv = uv;
  builder.MakeEdge(result.base);
  builder.UpdateEdge(result.base, uv.curve, wrap.base, TopLoc_Location(), wrap.tolerance);
  builder.Add(result.base, base[0].Oriented(TopAbs_FORWARD));
  builder.Add(result.base, base[1].Oriented(TopAbs_REVERSED));
  builder.Range(result.base, uv.first, uv.last);
  if (!BRepLib::BuildCurve3d(result.base, wrap.tolerance, GeomAbs_C1, 14, 10000)) {
    throw std::runtime_error("an edge of the profile could not be laid onto the cylinder");
  }
  double first = 0.0;
  double last = 0.0;
  occ::handle<Geom_Curve> curve = BRep_Tool::Curve(result.base, first, last);
  for (occ::handle<Geom_TrimmedCurve> trimmed = occ::down_cast<Geom_TrimmedCurve>(curve); !trimmed.IsNull();
       trimmed = occ::down_cast<Geom_TrimmedCurve>(curve)) {
    curve = trimmed->BasisCurve();
  }
  const double k = wrap.to / wrap.radius;
  const gp_Dir axis = wrap.frame.Direction();
  occ::handle<Geom_Curve> image;
  const occ::handle<Geom_Line> line = occ::down_cast<Geom_Line>(curve);
  const occ::handle<Geom_Circle> circle = occ::down_cast<Geom_Circle>(curve);
  if (!line.IsNull() && line->Lin().Direction().IsParallel(axis, 1.0e-9)) {
    image = new Geom_Line(scaled(wrap.frame, k, line->Lin().Location()), line->Lin().Direction());
  } else if (!circle.IsNull() && circle->Circ().Axis().Direction().IsParallel(axis, 1.0e-9)) {
    gp_Ax2 position = circle->Position();
    position.SetLocation(scaled(wrap.frame, k, position.Location()));
    image = new Geom_Circle(position, circle->Radius() * k);
  } else {
    occ::handle<Geom_BSplineCurve> spline = occ::down_cast<Geom_BSplineCurve>(curve);
    if (spline.IsNull()) {
      spline = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(curve, first, last));
    } else {
      spline = occ::down_cast<Geom_BSplineCurve>(spline->Copy());
    }
    if (std::abs(spline->FirstParameter() - first) > 1.0e-12 || std::abs(spline->LastParameter() - last) > 1.0e-12) {
      // The base edge takes the same curve (SameParameter below mends the
      // parameters of its curve on the cylinder).
      spline = reparametrized(spline, first, last);
      builder.UpdateEdge(result.base, occ::down_cast<Geom_Curve>(spline->Copy()), BRep_Tool::Tolerance(result.base));
    }
    for (int i = 1; i <= spline->NbPoles(); ++i) {
      spline->SetPole(i, scaled(wrap.frame, k, spline->Pole(i)));
    }
    image = spline;
  }
  const double tolerance = BRep_Tool::Tolerance(result.base) * std::max(1.0, k);
  builder.MakeEdge(result.top, image, tolerance);
  builder.UpdateEdge(result.top, uv.curve, wrap.top, TopLoc_Location(), tolerance);
  builder.Add(result.top, top[0].Oriented(TopAbs_FORWARD));
  builder.Add(result.top, top[1].Oriented(TopAbs_REVERSED));
  builder.Range(result.top, first, last);
  return result;
}

using EdgeMap = NCollection_DataMap<TopoDS_Shape, WrappedEdge, TopTools_ShapeMapHasher>;
using VertexMap = NCollection_DataMap<TopoDS_Shape, std::pair<TopoDS_Vertex, TopoDS_Vertex>, TopTools_ShapeMapHasher>;

// The signed area the wrapped wire encloses in the cylinders' parameters.
double uv_area(const TopoDS_Wire& wire, const TopoDS_Face& face, const EdgeMap& edges) {
  std::vector<gp_Pnt2d> points;
  for (BRepTools_WireExplorer e(wire, face); e.More(); e.Next()) {
    const UvCurve& uv = edges.Find(e.Current()).uv;
    constexpr int kSamples = 16;
    for (int i = 0; i < kSamples; ++i) {
      double t = static_cast<double>(i) / kSamples;
      if (e.Current().Orientation() == TopAbs_REVERSED) {
        t = 1.0 - t;
      }
      points.push_back(uv.curve->Value(uv.first + t * (uv.last - uv.first)));
    }
  }
  double area = 0.0;
  for (std::size_t i = 0; i < points.size(); ++i) {
    const gp_Pnt2d& p = points[i];
    const gp_Pnt2d& q = points[(i + 1) % points.size()];
    area += p.X() * q.Y() - q.X() * p.Y();
  }
  return area / 2.0;
}

// The orientation of `edge` in the face as it lies in the shell.
TopAbs_Orientation orientation_in(const TopoDS_Face& face, const TopoDS_Edge& edge) {
  for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
    if (e.Current().IsSame(edge)) {
      return e.Current().Orientation();
    }
  }
  throw std::runtime_error("a wall of the emboss does not meet its profile's face");
}

// The solid of one profile face wrapped and grown.
ShapePtr wrapped_solid(const std::string& feature, const Wrap& wrap, const detail::SweepFace& profile) {
  const TopoDS_Face& planar = profile.face;
  BRep_Builder builder;
  VertexMap vertices;
  for (TopExp_Explorer e(planar, TopAbs_VERTEX); e.More(); e.Next()) {
    if (vertices.IsBound(e.Current())) {
      continue;
    }
    const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(e.Current()));
    const gp_Pnt2d uv = uv_of(gp_Pnt2d(p.X(), p.Y()), wrap.radius);
    TopoDS_Vertex base;
    TopoDS_Vertex top;
    builder.MakeVertex(base, wrap.base->Value(uv.X(), uv.Y()), wrap.tolerance);
    builder.MakeVertex(top, wrap.top->Value(uv.X(), uv.Y()), wrap.tolerance * std::max(1.0, wrap.to / wrap.radius));
    vertices.Bind(e.Current(), {base, top});
  }
  EdgeMap edges;
  for (TopExp_Explorer e(planar, TopAbs_EDGE); e.More(); e.Next()) {
    if (edges.IsBound(e.Current())) {
      continue;
    }
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    TopoDS_Vertex first;
    TopoDS_Vertex last;
    TopExp::Vertices(edge, first, last);
    const auto& [base0, top0] = vertices.Find(first);
    const auto& [base1, top1] = vertices.Find(last);
    const TopoDS_Vertex base[2] = {base0, base1};
    const TopoDS_Vertex top[2] = {top0, top1};
    edges.Bind(edge, wrapped_edge(wrap, uv_curve(edge, wrap.radius), base, top));
  }

  // The loops on both cylinders, edge for edge in the same order.
  std::vector<std::pair<TopoDS_Wire, TopoDS_Wire>> loops;
  const TopoDS_Wire outer = BRepTools::OuterWire(planar);
  bool flip = false;
  for (TopExp_Explorer w(planar, TopAbs_WIRE); w.More(); w.Next()) {
    const TopoDS_Wire& wire = TopoDS::Wire(w.Current());
    TopoDS_Wire base;
    TopoDS_Wire top;
    builder.MakeWire(base);
    builder.MakeWire(top);
    for (BRepTools_WireExplorer e(wire, planar); e.More(); e.Next()) {
      const WrappedEdge& wrapped = edges.Find(e.Current());
      builder.Add(base, wrapped.base.Oriented(e.Current().Orientation()));
      builder.Add(top, wrapped.top.Oriented(e.Current().Orientation()));
    }
    base.Closed(true);
    top.Closed(true);
    if (wire.IsSame(outer)) {
      // The outer loop runs counter-clockwise in the parameters of a face
      // whose normal points out from the axis.
      flip = uv_area(wire, planar, edges) < 0.0;
    }
    loops.emplace_back(base, top);
  }
  TopoDS_Face base;
  TopoDS_Face top;
  builder.MakeFace(base, wrap.base, wrap.tolerance);
  builder.MakeFace(top, wrap.top, wrap.tolerance);
  for (const auto& [b, t] : loops) {
    builder.Add(base, flip ? TopoDS::Wire(b.Reversed()) : b);
    builder.Add(top, flip ? TopoDS::Wire(t.Reversed()) : t);
  }
  // Both point out from the axis; the solid's faces point out of it.
  if (wrap.to > wrap.radius) {
    base.Reverse();
  } else {
    top.Reverse();
  }

  TopoDS_Shell shell;
  builder.MakeShell(shell);
  builder.Add(shell, base);
  builder.Add(shell, top);
  std::vector<std::pair<TopoDS_Face, std::string>> walls;
  for (const auto& [b, t] : loops) {
    BRepOffsetAPI_ThruSections sections(false, true, wrap.tolerance);
    sections.CheckCompatibility(false);
    sections.AddWire(b);
    sections.AddWire(t);
    detail::build(sections);
    if (!sections.IsDone()) {
      throw std::runtime_error("the walls of the emboss could not be built");
    }
    for (TopExp_Explorer e(b, TopAbs_EDGE); e.More(); e.Next()) {
      const TopoDS_Shape generated = sections.GeneratedFace(e.Current());
      if (generated.IsNull() || generated.ShapeType() != TopAbs_FACE) {
        throw std::runtime_error("an edge of the profile has no wall");
      }
      TopoDS_Face wall = TopoDS::Face(generated);
      const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
      // Each edge runs one way in one face and the other way in the other.
      if (orientation_in(wall, edge) == orientation_in(base, edge)) {
        wall.Reverse();
      }
      builder.Add(shell, wall);
      for (const auto& [source, name] : profile.edges) {
        if (edges.Find(source).base.IsSame(edge)) {
          walls.emplace_back(wall, name);
        }
      }
    }
  }
  shell.Closed(true);
  TopoDS_Solid solid;
  builder.MakeSolid(solid);
  builder.Add(solid, shell);
  BRepLib::SameParameter(solid, wrap.tolerance, true);
  BRepLib::OrientClosedSolid(solid);

  detail::FaceNamer namer(solid);
  for (const std::string& name : detail::cap_names(feature, "start", profile)) {
    namer.add(base, name);
  }
  for (const std::string& name : detail::cap_names(feature, "end", profile)) {
    namer.add(top, name);
  }
  for (const auto& [wall, name] : walls) {
    namer.add(wall, name);
  }
  return namer.shape();
}

} // namespace

ShapePtr wrapped_emboss(const EmbossWrapSpec& spec) {
  if (spec.regions.empty()) {
    throw std::invalid_argument("no profiles to emboss");
  }
  detail::require_positive("cylinder radius", spec.radius);
  detail::require_positive("emboss radius", spec.to_radius);
  if (!(std::abs(spec.to_radius - spec.radius) > 1.0e-7)) {
    throw std::invalid_argument("the emboss has no depth");
  }
  const gp_Dir axis = spec.axis.Direction();
  gp_Vec normal(spec.frame.normal());
  if (std::abs(normal.Dot(gp_Vec(axis))) > 1.0e-6) {
    throw std::invalid_argument("to wrap onto a cylinder, the sketch plane must be parallel to its axis");
  }
  return detail::run("emboss", [&] {
    // Towards the sketch plane from the axis.
    if (gp_Vec(spec.axis.Location(), spec.frame.origin).Dot(normal) < 0.0) {
      normal.Reverse();
    }
    normal -= gp_Vec(axis) * normal.Dot(gp_Vec(axis));
    Wrap wrap;
    wrap.frame = gp_Ax3(spec.axis.Location(), axis, gp_Dir(normal));
    wrap.radius = spec.radius;
    wrap.to = spec.to_radius;
    wrap.tolerance = 1.0e-7 * std::max(1.0, std::max(spec.radius, spec.to_radius));
    wrap.base = new Geom_CylindricalSurface(wrap.frame, spec.radius);
    wrap.top = new Geom_CylindricalSurface(wrap.frame, spec.to_radius);

    const Frame flat = flat_frame(spec.frame, wrap.frame);
    std::vector<ShapePtr> parts;
    for (const detail::SweepFace& face : detail::region_faces(spec.feature, flat, spec.regions)) {
      Bnd_Box box;
      BRepBndLib::Add(face.face, box);
      double xmin = 0.0, ymin = 0.0, zmin = 0.0, xmax = 0.0, ymax = 0.0, zmax = 0.0;
      box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
      if (xmax - xmin >= kTwoPi * spec.radius) {
        throw std::invalid_argument("the profiles reach around the cylinder: they are wider than its circumference");
      }
      parts.push_back(wrapped_solid(spec.feature, wrap, face));
    }
    return detail::finished(*detail::fused(parts, "the emboss"), "the emboss");
  });
}

} // namespace mitcad::geometry
