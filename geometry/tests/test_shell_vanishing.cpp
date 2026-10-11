// SPDX-License-Identifier: MIT
// Shells whose wall makes a narrow face vanish: a chamfer strip narrower,
// or a convex rounding tighter, than the wall. OCCT's offset drops such a
// face and closes the offsets of the faces beside it against each other
// with the port's patch 0100 (third_party/vcpkg-ports, mitcad#172); before,
// it failed or gave a wrong solid. Exact volumes: with the face gone, the
// inside of the wall is that of the body without it.

#include <cmath>
#include <string>

#include <BRepAlgoAPI_Cut.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_List.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>

#include "check.hpp"
#include "mitcad/geometry/dressup.hpp"
#include "mitcad/geometry/faceops.hpp"
#include "mitcad/geometry/query.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

constexpr double kW = 60.0, kD = 40.0, kH = 20.0;

// Sketch1's 60 x 40 rectangle (c1..c4) extruded 20 mm by F2: side 0 is
// y = 0, side 1 x = 60, side 2 y = 40, side 3 x = 0.
ShapePtr block() { return extrude("F2", {rectangle(1, 0, 0, kW, kD)}, 0, kH); }

std::string bottom_front() { return edge(start_cap("F2", 1), side("F2", 1, 0)); }

// The block with its bottom front edge chamfered c (F3).
ShapePtr chamfered(double c) {
  ChamferSpec spec;
  spec.distance = c;
  return chamfer("F3", *block(), {bottom_front()}, spec);
}

// The cavity of the chamfered block shelled inside by t from the open top:
// the walls' offsets, cut by the chamfer's where 2 t < c + t sqrt(2) (its
// offset plane, y + z = c + t sqrt(2), cuts their corner at y = z = t).
double chamfer_cavity(double c, double t) {
  double cavity = (kW - 2 * t) * (kD - 2 * t) * (kH - t);
  const double leg = c + t * std::sqrt(2.0) - 2 * t;
  if (leg > 0) {
    cavity -= leg * leg / 2 * (kW - 2 * t);
  }
  return cavity;
}

// Removed from a 90-degree corner of radius R by a rounding of radius rho
// about it (a ring, by Pappus).
double ring_loss(double R, double rho) {
  const double area = fillet_loss(rho);
  const double centroid =
      (rho * rho * (R - rho / 2) - kPi * rho * rho / 4 * (R - rho + 4 * rho / (3 * kPi))) / area;
  return 2 * kPi * centroid * area;
}

void test_chamfer_strip() {
  const double c = 0.2;
  const ShapePtr body = chamfered(c);
  const double outer = kW * kD * kH - c * c / 2 * kW;
  CHECK_NEAR(volume(*body), outer);
  const std::string strip = "F3:chamfer(" + bottom_front() + ")";
  for (bool rounded : {false, true}) {
    ShellSpec open;
    open.faces = {end_cap("F2", 1)};
    open.rounded = rounded;
    // A wall thinner than the strip's vanishing thickness (c / (2 - sqrt 2)):
    // the strip has its offset.
    open.inside = 0.1;
    const ShapePtr thin = shell("F5", *body, open);
    CHECK_NEAR(volume(*thin), outer - chamfer_cavity(c, 0.1));
    CHECK(faces_named(*thin, "F5:offset(" + strip + ")") == 1);
    // At 0.8 and 1 mm the strip vanishes: the inside of the wall is that of
    // the plain block, the front's and the bottom's offsets meeting.
    for (double t : {0.8, 1.0}) {
      open.inside = t;
      const ShapePtr wall = shell("F5", *body, open);
      CHECK_NEAR(volume(*wall), outer - chamfer_cavity(c, t));
      CHECK(faces_named(*wall, strip) == 1);
      CHECK(faces_named(*wall, "F5:offset(" + strip + ")") == 0);
      CHECK(faces_named(*wall, "F5:offset(" + side("F2", 1, 0) + ")") == 1);
      CHECK(faces_named(*wall, "F5:offset(" + start_cap("F2", 1) + ")") == 1);
      CHECK(wall->face_count() == 12);
      CHECK(edge_names_unique(*wall));
    }
  }
  // Closed (no face removed): the void is the plain block's offset.
  ShellSpec closed;
  closed.inside = 1;
  CHECK_NEAR(volume(*shell("F5", *body, closed)), outer - (kW - 2) * (kD - 2) * (kH - 2));
}

