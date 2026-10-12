// SPDX-License-Identifier: MIT
#include "ProjectsPanel.hpp"

#include <algorithm>
#include <functional>
#include <utility>

#include <QAbstractItemDelegate>
#include <QAction>
#include <QApplication>
#include <QDateTime>
#include <QDir>
#include <QEvent>
#include <QFileInfo>
#include <QFont>
#include <QHeaderView>
#include <QKeyEvent>
#include <QMenu>
#include <QMouseEvent>
#include <QScrollBar>
#include <QToolBar>
#include <QToolButton>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>
#include <QtLogging>

#include "../files/RemoteTask.hpp"
#include "../framework/AppSettings.hpp"
#include "../framework/ChromeStyle.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/GlassCard.hpp"
#include "../framework/Icons.hpp"
#include "../framework/TestSync.hpp"

namespace mitcad {
namespace {

constexpr int kName = 0;
constexpr int kState = 1;
constexpr int kTypeRole = Qt::UserRole;
constexpr int kRootRole = Qt::UserRole + 1;
constexpr int kPathRole = Qt::UserRole + 2;
constexpr int kKeyRole = Qt::UserRole + 3;
constexpr int kDefaultWidth = 240;

const QString kSuffix = QStringLiteral(".mitcad");

QString dot() { return QStringLiteral(" ") + QChar(0x00B7) + QStringLiteral(" "); }

QString joinPath(const QString& folder, const QString& name) {
  return folder.isEmpty() ? name : QStringLiteral("%1/%2").arg(folder, name);
}

QString dirOf(const QString& path) {
  const int slash = path.lastIndexOf(QLatin1Char('/'));
  return slash < 0 ? QString() : path.left(slash);
}

QString lastPart(const QString& path) { return path.mid(path.lastIndexOf(QLatin1Char('/')) + 1); }

// A design's name as the panel shows it: its file's name without .mitcad.
QString designName(const QString& path) {
  const QString file = lastPart(path);
  return file.endsWith(kSuffix, Qt::CaseInsensitive) ? file.left(file.size() - kSuffix.size()) : file;
}

// Delete deletes; on macOS the delete key of the keyboard (Backspace) too.
bool isDeleteKey(int key) {
#ifdef Q_OS_MACOS
  return key == Qt::Key_Delete || key == Qt::Key_Backspace;
#else
  return key == Qt::Key_Delete;
#endif
}

// What a row shows in its second column, and what the log says.
struct StateText {
  QString shown;
  QString logged;
};

StateText projectState(const ProjectRow& row, const ProjectIndicator::Remote* remote) {
  switch (row.kind) {
  case ProjectRow::Kind::Missing:
    return {ProjectsPanel::tr("Missing"), QStringLiteral("missing")};
  case ProjectRow::Kind::Unreadable:
    return {ProjectsPanel::tr("Cannot be read"), QStringLiteral("unreadable")};
  case ProjectRow::Kind::NoHistory:
    return {ProjectsPanel::tr("No versions"), QStringLiteral("no versions")};
  case ProjectRow::Kind::Local:
    if (row.noRemoteChosen) {
      return {ProjectsPanel::tr("Cloud") + dot() + ProjectsPanel::tr("No remote chosen"),
              QStringLiteral("cloud, no remote chosen")};
    }
    return {ProjectsPanel::tr("Local"), QStringLiteral("local")};
  case ProjectRow::Kind::Cloud:
    break;
  }
  // As the project indicator shows it: the current project's remote as
  // the remote work knows it, others' from their local refs.
  int ahead = row.ahead;
  int behind = row.behind;
  QString problem;
  bool conflict = false;
  if (remote != nullptr) {
    ahead = remote->ahead >= 0 ? remote->ahead : ahead;
    behind = remote->behind >= 0 ? remote->behind : behind;
    problem = remote->problem;
    conflict = remote->conflict;
  }
  const QString up = QString(QChar(0x2191));
  const QString down = QString(QChar(0x2193));
  QStringList shown{ProjectsPanel::tr("Cloud")};
  QStringList logged{QStringLiteral("cloud")};
  if (conflict) {
    shown << ProjectsPanel::tr("Conflict");
    logged << QStringLiteral("conflict");
  } else if (problem == QLatin1String("network") || problem == QLatin1String("timed_out")) {
    shown << (ahead > 0 ? ProjectsPanel::tr("Offline, %1%2 waiting").arg(up).arg(ahead) : ProjectsPanel::tr("Offline"));
    logged << QStringLiteral("offline");
  } else if (problem == QLatin1String("auth_failed") || problem == QLatin1String("host_key_unknown")) {
    shown << ProjectsPanel::tr("Sign-in needed");
    logged << QStringLiteral("sign-in needed");
  } else {
    if (ahead > 0) {
      shown << up + QString::number(ahead);
      logged << QStringLiteral("ahead %1").arg(ahead);
    }
    if (behind > 0) {
      shown << down + QString::number(behind);
      logged << QStringLiteral("behind %1").arg(behind);
    }
    if (ahead == 0 && behind == 0) {
      shown << QString(QChar(0x2713));
      logged << QStringLiteral("synced");
    } else if (ahead <= 0 && behind <= 0) {
      shown << ProjectsPanel::tr("Not synced yet");
      logged << QStringLiteral("not synced");
    }
  }
  return {shown.join(dot()), logged.join(QStringLiteral(", "))};
}

StateText designState(const ProjectDesignRow& design) {
  QStringList shown;
  QStringList logged;
  if (design.version > 0) {
    shown << ProjectsPanel::tr("v%1").arg(design.version);
    logged << QStringLiteral("v%1").arg(design.version);
  } else {
    shown << ProjectsPanel::tr("Not recorded");
    logged << QStringLiteral("not recorded");
  }
  if (design.recorded && design.changed) {
    shown << ProjectsPanel::tr("Changed");
    logged << QStringLiteral("changed");
  }
  if (design.locked) {
    if (design.lockMine) {
      shown << ProjectsPanel::tr("Editing");
      logged << QStringLiteral("editing");
    } else if (design.lockSameOwner) {
      shown << ProjectsPanel::tr("Editing elsewhere");
      logged << QStringLiteral("editing elsewhere");
    } else {
      shown << ProjectsPanel::tr("Locked: %1").arg(design.lockOwner);
      logged << QStringLiteral("locked by %1").arg(design.lockOwner);
    }
  }
  return {shown.join(dot()), logged.join(QStringLiteral(", "))};
}

QString designTip(const QString& root, const ProjectDesignRow& design) {
  QStringList tip{QDir::toNativeSeparators(QDir(root).filePath(design.path))};
  if (design.version > 0) {
    const QDateTime when = QDateTime::fromSecsSinceEpoch(design.versionTime).toLocalTime();
    tip << ProjectsPanel::tr("Version %1 (%2), saved %3 by %4")
               .arg(design.version)
               .arg(design.versionId, when.toString(QStringLiteral("yyyy-MM-dd HH:mm")), design.versionAuthor);
  } else {
    tip << ProjectsPanel::tr("No version recorded yet: Save records one");
  }
  if (design.recorded && design.changed) {
    tip << ProjectsPanel::tr("Changed since its latest version");
  }
  if (design.locked) {
    tip << (design.lockMine        ? ProjectsPanel::tr("You are editing it here")
            : design.lockSameOwner ? ProjectsPanel::tr("You are editing it in another window or on another computer")
                                   : ProjectsPanel::tr("%1 is editing it: it opens read-only").arg(design.lockOwner));
  }
  return tip.join(QLatin1Char('\n'));
}

// A context menu whose entries are logged ("Context menu: A | B") for the
// UI tests, which choose entries by their place.
class LoggedMenu {
public:
  explicit LoggedMenu(QWidget* parent) : m_menu(parent) {}
  void add(const QString& icon, const QString& text, std::function<void()> call) {
    QAction* action = m_menu.addAction(icon.isEmpty() ? QIcon() : themeIcon(icon), text);
    QObject::connect(action, &QAction::triggered, &m_menu, std::move(call));
    m_entries << QString(text).remove(QLatin1Char('&'));
  }
  void separator() {
    if (!m_entries.isEmpty()) {
      m_menu.addSeparator();
    }
  }
  void exec(const QPoint& at) {
    if (m_entries.isEmpty()) {
      return;
    }
    qDebug().noquote() << QStringLiteral("Context menu: %1").arg(m_entries.join(QStringLiteral(" | ")));
    execMenu(&m_menu, at);
  }

private:
  QMenu m_menu;
  QStringList m_entries;
};

} // namespace

// ---------------------------------------------------------------------------
// The tree: keys, double-clicks and drags of rows

class ProjectsTreeWidget : public QTreeWidget {
public:
  explicit ProjectsTreeWidget(ProjectsPanel& panel) : QTreeWidget(&panel), m_panel(panel) {}

