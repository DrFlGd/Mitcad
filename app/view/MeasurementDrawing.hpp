// SPDX-License-Identifier: MIT
#pragma once

// Annotations drawn in the view (mitcad#702, grown from the measurement
// objects of mitcad#649; app/COMMANDS.md "Annotations"): the shown entries
// of the model's `annotations` query, laid out in a picture's pixels
// through a projection of the design's points, and painted with QPainter.
//
// An entry with the model's `drawing` (every annotation now) is drawn in
// its plane: its extension and dimension lines, an angle's arc, the arrows
// and a leader's (a note, a datum label's triangle, a feature control
// frame's) projected from the design, and its text set in its plane
// (foreshortened as the plane turns away, turned round where it would read
// upside down or mirrored, fading as the plane turns edge-on), ISO 129
// style: the value, a tolerance's deviations stacked after it, the own
// text after them; a feature control frame's cells ISO 1101, a datum
// label's letter in its box ISO 5459. An entry without one (the measurement objects'
// first form) keeps the screen-facing layout: the dimension line above its
// ends, the label "<name> = <value>" on a card.
//
// Everything is in the annotation's colour over a halo in the view's halo
// tone (mitcad#633), so that it reads over light and dark faces. The view
// (MeasurementOverlay), its snapshots (OcctViewer::snapshot) and Render
// Image draw them the same way; pixel sizes grow with a picture's scale.
// Plain Qt Gui and OCCT's points: no model, no widgets.

#include <array>
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
#include <QStringList>
#include <QTransform>

#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>

class QPainter;

namespace mitcad {

// An annotation as the model measured it at the marker, in the design's
// coordinates (commands.md, "Measurement objects" and "Annotations").
struct MeasurementDisplay {
  QString name;
  // distance, angle, offset, axis_distance, center_distance, radius,
  // diameter; the leaders note, datum, frame
  QString kind;
  QColor color;
  QString group;
  QString label; // on one line: "Ø20 H7 (+0.021/0)"; "Distance1 = 40 mm" without the model's label
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

  // The model's drawing in the annotation's plane (mitcad#702).
  struct Segment {
    gp_Pnt from;
    gp_Pnt to;
  };
  struct Arc {
    gp_Pnt center;
    double radius = 0.0;
    gp_Vec from; // unit rays it runs between
    gp_Vec to;
    gp_Vec normal;
  };
  struct Arrow {
    gp_Pnt tip;
    gp_Vec direction; // the way it points
  };
  bool planar = false;
  gp_Vec planeX; // the axes text_offset is along (unit)
  gp_Vec planeY;
  gp_Vec planeNormal;
  std::vector<Segment> extensionLines; // from a measured point to the dimension line
  std::vector<Segment> dimensionLines; // and a leader
  std::vector<Arc> arcs;
  std::vector<Arrow> arrows;
  std::optional<gp_Pnt> centerMark;
  gp_Pnt textAt;
  gp_Vec textDirection;
  gp_Vec textUp;
  double lineOffset = 0.0;                 // in use: a length's line, an angle's arc radius
  std::array<double, 2> textOffset{0, 0}; // in use, along planeX and planeY
  // The label in parts (ISO 129): the value with its prefix and own text
  // before it; deviations stacked after it (a fit's in brackets); the own
  // text after them. A note's lines.
  QString main;
  QString upper;
  QString lower;
  bool brackets = false;
  QString after;
  QStringList lines;
  // A feature control frame's cells (ISO 1101): the characteristic's
  // symbol, the tolerance, the datums referred to.
  QStringList frame;
  // Its characteristic (`perpendicularity`) and material condition (M,
  // L), whose symbols are drawn as lines, not taken from the font.
  QString characteristic;
  QString modifier;
  // A datum label's letter (ISO 5459); its leader ends in a filled
  // triangle on the feature.
  QString datum;
  bool datumTerminator = false;
  // A frame under a dimension: that dimension's name; the frame is set
  // just below its dimension line, at its text.
  QString attached;
};

// A value of a kind in words: "40 mm", "90°".
using MeasurementValueText = std::function<QString(const QString& kind, double value)>;

// Millimetres per document length unit (mm, cm, m, in, ft; 1 for others).
double lengthUnitMillimetres(const QString& unit);
// A value of a kind (mm or radians) in words: a length in `unit` (of
// `unitMillimetres` mm), up to three decimals, "40 mm"; an angle in
// degrees, "90°".
QString measurementValueText(const QString& kind, double value, const QString& unit, double unitMillimetres);
// A small swatch of an annotation's colour, a row's icon.
QIcon measurementSwatch(const QColor& color);

// The annotations of the `annotations` query to draw: those shown that the
// model laid out (hidden and broken ones left out), each with its label
// (the model's, else "<name> = <value>").
std::vector<MeasurementDisplay> measurementDisplays(const QJsonArray& measurements,
                                                    const MeasurementValueText& valueText);

// A point of the design in a picture's pixels; none where it cannot be
// drawn (behind a perspective camera).
using MeasurementProjection = std::function<std::optional<QPointF>(const gp_Pnt&)>;

// An annotation laid out in a picture's pixels.
struct MeasurementShape {
  QString name;
  QColor color;
  std::vector<QLineF> lines;                       // extension and dimension lines, rays, leaders
  std::vector<QPolygonF> arcs;                     // an angle's arc
  std::vector<std::pair<QPointF, QPointF>> arrows; // the tip and the direction it points along
  QPointF labelAt;                                 // the label's middle
  QString label;
  bool valid = false;

