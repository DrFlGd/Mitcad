// SPDX-License-Identifier: MIT
#pragma once

#include <deque>
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include <QByteArrayView>
#include <QDialog>
#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QMainWindow>
#include <QPointer>
#include <QString>
#include <QStringList>

#include <gp_Trsf.hxx>

#include "OcctViewer.hpp"
#include "browser/BrowserPanel.hpp"
#include "browser/DocumentHost.hpp"
#include "browser/DocumentSnapshot.hpp"
#include "files/ProjectDialogs.hpp"
#include "files/Projects.hpp"
#include "framework/CommandSession.hpp"
#include "framework/DialogFolders.hpp"
#include "framework/DocumentSession.hpp"
#include "framework/ModelWorker.hpp"
#include "mitcad/geometry/shape.hpp"
#include "mitcad_bridge/lib.h"
#include "projects/ProjectPicker.hpp"
#include "rust/cxx.h"
#include "sketch/SketchController.hpp"

class QAction;
class QCloseEvent;
class QDockWidget;
class QDragEnterEvent;
class QDropEvent;
class QElapsedTimer;
class QLabel;
class QMenu;
class QStackedWidget;

namespace mitcad {

class AutosaveManager;
class AutoselectPanel;
class BrowserController;
class CommandRegistry;
class CommandSearch;
class ComputeProgress;
class F3dImport;
class FloatingLayout;
class GlassCard;
class LockController;
class ProjectIndicator;
class ProjectsPanel;
class RemoteController;
class LiveController;
class ModeBanner;
class Ribbon;
class SelectionFilterPanel;
class TitleBar;
class UpdateController;
class ReportCenter;
class ViewController;
#ifdef MITCAD_MACOS_NATIVE_UI
namespace mac {
class NativeMacWindow;
}
using NativeMacWindow = mac::NativeMacWindow;
class NativeCommandForm;
class NativeWidgetView;
#endif
struct BrowserNode;
struct FileVersion;
struct RecoverableSession;

namespace sketch {
class SketchPalette;
}

// The model is driven with JSON commands and queries (see
// core/model/src/api/commands.md); geometry comes as Shape handles.
// Commands are defined declaratively (app/COMMANDS.md) and run by
// CommandSession; sketch mode is SketchController's; the browser, the
// timeline and the parameters dialog are BrowserController's (U3). The
// window holds the document, the selection and the modes.
class MainWindow : public QMainWindow, public SketchHost, public DocumentHost {
  Q_OBJECT

public:
  // With demo set, starts with a dimensioned block instead of an empty part.
  explicit MainWindow(bool demo = false, QWidget* parent = nullptr, bool nativeUi = false);
  ~MainWindow() override;

  // Presents the selected frontend; the Qt host is the embedded viewport
  // root when AppKit owns the visible window.
  void present();
  bool usesNativeUi() const { return m_nativeUi; }
  // Logs the window's size and where its bars, docks and central widget
  // are ("Window layout: ..."), which every UI test's clicks and
  // screenshots depend on (tools/ui-layout.sh, mitcad#412).
  void logLayout() const;

  OcctViewer* viewerWidget() const { return m_viewer; }
  DocumentSession& documentSession() const { return *m_documentSession; }

  // Live MCP access runs on the UI thread and follows the same model and
  // edit-lock workflows as this window. A replacement invalidates its ID.
  QString mcpDocumentId() const { return m_mcpDocumentId; }
  QJsonObject mcpDocumentInfo() const;
  QJsonObject mcpQuery(const QJsonObject& request) const;
  QJsonObject mcpCommand(const QJsonObject& request);
  // One call of the live catalog's tools on this window and its document
  // (mitcad#265; MainWindowMcp.cpp, MainWindowMcpUi.cpp): document_list,
  // model_query, model_command, document_save and the ui_* tools. Throws
  // mcp::Refusal (busy, with when to retry) or another std::exception with
  // the reason when it is refused.
  QJsonObject mcpTool(const QString& name, const QJsonObject& arguments);
  // The agents connected through MCP (initialized sessions): the project
  // indicator says "Agent connected" and its menu offers Disconnect Agent,
  // which calls `disconnectAll`.
  void setMcpAgents(int count, std::function<void()> disconnectAll);

  // Sets a parameter's value (mm or rad, written in its unit), as typed in
  // the parameters dialog (--set).
  bool setParameterValue(const QString& name, double value);

  // Replaces the document with a file without asking about unsaved
  // changes: a design, or any file Import takes as a new document (an
  // .f3d design is imported with its history after the window shows;
  // U6). On failure, returns false with the reason in error; when the
  // user cancelled opening a design, error is empty.
  bool openFile(const QString& path, QString& error);

  // Opens a file the system asks for (Finder, the Dock, `open -a`) the way
  // File > Open does, after asking about unsaved changes, and brings the
  // window to the front.
  void openExternalFile(const QString& path);

  // Offers the unsaved work that sessions which did not end normally left
  // in the recovery folder (P8c), if there is any; an .f3d design opened at
  // start-up comes in first.
  void recoverAtStart();

  // A job computes the model on the worker thread (P7); the document is
  // the worker's until it is done. (CommandHost and DocumentHost.)
  bool modelBusy() const override { return m_documentSession->busy(); }
  void whenIdle(QObject* context, std::function<void()> call) override;
  bool lastCancelled() const override { return m_lastCancelled; }
#ifdef MITCAD_MACOS_NATIVE_UI
  bool asynchronousCommands() const override { return m_nativeUi; }
  bool modelCommandAsync(const QJsonObject& request, QObject* context, AsyncCompletion completion) override;
  bool modelPreviewAsync(const QByteArray& request, const QString& name, QObject* context,
                         AsyncCompletion completion) override;
  bool modelInspectionAsync(const QString& name, QObject* context, InspectionTask task,
                            InspectionCompletion completion) override;
  void cancelModelJob() override { m_documentSession->cancelJob(); }
#endif

