// SPDX-License-Identifier: MIT
// Join, Cut and Intersect with participant bodies: pieces, their sources and
// the face names they carry.

#include <utility>

#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRep_Builder.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp_Pln.hxx>

#include "../src/history.hpp"
#include "check.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

// Sketch1's 60 x 40 rectangle (c1..c4) extruded 20 mm by F2.
ShapePtr block() { return extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20); }

BooleanResult run(BooleanOp op, const std::vector<ShapePtr>& targets, const ShapePtr& tool) {
  std::vector<const Shape*> raw;
  for (const ShapePtr& target : targets) {
    raw.push_back(target.get());
  }
  return boolean(op, raw, *tool);
}

void test_join_keeps_names_and_names_new_edges() {
  // A 20 x 10 boss (c5..c8) from the sketch plane through the block's top.
  const ShapePtr boss = extrude("F4", {rectangle(5, 10, 10, 20, 10)}, 0, 30);
  const BooleanResult result = run(BooleanOp::Join, {block()}, boss);
  CHECK(result.touched == std::vector<bool>{true});
  CHECK(result.pieces.size() == 1);
  if (result.pieces.size() != 1) {
    return;
  }
  const Shape& joined = *result.pieces[0].shape;
  CHECK(result.pieces[0].sources == std::vector<std::size_t>{0});
  CHECK(near(volume(joined), 60.0 * 40.0 * 20.0 + 20.0 * 10.0 * 10.0));
  // The block's faces and the boss's sides and top keep their names. The
  // boss's base lies on the block's bottom face, which carries both names.
  for (int i = 0; i < 4; ++i) {
    CHECK(faces_named(joined, side("F2", 1, i)) == 1);
    CHECK(faces_named(joined, side("F4", 5, i)) == 1);
  }
  CHECK(faces_named(joined, end_cap("F2", 1)) == 1);
  CHECK(faces_named(joined, end_cap("F4", 5)) == 1);
  CHECK(joined.find_faces(start_cap("F4", 5)).size() == 1);
  CHECK(joined.find_faces(start_cap("F4", 5)) == joined.find_faces(start_cap("F2", 1)));
  CHECK(edge_names_unique(joined));
  // The edges where the boss meets the block's top have names now, so they
  // can be referenced (V0 could not).
  for (int i = 0; i < 4; ++i) {
    CHECK(joined.find_edges(edge(end_cap("F2", 1), side("F4", 5, i))).size() == 1);
  }
}

void test_join_merges_flush_faces() {
  const ShapePtr extension = extrude("F4", {rectangle(5, 60, 0, 20, 40)}, 0, 20);
  const BooleanResult result = run(BooleanOp::Join, {block()}, extension);
  CHECK(result.pieces.size() == 1);
  const Shape& joined = *result.pieces.at(0).shape;
  CHECK(joined.face_count() == 6);
  // The coplanar top faces became one face with both names.
  CHECK(joined.find_faces(end_cap("F2", 1)) == joined.find_faces(end_cap("F4", 5)));
  CHECK(joined.find_faces(end_cap("F2", 1)).size() == 1);
  CHECK(joined.find_faces(side("F2", 1, 1)).empty()); // the shared wall at x = 60
  CHECK(edge_names_unique(joined));
}

