// SPDX-License-Identifier: MIT
#include "mitcad/geometry/thread.hpp"

#include <algorithm>
#include <cmath>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <utility>
#include <vector>

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepTools.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRep_Builder.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Surface.hxx>
#include <NCollection_HArray1.hxx>
#include <NCollection_List.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_GTrsf2d.hxx>
#include <gp_Pnt2d.hxx>

#include "helix_brep.hpp"
#include "history.hpp"
#include "local_boolean.hpp"
#include "mitcad/geometry/reference.hpp"
#include "sweep.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

constexpr double kTwoPi = 6.28318530717958647692;
constexpr double kTan30 = 0.57735026918962576451;
constexpr double kCos30 = 0.86602540378443864676;
// Radii closer than this are the same.
constexpr double kSame = 1.0e-6;
// The narrowest crest a flank clearance may leave, in pitches.
constexpr double kMinCrest = 0.02;
// How near the screw surfaces of the grooves' faces keep to the screw
// motion (mm), finer than the sweeps a turn at a time (on_screw_surfaces).
constexpr double kScrewExact = 1.0e-7;

// One face's thread: the face's cylinder and the profile on it.
struct Thread {
  // On the axis at the face's low end, the axis, and the direction the
  // thread's angle is measured from.
  gp_Pnt origin;
  gp_Dir along;
  gp_Dir reference;
  double face_radius = 0.0;
  double length = 0.0;
  bool internal = false;

  double pitch = 0.0;
  double pitch_radius = 0.0;
  double major_radius = 0.0;
  double minor_radius = 0.0;
  bool right_handed = true;
  double angle = 0.0;
  // How far the flank clearance moves each flank along the axis, into the
  // tooth (the radii above already carry the radial clearance).
  double relief = 0.0;

  double crest() const { return internal ? minor_radius : major_radius; }
  double root() const { return internal ? major_radius : minor_radius; }

  // Half the width of the groove (the space between the teeth) at a
  // radius: a quarter pitch at the pitch diameter, wider towards the crest
  // along flanks at 30 degrees, and wider by the flank relief.
  double groove_half(double radius) const {
    return pitch / 4.0 + relief + (internal ? pitch_radius - radius : radius - pitch_radius) * kTan30;
  }

  // How far from the pitch diameter the flanks of a tooth meet: H/2 =
  // 0.433 P, less where the flank relief has thinned the tooth.
  double reach() const { return 0.433 * pitch - relief / kTan30; }

  // The middle of the groove at the pitch diameter, along the axis from
  // the origin, at an angle about the axis from the reference direction.
  double groove_centre(double theta) const {
    const double turn = (right_handed ? 1.0 : -1.0) * pitch * (theta - angle) / kTwoPi;
    return (internal ? 0.75 : 0.25) * pitch + turn;
  }

  gp_Dir direction(double theta) const {
    return reference.Rotated(gp_Ax1(origin, along), theta);
  }

  gp_Pnt point(double s, double theta, double radius) const {
    return origin.Translated(gp_Vec(along) * s + gp_Vec(direction(theta)) * radius);
  }
};

// The direction a thread's angle is measured from: the X axis projected
// across the axis (Y for axes near X).
gp_Dir reference_direction(const gp_Dir& along) {
  const gp_Vec x = std::abs(along.X()) > 0.9 ? gp_Vec(0, 1, 0) : gp_Vec(1, 0, 0);
  return gp_Dir(x - gp_Vec(along) * x.Dot(gp_Vec(along)));
}

// A solid tube about the thread's axis between two radii (a cylinder from
// radius 0) and two positions along the axis.
TopoDS_Shape tube(const Thread& t, double inner, double outer, double from, double to) {
  const gp_Ax2 frame(t.origin.Translated(gp_Vec(t.along) * from), t.along, t.reference);
  TopoDS_Shape shape = BRepPrimAPI_MakeCylinder(frame, outer, to - from).Shape();
  if (inner > kSame) {
    BRepAlgoAPI_Cut cut(shape, BRepPrimAPI_MakeCylinder(frame, inner, to - from).Shape());
    if (!cut.IsDone() || cut.HasErrors()) {
      throw std::runtime_error("the thread's tube could not be built");
    }
    shape = cut.Shape();
  }
  return shape;
}

// The groove over several turns from the angle `theta`: its axial section
// between two radii swept along the helix of its middle, a spine of one
// edge per turn. (A spine of one edge over many turns makes OCCT's cut
// remove nothing or everything; separate sweeps per turn touch each other
// at their ends, which the local boolean cannot take.)
// The common of a shape and a tool, which a cancel request stops inside
// (detail::build): the grooves' common is seconds of the thread's time.
TopoDS_Shape common_of(const TopoDS_Shape& shape, const TopoDS_Shape& tool, const char* failure) {
  BRepAlgoAPI_Common common;
  NCollection_List<TopoDS_Shape> arguments;
  arguments.Append(shape);
  NCollection_List<TopoDS_Shape> tools;
  tools.Append(tool);
  common.SetArguments(arguments);
  common.SetTools(tools);
  detail::build(common);
  if (!common.IsDone() || common.HasErrors()) {
    throw std::runtime_error(failure);
  }
  return common.Shape();
}