  bool editing() const { return state() == QAbstractItemView::EditingState; }
  bool dragging() const { return m_dragging; }

  bool cancelEdit() {
    if (!editing()) {
      return false;
    }
    if (QWidget* editor = indexWidget(currentIndex())) {
      closeEditor(editor, QAbstractItemDelegate::RevertModelCache);
    }
    return true;
  }

protected:
  bool event(QEvent* event) override {
    // The panel's keys before the window's shortcuts (Return confirms a
    // command there, Delete deletes in a sketch).
    if (event->type() == QEvent::ShortcutOverride && !editing() && currentItem() != nullptr) {
      const auto* key = static_cast<QKeyEvent*>(event);
      const bool plain = (key->modifiers() & ~Qt::KeypadModifier) == Qt::NoModifier;
      if (plain && (key->key() == Qt::Key_Return || key->key() == Qt::Key_Enter || key->key() == Qt::Key_F2 ||
                    isDeleteKey(key->key()))) {
        event->accept();
        return true;
      }
    }
    return QTreeWidget::event(event);
  }

  void keyPressEvent(QKeyEvent* event) override {
    if (!editing() && currentItem() != nullptr) {
      if (event->key() == Qt::Key_F2) {
        m_panel.renameCurrent();
        return;
      }
      if (isDeleteKey(event->key())) {
        m_panel.deleteCurrent();
        return;
      }
      if (event->key() == Qt::Key_Return || event->key() == Qt::Key_Enter) {
        m_panel.openCurrent();
        return;
      }
    }
    QTreeWidget::keyPressEvent(event);
  }

  void mouseDoubleClickEvent(QMouseEvent* event) override {
    QTreeWidgetItem* item = itemAt(event->position().toPoint());
    if (item != nullptr && event->button() == Qt::LeftButton &&
        m_panel.typeOf(item) == ProjectsPanel::RowType::Design) {
      setCurrentItem(item);
      m_panel.openCurrent();
      return;
    }
    // A Cloud project: its settings (mitcad#152) instead of expanding the
    // row.
    if (item != nullptr && event->button() == Qt::LeftButton && m_panel.settingsOnDoubleClick(item)) {
      return;
    }
    QTreeWidget::mouseDoubleClickEvent(event);
  }

  // A drag of a design or a folder onto a folder, a project or a design
  // (its folder) of the same project moves it there. Done here rather than
  // with the system's drag and drop: nothing leaves the panel.
  void mousePressEvent(QMouseEvent* event) override {
    m_dragged = nullptr;
    m_dragging = false;
    if (event->button() == Qt::LeftButton && !editing()) {
      m_pressed = event->position().toPoint();
      QTreeWidgetItem* item = itemAt(m_pressed);
      const ProjectsPanel::RowType type = m_panel.typeOf(item);
      if (item != nullptr && (type == ProjectsPanel::RowType::Design || type == ProjectsPanel::RowType::Folder)) {
        m_dragged = item;
      }
    }
    QTreeWidget::mousePressEvent(event);
  }

  void mouseMoveEvent(QMouseEvent* event) override {
    if (m_dragged != nullptr && (event->buttons() & Qt::LeftButton) != 0) {
      const QPoint at = event->position().toPoint();
      if (!m_dragging && (at - m_pressed).manhattanLength() >= QApplication::startDragDistance()) {
        m_dragging = true;
        qInfo().noquote() << QStringLiteral("Projects panel: dragging %1").arg(m_panel.keyOf(m_dragged));
      }
      if (m_dragging) {
        setOver(itemAt(at));
        return;
      }
    }
    QTreeWidget::mouseMoveEvent(event);
  }

  void mouseReleaseEvent(QMouseEvent* event) override {
    if (m_dragging) {
      QTreeWidgetItem* dragged = m_dragged;
      QTreeWidgetItem* over = itemAt(event->position().toPoint());
      setOver(nullptr);
      unsetCursor();
      m_dragging = false;
      m_dragged = nullptr;
      if (event->button() == Qt::LeftButton) {
        m_panel.dropped(dragged, over);
      }
      m_panel.applyWaitingRows();
      return;
    }
    m_dragged = nullptr;
    QTreeWidget::mouseReleaseEvent(event);
  }

private:
  void setOver(QTreeWidgetItem* item) {
    if (item == m_over) {
      return;
    }
    if (m_over != nullptr) {
      for (int column = 0; column < columnCount(); ++column) {
        m_over->setData(column, Qt::BackgroundRole, QVariant());
      }
    }
    m_over = nullptr;
    QString folder;
    if (item != nullptr && m_panel.dropTarget(m_dragged, item, &folder)) {
      m_over = item;
      QColor color = palette().color(QPalette::Highlight);
      color.setAlpha(80);
      for (int column = 0; column < columnCount(); ++column) {
        m_over->setBackground(column, color);
      }
      setCursor(Qt::DragMoveCursor);
    } else if (m_dragging) {
      setCursor(Qt::ForbiddenCursor);
    }
  }

