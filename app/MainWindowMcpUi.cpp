// SPDX-License-Identifier: MIT
// Live MCP's view of the application and its actions in it (mitcad#265):
// ui_state, ui_log, ui_screenshot, ui_select and ui_command. What the user
// can see and do in the window, an agent can read and do, with the same
// refusals and undo.
#include "MainWindow.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <functional>
#include <limits>
#include <memory>
#include <optional>
#include <stdexcept>
#include <utility>
#include <vector>

#include <QApplication>
#include <QBuffer>
#include <QCoreApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QLabel>
#include <QPainter>
#include <QPixmap>
#include <QPoint>
#include <QPointer>
#include <QRect>
#include <QRegion>
#include <QScopedValueRollback>
#include <QStackedWidget>
#include <QSysInfo>
#include <QTemporaryDir>
#include <QTimer>
#include <QWidget>
#include <QDebug>
#include <QtLogging>

#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <Precision.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>
#include <gp_XYZ.hxx>

#include "framework/ChromeStyle.hpp"
#include "framework/CommandRegistry.hpp"
#include "framework/ResultCache.hpp"
#include "mcp/LiveErrors.hpp"
#include "mcp/LogRing.hpp"
#include "mcp/ModalGuard.hpp"
#include "platform/MacChrome.hpp"
#include "sketch/SketchPalette.hpp"
#include "sketch/SketchTool.hpp"
#include "view/ViewReveal.hpp"
#ifdef MITCAD_MACOS_NATIVE_UI
#include "platform/NativeForms.hpp"
#endif