  // CommandHost
  QJsonValue query(const QJsonObject& query) const override;
  std::shared_ptr<geometry::Shape> bodyShape(const QString& uid) const override;
  OcctViewer& viewer() override { return *m_viewer; }
  QJsonObject modelCommand(const QJsonObject& request) override { return command(request); }
  QJsonObject modelPreview(const QByteArray& command, const QString& name) override;
  void clearModelPreview() override;
  std::shared_ptr<geometry::Shape> previewBodyShape(const std::string& uid) const override;
  std::shared_ptr<geometry::Shape> previewToolShape() const override;
  bool runCommand(const QJsonObject& command, QJsonObject* result = nullptr) override;
  bool runCommands(const QJsonArray& commands, const QString& label) override;
  void refreshScene() override;
  void inputActivated(SelectFilter filter) override;
  void setConstructionHidden(bool hidden) override;
  void showHint(const QString& message) override;
  TopoDS_Shape itemShape(const SelectionItem& item) const override;
  TopoDS_Shape analysisShape(const QJsonObject& request) const override;
  double pixelSize(const gp_Pnt& point) const override; // mitcad#417
  QStringList bodyUids() const override;
  std::vector<BodyDisplay> modelBodies() const override;
  void showSection(const QJsonValue& plane) override;
  QJsonValue section() const override { return m_section; }
  void editInstead(const QString& uid, const QString& input) override;
  QString profileOccurrence(const QString& sketch) const override { return placingOccurrence(sketch); }
  QString bodyOccurrence(const QString& body) const override;
  QString datumOccurrence(const QString& datum) const override;
  const SelectionScope* commandScope() const override; // mitcad#217
  // Parameter names picked from the view (mitcad#250;
  // MainWindowParameterPicks.cpp).
  std::vector<ShownSketch> shownSketches() const override;
  const CommandDef* featureEditor(const QJsonObject& def) const override;

  // SketchHost
  const Selection& selection() const override { return m_selection; }
  void setSelection(const Selection& items) override;
  void pick(const Selection& items, Qt::KeyboardModifiers modifiers) override;
  void showError(const QString& message) override;
  bool sketchClickable(const SelectionItem& item, bool expanded) const override; // mitcad#217
  void deliverPendingPicks() override;                                           // mitcad#488

  // DocumentHost (the browser, the timeline and the parameters dialog)
  const CommandContext& model() const override { return *this; }
  bool runModelCommand(const QJsonObject& command, QJsonObject* result = nullptr) override;
  bool runModelCommands(const QJsonArray& commands, const QString& label) override;
  QString lastError() const override { return m_lastError; }
  bool canChangeModel(bool visibilityOnly = false) const override;
  QString changeBlockedReason() const override;
  QString readOnlyReason() const override;
  // The live MCP document id, new with every design (mitcad#452).
  QString designId() const override { return m_mcpDocumentId; }
  void showStatus(const QString& message, bool error) override;
  void editFeature(const QString& uid) override;
  void startCommand(const QString& id, const Selection& preselection,
                    std::function<void(bool committed)> finished) override;
  void createSketchOn(const SelectionItem& plane) override;
  void pickItems(const Selection& items) override;
  bool takesFeatures() const override;
  void lookAt(const SelectionItem& item) override;
  void showNamedView(const QString& view) override;
  void saveNamedView(const QString& name, bool replace) override;
  void exportSketch(const QString& sketch) override;
  void editAnalysis(const QString& name) override;
  void animateJoint(const QString& uid) override;
  // Motion studies and contact sets (mitcad#454; MainWindowMotion.cpp).
  void openMotionStudy(const QString& name) override;
  void editContactSet(const QString& name) override;
  void setIsolation(const Selection& items) override;
  const Selection& isolation() const override { return m_isolation; }
  void setOriginShown(bool shown) override;
  bool originShown() const override { return m_originShown; }
  QString documentFolder() const override;
  void exportFlatPattern(const QString& body) override;
  // Light bulbs in every mode (mitcad#295; MainWindowVisibility.cpp).
  bool runVisibilityCommands(const QJsonArray& commands, const QString& label) override;
  QString activeCommandName() const override;
  bool chooseInCommand(const QString& command, const QString& input, const QString& value) override;

protected:
  void closeEvent(QCloseEvent* event) override;
  // Files dropped on the window are opened or imported (U6).
  void dragEnterEvent(QDragEnterEvent* event) override;
  void dropEvent(QDropEvent* event) override;

private:
  bool m_nativeUi = false;
#ifdef MITCAD_MACOS_NATIVE_UI
  void createNativeWindow();
  void refreshNativeWindow();
  void refreshNativeActions();
  void refreshNativeInspector();
  bool requestNativeClose();
  void nativeAction(const QString& id);
  std::unique_ptr<NativeMacWindow> m_nativeWindow;
  std::unique_ptr<NativeCommandForm> m_nativeForm;
  std::unique_ptr<NativeWidgetView> m_nativeIdleForm;
  QHash<QString, QPointer<QAction>> m_nativeActions;
  QHash<QString, BrowserNode> m_nativeNodes;
  QByteArray m_nativeSnapshotKey;
  bool m_nativeCanRename = false;
  bool m_nativeCanToggleVisibility = false;
  QPointer<QWidget> m_nativeIdlePage;
  QPointer<ComputeProgress> m_nativeProgress;
  quint64 m_nativeJobGeneration = 0;
  bool m_nativeClosingCommand = false;
  QString m_nativeFocusInput;
#endif
  using Mode = DocumentSession::Mode;
  QString mcpBlockedReason(bool editing) const;
  // Live MCP (mitcad#265): busy refusals, a sketch's commands through its
  // session, the agent's undo labels, Save, and the ui_* tools.
  void mcpRefuseBusy() const;
  QJsonObject mcpSketchCommand(const QJsonObject& request);
  void mcpLabelAgentChange(int depthBefore);
  QJsonObject mcpSave(const QJsonObject& arguments);
  QJsonObject mcpUiState();
  QJsonObject mcpLog(const QJsonObject& arguments) const;
  QJsonObject mcpScreenshot(const QJsonObject& arguments);
  QJsonObject mcpSelect(const QJsonObject& arguments);
  QJsonObject mcpUiCommand(const QJsonObject& arguments);
  // Open into the window from an allowed folder, and the view's camera
  // (mitcad#402, mitcad#405).
  QJsonObject mcpOpen(const QJsonObject& arguments);
  QJsonObject mcpCamera(const QJsonObject& arguments);
  QJsonObject mcpCameraJson() const;
  // The corners of the boxes of entities as the view shows them; false with
  // `missing` naming one that must be in the model and is not.
  bool mcpItemCorners(const Selection& items, std::vector<gp_Pnt>& corners, QString* missing) const;
  // View > Fit (mitcad#405): the selection when something with a shape is
  // selected outside commands and sketches, the shown bodies when the
  // design isolates some, otherwise everything shown.
  void fitView();

  void registerCommands();
  void createMenus();
#ifdef Q_OS_MACOS
  void createWindowMenu();
#endif
  void createRibbon();
  void createPanels();
  void createIdlePage();
  // Floating chrome: the panels as cards over the view.
  void createFloatingPanels();
  int commandContentHeight() const;
  void updateCommandCard();
  // Shows the status label's text in the pill (Floating chrome).
  void updateStatusPill(bool error);
  bool isAvailable(const CommandDef& def) const;
  void updateActions();
  void trigger(const QString& id) override;
  // `analysis`: an analysis to edit instead (an entry of the `analyses`
  // query, mitcad#41).
  void startFeatureCommand(const CommandDef& def, const QString& editUid = QString(),
                           const QJsonObject& analysis = QJsonObject());
  // The command that can edit a feature, and the feature's name.
  const CommandDef* editorOf(const QString& uid, QString* name = nullptr) const;
  void onCommandFinished(bool committed);
  void confirm();
  void cancel();

