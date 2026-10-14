// SPDX-License-Identifier: MIT
// The sketch snapper's body targets (mitcad#258): body vertices and the
// middles of body edges in the sketch plane are found within the pixel
// tolerances when a tool's switch gives them, never without it, and the
// sketch's own geometry wins ties against them.
#include "sketch/SketchSnap.hpp"

#include <cmath>
#include <cstdio>
#include <utility>

#include <QJsonArray>
#include <QJsonObject>
#include <QPointF>
#include <QString>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "sketch_snap.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using namespace mitcad::sketch;

// 10 pixels per millimetre, y up.
QPointF screenOf(const V2& p) { return QPointF(p.x * 10.0, -p.y * 10.0); }

bool sameAt(const V2& a, const V2& b) { return dist(a, b) <= 1e-9; }

// Lines are all this test needs of the model's curve JSON.
Curve2 readLine(const QString& type, const QJsonObject& geometry) {
  const auto at = [&geometry](const char* key) {
    const QJsonArray xy = geometry.value(QLatin1String(key)).toArray();
    return V2{xy.at(0).toDouble(), xy.at(1).toDouble()};
  };
  return type == QStringLiteral("line") ? Curve2::line(at("start"), at("end")) : Curve2();
}

QJsonObject vertex(const char* source, double x, double y) {
  return {{QStringLiteral("source"), QLatin1String(source)},
          {QStringLiteral("body"), QStringLiteral("F2.b0")},
          {QStringLiteral("kind"), QStringLiteral("vertex")},
          {QStringLiteral("geometry"), QJsonObject{{QStringLiteral("at"), QJsonArray{x, y}}}}};
}

QJsonObject line(const char* source, double x0, double y0, double x1, double y1) {
  return {{QStringLiteral("source"), QLatin1String(source)},
          {QStringLiteral("body"), QStringLiteral("F2.b0")},
          {QStringLiteral("kind"), QStringLiteral("line")},
          {QStringLiteral("geometry"),
           QJsonObject{{QStringLiteral("start"), QJsonArray{x0, y0}}, {QStringLiteral("end"), QJsonArray{x1, y1}}}}};
}

Snap snapAt(const SnapSettings& settings, const V2& at) { return snap(settings, at, screenOf(at)); }