  ProjectsPanel& m_panel;
  QPoint m_pressed;
  QTreeWidgetItem* m_dragged = nullptr;
  QTreeWidgetItem* m_over = nullptr;
  bool m_dragging = false;
};

// ---------------------------------------------------------------------------
// The panel

ProjectsPanel::ProjectsPanel(Host host, QWidget* parent) : QWidget(parent), m_host(std::move(host)) {
  setObjectName(QStringLiteral("projectsPanel"));
  auto* layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(2);
  m_toolbar = new QToolBar(this);
  m_toolbar->setObjectName(QStringLiteral("projectsToolbar"));
  m_toolbar->setIconSize(QSize(16, 16));
  m_toolbar->setToolButtonStyle(Qt::ToolButtonIconOnly);
  layout->addWidget(m_toolbar);
  m_tree = new ProjectsTreeWidget(*this);
  m_tree->setObjectName(QStringLiteral("projectsTree"));
  layout->addWidget(m_tree, 1);
  m_tree->setColumnCount(2);
  m_tree->setHeaderHidden(true);
  m_tree->header()->setStretchLastSection(false);
  m_tree->header()->setSectionResizeMode(kName, QHeaderView::Stretch);
  m_tree->header()->setSectionResizeMode(kState, QHeaderView::ResizeToContents);
  m_tree->setSelectionMode(QAbstractItemView::SingleSelection);
  m_tree->setEditTriggers(QAbstractItemView::NoEditTriggers);
  m_tree->setContextMenuPolicy(Qt::CustomContextMenu);
  m_tree->setUniformRowHeights(true);
  m_tree->setIconSize(QSize(16, 16));
  m_tree->setExpandsOnDoubleClick(true);
  // Only a click gives the tree the keyboard, so that the view keeps the
  // single-letter shortcuts.
  m_tree->setFocusPolicy(Qt::ClickFocus);
  setMinimumWidth(160);
  if (chromeStyle() == ChromeStyle::Floating) {
    // Inside a card: the card's material shows through.
    m_tree->setFrameShape(QFrame::NoFrame);
    m_tree->setAttribute(Qt::WA_MacShowFocusRect, false);
    QPalette palette = m_tree->palette();
    for (const QPalette::ColorRole role : {QPalette::Base, QPalette::AlternateBase}) {
      palette.setColor(role, Qt::transparent);
    }
    m_tree->setPalette(palette);
    m_tree->viewport()->setAutoFillBackground(false);
  }

  const auto action = [this](const char* icon, const QString& text, const QString& tip, auto run) {
    QAction* made = m_toolbar->addAction(themeIcon(QString::fromLatin1(icon)), text);
    made->setToolTip(tip);
    connect(made, &QAction::triggered, this, run);
    return made;
  };
  QAction* newProject = action("add", tr("New Project"), tr("New Project: create a Local or Cloud project"), [this] {
    emit commandRequested(QStringLiteral("file.new_project"));
  });
  newProject->setObjectName(QStringLiteral("projectsNewProject"));
  m_newDesign = action("new-design", tr("New Design"),
                       tr("New Design: a design in the selected project folder, named here"), [this] {
    QString root = selectedRoot();
    if (root.isEmpty() && m_host.currentRoot) {
      root = m_host.currentRoot();
    }
    if (!root.isEmpty()) {
      startNewDesign(root, sameRoot(root, selectedRoot()) ? selectedFolder() : QString());
    }
  });
  m_newDesign->setObjectName(QStringLiteral("projectsNewDesign"));
  m_newFolder = action("folder", tr("New Folder"), tr("A new folder in the selected folder, named here"), [this] {
    QString root = selectedRoot();
    if (root.isEmpty() && m_host.currentRoot) {
      root = m_host.currentRoot();
    }
    if (!root.isEmpty()) {
      startNewFolder(root, sameRoot(root, selectedRoot()) ? selectedFolder() : QString());
    }
  });
  m_rename = action("edit", tr("Rename"), tr("Renames the selected design or folder (F2)"), [this] { renameCurrent(); });
  m_delete = action("delete", tr("Delete"),
                    tr("Deletes the selected design or folder as a new version: the history keeps it (Del)"),
                    [this] { deleteCurrent(); });
  m_history = action("version-history", tr("Version History"), tr("The selected design's versions"), [this] {
    QTreeWidgetItem* item = m_tree->currentItem();
    if (typeOf(item) == RowType::Design) {
      emit versionHistoryRequested(rootOf(item), pathOf(item));
    }
  });
  m_sync = action("sync", tr("Sync Now"), tr("Takes the project's newer versions and sends yours"), [this] {
    if (!selectedRoot().isEmpty()) {
      emit syncRequested(selectedRoot());
    }
  });
  m_settings = action("settings", tr("Project Settings"), tr("The selected project's settings"), [this] {
    // The project of the selected row: the project itself, a folder or a
    // design in it (mitcad#153).
    QTreeWidgetItem* item = m_tree->currentItem();
    const QString root = settingsRoot(item);
    qInfo().noquote() << QStringLiteral("Projects panel: settings of %1%2")
                             .arg(root.isEmpty() ? QStringLiteral("no project") : root,
                                  item == nullptr ? QString() : QStringLiteral(" (%1 selected)").arg(keyOf(item)));
    if (!root.isEmpty()) {
      emit projectSettingsRequested(root);
    }
  });
  auto* more = new QMenu(this);
  const auto entry = [this, more](const char* icon, const QString& text, auto run) {
    QAction* made = more->addAction(icon == nullptr ? QIcon() : themeIcon(QString::fromLatin1(icon)), text);
    connect(made, &QAction::triggered, this, run);
    return made;
  };
  entry("new-project", tr("New Project..."), [this] { emit commandRequested(QStringLiteral("file.new_project")); });
  entry("remote-open", tr("Open from Cloud..."), [this] { emit commandRequested(QStringLiteral("file.open_remote")); });
  entry("folder", tr("Add Existing Project Folder..."), [this] { emit addFolderRequested(); });
  more->addSeparator();
  QAction* show = entry(nullptr, tr("Show in Folder"), [this] {
    QTreeWidgetItem* item = m_tree->currentItem();
    if (!rootOf(item).isEmpty()) {
      emit showInFolderRequested(QDir(rootOf(item)).filePath(folderFor(item)));
    }
  });
  QAction* remove = entry(nullptr, tr("Remove from List"), [this] {
    QTreeWidgetItem* item = m_tree->currentItem();
    if (typeOf(item) == RowType::Project) {
      emit removeFromListRequested(rootOf(item));
    }
  });
  connect(more, &QMenu::aboutToShow, this, [this, show, remove] {
    QTreeWidgetItem* item = m_tree->currentItem();
    show->setEnabled(!rootOf(item).isEmpty() && project(rootOf(item)) != nullptr &&
                     project(rootOf(item))->kind != ProjectRow::Kind::Missing);
    remove->setEnabled(typeOf(item) == RowType::Project);
  });
  m_more = m_toolbar->addAction(themeIcon(QStringLiteral("new-project")), tr("Projects"));
  m_more->setToolTip(tr("New Project, Open from Cloud, Add Existing Project Folder, Show in Folder, Remove from "
                        "List"));
  m_more->setMenu(more);
  if (auto* button = qobject_cast<QToolButton*>(m_toolbar->widgetForAction(m_more))) {
    button->setPopupMode(QToolButton::InstantPopup);
  }
  for (QAction* each : m_toolbar->actions()) {
    if (QWidget* button = m_toolbar->widgetForAction(each)) {
      button->setFocusPolicy(Qt::NoFocus);
      button->setAccessibleName(each->text());
    }
  }

  connect(m_tree, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* item) {
    if (m_building) {
      return;
    }
    updateActions();
    const QString key = keyOf(item);
    if (!key.isEmpty() && key != m_loggedSelection) {
      m_loggedSelection = key;
      qInfo().noquote() << QStringLiteral("Projects panel selected %1").arg(key);
    }
  });
  connect(m_tree, &QTreeWidget::customContextMenuRequested, this, &ProjectsPanel::contextMenu);
  const auto remember = [this](QTreeWidgetItem* item, bool expanded) {
    if (!m_building) {
      m_expanded.insert(keyOf(item), expanded);
      emit contentHeightChanged();
      scheduleLayoutLog();
    }
  };
  connect(m_tree, &QTreeWidget::itemExpanded, this, [remember](QTreeWidgetItem* item) { remember(item, true); });
  connect(m_tree, &QTreeWidget::itemCollapsed, this, [remember](QTreeWidgetItem* item) { remember(item, false); });
  connect(m_tree->verticalScrollBar(), &QScrollBar::valueChanged, this, [this] { scheduleLayoutLog(); });
  // The end of a name typed in place: after the view has taken the text.
  connect(m_tree->itemDelegate(), &QAbstractItemDelegate::closeEditor, this,
          [this](QWidget*, QAbstractItemDelegate::EndEditHint hint) {
            editingClosed(hint != QAbstractItemDelegate::RevertModelCache);
          },
          Qt::QueuedConnection);
  updateActions();
}

ProjectsPanel::~ProjectsPanel() = default;

QSize ProjectsPanel::sizeHint() const { return QSize(kDefaultWidth, QWidget::sizeHint().height()); }

void ProjectsPanel::showEvent(QShowEvent* event) {
  QWidget::showEvent(event);
  refresh();
  scheduleLayoutLog();
}

void ProjectsPanel::resizeEvent(QResizeEvent* event) {
  QWidget::resizeEvent(event);
  scheduleLayoutLog();
}

