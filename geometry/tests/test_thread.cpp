// SPDX-License-Identifier: MIT
// Modelled threads: the volume against the profile's exact one (bolts and
// tapped holes, a class's diameters and the basic ones), where the helix
// starts, ends at shoulders and partial lengths, end chamfers, names, and
// the speed on a body of thousands of faces.

#include <chrono>
#include <cmath>
#include <cstdio>
#include <limits>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <GProp_GProps.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>

#include "../src/face_join.hpp"
#include "../src/history.hpp"
#include "check.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

constexpr double kTan30 = 0.57735026918962576451;
constexpr double kH = 0.8660254037844386; // H / P
constexpr double kCos30 = 0.8660254037844386;

struct Diameters {
  double major;
  double minor;
  double pitch;
};

// The basic profile of a size (ISO 68-1).
Diameters basic(double d, double pitch) { return {d, d - 1.25 * kH * pitch, d - 0.75 * kH * pitch}; }

// M10x1.5 of class 6g and 6H: the middles of the tolerances, as the
// reference models store them.
constexpr Diameters k6g{9.85, 8.141, 8.928};
constexpr Diameters k6H{10.198, 8.526, 9.116};

ThreadSpec spec_of(const std::string& feature, const std::string& face, double pitch, const Diameters& d) {
  ThreadSpec spec;
  spec.feature = feature;
  spec.faces = {face};
  spec.pitch = pitch;
  spec.major = d.major;
  spec.minor = d.minor;
  spec.pitch_diameter = d.pitch;
  return spec;
}

// The material of the thread per millimetre of its length between the
// minor and major diameters: the teeth (an external thread's narrowing
// outwards from P/2 at the pitch diameter, an internal thread's widening)
// swept round the axis, integrated exactly. A flank clearance moves each
// flank `flank` perpendicular to itself into the tooth: the tooth is
// 2 flank / cos 30 narrower at every radius.
double teeth_per_mm(double pitch, const Diameters& d, bool internal, double flank = 0.0) {
  const double rp = d.pitch / 2.0;
  const double a = d.minor / 2.0;
  const double b = d.major / 2.0;
  // width(r) = c0 + c1 r
  const double c1 = internal ? 2.0 * kTan30 : -2.0 * kTan30;
  const double c0 = pitch / 2.0 - c1 * rp - 2.0 * flank / kCos30;
  const auto integral = [&](double r) { return c0 * r * r / 2.0 + c1 * r * r * r / 3.0; };
  return 2.0 * kPi / pitch * (integral(b) - integral(a));
}

// An external thread's section per millimetre: the core and the teeth.
double bolt_per_mm(double pitch, const Diameters& d, double flank = 0.0) {
  return kPi * d.minor * d.minor / 4.0 + teeth_per_mm(pitch, d, false, flank);
}

// What an internal thread takes from a solid per millimetre: everything
// inside the major diameter but the teeth.
double nut_hole_per_mm(double pitch, const Diameters& d, double flank = 0.0) {
  return kPi * d.major * d.major / 4.0 - teeth_per_mm(pitch, d, true, flank);
}

// A profile moved by a radial clearance away from the material: every
// diameter 2 radial larger inside, smaller outside.
Diameters loosened(const Diameters& d, double radial, bool internal) {
  const double shift = internal ? 2.0 * radial : -2.0 * radial;
  return {d.major + shift, d.minor + shift, d.pitch + shift};
}

// The centre of mass, by the analysis library's integrals (OCCT's own
// integration loses precision on faces that run many turns).
gp_Pnt centre(const Shape& shape) { return mass_properties(shape).center; }

bool inside(const Shape& shape, const gp_Pnt& point) {
  BRepClass3d_SolidClassifier classifier(shape.occt(), point, 1e-7);
  return classifier.State() == TopAbs_IN;
}

void test_bolt() {
  // The reference model's bolt: M10x1.5 6g over a 10 mm rod 20 long,
  // whose stored body has 1264.9244 mm3; the exact profile 1264.9649
  // (the stored flanks are fitted splines).
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 5)}, 0, 20);
  const ThreadSpec spec = spec_of("F3", "F2:side(c1)", 1.5, k6g);
  const ShapePtr bolt = modeled_thread(*rod, spec);
  const double exact = 20 * bolt_per_mm(1.5, k6g);
  CHECK_NEAR_TOL(volume(*bolt), exact, 2e-5);
  CHECK_NEAR_TOL(volume(*bolt), 1264.9244242962456, 1e-4);
  CHECK(!bolt->find_faces("F3:thread(F2:side(c1))").empty());
  CHECK(bolt->find_faces("F2:end(r{c1})").size() == 1);
  CHECK(bolt->find_faces("F2:start(r{c1})").size() == 1);
  CHECK(bolt->find_faces("F2:side(c1)").empty());
  CHECK(near(bounding_box(*bolt).max.Z(), 20, 1e-6));
  CHECK(near(bounding_box(*bolt).max.X(), 4.925, 1e-4));

  // Where the helix starts: at angle 0, on the X axis from the face's low
  // end, the groove spans the first half pitch at the pitch diameter.
  const double rp = k6g.pitch / 2.0;
  CHECK(!inside(*bolt, gp_Pnt(rp, 0, 0.375)));
  CHECK(inside(*bolt, gp_Pnt(rp, 0, 1.125)));
  // A quarter turn on (right-hand) the groove has risen a quarter pitch.
  CHECK(!inside(*bolt, gp_Pnt(0, rp, 0.75)));
  CHECK(inside(*bolt, gp_Pnt(0, rp, 1.5)));

  // Turned by an angle: the same volume, the centre of mass turned with it.
  ThreadSpec turned = spec;
  turned.angle = kPi / 2.0;
  const ShapePtr other = modeled_thread(*rod, turned);
  CHECK_NEAR_TOL(volume(*other), volume(*bolt), 1e-6);
  const gp_Pnt a = centre(*bolt);
  const gp_Pnt b = centre(*other);
  CHECK(std::hypot(a.X(), a.Y()) > 1e-3);
  CHECK(std::abs(b.X() + a.Y()) < 1e-4 && std::abs(b.Y() - a.X()) < 1e-4);

  // A left-hand thread has the same volume; the reference model's centre
  // lies where the right-hand one's does.
  ThreadSpec left = spec;
  left.right_handed = false;
  CHECK_NEAR_TOL(volume(*modeled_thread(*rod, left)), exact, 2e-5);
  CHECK(std::abs(a.X() - (-0.00805680684860739)) < 2e-3 && std::abs(a.Y() - 0.004548209334390836) < 2e-3);
}

