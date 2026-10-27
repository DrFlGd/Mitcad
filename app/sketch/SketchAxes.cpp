// SPDX-License-Identifier: MIT
// The sketch's origin and its x and y axes as picks of the tools that
// constrain and dimension (mitcad#500): the entities the model makes for
// them (`sketch.axis`, core/sketch/src/axes.rs), and the pick ids
// that stand for them until a step makes them. Only the sketch model's
// data: the app's unit tests take this file alone.
#include "SketchModel.hpp"

namespace mitcad::sketch {

QString SketchModel::originPoint() const {
  for (const PointData& p : points) {
    if (p.fixed && p.axis.isEmpty() && norm(p.at) < 1e-9) {
      return p.id;
    }
  }
  return QString();
}

QString SketchModel::axisLine(const QString& axis) const {
  for (const CurveData& c : curves) {
    if (c.axis == axis && c.isLine()) {
      return c.id;
    }
  }
  return QString();
}

QString SketchModel::axisOf(const QString& id) const {
  for (const CurveData& c : curves) {
    if (c.id == id) {
      return c.axis;
    }
  }
  if (id == axisPick(QStringLiteral("x"))) {
    return QStringLiteral("x");
  }
  if (id == axisPick(QStringLiteral("y"))) {
    return QStringLiteral("y");
  }
  return QString();
}

bool SketchModel::isAxisOrOrigin(const QString& id) const {
  if (id.isEmpty()) {
    return false;
  }
  return id == originPick() || !axisOf(id).isEmpty() || id == originPoint();
}

void SketchModel::addAxisPicks() {
  QString origin = originPoint();
  if (origin.isEmpty()) {
    PointData p;
    p.id = originPick();
    p.fixed = true;
    p.constrained = true;
    points.push_back(p);
    origin = p.id;
  }
  for (const QString& axis : {QStringLiteral("x"), QStringLiteral("y")}) {
    if (!axisLine(axis).isEmpty()) {
      continue;
    }
    const V2 end = axis == QStringLiteral("x") ? V2{kAxisLineLength, 0.0} : V2{0.0, kAxisLineLength};
    CurveData c;
    c.id = axisPick(axis);
    c.type = QStringLiteral("line");
    c.curve = Curve2::line(V2{}, end);
    c.start = origin;
    c.end = c.id + QStringLiteral(":end");
    c.points << c.start << c.end;
    c.construction = true;
    c.fixed = true;
    c.constrained = true;
    c.axis = axis;
    PointData far;
    far.id = c.end;
    far.at = end;
    far.fixed = true;
    far.constrained = true;
    far.axis = axis;
    points.push_back(far);
    curves.push_back(c);
  }
}

QString originPick() { return QStringLiteral("origin"); }

QString axisPick(const QString& axis) { return axis + QStringLiteral("_axis"); }

bool isAxisPick(const QString& id) { return !axisPickName(id).isEmpty(); }

QString axisPickName(const QString& id) {
  if (id == originPick()) {
    return QStringLiteral("origin");
  }
  if (id == axisPick(QStringLiteral("x"))) {
    return QStringLiteral("x");
  }
  if (id == axisPick(QStringLiteral("y"))) {
    return QStringLiteral("y");
  }
  return QString();
}

} // namespace mitcad::sketch