bool ProjectsPanel::eventFilter(QObject* watched, QEvent* event) {
  if (event->type() == QEvent::Move || event->type() == QEvent::Resize || event->type() == QEvent::Show) {
    scheduleLayoutLog();
  }
  return QWidget::eventFilter(watched, event);
}

void ProjectsPanel::watchContainer(QWidget* container) {
  if (container != nullptr) {
    container->installEventFilter(this);
  }
}

// ---------------------------------------------------------------------------
// Reading

void ProjectsPanel::refresh() {
  if (!isVisible()) {
    return; // read when shown
  }
  if (m_task != nullptr) {
    m_readAgain = true;
    return;
  }
  startRead();
}

void ProjectsPanel::startRead() {
  const ProjectsSettings settings = ProjectsSettings::load();
  const QStringList known = settings.known;
  const QStringList hidden = settings.hidden;
  const QString folder = projectsDirectory();
  const QString session = m_host.session ? m_host.session() : QString();
  RemoteTask* task = RemoteTask::custom(
      QStringLiteral("project_tree"),
      [known, folder, hidden, session](const SyncControl&) { return readProjects(known, folder, hidden, session); },
      this);
  m_task = task;
  connect(task, &RemoteTask::finished, this, [this, task] { readFinished(task); });
  task->start();
}

void ProjectsPanel::readFinished(RemoteTask* task) {
  if (task != m_task) {
    return;
  }
  m_task = nullptr;
  const QJsonObject answer = task->answer();
  task->deleteLater();
  if (answer.contains(QStringLiteral("error"))) {
    qWarning().noquote() << QStringLiteral("Projects panel: the projects could not be read: %1")
                                .arg(answer.value(QStringLiteral("error"))
                                         .toObject()
                                         .value(QStringLiteral("message"))
                                         .toString());
  }
  const QVector<ProjectRow> rows = projectRows(answer);
  if (m_isEditing || m_tree->dragging()) {
    // Not under the name being typed or the row being dragged.
    m_waitingRows = rows;
    m_rowsWaiting = true;
  } else {
    m_rows = rows;
    rebuild();
  }
  if (m_readAgain) {
    m_readAgain = false;
    startRead();
  }
}

void ProjectsPanel::applyWaitingRows() {
  if (!m_rowsWaiting || m_isEditing || m_tree->dragging()) {
    return;
  }
  m_rowsWaiting = false;
  m_rows = m_waitingRows;
  m_waitingRows.clear();
  rebuild();
}

void ProjectsPanel::rebuild() {
  const QString selected = keyOf(m_tree->currentItem());
  const int scroll = m_tree->verticalScrollBar()->value();
  const QString current = m_host.currentRoot ? m_host.currentRoot() : QString();
  m_building = true;
  m_tree->clear();
  // The projects at the top, nothing above them (mitcad#173), by the name
  // shown.
  QVector<const ProjectRow*> rows;
  for (const ProjectRow& row : std::as_const(m_rows)) {
    rows << &row;
  }
  std::sort(rows.begin(), rows.end(), [](const ProjectRow* a, const ProjectRow* b) {
    const int byName = a->name.compare(b->name, Qt::CaseInsensitive);
    return byName != 0 ? byName < 0 : a->root < b->root;
  });
  for (const ProjectRow* row : std::as_const(rows)) {
    QTreeWidgetItem* projectRow = addProjectRow(*row, projectKeyOf(*row));
    const bool open = m_revealed.contains(row->root) || sameRoot(row->root, current);
    projectRow->setExpanded(m_expanded.value(keyOf(projectRow), open));
  }
  QTreeWidgetItem* select = nullptr;
  if (!m_reveal.isEmpty()) {
    select = projectItem(m_reveal);
    if (select != nullptr) {
      m_reveal.clear();
      for (QTreeWidgetItem* up = select->parent(); up != nullptr; up = up->parent()) {
        up->setExpanded(true);
      }
      select->setExpanded(true);
    }
  }
  if (select == nullptr && !selected.isEmpty()) {
    for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
      if (keyOf(*it) == selected) {
        select = *it;
        break;
      }
    }
  }
  if (select != nullptr) {
    m_tree->setCurrentItem(select);
  }
  m_tree->verticalScrollBar()->setValue(scroll);
  if (select != nullptr && keyOf(select) != selected) {
    m_tree->scrollToItem(select);
  }
  m_building = false;
  if (select != nullptr && keyOf(select) != m_loggedSelection) {
    m_loggedSelection = keyOf(select);
    qInfo().noquote() << QStringLiteral("Projects panel selected %1").arg(m_loggedSelection);
  }
  applyMarks();
  updateActions();
  emit contentHeightChanged();
  scheduleLayoutLog();
  if (m_hasPending) {
    m_hasPending = false;
    if (folderItem(m_pending.root, m_pending.parent) != nullptr) {
      startNew(m_pending.folder, m_pending.root, m_pending.parent);
    } else {
      qInfo().noquote() << QStringLiteral("Projects panel: %1 is not listed: nothing to name")
                               .arg(joinPath(m_pending.root, m_pending.parent));
    }
  }
}

QString ProjectsPanel::projectKeyOf(const ProjectRow& row) const {
  // The folder's name, which a display name does not change.
  int same = 0;
  for (const ProjectRow& other : m_rows) {
    same += other.folderName == row.folderName ? 1 : 0;
  }
  return same > 1 ? row.root : row.folderName;
}

QTreeWidgetItem* ProjectsPanel::addProjectRow(const ProjectRow& row, const QString& projectKey) {
  // Local and Cloud told apart by the icon and the tooltip (mitcad#173).
  const bool cloud = row.kind == ProjectRow::Kind::Cloud || row.noRemoteChosen;
  auto* top = new QTreeWidgetItem(m_tree);
  top->setText(kName, row.name);
  top->setIcon(kName, themeIcon(cloud ? QStringLiteral("remote-open") : QStringLiteral("home")));
  top->setData(kName, kTypeRole, static_cast<int>(RowType::Project));
  top->setData(kName, kRootRole, row.root);
  top->setData(kName, kPathRole, QString());
  top->setData(kName, kKeyRole, projectKey);
  top->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable);
  QStringList tip;
  if (row.kind == ProjectRow::Kind::Cloud) {
    tip << tr("Cloud project: its versions are also in a remote repository");
  } else if (row.noRemoteChosen) {
    tip << tr("Cloud project: no remote chosen");
  } else if (row.kind == ProjectRow::Kind::Local) {
    tip << tr("Local project: its versions are on this computer");
  }
  tip << tr("Folder: %1").arg(QDir::toNativeSeparators(row.root));
  if (!row.remoteUrl.isEmpty()) {
    tip << tr("Remote: %1").arg(row.remoteUrl);
  }
  if (row.kind == ProjectRow::Kind::NoHistory) {
    tip << (row.problem.isEmpty() ? tr("The project keeps no versions")
                                  : tr("The project keeps no versions: it is inside the git repository %1")
                                        .arg(QDir::toNativeSeparators(row.problem)));
  } else if (row.kind == ProjectRow::Kind::Missing) {
    tip << tr("The folder is not there any more: Remove from List takes it out of the list");
  } else if (!row.problem.isEmpty()) {
    tip << row.problem;
  }
  if (row.truncated) {
    tip << tr("Only the first designs and folders are listed");
  }
  top->setToolTip(kName, tip.join(QLatin1Char('\n')));
  top->setToolTip(kState, tip.join(QLatin1Char('\n')));
  QHash<QString, QTreeWidgetItem*> folders;
  folders.insert(QString(), top);
  std::function<QTreeWidgetItem*(const QString&)> ensure = [&](const QString& path) -> QTreeWidgetItem* {
    if (QTreeWidgetItem* found = folders.value(path)) {
      return found;
    }
    QTreeWidgetItem* parent = ensure(dirOf(path));
    auto* folder = new QTreeWidgetItem(parent);
    folder->setText(kName, lastPart(path));
    folder->setIcon(kName, themeIcon(QStringLiteral("folder")));
    folder->setData(kName, kTypeRole, static_cast<int>(RowType::Folder));
    folder->setData(kName, kRootRole, row.root);
    folder->setData(kName, kPathRole, path);
    folder->setData(kName, kKeyRole, QStringLiteral("%1/%2/").arg(projectKey, path));
    folder->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable | Qt::ItemIsEditable);
    folder->setToolTip(kName, QDir::toNativeSeparators(QDir(row.root).filePath(path)));
    folders.insert(path, folder);
    return folder;
  };
  for (const QString& folder : row.folders) {
    ensure(folder);
  }
  for (const ProjectDesignRow& design : row.designs) {
    auto* item = new QTreeWidgetItem(ensure(dirOf(design.path)));
    item->setText(kName, designName(design.path));
    item->setIcon(kName, themeIcon(QStringLiteral("project")));
    item->setData(kName, kTypeRole, static_cast<int>(RowType::Design));
    item->setData(kName, kRootRole, row.root);
    item->setData(kName, kPathRole, design.path);
    item->setData(kName, kKeyRole, QStringLiteral("%1/%2").arg(projectKey, design.path));
    item->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable | Qt::ItemIsEditable);
    item->setToolTip(kName, designTip(row.root, design));
    item->setToolTip(kState, designTip(row.root, design));
  }
  for (auto it = folders.cbegin(); it != folders.cend(); ++it) {
    if (!it.key().isEmpty()) {
      it.value()->setExpanded(m_expanded.value(keyOf(it.value()), true));
    }
  }
  return top;
}

