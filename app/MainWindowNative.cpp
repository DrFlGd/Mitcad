// SPDX-License-Identifier: MIT
#include "MainWindow.hpp"

#include <functional>
#include <stdexcept>
#include <utility>

#include <QAction>
#include <QApplication>
#include <QCloseEvent>
#include <QElapsedTimer>
#include <QEvent>
#include <QFileInfo>
#include <QLabel>
#include <QMenu>
#include <QMenuBar>
#include <QObject>
#include <QPalette>
#include <QSet>
#include <QStackedWidget>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QtLogging>

#include "browser/BrowserController.hpp"
#include "browser/TimelineWidget.hpp"
#include "files/ProjectIndicator.hpp"
#include "files/LockController.hpp"
#include "framework/CommandRegistry.hpp"
#include "framework/ComputeProgress.hpp"
#include "framework/Dialogs.hpp"
#include "framework/Icons.hpp"
#include "framework/Ribbon.hpp"
#include "platform/NativeDialogs.hpp"
#include "platform/NativeForms.hpp"
#include "platform/NativeMacWindow.hpp"
#include "report/ReportCenter.hpp"
#include "sketch/SketchPalette.hpp"

namespace mitcad {
namespace {

// AppKit holds rendered icon images, so a Qt palette change must replace
// those images after Qt finishes updating the application's palette.
class NativePaletteWatcher : public QObject {
public:
  NativePaletteWatcher(QWidget* root, std::function<void()> refresh)
      : QObject(root), m_root(root), m_refresh(std::move(refresh)), m_palette(QApplication::palette()) {
    root->installEventFilter(this);
    qApp->installEventFilter(this);
  }

