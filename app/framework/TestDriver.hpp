// SPDX-License-Identifier: MIT
#pragma once

// The in-process UI test driver. Created in main() and inert unless the
// environment variable MITCAD_TEST_INPUT names a script: then it runs the
// script's steps in the application itself, one per timer tick, by
// synthesising Qt events (QKeyEvent, QMouseEvent, QWheelEvent) for the
// widgets and by triggering actions. Nothing needs the operating system's
// input: no xdotool, no Accessibility or Screen Recording permission on
// macOS, no window messages on Windows, so the same scripts run over SSH on
// every platform. (Qt::Test is not linked into the application.)
//
// Environment (besides MITCAD_TEST_INPUT=<script>):
//   MITCAD_TEST_STEP_MS    the tick, in ms (default 50): one step, or one
//                          event of a gesture, per tick
//   MITCAD_TEST_SETTLE_MS  the pause after an input before the next step
//                          (default 250)
// The driver also reads variables for the script: ${NAME} in an argument is
// the environment variable NAME (see below).
//
// What it does about the application's state: it records every log line
// (qDebug, qWarning, ...; the messages still go to the handler that was
// installed before, so stderr is as without it) and takes "Computing X
// started" and "... done in / cancelled after" to know when the model is
// busy: steps that give input wait while a job runs (as ui_wait_idle does in
// tools/ui-test-lib.sh), and steps for a window wait while a modal dialog
// has the input. A step that opens a modal dialog does not return before the
// dialog closes (its nested event loop), but the driver goes on with the next
// steps from inside that loop, which is how the script closes the dialog.
// The driver's own log lines start with "TestDriver: " and are not part of
// what `expect-log` and the others match.
//
// Exit: "TestDriver: done (<n> steps)" and exit code 0 at the end of the
// script (not with `stay`); a failed step logs "TestDriver: FAIL line <n>:
// <reason>" and exits with 1. A script that cannot be read or has a syntax
// error fails the same way before it runs anything.
//
// Test script format
// ------------------
// One step per line: a name and arguments, separated by spaces. `#` starts a
// comment (at the start of a word); blank lines are ignored. Quoting is as
// in a shell: 'single quotes' keep everything as it is (best for regular
// expressions), "double quotes" interpret only \" \\ \$, a backslash outside
// quotes escapes the next character. `${NAME}` in an argument is replaced
// when the step runs, by a variable of the script (`set`, `capture`) or else
// the environment variable NAME (an undefined one fails the step);
// `${NAME:re}` is the value escaped for a regular expression (paths);
// `\${` is a literal `${`. Regular expressions are PCRE (QRegularExpression)
// and match anywhere in a log line. Percentages are of the 3D view widget;
// coordinates that the application logs ("at x,y") are in the main window's
// logical pixels. Buttons: left (default), right, middle. Modifiers: ctrl
// (the Command key on macOS), shift, alt, meta, joined with +.
//
// Input (waits until the app computes nothing; steps for the main window
// wait while a modal dialog is open):
//   key <key>...            a key press and release for the widget with the
//                           focus of the target window (see focus-dialog),
//                           or for the open popup menu. <key> is a portable
//                           key text: r, F6, Return, Escape, Tab, Down,
//                           ctrl+z, ctrl+shift+s, alt+r, ... A key with a
//                           shortcut (a QAction, a QShortcut, a button's
//                           mnemonic) triggers it unless the focused widget
//                           takes the key as text.
//   type <text>             the characters as key events
//   expect-widget-property <object-name> <property> <value>
//                           a Qt target widget's property as text
//   expect-projects-layout <count>  actual Projects/Browser bounds and card
//                           slots, toolbar containment and loaded project count
//   expect-sketch-finish-layout  no title-row Finish Sketch or reserved gap;
//                           ribbon and palette retain the enabled action
//   expect-ribbon-labels <label>...  actual workspace labels, including
//                           accessibility, and no all-caps group captions
//   collapse-card <object-name> true|false  use the card's chevron button
//   expect-setting <key> <value>     the current saved QSettings value
//   expect-dialog-gone <title-regex> no matching Qt tool window remains visible
//   hover-logged <regex>       move to a logged control without clicking
//   timeline-leave             leave the timeline and clear its preview
//   expect-timeline-hover <uids|none>  actual preview feature identifiers
//   expect-timeline-tooltip <regex>  actual visible tooltip plain text
//   capture-view-image <path>  save only the OCCT framebuffer
//   expect-view-image <path> same|changed  compare the actual framebuffer
//   expect-navigation-runtime       saved navigation bindings and camera gestures
//   expect-assistant-setup <mode>    isolated real setup controls and config results
//   expect-formula-builder          the open expression editor's real controls
//   open-formula-field <name>        open the editor attached to a real value field
//   expect-navigation-controls <mode>  default|configure|restored|reset
//   expect-selection-info <text-regex> [maximum-card-height] [with-create-row]
//                           idle Qt selection measurements and compact card;
//                           optionally add the visible Create row's size hint
//   click <x%> <y%> [button] [mods]
//   dblclick <x%> <y%> [button] [mods]
//   drag <x1%> <y1%> <x2%> <y2%> [button] [mods]     with moves between
//   wheel <x%> <y%> <notches> [mods]                 positive: away from you
//   wheel-burst <x%> <y%> <notches> <count> [mods]   <count> wheel events of
//                           <notches>, one per tick (a quick burst)
//   trackpad-scroll <x%> <y%> <dx> <dy> <mods> [momentum]
//                           pixel scroll through the viewer's wheel handler
//   click-logged <regex> [button]       the last log line matching regex that
//                           has "at <x>,<y>" ("Datum xy at ...", "Ribbon ...
//                           at ...", "Panel <command> input <id> at ...")
//   dblclick-logged <regex>
//   click-pick <regex>      like click-logged, among the "Pick <kind> <name>
//                           at x,y" lines after the last "Pick places:"
//   expect-pick-offset <regex> <dx> <dy> <item-regex>
//                           probe fresh viewport picks plus logical pixels
//   click-pick-offset <regex> <dx> <dy> <item-regex>   probe, then click there
//   click-visible-face      probe fresh visible OCCT face points, click one
//                           through the viewport input path, then verify the
//                           actual shared selection contains that exact face
//   drag-logged <from regex> <to regex> [dx]    left drag between two logged
//                           places; dx pixels right of the end
//   drag-from <regex> <dx> <dy>         left drag from a logged place by
//                           dx, dy pixels (a manipulator's handle)
//   sketch-click <x> <y> [button] [mods]   a sketch point in mm, from the
//                           logged "Sketch view" (the origin, (100, 0) and
//                           (0, 100) in window coordinates)
//   sketch-drag <x1> <y1> <x2> <y2>
//   sketch-pointer press|move|release <x> <y>  separate held left-drag events
//   expect-profiles <count> inspect the actual displayed profile objects
//   type-in <regex> <text>  reveal/focus the identified panel field, select
//                           all and type through its actual frontend editor
//   expect-input-text <id> <regex> check the actual Qt or AppKit field text
//   expect-model-parameter <name> <value|absent> check a user parameter in mm
//   command <id>            trigger the command's action (objectName
//                           "command_<id>": "sketch.create", "file.export")
//   open-file <path>        open through the external-file document host
//   open-file-now <path>    use that same host while its preview owns the model
//                           (Finder or drops), including save/lock questions
//   file-dialog save|directory <initial> [filters] [selected-filter]
//                           open a shared chooser with explicit initial state
//   menu <Top>/<Item>[/...] trigger an entry of the main window's menu bar
//                           ('&' and "..." in the names are ignored)
//   popup <entry>           choose an entry of the open popup menu (a
//                           context menu), by its text
//   button <regex> [objectName] click a visible button whose text (without
//                           '&') matches; optional Qt objectName selects an
//                           exact control in the target window
//   native-action <id>      dispatch the native AppKit menu command
//   native-click <id>       click a native control by accessibilityIdentifier
//   native-hover <id> enter|leave send an AppKit tracking event through the
//                           control's registered hover owner
//   native-type <id> <text> replace an AppKit field editor's text
//   native-choice <id> <title> choose a native popup item or workspace segment
//   expect-native-duplicate-choices
//                           verify distinct values with duplicate native labels
//                           in command forms and secondary-dialog bindings
//   expect-native-form-controls
//                           verify native icon artwork, values and disclosure;
//                           mount a synthetic disclosure command for layout checks
//   expect-native-disclosure-layout
//                           verify the mounted fixture's card height cap and
//                           scrolling to its last expanded input
//   native-toolbar-item <id> insert|remove|palette change the actual saved
//                           AppKit configuration or request a palette sample
//   expect-native-toolbar-item <id> true|false check actual toolbar membership
//   native-activate <id>    edit a native history icon or activate a model row
//   native-select <id> [shift|ctrl] select a native history icon/model row;
//                           modifiers exercise actual feature mouse input
//   native-expand <id>      expand a native outline hierarchy
//   native-click-now <id>   native click without waiting for model idleness
//   native-click-on-job <job> <id> arm one native button action on the next
//                           matching actual jobStarted while modelBusy is
//                           true, before progress-sheet/animation timing;
//                           fails if the armed job never starts
//   fail-next-recovery-command restore_command|discard_command
//                           fail one recovery job before it mutates the model;
//                           requires an idle native command session
//   expect-picking-on-job <job> probe a visible body during the next actual
//                           job: MoveTo must reject it without model reads;
//                           fails if the armed job never starts
//   native-type-now <id> <text> native edit without waiting for idleness;
//                           these exercise preview cancellation and races
//   native-key-now <key>... key input without waiting for model idleness
//   key-now <key>...       shared key input without the model-idle gate
//   qt-type-now <id> <text> replace a visible Qt editor through actual key
//                           events without waiting for model idleness
//   qt-cancel-progress <job> await the actual modal Qt ComputeProgress for
//                           the newest started job and click its real Cancel
//                           button, requiring modelBusy and cancelling state
//   native-key-event <key>... await actual AppKit activation/key focus, then
//                            send key down/up through NSApp's event dispatch
//   expect-native-application-delegate check Qt activation signals, deactivate
//                           events, Dock reopen and secure state restoration
//   native-file-name <name> edit the real native save filename value
//   native-file-directory <path> navigate AppKit chooser to existing folder
//   native-file-cancel      cancel the focused AppKit open/save panel
//                           AppKit requires actual user acceptance of its
//                           separate-process chooser; no accept verb exists.
//   native-resize <w> <h>   resize the native document content in points
//   window-resize <w> <h>   resize this test's Qt or native main window
//   window-move <x> <y>     move only this test's Qt/native document relative
//                           to its screen's visible origin
//   window-state normal|minimized|fullscreen change the document state
//   expect-window-state normal|minimized|fullscreen await its actual state
//   expect-viewport         check the real framebuffer dimensions against
//                           the viewport's backing scale; log GL version,
//                           logical screen size and reported refresh rate
//                           MITCAD_UI_REQUIRE_RETINA=1 additionally requires 2x
//   capture-view-target <var> record actual OCCT camera target
//   expect-view-pan <before> changed|unchanged compare target in mm
//   native-move <x> <y>     frame origin relative to the main screen's visible
//                           origin; native.window.origin and native.window.size
//                           expose the frame origin and content size as text
//   native-appearance light|dark set this app's AppKit appearance and Qt
//                           palette from real system colors, leaving OS
//                           preferences untouched
//   native-pinch <x%> <y%> <delta> dispatch magnify Began/Changed/Ended through
//                           the actual native document NSWindow
//   qt-pinch <x%> <y%> <delta> send the shared viewer a QNativeGestureEvent
//                           sequence, for comparison with AppKit routing
//   native-context-menu <id> cancel|item open a real AppKit browser/history
//                           context menu; cancel or select in its tracking loop
//   native-popup <id> cancel|item click a native popup button and choose its
//                           actual tracked AppKit menu leaf
//   view-context-menu <x%> <y%> cancel|item right click the actual viewport
//                           or orientation cube, including native menu tracking;
//                           "Sub/Item" chooses an item of a submenu
//   view-context-menu-logged <regex> cancel|item right click a logged position
//   view-pointer press|move|release <x%> <y%> none|left|right|middle [mods]
//                           separate events can simulate a release lost to menus
//   capture-view-orientation <var> record actual camera direction and up
//   expect-view-orientation <before> changed|unchanged compare that orientation
//   expect-view-buttons none|left|right|middle inspect OCCT's actual held buttons
//   expect-selection <total> <profiles> inspect the actual shared selection
//   capture-view-height <var> record the actual OCCT camera height in mm
//   expect-view-zoom <before> in|out|unchanged compare actual camera height
//                           [control-before control-after] also compares
//                           its ratio with a control gesture, detecting
//                           duplicated magnification as well as lost input
//   expect-native-text <id> <regex> assert native control text (6 s)
//                           native.file.name/directory read the focused
//                           file panel's public filename/folder properties.
//   expect-native-enabled <id> true|false assert native control state (6 s)
//   expect-native-visible <id> true|false assert native document view state
//   expect-native-present <id> true|false assert document view membership, including hidden views
//   expect-native-layout    full-window canvas, floating card geometry and
//                           real AppKit hit testing above/around the canvas
//   expect-native-history [uid...] compare horizontal native history icons,
//                           marker geometry and rollback appearance with the
//                           actual model; optional IDs check native and shared
//                           selection (a single '-' checks empty selection)
//   native-history-drag begin|to <index>|end send actual marker mouse events;
//                           separate steps let the real model settle during
//                           a single drag before its undo steps are merged
//   native-history-drag-now begin|to <index>|end send the same actual event
//                           immediately, including while the model is busy
//   expect-model-busy true|false assert the worker's actual state immediately
//   test-manipulator-feedback check actual subpixel length/angle overlay
//                           events at two zooms, placements and cancellation
//   menu-now <menu path> invoke the actual menu action without waiting for
//                           an active worker, to test document transitions
//   cancel-model-job <command name> cancel its running preview through the
//                           worker control, to test its interrupted result;
//                           normal interactive previews have no modal dialog
//   capture-undo-depth <var> read the real document's undo depth
//   expect-undo-depth <base> <change> assert its change since capture
//   expect-model-bodies <count> <volume-mm3> query real OCCT body volumes
//   expect-shown-body <uid> <occurrence|-> <volume-mm3>
//                           [min-x min-y min-z max-x max-y max-z] compare
//                           the viewer's actual clipped shape and world bounds
//   expect-native-dialog-gone <regex> [seconds] assert native window closed
//                           These native steps require the macOS AppKit UI.
//                           key/type/button/menu/focus-dialog/screenshot
//                           also use AppKit when native controls have focus.
//
// Windows:
//   focus-dialog <title regex> [seconds]  wait for a visible window of the
//                           application with that title (a dialog); key,
//                           type and button then go to it. A message box has
//                           no title on macOS: the regex is matched with its
//                           text and informative text there (a script
//                           gives both: '^Restore Version$|^Restore v1 ')
//   focus-main              give them to the main window again (a dialog
//                           that closed is replaced by the main window too)
//   focus-widget <objectName> focus a visible Qt widget in the target window
//                           or active popup; it becomes screenshot-dialog's target
//   dialog-resize <w> <h>   resize the focused Qt dialog in logical pixels
//   expect-dialog-appearance light|dark inspect its actual palette and scale;
//                           MITCAD_UI_REQUIRE_RETINA=1 additionally requires 2x
//   expect-command-search <filter> <current-id|*|-> [disabled|shortcuts]
//                           check real search rows, enabled selection and native
//                           shortcut text; * any selected, - none; optional flag
//                           requires disabled rows or the macOS Command glyph
//   click-parameter <name> <column> click the real visible Qt table cell (0..5)
//   expect-parameter-cell <name> <column> <regex> visible|hidden|refused|absent
//                           check its actual text, filter visibility and error
//
// The log (lines are numbered; `mark` remembers where the log is):
//   mark
//   wait-log <regex> [seconds]     a line since the mark matches (30 s)
//   expect-log <regex> [seconds]   the same, 6 s by default
//   expect-no-log <regex>          no line since the mark matches (now)
//   expect-ever <regex> [seconds]  a line anywhere in the log matches (also
//                           what was logged before the script started)
//   expect-never <regex>           no line in the whole log matches
//   capture <var> <regex> [seconds]   the last line since the mark that
//                           matches (10 s); group 1 (or all) into ${var}
//   set <var> <value>
//   expect-near <actual> <expected> [rel] [abs]   numbers: |a - e| <=
//                           max(rel * |e|, abs); rel 1e-6, abs 0.01 by default
//
// The system:
//   wait-idle [seconds]            no job computes (60 s)
//   sleep <ms>
//   expect-file <path> [seconds]   the file or folder exists
//   expect-files <dir> <glob> <count> [seconds]   <count> files in <dir>
//                           match the glob (hidden ones included)
//   screenshot <png>        the main window (the 3D view composed in)
//   screenshot-view <png>   the 3D view's framebuffer, as --screenshot
//   screenshot-screen <png> macOS: what the window server shows of the
//                           application's windows, the floating panels' child
//                           windows and their glass included (the others
//                           grab widgets; the 3D view of a session without a
//                           GPU may be blank in it)
//   screenshot-dialog <png> the window focus-dialog chose (the main window
//                           when none), as its widgets draw it
//   quit [code]             end at once, with exit code <code> (0)
//   kill                    end the process at once, as a killed app would
//                           (no cleanup, autosave files stay), exit code 0
//   stay                    at the end of the script: do not exit
//
// Example (see tools/ui-scripts/ for the workflow tests):
//   command sketch.create
//   expect-log 'Create Sketch: select a plane'
//   click-logged 'Datum xy'
//   key r
//   sketch-click 0 0
//   type 40
//   key Tab
//   type 25
//   key Return
//   expect-log 'Added rectangle 40 x 25 mm'