void ProjectsPanel::updateMarks() {
  if (!m_building) {
    applyMarks();
  }
}

void ProjectsPanel::applyMarks() {
  const QString open = m_host.openDesign ? m_host.openDesign() : QString();
  const QString current = m_host.currentRoot ? m_host.currentRoot() : QString();
  const ProjectIndicator::Remote remote = m_host.currentRemote ? m_host.currentRemote() : ProjectIndicator::Remote();
  for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
    QTreeWidgetItem* item = *it;
    const RowType type = typeOf(item);
    if (type == RowType::Project) {
      if (const ProjectRow* row = project(rootOf(item))) {
        const bool isCurrent = !current.isEmpty() && sameRoot(row->root, current);
        item->setText(kState, projectState(*row, isCurrent ? &remote : nullptr).shown);
        QFont font = item->font(kName);
        font.setBold(isCurrent);
        item->setFont(kName, font);
      }
    } else if (type == RowType::Design) {
      if (const ProjectDesignRow* design = designOf(item)) {
        item->setText(kState, designState(*design).shown);
        const bool isOpen = !open.isEmpty() && sameRoot(QDir(rootOf(item)).filePath(design->path), open);
        QFont font = item->font(kName);
        font.setBold(isOpen);
        item->setFont(kName, font);
        item->setFont(kState, font);
      }
    }
  }
  logTree();
}

void ProjectsPanel::logTree() {
  const QString open = m_host.openDesign ? m_host.openDesign() : QString();
  const QString current = m_host.currentRoot ? m_host.currentRoot() : QString();
  const ProjectIndicator::Remote remote = m_host.currentRemote ? m_host.currentRemote() : ProjectIndicator::Remote();
  QStringList projects;
  QStringList names;
  QHash<QString, QString> lines;
  for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
    QTreeWidgetItem* item = *it;
    if (typeOf(item) != RowType::Project) {
      continue;
    }
    const ProjectRow* row = project(rootOf(item));
    if (row == nullptr) {
      continue;
    }
    const QString key = keyOf(item);
    projects << key;
    names << QStringLiteral("%1 = %2%3").arg(key, item->text(kName),
                                             row->displayName.isEmpty() ? QString() : QStringLiteral(" (set)"));
    const bool isCurrent = !current.isEmpty() && sameRoot(row->root, current);
    QStringList entries;
    for (const QString& folder : row->folders) {
      entries << folder + QLatin1Char('/');
    }
    for (const ProjectDesignRow& design : row->designs) {
      QString entry = QStringLiteral("%1 %2").arg(design.path, designState(design).logged);
      if (!open.isEmpty() && sameRoot(QDir(row->root).filePath(design.path), open)) {
        entry += QStringLiteral(", open");
      }
      entries << entry;
    }
    if (entries.isEmpty()) {
      entries << QStringLiteral("no designs");
    }
    lines.insert(key, QStringLiteral("Projects tree: %1 (%2%3): %4")
                          .arg(key, projectState(*row, isCurrent ? &remote : nullptr).logged,
                               isCurrent ? QStringLiteral(", current") : QString(),
                               entries.join(QStringLiteral(" | "))));
  }
  QString counted = projects.size() == 1 ? QStringLiteral("Projects panel: 1 project")
                                         : QStringLiteral("Projects panel: %1 projects").arg(projects.size());
  if (!projects.isEmpty()) {
    counted += QStringLiteral(": ") + projects.join(QStringLiteral(" | "));
  }
  if (counted != m_loggedProjects) {
    m_loggedProjects = counted;
    qInfo().noquote() << counted;
  }
  // The rows at the top of the tree (mitcad#173): the projects' keys, in
  // their order; nothing else.
  QStringList topRows;
  for (int i = 0; i < m_tree->topLevelItemCount(); ++i) {
    topRows << keyOf(m_tree->topLevelItem(i));
  }
  const QString top = QStringLiteral("Projects top rows: %1").arg(topRows.join(QStringLiteral(" | ")));
  if (!topRows.isEmpty() && top != m_loggedTopRows) {
    m_loggedTopRows = top;
    qInfo().noquote() << top;
  }
  // The names shown (mitcad#152, #173): `Shop = Shop`, a Cloud project's
  // `gripper = github.com/acme/gripper`, `(set)` for a display name.
  const QString named = QStringLiteral("Projects names: %1").arg(names.join(QStringLiteral(" | ")));
  if (!names.isEmpty() && named != m_loggedNames) {
    m_loggedNames = named;
    qInfo().noquote() << named;
  }
  for (auto it = lines.cbegin(); it != lines.cend(); ++it) {
    if (m_loggedTrees.value(it.key()) != it.value()) {
      m_loggedTrees.insert(it.key(), it.value());
      qInfo().noquote() << it.value();
    }
  }
}

void ProjectsPanel::scheduleLayoutLog() {
  if (m_layoutLogPending) {
    return;
  }
  m_layoutLogPending = true;
  // Once the layout has placed the rows (UI tests click there).
  TestSync::singleShot(50, this, [this] {
    m_layoutLogPending = false;
    logLayout();
  });
}

void ProjectsPanel::logLayout() {
  if (!isVisible()) {
    return;
  }
  QStringList lines;
  const QRect viewport = m_tree->viewport()->rect();
  for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
    const QRect row = m_tree->visualItemRect(*it);
    if (row.isEmpty() || !viewport.intersects(row)) {
      continue; // in a collapsed folder or scrolled away
    }
    const QPoint point = GlassCard::mapToHost(m_tree->viewport(), row.center());
    lines << QStringLiteral("Projects row %1 at %2,%3").arg(keyOf(*it)).arg(point.x()).arg(point.y());
  }
  for (QAction* action : m_toolbar->actions()) {
    QWidget* button = m_toolbar->widgetForAction(action);
    if (button == nullptr || !button->isVisible()) {
      continue;
    }
    const QPoint point = GlassCard::mapToHost(button, button->rect().center());
    lines << QStringLiteral("Projects button %1 at %2,%3").arg(action->text()).arg(point.x()).arg(point.y());
  }
  const QString logged = lines.join(QLatin1Char('\n'));
  if (logged != m_loggedLayout) {
    m_loggedLayout = logged;
    for (const QString& line : std::as_const(lines)) {
      qDebug().noquote() << line;
    }
  }
}

