// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
#include <QHash>
#include <QJsonObject>
#include <QPair>
#include <QString>

#include "../framework/ParameterEdits.hpp"

class QEvent;
class QLabel;
class QLineEdit;
class QPushButton;
class QTreeWidget;
class QTreeWidgetItem;

namespace mitcad {

class DecimalAlignDelegate;
class DocumentHost;

// Change Parameters: the user parameters and the model's (the
// dimensions of each feature) with their name, unit, expression, value and
// comment. Each change is a model command and an undo step at once; names,
// expressions, units and comments are edited in place (double-click or
// F2), and what the model refuses (a cycle, a unit that does not fit, an
// unknown name) stays in the cell in red with the reason. The star column
// marks favourites (P9), which are also listed in a Favorites group at the
// top.
class ParametersDialog : public QDialog {
  Q_OBJECT

public:
  ParametersDialog(DocumentHost& host, QWidget* parent = nullptr);

  // Shows the parameters as the model has them now.
  void refresh();

private:
  enum Column { kName, kUnit, kExpression, kValue, kComment, kFavorite };

  QTreeWidgetItem* addRow(QTreeWidgetItem* parent, const QJsonObject& parameter);
  void edit(QTreeWidgetItem* item, int column);
  void cellEdited(QTreeWidgetItem* item, int column);
  void addParameter();
  void deleteParameter();
  void updateButtons();
  void applyFilter();
  void logLayout();

  DocumentHost& m_host;
  QTreeWidget* m_table = nullptr;
  QLineEdit* m_filter = nullptr;
  QLabel* m_message = nullptr;
  QPushButton* m_add = nullptr;
  QPushButton* m_delete = nullptr;
  QPushButton* m_ok = nullptr;
  bool m_building = false;
  // Edits the model refused, by "name|column": the typed text and why.
  QHash<QString, QPair<QString, QString>> m_refused;
  QString m_logged;     // the cells' places
  QString m_loggedRows; // the parameters as shown
  QString m_loggedFavorites;

  // While a sketch or a feature is edited (mitcad#564): what stays open
  // (a line above the rows), the open panel's own parameters left to it,
  // and the table read again when it is activated in another edit.
  void updateScope();
  bool event(QEvent* event) override;
  ParameterEditScope m_scope;
  QLabel* m_scopeNote = nullptr;
  QString m_loggedScope;
  // Expression's and Value's numbers on one decimal axis each (mitcad#566).
  DecimalAlignDelegate* m_align = nullptr;
};

} // namespace mitcad
