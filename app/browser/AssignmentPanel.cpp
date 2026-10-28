// SPDX-License-Identifier: MIT
#include "AssignmentPanel.hpp"

#include <algorithm>
#include <exception>
#include <utility>

#include <QAbstractItemView>
#include <QComboBox>
#include <QFont>
#include <QFrame>
#include <QHBoxLayout>
#include <QHash>
#include <QHeaderView>
#include <QItemSelectionModel>
#include <QJsonValue>
#include <QLabel>
#include <QPixmap>
#include <QPushButton>
#include <QSignalBlocker>
#include <QStyle>
#include <QTableWidget>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/Command.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/FormulaEditor.hpp"
#include "../framework/ParameterField.hpp"
#include "../framework/TestSync.hpp"
#include "../framework/Theme.hpp"
#include "Bindings.hpp"

namespace mitcad {
namespace {

// The entries of Varies with that are no option, and the key of the Other
// row (rowKey): no parameter's name starts with '#'.
constexpr const char kCustomEntry[] = "#custom";
constexpr const char kAddEntry[] = "#add";
constexpr const char kOtherKey[] = "#other";

// The table's columns: the value ("Black (now)" for the design's value
// now) and the result.
constexpr int kValueColumn = 0;
constexpr int kResultColumn = 1;
// Rows in sight before the table scrolls.
constexpr int kRowsInSight = 6;
constexpr int kSwatch = 18;

// Two values of an option as expressions: the same text, the same text
// literal, or the same number written with other spaces.
bool sameValue(const QString& a, const QString& b) {
  const QString x = a.trimmed();
  const QString y = b.trimmed();
  if (x == y) {
    return true;
  }
  const auto quoted = [](const QString& text) {
    return text.startsWith(QLatin1Char('\'')) || text.startsWith(QLatin1Char('"'));
  };
  if (quoted(x) && quoted(y)) {
    return unquotedText(x) == unquotedText(y);
  }
  QString left = x;
  QString right = y;
  return left.remove(QLatin1Char(' ')) == right.remove(QLatin1Char(' '));
}

} // namespace

AssignmentPanel::AssignmentPanel(Kind kind, const QString& logName, Host host, QWidget* parent)
    : QWidget(parent), m_kind(kind), m_log(logName), m_host(std::move(host)) {
  setObjectName(QStringLiteral("assignmentPanel"));
  auto* layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(4);

  // The header line: the title naming the targets, what they have now,
  // Varies with and the fx mark. The windows grow by this line (their lists
  // keep their height).
  auto* header = new QHBoxLayout;
  m_title = new QLabel;
  QFont bold = m_title->font();
  bold.setBold(true);
  m_title->setFont(bold);
  header->addWidget(m_title);
  header->addSpacing(12);
  m_nowLabel = new QLabel(tr("Now:"));
  header->addWidget(m_nowLabel);
  m_nowSwatch = new QLabel;
  header->addWidget(m_nowSwatch);
  m_now = new QLabel;
  m_now->setToolTip(m_kind == Kind::Material ? tr("The material the targets have now")
                                             : tr("The appearance the targets have now"));
  header->addWidget(m_now);
  header->addStretch(1);
  auto* variesLabel = new QLabel(tr("Varies with"));
  header->addWidget(variesLabel);
  m_varies = new QComboBox;
  m_varies->setObjectName(QStringLiteral("assignmentVaries"));
  m_varies->setToolTip(m_kind == Kind::Material
                           ? tr("None: one material. An option: a material for each of its values")
                           : tr("None: one appearance. An option: an appearance for each of its values"));
  m_varies->setSizeAdjustPolicy(QComboBox::AdjustToContents);
  variesLabel->setBuddy(m_varies);
  header->addWidget(m_varies);
  m_fx = new QLabel(QStringLiteral("fx"));
  QFont fx = m_fx->font();
  fx.setBold(true);
  fx.setItalic(true);
  m_fx->setFont(fx);
  m_fx->setStyleSheet(QStringLiteral("color: #1f5fc0;"));
  m_fx->setToolTip(tr("The targets follow an expression of parameters"));
  header->addWidget(m_fx);
  layout->addLayout(header);

  // Targets in different states (mitcad#861): a tinted notice with a
  // warning sign, hard to miss.
  auto* mixedBox = new QFrame;
  mixedBox->setObjectName(QStringLiteral("assignmentMixed"));
  mixedBox->setStyleSheet(QStringLiteral("QFrame#assignmentMixed { background: rgba(255, 193, 7, 56); "
                                         "border: 1px solid rgba(214, 150, 0, 170); border-radius: 4px; }"));
  auto* mixedRow = new QHBoxLayout(mixedBox);
  mixedRow->setContentsMargins(6, 4, 6, 4);
  auto* warning = new QLabel;
  warning->setPixmap(style()->standardIcon(QStyle::SP_MessageBoxWarning).pixmap(16, 16));
  warning->setAlignment(Qt::AlignTop);
  mixedRow->addWidget(warning);
  m_mixed = new QLabel;
  m_mixed->setWordWrap(true);
  mixedRow->addWidget(m_mixed, 1);
  m_mixedBox = mixedBox;
  layout->addWidget(m_mixedBox);

  // The option's values and their results.
  m_table = new QTableWidget(0, 2);
  m_table->setObjectName(QStringLiteral("assignmentRows"));
  m_table->setHorizontalHeaderLabels({tr("Value"), m_kind == Kind::Material ? tr("Material") : tr("Appearance")});
  m_table->verticalHeader()->setVisible(false);
  m_table->setSelectionBehavior(QAbstractItemView::SelectRows);
  m_table->setSelectionMode(QAbstractItemView::ExtendedSelection);
  m_table->setEditTriggers(QAbstractItemView::NoEditTriggers);
  m_table->setShowGrid(false);
  m_table->setWordWrap(false);
  m_table->setIconSize(QSize(kSwatch, kSwatch));
  m_table->horizontalHeader()->setSectionResizeMode(kValueColumn, QHeaderView::ResizeToContents);
  m_table->horizontalHeader()->setSectionResizeMode(kResultColumn, QHeaderView::Stretch);
  m_table->horizontalHeader()->setStretchLastSection(true);
  m_table->setToolTip(tr("Click a row, then double-click an item of the list or press Assign (Shift, Ctrl: several rows)."));
  layout->addWidget(m_table);

  // Under the table: an expression of another form with Replace with a
  // table, and the Formula Editor at the right edge.
  auto* below = new QHBoxLayout;
  m_customText = new QLabel;
  m_customText->setWordWrap(true);
  m_customText->setTextInteractionFlags(Qt::TextSelectableByMouse);
  below->addWidget(m_customText, 1);
  below->addStretch(0); // the buttons at the right edge while there is no text
  m_replace = new QPushButton(tr("Replace with a table"));
  m_replace->setAutoDefault(false);
  m_replace->setToolTip(tr("A row per value of an option instead; nothing changes until a row does"));
  below->addWidget(m_replace);
  m_formula = new QPushButton(tr("Formula Editor…"));
  m_formula->setAutoDefault(false);
  m_formula->setToolTip(tr("Edit the expression as text"));
  below->addWidget(m_formula);
  layout->addLayout(below);

  m_message = new QLabel;
  m_message->setWordWrap(true);
  setErrorStyleSheet(m_message);
  layout->addWidget(m_message);

  connect(m_varies, &QComboBox::activated, this, &AssignmentPanel::optionChosen);
  connect(m_table, &QTableWidget::currentCellChanged, this, [this](int row) {
    if (m_building || row < 0 || row >= m_rows.size()) {
      return;
    }
    m_activeKey = rowKey(row);
    m_noActive = false;
    m_message->clear();
    m_message->hide();
    qDebug().noquote() << QStringLiteral("%1 active row: %2").arg(m_log, m_rows[row].shown);
    emit assignTextChanged();
  });
  connect(m_table, &QTableWidget::itemSelectionChanged, this, [this] {
    if (m_building) {
      return;
    }
    // Rows the user selected take the next pick again.
    if (!m_table->selectionModel()->selectedRows().isEmpty()) {
      m_noActive = false;
    }
    emit assignTextChanged();
  });
  connect(m_formula, &QPushButton::clicked, this, &AssignmentPanel::openFormulaEditor);
  connect(m_replace, &QPushButton::clicked, this, &AssignmentPanel::replaceWithTable);
  setVisible(false);
}

AssignmentPanel::~AssignmentPanel() = default;

void AssignmentPanel::setTargets(const QStringList& bodies, const QVector<QPair<QString, QString>>& faces) {
  m_targets.clear();
  for (const QString& uid : bodies) {
    m_targets.append(Target{uid, QString(), model().bodyName(uid), QString(), QString()});
  }
  for (const auto& [owner, face] : faces) {
    m_targets.append(Target{owner, face, tr("Face of %1").arg(model().bodyName(owner)), QString(), QString()});
  }
  m_pending = false;
  m_activeKey.clear();
  m_noActive = false;
  m_loggedState.clear();
  m_message->clear();
  m_message->hide();
  reload();
}

void AssignmentPanel::reload() {
  if (m_targets.isEmpty()) {
    m_option.clear();
    m_custom.clear();
    m_rows.clear();
    setVisible(false);
    emit assignTextChanged();
    return;
  }
  setVisible(true);
  loadOptions();
  readTargets();
  if (m_pending) {
    // An option chosen and not written yet stays; the marks follow.
    fillCombo();
    showState();
  } else {
    rebuildFromState();
  }
}

bool AssignmentPanel::varies() const {
  return !m_targets.isEmpty() && !m_option.isEmpty() && m_custom.isEmpty() && !m_rows.isEmpty();
}

QString AssignmentPanel::assignText() const {
  if (!canFill()) {
    return tr("Assign");
  }
  // "Assign to Black" would read as "make it black": the row is named so.
  const QString targets = m_targets.size() > 1 ? QStringLiteral(" (%1)").arg(targetsCount()) : QString();
  const int selected = static_cast<int>(m_table->selectionModel()->selectedRows().size());
  if (selected > 1) {
    return tr("Assign to %1 Rows").arg(selected) + targets;
  }
  int row = m_table->currentRow();
  if (row < 0 || row >= m_rows.size()) {
    const QModelIndexList rows = m_table->selectionModel()->selectedRows();
    row = rows.isEmpty() ? -1 : rows.first().row();
  }
  if (row < 0 || row >= m_rows.size()) {
    return tr("Assign");
  }
  return tr("Assign to row %1").arg(m_rows[row].shown) + targets;
}

bool AssignmentPanel::canFill() const {
  if (!varies() || m_noActive) {
    return false;
  }
  const int row = m_table->currentRow();
  return !m_table->selectionModel()->selectedRows().isEmpty() || (row >= 0 && row < m_rows.size());
}

QString AssignmentPanel::assignTip() const {
  if (!canFill()) {
    return tr("Click a row to fill it");
  }
  return m_kind == Kind::Material ? tr("Give the selected rows of the table the selected material")
                                  : tr("Give the selected rows of the table the selected appearance");
}

bool AssignmentPanel::fill(const QString& result, const QJsonArray& before) {
  if (!varies() || result.isEmpty()) {
    return false;
  }
  QVector<int> rows;
  for (const QModelIndex& index : m_table->selectionModel()->selectedRows()) {
    rows << index.row();
  }
  if (rows.isEmpty() && m_table->currentRow() >= 0 && !m_noActive) {
    rows << m_table->currentRow();
  }
  rows.erase(std::remove_if(rows.begin(), rows.end(), [this](int row) { return row < 0 || row >= m_rows.size(); }),
             rows.end());
  if (rows.isEmpty()) {
    m_message->setText(tr("Click a row to fill it."));
    m_message->show();
    qDebug().noquote() << QStringLiteral("%1: no row to fill").arg(m_log);
    return false;
  }
  std::sort(rows.begin(), rows.end());
  const QVector<Row> previous = m_rows;
  QStringList filled;
  for (const int row : std::as_const(rows)) {
    m_rows[row].result = result;
    filled << m_rows[row].shown;
  }
  // The next row takes the next pick; after the last row none does, so that
  // the next pick does not overwrite it unasked.
  const bool last = rows.last() + 1 >= m_rows.size();
  const QString previousActive = m_activeKey;
  m_activeKey = last ? QString() : rowKey(rows.last() + 1);
  m_noActive = last;
  qDebug().noquote() << QStringLiteral("%1 filled %2 with %3").arg(m_log, filled.join(QStringLiteral(", ")), result);
  if (!write(rowsExpression(), before)) {
    m_rows = previous;
    m_activeKey = previousActive;
    m_noActive = false;
    showState();
    return false;
  }
  if (last) {
    qDebug().noquote() << QStringLiteral("%1 active row: none").arg(m_log);
  }
  return true;
}

void AssignmentPanel::showEvent(QShowEvent* event) {
  QWidget::showEvent(event);
  m_loggedPlaces.clear();
  scheduleLogPlaces();
}

void AssignmentPanel::resizeEvent(QResizeEvent* event) {
  QWidget::resizeEvent(event);
  scheduleLogPlaces();
}

void AssignmentPanel::loadOptions() {
  m_options.clear();
  // The configuration's options first (mitcad#798) with their domains,
  // then the other user parameters that have a list of allowed values.
  try {
    const QJsonObject configure = model().queryObject({{QStringLiteral("query"), QStringLiteral("configure")}});
    for (const QJsonValue& value : configure.value(QStringLiteral("options")).toArray()) {
      const QJsonObject o = value.toObject();
      Option option;
      option.name = o.value(QStringLiteral("name")).toString();
      option.text = o.value(QStringLiteral("kind")).toString() == QStringLiteral("text");
      option.currentExpression = o.value(QStringLiteral("expression")).toString();
      option.currentText = o.value(QStringLiteral("text")).toString();
      const QJsonObject domain = o.value(QStringLiteral("domain")).toObject();
      option.closed = domain.value(QStringLiteral("kind")).toString() == QStringLiteral("list");
      for (const QJsonValue& v : domain.value(QStringLiteral("values")).toArray()) {
        const QJsonObject listed = v.toObject();
        option.values << listed.value(QStringLiteral("expression")).toString();
        option.shown << listed.value(QStringLiteral("text")).toString();
      }
      if (!option.name.isEmpty()) {
        m_options.append(option);
      }
    }
  } catch (const std::exception&) {
    // No configuration: the user parameters only.
  }
  try {
    for (const QJsonValue& value : model().queryArray(QStringLiteral("parameters"))) {
      const QJsonObject p = value.toObject();
      const QString name = p.value(QStringLiteral("name")).toString();
      const QJsonObject limits = p.value(QStringLiteral("limits")).toObject();
      const QJsonArray values = limits.value(QStringLiteral("values")).toArray();
      if (p.value(QStringLiteral("kind")).toString() != QStringLiteral("user") || values.isEmpty() ||
          std::any_of(m_options.begin(), m_options.end(), [&name](const Option& o) { return o.name == name; })) {
        continue;
      }
      Option option;
      option.name = name;
      option.text = p.value(QStringLiteral("unit")).toString() == QStringLiteral("Text");
      option.closed = true;
      option.currentExpression = p.value(QStringLiteral("expression")).toString();
      option.currentText = p.value(QStringLiteral("text")).toString();
      if (option.text && option.currentText.isEmpty()) {
        option.currentText = unquotedText(option.currentExpression);
      }
      const QJsonArray texts = limits.value(QStringLiteral("values_text")).toArray();
      for (int i = 0; i < values.size(); ++i) {
        option.values << values[i].toString();
        option.shown << (i < texts.size() ? texts[i].toString() : unquotedText(values[i].toString()));
      }
      m_options.append(option);
    }
  } catch (const std::exception& e) {
    qWarning().noquote() << QStringLiteral("%1: no parameters: %2").arg(m_log, QString::fromUtf8(e.what()));
  }
}

void AssignmentPanel::readTargets() {
  QJsonArray bodies;
  try {
    bodies = model().queryArray(QStringLiteral("bodies"));
  } catch (const std::exception& e) {
    qWarning().noquote() << QStringLiteral("%1: no bodies: %2").arg(m_log, QString::fromUtf8(e.what()));
  }
  const bool material = m_kind == Kind::Material;
  for (Target& target : m_targets) {
    target.expression.clear();
    target.value.clear();
    QJsonObject body;
    for (const QJsonValue& value : std::as_const(bodies)) {
      if (value.toObject().value(QStringLiteral("uid")).toString() == target.body) {
        body = value.toObject();
      }
    }
    if (material) {
      target.expression = body.value(QStringLiteral("material_expr")).toString();
      target.value = body.value(QStringLiteral("material")).toString();
    } else if (target.face.isEmpty()) {
      target.expression = body.value(QStringLiteral("appearance_expr")).toString();
      target.value = body.value(QStringLiteral("appearance")).toString();
    } else {
      // The face's own entry: by the name it was given, or one of the faces
      // that name finds now.
      QString stored = target.face;
      for (const QJsonValue& entry : body.value(QStringLiteral("face_appearances")).toArray()) {
        const QJsonObject look = entry.toObject();
        bool named = look.value(QStringLiteral("face")).toString() == target.face;
        for (const QJsonValue& name : look.value(QStringLiteral("faces")).toArray()) {
          named = named || name.toString() == target.face;
        }
        if (named) {
          stored = look.value(QStringLiteral("face")).toString();
          target.value = look.value(QStringLiteral("appearance")).toString();
        }
      }
      target.expression = body.value(QStringLiteral("face_appearance_exprs")).toObject().value(stored).toString();
      if (target.value.isEmpty()) {
        target.value = body.value(QStringLiteral("appearance")).toString();
      }
    }
    if (target.value.isEmpty()) {
      // None of its own: what it shows, its material's (its component's,
      // else steel).
      try {
        const QJsonObject made = model().queryObject(
            {{QStringLiteral("query"), QStringLiteral("body_material")}, {QStringLiteral("body"), target.body}});
        target.value = made.value(material ? QStringLiteral("id") : QStringLiteral("appearance")).toString();
      } catch (const std::exception&) {
        // Shown as the default.
      }
    }
  }
}

QString AssignmentPanel::stateOf(const Target& target) const {
  return target.expression.isEmpty() ? QStringLiteral("has ") + target.value
                                     : QStringLiteral("follows ") + target.expression;
}

bool AssignmentPanel::mixed() const {
  return std::any_of(m_targets.begin(), m_targets.end(),
                     [this](const Target& target) { return stateOf(target) != stateOf(m_targets.first()); });
}

QString AssignmentPanel::mixedText() const {
  // The targets by state, in their order: "Body1 and Body2 follow Color,
  // Body3 has Steel - Satin".
  QStringList states;
  QVector<QStringList> names;
  QVector<int> firsts;
  for (int i = 0; i < m_targets.size(); ++i) {
    const QString state = stateOf(m_targets[i]);
    const int at = static_cast<int>(states.indexOf(state));
    if (at < 0) {
      states << state;
      names.append(QStringList{m_targets[i].name});
      firsts << i;
    } else {
      names[at] << m_targets[i].name;
    }
  }
  QStringList parts;
  for (int i = 0; i < states.size(); ++i) {
    const Target& target = m_targets[firsts[i]];
    const bool many = names[i].size() > 1;
    const QString who = joinNames(names[i]);
    if (!target.expression.isEmpty()) {
      OptionMapping mapping;
      const QString what = OptionMapping::parse(target.expression, &mapping) ? mapping.option : target.expression;
      parts << (many ? tr("%1 follow %2") : tr("%1 follows %2")).arg(who, what);
    } else {
      const QString name = target.value.isEmpty() ? tr("the default") : m_host.describe(target.value).name;
      parts << (many ? tr("%1 have %2") : tr("%1 has %2")).arg(who, name.isEmpty() ? target.value : name);
    }
  }
  // What the first change does to them.
  const QString then =
      !m_option.isEmpty() && m_custom.isEmpty() ? tr("Filling a row gives %1 this table.").arg(allTargets())
      : m_kind == Kind::Material                ? tr("Assign gives %1 the selected material.").arg(allTargets())
                                                : tr("Assign gives %1 the selected appearance.").arg(allTargets());
  return tr("Mixed: %1. %2").arg(parts.join(QStringLiteral(", ")), then);
}

QString AssignmentPanel::targetsCount() const {
  int bodies = 0;
  int faces = 0;
  for (const Target& target : m_targets) {
    (target.face.isEmpty() ? bodies : faces) += 1;
  }
  const int count = static_cast<int>(m_targets.size());
  if (faces == 0) {
    return count == 1 ? tr("1 body") : tr("%1 bodies").arg(count);
  }
  if (bodies == 0) {
    return count == 1 ? tr("1 face") : tr("%1 faces").arg(count);
  }
  return tr("%1 items").arg(count);
}

QString AssignmentPanel::allTargets() const {
  if (m_targets.size() == 2) {
    // "both bodies", "both faces", "both items".
    return tr("both %1").arg(targetsCount().section(QLatin1Char(' '), 1));
  }
  return tr("all %1").arg(targetsCount());
}

QString AssignmentPanel::joinNames(const QStringList& names) {
  if (names.size() <= 1) {
    return names.value(0);
  }
  return tr("%1 and %2").arg(names.mid(0, names.size() - 1).join(QStringLiteral(", ")), names.last());
}

void AssignmentPanel::rebuildFromState() {
  m_option.clear();
  m_custom.clear();
  m_rows.clear();
  m_keptOtherwise.clear();
  const Target& first = m_targets.first();
  if (!first.expression.isEmpty()) {
    OptionMapping mapping;
    if (OptionMapping::parse(first.expression, &mapping)) {
      m_option = mapping.option;
      buildRows(&mapping);
    } else {
      // Another form: shown as it is, never rewritten silently.
      m_custom = first.expression;
    }
  }
  fillCombo();
  showState();
}

void AssignmentPanel::buildRows(const OptionMapping* mapping) {
  m_rows.clear();
  m_keptOtherwise.clear();
  const Target& first = m_targets.first();
  // What the targets have now: nothing changes until a row does.
  const QString prefill = first.value.isEmpty() ? QString() : quotedText(first.value);
  const Option* chosen = option(m_option);
  QVector<bool> used(mapping != nullptr ? mapping->rows.size() : 0, false);
  const auto resultFor = [mapping, &used, &prefill](const QString& value) {
    if (mapping == nullptr) {
      return prefill;
    }
    for (int i = 0; i < mapping->rows.size(); ++i) {
      if (!used[i] && sameValue(mapping->rows[i].first, value)) {
        used[i] = true;
        return mapping->rows[i].second;
      }
    }
    return mapping->otherwise;
  };
  if (chosen != nullptr) {
    for (int i = 0; i < chosen->values.size(); ++i) {
      m_rows.append(Row{chosen->values[i], chosen->shown[i], resultFor(chosen->values[i]), false});
    }
  }
  // Values the expression names that the option does not list (any more).
  if (mapping != nullptr) {
    for (int i = 0; i < mapping->rows.size(); ++i) {
      if (!used[i]) {
        m_rows.append(Row{mapping->rows[i].first, unquotedText(mapping->rows[i].first), mapping->rows[i].second, false});
      }
    }
  }
  if (chosen != nullptr && chosen->closed) {
    // No Other row; a default the expression had stays.
    m_keptOtherwise = mapping != nullptr ? mapping->otherwise : QString();
    return;
  }
  // Values that are not a closed list: the design's value now when no row
  // has it, and Other (the switch()'s default).
  const QString otherwise = mapping != nullptr ? mapping->otherwise : prefill;
  if (chosen != nullptr && !chosen->currentExpression.isEmpty() &&
      std::none_of(m_rows.begin(), m_rows.end(),
                   [chosen](const Row& row) { return sameValue(row.value, chosen->currentExpression); })) {
    m_rows.append(Row{chosen->currentExpression, chosen->currentText.isEmpty() ? chosen->currentExpression
                                                                               : chosen->currentText,
                      otherwise, false});
  }
  m_rows.append(Row{QString(), tr("Other"), otherwise, true});
}

const AssignmentPanel::Option* AssignmentPanel::option(const QString& name) const {
  for (const Option& o : m_options) {
    if (o.name == name) {
      return &o;
    }
  }
  return nullptr;
}

bool AssignmentPanel::isCurrent(int row) const {
  const Option* chosen = option(m_option);
  if (chosen == nullptr || row < 0 || row >= m_rows.size()) {
    return false;
  }
  const auto holds = [this, chosen](int i) {
    const Row& r = m_rows[i];
    return !r.other && (sameValue(r.value, chosen->currentExpression) ||
                        (!chosen->currentText.isEmpty() && r.shown == chosen->currentText));
  };
  if (!m_rows[row].other) {
    return holds(row);
  }
  for (int i = 0; i < m_rows.size(); ++i) {
    if (holds(i)) {
      return false;
    }
  }
  return true;
}

QString AssignmentPanel::rowsExpression() const {
  OptionMapping mapping;
  mapping.option = m_option;
  bool other = false;
  for (const Row& row : m_rows) {
    if (row.other) {
      other = true;
      mapping.otherwise = row.result;
    } else if (!row.result.isEmpty()) {
      mapping.rows.append({row.value, row.result});
    }
  }
  if (!other) {
    mapping.otherwise = m_keptOtherwise;
  }
  if (mapping.rows.isEmpty()) {
    return mapping.otherwise;
  }
  return mapping.expression();
}

QString AssignmentPanel::rowKey(int row) const {
  if (row < 0 || row >= m_rows.size()) {
    return QString();
  }
  return m_rows[row].other ? QString::fromLatin1(kOtherKey) : m_rows[row].value;
}

int AssignmentPanel::rowOfKey(const QString& key) const {
  for (int i = 0; i < m_rows.size(); ++i) {
    if (m_rows[i].other ? key == QLatin1String(kOtherKey) : sameValue(m_rows[i].value, key)) {
      return i;
    }
  }
  return -1;
}

QString AssignmentPanel::title() const {
  int bodies = 0;
  int faces = 0;
  for (const Target& target : m_targets) {
    (target.face.isEmpty() ? bodies : faces) += 1;
  }
  if (faces == 0) {
    return bodies == 1 ? tr("Assign to %1").arg(m_targets.first().name) : tr("Assign to %1 Bodies").arg(bodies);
  }
  if (bodies == 0) {
    return faces == 1 ? tr("Assign to Face") : tr("Assign to %1 Faces").arg(faces);
  }
  return tr("Assign to %1 Items").arg(m_targets.size());
}

QString AssignmentPanel::targetsLabel() const {
  QStringList names;
  for (const Target& target : m_targets) {
    names << target.name;
  }
  return joinNames(names);
}

void AssignmentPanel::fillCombo() {
  m_building = true;
  {
    const QSignalBlocker block(m_varies);
    m_varies->clear();
    m_varies->addItem(tr("None"), QString());
    for (const Option& o : std::as_const(m_options)) {
      m_varies->addItem(o.name, o.name);
    }
    // An option the targets follow that the design does not list (now).
    if (!m_option.isEmpty() && option(m_option) == nullptr) {
      m_varies->addItem(m_option, m_option);
    }
    if (!m_custom.isEmpty()) {
      m_varies->addItem(tr("Custom"), QString::fromLatin1(kCustomEntry));
    }
    if (m_options.isEmpty()) {
      m_varies->addItem(tr("Add Option…"), QString::fromLatin1(kAddEntry));
    }
    const QString shown = !m_custom.isEmpty() ? QString::fromLatin1(kCustomEntry) : m_option;
    m_varies->setCurrentIndex(std::max(0, m_varies->findData(shown)));
  }
  m_building = false;
}

void AssignmentPanel::showState() {
  const bool table = !m_option.isEmpty() && m_custom.isEmpty();
  const bool custom = !m_custom.isEmpty();
  const Target& first = m_targets.first();
  m_title->setText(title());
  m_fx->setVisible(!m_pending && !first.expression.isEmpty());
  // None: one row, what the first target has now.
  const Result now = first.value.isEmpty() ? Result() : m_host.describe(first.value);
  m_now->setText(first.value.isEmpty() ? tr("Default") : (now.name.isEmpty() ? first.value : now.name));
  m_nowSwatch->setPixmap(now.icon.isNull() ? QPixmap() : now.icon.pixmap(kSwatch, kSwatch));
  m_nowLabel->setVisible(!table && !custom);
  m_now->setVisible(!table && !custom);
  m_nowSwatch->setVisible(!table && !custom && !now.icon.isNull());
  m_table->setVisible(table);
  if (table) {
    showRows();
  } else {
    m_building = true;
    m_table->setRowCount(0);
    m_building = false;
  }
  m_customText->setText(tr("Custom: %1").arg(m_custom));
  m_customText->setVisible(custom);
  m_replace->setVisible(custom);
  m_replace->setEnabled(!m_options.isEmpty());
  m_formula->setVisible(table || custom);
  const bool isMixed = mixed();
  m_mixed->setText(isMixed ? mixedText() : QString());
  m_mixedBox->setVisible(isMixed);
  if (m_message->text().isEmpty()) {
    m_message->hide();
  }
  // The state as a script reads it.
  QStringList lines;
  lines << QStringLiteral("%1 panel: %2; varies with %3")
               .arg(m_log, title(),
                    custom  ? QStringLiteral("custom: %1").arg(m_custom)
                    : table ? QStringLiteral("%1 (%2)").arg(m_option, m_pending ? QStringLiteral("not written")
                                                                                : QStringLiteral("bound"))
                            : QStringLiteral("None (%1)").arg(m_now->text()));
  QStringList entries;
  for (int i = 0; i < m_varies->count(); ++i) {
    entries << m_varies->itemText(i);
  }
  lines << QStringLiteral("%1 varies with: %2").arg(m_log, entries.join(QStringLiteral(" | ")));
  if (isMixed) {
    lines << QStringLiteral("%1 mixed: %2").arg(m_log, m_mixed->text());
  }
  for (int i = 0; table && i < m_rows.size(); ++i) {
    lines << QStringLiteral("%1 value %2: %3%4%5")
                 .arg(m_log, m_rows[i].shown, m_table->item(i, kResultColumn)->text(),
                      isCurrent(i) ? QStringLiteral(" (now)") : QString(),
                      m_table->currentRow() == i ? QStringLiteral(" (active)") : QString());
  }
  const QString logged = lines.join(QLatin1Char('\n'));
  if (logged != m_loggedState) {
    m_loggedState = logged;
    for (const QString& logLine : std::as_const(lines)) {
      qDebug().noquote() << logLine;
    }
  }
  emit assignTextChanged();
  scheduleLogPlaces();
}

void AssignmentPanel::showRows() {
  m_building = true;
  m_table->clearSelection();
  m_table->setRowCount(static_cast<int>(m_rows.size()));
  for (int i = 0; i < m_rows.size(); ++i) {
    const Row& row = m_rows[i];
    const auto cell = [this, i](int column) {
      QTableWidgetItem* item = m_table->item(i, column);
      if (item == nullptr) {
        item = new QTableWidgetItem;
        item->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable);
        m_table->setItem(i, column, item);
      }
      return item;
    };
    // The design's value now in words: "Black (now)".
    QTableWidgetItem* value = cell(kValueColumn);
    const bool current = isCurrent(i);
    value->setText(current ? tr("%1 (now)").arg(row.shown) : row.shown);
    const QString what = row.other ? tr("Every other value (the default)") : row.value;
    value->setToolTip(current ? tr("%1: the design's value now").arg(what) : what);
    QTableWidgetItem* result = cell(kResultColumn);
    const QString text = unquotedText(row.result);
    const Result described = text.isEmpty() ? Result() : m_host.describe(text);
    result->setText(row.result.isEmpty() ? tr("None") : (described.name.isEmpty() ? text : described.name));
    result->setIcon(described.icon);
    result->setToolTip(row.result.isEmpty() ? tr("No result: this value gives the default") : row.result);
  }
  // The row the next pick fills is the selected one (no mark of its own);
  // none after the last row was filled, until one is clicked.
  int active = m_noActive ? -1 : rowOfKey(m_activeKey);
  if (active < 0 && !m_noActive) {
    active = 0;
  }
  if (active >= 0 && active < m_rows.size()) {
    m_table->setCurrentCell(active, kValueColumn);
    m_table->selectRow(active);
  } else {
    m_table->setCurrentIndex(QModelIndex());
    m_table->clearSelection();
  }
  m_activeKey = rowKey(active);
  // As high as its rows, up to a few, then it scrolls; the result column
  // always takes the rest of the width.
  const int shown = std::min(static_cast<int>(m_rows.size()), kRowsInSight);
  int tableHeight = m_table->horizontalHeader()->sizeHint().height() + 2 * m_table->frameWidth() + 2;
  for (int i = 0; i < shown; ++i) {
    tableHeight += m_table->rowHeight(i);
  }
  m_table->setFixedHeight(tableHeight);
  m_building = false;
}

