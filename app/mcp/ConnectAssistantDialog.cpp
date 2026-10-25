// SPDX-License-Identifier: MIT
#include "ConnectAssistantDialog.hpp"

#include <exception>
#include <memory>
#include <utility>

#include <QApplication>
#include <QClipboard>
#include <QComboBox>
#include <QDateTime>
#include <QDesktopServices>
#include <QDialogButtonBox>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFormLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QProcess>
#include <QProcessEnvironment>
#include <QPushButton>
#include <QSaveFile>
#include <QSettings>
#include <QStandardPaths>
#include <QTimer>
#include <QTemporaryDir>
#include <QUrl>
#include <QUuid>
#include <QVBoxLayout>

#include "AgentFolders.hpp"
#include "LocalEndpoint.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/FitToScreen.hpp"
#include "../framework/Json.hpp"
#include "mitcad_bridge/lib.h"

namespace mitcad::mcp {
namespace {
constexpr bool windows =
#ifdef Q_OS_WIN
    true;
#else
    false;
#endif

QString jsonString(const QString& text) {
  const QByteArray list = QJsonDocument(QJsonArray{text}).toJson(QJsonDocument::Compact);
  return QString::fromUtf8(list.mid(1, list.size() - 2));
}

QString codexHomeCommand(const QString& command, const QString& path) {
  const QString home = QFileInfo(path).absolutePath();
  const QString quoted = windows ? assistantCommand(home, {}, true).mid(2) : assistantCommand(home, {}, false);
  return windows ? QStringLiteral("$env:CODEX_HOME = %1; %2").arg(quoted, command)
                 : QStringLiteral("CODEX_HOME=%1 %2").arg(quoted, command);
}

QJsonObject codexPlan(const QString& current, const QString& bridge, bool replace = false) {
  return parseObject(plan_codex_mcp_config(rustStr(current.toUtf8()), rustStr(bridge.toUtf8()), replace));
}

bool confirmReplacement(QWidget* parent) {
  return sheetQuestion(parent, QObject::tr("Replace Mitcad Connection"),
      QObject::tr("This client already has a different Mitcad entry. Replace only that entry? "
                  "The existing configuration will be backed up."), QMessageBox::Yes | QMessageBox::No,
                  QMessageBox::No) == QMessageBox::Yes;
}

QString claudeUserConfiguration() {
  const QString work = qEnvironmentVariable("MITCAD_ASSISTANT_TEST_WORK");
  if (!qEnvironmentVariableIsEmpty("MITCAD_TEST_INPUT") && !work.isEmpty()) {
    return QDir(work).filePath(QStringLiteral("claude.json"));
  }
  const QString custom = qEnvironmentVariable("CLAUDE_CONFIG_DIR");
  if (!custom.isEmpty()) return QDir(custom).absoluteFilePath(QStringLiteral(".claude.json"));
  return QDir::home().filePath(QStringLiteral(".claude.json"));
}
}

ConnectAssistantDialog::ConnectAssistantDialog(Host host, QWidget* parent)
    : QDialog(parent), m_host(std::move(host)), m_bridge(installedBridge(QCoreApplication::applicationDirPath())) {
  setObjectName(QStringLiteral("connectAssistantDialog"));
  setWindowTitle(tr("Connect AI Assistant"));
  setAttribute(Qt::WA_DeleteOnClose);
  auto* outer = new QVBoxLayout(this);
  auto* area = new FittingScrollArea;
  area->setWidgetResizable(true);
  area->setFrameShape(QFrame::NoFrame);
  auto* page = new QWidget;
  auto* layout = new QVBoxLayout(page);
  auto* intro = new QLabel(tr("Connect an assistant on this computer to the open Mitcad design. "
      "You keep the normal editing and undo workflow. Client installation, sign-in and approvals stay with the client."));
  intro->setWordWrap(true); layout->addWidget(intro);
  auto* form = new QFormLayout;
  m_client = new QComboBox;
  m_client->setObjectName(QStringLiteral("assistantClient"));
  m_client->addItem(tr("Claude Desktop"), int(AssistantClient::ClaudeDesktop));
  m_client->addItem(tr("Claude Code"), int(AssistantClient::ClaudeCode));
  m_client->addItem(tr("Codex / ChatGPT Desktop (local MCP)"), int(AssistantClient::Codex));
  m_client->addItem(tr("Other MCP client"), int(AssistantClient::Other));
  form->addRow(tr("Assistant:"), m_client);
  m_config = new QLineEdit(codexConfigurationPath());
  m_config->setObjectName(QStringLiteral("assistantConfigPath"));
  auto* configRow = new QHBoxLayout; configRow->addWidget(m_config, 1);
  auto* chooseConfig = new QPushButton(tr("Choose…"));
  chooseConfig->setObjectName(QStringLiteral("assistantChooseConfig")); configRow->addWidget(chooseConfig);
  form->addRow(tr("Codex settings file:"), configRow);
  layout->addLayout(form);
  m_details = new QLabel;
  m_details->setObjectName(QStringLiteral("assistantInstructions"));
  m_details->setWordWrap(true); layout->addWidget(m_details);
  auto* actions = new QHBoxLayout;
  m_setup = new QPushButton(tr("Set up connection"));
  m_setup->setObjectName(QStringLiteral("assistantSetup"));
  actions->addWidget(m_setup);
  auto* install = new QPushButton(tr("Installation instructions"));
  install->setObjectName(QStringLiteral("assistantInstallation")); actions->addWidget(install);
  auto* refresh = new QPushButton(tr("Refresh")); actions->addWidget(refresh);
  actions->addStretch(); layout->addLayout(actions);
  m_state = new QLabel;
  m_state->setTextFormat(Qt::PlainText);
  m_state->setObjectName(QStringLiteral("assistantConnectionState"));
  m_state->setWordWrap(true); layout->addWidget(m_state);
  m_result = new QLabel;
  m_result->setTextFormat(Qt::PlainText);
  m_result->setObjectName(QStringLiteral("assistantSetupResult"));
  m_result->setWordWrap(true); m_result->setTextInteractionFlags(Qt::TextSelectableByMouse);
  layout->addWidget(m_result);
  m_disconnect = new QPushButton(tr("Disconnect Agent"));
  m_disconnect->setObjectName(QStringLiteral("assistantDisconnect")); layout->addWidget(m_disconnect);
  auto* targets = new QLabel(tr("Target: this open design. With several Mitcad windows, the bridge refuses to choose. "
      "Keep only the intended Mitcad window open before connecting, or use the bridge's --list and --instance options "
      "for that launch. Saved setup never pins a temporary instance id."));
  targets->setObjectName(QStringLiteral("assistantTargetHelp")); targets->setWordWrap(true); layout->addWidget(targets);
  auto* folders = new QFormLayout;
  m_folder = new QLineEdit(m_host.suggestedFolder);
  m_folder->setObjectName(QStringLiteral("assistantFolder"));
  auto* folderRow = new QHBoxLayout; folderRow->addWidget(m_folder, 1);
  auto* chooseFolder = new QPushButton(tr("Choose…")); folderRow->addWidget(chooseFolder);
  auto* allow = new QPushButton(tr("Allow folder")); allow->setObjectName(QStringLiteral("assistantAllowFolder"));
  folderRow->addWidget(allow); folders->addRow(tr("Agent file access:"), folderRow);
  layout->addLayout(folders);
  auto* folderHelp = new QLabel(tr("Allow folder explicitly grants Open and Save As access, including subfolders, "
      "and creates the folder if necessary. Without an allowed folder these operations are refused; "
      "saving the open design to its existing file is still available."));
  folderHelp->setWordWrap(true); layout->addWidget(folderHelp);
  auto* fallbackTitle = new QLabel(tr("Manual fallback — configuration and command"));
  layout->addWidget(fallbackTitle);
  m_fallback = new QPlainTextEdit; m_fallback->setReadOnly(true);
  m_fallback->setObjectName(QStringLiteral("assistantFallback")); m_fallback->setMaximumHeight(110);
  layout->addWidget(m_fallback);
  auto* copyRow = new QHBoxLayout;
  auto* copy = new QPushButton(tr("Copy configuration")); copy->setObjectName(QStringLiteral("assistantCopyConfiguration"));
  auto* copyCommand = new QPushButton(tr("Copy command")); copyCommand->setObjectName(QStringLiteral("assistantCopyCommand"));
  copyRow->addWidget(copy); copyRow->addWidget(copyCommand); copyRow->addStretch(); layout->addLayout(copyRow);
  auto* boundary = new QLabel(tr("Local desktop clients only: a browser, mobile app, WSL client, remote executor "
      "or cloud conversation cannot automatically launch this native bridge. No public listener or tunnel is enabled."));
  boundary->setWordWrap(true); layout->addWidget(boundary);
  area->setWidget(page); outer->addWidget(area);
  auto* buttons = new QDialogButtonBox(QDialogButtonBox::Close);
  connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::close); outer->addWidget(buttons);
  connect(m_client, &QComboBox::currentIndexChanged, this, [this] { updateClient(); });
  connect(m_config, &QLineEdit::editingFinished, this, [this] { updateClient(); });
  connect(m_setup, &QPushButton::clicked, this, [this] { setup(); });
  connect(refresh, &QPushButton::clicked, this, [this] { refreshConnection(); });
  connect(m_disconnect, &QPushButton::clicked, this, [this] { if (m_host.disconnect) m_host.disconnect(); refreshConnection(); });
  connect(allow, &QPushButton::clicked, this, [this] { grantFolder(); });
  connect(chooseFolder, &QPushButton::clicked, this, [this] {
    const QString chosen = dialogDirectory(this, tr("Allow Folder for Agents"), m_folder->text());
    if (!chosen.isEmpty()) m_folder->setText(chosen);
  });
  connect(chooseConfig, &QPushButton::clicked, this, [this] {
    const QString chosen = dialogOpenFileName(this, tr("Choose Codex Settings"), m_config->text(), tr("TOML files (*.toml)"));
    if (!chosen.isEmpty()) { m_config->setText(chosen); updateClient(); }
  });
  connect(copy, &QPushButton::clicked, this, [this] {
    const QString configuration = client() == AssistantClient::Codex
        ? QStringLiteral("[mcp_servers.mitcad-desktop]\ncommand = %1\nargs = [\"mcp\", \"--connect\"]\n").arg(jsonString(m_bridge))
        : QString::fromUtf8(QJsonDocument(assistantJsonConfiguration(m_bridge)).toJson(QJsonDocument::Indented));
    QApplication::clipboard()->setText(configuration);
    message(client() == AssistantClient::Codex ? tr("Copied Codex TOML configuration.") : tr("Copied mcpServers JSON configuration."));
  });
  connect(copyCommand, &QPushButton::clicked, this, [this] {
    const AssistantClient selected = client();
    if (selected == AssistantClient::Codex && QFileInfo(m_config->text()).fileName() != QStringLiteral("config.toml")) {
      QApplication::clipboard()->setText(QStringLiteral("[mcp_servers.mitcad-desktop]\ncommand = %1\nargs = [\"mcp\", \"--connect\"]\n").arg(jsonString(m_bridge)));
      message(tr("Copied TOML for the selected settings file. The Codex CLI only registers into CODEX_HOME/config.toml."));
      return;
    }
    const bool registration = selected == AssistantClient::ClaudeCode || selected == AssistantClient::Codex;
    const QString program = registration ? (selected == AssistantClient::ClaudeCode ? QStringLiteral("claude") : QStringLiteral("codex")) : m_bridge;
    const QStringList arguments = registration ? assistantRegistration(selected, m_bridge)
        : QStringList{QStringLiteral("mcp"), QStringLiteral("--connect")};
    QString command = assistantCommand(program, arguments, windows);
    if (selected == AssistantClient::Codex) {
      command = codexHomeCommand(command, m_config->text());
    }
    QApplication::clipboard()->setText(command);
    message(tr("Copied the registration command. Run it on the same computer as Mitcad."));
  });
  connect(install, &QPushButton::clicked, this, [this] {
    const QString link = client() == AssistantClient::ClaudeDesktop ? QStringLiteral("https://claude.ai/download")
        : client() == AssistantClient::ClaudeCode ? QStringLiteral("https://code.claude.com/docs/en/quickstart")
        : QStringLiteral("https://learn.chatgpt.com/docs/extend/mcp");
    QDesktopServices::openUrl(QUrl(link));
  });
  auto* timer = new QTimer(this); timer->setInterval(1000);
  connect(timer, &QTimer::timeout, this, [this] { refreshConnection(); }); timer->start();
  updateClient();
  fitWindowToContents(this);
}

