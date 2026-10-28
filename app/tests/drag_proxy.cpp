// SPDX-License-Identifier: MIT
// The provisional drag presentation of Extrude (mitcad#636): the owned
// copy of a meshed tool (proxyMeshOf), its changes for each dragged value
// (applyDragProxy: one side, flipped, negative, symmetric, two sides, the
// start offset, a placement that turns the arrow, a tool that does not
// fit) and Extrude's rules for when and how (extrudeDragProxy). Move/
// Copy's provisional motion (mitcad#838): the placements a drag gives the
// bodies it moves and the components it places (draggedPlacement), and
// the motion of its inputs (moveMotion). No window and no model: the tools
// are OCCT boxes meshed here.
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <limits>
#include <memory>
#include <vector>

#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepTools.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Ax1.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

#include "commands/ExtrudeDragProxy.hpp"
#include "commands/MoveDragMotion.hpp"
#include "framework/DragProxy.hpp"

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "drag_proxy.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using mitcad::CommandState;
using mitcad::DragProxyRule;
using mitcad::ProxyMesh;

bool near(double a, double b, double tolerance = 1e-6) { return std::abs(a - b) <= tolerance; }

// A box from `low` to `high`, meshed as a preview's tool would be.
TopoDS_Shape box(const gp_Pnt& low, const gp_Pnt& high) {
  TopoDS_Shape shape = BRepPrimAPI_MakeBox(low, high).Shape();
  BRepMesh_IncrementalMesh(shape, 0.01, false, 0.5, false);
  return shape;
}

struct Range {
  double low = std::numeric_limits<double>::infinity();
  double high = -std::numeric_limits<double>::infinity();
};

// The mesh's extent along a direction.
Range along(const ProxyMesh& mesh, const gp_Dir& direction) {
  Range range;
  for (const gp_Pnt& node : mesh.nodes) {
    const double h = node.XYZ().Dot(direction.XYZ());
    range.low = std::min(range.low, h);
    range.high = std::max(range.high, h);
  }
  return range;
}

// The volume the triangles enclose, positive when they face outwards.
double volume(const ProxyMesh& mesh) {
  double sum = 0.0;
  for (const std::array<int, 3>& t : mesh.triangles) {
    const gp_XYZ a = mesh.nodes[static_cast<std::size_t>(t[0])].XYZ();
    const gp_XYZ b = mesh.nodes[static_cast<std::size_t>(t[1])].XYZ();
    const gp_XYZ c = mesh.nodes[static_cast<std::size_t>(t[2])].XYZ();
    sum += a.Dot(b.Crossed(c));
  }
  return sum / 6.0;
}

DragProxyRule rule(DragProxyRule::Kind kind, double from, double to, double length, double reach) {
  DragProxyRule r;
  r.kind = kind;
  r.from = from;
  r.to = to;
  r.length = length;
  r.reach = reach;
  return r;
}

const gp_Dir kUp(0, 0, 1);
const gp_Dir kDown(0, 0, -1);

void testMeshCopy() {
  // The ticket's boss: 20 x 10 mm at (50, 45), 5 mm up from z = 20.
  TopoDS_Shape tool = box(gp_Pnt(50, 45, 20), gp_Pnt(70, 55, 25));
  const std::shared_ptr<const ProxyMesh> mesh = mitcad::proxyMeshOf(tool);
  CHECK(mesh != nullptr);
  if (!mesh) {
    return;
  }
  CHECK(mesh->triangles.size() == 12);
  CHECK(mesh->outlines.size() == 12); // each edge once
  CHECK(near(volume(*mesh), 20.0 * 10.0 * 5.0, 1e-6));
  const Range z = along(*mesh, kUp);
  CHECK(near(z.low, 20.0) && near(z.high, 25.0));
  // Owned: the tool meshed again (or its meshes dropped) leaves the copy.
  const std::size_t nodes = mesh->nodes.size();
  BRepTools::Clean(tool);
  CHECK(mesh->nodes.size() == nodes && near(along(*mesh, kUp).high, 25.0));
  // No triangulation, no mesh.
  CHECK(mitcad::proxyMeshOf(tool) == nullptr);
  CHECK(mitcad::proxyMeshOf(TopoDS_Shape()) == nullptr);
}