void test_rounding() {
  // A vertical edge rounded 0.45 mm, the wall 2 mm: the rounding vanishes,
  // the inside corner is sharp.
  const double r = 0.45;
  const ShapePtr body = fillet("F3", *block(), {edge(side("F2", 1, 0), side("F2", 1, 1))}, r);
  const double outer = kW * kD * kH - fillet_loss(r) * kH;
  CHECK_NEAR(volume(*body), outer);
  for (bool rounded : {false, true}) {
    ShellSpec open;
    open.faces = {end_cap("F2", 1)};
    open.rounded = rounded;
    open.inside = 0.4;
    const double thin = (kW - 0.8) * (kD - 0.8) * (kH - 0.4) - fillet_loss(r - 0.4) * (kH - 0.4);
    CHECK_NEAR(volume(*shell("F5", *body, open)), outer - thin);
    open.inside = 2;
    const ShapePtr wall = shell("F5", *body, open);
    CHECK_NEAR(volume(*wall), outer - (kW - 4) * (kD - 4) * (kH - 2));
    CHECK(wall->face_count() == 12);
    CHECK(edge_names_unique(*wall));
  }
}

void test_boss() {
  // A round boss whose top edge is rounded 0.45 mm, shelled from the bottom
  // with a 2 mm wall: the ring of the rounding vanishes.
  const double R = 5, h = 10, rho = 0.45, t = 2;
  const ShapePtr boss = extrude("F2", {circle(5, 0, 0, R)}, 0, h);
  const ShapePtr body = fillet("F3", *boss, {edge("F2:side(c5)", "F2:end(r{c5})")}, rho);
  const double outer = kPi * R * R * h - ring_loss(R, rho);
  CHECK_NEAR(volume(*body), outer);
  ShellSpec cup;
  cup.faces = {"F2:start(r{c5})"};
  cup.inside = t;
  const ShapePtr wall = shell("F5", *body, cup);
  CHECK_NEAR(volume(*wall), outer - kPi * (R - t) * (R - t) * (h - t));
  CHECK(edge_names_unique(*wall));
  cup.inside = 0.3;
  const double thin =
      kPi * (R - 0.3) * (R - 0.3) * (h - 0.3) - ring_loss(R - 0.3, rho - 0.3);
  CHECK_NEAR(volume(*shell("F5", *body, cup)), outer - thin);
}

double occt_volume(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props);
  return props.Mass();
}

// The edge of the shape whose middle is nearest to the point.
TopoDS_Edge edge_near(const TopoDS_Shape& shape, const gp_Pnt& point) {
  TopoDS_Edge best;
  double nearest = 1e100;
  for (TopExp_Explorer it(shape, TopAbs_EDGE); it.More(); it.Next()) {
    const TopoDS_Edge e = TopoDS::Edge(it.Current());
    TopoDS_Vertex a, b;
    TopExp::Vertices(e, a, b);
    const gp_Pnt middle((BRep_Tool::Pnt(a).XYZ() + BRep_Tool::Pnt(b).XYZ()) / 2);
    if (middle.Distance(point) < nearest) {
      nearest = middle.Distance(point);
      best = e;
    }
  }
  return best;
}

// The planar face of the shape whose centre is nearest to the point.
TopoDS_Face face_near(const TopoDS_Shape& shape, const gp_Pnt& point) {
  TopoDS_Face best;
  double nearest = 1e100;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    GProp_GProps props;
    BRepGProp::SurfaceProperties(it.Current(), props);
    if (props.CentreOfMass().Distance(point) < nearest) {
      nearest = props.CentreOfMass().Distance(point);
      best = TopoDS::Face(it.Current());
    }
  }
  return best;
}

