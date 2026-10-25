// SPDX-License-Identifier: MIT
#include "MeasurementDrawing.hpp"

#include <algorithm>
#include <cmath>
#include <initializer_list>

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
constexpr double kArcRadius = 42.0;   // an angle's arc
constexpr double kCentreMark = 4.0;   // half a circle's centre mark
constexpr double kLineWidth = 1.5;    // lines
constexpr double kHaloExtra = 3.0;    // a halo's width beyond its line's
constexpr double kLabelPixels = 12.0; // the labels' font
constexpr double kPadding = 3.0;      // around a label's text on its card
constexpr int kCircleSamples = 48;    // points tried for a radius' rim
constexpr double kPi = 3.14159265358979323846;

QPointF unitOf(const QPointF& p) {
  const double n = std::hypot(p.x(), p.y());
  return n > 1e-12 ? QPointF(p.x() / n, p.y() / n) : QPointF(1.0, 0.0);
}

double lengthOf(const QPointF& p) { return std::hypot(p.x(), p.y()); }

// The arrowhead with its tip at `tip`, pointing along `along`.
QPolygonF arrowHead(const QPointF& tip, const QPointF& along, double scale) {
  const QPointF dir = unitOf(along);
  const QPointF side(-dir.y(), dir.x());
  const double length = kArrowLength * scale;
  QPolygonF head;
  head << tip << tip - dir * length + side * (length * 0.32) << tip - dir * length - side * (length * 0.32);
  return head;
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

// A number with up to three decimals, without trailing zeros.
QString trimmed(double value) {
  QString text = QString::number(value, 'f', 3);
  while (text.contains(QLatin1Char('.')) && (text.endsWith(QLatin1Char('0')) || text.endsWith(QLatin1Char('.')))) {
    text.chop(1);
  }
  return text == QStringLiteral("-0") ? QStringLiteral("0") : text;
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
    if (!m.value(QStringLiteral("visible")).toBool(true) || m.contains(QStringLiteral("error")) ||
        !m.value(QStringLiteral("value")).isDouble()) {
      continue;
    }
    MeasurementDisplay d;
    d.name = m.value(QStringLiteral("name")).toString();
    d.kind = m.value(QStringLiteral("kind")).toString();
    d.color = QColor(m.value(QStringLiteral("color")).toString());
    if (!d.color.isValid()) {
      d.color = QColor(0xd9, 0x48, 0x0f);
    }
    const double value = m.value(QStringLiteral("value")).toDouble();
    d.label = QStringLiteral("%1 = %2").arg(d.name, valueText ? valueText(d.kind, value) : QString::number(value));
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
  // Every halo first: one never cuts another measurement's line.
  for (const MeasurementShape& shape : shapes) {
    if (!shape.valid) {
      continue;
    }
    painter.setPen(QPen(halo, width + kHaloExtra * scale, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    painter.setBrush(Qt::NoBrush);
    for (const QLineF& line : shape.lines) {
      painter.drawLine(line);
    }
    for (const QPolygonF& arc : shape.arcs) {
      painter.drawPolyline(arc);
    }
    painter.setPen(QPen(halo, kHaloExtra * scale * 0.7, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    painter.setBrush(halo);
    for (const auto& [tip, along] : shape.arrows) {
      painter.drawPolygon(arrowHead(tip, along, scale));
    }
  }
  QColor card = halo;
  card.setAlpha(225);
  for (const MeasurementShape& shape : shapes) {
    if (!shape.valid) {
      continue;
    }
    painter.setPen(QPen(shape.color, width, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
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
      painter.drawPolygon(arrowHead(tip, along, scale));
    }
    const QRectF rect = measurementLabelRect(shape, font, scale);
    painter.setBrush(card);
    painter.drawRoundedRect(rect, 3.0 * scale, 3.0 * scale);
    painter.setPen(shape.color);
    painter.drawText(rect, Qt::AlignCenter, shape.label);
  }
  painter.restore();
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
