// SPDX-License-Identifier: MIT
#pragma once

#include <functional>
#include <optional>

#include <QByteArray>
#include <QDialog>
#include <QSet>
#include <QString>
#include <QVector>

#include "DocumentSnapshot.hpp"
#include "FindInDesign.hpp"

class QCheckBox;
class QComboBox;
class QLabel;
class QLineEdit;
class QPushButton;
class QTableView;
class QTimer;

namespace mitcad {

class FindModel;

// Find in Design (mitcad#350): a tool window beside the browser that finds
// components (occurrences) and bodies by name on the model's snapshot of
// names, and lists them with their place in the assembly. The text takes
// `*` and `?` as wildcards; Components, Bodies and Visible / Hidden narrow
// it. The results follow the text as it is typed (after a short pause, so
// that large designs stay responsive) and the model as it changes.
// A text with a comparison searches property values instead of names
// (`FireRating = EI60`, `Pset_WallCommon/FireRating = EI60`, `Mass > 10
// kg`; mitcad#496): the model finds them.
// Selecting rows selects them in the view and the browser (selectionPicked);
// the buttons act on the selected rows, or on all when none is selected:
// Show, Hide, Appearance..., Select. What they do is the
// BrowserController's; this window only finds and lists.
class FindDialog : public QDialog {
  Q_OBJECT

public:
  explicit FindDialog(QWidget* parent = nullptr);
  ~FindDialog() override;

  // The design as it is now; the results are found again.
  void setSnapshot(const DocumentSnapshot& snapshot);
  // The selected results, or all when none is selected.
  QVector<FindResult> targets() const;
  // The selected results only.
  QVector<FindResult> selected() const;
  // The result row last made current (double-clicked), if any.
  bool current(FindResult* result) const;
  // Puts the text in the search field and searches at once.
  void setText(const QString& text);
  // Focuses the search field.
  void focusSearch();
  // How a property search (`FireRating = EI60`, mitcad#496) asks the model:
  // the uids of the components, occurrences and bodies that match, or none
  // and why.
  using PropertySearch = std::function<std::optional<QSet<QString>>(const QString& text, QString* error)>;
  void setPropertySearch(PropertySearch search);

signals:
  void showRequested();
  void hideRequested();
  void appearanceRequested();
  void selectRequested();
  // The selected rows changed (by the user).
  void selectionPicked();
  // A row was double-clicked: reveal it in the browser.
  void revealRequested();

protected:
  void showEvent(QShowEvent* event) override;
  void resizeEvent(QResizeEvent* event) override;
  void moveEvent(QMoveEvent* event) override;

private:
  void search();
  void updateButtons();
  QString countText() const;
  // Logs where the controls are (and the rows in sight), for UI tests.
  void logLayout();

  FindModel* m_model = nullptr;
  QLineEdit* m_text = nullptr;
  QCheckBox* m_components = nullptr;
  QCheckBox* m_bodies = nullptr;
  QComboBox* m_visibility = nullptr;
  QTableView* m_table = nullptr;
  QLabel* m_count = nullptr;
  QPushButton* m_show = nullptr;
  QPushButton* m_hide = nullptr;
  QPushButton* m_appearance = nullptr;
  QPushButton* m_select = nullptr;
  QPushButton* m_all = nullptr;
  QPushButton* m_none = nullptr;
  QPushButton* m_close = nullptr;
  QTimer* m_timer = nullptr;
  DocumentSnapshot m_snapshot;
  bool m_haveSnapshot = false;
  bool m_updating = false;
  QString m_logged;
  PropertySearch m_propertySearch;
};

} // namespace mitcad