void AssignmentPanel::optionChosen(int index) {
  if (m_building || m_targets.isEmpty()) {
    return;
  }
  const QString data = m_varies->itemData(index).toString();
  const Target& first = m_targets.first();
  m_message->clear();
  if (data == QLatin1String(kAddEntry)) {
    // No option yet: the Configurations window makes one.
    fillCombo();
    qDebug().noquote() << QStringLiteral("%1: Add Option…").arg(m_log);
    if (m_host.openConfigurations) {
      m_host.openConfigurations();
    }
    return;
  }
  if (data == QLatin1String(kCustomEntry)) {
    return;
  }
  if (data.isEmpty()) {
    const bool bound = std::any_of(m_targets.begin(), m_targets.end(),
                                   [](const Target& target) { return !target.expression.isEmpty(); });
    if (bound) {
      backToNone();
      return;
    }
    // An option chosen, nothing written: None again as it was.
    m_pending = false;
    rebuildFromState();
    qDebug().noquote() << QStringLiteral("%1 varies with None").arg(m_log);
    return;
  }
  OptionMapping mapping;
  m_option = data;
  m_custom.clear();
  if (!first.expression.isEmpty() && OptionMapping::parse(first.expression, &mapping) && mapping.option == data) {
    // The option the first target follows: its table.
    m_pending = false;
    buildRows(&mapping);
  } else {
    // Prefilled with what the targets have: nothing is written until a row
    // changes.
    m_pending = true;
    buildRows(nullptr);
  }
  m_activeKey = rowKey(0);
  m_noActive = false;
  fillCombo();
  showState();
  qDebug().noquote() << QStringLiteral("%1 varies with %2: %3 rows%4")
                            .arg(m_log, data)
                            .arg(m_rows.size())
                            .arg(m_pending ? QStringLiteral(", not written yet") : QString());
}

