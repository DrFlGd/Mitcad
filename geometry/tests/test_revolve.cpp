// SPDX-License-Identifier: MIT
// Revolution: angles, sides, full turns, targets and names. Volumes follow
// the T0 models (rev_*).

#include "check.hpp"

#include <array>
#include <cmath>
#include <vector>

namespace test {
namespace {

using namespace mitcad::geometry;

// The rectangle (10, 0)-(20, 30) about the Y axis, as in rev_full_sketch_line.
RevolveSpec tube(double angle) {
  RevolveSpec spec;
  spec.feature = "F3";
  spec.regions = {rectangle(1, 10, 0, 10, 30)};
  spec.axis = gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 1, 0));
  spec.angle1 = angle;
  return spec;
}

const double kTube = kPi * (20 * 20 - 10 * 10) * 30;

void test_angles() {
  const ShapePtr full = revolve(tube(2 * kPi));
  CHECK(near(volume(*full), kTube));
  CHECK(full->face_count() == 4);
  CHECK(faces_named(*full, side("F3", 1, 1)) == 1); // x = 20: the outer cylinder
  CHECK(faces_named(*full, side("F3", 1, 3)) == 1); // x = 10: the inner one
  CHECK(full->find_faces(start_cap("F3", 1)).empty() && full->find_faces(end_cap("F3", 1)).empty());
  CHECK(full->find_edges(edge(side("F3", 1, 1), side("F3", 1, 1))).size() == 1); // the seam
  CHECK(edge_names_unique(*full));
  // The flat rings at y = 0 and y = 30: their planes' normals point out
  // of the body (a revolution's planes can be left-handed).
  for (const int k : {0, 2}) {
    const gp_Pln ring = face_plane(*full, side("F3", 1, k));
    const gp_Vec out(gp_Pnt(0, 15, 0), ring.Location());
    CHECK(near(std::abs(ring.Axis().Direction().Y()), 1.0) && ring.Axis().Direction().Dot(gp_Dir(out)) > 0.9);
  }

  // A quarter, turning right-handed about +Y: from +x towards -z.
  const ShapePtr quarter = revolve(tube(kPi / 2));
  CHECK(near(volume(*quarter), kTube / 4));
  const BoundingBox box = bounding_box(*quarter);
  CHECK(near(box.min.Z(), -20) && near(box.max.Z(), 0) && near(box.min.X(), 0));
  CHECK(faces_named(*quarter, start_cap("F3", 1)) == 1 && faces_named(*quarter, end_cap("F3", 1)) == 1);
  const std::vector<int> start = quarter->find_faces(start_cap("F3", 1));
  CHECK(start.size() == 1 && near(bounding_box(Shape(quarter->face(start.at(0)))).max.Z(), 0) &&
        near(bounding_box(Shape(quarter->face(start.at(0)))).min.Z(), 0));
  CHECK(edge_names_unique(*quarter));

  // Symmetric 45 + 45 degrees (rev_symmetric) and two sides 60 + 30
  // (rev_two_sides): a quarter each; the start is the end of side one.
  RevolveSpec symmetric = tube(kPi / 4);
  symmetric.angle2 = kPi / 4;
  const ShapePtr both = revolve(symmetric);
  CHECK(near(volume(*both), kTube / 4));
  const std::vector<int> far = both->find_faces(start_cap("F3", 1));
  CHECK(far.size() == 1 && bounding_box(Shape(both->face(far.at(0)))).max.Z() < -1);
  RevolveSpec two = tube(kPi / 3);
  two.angle2 = kPi / 6;
  CHECK(near(volume(*revolve(two)), kTube / 4));
  two.angle1 = 2 * kPi;
  CHECK(throws_with([&] { revolve(two); }, "more than a full turn"));
}

void test_axes() {
  // About a profile edge: the cylinder of rev_groove_cut, r = 20, 40 long.
  RevolveSpec cylinder = tube(2 * kPi);
  cylinder.regions = {rectangle(1, 0, 0, 20, 40)};
  CHECK(near(volume(*revolve(cylinder)), kPi * 400 * 40));
  // The groove: (18, 15)-(20, 25) about the Y axis, cut.
  RevolveSpec groove = tube(2 * kPi);
  groove.feature = "F5";
  groove.regions = {rectangle(5, 18, 15, 2, 10)};
  const BooleanResult cut = boolean(BooleanOp::Cut, {revolve(cylinder).get()}, *revolve(groove));
  CHECK(cut.pieces.size() == 1 &&
        near(volume(*cut.pieces.at(0).shape), kPi * 400 * 40 - kPi * (400 - 324) * 10));

  // A profile across the axis is swept in its parts on either side of it for a
  // turn of less than half (a quarter turn of each half of the rectangle: half
  // the cylinder), else it fails (the parts overlap); so does one the axis
  // passes through.
  RevolveSpec across = tube(kPi / 2);
  across.regions = {rectangle(1, -5, 0, 10, 30)};
  CHECK(near(volume(*revolve(across)), kPi * 25 * 30 / 2, 1e-5));
  across.angle1 = kPi;
  CHECK(throws_with([&] { revolve(across); }, "crosses the revolve axis"));
  across.axis = gp_Ax1(gp_Pnt(0, 10, 0), gp_Dir(0, 0, 1));
  CHECK(throws_with([&] { revolve(across); }, "passes through profile"));
  // An axis out of the sketch plane, parallel to it: a point at x lies
  // sqrt(x^2 + 25) from it, so the tube's area stays the same.
  RevolveSpec lifted = tube(2 * kPi);
  lifted.axis = gp_Ax1(gp_Pnt(0, 0, 5), gp_Dir(0, 1, 0));
  CHECK(near(volume(*revolve(lifted)), kTube, 1e-5));
}

