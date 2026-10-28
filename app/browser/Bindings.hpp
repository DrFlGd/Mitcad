// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
#include <QIcon>
#include <QJsonArray>
#include <QPair>
#include <QString>
#include <QStringList>
#include <QVector>

#include "../framework/Command.hpp"

class QComboBox;
class QLabel;
class QLineEdit;
class QPushButton;
class QShowEvent;
class QTableWidget;

namespace mitcad {

// Bindings (mitcad#809; the model's mitcad#800, commands.md "Bindings:
// values from expressions"): a body's appearance and material, a face's
// appearance, a feature's suppression and the visibility of a body or an
// occurrence follow an expression of parameters. The context menus open the
// dialogs below; the browser and the timeline mark a bound item with an fx
// badge, the expression in its tooltip.

// An icon with a small "fx" in its lower right corner: the item follows an
// expression.
QIcon boundIcon(const QIcon& icon);

// A text as an expression's text literal: 'It''s'.
QString quotedText(const QString& text);

// A mapping from an option's values to results, as Follow Option writes it:
// switch(Option; v1; r1; v2; r2; ...; otherwise). Each value and result is
// an expression text ('Black', 800 mm, 'paint_black').
struct OptionMapping {
  QString option;
  QVector<QPair<QString, QString>> rows;
  QString otherwise;

  QString expression() const;
  // The mapping of an expression of that form; false for any other form
  // (the Formula Editor edits those).
  static bool parse(const QString& source, OptionMapping* mapping);
};

// Suppress When… and Show When…: a condition (an expression field with the
// parameters' suggestions and the Formula Editor). OK sets it (an empty
// field removes the condition and keeps the value it gave); Remove
// Condition removes it.
class ConditionDialog : public QDialog {
  Q_OBJECT

public:
  // `what` is the dialog's title ("Suppress Extrude2 When"), `help` the line
  // above the field.
  ConditionDialog(const QString& what, const QString& help, const QString& initial, QWidget* parent = nullptr);

  // The condition to set; empty: remove it.
  QString condition() const;

protected:
  void showEvent(QShowEvent* event) override;

private:
  void openFormulaEditor();
  void logPlaces();

  QLineEdit* m_condition = nullptr;
  QPushButton* m_formula = nullptr;
  QPushButton* m_remove = nullptr;
  QPushButton* m_ok = nullptr;
  bool m_removed = false;
};

// Appearance or Material > Follow Option…: a table from an option's values
// to appearances or materials, written as switch(). An option is a
// parameter of the configuration (the `configure` query, mitcad#798), else
// any user parameter; its allowed values fill the table, and values can be
// added by hand. A result is chosen from the design's appearances or
// materials, or typed (an id or a name: the model warns about one it does
// not know). Formula Editor… edits the expression as text, which is how an
// expression of another form is edited.
class FollowOptionDialog : public QDialog {
  Q_OBJECT

public:
  enum class Kind { Appearance, Material };

  FollowOptionDialog(const CommandContext& model, Kind kind, const QString& what, const QString& current,
                     QWidget* parent = nullptr);

  // The expression to set; empty: remove the binding (the value it gave
  // stays).
  QString expression() const;

protected:
  void showEvent(QShowEvent* event) override;

private:
  struct Option {
    QString name;
    bool text = false;
    QStringList values; // as expression texts ('Black', 800 mm)
    QStringList shown;  // as the table shows them (Black, 800 mm)
  };

  void loadOptions();
  void loadChoices();
  void optionChosen();
  void addRow(const QString& value, const QString& shown, const QString& result);
  QComboBox* choiceBox(const QString& result) const;
  // A result's expression: the id of the choice named so, else the text.
  QString resultExpression(const QComboBox* box) const;
  // A value's expression as typed in the table.
  QString valueExpression(int row) const;
  OptionMapping mapping() const;
  void openFormulaEditor();
  void showText(const QString& made);
  void logPlaces();

  const CommandContext& m_model;
  Kind m_kind;
  QVector<Option> m_options;
  // The choices: (id, name).
  QVector<QPair<QString, QString>> m_choices;
  QComboBox* m_option = nullptr;
  QTableWidget* m_table = nullptr;
  QComboBox* m_otherwise = nullptr;
  QLabel* m_text = nullptr;
  QPushButton* m_add = nullptr;
  QPushButton* m_removeRow = nullptr;
  QPushButton* m_formula = nullptr;
  QPushButton* m_remove = nullptr;
  QPushButton* m_ok = nullptr;
  // An expression of another form, from the Formula Editor or as it was:
  // set as it is while the table is not changed.
  QString m_free;
  bool m_removed = false;
};

} // namespace mitcad
