// SPDX-License-Identifier: MIT
#include "TexturePreferences.hpp"

#include <algorithm>

#include <QCheckBox>
#include <QDir>
#include <QFileInfo>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QPushButton>
#include <QTimer>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/Dialogs.hpp"
#include "../platform/MacDialogs.hpp"

namespace mitcad {

TexturePreferencesBox::TexturePreferencesBox(QWidget* parent)
#ifdef Q_OS_MACOS
    : QWidget(parent),
#else
    : QGroupBox(tr("Textures"), parent),
#endif
      m_settings(TextureSettings::load()) {
  auto* layout = new QVBoxLayout(this);
  auto* intro = new QLabel(tr("Edit Appearances searches these folders (and their subfolders) for texture maps: "
                              "PNG or JPEG images named by their maps, such as oak_diff.png, oak_nor_gl.png, "
                              "oak_rough.png, oak_metal.png, oak_ao.png. A texture used in a design is embedded "
                              "in it."));
  intro->setWordWrap(true);
  layout->addWidget(intro);
  m_folders = new QListWidget;
  m_folders->setObjectName(QStringLiteral("textureFolders"));
  m_folders->setSelectionMode(QAbstractItemView::ExtendedSelection);
  m_folders->setMinimumHeight(90);
  for (const QString& folder : std::as_const(m_settings.folders)) {
    m_folders->addItem(QDir::toNativeSeparators(folder));
  }
  layout->addWidget(m_folders);
  mac::stylePreview(m_folders);
  auto* row = new QHBoxLayout;
  m_typed = new QLineEdit;
  m_typed->setObjectName(QStringLiteral("textureFolder"));
  m_typed->setPlaceholderText(tr("A folder of textures"));
  m_typed->setToolTip(tr("Add, or OK, adds the folder typed here"));
  auto* add = new QPushButton(tr("Add"));
  add->setToolTip(tr("Adds the typed folder"));
  auto* choose = new QPushButton(tr("Choose…"));
  choose->setToolTip(tr("Chooses a folder to add"));
  m_remove = new QPushButton(tr("Remove"));
  m_remove->setToolTip(tr("Removes the selected folders from the list (the files stay)"));
  m_remove->setEnabled(false);
  for (QPushButton* button : {add, choose, m_remove}) {
    button->setAutoDefault(false);
  }
#ifdef Q_OS_MACOS
  auto* controls = new QHBoxLayout;
  controls->setSpacing(0);
  controls->addWidget(add);
  controls->addWidget(m_remove);
  controls->addStretch(1);
  layout->addLayout(controls);
#else
  row->addWidget(add);
#endif
  row->insertWidget(0, m_typed, 1);
  row->addWidget(choose);
#ifndef Q_OS_MACOS
  row->addWidget(m_remove);
#endif
  mac::styleListButtons(add, m_remove);
  layout->addLayout(row);
  const bool network = std::any_of(textureSourceRegistry().cbegin(), textureSourceRegistry().cend(),
                                   [](const TextureSourceEntry& entry) { return entry.usesNetwork; });
  if (network) {
    m_network = new QCheckBox(tr("Allow texture sources on the &internet"));
    m_network->setChecked(m_settings.allowNetwork);
    m_network->setToolTip(tr("Online texture libraries are searched only when this is on"));
    layout->addWidget(m_network);
    connect(m_network, &QCheckBox::toggled, this, &TexturePreferencesBox::changed);
  }

  connect(add, &QPushButton::clicked, this, [this] {
    addFolder(m_typed->text());
    m_typed->clear();
  });
  connect(choose, &QPushButton::clicked, this, [this] {
    const QString folder = dialogDirectory(this, tr("Texture Folder"));
    if (!folder.isEmpty()) {
      addFolder(folder);
    }
  });
  connect(m_folders, &QListWidget::itemSelectionChanged, this,
          [this] { m_remove->setEnabled(!m_folders->selectedItems().isEmpty()); });
  connect(m_remove, &QPushButton::clicked, this, [this] {
    qDeleteAll(m_folders->selectedItems());
    emit changed();
  });
}

void TexturePreferencesBox::showEvent(QShowEvent* event) {
  TexturePreferencesBase::showEvent(event);
  // Where the folder field is, in the main window's coordinates (UI tests).
  QTimer::singleShot(0, this, [this] {
    QWidget* main = window()->parentWidget() != nullptr ? window()->parentWidget()->window() : nullptr;
    if (main != nullptr && m_typed->isVisible()) {
      const QPoint at = m_typed->mapToGlobal(m_typed->rect().center()) - main->mapToGlobal(QPoint(0, 0));
      qDebug().noquote() << QStringLiteral("Preferences texture folder at %1,%2").arg(at.x()).arg(at.y());
    }
  });
}

void TexturePreferencesBox::addFolder(const QString& typed) {
  if (typed.trimmed().isEmpty()) {
    return;
  }
  const QString folder = QDir::cleanPath(QFileInfo(QDir::fromNativeSeparators(typed.trimmed())).absoluteFilePath());
  for (int i = 0; i < m_folders->count(); ++i) {
    if (QDir::fromNativeSeparators(m_folders->item(i)->text()) == folder) {
      return;
    }
  }
  auto* item = new QListWidgetItem(QDir::toNativeSeparators(folder), m_folders);
  if (!QFileInfo(folder).isDir()) {
    item->setToolTip(tr("The folder is not there now"));
  }
  emit changed();
}

TextureSettings TexturePreferencesBox::chosen() const {
  // The browser's resolution as it is now (Edit Appearances keeps it).
  TextureSettings settings = TextureSettings::load();
  settings.folders.clear();
  for (int i = 0; i < m_folders->count(); ++i) {
    settings.folders << QDir::fromNativeSeparators(m_folders->item(i)->text());
  }
  // A folder typed and not added yet counts too (OK, or the settings
  // window closed).
  const QString typed = m_typed->text().trimmed();
  if (!typed.isEmpty()) {
    const QString folder = QDir::cleanPath(QFileInfo(QDir::fromNativeSeparators(typed)).absoluteFilePath());
    if (!settings.folders.contains(folder)) {
      settings.folders << folder;
    }
  }
  settings.allowNetwork = m_network != nullptr ? m_network->isChecked() : m_settings.allowNetwork;
  return settings;
}

} // namespace mitcad
