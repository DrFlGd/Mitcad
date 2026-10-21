// SPDX-License-Identifier: MIT
#pragma once

// File > Export Drawing (mitcad#471): the dialog of a drawing's file and
// choices, which it starts with from the settings (DrawingSettings, group
// `drawing`) and saves back on OK.

#include <optional>

#include <QString>
#include <QStringList>

#include "DrawingSettings.hpp"

class QWidget;

namespace mitcad {

struct DrawingChoice {
  QString path;
  DrawingSettings settings;
  bool selectedOnly = false;
  QString section; // a Section Analysis by name; empty: none
};

// `selectedBodies`: how many bodies the selection stands for;
// `analyses`: the document's Section Analyses by name.
std::optional<DrawingChoice> askDrawing(QWidget* parent, const QString& path, int selectedBodies,
                                        const QStringList& analyses);

} // namespace mitcad