AssistantClient ConnectAssistantDialog::client() const { return AssistantClient(m_client->currentData().toInt()); }

void ConnectAssistantDialog::message(const QString& text, bool error) {
  m_result->setText(text);
  m_result->setProperty("setupError", error);
  qInfo().noquote() << QStringLiteral("Assistant setup: %1").arg(error ? QStringLiteral("error") : QStringLiteral("updated"));
}

void ConnectAssistantDialog::updateClient() {
  m_result->clear();
  const auto selected = client();
  if (auto* install = findChild<QPushButton*>(QStringLiteral("assistantInstallation"))) {
    install->setEnabled(selected != AssistantClient::Other);
  }
  m_config->setEnabled(selected == AssistantClient::Codex);
  if (selected == AssistantClient::ClaudeDesktop) {
    m_details->setText(tr("Set up connection opens a Mitcad MCP Bundle in Claude Desktop. Approve installation "
        "and enable it there, then start a new conversation. If tools are missing, fully quit and reopen Claude Desktop. "
        "Opening the installer does not prove installation or an active connection."));
  } else if (selected == AssistantClient::ClaudeCode) {
    m_details->setText(tr("Set up connection registers the installed bridge in Claude Code's user scope, for all projects. "
        "Claude Code must be installed first. Open a new session after setup and check /mcp. Client trust approvals still apply."));
  } else if (selected == AssistantClient::Codex) {
    m_details->setText(tr("Set up connection uses Codex's shared local MCP settings, with its CLI when available and "
        "a backed-up atomic TOML update otherwise. CODEX_HOME is respected; choose the actual settings file if the client "
        "uses another location. Restart the desktop client or start a new CLI conversation after setup. Managed policies still apply."));
  } else {
    m_details->setText(tr("Copy the mcpServers JSON into your local MCP client's supported setup interface. "
        "The command runs this installed bridge on the same computer. This fallback does not register a client automatically."));
  }
  const QString program = selected == AssistantClient::ClaudeCode ? QStringLiteral("claude") : QStringLiteral("codex");
  const bool registration = selected == AssistantClient::ClaudeCode || selected == AssistantClient::Codex;
  QString fallbackCommand = assistantCommand(registration ? program : m_bridge,
      registration ? assistantRegistration(selected, m_bridge) : QStringList{QStringLiteral("mcp"), QStringLiteral("--connect")}, windows);
  if (selected == AssistantClient::Codex) {
    fallbackCommand = QFileInfo(m_config->text()).fileName() == QStringLiteral("config.toml")
        ? codexHomeCommand(fallbackCommand, m_config->text())
        : tr("Apply the TOML above to the selected settings file: %1").arg(m_config->text());
  }
  m_fallback->setPlainText((selected == AssistantClient::Codex
      ? QStringLiteral("Codex TOML:\n[mcp_servers.mitcad-desktop]\ncommand = %1\nargs = [\"mcp\", \"--connect\"]\n").arg(jsonString(m_bridge))
      : QStringLiteral("mcpServers JSON:\n") + QString::fromUtf8(QJsonDocument(assistantJsonConfiguration(m_bridge)).toJson(QJsonDocument::Indented)))
      + QStringLiteral("\n") + fallbackCommand);
  refreshConnection();
}