TopoDS_Shape groove_sweep(const Thread& t, double theta, int turns, double low, double high) {
  const gp_Dir start_direction = t.direction(theta);
  const double centre = t.groove_centre(theta);
  const gp_Pnt base = t.origin.Translated(gp_Vec(t.along) * centre);
  const occ::handle<Geom_CylindricalSurface> surface =
      new Geom_CylindricalSurface(gp_Ax3(base, t.along, start_direction), t.pitch_radius);
  // On the unrolled cylinder the helix is a line: a turn of 2 pi about the
  // axis rises one pitch, or falls one for a left-hand thread. (Turning the
  // other way instead, or mirroring a right-hand groove, gives sweeps that
  // OCCT's boolean cuts wrongly.)
  const occ::handle<Geom2d_Line> line =
      new Geom2d_Line(gp_Pnt2d(0.0, 0.0), gp_Dir2d(kTwoPi, t.right_handed ? t.pitch : -t.pitch));
  const double turn = std::hypot(kTwoPi, t.pitch);
  BRepBuilderAPI_MakeWire wire;
  for (int k = 0; k < turns; ++k) {
    BRepBuilderAPI_MakeEdge helix(line, surface, turn * k, turn * (k + 1));
    if (!helix.IsDone()) {
      throw std::runtime_error("the thread's helix could not be built");
    }
    TopoDS_Edge edge = helix.Edge();
    BRepLib::BuildCurves3d(edge, 1.0e-6, GeomAbs_C1, 14, 30);
    wire.Add(edge);
  }
  const TopoDS_Wire spine = wire.Wire();

  // The section in the axial plane where the helix starts.
  const auto corner = [&](double side, double radius) {
    return base.Translated(gp_Vec(t.along) * (side * t.groove_half(radius)) + gp_Vec(start_direction) * radius);
  };
  BRepBuilderAPI_MakePolygon section(corner(-1.0, low), corner(1.0, low), corner(1.0, high), corner(-1.0, high),
                                     true);
  BRepOffsetAPI_MakePipeShell pipe(spine);
  // A fixed binormal along the axis keeps the section in the axial plane.
  pipe.SetMode(t.along);
  // Fine enough for the boolean with the body and for the volume.
  pipe.SetTolerance(1.0e-6, 1.0e-6, 1.0e-4);
  pipe.SetMaxSegments(32);
  pipe.Add(section.Wire());
  detail::build(pipe);
  if (!pipe.IsDone() || !pipe.MakeSolid()) {
    throw std::runtime_error("the thread's groove could not be swept");
  }
  return pipe.Shape();
}

// The surfaces groove_sweep's faces approximate a turn at a time: each
// edge of the groove's section moved by the thread's screw motion over all
// the turns (detail::screwed_surfaces with the same section, from the same
// start: for a left-hand groove its other end, where the screw motion
// rises from), as the bodies stored in files keep threads.
std::vector<occ::handle<Geom_Surface>> screw_surfaces(const Thread& t, double theta, int turns, double low,
                                                      double high) {
  const double start = t.right_handed ? theta : theta + kTwoPi * turns;
  const gp_Dir start_direction = t.direction(start);
  const gp_Pnt base = t.origin.Translated(gp_Vec(t.along) * t.groove_centre(start));
  const auto corner = [&](double side, double radius) {
    return base.Translated(gp_Vec(t.along) * (side * t.groove_half(radius)) + gp_Vec(start_direction) * radius);
  };
  BRepBuilderAPI_MakePolygon section(corner(-1.0, low), corner(1.0, low), corner(1.0, high), corner(-1.0, high),
                                     true);
  detail::NamedWire loop;
  loop.wire = section.Wire();
  return detail::screwed_surfaces(loop, gp_Ax1(t.origin, t.along), t.pitch, static_cast<double>(turns),
                                  !t.right_handed, kScrewExact);
}

// The grooves between two radii, from `from` to `to` along the axis: whole
// turns covering more than a pitch past both ends, cut to the length by a
// cylinder of radius `outer`. `screws` gets the screw surfaces its faces
// approximate (screw_surfaces).
TopoDS_Shape grooves(const Thread& t, double low, double high, double from, double to, double outer,
                     std::vector<occ::handle<Geom_Surface>>& screws) {
  const double sign = t.right_handed ? 1.0 : -1.0;
  // The turns start a pitch before the start of the thread's helix (the
  // low end of a right-hand thread, the high end of a left-hand one).
  const double target = t.right_handed ? from - 1.5 * t.pitch : to + 1.5 * t.pitch;
  const double first = t.angle + sign * kTwoPi * (target - t.groove_centre(t.angle)) / t.pitch;
  const int turns = static_cast<int>(std::ceil((to - from) / t.pitch)) + 3;
  screws = screw_surfaces(t, first, turns, low, high);
  return common_of(groove_sweep(t, first, turns, low, high), tube(t, 0.0, outer, from, to),
                   "the thread's grooves could not be cut to length");
}

// Whether the body has material just past the end of the threaded part
// (`side` -1 below `at`, +1 above it) where the thread's tools would
// reach: the radii between `inner` and `outer` at eight angles.
bool material_past(const TopoDS_Shape& body, const Thread& t, double at, double side, double inner,
                   double outer) {
  BRepClass3d_SolidClassifier classifier(body);
  const double s = at + side * 0.25 * t.pitch;
  for (int k = 0; k < 8; ++k) {
    for (const double f : {0.05, 0.5, 0.95}) {
      classifier.Perform(t.point(s, kTwoPi * k / 8.0, inner + f * (outer - inner)), 1.0e-7);
      if (classifier.State() != TopAbs_OUT) {
        return true;
      }
    }
  }
  return false;
}

