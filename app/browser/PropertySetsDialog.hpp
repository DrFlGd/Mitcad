// SPDX-License-Identifier: MIT
#pragma once

#include <functional>

#include <QDialog>
#include <QJsonArray>
#include <QJsonObject>
#include <QList>
#include <QPair>
#include <QPointer>
#include <QSet>
#include <QString>
#include <QStringList>

#include "../print/PrintKeys.hpp"

class QCheckBox;
class QLabel;
class QPushButton;
class QTreeWidget;
class QTreeWidgetItem;

namespace mitcad {

class DocumentHost;

// What Property Sets shows (mitcad#496): a body, an occurrence (a path
// from the root, "O1/O4", or its uid) and a component (C0 the root);
// fields left empty are not shown, and the component follows from the
// occurrence or the body.
struct PropertyTarget {
  QString body;
  QString occurrence;
  QString component;

  bool isEmpty() const { return body.isEmpty() && occurrence.isEmpty() && component.isEmpty(); }
};

// Property Sets (mitcad#496): the typed properties of a body, an
// occurrence and its component, a table per level with the sets foldable,
// from the model's `property_sets` query (stored values, those a library
// part gives and the computed ones). Each value shows its type and its
// source (user, rvt, ifc, ipt, f3d, library, computed); an imported value
// the user changed shows the value its source gave. Values are edited in
// place (double-click or F2): a value of an own type with allowed values,
// and a yes/no, as a choice. An occurrence lists its component's values
// as inherited; editing one sets the occurrence's own value over it. Add
// and Remove set and remove a property; Types... defines and removes the
// design's own types. Each change is one model command, one undo step.
class PropertySetsDialog : public QDialog {
  Q_OBJECT

public:
  PropertySetsDialog(DocumentHost& host, QWidget* parent = nullptr);

  // What to show; refreshes.
  void setTarget(const PropertyTarget& target);
  // Shows the properties as the model has them now.
  void refresh();
  // Material… of the print settings' Filament row (mitcad#655): the
  // Materials window for the bodies (uids) or the component.
  std::function<void(const QStringList& bodies, const QString& component)> openMaterial;
  // Edit… beside a Printer or Process profile: the profile editor of the
  // kind and the item ("<place>/<id>").
  std::function<void(const QString& kind, const QString& key)> openProfile;
  // Own settings' pencil below the root (mitcad#655): the level's own
  // process settings in the settings form, `owner` its uid and `name` as
  // the design shows it. Process's pencil opens the process (openProfile).
  std::function<void(const QString& owner, const QString& name)> openOwnSettings;

private:
  enum Column { kKey, kValue, kType, kSource };

  void edit(QTreeWidgetItem* item, int column);
  void editChoice(QTreeWidgetItem* item, const QStringList& choices);
  void cellEdited(QTreeWidgetItem* item, int column);
  void setValue(QTreeWidgetItem* item, const QString& text);
  void addProperty();
  void removeProperty();
  void editTypes();
  // The print settings the most specific level prints with (mitcad#640,
  // mitcad#655; the `print_settings` query), titled Print settings: Printer
  // and Process with the profile chosen (plain; a click opens a drop-down of
  // the choosable items, Edit opens Printers and Processes) and Material
  // (a click or Edit opens Materials), each value under a category ›
  // sub-group heading in the key table's order with the icon of the owner
  // that gives it; a click on a value opens the settings form's field
  // (SettingField) and sets that level's own value, shown bold with ↺ after
  // it. Add a setting… under Printer and Process adds a key of that set.
  // Edit and ↺ follow the value as shown, elided to leave them room inside
  // the Value column; a click on one does its action only, never also the
  // value's (mitcad#827). The profile drop-down is opaque and ends when its
  // list closes, with a choice or without one.
  void addPrintSettings(const QJsonArray& levels, const QSet<QString>& folded, const QString& current,
                        QStringList& rows);
  // Opens a print value's field (the delegate's editor).
  void editPrint(QTreeWidgetItem* item);
  // The native table mounts real shared setting fields and glyph actions;
  // its cells do not run Qt delegate painting or mouse hit tests.
  void createNativePrintCells();
  // Materials for the body or the component shown (Material's row).
  void openPrintMaterial();
  // Own settings of the level shown (`owner`) in the settings form.
  void openLevelOwnSettings(const QString& owner);
  // Sets (null: removes) a print value or profile of the shown level, later.
  void setPrint(const QString& set, const QString& key, const QJsonObject& value, const QString& shown);
  void updateButtons();
  // The row's id in logs and refusals: "<level> <set>/<key>".
  static QString rowName(const QTreeWidgetItem* item);
  void logLayout();

  DocumentHost& m_host;
  PropertyTarget m_target;
  QTreeWidget* m_table = nullptr;
  QLabel* m_title = nullptr;
  QLabel* m_message = nullptr;
  QCheckBox* m_computed = nullptr;
  QPushButton* m_add = nullptr;
  QPushButton* m_remove = nullptr;
  QPushButton* m_types = nullptr;
  QPushButton* m_close = nullptr;
  // The levels as the last answer gave them.
  QJsonObject m_answer;
  bool m_building = false;
  QString m_logged;     // the cells' places
  QString m_loggedRows; // the properties as shown
  PrintKeyTable m_keys;     // print_keys (mitcad#655)
  QString m_addedKey;       // a key Add a setting… added, shown until it is set
  // The print block's widgets by the name their place is logged under.
  QList<QPair<QString, QPointer<QWidget>>> m_printWidgets;
};

} // namespace mitcad
