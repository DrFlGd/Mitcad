// SPDX-License-Identifier: MIT
#pragma once

#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#include <QByteArray>
#include <QHash>
#include <QJsonArray>
#include <QLineEdit>
#include <QList>
#include <QObject>
#include <QPointer>
#include <QSet>
#include <QStringList>
#include <QTimer>

#include <TopoDS_Shape.hxx>
#include <gp_Trsf.hxx>

#include "../OcctViewer.hpp"
#include "Command.hpp"
#include "CommandPanel.hpp"
#include "mitcad_bridge/lib.h"

namespace mitcad {

class ManipulatorOverlay;
class ParameterPicker;

// What a running command needs from the main window.
//
// The model computes on a worker thread (P7, docs/architecture.md): the
// calls that compute (commands, previews) run there as jobs and return when
// they are done, with a progress dialog whose Cancel rejects what they ran.
// Meanwhile nothing else may use the model: modelBusy() says so, and what
// comes from a timer or a queued signal then waits (whenIdle).
class CommandHost : public CommandContext {
public:
  struct AsyncResult {
    QJsonObject answer;
    QString error;
    bool cancelled = false;
    bool succeeded() const { return error.isEmpty() && !cancelled; }
  };
  using AsyncCompletion = std::function<void(const AsyncResult&)>;
  using InspectionTask = std::function<Inspection(const Document&)>;
  using InspectionCompletion = std::function<void(const Inspection&, const AsyncResult&)>;
  // Native presentation starts worker jobs and resumes on the UI thread.
  // Legacy hosts keep their existing synchronous command path.
  virtual bool asynchronousCommands() const { return false; }
  virtual bool modelCommandAsync(const QJsonObject&, QObject*, AsyncCompletion) { return false; }
  virtual bool modelPreviewAsync(const QByteArray&, const QString&, QObject*, AsyncCompletion) { return false; }
  virtual bool modelInspectionAsync(const QString&, QObject*, InspectionTask, InspectionCompletion) { return false; }
  virtual void cancelModelJob() {}
  virtual OcctViewer& viewer() = 0;
  // Runs a model command as it is and returns its result; throws
  // rust::Error when the model rejects it, ComputationCancelled when it was
  // cancelled, ModelBusy while another job computes the model.
  virtual QJsonObject modelCommand(const QJsonObject& command) = 0;
  // Evaluates an add_feature or edit_feature command without committing it
  // (the bridge's preview; `name` is the command's for the progress); throws
  // as modelCommand does. The previewed bodies are then previewBodyShape's.
  virtual QJsonObject modelPreview(const QByteArray& command, const QString& name) = 0;
  virtual void clearModelPreview() = 0;
  // A body after the previewed command, or null; the previewed feature's
  // tool body, or null.
  virtual std::shared_ptr<geometry::Shape> previewBodyShape(const std::string& uid) const = 0;
  virtual std::shared_ptr<geometry::Shape> previewToolShape() const = 0;
  // A job computes the model now (the caller waits for it in a nested event
  // loop): what a timer or a queued signal starts then must wait.
  virtual bool modelBusy() const = 0;
  // Calls `call` once no job computes the model (soon when none does now),
  // unless `context` is gone by then.
  virtual void whenIdle(QObject* context, std::function<void()> call) = 0;
  // Runs a model command; shows the error and returns false when the model
  // rejects it (or the user cancelled its computation: lastCancelled()).
  virtual bool runCommand(const QJsonObject& command, QJsonObject* result = nullptr) = 0;
  // Whether the last command that failed was cancelled rather than
  // rejected: it changed nothing, and is no error to show.
  virtual bool lastCancelled() const = 0;
  // Runs commands as one undo step named `label` and shows the result;
  // those before a rejected one are taken back.
  virtual bool runCommands(const QJsonArray& commands, const QString& label) = 0;
  // Shows the model as it is at the marker.
  virtual void refreshScene() = 0;
  // An input became active (none: an empty filter): shows what it can pick
  // that is hidden otherwise, such as the origin planes and axes.
  virtual void inputActivated(SelectFilter filter) = 0;
  // Construction planes, axes and points hidden in the view (not their
  // visibility in the design) while an input takes none of them
  // (InputDef::hidesConstruction, mitcad#394); false shows them as they
  // are again.
  virtual void setConstructionHidden(bool /*hidden*/) {}
  virtual void showHint(const QString& message) = 0;
  // Bodies at the marker.
  virtual QStringList bodyUids() const = 0;
  // The bodies as the view shows them: placed by their occurrences.
  virtual std::vector<BodyDisplay> modelBodies() const = 0;
  // Section Analysis: the bodies shown cut by a plane (a plane reference),
  // or whole (null).
  virtual void showSection(const QJsonValue& plane) = 0;
  virtual QJsonValue section() const = 0;
  // Soon (not inside the running command's call): cancels the command and
  // opens feature `uid` for editing, its input `input` taking the keyboard
  // (P6: a command whose inputs ask for another feature's edit).
  virtual void editInstead(const QString& uid, const QString& input) = 0;
  // The occurrence the view places a sketch's profiles by (empty in the
  // root component): profiles a command takes from elsewhere than a click
  // (the newest, a loaded definition) are highlighted where they are shown,
  // and a click on one is on the same entry (mitcad#199).
  virtual QString profileOccurrence(const QString& /*sketch*/) const { return QString(); }
  // The same for a body's faces, edges and vertices: the first occurrence of
  // the body's component (empty in the root). A loaded definition's face of
  // a placed component was otherwise highlighted unplaced, on another part.
  virtual QString bodyOccurrence(const QString& /*body*/) const { return QString(); }
  // The same for a datum (a construction plane, axis or point): the first
  // occurrence of its feature's component, where the view shows it; empty
  // for the origin's planes, axes and point, which the view draws unplaced.
  virtual QString datumOccurrence(const QString& /*datum*/) const { return QString(); }
  // The selection filter context the running command's selections pick
  // through besides their inputs' own tests (mitcad#217: Project in sketch
  // mode; framework/SelectionScope.hpp), null when there is none. Shift
  // widens a click to its empty rows, never beyond what the input takes.
  virtual const SelectionScope* commandScope() const { return nullptr; }
  // Parameter names picked from the view into a value field (mitcad#250):
  // the sketches the view shows (not one sketch mode edits), each with the
  // placement of the occurrence that places it, and the feature command
  // that edits a definition (the `feature` query's def; null: none).
  struct ShownSketch {
    QString uid;
    gp_Trsf placement;
  };
  virtual std::vector<ShownSketch> shownSketches() const { return {}; }
  virtual const CommandDef* featureEditor(const QJsonObject& /*def*/) const { return nullptr; }
};

// A feature command from start to OK or Cancel: its inputs' state and
// panel, the selections in the view, previews and the commit.
//
// Previews run a short while after the last change and are cached by the
// definition, so going back to an earlier value shows its shapes at once;
// the model caches the results too, so OK does not compute them again.
// Previews and OK compute on the model's worker thread (P7): a preview
// cancelled in the progress dialog shows nothing, and OK then computes the
// command itself; OK cancelled leaves the panel open and the model as it
// was.
//
// Editing rolls the timeline back to just before the feature: the view
// then shows what the feature starts from, and the preview adds the edited
// definition there. OK takes the roll-back away and edits the feature, so
// the edit is one undo step.
//
// Inputs are named by their keys in the state: an input's id, or
// "<list>.<row>.<id>" for the inputs of a list's rows.
class CommandSession : public QObject {
  Q_OBJECT

public:
  CommandSession(const CommandDef& def, CommandHost& host, const QString& editUid = QString(),
                 QObject* parent = nullptr);
  ~CommandSession() override;

