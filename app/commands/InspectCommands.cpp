// SPDX-License-Identifier: MIT
// The INSPECT group of the SOLID tab (U4): Measure, Interference, Section
// Analysis, Print Analysis and Physical Properties, on the model's
// analysis queries (commands.md, "Analysis queries", "Print analysis").
// They show answers; only Section Analysis and Print Analysis leave
// something behind: OK keeps them in the document as analyses (the
// browser's Analysis folder, mitcad#41, mitcad#620), and while one is shown
// the bodies stay cut at its plane, or its overhangs and thin walls stay
// coloured over them.
#include "Commands.hpp"

#include <algorithm>
#include <cmath>
#include <memory>
#include <optional>

#include <QDialog>
#include <QDialogButtonBox>
#include <QJsonArray>
#include <QKeySequence>
#include <QLabel>
#include <QScrollArea>
#include <QSettings>
#include <QStringList>
#include <QVBoxLayout>
#include <QtLogging>
#include <QtMath>

#include <BRepAdaptor_Curve.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepGProp.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>

#include "../files/Print3d.hpp"
#include "../framework/Appearances.hpp"
#include "../framework/CommandRegistry.hpp"
#include "../framework/CommandSession.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Json.hpp"
#include "../framework/ModelShapes.hpp"
#include "../framework/Numbers.hpp"
#include "CommandSupport.hpp"
#include "PrintAnalysis.hpp"
#include "mitcad/geometry/import.hpp"
#include "mitcad/geometry/shape.hpp"