QString ConnectAssistantDialog::configuredBridge() const {
  QString text, error;
  if (!readAssistantConfiguration(m_config->text(), text, error)) return {};
  try {
    if (codexPlan(text, m_bridge).value(QStringLiteral("status")).toString() == QStringLiteral("unchanged")) return m_bridge;
  } catch (const std::exception&) { }
  return {};
}

void ConnectAssistantDialog::refreshConnection() {
  const QStringList clients = m_host.clients ? m_host.clients() : QStringList();
  const bool connected = !clients.isEmpty();
  m_disconnect->setEnabled(connected);
  m_client->setEnabled(!m_process);
  m_config->setEnabled(!m_process && client() == AssistantClient::Codex);
  bool configured = client() == AssistantClient::Codex && !m_bridge.isEmpty() && !configuredBridge().isEmpty();
  if (client() == AssistantClient::ClaudeCode) {
    QString text, error;
    if (readAssistantConfiguration(claudeUserConfiguration(), text, error)) {
      const auto entry = QJsonDocument::fromJson(text.toUtf8()).object().value(QStringLiteral("mcpServers")).toObject()
          .value(QStringLiteral("mitcad")).toObject();
      configured = entry.value(QStringLiteral("command")) == m_bridge &&
          entry.value(QStringLiteral("args")) == QJsonArray{QStringLiteral("mcp"), QStringLiteral("--connect")};
    }
  }
  m_state->setText(connected ? tr("Connected: %1").arg(clients.join(QStringLiteral(", ")))
      : configured ? tr("Configured, waiting for connection")
      : client() == AssistantClient::ClaudeDesktop ? tr("Not connected; installation status unknown") : tr("Not configured"));
  m_setup->setEnabled(!m_process && !m_bridge.isEmpty() && client() != AssistantClient::Other);
  for (const char* name : {"assistantCopyConfiguration", "assistantCopyCommand"}) {
    if (auto* copy = findChild<QPushButton*>(QLatin1String(name))) copy->setEnabled(!m_bridge.isEmpty());
  }
#ifndef Q_OS_MACOS
#ifndef Q_OS_WIN
  if (client() == AssistantClient::ClaudeDesktop) m_setup->setEnabled(false);
#endif
#endif
  if (m_bridge.isEmpty() && m_result->text().isEmpty()) message(tr("The installed Mitcad CLI was not found. Repair or reinstall Mitcad."), true);
}

