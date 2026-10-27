// SPDX-License-Identifier: MIT
#pragma once

// Export Drawing (mitcad#471): the choices of its dialog, kept in the
// settings (group `drawing`) as the other export dialogs keep theirs, and
// the model's export_drawing command they make (core/model-api/src/
// commands.md). Plain Qt Core: app.unit tests it.

#include <QJsonObject>
#include <QList>
#include <QPair>
#include <QString>
#include <QStringList>

class QSettings;

namespace mitcad {

struct DrawingSettings {
  QString format = QStringLiteral("pdf"); // pdf, svg, dxf, dwg
  QString sheet = QStringLiteral("A4");   // A4..A0, ANSI_A..ANSI_E
  bool portrait = false;
  QString scale = QStringLiteral("fit"); // fit, or "1:2", "2:1"
  bool firstAngle = false;
  bool front = true;
  bool top = true;
  bool right = true;
  bool iso = false;
  bool hiddenLines = false;
  bool tangentEdges = false;
  bool dimensions = true;
  bool holeTable = true;
  bool sketchDimensions = false;
  bool perBody = false;

  // The sheets the dialog offers, in its order: (value, label).
  static QList<QPair<QString, QString>> sheets();

  // From the settings' group `drawing`, unknown values as the defaults.
  static DrawingSettings load(QSettings& settings);
  void save(QSettings& settings) const;

  // The views in the command's order.
  QStringList views() const;

  // export_drawing for the file; `bodies` empty: the shown ones; `section`
  // a Section Analysis by name, or none.
  QJsonObject command(const QString& path, const QStringList& bodies, const QString& title,
                      const QString& section) const;
};

// A scale typed or chosen ("1:2", "2:1", "fit"), or empty when it is none.
QString normalizedScale(const QString& text);

} // namespace mitcad