namespace mitcad {
namespace {

using namespace cmd;

QString n(double value, int decimals = 3) {
  QString text = QString::number(value, 'f', decimals);
  while (text.contains(QLatin1Char('.')) && (text.endsWith(QLatin1Char('0')) || text.endsWith(QLatin1Char('.')))) {
    text.chop(1);
  }
  return text == QStringLiteral("-0") ? QStringLiteral("0") : text;
}

QString xyz(const QJsonValue& value) {
  const QJsonArray a = value.toArray();
  return QStringLiteral("(%1, %2, %3)").arg(n(a.at(0).toDouble()), n(a.at(1).toDouble()), n(a.at(2).toDouble()));
}

// A circle or arc (mitcad#194): its radius with its centre, and the
// diameter, twice the radius before either is rounded for display.
void describeCircle(double radius, const QString& centre, QStringList& lines, QStringList& log) {
  const double diameter = 2.0 * radius;
  lines << QObject::tr("Radius: %1 mm, centre %2").arg(n(radius), centre);
  lines << QObject::tr("Diameter: %1 mm").arg(n(diameter));
  log << QStringLiteral("radius %1").arg(n(radius)) << QStringLiteral("diameter %1").arg(n(diameter));
}

// What the measure query says of one selection.
QString describeMeasured(const QJsonObject& item, QStringList& log) {
  QStringList lines;
  const QString kind = item.value(QStringLiteral("kind")).toString();
  lines << QStringLiteral("<b>%1</b>").arg(kind);
  const auto add = [&](const char* key, const QString& label, const QString& unit) {
    if (item.contains(QLatin1String(key))) {
      const QString value = n(item.value(QLatin1String(key)).toDouble());
      lines << QStringLiteral("%1: %2 %3").arg(label, value, unit);
      log << QStringLiteral("%1 %2").arg(QString::fromLatin1(key), value);
    }
  };
  add("volume", QObject::tr("Volume"), QStringLiteral("mm³"));
  add("area", QObject::tr("Area"), QStringLiteral("mm²"));
  add("length", QObject::tr("Length"), QStringLiteral("mm"));
  if (item.contains(QStringLiteral("point"))) {
    lines << QObject::tr("Position: %1").arg(xyz(item.value(QStringLiteral("point"))));
    log << QStringLiteral("point %1").arg(xyz(item.value(QStringLiteral("point"))));
  }
  const QJsonObject circle = item.value(QStringLiteral("circle")).toObject();
  if (!circle.isEmpty()) {
    describeCircle(circle.value(QStringLiteral("radius")).toDouble(), xyz(circle.value(QStringLiteral("center"))),
                   lines, log);
  }
  return lines.join(QStringLiteral("<br>"));
}

QJsonValue measureRef(const SelectionItem& item) {
  if (item.kind == SelectKind::Plane || item.kind == SelectKind::Axis || item.kind == SelectKind::Point) {
    return QJsonObject{{QStringLiteral("datum"), item.owner}};
  }
  return item.reference();
}

bool isSketchItem(const SelectionItem& item) {
  return item.kind == SelectKind::SketchCurve || item.kind == SelectKind::SketchPoint;
}

QString pointText(const gp_Pnt& p) { return QStringLiteral("(%1, %2, %3)").arg(n(p.X()), n(p.Y()), n(p.Z())); }

// The single straight edge of a shape, if it is one.
std::optional<gp_Dir> lineDirection(const TopoDS_Shape& shape) {
  TopExp_Explorer edges(shape, TopAbs_EDGE);
  if (!edges.More()) {
    return std::nullopt;
  }
  const BRepAdaptor_Curve curve(TopoDS::Edge(edges.Current()));
  edges.Next();
  if (edges.More() || curve.GetType() != GeomAbs_Line) {
    return std::nullopt;
  }
  return curve.Line().Direction();
}

// Measure with sketch geometry (P9): the model measures bodies and datums;
// sketch curves and points (in sketch mode, or shown sketches) are measured
// here on the shapes the view shows.
QString describeShape(const SelectionItem& item, const TopoDS_Shape& shape, QStringList& log) {
  QStringList lines;
  lines << QStringLiteral("<b>%1</b>").arg(SelectionItem::kindName(item.kind));
  if (shape.ShapeType() == TopAbs_VERTEX) {
    const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(shape));
    lines << QObject::tr("Position: %1").arg(pointText(p));
    log << QStringLiteral("point %1").arg(pointText(p));
    return lines.join(QStringLiteral("<br>"));
  }
  GProp_GProps props;
  BRepGProp::LinearProperties(shape, props);
  lines << QObject::tr("Length: %1 mm").arg(n(props.Mass()));
  log << QStringLiteral("length %1").arg(n(props.Mass()));
  TopExp_Explorer edges(shape, TopAbs_EDGE);
  if (edges.More()) {
    const BRepAdaptor_Curve curve(TopoDS::Edge(edges.Current()));
    if (curve.GetType() == GeomAbs_Circle) {
      const gp_Circ circle = curve.Circle();
      describeCircle(circle.Radius(), pointText(circle.Location()), lines, log);
    }
  }
  return lines.join(QStringLiteral("<br>"));
}

Inspection measureShapes(const SelectionItem& first, const SelectionItem* second, const CommandContext& context) {
  Inspection result;
  QStringList log;
  QStringList parts;
  const TopoDS_Shape a = context.itemShape(first);
  const TopoDS_Shape b = second != nullptr ? context.itemShape(*second) : TopoDS_Shape();
  if (a.IsNull() || (second != nullptr && b.IsNull())) {
    result.error = QObject::tr("The selection is not there any more.");
    return result;
  }
  parts << describeShape(first, a, log);
  if (second != nullptr) {
    parts << describeShape(*second, b, log);
    QStringList lines;
    BRepExtrema_DistShapeShape distance(a, b);
    if (distance.IsDone() && distance.NbSolution() > 0) {
      lines << QObject::tr("<b>Distance: %1 mm</b>").arg(n(distance.Value()));
      log << QStringLiteral("distance %1").arg(n(distance.Value()));
      const gp_Pnt p = distance.PointOnShape1(1);
      const gp_Pnt q = distance.PointOnShape2(1);
      if (p.Distance(q) > 1e-9) {
        result.shape = BRepBuilderAPI_MakeEdge(p, q).Edge();
      }
      lines << QObject::tr("From %1 to %2").arg(pointText(p), pointText(q));
    }
    const auto da = lineDirection(a);
    const auto db = lineDirection(b);
    if (da && db) {
      // The angle between the lines, 0 to 90 degrees.
      const QString angle = degrees(std::acos(std::min(1.0, std::abs(da->Dot(*db)))));
      lines << QObject::tr("<b>Angle: %1°</b>").arg(angle);
      log << QStringLiteral("angle %1 deg").arg(angle);
    }
    parts << lines.join(QStringLiteral("<br>"));
  }
  result.text = parts.join(QStringLiteral("<br><br>"));
  result.log = log.join(QStringLiteral(", "));
  return result;
}

// ---------------------------------------------------------------------------
// Measure

CommandDef measureCommand() {
  CommandDef def;
  def.id = QStringLiteral("inspect.measure");
  def.name = QObject::tr("Measure");
  def.icon = QStringLiteral("measure");
  def.tooltip = QObject::tr("Distances, angles, lengths, areas and positions of what is picked");
  def.shortcut = QKeySequence(Qt::Key_I);
  def.group = QStringLiteral("INSPECT");
  def.pinned = true;
  def.keywords = {QStringLiteral("distance"), QStringLiteral("angle"), QStringLiteral("dimension")};
  // Faces, edges and vertices, which Measure picks by default, or
  // whole bodies (the Selection Filter; a body would win over its faces
  // in the view, so it is a choice); datums that are shown (construction
  // features, the origin when its light bulb is on); and sketch curves and
  // points (P9), also in sketch mode, where Measure starts without leaving
  // the sketch.
  def.mode = CommandDef::Mode::Any;
  const SelectFilter kinds = SelectKind::Face | SelectKind::Edge | SelectKind::Vertex |
                             SelectKind::Plane | SelectKind::Axis | SelectKind::Point |
                             SelectKind::SketchCurve | SelectKind::SketchPoint;
  const auto bodies = [](const CommandState& s) {
    return s.choice(QStringLiteral("filter")) == QStringLiteral("bodies");
  };
  const auto geometry = [bodies](const CommandState& s) { return !bodies(s); };
  def.inputs = {
      choiceInput(QStringLiteral("filter"), QObject::tr("Selection Filter"),
                  {{QStringLiteral("geometry"), QObject::tr("Faces, Edges and Vertices")},
                   {QStringLiteral("bodies"), QObject::tr("Bodies")}},
                  QStringLiteral("geometry")),
      selectionInput(QStringLiteral("a"), QObject::tr("Selection 1"), kinds, 1, 1).withVisible(geometry),
      selectionInput(QStringLiteral("b"), QObject::tr("Selection 2"), kinds, 0, 1).withVisible(geometry),
      selectionInput(QStringLiteral("body_a"), QObject::tr("Body 1"), SelectKind::Body, 1, 1).withVisible(bodies),
      selectionInput(QStringLiteral("body_b"), QObject::tr("Body 2"), SelectKind::Body, 0, 1).withVisible(bodies),
  };
  for (InputDef& input : def.inputs) {
    input.showsOrigin = false;
  }
  def.inspect = [bodies](const CommandState& state, const CommandContext& context) {
    Inspection result;
    const bool whole = bodies(state);
    const Selection& first = state.items(whole ? QStringLiteral("body_a") : QStringLiteral("a"));
    const Selection& second = state.items(whole ? QStringLiteral("body_b") : QStringLiteral("b"));
    if (isSketchItem(first.first()) || (!second.isEmpty() && isSketchItem(second.first()))) {
      return measureShapes(first.first(), second.isEmpty() ? nullptr : &second.first(), context);
    }
    QJsonObject request{{QStringLiteral("query"), QStringLiteral("measure")},
                        {QStringLiteral("a"), measureRef(first.first())}};
    if (!second.isEmpty()) {
      request.insert(QStringLiteral("b"), measureRef(second.first()));
    }
    const QJsonObject answer = context.queryObject(request);
    QStringList log;
    QStringList parts;
    parts << describeMeasured(answer.value(QStringLiteral("a")).toObject(), log);
    if (answer.contains(QStringLiteral("b"))) {
      parts << describeMeasured(answer.value(QStringLiteral("b")).toObject(), log);
    }
    const QJsonObject between = answer.value(QStringLiteral("between")).toObject();
    if (!between.isEmpty()) {
      QStringList lines;
      const double distance = between.value(QStringLiteral("distance")).toDouble();
      lines << QObject::tr("<b>Distance: %1 mm</b>").arg(n(distance));
      log << QStringLiteral("distance %1").arg(n(distance));
      if (between.value(QStringLiteral("angle")).isDouble()) {
        const QString angle = degrees(between.value(QStringLiteral("angle")).toDouble());
        lines << QObject::tr("<b>Angle: %1°</b>").arg(angle);
        log << QStringLiteral("angle %1 deg").arg(angle);
      }
      const QJsonArray a = between.value(QStringLiteral("on_a")).toArray();
      const QJsonArray c = between.value(QStringLiteral("on_b")).toArray();
      if (a.size() == 3 && c.size() == 3) {
        const gp_Pnt p = pointOf(a);
        const gp_Pnt q = pointOf(c);
        lines << QObject::tr("Δx %1, Δy %2, Δz %3 mm")
                     .arg(n(q.X() - p.X()), n(q.Y() - p.Y()), n(q.Z() - p.Z()));
        lines << QObject::tr("From %1 to %2").arg(xyz(a), xyz(c));
        if (p.Distance(q) > 1e-9) {
          result.shape = BRepBuilderAPI_MakeEdge(p, q).Edge();
        }
      }
      parts << lines.join(QStringLiteral("<br>"));
    }
    result.text = parts.join(QStringLiteral("<br><br>"));
    result.log = log.join(QStringLiteral(", "));
    return result;
  };
  return def;
}

// ---------------------------------------------------------------------------
// Interference

CommandDef interferenceCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("inspect.interference");
  def.name = QObject::tr("Interference");
  def.icon = QStringLiteral("interference");
  def.tooltip = QObject::tr("Where bodies overlap, and by how much");
  def.group = QStringLiteral("INSPECT");
  def.keywords = {QStringLiteral("clash"), QStringLiteral("overlap"), QStringLiteral("collision")};
  def.inputs = {
      selectionInput(QStringLiteral("bodies"), QObject::tr("Bodies"), SelectKind::Body, 0, 0)
          .withTooltip(QObject::tr("The bodies to check; none: all")),
  };
  def.enabled = [&context] { return context.queryArray(QStringLiteral("bodies")).size() > 1; };
  def.inspect = [](const CommandState& state, const CommandContext& model) {
    Inspection result;
    QJsonObject request{{QStringLiteral("query"), QStringLiteral("interference")}};
    const Selection& bodies = state.items(QStringLiteral("bodies"));
    if (!bodies.isEmpty()) {
      request.insert(QStringLiteral("bodies"), bodyUids(bodies));
    }
    const QJsonObject answer = model.queryObject(request);
    const QJsonArray pairs = answer.value(QStringLiteral("pairs")).toArray();
    TopoDS_Compound overlaps;
    BRep_Builder builder;
    builder.MakeCompound(overlaps);
    QStringList rows;
    QStringList log;
    for (const QJsonValue& value : pairs) {
      const QJsonObject pair = value.toObject();
      const QJsonArray names = pair.value(QStringLiteral("names")).toArray();
      const QString volume = n(pair.value(QStringLiteral("volume")).toDouble());
      rows << QStringLiteral("<tr><td>%1</td><td>%2</td><td align='right'>%3 mm³</td></tr>")
                  .arg(names.at(0).toString().toHtmlEscaped(), names.at(1).toString().toHtmlEscaped(), volume);
      log << QStringLiteral("%1 x %2: %3").arg(names.at(0).toString(), names.at(1).toString(), volume);
      const TopoDS_Shape overlap = model.analysisShape({{QStringLiteral("shape"), QStringLiteral("interference")},
                                                        {QStringLiteral("a"), pair.value(QStringLiteral("a"))},
                                                        {QStringLiteral("b"), pair.value(QStringLiteral("b"))}});
      if (!overlap.IsNull()) {
        builder.Add(overlaps, overlap);
      }
    }
    if (pairs.isEmpty()) {
      result.text = QObject::tr("No interference between %1 bodies.").arg(answer.value(QStringLiteral("bodies")).toInt());
      result.log = QStringLiteral("none");
    } else {
      result.text = QObject::tr("%1 interference(s):").arg(pairs.size()) +
                    QStringLiteral("<table width='100%'>%1</table>").arg(rows.join(QString()));
      result.log = log.join(QStringLiteral("; "));
      result.shape = overlaps;
      result.red = true;
    }
    return result;
  };
  return def;
}

