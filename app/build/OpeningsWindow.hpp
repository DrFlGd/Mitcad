// SPDX-License-Identifier: MIT
#pragma once

// The Openings window (mitcad#519; app/COMMANDS.md, "Timber", Openings): a
// wall's windows and doors, or a floor's openings, as rows with their own
// fields, beside the panel of Wall, Floor / Roof or Timber Frame. Non-modal:
// every change goes into the running command at once (its text input
// `frame_openings`, the command's data, through the main window), so the
// preview follows; the selected row's opening is marked in the view, and
// each row's problem shows on it as it is typed. A wall's choice of
// load-bearing or not (mitcad#519) is the window's too. It closes with the
// command.
//
// For UI tests it logs its places ("Openings place <name> at x,y; in the
// window x,y": add_window, add_door, add_opening, remove, bearing, close,
// and each row's fields, kind.1, from.1, offset.1, width.1, height.1,
// sill.1, across.1), again whenever they move, and what it writes
// ("Openings rows: <text>", "Openings row 2: <problem>", "Openings rows
// ok", "Openings bearing: load-bearing", "Openings selected row 2").

#include <vector>

#include <QDialog>
#include <QPointer>
#include <QString>
#include <QTimer>

#include "build/TimberOpenings.hpp"

class QComboBox;
class QGridLayout;
class QLabel;
class QLineEdit;
class QPushButton;
class QWidget;

namespace mitcad {

class OpeningsWindow : public QDialog {
  Q_OBJECT

public:
  // What the window edits: the openings of the command `command`
  // (build.wall, build.floor_roof, build.frame), a floor's or a wall's,
  // their text, a wall's kind and the sizes its rows are checked against.
  struct Target {
    QString command;
    bool floor = false;
    QString text;
    bool loadBearing = false;
    build::WallSize wall;
    double floorLength = build::kUnknown;
    double floorWidth = build::kUnknown;
  };

  // The window of a main window, made the first time it is asked for.
  static OpeningsWindow* of(QWidget* window);
  explicit OpeningsWindow(QWidget* parent = nullptr);
  ~OpeningsWindow() override;

  using QDialog::open;

  // Shows the target's rows, beside the panel.
  void open(const Target& target);
  // The command's openings changed elsewhere (live MCP, Timber Frame's
  // member picked): its rows again unless it is what the window wrote;
  // the sizes the rows are checked against. Nothing while it shows another
  // command's or is closed.
  void follow(const Target& target);
  // The command ended: the window closes.
  void commandEnded(const QString& command);
  QString command() const { return m_target.command; }

signals:
  // The rows as the command's text (`frame_openings`).
  void textChanged(const QString& command, const QString& text);
  // A wall's kind: load-bearing or a partition.
  void bearingChanged(const QString& command, bool loadBearing);
  // The row selected, from 1 (0: none): its opening is marked in the view.
  void selectedChanged(const QString& command, int row);

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void moveEvent(QMoveEvent* event) override;
  void resizeEvent(QResizeEvent* event) override;
  void showEvent(QShowEvent* event) override;
  // Hidden (Close, Escape, the command ending): the main window active
  // again (mitcad#532), so that its Enter, Escape and shortcuts work
  // without a click on it first.
  void hideEvent(QHideEvent* event) override;

private:
  struct RowWidgets {
    QLabel* number = nullptr;
    QComboBox* kind = nullptr;
    QComboBox* from = nullptr;
    QLineEdit* offset = nullptr;
    QLineEdit* across = nullptr;
    QLineEdit* width = nullptr;
    QLineEdit* height = nullptr;
    QLineEdit* sill = nullptr;
    QLabel* problem = nullptr;
  };

  // The rows' widgets made again from `m_rows`.
  void rebuild();
  // The rows read from the widgets (a field that is no number keeps its
  // row's value and is flagged).
  void readRows();
  // The rows checked: each row's problem on it, logged.
  void check();
  // The rows written into the command when they changed.
  void write();
  void select(int row);
  void addRow(build::OpeningRow::Kind kind);
  void removeRow();
  void logPlacesLater();
  void logPlaces() const;

  Target m_target;
  std::vector<build::OpeningRow> m_rows;
  std::vector<RowWidgets> m_widgets;
  QString m_written; // the text written last (or opened with)
  int m_selected = 0;
  QStringList m_badFields; // "offset.2": not a number
  QWidget* m_grid = nullptr;
  QGridLayout* m_layout = nullptr;
  QComboBox* m_bearing = nullptr;
  QLabel* m_bearingLabel = nullptr;
  QPushButton* m_addWindow = nullptr;
  QPushButton* m_addDoor = nullptr;
  QPushButton* m_addOpening = nullptr;
  QPushButton* m_remove = nullptr;
  QPushButton* m_close = nullptr;
  QLabel* m_summary = nullptr;
  QTimer m_placesTimer;
  bool m_placed = false;
};

} // namespace mitcad
