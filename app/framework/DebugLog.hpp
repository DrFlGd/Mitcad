// SPDX-License-Identifier: MIT
#pragma once

// The debug log (mitcad#269): the application's log written to a file, so
// that a fault in real use can be traced afterwards.
//
// - Switched on in Preferences (Diagnostics: Write a debug log; the setting
//   diagnostics/debugLog, off by default, takes effect at once), for one
//   run by the command line option --debug-log [FILE] or the variable
//   MITCAD_DEBUG_LOG=1|<file>.
// - Every message the log ring sees (mcp/LogRing.hpp: debug, info, warning,
//   critical) goes into the file, one line each: the time in ms, the level,
//   the thread when it is not the GUI thread, the text.
// - The files: in the log folder (AppLocalDataLocation/logs;
//   MITCAD_DEBUG_LOG_DIR moves it, for tests), mitcad-YYYYMMDD-HHMMSS-<pid>.log,
//   or the file asked for. A file rolls over at 20 MB into a new one
//   (<name>-2.log when the name is taken; a file asked for continues in
//   <base>-2.log, -3, ...); the folder keeps at most 10 files and 200 MB,
//   the oldest going first.
// - Written at least once a second and at every warning, so that a crash
//   loses at most a second.
// - Each file begins with a header: the version, the build, the system,
//   the GPU and its OpenGL renderer, the command line, the open design's
//   path and the switches in use; the first file of a start then has the
//   lines the ring kept from before.
//   A log started before the 3D view (--debug-log, MITCAD_DEBUG_LOG, the
//   setting at start-up) makes its file at once but writes it once the
//   view knows its GPU and renderer (the fact "Graphics"), with the ring's
//   lines of the wait; after 5 s without them, at the first warning or
//   critical message, or when it is stopped, it writes the file with
//   "Graphics: not known yet", a line padded with spaces that the graphics
//   overwrite if they come later.
// - Logs stay on the computer: a report (report/ReportCenter) offers the
//   latest file, attached only by the user's choice. A log may contain file
//   paths, design names and entity names; typed text and picks only with
//   their own switches (MITCAD_LOG_TYPED_TEXT, MITCAD_LOG_PICKS).

#include <mutex>

#include <QDateTime>
#include <QFile>
#include <QList>
#include <QObject>
#include <QPair>
#include <QString>
#include <QStringList>
#include <QtGlobal>

class QTimer;

namespace mitcad {

namespace mcp {
struct LogLine;
}

struct DebugLogLimits {
  qint64 fileBytes = 20LL * 1024 * 1024;   // a file rolls over at this size
  int files = 10;                           // the folder keeps at most this many
  qint64 folderBytes = 200LL * 1024 * 1024; // and at most this much
  int graphicsWaitMs = 5000;               // a header waits this long for the graphics
};

// A log file in the folder, for the limits.
struct DebugLogFile {
  QString path;
  qint64 size = 0;
  QDateTime modified;
};

// The files to remove so that the folder keeps the limits, the oldest
// first (by modification time, then by name: x.log before x-2.log before
// x-10.log); never `keep` (the file being written), which counts.
QStringList debugLogFilesToRemove(QList<DebugLogFile> files, const QString& keep, const DebugLogLimits& limits);

// The log files of a folder: mitcad-YYYYMMDD-HHMMSS-<pid>[-n].log, or for a
// file asked for (`asked`, its name) <base>[-n].<suffix>.
QList<DebugLogFile> debugLogFiles(const QString& folder, const QString& asked = QString());

// What the command line and the environment ask for: --debug-log, a
// following argument that ends in .log as its file, --debug-log=FILE; else
// MITCAD_DEBUG_LOG (1: the log folder, another value but 0: that file).
// `arguments` keeps a plain --debug-log for QCommandLineParser.
struct DebugLogRequest {
  bool on = false;
  QString file;   // empty: a new file in the log folder
  QString source; // "--debug-log", "MITCAD_DEBUG_LOG"
};
DebugLogRequest debugLogRequest(QStringList& arguments, const QString& environment);

class DebugLog : public QObject {
  Q_OBJECT

public:
  // Made by the first call, on the GUI thread (main()); it lives until the
  // process ends.
  static DebugLog& instance();

  // AppLocalDataLocation/logs, or MITCAD_DEBUG_LOG_DIR.
  static QString defaultFolder();
  // mitcad-YYYYMMDD-HHMMSS-<pid>.log
  static QString fileName(const QDateTime& when, qint64 pid);
  // Preferences' switch (diagnostics/debugLog).
  static bool settingOn();
  static void setSettingOn(bool on);

  // Starts writing: to `file`, or to a new file in the log folder when it
  // is empty; `source` says what switched it on (the header's switches).
  // A log that runs is stopped first. On the GUI thread.
  bool start(const QString& file, const QString& source, QString* error = nullptr);
  void stop();
  bool running() const;
  QString currentFile() const;
  // Where the files go: the running file's folder, else the log folder.
  QString folder() const;
  // The running file, else the newest file of the log folder ("" none).
  QString latestFile() const;

  // A fact for the header ("Graphics", "Design"), from any thread.
  void setFact(const QString& name, const QString& value);
  // For tests: smaller limits.
  void setLimits(const DebugLogLimits& limits);
  void flush();

signals:
  void runningChanged(bool running);

private:
  DebugLog();
  static void sink(QtMsgType type, const mcp::LogLine& line);
  void write(QtMsgType type, const mcp::LogLine& line);
  // Under m_mutex:
  void writeText(const QString& text);
  void writeHeader(const QString& continuedFrom);
  void beginFile();   // the open file's header and the ring's earlier lines
  void openPending(); // the file whose header waited, now
  void rollOver();
  void prune();
  QString nextPath() const;
  QString fact(const QString& name) const;

  mutable std::mutex m_mutex;
  QFile m_file;
  QString m_asked;  // the file asked for, "" the log folder
  QString m_source; // what switched it on
  int m_part = 0;   // the file of this start: 1, 2, ...
  qint64 m_written = 0;
  quint64 m_snapshotUpTo = 0; // the ring's lines up to this one went in with the header
  qint64 m_lastFlush = 0;
  bool m_dirty = false;
  QString m_problem; // a failure inside the handler, logged by the timer
  DebugLogLimits m_limits;
  QList<QPair<QString, QString>> m_facts;
  QTimer* m_timer = nullptr;
  bool m_pending = false;            // started; the file opens once the graphics are known
  qint64 m_graphicsAt = -1;          // where a header's graphics line waits for them
  QTimer* m_graphicsTimer = nullptr; // or after graphicsWaitMs
};

} // namespace mitcad
