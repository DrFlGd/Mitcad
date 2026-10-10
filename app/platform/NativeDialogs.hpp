// SPDX-License-Identifier: MIT
#pragma once

#include <memory>
#include <functional>
#include <QColorDialog>
#include <QPoint>
#include <QUrl>
#include <QtGlobal>

class QAction;
class QDialog;
class QFileDialog;
class QMenu;
class QWidget;

namespace mitcad {

// Optional per-cell edit permission for views whose Qt delegates are
// opened manually. An unset role uses the view's normal edit policy.
inline constexpr int NativeEditableRole = Qt::UserRole + 4096;

// Secondary AppKit views bind the existing Qt presenter widgets' data,
// signals and models. They do not embed or draw those widgets.
class NativeWidgetView {
public:
  virtual ~NativeWidgetView() = default;
  virtual void* nativeView() const = 0;
  virtual void refresh() = 0;
};

#if defined(Q_OS_MACOS) && defined(MITCAD_MACOS_NATIVE_UI)
void setNativeDialogWindow(void* nativeNSWindow);
bool nativeDialogsActive();
// Includes tracked sheets and AppKit's standalone modal alert/file panels.
bool nativeDialogModalActive();
void waitNativeUntil(const std::function<bool()>& completed);
std::unique_ptr<NativeWidgetView> createNativeWidgetView(QWidget* widget);
int execNativeDialog(QDialog* dialog);
int execNativeFileDialog(QFileDialog* dialog);
// Borrowed NSSavePanel/NSOpenPanel, valid only during native file-dialog exec.
void* nativeFileDialogPanel();
QList<QUrl> nativeFileDialogUrls(const QFileDialog* dialog);
QColor getNativeColor(const QColor& initial, QWidget* parent, const QString& title,
                     QColorDialog::ColorDialogOptions options = {});
void showNativeWidget(QWidget* widget);
QAction* execNativeMenu(QMenu* menu, const QPoint& point);
#else
inline void setNativeDialogWindow(void*) {}
inline bool nativeDialogsActive() { return false; }
inline bool nativeDialogModalActive() { return false; }
inline void* nativeFileDialogPanel() { return nullptr; }
inline std::unique_ptr<NativeWidgetView> createNativeWidgetView(QWidget*) { return {}; }
#endif

} // namespace mitcad
