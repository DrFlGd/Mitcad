// SPDX-License-Identifier: MIT
#pragma once

// View > Export Snapshot (mitcad#649, app/COMMANDS.md "Measurement
// objects"): the dialog of a snapshot's file and choices, which it starts
// with from the settings (group `snapshot`) and saves back on OK.

#include <optional>

#include <QSize>
#include <QString>

class QWidget;

namespace mitcad {

struct SnapshotChoice {
  QString path;      // a .png (or .jpg) file
  QString sizeKey;   // 2x, 4x, width
  QSize size;        // the picture's pixels
  bool transparent = false; // PNG only
  bool measurements = true; // the shown measurements drawn over it
};

// The picture's size for a view of `view` logical pixels: 2x or 4x the
// view, or `width` pixels wide with the view's aspect.
QSize snapshotSize(const QString& sizeKey, int width, const QSize& view);

// `measurements`: how many the view shows.
std::optional<SnapshotChoice> askSnapshot(QWidget* parent, const QString& path, const QSize& view,
                                          int measurements);

} // namespace mitcad
