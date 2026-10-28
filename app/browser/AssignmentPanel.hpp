// SPDX-License-Identifier: MIT
#pragma once

#include <functional>

#include <QIcon>
#include <QJsonArray>
#include <QJsonObject>
#include <QPair>
#include <QString>
#include <QStringList>
#include <QVector>
#include <QWidget>

class QComboBox;
class QLabel;
class QPushButton;
class QResizeEvent;
class QShowEvent;
class QTableWidget;

namespace mitcad {

class CommandContext;
struct OptionMapping;

// The assignment panel of the Appearances and the Materials windows
// (mitcad#861): what the targets (bodies, faces) have and whether it varies
// with an option, edited where it is assigned. One widget for both windows.
//
// - A header line: the title naming the targets (`Assign to Body1`,
//   `Assign to 3 Bodies`, `Assign to 2 Faces`), what they have now (`Now:
//   Steel - Satin`), **Varies with** (None, the configuration's options from
//   the `configure` query, the other user parameters with a list of allowed
//   values; with none of them, **Add Option…** opens the Configurations
//   window) and an fx mark while the targets are bound. The windows are
//   taller by this line, so their lists keep their height.
// - None: the header alone; the window's Assign (or a double-click in its
//   library) sets the value as before.
// - An option: a table of a row per value (its value, the result's swatch
//   and name), prefilled with what the targets have now, so nothing changes
//   until a row does; the design's value now reads `Black (now)`, the
//   selected row is the one the next pick fills. An Other row (the
//   `switch()`'s default) only for an option whose values are not a closed
//   list; Formula Editor… at the table's right edge. The window's pick
//   (Assign, a double-click, New, Duplicate, a texture's Use) fills the
//   selected rows (fill()); every fill writes the whole `switch()` at once,
//   one command per target and one undo step, and the next row becomes
//   active; after the last row none is, until a row is clicked.
// - None again after a binding keeps the result of the value now as the
//   value set by hand (one undo step).
// - An expression of another form shows `Custom: <expression>` with Formula
//   Editor… and Replace with a table; it is never rewritten silently.
// - Targets in different states show `Mixed: …` above the table, which shows
//   the first target's state; the first change writes it to all of them.
class AssignmentPanel : public QWidget {
  Q_OBJECT

public:
  enum class Kind { Appearance, Material };

  // A result as the window shows it: its id (empty when the text names
  // none), name and swatch.
  struct Result {
    QString id;
    QString name;
    QIcon icon;
  };

  // What the panel asks of its window.
  struct Host {
    std::function<const CommandContext&()> model;
    // One command (its own undo step), and several as one undo step.
    std::function<bool(const QJsonObject&)> runCommand;
    std::function<bool(const QJsonArray&, const QString&)> runCommands;
    std::function<QString()> lastError;
    // Whether the model can change now; the window says why not.
    std::function<bool()> canChange;
    // The result an id or a name gives.
    std::function<Result(const QString&)> describe;
    // Add Option…: the Configurations window.
    std::function<void()> openConfigurations;
  };

  // `logName` heads the log lines ("Appearances", "Materials").
  AssignmentPanel(Kind kind, const QString& logName, Host host, QWidget* parent = nullptr);
  ~AssignmentPanel() override;

