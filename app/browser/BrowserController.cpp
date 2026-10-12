// SPDX-License-Identifier: MIT
#include "BrowserController.hpp"

#include <algorithm>
#include <functional>
#include <tuple>
#include <utility>

#include <QInputDialog>
#include <QJsonArray>
#include <QLineEdit>
#include <QMenu>
#include <QMessageBox>
#include <QPushButton>
#include <QToolButton>
#include <QWidget>
#include <QtLogging>

#include "../framework/Dialogs.hpp"
#include "../framework/GlassCard.hpp"
#include "../framework/Icons.hpp"
#include "../framework/TestSync.hpp"
#include "../framework/Theme.hpp"
#include "DocumentHost.hpp"
#include "AppearanceDialog.hpp"
#include "ParametersDialog.hpp"
#include "TimelineWidget.hpp"

namespace mitcad {
namespace {

QJsonObject cmd(const char* name, QJsonObject fields = {}) {
  fields.insert(QStringLiteral("cmd"), QString::fromLatin1(name));
  return fields;
}

// A context menu whose entries are logged ("Context menu: A | B") for the
// UI tests, which choose entries by their place.
class Menu {
public:
  explicit Menu(QWidget* parent) : m_menu(parent) {}

  void add(const QString& icon, const QString& text, std::function<void()> call) {
    QAction* action = m_menu.addAction(themeIcon(icon), text);
    QObject::connect(action, &QAction::triggered, &m_menu, std::move(call));
    m_entries << text;
  }
  void separator() { m_menu.addSeparator(); }
  bool isEmpty() const { return m_entries.isEmpty(); }
  void exec(const QPoint& at) {
    qDebug().noquote() << QStringLiteral("Context menu: %1").arg(m_entries.join(QStringLiteral(" | ")));
    execMenu(&m_menu, at);
  }

private:
  QMenu m_menu;
  QStringList m_entries;
};

const QStringList kLengthUnits = {QStringLiteral("mm"), QStringLiteral("cm"), QStringLiteral("m"),
                                  QStringLiteral("in"), QStringLiteral("ft")};

} // namespace

template <typename Sender, typename... Args>
void BrowserController::follow(Sender* sender, void (Sender::*signal)(Args...),
                               void (BrowserController::*slot)(Args...), bool dropWhenBusy) {
  connect(
      sender, signal, this,
      [this, slot, dropWhenBusy](Args... args) {
        if (!m_host.modelBusy()) {
          (this->*slot)(args...);
        } else if (!dropWhenBusy) {
          m_host.whenIdle(this, [this, slot, args...] { (this->*slot)(args...); });
        }
      },
      Qt::QueuedConnection);
}

BrowserController::BrowserController(DocumentHost& host, QWidget* window)
    : QObject(window), m_host(host), m_window(window) {
  m_browser = new BrowserPanel(window);
  m_timeline = new TimelineWidget(host.model(), window);
  m_failures = new QToolButton(window);
  m_failures->setAutoRaise(true);
  m_failures->setFocusPolicy(Qt::NoFocus);
  m_failures->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
  m_failures->setIcon(themeIcon(QStringLiteral("warning")));
  setErrorStyleSheet(m_failures, QStringLiteral("QToolButton { color: %1; }"));
  m_failures->hide();
  // Features with warnings, the import's among them (mitcad#170): yellow.
  m_warnings = new QToolButton(window);
  m_warnings->setObjectName(QStringLiteral("warningsBadge"));
  m_warnings->setAutoRaise(true);
  m_warnings->setFocusPolicy(Qt::NoFocus);
  m_warnings->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
  m_warnings->setIcon(themeIcon(QStringLiteral("warning")));
  setWarningStyleSheet(m_warnings, QStringLiteral("QToolButton { color: %1; }"));
  m_warnings->hide();

  follow(m_browser, &BrowserPanel::itemsPicked, &BrowserController::pickItems);
  follow(m_browser, &BrowserPanel::rowsPicked, &BrowserController::pickRows);
  follow(m_browser, &BrowserPanel::renameClicked, &BrowserController::renameClicked, true);
  follow(m_browser, &BrowserPanel::eyeClicked, &BrowserController::toggleVisibility);
  follow(m_browser, &BrowserPanel::activateClicked, &BrowserController::activate);
  follow(m_browser, &BrowserPanel::renamed, &BrowserController::rename);
  follow(m_browser, &BrowserPanel::doubleClicked, &BrowserController::browserDoubleClicked);
  follow(m_browser, &BrowserPanel::deleteRequested, &BrowserController::deleteNode);
  follow(m_browser, &BrowserPanel::contextMenuRequested, &BrowserController::browserMenu, true);

  follow(m_timeline, &TimelineWidget::featuresClicked, &BrowserController::featuresClicked);
  follow(m_timeline, &TimelineWidget::editRequested, &BrowserController::editFeature);
  follow(m_timeline, &TimelineWidget::contextMenuRequested, &BrowserController::timelineMenu, true);
  follow(m_timeline, &TimelineWidget::groupMenuRequested, &BrowserController::groupMenu, true);
  follow(m_timeline, &TimelineWidget::deleteRequested, &BrowserController::deleteFeature);
  follow(m_timeline, &TimelineWidget::renamed, &BrowserController::renameFeature);
  follow(m_timeline, &TimelineWidget::reorderRequested, &BrowserController::reorder);
  follow(m_timeline, &TimelineWidget::markerRequested, &BrowserController::setMarker);
  // A press: never while a job computes the model, whose caller waits with
  // input queued or blocked.
  connect(m_timeline, &TimelineWidget::markerDragStarted, this, [this] {
    m_markerDepth = canChange() ? undoDepth() : -1;
    m_markerStart = m_snapshot.marker;
  });
  follow(m_timeline, &TimelineWidget::markerDragged, &BrowserController::markerDragged);
  follow(m_timeline, &TimelineWidget::markerDragFinished, &BrowserController::markerDragFinished);
  // The timeline's search (mitcad#179): Ctrl+F in the browser opens it; the
  // selection from before it comes back when it is cleared.
  connect(m_browser, &BrowserPanel::searchRequested, m_timeline, &TimelineWidget::openSearch);
  connect(m_timeline, &TimelineWidget::searchStarted, this, [this] {
    m_searchItems = m_shownItems;
    m_searchFeatures = m_shownFeatures;
  });
  follow(m_timeline, &TimelineWidget::searchRestored, &BrowserController::restoreSearchSelection);

  // A click on a badge goes to its feature, or lists them to go to.
  const auto listed = [this](QToolButton* button, const QVector<const DocumentSnapshot::Feature*>& features) {
    if (features.size() == 1) {
      reveal(features.first()->uid);
      return;
    }
    Menu menu(m_window);
    for (const DocumentSnapshot::Feature* feature : features) {
      const QString uid = feature->uid;
      menu.add(featureIcon(feature->type), QStringLiteral("%1: %2").arg(feature->name, feature->error),
               [this, uid] { reveal(uid); });
    }
    menu.exec(button->mapToGlobal(QPoint(0, 0)));
  };
  connect(m_failures, &QToolButton::clicked, this, [this, listed] { listed(m_failures, m_snapshot.failures()); });
  connect(m_warnings, &QToolButton::clicked, this, [this, listed] { listed(m_warnings, m_snapshot.warnings()); });
}

// ---------------------------------------------------------------------------
// Keeping in step

void BrowserController::refresh(const DocumentSnapshot& snapshot) {
  const bool origin = m_host.originShown();
  if (!m_shown || snapshot.key != m_snapshot.key || origin != m_originShown) {
    m_shown = true;
    m_snapshot = snapshot;
    m_originShown = origin;
    m_browser->rebuild(snapshot, origin);
    m_timeline->rebuild(snapshot);
    updateFailures();
  }
  if (m_parameters && m_parameters->isVisible()) {
    m_parameters->refresh();
  }
  if (m_appearances && m_appearances->isVisible()) {
    m_appearances->refresh();
  }
}

void BrowserController::showSelection(const Selection& items) {
  // A pick of features shows their rows under the sketches too (mitcad#177),
  // also when the same selection is shown again; another selection (in the
  // view) does not.
  if (m_picking) {
    m_shownFeatures = m_pickedFeatures;
  } else if (items != m_shownItems) {
    m_shownFeatures.clear();
  }
  m_shownItems = items;
  m_browser->showSelection(items, m_shownFeatures);
}

bool BrowserController::cancelEditing() {
  return m_browser->cancelEditing() || m_timeline->cancelEditing();
}

void BrowserController::updateFailures() {
  const QVector<const DocumentSnapshot::Feature*> failed = m_snapshot.failures();
  QStringList lines;
  for (const DocumentSnapshot::Feature* feature : failed) {
    lines << QStringLiteral("%1: %2").arg(feature->name, feature->error);
  }
  m_failures->setVisible(!failed.isEmpty());
  m_failures->setText(tr("%n failed", nullptr, static_cast<int>(failed.size())));
  m_failures->setToolTip(lines.join(QLatin1Char('\n')));
  const QString logged = lines.join(QStringLiteral("; "));
  if (logged != m_loggedFailures) {
    m_loggedFailures = logged;
    qDebug().noquote() << QStringLiteral("Failed features: %1")
                              .arg(logged.isEmpty() ? QStringLiteral("none") : logged);
    // Where to click, once the status bar has placed it.
    TestSync::singleShot(300, m_failures, [this] {
      if (m_failures->isVisible()) {
        const QPoint at = GlassCard::mapToHost(m_failures, m_failures->rect().center());
        qDebug().noquote() << QStringLiteral("Failures button at %1,%2").arg(at.x()).arg(at.y());
      }
    });
  }
  // Features that succeeded with warnings are yellow in the timeline (P9).
  QStringList warnings;
  for (const DocumentSnapshot::Feature& feature : std::as_const(m_snapshot.features)) {
    if (feature.status == QStringLiteral("warning")) {
      warnings << QStringLiteral("%1: %2").arg(feature.name, feature.error);
    }
  }
  const QString warned = warnings.join(QStringLiteral("; "));
  if (warned != m_loggedWarnings) {
    m_loggedWarnings = warned;
    qDebug().noquote() << QStringLiteral("Feature warnings: %1").arg(warned.isEmpty() ? QStringLiteral("none") : warned);
  }
  // The yellow badge beside the failures' (mitcad#170): those, and what the
  // import left a note on (a placeholder of a skipped item is suppressed).
  const QVector<const DocumentSnapshot::Feature*> noted = m_snapshot.warnings();
  QStringList badge;
  for (const DocumentSnapshot::Feature* feature : noted) {
    badge << QStringLiteral("%1: %2").arg(feature->name, feature->error);
  }
  m_warnings->setVisible(!noted.isEmpty());
  m_warnings->setText(tr("%n warning(s)", nullptr, static_cast<int>(noted.size())));
  m_warnings->setToolTip(badge.join(QLatin1Char('\n')));
  const QString badged = badge.join(QStringLiteral("; "));
  if (badged != m_loggedBadge) {
    m_loggedBadge = badged;
    qDebug().noquote() << QStringLiteral("Warnings badge: %1").arg(badged.isEmpty() ? QStringLiteral("none") : badged);
    TestSync::singleShot(300, m_warnings, [this] {
      if (m_warnings->isVisible()) {
        const QPoint at = GlassCard::mapToHost(m_warnings, m_warnings->rect().center());
        qDebug().noquote() << QStringLiteral("Warnings button at %1,%2").arg(at.x()).arg(at.y());
      }
    });
  }
  // The sketches' degrees of freedom the browser shows (P9).
  QStringList dofs;
  for (const DocumentSnapshot::Feature& feature : std::as_const(m_snapshot.features)) {
    if (feature.isSketch() && feature.dof >= 0) {
      dofs << QStringLiteral("%1 %2").arg(feature.name).arg(feature.dof);
    }
  }
  const QString dof = dofs.join(QStringLiteral(", "));
  if (dof != m_loggedDof) {
    m_loggedDof = dof;
    qDebug().noquote() << QStringLiteral("Browser sketch DOF: %1").arg(dof.isEmpty() ? QStringLiteral("none") : dof);
  }
  // The joints (mitcad#55): each one's state, and the degrees of freedom of
  // each component with joints.
  QStringList joints;
  for (const QJsonValue& value : m_snapshot.joints.value(QStringLiteral("joints")).toArray()) {
    const QJsonObject joint = value.toObject();
    joints << QStringLiteral("%1 %2").arg(joint.value(QStringLiteral("name")).toString(),
                                          joint.value(QStringLiteral("state")).toString());
  }
  for (const QJsonValue& value : m_snapshot.joints.value(QStringLiteral("dof")).toArray()) {
    const QJsonObject component = value.toObject();
    joints << QStringLiteral("DOF %1 %2")
                  .arg(m_snapshot.componentName(component.value(QStringLiteral("component")).toString()))
                  .arg(component.value(QStringLiteral("total")).toInt());
  }
  const QString jointLog = joints.join(QStringLiteral(", "));
  if (jointLog != m_loggedJoints) {
    m_loggedJoints = jointLog;
    qDebug().noquote() << QStringLiteral("Browser joints: %1").arg(jointLog.isEmpty() ? QStringLiteral("none") : jointLog);
  }
}

void BrowserController::reveal(const QString& uid) {
  const DocumentSnapshot::Feature* feature = m_snapshot.feature(uid);
  if (feature == nullptr) {
    return;
  }
  m_timeline->selectFeature(uid);
  m_browser->reveal(uid);
  if (!feature->error.isEmpty()) {
    m_host.showStatus(QStringLiteral("%1: %2").arg(feature->name, feature->error), true);
  }
  qDebug().noquote() << QStringLiteral("Revealed %1").arg(feature->name);
}

bool BrowserController::canChange(bool visibilityOnly) {
  if (m_host.canChangeModel(visibilityOnly)) {
    return true;
  }
  const QString readOnly = m_host.readOnlyReason();
  m_host.showStatus(readOnly.isEmpty() ? tr("Finish the command or the sketch first.") : readOnly, true);
  return false;
}

int BrowserController::undoDepth() const {
  try {
    return m_host.model()
        .queryObject({{QStringLiteral("query"), QStringLiteral("document")}})
        .value(QStringLiteral("undo_depth"))
        .toInt();
  } catch (const std::exception&) {
    return 0;
  }
}

const DocumentSnapshot::Occurrence* BrowserController::occurrence(const QString& path) const {
  std::function<const DocumentSnapshot::Occurrence*(const QVector<DocumentSnapshot::Occurrence>&)> find =
      [&](const QVector<DocumentSnapshot::Occurrence>& occurrences) -> const DocumentSnapshot::Occurrence* {
    for (const DocumentSnapshot::Occurrence& o : occurrences) {
      if (o.path == path) {
        return &o;
      }
      if (const DocumentSnapshot::Occurrence* inside = find(o.children)) {
        return inside;
      }
    }
    return nullptr;
  };
  return find(m_snapshot.occurrences);
}

// ---------------------------------------------------------------------------
// The browser

void BrowserController::toggleVisibility(const BrowserNode& node) {
  const bool show = !node.visible;
  const QString verb = show ? QStringLiteral("shown") : QStringLiteral("hidden");
  if (node.type == BrowserNode::Type::Origin) {
    m_host.setOriginShown(show);
    qDebug().noquote() << QStringLiteral("Visibility %1: %2").arg(node.path, verb);
    return;
  }
  if (!canChange(true)) {
    return;
  }
  bool done = false;
  switch (node.type) {
  case BrowserNode::Type::Body:
    done = m_host.runModelCommand(cmd("set_body_visible", {{QStringLiteral("uid"), node.uid},
                                                           {QStringLiteral("visible"), show}}));
    break;
  case BrowserNode::Type::Sketch:
  case BrowserNode::Type::Datum:
    done = m_host.runModelCommand(cmd("set_feature_visible", {{QStringLiteral("uid"), node.uid},
                                                              {QStringLiteral("visible"), show}}));
    break;
  case BrowserNode::Type::Component:
    done = m_host.runModelCommand(cmd("set_occurrence_visible",
                                      {{QStringLiteral("occurrence"), node.occurrence},
                                       {QStringLiteral("visible"), show}}));
    break;
  case BrowserNode::Type::Bodies: {
    QJsonArray commands;
    const DocumentSnapshot::Component* component = m_snapshot.component(node.component);
    for (const QString& uid : component != nullptr ? component->bodies : QStringList()) {
      commands.append(cmd("set_body_visible", {{QStringLiteral("uid"), uid}, {QStringLiteral("visible"), show}}));
    }
    done = m_host.runModelCommands(commands, show ? tr("Show Bodies") : tr("Hide Bodies"));
    break;
  }
  case BrowserNode::Type::Sketches:
  case BrowserNode::Type::Construction: {
    const bool sketches = node.type == BrowserNode::Type::Sketches;
    QJsonArray commands;
    for (const DocumentSnapshot::Feature& feature : m_snapshot.features) {
      if (feature.component == node.component && feature.isActive() &&
          (sketches ? feature.isSketch() : feature.isConstruction())) {
        commands.append(cmd("set_feature_visible", {{QStringLiteral("uid"), feature.uid},
                                                    {QStringLiteral("visible"), show}}));
      }
    }
    const QString what = sketches ? tr("Sketches") : tr("Construction");
    done = m_host.runModelCommands(commands, (show ? tr("Show %1") : tr("Hide %1")).arg(what));
    break;
  }
  case BrowserNode::Type::FlatPattern:
    // The sheet shown flat in place of the folded body (mitcad#210).
    done = m_host.runModelCommand(cmd("set_flat_pattern", {{QStringLiteral("body"), node.uid},
                                                           {QStringLiteral("shown"), show}}));
    break;
  case BrowserNode::Type::Analysis:
    // Showing it hides the others: one section at a time (mitcad#41).
    done = m_host.runModelCommand(cmd("set_analysis_visible", {{QStringLiteral("name"), node.uid},
                                                               {QStringLiteral("visible"), show}}));
    break;
  default:
    return;
  }
  if (done) {
    qDebug().noquote() << QStringLiteral("Visibility %1: %2").arg(node.path, verb);
  }
}

void BrowserController::activate(const BrowserNode& node) {
  if (node.type != BrowserNode::Type::Component || !canChange()) {
    return;
  }
  if (m_snapshot.activeComponent == node.uid) {
    return;
  }
  if (m_host.runModelCommand(cmd("activate_component", {{QStringLiteral("component"), node.uid}}))) {
    qDebug().noquote() << QStringLiteral("Activated %1").arg(m_snapshot.componentName(node.uid));
  }
}

void BrowserController::rename(const BrowserNode& node, const QString& name) {
  if (!canChange()) {
    m_browser->rebuild(m_snapshot, m_originShown); // puts the old name back
    return;
  }
  QJsonObject command;
  switch (node.type) {
  case BrowserNode::Type::Component:
    command = cmd("rename_component", {{QStringLiteral("component"), node.uid}, {QStringLiteral("name"), name}});
    break;
  case BrowserNode::Type::Body:
    command = cmd("rename_body", {{QStringLiteral("uid"), node.uid}, {QStringLiteral("name"), name}});
    break;
  case BrowserNode::Type::Sketch:
  case BrowserNode::Type::Datum:
  case BrowserNode::Type::Joint:
  case BrowserNode::Type::Feature:
    command = cmd("rename_feature", {{QStringLiteral("uid"), node.uid}, {QStringLiteral("name"), name}});
    break;
  case BrowserNode::Type::NamedView:
    command = cmd("rename_named_view",
                  {{QStringLiteral("name"), node.uid.mid(6)}, {QStringLiteral("new_name"), name}});
    break;
  case BrowserNode::Type::Analysis:
    command = cmd("rename_analysis", {{QStringLiteral("name"), node.uid}, {QStringLiteral("new_name"), name}});
    break;
  default:
    return;
  }
  if (m_host.runModelCommand(command)) {
    qDebug().noquote() << QStringLiteral("Renamed %1 to %2").arg(node.name, name);
  } else {
    m_browser->rebuild(m_snapshot, m_originShown);
  }
}

void BrowserController::renameFeature(const QString& uid, const QString& name) {
  const DocumentSnapshot::Feature* feature = m_snapshot.feature(uid);
  if (feature == nullptr || feature->name == name || !canChange()) {
    return;
  }
  const QString old = feature->name;
  if (m_host.runModelCommand(cmd("rename_feature", {{QStringLiteral("uid"), uid}, {QStringLiteral("name"), name}}))) {
    qDebug().noquote() << QStringLiteral("Renamed %1 to %2").arg(old, name);
  }
}

void BrowserController::browserDoubleClicked(const BrowserNode& node) {
  switch (node.type) {
  case BrowserNode::Type::Sketch:
  case BrowserNode::Type::Datum:
  case BrowserNode::Type::Joint:
  case BrowserNode::Type::Feature:
  case BrowserNode::Type::SheetRules:
    m_host.editFeature(node.uid);
    break;
  case BrowserNode::Type::Body:
  case BrowserNode::Type::Component:
    if (canChange()) {
      m_browser->startRename(node);
    }
    break;
  case BrowserNode::Type::FlatPattern:
    editFlatPattern(node.uid);
    break;
  case BrowserNode::Type::Units:
    changeUnits();
    break;
  case BrowserNode::Type::NamedView:
    m_host.showNamedView(node.uid);
    break;
  case BrowserNode::Type::Analysis:
    m_host.editAnalysis(node.uid);
    break;
  default:
    break;
  }
}

void BrowserController::editFlatPattern(const QString& body) {
  if (canChange()) {
    m_host.startCommand(QStringLiteral("sheet.flat_pattern"), {SelectionItem{SelectKind::Body, body, QString(), QString()}},
                        nullptr);
  }
}

void BrowserController::deleteNode(const BrowserNode& node) {
  switch (node.type) {
  case BrowserNode::Type::FlatPattern:
    if (canChange() && m_host.runModelCommand(cmd("remove_flat_pattern", {{QStringLiteral("body"), node.uid}}))) {
      qDebug().noquote() << QStringLiteral("Deleted the flat pattern of %1").arg(m_host.model().bodyName(node.uid));
    }
    break;
  case BrowserNode::Type::Body:
    // The feature that made it goes, as nothing removes a body by itself.
    deleteFeature(node.uid.section(QLatin1Char('.'), 0, 0));
    break;
  case BrowserNode::Type::Sketch:
  case BrowserNode::Type::Datum:
  case BrowserNode::Type::Joint:
  case BrowserNode::Type::Feature:
    deleteFeature(node.uid);
    break;
  case BrowserNode::Type::Component:
    if (!node.occurrence.isEmpty() && canChange()) {
      QJsonObject result;
      if (m_host.runModelCommand(cmd("delete_occurrence", {{QStringLiteral("occurrence"), node.occurrence}}),
                                 &result)) {
        qDebug().noquote() << QStringLiteral("Deleted %1").arg(node.name);
      }
    }
    break;
  case BrowserNode::Type::NamedView:
    if (node.uid.startsWith(QStringLiteral("named:")) && canChange() &&
        m_host.runModelCommand(cmd("delete_named_view", {{QStringLiteral("name"), node.uid.mid(6)}}))) {
      qDebug().noquote() << QStringLiteral("Deleted named view %1").arg(node.name);
    }
    break;
  case BrowserNode::Type::Analysis:
    if (canChange() && m_host.runModelCommand(cmd("delete_analysis", {{QStringLiteral("name"), node.uid}}))) {
      qDebug().noquote() << QStringLiteral("Deleted analysis %1").arg(node.name);
    }
    break;
  default:
    break;
  }
}

void BrowserController::newComponent(const BrowserNode& node) {
  if (!canChange()) {
    return;
  }
  // Made in the root with modelled bodies: a hybrid assembly (mitcad#171).
  ComponentRequest request;
  request.kind = ComponentRequest::Kind::NewComponent;
  request.parent = node.uid;
  if (askHybridAssembly(m_window, hybridModel(), request).outcome == ComponentAnswer::Outcome::Cancel) {
    return;
  }
  QJsonArray commands;
  if (node.uid != m_snapshot.activeComponent) {
    // It is made in the component the menu was opened on.
    commands.append(cmd("activate_component", {{QStringLiteral("component"), node.uid}}));
  }
  commands.append(cmd("create_component"));
  if (m_host.runModelCommands(commands, tr("New Component"))) {
    qDebug().noquote() << QStringLiteral("New component in %1").arg(node.name);
  }
}

HybridModel BrowserController::hybridModel() {
  HybridModel model;
  model.query = [this](const QJsonObject& query) { return m_host.model().queryObject(query); };
  model.run = [this](const QJsonObject& command, QJsonObject* result) {
    return m_host.runModelCommand(command, result);
  };
  return model;
}

// Create Components from Bodies on a body: asked first when the body is
// one of the root's (mitcad#171); moved, it takes the features that made
// it along.
void BrowserController::componentsFromBodies(const BrowserNode& node) {
  if (!canChange()) {
    return;
  }
  ComponentRequest request;
  request.kind = ComponentRequest::Kind::FromBodies;
  request.bodies = QJsonArray{node.uid};
  const ComponentAnswer answer = askHybridAssembly(m_window, hybridModel(), request);
  if (answer.outcome != ComponentAnswer::Outcome::Proceed) {
    return;
  }
  QJsonObject result;
  if (m_host.runModelCommand(cmd("components_from_bodies", {{QStringLiteral("bodies"), QJsonArray{node.uid}}}),
                             &result)) {
    qDebug().noquote() << QStringLiteral("Component from %1").arg(node.name);
  }
}

void BrowserController::moveToComponent(const QJsonObject& selection) {
  if (canChange()) {
    mitcad::moveToComponent(m_window, hybridModel(), selection);
  }
}

// The features that made a component-from-bodies' bodies go into its
// component, and the step goes (mitcad#171).
void BrowserController::moveHistory(const QString& uid) {
  const DocumentSnapshot::Feature* feature = m_snapshot.feature(uid);
  if (feature == nullptr || !canChange()) {
    return;
  }
  const QString name = feature->name;
  QJsonObject result;
  if (m_host.runModelCommand(cmd("move_history_into_component", {{QStringLiteral("feature"), uid}}), &result)) {
    QStringList names;
    for (const QJsonValue& f : result.value(QStringLiteral("features")).toArray()) {
      const DocumentSnapshot::Feature* moved = m_snapshot.feature(f.toString());
      names << (moved != nullptr ? moved->name : f.toString());
    }
    qDebug().noquote() << QStringLiteral("Moved %1 into %2 instead of %3")
                              .arg(names.join(QStringLiteral(", ")),
                                   result.value(QStringLiteral("name")).toString(), name);
  }
}

// Paste and Paste New: the copy is placed where the original is
// and Move/Copy opens on it (P9). OK leaves it where it was moved to, as
// its own placement (no Move in the timeline): paste and placement are one
// undo step. Cancel takes the paste back.
void BrowserController::paste(bool asNew) {
  if (m_clipboard.isEmpty() || !canChange()) {
    return;
  }
  const auto undoDepth = [this] {
    return m_host.model()
        .queryObject({{QStringLiteral("query"), QStringLiteral("document")}})
        .value(QStringLiteral("undo_depth"))
        .toInt();
  };
  const int depth = undoDepth();
  QJsonObject result;
  if (!m_host.runModelCommand(cmd(asNew ? "paste_new" : "copy_occurrence",
                                  {{QStringLiteral("occurrence"), m_clipboard}}),
                              &result)) {
    return;
  }
  const QString uid = result.value(QStringLiteral("occurrence")).toString();
  const QString name = result.value(QStringLiteral("name")).toString();
  const QString label = asNew ? tr("Paste New %1").arg(name) : tr("Paste %1").arg(name);
  qDebug().noquote() << QStringLiteral("Pasted %1%2 as %3")
                            .arg(m_clipboardName, asNew ? QStringLiteral(" (new component)") : QString(), name);
  // The occurrence in the browser's form: its path and its component.
  std::function<const DocumentSnapshot::Occurrence*(const QVector<DocumentSnapshot::Occurrence>&)> find =
      [&](const QVector<DocumentSnapshot::Occurrence>& level) -> const DocumentSnapshot::Occurrence* {
    for (const DocumentSnapshot::Occurrence& placed : level) {
      if (placed.uid == uid) {
        return &placed;
      }
      if (const DocumentSnapshot::Occurrence* inside = find(placed.children)) {
        return inside;
      }
    }
    return nullptr;
  };
  const DocumentSnapshot::Occurrence* pasted = find(m_snapshot.occurrences);
  if (pasted == nullptr) {
    return;
  }
  SelectionItem item;
  item.kind = SelectKind::Component;
  item.occurrence = pasted->path;
  item.owner = pasted->component;
  m_host.startCommand(QStringLiteral("solid.move"), {item}, [this, depth, uid, name, label, undoDepth](bool committed) {
    if (!committed) {
      while (undoDepth() > depth && m_host.runModelCommand(cmd("undo"))) {
      }
      qDebug().noquote() << QStringLiteral("Paste of %1 cancelled").arg(name);
      return;
    }
    // Where Move/Copy put it, then the move feature taken back and the
    // placement set instead.
    QJsonArray transform;
    std::function<void(const QJsonArray&)> look = [&](const QJsonArray& level) {
      for (const QJsonValue& value : level) {
        const QJsonObject placed = value.toObject();
        if (placed.value(QStringLiteral("uid")).toString() == uid) {
          transform = placed.value(QStringLiteral("transform")).toArray();
        }
        look(placed.value(QStringLiteral("children")).toArray());
      }
    };
    look(m_host.model()
             .queryObject({{QStringLiteral("query"), QStringLiteral("components")}})
             .value(QStringLiteral("occurrences"))
             .toArray());
    if (undoDepth() > depth + 1 && !transform.isEmpty() && m_host.runModelCommand(cmd("undo"))) {
      m_host.runModelCommand(cmd("set_occurrence_transform", {{QStringLiteral("occurrence"), uid},
                                                              {QStringLiteral("transform"), transform}}));
    }
    m_host.runModelCommand(cmd("merge_undo", {{QStringLiteral("depth"), depth}, {QStringLiteral("label"), label}}));
    QStringList at;
    for (const QJsonValue& row : transform) {
      at << QString::number(row.toArray().at(3).toDouble(), 'g', 6);
    }
    qDebug().noquote() << QStringLiteral("Placed %1 at (%2)").arg(name, at.join(QStringLiteral(", ")));
  });
}

void BrowserController::changeUnits() {
  if (!canChange()) {
    return;
  }
  bool ok = false;
  qDebug().noquote() << QStringLiteral("Units dialog opened: %1 (%2)")
                            .arg(m_snapshot.lengthUnit, kLengthUnits.join(QStringLiteral(" | ")));
  const QString unit = dialogItem(m_window, tr("Units"), tr("Document length unit:"), kLengthUnits,
                                             std::max(0, static_cast<int>(kLengthUnits.indexOf(m_snapshot.lengthUnit))),
                                             &ok);
  if (ok && unit != m_snapshot.lengthUnit &&
      m_host.runModelCommand(cmd("set_units", {{QStringLiteral("length"), unit}}))) {
    qDebug().noquote() << QStringLiteral("Units: %1").arg(unit);
  }
}

void BrowserController::browserMenu(const BrowserNode& node, const QPoint& at) {
  Menu menu(m_window);
  const SelectionItem item = node.item();
  const bool isolated = !m_host.isolation().isEmpty();
  const auto visibility = [&] {
    if (node.hasEye) {
      menu.add(node.visible ? QStringLiteral("eye-off") : QStringLiteral("eye"),
               node.visible ? tr("Hide") : tr("Show"), [this, node] { toggleVisibility(node); });
    }
  };
  const auto isolate = [&] {
    if (item.isValid()) {
      menu.add(QStringLiteral("isolate"), tr("Isolate"), [this, item] {
        m_host.setIsolation({item});
        qDebug().noquote() << QStringLiteral("Isolated %1").arg(item.describe());
      });
    }
    if (isolated) {
      menu.add(QString(), tr("Unisolate"), [this] {
        m_host.setIsolation({});
        qDebug().noquote() << "Unisolated";
      });
    }
  };
  const auto renameEntry = [&] {
    menu.add(QStringLiteral("edit"), tr("Rename"), [this, node] {
      if (canChange()) {
        m_browser->startRename(node);
      }
    });
  };
  const auto timelineEntry = [&](const QString& feature) {
    menu.add(QStringLiteral("timeline"), tr("Find in Timeline"), [this, feature] { findInTimeline(feature); });
  };
  const bool plane = item.kind == SelectKind::Plane;

  switch (node.type) {
  case BrowserNode::Type::Component: {
    const bool root = node.occurrence.isEmpty();
    if (m_snapshot.activeComponent != node.uid) {
      menu.add(QStringLiteral("radio-on"), tr("Activate Component"), [this, node] { activate(node); });
    }
    menu.add(QStringLiteral("component"), tr("New Component"), [this, node] { newComponent(node); });
    // Its modelled bodies with their features into a component of their
    // own (mitcad#171).
    for (const DocumentSnapshot::Component& c : std::as_const(m_snapshot.components)) {
      if (c.uid == node.uid && !c.linked && !c.bodies.isEmpty()) {
        menu.add(QStringLiteral("component"), tr("Move Features into a Component"), [this, node] {
          moveToComponent({{QStringLiteral("component"), node.uid},
                           {QStringLiteral("source_occurrence"), node.occurrence}});
        });
      }
    }
    if (!root) {
      menu.add(QStringLiteral("copy"), tr("Copy"), [this, node] {
        m_clipboard = node.occurrence;
        m_clipboardName = node.name;
        qDebug().noquote() << QStringLiteral("Copied %1").arg(node.name);
      });
    }
    if (!m_clipboard.isEmpty()) {
      menu.add(QStringLiteral("paste"), tr("Paste"), [this] { paste(false); });
      menu.add(QStringLiteral("paste"), tr("Paste New"), [this] { paste(true); });
    }
    menu.separator();
    if (!root) {
      const DocumentSnapshot::Occurrence* placed = occurrence(node.occurrence);
      const bool grounded = placed != nullptr && placed->grounded;
      menu.add(QStringLiteral("ground"), grounded ? tr("Unground") : tr("Ground"), [this, node, grounded] {
        if (canChange() &&
            m_host.runModelCommand(cmd("ground_occurrence", {{QStringLiteral("occurrence"), node.occurrence},
                                                             {QStringLiteral("grounded"), !grounded}}))) {
          qDebug().noquote() << QStringLiteral("%1 %2").arg(grounded ? QStringLiteral("Unground")
                                                                     : QStringLiteral("Ground"),
                                                            node.name);
        }
      });
      visibility();
    }
    isolate();
    // The timeline's filter (mitcad#98), this window's view.
    menu.add(QStringLiteral("filter"), tr("Show in Timeline Only"), [this, node] {
      m_timeline->setFilter(node.uid, m_timeline->filterSubcomponents());
    });
    if (!m_timeline->filter().isEmpty()) {
      menu.add(QString(), tr("Show All in Timeline"),
               [this] { m_timeline->setFilter(QString(), m_timeline->filterSubcomponents()); });
    }
    menu.separator();
    renameEntry();
    if (!root) {
      menu.add(QStringLiteral("delete"), tr("Delete"), [this, node] { deleteNode(node); });
    }
    break;
  }
  case BrowserNode::Type::Body: {
    const QString feature = node.uid.section(QLatin1Char('.'), 0, 0);
    const DocumentSnapshot::Feature* made = m_snapshot.feature(feature);
    menu.add(QStringLiteral("edit"), tr("Edit %1").arg(made != nullptr ? made->name : feature),
             [this, feature] { m_host.editFeature(feature); });
    menu.separator();
    visibility();
    isolate();
    menu.add(QStringLiteral("component"), tr("Create Components from Bodies"),
             [this, node] { componentsFromBodies(node); });
    // The body with the features that made it (mitcad#171).
    menu.add(QStringLiteral("component"), tr("Move to Component..."), [this, node] {
      moveToComponent({{QStringLiteral("component"), node.component},
                       {QStringLiteral("source_occurrence"), node.occurrence},
                       {QStringLiteral("bodies"), QJsonArray{node.uid}}});
    });
    menu.add(QStringLiteral("appearance"), tr("Appearance..."), [this, node] {
      openAppearances({SelectionItem{SelectKind::Body, node.uid, QString(), QString()}});
    });
    // A sheet-metal body (mitcad#210): its rules and Flat Pattern.
    if (const QJsonObject sheet = m_snapshot.sheet(node.uid); !sheet.isEmpty()) {
      const QString rules = sheet.value(QStringLiteral("rules")).toString();
      menu.add(QStringLiteral("sheet-rules"), tr("Edit Sheet Rules"), [this, rules] { m_host.editFeature(rules); });
      if (sheet.value(QStringLiteral("flat_pattern")).isObject()) {
        menu.add(QStringLiteral("export-dxf"), tr("Export Flat Pattern..."),
                 [this, node] { m_host.exportFlatPattern(node.uid); });
      } else {
        menu.add(QStringLiteral("flat-pattern"), tr("Create Flat Pattern"), [this, node] { editFlatPattern(node.uid); });
      }
    }
    menu.separator();
    renameEntry();
    menu.add(QStringLiteral("delete"), tr("Delete"), [this, node] { deleteNode(node); });
    timelineEntry(feature);
    break;
  }
  case BrowserNode::Type::Sketch:
    menu.add(QStringLiteral("sketch"), tr("Edit Sketch"), [this, node] { m_host.editFeature(node.uid); });
    menu.add(QStringLiteral("look-at"), tr("Look At"), [this, item] { m_host.lookAt(item); });
    menu.add(QStringLiteral("export"), tr("Save As DXF"), [this, node] { m_host.exportSketch(node.uid); });
    menu.separator();
    visibility();
    renameEntry();
    menu.add(QStringLiteral("delete"), tr("Delete"), [this, node] { deleteNode(node); });
    timelineEntry(node.uid);
    break;
  case BrowserNode::Type::Datum:
    menu.add(QStringLiteral("edit"), tr("Edit %1").arg(node.name), [this, node] { m_host.editFeature(node.uid); });
    if (plane) {
      menu.add(QStringLiteral("sketch"), tr("Create Sketch"), [this, item] { m_host.createSketchOn(item); });
      menu.add(QStringLiteral("look-at"), tr("Look At"), [this, item] { m_host.lookAt(item); });
    }
    menu.separator();
    visibility();
    renameEntry();
    menu.add(QStringLiteral("delete"), tr("Delete"), [this, node] { deleteNode(node); });
    timelineEntry(node.uid);
    break;
  case BrowserNode::Type::OriginDatum:
    if (plane) {
      menu.add(QStringLiteral("sketch"), tr("Create Sketch"), [this, item] { m_host.createSketchOn(item); });
      menu.add(QStringLiteral("look-at"), tr("Look At"), [this, item] { m_host.lookAt(item); });
    }
    break;
  case BrowserNode::Type::Origin:
  case BrowserNode::Type::Bodies:
  case BrowserNode::Type::Sketches:
  case BrowserNode::Type::Construction:
    menu.add(node.visible ? QStringLiteral("eye-off") : QStringLiteral("eye"),
             node.visible ? tr("Hide All") : tr("Show All"), [this, node] { toggleVisibility(node); });
    break;
  case BrowserNode::Type::Units:
  case BrowserNode::Type::Settings:
    menu.add(QStringLiteral("units"), tr("Change Active Units"), [this] { changeUnits(); });
    break;
  case BrowserNode::Type::NamedViews:
    menu.add(QStringLiteral("named-view"), tr("New Named View"), [this] {
      if (canChange()) {
        m_host.saveNamedView(QString(), false);
      }
    });
    break;
  case BrowserNode::Type::NamedView:
    menu.add(QStringLiteral("named-view"), tr("Show View"), [this, node] { m_host.showNamedView(node.uid); });
    if (node.uid == QStringLiteral("home")) {
      menu.add(QStringLiteral("home"), tr("Set Current View as Home"), [this] {
        if (canChange()) {
          m_host.saveNamedView(QStringLiteral("Home"), true);
        }
      });
    } else if (node.uid.startsWith(QStringLiteral("named:"))) {
      menu.add(QStringLiteral("named-view"), tr("Update with Current View"), [this, node] {
        if (canChange()) {
          m_host.saveNamedView(node.uid.mid(6), true);
        }
      });
      menu.separator();
      renameEntry();
      menu.add(QStringLiteral("delete"), tr("Delete"), [this, node] { deleteNode(node); });
    }
    break;
  case BrowserNode::Type::Analyses:
    menu.add(QStringLiteral("section-analysis"), tr("New Section Analysis"), [this] {
      if (canChange()) {
        m_host.startCommand(QStringLiteral("inspect.section"), {}, nullptr);
      }
    });
    break;
  case BrowserNode::Type::Joints:
    // Joints (mitcad#55): in the component the folder is in, when active.
    for (const auto& [id, icon, text] : {std::tuple{"assemble.joint", "joint", tr("New Joint")},
                                         std::tuple{"assemble.as_built_joint", "as-built-joint", tr("New As-Built Joint")},
                                         std::tuple{"assemble.rigid_group", "rigid-group", tr("New Rigid Group")},
                                         std::tuple{"assemble.constraint", "constraint", tr("New Constraint")}}) {
      const QString command = QString::fromLatin1(id);
      menu.add(QString::fromLatin1(icon), text, [this, command] {
        if (canChange()) {
          m_host.startCommand(command, {}, nullptr);
        }
      });
    }
    break;
  case BrowserNode::Type::Joint: {
    const DocumentSnapshot::Feature* feature = m_snapshot.feature(node.uid);
    menu.add(QStringLiteral("edit"), tr("Edit %1").arg(node.name), [this, node] { m_host.editFeature(node.uid); });
    if (!m_snapshot.joint(node.uid).value(QStringLiteral("motions")).toArray().isEmpty()) {
      menu.add(QStringLiteral("drive-joint"), tr("Drive %1").arg(node.name), [this, item] {
        if (canChange()) {
          m_host.startCommand(QStringLiteral("assemble.drive_joint"), {item}, nullptr);
        }
      });
      menu.add(QStringLiteral("drive-joint"), tr("Animate %1").arg(node.name), [this, node] {
        if (canChange()) {
          m_host.animateJoint(node.uid);
        }
      });
    }
    const bool suppressed = feature != nullptr && feature->suppressed;
    menu.add(QStringLiteral("suppress"), suppressed ? tr("Unsuppress") : tr("Suppress"), [this, node, suppressed] {
      if (canChange() &&
          m_host.runModelCommand(cmd("suppress_feature", {{QStringLiteral("uid"), node.uid},
                                                          {QStringLiteral("suppressed"), !suppressed}}))) {
        qDebug().noquote() << QStringLiteral("%1 %2").arg(suppressed ? QStringLiteral("Unsuppressed")
                                                                     : QStringLiteral("Suppressed"),
                                                          node.name);
      }
    });
    menu.separator();
    renameEntry();
    menu.add(QStringLiteral("delete"), tr("Delete"), [this, node] { deleteNode(node); });
    timelineEntry(node.uid);
    break;
  }
  case BrowserNode::Type::Analysis:
    menu.add(QStringLiteral("edit"), tr("Edit Section Analysis"), [this, node] { m_host.editAnalysis(node.uid); });
    menu.separator();
    visibility();
    renameEntry();
    menu.add(QStringLiteral("delete"), tr("Delete"), [this, node] { deleteNode(node); });
    break;
  case BrowserNode::Type::Feature:
    // The timeline's menu of the feature (mitcad#177).
    featureMenu(node.uid, at, node);
    break;
  case BrowserNode::Type::SheetRules:
    menu.add(QStringLiteral("sheet-rules"), tr("Edit Sheet Rules"), [this, node] { m_host.editFeature(node.uid); });
    timelineEntry(node.uid);
    break;
  case BrowserNode::Type::FlatPattern:
    menu.add(QStringLiteral("flat-pattern"), tr("Edit Flat Pattern"), [this, node] { editFlatPattern(node.uid); });
    menu.add(node.visible ? QStringLiteral("eye-off") : QStringLiteral("eye"),
             node.visible ? tr("Show Folded") : tr("Show Flat"), [this, node] { toggleVisibility(node); });
    menu.add(QStringLiteral("export-dxf"), tr("Export DXF..."), [this, node] { m_host.exportFlatPattern(node.uid); });
    menu.separator();
    menu.add(QStringLiteral("delete"), tr("Delete"), [this, node] { deleteNode(node); });
    break;
  default:
    break;
  }
  if (!menu.isEmpty()) {
    menu.exec(at);
  }
}

// ---------------------------------------------------------------------------
// The timeline

void BrowserController::featuresClicked(const QStringList& uids) {
  // The browser shows the features' rows under their sketches too
  // (mitcad#177); a command that takes features takes them instead.
  pick(featureItems(uids), m_host.takesFeatures() ? QStringList() : uids);
}

Selection BrowserController::featureItems(const QStringList& uids) const {
  Selection items;
  const auto add = [&items](const SelectionItem& item) {
    if (item.isValid() && !items.contains(item)) {
      items.append(item);
    }
  };
  if (m_host.takesFeatures()) {
    for (const QString& uid : uids) {
      add({SelectKind::Feature, uid, QString(), QString()});
    }
    return items;
  }
  // Each feature's sketch, datum or bodies are selected. A feature that made
  // no body of its own (a fillet, a chamfer, a hole, a join) is selected as
  // the faces it made on the bodies at the marker (mitcad#28), placed by its
  // component's first occurrence as its sketches are; the main window keeps
  // it only where it finds such faces.
  for (const QString& uid : uids) {
    bool body = false;
    for (const BrowserNode& node : m_browser->nodesOfFeature(uid)) {
      body = body || node.type == BrowserNode::Type::Body;
      add(node.item());
    }
    const DocumentSnapshot::Feature* feature = m_snapshot.feature(uid);
    if (!body && feature != nullptr) {
      add({SelectKind::Feature, uid, QString(), QString(), m_snapshot.firstOccurrence(feature->component)});
    }
  }
  return items;
}

void BrowserController::pick(const Selection& items, const QStringList& features) {
  // The main window shows the selection here at once (showSelection).
  m_picking = true;
  m_pickedFeatures = features;
  m_host.pickItems(items);
  m_picking = false;
  m_pickedFeatures.clear();
}

void BrowserController::deleteFeature(const QString& uid) {
  const DocumentSnapshot::Feature* feature = m_snapshot.feature(uid);
  if (feature == nullptr || !canChange()) {
    return;
  }
  const QString name = feature->name;
  QStringList dependents;
  try {
    for (const QJsonValue& value : m_host.model()
                                       .queryObject({{QStringLiteral("query"), QStringLiteral("dependents")},
                                                     {QStringLiteral("uid"), uid}})
                                       .value(QStringLiteral("dependents"))
                                       .toArray()) {
      dependents << value.toObject().value(QStringLiteral("name")).toString();
    }
  } catch (const std::exception& e) {
    m_host.showStatus(QString::fromUtf8(e.what()), true);
    return;
  }
  if (!dependents.isEmpty()) {
    // The user is asked: the features that use it go too, or nothing does.
    qDebug().noquote() << QStringLiteral("Delete %1: also %2?").arg(name, dependents.join(QStringLiteral(", ")));
    QMessageBox box(QMessageBox::Warning, tr("Delete"), tr("Delete %1?").arg(name), QMessageBox::Cancel,
                    m_window);
    box.setInformativeText(tr("These features use it and will be deleted too:\n%1")
                               .arg(dependents.join(QLatin1Char('\n'))));
    QPushButton* all = box.addButton(tr("Delete &All"), QMessageBox::AcceptRole);
    box.setDefaultButton(all);
    prepareModal(&box);
    execDialog(&box);
    if (box.clickedButton() != all) {
      qDebug().noquote() << QStringLiteral("Delete %1 cancelled").arg(name);
      return;
    }
  }
  QJsonObject result;
  if (m_host.runModelCommand(cmd("delete_feature", {{QStringLiteral("uid"), uid},
                                                    {QStringLiteral("dependents"), !dependents.isEmpty()}}),
                             &result)) {
    qDebug().noquote() << QStringLiteral("Deleted %1%2")
                              .arg(name, dependents.isEmpty()
                                             ? QString()
                                             : QStringLiteral(", ") + dependents.join(QStringLiteral(", ")));
  }
}

void BrowserController::reorder(const QString& uid, int index) {
  const DocumentSnapshot::Feature* feature = m_snapshot.feature(uid);
  if (feature == nullptr || feature->index == index || !canChange()) {
    return;
  }
  const QString name = feature->name;
  if (m_host.runModelCommand(cmd("reorder_feature", {{QStringLiteral("uid"), uid}, {QStringLiteral("index"), index}}))) {
    qDebug().noquote() << QStringLiteral("Moved %1 to %2 in the timeline").arg(name).arg(index);
  } else {
    qDebug().noquote() << QStringLiteral("Move of %1 refused: %2").arg(name, m_host.lastError());
  }
}

void BrowserController::pickItems(const Selection& items) { pickRows(items, QStringList()); }

void BrowserController::pickRows(const Selection& items, const QStringList& features) {
  // The features of the rows are selected in the timeline (mitcad#177),
  // without its signal: sketches, datums, joints and the features under
  // sketches; a body, a component or the origin's rows clear it.
  QStringList uids = features;
  for (const SelectionItem& item : items) {
    if (item.kind != SelectKind::Body && item.kind != SelectKind::Component &&
        m_snapshot.feature(item.owner) != nullptr && !uids.contains(item.owner)) {
      uids.append(item.owner);
    }
  }
  std::sort(uids.begin(), uids.end(), [this](const QString& a, const QString& b) {
    const DocumentSnapshot::Feature* first = m_snapshot.feature(a);
    const DocumentSnapshot::Feature* second = m_snapshot.feature(b);
    return (first != nullptr ? first->index : -1) < (second != nullptr ? second->index : -1);
  });
  m_timeline->setSelectedFeatures(uids);
  if (features.isEmpty()) {
    pick(items, QStringList());
    return;
  }
  // A feature row selects what the feature's cell in the timeline does.
  Selection all = items;
  for (const SelectionItem& item : featureItems(features)) {
    if (!all.contains(item)) {
      all.append(item);
    }
  }
  pick(all, features);
}

void BrowserController::renameClicked(const BrowserNode& node) {
  // Silently nothing while the model cannot change (a command, a sketch).
  if (m_host.canChangeModel(false)) {
    m_browser->startRename(node);
  }
}

void BrowserController::restoreSearchSelection() {
  // Outside commands and sketches only: a command's inputs keep what the
  // steps gave them.
  if (!m_host.canChangeModel()) {
    return;
  }
  pick(m_searchItems, m_searchFeatures);
  qDebug().noquote() << QStringLiteral("Selection restored after the timeline search: %1 item(s)")
                            .arg(m_searchItems.size());
}

void BrowserController::findInTimeline(const QString& uid) {
  m_timeline->selectFeature(uid);
  qDebug().noquote() << QStringLiteral("Found %1 in the timeline").arg(uid);
}

void BrowserController::editFeature(const QString& uid) { m_host.editFeature(uid); }

void BrowserController::markerDragged(int position) {
  if (m_markerDepth >= 0) {
    m_host.runModelCommand(cmd("set_marker", {{QStringLiteral("position"), position}}));
  }
}

void BrowserController::setMarker(int position) {
  if (position == m_snapshot.marker || !canChange()) {
    return;
  }
  if (m_host.runModelCommand(cmd("set_marker", {{QStringLiteral("position"), position}}))) {
    qDebug().noquote() << QStringLiteral("Marker at %1 of %2").arg(position).arg(m_snapshot.features.size());
  }
}

void BrowserController::markerDragFinished(int position) {
  const int depth = m_markerDepth;
  m_markerDepth = -1;
  if (depth < 0) {
    return;
  }
  if (position == m_markerStart) {
    // Back where it was: no undo step.
    for (int guard = 0; guard < 256 && undoDepth() > depth; ++guard) {
      if (!m_host.runModelCommand(cmd("undo"))) {
        break;
      }
    }
    return;
  }
  if (m_snapshot.marker != position) {
    m_host.runModelCommand(cmd("set_marker", {{QStringLiteral("position"), position}}));
  }
  if (undoDepth() > depth + 1) {
    m_host.runModelCommand(cmd("merge_undo", {{QStringLiteral("depth"), depth},
                                              {QStringLiteral("label"), QStringLiteral("Move Timeline Marker")}}));
  }
  qDebug().noquote() << QStringLiteral("Marker at %1 of %2").arg(position).arg(m_snapshot.features.size());
}

void BrowserController::timelineMenu(const QString& uid, const QPoint& at) { featureMenu(uid, at, BrowserNode()); }

void BrowserController::featureMenu(const QString& uid, const QPoint& at, const BrowserNode& row) {
  const DocumentSnapshot::Feature* feature = m_snapshot.feature(uid);
  if (feature == nullptr) {
    return;
  }
  Menu menu(m_window);
  const bool suppressed = feature->suppressed;
  const int index = feature->index;
  menu.add(feature->isSketch() ? QStringLiteral("sketch") : QStringLiteral("edit"),
           feature->isSketch() ? tr("Edit Sketch") : tr("Edit Feature"),
           [this, uid] { m_host.editFeature(uid); });
  // Renamed in place where the menu was opened.
  menu.add(QStringLiteral("edit"), tr("Rename"), [this, uid, row] {
    if (!canChange()) {
      return;
    }
    if (row.isValid()) {
      m_browser->startRename(row);
    } else {
      m_timeline->startRename(uid);
    }
  });
  menu.add(QStringLiteral("suppress"), suppressed ? tr("Unsuppress Features") : tr("Suppress Features"),
           [this, uid, suppressed] {
             const DocumentSnapshot::Feature* f = m_snapshot.feature(uid);
             const QString name = f != nullptr ? f->name : uid;
             if (canChange() &&
                 m_host.runModelCommand(cmd("suppress_feature", {{QStringLiteral("uid"), uid},
                                                                 {QStringLiteral("suppressed"), !suppressed}}))) {
               qDebug().noquote() << QStringLiteral("%1 %2").arg(suppressed ? QStringLiteral("Unsuppressed")
                                                                            : QStringLiteral("Suppressed"),
                                                                 name);
             }
           });
  menu.add(QStringLiteral("delete"), tr("Delete"), [this, uid] { deleteFeature(uid); });
  // Moving features into components (mitcad#171): a modelled feature with
  // what goes with it, or the history of a component made from bodies.
  if (feature->type == QStringLiteral("component_from_bodies")) {
    menu.add(QStringLiteral("component"), tr("Move the Features That Made It into the Component"),
             [this, uid] { moveHistory(uid); });
  } else if (!feature->isJoint() && feature->type != QStringLiteral("move_occurrence") &&
             feature->type != QStringLiteral("capture_position") &&
             feature->type != QStringLiteral("constraint_set")) {
    const QString component = feature->component;
    menu.add(QStringLiteral("component"), tr("Move to Component..."), [this, uid, component, row] {
      QJsonObject selection{{QStringLiteral("component"), component},
                            {QStringLiteral("features"), QJsonArray{uid}}};
      // A browser row identifies an occurrence. A timeline-only selection
      // has none; the model must establish that its source is unambiguous.
      if (row.isValid()) { selection.insert(QStringLiteral("source_occurrence"), row.occurrence); }
      moveToComponent(selection);
    });
  }
  menu.separator();
  // Timeline groups (P9): the selected run (Shift+click), or this feature.
  QStringList run = m_timeline->selectedFeatures();
  if (!run.contains(uid)) {
    run = QStringList{uid};
  }
  QString group;
  for (const DocumentSnapshot::Group& g : std::as_const(m_snapshot.groups)) {
    if (g.features.contains(uid)) {
      group = g.name;
    }
  }
  if (group.isEmpty()) {
    menu.add(QStringLiteral("folder"), tr("Create Group"), [this, run] { groupFeatures(run); });
  } else {
    menu.add(QStringLiteral("folder"), tr("Collapse Group"),
             [this, group] { m_timeline->setGroupCollapsed(group, true); });
    menu.add(QString(), tr("Ungroup"), [this, group] {
      if (canChange() && m_host.runModelCommand(cmd("ungroup", {{QStringLiteral("name"), group}}))) {
        qDebug().noquote() << QStringLiteral("Ungrouped %1").arg(group);
      }
    });
  }
  menu.add(QStringLiteral("marker"), tr("Roll History Marker Here"), [this, index] { setMarker(index + 1); });
  // Found in the other place.
  if (row.isValid()) {
    menu.add(QStringLiteral("timeline"), tr("Find in Timeline"), [this, uid] { findInTimeline(uid); });
  } else {
    menu.add(QStringLiteral("browser"), tr("Find in Browser"), [this, uid] {
      const DocumentSnapshot::Feature* f = m_snapshot.feature(uid);
      if (m_browser->reveal(uid)) {
        qDebug().noquote() << QStringLiteral("Found %1 in the browser").arg(f != nullptr ? f->name : uid);
      } else {
        m_host.showStatus(tr("%1 has nothing in the browser.").arg(f != nullptr ? f->name : uid), false);
      }
    });
  }
  menu.exec(at);
}

void BrowserController::groupFeatures(const QStringList& uids) {
  if (!canChange()) {
    return;
  }
  QJsonObject result;
  if (m_host.runModelCommand(cmd("group_features", {{QStringLiteral("features"), QJsonArray::fromStringList(uids)}}),
                             &result)) {
    QStringList names;
    for (const QString& uid : uids) {
      const DocumentSnapshot::Feature* feature = m_snapshot.feature(uid);
      names << (feature != nullptr ? feature->name : uid);
    }
    qDebug().noquote() << QStringLiteral("Grouped %1 as %2")
                              .arg(names.join(QStringLiteral(", ")), result.value(QStringLiteral("name")).toString());
  }
}

void BrowserController::groupMenu(const QString& name, const QPoint& at) {
  Menu menu(m_window);
  const bool collapsed = m_timeline->isGroupCollapsed(name);
  menu.add(QStringLiteral("folder"), collapsed ? tr("Expand Group") : tr("Collapse Group"),
           [this, name, collapsed] { m_timeline->setGroupCollapsed(name, !collapsed); });
  menu.add(QStringLiteral("edit"), tr("Rename Group"), [this, name] {
    if (!canChange()) {
      return;
    }
    bool ok = false;
    const QString renamed = dialogText(m_window, tr("Rename Group"), tr("Group name:"), name, &ok)
                                .trimmed();
    if (ok && !renamed.isEmpty() && renamed != name &&
        m_host.runModelCommand(cmd("rename_group", {{QStringLiteral("name"), name},
                                                    {QStringLiteral("new_name"), renamed}}))) {
      m_timeline->groupRenamed(name, renamed);
      qDebug().noquote() << QStringLiteral("Renamed group %1 to %2").arg(name, renamed);
    }
  });
  menu.add(QString(), tr("Ungroup"), [this, name] {
    if (canChange() && m_host.runModelCommand(cmd("ungroup", {{QStringLiteral("name"), name}}))) {
      qDebug().noquote() << QStringLiteral("Ungrouped %1").arg(name);
    }
  });
  menu.exec(at);
}

void BrowserController::findInBrowser(const SelectionItem& item) {
  if (m_browser->revealItem(item)) {
    qDebug().noquote() << QStringLiteral("Found %1 in the browser").arg(item.describe());
  } else {
    m_host.showStatus(tr("Nothing in the browser stands for it."), false);
  }
}

void BrowserController::openParameters() {
  if (!m_parameters) {
    m_parameters = new ParametersDialog(m_host, m_window);
  }
  showDialog(m_parameters);
  if (!m_parameters->testAttribute(Qt::WA_DontShowOnScreen)) {
    m_parameters->raise();
    m_parameters->activateWindow();
  }
  qDebug().noquote() << "Parameters dialog opened";
  m_parameters->refresh();
}

void BrowserController::openAppearances(const Selection& targets) {
  if (!m_appearances) {
    m_appearances = new AppearanceDialog(m_host, m_window);
  }
  showDialog(m_appearances);
  if (!m_appearances->testAttribute(Qt::WA_DontShowOnScreen)) {
    m_appearances->raise();
    m_appearances->activateWindow();
  }
  qDebug().noquote() << "Appearances dialog opened";
  m_appearances->refresh();
  m_appearances->setTargets(targets);
}

} // namespace mitcad