void ConnectAssistantDialog::setup() {
  if (m_process || m_bridge.isEmpty()) return;
  if (client() == AssistantClient::Codex) setupCodex();
  else if (client() == AssistantClient::ClaudeCode) setupClaudeCode();
  else if (client() == AssistantClient::ClaudeDesktop) setupBundle();
}

void ConnectAssistantDialog::setupCodex() {
  const QString path = QFileInfo(m_config->text()).absoluteFilePath();
  if (const QString policy = assistantConfigurationPolicy(path); !policy.isEmpty()) { message(policy, true); return; }
  QString current, error;
  if (!readAssistantConfiguration(path, current, error)) { message(error, true); return; }
  QJsonObject plan;
  try {
    plan = codexPlan(current, m_bridge);
    if (plan.value(QStringLiteral("status")).toString() == QStringLiteral("conflict") && confirmReplacement(this)) {
      plan = codexPlan(current, m_bridge, true);
    }
  } catch (const std::exception& failure) { message(errorText(failure), true); return; }
  const QString status = plan.value(QStringLiteral("status")).toString();
  if (status == QStringLiteral("unchanged")) { message(tr("Already configured. Restart the client or start a new conversation.")); return; }
  if (status != QStringLiteral("updated")) { message(plan.value(QStringLiteral("error")).toString(tr("Setup was not changed.")), true); return; }
  QString cli = assistantExecutable(QStringLiteral("codex"));
  // A selected nonstandard TOML file cannot be addressed by the CLI's home
  // argument. The configuration action remains available without a CLI.
  if (QFileInfo(path).fileName() != QStringLiteral("config.toml")) cli.clear();
  if (!cli.isEmpty()) {
    // Let the supported CLI operate on a private copy. A faulty client or a
    // concurrent edit of the real config cannot overwrite unrelated settings.
    const QString work = !qEnvironmentVariableIsEmpty("MITCAD_TEST_INPUT") ? qEnvironmentVariable("MITCAD_ASSISTANT_TEST_WORK") : QString();
    auto staged = std::make_shared<QTemporaryDir>(QDir(work.isEmpty() ? QDir::tempPath() : work)
        .filePath(QStringLiteral("mitcad-codex-XXXXXX")));
    if (!staged->isValid()) { message(tr("The private setup folder could not be created."), true); return; }
    const QString stagedPath = QDir(staged->path()).filePath(QStringLiteral("config.toml"));
    QSaveFile seed(stagedPath);
    const QByteArray initial = current.toUtf8();
    if (!seed.open(QIODevice::WriteOnly)) { message(tr("The private settings copy could not be created."), true); return; }
    seed.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    if (seed.write(initial) != initial.size() || !seed.commit()) {
      message(tr("The private settings copy could not be written."), true); return;
    }
    auto* process = new QProcess(this);
    auto environment = QProcessEnvironment::systemEnvironment();
    environment.insert(QStringLiteral("CODEX_HOME"), staged->path());
    process->setProcessEnvironment(environment);
    m_process = process;
    runRegistration(cli, assistantRegistration(AssistantClient::Codex, m_bridge), [this, path, current, staged] {
      QString written, error;
      bool valid = readAssistantConfiguration(QDir(staged->path()).filePath(QStringLiteral("config.toml")), written, error);
      if (valid) {
        try {
          const auto verified = parseObject(verify_codex_mcp_config(rustStr(current.toUtf8()), rustStr(written.toUtf8()), rustStr(m_bridge.toUtf8())));
          valid = verified.value(QStringLiteral("valid")).toBool();
          error = verified.value(QStringLiteral("error")).toString();
        } catch (const std::exception& failure) { valid = false; error = errorText(failure); }
      }
      if (valid) {
        QString backup;
        if (!writeAssistantConfiguration(path, current, written, backup, error)) { message(error, true); return; }
        message(tr("Configured, waiting for connection. Restart the desktop client or start a new conversation.") +
            (backup.isEmpty() ? QString() : tr(" Backup: %1").arg(QDir::toNativeSeparators(backup))));
      } else {
        message(tr("The client configuration could not be verified: %1. The original configuration was kept.")
            .arg(error), true);
      }
    });
    return;
  }
  QString backup;
  if (!writeAssistantConfiguration(path, current, plan.value(QStringLiteral("content")).toString(), backup, error)) {
    message(error, true); return;
  }
  message(tr("Configured, waiting for connection. Restart the desktop client or start a new conversation.") +
      (backup.isEmpty() ? QString() : tr(" Backup: %1").arg(QDir::toNativeSeparators(backup))));
  refreshConnection();
}

