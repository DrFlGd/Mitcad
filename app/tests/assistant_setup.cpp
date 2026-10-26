// SPDX-License-Identifier: MIT
#include "mcp/AssistantSetup.hpp"

#include <cstdio>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QTemporaryDir>

namespace {
int failures = 0;
void check(bool value, const char* expression, int line) {
  if (!value) { std::fprintf(stderr, "assistant_setup.cpp:%d: %s\n", line, expression); ++failures; }
}
#define CHECK(value) check((value), #value, __LINE__)
bool put(const QString& path, const QByteArray& bytes) {
  QFile file(path);
  return file.open(QIODevice::WriteOnly) && file.write(bytes) == bytes.size();
}
}

int assistantSetupTests() {
  using namespace mitcad::mcp;
  const QString bridge = QStringLiteral("/Applications/Example's app/mitcad-cli");
  const auto codex = assistantRegistration(AssistantClient::Codex, bridge);
  CHECK(codex == QStringList({QStringLiteral("mcp"), QStringLiteral("add"), QStringLiteral("mitcad-desktop"),
        QStringLiteral("--"), bridge, QStringLiteral("mcp"), QStringLiteral("--connect")}));
  const auto claude = assistantRegistration(AssistantClient::ClaudeCode, bridge);
  CHECK(claude.mid(2, 2) == QStringList({QStringLiteral("--scope"), QStringLiteral("user")}));
  CHECK(claude.at(6) == bridge);
  CHECK(assistantCommand(QStringLiteral("codex"), codex, false).contains(QStringLiteral("Example'\\''s app")));
  // Only the words that need it are quoted; PowerShell's `--` always is, and
  // its call operator comes only before a quoted program.
  CHECK(assistantCommand(QStringLiteral("codex"), codex, true).startsWith(QStringLiteral("codex mcp add mitcad-desktop '--' '")));
  CHECK(assistantCommand(QStringLiteral("codex"), codex, true).contains(QStringLiteral("Example''s app")));
  CHECK(assistantCommand(QStringLiteral("codex"), codex, false).startsWith(QStringLiteral("codex mcp add mitcad-desktop -- '")));
  // A program without a folder, as the native separators differ by platform.
  CHECK(assistantCommand(QStringLiteral("mitcad-cli"), {QStringLiteral("mcp"), QStringLiteral("--connect")}, false) ==
        QStringLiteral("mitcad-cli mcp --connect"));
  CHECK(assistantCommand(QStringLiteral("/opt/My Apps/mitcad-cli"), {QStringLiteral("mcp")}, true).startsWith(QStringLiteral("& '")));
  CHECK(assistantShellWord(QStringLiteral("/home/user/.codex"), true, true) == QStringLiteral("'/home/user/.codex'"));
  // The connected clients' names for people (mitcad#706, mitcad#710).
  CHECK(assistantDisplayName(QStringLiteral("claude-ai")) == QStringLiteral("Claude Desktop"));
  CHECK(assistantDisplayName(QStringLiteral("claude-code")) == QStringLiteral("Claude Code"));
  // "ChatGPT / Codex" names its choice only; a sentence says "Codex" (mitcad#724).
  CHECK(assistantName(AssistantClient::Codex) == QStringLiteral("ChatGPT / Codex"));
  CHECK(assistantSentenceName(AssistantClient::Codex) == QStringLiteral("Codex"));
  CHECK(assistantSentenceName(AssistantClient::ClaudeCode) == QStringLiteral("Claude Code"));
  CHECK(assistantDisplayName(QStringLiteral("codex-mcp-client")) == QStringLiteral("Codex"));
  CHECK(assistantDisplayName(QStringLiteral("example-client")) == QStringLiteral("example-client"));
  const auto json = assistantJsonConfiguration(bridge).value(QStringLiteral("mcpServers")).toObject()
      .value(QStringLiteral("mitcad")).toObject();
  // Connected shows for the assistant whose label a client gave (mitcad#706).
  CHECK(assistantLabelMatches(AssistantClient::ClaudeDesktop, QStringLiteral("claude-ai")));
  CHECK(!assistantLabelMatches(AssistantClient::ClaudeCode, QStringLiteral("claude-ai")));
  CHECK(assistantLabelMatches(AssistantClient::ClaudeCode, QStringLiteral("claude-code")));
  CHECK(!assistantLabelMatches(AssistantClient::ClaudeDesktop, QStringLiteral("claude-code")));
  CHECK(assistantLabelMatches(AssistantClient::Codex, QStringLiteral("codex-mcp-client")));
  CHECK(!assistantLabelMatches(AssistantClient::Other, QStringLiteral("Codex")));
  CHECK(assistantLabelMatches(AssistantClient::Other, QStringLiteral("example-client")));
  CHECK(!assistantLabelMatches(AssistantClient::Codex, QStringLiteral("example-client")));
  CHECK(json.value(QStringLiteral("command")) == bridge);
  CHECK(json.value(QStringLiteral("args")) == QJsonArray({QStringLiteral("mcp"), QStringLiteral("--connect")}));
  for (bool win : {false, true}) {
    const auto manifest = assistantBundleManifest(bridge, win, win ? QStringLiteral("win32") : QStringLiteral("darwin"));
    const auto server = manifest.value(QStringLiteral("server")).toObject();
    CHECK(manifest.value(QStringLiteral("manifest_version")) == QStringLiteral("0.3"));
    CHECK(server.value(QStringLiteral("type")) == QStringLiteral("binary"));
    CHECK(server.value(QStringLiteral("entry_point")).toString().endsWith(win ? QStringLiteral(".exe") : QStringLiteral("launcher")));
    CHECK(server.value(QStringLiteral("mcp_config")).toObject().value(QStringLiteral("args")).toArray().first() == bridge);
  }

  QTemporaryDir directory;
  CHECK(directory.isValid());
  const QString path = directory.filePath(QStringLiteral("config.toml"));
  const QString initial = QStringLiteral("# Retain this comment\nmodel = 'example'\n");
  const QString updated = initial + QStringLiteral("\n[mcp_servers.mitcad-desktop]\ncommand = 'example'\n");
  QString content, backup, error;
  CHECK(readAssistantConfiguration(path, content, error));
  CHECK(content.isEmpty());
  CHECK(put(path, initial.toUtf8()));
  CHECK(writeAssistantConfiguration(path, initial, updated, backup, error));
  CHECK(!backup.isEmpty() && backup != path);
  QFile previous(backup);
  CHECK(previous.open(QIODevice::ReadOnly));
  CHECK(previous.readAll() == initial.toUtf8());
  CHECK(readAssistantConfiguration(path, content, error) && content == updated);
  const auto files = QDir(directory.path()).entryList(QDir::Files);
  CHECK(writeAssistantConfiguration(path, updated, updated, backup, error));
  CHECK(backup.isEmpty());
  CHECK(QDir(directory.path()).entryList(QDir::Files) == files);
  CHECK(!writeAssistantConfiguration(path, initial, QStringLiteral("wrong"), backup, error));
  CHECK(error.contains(QStringLiteral("changed")));
  CHECK(readAssistantConfiguration(path, content, error) && content == updated);
  CHECK(!readAssistantConfiguration(QStringLiteral("relative/config.toml"), content, error));
  CHECK(put(directory.filePath(QStringLiteral("invalid.toml")), QByteArray("\xff", 1)));
  CHECK(!readAssistantConfiguration(directory.filePath(QStringLiteral("invalid.toml")), content, error));
  CHECK(put(directory.filePath(QStringLiteral("requirements.toml")), QByteArrayLiteral("# Managed\n")));
  CHECK(!assistantConfigurationPolicy(path).isEmpty());
#ifndef Q_OS_WIN
  const QString linked = directory.filePath(QStringLiteral("linked.toml"));
  CHECK(QFile::link(path, linked));
  CHECK(!writeAssistantConfiguration(linked, updated, QStringLiteral("wrong"), backup, error));
#endif
  return failures;
}