// A pillar (a circle of radius 10 extruded from 0 to 10, F2) and on it a
// ring turned about its axis (the rectangle 8..10 x 10..12, F4) whose
// profile lies on either side of the axis: the ring's seam faces the
// pillar's or lies opposite it. The join has one outer cylinder from 0 to
// 12, as the stored bodies of designs do: the face merge after the boolean
// failed to build the merged face's seam where the two seams lay half a
// turn apart (OCCT port patch 0220, mitcad#284).
void test_join_merges_coaxial_cylinders_whatever_their_seams() {
  for (const double side_of_axis : {1.0, -1.0}) {
    const ShapePtr pillar = extrude("F2", {circle(1, 0, 0, 10)}, 0, 10);
    mitcad::geometry::RevolveSpec spec;
    spec.feature = "F4";
    spec.frame.x_axis = gp_Dir(side_of_axis, 0, 0);
    spec.frame.y_axis = gp_Dir(0, 0, 1);
    spec.regions = {rectangle(2, 8, 10, 2, 2)};
    spec.axis = gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1));
    spec.angle1 = 2.0 * kPi;
    const ShapePtr ring = revolve(spec);
    const BooleanResult result = run(BooleanOp::Join, {pillar}, ring);
    CHECK(result.pieces.size() == 1);
    if (result.pieces.size() != 1) {
      continue;
    }
    const Shape& joined = *result.pieces[0].shape;
    CHECK(near(volume(joined), kPi * 100.0 * 10.0 + kPi * (100.0 - 64.0) * 2.0));
    // The bottom, the outer cylinder, the ring's top and inner wall, the
    // pillar's top inside the ring.
    CHECK(joined.face_count() == 5);
    const std::string pillar_wall = mitcad::geometry::face_name("F2", "side", "c1");
    const std::string ring_wall = side("F4", 2, 1); // x = 10 in the profile
    CHECK(faces_named(joined, pillar_wall) == 1);
    CHECK(joined.find_faces(pillar_wall) == joined.find_faces(ring_wall));
    CHECK(edge_names_unique(joined));
  }
}

void test_slivers_are_no_pieces() {
  // A result of the block and a flat solid beside it: two faces of a strip
  // 1.5 mm long, back to back (no volume), as a join of walls that continue
  // the body's walls left between coinciding faces. The strip is no piece;
  // a sliver alone stays the result.
  const ShapePtr body = block();
  const TopoDS_Face strip =
      BRepBuilderAPI_MakeFace(gp_Pln(gp_Pnt(0, -1, 0), gp_Dir(0, 1, 0)), 0.0, 1e-6, 0.0, 1.5).Face();
  BRep_Builder builder;
  TopoDS_Shell shell;
  builder.MakeShell(shell);
  builder.Add(shell, strip);
  builder.Add(shell, strip.Reversed());
  TopoDS_Solid flat;
  builder.MakeSolid(flat);
  builder.Add(flat, shell);
  TopoDS_Compound both;
  builder.MakeCompound(both);
  builder.Add(both, body->occt());
  builder.Add(both, flat);
  const std::vector<detail::FaceNamer::Piece> pieces = detail::FaceNamer(both).pieces();
  CHECK(pieces.size() == 1);
  CHECK(!pieces.empty() && near(volume(*pieces.at(0).shape), 60.0 * 40.0 * 20.0));
  CHECK(detail::FaceNamer(flat).pieces().size() == 1);
}

void test_join_with_several_participants() {
  const ShapePtr left = extrude("F2", {rectangle(1, 0, 0, 10, 10)}, 0, 10);
  const ShapePtr right = extrude("F3", {rectangle(5, 20, 0, 10, 10)}, 0, 10);
  const ShapePtr far = extrude("F4", {rectangle(9, 100, 0, 10, 10)}, 0, 10);
  // A bridge over the gap touches the left and right blocks only.
  const ShapePtr bridge = extrude("F6", {rectangle(13, 5, 0, 20, 10)}, 5, 15);
  const BooleanResult result = run(BooleanOp::Join, {left, far, right}, bridge);
  CHECK((result.touched == std::vector<bool>{true, false, true}));
  CHECK(result.pieces.size() == 1);
  CHECK((result.pieces.at(0).sources == std::vector<std::size_t>{0, 2}));

  // A tool that touches nothing joins nothing.
  const ShapePtr apart = extrude("F6", {rectangle(13, 50, 50, 5, 5)}, 0, 5);
  const BooleanResult none = run(BooleanOp::Join, {left, right}, apart);
  CHECK((none.touched == std::vector<bool>{false, false}) && none.pieces.empty());
}

