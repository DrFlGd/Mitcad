// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
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
// and has its own choice beside it: Copy (the default: a new item,
// "<name> 2") or Override (the existing item takes its values). "Same
// name:" above the list sets every such row at once; it shows only when
// there are such rows. New presets are ticked at first. Import (disabled
// with nothing ticked) and Cancel.
//
// Logs "Slicer presets opened: <slicer>, <n> presets", each row as
// "Slicer presets row: <kind> <name>, ticked|unticked[, same name: <tooltip>,
// copy|override]", "Slicer presets ticked: <n>" when that changes, and
// the places "Slicer presets <what> at x,y" (`slicer`, `same_all`,
// `import`, `cancel`, `item <name>`: a row's box).
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
  // The slicer shown ("Bambu Studio").
  QString slicerName() const;
  // A slicer's name in the box and the menus: "OrcaSlicer (Flatpak)" for
  // a Flatpak's.
  static QString slicerTitle(const QJsonObject& slicer);

protected:
  void showEvent(QShowEvent* event) override;

private:
  // The slicer shown, of `slicer_presets`' `slicers`.
  QJsonObject shownSlicer() const;
  void showSlicer(int index);
  // The tooltip of a preset whose name the target has.
  QString sameTooltip(const QString& kind, const QJsonObject& same) const;
  // A same-name row's choice, `copy` or `override`.
  void setRowChoice(QTreeWidgetItem* row, const QString& choice);
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
  bool m_building = false;
  int m_loggedTicked = -1;
  QString m_loggedPlaces;
};

} // namespace mitcad
