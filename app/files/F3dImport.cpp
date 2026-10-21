// SPDX-License-Identifier: MIT
#ifdef _WIN32
#include <windows.h>
#else
#include <cerrno>
#include <unistd.h>
#endif

#include "F3dImport.hpp"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <memory>
#include <string>
#include <thread>
#include <utility>

#include <QCloseEvent>
#include <QCommandLineParser>
#include <QCoreApplication>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLabel>
#include <QMessageBox>
#include <QProcess>
#include <QProcessEnvironment>
#include <QProgressBar>
#include <QPushButton>
#include <QRegularExpression>
#include <QSaveFile>
#include <QTimer>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/AppSettings.hpp"
#include "../framework/Diagnostics.hpp"
#include "../framework/Json.hpp"
#include "../framework/ResultCache.hpp"
#include "mitcad_bridge/lib.h"

#include "../framework/Dialogs.hpp"

namespace mitcad {
namespace {

// Seconds without progress after which the importer takes a geometry
// kernel call to hang (the model's hang_limit).
constexpr int kHangLimit = 90;

// The threads that evaluate an item's definitions at once (mitcad#95):
// the logical cores, at most 8 (more gain little: an item rarely has more
// definitions to try, and each worker holds its definition's memory).
int importThreads() {
  const unsigned cores = std::thread::hardware_concurrency();
  return static_cast<int>(std::clamp(cores, 1u, 8u));
}

const QRegularExpression kItemStart(QStringLiteral(R"(^import: \[[0-9.]+\] item ([0-9]+): (.*)$)"));
const QRegularExpression kItemDone(
    QStringLiteral(R"(^import: \[[0-9.]+\] (.+): (parametric|partial|fallback|skipped) in [0-9.]+ s$)"));

bool writeFile(const QString& path, const QByteArray& data) {
  QSaveFile file(path);
  if (!file.open(QIODevice::WriteOnly)) {
    return false;
  }
  file.write(data);
  return file.commit();
}

// The application's requests on the worker's standard input, a line each,
// until it closes: "stop" (T1e) cancels the job of the import's document,
// which stops the import early; "answer keep" or "answer replace"
// (mitcad#283) answers the import's question (relayQuestions). Read
// without the C library's streams, whose locks the process's exit takes
// while this thread waits.
void readRequests(const std::shared_ptr<rust::Box<JobControl>>& control) {
  std::string pending;
  char buffer[256];
  for (;;) {
#ifdef _WIN32
    DWORD count = 0;
    if (ReadFile(GetStdHandle(STD_INPUT_HANDLE), buffer, static_cast<DWORD>(sizeof buffer), &count, nullptr) == 0 ||
        count == 0) {
      return;
    }
#else
    const ssize_t count = ::read(STDIN_FILENO, buffer, sizeof buffer);
    if (count < 0 && errno == EINTR) {
      continue;
    }
    if (count <= 0) {
      return;
    }
#endif
    pending.append(buffer, static_cast<std::size_t>(count));
    std::size_t end = 0;
    while ((end = pending.find('\n')) != std::string::npos) {
      // (Without the line's end and spaces around: cmd's echo leaves some.)
      const QByteArray request = QByteArray::fromStdString(pending.substr(0, end)).trimmed();
      pending.erase(0, end + 1);
      if (request == "stop" && !(*control)->is_cancelled()) {
        (*control)->cancel();
        std::printf("import-stopping\n");
        std::fflush(stdout);
      } else if (request.startsWith("answer ")) {
        (*control)->answer(rustStr(request.mid(7).trimmed()));
      }
    }
  }
}

// The import's question to the user (mitcad#283), printed once as a line
// "import-ask <question>" for the application, until `done`.
void relayQuestions(const std::shared_ptr<rust::Box<JobControl>>& control,
                    const std::shared_ptr<std::atomic<bool>>& done) {
  std::string asked;
  while (!done->load()) {
    const rust::String question = (*control)->question();
    const std::string text(question.data(), question.size());
    if (!text.empty() && text != asked) {
      std::printf("import-ask %s\n", text.c_str());
      std::fflush(stdout);
    }
    asked = text;
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
  }
}

} // namespace

// ---------------------------------------------------------------------------
// The progress dialog

// The import's progress (T1e), modal to the window: the file, the timeline
// item being replayed of how many, the time so far, and two ways to end the
// import early. Esc and closing the dialog cancel the import; Enter and
// Space do nothing, as they were meant for the window.
class ImportProgress : public QDialog {
public:
  ImportProgress(QWidget* parent, std::function<void()> stop, std::function<void()> cancel)
      : QDialog(parent), m_stop(std::move(stop)), m_cancel(std::move(cancel)), m_file(new QLabel(this)),
        m_item(new QLabel(this)), m_bar(new QProgressBar(this)), m_time(new QLabel(this)) {
    setObjectName(QStringLiteral("importProgress"));
    setWindowTitle(F3dImport::tr("Import"));
    setWindowModality(Qt::WindowModal);
    setMinimumWidth(460);
    auto* layout = new QVBoxLayout(this);
    layout->addWidget(m_file);
    layout->addWidget(m_item);
    m_bar->setTextVisible(false);
    m_bar->setRange(0, 0); // busy until the worker counts the items
    layout->addWidget(m_bar);
    layout->addWidget(m_time);
    auto* buttons = new QDialogButtonBox(this);
    m_stopButton =
        buttons->addButton(F3dImport::tr("&Stop and Keep What Is Imported"), QDialogButtonBox::ActionRole);
    m_stopButton->setToolTip(F3dImport::tr("End the import now and keep what is imported"));
    QPushButton* cancelButton = buttons->addButton(F3dImport::tr("&Cancel Import"), QDialogButtonBox::RejectRole);
    cancelButton->setToolTip(F3dImport::tr("Discard the import; the open document stays as it is"));
    for (QPushButton* button : {m_stopButton, cancelButton}) {
      button->setAutoDefault(false);
      button->setDefault(false);
      button->setFocusPolicy(Qt::NoFocus);
    }
    connect(m_stopButton, &QPushButton::clicked, this, [this] { m_stop(); });
    connect(cancelButton, &QPushButton::clicked, this, [this] { m_cancel(); });
    layout->addWidget(buttons);
  }

