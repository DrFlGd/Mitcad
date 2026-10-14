// SPDX-License-Identifier: MIT
#include "WaitCursor.hpp"

#include <QApplication>
#include <QCursor>
#include <QGuiApplication>
#include <QtLogging>

namespace mitcad {
namespace {

// The GUI thread's state.
int g_depth = 0;
QString g_last;            // the latest wait's job
bool g_checkPending = false; // a wait ended: the next idle check says what it found

// More than any real stack: a runaway loop stops here.
constexpr int kMostCleared = 64;

} // namespace

WaitCursor::WaitCursor(const QString& label) : m_label(label) {
  QApplication::setOverrideCursor(Qt::WaitCursor);
  m_held = true;
  m_depth = ++g_depth;
  g_last = label;
  qDebug().noquote() << QStringLiteral("Wait cursor on: %1 (depth %2)").arg(m_label).arg(m_depth);
}

WaitCursor::~WaitCursor() { release(); }

void WaitCursor::release() {
  if (!m_held) {
    return;
  }
  m_held = false;
  QApplication::restoreOverrideCursor();
  qDebug().noquote() << QStringLiteral("Wait cursor off: %1 (depth %2)").arg(m_label).arg(m_depth);
  --g_depth;
  if (g_depth <= 0) {
    g_depth = 0;
    g_checkPending = true;
  }
}

int WaitCursor::depth() { return g_depth; }

int WaitCursor::recoverIfIdle() {
  if (g_depth > 0 || QApplication::activeModalWidget() != nullptr || QApplication::activePopupWidget() != nullptr ||
      QGuiApplication::mouseButtons() != Qt::NoButton) {
    return 0;
  }
  int cleared = 0;
  while (QGuiApplication::overrideCursor() != nullptr && cleared < kMostCleared) {
    QGuiApplication::restoreOverrideCursor();
    ++cleared;
  }
  if (cleared > 0) {
    const QString job = g_last.isEmpty() ? QStringLiteral("an unknown wait") : g_last;
    qWarning().noquote() << QStringLiteral("Wait cursor left on after %1: cleared%2")
                                .arg(job, cleared > 1 ? QStringLiteral(" (%1 override cursors)").arg(cleared)
                                                      : QString());
    g_checkPending = true;
  }
  if (g_checkPending) {
    g_checkPending = false;
    qDebug().noquote() << QStringLiteral("Wait cursor at idle: none set");
  }
  return cleared;
}

bool WaitCursor::testHooks() {
  static const bool hooks = qEnvironmentVariableIntValue("MITCAD_TEST_WAIT_CURSOR") != 0;
  return hooks;
}

void WaitCursor::leaveOnForTest(const QString& label) {
  QApplication::setOverrideCursor(Qt::WaitCursor);
  g_last = label;
  qInfo().noquote() << QStringLiteral("Wait cursor left on for a test: %1").arg(label);
}

} // namespace mitcad
