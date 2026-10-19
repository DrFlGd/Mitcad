// SPDX-License-Identifier: MIT
// Sketch tools that pick geometry: Trim and Extend (a click on a curve),
// the sketch dimension (the kind follows what is picked and where the value
// is placed) and the geometric constraints.
#include <algorithm>
#include <cmath>
#include <limits>
#include <tuple>
#include <utility>
#include <vector>

#include <QHash>
#include <QObject>
#include <QtLogging>

#include "SketchController.hpp"
#include "SketchOverlay.hpp"
#include "SketchTool.hpp"
#include "ToolSupport.hpp"

namespace mitcad::sketch {
namespace {

using Style = ToolPreview::Style;

bool isPoint(const SketchModel& model, const QString& id) { return model.point(id) != nullptr; }

bool isLine(const SketchModel& model, const QString& id) {
  const CurveData* c = model.curve(id);
  return c != nullptr && c->isLine();
}

// ---------------------------------------------------------------------------
// Trim and Extend

// The piece of a curve between the curves crossing it around `at`.
std::vector<V2> trimPiece(const SketchModel& model, const CurveData& c, const V2& at, double step) {
  std::vector<double> cuts;
  for (const CurveData& other : model.curves) {
    if (other.id == c.id) {
      continue;
    }
    for (const V2& hit : intersections(c.curve, other.curve)) {
      cuts.push_back(c.curve.parameterOf(hit));
    }
    for (const QString& id : {other.start, other.end}) {
      const auto p = model.pointAt(id);
      if (p && c.curve.distanceTo(*p) < 1e-6) {
        cuts.push_back(c.curve.parameterOf(*p));
      }
    }
  }
  for (const PointData& p : model.points) {
    if (p.id != c.start && p.id != c.end && p.id != c.center && c.curve.distanceTo(p.at) < 1e-6) {
      cuts.push_back(c.curve.parameterOf(p.at));
    }
  }
  const double t = c.curve.parameterOf(at);
  double lo = c.curve.t0();
  double hi = c.curve.t1();
  if (c.curve.isClosed()) {
    std::sort(cuts.begin(), cuts.end());
    cuts.erase(std::unique(cuts.begin(), cuts.end(), [](double a, double b) { return std::abs(a - b) < 1e-9; }),
               cuts.end());
    if (cuts.size() >= 2) {
      const auto above = std::upper_bound(cuts.begin(), cuts.end(), t);
      hi = above == cuts.end() ? cuts.front() + kTwoPi : *above;
      lo = above == cuts.begin() ? cuts.back() - kTwoPi : *(above - 1);
    }
  } else {
    for (const double cut : cuts) {
      if (cut <= c.curve.t0() + 1e-9 || cut >= c.curve.t1() - 1e-9) {
        continue;
      }
      if (cut <= t) {
        lo = std::max(lo, cut);
      } else {
        hi = std::min(hi, cut);
      }
    }
  }
  std::vector<V2> piece;
  const double length = (hi - lo) * (c.curve.type == Curve2::Type::Line ? norm(c.curve.b - c.curve.a)
                                                                        : std::max(c.curve.radius, 1.0));
  const int count = c.curve.type == Curve2::Type::Line
                        ? 1
                        : std::clamp(static_cast<int>(std::abs(length) / std::max(step, 1e-6)), 8, 400);
  for (int i = 0; i <= count; ++i) {
    piece.push_back(c.curve.at(lo + (hi - lo) * i / count));
  }
  return piece;
}

class TrimTool : public SketchTool {
public:
  TrimTool(SketchController& c, bool extend) : SketchTool(c), m_extend(extend) {}

  QString name() const override { return m_extend ? QObject::tr("Extend") : QObject::tr("Trim"); }
  bool snaps() const override { return false; }

  void start() override {
    m_c.hint(m_extend ? QObject::tr("Extend: click near the end of a line or an arc to extend it.")
                      : QObject::tr("Trim: click the part of a curve to remove."));
  }

  void move(const Snap& snap) override {
    m_at = snap.at;
    m_curve = m_c.entityAt(snap.screen, PickCurves);
    if (m_extend) {
      m_c.setHover(m_curve);
    }
  }

  void press(const Snap& snap) override {
    const QString curve = m_c.entityAt(snap.screen, PickCurves);
    const CurveData* c = m_c.model().curve(curve);
    if (c == nullptr) {
      return;
    }
    const V2 on = c->curve.nearest(snap.at);
    SketchOp op(m_c);
    QJsonObject cmd = op.command(m_extend ? QStringLiteral("sketch.extend") : QStringLiteral("sketch.trim"));
    cmd.insert(QStringLiteral("curve"), curve);
    cmd.insert(QStringLiteral("at"), xy(on));
    QJsonObject result;
    if (!op.run(cmd, &result)) {
      const QString reason = op.error();
      op.rollback();
      m_c.error(QObject::tr("%1: %2").arg(name(), reason));
      return;
    }
    op.commit();
    m_curve.clear();
    m_c.setHover(QString());
    // An end that reached an existing point joined it (mitcad#259).
    const QString joined = result.value(QStringLiteral("joined")).toString();
    qDebug().noquote() << QStringLiteral("%1 %2 at %3%4")
                              .arg(m_extend ? QStringLiteral("Extended") : QStringLiteral("Trimmed"), curve,
                                   text(on),
                                   joined.isEmpty() ? QString() : QStringLiteral(" (joined %1)").arg(joined));
  }

