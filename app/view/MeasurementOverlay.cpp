// SPDX-License-Identifier: MIT
// Measurement objects over the 3D view (mitcad#649, app/COMMANDS.md
// "Measurement objects"): OcctViewer::setMeasurements and the overlay that
// draws them.
#include "MeasurementOverlay.hpp"

#include <cmath>
#include <optional>
#include <vector>

#include <QEvent>
#include <QPainter>
#include <QStringList>
#include <QtLogging>

#include "../OcctViewer.hpp"
#include "MeasurementDrawing.hpp"

namespace mitcad {

void OcctViewer::setMeasurements(const std::vector<MeasurementDisplay>& measurements) {
  m_measurements = measurements;
  if (m_measurementOverlay == nullptr && !measurements.empty()) {
    m_measurementOverlay = new MeasurementOverlay(this);
  }
  if (m_measurementOverlay != nullptr) {
    m_measurementOverlay->refresh();
  }
  // For UI tests: what is drawn, when it changes; and where the labels
  // are, in the window's coordinates.
  QStringList drawn;
  for (const MeasurementDisplay& m : measurements) {
    drawn << QStringLiteral("%1 (%2)").arg(m.label, m.color.name());
  }
  const QString logged =
      QStringLiteral("Measurements drawn: %1").arg(drawn.isEmpty() ? QStringLiteral("none") : drawn.join(QStringLiteral(" | ")));
  if (logged != m_loggedMeasurements) {
    m_loggedMeasurements = logged;
    qDebug().noquote() << logged;
    const MeasurementProjection project = [this](const gp_Pnt& p) -> std::optional<QPointF> { return toWidgetF(p); };
    for (const MeasurementDisplay& m : measurements) {
      const MeasurementShape shape = layoutMeasurement(m, project);
      if (shape.valid) {
        const QPoint at = mapTo(window(), shape.labelAt.toPoint());
        qDebug().noquote() << QStringLiteral("Measurement label %1 at %2,%3").arg(m.name).arg(at.x()).arg(at.y());
      }
    }
  }
}

MeasurementOverlay::MeasurementOverlay(OcctViewer* viewer) : QWidget(viewer), m_viewer(viewer) {
  setObjectName(QStringLiteral("measurementOverlay"));
  setAttribute(Qt::WA_TransparentForMouseEvents);
  setAttribute(Qt::WA_NoSystemBackground);
  setAttribute(Qt::WA_TranslucentBackground);
  setFocusPolicy(Qt::NoFocus);
  hide();
  viewer->installEventFilter(this);
  // The camera moved (a frame was drawn): the dimensions follow their
  // points.
  connect(viewer, &OcctViewer::viewChanged, this, [this] {
    if (isVisible()) {
      follow();
    }
  });
  // The background turned dark or light: the halos take its tone.
  connect(viewer, &OcctViewer::backgroundToneChanged, this, [this] { update(); });
}

void MeasurementOverlay::refresh() {
  if (m_viewer->measurements().empty()) {
    hide();
    return;
  }
  show();
  lower(); // under the view's own buttons and cards
  if (geometry() != m_viewer->rect()) {
    setGeometry(m_viewer->rect());
  }
  m_drawnKey = placesKey();
  update();
}

QString MeasurementOverlay::placesKey() const {
  QStringList places;
  for (const MeasurementDisplay& m : m_viewer->measurements()) {
    const gp_Pnt p = m.arc ? m.corner : m.circle ? m.center : !m.points.empty() ? m.points.front() : gp_Pnt();
    const QPointF at = m_viewer->toWidgetF(p);
    places << QStringLiteral("%1,%2").arg(std::lround(at.x())).arg(std::lround(at.y()));
  }
  return places.join(QLatin1Char(' '));
}

void MeasurementOverlay::follow() {
  if (geometry() != m_viewer->rect()) {
    setGeometry(m_viewer->rect());
    update();
  }
  const QString key = placesKey();
  if (key != m_drawnKey) {
    m_drawnKey = key;
    update();
  }
}

bool MeasurementOverlay::eventFilter(QObject* watched, QEvent* event) {
  if (watched == m_viewer && event->type() == QEvent::Resize && isVisible()) {
    follow();
  }
  return QWidget::eventFilter(watched, event);
}

void MeasurementOverlay::paintEvent(QPaintEvent* /*event*/) {
  const MeasurementProjection project = [this](const gp_Pnt& p) -> std::optional<QPointF> {
    return m_viewer->toWidgetF(p);
  };
  std::vector<MeasurementShape> shapes;
  for (const MeasurementDisplay& m : m_viewer->measurements()) {
    shapes.push_back(layoutMeasurement(m, project));
  }
  QPainter painter(this);
  paintMeasurements(painter, shapes, m_viewer->sketchHaloColor(), measurementFont(font()));
}

} // namespace mitcad
