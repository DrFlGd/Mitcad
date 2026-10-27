// SPDX-License-Identifier: MIT
// Measurement objects drawn as dimensions (mitcad#649): the `measurements`
// query's entries to what is drawn (hidden and broken ones left out), the
// values in words in the document's unit, a length's, a radius', a
// diameter's and an angle's layout through a projection and at a larger
// picture's scale, and the projection of a final render's camera.
#include "view/MeasurementDrawing.hpp"

#include <array>
#include <cmath>
#include <cstdio>
#include <initializer_list>
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

// A list of one segment. MSVC takes QJsonArray{QJsonArray{p, q}} for a copy of
// the inner array (one element in braces), not a list holding it.
QJsonArray oneSegment(const QJsonArray& p, const QJsonArray& q) {
  QJsonArray list;
  list.append(QJsonArray{p, q});
  return list;
}

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

  // Annotations in their planes (mitcad#702): a length of 40 along X in
  // the plane square to Z, its dimension line 10 off along Y, the text at
  // its middle running along X; the model's label.
  const QJsonObject planar{
      {QStringLiteral("name"), QStringLiteral("Distance2")},
      {QStringLiteral("kind"), QStringLiteral("distance")},
      {QStringLiteral("color"), QStringLiteral("#2f9e44")},
      {QStringLiteral("group"), QStringLiteral("Shop")},
      {QStringLiteral("value"), 40.0},
      {QStringLiteral("points"), QJsonArray{xyz(0, 0, 0), xyz(40, 0, 0)}},
      {QStringLiteral("label"), QJsonObject{{QStringLiteral("text"), QStringLiteral("40 +0.2/-0.1")},
                                            {QStringLiteral("main"), QStringLiteral("40")},
                                            {QStringLiteral("upper"), QStringLiteral("+0.2")},
                                            {QStringLiteral("lower"), QStringLiteral("-0.1")},
                                            {QStringLiteral("brackets"), false}}},
      {QStringLiteral("drawing"),
       QJsonObject{
           {QStringLiteral("plane"), QJsonObject{{QStringLiteral("origin"), xyz(0, 0, 0)},
                                                 {QStringLiteral("x"), xyz(1, 0, 0)},
                                                 {QStringLiteral("y"), xyz(0, 1, 0)},
                                                 {QStringLiteral("normal"), xyz(0, 0, 1)}}},
           {QStringLiteral("line_offset"), 10.0},
           {QStringLiteral("text_offset"), QJsonArray{0.0, 0.0}},
           {QStringLiteral("extension_lines"),
            QJsonArray{QJsonArray{xyz(0, 0, 0), xyz(0, 10, 0)}, QJsonArray{xyz(40, 0, 0), xyz(40, 10, 0)}}},
           {QStringLiteral("dimension_lines"), oneSegment(xyz(0, 10, 0), xyz(40, 10, 0))},
           {QStringLiteral("arcs"), QJsonArray{}},
           {QStringLiteral("arrows"),
            QJsonArray{QJsonObject{{QStringLiteral("tip"), xyz(0, 10, 0)}, {QStringLiteral("direction"), xyz(-1, 0, 0)}},
                       QJsonObject{{QStringLiteral("tip"), xyz(40, 10, 0)}, {QStringLiteral("direction"), xyz(1, 0, 0)}}}},
           {QStringLiteral("text"), QJsonObject{{QStringLiteral("at"), xyz(20, 10, 0)},
                                                {QStringLiteral("direction"), xyz(1, 0, 0)},
                                                {QStringLiteral("up"), xyz(0, 1, 0)}}}}}};
  const QJsonObject note{
      {QStringLiteral("name"), QStringLiteral("Note1")},
      {QStringLiteral("kind"), QStringLiteral("note")},
      {QStringLiteral("color"), QStringLiteral("#7048e8")},
      {QStringLiteral("text"), QStringLiteral("Deburr\nall edges")},
      {QStringLiteral("label"), QJsonObject{{QStringLiteral("text"), QStringLiteral("Deburr / all edges")},
                                            {QStringLiteral("lines"), QJsonArray{QStringLiteral("Deburr"),
                                                                                 QStringLiteral("all edges")}}}},
      {QStringLiteral("drawing"),
       QJsonObject{
           {QStringLiteral("plane"), QJsonObject{{QStringLiteral("origin"), xyz(0, 0, 0)},
                                                 {QStringLiteral("x"), xyz(1, 0, 0)},
                                                 {QStringLiteral("y"), xyz(0, 1, 0)},
                                                 {QStringLiteral("normal"), xyz(0, 0, 1)}}},
           {QStringLiteral("line_offset"), 0.0},
           {QStringLiteral("text_offset"), QJsonArray{10.0, 10.0}},
           {QStringLiteral("dimension_lines"), oneSegment(xyz(10, 10, 0), xyz(0, 0, 0))},
           {QStringLiteral("arrows"),
            QJsonArray{QJsonObject{{QStringLiteral("tip"), xyz(0, 0, 0)}, {QStringLiteral("direction"), xyz(-1, -1, 0)}}}},
           {QStringLiteral("text"), QJsonObject{{QStringLiteral("at"), xyz(10, 10, 0)},
                                                {QStringLiteral("direction"), xyz(1, 0, 0)},
                                                {QStringLiteral("up"), xyz(0, 1, 0)}}}}}};
  const std::vector<MeasurementDisplay> drawn = measurementDisplays(QJsonArray{planar, note}, words);
  CHECK(drawn.size() == 2);
  if (drawn.size() != 2) {
    return failures;
  }
  CHECK(drawn[0].planar && drawn[0].label == QStringLiteral("40 +0.2/-0.1") && drawn[0].group == QStringLiteral("Shop"));
  CHECK(drawn[0].upper == QStringLiteral("+0.2") && drawn[0].lower == QStringLiteral("-0.1"));
  CHECK(drawn[0].extensionLines.size() == 2 && drawn[0].dimensionLines.size() == 1 && drawn[0].arrows.size() == 2);
  CHECK(near(drawn[0].lineOffset, 10.0));
  CHECK(drawn[1].planar && drawn[1].lines.size() == 2);

  // Face on: drawn whole, the text along X, upright, a pixel per pixel.
  const MeasurementShape face = layoutMeasurement(drawn[0], top);
  CHECK(face.valid && face.planar && near(face.opacity, 1.0));
  CHECK(face.lines.size() == 3 && face.arrows.size() == 2 && face.grips.size() == 1);
  const QPointF origin = face.textTransform.map(QPointF(0.0, 0.0));
  CHECK(near(origin.x(), 300.0, 1e-6) && near(origin.y(), 0.0, 1e-6));
  const QPointF along = face.textTransform.map(QPointF(1.0, 0.0)) - origin;
  const QPointF down = face.textTransform.map(QPointF(0.0, 1.0)) - origin;
  CHECK(near(along.x(), 1.0, 1e-6) && near(along.y(), 0.0, 1e-6));
  CHECK(near(down.x(), 0.0, 1e-6) && near(down.y(), 1.0, 1e-6));
  // The text above the line takes clicks; the line away from it too.
  CHECK(measurementHit(face, face.labelAt) == 1);
  CHECK(face.labelAt.y() < 0.0);
  CHECK(measurementHit(face, QPointF(130.0, 0.0)) == 2);
  CHECK(measurementHit(face, QPointF(130.0, 60.0)) == 0);
  // A drag in pixels into millimetres along the plane's axes.
  const std::optional<std::array<double, 2>> dragged = measurementDragMillimetres(face, QPointF(30.0, -20.0));
  CHECK(dragged && near((*dragged)[0], 3.0, 1e-6) && near((*dragged)[1], 2.0, 1e-6));
  const MeasurementShape movedText = draggedMeasurement(face, false, QPointF(30.0, 0.0));
  CHECK(near(movedText.labelAt.x(), face.labelAt.x() + 30.0, 1e-6));
  const MeasurementShape movedLine = draggedMeasurement(face, true, QPointF(30.0, -20.0));
  CHECK(near(movedLine.grips.front().first().y(), face.grips.front().first().y() - 20.0, 1e-6));
  CHECK(near(movedLine.grips.front().first().x(), face.grips.front().first().x(), 1e-6));
  // Seen obliquely, the plane's axes are not at right angles on the
  // picture: the line's preview shifts as the drag is committed (its part
  // along y of the drag split into the axes), not by the drag's
  // perpendicular projection on y (mitcad#759).
  const auto oblique = [](const gp_Pnt& p) -> std::optional<QPointF> {
    return QPointF(100.0 + 10.0 * p.X() + 6.0 * p.Y(), 100.0 - 4.0 * p.Y() - 10.0 * p.Z());
  };
  const MeasurementShape skewed = layoutMeasurement(drawn[0], oblique);
  CHECK(skewed.valid && skewed.planar && !skewed.grips.empty());
  if (skewed.valid && !skewed.grips.empty()) {
    for (const QPointF& drag : {QPointF(0.0, -10.0), QPointF(10.0, 0.0), QPointF(7.0, -3.0)}) {
      const std::optional<std::array<double, 2>> committed = measurementDragMillimetres(skewed, drag);
      CHECK(committed.has_value());
      if (!committed) {
        continue;
      }
      const QPointF expected = skewed.yPerMillimetre * (*committed)[1];
      const MeasurementShape shifted = draggedMeasurement(skewed, true, drag);
      const QPointF moved = shifted.grips.front().first() - skewed.grips.front().first();
      CHECK(near(moved.x(), expected.x(), 1e-6) && near(moved.y(), expected.y(), 1e-6));
      CHECK(near(shifted.labelAt.x() - skewed.labelAt.x(), expected.x(), 1e-6) &&
            near(shifted.labelAt.y() - skewed.labelAt.y(), expected.y(), 1e-6));
    }
    // A drag along the line (its x axis) commits no shift, and shows none.
    const MeasurementShape alongLine = draggedMeasurement(skewed, true, skewed.xPerMillimetre * 2.0);
    CHECK(near(alongLine.grips.front().first().x(), skewed.grips.front().first().x(), 1e-6) &&
          near(alongLine.grips.front().first().y(), skewed.grips.front().first().y(), 1e-6));
  }

  // Seen from below (the picture mirrored) and from behind (right to
  // left): the text still reads left to right, upright.
  const auto below = [](const gp_Pnt& p) -> std::optional<QPointF> {
    return QPointF(100.0 + 10.0 * p.X(), 100.0 + 10.0 * p.Y());
  };
  const auto behind = [](const gp_Pnt& p) -> std::optional<QPointF> {
    return QPointF(100.0 - 10.0 * p.X(), 100.0 - 10.0 * p.Y());
  };
  for (const MeasurementProjection& seen : {MeasurementProjection(below), MeasurementProjection(behind)}) {
    const MeasurementShape turned = layoutMeasurement(drawn[0], seen);
    const QPointF at = turned.textTransform.map(QPointF(0.0, 0.0));
    const QPointF right = turned.textTransform.map(QPointF(1.0, 0.0)) - at;
    const QPointF up = turned.textTransform.map(QPointF(0.0, -1.0)) - at;
    CHECK(turned.valid && right.x() > 0.9 && up.y() < -0.9);
  }
  // Edge on (looking along Y at a plane square to Z): faded away, no
  // clicks.
  const auto side = [](const gp_Pnt& p) -> std::optional<QPointF> {
    return QPointF(100.0 + 10.0 * p.X(), 100.0 - 10.0 * p.Z());
  };
  const MeasurementShape edgeOn = layoutMeasurement(drawn[0], side);
  CHECK(edgeOn.valid && near(edgeOn.opacity, 0.0));
  CHECK(measurementHit(edgeOn, edgeOn.labelAt) == 0);

  // A note: its leader and arrow, its lines starting at the leader's end.
  const MeasurementShape noted = layoutMeasurement(drawn[1], top);
  CHECK(noted.valid && noted.note && noted.noteLines.size() == 2);
  CHECK(noted.lines.size() == 1 && noted.arrows.size() == 1 && noted.grips.empty());
  CHECK(!noted.rightAligned);
  CHECK(measurementHit(noted, noted.labelAt) == 1);

  // Painting them, highlighted too.
  QImage planarImage(420, 240, QImage::Format_ARGB32);
  planarImage.fill(Qt::white);
  {
    MeasurementShape highlighted = face;
    highlighted.highlighted = true;
    QPainter painter(&planarImage);
    paintMeasurements(painter, {highlighted, noted, edgeOn}, QColor(250, 250, 252), measurementFont(QFont()));
  }
  bool green = false;
  for (int y = 0; y < planarImage.height() && !green; ++y) {
    for (int x = 0; x < planarImage.width(); ++x) {
      if (planarImage.pixelColor(x, y) == QColor(0x2f, 0x9e, 0x44)) {
        green = true;
        break;
      }
    }
  }
  CHECK(green);

  // A datum label and feature control frames (mitcad#702 phase 2): on the
  // note's leader, a letter in its square at the leader's end and a filled
  // triangle on the feature; a frame's cells in a row; a frame under the
  // dimension, centred below its line and not dragged.
  QJsonObject datum = note;
  datum.insert(QStringLiteral("name"), QStringLiteral("DatumA"));
  datum.insert(QStringLiteral("kind"), QStringLiteral("datum"));
  datum.insert(QStringLiteral("label"),
               QJsonObject{{QStringLiteral("text"), QStringLiteral("A")}, {QStringLiteral("datum"), QStringLiteral("A")}});
  QJsonObject datumDrawing = note.value(QStringLiteral("drawing")).toObject();
  datumDrawing.insert(QStringLiteral("terminator"), QStringLiteral("datum"));
  datum.insert(QStringLiteral("drawing"), datumDrawing);
  const QJsonObject frameLabel{
      {QStringLiteral("text"), QStringLiteral("⟂ | Ø0.05 Ⓜ | A")},
      {QStringLiteral("frame"), QJsonArray{QStringLiteral("⟂"), QStringLiteral("Ø0.05 Ⓜ"), QStringLiteral("A")}},
      {QStringLiteral("tolerance"), QStringLiteral("Ø0.05 Ⓜ")}};
  QJsonObject frame = note;
  frame.insert(QStringLiteral("name"), QStringLiteral("Frame1"));
  frame.insert(QStringLiteral("kind"), QStringLiteral("frame"));
  frame.insert(QStringLiteral("label"), frameLabel);
  frame.insert(QStringLiteral("frame"), QJsonObject{{QStringLiteral("characteristic"), QStringLiteral("perpendicularity")},
                                                  {QStringLiteral("value"), 0.05},
                                                  {QStringLiteral("diameter"), true},
                                                  {QStringLiteral("modifier"), QStringLiteral("M")},
                                                  {QStringLiteral("datums"), QJsonArray{QStringLiteral("A")}}});
  QJsonObject under = frame;
  under.insert(QStringLiteral("name"), QStringLiteral("Frame2"));
  under.insert(QStringLiteral("drawing"),
               QJsonObject{{QStringLiteral("plane"), QJsonObject{{QStringLiteral("origin"), xyz(20, 10, 0)},
                                                                 {QStringLiteral("x"), xyz(1, 0, 0)},
                                                                 {QStringLiteral("y"), xyz(0, 1, 0)},
                                                                 {QStringLiteral("normal"), xyz(0, 0, 1)}}},
                           {QStringLiteral("line_offset"), 0.0},
                           {QStringLiteral("text_offset"), QJsonArray{0.0, 0.0}},
                           {QStringLiteral("text"), QJsonObject{{QStringLiteral("at"), xyz(20, 10, 0)},
                                                                {QStringLiteral("direction"), xyz(1, 0, 0)},
                                                                {QStringLiteral("up"), xyz(0, 1, 0)}}},
                           {QStringLiteral("attached"), QStringLiteral("Distance2")}});
  const std::vector<MeasurementDisplay> leaders = measurementDisplays(QJsonArray{datum, frame, under}, words);
  CHECK(leaders.size() == 3);
  if (leaders.size() != 3) {
    return failures;
  }
  CHECK(leaders[0].datum == QStringLiteral("A") && leaders[0].datumTerminator);
  CHECK(leaders[1].frame.size() == 3 && leaders[2].attached == QStringLiteral("Distance2"));
  const MeasurementShape datumShape = layoutMeasurement(leaders[0], top);
  CHECK(datumShape.valid && datumShape.leader && datumShape.datumTerminator && datumShape.datumLetter == QStringLiteral("A"));
  CHECK(datumShape.noteLines.isEmpty() && datumShape.arrows.size() == 1);
  CHECK(measurementHit(datumShape, datumShape.labelAt) == 1);
  const QFont font = measurementFont(QFont());
  // A square box beside the leader's end.
  const QRectF letterBox = measurementLabelRect(datumShape, font);
  CHECK(near(letterBox.width(), letterBox.height(), 1e-6) && near(letterBox.left(), 0.0, 1e-6));
  const MeasurementShape frameShape = layoutMeasurement(leaders[1], top);
  CHECK(frameShape.valid && frameShape.leader && frameShape.frameCells.size() == 3 && !frameShape.datumTerminator);
  // Its symbol and Ⓜ drawn in lines (fonts may lack them).
  CHECK(frameShape.characteristic == QStringLiteral("perpendicularity") && frameShape.modifier == QStringLiteral("M"));
  const QRectF cells = measurementLabelRect(frameShape, font);
  CHECK(cells.width() > 3.0 * cells.height() - 1e-6 && near(cells.center().y(), 0.0, 1e-6));
  const MeasurementShape underShape = layoutMeasurement(leaders[2], top);
  CHECK(underShape.valid && underShape.attached && underShape.arrows.empty() && underShape.lines.empty());
  const QRectF underRect = measurementLabelRect(underShape, font);
  CHECK(underRect.top() > 0.0 && near(underRect.center().x(), 0.0, 1e-6));
  CHECK(!measurementDragMillimetres(underShape, QPointF(5.0, 5.0)));
  const MeasurementShape notMoved = draggedMeasurement(underShape, false, QPointF(5.0, 5.0));
  CHECK(near(notMoved.labelAt.x(), underShape.labelAt.x(), 1e-9) && near(notMoved.labelAt.y(), underShape.labelAt.y(), 1e-9));
  // Painted: the datum's triangle filled in its colour.
  QImage leaderImage(420, 240, QImage::Format_ARGB32);
  leaderImage.fill(Qt::white);
  {
    QPainter painter(&leaderImage);
    paintMeasurements(painter, {datumShape, frameShape, underShape}, QColor(250, 250, 252), font);
  }
  const std::optional<QPointF> foot = top(gp_Pnt(0, 0, 0));
  CHECK(foot.has_value());
  if (foot) {
    // Beside the leader's end, inside the triangle's base, where an
    // arrowhead's point leaves the halo: the leader comes down to the left.
    const QPoint inside = foot->toPoint() + QPoint(2, 1);
    CHECK(leaderImage.pixelColor(inside) == QColor(0x70, 0x48, 0xe8));
  }
  return failures;
}
