// SPDX-License-Identifier: MIT
#include "CommandSession.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <utility>

#include <BRepBuilderAPI_Copy.hxx>

#include <QJsonArray>
#include <QRegularExpression>
#include <QSet>
#include <QtLogging>

#include "Diagnostics.hpp"
#include "HybridAssembly.hpp"
#include "Json.hpp"
#include "ManipulatorOverlay.hpp"
#include "ModelShapes.hpp"
#include "ModelWorker.hpp"
#include "Numbers.hpp"
#include "TestSync.hpp"

namespace mitcad {
namespace {

// Previews wait for this long after the last change.
constexpr int kPreviewDelayMs = 150;
// Previews kept per command.
constexpr int kCachedPreviews = 24;
constexpr double kDegreesPerRadian = 57.29577951308232;

const QColor kFailedColor(0xd0, 0x20, 0x20);

struct InspectionShape {
  SelectionItem item;
  TopoDS_Shape shape;
  mutable TopoDS_Shape copy;
  mutable bool copied = false;
};

// This context lives only inside the document's inspection worker job.
// Placements and picked shapes were resolved while the UI owned the
// document; calculations use private geometry copies, never the window.
class WorkerInspectionContext final : public CommandContext {
public:
  WorkerInspectionContext(const Document& document, std::vector<InspectionShape> shapes)
      : m_document(document), m_shapes(std::move(shapes)) {}

  QJsonValue query(const QJsonObject& request) const override {
    const QJsonDocument answer = parseJson(m_document.query(rustStr(compactJson(request))));
    return answer.isArray() ? QJsonValue(answer.array()) : QJsonValue(answer.object());
  }

  std::shared_ptr<geometry::Shape> bodyShape(const QString& uid) const override {
    return m_document.body_shape(uid.toStdString());
  }

  TopoDS_Shape itemShape(const SelectionItem& item) const override {
    for (const InspectionShape& snapshot : m_shapes) {
      if (snapshot.item != item) { continue; }
      if (!snapshot.copied) {
        if (!snapshot.shape.IsNull()) {
          BRepBuilderAPI_Copy copier(snapshot.shape, true, false);
          snapshot.copy = copier.Shape();
        }
        snapshot.copied = true;
      }
      return snapshot.copy;
    }
    return TopoDS_Shape();
  }

  TopoDS_Shape analysisShape(const QJsonObject& request) const override {
    const auto shape = m_document.analysis_shape(rustStr(compactJson(request)));
    return shape ? shape->occt() : TopoDS_Shape();
  }

private:
  const Document& m_document;
  std::vector<InspectionShape> m_shapes;
};

QString kindKey(ValueKind kind) {
  switch (kind) {
  case ValueKind::Angle:
    return QStringLiteral("angle");
  case ValueKind::Unitless:
    return QStringLiteral("unitless");
  case ValueKind::Length:
    break;
  }
  return QStringLiteral("length");
}

// A number without trailing zeros.
QString plain(double value, int decimals) {
  QString text = QString::number(value, 'f', decimals);
  if (text.contains(QLatin1Char('.'))) {
    while (text.endsWith(QLatin1Char('0'))) {
      text.chop(1);
    }
    if (text.endsWith(QLatin1Char('.'))) {
      text.chop(1);
    }
  }
  return text == QStringLiteral("-0") ? QStringLiteral("0") : text;
}

} // namespace

CommandSession::CommandSession(const CommandDef& def, CommandHost& host, const QString& editUid,
                               QObject* parent)
    : QObject(parent), m_def(def), m_host(host), m_editUid(editUid) {
  m_previewTimer.setSingleShot(true);
  m_previewTimer.setInterval(kPreviewDelayMs);
  connect(&m_previewTimer, &QTimer::timeout, this, &CommandSession::runPreview);
  TestSync::watch(&m_previewTimer);
}

CommandSession::~CommandSession() {
  if (m_presenter != nullptr) {
    m_presenter->setActions({});
  }
  delete m_panel;
  delete m_manipulators;
}

// ---------------------------------------------------------------------------
// Inputs

QVector<CommandSession::Slot> CommandSession::slotsOf(bool shownOnly) const {
  QVector<Slot> result;
  for (const InputDef& input : m_def.inputs) {
    if (shownOnly && input.visible && !input.visible(m_state)) {
      continue;
    }
    if (input.type != InputDef::Type::List) {
      result.append({input.id, &input, QString(), -1});
      continue;
    }
    for (int i = 0; i < m_state.rows(input.id); ++i) {
      const CommandState row = m_state.row(input.id, i);
      for (const InputDef& child : input.children) {
        if (shownOnly && child.visible && !child.visible(row)) {
          continue;
        }
        result.append({CommandState::rowKey(input.id, i, child.id), &child, input.id, i});
      }
    }
  }
  return result;
}

std::optional<CommandSession::Slot> CommandSession::slotOf(const QString& key) const {
  for (const Slot& slot : slotsOf(false)) {
    if (slot.key == key) {
      return slot;
    }
  }
  return std::nullopt;
}

CommandState CommandSession::scopeOf(const Slot& slot) const {
  return slot.list.isEmpty() ? m_state : m_state.row(slot.list, slot.row);
}

bool CommandSession::isShown(const Slot& slot) const {
  if (!slot.list.isEmpty()) {
    const int list = m_def.inputIndex(slot.list);
    const InputDef& owner = m_def.inputs[list];
    if (owner.visible && !owner.visible(m_state)) {
      return false;
    }
  }
  return !slot.def->visible || slot.def->visible(scopeOf(slot));
}

void CommandSession::setDefaults(const InputDef& input, const QString& key) {
  switch (input.type) {
  case InputDef::Type::Value:
  case InputDef::Type::Text:
    if (!m_state.hasText(key)) {
      m_state.setText(key, input.defaultText);
    }
    break;
  case InputDef::Type::Choice:
    if (!m_state.hasChoice(key)) {
      m_state.setChoice(key, input.defaultChoice);
    }
    break;
  case InputDef::Type::Check:
  case InputDef::Type::Flip:
    if (!m_state.hasCheck(key)) {
      m_state.setChecked(key, input.defaultChecked);
    }
    break;
  case InputDef::Type::List:
    if (m_state.rows(key) < input.minRows) {
      m_state.setRows(key, input.minRows);
    }
    break;
  case InputDef::Type::Selection:
    break;
  }
}

void CommandSession::setRowDefaults() {
  for (const InputDef& input : m_def.inputs) {
    if (input.type != InputDef::Type::List) {
      continue;
    }
    for (int i = 0; i < m_state.rows(input.id); ++i) {
      for (const InputDef& child : input.children) {
        setDefaults(child, CommandState::rowKey(input.id, i, child.id));
      }
    }
  }
}

bool CommandSession::start(const Selection& preselection, QString& error) {
  for (const InputDef& input : m_def.inputs) {
    setDefaults(input, input.id);
  }
  setRowDefaults();

  if (isEditing()) {
    QJsonObject feature;
    QJsonArray timeline;
    int marker = 0;
    try {
      feature = m_host.queryObject(
          {{QStringLiteral("query"), QStringLiteral("feature")}, {QStringLiteral("uid"), m_editUid}});
      const QJsonObject answer =
          m_host.queryObject({{QStringLiteral("query"), QStringLiteral("timeline")}});
      timeline = answer.value(QStringLiteral("features")).toArray();
      marker = answer.value(QStringLiteral("marker")).toInt();
    } catch (const std::exception& e) {
      error = errorText(e);
      return false;
    }
    QJsonObject def = feature.value(QStringLiteral("def")).toObject();
    // With the feature's uid, so that its own parameters can be told apart.
    def.insert(QStringLiteral("uid"), m_editUid);
    const QString type = def.value(QStringLiteral("type")).toString();
    if ((type != m_def.featureType && !m_def.editsAlso.contains(type)) || !m_def.load ||
        (m_def.canEdit && !m_def.canEdit(def))) {
      error = tr("%1 cannot edit %2.").arg(m_def.name, feature.value(QStringLiteral("name")).toString());
      return false;
    }
    m_def.load(def, m_state, m_host);
    setRowDefaults();
    int position = -1;
    for (int i = 0; i < timeline.size(); ++i) {
      if (timeline[i].toObject().value(QStringLiteral("uid")).toString() == m_editUid) {
        position = i;
      }
    }
    if (position >= 0 && marker > position) {
      // Show the model as the feature finds it.
      const QJsonObject rollback{{QStringLiteral("cmd"), QStringLiteral("set_marker")},
                                 {QStringLiteral("position"), position}};
      m_markerPosition = position;
      m_originalMarker = marker;
      if (m_deferStartRollback) {
        m_startRollback = rollback;
        m_startRollback.insert(QStringLiteral("temporary"), true);
      } else {
        if (!m_host.runCommand(rollback)) {
          error = tr("Could not roll the timeline back to %1.")
                      .arg(feature.value(QStringLiteral("name")).toString());
          return false;
        }
        m_markerMoved = true;
        m_host.refreshScene();
      }
    }
  } else if (!m_analysis.isEmpty()) {
    if (!m_def.load) {
      error = tr("%1 cannot edit %2.").arg(m_def.name, m_analysis.value(QStringLiteral("name")).toString());
      return false;
    }
    m_def.load(m_analysis, m_state, m_host);
    setRowDefaults();
  } else {
    // What was selected before the command goes to the first input that
    // takes it.
    const QVector<Slot> all = slotsOf(false);
    for (const SelectionItem& picked : preselection) {
      for (const Slot& slot : all) {
        const InputDef& input = *slot.def;
        if (input.type != InputDef::Type::Selection || !input.accepts(picked) ||
            !isReferable(picked)) {
          continue;
        }
        const SelectionItem item = input.convert ? input.convert(picked) : picked;
        Selection& items = m_state.items(slot.key);
        if (!items.contains(item) && (input.max == 0 || items.size() < input.max)) {
          items.append(item);
          break;
        }
      }
    }
    if (m_def.init) {
      m_def.init(m_state, m_host);
    }
    setRowDefaults();
  }
  return true;
}

void CommandSession::startAsync(const Selection& preselection,
                                std::function<void(bool, const QString&)> completion) {
  if (m_done || m_cancelRequested) {
    if (!m_done) { finish(false); }
    completion(false, tr("Computation cancelled."));
    return;
  }
  if (m_host.modelBusy()) {
    m_host.whenIdle(this, [this, preselection, completion = std::move(completion)] {
      startAsync(preselection, completion);
    });
    return;
  }
  m_deferStartRollback = true;
  QString error;
  const bool ready = start(preselection, error);
  m_deferStartRollback = false;
  if (!ready) {
    completion(false, error);
    return;
  }
  const auto stopped = [this, completion](const QString& reason) {
    connect(this, &CommandSession::finished, this, [completion, reason](bool) {
      completion(false, reason);
    });
    finish(false);
  };
  m_asyncPending = true;
  commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("begin_command")}}},
      [this, completion, stopped](const CommandHost::AsyncResult& checkpoint) {
        if (checkpoint.succeeded()) {
          m_checkpoint = checkpoint.answer.value(QStringLiteral("token"));
        }
        if (!checkpoint.succeeded() || !m_checkpoint.isString()) {
          m_asyncPending = false;
          const QString error = checkpoint.cancelled ? tr("Computation cancelled.") : checkpoint.error;
          if (m_cancelRequested) { stopped(error); }
          else { completion(false, error); }
          return;
        }
        if (m_cancelRequested) {
          m_asyncPending = false;
          stopped(tr("Computation cancelled."));
          return;
        }
        if (m_startRollback.isEmpty()) {
          m_asyncPending = false;
          completion(true, QString());
          return;
        }
        commandSequenceAsync(QJsonArray{m_startRollback},
            [this, completion, stopped](const CommandHost::AsyncResult& result) {
              m_asyncPending = false;
              m_markerMoved = result.succeeded();
              m_startRollback = {};
              const QString error = result.cancelled ? tr("Computation cancelled.") : result.error;
              if (m_cancelRequested || !result.succeeded()) {
                if (!error.isEmpty()) { m_host.showHint(error); }
                stopped(error);
                return;
              }
              m_host.refreshScene();
              completion(true, QString());
            });
      });
}