  // Fills the inputs (from the feature when editing, else from defaults and
  // the selection made before) and shows the panel's state. False when the
  // command cannot run, with the reason in `error`.
  bool start(const Selection& preselection, QString& error);
  // Native edit startup rolls the marker back without a nested event loop.
  void startAsync(const Selection& preselection, std::function<void(bool, const QString&)> completion);
  CommandPanel* createPanel(QWidget* parent);
  // Attaches a frontend once after start(). The presenter may be destroyed
  // before the session; destruction automatically detaches its callbacks.
  void setPresenter(CommandPresenter& presenter);
  const CommandState& state() const { return m_state; }
  // Whether everything to pick was there when the panel opened: the
  // keyboard then goes to the first value, else the view keeps it.
  bool startsWithValues() const { return m_selectionsComplete; }
  const CommandDef& def() const { return m_def; }
  bool isEditing() const { return !m_editUid.isEmpty(); }
  // Before start: the session edits an analysis kept in the document (an
  // entry of the `analyses` query) instead of adding one (mitcad#41).
  void editAnalysis(const QJsonObject& analysis) { m_analysis = analysis; }
  // Whether the active input takes items of this kind.
  bool activeTakes(SelectKind kind) const;
  // For autosave (P8): the timeline marker the edit restores when it rolled
  // it back (else -1), and whether a sketch command's preview is applied to
  // the model now (its commands are taken back when the panel closes).
  int restoresMarker() const { return m_markerMoved ? m_originalMarker : -1; }
  bool previewApplied() const { return m_appliedDepth >= 0; }

