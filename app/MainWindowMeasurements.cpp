// SPDX-License-Identifier: MIT
// Measurement objects (mitcad#649, app/COMMANDS.md "Measurement objects"):
// Keep in the Measure panel and on a row of its history adds the picks as
// a measurement object of the design (the model's add_measurement, whose
// default kind and name the model chooses), outside the panel's undo
// range, as a light bulb's change is (CommandSession::runVisibility); the
// shown ones are drawn as dimensions over the view after every refresh
// (OcctViewer::setMeasurements).
#include "MainWindow.hpp"

#include <initializer_list>
#include <utility>

#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QStringList>
#include <QtLogging>

#include "framework/Command.hpp"
#include "framework/CommandSession.hpp"
#include "view/MeasurementDrawing.hpp"

namespace mitcad {
namespace {

// A pick as add_measurement takes it (the measure query's selection as
// fields, with its occurrence); null for what the model does not measure
// (sketch curves and points, measured by the panel itself).
QJsonValue measurementPick(const SelectionItem& item) {
  QJsonObject pick;
  switch (item.kind) {
  case SelectKind::Plane:
  case SelectKind::Axis:
  case SelectKind::Point:
    pick.insert(QStringLiteral("datum"), item.owner);
    break;
  case SelectKind::Body:
  case SelectKind::Face:
  case SelectKind::Edge:
  case SelectKind::Vertex: {
    const QJsonValue reference = item.reference();
    if (!reference.isObject()) {
      return QJsonValue();
    }
    pick = reference.toObject();
    break;
  }
  default:
    return QJsonValue();
  }
  if (!item.occurrence.isEmpty()) {
    pick.insert(QStringLiteral("occurrence"), item.occurrence);
  }
  return pick;
}

} // namespace

bool MainWindow::measurementPicks(const QHash<QString, Selection>& inputs, QJsonObject& command, QString& why) {
  const auto first = [&inputs](std::initializer_list<const char*> keys) -> const SelectionItem* {
    for (const char* key : keys) {
      const auto found = inputs.constFind(QString::fromLatin1(key));
      if (found != inputs.constEnd() && !found->isEmpty()) {
        return &found->first();
      }
    }
    return nullptr;
  };
  const SelectionItem* a = first({"a", "body_a"});
  const SelectionItem* b = first({"b", "body_b"});
  if (a == nullptr) {
    why = tr("Pick what to measure first.");
    return false;
  }
  const QJsonValue pickA = measurementPick(*a);
  const QJsonValue pickB = b != nullptr ? measurementPick(*b) : QJsonValue();
  if (pickA.isNull() || (b != nullptr && pickB.isNull())) {
    why = tr("Sketch curves and points are measured in the panel only: pick the bodies' faces, edges or "
             "vertices to keep a measurement.");
    return false;
  }
  command.insert(QStringLiteral("a"), pickA);
  if (b != nullptr) {
    command.insert(QStringLiteral("b"), pickB);
  }
  return true;
}

void MainWindow::keepMeasurement(const QHash<QString, Selection>& inputs, const QString& what) {
  if (!m_session || m_session->def().id != QStringLiteral("inspect.measure")) {
    return;
  }
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("add_measurement")}};
  QString why;
  if (!measurementPicks(inputs, command, why)) {
    showHint(why);
    qDebug().noquote() << QStringLiteral("Measurement not kept (%1): %2").arg(what, why);
    return;
  }
  // Listed once the model has it (followMeasurements); a light bulb's way
  // outside the panel's undo range, so that closing the panel keeps it.
  m_measurementKept = true;
  if (!m_session->runVisibility(QJsonArray{command}, tr("Keep Measurement"))) {
    m_measurementKept = false;
    qDebug().noquote() << QStringLiteral("Measurement not kept (%1): %2").arg(what, lastError());
  }
}

void MainWindow::keepMeasurementRow(int number) {
  const MeasureHistory::Row* row = m_measureHistory.row(number);
  if (row == nullptr) {
    return;
  }
  // A copy: keeping refreshes the rows.
  const QHash<QString, Selection> inputs = row->inputs;
  keepMeasurement(inputs, QStringLiteral("row %1").arg(number));
}

void MainWindow::followMeasurements() {
  const QString unit = m_snapshot.lengthUnit;
  const double millimetres = lengthUnitMillimetres(unit);
  m_viewer->setMeasurements(measurementDisplays(
      m_snapshot.measurements, [unit, millimetres](const QString& kind, double value) {
        return measurementValueText(kind, value, unit, millimetres);
      }));
  // For UI tests: each one's state when they change, and one just kept.
  QStringList names;
  QStringList states;
  for (const QJsonValue& value : std::as_const(m_snapshot.measurements)) {
    const QJsonObject measurement = value.toObject();
    const QString name = measurement.value(QStringLiteral("name")).toString();
    names << name;
    const QString error = measurement.value(QStringLiteral("error")).toString();
    states << (error.isEmpty() ? QStringLiteral("%1 %2%3")
                                     .arg(name, measurement.value(QStringLiteral("kind")).toString(),
                                          measurement.value(QStringLiteral("visible")).toBool(true)
                                              ? QString()
                                              : QStringLiteral(" (hidden)"))
                               : QStringLiteral("%1 broken: %2").arg(name, error));
  }
  const QString logged = QStringLiteral("Measurements: %1")
                             .arg(states.isEmpty() ? QStringLiteral("none") : states.join(QStringLiteral(" | ")));
  if (logged != m_loggedMeasurements) {
    m_loggedMeasurements = logged;
    qDebug().noquote() << logged;
  }
  if (m_measurementKept) {
    for (const QString& name : std::as_const(names)) {
      if (!m_measurementNames.contains(name)) {
        qDebug().noquote() << QStringLiteral("Measurement kept: %1").arg(name);
        m_measurementKept = false;
      }
    }
  }
  m_measurementNames = names;
}

} // namespace mitcad
