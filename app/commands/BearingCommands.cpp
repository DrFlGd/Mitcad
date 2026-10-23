// SPDX-License-Identifier: MIT
// Bearing (mitcad#577; commands.md, "Printable bearings"): a ball bearing
// printed in one piece (dumbbell, roller or printed ball elements) or one
// for bought airsoft balls in a snap-in cage, of a standard or custom size,
// on a plane or with its raceway cut into a round face of a body (Groove
// Into). The panel shows the model's `bearing_values`: the pitch diameter,
// the elements, the walls and the least gap, or why the values do not
// print. The sizes come from the model's `bearing_sizes`. A Shaft (a
// shaft's end edge or side face, or a hole's rim or wall; mitcad#578,
// SeatInputs.hpp) places it on the shaft or in the hole instead, Offset
// along it (in a hole: how deep), and picks the size whose bore fits the
// shaft (whose outside diameter fits the hole) within the Fit Clearance;
// another size may be chosen after.
#include <memory>

#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QObject>
#include <QString>
#include <QStringList>

#include "../framework/CommandRegistry.hpp"
#include "CommandFactories.hpp"
#include "CommandSupport.hpp"
#include "Commands.hpp"
#include "SeatInputs.hpp"

namespace mitcad::cmd {
namespace {

const Choices kElements = {{QStringLiteral("dumbbell"), QStringLiteral("Dumbbell")},
                           {QStringLiteral("roller"), QStringLiteral("Roller")},
                           {QStringLiteral("ball"), QStringLiteral("Ball")},
                           {QStringLiteral("airsoft"), QStringLiteral("Airsoft")}};

const Choices kOperationsMade = {{QStringLiteral("new_component"), QStringLiteral("New Component")},
                                 {QStringLiteral("new_body"), QStringLiteral("New Body")}};

const QString kCustom = QStringLiteral("custom");

bool isChoice(const Choices& choices, const QString& value) {
  for (const auto& choice : choices) {
    if (choice.first == value) {
      return true;
    }
  }
  return false;
}

bool grooved(const CommandState& state) { return !state.items(QStringLiteral("groove")).isEmpty(); }
bool custom(const CommandState& state) { return state.choice(QStringLiteral("size")) == kCustom; }
bool flanged(const CommandState& state) { return state.checked(QStringLiteral("flange")); }

// On a shaft or in a hole (mitcad#578): not with Groove Into; no bore
// clearance (the size's bore is the bore).
SeatPanel seatPanel() {
  SeatPanel panel;
  panel.offered = [](const CommandState& state) { return !grooved(state); };
  panel.clearance = false;
  return panel;
}
bool onSeat(const CommandState& state) { return seated(state, seatPanel()); }
bool onPlane(const CommandState& state) { return !grooved(state) && !onSeat(state); }

// The size that fits the picked shaft (by its bore) or hole (by its
// outside diameter) within the Fit Clearance, the nearest; the size stays
// when none does or the size is custom.
void fitSize(CommandState& state, const CommandContext& model) {
  if (!onSeat(state) || custom(state)) {
    return;
  }
  QJsonObject probe;
  addSeat(probe, state, model, seatPanel());
  try {
    const QJsonObject frame = model.queryObject(
        {{QStringLiteral("query"), QStringLiteral("seat_frame")}, {QStringLiteral("seat"), probe.value(QStringLiteral("seat"))}});
    const bool hole = frame.value(QStringLiteral("hole")).toBool();
    QJsonObject query{{QStringLiteral("query"), QStringLiteral("bearing_sizes")},
                      {QStringLiteral("fit"), hole ? QStringLiteral("outside") : QStringLiteral("bore")},
                      {QStringLiteral("diameter"), frame.value(QStringLiteral("diameter"))}};
    if (nonZero(state, QStringLiteral("fit_clearance"))) {
      query.insert(QStringLiteral("clearance"), state.value(QStringLiteral("fit_clearance")));
    }
    const QJsonArray sizes = model.queryObject(query).value(QStringLiteral("sizes")).toArray();
    if (!sizes.isEmpty()) {
      state.setChoice(QStringLiteral("size"), sizes.first().toObject().value(QStringLiteral("designation")).toString());
    }
  } catch (...) {
  }
}

// The standard sizes as the model lists them ("608 (8 x 22 x 7)"), then
// Custom; read once.
const Choices& sizeChoices(const CommandContext& context, const std::shared_ptr<Choices>& cache) {
  if (cache->isEmpty()) {
    QJsonArray sizes;
    try {
      sizes = context.queryObject({{QStringLiteral("query"), QStringLiteral("bearing_sizes")}})
                  .value(QStringLiteral("sizes"))
                  .toArray();
    } catch (...) {
    }
    for (const QJsonValue& value : sizes) {
      const QJsonObject size = value.toObject();
      const QString designation = size.value(QStringLiteral("designation")).toString();
      cache->append({designation, QStringLiteral("%1 (%2 x %3 x %4)")
                                      .arg(designation, number(size.value(QStringLiteral("bore")).toDouble()),
                                           number(size.value(QStringLiteral("outside_diameter")).toDouble()),
                                           number(size.value(QStringLiteral("width")).toDouble()))});
    }
    if (cache->isEmpty()) {
      cache->append({QStringLiteral("608"), QStringLiteral("608 (8 x 22 x 7)")});
    }
    cache->append({kCustom, QObject::tr("Custom")});
  }
  return *cache;
}

// A cylindrical face: the raceway is cut into it.
bool isRound(const SelectionItem& item) {
  return item.kind == SelectKind::Face && item.geometry == QStringLiteral("cylinder");
}

} // namespace

CommandDef bearingCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.bearing");
  def.name = QObject::tr("Bearing");
  def.icon = QStringLiteral("bearing");
  def.tooltip = QObject::tr("A ball bearing printed in one piece, or one for bought airsoft balls");
  def.group = QStringLiteral("CREATE");
  def.section = QStringLiteral("machine");
  def.subgroup = QStringLiteral("Machine Elements");
  def.keywords = {QStringLiteral("printable bearing"), QStringLiteral("print in place"),
                  QStringLiteral("ball bearing"),      QStringLiteral("roller bearing"),
                  QStringLiteral("airsoft"),           QStringLiteral("608"),
                  QStringLiteral("race")};
  def.featureType = QStringLiteral("printable_bearing");
  const auto sizes = std::make_shared<Choices>();
  const auto ungrooved = [](const CommandState& state) { return !grooved(state); };
  def.inputs = {
      choiceInput(QStringLiteral("element"), QObject::tr("Element"), kElements, QStringLiteral("dumbbell"))
          .withTooltip(QObject::tr("Dumbbell: a self-centring waisted roller; Airsoft: "
                                   "bought 6 mm balls in a printed cage")),
      selectionInput(QStringLiteral("groove"), QObject::tr("Groove Into"), SelectKind::Face, 0, 1)
          .withTooltip(QObject::tr("A round face of a body (a bore or a shaft): the raceway is cut into it "
                                   "and the body becomes one race"))
          .withAccepts(isRound)
          .withVisible([](const CommandState& state) { return !onSeat(state); }),
      valueInput(QStringLiteral("offset"), QObject::tr("Offset"), ValueKind::Length, QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("Along the face from its end; 0: the middle of the face"))
          .withVisible(grooved),
      selectionInput(QStringLiteral("plane"), QObject::tr("Plane"), kPlaneKinds, 0, 1)
          .withTooltip(QObject::tr("A plane or planar face; a click places the centre"))
          .withAccepts(isPlanar)
          .withOnPick([](const SelectionItem& item, CommandState& state, const CommandContext& model) {
            placeAt(item, state, model, QStringLiteral("x"), QStringLiteral("y"));
          })
          .withVisible(onPlane),
      valueInput(QStringLiteral("x"), QObject::tr("Center X"), ValueKind::Length, QStringLiteral("0 mm"))
          .withVisible(onPlane),
      valueInput(QStringLiteral("y"), QObject::tr("Center Y"), ValueKind::Length, QStringLiteral("0 mm"))
          .withVisible(onPlane),
      valueInput(QStringLiteral("angle"), QObject::tr("Angle"), ValueKind::Angle, QStringLiteral("0 deg"))
          .withTooltip(QObject::tr("Turns the elements about the axis"))
          .withVisible(ungrooved),
      choiceInput(QStringLiteral("size"), QObject::tr("Size"), {}, QStringLiteral("608"))
          .withChoices([&context, sizes](const CommandState&) { return sizeChoices(context, sizes); })
          .withTooltip(QObject::tr("A standard size: bore x outside diameter x width"))
          .withHeading(QObject::tr("Size")),
      valueInput(QStringLiteral("bore"), QObject::tr("Bore"), ValueKind::Length, QStringLiteral("8 mm"))
          .withVisible(custom),
      valueInput(QStringLiteral("outside_diameter"), QObject::tr("Outside Diameter"), ValueKind::Length,
                 QStringLiteral("22 mm"))
          .withVisible(custom),
      valueInput(QStringLiteral("width"), QObject::tr("Width"), ValueKind::Length, QStringLiteral("7 mm"))
          .withVisible(custom),
      checkInput(QStringLiteral("flange"), QObject::tr("Flange"), false)
          .withTooltip(QObject::tr("On the outer race at the face on the plane (a flanged size has its own)")),
      valueInput(QStringLiteral("flange_diameter"), QObject::tr("Flange Diameter"), ValueKind::Length,
                 QStringLiteral("25 mm"))
          .withVisible(flanged),
      valueInput(QStringLiteral("flange_thickness"), QObject::tr("Flange Thickness"), ValueKind::Length,
                 QStringLiteral("1.5 mm"))
          .withVisible(flanged),
      valueInput(QStringLiteral("radial_clearance"), QObject::tr("Radial Clearance"), ValueKind::Length,
                 QStringLiteral("0.4 mm"))
          .withTooltip(QObject::tr("The gap across the round faces between the elements and the races"))
          .withHeading(QObject::tr("Printing")),
      valueInput(QStringLiteral("axial_clearance"), QObject::tr("Axial Clearance"), ValueKind::Length,
                 QStringLiteral("0.4 mm"))
          .withTooltip(QObject::tr("The gap along the axis at the sloped faces")),
      valueInput(QStringLiteral("element_gap"), QObject::tr("Element Gap"), ValueKind::Length, QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("The least gap between two elements; 0: 0.6 mm, with airsoft balls room for "
                                   "the cage")),
      valueInput(QStringLiteral("element_count"), QObject::tr("Elements"), ValueKind::Unitless, QStringLiteral("0"))
          .withTooltip(QObject::tr("How many elements; 0: the most that fit")),
      valueInput(QStringLiteral("element_diameter"), QObject::tr("Element Diameter"), ValueKind::Length,
                 QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("0: the largest that leaves the races their walls; airsoft balls 5.95 mm")),
      valueInput(QStringLiteral("chamfer"), QObject::tr("Chamfer"), ValueKind::Length, QStringLiteral("0.4 mm"))
          .withTooltip(QObject::tr("At the bore and the outside")),
      valueInput(QStringLiteral("min_wall"), QObject::tr("Smallest Wall"), ValueKind::Length, QStringLiteral("1.2 mm"))
          .withTooltip(QObject::tr("The thinnest wall of a race, an element or the cage")),
      valueInput(QStringLiteral("flank_angle"), QObject::tr("Flank Angle"), ValueKind::Angle, QStringLiteral("45 deg"))
          .withTooltip(QObject::tr("The sloped faces from the axis: their overhang printed with the axis vertical "
                                   "(45 deg at most)")),
      choiceInput(QStringLiteral("operation"), QObject::tr("Operation"), kOperationsMade,
                  QStringLiteral("new_component"))
          .withTooltip(QObject::tr("A component of its own, named after the size, or bodies of the active "
                                   "component"))
          .withVisible(ungrooved),
  };
  // The shaft or hole with its offset and flip (mitcad#578) and the fit
  // clearance before the plane and centre they stand for; a picked shaft
  // also takes the size that fits it.
  {
    QVector<InputDef> seatInputList = seatInputs(seatPanel());
    for (InputDef& input : seatInputList) {
      if (input.id == QStringLiteral("shaft")) {
        input.tooltip = QObject::tr("Pick a shaft edge or face, or a hole rim or wall; the bearing size is chosen to fit");
        const auto picked = input.onChange;
        input.onChange = [picked](CommandState& state, const CommandContext& model) {
          if (picked) {
            picked(state, model);
          }
          fitSize(state, model);
        };
      }
    }
    seatInputList.append(valueInput(QStringLiteral("fit_clearance"), QObject::tr("Fit Clearance"), ValueKind::Length,
                                    QStringLiteral("0.5 mm"))
                             .withTooltip(QObject::tr("Allowed difference between the standard bearing "
                                                      "and the shaft or hole diameter"))
                             .withVisible(onSeat));
    qsizetype at = 0;
    while (at < def.inputs.size() && def.inputs.at(at).id != QStringLiteral("plane")) {
      ++at;
    }
    for (const InputDef& input : seatInputList) {
      def.inputs.insert(at++, input);
    }
  }