  void paint(ToolPreview& preview) const override {
    const CurveData* c = m_c.model().curve(m_curve);
    if (c == nullptr || m_extend) {
      return;
    }
    preview.add(trimPiece(m_c.model(), *c, m_at, 2.0 * m_c.pixel()), Style::Remove);
  }

private:
  bool m_extend;
  QString m_curve;
  V2 m_at;
};

// ---------------------------------------------------------------------------
// Sketch dimension

class DimensionTool : public SketchTool {
public:
  using SketchTool::SketchTool;

  QString name() const override { return QObject::tr("Sketch Dimension"); }
  bool snaps() const override { return false; }

  void start() override { reset(); }

  void move(const Snap& snap) override {
    m_cursor = snap.at;
    if (m_state == State::Edit) {
      return;
    }
    QString hover = entityAt(snap.screen);
    if (m_picks.contains(hover) || (m_state == State::Place && !secondPick(hover))) {
      hover.clear();
    }
    m_c.setHover(hover);
  }

  void press(const Snap& snap) override {
    m_cursor = snap.at;
    const QString entity = entityAt(snap.screen);    switch (m_state) {
    case State::Pick:
      if (entity.isEmpty()) {
        return;
      }
      m_picks = {entity};
      m_state = isPoint(view(), entity) ? State::Pick2 : State::Place;
      if (m_state == State::Place && !decide()) {
        // A spline: nothing to measure alone.
        m_state = State::Pick2;
      }
      break;
    case State::Pick2:
      if (entity.isEmpty() || m_picks.contains(entity)) {
        return;
      }
      m_picks << entity;
      if (!decide()) {
        m_c.hint(QObject::tr("Sketch Dimension: these cannot be dimensioned together."));
        reset();
        return;
      }
      m_state = State::Place;
      break;
    case State::Place:
      if (secondPick(entity)) {
        m_picks << entity;
        if (!decide()) {
          m_picks.removeLast();
        }
        break;
      }
      place();
      return;
    case State::Edit:
      return;
    }
    logBodyPick(entity);
    m_c.setPicked(m_picks);
    m_c.setHover(QString());
    prompt();
  }

  bool cancel() override {
    if (m_state == State::Pick) {
      return false;
    }
    reset();
    return true;
  }

  void paint(ToolPreview& preview) const override {
    if (m_state == State::Place || m_state == State::Edit) {
      if (auto d = decide()) {
        d->text = m_state == State::Edit ? m_placed : m_cursor;
        preview.dimension = *d;
        preview.model = &view();
      }
    }
  }

private:
  enum class State { Pick, Pick2, Place, Edit };

  // The point or curve under the cursor: the sketch's, or a body edge or
  // vertex in the sketch plane (mitcad#256), read again after the document
  // changed.
  QString entityAt(const QPointF& screen) {
    m_c.bodyGeometry();
    return m_c.entityAt(screen, PickAll | PickBody);
  }

  // The sketch with the body edges and vertices as the fixed entities they
  // project to, so that a body pick makes the dimensions a sketch line,
  // circle, arc or point would.
  const SketchModel& view() const {
    const std::pair<int, int> key{m_c.modelRevision(), m_c.bodyGeometryShown().revision()};
    if (key != m_viewKey) {
      m_view = m_c.model();
      m_c.bodyGeometryShown().addTo(m_view);
      m_viewKey = key;
    }
    return m_view;
  }

  // For UI tests: a body edge or vertex taken as a pick.
  void logBodyPick(const QString& entity) const {
    const BodyItem* item = m_c.bodyGeometryShown().item(entity);
    if (item != nullptr && m_picks.contains(entity)) {
      qDebug().noquote() << QStringLiteral("Sketch Dimension picked %1 of %2 (%3)")
                                .arg(item->describe(), item->body, entity);
    }
  }

