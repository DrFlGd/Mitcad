// SPDX-License-Identifier: MIT
// Pulley (mitcad#562; commands.md, "Pulleys"): timing belt pulleys (GT2,
// HTD, T5, XL), smooth idlers and toothed idlers, on a plane or at a centre
// distance from another pulley that the belt runs round (`belt_with`). The
// panel shows the derived values (the model's `pulley_values`): the
// diameters, and with `belt_with` the belt length, the nearest of the
// catalogue lengths typed in the panel and the teeth in mesh. A Shaft
// places it on a shaft instead of the plane (mitcad#578, SeatInputs.hpp),
// its bore the shaft's diameter and the bore clearance.
#include <cmath>

#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QObject>
#include <QRegularExpression>
#include <QString>
#include <QStringList>

#include "../framework/CommandRegistry.hpp"
#include "CommandFactories.hpp"
#include "CommandSupport.hpp"
#include "Commands.hpp"
#include "SeatInputs.hpp"

namespace mitcad::cmd {
namespace {

const Choices kKinds = {{QStringLiteral("toothed"), QStringLiteral("Toothed")},
                        {QStringLiteral("idler"), QStringLiteral("Idler")},
                        {QStringLiteral("toothed_idler"), QStringLiteral("Toothed Idler")}};

const Choices kProfiles = {{QStringLiteral("gt2_2mm"), QStringLiteral("GT2 2 mm")},
                           {QStringLiteral("gt2_3mm"), QStringLiteral("GT2 3 mm")},
                           {QStringLiteral("htd_3m"), QStringLiteral("HTD 3M")},
                           {QStringLiteral("htd_5m"), QStringLiteral("HTD 5M")},
                           {QStringLiteral("t5"), QStringLiteral("T5")},
                           {QStringLiteral("xl"), QStringLiteral("XL")}};

const Choices kFlangeSides = {{QStringLiteral("none"), QStringLiteral("None")},
                              {QStringLiteral("one"), QStringLiteral("One")},
                              {QStringLiteral("both"), QStringLiteral("Both")}};

const Choices kBores = {{QStringLiteral("none"), QStringLiteral("None")},
                        {QStringLiteral("plain"), QStringLiteral("Plain")},
                        {QStringLiteral("thread"), QStringLiteral("Thread")}};

bool isChoice(const Choices& choices, const QString& value) {
  for (const auto& choice : choices) {
    if (choice.first == value) {
      return true;
    }
  }
  return false;
}

QString kindOf(const CommandState& state) { return state.choice(QStringLiteral("kind")); }

// Teeth: a pulley or a toothed idler; a smooth idler has a diameter.
bool toothed(const CommandState& state) { return kindOf(state) != QStringLiteral("idler"); }

// Only a pulley has a hub and set screws; idlers run on a bearing.
bool takesHub(const CommandState& state) { return kindOf(state) == QStringLiteral("toothed"); }
bool hubShown(const CommandState& state) { return takesHub(state) && state.checked(QStringLiteral("hub")); }
bool screwsShown(const CommandState& state) { return hubShown(state) && state.checked(QStringLiteral("set_screws")); }

bool belted(const CommandState& state) { return !state.items(QStringLiteral("belt_with")).isEmpty(); }

bool flangesShown(const CommandState& state) { return state.choice(QStringLiteral("flanges")) != QStringLiteral("none"); }

bool plainBore(const CommandState& state) { return state.choice(QStringLiteral("bore")) == QStringLiteral("plain"); }

// On a shaft (mitcad#578): not while running with another pulley.
SeatPanel seatPanel() {
  SeatPanel panel;
  panel.offered = [](const CommandState& state) { return !belted(state); };
  return panel;
}
bool onShaft(const CommandState& state) { return !belted(state) && seated(state); }
// Placed by the plane and centre.
bool onPlane(const CommandState& state) { return !belted(state) && !seated(state); }
// A plain bore of its own diameter; on a shaft any bore but a thread is
// the shaft's.
bool ownPlainBore(const CommandState& state) { return plainBore(state) && !onShaft(state); }
bool shaftBore(const CommandState& state) {
  return onShaft(state) && state.choice(QStringLiteral("bore")) != QStringLiteral("thread");
}

// The type of the feature picked to run the belt with (kept beside the
// inputs): a pulley's, or empty.
QString mateType(const CommandState& state) { return state.object(QStringLiteral("mate")).value(QStringLiteral("type")).toString(); }

// Remembers the feature `uid` for the inputs that follow it; a pulley's
// belt profile becomes this one's, since one belt runs round both.
void setMate(CommandState& state, const QString& uid, const CommandContext& context) {
  // The feature query gives the type beside the definition ("type", "def").
  QJsonObject feature;
  try {
    feature = context.queryObject({{QStringLiteral("query"), QStringLiteral("feature")}, {QStringLiteral("uid"), uid}});
  } catch (...) {
  }
  const QJsonObject def = feature.value(QStringLiteral("def")).toObject();
  const QString type = feature.value(QStringLiteral("type")).toString(def.value(QStringLiteral("type")).toString());
  const QString profile = def.value(QStringLiteral("profile")).toString();
  state.setObject(QStringLiteral("mate"), {{QStringLiteral("uid"), uid},
                                           {QStringLiteral("type"), type},
                                           {QStringLiteral("kind"), def.value(QStringLiteral("kind")).toString()},
                                           {QStringLiteral("profile"), profile}});
  if (type == QStringLiteral("pulley") && isChoice(kProfiles, profile)) {
    state.setChoice(QStringLiteral("profile"), profile);
  }
}

// The catalogue belt lengths typed in the panel (in the document's unit,
// separated by commas, semicolons or spaces), in millimetres.
QJsonArray beltLengths(const CommandState& state, const CommandContext& context) {
  static const QRegularExpression separators(QStringLiteral("[,;\\s]+"));
  QJsonArray lengths;
  const QStringList parts = state.text(QStringLiteral("belt_lengths")).split(separators, Qt::SkipEmptyParts);
  for (const QString& part : parts) {
    bool ok = false;
    const double value = part.toDouble(&ok);
    if (ok && std::isfinite(value) && value > 0.0) {
      lengths.append(value * context.unitMillimetres());
    }
  }
  return lengths;
}

// A thread's designation as the model takes it.
QJsonObject threadOf(const CommandState& state, const QString& key) {
  return {{QStringLiteral("designation"), state.text(key).trimmed()}};
}

} // namespace

CommandDef pulleyCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.pulley");
  def.name = QObject::tr("Pulley");
  def.icon = QStringLiteral("pulley");
  def.tooltip = QObject::tr("A timing belt pulley (GT2, HTD, T5, XL), a smooth idler or a toothed idler");
  def.group = QStringLiteral("CREATE");
  def.section = QStringLiteral("machine");
  def.subgroup = QStringLiteral("Machine Elements");
  def.keywords = {QStringLiteral("timing belt"), QStringLiteral("belt"), QStringLiteral("idler"),
                  QStringLiteral("GT2"),         QStringLiteral("HTD"),  QStringLiteral("T5"),
                  QStringLiteral("XL"),          QStringLiteral("sheave")};
  def.featureType = QStringLiteral("pulley");
  const auto unbelted = [](const CommandState& state) { return !belted(state); };
  def.inputs = {
      choiceInput(QStringLiteral("kind"), QObject::tr("Type"), kKinds, QStringLiteral("toothed"))
          .withChoiceIcons({{QStringLiteral("toothed"), QStringLiteral("pulley")},
                            {QStringLiteral("idler"), QStringLiteral("pulley-idler")},
                            {QStringLiteral("toothed_idler"), QStringLiteral("pulley-toothed-idler")}}),
      selectionInput(QStringLiteral("belt_with"), QObject::tr("Belt With"), SelectKind::Feature | SelectKind::Face, 0, 1)
          .withTooltip(QObject::tr("A pulley the belt also runs round: the new one sits at the centre distance from it, in its plane"))
          .withAccepts([](const SelectionItem& item) { return !item.creatingFeature().isEmpty(); })
          .withConvert([](const SelectionItem& item) {
            return SelectionItem{SelectKind::Feature, item.creatingFeature(), QString(), QString()};
          })
          .withOnPick([](const SelectionItem& item, CommandState& state, const CommandContext& model) {
            // The pick before its conversion: a face's owner is its body,
            // the feature is the one that made it.
            const QString feature = item.creatingFeature();
            setMate(state, feature.isEmpty() ? item.owner : feature, model);
          }),
      valueInput(QStringLiteral("center_distance"), QObject::tr("Centre Distance"), ValueKind::Length, QStringLiteral("60 mm"))
          .withTooltip(QObject::tr("From the other pulley's axis to this one's"))
          .withVisible(belted),
      valueInput(QStringLiteral("belt_angle"), QObject::tr("Direction"), ValueKind::Angle, QStringLiteral("0 deg"))
          .withTooltip(QObject::tr("Where this pulley sits around the other one, from its plane's x axis"))
          .withVisible(belted),
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
          .withTooltip(QObject::tr("Turns the pulley about its axis"))
          .withVisible(unbelted),
      choiceInput(QStringLiteral("profile"), QObject::tr("Belt Profile"), kProfiles, QStringLiteral("gt2_2mm"))
          .withHeading(QObject::tr("Teeth"))
          .withVisible(toothed),
      valueInput(QStringLiteral("teeth"), QObject::tr("Teeth"), ValueKind::Unitless, QStringLiteral("20"))
          .withVisible(toothed),
      valueInput(QStringLiteral("diameter"), QObject::tr("Diameter"), ValueKind::Length, QStringLiteral("20 mm"))
          .withTooltip(QObject::tr("Where the belt's back runs"))
          .withVisible([](const CommandState& s) { return !toothed(s); }),
      valueInput(QStringLiteral("belt_width"), QObject::tr("Belt Width"), ValueKind::Length, QStringLiteral("6 mm")),
      valueInput(QStringLiteral("clearance"), QObject::tr("Clearance"), ValueKind::Length, QStringLiteral("1 mm"))
          .withTooltip(QObject::tr("Added to the belt width: the width between the flanges")),
      valueInput(QStringLiteral("pitch_correction"), QObject::tr("Pitch Correction"), ValueKind::Length, QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("Added to the pitch diameter (negative: a looser fit of the belt)"))
          .withVisible(toothed),
      choiceInput(QStringLiteral("flanges"), QObject::tr("Flanges"), kFlangeSides, QStringLiteral("both"))
          .withTooltip(QObject::tr("One: on the side away from the hub"))
          .withHeading(QObject::tr("Flanges")),
      valueInput(QStringLiteral("flange_diameter"), QObject::tr("Flange Diameter"), ValueKind::Length, QStringLiteral("18 mm"))
          .withVisible(flangesShown),
      valueInput(QStringLiteral("flange_thickness"), QObject::tr("Flange Thickness"), ValueKind::Length, QStringLiteral("1 mm"))
          .withVisible(flangesShown),
      choiceInput(QStringLiteral("bore"), QObject::tr("Bore"), kBores, QStringLiteral("plain"))
          .withHeading(QObject::tr("Bore and Hub")),
      valueInput(QStringLiteral("bore_diameter"), QObject::tr("Bore Diameter"), ValueKind::Length, QStringLiteral("5 mm"))
          .withVisible(ownPlainBore),
      textInput(QStringLiteral("thread"), QObject::tr("Thread Size"), QStringLiteral("M5x0.8"))
          .withTooltip(QObject::tr("An ISO metric size: the bore is its tap drill"))
          .withVisible([](const CommandState& s) { return s.choice(QStringLiteral("bore")) == QStringLiteral("thread"); }),
      checkInput(QStringLiteral("keyway"), QObject::tr("Keyway"), false)
          .withTooltip(QObject::tr("DIN 6885 for the bore's diameter"))
          .withVisible([](const CommandState& s) { return plainBore(s) || shaftBore(s); }),
      checkInput(QStringLiteral("hub"), QObject::tr("Hub"), true).withVisible(takesHub),
      valueInput(QStringLiteral("hub_diameter"), QObject::tr("Hub Diameter"), ValueKind::Length, QStringLiteral("16 mm"))
          .withVisible(hubShown),
      valueInput(QStringLiteral("hub_length"), QObject::tr("Hub Length"), ValueKind::Length, QStringLiteral("7 mm"))
          .withVisible(hubShown),
      checkInput(QStringLiteral("set_screws"), QObject::tr("Set Screws"), true)
          .withTooltip(QObject::tr("Tapped holes through the hub to the bore"))
          .withVisible(hubShown),
      valueInput(QStringLiteral("set_screw_count"), QObject::tr("Set Screw Count"), ValueKind::Unitless, QStringLiteral("2"))
          .withTooltip(QObject::tr("Two: 90 deg apart"))
          .withVisible(screwsShown),
      textInput(QStringLiteral("set_screw_thread"), QObject::tr("Set Screw Size"), QStringLiteral("M3x0.5"))
          .withTooltip(QObject::tr("An ISO metric size: the holes are its tap drill"))
          .withVisible(screwsShown),
      valueInput(QStringLiteral("set_screw_position"), QObject::tr("Set Screw Position"), ValueKind::Length, QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("From the hub's outer end; 0: the middle of the hub"))
          .withVisible(screwsShown),
      textInput(QStringLiteral("belt_lengths"), QObject::tr("Catalogue Lengths"), QString())
          .withTooltip(QObject::tr("Belt lengths to choose from, separated by commas: the panel shows the nearest"))
          .withHeading(QObject::tr("Belt"))
          .withVisible(belted),
  };
  // The shaft and its offset, flip and bore clearance (mitcad#578) before
  // the plane and centre they stand for.
  const SeatPanel seat = seatPanel();
  {
    const QVector<InputDef> seatInputList = seatInputs(seat);
    qsizetype at = 0;
    while (at < def.inputs.size() && def.inputs.at(at).id != QStringLiteral("plane")) {
      ++at;
    }
    for (InputDef input : seatInputList) {
      if (input.id == QStringLiteral("bore_clearance")) {
        input.withPrintedFit(QStringLiteral("pulley"));
      }
      def.inputs.insert(at++, input);
    }
  }
  def.inputs += operationInputs();