void test_join_of_many_copies_finds_what_they_touch() {
  // Ten 2 mm cubes along x, as a pattern's copies (one shape of ten solids).
  std::vector<ShapePtr> cubes;
  std::vector<const Shape*> parts;
  for (int k = 0; k < 10; ++k) {
    cubes.push_back(extrude("F6", {rectangle(13, 10.0 * k, 0, 2, 2)}, 0, 2));
    parts.push_back(cubes.back().get());
  }
  const ShapePtr copies = unite(parts);
  // On a face of the fourth copy, on an edge of the sixth, a post 0.27 mm
  // from a corner of the eighth (their boxes overlap), a block 1.5e-6 mm
  // from the ninth, one inside the tenth, and one far from all.
  const ShapePtr face = extrude("F2", {rectangle(1, 32, 0, 5, 2)}, 0, 2);
  const ShapePtr edge = extrude("F3", {rectangle(5, 52, 2, 3, 3)}, 0, 2);
  const ShapePtr post = extrude("F4", {circle(9, 72.9, 2.9, 1)}, 0, 2);
  const ShapePtr gap = extrude("F5", {rectangle(10, 82.0000015, 0, 3, 2)}, 0, 2);
  const ShapePtr inside = extrude("F7", {rectangle(17, 90.5, 0.5, 1, 1)}, 0.5, 1.5);
  const ShapePtr far = extrude("F8", {rectangle(21, 0, 100, 2, 2)}, 0, 2);
  const BooleanResult result =
      run(BooleanOp::Join, {face, edge, post, gap, inside, far}, copies);
  CHECK((result.touched == std::vector<bool>{true, true, false, false, true, false}));
  // The swallowed block leaves no face of its own.
  std::vector<std::size_t> sources;
  for (const BooleanPiece& piece : result.pieces) {
    sources.insert(sources.end(), piece.sources.begin(), piece.sources.end());
  }
  std::sort(sources.begin(), sources.end());
  CHECK((sources == std::vector<std::size_t>{0, 1}));
}

void test_join_with_a_plate_of_many_holes() {
  // A 60 x 40 plate 2 mm thick with 40 x 26 holes: the faces near a tool
  // have over a thousand edges, so the tool's vertices are tried before the
  // faces are measured.
  Region plate = rectangle(1, 0, 0, 60, 40);
  int curve = 5;
  for (int i = 0; i < 40; ++i) {
    for (int j = 0; j < 26; ++j) {
      plate.loops.push_back(circle(curve++, 0.75 + 1.5 * i, 0.75 + 1.5 * j, 0.4).loops.at(0));
    }
  }
  const ShapePtr holed = extrude("F2", {plate}, 0, 2);
  // A small block above the plate between two posts on it, touching neither.
  const ShapePtr above = extrude("F3", {rectangle(1045, 2.8, 1.3, 0.4, 0.4)}, 3, 4);
  const auto posts = [](double from) {
    return extrude("F4", {rectangle(1049, 1.3, 1.3, 0.4, 0.4), rectangle(1053, 4.3, 1.3, 0.4, 0.4)},
                   from, 5);
  };
  // Standing on the plate between its holes: joined to it.
  const BooleanResult standing = run(BooleanOp::Join, {holed, above}, posts(2));
  CHECK((standing.touched == std::vector<bool>{true, false}));
  CHECK(standing.pieces.size() == 1);
  if (standing.pieces.size() == 1) {
    CHECK((standing.pieces[0].sources == std::vector<std::size_t>{0}));
    CHECK(near(volume(*standing.pieces[0].shape), volume(*holed) + 2 * 0.4 * 0.4 * 3));
  }
  // Sunk into the plate: joined too.
  CHECK((run(BooleanOp::Join, {holed, above}, posts(1)).touched == std::vector<bool>{true, false}));
  // A thousandth of a millimetre above it: nothing touched.
  const BooleanResult hovering = run(BooleanOp::Join, {holed, above}, posts(2.001));
  CHECK((hovering.touched == std::vector<bool>{false, false}) && hovering.pieces.empty());
  // A nanometre and a half above it (the noise of the geometry a file stores): it
  // still joins, as one solid with the plate.
  const BooleanResult nearly = run(BooleanOp::Join, {holed, above}, posts(2.0000015));
  CHECK((nearly.touched == std::vector<bool>{true, false}));
  CHECK(nearly.pieces.size() == 1);
  if (nearly.pieces.size() == 1) {
    CHECK((nearly.pieces[0].sources == std::vector<std::size_t>{0}));
    CHECK(near(volume(*nearly.pieces[0].shape), volume(*holed) + 2 * 0.4 * 0.4 * 3));
  }
}

