// SPDX-License-Identifier: MIT
#pragma once

#include <QJsonObject>
#include <QJsonValue>
#include <QString>
#include <QVector>
#include <QWidget>

#include "ConfigurationsData.hpp"

class QComboBox;
class QGridLayout;
class QLabel;
class QLineEdit;
class QSlider;

namespace mitcad {

class DocumentHost;

// The Configuration bar at the top of the browser (mitcad#807;
// docs/configurations.md, "Active configuration"): the options of the
// design's configuration chosen without opening the Configurations window.
// A row preset combo, then a chooser per option in choice order: a combo
// for a list, a field and a slider for a range (a field alone when a bound
// is missing or the option is free), the value alone for a driven option.
// The choosers offer only what the rules allow now (the `configure` query);
// a choice is the `configure` command, a row `apply_configuration`, each one
// undo step, and the cascade's moves are shown in a notice under the
// choosers (and in the status bar). Hidden while the design has no
// options and no rows.
//
// Logged for UI tests: "Configuration bar: Color = White (Black | White);
// Length = 1200 mm (300 mm … 1000 mm, step 10 mm); row none" when it
// changes; the places "Configuration bar row at x,y", "Configuration bar
// <option> at x,y" (its combo or field) and "Configuration bar <option>
// slider at x,y" in the main window's coordinates; "Configuration chosen:
// Color = Black", "Configuration moves: <the notice>", "Configuration
// refused: <why>".
class ConfigurationBar : public QWidget {
  Q_OBJECT

public:
  explicit ConfigurationBar(DocumentHost& host, QWidget* parent = nullptr);

  // Reads the model's options again (after any change of the design).
  void refresh();
  void logLayout();

private:
  struct Chooser {
    QString option;
    QComboBox* combo = nullptr;
    QLineEdit* field = nullptr;
    QSlider* slider = nullptr;
    QLabel* label = nullptr; // a driven option's value
    int steps = 0;           // the slider's steps over the allowed range
  };

  void rebuild();
  void showValues();
  // A value chosen for an option, and a row chosen: the commands, run once
  // the widget's own handling is over and no job computes the model.
  void choose(const QString& option, const QJsonValue& value, const QString& shown);
  void chooseRow(const QString& row);
  void run(const QJsonObject& command, const QString& what);
  void showNotice(const QString& text, bool error);
  double sliderValue(const Chooser& chooser, int position) const;
  int sliderPosition(const Chooser& chooser, double value) const;

  DocumentHost& m_host;
  QGridLayout* m_grid = nullptr;
  QLabel* m_title = nullptr;
  QComboBox* m_row = nullptr;
  QLabel* m_notice = nullptr;
  QVector<Chooser> m_choosers;
  QVector<QWidget*> m_owned; // the choosers' widgets, replaced by rebuild
  configurations::Table m_table;
  configurations::Report m_report;
  QString m_structure; // what the widgets were built for
  QString m_loggedState;
  QString m_logged;
};

} // namespace mitcad
