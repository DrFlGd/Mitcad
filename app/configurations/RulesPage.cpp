// SPDX-License-Identifier: MIT
#include "RulesPage.hpp"

#include <optional>
#include <utility>

#include <QAbstractItemView>
#include <QApplication>
#include <QDebug>
#include <QDialog>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QKeyEvent>
#include <QKeySequence>
#include <QLabel>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSplitter>
#include <QStyle>
#include <QTimer>
#include <QTreeWidget>
#include <QVBoxLayout>

#include "../framework/FormulaEditor.hpp"
#include "../framework/ParameterSuggest.hpp"
#include "../framework/Theme.hpp"
#include "RuleCanvas.hpp"

namespace {

// Whether a list's item editor is open: the focus is on a widget inside the
// list's viewport (QAbstractItemView::state() is protected).
bool editingIn(const QAbstractItemView* view) {
  const QWidget* focus = QApplication::focusWidget();
  return focus != nullptr && view->viewport()->isAncestorOf(focus);
}

} // namespace

namespace mitcad::rules {

namespace {

QStringList strings(const QJsonValue& value) {
  QStringList out;
  const QJsonArray array = value.toArray();
  for (const QJsonValue& item : array) {
    if (item.isString()) {
      out << item.toString();
    }
  }
  return out;
}

QWidget* labelled(QLabel* label, QWidget* buddy) {
  label->setBuddy(buddy);
  return label;
}

} // namespace

RulesPage::RulesPage(RulesSource source, QWidget* parent) : QWidget(parent), m_source(std::move(source)) {
  setObjectName(QStringLiteral("rulesPage"));
  m_list = new QTreeWidget;
  m_list->setObjectName(QStringLiteral("rulesList"));
  m_list->setColumnCount(2);
  m_list->setHeaderLabels({QStringLiteral("Rule"), QString()});
  m_list->setRootIsDecorated(false);
  m_list->setUniformRowHeights(true);
  m_list->header()->setStretchLastSection(false);
  m_list->header()->setSectionResizeMode(0, QHeaderView::Stretch);
  m_list->header()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
  m_list->setEditTriggers(QAbstractItemView::DoubleClicked | QAbstractItemView::EditKeyPressed);
  m_list->installEventFilter(this);
  m_add = new QPushButton(QStringLiteral("&Add"));
  m_duplicate = new QPushButton(QStringLiteral("D&uplicate"));
  m_delete = new QPushButton(QStringLiteral("&Delete"));
  for (QPushButton* button : {m_add, m_duplicate, m_delete}) {
    button->setAutoDefault(false);
  }
  auto* left = new QWidget;
  auto* leftLayout = new QVBoxLayout(left);
  leftLayout->setContentsMargins(0, 0, 0, 0);
  leftLayout->addWidget(labelled(new QLabel(QStringLiteral("&Rules")), m_list));
  leftLayout->addWidget(m_list, 1);
  auto* buttons = new QHBoxLayout;
  buttons->addWidget(m_add);
  buttons->addWidget(m_duplicate);
  buttons->addWidget(m_delete);
  buttons->addStretch(1);
  leftLayout->addLayout(buttons);

  m_palette = new RulePalette;
  m_canvas = new RuleCanvas;
  m_palette->setCanvas(m_canvas);
  m_canvas->setDeleteArea(m_palette);
  m_palette->installEventFilter(this);
  m_canvas->installEventFilter(this);
  m_viewText = new QPushButton(QStringLiteral("View as &Text"));
  m_viewText->setCheckable(true);
  m_viewText->setAutoDefault(false);
  m_check = new QPushButton(QStringLiteral("Chec&k Rules"));
  m_check->setAutoDefault(false);
  m_status = new QLabel;
  m_status->setObjectName(QStringLiteral("rulesStatus"));
  m_status->setWordWrap(true);
  m_status->hide();
  m_text = new QPlainTextEdit;
  m_text->setObjectName(QStringLiteral("ruleText"));
  m_text->setReadOnly(true);
  m_text->setMaximumHeight(110);
  m_text->hide();
  m_report = new QPlainTextEdit;
  m_report->setObjectName(QStringLiteral("rulesReport"));
  m_report->setReadOnly(true);
  m_report->setMaximumHeight(140);
  m_report->hide();

  auto* paletteColumn = new QWidget;
  auto* paletteLayout = new QVBoxLayout(paletteColumn);
  paletteLayout->setContentsMargins(0, 0, 0, 0);
  paletteLayout->addWidget(labelled(new QLabel(QStringLiteral("&Palette")), m_palette));
  paletteLayout->addWidget(m_palette, 1);
  auto* canvasColumn = new QWidget;
  auto* canvasLayout = new QVBoxLayout(canvasColumn);
  canvasLayout->setContentsMargins(0, 0, 0, 0);
  auto* tools = new QHBoxLayout;
  tools->addWidget(labelled(new QLabel(QStringLiteral("&Blocks")), m_canvas));
  tools->addStretch(1);
  tools->addWidget(m_viewText);
  tools->addWidget(m_check);
  canvasLayout->addLayout(tools);
  canvasLayout->addWidget(m_canvas, 1);
  auto* editor = new QSplitter(Qt::Horizontal);
  editor->addWidget(paletteColumn);
  editor->addWidget(canvasColumn);
  editor->setStretchFactor(0, 0);
  editor->setStretchFactor(1, 1);
  editor->setSizes({170, 480});
  auto* right = new QWidget;
  auto* rightLayout = new QVBoxLayout(right);
  rightLayout->setContentsMargins(0, 0, 0, 0);
  rightLayout->addWidget(editor, 1);
  rightLayout->addWidget(m_status);
  rightLayout->addWidget(m_text);
  rightLayout->addWidget(m_report);

  auto* split = new QSplitter(Qt::Horizontal);
  split->addWidget(left);
  split->addWidget(right);
  split->setStretchFactor(0, 0);
  split->setStretchFactor(1, 1);
  split->setSizes({200, 660});
  auto* layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->addWidget(split);

  if (m_source.parameters.parameters) {
    m_canvas->setFormulaEditor([this](const QString& text, QWidget* owner) -> std::optional<QString> {
      ParameterFieldOptions options;
      options.anyKind = true;
      options.kind = ParameterKind::Unitless;
      options.create = false;
      options.label = QStringLiteral("rule condition");
      FormulaEditor dialog(m_source.parameters, options, text, owner);
      if (dialog.exec() != QDialog::Accepted) {
        return std::nullopt;
      }
      return dialog.expression();
    });
  }

  connect(m_add, &QPushButton::clicked, this, [this] { addRule(); });
  connect(m_duplicate, &QPushButton::clicked, this, [this] { duplicateRule(); });
  connect(m_delete, &QPushButton::clicked, this, [this] { deleteRule(); });
  connect(m_check, &QPushButton::clicked, this, [this] { checkRules(); });
  connect(m_viewText, &QPushButton::toggled, this, [this](bool shown) { setTextShown(shown); });
  connect(m_list, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* item) {
    if (m_building) {
      return;
    }
    m_current = item != nullptr ? m_list->indexOfTopLevelItem(item) : -1;
    showCurrent();
  });
  connect(m_list, &QTreeWidget::itemChanged, this, [this](QTreeWidgetItem* item) {
    if (m_building || item == nullptr) {
      return;
    }
    const int index = m_list->indexOfTopLevelItem(item);
    // After the list's own handling of the edit: the list is filled anew.
    QTimer::singleShot(0, this, [this, index] { listEdited(index); });
  });
  connect(m_canvas, &RuleCanvas::edited, this, [this](const Rule& rule, const QString& what) {
    if (m_current < 0 || m_current >= static_cast<int>(m_rules.size())) {
      return;
    }
    std::vector<Rule> changed = m_rules;
    changed[static_cast<std::size_t>(m_current)] = rule;
    commit(std::move(changed), what);
  });
  connect(m_canvas, &RuleCanvas::paletteRequested, this, [this] {
    if (m_palette->currentItem() == nullptr || m_palette->currentItem()->childCount() > 0) {
      m_palette->setCurrentItem(m_palette->entry(QStringLiteral("compare")));
    }
    m_palette->setFocus(Qt::OtherFocusReason);
    qDebug().noquote() << QStringLiteral("Rule palette chooses for %1").arg(m_canvas->focusName());
  });
  refresh();
}