// ---------------------------------------------------------------------------
// Section Analysis

// Preview and saved analyses resolve the same occurrence-aware plane in
// the model. The displayed item is in design coordinates, while a datum
// query or a body's underlying shape is in its component's coordinates.
std::optional<std::pair<gp_Pnt, gp_Dir>> sectionPlane(const CommandState& state, const CommandContext& context,
                                                    bool basePlane = false) {
  const Selection& plane = state.items(QStringLiteral("plane"));
  if (plane.isEmpty()) {
    return std::nullopt;
  }
  const double offset = basePlane ? 0.0 : state.value(QStringLiteral("offset"));
  if (!std::isfinite(offset)) {
    return std::nullopt;
  }
  try {
    const QJsonObject resolved = context.queryObject({
        {QStringLiteral("query"), QStringLiteral("section_plane")},
        {QStringLiteral("plane"), plane.first().reference()},
        {QStringLiteral("occurrence"), plane.first().occurrence},
        {QStringLiteral("offset"), offset},
        {QStringLiteral("flip"), !basePlane && state.checked(QStringLiteral("flip"))}});
    return std::make_pair(pointOf(resolved.value(QStringLiteral("origin"))),
                          gp_Dir(vectorOf(resolved.value(QStringLiteral("normal")))));
  } catch (const std::exception&) {
    return std::nullopt;
  }
}

