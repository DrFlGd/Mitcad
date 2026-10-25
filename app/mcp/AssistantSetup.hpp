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

} // namespace mitcad::mcp