CommandPanel* CommandSession::createPanel(QWidget* parent) {
  m_panel = new CommandPanel(m_def, m_state, isEditing() || !m_analysis.isEmpty(), parent);
  setPresenter(*m_panel);
  return m_panel;
}

void CommandSession::setPresenter(CommandPresenter& presenter) {
  if (m_presenter != nullptr) {
    m_presenter->setActions({});
  }
  m_presenter = &presenter;
  CommandPresenter::Actions actions;
  actions.textEdited = [this](const QString& key, const QString& text) {
    m_state.setText(key, text);
    const auto slot = slotOf(key);
    if (slot && slot->def->type == InputDef::Type::Value) {
      evaluate(key);
    } else if (slot) {
      qDebug().noquote() << QStringLiteral("%1 %2: %3").arg(m_def.name, slot->def->label, text);
    }
    inputsChanged();
  };
  // A value entered with decimal commas shows them as points, as the model
  // keeps it ("12,5" becomes "12.5 mm"; mitcad#2).
  actions.valueCommitted = [this](const QString& key) {
    const QString text = m_state.text(key).trimmed();
    const QString expression = m_state.expression(key);
    if (m_done || !std::isfinite(m_state.value(key)) || !text.contains(QLatin1Char(',')) ||
        expression.isEmpty() || expression == text) {
      return;
    }
    ++m_generation;
    m_state.setText(key, expression);
    evaluate(key);
    inputsChanged();
  };
  actions.choiceChanged = [this](const QString& key, const QString& value) {
    m_state.setChoice(key, value);
    if (const auto slot = slotOf(key)) {
      for (const auto& [choice, label] : slot->def->choicesIn(scopeOf(*slot))) {
        if (choice == value) {
          qDebug().noquote()
              << QStringLiteral("%1: %2 = %3").arg(m_def.name, slot->def->label, label);
          break;
        }
      }
      if (slot->def->onChange) {
        slot->def->onChange(m_state, m_host);
        // Values it set (made NaN) are evaluated again (Drive
        // Joint: the motion's value where it is).
        if (m_presenter) {
          m_presenter->refresh(m_state);
        }
        for (const Slot& other : slotsOf(true)) {
          if (other.def->type == InputDef::Type::Value && std::isnan(m_state.value(other.key))) {
            evaluate(other.key);
          }
        }
      }
    }
    inputsChanged();
  };
  actions.checkChanged = [this](const QString& key, bool on) {
    m_state.setChecked(key, on);
    if (const auto slot = slotOf(key)) {
      qDebug().noquote() << QStringLiteral("%1: %2 = %3")
                                .arg(m_def.name, slot->def->label,
                                     on ? QStringLiteral("on") : QStringLiteral("off"));
    }
    inputsChanged();
  };
  actions.selectionActivated = [this](const QString& key) { activate(key); };
  actions.selectionCleared = [this](const QString& key) {
    m_state.items(key).clear();
    activate(key);
    selectionChanged(key);
  };
  actions.itemMoved = [this](const QString& key, int from, int to) {
    Selection& items = m_state.items(key);
    if (from >= 0 && from < items.size() && to >= 0 && to < items.size()) {
      items.move(from, to);
      selectionChanged(key);
    }
  };
  actions.itemRemoved = [this](const QString& key, int index) {
    Selection& items = m_state.items(key);
    if (index >= 0 && index < items.size()) {
      items.removeAt(index);
      selectionChanged(key);
    }
  };
  actions.rowAdded = [this](const QString& list) {
    const int row = m_state.rows(list);
    m_state.setRows(list, row + 1);
    setRowDefaults();
    m_presenter->refresh(m_state);
    QString first;
    for (const Slot& slot : slotsOf(false)) {
      if (slot.list == list && slot.row == row) {
        if (slot.def->type == InputDef::Type::Value) {
          evaluate(slot.key);
        } else if (slot.def->type == InputDef::Type::Selection && first.isEmpty()) {
          first = slot.key;
        }
      }
    }
    qDebug().noquote() << QStringLiteral("%1: added %2 row %3").arg(m_def.name, list).arg(row + 1);
    if (!first.isEmpty()) {
      activate(first);
    }
    updateHighlights();
    inputsChanged();
  };
  actions.rowRemoved = [this](const QString& list, int index) {
    const int list_ = m_def.inputIndex(list);
    if (list_ < 0 || m_state.rows(list) <= m_def.inputs[list_].minRows) {
      return;
    }
    m_state.removeRow(list, index);
    qDebug().noquote() << QStringLiteral("%1: removed %2 row %3").arg(m_def.name, list).arg(index + 1);
    m_presenter->refresh(m_state);
    for (const Slot& slot : slotsOf(false)) {
      if (slot.list == list && slot.def->type == InputDef::Type::Value) {
        evaluate(slot.key);
      }
    }
    if (m_active.startsWith(list + QLatin1Char('.'))) {
      activateFirstNeeded();
    }
    updateHighlights();
    inputsChanged();
  };
  actions.accepted = [this] { commit(); };
  actions.rejected = [this] { cancel(); };

  // AppKit can deliver edits while a worker computes. Keep their values
  // queued until the model is idle, and invalidate the preview immediately.
  {
    auto action = actions.textEdited;
    actions.textEdited = [this, action](const QString& key, const QString& value) {
      inputAction([action, key, value] { action(key, value); });
    };
  }
  {
    auto action = actions.valueCommitted;
    actions.valueCommitted = [this, action](const QString& key) {
      inputAction([action, key] { action(key); }, false);
    };
  }
  {
    auto action = actions.choiceChanged;
    actions.choiceChanged = [this, action](const QString& key, const QString& value) {
      inputAction([action, key, value] { action(key, value); });
    };
  }
  {
    auto action = actions.checkChanged;
    actions.checkChanged = [this, action](const QString& key, bool value) {
      inputAction([action, key, value] { action(key, value); });
    };
  }
  {
    auto action = actions.selectionActivated;
    actions.selectionActivated = [this, action](const QString& key) {
      inputAction([action, key] { action(key); });
    };
  }
  {
    auto action = actions.selectionCleared;
    actions.selectionCleared = [this, action](const QString& key) {
      structuralInputAction(key, [action, key] { action(key); });
    };
  }
  {
    auto action = actions.itemMoved;
    actions.itemMoved = [this, action](const QString& key, int from, int to) {
      structuralInputAction(key, [action, key, from, to] { action(key, from, to); });
    };
  }
  {
    auto action = actions.itemRemoved;
    actions.itemRemoved = [this, action](const QString& key, int index) {
      structuralInputAction(key, [action, key, index] { action(key, index); });
    };
  }
  {
    auto action = actions.rowAdded;
    actions.rowAdded = [this, action](const QString& key) {
      structuralInputAction(key, [action, key] { action(key); });
    };
  }
  {
    auto action = actions.rowRemoved;
    actions.rowRemoved = [this, action](const QString& key, int index) {
      structuralInputAction(key, [action, key, index] { action(key, index); });
    };
  }
  actions.detached = [this] { presenterDetached(); };
  presenter.setActions(std::move(actions));

  for (const Slot& slot : slotsOf(false)) {
    if (slot.def->type == InputDef::Type::Value) {
      evaluate(slot.key);
    } else if (slot.def->type == InputDef::Type::Selection) {
      logSelection(slot.key);
    }
  }
  const QVector<Slot> shown = slotsOf(true);
  m_selectionsComplete = !std::any_of(shown.begin(), shown.end(), [this](const Slot& s) {
    return s.def->type == InputDef::Type::Selection && m_state.items(s.key).size() < s.def->min;
  });
  for (const Slot& slot : shown) {
    if (slot.def->type == InputDef::Type::Selection) {
      m_shownSelections.insert(slot.key);
    }
  }
  activateFirstNeeded();
  m_presenter->refresh(m_state);
  if (m_def.inspect) {
    m_sectionBefore = m_host.section();
  }
  bool handles = false;
  for (const Slot& slot : slotsOf(false)) {
    handles = handles || static_cast<bool>(slot.def->manipulator) || static_cast<bool>(slot.def->toggles);
  }
  if (handles) {
    m_manipulators = new ManipulatorOverlay(m_host.viewer(), m_def.name);
    connect(m_manipulators, &ManipulatorOverlay::dragged, this,
            &CommandSession::manipulatorDragged);
    connect(m_manipulators, &ManipulatorOverlay::toggled, this, &CommandSession::toggleElement);
  }
  updateHighlights();
  updateManipulators();
  if (isEditing()) {
    qDebug().noquote() << QStringLiteral("Editing %1 with %2").arg(m_editUid, m_def.name);
  } else if (!m_analysis.isEmpty()) {
    qDebug().noquote() << QStringLiteral("Editing analysis %1 with %2")
                              .arg(m_analysis.value(QStringLiteral("name")).toString(), m_def.name);
  } else {
    qDebug().noquote() << QStringLiteral("Command %1 started").arg(m_def.name);
  }
  // The choices the panel starts with (P11: metric defaults), for UI tests.
  QStringList choices;
  for (const InputDef& input : m_def.inputs) {
    if (input.type == InputDef::Type::Choice) {
      choices << QStringLiteral("%1=%2").arg(input.id, m_state.choice(input.id));
    }
  }
  if (!choices.isEmpty()) {
    qDebug().noquote() << QStringLiteral("Panel %1 choices: %2").arg(m_def.name, choices.join(QStringLiteral(", ")));
  }
  // Once the panel is shown and laid out.
  TestSync::singleShot(100, this, [this] {
    if (m_presenter != nullptr) {
      m_presenter->logLayout();
    }
  });
  m_previewTimer.start(0);
}

// ---------------------------------------------------------------------------
// Selection

const InputDef* CommandSession::activeInput() const {
  const auto slot = slotOf(m_active);
  return slot ? slot->def : nullptr;
}

bool CommandSession::activeTakes(SelectKind kind) const {
  const InputDef* input = activeInput();
  return input != nullptr && input->filter.testFlag(kind);
}

void CommandSession::activateFirstNeeded() {
  // The first selection that is still needed, else the first one.
  QString first;
  QString needed;
  for (const Slot& slot : slotsOf(true)) {
    if (slot.def->type != InputDef::Type::Selection) {
      continue;
    }
    if (first.isEmpty()) {
      first = slot.key;
    }
    if (needed.isEmpty() && m_state.items(slot.key).size() < slot.def->min) {
      needed = slot.key;
    }
  }
  activate(needed.isEmpty() ? first : needed);
}