void test_bolt_profiles() {
  // The basic profile: crests on the 10 mm face itself.
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 5)}, 0, 12);
  const Diameters m10 = basic(10, 1.5);
  CHECK_NEAR_TOL(volume(*modeled_thread(*rod, spec_of("F3", "F2:side(c1)", 1.5, m10))), 12 * bolt_per_mm(1.5, m10),
                 2e-5);
  // A rod thinner than the major diameter gets its teeth added up to it.
  const ShapePtr thin = extrude("F2", {circle(1, 0, 0, 4.7)}, 0, 12);
  const ShapePtr added = modeled_thread(*thin, spec_of("F3", "F2:side(c1)", 1.5, k6g));
  CHECK_NEAR_TOL(volume(*added), 12 * bolt_per_mm(1.5, k6g), 2e-5);
  CHECK(near(bounding_box(*added).max.X(), 4.925, 1e-4));
  // M6x1 and a Unified 1/4-20 (6.35 mm, 20 threads per inch) basic.
  const ShapePtr m6 = extrude("F2", {circle(1, 0, 0, 3)}, 0, 6);
  CHECK_NEAR_TOL(volume(*modeled_thread(*m6, spec_of("F3", "F2:side(c1)", 1.0, basic(6, 1.0)))),
                 6 * bolt_per_mm(1.0, basic(6, 1.0)), 2e-5);
  const ShapePtr unc = extrude("F2", {circle(1, 0, 0, 3.175)}, 0, 10);
  const double pitch = 25.4 / 20;
  CHECK_NEAR_TOL(volume(*modeled_thread(*unc, spec_of("F3", "F2:side(c1)", pitch, basic(6.35, pitch)))),
                 10 * bolt_per_mm(pitch, basic(6.35, pitch)), 2e-5);
}

void test_shoulder_and_partial() {
  // A bolt: a 10 mm shank 20 long on a 16 mm head. The thread over the
  // shank stops at the head (no groove into it) and runs out at the tip.
  const ShapePtr head = extrude("F1", {circle(1, 0, 0, 8)}, -6, 0);
  const ShapePtr shank = extrude("F2", {circle(2, 0, 0, 5)}, 0, 20);
  const BooleanResult joined = boolean(BooleanOp::Join, {head.get()}, *shank);
  CHECK(joined.pieces.size() == 1);
  const ShapePtr bolt = joined.pieces.at(0).shape;
  const ShapePtr threaded = modeled_thread(*bolt, spec_of("F3", "F2:side(c2)", 1.5, k6g));
  CHECK_NEAR_TOL(volume(*threaded), kPi * 64 * 6 + 20 * bolt_per_mm(1.5, k6g), 2e-5);
  CHECK(near(bounding_box(*threaded).min.Z(), -6, 1e-6));

  // 8 mm of thread 2 mm below the top: the rest of the face stays.
  ThreadSpec partial = spec_of("F3", "F2:side(c2)", 1.5, k6g);
  partial.full_length = false;
  partial.length = 8;
  partial.offset = 2;
  const ShapePtr part = modeled_thread(*bolt, partial);
  CHECK_NEAR_TOL(volume(*part), kPi * 64 * 6 + 12 * kPi * 25 + 8 * bolt_per_mm(1.5, k6g), 2e-5);
  CHECK(!part->find_faces("F2:side(c2)").empty());
  partial.length = 19;
  CHECK(throws_with([&] { modeled_thread(*bolt, partial); }, "does not fit"));
  partial.faces = {"F2:end(r{c2})"};
  CHECK(throws_with([&] { modeled_thread(*bolt, partial); }, "not cylindrical"));
  // Diameters that do not make a profile.
  ThreadSpec wrong = spec_of("F3", "F2:side(c2)", 1.5, {9.85, 8.928, 8.141});
  CHECK(throws_with([&] { modeled_thread(*bolt, wrong); }, "diameters"));
  wrong = spec_of("F3", "F2:side(c2)", 0.5, k6g);
  CHECK(throws_with([&] { modeled_thread(*bolt, wrong); }, "too far apart"));
}

void test_tapped_holes() {
  // M10x1.5 6H through a 20 mm plate: a tap drill bore of 8.5 mm (inside
  // the minor diameter: the crests are cut to it) and a 10 mm bore (outside
  // it: the teeth are added). Both leave the same nut.
  const ShapePtr plate = extrude("F2", {rectangle(1, -15, -15, 30, 30)}, 0, 20);
  const double expected = 30 * 30 * 20 - 20 * nut_hole_per_mm(1.5, k6H);
  for (const double bore : {8.5, 10.0}) {
    const ShapePtr drill = extrude("F3", {circle(5, 0, 0, bore / 2.0)}, -1, 21);
    const BooleanResult drilled = boolean(BooleanOp::Cut, {plate.get()}, *drill);
    CHECK(drilled.pieces.size() == 1);
    const ShapePtr nut = drilled.pieces.at(0).shape;
    const ShapePtr threaded = modeled_thread(*nut, spec_of("F4", "F3:side(c5)", 1.5, k6H));
    CHECK_NEAR_TOL(volume(*threaded), expected, 2e-5);
    CHECK(!threaded->find_faces("F4:thread(F3:side(c5))").empty());
    // The internal thread's tooth spans the first half pitch at angle 0.
    const double rp = k6H.pitch / 2.0;
    CHECK(inside(*threaded, gp_Pnt(rp, 0, 0.375)));
    CHECK(!inside(*threaded, gp_Pnt(rp, 0, 1.125)));
  }

  // The basic M6x1 in a drilled hole through a 6 mm plate: the wall's axis
  // points down, into the material.
  const ShapePtr block = extrude("F2", {rectangle(1, 100, 0, 30, 30)}, 0, 6);
  HoleSpec hole;
  hole.feature = "F4";
  hole.positions = {gp_Ax1(gp_Pnt(115, 15, 6), gp_Dir(0, 0, -1))};
  hole.diameter = basic(6, 1).minor;
  hole.extent = HoleExtent::ThroughAll;
  hole.bodies = {block};
  const BooleanResult drilled = boolean(BooleanOp::Cut, {block.get()}, *hole_tool(hole));
  const ShapePtr nut = drilled.pieces.at(0).shape;
  const ShapePtr in_hole = modeled_thread(*nut, spec_of("F5", "F4:hole0.wall", 1.0, basic(6, 1)));
  CHECK_NEAR_TOL(volume(*in_hole), 30 * 30 * 6 - 6 * nut_hole_per_mm(1.0, basic(6, 1)), 2e-5);

  // A blind hole: the thread stops at the drill point's start.
  const ShapePtr blind_block = extrude("F2", {rectangle(1, 100, 0, 30, 30)}, 0, 20);
  HoleSpec blind = hole;
  blind.positions = {gp_Ax1(gp_Pnt(115, 15, 20), gp_Dir(0, 0, -1))};
  blind.extent = HoleExtent::Distance;
  blind.depth = 12;
  blind.bodies = {blind_block};
  const BooleanResult blind_drilled = boolean(BooleanOp::Cut, {blind_block.get()}, *hole_tool(blind));
  const ShapePtr blind_nut = blind_drilled.pieces.at(0).shape;
  const double before = volume(*blind_nut);
  const ShapePtr blind_threaded = modeled_thread(*blind_nut, spec_of("F5", "F4:hole0.wall", 1.0, basic(6, 1)));
  const double removed = 12 * (nut_hole_per_mm(1.0, basic(6, 1)) - kPi * hole.diameter * hole.diameter / 4.0);
  CHECK_NEAR_TOL(volume(*blind_threaded), before - removed, 2e-5);
}

