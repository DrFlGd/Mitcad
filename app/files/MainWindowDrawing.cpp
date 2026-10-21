// SPDX-License-Identifier: MIT
// File > Export Drawing (mitcad#471, DrawingDialog.hpp): the selected
// bodies, or every shown one, as a drawing with views, dimensions and a
// hole table, written by the model's export_drawing as PDF, SVG or DXF.
#include "../MainWindow.hpp"

#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonObject>
#include <QtLogging>

#include "../framework/Dialogs.hpp"
#include "../framework/WaitCursor.hpp"
#include "DrawingDialog.hpp"

namespace mitcad {
namespace {

// The bodies a selection stands for: bodies, and the bodies of faces,
// edges and vertices (as Export takes them).
QStringList bodiesOf(const Selection& selection) {
  QStringList uids;
  for (const SelectionItem& item : selection) {
    const bool onBody = item.kind == SelectKind::Body || item.kind == SelectKind::Face ||
                        item.kind == SelectKind::Edge || item.kind == SelectKind::Vertex;
    if (onBody && !uids.contains(item.owner)) {
      uids << item.owner;
    }
  }
  return uids;
}

} // namespace

void MainWindow::exportDrawing() {
  const QStringList selected = bodiesOf(m_selection);
  QStringList analyses;
  for (const QJsonValue value : std::as_const(m_snapshot.analyses)) {
    const QJsonObject analysis = value.toObject();
    if (analysis.value(QStringLiteral("type")).toString() == QStringLiteral("section")) {
      analyses << analysis.value(QStringLiteral("name")).toString();
    }
  }
  const QString base = m_filePath.isEmpty() ? documentName() : QFileInfo(m_filePath).completeBaseName();
  const QString suggested = QDir(dialogFolder(DialogGroup::Export)).filePath(base);
  const std::optional<DrawingChoice> choice =
      askDrawing(this, suggested, static_cast<int>(selected.size()), analyses);
  if (!choice) {
    return;
  }
  rememberDialogFolder(DialogGroup::Export, choice->path);
  const QJsonObject command =
      choice->settings.command(choice->path, choice->selectedOnly ? selected : QStringList(), base, choice->section);
  QJsonObject result;
  bool written = false;
  {
    const WaitCursor wait(QStringLiteral("Export drawing"));
    written = runCommand(command, &result);
  }
  if (!written) {
    qWarning().noquote() << QStringLiteral("Export of %1 failed: %2").arg(choice->path, m_lastError);
    sheetWarning(this, tr("Export Drawing"), tr("Could not export:\n%1").arg(m_lastError));
    return;
  }
  for (const QJsonValue warning : result.value(QStringLiteral("warnings")).toArray()) {
    qWarning().noquote() << QStringLiteral("Export drawing: %1").arg(warning.toString());
  }
  const int views = result.value(QStringLiteral("views")).toInt();
  const auto bodies = result.value(QStringLiteral("bodies")).toArray().size();
  const QString sheet = result.value(QStringLiteral("sheet")).toString();
  const QString scale = result.value(QStringLiteral("scale")).toString();
  qInfo().noquote() << QStringLiteral("Exported %1: drawing, %2 views, %3 bodies, %4, %5")
                           .arg(choice->path)
                           .arg(views)
                           .arg(bodies)
                           .arg(sheet, scale);
  const int sheets = result.value(QStringLiteral("sheets")).toInt();
  showHint(tr("Exported %1 (%2 view(s) of %3 body(ies), %4 at %5%6).")
               .arg(QDir::toNativeSeparators(choice->path))
               .arg(views)
               .arg(bodies)
               .arg(QString(sheet).replace(QLatin1Char('_'), QLatin1Char(' ')), scale,
                    sheets > 1 ? tr(", %1 sheets").arg(sheets) : QString()));
}

} // namespace mitcad
