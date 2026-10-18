// SPDX-License-Identifier: MIT
#pragma once

#include <QString>
#include <QStringList>

namespace mitcad {

// Starting Mitcad again (mitcad#419: a new window layout takes effect at
// the next start). main() gives the arguments a restart keeps: the run's
// settings (--ui, --no-native-dialogs, --no-recovery, --no-glass, --no-mcp,
// --mcp-folder, a plain --debug-log), not its one-off start actions (--open,
// the file argument, --demo, --set, --screenshot) nor --chrome, so that the
// layout preference decides.
void setRestartArguments(const QStringList& arguments);

// Starts Mitcad again once this process has quit, from QCoreApplication's
// destructor as an installed update starts (the window, its document's
// locks and the local MCP connection are gone by then), with the kept
// arguments and `extra` (the design to open). The program is the AppImage
// itself when it runs from one. MITCAD_TEST_NO_RESTART=1 (UI tests) logs
// the command instead of starting it. The returned text is the command, for
// the log.
QString scheduleRestart(const QStringList& extra);
// The window stayed open after all (its close was cancelled).
void cancelRestart();

} // namespace mitcad