  // A pick in the view goes to the active selection input.
  void picked(const Selection& items, Qt::KeyboardModifiers modifiers, bool window);
  // The host's selection filter context (CommandHost::commandScope)
  // changed: the active input picks through it as it is now (mitcad#217).
  void scopeChanged();
  // OK: adds or edits the feature; false (with the panel showing why) when
  // it cannot, or when its computation was cancelled.
  bool commit();
  void cancel();
  // Ends without changing the model: the document goes (another is opened).
  void discard();
  // Puts the keyboard into a value input (by its key) of the panel.
  bool focusValue(const QString& key);
  // A choice made outside the panel (mitcad#418: BUILD's part picker), as
  // if chosen in it: its onChange runs and the preview follows; or the
  // text of a text input (mitcad#519: BUILD's Openings window), as if
  // typed. False when the command has no such input `key` or has ended.
  bool choose(const QString& key, const QString& value);
  // A light bulb of the browser while the panel is open (mitcad#295;
  // CommandSessionVisibility.cpp): runs the visibility commands as one undo
  // step `label` outside the command's undo range. An applied sketch
  // preview is taken back before and applied again after; what takes the
  // range back later (Cancel, OK of an edit, an asynchronous checkpoint
  // restored) runs the changes again, so they stay as the user set them and
  // never go into the command's own undo step. The inputs stay selected;
  // the preview is drawn again over the bodies shown now (computed again
  // only when a body shown now was changed by the command and its result
  // was not drawn). False when the model refuses them, or while an
  // asynchronous command computes.
  bool runVisibility(const QJsonArray& commands, const QString& label);

  // Live MCP (mitcad#265, CommandSessionScript.cpp): an agent fills the
  // inputs by their keys through the panel's own actions, as typing,
  // choosing and checking would (a number or text for values and texts, a
  // choice's value, a boolean for checks and flips, an object for cut
  // targets); a selection input takes the items `read` makes of the JSON
  // array's entries. False with the reason when a key or value does not
  // fit; the inputs before it stay set.
  using ItemReader = std::function<std::optional<SelectionItem>(const QJsonValue&, QString&)>;
  bool scriptInputs(const QJsonObject& inputs, const ItemReader& read, QString& error);
  // A preview waits or computes, or OK or Cancel is under way.
  bool scriptBusy() const;
  // How often OK applied a command that repeats (CommandDef::repeats,
  // BUILD's Place): its panel stays open after each.
  int appliedCount() const { return m_applied; }
  // The inputs as they are, whether OK can run (`ready`, else `message`),
  // the last preview's outcome and the panel's messages (shownState).
  QJsonObject scriptState();

signals:
  void finished(bool committed);
  // A failed cancellation leaves the session open for another attempt.
  // Successful cancellation can retain earlier applications of a repeating command.
  void cancelFinished(bool succeeded);

protected:
  // Tab steps a choice with a cycle (InputDef::tabCycle, mitcad#375).
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  // An input as the panel shows it: one of the definition's, or an input
  // of a list's row.
  struct Slot {
    QString key;
    const InputDef* def = nullptr;
    QString list;
    int row = -1;
  };
  struct Preview {
    QJsonObject report;
    std::vector<BodyDisplay> bodies;
    TopoDS_Shape tool;
  };

