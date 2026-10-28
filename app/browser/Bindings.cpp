// SPDX-License-Identifier: MIT
#include "Bindings.hpp"

#include <algorithm>
#include <exception>
#include <utility>

#include <QComboBox>
#include <QDialogButtonBox>
#include <QFont>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QIconEngine>
#include <QJsonObject>
#include <QJsonValue>
#include <QLabel>
#include <QLineEdit>
#include <QPainter>
#include <QPixmap>
#include <QPushButton>
#include <QShowEvent>
#include <QSignalBlocker>
#include <QTableWidget>
#include <QTimer>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/Dialogs.hpp"
#include "../framework/FormulaEditor.hpp"
#include "../framework/ParameterField.hpp"

namespace mitcad {
namespace {

// An icon with "fx" over its lower right corner.
class BoundIconEngine : public QIconEngine {
public:
  explicit BoundIconEngine(QIcon icon) : m_icon(std::move(icon)) {}

  void paint(QPainter* painter, const QRect& rect, QIcon::Mode mode, QIcon::State state) override {
    m_icon.paint(painter, rect, Qt::AlignCenter, mode, state);
    const int side = std::max(7, rect.height() * 3 / 5);
    const QRect badge(rect.right() + 1 - side, rect.bottom() + 1 - side, side, side);
    QFont font = painter->font();
    font.setItalic(true);
    font.setBold(true);
    font.setPixelSize(std::max(6, side * 4 / 5));
    painter->save();
    painter->setRenderHint(QPainter::Antialiasing);
    painter->setPen(Qt::NoPen);
    painter->setBrush(QColor(255, 255, 255, 220));
    painter->drawRoundedRect(badge, 2, 2);
    painter->setFont(font);
    painter->setPen(QColor(0x1f, 0x5f, 0xc0));
    painter->drawText(badge, Qt::AlignCenter, QStringLiteral("fx"));
    painter->restore();
  }

  QPixmap pixmap(const QSize& size, QIcon::Mode mode, QIcon::State state) override {
    QPixmap pixmap(size);
    pixmap.fill(Qt::transparent);
    QPainter painter(&pixmap);
    paint(&painter, QRect(QPoint(), size), mode, state);
    return pixmap;
  }