  void showProgress(const QString& file, const QString& item, int done, int items, const QString& time) {
    m_file->setText(file);
    m_item->setText(item);
    if (items > 0) {
      m_bar->setRange(0, items);
      m_bar->setValue(std::min(done, items));
    }
    m_time->setText(time);
  }

  // A stop was asked for: once is enough.
  void setStopping() { m_stopButton->setEnabled(false); }

  // An import that cannot stop early (a FreeCAD document's bodies, an
  // .ipt part's) has only Cancel.
  void setStoppable(bool stoppable) { m_stopButton->setVisible(stoppable); }

protected:
  void reject() override { m_cancel(); }

  void closeEvent(QCloseEvent* event) override {
    event->ignore();
    m_cancel();
  }

private:
  std::function<void()> m_stop;
  std::function<void()> m_cancel;
  QLabel* m_file;
  QLabel* m_item;
  QProgressBar* m_bar;
  QLabel* m_time;
  QPushButton* m_stopButton = nullptr;
};

// ---------------------------------------------------------------------------
// The worker process

bool isImportWorker(int argc, char* argv[]) {
  for (int i = 1; i < argc; ++i) {
    if (std::strcmp(argv[i], "--import-worker") == 0) {
      return true;
    }
  }
  return false;
}

namespace {

// A FreeCAD document (.FCStd) rather than an .f3d design.
bool isFreeCadFile(const QString& path) {
  return QFileInfo(path).suffix().compare(QStringLiteral("fcstd"), Qt::CaseInsensitive) == 0;
}

// An .ipt part file or an .iam assembly (mitcad#60), both by import_ipt.
bool isIptFile(const QString& path) {
  const QString suffix = QFileInfo(path).suffix();
  return suffix.compare(QStringLiteral("ipt"), Qt::CaseInsensitive) == 0 ||
         suffix.compare(QStringLiteral("iam"), Qt::CaseInsensitive) == 0;
}

// An IFC file or archive (mitcad#273), by import_ifc.
bool isIfcFile(const QString& path) {
  const QString suffix = QFileInfo(path).suffix();
  return suffix.compare(QStringLiteral("ifc"), Qt::CaseInsensitive) == 0 ||
         suffix.compare(QStringLiteral("ifczip"), Qt::CaseInsensitive) == 0;
}

// A building project or family file (mitcad#274), by import_rvt.
bool isRvtFile(const QString& path) {
  const QString suffix = QFileInfo(path).suffix().toLower();
  return suffix == QLatin1String("rvt") || suffix == QLatin1String("rfa") || suffix == QLatin1String("rte") ||
         suffix == QLatin1String("rft");
}

// A SolidCraft design (mitcad#450), by import_solidcraft.
bool isSolidcraftFile(const QString& path) {
  const QString suffix = QFileInfo(path).suffix().toLower();
  return suffix == QLatin1String("solidcraft") || suffix == QLatin1String("solvecraft");
}

// A drawing's 3D content (mitcad#453), by import_dwg.
bool isDwgFile(const QString& path) {
  return QFileInfo(path).suffix().compare(QStringLiteral("dwg"), Qt::CaseInsensitive) == 0;
}

// The worker's exit status. While import threads that the hang watchdog
// gave up still run (their geometry kernel call has not returned), the
// worker ends here, without the static destructors that would tear down
// OCCT's state under them and crash the worker after it wrote its result
// (mitcad#82).
int endWorker(int code) {
  if (abandoned_imports() > 0) {
    std::fflush(nullptr);
    std::_Exit(code);
  }
  return code;
}

} // namespace

int runImportWorker(int argc, char* argv[]) {
  QCoreApplication app(argc, argv);
  QCommandLineParser parser;
  const QCommandLineOption worker(QStringLiteral("import-worker"),
                                  QStringLiteral(".f3d, .f3z, .FCStd, .ipt, .iam, .ifc, .ifczip, .rvt, .rfa, "
                                                 ".solidcraft or .dwg file"),
                                  QStringLiteral("file"));
  const QCommandLineOption output(QStringLiteral("output"), QStringLiteral("Design file to write"),
                                  QStringLiteral("file"));
  const QCommandLineOption result(QStringLiteral("result"), QStringLiteral("Result (JSON) to write"),
                                  QStringLiteral("file"));
  // The result store (P7d): the design's costly results go there, so that
  // the application does not compute them again when it reads the design.
  const QCommandLineOption store(QStringLiteral("result-store"), QStringLiteral("Result store folder"),
                                 QStringLiteral("folder"));
  const QCommandLineOption minMs(QStringLiteral("persist-min-ms"), QStringLiteral("Milliseconds"),
                                 QStringLiteral("ms"));
  // The import's evaluation handed over (mitcad#413): every result a
  // store takes, however quick, goes to this store of the import's own,
  // which the application reads the design with.
  const QCommandLineOption handOff(QStringLiteral("hand-off-store"), QStringLiteral("Hand-off store folder"),
                                   QStringLiteral("folder"));
  // More options of the import (JSON): the files of the documents the
  // design refers to that the user chose (mitcad#196).
  const QCommandLineOption more(QStringLiteral("import-options"), QStringLiteral("Import options (JSON)"),
                                QStringLiteral("file"));
  parser.addOptions({worker, output, result, store, minMs, handOff, more});
  parser.process(app);
  const QByteArray path = parser.value(worker).toUtf8();

  rust::Box<Document> document = new_document();
  // Stop (T1e): import_f3d takes the cancel of the document's job as a
  // request to stop early. For tests, the job's delay slows down the
  // definitions it tries (MITCAD_TEST_RECOMPUTE_DELAY_MS=import_f3d=<ms>).
  const auto control = std::make_shared<rust::Box<JobControl>>(new_job_control());
  if (const int delay = testRecomputeDelay(QStringLiteral("import_f3d")); delay > 0) {
    (*control)->set_test_delay(static_cast<std::uint32_t>(delay));
  }
  std::thread([control] { readRequests(control); }).detach();
  // A FreeCAD document: its structure, sketches and history in one
  // command, which has no stop yet (and no item count for the progress);
  // an .ipt part's bodies likewise.
  const bool freecad = isFreeCadFile(QString::fromUtf8(path));
  const bool ipt = isIptFile(QString::fromUtf8(path));
  // An IFC file: its building model in one command (mitcad#273).
  const bool ifc = isIfcFile(QString::fromUtf8(path));
  // A building project's elements likewise (mitcad#274).
  const bool rvt = isRvtFile(QString::fromUtf8(path));
  // A SolidCraft design: its history in one command (mitcad#450).
  const bool solidcraft = isSolidcraftFile(QString::fromUtf8(path));
  // A drawing's 3D content likewise (mitcad#453).
  const bool dwg = isDwgFile(QString::fromUtf8(path));
  // The timeline's length, for the progress dialog.
  try {
    if (!freecad && !ipt && !ifc && !rvt && !solidcraft && !dwg) {
      const QJsonObject designs = parseObject(document->import_f3d_timeline(rustStr(path), "{\"list\": true}"));
      int items = 0;
      for (const QJsonValue& design : designs.value(QStringLiteral("designs")).toArray()) {
        items = std::max(items, design.toObject().value(QStringLiteral("items")).toInt());
      }
      std::printf("import-items %d\n", items);
      std::fflush(stdout);
    }
  } catch (const std::exception&) {
    // The import itself says what is wrong with the file.
  }
  QJsonObject moreOptions;
  if (parser.isSet(more)) {
    QFile file(parser.value(more));
    if (file.open(QIODevice::ReadOnly)) {
      moreOptions = QJsonDocument::fromJson(file.readAll()).object();
    }
  }
  QJsonObject command =
      freecad ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("import_fcstd")},
                            {QStringLiteral("path"), QString::fromUtf8(path)},
                            {QStringLiteral("bodies_only"), false}}
      : ifc   ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("import_ifc")},
                            {QStringLiteral("path"), QString::fromUtf8(path)}}
      : rvt   ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("import_rvt")},
                            {QStringLiteral("path"), QString::fromUtf8(path)}}
      : solidcraft
          ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("import_solidcraft")},
                        {QStringLiteral("path"), QString::fromUtf8(path)}}
      : dwg   ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("import_dwg")},
                            {QStringLiteral("path"), QString::fromUtf8(path)}}
      : ipt   ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("import_ipt")},
                            {QStringLiteral("path"), QString::fromUtf8(path)},
                            {QStringLiteral("hang_limit"), kHangLimit}}
              : QJsonObject{{QStringLiteral("cmd"), QStringLiteral("import_f3d")},
                            {QStringLiteral("path"), QString::fromUtf8(path)},
                            {QStringLiteral("hang_limit"), kHangLimit},
                            // An item's definitions evaluated in parallel
                            // (mitcad#95): the logical cores, at most 8.
                            {QStringLiteral("threads"), importThreads()}};
  // (A building project's: the linked models' files chosen, mitcad#275.)
  if (!freecad && !ipt && !ifc && !solidcraft && !dwg) {
    for (auto it = moreOptions.begin(); it != moreOptions.end(); ++it) {
      command.insert(it.key(), it.value());
    }
  }
  // For tests: an .f3d design's timeline from a dump (import_f3d's `dump`)
  // instead of its own streams, as a synthetic design has none
  // (tools/ui-import-deviations-test.sh, mitcad#379).
  if (const QString dump = qEnvironmentVariable("MITCAD_TEST_IMPORT_DUMP");
      !dump.isEmpty() && !freecad && !ipt && !ifc && !rvt && !solidcraft && !dwg) {
    command.insert(QStringLiteral("dump"), dump);
  }
  QJsonObject answer;
  document->attach_job(**control);
  const auto relayed = std::make_shared<std::atomic<bool>>(false);
  std::thread questions([control, relayed] { relayQuestions(control, relayed); });
  const auto endQuestions = [&questions, &relayed] {
    relayed->store(true);
    if (questions.joinable()) {
      questions.join();
    }
  };
  try {
    answer = parseObject(document->command(rustStr(compactJson(command))));
  } catch (const std::exception& e) {
    endQuestions();
    std::fprintf(stderr, "import-failed %s\n", e.what());
    return endWorker(3);
  }
  endQuestions();
  // A stop from now on has nothing to stop.
  document->detach_job();
  const QByteArray label = QFileInfo(QString::fromUtf8(path)).fileName().toUtf8();
  if (parser.isSet(handOff)) {
    const QJsonObject handed = parseObject(document->hand_off_results(
        rustStr(parser.value(handOff).toUtf8()), rustStr(resultStoreBuildId().toUtf8()), rustStr(label)));
    std::printf("import-handed-over %d\n", handed.value(QStringLiteral("results")).toInt());
    std::fflush(stdout);
  }
  if (parser.isSet(store)) {
    const QByteArray folder = parser.value(store).toUtf8();
    document->set_result_store(rustStr(folder), rustStr(resultStoreBuildId().toUtf8()), rustStr(label));
    const QJsonObject persisted = parseObject(document->persist_results(parser.value(minMs).toDouble()));
    std::printf("import-stored %d\n", persisted.value(QStringLiteral("results")).toInt());
    std::fflush(stdout);
  }
  const rust::String design = document->to_json();
  if (!writeFile(parser.value(output), QByteArray(design.data(), static_cast<qsizetype>(design.size()))) ||
      !writeFile(parser.value(result), QJsonDocument(answer).toJson(QJsonDocument::Compact))) {
    std::fprintf(stderr, "import-failed cannot write the result\n");
    return endWorker(3);
  }
  std::printf("import-done\n");
  std::fflush(stdout);
  return endWorker(0);
}