  // Every input (shown only: those shown now) in the panel's order.
  QVector<Slot> slotsOf(bool shownOnly) const;
  std::optional<Slot> slotOf(const QString& key) const;
  CommandState scopeOf(const Slot& slot) const;
  bool isShown(const Slot& slot) const;
  const InputDef* activeInput() const;
  void activate(const QString& key);
  // What the view lets the input pick (with the host's selection filter
  // context, when it has one).
  void applyPickFilter(const InputDef* input);
  // What the active input takes now (InputDef::filterIn, mitcad#394), and
  // whether it takes an item now.
  SelectFilter activeKinds(const InputDef* input) const;
  bool acceptsNow(const InputDef* input, const SelectionItem& item) const;
  // Shows what it takes now: the origin's planes, construction geometry
  // hidden or not (InputDef::hidesConstruction), the pick places.
  void showActiveKinds(const InputDef* input);
  // After a change of the inputs: the active input takes what it takes
  // now.
  void followActiveKinds();
  SelectFilter m_activeFilter; // what the active input took when it was shown
  void activateFirstNeeded();
  void advanceIfFull();
  void selectionChanged(const QString& key);
  void logSelection(const QString& key) const;
  void evaluate(const QString& key);
  void setDefaults(const InputDef& input, const QString& key);
  void setRowDefaults();
  void inputsChanged();
  void inputAction(std::function<void()> action, bool invalidatesPreview = true,
                   std::function<void()> rejected = {});
  void structuralInputAction(const QString& key, std::function<void()> action);
  void presenterDetached();
  bool asyncPresenterAvailable();
  void resumePendingCommit();
  // `shapeOf` gives the shapes of the bodies the command made or changed
  // (by default the host's last preview: previewBodyShape).
  Preview previewFromReport(const QJsonObject& report,
                            const std::function<std::shared_ptr<geometry::Shape>(const std::string&)>& shapeOf = {});
  void acceptFeaturePreview(const QByteArray& key, const Built& built, const Preview& preview, bool cache);
  void commandSequenceAsync(const QJsonArray& commands, CommandHost::AsyncCompletion completion,
                            int index = 0, std::function<bool()> aborted = {});
  void undoToDepthAsync(int depth, CommandHost::AsyncCompletion completion, int guard = -1);
  void restoreCheckpointAsync(CommandHost::AsyncCompletion completion);
  void rollForwardAsync(CommandHost::AsyncCompletion completion);
  void showSketchPreviewAsync(const Built& built);
  bool commitAsync();
  void recoverFailedCommit(const CommandHost::AsyncResult& result);
  void cancelAsync();
  void finishAsyncCancel();
  void updateManipulators();
  void manipulatorDragged(const QString& key, double value);
  // A toggle in the view was clicked: its element's number goes in or out
  // of the text input (P9: a pattern's suppressed instances).
  void toggleElement(const QString& key, int element);
  void schedulePreview();
  void runPreview();
  bool previewReady(QString& message, bool& error);
  void showPreview(const Preview& preview, const QString& operation);
  // The preview's computation was cancelled (P7): nothing is shown, and OK
  // computes the command itself; a change of an input previews again.
  void showCancelledPreview(const QByteArray& key);
  void runInspection();
  void acceptInspection(const Inspection& result);
  void clearPreview();
  void updateHighlights();
  // The occurrences the inputs' items name as ghosts (InputDef::ghosts),
  // those of the active input left out, to the view (mitcad#147).
  void updateGhosts();
  bool isReferable(const SelectionItem& item) const;
  // Names the region a profile item means now (the model's `profile`
  // query), each item asked once per resolver: selection inputs keep a
  // region once however its key or occurrence is given (mitcad#199).
  ItemResolver resolver() const;
  // The selections filled from elsewhere than clicks (the definition
  // edited, the selection before the command, `init`) in their canonical
  // form: profiles placed where the view shows them, each region once.
  void canonicalizeSelections();
  void finish(bool committed);
  // False when the roll-back stays (its computation was cancelled).
  bool rollForward();
  // OK of an inspection that keeps an analysis in the document: adds it,
  // or changes the one edited; false when the model refuses it.
  bool keepAnalysis();
  // OK of an application command (CommandDef::accept, mitcad#234): does
  // its work; false (the panel stays) when it could not.
  bool acceptCommand();
  // OK of a command with its own model commands (CommandDef::apply,
  // mitcad#375): runs them; one that repeats stays open for the next,
  // another closes. False (the panel stays) when the model refused.
  bool applyCommand();
  // Whether a choice with a Tab cycle is shown (the filter is installed
  // only for such commands) and steps it; false when there is none.
  bool cycleChoice();
  int m_applied = 0; // OKs of a command that repeats
  // The box at the cursor of a value input with InputDef::cursorBox
  // (mitcad#375): made with the panel, moved with the mouse over the view,
  // its text in step with the input's (the report's "cursor" text while
  // the input has none of its own typed).
  void createCursorBox();
  void updateCursorBox();
  void moveCursorBox(const QPoint& at);
  // Which value input's box is at the cursor (the one whose handle was
  // dragged last, mitcad#444).
  void setCursorKey(const QString& key);
  QPointer<QLineEdit> m_cursorBox;
  QString m_cursorKey;
  QString m_loggedCursorBox;
  // OK of a command that makes a component in a root with modelled bodies
  // (mitcad#171): asks whether the root's features move into a component
  // first. False when the user cancels; `done` when the move did the
  // command's work (Create Components from Bodies). A new feature whose
  // references moved goes into their component (m_addTo).
  bool askBeforeComponent(const Built& built, bool& done);
  QJsonObject addFeatureCommand(const Built& built) const;
  QJsonObject bodyVolumes() const;
  void logVolumes(const QJsonObject& before) const;
  // Sketch commands: applied for the preview, taken back on a change.
  void showSketchPreview(const Built& built);
  void takeBackSketchPreview();
  int undoDepth() const;