void CommandSession::activate(const QString& key) {
  m_active = key;
  const InputDef* input = activeInput();
  if (input == nullptr) {
    m_active.clear();
  }
  if (m_presenter) {
    m_presenter->setActiveSelection(m_active);
  }
  OcctViewer& viewer = m_host.viewer();
  if (input == nullptr) {
    viewer.setPickFilter(SelectFilter());
    updateGhosts();
    m_host.inputActivated(SelectFilter());
    return;
  }
  viewer.setPickFilter(input->filter, [this, input](const SelectionItem& item) {
    return input->accepts(item) && isReferable(item);
  });
  // Before the input logs its pick places: the ghosts let picks through.
  updateGhosts();
  if (m_presenter) {
    viewer.setHoverColor(m_presenter->colorOf(m_active).lighter(130));
  }
  m_host.inputActivated(input->showsOrigin ? input->filter : input->filter & ~kConstructionGeometry);
  m_host.showHint(tr("%1: select %2.").arg(m_def.name, input->label.toLower()));
}

// Items made by the previewed feature itself, such as a fillet's own faces
// on a preview body, cannot be its input.
bool CommandSession::isReferable(const SelectionItem& item) const {
  switch (item.kind) {
  case SelectKind::Face:
  case SelectKind::Edge:
  case SelectKind::Vertex: {
    const auto shape = m_host.bodyShape(item.owner);
    if (!shape) {
      return false;
    }
    const std::string name = item.name.toStdString();
    if (item.kind == SelectKind::Face) {
      return !shape->find_faces(name).empty();
    }
    return item.kind == SelectKind::Edge ? !shape->find_edges(name).empty()
                                         : !shape->find_vertices(name).empty();
  }
  case SelectKind::Body:
    return m_host.bodyShape(item.owner) != nullptr;
  default:
    return true;
  }
}

void CommandSession::picked(const Selection& items, Qt::KeyboardModifiers modifiers, bool window) {
  if (m_presenter && !m_presenter->inputEnabled(m_active)) { return; }
  if (m_host.modelBusy()) {
    inputAction([this, items, modifiers, window] { picked(items, modifiers, window); });
    return;
  }
  ++m_generation;
  Q_UNUSED(modifiers);
  const InputDef* input = activeInput();
  if (input == nullptr || m_done) {
    return;
  }
  const QString key = m_active;
  Selection accepted;
  Selection raw; // as picked, with where the click hit
  for (const SelectionItem& item : items) {
    if (input->accepts(item) && isReferable(item)) {
      accepted.append(input->convert ? input->convert(item) : item);
      raw.append(item);
    }
  }
  Selection& selection = m_state.items(key);
  if (window) {
    // A window keeps one kind, and adds to what is there.
    const SelectKind kind = preferredKind(accepted);
    for (const SelectionItem& item : accepted) {
      if (item.kind == kind && !selection.contains(item) &&
          (input->max == 0 || selection.size() < input->max)) {
        selection.append(item);
      }
    }
  } else if (!accepted.isEmpty()) {
    // A click toggles; an input of one item takes the new one instead.
    const SelectionItem& item = accepted.first();
    if (selection.contains(item)) {
      if (!input->keepOnRepick) {
        selection.removeAll(item);
      }
    } else if (input->max == 1) {
      selection = {item};
    } else if (input->max == 0 || selection.size() < input->max) {
      selection.append(item);
    }
    if (input->onPick && selection.contains(item)) {
      input->onPick(raw.first(), m_state, m_host);
      if (m_presenter) {
        m_presenter->refresh(m_state);
      }
      // Values the pick set (positions, a centre) are evaluated again.
      for (const Slot& slot : slotsOf(false)) {
        if (slot.def->type == InputDef::Type::Value && std::isnan(m_state.value(slot.key))) {
          evaluate(slot.key);
        }
      }
    }
  } else {
    return; // a click on nothing changes nothing in a command
  }
  selectionChanged(key);
  advanceIfFull();
}

void CommandSession::advanceIfFull() {
  const InputDef* input = activeInput();
  if (input == nullptr || input->max == 0 || input->keepOnRepick ||
      m_state.items(m_active).size() < input->max) {
    return;
  }
  const QVector<Slot> shown = slotsOf(true);
  bool after = false;
  for (const Slot& slot : shown) {
    if (slot.key == m_active) {
      after = true;
      continue;
    }
    if (after && slot.def->type == InputDef::Type::Selection &&
        (slot.def->max == 0 || m_state.items(slot.key).size() < slot.def->max)) {
      activate(slot.key);
      return;
    }
  }
}

void CommandSession::selectionChanged(const QString& key) {
  logSelection(key);
  updateHighlights();
  inputsChanged();
}

void CommandSession::logSelection(const QString& key) const {
  const auto slot = slotOf(key);
  if (!slot) {
    return;
  }
  const Selection& items = m_state.items(key);
  QStringList names;
  for (const SelectionItem& item : items) {
    names << item.describe();
  }
  qDebug().noquote() << QStringLiteral("%1 %2: %3%4")
                            .arg(m_def.name, slot->def->label, summarize(items),
                                 names.isEmpty() ? QString()
                                                 : QStringLiteral(" [%1]").arg(names.join(
                                                       QStringLiteral("; "))));
}

void CommandSession::updateHighlights() {
  if (m_host.modelBusy()) {
    m_host.whenIdle(this, [this] { if (!m_done) { updateHighlights(); } });
    return;
  }
  std::vector<HighlightDisplay> highlights;
  for (const Slot& slot : slotsOf(true)) {
    if (slot.def->type != InputDef::Type::Selection) {
      continue;
    }
    const QColor color =
        m_failed ? kFailedColor : (m_presenter ? m_presenter->colorOf(slot.key) : QColor(Qt::blue));
    for (const SelectionItem& item : m_state.items(slot.key)) {
      highlights.push_back({item, m_host.itemShape(item), color});
    }
  }
  m_host.viewer().setHighlights(highlights);
  updateGhosts();
}

void CommandSession::updateGhosts() {
  if (m_done) {
    return;
  }
  if (m_host.modelBusy()) {
    m_host.whenIdle(this, [this] { updateGhosts(); });
    return;
  }
  QStringList ghosts;
  for (const Slot& slot : slotsOf(true)) {
    // The active input's own picks may lie on them.
    if (slot.def->type != InputDef::Type::Selection || !slot.def->ghosts || slot.key == m_active) {
      continue;
    }
    const CommandState scope = scopeOf(slot);
    for (const SelectionItem& item : m_state.items(slot.key)) {
      for (const QString& occurrence : slot.def->ghosts(item, scope, m_host)) {
        if (!occurrence.isEmpty() && !ghosts.contains(occurrence)) {
          ghosts << occurrence;
        }
      }
    }
  }
  m_host.viewer().setGhosts(ghosts);
}

// ---------------------------------------------------------------------------
// Values and previews

void CommandSession::evaluate(const QString& key) {
  const auto slot = slotOf(key);
  if (!slot) {
    return;
  }
  const InputDef& input = *slot->def;
  const QString text = m_state.text(key).trimmed();
  m_state.setValue(key, std::numeric_limits<double>::quiet_NaN());
  QString problem;
  QString hint;
  QString shown;
  if (text.isEmpty()) {
    problem = tr("Enter a value.");
  } else {
    try {
      const QJsonObject answer = m_host.queryObject({{QStringLiteral("query"), QStringLiteral("evaluate")},
                                                     {QStringLiteral("expression"), text},
                                                     {QStringLiteral("kind"), kindKey(input.valueKind)}});
      m_state.setValue(key, answer.value(QStringLiteral("value")).toDouble());
      shown = answer.value(QStringLiteral("text")).toString();
      // The model's text has decimal points where commas were typed.
      const QString expression = answer.value(QStringLiteral("expression")).toString(text);
      const bool isNumber = parsePlainNumber(text).has_value();
      const int space = static_cast<int>(shown.lastIndexOf(QLatin1Char(' ')));
      m_state.setExpression(key, isNumber && space > 0
                                     ? expression + shown.mid(space) // "20" -> "20 mm"
                                     : expression);
      // Only worth showing when the expression is more than the value.
      if (!isNumber && (!answer.value(QStringLiteral("references")).toArray().isEmpty() ||
                        text.contains(QRegularExpression(QStringLiteral("[-+*/^()]"))))) {
        hint = QStringLiteral("= %1").arg(shown);
      }
    } catch (const std::exception& e) {
      problem = errorText(e);
    }
  }
  if (m_presenter) {
    m_presenter->setInputError(key, problem);
    m_presenter->setValueHint(key, hint);
  }
  qDebug().noquote() << QStringLiteral("%1 %2: %3 %4")
                            .arg(m_def.name, input.label, text,
                                 problem.isEmpty() ? QStringLiteral("= %1").arg(shown)
                                                   : QStringLiteral("is invalid: %1").arg(problem));
}

void CommandSession::inputsChanged() {
  if (!m_active.isEmpty()) {
    // An input that a choice hid takes no more picks.
    const auto slot = slotOf(m_active);
    if (!slot || !isShown(*slot)) {
      activateFirstNeeded();
    }
  }
  // A selection a choice showed, still empty, takes the next picks
  // (Extrude's To Object).
  QSet<QString> shownSelections;
  QString newlyShown;
  for (const Slot& slot : slotsOf(true)) {
    if (slot.def->type != InputDef::Type::Selection) {
      continue;
    }
    shownSelections.insert(slot.key);
    if (newlyShown.isEmpty() && !m_shownSelections.contains(slot.key) && slot.def->min > 0 &&
        m_state.items(slot.key).isEmpty()) {
      newlyShown = slot.key;
    }
  }
  m_shownSelections = shownSelections;
  if (!newlyShown.isEmpty() && newlyShown != m_active) {
    activate(newlyShown);
  }
  if (m_presenter) {
    m_presenter->refresh(m_state);
  }
  updateManipulators();
  schedulePreview();
}

void CommandSession::updateManipulators() {
  if (m_host.modelBusy()) {
    m_host.whenIdle(this, [this] { if (!m_done) { updateManipulators(); } });
    return;
  }
  if (!m_manipulators) {
    return;
  }
  std::vector<ManipulatorOverlay::Handle> handles;
  for (const Slot& slot : slotsOf(true)) {
    if (slot.def->toggles && !m_shownReport.isEmpty()) {
      try {
        for (const Manipulator& toggle : slot.def->toggles(scopeOf(slot), m_host, m_shownReport)) {
          handles.push_back({slot.key, slot.def->label, toggle, 0.0});
        }
      } catch (const std::exception&) {
        // the model could not place them
      }
    }
    if (slot.def->type != InputDef::Type::Value || !slot.def->manipulator) {
      continue;
    }
    const double value = m_state.value(slot.key);
    if (!std::isfinite(value)) {
      continue;
    }
    std::optional<Manipulator> handle;
    try {
      handle = slot.def->manipulator(scopeOf(slot), m_host);
    } catch (const std::exception&) {
      handle.reset(); // the model could not place it
    }
    if (handle) {
      handles.push_back({slot.key, slot.def->label, *handle, value});
    }
  }
  m_manipulators->setHandles(std::move(handles));
}

