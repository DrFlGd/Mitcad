// SPDX-License-Identifier: MIT
#include "MeasurementDrawing.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <initializer_list>
#include <iterator>
#include <limits>

#include <QFontMetricsF>
#include <QJsonValue>
#include <QPainter>
#include <QPen>
#include <QPixmap>

namespace mitcad {
namespace {

// Sizes in the view's pixels, multiplied by a picture's scale.
constexpr double kOffset = 22.0;      // a length's dimension line from its ends
constexpr double kGap = 3.0;          // an extension line from its end
constexpr double kBeyond = 5.0;       // an extension line past the dimension line
constexpr double kArrowLength = 9.0;  // an arrowhead
constexpr double kTriangle = 7.0;     // a datum label's filled triangle, base to tip
constexpr double kArcRadius = 42.0;   // an angle's arc
constexpr double kCentreMark = 4.0;   // half a circle's centre mark
constexpr double kLineWidth = 1.5;    // lines
constexpr double kHaloExtra = 3.0;    // a halo's width beyond its line's
constexpr double kLabelPixels = 12.0; // the labels' font
constexpr double kPadding = 3.0;      // around a label's text on its card
constexpr double kTextGap = 2.0;      // a dimension's text above its line, a note's from its leader
constexpr double kStackShare = 0.72;  // stacked deviations against the value's size
constexpr int kCircleSamples = 48;    // points tried for a radius' rim
constexpr double kPi = 3.14159265358979323846;
// A plane seen at most this face on (the sine of the angle to the line of
// sight) is not drawn; from the second it is drawn whole.
constexpr double kFadeFrom = 0.08;
constexpr double kFadeFull = 0.3;

QPointF unitOf(const QPointF& p) {
  const double n = std::hypot(p.x(), p.y());
  return n > 1e-12 ? QPointF(p.x() / n, p.y() / n) : QPointF(1.0, 0.0);
}

double lengthOf(const QPointF& p) { return std::hypot(p.x(), p.y()); }

double dotOf(const QPointF& a, const QPointF& b) { return a.x() * b.x() + a.y() * b.y(); }

// The arrowhead with its tip at `tip`, pointing along `along`.
QPolygonF arrowHead(const QPointF& tip, const QPointF& along, double scale) {
  const QPointF dir = unitOf(along);
  const QPointF side(-dir.y(), dir.x());
  const double length = kArrowLength * scale;
  QPolygonF head;
  head << tip << tip - dir * length + side * (length * 0.32) << tip - dir * length - side * (length * 0.32);
  return head;
}

// A datum label's filled triangle (ISO 5459): its base on the feature at
// `foot`, square to the leader coming along `along`, its tip back up the
// leader.
QPolygonF datumTriangle(const QPointF& foot, const QPointF& along, double scale) {
  const QPointF dir = unitOf(along);
  const QPointF side(-dir.y(), dir.x());
  const double height = kTriangle * scale;
  const double half = height / std::sqrt(3.0);
  QPolygonF triangle;
  triangle << foot - dir * height << foot + side * half << foot - side * half;
  return triangle;
}

// A terminator of a laid out annotation: an arrowhead, or a datum label's
// triangle.
QPolygonF terminator(const MeasurementShape& shape, const QPointF& tip, const QPointF& along, double scale) {
  return shape.datumTerminator ? datumTriangle(tip, along, scale) : arrowHead(tip, along, scale);
}

gp_Pnt pointIn(const QJsonValue& value) {
  const QJsonArray a = value.toArray();
  return gp_Pnt(a.at(0).toDouble(), a.at(1).toDouble(), a.at(2).toDouble());
}

gp_Vec vectorIn(const QJsonValue& value) {
  const QJsonArray a = value.toArray();
  return gp_Vec(a.at(0).toDouble(), a.at(1).toDouble(), a.at(2).toDouble());
}

bool isPoint(const QJsonValue& value) {
  const QJsonArray a = value.toArray();
  return a.size() == 3 && a.at(0).isDouble() && a.at(1).isDouble() && a.at(2).isDouble();
}

// A unit vector square to `axis`.
gp_Vec squareTo(const gp_Vec& axis) {
  if (axis.Magnitude() < 1e-12) {
    return gp_Vec(1.0, 0.0, 0.0);
  }
  const gp_Vec n = axis.Normalized();
  gp_Vec e(1.0, 0.0, 0.0);
  if (std::abs(n.Y()) < std::abs(n.X()) && std::abs(n.Y()) <= std::abs(n.Z())) {
    e = gp_Vec(0.0, 1.0, 0.0);
  } else if (std::abs(n.Z()) < std::abs(n.X()) && std::abs(n.Z()) < std::abs(n.Y())) {
    e = gp_Vec(0.0, 0.0, 1.0);
  }
  const gp_Vec square = e - n * e.Dot(n);
  return square.Magnitude() > 1e-12 ? square.Normalized() : gp_Vec(1.0, 0.0, 0.0);
}

// The label's middle a little off a line's middle, on the side `normal`
// points to.
QPointF besideLine(const QPointF& middle, const QPointF& normal, double scale) {
  return middle + normal * ((kLabelPixels * 0.75 + kPadding + 2.0) * scale);
}

// A unit normal of a picture's line that points up the picture (or left
// where the line is upright).
QPointF upwardNormal(const QPointF& along) {
  const QPointF dir = unitOf(along);
  QPointF n(-dir.y(), dir.x());
  if (n.y() > 1e-9 || (std::abs(n.y()) <= 1e-9 && n.x() > 0.0)) {
    n = -n;
  }
  return n;
}

void layoutLength(const MeasurementDisplay& m, const MeasurementProjection& project, double scale,
                  MeasurementShape& shape) {
  if (m.points.size() != 2) {
    return;
  }
  const std::optional<QPointF> p = project(m.points[0]);
  const std::optional<QPointF> q = project(m.points[1]);
  if (!p || !q) {
    return;
  }
  const QPointF along = *q - *p;
  const double length = lengthOf(along);
  shape.valid = true;
  if (length < 1.0) {
    // Seen end on: the label alone.
    shape.labelAt = besideLine(*p, QPointF(0.0, -1.0), scale);
    return;
  }
  const QPointF n = upwardNormal(along);
  const double off = kOffset * scale;
  const QPointF a = *p + n * off;
  const QPointF b = *q + n * off;
  shape.lines.emplace_back(*p + n * (kGap * scale), *p + n * (off + kBeyond * scale));
  shape.lines.emplace_back(*q + n * (kGap * scale), *q + n * (off + kBeyond * scale));
  shape.lines.emplace_back(a, b);
  const QPointF dir = unitOf(b - a);
  if (length >= 2.5 * kArrowLength * scale) {
    shape.arrows.emplace_back(a, -dir);
    shape.arrows.emplace_back(b, dir);
  } else {
    // Short: the arrows outside, pointing in.
    const QPointF tail = dir * (2.0 * kArrowLength * scale);
    shape.lines.emplace_back(a - tail, a);
    shape.lines.emplace_back(b, b + tail);
    shape.arrows.emplace_back(a, dir);
    shape.arrows.emplace_back(b, -dir);
  }
  shape.labelAt = besideLine((a + b) / 2.0, n, scale);
}

void layoutCircle(const MeasurementDisplay& m, const MeasurementProjection& project, double scale,
                  MeasurementShape& shape) {
  gp_Pnt centre = m.center;
  gp_Pnt rim;
  bool found = false;
  const std::optional<QPointF> c = m.circle ? project(m.center) : std::nullopt;
  if (m.circle && c && m.radius > 0.0) {
    // The point of the circle farthest from its centre on the picture: the
    // radius is drawn where it shows longest.
    const gp_Vec u = squareTo(m.axis);
    const gp_Vec v = m.axis.Magnitude() > 1e-12 ? m.axis.Normalized().Crossed(u) : gp_Vec(0.0, 1.0, 0.0);
    double best = -1.0;
    for (int i = 0; i < kCircleSamples; ++i) {
      const double t = 2.0 * kPi * i / kCircleSamples;
      const gp_Pnt point = m.center.Translated((u * std::cos(t) + v * std::sin(t)) * m.radius);
      const std::optional<QPointF> at = project(point);
      if (at && lengthOf(*at - *c) > best + 1e-9) {
        best = lengthOf(*at - *c);
        rim = point;
        found = true;
      }
    }
  } else if (m.points.size() == 2) {
    centre = m.kind == QLatin1String("diameter")
                 ? gp_Pnt((m.points[0].XYZ() + m.points[1].XYZ()) / 2.0)
                 : m.points[0];
    rim = m.points[1];
    found = true;
  }
  if (!found) {
    return;
  }
  const std::optional<QPointF> from = project(centre);
  const std::optional<QPointF> to = project(rim);
  if (!from || !to) {
    return;
  }
  shape.valid = true;
  const QPointF along = *to - *from;
  if (lengthOf(along) < 1.0) {
    shape.labelAt = besideLine(*from, QPointF(0.0, -1.0), scale);
    return;
  }
  const QPointF dir = unitOf(along);
  const double mark = kCentreMark * scale;
  shape.lines.emplace_back(*from - QPointF(mark, 0.0), *from + QPointF(mark, 0.0));
  shape.lines.emplace_back(*from - QPointF(0.0, mark), *from + QPointF(0.0, mark));
  if (m.kind == QLatin1String("diameter")) {
    const std::optional<QPointF> opposite = project(centre.Translated(gp_Vec(rim, centre)));
    if (!opposite) {
      shape.valid = false;
      return;
    }
    shape.lines.emplace_back(*opposite, *to);
    shape.arrows.emplace_back(*to, dir);
    shape.arrows.emplace_back(*opposite, -dir);
    shape.labelAt = besideLine(*from, upwardNormal(along), scale);
  } else {
    shape.lines.emplace_back(*from, *to);
    shape.arrows.emplace_back(*to, dir);
    shape.labelAt = *to + dir * (kOffset * scale) + upwardNormal(along) * (kLabelPixels * 0.5 * scale);
  }
}

void layoutAngle(const MeasurementDisplay& m, const MeasurementProjection& project, double scale,
                 MeasurementShape& shape) {
  const std::optional<QPointF> corner = project(m.corner);
  if (!m.arc || !corner) {
    return;
  }
  shape.valid = true;
  shape.labelAt = besideLine(*corner, QPointF(0.0, -1.0), scale);
  if (m.from.Magnitude() < 1e-12 || m.to.Magnitude() < 1e-12) {
    return;
  }
  const gp_Vec f = m.from.Normalized();
  const gp_Vec t = m.to.Normalized();
  const double angle = std::acos(std::clamp(f.Dot(t), -1.0, 1.0));
  if (angle < 1e-6) {
    return; // parallel: no sector
  }
  gp_Vec w = t - f * f.Dot(t);
  w = w.Magnitude() > 1e-9 ? w.Normalized() : squareTo(f);
  // The arc's radius in the design: kArcRadius pixels at the corner.
  double perMillimetre = 0.0;
  for (const gp_Vec& d : {f, w}) {
    if (const std::optional<QPointF> one = project(m.corner.Translated(d))) {
      perMillimetre = std::max(perMillimetre, lengthOf(*one - *corner));
    }
  }
  if (!(perMillimetre > 1e-9) || !std::isfinite(perMillimetre)) {
    return;
  }
  const double r = kArcRadius * scale / perMillimetre;
  const int steps = std::max(8, static_cast<int>(std::ceil(angle / (kPi / 36.0))));
  QPolygonF arc;
  for (int i = 0; i <= steps; ++i) {
    const double s = angle * i / steps;
    if (const std::optional<QPointF> at = project(m.corner.Translated((f * std::cos(s) + w * std::sin(s)) * r))) {
      arc << *at;
    }
  }
  if (arc.size() < 2) {
    return;
  }
  // The rays a little past the arc, as extension lines.
  for (const gp_Vec& ray : {f, t}) {
    if (const std::optional<QPointF> end = project(m.corner.Translated(ray * (r * 1.2)))) {
      shape.lines.emplace_back(*corner, *end);
    }
  }
  shape.arcs.push_back(arc);
  const qsizetype last = arc.size() - 1;
  shape.arrows.emplace_back(arc[0], arc[0] - arc[1]);
  shape.arrows.emplace_back(arc[last], arc[last] - arc[last - 1]);
  const gp_Vec middle = f * std::cos(angle / 2.0) + w * std::sin(angle / 2.0);
  if (const std::optional<QPointF> at = project(m.corner.Translated(middle * (r * 1.45)))) {
    shape.labelAt = *at;
  }
}

// The picture's direction of a design direction at a point: its pixels per
// millimetre along it. None where the point cannot be drawn.
std::optional<QPointF> pictureDirection(const MeasurementProjection& project, const gp_Pnt& at, const gp_Vec& along,
                                        const QPointF& here) {
  if (along.Magnitude() < 1e-12) {
    return std::nullopt;
  }
  // A small step: a perspective's projection is not linear.
  constexpr double kStep = 1e-3;
  const std::optional<QPointF> there = project(at.Translated(along.Normalized() * kStep));
  if (!there) {
    return std::nullopt;
  }
  return (*there - here) / kStep;
}

// The label's sizes in its own coordinates (x along it, y down, 0 at its
// anchor): the value's line, a stack of deviations, the text after; a
// note's lines.
struct LabelBox {
  QRectF rect;        // with the card's padding
  double mainWidth = 0.0;
  double stackWidth = 0.0;
  double bracket = 0.0; // a bracket's width
  double baseline = 0.0;
  double lineHeight = 0.0;
  std::vector<double> cells; // a feature control frame's cells' widths
};

QFont stackFont(const QFont& font) {
  QFont reduced = font;
  reduced.setPixelSize(std::max(1, static_cast<int>(std::lround(font.pixelSize() * kStackShare))));
  return reduced;
}

// A feature control frame's cell as written: the tolerance without its
// material condition's circled letter, which is drawn (circledWidth).
QString cellWords(const MeasurementShape& shape, std::size_t i) {
  QString words = shape.frameCells.at(static_cast<qsizetype>(i));
  // " Ⓜ" or " Ⓛ": a circled capital (U+24B6 to U+24CF) after a space.
  if (i == 1 && !shape.modifier.isEmpty() && words.size() >= 2 && words.back().unicode() >= 0x24B6 &&
      words.back().unicode() <= 0x24CF) {
    words.chop(2);
  }
  return words;
}

// The circle of a material condition (Ⓜ, Ⓛ) and the space before it.
double circledWidth(const MeasurementShape& shape, std::size_t i, const QFontMetricsF& metrics) {
  if (cellWords(shape, i).size() == shape.frameCells.at(static_cast<qsizetype>(i)).size()) {
    return 0.0; // none, or written as it is
  }
  return metrics.horizontalAdvance(QLatin1Char(' ')) + metrics.ascent();
}

// An ISO 1101 characteristic's symbol in lines (fonts often lack them),
// about `r` from `c` each way, in the painter's pen; arrowheads filled in
// its colour.
void paintCharacteristic(QPainter& painter, const QString& key, const QPointF& c, double r) {
  const auto at = [&c, r](double x, double y) { return c + QPointF(x * r, y * r); };
  const auto line = [&painter, &at](double x1, double y1, double x2, double y2) {
    painter.drawLine(at(x1, y1), at(x2, y2));
  };
  const auto circle = [&painter, &c](double radius) {
    painter.setBrush(Qt::NoBrush);
    painter.drawEllipse(c, radius, radius);
  };
  // A line from (x1, y1) to an arrowhead at (x2, y2).
  const auto arrow = [&painter, &at, &line, r](double x1, double y1, double x2, double y2) {
    line(x1, y1, x2, y2);
    const QPointF tip = at(x2, y2);
    const QPointF along = unitOf(at(x2, y2) - at(x1, y1));
    const QPointF side(-along.y(), along.x());
    const double length = 0.55 * r;
    QPolygonF head;
    head << tip << tip - along * length + side * (length * 0.4) << tip - along * length - side * (length * 0.4);
    painter.setBrush(painter.pen().color());
    painter.drawPolygon(head);
    painter.setBrush(Qt::NoBrush);
  };
  // 60° slopes, as ISO 1101 draws the inclined strokes.
  const double dx = 0.5;
  const double dy = 0.866;
  if (key == QLatin1String("straightness")) {
    line(-1.0, 0.0, 1.0, 0.0);
  } else if (key == QLatin1String("flatness")) {
    painter.setBrush(Qt::NoBrush);
    painter.drawPolygon(QPolygonF{at(-1.0, 0.45), at(0.55, 0.45), at(1.0, -0.45), at(-0.55, -0.45)});
  } else if (key == QLatin1String("circularity")) {
    circle(0.8 * r);
  } else if (key == QLatin1String("cylindricity")) {
    circle(0.55 * r);
    for (const double side : {-1.0, 1.0}) {
      const double ox = side * 0.55 * dy;
      const double oy = side * 0.55 * dx;
      line(ox - dx, oy + dy, ox + dx, oy - dy);
    }
  } else if (key == QLatin1String("line_profile") || key == QLatin1String("surface_profile")) {
    painter.setBrush(Qt::NoBrush);
    const QRectF bounds(at(-0.9, -0.5), at(0.9, 1.3));
    painter.drawArc(bounds, 0, 180 * 16);
    if (key == QLatin1String("surface_profile")) {
      line(-0.9, 0.4, 0.9, 0.4);
    }
  } else if (key == QLatin1String("parallelism")) {
    for (const double ox : {-0.35, 0.35}) {
      line(ox - dx, dy, ox + dx, -dy);
    }
  } else if (key == QLatin1String("perpendicularity")) {
    line(0.0, -1.0, 0.0, 0.9);
    line(-0.9, 0.9, 0.9, 0.9);
  } else if (key == QLatin1String("angularity")) {
    line(-0.9, 0.9, 0.9, 0.9);
    line(-0.9, 0.9, 0.9, 0.9 - 1.8 * 0.577);
  } else if (key == QLatin1String("position")) {
    circle(0.6 * r);
    line(-1.0, 0.0, 1.0, 0.0);
    line(0.0, -1.0, 0.0, 1.0);
  } else if (key == QLatin1String("concentricity")) {
    circle(0.85 * r);
    circle(0.45 * r);
  } else if (key == QLatin1String("symmetry")) {
    line(-1.0, 0.0, 1.0, 0.0);
    line(-0.6, -0.5, 0.6, -0.5);
    line(-0.6, 0.5, 0.6, 0.5);
  } else if (key == QLatin1String("circular_runout")) {
    arrow(-0.6, 0.8, 0.5, -0.9);
  } else if (key == QLatin1String("total_runout")) {
    arrow(-0.8, 0.8, 0.1, -0.9);
    arrow(0.0, 0.8, 0.9, -0.9);
    line(-1.0, 0.8, 0.2, 0.8);
  }
}

// Whether a characteristic's symbol is drawn in lines (one of ISO 1101's
// fourteen) rather than written.
bool drawnSymbol(const QString& key) {
  static const char* const kKeys[] = {"straightness",  "flatness",         "circularity", "cylindricity",
                                      "line_profile",  "surface_profile",  "parallelism", "perpendicularity",
                                      "angularity",    "position",         "concentricity", "symmetry",
                                      "circular_runout", "total_runout"};
  return std::any_of(std::begin(kKeys), std::end(kKeys), [&key](const char* own) { return key == QLatin1String(own); });
}

LabelBox labelBox(const MeasurementShape& shape, const QFont& font, double scale) {
  const QFontMetricsF metrics(font);
  const double pad = kPadding * scale;
  LabelBox box;
  box.lineHeight = metrics.height();
  if (!shape.frameCells.isEmpty() || !shape.datumLetter.isEmpty()) {
    // A feature control frame's cells in a row, a datum label's letter in
    // a square: the box is the frame drawn round them. At the end of the
    // leader, its middle on it; under a dimension's line, centred there.
    // The symbol's cell is a square.
    double width = 0.0;
    const double height = box.lineHeight + 2.0 * pad;
    if (!shape.frameCells.isEmpty()) {
      for (std::size_t i = 0; i < static_cast<std::size_t>(shape.frameCells.size()); ++i) {
        const double words = metrics.horizontalAdvance(cellWords(shape, i)) + circledWidth(shape, i, metrics);
        box.cells.push_back(i == 0 && drawnSymbol(shape.characteristic) ? height
                                                                        : std::max(height, words + 4.0 * pad));
        width += box.cells.back();
      }
    } else {
      width = std::max(height, metrics.horizontalAdvance(shape.datumLetter) + 2.0 * pad);
    }
    double left = shape.rightAligned ? -width : 0.0;
    double top = -height / 2.0;
    if (shape.attached) {
      left = -width / 2.0;
      top = kTextGap * scale;
    }
    box.mainWidth = width;
    box.rect = QRectF(left, top, width, height);
    box.baseline = top + pad + metrics.ascent();
    return box;
  }
  if (shape.note) {
    double width = 0.0;
    for (const QString& line : shape.noteLines) {
      width = std::max(width, metrics.horizontalAdvance(line));
    }
    const double height = box.lineHeight * static_cast<double>(std::max<qsizetype>(1, shape.noteLines.size()));
    const double gap = kTextGap * scale + pad;
    const double left = shape.rightAligned ? -gap - width : gap;
    box.mainWidth = width;
    box.rect = QRectF(left - pad, -height / 2.0 - pad, width + 2.0 * pad, height + 2.0 * pad);
    box.baseline = -height / 2.0 + metrics.ascent();
    return box;
  }
  const QFontMetricsF stacked(stackFont(font));
  const QString main = shape.main.isEmpty() && shape.upper.isEmpty() ? shape.label : shape.main;
  box.mainWidth = metrics.horizontalAdvance(main);
  double width = box.mainWidth;
  double height = metrics.height();
  if (!shape.upper.isEmpty() || !shape.lower.isEmpty()) {
    box.stackWidth = std::max(stacked.horizontalAdvance(shape.upper), stacked.horizontalAdvance(shape.lower));
    box.bracket = shape.brackets ? metrics.horizontalAdvance(QStringLiteral("(")) : 0.0;
    width += (box.mainWidth > 0.0 ? metrics.horizontalAdvance(QLatin1Char(' ')) : 0.0) + box.stackWidth +
             2.0 * box.bracket;
    height = std::max(height, 2.0 * stacked.height());
  }
  if (!shape.after.isEmpty()) {
    width += metrics.horizontalAdvance(QLatin1Char(' ') + shape.after);
  }
  // Centred on the anchor, a little above it (the dimension line).
  const double bottom = -kTextGap * scale;
  box.rect = QRectF(-width / 2.0 - pad, bottom - height - pad, width + 2.0 * pad, height + 2.0 * pad);
  box.baseline = bottom - (height - metrics.height()) / 2.0 - metrics.descent();
  return box;
}

void layoutPlanar(const MeasurementDisplay& m, const MeasurementProjection& project, double scale,
                  MeasurementShape& shape) {
  shape.planar = true;
  shape.note = m.kind == QLatin1String("note");
  shape.leader = shape.note || m.kind == QLatin1String("datum") || m.kind == QLatin1String("frame");
  shape.datumTerminator = m.datumTerminator;
  shape.attached = !m.attached.isEmpty();
  if (!m.frame.isEmpty()) {
    shape.frameCells = m.frame;
    shape.characteristic = m.characteristic;
    shape.modifier = m.modifier;
  } else if (!m.datum.isEmpty()) {
    shape.datumLetter = m.datum;
  } else if (shape.leader) {
    shape.noteLines = m.lines.isEmpty() ? QStringList{m.label} : m.lines;
  } else {
    shape.main = m.main;
    shape.upper = m.upper;
    shape.lower = m.lower;
    shape.brackets = m.brackets;
    shape.after = m.after;
  }
  const std::optional<QPointF> at = project(m.textAt);
  if (!at) {
    return;
  }
  // The text's axes on the picture, per millimetre.
  const std::optional<QPointF> ex = pictureDirection(project, m.textAt, m.textDirection, *at);
  const std::optional<QPointF> ey = pictureDirection(project, m.textAt, m.textUp, *at);
  if (!ex || !ey) {
    return;
  }
  // How face on the plane is: the smaller of the projection's two singular
  // values against the larger.
  const double a = dotOf(*ex, *ex) + dotOf(*ey, *ey);
  const double det = ex->x() * ey->y() - ex->y() * ey->x();
  const double larger = std::sqrt(std::max(0.0, (a + std::sqrt(std::max(0.0, a * a - 4.0 * det * det))) / 2.0));
  if (!(larger > 1e-12) || !std::isfinite(larger)) {
    return;
  }
  const double faceOn = std::abs(det) / (larger * larger);
  shape.opacity = std::clamp((faceOn - kFadeFrom) / (kFadeFull - kFadeFrom), 0.0, 1.0);
  shape.valid = true;
  // The text's axes, a pixel per pixel face on, and readable: never
  // mirrored (the picture's y runs down), never running right to left, and
  // upright text reads upwards.
  QPointF ux = *ex / larger;
  QPointF uy = *ey / larger;
  if (ux.x() * uy.y() - ux.y() * uy.x() > 0.0) {
    uy = -uy;
  }
  if (ux.x() < -1e-6 || (std::abs(ux.x()) <= 1e-6 && ux.y() > 0.0)) {
    ux = -ux;
    uy = -uy;
  }
  shape.textTransform = QTransform(ux.x(), ux.y(), -uy.x(), -uy.y(), at->x(), at->y());
  // A leader comes from the side of its arrow.
  if (shape.leader && !m.arrows.empty()) {
    if (const std::optional<QPointF> tip = project(m.arrows.front().tip)) {
      shape.rightAligned = dotOf(*tip - *at, ux) > 0.0;
    }
  }
  // The plane's axes per millimetre at the text, for drags.
  if (const std::optional<QPointF> px = pictureDirection(project, m.textAt, m.planeX, *at)) {
    shape.xPerMillimetre = *px;
  }
  if (const std::optional<QPointF> py = pictureDirection(project, m.textAt, m.planeY, *at)) {
    shape.yPerMillimetre = *py;
  }

  for (const MeasurementDisplay::Segment& segment : m.extensionLines) {
    const std::optional<QPointF> p = project(segment.from);
    const std::optional<QPointF> q = project(segment.to);
    if (!p || !q || lengthOf(*q - *p) < 1e-6) {
      continue;
    }
    // A gap at the measured point, a little past the dimension line.
    const QPointF u = unitOf(*q - *p);
    const double gap = std::min(kGap * scale, lengthOf(*q - *p) / 2.0);
    shape.lines.emplace_back(*p + u * gap, *q + u * (kBeyond * scale));
  }
  const bool length = !shape.leader && m.kind != QLatin1String("angle") && m.kind != QLatin1String("radius") &&
                      m.kind != QLatin1String("diameter");
  bool first = true;
  for (const MeasurementDisplay::Segment& segment : m.dimensionLines) {
    const std::optional<QPointF> p = project(segment.from);
    const std::optional<QPointF> q = project(segment.to);
    if (!p || !q) {
      continue;
    }
    shape.lines.emplace_back(*p, *q);
    // A length's dimension line itself (the first), for a drag.
    if (length && first) {
      shape.grips.push_back(QPolygonF{*p, *q});
    }
    first = false;
  }
  for (const MeasurementDisplay::Arc& arc : m.arcs) {
    if (arc.from.Magnitude() < 1e-12 || arc.to.Magnitude() < 1e-12 || arc.normal.Magnitude() < 1e-12) {
      continue;
    }
    const gp_Vec f = arc.from.Normalized();
    const gp_Vec w = arc.normal.Normalized().Crossed(f);
    const double angle = std::acos(std::clamp(f.Dot(arc.to.Normalized()), -1.0, 1.0));
    const int steps = std::max(8, static_cast<int>(std::ceil(angle / (kPi / 36.0))));
    QPolygonF points;
    for (int i = 0; i <= steps; ++i) {
      const double s = angle * i / steps;
      if (const std::optional<QPointF> p =
              project(arc.center.Translated((f * std::cos(s) + w * std::sin(s)) * arc.radius))) {
        points << *p;
      }
    }
    if (points.size() >= 2) {
      shape.arcs.push_back(points);
      shape.grips.push_back(points);
    }
  }
  for (const MeasurementDisplay::Arrow& arrow : m.arrows) {
    const std::optional<QPointF> tip = project(arrow.tip);
    if (!tip) {
      continue;
    }
    const std::optional<QPointF> along = pictureDirection(project, arrow.tip, arrow.direction, *tip);
    if (along && lengthOf(*along) > 1e-9) {
      shape.arrows.emplace_back(*tip, *along);
    }
  }
  // A length too short on the picture for its arrows inside: outside,
  // pointing in, with a tail each.
  if (length && shape.arrows.size() == 2 && !shape.grips.empty()) {
    const QPointF from = shape.grips.front().first();
    const QPointF to = shape.grips.front().last();
    if (lengthOf(to - from) < 2.5 * kArrowLength * scale && lengthOf(to - from) > 1e-6) {
      const QPointF dir = unitOf(to - from);
      const QPointF tail = dir * (2.0 * kArrowLength * scale);
      shape.lines.emplace_back(from - tail, from);
      shape.lines.emplace_back(to, to + tail);
      shape.arrows = {{from, dir}, {to, -dir}};
    }
  }
  if (m.centerMark) {
    if (const std::optional<QPointF> c = project(*m.centerMark)) {
      const double mark = kCentreMark * scale;
      shape.lines.emplace_back(*c - QPointF(mark, 0.0), *c + QPointF(mark, 0.0));
      shape.lines.emplace_back(*c - QPointF(0.0, mark), *c + QPointF(0.0, mark));
    }
  }
  // Where the text is, for clicks and the tests' log; the label's middle.
  const QRectF box = labelBox(shape, measurementFont(QFont(), scale), scale).rect;
  shape.textArea = shape.textTransform.map(QPolygonF(box));
  shape.labelAt = shape.textTransform.map(box.center());
}

// The deviation on the picture of a drag `delta` across a shape's line
// (along its plane's y axis).
QPointF acrossLine(const MeasurementShape& shape, const QPointF& delta) {
  const QPointF y = unitOf(shape.yPerMillimetre);
  return y * dotOf(delta, y);
}

void paintPlanarLabel(QPainter& painter, const MeasurementShape& shape, const QFont& font, double scale,
                      const QColor& card) {
  const LabelBox box = labelBox(shape, font, scale);
  painter.save();
  painter.setTransform(shape.textTransform, true);
  painter.setPen(Qt::NoPen);
  painter.setBrush(card);
  painter.drawRoundedRect(box.rect, 3.0 * scale, 3.0 * scale);
  painter.setPen(shape.color);
  painter.setFont(font);
  const double pad = kPadding * scale;
  if (!box.cells.empty() || !shape.datumLetter.isEmpty()) {
    // The frame's outline and the lines between its cells, each cell's
    // text centred in it; a datum label's letter in its square.
    painter.setPen(QPen(shape.color, (shape.highlighted ? 1.6 : 1.0) * kLineWidth * scale));
    painter.setBrush(Qt::NoBrush);
    painter.drawRect(box.rect);
    const QFontMetricsF metrics(font);
    if (box.cells.empty()) {
      const double width = metrics.horizontalAdvance(shape.datumLetter);
      painter.drawText(QPointF(box.rect.center().x() - width / 2.0, box.baseline), shape.datumLetter);
      painter.restore();
      return;
    }
    double x = box.rect.left();
    const double middle = box.rect.center().y();
    for (std::size_t i = 0; i < box.cells.size(); ++i) {
      if (i > 0) {
        painter.drawLine(QPointF(x, box.rect.top()), QPointF(x, box.rect.bottom()));
      }
      if (i == 0 && drawnSymbol(shape.characteristic)) {
        // The symbol in lines, in the middle of its square cell.
        const double r = 0.5 * box.lineHeight * 0.75;
        paintCharacteristic(painter, shape.characteristic, QPointF(x + box.cells[i] / 2.0, middle), r);
        x += box.cells[i];
        continue;
      }
      const QString words = cellWords(shape, i);
      const double circled = circledWidth(shape, i, metrics);
      const double width = metrics.horizontalAdvance(words) + circled;
      const double left = x + (box.cells[i] - width) / 2.0;
      painter.drawText(QPointF(left, box.baseline), words);
      if (circled > 0.0) {
        // Ⓜ or Ⓛ: the letter in a circle after the tolerance.
        const double diameter = metrics.ascent();
        const QPointF centre(left + width - diameter / 2.0, box.baseline - metrics.ascent() / 2.0 + metrics.descent() / 2.0);
        painter.setBrush(Qt::NoBrush);
        painter.drawEllipse(centre, diameter / 2.0, diameter / 2.0);
        const QFont reduced = stackFont(font);
        const QFontMetricsF reducedMetrics(reduced);
        painter.setFont(reduced);
        const double letter = reducedMetrics.horizontalAdvance(shape.modifier);
        painter.drawText(QPointF(centre.x() - letter / 2.0, centre.y() + reducedMetrics.ascent() / 2.0 - reducedMetrics.descent() / 2.0),
                         shape.modifier);
        painter.setFont(font);
      }
      x += box.cells[i];
    }
    painter.restore();
    return;
  }
  if (shape.note) {
    double y = box.baseline;
    for (const QString& line : shape.noteLines) {
      painter.drawText(QPointF(box.rect.left() + pad, y), line);
      y += box.lineHeight;
    }
    painter.restore();
    return;
  }
  const QFontMetricsF metrics(font);
  double x = box.rect.left() + pad;
  const QString main = shape.main.isEmpty() && shape.upper.isEmpty() ? shape.label : shape.main;
  painter.drawText(QPointF(x, box.baseline), main);
  x += box.mainWidth;
  if (!shape.upper.isEmpty() || !shape.lower.isEmpty()) {
    if (box.mainWidth > 0.0) {
      x += metrics.horizontalAdvance(QLatin1Char(' '));
    }
    if (shape.brackets) {
      painter.drawText(QPointF(x, box.baseline), QStringLiteral("("));
      x += box.bracket;
    }
    const QFont reduced = stackFont(font);
    const QFontMetricsF smallMetrics(reduced);
    painter.setFont(reduced);
    // The upper deviation above the lower, both beside the value.
    const double middle = box.baseline - metrics.ascent() / 2.0 + metrics.descent() / 2.0;
    painter.drawText(QPointF(x, middle - smallMetrics.descent() - 0.5 * scale), shape.upper);
    painter.drawText(QPointF(x, middle + smallMetrics.ascent() + 0.5 * scale), shape.lower);
    painter.setFont(font);
    x += box.stackWidth;
    if (shape.brackets) {
      painter.drawText(QPointF(x, box.baseline), QStringLiteral(")"));
      x += box.bracket;
    }
  }
  if (!shape.after.isEmpty()) {
    painter.drawText(QPointF(x, box.baseline), QLatin1Char(' ') + shape.after);
  }
  painter.restore();
}

// A number with up to three decimals, without trailing zeros.
QString trimmed(double value) {
  QString text = QString::number(value, 'f', 3);
  while (text.contains(QLatin1Char('.')) && (text.endsWith(QLatin1Char('0')) || text.endsWith(QLatin1Char('.')))) {
    text.chop(1);
  }
  return text == QStringLiteral("-0") ? QStringLiteral("0") : text;
}

double distanceToPolyline(const QPolygonF& line, const QPointF& at) {
  double best = std::numeric_limits<double>::infinity();
  for (qsizetype i = 0; i + 1 < line.size(); ++i) {
    const QPointF a = line[i];
    const QPointF b = line[i + 1];
    const QPointF ab = b - a;
    const double length = dotOf(ab, ab);
    const double t = length > 1e-12 ? std::clamp(dotOf(at - a, ab) / length, 0.0, 1.0) : 0.0;
    best = std::min(best, lengthOf(at - (a + ab * t)));
  }
  return best;
}

} // namespace

double lengthUnitMillimetres(const QString& unit) {
  if (unit == QLatin1String("cm")) {
    return 10.0;
  }
  if (unit == QLatin1String("m")) {
    return 1000.0;
  }
  if (unit == QLatin1String("in")) {
    return 25.4;
  }
  if (unit == QLatin1String("ft")) {
    return 304.8;
  }
  return 1.0;
}

QString measurementValueText(const QString& kind, double value, const QString& unit, double unitMillimetres) {
  if (kind == QLatin1String("angle")) {
    return trimmed(value * 180.0 / kPi) + QStringLiteral("°");
  }
  const double per = unitMillimetres > 0.0 ? unitMillimetres : 1.0;
  return QStringLiteral("%1 %2").arg(trimmed(value / per), unit.isEmpty() ? QStringLiteral("mm") : unit);
}

QIcon measurementSwatch(const QColor& color) {
  QPixmap pixmap(16, 16);
  pixmap.fill(Qt::transparent);
  QPainter painter(&pixmap);
  painter.setRenderHint(QPainter::Antialiasing);
  // A dimension: a line with an arrowhead at each end, in the colour.
  const QColor ink = color.isValid() ? color : QColor(0xd9, 0x48, 0x0f);
  painter.setPen(QPen(ink, 1.6));
  painter.drawLine(QPointF(2.5, 9.0), QPointF(13.5, 9.0));
  painter.drawLine(QPointF(1.5, 4.0), QPointF(1.5, 14.0));
  painter.drawLine(QPointF(14.5, 4.0), QPointF(14.5, 14.0));
  painter.setPen(Qt::NoPen);
  painter.setBrush(ink);
  painter.drawPolygon(arrowHead(QPointF(2.0, 9.0), QPointF(-1.0, 0.0), 0.5));
  painter.drawPolygon(arrowHead(QPointF(14.0, 9.0), QPointF(1.0, 0.0), 0.5));
  painter.end();
  return QIcon(pixmap);
}

std::vector<MeasurementDisplay> measurementDisplays(const QJsonArray& measurements,
                                                    const MeasurementValueText& valueText) {
  std::vector<MeasurementDisplay> shown;
  for (const QJsonValue& entry : measurements) {
    const QJsonObject m = entry.toObject();
    const QJsonObject drawing = m.value(QStringLiteral("drawing")).toObject();
    const QString kindOf = m.value(QStringLiteral("kind")).toString();
    // A leader (a note, datum label, feature control frame) has no value.
    const bool leader = kindOf == QLatin1String("note") || kindOf == QLatin1String("datum") ||
                        kindOf == QLatin1String("frame");
    if (!m.value(QStringLiteral("visible")).toBool(true) || m.contains(QStringLiteral("error")) ||
        (!m.value(QStringLiteral("value")).isDouble() && !(leader && !drawing.isEmpty()))) {
      continue;
    }
    MeasurementDisplay d;
    d.name = m.value(QStringLiteral("name")).toString();
    d.kind = m.value(QStringLiteral("kind")).toString();
    d.group = m.value(QStringLiteral("group")).toString();
    d.color = QColor(m.value(QStringLiteral("color")).toString());
    if (!d.color.isValid()) {
      d.color = QColor(0xd9, 0x48, 0x0f);
    }
    const double value = m.value(QStringLiteral("value")).toDouble();
    const QJsonObject label = m.value(QStringLiteral("label")).toObject();
    if (label.contains(QStringLiteral("text"))) {
      d.label = label.value(QStringLiteral("text")).toString();
      d.main = label.value(QStringLiteral("main")).toString();
      d.upper = label.value(QStringLiteral("upper")).toString();
      d.lower = label.value(QStringLiteral("lower")).toString();
      d.brackets = label.value(QStringLiteral("brackets")).toBool();
      d.after = label.value(QStringLiteral("after")).toString();
      for (const QJsonValue& line : label.value(QStringLiteral("lines")).toArray()) {
        d.lines << line.toString();
      }
      for (const QJsonValue& cell : label.value(QStringLiteral("frame")).toArray()) {
        d.frame << cell.toString();
      }
      d.datum = label.value(QStringLiteral("datum")).toString();
      const QJsonObject frameOf = m.value(QStringLiteral("frame")).toObject();
      d.characteristic = frameOf.value(QStringLiteral("characteristic")).toString();
      d.modifier = frameOf.value(QStringLiteral("modifier")).toString();
    } else {
      d.label = QStringLiteral("%1 = %2").arg(d.name, valueText ? valueText(d.kind, value) : QString::number(value));
    }
    const QJsonArray points = m.value(QStringLiteral("points")).toArray();
    if (points.size() == 2 && isPoint(points.at(0)) && isPoint(points.at(1))) {
      d.points = {pointIn(points.at(0)), pointIn(points.at(1))};
    }
    if (isPoint(m.value(QStringLiteral("center"))) && m.value(QStringLiteral("radius")).isDouble()) {
      d.circle = true;
      d.center = pointIn(m.value(QStringLiteral("center")));
      d.axis = vectorIn(m.value(QStringLiteral("axis")));
      d.radius = m.value(QStringLiteral("radius")).toDouble();
    }
    const QJsonObject arc = m.value(QStringLiteral("arc")).toObject();
    if (isPoint(arc.value(QStringLiteral("center")))) {
      d.arc = true;
      d.corner = pointIn(arc.value(QStringLiteral("center")));
      d.from = vectorIn(arc.value(QStringLiteral("from")));
      d.to = vectorIn(arc.value(QStringLiteral("to")));
    }
    // The model's drawing in the annotation's plane (mitcad#702).
    const QJsonObject text = drawing.value(QStringLiteral("text")).toObject();
    if (isPoint(text.value(QStringLiteral("at")))) {
      d.planar = true;
      const QJsonObject plane = drawing.value(QStringLiteral("plane")).toObject();
      d.planeX = vectorIn(plane.value(QStringLiteral("x")));
      d.planeY = vectorIn(plane.value(QStringLiteral("y")));
      d.planeNormal = vectorIn(plane.value(QStringLiteral("normal")));
      const auto segments = [&drawing](const char* key) {
        std::vector<MeasurementDisplay::Segment> out;
        for (const QJsonValue& entryOf : drawing.value(QLatin1String(key)).toArray()) {
          const QJsonArray pair = entryOf.toArray();
          if (pair.size() == 2 && isPoint(pair.at(0)) && isPoint(pair.at(1))) {
            out.push_back({pointIn(pair.at(0)), pointIn(pair.at(1))});
          }
        }
        return out;
      };
      d.extensionLines = segments("extension_lines");
      d.dimensionLines = segments("dimension_lines");
      for (const QJsonValue& arcEntry : drawing.value(QStringLiteral("arcs")).toArray()) {
        const QJsonObject a = arcEntry.toObject();
        d.arcs.push_back({pointIn(a.value(QStringLiteral("center"))), a.value(QStringLiteral("radius")).toDouble(),
                          vectorIn(a.value(QStringLiteral("from"))), vectorIn(a.value(QStringLiteral("to"))),
                          vectorIn(a.value(QStringLiteral("normal")))});
      }
      for (const QJsonValue& arrowEntry : drawing.value(QStringLiteral("arrows")).toArray()) {
        const QJsonObject a = arrowEntry.toObject();
        d.arrows.push_back({pointIn(a.value(QStringLiteral("tip"))), vectorIn(a.value(QStringLiteral("direction")))});
      }
      if (isPoint(drawing.value(QStringLiteral("center")))) {
        d.centerMark = pointIn(drawing.value(QStringLiteral("center")));
      }
      d.textAt = pointIn(text.value(QStringLiteral("at")));
      d.textDirection = vectorIn(text.value(QStringLiteral("direction")));
      d.textUp = vectorIn(text.value(QStringLiteral("up")));
      d.lineOffset = drawing.value(QStringLiteral("line_offset")).toDouble();
      const QJsonArray offset = drawing.value(QStringLiteral("text_offset")).toArray();
      d.textOffset = {offset.at(0).toDouble(), offset.at(1).toDouble()};
      d.datumTerminator = drawing.value(QStringLiteral("terminator")).toString() == QLatin1String("datum");
      d.attached = drawing.value(QStringLiteral("attached")).toString();
    }
    shown.push_back(d);
  }
  return shown;
}

MeasurementShape layoutMeasurement(const MeasurementDisplay& measurement, const MeasurementProjection& project,
                                   double scale) {
  MeasurementShape shape;
  shape.name = measurement.name;
  shape.color = measurement.color;
  shape.label = measurement.label;
  if (!project) {
    return shape;
  }
  if (measurement.planar) {
    layoutPlanar(measurement, project, scale, shape);
    return shape;
  }
  if (measurement.kind == QLatin1String("angle")) {
    layoutAngle(measurement, project, scale, shape);
  } else if (measurement.kind == QLatin1String("radius") || measurement.kind == QLatin1String("diameter")) {
    layoutCircle(measurement, project, scale, shape);
  } else {
    layoutLength(measurement, project, scale, shape);
  }
  return shape;
}

QFont measurementFont(const QFont& base, double scale) {
  QFont font = base;
  font.setPixelSize(std::max(1, static_cast<int>(std::lround(kLabelPixels * scale))));
  font.setBold(true);
  return font;
}

QRectF measurementLabelRect(const MeasurementShape& shape, const QFont& font, double scale) {
  if (shape.planar) {
    return labelBox(shape, font, scale).rect;
  }
  const QFontMetricsF metrics(font);
  const double w = metrics.horizontalAdvance(shape.label) + 2.0 * kPadding * scale;
  const double h = metrics.height() + kPadding * scale;
  return QRectF(shape.labelAt.x() - w / 2.0, shape.labelAt.y() - h / 2.0, w, h);
}

void paintMeasurements(QPainter& painter, const std::vector<MeasurementShape>& shapes, const QColor& halo,
                       const QFont& font, double scale) {
  painter.save();
  painter.setRenderHint(QPainter::Antialiasing);
  painter.setFont(font);
  const double width = kLineWidth * scale;
  const auto drawn = [](const MeasurementShape& shape) { return shape.valid && shape.opacity > 0.0; };
  // Every halo first: one never cuts another annotation's line. A
  // highlighted one has a wider halo of the selection's colour.
  for (const MeasurementShape& shape : shapes) {
    if (!drawn(shape)) {
      continue;
    }
    painter.setOpacity(shape.opacity);
    const QColor tone = shape.highlighted ? QColor(0x4d, 0xab, 0xf7) : halo;
    const double extra = (shape.highlighted ? 2.0 : 1.0) * kHaloExtra * scale;
    painter.setPen(QPen(tone, width + extra, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    painter.setBrush(Qt::NoBrush);
    for (const QLineF& line : shape.lines) {
      painter.drawLine(line);
    }
    for (const QPolygonF& arc : shape.arcs) {
      painter.drawPolyline(arc);
    }
    painter.setPen(QPen(tone, extra * 0.7, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    painter.setBrush(tone);
    for (const auto& [tip, along] : shape.arrows) {
      painter.drawPolygon(terminator(shape, tip, along, scale));
    }
  }
  QColor card = halo;
  card.setAlpha(225);
  for (const MeasurementShape& shape : shapes) {
    if (!drawn(shape)) {
      continue;
    }
    painter.setOpacity(shape.opacity);
    const double lineWidth = shape.highlighted ? width * 1.6 : width;
    painter.setPen(QPen(shape.color, lineWidth, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    painter.setBrush(Qt::NoBrush);
    for (const QLineF& line : shape.lines) {
      painter.drawLine(line);
    }
    for (const QPolygonF& arc : shape.arcs) {
      painter.drawPolyline(arc);
    }
    painter.setPen(Qt::NoPen);
    painter.setBrush(shape.color);
    for (const auto& [tip, along] : shape.arrows) {
      painter.drawPolygon(terminator(shape, tip, along, scale));
    }
    if (shape.planar) {
      QColor cardOf = card;
      if (shape.highlighted) {
        cardOf = QColor(0x4d, 0xab, 0xf7);
        cardOf.setAlpha(90);
      }
      paintPlanarLabel(painter, shape, font, scale, cardOf);
      continue;
    }
    const QRectF rect = measurementLabelRect(shape, font, scale);
    painter.setBrush(card);
    painter.drawRoundedRect(rect, 3.0 * scale, 3.0 * scale);
    painter.setPen(shape.color);
    painter.drawText(rect, Qt::AlignCenter, shape.label);
  }
  painter.restore();
}

std::optional<std::array<double, 2>> measurementDragMillimetres(const MeasurementShape& shape, const QPointF& delta) {
  // delta = a x + b y with x and y the plane's axes per millimetre.
  const QPointF x = shape.xPerMillimetre;
  const QPointF y = shape.yPerMillimetre;
  const double det = x.x() * y.y() - x.y() * y.x();
  // A frame under a dimension stands where that dimension's text is.
  if (!shape.planar || shape.attached || !(std::abs(det) > 1e-12) || !std::isfinite(det)) {
    return std::nullopt;
  }
  const double a = (delta.x() * y.y() - delta.y() * y.x()) / det;
  const double b = (x.x() * delta.y() - x.y() * delta.x()) / det;
  return std::array<double, 2>{a, b};
}

MeasurementShape draggedMeasurement(const MeasurementShape& shape, bool line, const QPointF& delta) {
  MeasurementShape moved = shape;
  if (shape.attached) {
    return moved;
  }
  if (!line) {
    moved.textTransform = shape.textTransform * QTransform::fromTranslate(delta.x(), delta.y());
    moved.textArea = shape.textArea.translated(delta);
    moved.labelAt = shape.labelAt + delta;
    return moved;
  }
  // Across the line only: the dimension line, its arrows and the text move,
  // the extension lines' far ends with them.
  const QPointF shift = acrossLine(shape, delta);
  for (QPolygonF& grip : moved.grips) {
    grip.translate(shift);
  }
  for (QPolygonF& arc : moved.arcs) {
    arc.translate(shift);
  }
  for (auto& arrow : moved.arrows) {
    arrow.first += shift;
  }
  for (QLineF& drawnLine : moved.lines) {
    // Lines that touch the dimension line follow at that end.
    for (const QPolygonF& grip : shape.grips) {
      if (distanceToPolyline(grip, drawnLine.p2()) < 1.0) {
        drawnLine.setP2(drawnLine.p2() + shift);
      }
      if (distanceToPolyline(grip, drawnLine.p1()) < 1.0) {
        drawnLine.setP1(drawnLine.p1() + shift);
      }
    }
  }
  moved.textTransform = shape.textTransform * QTransform::fromTranslate(shift.x(), shift.y());
  moved.textArea = shape.textArea.translated(shift);
  moved.labelAt = shape.labelAt + shift;
  return moved;
}

int measurementHit(const MeasurementShape& shape, const QPointF& at, double reach) {
  // Nothing of an annotation seen nearly edge on takes clicks.
  if (!shape.valid || !shape.planar || shape.opacity < 0.5) {
    return 0;
  }
  if (shape.textArea.containsPoint(at, Qt::OddEvenFill)) {
    return 1;
  }
  for (const QPolygonF& grip : shape.grips) {
    if (distanceToPolyline(grip, at) <= reach) {
      return 2;
    }
  }
  return 0;
}

MeasurementProjection renderViewProjection(const QJsonObject& view) {
  const gp_Pnt eye = pointIn(view.value(QStringLiteral("eye")));
  const gp_Pnt target = pointIn(view.value(QStringLiteral("target")));
  const gp_Vec up = vectorIn(view.value(QStringLiteral("up")));
  const double half = view.value(QStringLiteral("half_height")).toDouble();
  const QJsonArray size = view.value(QStringLiteral("size")).toArray();
  const double width = size.at(0).toDouble();
  const double height = size.at(1).toDouble();
  const bool perspective = view.value(QStringLiteral("projection")).toString() == QLatin1String("perspective");
  gp_Vec forward(eye, target);
  const double distance = forward.Magnitude();
  if (!(distance > 1e-12) || !(half > 0.0) || !(height > 0.0)) {
    return [](const gp_Pnt&) -> std::optional<QPointF> { return std::nullopt; };
  }
  forward /= distance;
  gp_Vec right = forward.Crossed(up);
  if (right.Magnitude() < 1e-12) {
    return [](const gp_Pnt&) -> std::optional<QPointF> { return std::nullopt; };
  }
  right.Normalize();
  const gp_Vec trueUp = right.Crossed(forward);
  const double pixels = height / (2.0 * half); // per mm at the target
  return [=](const gp_Pnt& point) -> std::optional<QPointF> {
    const gp_Vec d(eye, point);
    double x = d.Dot(right);
    double y = d.Dot(trueUp);
    if (perspective) {
      const double depth = d.Dot(forward);
      if (depth <= 1e-9) {
        return std::nullopt;
      }
      x *= distance / depth;
      y *= distance / depth;
    }
    return QPointF(width / 2.0 + x * pixels, height / 2.0 - y * pixels);
  };
}

} // namespace mitcad
