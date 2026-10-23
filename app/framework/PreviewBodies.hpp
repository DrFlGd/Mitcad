// SPDX-License-Identifier: MIT
#pragma once

// Which bodies a feature's preview draws, and where (mitcad#266): from the
// model's preview report (`preview`, core/model/src/api/commands.md) and the
// bodies the view shows now. Plain Qt Core, so that the rules are tested
// without a model or a view (app/tests).
//
// The preview is the design as the normal view shows it, with the
// command's result:
//   - a body the view shows keeps its occurrence's placement and its look;
//     the preview's shape replaces its own when the command changed it; a
//     body or an occurrence/body pair the command removes is left out;
//   - an occurrence the command places elsewhere (`placements`) moves its
//     bodies there;
//   - an occurrence/body pair the command makes (`instances`: a new body,
//     in the root or in a component, or a new occurrence of a component) is
//     drawn as new, at the placement into the design the model gives, when
//     it is visible;
//   - nothing else. A body the view does not show (a hidden body, one in a
//     hidden occurrence, one outside an isolation) stays hidden, also when
//     the command changes it, and a body is never drawn without a known
//     placement (at the design's origin in its component's coordinates).

#include <vector>

#include <QJsonArray>
#include <QJsonObject>
#include <QString>

namespace mitcad {

// A body as the view shows it now (MainWindow::modelBodies).
struct ShownBody {
  QString uid;
  // The occurrence that places it: uids from the root ("O1/O4"); empty in
  // the root component.
  QString occurrence;
  // Its placement into the design: 3 or 4 rows of 4 numbers; empty for the
  // identity.
  QJsonArray transform;
};

// A body the preview draws.
struct PreviewBody {
  QString uid;
  QString occurrence;
  // The shown body whose look (colour, appearance, clipping) it takes: its
  // index in the shown bodies, or -1.
  int shown = -1;
  // Drawn with the preview's shape of the body (preview_body_shape).
  bool previewShape = false;
  // Made by the command: drawn see-through, not pickable.
  bool made = false;
  // The placement comes from the report (a moved occurrence or a new
  // pair), not from the shown body.
  bool placed = false;
  // Where it is drawn: rows into the design as above; empty for the
  // identity.
  QJsonArray transform;
};

// The bodies a successful preview report draws, given the bodies shown now;
// none for a report whose status is not "ok".
std::vector<PreviewBody> previewBodies(const QJsonObject& report, const std::vector<ShownBody>& shown);

// The shown bodies a successful preview no longer has (mitcad#561: a tool
// the command consumes, gone from every occurrence of its component): the
// indices in `shown` of each such occurrence/body pair; none for a report
// whose status is not "ok". A command that consumes tools draws them as
// tool overlays where they are not picked, so that they can be picked
// there too.
std::vector<int> removedShownBodies(const QJsonObject& report, const std::vector<ShownBody>& shown);

} // namespace mitcad