// OCCT alone, as a reproduction for the patch.
void test_kernel_alone() {
  // An L block whose concave edge is chamfered 0.2 mm, thickened outside by
  // 1 mm with the bottom open: the concave strip vanishes in the outside
  // offset, whose walls meet in a sharp corner. Before the patch: an error.
  const double c = 0.2, t = 1;
  BRepPrimAPI_MakeBox box(kW, kD, kH);
  BRepPrimAPI_MakeBox notch(gp_Pnt(30, -1, 10), gp_Pnt(kW + 1, kD + 1, kH + 1));
  BRepAlgoAPI_Cut cut(box.Shape(), notch.Shape());
  BRepFilletAPI_MakeChamfer bevel(cut.Shape());
  bevel.Add(c, edge_near(cut.Shape(), gp_Pnt(30, kD / 2, 10)));
  const TopoDS_Shape l_block = bevel.Shape();
  const double inside = kW * kD * kH - 30 * kD * 10 + c * c / 2 * kD;
  CHECK_NEAR(occt_volume(l_block), inside);
  NCollection_List<TopoDS_Shape> open;
  open.Append(face_near(l_block, gp_Pnt(kW / 2, kD / 2, 0)));
  BRepOffsetAPI_MakeThickSolid grown;
  grown.MakeThickSolidByJoin(l_block, open, t, 1e-7, BRepOffset_Skin, false, false,
                             GeomAbs_Intersection);
  CHECK(grown.IsDone());
  if (grown.IsDone()) {
    const double outside = (kW + 2 * t) * (kD + 2 * t) * (kH + t) - 30 * (kD + 2 * t) * 10;
    CHECK_NEAR(occt_volume(grown.Shape()), outside - inside);
    CHECK(BRepCheck_Analyzer(grown.Shape()).IsValid());
  }

  // The chamfered block offset inside as a closed solid, both joins.
  BRepPrimAPI_MakeBox plain(kW, kD, kH);
  BRepFilletAPI_MakeChamfer strip(plain.Shape());
  strip.Add(c, edge_near(plain.Shape(), gp_Pnt(kW / 2, 0, 0)));
  for (GeomAbs_JoinType join : {GeomAbs_Intersection, GeomAbs_Arc}) {
    BRepOffsetAPI_MakeOffsetShape inner;
    inner.PerformByJoin(strip.Shape(), -t, 1e-7, BRepOffset_Skin, false, false, join);
    CHECK(inner.IsDone());
    if (inner.IsDone()) {
      CHECK_NEAR(occt_volume(inner.Shape()), (kW - 2 * t) * (kD - 2 * t) * (kH - 2 * t));
    }
  }

  // A thick solid whose offset fails no longer comes back as the body
  // itself: the rounded block of test_shell opened at the front only, the
  // front ending smoothly into the kept rounding (patch 0100 makes it an
  // error; before, IsDone with the unshelled block).
  BRepPrimAPI_MakeBox front_box(kW, kD, kH);
  BRepFilletAPI_MakeFillet round(front_box.Shape());
  round.Add(5, edge_near(front_box.Shape(), gp_Pnt(kW, 0, kH / 2)));
  NCollection_List<TopoDS_Shape> front;
  front.Append(face_near(round.Shape(), gp_Pnt(27.5, 0, kH / 2)));
  BRepOffsetAPI_MakeThickSolid hollowed;
  hollowed.MakeThickSolidByJoin(round.Shape(), front, -1, 1e-7, BRepOffset_Skin, false, false,
                                GeomAbs_Intersection);
  CHECK(!hollowed.IsDone());
}

} // namespace

void shell_vanishing_tests() {
  guarded("test_chamfer_strip", test_chamfer_strip);
  guarded("test_rounding", test_rounding);
  guarded("test_boss", test_boss);
  guarded("test_kernel_alone", test_kernel_alone);
}

} // namespace test
