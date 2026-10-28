// SPDX-License-Identifier: MIT
#pragma once

#include <functional>
#include <vector>

#include <QJsonArray>
#include <QJsonObject>
#include <QString>
#include <QStringList>
#include <QWidget>

#include "../framework/ParameterField.hpp"
#include "RuleBlocks.hpp"

class QLabel;
class QPlainTextEdit;
class QPushButton;
class QTreeWidget;

namespace mitcad::rules {

class RuleCanvas;
class RulePalette;

// What the Rules page asks of the design (the app talks to the model only
// through JSON commands and queries, core/model-api/src/commands.md
// "Configurations: options, rows and rules"). The Configurations window
// gives one made from its DocumentHost (makeConfigurationRulesPage); the
// unit tests give one of their own.
struct RulesSource {
  // The `configurations` query's answer: the table (with its `options` and
  // `rules`) and `problems`.
  std::function<QJsonObject()> configurations;
  // The `configure` query's answer, for which options are numbers; may be
  // left out.
  std::function<QJsonObject()> configure;
  // `set_configurations` with the table as it is and these rules: one undo
  // step; false with the model's reason in `error` when it is refused.
  std::function<bool(const QJsonArray& rules, QString* error)> setRules;
  // The `check_rules` query's answer (`{"error": …}` when it failed).
  std::function<QJsonObject()> checkRules;
  // The design's tables and their columns (`formula_help`'s `tables`:
  // [{name, columns: [{name, unit}]}]), for the "combination in table"
  // block (mitcad#849); may be left out.
  std::function<QJsonArray()> tables;
  // The design's Undo and Redo, for the page's Ctrl+Z once no change of
  // its own waits to be saved; may be left out.
  std::function<void()> undo;
  std::function<void()> redo;
  // The Formula Editor's (ParameterField.hpp); without `parameters` a
  // formula block is edited in place.
  ParameterSource parameters;
};

// The Rules tab of the Configurations window (mitcad#808): the list of
// rules (name, enabled check, problem mark; Add, Duplicate, Delete), the
// palette and the block canvas of the selected rule, View as Text, and
// Check Rules with its report.
//
// Every edit is saved at once as the table's new rules (`setRules`, one
// undo step), when every rule is complete. An edit that leaves an empty
// socket (a compare just put in, a new else) or that the model refuses is
// kept on the page, marked in the list, until an edit completes it; Ctrl+Z
// takes such edits back one by one, then the design's own steps.
class RulesPage : public QWidget {
  Q_OBJECT

public:
  explicit RulesPage(RulesSource source, QWidget* parent = nullptr);

  // Reads the table again (after any change of the design); the changes
  // not saved yet stay.
  void refresh();

  void addRule();
  void duplicateRule();
  void deleteRule();
  void checkRules();
  void setTextShown(bool shown);
  bool selectRule(const QString& name);
  // Takes back the last change not saved, else the design's last step.
  bool undo();

  // For tests.
  RuleCanvas* canvas() const { return m_canvas; }
  RulePalette* rulePalette() const { return m_palette; }
  QTreeWidget* ruleList() const { return m_list; }
  QPlainTextEdit* textView() const { return m_text; }
  QPlainTextEdit* report() const { return m_report; }
  const std::vector<Rule>& rules() const { return m_rules; }
  bool hasUnsaved() const { return !m_drafts.empty(); }
  // A rule's mark in the list: why it is not saved or what is wrong with
  // it; empty when nothing is.
  QString problemOf(int index) const;
  void logPlaces();

signals:
  // The rules were saved to the design.
  void saved();

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void showEvent(QShowEvent* event) override;
  void resizeEvent(QResizeEvent* event) override;

private:
  void commit(std::vector<Rule> rules, const QString& what);
  void fillList();
  void showCurrent();
  void updateText();
  void updateStatus();
  void listEdited(int index);
  // "<base> <n>", the first n from `first` that no rule has.
  QString uniqueName(const QString& base, int first = 1) const;

  RulesSource m_source;
  QJsonObject m_table;
  QStringList m_problems;
  std::vector<Rule> m_rules;
  // The rules before each change not saved yet: the first is as saved.
  std::vector<std::vector<Rule>> m_drafts;
  QString m_refusal; // why the model refused the last change
  QJsonArray m_shownRules; // the model's rules as last read
  QStringList m_shownProblems;
  int m_current = -1;
  bool m_building = false;

  QTreeWidget* m_list = nullptr;
  QPushButton* m_add = nullptr;
  QPushButton* m_duplicate = nullptr;
  QPushButton* m_delete = nullptr;
  QPushButton* m_viewText = nullptr;
  QPushButton* m_check = nullptr;
  RulePalette* m_palette = nullptr;
  RuleCanvas* m_canvas = nullptr;
  QLabel* m_status = nullptr;
  QPlainTextEdit* m_text = nullptr;
  QPlainTextEdit* m_report = nullptr;
  QString m_loggedRows;
  QString m_loggedText;
  QString m_loggedPlaces;
};

} // namespace mitcad::rules