void testStretches() {
  const auto mesh = mitcad::proxyMeshOf(box(gp_Pnt(50, 45, 20), gp_Pnt(70, 55, 25)));
  if (!mesh) {
    CHECK(false);
    return;
  }
  using Kind = DragProxyRule::Kind;
  // One side, up: from the start at z = 20 to 12 mm.
  auto proxy = mitcad::applyDragProxy(*mesh, rule(Kind::Stretch, 5, 12, 5, 5), {kUp});
  CHECK(proxy.has_value());
  if (proxy) {
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 20.0) && near(z.high, 32.0));
    CHECK(near(proxy->fixed, 20.0));
    const Range x = along(*proxy->mesh, gp_Dir(1, 0, 0));
    CHECK(near(x.low, 50.0) && near(x.high, 70.0));
    CHECK(near(volume(*proxy->mesh), 20.0 * 10.0 * 12.0, 1e-6));
    CHECK(proxy->mesh->outlines == mesh->outlines);
    // The basis is not changed.
    CHECK(near(along(*mesh, kUp).high, 25.0));
  }
  // Through the start to the other side (a negative distance): mirrored,
  // its triangles still facing out.
  proxy = mitcad::applyDragProxy(*mesh, rule(Kind::Stretch, 5, -3, 5, 5), {kUp});
  CHECK(proxy.has_value());
  if (proxy) {
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 17.0) && near(z.high, 20.0));
    CHECK(near(volume(*proxy->mesh), 20.0 * 10.0 * 3.0, 1e-6));
  }
  // A flat tool is no proxy.
  CHECK(!mitcad::applyDragProxy(*mesh, rule(Kind::Stretch, 5, 0, 5, 5), {kUp}));
  // Flipped (the arrow points down) from z = 20 down 5 mm, to 8 mm.
  const auto flipped = mitcad::proxyMeshOf(box(gp_Pnt(50, 45, 15), gp_Pnt(70, 55, 20)));
  proxy = mitcad::applyDragProxy(*flipped, rule(Kind::Stretch, 5, 8, 5, 5), {kDown});
  CHECK(proxy.has_value());
  if (proxy) {
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 12.0) && near(z.high, 20.0));
  }
  // A negative distance in the exact preview: the tool lies behind the
  // start (z 15 to 20 along an arrow up), the start at its far end.
  proxy = mitcad::applyDragProxy(*flipped, rule(Kind::Stretch, -5, -2, 5, 0), {kUp});
  CHECK(proxy.has_value());
  if (proxy) {
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 18.0) && near(z.high, 20.0));
  }
  // Symmetric, half length 10 about z = 20, to 4.
  const auto symmetric = mitcad::proxyMeshOf(box(gp_Pnt(50, 45, 10), gp_Pnt(70, 55, 30)));
  proxy = mitcad::applyDragProxy(*symmetric, rule(Kind::Stretch, 10, 4, 20, 10), {kUp});
  CHECK(proxy.has_value());
  if (proxy) {
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 16.0) && near(z.high, 24.0));
  }
  // Two sides from z = 20: 5 up, 3 down. Side 1 to 9, side 2 to 6.
  const auto two = mitcad::proxyMeshOf(box(gp_Pnt(50, 45, 17), gp_Pnt(70, 55, 25)));
  proxy = mitcad::applyDragProxy(*two, rule(Kind::StretchSide, 5, 9, 8, 5), {kUp});
  CHECK(proxy.has_value());
  if (proxy) {
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 17.0) && near(z.high, 29.0));
  }
  proxy = mitcad::applyDragProxy(*two, rule(Kind::StretchSide, 3, 6, 8, 3), {kDown});
  CHECK(proxy.has_value());
  if (proxy) {
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 14.0) && near(z.high, 25.0));
  }
  // A side does not turn through the profile plane.
  CHECK(!mitcad::applyDragProxy(*two, rule(Kind::StretchSide, 5, -1, 8, 5), {kUp}));
  // The start offset moves the whole tool: from 20 to 26.
  proxy = mitcad::applyDragProxy(*mesh, rule(Kind::Translate, 20, 26, 5, 0), {kUp});
  CHECK(proxy.has_value());
  if (proxy) {
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 26.0) && near(z.high, 31.0));
    CHECK(near(volume(*proxy->mesh), 1000.0, 1e-6));
  }
}