  def.init = [](CommandState& state, const CommandContext& model) {
    initSeat(state, model);
    // A face selected before the command runs the belt with its pulley;
    // another feature's face is no pulley.
    Selection& belt = state.items(QStringLiteral("belt_with"));
    if (!belt.isEmpty()) {
      setMate(state, belt.first().owner, model);
      if (mateType(state) != QStringLiteral("pulley")) {
        belt.clear();
        state.setObject(QStringLiteral("mate"), QJsonObject());
      }
    }
    if (state.items(QStringLiteral("plane")).isEmpty()) {
      state.setItems(QStringLiteral("plane"), {itemOf(QJsonValue(QStringLiteral("xy")), model)});
    }
  };

  const auto buildPulley = [seat](const CommandState& state, const CommandContext& model) {
    Built result;
    const QString kind = kindOf(state);
    QJsonObject pulley{{QStringLiteral("type"), QStringLiteral("pulley")}, {QStringLiteral("kind"), kind}};
    if (belted(state)) {
      if (mateType(state) != QStringLiteral("pulley")) {
        return Built::failure(QObject::tr("Pick a pulley for the belt to run round"), QStringLiteral("belt_with"));
      }
      pulley.insert(QStringLiteral("belt_with"),
                    QJsonObject{{QStringLiteral("pulley"), state.items(QStringLiteral("belt_with")).first().owner},
                                {QStringLiteral("center_distance"), state.expression(QStringLiteral("center_distance"))},
                                {QStringLiteral("angle"), state.expression(QStringLiteral("belt_angle"))}});
    } else if (onShaft(state)) {
      addSeat(pulley, state, model, seat);
      if (nonZero(state, QStringLiteral("angle"))) {
        pulley.insert(QStringLiteral("angle"), state.expression(QStringLiteral("angle")));
      }
    } else {
      const Selection& plane = state.items(QStringLiteral("plane"));
      if (plane.isEmpty()) {
        return Built::failure(QObject::tr("Pick a plane, or a pulley for the belt to run round"), QStringLiteral("plane"));
      }
      pulley.insert(QStringLiteral("plane"), plane.first().reference());
      pulley.insert(QStringLiteral("center"), QJsonArray{state.value(QStringLiteral("x")), state.value(QStringLiteral("y"))});
      if (nonZero(state, QStringLiteral("angle"))) {
        pulley.insert(QStringLiteral("angle"), state.expression(QStringLiteral("angle")));
      }
    }
    if (toothed(state)) {
      pulley.insert(QStringLiteral("profile"), state.choice(QStringLiteral("profile")));
      pulley.insert(QStringLiteral("teeth"), countValue(state, QStringLiteral("teeth")));
      if (nonZero(state, QStringLiteral("pitch_correction"))) {
        pulley.insert(QStringLiteral("pitch_correction"), state.expression(QStringLiteral("pitch_correction")));
      }
    } else {
      pulley.insert(QStringLiteral("diameter"), state.expression(QStringLiteral("diameter")));
    }
    pulley.insert(QStringLiteral("belt_width"), state.expression(QStringLiteral("belt_width")));
    pulley.insert(QStringLiteral("clearance"), state.expression(QStringLiteral("clearance")));
    if (flangesShown(state)) {
      pulley.insert(QStringLiteral("flanges"),
                    QJsonObject{{QStringLiteral("sides"), state.choice(QStringLiteral("flanges"))},
                                {QStringLiteral("diameter"), state.expression(QStringLiteral("flange_diameter"))},
                                {QStringLiteral("thickness"), state.expression(QStringLiteral("flange_thickness"))}});
    }
    if (hubShown(state)) {
      pulley.insert(QStringLiteral("hub"),
                    QJsonObject{{QStringLiteral("diameter"), state.expression(QStringLiteral("hub_diameter"))},
                                {QStringLiteral("length"), state.expression(QStringLiteral("hub_length"))}});
    }
    const QString bore = state.choice(QStringLiteral("bore"));
    if (shaftBore(state)) {
      // The shaft's diameter and the bore clearance (the seat's).
      if (state.checked(QStringLiteral("keyway"))) {
        pulley.insert(QStringLiteral("keyway"), true);
      }
    } else if (bore == QStringLiteral("plain")) {
      pulley.insert(QStringLiteral("bore"), QJsonObject{{QStringLiteral("type"), QStringLiteral("plain")},
                                                        {QStringLiteral("diameter"),
                                                         state.expression(QStringLiteral("bore_diameter"))}});
      if (state.checked(QStringLiteral("keyway"))) {
        pulley.insert(QStringLiteral("keyway"), true);
      }
    } else if (bore == QStringLiteral("thread")) {
      pulley.insert(QStringLiteral("bore"), QJsonObject{{QStringLiteral("type"), QStringLiteral("thread")},
                                                        {QStringLiteral("thread"), threadOf(state, QStringLiteral("thread"))}});
    }
    if (screwsShown(state)) {
      QJsonObject screws{{QStringLiteral("count"), countValue(state, QStringLiteral("set_screw_count"))},
                         {QStringLiteral("thread"), threadOf(state, QStringLiteral("set_screw_thread"))}};
      if (nonZero(state, QStringLiteral("set_screw_position"))) {
        screws.insert(QStringLiteral("position"), state.expression(QStringLiteral("set_screw_position")));
      }
      pulley.insert(QStringLiteral("set_screws"), screws);
    }
    addOperation(pulley, state);
    result.def = pulley;
    return result;
  };
  def.build = buildPulley;

