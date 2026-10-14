// SPDX-License-Identifier: MIT
#pragma once

// The wait cursor while the window waits for a job (mitcad#268,
// MainWindow::waitForJob):
//
// - A guard that restores exactly what it set: once, when the progress
//   dialog takes over (release()), else when it goes out of scope, also
//   when the wait ends early (an exception) and when waits nest. Every
//   guard pushes the same cursor, so the one it pops is its own in effect.
// - Log lines that pair up (they reach the debug log, mitcad#269):
//     Wait cursor on: <job> (depth N)
//     Wait cursor off: <job> (depth N)
//   N counts the guards that hold a cursor, this one included.
// - Idle recovery: when the application is idle (the caller knows no job
//   runs; no mouse button is held, as in a drag; no modal dialog or popup
//   is open) and an override cursor is still set that no guard holds, it
//   is cleared, with the warning
//     Wait cursor left on after <job>: cleared
//   (<job>: the latest wait's). The first idle check after a wait logs
//     Wait cursor at idle: none set
// - For tests (MITCAD_TEST_WAIT_CURSOR=1): an override cursor set and left
//   as a stray one would be, "Wait cursor left on for a test: <job>".

#include <QString>

namespace mitcad {

class WaitCursor {
public:
  explicit WaitCursor(const QString& label);
  ~WaitCursor();
  WaitCursor(const WaitCursor&) = delete;
  WaitCursor& operator=(const WaitCursor&) = delete;

  // The progress dialog takes over: the cursor goes now (once).
  void release();
  bool holds() const { return m_held; }

  // The guards holding a cursor now.
  static int depth();
  // The idle check, on the GUI thread when no job runs: with no guard
  // holding a cursor, clears every override cursor left (and logs it);
  // a held mouse button, a modal dialog or a popup leave it to the next
  // check. Returns how many it cleared.
  static int recoverIfIdle();
  // Whether MITCAD_TEST_WAIT_CURSOR asks for the tests' hooks.
  static bool testHooks();
  // For tests: an override cursor set and left, named `label`.
  static void leaveOnForTest(const QString& label);

private:
  QString m_label;
  int m_depth = 0;
  bool m_held = false;
};

} // namespace mitcad