// ---------------------------------------------------------------------------
// The application's side

F3dImport::F3dImport(const QString& path, QWidget* window)
    : QObject(window), m_path(path), m_window(window) {}

F3dImport::~F3dImport() {
  if (m_process != nullptr && m_process->state() != QProcess::NotRunning) {
    m_process->kill();
    m_process->waitForFinished(2000);
  }
  delete m_dialog;
}

void F3dImport::start() {
  if (!m_dir.isValid()) {
    done(false, tr("No temporary directory for the import."));
    return;
  }
  m_process = new QProcess(this);
  QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
  // The importer's trace gives a line per timeline item: the progress.
  environment.insert(QStringLiteral("MITCAD_IMPORT_TRACE"), QStringLiteral("1"));
  m_process->setProcessEnvironment(environment);
  connect(m_process, &QProcess::readyReadStandardOutput, this, &F3dImport::readProgress);
  connect(m_process, &QProcess::readyReadStandardError, this, &F3dImport::readProgress);
  connect(m_process, &QProcess::finished, this, [this](int exitCode) { processFinished(exitCode); });
  connect(m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
    if (error == QProcess::FailedToStart) {
      done(false, tr("The import could not start: %1").arg(m_process->errorString()));
    }
  });

  const QString name = QFileInfo(m_path).fileName();
  if (m_preset) {
    // (Run again with the options of the import before, mitcad#379.)
  } else if (!isFreeCadFile(m_path) && !isIptFile(m_path) && !isIfcFile(m_path) && !isRvtFile(m_path) &&
             !isSolidcraftFile(m_path) && !isDwgFile(m_path)) {
    m_options = askReferences();
    // The geometric tolerance of the Preferences (mitcad#224).
    m_options.insert(QStringLiteral("geometric_tolerance"), GeneralSettings::load().importTolerance);
    // Whether a retry on the file's stored bodies replaces imported bodies
    // that match them: asked when it comes to that (mitcad#283).
    m_options.insert(QStringLiteral("retry_bodies"), QStringLiteral("ask"));
  } else if (isRvtFile(m_path)) {
    m_options = askRvtLinks();
  }
  m_dialog = new ImportProgress(
      m_window, [this] { stop(); }, [this] { cancel(); });
  m_dialog->setStoppable(!isFreeCadFile(m_path) && !isIptFile(m_path) && !isIfcFile(m_path) &&
                         !isRvtFile(m_path) && !isSolidcraftFile(m_path) && !isDwgFile(m_path));
  m_ticker = new QTimer(this);
  m_ticker->setInterval(500);
  connect(m_ticker, &QTimer::timeout, this, &F3dImport::tick);

  m_clock.start();
  m_ticker->start();
  tick();
  showDialog(m_dialog);
  qInfo().noquote() << QStringLiteral("Import of %1 started").arg(name);
  QStringList arguments{QStringLiteral("--import-worker"), m_path, QStringLiteral("--output"),
                        m_dir.filePath(QStringLiteral("import.mitcad")), QStringLiteral("--result"),
                        m_dir.filePath(QStringLiteral("result.json"))};
  // The import's evaluation comes back in a store of its own (mitcad#413),
  // whether the user's store is on or not.
  arguments << QStringLiteral("--hand-off-store") << m_dir.filePath(QStringLiteral("results"));
  // The worker has no application name for the user's folders: it is
  // told where the result store is (P7d).
  if (const QString store = resultStoreDirectory(); !store.isEmpty()) {
    arguments << QStringLiteral("--result-store") << store << QStringLiteral("--persist-min-ms")
              << QString::number(resultStoreMinMs());
  }
  if (!m_options.isEmpty()) {
    const QString options = m_dir.filePath(QStringLiteral("options.json"));
    if (writeFile(options, QJsonDocument(m_options).toJson(QJsonDocument::Compact))) {
      arguments << QStringLiteral("--import-options") << options;
    }
  }
  m_process->start(QCoreApplication::applicationFilePath(), arguments);
}

