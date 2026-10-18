// SPDX-License-Identifier: MIT
// Geometry facade tests with real OCCT; see check.hpp.

#include <set>

#include "check.hpp"

namespace test {

std::string rectangle_segment(int first, int i) {
  const auto curve = [first](int k) { return "c" + std::to_string(first + (k + 4) % 4); };
  return curve(i) + "[" + curve(i - 1) + "," + curve(i + 1) + "]";
}

std::string rectangle_region_name(int first) {
  std::string name = "r{";
  for (int i = 0; i < 4; ++i) {
    name += (i == 0 ? "" : ",") + rectangle_segment(first, i);
  }
  return name + "}";
}

Region rectangle(int first, double x, double y, double width, double height) {
  Region region = mitcad::geometry::rectangle_region(x, y, width, height);
  region.name = rectangle_region_name(first);
  for (int i = 0; i < 4; ++i) {
    region.loops[0].segments[static_cast<std::size_t>(i)].name = rectangle_segment(first, i);
  }
  return region;
}

Region circle(int curve, double cx, double cy, double radius) {
  Region region = mitcad::geometry::circle_region(cx, cy, radius);
  region.loops[0].segments[0].name = "c" + std::to_string(curve);
  region.name = "r{c" + std::to_string(curve) + "}";
  return region;
}

ShapePtr extrude(const std::string& feature, std::vector<Region> regions, double start, double end) {
  mitcad::geometry::ExtrudeSpec spec;
  spec.feature = feature;
  spec.regions = std::move(regions);
  spec.start = start;
  spec.end = end;
  return mitcad::geometry::extrude(spec);
}

std::string side(const std::string& feature, int first, int i) {
  return mitcad::geometry::face_name(feature, "side", rectangle_segment(first, i));
}

std::string start_cap(const std::string& feature, int first) {
  return mitcad::geometry::face_name(feature, "start", rectangle_region_name(first));
}

std::string end_cap(const std::string& feature, int first) {
  return mitcad::geometry::face_name(feature, "end", rectangle_region_name(first));
}

std::string edge(const std::string& a, const std::string& b) {
  return mitcad::geometry::make_edge_name(a, b);
}

bool has_name(const mitcad::geometry::NameList& names, const std::string& name) {
  return std::find(names.begin(), names.end(), name) != names.end();
}

int faces_named(const mitcad::geometry::Shape& shape, const std::string& name) {
  int count = 0;
  for (int i = 0; i < shape.face_count(); ++i) {
    count += has_name(shape.face_names(i), name) ? 1 : 0;
  }
  return count;
}

bool edge_names_unique(const mitcad::geometry::Shape& shape) {
  std::set<std::string> names;
  for (int i = 0; i < shape.edge_count(); ++i) {
    const auto faces = shape.edge_faces(i);
    const bool named_faces = faces[0] >= 0 && faces[1] >= 0 &&
                             !shape.face_names(faces[0]).empty() &&
                             !shape.face_names(faces[1]).empty();
    if (named_faces && shape.edge_name(i).empty()) {
      std::fprintf(stderr, "edge %d between named faces has no name\n", i);
      return false;
    }
    if (!shape.edge_name(i).empty() && !names.insert(shape.edge_name(i)).second) {
      std::fprintf(stderr, "edge name used twice: %s\n", shape.edge_name(i).c_str());
      return false;
    }
    // Every name resolves back to its own edge.
    if (!shape.edge_name(i).empty() && shape.find_edges(shape.edge_name(i)) != std::vector<int>{i}) {
      std::fprintf(stderr, "edge name does not resolve to its edge: %s\n",
                   shape.edge_name(i).c_str());
      return false;
    }
  }
  return true;
}

} // namespace test

int main() {
  // Every operation of the run is checked for changing its inputs (T0e).
  mitcad::geometry::enable_input_checks(true);
  test::input_check_self_tests();
  test::naming_tests();
  test::profile_tests();
  test::extrude_tests();
  test::boolean_tests();
  test::dressup_tests();
  test::query_tests();
  test::transform_tests();
  // Face operations (F2).
  test::faceops_tests();
  test::split_tests();
  test::datum_tests();
  test::extrude_feature_tests();
  test::revolve_tests();
  test::hole_tests();
  test::thread_tests();
  // Sweeps, lofts, pipes, coils, ribs and webs (F3).
  test::sweeps_tests();
  test::text_tests();
  // Lofts with end conditions and rails, curve and surface fitting (P2).
  test::skin_tests();
  // Shapes with their names as bytes (P7d).
  test::persist_tests();
  // Long operations stopped on request (P7e).
  test::cancel_tests();
  // Operations leave their inputs as they are (T0e); none of the run's did.
  test::input_check_tests();
  // Booleans on perforated bodies (mitcad#72).
  test::far_features_tests();
  // The material a near copy of a body lacks (mitcad#85).
  test::removed_tests();
  // A mirror image joined only where it differs from the body (mitcad#88).
  test::mirror_join_tests();
  // Roundings wider than a neighbouring face (mitcad#121).
  test::fillet_overflow_tests();
  // Roundings OCCT's fillet failed on, with the port's patches (mitcad#122).
  test::fillet_kernel_tests();
  // Ends, seams and corner caps of OCCT's roundings (mitcad#133).
  test::fillet_end_tests();
  // Allocations that fail inside OCCT, with the port's patch (mitcad#132).
  test::allocation_tests();
  // ShapeFix on faces with many holes, with the port's patch 0060 (mitcad#60).
  test::shapefix_tests();
  // A chamfer whose pieces meet at a vertex of the edge they reach, with the
  // port's patches 0090 and 0091 (mitcad#160).
  test::chamfer_kernel_tests();
  // Shells whose wall makes a narrow face vanish, with the port's patch 0100
  // (mitcad#172).
  test::shell_vanishing_tests();
  // Walked roundings that are true rolling balls, with the port's patches
  // 0075-0077 (mitcad#121).
  test::fillet_rolling_ball_tests();
  // The faces a run of features made, copied by a pattern.
  test::face_copy_tests();
  // Operations on bodies of thousands of faces, with the port's patch 0110
  // (mitcad#121).
  test::large_body_tests();
  // Corners where a rounding ends, with the port's patches 0130 and later
  // (mitcad#236).
  test::fillet_corner_tests();
  // Chamfers wider than a face beside them, with the port's patches 0140 and
  // later (mitcad#240).
  test::chamfer_overflow_tests();
  // Roundings whose results were invalid or whose computation failed, with
  // the port's patches 0150 and later (mitcad#239).
  test::fillet_result_tests();
  // Fillet contours that stop where their edges turn, with the port's patch
  // 0180 (mitcad#241).
  test::fillet_chain_tests();
  // The tangent chains of a body's edges and faces (mitcad#290).
  test::tangent_chain_tests();
  // Roundings that run over cut-outs of their faces (mitcad#311).
  test::fillet_cutout_tests();
  // Section curves of sketches (mitcad#389).
  test::section_curve_tests();
  for (const std::string& change : mitcad::geometry::input_changes()) {
    std::fprintf(stderr, "%s\n", change.c_str());
  }
  CHECK(mitcad::geometry::input_changes().empty());
  if (test::failures == 0) {
    std::puts("test_geometry: all checks passed");
  }
  return test::failures == 0 ? 0 : 1;
}
