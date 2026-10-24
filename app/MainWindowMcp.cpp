// SPDX-License-Identifier: MIT
// Live MCP operates on the window's document, through its ordinary command
// workflow. It never owns another model or accesses the worker's document.
// The application's own state and actions (ui_state, ui_select, ui_command,
// ...) are in MainWindowMcpUi.cpp (mitcad#265).
#include "MainWindow.hpp"

#include <stdexcept>
#include <utility>

#include <QAction>
#include <QApplication>
#include <QDebug>
#include <QDir>
#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QLabel>
#include <QList>
#include <QScopedValueRollback>
#include <QString>
#include <QThread>

#include "browser/BrowserController.hpp"
#include "files/LockController.hpp"
#include "files/ProjectIndicator.hpp"
#include "framework/Json.hpp"
#include "mcp/AgentFolders.hpp"
#include "mcp/LiveErrors.hpp"
#include "mcp/ModalGuard.hpp"
#ifdef MITCAD_MACOS_NATIVE_UI
#include "platform/NativeDialogs.hpp"
#include "platform/NativeMacWindow.hpp"
#endif

namespace mitcad {
namespace {

// What an agent's change says in the undo text and the history (mitcad#265).
const QString kAgentSuffix = QStringLiteral(" (agent)");

[[noreturn]] void refuse(const QString& reason) {
  qInfo().noquote() << QStringLiteral("MCP refused: %1").arg(reason);
  throw std::runtime_error(reason.toStdString());
}

} // namespace

void MainWindow::mcpRefuseBusy() const {
  // Only computation refuses everything, queries too (mitcad#265): the
  // agent is told when to retry.
  int retry = 0;
  QString reason;
  if (m_mcpRequestActive) {
    reason = tr("Another external request is running");
    retry = 500;
  } else if (modelBusy()) {
    reason = tr("The model is computing");
    retry = 1000;
  } else if (m_viewer->isPainting()) {
    reason = tr("The view is drawing");
    retry = 100;
  } else if (!m_import.isNull()) {
    reason = tr("An import is running");
    retry = 2000;
  }
  if (retry > 0) {
    const QString message = tr("busy: %1; retry in %2 ms").arg(reason).arg(retry);
    qInfo().noquote() << QStringLiteral("MCP refused: %1").arg(message);
    throw mcp::Refusal(message, {{QStringLiteral("busy"), true}, {QStringLiteral("retry_after_ms"), retry}});
  }
}

QString MainWindow::mcpBlockedReason(bool editing) const {
  if (m_mcpRequestActive) {
    return tr("Another external model request is running.");
  }
  if (modelBusy() || m_viewer->isPainting()) {
    return tr("The document is busy; retry when computation or drawing has finished.");
  }
  // A sketch being edited takes its own commands (mcpSketchCommand); a
  // command panel or a plane pick takes none (mitcad#265).
  if (m_mode == Mode::Command || m_session || m_panelScroll) {
    const QString name = m_session ? m_session->def().name : tr("a command");
    return tr("The command panel of %1 is open: finish it (ui_command with finish ok or cancel) before changing the "
              "model; queries and the ui tools work meanwhile.")
        .arg(name);
  }
  if (m_mode == Mode::PickPlane) {
    return tr("Create Sketch waits for a plane to be picked: pick one or cancel it first.");
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
  // Read-only: also with a sketch or command open, during drags and
  // dialogs; refused only while the model computes (mitcad#265).
  mcpRefuseBusy();
  // The shared MCP validator excludes disk reads and external workflows.
  validate_mcp_query(rustStr(compactJson(request)));
  return {{QStringLiteral("document"), m_mcpDocumentId}, {QStringLiteral("result"), query(request)}};
}

QJsonObject MainWindow::mcpCommand(const QJsonObject& request) {
  Q_ASSERT(QThread::currentThread() == thread());
  mcpRefuseBusy();
  const QString refusal = mcpBlockedReason(true);
  if (!refusal.isEmpty()) {
    refuse(refusal);
  }
  // By class: file IO, remote and internal checkpoint commands stay
  // unavailable (mitcad#265, core/model/src/api/classes.rs).
  idleDocument().validate_mcp_document_command(rustStr(compactJson(request)));
  const QScopedValueRollback<bool> active(m_mcpRequestActive, true);
  if (m_mode == Mode::Sketch) {
    return mcpSketchCommand(request);
  }
  const QString kind = request.value(QStringLiteral("cmd")).toString();
  const int depth = undoDepth();
  QJsonObject answer;
  if (!runModelCommand(request, &answer)) {
    const QString failure = lastError().isEmpty() ? tr("The model command was refused.") : lastError();
    refuse(failure);
  }
  if (kind != QStringLiteral("undo") && kind != QStringLiteral("redo")) {
    mcpLabelAgentChange(depth);
  }
  qInfo().noquote() << QStringLiteral("MCP: an agent ran %1").arg(kind);
  return {{QStringLiteral("document"), m_mcpDocumentId},
          {QStringLiteral("applied"), true},
          {QStringLiteral("result"), answer},
          {QStringLiteral("state"), query({{QStringLiteral("query"), QStringLiteral("document")}})},
          {QStringLiteral("timeline"), query({{QStringLiteral("query"), QStringLiteral("timeline")}})},
          {QStringLiteral("bodies"), query({{QStringLiteral("query"), QStringLiteral("bodies")}})}};
}

QJsonObject MainWindow::mcpSketchCommand(const QJsonObject& request) {
  // The sketch being edited: its own commands go through the sketch
  // session, as the user's would (mitcad#265): one undo step, the view,
  // the palette and the sketch's undo follow.
  const QString kind = request.value(QStringLiteral("cmd")).toString();
  const QString uid = m_sketch->uid();
  const QString sketchName = m_sketch->model().name;
  QJsonObject answer;
  if (kind == QStringLiteral("undo") || kind == QStringLiteral("redo")) {
    const QString label =
        queryObject({{QStringLiteral("query"), QStringLiteral("document")}}).value(kind).toString();
    if (label.isEmpty()) {
      refuse(tr("Nothing to %1.").arg(kind));
    }
    if (kind == QStringLiteral("undo")) {
      undo();
    } else {
      redo();
    }
    answer = {{QStringLiteral("label"), label}};
  } else {
    const bool own =
        (kind.startsWith(QStringLiteral("sketch.")) && request.value(QStringLiteral("sketch")).toString() == uid) ||
        (kind == QStringLiteral("edit_feature") && request.value(QStringLiteral("uid")).toString() == uid);
    if (!own) {
      refuse(tr("%1 (%2) is being edited: only its own sketch commands (with \"sketch\": \"%2\"), edit_feature of "
                "it, undo and redo run now. Finish it (ui_command with id sketch.finish) for other commands.")
                 .arg(sketchName, uid));
    }
    const int depth = undoDepth();
    sketch::SketchOp op(*m_sketch);
    if (!op.run(request, &answer)) {
      const QString reason = op.error();
      op.rollback();
      refuse(reason.isEmpty() ? tr("The sketch command was refused.") : reason);
    }
    op.commit();
    m_mcpSketchEdited = true;
    mcpLabelAgentChange(depth);
  }
  qInfo().noquote() << QStringLiteral("MCP: an agent ran %1 in sketch %2").arg(kind, uid);
  return {{QStringLiteral("document"), m_mcpDocumentId},
          {QStringLiteral("applied"), true},
          {QStringLiteral("sketch"), uid},
          {QStringLiteral("result"), answer},
          {QStringLiteral("state"), query({{QStringLiteral("query"), QStringLiteral("document")}})},
          {QStringLiteral("timeline"), query({{QStringLiteral("query"), QStringLiteral("timeline")}})},
          {QStringLiteral("bodies"), query({{QStringLiteral("query"), QStringLiteral("bodies")}})}};
}

void MainWindow::mcpLabelAgentChange(int depthBefore) {
  // The steps an agent added become one, labelled "... (agent)" in the
  // undo text and the version history (mitcad#265); they undo as one.
  try {
    const QJsonObject document = queryObject({{QStringLiteral("query"), QStringLiteral("document")}});
    const int depth = document.value(QStringLiteral("undo_depth")).toInt();
    const QString label = document.value(QStringLiteral("undo")).toString();
    if (depth <= depthBefore || label.isEmpty() || label.endsWith(kAgentSuffix)) {
      return;
    }
    if (runCommand({{QStringLiteral("cmd"), QStringLiteral("merge_undo")},
                    {QStringLiteral("depth"), depthBefore},
                    {QStringLiteral("label"), label + kAgentSuffix}})) {
      qInfo().noquote() << QStringLiteral("Agent change: %1").arg(label + kAgentSuffix);
      updateActions();
    }
  } catch (const std::exception&) {
    // The label stays as the command left it.
  }
}

QJsonObject MainWindow::mcpSave(const QJsonObject& arguments) {
  mcpRefuseBusy();
  // As File > Save does (mitcad#265): to the document's own path, with the
  // project's version and commit message; with a path as Save As does, into
  // a folder the user allows agents (mitcad#402).
  if (m_session || m_mode == Mode::Command) {
    refuse(tr("Finish the current command before saving."));
  }
  if (m_closePending || QApplication::activeModalWidget() != nullptr) {
    refuse(tr("The document is closing or waiting for a dialog."));
  }
  if (windowReadOnly()) {
    refuse(readOnlyReason());
  }
  QString target;
  if (arguments.contains(QStringLiteral("path"))) {
    QString reason;
    target = mcp::agentDesignPath(arguments.value(QStringLiteral("path")).toString(), true,
                                  arguments.value(QStringLiteral("overwrite")).toBool(false), reason);
    if (target.isEmpty()) {
      refuse(reason);
    }
  } else if (m_filePath.isEmpty()) {
    refuse(tr("%1 has never been saved: it has no path to save to. Give a path in a folder agents may use "
              "(document_list lists them), or save it in the application (File > Save As).")
               .arg(documentName()));
  }
  const QScopedValueRollback<bool> active(m_mcpRequestActive, true);
  bool saved = false;
  QString dialog;
  {
    mcp::ModalGuard guard;
    saved = target.isEmpty() ? save() : writeFile(target);
    dialog = guard.opened();
  }
  if (!dialog.isEmpty()) {
    refuse(tr("Saving asked a question (%1), which an agent cannot answer: save in the application.").arg(dialog));
  }
  if (!saved || isModified()) {
    refuse(m_statusLabel->text().isEmpty() ? tr("The document was not saved.") : m_statusLabel->text());
  }
  qInfo().noquote() << QStringLiteral("MCP: an agent saved %1").arg(m_filePath);
  return {{QStringLiteral("document"), m_mcpDocumentId},
          {QStringLiteral("path"), m_filePath},
          {QStringLiteral("saved"), true},
          {QStringLiteral("status"), m_statusLabel->text()},
          {QStringLiteral("state"), query({{QStringLiteral("query"), QStringLiteral("document")}})}};
}

QJsonObject MainWindow::mcpOpen(const QJsonObject& arguments) {
  mcpRefuseBusy();
  // As File > Open does, without its dialogs (mitcad#402): a design from a
  // folder the user allows agents; the window's document is replaced and
  // with it the handle.
  QString reason;
  const QString path = mcp::agentDesignPath(arguments.value(QStringLiteral("path")).toString(), false, false, reason);
  if (path.isEmpty()) {
    refuse(reason);
  }
  if (!m_session && m_mode == Mode::PickPlane) {
    refuse(tr("Create Sketch waits for a plane to be picked: pick one or cancel it first."));
  }
  if (m_session || m_mode != Mode::Idle) {
    refuse(tr("Finish the sketch or command first (ui_command with finish ok or cancel)."));
  }
  // What else keeps a model command from running: drags, imports, dialogs,
  // a native command form.
  const QString blocked = mcpBlockedReason(false);
  if (!blocked.isEmpty()) {
    refuse(blocked);
  }
  const bool discard = arguments.value(QStringLiteral("discard")).toBool(false);
  if (!discard && (windowReadOnly() ? m_locks->keptChanges() : isModified())) {
    refuse(tr("%1 has unsaved changes: save them first (document_save), or pass discard: true to lose them.")
               .arg(documentName()));
  }
  const QString before = m_mcpDocumentId;
  const QScopedValueRollback<bool> active(m_mcpRequestActive, true);
  QString error;
  bool opened = false;
  QString dialog;
  {
    mcp::ModalGuard guard;
    opened = openFile(path, error);
    dialog = guard.opened();
  }
  if (!dialog.isEmpty()) {
    refuse(tr("Opening asked a question (%1), which an agent cannot answer: open it in the application.").arg(dialog));
  }
  if (!opened || m_mcpDocumentId == before) {
    refuse(error.isEmpty() ? tr("%1 was not opened.").arg(QDir::toNativeSeparators(path))
                           : tr("Could not open %1: %2").arg(QDir::toNativeSeparators(path), error));
  }
  qInfo().noquote() << QStringLiteral("MCP: an agent opened %1").arg(m_filePath);
  QJsonObject result = mcpDocumentInfo();
  result.insert(QStringLiteral("opened"), true);
  result.insert(QStringLiteral("replaced"), before);
  return result;
}

void MainWindow::setMcpAgents(int count, std::function<void()> disconnectAll) {
  m_mcpDisconnectAll = std::move(disconnectAll);
  if (count == m_mcpAgents) {
    return;
  }
  const bool before = m_mcpAgents > 0;
  m_mcpAgents = count;
  if (m_mcpDisconnect == nullptr) {
    m_mcpDisconnect = new QAction(tr("Disconnect Agent"), this);
    m_mcpDisconnect->setObjectName(QStringLiteral("disconnectAgent"));
    m_mcpDisconnect->setMenuRole(QAction::NoRole);
    connect(m_mcpDisconnect, &QAction::triggered, this, [this] {
      qInfo().noquote() << QStringLiteral("Agent disconnected by the user");
      if (m_mcpDisconnectAll) {
        m_mcpDisconnectAll();
      }
    });
  }
  m_indicator->setAgentText(count == 0   ? QString()
                            : count == 1 ? tr("Agent connected")
                                         : tr("%1 agents connected").arg(count));
  m_indicator->setAgentActions(count > 0 ? QList<QAction*>{m_mcpDisconnect} : QList<QAction*>{});
  if (count > 0 && !before) {
    qInfo().noquote() << QStringLiteral("Agent connected");
    showHint(tr("An agent is connected through MCP; the project indicator's menu disconnects it."));
  } else if (count == 0 && before) {
    qInfo().noquote() << QStringLiteral("Agent disconnected");
    showHint(tr("The agent disconnected."));
  }
}

QJsonObject MainWindow::mcpTool(const QString& name, const QJsonObject& arguments) {
  Q_ASSERT(QThread::currentThread() == thread());
  if (name == QStringLiteral("document_list")) {
    // With the folders document_save's path and document_open may use
    // (mitcad#402).
    QJsonArray folders;
    for (const QString& folder : mcp::agentFolders()) {
      folders.append(QDir::toNativeSeparators(folder));
    }
    return {{QStringLiteral("documents"), QJsonArray{mcpDocumentInfo()}}, {QStringLiteral("folders"), folders}};
  }
  // The window's document replaced: no handle (mitcad#402).
  if (name == QStringLiteral("document_open")) {
    return mcpOpen(arguments);
  }
  // What the window shows: no document handle.
  if (name == QStringLiteral("ui_state")) {
    return mcpUiState();
  }
  if (name == QStringLiteral("ui_log")) {
    return mcpLog(arguments);
  }
  if (name == QStringLiteral("ui_screenshot")) {
    return mcpScreenshot(arguments);
  }
  if (name == QStringLiteral("ui_camera")) {
    return mcpCamera(arguments);
  }
  if (arguments.value(QStringLiteral("document")).toString() != m_mcpDocumentId) {
    throw std::runtime_error("No open document with this handle; call document_list after New or Open");
  }
  if (name == QStringLiteral("model_query")) {
    return mcpQuery(arguments.value(QStringLiteral("query")).toObject());
  }
  if (name == QStringLiteral("model_command")) {
    return mcpCommand(arguments.value(QStringLiteral("command")).toObject());
  }
  if (name == QStringLiteral("document_save")) {
    return mcpSave(arguments);
  }
  if (name == QStringLiteral("ui_select")) {
    return mcpSelect(arguments);
  }
  if (name == QStringLiteral("ui_command")) {
    return mcpUiCommand(arguments);
  }
  throw std::runtime_error("Unknown live document tool");
}

} // namespace mitcad