void F3dImport::setOptions(const QJsonObject& options) {
  m_options = options;
  m_preset = true;
}

QJsonObject F3dImport::askReferences() {
  QJsonObject files;
  QJsonArray skipped;
  QStringList asked;
  QString folder = QFileInfo(m_path).absolutePath();
  const QString design = QFileInfo(m_path).fileName();
  for (;;) {
    // Listed again after each chosen file: its folder may hold the others,
    // and a document found refers to its own.
    QJsonObject listed;
    try {
      const QJsonObject options{{QStringLiteral("list_references"), true},
                                {QStringLiteral("reference_files"), files}};
      listed = parseObject(new_document()->import_f3d_timeline(rustStr(m_path.toUtf8()), rustStr(compactJson(options))));
    } catch (const std::exception& e) {
      // The import itself says what is wrong with the file.
      qInfo().noquote() << QStringLiteral("Import references of %1 not listed: %2").arg(design, QString::fromUtf8(e.what()));
      break;
    }
    QJsonObject missing;
    int found = 0;
    for (const QJsonValue& value : listed.value(QStringLiteral("documents")).toArray()) {
      const QJsonObject document = value.toObject();
      if (!document.value(QStringLiteral("file")).isString()) {
        if (missing.isEmpty() && !asked.contains(document.value(QStringLiteral("document")).toString())) {
          missing = document;
        }
      } else {
        ++found;
      }
    }
    if (missing.isEmpty()) {
      qInfo().noquote() << QStringLiteral("Import references of %1: %2 found, %3 skipped")
                               .arg(design)
                               .arg(found)
                               .arg(skipped.size());
      break;
    }
    const QString urn = missing.value(QStringLiteral("document")).toString();
    asked.append(urn);
    const QString name = missing.value(QStringLiteral("name")).toString();
    const int version = missing.value(QStringLiteral("version")).toInt();
    const QString what = name.isEmpty() ? tr("a Document %1 Refers To").arg(design) : name;
    const QString title = version > 0 ? tr("Find %1, Version %2").arg(what).arg(version) : tr("Find %1").arg(what);
    qInfo().noquote() << QStringLiteral("References dialog: %1 version %2 of %3")
                             .arg(name.isEmpty() ? QStringLiteral("?") : name)
                             .arg(version)
                             .arg(design);
    const QString file =
        dialogChooseFile(m_window, title, folder, tr("Designs (*.f3d *.f3z)"), tr("&Use"), tr("&Skip"));
    if (file.isEmpty()) {
      skipped.append(urn);
      qInfo().noquote() << QStringLiteral("References dialog: skipped");
      continue;
    }
    files.insert(urn, file);
    folder = QFileInfo(file).absolutePath();
    qInfo().noquote() << QStringLiteral("References dialog: chose %1").arg(QFileInfo(file).fileName());
  }
  QJsonObject options;
  if (!files.isEmpty()) {
    options.insert(QStringLiteral("reference_files"), files);
  }
  if (!skipped.isEmpty()) {
    options.insert(QStringLiteral("skipped_references"), skipped);
  }
  return options;
}