// Extra clearance on bolts (mitcad#195): the radial one moves the crests
// and roots in, the flank one moves each flank perpendicular to itself;
// the volumes against the exact profile and points either side of the
// moved crests, roots and flanks.
void test_bolt_clearance() {
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 5)}, 0, 12);
  const double rp = k6g.pitch / 2.0;

  // Radial only: 0.2 per side takes 0.4 off every diameter.
  ThreadSpec radial = spec_of("F3", "F2:side(c1)", 1.5, k6g);
  radial.radial_clearance = 0.2;
  const ShapePtr loose = modeled_thread(*rod, radial);
  CHECK_NEAR_TOL(volume(*loose), 12 * bolt_per_mm(1.5, loosened(k6g, 0.2, false)), 2e-5);
  CHECK(near(bounding_box(*loose).max.X(), 4.725, 1e-4));
  CHECK(near(2.0 * bounding_box(*loose).max.X(), k6g.major - 0.4, 1e-4));
  // The crest (the tooth's middle at angle 0 is 1.125 up) and the root (the
  // groove's middle 0.375 up) each 0.2 further in.
  CHECK(inside(*loose, gp_Pnt(4.725 - 0.01, 0, 1.125)));
  CHECK(!inside(*loose, gp_Pnt(4.725 + 0.01, 0, 1.125)));
  const double root = k6g.minor / 2.0 - 0.2;
  CHECK(inside(*loose, gp_Pnt(root - 0.01, 0, 0.375)));
  CHECK(!inside(*loose, gp_Pnt(root + 0.01, 0, 0.375)));

  // Flank only: the crests keep their radius; each flank moves 0.05 / cos 30
  // along the axis, the groove at the pitch diameter spanning
  // [-relief, 0.75 + relief] instead of [0, 0.75], every pitch.
  const double flank = 0.05;
  const double relief = flank / kCos30;
  ThreadSpec thin = spec_of("F3", "F2:side(c1)", 1.5, k6g);
  thin.flank_clearance = flank;
  const ShapePtr relieved = modeled_thread(*rod, thin);
  CHECK_NEAR_TOL(volume(*relieved), 12 * bolt_per_mm(1.5, k6g, flank), 2e-5);
  CHECK(near(bounding_box(*relieved).max.X(), 4.925, 1e-4));
  for (const double turn : {0.0, 1.5, 3.0}) {
    CHECK(!inside(*relieved, gp_Pnt(rp, 0, turn + 0.75 + 0.9 * relief)));
    CHECK(inside(*relieved, gp_Pnt(rp, 0, turn + 0.75 + 1.1 * relief)));
    CHECK(!inside(*relieved, gp_Pnt(rp, 0, turn + 1.5 - 0.9 * relief)));
    CHECK(inside(*relieved, gp_Pnt(rp, 0, turn + 1.5 - 1.1 * relief)));
  }
  // The flank keeps its angle: 0.2 further out it lies 0.2 tan 30 further
  // along, moved by the same relief.
  const double out = rp + 0.2;
  const double edge = 0.75 + 0.2 * kTan30 + relief;
  CHECK(!inside(*relieved, gp_Pnt(out, 0, edge - 0.1 * relief)));
  CHECK(inside(*relieved, gp_Pnt(out, 0, edge + 0.1 * relief)));

  // Both, on a left-hand thread 8 long 2 below the top: the profile is
  // moved in and then relieved, the rest of the face stays.
  ThreadSpec both = spec_of("F3", "F2:side(c1)", 1.5, k6g);
  both.radial_clearance = 0.2;
  both.flank_clearance = flank;
  both.right_handed = false;
  both.full_length = false;
  both.length = 8;
  both.offset = 2;
  const ShapePtr part = modeled_thread(*rod, both);
  CHECK_NEAR_TOL(volume(*part), 4 * kPi * 25 + 8 * bolt_per_mm(1.5, loosened(k6g, 0.2, false), flank), 2e-5);
  CHECK(!part->find_faces("F2:side(c1)").empty());
  CHECK(inside(*part, gp_Pnt(4.99, 0, 1)));
  CHECK(inside(*part, gp_Pnt(4.99, 0, 11)));
  CHECK(!inside(*part, gp_Pnt(4.8, 0, 5.625)));
  CHECK(inside(*part, gp_Pnt(4.725 - 0.01, 0, 5.625)));
  CHECK(!inside(*part, gp_Pnt(4.725 + 0.01, 0, 5.625)));
  const double moved = rp - 0.2;
  CHECK(!inside(*part, gp_Pnt(moved, 0, 5.25 + 0.9 * relief)));
  CHECK(inside(*part, gp_Pnt(moved, 0, 5.25 + 1.1 * relief)));
  // A quarter turn on, the left-hand groove has fallen a quarter pitch
  // (a right-hand one would have risen): the tooth's middle at 5.25.
  CHECK(inside(*part, gp_Pnt(0, moved, 5.25)));
  CHECK(!inside(*part, gp_Pnt(0, moved, 6.0)));

  // Values that leave no thread, and values that are none.
  ThreadSpec wrong = spec_of("F3", "F2:side(c1)", 1.5, k6g);
  wrong.radial_clearance = -0.1;
  CHECK(throws_with([&] { modeled_thread(*rod, wrong); }, "must not be negative"));
  wrong.radial_clearance = 0.0;
  wrong.flank_clearance = std::numeric_limits<double>::quiet_NaN();
  CHECK(throws_with([&] { modeled_thread(*rod, wrong); }, "finite"));
  // The crest of 6g is 0.218 wide: 0.1 per flank takes 0.231 off it.
  wrong.flank_clearance = 0.1;
  CHECK(throws_with([&] { modeled_thread(*rod, wrong); }, "no usable tooth"));
  wrong.flank_clearance = 0.0;
  wrong.radial_clearance = 4.1;
  CHECK(throws_with([&] { modeled_thread(*rod, wrong); }, "no core"));
}

