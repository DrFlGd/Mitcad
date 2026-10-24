// SPDX-License-Identifier: MIT
#pragma once

#include <QModelIndex>
#include <QPersistentModelIndex>
#include <QPoint>

namespace mitcad {

// A row pressed before a drag starts can disappear in an asynchronous
// refresh. Keep its model identity, which is invalidated by removal/reset,
// rather than the deleted tree item's address.
class ProjectRowDrag {
public:
  void clear() {
    m_item = QPersistentModelIndex();
    m_dragging = false;
  }
  void press(const QModelIndex& item, const QPoint& at) {
    clear();
    m_item = item;
    m_pressed = at;
  }
  QModelIndex item() const { return m_item; }
  bool dragging() const { return m_dragging && m_item.isValid(); }
  bool move(const QPoint& at, int distance) {
    if (!m_item.isValid()) {
      clear();
      return false;
    }
    if ((at - m_pressed).manhattanLength() >= distance) {
      m_dragging = true;
    }
    return dragging();
  }

private:
  QPersistentModelIndex m_item;
  QPoint m_pressed;
  bool m_dragging = false;
};

} // namespace mitcad
