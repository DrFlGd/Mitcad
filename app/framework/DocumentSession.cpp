// SPDX-License-Identifier: MIT
#include "DocumentSession.hpp"

#include <chrono>
#include <cstdint>
#include <exception>
#include <utility>

#include <QEventLoop>
#include <QFileInfo>
#include <QJsonDocument>
#include <QtLogging>

#include "CommandSession.hpp"
#include "Diagnostics.hpp"
#include "Json.hpp"
#include "ResultCache.hpp"

namespace mitcad {
namespace {

constexpr std::chrono::milliseconds kBlockFor(50);

QString commandStage(QString name) {
  name.replace(QLatin1Char('_'), QLatin1Char(' ')).replace(QLatin1Char('.'), QLatin1Char(' '));
  if (!name.isEmpty()) {
    name[0] = name[0].toUpper();
  }
  return name;
}

} // namespace

struct DocumentSession::AsyncJob {
  ModelJob job;
  QString label;
  QElapsedTimer clock;
  std::shared_ptr<AsyncResult> result;
  QPointer<QObject> context;
  Completion completion;
};

DocumentSession::State::State(bool demo) : document(demo ? new_demo_document() : new_document()) {}

DocumentSession::DocumentSession(bool demo, QObject* parent)
    : QObject(parent), m_state(demo), m_worker(std::make_unique<ModelWorker>()) {
  markSaved();
  m_state.document->set_result_store(rustStr(resultStoreDirectory().toUtf8()),
                                    rustStr(resultStoreBuildId().toUtf8()), rustStr(tr("Untitled").toUtf8()));
  m_state.document->set_memory_cache_budget(static_cast<std::uint64_t>(CacheSettings::load().memoryMegabytes));
  collectResultStoreGarbage();
  connect(m_worker.get(), &ModelWorker::jobDone, this, &DocumentSession::completeAsync, Qt::QueuedConnection);
  m_progressTimer.setInterval(100);
  connect(&m_progressTimer, &QTimer::timeout, this, [this] {
    if (!m_async || !m_job) {
      return;
    }
    const JobProgress progress = m_job->control().progress();
    emit jobProgress(m_job->stage(), static_cast<quint64>(progress.position),
                     static_cast<quint64>(progress.total), m_async->clock.elapsed());
  });
  m_worker->start();
  notifyChanged();
}

DocumentSession::~DocumentSession() {
  m_progressTimer.stop();
  cancelJob();
  // The worker must release the document before the owned job and model
  // are destroyed. Queued completions cannot run during this destruction.
  m_worker.reset();
  m_job = nullptr;
  m_async.reset();
  delete m_state.activeCommand;
}

const Document& DocumentSession::idleDocument() const {
  if (busy()) {
    throw ModelBusy();
  }
  return *m_state.document;
}

QJsonValue DocumentSession::query(const QJsonObject& request) const {
  const QJsonDocument answer = parseJson(idleDocument().query(rustStr(compactJson(request))));
  return answer.isArray() ? QJsonValue(answer.array()) : QJsonValue(answer.object());
}

std::shared_ptr<geometry::Shape> DocumentSession::bodyShape(const QString& uid) const {
  return idleDocument().body_shape(uid.toStdString());
}

std::shared_ptr<geometry::Shape> DocumentSession::previewBodyShape(const std::string& uid) const {
  return idleDocument().preview_body_shape(uid);
}

std::shared_ptr<geometry::Shape> DocumentSession::previewToolShape() const {
  return idleDocument().preview_tool_shape();
}

int DocumentSession::undoDepth() const {
  return query({{QStringLiteral("query"), QStringLiteral("document")}}).toObject()
      .value(QStringLiteral("undo_depth")).toInt();
}

bool DocumentSession::modified() const {
  return query({{QStringLiteral("query"), QStringLiteral("document")}}).toObject()
      .value(QStringLiteral("modified")).toBool();
}

DocumentSession::Snapshot DocumentSession::snapshot() const {
  Snapshot result;
  // During computation the cached info is the last committed state; no
  // document is read concurrently with the model worker.
  result.document = busy() ? m_documentInfo
                          : query({{QStringLiteral("query"), QStringLiteral("document")}}).toObject();
  result.selection = m_state.selection;
  result.mode = m_state.mode;
  result.filePath = m_state.filePath;
  result.name = m_state.filePath.isEmpty()
                    ? (m_state.documentName.isEmpty() ? tr("Untitled") : m_state.documentName)
                    : QFileInfo(m_state.filePath).fileName();
  result.busy = busy();
  result.modified = result.document.value(QStringLiteral("modified")).toBool();
  result.canUndo = !result.document.value(QStringLiteral("undo")).toString().isEmpty();
  result.canRedo = !result.document.value(QStringLiteral("redo")).toString().isEmpty();
  result.lastError = m_state.lastError;
  result.lastCancelled = m_state.lastCancelled;
  return result;
}

bool DocumentSession::runJob(const QString& label, const QString& stage, const Job& job, JobLog log) {
  if (busy()) {
    throw ModelBusy();
  }
  ModelJob state;
  state.setStage(stage);
  if (const int delay = testRecomputeDelay(label); delay > 0) {
    state.control().set_test_delay(static_cast<std::uint32_t>(delay));
  }
  bool logged = log == JobLog::Always;
  if (logged) {
    qDebug().noquote() << QStringLiteral("Computing %1 started").arg(label);
  }
  QElapsedTimer clock;
  clock.start();
  m_job = &state;
  emit busyChanged(true);
  emit jobStarted(label, stage);
  std::exception_ptr error;
  if (computeInline()) {
    try {
      job(state);
    } catch (...) {
      error = std::current_exception();
    }
  } else {
    m_worker->post([&job, &state] { job(state); });
    try {
      if (!m_worker->waitFor(kBlockFor)) {
        if (m_waitHandler) {
          m_waitHandler(label, state, clock, logged);
        } else {
          waitWithoutPresentation(state);
        }
      }
    } catch (...) {
      state.control().cancel();
      m_worker->take();
      finishJob(label, true, QString());
      throw;
    }
    error = m_worker->take();
  }
  const bool cancelled = error && state.control().is_cancelled();
  QString failure;
  if (error && !cancelled) {
    try {
      std::rethrow_exception(error);
    } catch (const std::exception& exception) {
      failure = errorText(exception);
    } catch (...) {
      failure = tr("An unknown computation error occurred.");
    }
  }
  finishJob(label, cancelled, failure);
  if (cancelled) {
    qDebug().noquote() << QStringLiteral("Computing %1 cancelled after %2 ms").arg(label).arg(clock.elapsed());
    return false;
  }
  if (logged) {
    qDebug().noquote() << QStringLiteral("Computing %1 done in %2 ms (%3 evaluated)")
                             .arg(label).arg(clock.elapsed())
                             .arg(static_cast<qulonglong>(state.control().progress().evaluated));
  }
  if (error) {
    std::rethrow_exception(error);
  }
  return true;
}

void DocumentSession::waitWithoutPresentation(ModelJob& job) {
  QEventLoop loop;
  const auto done = connect(m_worker.get(), &ModelWorker::jobDone, &loop, &QEventLoop::quit);
  QTimer ticker;
  ticker.setInterval(100);
  QElapsedTimer clock;
  clock.start();
  connect(&ticker, &QTimer::timeout, &loop, [this, &job, &clock] {
    const JobProgress progress = job.control().progress();
    emit jobProgress(job.stage(), static_cast<quint64>(progress.position),
                     static_cast<quint64>(progress.total), clock.elapsed());
  });
  ticker.start();
  while (!m_worker->waitFor(std::chrono::milliseconds(0))) {
    loop.exec(QEventLoop::ExcludeUserInputEvents);
    m_worker->waitFor(std::chrono::milliseconds(10));
  }
  disconnect(done);
}

QJsonObject DocumentSession::command(const QJsonObject& request) {
  const QString name = request.value(QStringLiteral("cmd")).toString();
  const QByteArray json = compactJson(request);
  rust::String answer;
  Document& document = *m_state.document;
  ScopedTiming timing("command");
  timing.setDetail(name);
  const bool done = runJob(name, commandStage(name), [&document, &json, &answer](ModelJob& job) {
    const AttachedJob attached(document, job);
    answer = document.command(rustStr(json));
  }, JobLog::WhenSlow);
  if (!done) {
    throw ComputationCancelled();
  }
  timing.finish();
  return parseObject(answer);
}

QJsonObject DocumentSession::preview(const QByteArray& request, const QString& name) {
  rust::String answer;
  Document& document = *m_state.document;
  ScopedTiming timing("preview");
  timing.setDetail(name);
  const bool done = runJob(QStringLiteral("preview"), tr("Preview of %1").arg(name),
                          [&document, &request, &answer](ModelJob& job) {
    const AttachedJob attached(document, job);
    answer = document.preview(rustStr(request));
  }, JobLog::WhenSlow);
  if (!done) {
    throw ComputationCancelled();
  }
  timing.finish();
  return parseObject(answer);
}

bool DocumentSession::commandAsync(const QJsonObject& request, QObject* context, Completion completion) {
  const QString name = request.value(QStringLiteral("cmd")).toString();
  const QByteArray json = compactJson(request);
  auto result = std::make_shared<AsyncResult>();
  Document* document = &*m_state.document;
  return startAsync(name, commandStage(name), [document, json, result](ModelJob& job) {
    const AttachedJob attached(*document, job);
    result->answer = parseObject(document->command(rustStr(json)));
  }, result, context, std::move(completion));
}

bool DocumentSession::previewAsync(const QByteArray& request, const QString& name, QObject* context,
                                   Completion completion) {
  auto result = std::make_shared<AsyncResult>();
  Document* document = &*m_state.document;
  return startAsync(QStringLiteral("preview"), tr("Preview of %1").arg(name),
                    [document, request, result](ModelJob& job) {
    const AttachedJob attached(*document, job);
    result->answer = parseObject(document->preview(rustStr(request)));
  }, result, context, std::move(completion));
}

bool DocumentSession::inspectionAsync(const QString& name, QObject* context, InspectionTask task,
                                      InspectionCompletion completion) {
  auto result = std::make_shared<AsyncResult>();
  auto inspection = std::make_shared<Inspection>();
  Document* document = &*m_state.document;
  return startAsync(QStringLiteral("inspection"), tr("Inspection of %1").arg(name),
                    [document, task = std::move(task), inspection](ModelJob& job) {
    const AttachedJob attached(*document, job);
    if (job.control().is_cancelled()) { throw ComputationCancelled(); }
    *inspection = task(*document);
    // Some direct OCCT measurements cannot interrupt midway. Discard
    // their result if the user cancelled or changed the inspection.
    if (job.control().is_cancelled()) { throw ComputationCancelled(); }
  }, result, context, [inspection, completion = std::move(completion)](const AsyncResult& outcome) {
    if (completion) { completion(*inspection, outcome); }
  });
}

bool DocumentSession::startAsync(const QString& label, const QString& stage, Job job,
                                 std::shared_ptr<AsyncResult> result, QObject* context, Completion completion) {
  if (busy()) {
    return false;
  }
  m_async = std::make_unique<AsyncJob>();
  m_async->label = label;
  m_async->result = std::move(result);
  m_async->context = context;
  m_async->completion = std::move(completion);
  m_async->job.setStage(stage);
  if (const int delay = testRecomputeDelay(label); delay > 0) {
    m_async->job.control().set_test_delay(static_cast<std::uint32_t>(delay));
  }
  m_async->clock.start();
  m_job = &m_async->job;
  ModelJob* state = m_job;
  m_worker->post([job = std::move(job), state] { job(*state); });
  m_progressTimer.start();
  qDebug().noquote() << QStringLiteral("Computing %1 started").arg(label);
  emit busyChanged(true);
  emit jobStarted(label, stage);
  return true;
}

void DocumentSession::completeAsync() {
  if (!m_async || !m_worker->waitFor(std::chrono::milliseconds(0))) {
    return;
  }
  const std::exception_ptr error = m_worker->take();
  std::unique_ptr<AsyncJob> finished = std::move(m_async);
  m_progressTimer.stop();
  finished->result->cancelled = error && finished->job.control().is_cancelled();
  if (error && !finished->result->cancelled) {
    try {
      std::rethrow_exception(error);
    } catch (const std::exception& exception) {
      finished->result->error = errorText(exception);
    } catch (...) {
      finished->result->error = tr("An unknown computation error occurred.");
    }
  }
  m_state.lastCancelled = finished->result->cancelled;
  m_state.lastError = finished->result->error;
  if (finished->result->cancelled) {
    qDebug().noquote() << QStringLiteral("Computing %1 cancelled after %2 ms")
                             .arg(finished->label).arg(finished->clock.elapsed());
  } else {
    // The UI test driver also balances failed jobs with this completion
    // line; failure details remain in jobFinished and the result.
    qDebug().noquote() << QStringLiteral("Computing %1 done in %2 ms (%3 evaluated)")
                             .arg(finished->label).arg(finished->clock.elapsed())
                             .arg(static_cast<qulonglong>(finished->job.control().progress().evaluated));
  }
  finishJob(finished->label, finished->result->cancelled, finished->result->error);
  if (finished->context && finished->completion) {
    finished->completion(*finished->result);
  }
}

void DocumentSession::finishJob(const QString& label, bool cancelled, const QString& error) {
  m_job = nullptr;
  emit busyChanged(false);
  notifyChanged();
  emit jobFinished(label, cancelled, error);
  scheduleIdleCalls();
}

void DocumentSession::clearPreview() {
  if (busy()) {
    throw ModelBusy();
  }
  m_state.document->clear_preview();
}

void DocumentSession::markSaved() {
  if (busy()) {
    throw ModelBusy();
  }
  m_state.document->command(rustStr(compactJson({{QStringLiteral("cmd"), QStringLiteral("mark_saved")}})));
}

void DocumentSession::cancelJob() {
  if (m_job) {
    m_job->control().cancel();
  }
}

void DocumentSession::whenIdle(QObject* context, std::function<void()> call) {
  m_idleCalls.push_back({QPointer<QObject>(context), std::move(call)});
  scheduleIdleCalls();
}

void DocumentSession::scheduleIdleCalls() {
  if (!busy() && !m_idleCalls.empty() && !m_idleCallsPosted) {
    m_idleCallsPosted = true;
    QTimer::singleShot(0, this, &DocumentSession::runIdleCalls);
  }
}

void DocumentSession::runIdleCalls() {
  m_idleCallsPosted = false;
  while (!m_idleCalls.empty() && !busy()) {
    IdleCall next = std::move(m_idleCalls.front());
    m_idleCalls.pop_front();
    if (next.context) {
      next.call();
    }
  }
}

void DocumentSession::setSelection(const Selection& selection) {
  m_state.selection = selection;
  emit changed();
}

void DocumentSession::setMode(Mode mode) {
  if (m_state.mode != mode) {
    m_state.mode = mode;
    emit changed();
  }
}

void DocumentSession::notifyChanged() {
  if (!busy()) {
    m_documentInfo = query({{QStringLiteral("query"), QStringLiteral("document")}}).toObject();
  }
  emit changed();
}

} // namespace mitcad