void RulesPage::refresh() {
  const QString current =
      m_current >= 0 && m_current < static_cast<int>(m_rules.size()) ? m_rules[static_cast<std::size_t>(m_current)].name
                                                                       : QString();
  QJsonObject answer;
  if (m_source.configurations) {
    answer = m_source.configurations();
  }
  m_table = answer.value(QStringLiteral("configurations")).toObject();
  m_problems = strings(answer.value(QStringLiteral("problems")));
  const QStringList options = strings(m_table.value(QStringLiteral("options")));
  QStringList numbers;
  if (m_source.configure) {
    const QJsonArray reports = m_source.configure().value(QStringLiteral("options")).toArray();
    for (const QJsonValue& report : reports) {
      const QJsonObject option = report.toObject();
      if (option.value(QStringLiteral("kind")).toString() == QLatin1String("number")) {
        numbers << option.value(QStringLiteral("name")).toString();
      }
    }
  }
  m_canvas->setOptions(options, numbers);
  QStringList parameters;
  if (m_source.parameters.parameters) {
    try {
      const QJsonArray all = m_source.parameters.parameters();
      for (const QJsonValue& parameter : all) {
        const QString name = parameter.toObject().value(QStringLiteral("name")).toString();
        if (!name.isEmpty()) {
          parameters << name;
        }
      }
    } catch (...) {
      // The model computes now: the names come with the next refresh.
    }
  }
  m_canvas->setParameters(parameters);
  if (m_source.tables) {
    m_canvas->setTables(suggestedTables(m_source.tables()));
  }
  const QJsonArray rules = m_table.value(QStringLiteral("rules")).toArray();
  // Nothing to show anew (the window refreshes the page after every change
  // of the design): an open editor of a name or a block stays open.
  if (!m_drafts.empty() || (rules == m_shownRules && m_problems == m_shownProblems)) {
    m_shownProblems = m_problems;
    if (!editingIn(m_list) && m_canvas->editor() == nullptr) {
      fillList();
    }
    return;
  }
  m_shownRules = rules;
  m_shownProblems = m_problems;
  m_rules.clear();
  for (const QJsonValue& rule : rules) {
    m_rules.push_back(ruleFromJson(rule.toObject()));
  }
  m_refusal.clear();
  m_current = -1;
  for (std::size_t i = 0; i < m_rules.size(); ++i) {
    if (m_rules[i].name == current) {
      m_current = static_cast<int>(i);
    }
  }
  if (m_current < 0 && !m_rules.empty()) {
    m_current = 0;
  }
  fillList();
  showCurrent();
}

