// SPDX-License-Identifier: MIT
// The view's context menu's split (mitcad#494): whether a selection alone
// completes a command (its first entries) or the command needs further
// picks (under More), computed from the definition (CommandDef::completedBy).
#include "framework/Command.hpp"

#include <cstdio>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "context_menu.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

} // namespace

int contextMenuTests() {
  using namespace mitcad;
  const SelectionItem face{SelectKind::Face, QStringLiteral("F2.b0"), QStringLiteral("F2:end(r1)"),
                           QStringLiteral("plane")};
  const SelectionItem side{SelectKind::Face, QStringLiteral("F2.b0"), QStringLiteral("F2:side(c1)"),
                           QStringLiteral("plane")};
  const SelectionItem edge{SelectKind::Edge, QStringLiteral("F2.b0"), QStringLiteral("E{a|b}"), QStringLiteral("line")};
  const SelectionItem profile{SelectKind::Profile, QStringLiteral("F1"), QStringLiteral("r1"), QStringLiteral("plane")};

  // One selection input and values: the faces complete it.
  CommandDef offset;
  offset.group = QStringLiteral("MODIFY");
  offset.inputs = {selectionInput(QStringLiteral("faces"), QStringLiteral("Faces"), SelectKind::Face, 1, 0),
                   valueInput(QStringLiteral("distance"), QStringLiteral("Distance"), ValueKind::Length,
                              QStringLiteral("2 mm"))};
  CHECK(offset.completedBy({face}));
  CHECK(offset.completedBy({face, side}));
  CHECK(!offset.completedBy({edge}));
  CHECK(!offset.completedBy({}));

  // A second required selection is a further pick; an optional one is not.
  CommandDef draft = offset;
  draft.inputs.insert(1, selectionInput(QStringLiteral("plane"), QStringLiteral("Plane"), SelectKind::Plane, 1, 1));
  CHECK(!draft.completedBy({face}));
  CommandDef optional = offset;
  optional.inputs.append(selectionInput(QStringLiteral("objects"), QStringLiteral("Objects"), SelectKind::Body, 0, 0));
  CHECK(optional.completedBy({face}));

  // A required input shown only in another mode is not needed at the start.
  CommandDef modes = offset;
  modes.inputs.append(choiceInput(QStringLiteral("mode"), QStringLiteral("Mode"),
                                  {{QStringLiteral("faces"), QStringLiteral("Faces")},
                                   {QStringLiteral("points"), QStringLiteral("Points")}},
                                  QStringLiteral("faces")));
  modes.inputs.append(selectionInput(QStringLiteral("points"), QStringLiteral("Points"), SelectKind::SketchPoint, 1, 0)
                          .withVisible([](const CommandState& state) {
                            return state.choice(QStringLiteral("mode")) == QStringLiteral("points");
                          }));
  CHECK(modes.completedBy({face}));
  modes.inputs[2].defaultChoice = QStringLiteral("points");
  CHECK(!modes.completedBy({face}));

  // As many as the first input needs, and no more than it takes.
  CommandDef sections = offset;
  sections.inputs[0].min = 2;
  CHECK(!sections.completedBy({face}));
  CHECK(sections.completedBy({face, side}));
  CommandDef single = offset;
  single.inputs[0].max = 1;
  CHECK(!single.completedBy({face, side}));

  // A list's first row: its other required inputs are further picks
  // (Fillet's edges alone complete it, a constraint's two geometries not).
  CommandDef fillet;
  fillet.group = QStringLiteral("MODIFY");
  fillet.inputs = {listInput(QStringLiteral("sets"), QStringLiteral("Sets"),
                             {selectionInput(QStringLiteral("edges"), QStringLiteral("Edges"),
                                             SelectKind::Edge | SelectKind::Face, 1, 0),
                              selectionInput(QStringLiteral("vertex"), QStringLiteral("Vertex"), SelectKind::Vertex, 0, 1)},
                             QStringLiteral("Add"), QStringLiteral("Set %1"))};
  CHECK(fillet.completedBy({edge}));
  CommandDef pair = fillet;
  pair.inputs[0].children[1].min = 1;
  CHECK(!pair.completedBy({edge}));

  // A creation command given a face only stands on it; a profile is what
  // it makes, and `completes` decides in place of the rule.
  CommandDef box;
  box.group = QStringLiteral("CREATE");
  box.inputs = {selectionInput(QStringLiteral("plane"), QStringLiteral("Plane"), SelectKind::Plane | SelectKind::Face, 1, 1)};
  CHECK(!box.completedBy({face}));
  CommandDef extrude;
  extrude.group = QStringLiteral("CREATE");
  extrude.inputs = {selectionInput(QStringLiteral("profiles"), QStringLiteral("Profiles"), SelectKind::Profile, 1, 0)};
  CHECK(extrude.completedBy({profile}));
  CommandDef thread = offset;
  thread.group = QStringLiteral("CREATE");
  CHECK(!thread.completedBy({face}));
  thread.completes = [](const Selection&) { return true; };
  CHECK(thread.completedBy({face}));
  CommandDef narrowed = offset;
  narrowed.completes = [](const Selection& items) { return items.size() == 1; };
  CHECK(narrowed.completedBy({face}) && !narrowed.completedBy({face, side}));

  // The state a command starts with: defaults and a list's first rows.
  const CommandState start = modes.initialState();
  CHECK(start.text(QStringLiteral("distance")) == QStringLiteral("2 mm"));
  CHECK(start.choice(QStringLiteral("mode")) == QStringLiteral("points"));
  CHECK(fillet.initialState().rows(QStringLiteral("sets")) == 1);
  return failures;
}