int ProjectsPanel::contentHeight() const {
  int rows = 0;
  for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
    bool shown = true;
    for (QTreeWidgetItem* up = (*it)->parent(); up != nullptr && shown; up = up->parent()) {
      shown = up->isExpanded();
    }
    rows += shown ? 1 : 0;
  }
  const int rowHeight = std::max(m_tree->sizeHintForRow(0), fontMetrics().height() + 6);
  return m_toolbar->sizeHint().height() + 2 + std::max(rows, 2) * rowHeight + 2 * m_tree->frameWidth() + 4;
}

void ProjectsPanel::updateActions() {
  QTreeWidgetItem* item = m_tree->currentItem();
  const RowType type = typeOf(item);
  const ProjectRow* row = project(rootOf(item));
  const bool healthy = row != nullptr && (row->kind == ProjectRow::Kind::Local || row->kind == ProjectRow::Kind::Cloud);
  const bool anyProject = healthy || (m_host.currentRoot && !m_host.currentRoot().isEmpty());
  m_newDesign->setEnabled(anyProject && (healthy || row == nullptr));
  m_newFolder->setEnabled(anyProject && (healthy || row == nullptr));
  m_rename->setEnabled(healthy && (type == RowType::Design || type == RowType::Folder));
  m_delete->setEnabled(healthy && (type == RowType::Design || type == RowType::Folder));
  m_history->setEnabled(healthy && type == RowType::Design);
  m_sync->setEnabled(healthy && row->kind == ProjectRow::Kind::Cloud);
  m_settings->setEnabled(!settingsRoot(item).isEmpty());
}

QString ProjectsPanel::settingsRoot(const QTreeWidgetItem* item) const {
  if (item == nullptr || typeOf(item) == RowType::NewItem) {
    return {};
  }
  const ProjectRow* row = project(rootOf(item));
  const bool healthy = row != nullptr && (row->kind == ProjectRow::Kind::Local || row->kind == ProjectRow::Kind::Cloud);
  return healthy ? row->root : QString();
}

bool ProjectsPanel::settingsOnDoubleClick(QTreeWidgetItem* item) {
  if (typeOf(item) != RowType::Project) {
    return false;
  }
  const QString root = settingsRoot(item);
  const ProjectRow* row = project(root);
  if (row == nullptr || (row->kind != ProjectRow::Kind::Cloud && !row->noRemoteChosen)) {
    return false; // a Local project expands
  }
  m_tree->setCurrentItem(item);
  qInfo().noquote() << QStringLiteral("Projects panel: settings of %1 (double-click on %2)").arg(root, keyOf(item));
  emit projectSettingsRequested(root);
  return true;
}

QString ProjectsPanel::stateText(const QString& root) const {
  const ProjectRow* row = project(root);
  if (row == nullptr) {
    return {};
  }
  const QString current = m_host.currentRoot ? m_host.currentRoot() : QString();
  const bool isCurrent = !current.isEmpty() && sameRoot(row->root, current);
  const ProjectIndicator::Remote remote = m_host.currentRemote ? m_host.currentRemote() : ProjectIndicator::Remote();
  return projectState(*row, isCurrent ? &remote : nullptr).shown;
}

// ---------------------------------------------------------------------------
// Rows

ProjectsPanel::RowType ProjectsPanel::typeOf(const QTreeWidgetItem* item) const {
  return item == nullptr ? RowType::NewItem : static_cast<RowType>(item->data(kName, kTypeRole).toInt());
}

QString ProjectsPanel::rootOf(const QTreeWidgetItem* item) const {
  return item == nullptr ? QString() : item->data(kName, kRootRole).toString();
}

QString ProjectsPanel::pathOf(const QTreeWidgetItem* item) const {
  return item == nullptr ? QString() : item->data(kName, kPathRole).toString();
}

QString ProjectsPanel::keyOf(const QTreeWidgetItem* item) const {
  return item == nullptr ? QString() : item->data(kName, kKeyRole).toString();
}

QTreeWidgetItem* ProjectsPanel::projectItem(const QString& root) const {
  for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
    if (typeOf(*it) == RowType::Project && sameRoot(rootOf(*it), root)) {
      return *it;
    }
  }
  return nullptr;
}

QTreeWidgetItem* ProjectsPanel::folderItem(const QString& root, const QString& folder) const {
  if (folder.isEmpty()) {
    return projectItem(root);
  }
  for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
    if (typeOf(*it) == RowType::Folder && sameRoot(rootOf(*it), root) && pathOf(*it) == folder) {
      return *it;
    }
  }
  return nullptr;
}

const ProjectRow* ProjectsPanel::project(const QString& root) const {
  if (root.isEmpty()) {
    return nullptr;
  }
  for (const ProjectRow& row : m_rows) {
    if (sameRoot(row.root, root)) {
      return &row;
    }
  }
  return nullptr;
}

const ProjectDesignRow* ProjectsPanel::designOf(const QTreeWidgetItem* item) const {
  const ProjectRow* row = project(rootOf(item));
  if (row == nullptr || typeOf(item) != RowType::Design) {
    return nullptr;
  }
  const QString path = pathOf(item);
  for (const ProjectDesignRow& design : row->designs) {
    if (design.path == path) {
      return &design;
    }
  }
  return nullptr;
}

QString ProjectsPanel::folderFor(const QTreeWidgetItem* item) const {
  switch (typeOf(item)) {
  case RowType::Folder:
  case RowType::NewItem:
    return pathOf(item);
  case RowType::Design:
    return dirOf(pathOf(item));
  case RowType::Project:
    break;
  }
  return {};
}

QString ProjectsPanel::selectedRoot() const { return rootOf(m_tree->currentItem()); }

QString ProjectsPanel::selectedFolder() const { return folderFor(m_tree->currentItem()); }

void ProjectsPanel::reveal(const QString& root) {
  if (root.isEmpty()) {
    return;
  }
  m_revealed.insert(QDir::cleanPath(root));
  if (QTreeWidgetItem* item = projectItem(root)) {
    for (QTreeWidgetItem* up = item; up != nullptr; up = up->parent()) {
      up->setExpanded(true);
    }
    m_tree->setCurrentItem(item);
    m_tree->scrollToItem(item);
  } else {
    m_reveal = QDir::cleanPath(root);
  }
  refresh();
}

// ---------------------------------------------------------------------------
// Names typed in place

void ProjectsPanel::startNewDesign(const QString& root, const QString& folder) { startNew(false, root, folder); }

void ProjectsPanel::startNewFolder(const QString& root, const QString& folder) { startNew(true, root, folder); }

