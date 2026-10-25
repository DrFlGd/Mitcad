// SPDX-License-Identifier: MIT
#include "FormulaEditor.hpp"

#include <algorithm>
#include <exception>
#include <functional>

#include <QCheckBox>
#include <QComboBox>
#include <QColor>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QHeaderView>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QMouseEvent>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QRegularExpression>
#include <QShowEvent>
#include <QTextBlock>
#include <QTextCursor>
#include <QTextEdit>
#include <QTimer>
#include <QTreeWidget>
#include <QVBoxLayout>
#include <QtLogging>

#include "Dialogs.hpp"
#include "FitToScreen.hpp"
#include "Theme.hpp"

namespace mitcad {
namespace {

class FormulaText : public QPlainTextEdit {
public:
  std::function<void(int)> hovered;
  explicit FormulaText(QWidget* parent = nullptr) : QPlainTextEdit(parent) {
    viewport()->setMouseTracking(true);
    setMouseTracking(true);
  }
protected:
  void mouseMoveEvent(QMouseEvent* event) override {
    QPlainTextEdit::mouseMoveEvent(event);
    if (hovered) hovered(cursorForPosition(event->position().toPoint()).position());
  }
};

QString limitText(const QJsonObject& limits, const char* name) {
  return limits.value(QLatin1String(name)).toString();
}

} // namespace

QString formulaKindName(ParameterKind kind) {
  return kind == ParameterKind::Angle ? QStringLiteral("angle") :
         kind == ParameterKind::Unitless ? QStringLiteral("unitless") : QStringLiteral("length");
}

QString formulaSignatureHtml(const QJsonObject& signature) {
  if (signature.isEmpty()) return {};
  const QJsonArray arguments = signature.value(QStringLiteral("arguments")).toArray();
  const int active = std::min(signature.value(QStringLiteral("argument")).toInt(), int(arguments.size()) - 1);
  QStringList text;
  for (int i = 0; i < arguments.size(); ++i) {
    const QString argument = arguments.at(i).toString().toHtmlEscaped();
    text << (i == active ? QStringLiteral("<b><u>%1</u></b>").arg(argument) : argument);
  }
  return QStringLiteral("%1(%2)").arg(signature.value(QStringLiteral("name")).toString().toHtmlEscaped(),
                                       text.join(QStringLiteral("; ")));
}

FormulaEditor::FormulaEditor(ParameterSource source, ParameterFieldOptions options, const QString& expression,
                             QWidget* parent)
    : QDialog(parent), m_source(std::move(source)), m_options(std::move(options)) {
  setWindowTitle(tr("Formula Editor"));
  setObjectName(QStringLiteral("formulaEditor"));
  auto* layout = new QVBoxLayout(this);
  auto* about = new QLabel(tr("The formula text is the value. Insert a function or template, then edit it freely."));
  about->setWordWrap(true);
  layout->addWidget(about);
  auto* search = new QLineEdit;
  search->setObjectName(QStringLiteral("formulaFunctionSearch"));
  search->setPlaceholderText(tr("Search functions, descriptions and examples"));
  layout->addWidget(search);
  m_functions = new QTreeWidget;
  m_functions->setObjectName(QStringLiteral("formulaFunctions"));
  m_functions->setColumnCount(3);
  m_functions->setHeaderLabels({tr("Function"), tr("Description"), tr("Example")});
  m_functions->setRootIsDecorated(false);
  m_functions->setMaximumHeight(150);
  m_functions->header()->setStretchLastSection(false);
  m_functions->header()->setSectionResizeMode(0,QHeaderView::ResizeToContents);
  m_functions->header()->setSectionResizeMode(1,QHeaderView::Stretch);
  m_functions->header()->setSectionResizeMode(2,QHeaderView::Stretch);
  layout->addWidget(m_functions);
  auto* actions = new QHBoxLayout;
  auto* templates = new QComboBox;
  templates->setObjectName(QStringLiteral("formulaTemplates"));
  templates->addItems({tr("Choose a template"), tr("How many fit"), tr("Spacing for N items"),
                       tr("Even count"), tr("Odd count"), tr("Round to a step"), tr("Keep within limits")});
  actions->addWidget(templates, 1);
  auto* indent = new QPushButton(tr("Indent LET"));
  indent->setObjectName(QStringLiteral("formulaIndent"));
  actions->addWidget(indent);
  layout->addLayout(actions);
  auto* editor = new FormulaText;
  m_expression = editor;
  m_expression->setObjectName(QStringLiteral("formulaText"));
  m_expression->setAccessibleName(tr("Formula text"));
  m_expression->setPlainText(expression);
  m_expression->setMinimumHeight(110);
  m_expression->setLineWrapMode(QPlainTextEdit::WidgetWidth);
  layout->addWidget(m_expression, 1);
  m_signature = new QLabel;
  m_signature->setObjectName(QStringLiteral("formulaSignature"));
  m_signature->setTextFormat(Qt::RichText);
  m_signature->setWordWrap(true);
  m_value = new QLabel;
  m_value->setObjectName(QStringLiteral("formulaValue"));
  m_part = new QLabel;
  m_part->setObjectName(QStringLiteral("formulaPartValue"));
  m_part->setWordWrap(true);
  m_error = new QLabel;
  m_error->setObjectName(QStringLiteral("formulaError"));
  m_error->setWordWrap(true);
  setErrorStyleSheet(m_error);
  for (QLabel* label : {m_signature, m_value, m_part, m_error}) layout->addWidget(label);
  auto* limits = new QFormLayout;
  m_min = new QLineEdit;
  m_min->setObjectName(QStringLiteral("formulaMin"));
  m_max = new QLineEdit;
  m_max->setObjectName(QStringLiteral("formulaMax"));
  m_step = new QLineEdit;
  m_step->setObjectName(QStringLiteral("formulaStep"));
  m_strict = new QCheckBox(tr("Refuse values outside the limits"));
  m_strict->setObjectName(QStringLiteral("formulaStrict"));
  QString limitProblem;
  try {
    if (!m_options.own.isEmpty() && m_source.parameters) {
      for (const QJsonValue& value : m_source.parameters()) {
        const QJsonObject parameter = value.toObject();
        if (parameter.value(QStringLiteral("name")).toString() == m_options.own &&
            parameter.value(QStringLiteral("kind")).toString() != QStringLiteral("reference")) {
          m_canLimit = true;
          m_originalLimits = parameter.value(QStringLiteral("limits")).toObject();
          if (m_source.limitProblem) {
            limitProblem=m_source.limitProblem(parameter);
            m_canLimit=limitProblem.isEmpty();
          }
          break;
        }
      }
    }
  } catch (const std::exception&) {}
  m_min->setText(limitText(m_originalLimits, "min"));
  m_max->setText(limitText(m_originalLimits, "max"));
  m_step->setText(limitText(m_originalLimits, "step"));
  m_strict->setChecked(m_originalLimits.value(QStringLiteral("strict")).toBool());
  for (QLineEdit* field : {m_min, m_max, m_step}) field->setEnabled(m_canLimit);
  m_strict->setEnabled(m_canLimit);
  limits->addRow(tr("Minimum:"), m_min);
  limits->addRow(tr("Maximum:"), m_max);
  limits->addRow(tr("Step:"), m_step);
  limits->addRow(m_strict);
  if (!m_canLimit) {
    auto* hint = new QLabel(limitProblem.isEmpty()
        ? tr("Limits belong to a saved parameter. Create one or select one in Change Parameters.") : limitProblem);
    hint->setWordWrap(true);
    limits->addRow(hint);
  }
  layout->addLayout(limits);
  auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
  layout->addWidget(buttons);
  connect(buttons, &QDialogButtonBox::accepted, this, &FormulaEditor::accept);
  connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
  connect(search, &QLineEdit::textChanged, this, &FormulaEditor::filterFunctions);
  connect(m_functions, &QTreeWidget::itemDoubleClicked, this, [this](QTreeWidgetItem* item) { insertFunction(item); });
  connect(templates, &QComboBox::currentIndexChanged, this, &FormulaEditor::insertTemplate);
  connect(indent, &QPushButton::clicked, this, &FormulaEditor::indentLet);
  auto* debounce = new QTimer(this);
  debounce->setSingleShot(true);
  debounce->setInterval(120);
  connect(m_expression, &QPlainTextEdit::textChanged, debounce, [debounce] { debounce->start(); });
  connect(m_expression, &QPlainTextEdit::cursorPositionChanged, debounce, [debounce] { debounce->start(); });
  connect(debounce, &QTimer::timeout, this, &FormulaEditor::updateHelp);
  editor->hovered = [this](int at) { inspectAt(at); };
  filterFunctions({});
  updateHelp();
  fitWindowToContents(this);
  m_expression->setFocus();
}

QString FormulaEditor::expression() const { return m_expression->toPlainText().trimmed(); }

QSize FormulaEditor::sizeHint() const {
  // A multiline editor needs a reading width of its own. The fitting helper
  // caps this preference to the available screen instead of shrinking to a
  // word-wrapped label's narrow natural hint.
  return QSize(760,720).expandedTo(minimumSizeHint());
}

void FormulaEditor::showEvent(QShowEvent* event) {
  QDialog::showEvent(event);
  fitWindowToContents(this);
}

bool FormulaEditor::limitsChanged() const {
  return m_canLimit && (m_min->text().trimmed() != limitText(m_originalLimits,"min") ||
      m_max->text().trimmed() != limitText(m_originalLimits,"max") ||
      m_step->text().trimmed() != limitText(m_originalLimits,"step") ||
      m_strict->isChecked() != m_originalLimits.value(QStringLiteral("strict")).toBool());
}

QJsonObject FormulaEditor::parameterCommand() const {
  QJsonObject command{{QStringLiteral("cmd"),QStringLiteral("set_parameter")},
                      {QStringLiteral("name"),m_options.own}, {QStringLiteral("expression"),expression()}};
  for (const auto& field : {qMakePair(QStringLiteral("min"),m_min),qMakePair(QStringLiteral("max"),m_max),
                           qMakePair(QStringLiteral("step"),m_step)}) {
    command.insert(field.first,field.second->text().trimmed().isEmpty() ? QJsonValue() : QJsonValue(field.second->text().trimmed()));
  }
  command.insert(QStringLiteral("strict"),m_strict->isChecked());
  return command;
}

QJsonObject FormulaEditor::help(int position, int end) const {
  if (!m_source.query) return {};
  const QString text = m_expression->toPlainText();
  QJsonObject query{{QStringLiteral("query"),QStringLiteral("formula_help")}, {QStringLiteral("expression"),text},
                    {QStringLiteral("cursor"),int(text.left(position).toUtf8().size())},
                    {QStringLiteral("kind"),formulaKindName(m_options.kind)}};
  if (end >= 0) query.insert(QStringLiteral("selection_end"),int(text.left(end).toUtf8().size()));
  if (m_canLimit) query.insert(QStringLiteral("parameter"),m_options.own);
  return m_source.query(query);
}

void FormulaEditor::updateHelp() {
  try {
    const QTextCursor cursor=m_expression->textCursor();
    const QJsonObject value=help(cursor.selectionStart(),cursor.selectionEnd());
    m_signature->setText(formulaSignatureHtml(value.value(QStringLiteral("signature")).toObject()));
    m_value->setText(tr("Value: %1").arg(value.value(QStringLiteral("result")).toObject().value(QStringLiteral("text")).toString()));
    const QJsonObject error=value.value(QStringLiteral("error")).toObject();
    m_error->setText(error.value(QStringLiteral("message")).toString());
    QList<QTextEdit::ExtraSelection> markings;
    if (!error.isEmpty()) {
      const QByteArray bytes=m_expression->toPlainText().toUtf8();
      QTextEdit::ExtraSelection mark;
      mark.cursor=m_expression->textCursor();
      mark.cursor.setPosition(int(QString::fromUtf8(bytes.left(error.value(QStringLiteral("start")).toInt())).size()));
      mark.cursor.setPosition(int(QString::fromUtf8(bytes.left(error.value(QStringLiteral("end")).toInt())).size()),QTextCursor::KeepAnchor);
      mark.format.setUnderlineStyle(QTextCharFormat::WaveUnderline);
      mark.format.setUnderlineColor(QColor(0xd0,0x36,0x36));
      markings << mark;
    }
    m_expression->setExtraSelections(markings);
    inspectAt(cursor.selectionStart(),cursor.selectionEnd());
    qDebug().noquote() << QStringLiteral("Formula signature: %1").arg(m_signature->text());
  } catch (const std::exception& error) {
    m_error->setText(QString::fromUtf8(error.what()));
    m_signature->clear();
    m_value->clear();
    m_part->clear();
    m_expression->setExtraSelections({});
  }
}

void FormulaEditor::inspectAt(int position,int end) {
  try {
    const QJsonObject part=help(position,end).value(QStringLiteral("selection")).toObject();
    const QByteArray bytes=m_expression->toPlainText().toUtf8();
    const QString source=QString::fromUtf8(bytes.mid(part.value(QStringLiteral("start")).toInt(),
        part.value(QStringLiteral("end")).toInt()-part.value(QStringLiteral("start")).toInt()));
    const QString value=part.value(QStringLiteral("error")).toString(part.value(QStringLiteral("text")).toString());
    m_part->setText(part.isEmpty() ? QString() : QStringLiteral("%1 → %2").arg(source,value));
    qDebug().noquote() << QStringLiteral("Formula part: %1").arg(m_part->text());
  } catch (const std::exception&) { m_part->clear(); }
}

void FormulaEditor::filterFunctions(const QString& filter) {
  m_functions->clear();
  if (!m_source.query) return;
  try {
    const QJsonArray functions=m_source.query({{QStringLiteral("query"),QStringLiteral("formula_help")}})
        .value(QStringLiteral("functions")).toArray();
    for (const QJsonValue& value:functions) {
      const QJsonObject function=value.toObject();
      const QString name=function.value(QStringLiteral("name")).toString();
      const QString description=function.value(QStringLiteral("description")).toString();
      const QString example=function.value(QStringLiteral("example")).toString();
      if (!(name+description+example).contains(filter,Qt::CaseInsensitive)) continue;
      auto* row=new QTreeWidgetItem(m_functions,{name,description,example});
      row->setData(0,Qt::UserRole,name);
      row->setToolTip(0,description+QStringLiteral("\n")+example);
      row->setToolTip(1,description);
      row->setToolTip(2,example);
    }
  } catch (const std::exception&) {}
}

void FormulaEditor::insertFunction(QTreeWidgetItem* item) {
  if (!item) return;
  QTextCursor cursor=m_expression->textCursor();
  cursor.insertText(item->data(0,Qt::UserRole).toString()+QStringLiteral("()"));
  cursor.movePosition(QTextCursor::Left);
  m_expression->setTextCursor(cursor);
  m_expression->setFocus();
}

void FormulaEditor::insertTemplate(int index) {
  static const QStringList templates{QString(),QStringLiteral("FLOOR(length / pitch)"),
      QStringLiteral("length / (count - 1)"),QStringLiteral("EVEN(count)"),QStringLiteral("ODD(count)"),
      QStringLiteral("MROUND(value; step)")};
  if (index==6) { m_min->setFocus(); return; }
  if (index<=0 || index>=templates.size()) return;
  m_expression->textCursor().insertText(templates.at(index));
  m_expression->setFocus();
  qDebug().noquote() << QStringLiteral("Formula template: %1").arg(templates.at(index));
}

void FormulaEditor::indentLet() {
  try {
    const QJsonObject result=help(m_expression->textCursor().position());
    if (result.contains(QStringLiteral("formatted"))) {
      m_expression->setPlainText(result.value(QStringLiteral("formatted")).toString());
    }
  } catch (const std::exception& error) { m_error->setText(QString::fromUtf8(error.what())); }
}

void FormulaEditor::accept() {
  try {
    const QJsonObject value=help(m_expression->textCursor().position());
    if (value.contains(QStringLiteral("error"))) {
      m_error->setText(value.value(QStringLiteral("error")).toObject().value(QStringLiteral("message")).toString());
      return;
    }
    if (expression().isEmpty()) { m_error->setText(tr("Enter a formula.")); return; }
    QDialog::accept();
  } catch (const std::exception& error) { m_error->setText(QString::fromUtf8(error.what())); }
}

} // namespace mitcad
