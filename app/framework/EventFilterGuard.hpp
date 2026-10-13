// SPDX-License-Identifier: MIT
#pragma once

#include <QObject>
#include <QPointer>

namespace mitcad {

// An event filter on an object that the filter does not own (the 3D view,
// for a dialog or an overlay), removed again when the guard goes or by
// remove(), but only while the watched object still exists. When the main
// window deletes its children, the view can go before a dialog that watches
// it, and removeEventFilter() on the destroyed view crashed at the close
// (mitcad#226). Qt itself skips the filters of destroyed objects, so a
// filter that outlives its object or an object that outlives its filter
// leaves nothing behind.
class EventFilterGuard {
public:
  // The filter: the object whose eventFilter() gets the events. A member of
  // it, typically.
  explicit EventFilterGuard(QObject* filter) : m_filter(filter) {}
  ~EventFilterGuard() { remove(); }
  EventFilterGuard(const EventFilterGuard&) = delete;
  EventFilterGuard& operator=(const EventFilterGuard&) = delete;

  // Watches the object (and stops watching the one before).
  void install(QObject* watched) {
    remove();
    m_watched = watched;
    if (watched != nullptr) {
      watched->installEventFilter(m_filter);
    }
  }
  // Stops watching, if the watched object still exists.
  void remove() {
    if (m_watched) {
      m_watched->removeEventFilter(m_filter);
    }
    m_watched = nullptr;
  }
  // Watching an object that still exists.
  bool active() const { return !m_watched.isNull(); }

private:
  QObject* m_filter;
  QPointer<QObject> m_watched;
};

} // namespace mitcad