  // Projects the body edges and vertices a dimension names into the sketch
  // (linked), or takes an existing projection of the same source, and names
  // the projected entities instead: a line, circle or arc, its end or centre,
  // a vertex's point (mitcad#256). False when a projection is refused.
  bool projectPicks(SketchOp& op, QJsonObject& dimension) const {
    // The items as they are now: projecting reads the sketch again.
    std::vector<std::tuple<QString, BodyItem, QString>> refs; // field, item, part
    for (auto it = dimension.constBegin(); it != dimension.constEnd(); ++it) {
      QString part;
      if (const BodyItem* item = m_c.bodyGeometryShown().item(it.value().toString(), &part)) {
        refs.emplace_back(it.key(), *item, part);
      }
    }
    for (const auto& [field, item, part] : refs) {
      const ProjectedItem projected = op.projectBody(item);
      if (projected.empty()) {
        return false;
      }
      const SketchModel& model = op.model();
      QString entity = item.isVertex() ? projected.point : projected.curve;
      if (!part.isEmpty()) {
        const CurveData* curve = model.curve(projected.curve);
        entity.clear();
        if (curve != nullptr && part == QStringLiteral("center")) {
          entity = curve->center;
        } else if (curve != nullptr) {
          // The projected curve's end where the edge's is.
          const V2 want = part == QStringLiteral("start") ? item.curve.startPoint() : item.curve.endPoint();
          double nearest = std::numeric_limits<double>::max();
          for (const QString& end : {curve->start, curve->end}) {
            const auto at = model.pointAt(end);
            if (at && dist(*at, want) < nearest) {
              nearest = dist(*at, want);
              entity = end;
            }
          }
        }
      }
      if (entity.isEmpty()) {
        return false;
      }
      dimension.insert(field, entity);
    }
    return true;
  }

  void reset() {
    m_state = State::Pick;
    m_picks.clear();
    m_c.setPicked({});
    m_c.setHover(QString());
    prompt();
  }

  void prompt() const {
    switch (m_state) {
    case State::Pick:
      m_c.hint(QObject::tr("Sketch Dimension: pick a line, circle, arc or point, of the sketch or a body edge or "
                           "vertex in its plane."));
      break;
    case State::Pick2:
      m_c.hint(QObject::tr("Sketch Dimension: pick another point or curve."));
      break;
    case State::Place:
      m_c.hint(QObject::tr("Sketch Dimension: click where the value goes (or pick a second entity)."));
      break;
    case State::Edit:
      m_c.hint(QObject::tr("Sketch Dimension: type the value or an expression and press Enter."));
      break;
    }
  }

  // A second entity that makes another kind of dimension with the first.
  bool secondPick(const QString& entity) const {
    if (entity.isEmpty() || m_picks.size() != 1 || m_picks.contains(entity)) {
      return false;
    }
    const SketchModel& model = view();
    const CurveData* first = model.curve(m_picks.first());
    const CurveData* second = model.curve(entity);
    const bool kind = first == nullptr ||
                      (first->isLine() && (second == nullptr || second->isLine() || second->isCircular())) ||
                      (first->isCircular() && (second == nullptr || second->isCircular() || second->isLine()));
    // Only one that makes a dimension with it (mitcad#445).
    return kind && decide({m_picks.first(), entity}).has_value();
  }

  // The dimension the picks and the cursor make (without its value); while
  // the value is typed, the place it was put at.
  std::optional<DimensionData> decide() const { return decide(m_picks); }