  // Drawn in its plane (mitcad#702).
  bool planar = false;
  bool note = false;
  // A note, datum label or feature control frame: its text at the end of
  // its leader.
  bool leader = false;
  // The text's own coordinates (x along it, y down; 0 at its anchor) to
  // the picture's.
  QTransform textTransform;
  // A note's text ends at its anchor (its leader comes from the right)
  // rather than starting there.
  bool rightAligned = false;
  // 1 face on, 0 edge on: everything is drawn this opaque.
  double opacity = 1.0;
  QString main;
  QString upper;
  QString lower;
  bool brackets = false;
  QString after;
  QStringList noteLines; // a note's text
  QStringList frameCells; // a feature control frame's cells
  QString characteristic; // its symbol drawn (the first cell)
  QString modifier;       // M or L: circled after the tolerance
  QString datumLetter;    // a datum label's letter
  // The leader ends in a filled triangle (a datum label), not an arrow.
  bool datumTerminator = false;
  // A frame set below a dimension's line: drawn there, not dragged.
  bool attached = false;
  // Where the text is on the picture, for clicks.
  QPolygonF textArea;
  // The lines a drag of the dimension line takes (a length's line, an
  // angle's arc); none for a circle or a note.
  std::vector<QPolygonF> grips;
  // The plane's axes (planeX, planeY) per millimetre on the picture at the
  // text: a drag in pixels into millimetres.
  QPointF xPerMillimetre;
  QPointF yPerMillimetre;
  // Drawn bolder, with the selection's halo (a row selected in the
  // Annotations window).
  bool highlighted = false;
};

// Lays out an annotation through `project`. With the model's drawing: its
// lines, arcs and arrows projected (arrows of a length too short for them
// put outside, pointing in), its text in its plane. Without: a length's
// dimension line parallel to its ends a little off them (above them on the
// picture) with extension lines from the ends, a radius from the centre to
// the point of the circle farthest from it on the picture, a diameter
// across the circle there, an angle's arc about its corner between its
// rays. `scale` is the picture's size against the view's (gaps, arrows,
// the arc's radius and the text grow with it). Not valid when its points
// cannot be drawn.
MeasurementShape layoutMeasurement(const MeasurementDisplay& measurement, const MeasurementProjection& project,
                                   double scale = 1.0);

// The labels' font at `scale`: `base`'s family, bold, of a fixed pixel
// size.
QFont measurementFont(const QFont& base, double scale = 1.0);
// Where a laid out annotation's label is drawn, in `font`: on the picture
// for a screen-facing one, in the text's own coordinates for one in its
// plane.
QRectF measurementLabelRect(const MeasurementShape& shape, const QFont& font, double scale = 1.0);

// Paints laid out annotations: every halo first (in `halo`), so that a
// halo never cuts another annotation's line, then each one's lines, arrows
// and label in its colour, the label on a card of the halo's tone.
void paintMeasurements(QPainter& painter, const std::vector<MeasurementShape>& shapes, const QColor& halo,
                       const QFont& font, double scale = 1.0);

// A drag of an annotation's text (`line` false) or dimension line by
// `delta` pixels on the picture, in millimetres along the plane's axes
// (planeX, planeY); none where the plane is seen edge on.
std::optional<std::array<double, 2>> measurementDragMillimetres(const MeasurementShape& shape, const QPointF& delta);
// The shape as it shows while its text or its line is dragged by `delta`
// pixels: the text moved, or the dimension line, its arrows and the text
// moved across it.
MeasurementShape draggedMeasurement(const MeasurementShape& shape, bool line, const QPointF& delta);
// What of the annotation is at a point of the picture: 1 its text, 2 its
// dimension line, 0 nothing (within `reach` pixels of a line).
int measurementHit(const MeasurementShape& shape, const QPointF& at, double reach = 5.0);

// The projection of a final render's camera (render::finalView's "view":
// projection, eye, target, up, half_height, size), for Render Image's
// annotations: half the image's height spans `half_height` at the target.
MeasurementProjection renderViewProjection(const QJsonObject& view);

} // namespace mitcad
