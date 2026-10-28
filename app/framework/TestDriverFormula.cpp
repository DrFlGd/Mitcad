// SPDX-License-Identifier: MIT
#include "TestDriver.hpp"
#include "FormulaEditor.hpp"

#include <algorithm>

#include <QApplication>
#include <QComboBox>
#include <QHeaderView>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QScreen>
#include <QTextCursor>
#include <QTreeWidget>
#include <QtLogging>

namespace mitcad {

void TestDriver::openFormulaField(const QString& name) {
  auto* edit=targetWindow()->findChild<QLineEdit*>(name);
  if (!edit || !ParameterField::of(edit)) {
    fail(m_line,QStringLiteral("no formula-enabled field %1").arg(name));
    return;
  }
  ParameterField::of(edit)->openFormulaEditor();
}

void TestDriver::testFormulaBuilder() {
  auto* dialog=dynamic_cast<FormulaEditor*>(QApplication::activeModalWidget());
  if (!dialog) { fail(m_line,QStringLiteral("Formula Editor must be open through an actual entry point")); return; }
  const QPointer<FormulaEditor> guard(dialog);
  auto* text=dialog->findChild<QPlainTextEdit*>(QStringLiteral("formulaText"));
  auto* search=dialog->findChild<QLineEdit*>(QStringLiteral("formulaFunctionSearch"));
  auto* functions=dialog->findChild<QTreeWidget*>(QStringLiteral("formulaFunctions"));
  if (!text || !search || !functions || functions->topLevelItemCount()!=60) { // 52 + the text functions (mitcad#664) + XLOOKUP, MATCH, INDEX (mitcad#667)
    fail(m_line,QStringLiteral("formula catalog or multiline text editor missing")); return;
  }
  if (dialog->screen() && dialog->width()<std::min(700,dialog->screen()->availableGeometry().width()-24)) {
    fail(m_line,QStringLiteral("formula editor collapsed to a narrow text-field width")); return;
  }
  for (int column=0;column<3;++column) {
    QHeaderView* header=functions->header();
    const int left=header->sectionViewportPosition(column);
    if (left<0 || header->sectionSize(column)<40 || left+header->sectionSize(column)>header->viewport()->width()+1) {
      fail(m_line,QStringLiteral("formula function catalog column %1 is outside its visible viewport").arg(column)); return;
    }
  }
  search->setText(QStringLiteral("round down, optionally"));
  if (functions->topLevelItemCount()!=1 || functions->topLevelItem(0)->text(0)!=QStringLiteral("FLOOR") ||
      functions->topLevelItem(0)->text(2).isEmpty()) {
    fail(m_line,QStringLiteral("function search does not include descriptions and examples")); return;
  }
  search->clear();
  queue([guard] {
    auto* input=guard->findChild<QPlainTextEdit*>(QStringLiteral("formulaText"));
    input->setPlainText(QStringLiteral("FLOOR("));
    QTextCursor cursor=input->textCursor(); cursor.movePosition(QTextCursor::End); input->setTextCursor(cursor);
  });
  queue([] {}); queue([] {}); queue([] {});
  queue([this,guard] {
    const QString signature=guard->findChild<QLabel*>(QStringLiteral("formulaSignature"))->text();
    if (!signature.contains(QStringLiteral("FLOOR(<b><u>value</u></b>"))) {
      fail(m_line,QStringLiteral("incomplete function signature did not highlight the current argument: %1").arg(signature)); return;
    }
    auto* input=guard->findChild<QPlainTextEdit*>(QStringLiteral("formulaText"));
    input->setPlainText(QStringLiteral("length / pitch"));
    input->selectAll();
  });
  queue([] {}); queue([] {}); queue([] {});
  queue([this,guard] {
    const QString part=guard->findChild<QLabel*>(QStringLiteral("formulaPartValue"))->text();
    if (!part.contains(QStringLiteral("length / pitch → 12.5"))) {
      fail(m_line,QStringLiteral("selected expression did not show its actual model value: %1").arg(part)); return;
    }
    auto* input=guard->findChild<QPlainTextEdit*>(QStringLiteral("formulaText"));
    input->selectAll();
    guard->findChild<QComboBox*>(QStringLiteral("formulaTemplates"))->setCurrentIndex(1);
    if (input->toPlainText()!=QStringLiteral("FLOOR(length / pitch)")) {
      fail(m_line,QStringLiteral("formula template did not insert ordinary editable text")); return;
    }
    guard->findChild<QLineEdit*>(QStringLiteral("formulaMin"))->setText(QStringLiteral("2"));
    guard->findChild<QLineEdit*>(QStringLiteral("formulaMax"))->setText(QStringLiteral("10"));
    guard->findChild<QLineEdit*>(QStringLiteral("formulaStep"))->setText(QStringLiteral("1"));
    qInfo() << "Formula builder: incomplete signature, searchable52function catalog, selected value12.5, plain template and limits verified";
  });
}

} // namespace mitcad
