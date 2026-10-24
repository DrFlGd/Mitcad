// SPDX-License-Identifier: MIT
#pragma once

#include <functional>
#include <memory>
#include <vector>

#include <QHash>
#include <QKeySequence>
#include <QObject>
#include <QString>

#include "Command.hpp"
#include "ShortcutBindings.hpp"

class QAction;
class QWidget;

namespace mitcad {

// Every command of the application with its action: the toolbar, the menus,
// the command search and the context menu use the same actions, and their
// shortcuts can be changed (saved in the settings, group "shortcuts").
class CommandRegistry : public QObject {
  Q_OBJECT

public:
  explicit CommandRegistry(QObject* parent = nullptr);
  ~CommandRegistry() override;

  // Adds a command; its action is made by createActions().
  void add(const CommandDef& def);
  const std::vector<std::unique_ptr<CommandDef>>& commands() const { return m_commands; }
  // A command by its id, or by an id it replaced (alias()).
  const CommandDef* find(const QString& id) const;
  // `oldId` is a former id of the command `id` (mitcad#503: Move to a
  // Project became Save to Project): find() and action() accept it, and
  // a shortcut saved under it moves to `id` (kept when `id` has one of its
  // own).
  void alias(const QString& oldId, const QString& id);
  // The feature command that can edit a definition (the `feature` query's
  // def), or null.
  const CommandDef* editorOf(const QJsonObject& def) const;

  // Makes an action per command, with window-wide shortcuts on `owner`;
  // triggering one calls `trigger` with the command's id.
  void createActions(QWidget* owner, const std::function<void(const QString&)>& trigger);
  QAction* action(const QString& id) const { return m_actions.value(m_aliases.value(id, id)); }

  QKeySequence shortcut(const QString& id) const;
  QKeySequence defaultShortcut(const QString& id) const;
  // The runtime bindings, including default-only alternates. Conflicting
  // legacy settings remain editable but are not installed on actions.
  QList<QKeySequence> effectiveShortcuts(const QString& id) const;
  QList<QKeySequence> effectiveShortcuts(const QString& id, const QKeySequence& primary) const;
  QList<ShortcutConflict> shortcutConflicts(const QHash<QString, QKeySequence>& proposed = {}) const;
  // Registered defaults, independent of any saved user overrides.
  QList<ShortcutConflict> defaultShortcutConflicts() const;
  const QStringList& shortcutWarnings() const { return m_shortcutWarnings; }
  QString shortcutHint(const QString& id) const;
  // Applies one complete proposal so swaps do not create intermediate conflicts.
  void setShortcuts(const QHash<QString, QKeySequence>& shortcuts);
  // Changes a shortcut and saves it; an empty sequence removes it.
  void setShortcut(const QString& id, const QKeySequence& shortcut);

  // Enables the actions of the commands `available` accepts.
  void updateEnabled(const std::function<bool(const CommandDef&)>& available);

private:
  ShortcutBinding binding(const CommandDef& def, const QKeySequence& primary) const;
  void refreshShortcuts();
  void applyShortcut(const CommandDef& def);

  std::vector<std::unique_ptr<CommandDef>> m_commands;
  QHash<QString, QAction*> m_actions;
  QHash<QString, QString> m_aliases; // former id -> id
  QSet<QString> m_disabledShortcuts;
  QStringList m_shortcutWarnings;
};

} // namespace mitcad