CommandDef sectionCommand() {
  CommandDef def;
  def.id = QStringLiteral("inspect.section");
  def.name = QObject::tr("Section Analysis");
  def.icon = QStringLiteral("section-analysis");
  def.tooltip = QObject::tr("Shows the bodies cut at a plane, with the cut's area");
  def.group = QStringLiteral("INSPECT");
  def.pinned = true;
  def.keywords = {QStringLiteral("cut"), QStringLiteral("clip"), QStringLiteral("cross section")};
  def.keepsSection = true;
  def.inputs = {
      selectionInput(QStringLiteral("plane"), QObject::tr("Faces/Plane"), kPlaneKinds, 1, 1).withAccepts(isPlanar),
      valueInput(QStringLiteral("offset"), QObject::tr("Distance"), ValueKind::Length, QStringLiteral("0 mm"))
          .withManipulator([](const CommandState& state, const CommandContext& context) -> std::optional<Manipulator> {
            const Selection& plane = state.items(QStringLiteral("plane"));
            if (plane.isEmpty()) {
              return std::nullopt;
            }
            const auto frame = sectionPlane(state, context, true);
            const auto center = centerOf(plane, context);
            if (!frame || !center) {
              return std::nullopt;
            }
            gp_Pnt origin = *center;
            origin.Translate(-gp_Vec(frame->second) * gp_Vec(frame->first, origin).Dot(gp_Vec(frame->second)));
            return arrow(origin, frame->second);
          }),
      flipInput(QStringLiteral("flip"), QObject::tr("Flip")).withTooltip(QObject::tr("Cut away the other side")),
  };
  def.inspect = [](const CommandState& state, const CommandContext& context) {
    Inspection result;
    const auto plane = sectionPlane(state, context);
    if (!plane) {
      result.error = QObject::tr("The plane cannot be found.");
      return result;
    }
    const QJsonObject fixed{{QStringLiteral("origin"), QJsonArray{plane->first.X(), plane->first.Y(), plane->first.Z()}},
                            {QStringLiteral("normal"),
                             QJsonArray{plane->second.X(), plane->second.Y(), plane->second.Z()}}};
    // The view shows the bodies cut at the plane with the cut faces as caps
    // (MainWindow::showSectionCaps, U5); the panel's totals of all bodies
    // are measured after the view has its cut bodies, off the UI thread
    // (mitcad#438: seconds on a large design).
    result.text = QObject::tr("Section area: …<br>Outline length: …");
    result.log = QStringLiteral("measuring");
    result.section = fixed;
    result.laterQuery = {{QStringLiteral("query"), QStringLiteral("section")},
                         {QStringLiteral("plane"), fixed},
                         {QStringLiteral("world"), true}};
    result.measured = [](const QJsonObject& answer, Inspection& shown) {
      const double area = answer.value(QStringLiteral("area")).toDouble();
      const double length = answer.value(QStringLiteral("length")).toDouble();
      shown.text = QObject::tr("Section area: %1 mm²<br>Outline length: %2 mm").arg(n(area), n(length));
      shown.log = QStringLiteral("area %1, length %2").arg(n(area), n(length));
    };
    return result;
  };
  // OK keeps it in the document as an analysis (mitcad#41): the plane as
  // a reference, so that a face's section follows the model.
  def.build = [](const CommandState& state, const CommandContext&) {
    const Selection& plane = state.items(QStringLiteral("plane"));
    if (plane.isEmpty()) {
      return Built::failure(QObject::tr("Select a plane."), QStringLiteral("plane"));
    }
    const double offset = state.value(QStringLiteral("offset"));
    if (!std::isfinite(offset)) {
      return Built::failure(QObject::tr("Distance: check the value."), QStringLiteral("offset"));
    }
    QJsonObject definition{{QStringLiteral("type"), QStringLiteral("section")},
                      {QStringLiteral("plane"), refsOf(plane).first()},
                      {QStringLiteral("offset"), offset},
                      {QStringLiteral("flip"), state.checked(QStringLiteral("flip"))}};
    if (!plane.first().occurrence.isEmpty()) {
      definition.insert(QStringLiteral("occurrence"), plane.first().occurrence);
    }
    return Built::of(definition);
  };
  def.load = [](const QJsonObject& analysis, CommandState& state, const CommandContext& context) {
    SelectionItem plane = itemOf(analysis.value(QStringLiteral("plane")), context);
    if (plane.isValid()) {
      if (analysis.value(QStringLiteral("occurrence")).isString()) {
        plane.occurrence = analysis.value(QStringLiteral("occurrence")).toString();
      } else {
        // Older saved analyses had no occurrence. Resolve only an
        // unambiguous component; never silently pick a repeated instance.
        plane.occurrence = context.queryObject({{QStringLiteral("query"), QStringLiteral("section_plane")},
            {QStringLiteral("plane"), analysis.value(QStringLiteral("plane"))}})
                               .value(QStringLiteral("occurrence")).toString();
      }
      state.items(QStringLiteral("plane")) = {plane};
    }
    state.setText(QStringLiteral("offset"), lengthText(analysis.value(QStringLiteral("offset")).toDouble(), context));
    state.setChecked(QStringLiteral("flip"), analysis.value(QStringLiteral("flip")).toBool());
  };
  return def;
}

