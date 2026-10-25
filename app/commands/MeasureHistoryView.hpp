// SPDX-License-Identifier: MIT
#pragma once

#include <functional>
#include <utility>

#include <QStringList>
#include <QWidget>

class QLabel;
class QListWidget;
class QPushButton;

namespace mitcad {

class MeasureHistory;

// Measure's history in its panel (mitcad#648, app/COMMANDS.md "Measure's
// history"): one row per measurement, newest first, in a list of limited
// height that scrolls; a click on a row shows it again, Copy copies the
// row clicked and Copy All every row as tab-separated text, Clear empties
// the history. A row whose geometry is gone is grey and says so.
//
// The rows are logged whenever they change ("Measure history: 3 rows",
// "Measure history row 3: <picked>: <values>"), and their places and the
// buttons' with the panel's ("Measure history row 3 at x,y").
class MeasureHistoryView : public QWidget {
public:
  struct Actions {
    std::function<void(int number)> show; // a row clicked
    std::function<void()> clear;          // Clear clicked
    // Measurement objects (mitcad#649): Keep clicked (the measurement the
    // panel shows), Keep Row clicked (the row clicked last).
    std::function<void()> keep;
    std::function<void(int number)> keepRow;
  };

  explicit MeasureHistoryView(const MeasureHistory& history, QWidget* parent = nullptr);
  void setActions(Actions actions) { m_actions = std::move(actions); }
  // Whether the measurement the panel shows can be kept (picks of the
  // model's, with a result): Keep is enabled; logs "Measure Keep enabled"
  // or "… disabled" when it changes, and the places again.
  void setKeepable(bool keepable);
  // The rows again from the history; logs them, and their places once laid
  // out.
  void refresh();
  // "Measure history row 3 at x,y" ("… out of sight" scrolled away), and
  // the buttons', in the main window's coordinates.
  QStringList places() const;

private:
  void copy(bool all);
  void logPlaces();
  // The list as high as its rows up to a limit. Each row at least a line
  // of text high: a list not shown yet (Windows session 0) gave smaller
  // size hints, and only one row was in sight.
  void fitHeight();

  const MeasureHistory& m_history;
  Actions m_actions;
  QLabel* m_title = nullptr;
  QListWidget* m_list = nullptr;
  QPushButton* m_copy = nullptr;
  QPushButton* m_copyAll = nullptr;
  QPushButton* m_clear = nullptr;
  QPushButton* m_keep = nullptr;    // keeps the measurement shown (mitcad#649)
  QPushButton* m_keepRow = nullptr; // keeps the row clicked last
  QStringList m_logged; // the places logged last
};

} // namespace mitcad