void ConnectAssistantDialog::setupClaudeCode() {
  const QString cli = assistantExecutable(QStringLiteral("claude"));
  if (cli.isEmpty()) { message(tr("Claude Code is not installed or its executable was not found. Open Installation instructions, then try again."), true); return; }
  const QString path = claudeUserConfiguration();
  QString current, error;
  if (!readAssistantConfiguration(path, current, error)) { message(error, true); return; }
  QJsonParseError parse;
  const auto document = QJsonDocument::fromJson(current.toUtf8(), &parse);
  if (!current.isEmpty() && (parse.error != QJsonParseError::NoError || !document.isObject())) {
    message(tr("The existing Claude Code configuration is not valid JSON."), true); return;
  }
  const QJsonObject original = document.isObject() ? document.object() : QJsonObject{};
  const auto existing = original.value(QStringLiteral("mcpServers"));
  if (!existing.isUndefined() && !existing.isObject()) {
    message(tr("Claude Code's mcpServers configuration must be a JSON object."), true); return;
  }
  QJsonObject servers = existing.toObject();
  const auto entry = servers.value(QStringLiteral("mitcad"));
  if (!entry.isUndefined()) {
    if (entry.toObject().value(QStringLiteral("command")) == m_bridge &&
        entry.toObject().value(QStringLiteral("args")) == QJsonArray{QStringLiteral("mcp"), QStringLiteral("--connect")}) {
      message(tr("Already configured in user scope. Start a new Claude Code session.")); return;
    }
    if (!confirmReplacement(this)) return;
  }
  // CLAUDE_CONFIG_DIR places user-scope .claude.json inside this private
  // directory. Run the supported CLI there, never remove the real entry.
  const QString work = !qEnvironmentVariableIsEmpty("MITCAD_TEST_INPUT") ? qEnvironmentVariable("MITCAD_ASSISTANT_TEST_WORK") : QString();
  auto staged = std::make_shared<QTemporaryDir>(QDir(work.isEmpty() ? QDir::tempPath() : work)
      .filePath(QStringLiteral("mitcad-claude-XXXXXX")));
  if (!staged->isValid()) { message(tr("The private setup folder could not be created."), true); return; }
  QJsonObject seed = original;
  servers.remove(QStringLiteral("mitcad"));
  seed.insert(QStringLiteral("mcpServers"), servers);
  QSaveFile file(QDir(staged->path()).filePath(QStringLiteral(".claude.json")));
  const QByteArray initial = QJsonDocument(seed).toJson();
  if (!file.open(QIODevice::WriteOnly)) { message(tr("The private settings copy could not be created."), true); return; }
  file.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
  if (file.write(initial) != initial.size() || !file.commit()) {
    message(tr("The private settings copy could not be written."), true); return;
  }
  auto* process = new QProcess(this);
  auto environment = QProcessEnvironment::systemEnvironment();
  environment.insert(QStringLiteral("CLAUDE_CONFIG_DIR"), staged->path());
  process->setProcessEnvironment(environment);
  m_process = process;
  runRegistration(cli, assistantRegistration(AssistantClient::ClaudeCode, m_bridge), [this, path, current, seed, staged] {
    QString written, error;
    if (!readAssistantConfiguration(QDir(staged->path()).filePath(QStringLiteral(".claude.json")), written, error)) {
      message(error, true); return;
    }
    QJsonParseError parse;
    const auto document = QJsonDocument::fromJson(written.toUtf8(), &parse);
    QJsonObject result = document.object();
    QJsonObject servers = result.value(QStringLiteral("mcpServers")).toObject();
    const auto entry = servers.take(QStringLiteral("mitcad")).toObject();
    result.insert(QStringLiteral("mcpServers"), servers);
    if (parse.error != QJsonParseError::NoError || !document.isObject() || result != seed ||
        entry.value(QStringLiteral("command")) != m_bridge ||
        entry.value(QStringLiteral("args")) != QJsonArray{QStringLiteral("mcp"), QStringLiteral("--connect")} ||
        (entry.contains(QStringLiteral("type")) && entry.value(QStringLiteral("type")) != QStringLiteral("stdio"))) {
      message(tr("Claude Code registration could not be verified. The original configuration was kept."), true); return;
    }
    QString backup;
    if (!writeAssistantConfiguration(path, current, written, backup, error)) { message(error, true); return; }
    message(tr("Configured in user scope, waiting for connection. Start a new Claude Code session and check /mcp.") +
        (backup.isEmpty() ? QString() : tr(" Backup: %1").arg(QDir::toNativeSeparators(backup))));
  });
}