// ---------------------------------------------------------------------------
// Print Analysis (mitcad#620)

// Solids and mesh bodies can be printed (as 3D Print takes them); sheet and
// empty bodies cannot.
bool printable(const std::shared_ptr<geometry::Shape>& shape) {
  const geometry::BodyKind kind = shape ? geometry::body_kind(shape->occt()) : geometry::BodyKind::Empty;
  return kind == geometry::BodyKind::Solid || kind == geometry::BodyKind::Mesh;
}

// The Material row: the bodies' own (PLA for a body without one), or one
// of the physical materials.
QVector<QPair<QString, QString>> printMaterials() {
  QVector<QPair<QString, QString>> choices{{QStringLiteral("bodies"), QObject::tr("The bodies' material (else PLA)")}};
  choices += physicalMaterials();
  return choices;
}

// The panel's inputs from settings: the settings group's (a new analysis)
// or an analysis' own (its definition).
void setPrintSettings(CommandState& state, const PrintAnalysisSettings& s, const CommandContext& context) {
  state.setChecked(QStringLiteral("overhang"), s.overhang);
  state.setText(QStringLiteral("angle"), QStringLiteral("%1 deg").arg(n(s.angle)));
  state.setChecked(QStringLiteral("thin_wall"), s.thinWall);
  state.setText(QStringLiteral("minimum"), lengthText(s.minimum, context));
  state.setChecked(QStringLiteral("estimate"), s.estimate);
  state.setChoice(QStringLiteral("material"), s.material.isEmpty() ? QStringLiteral("bodies") : s.material);
  state.setText(QStringLiteral("line_width"), lengthText(s.lineWidth, context));
  state.setText(QStringLiteral("walls"), QString::number(s.walls));
  state.setText(QStringLiteral("layer_height"), lengthText(s.layerHeight, context));
  state.setText(QStringLiteral("top_bottom_layers"), QString::number(s.topBottomLayers));
  state.setText(QStringLiteral("infill"), n(s.infill * 100.0));
  state.setText(QStringLiteral("filament_diameter"), lengthText(s.filamentDiameter, context));
  state.setText(QStringLiteral("price"), s.pricePerKg ? n(*s.pricePerKg, 2) : QString());
}