void CommandSession::manipulatorDragged(const QString& key, double value) {
  if (m_presenter && !m_presenter->inputEnabled(key)) { return; }
  if (m_host.modelBusy()) {
    inputAction([this, key, value] { manipulatorDragged(key, value); });
    return;
  }
  ++m_generation;
  const auto slot = slotOf(key);
  if (!slot || m_done) {
    return;
  }
  QString text;
  switch (slot->def->valueKind) {
  case ValueKind::Length:
    text = plain(value / m_host.unitMillimetres(), 4) + QLatin1Char(' ') + m_host.lengthUnit();
    break;
  case ValueKind::Angle:
    text = plain(value * kDegreesPerRadian, 2) + QStringLiteral(" deg");
    break;
  case ValueKind::Unitless:
    text = plain(value, 4);
    break;
  }
  m_state.setText(key, text);
  evaluate(key);
  inputsChanged();
}

void CommandSession::toggleElement(const QString& key, int element) {
  if (m_presenter && !m_presenter->inputEnabled(key)) { return; }
  if (m_host.modelBusy()) {
    inputAction([this, key, element] { toggleElement(key, element); });
    return;
  }
  ++m_generation;
  if (m_done) {
    return;
  }
  QList<int> numbers;
  for (const QString& part : m_state.text(key).split(QLatin1Char(','), Qt::SkipEmptyParts)) {
    bool ok = false;
    const int number = part.trimmed().toInt(&ok);
    if (ok && !numbers.contains(number)) {
      numbers.append(number);
    }
  }
  if (numbers.contains(element)) {
    numbers.removeAll(element);
  } else {
    numbers.append(element);
  }
  std::sort(numbers.begin(), numbers.end());
  QStringList texts;
  for (const int number : numbers) {
    texts << QString::number(number);
  }
  m_state.setText(key, texts.join(QStringLiteral(", ")));
  if (const auto slot = slotOf(key)) {
    qDebug().noquote() << QStringLiteral("%1 %2: %3").arg(m_def.name, slot->def->label, m_state.text(key));
  }
  inputsChanged();
}

void CommandSession::schedulePreview() {
  if (!m_done) {
    m_previewTimer.start(kPreviewDelayMs);
  }
}

bool CommandSession::previewReady(QString& message, bool& error) {
  for (const Slot& slot : slotsOf(true)) {
    const InputDef& input = *slot.def;
    if (input.type == InputDef::Type::Selection && m_state.items(slot.key).size() < input.min) {
      message = tr("Select %1.").arg(input.label.toLower());
      error = false;
      return false;
    }
    if (input.type == InputDef::Type::Value && std::isnan(m_state.value(slot.key))) {
      message = tr("%1: check the value.").arg(input.label);
      error = true;
      return false;
    }
  }
  return true;
}

void CommandSession::runInspection() {
  QString message;
  bool error = false;
  if (!previewReady(message, error)) {
    m_presenter->setResult(QString());
    m_presenter->setMessage(message, error);
    m_host.viewer().setPreview(TopoDS_Shape());
    m_previewOk = true; // OK closes it anyway
    m_shownKey = QByteArrayLiteral("inspection");
    m_presenter->setOkEnabled(true);
    return;
  }
  if (m_host.asynchronousCommands()) {
    std::vector<InspectionShape> shapes;
    try {
      for (const Slot& slot : slotsOf(true)) {
        if (slot.def->type != InputDef::Type::Selection) { continue; }
        for (const SelectionItem& item : m_state.items(slot.key)) {
          const bool captured = std::any_of(shapes.begin(), shapes.end(), [&item](const InspectionShape& shape) {
            return shape.item == item;
          });
          if (!captured) { shapes.push_back({item, m_host.itemShape(item), {}, false}); }
        }
      }
    } catch (const std::exception& failure) {
      Inspection result;
      result.error = errorText(failure);
      acceptInspection(result);
      return;
    }
    const CommandState state = m_state;
    const auto inspect = m_def.inspect;
    const quint64 generation = m_generation;
    m_asyncPending = true;
    m_presenter->setMessage(tr("Computing inspection…"), false);
    m_presenter->setOkEnabled(false);
    const bool started = m_host.modelInspectionAsync(m_def.name, this,
        [state, inspect, shapes = std::move(shapes)](const Document& document) {
          const WorkerInspectionContext context(document, shapes);
          return inspect(state, context);
        }, [this, generation](const Inspection& result, const CommandHost::AsyncResult& outcome) {
          if (!asyncPresenterAvailable()) { return; }
          m_asyncPending = false;
          if (m_cancelRequested) {
            finishAsyncCancel();
            return;
          }
          if (generation != m_generation) {
            schedulePreview();
            return;
          }
          if (outcome.cancelled) {
            m_previewOk = true; // Close remains available after cancellation.
            m_shownKey = QByteArrayLiteral("inspection");
            m_presenter->setResult(QString());
            m_presenter->setMessage(tr("Inspection cancelled; change an input to try again."), false);
            m_presenter->setOkEnabled(true);
            m_host.viewer().setPreview(TopoDS_Shape());
            qDebug().noquote() << QStringLiteral("Inspect %1: cancelled").arg(m_def.name);
          } else {
            Inspection shown = result;
            if (!outcome.error.isEmpty()) { shown.error = outcome.error; }
            acceptInspection(shown);
          }
          resumePendingCommit();
        });
    if (!started) {
      m_asyncPending = false;
      schedulePreview();
    }
    return;
  }
  Inspection result;
  try {
    result = m_def.inspect(m_state, m_host);
  } catch (const std::exception& e) {
    result.error = errorText(e);
  }
  acceptInspection(result);
}

void CommandSession::acceptInspection(const Inspection& result) {
  if (m_done || !m_presenter) { return; }
  m_failed = !result.error.isEmpty();
  m_presenter->setResult(result.text);
  m_presenter->setMessage(result.error, m_failed);
  m_host.viewer().setPreview(result.shape, result.red ? OcctViewer::PreviewStyle::Remove
                                                      : OcctViewer::PreviewStyle::Add);
  if (m_def.keepsSection || !result.section.isNull()) {
    m_host.showSection(result.section);
  }
  qDebug().noquote() << QStringLiteral("Inspect %1: %2")
                            .arg(m_def.name, m_failed ? QStringLiteral("failed: ") + result.error
                                                      : result.log);
  m_previewOk = true;
  m_shownKey = QByteArrayLiteral("inspection");
  m_presenter->setOkEnabled(true);
  updateHighlights();
}

void CommandSession::runPreview() {
  m_previewTimer.stop();
  if (m_done || !m_presenter) {
    return;
  }
  if (m_host.modelBusy()) {
    // The timer ran out while a job computes the model (the caller of the
    // job waits in an event loop): after it.
    m_previewTimer.start(kPreviewDelayMs);
    return;
  }
  if (m_def.inspect) {
    runInspection();
    return;
  }
  QString message;
  bool error = false;
  Built built;
  if (previewReady(message, error)) {
    built = m_def.build(m_state, m_host);
    if (!built.edit.isEmpty()) {
      // The inputs ask for another feature's edit (P6).
      const QString field = built.editInput.section(QLatin1Char('.'), -1);
      qDebug().noquote() << QStringLiteral("%1 opened %2 for its %3")
                                .arg(m_def.name, m_host.featureName(built.edit), field);
      m_host.editInstead(built.edit, built.editInput);
      return;
    }
    if (!built.error.isEmpty()) {
      message = built.error;
      error = true;
    }
  }
  if (!message.isEmpty()) {
    clearPreview();
    m_shownKey.clear();
    m_previewOk = false;
    m_failed = error;
    m_presenter->setMessage(message, error);
    m_presenter->setOkEnabled(false);
    updateHighlights();
    return;
  }
  if (m_def.modelCommand) {
    // Nothing to show before it runs.
    m_shownKey = compactJson(built.def);
    m_previewOk = true;
    m_failed = false;
    m_presenter->setMessage(QString(), false);
    m_presenter->setOkEnabled(true);
    m_host.showHint(tr("%1: Enter to confirm, Esc to cancel.").arg(m_def.name));
    updateHighlights();
    return;
  }
  if (m_def.sketchCommand) {
    if (m_host.asynchronousCommands()) {
      showSketchPreviewAsync(built);
    } else {
      showSketchPreview(built);
    }
    return;
  }

  const QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("add_feature")},
                            {QStringLiteral("def"), built.def}};
  const QByteArray key = compactJson(command);
  if (key == m_shownKey && !m_previewCancelled) {
    return; // already shown
  }
  m_previewCancelled = false;
  const auto cached = m_cache.constFind(key);
  if (cached != m_cache.cend()) {
    acceptFeaturePreview(key, built, cached.value(), false);
    resumePendingCommit();
    return;
  }
  if (m_host.asynchronousCommands()) {
    const quint64 generation = m_generation;
    m_asyncPending = true;
    m_presenter->setMessage(tr("Computing preview…"), false);
    m_presenter->setOkEnabled(false);
    const bool started = m_host.modelPreviewAsync(key, m_def.name, this,
        [this, generation, key, built](const CommandHost::AsyncResult& result) {
          if (!asyncPresenterAvailable()) { return; }
          m_asyncPending = false;
          if (m_cancelRequested) {
            finishAsyncCancel();
            return;
          }
          if (generation != m_generation) {
            schedulePreview();
            return;
          }
          if (result.cancelled) {
            showCancelledPreview(key);
          } else {
            const QJsonObject report = result.error.isEmpty() ? result.answer
                : QJsonObject{{QStringLiteral("status"), QStringLiteral("error")},
                              {QStringLiteral("error"), result.error}};
            acceptFeaturePreview(key, built, previewFromReport(report), true);
          }
          resumePendingCommit();
        });
    if (!started) {
      m_asyncPending = false;
      schedulePreview();
    }
    return;
  }
  QJsonObject report;
  try {
    report = m_host.modelPreview(key, m_def.name);
  } catch (const ComputationCancelled&) {
    showCancelledPreview(key);
    return;
  } catch (const std::exception& error) {
    report = {{QStringLiteral("status"), QStringLiteral("error")},
              {QStringLiteral("error"), errorText(error)}};
  }
  acceptFeaturePreview(key, built, previewFromReport(report), true);
}