  QIconEngine* clone() const override { return new BoundIconEngine(m_icon); }

private:
  QIcon m_icon;
};

// The text of a text literal ('It''s' gives It's); the text itself when it
// is none.
QString unquoted(const QString& expression) {
  const QString text = expression.trimmed();
  if (text.size() >= 2 && (text.front() == QLatin1Char('\'') || text.front() == QLatin1Char('"')) &&
      text.back() == text.front()) {
    const QString quote(text.front());
    return text.mid(1, text.size() - 2).replace(quote + quote, quote);
  }
  return text;
}

// The arguments of a call's text between its parentheses, split at the
// semicolons outside quotes and parentheses.
QStringList splitArguments(const QString& inner) {
  QStringList parts;
  QString current;
  int depth = 0;
  QChar quote;
  for (const QChar c : inner) {
    if (!quote.isNull()) {
      current += c;
      if (c == quote) {
        quote = QChar(); // a doubled quote opens again at once
      }
      continue;
    }
    if (c == QLatin1Char('\'') || c == QLatin1Char('"')) {
      quote = c;
    } else if (c == QLatin1Char('(')) {
      ++depth;
    } else if (c == QLatin1Char(')')) {
      --depth;
    } else if (c == QLatin1Char(';') && depth == 0) {
      parts << current.trimmed();
      current.clear();
      continue;
    }
    current += c;
  }
  parts << current.trimmed();
  return parts;
}

bool isName(const QString& text) {
  if (text.isEmpty() || !(text.front().isLetter() || text.front() == QLatin1Char('_'))) {
    return false;
  }
  return std::all_of(text.begin(), text.end(),
                     [](QChar c) { return c.isLetterOrNumber() || c == QLatin1Char('_'); });
}

// "<name> at x,y" in the main window's coordinates, as the other logged
// places.
QString placeOf(const QWidget* dialog, const QWidget* widget) {
  const QWidget* window = dialog->parentWidget() != nullptr ? dialog->parentWidget()->window() : dialog;
  const QPoint at = widget->mapToGlobal(widget->rect().center()) - window->mapToGlobal(QPoint(0, 0));
  return QStringLiteral("%1,%2").arg(at.x()).arg(at.y());
}

} // namespace

QIcon boundIcon(const QIcon& icon) { return QIcon(new BoundIconEngine(icon)); }

QString quotedText(const QString& text) {
  QString doubled = text;
  doubled.replace(QLatin1Char('\''), QStringLiteral("''"));
  return QLatin1Char('\'') + doubled + QLatin1Char('\'');
}

QString OptionMapping::expression() const {
  QStringList parts{option};
  for (const auto& row : rows) {
    parts << row.first << row.second;
  }
  if (!otherwise.isEmpty()) {
    parts << otherwise;
  }
  return QStringLiteral("switch(%1)").arg(parts.join(QStringLiteral("; ")));
}

bool OptionMapping::parse(const QString& source, OptionMapping* mapping) {
  const QString text = source.trimmed();
  if (!text.startsWith(QStringLiteral("switch"), Qt::CaseInsensitive) || !text.endsWith(QLatin1Char(')'))) {
    return false;
  }
  const QString rest = text.mid(6).trimmed();
  if (!rest.startsWith(QLatin1Char('('))) {
    return false;
  }
  const QStringList parts = splitArguments(rest.mid(1, rest.size() - 2));
  if (parts.size() < 3 || !isName(parts.front()) ||
      std::any_of(parts.begin(), parts.end(), [](const QString& part) { return part.isEmpty(); })) {
    return false;
  }
  OptionMapping out;
  out.option = parts.front();
  int i = 1;
  for (; i + 1 < parts.size(); i += 2) {
    out.rows.append({parts[i], parts[i + 1]});
  }
  if (i < parts.size()) {
    out.otherwise = parts[i];
  }
  *mapping = out;
  return true;
}

// Suppress When… and Show When….

ConditionDialog::ConditionDialog(const QString& what, const QString& help, const QString& initial,
                                 QWidget* parent)
    : QDialog(parent) {
  setObjectName(QStringLiteral("conditionDialog"));
  setWindowTitle(what);
  auto* layout = new QVBoxLayout(this);
  auto* intro = new QLabel(help);
  intro->setWordWrap(true);
  layout->addWidget(intro);

  auto* row = new QHBoxLayout;
  m_condition = new QLineEdit(initial);
  m_condition->setObjectName(QStringLiteral("bindingCondition"));
  m_condition->setPlaceholderText(tr("Color = 'Black'"));
  m_condition->setMinimumWidth(m_condition->fontMetrics().horizontalAdvance(QLatin1Char('M')) * 28);
  ParameterFieldOptions options;
  options.kind = ParameterKind::Unitless;
  options.anyKind = true;
  options.label = what;
  ParameterField::attach(m_condition, options);
  row->addWidget(m_condition, 1);
  m_formula = new QPushButton(tr("Formula Editor…"));
  row->addWidget(m_formula);
  layout->addLayout(row);

  auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
  m_ok = buttons->button(QDialogButtonBox::Ok);
  m_remove = buttons->addButton(tr("Remove Condition"), QDialogButtonBox::ResetRole);
  m_remove->setEnabled(!initial.trimmed().isEmpty());
  layout->addWidget(buttons);
  connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
  connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
  connect(m_remove, &QPushButton::clicked, this, [this] {
    m_removed = true;
    accept();
  });
  connect(m_formula, &QPushButton::clicked, this, &ConditionDialog::openFormulaEditor);
  qDebug().noquote() << QStringLiteral("Condition dialog %1: %2")
                            .arg(what, initial.isEmpty() ? QStringLiteral("none") : initial);
}

QString ConditionDialog::condition() const { return m_removed ? QString() : m_condition->text().trimmed(); }

void ConditionDialog::showEvent(QShowEvent* event) {
  QDialog::showEvent(event);
  m_condition->setFocus();
  QTimer::singleShot(0, this, &ConditionDialog::logPlaces);
}

void ConditionDialog::openFormulaEditor() {
  const ParameterSource* source = parameterSourceOf(this);
  if (source == nullptr || !source->query) {
    return;
  }
  ParameterFieldOptions options;
  options.kind = ParameterKind::Unitless;
  options.anyKind = true;
  options.label = windowTitle();
  FormulaEditor editor(*source, options, m_condition->text(), this);
  if (execDialog(&editor) == QDialog::Accepted) {
    m_condition->setText(editor.expression());
    qDebug().noquote() << QStringLiteral("Condition from the Formula Editor: %1").arg(editor.expression());
  }
}

void ConditionDialog::logPlaces() {
  if (!isVisible()) {
    return;
  }
  qDebug().noquote() << QStringLiteral("Condition field at %1").arg(placeOf(this, m_condition));
  qDebug().noquote() << QStringLiteral("Condition formula at %1").arg(placeOf(this, m_formula));
  qDebug().noquote() << QStringLiteral("Condition remove at %1").arg(placeOf(this, m_remove));
  qDebug().noquote() << QStringLiteral("Condition OK at %1").arg(placeOf(this, m_ok));
}

// Follow Option….

FollowOptionDialog::FollowOptionDialog(const CommandContext& model, Kind kind, const QString& what,
                                       const QString& current, QWidget* parent)
    : QDialog(parent), m_model(model), m_kind(kind) {
  const bool material = kind == Kind::Material;
  setObjectName(QStringLiteral("followOptionDialog"));
  setWindowTitle(what);
  auto* layout = new QVBoxLayout(this);
  auto* intro = new QLabel(material ? tr("The material follows the option: a material for each of its values.")
                                    : tr("The appearance follows the option: an appearance for each of its values."));
  intro->setWordWrap(true);
  layout->addWidget(intro);

  auto* form = new QFormLayout;
  m_option = new QComboBox;
  m_option->setObjectName(QStringLiteral("followOptionOption"));
  form->addRow(tr("&Option:"), m_option);
  layout->addLayout(form);

  m_table = new QTableWidget(0, 2);
  m_table->setObjectName(QStringLiteral("followOptionTable"));
  m_table->setHorizontalHeaderLabels({tr("Value"), material ? tr("Material") : tr("Appearance")});
  m_table->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
  m_table->horizontalHeader()->setSectionResizeMode(1, QHeaderView::Stretch);
  m_table->verticalHeader()->setVisible(false);
  m_table->setMinimumWidth(440);
  layout->addWidget(m_table, 1);

  auto* rowButtons = new QHBoxLayout;
  m_add = new QPushButton(tr("Add Value"));
  m_removeRow = new QPushButton(tr("Remove Value"));
  m_formula = new QPushButton(tr("Formula Editor…"));
  rowButtons->addWidget(m_add);
  rowButtons->addWidget(m_removeRow);
  rowButtons->addStretch();
  rowButtons->addWidget(m_formula);
  layout->addLayout(rowButtons);

  auto* otherwise = new QFormLayout;
  m_otherwise = new QComboBox;
  m_otherwise->setObjectName(QStringLiteral("followOptionOtherwise"));
  otherwise->addRow(tr("Other values:"), m_otherwise);
  layout->addLayout(otherwise);

  m_text = new QLabel;
  m_text->setObjectName(QStringLiteral("followOptionText"));
  m_text->setWordWrap(true);
  m_text->setTextInteractionFlags(Qt::TextSelectableByMouse);
  layout->addWidget(m_text);

  auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
  m_ok = buttons->button(QDialogButtonBox::Ok);
  m_remove = buttons->addButton(tr("Remove Expression"), QDialogButtonBox::ResetRole);
  m_remove->setEnabled(!current.trimmed().isEmpty());
  layout->addWidget(buttons);
  connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
  connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
  connect(m_remove, &QPushButton::clicked, this, [this] {
    m_removed = true;
    accept();
  });

  loadChoices();
  loadOptions();
  // The otherwise choice has no entry of its own: the first row's result.
  m_otherwise->addItem(QString());
  for (const auto& choice : std::as_const(m_choices)) {
    m_otherwise->addItem(choice.second, choice.first);
  }
  m_otherwise->setEditable(true);
  m_otherwise->setCompleter(nullptr);

  // As it is: a mapping of this form fills the table, any other form is
  // kept as it is until the table is changed.
  OptionMapping given;
  const int found = OptionMapping::parse(current, &given) ? m_option->findData(given.option) : -1;
  if (found >= 0) {
    m_option->setCurrentIndex(found);
    for (const auto& row : std::as_const(given.rows)) {
      addRow(row.first, unquoted(row.first), row.second);
    }
    // The option's other values, without a result yet.
    const Option& option = m_options[found];
    for (int i = 0; i < option.values.size(); ++i) {
      const QString& value = option.values[i];
      const bool listed = std::any_of(given.rows.begin(), given.rows.end(),
                                      [&value](const auto& row) { return row.first == value; });
      if (!listed) {
        addRow(option.values[i], option.shown[i], QString());
      }
    }
    const QString other = unquoted(given.otherwise);
    const auto choice = std::find_if(m_choices.begin(), m_choices.end(),
                                     [&other](const auto& c) { return c.first == other; });
    m_otherwise->setEditText(choice != m_choices.end() ? choice->second : other);
  } else {
    optionChosen();
    if (!current.trimmed().isEmpty()) {
      m_free = current.trimmed();
    }
  }
  showText(current.trimmed());

  connect(m_option, &QComboBox::activated, this, [this] {
    optionChosen();
    showText(mapping().expression());
  });
  connect(m_add, &QPushButton::clicked, this, [this] {
    addRow(QString(), QString(), QString());
    m_table->setCurrentCell(m_table->rowCount() - 1, 0);
    m_table->editItem(m_table->item(m_table->rowCount() - 1, 0));
  });
  connect(m_removeRow, &QPushButton::clicked, this, [this] {
    const int row = m_table->currentRow();
    if (row >= 0) {
      m_table->removeRow(row);
      m_free.clear();
      showText(mapping().expression());
    }
  });
  connect(m_table, &QTableWidget::itemChanged, this, [this](QTableWidgetItem* item) {
    // A value typed by hand: its expression is made from the text.
    if (item->column() == 0 && item->text() != item->data(Qt::UserRole + 1).toString()) {
      item->setData(Qt::UserRole, QString());
    }
    m_free.clear();
    showText(mapping().expression());
  });
  connect(m_otherwise, &QComboBox::currentTextChanged, this, [this] {
    m_free.clear();
    showText(mapping().expression());
  });
  connect(m_formula, &QPushButton::clicked, this, &FollowOptionDialog::openFormulaEditor);
  qDebug().noquote() << QStringLiteral("Follow Option %1: option %2, %3 value(s); %4")
                            .arg(what, m_option->currentData().toString())
                            .arg(m_table->rowCount())
                            .arg(current.isEmpty() ? QStringLiteral("no expression") : current);
}

QString FollowOptionDialog::expression() const {
  if (m_removed) {
    return QString();
  }
  if (!m_free.isEmpty()) {
    return m_free;
  }
  const OptionMapping map = mapping();
  return map.rows.isEmpty() ? QString() : map.expression();
}

void FollowOptionDialog::showEvent(QShowEvent* event) {
  QDialog::showEvent(event);
  QTimer::singleShot(0, this, &FollowOptionDialog::logPlaces);
}

void FollowOptionDialog::loadChoices() {
  const QString query = m_kind == Kind::Material ? QStringLiteral("materials") : QStringLiteral("appearances");
  try {
    for (const QJsonValue& value : m_model.queryArray(query)) {
      const QJsonObject item = value.toObject();
      const QString id = item.value(QStringLiteral("id")).toString();
      if (!id.isEmpty()) {
        m_choices.append({id, item.value(QStringLiteral("name")).toString(id)});
      }
    }
  } catch (const std::exception& e) {
    qWarning().noquote() << QStringLiteral("Follow Option: no %1: %2").arg(query, QString::fromUtf8(e.what()));
  }
}

void FollowOptionDialog::loadOptions() {
  // The configuration's options first (mitcad#798), with their allowed
  // values; then the other user parameters, with their `values` limit.
  try {
    const QJsonObject configure = m_model.queryObject({{QStringLiteral("query"), QStringLiteral("configure")}});
    for (const QJsonValue& value : configure.value(QStringLiteral("options")).toArray()) {
      const QJsonObject o = value.toObject();
      Option option;
      option.name = o.value(QStringLiteral("name")).toString();
      option.text = o.value(QStringLiteral("kind")).toString() == QStringLiteral("text");
      const QJsonObject domain = o.value(QStringLiteral("domain")).toObject();
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
    // No configuration: user parameters only.
  }
  try {
    for (const QJsonValue& value : m_model.queryArray(QStringLiteral("parameters"))) {
      const QJsonObject p = value.toObject();
      const QString name = p.value(QStringLiteral("name")).toString();
      if (p.value(QStringLiteral("kind")).toString() != QStringLiteral("user") ||
          std::any_of(m_options.begin(), m_options.end(), [&name](const Option& o) { return o.name == name; })) {
        continue;
      }
      Option option;
      option.name = name;
      option.text = p.value(QStringLiteral("unit")).toString() == QStringLiteral("Text");
      const QJsonObject limits = p.value(QStringLiteral("limits")).toObject();
      const QJsonArray values = limits.value(QStringLiteral("values")).toArray();
      const QJsonArray texts = limits.value(QStringLiteral("values_text")).toArray();
      for (int i = 0; i < values.size(); ++i) {
        option.values << values[i].toString();
        option.shown << (i < texts.size() ? texts[i].toString() : unquoted(values[i].toString()));
      }
      m_options.append(option);
    }
  } catch (const std::exception& e) {
    qWarning().noquote() << QStringLiteral("Follow Option: no parameters: %1").arg(QString::fromUtf8(e.what()));
  }
  for (const Option& option : std::as_const(m_options)) {
    m_option->addItem(option.name, option.name);
  }
  if (m_options.isEmpty()) {
    m_text->setText(tr("The design has no user parameter to follow: add one in Change Parameters."));
  }
}

void FollowOptionDialog::optionChosen() {
  m_table->setRowCount(0);
  m_free.clear();
  const int index = m_option->currentIndex();
  if (index < 0 || index >= m_options.size()) {
    return;
  }
  const Option& option = m_options[index];
  for (int i = 0; i < option.values.size(); ++i) {
    addRow(option.values[i], option.shown[i], QString());
  }
  qDebug().noquote() << QStringLiteral("Follow Option values of %1: %2")
                            .arg(option.name, option.shown.join(QStringLiteral(", ")));
  QTimer::singleShot(0, this, &FollowOptionDialog::logPlaces);
}

void FollowOptionDialog::addRow(const QString& value, const QString& shown, const QString& result) {
  const QSignalBlocker blocker(m_table);
  const int row = m_table->rowCount();
  m_table->insertRow(row);
  auto* item = new QTableWidgetItem(shown);
  item->setData(Qt::UserRole, value);       // the value's expression
  item->setData(Qt::UserRole + 1, shown);   // as shown, to tell an edit
  m_table->setItem(row, 0, item);
  QComboBox* box = choiceBox(result);
  m_table->setCellWidget(row, 1, box);
  connect(box, &QComboBox::currentTextChanged, this, [this] {
    m_free.clear();
    showText(mapping().expression());
  });
}

QComboBox* FollowOptionDialog::choiceBox(const QString& result) const {
  auto* box = new QComboBox;
  box->setEditable(true);
  box->setCompleter(nullptr);
  box->setInsertPolicy(QComboBox::NoInsert);
  box->addItem(QString());
  for (const auto& choice : m_choices) {
    box->addItem(choice.second, choice.first);
  }
  const QString text = unquoted(result);
  const auto choice =
      std::find_if(m_choices.begin(), m_choices.end(), [&text](const auto& c) { return c.first == text; });
  box->setEditText(choice != m_choices.end() ? choice->second : text);
  return box;
}

QString FollowOptionDialog::resultExpression(const QComboBox* box) const {
  const QString text = box->currentText().trimmed();
  if (text.isEmpty()) {
    return QString();
  }
  for (const auto& choice : m_choices) {
    if (choice.second == text || choice.first == text) {
      return quotedText(choice.first);
    }
  }
  return quotedText(text);
}

QString FollowOptionDialog::valueExpression(int row) const {
  const QTableWidgetItem* item = m_table->item(row, 0);
  if (item == nullptr) {
    return QString();
  }
  const QString stored = item->data(Qt::UserRole).toString();
  if (!stored.isEmpty()) {
    return stored;
  }
  const QString text = item->text().trimmed();
  const int index = m_option->currentIndex();
  const bool asText = index >= 0 && index < m_options.size() && m_options[index].text;
  return text.isEmpty() ? QString() : asText ? quotedText(text) : text;
}

OptionMapping FollowOptionDialog::mapping() const {
  OptionMapping map;
  map.option = m_option->currentData().toString();
  for (int row = 0; row < m_table->rowCount(); ++row) {
    const auto* box = qobject_cast<const QComboBox*>(m_table->cellWidget(row, 1));
    const QString value = valueExpression(row);
    const QString result = box != nullptr ? resultExpression(box) : QString();
    if (!value.isEmpty() && !result.isEmpty()) {
      map.rows.append({value, result});
    }
  }
  map.otherwise = resultExpression(m_otherwise);
  if (map.otherwise.isEmpty() && !map.rows.isEmpty()) {
    map.otherwise = map.rows.front().second;
  }
  return map;
}

void FollowOptionDialog::openFormulaEditor() {
  const ParameterSource* source = parameterSourceOf(this);
  if (source == nullptr || !source->query) {
    return;
  }
  ParameterFieldOptions options;
  options.kind = ParameterKind::Text;
  options.anyKind = true;
  options.label = windowTitle();
  FormulaEditor editor(*source, options, expression(), this);
  if (execDialog(&editor) == QDialog::Accepted) {
    m_free = editor.expression().trimmed();
    showText(m_free);
    qDebug().noquote() << QStringLiteral("Follow Option from the Formula Editor: %1").arg(m_free);
  }
}

void FollowOptionDialog::showText(const QString& made) {
  if (m_options.isEmpty()) {
    m_ok->setEnabled(false);
    return;
  }
  const QString shown = !m_free.isEmpty() ? m_free : made;
  m_text->setText(shown.isEmpty() ? tr("Choose a result for at least one value.") : tr("Expression: %1").arg(shown));
  m_ok->setEnabled(!this->expression().isEmpty());
  qDebug().noquote() << QStringLiteral("Follow Option expression: %1")
                            .arg(this->expression().isEmpty() ? QStringLiteral("none") : this->expression());
}

void FollowOptionDialog::logPlaces() {
  if (!isVisible()) {
    return;
  }
  qDebug().noquote() << QStringLiteral("Follow Option option at %1").arg(placeOf(this, m_option));
  for (int row = 0; row < m_table->rowCount(); ++row) {
    const QRect cell = m_table->visualRect(m_table->model()->index(row, 0));
    const QPoint at = m_table->viewport()->mapToGlobal(cell.center()) -
                      (parentWidget() != nullptr ? parentWidget()->window() : this)->mapToGlobal(QPoint(0, 0));
    qDebug().noquote() << QStringLiteral("Follow Option value %1 at %2,%3").arg(row + 1).arg(at.x()).arg(at.y());
    if (const QWidget* box = m_table->cellWidget(row, 1)) {
      qDebug().noquote() << QStringLiteral("Follow Option result %1 at %2").arg(row + 1).arg(placeOf(this, box));
    }
  }
  qDebug().noquote() << QStringLiteral("Follow Option otherwise at %1").arg(placeOf(this, m_otherwise));
  qDebug().noquote() << QStringLiteral("Follow Option add at %1").arg(placeOf(this, m_add));
  qDebug().noquote() << QStringLiteral("Follow Option formula at %1").arg(placeOf(this, m_formula));
  qDebug().noquote() << QStringLiteral("Follow Option remove at %1").arg(placeOf(this, m_remove));
  qDebug().noquote() << QStringLiteral("Follow Option OK at %1").arg(placeOf(this, m_ok));
}

} // namespace mitcad
