// SPDX-License-Identifier: MIT
#pragma once

// The Selection filter control of sketch mode (mitcad#217), in the SKETCH
// tab's SELECT group: one row per category of the selection filter context
// in use (the sketch's, or Project's while it runs), each a button that
// cycles its state when clicked (empty -> square -> cross -> empty), the
// symbols explained above them, and Reset to Defaults below.
//
// Every row is an action named `command_select.filter.<row id>` (and
// `command_select.filter.reset`), so that the test driver's `command` step
// reaches them as it reaches commands. Shown, the panel logs where its rows
// are in the main window ("Selection filter row <id> at x,y") for the UI
// tests that click them.

#include <array>
#include <cstddef>
#include <functional>

#include <QColor>
#include <QIcon>
#include <QPointer>
#include <QString>
#include <QWidget>

#include "SelectionScope.hpp"

class QAction;
class QLabel;
class QToolButton;

namespace mitcad {

// The row symbols: an empty square, a filled square, a square with a cross,
// drawn in the palette's text colour.
QIcon filterStateIcon(FilterState state, const QColor& color);

class SelectionFilterPanel : public QWidget {
  Q_OBJECT

public:
  // `scope`: the context shown and changed (asked each time: it changes
  // when Project starts and ends), null while none is in use; `changed`:
  // called after a row cycled or a reset, with what happened for the log.
  // `host`: the main window, which the logged places are relative to.
  SelectionFilterPanel(std::function<SelectionScope*()> scope, std::function<void(const QString&)> changed,
                       QWidget* host, QWidget* parent = nullptr);

  // The rows and states of the context in use now.
  void refresh();
  QAction* rowAction(FilterRow row) const { return m_actions[static_cast<std::size_t>(row)]; }
  QAction* resetAction() const { return m_reset; }

protected:
  void showEvent(QShowEvent* event) override;
  void changeEvent(QEvent* event) override;

private:
  void cycle(FilterRow row);
  void logPlaces() const;

  std::function<SelectionScope*()> m_scope;
  std::function<void(const QString&)> m_changed;
  QPointer<QWidget> m_window;
  QLabel* m_title = nullptr;
  std::array<QAction*, kFilterRowCount> m_actions{};
  std::array<QToolButton*, kFilterRowCount> m_buttons{};
  QAction* m_reset = nullptr;
  QToolButton* m_resetButton = nullptr;
};

} // namespace mitcad