CommandSession::Preview CommandSession::previewFromReport(const QJsonObject& report) {
  Preview preview;
  preview.report = report;
  if (preview.report.value(QStringLiteral("status")).toString() == QStringLiteral("ok")) {
    // The bodies as shown now (placed by their occurrences), changed ones
    // replaced by the preview's, removed ones left out, new ones added.
    QHash<QString, bool> after; // uid: changed
    for (const QJsonValue& value : preview.report.value(QStringLiteral("bodies")).toArray()) {
      const QJsonObject body = value.toObject();
      after.insert(body.value(QStringLiteral("uid")).toString(),
                   body.value(QStringLiteral("changed")).toBool());
    }
    QSet<QString> shown;
    for (BodyDisplay display : m_host.modelBodies()) {
      const QString uid = QString::fromStdString(display.uid);
      if (!after.contains(uid)) {
        continue;
      }
      if (after.value(uid)) {
        display.shape = m_host.previewBodyShape(display.uid);
      }
      shown.insert(uid);
      preview.bodies.push_back(display);
    }
    for (auto it = after.cbegin(); it != after.cend(); ++it) {
      if (!shown.contains(it.key())) {
        BodyDisplay display;
        display.uid = it.key().toStdString();
        display.shape = m_host.previewBodyShape(display.uid);
        display.style = BodyDisplay::Style::New;
        preview.bodies.push_back(display);
      }
    }
    // Occurrences the command places elsewhere (joints, moves of
    // occurrences; mitcad#55): their bodies where it puts them.
    QHash<QString, QJsonArray> placed;
    for (const QJsonValue& value : preview.report.value(QStringLiteral("placements")).toArray()) {
      const QJsonObject placement = value.toObject();
      placed.insert(placement.value(QStringLiteral("path")).toString(),
                    placement.value(QStringLiteral("transform")).toArray());
    }
    for (BodyDisplay& display : preview.bodies) {
      const auto moved = placed.constFind(QString::fromStdString(display.occurrence));
      if (moved != placed.cend()) {
        display.placement = isIdentity(moved.value()) ? TopLoc_Location() : TopLoc_Location(trsfOf(moved.value()));
      }
    }
    if (const auto tool = m_host.previewToolShape()) {
      preview.tool = tool->occt();
    } else if (preview.report.contains(QStringLiteral("datum"))) {
      // A construction feature shows its plane, axis or point.
      preview.tool = datumShape(preview.report.value(QStringLiteral("datum")).toObject(), 100.0);
    }
  }
  return preview;
}

void CommandSession::acceptFeaturePreview(const QByteArray& key, const Built& built,
                                         const Preview& preview, bool cache) {
  if (m_done || !m_presenter) { return; }
  if (cache) {
    if (m_cacheOrder.size() >= kCachedPreviews) {
      m_cache.remove(m_cacheOrder.takeFirst());
    }
    m_cache.insert(key, preview);
    m_cacheOrder.append(key);
  }
  m_shownKey = key;
  const QString status = preview.report.value(QStringLiteral("status")).toString();
  m_previewOk = status == QStringLiteral("ok");
  m_failed = !m_previewOk;
  if (m_previewOk) {
    showPreview(preview, built.def.value(QStringLiteral("operation")).toString());
    // A feature that succeeds with a caveat says so (P9), not in red.
    const QStringList warnings = preview.report.value(QStringLiteral("warnings")).toVariant().toStringList();
    m_presenter->setMessage(warnings.join(QLatin1Char('\n')), false);
    m_host.showHint(tr("%1: Enter to confirm, Esc to cancel.").arg(m_def.name));
    qDebug().noquote() << QStringLiteral("Preview %1: ok%2")
                              .arg(m_def.name, warnings.isEmpty()
                                                   ? QString()
                                                   : QStringLiteral(", warning: ") + warnings.join(QStringLiteral("; ")));
    // The occurrences it shows elsewhere (mitcad#55), for UI tests.
    QStringList moved;
    for (const QJsonValue& value : preview.report.value(QStringLiteral("placements")).toArray()) {
      moved << value.toObject().value(QStringLiteral("path")).toString();
    }
    if (!moved.isEmpty()) {
      qDebug().noquote() << QStringLiteral("Preview %1 moves %2").arg(m_def.name, moved.join(QStringLiteral(", ")));
    }
  } else {
    clearPreview();
    const QString problem = preview.report.value(QStringLiteral("error")).toString();
    m_presenter->setMessage(problem, true);
    qDebug().noquote() << QStringLiteral("Preview %1: failed: %2").arg(m_def.name, problem);
  }
  m_presenter->setOkEnabled(m_previewOk);
  updateHighlights();
  // Toggles follow the shown preview (a pattern's instances, P9).
  const QJsonObject shown = m_previewOk ? preview.report : QJsonObject();
  if (shown != m_shownReport) {
    m_shownReport = shown;
    updateManipulators();
  }
}

void CommandSession::showPreview(const Preview& preview, const QString& operation) {
  OcctViewer& viewer = m_host.viewer();
  viewer.setBodies(preview.bodies);
  // New bodies show as see-through bodies; a tool that joins, cuts or
  // intersects shows over the result, red for a cut; so does
  // a construction feature's datum.
  if (operation == QStringLiteral("cut")) {
    viewer.setPreview(preview.tool, OcctViewer::PreviewStyle::Remove);
  } else if (operation == QStringLiteral("new_body") ||
             operation == QStringLiteral("new_component")) {
    viewer.setPreview(TopoDS_Shape());
  } else {
    viewer.setPreview(preview.tool, OcctViewer::PreviewStyle::Add);
  }
}

void CommandSession::showCancelledPreview(const QByteArray& key) {
  if (m_done || !m_presenter) { return; }
  clearPreview();
  m_shownKey = key;
  m_previewCancelled = true;
  m_previewOk = true; // OK computes it, with a progress and a cancel of its own
  m_failed = false;
  m_presenter->setMessage(tr("Preview cancelled; change an input to preview again."), false);
  m_presenter->setOkEnabled(true);
  updateHighlights();
  if (!m_shownReport.isEmpty()) {
    m_shownReport = QJsonObject();
    updateManipulators();
  }
  qDebug().noquote() << QStringLiteral("Preview %1: cancelled").arg(m_def.name);
}

int CommandSession::undoDepth() const {
  try {
    return m_host.queryObject({{QStringLiteral("query"), QStringLiteral("document")}})
        .value(QStringLiteral("undo_depth"))
        .toInt();
  } catch (const std::exception&) {
    return 0;
  }
}

void CommandSession::showSketchPreview(const Built& built) {
  const QByteArray key = compactJson(built.def);
  if (key == m_shownKey && m_appliedDepth >= 0) {
    return;
  }
  takeBackSketchPreview();
  const QJsonArray commands = built.def.contains(QStringLiteral("commands"))
                                  ? built.def.value(QStringLiteral("commands")).toArray()
                                  : QJsonArray{built.def};
  const int depth = undoDepth();
  QString problem;
  bool cancelled = false;
  for (const QJsonValue& command : commands) {
    try {
      m_host.modelCommand(command.toObject());
    } catch (const ComputationCancelled& e) {
      problem = errorText(e);
      cancelled = true;
      break;
    } catch (const std::exception& e) {
      problem = errorText(e);
      break;
    }
  }
  m_appliedDepth = depth;
  if (!problem.isEmpty()) {
    takeBackSketchPreview();
  }
  m_shownKey = key;
  m_previewCancelled = cancelled;
  if (cancelled) {
    // Not applied: OK applies it (commit).
    m_previewOk = true;
    m_failed = false;
    m_host.refreshScene();
    m_presenter->setMessage(tr("Preview cancelled; change an input to preview again."), false);
    m_presenter->setOkEnabled(true);
    updateHighlights();
    qDebug().noquote() << QStringLiteral("Preview %1: cancelled").arg(m_def.name);
    return;
  }
  m_previewOk = problem.isEmpty();
  m_failed = !m_previewOk;
  m_host.refreshScene();
  if (m_previewOk) {
    m_presenter->setMessage(QString(), false);
    m_host.showHint(tr("%1: Enter to confirm, Esc to cancel.").arg(m_def.name));
    qDebug().noquote() << QStringLiteral("Preview %1: ok").arg(m_def.name);
  } else {
    m_presenter->setMessage(problem, true);
    qDebug().noquote() << QStringLiteral("Preview %1: failed: %2").arg(m_def.name, problem);
  }
  m_presenter->setOkEnabled(m_previewOk);
  updateHighlights();
}

void CommandSession::takeBackSketchPreview() {
  if (m_host.asynchronousCommands() && m_appliedDepth >= 0) {
    const int depth = m_appliedDepth;
    m_appliedDepth = -1;
    m_shownKey.clear();
    m_asyncPending = true;
    undoToDepthAsync(depth, [this, depth](const CommandHost::AsyncResult& result) {
      m_asyncPending = false;
      m_host.refreshScene();
      if (!result.succeeded()) {
        m_appliedDepth = depth;
        m_previewOk = false;
        m_presenter->setMessage(result.error, true);
      }
      if (m_cancelRequested) { finishAsyncCancel(); }
      else { resumePendingCommit(); }
    });
    return;
  }
  if (m_appliedDepth < 0) {
    return;
  }
  const int depth = m_appliedDepth;
  m_appliedDepth = -1;
  m_shownKey.clear();
  for (int guard = 0; guard < 64 && undoDepth() > depth; ++guard) {
    try {
      m_host.modelCommand({{QStringLiteral("cmd"), QStringLiteral("undo")}});
    } catch (const std::exception&) {
      break;
    }
  }
  m_host.refreshScene();
}

void CommandSession::clearPreview() {
  if (m_def.sketchCommand) {
    takeBackSketchPreview();
    return;
  }
  if (m_def.modelCommand) {
    return;
  }
  m_host.clearModelPreview();
  m_host.viewer().setBodies(m_host.modelBodies());
  m_host.viewer().setPreview(TopoDS_Shape());
}

// ---------------------------------------------------------------------------
// OK and Cancel

QJsonObject CommandSession::bodyVolumes() const {
  QJsonObject volumes;
  if (!volumesLogged()) {
    return volumes;
  }
  try {
    for (const QJsonValue& value :
         m_host.query({{QStringLiteral("query"), QStringLiteral("bodies")},
                       {QStringLiteral("volumes"), true}})
             .toArray()) {
      const QJsonObject body = value.toObject();
      volumes.insert(body.value(QStringLiteral("uid")).toString(), body);
    }
  } catch (const std::exception&) {
    // Volumes are only logged.
  }
  return volumes;
}

void CommandSession::logVolumes(const QJsonObject& before) const {
  if (!volumesLogged()) {
    return;
  }
  const QJsonObject after = bodyVolumes();
  const auto volume = [](const QJsonValue& body) {
    return body.toObject().value(QStringLiteral("volume")).toDouble();
  };
  const auto name = [](const QJsonValue& body) {
    return body.toObject().value(QStringLiteral("name")).toString();
  };
  for (auto it = after.begin(); it != after.end(); ++it) {
    const double now = volume(it.value());
    if (!before.contains(it.key())) {
      qDebug().noquote() << QStringLiteral("New body %1 (%2): volume %3 mm3")
                                .arg(name(it.value()), it.key())
                                .arg(now, 0, 'f', 3);
      continue;
    }
    const double was = volume(before.value(it.key()));
    if (std::abs(now - was) > 1e-9 * std::max(1.0, std::abs(was))) {
      qDebug().noquote() << QStringLiteral("Body %1 (%2): volume %3 -> %4 mm3")
                                .arg(name(it.value()), it.key())
                                .arg(was, 0, 'f', 3)
                                .arg(now, 0, 'f', 3);
    }
  }
  for (auto it = before.begin(); it != before.end(); ++it) {
    if (!after.contains(it.key())) {
      qDebug().noquote() << QStringLiteral("Removed body %1 (%2)").arg(name(it.value()), it.key());
    }
  }
}