void bodyTargetTests() {
  // The sketch: a point at the origin's right (10, 0) and a line from
  // (0, 20) to (10, 20). The bodies: corners at (10, 0) (the sketch point's
  // place), (30, 0) and (0, 50) (the origin's other side), a line from
  // (30, 0) to (30, 10) and one from (0, 20) to (10, 20) (the sketch line's
  // place).
  SketchModel model;
  PointData point;
  point.id = QStringLiteral("p1");
  point.at = V2{10.0, 0.0};
  model.points.push_back(point);
  const std::pair<const char*, V2> ends[] = {{"p2", V2{0.0, 20.0}}, {"p3", V2{10.0, 20.0}}};
  for (const auto& [id, at] : ends) {
    PointData end;
    end.id = QLatin1String(id);
    end.at = at;
    model.points.push_back(end);
  }
  CurveData sketchLine;
  sketchLine.id = QStringLiteral("c4");
  sketchLine.type = QStringLiteral("line");
  sketchLine.curve = Curve2::line(V2{0.0, 20.0}, V2{10.0, 20.0});
  sketchLine.start = QStringLiteral("p2");
  sketchLine.end = QStringLiteral("p3");
  sketchLine.points << sketchLine.start << sketchLine.end;
  model.curves.push_back(sketchLine);

  BodyGeometry body;
  body.load(QJsonArray{vertex("V{a}", 10.0, 0.0), vertex("V{b}", 30.0, 0.0), vertex("V{c}", 0.0, 50.0),
                       line("E{d}", 30.0, 0.0, 30.0, 10.0), line("E{e}", 0.0, 20.0, 10.0, 20.0)},
            &readLine);
  CHECK(body.items().size() == 5);
  CHECK(body.vertexCount() == 3 && body.edgeCount() == 2);

  SnapSettings settings;
  settings.model = &model;
  settings.toScreen = screenOf;
  settings.grid = false;

  // The switch off (other tools): no body targets.
  Snap s = snapAt(settings, V2{30.3, 0.0});
  CHECK(s.kind == Snap::Kind::None);

  settings.body = &body;
  // A body corner 3 pixels away.
  s = snapAt(settings, V2{30.3, 0.0});
  CHECK(s.kind == Snap::Kind::BodyVertex);
  CHECK(sameAt(s.at, V2{30.0, 0.0}));
  CHECK(s.bodyItem.source == QStringLiteral("V{b}"));
  CHECK(s.bodyItem.body == QStringLiteral("F2.b0"));
  CHECK(s.onGeometry() && !s.atPoint());
  CHECK(s.describe() == QStringLiteral("body vertex V{b}"));
  // 9 pixels away: within the point tolerance (10); 12 pixels: not.
  CHECK(snapAt(settings, V2{30.0, 0.9}).kind == Snap::Kind::BodyVertex);
  s = snapAt(settings, V2{0.0, 51.2});
  CHECK(s.kind == Snap::Kind::None);
  CHECK(sameAt(s.at, V2{0.0, 51.2}));
  // The middle of a body line, 2 pixels away, rather than the line's end
  // 5 mm away.
  s = snapAt(settings, V2{30.2, 5.0});
  CHECK(s.kind == Snap::Kind::BodyMidpoint);
  CHECK(sameAt(s.at, V2{30.0, 5.0}));
  CHECK(s.bodyItem.source == QStringLiteral("E{d}"));
  CHECK(s.bodyItem.type == QStringLiteral("line"));
  CHECK(s.describe() == QStringLiteral("midpoint of body edge E{d}"));
  // 7 pixels from the middle (within 8); 9 pixels: neither the middle nor
  // a corner, and a body edge is no curve to snap onto.
  CHECK(snapAt(settings, V2{30.7, 5.0}).kind == Snap::Kind::BodyMidpoint);
  CHECK(snapAt(settings, V2{30.9, 5.0}).kind == Snap::Kind::None);

  // Ties: the sketch's point at the body corner's place wins, also when
  // the cursor is a little nearer the corner; the sketch line's middle wins
  // over the body line's at the same place.
  s = snapAt(settings, V2{10.0, 0.0});
  CHECK(s.kind == Snap::Kind::Point);
  CHECK(s.point == QStringLiteral("p1"));
  s = snapAt(settings, V2{10.05, 0.0});
  CHECK(s.kind == Snap::Kind::Point);
  s = snapAt(settings, V2{5.0, 20.1});
  CHECK(s.kind == Snap::Kind::Midpoint);
  CHECK(s.curve == QStringLiteral("c4"));
  // A body corner still wins over a sketch point clearly further away.
  PointData beside;
  beside.id = QStringLiteral("p5");
  beside.at = V2{30.8, 0.0};
  model.points.push_back(beside);
  s = snapAt(settings, V2{30.0, 0.0});
  CHECK(s.kind == Snap::Kind::BodyVertex);
  s = snapAt(settings, V2{30.8, 0.0});
  CHECK(s.kind == Snap::Kind::Point && s.point == QStringLiteral("p5"));

  // A body item's ids: the item, its points, and nothing else.
  CHECK(BodyGeometry::isBodyId(QStringLiteral("body:3")) && !BodyGeometry::isBodyId(QStringLiteral("p3")));
  CHECK(body.pointAt(QStringLiteral("body:1")).has_value());
  CHECK(sameAt(*body.pointAt(QStringLiteral("body:3:end")), V2{30.0, 10.0}));
  CHECK(body.curve(QStringLiteral("body:3")) != nullptr);
  CHECK(body.curve(QStringLiteral("body:1")) == nullptr);
  CHECK(!body.pointAt(QStringLiteral("body:9")).has_value());
  CHECK(!body.pointAt(QStringLiteral("body:1:start")).has_value());
}

} // namespace

int sketchSnapTests() {
  failures = 0;
  bodyTargetTests();
  return failures;
}
