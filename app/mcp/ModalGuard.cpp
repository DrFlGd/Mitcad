// SPDX-License-Identifier: MIT
#include "ModalGuard.hpp"

#include <QApplication>
#include <QDebug>
#include <QDialog>
#include <QEvent>
#include <QMetaObject>
#include <QPointer>
#include <QWidget>
#include <QtLogging>

namespace mitcad::mcp {

ModalGuard::ModalGuard() {
  qApp->installEventFilter(this);
}

ModalGuard::~ModalGuard() {
  qApp->removeEventFilter(this);
}

bool ModalGuard::eventFilter(QObject* watched, QEvent* event) {
  if (event->type() != QEvent::Show || !watched->isWidgetType()) {
    return false;
  }
  auto* widget = static_cast<QWidget*>(watched);
  if (!widget->isWindow() || !widget->isModal() || widget->objectName() == QStringLiteral("computeProgress")) {
    return false;
  }
  const QString name = widget->windowTitle().isEmpty() ? QString::fromLatin1(widget->metaObject()->className())
                                                        : widget->windowTitle();
  if (m_opened.isEmpty()) {
    m_opened = name;
  }
  qInfo().noquote() << QStringLiteral("MCP: closed the dialog %1 (an agent cannot answer it)").arg(name);
  // In the dialog's own event loop, once it has shown.
  const QPointer<QWidget> guarded(widget);
  QMetaObject::invokeMethod(
      widget,
      [guarded] {
        if (!guarded) {
          return;
        }
        if (auto* dialog = qobject_cast<QDialog*>(guarded.data())) {
          dialog->reject();
        } else {
          guarded->close();
        }
      },
      Qt::QueuedConnection);
  return false;
}

} // namespace mitcad::mcp
