// SPDX-License-Identifier: MIT
// Light bulbs in every mode (mitcad#295) and the origin planes of Create
// Sketch's plane pick (mitcad#294, mitcad#474: its banner and the picks it
// refuses) in the main window.
#include "MainWindow.hpp"

#include <QtLogging>

#include "OcctViewer.hpp"
#include "view/ModeBanner.hpp"

namespace mitcad {

// The browser's light bulbs while Create Sketch asks for a plane, while
// sketching and during commands: a command's session runs them outside its
// undo range; the plane pick goes on over the bodies shown now.
bool MainWindow::runVisibilityCommands(const QJsonArray& commands, const QString& label) {
  if (commands.isEmpty()) {
    return true;
  }
  if (m_mode == Mode::Command && m_session) {
    return m_session->runVisibility(commands, label);
  }
  const bool done =
      commands.size() == 1 ? runModelCommand(commands.first().toObject()) : runModelCommands(commands, label);
  if (m_mode == Mode::PickPlane) {
    // refreshScene kept the pick's filter (updateIdleView) and its hint.
    logSketchPicks();
  }
  return done;
}

QString MainWindow::activeCommandName() const {
  switch (m_mode) {
  case Mode::Command:
    return m_session ? m_session->def().name : QString();
  case Mode::PickPlane:
    return QStringLiteral("Create Sketch");
  case Mode::Sketch:
    return m_sketch->isActive() ? m_sketch->model().name : QString();
  default:
    return QString();
  }
}

// "No components" counts neither the active component nor a root without
// bodies of its own: a new design's empty Part1 is active under an empty
// root, and its first sketch starts on the origin planes; so does a design
// whose bodies are all the active component's. Another component, or the
// root's bodies beside the active component, is where sketches usually
// start instead (a component's sketch may attach there), and the
// planes would only cover it; the Origin folder shows them on request.
bool MainWindow::sketchShowsOrigin() const {
  for (const DocumentSnapshot::Component& component : m_snapshot.components) {
    if (component.uid == m_snapshot.activeComponent) {
      continue;
    }
    if (component.uid != QStringLiteral("C0") || !component.bodies.isEmpty()) {
      return false;
    }
  }
  return true;
}

void MainWindow::logSketchPicks() {
  if (!qEnvironmentVariableIsSet("MITCAD_LOG_PICKS")) {
    return;
  }
  // Where a click picks each face the policy lets the sketch lie on, and
  // where it hits each face that cannot take the sketch (mitcad#474: not
  // planar, or left out by the policy), as "Pick refused face ...".
  qDebug().noquote() << "Pick places:";
  const auto logPlace = [this](const SelectionItem& item, const QPoint& at, const QString& prefix) {
    const QPoint point = m_viewer->mapTo(this, at);
    qDebug().noquote() << QStringLiteral("Pick %1%2 %3/%4%5 at %6,%7")
                              .arg(prefix, SelectionItem::kindName(item.kind), item.owner, item.name,
                                   item.occurrence.isEmpty() ? QString() : QStringLiteral(" in ") + item.occurrence)
                              .arg(point.x())
                              .arg(point.y());
  };
  for (const auto& [item, at] : m_viewer->pickPoints(SelectKind::Face)) {
    logPlace(item, at, QString());
  }
  for (const auto& [item, at] : m_viewer->refusedPickPoints(SelectKind::Face)) {
    logPlace(item, at, QStringLiteral("refused "));
  }
}

QString MainWindow::pickPlaneText() const {
  return tr("Create Sketch: pick a plane or a planar face. Esc cancels.");
}

void MainWindow::updateModeBanner() {
  if (m_modeBanner != nullptr) {
    m_modeBanner->setText(m_mode == Mode::PickPlane ? pickPlaneText() : QString());
  }
}

void MainWindow::refusePlanePick(const SelectionItem& item) {
  QString reason;
  if (item.kind == SelectKind::Plane ||
      (item.kind == SelectKind::Face && item.geometry == QStringLiteral("plane"))) {
    reason = sketchSupportRefusal(item); // the design policy's (mitcad#220)
  } else if (item.kind == SelectKind::Face) {
    // What the face is instead of a plane, from its surface's type.
    const QString& surface = item.geometry;
    const QString kind = surface == QStringLiteral("cylinder") ? tr("a cylindrical surface")
                         : surface == QStringLiteral("cone")   ? tr("a conical surface")
                         : surface == QStringLiteral("sphere") ? tr("a spherical surface")
                         : surface == QStringLiteral("torus")  ? tr("a toroidal surface")
                         : surface == QStringLiteral("mesh")   ? tr("a mesh's triangles")
                                                               : tr("a free-form surface, such as a thread's");
    reason = tr("%1 is not planar (%2); pick a planar face or a plane, or make one with Construct.")
                 .arg(item.describe(), kind);
  }
  if (reason.isEmpty()) {
    return;
  }
  showError(tr("Create Sketch: %1").arg(reason));
  qInfo().noquote() << QStringLiteral("Create Sketch: refused pick %1: %2").arg(item.describe(), reason);
}

} // namespace mitcad
