// SPDX-License-Identifier: MIT
// Sketch selection filters (mitcad#217): the three-state rows, the
// sketch's and Project's defaults, Shift's wider clicks, windows, body
// edges in sight and behind faces, and other sketches' geometry.
#include "framework/SelectionScope.hpp"

#include <cstdio>

#include <QString>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "selection_scope.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using namespace mitcad;

const QString kSketch = QStringLiteral("F4");

SelectionItem item(SelectKind kind, const QString& owner, const QString& name = QString()) {
  SelectionItem result;
  result.kind = kind;
  result.owner = owner;
  result.name = name;
  return result;
}

// The test sketch: c1 ordinary, c2 construction, c3 projected; p1 ordinary,
// p2 construction, p3 projected.
FilterRow sketchRow(const SelectionItem& own) {
  if (own.name.endsWith(QLatin1Char('2'))) {
    return FilterRow::ConstructionGeometry;
  }
  if (own.name.endsWith(QLatin1Char('3'))) {
    return FilterRow::ProjectedGeometry;
  }
  return FilterRow::SketchGeometry;
}

PickCase click(bool shift = false) {
  PickCase how;
  how.expanded = shift;
  return how;
}

PickCase window() {
  PickCase how;
  how.window = true;
  return how;
}

// A pick of a body edge whose hit part is `sight`; counts the looks.
PickCase edgePick(bool isWindow, bool shift, EdgeSight sight, int* looks) {
  PickCase how;
  how.window = isWindow;
  how.expanded = shift && !isWindow;
  how.sight = [sight, looks] {
    if (looks != nullptr) {
      ++*looks;
    }
    return sight;
  };
  return how;
}

bool takes(const SelectionScope& scope, const SelectionItem& picked, const PickCase& how) {
  return scopeAccepts(scope, picked, how, kSketch, sketchRow);
}

void cycleTests() {
  CHECK(nextFilterState(FilterState::Off) == FilterState::Click);
  CHECK(nextFilterState(FilterState::Click) == FilterState::Window);
  CHECK(nextFilterState(FilterState::Window) == FilterState::Off);
  CHECK(filterStateName(FilterState::Off) == QStringLiteral("empty"));
  CHECK(filterStateName(FilterState::Click) == QStringLiteral("square"));
  CHECK(filterStateName(FilterState::Window) == QStringLiteral("cross"));
  for (int i = 0; i < kFilterRowCount; ++i) {
    const auto row = static_cast<FilterRow>(i);
    CHECK(filterRowFromId(filterRowId(row)) == row);
    CHECK(!filterRowLabel(row).isEmpty());
  }
  CHECK(!filterRowFromId(QStringLiteral("nothing")).has_value());

  SelectionScope scope = SelectionScope::sketch();
  CHECK(scope.cycle(FilterRow::Faces) == FilterState::Click);
  CHECK(scope.cycle(FilterRow::Faces) == FilterState::Window);
  CHECK(scope.cycle(FilterRow::Faces) == FilterState::Off);
  CHECK(scope.isDefault());
  scope.cycle(FilterRow::VisibleEdges);
  CHECK(!scope.isDefault());
  scope.reset();
  CHECK(scope.isDefault());
  // A row the scope does not have stays empty.
  SelectionScope project = SelectionScope::project();
  CHECK(!project.has(FilterRow::SketchGeometry));
  project.setState(FilterRow::SketchGeometry, FilterState::Window);
  CHECK(project.state(FilterRow::SketchGeometry) == FilterState::Off);
}

void defaultTests() {
  const SelectionScope sketch = SelectionScope::sketch();
  CHECK(sketch.rows().size() == kFilterRowCount);
  CHECK(sketch.describe() ==
        QStringLiteral("sketch-geometry=cross, construction-geometry=cross, projected-geometry=cross, "
                       "annotations=square, profiles=square, visible-edges=square, occluded-edges=empty, "
                       "faces=empty, vertices=empty, bodies=empty, components=empty, "
                       "reference-geometry=empty"));
  const SelectionScope project = SelectionScope::project();
  CHECK(project.describe() ==
        QStringLiteral("visible-edges=cross, occluded-edges=empty, faces=square, vertices=square"));
  // What the view makes pickable.
  CHECK(sketch.kinds(false) ==
        (SelectKind::SketchCurve | SelectKind::SketchPoint | SelectKind::Profile | SelectKind::Edge));
  CHECK(sketch.kinds(true).testFlag(SelectKind::Face));
  CHECK(sketch.kinds(true).testFlag(SelectKind::Vertex));
  CHECK(sketch.kinds(true).testFlag(SelectKind::Plane));
  CHECK(!sketch.kinds(true).testFlag(SelectKind::Body)); // faces win over bodies
  CHECK(project.kinds(false) == (SelectKind::Edge | SelectKind::Face | SelectKind::Vertex));
  CHECK(project.kinds(true) == project.kinds(false));
}

