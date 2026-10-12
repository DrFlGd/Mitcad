// SPDX-License-Identifier: MIT
// The name grammar on the C++ side: composing, matching and parsing.

#include "check.hpp"

#include <algorithm>
#include <string>
#include <vector>

#include <BRepPrimAPI_MakeBox.hxx>
#include <BRep_Builder.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Pnt.hxx>

#include "mitcad/geometry/shape.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

void test_face_names_and_matching() {
  CHECK(face_name("F3", "side", "c1[c4,c2]") == "F3:side(c1[c4,c2])");
  CHECK(face_name("F7", "top") == "F7:top");
  // A reference matches the name itself and all its split pieces.
  CHECK(name_matches("F3:end(r{c5})", "F3:end(r{c5})"));
  CHECK(name_matches("F3:end(r{c5})#1", "F3:end(r{c5})"));
  CHECK(name_matches("F3:end(r{c5})#1#0", "F3:end(r{c5})#1"));
  CHECK(!name_matches("F3:end(r{c5})#10", "F3:end(r{c5})#1"));
  CHECK(!name_matches("F3:end(r{c5})", "F3:end(r{c5})#1"));
  CHECK(!name_matches("F31:top", "F3"));
  CHECK(face_matches({"F2:side(c1)", "F4:side(c9)"}, "F4:side(c9)"));
  CHECK(!face_matches({}, "F4:side(c9)"));
}

void test_edge_and_vertex_names_are_canonical() {
  // Byte-wise order, the same as the Rust model's canonical order.
  CHECK(make_edge_name("F3:side(c2)", "F10:end(r{c1})") == "E{F10:end(r{c1})|F3:side(c2)}");
  CHECK(make_edge_name("F2:a", "F2:b", 1) == "E{F2:a|F2:b}#1");
  CHECK(make_vertex_name({"F2:c", "F2:a", "F2:b", "F2:a"}) == "V{F2:a|F2:b|F2:c}");
  CHECK(make_vertex_name({"F2:b", "F2:a"}, 0) == "V{F2:a|F2:b}#0");
}

void test_composite_names_parse() {
  const auto edge = parse_composite_name("E{F2:end(r{c1[c4,c2],c2[c1,c3]})|F5:fillet(E{F2:a|F2:b}#1)}#3");
  CHECK(edge && edge->kind == 'E' && edge->index == 3);
  CHECK(edge && edge->faces.size() == 2);
  if (edge && edge->faces.size() == 2) {
    CHECK(edge->faces[0] == "F2:end(r{c1[c4,c2],c2[c1,c3]})");
    CHECK(edge->faces[1] == "F5:fillet(E{F2:a|F2:b}#1)");
  }
  const auto vertex = parse_composite_name("V{F2:a|F2:b#0|F3:c(V{F1:x|F1:y|F1:z})}");
  CHECK(vertex && vertex->kind == 'V' && vertex->index == -1 && vertex->faces.size() == 3);

  for (const char* bad : {"", "E{}", "E{F2:a}", "E{F2:a|F2:b", "E{F2:a|F2:b}#", "E{F2:a|F2:b}x",
                          "E{F2:a||F2:b}", "F2:side(c1)", "X{F2:a|F2:b}", "E{F2:a|F2:b}#99999999999"}) {
    if (parse_composite_name(bad)) {
      std::fprintf(stderr, "parsed an invalid name: %s\n", bad);
      CHECK(false);
    }
  }
}

// A shape of many faces finds them through an index of their names
// (mitcad#181): the same faces as matching every face's names, in the same
// geometric order as a small shape gives them.
void test_large_shapes_find_faces_by_an_index() {
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  std::vector<Shape::NamedFace> named;
  for (int k = 0; k < 20; ++k) {
    const TopoDS_Shape box = BRepPrimAPI_MakeBox(gp_Pnt(3.0 * k, 0.0, 0.0), 1.0, 1.0, 1.0).Shape();
    builder.Add(compound, box);
    int f = 0;
    for (TopExp_Explorer it(box, TopAbs_FACE); it.More(); it.Next(), ++f) {
      // Pieces of one face of each box ("#k"), names shared by every
      // box's face, and a face of two names.
      NameList names{"F1:side(c" + std::to_string(f) + ")#" + std::to_string(k)};
      if (f == 5) {
        names.push_back("F2:end(r{c" + std::to_string(k % 3) + "})");
      }
      named.push_back({it.Current(), names});
    }
  }
  const Shape shape(compound, named);
  CHECK(shape.face_count() == 120);
  const auto matching = [&](const std::string& reference) {
    std::vector<int> found;
    for (int i = 0; i < shape.face_count(); ++i) {
      if (face_matches(shape.face_names(i), reference)) {
        found.push_back(i);
      }
    }
    return found;
  };
  for (const std::string reference :
       {"F1:side(c0)", "F1:side(c5)#7", "F2:end(r{c1})", "F1:side(c9)", "F1", "F1:side(c3)#1"}) {
    std::vector<int> found = shape.find_faces(reference);
    std::vector<int> sorted = found;
    std::sort(sorted.begin(), sorted.end());
    CHECK(sorted == matching(reference));
  }
  CHECK(shape.find_faces("F1:side(c0)").size() == 20);
  CHECK(shape.find_faces("F1:side(c5)#7").size() == 1);
  CHECK(shape.find_faces("F2:end(r{c1})").size() == 7);
  CHECK(shape.find_faces("F1").empty());
  // In the geometric order: along x, as the boxes lie (and as they were
  // added).
  const std::vector<int> sides = shape.find_faces("F1:side(c0)");
  CHECK(std::is_sorted(sides.begin(), sides.end()));
}

} // namespace

void naming_tests() {
  test_face_names_and_matching();
  test_edge_and_vertex_names_are_canonical();
  test_composite_names_parse();
  test_large_shapes_find_faces_by_an_index();
}

} // namespace test