void CommandSession::inputAction(std::function<void()> action, bool invalidatesPreview,
                                 std::function<void()> rejected) {
  if (m_done || m_cancelRequested || m_committing) {
    if (rejected) { rejected(); }
    return;
  }
  if (invalidatesPreview) {
    ++m_generation;
    m_commitRequested = false;
  }
  if (m_host.modelBusy()) {
    if (invalidatesPreview && m_host.asynchronousCommands() && m_asyncPending) {
      m_host.cancelModelJob();
    }
    m_host.whenIdle(this, [this, action = std::move(action), rejected = std::move(rejected)] {
      if (!m_done && !m_cancelRequested && !m_committing) {
        action();
      } else if (rejected) {
        rejected();
      }
    });
    return;
  }
  action();
}

void CommandSession::structuralInputAction(const QString& key, std::function<void()> action) {
  const auto applied = [this, key] {
    if (m_presenter) { m_presenter->inputActionApplied(key); }
  };
  inputAction([action = std::move(action), applied] {
    action();
    applied();
  }, true, applied);
}

void CommandSession::presenterDetached() {
  if (m_panel && m_presenter == static_cast<CommandPresenter*>(m_panel)) {
    m_panel = nullptr;
  }
  m_presenter = nullptr;
  m_previewTimer.stop();
  ++m_generation;
  if (!m_done && m_host.asynchronousCommands()) {
    // A frontend may disappear while its worker still owns the document.
    // Keep the session alive until cancellation restores its checkpoint.
    cancelAsync();
  }
}

bool CommandSession::asyncPresenterAvailable() {
  if (m_done || m_finalizing) { return false; }
  if (m_presenter) { return true; }
  m_asyncPending = false;
  m_cancelRequested = true;
  m_commitRequested = false;
  finishAsyncCancel();
  return false;
}

void CommandSession::resumePendingCommit() {
  if (m_commitRequested && !m_asyncPending && !m_host.modelBusy() && !m_done) {
    m_commitRequested = false;
    const quint64 generation = m_generation;
    TestSync::singleShot(0, this, [this, generation] {
      if (generation == m_generation && !m_done && !m_cancelRequested) { commitAsync(); }
    });
  }
}

void CommandSession::commandSequenceAsync(const QJsonArray& commands,
                                          CommandHost::AsyncCompletion completion, int index,
                                          std::function<bool()> aborted) {
  if (aborted && aborted()) {
    completion({{}, QString(), true});
    return;
  }
  if (index >= commands.size()) {
    completion({});
    return;
  }
  if (m_host.modelBusy()) {
    m_host.whenIdle(this, [this, commands, completion = std::move(completion), index, aborted] {
      commandSequenceAsync(commands, completion, index, aborted);
    });
    return;
  }
  const bool started = m_host.modelCommandAsync(commands[index].toObject(), this,
      [this, commands, completion, index, aborted](const CommandHost::AsyncResult& result) {
        if (!result.succeeded()) {
          completion(result);
        } else if (aborted && aborted()) {
          completion({{}, QString(), true});
        } else if (index + 1 >= commands.size()) {
          completion(result);
        } else {
          commandSequenceAsync(commands, completion, index + 1, aborted);
        }
      });
  if (!started) {
    completion({{}, tr("Could not start the model command."), false});
  }
}

void CommandSession::undoToDepthAsync(int depth, CommandHost::AsyncCompletion completion, int guard) {
  if (m_done || m_finalizing) {
    completion({{}, QString(), true});
    return;
  }
  if (m_host.modelBusy()) {
    m_host.whenIdle(this, [this, depth, completion = std::move(completion), guard] {
      undoToDepthAsync(depth, completion, guard);
    });
    return;
  }
  const int currentDepth = undoDepth();
  if (currentDepth <= depth) {
    completion({});
    return;
  }
  if (guard < 0) {
    // A batch can contain more than 64 selections. Allow one undo per
    // accepted command, with room to retry a cancelled recovery job.
    guard = currentDepth - depth + 8;
  }
  if (guard <= 0) {
    completion({{}, tr("Could not restore the command's undo state."), false});
    return;
  }
  commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("undo")}}},
      [this, depth, completion, guard](const CommandHost::AsyncResult& result) {
        if (!result.succeeded() && !result.cancelled) {
          completion(result);
        } else {
          undoToDepthAsync(depth, completion, guard - 1);
        }
      });
}

void CommandSession::restoreCheckpointAsync(CommandHost::AsyncCompletion completion) {
  if (!m_checkpoint.isString()) {
    completion({});
    return;
  }
  commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("restore_command")},
                                              {QStringLiteral("token"), m_checkpoint}}},
      [this, completion](const CommandHost::AsyncResult& result) {
        if (result.cancelled) {
          restoreCheckpointAsync(completion);
          return;
        }
        if (result.succeeded()) {
          m_markerMoved = false;
          m_appliedDepth = -1;
          m_shownKey.clear();
        }
        completion(result);
      });
}

void CommandSession::rollForwardAsync(CommandHost::AsyncCompletion completion) {
  if (m_done || m_finalizing) {
    completion({{}, QString(), true});
    return;
  }
  if (!m_markerMoved) {
    completion({});
    return;
  }
  const QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("set_marker")},
                           {QStringLiteral("position"), m_originalMarker},
                           {QStringLiteral("temporary"), true}};
  commandSequenceAsync(QJsonArray{command}, [this, completion](const CommandHost::AsyncResult& result) {
    if (result.cancelled) {
      // Restoring the editor's marker must finish before releasing its
      // session, including a late cancellation of the restoration job.
      rollForwardAsync(completion);
      return;
    }
    m_markerMoved = !result.succeeded();
    completion(result);
  });
}

void CommandSession::showSketchPreviewAsync(const Built& built) {
  const QByteArray key = compactJson(built.def);
  if (key == m_shownKey && m_appliedDepth >= 0) {
    resumePendingCommit();
    return;
  }
  const quint64 generation = m_generation;
  const int oldDepth = m_appliedDepth;
  m_appliedDepth = -1;
  m_asyncPending = true;
  m_presenter->setOkEnabled(false);
  const auto apply = [this, generation, built, key, oldDepth](const CommandHost::AsyncResult& undone) {
    if (!asyncPresenterAvailable()) { return; }
    if (!undone.succeeded()) {
      m_asyncPending = false;
      m_appliedDepth = oldDepth;
      m_cancelRequested = false;
      m_previewOk = false;
      m_presenter->setMessage(undone.error, true);
      return;
    }
    if (m_cancelRequested) {
      m_asyncPending = false;
      finishAsyncCancel();
      return;
    }
    if (generation != m_generation) {
      m_asyncPending = false;
      schedulePreview();
      return;
    }
    const QJsonArray commands = built.def.contains(QStringLiteral("commands"))
        ? built.def.value(QStringLiteral("commands")).toArray() : QJsonArray{built.def};
    const int depth = undoDepth();
    commandSequenceAsync(commands, [this, generation, key, depth](const CommandHost::AsyncResult& result) {
      if (!asyncPresenterAvailable()) { return; }
      m_appliedDepth = depth;
      const auto show = [this, generation, key, result, depth](const CommandHost::AsyncResult& undone) {
        if (!asyncPresenterAvailable()) { return; }
        m_asyncPending = false;
        if (!undone.succeeded()) {
          m_appliedDepth = depth;
          m_cancelRequested = false;
          m_previewOk = false;
          m_presenter->setMessage(undone.error, true);
          return;
        }
        if (m_cancelRequested) {
          finishAsyncCancel();
          return;
        }
        if (generation != m_generation) {
          schedulePreview();
          return;
        }
        m_shownKey = key;
        m_previewCancelled = result.cancelled;
        m_previewOk = result.succeeded() || result.cancelled;
        m_failed = !m_previewOk;
        m_host.refreshScene();
        m_presenter->setMessage(result.cancelled
            ? tr("Preview cancelled; change an input to preview again.") : result.error, !m_previewOk);
        m_presenter->setOkEnabled(m_previewOk);
        updateHighlights();
        qDebug().noquote() << QStringLiteral("Preview %1: %2").arg(m_def.name,
            result.cancelled ? QStringLiteral("cancelled")
                             : (m_previewOk ? QStringLiteral("ok") : QStringLiteral("failed: ") + result.error));
        resumePendingCommit();
      };
      if (!result.succeeded() || generation != m_generation || m_cancelRequested) {
        m_appliedDepth = -1;
        undoToDepthAsync(depth, show);
      } else {
        show({});
      }
    }, 0, [this, generation] { return m_cancelRequested || generation != m_generation; });
  };
  if (oldDepth >= 0) {
    undoToDepthAsync(oldDepth, apply);
  } else {
    apply({});
  }
}

void CommandSession::recoverFailedCommit(const CommandHost::AsyncResult& result) {
  if (!asyncPresenterAvailable()) { return; }
  if (m_cancelRequested) {
    m_committing = false;
    finishAsyncCancel();
    return;
  }
  const auto show = [this, result](const CommandHost::AsyncResult& recovered) {
    if (!asyncPresenterAvailable()) { return; }
    m_asyncPending = false;
    m_committing = false;
    if (m_cancelRequested) {
      finishAsyncCancel();
      return;
    }
    if (recovered.cancelled) {
      recoverFailedCommit(result);
      return;
    }
    if (!recovered.succeeded()) {
      m_previewOk = false;
      m_presenter->setMessage(recovered.error, true);
      return;
    }
    m_host.refreshScene();
    if (m_presenter) { m_presenter->refresh(m_state); }
    m_presenter->setMessage(result.cancelled ? tr("Computation cancelled.") : result.error,
                            !result.cancelled);
    m_presenter->setOkEnabled(true);
    qDebug().noquote() << QStringLiteral("%1: OK %2").arg(m_def.name,
        result.cancelled ? QStringLiteral("cancelled") : QStringLiteral("not done"));
    schedulePreview();
  };
  if (!m_checkpoint.isString()) { show({}); return; }
  m_asyncPending = true;
  m_committing = true;
  restoreCheckpointAsync([this, show](const CommandHost::AsyncResult& restored) {
    if (!restored.succeeded() || m_cancelRequested || !isEditing() ||
        m_originalMarker <= m_markerPosition) {
      show(restored);
      return;
    }
    commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("set_marker")},
        {QStringLiteral("position"), m_markerPosition}, {QStringLiteral("temporary"), true}}},
        [this, show](const CommandHost::AsyncResult& rolled) {
          m_markerMoved = rolled.succeeded();
          show(rolled);
        });
  });
}

