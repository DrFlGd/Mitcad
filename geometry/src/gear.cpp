// SPDX-License-Identifier: MIT
#include "mitcad/geometry/gear.hpp"

#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <BRepAlgoAPI_Common.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GeomConvert.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_Array2.hxx>
#include <NCollection_HArray1.hxx>
#include <Precision.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp_Ax1.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

#include "history.hpp"
#include "mitcad/geometry/boolean.hpp"
#include "mitcad/geometry/naming.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

// The screw motion's cos and sin of the turn over a step are interpolated
// at steps of this many radians: a point at radius r is off its exact
// path by about 5/384 r h^4 (1e-9 mm at r = 100).
constexpr double kTurnStep = 0.01;
// Faces whose edges come from the same curves meet within rounding.
constexpr double kSewing = 1.0e-6;

// The frame turned by `turn` about its normal and moved `start` along it.
Frame moved(const Frame& frame, double turn, double start) {
  Frame out = frame;
  const gp_Vec n(frame.normal());
  out.origin = frame.origin.Translated(n * start);
  const gp_Vec x = gp_Vec(frame.x_axis) * std::cos(turn) + gp_Vec(frame.y_axis) * std::sin(turn);
  out.x_axis = gp_Dir(x);
  out.y_axis = gp_Dir(n.Crossed(x));
  return out;
}

// The edge's curve as a clamped B-spline, as the edge runs.
occ::handle<Geom_BSplineCurve> edge_spline(const TopoDS_Edge& edge) {
  double first = 0.0;
  double last = 0.0;
  const occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, first, last);
  if (curve.IsNull()) {
    throw std::runtime_error("a gear profile's edge has no curve");
  }
  occ::handle<Geom_BSplineCurve> spline =
      GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(curve, first, last));
  if (spline.IsNull()) {
    throw std::runtime_error("a gear profile's edge has no B-spline form");
  }
  if (spline->IsPeriodic()) {
    spline->SetNotPeriodic();
  }
  return spline;
}

// cos and sin of the turn from `from` by `twist`, and the step's share
// (0 to 1), as one B-spline over [0, 1].
occ::handle<Geom_BSplineCurve> turn_basis(double from, double twist) {
  const int count = std::max(2, static_cast<int>(std::ceil(std::abs(twist) / kTurnStep)) + 1);
  const int points = twist == 0.0 ? 2 : std::max(count, 4);
  occ::handle<NCollection_HArray1<gp_Pnt>> values = new NCollection_HArray1<gp_Pnt>(1, points);
  occ::handle<NCollection_HArray1<double>> params = new NCollection_HArray1<double>(1, points);
  for (int i = 0; i < points; ++i) {
    const double v = static_cast<double>(i) / (points - 1);
    const double a = from + twist * v;
    values->SetValue(i + 1, gp_Pnt(std::cos(a), std::sin(a), v));
    params->SetValue(i + 1, v);
  }
  GeomAPI_Interpolate interpolate(values, params, false, 1.0e-12);
  interpolate.Perform();
  if (!interpolate.IsDone()) {
    throw std::runtime_error("the gear's helix could not be built");
  }
  return interpolate.Curve();
}

// The face a curve sweeps over one step: each pole moved along the step's
// screw motion.
TopoDS_Face swept_face(const occ::handle<Geom_BSplineCurve>& curve, const Frame& frame, const GearStep& step) {
  const occ::handle<Geom_BSplineCurve> basis = turn_basis(0.0, step.twist);
  const gp_Vec n(frame.normal());
  const gp_Vec x(frame.x_axis);
  const gp_Vec y(frame.y_axis);
  const int nu = curve->NbPoles();
  const int nv = basis->NbPoles();
  NCollection_Array2<gp_Pnt> poles(1, nu, 1, nv);
  NCollection_Array2<double> weights(1, nu, 1, nv);
  for (int k = 1; k <= nu; ++k) {
    const gp_Vec p(frame.origin, curve->Pole(k));
    const double a = p.Dot(x);
    const double b = p.Dot(y);
    const double c = p.Dot(n);
    for (int j = 1; j <= nv; ++j) {
      const gp_Pnt t = basis->Pole(j);
      const double cs = t.X();
      const double sn = t.Y();
      const double v = t.Z();
      const gp_Vec q = n * (c + step.length * v) + x * (step.shift * v) +
                       x * (a * cs - b * sn) + y * (a * sn + b * cs);
      poles.SetValue(k, j, frame.origin.Translated(q));
      weights.SetValue(k, j, curve->Weight(k));
    }
  }
  occ::handle<Geom_BSplineSurface> surface;
  if (curve->IsRational()) {
    surface = new Geom_BSplineSurface(poles, weights, curve->Knots(), basis->Knots(), curve->Multiplicities(),
                                      basis->Multiplicities(), curve->Degree(), basis->Degree());
  } else {
    surface = new Geom_BSplineSurface(poles, curve->Knots(), basis->Knots(), curve->Multiplicities(),
                                      basis->Multiplicities(), curve->Degree(), basis->Degree());
  }
  BRepBuilderAPI_MakeFace face(surface, Precision::Confusion());
  if (!face.IsDone()) {
    throw std::runtime_error("a face of the gear could not be built");
  }
  return face.Face();
}

