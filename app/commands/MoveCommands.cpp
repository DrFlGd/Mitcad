// SPDX-License-Identifier: MIT
// Scale, Combine, Move/Copy, Align, Physical Material and Appearance
// (commands.md, "move, align, scale", "combine", "Visibility and
// display"). Move/Copy's free move has
// handles: arrows along X, Y and Z and rings about them, at the middle of
// what moves.
#include <algorithm>
#include <cmath>
#include <functional>

#include <QHash>
#include <QJsonArray>
#include <QKeySequence>
#include <QStringList>

#include <gp_Ax1.hxx>
#include <gp_Trsf.hxx>

#include "../framework/Appearances.hpp"
#include "CommandFactories.hpp"
#include "CommandSupport.hpp"
#include "MoveFaceText.hpp"

namespace mitcad::cmd {
namespace {

std::function<bool(const CommandState&)> choiceIs(const QString& id, const QString& value) {
  return [id, value](const CommandState& state) { return state.choice(id) == value; };
}

const Choices kMoveTypes = {{QStringLiteral("free"), QStringLiteral("Free Move")},
                            {QStringLiteral("translate"), QStringLiteral("Translate")},
                            {QStringLiteral("rotate"), QStringLiteral("Rotate")},
                            {QStringLiteral("point_to_point"), QStringLiteral("Point to Point")},
                            {QStringLiteral("point_to_position"), QStringLiteral("Point to Position")}};

// The points of a move: vertices, construction and sketch points, and the
// centres of circles and arcs, given by their edges (the model takes a
// circular edge as its centre).
constexpr SelectFilter kMovePointKinds = kPointKinds | SelectKind::Edge;
bool isMovePoint(const SelectionItem& item) { return item.kind != SelectKind::Edge || isCircular(item); }

// What moves: bodies, or occurrences of components (picked in the browser,
// or through a body they place); Move Face's faces (it has no Move Object).
QString movedKey(const CommandState& state) {
  const QString key = state.choice(QStringLiteral("move_type"));
  return key.isEmpty() ? QStringLiteral("faces") : key;
}

// The middle the free move turns about: kept from an edit, else the middle
// of what moves.
std::optional<gp_Pnt> pivotOf(const CommandState& state, const CommandContext& context) {
  const QStringList parts = state.text(QStringLiteral("pivot")).split(QLatin1Char(','));
  if (parts.size() == 3) {
    return gp_Pnt(parts[0].toDouble(), parts[1].toDouble(), parts[2].toDouble());
  }
  return centerOf(state.items(movedKey(state)), context);
}

// The rotation of a free move: about X, then Y, then Z.
gp_Trsf rotationOf(double rx, double ry, double rz) {
  gp_Trsf x;
  x.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(1, 0, 0)), rx);
  gp_Trsf y;
  y.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 1, 0)), ry);
  gp_Trsf z;
  z.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), rz);
  return z * y * x;
}

// The occurrence path ("O1/O4") of an occurrence given by its uid or path.
QString occurrencePath(const QString& occurrence, const CommandContext& context) {
  if (occurrence.contains(QLatin1Char('/'))) {
    return occurrence;
  }
  std::function<QString(const QJsonArray&, const QString&)> find = [&](const QJsonArray& tree, const QString& prefix) {
    for (const QJsonValue& value : tree) {
      const QJsonObject node = value.toObject();
      const QString uid = node.value(QStringLiteral("uid")).toString();
      const QString path = prefix.isEmpty() ? uid : prefix + QLatin1Char('/') + uid;
      if (uid == occurrence) {
        return path;
      }
      const QString found = find(node.value(QStringLiteral("children")).toArray(), path);
      if (!found.isEmpty()) {
        return found;
      }
    }
    return QString();
  };
  try {
    const QString path = find(context.queryObject({{QStringLiteral("query"), QStringLiteral("components")}})
                                  .value(QStringLiteral("occurrences"))
                                  .toArray(),
                              QString());
    return path.isEmpty() ? occurrence : path;
  } catch (const std::exception&) {
    return occurrence;
  }
}

// The names of the occurrences by their paths ("O1/O4" → "Screw:1").
QHash<QString, QString> occurrenceNames(const CommandContext& context) {
  QHash<QString, QString> names;
  std::function<void(const QJsonArray&, const QString&)> walk = [&](const QJsonArray& tree, const QString& prefix) {
    for (const QJsonValue& value : tree) {
      const QJsonObject node = value.toObject();
      const QString uid = node.value(QStringLiteral("uid")).toString();
      const QString path = prefix.isEmpty() ? uid : prefix + QLatin1Char('/') + uid;
      names.insert(path, node.value(QStringLiteral("name")).toString(path));
      walk(node.value(QStringLiteral("children")).toArray(), path);
    }
  };
  try {
    walk(context.queryObject({{QStringLiteral("query"), QStringLiteral("components")}})
             .value(QStringLiteral("occurrences"))
             .toArray(),
         QString());
  } catch (const std::exception&) {
  }
  return names;
}

// Combine's tools by name once one is picked in another occurrence than the
// target: each such tool by its occurrence (two screws of one part,
// "Screw:1, Screw:2"; with its body when the occurrence gives two tools),
// the others by their bodies (mitcad#544). Empty otherwise: the panel
// counts them.
QString combineToolsText(const Selection& tools, const CommandState& state, const CommandContext& context) {
  const Selection& targets = state.items(QStringLiteral("target"));
  const QString here = targets.isEmpty() ? QString() : targets.first().occurrence;
  const auto elsewhere = [&here](const SelectionItem& tool) {
    return !tool.occurrence.isEmpty() && tool.occurrence != here;
  };
  if (std::none_of(tools.begin(), tools.end(), elsewhere)) {
    return QString();
  }
  const QHash<QString, QString> occurrences = occurrenceNames(context);
  const auto bodyName = [&context](const QString& uid) {
    try {
      return context.bodyName(uid);
    } catch (const std::exception&) {
      return uid;
    }
  };
  QStringList names;
  for (const SelectionItem& tool : tools) {
    if (!elsewhere(tool)) {
      names << bodyName(tool.owner);
      continue;
    }
    const QString occurrence = occurrences.value(tool.occurrence, tool.occurrence);
    const bool shared = std::count_if(tools.begin(), tools.end(), [&tool](const SelectionItem& other) {
                          return other.occurrence == tool.occurrence;
                        }) > 1;
    names << (shared ? QStringLiteral("%1 (%2)").arg(bodyName(tool.owner), occurrence) : occurrence);
  }
  return names.join(QStringLiteral(", "));
}

