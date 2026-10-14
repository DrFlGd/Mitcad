// SPDX-License-Identifier: MIT
// Profile regions from every segment kind, holes and loop orientation.

#include <BRep_Tool.hxx>
#include <TopExp.hxx>

#include <cmath>

#include "check.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

Segment line(const char* name, double x0, double y0, double x1, double y1) {
  Segment s;
  s.name = name;
  s.kind = SegmentKind::Line;
  s.start = gp_Pnt2d(x0, y0);
  s.end = gp_Pnt2d(x1, y1);
  return s;
}

double area_of(const Region& region, const Frame& frame = Frame()) {
  return mass_properties(*profile_shape(frame, {region})).area;
}

void test_rectangle_profile_names_its_edges() {
  const ProfileFace profile = make_profile(Frame(), rectangle(1, 0, 0, 60, 40));
  CHECK(profile.edges.size() == 4);
  CHECK(profile.edges[1].second == "c2[c1,c3]");
  CHECK(near(area_of(rectangle(1, 0, 0, 60, 40)), 2400.0));
}

void test_arcs_ellipses_and_splines() {
  // A half disc: a line and an arc counter-clockwise from 90 to 270 degrees.
  Segment arc;
  arc.name = "c2[c1,c1]";
  arc.kind = SegmentKind::Arc;
  arc.center = gp_Pnt2d(0, 0);
  arc.radius = 10;
  arc.start_angle = kPi / 2;
  arc.end_angle = 3 * kPi / 2;
  CHECK(near(area_of(Region{"r", {Loop{{line("c1[c2,c2]", 0, -10, 0, 10), arc}}}}), kPi * 50.0));

  Segment ellipse;
  ellipse.name = "c3";
  ellipse.kind = SegmentKind::Ellipse;
  ellipse.center = gp_Pnt2d(5, 5);
  ellipse.radius = 20;
  ellipse.minor_radius = 10;
  ellipse.rotation = kPi / 6;
  CHECK(near(area_of(Region{"r", {Loop{{ellipse}}}}), kPi * 200.0));

  // Half an ellipse closed by its major axis.
  Segment half = ellipse;
  half.kind = SegmentKind::EllipseArc;
  half.center = gp_Pnt2d(0, 0);
  half.rotation = 0;
  half.start_angle = 0;
  half.end_angle = kPi;
  CHECK(near(area_of(Region{"r", {Loop{{half, line("c4", -20, 0, 20, 0)}}}}), kPi * 100.0));

  // A quadratic Bezier arch over a 10 mm chord: 2/3 of base times height.
  Segment arch;
  arch.name = "c5";
  arch.kind = SegmentKind::BSpline;
  arch.degree = 2;
  arch.poles = {gp_Pnt2d(0, 0), gp_Pnt2d(5, 10), gp_Pnt2d(10, 0)};
  arch.knots = {0, 1};
  arch.multiplicities = {3, 3};
  CHECK(near(area_of(Region{"r", {Loop{{arch, line("c6", 10, 0, 0, 0)}}}}), 2.0 / 3.0 * 10 * 5));

  // A rational quadratic B-spline is an exact quarter circle.
  Segment quarter = arch;
  quarter.poles = {gp_Pnt2d(10, 0), gp_Pnt2d(10, 10), gp_Pnt2d(0, 10)};
  quarter.weights = {1, std::sqrt(0.5), 1};
  const Region sector{"r", {Loop{{quarter, line("c7", 0, 10, 0, 0), line("c8", 0, 0, 10, 0)}}}};
  CHECK(near(area_of(sector), kPi * 25.0));
}

