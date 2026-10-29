// SPDX-License-Identifier: MIT
// The name grammar on the C++ side: composing, matching and parsing.

#include "check.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Pnt.hxx>

#include "../src/history.hpp"
#include "../src/util.hpp"
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

// A shape of many faces finds an edge through the edges of the faces its
// reference names (mitcad#878): the same edges as checking every edge, in
// the same geometric order, and every edge's own name gives the edge.
void test_large_shapes_find_edges_by_their_faces() {
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  std::vector<Shape::NamedFace> named;
  for (int k = 0; k < 20; ++k) {
    const TopoDS_Shape box = BRepPrimAPI_MakeBox(gp_Pnt(3.0 * k, 0.0, 0.0), 1.0, 1.0, 1.0).Shape();
    builder.Add(compound, box);
    int f = 0;
    for (TopExp_Explorer it(box, TopAbs_FACE); it.More(); it.Next(), ++f) {
      NameList names{"F1:side(c" + std::to_string(f) + ")#" + std::to_string(k)};
      if (f == 5) {
        names.push_back("F2:end(r{c" + std::to_string(k % 3) + "})");
      }
      named.push_back({it.Current(), names});
    }
  }
  const Shape shape(compound, named);
  CHECK(shape.face_count() == 120);
  const auto matching = [&](const std::string& a, const std::string& b) {
    std::vector<int> found;
    for (int i = 0; i < shape.edge_count(); ++i) {
      const auto faces = shape.edge_faces(i);
      if (faces[0] < 0 || faces[1] < 0) {
        continue;
      }
      const NameList& first = shape.face_names(faces[0]);
      const NameList& second = shape.face_names(faces[1]);
      if ((face_matches(first, a) && face_matches(second, b)) || (face_matches(first, b) && face_matches(second, a))) {
        found.push_back(i);
      }
    }
    return found;
  };
  int named_edges = 0;
  for (int i = 0; i < shape.edge_count(); ++i) {
    if (!shape.edge_name(i).empty()) {
      ++named_edges;
      CHECK(shape.find_edges(shape.edge_name(i)) == std::vector<int>{i});
    }
  }
  CHECK(named_edges == 240);
  // Edges between faces of every box (each box's "#k" faces), and between
  // a face some boxes share a name of and another.
  for (const auto& [a, b] : std::vector<std::pair<std::string, std::string>>{
           {"F1:side(c0)", "F1:side(c2)"}, {"F2:end(r{c1})", "F1:side(c0)"}, {"F1:side(c0)#3", "F1:side(c2)#3"},
           {"F1:side(c0)", "F1:side(c1)"}}) {
    const std::vector<int> found = shape.find_edges(make_edge_name(a, b, -1));
    std::vector<int> sorted = found;
    std::sort(sorted.begin(), sorted.end());
    CHECK(sorted == matching(a, b));
    // In the geometric order: along x, as the boxes lie.
    CHECK(std::is_sorted(found.begin(), found.end()));
  }
  CHECK(shape.find_edges(make_edge_name("F1:side(c0)", "F1:side(c2)", -1)).size() == 20);
  CHECK(shape.find_edges(make_edge_name("F1:side(c0)", "F1:side(c2)", 4)).size() == 1);
  CHECK(shape.find_edges(make_edge_name("F1:side(c0)", "F1:side(c1)", -1)).empty());
}

