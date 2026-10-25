// SPDX-License-Identifier: MIT
// Gear Pair and Belt Drive (mitcad#656; commands.md, "Gear Pair" and
// "Belt Drive"): two axes picked (shafts, holes, construction or origin
// axes, sketch lines, circles or arcs) and a ratio typed; the model's
// `gear_pair` / `belt_drive` lists what fits, best first, as rows of the
// panel; the row chosen is previewed (the command applied and taken back,
// CommandDef::sketchCommand) and OK makes both parts in one undo step
// (`add_gear_pair`, `add_belt_drive`). Each part is placed on its axis as
// Place On places it (`at.axis`); the second gear meshes with the first.
#include <algorithm>
#include <cmath>
#include <exception>
#include <limits>
#include <optional>
#include <utility>
#include <vector>

#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QtLogging>

#include "../browser/DocumentHost.hpp"
#include "../framework/CommandRegistry.hpp"
#include "CommandFactories.hpp"
#include "CommandSupport.hpp"
#include "Commands.hpp"

namespace mitcad::cmd {
namespace {

constexpr double kDegree = 3.14159265358979323846 / 180.0;

const QString kFirst = QStringLiteral("first");
const QString kSecond = QStringLiteral("second");
const QString kCandidate = QStringLiteral("candidate");
// The candidates of the last query: {"rows": [...], "error": text,
// "center_distance": mm}.
const QString kCandidates = QStringLiteral("candidates");
// The More that folds the Offsets (and Gear Pair's filters) away
// (mitcad#656), as a single part's placement does.
const QString kMore = QStringLiteral("drive_more");

// What an axis is picked as: a shaft's or a hole's round face or edge, an
// axis, a sketch line, circle or arc.
bool isAxisPick(const SelectionItem& item) {
  switch (item.kind) {
  case SelectKind::Face:
    return item.geometry == QStringLiteral("cylinder");
  case SelectKind::Edge:
    return isCircular(item);
  case SelectKind::Axis:
    return true;
  case SelectKind::SketchCurve:
    return isLine(item) || isCircular(item);
  default:
    return false;
  }
}

// The part's Offset along its axis (mitcad#656: the two parts lined up).
QString offsetKey(const QString& key) { return key + QStringLiteral("_offset"); }

// The `at` of a picked axis, with the end its click was nearer and the
// part's Offset along it (`numbers`: as a number, for queries while it is
// typed).
QJsonObject axisAt(const CommandState& state, const QString& key, bool numbers = false) {
  const Selection& items = state.items(key);
  if (items.isEmpty()) {
    return QJsonObject();
  }
  QJsonObject at{{QStringLiteral("axis"), items.first().reference()}};
  const QJsonValue nearPick = state.object(key + QStringLiteral("_pick")).value(QStringLiteral("near"));
  if (items.first().kind == SelectKind::Face && nearPick.isArray()) {
    at.insert(QStringLiteral("near"), nearPick);
  }
  if (nonZero(state, offsetKey(key))) {
    at.insert(QStringLiteral("offset"), numbers ? QJsonValue(state.value(offsetKey(key)))
                                                : QJsonValue(state.expression(offsetKey(key))));
  }
  return at;
}

// A part's Offset along its axis: an arrow with its value box from where
// the part would start without it.
InputDef offsetInput(const QString& key, const QString& label,
                     std::function<void(CommandState&, const CommandContext&)> find) {
  return valueInput(offsetKey(key), label, ValueKind::Length, QStringLiteral("0 mm"))
      .withOnChange(std::move(find))
      .withManipulator([key](const CommandState& state, const CommandContext& context) -> std::optional<Manipulator> {
        QJsonObject at = axisAt(state, key, true);
        if (at.isEmpty()) {
          return std::nullopt;
        }
        at.remove(QStringLiteral("offset"));
        QJsonObject frame;
        try {
          frame = context.queryObject({{QStringLiteral("query"), QStringLiteral("place_frame")}, {QStringLiteral("at"), at}});
        } catch (...) {
          return std::nullopt;
        }
        const QJsonArray origin = frame.value(QStringLiteral("origin")).toArray();
        const QJsonArray axis = frame.value(QStringLiteral("axis")).toArray();
        if (origin.size() != 3 || axis.size() != 3) {
          return std::nullopt;
        }
        const gp_Vec direction(axis[0].toDouble(), axis[1].toDouble(), axis[2].toDouble());
        if (direction.Magnitude() < 1e-12) {
          return std::nullopt;
        }
        Manipulator m = arrow(gp_Pnt(origin[0].toDouble(), origin[1].toDouble(), origin[2].toDouble()), gp_Dir(direction));
        m.valueBox = true;
        m.boxAtZero = false;
        return m;
      })
      .withHandleWhenHidden()
      .withVisible([key](const CommandState& state) {
        return !state.items(key).isEmpty() && (state.checked(kMore) || nonZero(state, offsetKey(key)));
      });
}

bool onBody(const CommandState& state, const QString& key) {
  const Selection& items = state.items(key);
  return !items.isEmpty() && (items.first().kind == SelectKind::Face || items.first().kind == SelectKind::Edge);
}

InputDef axisInput(const QString& key, const QString& label, const QString& tip) {
  return selectionInput(key, label, SelectKind::Face | SelectKind::Edge | SelectKind::Axis | SelectKind::SketchCurve, 1, 1)
      .withTooltip(tip)
      .withAccepts(isAxisPick)
      .withPlaces()
      .withOnPick([key](const SelectionItem& item, CommandState& state, const CommandContext&) {
        QJsonObject pick;
        if (item.at) {
          pick.insert(QStringLiteral("near"), QJsonArray{(*item.at)[0], (*item.at)[1], (*item.at)[2]});
        }
        state.setObject(key + QStringLiteral("_pick"), pick);
      },
                  false);
}

QJsonArray rowsOf(const CommandState& state) {
  return state.object(kCandidates).value(QStringLiteral("rows")).toArray();
}

// The candidate chosen; the best one while none is.
QJsonObject chosen(const CommandState& state) {
  const QJsonArray rows = rowsOf(state);
  bool ok = false;
  const int index = state.choice(kCandidate).toInt(&ok);
  if (ok && index >= 0 && index < rows.size()) {
    return rows.at(index).toObject();
  }
  return rows.isEmpty() ? QJsonObject() : rows.first().toObject();
}

// The ratio as typed, "1 : 2" (the first part's teeth to the second's,
// mitcad#656) or the second number alone ("2"): z2 / z1; NaN when it does
// not read as one.
double ratioOf(const CommandState& state) {
  const QStringList parts = state.text(QStringLiteral("ratio")).split(QLatin1Char(':'));
  const auto number = [](QString text, bool& ok) {
    return text.trimmed().replace(QLatin1Char(','), QLatin1Char('.')).toDouble(&ok);
  };
  bool firstOk = true;
  bool secondOk = false;
  const double first = parts.size() == 2 ? number(parts.at(0), firstOk) : 1.0;
  const double second = parts.size() <= 2 ? number(parts.last(), secondOk) : 0.0;
  if (!firstOk || !secondOk || !(first > 0.0) || !(second > 0.0)) {
    return std::numeric_limits<double>::quiet_NaN();
  }
  return second / first;
}

// Asks the model for the candidates (`query` with the two axes and the
// ratio) and keeps them; the best one chosen.
void findCandidates(CommandState& state, const CommandContext& context, const QString& command,
                    QJsonObject query) {
  QJsonObject kept;
  const QJsonObject first = axisAt(state, kFirst, true);
  const QJsonObject second = axisAt(state, kSecond, true);
  const double ratio = ratioOf(state);
  if (first.isEmpty() || second.isEmpty() || !std::isfinite(ratio) || ratio <= 0.0) {
    state.setObject(kCandidates, kept);
    state.setChoice(kCandidate, QString());
    return;
  }
  query.insert(QStringLiteral("first"), first);
  query.insert(QStringLiteral("second"), second);
  query.insert(QStringLiteral("ratio"), ratio);
  try {
    const QJsonObject answer = context.queryObject(query);
    // The model's warnings (the parts not lined up along their axes).
    QStringList warnings;
    if (answer.value(QStringLiteral("warning")).isString()) {
      warnings << answer.value(QStringLiteral("warning")).toString();
    }
    for (const QJsonValue& warning : answer.value(QStringLiteral("warnings")).toArray()) {
      warnings << warning.toString();
    }
    kept = {{QStringLiteral("rows"), answer.value(QStringLiteral("candidates"))},
            {QStringLiteral("center_distance"), answer.value(QStringLiteral("center_distance"))},
            {QStringLiteral("warning"), warnings.join(QStringLiteral("; "))}};
    if (!warnings.isEmpty()) {
      qDebug().noquote() << QStringLiteral("%1 warning: %2").arg(command, warnings.join(QStringLiteral("; ")));
    }
    qDebug().noquote() << QStringLiteral("%1 candidates: %2, centre distance %3")
                              .arg(command)
                              .arg(answer.value(QStringLiteral("candidates")).toArray().size())
                              .arg(lengthText(answer.value(QStringLiteral("center_distance")).toDouble(), context));
  } catch (const std::exception& e) {
    kept = {{QStringLiteral("error"), QString::fromUtf8(e.what())}};
    qDebug().noquote() << QStringLiteral("%1 candidates: none: %2").arg(command, QString::fromUtf8(e.what()));
  }
  state.setObject(kCandidates, kept);
  state.setChoice(kCandidate, rowsOf(state).isEmpty() ? QString() : QStringLiteral("0"));
}

// The failure while nothing is chosen: the axes to pick, or why nothing
// fits.
Built nothingChosen(const CommandState& state) {
  if (state.items(kFirst).isEmpty()) {
    return Built::failure(QObject::tr("Pick the first axis"), kFirst);
  }
  if (state.items(kSecond).isEmpty()) {
    return Built::failure(QObject::tr("Pick the second axis"), kSecond);
  }
  if (!std::isfinite(ratioOf(state))) {
    return Built::failure(QObject::tr("Type the ratio as 1 : 2"), QStringLiteral("ratio"));
  }
  const QString error = state.object(kCandidates).value(QStringLiteral("error")).toString();
  return Built::failure(error.isEmpty() ? QObject::tr("Nothing fits this ratio between these axes") : error,
                        error.isEmpty() ? QStringLiteral("ratio") : kSecond);
}

// A part's definition from a candidate's fields, on its axis.
QJsonObject partOf(const QJsonObject& fields, const CommandState& state, const QString& key) {
  QJsonObject part = fields;
  part.insert(QStringLiteral("at"), axisAt(state, key));
  return part;
}

// A candidate's ratio deviation, empty where it is the ratio asked.
QString deviationText(const QJsonObject& candidate) {
  const double fraction = candidate.value(QStringLiteral("ratio_deviation")).toDouble();
  if (std::abs(fraction) < 5e-5) {
    return QString();
  }
  return QStringLiteral("%1%2 %").arg(fraction > 0.0 ? QStringLiteral("+") : QString(),
                                      QString::number(fraction * 100.0, 'f', 2));
}

// Whether a gear of a candidate is undercut (`undercut`: one flag per gear).
bool undercut(const QJsonObject& candidate) {
  const QJsonValue flags = candidate.value(QStringLiteral("undercut"));
  if (flags.isBool()) {
    return flags.toBool();
  }
  for (const QJsonValue& flag : flags.toArray()) {
    if (flag.toBool()) {
      return true;
    }
  }
  return false;
}

// A candidate's profile shifts when either is not 0: "+0.10 / -0.10".
QString shiftText(const QJsonObject& candidate) {
  const QJsonArray shift = candidate.value(QStringLiteral("profile_shift")).toArray();
  if (shift.size() != 2 || (shift.at(0).toDouble() == 0.0 && shift.at(1).toDouble() == 0.0)) {
    return QString();
  }
  const auto signedNumber = [](double value) {
    return (value >= 0.0 ? QStringLiteral("+") : QString(QChar(0x2212))) + QString::number(std::abs(value), 'f', 2);
  };
  return QStringLiteral("%1 / %2").arg(signedNumber(shift.at(0).toDouble()), signedNumber(shift.at(1).toDouble()));
}

// How a gear pair fits its axes, in one short cell (mitcad#656): exact,
// shifted, or its helix angle; the shifts themselves are in the note.
QString fitText(const QJsonObject& candidate) {
  const QString fit = candidate.value(QStringLiteral("fit")).toString();
  if (fit == QStringLiteral("helix")) {
    return QObject::tr("helix %1%2").arg(QString::number(candidate.value(QStringLiteral("helix_angle")).toDouble() / kDegree, 'f', 0), QString(QChar(0x00B0)));
  }
  if (fit == QStringLiteral("profile_shift")) {
    return QObject::tr("shifted");
  }
  // Shifted only against undercut (the model's `corrected`): still exact.
  return candidate.value(QStringLiteral("corrected")).toBool() ? QObject::tr("exact, corrected") : QObject::tr("exact");
}

// The teeth of a candidate, "20:40", led by a warning sign that the panel
// shows as an icon when a gear is undercut (mitcad#656).
QString teethText(const QJsonObject& candidate) {
  const QJsonArray teeth = candidate.value(QStringLiteral("teeth")).toArray();
  const QString text = QStringLiteral("%1:%2").arg(number(teeth.at(0).toDouble()), number(teeth.at(1).toDouble()));
  return undercut(candidate) ? QString(QChar(0x26A0)) + QLatin1Char(' ') + text : text;
}

// A tension travel short, with its unit and sign, to 0.1 mm: "+0.5 mm".
QString travelCell(double millimetres, const CommandContext& context) {
  return travelText(std::round(millimetres * 10.0) / 10.0, context);
}

} // namespace

// ---------------------------------------------------------------------------
// Gear Pair

CommandDef gearPairCommand(const CommandContext& context, DocumentHost& host) {
  CommandDef def;
  def.id = QStringLiteral("solid.gear_pair");
  def.name = QObject::tr("Gear Pair");
  def.icon = QStringLiteral("gear");
  def.tooltip = QObject::tr("Two gears that mesh between two axes at a ratio");
  def.group = QStringLiteral("CREATE");
  def.section = QStringLiteral("machine");
  def.subgroup = QStringLiteral("Machine Elements");
  def.keywords = {QStringLiteral("gear"), QStringLiteral("mesh"), QStringLiteral("ratio"), QStringLiteral("transmission"),
                  QStringLiteral("centre distance")};
  def.sketchCommand = true;
  def.previewAsNew = true; // both parts drawn as a preview, framed (mitcad#656)
  const auto find = [](CommandState& state, const CommandContext& model) {
    QJsonObject query{{QStringLiteral("query"), QStringLiteral("gear_pair")}};
    const QString kind = state.choice(QStringLiteral("kind"));
    if (kind != QStringLiteral("any")) {
      query.insert(QStringLiteral("kind"), kind);
    }
    query.insert(QStringLiteral("pressure_angle"), state.choice(QStringLiteral("pressure_angle")).toDouble() * kDegree);
    if (state.choice(QStringLiteral("modules")) == QStringLiteral("range")) {
      const double low = state.value(QStringLiteral("module_min"));
      const double high = state.value(QStringLiteral("module_max"));
      if (std::isfinite(low) && std::isfinite(high)) {
        query.insert(QStringLiteral("modules"), QJsonArray{std::min(low, high), std::max(low, high)});
      }
    }
    findCandidates(state, model, QObject::tr("Gear Pair"), query);
  };
  const auto ranged = [](const CommandState& state) { return state.choice(QStringLiteral("modules")) == QStringLiteral("range"); };
  // Shown with More, or while not as it starts.
  const auto folded = [](const CommandState& state) {
    return state.checked(kMore) || state.choice(QStringLiteral("kind")) != QStringLiteral("any") ||
           state.choice(QStringLiteral("pressure_angle")) != QStringLiteral("20") ||
           state.choice(QStringLiteral("modules")) != QStringLiteral("standard");
  };
  def.inputs = {
      axisInput(kFirst, QObject::tr("First Axis"), QObject::tr("The first gear's shaft or axis")).withOnChange(find),
      axisInput(kSecond, QObject::tr("Second Axis"), QObject::tr("The second gear's shaft or axis")).withOnChange(find),
      textInput(QStringLiteral("ratio"), QObject::tr("Ratio"), QStringLiteral("1 : 2")).withOnChange(find),
      choiceInput(kCandidate, QObject::tr("Gears"), {}, QString())
          .withChoiceColumns(
              {QObject::tr("Module"), QObject::tr("Teeth"), QObject::tr("Ratio"), QObject::tr("Centre distance")},
              // The distance itself in its heading (mitcad#656).
              [&context](const CommandState& state) {
                const QJsonValue distance = state.object(kCandidates).value(QStringLiteral("center_distance"));
                return QStringList{QObject::tr("Module"), QObject::tr("Teeth"), QObject::tr("Ratio"),
                                   distance.isDouble()
                                       ? QObject::tr("Centre distance %1").arg(shortLength(distance.toDouble(), context))
                                       : QObject::tr("Centre distance")};
              })
          .withChoices([&context](const CommandState& state) {
            Choices rows;
            const QJsonArray candidates = rowsOf(state);
            for (int i = 0; i < candidates.size(); ++i) {
              const QJsonObject c = candidates.at(i).toObject();
              rows.append({QString::number(i),
                           QStringList{lengthText(c.value(QStringLiteral("module")).toDouble(), context), teethText(c),
                                       deviationText(c), fitText(c)}
                               .join(QLatin1Char('\t'))});
            }
            return rows;
          }),
      // What the list is drawn from, folded under More (mitcad#656: the
      // list and its first rows in sight).
      checkInput(kMore, QObject::tr("More"), false).withDisclosure(),
      offsetInput(kFirst, QObject::tr("First Offset"), find),
      offsetInput(kSecond, QObject::tr("Second Offset"), find),
      choiceInput(QStringLiteral("kind"), QObject::tr("Type"),
                  {{QStringLiteral("any"), QStringLiteral("Spur or Helical")},
                   {QStringLiteral("spur"), QStringLiteral("Spur")},
                   {QStringLiteral("helical"), QStringLiteral("Helical")}},
                  QStringLiteral("any"))
          .withOnChange(find)
          .withVisible(folded),
      choiceInput(QStringLiteral("pressure_angle"), QObject::tr("Pressure Angle"),
                  {{QStringLiteral("20"), QStringLiteral("20") + QChar(0x00B0)},
                   {QStringLiteral("14.5"), QStringLiteral("14.5") + QChar(0x00B0)},
                   {QStringLiteral("25"), QStringLiteral("25") + QChar(0x00B0)}},
                  QStringLiteral("20"))
          .withOnChange(find)
          .withVisible(folded),
      choiceInput(QStringLiteral("modules"), QObject::tr("Modules"),
                  {{QStringLiteral("standard"), QStringLiteral("All Standard")},
                   {QStringLiteral("range"), QStringLiteral("From To")}},
                  QStringLiteral("standard"))
          .withOnChange(find)
          .withVisible(folded),
      valueInput(QStringLiteral("module_min"), QObject::tr("From"), ValueKind::Length, QStringLiteral("1 mm"))
          .withOnChange(find)
          .withVisible([folded, ranged](const CommandState& state) { return folded(state) && ranged(state); }),
      valueInput(QStringLiteral("module_max"), QObject::tr("To"), ValueKind::Length, QStringLiteral("3 mm"))
          .withOnChange(find)
          .withVisible([folded, ranged](const CommandState& state) { return folded(state) && ranged(state); }),
      valueInput(QStringLiteral("face_width"), QObject::tr("Face Width"), ValueKind::Length, QStringLiteral("10 mm")),
      valueInput(QStringLiteral("bore_clearance"), QObject::tr("Bore Clearance"), ValueKind::Length, QStringLiteral("0 mm"))
          .withPrintedFit(QStringLiteral("gear"))
          .withVisible([](const CommandState& state) { return onBody(state, kFirst) || onBody(state, kSecond); }),
      choiceInput(QStringLiteral("operation"), QObject::tr("Operation"),
                  {{QStringLiteral("new_component"), QStringLiteral("New Component")},
                   {QStringLiteral("new_body"), QStringLiteral("New Body")}},
                  QStringLiteral("new_component")),
  };
  def.init = [find](CommandState& state, const CommandContext& model) { find(state, model); };

  def.build = [](const CommandState& state, const CommandContext&) {
    const QJsonObject candidate = chosen(state);
    if (candidate.isEmpty()) {
      return nothingChosen(state);
    }
    QJsonArray parts;
    for (const QString& key : {kFirst, kSecond}) {
      QJsonObject gear = partOf(candidate.value(key).toObject(), state, key);
      gear.insert(QStringLiteral("face_width"), state.expression(QStringLiteral("face_width")));
      if (onBody(state, key) && std::isfinite(state.value(QStringLiteral("bore_clearance")))) {
        gear.insert(QStringLiteral("bore_clearance"), state.expression(QStringLiteral("bore_clearance")));
      }
      gear.insert(QStringLiteral("operation"), state.choice(QStringLiteral("operation")));
      parts.append(gear);
    }
    return Built::of({{QStringLiteral("cmd"), QStringLiteral("add_gear_pair")},
                      {QStringLiteral("first"), parts.at(0)},
                      {QStringLiteral("second"), parts.at(1)}});
  };

  // The pair's values under the list, and the model's warning (the gears
  // not lined up along their axes, an undercut).
  def.note = [](const CommandState& state, const CommandContext&) -> QString {
    const QJsonObject candidate = chosen(state);
    if (candidate.isEmpty()) {
      return QString();
    }
    // The centre distance is in the list's heading (mitcad#656), not here.
    QStringList lines;
    const QString shifts = shiftText(candidate);
    if (!shifts.isEmpty()) {
      lines << QObject::tr("Profile shift %1").arg(shifts);
    }
    if (undercut(candidate)) {
      lines << QObject::tr("A gear is undercut");
    }
    const QString warning = state.object(kCandidates).value(QStringLiteral("warning")).toString();
    if (!warning.isEmpty()) {
      lines << warning;
    }
    return lines.join(QLatin1Char('\n'));
  };

  // After OK: the two gears' joints linked when both have one.
  def.describe = [&host](const CommandState& state, const CommandContext&) {
    const QJsonObject candidate = chosen(state);
    const QJsonArray teeth = candidate.value(QStringLiteral("teeth")).toArray();
    if (state.choice(QStringLiteral("operation")) == QStringLiteral("new_component")) {
      linkLater(host, QString());
    }
    return QStringLiteral("Added gear pair, module %1, %2 and %3 teeth (%4)")
        .arg(number(candidate.value(QStringLiteral("module")).toDouble()), number(teeth.at(0).toDouble()),
             number(teeth.at(1).toDouble()), candidate.value(QStringLiteral("fit")).toString());
  };
  return def;
}

// ---------------------------------------------------------------------------
// Belt Drive

CommandDef beltDriveCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.belt_drive");
  def.name = QObject::tr("Belt Drive");
  def.icon = QStringLiteral("pulley");
  def.tooltip = QObject::tr("Two timing belt pulleys between two axes at a ratio, with the belts that fit");
  def.group = QStringLiteral("CREATE");
  def.section = QStringLiteral("machine");
  def.subgroup = QStringLiteral("Machine Elements");
  def.keywords = {QStringLiteral("pulley"), QStringLiteral("timing belt"), QStringLiteral("belt"),
                  QStringLiteral("ratio"), QStringLiteral("GT2"), QStringLiteral("HTD")};
  def.sketchCommand = true;
  def.previewAsNew = true; // both parts drawn as a preview, framed (mitcad#656)
  const auto find = [](CommandState& state, const CommandContext& model) {
    findCandidates(state, model, QObject::tr("Belt Drive"),
                   {{QStringLiteral("query"), QStringLiteral("belt_drive")},
                    {QStringLiteral("profile"), state.choice(QStringLiteral("profile"))}});
  };
  def.inputs = {
      axisInput(kFirst, QObject::tr("First Axis"), QObject::tr("The first pulley's shaft or axis")).withOnChange(find),
      axisInput(kSecond, QObject::tr("Second Axis"), QObject::tr("The second pulley's shaft or axis")).withOnChange(find),
      textInput(QStringLiteral("ratio"), QObject::tr("Ratio"), QStringLiteral("1 : 2"))
          .withOnChange(find),
      choiceInput(QStringLiteral("profile"), QObject::tr("Belt Profile"),
                  {{QStringLiteral("gt2_2mm"), QStringLiteral("GT2 2 mm")},
                   {QStringLiteral("gt2_3mm"), QStringLiteral("GT2 3 mm")},
                   {QStringLiteral("htd_3m"), QStringLiteral("HTD 3M")},
                   {QStringLiteral("htd_5m"), QStringLiteral("HTD 5M")},
                   {QStringLiteral("t5"), QStringLiteral("T5")},
                   {QStringLiteral("xl"), QStringLiteral("XL")}},
                  QStringLiteral("gt2_2mm"))
          .withOnChange(find),
      choiceInput(kCandidate, QObject::tr("Pulleys"), {}, QString())
          .withChoiceColumns({QObject::tr("Teeth"), QObject::tr("Ratio"), QObject::tr("Belt"), QObject::tr("Travel")},
                             // The axes' distance in the Travel heading, as Gear Pair has it
                             // (mitcad#656): the travel is from there.
                             [&context](const CommandState& state) {
                               const QJsonValue distance =
                                   state.object(kCandidates).value(QStringLiteral("center_distance"));
                               return QStringList{
                                   QObject::tr("Teeth"), QObject::tr("Ratio"), QObject::tr("Belt"),
                                   distance.isDouble()
                                       ? QObject::tr("Travel from %1").arg(shortLength(distance.toDouble(), context))
                                       : QObject::tr("Travel")};
                             })
          .withChoices([&context](const CommandState& state) {
            Choices rows;
            const QJsonArray candidates = rowsOf(state);
            for (int i = 0; i < candidates.size(); ++i) {
              const QJsonObject c = candidates.at(i).toObject();
              const QJsonArray teeth = c.value(QStringLiteral("teeth")).toArray();
              const QJsonObject belt = c.value(QStringLiteral("belts")).toArray().at(0).toObject();
              // The belt short, "GT2 200 mm"; its width in the note.
              const QString beltCell =
                  belt.isEmpty() ? QString()
                                 : beltText(QJsonObject{{QStringLiteral("length"), belt.value(QStringLiteral("length"))}},
                                            state.choice(QStringLiteral("profile")), context);
              rows.append({QString::number(i),
                           QStringList{QStringLiteral("%1:%2").arg(number(teeth.at(0).toDouble()),
                                                                   number(teeth.at(1).toDouble())),
                                       deviationText(c), beltCell,
                                       travelCell(c.value(QStringLiteral("travel")).toDouble(), context)}
                               .join(QLatin1Char('\t'))});
            }
            return rows;
          })
          // What the row does not say (mitcad#656): the verdict and the belt's width.
          .withRowNote([](const CommandState& state, const CommandContext& model) -> QString {
            const QJsonObject belt = chosen(state).value(QStringLiteral("belts")).toArray().at(0).toObject();
            if (belt.isEmpty()) {
              return QString();
            }
            QStringList shown;
            QString verdict = beltFitText(belt.value(QStringLiteral("fit")).toString());
            if (!verdict.isEmpty()) {
              verdict[0] = verdict[0].toUpper();
              const bool fits = belt.value(QStringLiteral("fit")).toString() == QStringLiteral("ok");
              shown << QStringLiteral("<b style=\"color:%1\">%2</b>")
                           .arg(fits ? QStringLiteral("#1a7f37") : QStringLiteral("#b35900"), verdict.toHtmlEscaped());
            }
            if (belt.value(QStringLiteral("width")).isDouble()) {
              shown << QObject::tr("%1 wide").arg(shortLength(belt.value(QStringLiteral("width")).toDouble(), model))
                           .toHtmlEscaped();
            }
            return shown.join(QStringLiteral("<br>"));
          }),
      checkInput(kMore, QObject::tr("More"), false).withDisclosure(),
      offsetInput(kFirst, QObject::tr("First Offset"), find),
      offsetInput(kSecond, QObject::tr("Second Offset"), find),
      valueInput(QStringLiteral("belt_width"), QObject::tr("Belt Width"), ValueKind::Length, QStringLiteral("6 mm")),
      valueInput(QStringLiteral("bore_clearance"), QObject::tr("Bore Clearance"), ValueKind::Length, QStringLiteral("0 mm"))
          .withPrintedFit(QStringLiteral("pulley"))
          .withVisible([](const CommandState& state) { return onBody(state, kFirst) || onBody(state, kSecond); }),
      choiceInput(QStringLiteral("operation"), QObject::tr("Operation"),
                  {{QStringLiteral("new_component"), QStringLiteral("New Component")},
                   {QStringLiteral("new_body"), QStringLiteral("New Body")}},
                  QStringLiteral("new_component")),
  };
  def.init = [find](CommandState& state, const CommandContext& model) { find(state, model); };