void test_holes_and_orientation() {
  Region plate = rectangle(1, 0, 0, 40, 20);
  plate.loops.push_back(circle(5, 20, 10, 5).loops[0]);
  CHECK(near(area_of(plate), 800.0 - kPi * 25.0));

  // Loops given clockwise give the same face, with the sketch normal.
  Region reversed = plate;
  std::reverse(reversed.loops[0].segments.begin(), reversed.loops[0].segments.end());
  for (Segment& s : reversed.loops[0].segments) {
    std::swap(s.start, s.end);
  }
  CHECK(near(area_of(reversed), 800.0 - kPi * 25.0));
  ExtrudeSpec spec;
  spec.feature = "F2";
  spec.regions = {reversed};
  spec.end = 10;
  const BoundingBox box = bounding_box(*mitcad::geometry::extrude(spec));
  CHECK(near(box.min.Z(), 0.0) && near(box.max.Z(), 10.0));
}

void test_a_hole_that_touches_the_outer_boundary() {
  // A disc of radius 10 in two arcs (vertices at (10, 0) and (-10, 0)) with a
  // round hole of radius 5 tangent to it at (10, 0): the hole is a circle cut
  // there (one arc round to the point) or a whole circle.
  const auto arc = [](const char* name, double x, double y, double radius, double from, double to) {
    Segment s;
    s.name = name;
    s.kind = SegmentKind::Arc;
    s.center = gp_Pnt2d(x, y);
    s.radius = radius;
    s.start_angle = from;
    s.end_angle = to;
    return s;
  };
  Segment whole;
  whole.name = "c3";
  whole.kind = SegmentKind::Circle;
  whole.center = gp_Pnt2d(5, 0);
  whole.radius = 5;
  const Loop outer{{arc("c1[c3,c2]", 0, 0, 10, 0, kPi), arc("c2[c1,c3]", 0, 0, 10, kPi, 2 * kPi)}};
  const Segment cut = arc("c3[c1,c1]", 5, 0, 5, 0, 2 * kPi);
  for (const Segment& hole : {whole, cut}) {
    const Region region{"r{c1,c2}", {outer, Loop{{hole}}}};
    const ProfileFace profile = make_profile(Frame(), region);
    CHECK(profile.pinched);
    CHECK(profile.edges.size() == 3);
    CHECK(near(area_of(region), kPi * 75.0));
    // The prism has the face's area times the length, and the hole's wall.
    const ExtrudeFeatureSpec spec = [&] {
      ExtrudeFeatureSpec s;
      s.feature = "F2";
      s.regions = {region};
      s.side1.distance = 4.0;
      return s;
    }();
    const ShapePtr solid = extrude_feature(spec);
    CHECK(near(mass_properties(*solid).volume, kPi * 75.0 * 4.0));
  }
  // The same boundary without the hole touching it is no different: the
  // face is not pinched.
  const Region apart{"r", {outer, Loop{{arc("c3", 3, 0, 2, 0, 2 * kPi)}}}};
  CHECK(!make_profile(Frame(), apart).pinched);
  CHECK(near(area_of(apart), kPi * (100.0 - 4.0)));
  // Two regions that meet along the pinched one's boundary come out as one
  // solid: a disc of radius 10 with the hole and the lens of the hole
  // cut away from the outside beside it (a ring touching the face).
  const Segment half_hole = arc("c4[c3,c3]", 5, 0, 5, 0, 2 * kPi);
  const Region ring{"r{c1,c2}", {outer, Loop{{half_hole}}}};
  const Region inner = [&] {
    Segment s = half_hole;
    s.name = "c3";
    return Region{"r{c3}", {Loop{{s}}}};
  }();
  ExtrudeFeatureSpec both;
  both.feature = "F3";
  both.regions = {ring, inner};
  both.side1.distance = 2.0;
  const ShapePtr one = extrude_feature(both);
  CHECK(solids(*one).size() == 1);
  CHECK(near(mass_properties(*one).volume, kPi * 100.0 * 2.0));
}