QString RulesPage::problemOf(int index) const {
  if (index < 0 || index >= static_cast<int>(m_rules.size())) {
    return {};
  }
  const Rule& rule = m_rules[static_cast<std::size_t>(index)];
  if (!isComplete(rule)) {
    if (rule.name.trimmed().isEmpty()) {
      return QStringLiteral("Not saved: the rule has no name");
    }
    if (rule.hasElse && rule.otherwise.empty()) {
      return QStringLiteral("Not saved: its else has no action");
    }
    return QStringLiteral("Not saved: fill its empty sockets");
  }
  if (!m_drafts.empty() && !m_refusal.isEmpty() && (m_refusal.contains(rule.name) || index == m_current)) {
    return QStringLiteral("Not saved: %1").arg(m_refusal);
  }
  QStringList problems;
  for (const QString& problem : m_problems) {
    if (problem.contains(rule.name)) {
      problems << problem;
    }
  }
  return problems.join(QStringLiteral("; "));
}

void RulesPage::fillList() {
  m_building = true;
  m_list->clear();
  QStringList rows;
  for (std::size_t i = 0; i < m_rules.size(); ++i) {
    const Rule& rule = m_rules[i];
    auto* item = new QTreeWidgetItem(m_list, QStringList{rule.name});
    item->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable | Qt::ItemIsEditable | Qt::ItemIsUserCheckable);
    item->setCheckState(0, rule.enabled ? Qt::Checked : Qt::Unchecked);
    const QString comment = rule.extra.value(QStringLiteral("comment")).toString();
    const QString problem = problemOf(static_cast<int>(i));
    if (!problem.isEmpty()) {
      item->setIcon(1, style()->standardIcon(QStyle::SP_MessageBoxWarning));
      item->setToolTip(0, problem);
      item->setToolTip(1, problem);
    } else if (!comment.isEmpty()) {
      item->setToolTip(0, comment);
    }
    rows << QStringLiteral("Rules row %1: %2, %3")
                .arg(rule.name, rule.enabled ? QStringLiteral("enabled") : QStringLiteral("disabled"),
                     problem.isEmpty() ? QStringLiteral("ok") : problem);
  }
  if (m_current >= 0 && m_current < m_list->topLevelItemCount()) {
    m_list->setCurrentItem(m_list->topLevelItem(m_current));
  }
  m_building = false;
  const QString joined = rows.join(QLatin1Char('\n'));
  if (joined != m_loggedRows) {
    m_loggedRows = joined;
    qDebug().noquote() << QStringLiteral("Rules: %1").arg(m_rules.size());
    for (const QString& row : rows) {
      qDebug().noquote() << row;
    }
  }
  m_duplicate->setEnabled(m_current >= 0);
  m_delete->setEnabled(m_current >= 0);
  updateStatus();
  QTimer::singleShot(0, this, [this] { logPlaces(); });
}