  def.build = [](const CommandState& state, const CommandContext&) {
    const QJsonObject candidate = chosen(state);
    if (candidate.isEmpty()) {
      return nothingChosen(state);
    }
    QJsonArray parts;
    for (const QString& key : {kFirst, kSecond}) {
      QJsonObject pulley = partOf(candidate.value(key).toObject(), state, key);
      pulley.insert(QStringLiteral("belt_width"), state.expression(QStringLiteral("belt_width")));
      if (onBody(state, key) && std::isfinite(state.value(QStringLiteral("bore_clearance")))) {
        pulley.insert(QStringLiteral("bore_clearance"), state.expression(QStringLiteral("bore_clearance")));
      }
      pulley.insert(QStringLiteral("operation"), state.choice(QStringLiteral("operation")));
      parts.append(pulley);
    }
    return Built::of({{QStringLiteral("cmd"), QStringLiteral("add_belt_drive")},
                      {QStringLiteral("first"), parts.at(0)},
                      {QStringLiteral("second"), parts.at(1)}});
  };

  // The belt the drive takes, as a value line under the list.
  def.note = [](const CommandState& state, const CommandContext& model) -> QString {
    const QJsonObject candidate = chosen(state);
    if (candidate.isEmpty()) {
      return QString();
    }
    const QJsonObject belt = candidate.value(QStringLiteral("belts")).toArray().at(0).toObject();
    QStringList parts;
    // In Check Belt's words: "Belt length 200 mm (100 teeth), 10 teeth in
    // mesh"; the centre distance is in the list's heading (mitcad#656).
    const double length = candidate.value(QStringLiteral("belt_length")).toDouble();
    QString lengthPart = QObject::tr("Belt length %1").arg(shortLength(length, model));
    const double nearestTeeth = belt.value(QStringLiteral("teeth")).toDouble();
    const double nearestLength = belt.value(QStringLiteral("length")).toDouble();
    if (nearestTeeth > 0.0 && nearestLength > 0.0) {
      // Its teeth by the belt's pitch, the nearest belt's length per tooth.
      lengthPart += QObject::tr(" (%1 teeth)").arg(number(std::round(length * nearestTeeth / nearestLength * 10.0) / 10.0));
    }
    parts << lengthPart;
    if (!belt.isEmpty()) {
      // Under the list (withRowNote), not said twice; logged for the tests.
      qDebug().noquote() << QStringLiteral("Belt Drive belt: %1, travel %2, %3")
                                .arg(beltText(belt, state.choice(QStringLiteral("profile")), model),
                                     travelText(std::round(belt.value(QStringLiteral("travel")).toDouble() * 10.0) / 10.0, model),
                                     beltFitText(belt.value(QStringLiteral("fit")).toString()));
    }
    if (candidate.contains(QStringLiteral("teeth_in_mesh"))) {
      parts << QObject::tr("%1 teeth in mesh").arg(number(candidate.value(QStringLiteral("teeth_in_mesh")).toDouble()));
    }
    // The model's warning (the pulleys not lined up along their axes).
    const QString warning = state.object(kCandidates).value(QStringLiteral("warning")).toString();
    QString text = parts.join(QStringLiteral(", "));
    if (!warning.isEmpty()) {
      text += QLatin1Char('\n');
      text += warning;
    }
    return text;
  };

