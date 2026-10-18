// SPDX-License-Identifier: MIT
// The BUILD tab (mitcad#375; the model's side in commands.md, "BUILD:
// profile assemblies"; app/COMMANDS.md, "BUILD"): frames of slot profiles
// with their connectors, panels and linear parts.
//
//   PROFILE  Profile, Panel, Linear Part: the part as a tile that opens
//            the part picker, then clicks; Place (Enter) keeps the panel
//            open for the next one.
//   CONNECT  Connector (a junction, the fitting connectors), Fasteners.
//   MODIFY   Length (bind, detach), Ends (mitres), Flip, Rotate 90 deg.
//   OUTPUT   Cut List, Parts List.
//
// Everything goes through the model's BUILD commands and queries
// (build_place, build_edit, build_connect, build_panel, build_remove;
// build_preview, build_parts, build_junctions, build_cut_list) and the
// library commands (library_build_candidates, library_search,
// library_show): the commands are ordinary panels (apply with inspect as
// their preview, CommandDef), so live MCP fills them as any other.
#include "Commands.hpp"

#include <algorithm>
#include <cmath>
#include <exception>
#include <functional>
#include <limits>
#include <memory>
#include <mutex>
#include <optional>
#include <utility>
#include <vector>

#include <QByteArray>
#include <QHash>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QPointer>
#include <QStringList>
#include <QWidget>
#include <QtLogging>

#include <BRep_Tool.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Pnt.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>
#include <gp_XYZ.hxx>

#include "../browser/DocumentHost.hpp"
#include "../build/BuildCatalog.hpp"
#include "../build/BuildLists.hpp"
#include "../build/BuildModel.hpp"
#include "../files/Libraries.hpp"
#include "../framework/CommandRegistry.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Json.hpp"
#include "../library/PartPicker.hpp"
#include "../library/UsedParts.hpp"
#include "CommandSupport.hpp"

namespace mitcad {
namespace {

using build::BuildPart;
using build::Part;
using cmd::Choices;

const QString kTab = QStringLiteral("BUILD");
constexpr double kNaN = std::numeric_limits<double>::quiet_NaN();
constexpr double kPi = 3.14159265358979323846;
constexpr SelectFilter kPartKinds = SelectKind::Face | SelectKind::Body;

QString str(const QJsonObject& object, const char* key) { return object.value(QLatin1String(key)).toString(); }

// What the definitions share while the application runs: the parts of the
// pickers, the junctions, the design's BUILD parts, the preview shown last
// (the length's handle stands on it). Read and written on the UI thread;
// an inspection may read it on the model's worker (the AppKit frontend).
struct Shared {
  build::Catalog catalog;
  mutable std::mutex mutex;
  // The preview shown last: the new part's frame and length.
  build::Frame frame;
  double length = kNaN;
  bool bound = false;
  // build_junctions, and each junction's label for the choice.
  QJsonArray junctions;
  Choices junctionChoices;
  // The design's BUILD parts and which component each occurrence path places.
  QVector<BuildPart> parts;
  QHash<QString, QString> components;
  // The part chosen or placed last, per role: the picker starts with it.
  QHash<QString, QString> lastPart;
  // The system of the part chosen last, per role (mitcad#427): the part
  // picker opens filtered to it.
  QHash<QString, QString> lastSystem;
  // The configuration rows of a source's component (Fasteners' sizes).
  QHash<QString, Choices> rows;
  // The library's preview image of a part, by "<library>|<component>",
  // read once (library_preview); null when it has none.
  QHash<QString, QImage> thumbnails;
  // The side placement shown last (mitcad#417): its reference and offset
  // from it, its width along the slot and the target's length. A change of
  // From keeps the part where this puts it.
  struct Side {
    QString from;
    double offset = kNaN;
    double width = kNaN;
    double length = kNaN;
  };
  Side side;
  // The part picker (mitcad#418): the main window it belongs to, the host
  // its choices go through and the picker once wired. UI thread only.
  QWidget* window = nullptr;
  DocumentHost* host = nullptr;
  QPointer<PartPicker> picker;

  void setSide(const Side& shown) {
    const std::lock_guard<std::mutex> lock(mutex);
    side = shown;
  }
  Side shownSide() const {
    const std::lock_guard<std::mutex> lock(mutex);
    return side;
  }

  QImage thumbnail(const Part& part) {
    const QString key = part.library + QLatin1Char('|') + part.component;
    {
      const std::lock_guard<std::mutex> lock(mutex);
      const auto found = thumbnails.constFind(key);
      if (found != thumbnails.cend()) {
        return found.value();
      }
    }
    QJsonObject request{{QStringLiteral("cmd"), QStringLiteral("library_preview")},
                        {QStringLiteral("rev"), part.rev},
                        {QStringLiteral("component"), part.component}};
    if (!part.url.isEmpty()) {
      request.insert(QStringLiteral("url"), part.url);
    } else {
      request.insert(QStringLiteral("id"), part.library);
    }
    const QJsonObject answer = libraryCommand(request);
    QImage image;
    const QJsonValue png = answer.value(QStringLiteral("png"));
    if (png.isString()) {
      image = QImage::fromData(QByteArray::fromBase64(png.toString().toLatin1()), "PNG");
    }
    qInfo().noquote() << QStringLiteral("BUILD preview image of %1/%2: %3")
                             .arg(part.library, part.component,
                                  image.isNull() ? QStringLiteral("none")
                                                 : QStringLiteral("%1x%2").arg(image.width()).arg(image.height()));
    const std::lock_guard<std::mutex> lock(mutex);
    thumbnails.insert(key, image);
    return image;
  }

  void setPreview(const build::Frame& shown, double shownLength, bool shownBound) {
    const std::lock_guard<std::mutex> lock(mutex);
    frame = shown;
    length = shownLength;
    bound = shownBound;
  }
  std::pair<build::Frame, double> preview() const {
    const std::lock_guard<std::mutex> lock(mutex);
    return {frame, length};
  }
  // Whether the preview shown last had its length bound (an auto length
  // with a target): then the length has no handle.
  bool previewBound() const {
    const std::lock_guard<std::mutex> lock(mutex);
    return bound;
  }
  // Whether the design has parts a part of `role` goes on (mitcad#394):
  // profiles for a profile, profiles and linear parts (a rail for its
  // carriage) for a linear part. Without them a placement is a ground
  // placement.
  bool hasTargets(const QString& role) const {
    const std::lock_guard<std::mutex> lock(mutex);
    return std::any_of(parts.begin(), parts.end(), [&role](const BuildPart& part) {
      return targets(role, part.role);
    });
  }
  static bool targets(const QString& role, const QString& target) {
    return target == QStringLiteral("profile") ||
           (role == QStringLiteral("linear") && target == QStringLiteral("linear"));
  }
  QJsonObject junction(const QString& id) const {
    const std::lock_guard<std::mutex> lock(mutex);
    for (const QJsonValue& value : junctions) {
      if (str(value.toObject(), "id") == id) {
        return value.toObject();
      }
    }
    return {};
  }
  Choices junctionOptions() const {
    const std::lock_guard<std::mutex> lock(mutex);
    return junctionChoices;
  }
  QVector<BuildPart> designParts() const {
    const std::lock_guard<std::mutex> lock(mutex);
    return parts;
  }
  // The BUILD part an item's occurrence path places (the innermost).
  std::optional<BuildPart> partAt(const QString& occurrence) const {
    const std::lock_guard<std::mutex> lock(mutex);
    QStringList steps = occurrence.split(QLatin1Char('/'), Qt::SkipEmptyParts);
    while (!steps.isEmpty()) {
      const QString component = components.value(steps.join(QLatin1Char('/')));
      for (const BuildPart& part : parts) {
        if (part.component == component) {
          return part;
        }
      }
      steps.removeLast();
    }
    return std::nullopt;
  }

