// SPDX-License-Identifier: MIT
#pragma once

#include <QJsonObject>
#include <QString>
#include <QStringList>

namespace mitcad::mcp {

enum class AssistantClient { ClaudeDesktop, ClaudeCode, Codex, Other };

QString installedBridge(const QString& applicationDirectory);
QString assistantExecutable(const QString& name, const QString& home = {});
QString codexConfigurationPath();
// One word of a command for PowerShell (`windows`) or a POSIX shell, quoted
// only when it needs it (or `always`).
QString assistantShellWord(const QString& value, bool windows, bool always = false);
// A command line: the program (with the platform's separators) and its
// arguments, each quoted only when it needs it; PowerShell's call operator
// only before a quoted program.
QString assistantCommand(const QString& program, const QStringList& arguments, bool windows);
QStringList assistantRegistration(AssistantClient client, const QString& bridge);
QJsonObject assistantJsonConfiguration(const QString& bridge);
QJsonObject assistantBundleManifest(const QString& bridge, bool windows, const QString& platform);
bool readAssistantConfiguration(const QString& path, QString& content, QString& error);
// Compare the file again before replacing it; a symlink or concurrent edit
// never gets overwritten. Existing bytes are backed up with a unique name.
bool writeAssistantConfiguration(const QString& path, const QString& expected, const QString& content,
                                 QString& backup, QString& error);
// Managed settings are never changed by this user-level setup flow.
QString assistantConfigurationPolicy(const QString& path);
// Whether a connected client's display label (its clientInfo.name) is this
// assistant, so that the dialog shows Connected for the right choice. Labels
// are supplied by the clients: this picks a state to show, never a right.
bool assistantLabelMatches(AssistantClient client, const QString& label);
// The assistant's name on its choice ("Claude Desktop", "ChatGPT / Codex").
QString assistantName(AssistantClient client);
// The assistant's name in a sentence ("Ready to connect Codex"): its
// choice's name, but "Codex" for "ChatGPT / Codex".
QString assistantSentenceName(AssistantClient client);
// A connected client's name for people: the assistant its label matches, in
// a sentence's form ("Claude Desktop" for "claude-ai"), else the label as the
// client gave it.
QString assistantDisplayName(const QString& label);

} // namespace mitcad::mcp