bool CommandSession::commitAsync() {
  if (m_done || m_cancelRequested || m_committing) {
    return false;
  }
  if (m_asyncPending || m_host.modelBusy()) {
    m_commitRequested = true;
    m_host.whenIdle(this, [this] { resumePendingCommit(); });
    return true;
  }
  if (m_previewTimer.isActive() || m_shownKey.isEmpty()) {
    m_commitRequested = true;
    runPreview();
    if (m_asyncPending || m_host.modelBusy()) {
      return true;
    }
    m_commitRequested = false;
  }
  if (!m_def.inspect && !m_previewOk) {
    qDebug().noquote() << QStringLiteral("%1: OK refused").arg(m_def.name);
    return false;
  }
  // A component in a root with modelled bodies (mitcad#171).
  bool doneByMove = false;
  if (!m_def.inspect && !askBeforeComponent(m_def.build(m_state, m_host), doneByMove)) {
    qDebug().noquote() << QStringLiteral("%1: OK not done").arg(m_def.name);
    return false;
  }
  if (doneByMove) {
    m_host.refreshScene();
    qDebug().noquote() << QStringLiteral("Done %1").arg(m_def.name);
    finish(true);
    return true;
  }
  m_committing = true;
  m_presenter->setOkEnabled(false);
  const auto failed = [this](const CommandHost::AsyncResult& result) {
    recoverFailedCommit(result);
  };
  if (m_def.inspect) {
    if (!m_def.keepsSection || !m_def.build) {
      if (!m_def.keepsSection) {
        m_host.showSection(m_sectionBefore);
      }
      qDebug().noquote() << QStringLiteral("Closed %1").arg(m_def.name);
      finish(true);
      return true;
    }
    const Built built = m_def.build(m_state, m_host);
    if (!built.error.isEmpty()) {
      m_host.showSection(m_sectionBefore);
      finish(true);
      return true;
    }
    const QString name = m_analysis.value(QStringLiteral("name")).toString();
    const QJsonObject command = name.isEmpty()
        ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("add_analysis")},
                      {QStringLiteral("def"), built.def}}
        : QJsonObject{{QStringLiteral("cmd"), QStringLiteral("edit_analysis")},
                      {QStringLiteral("name"), name}, {QStringLiteral("def"), built.def},
                      {QStringLiteral("visible"), true}};
    const int depth = undoDepth();
    commandSequenceAsync(QJsonArray{command}, [this, failed, name, depth](const CommandHost::AsyncResult& result) {
      if (result.succeeded() && m_cancelRequested) {
        undoToDepthAsync(depth, [failed](const CommandHost::AsyncResult&) { failed({{}, QString(), true}); });
        return;
      }
      if (!result.succeeded()) { failed(result); return; }
      qDebug().noquote() << (name.isEmpty() ? QStringLiteral("Kept %1 as %2")
          .arg(m_def.name, result.answer.value(QStringLiteral("name")).toString())
          : QStringLiteral("Edited analysis %1").arg(name));
      finish(true);
    });
    return true;
  }
  if (m_def.modelCommand) {
    const Built built = m_def.build(m_state, m_host);
    const int depth = undoDepth();
    const QString label = built.def.value(QStringLiteral("label")).toString(m_def.name);
    commandSequenceAsync(built.def.value(QStringLiteral("commands")).toArray(),
        [this, failed, depth, label](const CommandHost::AsyncResult& result) {
          if (!result.succeeded() || m_cancelRequested) {
            const CommandHost::AsyncResult stopped = m_cancelRequested && result.succeeded()
                ? CommandHost::AsyncResult{{}, QString(), true} : result;
            undoToDepthAsync(depth, [failed, stopped](const CommandHost::AsyncResult&) { failed(stopped); });
            return;
          }
          const auto done = [this, failed, depth](const CommandHost::AsyncResult& merged) {
            if (!merged.succeeded() || m_cancelRequested) {
              const CommandHost::AsyncResult stopped = m_cancelRequested && merged.succeeded()
                  ? CommandHost::AsyncResult{{}, QString(), true} : merged;
              undoToDepthAsync(depth, [failed, stopped](const CommandHost::AsyncResult& undone) {
                failed(undone.succeeded() ? stopped : undone);
              });
              return;
            }
            if (m_def.describe) { qDebug().noquote() << m_def.describe(m_state, m_host); }
            qDebug().noquote() << QStringLiteral("Done %1").arg(m_def.name);
            m_host.refreshScene();
            finish(true);
          };
          if (undoDepth() > depth + 1) {
            commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("merge_undo")},
                {QStringLiteral("depth"), depth}, {QStringLiteral("label"), label}}}, done);
          } else {
            done({});
          }
        }, 0, [this] { return m_cancelRequested; });
    return true;
  }
  if (m_def.sketchCommand) {
    if (m_appliedDepth < 0) {
      m_committing = false;
      m_commitRequested = true;
      showSketchPreviewAsync(m_def.build(m_state, m_host));
      return true;
    }
    const int depth = m_appliedDepth;
    const auto done = [this, failed](const CommandHost::AsyncResult& result) {
      if (!result.succeeded() || m_cancelRequested) {
        failed(m_cancelRequested && result.succeeded()
            ? CommandHost::AsyncResult{{}, QString(), true} : result);
        return;
      }
      m_appliedDepth = -1;
      if (m_def.describe) { qDebug().noquote() << m_def.describe(m_state, m_host); }
      qDebug().noquote() << QStringLiteral("Added %1").arg(m_def.name);
      finish(true);
    };
    if (undoDepth() > depth + 1) {
      const Built built = m_def.build(m_state, m_host);
      const QString sketch = built.def.value(QStringLiteral("sketch")).toString(
          built.def.value(QStringLiteral("commands")).toArray().first().toObject()
              .value(QStringLiteral("sketch")).toString());
      commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("merge_undo")},
          {QStringLiteral("depth"), depth},
          {QStringLiteral("label"), QStringLiteral("%1 in %2").arg(m_def.name, m_host.featureName(sketch))}}}, done);
    } else {
      done({});
    }
    return true;
  }
  const Built built = m_def.build(m_state, m_host);
  const QString description = m_def.describe && m_def.describeBefore && !isEditing()
      ? m_def.describe(m_state, m_host) : QString();
  rollForwardAsync([this, failed, built, description](const CommandHost::AsyncResult& restored) {
    if (!restored.succeeded() || m_cancelRequested) {
      failed(m_cancelRequested && restored.succeeded()
          ? CommandHost::AsyncResult{{}, QString(), true} : restored);
      return;
    }
    const QJsonObject before = bodyVolumes();
    const int depth = undoDepth();
    const QJsonObject command = isEditing()
        ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("edit_feature")},
                      {QStringLiteral("uid"), m_editUid}, {QStringLiteral("def"), built.def}}
        : addFeatureCommand(built);
    commandSequenceAsync(QJsonArray{command}, [this, failed, before, description, depth](const CommandHost::AsyncResult& result) {
      if (result.succeeded() && m_cancelRequested) {
        undoToDepthAsync(depth, [this, failed](const CommandHost::AsyncResult&) {
          if (isEditing()) {
            commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("set_marker")},
                {QStringLiteral("position"), m_markerPosition}, {QStringLiteral("temporary"), true}}},
                [this, failed](const CommandHost::AsyncResult& rolled) {
                  m_markerMoved = rolled.succeeded();
                  failed({{}, QString(), true});
                });
          } else {
            failed({{}, QString(), true});
          }
        });
        return;
      }
      if (!result.succeeded()) {
        if (isEditing()) {
          commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("set_marker")},
              {QStringLiteral("position"), m_markerPosition}, {QStringLiteral("temporary"), true}}},
              [this, failed, result](const CommandHost::AsyncResult& rolled) {
                m_markerMoved = rolled.succeeded();
                failed(result);
              });
        } else {
          failed(result);
        }
        return;
      }
      if (m_def.describe && !isEditing()) {
        qDebug().noquote() << (m_def.describeBefore ? description : m_def.describe(m_state, m_host));
      }
      qDebug().noquote() << (isEditing() ? QStringLiteral("Edited %1").arg(m_editUid)
          : QStringLiteral("Added %1 as %2").arg(m_def.name, result.answer.value(QStringLiteral("uid")).toString()));
      logVolumes(before);
      finish(true);
    });
  });
  return true;
}

void CommandSession::cancelAsync() {
  if (m_done || m_cancelRequested || m_finalizing) {
    return;
  }
  m_cancelRequested = true;
  m_commitRequested = false;
  ++m_generation;
  m_previewTimer.stop();
  if (m_presenter) { m_presenter->setOkEnabled(false); }
  if (m_host.modelBusy()) {
    m_host.cancelModelJob();
    // Each active operation performs its own compensation before closing.
    if (!m_asyncPending && !m_committing) {
      m_host.whenIdle(this, [this] { finishAsyncCancel(); });
    }
    return;
  }
  finishAsyncCancel();
}

void CommandSession::finishAsyncCancel() {
  if (m_done || m_host.modelBusy()) {
    return;
  }
  if (m_checkpoint.isString()) {
    qDebug().noquote() << QStringLiteral("Command %1 cancelled").arg(m_def.name);
    finish(false);
    return;
  }
  m_committing = false;
  const int depth = m_appliedDepth;
  m_appliedDepth = -1;
  const auto restored = [this, depth](const CommandHost::AsyncResult& undone) {
    if (!undone.succeeded()) {
      // Keep ownership of an applied preview when recovery fails. The
      // user can retry Cancel rather than closing with uncommitted changes.
      m_appliedDepth = depth;
      m_cancelRequested = false;
      m_previewOk = false;
      if (m_presenter) { m_presenter->setMessage(undone.error, true); }
      return;
    }
    rollForwardAsync([this](const CommandHost::AsyncResult& result) {
      if (!result.succeeded()) {
        qDebug().noquote() << QStringLiteral("Command %1: the timeline stays rolled back").arg(m_def.name);
      }
      if (m_def.inspect) { m_host.showSection(m_sectionBefore); }
      qDebug().noquote() << QStringLiteral("Command %1 cancelled").arg(m_def.name);
      finish(false);
    });
  };
  if (depth >= 0) {
    undoToDepthAsync(depth, restored);
  } else {
    restored({});
  }
}