  // The chosen row's belt pitch line round both pulleys (mitcad#656), as
  // Check Belt draws it.
  def.viewLines = [](const CommandState& state, const CommandContext& model) -> std::vector<std::vector<gp_Pnt>> {
    const QJsonArray diameters = chosen(state).value(QStringLiteral("pitch_diameters")).toArray();
    if (diameters.size() != 2) {
      return {};
    }
    // Where each pulley starts on its axis: its origin and the axis.
    const auto frameAt = [&state, &model](const QString& key) -> std::optional<std::pair<gp_Pnt, gp_Dir>> {
      const QJsonObject at = axisAt(state, key, true);
      if (at.isEmpty()) {
        return std::nullopt;
      }
      try {
        const QJsonObject frame =
            model.queryObject({{QStringLiteral("query"), QStringLiteral("place_frame")}, {QStringLiteral("at"), at}});
        const QJsonArray origin = frame.value(QStringLiteral("origin")).toArray();
        const QJsonArray axis = frame.value(QStringLiteral("axis")).toArray();
        const gp_Vec along(axis.at(0).toDouble(), axis.at(1).toDouble(), axis.at(2).toDouble());
        if (origin.size() != 3 || along.Magnitude() < 1e-12) {
          return std::nullopt;
        }
        return std::make_pair(gp_Pnt(origin.at(0).toDouble(), origin.at(1).toDouble(), origin.at(2).toDouble()),
                              gp_Dir(along));
      } catch (...) {
        return std::nullopt;
      }
    };
    const auto first = frameAt(kFirst);
    const auto second = frameAt(kSecond);
    if (!first || !second) {
      return {};
    }
    std::vector<gp_Pnt> line = beltPitchLine(first->first, first->second, diameters.at(0).toDouble(), second->first,
                                             diameters.at(1).toDouble());
    if (line.empty()) {
      return {};
    }
    return {std::move(line)};
  };

  def.describe = [](const CommandState& state, const CommandContext&) {
    const QJsonObject candidate = chosen(state);
    const QJsonArray teeth = candidate.value(QStringLiteral("teeth")).toArray();
    return QStringLiteral("Added belt drive (%1), %2 and %3 teeth")
        .arg(state.choice(QStringLiteral("profile")), number(teeth.at(0).toDouble()), number(teeth.at(1).toDouble()));
  };
  return def;
}

} // namespace mitcad::cmd