  def.init = [](CommandState& state, const CommandContext& model) {
    initSeat(state, model);
    if (state.items(QStringLiteral("plane")).isEmpty() && !grooved(state)) {
      state.setItems(QStringLiteral("plane"), {itemOf(QJsonValue(QStringLiteral("xy")), model)});
    }
  };

  const auto buildBearing = [](const CommandState& state, const CommandContext& model) {
    Built result;
    QJsonObject bearing{{QStringLiteral("type"), QStringLiteral("printable_bearing")},
                        {QStringLiteral("element"), state.choice(QStringLiteral("element"))}};
    if (onSeat(state)) {
      addSeat(bearing, state, model, seatPanel());
      bearing.insert(QStringLiteral("fit_clearance"), state.expression(QStringLiteral("fit_clearance")));
      if (nonZero(state, QStringLiteral("angle"))) {
        bearing.insert(QStringLiteral("angle"), state.expression(QStringLiteral("angle")));
      }
    } else if (grooved(state)) {
      bearing.insert(QStringLiteral("groove"), state.items(QStringLiteral("groove")).first().reference());
      if (nonZero(state, QStringLiteral("offset"))) {
        bearing.insert(QStringLiteral("offset"), state.expression(QStringLiteral("offset")));
      }
    } else {
      const Selection& plane = state.items(QStringLiteral("plane"));
      if (plane.isEmpty()) {
        return Built::failure(QObject::tr("Pick a plane, or a round face to cut the raceway into"),
                              QStringLiteral("plane"));
      }
      bearing.insert(QStringLiteral("plane"), plane.first().reference());
      bearing.insert(QStringLiteral("center"), QJsonArray{state.value(QStringLiteral("x")), state.value(QStringLiteral("y"))});
      if (nonZero(state, QStringLiteral("angle"))) {
        bearing.insert(QStringLiteral("angle"), state.expression(QStringLiteral("angle")));
      }
    }
    if (custom(state)) {
      for (const char* key : {"bore", "outside_diameter", "width"}) {
        bearing.insert(QLatin1String(key), state.expression(QLatin1String(key)));
      }
    } else {
      bearing.insert(QStringLiteral("size"), state.choice(QStringLiteral("size")));
    }
    if (flanged(state)) {
      bearing.insert(QStringLiteral("flange"),
                     QJsonObject{{QStringLiteral("diameter"), state.expression(QStringLiteral("flange_diameter"))},
                                 {QStringLiteral("thickness"), state.expression(QStringLiteral("flange_thickness"))}});
    }
    for (const char* key : {"radial_clearance", "axial_clearance", "chamfer", "min_wall", "flank_angle"}) {
      bearing.insert(QLatin1String(key), state.expression(QLatin1String(key)));
    }
    // Zero leaves the model's choice: the default gap, the most elements,
    // the largest element.
    for (const char* key : {"element_gap", "element_diameter"}) {
      if (nonZero(state, QLatin1String(key))) {
        bearing.insert(QLatin1String(key), state.expression(QLatin1String(key)));
      }
    }
    if (nonZero(state, QStringLiteral("element_count"))) {
      bearing.insert(QStringLiteral("element_count"), countValue(state, QStringLiteral("element_count")));
    }
    // A raceway cut into a body stays in the body's component.
    bearing.insert(QStringLiteral("operation"),
                   grooved(state) ? QStringLiteral("new_body") : state.choice(QStringLiteral("operation")));
    result.def = bearing;
    return result;
  };
  def.build = buildBearing;