// An edge of a hole in a large face (mitcad#889): the face with many edges
// is named on one side, a hole's face or two on the other. Through the
// faces with fewer edges, either way round, the same edges as checking
// every edge.
void test_large_shapes_find_hole_edges_of_a_large_face() {
  BRep_Builder builder;
  TopoDS_Compound holes;
  builder.MakeCompound(holes);
  for (int i = 0; i < 4; ++i) {
    for (int j = 0; j < 4; ++j) {
      builder.Add(holes, BRepPrimAPI_MakeBox(gp_Pnt(10.0 + 20.0 * i, 10.0 + 20.0 * j, -1.0), 5.0, 5.0, 7.0).Shape());
    }
  }
  const TopoDS_Shape plate =
      BRepAlgoAPI_Cut(BRepPrimAPI_MakeBox(gp_Pnt(0.0, 0.0, 0.0), 100.0, 100.0, 5.0).Shape(), holes).Shape();
  // The top face is F2:start; every other face is named on its own, and
  // the first two holes' faces towards -y also share a name.
  std::vector<Shape::NamedFace> named;
  int k = 0;
  for (TopExp_Explorer it(plate, TopAbs_FACE); it.More(); it.Next(), ++k) {
    Bnd_Box box;
    BRepBndLib::Add(it.Current(), box);
    double x0 = 0.0, y0 = 0.0, z0 = 0.0, x1 = 0.0, y1 = 0.0, z1 = 0.0;
    box.Get(x0, y0, z0, x1, y1, z1);
    NameList names;
    if (z0 > 4.9 && x1 - x0 > 50.0) {
      names.push_back("F2:start(r{c4})");
    } else {
      names.push_back("F9:face(c" + std::to_string(k) + ")");
      const bool towards_y = y1 - y0 < 1e-3 && x1 - x0 < 6.0;
      if (towards_y && y0 < 12.0 && x0 < 35.0) {
        names.push_back("F5:pair");
      }
    }
    named.push_back({it.Current(), names});
  }
  const Shape shape(plate, named);
  CHECK(shape.face_count() == 6 + 16 * 4);
  const auto matching = [&](const std::string& a, const std::string& b) {
    std::vector<int> found;
    for (int i = 0; i < shape.edge_count(); ++i) {
      const auto faces = shape.edge_faces(i);
      if (faces[0] < 0 || faces[1] < 0) {
        continue;
      }
      const NameList& first = shape.face_names(faces[0]);
      const NameList& second = shape.face_names(faces[1]);
      if ((face_matches(first, a) && face_matches(second, b)) || (face_matches(first, b) && face_matches(second, a))) {
        found.push_back(i);
      }
    }
    return found;
  };
  int hole_edges = 0;
  for (int f = 0; f < shape.face_count(); ++f) {
    // The face's own name (a pair face also has the shared one).
    const NameList& names = shape.face_names(f);
    const auto own = std::find_if(names.begin(), names.end(),
                                  [](const std::string& name) { return name.rfind("F9:", 0) == 0; });
    if (own == names.end()) {
      continue;
    }
    const std::string other = *own;
    for (const auto& [a, b] : std::vector<std::pair<std::string, std::string>>{{"F2:start(r{c4})", other},
                                                                              {other, "F2:start(r{c4})"}}) {
      std::vector<int> found = shape.find_edges(make_edge_name(a, b, -1));
      std::sort(found.begin(), found.end());
      CHECK(found == matching(a, b));
      if (found.size() == 1) {
        ++hole_edges;
      }
    }
  }
  // Each of the 64 hole faces and the plate's four sides meet the top face
  // in one edge, found from either name.
  CHECK(hole_edges == 2 * (64 + 4));
  std::vector<int> pair = shape.find_edges(make_edge_name("F2:start(r{c4})", "F5:pair", -1));
  CHECK(pair.size() == 2);
  std::sort(pair.begin(), pair.end());
  CHECK(pair == matching("F2:start(r{c4})", "F5:pair"));
}

// A flat square of 1 mm in the plane z = `z`, its centre at (x, 0.5, z).
TopoDS_Face square_at(double x, double z) {
  BRepBuilderAPI_MakePolygon outline(gp_Pnt(x - 0.5, 0.0, z), gp_Pnt(x + 0.5, 0.0, z), gp_Pnt(x + 0.5, 1.0, z),
                                     gp_Pnt(x - 0.5, 1.0, z), true);
  return BRepBuilderAPI_MakeFace(outline.Wire(), true).Face();
}

// The faces `faces`, in a compound in their order, all named `name` by a
// FaceNamer, numbered by its finish().
ShapePtr named_pieces(const std::vector<TopoDS_Face>& faces, const std::string& name) {
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (const TopoDS_Face& face : faces) {
    builder.Add(compound, face);
  }
  detail::FaceNamer namer(compound);
  for (const TopoDS_Face& face : faces) {
    namer.add(face, name);
  }
  namer.finish();
  return namer.shape();
}

// The names of a face of `shape` (the face `face` of the compound it was
// made of).
NameList names_at(const Shape& shape, const TopoDS_Face& face) {
  const int index = shape.face_index(face);
  return index < 0 ? NameList{} : shape.face_names(index);
}