  // Model access. command() runs a model command as a job (MainWindowJobs.cpp)
  // and throws rust::Error when the model rejects it, ComputationCancelled
  // when the user cancelled it, ModelBusy during another job.
  QJsonObject command(const QJsonObject& command);
  // The document, for reading it on this thread: not while a job computes
  // it (a programming error; ModelBusy).
  const Document& idleDocument() const;
  // Background computation (P7, MainWindowJobs.cpp). runJob runs `job` on
  // the worker thread and waits for it here: briefly without anything else,
  // then drawing while input waits in Qt's queue, and after 400 ms with a
  // modal progress dialog whose Cancel asks the job to stop. True when the
  // job ran to its end; false when it failed after a cancel request (what
  // it ran was then rejected and changed nothing); any other exception of
  // the job comes out here. The job may use only what it is given, and the
  // documents it computes report to job.control() (AttachedJob). `stage`
  // is what the dialog says the job does; the job may change it. Jobs
  // logged WhenSlow log their start and end only when they take longer
  // than the first, blocked wait (model commands, which are mostly quick).
  using JobLog = DocumentSession::JobLog;
  bool runJob(const QString& label, const QString& stage, const std::function<void(ModelJob&)>& job,
              JobLog log = JobLog::Always);
  void waitForJob(const QString& label, ModelJob& job, const QElapsedTimer& clock, bool& logged);
  // How a new document comes to the window: read or made on the worker.
  using DocumentMaker = std::function<rust::Box<Document>(ModelJob&)>;
  // Makes a new document the window's: `make` reads or makes it, then it is
  // computed, all as a job named `label`; the window's document stays as it
  // was when that fails (false, the reason in error) or is cancelled (false,
  // error empty). `handedOver`: a store of results the import process
  // handed over (F3dImport::handedOverResults, mitcad#413), which the
  // computation takes them from instead of evaluating them again.
  bool installDocument(const QString& label, const QString& path, const DocumentMaker& make,
                       QString& error, const QString& handedOver = QString());
  // The result store (P7d, framework/ResultCache.hpp): the document's
  // results that were costly to compute go to the store as a job, for the
  // document named `label` (after Save); and what a recompute took from
  // it and what was stored, logged.
  void persistResults(const QString& label);
  void reportStored(const QJsonObject& recomputed, const QJsonObject& persisted);
  // Preferences changed the caches: the document uses the store and the
  // memory budget as they say.
  void applyCacheSettings();
  // Help > Diagnostics (framework/CacheDiagnostics.hpp).
  void showDiagnostics();
  // Starts Mitcad again (Preferences' Layout, mitcad#419): the window
  // closes as when Mitcad quits (the design saved or asked about) and the
  // design, when it has a file, opens in the new one; nothing when the close
  // is cancelled.
  void restartApplication();
  // Shows what the command's recompute did; true when nothing failed.
  bool reportRecompute(const QJsonObject& result, double milliseconds);
  // A new component placed beside a component that takes none (mitcad#292):
  // the reply's warning shown as a hint, also when its command finishes.
  void notePlacement(const QJsonObject& result);
  int undoDepth() const;

  void recompute();
  void showBodies();
  void showProfiles();
  void refreshAutoselect();
  void showSketches();
  void showDatums();
  // The datums' pick places for UI tests (showDatums; again when the camera
  // comes to rest, when they changed).
  void logDatumPlaces();
  QVector<QJsonObject> m_loggedDatums;
  QString m_loggedDatumPlaces;
  // Where the shown bodies are in the window, for UI tests (when changed).
  void logBodyPlaces();
  // Whether an instance of a body is shown while something is isolated.
  bool isIsolated(const QString& body, const QString& occurrence) const;
  // Where an occurrence path places what is in it; null for the root.
  TopLoc_Location placementOf(const QString& occurrence) const;
  // The occurrence that places a sketch or datum of a component (its
  // component's first), or empty: the root's, and the sketch being edited.
  QString placingOccurrence(const QString& feature) const;

  // Dragging components the joints move (mitcad#55, MainWindowJoints.cpp):
  // the `joint_drag` query per pointer move shows where the occurrences go,
  // `drag_occurrence` on release keeps it, one undo step.
  void setUpOccurrenceDrag();
  // What a left drag that starts on `item` does (mitcad#150), so that the
  // hover's hint and the refusal agree: Drag moves `path`, the innermost
  // occurrence of the item's path whose parent component has joints and
  // whose unit can move (`text`: "Drag to move Pin:1 (Joint1: rz)");
  // Refuse says why not in `text` (grounded, held fixed by its joints, no
  // joints, a body of the root, a command open, read-only); Select (a
  // window selection) in a design without components, and in a command or
  // sketch on what could not be dragged anyway.
  struct DragAnswer {
    OccurrenceDragStart start = OccurrenceDragStart::Select;
    QString path;
    QString text;
  };
  DragAnswer dragAnswer(const SelectionItem& item) const;
  // The pointer over `item` in the view: the move cursor and the hint
  // when a drag would move it.
  void showDragHint(const SelectionItem& item);
  void occurrenceDragStarted(const SelectionItem& item, const gp_Pnt& grabbed);
  void occurrenceDragged(const gp_Pnt& target);
  void occurrenceDragFinished(const gp_Pnt& target, bool cancelled);
  // Animate Joint: the joint selected in the timeline that has free
  // motions, else the newest one.
  void animateSelectedJoint();
  // One frame of the animation: the joint previewed at its next value.
  void animationFrame();
  void stopAnimation(const QString& why);

  // Selection outside commands.
  void onPicked(const Selection& items, Qt::KeyboardModifiers modifiers, bool window);
  void updateIdleView();
  SelectFilter idleFilter() const;
  void showContextMenu(const QPoint& globalPosition, const SelectionItem& item);
  // The menu outside sketches and commands (mitcad#494,
  // MainWindowContextMenu.cpp): the commands that fit the selection in
  // Solid, BUILD and Construct, Delete and Hide; `entries` gets what it
  // adds, as the log shows it.
  void fillModelContextMenu(QMenu& menu, QStringList& entries);
  // Delete Member and Delete Spacing Set of the menu: build_remove of the
  // BUILD parts (components) given, or of their sets, one undo step.
  void deleteBuildParts(const QStringList& components, bool sets);
  bool keepSelected(const SelectionItem& item) const;