// A free move's handles: an arrow along a model axis or a ring about it.
std::function<std::optional<Manipulator>(const CommandState&, const CommandContext&)>
freeHandle(int axis, bool turn) {
  return [axis, turn](const CommandState& state, const CommandContext& context) -> std::optional<Manipulator> {
    auto pivot = pivotOf(state, context);
    if (!pivot) {
      return std::nullopt;
    }
    const gp_Dir direction(axis == 0 ? 1 : 0, axis == 1 ? 1 : 0, axis == 2 ? 1 : 0);
    if (turn) {
      // About the moved middle.
      pivot->Translate(gp_Vec(state.value(QStringLiteral("tx")), state.value(QStringLiteral("ty")),
                              state.value(QStringLiteral("tz"))));
      return ring(*pivot, direction);
    }
    return arrow(*pivot, direction);
  };
}

std::function<std::optional<Manipulator>(const CommandState&, const CommandContext&)> translateHandle(int axis) {
  return [axis](const CommandState& state, const CommandContext& context) -> std::optional<Manipulator> {
    const auto center = centerOf(state.items(movedKey(state)), context);
    if (!center) {
      return std::nullopt;
    }
    return arrow(*center, gp_Dir(axis == 0 ? 1 : 0, axis == 1 ? 1 : 0, axis == 2 ? 1 : 0));
  };
}

// The model's transform of a move.
QJsonObject transformOf(const CommandState& state, const CommandContext& context, Built& error) {
  const QString type = state.choice(QStringLiteral("type"));
  if (type == QStringLiteral("free")) {
    const auto pivot = pivotOf(state, context);
    if (!pivot) {
      error = Built::failure(QObject::tr("Select what moves."), movedKey(state));
      return {};
    }
    // Turn about the pivot, then move: p -> R (p - c) + c + t.
    gp_Trsf move = rotationOf(state.value(QStringLiteral("rx")), state.value(QStringLiteral("ry")),
                              state.value(QStringLiteral("rz")));
    const gp_XYZ c = pivot->XYZ();
    gp_XYZ rc = c;
    move.Transforms(rc);
    const gp_XYZ t = c - rc +
                     gp_XYZ(state.value(QStringLiteral("tx")), state.value(QStringLiteral("ty")), state.value(QStringLiteral("tz")));
    QJsonArray rows;
    for (int r = 1; r <= 3; ++r) {
      rows.append(QJsonArray{move.Value(r, 1), move.Value(r, 2), move.Value(r, 3), t.Coord(r)});
    }
    return {{QStringLiteral("type"), type}, {QStringLiteral("matrix"), rows}};
  }
  if (type == QStringLiteral("translate")) {
    return {{QStringLiteral("type"), QStringLiteral("translate_xyz")},
            {QStringLiteral("x"), state.expression(QStringLiteral("x"))},
            {QStringLiteral("y"), state.expression(QStringLiteral("y"))},
            {QStringLiteral("z"), state.expression(QStringLiteral("z"))}};
  }
  if (type == QStringLiteral("along")) {
    // Move Face's Translate Along (mitcad#531).
    return {{QStringLiteral("type"), QStringLiteral("translate_along")},
            {QStringLiteral("axis"), state.items(QStringLiteral("along_axis")).first().reference()},
            {QStringLiteral("distance"), state.expression(QStringLiteral("distance"))}};
  }
  if (type == QStringLiteral("rotate")) {
    return {{QStringLiteral("type"), type},
            {QStringLiteral("axis"), state.items(QStringLiteral("axis")).first().reference()},
            {QStringLiteral("angle"), state.expression(QStringLiteral("angle"))}};
  }
  if (type == QStringLiteral("point_to_point")) {
    return {{QStringLiteral("type"), type},
            {QStringLiteral("from"), state.items(QStringLiteral("from")).first().reference()},
            {QStringLiteral("to"), state.items(QStringLiteral("to")).first().reference()}};
  }
  return {{QStringLiteral("type"), type},
          {QStringLiteral("point"), state.items(QStringLiteral("point")).first().reference()},
          {QStringLiteral("x"), state.expression(QStringLiteral("px"))},
          {QStringLiteral("y"), state.expression(QStringLiteral("py"))},
          {QStringLiteral("z"), state.expression(QStringLiteral("pz"))}};
}

