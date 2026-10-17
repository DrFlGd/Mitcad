// SPDX-License-Identifier: MIT
#include "Dialogs.hpp"

#include <QDialog>
#include <QEvent>
#include <QEventLoop>
#include <QFileDialog>
#include <QInputDialog>
#include <QPointer>
#include <QProgressDialog>
#include <QTimer>
#include <QVariant>
#include <QUrl>
#include <QMenu>
#include <QWidget>
#include <QtGlobal>
#ifdef MITCAD_MACOS_NATIVE_UI
#include "../platform/NativeDialogs.hpp"
#endif

namespace mitcad {

int execDialog(QDialog* dialog) {
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive()) {
    if (auto* file = qobject_cast<QFileDialog*>(dialog)) {
      return execNativeFileDialog(file);
    }
    return execNativeDialog(dialog);
  }
#endif
  return dialog->exec();
}

void prepareFileDialog(QFileDialog* dialog) {
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive()) {
    // Qt's Cocoa helper has no selected files before show().
    // Keep those values in the widget presenter used by our AppKit panel.
    dialog->setOption(QFileDialog::DontUseNativeDialog);
  }
#else
  Q_UNUSED(dialog);
#endif
}

void showDialog(QWidget* widget) {
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive() && qobject_cast<QFileDialog*>(widget) == nullptr) {
    showNativeWidget(widget);
    return;
  }
#endif
  widget->show();
}

void waitDialogUntil(const std::function<bool()>& completed) {
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive()) {
    waitNativeUntil(completed);
    return;
  }
#endif
  QEventLoop loop;
  QTimer timer;
  timer.setInterval(25);
  QObject::connect(&timer, &QTimer::timeout, &loop, [&] { if (completed()) { loop.quit(); } });
  timer.start();
  if (!completed()) { loop.exec(); }
}

#ifdef MITCAD_MACOS_NATIVE_UI
namespace {
class NativeProgressPresentation final : public QObject {
public:
  explicit NativeProgressPresentation(QProgressDialog* source) : QObject(source), m_source(source) {
    source->setProperty("nativePresentAsSheet", true);
    source->setAttribute(Qt::WA_DontShowOnScreen, true);
    source->setAttribute(Qt::WA_ShowWithoutActivating, true);
    source->setWindowModality(Qt::NonModal);
    source->installEventFilter(this);
  }
protected:
  bool eventFilter(QObject* watched, QEvent* event) override {
    if (watched == m_source && event->type() == QEvent::Show && !m_queued) {
      m_queued = true;
      QTimer::singleShot(0, this, [this] {
        m_queued = false;
        if (m_source != nullptr && m_source->isVisible()) { showDialog(m_source); }
      });
    }
    return QObject::eventFilter(watched, event);
  }
private:
  QPointer<QProgressDialog> m_source;
  bool m_queued = false;
};
} // namespace
#endif

void prepareProgressDialog(QProgressDialog* dialog) {
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive() && dialog != nullptr && !dialog->property("nativePresentAsSheet").toBool()) {
    new NativeProgressPresentation(dialog);
  }
#else
  Q_UNUSED(dialog);
#endif
}

QAction* execMenu(QMenu* menu, const QPoint& position) {
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive()) {
    return execNativeMenu(menu, position);
  }
#endif
  return menu->exec(position);
}

QString dialogText(QWidget* parent, const QString& title, const QString& label,
                   const QString& initial, bool* accepted) {
  QInputDialog dialog(parent);
  dialog.setWindowTitle(title);
  dialog.setLabelText(label);
  dialog.setInputMode(QInputDialog::TextInput);
  dialog.setTextValue(initial);
  prepareModal(&dialog);
  const bool ok = execDialog(&dialog) == QDialog::Accepted;
  if (accepted) {
    *accepted = ok;
  }
  return ok ? dialog.textValue() : QString();
}

QString dialogItem(QWidget* parent, const QString& title, const QString& label,
                   const QStringList& items, int current, bool* accepted) {
  QInputDialog dialog(parent);
  dialog.setWindowTitle(title);
  dialog.setLabelText(label);
  dialog.setComboBoxItems(items);
  dialog.setComboBoxEditable(false);
  if (current >= 0 && current < items.size()) {
    dialog.setTextValue(items[current]);
  }
  prepareModal(&dialog);
  const bool ok = execDialog(&dialog) == QDialog::Accepted;
  if (accepted) {
    *accepted = ok;
  }
  return ok ? dialog.textValue() : QString();
}