// Whether an axis points "up": its Z component positive, or for axes
// across Z its Y component, or for the X axis X. The thread's helix is
// placed in that direction, whichever way the face's surface runs.
bool upwards(const gp_Dir& d) {
  constexpr double kAcross = 1.0e-9;
  if (std::abs(d.Z()) > kAcross) {
    return d.Z() > 0.0;
  }
  if (std::abs(d.Y()) > kAcross) {
    return d.Y() > 0.0;
  }
  return d.X() > 0.0;
}

// The face's thread with its axis pointing up (`upwards`) from the face's
// low end that way.
Thread thread_of(const CylinderFace& cylinder, const ThreadSpec& spec) {
  Thread t;
  t.origin = cylinder.axis.Location();
  t.along = cylinder.axis.Direction();
  if (!upwards(t.along)) {
    t.origin.Translate(gp_Vec(t.along) * cylinder.length);
    t.along.Reverse();
  }
  t.reference = reference_direction(t.along);
  t.face_radius = cylinder.radius;
  t.length = cylinder.length;
  t.internal = cylinder.internal;
  t.pitch = spec.pitch;
  // The radial clearance moves the whole profile away from the material:
  // out for a bore, in for a shaft.
  const double shift = cylinder.internal ? spec.radial_clearance : -spec.radial_clearance;
  t.pitch_radius = spec.pitch_diameter / 2.0 + shift;
  t.major_radius = spec.major / 2.0 + shift;
  t.minor_radius = spec.minor / 2.0 + shift;
  t.right_handed = spec.right_handed;
  t.angle = spec.angle;
  // A flank moved by c perpendicular to itself, at 30 degrees to the
  // radius, moves c / cos 30 along the axis.
  t.relief = spec.flank_clearance / kCos30;
  return t;
}

// The solid between a face's pieces and their copies on the cylinder of
// radius `radius` about the same axis (the pieces thickened along their
// radii).
TopoDS_Shape radial_band(const Shape& body, const std::string& face, double face_radius, double radius) {
  BRep_Builder builder;
  TopoDS_Compound band;
  builder.MakeCompound(band);
  for (const int i : body.find_faces(face)) {
    // The offset writes into its input. Its direction is the surface's
    // normal, whichever way the face runs: the side is checked.
    const detail::InputCopy copy(body.face(i));
    TopoDS_Shape thick;
    for (const double distance : {radius - face_radius, face_radius - radius}) {
      BRepOffset_MakeOffset maker;
      maker.Initialize(copy.shape(), distance, 1.0e-7, BRepOffset_Skin, false, false, GeomAbs_Arc, true);
      detail::interruptible([&](const Message_ProgressRange& range) { maker.MakeOffsetShape(range); });
      if (!maker.IsDone()) {
        continue;
      }
      bool reaches = false;
      for (TopExp_Explorer f(maker.Shape(), TopAbs_FACE); f.More() && !reaches; f.Next()) {
        const BRepAdaptor_Surface surface(TopoDS::Face(f.Current()));
        reaches = surface.GetType() == GeomAbs_Cylinder && std::abs(surface.Cylinder().Radius() - radius) < 1.0e-6;
      }
      if (reaches) {
        thick = maker.Shape();
        break;
      }
    }
    if (thick.IsNull()) {
      throw std::runtime_error("the thread's teeth could not be built on face " + face);
    }
    for (TopExp_Explorer it(thick, TopAbs_SOLID); it.More(); it.Next()) {
      builder.Add(band, it.Current());
    }
  }
  return band;
}

// The end chamfers of one face (mitcad#304), in the thread's axis terms:
// whether its low and high end are to get one, the cone's radius at the
// end and the tangent of its angle to the axis, and the names of the cones.
struct EndChamfers {
  bool low = false;
  bool high = false;
  double radius = 0.0;
  double tan = 1.0;
  std::string low_name;
  std::string high_name;
};

// The solid an end chamfer takes away at `at` along the thread's axis (the
// low end with `side` -1, the high end with +1): for an external thread
// what lies outside the cone that widens from `radius` at the end into the
// material up to `outer`, for an internal one what lies inside the cone
// that narrows from `radius` down to `inner`; from a little beyond the end.
TopoDS_Shape chamfer_tool(const Thread& t, double at, double side, const EndChamfers& c, double inner, double outer) {
  // At `u` into the material from the end, at a radius.
  const auto point = [&](double u, double radius) {
    return t.origin.Translated(gp_Vec(t.along) * (at - side * u) + gp_Vec(t.reference) * radius);
  };
  BRepBuilderAPI_MakePolygon section;
  if (t.internal) {
    const double beyond = 0.1 * t.pitch;
    section.Add(point(-beyond, inner));
    section.Add(point(-beyond, c.radius + beyond * c.tan));
    section.Add(point((c.radius - inner) / c.tan, inner));
  } else {
    const double beyond = std::min(0.1 * t.pitch, 0.5 * c.radius / c.tan);
    section.Add(point(-beyond, c.radius - beyond * c.tan));
    section.Add(point(-beyond, outer));
    section.Add(point((outer - c.radius) / c.tan, outer));
  }
  section.Close();
  BRepBuilderAPI_MakeFace face(section.Wire(), true);
  if (!face.IsDone()) {
    throw std::runtime_error("the thread's end chamfer could not be built");
  }
  BRepPrimAPI_MakeRevol revol(face.Face(), gp_Ax1(t.origin, t.along), kTwoPi);
  if (!revol.IsDone()) {
    throw std::runtime_error("the thread's end chamfer could not be built");
  }
  return revol.Shape();
}

