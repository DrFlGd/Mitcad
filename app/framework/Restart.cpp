// SPDX-License-Identifier: MIT
#include "Restart.hpp"

#include <optional>

#include <QCoreApplication>
#include <QDir>
#include <QProcess>
#include <QProcessEnvironment>
#include <QtGlobal>
#include <QtLogging>

namespace mitcad {
namespace {

struct Pending {
  QString program;
  QStringList arguments;
  QString workingDirectory;
};

QStringList& keptArguments() {
  static QStringList arguments;
  return arguments;
}

std::optional<Pending>& pending() {
  static std::optional<Pending> restart;
  return restart;
}

QString describe(const Pending& restart) {
  QStringList parts{QDir::toNativeSeparators(restart.program)};
  for (const QString& argument : restart.arguments) {
    parts << (argument.contains(QLatin1Char(' ')) ? QStringLiteral("\"%1\"").arg(argument) : argument);
  }
  return parts.join(QLatin1Char(' '));
}

// From QCoreApplication's destructor (qAddPostRoutine).
void launchPending() {
  const std::optional<Pending> restart = pending();
  pending().reset();
  if (!restart) {
    return;
  }
  QProcess process;
  process.setProgram(restart->program);
  process.setArguments(restart->arguments);
  process.setWorkingDirectory(restart->workingDirectory);
  // The AppImage's runtime sets its variables for the new process again; a
  // debug log of this run's variable would be overwritten (the restart
  // passes --debug-log instead).
  QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
  for (const char* variable : {"APPIMAGE", "APPDIR", "ARGV0", "OWD", "MITCAD_DEBUG_LOG"}) {
    environment.remove(QString::fromLatin1(variable));
  }
  process.setProcessEnvironment(environment);
  qint64 pid = 0;
  if (process.startDetached(&pid)) {
    qInfo().noquote() << QStringLiteral("Restart: started %1 (process %2)").arg(restart->program).arg(pid);
  } else {
    qWarning().noquote() << "Restart: cannot start" << restart->program << "-" << process.errorString();
  }
}

} // namespace

void setRestartArguments(const QStringList& arguments) { keptArguments() = arguments; }

QString scheduleRestart(const QStringList& extra) {
  Pending restart;
  // Inside an AppImage the program is in its mount, which goes with this
  // process: the AppImage file starts it again.
  const QString appImage = qEnvironmentVariable("APPIMAGE");
  restart.program = appImage.isEmpty() ? QCoreApplication::applicationFilePath() : appImage;
  restart.arguments = keptArguments() + extra;
  restart.workingDirectory = QDir::currentPath();
  const QString command = describe(restart);
  if (qEnvironmentVariable("MITCAD_TEST_NO_RESTART") == QLatin1String("1")) {
    qInfo().noquote() << "Restart (test, not started):" << command;
    pending().reset();
    return command;
  }
  static bool routineAdded = false;
  if (!routineAdded) {
    qAddPostRoutine(launchPending);
    routineAdded = true;
  }
  pending() = restart;
  return command;
}

void cancelRestart() { pending().reset(); }

} // namespace mitcad