// A region of straight lines c<first>.. through the points (closed).
Region polygon(int first, const std::vector<gp_Pnt2d>& points) {
  Region region;
  Loop loop;
  std::string name = "r{";
  for (std::size_t i = 0; i < points.size(); ++i) {
    Segment segment;
    segment.name = "c" + std::to_string(first + static_cast<int>(i));
    segment.start = points[i];
    segment.end = points[(i + 1) % points.size()];
    loop.segments.push_back(segment);
    name += (i == 0 ? "" : ",") + segment.name;
  }
  region.loops.push_back(loop);
  region.name = name + "}";
  return region;
}

ShapePtr extrude_on(const std::string& feature, const Frame& frame, std::vector<Region> regions, double start,
                    double end) {
  ExtrudeSpec spec;
  spec.feature = feature;
  spec.frame = frame;
  spec.direction = frame.normal();
  spec.regions = std::move(regions);
  spec.start = start;
  spec.end = end;
  return mitcad::geometry::extrude(spec);
}

// A join within a tolerance (mitcad#298), on one target.
BooleanResult run_within(const ShapePtr& target, const ShapePtr& tool, double within) {
  const std::vector<const Shape*> raw{target.get()};
  return boolean(BooleanOp::Join, raw, *tool, within);
}

void test_join_within_a_tolerance_reaches_a_tool_micrometres_away() {
  // A pin 2.6 micrometres above the block's top: apart for an exact join,
  // joined within 0.01 mm (as a body the import kept within its tolerance
  // joins the tool the file joined to it).
  const ShapePtr body = block();
  const ShapePtr pin = extrude("F4", {circle(5, 30, 20, 2.5)}, 20.0026, 21.5);
  const BooleanResult exact = run(BooleanOp::Join, {body}, pin);
  CHECK((exact.touched == std::vector<bool>{false}) && exact.pieces.empty());
  const BooleanResult within = run_within(body, pin, 0.01);
  CHECK((within.touched == std::vector<bool>{true}));
  CHECK(within.pieces.size() == 1);
  if (within.pieces.size() == 1) {
    CHECK((within.pieces[0].sources == std::vector<std::size_t>{0}));
    CHECK_NEAR_TOL(volume(*within.pieces[0].shape), volume(*body) + volume(*pin), 1e-5);
    CHECK(faces_named(*within.pieces[0].shape, "F4:side(c5)") == 1);
  }
  // Farther away than the tolerance: still apart.
  const ShapePtr far = extrude("F4", {circle(5, 30, 20, 2.5)}, 20.02, 21.5);
  const BooleanResult apart = run_within(body, far, 0.01);
  CHECK((apart.touched == std::vector<bool>{false}) && apart.pieces.empty());
  // Touching exactly: the exact join's result.
  const ShapePtr standing = extrude("F4", {circle(5, 30, 20, 2.5)}, 20, 21.5);
  const BooleanResult on = run_within(body, standing, 0.01);
  CHECK(on.pieces.size() == 1);
  CHECK(!on.pieces.empty() &&
        near(volume(*on.pieces[0].shape), volume(*run(BooleanOp::Join, {body}, standing).pieces.at(0).shape)));
}