// How far into the material a chamfer's cone reaches the radius `material`
// (the face, or the thread's crest where that lies further into it), and
// checks that the cone cuts it.
double chamfer_depth(const Thread& t, const EndChamfers& c, double material, const std::string& face) {
  const double depth = (t.internal ? c.radius - material : material - c.radius) / c.tan;
  if (depth <= kSame) {
    std::ostringstream text;
    text << "the thread's chamfer diameter " << 2.0 * c.radius << " must be "
         << (t.internal ? "larger" : "smaller") << " than the diameter " << 2.0 * material << " of face " << face;
    throw std::invalid_argument(text.str());
  }
  return depth;
}

// The chamfers at the free ends of the thread's part from `from` to `to`
// cut out of `body` (`free_low`, `free_high`: nothing lies beyond those
// ends); `inner` and `outer` bound the material at the face.
ShapePtr cut_chamfers(ShapePtr body, const Thread& t, double from, double to, const EndChamfers& c, bool free_low,
                      bool free_high, double inner, double outer, double material, const std::string& face) {
  const bool low = c.low && free_low;
  const bool high = c.high && free_high;
  if (!low && !high) {
    return body;
  }
  const double depth = chamfer_depth(t, c, material, face);
  if ((low ? depth : 0.0) + (high ? depth : 0.0) >= to - from - kSame) {
    throw std::invalid_argument("the thread's end chamfers on face " + face + " are deeper than the thread is long");
  }
  if (low) {
    body = detail::local_boolean(*body, chamfer_tool(t, from, -1.0, c, inner, outer), detail::LocalOperation::Cut,
                                 c.low_name)
               .shape;
  }
  if (high) {
    body = detail::local_boolean(*body, chamfer_tool(t, to, 1.0, c, inner, outer), detail::LocalOperation::Cut,
                                 c.high_name)
               .shape;
  }
  return body;
}

// A cosmetic thread's end chamfers on one face: no profile, only the cones
// at its free ends.
ShapePtr chamfer_face(const Shape& body, const Thread& t, double from, double to, const EndChamfers& c,
                      const std::string& face) {
  if (!c.low && !c.high) {
    return std::make_shared<Shape>(body);
  }
  const double margin = 0.1 * t.pitch;
  const double inner = t.internal ? std::max(t.face_radius - margin, 0.5 * t.face_radius) : c.radius;
  const double outer = t.internal ? c.radius : t.face_radius + margin;
  const bool free_low = from <= kSame && !material_past(body.occt(), t, from, -1.0, inner, outer);
  const bool free_high = to >= t.length - kSame && !material_past(body.occt(), t, to, 1.0, inner, outer);
  return cut_chamfers(std::make_shared<Shape>(body), t, from, to, c, free_low, free_high, inner, outer,
                      t.face_radius, face);
}

// One face's thread on the body: the teeth added where the face lies
// inside the crest, the end chamfers cut, then the grooves (and what lies
// beyond the crest) removed. `screws` gets the screw surfaces the grooves'
// faces approximate.
ShapePtr thread_face(const Shape& body, const std::string& face_reference, const Thread& t, double from, double to,
                     const EndChamfers& chamfers, const std::string& name,
                     std::vector<occ::handle<Geom_Surface>>& screws) {
  const double margin = 0.1 * t.pitch;
  // The face inside the crest: the band between them is filled first,
  // over the face itself (its pieces moved out to the crest, along the
  // radii).
  ShapePtr current = std::make_shared<Shape>(body);
  const double beyond = t.internal ? t.face_radius - t.crest() : t.crest() - t.face_radius;
  if (beyond > kSame) {
    TopoDS_Shape band = radial_band(body, face_reference, t.face_radius, t.crest());
    if (from > kSame || to < t.length - kSame) {
      band = common_of(band, tube(t, 0.0, std::max(t.face_radius, t.crest()) + margin, from, to),
                       "the thread's teeth could not be cut to length");
    }
    current = detail::local_boolean(*current, band, detail::LocalOperation::Fuse, name).shape;
  }
  const double face = beyond > kSame ? t.crest() : t.face_radius;

  // Past the ends of the face, where nothing lies beyond them, the tools
  // run out by a pitch.
  const double inner = t.internal ? std::max(0.0, std::min(face, t.crest()) - margin) : t.minor_radius;
  const double outer = t.internal ? t.major_radius : std::max(face, t.crest()) + margin;
  const bool free_low = from <= kSame && !material_past(current->occt(), t, from, -1.0, inner, outer);
  const bool free_high = to >= t.length - kSame && !material_past(current->occt(), t, to, 1.0, inner, outer);
  // The end chamfers, before the grooves: the first turn starts on the cone.
  if (chamfers.low || chamfers.high) {
    const double material = t.internal ? std::min(face, t.crest()) : std::max(face, t.crest());
    const double tool_inner = t.internal ? std::max(material - margin, 0.5 * material) : chamfers.radius;
    const double tool_outer = t.internal ? chamfers.radius : material + margin;
    current = cut_chamfers(current, t, from, to, chamfers, free_low, free_high, tool_inner, tool_outer, material,
                           face_reference);
  }
  if (free_low) {
    from -= t.pitch;
  }
  if (free_high) {
    to += t.pitch;
  }

  // What lies beyond the crest goes first, then the grooves from the root
  // to a little past the crest (no further: the grooves of neighbouring
  // turns must not meet).
  if (std::abs(face - t.crest()) > kSame) {
    const TopoDS_Shape beyond_crest = t.internal ? tube(t, std::max(0.0, face - margin), t.minor_radius, from, to)
                                                 : tube(t, t.major_radius, face + margin, from, to);
    current = detail::local_boolean(*current, beyond_crest, detail::LocalOperation::Cut, name).shape;
  }
  // Where the flanks of a tooth meet.
  const double reach = t.reach();
  TopoDS_Shape cut;
  if (t.internal) {
    const double past = std::min(margin / 2.0, 0.5 * (t.minor_radius - (t.pitch_radius - reach)));
    cut = grooves(t, t.minor_radius - past, t.major_radius, from, to, t.major_radius + margin, screws);
  } else {
    const double past = std::min(margin / 2.0, 0.5 * (t.pitch_radius + reach - t.major_radius));
    cut = grooves(t, t.minor_radius, t.major_radius + past, from, to, t.major_radius + margin, screws);
  }
  return detail::local_boolean(*current, cut, detail::LocalOperation::Cut, name).shape;
}