void ProjectsPanel::startNew(bool folder, const QString& root, const QString& parent) {
  if (m_isEditing) {
    cancelEditing();
  }
  QTreeWidgetItem* host = folderItem(root, parent);
  if (host == nullptr || m_task != nullptr) {
    // Not read yet (a project just made): named once it is.
    m_pending = {folder, QDir::cleanPath(root), parent};
    m_hasPending = true;
    m_revealed.insert(QDir::cleanPath(root));
    m_reveal = QDir::cleanPath(root);
    if (m_task != nullptr) {
      m_readAgain = true;
    } else {
      refresh();
    }
    return;
  }
  // A free name among the folder's rows.
  QStringList taken;
  int firstDesign = host->childCount();
  for (int i = 0; i < host->childCount(); ++i) {
    taken << host->child(i)->text(kName).toLower();
    if (typeOf(host->child(i)) == RowType::Design && firstDesign == host->childCount()) {
      firstDesign = i;
    }
  }
  const QString base = folder ? tr("Folder") : tr("Design");
  int n = 1;
  while (taken.contains(QStringLiteral("%1%2").arg(base).arg(n).toLower())) {
    ++n;
  }
  for (QTreeWidgetItem* up = host; up != nullptr; up = up->parent()) {
    up->setExpanded(true);
  }
  auto* item = new QTreeWidgetItem;
  if (folder) {
    host->insertChild(firstDesign, item);
  } else {
    host->addChild(item);
  }
  item->setText(kName, QStringLiteral("%1%2").arg(base).arg(n));
  item->setIcon(kName, themeIcon(folder ? QStringLiteral("folder") : QStringLiteral("project")));
  item->setData(kName, kTypeRole, static_cast<int>(RowType::NewItem));
  item->setData(kName, kRootRole, rootOf(host));
  item->setData(kName, kPathRole, parent);
  item->setData(kName, kKeyRole, QStringLiteral("%1 (new)").arg(keyOf(host)));
  item->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable | Qt::ItemIsEditable);
  m_editing = Editing();
  m_editing.kind = RowType::NewItem;
  m_editing.folder = folder;
  m_editing.root = rootOf(host);
  m_editing.parent = parent;
  m_editing.item = item;
  m_isEditing = true;
  m_tree->setCurrentItem(item);
  m_tree->scrollToItem(item);
  const QString line = QStringLiteral("Projects panel: naming a new %1 in %2")
                           .arg(folder ? QStringLiteral("folder") : QStringLiteral("design"), keyOf(host));
  openEditor(item, line);
  emit contentHeightChanged();
  scheduleLayoutLog();
}

void ProjectsPanel::openEditor(QTreeWidgetItem* item, const QString& line) {
  // After the menu or the dialog that asked for it has closed and given the
  // focus back, so that the editor keeps it.
  TestSync::singleShot(0, this, [this, item, line] {
    if (!m_isEditing || m_editing.item != item) {
      return;
    }
    m_tree->setCurrentItem(item);
    m_tree->scrollToItem(item);
    m_tree->setFocus(Qt::OtherFocusReason);
    m_tree->editItem(item, kName);
    qInfo().noquote() << line;
    scheduleLayoutLog();
  });
}

void ProjectsPanel::startRename(QTreeWidgetItem* item) {
  const RowType type = typeOf(item);
  if (type != RowType::Design && type != RowType::Folder) {
    return;
  }
  if (m_isEditing) {
    cancelEditing();
  }
  m_editing = Editing();
  m_editing.kind = type;
  m_editing.folder = type == RowType::Folder;
  m_editing.root = rootOf(item);
  m_editing.parent = dirOf(pathOf(item));
  m_editing.from = pathOf(item);
  m_editing.item = item;
  m_isEditing = true;
  m_tree->setCurrentItem(item);
  openEditor(item, QStringLiteral("Projects panel: renaming %1").arg(keyOf(item)));
}

bool ProjectsPanel::cancelEditing() {
  if (!m_isEditing) {
    return false;
  }
  // The editor, if it is open already.
  m_tree->cancelEdit();
  editingClosed(false);
  return true;
}

void ProjectsPanel::editingClosed(bool accepted) {
  if (!m_isEditing) {
    return;
  }
  const Editing editing = m_editing;
  m_isEditing = false;
  m_editing = Editing();
  QString name = editing.item != nullptr ? editing.item->text(kName).trimmed() : QString();
  if (editing.kind == RowType::NewItem && editing.item != nullptr) {
    delete editing.item; // the project's tree read again shows what was made
  }
  const QString what = editing.kind == RowType::NewItem
                           ? (editing.folder ? QStringLiteral("new folder") : QStringLiteral("new design"))
                           : QStringLiteral("rename of %1").arg(editing.from);
  if (!accepted) {
    qInfo().noquote() << QStringLiteral("Projects panel: %1 cancelled").arg(what);
    if (editing.kind != RowType::NewItem && editing.item != nullptr) {
      editing.item->setText(kName, editing.folder ? lastPart(editing.from) : designName(editing.from));
    }
    applyWaitingRows();
    return;
  }
  if (!editing.folder && name.endsWith(kSuffix, Qt::CaseInsensitive)) {
    name.chop(kSuffix.size());
    name = name.trimmed();
  }
  static const QString forbidden = QStringLiteral("/\\<>:\"|?*");
  bool bad = name.isEmpty() || name.startsWith(QLatin1Char('.')) || name.endsWith(QLatin1Char('.'));
  for (const QChar c : name) {
    bad = bad || forbidden.contains(c) || c.category() == QChar::Other_Control;
  }
  if (bad) {
    qInfo().noquote() << QStringLiteral("Projects panel: %1 refused: the name \"%2\"").arg(what, name);
    emit statusMessage(tr("\"%1\" cannot be a name: it may not be empty, start or end with a dot, or hold "
                          "/ \\ < > : \" | ? *.")
                           .arg(name),
                       true);
    if (editing.kind != RowType::NewItem && editing.item != nullptr) {
      editing.item->setText(kName, editing.folder ? lastPart(editing.from) : designName(editing.from));
    }
    applyWaitingRows();
    return;
  }
  const QString file = editing.folder ? name : name + kSuffix;
  if (editing.kind == RowType::NewItem) {
    const QString path = joinPath(editing.parent, file);
    qInfo().noquote() << QStringLiteral("Projects panel: %1 %2 in %3").arg(what, path, editing.root);
    applyWaitingRows();
    if (editing.folder) {
      emit newFolderRequested(editing.root, path);
    } else {
      emit newDesignRequested(editing.root, path);
    }
    return;
  }
  const QString to = joinPath(editing.parent, file);
  applyWaitingRows();
  if (to == editing.from) {
    qInfo().noquote() << QStringLiteral("Projects panel: %1: the same name").arg(what);
    return;
  }
  qInfo().noquote() << QStringLiteral("Projects panel: rename %1 to %2").arg(editing.from, to);
  emit moveRequested(editing.root, editing.from, to, editing.folder);
}

// ---------------------------------------------------------------------------
// Actions

void ProjectsPanel::openCurrent() {
  QTreeWidgetItem* item = m_tree->currentItem();
  switch (typeOf(item)) {
  case RowType::Design:
    qInfo().noquote() << QStringLiteral("Projects panel: open %1").arg(keyOf(item));
    emit openRequested(QDir::cleanPath(QDir(rootOf(item)).filePath(pathOf(item))));
    break;
  case RowType::Project:
    if (const ProjectRow* row = project(rootOf(item)); row != nullptr && row->kind == ProjectRow::Kind::NoHistory) {
      emit openProjectRequested(row->root);
      break;
    }
    item->setExpanded(!item->isExpanded());
    break;
  case RowType::Folder:
    item->setExpanded(!item->isExpanded());
    break;
  case RowType::NewItem:
    break;
  }
}

void ProjectsPanel::renameCurrent() {
  QTreeWidgetItem* item = m_tree->currentItem();
  const ProjectRow* row = project(rootOf(item));
  if (row != nullptr && (row->kind == ProjectRow::Kind::Local || row->kind == ProjectRow::Kind::Cloud)) {
    startRename(item);
  }
}

void ProjectsPanel::deleteCurrent() {
  QTreeWidgetItem* item = m_tree->currentItem();
  const RowType type = typeOf(item);
  const ProjectRow* row = project(rootOf(item));
  if (row == nullptr || (row->kind != ProjectRow::Kind::Local && row->kind != ProjectRow::Kind::Cloud) ||
      (type != RowType::Design && type != RowType::Folder)) {
    return;
  }
  qInfo().noquote() << QStringLiteral("Projects panel: delete %1").arg(keyOf(item));
  emit deleteRequested(rootOf(item), pathOf(item), type == RowType::Folder);
}