// Extra clearance on tapped holes: the radial one moves the crests and
// roots out, the flank one widens the groove.
void test_hole_clearance() {
  const ShapePtr plate = extrude("F2", {rectangle(1, -15, -15, 30, 30)}, 0, 20);
  const ShapePtr drill = extrude("F3", {circle(5, 0, 0, 4.25)}, -1, 21);
  const BooleanResult drilled = boolean(BooleanOp::Cut, {plate.get()}, *drill);
  CHECK(drilled.pieces.size() == 1);
  const ShapePtr nut = drilled.pieces.at(0).shape;
  const double flank = 0.03;
  const double relief = flank / kCos30;
  ThreadSpec spec = spec_of("F4", "F3:side(c5)", 1.5, k6H);
  spec.radial_clearance = 0.2;
  spec.flank_clearance = flank;
  const ShapePtr threaded = modeled_thread(*nut, spec);
  CHECK_NEAR_TOL(volume(*threaded), 30 * 30 * 20 - 20 * nut_hole_per_mm(1.5, loosened(k6H, 0.2, true), flank),
                 2e-5);
  CHECK(!threaded->find_faces("F4:thread(F3:side(c5))").empty());
  // The tooth at the moved pitch diameter spans [relief, 0.75 - relief].
  const double rp = k6H.pitch / 2.0 + 0.2;
  CHECK(inside(*threaded, gp_Pnt(rp, 0, 2.25 - 1.1 * relief)));
  CHECK(!inside(*threaded, gp_Pnt(rp, 0, 2.25 - 0.9 * relief)));
  CHECK(inside(*threaded, gp_Pnt(rp, 0, 1.5 + 1.1 * relief)));
  CHECK(!inside(*threaded, gp_Pnt(rp, 0, 1.5 + 0.9 * relief)));
  // The crest (the minor diameter, at the tooth's middle) and the root (the
  // major one, at the groove's middle) each 0.2 further out.
  const double crest = k6H.minor / 2.0 + 0.2;
  CHECK(inside(*threaded, gp_Pnt(crest + 0.01, 0, 1.875)));
  CHECK(!inside(*threaded, gp_Pnt(crest - 0.01, 0, 1.875)));
  const double root = k6H.major / 2.0 + 0.2;
  CHECK(!inside(*threaded, gp_Pnt(root - 0.01, 0, 2.625)));
  CHECK(inside(*threaded, gp_Pnt(root + 0.01, 0, 2.625)));
  // The crest of 6H is 0.409 wide: 0.17 per flank takes 0.393 off it.
  ThreadSpec wrong = spec_of("F4", "F3:side(c5)", 1.5, k6H);
  wrong.flank_clearance = 0.17;
  CHECK(throws_with([&] { modeled_thread(*nut, wrong); }, "no usable tooth"));

  // The basic M6x1 in a drilled hole through a 6 mm plate (the wall's axis
  // points down, into the material), as a tapped hole makes it.
  const ShapePtr block = extrude("F2", {rectangle(1, 100, 0, 30, 30)}, 0, 6);
  HoleSpec hole;
  hole.feature = "F4";
  hole.positions = {gp_Ax1(gp_Pnt(115, 15, 6), gp_Dir(0, 0, -1))};
  hole.diameter = basic(6, 1).minor;
  hole.extent = HoleExtent::ThroughAll;
  hole.bodies = {block};
  const BooleanResult holed = boolean(BooleanOp::Cut, {block.get()}, *hole_tool(hole));
  ThreadSpec tapped = spec_of("F5", "F4:hole0.wall", 1.0, basic(6, 1));
  tapped.radial_clearance = 0.15;
  tapped.flank_clearance = 0.02;
  const ShapePtr in_hole = modeled_thread(*holed.pieces.at(0).shape, tapped);
  CHECK_NEAR_TOL(volume(*in_hole), 30 * 30 * 6 - 6 * nut_hole_per_mm(1.0, loosened(basic(6, 1), 0.15, true), 0.02),
                 2e-5);
}

// End chamfers (mitcad#304). The material a chamfer takes off a thread is
// exact: at a radius r the cone cuts u(r) deep into the material, and over
// a whole turn the thread's material at that radius is the tooth's width
// over the pitch (the helix sweeps every phase), so the cut is the
// integral of 2 pi r u(r) times that fraction, piecewise a cubic in r
// (Simpson's rule is exact for it).
template <class F>
double integral(const F& f, double a, double b) {
  if (b <= a) {
    return 0.0;
  }
  const int n = 64;
  const double h = (b - a) / n;
  double sum = f(a) + f(b);
  for (int i = 1; i < n; ++i) {
    sum += f(a + i * h) * (i % 2 == 1 ? 4.0 : 2.0);
  }
  return sum * h / 3.0;
}

// The fraction of an external (or internal) thread's material at radius r:
// the core inside the minor diameter (outside the major one), the teeth
// between them, nothing beyond the crest.
double material_fraction(double pitch, const Diameters& d, bool internal, double r) {
  const double a = d.minor / 2.0;
  const double b = d.major / 2.0;
  if (r <= a) {
    return internal ? 0.0 : 1.0;
  }
  if (r >= b) {
    return internal ? 1.0 : 0.0;
  }
  const double c1 = internal ? 2.0 * kTan30 : -2.0 * kTan30;
  const double c0 = pitch / 2.0 - c1 * d.pitch / 2.0;
  return (c0 + c1 * r) / pitch;
}

// What one end chamfer of diameter `diameter` at `angle` to the axis takes
// off a thread (`threaded`) or off a plain face of `radius`.
double chamfer_cut(double pitch, const Diameters& d, bool internal, double diameter, double angle, bool threaded,
                   double radius = 0.0) {
  const double rc = diameter / 2.0;
  const double slope = std::tan(angle);
  const auto fraction = [&](double r) {
    if (threaded) {
      return material_fraction(pitch, d, internal, r);
    }
    return internal ? (r >= radius ? 1.0 : 0.0) : (r <= radius ? 1.0 : 0.0);
  };
  const auto cut = [&](double r) {
    const double u = (internal ? rc - r : r - rc) / slope;
    return 2.0 * kPi * r * u * fraction(r);
  };
  const double a = d.minor / 2.0;
  const double b = d.major / 2.0;
  if (!threaded) {
    return internal ? integral(cut, radius, rc) : integral(cut, rc, radius);
  }
  // Split where the fraction changes its formula.
  if (internal) {
    return integral(cut, a, std::min(b, rc)) + integral(cut, b, rc);
  }
  return integral(cut, rc, a) + integral(cut, std::max(a, rc), b);
}

