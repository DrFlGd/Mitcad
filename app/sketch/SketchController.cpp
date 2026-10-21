// SPDX-License-Identifier: MIT
#include "SketchController.hpp"
#include "platform/NativeInlineEditor.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

#include <QAction>
#include <QCoreApplication>
#include <QJsonArray>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPushButton>
#include <QRegularExpression>
#include <QResizeEvent>
#include <QSettings>
#include <QToolTip>
#include <QtLogging>

#include "../OcctViewer.hpp"
#include "../framework/CommandRegistry.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Json.hpp"
#include "../framework/ModelShapes.hpp"
#include "../framework/Numbers.hpp"
#include "../framework/TestSync.hpp"
#include "SketchOverlay.hpp"
#include "SketchTool.hpp"

namespace mitcad::sketch {
namespace {

// A press that moves further than this starts a drag.
constexpr double kDragPixels = 4.0;
// Sketch geometry this near the cursor is under it.
constexpr double kPointPixels = 8.0;
constexpr double kCurvePixels = 6.0;

// The fields float over the view, whose background does not follow the
// palette: their own light background needs its own dark text.
const QString kFieldStyle = QStringLiteral(
    "QLineEdit { background: rgba(255, 255, 255, 240); color: #202428; border: 1px solid #7aa6dc;"
    " border-radius: 3px; padding: 1px 4px; selection-background-color: #7aa6dc;"
    " selection-color: #101418; }");
const QString kInvalidFieldStyle = QStringLiteral(
    "QLineEdit { background: rgba(255, 240, 240, 240); color: #202428; border: 1px solid #c02020;"
    " border-radius: 3px; padding: 1px 4px; selection-background-color: #7aa6dc;"
    " selection-color: #101418; }");

// A dimension's value or a constraint's glyph of sketch `uid` as a
// selection item ("fix:<entity>" for a fixed entity's lock).
SelectionItem annotationItem(const QString& uid, const SketchOverlay::Hit& hit) {
  SelectionItem item;
  item.kind = hit.kind == SketchOverlay::Hit::Kind::Dimension ? SelectKind::SketchDimension
                                                              : SelectKind::SketchConstraint;
  item.owner = uid;
  item.name = hit.kind == SketchOverlay::Hit::Kind::Fixed ? QStringLiteral("fix:") + hit.id : hit.id;
  return item;
}

double unitScale(const QString& unit) {
  if (unit == QStringLiteral("cm")) {
    return 10.0;
  }
  if (unit == QStringLiteral("m")) {
    return 1000.0;
  }
  if (unit == QStringLiteral("in")) {
    return 25.4;
  }
  if (unit == QStringLiteral("ft")) {
    return 304.8;
  }
  if (unit == QStringLiteral("um")) {
    return 0.001;
  }
  return 1.0;
}

QString kindKey(FieldSpec::Kind kind) {
  switch (kind) {
  case FieldSpec::Kind::Angle:
    return QStringLiteral("angle");
  case FieldSpec::Kind::Count:
    return QStringLiteral("unitless");
  case FieldSpec::Kind::Length:
  case FieldSpec::Kind::Text:
    break;
  }
  return QStringLiteral("length");
}

double screenDistance(const QPointF& a, const QPointF& b) {
  return std::hypot(a.x() - b.x(), a.y() - b.y());
}

QJsonArray xy(const V2& p) { return QJsonArray{p.x, p.y}; }

} // namespace

// ---------------------------------------------------------------------------
// SketchOp

SketchOp::SketchOp(SketchController& controller) : m_c(controller), m_depth(controller.undoDepth()) {}

SketchOp::~SketchOp() {
  if (!m_finished) {
    rollback();
  }
}

QJsonObject SketchOp::command(const QString& name) const {
  return {{QStringLiteral("cmd"), name}, {QStringLiteral("sketch"), m_c.uid()}};
}

bool SketchOp::run(const QJsonObject& command, QJsonObject* result) {
  try {
    const QJsonObject answer = m_c.runModel(command);
    if (m_label.isEmpty()) {
      m_label = m_c.undoLabel();
    }
    m_stale = true;
    noteImplied(command, answer);
    if (result != nullptr) {
      *result = answer;
    }
    return true;
  } catch (const std::exception& e) {
    m_error = errorText(e);
    qDebug().noquote() << QStringLiteral("Sketch command %1 refused: %2")
                              .arg(command.value(QStringLiteral("cmd")).toString(), m_error);
    return false;
  }
}

bool SketchOp::attempt(const QJsonObject& command) {
  const QString kept = m_error;
  const bool done = run(command);
  if (!done) {
    // A constraint a snap or an inference adds, or a typed dimension: the
    // action goes on without it, and says why (mitcad#287).
    const bool dimension = command.contains(QStringLiteral("dimension"));
    const QJsonObject item =
        command.value(dimension ? QStringLiteral("dimension") : QStringLiteral("constraint")).toObject();
    const QString type = item.value(QStringLiteral("type")).toString();
    QStringList on;
    for (auto it = item.begin(); it != item.end(); ++it) {
      if (it.key() != QStringLiteral("type") && it.value().isString()) {
        on << it.value().toString();
      }
    }
    qDebug().noquote() << QStringLiteral("%1 %2 on %3 not added: %4")
                              .arg(dimension ? QStringLiteral("Dimension") : QStringLiteral("Constraint"), type,
                                   on.join(QStringLiteral(", ")), m_error);
    const QString what = dimension ? QObject::tr("Dimension") : constraintName(type);
    m_notices << QObject::tr("%1 not added: %2").arg(what, m_error);
  }
  m_error = kept;
  return done;
}

void SketchOp::noteImplied(const QJsonObject& command, const QJsonObject& answer) {
  const QJsonArray implied = answer.value(QStringLiteral("implied_removed")).toArray();
  if (implied.isEmpty()) {
    return;
  }
  const QString type = command.value(QStringLiteral("constraint")).toObject().value(QStringLiteral("type")).toString();
  const QJsonArray made = answer.value(QStringLiteral("constraints")).toArray();
  const QString id = made.isEmpty() ? QString() : QStringLiteral(" ") + made.first().toString();
  for (const QJsonValue& v : implied) {
    const QJsonObject removed = v.toObject();
    const QString removedType = removed.value(QStringLiteral("type")).toString();
    qDebug().noquote() << QStringLiteral("Removed constraint %1 %2: implied by the new %3%4")
                              .arg(removedType, removed.value(QStringLiteral("constraint")).toString(), type, id);
    m_notices << QObject::tr("%1 removed: implied by the new %2")
                     .arg(constraintName(removedType), constraintName(type));
  }
}

QJsonValue SketchOp::pointInput(const Snap& snap) {
  if (snap.atPoint()) {
    return snap.point;
  }
  if (snap.kind == Snap::Kind::BodyVertex) {
    // A body vertex: the point its projection gives (mitcad#258).
    const QString projected = projectBody(snap.bodyItem).point;
    if (!projected.isEmpty()) {
      return projected;
    }
  }
  if (snap.kind == Snap::Kind::Origin) {
    const QString origin = model().originPoint();
    if (!origin.isEmpty()) {
      return origin;
    }
    // The origin is a fixed point of the sketch, made when something
    // first snaps there.
    QJsonObject cmd = command(QStringLiteral("sketch.add_point"));
    cmd.insert(QStringLiteral("at"), QJsonArray{0.0, 0.0});
    cmd.insert(QStringLiteral("fixed"), true);
    QJsonObject result;
    if (run(cmd, &result)) {
      const QJsonArray made = result.value(QStringLiteral("entities")).toArray();
      if (!made.isEmpty()) {
        return made.first().toString();
      }
    }
  }
  return xy(snap.at);
}

void SketchOp::constrain(const QString& point, const Snap& snap) {
  const auto coincident = [&](const QString& entity) {
    if (entity.isEmpty() || entity == point) {
      return;
    }
    addConstraint({{QStringLiteral("type"), QStringLiteral("coincident")},
                   {QStringLiteral("point"), point},
                   {QStringLiteral("entity"), entity}});
  };
  switch (snap.kind) {
  case Snap::Kind::Point:
  case Snap::Kind::Center:
    coincident(snap.point);
    break;
  case Snap::Kind::Origin: {
    const QJsonValue origin = pointInput(snap);
    coincident(origin.toString());
    break;
  }
  case Snap::Kind::Midpoint:
    addConstraint({{QStringLiteral("type"), QStringLiteral("midpoint")},
                   {QStringLiteral("point"), point},
                   {QStringLiteral("curve"), snap.curve}});
    break;
  case Snap::Kind::OnCurve:
    coincident(snap.curve);
    break;
  case Snap::Kind::Intersection:
    coincident(snap.curve);
    coincident(snap.curve2);
    break;
  case Snap::Kind::BodyVertex:
    // On the projected vertex, which follows the body (mitcad#258).
    coincident(projectBody(snap.bodyItem).point);
    break;
  case Snap::Kind::BodyMidpoint: {
    // At the middle of the projected edge.
    const QString edge = projectBody(snap.bodyItem).curve;
    if (!edge.isEmpty()) {
      addConstraint({{QStringLiteral("type"), QStringLiteral("midpoint")},
                     {QStringLiteral("point"), point},
                     {QStringLiteral("curve"), edge}});
    }
    break;
  }
  default:
    break;
  }
}

void SketchOp::addConstraint(const QJsonObject& constraint) {
  QJsonObject cmd = command(QStringLiteral("sketch.add_constraint"));
  cmd.insert(QStringLiteral("constraint"), constraint);
  attempt(cmd);
}

void SketchOp::addDimension(const QJsonObject& dimension, const QString& value) {
  QJsonObject cmd = command(QStringLiteral("sketch.add_dimension"));
  cmd.insert(QStringLiteral("dimension"), dimension);
  cmd.insert(QStringLiteral("value"), value);
  attempt(cmd);
}

ProjectedItem SketchOp::projectBody(const BodyItem& item) {
  const QString key = QStringList{item.source, item.body, item.linked ? item.occurrence : QStringLiteral("-")}.join(
      QLatin1Char('|'));
  if (const auto done = m_projected.constFind(key); done != m_projected.cend()) {
    return done.value();
  }
  ProjectedItem out;
  if (const auto existing = existingProjection(model(), item)) {
    out = *existing;
    qDebug().noquote() << QStringLiteral("Reused the projection of %1 of %2: %3")
                              .arg(item.describe(), item.body, out.point.isEmpty() ? out.curve : out.point);
  } else {
    // Linked, as Project makes it by default; Project's own command.
    QJsonObject cmd = command(QStringLiteral("sketch.project"));
    cmd.insert(QStringLiteral("source"), item.source);
    cmd.insert(QStringLiteral("body"), item.body);
    cmd.insert(QStringLiteral("linked"), true);
    if (item.linked) {
      cmd.insert(QStringLiteral("occurrence"), item.occurrence);
    }
    // The action's undo step is named after what it makes, not after the
    // projection that comes first.
    const bool first = m_label.isEmpty();
    QJsonObject result;
    if (!run(cmd, &result)) {
      return out;
    }
    if (first) {
      m_label.clear();
    }
    const SketchModel& after = model();
    for (const QJsonValue& value : result.value(QStringLiteral("entities")).toArray()) {
      const QString id = value.toString();
      if (item.isVertex() && out.point.isEmpty() && after.point(id) != nullptr) {
        out.point = id;
      } else if (!item.isVertex() && out.curve.isEmpty() && after.curve(id) != nullptr) {
        out.curve = id;
      }
    }
    qDebug().noquote() << QStringLiteral("Projected %1 of %2: %3")
                              .arg(item.describe(), item.body, out.point.isEmpty() ? out.curve : out.point);
  }
  if (!out.empty()) {
    m_projected.insert(key, out);
  }
  return out;
}

const SketchModel& SketchOp::model() {
  if (m_stale) {
    m_c.reload();
    m_stale = false;
  }
  return m_c.model();
}

bool SketchOp::commit() {
  m_finished = true;
  const int depth = m_c.undoDepth();
  if (depth > m_depth + 1) {
    try {
      m_c.runModel({{QStringLiteral("cmd"), QStringLiteral("merge_undo")},
                    {QStringLiteral("depth"), m_depth},
                    {QStringLiteral("label"), m_label}});
    } catch (const std::exception& e) {
      qWarning().noquote() << "Could not merge the undo steps:" << errorText(e);
    }
  }
  m_c.host().refreshScene();
  if (!m_notices.isEmpty()) {
    m_c.notice(m_notices.join(QStringLiteral("; ")));
  }
  return depth > m_depth;
}

void SketchOp::rollback() {
  m_finished = true;
  for (int guard = 0; guard < 64 && m_c.undoDepth() > m_depth; ++guard) {
    try {
      m_c.runModel({{QStringLiteral("cmd"), QStringLiteral("undo")}});
    } catch (const std::exception&) {
      break;
    }
  }
  m_c.host().refreshScene();
}

// ---------------------------------------------------------------------------
// SketchController

SketchController::SketchController(SketchHost& host, OcctViewer& viewer, CommandRegistry& registry, QObject* parent)
    : QObject(parent), m_registry(registry), m_host(host), m_viewer(viewer) {
  m_overlay = new SketchOverlay(*this, &viewer);
  m_overlay->hide();
  const QString dimensionText = QSettings().value(QStringLiteral("sketch/dimensionText")).toString();
  m_dimensionText = dimensionText == QStringLiteral("name")         ? DimensionText::Name
                    : dimensionText == QStringLiteral("name_value") ? DimensionText::NameValue
                                                                    : DimensionText::Value;
  viewer.installEventFilter(this);
  // After the frame, which may have moved the camera.
  connect(
      &viewer, &OcctViewer::viewChanged, m_overlay,
      [this] {
        if (!isActive()) {
          return;
        }
        m_overlay->update();
        placeFields();
        logView();
      },
      Qt::QueuedConnection);
  // Without a tool, the curve or point a click would select is drawn over
  // the view as the tools draw theirs: OCCT's highlight only recolours it,
  // in lines one pixel wide on macOS.
  connect(
      &viewer, &OcctViewer::hovered, this,
      [this](const SelectionItem& item) {
        if (!isActive() || m_tool) {
          return;
        }
        const bool ours = item.owner == m_uid && (item.kind == SelectKind::SketchCurve ||
                                                  item.kind == SelectKind::SketchPoint);
        setHover(ours ? item.name : QString());
      },
      Qt::QueuedConnection);
  // Shift widens what a click picks (mitcad#217): the dimension or
  // constraint under the pointer is highlighted as the click would take it.
  connect(&viewer, &OcctViewer::shiftChanged, this, [this](bool held) {
    if (!isActive() || m_tool || m_paused) {
      return;
    }
    SketchOverlay::Hit hit = m_overlay->hitTest(m_cursor);
    if (hit.valid() && !m_host.sketchClickable(annotationItem(m_uid, hit), held)) {
      hit = SketchOverlay::Hit();
    }
    if (hit.id != m_hoverAnnotation) {
      m_hoverAnnotation = hit.id;
      m_overlay->update();
    }
  });
  m_dragTimer.setSingleShot(true);
  connect(&m_dragTimer, &QTimer::timeout, this, &SketchController::applyDrag);
  TestSync::watch(&m_dragTimer);
  // The pointer's place is logged once the pointer rests (mitcad#254).
  m_pointerTimer.setSingleShot(true);
  m_pointerTimer.setInterval(150);
  connect(&m_pointerTimer, &QTimer::timeout, this, &SketchController::logPointer);
  TestSync::watch(&m_pointerTimer);
}

SketchController::~SketchController() = default;

void SketchController::enter(const QString& uid) {
  m_uid = uid;
  m_status.clear();
  m_loggedProjections.clear();
  m_loggedView.clear();
  m_loggedSpacings.clear();
  m_loggedPointer.clear();
  reload();
  // Where the sketch lies in the design, which the overlay shows (mitcad#254).
  const gp_Pnt& origin = m_model.frame.origin;
  const V2 designOrigin = toSketch(m_model.frame, gp_Pnt(0.0, 0.0, 0.0));
  qDebug().noquote() << QStringLiteral("Sketch origin at %1, %2, %3 %4 in the design; the design origin at %5 in the sketch")
                            .arg(formatLength(origin.X()), formatLength(origin.Y()), formatLength(origin.Z()), m_unit,
                                 pointerTextAt(designOrigin));
  // What its dimensions show as it opens, for UI tests (the signs of signed
  // distances, mitcad#409).
  if (!m_model.dimensions.empty()) {
    logDimensionTexts();
  }
  m_viewer.setSketchPlane(toAx3(m_model.frame));
  m_overlay->resize(m_viewer.size());
  m_overlay->show();
  m_overlay->raise();
}

void SketchController::leave() {
  stopTool();
  closeEditor();
  m_notice.clear();
  m_pressed = false;
  m_dragging = false;
  m_dragTimer.stop();
  m_textOverride.reset();
  m_pointerTimer.stop();
  m_pointerText.clear();
  m_movedPointer.clear();
  m_cursorValid = false;
  m_overlay->hide();
  m_uid.clear();
  m_model = SketchModel();
  ++m_modelRevision;
  m_body.clear();
  m_bodyKey.clear();
  m_hover.clear();
  m_picked.clear();
  m_hoverAnnotation.clear();
  emit changed();
}

void SketchController::reload() {
  if (!isActive()) {
    return;
  }
  try {
    m_model.load(m_host.queryObject(
        {{QStringLiteral("query"), QStringLiteral("sketch")}, {QStringLiteral("uid"), m_uid}}));
    const QJsonObject document = m_host.queryObject({{QStringLiteral("query"), QStringLiteral("document")}});
    const QJsonObject units = document.value(QStringLiteral("units")).toObject();
    // The same revision means the same definition (the body geometry read
    // for it holds).
    m_revision = QString::number(document.value(QStringLiteral("revision")).toDouble(), 'f', 0);
    m_unit = units.value(QStringLiteral("length")).toString(QStringLiteral("mm"));
    m_unitScale = unitScale(m_unit);
  } catch (const std::exception&) {
    // The sketch was just taken back; the main window leaves sketch mode.
  }
  ++m_modelRevision;
  const QString status = describeStatus();
  if (status != m_status) {
    m_status = status;
    qDebug().noquote() << status;
  }
  // For UI tests: the linked projections, when their number changes
  // (projected by Project, a dimension or a point; mitcad#256).
  const QString projections =
      QStringLiteral("Sketch %1: %2 linked projection(s)").arg(m_model.name).arg(m_model.projections.size());
  if (isActive() && projections != m_loggedProjections) {
    m_loggedProjections = projections;
    qDebug().noquote() << projections;
  }
  logSpacings();
  updateConvertAction();
  m_overlay->update();
  emit changed();
}

void SketchController::setSpacingPreview(const QStringList& anchors) {
  if (anchors == m_spacingPreview) {
    return;
  }
  m_spacingPreview = anchors;
  if (!anchors.isEmpty()) {
    qDebug().noquote() << QStringLiteral("Equal spacing preview: %1").arg(anchors.join(QStringLiteral(", ")));
  }
  if (m_overlay != nullptr) {
    m_overlay->update();
  }
}

void SketchController::setStalePreview(const QJsonObject& answer) {
  const auto point = [](const QJsonValue& value) {
    const QJsonArray xy = value.toArray();
    return V2{xy.at(0).toDouble(), xy.at(1).toDouble()};
  };
  // `stale` of sketch.clean_up_stale_projections (core/model/src/api/commands.md).
  const QJsonObject stale = answer.value(QStringLiteral("stale")).toObject();
  const QJsonObject shown = stale.value(QStringLiteral("preview")).toObject();
  StalePreview preview;
  QStringList removed;
  for (const QJsonValue& value : shown.value(QStringLiteral("removed")).toArray()) {
    const QJsonObject curve = value.toObject();
    preview.removed.push_back(curveFromJson(curve.value(QStringLiteral("type")).toString(),
                                            curve.value(QStringLiteral("geometry")).toObject()));
    removed << curve.value(QStringLiteral("curve")).toString();
  }
  for (const QJsonValue& value : stale.value(QStringLiteral("trimmed")).toArray()) {
    preview.trimmed << value.toString();
  }
  for (const QJsonValue& value : shown.value(QStringLiteral("cut")).toArray()) {
    const QJsonObject piece = value.toObject();
    preview.cut.emplace_back(point(piece.value(QStringLiteral("start"))), point(piece.value(QStringLiteral("end"))));
  }
  const bool was = !m_stalePreview.empty();
  const bool redraw = was || !preview.empty();
  m_stalePreview = std::move(preview);
  if (was && m_stalePreview.empty()) {
    qDebug().noquote() << QStringLiteral("Preview highlights: none");
  }
  if (!m_stalePreview.empty()) {
    // "Preview highlights: removed c8; trimmed c3, c5, c7", for UI tests.
    const QString none = QStringLiteral("none");
    qDebug().noquote() << QStringLiteral("Preview highlights: removed %1; trimmed %2")
                              .arg(removed.isEmpty() ? none : removed.join(QStringLiteral(", ")),
                                   m_stalePreview.trimmed.isEmpty()
                                       ? none
                                       : m_stalePreview.trimmed.join(QStringLiteral(", ")));
  }
  if (redraw && m_overlay != nullptr) {
    m_overlay->update();
  }
}

void SketchController::logSpacings() {
  // "Equal spacing k4 horizontal: p1, c2, c5 (spacing k6 = 12.00)" when a chain
  // changes, for UI tests: the order the model keeps.
  QHash<QString, QString> now;
  for (const ConstraintData& k : m_model.constraints) {
    if (k.type != QStringLiteral("equal_spacing")) {
      continue;
    }
    QString text = QStringLiteral("%1: %2").arg(k.json.value(QStringLiteral("mode")).toString(),
                                                 k.refs.join(QStringLiteral(", ")));
    const QString direction = k.json.value(QStringLiteral("direction")).toString();
    if (!direction.isEmpty()) {
      text += QStringLiteral(" along %1").arg(direction);
    }
    if (const DimensionData* d = m_model.spacingDimension(k.id)) {
      text += QStringLiteral(" (spacing %1 = %2)").arg(d->id, formatLength(d->measured));
    }
    now.insert(k.id, text);
    if (m_loggedSpacings.value(k.id) != text) {
      qDebug().noquote() << QStringLiteral("Equal spacing %1 %2").arg(k.id, text);
    }
  }
  for (auto it = m_loggedSpacings.cbegin(); it != m_loggedSpacings.cend(); ++it) {
    if (!now.contains(it.key())) {
      qDebug().noquote() << QStringLiteral("Equal spacing %1 removed").arg(it.key());
    }
  }
  m_loggedSpacings = now;
}

QString SketchController::describeStatus() const {
  QString text = QStringLiteral("Sketch %1: %2 DOF").arg(m_model.name).arg(m_model.dof);
  if (m_model.fullyConstrained()) {
    text += QStringLiteral(", fully constrained");
  }
  if (!m_model.conflicts.empty()) {
    QStringList sets;
    for (const QStringList& set : m_model.conflicts) {
      sets << set.join(QLatin1Char('+'));
    }
    text += QStringLiteral(", conflicts %1").arg(sets.join(QStringLiteral(", ")));
  }
  return text;
}

void SketchController::lookAt() { m_viewer.lookAtSketchPlane(false); }

void SketchController::logView() {
  // Where sketch points are in the window, for UI tests that click at
  // sketch coordinates.
  QStringList places;
  for (const V2& p : {V2{0.0, 0.0}, V2{100.0, 0.0}, V2{0.0, 100.0}}) {
    const QPointF at = toScreen(p);
    const QPoint window = m_viewer.mapTo(m_viewer.window(), QPoint(0, 0));
    places << QStringLiteral("%1,%2").arg(at.x() + window.x(), 0, 'f', 2).arg(at.y() + window.y(), 0, 'f', 2);
  }
  const QString line = QStringLiteral("Sketch view %1").arg(places.join(QLatin1Char(' ')));
  if (line != m_loggedView) {
    m_loggedView = line;
    qDebug().noquote() << line;
    // Where each dimension's text is (mitcad#231: hovered and clicked).
    const QPoint window = m_viewer.mapTo(m_viewer.window(), QPoint(0, 0));
    for (const DimensionData& d : m_model.dimensions) {
      const QPointF at = m_overlay->dimensionTextRect(d.id).center();
      qDebug().noquote() << QStringLiteral("Dimension %1 text at %2,%3")
                                .arg(d.id)
                                .arg(qRound(at.x() + window.x()))
                                .arg(qRound(at.y() + window.y()));
    }
  }
}

// ---------------------------------------------------------------------------
// Tools

void SketchController::startTool(std::unique_ptr<SketchTool> tool) {
  stopTool();
  m_tool = std::move(tool);
  m_viewer.setSketchInput(true, m_toolIcons.value(m_tool->name()));
  ++m_toolSession; // the body geometry is read again for it
  m_viewer.setPickFilter(SelectFilter());
  qDebug().noquote() << QStringLiteral("Sketch tool %1").arg(m_tool->name());
  emit toolChanged(m_tool->name());
  // A new auto constraint session (mitcad#189).
  m_auto.reset(m_tool->autoKind());
  m_autoTarget.clear();
  emit autoConstraintsChanged();
  m_tool->start();
  if (m_cursorValid) {
    updateCursor(m_cursor);
    m_tool->move(m_snap);
  }
  m_overlay->update();
}

void SketchController::stopTool() {
  // An editor's accept callback can capture the tool. Retire it before
  // releasing the tool, including a controller-owned dimension editor.
  closeEditor();
  if (!m_tool) {
    return;
  }
  // A tool may end itself from one of its own handlers: delete it later.
  SketchTool* retired = m_tool.release();
  QTimer::singleShot(0, this, [retired] { delete retired; });
  clearFields();
  m_hover.clear();
  m_picked.clear();
  m_snap = Snap();
  m_viewer.setSketchInput(false);
  emit toolChanged(QString());
  // Leaving the tool ends its auto constraint session.
  m_auto.reset();
  m_autoHover = false;
  m_autoTarget.clear();
  emit autoConstraintsChanged();
  m_overlay->update();
}

bool SketchController::confirm() {
  if (!isActive()) {
    return false;
  }
  if (m_editor) {
    acceptEditor();
    return true;
  }
  if (m_tool) {
    if (!m_tool->confirm()) {
      stopTool();
      hint(tr("Sketch: choose a tool, or %1.").arg(m_registry.shortcutHint(QStringLiteral("sketch.finish"))));
    }
    m_overlay->update();
    return true;
  }
  return false;
}

bool SketchController::cancel() {
  if (!isActive()) {
    return false;
  }
  if (m_editor) {
    // An editor the tool opened (the Dimension tool's value) sends the tool
    // back a step; one opened from a context menu (Edit Dimension, with
    // the id of its dimension) is only closed, and a running tool keeps
    // its state (mitcad#354).
    const bool openedByTool = m_editorDimension.isEmpty();
    closeEditor();
    if (openedByTool && m_tool) {
      m_tool->cancel();
    }
    m_overlay->update();
    return true;
  }
  if (m_dragging) {
    return true;
  }
  if (m_tool) {
    if (!m_tool->cancel()) {
      stopTool();
      hint(tr("Sketch: choose a tool, or %1.").arg(m_registry.shortcutHint(QStringLiteral("sketch.finish"))));
    }
    m_overlay->update();
    return true;
  }
  if (!m_host.selection().isEmpty()) {
    m_host.setSelection({});
    return true;
  }
  return false;
}

void SketchController::resetTool() {
  closeEditor();
  if (m_tool) {
    for (int guard = 0; guard < 16 && m_tool->cancel(); ++guard) {
    }
  }
  m_overlay->update();
}

void SketchController::deleteSelection() {
  QJsonArray items;
  QJsonArray unfix;
  for (const SelectionItem& item : m_host.selection()) {
    if (item.owner != m_uid) {
      continue;
    }
    switch (item.kind) {
    case SelectKind::SketchCurve:
    case SelectKind::SketchPoint:
    case SelectKind::SketchDimension:
      items.append(item.name);
      break;
    case SelectKind::SketchConstraint:
      if (item.name.startsWith(QStringLiteral("fix:"))) {
        unfix.append(item.name.mid(4));
      } else {
        items.append(item.name);
      }
      break;
    default:
      break;
    }
  }
  if (items.isEmpty() && unfix.isEmpty()) {
    hint(tr("Select sketch geometry, constraints or dimensions to delete."));
    return;
  }
  SketchOp op(*this);
  bool ok = true;
  if (!unfix.isEmpty()) {
    QJsonObject cmd = op.command(QStringLiteral("sketch.set_fixed"));
    cmd.insert(QStringLiteral("entities"), unfix);
    cmd.insert(QStringLiteral("fixed"), false);
    ok = op.run(cmd);
  }
  if (ok && !items.isEmpty()) {
    QJsonObject cmd = op.command(QStringLiteral("sketch.remove"));
    cmd.insert(QStringLiteral("items"), items);
    ok = op.run(cmd);
  }
  if (!ok) {
    const QString reason = op.error();
    op.rollback();
    error(tr("Could not delete: %1").arg(reason));
    return;
  }
  m_host.setSelection({});
  op.commit();
  QStringList names;
  for (const QJsonValue& v : items) {
    names << v.toString();
  }
  for (const QJsonValue& v : unfix) {
    names << QStringLiteral("fix:") + v.toString();
  }
  qDebug().noquote() << QStringLiteral("Deleted %1").arg(names.join(QStringLiteral(", ")));
}

void SketchController::toggleConstruction() {
  QJsonArray curves;
  bool allConstruction = true;
  for (const SelectionItem& item : m_host.selection()) {
    if (item.owner == m_uid && item.kind == SelectKind::SketchCurve) {
      curves.append(item.name);
      const CurveData* c = m_model.curve(item.name);
      allConstruction = allConstruction && c != nullptr && c->construction;
    }
  }
  if (curves.isEmpty() || m_tool) {
    setConstruction(!m_construction);
    hint(m_construction ? tr("New geometry is construction geometry (X).")
                        : tr("New geometry is normal geometry (X)."));
    return;
  }
  SketchOp op(*this);
  QJsonObject cmd = op.command(QStringLiteral("sketch.set_construction"));
  cmd.insert(QStringLiteral("curves"), curves);
  cmd.insert(QStringLiteral("construction"), !allConstruction);
  if (!op.run(cmd)) {
    const QString reason = op.error();
    op.rollback();
    error(reason);
    return;
  }
  op.commit();
  qDebug().noquote() << QStringLiteral("Construction %1: %2")
                            .arg(allConstruction ? QStringLiteral("off") : QStringLiteral("on"),
                                 QJsonDocument(curves).toJson(QJsonDocument::Compact));
}

void SketchController::toggleCenterline() {
  QJsonArray lines;
  bool all = true;
  for (const SelectionItem& item : m_host.selection()) {
    const CurveData* c = m_model.curve(item.name);
    if (item.owner == m_uid && item.kind == SelectKind::SketchCurve && c != nullptr && c->isLine()) {
      lines.append(item.name);
      all = all && c->centerline;
    }
  }
  if (lines.isEmpty()) {
    hint(tr("Centerline: select sketch lines first."));
    return;
  }
  SketchOp op(*this);
  if (!all) {
    // A centre line is not construction geometry.
    QJsonArray construction;
    for (const QJsonValue& line : lines) {
      const CurveData* c = m_model.curve(line.toString());
      if (c != nullptr && c->construction) {
        construction.append(line);
      }
    }
    if (!construction.isEmpty()) {
      QJsonObject cmd = op.command(QStringLiteral("sketch.set_construction"));
      cmd.insert(QStringLiteral("curves"), construction);
      cmd.insert(QStringLiteral("construction"), false);
      op.run(cmd);
    }
  }
  QJsonObject cmd = op.command(QStringLiteral("sketch.set_centerline"));
  cmd.insert(QStringLiteral("lines"), lines);
  cmd.insert(QStringLiteral("centerline"), !all);
  if (!op.run(cmd)) {
    const QString reason = op.error();
    op.rollback();
    error(reason);
    return;
  }
  op.commit();
  qDebug().noquote() << QStringLiteral("Centerline %1: %2")
                            .arg(all ? QStringLiteral("off") : QStringLiteral("on"),
                                 QJsonDocument(lines).toJson(QJsonDocument::Compact));
}

bool SketchController::selectionHasProjected() const {
  if (!isActive()) {
    return false;
  }
  return std::any_of(m_host.selection().begin(), m_host.selection().end(), [this](const SelectionItem& item) {
    if (item.owner != m_uid) {
      return false;
    }
    const PointData* p = m_model.point(item.name);
    const CurveData* c = m_model.curve(item.name);
    return (p != nullptr && p->reference) || (c != nullptr && c->reference);
  });
}

void SketchController::updateConvertAction() {
  // The command is enabled while the selection holds projected geometry:
  // the main window refreshes the commands on model changes, not on each
  // selection change.
  if (QAction* action = m_registry.action(QStringLiteral("sketch.convert_projected"))) {
    action->setEnabled(selectionHasProjected());
  }
}

void SketchController::convertProjected() {
  QJsonArray entities;
  for (const SelectionItem& item : m_host.selection()) {
    if (item.owner == m_uid && (item.kind == SelectKind::SketchCurve || item.kind == SelectKind::SketchPoint)) {
      entities.append(item.name);
    }
  }
  if (entities.isEmpty()) {
    hint(tr("Convert to Sketch Geometry: select projected curves or points first."));
    return;
  }
  SketchOp op(*this);
  QJsonObject cmd = op.command(QStringLiteral("sketch.convert_projected"));
  cmd.insert(QStringLiteral("entities"), entities);
  QJsonObject result;
  if (!op.run(cmd, &result)) {
    const QString reason = op.error();
    op.rollback();
    error(tr("Convert to Sketch Geometry: %1").arg(reason));
    return;
  }
  op.commit();
  QStringList converted;
  for (const QJsonValue& v : result.value(QStringLiteral("converted")).toArray()) {
    converted << v.toString();
  }
  QStringList kept;
  for (const QJsonValue& v : result.value(QStringLiteral("kept")).toArray()) {
    kept << v.toString();
  }
  QString line = QStringLiteral("Converted to sketch geometry: %1").arg(converted.join(QStringLiteral(", ")));
  if (!kept.isEmpty()) {
    line += QStringLiteral("; kept projected: %1").arg(kept.join(QStringLiteral(", ")));
  }
  qDebug().noquote() << line;
  QStringList warnings;
  for (const QJsonValue& v : result.value(QStringLiteral("warnings")).toArray()) {
    warnings << v.toString();
  }
  if (!warnings.isEmpty()) {
    hint(tr("Convert to Sketch Geometry: %1").arg(warnings.join(QStringLiteral("; "))));
  }
  updateConvertAction();
}

void SketchController::toggleFixed() {
  QJsonArray entities;
  bool all = true;
  for (const SelectionItem& item : m_host.selection()) {
    if (item.owner != m_uid) {
      continue;
    }
    if (item.kind == SelectKind::SketchPoint) {
      const PointData* p = m_model.point(item.name);
      entities.append(item.name);
      all = all && p != nullptr && p->fixed;
    } else if (item.kind == SelectKind::SketchCurve) {
      const CurveData* c = m_model.curve(item.name);
      entities.append(item.name);
      all = all && c != nullptr && c->fixed;
    } else if (item.kind == SelectKind::SketchConstraint && item.name.startsWith(QStringLiteral("fix:"))) {
      entities.append(item.name.mid(4));
    }
  }
  if (entities.isEmpty()) {
    return;
  }
  SketchOp op(*this);
  QJsonObject cmd = op.command(QStringLiteral("sketch.set_fixed"));
  cmd.insert(QStringLiteral("entities"), entities);
  cmd.insert(QStringLiteral("fixed"), !all);
  if (!op.run(cmd)) {
    const QString reason = op.error();
    op.rollback();
    error(reason);
    return;
  }
  op.commit();
  qDebug().noquote() << QStringLiteral("%1 %2")
                            .arg(all ? QStringLiteral("Unfixed") : QStringLiteral("Fixed"),
                                 QJsonDocument(entities).toJson(QJsonDocument::Compact));
}

void SketchController::editDimension(const QString& id) {
  const DimensionData* d = m_model.dimension(id);
  if (d == nullptr) {
    return;
  }
  const QRectF rect = m_overlay->dimensionTextRect(id);
  const FieldSpec::Kind kind =
      d->type == QStringLiteral("angle") ? FieldSpec::Kind::Angle : FieldSpec::Kind::Length;
  QString text = d->expression;
  if (text.isEmpty() || d->driven) {
    text = kind == FieldSpec::Kind::Angle ? QString::number(d->measured * 180.0 / kPi, 'g', 10)
                                          : QString::number(d->measured / m_unitScale, 'g', 10);
  }
  qDebug().noquote() << QStringLiteral("Editing dimension %1 (%2)").arg(id, text);
  openValueEditor(
      rect.center(), text, kind,
      [this, id](const QString& expression) {
        SketchOp op(*this);
        QJsonObject cmd = op.command(QStringLiteral("sketch.set_dimension"));
        cmd.insert(QStringLiteral("dimension"), id);
        cmd.insert(QStringLiteral("value"), expression);
        if (!op.run(cmd)) {
          const QString reason = op.error();
          op.rollback();
          error(reason);
          return;
        }
        op.commit();
        const DimensionData* changed = m_model.dimension(id);
        qDebug().noquote() << QStringLiteral("Dimension %1 = %2 (%3)")
                                  .arg(id, expression, changed != nullptr ? changed->parameter : QString());
      },
      d->driven ? QString() : d->parameter);
  m_editorDimension = id;
  // What the dimension is, as its tooltip says (mitcad#231).
  if (m_editor) {
    const QString about = m_overlay->dimensionTooltip(*d);
    m_editor->setToolTip(about + QLatin1Char('\n') + m_editor->toolTip());
    qDebug().noquote() << QStringLiteral("Sketch value editor tooltip: %1").arg(about);
  }
}

void SketchController::setDimensionSigned(const QString& id, bool on) {
  const DimensionData* d = m_model.dimension(id);
  if (d == nullptr || !d->isAxisDistance() || d->signedOffset == on) {
    return;
  }
  SketchOp op(*this);
  QJsonObject cmd = op.command(QStringLiteral("sketch.set_signed"));
  cmd.insert(QStringLiteral("dimension"), id);
  cmd.insert(QStringLiteral("signed"), on);
  if (!op.run(cmd)) {
    const QString reason = op.error();
    op.rollback();
    error(reason);
    return;
  }
  op.commit();
  const DimensionData* changed = m_model.dimension(id);
  // "Dimension k5 signed: fx: -15.00 (-(r * cos(a)))", for UI tests.
  qDebug().noquote() << QStringLiteral("Dimension %1 %2: %3 (%4)")
                            .arg(id, on ? QStringLiteral("signed") : QStringLiteral("unsigned"),
                                 changed != nullptr ? m_overlay->dimensionText(*changed) : QString(),
                                 changed != nullptr ? changed->expression : QString());
}

// ---------------------------------------------------------------------------
// Options

void SketchController::setConstruction(bool on) {
  if (on != m_construction) {
    m_construction = on;
    qDebug().noquote() << QStringLiteral("Construction mode %1").arg(on ? QStringLiteral("on")
                                                                         : QStringLiteral("off"));
    emit optionsChanged();
    m_overlay->update();
  }
}

void SketchController::setSignedDistances(bool on) {
  if (on != m_signedDistances) {
    m_signedDistances = on;
    qDebug().noquote() << QStringLiteral("Signed distances %1").arg(on ? QStringLiteral("on") : QStringLiteral("off"));
    emit optionsChanged();
    m_overlay->update();
  }
}

void SketchController::setGridSnap(bool on) {
  if (on == m_gridSnap) {
    return;
  }
  m_gridSnap = on;
  emit optionsChanged();
}

void SketchController::setShowDimensions(bool on) {
  m_showDimensions = on;
  emit optionsChanged();
  m_overlay->update();
}

void SketchController::setShowConstraints(bool on) {
  m_showConstraints = on;
  emit optionsChanged();
  m_overlay->update();
}

void SketchController::setShowProfiles(bool on) {
  m_showProfiles = on;
  emit optionsChanged();
  m_host.refreshScene();
}

void SketchController::setHideAbove(bool on) {
  if (on == m_hideAbove) {
    return;
  }
  m_hideAbove = on;
  qDebug().noquote() << QStringLiteral("Hide Above Sketch %1").arg(on ? QStringLiteral("on") : QStringLiteral("off"));
  emit optionsChanged();
  m_host.refreshScene();
}

namespace {

const char* dimensionTextKey(SketchController::DimensionText mode) {
  switch (mode) {
  case SketchController::DimensionText::Name:
    return "name";
  case SketchController::DimensionText::NameValue:
    return "name_value";
  case SketchController::DimensionText::Value:
    break;
  }
  return "value";
}

} // namespace

void SketchController::setDimensionTextMode(DimensionText mode) {
  if (mode == m_dimensionText) {
    return;
  }
  m_dimensionText = mode;
  QSettings().setValue(QStringLiteral("sketch/dimensionText"), QString::fromLatin1(dimensionTextKey(mode)));
  qDebug().noquote() << QStringLiteral("Dimension text %1").arg(QString::fromLatin1(dimensionTextKey(mode)));
  emit optionsChanged();
  m_overlay->update();
  logDimensionTexts();
}

void SketchController::logDimensionTexts() const {
  QStringList texts;
  for (const DimensionData& d : m_model.dimensions) {
    texts << QStringLiteral("%1 %2").arg(d.id, m_overlay->dimensionText(d));
  }
  qDebug().noquote() << QStringLiteral("Dimension texts: %1").arg(texts.join(QStringLiteral(", ")));
}

void SketchController::showDimensionTooltip(const QString& dimension, const QPointF& at) {
  if (dimension == m_tooltipDimension) {
    return;
  }
  m_tooltipDimension = dimension;
  const DimensionData* d = dimension.isEmpty() ? nullptr : m_model.dimension(dimension);
  if (d == nullptr) {
    QToolTip::hideText();
    return;
  }
  const QString text = m_overlay->dimensionTooltip(*d);
  QToolTip::showText(m_viewer.mapToGlobal(at.toPoint()), text, &m_viewer);
  qDebug().noquote() << QStringLiteral("Dimension tooltip %1: %2").arg(dimension, text);
}

// ---------------------------------------------------------------------------
// Auto constraints (mitcad#189)

namespace {

// A line, circle or arc of the sketch as the auto constraints see it.
std::optional<AutoEntity> autoEntity(const SketchModel& model, const QString& id) {
  const CurveData* c = model.curve(id);
  if (c == nullptr) {
    return std::nullopt;
  }
  AutoEntity entity;
  entity.id = id;
  if (c->isLine()) {
    entity.kind = AutoKind::Line;
    entity.a = c->curve.a;
    entity.b = c->curve.b;
  } else if (c->isCircular()) {
    entity.kind = AutoKind::Circular;
    entity.center = c->curve.center;
    entity.radius = c->curve.radius;
    entity.centerPoint = c->center;
  } else {
    return std::nullopt;
  }
  return entity;
}

} // namespace

void SketchController::setAutoLink(AutoConstraintSession::Link link) {
  if (link == m_auto.link()) {
    return;
  }
  m_auto.setLink(link);
  qDebug().noquote() << QStringLiteral("Auto constraint link: %1")
                            .arg(link == AutoConstraintSession::Link::First ? QStringLiteral("first")
                                                                            : QStringLiteral("last"));
  emit autoConstraintsChanged();
  m_overlay->update();
}

void SketchController::setAutoConstraint(const QString& type, bool on) {
  if (!m_auto.setChecked(type, on)) {
    return;
  }
  const QStringList checked = m_auto.checkedTypes();
  qDebug().noquote() << QStringLiteral("Auto constraint %1: %2 (checked: %3)")
                            .arg(type, on ? QStringLiteral("on") : QStringLiteral("off"),
                                 checked.isEmpty() ? QStringLiteral("none") : checked.join(QStringLiteral(", ")));
  emit autoConstraintsChanged();
  // The preview of the entity being drawn follows the choice.
  if (m_tool && m_cursorValid) {
    m_tool->move(m_snap);
  }
  m_overlay->update();
}

QString SketchController::autoReference() const {
  if (m_auto.kind() == AutoKind::None) {
    return {};
  }
  return m_auto.reference([this](const QString& id) { return m_model.curve(id) != nullptr; });
}

bool SketchController::autoConstraintsShown() const {
  return m_tool && m_auto.kind() != AutoKind::None && !autoReference().isEmpty();
}

std::optional<AutoPlan> SketchController::autoPlan() const {
  const QStringList types = m_auto.checkedTypes();
  if (types.isEmpty()) {
    return std::nullopt;
  }
  const QString id = autoReference();
  const auto reference = autoEntity(m_model, id);
  if (!reference) {
    return std::nullopt;
  }
  if (id != m_autoTarget) {
    m_autoTarget = id;
    qDebug().noquote() << QStringLiteral("Auto constraint target %1").arg(id);
  }
  return AutoPlan{*reference, types};
}

void SketchController::setAutoHover(bool on) {
  if (on == m_autoHover) {
    return;
  }
  m_autoHover = on;
  const QString reference = autoReference();
  if (!reference.isEmpty()) {
    qDebug().noquote() << (on ? QStringLiteral("Auto constraint reference highlighted: %1").arg(reference)
                              : QStringLiteral("Auto constraint reference highlight off"));
  }
  m_overlay->update();
}

bool SketchController::applyAutoConstraints(SketchOp& op, const QString& made, const QString& what) {
  const QStringList types = m_auto.checkedTypes();
  if (types.isEmpty() || made.isEmpty()) {
    return true;
  }
  const SketchModel& model = op.model();
  // The new entity may have the id of one undo took back: that one is
  // not the reference.
  const QString id = m_auto.reference(
      [&model, &made](const QString& entity) { return entity != made && model.curve(entity) != nullptr; });
  if (id.isEmpty()) {
    return true;
  }
  const auto reference = autoEntity(model, id);
  const auto entity = autoEntity(model, made);
  if (!reference || !entity) {
    return true;
  }
  QStringList failures;
  for (const QString& type : types) {
    const QString label = autoConstraintLabel(type);
    if (autoConstraintShared(type, *reference, *entity)) {
      qDebug().noquote() << QStringLiteral("Auto constraint %1 on %2, %3: one centre point").arg(type, id, made);
      continue;
    }
    const QJsonObject definition = autoConstraintDefinition(type, *reference, *entity);
    if (definition.isEmpty()) {
      failures << tr("%1: %2 and %3 do not take it").arg(label, id, made);
      qDebug().noquote() << QStringLiteral("Auto constraint %1 on %2, %3 does not fit").arg(type, id, made);
      continue;
    }
    QJsonObject cmd = op.command(QStringLiteral("sketch.add_constraint"));
    cmd.insert(QStringLiteral("constraint"), definition);
    QJsonObject result;
    if (op.run(cmd, &result)) {
      qDebug().noquote() << QStringLiteral("Auto constraint %1 %2 on %3, %4")
                                .arg(definition.value(QStringLiteral("type")).toString(),
                                     result.value(QStringLiteral("constraints")).toArray().first().toString(), id,
                                     made);
    } else {
      failures << QStringLiteral("%1: %2").arg(label, op.error());
      qDebug().noquote() << QStringLiteral("Auto constraint %1 on %2, %3 refused: %4").arg(type, id, made, op.error());
    }
  }
  if (failures.isEmpty()) {
    return true;
  }
  // The new entity is shown while the user decides.
  op.model();
  m_overlay->update();
  qDebug().noquote() << QStringLiteral("Auto constraint dialog: %1").arg(failures.join(QStringLiteral("; ")));
  QMessageBox box(QMessageBox::Warning, tr("Auto Constraint"),
                  tr("These auto constraints could not be added to the new %1:\n\n%2\n\n"
                     "Accept keeps the %1 without them; Cancel takes it back.")
                      .arg(what, failures.join(QLatin1Char('\n'))),
                  QMessageBox::Cancel, m_viewer.window());
  QPushButton* accept = box.addButton(tr("Accept"), QMessageBox::AcceptRole);
  box.setDefaultButton(accept);
  box.setEscapeButton(QMessageBox::Cancel);
  prepareModal(&box);
  execDialog(&box);
  const bool accepted = box.clickedButton() == accept;
  qDebug().noquote() << (accepted ? QStringLiteral("Auto constraint accepted: %1 kept without them").arg(made)
                                  : QStringLiteral("Auto constraint cancelled: %1 taken back").arg(made));
  return accepted;
}

void SketchController::recordAutoEntity(const QString& made) {
  if (m_auto.kind() == AutoKind::None || made.isEmpty()) {
    return;
  }
  m_auto.record(made, [this](const QString& id) { return m_model.curve(id) != nullptr; });
  qDebug().noquote() << QStringLiteral("Auto constraint entities: %1").arg(m_auto.entities().join(QStringLiteral(", ")));
  emit autoConstraintsChanged();
}

// ---------------------------------------------------------------------------
// View

QPointF SketchController::toScreen(const V2& p) const {
  return m_viewer.toWidgetF(toModel(m_model.frame, p));
}

std::optional<V2> SketchController::sketchAt(const QPointF& screen) const {
  if (const auto p = m_viewer.sketchPlanePoint(screen)) {
    return V2{p->X(), p->Y()};
  }
  return std::nullopt;
}

double SketchController::pixel() const {
  const QPointF o = toScreen(V2{});
  const double sx = screenDistance(toScreen(V2{1.0, 0.0}), o);
  const double sy = screenDistance(toScreen(V2{0.0, 1.0}), o);
  const double perMm = std::max(sx, sy);
  return perMm > 1e-9 ? 1.0 / perMm : 1.0;
}

QString SketchController::entityAt(const QPointF& screen, unsigned filter) const {
  QString best;
  double bestDistance = std::numeric_limits<double>::max();
  // Body geometry in the plane (mitcad#256) counts as this much further
  // than it is: the sketch's own entity at the same place wins.
  constexpr double kBodyTiePixels = 1.0;
  const bool body = (filter & PickBody) != 0;
  if (filter & PickPoints) {
    for (const PointData& p : m_model.points) {
      const double d = screenDistance(toScreen(p.at), screen);
      if (d <= kPointPixels && d < bestDistance) {
        best = p.id;
        bestDistance = d;
      }
    }
    for (const auto& [id, at] : body ? m_body.points() : std::vector<std::pair<QString, V2>>()) {
      const double d = screenDistance(toScreen(at), screen);
      if (d <= kPointPixels && d + kBodyTiePixels < bestDistance) {
        best = id;
        bestDistance = d + kBodyTiePixels;
      }
    }
    if (!best.isEmpty()) {
      return best;
    }
  }
  if (filter & PickCurves) {
    const auto raw = sketchAt(screen);
    if (!raw) {
      return best;
    }
    const double px = pixel();
    for (const CurveData& c : m_model.curves) {
      const double d = c.curve.distanceTo(*raw) / px;
      if (d <= kCurvePixels && d < bestDistance) {
        best = c.id;
        bestDistance = d;
      }
    }
    for (std::size_t i = 0; body && i < m_body.items().size(); ++i) {
      const BodyItem& item = m_body.items()[i];
      if (item.isVertex()) {
        continue;
      }
      const double d = item.curve.distanceTo(*raw) / px;
      if (d <= kCurvePixels && d + kBodyTiePixels < bestDistance) {
        best = BodyGeometry::itemId(i);
        bestDistance = d + kBodyTiePixels;
      }
    }
  }
  return best;
}

const BodyGeometry& SketchController::bodyGeometry() {
  if (!isActive()) {
    return m_body;
  }
  const QString key = QStringLiteral("%1 %2 %3").arg(m_uid, m_revision).arg(m_toolSession);
  // Queries wait while a job computes the model: the last read until then.
  if (key == m_bodyKey || m_host.modelBusy()) {
    return m_body;
  }
  m_bodyKey = key;
  try {
    m_body.load(m_host.query({{QStringLiteral("query"), QStringLiteral("plane_geometry")},
                              {QStringLiteral("sketch"), m_uid}})
                    .toArray(),
                &curveFromJson);
  } catch (const std::exception& e) {
    m_body.clear();
    qDebug().noquote() << QStringLiteral("Sketch plane geometry not read: %1").arg(errorText(e));
    return m_body;
  }
  // For UI tests: what the tools can pick and snap to.
  qDebug().noquote() << QStringLiteral("Sketch plane geometry: %1 edge(s), %2 vertex(es)")
                            .arg(m_body.edgeCount())
                            .arg(m_body.vertexCount());
  return m_body;
}

void SketchController::setHover(const QString& entity) {
  if (entity != m_hover) {
    m_hover = entity;
    m_overlay->update();
  }
}

void SketchController::setPicked(const QStringList& entities) {
  m_picked = entities;
  m_overlay->update();
}

void SketchController::refreshView() {
  m_overlay->update();
  updateConvertAction();
}

void SketchController::hint(const QString& message) {
  if (m_notice.isEmpty()) {
    m_host.showHint(message);
    return;
  }
  // The prompt that follows a notice keeps it in front, once.
  m_host.showHint(QStringLiteral("%1 %2").arg(m_notice, message));
  m_notice.clear();
}

void SketchController::notice(const QString& message) {
  m_notice = message.endsWith(QLatin1Char('.')) ? message : message + QLatin1Char('.');
  qDebug().noquote() << QStringLiteral("Sketch notice: %1").arg(m_notice);
  m_host.showHint(m_notice);
}

void SketchController::error(const QString& message) {
  m_notice.clear();
  m_host.showError(message);
}

void SketchController::updateCursor(const QPointF& position) {
  m_cursor = position;
  const auto raw = sketchAt(position);
  m_cursorValid = raw.has_value();
  if (!raw) {
    setPointerText(QString());
    return;
  }
  if (m_tool && m_tool->snaps()) {
    SnapSettings settings;
    settings.model = &m_model;
    settings.toScreen = [this](const V2& p) { return toScreen(p); };
    settings.grid = m_gridSnap;
    settings.gridStep = m_gridStep > 0.0 ? m_gridStep : m_viewer.gridStep(); // the lines shown
    settings.body = m_tool->bodySnaps() ? &bodyGeometry() : nullptr;
    m_snap = snap(settings, *raw, position);
  } else {
    m_snap = Snap();
    m_snap.at = *raw;
    m_snap.screen = position;
  }
  // Snapped to a point (not onto a curve or the grid): the cursor shows it.
  m_viewer.setSketchSnapping(m_tool && m_snap.onGeometry() && m_snap.kind != Snap::Kind::OnCurve);
  setPointerText(pointerTextAt(m_snap.at));
}

QString SketchController::pointerTextAt(const V2& at) const {
  const auto coordinate = [this](double mm) {
    const QString text = formatLength(mm);
    return text == QStringLiteral("-0.00") ? QStringLiteral("0.00") : text;
  };
  return QStringLiteral("x %1, y %2 %3").arg(coordinate(at.x), coordinate(at.y), m_unit);
}

void SketchController::setPointerText(const QString& text) {
  if (text == m_pointerText) {
    return;
  }
  m_pointerText = text;
  m_overlay->update();
  if (!text.isEmpty()) {
    m_movedPointer = text;
    m_pointerTimer.start(); // again: logged once the pointer rests
  }
}

void SketchController::logPointer() {
  // For UI tests: what the view showed for the place the pointer was moved
  // to last, also when it has left the view since. Moves posted to the
  // window (the Windows UI tests in session 0, where no real pointer is
  // over it) are followed by a Leave at once, which hides the place before
  // the pointer rests.
  if (!isActive() || m_movedPointer.isEmpty() || m_movedPointer == m_loggedPointer) {
    return;
  }
  m_loggedPointer = m_movedPointer;
  qDebug().noquote() << QStringLiteral("Sketch pointer %1").arg(m_movedPointer);
}

// ---------------------------------------------------------------------------
// Mouse

bool SketchController::eventFilter(QObject* watched, QEvent* event) {
  if (watched == &m_viewer) {
    return viewerEvent(event);
  }
  if (auto* edit = qobject_cast<QLineEdit*>(watched)) {
    return fieldEvent(edit, event);
  }
  return QObject::eventFilter(watched, event);
}

bool SketchController::viewerEvent(QEvent* event) {
  if (event->type() == QEvent::Resize) {
    m_overlay->resize(static_cast<QResizeEvent*>(event)->size());
    return false;
  }
  if (event->type() == QEvent::WindowBlocked) {
    interruptDrag();
    return false;
  }
  // The pointer's place shows also while a command's panel runs in the
  // sketch (mitcad#254).
  if (isActive() && m_paused && !m_forwarding) {
    if (event->type() == QEvent::MouseMove) {
      const auto at = sketchAt(static_cast<QMouseEvent*>(event)->position());
      setPointerText(at ? pointerTextAt(*at) : QString());
    } else if (event->type() == QEvent::Leave) {
      setPointerText(QString());
    }
  }
  if (!isActive() || m_forwarding || m_paused) {
    return false;
  }
  // The orientation cube is the view's: clicks turn the view, drags orbit (U5).
  if (event->type() == QEvent::MouseButtonPress &&
      static_cast<QMouseEvent*>(event)->button() == Qt::LeftButton &&
      m_viewer.isOnOrientationCube(static_cast<QMouseEvent*>(event)->position())) {
    return false;
  }
  if ((event->type() == QEvent::MouseMove || event->type() == QEvent::MouseButtonRelease) &&
      m_viewer.cubeInteraction()) {
    return false;
  }
  // The dimension or constraint under the pointer, when the selection
  // filter lets a click take it (with Shift: its empty rows too; mitcad#217).
  const auto clickableHit = [this](const QPointF& position, Qt::KeyboardModifiers modifiers) {
    SketchOverlay::Hit hit = m_overlay->hitTest(position);
    if (hit.valid() &&
        !m_host.sketchClickable(annotationItem(m_uid, hit), modifiers.testFlag(Qt::ShiftModifier))) {
      hit = SketchOverlay::Hit();
    }
    return hit;
  };
  switch (event->type()) {
  case QEvent::Leave:
    m_cursorValid = false;
    setPointerText(QString());
    showDimensionTooltip(QString(), QPointF());
    if (!m_hoverAnnotation.isEmpty()) {
      m_hoverAnnotation.clear();
    }
    m_overlay->update();
    return false;
  case QEvent::MouseMove: {
    auto* e = static_cast<QMouseEvent*>(event);
    updateCursor(e->position());
    if (m_pressed && !m_dragging && screenDistance(e->position(), m_pressPosition) > kDragPixels) {
      startDrag();
    }
    if (m_dragging) {
      if (m_cursorValid) {
        m_dragTarget = m_snap.at;
        m_dragPending = true;
        if (!m_dragTimer.isActive()) {
          m_dragTimer.start(0);
        }
      }
      return true;
    }
    if (m_tool) {
      m_tool->move(m_snap);
      placeFields();
      m_overlay->update();
      return (e->buttons() & Qt::LeftButton) != 0;
    }
    const SketchOverlay::Hit hit = clickableHit(e->position(), e->modifiers());
    if (hit.id != m_hoverAnnotation) {
      m_hoverAnnotation = hit.id;
      m_overlay->update();
    }
    const SketchOverlay::Hit under = m_overlay->hitTest(e->position());
    showDimensionTooltip(under.kind == SketchOverlay::Hit::Kind::Dimension ? under.id : QString(), e->position());
    return m_pressed;
  }
  case QEvent::MouseButtonPress: {
    auto* e = static_cast<QMouseEvent*>(event);
    if (e->button() == Qt::RightButton) {
      m_rightPressPosition = e->position(); // a click or a drag of the view
      return false;
    }
    if (e->button() != Qt::LeftButton) {
      return false;
    }
    if (m_editor) {
      // A click on another dimension puts its name into the expression
      // (mitcad#231); elsewhere it accepts.
      const SketchOverlay::Hit hit = m_overlay->hitTest(e->position());
      if (hit.kind == SketchOverlay::Hit::Kind::Dimension) {
        const DimensionData* d = m_model.dimension(hit.id);
        if (hit.id != m_editorDimension && d != nullptr && !d->parameter.isEmpty()) {
          insertIntoEditor(d->parameter);
        }
        return true;
      }
      acceptEditor();
      return true;
    }
    updateCursor(e->position());
    if (m_tool) {
      if (m_cursorValid) {
        m_tool->press(m_snap);
      }
      placeFields();
      m_overlay->update();
      return true;
    }
    const SketchOverlay::Hit hit = clickableHit(e->position(), e->modifiers());
    if (hit.valid()) {
      if (hit.kind == SketchOverlay::Hit::Kind::Dimension) {
        selectAnnotation(QStringLiteral("dimension"), hit.id, e->modifiers());
        m_pressed = true;
        m_dragDimension = hit.id;
        m_pressPosition = e->position();
      } else {
        selectAnnotation(QStringLiteral("constraint"),
                         hit.kind == SketchOverlay::Hit::Kind::Fixed ? QStringLiteral("fix:") + hit.id
                                                                       : hit.id,
                         e->modifiers());
      }
      return true;
    }
    const QString entity = entityAt(e->position());
    // Geometry the selection filter leaves empty is neither dragged nor
    // clicked here: the press is the view's (a window, mitcad#217).
    SelectionItem pressed;
    pressed.kind = m_model.point(entity) != nullptr ? SelectKind::SketchPoint : SelectKind::SketchCurve;
    pressed.owner = m_uid;
    pressed.name = entity;
    if (!entity.isEmpty() && !(e->modifiers() & (Qt::ControlModifier | Qt::ShiftModifier)) &&
        m_host.sketchClickable(pressed, false)) {
      m_pressed = true;
      m_dragEntity = entity;
      m_pressPosition = e->position();
      m_pressModifiers = e->modifiers();
      return true;
    }
    return false;
  }
  case QEvent::MouseButtonRelease: {
    auto* e = static_cast<QMouseEvent*>(event);
    if (e->button() == Qt::RightButton && !m_tool && !m_editor &&
        screenDistance(e->position(), m_rightPressPosition) <= kDragPixels) {
      // A right click on a dimension or a constraint's symbol selects it,
      // unless it is selected already, as a right click on geometry does:
      // the view's context menu, which comes after, then offers its
      // commands (Edit Dimension, Edit Equal Spacing).
      const SketchOverlay::Hit hit = clickableHit(e->position(), Qt::NoModifier);
      if (hit.valid() && !m_host.selection().contains(annotationItem(m_uid, hit))) {
        m_host.pick({annotationItem(m_uid, hit)}, Qt::NoModifier);
        m_overlay->update();
      }
      return false;
    }
    if (e->button() != Qt::LeftButton) {
      return false;
    }
    if (m_tool) {
      updateCursor(e->position());
      if (m_cursorValid) {
        m_tool->release(m_snap);
      }
      m_overlay->update();
      return true;
    }
    if (m_dragging) {
      endDrag();
      return true;
    }
    if (m_pressed) {
      m_pressed = false;
      if (!m_dragEntity.isEmpty()) {
        forwardClick(e);
      }
      m_dragEntity.clear();
      m_dragDimension.clear();
      return true;
    }
    return false;
  }
  case QEvent::MouseButtonDblClick: {
    auto* e = static_cast<QMouseEvent*>(event);
    if (e->button() != Qt::LeftButton) {
      return false;
    }
    if (m_tool) {
      updateCursor(e->position());
      m_tool->doubleClick(m_snap);
      m_overlay->update();
      return true;
    }
    const SketchOverlay::Hit hit = m_overlay->hitTest(e->position());
    if (hit.kind == SketchOverlay::Hit::Kind::Dimension) {
      m_pressed = false;
      m_dragDimension.clear();
      editDimension(hit.id);
      return true;
    }
    // An equal spacing's symbol (the first click selected it) opens its
    // edit panel (mitcad#221).
    if (const ConstraintData* k = hit.kind == SketchOverlay::Hit::Kind::Constraint ? m_model.constraint(hit.id)
                                                                                    : nullptr;
        k != nullptr && k->type == QStringLiteral("equal_spacing")) {
      qDebug().noquote() << QStringLiteral("Double-click on %1: sketch.edit_equal_spacing").arg(hit.id);
      m_host.trigger(QStringLiteral("sketch.edit_equal_spacing"));
      return true;
    }
    // A text, a pattern's curve or an offset's curve the first click picked
    // opens its edit panel (P3, P4).
    // The view picks a click with its next frame and hands the pick on by a
    // queued call, so under load a click's pick can still be on its way
    // when the double-click is here (mitcad#488). Deliver the ones queued
    // already, before the selection is read: otherwise they reach the
    // command that opens and, in its input that holds the item already,
    // take the item out again (Edit QR Code then has no code to preview).
    m_host.deliverPendingPicks();
    const Selection& selected = m_host.selection();
    if (selected.size() == 1 && selected.first().owner == m_uid &&
        (selected.first().kind == SelectKind::SketchCurve || selected.first().kind == SelectKind::SketchPoint)) {
      const QString id = selected.first().name;
      const sketch::TextData* text = m_model.text(id);
      const char* command = text != nullptr && text->code ? "sketch.edit_code"
                            : text != nullptr             ? "sketch.edit_text"
                            : m_model.offsetOf(id) != nullptr ? "sketch.edit_offset"
                            : m_model.patternOf(id) != nullptr ? "sketch.edit_pattern"
                                                               : nullptr;
      if (command != nullptr) {
        m_pressed = false;
        m_dragEntity.clear();
        // The picks of the clicks that the view has not handled yet are
        // dropped for the same reason.
        m_viewer.cancelPointerInteraction();
        qDebug().noquote() << QStringLiteral("Double-click on %1: %2").arg(id, QString::fromLatin1(command));
        m_host.trigger(QString::fromLatin1(command));
        return true;
      }
    }
    return false;
  }
  default:
    return false;
  }
}

void SketchController::forwardClick(const QMouseEvent* release) {
  // A press on geometry that did not become a drag is a click: the view
  // picks as usual.
  QMouseEvent press(QEvent::MouseButtonPress, m_pressPosition, m_viewer.mapToGlobal(m_pressPosition),
                    Qt::LeftButton, Qt::LeftButton, m_pressModifiers);
  QMouseEvent up(QEvent::MouseButtonRelease, release->position(), release->globalPosition(),
                 Qt::LeftButton, Qt::NoButton, release->modifiers());
  m_forwarding = true;
  QCoreApplication::sendEvent(&m_viewer, &press);
  QCoreApplication::sendEvent(&m_viewer, &up);
  m_forwarding = false;
}

void SketchController::selectAnnotation(const QString& kind, const QString& id,
                                        Qt::KeyboardModifiers modifiers) {
  SelectionItem item;
  item.kind = kind == QStringLiteral("dimension") ? SelectKind::SketchDimension
                                                  : SelectKind::SketchConstraint;
  item.owner = m_uid;
  item.name = id;
  m_host.pick({item}, modifiers);
  m_overlay->update();
}

bool SketchController::annotationAt(const QPointF& position) const {
  if (!isActive()) {
    return false;
  }
  const SketchOverlay::Hit hit = m_overlay->hitTest(position);
  return hit.valid() && m_host.sketchClickable(annotationItem(m_uid, hit), false);
}

Selection SketchController::annotationsIn(const QRectF& area, bool crossing) const {
  Selection items;
  if (!isActive()) {
    return items;
  }
  for (const SketchOverlay::Hit& hit : m_overlay->hitsIn(area, crossing)) {
    const SelectionItem item = annotationItem(m_uid, hit);
    if (!items.contains(item)) {
      items.append(item);
    }
  }
  return items;
}

// ---------------------------------------------------------------------------
// Drag (the solver's drag mode)

void SketchController::startDrag() {
  m_dragging = true;
  m_dragDepth = undoDepth();
  m_dragSteps = 0;
  m_dragStart = sketchAt(m_pressPosition).value_or(V2{});
  m_dragLast = m_dragStart;
  m_dragTarget = m_dragStart;
  if (!m_dragDimension.isEmpty()) {
    m_textOverride = std::make_pair(m_dragDimension, m_dragStart);
  }
  hint(tr("Drag: the geometry follows as far as its constraints let it."));
}

void SketchController::applyDrag() {
  if (!m_dragging || !m_dragPending) {
    return;
  }
  if (m_host.modelBusy()) {
    // A job computes the model (this timer ran out meanwhile): after it.
    m_dragTimer.start(10);
    return;
  }
  m_dragPending = false;
  const V2 target = m_dragTarget;
  if (!m_dragDimension.isEmpty()) {
    m_textOverride = std::make_pair(m_dragDimension, target);
    m_overlay->update();
    return;
  }
  QJsonObject cmd{{QStringLiteral("cmd"), QStringLiteral("sketch.drag")},
                  {QStringLiteral("sketch"), m_uid},
                  {QStringLiteral("entity"), m_dragEntity}};
  if (m_model.point(m_dragEntity) != nullptr) {
    cmd.insert(QStringLiteral("to"), xy(target));
  } else if (const CurveData* c = m_model.curve(m_dragEntity)) {
    if (c->isCircle()) {
      cmd.insert(QStringLiteral("radius"), std::max(dist(target, c->curve.center), 1e-3));
    } else {
      // The point of the curve that was grabbed follows the cursor.
      const V2 grabbed = c->curve.nearest(m_dragLast);
      cmd.insert(QStringLiteral("by"), xy(target - grabbed));
    }
  } else {
    return;
  }
  try {
    runModel(cmd);
    ++m_dragSteps;
    m_dragLast = target;
  } catch (const std::exception&) {
    // Fixed geometry does not move.
  }
  reload();
  m_host.refreshScene();
}

void SketchController::interruptDrag() {
  if (!m_pressed && !m_dragging) {
    return;
  }
  m_pressed = false;
  if (!m_dragging) {
    m_dragEntity.clear();
    m_dragDimension.clear();
    return;
  }
  // The steps so far stay; the one waiting is dropped. The drag's last
  // step may be the job now running: what ends it waits for that.
  m_dragging = false;
  m_dragPending = false;
  m_dragTimer.stop();
  qDebug().noquote() << QStringLiteral("Drag of %1 ended by a dialog").arg(
      m_dragEntity.isEmpty() ? m_dragDimension : m_dragEntity);
  m_host.whenIdle(this, [this] {
    if (isActive()) {
      endDrag();
    }
  });
}

void SketchController::endDrag() {
  m_dragTimer.stop();
  applyDrag();
  m_dragging = false;
  m_pressed = false;
  if (!m_dragDimension.isEmpty()) {
    const QString id = m_dragDimension;
    const V2 at = m_textOverride ? m_textOverride->second : m_dragTarget;
    m_dragDimension.clear();
    m_textOverride.reset();
    SketchOp op(*this);
    QJsonObject cmd = op.command(QStringLiteral("sketch.set_dimension_text"));
    cmd.insert(QStringLiteral("dimension"), id);
    cmd.insert(QStringLiteral("text"), xy(at));
    if (op.run(cmd)) {
      op.commit();
      qDebug().noquote() << QStringLiteral("Moved dimension %1 to (%2, %3)").arg(id).arg(at.x).arg(at.y);
    } else {
      op.rollback();
    }
    return;
  }
  const QString entity = m_dragEntity;
  m_dragEntity.clear();
  if (m_dragSteps > 1) {
    try {
      runModel({{QStringLiteral("cmd"), QStringLiteral("merge_undo")},
                {QStringLiteral("depth"), m_dragDepth},
                {QStringLiteral("label"), QStringLiteral("Drag in %1").arg(m_model.name)}});
    } catch (const std::exception&) {
      // Each step stays its own.
    }
  }
  if (m_dragSteps == 0) {
    hint(tr("Fully constrained or fixed geometry does not move."));
    qDebug().noquote() << QStringLiteral("Drag of %1 did not move it").arg(entity);
    return;
  }
  // Where the dragged point is, or the curve's point nearest the cursor.
  V2 at = m_dragLast;
  if (const auto p = m_model.pointAt(entity)) {
    at = *p;
  } else if (const CurveData* c = m_model.curve(entity)) {
    at = c->curve.nearest(m_dragLast);
  }
  qDebug().noquote() << QStringLiteral("Dragged %1 to (%2, %3)")
                            .arg(entity)
                            .arg(at.x, 0, 'f', 3)
                            .arg(at.y, 0, 'f', 3);
  m_host.refreshScene();
}

// ---------------------------------------------------------------------------
// Typed values

int SketchController::fieldIndex(const QString& id) const {
  for (int i = 0; i < m_fields.size(); ++i) {
    if (m_fields[i].spec.id == id) {
      return i;
    }
  }
  return -1;
}

void SketchController::setFields(const QVector<FieldSpec>& fields) {
  clearFields();
  for (const FieldSpec& spec : fields) {
    Field field;
    field.spec = spec;
    auto* edit = new QLineEdit(&m_viewer);
    edit->setObjectName(QStringLiteral("sketchField_") + spec.id);
    edit->setStyleSheet(kFieldStyle);
    edit->setFixedWidth(spec.kind == FieldSpec::Kind::Text ? 140 : 84);
    edit->setToolTip(tr("%1: type a value or an expression; Tab moves on, Enter accepts")
                         .arg(spec.label));
    edit->setPlaceholderText(spec.label);
    edit->installEventFilter(this);
    const int index = static_cast<int>(m_fields.size());
    connect(edit, &QLineEdit::textEdited, this, [this, index] { fieldEdited(index); });
    field.edit = edit;
    m_fields.append(field);
  }
  placeFields();
  for (const Field& field : m_fields) {
    if (!showNativeInlineEditor(field.edit)) {
      field.edit->show();
    }
  }
  if (!m_fields.isEmpty()) {
    if (!focusNativeInlineEditor(m_fields.first().edit)) {
      m_fields.first().edit->setFocus();
    }
    m_fields.first().edit->selectAll();
  }
}

void SketchController::clearFields() {
  const bool hadFocus =
      std::any_of(m_fields.begin(), m_fields.end(), [](const Field& f) {
        return f.edit && (f.edit->hasFocus() || nativeInlineEditorFocused(f.edit));
      });
  for (Field& field : m_fields) {
    if (field.edit) {
      hideNativeInlineEditor(field.edit);
      field.edit->hide();
      field.edit->deleteLater();
    }
  }
  m_fields.clear();
  if (hadFocus) {
    m_viewer.setFocus();
  }
}

void SketchController::placeFields() {
  if (m_fields.isEmpty()) {
    return;
  }
  QPointF at = m_cursor + QPointF(20.0, 16.0);
  for (Field& field : m_fields) {
    if (!field.edit) {
      continue;
    }
    const QSize size = field.edit->sizeHint();
    const int x = std::clamp(static_cast<int>(at.x()), 0, std::max(0, m_viewer.width() - field.edit->width()));
    const int y = std::clamp(static_cast<int>(at.y()), 0, std::max(0, m_viewer.height() - size.height()));
    field.edit->move(x, y);
    at.ry() += size.height() + 3.0;
  }
}

void SketchController::fieldEdited(int index) {
  if (index < 0 || index >= m_fields.size()) {
    return;
  }
  Field& field = m_fields[index];
  const QString text = field.edit ? field.edit->text().trimmed() : QString();
  field.typed = !text.isEmpty();
  field.valid = false;
  QString problem;
  if (field.typed) {
    field.valid = evaluate(text, field.spec.kind, field.expression, field.value, problem);
  }
  if (field.edit) {
    field.edit->setStyleSheet(field.typed && !field.valid ? kInvalidFieldStyle : kFieldStyle);
    field.edit->setProperty("nativeInlineInvalid", field.typed && !field.valid);
    field.edit->setToolTip(problem);
  }
  qDebug().noquote() << QStringLiteral("Sketch value %1: %2%3")
                            .arg(field.spec.id, text,
                                 field.valid ? QStringLiteral(" = %1").arg(field.expression)
                                 : field.typed ? QStringLiteral(" is invalid: %1").arg(problem)
                                               : QString());
  if (m_tool) {
    m_tool->fieldsEdited();
  }
  m_overlay->update();
}

bool SketchController::fieldEvent(QLineEdit* edit, QEvent* event) {
  if (event->type() != QEvent::KeyPress) {
    return false;
  }
  auto* key = static_cast<QKeyEvent*>(event);
  if (key->key() != Qt::Key_Tab && key->key() != Qt::Key_Backtab) {
    return false;
  }
  // Tab moves between the values.
  int index = -1;
  for (int i = 0; i < m_fields.size(); ++i) {
    if (m_fields[i].edit == edit) {
      index = i;
    }
  }
  if (index < 0 || m_fields.isEmpty()) {
    return false;
  }
  const int step = key->key() == Qt::Key_Backtab ? -1 : 1;
  const int next = (index + step + static_cast<int>(m_fields.size())) % static_cast<int>(m_fields.size());
  if (QLineEdit* target = m_fields[next].edit) {
    if (!focusNativeInlineEditor(target)) {
      target->setFocus();
    }
    target->selectAll();
  }
  return true;
}

bool SketchController::typed(const QString& id) const {
  const int i = fieldIndex(id);
  return i >= 0 && m_fields[i].typed && m_fields[i].valid;
}

std::optional<double> SketchController::fieldValue(const QString& id) const {
  const int i = fieldIndex(id);
  if (i < 0 || !m_fields[i].typed || !m_fields[i].valid) {
    return std::nullopt;
  }
  return m_fields[i].value;
}

QString SketchController::fieldExpression(const QString& id) const {
  const int i = fieldIndex(id);
  return i >= 0 && m_fields[i].valid ? m_fields[i].expression : QString();
}

QString SketchController::fieldText(const QString& id) const {
  const int i = fieldIndex(id);
  return i >= 0 && m_fields[i].edit ? m_fields[i].edit->text() : QString();
}

void SketchController::setLive(const QString& id, double value) {
  const int i = fieldIndex(id);
  if (i < 0 || m_fields[i].typed || !m_fields[i].edit) {
    return;
  }
  QString text;
  switch (m_fields[i].spec.kind) {
  case FieldSpec::Kind::Angle:
    text = QString::number(value * 180.0 / kPi, 'f', 1);
    break;
  case FieldSpec::Kind::Count:
    text = QString::number(static_cast<int>(std::lround(value)));
    break;
  case FieldSpec::Kind::Length:
    text = QString::number(value / m_unitScale, 'f', 2);
    break;
  case FieldSpec::Kind::Text:
    return;
  }
  QLineEdit* edit = m_fields[i].edit;
  if (edit->text() != text) {
    edit->setText(text);
    if (edit->hasFocus() || nativeInlineEditorFocused(edit)) {
      edit->selectAll();
    }
  }
}

void SketchController::openValueEditor(const QPointF& at, const QString& text, FieldSpec::Kind kind,
                                       std::function<void(const QString&)> accept, const QString& name) {
  closeEditor();
  auto* edit = new QLineEdit(&m_viewer);
  edit->setObjectName(QStringLiteral("sketchValueEditor"));
  edit->setStyleSheet(kFieldStyle);
  edit->setText(text);
  int width = 110;
  if (!name.isEmpty()) {
    // The parameter's name before the text, not part of it (mitcad#231).
    auto* label = new QLabel(QStringLiteral("%1 =").arg(name), edit);
    label->setObjectName(QStringLiteral("sketchValueEditorName"));
    label->setStyleSheet(QStringLiteral("color: palette(mid); background: transparent; padding-left: 4px;"));
    label->adjustSize();
    edit->setTextMargins(label->width(), 0, 0, 0);
    width += label->width();
    m_editorLabel = label;
    // The native inline editor hides this widget with the label: it shows
    // the name itself (mitcad#250).
    edit->setProperty("nativeInlineName", label->text());
  }
  edit->setFixedWidth(width);
  edit->setToolTip(tr("A value or an expression, such as 20, 2 in or d1 * 2; name = value renames"));
  const QSize size = edit->sizeHint();
  if (m_editorLabel) {
    m_editorLabel->setFixedHeight(size.height());
  }
  edit->move(static_cast<int>(at.x() - width / 2.0), static_cast<int>(at.y() - size.height() / 2.0));
  if (!showNativeInlineEditor(edit)) {
    edit->show();
    edit->raise();
    edit->setFocus();
  } else {
    focusNativeInlineEditor(edit);
  }
  edit->selectAll();
  m_editor = edit;
  m_editorKind = kind;
  m_editorText = text;
  m_editorAccept = std::move(accept);
  m_editorToolSession = m_tool ? std::optional<int>(m_toolSession) : std::nullopt;
  const QPoint corner = m_viewer.mapTo(m_viewer.window(), edit->geometry().center());
  qDebug().noquote() << QStringLiteral("Sketch value editor at %1,%2 with %3%4")
                            .arg(corner.x())
                            .arg(corner.y())
                            .arg(name.isEmpty() ? QString() : QStringLiteral("%1 = ").arg(name), text);
}

void SketchController::insertIntoEditor(const QString& name) {
  if (!m_editor || name.isEmpty()) {
    return;
  }
  // Into a value not edited yet, the name replaces it, as typing would.
  if (m_editor->text() == m_editorText) {
    m_editor->selectAll();
  }
  m_editor->insert(name);
  if (!focusNativeInlineEditor(m_editor)) {
    m_editor->setFocus();
  }
  qDebug().noquote() << QStringLiteral("Sketch value editor: inserted %1, now %2").arg(name, m_editor->text());
}

void SketchController::acceptEditor() {
  if (!m_editor) {
    return;
  }
  // A tool is retired before its deferred deletion, so checking only its
  // lifetime would still allow a callback into a tool that has ended.
  if (m_editorToolSession && (!m_tool || *m_editorToolSession != m_toolSession)) {
    closeEditor();
    return;
  }
  QString text = m_editor->text().trimmed();
  // `name = value` renames the parameter as it sets it (mitcad#231): the
  // value is checked here, the name by the model.
  QString rename;
  if (m_editorKind != FieldSpec::Kind::Text) {
    static const QRegularExpression named(QStringLiteral(R"(^(\w+)\s*=(?!=)\s*(.*)$)"),
                                          QRegularExpression::UseUnicodePropertiesOption);
    const QRegularExpressionMatch match = named.match(text);
    if (match.hasMatch()) {
      rename = match.captured(1);
      text = match.captured(2).trimmed();
    }
  }
  QString expression;
  double value = 0.0;
  QString problem;
  if (!evaluate(text, m_editorKind, expression, value, problem)) {
    m_editor->setStyleSheet(kInvalidFieldStyle);
    m_editor->setProperty("nativeInlineInvalid", true);
    error(tr("%1: %2").arg(text, problem));
    return;
  }
  if (!rename.isEmpty()) {
    expression = QStringLiteral("%1 = %2").arg(rename, expression);
  }
  auto accept = std::move(m_editorAccept);
  // A refused name keeps the editor open with what was typed.
  const QPointer<QLineEdit> editor = m_editor;
  const QString typed = editor->text();
  const QString dimension = m_editorDimension;
  hideNativeInlineEditor(editor);
  editor->hide();
  if (accept) {
    accept(expression);
  }
  // Only an existing dimension's editor knows its dimension: the Dimension
  // tool adds one and leaves no id here (mitcad#352).
  if (!rename.isEmpty() && !dimension.isEmpty() && editor && editor == m_editor) {
    const DimensionData* d = m_model.dimension(dimension);
    if (d == nullptr || d->parameter != rename) {
      m_editorAccept = std::move(accept);
      editor->setText(typed);
      editor->setStyleSheet(kInvalidFieldStyle);
      editor->setProperty("nativeInlineInvalid", true);
      if (!showNativeInlineEditor(editor)) {
        editor->show();
        editor->setFocus();
      } else {
        focusNativeInlineEditor(editor);
      }
      qDebug().noquote() << QStringLiteral("Sketch value editor kept open with %1").arg(typed);
      return;
    }
  }
  if (editor && editor == m_editor) {
    closeEditor();
  }
}

void SketchController::closeEditor() {
  if (m_editor) {
    hideNativeInlineEditor(m_editor);
    m_editor->hide();
    m_editor->deleteLater();
    m_editor = nullptr;
    m_viewer.setFocus();
    qDebug().noquote() << QStringLiteral("Sketch value editor closed");
  }
  m_editorAccept = nullptr;
  m_editorToolSession.reset();
  m_editorDimension.clear();
  m_editorLabel = nullptr;
}

bool SketchController::evaluate(const QString& text, FieldSpec::Kind kind, QString& expression,
                                double& value, QString& problem) const {
  if (kind == FieldSpec::Kind::Text) {
    expression = text;
    return !text.isEmpty();
  }
  if (text.trimmed().isEmpty()) {
    problem = tr("Enter a value.");
    return false;
  }
  try {
    const QJsonObject answer = m_host.queryObject({{QStringLiteral("query"), QStringLiteral("evaluate")},
                                                   {QStringLiteral("expression"), text},
                                                   {QStringLiteral("kind"), kindKey(kind)}});
    value = answer.value(QStringLiteral("value")).toDouble();
    const QString shown = answer.value(QStringLiteral("text")).toString();
    // With decimal points where commas were typed (mitcad#2).
    const QString typed = answer.value(QStringLiteral("expression")).toString(text);
    const bool isNumber = parsePlainNumber(text).has_value();
    const int space = static_cast<int>(shown.lastIndexOf(QLatin1Char(' ')));
    expression = isNumber && space > 0 && kind != FieldSpec::Kind::Count ? typed + shown.mid(space) : typed;
    return true;
  } catch (const std::exception& e) {
    problem = errorText(e);
    return false;
  }
}

QString SketchController::formatLength(double mm) const {
  return QString::number(mm / m_unitScale, 'f', 2);
}

QString SketchController::formatAngle(double radians) const {
  return QString::number(radians * 180.0 / kPi, 'f', 1) + QStringLiteral("°");
}

// ---------------------------------------------------------------------------
// Model

QJsonObject SketchController::runModel(const QJsonObject& command) { return m_host.modelCommand(command); }

int SketchController::undoDepth() const {
  return m_host.queryObject({{QStringLiteral("query"), QStringLiteral("document")}})
      .value(QStringLiteral("undo_depth"))
      .toInt();
}

QString SketchController::undoLabel() const {
  return m_host.queryObject({{QStringLiteral("query"), QStringLiteral("document")}})
      .value(QStringLiteral("undo"))
      .toString();
}

} // namespace mitcad::sketch