QJsonObject F3dImport::askRvtLinks() {
  QJsonObject files;
  QString folder = QFileInfo(m_path).absolutePath();
  const QString project = QFileInfo(m_path).fileName();
  // Listed once (mitcad#413: each listing reads the project): a link is
  // found by its own name, so a file chosen for one finds no other.
  QJsonObject listed;
  try {
    const QJsonObject options{{QStringLiteral("list_links"), true}};
    listed = parseObject(new_document()->import_rvt(rustStr(m_path.toUtf8()), rustStr(compactJson(options))));
  } catch (const std::exception& e) {
    // The import itself says what is wrong with the file.
    qInfo().noquote() << QStringLiteral("Import links of %1 not listed: %2").arg(project, QString::fromUtf8(e.what()));
  }
  QStringList asked;
  for (const QJsonValue& value : listed.value(QStringLiteral("links")).toArray()) {
    const QJsonObject link = value.toObject();
    const QString missing = link.value(QStringLiteral("file")).toString();
    if (link.value(QStringLiteral("found")).isString() || missing.isEmpty() || asked.contains(missing)) {
      continue;
    }
    asked.append(missing);
    qInfo().noquote() << QStringLiteral("Links dialog: %1 of %2").arg(missing, project);
    const QString file = dialogChooseFile(m_window, tr("Find the Linked Model %1").arg(missing), folder,
                                          tr("Building projects (*.rvt)"), tr("&Use"), tr("&Skip"));
    if (file.isEmpty()) {
      qInfo().noquote() << QStringLiteral("Links dialog: skipped");
      continue;
    }
    files.insert(missing, file);
    folder = QFileInfo(file).absolutePath();
    qInfo().noquote() << QStringLiteral("Links dialog: chose %1").arg(QFileInfo(file).fileName());
  }
  QJsonObject options;
  if (!files.isEmpty()) {
    options.insert(QStringLiteral("link_files"), files);
  }
  return options;
}