void ConnectAssistantDialog::runRegistration(const QString& program, const QStringList& arguments,
                                             std::function<void()> success) {
  QProcess* process = m_process ? m_process : new QProcess(this);
  m_process = process;
  m_setup->setEnabled(false);
  refreshConnection();
  process->setWorkingDirectory(QDir::homePath());
  auto* timeout = new QTimer(process); timeout->setSingleShot(true); timeout->setInterval(30000);
  connect(timeout, &QTimer::timeout, process, [process] { process->kill(); });
  connect(process, &QProcess::errorOccurred, this, [this, process](QProcess::ProcessError error) {
    if (error != QProcess::FailedToStart) return;
    message(tr("The assistant registration command could not start."), true);
    m_process = nullptr; process->deleteLater(); refreshConnection();
  });
  connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), this,
      [this, process, timeout, success = std::move(success)](int code, QProcess::ExitStatus exit) {
        timeout->stop();
        m_process = nullptr;
        if (code == 0 && exit == QProcess::NormalExit) success();
        else message(tr("The assistant registration command failed. %1")
            .arg(QString::fromUtf8(process->readAllStandardError()).left(2000)), true);
        process->deleteLater(); refreshConnection();
      });
  process->start(program, arguments, QIODevice::ReadOnly); timeout->start();
}

