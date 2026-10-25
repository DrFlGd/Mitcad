// SPDX-License-Identifier: MIT
#include "TestDriver.hpp"
#include "../MainWindow.hpp"
#include "../mcp/AssistantSetup.hpp"
#include "../mcp/AgentFolders.hpp"
#include "../mcp/ConnectAssistantDialog.hpp"

#include <QApplication>
#include <QClipboard>
#include <QComboBox>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QRegularExpression>
#include <QTimer>

namespace mitcad {

void TestDriver::testAssistantSetup(const QString& mode) {
  mcp::ConnectAssistantDialog* dialog = nullptr;
  for (QWidget* top : QApplication::topLevelWidgets()) {
    if (top->isVisible()) {
      if (auto* candidate = dynamic_cast<mcp::ConnectAssistantDialog*>(top)) dialog = candidate;
    }
  }
  if (!dialog) { fail(m_line, QStringLiteral("Connect AI Assistant dialog missing")); return; }
  auto* client = dialog->findChild<QComboBox*>(QStringLiteral("assistantClient"));
  auto* config = dialog->findChild<QLineEdit*>(QStringLiteral("assistantConfigPath"));
  auto* setup = dialog->findChild<QPushButton*>(QStringLiteral("assistantSetup"));
  auto* status = dialog->findChild<QLabel*>(QStringLiteral("assistantConnectionState"));
  auto* result = dialog->findChild<QLabel*>(QStringLiteral("assistantSetupResult"));
  if (!client || client->count() != 4 || !config || !setup || !status || !result) {
    fail(m_line, QStringLiteral("assistant choices or setup controls missing")); return;
  }
  const QString work = qEnvironmentVariable("MITCAD_ASSISTANT_TEST_WORK");
  if (work.isEmpty()) { fail(m_line, QStringLiteral("assistant setup test requires an isolated work folder")); return; }
  const QString original = QStringLiteral("# Keep the user's comment\nmodel = 'unchanged'\n");
  if (mode == QStringLiteral("other")) {
    client->setCurrentIndex(client->findData(int(mcp::AssistantClient::Other)));
    auto* copy = dialog->findChild<QPushButton*>(QStringLiteral("assistantCopyConfiguration"));
    copy->click();
    const auto data = QJsonDocument::fromJson(QApplication::clipboard()->text().toUtf8());
    if (!data.object().contains(QStringLiteral("mcpServers")) || setup->isEnabled()) {
      fail(m_line, QStringLiteral("generic client fallback has the wrong format or an automatic setup action"));
    }
  } else if (mode == QStringLiteral("folder")) {
    auto* folder = dialog->findChild<QLineEdit*>(QStringLiteral("assistantFolder"));
    auto* allow = dialog->findChild<QPushButton*>(QStringLiteral("assistantAllowFolder"));
    const QString path = QDir(work).filePath(QStringLiteral("explicitly allowed/new folder"));
    folder->setText(path); allow->click();
    QString reason;
    if (!QFileInfo(path).isDir() || !mcp::savedAgentFolders().contains(QFileInfo(path).canonicalFilePath()) ||
        mcp::agentDesignPath(QDir(path).filePath(QStringLiteral("design.mitcad")), true, false, reason).isEmpty() ||
        !mcp::agentDesignPath(QDir(work).filePath(QStringLiteral("outside.mitcad")), true, false, reason).isEmpty()) {
      fail(m_line, QStringLiteral("explicit folder consent did not preserve file access boundaries"));
    }
  } else if (mode == QStringLiteral("missing-claude")) {
    client->setCurrentIndex(client->findData(int(mcp::AssistantClient::ClaudeCode)));
    setup->click();
    if (!result->property("setupError").toBool() || !result->text().contains(QStringLiteral("not installed"))) {
      fail(m_line, QStringLiteral("missing client was not reported before registration"));
    }
  } else if (mode == QStringLiteral("claude")) {
    const QString executable = QDir(qEnvironmentVariable("MITCAD_TEST_ASSISTANT_EXECUTABLES"))
#ifdef Q_OS_WIN
        .filePath(QStringLiteral("claude.exe"));
#else
        .filePath(QStringLiteral("claude"));
#endif
    if (!QFile::copy(qEnvironmentVariable("MITCAD_TEST_ASSISTANT_STUB"), executable)) {
      fail(m_line, QStringLiteral("isolated Claude client could not be prepared")); return;
    }
    QFile::setPermissions(executable, QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);
    client->setCurrentIndex(client->findData(int(mcp::AssistantClient::ClaudeCode)));
    QTimer::singleShot(100, dialog, [] {
      for (QWidget* top : QApplication::topLevelWidgets()) {
        if (auto* question = qobject_cast<QMessageBox*>(top); question &&
            dialogMatches(question, QRegularExpression(QStringLiteral("^Replace Mitcad Connection$|^This client already has a different Mitcad entry\\.")))) {
          question->button(QMessageBox::Yes)->click();
        }
      }
    });
    // Establish the driver's wait before a nested confirmation event loop
    // can advance the script to another client choice.
    QTimer::singleShot(0, setup, [setup] { setup->click(); });
    const bool failure = !qEnvironmentVariableIsEmpty("MITCAD_TEST_ASSISTANT_FAIL") ||
                         !qEnvironmentVariableIsEmpty("MITCAD_TEST_ASSISTANT_CORRUPT") ||
                         !qEnvironmentVariableIsEmpty("MITCAD_TEST_ASSISTANT_CONCURRENT");
    whenReady([status, result, setup, failure, work] {
      return QFileInfo(QDir(work).filePath(QStringLiteral("claude-registration-argv.json"))).isFile() &&
          setup->isEnabled() && (failure ? !result->text().isEmpty() && result->property("setupError").toBool()
          : status->text() == QStringLiteral("Configured, waiting for connection"));
    }, 10, QStringLiteral("Claude registration did not reach its verified state"), [this, setup, failure, work] {
      if (!failure) {
        const auto backups = QDir(work).entryList({QStringLiteral("claude.json.mitcad-backup-*")}, QDir::Files);
        setup->click();
        if (QDir(work).entryList({QStringLiteral("claude.json.mitcad-backup-*")}, QDir::Files) != backups) {
          fail(m_line, QStringLiteral("repeating Claude registration made another backup"));
        }
      }
    });
  } else if (mode == QStringLiteral("bundle")) {
    client->setCurrentIndex(client->findData(int(mcp::AssistantClient::ClaudeDesktop)));
    setup->click();
    if (!QFileInfo(dialog->property("assistantBundlePath").toString()).isFile()) {
      fail(m_line, QStringLiteral("native MCP Bundle was not prepared for the isolated installer"));
    }
  } else if (mode == QStringLiteral("blocked")) {
    client->setCurrentIndex(client->findData(int(mcp::AssistantClient::Codex)));
    config->setText(QDir(work).filePath(QStringLiteral("managed/config.toml")));
    setup->click();
    if (!result->property("setupError").toBool() || !result->text().contains(QStringLiteral("Managed"))) {
      fail(m_line, QStringLiteral("managed settings were not protected"));
    }
  } else if (mode == QStringLiteral("codex")) {
    client->setCurrentIndex(client->findData(int(mcp::AssistantClient::Codex)));
    auto* copy = dialog->findChild<QPushButton*>(QStringLiteral("assistantCopyCommand"));
    copy->click();
    if (!QApplication::clipboard()->text().contains(QStringLiteral("CODEX_HOME"))) {
      fail(m_line, QStringLiteral("copied command omitted the selected Codex home")); return;
    }
    const QString selected = config->text();
    config->setText(QDir(work).filePath(QStringLiteral("custom.toml"))); copy->click();
    if (!QApplication::clipboard()->text().startsWith(QStringLiteral("[mcp_servers.mitcad-desktop]"))) {
      fail(m_line, QStringLiteral("nonstandard Codex settings copied a misleading CLI registration")); return;
    }
    config->setText(selected);
    setup->click();
    const bool failure = !qEnvironmentVariableIsEmpty("MITCAD_TEST_ASSISTANT_FAIL") ||
                         !qEnvironmentVariableIsEmpty("MITCAD_TEST_ASSISTANT_CORRUPT") ||
                         !qEnvironmentVariableIsEmpty("MITCAD_TEST_ASSISTANT_CONCURRENT");
    const QString expected = original + (!qEnvironmentVariableIsEmpty("MITCAD_TEST_ASSISTANT_CONCURRENT")
        ? QStringLiteral("# Concurrent user edit\n") : QString());
    whenReady([status, result, setup, failure] {
      return setup->isEnabled() && (failure ? result->property("setupError").toBool()
          : status->text() == QStringLiteral("Configured, waiting for connection"));
    }, 10, QStringLiteral("assistant registration did not reach the expected setup state"),
      [this, config, setup, failure, original, expected] {
        QString content, error;
        if (!mcp::readAssistantConfiguration(config->text(), content, error)) { fail(m_line, error); return; }
        if (!content.startsWith(original) || (failure && content != expected)) {
          fail(m_line, QStringLiteral("assistant setup changed unrelated text or failed to restore a rejected write")); return;
        }
        if (!failure) {
          const QDir folder(QFileInfo(config->text()).absolutePath());
          const auto backups = folder.entryList({QStringLiteral("*.mitcad*backup-*")}, QDir::Files);
          setup->click(); // An existing matching entry is idempotent.
          if (folder.entryList({QStringLiteral("*.mitcad*backup-*")}, QDir::Files) != backups) {
            fail(m_line, QStringLiteral("repeating assistant setup made another backup"));
          }
        }
      });
  } else {
    fail(m_line, QStringLiteral("unknown assistant setup test mode")); return;
  }
  qInfo().noquote() << QStringLiteral("Assistant setup UI: %1").arg(mode);
}

} // namespace mitcad
