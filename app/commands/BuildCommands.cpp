// SPDX-License-Identifier: MIT
// The BUILD tab (mitcad#375; the model's side in commands.md, "BUILD:
// profile assemblies"; app/COMMANDS.md, "BUILD"): frames of slot profiles
// with their connectors, panels and linear parts.
//
//   PROFILE  Profile, Panel, Linear Part: a searchable picker of the
//            library's parts, then clicks; Place (Enter) keeps the panel
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

#include <QByteArray>
#include <QHash>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QStringList>
#include <QWidget>
#include <QtLogging>

#include <gp_Vec.hxx>

#include "../browser/DocumentHost.hpp"
#include "../build/BuildCatalog.hpp"
#include "../build/BuildLists.hpp"
#include "../build/BuildModel.hpp"
#include "../files/Libraries.hpp"
#include "../framework/CommandRegistry.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Json.hpp"
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
  // build_junctions, and each junction's label for the choice.
  QJsonArray junctions;
  Choices junctionChoices;
  // The design's BUILD parts and which component each occurrence path places.
  QVector<BuildPart> parts;
  QHash<QString, QString> components;
  // The part placed last, per role: the picker starts with it.
  QHash<QString, QString> lastPart;
  // The configuration rows of a source's component (Fasteners' sizes).
  QHash<QString, Choices> rows;
  // The library's preview image of a part, by "<library>|<component>",
  // read once (library_preview); null when it has none.
  QHash<QString, QImage> thumbnails;

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

  void setPreview(const build::Frame& shown, double shownLength) {
    const std::lock_guard<std::mutex> lock(mutex);
    frame = shown;
    length = shownLength;
  }
  std::pair<build::Frame, double> preview() const {
    const std::lock_guard<std::mutex> lock(mutex);
    return {frame, length};
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

// The first part the search and the system leave, when the chosen one is
// not among them any more.
void followFilter(const Shared& shared, const QString& role, CommandState& state) {
  const QVector<Part> parts = shared.catalog.parts(role);
  const Choices shown = build::partChoices(parts, state.text(kSearch), state.choice(kSystem), QString());
  const bool kept = std::any_of(shown.begin(), shown.end(),
                                [&state](const auto& option) { return option.first == state.choice(kPart); });
  if (!kept && !shown.isEmpty()) {
    state.setChoice(kPart, shown.first().first);
  }
}

QVector<InputDef> pickerInputs(const SharedPtr& shared, const QString& role) {
  return {
      textInput(kSearch, QObject::tr("Search"))
          .withTooltip(QObject::tr("Words of the part's name, size, system, slots, supplier or part number"))
          .withOnChange([shared, role](CommandState& state, const CommandContext&) { followFilter(*shared, role, state); }),
      choiceInput(kSystem, QObject::tr("System"), {}, QString())
          .withChoices([shared, role](const CommandState&) { return build::systemChoices(shared->catalog.parts(role)); })
          .withOnChange([shared, role](CommandState& state, const CommandContext&) { followFilter(*shared, role, state); })
          .withTooltip(QObject::tr("The profile system: type, slot width and grid (B8 30: B-type slot 8, 30 mm)")),
      choiceInput(kPart, QObject::tr("Part"), {}, QString())
          .withChoices([shared, role](const CommandState& state) {
            return build::partChoices(shared->catalog.parts(role), state.text(kSearch), state.choice(kSystem),
                                      state.choice(kPart));
          })
          .withTooltip(QObject::tr("The library's parts that match, with their supplier and part number")),
  };
}

// The picker's parts read again; the part chosen last stays, else the one
// placed last, else the first.
void startPicker(Shared& shared, const QString& role, CommandState& state, const CommandContext& context) {
  shared.catalog.refresh(role, context);
  shared.refreshDesign(context);
  const QVector<Part> parts = shared.catalog.parts(role);
  Part known;
  if (!state.choice(kPart).isEmpty() && shared.catalog.find(state.choice(kPart), known)) {
    return;
  }
  const QString last = shared.lastPart.value(role);
  if (!last.isEmpty() && shared.catalog.find(last, known)) {
    state.setChoice(kPart, last);
  } else if (!parts.isEmpty()) {
    state.setChoice(kPart, parts.first().key);
  }
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

const Choices kPlaces = {{QStringLiteral("auto"), QStringLiteral("Automatic: the click decides")},
                         {QStringLiteral("side"), QStringLiteral("On the side")},
                         {QStringLiteral("end"), QStringLiteral("On the end")},
                         {QStringLiteral("grounded"), QStringLiteral("Grounded at the origin (no pick)")}};
// An end placement's axis (at.direction): along the target, or turned to
// an axis of the active component (a corner).
const Choices kDirections = {{QStringLiteral("along"), QStringLiteral("Along the part it meets")},
                             {QStringLiteral("+x"), QStringLiteral("+X")}, {QStringLiteral("-x"), QStringLiteral("-X")},
                             {QStringLiteral("+y"), QStringLiteral("+Y")}, {QStringLiteral("-y"), QStringLiteral("-Y")},
                             {QStringLiteral("+z"), QStringLiteral("+Z")}, {QStringLiteral("-z"), QStringLiteral("-Z")}};
const Choices kEnds = {{QStringLiteral("a"), QStringLiteral("End A")}, {QStringLiteral("b"), QStringLiteral("End B")}};
const QString kTyped = QStringLiteral("typed");
const Choices kLengthModes = {{QStringLiteral("face"), QStringLiteral("Auto: to the face")},
                              {QStringLiteral("centre"), QStringLiteral("Auto: to the centre")},
                              {QStringLiteral("through"), QStringLiteral("Auto: through")},
                              {kTyped, QStringLiteral("Typed length")}};
const QStringList kAutoModes = {QStringLiteral("face"), QStringLiteral("centre"), QStringLiteral("through")};
const Choices kMitres = {{QStringLiteral("none"), QStringLiteral("None")},
                         {QStringLiteral("new"), QStringLiteral("Cut the new part")},
                         {QStringLiteral("existing"), QStringLiteral("Cut the existing part")},
                         {QStringLiteral("shared"), QStringLiteral("Shared: both on the bisector")}};
const Choices kRotations = {{QStringLiteral("0"), QStringLiteral("0 deg")},
                            {QStringLiteral("90"), QStringLiteral("90 deg")},
                            {QStringLiteral("180"), QStringLiteral("180 deg")},
                            {QStringLiteral("270"), QStringLiteral("270 deg")}};
const Choices kTilts = {{QStringLiteral("x"), QStringLiteral("About x")},
                        {QStringLiteral("-x"), QStringLiteral("About x, the other way")},
                        {QStringLiteral("y"), QStringLiteral("About y")},
                        {QStringLiteral("-y"), QStringLiteral("About y, the other way")}};

// What `build_place` (and its preview) gets from the panel, the auto
// length's target found.
struct Placement {
  QJsonObject fields;
  QString problem;
  QJsonObject target;     // what the auto length runs to, or empty
  QString lengthNote;     // how the length was chosen, for the panel
};

QString angleValue(const QString& degrees) { return degrees + QStringLiteral(" deg"); }

// The panel's fields; with an auto length the preview names its target
// first (commands.md, `build_preview`'s `length.target`), and the length
// binds to it.
Placement placement(const CommandState& state, const CommandContext& context, const Part& part, bool profile) {
  Placement result;
  QJsonObject fields{{QStringLiteral("part"), part.ref()}};
  const QString place = state.choice(QStringLiteral("place"));
  std::optional<QJsonObject> at;
  if (place == QStringLiteral("grounded")) {
    // No geometry: grounded at the active component's origin along its +Z.
    at = QJsonObject();
  } else {
    at = build::atOf(QStringLiteral("at"), state, context, result.problem);
    if (!at) {
      return result;
    }
    at->insert(QStringLiteral("place"), place);
  }
  // On an end, the new part's axis turned (a corner): in the active
  // component's coordinates.
  const QString direction = state.choice(QStringLiteral("direction"));
  if (place != QStringLiteral("side") && place != QStringLiteral("grounded") && !direction.isEmpty() &&
      direction != QStringLiteral("along")) {
    const double sign = direction.startsWith(QLatin1Char('-')) ? -1.0 : 1.0;
    const QChar axis = direction.at(direction.size() - 1);
    at->insert(QStringLiteral("direction"), QJsonArray{axis == QLatin1Char('x') ? sign : 0.0,
                                                       axis == QLatin1Char('y') ? sign : 0.0,
                                                       axis == QLatin1Char('z') ? sign : 0.0});
  }
  if (state.checked(QStringLiteral("use_offset")) && place != QStringLiteral("grounded")) {
    at->insert(QStringLiteral("offset"), state.expression(QStringLiteral("offset")));
    at->insert(QStringLiteral("from"), state.choice(QStringLiteral("from")));
  }
  fields.insert(QStringLiteral("at"), *at);
  fields.insert(QStringLiteral("rotate"), angleValue(state.choice(QStringLiteral("rotate"))));
  fields.insert(QStringLiteral("flip"), state.checked(QStringLiteral("flip")));
  const QString mitre = state.choice(QStringLiteral("mitre"));
  if (profile && !mitre.isEmpty() && mitre != QStringLiteral("none")) {
    QJsonObject cut{{QStringLiteral("cut"), mitre}};
    if (mitre != QStringLiteral("shared")) {
      cut.insert(QStringLiteral("angle"), state.expression(QStringLiteral("mitre_angle")));
    }
    fields.insert(QStringLiteral("mitre"), cut);
  }
  if (part.cutToLength()) {
    const QString mode = state.choice(QStringLiteral("length_mode"));
    if (mode == kTyped) {
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
        // Nothing in the way (the first part): the typed length.
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

// The inspection of a placement: what build_preview says, the new part's
// shape (the model's, else its section along the frame), the auto
// length's target highlighted and the length at the free end.
Inspection inspectPlacement(const SharedPtr& shared, const QString& role, const CommandState& state,
                            const CommandContext& context, bool profile) {
  Inspection result;
  Part part;
  if (!chosenPart(*shared, state, part)) {
    result.error = noPartsText(*shared, role);
    return result;
  }
  const Placement placed = placement(state, context, part, profile);
  if (!placed.problem.isEmpty()) {
    result.error = placed.problem;
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
  shared->setPreview(frame, length);
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
  QStringList lines;
  lines << QStringLiteral("<b>%1</b>").arg(part.name.toHtmlEscaped());
  const QString seller = QStringList{part.supplier, part.partNumber}.join(QLatin1Char(' ')).trimmed();
  if (!seller.isEmpty()) {
    lines << seller.toHtmlEscaped();
  }
  if (!place.isEmpty()) {
    lines << (place == QStringLiteral("end")        ? QObject::tr("On the end")
              : place == QStringLiteral("side")     ? QObject::tr("On the side")
              : place == QStringLiteral("frame")    ? QObject::tr("On the part's frame")
              : place == QStringLiteral("grounded") ? QObject::tr("Grounded at the origin")
                                                    : QObject::tr("On the picked geometry"));
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
  result.text = lines.join(QStringLiteral("<br>"));
  result.error = problemsOf(answer);
  result.log = QStringLiteral("place %1, length %2 mm (%3%4), %5")
                   .arg(place.isEmpty() ? QStringLiteral("first") : place,
                        std::isfinite(length) ? QString::number(length, 'f', 2) : QStringLiteral("?"),
                        placed.lengthNote,
                        placed.target.isEmpty() ? QString()
                                                : QStringLiteral(" to ") + targetText(placed.target, context),
                        endsText(ends));
  // The length at the free end ("the length box"), and the slot's snaps.
  QJsonArray markers;
  if (frame.valid && std::isfinite(length) && part.cutToLength()) {
    markers.append(marker(QStringLiteral("%1 mm").arg(length, 0, 'f', 1),
                          frame.origin.Translated(gp_Vec(frame.z) * length)));
  }
  int snaps = 0;
  for (const QJsonValue& value : answer.value(QStringLiteral("snaps")).toArray()) {
    const QJsonObject snap = value.toObject();
    const QJsonArray point = snap.value(QStringLiteral("point")).toArray();
    if (point.size() >= 3 && snaps < 12) {
      markers.append(marker(str(snap, "label").isEmpty() ? str(snap, "kind") : str(snap, "label"),
                            gp_Pnt(point[0].toDouble(), point[1].toDouble(), point[2].toDouble())));
      ++snaps;
    }
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

// The inputs where and how a part goes, after the picker.
QVector<InputDef> placementInputs(const SharedPtr& shared, bool profile) {
  const auto cut = [shared](const CommandState& state) { return shownForCut(*shared, state); };
  const auto offset = [](const CommandState& state) { return state.checked(QStringLiteral("use_offset")); };
  const auto picked = [](const CommandState& state) {
    return state.choice(QStringLiteral("place")) != QStringLiteral("grounded");
  };
  QVector<InputDef> inputs = {
      selectionInput(QStringLiteral("at"), QObject::tr("Place On"),
                     SelectKind::Face | SelectKind::Plane | SelectKind::Point | SelectKind::Vertex, 1, 1)
          .withOnPick([](const SelectionItem& item, CommandState& state, const CommandContext& context) {
            build::recordPick(QStringLiteral("at"), item, state, context);
          },
                      false)
          .withVisible(picked)
          .withTooltip(QObject::tr("The first part: a plane, a point or a face. Then a profile: near an end on its "
                                   "side, or its end face")),
      choiceInput(QStringLiteral("place"), QObject::tr("Placement"), kPlaces, QStringLiteral("auto"))
          .withTooltip(QObject::tr("On a profile's side (along its slot) or on its end; Automatic: where the "
                                   "click was; Grounded: at the origin, without a pick")),
      choiceInput(QStringLiteral("direction"), QObject::tr("Direction"), kDirections, QStringLiteral("along"))
          .withVisible([](const CommandState& state) {
            const QString place = state.choice(QStringLiteral("place"));
            return place == QStringLiteral("auto") || place == QStringLiteral("end");
          })
          .withTooltip(QObject::tr("On an end: along the part it meets, or turned to an axis (a corner, with a "
                                   "shared mitre)")),
      checkInput(QStringLiteral("use_offset"), QObject::tr("Offset from an End"), false),
      valueInput(QStringLiteral("offset"), QObject::tr("Offset"), ValueKind::Length, QStringLiteral("0 mm"))
          .withVisible(offset)
          .withTooltip(QObject::tr("Along the profile it goes on, from the end below")),
      choiceInput(QStringLiteral("from"), QObject::tr("From"), kEnds, QStringLiteral("a")).withVisible(offset),
      choiceInput(QStringLiteral("length_mode"), QObject::tr("Length"), kLengthModes, QStringLiteral("face"))
          .withVisible(cut)
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
          })
          .withTooltip(QObject::tr("Auto: up to the part in the way (bound: it follows that part); Tab steps "
                                   "face, centre, through")),
      valueInput(QStringLiteral("length"), QObject::tr("Typed Length"), ValueKind::Length, QStringLiteral("500 mm"))
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
            if (state.choice(QStringLiteral("length_mode")) != kTyped) {
              return std::nullopt;
            }
            const build::Frame frame = shared->preview().first;
            if (!frame.valid) {
              return std::nullopt;
            }
            return cmd::arrow(frame.origin, frame.z);
          })
          .withTooltip(QObject::tr("The length when nothing is in the way, or typed; drag its handle at the free end")),
  };
  if (profile) {
    const auto mitred = [](const CommandState& state) {
      const QString mitre = state.choice(QStringLiteral("mitre"));
      return mitre == QStringLiteral("new") || mitre == QStringLiteral("existing");
    };
    inputs.append(choiceInput(QStringLiteral("mitre"), QObject::tr("Mitre"), kMitres, QStringLiteral("none"))
                      .withVisible(cut)
                      .withTooltip(QObject::tr("On an end: the existing part, the new one or both (shared); on a "
                                               "side only the new part")));
    inputs.append(valueInput(QStringLiteral("mitre_angle"), QObject::tr("Mitre Angle"), ValueKind::Angle,
                             QStringLiteral("45 deg"))
                      .withVisible(mitred));
  }
  inputs.append(choiceInput(QStringLiteral("rotate"), QObject::tr("Rotate"), kRotations, QStringLiteral("0"))
                    .withTooltip(QObject::tr("About the part's axis, in 90 degree steps")));
  inputs.append(flipInput(QStringLiteral("flip"), QObject::tr("Flip")));
  return inputs;
}

CommandDef placeCommand(const SharedPtr& shared, DocumentHost& host, const QString& role) {
  const bool profile = role == QStringLiteral("profile");
  CommandDef def;
  def.id = profile ? QStringLiteral("build.profile") : QStringLiteral("build.linear");
  def.name = profile ? QObject::tr("Profile") : QObject::tr("Linear Part");
  def.icon = profile ? QStringLiteral("build-profile") : QStringLiteral("build-linear");
  def.tooltip = profile ? QObject::tr("Places slot profiles: pick one, click where it goes, its length runs to the "
                                      "part in the way; Enter places it and the next one follows")
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
  def.inputs += placementInputs(shared, profile);
  def.init = [shared, role](CommandState& state, const CommandContext& context) {
    startPicker(*shared, role, state, context);
  };
  def.inspect = [shared, role, profile](const CommandState& state, const CommandContext& context) {
    return inspectPlacement(shared, role, state, context, profile);
  };
  def.previewMarkers = markersOf;
  def.apply = [shared, &host, role, profile](CommandState& state, const CommandContext& context) {
    Part part;
    if (!chosenPart(*shared, state, part)) {
      host.showStatus(noPartsText(*shared, role), true);
      return false;
    }
    const Placement placed = placement(state, context, part, profile);
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
    state.setItems(QStringLiteral("at"), {});
    state.setObject(QStringLiteral("at_pick"), {});
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
  };
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
            state.setChoice(kJunction, nearestJunction(*shared, build::pickPoint(QStringLiteral("at"), state, model)));
            chooseConnector(*shared, state);
          },
                      false)
          .withTooltip(QObject::tr("A click on a profile near a junction chooses that junction")),
      choiceInput(kJunction, QObject::tr("Junction"), {}, QString())
          .withChoices([shared](const CommandState&) { return shared->junctionOptions(); })
          .withOnChange([shared](CommandState& state, const CommandContext&) { chooseConnector(*shared, state); }),
      choiceInput(kPart, QObject::tr("Connector"), {}, QString())
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
          .withTooltip(QObject::tr("The connectors that fit the junction's systems and room, best first")),
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
  };
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
        const std::optional<QString> path = build::pathFromActive(targets.first(), model);
        if (!path) {
          host.showStatus(QObject::tr("%1 is not in the active component.").arg(targets.first().describe()), true);
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