  // Reads the junctions and the design's parts again (a command starts, a
  // part was placed or connected).
  void refreshDesign(const CommandContext& context) {
    const QJsonArray found = build::junctions(context);
    Choices labels;
    for (const QJsonValue& value : found) {
      const QJsonObject entry = value.toObject();
      labels.append({str(entry, "id"), build::junctionText(entry, context)});
    }
    const QVector<BuildPart> design = build::buildParts(context);
    QHash<QString, QString> placed;
    try {
      std::function<void(const QJsonArray&, const QString&)> walk = [&](const QJsonArray& level, const QString& parent) {
        for (const QJsonValue& value : level) {
          const QJsonObject node = value.toObject();
          const QString path = parent.isEmpty() ? str(node, "uid") : parent + QLatin1Char('/') + str(node, "uid");
          placed.insert(path, str(node, "component"));
          walk(node.value(QStringLiteral("children")).toArray(), path);
        }
      };
      walk(context.queryObject({{QStringLiteral("query"), QStringLiteral("components")}})
               .value(QStringLiteral("occurrences"))
               .toArray(),
           QString());
    } catch (const std::exception&) {
      // No occurrences then.
    }
    const std::lock_guard<std::mutex> lock(mutex);
    junctions = found;
    junctionChoices = labels;
    parts = design;
    components = placed;
  }
};

using SharedPtr = std::shared_ptr<Shared>;

// --- The picker

const QString kSearch = QStringLiteral("search");
const QString kSystem = QStringLiteral("system");
const QString kPart = QStringLiteral("part");

bool chosenPart(const Shared& shared, const CommandState& state, Part& part) {
  return shared.catalog.find(state.choice(kPart), part);
}

// A cut-to-length part starts with its default length (its stock length,
// else 500 mm, mitcad#394), unless a length was typed: the length shown
// follows the chosen part while it is the default one.
void followPart(const Shared& shared, CommandState& state, const CommandContext& context) {
  Part part;
  if (!chosenPart(shared, state, part) || !part.cutToLength()) {
    return;
  }
  const QString shown = cmd::lengthText(part.defaultLength(), context);
  const QString before = state.object(QStringLiteral("length_default")).value(QStringLiteral("text")).toString();
  const QString typed = state.text(QStringLiteral("length")).trimmed();
  if (typed.isEmpty() || typed == before) {
    state.setText(QStringLiteral("length"), shown);
    state.setValue(QStringLiteral("length"), kNaN);
    state.setObject(QStringLiteral("length_default"), {{QStringLiteral("text"), shown}});
  }
}

// The first part the search and the system leave, when the chosen one is
// not among them any more.
void followFilter(const Shared& shared, const QString& role, CommandState& state, const CommandContext& context) {
  const QVector<Part> parts = shared.catalog.parts(role);
  const Choices shown = build::partChoices(parts, state.text(kSearch), state.choice(kSystem), QString());
  const bool kept = std::any_of(shown.begin(), shown.end(),
                                [&state](const auto& option) { return option.first == state.choice(kPart); });
  if (!kept && !shown.isEmpty()) {
    state.setChoice(kPart, shown.first().first);
  }
  followPart(shared, state, context);
}

// --- The part picker (mitcad#418; app/COMMANDS.md, "Part picker")
//
// The parts a command takes as pictures, in a floating window of the main
// window: the Part tile (Connector's Connector; mitcad#427, the button
// beside the drop-down where a frontend has no tiles) opens it, a tile
// clicked there is chosen as the drop-down chooses it (the same key, so the
// preview follows at once), and it closes with the command unless pinned;
// a pinned picker that is open follows the next command that chooses
// parts. In this design reads `library_usage` (usedLibraryParts); the main
// window tells it when the design changed.

// The button beside the field that opens it.
const QString kButtonText = QStringLiteral("...");
QString buttonTip() {
  return QObject::tr("The part picker: the parts as pictures, your favourites first, and the parts this design "
                     "uses; a click chooses one");
}

// The id and the picker's title of a role's command.
QString commandOf(const QString& role) {
  return role == QStringLiteral("profile") ? QStringLiteral("build.profile")
         : role == QStringLiteral("panel") ? QStringLiteral("build.panel")
                                           : QStringLiteral("build.linear");
}
QString titleOf(const QString& role) {
  return role == QStringLiteral("profile") ? QObject::tr("Profile")
         : role == QStringLiteral("panel") ? QObject::tr("Panel")
                                           : QObject::tr("Linear Part");
}

// The main window's picker, wired the first time it is met: its choice
// goes into the BUILD command it was asked for (DocumentHost::
// chooseInCommand). Without `create` only a picker that is there already.
PartPicker* partPicker(Shared& shared, bool create) {
  if (shared.window == nullptr || shared.host == nullptr) {
    return nullptr;
  }
  PartPicker* picker = nullptr;
  if (create) {
    picker = PartPicker::of(shared.window);
  } else if (QWidget* top = shared.window->window()) {
    picker = top->findChild<PartPicker*>(QString(), Qt::FindDirectChildrenOnly);
  }
  if (picker != nullptr && shared.picker.data() != picker) {
    shared.picker = picker;
    DocumentHost* host = shared.host;
    picker->setDesignParts([host] { return usedLibraryParts(host->model()); });
    QObject::connect(picker, &PartPicker::chosen, picker, [host](const PickerPart& part, const QString& command) {
      if (!command.startsWith(QStringLiteral("build."))) {
        return; // another caller's
      }
      if (!host->chooseInCommand(command, kPart, part.key)) {
        qInfo().noquote() << QStringLiteral("BUILD picker: %1 not chosen, %2 does not run").arg(part.name, command);
      }
    });
  }
  return picker;
}

// The picker while it shows the parts of `command`, else null.
PartPicker* shownPicker(const Shared& shared, const QString& command) {
  PartPicker* picker = shared.picker.data();
  return picker != nullptr && picker->isVisible() && picker->command() == command ? picker : nullptr;
}

// An open, pinned picker: the command that starts takes it over.
PartPicker* pinnedPicker(Shared& shared) {
  PartPicker* picker = partPicker(shared, false);
  return picker != nullptr && picker->isVisible() && picker->isPinned() ? picker : nullptr;
}

// What a role's command asks the picker for: its catalog's parts (the
// design's sources and library_build_candidates of the role) with their
// keys, the part chosen and the panel's System.
PickerRequest roleRequest(const Shared& shared, const QString& role, const CommandState& state) {
  PickerRequest request;
  request.command = commandOf(role);
  request.title = titleOf(role);
  request.parts = pickerParts(shared.catalog.parts(role));
  request.current = state.choice(kPart);
  request.system = state.choice(kSystem);
  return request;
}

// The part chosen (in the part picker, or live MCP's `part`) sets the
// hidden System (mitcad#427): the part's own system, unless it fits
// several; the picker opens filtered to it, also the next time.
void systemFromPart(Shared& shared, const QString& role, CommandState& state) {
  Part part;
  if (!chosenPart(shared, state, part)) {
    return;
  }
  const QString own = part.systemText();
  if (!own.isEmpty() && !own.contains(QLatin1Char(','))) {
    state.setChoice(kSystem, own);
  }
  shared.lastPart.insert(role, part.key);
  shared.lastSystem.insert(role, state.choice(kSystem));
}

// The picker open for the command shows what its panel chose: the part
// and, with `system`, the system (set through live MCP's `system` or
// `search`; a part chosen in the picker leaves the picker's filter alone).
void syncPicker(const Shared& shared, const QString& role, const CommandState& state, bool system) {
  if (PartPicker* picker = shownPicker(shared, commandOf(role))) {
    if (system) {
      picker->grid()->setSystem(state.choice(kSystem));
    }
    picker->setCurrent(state.choice(kPart));
  }
}

// The tile of the part chosen (mitcad#427): the library's picture of it,
// its name, and its supplier and part number (else its system) under it.
ChoiceTile partTile(Shared& shared, const CommandState& state) {
  Part part;
  if (!chosenPart(shared, state, part)) {
    return {QImage(), QObject::tr("No part"), QString()};
  }
  QString detail = QStringList{part.supplier, part.partNumber}.join(QLatin1Char(' ')).trimmed();
  if (detail.isEmpty()) {
    detail = part.systemText();
  }
  const QString title = part.config.isEmpty() ? part.name : QStringLiteral("%1 (%2)").arg(part.name, part.config);
  return {shared.thumbnail(part), title, detail};
}

QVector<InputDef> pickerInputs(const SharedPtr& shared, const QString& role) {
  // Search and System are not shown (mitcad#427: the part picker has
  // them); they stay as inputs for live MCP: Search chooses the first part
  // its words match, System the first part of that system.
  const auto hidden = [](const CommandState&) { return false; };
  return {
      textInput(kSearch, QObject::tr("Search"))
          .withVisible(hidden)
          .withTooltip(QObject::tr("Words of the part's name, size, system, slots, supplier or part number"))
          .withOnChange([shared, role](CommandState& state, const CommandContext& context) {
            followFilter(*shared, role, state, context);
            syncPicker(*shared, role, state, true);
          }),
      choiceInput(kSystem, QObject::tr("System"), {}, QString())
          .withVisible(hidden)
          .withChoices([shared, role](const CommandState&) { return build::systemChoices(shared->catalog.parts(role)); })
          .withOnChange([shared, role](CommandState& state, const CommandContext& context) {
            followFilter(*shared, role, state, context);
            shared->lastSystem.insert(role, state.choice(kSystem));
            syncPicker(*shared, role, state, true);
          })
          .withTooltip(QObject::tr("The profile system: type, slot width and grid (B8 30: B-type slot 8, 30 mm)")),
      // The part as a tile (mitcad#427): a click on it, or Enter or Space,
      // opens the part picker, whose choice sets it. Every part of the
      // role is a choice (live MCP's `part`).
      choiceInput(kPart, QObject::tr("Part"), {}, QString())
          .withChoices([shared, role](const CommandState& state) {
            return build::partChoices(shared->catalog.parts(role), QString(), QString(), state.choice(kPart));
          })
          .withChoiceTile([shared](const CommandState& state) { return partTile(*shared, state); })
          .withOnChange([shared, role](CommandState& state, const CommandContext& context) {
            followPart(*shared, state, context);
            systemFromPart(*shared, role, state);
            syncPicker(*shared, role, state, false);
          })
          .withButton(kButtonText, buttonTip(),
                      [shared, role](const CommandState& state, const CommandContext&) {
                        if (PartPicker* picker = partPicker(*shared, true)) {
                          picker->ask(roleRequest(*shared, role, state));
                        }
                      })
          .withTooltip(QObject::tr("The part chosen: a click (or Enter) opens the part picker with the library's "
                                   "parts as pictures, your favourites first")),
  };
}

// The command ended: its picker closes unless pinned.
std::function<void(bool)> pickerEnded(const SharedPtr& shared, const QString& command) {
  return [shared, command](bool) {
    if (PartPicker* picker = shared->picker.data()) {
      picker->commandEnded(command);
    }
  };
}

// The picker's parts read again; the part chosen in the panel stays, else
// the one chosen or placed last, else the first.
void startPicker(Shared& shared, const QString& role, CommandState& state, const CommandContext& context) {
  shared.catalog.refresh(role, context);
  shared.refreshDesign(context);
  const QVector<Part> parts = shared.catalog.parts(role);
  // The part picker opens filtered to the system chosen last (mitcad#427).
  if (state.choice(kSystem).isEmpty()) {
    state.setChoice(kSystem, shared.lastSystem.value(role));
  }
  Part known;
  if (!state.choice(kPart).isEmpty() && shared.catalog.find(state.choice(kPart), known)) {
    followPart(shared, state, context);
    return;
  }
  const QString last = shared.lastPart.value(role);
  if (!last.isEmpty() && shared.catalog.find(last, known)) {
    state.setChoice(kPart, last);
  } else if (!parts.isEmpty()) {
    state.setChoice(kPart, parts.first().key);
  }
  followPart(shared, state, context);
}

// Why the picker is empty.
QString noPartsText(const Shared& shared, const QString& role) {
  const QString problem = shared.catalog.problem(role);
  QString text = QObject::tr("No %1 parts: add Mitcad's parts library in Tools > Libraries and fetch it "
                             "(or Update in Insert from Library).")
                     .arg(role);
  if (!problem.isEmpty()) {
    text += QLatin1Char(' ') + problem;
  }
  return text;
}

// --- Placing a part (Profile, Linear Part)
//
// The panel grows as the placement needs it (mitcad#394): at first the
// part, its length and, for a ground placement, its direction (X, Y, Z);
// an end pick adds the corner's direction and the mitre, a side pick the
// offset, a pick on another part rotate and flip; once the design has a
// part to go on, Placement shows as four icon buttons (mitcad#427: no More).
// Only what the placement can use highlights: in a ground placement
// (the design has no part to go on, or Placement "On a plane") planes,
// points and faces that are no BUILD part's, with the origin's planes
// shown and Enter placing the part at the origin; else only the faces of
// the parts it goes on, construction geometry hidden. The parts go into
// the BUILD host (the `build_host` query), shown as "Adds to".

// Placement's icon buttons (mitcad#427), their labels the tooltips.
const Choices kPlaces = {{QStringLiteral("auto"), QStringLiteral("Automatic: where the click is")},
                         {QStringLiteral("side"), QStringLiteral("On a profile's side")},
                         {QStringLiteral("end"), QStringLiteral("On a profile's end")},
                         {QStringLiteral("plane"), QStringLiteral("On a plane (as the first part)")}};
const QHash<QString, QString> kPlaceIcons = {{QStringLiteral("auto"), QStringLiteral("build-place-auto")},
                                             {QStringLiteral("side"), QStringLiteral("build-place-side")},
                                             {QStringLiteral("end"), QStringLiteral("build-place-end")},
                                             {QStringLiteral("plane"), QStringLiteral("build-place-plane")}};
// A ground placement's axis (at.direction): X, Y or Z of the host.
const Choices kAxes = {{QStringLiteral("x"), QStringLiteral("Along X")},
                       {QStringLiteral("y"), QStringLiteral("Along Y")},
                       {QStringLiteral("z"), QStringLiteral("Along Z (standing)")}};
const QHash<QString, QString> kAxisIcons = {{QStringLiteral("x"), QStringLiteral("build-axis-x")},
                                            {QStringLiteral("y"), QStringLiteral("build-axis-y")},
                                            {QStringLiteral("z"), QStringLiteral("build-axis-z")}};
// An end placement's axis (at.direction, mitcad#417): Auto (the click's
// zone of the end face decides), Straight, or one of the four turns square
// to the part it meets (`end.turns` of the preview, "turn0".."turn3").
const QString kAuto = QStringLiteral("auto");
const QString kStraight = QStringLiteral("straight");
const QString kTurn = QStringLiteral("turn");
// What a side placement's offset is measured from (at.from).
const Choices kFroms = {{QStringLiteral("a"), QStringLiteral("End A")},
                        {QStringLiteral("b"), QStringLiteral("End B")},
                        {QStringLiteral("middle"), QStringLiteral("Middle")}};
const Choices kEnds = {{QStringLiteral("a"), QStringLiteral("End A")}, {QStringLiteral("b"), QStringLiteral("End B")}};
// State kept besides the inputs (mitcad#417): whether the side's Offset
// and From were typed (sent as at.offset and at.from; a click leaves them
// to the click), what the preview said of the end picked last (its zone,
// direction, turns and mitre), and the end face under the mouse.
const QString kOffsetTyped = QStringLiteral("offset_typed");
const QString kEndPick = QStringLiteral("end_pick");
const QString kHover = QStringLiteral("hover_end");
// A hover that moved less than this many pixels shows the same.
constexpr double kHoverStepPixels = 4.0;
const QString kTyped = QStringLiteral("typed");
const Choices kLengthModes = {{QStringLiteral("face"), QStringLiteral("Auto: to the face")},
                              {QStringLiteral("centre"), QStringLiteral("Auto: to the centre")},
                              {QStringLiteral("through"), QStringLiteral("Auto: through")},
                              {kTyped, QStringLiteral("Typed length")}};
// The length mode's icon buttons (mitcad#427), their labels the tooltips;
// kLengthModes names the bound length in the inspection.
const Choices kLengthButtons = {{QStringLiteral("face"), QStringLiteral("Auto: to the face it meets")},
                                {QStringLiteral("centre"), QStringLiteral("Auto: to its centre")},
                                {QStringLiteral("through"), QStringLiteral("Auto: through it")},
                                {kTyped, QStringLiteral("Typed length")}};
const QHash<QString, QString> kLengthIcons = {{QStringLiteral("face"), QStringLiteral("build-length-face")},
                                              {QStringLiteral("centre"), QStringLiteral("build-length-centre")},
                                              {QStringLiteral("through"), QStringLiteral("build-length-through")},
                                              {kTyped, QStringLiteral("build-length-typed")}};
const QStringList kAutoModes = {QStringLiteral("face"), QStringLiteral("centre"), QStringLiteral("through")};
// Auto: the model's default (mitcad#427: none on a square corner, as the
// panel's turns are; shared on a corner at another angle at offset 0),
// its label saying which.
const Choices kMitres = {{QStringLiteral("auto"), QStringLiteral("Auto")},
                         {QStringLiteral("none"), QStringLiteral("None")},
                         {QStringLiteral("shared"), QStringLiteral("Shared: both on the bisector")},
                         {QStringLiteral("new"), QStringLiteral("Cut the new part")},
                         {QStringLiteral("existing"), QStringLiteral("Cut the existing part")}};
const Choices kRotations = {{QStringLiteral("0"), QStringLiteral("0 deg")},
                            {QStringLiteral("90"), QStringLiteral("90 deg")},
                            {QStringLiteral("180"), QStringLiteral("180 deg")},
                            {QStringLiteral("270"), QStringLiteral("270 deg")}};
const Choices kTilts = {{QStringLiteral("x"), QStringLiteral("About x")},
                        {QStringLiteral("-x"), QStringLiteral("About x, the other way")},
                        {QStringLiteral("y"), QStringLiteral("About y")},
                        {QStringLiteral("-y"), QStringLiteral("About y, the other way")}};
const QString kAt = QStringLiteral("at");

// How a placement goes now: on the ground (a plane, a face, a point or the
// origin), waiting for a pick on a part, or on a part's side or end.
enum class Against { Ground, Pick, Side, End };

bool isEndFace(const SelectionItem& item) {
  return item.name.endsWith(QStringLiteral(":end_a")) || item.name.endsWith(QStringLiteral(":end_b"));
}

Against against(const Shared& shared, const QString& role, const CommandState& state) {
  const QString place = state.choice(QStringLiteral("place"));
  if (place == QStringLiteral("plane") || (place != QStringLiteral("side") && place != QStringLiteral("end") &&
                                            !shared.hasTargets(role))) {
    return Against::Ground;
  }
  const Selection& items = state.items(kAt);
  const std::optional<BuildPart> part = items.isEmpty() ? std::nullopt : shared.partAt(items.first().occurrence);
  if (!part) {
    return Against::Pick;
  }
  if (place == QStringLiteral("side")) {
    return Against::Side;
  }
  if (place == QStringLiteral("end")) {
    return Against::End;
  }
  return isEndFace(items.first()) ? Against::End : Against::Side;
}

// What the pick input takes now (InputDef::withPickRule).
SelectFilter pickKinds(const Shared& shared, const QString& role, const CommandState& state) {
  if (against(shared, role, state) == Against::Ground) {
    return SelectKind::Face | SelectKind::Plane | SelectKind::Point | SelectKind::Vertex;
  }
  return SelectKind::Face;
}

bool pickTakes(const Shared& shared, const QString& role, const SelectionItem& item, const CommandState& state) {
  const std::optional<BuildPart> part = shared.partAt(item.occurrence);
  if (against(shared, role, state) == Against::Ground) {
    return !part; // planes, points and faces of other components
  }
  return item.kind == SelectKind::Face && part && Shared::targets(role, part->role);
}

// What `build_place` (and its preview) gets from the panel, the auto
// length's target found.
struct Placement {
  QJsonObject fields;
  QString problem;
  QJsonObject target;     // what the auto length runs to, or empty
  QString lengthNote;     // how the length was chosen, for the panel
  Against how = Against::Ground;
};

QString angleValue(const QString& degrees) { return degrees + QStringLiteral(" deg"); }

QJsonArray axisVector(const QString& axis) {
  const double sign = axis.startsWith(QLatin1Char('-')) ? -1.0 : 1.0;
  const QChar letter = axis.isEmpty() ? QLatin1Char('z') : axis.at(axis.size() - 1);
  return QJsonArray{letter == QLatin1Char('x') ? sign : 0.0, letter == QLatin1Char('y') ? sign : 0.0,
                    letter == QLatin1Char('z') ? sign : 0.0};
}

// --- An end's directions (mitcad#417)

double dotOf(const QJsonArray& a, const QJsonArray& b) {
  if (a.size() < 3 || b.size() < 3) {
    return 0.0;
  }
  return a[0].toDouble() * b[0].toDouble() + a[1].toDouble() * b[1].toDouble() + a[2].toDouble() * b[2].toDouble();
}

// The host axis a direction runs along ("+X", "-Y"), else empty.
QString axisName(const QJsonArray& direction) {
  if (direction.size() < 3) {
    return QString();
  }
  const QString letters = QStringLiteral("XYZ");
  for (int i = 0; i < 3; ++i) {
    const double component = direction[i].toDouble();
    if (std::abs(std::abs(component) - 1.0) < 1e-6) {
      return QString(component > 0.0 ? QLatin1Char('+') : QLatin1Char('-')) + letters.at(i);
    }
  }
  return QString();
}

// A turn of an end by the host axis it runs along, else "Side 1".."Side 4".
QString turnName(const QJsonArray& turns, int index) {
  const QString axis = axisName(turns.at(index).toArray());
  return axis.isEmpty() ? QObject::tr("Side %1").arg(index + 1) : axis;
}

// Straight on from an end: the end frame's z, square to its x and y (the
// first two turns), as the preview gives it.
QJsonArray straightOf(const QJsonObject& end) {
  const QJsonArray turns = end.value(QStringLiteral("turns")).toArray();
  if (turns.size() < 2) {
    return str(end, "zone") == kStraight ? end.value(QStringLiteral("direction")).toArray() : QJsonArray();
  }
  const QJsonArray x = turns.at(0).toArray();
  const QJsonArray y = turns.at(1).toArray();
  if (x.size() < 3 || y.size() < 3) {
    return QJsonArray();
  }
  const gp_Vec z = gp_Vec(x[0].toDouble(), x[1].toDouble(), x[2].toDouble())
                       .Crossed(gp_Vec(y[0].toDouble(), y[1].toDouble(), y[2].toDouble()));
  if (z.Magnitude() < 1e-12) {
    return QJsonArray();
  }
  const gp_Vec unit = z.Normalized();
  return QJsonArray{unit.X(), unit.Y(), unit.Z()};
}

// A direction of an end by name: the turn it is, else the axis it runs
// along, else "straight on".
QString directionText(const QJsonArray& direction, const QJsonArray& turns) {
  for (int i = 0; i < turns.size(); ++i) {
    if (dotOf(direction, turns.at(i).toArray()) > 1.0 - 1e-6) {
      return turnName(turns, i);
    }
  }
  const QString axis = axisName(direction);
  return axis.isEmpty() ? QObject::tr("straight on") : axis;
}

// What the preview said of the end picked now (kept by the click), or
// empty when the pick is another one.
QJsonObject endPick(const CommandState& state) {
  const QJsonObject end = state.object(kEndPick);
  const Selection& items = state.items(kAt);
  if (end.isEmpty() || items.isEmpty() || str(end, "item") != items.first().describe() ||
      str(end, "occurrence") != items.first().occurrence) {
    return {};
  }
  return end;
}

// Whether the end placement turns the new part (a corner): a turn chosen,
// or Auto with the click in the band along an edge.
bool turnsCorner(const CommandState& state) {
  const QString direction = state.choice(QStringLiteral("direction"));
  if (direction.startsWith(kTurn)) {
    return true;
  }
  if (direction == kStraight) {
    return false;
  }
  return str(endPick(state), "zone") == QStringLiteral("edge");
}

Choices directionChoices(const CommandState& state) {
  const QJsonObject end = endPick(state);
  const QJsonArray turns = end.value(QStringLiteral("turns")).toArray();
  QString clicked;
  if (!end.isEmpty()) {
    clicked = str(end, "zone") == QStringLiteral("edge")
                  ? directionText(end.value(QStringLiteral("direction")).toArray(), turns)
                  : QObject::tr("straight");
  }
  Choices choices{{kAuto, clicked.isEmpty() ? QObject::tr("Auto: by the click")
                                            : QObject::tr("Auto: by the click (%1)").arg(clicked)},
                  {kStraight, QObject::tr("Straight")}};
  for (int i = 0; i < turns.size() && i < 4; ++i) {
    choices.append({kTurn + QString::number(i), turnName(turns, i)});
  }
  return choices;
}

// The mitre Auto takes: what the click's preview said while Direction and
// the corner's offset are as then, else none: the panel's turns are square
// to the part they meet, and a square corner lies on the end face without
// a mitre (mitcad#427; only a corner at another angle is shared).
QString autoMitre(const CommandState& state) {
  const bool offset = cmd::nonZero(state, QStringLiteral("corner_offset"));
  const QJsonObject end = endPick(state);
  const QString clicked = end.value(QStringLiteral("mitre")).toObject().value(QStringLiteral("cut")).toString();
  if (!clicked.isEmpty() && state.choice(QStringLiteral("direction")) == kAuto && !offset) {
    return clicked;
  }
  return QStringLiteral("none");
}

Choices mitreChoices(const CommandState& state) {
  const bool offset = turnsCorner(state) && cmd::nonZero(state, QStringLiteral("corner_offset"));
  Choices choices;
  for (const auto& option : kMitres) {
    if (option.first == kAuto) {
      choices.append({kAuto, QObject::tr("Auto (%1)").arg(autoMitre(state))});
    } else if (option.first != QStringLiteral("shared") || !offset) {
      // A shared mitre meets only at offset 0.
      choices.append(option);
    }
  }
  return choices;
}

// The panel's fields; with an auto length the preview names its target
// first (commands.md, `build_preview`'s `length.target`), and the length
// binds to it.
Placement placement(const Shared& shared, const QString& role, const CommandState& state,
                    const CommandContext& context, const Part& part, bool profile) {
  Placement result;
  result.how = against(shared, role, state);
  if (result.how == Against::Pick) {
    result.problem = QObject::tr("Click a profile's side or end.");
    return result;
  }
  QJsonObject fields{{QStringLiteral("part"), part.ref()}};
  std::optional<QJsonObject> at = QJsonObject();
  if (!state.items(kAt).isEmpty()) {
    at = build::atOf(kAt, state, context, result.problem);
    if (!at) {
      return result;
    }
  }
  const QString place = state.choice(QStringLiteral("place"));
  // The click's least snap distance on the screen (mitcad#417).
  if (const std::optional<double> snap = build::pickSnap(kAt, state)) {
    at->insert(QStringLiteral("snap"), *snap);
  }
  if (result.how == Against::Ground) {
    // On the picked plane, face or point, else (Enter) at the origin,
    // along X, Y or Z.
    at->insert(QStringLiteral("direction"), axisVector(state.choice(QStringLiteral("axis"))));
  } else {
    at->insert(QStringLiteral("place"), place == QStringLiteral("side") || place == QStringLiteral("end")
                                            ? place
                                            : QStringLiteral("auto"));
    if (result.how == Against::End) {
      // The new part's axis in the host's coordinates: Straight or a turn
      // (a corner) as chosen; Auto leaves it to the click's zone.
      const QString direction = state.choice(QStringLiteral("direction"));
      const QJsonObject end = endPick(state);
      QJsonArray axis;
      if (direction == kStraight) {
        axis = end.value(QStringLiteral("straight")).toArray();
      } else if (direction.startsWith(kTurn)) {
        axis = end.value(QStringLiteral("turns")).toArray().at(direction.mid(kTurn.size()).toInt()).toArray();
      }
      if (axis.size() >= 3) {
        at->insert(QStringLiteral("direction"), axis);
      }
      // A corner's offset: the new part's end from the target's outer side.
      const QString offset = state.expression(QStringLiteral("corner_offset"));
      if (turnsCorner(state) && !offset.isEmpty()) {
        at->insert(QStringLiteral("offset"), offset);
      }
    }
    // A side's offset as typed (or kept by a change of From) from its
    // reference; else the click decides both (mitcad#417).
    if (result.how == Against::Side && state.checked(kOffsetTyped)) {
      at->insert(QStringLiteral("offset"), state.expression(QStringLiteral("offset")));
      at->insert(QStringLiteral("from"), state.choice(QStringLiteral("from")));
    }
    fields.insert(QStringLiteral("rotate"), angleValue(state.choice(QStringLiteral("rotate"))));
    fields.insert(QStringLiteral("flip"), state.checked(QStringLiteral("flip")));
    // Auto (or nothing chosen) leaves the mitre to the model; None says
    // so (a corner at an angle other than 90 degrees defaults to shared).
    const QString mitre = state.choice(QStringLiteral("mitre"));
    if (profile && result.how == Against::End && !mitre.isEmpty() && mitre != kAuto) {
      QJsonObject cut{{QStringLiteral("cut"), mitre}};
      if (mitre == QStringLiteral("new") || mitre == QStringLiteral("existing")) {
        cut.insert(QStringLiteral("angle"), state.expression(QStringLiteral("mitre_angle")));
      }
      fields.insert(QStringLiteral("mitre"), cut);
    }
  }
  fields.insert(kAt, *at);
  if (part.cutToLength()) {
    const QString mode = state.choice(QStringLiteral("length_mode"));
    if (mode == kTyped || result.how == Against::Ground) {
      fields.insert(QStringLiteral("length"), state.expression(QStringLiteral("length")));
      result.lengthNote = QObject::tr("typed");
    } else {
      // The face the ray from the new end meets, as the preview names it.
      QJsonObject probe = fields;
      probe.insert(QStringLiteral("query"), QStringLiteral("build_preview"));
      QJsonObject first;
      try {
        first = context.queryObject(probe);
      } catch (const std::exception& error) {
        result.problem = errorText(error);
        return result;
      }
      result.target = first.value(QStringLiteral("length")).toObject().value(QStringLiteral("target")).toObject();
      if (result.target.isEmpty()) {
        // Nothing in the way: the typed length.
        fields.insert(QStringLiteral("length"), state.expression(QStringLiteral("length")));
        result.lengthNote = QObject::tr("typed: nothing in the way for an auto length");
      } else {
        fields.insert(QStringLiteral("length"), QJsonObject{{QStringLiteral("bind"), result.target},
                                                            {QStringLiteral("mode"), mode},
                                                            {QStringLiteral("add"), 0}});
        result.lengthNote = QObject::tr("bound, %1").arg(cmd::label(kLengthModes, mode).toLower());
      }
    }
  }
  result.fields = fields;
  return result;
}

QString problemsOf(const QJsonObject& answer) {
  const QJsonValue problems = answer.value(QStringLiteral("problems"));
  if (problems.isString()) {
    return problems.toString();
  }
  QStringList texts;
  for (const QJsonValue& problem : problems.toArray()) {
    texts << (problem.isString() ? problem.toString()
                                 : str(problem.toObject(), "message").isEmpty() ? str(problem.toObject(), "reason")
                                                                                : str(problem.toObject(), "message"));
  }
  return texts.join(QStringLiteral(" "));
}

QStringList warningsOf(const QJsonObject& answer) {
  QStringList texts;
  for (const QJsonValue& warning : answer.value(QStringLiteral("warnings")).toArray()) {
    texts << warning.toString();
  }
  return texts;
}

// The preview's markers (previewMarkers): {"markers": [{label, point}]}.
std::vector<PreviewMarker> markersOf(const QJsonObject& report) {
  std::vector<PreviewMarker> markers;
  for (const QJsonValue& value : report.value(QStringLiteral("markers")).toArray()) {
    const QJsonObject marker = value.toObject();
    const QJsonArray point = marker.value(QStringLiteral("point")).toArray();
    if (point.size() >= 3) {
      markers.push_back({str(marker, "label"), gp_Pnt(point[0].toDouble(), point[1].toDouble(), point[2].toDouble())});
    }
  }
  return markers;
}

QJsonObject marker(const QString& label, const gp_Pnt& point) {
  return {{QStringLiteral("label"), label}, {QStringLiteral("point"), build::pointJson(point)}};
}

std::optional<gp_Pnt> pointOf(const QJsonValue& value) {
  const QJsonArray point = value.toArray();
  if (point.size() < 3) {
    return std::nullopt;
  }
  return gp_Pnt(point[0].toDouble(), point[1].toDouble(), point[2].toDouble());
}

// A side snap's name in the view: "flush A", "middle", ...
QString snapText(const QString& kind) {
  if (kind == QStringLiteral("end_a")) {
    return QObject::tr("flush A");
  }
  if (kind == QStringLiteral("end_b")) {
    return QObject::tr("flush B");
  }
  if (kind == QStringLiteral("middle")) {
    return QObject::tr("middle");
  }
  if (kind == QStringLiteral("grid")) {
    return QObject::tr("grid");
  }
  if (kind == QStringLiteral("clamped")) {
    return QObject::tr("kept on the part");
  }
  return kind;
}

QString targetText(const QJsonObject& target, const CommandContext& context) {
  const std::optional<SelectionItem> item = build::itemOfTarget(target, context);
  const QString occurrence = build::rootPath(str(target, "occurrence"), context);
  const QString component = build::componentAt(occurrence, context);
  const QString name = component.isEmpty() ? occurrence : build::componentName(component, context);
  return item ? QObject::tr("%1 of %2").arg(item->name.isEmpty() ? item->owner : item->name, name) : name;
}

QString endsText(const QJsonObject& ends) {
  QStringList shown;
  for (const char* end : {"a", "b"}) {
    const QJsonObject cut = ends.value(QLatin1String(end)).toObject();
    // Angles in radians, as the replies give them.
    const double angle = cut.value(QStringLiteral("angle")).toDouble();
    if (!std::isfinite(angle) || std::abs(angle) < 1e-9) {
      continue;
    }
    shown << QStringLiteral("%1 %2 deg%3")
                 .arg(QString::fromLatin1(end).toUpper(), QString::number(angle * 180.0 / kPi, 'g', 4),
                      str(cut, "tilt").isEmpty() ? QString() : QStringLiteral(" (%1)").arg(str(cut, "tilt")));
  }
  return shown.isEmpty() ? QObject::tr("square ends") : QObject::tr("ends %1").arg(shown.join(QStringLiteral(", ")));
}

// A side's reference in a sentence: "end A", "end B", "the middle".
QString fromText(const QString& from) {
  if (from == QStringLiteral("b")) {
    return QObject::tr("end B");
  }
  if (from == QStringLiteral("middle")) {
    return QObject::tr("the middle");
  }
  return QObject::tr("end A");
}

// The side placement of a preview's answer (`side`, mitcad#417), with the
// target's length from the snaps (end B's near face is the length less the
// width from end A), else from the target's own length.
Shared::Side sideOf(const QJsonObject& answer, double targetLength) {
  const QJsonObject side = answer.value(QStringLiteral("side")).toObject();
  Shared::Side shown;
  if (side.isEmpty()) {
    return shown;
  }
  shown.from = str(side, "from").isEmpty() ? QStringLiteral("a") : str(side, "from");
  shown.offset = side.value(QStringLiteral("offset")).toDouble(kNaN);
  shown.width = side.value(QStringLiteral("width")).toDouble(kNaN);
  shown.length = targetLength;
  for (const QJsonValue& value : answer.value(QStringLiteral("snaps")).toArray()) {
    const QJsonObject snap = value.toObject();
    const double offset = snap.value(QStringLiteral("offset")).toDouble(kNaN);
    if (str(snap, "kind") == QStringLiteral("end_b") && std::isfinite(offset) && std::isfinite(shown.width)) {
      shown.length = offset + shown.width;
    }
  }
  return shown;
}

// The state a click at the hover would make (mitcad#417): the end face
// under the mouse picked there, its Direction Auto. None without a hover.
std::optional<CommandState> hoverState(const CommandState& state) {
  const QJsonObject hover = state.object(kHover);
  if (hover.isEmpty()) {
    return std::nullopt;
  }
  CommandState shown = state;
  const SelectionItem item{SelectKind::Face, str(hover, "owner"), str(hover, "name"), str(hover, "geometry"),
                           str(hover, "occurrence")};
  shown.setItems(kAt, {item});
  QJsonObject pick{{QStringLiteral("item"), item.describe()}, {QStringLiteral("point"), hover.value(QStringLiteral("point"))}};
  if (hover.contains(QStringLiteral("snap"))) {
    pick.insert(QStringLiteral("snap"), hover.value(QStringLiteral("snap")));
  }
  shown.setObject(kAt + QStringLiteral("_pick"), pick);
  shown.setChoice(QStringLiteral("direction"), kAuto);
  shown.setObject(kEndPick, {});
  return shown;
}

// The hover on an end face of a part the new one goes on (mitcad#417),
// kept with where the mouse is, so that the preview shows what a click
// there places, straight on or turned, before the click. True when the
// hover changed; a move of a few pixels shows the same.
bool hoverEnd(const Shared& shared, const QString& role, const SelectionItem& item, CommandState& state,
              const CommandContext& context) {
  const QJsonObject before = state.object(kHover);
  const QString place = state.choice(QStringLiteral("place"));
  bool takes = item.isValid() && item.kind == SelectKind::Face && item.at && isEndFace(item) &&
               (place.isEmpty() || place == kAuto || place == QStringLiteral("end")) && shared.hasTargets(role);
  if (takes) {
    const std::optional<BuildPart> part = shared.partAt(item.occurrence);
    takes = part && Shared::targets(role, part->role);
  }
  QJsonObject record = takes ? build::pickRecord(item, context) : QJsonObject();
  if (record.isEmpty()) {
    if (before.isEmpty()) {
      return false;
    }
    state.setObject(kHover, {});
    qInfo().noquote() << QStringLiteral("BUILD hover off the end faces");
    return true;
  }
  if (str(before, "item") == str(record, "item") && str(before, "occurrence") == item.occurrence) {
    const std::optional<gp_Pnt> was = pointOf(before.value(QStringLiteral("point")));
    const std::optional<gp_Pnt> now = pointOf(record.value(QStringLiteral("point")));
    const double step = record.value(QStringLiteral("snap")).toDouble(0.0) * kHoverStepPixels / build::kSnapPixels;
    if (was && now && was->Distance(*now) < step) {
      return false;
    }
  }
  record.insert(QStringLiteral("owner"), item.owner);
  record.insert(QStringLiteral("name"), item.name);
  record.insert(QStringLiteral("geometry"), item.geometry);
  record.insert(QStringLiteral("occurrence"), item.occurrence);
  if (before.isEmpty() || str(before, "item") != str(record, "item")) {
    qInfo().noquote() << QStringLiteral("BUILD hover on %1").arg(item.describe());
  }
  state.setObject(kHover, record);
  return true;
}

// The end face's zones as markers (mitcad#417): "straight" in its middle,
// "turn <name>" in the middle of the band along each edge a click there
// turns towards; the one the placement takes marked "> ". Nothing when the
// view has no shape for the face.
void addEndMarkers(QJsonArray& markers, const QJsonObject& end, const SelectionItem& face,
                   const CommandContext& context) {
  const QJsonArray turns = end.value(QStringLiteral("turns")).toArray();
  const TopoDS_Shape shape = turns.size() == 4 ? context.itemShape(face) : TopoDS_Shape();
  if (shape.IsNull()) {
    return;
  }
  std::vector<gp_Pnt> corners;
  for (TopExp_Explorer vertices(shape, TopAbs_VERTEX); vertices.More(); vertices.Next()) {
    corners.push_back(BRep_Tool::Pnt(TopoDS::Vertex(vertices.Current())));
  }
  if (corners.empty()) {
    return;
  }
  gp_XYZ sum(0.0, 0.0, 0.0);
  for (const gp_Pnt& corner : corners) {
    sum += corner.XYZ();
  }
  const gp_Pnt middle(sum / static_cast<double>(corners.size()));
  // The turns are in the host's coordinates, the face in the design's.
  gp_Trsf toDesign;
  if (const std::optional<gp_Trsf> placed = build::worldOf(build::buildHost(context).path, context)) {
    toDesign = *placed;
  }
  const QJsonArray direction = end.value(QStringLiteral("direction")).toArray();
  const bool turned = str(end, "zone") != kStraight && dotOf(direction, straightOf(end)) < 1.0 - 1e-6;
  markers.append(marker(turned ? QObject::tr("straight") : QObject::tr("> straight"), middle));
  for (int i = 0; i < 4; ++i) {
    const QJsonArray turn = turns.at(i).toArray();
    if (turn.size() < 3) {
      continue;
    }
    const gp_Vec along = gp_Vec(turn[0].toDouble(), turn[1].toDouble(), turn[2].toDouble()).Transformed(toDesign);
    if (along.Magnitude() < 1e-12) {
      continue;
    }
    const gp_Vec unit = along.Normalized();
    double reach = 0.0;
    for (const gp_Pnt& corner : corners) {
      reach = std::max(reach, gp_Vec(middle, corner).Dot(unit));
    }
    const bool taken = turned && dotOf(direction, turn) > 1.0 - 1e-6;
    markers.append(marker(QStringLiteral("%1turn %2").arg(taken ? QStringLiteral("> ") : QString(), turnName(turns, i)),
                          middle.Translated(unit * (0.75 * reach))));
  }
}

// The inspection of a placement: what build_preview says, the new part's
// shape (the model's, else its section along the frame), the auto
// length's target highlighted, the length at the free end, the side's
// snaps and an end face's zones, where the part goes ("Adds to") and the
// first part's hint; with the mouse on an end face, what a click there
// places (mitcad#417).
Inspection inspectPlacement(const SharedPtr& shared, const QString& role, const CommandState& picked,
                            const CommandContext& context, bool profile) {
  Inspection result;
  Part part;
  if (!chosenPart(*shared, picked, part)) {
    result.error = noPartsText(*shared, role);
    return result;
  }
  // An end face under the mouse shows what a click there places
  // (mitcad#417); else the pick.
  const std::optional<CommandState> hovered = hoverState(picked);
  const CommandState& state = hovered ? *hovered : picked;
  const build::Host host = build::buildHost(context);
  const QString addsTo = QObject::tr("Adds to: %1").arg(host.name);
  const Placement placed = placement(*shared, role, state, context, part, profile);
  if (placed.how == Against::Ground && state.items(kAt).isEmpty()) {
    // The first part: a click or Enter (mitcad#394).
    result.viewHint = QObject::tr("Click a plane or a face, or press Enter");
  } else if (placed.how == Against::Pick) {
    result.viewHint = QObject::tr("Click a profile's side or end");
  }
  if (!placed.problem.isEmpty()) {
    result.text = QStringLiteral("<b>%1</b><br>%2").arg(part.name.toHtmlEscaped(), addsTo.toHtmlEscaped());
    result.error = placed.problem;
    result.log = QStringLiteral("waiting: %1; adds to %2; part %3").arg(placed.problem, host.name, part.name);
    return result;
  }
  QJsonObject request = placed.fields;
  request.insert(QStringLiteral("query"), QStringLiteral("build_preview"));
  QJsonObject answer;
  try {
    answer = context.queryObject(request);
  } catch (const std::exception& error) {
    result.error = errorText(error);
    return result;
  }
  const build::Frame frame = build::frameOf(answer.value(QStringLiteral("frame")));
  double length = build::lengthOf(answer.value(QStringLiteral("length")));
  if (!std::isfinite(length) && placed.target.isEmpty()) {
    length = state.value(QStringLiteral("length"));
  }
  shared->setPreview(frame, length, !placed.target.isEmpty());
  // The model's shape of the new part, else its section along the axis.
  QJsonObject shapeRequest = placed.fields;
  shapeRequest.insert(QStringLiteral("shape"), QStringLiteral("build_preview"));
  result.shape = context.analysisShape(shapeRequest);
  if (result.shape.IsNull() && part.sectionWidth() > 0.0) {
    result.shape = build::boxAlong(frame, part.sectionWidth(), part.sectionHeight(), length);
  }
  if (!placed.target.isEmpty()) {
    if (const std::optional<SelectionItem> target = build::itemOfTarget(placed.target, context)) {
      result.highlights.append(*target);
    } else if (const QString path = build::rootPath(str(placed.target, "occurrence"), context); !path.isEmpty()) {
      // A frame of the part in the way: the part highlighted.
      result.highlights.append(SelectionItem{SelectKind::Component, QString(), QString(), QString(), path});
    }
  }
  const QString place = str(answer, "place");
  const QJsonObject ends = answer.value(QStringLiteral("ends")).toObject();
  const QJsonObject side = answer.value(QStringLiteral("side")).toObject();
  const QJsonObject end = answer.value(QStringLiteral("end")).toObject();
  QStringList lines;
  lines << QStringLiteral("<b>%1</b>").arg(part.name.toHtmlEscaped());
  const QString seller = QStringList{part.supplier, part.partNumber}.join(QLatin1Char(' ')).trimmed();
  if (!seller.isEmpty()) {
    lines << seller.toHtmlEscaped();
  }
  lines << addsTo.toHtmlEscaped();
  if (!place.isEmpty()) {
    lines << (place == QStringLiteral("end")        ? QObject::tr("On the end")
              : place == QStringLiteral("side")     ? QObject::tr("On the side")
              : place == QStringLiteral("frame")    ? QObject::tr("On the part's frame")
              : place == QStringLiteral("grounded") ? QObject::tr("At the origin")
                                                    : QObject::tr("On the picked geometry"));
  }
  // On a side (mitcad#417): the offset from the reference the placement is
  // bound to, which a change of From keeps in place.
  QString sideLog;
  if (!side.isEmpty()) {
    const Shared::Side shown = sideOf(answer, [&] {
      const Selection& items = state.items(kAt);
      const std::optional<BuildPart> target = items.isEmpty() ? std::nullopt : shared->partAt(items.first().occurrence);
      return target ? target->length : kNaN;
    }());
    if (!hovered) {
      shared->setSide(shown);
    }
    const QString kind = str(side, "kind");
    const QString from = fromText(shown.from);
    lines << (kind.isEmpty() ? QObject::tr("%1 mm from %2").arg(shown.offset, 0, 'f', 1).arg(from)
                             : QObject::tr("%1 mm from %2 (%3)").arg(shown.offset, 0, 'f', 1).arg(from, snapText(kind)));
    sideLog = QStringLiteral("; snap %1 at %2 mm from %3")
                  .arg(kind.isEmpty() ? QStringLiteral("none") : kind)
                  .arg(shown.offset, 0, 'f', 1)
                  .arg(shown.from);
  }
  // On an end (mitcad#417): straight on or turned, a corner's offset and
  // the mitre the placement takes.
  QString endLog;
  if (!end.isEmpty()) {
    const QJsonArray turns = end.value(QStringLiteral("turns")).toArray();
    const QString towards = directionText(end.value(QStringLiteral("direction")).toArray(), turns);
    const QJsonObject mitre = end.value(QStringLiteral("mitre")).toObject();
    const QString cut = str(mitre, "cut").isEmpty() ? QStringLiteral("none") : str(mitre, "cut");
    const QJsonValue offset = end.value(QStringLiteral("offset"));
    const bool straight = dotOf(end.value(QStringLiteral("direction")).toArray(), straightOf(end)) > 1.0 - 1e-6;
    lines << (straight ? QObject::tr("Straight on (%1), mitre %2").arg(towards, cut)
                       : QObject::tr("Turned to %1 at %2 mm from the outer side, mitre %3")
                             .arg(towards)
                             .arg(offset.toDouble(), 0, 'f', 1)
                             .arg(cut));
    endLog = QStringLiteral("; end %1 towards %2, mitre %3%4")
                 .arg(str(end, "zone"), towards, cut,
                      offset.isDouble() ? QStringLiteral(", offset %1 mm").arg(offset.toDouble(), 0, 'f', 1)
                                        : QString());
  }
  if (part.cutToLength()) {
    QString shown = std::isfinite(length) ? QObject::tr("Length %1 mm").arg(length, 0, 'f', 1) : QObject::tr("Length");
    if (!placed.target.isEmpty()) {
      shown += QObject::tr(", to %1 (Tab: face, centre, through; typing a length detaches it)")
                   .arg(targetText(placed.target, context).toHtmlEscaped());
    } else if (!placed.lengthNote.isEmpty()) {
      shown += QStringLiteral(" (%1)").arg(placed.lengthNote.toHtmlEscaped());
    }
    lines << shown;
    lines << endsText(ends);
  }
  if (hovered) {
    lines << QObject::tr("<i>Where the mouse is: click to place it there</i>");
  }
  result.text = lines.join(QStringLiteral("<br>"));
  result.error = problemsOf(answer);
  const QJsonValue snap = placed.fields.value(kAt).toObject().value(QStringLiteral("snap"));
  result.log = QStringLiteral("place %1, length %2 mm (%3%4), %5; adds to %6%7%8")
                   .arg(place.isEmpty() ? QStringLiteral("first") : place,
                        std::isfinite(length) ? QString::number(length, 'f', 2) : QStringLiteral("?"),
                        placed.lengthNote,
                        placed.target.isEmpty() ? QString()
                                                : QStringLiteral(" to ") + targetText(placed.target, context),
                        endsText(ends), host.name, sideLog, endLog);
  if (snap.isDouble()) {
    result.log += QStringLiteral("; least snap %1 mm").arg(snap.toDouble(), 0, 'f', 2);
  }
  if (placed.how == Against::Ground) {
    result.log += QStringLiteral("; along %1").arg(state.choice(QStringLiteral("axis")));
  }
  if (hovered) {
    result.log += QStringLiteral("; on the hover");
  }
  // The part previewed (mitcad#418: a part chosen in the picker).
  result.log += QStringLiteral("; part %1").arg(part.name);
  // The length at the free end, and the side's snaps: the ends and the
  // middle, the one taken marked; an end face's zones.
  QJsonArray markers;
  if (frame.valid && std::isfinite(length) && part.cutToLength()) {
    markers.append(marker(QStringLiteral("%1 mm").arg(length, 0, 'f', 1),
                          frame.origin.Translated(gp_Vec(frame.z) * length)));
  }
  for (const QJsonValue& value : answer.value(QStringLiteral("snaps")).toArray()) {
    const QJsonObject snapped = value.toObject();
    const QString kind = str(snapped, "kind");
    const std::optional<gp_Pnt> point = pointOf(snapped.value(QStringLiteral("point")));
    if (point && kind != QStringLiteral("grid") && kind != str(side, "kind")) {
      markers.append(marker(snapText(kind), *point));
    }
  }
  if (const std::optional<gp_Pnt> point = pointOf(side.value(QStringLiteral("point")));
      point && !str(side, "kind").isEmpty()) {
    markers.append(marker(QStringLiteral("> %1").arg(snapText(str(side, "kind"))), *point));
  }
  if (!end.isEmpty() && !state.items(kAt).isEmpty() && isEndFace(state.items(kAt).first())) {
    addEndMarkers(markers, end, state.items(kAt).first(), context);
  }
  result.report = {{QStringLiteral("markers"), markers}};
  // The box at the cursor shows the auto length while it is bound; typing
  // there detaches it (the length input's onChange).
  if (!placed.target.isEmpty() && std::isfinite(length)) {
    result.report.insert(QStringLiteral("cursor"),
                         QJsonObject{{QStringLiteral("length"), cmd::lengthText(std::round(length * 10.0) / 10.0, context)}});
  }
  return result;
}

bool shownForCut(const Shared& shared, const CommandState& state) {
  Part part;
  return chosenPart(shared, state, part) && part.cutToLength();
}

// A click on a part fills the panel from what the preview makes of it
// (mitcad#417): on a side From and Offset with the reference and the
// offset the click resolved to (a flush end, the middle, else the nearer
// end), left to the click until typed; on an end Direction's choices (the
// end's turns), Direction Auto again.
void fillFromClick(const Shared& shared, const QString& role, bool profile, CommandState& state,
                   const CommandContext& context) {
  const Against how = against(shared, role, state);
  Part part;
  if ((how != Against::Side && how != Against::End) || !chosenPart(shared, state, part)) {
    return;
  }
  if (how == Against::End) {
    state.setChoice(QStringLiteral("direction"), kAuto);
  }
  // Asked with the mitre Auto: the answer's mitre is what Auto takes.
  CommandState probe = state;
  probe.setChoice(QStringLiteral("mitre"), kAuto);
  const Placement placed = placement(shared, role, probe, context, part, profile);
  if (!placed.problem.isEmpty()) {
    return;
  }
  QJsonObject request = placed.fields;
  request.insert(QStringLiteral("query"), QStringLiteral("build_preview"));
  QJsonObject answer;
  try {
    answer = context.queryObject(request);
  } catch (const std::exception& error) {
    qInfo().noquote() << QStringLiteral("BUILD click not placed: %1").arg(errorText(error));
    return;
  }
  if (how == Against::Side) {
    const QJsonObject side = answer.value(QStringLiteral("side")).toObject();
    const QJsonValue offset = side.value(QStringLiteral("offset"));
    if (str(side, "from").isEmpty() || !offset.isDouble()) {
      return;
    }
    state.setChoice(QStringLiteral("from"), str(side, "from"));
    state.setText(QStringLiteral("offset"), cmd::lengthText(std::round(offset.toDouble() * 100.0) / 100.0, context));
    state.setValue(QStringLiteral("offset"), kNaN);
    qInfo().noquote() << QStringLiteral("BUILD side click: From %1, Offset %2 (%3)")
                             .arg(cmd::label(kFroms, str(side, "from")), state.text(QStringLiteral("offset")),
                                  str(side, "kind").isEmpty() ? QStringLiteral("between snaps") : str(side, "kind"));
    return;
  }
  QJsonObject end = answer.value(QStringLiteral("end")).toObject();
  if (end.isEmpty()) {
    return;
  }
  const SelectionItem& item = state.items(kAt).first();
  end.insert(QStringLiteral("item"), item.describe());
  end.insert(QStringLiteral("occurrence"), item.occurrence);
  end.insert(QStringLiteral("straight"), straightOf(end));
  state.setObject(kEndPick, end);
  const QJsonArray turns = end.value(QStringLiteral("turns")).toArray();
  QStringList names;
  for (int i = 0; i < turns.size(); ++i) {
    names << turnName(turns, i);
  }
  qInfo().noquote() << QStringLiteral("BUILD end click: %1 towards %2, mitre %3; turns %4")
                           .arg(str(end, "zone"), directionText(end.value(QStringLiteral("direction")).toArray(), turns),
                                str(end.value(QStringLiteral("mitre")).toObject(), "cut"),
                                names.join(QStringLiteral(", ")));
}

// From changed (mitcad#417): the part stays where it is, its offset from
// the new reference recomputed from the side shown last: from end A the
// near face, from end B the far face (towards A), from the middle the
// part's centre (towards B). Sent as typed from then on.
void keepPlace(const Shared& shared, CommandState& state, const CommandContext& context) {
  const Shared::Side side = shared.shownSide();
  const QString to = state.choice(QStringLiteral("from"));
  state.setChecked(kOffsetTyped, true);
  const double width = side.width;
  const double length = side.length;
  const bool known = std::isfinite(side.offset) && std::isfinite(width) &&
                     (std::isfinite(length) || (side.from == QStringLiteral("a") && to == QStringLiteral("a")));
  if (side.from.isEmpty() || side.from == to || !known) {
    return;
  }
  // The near face from end A, then the offset from the new reference.
  const double nearFace = side.from == QStringLiteral("b")        ? length - side.offset - width
                          : side.from == QStringLiteral("middle") ? length / 2.0 + side.offset - width / 2.0
                                                                  : side.offset;
  const double offset = to == QStringLiteral("b")        ? length - nearFace - width
                        : to == QStringLiteral("middle") ? nearFace + width / 2.0 - length / 2.0
                                                         : nearFace;
  state.setText(QStringLiteral("offset"), cmd::lengthText(std::round(offset * 100.0) / 100.0, context));
  state.setValue(QStringLiteral("offset"), kNaN);
  qInfo().noquote() << QStringLiteral("BUILD From %1 keeps the part in place: Offset %2")
                           .arg(cmd::label(kFroms, to), state.text(QStringLiteral("offset")));
}

// The inputs where and how a part goes, after the picker.
QVector<InputDef> placementInputs(const SharedPtr& shared, const QString& role, bool profile) {
  const auto cut = [shared](const CommandState& state) { return shownForCut(*shared, state); };
  const auto how = [shared, role](const CommandState& state) { return against(*shared, role, state); };
  const auto ground = [how](const CommandState& state) { return how(state) == Against::Ground; };
  const auto onSide = [how](const CommandState& state) { return how(state) == Against::Side; };
  const auto onEnd = [how](const CommandState& state) { return how(state) == Against::End; };
  const auto onPart = [how](const CommandState& state) {
    return how(state) == Against::Side || how(state) == Against::End;
  };
  const auto cornered = [onEnd](const CommandState& state) { return onEnd(state) && turnsCorner(state); };
  QVector<InputDef> inputs = {
      selectionInput(kAt, QObject::tr("Place On"),
                     SelectKind::Face | SelectKind::Plane | SelectKind::Point | SelectKind::Vertex, 0, 1)
          .withOnPick([shared, role, profile](const SelectionItem& item, CommandState& state,
                                              const CommandContext& context) {
            // A new click: where it was, its snap distance, and what it
            // places fills the panel (mitcad#417). A click on the face
            // picked already places it again there (near the other end, on
            // another zone of the end face) rather than taking it out.
            build::recordPick(kAt, item, state, context);
            state.setObject(kHover, {});
            state.setChecked(kOffsetTyped, false);
            fillFromClick(*shared, role, profile, state, context);
          },
                      true)
          .withPickRule([shared, role](const CommandState& state) { return pickKinds(*shared, role, state); },
                        [shared, role](const SelectionItem& item, const CommandState& state) {
                          return pickTakes(*shared, role, item, state);
                        })
          .withHiddenConstruction()
          .withTooltip(QObject::tr("The first part: a plane, a point or a face, or none (Enter: at the origin). "
                                   "Then a profile: near an end on its side, or its end face")),
      // Icon buttons (mitcad#427), each one's tooltip naming it; Tab steps
      // face, centre, through.
      choiceInput(QStringLiteral("length_mode"), QObject::tr("Length"), kLengthButtons, QStringLiteral("face"))
          .withChoiceIcons(kLengthIcons)
          .withVisible([cut, ground](const CommandState& state) { return cut(state) && !ground(state); })
          .withTabCycle(kAutoModes)
          .withOnChange([shared](CommandState& state, const CommandContext& context) {
            // Typed from where the auto length is.
            if (state.choice(QStringLiteral("length_mode")) == kTyped) {
              const double shown = shared->preview().second;
              if (std::isfinite(shown) && shown > 0.0) {
                state.setText(QStringLiteral("length"), cmd::lengthText(std::round(shown * 10.0) / 10.0, context));
                state.setValue(QStringLiteral("length"), kNaN);
              }
            }
          }),
      valueInput(QStringLiteral("length"), QObject::tr("Length"), ValueKind::Length, QString())
          .withVisible(cut)
          .withCursorBox()
          .withOnChange([](CommandState& state, const CommandContext&) {
            // Typing a length detaches it from the auto length.
            if (state.choice(QStringLiteral("length_mode")) != kTyped) {
              state.setChoice(QStringLiteral("length_mode"), kTyped);
              qInfo().noquote() << QStringLiteral("BUILD length typed: detached from the auto length");
            }
          })
          .withManipulator([shared](const CommandState& state, const CommandContext&) -> std::optional<Manipulator> {
            // A handle at the free end while the length is a number (typed,
            // or nothing in the way): dragging it types the length.
            if (state.choice(QStringLiteral("length_mode")) != kTyped && shared->previewBound()) {
              return std::nullopt;
            }
            const build::Frame frame = shared->preview().first;
            if (!frame.valid) {
              return std::nullopt;
            }
            return cmd::arrow(frame.origin, frame.z);
          })
          .withTooltip(QObject::tr("The part's length: its stock length or 500 mm at first; typed here or in the "
                                   "box at the cursor, or dragged by its handle at the free end")),
      choiceInput(QStringLiteral("axis"), QObject::tr("Direction"), kAxes, QStringLiteral("z"))
          .withChoiceIcons(kAxisIcons)
          .withVisible(ground)
          .withTooltip(QObject::tr("The axis the part runs along from where it goes")),
      // On an end (mitcad#417): the click's zone of the end face decides
      // (its inner half straight on, the band along an edge turned towards
      // that edge), or as chosen.
      choiceInput(QStringLiteral("direction"), QObject::tr("Direction"),
                  Choices{{kAuto, QObject::tr("Auto: by the click")}}, kAuto)
          .withChoices([](const CommandState& state) { return directionChoices(state); })
          .withVisible(onEnd)
          .withTooltip(QObject::tr("On an end: by the click (the middle of the end face straight on, near an edge "
                                   "turned towards it), straight on, or turned to a side of the part it meets "
                                   "(a corner)")),
      valueInput(QStringLiteral("corner_offset"), QObject::tr("Offset"), ValueKind::Length, QStringLiteral("0 mm"))
          .withVisible(cornered)
          .withOnChange([](CommandState& state, const CommandContext&) {
            // A shared mitre meets only at offset 0.
            if (state.choice(QStringLiteral("mitre")) == QStringLiteral("shared") &&
                cmd::nonZero(state, QStringLiteral("corner_offset"))) {
              state.setChoice(QStringLiteral("mitre"), kAuto);
              qInfo().noquote() << QStringLiteral("BUILD corner offset: the shared mitre goes (it needs offset 0)");
            }
          })
          .withTooltip(QObject::tr("On a corner: the new part's end from the outer side of the part it meets (the "
                                   "side away from the turn), positive towards the turn; 0 is flush")),
  };
  if (profile) {
    const auto mitred = [onEnd](const CommandState& state) {
      const QString mitre = state.choice(QStringLiteral("mitre"));
      return onEnd(state) && (mitre == QStringLiteral("new") || mitre == QStringLiteral("existing"));
    };
    inputs.append(choiceInput(QStringLiteral("mitre"), QObject::tr("Mitre"), kMitres, kAuto)
                      .withChoices([](const CommandState& state) { return mitreChoices(state); })
                      .withVisible([cut, onEnd](const CommandState& state) { return cut(state) && onEnd(state); })
                      .withTooltip(QObject::tr("On an end: Auto (none: a square corner lies on the end face), none, "
                                               "both on the bisector (shared), the new part or the existing one")));
    inputs.append(valueInput(QStringLiteral("mitre_angle"), QObject::tr("Mitre Angle"), ValueKind::Angle,
                             QStringLiteral("45 deg"))
                      .withVisible(mitred));
  }
  // On a side (mitcad#417): the click fills both; a typed offset moves
  // the part from its reference, a change of From keeps it in place.
  inputs.append(valueInput(QStringLiteral("offset"), QObject::tr("Offset"), ValueKind::Length, QStringLiteral("0 mm"))
                    .withVisible(onSide)
                    .withOnChange([](CommandState& state, const CommandContext&) {
                      if (!state.checked(kOffsetTyped)) {
                        state.setChecked(kOffsetTyped, true);
                        qInfo().noquote() << QStringLiteral("BUILD side offset typed: from %1")
                                                 .arg(fromText(state.choice(QStringLiteral("from"))));
                      }
                    })
                    .withTooltip(QObject::tr("From End A to the new part's near face, from End B to its far face "
                                             "(towards A), from the Middle to its centre (towards B); 0 is flush, "
                                             "or centred")));
  inputs.append(choiceInput(QStringLiteral("from"), QObject::tr("From"), kFroms, QStringLiteral("a"))
                    .withVisible(onSide)
                    .withOnChange([shared](CommandState& state, const CommandContext& context) {
                      keepPlace(*shared, state, context);
                    })
                    .withTooltip(QObject::tr("What the offset is measured from; the part stays bound to it when "
                                             "the part it is on changes length")));
  inputs.append(choiceInput(QStringLiteral("rotate"), QObject::tr("Rotate"), kRotations, QStringLiteral("0"))
                    .withVisible(onPart)
                    .withTooltip(QObject::tr("About the part's axis, in 90 degree steps")));
  inputs.append(flipInput(QStringLiteral("flip"), QObject::tr("Flip")).withVisible(onPart));
  // Placement as icon buttons (mitcad#427: shown directly, no More), once
  // the design has a part to go on; each button's tooltip names it.
  inputs.append(choiceInput(QStringLiteral("place"), QObject::tr("Placement"), kPlaces, QStringLiteral("auto"))
                    .withChoiceIcons(kPlaceIcons)
                    .withVisible([shared, role](const CommandState&) { return shared->hasTargets(role); })
                    .withOnChange([shared, role](CommandState& state, const CommandContext&) {
                      // What the pick takes changes with it: a pick it no
                      // longer takes is picked again.
                      const Selection& items = state.items(kAt);
                      if (!items.isEmpty() && !pickTakes(*shared, role, items.first(), state)) {
                        state.setItems(kAt, {});
                        state.setObject(QStringLiteral("at_pick"), {});
                      }
                    }));
  return inputs;
}

CommandDef placeCommand(const SharedPtr& shared, DocumentHost& host, const QString& role) {
  const bool profile = role == QStringLiteral("profile");
  CommandDef def;
  def.id = profile ? QStringLiteral("build.profile") : QStringLiteral("build.linear");
  def.name = profile ? QObject::tr("Profile") : QObject::tr("Linear Part");
  def.icon = profile ? QStringLiteral("build-profile") : QStringLiteral("build-linear");
  def.tooltip = profile ? QObject::tr("Places slot profiles: pick one, click where it goes (the first one: a plane, "
                                      "or Enter at the origin), its length runs to the part in the way; Enter "
                                      "places it and the next one follows")
                        : QObject::tr("Places rails, carriages, bearings and motor plates: a carriage on its rail "
                                      "slides along it");
  def.tab = kTab;
  def.group = QStringLiteral("PROFILE");
  def.pinned = true;
  def.keywords = profile ? QStringList{QStringLiteral("aluminium"), QStringLiteral("extrusion"), QStringLiteral("frame"),
                                       QStringLiteral("slot"), QStringLiteral("build")}
                         : QStringList{QStringLiteral("rail"), QStringLiteral("carriage"), QStringLiteral("guide"),
                                       QStringLiteral("bearing"), QStringLiteral("nema"), QStringLiteral("build")};
  def.okLabel = QObject::tr("Place");
  def.repeats = true;
  def.inputs = pickerInputs(shared, role);
  def.inputs += placementInputs(shared, role, profile);
  def.init = [shared, role](CommandState& state, const CommandContext& context) {
    startPicker(*shared, role, state, context);
    qInfo().noquote() << QStringLiteral("BUILD %1 adds to %2%3")
                             .arg(role, build::buildHost(context).name,
                                  shared->hasTargets(role) ? QString() : QStringLiteral(" (a ground placement)"));
    if (PartPicker* picker = pinnedPicker(*shared)) {
      picker->ask(roleRequest(*shared, role, state));
    }
  };
  def.ended = pickerEnded(shared, def.id);
  def.inspect = [shared, role, profile](const CommandState& state, const CommandContext& context) {
    return inspectPlacement(shared, role, state, context, profile);
  };
  def.previewMarkers = markersOf;
  // The mouse on an end face shows what a click there places (mitcad#417).
  def.hoverPreview = [shared, role](const SelectionItem& item, CommandState& state, const CommandContext& context) {
    return hoverEnd(*shared, role, item, state, context);
  };
  def.apply = [shared, &host, role, profile](CommandState& state, const CommandContext& context) {
    Part part;
    if (!chosenPart(*shared, state, part)) {
      host.showStatus(noPartsText(*shared, role), true);
      return false;
    }
    const Placement placed = placement(*shared, role, state, context, part, profile);
    if (!placed.problem.isEmpty()) {
      host.showStatus(placed.problem, true);
      return false;
    }
    QJsonObject command = placed.fields;
    command.insert(QStringLiteral("cmd"), QStringLiteral("build_place"));
    QJsonObject result;
    if (!host.runModelCommand(command, &result)) {
      return false;
    }
    const QStringList warnings = warningsOf(result);
    qInfo().noquote() << QStringLiteral("Placed %1 as %2 (%3), length %4%5")
                             .arg(part.name, str(result, "name"), str(result, "occurrence"), placed.lengthNote,
                                  warnings.isEmpty() ? QString() : QStringLiteral("; ") + warnings.join(QStringLiteral("; ")));
    if (!warnings.isEmpty()) {
      host.showStatus(warnings.join(QLatin1Char(' ')), false);
    }
    // The next one: picked again; the part stays.
    state.setItems(kAt, {});
    state.setObject(QStringLiteral("at_pick"), {});
    state.setObject(kHover, {});
    state.setObject(kEndPick, {});
    state.setChecked(kOffsetTyped, false);
    shared->lastPart.insert(role, part.key);
    shared->catalog.refresh(role, context); // its source is the design's now
    shared->refreshDesign(context);
    return true;
  };
  return def;
}

// --- Panels

// A panel's thickness is a parameter of its own (else a fixed value).
bool thicknessIsParameter(const Part& part) {
  const QJsonValue thickness = part.build.value(QStringLiteral("panel")).toObject().value(QStringLiteral("thickness"));
  return thickness.isString() && !std::isfinite(build::millimetres(thickness));
}

QJsonObject panelFields(const CommandState& state, const CommandContext& context, const Part& part, QString& problem) {
  std::optional<QJsonObject> at = build::atOf(QStringLiteral("at"), state, context, problem);
  if (!at) {
    return {};
  }
  QJsonObject fields{{QStringLiteral("part"), part.ref()}, {QStringLiteral("at"), *at}};
  if (thicknessIsParameter(part)) {
    fields.insert(QStringLiteral("thickness"), state.expression(QStringLiteral("thickness")));
  }
  return fields;
}

CommandDef panelCommand(const SharedPtr& shared, DocumentHost& host) {
  const QString role = QStringLiteral("panel");
  CommandDef def;
  def.id = QStringLiteral("build.panel");
  def.name = QObject::tr("Panel");
  def.icon = QStringLiteral("build-panel");
  def.tooltip = QObject::tr("Fills a frame with a sheet: an inner face puts it in the slots, sized to the frame; an "
                            "outer face puts it on the face with clamps");
  def.tab = kTab;
  def.group = QStringLiteral("PROFILE");
  def.pinned = true;
  def.keywords = {QStringLiteral("sheet"), QStringLiteral("plate"), QStringLiteral("infill"), QStringLiteral("build")};
  def.okLabel = QObject::tr("Place");
  def.repeats = true;
  def.inputs = pickerInputs(shared, role);
  def.inputs.append(selectionInput(QStringLiteral("at"), QObject::tr("Frame Face"), SelectKind::Face, 1, 1)
                        .withOnPick([](const SelectionItem& item, CommandState& state, const CommandContext& context) {
                          build::recordPick(QStringLiteral("at"), item, state, context);
                        },
                                    false)
                        .withTooltip(QObject::tr("A face of one of the frame's profiles: inside the frame for the "
                                                 "slots, outside for the surface")));
  def.inputs.append(valueInput(QStringLiteral("thickness"), QObject::tr("Thickness"), ValueKind::Length,
                               QStringLiteral("3 mm"))
                        .withVisible([shared](const CommandState& state) {
                          Part part;
                          return chosenPart(*shared, state, part) && thicknessIsParameter(part);
                        }));
  def.init = [shared, role](CommandState& state, const CommandContext& context) {
    startPicker(*shared, role, state, context);
    if (PartPicker* picker = pinnedPicker(*shared)) {
      picker->ask(roleRequest(*shared, role, state));
    }
  };
  def.ended = pickerEnded(shared, def.id);
  def.inspect = [shared, role](const CommandState& state, const CommandContext& context) {
    Inspection result;
    Part part;
    if (!chosenPart(*shared, state, part)) {
      result.error = noPartsText(*shared, role);
      return result;
    }
    QString problem;
    QJsonObject request = panelFields(state, context, part, problem);
    if (!problem.isEmpty()) {
      result.error = problem;
      return result;
    }
    QJsonObject shapeRequest = request;
    request.insert(QStringLiteral("query"), QStringLiteral("build_preview"));
    QJsonObject answer;
    try {
      answer = context.queryObject(request);
    } catch (const std::exception& error) {
      result.error = errorText(error);
      return result;
    }
    shapeRequest.insert(QStringLiteral("shape"), QStringLiteral("build_preview"));
    result.shape = context.analysisShape(shapeRequest);
    QStringList lines{QStringLiteral("<b>%1</b>").arg(part.name.toHtmlEscaped())};
    const QString seller = QStringList{part.supplier, part.partNumber}.join(QLatin1Char(' ')).trimmed();
    if (!seller.isEmpty()) {
      lines << seller.toHtmlEscaped();
    }
    const bool fits = !answer.contains(QStringLiteral("fits")) || answer.value(QStringLiteral("fits")).toBool();
    if (!fits) {
      lines << QObject::tr("Does not fit: %1").arg(str(answer, "reason").toHtmlEscaped());
    }
    result.text = lines.join(QStringLiteral("<br>"));
    result.error = problemsOf(answer);
    result.log = QStringLiteral("panel %1%2").arg(part.name, fits ? QString() : QStringLiteral(", does not fit: ") + str(answer, "reason"));
    return result;
  };
  def.apply = [shared, &host, role](CommandState& state, const CommandContext& context) {
    Part part;
    if (!chosenPart(*shared, state, part)) {
      host.showStatus(noPartsText(*shared, role), true);
      return false;
    }
    QString problem;
    QJsonObject command = panelFields(state, context, part, problem);
    if (!problem.isEmpty()) {
      host.showStatus(problem, true);
      return false;
    }
    command.insert(QStringLiteral("cmd"), QStringLiteral("build_panel"));
    QJsonObject result;
    if (!host.runModelCommand(command, &result)) {
      return false;
    }
    const bool fits = !result.contains(QStringLiteral("fits")) || result.value(QStringLiteral("fits")).toBool();
    qInfo().noquote() << QStringLiteral("Placed panel %1 as %2 (%3)%4; %5 clamp(s)")
                             .arg(part.name, str(result, "name").isEmpty() ? str(result, "component") : str(result, "name"),
                                  str(result, "occurrence"),
                                  fits ? QStringLiteral(", fits") : QStringLiteral(", does not fit: ") + str(result, "reason"))
                             .arg(result.value(QStringLiteral("adds")).toArray().size());
    if (!fits) {
      host.showStatus(QObject::tr("%1 does not fit: %2").arg(part.name, str(result, "reason")), false);
    }
    state.setItems(QStringLiteral("at"), {});
    state.setObject(QStringLiteral("at_pick"), {});
    shared->lastPart.insert(role, part.key);
    shared->catalog.refresh(role, context);
    shared->refreshDesign(context);
    return true;
  };
  return def;
}

// --- Connectors

const QString kJunction = QStringLiteral("junction");

// The junction nearest to a point, not connected yet before connected ones.
QString nearestJunction(const Shared& shared, const std::optional<gp_Pnt>& point) {
  QString best;
  double bestDistance = std::numeric_limits<double>::infinity();
  for (const auto& option : shared.junctionOptions()) {
    const QJsonObject junction = shared.junction(option.first);
    const std::optional<gp_Pnt> at = build::junctionPoint(junction);
    double distance = point && at ? point->Distance(*at) : 0.0;
    if (!junction.value(QStringLiteral("connectors")).toArray().isEmpty()) {
      distance += 1e9; // a connected one only when no other is there
    }
    if (best.isEmpty() || distance < bestDistance) {
      best = option.first;
      bestDistance = distance;
    }
  }
  return best;
}

void chooseConnector(Shared& shared, CommandState& state) {
  const QJsonObject junction = shared.junction(state.choice(kJunction));
  if (junction.isEmpty()) {
    return;
  }
  const QVector<Part> parts = shared.catalog.forJunction(junction);
  const bool kept = std::any_of(parts.begin(), parts.end(),
                                [&state](const Part& part) { return part.key == state.choice(kPart); });
  if (!kept) {
    state.setChoice(kPart, parts.isEmpty() ? QString() : parts.first().key);
  }
}

const QString kConnectorCommand = QStringLiteral("build.connector");

// What Connector asks the part picker for (mitcad#418): the connectors
// that fit the junction, best first (the drop-down's keys), then those that
// nearly fit, greyed with the reason (library_build_candidates' `refused`).
PickerRequest connectorRequest(Shared& shared, const CommandState& state) {
  PickerRequest request;
  request.command = kConnectorCommand;
  request.title = QObject::tr("Connector");
  request.current = state.choice(kPart);
  const QJsonObject junction = shared.junction(state.choice(kJunction));
  if (junction.isEmpty()) {
    return request;
  }
  request.parts = pickerParts(shared.catalog.forJunction(junction));
  for (const PickerPart& nearly : buildCandidates(QJsonObject{{kJunction, junction}})) {
    if (!nearly.refused.isEmpty()) {
      request.parts.append(nearly);
    }
  }
  return request;
}

// The picker open for Connector follows it: another junction's connectors
// (`junction`), else the connector chosen.
void syncConnectorPicker(Shared& shared, const CommandState& state, bool junction) {
  if (PartPicker* picker = shownPicker(shared, kConnectorCommand)) {
    if (junction) {
      picker->setParts(connectorRequest(shared, state).parts, state.choice(kPart));
    } else {
      picker->setCurrent(state.choice(kPart));
    }
  }
}

// build_connect's fields, also its preview's.
QJsonObject connectFields(const CommandState& state, const QJsonObject& junction, const Part& part) {
  return {{kJunction, str(junction, "id")},
          {QStringLiteral("part"), part.ref()},
          {QStringLiteral("fasteners"), state.checked(QStringLiteral("fasteners"))},
          {QStringLiteral("machining"), state.checked(QStringLiteral("machining"))}};
}

// Holes to machine, [{end, hole: {diameter, depth, thread}}] in
// millimetres: "D6.8 x 15 M8 at end a".
QString holesText(const QJsonValue& machining) {
  QStringList holes;
  for (const QJsonValue& value : machining.toArray()) {
    const QJsonObject entry = value.toObject();
    // {end, hole: {...}} (build_edit's, build_parts'), or flat {at, diameter, depth,
    // thread} (build_preview's of a connector, the cut list's).
    const QJsonObject hole = entry.value(QStringLiteral("hole")).isObject() ? entry.value(QStringLiteral("hole")).toObject() : entry;
    // A reply's millimetres, or a value as a command was given it.
    const auto amount = [](const QJsonValue& value) {
      return value.isDouble() ? QString::number(value.toDouble(), 'g', 4) : value.toString();
    };
    QString shown = QStringLiteral("D%1").arg(amount(hole.value(QStringLiteral("diameter"))));
    if (!hole.value(QStringLiteral("depth")).isUndefined()) {
      shown += QStringLiteral(" x %1").arg(amount(hole.value(QStringLiteral("depth"))));
    }
    if (!str(hole, "thread").isEmpty()) {
      shown += QLatin1Char(' ') + str(hole, "thread");
    }
    if (!str(entry, "end").isEmpty()) {
      shown += QStringLiteral(" at end ") + str(entry, "end");
    } else if (!str(entry, "at").isEmpty()) {
      shown += QStringLiteral(" at ") + str(entry, "at");
    }
    holes << shown;
  }
  return holes.join(QStringLiteral("; "));
}

CommandDef connectorCommand(const SharedPtr& shared, DocumentHost& host, const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("build.connector");
  def.name = QObject::tr("Connector");
  def.icon = QStringLiteral("build-connector");
  def.tooltip = QObject::tr("Connects two profiles where they meet: the junctions are marked; the list has the "
                            "connectors that fit, each placed with its nuts, screws and end machining");
  def.tab = kTab;
  def.group = QStringLiteral("CONNECT");
  def.pinned = true;
  def.keywords = {QStringLiteral("bracket"), QStringLiteral("corner"), QStringLiteral("angle"),
                  QStringLiteral("junction"), QStringLiteral("build")};
  def.okLabel = QObject::tr("Connect");
  def.repeats = true;
  def.inputs = {
      selectionInput(QStringLiteral("at"), QObject::tr("Near"), kPartKinds, 0, 1)
          .withOnPick([shared](const SelectionItem& item, CommandState& state, const CommandContext& model) {
            build::recordPick(QStringLiteral("at"), item, state, model);
            const QString before = state.choice(kJunction);
            state.setChoice(kJunction, nearestJunction(*shared, build::pickPoint(QStringLiteral("at"), state, model)));
            chooseConnector(*shared, state);
            syncConnectorPicker(*shared, state, state.choice(kJunction) != before);
          },
                      false)
          .withTooltip(QObject::tr("A click on a profile near a junction chooses that junction")),
      choiceInput(kJunction, QObject::tr("Junction"), {}, QString())
          .withChoices([shared](const CommandState&) { return shared->junctionOptions(); })
          .withOnChange([shared](CommandState& state, const CommandContext&) {
            chooseConnector(*shared, state);
            syncConnectorPicker(*shared, state, true);
          }),
      choiceInput(kPart, QObject::tr("Connector"), {}, QString())
          .withOnChange([shared](CommandState& state, const CommandContext&) {
            syncConnectorPicker(*shared, state, false);
          })
          // A tile that opens the part picker (mitcad#427).
          .withChoiceTile([shared](const CommandState& state) { return partTile(*shared, state); })
          .withButton(kButtonText, buttonTip(),
                      [shared](const CommandState& state, const CommandContext&) {
                        if (PartPicker* picker = partPicker(*shared, true)) {
                          picker->ask(connectorRequest(*shared, state));
                        }
                      })
          .withChoices([shared](const CommandState& state) {
            Choices choices;
            const QJsonObject junction = shared->junction(state.choice(kJunction));
            if (junction.isEmpty()) {
              return choices;
            }
            for (const Part& part : shared->catalog.forJunction(junction)) {
              choices.append({part.key, part.label()});
            }
            return choices;
          })
          // Each connector with the library's picture of it.
          .withChoiceImages([shared](const QString& key) {
            Part part;
            return shared->catalog.find(key, part) ? shared->thumbnail(part) : QImage();
          })
          .withTooltip(QObject::tr("The connector chosen among those that fit the junction's systems and room: a "
                                   "click (or Enter) opens the part picker, the best fitting first")),
      checkInput(QStringLiteral("fasteners"), QObject::tr("With Nuts and Screws"), true),
      checkInput(QStringLiteral("machining"), QObject::tr("With End Machining"), true),
      checkInput(QStringLiteral("all_junctions"), QObject::tr("Show All Junctions"), false)
          .withTooltip(QObject::tr("Off: a profile's junctions show while the mouse is over it")),
  };
  def.enabled = [&context] { return build::hasBuildParts(context); };
  def.init = [shared](CommandState& state, const CommandContext& model) {
    shared->catalog.refresh(QStringLiteral("connector"), model);
    shared->refreshDesign(model);
    if (shared->junction(state.choice(kJunction)).isEmpty()) {
      state.setChoice(kJunction, nearestJunction(*shared, std::nullopt));
    }
    chooseConnector(*shared, state);
    if (PartPicker* picker = pinnedPicker(*shared)) {
      picker->ask(connectorRequest(*shared, state));
    }
  };
  def.ended = pickerEnded(shared, def.id);
  def.inspect = [shared](const CommandState& state, const CommandContext& model) {
    Inspection result;
    const QJsonObject junction = shared->junction(state.choice(kJunction));
    // The junctions with their parts, for the markers of the hover
    // (connectorMarkers); the chosen one's label first.
    QJsonArray junctions;
    for (const auto& option : shared->junctionOptions()) {
      const QJsonObject other = shared->junction(option.first);
      const std::optional<gp_Pnt> at = build::junctionPoint(other);
      if (!at) {
        continue;
      }
      QString shown = str(other, "kind");
      if (!other.value(QStringLiteral("connectors")).toArray().isEmpty()) {
        shown += QObject::tr(" (connected)");
      }
      if (option.first == state.choice(kJunction)) {
        shown = QStringLiteral("> ") + shown;
      }
      QJsonObject entry = marker(shown, *at);
      entry.insert(QStringLiteral("id"), option.first);
      QJsonArray parts;
      for (const char* side : {"a", "b"}) {
        parts.append(build::rootPath(str(other.value(QLatin1String(side)).toObject(), "occurrence"), model));
      }
      entry.insert(QStringLiteral("parts"), parts);
      junctions.append(entry);
    }
    result.report = {{QStringLiteral("junctions"), junctions}};
    if (junction.isEmpty()) {
      result.error = QObject::tr("No junctions: place profiles that meet first.");
      return result;
    }
    for (const char* side : {"a", "b"}) {
      const QString path = build::rootPath(str(junction.value(QLatin1String(side)).toObject(), "occurrence"), model);
      if (!path.isEmpty()) {
        result.highlights.append(SelectionItem{SelectKind::Component, QString(), QString(), QString(), path});
      }
    }
    QStringList refused;
    const QVector<Part> parts = shared->catalog.forJunction(junction, &refused);
    QStringList lines{QStringLiteral("<b>%1</b>").arg(build::junctionText(junction, model).toHtmlEscaped())};
    QStringList systems;
    for (const QJsonValue& system : junction.value(QStringLiteral("systems")).toArray()) {
      systems << (system.isString() ? system.toString()
                                    : QString::fromUtf8(QJsonDocument(system.toObject()).toJson(QJsonDocument::Compact)));
    }
    if (!systems.isEmpty()) {
      lines << QObject::tr("Systems: %1").arg(systems.join(QStringLiteral(", ")).toHtmlEscaped());
    }
    for (const Part& part : parts) {
      const QString shown = part.label().toHtmlEscaped();
      lines << (part.key == state.choice(kPart) ? QStringLiteral("<b>%1</b>").arg(shown) : shown);
    }
    for (const QString& reason : refused) {
      lines << QStringLiteral("<span style=\"color: gray\">%1</span>").arg(reason.toHtmlEscaped());
    }
    QString preview;
    Part part;
    if (parts.isEmpty()) {
      result.error = QObject::tr("No connector of the fetched libraries fits this junction.");
    } else if (shared->catalog.find(state.choice(kPart), part)) {
      // The chosen connector where it goes, with its fasteners and the
      // machining it needs (build_preview of build_connect's fields).
      const QJsonObject fields = connectFields(state, junction, part);
      QJsonObject request = fields;
      request.insert(QStringLiteral("query"), QStringLiteral("build_preview"));
      QJsonObject answer;
      try {
        answer = model.queryObject(request);
      } catch (const std::exception& error) {
        result.error = errorText(error);
      }
      QJsonObject shapeRequest = fields;
      shapeRequest.insert(QStringLiteral("shape"), QStringLiteral("build_preview"));
      result.shape = model.analysisShape(shapeRequest);
      QJsonArray markers;
      int fasteners = 0;
      for (const QJsonValue& value : answer.value(QStringLiteral("fasteners")).toArray()) {
        const QJsonObject fastener = value.toObject();
        const build::Frame frame = build::frameOf(fastener.value(QStringLiteral("frame")));
        if (frame.valid) {
          // {frame, at}: the connector's fastener frame it goes on.
          const QString at = str(fastener, "at");
          markers.append(marker(at.isEmpty() ? QObject::tr("fastener") : QObject::tr("fastener %1").arg(at), frame.origin));
          ++fasteners;
        }
      }
      // The holes it machines: {component, at, point, diameter, depth, thread}.
      for (const QJsonValue& value : answer.value(QStringLiteral("machining")).toArray()) {
        const QJsonObject hole = value.toObject();
        const QJsonArray point = hole.value(QStringLiteral("point")).toArray();
        if (point.size() >= 3) {
          markers.append(marker(QObject::tr("bore D%1").arg(hole.value(QStringLiteral("diameter")).toDouble(), 0, 'g', 4),
                                gp_Pnt(point[0].toDouble(), point[1].toDouble(), point[2].toDouble())));
        }
      }
      result.report.insert(QStringLiteral("markers"), markers);
      const QString holes = holesText(answer.value(QStringLiteral("machining")));
      lines << QObject::tr("Preview: %1, %2 fastener(s)%3")
                   .arg(part.name.toHtmlEscaped())
                   .arg(fasteners)
                   .arg(holes.isEmpty() ? QString() : QObject::tr(", machining %1").arg(holes.toHtmlEscaped()));
      if (result.error.isEmpty()) {
        result.error = problemsOf(answer);
      }
      preview = QStringLiteral(", preview %1 with %2 fastener(s)%3")
                    .arg(part.name)
                    .arg(fasteners)
                    .arg(holes.isEmpty() ? QString() : QStringLiteral(", machining ") + holes);
    }
    result.text = lines.join(QStringLiteral("<br>"));
    result.log = QStringLiteral("junction %1 (%2): %3 fit, %4 nearly%5")
                     .arg(str(junction, "id"), str(junction, "kind"))
                     .arg(parts.size())
                     .arg(refused.size())
                     .arg(preview);
    return result;
  };
  // The junctions show on the hover of their profiles (all of them with
  // Show All Junctions), the chosen one always; the connector's fasteners.
  def.hover = [](const SelectionItem& item, CommandState& state, const CommandContext&) {
    const QString occurrence = item.isValid() ? item.occurrence : QString();
    if (state.object(QStringLiteral("hover")).value(QStringLiteral("occurrence")).toString() == occurrence) {
      return false;
    }
    state.setObject(QStringLiteral("hover"), {{QStringLiteral("occurrence"), occurrence}});
    return true;
  };
  def.previewMarkersIn = [](const QJsonObject& report, const CommandState& state) {
    std::vector<PreviewMarker> markers = markersOf(report);
    const bool all = state.checked(QStringLiteral("all_junctions"));
    const QString hovered = state.object(QStringLiteral("hover")).value(QStringLiteral("occurrence")).toString();
    for (const QJsonValue& value : report.value(QStringLiteral("junctions")).toArray()) {
      const QJsonObject entry = value.toObject();
      bool shown = all || str(entry, "id") == state.choice(kJunction);
      for (const QJsonValue& part : entry.value(QStringLiteral("parts")).toArray()) {
        const QString path = part.toString();
        shown = shown || (!path.isEmpty() && !hovered.isEmpty() &&
                          (hovered == path || hovered.startsWith(path + QLatin1Char('/'))));
      }
      const QJsonArray point = entry.value(QStringLiteral("point")).toArray();
      if (shown && point.size() >= 3) {
        markers.push_back({str(entry, "label"), gp_Pnt(point[0].toDouble(), point[1].toDouble(), point[2].toDouble())});
      }
    }
    return markers;
  };
  def.apply = [shared, &host](CommandState& state, const CommandContext& model) {
    const QJsonObject junction = shared->junction(state.choice(kJunction));
    Part part;
    if (junction.isEmpty() || !shared->catalog.find(state.choice(kPart), part)) {
      host.showStatus(QObject::tr("Choose a junction and a connector."), true);
      return false;
    }
    QJsonObject command = connectFields(state, junction, part);
    command.insert(QStringLiteral("cmd"), QStringLiteral("build_connect"));
    QJsonObject result;
    if (!host.runModelCommand(command, &result)) {
      return false;
    }
    qInfo().noquote() << QStringLiteral("Connected %1 at junction %2 (%3) as %4: %5 added")
                             .arg(part.name, str(junction, "id"), str(junction, "kind"), str(result, "component"))
                             .arg(result.value(QStringLiteral("adds")).toArray().size());
    state.setItems(QStringLiteral("at"), {});
    shared->catalog.refresh(QStringLiteral("connector"), model);
    shared->refreshDesign(model);
    // The next junction not connected yet.
    state.setChoice(kJunction, nearestJunction(*shared, build::junctionPoint(junction)));
    chooseConnector(*shared, state);
    syncConnectorPicker(*shared, state, true);
    return true;
  };
  return def;
}

// --- Fasteners

// The connector of a picked part: itself, or the connector that added it
// (a nut, a screw).
std::optional<BuildPart> connectorOf(const Shared& shared, const CommandState& state) {
  const Selection& items = state.items(QStringLiteral("connector"));
  if (items.isEmpty()) {
    return std::nullopt;
  }
  const std::optional<BuildPart> picked = shared.partAt(items.first().occurrence);
  if (!picked) {
    return std::nullopt;
  }
  if (picked->role == QStringLiteral("connector") || picked->addedBy.isEmpty()) {
    return picked;
  }
  for (const BuildPart& part : shared.designParts()) {
    if (part.component == picked->addedBy) {
      return part;
    }
  }
  return picked;
}

// The connector's fasteners, by source and row: "<source>|<row>".
QHash<QString, QVector<BuildPart>> fastenersOf(const Shared& shared, const BuildPart& connector) {
  QHash<QString, QVector<BuildPart>> groups;
  for (const BuildPart& part : shared.designParts()) {
    if (part.addedBy == connector.component && part.role == QStringLiteral("fastener")) {
      groups[part.source + QLatin1Char('|') + part.config].append(part);
    }
  }
  return groups;
}

// The configuration rows of a source's library component (library_show),
// asked once per source.
Choices rowsOfSource(Shared& shared, const QString& source, const CommandContext& context) {
  {
    const std::lock_guard<std::mutex> lock(shared.mutex);
    const auto found = shared.rows.constFind(source);
    if (found != shared.rows.cend()) {
      return found.value();
    }
  }
  Choices rows;
  try {
    for (const QJsonValue& value : context.queryObject({{QStringLiteral("query"), QStringLiteral("library_parts")}})
                                       .value(QStringLiteral("parts"))
                                       .toArray()) {
      const QJsonObject entry = value.toObject();
      if (str(entry, "source") != source) {
        continue;
      }
      const QJsonObject record = entry.value(QStringLiteral("library")).toObject();
      const QJsonObject shown = libraryCommand({{QStringLiteral("cmd"), QStringLiteral("library_show")},
                                                {QStringLiteral("url"), str(record, "url")},
                                                {QStringLiteral("rev"), str(record, "rev")}});
      for (const QJsonValue& component : shown.value(QStringLiteral("components")).toArray()) {
        if (str(component.toObject(), "id") != str(record, "component")) {
          continue;
        }
        for (const QJsonValue& row : component.toObject()
                                         .value(QStringLiteral("configurations"))
                                         .toObject()
                                         .value(QStringLiteral("rows"))
                                         .toArray()) {
          rows.append({str(row.toObject(), "name"), str(row.toObject(), "name")});
        }
      }
    }
  } catch (const std::exception&) {
    // Its own row then.
  }
  const std::lock_guard<std::mutex> lock(shared.mutex);
  shared.rows.insert(source, rows);
  return rows;
}

CommandDef fastenersCommand(const SharedPtr& shared, DocumentHost& host, const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("build.fasteners");
  def.name = QObject::tr("Fasteners");
  def.icon = QStringLiteral("build-fasteners");
  def.tooltip = QObject::tr("The nuts and screws of a connector: another size, taken off, or added again");
  def.tab = kTab;
  def.group = QStringLiteral("CONNECT");
  def.pinned = true;
  def.keywords = {QStringLiteral("nut"), QStringLiteral("screw"), QStringLiteral("t-nut"), QStringLiteral("bolt"),
                  QStringLiteral("build")};
  const QString kFastener = QStringLiteral("fastener");
  const QString kSize = QStringLiteral("size");
  def.inputs = {
      selectionInput(QStringLiteral("connector"), QObject::tr("Connector"), kPartKinds, 1, 1)
          .withOnPick([shared, kFastener, kSize](const SelectionItem&, CommandState& state, const CommandContext&) {
            const std::optional<BuildPart> connector = connectorOf(*shared, state);
            const QHash<QString, QVector<BuildPart>> groups =
                connector ? fastenersOf(*shared, *connector) : QHash<QString, QVector<BuildPart>>();
            state.setChecked(QStringLiteral("with"), !groups.isEmpty() || !connector);
            if (!groups.isEmpty()) {
              const QString first = groups.keys().first();
              state.setChoice(kFastener, first);
              state.setChoice(kSize, first.section(QLatin1Char('|'), 1));
            }
          },
                      false)
          .withTooltip(QObject::tr("A connector, or one of its nuts or screws")),
      checkInput(QStringLiteral("with"), QObject::tr("With Nuts and Screws"), true)
          .withTooltip(QObject::tr("Off: they are taken off. On for a connector without them: it is connected again "
                                   "with them")),
      choiceInput(kFastener, QObject::tr("Fastener"), {}, QString())
          .withChoices([shared](const CommandState& state) {
            Choices choices;
            if (const std::optional<BuildPart> connector = connectorOf(*shared, state)) {
              const QHash<QString, QVector<BuildPart>> groups = fastenersOf(*shared, *connector);
              for (auto it = groups.cbegin(); it != groups.cend(); ++it) {
                choices.append({it.key(), QStringLiteral("%1 x %2").arg(it.value().size()).arg(it.value().first().shownName())});
              }
            }
            return choices;
          })
          .withVisible([](const CommandState& state) { return state.checked(QStringLiteral("with")); }),
      choiceInput(kSize, QObject::tr("Size"), {}, QString())
          .withChoices([shared, &context, kFastener](const CommandState& state) {
            const QString source = state.choice(kFastener).section(QLatin1Char('|'), 0, 0);
            return source.isEmpty() ? Choices() : rowsOfSource(*shared, source, context);
          })
          .withVisible([](const CommandState& state) { return state.checked(QStringLiteral("with")); }),
  };
  def.enabled = [&context] { return build::hasBuildParts(context); };
  def.init = [shared](CommandState&, const CommandContext& model) { shared->refreshDesign(model); };
  def.inspect = [shared](const CommandState& state, const CommandContext&) {
    Inspection result;
    const std::optional<BuildPart> connector = connectorOf(*shared, state);
    if (!connector) {
      result.error = QObject::tr("Pick a connector of the BUILD tab.");
      return result;
    }
    QStringList lines{QStringLiteral("<b>%1</b>").arg(connector->shownName().toHtmlEscaped())};
    const QHash<QString, QVector<BuildPart>> groups = fastenersOf(*shared, *connector);
    for (auto it = groups.cbegin(); it != groups.cend(); ++it) {
      lines << QStringLiteral("%1 x %2").arg(it.value().size()).arg(it.value().first().shownName().toHtmlEscaped());
    }
    if (groups.isEmpty()) {
      lines << QObject::tr("No nuts or screws");
    }
    result.text = lines.join(QStringLiteral("<br>"));
    result.log = QStringLiteral("connector %1: %2 kind(s) of fastener").arg(connector->component).arg(groups.size());
    return result;
  };
  def.apply = [shared, &host, kFastener, kSize](CommandState& state, const CommandContext& model) {
    const std::optional<BuildPart> connector = connectorOf(*shared, state);
    if (!connector) {
      host.showStatus(QObject::tr("Pick a connector of the BUILD tab."), true);
      return false;
    }
    const QHash<QString, QVector<BuildPart>> groups = fastenersOf(*shared, *connector);
    QJsonArray commands;
    if (!state.checked(QStringLiteral("with"))) {
      for (const QVector<BuildPart>& group : groups) {
        for (const BuildPart& fastener : group) {
          commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("build_remove")},
                                      {QStringLiteral("component"), fastener.component}});
        }
      }
    } else if (groups.isEmpty()) {
      // Connected again, with them, where it is.
      QString junction;
      for (const QJsonValue& value : build::junctions(model)) {
        if (value.toObject().value(QStringLiteral("connectors")).toArray().contains(connector->component)) {
          junction = str(value.toObject(), "id");
        }
      }
      if (junction.isEmpty() || connector->source.isEmpty()) {
        host.showStatus(QObject::tr("%1 is at no junction to connect again.").arg(connector->shownName()), true);
        return false;
      }
      QJsonObject part{{QStringLiteral("source"), connector->source}};
      if (!connector->config.isEmpty()) {
        part.insert(QStringLiteral("config"), connector->config);
      }
      commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("build_remove")},
                                  {QStringLiteral("component"), connector->component}});
      commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("build_connect")},
                                  {kJunction, junction},
                                  {QStringLiteral("part"), part},
                                  {QStringLiteral("fasteners"), true}});
    } else {
      const QString size = state.choice(kSize);
      for (const BuildPart& fastener : groups.value(state.choice(kFastener))) {
        if (!size.isEmpty() && fastener.config != size) {
          commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("build_edit")},
                                      {QStringLiteral("component"), fastener.component},
                                      {QStringLiteral("config"), size}});
        }
      }
    }
    if (commands.isEmpty()) {
      return true; // nothing to change
    }
    const QString label = QObject::tr("Fasteners of %1").arg(connector->shownName());
    if (!host.runModelCommands(commands, label)) {
      return false;
    }
    qInfo().noquote() << QStringLiteral("%1: %2 command(s)").arg(label).arg(commands.size());
    return true;
  };
  return def;
}

