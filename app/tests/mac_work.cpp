// SPDX-License-Identifier: MIT
// Native scheduling checks without windows; App Nap performance is a
// separate physical foreground/background acceptance check.
#include <atomic>
#include <chrono>
#include <cstdio>
#include <future>
#include <stdexcept>
#include <thread>

#include <pthread.h>
#include <pthread/qos.h>

#include <QCoreApplication>

#include <OSD_ThreadPool.hxx>

#include "framework/ModelWorker.hpp"
#include "framework/SectionWorker.hpp"
#include "platform/MacChrome.hpp"

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "mac_work.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

qos_class_t quality() {
  qos_class_t value = QOS_CLASS_UNSPECIFIED;
  if (pthread_get_qos_class_np(pthread_self(), &value, nullptr) != 0) {
    return QOS_CLASS_UNSPECIFIED;
  }
  return value;
}

// Visit every reserved thread, rather than assuming a range functor visits
// more than one. This observes the class set on the actual OCCT threads.
class PoolProbe : public OSD_ThreadPool {
  class Probe : public JobInterface {
  public:
    void Perform(int) override {
      if (!pthread_equal(pthread_self(), caller)) {
        ++visited;
        if (quality() != QOS_CLASS_USER_INITIATED) {
          ++wrong;
        }
      }
    }
    pthread_t caller = pthread_self();
    std::atomic<int> visited{0};
    std::atomic<int> wrong{0};
  };

  class ProbeLauncher : public Launcher {
  public:
    explicit ProbeLauncher(OSD_ThreadPool& pool) : Launcher(pool, pool.NbThreads()) {}
    void apply(JobInterface& job) { perform(job); }
  };

public:
  static void checkPool() {
    const auto& pool = DefaultPool();
    const int threads = pool->NbThreads();
    Probe probe;
    ProbeLauncher launcher(*pool);
    launcher.apply(probe);
    CHECK(probe.visited == threads - 1);
    CHECK(probe.wrong == 0);
  }
};

} // namespace

int main(int argc, char* argv[]) {
  QCoreApplication app(argc, argv);
  using namespace mitcad;
  using namespace std::chrono_literals;

  const qos_class_t caller = quality();
  const int threads = OSD_ThreadPool::DefaultPool()->NbThreads();
  mac::initializeComputeThreads();
  mac::initializeComputeThreads(); // repeat calls neither resize nor reset the caller
  CHECK(quality() == caller);
  CHECK(OSD_ThreadPool::DefaultPool()->NbThreads() == threads);
  PoolProbe::checkPool();

  // A utility job on a shared thread returns it to its original class,
  // including a nested user job and an exception unwinding both guards.
  std::atomic<bool> restored{false};
  std::atomic<bool> nested{false};
  std::atomic<bool> utility{false};
  std::thread scoped([&] {
    mac::setThreadQuality(mac::ThreadQuality::UserInitiated);
    try {
      const mac::ThreadQualityScope low(mac::ThreadQuality::Utility);
      utility = quality() == QOS_CLASS_UTILITY;
      {
        const mac::ThreadQualityScope high(mac::ThreadQuality::UserInitiated);
        nested = quality() == QOS_CLASS_USER_INITIATED;
      }
      utility = utility && quality() == QOS_CLASS_UTILITY;
      throw std::runtime_error("a failed background job");
    } catch (const std::runtime_error&) {
      restored = quality() == QOS_CLASS_USER_INITIATED;
    }
  });
  scoped.join();
  CHECK(utility);
  CHECK(nested);
  CHECK(restored);

  // The actual long-lived model worker keeps its class after a failed job.
  {
    bool reject = true;
    int attempts = 0;
    ModelWorker worker(nullptr, [&](QThread& thread) {
      ++attempts;
      if (!reject) { thread.start(); }
    });
    std::atomic<bool> ran{false};
    bool refused = false;
    try {
      worker.post([&] { ran = true; });
    } catch (const std::runtime_error& error) {
      refused = QString::fromUtf8(error.what()).contains(QStringLiteral("could not start"));
    }
    CHECK(refused && !ran && !worker.isRunning() && attempts > 1);
    reject = false;
    worker.post([&] { ran = true; });
    const bool done = worker.waitFor(5s);
    CHECK(done);
    if (done) {
      CHECK(worker.take() == nullptr);
      CHECK(ran);
    }
  }
  {
    ModelWorker worker;
    worker.start();
    std::atomic<bool> first{false};
    worker.post([&] {
      first = quality() == QOS_CLASS_USER_INITIATED;
      throw std::runtime_error("a failed model job");
    });
    const bool done = worker.waitFor(5s);
    CHECK(done);
    if (done) {
      CHECK(worker.take() != nullptr);
      CHECK(first);
      std::atomic<bool> second{false};
      worker.post([&] { second = quality() == QOS_CLASS_USER_INITIATED; });
      const bool nextDone = worker.waitFor(5s);
      CHECK(nextDone);
      if (nextDone) {
        CHECK(worker.take() == nullptr);
        CHECK(second);
      }
    }
  }
  {
    SectionWorker worker;
    std::promise<bool> reported;
    auto result = reported.get_future();
    worker.start(SectionWorker::Kind::View, QStringLiteral("quality"),
                 [&reported](const geometry::CancelSource&) {
                   reported.set_value(quality() == QOS_CLASS_USER_INITIATED);
                 }, {});
    const bool done = result.wait_for(5s) == std::future_status::ready;
    CHECK(done);
    if (done) {
      CHECK(result.get());
    }
    worker.stop();
  }
  return failures == 0 ? 0 : 1;
}