void testAxes() {
  // A tool of a component turned 90 degrees about X and moved: the arrow
  // drawn in the design points along -Y there, the tool's own axis is Z.
  const auto mesh = mitcad::proxyMeshOf(box(gp_Pnt(0, 0, 0), gp_Pnt(20, 10, 5)));
  gp_Trsf placement;
  constexpr double kQuarterTurn = 1.5707963267948966;
  placement.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(1, 0, 0)), kQuarterTurn);
  gp_Trsf shift;
  shift.SetTranslation(gp_Vec(60, 0, 0));
  placement = shift * placement;
  const gp_Dir designArrow = kUp.Transformed(placement);
  CHECK(near(designArrow.Y(), -1.0));
  using Kind = DragProxyRule::Kind;
  // The arrow as it is fits no axis of the tool of 5 mm (along -Y it is
  // 10 mm long); as the placement gives it, Z.
  auto proxy = mitcad::applyDragProxy(*mesh, rule(Kind::Stretch, 5, 7, 5, 5), {designArrow});
  CHECK(!proxy.has_value());
  proxy = mitcad::applyDragProxy(*mesh, rule(Kind::Stretch, 5, 7, 5, 5),
                                 {designArrow.Transformed(placement.Inverted()), designArrow});
  CHECK(proxy.has_value());
  if (proxy) {
    CHECK(proxy->axis.IsEqual(kUp, 1e-9));
    const Range z = along(*proxy->mesh, kUp);
    CHECK(near(z.low, 0.0) && near(z.high, 7.0));
  }
  // A tool of another length than the values give (a taper's, a cut's
  // through all) is never stretched.
  CHECK(!mitcad::applyDragProxy(*mesh, rule(Kind::Stretch, 6, 9, 6, 6), {kUp}));
  // Without a length to check, the first axis.
  DragProxyRule any = rule(Kind::Stretch, 5, 10, std::numeric_limits<double>::quiet_NaN(), 5);
  CHECK(mitcad::applyDragProxy(*mesh, any, {kUp}).has_value());
}

CommandState extrudeState() {
  CommandState state;
  state.setChoice(QStringLiteral("type"), QStringLiteral("solid"));
  state.setChoice(QStringLiteral("start"), QStringLiteral("profile_plane"));
  state.setChoice(QStringLiteral("direction"), QStringLiteral("one_side"));
  state.setChoice(QStringLiteral("extent"), QStringLiteral("distance"));
  state.setChoice(QStringLiteral("extent2"), QStringLiteral("distance"));
  state.setChoice(QStringLiteral("measurement"), QStringLiteral("half"));
  state.setChoice(QStringLiteral("operation"), QStringLiteral("join"));
  state.setValue(QStringLiteral("distance"), 5.0);
  state.setValue(QStringLiteral("distance2"), 3.0);
  state.setValue(QStringLiteral("start_offset"), 0.0);
  state.setValue(QStringLiteral("taper"), 0.0);
  state.setValue(QStringLiteral("taper2"), 0.0);
  state.setValue(QStringLiteral("thickness"), 1.0);
  return state;
}