void F3dImport::readProgress() {
  if (m_process == nullptr) {
    return;
  }
  m_errors += m_process->readAllStandardError();
  m_errors += m_process->readAllStandardOutput();
  const QString name = QFileInfo(m_path).fileName();
  qsizetype end = 0;
  while ((end = m_errors.indexOf('\n')) >= 0) {
    const QString line = QString::fromUtf8(m_errors.left(end)).trimmed();
    m_errors.remove(0, end + 1);
    if (line.startsWith(QStringLiteral("import-items "))) {
      m_items = line.mid(13).toInt();
      continue;
    }
    if (line.startsWith(QStringLiteral("import-handed-over "))) {
      m_handedOver = line.mid(19).toInt();
      qInfo().noquote() << QStringLiteral("Import handed over %1 results").arg(m_handedOver);
      continue;
    }
    if (line.startsWith(QStringLiteral("import-stored "))) {
      qInfo().noquote() << QStringLiteral("Import stored %1 results").arg(line.mid(14).toInt());
      continue;
    }
    if (line.startsWith(QStringLiteral("import-ask "))) {
      // (Asked once back in the event loop, not inside this read.)
      const QString question = line.mid(11);
      QTimer::singleShot(0, this, [this, question] { askRetryBodies(question); });
      continue;
    }
    if (line == QStringLiteral("import-stopping")) {
      qInfo().noquote() << QStringLiteral("Import of %1 stopping: the remaining items take the file's bodies")
                               .arg(name);
      continue;
    }
    const QRegularExpressionMatch start = kItemStart.match(line);
    if (start.hasMatch()) {
      m_position = start.captured(1).toInt();
      m_current = start.captured(2);
      continue;
    }
    const QRegularExpressionMatch item = kItemDone.match(line);
    if (item.hasMatch()) {
      ++m_done;
      qDebug().noquote() << QStringLiteral("Import item %1: %2 %3").arg(m_done).arg(item.captured(1), item.captured(2));
      continue;
    }
    if (line.startsWith(QStringLiteral("import-failed ")) ||
        (!line.startsWith(QStringLiteral("import:")) && !line.isEmpty())) {
      m_lastLine = line.startsWith(QStringLiteral("import-failed ")) ? line.mid(14) : line;
    }
  }
  tick();
}