// Checks that the extra clearance leaves a thread on an internal or an
// external face: the flank clearance a crest at least P/50 wide (it moves
// each flank flank_clearance / cos 30 into the tooth, the crest keeping its
// radius), the radial clearance an external thread a core.
void check_clearance(const ThreadSpec& spec, bool internal) {
  const auto text = [](double value) {
    std::ostringstream out;
    out << value;
    return out.str();
  };
  if (spec.flank_clearance > 0.0) {
    // The crest's width before the relief: half a pitch at the pitch
    // diameter, narrowing along the 30-degree flanks to the crest.
    const double depth = internal ? spec.pitch_diameter - spec.minor : spec.major - spec.pitch_diameter;
    const double crest = spec.pitch / 2.0 - depth * kTan30 - 2.0 * spec.flank_clearance / kCos30;
    if (crest < kMinCrest * spec.pitch) {
      throw std::invalid_argument("the thread's flank clearance of " + text(spec.flank_clearance) +
                                  " mm leaves no usable tooth: its crest would be " + text(crest) +
                                  " mm wide, at least " + text(kMinCrest * spec.pitch) + " mm is needed");
    }
  }
  if (!internal && spec.minor / 2.0 - spec.radial_clearance <= kSame) {
    throw std::invalid_argument("the thread's radial clearance of " + text(spec.radial_clearance) +
                                " mm leaves the external thread no core");
  }
}

// Whether a face holds one of the threads' names, numbered or not.
bool thread_named(const NameList& names, const std::vector<std::string>& threads) {
  for (const std::string& name : names) {
    for (const std::string& thread : threads) {
      if (name.compare(0, thread.size(), thread) == 0 &&
          (name.size() == thread.size() || name[thread.size()] == '#')) {
        return true;
      }
    }
  }
  return false;
}

// The faces named after each thread (its grooves' faces, a turn each, its
// crest's and its teeth's) numbered in the order of their keys, as
// FaceNamer::finish numbers split names, before the pieces are put on
// their screw surfaces (on_screw_surfaces, mitcad#343): their keys are
// those of the pieces swept a turn at a time, so that references saved
// before keep their faces.
ShapePtr per_turn_names(const Shape& body, const std::vector<std::string>& threads) {
  std::vector<NameList> names(static_cast<std::size_t>(body.face_count()));
  for (int i = 0; i < body.face_count(); ++i) {
    names[static_cast<std::size_t>(i)] = body.face_names(i);
  }
  for (const std::string& thread : threads) {
    std::vector<std::pair<detail::ShapeKey, int>> keyed;
    for (int i = 0; i < body.face_count(); ++i) {
      NameList& own = names[static_cast<std::size_t>(i)];
      const auto found = std::find(own.begin(), own.end(), thread);
      if (found != own.end()) {
        own.erase(found);
        keyed.emplace_back(detail::shape_key(body.face(i)), i);
      }
    }
    std::sort(keyed.begin(), keyed.end());
    for (std::size_t k = 0; k < keyed.size(); ++k) {
      names[static_cast<std::size_t>(keyed[k].second)].push_back(
          keyed.size() == 1 ? thread : thread + '#' + std::to_string(k));
    }
  }
  detail::FaceNamer namer(body.occt());
  for (int i = 0; i < body.face_count(); ++i) {
    for (const std::string& name : names[static_cast<std::size_t>(i)]) {
      namer.add(body.face(i), name);
    }
  }
  return namer.shape();
}

// The parameters of the point of surface `s` nearest to `p`, from `seed`
// near them: Newton's steps on the distance (the normal equations of the
// tangent plane's least squares), a few as the seed is close.
gp_Pnt2d nearest_on(const occ::handle<Geom_Surface>& s, const gp_Pnt2d& seed, const gp_Pnt& p) {
  double u = seed.X();
  double v = seed.Y();
  for (int k = 0; k < 8; ++k) {
    gp_Pnt q;
    gp_Vec su, sv;
    s->D1(u, v, q, su, sv);
    const gp_Vec r(q, p);
    const double a = su.Dot(su), b = su.Dot(sv), c = sv.Dot(sv);
    const double det = a * c - b * b;
    if (det <= gp::Resolution()) {
      break;
    }
    const double du = (c * r.Dot(su) - b * r.Dot(sv)) / det;
    const double dv = (a * r.Dot(sv) - b * r.Dot(su)) / det;
    u += du;
    v += dv;
    if (std::abs(du) + std::abs(dv) < 1.0e-14) {
      break;
    }
  }
  return gp_Pnt2d(u, v);
}

