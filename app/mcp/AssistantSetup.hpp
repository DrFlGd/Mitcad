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
// The largest settings file read: Codex's TOML (and a file chosen under
// Advanced) is small; Claude Code keeps its projects' state and history in
// its .claude.json, which grows to tens of megabytes (mitcad#760), and only
// its parse and the compare-before-write need the content.
constexpr qint64 kAssistantConfigurationLimit = 4LL * 1024 * 1024;
constexpr qint64 kClaudeConfigurationLimit = 256LL * 1024 * 1024;
bool readAssistantConfiguration(const QString& path, QString& content, QString& error,
                                qint64 limit = kAssistantConfigurationLimit);
// Compare the file again before replacing it; a symlink or concurrent edit
// never gets overwritten. Existing bytes are backed up with a unique name.
bool writeAssistantConfiguration(const QString& path, const QString& expected, const QString& content,
                                 QString& backup, QString& error, qint64 limit = kAssistantConfigurationLimit);
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

// Claude Desktop installed from the Microsoft Store, an MSIX package
// (mitcad#763): its package family, and whether the package's folder is under
// Packages in this local application data folder (LOCALAPPDATA): only this
// family's, not another `Claude_*` one. The application asks the package
// manager; a test run takes the folder for the package. Such a package
// registers no `.mcpb` file association: that no application opens the
// extension then does not mean that Claude Desktop is missing.
inline constexpr char kClaudeDesktopPackageFamily[] = "Claude_pzs8sxrjxfjjc";
bool claudeDesktopPackaged(const QString& localAppData);
// Claude Desktop's application on Windows by the local application data
// folder alone: the installer's application folder or the package's folder
// above, never the settings folder, which an uninstalled Claude Desktop
// leaves behind.
bool claudeDesktopApplicationInWindowsFolders(const QString& localAppData);
// Claude Desktop on Windows by the user's application data folders
// (LOCALAPPDATA, APPDATA): the installer's application folder, the package
// above, or the settings folder Claude Desktop makes on its first start.
bool claudeDesktopInWindowsFolders(const QString& localAppData, const QString& roamingAppData);

} // namespace mitcad::mcp