bool loadTransform(const QJsonObject& transform, CommandState& state, const QJsonObject& feature,
                   const CommandContext& context) {
  const QString type = str(transform, "type");
  if (type == QStringLiteral("free")) {
    state.setChoice(QStringLiteral("type"), type);
    const QJsonArray rows = transform.value(QStringLiteral("matrix")).toArray();
    const auto at = [&rows](int r, int c) { return rows.at(r).toArray().at(c).toDouble(); };
    // R = Rz Ry Rx.
    const double ry = std::asin(std::clamp(-at(2, 0), -1.0, 1.0));
    const double rx = std::atan2(at(2, 1), at(2, 2));
    const double rz = std::atan2(at(1, 0), at(0, 0));
    // The pivot is the middle of what moves as it is now; t = t' - c + R c.
    const auto pivot = centerOf(state.items(movedKey(state)), context);
    const gp_XYZ c = pivot ? pivot->XYZ() : gp_XYZ(0, 0, 0);
    gp_XYZ rc = c;
    rotationOf(rx, ry, rz).Transforms(rc);
    const gp_XYZ t = gp_XYZ(at(0, 3), at(1, 3), at(2, 3)) - c + rc;
    state.setText(QStringLiteral("pivot"),
                  QStringLiteral("%1,%2,%3").arg(number(c.X()), number(c.Y()), number(c.Z())));
    const auto deg = [](double radians) { return degrees(radians) + QStringLiteral(" deg"); };
    state.setText(QStringLiteral("tx"), lengthText(t.X(), context));
    state.setText(QStringLiteral("ty"), lengthText(t.Y(), context));
    state.setText(QStringLiteral("tz"), lengthText(t.Z(), context));
    state.setText(QStringLiteral("rx"), deg(rx));
    state.setText(QStringLiteral("ry"), deg(ry));
    state.setText(QStringLiteral("rz"), deg(rz));
    return true;
  }
  if (type == QStringLiteral("translate_xyz")) {
    state.setChoice(QStringLiteral("type"), QStringLiteral("translate"));
    for (const char* axis : {"x", "y", "z"}) {
      loadValue(state, QString::fromLatin1(axis), transform.value(QLatin1String(axis)), feature, context);
    }
    return true;
  }
  if (type == QStringLiteral("rotate")) {
    state.setChoice(QStringLiteral("type"), type);
    state.setItems(QStringLiteral("axis"), {itemOf(transform.value(QStringLiteral("axis")), context)});
    loadValue(state, QStringLiteral("angle"), transform.value(QStringLiteral("angle")), feature, context);
    return true;
  }
  if (type == QStringLiteral("point_to_point")) {
    state.setChoice(QStringLiteral("type"), type);
    state.setItems(QStringLiteral("from"), {itemOf(transform.value(QStringLiteral("from")), context)});
    state.setItems(QStringLiteral("to"), {itemOf(transform.value(QStringLiteral("to")), context)});
    return true;
  }
  if (type == QStringLiteral("point_to_position")) {
    state.setChoice(QStringLiteral("type"), type);
    state.setItems(QStringLiteral("point"), {itemOf(transform.value(QStringLiteral("point")), context)});
    loadValue(state, QStringLiteral("px"), transform.value(QStringLiteral("x")), feature, context);
    loadValue(state, QStringLiteral("py"), transform.value(QStringLiteral("y")), feature, context);
    loadValue(state, QStringLiteral("pz"), transform.value(QStringLiteral("z")), feature, context);
    return true;
  }
  if (type == QStringLiteral("translate_along")) {
    // Move Face's Translate Along (Move/Copy has no inputs for it).
    state.setChoice(QStringLiteral("type"), QStringLiteral("along"));
    state.setItems(QStringLiteral("along_axis"), {itemOf(transform.value(QStringLiteral("axis")), context)});
    loadValue(state, QStringLiteral("distance"), transform.value(QStringLiteral("distance")), feature, context);
    return true;
  }
  return false;
}

// Whether the panel can hold the motion; `along`: it has Translate Along
// (Move Face, not Move/Copy).
bool transformEditable(const QJsonObject& transform, const CommandContext& context, bool along = false) {
  const QString type = str(transform, "type");
  if (type == QStringLiteral("free") || type == QStringLiteral("translate_xyz")) {
    return true;
  }
  if (type == QStringLiteral("translate_along")) {
    return along && isShowable(transform.value(QStringLiteral("axis")), context);
  }
  if (type == QStringLiteral("rotate")) {
    return isShowable(transform.value(QStringLiteral("axis")), context);
  }
  if (type == QStringLiteral("point_to_point")) {
    return isShowable(transform.value(QStringLiteral("from")), context) &&
           isShowable(transform.value(QStringLiteral("to")), context);
  }
  if (type == QStringLiteral("point_to_position")) {
    return isShowable(transform.value(QStringLiteral("point")), context);
  }
  return false;
}

// Align's from and to: a point, an axis or a plane.
QJsonObject alignRef(const SelectionItem& item) {
  if (isPointLike(item)) {
    return {{QStringLiteral("type"), QStringLiteral("point")}, {QStringLiteral("point"), item.reference()}};
  }
  if (isPlanar(item)) {
    return {{QStringLiteral("type"), QStringLiteral("plane")}, {QStringLiteral("plane"), item.reference()}};
  }
  return {{QStringLiteral("type"), QStringLiteral("axis")}, {QStringLiteral("axis"), item.reference()}};
}

SelectionItem alignItem(const QJsonObject& ref, const CommandContext& context) {
  return itemOf(ref.value(str(ref, "type")), context);
}

