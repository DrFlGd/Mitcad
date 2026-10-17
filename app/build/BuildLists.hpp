// SPDX-License-Identifier: MIT
#pragma once

// BUILD's OUTPUT group (mitcad#375): the cut list (the `build_cut_list`
// query: the cut parts grouped by identical result, their stock and the
// other parts) and the parts list (`parts_list`), each a table that copies
// as CSV and saves as a PDF with the design's name and the date.

#include <QDialog>
#include <QJsonObject>
#include <QString>
#include <QStringList>
#include <QVector>

class QDoubleSpinBox;
class QTabWidget;
class QTableWidget;

namespace mitcad {

class CommandContext;

namespace build {

// A table of a list: its heading, the column names and the rows' cells.
struct ListTable {
  QString heading;
  QStringList columns;
  QVector<QStringList> rows;
};

// The tables as CSV: each table's columns and rows, a blank line between
// tables, fields quoted where they need it.
QString csvOf(const QVector<ListTable>& tables);
// Writes the tables to a PDF (A4) under a title and a line with the
// design's name and the date; false with the reason when it cannot.
bool writeListPdf(const QString& path, const QString& title, const QString& subtitle,
                  const QVector<ListTable>& tables, QString& error);

// The cut list's tables from `build_cut_list`'s answer: the rows, the
// stock, the other parts.
QVector<ListTable> cutListTables(const QJsonObject& answer);
// The parts list's table from `parts_list`'s rows.
QVector<ListTable> partsListTables(const QJsonObject& answer);

class BuildListsDialog : public QDialog {
  Q_OBJECT

public:
  enum class Page { CutList, PartsList };

  // `design` is the design's name for the PDF.
  BuildListsDialog(const CommandContext& model, const QString& design, Page page, QWidget* parent = nullptr);

private:
  void load();
  void copyCsv();
  void savePdf();
  QVector<ListTable> shownTables() const;

  const CommandContext& m_model;
  QString m_design;
  QTabWidget* m_tabs = nullptr;
  QDoubleSpinBox* m_kerf = nullptr;
  QVector<QTableWidget*> m_cutTables; // rows, stock, other
  QTableWidget* m_partsTable = nullptr;
  QVector<ListTable> m_cut;
  QVector<ListTable> m_parts;
};

} // namespace build
} // namespace mitcad