  std::optional<DimensionData> decide(const QStringList& picks) const {
    const SketchModel& model = view();
    const V2 cursor = m_state == State::Edit ? m_placed : m_cursor;
    DimensionData d;
    const auto set = [&d](const QString& type, std::initializer_list<std::pair<const char*, QString>> fields) {
      d.type = type;
      d.json = QJsonObject{{QStringLiteral("type"), type}};
      for (const auto& [key, value] : fields) {
        d.json.insert(QLatin1String(key), value);
        d.refs << value;
      }
    };
    // Distances between two points: aligned, or along an axis when the
    // value is placed above, below or beside them.
    const auto pointDistance = [&](const QString& a, const QString& b) {
      const V2 pa = *model.pointAt(a);
      const V2 pb = *model.pointAt(b);
      const V2 c = cursor;
      const bool insideX = c.x > std::min(pa.x, pb.x) && c.x < std::max(pa.x, pb.x);
      const bool insideY = c.y > std::min(pa.y, pb.y) && c.y < std::max(pa.y, pb.y);
      const bool alongX = std::abs(pa.y - pb.y) < 1e-9;
      const bool alongY = std::abs(pa.x - pb.x) < 1e-9;
      if (insideX && !insideY && !alongX && !alongY) {
        set(QStringLiteral("horizontal_distance"), {{"a", a}, {"b", b}});
      } else if (insideY && !insideX && !alongX && !alongY) {
        set(QStringLiteral("vertical_distance"), {{"a", a}, {"b", b}});
      } else {
        set(QStringLiteral("distance"), {{"a", a}, {"b", b}});
      }
    };
    if (picks.size() == 1) {
      const CurveData* c = model.curve(picks.first());
      if (c == nullptr) {
        return std::nullopt;
      }
      if (c->isLine()) {
        const V2 a = c->curve.a;
        const V2 b = c->curve.b;
        const V2 cur = cursor;
        const bool insideX = cur.x > std::min(a.x, b.x) && cur.x < std::max(a.x, b.x);
        const bool insideY = cur.y > std::min(a.y, b.y) && cur.y < std::max(a.y, b.y);
        const bool slanted = std::abs(a.x - b.x) > 1e-9 && std::abs(a.y - b.y) > 1e-9;
        if (slanted && insideX && !insideY) {
          set(QStringLiteral("horizontal_distance"), {{"a", c->start}, {"b", c->end}});
        } else if (slanted && insideY && !insideX) {
          set(QStringLiteral("vertical_distance"), {{"a", c->start}, {"b", c->end}});
        } else {
          set(QStringLiteral("length"), {{"line", c->id}});
        }
      } else if (c->isCircle()) {
        set(QStringLiteral("diameter"), {{"curve", c->id}});
      } else if (c->isArc()) {
        set(QStringLiteral("radius"), {{"curve", c->id}});
      } else if (c->isEllipse()) {
        const V2 major = polar(c->curve.rotation);
        const bool minor = std::abs(dot(unit(cursor - c->curve.center), major)) < std::sqrt(0.5);
        set(minor ? QStringLiteral("minor_radius") : QStringLiteral("major_radius"), {{"ellipse", c->id}});
      } else {
        return std::nullopt;
      }
    } else if (picks.size() == 2) {
      QString a = picks[0];
      QString b = picks[1];
      const CurveData* ca = model.curve(a);
      const CurveData* cb = model.curve(b);
      // Two concentric circles or arcs make the gap between them
      // (mitcad#445); other circles count by their centres.
      const bool concentric = ca != nullptr && cb != nullptr && ca->isCircular() && cb->isCircular() &&
                              dist(ca->curve.center, cb->curve.center) < 1e-9;
      if (!concentric && ca != nullptr && ca->isCircular()) {
        a = ca->center;
        ca = nullptr;
      }
      if (!concentric && cb != nullptr && cb->isCircular()) {
        b = cb->center;
        cb = nullptr;
      }
      if (concentric) {
        if (std::abs(ca->curve.radius - cb->curve.radius) < 1e-9) {
          return std::nullopt;
        }
        set(QStringLiteral("concentric_distance"), {{"a", a}, {"b", b}});
      } else if (ca == nullptr && cb == nullptr) {
        if (!model.pointAt(a) || !model.pointAt(b) || a == b) {
          return std::nullopt;
        }
        pointDistance(a, b);
      } else if (ca != nullptr && cb != nullptr) {
        if (!ca->isLine() || !cb->isLine()) {
          return std::nullopt;
        }
        const V2 da = unit(ca->curve.b - ca->curve.a);
        const V2 db = unit(cb->curve.b - cb->curve.a);
        if (std::abs(cross(da, db)) < 1e-9) {
          if (ca->centerline || cb->centerline) {
            const CurveData* axis = ca->centerline ? ca : cb;
            const CurveData* other = ca->centerline ? cb : ca;
            set(QStringLiteral("linear_diameter"), {{"axis", axis->id}, {"entity", other->id}});
          } else {
            set(QStringLiteral("line_distance"), {{"a", ca->id}, {"b", cb->id}});
          }
        } else {
          // The sector of the four the lines' extensions make that the
          // cursor is in, by the rays that bound it (mitcad#296): cursor -
          // corner = s da + t db, a ray reversed where its factor is
          // negative.
          set(QStringLiteral("angle"), {{"a", ca->id}, {"b", cb->id}});
          const double c = cross(da, db);
          const V2 corner = ca->curve.a + da * (cross(cb->curve.a - ca->curve.a, db) / c);
          const V2 v = cursor - corner;
          d.json.insert(QStringLiteral("reverse_a"), cross(v, db) / c < 0.0);
          d.json.insert(QStringLiteral("reverse_b"), cross(da, v) / c < 0.0);
        }
      } else {
        const CurveData* line = ca != nullptr ? ca : cb;
        const QString point = ca != nullptr ? b : a;
        if (!line->isLine() || !model.pointAt(point)) {
          return std::nullopt;
        }
        if (line->centerline) {
          set(QStringLiteral("linear_diameter"), {{"axis", line->id}, {"entity", point}});
        } else {
          set(QStringLiteral("point_line_distance"), {{"point", point}, {"line", line->id}});
        }
      }
    } else {
      return std::nullopt;
    }
    // The palette's Signed distances (mitcad#409): the offset from the
    // first point, which may be negative.
    if (m_c.signedDistances() && d.isAxisDistance()) {
      d.json.insert(QStringLiteral("signed"), true);
      d.signedOffset = true;
    }
    if (const auto value = measure(d, model)) {
      d.measured = *value;
    } else {
      return std::nullopt;
    }
    return d;
  }

  void place() {
    const auto d = decide();
    if (!d) {
      reset();
      return;
    }
    m_state = State::Edit;
    m_placed = m_cursor;
    m_c.setHover(QString());
    prompt();
    const bool angle = d->type == QStringLiteral("angle");
    const QString value = angle ? QString::number(d->measured * 180.0 / kPi, 'f', 2)
                                : QString::number(d->measured / m_c.lengthScale(), 'f', 2);
    const QJsonObject definition = d->json;
    const V2 at = m_placed;
    m_c.openValueEditor(m_c.toScreen(at), value, angle ? FieldSpec::Kind::Angle : FieldSpec::Kind::Length,
                        [this, definition, at](const QString& expression) { add(definition, at, expression); });
  }