void ConnectAssistantDialog::setupBundle() {
  const QString suffix = windows ? QStringLiteral(".exe") : QString();
  const QString launcher = QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("mitcad-mcp-launcher") + suffix);
  if (!QFileInfo(launcher).isExecutable()) { message(tr("The installed MCP launcher is missing. Repair or reinstall Mitcad."), true); return; }
  const QString platform = windows ? QStringLiteral("win32") : QStringLiteral("darwin");
  const QByteArray manifest = QJsonDocument(assistantBundleManifest(m_bridge, windows, platform)).toJson(QJsonDocument::Indented);
  try {
    const auto bytes = build_mcp_bundle(rustStr(manifest), rustStr(launcher.toUtf8()));
    const QString work = !qEnvironmentVariableIsEmpty("MITCAD_TEST_INPUT") ? qEnvironmentVariable("MITCAD_ASSISTANT_TEST_WORK") : QString();
    const QString folder = QDir(work.isEmpty() ? QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation) : work)
        .filePath(QStringLiteral("assistant-setup"));
    if (!QDir().mkpath(folder)) { message(tr("The bundle folder could not be created."), true); return; }
    const QString path = QDir(folder).filePath(QStringLiteral("mitcad-desktop.mcpb"));
    QSaveFile file(path);
    if (!file.open(QIODevice::WriteOnly) ||
        file.write(reinterpret_cast<const char*>(bytes.data()), qsizetype(bytes.size())) != qsizetype(bytes.size()) || !file.commit()) {
      message(tr("The MCP Bundle could not be saved."), true); return;
    }
    if (!work.isEmpty() && qEnvironmentVariable("MITCAD_TEST_ASSISTANT_BUNDLE_OPEN") == QStringLiteral("capture")) {
      setProperty("assistantBundlePath", path);
      message(tr("Bundle prepared for the isolated installer test.")); return;
    }
    if (!QDesktopServices::openUrl(QUrl::fromLocalFile(path))) {
      message(tr("No MCP Bundle installer opened. Install Claude Desktop, then open this bundle: %1").arg(QDir::toNativeSeparators(path)), true); return;
    }
    message(tr("Installation opened in Claude Desktop. Approve and enable the bundle, then start a new conversation. "
               "If necessary fully quit and reopen Claude Desktop. Installation status remains unknown until the client connects."));
  } catch (const std::exception& failure) { message(errorText(failure), true); }
}

void ConnectAssistantDialog::grantFolder() {
  const QFileInfo folder(m_folder->text().trimmed());
  if (!folder.isAbsolute() || !QDir().mkpath(folder.absoluteFilePath()) || !QFileInfo(folder.absoluteFilePath()).isDir()) {
    message(tr("Choose an absolute folder that can be created and accessed."), true); return;
  }
  QStringList folders = savedAgentFolders(); folders << folder.canonicalFilePath();
  saveAgentFolders(folders);
  for (QWidget* window : QApplication::topLevelWidgets()) {
    for (auto* preferences : window->findChildren<AgentPreferencesBox*>()) preferences->includeSavedFolders();
  }
  message(tr("Allowed folder: %1. Open and Save As can use it and its subfolders.").arg(QDir::toNativeSeparators(folder.canonicalFilePath())));
}

} // namespace mitcad::mcp
