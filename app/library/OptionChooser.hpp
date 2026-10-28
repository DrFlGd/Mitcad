// SPDX-License-Identifier: MIT
#pragma once

// The choosers of a configurable component (mitcad#810; docs/configurations.md,
// "Part pickers and BUILD"; app/COMMANDS.md, "Options of library parts"):
// Insert from Library's size, the BUILD part picker's options and Edit
// Options… of an inserted component.
//
// - A closed table (and a table of the old form, `selectors`) is chosen by
//   its rows as before: a combo box per option of the values the rows with
//   the earlier choices have, or the rows by name.
// - An open table has a chooser per option, in the options' order: a combo
//   box for a list, a number field within the allowed range for a range or
//   a free number, a text field for a free text. Each change asks the
//   `configure` answer (PartOptions.hpp) with the values chosen so far: the
//   later options keep their values where the rules still allow them and
//   move where they do not, and the notice under the choosers names the
//   rule. A combination the rules refuse is not taken: the choosers go back
//   and the notice says why. A driven option (a formula) shows its value
//   and is not chosen here.
//
// Logged for the UI tests, with the prefix given (setLogName: "Library" by
// default): "<prefix> size <row>" (rows), "<prefix> options: Color = Black,
// Length = 800 mm (no row)", "<prefix> option Length allowed 300 mm … 1000
// mm, step 10 mm (Black is short)", "<prefix> option moves: Length 1200 mm
// → 1000 mm: Black is short" and "<prefix> option refused: <why>".

#include <utility>

#include <QJsonArray>
#include <QJsonObject>
#include <QList>
#include <QString>
#include <QStringList>
#include <QWidget>

#include "PartOptions.hpp"

class QComboBox;
class QFormLayout;
class QLabel;

namespace mitcad {

class OptionChooser : public QWidget {
  Q_OBJECT

public:
  explicit OptionChooser(QWidget* parent = nullptr);

  // How an open table's options are asked (Insert from Library: the
  // library's component; Edit Options: the inserted component; empty: the
  // rows only, as before mitcad#810). Before setTable.
  void setOptionsQuery(OptionsQuery query) { m_query = std::move(query); }
  // The log lines' prefix ("Library", "BUILD", "Edit Options").
  void setLogName(const QString& name) { m_logName = name; }
  // The table as library_show gives it (`options`, `closed`, `rows`,
  // `default`; or the old `selectors`); `row`: the row to show first (else
  // the default). An open table without a query shows its rows as a closed
  // one. `parameters`: an open table's options to start with, over the
  // row's ({"Length": "800 mm"}, as parameters() gives them).
  void setTable(const QJsonObject& configurations, const QString& row = QString(),
                const QJsonObject& parameters = QJsonObject());
  // The options the query gives with no values (an inserted component's
  // own, Edit Options); false when it gives none (the notice says why).
  bool showOptions();
  // Asks the options again with the values chosen (Edit Options: an option
  // bound to the parent or set free).
  void requery();
  // An option set to a value (a text, or a number's constant), as if
  // chosen; the later options follow. An option not chosen here (bound,
  // driven, unknown) asks again instead.
  void setOption(const QString& option, const QJsonValue& value);

  // The chosen row's name: a closed table's row, or the row an open
  // table's combination matches; empty for none.
  QString row() const { return m_row; }
  // Whether the options are chosen one by one (an open table).
  bool isOpen() const { return m_open; }
  // An open table's options as constant expressions for `parameters`
  // ({"Color": "'Black'", "Length": "800 mm"}), driven ones left out; empty
  // for a closed table.
  QJsonObject parameters() const;
  // The last answer (an open table's).
  const OptionsAnswer& answer() const { return m_answer; }
  // The fields in tab order (combo boxes, number and text fields).
  QList<QWidget*> fields() const { return m_fields; }
  // The rows' combo boxes (a closed table's).
  QList<QComboBox*> combos() const { return m_combos; }
  // "Color = Black, Length = 800 mm (no row)"; a closed table's row.
  QString summary() const;

signals:
  void rowChanged(const QString& row);
  // An open table's values changed (after the answer).
  void optionsChanged();

private:
  void clear();
  // A closed table: the rows' choosers.
  void setRows(const QJsonObject& configurations, const QString& row);
  void fill(int from);
  void choose();
  // An open table: asks with the values of `wanted` (option -> query value),
  // `changed` the index of the option the user changed (-1: none), keeping
  // the later options' values where they are allowed. False when the
  // answer refuses the values (the last ones stay).
  bool settle(const QJsonObject& wanted, int changed);
  void build();
  void showAnswer(const OptionsAnswer& shown);
  void optionEdited(int index, const QJsonValue& value);
  void say(const QString& text, bool problem);

  QFormLayout* m_form = nullptr;
  QLabel* m_notice = nullptr;
  QList<QComboBox*> m_combos;
  QList<QWidget*> m_fields;
  QStringList m_selectors;
  QJsonArray m_rows;
  QString m_row;
  bool m_filling = false;
  // An open table.
  bool m_open = false;
  OptionsQuery m_query;
  QString m_logName = QStringLiteral("Library");
  QStringList m_names;     // the options' names, as the widgets were made
  QStringList m_kinds;     // "list", "number" or "text" per option, as made
  OptionsAnswer m_answer;
  QJsonObject m_wanted;    // the values chosen (option -> query value)
};

} // namespace mitcad