// --- MODIFY: a part picked

QString partsText(const QVector<BuildPart>& parts) {
  QStringList names;
  for (const BuildPart& part : parts) {
    names << part.shownName();
  }
  return names.join(QStringLiteral(", "));
}

const Choices kLengthEdits = {{kTyped, QStringLiteral("Detached: this length")},
                              {QStringLiteral("face"), QStringLiteral("Bound: to the face")},
                              {QStringLiteral("centre"), QStringLiteral("Bound: to the centre")},
                              {QStringLiteral("through"), QStringLiteral("Bound: through")}};

CommandDef lengthCommand(DocumentHost& host, const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("build.length");
  def.name = QObject::tr("Length");
  def.icon = QStringLiteral("build-length");
  def.tooltip = QObject::tr("A part's length: bound to the part it runs to (it follows when that one moves), or "
                            "detached as a number");
  def.tab = kTab;
  def.group = QStringLiteral("MODIFY");
  def.pinned = true;
  def.keywords = {QStringLiteral("bind"), QStringLiteral("detach"), QStringLiteral("cut"), QStringLiteral("build")};
  const auto typed = [](const CommandState& state) { return state.choice(QStringLiteral("mode")) == kTyped; };
  const auto bound = [](const CommandState& state) { return state.choice(QStringLiteral("mode")) != kTyped; };
  def.inputs = {
      selectionInput(kPart, QObject::tr("Part"), kPartKinds, 1, 1)
          .withOnPick([](const SelectionItem& item, CommandState& state, const CommandContext& model) {
            if (const std::optional<BuildPart> part = build::partOf(item, model)) {
              if (std::isfinite(part->length)) {
                state.setText(QStringLiteral("length"), cmd::lengthText(part->length, model));
                state.setValue(QStringLiteral("length"), kNaN);
              }
              const QString mode = str(part->bound, "mode");
              state.setChoice(QStringLiteral("mode"), part->bound.isEmpty() ? kTyped : (mode.isEmpty() ? QStringLiteral("face") : mode));
            }
          },
                      false),
      choiceInput(QStringLiteral("mode"), QObject::tr("Length"), kLengthEdits, kTyped),
      valueInput(QStringLiteral("length"), QObject::tr("Value"), ValueKind::Length, QStringLiteral("500 mm"))
          .withVisible(typed),
      selectionInput(QStringLiteral("target"), QObject::tr("Bound To"), SelectKind::Face, 0, 1)
          .withVisible(bound)
          .withTooltip(QObject::tr("The face the length runs to; none: the one it is bound to now")),
  };
  def.enabled = [&context] { return build::hasBuildParts(context); };
  def.inspect = [](const CommandState& state, const CommandContext& model) {
    Inspection result;
    const QVector<BuildPart> parts = build::partsOf(state.items(kPart), model);
    if (parts.isEmpty()) {
      result.error = QObject::tr("Pick a part of the BUILD tab.");
      return result;
    }
    const BuildPart& part = parts.first();
    QStringList lines{QStringLiteral("<b>%1</b>").arg(part.shownName().toHtmlEscaped())};
    if (std::isfinite(part.length)) {
      lines << QObject::tr("Length %1 mm").arg(part.length, 0, 'f', 1);
    }
    if (!part.bound.isEmpty()) {
      const QJsonObject to = part.bound.value(QStringLiteral("to")).toObject();
      lines << QObject::tr("Bound (%1) to %2").arg(str(part.bound, "mode"), targetText(to, model).toHtmlEscaped());
      if (const std::optional<SelectionItem> target = build::itemOfTarget(to, model)) {
        result.highlights.append(*target);
      }
    } else {
      lines << QObject::tr("A number (detached)");
    }
    result.text = lines.join(QStringLiteral("<br>"));
    result.log = QStringLiteral("%1: %2 mm, %3").arg(part.component, QString::number(part.length, 'f', 2),
                                                     part.bound.isEmpty() ? QStringLiteral("detached")
                                                                          : QStringLiteral("bound ") + str(part.bound, "mode"));
    return result;
  };
  def.apply = [&host](CommandState& state, const CommandContext& model) {
    const QVector<BuildPart> parts = build::partsOf(state.items(kPart), model);
    if (parts.isEmpty()) {
      host.showStatus(QObject::tr("Pick a part of the BUILD tab."), true);
      return false;
    }
    const BuildPart& part = parts.first();
    const QString mode = state.choice(QStringLiteral("mode"));
    QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("build_edit")}, {QStringLiteral("component"), part.component}};
    if (mode == kTyped) {
      command.insert(QStringLiteral("length"), state.expression(QStringLiteral("length")));
    } else {
      QJsonObject bind;
      const Selection& targets = state.items(QStringLiteral("target"));
      if (!targets.isEmpty()) {
        const std::optional<QString> path = build::pathFromHost(targets.first(), model);
        if (!path) {
          host.showStatus(QObject::tr("%1 is not in %2, where BUILD places parts.").arg(targets.first().describe(), build::buildHost(model).name), true);
          return false;
        }
        bind.insert(QStringLiteral("geometry"), targets.first().reference());
        if (!path->isEmpty()) {
          bind.insert(QStringLiteral("occurrence"), *path);
        }
      } else {
        bind = part.bound.value(QStringLiteral("to")).toObject();
      }
      if (bind.isEmpty()) {
        host.showStatus(QObject::tr("Pick the face %1's length runs to.").arg(part.shownName()), true);
        return false;
      }
      command.insert(QStringLiteral("length"), QJsonObject{{QStringLiteral("bind"), bind},
                                                           {QStringLiteral("mode"), mode},
                                                           {QStringLiteral("add"), 0}});
    }
    QJsonObject result;
    if (!host.runModelCommand(command, &result)) {
      return false;
    }
    qInfo().noquote() << QStringLiteral("Length of %1: %2").arg(part.shownName(),
                                                               mode == kTyped ? state.expression(QStringLiteral("length"))
                                                                              : QStringLiteral("bound, ") + mode);
    return true;
  };
  return def;
}