void testExtrudeRules() {
  using mitcad::cmd::extrudeDragProxy;
  using Kind = DragProxyRule::Kind;
  const QString distance = QStringLiteral("distance");
  CommandState shown = extrudeState();
  CommandState now = shown;
  // One side: from the start.
  auto r = extrudeDragProxy(shown, now, distance, 12.0);
  CHECK(r && r->kind == Kind::Stretch && near(r->from, 5) && near(r->to, 12) && near(r->length, 5) &&
        near(r->reach, 5));
  // The automatic operation's change is no change of the tool.
  now.setChoice(QStringLiteral("operation"), QStringLiteral("cut"));
  CHECK(extrudeDragProxy(shown, now, distance, 12.0).has_value());
  // Flipped: the same along its arrow.
  shown.setChecked(QStringLiteral("flip"), true);
  now = shown;
  r = extrudeDragProxy(shown, now, distance, 2.0);
  CHECK(r && r->kind == Kind::Stretch && near(r->reach, 5));
  // A flip since the exact preview: none.
  now.setChecked(QStringLiteral("flip"), false);
  CHECK(!extrudeDragProxy(shown, now, distance, 2.0));
  shown = extrudeState();
  // Another value changed since: none.
  now = shown;
  now.setValue(QStringLiteral("start_offset"), 4.0);
  CHECK(!extrudeDragProxy(shown, now, distance, 12.0));
  // The dragged one may differ (it is the one dragged).
  now = shown;
  now.setValue(distance, 9.0);
  CHECK(extrudeDragProxy(shown, now, distance, 12.0).has_value());
  // Negative in the exact preview: the start at the tool's far end.
  shown.setValue(distance, -4.0);
  now = shown;
  r = extrudeDragProxy(shown, now, distance, -6.0);
  CHECK(r && near(r->length, 4) && near(r->reach, 0));
  shown = extrudeState();
  // Tapered, to an object, through all, wrapped, from an object: none.
  CommandState tapered = shown;
  tapered.setValue(QStringLiteral("taper"), 0.1);
  CHECK(!extrudeDragProxy(tapered, tapered, distance, 7.0));
  CommandState object = shown;
  object.setChoice(QStringLiteral("extent"), QStringLiteral("to_object"));
  CHECK(!extrudeDragProxy(object, object, distance, 7.0));
  CommandState all = shown;
  all.setChoice(QStringLiteral("extent"), QStringLiteral("through_all"));
  CHECK(!extrudeDragProxy(all, all, distance, 7.0));
  CommandState wrapped = shown;
  mitcad::SelectionItem face;
  face.kind = mitcad::SelectKind::Face;
  face.owner = QStringLiteral("F2.b0");
  wrapped.setItems(QStringLiteral("wrap"), {face});
  CHECK(!extrudeDragProxy(wrapped, wrapped, distance, 7.0));
  CommandState fromObject = shown;
  fromObject.setChoice(QStringLiteral("start"), QStringLiteral("object"));
  CHECK(!extrudeDragProxy(fromObject, fromObject, distance, 7.0));
  // Symmetric: half and whole length.
  CommandState symmetric = shown;
  symmetric.setChoice(QStringLiteral("direction"), QStringLiteral("symmetric"));
  r = extrudeDragProxy(symmetric, symmetric, distance, 8.0);
  CHECK(r && r->kind == Kind::Stretch && near(r->length, 10) && near(r->reach, 5));
  symmetric.setChoice(QStringLiteral("measurement"), QStringLiteral("whole"));
  r = extrudeDragProxy(symmetric, symmetric, distance, 8.0);
  CHECK(r && near(r->length, 5) && near(r->reach, 2.5));
  CHECK(!extrudeDragProxy(symmetric, symmetric, distance, -1.0));
  // Two sides: each side on its own.
  CommandState two = shown;
  two.setChoice(QStringLiteral("direction"), QStringLiteral("two_sides"));
  r = extrudeDragProxy(two, two, distance, 9.0);
  CHECK(r && r->kind == Kind::StretchSide && near(r->length, 8) && near(r->reach, 5));
  r = extrudeDragProxy(two, two, QStringLiteral("distance2"), 6.0);
  CHECK(r && r->kind == Kind::StretchSide && near(r->length, 8) && near(r->reach, 3) && near(r->from, 3));
  // Across the profile plane (a negative side, mitcad#765): the exact
  // preview only.
  CHECK(!extrudeDragProxy(two, two, QStringLiteral("distance2"), -2.0));
  CommandState twoObject = two;
  twoObject.setChoice(QStringLiteral("extent2"), QStringLiteral("to_object"));
  CHECK(!extrudeDragProxy(twoObject, twoObject, distance, 9.0));
  // The start offset moves the tool.
  CommandState offset = shown;
  offset.setChoice(QStringLiteral("start"), QStringLiteral("offset"));
  offset.setValue(QStringLiteral("start_offset"), 20.0);
  r = extrudeDragProxy(offset, offset, QStringLiteral("start_offset"), 26.0);
  CHECK(r && r->kind == Kind::Translate && near(r->from, 20) && near(r->to, 26) && near(r->length, 5));
  CHECK(!extrudeDragProxy(shown, shown, QStringLiteral("start_offset"), 3.0)); // from the profile plane
  // A taper is dragged by a ring: no proxy for it.
  CHECK(!extrudeDragProxy(shown, shown, QStringLiteral("taper"), 0.2));
}

gp_Trsf moved(double x, double y, double z) {
  gp_Trsf trsf;
  trsf.SetTranslation(gp_Vec(x, y, z));
  return trsf;
}

// A quarter turn about Z through `through`.
gp_Trsf quarter(const gp_Pnt& through) {
  gp_Trsf trsf;
  trsf.SetRotation(gp_Ax1(through, gp_Dir(0, 0, 1)), std::atan(1.0) * 2.0);
  return trsf;
}

bool at(const gp_Pnt& point, double x, double y, double z) { return point.Distance(gp_Pnt(x, y, z)) < 1e-9; }