void test_to_object() {
  // Up to the YZ plane: the first time the turn meets it is a quarter.
  RevolveSpec spec = tube(0);
  Target plane;
  plane.plane = gp_Pln(gp_Pnt(0, 0, 0), gp_Dir(1, 0, 0));
  spec.target = plane;
  const ShapePtr quarter = revolve(spec);
  CHECK(near(volume(*quarter), kTube / 4, 1e-5));
  CHECK(faces_named(*quarter, start_cap("F3", 1)) == 1 && faces_named(*quarter, end_cap("F3", 1)) == 1);
  spec.target->offset = 1;
  CHECK(throws_with([&] { revolve(spec); }, "no offset"));

  // Up to a body below z = -5 (mitcad#539): the turn (from +x towards -z)
  // first meets it where r sin(a) = 5, and leaves it again at pi - a. Up
  // to its first face the tube keeps what turns before the body, through
  // it to its last face half the tube less what lies past the body's far
  // side; both are the strip 0 <= |z| <= 5 of the half ring (x > 0 and
  // x < 0 alike) times the tube's height.
  const ShapePtr below = extrude("F9", {rectangle(1, -50, -5, 100, 45)}, -50, -5);
  Target body;
  body.kind = Target::Kind::Body;
  body.body = below;
  spec.target = body;
  const auto strip = [](double a) { return 2.5 * std::sqrt(a * a - 25) + a * a / 2 * std::asin(5 / a); };
  const double ends = 30 * (strip(20) - strip(10));
  const ShapePtr first = revolve(spec);
  CHECK_NEAR_TOL(volume(*first), ends, 1e-5);
  spec.target->through = true;
  const ShapePtr through = revolve(spec);
  CHECK_NEAR_TOL(volume(*through), kTube / 2 - ends, 1e-5);
  CHECK(faces_named(*through, start_cap("F3", 1)) == 1 && !through->find_faces(end_cap("F3", 1)).empty());
  // Its far side, where the turn leaves the body, lies in z = -5 on the
  // other side of the axis: the outer wall ends at x = -sqrt(20^2 - 5^2).
  const BoundingBox box = bounding_box(*through);
  CHECK(near(box.min.X(), -std::sqrt(375.0), 1e-5) && near(box.max.Z(), 0, 1e-5) &&
        near(box.min.Z(), -20, 1e-5));
}

// A triangle c<first>..c<first+2> through the points (sketch coordinates).
Region triangle(const std::string& name, int first, const std::array<gp_Pnt2d, 3>& points) {
  Region region;
  region.name = name;
  Loop loop;
  for (std::size_t i = 0; i < 3; ++i) {
    Segment segment;
    segment.name = "c" + std::to_string(first + static_cast<int>(i));
    segment.start = points[i];
    segment.end = points[(i + 1) % 3];
    loop.segments.push_back(segment);
  }
  region.loops.push_back(loop);
  return region;
}

// Two regions that share an edge, with a straight outer side the shared edge
// cuts in two (a wedge between a wall of a cylinder and a cone and the line
// across both), revolved a full turn: one solid of three faces, the volume
// Pappus gives for the two triangles. Revolved apart and fused, the two
// parts of the cone face were merged into one face built wrong (2.6 % too
// much volume).
void test_touching_regions() {
  // The line from (27, 10) to (26, 50) crosses y = 20 at x = 26.75.
  const std::array<gp_Pnt2d, 3> lower{gp_Pnt2d(27, 10), gp_Pnt2d(27, 20), gp_Pnt2d(26.75, 20)};
  const std::array<gp_Pnt2d, 3> upper{gp_Pnt2d(26.75, 20), gp_Pnt2d(27, 20), gp_Pnt2d(26, 50)};
  RevolveSpec spec;
  spec.feature = "F3";
  spec.regions = {triangle("r{c1[c3,c2],c2[c1,c3],c3[c2,c1]}", 1, lower),
                  triangle("r{c4[c6,c5],c5[c4,c6],c6[c5,c4]}", 4, upper)};
  spec.axis = gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 1, 0));
  spec.angle1 = 2.0 * kPi;
  const auto pappus = [](const std::array<gp_Pnt2d, 3>& t) {
    const double area = std::abs((t[1].X() - t[0].X()) * (t[2].Y() - t[0].Y()) -
                                 (t[2].X() - t[0].X()) * (t[1].Y() - t[0].Y())) /
                        2.0;
    return 2.0 * kPi * (t[0].X() + t[1].X() + t[2].X()) / 3.0 * area;
  };
  const ShapePtr wedge = revolve(spec);
  CHECK(near(volume(*wedge), pappus(lower) + pappus(upper), 1e-9));
  CHECK(wedge->face_count() == 3);
  // The cone across both regions carries both their segments' names.
  const std::string lower_line = face_name("F3", "side", "c3");
  const std::string upper_line = face_name("F3", "side", "c6");
  CHECK(faces_named(*wedge, lower_line) == 1 && faces_named(*wedge, upper_line) == 1);
  CHECK(wedge->find_faces(lower_line) == wedge->find_faces(upper_line));
  CHECK(edge_names_unique(*wedge));
}

} // namespace

void revolve_tests() {
  guarded("test_angles", test_angles);
  guarded("test_axes", test_axes);
  guarded("test_to_object", test_to_object);
  guarded("test_touching_regions", test_touching_regions);
}

} // namespace test