#include <atomic>
#include <deque>
#include <functional>
#include <vector>

#include <QElapsedTimer>
#include <QEvent>
#include <QHash>
#include <QMutex>
#include <QObject>
#include <QPoint>
#include <QPointer>
#include <QString>
#include <QStringList>

class QRegularExpression;
class QRegularExpressionMatch;
class QTimer;
class QWidget;

namespace mitcad {

class GlassCard;

class TestDriver : public QObject {
public:
  // Reads MITCAD_TEST_INPUT; without it nothing happens (no handler, no
  // timer). Create it once the QApplication exists and before the main
  // window, so that the log of the start-up is recorded; the driver finds the
  // window itself.
  TestDriver();
  ~TestDriver() override;
  TestDriver(const TestDriver&) = delete;
  TestDriver& operator=(const TestDriver&) = delete;

  bool active() const { return m_active; }

  // A log line (from the message handler, any thread).
  void record(const QString& message);

private:
  struct Step {
    int line = 0;
    QString text; // the script line, for the log
    QString name;
    QStringList args; // as written; ${...} is expanded when it runs
  };
  struct KeyPress {
    int key = 0;
    Qt::KeyboardModifiers modifiers;
    QString text;
  };

  void load(const QString& path);
  void tick();
  void run(const Step& step);
  void fail(int line, const QString& reason);
  void finish();
  void leave(int code);