bool AssignmentPanel::write(const QString& expression, const QJsonArray& before) {
  if (expression.isEmpty() || !m_host.canChange()) {
    return false;
  }
  const bool material = m_kind == Kind::Material;
  QJsonArray commands = before;
  QStringList owners;
  QHash<QString, QJsonArray> faces;
  for (const Target& target : std::as_const(m_targets)) {
    if (target.face.isEmpty()) {
      commands.append(QJsonObject{
          {QStringLiteral("cmd"), material ? QStringLiteral("set_body_material") : QStringLiteral("set_body_appearance")},
          {QStringLiteral("uid"), target.body},
          {QStringLiteral("expression"), expression}});
    } else {
      if (!owners.contains(target.body)) {
        owners << target.body;
      }
      faces[target.body].append(target.face);
    }
  }
  for (const QString& owner : std::as_const(owners)) {
    commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("set_face_appearance")},
                                {QStringLiteral("uid"), owner},
                                {QStringLiteral("faces"), faces.value(owner)},
                                {QStringLiteral("expression"), expression}});
  }
  const QString who = targetsLabel();
  const QString label = material ? tr("Material of %1").arg(who) : tr("Appearance of %1").arg(who);
  const bool done =
      commands.size() == 1 ? m_host.runCommand(commands.first().toObject()) : m_host.runCommands(commands, label);
  if (!done) {
    m_message->setText(m_host.lastError());
    m_message->show();
    qDebug().noquote() << QStringLiteral("%1 binding of %2 refused: %3").arg(m_log, who, m_host.lastError());
    return false;
  }
  m_message->clear();
  m_message->hide();
  qDebug().noquote() << QStringLiteral("%1 binding of %2: %3").arg(m_log, who, expression);
  m_pending = false;
  reload();
  return true;
}