  // Sketches: Create Sketch picks the plane, then sketch mode until Finish
  // Sketch. Editing a sketch rolls the timeline back to it.
  void startSketch();
  // sketch.create with these fields (the plane, where it was picked).
  void createSketch(const QJsonObject& fields);
  // Design policy (mitcad#220): reads whose faces and planes sketches of
  // the active component may lie on (the model's sketch_supports) and
  // why the active component refuses sketches (empty when it does not).
  QString refreshSketchSupports();
  // Why a sketch cannot be made on this plane or face; empty when it can.
  QString sketchSupportRefusal(const SelectionItem& item) const;
  // Create Sketch's plane pick (MainWindowVisibility.cpp): whether the
  // origin planes are shown for it by themselves (mitcad#294: while the
  // design has no component but the active one and a root without bodies),
  // and with MITCAD_LOG_PICKS the faces a click picks, logged again after a
  // light bulb changed what is shown (mitcad#295).
  bool sketchShowsOrigin() const;
  void logSketchPicks();
  // While Create Sketch waits for a plane (mitcad#474): the text of the
  // view's banner and of the MCP's hint, the banner shown while the mode
  // lasts, and why a face or plane picked cannot take the sketch (a face
  // that is not planar, or what the design policy leaves out), shown and
  // logged as "Create Sketch: refused pick ...".
  QString pickPlaneText() const;
  void updateModeBanner();
  void refusePlanePick(const SelectionItem& item);
  ModeBanner* m_modeBanner = nullptr;
  // The occurrence paths ("O1/O4", "" the root) whose geometry the active
  // component's sketches may not lie on, with the reason.
  QHash<QString, QString> m_sketchDenied;
  // The names of the owners of those paths.
  QStringList m_sketchDeniedOwners;
  // A feature's error at the marker; empty when it did not fail.
  QString featureError(const QString& uid) const;
  void enterSketch(const QString& uid, bool existing);
  void editSketch(const QString& uid);
  // False when sketch mode stays: the timeline's roll-forward after an
  // edit was cancelled (P7).
  bool finishSketch();
  void leaveSketchMode();
  bool sketchExists(const QString& uid) const;
  // The sketch an item belongs to (profiles, sketch curves and points).
  static QString sketchOf(const SelectionItem& item);

  void undo();
  void redo();
  void openCommandSearch();
  void editShortcuts();
  // Look At (U5): the sketch plane in sketch mode, else the selected plane,
  // planar face, profile or sketch.
  void lookAtSelection();
  // The analysis shown (mitcad#41): its side cut away swapped, or hidden.
  void flipSection();
  void removeSection();
  // The bodies cut where the model's shown analysis says (mitcad#41),
  // unless an inspection's panel shows its own section.
  void followAnalyses();
  // Sets the plane the bodies are cut at, logging a change; showSection
  // also redraws.
  bool setSection(const QJsonValue& plane);
  // Whether the view shows the edited sketch: sketch mode, and also while
  // a command started in it (Offset, Mirror, Project, ...) has its panel
  // open (mitcad#223).
  bool sketchShown() const;
  // The plane the bodies are shown cut at: the sketch's while Hide Above
  // Sketch is on and the sketch is shown, else Section Analysis' (null:
  // whole).
  QJsonValue cutPlane() const;
  // The faces where the section cuts the shown bodies, for the view.
  void showSectionCaps(const std::vector<BodyDisplay>& bodies);
  // In sketch mode, for UI tests: how far each shown body reaches in front
  // of the sketch plane (at most 0 with Hide Above Sketch).
  void logSketchReach(const std::vector<BodyDisplay>& bodies);
  // Cosmetic threads on the shown bodies (the threads query).
  void showThreads(const std::vector<BodyDisplay>& bodies);

  void newDocument();
  bool deferForNativeCommand(std::function<void()> action);
  void clearNativeCommandContinuation();
  QMetaObject::Connection m_nativeCommandContinuation;
  void openDocument();
  bool save();
  bool saveAs();
  // A version recorded (recordVersion), or why not.
  struct VersionRecord {
    // What the status bar says after the save ("Saved bracket.mitcad as
    // version 3 (abc1234).", "Saved ..., but the version was not recorded:
    // ..."), red when `problem`.
    QString status;
    bool problem = false;
    // Recorded, or the file's content in the latest version already
    // (including commits with cleanup warnings).
    bool preserved = false;
    // When not preserved: why, as a sentence of its own ("The version of
    // bracket.mitcad was not recorded: HEAD is detached ..."); empty when
    // the user cancelled.
    QString failure;
  };
  // Writes the design file to `path`; in a project with version history
  // (P12d) also records a version with `description` (empty: the
  // automatic message of the undo steps since the last save). With
  // `required` the version is required (Save and Restore, mitcad#114):
  // false when none was recorded, even though the file may have been
  // written, and `required` says why (`failure`, empty when cancelled), so
  // that the caller writes nothing over those bytes.
  bool writeFile(const QString& path, const QString& description = QString(), VersionRecord* required = nullptr);
  // What follows writing the document's design file (`content`) to path:
  // the document is that file's now (mark_saved) and its autosave goes.
  // `status`: what the status bar says (empty: "Saved <path>"), red when
  // `problem`.
  void afterSaved(const QString& path, QByteArrayView content, const QString& status = QString(),
                  bool problem = false);

  // Versions (P12d, files/MainWindowVersions.cpp): Save in a project with
  // version history (a folder with .mitcad/project.json at the root of a
  // git repository) records a version; autosave never does.
  void registerVersionCommands();
  // File > Save with Description: a version with a description.
  bool saveVersion();
  // The project with version history a file is in; none with the reason.
  std::optional<rust::Box<Project>> versionedProject(const QString& path, QString* why = nullptr) const;
  // Who records versions ("Name <email>"): the project's or git's
  // configured author, else Preferences' default author; asked when none
  // has one (an older project, files/VersionDialogs.hpp). None when
  // cancelled.
  std::optional<QString> versionAuthor(const Project& project);

