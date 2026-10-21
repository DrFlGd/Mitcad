// SPDX-License-Identifier: MIT
// Emboss (mitcad#458; commands.md, "emboss"): sketch profiles, texts too,
// raised from or sunk into a planar face, or wrapped onto a cylindrical
// one.
#include <QJsonArray>

#include "CommandFactories.hpp"
#include "CommandSupport.hpp"

namespace mitcad::cmd {
namespace {

const Choices kModes = {{QStringLiteral("raise"), QStringLiteral("Raise")},
                        {QStringLiteral("sink"), QStringLiteral("Sink")}};

// A planar face, or a cylindrical or conical one to wrap onto (cones:
// mitcad#473).
bool takesEmbossFace(const SelectionItem& item) {
  return item.kind == SelectKind::Face &&
         (item.geometry == QStringLiteral("plane") || item.geometry == QStringLiteral("cylinder") ||
          item.geometry == QStringLiteral("cone"));
}

} // namespace

CommandDef embossCommand(const CommandContext& context) {
  CommandDef def;
  def.id = QStringLiteral("solid.emboss");
  def.name = QObject::tr("Emboss");
  def.icon = QStringLiteral("emboss");
  def.tooltip = QObject::tr("Raises or sinks sketch profiles on a flat, cylindrical or conical face");
  def.group = QStringLiteral("CREATE");
  def.keywords = {QStringLiteral("deboss"), QStringLiteral("engrave"), QStringLiteral("wrap"),
                  QStringLiteral("text"), QStringLiteral("label")};
  def.featureType = QStringLiteral("emboss");
  def.inputs = {
      selectionInput(QStringLiteral("profiles"), QObject::tr("Profiles"), SelectKind::Profile, 1, 0)
          .withTooltip(QObject::tr("Profiles of one sketch; a text's letters are profiles too")),
      selectionInput(QStringLiteral("face"), QObject::tr("Face"), SelectKind::Face, 0, 1)
          .withTooltip(QObject::tr("A planar face, or a cylindrical or conical face the profiles wrap "
                                   "onto; none: the sketch plane"))
          .withAccepts(takesEmbossFace)
          .withoutMeshes(),
      valueInput(QStringLiteral("depth"), QObject::tr("Depth"), ValueKind::Length, QStringLiteral("1 mm")),
      choiceInput(QStringLiteral("mode"), QObject::tr("Raise or Sink"), kModes, QStringLiteral("raise"))
          .withTooltip(QObject::tr("Out of the material (joined) or into it (cut)")),
      selectionInput(QStringLiteral("objects"), QObject::tr("Objects"), SelectKind::Body, 0, 0)
          .withTooltip(QObject::tr("The bodies it joins or cuts; none: the face's body, or all"))
          .withoutMeshes(),
  };
  def.init = [](CommandState& state, const CommandContext& model) { newestProfile(state, model); };
  def.enabled = [&context] { return hasProfiles(context) && hasBodies(context); };

  def.build = [](const CommandState& state, const CommandContext&) {
    Built result;
    const Selection& profiles = state.items(QStringLiteral("profiles"));
    if (oneSketch(profiles, result, QStringLiteral("profiles")).isEmpty()) {
      return result;
    }
    result.def = {{QStringLiteral("type"), QStringLiteral("emboss")},
                  {QStringLiteral("profiles"), refsOf(profiles)},
                  {QStringLiteral("depth"), state.expression(QStringLiteral("depth"))}};
    const Selection& face = state.items(QStringLiteral("face"));
    if (!face.isEmpty()) {
      result.def.insert(QStringLiteral("face"), face.first().reference());
    }
    if (state.choice(QStringLiteral("mode")) == QStringLiteral("sink")) {
      result.def.insert(QStringLiteral("mode"), QStringLiteral("sink"));
    }
    const Selection& objects = state.items(QStringLiteral("objects"));
    if (!objects.isEmpty()) {
      result.def.insert(QStringLiteral("participants"), bodyUids(objects));
    }
    return result;
  };

  def.load = [](const QJsonObject& feature, CommandState& state, const CommandContext& context) {
    state.setItems(QStringLiteral("profiles"), profilesOf(feature.value(QStringLiteral("profiles")).toArray()));
    const SelectionItem face = itemOf(feature.value(QStringLiteral("face")), context);
    if (face.isValid()) {
      state.setItems(QStringLiteral("face"), {face});
    }
    loadValue(state, QStringLiteral("depth"), feature.value(QStringLiteral("depth")), feature, context);
    state.setChoice(QStringLiteral("mode"), feature.value(QStringLiteral("mode")).toString(QStringLiteral("raise")));
    state.setItems(QStringLiteral("objects"), bodiesOf(feature.value(QStringLiteral("participants")).toArray()));
  };

  // A face the panel cannot show (none of the bodies before the feature
  // has it) would be dropped by OK.
  def.canEdit = [&context](const QJsonObject& feature) {
    const QJsonValue face = feature.value(QStringLiteral("face"));
    return face.isUndefined() || face.isNull() || isShowable(face, context);
  };

  def.describe = [](const CommandState& state, const CommandContext&) {
    const Selection& face = state.items(QStringLiteral("face"));
    const QString on = face.isEmpty() ? QStringLiteral("the sketch plane")
                       : face.first().geometry == QStringLiteral("cylinder")
                           ? QStringLiteral("a cylindrical face, wrapped")
                       : face.first().geometry == QStringLiteral("cone") ? QStringLiteral("a conical face, wrapped")
                                                                         : QStringLiteral("a planar face");
    return QStringLiteral("Added emboss (%1) of %2 profile(s) on %3, depth %4")
        .arg(label(kModes, state.choice(QStringLiteral("mode"))))
        .arg(state.items(QStringLiteral("profiles")).size())
        .arg(on)
        .arg(number(state.value(QStringLiteral("depth"))));
  };
  return def;
}

} // namespace mitcad::cmd