// Two pieces of one name lying symmetrically (mitcad#895): alike in X and
// Y, apart in Z. Their centres in X differ by 2e-8 mm, either way round,
// across a boundary of shape_key's rounding to 1e-6 mm, as noise elsewhere
// in a design moves them. shape_key orders them by that noise (so the
// numbering before mitcad#895 swapped #0 and #1); the pieces keep their
// numbers, the lower one #0, either way and in either order of the faces.
void test_symmetric_pieces_keep_their_numbers_under_noise() {
  const std::string name = "F2:start(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})";
  // 0.5000005 mm lies on a boundary of the rounding (500000.5 units).
  const double middle = 0.5000005;
  const double noise = 1.0e-8;
  for (const double sign : {1.0, -1.0}) {
    const TopoDS_Face lower = square_at(middle + sign * noise, 0.0);
    const TopoDS_Face upper = square_at(middle - sign * noise, 5.0);
    // The rounded centres differ in X by the noise's sign: shape_key alone
    // would put the upper piece first for one sign.
    const detail::ShapeKey lower_key = detail::shape_key(lower);
    const detail::ShapeKey upper_key = detail::shape_key(upper);
    CHECK(lower_key[0] != upper_key[0]);
    CHECK((lower_key < upper_key) == (sign < 0.0));
    for (const bool lower_first : {true, false}) {
      const std::vector<TopoDS_Face> faces =
          lower_first ? std::vector<TopoDS_Face>{lower, upper} : std::vector<TopoDS_Face>{upper, lower};
      const ShapePtr shape = named_pieces(faces, name);
      CHECK(names_at(*shape, lower) == NameList{name + "#0"});
      CHECK(names_at(*shape, upper) == NameList{name + "#1"});
      // A reference to all pieces lists piece #0 first, then #1.
      const std::vector<int> found = shape->find_faces(name);
      CHECK(found.size() == 2);
      if (found.size() == 2) {
        CHECK(found[0] == shape->face_index(lower));
        CHECK(found[1] == shape->face_index(upper));
      }
      CHECK(shape->find_faces(name + "#1") == std::vector<int>{shape->face_index(upper)});
    }
  }
}

// Pieces whose places differ by more than kPieceTie keep shape_key's order
// (the numbers before mitcad#895): the centres in X decide although the
// pieces lie the other way round in Z, also for centres only 2e-4 mm apart.
void test_distinct_pieces_keep_the_earlier_order() {
  std::vector<TopoDS_Shape> pieces;
  for (const double x : {4.0, 0.0, 2.0, 2.0002, 6.0}) {
    for (const double z : {3.0, 0.0}) {
      pieces.push_back(square_at(x, x == 2.0002 ? -z : z));
    }
  }
  std::vector<std::pair<detail::ShapeKey, std::size_t>> keyed;
  for (std::size_t i = 0; i < pieces.size(); ++i) {
    keyed.emplace_back(detail::shape_key(pieces[i]), i);
  }
  std::sort(keyed.begin(), keyed.end());
  std::vector<std::size_t> earlier;
  for (const auto& entry : keyed) {
    earlier.push_back(entry.second);
  }
  CHECK(detail::piece_order(pieces) == earlier);
}

