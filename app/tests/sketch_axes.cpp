// SPDX-License-Identifier: MIT
// The sketch's origin and axes as picks (mitcad#500): the pick ids, the
// fixed line and point a tool's view of the sketch gets for each the sketch
// lacks (once, however often it is asked), and the entities the model made
// for them found again instead.
#include "sketch/SketchModel.hpp"

#include <cmath>
#include <cstdio>

#include <QString>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "sketch_axes.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using namespace mitcad::sketch;

const PointData* pointOf(const SketchModel& model, const QString& id) {
  for (const PointData& p : model.points) {
    if (p.id == id) {
      return &p;
    }
  }
  return nullptr;
}

const CurveData* curveOf(const SketchModel& model, const QString& id) {
  for (const CurveData& c : model.curves) {
    if (c.id == id) {
      return &c;
    }
  }
  return nullptr;
}

void pickIdTests() {
  CHECK(originPick() == QStringLiteral("origin"));
  CHECK(axisPick(QStringLiteral("x")) == QStringLiteral("x_axis"));
  CHECK(axisPick(QStringLiteral("y")) == QStringLiteral("y_axis"));
  CHECK(isAxisPick(QStringLiteral("origin")) && isAxisPick(QStringLiteral("x_axis")) &&
        isAxisPick(QStringLiteral("y_axis")));
  CHECK(!isAxisPick(QStringLiteral("c3")) && !isAxisPick(QString()) && !isAxisPick(QStringLiteral("x")));
  CHECK(axisPickName(QStringLiteral("origin")) == QStringLiteral("origin"));
  CHECK(axisPickName(QStringLiteral("x_axis")) == QStringLiteral("x"));
  CHECK(axisPickName(QStringLiteral("y_axis")) == QStringLiteral("y"));
  CHECK(axisPickName(QStringLiteral("p1")).isEmpty());
}

void emptySketchTests() {
  // A new sketch: one free point, nothing at the origin.
  SketchModel model;
  PointData free;
  free.id = QStringLiteral("p1");
  free.at = V2{12.0, 20.0};
  model.points.push_back(free);
  CHECK(model.originPoint().isEmpty());
  CHECK(model.axisLine(QStringLiteral("x")).isEmpty());
  CHECK(model.axisOf(QStringLiteral("x_axis")) == QStringLiteral("x"));
  CHECK(model.isAxisOrOrigin(QStringLiteral("origin")));
  CHECK(!model.isAxisOrOrigin(QStringLiteral("p1")) && !model.isAxisOrOrigin(QString()));

  model.addAxisPicks();
  // The origin as a fixed point, each axis as a fixed construction line
  // from it along the axis with its far end.
  CHECK(model.points.size() == 4 && model.curves.size() == 2);
  const PointData* origin = pointOf(model, QStringLiteral("origin"));
  CHECK(origin != nullptr && origin->fixed && norm(origin->at) < 1e-12 && origin->axis.isEmpty());
  CHECK(model.originPoint() == QStringLiteral("origin"));
  const CurveData* x = curveOf(model, QStringLiteral("x_axis"));
  const CurveData* y = curveOf(model, QStringLiteral("y_axis"));
  CHECK(x != nullptr && y != nullptr);
  if (x != nullptr && y != nullptr) {
    CHECK(x->isLine() && x->construction && x->fixed && x->axis == QStringLiteral("x"));
    CHECK(x->start == QStringLiteral("origin") && y->start == QStringLiteral("origin"));
    CHECK(dist(x->curve.a, V2{}) < 1e-12 && dist(x->curve.b, V2{kAxisLineLength, 0.0}) < 1e-12);
    CHECK(dist(y->curve.b, V2{0.0, kAxisLineLength}) < 1e-12);
    // A point's distance from the line, as a tool measures it.
    CHECK(std::abs(x->curve.distanceTo(V2{30.0, 30.0}) - 30.0) < 1e-9);
    const PointData* far = pointOf(model, x->end);
    CHECK(far != nullptr && far->axis == QStringLiteral("x") && far->fixed);
  }
  CHECK(model.axisLine(QStringLiteral("x")) == QStringLiteral("x_axis"));
  CHECK(model.axisOf(QStringLiteral("y_axis")) == QStringLiteral("y"));
  CHECK(model.axisOf(QStringLiteral("p1")).isEmpty());
  CHECK(model.isAxisOrOrigin(QStringLiteral("x_axis")) && model.isAxisOrOrigin(QStringLiteral("origin")));
  // The far ends are not the origin.
  CHECK(!model.isAxisOrOrigin(QStringLiteral("p1")));

  // Asked again: nothing more.
  model.addAxisPicks();
  CHECK(model.points.size() == 4 && model.curves.size() == 2);
}

void madeAxesTests() {
  // The model made the origin point p1 (a snap, mitcad#355) and the x axis
  // c2 with its far end p3; the y axis is still to be made.
  SketchModel model;
  PointData origin;
  origin.id = QStringLiteral("p1");
  origin.fixed = true;
  model.points.push_back(origin);
  PointData far;
  far.id = QStringLiteral("p3");
  far.at = V2{kAxisLineLength, 0.0};
  far.fixed = true;
  far.axis = QStringLiteral("x");
  model.points.push_back(far);
  CurveData line;
  line.id = QStringLiteral("c2");
  line.type = QStringLiteral("line");
  line.curve = Curve2::line(V2{}, far.at);
  line.start = QStringLiteral("p1");
  line.end = QStringLiteral("p3");
  line.points << line.start << line.end;
  line.construction = true;
  line.fixed = true;
  line.axis = QStringLiteral("x");
  model.curves.push_back(line);
  // A fixed point of the user's elsewhere.
  PointData fixed;
  fixed.id = QStringLiteral("p4");
  fixed.at = V2{5.0, 5.0};
  fixed.fixed = true;
  model.points.push_back(fixed);

  CHECK(model.originPoint() == QStringLiteral("p1"));
  CHECK(model.axisLine(QStringLiteral("x")) == QStringLiteral("c2"));
  CHECK(model.axisLine(QStringLiteral("y")).isEmpty());
  CHECK(model.axisOf(QStringLiteral("c2")) == QStringLiteral("x"));
  CHECK(model.isAxisOrOrigin(QStringLiteral("p1")) && model.isAxisOrOrigin(QStringLiteral("c2")));
  CHECK(!model.isAxisOrOrigin(QStringLiteral("p4")));

  model.addAxisPicks();
  // Only the y axis, from the origin point there is.
  CHECK(model.points.size() == 4 && model.curves.size() == 2);
  CHECK(pointOf(model, QStringLiteral("origin")) == nullptr);
  CHECK(curveOf(model, QStringLiteral("x_axis")) == nullptr);
  const CurveData* y = curveOf(model, QStringLiteral("y_axis"));
  CHECK(y != nullptr && y->start == QStringLiteral("p1") && y->axis == QStringLiteral("y"));
  CHECK(model.axisLine(QStringLiteral("x")) == QStringLiteral("c2"));
}

} // namespace

int sketchAxesTests() {
  failures = 0;
  pickIdTests();
  emptySketchTests();
  madeAxesTests();
  return failures;
}