  // The derived values of the definition the preview showed, with the belt
  // round both pulleys when it runs with another.
  def.note = [buildPulley](const CommandState& state, const CommandContext& model) -> QString {
    Built built = buildPulley(state, model);
    if (!built.error.isEmpty()) {
      return QString();
    }
    built.def.remove(QStringLiteral("type"));
    QJsonObject query{{QStringLiteral("query"), QStringLiteral("pulley_values")}, {QStringLiteral("def"), built.def}};
    const QJsonArray lengths = beltLengths(state, model);
    if (belted(state) && !lengths.isEmpty()) {
      query.insert(QStringLiteral("belt_lengths"), lengths);
    }
    QJsonObject values;
    try {
      values = model.queryObject(query);
    } catch (...) {
      return QString();
    }
    QStringList parts;
    const auto add = [&](const char* key, const QString& text) {
      if (values.contains(QLatin1String(key))) {
        parts << text.arg(lengthText(values.value(QLatin1String(key)).toDouble(), model));
      }
    };
    add("diameter", QObject::tr("diameter %1"));
    add("pitch_diameter", QObject::tr("pitch diameter %1"));
    add("outside_diameter", QObject::tr("outside %1"));
    // The bore a shaft gives (mitcad#578).
    const QString shaft = seatNote(values, model);
    if (!shaft.isEmpty()) {
      parts << shaft;
    }
    add("center_distance", QObject::tr("centre distance %1"));
    add("belt_length", QObject::tr("belt length %1"));
    if (values.contains(QStringLiteral("belt_length")) && values.contains(QStringLiteral("belt_teeth"))) {
      parts.last() += QObject::tr(" (%1 teeth)").arg(number(values.value(QStringLiteral("belt_teeth")).toDouble()));
    }
    add("catalogue_length", QObject::tr("nearest catalogue length %1"));
    if (values.contains(QStringLiteral("teeth_in_mesh"))) {
      parts << QObject::tr("%1 teeth in mesh").arg(number(values.value(QStringLiteral("teeth_in_mesh")).toDouble()));
    }
    const QString error = values.value(QStringLiteral("error")).toString();
    if (!error.isEmpty()) {
      parts << error;
    }
    if (parts.isEmpty()) {
      return QString();
    }
    parts.first()[0] = parts.first()[0].toUpper();
    return parts.join(QStringLiteral(", "));
  };