// The smallest face of a shape.
double smallest_face(const Shape& shape) {
  double smallest = std::numeric_limits<double>::max();
  for (int i = 0; i < shape.face_count(); ++i) {
    GProp_GProps props;
    BRepGProp::SurfaceProperties(shape.face(i), props);
    smallest = std::min(smallest, props.Mass());
  }
  return smallest;
}

void test_end_chamfers() {
  const double pitch = 1.25;
  const Diameters m8 = basic(8, pitch);
  const double angle = kPi / 4.0;
  // The model's defaults: a tenth of a pitch inside the minor diameter
  // outside, outside the major diameter inside.
  const double d_ext = m8.minor - pitch / 10.0;
  const double d_int = m8.major + pitch / 10.0;

  // An M8 bolt: a shank 16 long on a 14 mm head, both ends asked for. Only
  // the tip is free: the head's end keeps its full thread.
  const ShapePtr head = extrude("F1", {circle(1, 0, 0, 7)}, -5, 0);
  const ShapePtr shank = extrude("F2", {circle(2, 0, 0, 4)}, 0, 16);
  const BooleanResult joined = boolean(BooleanOp::Join, {head.get()}, *shank);
  CHECK(joined.pieces.size() == 1);
  const ShapePtr bolt = joined.pieces.at(0).shape;
  ThreadSpec spec = spec_of("F3", "F2:side(c2)", pitch, m8);
  spec.chamfer = true;
  spec.chamfer_angle = angle;
  spec.chamfer_diameter = d_ext;
  spec.chamfer_ends = {{true, true}};
  const ShapePtr threaded = modeled_thread(*bolt, spec);
  const double plain = kPi * 49 * 5 + 16 * bolt_per_mm(pitch, m8);
  const double cut = chamfer_cut(pitch, m8, false, d_ext, angle, true);
  CHECK(cut > 1.0);
  CHECK_NEAR_TOL(volume(*threaded), plain - cut, 2e-5);
  const int low = static_cast<int>(threaded->find_faces("F3:thread_chamfer_low(F2:side(c2))").size());
  const int high = static_cast<int>(threaded->find_faces("F3:thread_chamfer_high(F2:side(c2))").size());
  CHECK((low == 0) != (high == 0));
  // The cone runs from the chamfer's diameter at the tip; no groove reaches
  // the tip's plane, and the first turn leaves no sliver.
  CHECK(near(bounding_box(*threaded).max.Z(), 16, 1e-6));
  CHECK(inside(*threaded, gp_Pnt(d_ext / 2.0 - 0.01, 0, 16 - 1e-3)));
  CHECK(!inside(*threaded, gp_Pnt(d_ext / 2.0 + 0.01, 0, 16 - 1e-3)));
  CHECK(smallest_face(*threaded) > 1e-4);
  // The input is left as it was.
  CHECK_NEAR_TOL(volume(*bolt), kPi * 49 * 5 + kPi * 16 * 16, 1e-9);

  // An M8 tapped hole through a plate 12 thick: both mouths countersunk to
  // the default diameter at 45 degrees (a 90-degree countersink).
  const ShapePtr plate = extrude("F2", {rectangle(1, -15, -15, 30, 30)}, 0, 12);
  const ShapePtr drill = extrude("F3", {circle(5, 0, 0, m8.minor / 2.0)}, -1, 13);
  const BooleanResult drilled = boolean(BooleanOp::Cut, {plate.get()}, *drill);
  CHECK(drilled.pieces.size() == 1);
  const ShapePtr nut = drilled.pieces.at(0).shape;
  ThreadSpec tapped = spec_of("F4", "F3:side(c5)", pitch, m8);
  tapped.chamfer = true;
  tapped.chamfer_angle = angle;
  tapped.chamfer_diameter = d_int;
  tapped.chamfer_ends = {{true, true}};
  const ShapePtr countersunk = modeled_thread(*nut, tapped);
  const double sink = chamfer_cut(pitch, m8, true, d_int, angle, true);
  CHECK(sink > 1.0);
  CHECK_NEAR_TOL(volume(*countersunk), 30 * 30 * 12 - 12 * nut_hole_per_mm(pitch, m8) - 2 * sink, 2e-5);
  CHECK(!countersunk->find_faces("F4:thread_chamfer_low(F3:side(c5))").empty());
  CHECK(!countersunk->find_faces("F4:thread_chamfer_high(F3:side(c5))").empty());
  CHECK(!inside(*countersunk, gp_Pnt(d_int / 2.0 - 0.01, 0, 12 - 1e-3)));
  CHECK(inside(*countersunk, gp_Pnt(d_int / 2.0 + 0.01, 0, 12 - 1e-3)));
  CHECK(smallest_face(*countersunk) > 1e-4);
  // Only the end asked for.
  tapped.chamfer_ends = {{true, false}};
  const ShapePtr one = modeled_thread(*nut, tapped);
  CHECK_NEAR_TOL(volume(*one), 30 * 30 * 12 - 12 * nut_hole_per_mm(pitch, m8) - sink, 2e-5);
  CHECK(one->find_faces("F4:thread_chamfer_high(F3:side(c5))").empty());

  // Cosmetic: the same cones on the plain faces, no profile.
  ThreadSpec cosmetic = spec;
  cosmetic.groove = false;
  const ShapePtr chamfered = modeled_thread(*bolt, cosmetic);
  CHECK_NEAR_TOL(volume(*chamfered),
                 kPi * 49 * 5 + kPi * 16 * 16 - chamfer_cut(pitch, m8, false, d_ext, angle, false, 4.0), 1e-6);
  CHECK(chamfered->find_faces("F3:thread(F2:side(c2))").empty());
  CHECK(!chamfered->find_faces("F2:side(c2)").empty());
  ThreadSpec cosmetic_hole = tapped;
  cosmetic_hole.groove = false;
  cosmetic_hole.chamfer_ends = {{true, true}};
  const ShapePtr sunk = modeled_thread(*nut, cosmetic_hole);
  CHECK_NEAR_TOL(volume(*sunk),
                 volume(*nut) - 2 * chamfer_cut(pitch, m8, true, d_int, angle, false, m8.minor / 2.0), 1e-6);

  // A partial thread: an end inside the face gets no chamfer.
  ThreadSpec partial = spec;
  partial.full_length = false;
  partial.length = 8;
  partial.offset = 2;
  const ShapePtr inner = modeled_thread(*bolt, partial);
  CHECK_NEAR_TOL(volume(*inner), kPi * 49 * 5 + 8 * kPi * 16 + 8 * bolt_per_mm(pitch, m8), 2e-5);

  // Sizes that cut nothing, or more than the thread.
  ThreadSpec wrong = spec;
  wrong.chamfer_diameter = 8.5;
  CHECK(throws_with([&] { modeled_thread(*bolt, wrong); }, "must be smaller"));
  wrong = tapped;
  wrong.chamfer_diameter = 6.0;
  CHECK(throws_with([&] { modeled_thread(*nut, wrong); }, "must be larger"));
  wrong = spec;
  wrong.chamfer_angle = 0.02;
  CHECK(throws_with([&] { modeled_thread(*bolt, wrong); }, "deeper than the thread"));
  wrong.chamfer_angle = kPi / 2.0;
  CHECK(throws_with([&] { modeled_thread(*bolt, wrong); }, "between 0 and 90"));
}

