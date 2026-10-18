// SPDX-License-Identifier: MIT
#include "AgentFolders.hpp"

#include <QDir>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QPushButton>
#include <QSettings>
#include <QVBoxLayout>

#include <utility>

#include "../framework/Dialogs.hpp"

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

QString agentDesignPath(const QString& given, bool forWriting, bool overwrite, QString& reason) {
  reason.clear();
  const QStringList folders = agentFolders();
  if (folders.isEmpty()) {
    reason = QObject::tr("No folder is allowed for agents: the user can add one in Preferences > Agents, or start "
                         "Mitcad with --mcp-folder DIR.");
    return QString();
  }
  const QString text = QDir::fromNativeSeparators(given.trimmed());
  if (text.isEmpty() || text.split(QLatin1Char('/')).contains(QStringLiteral(".."))) {
    reason = QObject::tr("Paths must be nonempty and cannot contain '..'.");
    return QString();
  }
  const QFileInfo info(QDir::cleanPath(QDir::isAbsolutePath(text) ? text : QDir(folders.first()).filePath(text)));
  if (info.suffix().compare(QStringLiteral("mitcad"), Qt::CaseInsensitive) != 0) {
    reason = QObject::tr("%1 is not a Mitcad design: the path must end in .mitcad.").arg(QDir::toNativeSeparators(text));
    return QString();
  }
  const QString folder = QFileInfo(info.absolutePath()).canonicalFilePath();
  if (folder.isEmpty() || !QFileInfo(folder).isDir()) {
    reason = QObject::tr("The folder %1 does not exist.").arg(QDir::toNativeSeparators(info.absolutePath()));
    return QString();
  }
  const QString outside = QObject::tr("%1 is not in a folder agents may use (%2).")
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

AgentPreferencesBox::AgentPreferencesBox(QWidget* parent)
#ifdef Q_OS_MACOS
    : QWidget(parent)
#else
    : QGroupBox(tr("Agents"), parent)
#endif
{
  auto* layout = new QVBoxLayout(this);
  auto* intro = new QLabel(tr("An agent connected through MCP (mitcad-cli mcp --connect) works on the open "
                              "document. It may save the design to, and open designs from, only these folders "
                              "(and their subfolders); with none, it can only save a design that already has a "
                              "file."));
  intro->setWordWrap(true);
  layout->addWidget(intro);
  m_folders = new QListWidget;
  m_folders->setObjectName(QStringLiteral("agentFolders"));
  m_folders->setSelectionMode(QAbstractItemView::ExtendedSelection);
  m_folders->setMinimumHeight(90);
  for (const QString& folder : savedAgentFolders()) {
    m_folders->addItem(QDir::toNativeSeparators(folder));
  }
  layout->addWidget(m_folders);
  auto* row = new QHBoxLayout;
  m_typed = new QLineEdit;
  m_typed->setObjectName(QStringLiteral("agentFolder"));
  m_typed->setPlaceholderText(tr("A folder for agents"));
  m_typed->setToolTip(tr("Add, or OK, adds the folder typed here"));
  auto* add = new QPushButton(tr("Add"));
  add->setToolTip(tr("Adds the folder typed at the left"));
  auto* choose = new QPushButton(tr("Choose..."));
  choose->setToolTip(tr("Chooses a folder to add"));
  m_remove = new QPushButton(tr("Remove"));
  m_remove->setToolTip(tr("Removes the selected folders from the list (the files stay)"));
  m_remove->setEnabled(false);
  for (QPushButton* button : {add, choose, m_remove}) {
    button->setAutoDefault(false);
  }
  row->addWidget(m_typed, 1);
  row->addWidget(add);
  row->addWidget(choose);
  row->addWidget(m_remove);
  layout->addLayout(row);

  connect(add, &QPushButton::clicked, this, [this] {
    addFolder(m_typed->text());
    m_typed->clear();
  });
  connect(choose, &QPushButton::clicked, this, [this] {
    const QString folder = dialogDirectory(this, tr("Folder for Agents"));
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

QStringList AgentPreferencesBox::chosen() const {
  QStringList folders;
  for (int i = 0; i < m_folders->count(); ++i) {
    addClean(folders, m_folders->item(i)->text());
  }
  addClean(folders, m_typed->text());
  return folders;
}

} // namespace mitcad::mcp
