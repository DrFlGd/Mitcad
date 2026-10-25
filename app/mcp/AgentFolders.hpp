// SPDX-License-Identifier: MIT
#pragma once

// The folders an agent connected through live MCP may save designs to and
// open them from (mitcad#402): live mode's equivalent of headless mode's
// --workspace. None by default: the user adds them in Preferences > Agents
// (the setting mcp/folders), or for one run with --mcp-folder DIR. A path
// is inside one when its folder, with symbolic links resolved, is that
// folder or below it; '..' is refused, and a relative path is relative to
// the first folder.

#include <QGroupBox>
#include <QString>
#include <QStringList>
#include <QWidget>

class QLineEdit;
class QListWidget;
class QPushButton;

namespace mitcad::mcp {

// The setting mcp/folders: absolute, cleaned, without duplicates.
QStringList savedAgentFolders();
void saveAgentFolders(const QStringList& folders);
// Folders allowed for this run only (--mcp-folder), besides the setting.
void setRunAgentFolders(const QStringList& folders);
// Every allowed folder: the setting's, then this run's.
QStringList agentFolders();

// A design file (.mitcad) in an allowed folder, as an absolute path with
// its folder's links resolved; empty with the reason when it is not one.
// `forWriting`: the folder must exist, an existing file must be a regular
// file (not a link out of the folder) and needs `overwrite`; otherwise the
// file must exist.
QString agentDesignPath(const QString& given, bool forWriting, bool overwrite, QString& reason);
// A file an agent writes in an allowed folder (mitcad#689: model_export,
// ui_screenshot's path), by the rules of agentDesignPath's `forWriting`:
// a new file, or with `overwrite` an existing regular one; with one of
// `suffixes` (any when empty). Empty with the reason when it is not one.
QString agentOutputPath(const QString& given, bool overwrite, QString& reason, const QStringList& suffixes = {});

// A group with a frame and a title, except on macOS, where the settings
// window's panes have none: a plain widget.
#ifdef Q_OS_MACOS
using AgentPreferencesBase = QWidget;
#else
using AgentPreferencesBase = QGroupBox;
#endif

// The Agents group of Preferences: the list of allowed folders.
class AgentPreferencesBox : public AgentPreferencesBase {
  Q_OBJECT

public:
  explicit AgentPreferencesBox(QWidget* parent = nullptr);

  // The folders as the box shows them (with one typed and not added yet),
  // to save when Preferences is accepted.
  QStringList chosen() const;
  void includeSavedFolders();

signals:
  void changed();
  void connectRequested();

private:
  void addFolder(const QString& folder);

  QListWidget* m_folders = nullptr;
  QLineEdit* m_typed = nullptr;
  QPushButton* m_remove = nullptr;
};

} // namespace mitcad::mcp
