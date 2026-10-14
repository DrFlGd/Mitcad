// SPDX-License-Identifier: MIT
#pragma once

#include <QEvent>
#include <QObject>
#include <QPointer>
#include <QWidget>

#include <functional>
#include <utility>

#include "TestSync.hpp"

namespace mitcad {

// Calls a function (once, a moment later) when a widget's place in its window
// may have changed: when it or one of the widgets it sits in moves, is
// resized, shown or put in another parent. A panel that logs where its parts
// are for the UI tests (they click at the logged places) uses it so that the
// places are logged again when something else makes room above or beside it
// (a banner, a docked panel), not only when its own content changes. The
// function is expected to log only what changed. Owned by the widget.
class PlaceWatcher : public QObject {
public:
  PlaceWatcher(QWidget* widget, std::function<void()> changed)
      : QObject(widget), m_widget(widget), m_changed(std::move(changed)) {
    attach();
  }

protected:
  bool eventFilter(QObject* watched, QEvent* event) override {
    switch (event->type()) {
    case QEvent::ParentChange:
      attach(); // another chain of widgets to watch
      schedule();
      break;
    case QEvent::Move:
    case QEvent::Resize:
    case QEvent::Show:
      schedule();
      break;
    default:
      break;
    }
    return QObject::eventFilter(watched, event);
  }

private:
  // The widget and the widgets above it. A filter installed again on the
  // same object is moved to the front, not added twice.
  void attach() {
    for (QWidget* widget = m_widget; widget != nullptr; widget = widget->parentWidget()) {
      widget->installEventFilter(this);
    }
  }

  void schedule() {
    if (m_pending) {
      return;
    }
    m_pending = true;
    // Watched: a UI test's sync waits for the log of the new places.
    TestSync::singleShot(0, this, [this] {
      m_pending = false;
      if (m_widget != nullptr && m_widget->isVisible()) {
        m_changed();
      }
    });
  }

  QPointer<QWidget> m_widget;
  std::function<void()> m_changed;
  bool m_pending = false;
};

} // namespace mitcad