void AssignmentPanel::backToNone() {
  if (!m_host.canChange()) {
    fillCombo();
    return;
  }
  // The result of the design's value now, else what the first target has.
  QString id;
  for (int i = 0; i < m_rows.size() && m_custom.isEmpty(); ++i) {
    if (isCurrent(i) && !m_rows[i].result.isEmpty()) {
      id = m_host.describe(unquotedText(m_rows[i].result)).id;
    }
  }
  if (id.isEmpty()) {
    id = m_targets.first().value;
  }
  const bool material = m_kind == Kind::Material;
  QJsonArray commands;
  QStringList owners;
  QHash<QString, QJsonArray> faces;
  for (const Target& target : std::as_const(m_targets)) {
    if (material) {
      commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("set_body_material")},
                                  {QStringLiteral("uid"), target.body},
                                  {QStringLiteral("material"), id.isEmpty() ? QJsonValue() : QJsonValue(id)}});
    } else if (target.face.isEmpty()) {
      commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("set_body_appearance")},
                                  {QStringLiteral("uid"), target.body},
                                  {QStringLiteral("appearance"), id.isEmpty() ? QJsonValue() : QJsonValue(id)}});
    } else {
      if (!owners.contains(target.body)) {
        owners << target.body;
      }
      faces[target.body].append(target.face);
    }
  }
  for (const QString& owner : std::as_const(owners)) {
    commands.append(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("set_face_appearance")},
                                {QStringLiteral("uid"), owner},
                                {QStringLiteral("faces"), faces.value(owner)},
                                {QStringLiteral("appearance"), id.isEmpty() ? QJsonValue() : QJsonValue(id)}});
  }
  const QString who = targetsLabel();
  const QString label = material ? tr("Material of %1").arg(who) : tr("Appearance of %1").arg(who);
  const bool done =
      commands.size() == 1 ? m_host.runCommand(commands.first().toObject()) : m_host.runCommands(commands, label);
  if (!done) {
    m_message->setText(m_host.lastError());
    m_message->show();
    qDebug().noquote() << QStringLiteral("%1 back to None refused: %2").arg(m_log, m_host.lastError());
    fillCombo();
    return;
  }
  m_message->clear();
  m_message->hide();
  qDebug().noquote() << QStringLiteral("%1 back to None: %2 for %3")
                            .arg(m_log, id.isEmpty() ? QStringLiteral("default") : id, who);
  m_pending = false;
  reload();
}

