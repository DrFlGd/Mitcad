// SPDX-License-Identifier: MIT
#include "ViewReveal.hpp"

#include <algorithm>
#include <limits>

#include <QMargins>
#include <QPointF>
#include <QRectF>

#include <Precision.hxx>
#include <gp_Vec.hxx>

#include "../OcctViewer.hpp"

namespace mitcad {

QString revealInView(OcctViewer& viewer, const std::vector<gp_Pnt>& points, bool fit) {
  if (points.empty()) {
    return QStringLiteral("none");
  }
  // The part of the view the panels and cards leave free.
  const QRectF area = QRectF(viewer.rect()).marginsRemoved(QMarginsF(viewer.overlayInsets()));
  const bool seen = std::any_of(points.begin(), points.end(),
                                [&](const gp_Pnt& point) { return area.contains(viewer.toWidgetF(point)); });
  if (seen && !fit) {
    return QStringLiteral("none");
  }
  CameraState camera = viewer.camera();
  gp_Vec view(camera.eye, camera.target);
  if (view.Magnitude() <= Precision::Confusion()) {
    return QStringLiteral("none");
  }
  view.Normalize();
  gp_Vec side = view.Crossed(gp_Vec(camera.up));
  if (side.Magnitude() <= Precision::Confusion()) {
    return QStringLiteral("none");
  }
  side.Normalize();
  const gp_Vec up = side.Crossed(view);
  // Their extent across the view, and the middle of their box.
  constexpr double kInfinity = std::numeric_limits<double>::infinity();
  double low[3] = {kInfinity, kInfinity, kInfinity};
  double high[3] = {-kInfinity, -kInfinity, -kInfinity};
  double minUp = kInfinity;
  double maxUp = -kInfinity;
  double minSide = kInfinity;
  double maxSide = -kInfinity;
  for (const gp_Pnt& point : points) {
    for (int i = 0; i < 3; ++i) {
      low[i] = std::min(low[i], point.Coord(i + 1));
      high[i] = std::max(high[i], point.Coord(i + 1));
    }
    const gp_Vec at(point.XYZ());
    minUp = std::min(minUp, at.Dot(up));
    maxUp = std::max(maxUp, at.Dot(up));
    minSide = std::min(minSide, at.Dot(side));
    maxSide = std::max(maxSide, at.Dot(side));
  }
  const gp_Pnt middle((low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0, (low[2] + high[2]) / 2.0);
  gp_Vec shift(camera.target, middle);
  shift -= view * shift.Dot(view); // across the view only: the distance stays
  camera.eye.Translate(shift);
  camera.target.Translate(shift);
  const double aspect = area.height() > 0.0 ? area.width() / area.height() : 1.0;
  // A margin around them.
  const double needed = std::max(maxUp - minUp, (maxSide - minSide) / std::max(aspect, 1e-6)) * 1.25;
  bool zoomed = false;
  if (needed > Precision::Confusion() && (needed > camera.height || fit)) {
    camera.height = needed;
    zoomed = true;
  }
  viewer.setCamera(camera);
  return zoomed ? QStringLiteral("zoom") : QStringLiteral("pan");
}

} // namespace mitcad