  // Local and Cloud projects (mitcad#89, files/MainWindowProjects.cpp):
  // the window's current project, which the indicator, remote work and
  // Project Settings follow, never the design's path.
  void registerProjectCommands();
  // The current project's folder when it keeps versions, else "".
  QString projectRootWithHistory() const;
  // The current project changed (`check`: its remote is checked).
  void setCurrentProject(const ProjectState& project, bool check);
  // A design opened or saved: the current project is its project (none
  // for a loose file).
  void followFileProject(const QString& path);
  void updateProjectIndicator();
  // File > New Project: the dialog, then the empty project in the Projects
  // panel (mitcad#145).
  void newProject();
  // What New Project chose: a project made (shown in the Projects panel, a
  // design it took from its folder opened, or the design `moving` moved
  // into it), Open It or Open It Instead. True when a project is the
  // current one now.
  bool handleNewProject(const std::optional<NewProjectResult>& result, const QString& moving);
  // File > Open Project: a folder (asked when empty) recognised; a project
  // opens with a design, anything else goes to New Project. `ask`: unsaved
  // changes are asked about first.
  void openProjectFolder(const QString& folder = QString(), bool ask = true);
  // An older project without versions: its repository and a first version.
  bool startProjectHistory(const QString& root);
  // A project shows in the Projects panel and its design opens: `design`
  // (relative), else the one opened last, else the newest; none (an
  // untitled design in the project) when it has none (mitcad#145).
  bool openProjectDesign(const QString& root, const QString& design);
  // File > Open from Cloud.
  void openFromCloud(const OpenFromCloudOptions& options = OpenFromCloudOptions(), bool ask = true);
  // File > Open Read-Only: a design without an edit lock.
  void openReadOnly();
  // File > Project Settings.
  void showProjectSettings();
  // Move to a Project: a loose design into a new or an existing project
  // (an existing one chosen in the project picker, mitcad#174).
  void moveToProject();
  // The design moved (or an untitled design saved) into the project at
  // `root`: to `target` (the picker's place), else under its name in the
  // project's top, its name asked in the picker when another design has it
  // there (an untitled design's always).
  bool moveIntoProject(const QString& root, const QString& target = QString());
  // The project picker in the window (mitcad#174, mitcad#176): the project
  // with versions and its folder (relative) selected in the Projects
  // panel, none ("", "") else.
  std::pair<QString, QString> panelProjectSelection() const;
  // Where Save As offers a place in the picker: the design's project and
  // folder, an untitled design's project folder, else the panel's
  // selection or the current project; none ("") for a design in no project
  // with versions (the file dialog).
  std::pair<QString, QString> saveAsProjectPlace() const;
  // A design saved or moved into a project shows selected in the Projects
  // panel.
  void showDesignInPanel(const QString& path);
  // Open Recent's label of a design: "bracket.mitcad - Robot arm".
  QString recentLabel(const QString& path) const;
  // The Projects panel (mitcad#145, projects/MainWindowProjectsPanel.cpp):
  // projects, folders and designs in one panel, and File > New Design.
  // view.projects and file.new_design.
  void registerProjectsPanelCommands();
  // The panel (none in the AppKit frontend), in a dock above the browser's
  // (Docked) or in a card above the browser's (Floating).
  void createProjectsPanel();
  void dockProjectsPanel(QDockWidget* browserDock);
  void addProjectsCard();
  bool projectsPanelShown() const;
  // Whether the panel shows at the start (mitcad#151): as the user last
  // chose, else shown when any project is configured.
  bool projectsPanelShownAtStart() const;
  // `remember`: the user's choice, kept for the next start.
  void setProjectsPanelShown(bool shown, bool remember);
  // New Project, Open Project, Open from Cloud: the panel shows the project.
  void showProjectInPanel(const QString& root);
  // Its projects read again (after Save, a sync, the focus back).
  void refreshProjectsPanel();
  // File > New Design (Ctrl+N): a design named in place in the panel's
  // selected folder of the current project, made as a version and opened;
  // without a current project an untitled design (newDocument, mitcad#175).
  void newDesign();
  // What the panel asks for (paths relative to the project at `root`).
  void createDesignInProject(const QString& root, const QString& path);
  void createFolderInProject(const QString& root, const QString& path);
  void moveInProject(const QString& root, const QString& from, const QString& to, bool folder);
  void deleteInProject(const QString& root, const QString& path, bool folder);
  void openPanelDesign(const QString& path);
  void panelVersionHistory(const QString& root, const QString& path);
  // Sync Now of a project: another project is opened first (as Open
  // Project).
  void panelProjectAction(const QString& root, const QString& id);
  // Project Settings of a project in the panel (mitcad#152, #153): the
  // current project's as File > Project Settings, another's without
  // opening it (its remote work is not this window's).
  void panelProjectSettings(const QString& root);
  // A version recorded in another Cloud project than the current one: sent
  // in the background when versions are sent at once.
  void sendPanelProjectVersions(const QString& root);
  void addExistingProjectFolder();
  // A command that changes a project's tree (create_folder, new_design,
  // move, delete) as a version, by the project's author, pushed as Save's
  // versions are; none when it was refused or cancelled. A refusal is told,
  // or with `refused` given only put there (the caller asks).
  std::optional<QJsonObject> projectWrite(const QString& root, QJsonObject command, const QString& title,
                                          QJsonObject* refused = nullptr);
  // The open design was moved or renamed to `path`: the window's file, its
  // version base and its edit lock follow.
  void openDesignMoved(const QString& path);
  // Where Save puts an untitled design: the panel's selected folder of the
  // current project, or "".
  QString projectsPanelFolder() const;
  // Before Save writes over its file in a project with history: true to
  // write (unchanged outside Mitcad, or Save as New Version chosen, which
  // records a change made outside first), false for Save As, none to
  // cancel: by the user (`failure` left empty) or because the status could
  // not be read or the change made outside not recorded (`failure` says
  // so; mitcad#114).
  std::optional<bool> askVersionConflict(const Project& project, const QString& path, const QString& author,
                                         QString& failure);
  // Records the saved file as a version. The displayed warning and the
  // preservation are separate: a failed or skipped commit (`preserved`
  // false) must stop an overwrite that requires the version (mitcad#114).
  VersionRecord recordVersion(const Project& project, const QString& path, QStringList paths,
                              const QString& message, const QString& author);
  // A file renamed outside Mitcad (m_renamedFrom): its rename recorded as a
  // version of its own while its content is as renamed; else the old path
  // goes with the next version (returned).
  QStringList recordRename(const Project& project, const QString& path, const QString& author);
  // The file's blob ids as opened or saved (m_versionBase).
  void rememberVersionBase(const Project& project, const QString& path);
  // Before a file is opened: one renamed outside Mitcad takes its display
  // state along from the path it had; returns that path (absolute) or "".
  QString followRename(const Project& project, const QString& path);
  // The status bar's project, branch and version of the open file.
  void updateVersionStatus();
  // File > Version History (P12e, files/VersionHistory.hpp): the open
  // file's versions, compared; Open, Restore or Save Copy As of one.
  void showVersionHistory();
  // Open: a version as a new, untitled design ("part v3") that Save As
  // keeps, read and computed as a job (asks about unsaved changes first).
  void openVersion(const QString& file, const FileVersion& version);
  // Restore: asked (the open design's unsaved changes saved as a version
  // first, or dropped), the version recorded as the latest (number `next`)
  // and the file opened again.
  void restoreVersion(const QString& file, const FileVersion& version, int next);
  // Save Copy As: a version written as a file of its own, which in a
  // project with history is recorded as a version of that file.
  void saveVersionCopy(QWidget* parent, const QString& file, const FileVersion& version);
  // The window's comparisons: a version with the open design (nothing
  // computed), and with the geometry (the versions computed as a job;
  // nothing when cancelled).
  QString compareWithOpenDesign(const QString& file, const FileVersion& version);
  std::optional<QString> compareVersionGeometry(const QString& file, const FileVersion* before,
                                                const FileVersion& version);
  // A version's preview, when it has none: the view as it is, kept by the
  // design file's blob id (versionThumbnailPath).
  void saveVersionThumbnail(const QString& blob);
  // Remote repositories (P12 remote, files/RemoteController.hpp): what the
  // controller asks the window about the open design.
  void createRemote();
  // Edit locks (mitcad#89, files/MainWindowLocks.cpp, files/LockController):
  // the controller of the open design's lock, and the window's read-only
  // mode, which refuses every edit where it passes: the commands'
  // availability (isAvailable), the model commands (command(): only those
  // that change no design, such as isolation and the section analyses, and
  // exporting), writing the design's file (writeFile), sketch mode and
  // command panels.
  void createLocks();
  bool windowReadOnly() const;
  // Whether a command is available in a read-only window: viewing,
  // inspecting, exporting, files, projects and versions.
  static bool readOnlyCommand(const CommandDef& def);
  // Whether a model command changes nothing of the design (allowed in a
  // read-only window).
  static bool readOnlyModelCommand(const QString& name);
  // In a read-only window: refuses `what` with the reason (status bar,
  // log) and returns true; else false.
  bool refuseReadOnly(const QString& what);
  void readOnlyChanged();
  // A read-only window shows a version of its design (the editor's, as
  // fetched), the file and the view left as they are.
  bool showLockedVersion(const QString& commit, QString& error);
  // A read-only window's unsaved changes: saved as a new version of the
  // file all the same (Sync's conflict dialog resolves them), or as a copy.
  bool saveAsNewVersion();
  // Save, Save As and closing in a read-only window: the unsaved changes
  // kept from editing are asked about (Save as Copy, Save as New Version,
  // Don't Save); nothing else is written. None: not read-only.
  std::optional<bool> readOnlySave(bool closing);
  // Live updates (mitcad#89, files/MainWindowLive.cpp): the live controller
  // with the remote's and the indicator's ends; updateLive tells it the
  // current project and the open design.
  void createLive();
  void updateLive();