  void add(const QJsonObject& definition, const V2& at, const QString& expression) {
    SketchOp op(m_c);
    // Body picks are projected now, in the same undo step (mitcad#256).
    QJsonObject dimension = definition;
    if (!projectPicks(op, dimension)) {
      const QString reason =
          op.error().isEmpty() ? QObject::tr("the body geometry could not be projected") : op.error();
      op.rollback();
      m_c.error(QObject::tr("Dimension not added: %1").arg(reason));
      reset();
      return;
    }
    QJsonObject cmd = op.command(QStringLiteral("sketch.add_dimension"));
    cmd.insert(QStringLiteral("dimension"), dimension);
    cmd.insert(QStringLiteral("value"), expression);
    cmd.insert(QStringLiteral("text"), xy(at));
    QJsonObject result;
    bool driven = false;
    bool ok = op.run(cmd, &result);
    const QString type = definition.value(QStringLiteral("type")).toString();
    if (!ok && op.error().contains(QStringLiteral("must not be negative")) &&
        (type == QStringLiteral("horizontal_distance") || type == QStringLiteral("vertical_distance")) &&
        !dimension.value(QStringLiteral("signed")).toBool()) {
      // A negative value typed for a horizontal or vertical distance makes
      // it signed (mitcad#409): the point goes to that side of the first.
      dimension.insert(QStringLiteral("signed"), true);
      cmd.insert(QStringLiteral("dimension"), dimension);
      ok = op.run(cmd, &result);
    }
    if (!ok) {
      if (!op.error().contains(QStringLiteral("over-constrain"))) {
        const QString reason = op.error();
        op.rollback();
        m_c.error(QObject::tr("Dimension not added: %1").arg(reason));
        reset();
        return;
      }
      // A driven dimension instead of over-constraining.
      cmd.remove(QStringLiteral("value"));
      cmd.insert(QStringLiteral("driven"), true);
      driven = true;
      if (!op.run(cmd, &result)) {
        const QString reason = op.error();
        op.rollback();
        m_c.error(QObject::tr("Dimension not added: %1").arg(reason));
        reset();
        return;
      }
    }
    op.commit();
    const QString id = result.value(QStringLiteral("dimensions")).toArray().first().toString();
    const DimensionData* d = m_c.model().dimension(id);
    // "horizontal_distance signed" for a signed one (mitcad#409).
    const QString kind = d != nullptr && d->signedOffset ? type + QStringLiteral(" signed") : type;
    if (driven) {
      m_c.hint(QObject::tr("The dimension would over-constrain the sketch: added as a driven dimension."));
      qDebug().noquote() << QStringLiteral("Added driven dimension %1 %2").arg(id, kind);
    } else {
      qDebug().noquote() << QStringLiteral("Added dimension %1 %2 %3 = %4")
                                .arg(id, kind, d != nullptr ? d->parameter : QString(), expression);
    }
    if (d != nullptr && d->signedOffset) {
      qDebug().noquote() << QStringLiteral("Dimension %1 shows %2").arg(id, m_c.overlay().dimensionText(*d));
    }
    // For UI tests: the sector an angle measures, as drawn (mitcad#296).
    if (d != nullptr && d->type == QStringLiteral("angle")) {
      if (const auto rays = angleRays(*d, m_c.model())) {
        const auto degrees = [](const V2& v) {
          return QString::number(std::fmod(angleOf(v) * 180.0 / kPi + 360.0, 360.0), 'f', 1);
        };
        // Rounded first, so that no "-0.00".
        const auto twoPlaces = [](double x) { return QString::number(std::round(x * 100.0) / 100.0 + 0.0, 'f', 2); };
        qDebug().noquote() << QStringLiteral("Angle dimension %1: %2 deg, the arc from %3 to %4 deg about %5,%6")
                                  .arg(id, QString::number(d->measured * 180.0 / kPi, 'f', 2), degrees(rays->da),
                                       degrees(rays->db), twoPlaces(rays->corner.x), twoPlaces(rays->corner.y));
      }
    }
    reset();
  }