// The motion's inputs, Move/Copy's and Move Face's (mitcad#531): Move Type
// of `types` and each type's inputs; `along` adds Translate Along's
// Direction and Distance (Move Face).
QVector<InputDef> motionInputs(const Choices& types, bool along) {
  const auto type = [](const char* kind) { return choiceIs(QStringLiteral("type"), QString::fromLatin1(kind)); };
  QVector<InputDef> inputs;
  inputs << choiceInput(QStringLiteral("type"), QObject::tr("Move Type"), types, QStringLiteral("free"))
         // Free Move
         << valueInput(QStringLiteral("tx"), QObject::tr("X Distance"), ValueKind::Length, QStringLiteral("0 mm"))
                .withVisible(type("free"))
                .withManipulator(freeHandle(0, false))
         << valueInput(QStringLiteral("ty"), QObject::tr("Y Distance"), ValueKind::Length, QStringLiteral("0 mm"))
                .withVisible(type("free"))
                .withManipulator(freeHandle(1, false))
         << valueInput(QStringLiteral("tz"), QObject::tr("Z Distance"), ValueKind::Length, QStringLiteral("0 mm"))
                .withVisible(type("free"))
                .withManipulator(freeHandle(2, false))
         << valueInput(QStringLiteral("rx"), QObject::tr("X Angle"), ValueKind::Angle, QStringLiteral("0 deg"))
                .withVisible(type("free"))
                .withManipulator(freeHandle(0, true))
         << valueInput(QStringLiteral("ry"), QObject::tr("Y Angle"), ValueKind::Angle, QStringLiteral("0 deg"))
                .withVisible(type("free"))
                .withManipulator(freeHandle(1, true))
         << valueInput(QStringLiteral("rz"), QObject::tr("Z Angle"), ValueKind::Angle, QStringLiteral("0 deg"))
                .withVisible(type("free"))
                .withManipulator(freeHandle(2, true))
         // Translate
         << valueInput(QStringLiteral("x"), QObject::tr("X Distance"), ValueKind::Length, QStringLiteral("0 mm"))
                .withVisible(type("translate"))
                .withManipulator(translateHandle(0))
         << valueInput(QStringLiteral("y"), QObject::tr("Y Distance"), ValueKind::Length, QStringLiteral("0 mm"))
                .withVisible(type("translate"))
                .withManipulator(translateHandle(1))
         << valueInput(QStringLiteral("z"), QObject::tr("Z Distance"), ValueKind::Length, QStringLiteral("0 mm"))
                .withVisible(type("translate"))
                .withManipulator(translateHandle(2));
  if (along) {
    // Translate Along: a distance along an axis, an edge or a face's axis.
    inputs << selectionInput(QStringLiteral("along_axis"), QObject::tr("Direction"), kAxisKinds, 1, 1)
                  .withAccepts(isAxial)
                  .withVisible(type("along"))
           << valueInput(QStringLiteral("distance"), QObject::tr("Distance"), ValueKind::Length, QStringLiteral("1 mm"))
                  .withVisible(type("along"))
                  .withManipulator([](const CommandState& state, const CommandContext& model) -> std::optional<Manipulator> {
                    const Selection& axis = state.items(QStringLiteral("along_axis"));
                    const auto center = centerOf(state.items(movedKey(state)), model);
                    if (axis.isEmpty() || !center) {
                      return std::nullopt;
                    }
                    const auto line = axisOf(axis.first(), model);
                    if (!line) {
                      return std::nullopt;
                    }
                    return arrow(*center, line->second);
                  });
  }
  // Rotate, Point to Point, Point to Position
  inputs
      << selectionInput(QStringLiteral("axis"), QObject::tr("Axis"), kAxisKinds, 1, 1)
             .withAccepts(isAxial)
             .withVisible(type("rotate"))
      << valueInput(QStringLiteral("angle"), QObject::tr("Angle"), ValueKind::Angle, QStringLiteral("90 deg"))
             .withTooltip(QObject::tr("Right-handed about the axis"))
             .withVisible(type("rotate"))
             .withManipulator([](const CommandState& state, const CommandContext& model) -> std::optional<Manipulator> {
               const Selection& axis = state.items(QStringLiteral("axis"));
               const auto center = centerOf(state.items(movedKey(state)), model);
               if (axis.isEmpty() || !center) {
                 return std::nullopt;
               }
               const auto line = axisOf(axis.first(), model);
               if (!line) {
                 return std::nullopt;
               }
               const gp_Vec along(line->second);
               const gp_Pnt foot = line->first.Translated(along * gp_Vec(line->first, *center).Dot(along));
               Manipulator m = ring(foot, line->second);
               if (foot.Distance(*center) > 1e-9) {
                 m.reference = gp_Dir(gp_Vec(foot, *center));
               }
               return m;
             })
      // Point to Point
      << selectionInput(QStringLiteral("from"), QObject::tr("Origin Point"), kMovePointKinds, 1, 1)
             .withAccepts(isMovePoint)
             .withVisible(type("point_to_point"))
      << selectionInput(QStringLiteral("to"), QObject::tr("Target Point"), kMovePointKinds, 1, 1)
             .withAccepts(isMovePoint)
             .withVisible(type("point_to_point"))
      // Point to Position
      << selectionInput(QStringLiteral("point"), QObject::tr("Point"), kMovePointKinds, 1, 1)
             .withAccepts(isMovePoint)
             .withVisible(type("point_to_position"))
      << valueInput(QStringLiteral("px"), QObject::tr("X"), ValueKind::Length, QStringLiteral("0 mm"))
             .withVisible(type("point_to_position"))
      << valueInput(QStringLiteral("py"), QObject::tr("Y"), ValueKind::Length, QStringLiteral("0 mm"))
             .withVisible(type("point_to_position"))
      << valueInput(QStringLiteral("pz"), QObject::tr("Z"), ValueKind::Length, QStringLiteral("0 mm"))
             .withVisible(type("point_to_position"));
  return inputs;
}

// The hidden pivot of a free move (kept from an edit).
InputDef pivotInput() {
  return textInput(QStringLiteral("pivot"), QObject::tr("Pivot")).withVisible([](const CommandState&) { return false; });
}

} // namespace

// ---------------------------------------------------------------------------
// Move/Copy

