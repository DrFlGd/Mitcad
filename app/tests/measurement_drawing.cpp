// SPDX-License-Identifier: MIT
// Measurement objects drawn as dimensions (mitcad#649): the `measurements`
// query's entries to what is drawn (hidden and broken ones left out), the
// values in words in the document's unit, a length's, a radius', a
// diameter's and an angle's layout through a projection and at a larger
// picture's scale, and the projection of a final render's camera.
#include "view/MeasurementDrawing.hpp"

#include <cmath>
#include <cstdio>
#include <optional>
#include <vector>

#include <QImage>
#include <QJsonArray>
#include <QJsonObject>
#include <QPainter>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "measurement_drawing.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

bool near(double a, double b, double tolerance = 1e-6) { return std::abs(a - b) <= tolerance; }

QJsonArray xyz(double x, double y, double z) { return QJsonArray{x, y, z}; }

// Looking down Z: x to the right, y up, 10 pixels per mm, the origin at
// (100, 100).
std::optional<QPointF> top(const gp_Pnt& p) { return QPointF(100.0 + 10.0 * p.X(), 100.0 - 10.0 * p.Y()); }

double lengthOf(const QPointF& p) { return std::hypot(p.x(), p.y()); }

} // namespace

int measurementDrawingTests() {
  using namespace mitcad;
  const auto words = [](const QString& kind, double value) {
    return measurementValueText(kind, value, QStringLiteral("mm"), 1.0);
  };

  // Values in words.
  CHECK(measurementValueText(QStringLiteral("distance"), 40.0, QStringLiteral("mm"), 1.0) == QStringLiteral("40 mm"));
  CHECK(measurementValueText(QStringLiteral("offset"), 12.5, QStringLiteral("cm"), lengthUnitMillimetres(QStringLiteral("cm"))) ==
        QStringLiteral("1.25 cm"));
  CHECK(measurementValueText(QStringLiteral("radius"), 25.4, QStringLiteral("in"), lengthUnitMillimetres(QStringLiteral("in"))) ==
        QStringLiteral("1 in"));
  CHECK(measurementValueText(QStringLiteral("angle"), std::acos(-1.0) / 2.0, QStringLiteral("mm"), 1.0) ==
        QStringLiteral("90°"));
  CHECK(near(lengthUnitMillimetres(QStringLiteral("ft")), 304.8));
  CHECK(near(lengthUnitMillimetres(QStringLiteral("mm")), 1.0));

  // The query's entries: hidden and broken ones are not drawn.
  const QJsonArray entries{
      QJsonObject{{QStringLiteral("name"), QStringLiteral("Distance1")},
                  {QStringLiteral("kind"), QStringLiteral("distance")},
                  {QStringLiteral("color"), QStringLiteral("#d9480f")},
                  {QStringLiteral("visible"), true},
                  {QStringLiteral("value"), 4.0},
                  {QStringLiteral("points"), QJsonArray{xyz(0, 0, 0), xyz(4, 0, 0)}}},
      QJsonObject{{QStringLiteral("name"), QStringLiteral("Hidden")},
                  {QStringLiteral("kind"), QStringLiteral("distance")},
                  {QStringLiteral("color"), QStringLiteral("#1971c2")},
                  {QStringLiteral("visible"), false},
                  {QStringLiteral("value"), 4.0},
                  {QStringLiteral("points"), QJsonArray{xyz(0, 0, 0), xyz(4, 0, 0)}}},
      QJsonObject{{QStringLiteral("name"), QStringLiteral("Broken")},
                  {QStringLiteral("kind"), QStringLiteral("offset")},
                  {QStringLiteral("color"), QStringLiteral("#2f9e44")},
                  {QStringLiteral("visible"), true},
                  {QStringLiteral("error"), QStringLiteral("a pick is not there: body F2.b0")}},
      QJsonObject{{QStringLiteral("name"), QStringLiteral("Diameter1")},
                  {QStringLiteral("kind"), QStringLiteral("diameter")},
                  {QStringLiteral("color"), QStringLiteral("#7048e8")},
                  {QStringLiteral("value"), 10.0},
                  {QStringLiteral("points"), QJsonArray{xyz(-5, 0, 0), xyz(5, 0, 0)}},
                  {QStringLiteral("center"), xyz(0, 0, 0)},
                  {QStringLiteral("axis"), xyz(0, 0, 1)},
                  {QStringLiteral("radius"), 5.0}},
      QJsonObject{{QStringLiteral("name"), QStringLiteral("Angle1")},
                  {QStringLiteral("kind"), QStringLiteral("angle")},
                  {QStringLiteral("color"), QStringLiteral("#c2255c")},
                  {QStringLiteral("value"), std::acos(-1.0) / 2.0},
                  {QStringLiteral("arc"), QJsonObject{{QStringLiteral("center"), xyz(0, 0, 0)},
                                                       {QStringLiteral("from"), xyz(1, 0, 0)},
                                                       {QStringLiteral("to"), xyz(0, 1, 0)}}}}};
  const std::vector<MeasurementDisplay> shown = measurementDisplays(entries, words);
  CHECK(shown.size() == 3);
  if (shown.size() != 3) {
    return failures;
  }
  CHECK(shown[0].label == QStringLiteral("Distance1 = 4 mm"));
  CHECK(shown[0].color == QColor(0xd9, 0x48, 0x0f));
  CHECK(shown[0].points.size() == 2);
  CHECK(shown[1].name == QStringLiteral("Diameter1") && shown[1].circle && near(shown[1].radius, 5.0));
  CHECK(shown[2].label == QStringLiteral("Angle1 = 90°") && shown[2].arc);

  // A length: extension lines from both ends, the dimension line above
  // them with an arrow at each end, the label above its middle.
  const MeasurementShape length = layoutMeasurement(shown[0], top);
  CHECK(length.valid);
  CHECK(length.lines.size() == 3);
  CHECK(length.arrows.size() == 2);
  if (length.lines.size() == 3) {
    const QLineF line = length.lines[2];
    CHECK(near(line.length(), 40.0));
    CHECK(near(line.p1().y(), line.p2().y()) && line.p1().y() < 100.0);
    CHECK(length.labelAt.y() < line.p1().y());
    CHECK(near(length.labelAt.x(), 120.0));
  }
  // Twice as large: everything twice as far.
  const auto twice = [](const gp_Pnt& p) -> std::optional<QPointF> { return *top(p) * 2.0; };
  const MeasurementShape larger = layoutMeasurement(shown[0], twice, 2.0);
  CHECK(larger.valid && larger.lines.size() == 3);
  if (larger.lines.size() == 3 && length.lines.size() == 3) {
    CHECK(near(larger.lines[2].p1().y(), 2.0 * length.lines[2].p1().y()));
  }

  // A diameter across the circle where it shows longest, arrows out.
  const MeasurementShape diameter = layoutMeasurement(shown[1], top);
  CHECK(diameter.valid);
  CHECK(diameter.arrows.size() == 2);
  bool across = false;
  for (const QLineF& line : diameter.lines) {
    across = across || near(line.length(), 100.0, 1e-6);
  }
  CHECK(across);
  // A radius from the centre to the rim.
  MeasurementDisplay radius = shown[1];
  radius.kind = QStringLiteral("radius");
  const MeasurementShape radial = layoutMeasurement(radius, top);
  CHECK(radial.valid && radial.arrows.size() == 1);
  if (radial.arrows.size() == 1) {
    CHECK(near(lengthOf(radial.arrows[0].first - QPointF(100.0, 100.0)), 50.0, 1e-6));
  }

  // An angle: an arc of 42 pixels about the corner from one ray to the
  // other, the rays drawn past it, arrows at its ends.
  const MeasurementShape angle = layoutMeasurement(shown[2], top);
  CHECK(angle.valid);
  CHECK(angle.arcs.size() == 1);
  CHECK(angle.arrows.size() == 2);
  CHECK(angle.lines.size() == 2);
  if (angle.arcs.size() == 1) {
    for (const QPointF& at : angle.arcs[0]) {
      CHECK(near(lengthOf(at - QPointF(100.0, 100.0)), 42.0, 1e-6));
    }
    CHECK(near(angle.arcs[0].first().x(), 142.0) && near(angle.arcs[0].last().y(), 58.0));
  }
  // Points that cannot be drawn: nothing.
  const MeasurementShape none = layoutMeasurement(shown[0], [](const gp_Pnt&) { return std::optional<QPointF>(); });
  CHECK(!none.valid);

  // Painting them does not fail, and draws in their colours.
  QImage image(240, 200, QImage::Format_ARGB32);
  image.fill(Qt::white);
  {
    QPainter painter(&image);
    paintMeasurements(painter, {length, diameter, angle}, QColor(250, 250, 252), measurementFont(QFont()));
  }
  bool coloured = false;
  for (int y = 0; y < image.height() && !coloured; ++y) {
    for (int x = 0; x < image.width(); ++x) {
      if (image.pixelColor(x, y) == QColor(0xd9, 0x48, 0x0f)) {
        coloured = true;
        break;
      }
    }
  }
  CHECK(coloured);

  // A final render's camera: from the front, 100 mm high (half_height 50)
  // in a 200 x 100 image, a pixel per mm.
  const QJsonObject front{{QStringLiteral("projection"), QStringLiteral("orthographic")},
                          {QStringLiteral("eye"), xyz(0, -100, 0)},
                          {QStringLiteral("target"), xyz(0, 0, 0)},
                          {QStringLiteral("up"), xyz(0, 0, 1)},
                          {QStringLiteral("half_height"), 50.0},
                          {QStringLiteral("size"), QJsonArray{200, 100}}};
  const MeasurementProjection camera = renderViewProjection(front);
  const std::optional<QPointF> middle = camera(gp_Pnt(0, 0, 0));
  CHECK(middle && near(middle->x(), 100.0) && near(middle->y(), 50.0));
  const std::optional<QPointF> high = camera(gp_Pnt(50, 30, 50));
  CHECK(high && near(high->x(), 150.0) && near(high->y(), 0.0));
  // Seen in perspective, a point behind the camera cannot be drawn, and one
  // twice as far away is half as far from the middle.
  QJsonObject perspective = front;
  perspective.insert(QStringLiteral("projection"), QStringLiteral("perspective"));
  const MeasurementProjection lens = renderViewProjection(perspective);
  CHECK(!lens(gp_Pnt(0, -200, 0)));
  const std::optional<QPointF> far = lens(gp_Pnt(0, 100, 50));
  CHECK(far && near(far->y(), 25.0));
  return failures;
}
