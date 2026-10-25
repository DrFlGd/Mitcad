// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
#include <QHash>
#include <QJsonObject>
#include <QPair>
#include <QString>

#include "../framework/ParameterEdits.hpp"

class QCheckBox;
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
// top. Limits (mitcad#665): Min, Max and Step are expressions edited in
// place, Strict refuses a result outside them instead of clamping it, a
// limited value is marked in its cell (`372 mm → 300 mm ▲max`, the tooltip
// says why), a parameter with a minimum and a maximum gets a slider, and
// "Only limited" shows only the parameters a limit changed.
class ParametersDialog : public QDialog {
  Q_OBJECT

public:
  ParametersDialog(DocumentHost& host, QWidget* parent = nullptr);

  // Shows the parameters as the model has them now.
  void refresh();

private:
  // Limits' columns come after the first six (whose numbers test scripts use).
  enum Column { kName, kUnit, kExpression, kValue, kComment, kFavorite, kMin, kMax, kStep, kStrict, kSlider };

  QTreeWidgetItem* addRow(QTreeWidgetItem* parent, const QJsonObject& parameter);
  void edit(QTreeWidgetItem* item, int column);
  void cellEdited(QTreeWidgetItem* item, int column);
  void addParameter();
  void deleteParameter();
  void editFormula();
  void updateButtons();
  void applyFilter();
  void logLayout();

  DocumentHost& m_host;
  QTreeWidget* m_table = nullptr;
  QLineEdit* m_filter = nullptr;
  QLabel* m_message = nullptr;
  QPushButton* m_add = nullptr;
  QPushButton* m_delete = nullptr;
  QPushButton* m_formula = nullptr;
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

  // Limits (mitcad#665): a model command for a cell, the slider or the
  // strict box, run after the table finished its own handling; the
  // slider of a row; what typing an expression would do; the filter.
  void submit(const QString& name, int column, const QString& text, const QString& what,
              const QJsonObject& command);
  void addSlider(QTreeWidgetItem* row, const QJsonObject& parameter);
  void previewExpression(const QString& name, const QString& text);
  QCheckBox* m_limitedOnly = nullptr;
  QString m_loggedPreview;
  QString m_loggedLimited;

  // The parameters as CSV (mitcad#666): Export writes Mitcad's table to a
  // file; Import reads Mitcad's table or the .f3d application's parameter
  // table, shows what it would add, change, rename, refuse and skip, and
  // imports it as one undo step. Not while a sketch or a panel is open.
  void exportParameters();
  void importParameters();
  QPushButton* m_import = nullptr;
  QPushButton* m_export = nullptr;
};

} // namespace mitcad
