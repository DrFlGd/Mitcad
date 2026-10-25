// SPDX-License-Identifier: MIT
#pragma once

#include <functional>

#include <QDialog>
#include <QJsonArray>
#include <QJsonObject>
#include <QString>
#include <QStringList>
#include <QVector>

#include "../framework/Appearances.hpp"
#include "PrintKeys.hpp"
#include "PrintLibrary.hpp"

class QAction;
class QComboBox;
class QLabel;
class QLineEdit;
class QPushButton;
class QToolButton;
class QTreeWidget;
class QTreeWidgetItem;

namespace mitcad {

class DocumentHost;
class SettingsForm;

// Materials (mitcad#655): a library window like Appearances, opened by
// Material... and Physical Material (with the bodies or the component it
// assigns to) and Edit Materials. On the left the materials the user can
// choose (the `library_items` query, one row with the design's copy),
// grouped by their class in Metals, Plastics (the filaments too) and
// Other, each with its appearance's swatch; no places: an item is
// built-in or the user's own. Opened for targets the top row is Default
// with what it gives ("Default (Steel)", its swatch, a line under it),
// and Assign (Assign to 2 bodies, Assign to Arm) gives the selected one.
//
// On the right the selected material with the values the design prints
// with: its name as a title, "Built-in" with a lock for a built-in one,
// and Save to for the user's own that is in no library yet (My library,
// in a project a menu with Project), then the tabs: General (Name, Class,
// Density in g/cm³, Appearance) and, for a plastic only, the print
// settings' (Filament, Cooling, ...; the window's one search is theirs).
// An unset filament colour shows the appearance's colour, which is what
// is exported. Each field is saved at Enter or when it is left: one model
// command and undo step, `edit_material` with the item's library when it
// is saved in one (the library's file and the design's copy together;
// Undo restores both). A saved item another design changed is asked
// about once (askChangedElsewhere: Use the changes, Keep these). Built-in
// ones are read only: Duplicate makes the user's own copy. New asks the
// class (Metal, Plastic, Other); New and Duplicate make the material in
// this design and select it. Delete removes the item (a saved one from
// its library, the design keeping its copy), always asked
// (confirmDelete). A refusal of a setting shows under its field, others
// on the message line. Read only is disabled, never hidden.
class MaterialsDialog : public QDialog {
  Q_OBJECT

public:
  MaterialsDialog(DocumentHost& host, QWidget* parent = nullptr);
  ~MaterialsDialog() override;

  // What Assign gives the material: bodies (their names in `names`), or a
  // component (`component`, its name in `names`); neither: no Assign.
  void setTargets(const QStringList& bodies, const QString& component, const QStringList& names);
  // Whether Assign leaves the window open (3D Print's Assign materials…,
  // mitcad#655); else it closes it.
  void setKeepOpen(bool keepOpen);
  // Shows the materials and the selected one as the model has them now.
  void refresh();
  // Edit Appearances… of the Appearance box (the controller's window).
  std::function<void()> editAppearances;

protected:
  void showEvent(QShowEvent* event) override;
  void resizeEvent(QResizeEvent* event) override;
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  const LibraryItem* current() const;
  void rebuildList();
  void showCurrent();
  void selectKey(const QString& key);
  // Runs `change` (fields of edit_material) on the selected item, later.
  void edit(const QJsonObject& change, const QString& what, const QString& field = QString());
  void newMaterial();
  void duplicate();
  void remove();
  void saveTo();
  // Asks once about the selected item when another design changed it.
  void askChanges();
  void assign();
  void appearanceChosen(int index);
  // The model can change now; else says why on the message line.
  bool canChange();
  bool run(const QJsonObject& command, QJsonObject* result = nullptr);
  void setMessage(const QString& text, bool error = false);
  void logRows();
  void scheduleLogPlaces();
  void logPlaces();

  DocumentHost& m_host;
  PrintKeyTable m_keys;
  QString m_dialect; // the design's slicer (designDialectOf)
  QVector<LibraryItem> m_items;
  QJsonArray m_designMaterials;
  QJsonObject m_library;
  QVector<Appearance> m_appearances;
  QJsonArray m_bodies;
  QStringList m_targetBodies;
  QString m_targetComponent;
  QStringList m_targetNames;
  QString m_currentKey;      // the selected row's key; "default" for Default or None
  QString m_currentDesignId; // its design copy, which a row keeps when its place changes
  QString m_pendingFocus;    // a row made just now, whose Name takes the keyboard
  bool m_pendingDensity = false; // ... its Density instead (a new material of no typical one)
  bool m_building = false;
  bool m_keepOpen = false;
  QTreeWidget* m_list = nullptr;
  QWidget* m_editor = nullptr;
  QLineEdit* m_name = nullptr;
  QComboBox* m_class = nullptr;
  QWidget* m_builtIn = nullptr; // "Built-in" with its lock and Duplicate
  QPushButton* m_headerDuplicate = nullptr;
  QLabel* m_saved = nullptr; // "In My library", a saved item's state
  QAction* m_nameSwatch = nullptr; // Default's swatch in the name field
  QLabel* m_emptyState = nullptr;  // "No material selected"
  QString m_defaultText;           // "Default (Steel)"
  QColor m_defaultColor;
  QLabel* m_usedState = nullptr; // "● Used in this design"
  QStringList m_usedIds;         // the design's materials its bodies use
  QPushButton* m_saveTo = nullptr;
  QLineEdit* m_density = nullptr;
  QLabel* m_densityError = nullptr;
  QComboBox* m_appearance = nullptr;
  SettingsForm* m_form = nullptr;
  QLabel* m_message = nullptr;
  QPushButton* m_new = nullptr;
  QPushButton* m_duplicate = nullptr;
  QPushButton* m_delete = nullptr;
  QPushButton* m_assign = nullptr;
  QLabel* m_mixed = nullptr; // the targets' mixed state beside Assign
  QPushButton* m_close = nullptr;
  QString m_loggedRows;
  QString m_loggedPlaces;
};

} // namespace mitcad