// The end of a part a pick is nearer to: its end face's name, else where
// the click was along the part (end A at z = 0 of its component).
QString endOfPick(const SelectionItem& item, const BuildPart& part) {
  if (item.name.endsWith(QStringLiteral(":end_b"))) {
    return QStringLiteral("b");
  }
  if (item.name.endsWith(QStringLiteral(":end_a"))) {
    return QStringLiteral("a");
  }
  if (item.at && std::isfinite(part.length)) {
    return (*item.at)[2] > part.length / 2.0 ? QStringLiteral("b") : QStringLiteral("a");
  }
  return QStringLiteral("a");
}

// The part whose end meets this end (an L or inline junction), and that
// end: for a shared mitre.
std::optional<std::pair<QString, QString>> otherEnd(const QString& component, const QString& end,
                                                    const CommandContext& context) {
  const QString frame = QStringLiteral("end_") + end;
  for (const QJsonValue& value : build::junctions(context)) {
    const QJsonObject junction = value.toObject();
    const QJsonObject a = junction.value(QStringLiteral("a")).toObject();
    const QJsonObject b = junction.value(QStringLiteral("b")).toObject();
    for (const auto& [mine, other] : {std::pair<QJsonObject, QJsonObject>(a, b), std::pair<QJsonObject, QJsonObject>(b, a)}) {
      if (str(mine, "component") == component && str(mine, "at") == frame && str(other, "at").startsWith(QStringLiteral("end_"))) {
        return std::make_pair(str(other, "component"), str(other, "at").mid(4));
      }
    }
  }
  return std::nullopt;
}