// The threads' faces that are pieces of the grooves' faces (each the sweep
// of a section's edge along a turn, an approximation of that edge's screw
// surface) put on the screw surface of their edge (`screws`, each over all
// the turns; each piece on its own copy of it), with the same edges: the
// pieces of one face then lie on one surface, and a rounding along them
// joins them (detail::join_surface_pieces). A piece's parameters map
// affinely onto the screw surface's (the section's edge runs as the edge
// itself does, the sweep along its turn as the screw motion's angle, or for
// a cylinder its angle and height), so its edges' p-curves are mapped so.
// The map is found by projecting three points of each piece onto the screw
// surface (from where the map of the piece before takes them) and checked
// at nine points of the piece. A piece that lies on none of them, or
// whose map does not hold, stays as it is.
ShapePtr on_screw_surfaces(const Shape& body, const std::vector<std::string>& threads,
                           const std::vector<occ::handle<Geom_Surface>>& screws) {
  // (How far the pieces may lie from the screw surfaces: the sweeps'
  // approximation is finer.)
  constexpr double kOnScrew = 1.0e-4;
  struct Screw {
    occ::handle<Geom_Surface> surface;
    occ::handle<ShapeAnalysis_Surface> analysis;
    std::optional<gp_GTrsf2d> last;
  };
  std::vector<Screw> known;
  for (const occ::handle<Geom_Surface>& s : screws) {
    known.push_back({s, nullptr, std::nullopt});
  }
  occ::handle<BRepTools_ReShape> reshape = new BRepTools_ReShape();
  bool any = false;
  for (int i = 0; i < body.face_count(); ++i) {
    if (!thread_named(body.face_names(i), threads)) {
      continue;
    }
    const TopoDS_Face face = TopoDS::Face(body.face(i).Oriented(TopAbs_FORWARD));
    const BRepAdaptor_Surface surface(face, false);
    if (surface.GetType() != GeomAbs_BSplineSurface) {
      continue;
    }
    double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    if (u1 - u0 < Precision::PConfusion() || v1 - v0 < Precision::PConfusion()) {
      continue;
    }
    // Whether a map takes the piece onto a surface.
    const auto holds = [&](const occ::handle<Geom_Surface>& s, const gp_GTrsf2d& map) {
      for (const double fu : {0.0, 0.5, 1.0}) {
        for (const double fv : {0.0, 0.5, 1.0}) {
          gp_XY uv(u0 + fu * (u1 - u0), v0 + fv * (v1 - v0));
          const gp_Pnt p = surface.Value(uv.X(), uv.Y());
          map.Transforms(uv);
          if (s->Value(uv.X(), uv.Y()).Distance(p) > kOnScrew) {
            return false;
          }
        }
      }
      return true;
    };
    // The piece's map onto a screw surface: its first point projected (from
    // where the surface's last map takes it, so that the projection only
    // refines it), the derivatives from steps of an eighth of the piece (a
    // periodic parameter, a cylinder's angle, taken within its period), and
    // checked.
    const Screw* screw = nullptr;
    gp_GTrsf2d map;
    for (Screw& s : known) {
      if (s.analysis.IsNull()) {
        s.analysis = new ShapeAnalysis_Surface(s.surface);
      }
      const gp_Pnt first = surface.Value(u0, v0);
      gp_Pnt2d p00;
      if (s.last) {
        gp_XY seed(u0, v0);
        s.last->Transforms(seed);
        p00 = nearest_on(s.surface, gp_Pnt2d(seed), first);
        if (s.surface->Value(p00.X(), p00.Y()).Distance(first) > kOnScrew) {
          continue;
        }
        // (The pieces of one face map alike but for a drift of micrometres:
        // the last map moved onto this piece's first point.)
        gp_GTrsf2d moved = *s.last;
        gp_XY at(u0, v0);
        moved.Transforms(at);
        moved.SetTranslationPart(moved.TranslationPart() + (p00.XY() - at));
        if (holds(s.surface, moved)) {
          s.last = moved;
          screw = &s;
          map = moved;
          break;
        }
      } else {
        // (The first piece on it: projected over the whole surface, whose
        // turns a local projection may confuse.)
        GeomAPI_ProjectPointOnSurf projection(first, s.surface);
        if (projection.NbPoints() == 0 || projection.LowerDistance() > kOnScrew) {
          continue;
        }
        double pu = 0, pv = 0;
        projection.LowerDistanceParameters(pu, pv);
        p00.SetCoord(pu, pv);
      }
      const double hu = (u1 - u0) / 8.0;
      const double hv = (v1 - v0) / 8.0;
      const auto step = [&](double u, double v) {
        gp_XY d = s.analysis->NextValueOfUV(p00, surface.Value(u, v), Precision::Confusion()).XY() - p00.XY();
        if (s.surface->IsUPeriodic()) {
          d.SetX(d.X() - s.surface->UPeriod() * std::round(d.X() / s.surface->UPeriod()));
        }
        if (s.surface->IsVPeriodic()) {
          d.SetY(d.Y() - s.surface->VPeriod() * std::round(d.Y() / s.surface->VPeriod()));
        }
        return d;
      };
      const gp_XY du = step(u0 + hu, v0) / hu;
      const gp_XY dv = step(u0, v0 + hv) / hv;
      gp_GTrsf2d m;
      m.SetValue(1, 1, du.X());
      m.SetValue(1, 2, dv.X());
      m.SetValue(2, 1, du.Y());
      m.SetValue(2, 2, dv.Y());
      m.SetTranslationPart(p00.XY() - du * u0 - dv * v0);
      if (holds(s.surface, m)) {
        s.last = m;
        screw = &s;
        map = m;
        break;
      }
    }
    if (screw == nullptr) {
      continue;
    }
    // Where the map turns the parameters' orientation round (the screw
    // surface's normal runs the other way), the loops run the other way
    // round and the face is reversed.
    const bool flipped = map.Value(1, 1) * map.Value(2, 2) - map.Value(1, 2) * map.Value(2, 1) < 0.0;
    BRep_Builder builder;
    TopoDS_Face on_screw;
    // (Each piece on its own piece of the surface: the turn it lies in and a
    // little more, as the sweeps a turn at a time were. Booleans would join
    // faces of one surface into faces of many turns, and on a surface of
    // many turns they take far longer. A rounding along the pieces joins
    // their surfaces again: detail::join_surface_pieces.)
    occ::handle<Geom_Surface> own = occ::down_cast<Geom_Surface>(screw->surface->Copy());
    if (const auto spline = occ::down_cast<Geom_BSplineSurface>(own)) {
      double v_low = RealLast(), v_high = -RealLast();
      for (const double fu : {0.0, 1.0}) {
        for (const double fv : {0.0, 1.0}) {
          gp_XY uv(u0 + fu * (u1 - u0), v0 + fv * (v1 - v0));
          map.Transforms(uv);
          v_low = std::min(v_low, uv.Y());
          v_high = std::max(v_high, uv.Y());
        }
      }
      double a0 = 0, a1 = 0, b0 = 0, b1 = 0;
      spline->Bounds(a0, a1, b0, b1);
      const double margin = 0.25 * (v_high - v_low);
      const double from = std::max(b0, v_low - margin);
      const double to = std::min(b1, v_high + margin);
      if (from < to) {
        spline->Segment(a0, a1, from, to);
      }
    }
    builder.MakeFace(on_screw, own, BRep_Tool::Tolerance(face));
    for (TopoDS_Iterator w(face); w.More(); w.Next()) {
      builder.Add(on_screw, flipped ? w.Value().Reversed() : w.Value());
    }
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
      const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
      if (BRep_Tool::Degenerated(edge) || BRep_Tool::IsClosed(edge, face)) {
        return std::make_shared<Shape>(body);
      }
      double f = 0, l = 0;
      const occ::handle<Geom2d_Curve> old = BRep_Tool::CurveOnSurface(edge, face, f, l);
      if (old.IsNull()) {
        return std::make_shared<Shape>(body);
      }
      // The p-curve on the screw surface through the projections of the
      // edge's points at its parameters, each from where the map takes the
      // old p-curve's point there (the map is affine only to a few
      // micrometres: the pieces were approximations).
      double cf = 0, cl = 0;
      const occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, cf, cl);
      if (curve.IsNull()) {
        return std::make_shared<Shape>(body);
      }
      const occ::handle<Geom2d_BSplineCurve> spline =
          Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(old, f, l));
      const int n = std::clamp(8 * spline->NbPoles() + 1, 65, 1601);
      occ::handle<NCollection_HArray1<gp_Pnt2d>> points = new NCollection_HArray1<gp_Pnt2d>(1, n);
      occ::handle<NCollection_HArray1<double>> params = new NCollection_HArray1<double>(1, n);
      for (int k = 0; k < n; ++k) {
        const double t = f + (l - f) * k / (n - 1);
        gp_XY seed = old->Value(t).XY();
        map.Transforms(seed);
        gp_Pnt2d q = nearest_on(screw->surface, gp_Pnt2d(seed), curve->Value(t));
        if (screw->surface->IsUPeriodic()) {
          const double period = screw->surface->UPeriod();
          q.SetX(q.X() + period * std::round((seed.X() - q.X()) / period));
        }
        if (screw->surface->IsVPeriodic()) {
          const double period = screw->surface->VPeriod();
          q.SetY(q.Y() + period * std::round((seed.Y() - q.Y()) / period));
        }
        points->SetValue(k + 1, q);
        params->SetValue(k + 1, t);
      }
      Geom2dAPI_Interpolate interpolation(points, params, false, Precision::PConfusion());
      interpolation.Perform();
      if (!interpolation.IsDone()) {
        return std::make_shared<Shape>(body);
      }
      const occ::handle<Geom2d_BSplineCurve> mapped = interpolation.Curve();
      builder.UpdateEdge(edge, mapped, on_screw, BRep_Tool::Tolerance(edge));
      builder.Range(edge, on_screw, f, l);
      // (The edge's tolerance takes in how far the piece lay from the screw
      // surface.)
      builder.SameParameter(edge, false);
      BRepLib::SameParameter(edge, BRep_Tool::Tolerance(edge));
    }
    reshape->Replace(face, flipped ? on_screw.Reversed() : on_screw);
    any = true;
  }
  if (!any) {
    return std::make_shared<Shape>(body);
  }
  const TopoDS_Shape result = reshape->Apply(body.occt());
  detail::FaceNamer namer(result);
  namer.carry(*reshape->History(), body);
  return namer.shape();
}

} // namespace