  const CommandDef& m_def;
  CommandHost& m_host;
  QString m_editUid;
  QString m_addTo; // the component a new feature goes into (mitcad#171)
  bool m_deferStartRollback = false;
  QJsonObject m_startRollback;
  bool m_markerMoved = false; // the edit rolled the marker back
  int m_markerPosition = 0;   // where the edited feature is
  int m_originalMarker = 0;   // where the marker was before
  CommandState m_state;
  Selection m_previewHighlights;
  QPointer<CommandPanel> m_panel; // only the legacy Qt frontend
  CommandPresenter* m_presenter = nullptr;
  QPointer<ManipulatorOverlay> m_manipulators;
  QString m_active; // the key of the active selection input
  QSet<QString> m_shownSelections; // the selection inputs shown at the last change
  bool m_selectionsComplete = false;
  // Keyboard hand-over to the first value once the picks are done (mitcad#338).
  bool selectionsSatisfied() const;
  void moveKeyboardToValues();
  bool m_valueKeyboard = false; // a value field has the keyboard now
  bool m_valueTouched = false;  // the user put the keyboard in a field himself
  bool m_autoFocusing = false;  // the session itself is focusing a field
  QTimer m_previewTimer;
  bool m_previewOk = false;
  QJsonObject m_shownReport; // the report of the preview shown (toggles)
  QString m_loggedPreviewBodies; // the bodies the last preview logged (mitcad#266)
  bool m_failed = false; // the model could not compute the preview
  QByteArray m_shownKey; // the command whose preview is shown
  bool m_previewCancelled = false; // ... or whose preview was cancelled
  QHash<QByteArray, Preview> m_cache;
  QList<QByteArray> m_cacheOrder;
  bool m_done = false;
  quint64 m_generation = 0;
  bool m_asyncPending = false;
  bool m_commitRequested = false;
  bool m_committing = false;
  bool m_cancelRequested = false;
  QJsonValue m_checkpoint;
  bool m_finalizing = false;
  int m_appliedDepth = -1; // undo depth before an applied sketch preview
  QJsonValue m_sectionBefore; // the section shown before an inspection
  QJsonObject m_analysis; // the analysis edited (mitcad#41), else empty