CommandDef endsCommand(DocumentHost& host, const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("build.ends");
  def.name = QObject::tr("Ends");
  def.icon = QStringLiteral("build-ends");
  def.tooltip = QObject::tr("A part's end cut: a mitre's angle and the axis it turns about, also shared with the "
                            "part that meets it there");
  def.tab = kTab;
  def.group = QStringLiteral("MODIFY");
  def.pinned = true;
  def.keywords = {QStringLiteral("mitre"), QStringLiteral("miter"), QStringLiteral("angle"), QStringLiteral("cut"),
                  QStringLiteral("bore"), QStringLiteral("tap"), QStringLiteral("machining"), QStringLiteral("build")};
  const Choices kMachining = {{QStringLiteral("keep"), QObject::tr("As it is")},
                              {QStringLiteral("bore"), QObject::tr("A bore")},
                              {QStringLiteral("none"), QObject::tr("None")}};
  const auto bore = [](const CommandState& state) { return state.choice(QStringLiteral("machining")) == QStringLiteral("bore"); };
  def.inputs = {
      selectionInput(kPart, QObject::tr("Part"), kPartKinds, 1, 1)
          .withOnPick([](const SelectionItem& item, CommandState& state, const CommandContext& model) {
            if (const std::optional<BuildPart> part = build::partOf(item, model)) {
              const QString end = endOfPick(item, *part);
              state.setChoice(QStringLiteral("end"), end);
              const QJsonObject cut = part->ends.value(end).toObject();
              if (!str(cut, "tilt").isEmpty()) {
                state.setChoice(QStringLiteral("tilt"), str(cut, "tilt"));
              }
            }
          },
                      false)
          .withTooltip(QObject::tr("Near the end to cut")),
      choiceInput(QStringLiteral("end"), QObject::tr("End"), kEnds, QStringLiteral("a")),
      valueInput(QStringLiteral("angle"), QObject::tr("Angle"), ValueKind::Angle, QStringLiteral("45 deg"))
          .withTooltip(QObject::tr("From square, 0 up to 90 degrees")),
      choiceInput(QStringLiteral("tilt"), QObject::tr("About"), kTilts, QStringLiteral("x")),
      checkInput(QStringLiteral("shared"), QObject::tr("Also the Part It Meets"), false)
          .withTooltip(QObject::tr("A shared mitre: the end of the part that meets this end is cut too")),
      // End machining (build_edit's `machining`): a bore in this end.
      choiceInput(QStringLiteral("machining"), QObject::tr("End Machining"), kMachining, QStringLiteral("keep"))
          .withHeading(QObject::tr("End machining"))
          .withTooltip(QObject::tr("A bore (tapped when a thread is given) in the chosen end; None takes the part's "
                                   "end machining away")),
      valueInput(QStringLiteral("bore_diameter"), QObject::tr("Diameter"), ValueKind::Length, QStringLiteral("6.8 mm"))
          .withVisible(bore),
      valueInput(QStringLiteral("bore_depth"), QObject::tr("Depth"), ValueKind::Length, QStringLiteral("15 mm"))
          .withVisible(bore),
      textInput(QStringLiteral("bore_thread"), QObject::tr("Thread"), QStringLiteral("M8"))
          .withVisible(bore)
          .withTooltip(QObject::tr("Empty: a plain bore")),
  };
  def.enabled = [&context] { return build::hasBuildParts(context); };
  def.inspect = [](const CommandState& state, const CommandContext& model) {
    Inspection result;
    const QVector<BuildPart> parts = build::partsOf(state.items(kPart), model);
    if (parts.isEmpty()) {
      result.error = QObject::tr("Pick a part of the BUILD tab, near the end to cut.");
      return result;
    }
    const BuildPart& part = parts.first();
    QStringList lines{QStringLiteral("<b>%1</b>").arg(part.shownName().toHtmlEscaped()), endsText(part.ends)};
    if (!part.machining.isEmpty()) {
      lines << QObject::tr("Machining: %1").arg(holesText(part.machining).toHtmlEscaped());
    }
    if (state.checked(QStringLiteral("shared"))) {
      const auto other = otherEnd(part.component, state.choice(QStringLiteral("end")), model);
      lines << (other ? QObject::tr("Also %1, end %2").arg(build::componentName(other->first, model), other->second.toUpper())
                      : QObject::tr("No part's end meets this end"));
    }
    result.text = lines.join(QStringLiteral("<br>"));
    result.log = QStringLiteral("%1 end %2: %3").arg(part.component, state.choice(QStringLiteral("end")), endsText(part.ends));
    return result;
  };
  def.apply = [&host](CommandState& state, const CommandContext& model) {
    const QVector<BuildPart> parts = build::partsOf(state.items(kPart), model);
    if (parts.isEmpty()) {
      host.showStatus(QObject::tr("Pick a part of the BUILD tab."), true);
      return false;
    }
    const BuildPart& part = parts.first();
    const QString end = state.choice(QStringLiteral("end"));
    const QJsonObject cut{{QStringLiteral("angle"), state.expression(QStringLiteral("angle"))},
                          {QStringLiteral("tilt"), state.choice(QStringLiteral("tilt"))}};
    QJsonObject edit{{QStringLiteral("cmd"), QStringLiteral("build_edit")},
                     {QStringLiteral("component"), part.component},
                     {QStringLiteral("ends"), QJsonObject{{end, cut}}}};
    // `machining` is the part's whole list: the other end's entries stay,
    // this end's are replaced (a bore) or left out (none).
    const QString machining = state.choice(QStringLiteral("machining"));
    QJsonArray added;
    if (machining != QStringLiteral("keep")) {
      QJsonArray list;
      for (const QJsonValue& value : part.machining) {
        if (str(value.toObject(), "end") != end) {
          list.append(value);
        }
      }
      if (machining == QStringLiteral("bore")) {
        QJsonObject hole{{QStringLiteral("diameter"), state.expression(QStringLiteral("bore_diameter"))},
                         {QStringLiteral("depth"), state.expression(QStringLiteral("bore_depth"))}};
        if (!state.text(QStringLiteral("bore_thread")).trimmed().isEmpty()) {
          hole.insert(QStringLiteral("thread"), state.text(QStringLiteral("bore_thread")).trimmed());
        }
        const QJsonObject entry{{QStringLiteral("end"), end}, {QStringLiteral("hole"), hole}};
        list.append(entry);
        added.append(entry);
      }
      edit.insert(QStringLiteral("machining"), list);
    }
    QJsonArray commands{edit};
    if (state.checked(QStringLiteral("shared"))) {
      const auto other = otherEnd(part.component, end, model);
      if (!other) {
        host.showStatus(QObject::tr("No part's end meets end %1 of %2.").arg(end.toUpper(), part.shownName()), true);
        return false;
      }
      commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("build_edit")},
                                  {QStringLiteral("component"), other->first},
                                  {QStringLiteral("ends"), QJsonObject{{other->second, cut}}}});
    }
    if (!host.runModelCommands(commands, QObject::tr("Ends of %1").arg(part.shownName()))) {
      return false;
    }
    qInfo().noquote() << QStringLiteral("Ends of %1: end %2 %3 about %4%5%6")
                             .arg(part.shownName(), end, state.expression(QStringLiteral("angle")),
                                  state.choice(QStringLiteral("tilt")),
                                  state.checked(QStringLiteral("shared")) ? QStringLiteral(", shared") : QString(),
                                  machining == QStringLiteral("keep")
                                      ? QString()
                                      : QStringLiteral(", machining ") +
                                            (machining == QStringLiteral("none")
                                                 ? QStringLiteral("none at end ") + end
                                                 : holesText(added)));
    return true;
  };
  return def;
}