void RulesPage::showCurrent() {
  if (m_current >= 0 && m_current < static_cast<int>(m_rules.size())) {
    m_canvas->setRule(m_rules[static_cast<std::size_t>(m_current)]);
  } else {
    m_canvas->clearRule();
  }
  m_duplicate->setEnabled(m_current >= 0);
  m_delete->setEnabled(m_current >= 0);
  updateText();
}

void RulesPage::updateStatus() {
  QStringList waiting;
  for (std::size_t i = 0; i < m_rules.size(); ++i) {
    if (!isComplete(m_rules[i])) {
      waiting << m_rules[i].name;
    }
  }
  QString text;
  if (!waiting.isEmpty()) {
    text = QStringLiteral("The rules are saved once every rule is complete: %1.").arg(waiting.join(QStringLiteral(", ")));
  } else if (!m_drafts.empty() && !m_refusal.isEmpty()) {
    text = QStringLiteral("Not saved: %1").arg(m_refusal);
  }
  m_status->setText(text);
  m_status->setVisible(!text.isEmpty());
  if (!m_drafts.empty() && !m_refusal.isEmpty()) {
    setErrorStyleSheet(m_status);
  } else {
    setWarningStyleSheet(m_status);
  }
}

void RulesPage::updateText() {
  if (!m_text->isVisible() && !m_viewText->isChecked()) {
    return;
  }
  QStringList lines;
  QString name;
  if (m_current >= 0 && m_current < static_cast<int>(m_rules.size())) {
    const Rule& rule = m_rules[static_cast<std::size_t>(m_current)];
    lines = ruleText(rule);
    name = rule.name;
  }
  m_text->setPlainText(lines.join(QLatin1Char('\n')));
  const QString logged = QStringLiteral("Rule as text %1: %2").arg(name, lines.join(QStringLiteral(" / ")));
  if (logged != m_loggedText) {
    m_loggedText = logged;
    qDebug().noquote() << logged;
  }
}

