// SPDX-License-Identifier: MIT
#include "SnapshotDialog.hpp"

#include <algorithm>
#include <cmath>
#include <utility>

#include <QCheckBox>
#include <QComboBox>
#include <QCoreApplication>
#include <QDialog>
#include <QDialogButtonBox>
#include <QEvent>
#include <QFileInfo>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPushButton>
#include <QSettings>
#include <QSpinBox>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/Dialogs.hpp"
#include "../framework/TestSync.hpp"
#include "../framework/Theme.hpp"
#include "../platform/MacDialogs.hpp"

namespace mitcad {
namespace {

const QString kGroup = QStringLiteral("snapshot/");
// The largest side of a picture: what graphics cards render offscreen in
// one piece or a few, and a few hundred megabytes of memory at most.
constexpr int kLargest = 16384;

QString onOff(bool on) { return on ? QStringLiteral("on") : QStringLiteral("off"); }

} // namespace

QSize snapshotSize(const QString& sizeKey, int width, const QSize& view) {
  if (view.width() <= 0 || view.height() <= 0) {
    return QSize();
  }
  double scale = 2.0;
  if (sizeKey == QLatin1String("4x")) {
    scale = 4.0;
  } else if (sizeKey == QLatin1String("width")) {
    scale = static_cast<double>(std::clamp(width, 16, kLargest)) / view.width();
  }
  // No side over the largest: the view's aspect kept.
  scale = std::min(scale, static_cast<double>(kLargest) / std::max(view.width(), view.height()));
  return QSize(std::max(1, static_cast<int>(std::lround(view.width() * scale))),
               std::max(1, static_cast<int>(std::lround(view.height() * scale))));
}

std::optional<SnapshotChoice> askSnapshot(QWidget* parent, const QString& path, const QSize& view,
                                          int measurements, const QList<QPair<QString, int>>& groups) {
  QSettings store;
  QDialog dialog(parent);
  dialog.setWindowTitle(QObject::tr("Export Snapshot"));
  auto* layout = new QVBoxLayout(&dialog);
  auto* form = new QFormLayout;
  layout->addLayout(form);

  auto* file = new QLineEdit(path);
  auto* browse = new QPushButton(QObject::tr("Browse…"));
  mac::auxiliaryButton(browse);
  auto* fileRow = new QHBoxLayout;
  fileRow->addWidget(file, 1);
  fileRow->addWidget(browse);
  form->addRow(QObject::tr("&File:"), fileRow);

  auto* size = new QComboBox;
  const QSize twice = snapshotSize(QStringLiteral("2x"), 0, view);
  const QSize four = snapshotSize(QStringLiteral("4x"), 0, view);
  size->addItem(QObject::tr("2 × the view (%1 × %2)").arg(twice.width()).arg(twice.height()), QStringLiteral("2x"));
  size->addItem(QObject::tr("4 × the view (%1 × %2)").arg(four.width()).arg(four.height()), QStringLiteral("4x"));
  size->addItem(QObject::tr("Width in pixels"), QStringLiteral("width"));
  size->setCurrentIndex(std::max(0, size->findData(store.value(kGroup + QStringLiteral("size"), QStringLiteral("2x")))));
  form->addRow(QObject::tr("Si&ze:"), size);
  auto* width = new QSpinBox;
  width->setRange(16, kLargest);
  width->setSuffix(QObject::tr(" px"));
  width->setValue(store.value(kGroup + QStringLiteral("width"), 3840).toInt());
  auto* height = new QLabel;
  auto* widthRow = new QHBoxLayout;
  widthRow->addWidget(width);
  widthRow->addWidget(height, 1);
  form->addRow(QObject::tr("&Width:"), widthRow);
  auto* transparent = new QCheckBox(QObject::tr("&Transparent background"));
  transparent->setToolTip(QObject::tr("Only the bodies and the annotations: the background left out (PNG)"));
  transparent->setChecked(store.value(kGroup + QStringLiteral("transparent"), false).toBool());
  form->addRow(QString(), transparent);
  // The annotations drawn over it (mitcad#702): every shown one, none, or
  // one group's shown ones.
  auto* shown = new QComboBox;
  shown->addItem(measurements > 0 ? QObject::tr("All shown (%1)").arg(measurements) : QObject::tr("All shown (none)"),
                 QStringLiteral("all"));
  shown->addItem(QObject::tr("None"), QStringLiteral("none"));
  for (const auto& [group, count] : groups) {
    shown->addItem(QObject::tr("Group %1 (%2 shown)").arg(group).arg(count), QStringLiteral("group:") + group);
  }
  shown->setCurrentIndex(
      store.value(kGroup + QStringLiteral("measurements"), true).toBool() ? 0 : shown->findData(QStringLiteral("none")));
  form->addRow(QObject::tr("&Annotations:"), shown);
  const auto annotationsKey = [shown] {
    const QString key = shown->currentData().toString();
    return key.startsWith(QLatin1String("group:")) ? QStringLiteral("group ") + key.mid(6) : key;
  };

  auto* message = new QLabel;
  setErrorStyleSheet(message);
  layout->addWidget(message);
  auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
  QObject::connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
  QObject::connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
  layout->addWidget(buttons);

  const auto chosen = [size, width, view] {
    return snapshotSize(size->currentData().toString(), width->value(), view);
  };
  const auto follow = [size, width, height, chosen] {
    const bool byWidth = size->currentData().toString() == QLatin1String("width");
    width->setEnabled(byWidth);
    const QSize pixels = chosen();
    height->setText(QObject::tr("%1 × %2 pixels").arg(pixels.width()).arg(pixels.height()));
  };
  follow();
  QObject::connect(size, &QComboBox::currentIndexChanged, &dialog, [size, follow] {
    follow();
    qDebug().noquote() << QStringLiteral("Snapshot dialog: size = %1").arg(size->currentData().toString());
  });
  QObject::connect(width, &QSpinBox::valueChanged, &dialog, [follow] { follow(); });
  QObject::connect(transparent, &QCheckBox::toggled, &dialog, [](bool on) {
    qDebug().noquote() << QStringLiteral("Snapshot dialog: transparent = %1").arg(onOff(on));
  });
  QObject::connect(shown, &QComboBox::currentIndexChanged, &dialog, [annotationsKey] {
    qDebug().noquote() << QStringLiteral("Snapshot dialog: annotations = %1").arg(annotationsKey());
  });
  QObject::connect(browse, &QPushButton::clicked, &dialog, [&dialog, file] {
    const QString picked = dialogSaveFileName(&dialog, QObject::tr("Export Snapshot"), file->text(),
                                              QObject::tr("Images (*.png *.jpg)"));
    if (!picked.isEmpty()) {
      file->setText(picked);
    }
  });
  file->setFocus();
  file->selectAll();
  dialog.resize(520, dialog.sizeHint().height());
  qDebug().noquote() << QStringLiteral("Snapshot dialog opened: size %1, transparent %2, annotations %3 (%4 shown)")
                            .arg(size->currentData().toString(), onOff(transparent->isChecked()), annotationsKey())
                            .arg(measurements);
  // Where the inputs are, in the dialog's own coordinates (UI tests post
  // their clicks to the dialog's window).
  TestSync::singleShot(200, &dialog, [&dialog, file, size, width, transparent, shown, buttons] {
    QCoreApplication::sendPostedEvents(nullptr, QEvent::LayoutRequest);
    const auto at = [&dialog](const QWidget* widget) {
      const QPoint p = widget->mapTo(&dialog, widget->rect().center());
      return QStringLiteral("%1,%2").arg(p.x()).arg(p.y());
    };
    const std::pair<const char*, QWidget*> places[] = {
        {"file", file}, {"size", size}, {"width", width}, {"transparent", transparent}, {"annotations", shown}};
    for (const auto& [name, widget] : places) {
      qDebug().noquote() << QStringLiteral("Snapshot dialog %1 at %2").arg(QLatin1String(name), at(widget));
    }
    if (QPushButton* ok = buttons->button(QDialogButtonBox::Ok)) {
      qDebug().noquote() << QStringLiteral("Snapshot dialog OK at %1").arg(at(ok));
    }
  });

  prepareModal(&dialog);
  while (execDialog(&dialog) == QDialog::Accepted) {
    SnapshotChoice choice;
    choice.path = file->text().trimmed();
    choice.sizeKey = size->currentData().toString();
    choice.size = chosen();
    choice.transparent = transparent->isChecked();
    const QString annotations = shown->currentData().toString();
    choice.measurements = annotations != QLatin1String("none");
    if (annotations.startsWith(QLatin1String("group:"))) {
      choice.group = annotations.mid(6);
    }
    if (choice.path.isEmpty()) {
      message->setText(QObject::tr("Give the file to write."));
      continue;
    }
    const QString suffix = QFileInfo(choice.path).suffix().toLower();
    if (suffix.isEmpty()) {
      choice.path += QStringLiteral(".png");
    } else if (suffix != QLatin1String("png") && suffix != QLatin1String("jpg") &&
               suffix != QLatin1String("jpeg")) {
      message->setText(QObject::tr("A snapshot is a PNG (.png) or JPEG (.jpg) image."));
      continue;
    }
    if (choice.transparent && QFileInfo(choice.path).suffix().toLower() != QLatin1String("png")) {
      message->setText(QObject::tr("A transparent background needs a PNG image."));
      continue;
    }
    if (!choice.size.isValid()) {
      message->setText(QObject::tr("The view has no size."));
      continue;
    }
    if (QFileInfo::exists(choice.path) &&
        sheetQuestion(&dialog, QObject::tr("Export Snapshot"),
                      QObject::tr("%1 exists. Replace it?").arg(QFileInfo(choice.path).fileName())) !=
            QMessageBox::Yes) {
      continue;
    }
    store.setValue(kGroup + QStringLiteral("size"), choice.sizeKey);
    store.setValue(kGroup + QStringLiteral("width"), width->value());
    store.setValue(kGroup + QStringLiteral("transparent"), choice.transparent);
    store.setValue(kGroup + QStringLiteral("measurements"), choice.measurements);
    qDebug().noquote() << QStringLiteral("Snapshot dialog: %1 %2 x %3 to %4")
                              .arg(choice.sizeKey)
                              .arg(choice.size.width())
                              .arg(choice.size.height())
                              .arg(choice.path);
    return choice;
  }
  return std::nullopt;
}

} // namespace mitcad
