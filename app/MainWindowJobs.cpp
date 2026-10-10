// SPDX-License-Identifier: MIT
// Background computation (P7, docs/architecture.md): the main window
// presents progress while DocumentSession runs jobs on its model worker.
// The legacy Qt callers stay synchronous; the native frontend can also
// use the session's asynchronous command and preview entry points.
#include "MainWindow.hpp"

#include <algorithm>
#include <chrono>
#include <memory>

#include <QApplication>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QPointer>
#include <QTimer>
#include <QtLogging>

#include "framework/ComputeProgress.hpp"
#include "framework/Dialogs.hpp"
#include "files/LockController.hpp"
#ifdef MITCAD_MACOS_NATIVE_UI
#include "platform/NativeDialogs.hpp"
#endif

namespace mitcad {
namespace {

using std::chrono::milliseconds;

// Until then it draws while input waits in Qt's queue; a job that takes
// longer gets the progress dialog, and input goes to that.
constexpr qint64 kDialogAfterMs = 400;
// How often the progress is read meanwhile, and logged at most.
constexpr int kTickMs = 100;
constexpr qint64 kLogEveryMs = 200;

QString featureOf(const JobProgress& progress) {
  return QString::fromUtf8(progress.feature.data(), static_cast<qsizetype>(progress.feature.size()));
}

// "57 of 196, Extrude6": the feature being computed.
QString positionOf(const JobProgress& progress) {
  const auto shown = static_cast<qulonglong>(std::min(progress.position + 1, progress.total));
  return QStringLiteral("%1 of %2, %3")
      .arg(shown)
      .arg(static_cast<qulonglong>(progress.total))
      .arg(featureOf(progress));
}

} // namespace

bool MainWindow::runJob(const QString& label, const QString& stage, const std::function<void(ModelJob&)>& job,
                        JobLog log) {
  // View picks are produced inside a frame; the presentation may not
  // begin a model computation until that frame has finished.
  Q_ASSERT(!m_viewer->isPainting());
  return m_documentSession->runJob(label, stage, job, log);
}

void MainWindow::waitForJob(const QString& label, ModelJob& job, const QElapsedTimer& clock, bool& logged) {
  if (!logged) {
    logged = true;
    qDebug().noquote() << QStringLiteral("Computing %1 started").arg(label);
  }
  QApplication::setOverrideCursor(Qt::WaitCursor);
  bool cursorSet = true;
  std::unique_ptr<ComputeProgress> dialog;
  QPointer<QWidget> before; // the window that had the input when it showed
  QEventLoop loop;
  // A wait ends when the worker is done, and when the dialog is due.
  const QMetaObject::Connection done = connect(m_worker, &ModelWorker::jobDone, &loop, &QEventLoop::quit);
  QString shown;
  qint64 shownAt = -kLogEveryMs;
  const auto tick = [&] {
    const JobProgress progress = job.control().progress();
    const qint64 elapsed = clock.elapsed();
    if (progress.total > 0) {
      const QString at = positionOf(progress);
      if (at != shown && elapsed - shownAt >= kLogEveryMs) {
        qDebug().noquote() << QStringLiteral("Computing %1: %2").arg(label, at);
        shown = at;
        shownAt = elapsed;
      }
    }
    if (dialog) {
      dialog->showProgress(job.stage(), progress, elapsed);
    } else if (elapsed >= kDialogAfterMs) {
      loop.quit();
    }
  };
  QTimer ticker;
  ticker.setInterval(kTickMs);
  connect(&ticker, &QTimer::timeout, &loop, tick);
  ticker.start();
  while (!m_worker->waitFor(milliseconds(0))) {
    if (!dialog && clock.elapsed() >= kDialogAfterMs) {
      // Modal to what has the input now: the window, or a dialog of its
      // own (Change Parameters), whose input it then blocks.
      QWidget* blocked = QApplication::activeModalWidget();
      before = QApplication::activeWindow();
      dialog = std::make_unique<ComputeProgress>(blocked != nullptr ? blocked : this);
      connect(dialog.get(), &ComputeProgress::cancelRequested, this, [&job, &label, &clock] {
        qDebug().noquote() << QStringLiteral("Computing %1: cancel requested after %2 ms").arg(label).arg(clock.elapsed());
        job.control().cancel();
      });
      dialog->showProgress(job.stage(), job.control().progress(), clock.elapsed());
      showDialog(dialog.get());
      // The dialog takes clicks; the cursor says so.
      QApplication::restoreOverrideCursor();
      cursorSet = false;
      qDebug().noquote() << QStringLiteral("Progress dialog shown: %1").arg(label);
    }
#ifdef MITCAD_MACOS_NATIVE_UI
    if (m_nativeUi) {
      waitNativeUntil([&] {
        return m_worker->waitFor(milliseconds(0)) || (!dialog && clock.elapsed() >= kDialogAfterMs);
      });
    } else
#endif
    {
      loop.exec(dialog ? QEventLoop::AllEvents : QEventLoop::ExcludeUserInputEvents);
    }
    // A loop returns at once while the application quits: wait a little.
    m_worker->waitFor(milliseconds(dialog ? 10 : 0));
  }
  disconnect(done);
  if (cursorSet) {
    QApplication::restoreOverrideCursor();
  }
  if (dialog) {
    // The input goes back to the window that had it (or the one the dialog
    // blocked) while the application is in front: without a window
    // manager, as on a test's X server, nothing else gives it back.
    const bool front = QApplication::activeWindow() == dialog.get();
    QPointer<QWidget> back = before ? before.data() : dialog->parentWidget()->window();
    dialog.reset();
    if (!m_nativeUi && front && back && back->isVisible()) {
      back->activateWindow();
    }
  }
}

void MainWindow::whenIdle(QObject* context, std::function<void()> call) {
  m_documentSession->whenIdle(context, std::move(call));
}

// ---------------------------------------------------------------------------
// Model commands and previews as jobs

QJsonObject MainWindow::command(const QJsonObject& command) {
  Q_ASSERT(!m_viewer->isPainting());
  const QString name = command.value(QStringLiteral("cmd")).toString();
  // Every change of the document passes here: a read-only window
  // (mitcad#89) refuses what would change the design.
  if (windowReadOnly() && !m_writeAnyway && !readOnlyModelCommand(name)) {
    throw ReadOnlyError(readOnlyReason().toStdString());
  }
  return m_documentSession->command(command);
}

QJsonObject MainWindow::modelPreview(const QByteArray& command, const QString& name) {
  Q_ASSERT(!m_viewer->isPainting());
  return m_documentSession->preview(command, name);
}

} // namespace mitcad
