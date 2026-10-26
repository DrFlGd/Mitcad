// SPDX-License-Identifier: MIT
#include "../MainWindow.hpp"
#include "ConnectAssistantDialog.hpp"
#include "../framework/AppSettings.hpp"
#include "../framework/Dialogs.hpp"

#include <QAction>

namespace mitcad {

void MainWindow::showConnectAssistant() {
  if (auto* existing = findChild<QDialog*>(QStringLiteral("connectAssistantDialog"))) {
    existing->show(); existing->raise(); existing->activateWindow(); return;
  }
  auto* dialog = new mcp::ConnectAssistantDialog({
      [this] { return m_agentIndicator->agents(); },
      [this](const QStringList& ids) { if (m_mcpDisconnect) m_mcpDisconnect(ids); },
      projectsDirectory()}, this);
  showDialog(dialog);
}

} // namespace mitcad