// The motion of a whole step: turned by `twist` about the axis, moved by
// `length` along it and `shift` along x.
gp_Trsf step_motion(const Frame& frame, double twist, double length, double shift) {
  gp_Trsf turn;
  turn.SetRotation(gp_Ax1(frame.origin, frame.normal()), twist);
  gp_Trsf move;
  move.SetTranslation(gp_Vec(frame.normal()) * length + gp_Vec(frame.x_axis) * shift);
  return move * turn;
}

ShapePtr prism(const GearSweepSpec& spec, const ProfileFace& profile, const GearStep& step) {
  const gp_Vec along = gp_Vec(spec.frame.normal()) * step.length + gp_Vec(spec.frame.x_axis) * step.shift;
  BRepPrimAPI_MakePrism maker(profile.face, along);
  detail::build(maker);
  if (!maker.IsDone()) {
    throw std::runtime_error("the gear's teeth could not be swept");
  }
  // Undercut teeth and many B-spline flanks: checked as the screwed ones.
  detail::require_valid(maker.Shape(), "gear");
  detail::FaceNamer namer(maker.Shape());
  for (const auto& [edge, name] : profile.edges) {
    namer.generated(maker, edge, spec.feature + ':' + name);
  }
  namer.add(maker.FirstShape(), face_name(spec.feature, spec.start_name));
  namer.add(maker.LastShape(), face_name(spec.feature, spec.end_name));
  namer.finish();
  return namer.shape();
}

ShapePtr screwed(const GearSweepSpec& spec, const ProfileFace& profile) {
  // The faces, step by step.
  std::vector<std::pair<TopoDS_Face, std::string>> faces;
  double turned = 0.0;
  double height = 0.0;
  double shifted = 0.0;
  for (const GearStep& step : spec.steps) {
    for (const auto& [edge, name] : profile.edges) {
      // The edge's curve where the step starts.
      occ::handle<Geom_BSplineCurve> curve = edge_spline(edge);
      const gp_Trsf before = step_motion(spec.frame, turned, height, shifted);
      curve = occ::down_cast<Geom_BSplineCurve>(curve->Transformed(before));
      faces.emplace_back(swept_face(curve, spec.frame, step), spec.feature + ':' + name);
    }
    turned += step.twist;
    height += step.length;
    shifted += step.shift;
  }
  const TopoDS_Face start = profile.face;
  BRepBuilderAPI_Transform end_maker(profile.face, step_motion(spec.frame, turned, height, shifted), true);
  const TopoDS_Face end = TopoDS::Face(end_maker.Shape());

  BRepBuilderAPI_Sewing sewing(kSewing);
  sewing.Add(start);
  sewing.Add(end);
  for (const auto& [face, name] : faces) {
    sewing.Add(face);
  }
  sewing.Perform();
  const TopoDS_Shape sewn = sewing.SewedShape();
  TopoDS_Shell shell;
  int shells = 0;
  for (TopExp_Explorer e(sewn, TopAbs_SHELL); e.More(); e.Next()) {
    shell = TopoDS::Shell(e.Current());
    ++shells;
  }
  if (shells != 1) {
    throw std::runtime_error("the gear's faces do not close into one shell");
  }
  BRepBuilderAPI_MakeSolid maker(shell);
  if (!maker.IsDone()) {
    throw std::runtime_error("the gear's faces do not close into a solid");
  }
  TopoDS_Solid solid = maker.Solid();
  BRepLib::OrientClosedSolid(solid);
  if (!detail::is_valid(solid)) {
    throw std::runtime_error("the gear's solid is not valid");
  }
  const auto image = [&](const TopoDS_Shape& face) {
    return sewing.IsModified(face) ? sewing.Modified(face) : face;
  };
  detail::FaceNamer namer(solid);
  const auto add = [&](const TopoDS_Shape& face, const std::string& name) {
    const TopoDS_Shape sewn_face = image(face);
    for (TopExp_Explorer f(solid, TopAbs_FACE); f.More(); f.Next()) {
      if (f.Current().IsSame(sewn_face)) {
        namer.add(f.Current(), name);
        return;
      }
    }
  };
  for (const auto& [face, name] : faces) {
    add(face, name);
  }
  add(start, face_name(spec.feature, spec.start_name));
  add(end, face_name(spec.feature, spec.end_name));
  namer.finish();
  return namer.shape();
}

} // namespace