namespace mitcad {
namespace {

constexpr double kPi = 3.14159265358979323846;

// The selection's kinds by the names MCP clients use.
const std::pair<SelectKind, const char*> kKinds[] = {
    {SelectKind::Face, "face"},
    {SelectKind::Edge, "edge"},
    {SelectKind::Vertex, "vertex"},
    {SelectKind::Body, "body"},
    {SelectKind::Profile, "profile"},
    {SelectKind::SketchCurve, "sketch_curve"},
    {SelectKind::SketchPoint, "sketch_point"},
    {SelectKind::SketchConstraint, "sketch_constraint"},
    {SelectKind::SketchDimension, "sketch_dimension"},
    {SelectKind::Plane, "plane"},
    {SelectKind::Axis, "axis"},
    {SelectKind::Point, "point"},
    {SelectKind::Component, "component"},
    {SelectKind::Sketch, "sketch"},
    {SelectKind::Feature, "feature"},
};

QString kindKey(SelectKind kind) {
  for (const auto& [value, key] : kKinds) {
    if (value == kind) {
      return QString::fromLatin1(key);
    }
  }
  return QStringLiteral("none");
}

QJsonObject itemJson(const SelectionItem& item) {
  return {{QStringLiteral("kind"), kindKey(item.kind)},
          {QStringLiteral("owner"), item.owner},
          {QStringLiteral("name"), item.name},
          {QStringLiteral("occurrence"), item.occurrence},
          {QStringLiteral("geometry"), item.geometry},
          {QStringLiteral("reference"), item.reference()},
          {QStringLiteral("text"), item.describe()}};
}

QJsonArray selectionJson(const Selection& items) {
  QJsonArray list;
  for (const SelectionItem& item : items) {
    list.append(itemJson(item));
  }
  return list;
}

// An entity as ui_select and ui_command's selection inputs take it, with
// the surface or curve type the view gives faces and edges (the inputs'
// filters ask), or none with the reason.
std::optional<SelectionItem> readItem(const QJsonValue& value, const CommandContext& model, QString& problem) {
  const QJsonObject entry = value.toObject();
  SelectionItem item;
  const QString kind = entry.value(QStringLiteral("kind")).toString();
  for (const auto& [known, key] : kKinds) {
    if (kind == QLatin1String(key)) {
      item.kind = known;
    }
  }
  item.owner = entry.value(QStringLiteral("owner")).toString();
  item.name = entry.value(QStringLiteral("name")).toString();
  item.occurrence = entry.value(QStringLiteral("occurrence")).toString();
  item.geometry = entry.value(QStringLiteral("geometry")).toString();
  if (!item.isValid() || item.owner.isEmpty()) {
    problem = QObject::tr("not an entity: %1").arg(QString::fromUtf8(QJsonDocument(entry).toJson(QJsonDocument::Compact)));
    return std::nullopt;
  }
  if (item.geometry.isEmpty() && (item.kind == SelectKind::Face || item.kind == SelectKind::Edge)) {
    try {
      const bool face = item.kind == SelectKind::Face;
      const QJsonArray entries =
          model
              .query({{QStringLiteral("query"), face ? QStringLiteral("faces") : QStringLiteral("edges")},
                      {QStringLiteral("body"), item.owner}})
              .toArray();
      for (const QJsonValue& known : entries) {
        const QJsonObject e = known.toObject();
        const bool same = face ? e.value(QStringLiteral("names")).toArray().contains(QJsonValue(item.name))
                               : e.value(QStringLiteral("name")).toString() == item.name;
        if (same) {
          item.geometry = e.value(face ? QStringLiteral("surface") : QStringLiteral("curve")).toString();
        }
      }
    } catch (const std::exception&) {
      // The input's own test decides without the type.
    }
  }
  return item;
}

QString modeName(DocumentSession::Mode mode) {
  switch (mode) {
  case DocumentSession::Mode::Idle:
    return QStringLiteral("idle");
  case DocumentSession::Mode::PickPlane:
    return QStringLiteral("pick_plane");
  case DocumentSession::Mode::Sketch:
    return QStringLiteral("sketch");
  case DocumentSession::Mode::Command:
    return QStringLiteral("command");
  }
  return QStringLiteral("idle");
}

[[noreturn]] void refuse(const QString& reason) {
  qInfo().noquote() << QStringLiteral("MCP refused: %1").arg(reason);
  throw std::runtime_error(reason.toStdString());
}

[[noreturn]] void refuseWith(const QString& reason, const QJsonObject& fields) {
  qInfo().noquote() << QStringLiteral("MCP refused: %1").arg(reason);
  throw mcp::Refusal(reason, fields);
}

// Runs the event loop until `done` holds (true) or the time is up (false):
// a native command panel starts and finishes on the model's worker, a
// preview after its delay.
bool waitUntil(const std::function<bool()>& done, int milliseconds) {
  QElapsedTimer clock;
  clock.start();
  while (!done()) {
    if (clock.elapsed() > milliseconds) {
      return false;
    }
    QEventLoop loop;
    QTimer::singleShot(10, &loop, &QEventLoop::quit);
    loop.exec();
  }
  return true;
}

// Commands an agent cannot use: they open file dialogs, other windows or
// the system's applications (mitcad#265). Any other modal dialog a command
// opens is closed at once (mcp::ModalGuard) and the call refused.
bool opensDialog(const QString& id) {
  for (const char* prefix : {"file.", "help.", "tools.", "insert."}) {
    if (id.startsWith(QLatin1String(prefix))) {
      return true;
    }
  }
  for (const char* known : {"sketch.insert_dxf", "sketch.import_dxf", "sheet.export_dxf", "make.print3d",
                            "view.render_environment", "view.grid_settings", "solid.parameters"}) {
    if (id == QLatin1String(known)) {
      return true;
    }
  }
  return false;
}

// The log lines since a sequence number, as text.
QJsonArray logTexts(quint64 since, int limit) {
  QJsonArray lines;
  for (const mcp::LogLine& line : mcp::logLines(since, limit)) {
    lines.append(line.text);
  }
  return lines;
}

quint64 logNext() {
  quint64 next = 0;
  mcp::logLines(std::numeric_limits<quint64>::max(), 0, &next);
  return next;
}

} // namespace

QJsonObject MainWindow::mcpUiState() {
  const DocumentSession::Snapshot snapshot = m_documentSession->snapshot();
  const QJsonObject document = snapshot.document;
  QJsonObject state{
      {QStringLiteral("document"), m_mcpDocumentId},
      {QStringLiteral("name"), snapshot.name},
      {QStringLiteral("path"), snapshot.filePath.isEmpty() ? QJsonValue() : QJsonValue(snapshot.filePath)},
      {QStringLiteral("modified"), snapshot.modified},
      {QStringLiteral("read_only"), windowReadOnly()},
      {QStringLiteral("mode"), modeName(m_mode)},
      {QStringLiteral("busy"), snapshot.busy},
      {QStringLiteral("undo"), document.value(QStringLiteral("undo"))},
      {QStringLiteral("redo"), document.value(QStringLiteral("redo"))},
      {QStringLiteral("undo_depth"), document.value(QStringLiteral("undo_depth"))},
      {QStringLiteral("selection"), selectionJson(m_selection)},
      {QStringLiteral("hint"), m_statusLabel->text()},
      // An error is shown red (setErrorStyleSheet), a hint plain.
      {QStringLiteral("hint_is_error"), !m_statusLabel->styleSheet().isEmpty()},
      {QStringLiteral("agents"), m_mcpAgents},
      {QStringLiteral("editable"), mcpBlockedReason(true).isEmpty()}};
  if (windowReadOnly()) {
    state.insert(QStringLiteral("read_only_reason"), readOnlyReason());
  }
  if (document.contains(QStringLiteral("active_component"))) {
    state.insert(QStringLiteral("active_component"), document.value(QStringLiteral("active_component")));
  }
  // The sketch being edited, and its Sketch Palette.
  if (m_sketch->isActive()) {
    const sketch::SketchModel& sketchModel = m_sketch->model();
    QJsonArray warnings;
    for (int i = 0; i < sketchModel.warnings.size(); ++i) {
      const QStringList entities = static_cast<std::size_t>(i) < sketchModel.warningEntities.size()
                                       ? sketchModel.warningEntities[static_cast<std::size_t>(i)]
                                       : QStringList();
      warnings.append(QJsonObject{{QStringLiteral("text"), sketchModel.warnings[i]},
                                  {QStringLiteral("entities"), QJsonArray::fromStringList(entities)}});
    }
    QJsonObject paletteState{{QStringLiteral("status"), m_palette != nullptr ? m_palette->statusText() : QString()},
                        {QStringLiteral("warnings"), warnings},
                        {QStringLiteral("clean_up_offered"), m_palette != nullptr && m_palette->cleanUpOffered()}};
    state.insert(QStringLiteral("sketch"),
                 QJsonObject{{QStringLiteral("uid"), m_sketch->uid()},
                             {QStringLiteral("name"), sketchModel.name},
                             {QStringLiteral("dof"), sketchModel.dof},
                             {QStringLiteral("fully_constrained"), sketchModel.fullyConstrained()},
                             {QStringLiteral("existing"), m_sketchExisting},
                             {QStringLiteral("tool"), m_sketch->tool() != nullptr ? QJsonValue(m_sketch->tool()->name())
                                                                                  : QJsonValue()},
                             {QStringLiteral("palette"), paletteState}});
  } else {
    state.insert(QStringLiteral("sketch"), QJsonValue());
  }
  // The running command with its inputs and panel.
  if (m_session) {
    QJsonObject command = m_session->scriptState();
    command.insert(QStringLiteral("in_sketch"), m_commandInSketch);
    state.insert(QStringLiteral("command"), command);
  } else {
    state.insert(QStringLiteral("command"), QJsonValue());
  }
  state.insert(QStringLiteral("app"),
               QJsonObject{{QStringLiteral("version"), QCoreApplication::applicationVersion()},
                           {QStringLiteral("build"), resultStoreBuildId()},
                           {QStringLiteral("qt"), QString::fromLatin1(qVersion())},
                           {QStringLiteral("platform"), QSysInfo::prettyProductName()},
                           {QStringLiteral("ui"), m_nativeUi ? QStringLiteral("native") : QStringLiteral("qt")},
                           {QStringLiteral("chrome"), chromeStyle() == ChromeStyle::Floating ? QStringLiteral("floating")
                                                                                             : QStringLiteral("docked")}});
  return state;
}

QJsonObject MainWindow::mcpLog(const QJsonObject& arguments) const {
  const qint64 since = std::max<qint64>(0, arguments.value(QStringLiteral("since")).toInteger(0));
  const int limit = std::clamp(arguments.value(QStringLiteral("limit")).toInt(200), 1, 2000);
  quint64 next = 0;
  bool dropped = false;
  QJsonArray lines;
  for (const mcp::LogLine& line : mcp::logLines(static_cast<quint64>(since), limit, &next, &dropped)) {
    lines.append(QJsonObject{{QStringLiteral("seq"), static_cast<qint64>(line.sequence)},
                             {QStringLiteral("level"), line.level},
                             {QStringLiteral("text"), line.text},
                             {QStringLiteral("time"), line.time}});
  }
  return {{QStringLiteral("lines"), lines},
          {QStringLiteral("next"), static_cast<qint64>(next)},
          {QStringLiteral("dropped"), dropped}};
}

QJsonObject MainWindow::mcpScreenshot(const QJsonObject& arguments) {
  const bool whole = arguments.value(QStringLiteral("window")).toBool(false);
  const int maxSize = std::clamp(arguments.value(QStringLiteral("max_size")).toInt(1280), 64, 4096);
  if (m_viewer->isPainting()) {
    refuseWith(tr("busy: the view is drawing; retry in 100 ms"),
               {{QStringLiteral("busy"), true}, {QStringLiteral("retry_after_ms"), 100}});
  }
  // The view's frame with the widgets over it (the sketch overlay, the
  // orientation cube's buttons), as the user sees it.
  const auto viewImage = [this] {
    QImage frame = m_viewer->grabFramebuffer();
    if (frame.isNull()) {
      return frame;
    }
    QPainter painter(&frame);
    for (QWidget* overlay : m_viewer->findChildren<QWidget*>(Qt::FindDirectChildrenOnly)) {
      if (overlay->isVisibleTo(m_viewer)) {
        overlay->render(&painter, overlay->pos(), QRegion(), QWidget::DrawChildren);
      }
    }
    return frame;
  };
  QImage image;
  QString what = QStringLiteral("view");
  if (!whole) {
    image = viewImage();
  } else if (m_nativeUi) {
    // AppKit draws the window: as the window server composes it.
    const QTemporaryDir directory;
    const QString path = QDir(directory.path()).filePath(QStringLiteral("window.png"));
    if (directory.isValid() && mac::captureOwnWindows(path)) {
      image.load(path);
    }
    what = QStringLiteral("window");
  } else {
    // The window's grab lacks what the view draws with OpenGL: the view's
    // frame goes in, and the widgets over it again.
    QPixmap shot = grab();
    const QImage frame = viewImage();
    if (!frame.isNull()) {
      QPainter painter(&shot);
      painter.drawImage(QRect(m_viewer->mapTo(this, QPoint(0, 0)), m_viewer->size()), frame);
      if (QWidget* parent = m_viewer->parentWidget()) {
        // Floating chrome: the cards over the view are its siblings.
        bool above = false;
        for (QObject* child : parent->children()) {
          auto* widget = qobject_cast<QWidget*>(child);
          if (widget == m_viewer) {
            above = true;
          } else if (above && widget != nullptr && widget->isVisibleTo(parent) && !widget->isWindow()) {
            widget->render(&painter, widget->mapTo(this, QPoint(0, 0)), QRegion(), QWidget::DrawChildren);
          }
        }
      }
    }
    image = shot.toImage();
    what = QStringLiteral("window");
  }
  if (image.isNull()) {
    refuse(tr("No image of the %1 could be made.").arg(what));
  }
  const int fullWidth = image.width();
  const int fullHeight = image.height();
  // Within max_size and well within the 8 MiB frame once in base64.
  constexpr qsizetype kBudget = 5 * 1024 * 1024;
  int limit = maxSize;
  QByteArray png;
  for (int attempt = 0; attempt < 8; ++attempt) {
    QImage scaled = image;
    if (scaled.width() > limit || scaled.height() > limit) {
      scaled = image.scaled(limit, limit, Qt::KeepAspectRatio, Qt::SmoothTransformation);
    }
    png.clear();
    QBuffer buffer(&png);
    buffer.open(QIODevice::WriteOnly);
    scaled.save(&buffer, "PNG");
    if (png.size() <= kBudget) {
      image = scaled;
      break;
    }
    limit = std::max(64, std::max(scaled.width(), scaled.height()) * 7 / 10);
    image = scaled;
  }
  if (png.size() > kBudget) {
    refuse(tr("The image of the %1 is too large even when scaled down.").arg(what));
  }
  qInfo().noquote() << QStringLiteral("MCP: screenshot of the %1, %2 x %3, %4 bytes")
                           .arg(what)
                           .arg(image.width())
                           .arg(image.height())
                           .arg(png.size());
  return {{QStringLiteral("format"), QStringLiteral("png")},
          {QStringLiteral("of"), what},
          {QStringLiteral("width"), image.width()},
          {QStringLiteral("height"), image.height()},
          {QStringLiteral("full_width"), fullWidth},
          {QStringLiteral("full_height"), fullHeight},
          {QStringLiteral("bytes"), static_cast<qint64>(png.size())},
          {QStringLiteral("png_base64"), QString::fromLatin1(png.toBase64())}};
}

QJsonObject MainWindow::mcpSelect(const QJsonObject& arguments) {
  mcpRefuseBusy();
  if (m_mode == Mode::PickPlane) {
    refuse(tr("Create Sketch waits for a plane to be picked: pick one or cancel it first."));
  }
  const QString reveal = arguments.value(QStringLiteral("reveal")).toString(QStringLiteral("pan"));
  Selection items;
  for (const QJsonValue& value : arguments.value(QStringLiteral("items")).toArray()) {
    QString problem;
    const std::optional<SelectionItem> item = readItem(value, *this, problem);
    if (!item) {
      refuse(problem);
    }
    if (!items.contains(*item)) {
      items.append(*item);
    }
  }
  if (m_session) {
    // A command panel takes the picks in its active input, as clicks in the
    // view and rows picked in the browser do (each a click: one already
    // there goes out again).
    const QPointer<CommandSession> session = m_session;
    for (const SelectionItem& item : items) {
      if (session) {
        session->picked({item}, Qt::NoModifier, false);
      }
    }
    waitUntil([&session] { return !session || !session->scriptBusy(); }, 120000);
    qInfo().noquote() << QStringLiteral("MCP: an agent picked %1 into the command panel").arg(summarize(items));
    QJsonObject picked{{QStringLiteral("document"), m_mcpDocumentId},
                       {QStringLiteral("picked"), static_cast<int>(items.size())},
                       {QStringLiteral("into_command"), true}};
    if (session) {
      picked.insert(QStringLiteral("command"), session->scriptState());
    }
    return picked;
  }
  const bool sketchItems =
      m_mode == Mode::Sketch && !items.isEmpty() && std::all_of(items.begin(), items.end(), [this](const SelectionItem& item) {
        return (item.kind == SelectKind::SketchCurve || item.kind == SelectKind::SketchPoint) &&
               item.owner == m_sketch->uid();
      });
  QString moved = QStringLiteral("none");
  if (sketchItems) {
    // The edited sketch's curves and points, as the palette's warnings
    // select them (mitcad#264).
    QStringList ids;
    QStringList missing;
    for (const SelectionItem& item : items) {
      const bool there = item.kind == SelectKind::SketchCurve ? m_sketch->model().curve(item.name) != nullptr
                                                              : m_sketch->model().point(item.name) != nullptr;
      (there ? ids : missing) << item.name;
    }
    if (!missing.isEmpty()) {
      refuse(tr("%1 has no %2").arg(m_sketch->model().name, missing.join(QStringLiteral(", "))));
    }
    m_sketch->selectEntities(ids, reveal != QStringLiteral("none"), reveal == QStringLiteral("fit"));
  } else {
    // Faces, edges, bodies, profiles, ...: each must be in the model, as
    // the view shows it.
    std::vector<gp_Pnt> places;
    QString missing;
    if (!mcpItemCorners(items, places, &missing)) {
      refuse(tr("Not in the model: %1").arg(missing));
    }
    setSelection(items);
    if (reveal != QStringLiteral("none")) {
      moved = revealInView(*m_viewer, places, reveal == QStringLiteral("fit"));
    }
  }
  qInfo().noquote() << QStringLiteral("MCP: an agent selected %1").arg(summarize(m_selection));
  QJsonObject result{{QStringLiteral("document"), m_mcpDocumentId},
                     {QStringLiteral("selected"), static_cast<int>(m_selection.size())},
                     {QStringLiteral("selection"), selectionJson(m_selection)}};
  if (!sketchItems) {
    result.insert(QStringLiteral("view"), moved);
  }
  if (reveal != QStringLiteral("none")) {
    // Where the view goes, not where its animation is now: the reply's
    // camera and the next screenshot show it (mitcad#405).
    m_viewer->settleCamera();
    result.insert(QStringLiteral("camera"), mcpCameraJson());
  }
  return result;
}

bool MainWindow::mcpItemCorners(const Selection& items, std::vector<gp_Pnt>& corners, QString* missing) const {
  for (const SelectionItem& item : items) {
    // Geometry must be there; datums, components, features and sketches
    // are shown when they have a shape.
    const bool required = item.kind == SelectKind::Face || item.kind == SelectKind::Edge ||
                          item.kind == SelectKind::Vertex || item.kind == SelectKind::Body ||
                          item.kind == SelectKind::Profile || item.kind == SelectKind::SketchCurve ||
                          item.kind == SelectKind::SketchPoint;
    TopoDS_Shape shape;
    try {
      shape = itemShape(item);
    } catch (const std::exception&) {
      shape.Nullify();
    }
    if (shape.IsNull()) {
      if (required && missing != nullptr) {
        *missing = item.describe();
        return false;
      }
      continue;
    }
    Bnd_Box box;
    BRepBndLib::Add(shape, box);
    if (box.IsVoid()) {
      continue;
    }
    double x0 = 0.0;
    double y0 = 0.0;
    double z0 = 0.0;
    double x1 = 0.0;
    double y1 = 0.0;
    double z1 = 0.0;
    box.Get(x0, y0, z0, x1, y1, z1);
    for (const double x : {x0, x1}) {
      for (const double y : {y0, y1}) {
        for (const double z : {z0, z1}) {
          corners.emplace_back(x, y, z);
        }
      }
    }
  }
  return true;
}

QJsonObject MainWindow::mcpCameraJson() const {
  const CameraState camera = m_viewer->camera();
  const auto xyz = [](const gp_XYZ& v) { return QJsonArray{v.X(), v.Y(), v.Z()}; };
  const double distance = camera.eye.Distance(camera.target);
  const gp_XYZ direction = distance > 0.0 ? (camera.target.XYZ() - camera.eye.XYZ()) / distance : gp_XYZ(0, 0, -1);
  const double aspect = m_viewer->height() > 0 ? double(m_viewer->width()) / double(m_viewer->height()) : 1.0;
  return {{QStringLiteral("eye"), xyz(camera.eye.XYZ())},
          {QStringLiteral("target"), xyz(camera.target.XYZ())},
          {QStringLiteral("up"), xyz(camera.up.XYZ())},
          {QStringLiteral("direction"), xyz(direction)},
          {QStringLiteral("distance"), distance},
          {QStringLiteral("height"), camera.height},
          {QStringLiteral("width"), camera.height * aspect},
          {QStringLiteral("projection"), camera.perspective ? QStringLiteral("perspective") : QStringLiteral("orthographic")},
          {QStringLiteral("fov"), m_viewer->fieldOfView()},
          {QStringLiteral("view_size"), QJsonArray{m_viewer->width(), m_viewer->height()}}};
}

QJsonObject MainWindow::mcpCamera(const QJsonObject& arguments) {
  if (m_viewer->isPainting()) {
    refuseWith(tr("busy: the view is drawing; retry in 100 ms"),
               {{QStringLiteral("busy"), true}, {QStringLiteral("retry_after_ms"), 100}});
  }
  const auto pointOf = [](const QString& key, const QJsonValue& given) {
    const QJsonArray array = given.toArray();
    if (array.size() != 3 || !array[0].isDouble() || !array[1].isDouble() || !array[2].isDouble()) {
      refuse(tr("%1 must be three numbers (x, y, z in mm).").arg(key));
    }
    return gp_XYZ(array[0].toDouble(), array[1].toDouble(), array[2].toDouble());
  };
  // Where the view is going, as the starting point.
  m_viewer->settleCamera();
  CameraState camera = m_viewer->camera();
  const bool hasEye = arguments.contains(QStringLiteral("eye"));
  const bool hasTarget = arguments.contains(QStringLiteral("target"));
  const bool hasHeight = arguments.contains(QStringLiteral("height"));
  bool set = false;
  if (arguments.contains(QStringLiteral("projection"))) {
    camera.perspective = arguments.value(QStringLiteral("projection")).toString() == QStringLiteral("perspective");
    set = true;
  }
  if (hasEye) {
    camera.eye = gp_Pnt(pointOf(QStringLiteral("eye"), arguments.value(QStringLiteral("eye"))));
  }
  if (hasTarget) {
    camera.target = gp_Pnt(pointOf(QStringLiteral("target"), arguments.value(QStringLiteral("target"))));
  }
  if (camera.eye.Distance(camera.target) <= Precision::Confusion()) {
    refuse(tr("The eye and the target must be apart."));
  }
  if (arguments.contains(QStringLiteral("up"))) {
    const gp_XYZ up = pointOf(QStringLiteral("up"), arguments.value(QStringLiteral("up")));
    const gp_XYZ direction = camera.target.XYZ() - camera.eye.XYZ();
    if (up.Modulus() <= Precision::Confusion() || up.Crossed(direction).Modulus() <= 1e-9 * up.Modulus() * direction.Modulus()) {
      refuse(tr("up must be a vector across the view direction."));
    }
    camera.up = gp_Dir(up);
    set = true;
  } else if (gp_Vec(camera.up).Crossed(gp_Vec(camera.eye, camera.target)).Magnitude() <=
             1e-9 * camera.eye.Distance(camera.target)) {
    // Looking along the current up: another up across the new direction.
    const gp_Dir direction(gp_Vec(camera.eye, camera.target));
    camera.up = std::abs(direction.Z()) < 0.9 ? gp_Dir(0, 0, 1) : gp_Dir(0, 1, 0);
  }
  if (arguments.contains(QStringLiteral("fov"))) {
    // Before the height: a perspective camera's height sets its distance
    // through the field of view.
    m_viewer->setFieldOfView(arguments.value(QStringLiteral("fov")).toDouble());
    set = true;
  }
  if (hasHeight) {
    const double height = arguments.value(QStringLiteral("height")).toDouble();
    if (!(height > 0.0)) {
      refuse(tr("height must be more than 0 mm."));
    }
    camera.height = height;
  } else if (camera.perspective && (hasEye || hasTarget)) {
    // The eye given stays where it is: the height follows from its distance.
    camera.height = 2.0 * camera.eye.Distance(camera.target) * std::tan(m_viewer->fieldOfView() * kPi / 360.0);
  }
  set = set || hasEye || hasTarget || hasHeight;
  if (set) {
    m_viewer->setCamera(camera);
    m_viewer->settleCamera();
  }
  // Then what to frame, along the view direction now.
  QString framed = QStringLiteral("none");
  Bnd_Box box;
  const QString frame = arguments.value(QStringLiteral("frame")).toString(QStringLiteral("none"));
  if (arguments.contains(QStringLiteral("box"))) {
    if (frame != QStringLiteral("none")) {
      refuse(tr("Give either box or frame, not both."));
    }
    const QJsonObject given = arguments.value(QStringLiteral("box")).toObject();
    box.Add(gp_Pnt(pointOf(QStringLiteral("box.min"), given.value(QStringLiteral("min")))));
    box.Add(gp_Pnt(pointOf(QStringLiteral("box.max"), given.value(QStringLiteral("max")))));
    framed = QStringLiteral("box");
  } else if (frame == QStringLiteral("shown")) {
    box = m_viewer->bodiesBox();
    if (box.IsVoid()) {
      refuse(tr("No body is shown to frame."));
    }
    framed = frame;
  } else if (frame == QStringLiteral("selection")) {
    std::vector<gp_Pnt> corners;
    mcpItemCorners(m_selection, corners, nullptr);
    if (corners.empty()) {
      refuse(tr("Nothing with a shape is selected to frame."));
    }
    for (const gp_Pnt& corner : corners) {
      box.Add(corner);
    }
    framed = frame;
  }
  if (!box.IsVoid()) {
    m_viewer->frameBox(box);
    m_viewer->settleCamera();
  }
  const QJsonObject result{{QStringLiteral("camera"), mcpCameraJson()}, {QStringLiteral("framed"), framed},
                           {QStringLiteral("changed"), set || framed != QStringLiteral("none")}};
  if (set || framed != QStringLiteral("none")) {
    const QJsonObject now = result.value(QStringLiteral("camera")).toObject();
    qInfo().noquote() << QStringLiteral("MCP: an agent set the camera: target %1, height %2 mm, %3%4")
                             .arg(QString::fromUtf8(QJsonDocument(now.value(QStringLiteral("target")).toArray())
                                                        .toJson(QJsonDocument::Compact)))
                             .arg(now.value(QStringLiteral("height")).toDouble(), 0, 'f', 2)
                             .arg(now.value(QStringLiteral("projection")).toString())
                             .arg(framed == QStringLiteral("none") ? QString() : QStringLiteral(", framed ") + framed);
  }
  return result;
}

QJsonObject MainWindow::mcpUiCommand(const QJsonObject& arguments) {
  mcpRefuseBusy();
  if (m_closePending || QApplication::activeModalWidget() != nullptr) {
    refuse(tr("The document is closing or waiting for a dialog."));
  }
  const QString id = arguments.value(QStringLiteral("id")).toString();
  const QString edit = arguments.value(QStringLiteral("edit")).toString();
  const QString finish = arguments.value(QStringLiteral("finish")).toString(QStringLiteral("ok"));
  const QJsonObject inputs = arguments.value(QStringLiteral("inputs")).toObject();
  if (!id.isEmpty() && !edit.isEmpty()) {
    refuse(tr("Give either id or edit, not both."));
  }
  const quint64 logStart = logNext() - 1;
  const int depthBefore = undoDepth();
  const bool inSketchBefore = m_mode == Mode::Sketch;
  const QScopedValueRollback<bool> active(m_mcpRequestActive, true);
  mcp::ModalGuard guard;
  QJsonObject result{{QStringLiteral("document"), m_mcpDocumentId}};
  bool changes = false; // the command's steps are the agent's (labelled)

  // A selection input's entries: as ui_select's items.
  const CommandSession::ItemReader reader = [this](const QJsonValue& value, QString& problem) {
    return readItem(value, *this, problem);
  };

  // The open panel: inputs, its preview, then OK, Cancel or left open.
  const auto runPanel = [&] {
    const QPointer<CommandSession> session = m_session;
    // Shared: the panel may end after this call, while the guard lives.
    const auto outcome = std::make_shared<std::pair<bool, bool>>(false, false); // ended, committed
    connect(session.data(), &CommandSession::finished, &guard, [outcome](bool ok) {
      outcome->first = true;
      outcome->second = ok;
    });
    const auto ended = [outcome] { return outcome->first; };
    result.insert(QStringLiteral("command"), session->def().id);
    if (!inputs.isEmpty()) {
      QString problem;
      if (!session->scriptInputs(inputs, reader, problem)) {
        QJsonObject fields{{QStringLiteral("panel"), session->scriptState()}};
        if (finish != QStringLiteral("keep_open")) {
          session->cancel();
          waitUntil(ended, 60000);
          fields.insert(QStringLiteral("cancelled"), true);
        }
        refuseWith(problem, fields);
      }
    }
    // The preview runs a moment after the last change.
    waitUntil([&session] { return !session || !session->scriptBusy(); }, 120000);
    if (!session) {
      result.insert(QStringLiteral("finished"), true);
      return;
    }
    const QJsonObject panel = session->scriptState();
    result.insert(QStringLiteral("panel"), panel);
    if (finish == QStringLiteral("keep_open")) {
      result.insert(QStringLiteral("open"), true);
      return;
    }
    const int appliedBefore = session->appliedCount();
    if (finish == QStringLiteral("cancel")) {
      session->cancel();
    } else {
      session->commit();
    }
    waitUntil([&ended, &session] { return ended() || !session || !session->scriptBusy(); }, 120000);
    if (!ended() && session && session->appliedCount() > appliedBefore) {
      // A command that repeats (BUILD's Place, mitcad#375) stays open after
      // its OK: the agent's OK places one and closes it, keeping what it
      // placed.
      session->cancel();
      waitUntil(ended, 60000);
    }
    if (!ended() && session) {
      // OK was refused: the panel stays open with what it says.
      refuseWith(tr("OK of %1 was refused: %2")
                     .arg(session->def().name,
                          panel.value(QStringLiteral("message")).toString(
                              panel.value(QStringLiteral("panel")).toObject().value(QStringLiteral("message")).toString())),
                 {{QStringLiteral("panel"), session->scriptState()}, {QStringLiteral("open"), true}});
    }
    result.insert(QStringLiteral("committed"), outcome->second);
    changes = outcome->second;
  };
  // A feature command's panel opens (on the worker in the AppKit frontend).
  const auto panelStarted = [this] {
#ifdef MITCAD_MACOS_NATIVE_UI
    if (m_nativeWindow) {
      return !m_session || m_nativeForm != nullptr;
    }
#endif
    static_cast<void>(this); // the Qt frontend starts it at once
    return true;
  };

  if (id.isEmpty() && edit.isEmpty()) {
    // What is open ends.
    if (m_session) {
      runPanel();
    } else if (m_mode == Mode::Sketch && finish == QStringLiteral("ok")) {
      result.insert(QStringLiteral("command"), QStringLiteral("sketch.finish"));
      if (!finishSketch()) {
        refuse(tr("The sketch stays open: computing the features after it was cancelled."));
      }
    } else if (m_mode == Mode::Sketch && finish == QStringLiteral("cancel")) {
      // Every change made since it was opened taken back, then left.
      result.insert(QStringLiteral("command"), QStringLiteral("sketch.cancel"));
      const QString name = m_sketch->model().name;
      for (int step = 0; step < 10000 && m_mode == Mode::Sketch && undoDepth() > m_sketchEnterDepth; ++step) {
        const int depth = undoDepth();
        undo();
        if (undoDepth() >= depth) {
          break; // refused
        }
      }
      if (m_mode == Mode::Sketch && !finishSketch()) {
        refuse(tr("The sketch stays open: computing the features after it was cancelled."));
      }
      qInfo().noquote() << QStringLiteral("MCP: an agent cancelled the sketch %1").arg(name);
    } else {
      refuse(tr("Nothing is open to finish: give id or edit."));
    }
  } else if (!edit.isEmpty()) {
    // As a double-click in the timeline: a sketch opens in sketch mode,
    // another feature in its panel.
    if (m_mode != Mode::Idle) {
      refuse(tr("Finish the sketch or command first."));
    }
    if (windowReadOnly()) {
      refuse(readOnlyReason());
    }
    if (!sketchExists(edit)) {
      refuse(tr("No feature %1 in the document.").arg(edit));
    }
    result.insert(QStringLiteral("command"), QStringLiteral("edit"));
    editFeature(edit);
    waitUntil(panelStarted, 60000);
    if (m_session) {
      runPanel();
    } else if (m_mode != Mode::Sketch) {
      refuse(m_statusLabel->text().isEmpty() ? tr("%1 could not be edited.").arg(edit) : m_statusLabel->text());
    }
  } else {
    const CommandDef* def = m_registry->find(id);
    if (def == nullptr) {
      refuse(tr("No command %1 in the command registry (app/COMMANDS.md).").arg(id));
    }
    if (opensDialog(id)) {
      refuse(tr("%1 opens a file dialog or another window, which an agent cannot use: run it in the application.")
                 .arg(def->name));
    }
    if (windowReadOnly() && !readOnlyCommand(*def)) {
      refuse(readOnlyReason());
    }
    if (!isAvailable(*def)) {
      refuse(tr("%1 is not available now (mode %2).").arg(def->name, modeName(m_mode)));
    }
    result.insert(QStringLiteral("command"), id);
    if (def->kind == CommandDef::Kind::Feature) {
      trigger(id);
      waitUntil(panelStarted, 60000);
      if (!m_session) {
        refuse(m_statusLabel->text().isEmpty() ? tr("%1 did not start.").arg(def->name) : m_statusLabel->text());
      }
      runPanel();
    } else {
      if (!inputs.isEmpty()) {
        refuse(tr("%1 has no panel: it takes no inputs.").arg(def->name));
      }
      trigger(id);
      waitUntil([this] { return !modelBusy(); }, 120000);
      // Entering and finishing sketches are the user's frame, not changes;
      // Undo and Redo move along the steps (a label would clear Redo).
      changes = id != QStringLiteral("sketch.finish") && id != QStringLiteral("sketch.create") &&
                id != QStringLiteral("edit.undo") && id != QStringLiteral("edit.redo");
    }
  }
  if (!guard.opened().isEmpty()) {
    refuse(tr("%1 opened the dialog %2, which was closed: an agent cannot answer dialogs; run it in the application.")
               .arg(id.isEmpty() ? edit : id, guard.opened()));
  }
  if (changes && undoDepth() > depthBefore) {
    mcpLabelAgentChange(depthBefore);
    if (inSketchBefore && m_mode == Mode::Sketch) {
      m_mcpSketchEdited = true;
    }
  }
  if (id.startsWith(QStringLiteral("view."))) {
    // A view command (Home, Fit, a standard view, ...): the camera where it
    // goes, at once, and in the reply (mitcad#405).
    m_viewer->settleCamera();
    result.insert(QStringLiteral("camera"), mcpCameraJson());
  }
  const QJsonArray lines = logTexts(logStart, 200);
  QJsonArray previews;
  for (const QJsonValue& line : lines) {
    if (line.toString().startsWith(QStringLiteral("Preview "))) {
      previews.append(line);
    }
  }
  const QJsonObject document = queryObject({{QStringLiteral("query"), QStringLiteral("document")}});
  result.insert(QStringLiteral("mode"), modeName(m_mode));
  result.insert(QStringLiteral("sketch"), m_sketch->isActive() ? QJsonValue(m_sketch->uid()) : QJsonValue());
  result.insert(QStringLiteral("undo"), document.value(QStringLiteral("undo")));
  result.insert(QStringLiteral("previews"), previews);
  result.insert(QStringLiteral("log"), lines);
  result.insert(QStringLiteral("hint"), m_statusLabel->text());
  qInfo().noquote() << QStringLiteral("MCP: an agent ran the UI command %1").arg(
      result.value(QStringLiteral("command")).toString());
  return result;
}

} // namespace mitcad
