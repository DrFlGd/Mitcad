// SPDX-License-Identifier: MIT
#pragma once

#include <QKeySequence>
#include <QList>
#include <QObject>
#include <QSet>
#include <QString>

namespace mitcad {

// Potentially simultaneous states, independent of the current selection.
// A model feature which accepts profiles also has SketchShortcutContext.
enum ShortcutContext : unsigned {
  ModelShortcutContext = 1,
  SketchShortcutContext = 2,
  CommandShortcutContext = 4,
  AllShortcutContexts = ModelShortcutContext | SketchShortcutContext | CommandShortcutContext,
};

struct ShortcutBinding {
  QString id;
  QString name;
  unsigned contexts = AllShortcutContexts;
  QKeySequence primary;
  QKeySequence defaultPrimary;
  QList<QKeySequence> alternates;
  bool fixed = false;

  QList<QKeySequence> effectiveKeys() const {
    QList<QKeySequence> keys;
    if (!primary.isEmpty()) {
      keys.append(primary);
      if (primary == defaultPrimary) {
        for (const QKeySequence& alternate : alternates) {
          if (!alternate.isEmpty() && !keys.contains(alternate)) keys.append(alternate);
        }
      }
    }
    return keys;
  }
};

struct ShortcutConflict {
  ShortcutBinding first;
  ShortcutBinding second;
  QKeySequence keys;

  QString description() const {
    return QObject::tr("%1 is assigned to both %2 and %3, which can be active together.%4")
        .arg(keys.toString(QKeySequence::NativeText), first.name, second.name,
             first.fixed || second.fixed
                 ? QObject::tr(" This application shortcut is reserved; choose another binding.")
                 : QObject::tr(" Change or clear one of these bindings."));
  }
};

inline QList<ShortcutConflict> shortcutConflicts(const QList<ShortcutBinding>& bindings) {
  QList<ShortcutConflict> conflicts;
  for (qsizetype i = 0; i < bindings.size(); ++i) {
    for (qsizetype j = i + 1; j < bindings.size(); ++j) {
      const auto& first = bindings[i];
      const auto& second = bindings[j];
      if (first.id == second.id || !(first.contexts & second.contexts)) continue;
      for (const auto& a : first.effectiveKeys()) {
        for (const auto& b : second.effectiveKeys()) {
          // Prefixes are ambiguous too if a legacy setting contains a chord.
          if (a.matches(b) != QKeySequence::NoMatch || b.matches(a) != QKeySequence::NoMatch) {
            conflicts.append({first, second, a});
          }
        }
      }
    }
  }
  return conflicts;
}

// Do not erase legacy settings. Disable their runtime bindings until the
// user repairs them, preserving every menu action. Defaults and fixed keys
// win over custom bindings; two colliding custom bindings are both disabled.
inline QSet<QString> conflictingShortcutIds(const QList<ShortcutConflict>& conflicts) {
  QSet<QString> disabled;
  for (const auto& conflict : conflicts) {
    const bool customFirst = !conflict.first.fixed &&
                             conflict.first.primary != conflict.first.defaultPrimary;
    const bool customSecond = !conflict.second.fixed &&
                              conflict.second.primary != conflict.second.defaultPrimary;
    if (customFirst) disabled.insert(conflict.first.id);
    if (customSecond) disabled.insert(conflict.second.id);
    if (!customFirst && !customSecond) {
      if (!conflict.first.fixed) disabled.insert(conflict.first.id);
      if (!conflict.second.fixed) disabled.insert(conflict.second.id);
    }
  }
  return disabled;
}

// These keys are installed outside the configurable command registry.
inline QList<ShortcutBinding> fixedShortcutBindings() {
  QList<ShortcutBinding> bindings;
  const auto add = [&bindings](const char* id, const QString& name, const QKeySequence& key,
                               unsigned contexts = AllShortcutContexts) {
    if (!key.isEmpty()) bindings.append({QString::fromLatin1(id), name, contexts, key, key, {}, true});
  };
  add("file.open", QObject::tr("Open"), QKeySequence::Open);
  add("file.close", QObject::tr("Close"), QKeySequence(Qt::CTRL | Qt::Key_W));
  add("file.save", QObject::tr("Save"), QKeySequence::Save);
  add("file.save_as", QObject::tr("Save As"), QKeySequence(Qt::CTRL | Qt::SHIFT | Qt::Key_S));
  add("file.quit", QObject::tr("Quit"), QKeySequence::Quit);
  add("edit.cut", QObject::tr("Cut"), QKeySequence::Cut);
  add("edit.copy", QObject::tr("Copy"), QKeySequence::Copy);
  add("edit.paste", QObject::tr("Paste"), QKeySequence::Paste);
  add("edit.select_all", QObject::tr("Select All"), QKeySequence::SelectAll);
  add("view.search_timeline", QObject::tr("Search Timeline (Browser/history focus)"), QKeySequence::Find);
  add("browser.rename", QObject::tr("Rename (Browser/history focus)"), QKeySequence(Qt::Key_F2),
      ModelShortcutContext);
  // Collection mutations require model idle, so they are exclusive with
  // sketch.delete. A global Model/Any command still conflicts with them.
  add("browser.delete", QObject::tr("Delete selection (Browser/history focus)"), QKeySequence(Qt::Key_Delete),
      ModelShortcutContext);
#ifdef Q_OS_MACOS
  add("browser.delete", QObject::tr("Delete selection (Browser/history focus)"), QKeySequence(Qt::Key_Backspace),
      ModelShortcutContext);
#endif
  add("command.accept", QObject::tr("Activate selection or accept command/value"), QKeySequence(Qt::Key_Return));
  add("command.accept", QObject::tr("Activate selection or accept command/value"), QKeySequence(Qt::Key_Enter));
  add("command.cancel", QObject::tr("Cancel or clear selection"), QKeySequence(Qt::Key_Escape));
#ifdef Q_OS_MACOS
  add("window.minimize", QObject::tr("Minimize"), QKeySequence(Qt::CTRL | Qt::Key_M));
  add("application.hide", QObject::tr("Hide Mitcad"), QKeySequence(Qt::CTRL | Qt::Key_H));
  add("application.hide_others", QObject::tr("Hide Others"), QKeySequence(Qt::CTRL | Qt::ALT | Qt::Key_H));
  // Qt Meta represents the physical Control key on macOS.
  add("window.full_screen", QObject::tr("Enter Full Screen"), QKeySequence(Qt::CTRL | Qt::META | Qt::Key_F));
#endif
  return bindings;
}

} // namespace mitcad