namespace {
QString fileNameDialog(QWidget* parent, const QString& title, const QString& directory,
                       const QString& filter, QString* selectedFilter, QFileDialog::AcceptMode mode) {
  QFileDialog dialog(parent, title, directory, filter);
  prepareFileDialog(&dialog);
  dialog.setAcceptMode(mode);
  dialog.setFileMode(mode == QFileDialog::AcceptSave ? QFileDialog::AnyFile : QFileDialog::ExistingFile);
  if (selectedFilter && !selectedFilter->isEmpty()) {
    dialog.selectNameFilter(*selectedFilter);
  }
  if (execDialog(&dialog) != QDialog::Accepted) {
    return {};
  }
  if (selectedFilter) {
    *selectedFilter = dialog.selectedNameFilter();
  }
  QList<QUrl> files;
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive()) {
    files = nativeFileDialogUrls(&dialog);
  } else
#endif
  {
    files = dialog.selectedUrls();
  }
  return files.isEmpty() ? QString() : files.first().toLocalFile();
}
} // namespace

QString dialogOpenFileName(QWidget* parent, const QString& title, const QString& directory,
                           const QString& filter, QString* selectedFilter) {
  return fileNameDialog(parent, title, directory, filter, selectedFilter, QFileDialog::AcceptOpen);
}

QString dialogSaveFileName(QWidget* parent, const QString& title, const QString& directory,
                           const QString& filter, QString* selectedFilter) {
  return fileNameDialog(parent, title, directory, filter, selectedFilter, QFileDialog::AcceptSave);
}

QString dialogChooseFile(QWidget* parent, const QString& title, const QString& directory, const QString& filter,
                         const QString& acceptLabel, const QString& rejectLabel) {
  QFileDialog dialog(parent, title, directory, filter);
  prepareFileDialog(&dialog);
  dialog.setAcceptMode(QFileDialog::AcceptOpen);
  dialog.setFileMode(QFileDialog::ExistingFile);
  dialog.setLabelText(QFileDialog::Accept, acceptLabel);
  dialog.setLabelText(QFileDialog::Reject, rejectLabel);
  if (execDialog(&dialog) != QDialog::Accepted) {
    return {};
  }
  QList<QUrl> files;
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive()) {
    files = nativeFileDialogUrls(&dialog);
  } else
#endif
  {
    files = dialog.selectedUrls();
  }
  return files.isEmpty() ? QString() : files.first().toLocalFile();
}

QString dialogDirectory(QWidget* parent, const QString& title, const QString& directory) {
  QFileDialog dialog(parent, title, directory);
  prepareFileDialog(&dialog);
  dialog.setFileMode(QFileDialog::Directory);
  dialog.setOption(QFileDialog::ShowDirsOnly);
  if (execDialog(&dialog) != QDialog::Accepted) {
    return {};
  }
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive()) {
    const auto urls = nativeFileDialogUrls(&dialog);
    return urls.isEmpty() ? QString() : urls.first().toLocalFile();
  }
#endif
  return dialog.selectedFiles().value(0);
}

QColor dialogColor(const QColor& initial, QWidget* parent, const QString& title,
                   QColorDialog::ColorDialogOptions options) {
#ifdef MITCAD_MACOS_NATIVE_UI
  if (nativeDialogsActive()) {
    return getNativeColor(initial, parent, title, options);
  }
#endif
  return QColorDialog::getColor(initial, parent, title, options);
}

void prepareModal(QDialog* dialog) {
#ifdef Q_OS_MACOS
  const QWidget* parent = dialog != nullptr && dialog->parentWidget() != nullptr ? dialog->parentWidget()->window()
                                                                                 : nullptr;
  if (parent != nullptr && parent->isVisible() && qobject_cast<const QDialog*>(parent) == nullptr) {
    dialog->setWindowModality(Qt::WindowModal);
  }
#else
  Q_UNUSED(dialog);
#endif
}

namespace {

QMessageBox::StandardButton show(QMessageBox::Icon icon, QWidget* parent, const QString& title, const QString& text,
                                 QMessageBox::StandardButtons buttons, QMessageBox::StandardButton defaultButton) {
  QMessageBox box(icon, title, text, buttons, parent);
  if (defaultButton != QMessageBox::NoButton) {
    box.setDefaultButton(defaultButton);
  }
  prepareModal(&box);
  execDialog(&box);
  return box.standardButton(box.clickedButton());
}

} // namespace

QMessageBox::StandardButton sheetWarning(QWidget* parent, const QString& title, const QString& text,
                                         QMessageBox::StandardButtons buttons,
                                         QMessageBox::StandardButton defaultButton) {
  return show(QMessageBox::Warning, parent, title, text, buttons, defaultButton);
}

QMessageBox::StandardButton sheetInformation(QWidget* parent, const QString& title, const QString& text,
                                             QMessageBox::StandardButtons buttons,
                                             QMessageBox::StandardButton defaultButton) {
  return show(QMessageBox::Information, parent, title, text, buttons, defaultButton);
}

QMessageBox::StandardButton sheetQuestion(QWidget* parent, const QString& title, const QString& text,
                                          QMessageBox::StandardButtons buttons,
                                          QMessageBox::StandardButton defaultButton) {
  return show(QMessageBox::Question, parent, title, text, buttons, defaultButton);
}

} // namespace mitcad