// Half of a tee of two pipes of the same outer radius (13, wall 3), cut at
// the plane y = 0 through the main pipe's axis (z): the branch pipe (along
// -y, at z = 50) meets the main one where their outer cylinders are
// tangent, at (-13, 0, 50) on the cut face's edge. With a gusset plate
// 1.5 mm thick on the cut face, against the main pipe's outer line x = -13,
// `offset` off the face and its normal turned by `tilt` (the noise of a
// body that matches a file's within a micrometre).
std::pair<ShapePtr, ShapePtr> half_tee_and_plate(double offset, double tilt) {
  const ShapePtr main_outer = test::extrude("F1", {circle(1, 0, 0, 13)}, 0, 100);
  Frame xz;
  xz.origin = gp_Pnt(0, 0, 0);
  xz.x_axis = gp_Dir(1, 0, 0);
  xz.y_axis = gp_Dir(0, 0, 1);
  const ShapePtr branch_outer = extrude_on("F2", xz, {circle(2, 0, 50, 13)}, 0, 50);
  const ShapePtr main_inner = test::extrude("F3", {circle(3, 0, 0, 10)}, 0, 100);
  const ShapePtr branch_inner = extrude_on("F4", xz, {circle(4, 0, 50, 10)}, 0, 50);
  const ShapePtr half = test::extrude("F6", {rectangle(8, -20, 0, 40, 20)}, -1, 101);
  ShapePtr tee = run(BooleanOp::Cut, {main_outer}, half).pieces.at(0).shape;
  tee = run(BooleanOp::Join, {tee}, branch_outer).pieces.at(0).shape;
  tee = run(BooleanOp::Cut, {tee}, main_inner).pieces.at(0).shape;
  tee = run(BooleanOp::Cut, {tee}, branch_inner).pieces.at(0).shape;
  Frame side;
  side.origin = gp_Pnt(-13, offset, 0);
  side.x_axis = gp_Dir(tilt, 1, 0);
  side.y_axis = gp_Dir(0, 0, 1);
  const ShapePtr plate =
      extrude_on("F5", side, {polygon(5, {gp_Pnt2d(0, 0), gp_Pnt2d(50, 50), gp_Pnt2d(0, 100)})}, 0, 1.5);
  return {tee, plate};
}

void test_join_within_a_tolerance_on_a_near_coincident_face() {
  // The plate half a micrometre into the cut face, turned by 4e-9: the
  // exact join's result has an invalid face (a self-intersecting wire at
  // the tangent point); within 0.01 mm the join is retried with the
  // smallest fuzzy value that gives a valid result.
  const auto [tee, plate] = half_tee_and_plate(-5e-7, 3.9e-9);
  const BooleanResult within = run_within(tee, plate, 0.01);
  CHECK((within.touched == std::vector<bool>{true}));
  CHECK(within.pieces.size() == 1);
  if (within.pieces.size() == 1) {
    CHECK_NEAR_TOL(volume(*within.pieces[0].shape), volume(*tee) + 2500.0 * 1.5, 1e-6);
  }
  // Half a micrometre off the face (within the contact distance, but not
  // on the face for the exact boolean): the exact join leaves the plate a
  // solid of its own beside the tee; within the tolerance it joins it.
  const auto [tee2, apart] = half_tee_and_plate(5e-7, 0.0);
  const BooleanResult stray = run(BooleanOp::Join, {tee2}, apart);
  const BooleanResult joined = run_within(tee2, apart, 0.01);
  CHECK(joined.pieces.size() == 1);
  if (joined.pieces.size() == 1) {
    CHECK((joined.pieces[0].sources == std::vector<std::size_t>{0}));
    CHECK_NEAR_TOL(volume(*joined.pieces[0].shape), volume(*tee2) + 2500.0 * 1.5, 1e-6);
  }
  // Where the exact join merges them, the join within the tolerance is it.
  if (stray.pieces.size() == 1) {
    CHECK(near(volume(*stray.pieces[0].shape), volume(*joined.pieces.at(0).shape)));
  }
}