CommandDef moveCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.move");
  def.name = QObject::tr("Move/Copy");
  def.icon = QStringLiteral("move");
  def.tooltip = QObject::tr("Moves or copies bodies, or moves components");
  def.shortcut = QKeySequence(Qt::Key_M);
  def.group = QStringLiteral("MODIFY");
  def.pinned = false;
  def.keywords = {QStringLiteral("translate"), QStringLiteral("rotate"), QStringLiteral("copy"),
                  QStringLiteral("transform")};
  def.featureType = QStringLiteral("move");
  def.editsAlso = {QStringLiteral("move_occurrence")};
  const auto bodies = choiceIs(QStringLiteral("move_type"), QStringLiteral("bodies"));
  def.inputs = {
      choiceInput(QStringLiteral("move_type"), QObject::tr("Move Object"),
                  {{QStringLiteral("bodies"), QStringLiteral("Bodies")},
                   {QStringLiteral("components"), QStringLiteral("Components")}},
                  QStringLiteral("bodies")),
      selectionInput(QStringLiteral("bodies"), QObject::tr("Objects"), SelectKind::Body, 1, 0)
          .withVisible(bodies),
      selectionInput(QStringLiteral("components"), QObject::tr("Objects"), SelectKind::Component | SelectKind::Body, 1, 0)
          .withTooltip(QObject::tr("Components in the browser, or a body a component's occurrence places"))
          .withAccepts([](const SelectionItem& item) { return !item.occurrence.isEmpty(); })
          .withConvert([](const SelectionItem& item) {
            SelectionItem occurrence{SelectKind::Component, QString(), QString(), QString()};
            occurrence.occurrence = item.occurrence;
            if (item.kind == SelectKind::Component) {
              occurrence.owner = item.owner;
            }
            return occurrence;
          })
          .withVisible(choiceIs(QStringLiteral("move_type"), QStringLiteral("components"))),
  };
  // The motion, as Move Face's (mitcad#531).
  def.inputs << motionInputs(kMoveTypes, false);
  def.inputs << checkInput(QStringLiteral("copy"), QObject::tr("Create Copy"), false)
                    .withTooltip(QObject::tr("Move copies of the bodies; the originals stay"))
                    .withVisible(bodies)
             << pivotInput();
  def.enabled = [&context] { return hasBodies(context); };
  // Components selected before (a pasted copy, P9) move as components.
  def.init = [](CommandState& state, const CommandContext&) {
    if (!state.items(QStringLiteral("components")).isEmpty() && state.items(QStringLiteral("bodies")).isEmpty()) {
      state.setChoice(QStringLiteral("move_type"), QStringLiteral("components"));
    }
  };

  def.build = [](const CommandState& state, const CommandContext& context) {
    Built result;
    const QJsonObject transform = transformOf(state, context, result);
    if (!result.error.isEmpty()) {
      return result;
    }
    if (state.choice(QStringLiteral("move_type")) == QStringLiteral("components")) {
      QJsonArray occurrences;
      for (const SelectionItem& item : state.items(QStringLiteral("components"))) {
        occurrences.append(item.occurrence);
      }
      result.def = {{QStringLiteral("type"), QStringLiteral("move_occurrence")},
                    {QStringLiteral("occurrences"), occurrences},
                    {QStringLiteral("transform"), transform}};
      return result;
    }
    result.def = {{QStringLiteral("type"), QStringLiteral("move")},
                  {QStringLiteral("bodies"), bodyUids(state.items(QStringLiteral("bodies")))},
                  {QStringLiteral("transform"), transform}};
    if (state.checked(QStringLiteral("copy"))) {
      result.def.insert(QStringLiteral("copy"), true);
    }
    return result;
  };

  def.load = [](const QJsonObject& feature, CommandState& state, const CommandContext& context) {
    if (str(feature, "type") == QStringLiteral("move_occurrence")) {
      state.setChoice(QStringLiteral("move_type"), QStringLiteral("components"));
      Selection items;
      for (const QJsonValue& occurrence : feature.value(QStringLiteral("occurrences")).toArray()) {
        SelectionItem item{SelectKind::Component, QString(), QString(), QString()};
        item.occurrence = occurrencePath(occurrence.toString(), context);
        items.append(item);
      }
      state.setItems(QStringLiteral("components"), items);
    } else {
      state.setItems(QStringLiteral("bodies"), bodiesOf(feature.value(QStringLiteral("bodies")).toArray()));
      state.setChecked(QStringLiteral("copy"), feature.value(QStringLiteral("copy")).toBool());
    }
    loadTransform(feature.value(QStringLiteral("transform")).toObject(), state, feature, context);
  };

  def.canEdit = [&context](const QJsonObject& feature) {
    return transformEditable(feature.value(QStringLiteral("transform")).toObject(), context);
  };

  def.describe = [](const CommandState& state, const CommandContext&) {
    return QStringLiteral("%1 %2 %3 (%4)")
        .arg(state.checked(QStringLiteral("copy")) ? QStringLiteral("Copied") : QStringLiteral("Moved"))
        .arg(state.items(movedKey(state)).size())
        .arg(state.choice(QStringLiteral("move_type")), label(kMoveTypes, state.choice(QStringLiteral("type"))));
  };
  return def;
}

// ---------------------------------------------------------------------------
// Move Face (mitcad#531; commands.md, "Move face")

namespace {

const Choices kMoveFaceTypes = {{QStringLiteral("free"), QStringLiteral("Free Move")},
                                {QStringLiteral("translate"), QStringLiteral("Translate")},
                                {QStringLiteral("along"), QStringLiteral("Translate Along")},
                                {QStringLiteral("rotate"), QStringLiteral("Rotate")},
                                {QStringLiteral("point_to_point"), QStringLiteral("Point to Point")},
                                {QStringLiteral("point_to_position"), QStringLiteral("Point to Position")}};

// A definition's faces as items of its body.
Selection movedFacesOf(const QJsonObject& feature, const CommandContext& context) {
  Selection items;
  for (const QJsonValue& face : feature.value(QStringLiteral("faces")).toArray()) {
    items.append(itemOf(QJsonObject{{QStringLiteral("body"), feature.value(QStringLiteral("body"))},
                                    {QStringLiteral("face"), face}},
                        context));
  }
  return items;
}

// The motion as the panel has it, its values as numbers (mm, radians),
// for the summary; empty while it is not complete.
QJsonObject shownMotion(const CommandState& state, const CommandContext& context) {
  Built error;
  QJsonObject transform = transformOf(state, context, error);
  if (!error.error.isEmpty()) {
    return QJsonObject();
  }
  const QString type = str(transform, "type");
  const auto set = [&transform, &state](const char* slot, const char* input) {
    transform.insert(QLatin1String(slot), state.value(QLatin1String(input)));
  };
  if (type == QStringLiteral("translate_xyz")) {
    set("x", "x");
    set("y", "y");
    set("z", "z");
  } else if (type == QStringLiteral("translate_along")) {
    set("distance", "distance");
  } else if (type == QStringLiteral("rotate")) {
    set("angle", "angle");
  } else if (type == QStringLiteral("point_to_position")) {
    set("x", "px");
    set("y", "py");
    set("z", "pz");
  }
  return transform;
}

QString shownSummary(const CommandState& state, const CommandContext& context) {
  const Selection& faces = state.items(QStringLiteral("faces"));
  const QJsonObject motion = shownMotion(state, context);
  if (faces.isEmpty() || motion.isEmpty()) {
    return QString();
  }
  const QString body = context.bodyName(faces.first().owner);
  return moveFaceSummary(QObject::tr("Move Face"), static_cast<int>(faces.size()),
                         body.isEmpty() ? faces.first().owner : body, motion);
}

} // namespace