  // The derived values of the definition the preview showed, or why it
  // does not print.
  def.note = [buildBearing](const CommandState& state, const CommandContext& model) -> QString {
    Built built = buildBearing(state, model);
    if (!built.error.isEmpty()) {
      return QString();
    }
    built.def.remove(QStringLiteral("type"));
    QJsonObject values;
    try {
      values = model.queryObject({{QStringLiteral("query"), QStringLiteral("bearing_values")},
                                  {QStringLiteral("def"), built.def}});
    } catch (...) {
      return QString();
    }
    const QString error = values.value(QStringLiteral("error")).toString();
    if (!error.isEmpty()) {
      QString text = error;
      text[0] = text[0].toUpper();
      return text;
    }
    const auto length = [&](const char* key) { return lengthText(values.value(QLatin1String(key)).toDouble(), model); };
    QStringList parts;
    parts << QObject::tr("Pitch diameter %1").arg(length("pitch_diameter"));
    parts << QObject::tr("%1 elements of %2")
                 .arg(values.value(QStringLiteral("element_count")).toInt())
                 .arg(length("element_diameter"));
    QStringList walls;
    for (const char* key : {"inner_wall", "outer_wall"}) {
      if (values.value(QLatin1String(key)).isDouble()) {
        walls << length(key);
      }
    }
    if (!walls.isEmpty()) {
      parts << QObject::tr("walls %1").arg(walls.join(QStringLiteral(" and ")));
    }
    parts << QObject::tr("least gap %1").arg(length("min_gap"));
    // On a shaft or in a hole (mitcad#578), and whether a size fits it.
    if (values.contains(QStringLiteral("seat"))) {
      const bool hole = values.value(QStringLiteral("seat")).toObject().value(QStringLiteral("hole")).toBool();
      parts << (hole ? QObject::tr("in a %1 hole") : QObject::tr("on a %1 shaft")).arg(length("shaft_diameter"));
      if (values.value(QStringLiteral("fits")).toArray().isEmpty()) {
        parts << QObject::tr("no standard size fits it");
      }
    }
    for (const QJsonValue& value : values.value(QStringLiteral("purchased")).toArray()) {
      const QJsonObject item = value.toObject();
      parts << QObject::tr("buy %1 x %2")
                   .arg(item.value(QStringLiteral("quantity")).toInt())
                   .arg(item.value(QStringLiteral("designation")).toString());
    }
    return parts.join(QStringLiteral(", "));
  };