void AssignmentPanel::openFormulaEditor() {
  const ParameterSource* source = parameterSourceOf(this);
  if (source == nullptr || !source->query || m_targets.isEmpty()) {
    return;
  }
  ParameterFieldOptions options;
  options.kind = ParameterKind::Text;
  options.anyKind = true;
  options.label = title();
  const QString shown = !m_custom.isEmpty() ? m_custom : rowsExpression();
  FormulaEditor editor(*source, options, shown, this);
  if (execDialog(&editor) != QDialog::Accepted) {
    return;
  }
  const QString text = editor.expression().trimmed();
  qDebug().noquote() << QStringLiteral("%1 from the Formula Editor: %2").arg(m_log, text);
  if (!text.isEmpty() && (text != m_targets.first().expression || mixed())) {
    write(text);
  }
}

void AssignmentPanel::replaceWithTable() {
  if (m_options.isEmpty() || m_targets.isEmpty()) {
    return;
  }
  // The first option, prefilled: nothing is written until a row changes.
  m_custom.clear();
  m_option = m_options.first().name;
  m_pending = true;
  buildRows(nullptr);
  m_activeKey = rowKey(0);
  m_noActive = false;
  fillCombo();
  showState();
  qDebug().noquote() << QStringLiteral("%1: the expression replaced with a table of %2 (not written yet)")
                            .arg(m_log, m_option);
}