// The panel's values as an analysis' definition, or what is wrong (and in
// which input). Values of a part switched off are not checked: they keep
// the defaults when they cannot be read.
Built printDefinition(const CommandState& state) {
  PrintAnalysisSettings s;
  const auto valueOf = [&state](const char* key) { return state.value(QLatin1String(key)); };
  const auto whole = [](double value) { return value >= 0.0 && value <= 1000.0 && std::floor(value) == value; };
  s.overhang = state.checked(QStringLiteral("overhang"));
  if (s.overhang) {
    const double angle = qRadiansToDegrees(valueOf("angle"));
    if (!(angle >= 0.0 && angle <= 90.0)) {
      return Built::failure(QObject::tr("Angle: between 0 and 90 deg."), QStringLiteral("angle"));
    }
    s.angle = angle;
  }
  s.thinWall = state.checked(QStringLiteral("thin_wall"));
  if (s.thinWall) {
    const double minimum = valueOf("minimum");
    if (!(minimum > 0.0 && minimum <= 100.0)) {
      return Built::failure(QObject::tr("Minimum wall: more than 0 mm."), QStringLiteral("minimum"));
    }
    s.minimum = minimum;
  }
  s.estimate = state.checked(QStringLiteral("estimate"));
  if (s.estimate) {
    const QString material = state.choice(QStringLiteral("material"));
    s.material = material == QStringLiteral("bodies") ? QString() : material;
    const double lineWidth = valueOf("line_width");
    if (!(lineWidth > 0.0 && lineWidth <= 100.0)) {
      return Built::failure(QObject::tr("Line width: more than 0 mm."), QStringLiteral("line_width"));
    }
    s.lineWidth = lineWidth;
    const double walls = valueOf("walls");
    if (!whole(walls)) {
      return Built::failure(QObject::tr("Walls: a whole number, 0 or more."), QStringLiteral("walls"));
    }
    s.walls = static_cast<int>(walls);
    const double layerHeight = valueOf("layer_height");
    if (!(layerHeight > 0.0 && layerHeight <= 100.0)) {
      return Built::failure(QObject::tr("Layer height: more than 0 mm."), QStringLiteral("layer_height"));
    }
    s.layerHeight = layerHeight;
    const double layers = valueOf("top_bottom_layers");
    if (!whole(layers)) {
      return Built::failure(QObject::tr("Top/bottom layers: a whole number, 0 or more."),
                            QStringLiteral("top_bottom_layers"));
    }
    s.topBottomLayers = static_cast<int>(layers);
    const double infill = valueOf("infill");
    if (!(infill >= 0.0 && infill <= 100.0)) {
      return Built::failure(QObject::tr("Infill: between 0 and 100 %."), QStringLiteral("infill"));
    }
    s.infill = infill / 100.0;
    const double diameter = valueOf("filament_diameter");
    if (!(diameter > 0.0 && diameter <= 100.0)) {
      return Built::failure(QObject::tr("Filament diameter: more than 0 mm."), QStringLiteral("filament_diameter"));
    }
    s.filamentDiameter = diameter;
    const QString price = state.text(QStringLiteral("price")).trimmed();
    if (!price.isEmpty()) {
      const std::optional<double> perKg = parsePlainNumber(price);
      if (!perKg || !std::isfinite(*perKg) || *perKg < 0.0) {
        return Built::failure(QObject::tr("Price: a number, or empty for no cost."), QStringLiteral("price"));
      }
      s.pricePerKg = *perKg;
    }
  }
  // Each body once, in the order picked (a body shown more than once is
  // one body of the analysis).
  QJsonArray bodies;
  for (const SelectionItem& item : state.items(QStringLiteral("bodies"))) {
    if (!bodies.contains(item.owner)) {
      bodies.append(item.owner);
    }
  }
  if (bodies.isEmpty()) {
    return Built::failure(QObject::tr("Select the bodies."), QStringLiteral("bodies"));
  }
  const Selection& bed = state.items(QStringLiteral("bed_face"));
  if (!bed.isEmpty() && !bodies.contains(bed.first().owner)) {
    return Built::failure(QObject::tr("The bed face is not a face of the bodies."), QStringLiteral("bed_face"));
  }
  return Built::of(s.def(bodies, bed.isEmpty() ? QJsonValue() : bed.first().reference()));
}

// What a long query's progress says.
QString printStage(const QJsonObject& request) {
  return request.value(QStringLiteral("query")).toString() == QStringLiteral("print_thickness")
             ? QObject::tr("Measuring the walls")
             : QObject::tr("Finding the overhangs");
}

// Overhangs and thickness, which may take long on large meshes: in the
// window as a model job on the model's worker, with the progress and its
// Cancel (CommandHost::jobQuery); an inspection of the native window runs on
// the worker already and asks the document as it is.
QJsonObject printQuery(const CommandContext& context, const QJsonObject& request, bool slow) {
  if (slow) {
    if (const auto* host = dynamic_cast<const CommandHost*>(&context)) {
      return const_cast<CommandHost*>(host)->jobQuery(request, printStage(request));
    }
  }
  return context.queryObject(request);
}