// Flip and Rotate 90 deg: build_edit of each part picked, one undo step.
CommandDef turnCommand(DocumentHost& host, const CommandContext& context, bool rotate) {
  CommandDef def;
  def.id = rotate ? QStringLiteral("build.rotate") : QStringLiteral("build.flip");
  def.name = rotate ? QObject::tr("Rotate 90 deg") : QObject::tr("Flip");
  def.icon = rotate ? QStringLiteral("build-rotate") : QStringLiteral("flip");
  def.tooltip = rotate ? QObject::tr("Turns parts a quarter turn about their axes; Enter turns them again")
                       : QObject::tr("Turns parts end for end: the other end first");
  def.tab = kTab;
  def.group = QStringLiteral("MODIFY");
  def.pinned = true;
  def.keywords = rotate ? QStringList{QStringLiteral("turn"), QStringLiteral("90"), QStringLiteral("build")}
                        : QStringList{QStringLiteral("reverse"), QStringLiteral("other end"), QStringLiteral("build")};
  def.okLabel = rotate ? QObject::tr("Rotate") : QObject::tr("Flip");
  // Rotate turns again on each Enter.
  def.repeats = rotate;
  def.inputs = {selectionInput(QStringLiteral("parts"), QObject::tr("Parts"), kPartKinds, 1, 0)};
  def.enabled = [&context] { return build::hasBuildParts(context); };
  def.inspect = [rotate](const CommandState& state, const CommandContext& model) {
    Inspection result;
    const QVector<BuildPart> parts = build::partsOf(state.items(QStringLiteral("parts")), model);
    if (parts.isEmpty()) {
      result.error = QObject::tr("Pick parts of the BUILD tab.");
      return result;
    }
    QStringList lines;
    for (const BuildPart& part : parts) {
      const QString line = rotate ? QObject::tr("%1: %2 deg, then %3 deg")
                                        .arg(part.shownName())
                                        .arg(part.rotate)
                                        .arg((part.rotate + 90) % 360)
                                  : QObject::tr("%1: %2").arg(part.shownName(), part.flip ? QObject::tr("flipped, then as placed")
                                                                                          : QObject::tr("the other end first"));
      lines << line.toHtmlEscaped();
    }
    result.text = lines.join(QStringLiteral("<br>"));
    result.log = QStringLiteral("%1 part(s): %2").arg(parts.size()).arg(partsText(parts));
    return result;
  };
  def.apply = [&host, rotate](CommandState& state, const CommandContext& model) {
    const QVector<BuildPart> parts = build::partsOf(state.items(QStringLiteral("parts")), model);
    if (parts.isEmpty()) {
      host.showStatus(QObject::tr("Pick parts of the BUILD tab."), true);
      return false;
    }
    QJsonArray commands;
    for (const BuildPart& part : parts) {
      QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("build_edit")}, {QStringLiteral("component"), part.component}};
      if (rotate) {
        command.insert(QStringLiteral("rotate"), angleValue(QString::number((part.rotate + 90) % 360)));
      } else {
        command.insert(QStringLiteral("flip"), !part.flip);
      }
      commands.append(command);
    }
    const QString label = rotate ? QObject::tr("Rotate %1 90 deg").arg(partsText(parts))
                                 : QObject::tr("Flip %1").arg(partsText(parts));
    if (!host.runModelCommands(commands, label)) {
      return false;
    }
    qInfo().noquote() << label;
    return true;
  };
  return def;
}

