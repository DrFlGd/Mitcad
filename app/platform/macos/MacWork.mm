// SPDX-License-Identifier: MIT
#include "platform/MacChrome.hpp"

#import <Foundation/Foundation.h>

#include <atomic>
#include <mutex>
#include <pthread.h>
#include <pthread/qos.h>

#include <QDebug>

#include <OSD_ThreadPool.hxx>
#include <Standard_Failure.hxx>

namespace mitcad::mac {
namespace {

qos_class_t nativeQuality(ThreadQuality quality) {
  return quality == ThreadQuality::Utility ? QOS_CLASS_UTILITY : QOS_CLASS_USER_INITIATED;
}

// The protected job interface runs once on EVERY reserved thread, unlike
// Perform's range functor, whose iterations may all run on one thread.
// No barrier or assumptions about pthread QoS inheritance are needed.
class ComputePoolSetup : public OSD_ThreadPool {
  class QualityJob : public JobInterface {
  public:
    void Perform(int) override {
      if (!pthread_equal(pthread_self(), caller)) {
        if (pthread_set_qos_class_self_np(QOS_CLASS_USER_INITIATED, 0) != 0) {
          failed = true;
        }
      }
    }
    pthread_t caller = pthread_self();
    std::atomic<bool> failed{false};
  };

  class QualityLauncher : public Launcher {
  public:
    explicit QualityLauncher(OSD_ThreadPool& pool) : Launcher(pool, pool.NbThreads()) {}
    void apply(JobInterface& job) { perform(job); }
  };

public:
  static void initialize() {
    QualityJob job;
    QualityLauncher launcher(*DefaultPool());
    launcher.apply(job);
    if (job.failed) {
      qWarning() << "Could not set a compute pool thread's quality of service";
    }
  }
};

} // namespace

void setThreadQuality(ThreadQuality quality) {
  if (pthread_set_qos_class_self_np(nativeQuality(quality), 0) != 0) {
    qWarning() << "Could not set the thread's quality of service";
  }
}

ThreadQualityScope::ThreadQualityScope(ThreadQuality quality) {
  qos_class_t previous = QOS_CLASS_UNSPECIFIED;
  if (pthread_get_qos_class_np(pthread_self(), &previous, &m_priority) != 0) {
    return;
  }
  // Unclassified pthreads use the default class; UNSPECIFIED itself cannot
  // be passed to pthread_set_qos_class_self_np.
  m_previous = previous == QOS_CLASS_UNSPECIFIED ? QOS_CLASS_DEFAULT : previous;
  if (previous == QOS_CLASS_UNSPECIFIED) {
    m_priority = 0;
  }
  m_changed = pthread_set_qos_class_self_np(nativeQuality(quality), 0) == 0;
}

ThreadQualityScope::~ThreadQualityScope() {
  if (m_changed) {
    if (pthread_set_qos_class_self_np(static_cast<qos_class_t>(m_previous), m_priority) != 0) {
      qWarning() << "Could not restore the thread's quality of service";
    }
  }
}

void initializeComputeThreads() {
  static std::once_flag once;
  std::call_once(once, [] {
    try {
      ComputePoolSetup::initialize();
    } catch (const Standard_Failure& failure) {
      qWarning() << "Could not initialize compute thread quality:" << failure.what();
    }
  });
}

void ProcessActivity::start(const char* reason) {
  stop();
  @autoreleasepool {
    // Suppress App Nap without blocking the user's normal idle sleep.
    // LatencyCritical requests high-priority I/O (audio/video recording),
    // which a render session does not need.
    id token = [[NSProcessInfo processInfo] beginActivityWithOptions:NSActivityUserInitiatedAllowingIdleSystemSleep
                                                            reason:[NSString stringWithUTF8String:reason]];
    m_token = (__bridge_retained void*)token;
  }
}

void ProcessActivity::stop() {
  if (m_token != nullptr) {
    @autoreleasepool {
      id token = (__bridge_transfer id)m_token;
      m_token = nullptr;
      [[NSProcessInfo processInfo] endActivity:token];
    }
  }
}

} // namespace mitcad::mac
