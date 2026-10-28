// SPDX-License-Identifier: MIT
#include "OptionChooser.hpp"

#include <algorithm>
#include <cmath>
#include <utility>

#include <QComboBox>
#include <QDoubleSpinBox>
#include <QFormLayout>
#include <QLabel>
#include <QLineEdit>
#include <QSignalBlocker>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/Json.hpp"
#include "../framework/Theme.hpp"

namespace mitcad {
namespace {

QString kindOf(const PartOption& option) {
  if (option.driven) {
    return QStringLiteral("driven");
  }
  if (option.kind == PartOption::Kind::List) {
    return QStringLiteral("list");
  }
  return option.text ? QStringLiteral("text") : QStringLiteral("number");
}

// A number field's limits in the option's unit (none: a wide range).
constexpr double kWide = 1e9;

} // namespace

OptionChooser::OptionChooser(QWidget* parent) : QWidget(parent) {
  auto* layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  m_form = new QFormLayout;
  m_form->setContentsMargins(0, 0, 0, 0);
  layout->addLayout(m_form);
  m_notice = new QLabel;
  m_notice->setObjectName(QStringLiteral("partOptionsNotice"));
  m_notice->setWordWrap(true);
  m_notice->hide();
  layout->addWidget(m_notice);
}

void OptionChooser::clear() {
  // The widgets go when control is back in the event loop: a chooser's own
  // signal may have asked for the new ones.
  while (m_form->rowCount() > 0) {
    const QFormLayout::TakeRowResult taken = m_form->takeRow(0);
    QLayoutItem* const items[] = {taken.labelItem, taken.fieldItem};
    for (QLayoutItem* item : items) {
      if (item == nullptr) {
        continue;
      }
      if (QWidget* widget = item->widget()) {
        widget->hide();
        widget->deleteLater();
      }
      delete item;
    }
  }
  m_combos.clear();
  m_fields.clear();
  m_names.clear();
  m_kinds.clear();
}

void OptionChooser::say(const QString& text, bool problem) {
  m_notice->setText(text);
  m_notice->setVisible(!text.isEmpty());
  if (problem) {
    setErrorStyleSheet(m_notice);
  } else {
    m_notice->setStyleSheet(QString());
  }
}

void OptionChooser::setTable(const QJsonObject& configurations, const QString& row, const QJsonObject& parameters) {
  m_filling = true;
  clear();
  say(QString(), false);
  m_selectors.clear();
  m_rows = configurations.value(QStringLiteral("rows")).toArray();
  m_row.clear();
  m_answer = OptionsAnswer();
  m_wanted = QJsonObject();
  m_open = static_cast<bool>(m_query) && isOpenTable(configurations);
  if (!m_open) {
    setRows(configurations, row);
    return;
  }
  // The values to start with: the row asked for, else the default row's
  // (`config` gives the starting values, docs/configurations.md).
  const QString first = row.isEmpty() ? str(configurations, "default") : row;
  QJsonObject start;
  for (const QJsonValue& value : m_rows) {
    if (!first.isEmpty() && str(value.toObject(), "name") == first) {
      start = rowOptionValues(configurations, value.toObject());
    }
  }
  for (auto it = parameters.begin(); it != parameters.end(); ++it) {
    start.insert(it.key(), valueOfExpression(it.value().toString()));
  }
  m_filling = false;
  if (!settle(start, -1) && m_answer.options.isEmpty()) {
    // A library or a model without the options' answer: its rows as before.
    qInfo().noquote() << QStringLiteral("%1 options not read (%2): the rows instead")
                             .arg(m_logName, m_answer.ok() ? QStringLiteral("refused") : m_answer.error);
    m_open = false;
    m_filling = true;
    setRows(configurations, row);
  }
}

bool OptionChooser::showOptions() {
  m_filling = true;
  clear();
  say(QString(), false);
  m_selectors.clear();
  m_rows = QJsonArray();
  m_row.clear();
  m_answer = OptionsAnswer();
  m_wanted = QJsonObject();
  m_filling = false;
  m_open = static_cast<bool>(m_query);
  if (!m_open || !settle(QJsonObject(), -1)) {
    m_open = false;
    if (!m_answer.error.isEmpty()) {
      say(m_answer.error, true);
    }
    return false;
  }
  return true;
}

void OptionChooser::requery() {
  if (m_open) {
    settle(m_wanted, -1);
  }
}

QJsonObject OptionChooser::parameters() const {
  QJsonObject out;
  if (!m_open) {
    return out;
  }
  for (const PartOption& option : m_answer.options) {
    if (option.driven) {
      continue;
    }
    out.insert(option.name, isConstantExpression(option.expression) ? option.expression
                                                                     : constantExpression(option, option.valueText));
  }
  return out;
}

QString OptionChooser::summary() const {
  if (m_open) {
    return optionsSummary(m_answer);
  }
  return m_row.isEmpty() ? QStringLiteral("-") : m_row;
}

// --- A closed table: its rows

void OptionChooser::setRows(const QJsonObject& configurations, const QString& row) {
  m_selectors = tableOptions(configurations);
  if (m_rows.isEmpty()) {
    m_filling = false;
    emit rowChanged(m_row);
    return;
  }
  const QString first = row.isEmpty() ? str(configurations, "default") : row;
  QJsonObject start = m_rows.first().toObject();
  for (const QJsonValue& value : m_rows) {
    if (str(value.toObject(), "name") == first) {
      start = value.toObject();
    }
  }
  if (m_selectors.isEmpty()) {
    // Rows chosen by their names.
    auto* combo = new QComboBox;
    combo->setObjectName(QStringLiteral("librarySize0"));
    for (const QJsonValue& value : m_rows) {
      combo->addItem(str(value.toObject(), "name"));
    }
    combo->setCurrentText(str(start, "name"));
    m_form->addRow(tr("Size:"), combo);
    m_combos << combo;
    m_fields << combo;
    connect(combo, &QComboBox::currentIndexChanged, this, [this] { choose(); });
  } else {
    for (int i = 0; i < m_selectors.size(); ++i) {
      auto* combo = new QComboBox;
      combo->setObjectName(QStringLiteral("librarySize%1").arg(i));
      m_form->addRow(m_selectors[i] + QLatin1Char(':'), combo);
      m_combos << combo;
      m_fields << combo;
      connect(combo, &QComboBox::currentIndexChanged, this, [this, i] {
        if (!m_filling) {
          fill(i + 1);
          choose();
        }
      });
    }
    // The values of the first selector, then the start row's.
    for (int i = 0; i < m_combos.size(); ++i) {
      fill(i);
      m_combos[i]->setCurrentText(rowOptionText(start, m_selectors[i]));
    }
  }
  m_filling = false;
  choose();
}

void OptionChooser::fill(int from) {
  const bool filling = m_filling;
  m_filling = true;
  for (int i = from; i < m_combos.size() && i < m_selectors.size(); ++i) {
    QStringList values;
    for (const QJsonValue& value : m_rows) {
      const QJsonObject one = value.toObject();
      bool matches = true;
      for (int j = 0; j < i; ++j) {
        matches = matches && rowOptionText(one, m_selectors[j]) == m_combos[j]->currentText();
      }
      const QString v = rowOptionText(one, m_selectors[i]);
      if (matches && !values.contains(v)) {
        values << v;
      }
    }
    const QString keep = m_combos[i]->currentText();
    m_combos[i]->clear();
    m_combos[i]->addItems(values);
    if (values.contains(keep)) {
      m_combos[i]->setCurrentText(keep);
    }
  }
  m_filling = filling;
}

void OptionChooser::choose() {
  if (m_filling) {
    return;
  }
  QString found;
  if (m_selectors.isEmpty()) {
    found = m_combos.isEmpty() ? QString() : m_combos.first()->currentText();
  } else {
    for (const QJsonValue& value : m_rows) {
      const QJsonObject one = value.toObject();
      bool matches = true;
      for (int i = 0; i < m_selectors.size() && i < m_combos.size(); ++i) {
        matches = matches && rowOptionText(one, m_selectors[i]) == m_combos[i]->currentText();
      }
      if (matches) {
        found = str(one, "name");
        break;
      }
    }
  }
  if (found != m_row) {
    m_row = found;
    qInfo().noquote() << QStringLiteral("%1 size %2").arg(m_logName, m_row);
    emit rowChanged(m_row);
  }
}

// --- An open table: a chooser per option

bool OptionChooser::settle(const QJsonObject& wanted, int changed) {
  if (!m_query) {
    return false;
  }
  const OptionsAnswer before = m_answer;
  // The options up to the one changed, as chosen.
  QJsonObject fixed;
  const QStringList order = m_names.isEmpty() ? QStringList(wanted.keys()) : m_names;
  for (int i = 0; i <= changed && i < order.size(); ++i) {
    if (wanted.contains(order[i])) {
      fixed.insert(order[i], wanted.value(order[i]));
    }
  }
  OptionsAnswer answer = m_query(fixed);
  if (!answer.ok() || !answer.refused.isEmpty()) {
    const QString why = answer.ok() ? answer.refused : answer.error;
    qInfo().noquote() << QStringLiteral("%1 option refused: %2").arg(m_logName, why);
    if (before.options.isEmpty()) {
      m_answer = answer;
      if (!answer.ok() || answer.options.isEmpty()) {
        return false; // the caller falls back to the rows
      }
      // The start values refused (a row that breaks a rule): the options
      // as they are, with why.
      for (const PartOption& option : m_answer.options) {
        if (!option.driven) {
          m_wanted.insert(option.name, queryValue(option, option.valueText));
        }
      }
      showAnswer(m_answer);
      say(why, true);
      m_row = m_answer.row;
      emit rowChanged(m_row);
      emit optionsChanged();
      return true;
    }
    say(why, true);
    showAnswer(m_answer);
    return false;
  }
  // The later options keep their values where the rules allow them, in
  // order (decision 5: the cascade); the others moved.
  QList<OptionMove> moves;
  for (int i = changed + 1; i < answer.options.size(); ++i) {
    const PartOption now = answer.options[i];
    if (now.driven || !wanted.contains(now.name)) {
      continue;
    }
    const QJsonValue value = wanted.value(now.name);
    if (sameValue(now, value)) {
      fixed.insert(now.name, value);
      continue;
    }
    const bool allowed = now.text ? now.allows(value.toString())
                                  : now.allowsNumber(value.isDouble() ? value.toDouble()
                                                                      : constantNumber(value.toString(), now.unit));
    if (allowed) {
      QJsonObject tried = fixed;
      tried.insert(now.name, value);
      const OptionsAnswer kept = m_query(tried);
      if (kept.ok() && kept.refused.isEmpty()) {
        fixed = tried;
        answer = kept;
        continue;
      }
    }
    // Moved: from what it was to what the rules leave.
    const PartOption* was = before.option(now.name);
    QString rule = now.restrictedBy.join(QStringLiteral(", "));
    for (const OptionMove& move : answer.moves) {
      if (move.option == now.name && !move.rule.isEmpty()) {
        rule = move.rule;
      }
    }
    const QString from = was != nullptr ? was->valueText
                         : now.text  ? value.toString()
                                     : numberText(constantNumber(value.toString(), now.unit), now.unit);
    moves.append({now.name, from, now.valueText, rule});
  }
  m_answer = answer;
  m_wanted = QJsonObject();
  for (const PartOption& option : m_answer.options) {
    if (!option.driven) {
      m_wanted.insert(option.name, queryValue(option, option.valueText));
    }
  }
  showAnswer(m_answer);
  QStringList texts;
  for (const OptionMove& move : moves) {
    texts << move.text();
  }
  if (!texts.isEmpty()) {
    qInfo().noquote() << QStringLiteral("%1 option moves: %2").arg(m_logName, texts.join(QStringLiteral("; ")));
  }
  say(texts.join(QLatin1Char('\n')), false);
  qInfo().noquote() << QStringLiteral("%1 options: %2").arg(m_logName, optionsSummary(m_answer));
  for (const PartOption& option : m_answer.options) {
    qInfo().noquote() << QStringLiteral("%1 option %2 allowed %3%4")
                             .arg(m_logName, option.name, option.allowedText(),
                                  option.restrictedBy.isEmpty()
                                      ? QString()
                                      : QStringLiteral(" (%1)").arg(option.restrictedBy.join(QStringLiteral(", "))));
  }
  if (m_answer.row != m_row) {
    m_row = m_answer.row;
    emit rowChanged(m_row);
  }
  emit optionsChanged();
  return true;
}

void OptionChooser::showAnswer(const OptionsAnswer& shown) {
  QStringList names;
  QStringList kinds;
  for (const PartOption& option : shown.options) {
    names << option.name;
    kinds << kindOf(option);
  }
  if (names != m_names || kinds != m_kinds) {
    clear();
    m_names = names;
    m_kinds = kinds;
    build();
  }
  const bool filling = m_filling;
  m_filling = true;
  for (int i = 0; i < shown.options.size() && i < m_fields.size(); ++i) {
    const PartOption& option = shown.options[i];
    QWidget* field = m_fields[i];
    QString tip = tr("Allowed: %1").arg(option.allowedText());
    if (!option.restrictedBy.isEmpty()) {
      tip += QLatin1Char('\n') + tr("Narrowed by: %1").arg(option.restrictedBy.join(QStringLiteral(", ")));
    }
    if (option.driven) {
      tip = tr("Follows %1").arg(option.bound ? option.parameter : option.expression);
    }
    field->setToolTip(tip);
    if (auto* combo = qobject_cast<QComboBox*>(field)) {
      const QSignalBlocker blocker(combo);
      combo->clear();
      for (const OptionValueChoice& choice : option.values) {
        combo->addItem(choice.text);
      }
      if (combo->findText(option.valueText) < 0) {
        combo->addItem(option.valueText); // what it is, also when no longer allowed
      }
      combo->setCurrentText(option.valueText);
    } else if (auto* spin = qobject_cast<QDoubleSpinBox*>(field)) {
      const QSignalBlocker blocker(spin);
      const double factor = unitFactor(option.unit);
      const double low = std::isfinite(option.min) ? option.min / factor : -kWide;
      const double high = std::isfinite(option.max) ? option.max / factor : kWide;
      spin->setRange(low, std::max(low, high));
      spin->setSingleStep(std::isfinite(option.step) && option.step > 0.0 ? option.step / factor : 1.0);
      spin->setSuffix(option.unit.isEmpty() ? QString() : QLatin1Char(' ') + option.unit);
      if (std::isfinite(option.value)) {
        spin->setValue(option.value / factor);
      }
    } else if (auto* edit = qobject_cast<QLineEdit*>(field)) {
      const QSignalBlocker blocker(edit);
      edit->setText(option.driven ? option.valueText : valueOfExpression(option.valueText));
    }
  }
  m_filling = filling;
}

void OptionChooser::build() {
  for (int i = 0; i < m_names.size(); ++i) {
    const QString kind = m_kinds[i];
    QWidget* field = nullptr;
    if (kind == QStringLiteral("list")) {
      auto* combo = new QComboBox;
      connect(combo, &QComboBox::currentIndexChanged, this, [this, i, combo] {
        if (!m_filling && i < m_answer.options.size()) {
          optionEdited(i, queryValue(m_answer.options[i], combo->currentText()));
        }
      });
      field = combo;
    } else if (kind == QStringLiteral("number")) {
      auto* spin = new QDoubleSpinBox;
      spin->setDecimals(4);
      // The value is taken when it is confirmed (Enter, Tab), not at each key.
      spin->setKeyboardTracking(false);
      connect(spin, &QDoubleSpinBox::valueChanged, this, [this, i](double typed) {
        if (!m_filling && i < m_answer.options.size()) {
          const PartOption& option = m_answer.options[i];
          optionEdited(i, constantExpression(option, QString::number(typed, 'g', 12)));
        }
      });
      field = spin;
    } else {
      auto* edit = new QLineEdit;
      if (kind == QStringLiteral("driven")) {
        edit->setReadOnly(true);
      } else {
        connect(edit, &QLineEdit::editingFinished, this, [this, i, edit] {
          if (!m_filling && i < m_answer.options.size() &&
              edit->text() != valueOfExpression(m_answer.options[i].valueText)) {
            optionEdited(i, queryValue(m_answer.options[i], edit->text()));
          }
        });
      }
      field = edit;
    }
    field->setObjectName(QStringLiteral("partOption%1").arg(i));
    m_form->addRow(m_names[i] + QLatin1Char(':'), field);
    m_fields << field;
  }
}

void OptionChooser::setOption(const QString& option, const QJsonValue& value) {
  const int index = static_cast<int>(m_names.indexOf(option));
  if (index < 0) {
    requery();
    return;
  }
  optionEdited(index, value);
}

void OptionChooser::optionEdited(int index, const QJsonValue& value) {
  if (index < 0 || index >= m_answer.options.size()) {
    return;
  }
  const QString name = m_answer.options[index].name;
  qInfo().noquote() << QStringLiteral("%1 option %2 chosen: %3")
                           .arg(m_logName, name, value.isString() ? value.toString() : QString::number(value.toDouble()));
  QJsonObject wanted = m_wanted;
  wanted.insert(name, value);
  settle(wanted, index);
}

} // namespace mitcad
