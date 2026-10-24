// SPDX-License-Identifier: MIT
#pragma once

#include <QObject>
#include <QString>

namespace mitcad::render {

// An actual process crash takes precedence over an earlier recoverable
// command error. A protocol mismatch is different: the client itself
// killed that worker and should retain the reason it did so.
inline QString workerExitReason(const QString& error, int exitCode, bool crashed, bool incompatible) {
  const QString exit = crashed ? QObject::tr("the render process crashed (exit code %1)").arg(exitCode)
                               : QObject::tr("the render process ended (exit code %1)").arg(exitCode);
  if (!error.isEmpty() && (!crashed || incompatible)) {
    return QObject::tr("%1; %2").arg(error, exit);
  }
  return exit;
}

} // namespace mitcad::render
