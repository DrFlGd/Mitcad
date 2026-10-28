// SPDX-License-Identifier: MIT
#pragma once

// The Sketch page of Preferences (mitcad#860): "Scale the sketch to its
// first dimension", on by default. While it is on, a sketch action's
// `sketch.add_dimension` asks the model for the first-dimension rule
// (`scale_sketch`, commands.md "Scaling a sketch"): the first driving
// dimension of a sketch drawn out of proportion scales the whole sketch to
// it, in the same undo step.

#include <QJsonObject>

namespace mitcad::sketch {

// The preference (settings "sketch/scaleToFirstDimension"). Never saved:
// on, unless MITCAD_SKETCH_FIRST_DIMENSION_SCALE is 0 (the UI test
// libraries set it, so that their drawn sketches keep their sizes).
bool scaleToFirstDimension();
void saveScaleToFirstDimension(bool on);

// `command` with `scale_sketch` when it is a `sketch.add_dimension` and the
// preference is on; any other command as it is.
QJsonObject withFirstDimensionScale(const QJsonObject& command);

} // namespace mitcad::sketch
