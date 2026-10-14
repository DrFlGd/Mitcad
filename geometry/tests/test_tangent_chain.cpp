// SPDX-License-Identifier: MIT
// The tangent chains of a body's edges and faces (mitcad#290), and that a
// fillet, chamfer and shell with a tangent chain take exactly the chain.

#include <algorithm>
#include <string>
#include <vector>

#include "check.hpp"
#include "mitcad/geometry/dressup.hpp"
#include "mitcad/geometry/faceops.hpp"
#include "mitcad/geometry/tangent_chain.hpp"

namespace test {
namespace {

using namespace mitcad::geometry;

// A 60 x 40 x 20 block (F2) with lines c1..c4.
ShapePtr block() { return extrude("F2", {rectangle(1, 0, 0, 60, 40)}, 0, 20); }

// The vertical edge between side i - 1 and side i, and the top edge of side i.
std::string vertical(int i) { return edge(side("F2", 1, (i + 3) % 4), side("F2", 1, i)); }
std::string top(int i) { return edge(end_cap("F2", 1), side("F2", 1, i)); }

// The chain that holds the item, or just the item.
std::vector<int> chain_of(const std::vector<std::vector<int>>& chains, int item) {
  for (const std::vector<int>& chain : chains) {
    if (std::find(chain.begin(), chain.end(), item) != chain.end()) {
      return chain;
    }
  }
  return {item};
}

int edge_index(const Shape& body, const std::string& name) {
  const std::vector<int> found = body.find_edges(name);
  CHECK(found.size() == 1);
  return found.empty() ? -1 : found.front();
}

// The number of faces with a name that starts with the prefix.
int faces_starting(const Shape& shape, const std::string& prefix) {
  int count = 0;
  for (int f = 0; f < shape.face_count(); ++f) {
    const NameList& names = shape.face_names(f);
    if (std::any_of(names.begin(), names.end(),
                    [&](const std::string& name) { return name.rfind(prefix, 0) == 0; })) {
      ++count;
    }
  }
  return count;
}

// A fillet and a chamfer of the one edge with a tangent chain round and
// bevel every edge of its chain and no other.
void check_dressups_take_the_chain(const Shape& body, const std::string& seed) {
  const std::vector<int> chain = chain_of(edge_tangent_chains(body), edge_index(body, seed));
  FilletSet rounding;
  rounding.edges = {seed};
  rounding.radius = 1;
  const ShapePtr rounded = fillet("F9", body, std::vector<FilletSet>{rounding});
  CHECK(rounded->notes().empty());
  CHECK(faces_starting(*rounded, "F9:fillet(") == static_cast<int>(chain.size()));
  ChamferSet bevel;
  bevel.edges = {seed};
  bevel.spec.distance = 1;
  const ShapePtr bevelled = chamfer("F9", body, std::vector<ChamferSet>{bevel});
  CHECK(bevelled->notes().empty());
  CHECK(faces_starting(*bevelled, "F9:chamfer(") == static_cast<int>(chain.size()));
  for (const int e : chain) {
    CHECK(faces_named(*rounded, "F9:fillet(" + body.edge_name(e) + ")") == 1);
    CHECK(faces_named(*bevelled, "F9:chamfer(" + body.edge_name(e) + ")") == 1);
  }
}

void test_sharp_box_has_no_chains() {
  const ShapePtr box = block();
  CHECK(edge_tangent_chains(*box).empty());
  CHECK(face_tangent_chains(*box).empty());
  check_dressups_take_the_chain(*box, top(0));
}

void test_rounded_corner_joins_its_edges() {
  // One vertical edge rounded: the top edges on both sides of it continue
  // tangentially into the rounding's top edge; the box's sharp corners
  // stop the chain.
  const ShapePtr box = block();
  const ShapePtr rounded = fillet("F3", *box, {vertical(1)}, 5);
  const std::vector<std::vector<int>> chains = edge_tangent_chains(*rounded);
  CHECK(chains.size() == 2); // along the top and along the bottom
  const std::vector<int> along = chain_of(chains, edge_index(*rounded, top(0)));
  CHECK(along.size() == 3);
  CHECK(std::count(along.begin(), along.end(), edge_index(*rounded, top(1))) == 1);
  CHECK(chain_of(chains, edge_index(*rounded, top(2))).size() == 1);
  CHECK(chain_of(chains, edge_index(*rounded, vertical(2))).size() == 1);
  check_dressups_take_the_chain(*rounded, top(0));
  check_dressups_take_the_chain(*rounded, top(2));

  // The faces: the rounding and the two sides it joins.
  const std::vector<std::vector<int>> faces = face_tangent_chains(*rounded);
  CHECK(faces.size() == 1);
  const std::vector<int> sides = chain_of(faces, rounded->find_faces(side("F2", 1, 0)).front());
  CHECK(sides.size() == 3);
  CHECK(std::count(sides.begin(), sides.end(), rounded->find_faces("F3:fillet(" + vertical(1) + ")").front()) ==
        1);
  // A shell that removes one of them with a tangent chain removes all three.
  ShellSpec shell;
  shell.faces = {side("F2", 1, 0)};
  shell.inside = 2;
  const ShapePtr hollow = mitcad::geometry::shell("F9", *rounded, shell);
  CHECK(faces_named(*hollow, side("F2", 1, 1)) == 0);
  CHECK(faces_named(*hollow, side("F2", 1, 2)) == 1);
}

void test_rounded_box_top_loop_closes() {
  // Every vertical edge rounded: the top outline is one closed chain of
  // eight edges, and so is the bottom one.
  const ShapePtr box = block();
  const ShapePtr rounded = fillet("F3", *box, {vertical(0), vertical(1), vertical(2), vertical(3)}, 5);
  const std::vector<std::vector<int>> chains = edge_tangent_chains(*rounded);
  CHECK(chains.size() == 2);
  for (int i = 0; i < 4; ++i) {
    CHECK(chain_of(chains, edge_index(*rounded, top(i))).size() == 8);
  }
  CHECK(face_tangent_chains(*rounded).size() == 1);
  CHECK(face_tangent_chains(*rounded).front().size() == 8);
  check_dressups_take_the_chain(*rounded, top(3));
}

} // namespace

void tangent_chain_tests() {
  guarded("test_sharp_box_has_no_chains", test_sharp_box_has_no_chains);
  guarded("test_rounded_corner_joins_its_edges", test_rounded_corner_joins_its_edges);
  guarded("test_rounded_box_top_loop_closes", test_rounded_box_top_loop_closes);
}

} // namespace test
