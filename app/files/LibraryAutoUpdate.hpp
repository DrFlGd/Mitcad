// SPDX-License-Identifier: MIT
#pragma once

// Auto-update of the library sources (mitcad#576): the sources that are on
// and have Auto-update on (Tools > Libraries...) are fetched in the
// background when Mitcad starts, one after another, so that newer versions
// are there to choose from. Each fetch is a RemoteTask (a thread of its own
// as in fetchLibraries), but without a progress dialog: nothing blocks the
// window. A failure (offline, server down) is only logged. A design's parts
// keep their chosen version; the fetch only brings newer ones.
//
// The one object (instance()) also tells the open library browser, the
// Libraries dialog and the part picker that something new came (updated()),
// so that they list it. The main window starts it once after it shows
// (MainWindow::updateLibrariesAtStart); the CLI and the MCP server have no
// such start, nor has a run of a UI test script or a screenshot run.

#include <QObject>
#include <QPointer>
#include <QString>
#include <QStringList>

namespace mitcad {

class RemoteTask;

class LibraryAutoUpdate : public QObject {
  Q_OBJECT

public:
  // The updater of the application, made the first time it is asked for.
  static LibraryAutoUpdate& instance();

  // Fetches the sources with Auto-update (and On) one after another; nothing
  // when none has it, or while a run is going on.
  void start();
  // Ends a run that is going on (the fetch under way is cancelled and
  // waited for); a fetch the user asks for does this first.
  void cancel();
  bool isRunning() const;

  // What a run says in the status bar: "Libraries updated: mitcad-parts
  // v2.7.0", the failures as a short note after it or alone; empty when
  // nothing is new and nothing failed. `news`: one "<name> <version>" per
  // source with a version that was not there before.
  static QString summary(const QStringList& news, int failed);

signals:
  // A fetch brought something new (a version, or a source fetched for the
  // first time): lists that show libraries read them again.
  void updated();
  // A run ended: the text for the status bar (summary()), empty for none.
  void finished(const QString& message);

private:
  explicit LibraryAutoUpdate(QObject* parent);
  void next();
  void fetched(const QString& url);
  void finish();

  QStringList m_queue;
  QPointer<RemoteTask> m_task;
  // The versions of the source under way before its fetch (their texts).
  QStringList m_before;
  QStringList m_news;
  int m_fetched = 0;
  int m_failed = 0;
};

} // namespace mitcad