void test_cut_hole_names_the_walls() {
  const ShapePtr drill = extrude("F4", {circle(5, 30, 20, 5)}, 0, 30);
  const BooleanResult result = run(BooleanOp::Cut, {block()}, drill);
  CHECK(result.touched == std::vector<bool>{true});
  CHECK(result.pieces.size() == 1);
  const Shape& holed = *result.pieces.at(0).shape;
  CHECK(near(volume(holed), 60.0 * 40.0 * 20.0 - kPi * 25.0 * 20.0));
  CHECK(faces_named(holed, "F4:side(c5)") == 1);
  CHECK(holed.find_faces("F4:end(r{c5})").empty());
  CHECK(edge_names_unique(holed));
  // The rims of the hole.
  CHECK(holed.find_edges(edge(end_cap("F2", 1), "F4:side(c5)")).size() == 1);
  CHECK(holed.find_edges(edge(start_cap("F2", 1), "F4:side(c5)")).size() == 1);
}

void test_cut_splitting_a_face_numbers_the_pieces() {
  // A slot across the whole top, 5 mm deep: the top face splits in two, the
  // front and back faces stay whole (with a notch).
  const ShapePtr slot = extrude("F4", {rectangle(5, 25, -5, 10, 50)}, 15, 25);
  const BooleanResult result = run(BooleanOp::Cut, {block()}, slot);
  CHECK(result.pieces.size() == 1);
  const Shape& slotted = *result.pieces.at(0).shape;
  const std::string top = end_cap("F2", 1);
  CHECK(slotted.find_faces(top).size() == 2);
  CHECK(faces_named(slotted, top + "#0") == 1 && faces_named(slotted, top + "#1") == 1);
  // Pieces are numbered in geometric order: #0 is the one at smaller x.
  const std::vector<int> first = slotted.find_faces(top + "#0");
  CHECK(first.size() == 1 && near(bounding_box(Shape(slotted.face(first.at(0)))).max.X(), 25));
  CHECK(faces_named(slotted, side("F2", 1, 0)) == 1);
  CHECK(edge_names_unique(slotted));
  // An edge between the top and the front face exists on both sides of the
  // slot: the reference without "#k" resolves to both, numbered ones to one.
  const std::string top_front = edge(top, side("F2", 1, 0));
  CHECK(slotted.find_edges(top_front).size() == 2);
  CHECK(slotted.find_edges(edge(top + "#1", side("F2", 1, 0))).size() == 1);
}

void test_cut_splitting_the_body() {
  // A slot through the whole height splits the block into two pieces.
  const ShapePtr saw = extrude("F4", {rectangle(5, 25, -5, 10, 50)}, -1, 21);
  const BooleanResult result = run(BooleanOp::Cut, {block()}, saw);
  CHECK(result.touched == std::vector<bool>{true});
  CHECK(result.pieces.size() == 2);
  if (result.pieces.size() == 2) {
    for (const BooleanPiece& piece : result.pieces) {
      CHECK(piece.sources == std::vector<std::size_t>{0});
    }
    // Geometric order: the left piece first.
    CHECK(near(volume(*result.pieces[0].shape), 25.0 * 40.0 * 20.0));
    CHECK(near(volume(*result.pieces[1].shape), 25.0 * 40.0 * 20.0));
    CHECK(bounding_box(*result.pieces[0].shape).max.X() < 30);
    // The split faces are numbered across both pieces.
    const std::string top = end_cap("F2", 1);
    CHECK(faces_named(*result.pieces[0].shape, top + "#0") == 1);
    CHECK(faces_named(*result.pieces[1].shape, top + "#1") == 1);
    CHECK(faces_named(*result.pieces[0].shape, side("F2", 1, 3)) == 1);
    CHECK(faces_named(*result.pieces[1].shape, side("F2", 1, 1)) == 1);
  }
}

