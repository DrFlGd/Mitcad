// SPDX-License-Identifier: MIT
// Parameter names picked from the view into a feature panel's value field
// (mitcad#250; framework/ParameterPicker.hpp): what the window tells a
// running command about the sketches the view shows and the commands that
// edit features.
#include "MainWindow.hpp"

#include <utility>

#include <TopLoc_Location.hxx>

#include "framework/CommandRegistry.hpp"

namespace mitcad {

std::vector<CommandHost::ShownSketch> MainWindow::shownSketches() const {
  // As showSketches() shows them (also those a command shows for its time,
  // mitcad#546), without one sketch mode edits (it draws its own
  // dimensions).
  std::vector<ShownSketch> shown;
  for (const DocumentSnapshot::Feature& feature : std::as_const(m_snapshot.features)) {
    if (!feature.isSketch() || !feature.succeeded() || !sketchShown(feature.uid) ||
        (m_sketch->isActive() && feature.uid == m_sketch->uid())) {
      continue;
    }
    ShownSketch entry{feature.uid, gp_Trsf()};
    const QString occurrence = placingOccurrence(feature.uid);
    if (!occurrence.isEmpty()) {
      if (!m_snapshot.isOccurrenceShown(occurrence)) {
        continue; // the component's occurrence is hidden
      }
      entry.placement = placementOf(occurrence).Transformation();
    }
    shown.push_back(std::move(entry));
  }
  return shown;
}

const CommandDef* MainWindow::featureEditor(const QJsonObject& def) const {
  return m_registry != nullptr ? m_registry->editorOf(def) : nullptr;
}

} // namespace mitcad