  def.load = [](const QJsonObject& feature, CommandState& state, const CommandContext& context) {
    state.setChoice(QStringLiteral("element"), feature.value(QStringLiteral("element")).toString(QStringLiteral("dumbbell")));
    const QJsonValue groove = feature.value(QStringLiteral("groove"));
    if (loadSeat(feature, state, context)) {
      state.setItems(QStringLiteral("groove"), {});
      loadValue(state, QStringLiteral("fit_clearance"), feature.value(QStringLiteral("fit_clearance")), feature, context);
      loadValue(state, QStringLiteral("angle"), feature.value(QStringLiteral("angle")), feature, context);
    } else if (!groove.isUndefined() && !groove.isNull()) {
      state.setItems(QStringLiteral("groove"), {itemOf(groove, context)});
      state.setItems(QStringLiteral("plane"), {});
      loadValue(state, QStringLiteral("offset"), feature.value(QStringLiteral("offset")), feature, context);
    } else {
      state.setItems(QStringLiteral("groove"), {});
      state.setItems(QStringLiteral("plane"), {itemOf(feature.value(QStringLiteral("plane")), context)});
      const QJsonArray center = feature.value(QStringLiteral("center")).toArray();
      state.setText(QStringLiteral("x"), lengthText(center.at(0).toDouble(), context));
      state.setText(QStringLiteral("y"), lengthText(center.at(1).toDouble(), context));
      loadValue(state, QStringLiteral("angle"), feature.value(QStringLiteral("angle")), feature, context);
    }
    const QString size = feature.value(QStringLiteral("size")).toString();
    state.setChoice(QStringLiteral("size"), size.isEmpty() ? kCustom : size);
    for (const char* key : {"bore", "outside_diameter", "width", "radial_clearance", "axial_clearance", "element_gap",
                            "element_diameter", "chamfer", "min_wall", "flank_angle"}) {
      loadValue(state, QLatin1String(key), feature.value(QLatin1String(key)), feature, context);
    }
    loadCount(state, QStringLiteral("element_count"), feature.value(QStringLiteral("element_count")), feature, context);
    const QJsonObject flange = feature.value(QStringLiteral("flange")).toObject();
    state.setChecked(QStringLiteral("flange"), !flange.isEmpty());
    loadValue(state, QStringLiteral("flange_diameter"), flange.value(QStringLiteral("diameter")), feature, context);
    loadValue(state, QStringLiteral("flange_thickness"), flange.value(QStringLiteral("thickness")), feature, context);
    state.setChoice(QStringLiteral("operation"),
                    feature.value(QStringLiteral("operation")).toString(QStringLiteral("new_component")));
  };

