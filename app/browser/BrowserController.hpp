// SPDX-License-Identifier: MIT
#pragma once

#include <QObject>
#include <QPoint>
#include <QPointer>
#include <QString>
#include <QStringList>

#include "../framework/HybridAssembly.hpp"
#include "BrowserPanel.hpp"
#include "DocumentSnapshot.hpp"

class QToolButton;
class QWidget;

namespace mitcad {

class AppearanceDialog;
class DocumentHost;
class ParametersDialog;
class TimelineWidget;

// What the browser, the timeline, the parameters dialog and the status
// bar's list of failed features do (U3): their menus and the model commands
// behind them, the history marker's drags as one undo step, and keeping
// them in step with the model (refresh) and the selection (showSelection).
class BrowserController : public QObject {
  Q_OBJECT

public:
  BrowserController(DocumentHost& host, QWidget* window);

  BrowserPanel* browser() const { return m_browser; }
  TimelineWidget* timeline() const { return m_timeline; }
  bool markerDragActive() const { return m_markerDepth >= 0; }
  // The status bar's summary of failed features; a click goes to them.
  QToolButton* failures() const { return m_failures; }
  // Beside it, yellow: features with warnings, the import's fallbacks,
  // partial items and placeholders among them (mitcad#170); a click goes
  // to them.
  QToolButton* warnings() const { return m_warnings; }

  // After the model or the origin's visibility changed.
  void refresh(const DocumentSnapshot& snapshot);
  // The selection outside commands changed (in the view or here).
  void showSelection(const Selection& items);
  // Closes an open rename editor; false when none was open.
  bool cancelEditing();
  // Change Parameters.
  void openParameters();
  // Edit Appearances (mitcad#46); Assign gives the targets (bodies, and
  // faces: mitcad#53) the selected appearance.
  void openAppearances(const Selection& targets);
  // Find in Browser for an item picked in the view (a face finds its body).
  void findInBrowser(const SelectionItem& item);

private:
  // Connects a signal of the browser or the timeline to a slot, queued:
  // the slot may change the model, which rebuilds the widget that emitted
  // it. A call that comes while a job computes the model (P7: posted before
  // the job began, or the end of a drag that the progress dialog cut short)
  // waits until the job is done; a menu is not shown then (dropWhenBusy).
  template <typename Sender, typename... Args>
  void follow(Sender* sender, void (Sender::*signal)(Args...), void (BrowserController::*slot)(Args...),
              bool dropWhenBusy = false);
  void pickItems(const Selection& items);
  // Rows picked in the browser: the items of the rows, and the features of
  // the feature rows under sketches (mitcad#177). Their features are
  // selected in the timeline too.
  void pickRows(const Selection& items, const QStringList& features);
  // Selects the items; the browser shows the rows of these features too.
  void pick(const Selection& items, const QStringList& features);
  // What selecting features selects: their sketches, datums or bodies, or
  // the features themselves (a command that takes features; one that
  // made no body).
  Selection featureItems(const QStringList& uids) const;
  void editFeature(const QString& uid);
  void markerDragged(int position);
  void browserMenu(const BrowserNode& node, const QPoint& at);
  void timelineMenu(const QString& uid, const QPoint& at);
  // A feature's menu, the same in the timeline and on its rows under
  // sketches in the browser (mitcad#177): `row` is the browser's row, or
  // none in the timeline.
  void featureMenu(const QString& uid, const QPoint& at, const BrowserNode& row);
  void findInTimeline(const QString& uid);
  // The timeline's search was cleared (mitcad#179): the selection from
  // before it again.
  void restoreSearchSelection();
  // A slow second click on a browser row (mitcad#177).
  void renameClicked(const BrowserNode& node);
  // Timeline groups (P9): a group's menu, and grouping features.
  void groupMenu(const QString& name, const QPoint& at);
  void groupFeatures(const QStringList& uids);
  void browserDoubleClicked(const BrowserNode& node);
  void toggleVisibility(const BrowserNode& node);
  void activate(const BrowserNode& node);
  void rename(const BrowserNode& node, const QString& name);
  void renameFeature(const QString& uid, const QString& name);
  void deleteNode(const BrowserNode& node);
  void deleteFeature(const QString& uid);
  // Opens Flat Pattern on a sheet-metal body: its settings, or a new one
  // (mitcad#210).
  void editFlatPattern(const QString& body);
  void featuresClicked(const QStringList& uids);
  void reorder(const QString& uid, int index);
  void setMarker(int position);
  void markerDragFinished(int position);
  void newComponent(const BrowserNode& node);
  // Hybrid assemblies (mitcad#171): the question before a component is made
  // in a root with modelled bodies, and Move to Component.
  HybridModel hybridModel();
  void componentsFromBodies(const BrowserNode& node);
  void moveToComponent(const QJsonObject& selection);
  void moveHistory(const QString& uid);
  void paste(bool asNew);
  void changeUnits();
  void reveal(const QString& uid);
  void updateFailures();
  bool canChange(bool visibilityOnly = false);
  const DocumentSnapshot::Occurrence* occurrence(const QString& path) const;
  int undoDepth() const;

  DocumentHost& m_host;
  QWidget* m_window;
  BrowserPanel* m_browser = nullptr;
  TimelineWidget* m_timeline = nullptr;
  QToolButton* m_failures = nullptr;
  QToolButton* m_warnings = nullptr;
  QPointer<ParametersDialog> m_parameters;
  QPointer<AppearanceDialog> m_appearances;
  DocumentSnapshot m_snapshot;
  bool m_shown = false;      // something was shown
  bool m_originShown = false; // as the browser shows it
  QString m_clipboard;        // a copied occurrence's path
  QString m_clipboardName;
  int m_markerDepth = -1; // undo depth when a marker drag began
  int m_markerStart = 0;
  // While a pick of features runs: the features whose browser rows show it.
  bool m_picking = false;
  QStringList m_pickedFeatures;
  // The selection shown in the browser, and the features whose rows show it.
  Selection m_shownItems;
  QStringList m_shownFeatures;
  // What was selected when the timeline's search began (mitcad#179).
  Selection m_searchItems;
  QStringList m_searchFeatures;
  QString m_loggedFailures;
  QString m_loggedWarnings;
  QString m_loggedBadge; // the warnings badge (mitcad#170)
  QString m_loggedDof;
  QString m_loggedJoints;
};

} // namespace mitcad