void F3dImport::askRetryBodies(const QString& question) {
  if (m_finished || m_process == nullptr || m_question != nullptr) {
    return;
  }
  const QString name = QFileInfo(m_path).fileName();
  qInfo().noquote() << QStringLiteral("Import of %1 asks: %2").arg(name, question);
  // (On the heap, under the window: the import may end, and this object
  // go, while it asks.)
  QPointer<QMessageBox> box =
      new QMessageBox(QMessageBox::Question, tr("Keep the Imported Bodies?"), question, QMessageBox::NoButton,
                      m_window.data());
  box->setObjectName(QStringLiteral("importRetryBodies"));
  QPushButton* keep = box->addButton(tr("&Keep Imported Bodies"), QMessageBox::AcceptRole);
  keep->setToolTip(tr("The bodies imported as features stay"));
  QPushButton* replace = box->addButton(tr("&Replace with Stored Bodies"), QMessageBox::DestructiveRole);
  replace->setToolTip(tr("The file's stored bodies replace them, so that the item can build on those"));
  box->setDefaultButton(keep);
  box->setEscapeButton(keep);
  m_question = box;
  const QPointer<F3dImport> self(this);
  prepareModal(box);
  execDialog(box);
  if (box == nullptr) {
    return;
  }
  const bool replacing = box->clickedButton() == replace;
  box->deleteLater();
  if (self == nullptr || m_finished || m_process == nullptr) {
    return;
  }
  m_question = nullptr;
  m_process->write(replacing ? "answer replace\n" : "answer keep\n");
  qInfo().noquote() << QStringLiteral("Import of %1 answer: %2")
                           .arg(name, replacing ? QStringLiteral("replace") : QStringLiteral("keep"));
}