  bool expand(const QString& raw, QString& out, QString& error) const;
  QWidget* inputWindow(const Step& step) const;
  QWidget* targetWindow() const;
  QWidget* viewer() const;
  bool findMain();
  static bool dialogMatches(const QWidget* window, const QRegularExpression& re);
  static bool isBlocked(const QWidget* window);
  bool computing() const;

  // Waiting: `ready` is polled every tick (without side effects), `then`
  // runs once it holds; `gate` (a window) must also not be blocked by a
  // modal dialog.
  void whenReady(std::function<bool()> ready, double seconds, const QString& timeoutMessage,
                 std::function<void()> then = {}, QWidget* gate = nullptr);
  void pollWait();

  // The log.
  int lineCount() const;
  int findLine(const QRegularExpression& re, int from, QString* line = nullptr) const;
  int findLastLine(const QRegularExpression& re, int from, QString* line = nullptr,
                   QRegularExpressionMatch* match = nullptr) const;
  bool loggedPoint(const QRegularExpression& re, int from, QPoint& out) const;
  bool sketchPoint(double x, double y, QPoint& out) const;
  bool viewPoint(double xPercent, double yPercent, QPoint& out) const;

  // Events. Gestures and keys are queued, one event per tick.
  void queue(std::function<void()> action);
  void sendKey(const KeyPress& press);
  bool triggerShortcut(QWidget* window, const KeyPress& press);
  GlassCard* cardAt(const QPoint& windowPos) const;
  void sendText(const QString& text);
  void sendMouse(QEvent::Type type, const QPoint& windowPos, Qt::MouseButton button, Qt::MouseButtons buttons,
                 Qt::KeyboardModifiers modifiers);
  void queueClick(const QPoint& at, Qt::MouseButton button, Qt::KeyboardModifiers modifiers, int clicks);
  void queueDrag(const QPoint& from, const QPoint& to, Qt::MouseButton button, Qt::KeyboardModifiers modifiers);
  void testManipulatorFeedback();
  void showDialogStyleFixture(const QString& kind);
  void testDialogControls(const QString& kind);
  void testMacToolButtons();
  void testSketchFinishLayout();
  void testRibbonLabels(const QStringList& expected);
  void testNavigationRuntime();
  void testNavigationControls(const QString& mode);
  void testAssistantSetup(const QString& mode);
  void testFormulaBuilder();
  void openFormulaField(const QString& name);
  void sendWheel(const QPoint& windowPos, int notches, Qt::KeyboardModifiers modifiers);
  bool saveScreenshot(const QString& path, bool viewOnly);