void AssignmentPanel::scheduleLogPlaces() {
  TestSync::singleShot(0, this, [this] { logPlaces(); });
}

void AssignmentPanel::logPlaces() {
  // In the main window's coordinates (the parent of the panel's window), as
  // the other logged places.
  if (!isVisible() || window()->parentWidget() == nullptr) {
    return;
  }
  const QPoint origin = window()->parentWidget()->window()->mapToGlobal(QPoint(0, 0));
  const auto at = [&origin](const QWidget* widget, const QPoint& point) {
    const QPoint global = widget->mapToGlobal(point) - origin;
    return QStringLiteral("%1,%2").arg(global.x()).arg(global.y());
  };
  QStringList lines;
  lines << QStringLiteral("%1 varies at %2").arg(m_log, at(m_varies, m_varies->rect().center()));
  if (m_table->isVisible()) {
    for (int i = 0; i < m_table->rowCount() && i < m_rows.size(); ++i) {
      const QRect rect = m_table->visualItemRect(m_table->item(i, kValueColumn));
      if (m_table->viewport()->rect().contains(rect.center())) {
        lines << QStringLiteral("%1 value %2 at %3").arg(m_log, m_rows[i].shown, at(m_table->viewport(), rect.center()));
      }
    }
  }
  if (m_formula->isVisible()) {
    lines << QStringLiteral("%1 formula at %2").arg(m_log, at(m_formula, m_formula->rect().center()));
  }
  if (m_replace->isVisible()) {
    lines << QStringLiteral("%1 replace_table at %2").arg(m_log, at(m_replace, m_replace->rect().center()));
  }
  const QString logged = lines.join(QLatin1Char('\n'));
  if (logged != m_loggedPlaces) {
    m_loggedPlaces = logged;
    for (const QString& entry : std::as_const(lines)) {
      qDebug().noquote() << entry;
    }
  }
}

} // namespace mitcad