  bool eventFilter(QObject* watched, QEvent* event) override {
    if ((watched == m_root || watched == qApp) && !m_pending &&
        (event->type() == QEvent::ApplicationPaletteChange || event->type() == QEvent::PaletteChange)) {
      m_pending = true;
      QTimer::singleShot(0, this, [this] {
        m_pending = false;
        const QPalette palette = QApplication::palette();
        if (palette == m_palette) {
          return;
        }
        m_palette = palette;
        m_refresh();
      });
    }
    return false;
  }

private:
  QWidget* m_root;
  std::function<void()> m_refresh;
  QPalette m_palette;
  bool m_pending = false;
};

QString menuLabel(QString text) {
  text.remove(QLatin1Char('&'));
  return text;
}

QString symbolForAction(const QString& id) {
  static const QHash<QString, QString> symbols = {
      {QStringLiteral("edit.undo"), QStringLiteral("arrow.uturn.backward")},
      {QStringLiteral("edit.redo"), QStringLiteral("arrow.uturn.forward")},
      {QStringLiteral("file.new"), QStringLiteral("doc.badge.plus")},
      {QStringLiteral("file.open"), QStringLiteral("folder")},
      {QStringLiteral("file.save"), QStringLiteral("square.and.arrow.down")},
      {QStringLiteral("view.fit"), QStringLiteral("arrow.up.left.and.arrow.down.right")},
      {QStringLiteral("view.home"), QStringLiteral("house")},
      {QStringLiteral("tools.search"), QStringLiteral("magnifyingglass")},
      {QStringLiteral("tools.preferences"), QStringLiteral("gearshape")},
      {QStringLiteral("sketch.create"), QStringLiteral("pencil.and.outline")},
      {QStringLiteral("sketch.finish"), QStringLiteral("checkmark")}};
  return symbols.value(id);
}

QString symbolForNode(const BrowserNode& node) {
  switch (node.type) {
  case BrowserNode::Type::Body: return QStringLiteral("cube");
  case BrowserNode::Type::Component: return QStringLiteral("square.stack.3d.up");
  case BrowserNode::Type::Sketch: return QStringLiteral("pencil.and.outline");
  case BrowserNode::Type::Settings:
  case BrowserNode::Type::Units: return QStringLiteral("gearshape");
  case BrowserNode::Type::NamedView: return QStringLiteral("viewfinder");
  case BrowserNode::Type::Joint: return QStringLiteral("link");
  default: return node.isFolder() ? QStringLiteral("folder") : QStringLiteral("square.dashed");
  }
}

} // namespace

void MainWindow::createNativeWindow() {
  m_viewer->setNativeControls(true);
  mac::NativeMacWindowCallbacks callbacks;
  callbacks.cancelPointerInteraction = [this] { m_viewer->cancelPointerInteraction(); };
  callbacks.action = [this](const QString& id) {
    // AppKit may be dispatching an action from a menu that a command
    // rebuilds. Run it after that dispatch and the viewport's paint.
    QTimer::singleShot(0, this, [this, id] { nativeAction(id); });
  };
  callbacks.confirm = [this] {
    if (!m_confirmAction->isEnabled()) {
      return false;
    }
    QTimer::singleShot(0, this, [this] {
      if (!m_session && modelBusy()) {
        whenIdle(this, [this] { confirm(); });
      } else {
        confirm();
      }
    });
    return true;
  };
  callbacks.cancel = [this] {
    QTimer::singleShot(0, this, [this] { cancel(); });
    return true;
  };
  callbacks.selection = [this](const QStringList& keys) {
    Selection items;
    for (const QString& key : keys) {
      const SelectionItem item = m_nativeNodes.value(key).item();
      if (item.isValid()) {
        items.append(item);
      }
    }
    emit m_controller->browser()->itemsPicked(items);
  };
  callbacks.activate = [this](const QString& key) {
    const BrowserNode node = m_nativeNodes.value(key);
    if (node.isValid()) {
      emit m_controller->browser()->doubleClicked(node);
    }
  };
  callbacks.rename = [this](const QString& key, const QString& name) {
    const BrowserNode node = m_nativeNodes.value(key);
    if (node.isValid()) {
      emit m_controller->browser()->renamed(node, name);
    }
  };
  callbacks.treeVisibility = [this](const QString& key) {
    const BrowserNode node = m_nativeNodes.value(key);
    if (node.hasEye) {
      emit m_controller->browser()->eyeClicked(node);
    }
  };
  callbacks.treeContextMenu = [this](const QString& key, const QPoint& point) {
    const BrowserNode node = m_nativeNodes.value(key);
    if (node.isValid()) {
      emit m_controller->browser()->contextMenuRequested(node, point);
    }
  };
  callbacks.historyActivated = [this](const QString& uid) {
    emit m_controller->timeline()->editRequested(uid);
  };
  callbacks.historySelection = [this](const QStringList& uids) {
    m_controller->timeline()->setSelectedFeatures(uids);
    emit m_controller->timeline()->featuresClicked(uids);
  };
  callbacks.historyMarkerChanged = [this](int marker) {
    emit m_controller->timeline()->markerRequested(marker);
  };
  callbacks.historyCanChange = [this] { return !modelBusy() && canChangeModel(); };
  callbacks.historyMarkerDragStarted = [this] {
    if (!modelBusy() && canChangeModel()) {
      emit m_controller->timeline()->markerDragStarted();
    }
  };
  callbacks.historyMarkerDragged = [this](int marker) {
    emit m_controller->timeline()->markerDragged(marker);
  };
  callbacks.historyMarkerDragFinished = [this](int marker) {
    emit m_controller->timeline()->markerDragFinished(marker);
  };
  callbacks.historyReordered = [this](const QString& uid, int position) {
    emit m_controller->timeline()->reorderRequested(uid, position);
  };
  callbacks.historyContextMenu = [this](const QString& uid, const QPoint& point) {
    emit m_controller->timeline()->contextMenuRequested(uid, point);
  };
  callbacks.historyFilterMenu = [this](const QPoint& point) {
    m_controller->timeline()->showFilterMenu(point);
  };
  callbacks.projectMenu = [this](const QPoint& point) {
    if (!m_indicator || !m_indicator->menu()) {
      return;
    }
    execMenu(m_indicator->menu(), point);
  };
  callbacks.requestClose = [this] { return requestNativeClose(); };
  callbacks.closed = [] { QCoreApplication::quit(); };
  callbacks.openFile = [this](const QString& path) {
    whenIdle(this, [this, path] { openExternalFile(path); });
  };
  callbacks.viewportOverlays = [this](const QMargins& insets, const std::vector<QRect>& covered) {
    m_viewer->setOverlayInsets(insets);
    m_viewer->setCoveredRects(covered);
  };
  m_nativeWindow = std::make_unique<NativeMacWindow>(this, std::move(callbacks));
  if (!m_nativeWindow->isEmbedded()) {
    throw std::runtime_error(m_nativeWindow->embeddingError().toStdString());
  }
  setNativeDialogWindow(m_nativeWindow->nativeWindow());
  new NativePaletteWatcher(this, [this] {
    m_nativeSnapshotKey.clear();
    refreshNativeWindow(); // A busy document refreshes from busyChanged after it becomes idle.
  });
  connect(m_controller->timeline(), &TimelineWidget::filterChanged, this, [this] {
    m_nativeSnapshotKey.clear();
    refreshNativeWindow();
  }, Qt::QueuedConnection);
  connect(m_indicator, &ProjectIndicator::presentationChanged, this, [this] {
    if (m_nativeWindow) {
      m_nativeWindow->setProjectStatus(m_indicator->shownText(), m_indicator->toolTip());
    }
  });
  m_nativeWindow->setProjectStatus(m_indicator->shownText(), m_indicator->toolTip());
  // Notices remain shared state, but their Qt toolbar does not enter
  // AppKit's viewport. The native status and menus expose their actions.
  auto* noticeStore = new QWidget(this);
  noticeStore->hide();
  for (auto* toolbar : findChildren<QToolBar*>(QString(), Qt::FindDirectChildrenOnly)) {
    removeToolBar(toolbar);
    toolbar->setParent(noticeStore);
    toolbar->hide();
  }
  connect(m_documentSession, &DocumentSession::busyChanged, this, [this](bool busy) {
    refreshNativeActions();
    if (!busy) {
      QTimer::singleShot(0, this, [this] {
        if (!modelBusy()) {
          refreshNativeWindow();
        }
      });
    }
  });
  connect(m_documentSession, &DocumentSession::jobStarted, this,
          [this](const QString& label, const QString& stage) {
    if (!m_documentSession->asynchronousJob() || label == QStringLiteral("preview") ||
        (label == QStringLiteral("inspection") && m_session)) {
      return;
    }
    const quint64 generation = ++m_nativeJobGeneration;
    QTimer::singleShot(400, this, [this, generation, label, stage] {
      if (generation != m_nativeJobGeneration || !m_documentSession->asynchronousJob()) {
        return;
      }
      m_nativeProgress = new ComputeProgress(this);
      connect(m_nativeProgress, &ComputeProgress::cancelRequested, m_documentSession,
              &DocumentSession::cancelJob);
      if (ModelJob* job = m_documentSession->activeJob()) {
        m_nativeProgress->showProgress(stage, job->control().progress(), 400);
      }
      showDialog(m_nativeProgress);
      qDebug().noquote() << QStringLiteral("Progress dialog shown: %1").arg(label);
    });
  });
  connect(m_documentSession, &DocumentSession::jobProgress, this,
          [this](const QString& stage, quint64, quint64, qint64 elapsed) {
    if (m_nativeProgress) {
      if (ModelJob* job = m_documentSession->activeJob()) {
        m_nativeProgress->showProgress(stage, job->control().progress(), elapsed);
      }
    }
  });
  connect(m_documentSession, &DocumentSession::jobFinished, this, [this] {
    ++m_nativeJobGeneration;
    if (m_nativeProgress) {
      delete m_nativeProgress.data();
    }
  });
  refreshNativeWindow();
  qInfo() << "Native UI: AppKit shell with embedded OCCT viewport";
}

void MainWindow::nativeAction(const QString& id) {
  if (id == QStringLiteral("tools.search")) {
    m_nativeWindow->showCommandSearch();
    return;
  }
  if (id == QStringLiteral("native.quit")) {
    m_nativeWindow->close();
    return;
  }
  if (modelBusy()) {
    whenIdle(this, [this, id] { nativeAction(id); });
    return;
  }
  if (const QPointer<QAction> action = m_nativeActions.value(id); action && action->isEnabled()) {
    action->trigger();
  } else if (m_registry->find(id)) {
    trigger(id);
  }
  if (!modelBusy()) {
    refreshNativeWindow();
  }
}

void MainWindow::refreshNativeActions() {
  if (!m_nativeWindow) {
    return;
  }
  QList<mac::NativeMacAction> actions;
  QSet<QAction*> assigned;
  QHash<QString, int> commandOrder;
  int order = 0;
  for (const auto& def : m_registry->commands()) {
    commandOrder.insert(def->id, order++);
  }
  m_nativeActions.clear();
  const auto append = [this, &actions, &assigned, &commandOrder](QAction* action, const QString& menu) {
    if (action->isSeparator()) {
      mac::NativeMacAction separator;
      separator.menu = menu;
      separator.separator = true;
      actions.append(separator);
      return;
    }
    if (!action->isVisible()) {
      return;
    }
    if (action->menuRole() == QAction::QuitRole) {
      assigned.insert(action);
      return; // AppKit's Quit action passes through the native close policy.
    }
    mac::NativeMacAction item;
    item.id = action->objectName().startsWith(QStringLiteral("command_"))
                  ? action->objectName().mid(8)
                  : QStringLiteral("native.action.%1").arg(reinterpret_cast<quintptr>(action), 0, 16);
    item.label = menuLabel(action->text());
    item.menu = menu;
    const bool fileMenu = menu == QStringLiteral("File") || menu.startsWith(QStringLiteral("File/"));
    item.enabled = action->isEnabled() && !modelBusy() && !(m_session && fileMenu);
    item.checked = action->isChecked();
    item.tooltip = action->toolTip();
    // The ribbon and Sketch Palette share the same coloured CAD artwork,
    // including its palette-dependent light/dark rendering.
    item.icon = action->icon();
    item.iconTemplate = false;
    item.symbol = symbolForAction(item.id);
    item.shortcut = action->shortcut().toString(QKeySequence::PortableText);
    if (const CommandDef* def = m_registry->find(item.id)) {
      item.group = def->group;
      item.workspace = def->tab;
      item.keywords = def->keywords;
      item.toolbarPinned = def->pinned;
      item.toolbarOrder = commandOrder.value(def->id);
      if (!item.workspace.isEmpty()) {
        item.symbol.clear(); // Keep the familiar CAD command drawing.
      }
      item.shortcut = m_registry->shortcut(item.id).toString(QKeySequence::PortableText);
    }
    if (action->menuRole() == QAction::PreferencesRole || action->menuRole() == QAction::AboutRole) {
      item.menu = QStringLiteral("Mitcad");
    }
    m_nativeActions.insert(item.id, action);
    actions.append(item);
    assigned.insert(action);
  };
  const auto addMenu = [&append](const auto& self, QMenu* menu, const QString& path) -> void {
    for (QAction* action : menu->actions()) {
      if (action->menu()) {
        self(self, action->menu(), path + QLatin1Char('/') + menuLabel(action->text()));
      } else {
        append(action, path);
      }
    }
  };
  for (QAction* top : menuBar()->actions()) {
    if (top->menu() && menuLabel(top->text()) != QStringLiteral("Window")) {
      addMenu(addMenu, top->menu(), menuLabel(top->text()));
    }
  }
  for (const auto& def : m_registry->commands()) {
    QAction* action = m_registry->action(def->id);
    if (action && !assigned.contains(action)) {
      append(action, QString());
    }
  }
  // SELECT uses shared actions added to the ribbon rather than registry
  // commands. Keep its pinned filter icon and its actual checked submenu
  // available in both native workspaces.
  if (auto* select = m_ribbon->findChild<QToolButton*>(QStringLiteral("ribbonMenu_SOLID/SELECT"))) {
    if (QMenu* menu = select->menu()) {
      for (QAction* action : menu->actions()) {
        if (!action->isVisible()) {
          continue;
        }
        append(action, QStringLiteral("Select"));
        auto& item = actions.last();
        item.group = QStringLiteral("SELECT");
        item.toolbarOrder = order++;
        if (QMenu* submenu = action->menu()) {
          item.toolbarPinned = true;
          item.submenu = QStringLiteral("Select/") + menuLabel(action->text());
          const QString path = item.submenu;
          addMenu(addMenu, submenu, path);
        }
      }
    }
  }
  m_nativeWindow->setActions(actions);
}

void MainWindow::refreshNativeWindow() {
  if (!m_nativeWindow || modelBusy()) {
    return;
  }
  refreshNativeActions();
  m_nativeWindow->setDocument(documentName(), m_filePath, isModified());
  m_nativeWindow->setSketchMode(m_sketch->isActive());
  m_nativeWindow->setHistoryFilter(m_controller->timeline()->filterLabel(),
                                 !m_controller->timeline()->filter().isEmpty());
  const bool canRename = canChangeModel();
  const bool canToggleVisibility = canChangeModel(true);
  if (m_nativeSnapshotKey != m_snapshot.key || m_nativeNodes.isEmpty() || m_nativeCanRename != canRename ||
      m_nativeCanToggleVisibility != canToggleVisibility) {
    QList<mac::NativeMacTreeItem> rows;
    m_nativeNodes.clear();
    for (const auto& row : m_controller->browser()->presentationRows()) {
      mac::NativeMacTreeItem item;
      item.id = row.key;
      item.parentId = row.parent;
      item.title = row.node.name;
      item.detail = row.tooltip;
      item.symbol = symbolForNode(row.node);
      item.kind = row.node.isFolder() ? QStringLiteral("folder") : QString::number(static_cast<int>(row.node.type));
      item.editable = canRename && (row.node.type == BrowserNode::Type::Body || row.node.type == BrowserNode::Type::Sketch ||
                                   row.node.type == BrowserNode::Type::Component || row.node.type == BrowserNode::Type::Datum ||
                                   row.node.type == BrowserNode::Type::Analysis || row.node.type == BrowserNode::Type::Joint);
      item.hasVisibility = row.node.hasEye;
      item.canToggleVisibility = row.node.hasEye && canToggleVisibility;
      item.visible = row.node.visible;
      item.checked = row.node.visible;
      rows.append(item);
      m_nativeNodes.insert(row.key, row.node);
    }
    m_nativeWindow->setModelItems(rows);
    QList<mac::NativeMacHistoryItem> history;
    const QSet<QString> filtered = m_controller->timeline()->filteredFeatures();
    for (const auto& feature : m_snapshot.features) {
      mac::NativeMacHistoryItem item;
      item.id = feature.uid;
      item.title = feature.name;
      QStringList detail = {feature.name, feature.type};
      if (!feature.error.isEmpty()) detail.append(feature.error);
      if (feature.suppressed) detail.append(tr("Suppressed"));
      else if (feature.index >= m_snapshot.marker || feature.status == QStringLiteral("rolled_back")) {
        detail.append(tr("Rolled back"));
      } else if (feature.status == QStringLiteral("pending")) {
        detail.append(tr("Pending"));
      }
      item.detail = detail.join(QLatin1Char('\n'));
      item.symbol = feature.isSketch() ? QStringLiteral("pencil.and.outline") : QStringLiteral("cube");
      item.icon = themeIcon(featureIcon(feature.type));
      item.status = feature.status;
      item.suppressed = feature.suppressed;
      item.filtered = filtered.contains(feature.uid);
      history.append(item);
    }
    m_nativeWindow->setHistory(history, m_snapshot.marker);
    m_nativeSnapshotKey = m_snapshot.key;
    m_nativeCanRename = canRename;
    m_nativeCanToggleVisibility = canToggleVisibility;
  }
  QStringList selected;
  for (auto node = m_nativeNodes.cbegin(); node != m_nativeNodes.cend(); ++node) {
    if (m_selection.contains(node.value().item())) {
      selected.append(node.key());
    }
  }
  m_nativeWindow->setSelection(selected);
  m_nativeWindow->setHistorySelection(m_controller->timeline()->selectedFeatures());
  refreshNativeInspector();
}

void MainWindow::refreshNativeInspector() {
  if (m_nativeForm || !m_nativeWindow) {
    return;
  }
  QWidget* page = m_mode == Mode::Sketch ? static_cast<QWidget*>(m_palette) : static_cast<QWidget*>(m_idleInfo);
  m_nativeWindow->showInspector(m_mode == Mode::Sketch || !m_selection.isEmpty());
  if (m_nativeIdlePage != page || !m_nativeIdleForm) {
    m_nativeIdleForm = createNativeWidgetView(page);
    m_nativeIdlePage = page;
    m_nativeWindow->setInspectorView(m_nativeIdleForm ? m_nativeIdleForm->nativeView() : nullptr);
  } else {
    m_nativeIdleForm->refresh();
  }
  m_nativeWindow->setInspectorView(m_nativeIdleForm ? m_nativeIdleForm->nativeView() : nullptr);
}

bool MainWindow::requestNativeClose() {
  if (m_session) {
    if (!m_nativeClosingCommand) {
      m_nativeClosingCommand = true;
      connect(m_session, &CommandSession::finished, this, [this] {
        m_nativeClosingCommand = false;
        QTimer::singleShot(0, this, [this] { m_nativeWindow->close(); });
      });
      m_session->cancel();
    }
    return false;
  }
  QCloseEvent event;
  closeEvent(&event);
  return event.isAccepted();
}

bool MainWindow::modelCommandAsync(const QJsonObject& request, QObject* context, AsyncCompletion completion) {
  if (!m_nativeUi) {
    return false;
  }
  const QString name = request.value(QStringLiteral("cmd")).toString();
  // Native jobs bypass command(), so apply the same edit-lock policy
  // before giving the worker access to the document. A refusal completes
  // asynchronously rather than looking like a busy job to its caller.
  if (windowReadOnly() && !m_writeAnyway && !readOnlyModelCommand(name)) {
    const QString error = readOnlyReason();
    m_lastError = error;
    m_lastCancelled = false;
    qInfo().noquote() << QStringLiteral("Read-only: refused %1").arg(name);
    QTimer::singleShot(0, context ? context : this, [error, completion = std::move(completion)] {
      completion({{}, error, false});
    });
    return true;
  }
  if (m_reports) {
    m_reports->noteAction(QStringLiteral("model ") + name);
  }
  QElapsedTimer clock;
  clock.start();
  return m_documentSession->commandAsync(request, context, [this, clock, name, completion = std::move(completion)](
                                                               const DocumentSession::AsyncResult& result) {
    if (result.succeeded()) {
      reportRecompute(result.answer, static_cast<double>(clock.nsecsElapsed()) / 1.0e6);
      m_locks->activity();
    } else if (result.cancelled) {
      showHint(tr("Computation cancelled."));
      qInfo().noquote() << QStringLiteral("Command %1 cancelled").arg(name);
    }
    completion({result.answer, result.error, result.cancelled});
  });
}

bool MainWindow::modelPreviewAsync(const QByteArray& request, const QString& name, QObject* context,
                                  AsyncCompletion completion) {
  if (!m_nativeUi) {
    return false;
  }
  return m_documentSession->previewAsync(request, name, context, [completion = std::move(completion)](
                                                                       const DocumentSession::AsyncResult& result) {
    completion({result.answer, result.error, result.cancelled});
  });
}

bool MainWindow::modelInspectionAsync(const QString& name, QObject* context, InspectionTask task,
                                     InspectionCompletion completion) {
  if (!m_nativeUi) {
    return false;
  }
  return m_documentSession->inspectionAsync(name, context, std::move(task),
      [completion = std::move(completion)](const Inspection& inspection,
                                          const DocumentSession::AsyncResult& result) {
        completion(inspection, {result.answer, result.error, result.cancelled});
      });
}

} // namespace mitcad