  def.load = [](const QJsonObject& feature, CommandState& state, const CommandContext& context) {
    state.setChoice(QStringLiteral("kind"), feature.value(QStringLiteral("kind")).toString(QStringLiteral("toothed")));
    const QJsonObject belt = feature.value(QStringLiteral("belt_with")).toObject();
    const bool shaft = loadSeat(feature, state, context);
    if (shaft) {
      loadValue(state, QStringLiteral("angle"), feature.value(QStringLiteral("angle")), feature, context);
    } else if (!belt.isEmpty()) {
      const QString mate = belt.value(QStringLiteral("pulley")).toString();
      state.setItems(QStringLiteral("belt_with"), {SelectionItem{SelectKind::Feature, mate, QString(), QString()}});
      setMate(state, mate, context);
      loadValue(state, QStringLiteral("center_distance"), belt.value(QStringLiteral("center_distance")), feature, context);
      loadValue(state, QStringLiteral("belt_angle"), belt.value(QStringLiteral("angle")), feature, context);
    } else {
      state.setItems(QStringLiteral("plane"), {itemOf(feature.value(QStringLiteral("plane")), context)});
      const QJsonArray center = feature.value(QStringLiteral("center")).toArray();
      state.setText(QStringLiteral("x"), lengthText(center.at(0).toDouble(), context));
      state.setText(QStringLiteral("y"), lengthText(center.at(1).toDouble(), context));
      loadValue(state, QStringLiteral("angle"), feature.value(QStringLiteral("angle")), feature, context);
    }
    const QString profile = feature.value(QStringLiteral("profile")).toString();
    if (!profile.isEmpty()) {
      state.setChoice(QStringLiteral("profile"), profile);
    }
    loadCount(state, QStringLiteral("teeth"), feature.value(QStringLiteral("teeth")), feature, context);
    loadValue(state, QStringLiteral("diameter"), feature.value(QStringLiteral("diameter")), feature, context);
    loadValue(state, QStringLiteral("belt_width"), feature.value(QStringLiteral("belt_width")), feature, context);
    loadValue(state, QStringLiteral("clearance"), feature.value(QStringLiteral("clearance")), feature, context);
    loadValue(state, QStringLiteral("pitch_correction"), feature.value(QStringLiteral("pitch_correction")), feature, context);
    const QJsonObject flanges = feature.value(QStringLiteral("flanges")).toObject();
    state.setChoice(QStringLiteral("flanges"),
                    flanges.isEmpty() ? QStringLiteral("none")
                                      : flanges.value(QStringLiteral("sides")).toString(QStringLiteral("both")));
    loadValue(state, QStringLiteral("flange_diameter"), flanges.value(QStringLiteral("diameter")), feature, context);
    loadValue(state, QStringLiteral("flange_thickness"), flanges.value(QStringLiteral("thickness")), feature, context);
    const QJsonObject bore = feature.value(QStringLiteral("bore")).toObject();
    const QString boreType = bore.value(QStringLiteral("type")).toString(QStringLiteral("none"));
    // On a shaft without a bore of its own: the shaft's, a plain one.
    state.setChoice(QStringLiteral("bore"),
                    shaft && boreType == QStringLiteral("none") ? QStringLiteral("plain") : boreType);
    loadValue(state, QStringLiteral("bore_diameter"), bore.value(QStringLiteral("diameter")), feature, context);
    if (boreType == QStringLiteral("thread")) {
      state.setText(QStringLiteral("thread"),
                    bore.value(QStringLiteral("thread")).toObject().value(QStringLiteral("designation")).toString());
    }
    state.setChecked(QStringLiteral("keyway"), feature.value(QStringLiteral("keyway")).toBool());
    const QJsonObject hub = feature.value(QStringLiteral("hub")).toObject();
    state.setChecked(QStringLiteral("hub"), !hub.isEmpty());
    loadValue(state, QStringLiteral("hub_diameter"), hub.value(QStringLiteral("diameter")), feature, context);
    loadValue(state, QStringLiteral("hub_length"), hub.value(QStringLiteral("length")), feature, context);
    const QJsonObject screws = feature.value(QStringLiteral("set_screws")).toObject();
    state.setChecked(QStringLiteral("set_screws"), !screws.isEmpty());
    loadCount(state, QStringLiteral("set_screw_count"), screws.value(QStringLiteral("count")), feature, context);
    const QString screwThread =
        screws.value(QStringLiteral("thread")).toObject().value(QStringLiteral("designation")).toString();
    if (!screwThread.isEmpty()) {
      state.setText(QStringLiteral("set_screw_thread"), screwThread);
    }
    loadValue(state, QStringLiteral("set_screw_position"), screws.value(QStringLiteral("position")), feature, context);
    loadOperation(feature, state);
  };