  // Only what the panel can show: its elements and operations, a standard
  // size it lists, a plane or a face it shows.
  def.canEdit = [&context, sizes](const QJsonObject& feature) {
    if (!isChoice(kElements, feature.value(QStringLiteral("element")).toString(QStringLiteral("dumbbell")))) {
      return false;
    }
    if (!isChoice(kOperationsMade, feature.value(QStringLiteral("operation")).toString(QStringLiteral("new_component")))) {
      return false;
    }
    const QString size = feature.value(QStringLiteral("size")).toString();
    if (!size.isEmpty() && !isChoice(sizeChoices(context, sizes), size)) {
      return false;
    }
    for (const char* key : {"plane", "groove"}) {
      const QJsonValue ref = feature.value(QLatin1String(key));
      if (!ref.isUndefined() && !ref.isNull() && !isShowable(ref, context)) {
        return false;
      }
    }
    // A size the seat fits is given by the panel (mitcad#578).
    return seatShowable(feature, context) &&
           (feature.value(QStringLiteral("seat")).isUndefined() || !feature.value(QStringLiteral("size")).isUndefined() ||
            !feature.value(QStringLiteral("width")).isUndefined());
  };

  def.describe = [](const CommandState& state, const CommandContext& model) {
    const QString element = label(kElements, state.choice(QStringLiteral("element")));
    if (custom(state)) {
      return QStringLiteral("Added bearing (%1), %2 x %3 x %4")
          .arg(element, lengthText(state.value(QStringLiteral("bore")), model),
               lengthText(state.value(QStringLiteral("outside_diameter")), model),
               lengthText(state.value(QStringLiteral("width")), model));
    }
    QString where;
    if (onSeat(state)) {
      where = QStringLiteral(", on a shaft or in a hole");
    } else if (grooved(state)) {
      where = QStringLiteral(", groove into a face");
    }
    return QStringLiteral("Added bearing (%1), %2%3").arg(element, state.choice(QStringLiteral("size")), where);
  };
  return def;
}

} // namespace mitcad::cmd

namespace mitcad {

void registerBearingCommands(CommandRegistry& registry, const CommandContext& context) {
  registry.add(cmd::bearingCommand(context));
}

} // namespace mitcad
