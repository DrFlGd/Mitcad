// SPDX-License-Identifier: MIT
#pragma once

#include <QString>
#include <QWidget>

namespace mitcad {

class OcctViewer;

// Measurement objects over the 3D view (mitcad#649,
// OcctViewer::setMeasurements): the shown ones drawn as dimensions
// (view/MeasurementDrawing.hpp), kept where their points are as the camera
// moves. A child of the view as large as it, which lets the mouse through,
// under the view's own buttons and cards.
class MeasurementOverlay : public QWidget {
public:
  explicit MeasurementOverlay(OcctViewer* viewer);

  // The view's measurements changed: shown when there are any.
  void refresh();

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void paintEvent(QPaintEvent* event) override;

private:
  // As large as the view; drawn again when the measurements' places on
  // the view moved.
  void follow();
  // Where the measurements' first points are on the view, rounded.
  QString placesKey() const;

  OcctViewer* m_viewer;
  QString m_drawnKey; // placesKey when drawn last
};

} // namespace mitcad