bool CommandSession::commit() {
  if (m_host.asynchronousCommands()) {
    return commitAsync();
  }
  if (m_done) {
    return false;
  }
  if (m_previewTimer.isActive() || m_shownKey.isEmpty()) {
    runPreview();
  }
  if (m_def.inspect) {
    if (m_def.keepsSection && m_def.build) {
      if (!keepAnalysis()) {
        return false;
      }
    } else if (!m_def.keepsSection) {
      m_host.showSection(m_sectionBefore);
    }
    qDebug().noquote() << QStringLiteral("Closed %1").arg(m_def.name);
    finish(true);
    return true;
  }
  if (!m_previewOk) {
    qDebug().noquote() << QStringLiteral("%1: OK refused").arg(m_def.name);
    return false;
  }
  // A component in a root with modelled bodies (mitcad#171).
  bool doneByMove = false;
  if (!askBeforeComponent(m_def.build(m_state, m_host), doneByMove)) {
    qDebug().noquote() << QStringLiteral("%1: OK not done").arg(m_def.name);
    return false;
  }
  if (doneByMove) {
    m_host.refreshScene();
    qDebug().noquote() << QStringLiteral("Done %1").arg(m_def.name);
    finish(true);
    return true;
  }
  if (m_def.modelCommand) {
    const Built built = m_def.build(m_state, m_host);
    const QString label = built.def.value(QStringLiteral("label")).toString(m_def.name);
    if (!m_host.runCommands(built.def.value(QStringLiteral("commands")).toArray(), label)) {
      return false;
    }
    if (m_def.describe) {
      qDebug().noquote() << m_def.describe(m_state, m_host);
    }
    qDebug().noquote() << QStringLiteral("Done %1").arg(m_def.name);
    finish(true);
    return true;
  }
  if (m_def.sketchCommand) {
    if (m_appliedDepth < 0) {
      // Its preview was cancelled: OK applies it now.
      showSketchPreview(m_def.build(m_state, m_host));
      if (m_appliedDepth < 0) {
        qDebug().noquote() << QStringLiteral("%1: OK not done").arg(m_def.name);
        return false;
      }
    }
    // The preview made it already: one undo step for all of it.
    const int depth = m_appliedDepth;
    m_appliedDepth = -1;
    if (undoDepth() > depth + 1) {
      const Built built = m_def.build(m_state, m_host);
      const QString sketch = built.def.value(QStringLiteral("sketch")).toString(
          built.def.value(QStringLiteral("commands")).toArray().first().toObject().value(QStringLiteral("sketch")).toString());
      m_host.runCommand({{QStringLiteral("cmd"), QStringLiteral("merge_undo")},
                         {QStringLiteral("depth"), depth},
                         {QStringLiteral("label"),
                          QStringLiteral("%1 in %2").arg(m_def.name, m_host.featureName(sketch))}});
    }
    if (m_def.describe) {
      qDebug().noquote() << m_def.describe(m_state, m_host);
    }
    qDebug().noquote() << QStringLiteral("Added %1").arg(m_def.name);
    finish(true);
    return true;
  }
  const Built built = m_def.build(m_state, m_host);
  const QString description =
      m_def.describe && m_def.describeBefore && !isEditing() ? m_def.describe(m_state, m_host) : QString();
  if (!rollForward()) {
    qDebug().noquote() << QStringLiteral("%1: OK not done").arg(m_def.name);
    return false;
  }
  const QJsonObject before = bodyVolumes();
  QJsonObject command = addFeatureCommand(built);
  if (isEditing()) {
    command = {{QStringLiteral("cmd"), QStringLiteral("edit_feature")},
               {QStringLiteral("uid"), m_editUid},
               {QStringLiteral("def"), built.def}};
  }
  QJsonObject result;
  if (!m_host.runCommand(command, &result)) {
    if (m_host.lastCancelled()) {
      qDebug().noquote() << QStringLiteral("%1: OK cancelled").arg(m_def.name);
    }
    if (isEditing() && m_host.runCommand({{QStringLiteral("cmd"), QStringLiteral("set_marker")},
                                          {QStringLiteral("position"), m_markerPosition}})) {
      m_markerMoved = true;
    }
    return false;
  }
  if (m_def.describe && !isEditing()) {
    qDebug().noquote() << (m_def.describeBefore ? description : m_def.describe(m_state, m_host));
  }
  qDebug().noquote() << (isEditing()
                             ? QStringLiteral("Edited %1").arg(m_editUid)
                             : QStringLiteral("Added %1 as %2")
                                   .arg(m_def.name, result.value(QStringLiteral("uid")).toString()));
  logVolumes(before);
  finish(true);
  return true;
}

bool CommandSession::askBeforeComponent(const Built& built, bool& done) {
  done = false;
  m_addTo.clear();
  if (isEditing() || m_def.sketchCommand || m_def.inspect || !built.error.isEmpty()) {
    return true;
  }
  ComponentRequest request;
  bool makes = false;
  if (m_def.modelCommand) {
    for (const QJsonValue& value : built.def.value(QStringLiteral("commands")).toArray()) {
      const QJsonObject command = value.toObject();
      const QString name = command.value(QStringLiteral("cmd")).toString();
      if (name == QStringLiteral("create_component")) {
        makes = true;
        request.kind = ComponentRequest::Kind::NewComponent;
      } else if (name == QStringLiteral("components_from_bodies")) {
        makes = true;
        request.kind = ComponentRequest::Kind::FromBodies;
        request.bodies = command.value(QStringLiteral("bodies")).toArray();
      }
    }
  } else {
    makes = built.def.value(QStringLiteral("operation")).toString() == QStringLiteral("new_component") ||
            built.def.value(QStringLiteral("new_component")).toBool();
    request.kind = ComponentRequest::Kind::Feature;
  }
  if (!makes) {
    return true;
  }
  try {
    request.parent = m_host.queryObject({{QStringLiteral("query"), QStringLiteral("components")}})
                         .value(QStringLiteral("active"))
                         .toString();
  } catch (const std::exception&) {
    return true;
  }
  HybridModel model;
  model.query = [this](const QJsonObject& query) { return m_host.queryObject(query); };
  model.run = [this](const QJsonObject& command, QJsonObject* result) { return m_host.runCommand(command, result); };
  const ComponentAnswer answer = askHybridAssembly(m_host.viewer().window(), model, request);
  switch (answer.outcome) {
  case ComponentAnswer::Outcome::Cancel:
    return false;
  case ComponentAnswer::Outcome::Done:
    done = true;
    return true;
  case ComponentAnswer::Outcome::Proceed:
    break;
  }
  if (!answer.moved.isEmpty() && request.kind == ComponentRequest::Kind::Feature) {
    // What the feature uses went into the new component: it goes there too.
    try {
      const QJsonArray components =
          m_host.queryObject({{QStringLiteral("query"), QStringLiteral("feature_references")},
                              {QStringLiteral("def"), built.def}})
              .value(QStringLiteral("components"))
              .toArray();
      if (components.contains(answer.moved)) {
        m_addTo = answer.moved;
        qDebug().noquote() << QStringLiteral("%1 goes into %2 with what it uses").arg(m_def.name, m_addTo);
      }
    } catch (const std::exception&) {
      // Added where the user is, the model says what it misses.
    }
  }
  m_host.refreshScene();
  return true;
}

QJsonObject CommandSession::addFeatureCommand(const Built& built) const {
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("add_feature")}, {QStringLiteral("def"), built.def}};
  if (!m_addTo.isEmpty()) {
    command.insert(QStringLiteral("component"), m_addTo);
  }
  return command;
}

bool CommandSession::keepAnalysis() {
  const Built built = m_def.build(m_state, m_host);
  if (!built.error.isEmpty()) {
    // Nothing to keep (no plane picked): OK closes the panel all the same.
    m_host.showSection(m_sectionBefore);
    return true;
  }
  const QString name = m_analysis.value(QStringLiteral("name")).toString();
  const QJsonObject command =
      name.isEmpty() ? QJsonObject{{QStringLiteral("cmd"), QStringLiteral("add_analysis")},
                                   {QStringLiteral("def"), built.def}}
                     : QJsonObject{{QStringLiteral("cmd"), QStringLiteral("edit_analysis")},
                                   {QStringLiteral("name"), name},
                                   {QStringLiteral("def"), built.def},
                                   {QStringLiteral("visible"), true}};
  QJsonObject result;
  if (!m_host.runCommand(command, &result)) {
    qDebug().noquote() << QStringLiteral("%1: OK not done").arg(m_def.name);
    return false;
  }
  qDebug().noquote() << (name.isEmpty() ? QStringLiteral("Kept %1 as %2")
                                              .arg(m_def.name, result.value(QStringLiteral("name")).toString())
                                        : QStringLiteral("Edited analysis %1").arg(name));
  return true;
}

void CommandSession::cancel() {
  if (m_host.asynchronousCommands()) {
    cancelAsync();
    return;
  }
  if (m_done) {
    return;
  }
  if (!rollForward()) {
    // Its computation was cancelled (or failed): the panel closes all the
    // same, and the roll-back is the last undo step.
    qDebug().noquote() << QStringLiteral("Command %1: the timeline stays rolled back").arg(m_def.name);
  }
  takeBackSketchPreview();
  if (m_def.inspect) {
    m_host.showSection(m_sectionBefore);
  }
  qDebug().noquote() << QStringLiteral("Command %1 cancelled").arg(m_def.name);
  finish(false);
}

void CommandSession::discard() {
  if (m_host.asynchronousCommands() && m_host.modelBusy()) {
    m_host.cancelModelJob();
    m_host.whenIdle(this, [this] { discard(); });
    return;
  }
  if (m_done) {
    return;
  }
  // resetCommandState discards the whole old document immediately after
  // this call; its checkpoint must not launch a job on that dying owner.
  m_checkpoint = QJsonValue();
  m_markerMoved = false;
  m_appliedDepth = -1;
  qDebug().noquote() << QStringLiteral("Command %1 cancelled").arg(m_def.name);
  finish(false);
}

bool CommandSession::focusValue(const QString& key) { return m_presenter != nullptr && m_presenter->focusValue(key); }

// Takes an edit's roll-back away: with undo while it is the last step, so
// that the edit stays one undo step, else by moving the marker back.
bool CommandSession::rollForward() {
  if (!m_markerMoved) {
    return true;
  }
  QString last;
  try {
    last = m_host.queryObject({{QStringLiteral("query"), QStringLiteral("document")}})
               .value(QStringLiteral("undo"))
               .toString();
  } catch (const std::exception&) {
    // Moves the marker back below.
  }
  const bool done = last == QStringLiteral("Move Timeline Marker")
                        ? m_host.runCommand({{QStringLiteral("cmd"), QStringLiteral("undo")}})
                        : m_host.runCommand({{QStringLiteral("cmd"), QStringLiteral("set_marker")},
                                             {QStringLiteral("position"), m_originalMarker}});
  m_markerMoved = !done;
  return done;
}

void CommandSession::finish(bool committed) {
  if (m_done) { return; }
  if (m_host.asynchronousCommands() && m_checkpoint.isString()) {
    if (m_finalizing) { return; }
    m_finalizing = true;
    m_asyncPending = true;
    m_committing = true;
    const auto release = [this, committed](const CommandHost::AsyncResult& restored) {
      if (!restored.succeeded()) {
        m_finalizing = false;
        m_asyncPending = false;
        m_committing = false;
        m_cancelRequested = false;
        if (m_presenter) { m_presenter->setMessage(restored.error, true); }
        else { m_host.showHint(restored.error); }
        return;
      }
      commandSequenceAsync(QJsonArray{QJsonObject{{QStringLiteral("cmd"), QStringLiteral("discard_command")},
                                                  {QStringLiteral("token"), m_checkpoint}}},
          [this, committed](const CommandHost::AsyncResult& result) {
            if (!result.succeeded()) {
              m_finalizing = false;
              m_asyncPending = false;
              m_committing = false;
              m_cancelRequested = false;
              if (m_presenter) { m_presenter->setMessage(result.error, true); }
              else { m_host.showHint(result.error); }
              return;
            }
            m_checkpoint = QJsonValue();
            m_finalizing = false;
            m_asyncPending = false;
            m_committing = false;
            if (!committed && m_def.inspect) { m_host.showSection(m_sectionBefore); }
            finish(committed);
          });
    };
    if (committed) { release({}); }
    else { restoreCheckpointAsync(release); }
    return;
  }
  m_done = true;
  m_previewTimer.stop();
  m_host.clearModelPreview();
  if (m_manipulators) {
    m_manipulators->hide();
    m_manipulators->deleteLater();
  }
  OcctViewer& viewer = m_host.viewer();
  viewer.setPreview(TopoDS_Shape());
  viewer.setHighlights({});
  viewer.setGhosts({});
  m_host.inputActivated(SelectFilter());
  emit finished(committed);
}

} // namespace mitcad
