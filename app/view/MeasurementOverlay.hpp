// SPDX-License-Identifier: MIT
#pragma once

#include <QPointF>

#include "MeasurementDrawing.hpp"
#include <QString>
#include <QWidget>

namespace mitcad {

class OcctViewer;

// Annotations in the view (mitcad#649, mitcad#702;
// OcctViewer::setMeasurements): the shown ones drawn in their planes
// (view/MeasurementDrawing.hpp), kept where their points are as the camera
// moves. A child of the view as large as it, under the view's own buttons
// and cards. It lets the mouse through itself; it watches the view's mouse
// instead and takes only what lands on an annotation's text or dimension
// line: a click (OcctViewer::AnnotationActions::clicked), a drag of the
// text or the line (moved, once released; drawn where it goes meanwhile),
// a double-click (opened) and a right click (menu).
class MeasurementOverlay : public QWidget {
public:
  explicit MeasurementOverlay(OcctViewer* viewer);

  // The view's annotations changed: shown when there are any.
  void refresh();

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void paintEvent(QPaintEvent* event) override;

private:
  // As large as the view; drawn again when the annotations' places on the
  // view moved.
  void follow();
  // Where the annotations' first points are on the view, rounded.
  QString placesKey() const;
  // The annotation and its part (1 the text, 2 the dimension line) under a
  // point of the view; none: 0.
  int hitAt(const QPointF& at, QString* name) const;
  // The mouse on the view: true when an annotation takes it.
  bool mouse(QEvent* event);
  // The annotation under the mouse (none: empty), drawn in the hover's
  // colour (mitcad#902); logs "Annotation hovered: <name>|none".
  void setHovered(const QString& name);
  // A picture of the view under the annotations (mitcad#902), taken once
  // the camera rests or they change: Auto's ink is chosen against it.
  void scheduleBackdrop();
  void grabBackdrop();

  OcctViewer* m_viewer;
  QString m_drawnKey; // placesKey when drawn last
  // A press on an annotation, and the drag it starts.
  QString m_pressed;
  int m_part = 0;
  QPointF m_pressAt;
  QPointF m_delta;
  bool m_dragging = false;
  bool m_hovering = false; // the hand cursor is shown
  QString m_hovered;       // the annotation drawn in the hover's colour
  MeasurementBackdrop m_backdrop;
  QString m_backdropKey; // placesKey when it was taken
  bool m_backdropQueued = false;
  bool m_grabbing = false;
  // The release after a double-click or a right click on an annotation,
  // whose press the view did not see either.
  bool m_swallowRelease = false;
};

} // namespace mitcad