void F3dImport::tick() {
  if (m_finished || m_dialog == nullptr) {
    return;
  }
  const qint64 seconds = m_clock.elapsed() / 1000;
  QString item;
  if (m_items > 0) {
    const int position = std::clamp(m_position, 1, m_items);
    item = m_current.isEmpty() ? tr("Timeline item %1 of %2").arg(position).arg(m_items)
                               : tr("Timeline item %1 of %2: %3").arg(position).arg(m_items).arg(m_current);
  }
  const QString time = m_stopping ? tr("Stopping: the remaining items come in as the file's bodies (%1 s)").arg(seconds)
                                  : tr("%1 s elapsed").arg(seconds);
  m_dialog->showProgress(tr("Importing %1").arg(QFileInfo(m_path).fileName()), item, m_done, m_items, time);
}

void F3dImport::stop() {
  if (m_finished || m_stopping || m_process == nullptr) {
    return;
  }
  m_stopping = true;
  qInfo().noquote() << QStringLiteral("Import of %1: stop requested after %2 s")
                           .arg(QFileInfo(m_path).fileName())
                           .arg(m_clock.elapsed() / 1000);
  m_process->write("stop\n");
  m_dialog->setStopping();
  tick();
}

void F3dImport::cancel() {
  if (m_finished) {
    return;
  }
  qInfo().noquote() << QStringLiteral("Import of %1 cancelled").arg(QFileInfo(m_path).fileName());
  if (m_process != nullptr) {
    m_process->kill();
  }
  done(false, QString());
}

void F3dImport::processFinished(int exitCode) {
  if (m_finished) {
    return;
  }
  readProgress();
  if (exitCode != 0 || m_process->exitStatus() != QProcess::NormalExit) {
    const QString why = m_lastLine.isEmpty() ? tr("the import process stopped (exit code %1)").arg(exitCode)
                                             : m_lastLine;
    done(false, why);
    return;
  }
  QFile design(m_dir.filePath(QStringLiteral("import.mitcad")));
  QFile result(m_dir.filePath(QStringLiteral("result.json")));
  if (!design.open(QIODevice::ReadOnly) || !result.open(QIODevice::ReadOnly)) {
    done(false, tr("The import wrote no result."));
    return;
  }
  m_finished = true;
  m_ticker->stop();
  m_dialog->hide();
  if (m_question != nullptr) {
    m_question->reject(); // (the import no longer waits for the answer)
  }
  emit finished(true, design.readAll(), QJsonDocument::fromJson(result.readAll()).object(), QString());
  deleteLater();
}

void F3dImport::done(bool ok, const QString& error) {
  if (m_finished) {
    return;
  }
  m_finished = true;
  if (m_ticker != nullptr) {
    m_ticker->stop();
  }
  if (m_dialog != nullptr) {
    m_dialog->hide();
  }
  if (m_question != nullptr) {
    m_question->reject();
  }
  emit finished(ok, QByteArray(), QJsonObject(), error);
  deleteLater();
}

} // namespace mitcad
