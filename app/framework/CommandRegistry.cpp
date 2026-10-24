// SPDX-License-Identifier: MIT
#include "CommandRegistry.hpp"

#include <QAction>
#include <QJsonObject>
#include <QSettings>
#include <QWidget>
#include <QtLogging>

#include "Icons.hpp"

namespace mitcad {
namespace {

const QString kShortcutGroup = QStringLiteral("shortcuts");

QString settingsKey(const QString& id) { return kShortcutGroup + QLatin1Char('/') + id; }

} // namespace

CommandRegistry::CommandRegistry(QObject* parent) : QObject(parent) {}

CommandRegistry::~CommandRegistry() = default;

void CommandRegistry::add(const CommandDef& def) {
  m_commands.push_back(std::make_unique<CommandDef>(def));
}

const CommandDef* CommandRegistry::find(const QString& id) const {
  const QString current = m_aliases.value(id, id);
  for (const auto& def : m_commands) {
    if (def->id == current) {
      return def.get();
    }
  }
  return nullptr;
}

void CommandRegistry::alias(const QString& oldId, const QString& id) {
  if (oldId == id) {
    return;
  }
  m_aliases.insert(oldId, id);
  // A shortcut saved under the former id (a custom one only: the default
  // is the command's own) is the new command's, unless that has one.
  QSettings settings;
  if (settings.contains(settingsKey(oldId))) {
    if (!settings.contains(settingsKey(id))) {
      settings.setValue(settingsKey(id), settings.value(settingsKey(oldId)));
    }
    settings.remove(settingsKey(oldId));
  }
}

const CommandDef* CommandRegistry::editorOf(const QJsonObject& feature) const {
  const QString type = feature.value(QStringLiteral("type")).toString();
  for (const auto& def : m_commands) {
    if (def->kind == CommandDef::Kind::Feature && !type.isEmpty() &&
        (def->featureType == type || def->editsAlso.contains(type)) && def->load &&
        (!def->canEdit || def->canEdit(feature))) {
      return def.get();
    }
  }
  return nullptr;
}

void CommandRegistry::createActions(QWidget* owner,
                                    const std::function<void(const QString&)>& trigger) {
  for (const auto& def : m_commands) {
    const QIcon icon = themeIcon(def->icon);
    if (icon.isNull()) {
      qWarning().noquote() << QStringLiteral("Command %1 has no icon '%2'").arg(def->id, def->icon);
    }
    auto* action = new QAction(icon, def->name, owner);
    action->setObjectName(QStringLiteral("command_") + def->id);
    const QString id = def->id;
    connect(action, &QAction::triggered, owner, [trigger, id] { trigger(id); });
    // Shortcuts work wherever the keyboard is in the window.
    action->setShortcutContext(Qt::WindowShortcut);
    owner->addAction(action);
    m_actions.insert(id, action);
  }
  refreshShortcuts();
}

QKeySequence CommandRegistry::defaultShortcut(const QString& id) const {
  const CommandDef* def = find(id);
  return def != nullptr ? def->shortcut : QKeySequence();
}

QKeySequence CommandRegistry::shortcut(const QString& id) const {
  const QSettings settings;
  if (settings.contains(settingsKey(id))) {
    return QKeySequence(settings.value(settingsKey(id)).toString(), QKeySequence::PortableText);
  }
  return defaultShortcut(id);
}

ShortcutBinding CommandRegistry::binding(const CommandDef& def, const QKeySequence& primary) const {
  unsigned contexts = def.mode == CommandDef::Mode::Sketch ? SketchShortcutContext : ModelShortcutContext;
  if (def.mode == CommandDef::Mode::Any) contexts = ModelShortcutContext | SketchShortcutContext;
  if (def.mode == CommandDef::Mode::Model && def.kind == CommandDef::Kind::Feature) {
    // Keep this in step with MainWindow::isAvailable(): Extrude/Revolve can
    // finish a sketch and start a feature, unlike model Move/Copy.
    const InputDef* input = def.primarySelection();
    if (input != nullptr && input->filter.testFlag(SelectKind::Profile)) contexts |= SketchShortcutContext;
  }
  if (def.duringCommands) contexts |= CommandShortcutContext;
  return {def.id, def.name, contexts, primary, def.shortcut, def.alternates, false};
}

QList<QKeySequence> CommandRegistry::effectiveShortcuts(const QString& id,
                                                        const QKeySequence& primary) const {
  const CommandDef* def = find(id);
  return def != nullptr ? binding(*def, primary).effectiveKeys() : QList<QKeySequence>();
}

QList<QKeySequence> CommandRegistry::effectiveShortcuts(const QString& id) const {
  return m_disabledShortcuts.contains(id) ? QList<QKeySequence>() : effectiveShortcuts(id, shortcut(id));
}

QString CommandRegistry::shortcutHint(const QString& id) const {
  const CommandDef* def = find(id);
  if (def == nullptr) return QString();
  const QList<QKeySequence> keys = effectiveShortcuts(id);
  return keys.isEmpty() ? def->name
                        : tr("%1 (%2)").arg(def->name, keys.front().toString(QKeySequence::NativeText));
}

QList<ShortcutConflict> CommandRegistry::shortcutConflicts(const QHash<QString, QKeySequence>& proposed) const {
  QList<ShortcutBinding> bindings = fixedShortcutBindings();
  for (const auto& def : m_commands) {
    bindings.append(binding(*def, proposed.contains(def->id) ? proposed.value(def->id) : shortcut(def->id)));
  }
  return mitcad::shortcutConflicts(bindings);
}

QList<ShortcutConflict> CommandRegistry::defaultShortcutConflicts() const {
  QHash<QString, QKeySequence> defaults;
  for (const auto& def : m_commands) defaults.insert(def->id, def->shortcut);
  return shortcutConflicts(defaults);
}

void CommandRegistry::setShortcut(const QString& id, const QKeySequence& sequence) {
  setShortcuts({{id, sequence}});
}

void CommandRegistry::setShortcuts(const QHash<QString, QKeySequence>& shortcuts) {
  QSettings settings;
  for (auto it = shortcuts.cbegin(); it != shortcuts.cend(); ++it) {
    const CommandDef* def = find(it.key());
    if (def == nullptr) continue;
    if (it.value() == def->shortcut) settings.remove(settingsKey(it.key()));
    else settings.setValue(settingsKey(it.key()), it.value().toString(QKeySequence::PortableText));
  }
  refreshShortcuts();
}

void CommandRegistry::refreshShortcuts() {
  const QList<ShortcutConflict> conflicts = shortcutConflicts();
  m_disabledShortcuts = conflictingShortcutIds(conflicts);
  m_shortcutWarnings.clear();
  for (const auto& conflict : conflicts) {
    m_shortcutWarnings.append(conflict.description());
  }
  for (const auto& def : m_commands) applyShortcut(*def);
  if (!m_shortcutWarnings.isEmpty()) {
    qWarning().noquote() << tr("Conflicting saved shortcuts are disabled; commands remain in menus. "
                               "Open Keyboard Shortcuts to repair them.")
                        << m_shortcutWarnings.join(QLatin1Char('\n'));
  }
}

void CommandRegistry::applyShortcut(const CommandDef& def) {
  QAction* action = m_actions.value(def.id);
  if (action == nullptr) return;
  const QList<QKeySequence> keys = effectiveShortcuts(def.id);
  action->setShortcuts(keys);
  QString tip = def.tooltip.isEmpty() ? def.name : QStringLiteral("%1: %2").arg(def.name, def.tooltip);
  if (!keys.isEmpty()) {
    tip += QStringLiteral(" (%1)").arg(keys.front().toString(QKeySequence::NativeText));
  } else if (m_disabledShortcuts.contains(def.id)) {
    tip += tr(" (conflicting shortcut disabled; change it in Keyboard Shortcuts)");
  }
  action->setToolTip(tip);
}

void CommandRegistry::updateEnabled(const std::function<bool(const CommandDef&)>& available) {
  for (const auto& def : m_commands) {
    if (QAction* action = m_actions.value(def->id)) {
      action->setEnabled(available(*def));
    }
  }
}

} // namespace mitcad
