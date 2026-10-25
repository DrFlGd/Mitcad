// SPDX-License-Identifier: MIT
#include <cstdio>

#include <QComboBox>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QTreeWidget>

#include "framework/FormulaEditor.hpp"

namespace {
int failures=0;
void check(bool value,const char* expression,int line) {
  if (!value) { std::fprintf(stderr,"formula_editor.cpp:%d: %s\n",line,expression); ++failures; }
}
#define CHECK(value) check((value),#value,__LINE__)
}

int formulaEditorTests() {
  using namespace mitcad;
  int mutations=0;
  ParameterSource source;
  source.parameters=[] { return QJsonArray{QJsonObject{{QStringLiteral("name"),QStringLiteral("count")},
      {QStringLiteral("kind"),QStringLiteral("user")}, {QStringLiteral("limits"),QJsonObject{{QStringLiteral("min"),QStringLiteral("2")}}}}}; };
  source.addParameter=[&mutations](const QJsonObject&,QString*) { ++mutations; return true; };
  source.query=[](const QJsonObject&) {
    return QJsonObject{{QStringLiteral("functions"),QJsonArray{
        QJsonObject{{QStringLiteral("name"),QStringLiteral("FLOOR")},{QStringLiteral("description"),QStringLiteral("Round down")},{QStringLiteral("example"),QStringLiteral("FLOOR(4.5)")}},
        QJsonObject{{QStringLiteral("name"),QStringLiteral("SIN")},{QStringLiteral("description"),QStringLiteral("Sine")},{QStringLiteral("example"),QStringLiteral("SIN(30 deg)")}}}}};
  };
  ParameterFieldOptions options;
  options.own=QStringLiteral("count");
  options.kind=ParameterKind::Unitless;
  FormulaEditor editor(source,options,QStringLiteral("count + 1"));
  auto* text=editor.findChild<QPlainTextEdit*>(QStringLiteral("formulaText"));
  auto* search=editor.findChild<QLineEdit*>(QStringLiteral("formulaFunctionSearch"));
  auto* functions=editor.findChild<QTreeWidget*>(QStringLiteral("formulaFunctions"));
  auto* templates=editor.findChild<QComboBox*>(QStringLiteral("formulaTemplates"));
  auto* minimum=editor.findChild<QLineEdit*>(QStringLiteral("formulaMin"));
  auto* maximum=editor.findChild<QLineEdit*>(QStringLiteral("formulaMax"));
  CHECK(text && search && functions && templates && minimum && maximum);
  if (!text || !search || !functions || !templates || !minimum || !maximum) return failures;
  CHECK(functions->topLevelItemCount()==2);
  search->setText(QStringLiteral("round down"));
  CHECK(functions->topLevelItemCount()==1);
  CHECK(functions->topLevelItem(0)->text(0)==QStringLiteral("FLOOR"));
  CHECK(functions->topLevelItem(0)->text(2)==QStringLiteral("FLOOR(4.5)"));
  CHECK(editor.expression()==QStringLiteral("count + 1"));
  text->selectAll();
  templates->setCurrentIndex(1);
  CHECK(editor.expression()==QStringLiteral("FLOOR(length / pitch)"));
  CHECK(minimum->isEnabled() && minimum->text()==QStringLiteral("2"));
  maximum->setText(QStringLiteral("20"));
  CHECK(editor.limitsChanged());
  const QJsonObject command=editor.parameterCommand();
  CHECK(command.value(QStringLiteral("name")).toString()==QStringLiteral("count"));
  CHECK(command.value(QStringLiteral("max")).toString()==QStringLiteral("20"));
  CHECK(command.value(QStringLiteral("step")).isNull());
  editor.reject();
  CHECK(mutations==0); // Opening and canceling only edit local formula text.
  source.limitProblem=[](const QJsonObject&) { return QStringLiteral("Held by the open feature editor"); };
  FormulaEditor held(source,options,QStringLiteral("5"));
  CHECK(!held.findChild<QLineEdit*>(QStringLiteral("formulaMin"))->isEnabled());
  CHECK(!held.findChild<QLineEdit*>(QStringLiteral("formulaMax"))->isEnabled());
  const QString signature=formulaSignatureHtml({{QStringLiteral("name"),QStringLiteral("FLOOR")},
      {QStringLiteral("arguments"),QJsonArray{QStringLiteral("value"),QStringLiteral("[step]")}},
      {QStringLiteral("argument"),1}});
  CHECK(signature==QStringLiteral("FLOOR(value; <b><u>[step]</u></b>)"));
  return failures;
}