void test_cut_misses_or_removes() {
  const ShapePtr body = block();
  const ShapePtr far = extrude("F4", {rectangle(5, 100, 0, 10, 10)}, 0, 20);
  const BooleanResult missed = run(BooleanOp::Cut, {body}, far);
  CHECK(missed.touched == std::vector<bool>{false} && missed.pieces.empty());
  // Touching a face removes nothing either.
  const ShapePtr flush = extrude("F4", {rectangle(5, 60, 0, 10, 40)}, 0, 20);
  CHECK(run(BooleanOp::Cut, {body}, flush).touched == std::vector<bool>{false});
  // A tool in the body's box that passes it by (in the notch of an L):
  // the cut leaves it as it was, unchecked (mitcad#181).
  const ShapePtr notch = extrude("F4", {rectangle(5, 40, 20, 30, 30)}, -1, 30);
  const BooleanResult cut_notch = run(BooleanOp::Cut, {body}, notch);
  CHECK(cut_notch.touched == std::vector<bool>{true} && cut_notch.pieces.size() == 1);
  if (cut_notch.pieces.size() == 1) {
    const ShapePtr l_shape = cut_notch.pieces[0].shape;
    const ShapePtr inside = extrude("F6", {rectangle(9, 50, 30, 5, 5)}, 5, 10);
    const BooleanResult passed = run(BooleanOp::Cut, {l_shape}, inside);
    CHECK(passed.touched == std::vector<bool>{false} && passed.pieces.empty());
    CHECK(near(volume(*l_shape), 60.0 * 40.0 * 20.0 - 20.0 * 20.0 * 20.0));
  }
  const ShapePtr all = extrude("F4", {rectangle(5, -10, -10, 80, 60)}, -1, 30);
  const BooleanResult removed = run(BooleanOp::Cut, {body}, all);
  CHECK(removed.touched == std::vector<bool>{true} && removed.pieces.empty());
  // The inputs are cached model results and stay untouched.
  CHECK(near(volume(*body), 60.0 * 40.0 * 20.0));
  CHECK(body->face_count() == 6);
}

void test_cut_changes_by_more_than_a_billionth() {
  // A body changes when its volume changes by more than 1e-9 of it
  // (48 000 mm3 here): a pocket of 1e-4 mm3 does, one of 2e-5 mm3 does not.
  const ShapePtr deep = extrude("F4", {rectangle(5, 30, 20, 0.1, 0.1)}, 19.99, 21);
  CHECK(run(BooleanOp::Cut, {block()}, deep).touched == std::vector<bool>{true});
  const ShapePtr shallow = extrude("F4", {rectangle(5, 30, 20, 0.1, 0.1)}, 19.998, 21);
  CHECK(run(BooleanOp::Cut, {block()}, shallow).touched == std::vector<bool>{false});
}

void test_intersect() {
  const ShapePtr post = extrude("F4", {circle(5, 30, 20, 5)}, 10, 30);
  const BooleanResult result = run(BooleanOp::Intersect, {block()}, post);
  CHECK(result.touched == std::vector<bool>{true});
  CHECK(result.pieces.size() == 1);
  const Shape& common = *result.pieces.at(0).shape;
  CHECK(near(volume(common), kPi * 25.0 * 10.0));
  CHECK(faces_named(common, "F4:side(c5)") == 1);
  CHECK(faces_named(common, "F4:start(r{c5})") == 1);
  CHECK(faces_named(common, end_cap("F2", 1)) == 1);
  // A tool around the whole body leaves it as it is.
  const ShapePtr around = extrude("F4", {rectangle(5, -10, -10, 80, 60)}, -1, 30);
  CHECK(run(BooleanOp::Intersect, {block()}, around).touched == std::vector<bool>{false});
}

} // namespace

void boolean_tests() {
  test_join_keeps_names_and_names_new_edges();
  test_join_merges_flush_faces();
  guarded("test_join_merges_coaxial_cylinders_whatever_their_seams",
          test_join_merges_coaxial_cylinders_whatever_their_seams);
  test_slivers_are_no_pieces();
  test_join_with_several_participants();
  test_join_of_many_copies_finds_what_they_touch();
  test_join_with_a_plate_of_many_holes();
  test_cut_hole_names_the_walls();
  test_cut_splitting_a_face_numbers_the_pieces();
  test_cut_splitting_the_body();
  test_cut_misses_or_removes();
  test_cut_changes_by_more_than_a_billionth();
  test_intersect();
  // Joins within a tolerance (mitcad#298).
  test_join_within_a_tolerance_reaches_a_tool_micrometres_away();
  test_join_within_a_tolerance_on_a_near_coincident_face();
}

} // namespace test
