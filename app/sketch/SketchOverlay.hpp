// SPDX-License-Identifier: MIT
#pragma once

// What sketch mode draws over the 3D view: constraint glyphs and
// dimensions (both selectable), the active tool's preview, the snap marker,
// the sketch's axes and origin, the pointer's place and the degrees of
// freedom. Drawn with QPainter in view pixels, so text
// and glyphs keep their size at any zoom; the widget lets the mouse
// through to the view, and the controller asks it what lies under the
// cursor.

#include <functional>
#include <optional>
#include <utility>
#include <vector>

#include <QLineF>
#include <QPointF>
#include <QPolygonF>
#include <QRectF>
#include <QString>
#include <QWidget>

#include "SketchModel.hpp"

class QPainter;

namespace mitcad::sketch {

class SketchController;
struct ToolPreview;

// A dimension as drawn, in view pixels.
struct DimensionShape {
  std::vector<QLineF> lines;              // extension and dimension lines
  std::vector<QPolygonF> arcs;            // angle and arc length arcs
  std::vector<std::pair<QPointF, QPointF>> arrows; // tip and the direction it points to
  QPointF textAt;
  QString text;
  bool valid = false;
};

// Maps sketch coordinates to the view.
struct Projection {
  std::function<QPointF(const V2&)> toScreen;
  double pixel = 0.1; // mm per pixel
};

// Lays out a dimension of a sketch (a placed one, or one being placed).
DimensionShape dimensionShape(const DimensionData& dimension, const SketchModel& model,
                              const Projection& projection, const QString& text);
// The default place of a dimension's value.
V2 defaultTextPosition(const DimensionData& dimension, const SketchModel& model, double pixel);
// What a dimension measures on the current geometry (mm or radians), when
// it can be measured.
std::optional<double> measure(const DimensionData& dimension, const SketchModel& model);

// An angle dimension's corner, where its lines meet, and the unit rays from
// it that bound the sector it measures (mitcad#296): the lines' directions
// (start to end), reversed where its `reverse_a` and `reverse_b` say. One
// stored without them measures the angle closer to its value: with
// `byValue` the sector of that angle nearer to its text, else the
// directions. None for parallel lines.
struct AngleRays {
  V2 corner, da, db;
};
std::optional<AngleRays> angleRays(const DimensionData& dimension, const SketchModel& model, bool byValue = true);

class SketchOverlay : public QWidget {
  Q_OBJECT

public:
  struct Hit {
    enum class Kind { None, Constraint, Fixed, Dimension };
    Kind kind = Kind::None;
    QString id; // k3; for Fixed the entity
    bool valid() const { return kind != Kind::None; }
  };

  SketchOverlay(SketchController& controller, QWidget* view);

  Hit hitTest(const QPointF& position) const;
  // The dimensions' values and constraint glyphs a window holds (view
  // pixels): wholly inside it, or with `crossing` also those it touches
  // (mitcad#217). Only those shown.
  std::vector<Hit> hitsIn(const QRectF& area, bool crossing) const;
  // Where a dimension's value is drawn.
  QRectF dimensionTextRect(const QString& id) const;
  // The text a dimension shows: its value, "fx:" for an expression,
  // parentheses when driven.
  QString dimensionText(const DimensionData& dimension) const;
  // A dimension's tooltip: its name, its expression, its value in the
  // document's unit and its comment ("d3 = d1 / 2 → 20 mm (Sketch1
  // distance)", mitcad#231).
  QString dimensionTooltip(const DimensionData& dimension) const;

protected:
  void paintEvent(QPaintEvent* event) override;
  void hideEvent(QHideEvent* event) override;

private:
  struct Glyph {
    QString constraint; // k3, or empty for a fixed entity
    QString entity;     // what it is drawn at
    QString type;       // constraint type, or "fix"
    QRectF rect;
  };
  struct DimensionLayout {
    QString id;
    DimensionShape shape;
    QRectF textRect;
  };

  Projection projection() const;
  std::vector<Glyph> glyphs() const;
  std::vector<DimensionLayout> dimensions() const;
  QRectF textRect(const QPointF& at, const QString& text) const;
  // The sketch's x and y axes across the view, its origin, and the design
  // origin's place when the sketch's origin is elsewhere (mitcad#254).
  void paintAxes(QPainter& painter) const;
  void paintGlyph(QPainter& painter, const Glyph& glyph, bool selected, bool hovered) const;
  // `color`: the value's text; `lines`: the dimension and extension lines
  // and arrows, light on a dark background.
  void paintDimension(QPainter& painter, const DimensionShape& shape, const QRectF& textRect,
                      const QColor& color, const QColor& lines, bool driven) const;
  void paintPreview(QPainter& painter, const ToolPreview& preview) const;
  void paintSnap(QPainter& painter) const;
  void paintEntityHighlight(QPainter& painter, const QString& entity, const QColor& color) const;
  // An equal spacing's anchors joined in order and numbered (mitcad#221).
  void paintChain(QPainter& painter, const QStringList& anchors, const QColor& color) const;
  bool isSelected(const QString& kind, const QString& id) const;

  SketchController& m_c;
};

} // namespace mitcad::sketch