ShapePtr gear_sweep(const GearSweepSpec& spec) {
  if (spec.steps.empty()) {
    throw std::invalid_argument("a gear sweep needs a step");
  }
  for (const GearStep& step : spec.steps) {
    detail::require_positive("gear width", step.length);
    detail::require_finite("gear twist", step.twist);
    detail::require_finite("gear shift", step.shift);
  }
  return detail::run("gear", [&] {
    const Frame frame = moved(spec.frame, spec.turn, spec.start);
    const ProfileFace profile = make_profile(frame, spec.region);
    if (spec.steps.size() == 1 && spec.steps.front().twist == 0.0) {
      return prism(spec, profile, spec.steps.front());
    }
    GearSweepSpec at = spec;
    at.frame = Frame{frame.origin, spec.frame.x_axis, spec.frame.y_axis};
    return screwed(at, profile);
  });
}

ShapePtr gear_cone(const GearConeSpec& spec) {
  detail::require_positive("bevel gear apex", spec.apex);
  if (!(spec.scale > 0.0 && spec.scale < 1.0)) {
    throw std::invalid_argument("a bevel gear's toe must be smaller than its heel");
  }
  return detail::run("bevel gear", [&] {
    const ProfileFace profile = make_profile(spec.frame, spec.region);
    const gp_Pnt apex = spec.frame.origin.Translated(gp_Vec(spec.frame.normal()) * spec.apex);
    const TopoDS_Wire heel = BRepTools::OuterWire(profile.face);
    gp_Trsf scale;
    scale.SetScale(apex, spec.scale);
    BRepBuilderAPI_Transform toe(heel, scale, true);
    BRepOffsetAPI_ThruSections sections(true, true, 1.0e-7);
    sections.CheckCompatibility(false);
    sections.AddWire(heel);
    sections.AddWire(TopoDS::Wire(toe.Shape()));
    detail::build(sections);
    if (!sections.IsDone()) {
      throw std::runtime_error("the bevel gear's teeth could not be built");
    }
    detail::FaceNamer teeth_namer(sections.Shape());
    for (const auto& [edge, name] : profile.edges) {
      const TopoDS_Shape face = sections.GeneratedFace(edge);
      if (!face.IsNull()) {
        teeth_namer.add(face, spec.feature + ':' + name);
      }
    }
    teeth_namer.add(sections.FirstShape(), face_name(spec.feature, "back"));
    teeth_namer.add(sections.LastShape(), face_name(spec.feature, "front"));
    teeth_namer.finish();
    const ShapePtr teeth = teeth_namer.shape();

    // The blank: its loop in the plane through the axis and the frame's x
    // axis, turned about the axis.
    Frame half;
    half.origin = spec.frame.origin;
    half.x_axis = spec.frame.x_axis;
    half.y_axis = spec.frame.normal();
    Region region;
    region.name = "blank";
    region.loops.push_back(spec.blank);
    const ProfileFace blank_face = make_profile(half, region);
    BRepPrimAPI_MakeRevol revol(blank_face.face, gp_Ax1(spec.frame.origin, spec.frame.normal()));
    detail::build(revol);
    if (!revol.IsDone()) {
      throw std::runtime_error("the bevel gear's blank could not be built");
    }
    detail::FaceNamer blank_namer(revol.Shape());
    for (const auto& [edge, name] : blank_face.edges) {
      blank_namer.generated(revol, edge, spec.feature + ':' + name);
    }
    blank_namer.finish();
    const ShapePtr blank = blank_namer.shape();

    const BooleanResult common = boolean(BooleanOp::Intersect, {teeth.get()}, *blank);
    if (common.pieces.size() != 1) {
      throw std::runtime_error("the bevel gear's blank leaves " + std::to_string(common.pieces.size()) +
                               " pieces of its teeth");
    }
    return common.pieces.front().shape;
  });
}

} // namespace mitcad::geometry
