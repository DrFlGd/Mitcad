// SPDX-License-Identifier: MIT
// The view's context menu outside sketches and commands (mitcad#494;
// app/COMMANDS.md, "The view's context menu"): the commands that fit the
// selection in submenus by kind (Solid, BUILD, Construct), in each first
// those the selection alone completes and the others under More; Delete
// for whole things (a BUILD member, its spacing set, a body, a component,
// the feature that made a face) and Hide (mitcad#347).
#include "MainWindow.hpp"

#include <algorithm>
#include <functional>
#include <memory>
#include <optional>
#include <utility>

#include <QAction>
#include <QMenu>
#include <QPair>
#include <QtLogging>

#include "browser/BrowserController.hpp"
#include "build/BuildModel.hpp"
#include "framework/CommandRegistry.hpp"
#include "framework/Icons.hpp"
#include "mitcad/geometry/import.hpp"

namespace mitcad {
namespace {

// The submenus, in the menu's order; Inspect's commands (Measure) are
// entries of the menu itself.
enum class Kind { Solid, Build, Construct, Inspect };

Kind kindOf(const CommandDef& def) {
  if (def.tab == QStringLiteral("BUILD")) {
    return Kind::Build;
  }
  if (def.group == QStringLiteral("CONSTRUCT")) {
    return Kind::Construct;
  }
  if (def.group == QStringLiteral("INSPECT")) {
    return Kind::Inspect;
  }
  return Kind::Solid;
}

bool onBody(const SelectionItem& item) {
  return item.kind == SelectKind::Face || item.kind == SelectKind::Edge || item.kind == SelectKind::Vertex ||
         item.kind == SelectKind::Body;
}

// What Property Sets shows for a picked item (mitcad#496): a body (also of
// a face, edge or vertex) with its occurrence, or a component with its
// occurrence; nothing for other items.
PropertyTarget propertyTarget(const SelectionItem& item) {
  if (onBody(item)) {
    return {item.owner, item.occurrence, QString()};
  }
  if (item.kind == SelectKind::Component) {
    return {QString(), item.occurrence, item.occurrence.isEmpty() ? item.owner : QString()};
  }
  return {};
}

// A plane to sketch on: a datum plane or a planar face.
bool isSketchPlane(const SelectionItem& item) {
  return item.kind == SelectKind::Plane ||
         (item.kind == SelectKind::Face && item.geometry == QStringLiteral("plane"));
}

// A submenu whose entries are logged as "Context menu <title>: A | B".
class Submenu {
public:
  Submenu(const QString& title, QMenu* parent) : m_menu(new QMenu(title, parent)) {}
  QMenu* menu() const { return m_menu; }
  bool isEmpty() const { return m_entries.isEmpty(); }
  void add(QAction* action) {
    m_menu->addAction(action);
    m_entries << action->text().remove(QLatin1Char('&'));
  }
  void addCall(const QString& text, std::function<void()> call) {
    QAction* action = m_menu->addAction(text);
    QObject::connect(action, &QAction::triggered, m_menu, std::move(call));
    m_entries << text;
  }
  void addMenu(QMenu* menu) {
    m_menu->addMenu(menu);
    m_entries << menu->title();
  }
  void log(const QString& name) const {
    qDebug().noquote() << QStringLiteral("Context menu %1: %2").arg(name, m_entries.join(QStringLiteral(" | ")));
  }

private:
  QMenu* m_menu;
  QStringList m_entries;
};

} // namespace

void MainWindow::fillModelContextMenu(QMenu& menu, QStringList& entries) {
  const auto add = [&menu, &entries](QAction* action) {
    if (menu.actions().contains(action)) {
      return; // offered already
    }
    menu.addAction(action);
    entries << action->text().remove(QLatin1Char('&'));
  };
  const auto addCall = [&menu, &add](const QString& icon, const QString& text, std::function<void()> call) {
    QAction* action = new QAction(themeIcon(icon), text, &menu);
    connect(action, &QAction::triggered, &menu, std::move(call));
    add(action);
  };
  const auto addSubmenu = [&menu, &entries](const Submenu& submenu) {
    menu.addMenu(submenu.menu());
    entries << submenu.menu()->title();
  };

  if (const CommandDef* last = m_registry->find(m_lastCommand); last && isAvailable(*last)) {
    const QString id = last->id;
    addCall(last->icon, tr("Repeat %1").arg(last->name), [this, id] { trigger(id); });
    menu.addSeparator();
  }
  if (m_selection.isEmpty()) {
    add(m_registry->action(QStringLiteral("edit.undo")));
    return;
  }
  const Selection selection = m_selection;
  const SelectionItem& first = selection.first();

  // The BUILD parts the items belong to: the BUILD submenu is only theirs.
  QVector<build::BuildPart> parts;
  bool allParts = true;
  for (const SelectionItem& item : selection) {
    const std::optional<build::BuildPart> part = onBody(item) ? build::partOf(item, model()) : std::nullopt;
    allParts = allParts && part.has_value();
    if (part && std::none_of(parts.begin(), parts.end(),
                             [&part](const build::BuildPart& other) { return other.component == part->component; })) {
      parts.append(*part);
    }
  }
  const bool surfaces = std::all_of(selection.begin(), selection.end(), [this](const SelectionItem& item) {
    const std::shared_ptr<geometry::Shape> shape = onBody(item) ? bodyShape(item.owner) : nullptr;
    return shape && geometry::body_kind(shape->occt()) == geometry::BodyKind::Sheet;
  });

  // The feature commands whose first selection input takes everything
  // selected, by kind, in the registry's (the ribbon's) order.
  QVector<QPair<const CommandDef*, bool>> fitting[4];
  for (const auto& def : m_registry->commands()) {
    const InputDef* input = def->primarySelection();
    if (def->kind != CommandDef::Kind::Feature || input == nullptr || !isAvailable(*def) ||
        !std::all_of(selection.begin(), selection.end(),
                     [input](const SelectionItem& s) { return input->takes(s); }) ||
        (def->surfaceBodiesOnly && !surfaces) || (def->offeredFor && !def->offeredFor(selection))) {
      continue;
    }
    const Kind kind = kindOf(*def);
    if (kind == Kind::Build && !allParts) {
      continue;
    }
    fitting[static_cast<int>(kind)].append({def.get(), def->completedBy(selection)});
  }
  // `key` names the submenu in the log, "Context menu Solid: A | More" and
  // "Context menu Solid More: B | C".
  const auto addKind = [this, &menu, &addSubmenu, &fitting](Kind kind, const QString& title, const QString& key) {
    const QVector<QPair<const CommandDef*, bool>>& commands = fitting[static_cast<int>(kind)];
    if (commands.isEmpty()) {
      return;
    }
    Submenu submenu(title, &menu);
    Submenu more(tr("More"), submenu.menu());
    for (const QPair<const CommandDef*, bool>& command : commands) {
      (command.second ? submenu : more).add(m_registry->action(command.first->id));
    }
    if (!more.isEmpty()) {
      if (!submenu.isEmpty()) {
        submenu.menu()->addSeparator();
      }
      submenu.addMenu(more.menu());
    }
    addSubmenu(submenu);
    submenu.log(key);
    if (!more.isEmpty()) {
      more.log(key + QStringLiteral(" More"));
    }
  };
  addKind(Kind::Solid, tr("Solid"), QStringLiteral("Solid"));
  addKind(Kind::Build, tr("Build"), QStringLiteral("Build"));
  addKind(Kind::Construct, tr("Construct"), QStringLiteral("Construct"));
  menu.addSeparator();

  if (selection.size() == 1 && isSketchPlane(first)) {
    add(m_registry->action(QStringLiteral("sketch.create")));
  }
  const QString feature = first.creatingFeature();
  QString name;
  if (selection.size() == 1 && !feature.isEmpty() && editorOf(feature, &name) != nullptr) {
    addCall(QStringLiteral("edit"), tr("Edit %1").arg(name), [this, feature] { editFeature(feature); });
  }
  const QString sketch = sketchOf(first);
  if (selection.size() == 1 && !sketch.isEmpty()) {
    addCall(QStringLiteral("sketch"), tr("Edit Sketch"), [this, sketch] { editSketch(sketch); });
    addCall(QStringLiteral("export"), tr("Save As DXF"), [this, sketch] { exportSketch(sketch); });
  }
  for (const QPair<const CommandDef*, bool>& command : fitting[static_cast<int>(Kind::Inspect)]) {
    add(m_registry->action(command.first->id));
  }
  // Appearances of the bodies selected or picked on (mitcad#46).
  if (const Selection targets = appearanceTargets(); !targets.isEmpty()) {
    addCall(QStringLiteral("appearance"), tr("Appearance…"), [this, targets] { m_controller->openAppearances(targets); });
    // The bodies' material (mitcad#640): a face stands for its body.
    QStringList bodies;
    for (const SelectionItem& target : targets) {
      if (!bodies.contains(target.owner)) {
        bodies << target.owner;
      }
    }
    addCall(QStringLiteral("material"), tr("Material…"), [this, bodies] { m_controller->openMaterial(bodies); });
  } else if (selection.size() == 1 && first.kind == SelectKind::Component) {
    // A component's default material (mitcad#640).
    const QString component = first.owner;
    addCall(QStringLiteral("material"), tr("Material…"),
            [this, component] { m_controller->openMaterial({}, component); });
  }
  // The properties of the first item's body, occurrence and component
  // (mitcad#496).
  if (const PropertyTarget target = propertyTarget(first); !target.isEmpty()) {
    addCall(QStringLiteral("parameters"), tr("Property Sets…"),
            [this, target] { m_controller->openPropertySets(target); });
  }

  // Delete: whole things, each shown when the selection has it, each one
  // undo step. A BUILD part goes as a member (with what it added), never
  // by the features that make it.
  const QStringList bodies = BrowserController::hideableBodies(selection);
  const QStringList occurrences = BrowserController::hideableOccurrences(selection);
  if (!windowReadOnly()) {
    Submenu remove(tr("Delete"), &menu);
    if (!parts.isEmpty()) {
      QStringList members;
      QStringList sets;
      for (const build::BuildPart& part : parts) {
        members << part.component;
        const QString lead = part.set.isEmpty() && !part.spacing.isEmpty() ? part.component : part.set;
        if (!lead.isEmpty() && !sets.contains(lead)) {
          sets << lead;
        }
      }
      remove.addCall(members.size() == 1 ? tr("Delete Member") : tr("Delete %1 Members").arg(members.size()),
                     [this, members] { deleteBuildParts(members, false); });
      if (!sets.isEmpty()) {
        remove.addCall(sets.size() == 1 ? tr("Delete Spacing Set") : tr("Delete %1 Spacing Sets").arg(sets.size()),
                       [this, sets] { deleteBuildParts(sets, true); });
      }
    }
    if (!bodies.isEmpty()) {
      remove.addCall(bodies.size() == 1 ? tr("Delete Body") : tr("Delete %1 Bodies").arg(bodies.size()),
                     [this, selection] { m_controller->deleteBodies(selection); });
    }
    if (!occurrences.isEmpty()) {
      remove.addCall(occurrences.size() == 1 ? tr("Delete Component")
                                             : tr("Delete %1 Components").arg(occurrences.size()),
                     [this, selection] { m_controller->deleteOccurrences(selection); });
    }
    // The feature that made the items, when one did and it is in the
    // timeline (a face, an edge, a body; mitcad#28's feature).
    QStringList features;
    for (const SelectionItem& item : selection) {
      const QString uid = item.kind == SelectKind::Feature ? item.owner : item.creatingFeature();
      if (!features.contains(uid)) {
        features << uid;
      }
    }
    if (parts.isEmpty() && features.size() == 1 && m_snapshot.feature(features.first()) != nullptr) {
      const QString uid = features.first();
      remove.addCall(tr("Delete Feature"), [this, uid] { m_controller->deleteFeature(uid); });
    }
    if (!remove.isEmpty()) {
      addSubmenu(remove);
      remove.log(QStringLiteral("Delete"));
    }
  }
  // Hide Body and Hide Component: the bodies of the faces, edges, vertices
  // and bodies selected, and the occurrences that place them (mitcad#347).
  if (!bodies.isEmpty()) {
    Submenu hide(tr("Hide"), &menu);
    hide.addCall(bodies.size() == 1 ? tr("Hide Body") : tr("Hide %1 Bodies").arg(bodies.size()),
                 [this, selection] { m_controller->hideBodies(selection); });
    if (!occurrences.isEmpty()) {
      hide.addCall(occurrences.size() == 1 ? tr("Hide Component") : tr("Hide %1 Components").arg(occurrences.size()),
                   [this, selection] { m_controller->hideOccurrences(selection); });
    }
    addSubmenu(hide);
    hide.log(QStringLiteral("Hide"));
  }
  // A feature selected as its faces (mitcad#28) has no row of its own.
  if (selection.size() == 1 && first.kind != SelectKind::Feature && (onBody(first) || !feature.isEmpty())) {
    addCall(QStringLiteral("browser"), tr("Find in Browser"), [this, first] { m_controller->findInBrowser(first); });
  }
  menu.addSeparator();
  addCall(QString(), tr("Clear Selection"), [this] { setSelection({}); });
  add(m_registry->action(QStringLiteral("edit.undo")));
}

void MainWindow::deleteBuildParts(const QStringList& components, bool sets) {
  if (components.isEmpty()) {
    return;
  }
  if (!canChangeModel()) {
    const QString readOnly = readOnlyReason();
    showStatus(readOnly.isEmpty() ? changeBlockedReason() : readOnly, true);
    return;
  }
  QJsonArray commands;
  QStringList names;
  const QVector<build::BuildPart> parts = build::buildParts(model());
  for (const QString& component : components) {
    QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("build_remove")},
                        {QStringLiteral("component"), component}};
    if (sets) {
      command.insert(QStringLiteral("set"), true);
    }
    commands.append(command);
    const auto part = std::find_if(parts.begin(), parts.end(),
                                   [&component](const build::BuildPart& p) { return p.component == component; });
    names << (part != parts.end() ? part->shownName() : component);
  }
  const QString label = sets ? tr("Delete Spacing Sets") : tr("Delete Members");
  if (runModelCommands(commands, label)) {
    qDebug().noquote() << QStringLiteral("Deleted %1 %2")
                              .arg(sets ? QStringLiteral("spacing set of") : QStringLiteral("member"),
                                   names.join(QStringLiteral(", ")));
  }
}

} // namespace mitcad