void sketchWindowTests() {
  const SelectionScope scope = SelectionScope::sketch();
  // A window takes the sketch's ordinary, construction and projected
  // geometry together...
  for (const QString& name : {QStringLiteral("c1"), QStringLiteral("c2"), QStringLiteral("c3")}) {
    CHECK(takes(scope, item(SelectKind::SketchCurve, kSketch, name), window()));
  }
  for (const QString& name : {QStringLiteral("p1"), QStringLiteral("p2"), QStringLiteral("p3")}) {
    CHECK(takes(scope, item(SelectKind::SketchPoint, kSketch, name), window()));
  }
  // ... but no body edges, faces, vertices, bodies, profiles, dimensions,
  // planes, nor another sketch's geometry.
  int looks = 0;
  CHECK(!takes(scope, item(SelectKind::Edge, QStringLiteral("F1.b0"), QStringLiteral("e")),
               edgePick(true, false, EdgeSight::Visible, &looks)));
  CHECK(looks == 0); // neither edge row takes windows: no look needed
  CHECK(!takes(scope, item(SelectKind::Face, QStringLiteral("F1.b0"), QStringLiteral("f")), window()));
  CHECK(!takes(scope, item(SelectKind::Vertex, QStringLiteral("F1.b0"), QStringLiteral("v")), window()));
  CHECK(!takes(scope, item(SelectKind::Body, QStringLiteral("F1.b0")), window()));
  CHECK(!takes(scope, item(SelectKind::Profile, kSketch, QStringLiteral("r1")), window()));
  CHECK(!takes(scope, item(SelectKind::SketchDimension, kSketch, QStringLiteral("k1")), window()));
  CHECK(!takes(scope, item(SelectKind::Plane, QStringLiteral("xy")), window()));
  CHECK(!takes(scope, item(SelectKind::SketchCurve, QStringLiteral("F2"), QStringLiteral("c1")), window()));
  // Shift does not widen a window.
  PickCase shiftWindow = window();
  shiftWindow.expanded = true;
  CHECK(!takes(scope, item(SelectKind::Face, QStringLiteral("F1.b0"), QStringLiteral("f")), shiftWindow));
}

void clickTests() {
  SelectionScope scope = SelectionScope::sketch();
  const SelectionItem edge = item(SelectKind::Edge, QStringLiteral("F1.b0"), QStringLiteral("e"));
  // Visible body edges by a click, occluded ones not; with Shift both.
  CHECK(takes(scope, edge, edgePick(false, false, EdgeSight::Visible, nullptr)));
  CHECK(!takes(scope, edge, edgePick(false, false, EdgeSight::Occluded, nullptr)));
  CHECK(takes(scope, edge, edgePick(false, true, EdgeSight::Occluded, nullptr)));
  // Dimensions, constraints and profiles by a click.
  CHECK(takes(scope, item(SelectKind::SketchDimension, kSketch, QStringLiteral("k1")), click()));
  CHECK(takes(scope, item(SelectKind::SketchConstraint, kSketch, QStringLiteral("fix:p1")), click()));
  CHECK(takes(scope, item(SelectKind::Profile, kSketch, QStringLiteral("r1")), click()));
  // Empty rows only with Shift.
  const SelectionItem face = item(SelectKind::Face, QStringLiteral("F1.b0"), QStringLiteral("f"));
  CHECK(!takes(scope, face, click()));
  CHECK(takes(scope, face, click(true)));
  CHECK(takes(scope, item(SelectKind::Vertex, QStringLiteral("F1.b0"), QStringLiteral("v")), click(true)));
  CHECK(takes(scope, item(SelectKind::Axis, QStringLiteral("x")), click(true)));
  CHECK(takes(scope, item(SelectKind::Component, QStringLiteral("C1")), click(true)));
  // Faces win over bodies: Shift does not widen to bodies beside faces.
  const SelectionItem body = item(SelectKind::Body, QStringLiteral("F1.b0"));
  CHECK(!takes(scope, body, click(true)));
  scope.setState(FilterRow::Bodies, FilterState::Click);
  CHECK(takes(scope, body, click()));
  CHECK(!takes(scope, face, click(true))); // bodies offered: faces not widened
  // Other sketches' geometry never, also with Shift; texts and unknown
  // kinds neither.
  for (const SelectKind kind : {SelectKind::SketchCurve, SelectKind::SketchPoint, SelectKind::Profile,
                                SelectKind::SketchDimension, SelectKind::SketchConstraint}) {
    CHECK(!takes(scope, item(kind, QStringLiteral("F2"), QStringLiteral("c1")), click(true)));
  }
  CHECK(!takes(scope, item(SelectKind::Sketch, kSketch), click(true)));
  CHECK(!takes(scope, item(SelectKind::Feature, QStringLiteral("F3")), click(true)));
}