  // Parameter names picked from the view (mitcad#250;
  // CommandSessionPicks.cpp): while a value field has the keyboard, the
  // visible sketches' dimensions and the earlier features' values are
  // labels in the view, and a click on one inserts its parameter's name
  // there. A click in the view takes the keyboard before the view sees it,
  // so the labels stay a moment after the field loses it.
  void valueFocused(const QString& key, bool focused);
  void showParameterLabels();
  void hideParameterLabels();
  void insertParameter(const QString& name);
  // The active input is a selection of several items that takes more.
  bool activeTakesMore() const;
  // A value field's tooltip when the feature owns its parameter, as a
  // sketch dimension's: "d3 = d1 / 2 → 20 mm (Sketch1 width)" (mitcad#250).
  void describeParameter(const QString& key, const QString& rename, const QString& text, const QString& shown);
  QString m_pickTarget;       // the value field that takes picked names
  bool m_pickFocused = false; // ... and has the keyboard now
  bool m_inserting = false;   // a name goes in now (the field takes the keyboard back)
  QPointer<ParameterPicker> m_picker;
  QHash<QString, QString> m_parameterComments; // by name, asked once

  // The hover of a tangent chain (mitcad#290; CommandSessionHover.cpp):
  // while the active input has one (InputDef::chainKinds) and its check is
  // on, the hover shows the rest of the chain of the edge or face under the
  // mouse. The chains of a body are asked once per command run (the
  // model's `tangent_chains` query) and looked up on each hover; a query
  // that fails leaves the hover on the one item.
  void hovered(const SelectionItem& item);
  void updateHoverChain();
  // The chain of the item (its names), or the item alone.
  QStringList hoverChain(const SelectionItem& item);
  SelectionItem m_hovered;
  // By "<body>/edges" or "<body>/faces": the chain of each member.
  QHash<QString, QHash<QString, QStringList>> m_chains;
  // The shown chain's shape (without the hovered item), by its item.
  QString m_chainKey;
  TopoDS_Shape m_chainShape;
  QString m_loggedHover; // the hover's last log line
  // The point under the mouse moved (CommandDef::hoverPreview, mitcad#417):
  // the command's preview again when the command says so.
  void hoverMoved(const SelectionItem& item);

  // Visibility changes while the panel is open (mitcad#295;
  // CommandSessionVisibility.cpp), in their order, to run again where the
  // command's undo range is taken back.
  struct VisibilityChange {
    QJsonArray commands;
    QString label;
  };
  QVector<VisibilityChange> m_visibility;
  int m_startDepth = -1; // the undo depth before an edit's roll-back (synchronous)
  // The bodies the view shows ("F2.b0@O1"), read after a change; none read
  // before the first one: every body counts as shown.
  std::optional<QSet<QString>> m_shownBodies;
  // Whether an input's item is highlighted: not on a body hidden meanwhile.
  bool onShownBody(const SelectionItem& item) const;
  // After a change: the preview again (applied again for a sketch command,
  // else drawn again), the highlights and the active input's pick places.
  void visibilityChanged(bool sketchPreviewTakenBack);
  // The preview shown again from its cached result over the bodies shown
  // now; false when that needs a body's result the cache does not have.
  bool redrawPreview();
  // Synchronous Cancel and OK of an edit: the roll-back and the changes
  // after it taken back to m_startDepth, the changes run again there.
  void settleVisibility();
  // Runs the recorded changes again from `index` (asynchronous), each as
  // one undo step; one the model refuses is skipped.
  void reapplyVisibilityAsync(int index, CommandHost::AsyncCompletion completion);
  void runVisibilityAsync(const QJsonArray& commands, const QString& label,
                          CommandHost::AsyncCompletion completion);

  // A new feature's operation chosen by the model (CommandDef::
  // autoOperation, mitcad#289; CommandSessionAuto.cpp): while it is on,
  // each preview asks for the choice ("auto_operation", commands.md) and
  // its answer sets the inputs "operation" and "objects"; the user's own
  // change of either turns it off for the rest of the command.
  void startAutoOperation();
  void addAutoOperation(QJsonObject& command) const;
  // True when the answer changed the inputs: their own preview follows
  // instead of the answer's.
  bool takeAutoOperation(const QJsonObject& report);
  void autoOperationOverruled(const QString& key);
  bool m_autoOperation = false;
};

} // namespace mitcad
