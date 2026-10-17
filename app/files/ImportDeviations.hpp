// SPDX-License-Identifier: MIT
#pragma once

#include <functional>
#include <optional>
#include <vector>

#include <QJsonArray>
#include <QJsonObject>
#include <QSet>
#include <QString>
#include <QStringList>

class QWidget;

namespace mitcad {

// An item of an imported .f3d design whose decoded definition built a
// result that deviates from the bodies stored in the file (mitcad#379):
// the import report's `deviation` of the item, which came in as the file's
// stored geometry.
struct DeviationRow {
  qint64 index = 0;   // timeline index
  QString name;       // Fillet6
  QString type;       // FilletFeature
  QString component;  // empty: the root component
  double volumeChange = 0.0;     // mm³ the item adds (negative: removes)
  double fileVolumeChange = 0.0; // mm³ the file's operation adds
  double difference = 0.0;       // the two apart, mm³
  std::optional<double> relative;      // the difference against the file's change
  std::optional<double> largest;       // mm, its new faces from the file's
  std::optional<double> beyond;        // share of their area beyond the tolerance
  std::optional<double> fileLargest;   // the same of the file's new faces
  double bodyVolume = 0.0;       // mm³, the bodies it changed
  double storedBodyVolume = 0.0; // mm³, the file's
  double tolerance = 0.01;       // mm
  QString reason;
  QJsonArray faces; // [{"body", "face"}]: where it deviates, to select
  // A rounding the geometry kernel fails with all its edges, offered
  // without these.
  QStringList missingEdges;
};

// The import result's deviating items (`report.designs[0].items[].deviation`).
std::vector<DeviationRow> deviationRows(const QJsonObject& result);

// The dialog over the imported design (mitcad#379): a row per deviating
// item with how far off it is, each to keep parametric or to leave as the
// file's stored geometry (the default). Selecting a row calls `highlight`
// with it (nullptr when none is selected), to select its faces in the
// view. Keep All Parametric, Use Stored for All. Returns the timeline
// indices to keep parametric (none: all stay the file's).
QSet<qint64> askDeviations(QWidget* parent, const QString& file, const std::vector<DeviationRow>& rows,
                           const std::function<void(const DeviationRow*)>& highlight);

} // namespace mitcad