void RulesPage::setTextShown(bool shown) {
  if (m_viewText->isChecked() != shown) {
    m_viewText->setChecked(shown); // toggled() comes back here
    return;
  }
  m_text->setVisible(shown);
  if (shown) {
    m_loggedText.clear();
    updateText();
  }
}

QString RulesPage::uniqueName(const QString& base, int first) const {
  const auto taken = [this](const QString& name) {
    for (const Rule& rule : m_rules) {
      if (rule.name == name) {
        return true;
      }
    }
    return false;
  };
  for (int n = first;; ++n) {
    const QString name = QStringLiteral("%1 %2").arg(base).arg(n);
    if (!taken(name)) {
      return name;
    }
  }
}

void RulesPage::addRule() {
  std::vector<Rule> changed = m_rules;
  Rule rule;
  rule.name = uniqueName(QStringLiteral("Rule"));
  changed.push_back(rule);
  m_current = static_cast<int>(changed.size()) - 1;
  const QString name = rule.name;
  commit(std::move(changed), QStringLiteral("Add rule %1").arg(name));
  selectRule(name);
  // Its name, to be typed over at once.
  if (QTreeWidgetItem* item = m_list->currentItem()) {
    m_list->setFocus(Qt::OtherFocusReason);
    m_list->editItem(item, 0);
    qDebug().noquote() << QStringLiteral("Rules naming %1").arg(name);
  }
}

void RulesPage::duplicateRule() {
  if (m_current < 0 || m_current >= static_cast<int>(m_rules.size())) {
    return;
  }
  std::vector<Rule> changed = m_rules;
  Rule copy = changed[static_cast<std::size_t>(m_current)];
  copy.name = uniqueName(copy.name, 2);
  const QString name = copy.name;
  changed.insert(changed.begin() + m_current + 1, copy);
  m_current += 1;
  commit(std::move(changed), QStringLiteral("Duplicate rule as %1").arg(name));
  selectRule(name);
}

void RulesPage::deleteRule() {
  if (m_current < 0 || m_current >= static_cast<int>(m_rules.size())) {
    return;
  }
  std::vector<Rule> changed = m_rules;
  const QString name = changed[static_cast<std::size_t>(m_current)].name;
  changed.erase(changed.begin() + m_current);
  if (m_current >= static_cast<int>(changed.size())) {
    m_current = static_cast<int>(changed.size()) - 1;
  }
  commit(std::move(changed), QStringLiteral("Delete rule %1").arg(name));
}

bool RulesPage::selectRule(const QString& name) {
  for (std::size_t i = 0; i < m_rules.size(); ++i) {
    if (m_rules[i].name == name) {
      m_current = static_cast<int>(i);
      m_building = true;
      m_list->setCurrentItem(m_list->topLevelItem(m_current));
      m_building = false;
      showCurrent();
      return true;
    }
  }
  return false;
}

void RulesPage::listEdited(int index) {
  if (index < 0 || index >= static_cast<int>(m_rules.size()) || index >= m_list->topLevelItemCount()) {
    return;
  }
  const QTreeWidgetItem* item = m_list->topLevelItem(index);
  const QString name = item->text(0).trimmed();
  const bool enabled = item->checkState(0) == Qt::Checked;
  const Rule& rule = m_rules[static_cast<std::size_t>(index)];
  if (name == rule.name && enabled == rule.enabled) {
    return;
  }
  if (name != rule.name) {
    QString refused;
    if (name.isEmpty()) {
      refused = QStringLiteral("A rule needs a name.");
    }
    for (std::size_t i = 0; i < m_rules.size() && refused.isEmpty(); ++i) {
      if (static_cast<int>(i) != index && m_rules[i].name == name) {
        refused = QStringLiteral("Another rule is named %1.").arg(name);
      }
    }
    if (!refused.isEmpty()) {
      qDebug().noquote() << QStringLiteral("Rules rename refused: %1").arg(refused);
      fillList();
      m_status->setText(refused);
      setErrorStyleSheet(m_status);
      m_status->show();
      return;
    }
  }
  std::vector<Rule> changed = m_rules;
  Rule& edited = changed[static_cast<std::size_t>(index)];
  const QString what = name != edited.name ? QStringLiteral("Rename rule %1 to %2").arg(edited.name, name)
                       : enabled          ? QStringLiteral("Enable rule %1").arg(name)
                                          : QStringLiteral("Disable rule %1").arg(name);
  edited.name = name;
  if (enabled != edited.enabled) {
    edited.enabled = enabled;
    edited.enabledWritten = edited.enabledWritten || !enabled;
  }
  m_current = index;
  commit(std::move(changed), what);
}