// A rod of a radius from 0 to 20 with a 45-degree chamfer `size` wide at
// its top end (`F4:chamfer(E{F2:end(r{c1})|F2:side(c1)})`).
ShapePtr chamfered_rod(double radius, double size) {
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, radius)}, 0, 20);
  ChamferSpec equal;
  equal.distance = size;
  return chamfer("F4", *rod, {"E{F2:end(r{c1})|F2:side(c1)}"}, equal);
}

void test_sized_chamfered_end() {
  // The bodies stored after a modelled thread on a shaft sized to the
  // major diameter keep a chamfer at its free end on the sized face: the
  // cone moves along the radius with the face, its edge with the face
  // stays, and its end circle shrinks by as much (mitcad#335). Sizing the
  // face and moving the cone by the radial step times cos 45 along its
  // normal (as the .f3d import does) gives the thread on a rod made at the
  // major diameter with the same chamfer.
  const double step = (k6g.major - 10.0) / 2.0;
  const std::string cone = "F4:chamfer(E{F2:end(r{c1})|F2:side(c1)})";
  const ShapePtr rod = chamfered_rod(5.0, 1.5);
  CHECK(faces_named(*rod, cone) == 1);
  const ShapePtr sized = offset_faces("F5", *rod, {"F2:side(c1)"}, step);
  const ShapePtr moved = offset_faces("F6", *sized, {cone}, step * std::sqrt(0.5));
  const ThreadSpec spec = spec_of("F7", "F2:side(c1)", 1.5, k6g);
  const ShapePtr threaded = modeled_thread(*moved, spec);
  const ShapePtr made = modeled_thread(*chamfered_rod(k6g.major / 2.0, 1.5), spec);
  CHECK_NEAR_TOL(volume(*threaded), volume(*made), 1e-7);
  // The cone from the crest at 18.5 to a radius of 4.925 - 1.5 at the end.
  const double end = k6g.major / 2.0 - 1.5;
  CHECK(inside(*threaded, gp_Pnt(end - 0.01, 0, 20 - 1e-3)));
  CHECK(!inside(*threaded, gp_Pnt(end + 0.01, 0, 20 - 1e-3)));
  CHECK(near(bounding_box(*threaded).max.Z(), 20, 1e-6));
  // With the face sized alone the cone stays where it was: 1.92 mm3 more,
  // 0.15 % of the bolt.
  const ShapePtr alone = modeled_thread(*sized, spec);
  CHECK(volume(*alone) - volume(*made) > 1.5);
  // The two moves alone give the rod made at the major diameter.
  CHECK_NEAR_TOL(volume(*moved), volume(*chamfered_rod(k6g.major / 2.0, 1.5)), 1e-9);
}

// A plate with a grid of n x n small holes (some thousands of faces for n
// of 56) with a 10 mm shaft 20 long standing on a pad on it.
ShapePtr perforated(int n) {
  TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(-100, -100, -10), 200, 200, 10).Shape();
  if (n > 0) {
    BRep_Builder builder;
    TopoDS_Compound holes;
    builder.MakeCompound(holes);
    for (int i = 0; i < n; ++i) {
      for (int j = 0; j < n; ++j) {
        const double x = -95 + 190.0 * (i + 0.5) / n;
        const double y = -95 + 190.0 * (j + 0.5) / n;
        if (std::abs(x) < 15 && std::abs(y) < 15) {
          continue;
        }
        builder.Add(holes, BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(x, y, -11), gp_Dir(0, 0, 1)), 1.0, 12).Shape());
      }
    }
    plate = BRepAlgoAPI_Cut(plate, holes).Shape();
  }
  const Shape base(plate);
  const ShapePtr pad = extrude("P", {circle(2, 0, 0, 10)}, 0, 4);
  const BooleanResult padded = boolean(BooleanOp::Join, {&base}, *pad);
  CHECK(padded.pieces.size() == 1);
  const ShapePtr shaft = extrude("S", {circle(1, 0, 0, 5)}, 4, 24);
  const BooleanResult joined = boolean(BooleanOp::Join, {padded.pieces.at(0).shape.get()}, *shaft);
  CHECK(joined.pieces.size() == 1);
  return joined.pieces.at(0).shape;
}

void test_large_body() {
  // The thread changes the large body exactly as the plain one, in about
  // the same time: only the faces near the thread take part.
  using Clock = std::chrono::steady_clock;
  const ShapePtr small = perforated(0);
  const ShapePtr large = perforated(56);
  CHECK(large->face_count() > 3000);
  const ThreadSpec spec = spec_of("T", "S:side(c1)", 1.5, k6g);
  const auto t0 = Clock::now();
  const ShapePtr a = modeled_thread(*small, spec);
  const auto t1 = Clock::now();
  const ShapePtr b = modeled_thread(*large, spec);
  const auto t2 = Clock::now();
  CHECK_NEAR_TOL(volume(*small) - volume(*a), volume(*large) - volume(*b), 1e-5);
  CHECK(b->face_count() - large->face_count() == a->face_count() - small->face_count());
  CHECK(!b->find_faces("T:thread(S:side(c1))").empty());
  const double small_ms = std::chrono::duration<double, std::milli>(t1 - t0).count();
  const double large_ms = std::chrono::duration<double, std::milli>(t2 - t1).count();
  std::printf("thread: %d faces in %.0f ms, %d faces in %.0f ms\n", small->face_count(), small_ms,
              large->face_count(), large_ms);
  CHECK(large_ms < 4.0 * small_ms + 2000.0);
}

// A face's name the grooves gave while they were swept one turn at a time,
// and a point on that face then.
struct LegacyPiece {
  const char* name;
  double x;
  double y;
  double z;
};

// The largest tolerance of the shape's vertices and edges.
double max_tolerance(const Shape& shape) {
  double tolerance = 0.0;
  for (TopExp_Explorer v(shape.occt(), TopAbs_VERTEX); v.More(); v.Next()) {
    tolerance = std::max(tolerance, BRep_Tool::Tolerance(TopoDS::Vertex(v.Current())));
  }
  for (TopExp_Explorer e(shape.occt(), TopAbs_EDGE); e.More(); e.Next()) {
    tolerance = std::max(tolerance, BRep_Tool::Tolerance(TopoDS::Edge(e.Current())));
  }
  return tolerance;
}