  State m_state = State::Pick;
  QStringList m_picks;
  V2 m_cursor;
  V2 m_placed;
  // view(): the sketch with the body geometry, and when it was made.
  mutable SketchModel m_view;
  mutable std::pair<int, int> m_viewKey{-1, -1};
};

// ---------------------------------------------------------------------------
// Constraints

int needed(const QString& type, const QStringList& picks, const SketchModel& model) {
  if (type == QStringLiteral("fix")) {
    return 1;
  }
  if (type == QStringLiteral("horizontal_vertical")) {
    return !picks.isEmpty() && isLine(model, picks.first()) ? 1 : 2;
  }
  if (type == QStringLiteral("symmetric")) {
    return 3;
  }
  return 2;
}

// Whether `entity` can be the next pick.
bool acceptable(const QString& type, const QStringList& picks, const QString& entity,
                const SketchModel& model) {
  if (entity.isEmpty() || picks.contains(entity)) {
    return false;
  }
  const bool point = isPoint(model, entity);
  const CurveData* curve = model.curve(entity);
  const int index = static_cast<int>(picks.size());
  const auto pickedPoints = [&] {
    return std::count_if(picks.begin(), picks.end(), [&](const QString& p) { return isPoint(model, p); });
  };
  if (type == QStringLiteral("fix")) {
    return true;
  }
  if (type == QStringLiteral("coincident")) {
    // A point and a point or curve, in either order.
    return index == 0 || point || pickedPoints() > 0;
  }
  if (type == QStringLiteral("horizontal_vertical")) {
    return index == 0 ? point || (curve != nullptr && curve->isLine()) : point;
  }
  if (type == QStringLiteral("parallel") || type == QStringLiteral("perpendicular") ||
      type == QStringLiteral("collinear")) {
    return curve != nullptr && curve->isLine();
  }
  if (type == QStringLiteral("tangent") || type == QStringLiteral("smooth")) {
    return curve != nullptr;
  }
  if (type == QStringLiteral("equal")) {
    if (curve == nullptr || !(curve->isLine() || curve->isCircular())) {
      return false;
    }
    const CurveData* first = index == 0 ? nullptr : model.curve(picks.first());
    return first == nullptr || first->isLine() == curve->isLine();
  }
  if (type == QStringLiteral("concentric")) {
    return curve != nullptr && (curve->isCircular() || curve->isEllipse());
  }
  if (type == QStringLiteral("midpoint")) {
    if (index == 0) {
      return point || (curve != nullptr && (curve->isLine() || curve->isArc()));
    }
    return pickedPoints() > 0 ? curve != nullptr && (curve->isLine() || curve->isArc()) : point;
  }
  if (type == QStringLiteral("symmetric")) {
    if (index == 2) {
      return curve != nullptr && curve->isLine();
    }
    if (index == 1) {
      const CurveData* first = model.curve(picks.first());
      return first == nullptr ? point : curve != nullptr && curve->type == first->type;
    }
    return true;
  }
  return false;
}

// The constraint of complete picks.
QJsonObject build(const QString& type, const QStringList& picks, const SketchModel& model) {
  const auto obj = [](const QString& t, std::initializer_list<std::pair<const char*, QString>> fields) {
    QJsonObject k{{QStringLiteral("type"), t}};
    for (const auto& [key, value] : fields) {
      k.insert(QLatin1String(key), value);
    }
    return k;
  };
  if (type == QStringLiteral("coincident") || type == QStringLiteral("midpoint")) {
    const bool firstPoint = isPoint(model, picks[0]);
    const QString point = firstPoint ? picks[0] : picks[1];
    const QString other = firstPoint ? picks[1] : picks[0];
    return type == QStringLiteral("coincident") ? obj(type, {{"point", point}, {"entity", other}})
                                                : obj(type, {{"point", point}, {"curve", other}});
  }
  if (type == QStringLiteral("horizontal_vertical")) {
    if (picks.size() == 1) {
      const CurveData* c = model.curve(picks[0]);
      const V2 d = c->curve.b - c->curve.a;
      return obj(std::abs(d.x) >= std::abs(d.y) ? QStringLiteral("horizontal") : QStringLiteral("vertical"),
                 {{"line", picks[0]}});
    }
    // The rule the auto constraints align centres by (mitcad#189).
    return obj(alignsHorizontally(*model.pointAt(picks[0]), *model.pointAt(picks[1]))
                   ? QStringLiteral("horizontal_points")
                   : QStringLiteral("vertical_points"),
               {{"a", picks[0]}, {"b", picks[1]}});
  }
  if (type == QStringLiteral("symmetric")) {
    return obj(type, {{"a", picks[0]}, {"b", picks[1]}, {"axis", picks[2]}});
  }
  return obj(type, {{"a", picks[0]}, {"b", picks[1]}});
}

// Adds a constraint for each set of complete picks, as one undo step. A
// refused one is left out, so entities already constrained do not stop
// the others; when all are refused nothing changes.
bool applyEach(SketchController& c, const QString& type, const QVector<QStringList>& sets) {
  SketchOp op(c);
  int added = 0;
  QString reason;
  // Points a coincident merged away (mitcad#260): removed -> kept, for the
  // later picks and the selection.
  QHash<QString, QString> merged;
  const auto current = [&merged](QString id) {
    for (int guard = 0; guard < 64 && merged.contains(id); ++guard) {
      id = merged.value(id);
    }
    return id;
  };
  for (QStringList picks : sets) {
    for (QString& pick : picks) {
      pick = current(pick);
    }
    const QJsonObject constraint = build(type, picks, op.model());
    QJsonObject cmd = op.command(QStringLiteral("sketch.add_constraint"));
    cmd.insert(QStringLiteral("constraint"), constraint);
    QJsonObject result;
    if (!op.run(cmd, &result)) {
      reason = op.error();
      qDebug().noquote() << QStringLiteral("Constraint %1 refused: %2")
                                .arg(constraint.value(QStringLiteral("type")).toString(), reason);
      continue;
    }
    ++added;
    const QJsonObject merge = result.value(QStringLiteral("merged")).toObject();
    if (!merge.isEmpty()) {
      // Two curve ends became one point.
      const QString kept = merge.value(QStringLiteral("kept")).toString();
      const QString removed = merge.value(QStringLiteral("removed")).toString();
      merged.insert(removed, kept);
      qDebug().noquote() << QStringLiteral("Merged points %1 into %2 (coincident on %3)")
                                .arg(removed, kept, picks.join(QStringLiteral(", ")));
      continue;
    }
    const QJsonArray made = result.value(QStringLiteral("constraints")).toArray();
    qDebug().noquote() << QStringLiteral("Added constraint %1 %2 on %3")
                              .arg(constraint.value(QStringLiteral("type")).toString(),
                                   made.isEmpty() ? QString() : made.first().toString(),
                                   picks.join(QStringLiteral(", ")));
  }
  if (added == 0) {
    op.rollback();
    c.error(QObject::tr("%1 not added: %2").arg(constraintName(type), reason));
    return false;
  }
  if (!merged.isEmpty()) {
    // The selection holds the kept point instead of a removed one.
    Selection selection;
    for (SelectionItem item : c.host().selection()) {
      if (item.owner == c.uid() && item.kind == SelectKind::SketchPoint && merged.contains(item.name)) {
        item.name = current(item.name);
        if (selection.contains(item)) {
          continue;
        }
      }
      selection.append(item);
    }
    if (selection != c.host().selection()) {
      c.host().setSelection(selection);
    }
  }
  op.commit();
  if (added < static_cast<int>(sets.size())) {
    c.hint(QObject::tr("%1: %2 of %3 added; the others were refused: %4")
               .arg(constraintName(type))
               .arg(added)
               .arg(static_cast<int>(sets.size()))
               .arg(reason));
  }
  return true;
}

bool apply(SketchController& c, const QString& type, const QStringList& picks) {
  if (type == QStringLiteral("fix")) {
    SketchOp op(c);
    bool fixed = true;
    for (const QString& id : picks) {
      const PointData* p = c.model().point(id);
      const CurveData* curve = c.model().curve(id);
      fixed = fixed && ((p != nullptr && p->fixed) || (curve != nullptr && curve->fixed));
    }
    QJsonObject cmd = op.command(QStringLiteral("sketch.set_fixed"));
    cmd.insert(QStringLiteral("entities"), QJsonArray::fromStringList(picks));
    cmd.insert(QStringLiteral("fixed"), !fixed);
    if (!op.run(cmd)) {
      const QString reason = op.error();
      op.rollback();
      c.error(QObject::tr("Fix/Unfix: %1").arg(reason));
      return false;
    }
    op.commit();
    qDebug().noquote() << QStringLiteral("%1 %2").arg(fixed ? QStringLiteral("Unfixed") : QStringLiteral("Fixed"),
                                                      picks.join(QStringLiteral(", ")));
    return true;
  }
  return applyEach(c, type, {picks});
}

// Constraints that a selection of more than they take applies to the
// first entity with each of the others (a line's horizontal/vertical: to
// each line).
bool pairwise(const QString& type) {
  return type == QStringLiteral("equal") || type == QStringLiteral("parallel") ||
         type == QStringLiteral("perpendicular") || type == QStringLiteral("collinear") ||
         type == QStringLiteral("concentric") || type == QStringLiteral("tangent") ||
         type == QStringLiteral("coincident") || type == QStringLiteral("horizontal_vertical");
}

// The picks of each constraint a preselection makes: the selection itself
// when it is what the constraint takes; for a pairwise constraint, the
// first entity with each of the others, in the order they were selected.
// Empty when the selection does not fit.
QVector<QStringList> selectionSets(const QString& type, const QStringList& picks, const SketchModel& model) {
  QStringList ordered;
  for (const QString& pick : picks) {
    if (!acceptable(type, ordered, pick, model)) {
      break;
    }
    ordered << pick;
  }
  if (ordered.size() == picks.size() &&
      (type == QStringLiteral("fix") || static_cast<int>(ordered.size()) == needed(type, ordered, model))) {
    return {ordered};
  }
  if (!pairwise(type) || picks.size() < 2) {
    return {};
  }
  const QString& first = picks.first();
  QVector<QStringList> sets;
  if (needed(type, {first}, model) == 1) {
    for (const QString& pick : picks) {
      if (!acceptable(type, {}, pick, model) || needed(type, {pick}, model) != 1) {
        return {};
      }
      sets << QStringList{pick};
    }
    return sets;
  }
  for (const QString& pick : picks.mid(1)) {
    if (!acceptable(type, {first}, pick, model)) {
      return {};
    }
    sets << QStringList{first, pick};
  }
  return sets;
}

class ConstraintTool : public SketchTool {
public:
  ConstraintTool(SketchController& c, const QString& type) : SketchTool(c), m_type(type) {}

