// SPDX-License-Identifier: MIT
#pragma once

#include <algorithm>
#include <functional>

#include <QString>
#include <QThread>

namespace mitcad {

using WorkerThreadStarter = std::function<void(QThread&)>;

// QThread marks a successfully created native thread Running before
// start() returns. Do not wait for its queued started signal: the UI
// event loop need not run before its first job can be posted.
inline bool startWorkerThread(QThread& thread, QString& error, const WorkerThreadStarter& starter = {}) {
  error.clear();
  if (thread.isRunning()) {
    return true;
  }
  constexpr unsigned minimum = 8u << 20;
  unsigned stack = thread.stackSize();
  if (stack == 0) {
    stack = 256u << 20;
  }
  const unsigned preferred = stack;
  for (;;) {
    thread.setStackSize(stack);
    if (starter) {
      starter(thread);
    } else {
      thread.start();
    }
    if (thread.isRunning()) {
      return true;
    }
    if (stack <= minimum) {
      break;
    }
    stack = std::max(minimum, stack / 2);
  }
  thread.setStackSize(preferred);
  error = QThread::tr("The %1 thread could not start, including with an 8 MiB stack; no computation was queued.")
              .arg(thread.objectName().isEmpty() ? QThread::tr("worker") : thread.objectName());
  return false;
}

} // namespace mitcad
