// SPDX-License-Identifier: MIT
#include "AgentFolders.hpp"

#include <QDir>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QLabel>
#include <QListWidget>
#include <QPushButton>
#include <QSettings>
#include <QSignalBlocker>
#include <QVBoxLayout>

#include <utility>

#include "../framework/Dialogs.hpp"
#include "../platform/MacDialogs.hpp"

namespace mitcad::mcp {
namespace {

const QString kSetting = QStringLiteral("mcp/folders");

QStringList& runFolders() {
  static QStringList folders;
  return folders;
}

// Absolute and cleaned, with '/' separators.
QString cleanFolder(const QString& folder) {
  return QDir::cleanPath(QFileInfo(QDir::fromNativeSeparators(folder.trimmed())).absoluteFilePath());
}

void addClean(QStringList& list, const QString& folder) {
  if (folder.trimmed().isEmpty()) {
    return;
  }
  const QString clean = cleanFolder(folder);
  if (!list.contains(clean)) {
    list << clean;
  }
}

// `path` is `folder` or below it (both resolved).
bool inside(const QString& path, const QString& folder) {
#ifdef Q_OS_WIN
  constexpr Qt::CaseSensitivity kCase = Qt::CaseInsensitive;
#else
  constexpr Qt::CaseSensitivity kCase = Qt::CaseSensitive;
#endif
  if (folder.isEmpty()) {
    return false;
  }
  const QString prefix = folder.endsWith(QLatin1Char('/')) ? folder : folder + QLatin1Char('/');
  return path.compare(folder, kCase) == 0 || path.startsWith(prefix, kCase);
}

bool insideAny(const QString& path, const QStringList& folders) {
  for (const QString& folder : folders) {
    if (inside(path, QFileInfo(folder).canonicalFilePath())) {
      return true;
    }
  }
  return false;
}

} // namespace

QStringList savedAgentFolders() {
  QStringList folders;
  for (const QString& folder : QSettings().value(kSetting).toStringList()) {
    addClean(folders, folder);
  }
  return folders;
}

void saveAgentFolders(const QStringList& folders) {
  QStringList clean;
  for (const QString& folder : folders) {
    addClean(clean, folder);
  }
  QSettings().setValue(kSetting, clean);
}

void setRunAgentFolders(const QStringList& folders) {
  runFolders().clear();
  for (const QString& folder : folders) {
    addClean(runFolders(), folder);
  }
}

QStringList agentFolders() {
  QStringList folders = savedAgentFolders();
  for (const QString& folder : std::as_const(runFolders())) {
    addClean(folders, folder);
  }
  return folders;
}

namespace {

// A file in an allowed folder (agentDesignPath, agentOutputPath): one of
// `suffixes` (any when empty; `design` says it must be a Mitcad design).
QString agentPath(const QString& given, const QStringList& suffixes, bool design, bool forWriting, bool overwrite,
                  QString& reason) {
  reason.clear();
  const QStringList folders = agentFolders();
  if (folders.isEmpty()) {
    // The window as the platform names it (tools.preferences); the
    // command-line option is in docs/mcp.md only.
#ifdef Q_OS_MACOS
    reason = QObject::tr("No folder for designs is allowed: add one in Settings › AI Assistants.");
#else
    reason = QObject::tr("No folder for designs is allowed: add one in Preferences › AI Assistants.");
#endif
    return QString();
  }
  const QString text = QDir::fromNativeSeparators(given.trimmed());
  if (text.isEmpty() || text.split(QLatin1Char('/')).contains(QStringLiteral(".."))) {
    reason = QObject::tr("Paths must be nonempty and cannot contain '..'.");
    return QString();
  }
  const QFileInfo info(QDir::cleanPath(QDir::isAbsolutePath(text) ? text : QDir(folders.first()).filePath(text)));
  if (!suffixes.isEmpty() && !suffixes.contains(info.suffix(), Qt::CaseInsensitive)) {
    reason = design ? QObject::tr("%1 is not a Mitcad design: the path must end in .mitcad.")
                          .arg(QDir::toNativeSeparators(text))
                    : QObject::tr("%1 must end in .%2.")
                          .arg(QDir::toNativeSeparators(text), suffixes.join(QStringLiteral(" or .")));
    return QString();
  }
  const QString folder = QFileInfo(info.absolutePath()).canonicalFilePath();
  if (folder.isEmpty() || !QFileInfo(folder).isDir()) {
    reason = QObject::tr("The folder %1 does not exist.").arg(QDir::toNativeSeparators(info.absolutePath()));
    return QString();
  }
  const QString outside = QObject::tr("%1 is not in a folder for designs (%2).")
                              .arg(QDir::toNativeSeparators(info.absoluteFilePath()),
                                   QDir::toNativeSeparators(folders.join(QStringLiteral(", "))));
  if (!insideAny(folder, folders)) {
    reason = outside;
    return QString();
  }
  const QString path = QDir(folder).filePath(info.fileName());
  const QFileInfo target(path);
  if (forWriting) {
    if (target.exists() || target.isSymLink()) {
      if (target.isSymLink() || !target.isFile()) {
        reason = QObject::tr("%1 is not a regular file.").arg(QDir::toNativeSeparators(path));
        return QString();
      }
      if (!overwrite) {
        reason = QObject::tr("%1 already exists: pass overwrite: true to replace it.").arg(QDir::toNativeSeparators(path));
        return QString();
      }
    }
    return path;
  }
  // Reading: a link is followed, and where it leads must be allowed too.
  const QString resolved = target.canonicalFilePath();
  if (resolved.isEmpty() || !QFileInfo(resolved).isFile()) {
    reason = QObject::tr("%1 does not exist.").arg(QDir::toNativeSeparators(path));
    return QString();
  }
  if (!insideAny(QFileInfo(resolved).absolutePath(), folders)) {
    reason = outside;
    return QString();
  }
  return resolved;
}

} // namespace

QString agentDesignPath(const QString& given, bool forWriting, bool overwrite, QString& reason) {
  return agentPath(given, {QStringLiteral("mitcad")}, true, forWriting, overwrite, reason);
}

QString agentOutputPath(const QString& given, bool overwrite, QString& reason, const QStringList& suffixes) {
  return agentPath(given, suffixes, false, true, overwrite, reason);
}

AgentPreferencesBox::AgentPreferencesBox(QWidget* parent)
#ifdef Q_OS_MACOS
    : QWidget(parent)
#else
    : QGroupBox(tr("AI Assistants"), parent)
#endif
{
  auto* layout = new QVBoxLayout(this);
  auto* connectAssistant = new QPushButton(tr("Connect AI Assistant…"));
  connectAssistant->setObjectName(QStringLiteral("connectAssistant"));
  connectAssistant->setAutoDefault(false);
  connect(connectAssistant, &QPushButton::clicked, this, &AgentPreferencesBox::connectRequested);
  // Its natural width, as the other pages' buttons.
  layout->addWidget(connectAssistant, 0, Qt::AlignLeft);
  // The list's name, as the Connect AI Assistant dialog's folder line.
  auto* title = new QLabel(tr("Folders for designs"));
  layout->addWidget(title);
  m_folders = new QListWidget;
  m_folders->setObjectName(QStringLiteral("agentFolders"));
  // Named also while it is empty.
  m_folders->setAccessibleName(tr("Folders for designs"));
  title->setBuddy(m_folders);
  m_folders->setSelectionMode(QAbstractItemView::ExtendedSelection);
  m_folders->setMinimumHeight(90);
  for (const QString& folder : savedAgentFolders()) {
    m_folders->addItem(QDir::toNativeSeparators(folder));
  }
  // The list takes the page's height.
  layout->addWidget(m_folders, 1);
  mac::stylePreview(m_folders);
  // Add… chooses a folder, Remove takes the selected ones off the list.
  auto* add = new QPushButton(tr("Add…"));
  add->setObjectName(QStringLiteral("agentFolderAdd"));
  add->setToolTip(tr("Chooses a folder to add"));
  m_remove = new QPushButton(tr("Remove"));
  m_remove->setToolTip(tr("Removes the selected folders from the list (the files stay)"));
  m_remove->setEnabled(false);
  for (QPushButton* button : {add, m_remove}) {
    button->setAutoDefault(false);
  }
  auto* controls = new QHBoxLayout;
#ifdef Q_OS_MACOS
  controls->setSpacing(0);
#endif
  controls->addWidget(add);
  controls->addWidget(m_remove);
  controls->addStretch(1);
  mac::styleListButtons(add, m_remove);
  layout->addLayout(controls);

  connect(add, &QPushButton::clicked, this, [this] {
    const QString folder = dialogDirectory(this, tr("Folder for Designs"));
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

void AgentPreferencesBox::addFolder(const QString& typed) {
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

void AgentPreferencesBox::includeSavedFolders() {
  const QSignalBlocker blocker(this);
  for (const QString& folder : savedAgentFolders()) addFolder(folder);
}

QStringList AgentPreferencesBox::chosen() const {
  QStringList folders;
  for (int i = 0; i < m_folders->count(); ++i) {
    addClean(folders, m_folders->item(i)->text());
  }
  return folders;
}

} // namespace mitcad::mcp