void ProjectsPanel::contextMenu(const QPoint& at) {
  QTreeWidgetItem* item = m_tree->itemAt(at);
  if (item != nullptr && item != m_tree->currentItem()) {
    m_tree->setCurrentItem(item); // a right click selects the row
  }
  LoggedMenu menu(this);
  const RowType type = typeOf(item);
  const QString root = rootOf(item);
  const ProjectRow* row = project(root);
  const bool healthy = row != nullptr && (row->kind == ProjectRow::Kind::Local || row->kind == ProjectRow::Kind::Cloud);
  const QString folder = folderFor(item);
  if (item != nullptr && type == RowType::Design && healthy) {
    menu.add(QString(), tr("Open"), [this] { openCurrent(); });
    menu.add(QStringLiteral("edit"), tr("Rename"), [this] { renameCurrent(); });
    menu.add(QStringLiteral("delete"), tr("Delete"), [this] { deleteCurrent(); });
    menu.add(QStringLiteral("version-history"), tr("Version History..."),
             [this, root, path = pathOf(item)] { emit versionHistoryRequested(root, path); });
    menu.add(QString(), tr("Show in Folder"),
             [this, root, folder] { emit showInFolderRequested(QDir(root).filePath(folder)); });
    menu.separator();
  }
  if (item != nullptr && type == RowType::Folder && healthy) {
    menu.add(QStringLiteral("new-design"), tr("New Design"), [this, root, folder] { startNewDesign(root, folder); });
    menu.add(QStringLiteral("folder"), tr("New Folder"), [this, root, folder] { startNewFolder(root, folder); });
    menu.add(QStringLiteral("edit"), tr("Rename"), [this] { renameCurrent(); });
    menu.add(QStringLiteral("delete"), tr("Delete"), [this] { deleteCurrent(); });
    menu.add(QString(), tr("Show in Folder"),
             [this, root, folder] { emit showInFolderRequested(QDir(root).filePath(folder)); });
    menu.exec(m_tree->viewport()->mapToGlobal(at));
    return;
  }
  if (item != nullptr && type == RowType::Design && healthy) {
    menu.add(QStringLiteral("new-design"), tr("New Design"), [this, root, folder] { startNewDesign(root, folder); });
    menu.add(QStringLiteral("folder"), tr("New Folder"), [this, root, folder] { startNewFolder(root, folder); });
    menu.exec(m_tree->viewport()->mapToGlobal(at));
    return;
  }
  if (item != nullptr && type == RowType::Project && row != nullptr) {
    if (healthy) {
      menu.add(QStringLiteral("new-design"), tr("New Design"), [this, root] { startNewDesign(root, QString()); });
      menu.add(QStringLiteral("folder"), tr("New Folder"), [this, root] { startNewFolder(root, QString()); });
      menu.add(QString(), tr("Show in Folder"), [this, root] { emit showInFolderRequested(root); });
      menu.add(QStringLiteral("settings"), tr("Project Settings..."), [this, root] { emit projectSettingsRequested(root); });
      if (row->kind == ProjectRow::Kind::Cloud) {
        menu.add(QStringLiteral("sync"), tr("Sync Now"), [this, root] { emit syncRequested(root); });
      }
    } else if (row->kind == ProjectRow::Kind::NoHistory) {
      menu.add(QString(), tr("Open Project"), [this, root] { emit openProjectRequested(root); });
      menu.add(QString(), tr("Show in Folder"), [this, root] { emit showInFolderRequested(root); });
    }
    menu.separator();
    menu.add(QString(), tr("Remove from List"), [this, root] { emit removeFromListRequested(root); });
  }
  menu.separator();
  menu.add(QStringLiteral("new-project"), tr("New Project..."),
           [this] { emit commandRequested(QStringLiteral("file.new_project")); });
  menu.add(QStringLiteral("remote-open"), tr("Open from Cloud..."),
           [this] { emit commandRequested(QStringLiteral("file.open_remote")); });
  menu.add(QStringLiteral("folder"), tr("Add Existing Project Folder..."), [this] { emit addFolderRequested(); });
  menu.exec(m_tree->viewport()->mapToGlobal(at));
}

bool ProjectsPanel::dropTarget(const QTreeWidgetItem* dragged, const QTreeWidgetItem* over, QString* folder) const {
  if (dragged == nullptr || over == nullptr || dragged == over) {
    return false;
  }
  const RowType type = typeOf(dragged);
  if (type != RowType::Design && type != RowType::Folder) {
    return false;
  }
  const QString root = rootOf(dragged);
  if (root.isEmpty() || !sameRoot(rootOf(over), root)) {
    return false; // not between projects (copying is a later step)
  }
  QString target;
  switch (typeOf(over)) {
  case RowType::Project:
    break;
  case RowType::Folder:
    target = pathOf(over);
    break;
  case RowType::Design:
    target = dirOf(pathOf(over));
    break;
  case RowType::NewItem:
    return false;
  }
  const QString from = pathOf(dragged);
  if (dirOf(from) == target) {
    return false; // where it is
  }
  if (type == RowType::Folder && (target == from || target.startsWith(from + QLatin1Char('/')))) {
    return false; // into itself
  }
  if (folder != nullptr) {
    *folder = target;
  }
  return true;
}

void ProjectsPanel::dropped(QTreeWidgetItem* dragged, QTreeWidgetItem* over) {
  QString folder;
  if (!dropTarget(dragged, over, &folder)) {
    qInfo().noquote() << QStringLiteral("Projects panel: drop of %1 refused%2")
                             .arg(keyOf(dragged), over != nullptr ? QStringLiteral(" on %1").arg(keyOf(over))
                                                                  : QString());
    if (over != nullptr && dragged != nullptr && !rootOf(over).isEmpty() && !sameRoot(rootOf(over), rootOf(dragged))) {
      emit statusMessage(tr("Designs and folders move only within their project."), false);
    }
    return;
  }
  const QString from = pathOf(dragged);
  const QString to = joinPath(folder, lastPart(from));
  qInfo().noquote() << QStringLiteral("Projects panel: drag %1 to %2").arg(from, to);
  emit moveRequested(rootOf(dragged), from, to, typeOf(dragged) == RowType::Folder);
}

// ---------------------------------------------------------------------------
// A design revealed from outside the panel (mitcad#174)

void ProjectsPanel::revealDesign(const QString& root, const QString& path) {
  if (root.isEmpty() || path.isEmpty()) {
    return;
  }
  m_revealDesignRoot = QDir::cleanPath(root);
  m_revealDesignPath = path;
  // Each rebuild ends with contentHeightChanged: the design is selected
  // once a read lists it.
  connect(this, &ProjectsPanel::contentHeightChanged, this, &ProjectsPanel::selectRevealedDesign,
          Qt::UniqueConnection);
  reveal(root);
  selectRevealedDesign();
}

void ProjectsPanel::selectRevealedDesign() {
  if (m_revealDesignPath.isEmpty() || m_building) {
    return;
  }
  for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
    QTreeWidgetItem* item = *it;
    if (typeOf(item) != RowType::Design || pathOf(item) != m_revealDesignPath ||
        !sameRoot(rootOf(item), m_revealDesignRoot)) {
      continue;
    }
    m_revealDesignRoot.clear();
    m_revealDesignPath.clear();
    disconnect(this, &ProjectsPanel::contentHeightChanged, this, &ProjectsPanel::selectRevealedDesign);
    for (QTreeWidgetItem* up = item->parent(); up != nullptr; up = up->parent()) {
      up->setExpanded(true);
    }
    m_tree->setCurrentItem(item);
    m_tree->scrollToItem(item);
    return;
  }
}

} // namespace mitcad