CommandDef moveFaceCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.move_face");
  def.name = QObject::tr("Move Face");
  def.icon = QStringLiteral("move-face");
  def.tooltip = QObject::tr("Moves faces of a body rigidly; the faces around extend or shorten to follow");
  def.group = QStringLiteral("MODIFY");
  def.keywords = {QStringLiteral("move face"), QStringLiteral("tilt face"), QStringLiteral("rotate face"),
                  QStringLiteral("push"), QStringLiteral("translate"), QStringLiteral("modify")};
  def.featureType = QStringLiteral("move_face");
  def.inputs = {
      selectionInput(QStringLiteral("faces"), QObject::tr("Faces"), SelectKind::Face, 1, 0)
          .withoutMeshes()
          .withTooltip(QObject::tr("Faces of one body; they move together")),
  };
  // The motion, as Move/Copy's, with Translate Along.
  def.inputs << motionInputs(kMoveFaceTypes, true) << pivotInput();
  def.enabled = [&context] { return hasBodies(context); };
  def.build = [](const CommandState& state, const CommandContext& context) {
    Built result;
    const Selection& faces = state.items(QStringLiteral("faces"));
    const QString body = oneBody(faces, result, QStringLiteral("faces"));
    if (body.isEmpty()) {
      return result;
    }
    const QJsonObject transform = transformOf(state, context, result);
    if (!result.error.isEmpty()) {
      return result;
    }
    result.def = {{QStringLiteral("type"), QStringLiteral("move_face")},
                  {QStringLiteral("body"), body},
                  {QStringLiteral("faces"), names(faces, SelectKind::Face)},
                  {QStringLiteral("transform"), transform}};
    return result;
  };
  // An edit (made here or by the .f3d import) opens with its faces first:
  // a free move turns about their middle.
  def.load = [](const QJsonObject& feature, CommandState& state, const CommandContext& context) {
    state.setItems(QStringLiteral("faces"), movedFacesOf(feature, context));
    loadTransform(feature.value(QStringLiteral("transform")).toObject(), state, feature, context);
  };
  def.canEdit = [&context](const QJsonObject& feature) {
    return transformEditable(feature.value(QStringLiteral("transform")).toObject(), context, true);
  };
  // Under the inputs after a preview: what moves and how.
  def.note = [](const CommandState& state, const CommandContext& context) { return shownSummary(state, context); };
  def.describe = [](const CommandState& state, const CommandContext& context) {
    return QStringLiteral("Moved faces: %1").arg(shownSummary(state, context));
  };
  return def;
}

// ---------------------------------------------------------------------------
// Align

CommandDef alignCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.align");
  def.name = QObject::tr("Align");
  def.icon = QStringLiteral("align");
  def.tooltip = QObject::tr("Moves bodies so that a point, axis or plane of them meets another");
  def.group = QStringLiteral("MODIFY");
  def.featureType = QStringLiteral("align");
  const SelectFilter kinds = kPointKinds | kAxisKinds | kPlaneKinds;
  const auto fits = [](const SelectionItem& item) { return isPointLike(item) || isAxial(item) || isPlanar(item); };
  def.inputs = {
      selectionInput(QStringLiteral("bodies"), QObject::tr("Objects"), SelectKind::Body, 1, 0),
      selectionInput(QStringLiteral("from"), QObject::tr("From"), kinds, 1, 1)
          .withTooltip(QObject::tr("A point, an axis or a plane of what moves"))
          .withAccepts(fits),
      selectionInput(QStringLiteral("to"), QObject::tr("To"), kinds, 1, 1)
          .withTooltip(QObject::tr("Where it goes"))
          .withAccepts(fits),
      flipInput(QStringLiteral("flip"), QObject::tr("Flip")),
      valueInput(QStringLiteral("angle"), QObject::tr("Angle"), ValueKind::Angle, QStringLiteral("0 deg"))
          .withTooltip(QObject::tr("Turns about the target's axis or normal")),
  };
  def.enabled = [&context] { return hasBodies(context); };
  def.build = [](const CommandState& state, const CommandContext&) {
    Built result;
    result.def = {{QStringLiteral("type"), QStringLiteral("align")},
                  {QStringLiteral("bodies"), bodyUids(state.items(QStringLiteral("bodies")))},
                  {QStringLiteral("from"), alignRef(state.items(QStringLiteral("from")).first())},
                  {QStringLiteral("to"), alignRef(state.items(QStringLiteral("to")).first())}};
    if (state.checked(QStringLiteral("flip"))) {
      result.def.insert(QStringLiteral("flip"), true);
    }
    if (nonZero(state, QStringLiteral("angle"))) {
      result.def.insert(QStringLiteral("angle"), state.expression(QStringLiteral("angle")));
    }
    return result;
  };
  def.load = [](const QJsonObject& feature, CommandState& state, const CommandContext& context) {
    state.setItems(QStringLiteral("bodies"), bodiesOf(feature.value(QStringLiteral("bodies")).toArray()));
    state.setItems(QStringLiteral("from"), {alignItem(feature.value(QStringLiteral("from")).toObject(), context)});
    state.setItems(QStringLiteral("to"), {alignItem(feature.value(QStringLiteral("to")).toObject(), context)});
    state.setChecked(QStringLiteral("flip"), feature.value(QStringLiteral("flip")).toBool());
    loadValue(state, QStringLiteral("angle"), feature.value(QStringLiteral("angle")), feature, context);
  };
  def.canEdit = [&context](const QJsonObject& feature) {
    return alignItem(feature.value(QStringLiteral("from")).toObject(), context).isValid() &&
           alignItem(feature.value(QStringLiteral("to")).toObject(), context).isValid();
  };
  def.describe = [](const CommandState& state, const CommandContext&) {
    return QStringLiteral("Aligned %1 body(ies)").arg(state.items(QStringLiteral("bodies")).size());
  };
  return def;
}

// ---------------------------------------------------------------------------
// Scale

