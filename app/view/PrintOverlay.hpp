// SPDX-License-Identifier: MIT
#pragma once

#include <optional>
#include <utility>
#include <vector>

#include <QColor>
#include <QPointF>
#include <QString>
#include <QWidget>

#include <gp_Pnt.hxx>

namespace mitcad {

class OcctViewer;
struct PrintOverlay;

// Print Analysis over the 3D view (mitcad#620, OcctViewer::setPrintOverlay):
// a marker and a label at the thinnest point, kept where the point is as
// the camera moves, and the legend (the analysis' name, its colours with
// the angle and the minimum wall) in the view's lower left corner. A child
// of the view as large as it, which lets the mouse through; the triangles
// themselves are drawn by the view (PrintOverlay.cpp).
class PrintOverlayLabels : public QWidget {
public:
  explicit PrintOverlayLabels(OcctViewer* viewer);

  // Shows what the overlay has of it; an empty overlay hides it.
  void setOverlay(const PrintOverlay& overlay);

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void paintEvent(QPaintEvent* event) override;

private:
  // As large as the view, drawn again when the marker's place moved.
  void follow();

  OcctViewer* m_viewer;
  std::optional<gp_Pnt> m_point; // the thinnest point, in the design
  QString m_label;
  QString m_title;
  std::vector<std::pair<QColor, QString>> m_legend;
  QColor m_markerColor;
  QPointF m_drawnAt; // where the marker was drawn last, in the view
};

} // namespace mitcad