void RulesPage::commit(std::vector<Rule> rules, const QString& what) {
  std::vector<Rule> previous = std::move(m_rules);
  m_rules = std::move(rules);
  bool complete = true;
  QJsonArray array;
  for (const Rule& rule : m_rules) {
    bool done = false;
    array.append(ruleToJson(rule, &done));
    complete = complete && done;
  }
  if (complete && m_source.setRules) {
    QString error;
    if (m_source.setRules(array, &error)) {
      m_drafts.clear();
      m_refusal.clear();
      qDebug().noquote() << QStringLiteral("Rules saved: %1").arg(what);
      refresh();
      emit saved();
      return;
    }
    m_refusal = error.isEmpty() ? QStringLiteral("the design refused the rules") : error;
    qDebug().noquote() << QStringLiteral("Rules refused: %1: %2").arg(what, m_refusal);
  } else {
    m_refusal.clear();
    qDebug().noquote() << QStringLiteral("Rules not saved yet: %1").arg(what);
  }
  m_drafts.push_back(std::move(previous));
  fillList();
  showCurrent();
}

bool RulesPage::undo() {
  if (!m_drafts.empty()) {
    m_rules = std::move(m_drafts.back());
    m_drafts.pop_back();
    m_refusal.clear();
    qDebug().noquote() << QStringLiteral("Rules undo: the last change not saved");
    if (m_current >= static_cast<int>(m_rules.size())) {
      m_current = static_cast<int>(m_rules.size()) - 1;
    }
    fillList();
    showCurrent();
    return true;
  }
  if (m_source.undo) {
    qDebug().noquote() << QStringLiteral("Rules undo: the design's last step");
    m_source.undo();
    refresh();
    return true;
  }
  return false;
}

void RulesPage::checkRules() {
  if (!m_source.checkRules) {
    return;
  }
  const QJsonObject report = m_source.checkRules();
  QStringList lines;
  if (!m_drafts.empty()) {
    lines << QStringLiteral("The saved rules were checked; the changes not saved yet were not.");
  }
  if (report.contains(QStringLiteral("error"))) {
    lines << QStringLiteral("Check Rules failed: %1").arg(report.value(QStringLiteral("error")).toString());
  } else {
    QStringList found = strings(report.value(QStringLiteral("lines")));
    if (found.isEmpty()) {
      const QJsonArray never = report.value(QStringLiteral("never_allowed")).toArray();
      for (const QJsonValue& value : never) {
        const QJsonObject entry = value.toObject();
        found << QStringLiteral("%1 %2 is never allowed")
                     .arg(entry.value(QStringLiteral("option")).toString(), entry.value(QStringLiteral("text")).toString());
      }
      for (const QString& rule : strings(report.value(QStringLiteral("never_applies")))) {
        found << QStringLiteral("The rule %1 never applies").arg(rule);
      }
      const QJsonArray rows = report.value(QStringLiteral("rows")).toArray();
      for (const QJsonValue& value : rows) {
        const QJsonObject entry = value.toObject();
        found << QStringLiteral("The row %1 breaks the rule %2: %3")
                     .arg(entry.value(QStringLiteral("row")).toString(), entry.value(QStringLiteral("rule")).toString(),
                          entry.value(QStringLiteral("message")).toString());
      }
      if (found.isEmpty()) {
        found << QStringLiteral("No problems found.");
      }
    }
    lines << found;
  }
  m_report->setPlainText(lines.join(QLatin1Char('\n')));
  m_report->show();
  for (const QString& line : lines) {
    qDebug().noquote() << QStringLiteral("Check Rules: %1").arg(line);
  }
  const QJsonArray never = report.value(QStringLiteral("never_allowed")).toArray();
  for (const QJsonValue& value : never) {
    const QJsonObject entry = value.toObject();
    qDebug().noquote() << QStringLiteral("Check Rules never allowed: %1 %2")
                              .arg(entry.value(QStringLiteral("option")).toString(),
                                   entry.value(QStringLiteral("text")).toString());
  }
}