  // Autosave (P8, files/MainWindowAutosave.cpp): the manager asks the
  // window about its document.
  void createAutosave();
  // The timeline marker an open edit restores when it rolled it back
  // (editing a sketch or a feature), else -1: autosave writes that.
  int autosaveMarker() const;
  // Recovery (P8c, files/MainWindowAutosave.cpp): the dialog of the work
  // left to recover (at start-up, or File > Recover Documents), and the
  // document restored from it: modified, as the file it came from.
  void recoverDocuments(bool atStart);
  bool restoreRecovered(RecoverableSession& session);
  // Before Save writes over a recovered document's file: true to write it
  // (unchanged since the document was opened, or Overwrite chosen), false
  // for Save As instead, nothing to cancel.
  std::optional<bool> askOverwriteChanged();

  // Component libraries (mitcad#64, mitcad#63, files/MainWindowLibraries.cpp).
  void registerLibraryCommands();
  void createLibraryMenu(QMenu* tools);
  void insertFromLibrary(bool community);
  void showLibraryParts();
  void manageLibraries();
  void publishToLibrary();

  // Files (U6, MainWindowFiles.cpp): the File menu, import and export.
  void createFileMenu();
  void registerFileCommands();
  // Opens a file as the document after asking about unsaved changes
  // (recent files, drops).
  void openPath(const QString& path);
  bool loadDesign(const QString& path, QString& error);
  // Imports an exchange file into a temporary document, then installs it.
  bool startImportedDocument(const QString& path, QString& error);
  // Import: into the open document (an .f3d design: a new document).
  void importFile();
  bool importPath(const QString& path, bool interactive, QString* error = nullptr);
  bool importCad(const QString& path, double unitMillimetres, QString* error);
  bool importDrawing(const QString& path, bool interactive, QString* error);
  // Insert SVG (mitcad#473): an SVG drawing into a new sketch, or the
  // edited one.
  void insertSvg();
  void insertMesh();
  void insertDxf();
  void insertComponent(const QString& path = QString());
  // `rerun`: the options of an .f3d import run again with the items whose
  // deviating results the user keeps (mitcad#379); empty for a new import.
  void startF3dImport(const QString& path, const QJsonObject& rerun = QJsonObject());
  // The items of an imported .f3d design whose results deviate from the
  // file's (mitcad#379): asked which to keep parametric, and the import run
  // again with them (`options`: those it ran with).
  void offerDeviations(const QString& path, const QJsonObject& result, QJsonObject options);
  void exportFile();
  // Export Drawing (mitcad#471; files/MainWindowDrawing.cpp).
  void exportDrawing();
  // 3D Print (mitcad#13, mitcad#234; files/MainWindowPrint.cpp): a panel of
  // the bodies to send, picked in the view, and the format, refinement and
  // slicer; OK writes the files and starts the slicer (sendToSlicer: false
  // when the files could not be written).
  CommandDef print3dCommand();
  bool sendToSlicer(const CommandState& state);
  // The appearance colours the view shows of bodies (all when empty), as
  // `export`'s `colors` for 3MF files.
  QJsonObject appearanceColors(const QStringList& bodies) const;
  // The bodies Edit Appearances assigns to: those the selection stands for
  // (bodies, and the bodies of faces, edges and vertices), each once.
  Selection appearanceTargets() const;
  void closeDocument();
  void addRecentFile(const QString& path);
  void updateRecentMenu();
  // Offers to save unsaved changes; false when the user cancels.
  bool maybeSave();
  bool isModified() const;
  QString documentName() const;
  // Where a file dialog starts: the design's folder, else the project,
  // the Projects panel's folder, else Documents.
  QString fileDialogDirectory() const;
  // Where a file dialog of the group starts: its last folder while it
  // exists, else fileDialogDirectory() (mitcad#498, framework/DialogFolders).
  QString dialogFolder(DialogGroup group) const;
  // A new, empty document (New, Close, a file brought in as a document).
  // False when replacing the current design is cancelled or refused.
  bool installEmptyDocument(const QString& name, QString* error = nullptr);
  void resetCommandState();
  void updateWindowTitle();

  // A profile's face as last shown, kept while the model's hash of its
  // region and its occurrence's placement stay the same (showProfiles).
  struct ProfileFace {
    QString hash;
    gp_Trsf placement;
    TopoDS_Shape face;
  };

