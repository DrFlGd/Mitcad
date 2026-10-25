// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
#include <QString>

#include "PrintImportSummary.hpp"

class QPushButton;

namespace mitcad {

// Details… of an import from a slicer (mitcad#654, mitcad#655): the whole
// report (PrintImportSummary: its title and the sections that have
// something, under bold headings) in a small read-only view titled
// Import Details, with Copy (the report to the clipboard) and Close. Logs
// "Import details opened: <n> lines", each line as "Import details:
// <line>", its buttons' places ("Import details copy at x,y", `close`)
// and "Import details closed".
class ImportDetailsDialog : public QDialog {
  Q_OBJECT

public:
  ImportDetailsDialog(const PrintImportSummary& summary, QWidget* parent = nullptr);

  // Shows the report modally (a sheet on macOS) and logs it.
  static void showReport(const PrintImportSummary& summary, QWidget* parent);

protected:
  void showEvent(QShowEvent* event) override;

private:
  void logPlaces();

  QString m_report;
  QPushButton* m_copy = nullptr;
  QPushButton* m_close = nullptr;
};

} // namespace mitcad