void test_pieces_on_screw_surfaces_and_legacy_names() {
  // The thread's pieces, a crest, two flanks and a root a turn, are put on
  // the exact screw surfaces of the groove's section (mitcad#343), each
  // flank's pieces on one surface, where they were sweeps of their own. The
  // names those 57 pieces had (taken from a build of that time, with a point
  // on each) still name the piece that holds the point, and only it, so
  // that references saved then resolve.
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 5)}, 0, 20);
  ThreadSpec spec = spec_of("F3", "F2:side(c1)", 1.5, k6g);
  // The points were taken on Linux. The pieces of whole turns have their
  // centres on the axis, so their numbers (FaceNamer's order of centres,
  // rounded to 1e-6 mm) follow the nanometres by which the booleans'
  // intersection curves make some turns differ from the others, and which
  // turns those are differs between the platforms' maths libraries
  // (0.16.1-rc1 on Windows: six names on other whole turns). Elsewhere such
  // a name may name another whole turn's piece, with a note; the other
  // pieces' names, and every name on Linux, hold exactly.
#if !defined(__linux__)
  const auto on_axis = [](const TopoDS_Shape& face) {
    GProp_GProps properties;
    BRepGProp::SurfaceProperties(face, properties);
    const gp_Pnt centre = properties.CentreOfMass();
    return std::hypot(centre.X(), centre.Y()) < 1e-4;
  };
#endif
  const auto distance_to = [](const LegacyPiece& piece, const TopoDS_Shape& face) {
    const TopoDS_Vertex at = BRepBuilderAPI_MakeVertex(gp_Pnt(piece.x, piece.y, piece.z));
    BRepExtrema_DistShapeShape between(at, face);
    return between.IsDone() ? between.Value() : 1.0;
  };
  const auto check_pieces = [&](const Shape& threaded, const std::vector<LegacyPiece>& pieces) {
    const std::vector<int> all = threaded.find_faces("F3:thread(F2:side(c1))");
    CHECK(all.size() == 57);
    for (const LegacyPiece& piece : pieces) {
      const std::vector<int> faces = threaded.find_faces(piece.name);
      CHECK(faces.size() == 1);
      if (faces.size() != 1) {
        std::printf("  %s names %zu faces\n", piece.name, faces.size());
        continue;
      }
      const double distance = distance_to(piece, threaded.face(faces.front()));
      if (distance < 2e-4) {
        continue;
      }
#if !defined(__linux__)
      bool whole_turns = on_axis(threaded.face(faces.front()));
      bool held = false;
      for (const int i : all) {
        if (distance_to(piece, threaded.face(i)) < 2e-4) {
          held = true;
          whole_turns = whole_turns && on_axis(threaded.face(i));
        }
      }
      if (held && whole_turns) {
        std::printf("  note: %s names another whole turn's piece here (its point is %g from it)\n", piece.name,
                    distance);
        continue;
      }
#endif
      CHECK(distance < 2e-4);
      std::printf("  %s is %g from its point\n", piece.name, distance);
    }
  };
  const ShapePtr bolt = modeled_thread(*rod, spec);
  check_pieces(*bolt, {
                          {"F3:thread(F2:side(c1))#56", 4.006096174, 2.182098960, 0.062583800},
                          {"F3:thread(F2:side(c1))#55", 4.068958467, -0.112013955, 0.346257715},
                          {"F3:thread(F2:side(c1))#54", 2.735840279, -3.488917118, 0.516242316},
                          {"F3:thread(F2:side(c1))#2", -4.854156205, -0.832341600, 0.370152455},
                          {"F3:thread(F2:side(c1))#18", 2.071030938, -4.064627041, 1.181013414},
                          {"F3:thread(F2:side(c1))#19", 1.847968323, -3.626842013, 1.590328100},
                          {"F3:thread(F2:side(c1))#20", 2.012840691, -3.950422251, 1.969984694},
                          {"F3:thread(F2:side(c1))#6", -4.388207132, 2.235903211, 1.789115846},
                          {"F3:thread(F2:side(c1))#33", 2.071030938, -4.064627041, 8.681013414},
                          {"F3:thread(F2:side(c1))#53", 2.012840691, -3.950422251, 18.469984694},
                          {"F3:thread(F2:side(c1))#17", -4.388207132, 2.235903211, 18.289115846},
                          {"F3:thread(F2:side(c1))#5", 2.071030938, -4.064627041, 19.181013414},
                          {"F3:thread(F2:side(c1))#4", 0.908611370, -3.967794710, 19.531570384},
                          {"F3:thread(F2:side(c1))#1", -2.074516276, -3.918385626, 19.741242409},
                          {"F3:thread(F2:side(c1))#3", -0.884984551, 4.844835121, 19.581867354},
                          {"F3:thread(F2:side(c1))#0", -3.750502502, 2.726913306, 19.999999926},
                      });
  // The faces put on the exact screw surfaces of the section's edges, the
  // per-turn pieces were approximations of: the shape valid, its tolerances
  // no larger, and its volume and area those of the per-turn thread (taken
  // from a build of that time) within 1e-6, as with the clearances and the
  // end chamfers.
  const auto as_before = [](const Shape& threaded, double volume, double area) {
    CHECK(BRepCheck_Analyzer(threaded.occt()).IsValid());
    CHECK(max_tolerance(threaded) <= 5.0e-6 + 1.0e-12);
    const MassProperties properties = mass_properties(threaded);
    CHECK_NEAR_TOL(properties.volume, volume, 1e-6);
    CHECK_NEAR_TOL(properties.area, area, 1e-6);
  };
  as_before(*bolt, 1264.964921282, 1061.486141608);
  ThreadSpec loose = spec;
  loose.radial_clearance = 0.2;
  loose.flank_clearance = 0.05;
  as_before(*modeled_thread(*rod, loose), 1119.469785640, 997.042469442);
  ThreadSpec chamfered = spec;
  chamfered.chamfer = true;
  chamfered.chamfer_angle = kPi / 4.0;
  chamfered.chamfer_diameter = 7.8;
  chamfered.chamfer_ends = {{true, true}};
  as_before(*modeled_thread(*rod, chamfered), 1252.800363702, 1017.873373268);
  spec.right_handed = false;
  check_pieces(*modeled_thread(*rod, spec),
               {
                   {"F3:thread(F2:side(c1))#0", -4.361517055, 1.243916527, 0.107568637},
                   {"F3:thread(F2:side(c1))#53", 4.427586400, -0.232039955, 0.744984711},
                   {"F3:thread(F2:side(c1))#54", 3.890516701, -1.197016826, 0.424085815},
                   {"F3:thread(F2:side(c1))#3", 2.424278895, -3.864354288, 0.184755699},
                   {"F3:thread(F2:side(c1))#1", -2.884529314, 3.991881215, 0.555228680},
                   {"F3:thread(F2:side(c1))#20", -4.388207132, 2.235903211, 2.003826227},
                   {"F3:thread(F2:side(c1))#7", 4.427586400, -0.232039955, 2.244984711},
                   {"F3:thread(F2:side(c1))#19", 4.555585836, -0.238748121, 1.456013361},
                   {"F3:thread(F2:side(c1))#52", -4.388207132, 2.235903211, 5.003826227},
                   {"F3:thread(F2:side(c1))#5", -4.388207132, 2.235903211, 11.003826227},
                   {"F3:thread(F2:side(c1))#17", 4.427586496, -0.232039960, 11.244984767},
                   {"F3:thread(F2:side(c1))#55", 4.516306830, 0.642912705, 19.409755738},
                   {"F3:thread(F2:side(c1))#4", -3.528993778, -3.435378860, 19.581867357},
                   {"F3:thread(F2:side(c1))#2", 0.110111675, 4.432295064, 19.863414325},
                   {"F3:thread(F2:side(c1))#56", 2.131486164, 3.467814422, 19.609398484},
               });
}