void test_holes_touching_tangentially() {
  // mitcad#235. An arch over a line, with a round hole on the line where its
  // two pieces meet (a ring of 1.75 and 2.75 mm cut by a line along the
  // inner circle's bottom): the hole is tangent to both pieces at their
  // vertex.
  const auto arc = [](const char* name, double x, double y, double radius, double from, double to) {
    Segment s;
    s.name = name;
    s.kind = SegmentKind::Arc;
    s.center = gp_Pnt2d(x, y);
    s.radius = radius;
    s.start_angle = from;
    s.end_angle = to;
    return s;
  };
  // The values as the sketch solved them, a few ulps off round numbers.
  const double reach = std::sqrt(2.75 * 2.75 - 1.75 * 1.75);
  const Loop arch{{line("c1[c3,c2]", -43.021320343559651, -6.5000000000000098, -40.900000000000006,
                        -6.5000000000000124),
                   line("c2[c1,c3]", -40.900000000000006, -6.5000000000000124, -38.77867965644036,
                        -6.5000000000000124),
                   arc("c3[c2,c1]", -40.900000000000006, -4.7500000000000124, 2.7500000000000009,
                       5.5934103063940874, 10.114552961554878)}};
  const Region pierced{"r{c1,c2,c3}",
                       {arch, Loop{{arc("c4[c1+c2,c1+c2]", -40.900000000000006, -4.7500000000000124,
                                        1.7500000000000002, 4.7123889803846897, 10.995574287564276)}}}};
  // On a plane off the origin, as the stored sketch has it.
  Frame side;
  side.origin = gp_Pnt(1, 6.5, -54.4);
  side.x_axis = gp_Dir(0, 0, -1);
  side.y_axis = gp_Dir(0, 1, 0);
  const double segment = 2.75 * 2.75 * std::acos(1.75 / 2.75) - 1.75 * reach;
  for (const Frame& frame : {Frame(), side}) {
    const ProfileFace face = make_profile(frame, pierced);
    CHECK(face.pinched);
    CHECK(near(area_of(pierced, frame), kPi * (2.75 * 2.75 - 1.75 * 1.75) - segment, 1e-6));
    ExtrudeFeatureSpec spec;
    spec.feature = "F2";
    spec.frame = frame;
    spec.direction = frame.normal();
    spec.regions = {pierced};
    spec.side1.distance = 4.0;
    const ShapePtr solid = extrude_feature(spec);
    CHECK(near(mass_properties(*solid).volume, 4.0 * (kPi * (2.75 * 2.75 - 1.75 * 1.75) - segment), 1e-6));
  }
  // A hole of arcs whose vertex is where it touches the outer circle inside:
  // a circle of 8 about (0, 3) holding a lens of a circle of 5 about the
  // origin (tangent at (0, -5)) and one of 6.5 about (0, 3).
  const Loop disc{{arc("c5[c5,c5]", 0, 3, 8, 1.5 * kPi, 3.5 * kPi)}};
  // Where the circles of 5 and 6.5 cross: x^2 + y^2 = 25, x^2 + (y-3)^2 = 42.25.
  const double y = (25.0 - 42.25 + 9.0) / 6.0;
  const double x = std::sqrt(25.0 - y * y);
  const double a5 = std::atan2(y, x);
  const double a65 = std::atan2(y - 3.0, x);
  const Loop lens{{arc("c6[c7,c7]", 0, 3, 6.5, a65 + 2.0 * kPi, kPi - a65 + 2.0 * kPi),
                   arc("c7[c6,c7]", 0, 0, 5, kPi - a5, 1.5 * kPi),
                   arc("c7[c7,c6]", 0, 0, 5, 1.5 * kPi, 2.0 * kPi + a5)}};
  const Region holed{"r{c5}", {disc, lens}};
  const ProfileFace inside = make_profile(Frame(), holed);
  CHECK(inside.pinched);
  CHECK(area_of(holed) > 0.0 && area_of(holed) < kPi * 64.0);
}

