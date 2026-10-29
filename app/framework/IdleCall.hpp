// SPDX-License-Identifier: MIT
#pragma once

#include <functional>
#include <utility>

#include <QObject>
#include <QPointer>

#include "TestSync.hpp"

namespace mitcad {

// A call after the current event, for what reads or changes the model: as
// TestSync::singleShot(0) (the UI tests' sync waits for it), but never
// while a job computes the model. A job's wait runs the timers (a
// synchronous command's event loop, an asynchronous preview's), so a call
// due then would read the document the worker owns (docs/architecture.md,
// "Background computation"): it waits for the job's end instead
// (whenIdle, mitcad#888). `Host` is the window's CommandHost or
// DocumentHost (modelBusy, whenIdle), which outlives `context`.
template <typename Host>
void afterEventWhenIdle(Host& host, QObject* context, std::function<void()> call) {
  TestSync::singleShot(0, context, [&host, context = QPointer<QObject>(context), call = std::move(call)] {
    if (!context) {
      return;
    }
    if (host.modelBusy()) {
      host.whenIdle(context, call);
    } else {
      call();
    }
  });
}

} // namespace mitcad