// --- OUTPUT

CommandDef listCommand(const CommandContext& context, QWidget* window, std::function<QString()> design, bool cut) {
  CommandDef def;
  def.id = cut ? QStringLiteral("build.cut_list") : QStringLiteral("build.parts_list");
  def.name = cut ? QObject::tr("Cut List") : QObject::tr("Parts List");
  def.icon = cut ? QStringLiteral("build-cut-list") : QStringLiteral("parts-list");
  def.tooltip = cut ? QObject::tr("The parts to cut with their lengths, end angles, machining, supplier and part "
                                  "number, the stock they come from; Copy as CSV, Save as PDF")
                    : QObject::tr("Every part with its quantity, supplier and part number; Copy as CSV, Save as PDF");
  def.tab = kTab;
  def.group = QStringLiteral("OUTPUT");
  def.pinned = true;
  def.keywords = cut ? QStringList{QStringLiteral("saw"), QStringLiteral("lengths"), QStringLiteral("csv"),
                                   QStringLiteral("pdf"), QStringLiteral("build")}
                     : QStringList{QStringLiteral("bill of materials"), QStringLiteral("bom"), QStringLiteral("csv"),
                                   QStringLiteral("pdf"), QStringLiteral("build")};
  def.kind = CommandDef::Kind::Action;
  def.run = [&context, window, design, cut] {
    build::BuildListsDialog dialog(context, design(),
                                   cut ? build::BuildListsDialog::Page::CutList : build::BuildListsDialog::Page::PartsList,
                                   window);
    prepareModal(&dialog);
    execDialog(&dialog);
  };
  if (cut) {
    def.enabled = [&context] { return build::hasBuildParts(context); };
  }
  return def;
}

} // namespace

void registerBuildCommands(CommandRegistry& registry, const CommandContext& context, DocumentHost& host,
                           QWidget* window, std::function<QString()> designName) {
  const auto shared = std::make_shared<Shared>();
  // The part picker's window and the host its choices go through (mitcad#418).
  shared->window = window;
  shared->host = &host;
  // PROFILE
  registry.add(placeCommand(shared, host, QStringLiteral("profile")));
  registry.add(panelCommand(shared, host));
  registry.add(placeCommand(shared, host, QStringLiteral("linear")));
  // CONNECT
  registry.add(connectorCommand(shared, host, context));
  registry.add(fastenersCommand(shared, host, context));
  // MODIFY
  registry.add(lengthCommand(host, context));
  registry.add(endsCommand(host, context));
  registry.add(turnCommand(host, context, false));
  registry.add(turnCommand(host, context, true));
  // OUTPUT
  registry.add(listCommand(context, window, designName, true));
  registry.add(listCommand(context, window, designName, false));
}

} // namespace mitcad