void independentRowTests() {
  // Ordinary, construction and projected geometry are separate rows: one
  // empty leaves the others as they are.
  SelectionScope scope = SelectionScope::sketch();
  scope.setState(FilterRow::SketchGeometry, FilterState::Off);
  CHECK(!takes(scope, item(SelectKind::SketchCurve, kSketch, QStringLiteral("c1")), click()));
  CHECK(takes(scope, item(SelectKind::SketchCurve, kSketch, QStringLiteral("c2")), window()));
  CHECK(takes(scope, item(SelectKind::SketchCurve, kSketch, QStringLiteral("c3")), window()));
  CHECK(takes(scope, item(SelectKind::SketchCurve, kSketch, QStringLiteral("c1")), click(true)));
  CHECK(!takes(scope, item(SelectKind::SketchCurve, kSketch, QStringLiteral("c1")), window()));
  scope.reset();
  scope.setState(FilterRow::ProjectedGeometry, FilterState::Click);
  CHECK(takes(scope, item(SelectKind::SketchPoint, kSketch, QStringLiteral("p3")), click()));
  CHECK(!takes(scope, item(SelectKind::SketchPoint, kSketch, QStringLiteral("p3")), window()));
  CHECK(takes(scope, item(SelectKind::SketchPoint, kSketch, QStringLiteral("p1")), window()));
}

void edgeSightTests() {
  SelectionScope scope = SelectionScope::sketch();
  const SelectionItem edge = item(SelectKind::Edge, QStringLiteral("F1.b0"), QStringLiteral("e"));
  // Visible edges a cross: a window takes an edge with a part in sight
  // inside it (deduplicated: one item), not one wholly behind faces.
  scope.setState(FilterRow::VisibleEdges, FilterState::Window);
  int looks = 0;
  CHECK(takes(scope, edge, edgePick(true, false, EdgeSight::Visible, &looks)));
  CHECK(takes(scope, edge, edgePick(true, false, EdgeSight::Both, &looks)));
  CHECK(!takes(scope, edge, edgePick(true, false, EdgeSight::Occluded, &looks)));
  CHECK(looks == 3);
  // Occluded edges a cross and visible empty: the other way round.
  scope.setState(FilterRow::VisibleEdges, FilterState::Off);
  scope.setState(FilterRow::OccludedEdges, FilterState::Window);
  CHECK(!takes(scope, edge, edgePick(true, false, EdgeSight::Visible, nullptr)));
  CHECK(takes(scope, edge, edgePick(true, false, EdgeSight::Both, nullptr)));
  CHECK(takes(scope, edge, edgePick(true, false, EdgeSight::Occluded, nullptr)));
  CHECK(!takes(scope, edge, edgePick(false, false, EdgeSight::Visible, nullptr)));
  CHECK(takes(scope, edge, edgePick(false, false, EdgeSight::Occluded, nullptr)));
  // Both rows alike: no look along the view.
  scope.setState(FilterRow::VisibleEdges, FilterState::Window);
  looks = 0;
  CHECK(takes(scope, edge, edgePick(true, false, EdgeSight::Occluded, &looks)));
  CHECK(takes(scope, edge, edgePick(false, false, EdgeSight::Visible, &looks)));
  CHECK(looks == 0);
}

void projectTests() {
  const SelectionScope scope = SelectionScope::project();
  const SelectionItem edge = item(SelectKind::Edge, QStringLiteral("F1.b0"), QStringLiteral("e"));
  CHECK(takes(scope, edge, edgePick(true, false, EdgeSight::Visible, nullptr)));
  CHECK(takes(scope, edge, edgePick(false, false, EdgeSight::Visible, nullptr)));
  CHECK(!takes(scope, edge, edgePick(false, false, EdgeSight::Occluded, nullptr)));
  CHECK(takes(scope, edge, edgePick(false, true, EdgeSight::Occluded, nullptr)));
  CHECK(!takes(scope, edge, edgePick(true, false, EdgeSight::Occluded, nullptr)));
  const SelectionItem face = item(SelectKind::Face, QStringLiteral("F1.b0"), QStringLiteral("f"));
  CHECK(takes(scope, face, click()));
  CHECK(!takes(scope, face, window()));
  CHECK(takes(scope, item(SelectKind::Vertex, QStringLiteral("F1.b0"), QStringLiteral("v")), click()));
  // Nothing else, also with Shift.
  for (const SelectKind kind : {SelectKind::Body, SelectKind::Plane, SelectKind::Axis, SelectKind::Point,
                                SelectKind::Component}) {
    CHECK(!takes(scope, item(kind, QStringLiteral("F1.b0")), click(true)));
  }
  CHECK(!takes(scope, item(SelectKind::SketchCurve, kSketch, QStringLiteral("c1")), click(true)));
  CHECK(!takes(scope, item(SelectKind::Profile, kSketch, QStringLiteral("r1")), click(true)));
  // Project's scope is the one command that has one.
  CHECK(commandSelectionScope(QStringLiteral("sketch.project")).has_value());
  CHECK(commandSelectionScope(QStringLiteral("sketch.project"))->name() == QStringLiteral("project"));
  CHECK(!commandSelectionScope(QStringLiteral("sketch.offset")).has_value());
  CHECK(!commandSelectionScope(QStringLiteral("solid.fillet")).has_value());
}

} // namespace

int selectionScopeTests() {
  failures = 0;
  cycleTests();
  defaultTests();
  sketchWindowTests();
  clickTests();
  independentRowTests();
  edgeSightTests();
  projectTests();
  return failures;
}