// Two unit boxes lying symmetrically (mitcad#896): alike in X and Y, the
// lower at z 0 to 1 with its corner in X at `lower_x`, the upper at z 5
// to 6 with its corner at `upper_x`, in a compound, the lower first when
// `lower_first`. Each box's top is named F2:top, its face towards -Y
// F2:front and its face towards -X F2:left, so E{F2:front|F2:top} is an
// edge and V{F2:front|F2:left|F2:top} a corner of each box; every other
// face has a name of its own.
ShapePtr symmetric_boxes(double lower_x, double upper_x, bool lower_first) {
  const TopoDS_Shape lower = BRepPrimAPI_MakeBox(gp_Pnt(lower_x, 0.0, 0.0), 1.0, 1.0, 1.0).Shape();
  const TopoDS_Shape upper = BRepPrimAPI_MakeBox(gp_Pnt(upper_x, 0.0, 5.0), 1.0, 1.0, 1.0).Shape();
  std::vector<TopoDS_Shape> boxes;
  boxes.push_back(lower_first ? lower : upper);
  boxes.push_back(lower_first ? upper : lower);
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  std::vector<Shape::NamedFace> named;
  int k = 0;
  for (const TopoDS_Shape& box : boxes) {
    builder.Add(compound, box);
    for (TopExp_Explorer it(box, TopAbs_FACE); it.More(); it.Next(), ++k) {
      Bnd_Box bounds;
      BRepBndLib::Add(it.Current(), bounds);
      double x0 = 0.0, y0 = 0.0, z0 = 0.0, x1 = 0.0, y1 = 0.0, z1 = 0.0;
      bounds.Get(x0, y0, z0, x1, y1, z1);
      std::string own = "F9:face(c" + std::to_string(k) + ")";
      if (z1 - z0 < 1e-3 && (std::abs(z0 - 1.0) < 1e-3 || std::abs(z0 - 6.0) < 1e-3)) {
        own = "F2:top";
      } else if (y1 - y0 < 1e-3 && std::abs(y0) < 1e-3) {
        own = "F2:front";
      } else if (x1 - x0 < 1e-3 && x0 < 0.5) {
        own = "F2:left";
      }
      named.push_back({it.Current(), NameList{own}});
    }
  }
  return std::make_shared<Shape>(compound, named);
}

// The edge of `shape` along X at y 0 and height `z` (a box's edge between
// F2:front and F2:top), or -1.
int edge_along_x(const Shape& shape, double z) {
  for (int i = 0; i < shape.edge_count(); ++i) {
    Bnd_Box bounds;
    BRepBndLib::Add(shape.edge(i), bounds);
    double x0 = 0.0, y0 = 0.0, z0 = 0.0, x1 = 0.0, y1 = 0.0, z1 = 0.0;
    bounds.Get(x0, y0, z0, x1, y1, z1);
    if (x1 - x0 > 0.5 && y1 < 1e-3 && std::abs(z0 - z) < 1e-3 && std::abs(z1 - z) < 1e-3) {
      return i;
    }
  }
  return -1;
}

// The vertex of `shape` near x 0, at y 0 and height `z` (a box's corner
// between F2:front, F2:left and F2:top), or -1.
int corner_at(const Shape& shape, double z) {
  for (int i = 0; i < shape.vertex_count(); ++i) {
    const gp_Pnt point = BRep_Tool::Pnt(shape.vertex(i));
    if (std::abs(point.X()) < 1e-3 && std::abs(point.Y()) < 1e-3 && std::abs(point.Z() - z) < 1e-3) {
      return i;
    }
  }
  return -1;
}

// Two edges between the same two faces, and two vertices between the same
// three, lying symmetrically (mitcad#896): alike in X and Y, apart in Z.
// The boxes' corners in X differ by 2e-8 mm, either way round, across a
// boundary of the rounding to 1e-6 mm, and so do the edges' middles. The
// rounded keys order them by that noise (so the numbering before
// mitcad#896 swapped #0 and #1); the edges and vertices keep their
// numbers, the lower one #0, either way and in either order of the boxes.
void test_symmetric_edges_and_vertices_keep_their_numbers_under_noise() {
  const std::string edge_reference = make_edge_name("F2:front", "F2:top");
  const NameList corner_faces{"F2:front", "F2:left", "F2:top"};
  const std::string vertex_reference = make_vertex_name(corner_faces);
  // 5e-7 mm lies on a boundary of the rounding (0.5 units), and so does
  // 0.5000005 mm, the middle of an edge 1 mm long starting there.
  const double boundary = 5.0e-7;
  const double noise = 1.0e-8;
  for (const double sign : {1.0, -1.0}) {
    for (const bool lower_first : {true, false}) {
      const ShapePtr shape = symmetric_boxes(boundary + sign * noise, boundary - sign * noise, lower_first);
      const int lower_edge = edge_along_x(*shape, 1.0);
      const int upper_edge = edge_along_x(*shape, 6.0);
      const int lower_corner = corner_at(*shape, 1.0);
      const int upper_corner = corner_at(*shape, 6.0);
      CHECK(lower_edge >= 0 && upper_edge >= 0 && lower_corner >= 0 && upper_corner >= 0);
      if (lower_edge < 0 || upper_edge < 0 || lower_corner < 0 || upper_corner < 0) {
        continue;
      }
      // The rounded keys differ in X by the noise's sign: alone they would
      // put the upper edge and vertex first for one sign.
      const detail::ShapeKey lower_edge_key = detail::edge_key(shape->edge(lower_edge));
      const detail::ShapeKey upper_edge_key = detail::edge_key(shape->edge(upper_edge));
      CHECK(lower_edge_key[0] != upper_edge_key[0]);
      CHECK((lower_edge_key < upper_edge_key) == (sign < 0.0));
      const detail::ShapeKey lower_corner_key = detail::shape_key(shape->vertex(lower_corner));
      const detail::ShapeKey upper_corner_key = detail::shape_key(shape->vertex(upper_corner));
      CHECK(lower_corner_key[0] != upper_corner_key[0]);
      CHECK((lower_corner_key < upper_corner_key) == (sign < 0.0));

      CHECK(shape->edge_name(lower_edge) == make_edge_name("F2:front", "F2:top", 0));
      CHECK(shape->edge_name(upper_edge) == make_edge_name("F2:front", "F2:top", 1));
      // A reference to all of them lists #0 first, then #1.
      CHECK((shape->find_edges(edge_reference) == std::vector<int>{lower_edge, upper_edge}));
      CHECK(shape->find_edges(make_edge_name("F2:front", "F2:top", 1)) == std::vector<int>{upper_edge});

      CHECK(shape->vertex_name(lower_corner) == make_vertex_name(corner_faces, 0));
      CHECK(shape->vertex_name(upper_corner) == make_vertex_name(corner_faces, 1));
      CHECK((shape->find_vertices(vertex_reference) == std::vector<int>{lower_corner, upper_corner}));
      CHECK(shape->find_vertices(make_vertex_name(corner_faces, 1)) == std::vector<int>{upper_corner});
    }
  }
}