CommandDef scaleCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.scale");
  def.name = QObject::tr("Scale");
  def.icon = QStringLiteral("scale");
  def.tooltip = QObject::tr("Scales bodies about a point, evenly or along X, Y and Z");
  def.group = QStringLiteral("MODIFY");
  def.keywords = {QStringLiteral("resize"), QStringLiteral("enlarge")};
  def.featureType = QStringLiteral("scale");
  const auto uniform = choiceIs(QStringLiteral("scale_type"), QStringLiteral("uniform"));
  const auto other = [uniform](const CommandState& s) { return !uniform(s); };
  def.inputs = {
      selectionInput(QStringLiteral("bodies"), QObject::tr("Entities"), SelectKind::Body, 1, 0),
      selectionInput(QStringLiteral("point"), QObject::tr("Point"), kPointKinds, 1, 1)
          .withTooltip(QObject::tr("The point that stays; the origin at first")),
      choiceInput(QStringLiteral("scale_type"), QObject::tr("Scale Type"),
                  {{QStringLiteral("uniform"), QStringLiteral("Uniform")},
                   {QStringLiteral("non_uniform"), QStringLiteral("Non Uniform")}},
                  QStringLiteral("uniform")),
      valueInput(QStringLiteral("factor"), QObject::tr("Scale Factor"), ValueKind::Unitless, QStringLiteral("2"))
          .withVisible(uniform),
      valueInput(QStringLiteral("x"), QObject::tr("X Scale"), ValueKind::Unitless, QStringLiteral("1")).withVisible(other),
      valueInput(QStringLiteral("y"), QObject::tr("Y Scale"), ValueKind::Unitless, QStringLiteral("1")).withVisible(other),
      valueInput(QStringLiteral("z"), QObject::tr("Z Scale"), ValueKind::Unitless, QStringLiteral("1")).withVisible(other),
  };
  def.init = [](CommandState& state, const CommandContext&) {
    if (state.items(QStringLiteral("point")).isEmpty()) {
      state.setItems(QStringLiteral("point"), {{SelectKind::Point, QStringLiteral("origin"), QString(), QStringLiteral("point")}});
    }
  };
  def.enabled = [&context] { return hasBodies(context); };
  def.build = [](const CommandState& state, const CommandContext&) {
    Built result;
    QJsonObject scale{{QStringLiteral("type"), state.choice(QStringLiteral("scale_type"))}};
    if (state.choice(QStringLiteral("scale_type")) == QStringLiteral("uniform")) {
      scale.insert(QStringLiteral("factor"), countValue(state, QStringLiteral("factor")));
    } else {
      for (const char* axis : {"x", "y", "z"}) {
        scale.insert(QLatin1String(axis), countValue(state, QString::fromLatin1(axis)));
      }
    }
    result.def = {{QStringLiteral("type"), QStringLiteral("scale")},
                  {QStringLiteral("bodies"), bodyUids(state.items(QStringLiteral("bodies")))},
                  {QStringLiteral("point"), state.items(QStringLiteral("point")).first().reference()},
                  {QStringLiteral("scale"), scale}};
    return result;
  };
  def.load = [](const QJsonObject& feature, CommandState& state, const CommandContext& context) {
    state.setItems(QStringLiteral("bodies"), bodiesOf(feature.value(QStringLiteral("bodies")).toArray()));
    state.setItems(QStringLiteral("point"), {itemOf(feature.value(QStringLiteral("point")), context)});
    const QJsonObject scale = feature.value(QStringLiteral("scale")).toObject();
    state.setChoice(QStringLiteral("scale_type"), str(scale, "type"));
    loadCount(state, QStringLiteral("factor"), scale.value(QStringLiteral("factor")), feature, context);
    for (const char* axis : {"x", "y", "z"}) {
      if (scale.contains(QLatin1String(axis))) {
        loadCount(state, QString::fromLatin1(axis), scale.value(QLatin1String(axis)), feature, context);
      }
    }
  };
  def.canEdit = [&context](const QJsonObject& feature) {
    return isShowable(feature.value(QStringLiteral("point")), context);
  };
  def.describe = [](const CommandState& state, const CommandContext&) {
    return QStringLiteral("Scaled %1 body(ies)").arg(state.items(QStringLiteral("bodies")).size());
  };
  return def;
}

// ---------------------------------------------------------------------------
// Combine

CommandDef combineCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.combine");
  def.name = QObject::tr("Combine");
  def.icon = QStringLiteral("combine");
  def.tooltip = QObject::tr("Joins, cuts or intersects bodies with a target body");
  def.group = QStringLiteral("MODIFY");
  def.pinned = true;
  def.keywords = {QStringLiteral("boolean"), QStringLiteral("union"), QStringLiteral("subtract")};
  def.featureType = QStringLiteral("combine");
  // A pick per occurrence: two screws of one part are two tools, listed by
  // their occurrences (mitcad#544).
  InputDef toolsInput = selectionInput(QStringLiteral("tools"), QObject::tr("Tool Bodies"), SelectKind::Body, 1, 0);
  toolsInput.withoutMeshes();
  toolsInput.itemsText = combineToolsText;
  def.inputs = {
      selectionInput(QStringLiteral("target"), QObject::tr("Target Body"), SelectKind::Body, 1, 1).withoutMeshes(),
      toolsInput,
      choiceInput(QStringLiteral("operation"), QObject::tr("Operation"),
                  {{QStringLiteral("join"), QStringLiteral("Join")},
                   {QStringLiteral("cut"), QStringLiteral("Cut")},
                   {QStringLiteral("intersect"), QStringLiteral("Intersect")}},
                  QStringLiteral("join")),
      checkInput(QStringLiteral("new_component"), QObject::tr("New Component"), false),
      checkInput(QStringLiteral("keep_tools"), QObject::tr("Keep Tools"), false),
  };
  def.enabled = [&context] { return context.queryArray(QStringLiteral("bodies")).size() > 1; };
  def.build = [](const CommandState& state, const CommandContext&) {
    Built result;
    const SelectionItem& targetItem = state.items(QStringLiteral("target")).first();
    const QString target = targetItem.owner;
    // A tool picked in another occurrence than the target (a body of
    // another component) is read where it was picked: a body in that
    // occurrence, once per occurrence (commands.md, "combine", mitcad#104,
    // mitcad#544).
    QJsonArray tools;
    for (const SelectionItem& tool : state.items(QStringLiteral("tools"))) {
      if (tool.owner == target) {
        return Built::failure(QObject::tr("The target cannot be a tool too."), QStringLiteral("tools"));
      }
      if (tool.occurrence != targetItem.occurrence) {
        QJsonObject entry{{QStringLiteral("body"), tool.owner}, {QStringLiteral("occurrence"), tool.occurrence}};
        if (!targetItem.occurrence.isEmpty()) {
          entry.insert(QStringLiteral("context"), targetItem.occurrence);
        }
        tools.append(entry);
      } else {
        tools.append(tool.owner);
      }
    }
    result.def = {{QStringLiteral("type"), QStringLiteral("combine")},
                  {QStringLiteral("target"), target},
                  {QStringLiteral("tools"), tools},
                  {QStringLiteral("operation"), state.choice(QStringLiteral("operation"))}};
    if (state.checked(QStringLiteral("keep_tools"))) {
      result.def.insert(QStringLiteral("keep_tools"), true);
    }
    if (state.checked(QStringLiteral("new_component"))) {
      result.def.insert(QStringLiteral("new_component"), true);
    }
    return result;
  };
  def.load = [](const QJsonObject& feature, CommandState& state, const CommandContext&) {
    // Tools in occurrences where they were picked, the target where they
    // see it; a plain id linked by tool_links as earlier versions wrote it
    // (mitcad#544).
    const QJsonObject links = feature.value(QStringLiteral("tool_links")).toObject();
    Selection target = bodiesOf({feature.value(QStringLiteral("target"))});
    Selection tools;
    for (const QJsonValue& value : feature.value(QStringLiteral("tools")).toArray()) {
      SelectionItem tool{SelectKind::Body, value.toString(), QString(), QString()};
      QJsonObject link;
      bool linked = false;
      if (value.isObject()) {
        const QJsonObject entry = value.toObject();
        tool.owner = str(entry, "body");
        link = {{QStringLiteral("source"), str(entry, "occurrence")},
                {QStringLiteral("target"), str(entry, "context")}};
        linked = true;
      } else if (links.contains(tool.owner)) {
        link = links.value(tool.owner).toObject();
        linked = true;
      }
      if (linked) {
        tool.occurrence = str(link, "source");
        target.first().occurrence = str(link, "target");
      }
      tools.append(tool);
    }
    state.setItems(QStringLiteral("target"), target);
    state.setItems(QStringLiteral("tools"), tools);
    state.setChoice(QStringLiteral("operation"), str(feature, "operation"));
    state.setChecked(QStringLiteral("keep_tools"), feature.value(QStringLiteral("keep_tools")).toBool());
    state.setChecked(QStringLiteral("new_component"), feature.value(QStringLiteral("new_component")).toBool());
  };
  def.describe = [](const CommandState& state, const CommandContext&) {
    return QStringLiteral("Added combine (%1) of %2 tool(s)")
        .arg(state.choice(QStringLiteral("operation")))
        .arg(state.items(QStringLiteral("tools")).size());
  };
  return def;
}