CommandDef printAnalysisCommand(const CommandContext& model) {
  CommandDef def;
  def.id = QStringLiteral("inspect.print_analysis");
  def.name = QObject::tr("Print Analysis");
  def.icon = QStringLiteral("print-analysis");
  def.tooltip = QObject::tr("Overhangs, thin walls and an estimate of weight, filament and cost");
  def.group = QStringLiteral("INSPECT");
  def.keywords = {QStringLiteral("3d print"), QStringLiteral("overhang"), QStringLiteral("thin wall"),
                  QStringLiteral("filament"), QStringLiteral("weight"), QStringLiteral("support"),
                  QStringLiteral("print estimate")};
  // OK keeps it in the document as an analysis, shown (mitcad#41).
  def.keepsSection = true;
  const auto checked = [](const QString& key) {
    return [key](const CommandState& state) { return state.checked(key); };
  };
  const auto overhang = checked(QStringLiteral("overhang"));
  const auto thin = checked(QStringLiteral("thin_wall"));
  const auto estimate = checked(QStringLiteral("estimate"));
  def.inputs = {
      selectionInput(QStringLiteral("bodies"), QObject::tr("Bodies"), SelectKind::Body, 1, 0)
          .withAccepts([&model](const SelectionItem& item) { return printable(model.bodyShape(item.owner)); })
          .withTooltip(QObject::tr("The bodies to check: a click on a body adds or removes it")),
      selectionInput(QStringLiteral("bed_face"), QObject::tr("Bed face"), SelectKind::Face, 0, 1)
          .withAccepts(isPlanar)
          .withTooltip(QObject::tr("The planar face that lies on the bed; none: -Z is down")),
      checkInput(QStringLiteral("overhang"), QObject::tr("Overhangs"), true)
          .withTooltip(QObject::tr("Faces that face down more steeply than the angle need support")),
      valueInput(QStringLiteral("angle"), QObject::tr("Angle"), ValueKind::Angle, QStringLiteral("45 deg"))
          .withVisible(overhang)
          .withTooltip(QObject::tr("The steepest overhang the printer makes without support, from the vertical")),
      checkInput(QStringLiteral("thin_wall"), QObject::tr("Thin walls"), true)
          .withTooltip(QObject::tr("Walls thinner than the printer can make")),
      valueInput(QStringLiteral("minimum"), QObject::tr("Minimum wall"), ValueKind::Length, QStringLiteral("0.8 mm"))
          .withVisible(thin)
          .withTooltip(QObject::tr("The thinnest wall the printer makes: about twice the line width")),
      checkInput(QStringLiteral("estimate"), QObject::tr("Estimate"), true)
          .withTooltip(QObject::tr("About the weight, filament length and cost of the print")),
      choiceInput(QStringLiteral("material"), QObject::tr("Material"), printMaterials(), QStringLiteral("bodies"))
          .withVisible(estimate),
      valueInput(QStringLiteral("line_width"), QObject::tr("Line width"), ValueKind::Length, QStringLiteral("0.4 mm"))
          .withVisible(estimate),
      valueInput(QStringLiteral("walls"), QObject::tr("Walls"), ValueKind::Unitless, QStringLiteral("2"))
          .withVisible(estimate)
          .withTooltip(QObject::tr("The perimeters around each layer")),
      valueInput(QStringLiteral("layer_height"), QObject::tr("Layer height"), ValueKind::Length,
                 QStringLiteral("0.2 mm"))
          .withVisible(estimate),
      valueInput(QStringLiteral("top_bottom_layers"), QObject::tr("Top/bottom layers"), ValueKind::Unitless,
                 QStringLiteral("4"))
          .withVisible(estimate),
      valueInput(QStringLiteral("infill"), QObject::tr("Infill %"), ValueKind::Unitless, QStringLiteral("15"))
          .withVisible(estimate)
          .withTooltip(QObject::tr("How much of the inside is filled, in per cent")),
      valueInput(QStringLiteral("filament_diameter"), QObject::tr("Filament diameter"), ValueKind::Length,
                 QStringLiteral("1.75 mm"))
          .withVisible(estimate),
      textInput(QStringLiteral("price"), QObject::tr("Price €/kg"))
          .withVisible(estimate)
          .withTooltip(QObject::tr("The filament's price per kilogram; empty: no cost")),
  };
  for (InputDef& input : def.inputs) {
    input.showsOrigin = false;
  }
  // What the selection stands for (a face, edge or vertex its body, an
  // occurrence the bodies shown in it), else every shown body; each body
  // once, sheets and empty bodies left out (as 3D Print starts).
  def.preselect = [](const Selection& before, CommandState& state, const CommandContext& context) {
    bool selected = false;
    const Selection start = printStartBodies(before, context.queryArray(QStringLiteral("instances")), &selected);
    Selection bodies;
    QStringList seen;
    for (const SelectionItem& item : start) {
      if (seen.contains(item.owner)) {
        continue;
      }
      seen << item.owner;
      if (printable(context.bodyShape(item.owner))) {
        bodies.append(item);
      }
    }
    state.setItems(QStringLiteral("bodies"), bodies);
  };
  // A new analysis starts with the settings group's numbers.
  def.init = [](CommandState& state, const CommandContext& context) {
    QSettings settings;
    setPrintSettings(state, PrintAnalysisSettings::load(settings), context);
  };
  def.inspect = [](const CommandState& state, const CommandContext& context) {
    Inspection result;
    const Built built = printDefinition(state);
    if (!built.error.isEmpty()) {
      result.error = built.error;
      return result;
    }
    PrintAnswers answers;
    const QString error = runPrintAnalysis(
        built.def,
        [&context](const QJsonObject& request, bool slow) { return printQuery(context, request, slow); }, answers);
    if (!error.isEmpty()) {
      result.error = error;
      return result;
    }
    const BodyNames name = [&context](const QString& uid) { return context.bodyName(uid); };
    result.text = printAnalysisText(answers, name);
    result.log = printAnalysisLog(answers, name);
    // Drawn over the bodies (CommandHost::showPrintAnalysis).
    result.printOverlay = printOverlayJson(built.def, answers, QObject::tr("Print Analysis"));
    return result;
  };
  // OK keeps it (add_analysis, or edit_analysis of the one edited), and the
  // numbers become the settings group's for the next one. An inspection's
  // `build` runs only then (CommandSession::keepAnalysis).
  def.build = [](const CommandState& state, const CommandContext&) {
    const Built built = printDefinition(state);
    if (built.error.isEmpty()) {
      QSettings settings;
      PrintAnalysisSettings::fromDef(built.def).save(settings);
    }
    return built;
  };
  def.load = [](const QJsonObject& analysis, CommandState& state, const CommandContext& context) {
    state.setItems(QStringLiteral("bodies"), bodiesOf(analysis.value(QStringLiteral("bodies")).toArray()));
    const QJsonValue bed = analysis.value(QStringLiteral("bed_face"));
    if (bed.isObject() || bed.isString()) {
      const SelectionItem face = itemOf(bed, context);
      if (face.isValid()) {
        state.items(QStringLiteral("bed_face")) = {face};
      }
    }
    setPrintSettings(state, PrintAnalysisSettings::fromDef(analysis), context);
  };
  return def;
}

// ---------------------------------------------------------------------------
// Physical Properties