  bool m_active = false;
  QTimer* m_timer = nullptr;
  QElapsedTimer m_clock;
  qint64 m_notBefore = 0;
  int m_stepMs = 50;
  int m_settleMs = 250;

  std::vector<Step> m_steps;
  std::size_t m_next = 0;
  int m_line = 0; // the script line being run
  QString m_error;
  int m_errorLine = 0;
  bool m_stay = false;
  bool m_finished = false;
  QMetaObject::Connection m_nativeJobClickConnection;
  QString m_nativeJobClickLabel;
  int m_nativeJobClickLine = 0;
  QMetaObject::Connection m_nativeJobHistoryDragConnection;
  QString m_nativeJobHistoryDragLabel;
  int m_nativeJobHistoryDragLine = 0;
  QMetaObject::Connection m_jobPickConnection;
  QString m_jobPickLabel;
  int m_jobPickLine = 0;

  QPointer<QWidget> m_main;
  QPointer<QWidget> m_dialog; // the target of key, type and button; else the main window
  QPointer<QWidget> m_grab;   // the widget a pressed mouse button belongs to
  QPointer<QObject> m_found;  // what a wait found
  mutable QPointer<QWidget> m_viewer;

  std::deque<std::function<void()>> m_micro;
  bool m_waiting = false;
  std::function<bool()> m_ready;
  std::function<void()> m_then;
  QElapsedTimer m_waitClock;
  qint64 m_waitLimitMs = 0;
  QString m_waitMessage;
  QString m_waitDetail;
  int m_waitLine = 0;
  QPointer<QWidget> m_gate;
  qint64 m_blockedSince = -1;
  qint64 m_busySince = -1;

  QHash<QString, QString> m_vars;
  int m_mark = 0;

  mutable QMutex m_mutex;
  QStringList m_lines;
  std::atomic<int> m_started{0};
  std::atomic<int> m_ended{0};
  int m_ignoreStarted = -1;
};

} // namespace mitcad