  QString name() const override { return constraintName(m_type); }
  bool snaps() const override { return false; }
  void start() override { prompt(); }

  void move(const Snap& snap) override {
    const QString entity = m_c.entityAt(snap.screen);
    m_c.setHover(acceptable(m_type, m_picks, entity, m_c.model()) ? entity : QString());
  }

  void press(const Snap& snap) override {
    const QString entity = m_c.entityAt(snap.screen);
    if (!acceptable(m_type, m_picks, entity, m_c.model())) {
      return;
    }
    m_picks << entity;
    if (static_cast<int>(m_picks.size()) >= needed(m_type, m_picks, m_c.model())) {
      apply(m_c, m_type, m_picks);
      m_picks.clear();
    }
    m_c.setPicked(m_picks);
    m_c.setHover(QString());
    prompt();
  }

  bool cancel() override {
    if (m_picks.isEmpty()) {
      return false;
    }
    m_picks.clear();
    m_c.setPicked({});
    prompt();
    return true;
  }

private:
  void prompt() const {
    const QString what = m_type == QStringLiteral("coincident")
                             ? QObject::tr("a point and a point or curve; two curve ends become one point")
                         : m_type == QStringLiteral("horizontal_vertical") ? QObject::tr("a line or two points")
                         : m_type == QStringLiteral("midpoint")     ? QObject::tr("a point and a line or arc")
                         : m_type == QStringLiteral("symmetric")    ? QObject::tr("two entities and a line")
                         : m_type == QStringLiteral("fix")          ? QObject::tr("an entity")
                         : m_type == QStringLiteral("equal")        ? QObject::tr("two lines, or two circles or arcs")
                         : m_type == QStringLiteral("concentric")   ? QObject::tr("two circles or arcs")
                         : m_type == QStringLiteral("tangent") || m_type == QStringLiteral("smooth")
                             ? QObject::tr("two curves")
                             : QObject::tr("two lines");
    m_c.hint(QObject::tr("%1: pick %2 (%3 picked).").arg(name(), what).arg(m_picks.size()));
  }

