// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
#include <QJsonObject>
#include <QString>

#include "ParameterField.hpp"

class QCheckBox;
class QLabel;
class QLineEdit;
class QPlainTextEdit;
class QTreeWidget;
class QTreeWidgetItem;
class QShowEvent;

namespace mitcad {

// Text is always the expression: templates and formatting only edit that text.
class FormulaEditor : public QDialog {
public:
  FormulaEditor(ParameterSource source, ParameterFieldOptions options, const QString& expression,
                QWidget* parent = nullptr);
  QString expression() const;
  bool limitsChanged() const;
  QJsonObject parameterCommand() const;
  void accept() override;
  QSize sizeHint() const override;

protected:
  void showEvent(QShowEvent* event) override;

private:
  void updateHelp();
  void filterFunctions(const QString& filter);
  void insertFunction(QTreeWidgetItem* item);
  void insertTemplate(int index);
  void indentLet();
  void inspectAt(int position, int end = -1);
  QJsonObject help(int position, int end = -1) const;

  ParameterSource m_source;
  ParameterFieldOptions m_options;
  QPlainTextEdit* m_expression = nullptr;
  QTreeWidget* m_functions = nullptr;
  QLabel* m_signature = nullptr;
  QLabel* m_value = nullptr;
  QLabel* m_part = nullptr;
  QLabel* m_error = nullptr;
  QLineEdit* m_min = nullptr;
  QLineEdit* m_max = nullptr;
  QLineEdit* m_step = nullptr;
  QCheckBox* m_strict = nullptr;
  QJsonObject m_originalLimits;
  bool m_canLimit = false;
};

QString formulaSignatureHtml(const QJsonObject& signature);
QString formulaKindName(ParameterKind kind);

} // namespace mitcad
