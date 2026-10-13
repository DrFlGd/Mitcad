// SPDX-License-Identifier: MIT
#pragma once

#include <QString>

#include "HybridAssembly.hpp"

class QWidget;

namespace mitcad {

// Move to... on a component occurrence (mitcad#219): asks for the new
// parent among the root and the placed components, each by its full path
// with the reason when it cannot be one; shows what moves with it and the
// references that follow it; then runs the model's `reparent_occurrence`
// (one undo step; the occurrence stays where it is in the design).
// `occurrence` is its path from the root (`O1/O4`). Returns its new path,
// or an empty string when nothing moved (Cancel). A refused move keeps the
// dialog open with the model's reason.
QString moveOccurrence(QWidget* parent, const HybridModel& model, const QString& occurrence);

} // namespace mitcad