// The bolt's crest edge between z 3 and 5 (a turn's piece of it).
std::string crest_edge_of(const Shape& bolt) {
  for (int i = 0; i < bolt.edge_count(); ++i) {
    BRepAdaptor_Curve c(bolt.edge(i));
    const gp_Pnt m = c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
    if (std::abs(std::hypot(m.X(), m.Y()) - 4.925) < 1e-4 && c.GetType() == GeomAbs_BSplineCurve && m.Z() > 3 &&
        m.Z() < 5) {
      return bolt.edge_name(i);
    }
  }
  return {};
}

void test_thread_pieces_join_smoothly() {
  // The crest's and the flanks' pieces of a thread joined for a rounding
  // along them: their edges along the thread joined into edges that keep
  // their speed across the joints, as one curve swept at one pace does (a
  // rounding's guide along a curve whose speed jumps at its joints ends off
  // the face at its end, mitcad#343).
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 5)}, 0, 8);
  const ShapePtr bolt = modeled_thread(*rod, spec_of("F3", "F2:side(c1)", 1.5, k6g));
  std::vector<int> pieces;
  for (int i = 0; i < bolt->face_count(); ++i) {
    for (const std::string& name : bolt->face_names(i)) {
      if (name.rfind("F3:thread(", 0) == 0) {
        pieces.push_back(i);
        break;
      }
    }
  }
  const ShapePtr joined = detail::join_surface_pieces(*bolt, pieces);
  CHECK(joined != nullptr);
  CHECK(joined->face_count() < bolt->face_count());
  int long_edges = 0;
  for (int e = 0; e < joined->edge_count(); ++e) {
    BRepAdaptor_Curve c(joined->edge(e));
    if (c.GetType() != GeomAbs_BSplineCurve || GCPnts_AbscissaPoint::Length(c) < 40.0) {
      continue;
    }
    ++long_edges;
    double low = std::numeric_limits<double>::max();
    double high = 0.0;
    for (int k = 0; k <= 400; ++k) {
      gp_Pnt p;
      gp_Vec d;
      c.D1(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / 400.0, p, d);
      low = std::min(low, d.Magnitude());
      high = std::max(high, d.Magnitude());
    }
    CHECK(high < 1.05 * low);
  }
  CHECK(long_edges > 0);
}

void test_rounding_along_a_thread_keeps_its_pieces() {
  // A rounding along a thread's crest edge: the crest's and the flanks'
  // pieces, a turn each on one surface, are joined for the rounding's walk,
  // and its result split again where they met, so that the body keeps its
  // thread a turn a face and each piece its name, and the rounding is a face
  // a piece of the edge, named after that piece (mitcad#343).
  const ShapePtr rod = extrude("F2", {circle(1, 0, 0, 5)}, 0, 8);
  const ShapePtr bolt = modeled_thread(*rod, spec_of("F3", "F2:side(c1)", 1.5, k6g));
  const std::string crest_edge = crest_edge_of(*bolt);
  CHECK(!crest_edge.empty());
  FilletSet set;
  set.edges = {crest_edge};
  set.radius = 0.05;
  set.tangent_chain = true;
  const ShapePtr rounded = fillet("F4", *bolt, std::vector<FilletSet>{set});
  CHECK(BRepCheck_Analyzer(rounded->occt()).IsValid());
  for (int i = 0; i < bolt->face_count(); ++i) {
    for (const std::string& name : bolt->face_names(i)) {
      CHECK(rounded->find_faces(name).size() == 1);
    }
  }
  // (No face over more than one piece of the thread.)
  for (int i = 0; i < rounded->face_count(); ++i) {
    int pieces = 0;
    for (const std::string& name : rounded->face_names(i)) {
      pieces += name.rfind("F3:thread(", 0) == 0 ? 1 : 0;
    }
    CHECK(pieces <= 1);
  }
  int roundings = 0;
  for (int i = 0; i < rounded->face_count(); ++i) {
    for (const std::string& name : rounded->face_names(i)) {
      roundings += name.rfind("F4:fillet(", 0) == 0 ? 1 : 0;
    }
  }
  CHECK(roundings > 2);
  const double removed = volume(*bolt) - volume(*rounded);
  CHECK(removed > 0.0 && removed < 0.1);
}

} // namespace

void thread_tests() {
  guarded("test_bolt", test_bolt);
  guarded("test_bolt_profiles", test_bolt_profiles);
  guarded("test_shoulder_and_partial", test_shoulder_and_partial);
  guarded("test_tapped_holes", test_tapped_holes);
  guarded("test_bolt_clearance", test_bolt_clearance);
  guarded("test_hole_clearance", test_hole_clearance);
  guarded("test_end_chamfers", test_end_chamfers);
  guarded("test_sized_chamfered_end", test_sized_chamfered_end);
  guarded("test_large_body", test_large_body);
  guarded("test_pieces_on_screw_surfaces_and_legacy_names", test_pieces_on_screw_surfaces_and_legacy_names);
  guarded("test_thread_pieces_join_smoothly", test_thread_pieces_join_smoothly);
  guarded("test_rounding_along_a_thread_keeps_its_pieces", test_rounding_along_a_thread_keeps_its_pieces);
}

} // namespace test
