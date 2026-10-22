// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
#include <QJsonObject>
#include <QString>
#include <QStringList>

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

private:
  enum Column { kKey, kValue, kType, kSource };

  void edit(QTreeWidgetItem* item, int column);
  void editChoice(QTreeWidgetItem* item, const QStringList& choices);
  void cellEdited(QTreeWidgetItem* item, int column);
  void setValue(QTreeWidgetItem* item, const QString& text);
  void addProperty();
  void removeProperty();
  void editTypes();
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
};

} // namespace mitcad
