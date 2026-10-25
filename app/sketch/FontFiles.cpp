// SPDX-License-Identifier: MIT
#include "FontFiles.hpp"

#include <QDir>
#include <QDirIterator>
#include <QFileInfo>
#include <QFont>
#include <QFontDatabase>
#include <QHBoxLayout>
#include <QHash>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QPainter>
#include <QPushButton>
#include <QSettings>
#include <QTimer>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/Dialogs.hpp"
#include "../platform/MacDialogs.hpp"

namespace mitcad::sketch {

namespace {

const QString kChoicePrefix = QStringLiteral("file:");

QString cleanFolder(const QString& typed) {
  return QDir::cleanPath(QFileInfo(QDir::fromNativeSeparators(typed.trimmed())).absoluteFilePath());
}

} // namespace

QStringList savedFontFolders() {
  QStringList folders;
  for (const QString& folder : QSettings().value(QStringLiteral("fonts/folders")).toStringList()) {
    const QString clean = QDir::cleanPath(QDir::fromNativeSeparators(folder.trimmed()));
    if (!folder.trimmed().isEmpty() && !folders.contains(clean)) {
      folders << clean;
    }
  }
  return folders;
}

void saveFontFolders(const QStringList& folders) {
  QSettings().setValue(QStringLiteral("fonts/folders"), folders);
  forgetFolderFonts();
}

QStringList fontFilesIn(const QString& folder) {
  QStringList files;
  QDirIterator it(folder,
                  {QStringLiteral("*.ttf"), QStringLiteral("*.otf"), QStringLiteral("*.ttc"), QStringLiteral("*.woff"),
                   QStringLiteral("*.woff2")},
                  QDir::Files, QDirIterator::Subdirectories);
  while (it.hasNext()) {
    files << QDir::cleanPath(it.next());
  }
  files.sort(Qt::CaseInsensitive);
  return files;
}

const QStringList& installedFontFamilies() {
  static const QStringList families = [] {
    QStringList all;
    for (const QString& family : QFontDatabase::families()) {
      if (!QFontDatabase::isPrivateFamily(family)) {
        all << family;
      }
    }
    return all;
  }();
  return families;
}

QString fontFileFamily(const QString& path) {
  installedFontFamilies(); // before the file's family joins them
  static QHash<QString, QString> families;
  const auto found = families.constFind(path);
  if (found != families.constEnd()) {
    return *found;
  }
  QString family;
  const int id = QFontDatabase::addApplicationFont(path);
  if (id >= 0) {
    const QStringList names = QFontDatabase::applicationFontFamilies(id);
    if (!names.isEmpty()) {
      family = names.first();
    }
  }
  families.insert(path, family);
  return family;
}

QVector<FolderFont> folderFonts(const QStringList& folders) {
  QVector<FolderFont> fonts;
  for (const QString& folder : folders) {
    for (const QString& path : fontFilesIn(folder)) {
      const QString family = fontFileFamily(path);
      fonts.append({path, family.isEmpty() ? QFileInfo(path).completeBaseName() : family});
    }
  }
  return fonts;
}

namespace {

struct FolderFontCache {
  bool found = false;
  QVector<FolderFont> fonts;
};

FolderFontCache& folderFontCache() {
  static FolderFontCache cache;
  return cache;
}

} // namespace

const QVector<FolderFont>& savedFolderFonts() {
  FolderFontCache& cache = folderFontCache();
  if (!cache.found) {
    cache.fonts = folderFonts(savedFontFolders());
    cache.found = true;
  }
  return cache.fonts;
}

void forgetFolderFonts() { folderFontCache() = FolderFontCache(); }

QString findFontFile(const QString& path, const QStringList& folders) {
  if (path.isEmpty() || QFileInfo(path).isFile()) {
    return path;
  }
  const QString name = QFileInfo(QDir::fromNativeSeparators(path)).fileName();
  for (const QString& folder : folders) {
    for (const QString& file : fontFilesIn(folder)) {
      if (QFileInfo(file).fileName() == name) {
        return file;
      }
    }
  }
  return path;
}

QString fontFileChoice(const QString& path) { return kChoicePrefix + path; }

QString fontFileOfChoice(const QString& choice) {
  return choice.startsWith(kChoicePrefix) ? choice.mid(kChoicePrefix.size()) : QString();
}

QString fontFileLabel(const QString& path) {
  const QString family = fontFileFamily(path);
  const QString name = QFileInfo(path).fileName();
  return family.isEmpty() ? name : QStringLiteral("%1 (%2)").arg(family, name);
}

QImage fontPreview(const QString& choice) {
  static QHash<QString, QImage> previews;
  const auto found = previews.constFind(choice);
  if (found != previews.constEnd()) {
    return *found;
  }
  const QString file = fontFileOfChoice(choice);
  const QString family = file.isEmpty() ? choice : fontFileFamily(file);
  QImage image;
  if (!family.isEmpty()) {
    image = QImage(96, 96, QImage::Format_ARGB32_Premultiplied);
    image.setDevicePixelRatio(2.0);
    image.fill(Qt::transparent);
    QPainter painter(&image);
    painter.setRenderHint(QPainter::TextAntialiasing);
    QFont font(family);
    font.setPixelSize(30);
    painter.setFont(font);
    painter.setPen(QColor(60, 60, 60));
    painter.drawText(QRectF(0, 0, 48, 48), Qt::AlignCenter, QStringLiteral("Ag"));
  }
  previews.insert(choice, image);
  return image;
}

FontPreferencesBox::FontPreferencesBox(QWidget* parent)
#ifdef Q_OS_MACOS
    : QWidget(parent) {
#else
    : QGroupBox(tr("Fonts"), parent) {
#endif
  auto* layout = new QVBoxLayout(this);
  auto* intro = new QLabel(tr("Edit Text offers the fonts in these folders (and their subfolders) beside the "
                              "installed ones: TrueType and OpenType files (.ttf, .otf, .ttc) and web fonts "
                              "(.woff, .woff2). A text keeps its font file's outlines in the design, so the design "
                              "looks the same where the file is not."));
  intro->setWordWrap(true);
  layout->addWidget(intro);
  m_folders = new QListWidget;
  m_folders->setObjectName(QStringLiteral("fontFolders"));
  m_folders->setSelectionMode(QAbstractItemView::ExtendedSelection);
  m_folders->setMinimumHeight(90);
  for (const QString& folder : savedFontFolders()) {
    m_folders->addItem(QDir::toNativeSeparators(folder));
  }
  layout->addWidget(m_folders);
  mac::stylePreview(m_folders);
  auto* row = new QHBoxLayout;
  m_typed = new QLineEdit;
  m_typed->setObjectName(QStringLiteral("fontFolder"));
  m_typed->setPlaceholderText(tr("A folder of fonts"));
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

  connect(add, &QPushButton::clicked, this, [this] {
    addFolder(m_typed->text());
    m_typed->clear();
  });
  connect(choose, &QPushButton::clicked, this, [this] {
    const QString folder = dialogDirectory(this, tr("Font Folder"));
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

void FontPreferencesBox::showEvent(QShowEvent* event) {
  FontPreferencesBase::showEvent(event);
  // Where the folder field is, in the main window's coordinates (UI tests).
  QTimer::singleShot(0, this, [this] {
    QWidget* main = window()->parentWidget() != nullptr ? window()->parentWidget()->window() : nullptr;
    if (main != nullptr && m_typed->isVisible()) {
      const QPoint at = m_typed->mapToGlobal(m_typed->rect().center()) - main->mapToGlobal(QPoint(0, 0));
      qDebug().noquote() << QStringLiteral("Preferences font folder at %1,%2").arg(at.x()).arg(at.y());
    }
  });
}

void FontPreferencesBox::addFolder(const QString& typed) {
  if (typed.trimmed().isEmpty()) {
    return;
  }
  const QString folder = cleanFolder(typed);
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

QStringList FontPreferencesBox::chosen() const {
  QStringList folders;
  for (int i = 0; i < m_folders->count(); ++i) {
    folders << QDir::fromNativeSeparators(m_folders->item(i)->text());
  }
  const QString typed = m_typed->text().trimmed();
  if (!typed.isEmpty() && !folders.contains(cleanFolder(typed))) {
    folders << cleanFolder(typed);
  }
  return folders;
}

} // namespace mitcad::sketch