// Edges and vertices whose places differ by more than kPieceTie keep the
// order of the rounded keys (the numbers before mitcad#896): the middles
// (the points) in X decide although they lie the other way round in Z,
// also for ones only 2e-4 mm apart.
void test_distinct_edges_and_vertices_keep_the_earlier_order() {
  std::vector<TopoDS_Shape> lines;
  std::vector<TopoDS_Shape> points;
  for (const double x : {4.0, 0.0, 2.0, 2.0002, 6.0}) {
    for (const double z : {3.0, 0.0}) {
      const double at = x == 2.0002 ? -z : z;
      lines.push_back(BRepBuilderAPI_MakeEdge(gp_Pnt(x - 0.5, 0.0, at), gp_Pnt(x + 0.5, 0.0, at)).Edge());
      points.push_back(BRepBuilderAPI_MakeVertex(gp_Pnt(x, 0.0, at)).Vertex());
    }
  }
  std::vector<std::pair<detail::ShapeKey, std::size_t>> keyed_lines;
  std::vector<std::pair<detail::ShapeKey, std::size_t>> keyed_points;
  for (std::size_t i = 0; i < lines.size(); ++i) {
    keyed_lines.emplace_back(detail::edge_key(lines[i]), i);
    keyed_points.emplace_back(detail::shape_key(points[i]), i);
  }
  std::sort(keyed_lines.begin(), keyed_lines.end());
  std::sort(keyed_points.begin(), keyed_points.end());
  std::vector<std::size_t> earlier_lines;
  std::vector<std::size_t> earlier_points;
  for (std::size_t k = 0; k < keyed_lines.size(); ++k) {
    earlier_lines.push_back(keyed_lines[k].second);
    earlier_points.push_back(keyed_points[k].second);
  }
  CHECK(detail::edge_order(lines) == earlier_lines);
  CHECK(detail::vertex_order(points) == earlier_points);
}

} // namespace

void naming_tests() {
  test_face_names_and_matching();
  test_edge_and_vertex_names_are_canonical();
  test_composite_names_parse();
  test_large_shapes_find_faces_by_an_index();
  test_large_shapes_find_edges_by_their_faces();
  test_large_shapes_find_hole_edges_of_a_large_face();
  test_symmetric_pieces_keep_their_numbers_under_noise();
  test_distinct_pieces_keep_the_earlier_order();
  test_symmetric_edges_and_vertices_keep_their_numbers_under_noise();
  test_distinct_edges_and_vertices_keep_the_earlier_order();
}

} // namespace test