  QString m_type;
  QStringList m_picks;
};

} // namespace

QString constraintName(const QString& type) {
  if (type == QStringLiteral("coincident")) {
    return QObject::tr("Coincident");
  }
  if (type == QStringLiteral("horizontal_vertical")) {
    return QObject::tr("Horizontal/Vertical");
  }
  if (type == QStringLiteral("parallel")) {
    return QObject::tr("Parallel");
  }
  if (type == QStringLiteral("perpendicular")) {
    return QObject::tr("Perpendicular");
  }
  if (type == QStringLiteral("tangent")) {
    return QObject::tr("Tangent");
  }
  if (type == QStringLiteral("equal")) {
    return QObject::tr("Equal");
  }
  if (type == QStringLiteral("concentric")) {
    return QObject::tr("Concentric");
  }
  if (type == QStringLiteral("collinear")) {
    return QObject::tr("Collinear");
  }
  if (type == QStringLiteral("midpoint")) {
    return QObject::tr("Midpoint");
  }
  if (type == QStringLiteral("symmetric")) {
    return QObject::tr("Symmetry");
  }
  if (type == QStringLiteral("smooth")) {
    return QObject::tr("Curvature");
  }
  if (type == QStringLiteral("fix")) {
    return QObject::tr("Fix/Unfix");
  }
  // Types the model names (a constraint a new one replaced, mitcad#287).
  if (type == QStringLiteral("horizontal") || type == QStringLiteral("horizontal_points")) {
    return QObject::tr("Horizontal");
  }
  if (type == QStringLiteral("vertical") || type == QStringLiteral("vertical_points")) {
    return QObject::tr("Vertical");
  }
  if (type == QStringLiteral("equal_spacing")) {
    return QObject::tr("Equal Spacing");
  }
  return type;
}

bool constrainSelection(SketchController& c, const QString& type) {
  QStringList picks;
  for (const SelectionItem& item : c.host().selection()) {
    if (item.owner == c.uid() &&
        (item.kind == SelectKind::SketchCurve || item.kind == SelectKind::SketchPoint)) {
      picks << item.name;
    }
  }
  if (picks.isEmpty()) {
    return false;
  }
  const QVector<QStringList> sets = selectionSets(type, picks, c.model());
  if (sets.isEmpty()) {
    return false;
  }
  const bool done = sets.size() == 1 ? apply(c, type, sets.first()) : applyEach(c, type, sets);
  if (done) {
    c.host().setSelection({});
  }
  return true;
}

std::unique_ptr<SketchTool> trimTool(SketchController& c) { return std::make_unique<TrimTool>(c, false); }

std::unique_ptr<SketchTool> extendTool(SketchController& c) { return std::make_unique<TrimTool>(c, true); }

std::unique_ptr<SketchTool> dimensionTool(SketchController& c) { return std::make_unique<DimensionTool>(c); }

std::unique_ptr<SketchTool> constraintTool(SketchController& c, const QString& type) {
  return std::make_unique<ConstraintTool>(c, type);
}

} // namespace mitcad::sketch
