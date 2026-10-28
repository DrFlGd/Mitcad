// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QString>
#include <QStringList>

class QComboBox;
class QLabel;
class QPushButton;
class QTreeWidget;
class QTreeWidgetItem;

namespace mitcad {

// Presets of an installed slicer (mitcad#830): the user presets of a
// Bambu-family slicer (`slicer_presets`' `slicers`) to tick for an import,
// in a modal window titled Import Presets.
//
// At the top the slicer, a box when several were found ("Bambu Studio",
// "OrcaSlicer (Flatpak)"). The list groups the presets under Printers,
// Processes and Filaments, each group's box ticking all or none of it,
// each preset a box by its name. A preset whose name the target already
// has (`same`) is in the warning colour with a tooltip naming what it
// meets ("My library has a process of this name"), unticked at first,
// and has its own choice beside it ("<name>: same name"): Copy (the
// default: a new item, "<name> 2") or Override (the existing item takes
// its values); choosing in it ticks the row. "All same names:" above the
// list sets every such row's choice at once (the ticks stay); it shows
// the rows' choice when they agree and no value when they differ, and
// only when there are such rows. New presets are ticked at first. The
// name column is as wide as the names, each choice beside its name.
// Import (disabled with nothing ticked) and Cancel.
//
// Keyboard: the list has it at first, its first preset the current row,
// framed while the keyboard is in the row or its choice; the arrows move
// the row, Space ticks or unticks it (a group: all of it), Tab goes to
// the row's choice and on to Import, Shift+Tab from the choice back to
// the row.
//
// Logs "Slicer presets opened: <slicer>, <n> presets", each row as
// "Slicer presets row: <kind> <name>, ticked|unticked[, same name: <tooltip>,
// copy|override]", "Slicer presets ticked: <n>" when that changes,
// "Slicer presets tick: <name> ticked|unticked" for a row's change,
// "Slicer presets choice: <name> copy|override", "Slicer presets same
// name: copy|override" (All same names chosen), "Slicer presets all same
// names: copy|override|none" when what it shows changes, "Slicer presets
// current: <name>" (the current row moved), "Slicer presets focus: <name>
// choice" (a row's choice has the keyboard), and the places "Slicer
// presets <what> at x,y" (`slicer`, `same_all`, `import`, `cancel`,
// `item <name>`: a row's box).
class SlicerPresetsDialog : public QDialog {
  Q_OBJECT

public:
  // `slicers` from `slicer_presets`, `index` the one shown first, `into`
  // the target (`design`, `user`, `project`), which the tooltips name.
  SlicerPresetsDialog(const QJsonArray& slicers, int index, const QString& into, QWidget* parent = nullptr);

  // The `import_slicer_presets` command of the ticked presets (`dialect`,
  // `folder`, `presets` with `same_name`), without its target.
  QJsonObject command() const;
  // How many presets are ticked.
  int tickedCount() const;
  // The ticked presets' names, in the list's order.
  QStringList tickedNames() const;
  // The slicer shown ("Bambu Studio").
  QString slicerName() const;
  // A slicer's name in the box and the menus: "OrcaSlicer (Flatpak)" for
  // a Flatpak's.
  static QString slicerTitle(const QJsonObject& slicer);

protected:
  void showEvent(QShowEvent* event) override;
  // The list's Space and Tab, the rows' choices' Tab and focus.
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  // The slicer shown, of `slicer_presets`' `slicers`.
  QJsonObject shownSlicer() const;
  void showSlicer(int index);
  // The tooltip of a preset whose name the target has.
  QString sameTooltip(const QString& kind, const QJsonObject& same) const;
  // A same-name row's choice, `copy` or `override`.
  void setRowChoice(QTreeWidgetItem* row, const QString& choice);
  // All same names shows the rows' choice, or none when they differ.
  void sameChoicesChanged();
  void ticksChanged();
  void logRows();
  void scheduleLogPlaces();
  void logPlaces();

  QJsonArray m_slicers;
  int m_index = 0;
  QString m_into;
  QComboBox* m_slicer = nullptr;
  QLabel* m_slicerLabel = nullptr;
  QLabel* m_sameLabel = nullptr;
  QComboBox* m_sameAll = nullptr;
  QTreeWidget* m_list = nullptr;
  QPushButton* m_import = nullptr;
  QPushButton* m_cancel = nullptr;
  QHash<QObject*, QTreeWidgetItem*> m_boxRows; // a same-name row's choice, its row
  bool m_building = false;
  int m_loggedTicked = -1;
  QHash<QTreeWidgetItem*, bool> m_loggedTicks; // each row's tick as last logged
  QString m_loggedSameAll;
  QString m_loggedPlaces;
};

} // namespace mitcad
