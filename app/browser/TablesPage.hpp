// SPDX-License-Identifier: MIT
#pragma once

#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QPair>
#include <QString>
#include <QWidget>

class QEvent;
class QLabel;
class QLineEdit;
class QListWidget;
class QPushButton;
class QShowEvent;
class QTableWidget;
class QTableWidgetItem;

namespace mitcad {

class ColumnHeader;
class DocumentHost;

// The Tables tab of Change Parameters (mitcad#667, docs/table-lookups.md):
// the design's tables of data that formulas look up with XLOOKUP, MATCH
// and INDEX. A list of the tables on the left (New, Rename, Delete,
// Import CSV…, Export CSV…); on the right the selected table: its
// comment, a grid whose header cells give each column's name and unit,
// buttons that add, remove and move rows and columns, and the parameters
// that use it. Each change is one model command and one undo step (the
// model's `set_table`, `rename_table`, `rename_table_column`,
// `delete_table`, `import_table_csv`; commands.md); what the model refuses
// stays in its cell in red with the reason, as in the Parameters tab.
// Logged for UI tests (app/COMMANDS.md, "Tables").
class TablesPage : public QWidget {
  Q_OBJECT

public:
  explicit TablesPage(DocumentHost& host, QWidget* parent = nullptr);

  // Shows the tables as the model has them now.
  void refresh();
  // The places of the controls, in the main window's coordinates.
  void logLayout();
  // The keys to the grid (the list while there is no table).
  void takeFocus();

signals:
  // A command changed the design: the parameters' values may follow.
  void changed();

protected:
  void showEvent(QShowEvent* event) override;

private:
  // The selected table as the model has it ({} when none is).
  QJsonObject table() const;
  QJsonArray columns() const;
  QJsonArray rows() const;
  void showTable();
  void buildHeader(const QJsonArray& columns);
  void updateButtons();

  // A model command: in a sketch or a panel as the parameters' commands
  // run there (DocumentHost::runParameterCommand); false with `problem`.
  bool run(const QJsonObject& command, QJsonObject* result, QString* problem);
  // `set_table` of the selected table with these columns and rows, after
  // the grid's own handling (a cell's editor is still closing); `key` is
  // the refused cell's or header's key, `what` the log's words, `shown` the
  // message's (when they differ), `done` what the status bar says after it.
  void submitTable(const QJsonArray& columns, const QJsonArray& rows, const QString& comment, const QString& key,
                   const QString& typed, const QString& what, const QString& shown = QString(),
                   const QString& done = QString());

  void newTable();
  void renameTable();
  void deleteTable();
  void importCsv();
  void exportCsv();
  void cellEdited(QTableWidgetItem* item);
  void commentEdited();
  void renameColumn(int column, const QString& name);
  void changeUnit(int column, const QString& unit);
  void addRow();
  void removeRow();
  void moveRow(int by);
  void addColumn();
  void removeColumn();
  void moveColumn(int by);

  DocumentHost& m_host;
  QListWidget* m_list = nullptr;
  QPushButton* m_new = nullptr;
  QPushButton* m_rename = nullptr;
  QPushButton* m_delete = nullptr;
  QPushButton* m_import = nullptr;
  QPushButton* m_export = nullptr;
  QWidget* m_editor = nullptr; // the selected table's side
  QLabel* m_empty = nullptr;   // in its place while there is no table
  QLineEdit* m_comment = nullptr;
  QPushButton* m_addRow = nullptr;
  QPushButton* m_removeRow = nullptr;
  QPushButton* m_rowUp = nullptr;
  QPushButton* m_rowDown = nullptr;
  QPushButton* m_addColumn = nullptr;
  QPushButton* m_removeColumn = nullptr;
  QPushButton* m_columnLeft = nullptr;
  QPushButton* m_columnRight = nullptr;
  QTableWidget* m_grid = nullptr;
  ColumnHeader* m_header = nullptr;
  QLabel* m_usedBy = nullptr;
  QLabel* m_problems = nullptr;
  QLabel* m_message = nullptr;

  QJsonArray m_tables; // the `tables` query's answer
  QString m_current;   // the selected table's name
  bool m_building = false;
  // The grid's current cell, kept over the grid's rebuilds.
  int m_row = 0;
  int m_column = 0;
  // The grid takes the keys when it is shown next (after a button or a
  // dialog had them).
  bool m_focusGrid = false;
  bool eventFilter(QObject* watched, QEvent* event) override;
  // Edits the model refused, by "table|row|column" for cells and
  // "table|name|column" for a column's name: the typed text and why.
  QHash<QString, QPair<QString, QString>> m_refused;
  QString m_loggedTables;
  QString m_loggedColumns;
  QString m_loggedRows;
  QString m_loggedUsedBy;
  QString m_loggedRefused;
  QString m_loggedLayout;
};

} // namespace mitcad