  // Model and workflow state belong to the presentation-independent
  // document session. These references adapt the existing Qt host.
  DocumentSession* m_documentSession = nullptr;
  QString m_mcpDocumentId;
  bool m_mcpRequestActive = false;
  int m_mcpAgents = 0;                     // connected MCP clients (mitcad#265)
  QAction* m_mcpDisconnect = nullptr;      // the indicator's Disconnect Agent
  std::function<void()> m_mcpDisconnectAll;
  bool m_mcpSketchEdited = false;          // an agent changed the sketch being edited
  rust::Box<Document>& m_document = m_documentSession->state().document;
  ModelWorker* m_worker = &m_documentSession->worker();
  bool m_closePending = false; // the window closes when the job is back
  QHash<QString, ProfileFace> m_profileFaces; // by "<sketch>|<region>"
  QJsonArray m_currentProfiles; // exact region keys, including hidden profiles
  // Answers of queries while the actions are updated (updateActions).
  QHash<QString, QJsonValue>* m_queryMemo = nullptr;
  QString& m_filePath = m_documentSession->state().filePath;
  QString& m_documentName = m_documentSession->state().documentName;
  AutosaveManager* m_autosave = nullptr; // writes to the recovery folder (P8)
  // A recovered document's file as the earlier session had it (fileDigest;
  // P8c): Save asks before writing over a change made since. Cleared by
  // Save and by a new document.
  std::optional<QString>& m_recoveredDigest = m_documentSession->state().recoveredDigest;
  // Versions (P12d): the file's blob ids as opened or last saved, in the
  // folder and in the latest version (null: none), so that Save notices a
  // change made outside Mitcad; cleared by a new document.
  using VersionBase = DocumentSession::State::VersionBase;
  std::optional<VersionBase>& m_versionBase = m_documentSession->state().versionBase;
  // The path the file had in the history when it was renamed outside
  // Mitcad (absolute), recorded with the next version.
  QString& m_renamedFrom = m_documentSession->state().renamedFrom;
  // A project's folder for an untitled design (New Design): where Save As
  // puts it.
  QString& m_projectDir = m_documentSession->state().projectDir;
  QString m_loggedVersionStatus;
  // The current project (mitcad#89) and its indicator.
  ProjectState m_project;
  ProjectIndicator* m_indicator = nullptr;
  // The Projects panel (mitcad#145) and its dock or card.
  ProjectsPanel* m_projectsPanel = nullptr;
  QWidget* m_projectsContainer = nullptr;
  // Open Read-Only is opening a design: the lock controller takes no edit
  // lock on it, and the window is read-only.
  bool m_openingReadOnly = false;
  // The open design's edit lock and the read-only mode (mitcad#89).
  LockController* m_locks = nullptr;
  // Save as New Version writes a read-only window's file all the same, and
  // the editing that ends when the window turns read-only finishes.
  bool m_writeAnyway = false;
  // The next document installed keeps the camera (a read-only window
  // following the editor's versions).
  bool m_keepView = false;
  // The remote of the current project: sync, sending versions, its state
  // in the indicator (P12 remote).
  RemoteController* m_remote = nullptr;
  // Live updates through an MQTT broker (mitcad#89).
  LiveController* m_live = nullptr;
  bool& m_changeNoted = m_documentSession->state().changeNoted;
  QMenu* m_recentMenu = nullptr;
  QPointer<F3dImport> m_import; // an .f3d import in progress
  // The deviating results of the .f3d design imported last (mitcad#379):
  // the items the user left as the file's stored geometry, and the first
  // item kept parametric in the run since (the items after it were
  // replayed on its result and are asked again; -1: none).
  QString m_deviationsPath;
  QHash<qint64, bool> m_deviationsDeclined;
  qint64 m_deviationsRerunFrom = -1;
  OcctViewer* m_viewer = nullptr;
  CommandRegistry* m_registry = nullptr;
  Ribbon* m_ribbon = nullptr;
  // Floating chrome on macOS: the title bar and ribbon above the view; placed by createPanels().
  QWidget* m_header = nullptr;
  // Floating chrome elsewhere (floatingTopCards(), mitcad#425, mitcad#429): the title bar row over the
  // view without a card; it and the ribbon's group cards are rows of createFloatingPanels().
  TitleBar* m_titleRow = nullptr;
  CommandSearch* m_search = nullptr;
  ViewController* m_viewController = nullptr;
  UpdateController* m_updates = nullptr; // automatic updates (mitcad#9)
  ReportCenter* m_reports = nullptr;     // feedback and error reports (mitcad#61, mitcad#62)
  sketch::SketchController* m_sketch = nullptr;
  sketch::SketchPalette* m_palette = nullptr;
  // The palette's page of the command pages: the palette, or in the docked
  // layout a scroll area around it (mitcad#412).
  QWidget* m_palettePage = nullptr;

  QAction* m_confirmAction = nullptr;
  QAction* m_cancelAction = nullptr;
  QHash<int, QAction*> m_filterActions; // SELECT group, by SelectFilter

  BrowserController* m_controller = nullptr;
  // The document's structure as last read (refreshScene).
  DocumentSnapshot m_snapshot;
  // Occurrence paths ("O1/O4") and where they place their components.
  QHash<QString, gp_Trsf> m_placements;
  Selection& m_isolation = m_documentSession->state().isolation;
  bool& m_originShown = m_documentSession->state().originShown;
  QJsonValue& m_section = m_documentSession->state().section;
  QString m_loggedCaps;       // the section caps last logged
  QString m_loggedReach;      // how far the bodies reach in front of the sketch, last logged
  QString m_loggedThreads = QStringLiteral("Cosmetic threads shown: 0"); // as last logged
  QString& m_lastError = m_documentSession->state().lastError;
  bool& m_lastCancelled = m_documentSession->state().lastCancelled;
  QString m_placementNote; // the last command's placement warning (mitcad#292)
  QStackedWidget* m_commandPages = nullptr;
  QPointer<QWidget> m_panelScroll; // the running command's panel page
  QLabel* m_idleInfo = nullptr;
  QHash<QString, QString> m_idleMeasurements;
  QWidget* m_idlePage = nullptr;
  AutoselectPanel* m_autoselect = nullptr;
  Selection m_autoselectPreview; // hover only; never part of the actual selection
  QLabel* m_statusLabel = nullptr;
  QLabel* m_rendererLabel = nullptr;
  FloatingLayout* m_floating = nullptr; // the central widget in Floating chrome
  GlassCard* m_commandCard = nullptr;