void test_profile_on_another_plane() {
  // The XZ plane seen from -Y: sketch x = X, sketch y = Z.
  Frame frame;
  frame.origin = gp_Pnt(0, 5, 0);
  frame.x_axis = gp_Dir(1, 0, 0);
  frame.y_axis = gp_Dir(0, 0, 1);
  CHECK(frame.normal().IsEqual(gp_Dir(0, -1, 0), 1e-12));
  const BoundingBox box = bounding_box(*profile_shape(frame, {rectangle(1, 2, 3, 10, 20)}));
  CHECK(near(box.min.X(), 2) && near(box.max.X(), 12) && near(box.min.Z(), 3) && near(box.max.Z(), 23));
  CHECK(near(box.min.Y(), 5) && near(box.max.Y(), 5));
}

void test_segments_meeting_within_the_sketch_tolerance() {
  // A rectangle whose corner is 3e-7 mm off: over OCCT's tolerance, but one
  // point for the sketch (imported sketches).
  const double off = 3e-7;
  const Region region{"r", {Loop{{line("c1", 0, 0, 30, 0), line("c2", 30, -off, 30, 15),
                                  line("c3", 30, 15, 0, 15), line("c4", 0, 15, 0, 0)}}}};
  const ProfileFace profile = make_profile(Frame(), region);
  CHECK(profile.edges.size() == 4);
  CHECK(profile.edges[1].second == "c2");
  CHECK(near(area_of(region), 450.0));
}

void test_invalid_profiles() {
  CHECK(throws_with([] { make_profile(Frame(), Region{"r", {}}); }, "has no boundary"));
  const Region open{"r", {Loop{{line("c1", 0, 0, 10, 0), line("c2", 10, 0, 10, 10)}}}};
  CHECK(throws_with([&] { make_profile(Frame(), open); }, "not closed"));
  const Region gap{"r", {Loop{{line("c1", 0, 0, 10, 0), line("c2", 20, 0, 20, 10)}}}};
  CHECK(throws_with([&] { make_profile(Frame(), gap); }, "segment c2: it does not connect"));
  const Region point{"r", {Loop{{line("c1", 0, 0, 0, 0)}}}};
  CHECK(throws_with([&] { make_profile(Frame(), point); }, "segment c1: the line has zero length"));
  Segment ellipse;
  ellipse.name = "c3";
  ellipse.kind = SegmentKind::Ellipse;
  ellipse.radius = 5;
  ellipse.minor_radius = 10;
  CHECK(throws_with([&] { make_profile(Frame(), Region{"r", {Loop{{ellipse}}}}); },
                    "major radius is smaller"));
  CHECK(throws_with([] { rectangle_region(0, 0, 0, 5); }, "width must be greater than zero"));
}

void test_a_circle_starts_where_its_rotation_says() {
  // A whole circle starts on the x axis, or at the angle `rotation` gives (its
  // seam: the vertex its one edge starts and ends at).
  const auto seam = [](double rotation) {
    Segment circle;
    circle.name = "c1";
    circle.kind = SegmentKind::Circle;
    circle.center = gp_Pnt2d(1, 2);
    circle.radius = 5;
    circle.rotation = rotation;
    const ProfileFace profile = make_profile(Frame(), Region{"r", {Loop{{circle}}}});
    CHECK(profile.edges.size() == 1);
    return BRep_Tool::Pnt(TopExp::FirstVertex(profile.edges[0].first));
  };
  const gp_Pnt on_x = seam(0.0);
  CHECK(near(on_x.X(), 6.0) && near(on_x.Y(), 2.0));
  const gp_Pnt up = seam(kPi / 2);
  CHECK(near(up.X(), 1.0) && near(up.Y(), 7.0));
  const gp_Pnt back = seam(kPi);
  CHECK(near(back.X(), -4.0) && near(back.Y(), 2.0));
}

} // namespace

void profile_tests() {
  test_rectangle_profile_names_its_edges();
  test_arcs_ellipses_and_splines();
  test_holes_and_orientation();
  test_a_hole_that_touches_the_outer_boundary();
  test_holes_touching_tangentially();
  test_profile_on_another_plane();
  test_segments_meeting_within_the_sketch_tolerance();
  test_invalid_profiles();
  test_a_circle_starts_where_its_rotation_says();
}

} // namespace test
