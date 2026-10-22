// SPDX-License-Identifier: MIT
#pragma once

#include <QAbstractNativeEventFilter>
#include <QElapsedTimer>
#include <QObject>
#include <QTimer>

#include <utility>

namespace mitcad {

// The UI tests' sync (tools/ui-test-lib.sh, ui_sync), with MITCAD_TEST_SYNC=1:
// instead of pausing for a fixed time after each step, a test presses the
// Pause key (through XTest, after the input before it) and waits for
// "Sync <n>" in the log. The key never reaches Qt's widgets (an X event
// filter takes it, also while a modal dialog blocks the window that has
// the keyboard), and its line is logged once the input before it is
// handled (X events are handled in order), at least 20 ms later (what
// that input set going at once, such as a frame, has run), and when no
// watched timer runs: the timers after which the application logs what a
// test reads next (a panel's layout, the camera at rest, a preview). A
// long job does not hold it up: while a job keeps the input waiting, the
// key waits too, and once the progress dialog shows, ui_wait_idle waits
// for the job.
//
// Linux (X11), and Windows, where the Windows UI test library posts the
// Pause key to a window (tools/ui-windows-lib.ps1, Ui-Sync; Qt's event loop
// keeps it with the other input while a job waits, mitcad#271); elsewhere,
// and without the variable, nothing is installed and watching costs
// nothing.
class TestSync : public QObject, public QAbstractNativeEventFilter {
public:
  // Installs the sync on the application when MITCAD_TEST_SYNC is set.
  static void installIfRequested();

  // A timer whose timeout logs something UI tests read next: the sync waits
  // while it runs. Also kept while the in-process test driver runs
  // (MITCAD_TEST_INPUT, macOS's ui-macos tests), for `settling`.
  static void watch(QTimer* timer);
  // Whether a watched timer runs now: what it logs (a panel's places that
  // moved, mitcad#528) is not logged yet. The in-process test driver waits
  // for it before it clicks a logged place, as the Linux and Windows steps
  // wait for the sync.
  static bool settling();

  // As QTimer::singleShot(milliseconds, context, function), watched.
  template <typename Function>
  static void singleShot(int milliseconds, QObject* context, Function function) {
    auto* timer = new QTimer(context);
    timer->setSingleShot(true);
    QObject::connect(timer, &QTimer::timeout, context, [timer, function = std::move(function)] {
      timer->deleteLater();
      function();
    });
    watch(timer);
    timer->start(milliseconds);
  }

  bool nativeEventFilter(const QByteArray& eventType, void* message, qintptr* result) override;

private:
  explicit TestSync(unsigned keycode);
  void request();
  void poll();

  [[maybe_unused]] unsigned m_keycode; // read by the X11 key synthesis only
  int m_count = 0;
  int m_answered = 0;
  QElapsedTimer m_since; // since the oldest request not answered
  QElapsedTimer m_last;  // since the latest request
  QTimer m_poll;
};

} // namespace mitcad
