// SPDX-License-Identifier: MIT
// Live MCP operates on the window's document, through its ordinary command
// workflow. It never owns another model or accesses the worker's document.
#include "MainWindow.hpp"

#include <stdexcept>

#include <QApplication>
#include <QScopedValueRollback>
#include <QThread>

#include "browser/BrowserController.hpp"
#include "framework/Json.hpp"
#ifdef MITCAD_MACOS_NATIVE_UI
#include "platform/NativeDialogs.hpp"
#include "platform/NativeMacWindow.hpp"
#endif

namespace mitcad {

QString MainWindow::mcpBlockedReason(bool editing) const {
  if (m_mcpRequestActive) {
    return tr("Another external model request is running.");
  }
  if (modelBusy() || m_viewer->isPainting()) {
    return tr("The document is busy; retry when computation or drawing has finished.");
  }
  if (m_mode != Mode::Idle || m_session || m_panelScroll) {
    return tr("Finish the current command or sketch before accessing the model externally.");
  }
  if (m_drag || m_animation || (m_controller && m_controller->markerDragActive())) {
    return tr("Finish dragging or animating the model before accessing it externally.");
  }
  if (!m_import.isNull()) {
    return tr("An import is running; retry when it has finished.");
  }
  if (m_closePending || QApplication::activeModalWidget() != nullptr) {
    return tr("The document is closing or waiting for a dialog.");
  }
#ifdef MITCAD_MACOS_NATIVE_UI
  if (m_nativeForm || m_nativeClosingCommand || nativeDialogModalActive() ||
      (m_nativeWindow && m_nativeWindow->modalBlocked())) {
    return tr("The document is waiting for a native command or dialog.");
  }
#endif
  if (editing && windowReadOnly()) {
    return readOnlyReason();
  }
  return QString();
}

QJsonObject MainWindow::mcpDocumentInfo() const {
  Q_ASSERT(QThread::currentThread() == thread());
  const DocumentSession::Snapshot snapshot = m_documentSession->snapshot();
  return {{QStringLiteral("document"), m_mcpDocumentId},
          {QStringLiteral("path"), snapshot.filePath.isEmpty() ? QJsonValue(QJsonValue::Null)
                                                              : QJsonValue(snapshot.filePath)},
          {QStringLiteral("name"), snapshot.name},
          {QStringLiteral("state"), snapshot.document},
          {QStringLiteral("busy"), snapshot.busy},
          {QStringLiteral("editable"), mcpBlockedReason(true).isEmpty()}};
}

QJsonObject MainWindow::mcpQuery(const QJsonObject& request) const {
  Q_ASSERT(QThread::currentThread() == thread());
  const QString refusal = mcpBlockedReason(false);
  if (!refusal.isEmpty()) {
    throw std::runtime_error(refusal.toStdString());
  }
  // The shared MCP validator excludes disk reads and external workflows.
  validate_mcp_query(rustStr(compactJson(request)));
  return {{QStringLiteral("document"), m_mcpDocumentId}, {QStringLiteral("result"), query(request)}};
}

QJsonObject MainWindow::mcpCommand(const QJsonObject& request) {
  Q_ASSERT(QThread::currentThread() == thread());
  const QString refusal = mcpBlockedReason(true);
  if (!refusal.isEmpty()) {
    throw std::runtime_error(refusal.toStdString());
  }
  // Internal checkpoints, file IO and remote commands stay unavailable.
  validate_mcp_command(rustStr(compactJson(request)));
  const QScopedValueRollback<bool> active(m_mcpRequestActive, true);
  QJsonObject answer;
  if (!runModelCommand(request, &answer)) {
    const QString failure = lastError().isEmpty() ? tr("The model command was refused.") : lastError();
    throw std::runtime_error(failure.toStdString());
  }
  return {{QStringLiteral("document"), m_mcpDocumentId},
          {QStringLiteral("applied"), true},
          {QStringLiteral("result"), answer},
          {QStringLiteral("state"), query({{QStringLiteral("query"), QStringLiteral("document")}})},
          {QStringLiteral("timeline"), query({{QStringLiteral("query"), QStringLiteral("timeline")}})},
          {QStringLiteral("bodies"), query({{QStringLiteral("query"), QStringLiteral("bodies")}})}};
}

} // namespace mitcad