  // Only what the panel can show: its kinds, belt profiles and flange
  // sides, a plane it shows, threads of the ISO metric standard, and set
  // screws only where it shows them (a pulley's hub).
  def.canEdit = [&context](const QJsonObject& feature) {
    const QString kind = feature.value(QStringLiteral("kind")).toString(QStringLiteral("toothed"));
    if (!isChoice(kKinds, kind)) {
      return false;
    }
    const QJsonValue profile = feature.value(QStringLiteral("profile"));
    if (!profile.isUndefined() && !profile.isNull() && !isChoice(kProfiles, profile.toString())) {
      return false;
    }
    const QJsonObject flanges = feature.value(QStringLiteral("flanges")).toObject();
    if (!flanges.isEmpty() && !isChoice(kFlangeSides, flanges.value(QStringLiteral("sides")).toString(QStringLiteral("both")))) {
      return false;
    }
    const QJsonValue plane = feature.value(QStringLiteral("plane"));
    if (!plane.isUndefined() && !plane.isNull() && !isShowable(plane, context)) {
      return false;
    }
    if (!seatShowable(feature, context)) {
      return false;
    }
    const bool hub = !feature.value(QStringLiteral("hub")).toObject().isEmpty();
    const QJsonObject screws = feature.value(QStringLiteral("set_screws")).toObject();
    if ((hub || !screws.isEmpty()) && kind != QStringLiteral("toothed")) {
      return false;
    }
    if (!screws.isEmpty() && !hub) {
      return false;
    }
    const QJsonObject boreThread = feature.value(QStringLiteral("bore")).toObject().value(QStringLiteral("thread")).toObject();
    const QJsonObject screwThread = screws.value(QStringLiteral("thread")).toObject();
    return boreThread.value(QStringLiteral("standard")).isUndefined() &&
           screwThread.value(QStringLiteral("standard")).isUndefined();
  };

  def.describe = [](const CommandState& state, const CommandContext& model) {
    if (!toothed(state)) {
      return QStringLiteral("Added pulley (%1), diameter %2")
          .arg(label(kKinds, kindOf(state)), lengthText(state.value(QStringLiteral("diameter")), model));
    }
    return QStringLiteral("Added pulley (%1), %2, %3 teeth")
        .arg(label(kKinds, kindOf(state)), label(kProfiles, state.choice(QStringLiteral("profile"))),
             number(state.value(QStringLiteral("teeth"))));
  };
  return def;
}

} // namespace mitcad::cmd

namespace mitcad {

void registerPulleyCommands(CommandRegistry& registry, const CommandContext& context) {
  registry.add(cmd::pulleyCommand(context));
}

} // namespace mitcad