  // The bodies and the faces (body, face name) the window assigns to; none
  // hides the panel.
  void setTargets(const QStringList& bodies, const QVector<QPair<QString, QString>>& faces);
  // Reads the targets' state again (the model changed); a choice of an
  // option not written yet stays.
  void reload();
  // Whether a table of values is shown: the window's pick fills its rows.
  bool varies() const;
  // The window's Assign while varies(): `Assign to row Red`, `Assign to 2
  // Rows`, with the targets when there are several (`Assign to row Red (3
  // bodies)`); `Assign` while no row is selected.
  QString assignText() const;
  // Whether a row is selected for the next pick (varies() only); Assign's
  // tooltip then.
  bool canFill() const;
  QString assignTip() const;
  // Fills the selected rows (else the active one) with `result`, an
  // expression's text ('paint_black'), and writes the expression to every
  // target; `before` runs first in the same undo step (a library's
  // material copied into the design). False when nothing was written.
  bool fill(const QString& result, const QJsonArray& before = {});

signals:
  // The active row or the selection changed: the window's Assign text.
  void assignTextChanged();

protected:
  void showEvent(QShowEvent* event) override;
  void resizeEvent(QResizeEvent* event) override;

private:
  struct Target {
    QString body;
    QString face; // empty: the body itself
    QString name;
    QString expression; // what it follows; empty: a value set by hand
    QString value;      // the id it has now (what it shows)
  };
  struct Option {
    QString name;
    bool text = false;
    bool closed = false; // a list of values: no Other row
    QStringList values;  // as expressions ('Black', 800 mm)
    QStringList shown;   // as the rows show them (Black, 800 mm)
    QString currentExpression;
    QString currentText;
  };
  struct Row {
    QString value; // as an expression; empty for Other
    QString shown;
    QString result; // as an expression ('paint_black'); empty: none
    bool other = false;
  };

  const CommandContext& model() const { return m_host.model(); }
  void loadOptions();
  void readTargets();
  QString stateOf(const Target& target) const;
  bool mixed() const;
  QString mixedText() const;
  // The panel from the first target's state.
  void rebuildFromState();
  // The rows of m_option, from `mapping` where it gives a value's result,
  // else prefilled with what the first target has now.
  void buildRows(const OptionMapping* mapping);
  const Option* option(const QString& name) const;
  bool isCurrent(int row) const;
  QString rowsExpression() const;
  QString rowKey(int row) const;
  int rowOfKey(const QString& key) const;
  void showState();
  void showRows();
  void fillCombo();
  void optionChosen(int index);
  // Writes `expression` for every target, one undo step.
  bool write(const QString& expression, const QJsonArray& before = {});
  // None again: the result of the design's value now set by hand for every
  // target.
  void backToNone();
  void openFormulaEditor();
  void replaceWithTable();
  QString title() const;
  QString targetsLabel() const;
  // "3 bodies", "2 faces", "3 items"; "both bodies", "all 3 bodies".
  QString targetsCount() const;
  QString allTargets() const;
  // "Body1", "Body1 and Body2", "Body1, Body2 and Body3".
  static QString joinNames(const QStringList& names);
  void scheduleLogPlaces();
  void logPlaces();

  Kind m_kind;
  QString m_log;
  Host m_host;
  QVector<Target> m_targets;
  QVector<Option> m_options;
  // The state shown: an option (with its rows), an expression of another
  // form, or None (both empty).
  QString m_option;
  QString m_custom;
  QVector<Row> m_rows;
  // A closed list's default kept from the expression read (no Other row).
  QString m_keptOtherwise;
  // An option chosen (or a table asked for) but not written yet.
  bool m_pending = false;
  QString m_activeKey;
  // The last row was filled: no row is active until one is clicked.
  bool m_noActive = false;
  bool m_building = false;
  QLabel* m_title = nullptr;
  QLabel* m_nowLabel = nullptr; // "Now:", the swatch and the name
  QLabel* m_nowSwatch = nullptr;
  QLabel* m_now = nullptr;
  QComboBox* m_varies = nullptr;
  QLabel* m_fx = nullptr;
  QWidget* m_mixedBox = nullptr; // the tinted notice with its icon
  QLabel* m_mixed = nullptr;
  QTableWidget* m_table = nullptr;
  QLabel* m_customText = nullptr;
  QPushButton* m_formula = nullptr;
  QPushButton* m_replace = nullptr;
  QLabel* m_message = nullptr;
  QString m_loggedState;
  QString m_loggedPlaces;
};

} // namespace mitcad