bool RulesPage::eventFilter(QObject* watched, QEvent* event) {
  if (event->type() == QEvent::ShortcutOverride || event->type() == QEvent::KeyPress) {
    auto* key = static_cast<QKeyEvent*>(event);
    const bool undoKey = key->matches(QKeySequence::Undo);
    const bool redoKey = key->matches(QKeySequence::Redo);
    if ((watched == m_canvas || watched == m_palette || watched == m_list) && (undoKey || redoKey) &&
        m_canvas->editor() == nullptr) {
      const bool takes = undoKey ? (!m_drafts.empty() || static_cast<bool>(m_source.undo))
                                 : (m_drafts.empty() && static_cast<bool>(m_source.redo));
      if (takes) {
        event->accept();
        if (event->type() == QEvent::KeyPress) {
          if (undoKey) {
            undo();
          } else {
            m_source.redo();
            refresh();
          }
        }
        return event->type() == QEvent::KeyPress;
      }
    }
    if (watched == m_list && event->type() == QEvent::KeyPress && !editingIn(m_list) &&
        key->modifiers() == Qt::NoModifier) {
      if (key->key() == Qt::Key_Insert) {
        addRule();
        return true;
      }
      if (key->key() == Qt::Key_Delete) {
        deleteRule();
        return true;
      }
    }
  }
  return QWidget::eventFilter(watched, event);
}

void RulesPage::showEvent(QShowEvent* event) {
  QWidget::showEvent(event);
  QTimer::singleShot(0, this, [this] {
    logPlaces();
    m_palette->logPlaces();
    m_canvas->logPlaces();
  });
}

void RulesPage::resizeEvent(QResizeEvent* event) {
  QWidget::resizeEvent(event);
  logPlaces();
}

void RulesPage::logPlaces() {
  if (!isVisible()) {
    return;
  }
  QWidget* top = window();
  if (top->parentWidget() != nullptr) {
    top = top->parentWidget()->window();
  }
  const QPoint origin = top->mapToGlobal(QPoint(0, 0));
  QStringList lines;
  const std::pair<const char*, QWidget*> widgets[] = {
      {"add", m_add}, {"duplicate", m_duplicate}, {"delete", m_delete}, {"text", m_viewText}, {"check", m_check}};
  for (const auto& [name, widget] : widgets) {
    const QPoint at = widget->mapToGlobal(widget->rect().center()) - origin;
    lines << QStringLiteral("Rules %1 at %2,%3").arg(QLatin1String(name)).arg(at.x()).arg(at.y());
  }
  for (int i = 0; i < m_list->topLevelItemCount(); ++i) {
    QTreeWidgetItem* item = m_list->topLevelItem(i);
    const QRect rect = m_list->visualItemRect(item);
    if (!m_list->viewport()->rect().contains(rect.center())) {
      lines << QStringLiteral("Rules row %1 out of sight").arg(item->text(0));
      continue;
    }
    const QPoint at = m_list->viewport()->mapToGlobal(QPoint(rect.left() + 40, rect.center().y())) - origin;
    lines << QStringLiteral("Rules row %1 at %2,%3").arg(item->text(0)).arg(at.x()).arg(at.y());
  }
  const QString joined = lines.join(QLatin1Char('\n'));
  if (joined == m_loggedPlaces) {
    return;
  }
  m_loggedPlaces = joined;
  for (const QString& line : lines) {
    qDebug().noquote() << line;
  }
}

} // namespace mitcad::rules
