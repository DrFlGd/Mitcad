// SPDX-License-Identifier: MIT
// Gear (mitcad#472; commands.md, "Gears"): spur, helical, herringbone,
// ring, rack and straight bevel gears with involute teeth, on a plane or
// meshed with another gear. The panel shows the derived values (the
// model's `gear_values`); OK on a gear that meshes with another links
// their joints (`gear_link`, `add_motion_link`) when both gears' components
// have one. A Shaft places a gear with a bore on a shaft instead of the
// plane (mitcad#578, SeatInputs.hpp), its bore the shaft's diameter and the
// bore clearance.
#include <cmath>
#include <memory>

#include <QCoreApplication>
#include <QJsonArray>
#include <QTimer>

#include "../browser/DocumentHost.hpp"
#include "../framework/CommandRegistry.hpp"
#include "CommandFactories.hpp"
#include "CommandSupport.hpp"
#include "Commands.hpp"
#include "SeatInputs.hpp"

namespace mitcad::cmd {
namespace {

const Choices kKinds = {{QStringLiteral("spur"), QStringLiteral("Spur")},
                        {QStringLiteral("helical"), QStringLiteral("Helical")},
                        {QStringLiteral("herringbone"), QStringLiteral("Herringbone")},
                        {QStringLiteral("ring"), QStringLiteral("Ring")},
                        {QStringLiteral("rack"), QStringLiteral("Rack")},
                        {QStringLiteral("bevel"), QStringLiteral("Bevel")}};

const Choices kPressureAngles = {{QStringLiteral("20"), QStringLiteral("20 deg")},
                                 {QStringLiteral("14.5"), QStringLiteral("14.5 deg")},
                                 {QStringLiteral("25"), QStringLiteral("25 deg")}};

const Choices kBores = {{QStringLiteral("none"), QStringLiteral("None")},
                        {QStringLiteral("plain"), QStringLiteral("Plain")},
                        {QStringLiteral("thread"), QStringLiteral("Thread")}};

QString kindOf(const CommandState& state) { return state.choice(QStringLiteral("kind")); }

bool isOneOf(const CommandState& state, std::initializer_list<const char*> kinds) {
  const QString kind = kindOf(state);
  for (const char* k : kinds) {
    if (kind == QLatin1String(k)) {
      return true;
    }
  }
  return false;
}

bool meshes(const CommandState& state) { return !state.items(QStringLiteral("mesh_with")).isEmpty(); }

// The kind of the gear picked to mesh with (kept beside the inputs).
QString mateKind(const CommandState& state) { return state.object(QStringLiteral("mate")).value(QStringLiteral("kind")).toString(); }

bool takesBore(const CommandState& state) { return isOneOf(state, {"spur", "helical", "herringbone", "bevel"}); }

// On a shaft (mitcad#578): a gear with a bore, not placed by a mesh, not a
// bevel pair.
bool seatOffered(const CommandState& state) {
  const bool bevel = kindOf(state) == QStringLiteral("bevel");
  return takesBore(state) && !(meshes(state) && !bevel) && !(bevel && state.checked(QStringLiteral("pair")));
}
SeatPanel seatPanel() {
  SeatPanel panel;
  panel.offered = seatOffered;
  return panel;
}
bool onShaft(const CommandState& state) { return seatOffered(state) && seated(state); }
// A plain bore of its own diameter; on a shaft any bore but a thread is
// the shaft's.
bool ownPlainBore(const CommandState& state) {
  return takesBore(state) && state.choice(QStringLiteral("bore")) == QStringLiteral("plain") && !onShaft(state);
}
bool shaftBore(const CommandState& state) {
  return onShaft(state) && state.choice(QStringLiteral("bore")) != QStringLiteral("thread");
}

bool helixShown(const CommandState& state) {
  return isOneOf(state, {"helical", "herringbone"}) ||
         (isOneOf(state, {"ring", "rack"}) && state.checked(QStringLiteral("helical")));
}

// Remembers the kind of the gear feature `uid` for the inputs that follow it.
void setMate(CommandState& state, const QString& uid, const CommandContext& context) {
  QString kind;
  try {
    const QJsonObject feature =
        context.queryObject({{QStringLiteral("query"), QStringLiteral("feature")}, {QStringLiteral("uid"), uid}});
    kind = feature.value(QStringLiteral("def")).toObject().value(QStringLiteral("kind")).toString();
  } catch (...) {
  }
  state.setObject(QStringLiteral("mate"), {{QStringLiteral("uid"), uid}, {QStringLiteral("kind"), kind}});
}

// What OK made or edited, for the link after it.
struct GearShared {
  QString editing; // the uid of the feature edited, empty for a new one
};

// The uid of the newest gear feature.
QString newestGear(const CommandContext& context) {
  QString newest;
  long best = -1;
  try {
    const QJsonObject timeline = context.queryObject({{QStringLiteral("query"), QStringLiteral("timeline")}});
    for (const QJsonValue& value : timeline.value(QStringLiteral("features")).toArray()) {
      const QJsonObject feature = value.toObject();
      if (feature.value(QStringLiteral("type")).toString() != QStringLiteral("gear")) {
        continue;
      }
      const QString uid = feature.value(QStringLiteral("uid")).toString();
      const long number = uid.mid(1).toLong();
      if (number > best) {
        best = number;
        newest = uid;
      }
    }
  } catch (...) {
  }
  return newest;
}

// A length for the note, in the document's unit.
QString lengthOf(const QJsonObject& values, const char* key, const CommandContext& context) {
  return lengthText(values.value(QLatin1String(key)).toDouble(), context);
}

} // namespace

// Links the gear `editing` (the newest gear when empty) to its mate once
// the model is idle, when the model can (`gear_link`).
void linkLater(DocumentHost& host, const QString& editing);

CommandDef gearCommand(const CommandContext& context, DocumentHost& host) {
  CommandDef def;
  def.id = QStringLiteral("solid.gear");
  def.name = QObject::tr("Gear");
  def.icon = QStringLiteral("gear");
  def.tooltip = QObject::tr("A spur, helical, herringbone, ring, rack or bevel gear with involute teeth");
  def.group = QStringLiteral("CREATE");
  def.section = QStringLiteral("machine");
  def.subgroup = QStringLiteral("Machine Elements");
  def.keywords = {QStringLiteral("spur"),        QStringLiteral("helical"), QStringLiteral("herringbone"),
                  QStringLiteral("rack"),        QStringLiteral("pinion"),  QStringLiteral("bevel"),
                  QStringLiteral("ring"),        QStringLiteral("internal"), QStringLiteral("involute"),
                  QStringLiteral("cog")};
  def.featureType = QStringLiteral("gear");
  const auto shared = std::make_shared<GearShared>();
  const auto shownUnmeshed = [](const CommandState& state) { return !meshes(state) || kindOf(state) == QStringLiteral("bevel"); };
  const auto onPlane = [shownUnmeshed](const CommandState& state) { return shownUnmeshed(state) && !onShaft(state); };
  def.inputs = {
      choiceInput(QStringLiteral("kind"), QObject::tr("Type"), kKinds, QStringLiteral("spur"))
          .withChoiceIcons({{QStringLiteral("spur"), QStringLiteral("gear-spur")},
                            {QStringLiteral("helical"), QStringLiteral("gear-helical")},
                            {QStringLiteral("herringbone"), QStringLiteral("gear-herringbone")},
                            {QStringLiteral("ring"), QStringLiteral("gear-ring")},
                            {QStringLiteral("rack"), QStringLiteral("gear-rack")},
                            {QStringLiteral("bevel"), QStringLiteral("gear-bevel")}}),
      selectionInput(QStringLiteral("mesh_with"), QObject::tr("Mesh With"), SelectKind::Feature | SelectKind::Face, 0, 1)
          .withTooltip(QObject::tr("A gear to mesh with: the new gear sits at the centre distance, turned to engage"))
          .withAccepts([](const SelectionItem& item) { return !item.creatingFeature().isEmpty(); })
          .withConvert([](const SelectionItem& item) {
            return SelectionItem{SelectKind::Feature, item.creatingFeature(), QString(), QString()};
          })
          .withOnPick([](const SelectionItem& item, CommandState& state, const CommandContext& model) {
            setMate(state, item.owner, model);
          })
          .withVisible([](const CommandState& s) { return kindOf(s) != QStringLiteral("bevel"); }),
      valueInput(QStringLiteral("mesh_angle"), QObject::tr("Mesh Angle"), ValueKind::Angle, QStringLiteral("0 deg"))
          .withTooltip(QObject::tr("Where the gear sits around the gear it meshes with, from its plane's x axis"))
          .withVisible([](const CommandState& s) {
            return meshes(s) && kindOf(s) != QStringLiteral("bevel") && mateKind(s) != QStringLiteral("rack");
          }),
      valueInput(QStringLiteral("position"), QObject::tr("Position"), ValueKind::Length, QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("Along the rack it meshes with, from the middle of its teeth"))
          .withVisible([](const CommandState& s) { return meshes(s) && mateKind(s) == QStringLiteral("rack"); }),
      selectionInput(QStringLiteral("plane"), QObject::tr("Plane"), kPlaneKinds, 0, 1)
          .withTooltip(QObject::tr("A plane or planar face; a click places the centre (a rack's start)"))
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
          .withTooltip(QObject::tr("Turns the gear about its axis (a rack about its start)"))
          .withVisible(shownUnmeshed),
      valueInput(QStringLiteral("module"), QObject::tr("Module"), ValueKind::Length, QStringLiteral("2 mm"))
          .withTooltip(QObject::tr("The pitch diameter per tooth (normal module of helical teeth)")),
      valueInput(QStringLiteral("teeth"), QObject::tr("Teeth"), ValueKind::Unitless, QStringLiteral("20")),
      choiceInput(QStringLiteral("pressure_angle"), QObject::tr("Pressure Angle"), kPressureAngles, QStringLiteral("20")),
      valueInput(QStringLiteral("face_width"), QObject::tr("Face Width"), ValueKind::Length, QStringLiteral("10 mm")),
      checkInput(QStringLiteral("helical"), QObject::tr("Helical Teeth"), false)
          .withVisible([](const CommandState& s) { return isOneOf(s, {"ring", "rack"}); }),
      valueInput(QStringLiteral("helix_angle"), QObject::tr("Helix Angle"), ValueKind::Angle, QStringLiteral("20 deg"))
          .withVisible(helixShown),
      checkInput(QStringLiteral("left_handed"), QObject::tr("Left-Handed"), false).withVisible(helixShown),
      valueInput(QStringLiteral("gap"), QObject::tr("Gap"), ValueKind::Length, QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("Between the two halves, cut down below the roots"))
          .withVisible([](const CommandState& s) { return kindOf(s) == QStringLiteral("herringbone"); }),
      valueInput(QStringLiteral("rim"), QObject::tr("Rim Thickness"), ValueKind::Length, QStringLiteral("5 mm"))
          .withVisible([](const CommandState& s) { return kindOf(s) == QStringLiteral("ring"); }),
      valueInput(QStringLiteral("height"), QObject::tr("Height"), ValueKind::Length, QStringLiteral("20 mm"))
          .withTooltip(QObject::tr("From the rack's back to its tips"))
          .withVisible([](const CommandState& s) { return kindOf(s) == QStringLiteral("rack"); }),
      valueInput(QStringLiteral("length"), QObject::tr("Length"), ValueKind::Length, QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("0: the teeth's pitch line, teeth times the pitch"))
          .withVisible([](const CommandState& s) { return kindOf(s) == QStringLiteral("rack"); }),
      valueInput(QStringLiteral("shaft_angle"), QObject::tr("Shaft Angle"), ValueKind::Angle, QStringLiteral("90 deg"))
          .withVisible([](const CommandState& s) { return kindOf(s) == QStringLiteral("bevel"); }),
      valueInput(QStringLiteral("mate_teeth"), QObject::tr("Mate Teeth"), ValueKind::Unitless, QStringLiteral("20"))
          .withVisible([](const CommandState& s) { return kindOf(s) == QStringLiteral("bevel"); }),
      checkInput(QStringLiteral("pair"), QObject::tr("Make the Pair"), true)
          .withTooltip(QObject::tr("Also the mating gear, placed to mesh"))
          .withVisible([](const CommandState& s) { return kindOf(s) == QStringLiteral("bevel"); }),
      valueInput(QStringLiteral("backlash"), QObject::tr("Backlash"), ValueKind::Length, QStringLiteral("0 mm"))
          .withPrintedFit(QStringLiteral("gear"))
          .withTooltip(QObject::tr("Each tooth is thinner by half of it: two gears with the same value mesh with it"))
          .withHeading(QObject::tr("Profile")),
      valueInput(QStringLiteral("profile_shift"), QObject::tr("Profile Shift"), ValueKind::Unitless, QStringLiteral("0"))
          .withTooltip(QObject::tr("In modules, away from the axis")),
      choiceInput(QStringLiteral("bore"), QObject::tr("Bore"), kBores, QStringLiteral("none"))
          .withHeading(QObject::tr("Bore and Hub"))
          .withVisible(takesBore),
      valueInput(QStringLiteral("bore_diameter"), QObject::tr("Bore Diameter"), ValueKind::Length, QStringLiteral("8 mm"))
          .withVisible(ownPlainBore),
      textInput(QStringLiteral("thread"), QObject::tr("Thread Size"), QStringLiteral("M8x1.25"))
          .withTooltip(QObject::tr("An ISO metric size: the bore is its tap drill"))
          .withVisible([](const CommandState& s) { return takesBore(s) && s.choice(QStringLiteral("bore")) == QStringLiteral("thread"); }),
      checkInput(QStringLiteral("keyway"), QObject::tr("Keyway"), false)
          .withTooltip(QObject::tr("DIN 6885 for the bore's diameter"))
          .withVisible([](const CommandState& s) {
            return (takesBore(s) && s.choice(QStringLiteral("bore")) == QStringLiteral("plain")) || shaftBore(s);
          }),
      checkInput(QStringLiteral("hub"), QObject::tr("Hub"), false).withVisible(takesBore),
      valueInput(QStringLiteral("hub_diameter"), QObject::tr("Hub Diameter"), ValueKind::Length, QStringLiteral("20 mm"))
          .withVisible([](const CommandState& s) { return takesBore(s) && s.checked(QStringLiteral("hub")); }),
      valueInput(QStringLiteral("hub_length"), QObject::tr("Hub Length"), ValueKind::Length, QStringLiteral("8 mm"))
          .withVisible([](const CommandState& s) { return takesBore(s) && s.checked(QStringLiteral("hub")); }),
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
        input.withPrintedFit(QStringLiteral("gear"));
      }
      def.inputs.insert(at++, input);
    }
  }
  def.inputs += operationInputs();

  def.init = [shared](CommandState& state, const CommandContext& model) {
    shared->editing.clear();
    initSeat(state, model);
    // A face selected before the command meshes with its gear; another
    // feature's face is no gear to mesh with.
    Selection& mesh = state.items(QStringLiteral("mesh_with"));
    if (!mesh.isEmpty()) {
      setMate(state, mesh.first().owner, model);
      if (mateKind(state).isEmpty()) {
        mesh.clear();
        state.setObject(QStringLiteral("mate"), QJsonObject());
      }
    }
    if (state.items(QStringLiteral("plane")).isEmpty()) {
      state.setItems(QStringLiteral("plane"), {itemOf(QJsonValue(QStringLiteral("xy")), model)});
    }
  };

  const auto buildGear = [seat](const CommandState& state, const CommandContext& model) {
    Built result;
    const QString kind = kindOf(state);
    const bool meshed = meshes(state) && kind != QStringLiteral("bevel");
    QJsonObject gear{{QStringLiteral("type"), QStringLiteral("gear")},
                     {QStringLiteral("kind"), kind},
                     {QStringLiteral("module"), state.expression(QStringLiteral("module"))},
                     {QStringLiteral("teeth"), countValue(state, QStringLiteral("teeth"))},
                     {QStringLiteral("face_width"), state.expression(QStringLiteral("face_width"))}};
    if (meshed) {
      QJsonObject mesh{{QStringLiteral("gear"), state.items(QStringLiteral("mesh_with")).first().owner}};
      if (mateKind(state) == QStringLiteral("rack")) {
        if (nonZero(state, QStringLiteral("position"))) {
          mesh.insert(QStringLiteral("position"), state.expression(QStringLiteral("position")));
        }
      } else if (nonZero(state, QStringLiteral("mesh_angle"))) {
        mesh.insert(QStringLiteral("angle"), state.expression(QStringLiteral("mesh_angle")));
      }
      gear.insert(QStringLiteral("mesh_with"), mesh);
    } else if (onShaft(state)) {
      addSeat(gear, state, model, seat);
      if (nonZero(state, QStringLiteral("angle"))) {
        gear.insert(QStringLiteral("angle"), state.expression(QStringLiteral("angle")));
      }
    } else {
      const Selection& plane = state.items(QStringLiteral("plane"));
      if (plane.isEmpty()) {
        return Built::failure(QObject::tr("Pick a plane, or a gear to mesh with"), QStringLiteral("plane"));
      }
      gear.insert(QStringLiteral("plane"), plane.first().reference());
      gear.insert(QStringLiteral("center"),
                  QJsonArray{state.value(QStringLiteral("x")), state.value(QStringLiteral("y"))});
      if (nonZero(state, QStringLiteral("angle"))) {
        gear.insert(QStringLiteral("angle"), state.expression(QStringLiteral("angle")));
      }
    }
    const QString pressure = state.choice(QStringLiteral("pressure_angle"));
    if (pressure != QStringLiteral("20") && !pressure.isEmpty()) {
      gear.insert(QStringLiteral("pressure_angle"), pressure + QStringLiteral(" deg"));
    }
    if (helixShown(state)) {
      gear.insert(QStringLiteral("helix_angle"), state.expression(QStringLiteral("helix_angle")));
      if (state.checked(QStringLiteral("left_handed"))) {
        gear.insert(QStringLiteral("left_handed"), true);
      }
    }
    if (kind == QStringLiteral("herringbone") && nonZero(state, QStringLiteral("gap"))) {
      gear.insert(QStringLiteral("gap"), state.expression(QStringLiteral("gap")));
    }
    if (kind == QStringLiteral("ring")) {
      gear.insert(QStringLiteral("rim"), state.expression(QStringLiteral("rim")));
    }
    if (kind == QStringLiteral("rack")) {
      gear.insert(QStringLiteral("height"), state.expression(QStringLiteral("height")));
      if (nonZero(state, QStringLiteral("length"))) {
        gear.insert(QStringLiteral("length"), state.expression(QStringLiteral("length")));
      }
    }
    if (kind == QStringLiteral("bevel")) {
      gear.insert(QStringLiteral("shaft_angle"), state.expression(QStringLiteral("shaft_angle")));
      gear.insert(QStringLiteral("mate_teeth"), countValue(state, QStringLiteral("mate_teeth")));
      if (state.checked(QStringLiteral("pair"))) {
        gear.insert(QStringLiteral("pair"), true);
      }
    }
    if (std::isfinite(state.value(QStringLiteral("backlash")))) {
      gear.insert(QStringLiteral("backlash"), state.expression(QStringLiteral("backlash")));
    }
    if (nonZero(state, QStringLiteral("profile_shift"))) {
      gear.insert(QStringLiteral("profile_shift"), countValue(state, QStringLiteral("profile_shift")));
    }
    const bool pair = kind == QStringLiteral("bevel") && state.checked(QStringLiteral("pair"));
    if (takesBore(state) && !pair) {
      const QString bore = state.choice(QStringLiteral("bore"));
      if (shaftBore(state)) {
        // The shaft's diameter and the bore clearance (the seat's).
        if (state.checked(QStringLiteral("keyway"))) {
          gear.insert(QStringLiteral("keyway"), true);
        }
      } else if (bore == QStringLiteral("plain")) {
        gear.insert(QStringLiteral("bore"), QJsonObject{{QStringLiteral("type"), QStringLiteral("plain")},
                                                        {QStringLiteral("diameter"),
                                                         state.expression(QStringLiteral("bore_diameter"))}});
        if (state.checked(QStringLiteral("keyway"))) {
          gear.insert(QStringLiteral("keyway"), true);
        }
      } else if (bore == QStringLiteral("thread")) {
        gear.insert(QStringLiteral("bore"),
                    QJsonObject{{QStringLiteral("type"), QStringLiteral("thread")},
                                {QStringLiteral("thread"),
                                 QJsonObject{{QStringLiteral("designation"), state.text(QStringLiteral("thread")).trimmed()}}}});
      }
      if (state.checked(QStringLiteral("hub"))) {
        gear.insert(QStringLiteral("hub"),
                    QJsonObject{{QStringLiteral("diameter"), state.expression(QStringLiteral("hub_diameter"))},
                                {QStringLiteral("length"), state.expression(QStringLiteral("hub_length"))}});
      }
    }
    addOperation(gear, state);
    result.def = gear;
    return result;
  };
  def.build = buildGear;

  // The derived values of the definition the preview showed.
  def.note = [buildGear](const CommandState& state, const CommandContext& model) -> QString {
    Built built = buildGear(state, model);
    if (!built.error.isEmpty()) {
      return QString();
    }
    built.def.remove(QStringLiteral("type"));
    QJsonObject values;
    try {
      values = model.queryObject({{QStringLiteral("query"), QStringLiteral("gear_values")},
                                  {QStringLiteral("def"), built.def}});
    } catch (...) {
      return QString();
    }
    QStringList parts;
    const auto add = [&](const char* key, const QString& text) {
      if (values.contains(QLatin1String(key))) {
        parts << text.arg(lengthOf(values, key, model));
      }
    };
    add("pitch_diameter", QObject::tr("pitch diameter %1"));
    add("outside_diameter", QObject::tr("outside %1"));
    add("inside_diameter", QObject::tr("inside %1"));
    add("root_diameter", QObject::tr("root %1"));
    add("pitch_line_length", QObject::tr("pitch line %1"));
    add("cone_distance", QObject::tr("cone distance %1"));
    if (values.contains(QStringLiteral("pitch_angle"))) {
      parts << QObject::tr("pitch angle %1").arg(degrees(values.value(QStringLiteral("pitch_angle")).toDouble()));
    }
    add("centre_distance", QObject::tr("centre distance %1"));
    // The bore a shaft gives (mitcad#578).
    const QString shaft = seatNote(values, model);
    if (!shaft.isEmpty()) {
      parts << shaft;
    }
    if (values.value(QStringLiteral("undercut")).toBool()) {
      parts << QObject::tr("undercut");
    }
    if (parts.isEmpty()) {
      return QString();
    }
    parts.first()[0] = parts.first()[0].toUpper();
    return parts.join(QStringLiteral(", "));
  };

  def.load = [shared](const QJsonObject& feature, CommandState& state, const CommandContext& context) {
    shared->editing = feature.value(QStringLiteral("uid")).toString();
    const QString kind = feature.value(QStringLiteral("kind")).toString(QStringLiteral("spur"));
    state.setChoice(QStringLiteral("kind"), kind);
    const QJsonObject mesh = feature.value(QStringLiteral("mesh_with")).toObject();
    const bool shaft = loadSeat(feature, state, context);
    if (shaft) {
      loadValue(state, QStringLiteral("angle"), feature.value(QStringLiteral("angle")), feature, context);
    } else if (!mesh.isEmpty()) {
      const QString mate = mesh.value(QStringLiteral("gear")).toString();
      state.setItems(QStringLiteral("mesh_with"), {SelectionItem{SelectKind::Feature, mate, QString(), QString()}});
      setMate(state, mate, context);
      loadValue(state, QStringLiteral("mesh_angle"), mesh.value(QStringLiteral("angle")), feature, context);
      loadValue(state, QStringLiteral("position"), mesh.value(QStringLiteral("position")), feature, context);
    } else {
      state.setItems(QStringLiteral("plane"), {itemOf(feature.value(QStringLiteral("plane")), context)});
      const QJsonArray center = feature.value(QStringLiteral("center")).toArray();
      state.setText(QStringLiteral("x"), lengthText(center.at(0).toDouble(), context));
      state.setText(QStringLiteral("y"), lengthText(center.at(1).toDouble(), context));
      loadValue(state, QStringLiteral("angle"), feature.value(QStringLiteral("angle")), feature, context);
    }
    loadValue(state, QStringLiteral("module"), feature.value(QStringLiteral("module")), feature, context);
    loadCount(state, QStringLiteral("teeth"), feature.value(QStringLiteral("teeth")), feature, context);
    loadValue(state, QStringLiteral("face_width"), feature.value(QStringLiteral("face_width")), feature, context);
    const QJsonValue pressure = feature.value(QStringLiteral("pressure_angle"));
    if (!pressure.isUndefined() && !pressure.isNull()) {
      // The panel's choices: 14.5, 20 or 25 degrees (canEdit refuses others).
      const double degrees = context.queryObject({{QStringLiteral("query"), QStringLiteral("evaluate")},
                                                  {QStringLiteral("expression"), context.valueText(pressure, feature.value(QStringLiteral("uid")).toString())},
                                                  {QStringLiteral("kind"), QStringLiteral("angle")}})
                                 .value(QStringLiteral("value"))
                                 .toDouble() *
                             180.0 / 3.14159265358979323846;
      state.setChoice(QStringLiteral("pressure_angle"),
                      std::abs(degrees - 14.5) < 1e-6 ? QStringLiteral("14.5")
                      : std::abs(degrees - 25.0) < 1e-6 ? QStringLiteral("25")
                                                         : QStringLiteral("20"));
    }
    const QJsonValue helix = feature.value(QStringLiteral("helix_angle"));
    state.setChecked(QStringLiteral("helical"), !helix.isUndefined() && !helix.isNull());
    loadValue(state, QStringLiteral("helix_angle"), helix, feature, context);
    state.setChecked(QStringLiteral("left_handed"), feature.value(QStringLiteral("left_handed")).toBool());
    loadValue(state, QStringLiteral("gap"), feature.value(QStringLiteral("gap")), feature, context);
    loadValue(state, QStringLiteral("rim"), feature.value(QStringLiteral("rim")), feature, context);
    loadValue(state, QStringLiteral("height"), feature.value(QStringLiteral("height")), feature, context);
    loadValue(state, QStringLiteral("length"), feature.value(QStringLiteral("length")), feature, context);
    loadValue(state, QStringLiteral("shaft_angle"), feature.value(QStringLiteral("shaft_angle")), feature, context);
    loadCount(state, QStringLiteral("mate_teeth"), feature.value(QStringLiteral("mate_teeth")), feature, context);
    state.setChecked(QStringLiteral("pair"), feature.value(QStringLiteral("pair")).toBool());
    loadValue(state, QStringLiteral("backlash"), feature.value(QStringLiteral("backlash")), feature, context);
    loadCount(state, QStringLiteral("profile_shift"), feature.value(QStringLiteral("profile_shift")), feature, context);
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
    loadOperation(feature, state);
  };

  // Only what the panel can show: a plane it shows and a thread of the ISO
  // metric standard.
  def.canEdit = [&context](const QJsonObject& feature) {
    const QJsonValue plane = feature.value(QStringLiteral("plane"));
    if (!plane.isUndefined() && !plane.isNull() && !isShowable(plane, context)) {
      return false;
    }
    if (!seatShowable(feature, context)) {
      return false;
    }
    const QJsonObject thread = feature.value(QStringLiteral("bore")).toObject().value(QStringLiteral("thread")).toObject();
    return thread.value(QStringLiteral("standard")).isUndefined();
  };

  // After OK (describe reads the model after it): a gear that meshes with
  // another has their joints linked when both have one and they are not
  // linked yet, once the model is idle again.
  def.describe = [shared, &host](const CommandState& state, const CommandContext&) {
    const QString editing = shared->editing;
    if (meshes(state) && kindOf(state) != QStringLiteral("bevel")) {
      linkLater(host, editing);
    }
    return QStringLiteral("Added gear (%1), %2 teeth, module %3")
        .arg(label(kKinds, kindOf(state)))
        .arg(number(state.value(QStringLiteral("teeth"))))
        .arg(number(state.value(QStringLiteral("module"))));
  };
  return def;
}

void linkLater(DocumentHost& host, const QString& editing) {
  {
    QTimer::singleShot(0, qApp, [&host, editing] {
      host.whenIdle(qApp, [&host, editing] {
        const QString uid = editing.isEmpty() ? newestGear(host.model()) : editing;
        if (uid.isEmpty()) {
          return;
        }
        QJsonObject link;
        try {
          link = host.model().queryObject({{QStringLiteral("query"), QStringLiteral("gear_link")},
                                           {QStringLiteral("gear"), uid}});
        } catch (...) {
          return;
        }
        const QJsonObject command = link.value(QStringLiteral("command")).toObject();
        if (command.isEmpty()) {
          qDebug().noquote() << QStringLiteral("Gear %1 not linked: %2")
                                    .arg(uid, link.value(QStringLiteral("reason")).toString());
          return;
        }
        if (host.runModelCommand(command)) {
          qDebug().noquote() << QStringLiteral("Gear %1 linked to its mate's joint").arg(uid);
        }
      });
    });
  }
}

} // namespace mitcad::cmd

namespace mitcad {

void registerGearCommands(CommandRegistry& registry, const CommandContext& context, DocumentHost& host) {
  registry.add(cmd::gearCommand(context, host));
}

} // namespace mitcad