// ---------------------------------------------------------------------------
// Physical Material and Appearance

namespace {

CommandDef attributeCommand(const CommandContext& context, bool material) {
  CommandDef def;
  def.id = material ? QStringLiteral("solid.material") : QStringLiteral("solid.appearance");
  def.name = material ? QObject::tr("Physical Material") : QObject::tr("Appearance");
  def.icon = material ? QStringLiteral("material") : QStringLiteral("appearance");
  def.tooltip = material ? QObject::tr("The material of bodies: their density for masses")
                         : QObject::tr("How bodies look");
  def.group = QStringLiteral("MODIFY");
  def.keywords = material ? QStringList{QStringLiteral("density"), QStringLiteral("mass")}
                          : QStringList{QStringLiteral("colour"), QStringLiteral("color"), QStringLiteral("paint")};
  def.modelCommand = true;
  InputDef value = material ? choiceInput(QStringLiteral("value"), QObject::tr("Material"), physicalMaterials(),
                                          QStringLiteral("steel"))
                            : choiceInput(QStringLiteral("value"), QObject::tr("Appearance"), {},
                                          QStringLiteral("steel_satin"));
  if (!material) {
    // The library's appearances and the document's own (mitcad#46).
    value.withChoices([&context](const CommandState&) {
      Choices choices{{QString(), QObject::tr("Default")}};
      for (const Appearance& appearance : appearancesOf(context.queryArray(QStringLiteral("appearances")))) {
        choices.append({appearance.id, appearance.name});
      }
      return choices;
    });
  }
  // Appearances also go to single faces (mitcad#53), overriding their
  // body's; Default takes a face's own away.
  if (material) {
    def.inputs = {selectionInput(QStringLiteral("bodies"), QObject::tr("Bodies"), SelectKind::Body, 1, 0), value};
  } else {
    def.inputs = {selectionInput(QStringLiteral("bodies"), QObject::tr("Bodies"), SelectKind::Body, 0, 0),
                  selectionInput(QStringLiteral("faces"), QObject::tr("Faces"), SelectKind::Face, 0, 0), value};
  }
  def.enabled = [&context] { return hasBodies(context); };
  def.build = [material, name = def.name](const CommandState& state, const CommandContext&) {
    Built result;
    const QString value = state.choice(QStringLiteral("value"));
    const QJsonValue id =
        value.isEmpty() || (material && value == QStringLiteral("steel")) ? QJsonValue() : QJsonValue(value);
    QJsonArray commands;
    QStringList owners;
    QHash<QString, QJsonArray> faces;
    if (!material) {
      for (const SelectionItem& item : state.items(QStringLiteral("faces"))) {
        if (!owners.contains(item.owner)) {
          owners << item.owner;
        }
        faces[item.owner].append(item.name);
      }
      if (owners.isEmpty() && state.items(QStringLiteral("bodies")).isEmpty()) {
        return Built::failure(QObject::tr("Select bodies or faces"), QStringLiteral("bodies"));
      }
    }
    for (const SelectionItem& item : state.items(QStringLiteral("bodies"))) {
      commands.append(QJsonObject{
          {QStringLiteral("cmd"), material ? QStringLiteral("set_body_material") : QStringLiteral("set_body_appearance")},
          {QStringLiteral("uid"), item.owner},
          {material ? QStringLiteral("material") : QStringLiteral("appearance"), id}});
    }
    for (const QString& owner : std::as_const(owners)) {
      commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("set_face_appearance")},
                                  {QStringLiteral("uid"), owner},
                                  {QStringLiteral("faces"), faces.value(owner)},
                                  {QStringLiteral("appearance"), id}});
    }
    result.def = {{QStringLiteral("commands"), commands}, {QStringLiteral("label"), name}};
    return result;
  };
  def.describe = [material](const CommandState& state, const CommandContext&) {
    const auto faces = material ? 0 : state.items(QStringLiteral("faces")).size();
    return QStringLiteral("%1 %2 for %3 body(ies)%4")
        .arg(material ? QStringLiteral("Material") : QStringLiteral("Appearance"), state.choice(QStringLiteral("value")))
        .arg(state.items(QStringLiteral("bodies")).size())
        .arg(faces > 0 ? QStringLiteral(", %1 face(s)").arg(faces) : QString());
  };
  return def;
}

} // namespace

CommandDef materialCommand(const CommandContext& context) { return attributeCommand(context, true); }
CommandDef appearanceCommand(const CommandContext& context) { return attributeCommand(context, false); }

} // namespace mitcad::cmd