  Mode& m_mode = m_documentSession->state().mode;
  bool& m_commandInSketch = m_documentSession->state().commandInSketch;
  QPointer<CommandSession>& m_session = m_documentSession->state().activeCommand;
  QString& m_lastCommand = m_documentSession->state().lastCommand;
  // Called when the running command ends (startCommand).
  std::function<void(bool committed)>& m_commandFinished = m_documentSession->state().commandFinished;
  Selection& m_selection = m_documentSession->state().selection;
  QString m_loggedFeatureFaces = QStringLiteral("none"); // the selected features' faces (UI tests)
  bool m_originVisible = false; // an input takes construction geometry
  bool m_constructionHidden = false; // an input hides it (mitcad#394)
  std::vector<SketchEntityDisplay> m_sketchDisplay;
  QString m_loggedSketches; // the sketches whose curves are shown (UI tests)
  // The shown bodies' middles ("F3.b0 in O1"), and those of their shapes.
  QVector<QPair<QString, gp_Pnt>> m_bodyCenters;
  QHash<const void*, gp_Pnt> m_localCenters;
  QString m_loggedBodies;
  QString m_loggedBodyPlaces;

  // A component being dragged (mitcad#55).
  struct OccurrenceDragState {
    QString path;    // the occurrence that moves, uids from the root
    QString name;    // its name, for the log
    gp_Trsf parent;  // where its parent component is in the design
    gp_Pnt grabbed;  // the point dragged, in the parent's coordinates
    gp_Pnt target;   // where it was dragged to, in the parent's coordinates
    std::vector<BodyDisplay> bodies; // as shown when the drag began
    QString contact; // the contact that stops it now (mitcad#454), if any
  };
  std::optional<OccurrenceDragState> m_drag;
  QString m_dragHint; // the hover's drag hint shown (mitcad#150)
  // A joint being animated (mitcad#55): its definition, the motion and the
  // values still to show; previews only, the document stays as it is.
  struct JointAnimation {
    QString uid;
    QString name;
    QString motion;
    QJsonObject def;
    QVector<double> frames;
    double start = 0.0; // where the motion was before the first frame
    int next = 0;
    std::vector<BodyDisplay> bodies; // as shown when it began
  };
  std::optional<JointAnimation> m_animation;
  // A motion preview's placements (Animate Joint, a study's playback;
  // mitcad#454) shown on `bodies`, the bodies as shown before it.
  void showPreviewPlacements(const QJsonObject& report, const std::vector<BodyDisplay>& bodies);
  // The Motion Study panel (mitcad#454), while it is open, and the bodies
  // as shown when it opened.
  QPointer<QDialog> m_motionStudy;
  std::vector<BodyDisplay> m_motionBodies;
  // The text of a contact that stopped a motion (the model's `contact`):
  // "Body2 (Pin:1) and Body3 (Wall:1) in Contact1".
  static QString contactText(const QJsonObject& contact);

  // Sheet metal (mitcad#210, MainWindowSheet.cpp): Edit Sheet Rules, Show
  // Flat Pattern and Export Flat Pattern; the sheet-metal body the
  // selection stands for (else the only one, or the first with a Flat
  // Pattern when `flat`), and the bodies shown flat.
  void registerSheetActions();
  QJsonObject sheetBody(bool flat) const;
  void toggleFlatPattern(const QString& body);

  // The sketch being edited (sketch mode).
  bool& m_sketchExisting = m_documentSession->state().sketchExisting;
  int& m_sketchEnterDepth = m_documentSession->state().sketchEnterDepth;
  int& m_sketchRestoreMarker = m_documentSession->state().sketchRestoreMarker;

  // Sketch selection filters (mitcad#217, MainWindowSelection.cpp): the
  // three-state filter of sketch mode, Project's context while it runs, the
  // SKETCH tab's Selection Filter control, and what they do to picks. The
  // SOLID tab's filters (m_filterActions) are separate and stay as they are.
  void setUpSelectionScope();
  // The SKETCH tab's control: an action whose menu holds the panel.
  QAction* createSketchFilterAction();
  // The context the control shows and changes: the running command's
  // (Project's), else the sketch's while a sketch is edited; null else.
  SelectionScope* activeSelectionScope();
  // Sketch mode's picks through the sketch filter (none while a tool draws).
  void applySketchScope(bool drawing);
  // The row of the edited sketch's own curve or point: ordinary,
  // construction or projected (centre lines are ordinary: they bound
  // profiles).
  FilterRow sketchRowOf(const SelectionItem& item) const;
  // What a window in sketch mode selects: what the view took (every kind
  // the filter lets a window take) and the dimensions and constraints it
  // holds when their row is a cross.
  Selection sketchWindowItems(const Selection& items) const;
  // A command starts or ends: a command with a selection filter context
  // (Project in sketch mode) shows its own rows in the control while it
  // runs; the sketch's come back as they were, on OK and on Cancel.
  void beginCommandScope(const CommandDef& def);
  void endCommandScope();
  // A row of the control changed (`what`: "<row> <state>" or "reset").
  void selectionScopeChanged(const QString& what);
  // For UI tests (MITCAD_LOG_EDGE_PLACES): where a click hits each body edge,
  // in sight and behind faces ("Sketch pick edge F1.b0/<edge> visible at
  // x,y"), once the camera rests in sketch mode.
  void logSketchPickPlaces();
  SelectionScope m_sketchScope = SelectionScope::sketch();
  std::optional<SelectionScope> m_commandScope;
  QAction* m_sketchFilterAction = nullptr;
  SelectionFilterPanel* m_selectionFilterPanel = nullptr;

  // The debug log (mitcad#269, MainWindowDebugLog.cpp): Help > Open Log
  // Folder; the header's facts (the open design, the graphics); "Debug log
  // on" in the project indicator, whose menu then has Open Log Folder and
  // Turn Off Debug Log.
  void registerDebugLogCommands();
  void createDebugLog();
  void debugLogChanged(bool running);
  void openLogFolder();
  QAction* m_debugLogOff = nullptr;

  // The wait cursor (mitcad#268, MainWindowJobs.cpp): its idle recovery
  // after each job and every second; with MITCAD_TEST_WAIT_CURSOR the test
  // commands Leave Wait Cursor On (Test) and Nested Wait (Test).
  void createWaitCursorRecovery();
  void registerWaitCursorTestCommands();
  void nestedWaitForTest();

  // A project's web viewer (mitcad#423, web/MainWindowWeb.cpp): File >
  // Project > Web Viewer (`turnOn`) and Update Web Viewer write the current
  // project's web/ folder (every design's page, its thumbnail, designs.json,
  // the index) and record it as one version (turned on: with the project's
  // setting), by recordWebViewer (the version's id, or "").
  void registerWebViewerCommands();
  void writeWebViewer(bool turnOn);
  QString recordWebViewer(const QString& root, QJsonObject command, const QString& title);
  // Save (writeFile), before it records the version of the file at `path`:
  // in a project whose web viewer is on and updated on each Save, the
  // design's page from the open document, its thumbnail and the index
  // written; returns those files (and those removed) for the version. None
  // when the file did not change (unless `renamed`).
  QStringList webViewerOnSave(const Project& project, const QString& path, const QString& author, bool renamed);
};

} // namespace mitcad
