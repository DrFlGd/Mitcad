// SPDX-License-Identifier: MIT
#pragma once

// Measurement objects drawn as dimensions (mitcad#649, app/COMMANDS.md
// "Measurement objects"): the shown entries of the model's `measurements`
// query, laid out in a picture's pixels through a projection of the
// design's points, and painted with QPainter: extension lines, the
// dimension line or an angle's arc with arrows, and the label "<name> =
// <value>", all in the measurement's colour over a halo in the view's halo
// tone (mitcad#633), so that they read over light and dark faces. The view
// (MeasurementOverlay), its snapshots (OcctViewer::snapshot) and Render
// Image draw them the same way; pixel sizes grow with a picture's scale.
// Plain Qt Gui and OCCT's points: no model, no widgets.

#include <functional>
#include <optional>
#include <utility>
#include <vector>

#include <QColor>
#include <QFont>
#include <QIcon>
#include <QJsonArray>
#include <QJsonObject>
#include <QLineF>
#include <QPointF>
#include <QPolygonF>
#include <QRectF>
#include <QString>

#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>

class QPainter;

namespace mitcad {

// A measurement as the model measured it at the marker, in the design's
// coordinates (commands.md, "Measurement objects").
struct MeasurementDisplay {
  QString name;
  QString kind; // distance, angle, offset, axis_distance, center_distance, radius, diameter
  QColor color;
  QString label; // "Distance1 = 40 mm"
  // A length's two ends; a radius from the centre to the rim, a diameter
  // across the circle.
  std::vector<gp_Pnt> points;
  // A circle (radius, diameter): its centre, unit normal and radius.
  bool circle = false;
  gp_Pnt center;
  gp_Vec axis;
  double radius = 0.0;
  // An angle: its corner and the unit rays of its sector.
  bool arc = false;
  gp_Pnt corner;
  gp_Vec from;
  gp_Vec to;
};

// A value of a kind in words: "40 mm", "90°".
using MeasurementValueText = std::function<QString(const QString& kind, double value)>;

// Millimetres per document length unit (mm, cm, m, in, ft; 1 for others).
double lengthUnitMillimetres(const QString& unit);
// A value of a kind (mm or radians) in words: a length in `unit` (of
// `unitMillimetres` mm), up to three decimals, "40 mm"; an angle in
// degrees, "90°".
QString measurementValueText(const QString& kind, double value, const QString& unit, double unitMillimetres);
// A small swatch of a measurement's colour, the browser's row icon.
QIcon measurementSwatch(const QColor& color);

// The measurements of the `measurements` query to draw: those shown that
// have a value (hidden and broken ones left out), each labelled "<name> =
// <value>".
std::vector<MeasurementDisplay> measurementDisplays(const QJsonArray& measurements,
                                                    const MeasurementValueText& valueText);

// A point of the design in a picture's pixels; none where it cannot be
// drawn (behind a perspective camera).
using MeasurementProjection = std::function<std::optional<QPointF>(const gp_Pnt&)>;

// A measurement laid out in a picture's pixels.
struct MeasurementShape {
  QString name;
  QColor color;
  std::vector<QLineF> lines;                       // extension and dimension lines, rays
  std::vector<QPolygonF> arcs;                     // an angle's arc
  std::vector<std::pair<QPointF, QPointF>> arrows; // the tip and the direction it points along
  QPointF labelAt;                                 // the label's middle
  QString label;
  bool valid = false;
};

// Lays out a measurement through `project`: a length's dimension line
// parallel to its ends a little off them (above them on the picture) with
// extension lines from the ends, a radius from the centre to the point of
// the circle farthest from it on the picture, a diameter across the circle
// there, an angle's arc about its corner between its rays. `scale` is the
// picture's size against the view's (gaps, arrows, the arc's radius and
// the label grow with it). Not valid when its points cannot be drawn.
MeasurementShape layoutMeasurement(const MeasurementDisplay& measurement, const MeasurementProjection& project,
                                   double scale = 1.0);

// The labels' font at `scale`: `base`'s family, bold, of a fixed pixel
// size.
QFont measurementFont(const QFont& base, double scale = 1.0);
// Where a laid out measurement's label is drawn, in `font`.
QRectF measurementLabelRect(const MeasurementShape& shape, const QFont& font, double scale = 1.0);

// Paints laid out measurements: every halo first (in `halo`), so that a
// halo never cuts another measurement's line, then each one's lines,
// arrows and label in its colour, the label on a card of the halo's tone.
void paintMeasurements(QPainter& painter, const std::vector<MeasurementShape>& shapes, const QColor& halo,
                       const QFont& font, double scale = 1.0);

// The projection of a final render's camera (render::finalView's "view":
// projection, eye, target, up, half_height, size), for Render Image's
// measurements: half the image's height spans `half_height` at the target.
MeasurementProjection renderViewProjection(const QJsonObject& view);

} // namespace mitcad