Inspection propertiesInspection(const QJsonObject& answer) {
  QString html = QStringLiteral("<table cellspacing='4'>");
  const auto row = [&html](const QString& label, const QString& value) {
    html += QStringLiteral("<tr><td>%1</td><td>%2</td></tr>").arg(label, value);
  };
  QStringList log;
  for (const QJsonValue& value : answer.value(QStringLiteral("bodies")).toArray()) {
    const QJsonObject body = value.toObject();
    const QString name = body.value(QStringLiteral("name")).toString();
    html += QStringLiteral("<tr><td colspan='2'><b>%1</b></td></tr>").arg(name.toHtmlEscaped());
    row(QObject::tr("Material"), body.value(QStringLiteral("material")).toString(QStringLiteral("steel")));
    row(QObject::tr("Density"), QStringLiteral("%1 g/cm³").arg(n(body.value(QStringLiteral("density")).toDouble(), 4)));
    row(QObject::tr("Mass"), QStringLiteral("%1 kg").arg(n(body.value(QStringLiteral("mass")).toDouble(), 6)));
    row(QObject::tr("Volume"), QStringLiteral("%1 mm³").arg(n(body.value(QStringLiteral("volume")).toDouble())));
    row(QObject::tr("Area"), QStringLiteral("%1 mm²").arg(n(body.value(QStringLiteral("area")).toDouble())));
    row(QObject::tr("Center of Mass"), xyz(body.value(QStringLiteral("center_of_mass"))) + QStringLiteral(" mm"));
    const QJsonArray moments = body.value(QStringLiteral("principal_moments")).toArray();
    if (moments.size() == 3) {
      row(QObject::tr("Principal Moments"), QStringLiteral("%1, %2, %3 kg mm²")
                                               .arg(n(moments[0].toDouble()), n(moments[1].toDouble()),
                                                    n(moments[2].toDouble())));
    }
    log << QStringLiteral("%1 volume %2 mass %3 center %4")
               .arg(name, n(body.value(QStringLiteral("volume")).toDouble()),
                    n(body.value(QStringLiteral("mass")).toDouble(), 6),
                    xyz(body.value(QStringLiteral("center_of_mass"))));
  }
  const QJsonObject total = answer.value(QStringLiteral("total")).toObject();
  html += QStringLiteral("<tr><td colspan='2'><b>%1</b></td></tr>").arg(QObject::tr("Total"));
  row(QObject::tr("Mass"), QStringLiteral("%1 kg").arg(n(total.value(QStringLiteral("mass")).toDouble(), 6)));
  row(QObject::tr("Volume"), QStringLiteral("%1 mm³").arg(n(total.value(QStringLiteral("volume")).toDouble())));
  row(QObject::tr("Center of Mass"), xyz(total.value(QStringLiteral("center_of_mass"))) + QStringLiteral(" mm"));
  html += QStringLiteral("</table>");
  Inspection result;
  result.text = html;
  result.log = log.join(QStringLiteral("; "));
  return result;
}

void presentProperties(const Inspection& result, QWidget* window) {
  qDebug().noquote() << QStringLiteral("Physical properties: %1").arg(result.log);
  QDialog dialog(window);
  dialog.setWindowTitle(QObject::tr("Properties"));
  auto* layout = new QVBoxLayout(&dialog);
  auto* label = new QLabel(result.text);
  label->setTextInteractionFlags(Qt::TextSelectableByMouse);
  auto* scroll = new QScrollArea;
  scroll->setWidget(label);
  scroll->setWidgetResizable(true);
  layout->addWidget(scroll);
  auto* buttons = new QDialogButtonBox(QDialogButtonBox::Close);
  QObject::connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
  layout->addWidget(buttons);
  dialog.resize(420, 380);
  prepareModal(&dialog);
  execDialog(&dialog);
}

void showProperties(const CommandContext& context, QWidget* window, const Selection& selection) {
  QJsonObject request{{QStringLiteral("query"), QStringLiteral("properties")}};
  QJsonArray uids;
  for (const SelectionItem& item : selection) {
    const bool onBody = item.kind == SelectKind::Body || item.kind == SelectKind::Face ||
                        item.kind == SelectKind::Edge || item.kind == SelectKind::Vertex;
    if (onBody && !uids.contains(item.owner)) {
      uids.append(item.owner);
    }
  }
  if (!uids.isEmpty()) {
    request.insert(QStringLiteral("bodies"), uids);
  }
  auto* host = dynamic_cast<CommandHost*>(const_cast<CommandContext*>(&context));
  if (host && host->asynchronousCommands()) {
    host->modelInspectionAsync(QObject::tr("Physical Properties"), window,
        [request](const Document& document) {
          return propertiesInspection(parseObject(document.query(rustStr(compactJson(request)))));
        }, [window](const Inspection& result, const CommandHost::AsyncResult& outcome) {
          if (outcome.succeeded()) {
            presentProperties(result, window);
          } else if (!outcome.cancelled) {
            qWarning().noquote() << "Physical properties:" << outcome.error;
          }
        });
    return;
  }
  try {
    presentProperties(propertiesInspection(context.queryObject(request)), window);
  } catch (const std::exception& e) {
    qWarning().noquote() << "Physical properties:" << e.what();
  }
}

} // namespace

void registerInspectCommands(CommandRegistry& registry, const CommandContext& context,
                             QWidget* window, std::function<Selection()> selection) {
  registry.add(measureCommand());
  registry.add(interferenceCommand(context));
  registry.add(sectionCommand());
  registry.add(printAnalysisCommand(context));

  CommandDef properties;
  properties.id = QStringLiteral("inspect.properties");
  properties.name = QObject::tr("Physical Properties");
  properties.icon = QStringLiteral("properties");
  properties.tooltip = QObject::tr("Mass, volume, area and centre of mass of the selected bodies (all when none)");
  properties.group = QStringLiteral("INSPECT");
  properties.kind = CommandDef::Kind::Action;
  properties.keywords = {QStringLiteral("mass"), QStringLiteral("weight"), QStringLiteral("center of mass"),
                         QStringLiteral("inertia")};
  properties.enabled = [&context] { return hasBodies(context); };
  properties.run = [&context, window, selection] { showProperties(context, window, selection()); };
  registry.add(properties);
}

} // namespace mitcad