void testMotions() {
  using mitcad::MotionSide;
  using mitcad::draggedPlacement;
  // A body the move moved 5 mm along X, dragged to 12: 7 mm further.
  gp_Trsf placed = draggedPlacement(gp_Trsf(), MotionSide::Shape, moved(5, 0, 0), moved(12, 0, 0), gp_Trsf());
  CHECK(at(gp_Pnt(1, 2, 3).Transformed(placed), 8, 2, 3));
  // Not moved: where the exact preview draws it.
  placed = draggedPlacement(moved(0, 50, 0), MotionSide::None, moved(5, 0, 0), moved(12, 0, 0), gp_Trsf());
  CHECK(at(gp_Pnt(0, 0, 0).Transformed(placed), 0, 50, 0));
  // In an occurrence 50 mm along Y: the body turns in its component's
  // coordinates, a quarter about (10, 0, 0) there, and is drawn where the
  // occurrence puts it.
  placed = draggedPlacement(moved(0, 50, 0), MotionSide::Shape, gp_Trsf(), quarter(gp_Pnt(10, 0, 0)), gp_Trsf());
  CHECK(at(gp_Pnt(20, 0, 0).Transformed(placed), 10, 60, 0));
  // An occurrence the move places, in a component 100 mm along X: the
  // turn about that component's (10, 0, 0), the design's (110, 0, 0).
  placed = draggedPlacement(moved(110, 0, 0), MotionSide::Occurrence, gp_Trsf(), quarter(gp_Pnt(10, 0, 0)),
                            moved(100, 0, 0));
  CHECK(at(gp_Pnt(0, 0, 0).Transformed(placed), 110, 0, 0));
  CHECK(at(gp_Pnt(10, 0, 0).Transformed(placed), 110, 10, 0));
  // From a shown motion to another: only the change is applied.
  placed = draggedPlacement(moved(5, 0, 0), MotionSide::Occurrence, moved(5, 0, 0), moved(5, 0, 9), gp_Trsf());
  CHECK(at(gp_Pnt(0, 0, 0).Transformed(placed), 5, 0, 9));
  CHECK(mitcad::sameMotion(moved(1, 2, 3), moved(1, 2, 3 + 1e-12)));
  CHECK(!mitcad::sameMotion(moved(1, 2, 3), moved(1, 2, 3.001)));

  // Free Move about the pivot (10, 0, 0): a quarter about Z, then 5 up.
  CommandState state;
  state.setChoice(QStringLiteral("type"), QStringLiteral("free"));
  for (const char* id : {"tx", "ty", "tz", "rx", "ry", "rz"}) {
    state.setValue(QString::fromLatin1(id), 0.0);
  }
  const gp_Pnt pivot(10, 0, 0);
  auto motion = mitcad::cmd::moveMotion(state, pivot);
  CHECK(motion && mitcad::sameMotion(*motion, gp_Trsf()));
  state.setValue(QStringLiteral("rz"), std::atan(1.0) * 2.0);
  state.setValue(QStringLiteral("tz"), 5.0);
  motion = mitcad::cmd::moveMotion(state, pivot);
  CHECK(motion && at(gp_Pnt(20, 0, 0).Transformed(*motion), 10, 10, 5));
  CHECK(motion && at(pivot.Transformed(*motion), 10, 0, 5));
  // A drag of X from there: the body where the new motion puts it.
  CommandState dragged = state;
  dragged.setValue(QStringLiteral("tx"), 8.0);
  const auto to = mitcad::cmd::moveMotion(dragged, pivot);
  CHECK(to && motion);
  if (to && motion) {
    placed = draggedPlacement(gp_Trsf(), MotionSide::Shape, *motion, *to, gp_Trsf());
    CHECK(at(gp_Pnt(20, 0, 0).Transformed(*motion).Transformed(placed), 18, 10, 5));
  }
  // A value that is not a number: none.
  state.setValue(QStringLiteral("ty"), std::numeric_limits<double>::quiet_NaN());
  CHECK(!mitcad::cmd::moveMotion(state, pivot));
  // Translate: its distances.
  CommandState translate;
  translate.setChoice(QStringLiteral("type"), QStringLiteral("translate"));
  translate.setValue(QStringLiteral("x"), 1.0);
  translate.setValue(QStringLiteral("y"), -2.0);
  translate.setValue(QStringLiteral("z"), 3.0);
  motion = mitcad::cmd::moveMotion(translate, pivot);
  CHECK(motion && at(gp_Pnt(0, 0, 0).Transformed(*motion), 1, -2, 3));
  // The motions of picked geometry: none.
  CommandState rotate = translate;
  rotate.setChoice(QStringLiteral("type"), QStringLiteral("rotate"));
  CHECK(!mitcad::cmd::moveMotion(rotate, pivot));
}

} // namespace

int main() {
  testMeshCopy();
  testStretches();
  testAxes();
  testExtrudeRules();
  testMotions();
  if (failures > 0) {
    std::fprintf(stderr, "%d check(s) failed\n", failures);
    return 1;
  }
  std::printf("drag proxy: all checks passed\n");
  return 0;
}