ShapePtr modeled_thread(const Shape& body, const ThreadSpec& spec) {
  if (spec.faces.empty()) {
    throw std::invalid_argument("no faces to thread");
  }
  detail::require_positive("thread pitch", spec.pitch);
  if (spec.groove) {
    detail::require_positive("thread major diameter", spec.major);
    detail::require_positive("thread minor diameter", spec.minor);
    detail::require_positive("thread pitch diameter", spec.pitch_diameter);
    detail::require_finite("thread angle", spec.angle);
    if (!(spec.minor < spec.pitch_diameter && spec.pitch_diameter < spec.major)) {
      throw std::invalid_argument("the thread's diameters must grow from the minor to the pitch to the major one");
    }
    // The 60-degree flanks meet H/2 = 0.433 P on either side of the pitch
    // diameter: the crests and roots must lie within that.
    const double reach = 0.4330127 * spec.pitch;
    if (spec.major - spec.pitch_diameter >= 2.0 * reach || spec.pitch_diameter - spec.minor >= 2.0 * reach) {
      throw std::invalid_argument("the thread's diameters are too far apart for its pitch");
    }
  }
  if (spec.chamfer) {
    detail::require_finite("thread chamfer angle", spec.chamfer_angle);
    detail::require_positive("thread chamfer diameter", spec.chamfer_diameter);
    if (!(spec.chamfer_angle > 0.0 && spec.chamfer_angle < kTwoPi / 4.0)) {
      throw std::invalid_argument("the thread's chamfer angle must be between 0 and 90 degrees to the axis");
    }
    if (spec.chamfer_ends.size() != spec.faces.size()) {
      throw std::invalid_argument("the thread's chamfer ends must be given for each face");
    }
  }
  if (!spec.full_length) {
    detail::require_positive("thread length", spec.length);
    detail::require_finite("thread offset", spec.offset);
    if (spec.offset < 0.0) {
      throw std::invalid_argument("the thread offset must not be negative");
    }
  }
  detail::require_finite("thread radial clearance", spec.radial_clearance);
  detail::require_finite("thread flank clearance", spec.flank_clearance);
  if (spec.radial_clearance < 0.0) {
    throw std::invalid_argument("the thread's radial clearance must not be negative");
  }
  if (spec.flank_clearance < 0.0) {
    throw std::invalid_argument("the thread's flank clearance must not be negative");
  }
  return detail::run("thread", [&] {
    ShapePtr current = std::make_shared<Shape>(body);
    std::vector<std::string> threads;
    std::vector<occ::handle<Geom_Surface>> screws;
    for (std::size_t k = 0; k < spec.faces.size(); ++k) {
      const std::string& face = spec.faces[k];
      const CylinderFace cylinder = face_cylinder(*current, face);
      if (spec.groove) {
        check_clearance(spec, cylinder.internal);
        if (!cylinder.internal && cylinder.radius <= spec.minor / 2.0 - spec.radial_clearance + kSame) {
          throw std::invalid_argument("face " + face + " is too thin for the thread");
        }
        if (cylinder.internal && cylinder.radius >= spec.major / 2.0 + spec.radial_clearance - kSame) {
          throw std::invalid_argument("face " + face + " is too wide for the thread");
        }
      }
      const double tolerance = 1.0e-6;
      double from = 0.0;
      double to = cylinder.length;
      if (!spec.full_length) {
        if (spec.high_end) {
          to = cylinder.length - spec.offset;
          from = to - spec.length;
        } else {
          from = spec.offset;
          to = from + spec.length;
        }
        if (from < -tolerance || to > cylinder.length + tolerance) {
          throw std::invalid_argument("the thread does not fit on face " + face);
        }
        from = std::max(from, 0.0);
        to = std::min(to, cylinder.length);
      }
      const Thread thread = thread_of(cylinder, spec);
      EndChamfers chamfers;
      if (spec.chamfer) {
        chamfers.low = spec.chamfer_ends[k].low;
        chamfers.high = spec.chamfer_ends[k].high;
        chamfers.radius = spec.chamfer_diameter / 2.0;
        chamfers.tan = std::tan(spec.chamfer_angle);
        chamfers.low_name = face_name(spec.feature, "thread_chamfer_low", face);
        chamfers.high_name = face_name(spec.feature, "thread_chamfer_high", face);
      }
      if (!thread.along.IsEqual(cylinder.axis.Direction(), 1.0e-9)) {
        // Measured along the thread's axis.
        std::swap(from, to);
        from = cylinder.length - from;
        to = cylinder.length - to;
        std::swap(chamfers.low, chamfers.high);
        std::swap(chamfers.low_name, chamfers.high_name);
      }
      if (spec.groove) {
        threads.push_back(face_name(spec.feature, "thread", face));
        std::vector<occ::handle<Geom_Surface>> own;
        current = thread_face(*current, face, thread, from, to, chamfers, threads.back(), own);
        screws.insert(screws.end(), own.begin(), own.end());
      } else {
        current = chamfer_face(*current, thread, from, to, chamfers, face);
      }
    }
    // The threads' faces and the threaded faces' pieces (a crest the face
    // itself) numbered a turn each as their pieces are, then the grooves'
    // pieces put on their screw surfaces: a rounding along the thread joins
    // the pieces of each into one face over all the turns, as the bodies
    // stored in files keep threads (fillet, detail::join_surface_pieces).
    if (!threads.empty()) {
      std::vector<std::string> named = threads;
      if (spec.groove) {
        named.insert(named.end(), spec.faces.begin(), spec.faces.end());
      }
      current = per_turn_names(*current, named);
      current = on_screw_surfaces(*current, threads, screws);
    }
    // Number the pieces of split names, and check the faces that changed.
    detail::FaceNamer namer(current->occt());
    for (int i = 0; i < current->face_count(); ++i) {
      for (const std::string& name : current->face_names(i)) {
        namer.add(current->face(i), name);
      }
    }
    namer.finish();
    // The faces that are not the body's.
    ShapeMap before;
    TopExp::MapShapes(body.occt(), TopAbs_FACE, before);
    BRep_Builder builder;
    TopoDS_Compound changed;
    builder.MakeCompound(changed);
    for (TopExp_Explorer f(namer.result(), TopAbs_FACE); f.More(); f.Next()) {
      if (!before.Contains(f.Current())) {
        builder.Add(changed, f.Current());
      }
    }
    detail::require_valid_near(namer.result(), changed, "the thread");
    return namer.shape();
  });
}

} // namespace mitcad::geometry
