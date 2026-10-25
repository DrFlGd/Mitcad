// SPDX-License-Identifier: MIT
// View > Export Snapshot (mitcad#649, app/COMMANDS.md "Measurement
// objects"): a picture of the view, larger than the view, with the shown
// measurements drawn over it.
#include <optional>

#include <QDir>
#include <QImage>
#include <QtLogging>

#include "../browser/DocumentHost.hpp"
#include "../framework/DialogFolders.hpp"
#include "SnapshotDialog.hpp"
#include "ViewController.hpp"

namespace mitcad {

void ViewController::exportSnapshot() {
  const QString name = (m_documentName.isEmpty() ? tr("Snapshot") : m_documentName) + QStringLiteral(".png");
  const QString folder = dialogStartFolder(DialogGroup::Export, m_host.documentFolder());
  const QString start = folder.isEmpty() ? name : QDir(folder).filePath(name);
  const int shown = static_cast<int>(m_viewer.measurements().size());
  const std::optional<SnapshotChoice> choice = askSnapshot(m_window, start, m_viewer.size(), shown);
  if (!choice) {
    return;
  }
  rememberDialogFolder(DialogGroup::Export, choice->path);
  const SnapshotChoice chosen = *choice;
  // The view's shapes are read only while no model job writes into them.
  m_host.whenIdle(this, [this, chosen] {
    const QImage image = m_viewer.snapshot(chosen.size, chosen.transparent, chosen.measurements);
    const QString shownPath = QDir::toNativeSeparators(chosen.path);
    if (image.isNull() || !image.save(chosen.path)) {
      m_host.showStatus(tr("The snapshot cannot be saved as %1.").arg(shownPath), true);
      qWarning().noquote() << QStringLiteral("Snapshot not saved: %1 (%2)")
                                  .arg(chosen.path, image.isNull() ? QStringLiteral("the view rendered no picture")
                                                                   : QStringLiteral("the file cannot be written"));
      return;
    }
    m_host.showStatus(tr("Snapshot saved as %1.").arg(shownPath), false);
    const int drawn = chosen.measurements ? static_cast<int>(m_viewer.measurements().size()) : 0;
    qInfo().noquote() << QStringLiteral("Snapshot saved %1: %2 x %3, %4, %5 measurement(s)")
                             .arg(chosen.path)
                             .arg(image.width())
                             .arg(image.height())
                             .arg(image.hasAlphaChannel() ? QStringLiteral("transparent") : QStringLiteral("opaque"))
                             .arg(drawn);
  });
}

} // namespace mitcad
