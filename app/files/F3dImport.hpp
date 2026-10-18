// SPDX-License-Identifier: MIT
#pragma once

#include <QElapsedTimer>
#include <QJsonObject>
#include <QObject>
#include <QPointer>
#include <QString>
#include <QTemporaryDir>

class QMessageBox;
class QProcess;
class QTimer;
class QWidget;

namespace mitcad {

class ImportProgress;

// An .f3d design (.f3d, .f3z) imported with its history (U6, the model's
// import_f3d), a FreeCAD document (.FCStd) with its stored bodies
// (import_fcstd), an .ipt part's stored bodies (import_ipt, mitcad#60), or
// an IFC building model (import_ifc, mitcad#273),
// an .rvt project's elements by level (import_rvt, mitcad#274),
// in a process of its own: the application runs itself with
// --import-worker, and a crash of the geometry kernel cannot take the
// window with it. The import runs to its end; a progress dialog shows the
// timeline item being replayed and offers two ways to end it early (T1e):
// Stop and Keep What Is Imported asks the worker to stop (the remaining
// items take the file's bodies, and the report says where it stopped; not
// for a FreeCAD document or a part file, which have no timeline to replay
// yet), and Cancel Import stops the process, leaving the open document as
// it is. The first time an item of an .f3d design is to be tried again on
// the file's stored bodies, which would replace bodies the import already
// has as the file's (mitcad#283), the user is asked whether to keep the
// imported (parametric) bodies or to replace them; the answer holds for
// the rest of the import.
class F3dImport : public QObject {
  Q_OBJECT

public:
  F3dImport(const QString& path, QWidget* window);
  ~F3dImport() override;

  void start();

  // The import's options, set before start(): the import runs with these
  // (an .f3d design run again with items whose deviating results the user
  // keeps, mitcad#379) instead of asking for the documents it refers to.
  void setOptions(const QJsonObject& options);
  // The options the import ran with (the documents chosen, ...).
  const QJsonObject& options() const { return m_options; }
  // The store the worker hands the import's evaluation over in (every
  // result a store takes, mitcad#413), while this object lives: the
  // application reads the imported design with it rather than evaluating
  // every feature again. Empty: none was written.
  QString handedOverResults() const {
    return m_handedOver > 0 ? m_dir.filePath(QStringLiteral("results")) : QString();
  }

signals:
  // ok: `design` is the imported document's design file (JSON) and
  // `result` the import_f3d command's result with its report. Not ok:
  // `error` says why (cancelled: empty).
  void finished(bool ok, const QByteArray& design, const QJsonObject& result, const QString& error);

private:
  // The documents the design refers to (its components of other
  // documents, mitcad#196) whose files are not found next to it: a file
  // dialog for each, naming the document, whose Cancel skips it; a chosen
  // file's folder is searched for the other missing ones. Returns the
  // import's options: the chosen files and the skipped documents.
  QJsonObject askReferences();
  // A building project's linked models (mitcad#275) not found next to it:
  // a file dialog for each, naming the linked file, whose Cancel skips it
  // (its component stays empty). Returns the import's options: the
  // chosen files by the links' file names.
  QJsonObject askRvtLinks();
  void readProgress();
  // The worker's question (mitcad#283): keep the imported bodies that
  // match the file's or replace them with the stored ones; the answer goes
  // to the worker.
  void askRetryBodies(const QString& question);
  void processFinished(int exitCode);
  void stop();
  void cancel();
  void tick();
  void done(bool ok, const QString& error);

  QString m_path;
  QJsonObject m_options; // more options of the import (askReferences)
  bool m_preset = false; // m_options given (setOptions)
  QPointer<QWidget> m_window;
  QTemporaryDir m_dir;
  int m_handedOver = 0; // results the worker handed over (handedOverResults)
  QProcess* m_process = nullptr;
  ImportProgress* m_dialog = nullptr;
  QPointer<QMessageBox> m_question; // askRetryBodies, while it asks
  QTimer* m_ticker = nullptr;
  QElapsedTimer m_clock;
  QByteArray m_errors; // the worker's standard error, line by line
  int m_items = 0;     // timeline items, when the worker said
  int m_done = 0;      // items replayed
  int m_position = 0;  // the item being replayed (from 1)
  QString m_current;   // its name
  QString m_lastLine;  // the worker's last message
  bool m_stopping = false;
  bool m_finished = false;
};

// The worker process: imports the file given after --import-worker and
// writes the design (--output) and the command's result (--result). A
// line "stop" on its standard input stops the import early. The import's
// question to the user comes as a line "import-ask <question>" on its
// standard output, the answer as "answer keep" or "answer replace" on its
// standard input. Returns the process's exit code.
int runImportWorker(int argc, char* argv[]);

// Whether the command line asks for the worker.
bool isImportWorker(int argc, char* argv[]);

} // namespace mitcad
