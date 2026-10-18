// SPDX-License-Identifier: MIT
#pragma once

// Declarative command definitions (see app/COMMANDS.md). A feature command
// lists its inputs; the framework builds its panel, keeps the selections,
// previews the feature and adds or edits it. Actions and tools are commands
// without a panel that the toolbar, the command search and the shortcuts
// treat the same way.

#include <functional>
#include <limits>
#include <memory>
#include <optional>
#include <vector>

#include <QColor>
#include <QHash>
#include <QImage>
#include <QJsonArray>
#include <QJsonObject>
#include <QKeySequence>
#include <QList>
#include <QPair>
#include <QString>
#include <QStringList>
#include <QVector>

#include <TopoDS_Shape.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

#include "Selection.hpp"
#include "mitcad/geometry/shape.hpp"

namespace mitcad {

class CommandState;

// What the model knows, for command definitions (implemented by the main
// window on the document).
class CommandContext {
public:
  virtual ~CommandContext() = default;

  // A JSON query (commands.md); throws rust::Error when it is rejected.
  virtual QJsonValue query(const QJsonObject& query) const = 0;
  QJsonArray queryArray(const QString& name) const;
  QJsonObject queryObject(const QJsonObject& query) const;

  // Body at the timeline marker, or null.
  virtual std::shared_ptr<geometry::Shape> bodyShape(const QString& uid) const = 0;
  // The shape that stands for an item in the view (placed by its
  // occurrence); null when there is none.
  virtual TopoDS_Shape itemShape(const SelectionItem& item) const = 0;
  // A shape of the model's analyses (commands.md, "Analysis queries":
  // sections, clips, overlaps); null when there is none.
  virtual TopoDS_Shape analysisShape(const QJsonObject& request) const = 0;
  QString bodyName(const QString& uid) const;
  QString featureName(const QString& uid) const;
  // Profiles not used by a feature yet (the sketch's newest last).
  Selection unusedProfiles() const;
  // The expression to show for a parameter of feature `owner`: its own
  // dimensions show their expression, other parameters their name.
  QString valueText(const QJsonValue& slot, const QString& owner) const;
  // Number of edges a selection of faces and edges stands for.
  int edgeCount(const Selection& items) const;
  // The document's length unit ("mm", "in", ...) and its size in mm.
  QString lengthUnit() const;
  double unitMillimetres() const;
  // The size of one pixel of the 3D view in mm at a point of the design:
  // snap distances that keep their size on the screen (BUILD's `at.snap`,
  // mitcad#417). NaN without a view (an inspection on the model's worker).
  virtual double pixelSize(const gp_Pnt& /*point*/) const { return std::numeric_limits<double>::quiet_NaN(); }
};

enum class ValueKind { Length, Angle, Unitless };

// What a choice shown as a tile shows (InputDef::choiceTile, mitcad#427):
// the chosen option's picture (null: none), its name and a smaller line
// under it (BUILD: the supplier and part number).
struct ChoiceTile {
  QImage image;
  QString title;
  QString detail;
};

// A handle in the view that drags a value input (U4): an arrow for a
// distance along a direction, a ring for an angle about an axis. Model
// coordinates of the design (placed by occurrences).
struct Manipulator {
  // Toggle (P9): a dot at a pattern's instance that a click turns on or off.
  // Marker: a labelled point from the preview, without pointer interaction.
  enum class Kind { Arrow, Ring, Toggle, Marker };
  Kind kind = Kind::Arrow;
  gp_Pnt origin;
  // Arrow: the way the value grows, its handle at origin + value along it.
  // Ring: the axis, the angle right-handed about it.
  gp_Dir direction = gp_Dir(0, 0, 1);
  // Ring: where the angle 0 points (perpendicular to the axis).
  gp_Dir reference = gp_Dir(1, 0, 0);
  // Arrow: the value at the origin (a handle at origin + (value - base)).
  double base = 0.0;
  // Arrow: millimetres along the arrow per unit of the value (a diameter's
  // handle sits at the radius: 0.5).
  double factor = 1.0;
  // Toggle: the element it stands for and whether it is on.
  int element = -1;
  bool on = true;
};

// A labelled, non-interactive point from a successful feature preview,
// in design coordinates (for example a pattern's inferred anchor).
struct PreviewMarker {
  QString label;
  gp_Pnt point;
};

// One input of a feature command's panel.
struct InputDef {
  enum class Type {
    Selection,
    Value,
    Choice,
    Check,
    Flip,
    Text, // plain text, not evaluated (names, thread designations)
    CutTargets, // persistent assembly-cut scope and preview-fed checklist
    List, // rows of the `children` inputs (fillet sets, hole positions)
  };

  Type type = Type::Value;
  QString id;    // key in the command state
  QString label; // shown in the panel
  QString tooltip;

  // Selection: what the input accepts and how many (max 0: any number).
  SelectFilter filter;
  std::function<bool(const SelectionItem&)> test; // beyond the filter
  // Mesh bodies and their faces (item.geometry "mesh": triangles without
  // surfaces) are not taken: the operation needs surfaces (mitcad#399).
  bool refusesMeshes = false;
  int min = 1;
  int max = 1;
  QColor color; // highlight colour; the palette's next when invalid
  // The order of the picks matters (loft sections): the panel lists them
  // with buttons that move them.
  bool ordered = false;
  // Whether the origin's planes and axes are shown while the input takes
  // construction geometry (Measure leaves them hidden: they would cover
  // the model; construction features and a shown origin still count).
  bool showsOrigin = true;
  // A pick of an item already selected keeps it (and still reaches
  // onPick): a face clicked again adds another hole position.
  bool keepOnRepick = false;
  // The form a pick is kept in (a face stands for the feature that made it).
  std::function<SelectionItem(const SelectionItem&)> convert;
  // Called after a pick was taken, with the item as picked (with `at`).
  std::function<void(const SelectionItem&, CommandState&, const CommandContext&)> onPick;
  // Occurrences to draw as ghosts for an item of the input (mitcad#147):
  // paths of uids from the root, `O1/O4`. Their bodies are see-through and
  // not pickable, and so are the highlights on them, while the item is in
  // the input and the input does not take picks itself; a later input then
  // picks what lies behind them. Called with each item, the state (a list
  // row's for an input of a row) and the model, whenever the selections or
  // the active input change: a command can stop naming them (Joint: once
  // origin B is picked, the preview shows A where the joint puts it).
  std::function<QStringList(const SelectionItem&, const CommandState&, const CommandContext&)> ghosts;
  // The hover shows the whole tangent chain of an edge or face of a body
  // of these kinds (mitcad#290; the model's `tangent_chains` query) while
  // the command's check input `chainCheck` is on: what the operation will
  // take with the pick. Picks stay the item clicked.
  SelectFilter chainKinds;
  QString chainCheck;
  // What it takes now, following the other inputs (mitcad#394: BUILD's
  // planes and other faces only in a ground placement, a profile's faces
  // otherwise): the kinds (within `filter`) and a test with the state,
  // beside `test`. The hover highlights only what they take, the origin's
  // planes show only while the kinds have planes, and the active input
  // takes them again when they change.
  std::function<SelectFilter(const CommandState&)> filterNow;
  std::function<bool(const SelectionItem&, const CommandState&)> testNow;
  // Construction planes and axes are hidden while the input is active and
  // takes none now (BUILD's side and end placements); shown again when it
  // takes them, another input is active or the command ends.
  bool hidesConstruction = false;

  // Value: an expression with units, as text.
  ValueKind valueKind = ValueKind::Length;
  QString defaultText;
  // A handle in the view that drags the value; none when it returns none.
  std::function<std::optional<Manipulator>(const CommandState&, const CommandContext&)>
      manipulator;
  // Text: toggles in the view (P9: a dot at each instance of a pattern)
  // from the shown preview's report; a click toggles the element's number
  // in the text ("2, 5": the suppressed ones).
  std::function<std::vector<Manipulator>(const CommandState&, const CommandContext&, const QJsonObject&)>
      toggles;

  // Choice: (value, label) pairs.
  QVector<QPair<QString, QString>> choices;
  QString defaultChoice;
  // Choice: options that follow other inputs (a thread's sizes follow its
  // standard, P9); `choices` when unset.
  std::function<QVector<QPair<QString, QString>>(const CommandState&)> choicesFor;
  // Called after the user changed a choice, a value (typed, or dragged by
  // its handle) or a text: sets the inputs that follow it (a new thread
  // size takes its coarse designation; a length typed in BUILD detaches it
  // from its auto length, a search text narrows its picker).
  std::function<void(CommandState&, const CommandContext&)> onChange;
  // Choice: Tab, while the main window has the keyboard and the input is
  // shown, steps the choice to the next of these values (BUILD's auto
  // length: face, centre, through; mitcad#375). Shift+Tab still moves the
  // keyboard between the fields.
  QStringList tabCycle;
  // Choice: a picture for each option (by its value) beside its label;
  // a null image for none (BUILD's connectors: the library's previews).
  std::function<QImage(const QString&)> choiceImage;
  // Choice: shown as a row of icon buttons, one per option (by its value:
  // the icon's name in app/icons), the chosen one pressed, instead of a
  // drop-down (BUILD's X / Y / Z direction, mitcad#394). A frontend that
  // has no such row shows the drop-down.
  QHash<QString, QString> choiceIcons;
  // Choice: a button beside the drop-down (`buttonText`, `buttonTooltip`)
  // that calls `onButton` with the input's state and the model (BUILD's
  // part picker, mitcad#418). It changes nothing itself; what it opens may
  // choose an option later through the main window
  // (DocumentHost::chooseInCommand, CommandSession::choose). A frontend
  // without such a button (the AppKit inspector) shows none.
  QString buttonText;
  QString buttonTooltip;
  std::function<void(const CommandState&, const CommandContext&)> onButton;
  // Choice: shown as a tile instead of the drop-down and its button
  // (BUILD's part, mitcad#427): the chosen option's picture, name and a
  // smaller line under it, from the state. A click on it, or Enter or Space
  // while it has the keyboard, calls `onButton` (BUILD: the part picker,
  // whose choice sets the input). It takes the keyboard by Tab only, so
  // that Enter after a click still confirms the command. A frontend without
  // tiles shows the drop-down and its button.
  std::function<ChoiceTile(const CommandState&)> choiceTile;
  // Value: also a box at the cursor in the view (BUILD's length), in step
  // with the panel's field: what is typed in either is the input's text.
  // The shown inspection's report may give the box another text while the
  // input's own is not typed: {"cursor": {"<key>": "742.5 mm"}}.
  bool cursorBox = false;

  // Check and Flip.
  bool defaultChecked = false;
  // Check: shown as a small disclosure button with its label ("More",
  // mitcad#394) that opens the inputs below it whose `visible` follows it;
  // a frontend without one shows a check box.
  bool disclosure = false;

  // List: the inputs of a row, by their own ids (CommandState::row).
  std::vector<InputDef> children;
  QString addLabel; // the button that adds a row
  QString rowLabel; // "Set %1"
  int minRows = 1;

  // Shown only while this holds (for a list's input: of its row's state).
  std::function<bool(const CommandState&)> visible;

  // A bold heading above the row, shown and hidden with it: the first
  // input of a group of inputs (a thread's Extra clearance).
  QString heading;

  InputDef& withTooltip(const QString& tip) {
    tooltip = tip;
    return *this;
  }
  InputDef& withHeading(const QString& text) {
    heading = text;
    return *this;
  }
  InputDef& withAccepts(std::function<bool(const SelectionItem&)> extra) {
    test = std::move(extra);
    return *this;
  }
  InputDef& withoutMeshes() {
    refusesMeshes = true;
    return *this;
  }
  InputDef& withVisible(std::function<bool(const CommandState&)> shown) {
    visible = std::move(shown);
    return *this;
  }
  InputDef& withColor(const QColor& highlight) {
    color = highlight;
    return *this;
  }
  InputDef& withOrdered() {
    ordered = true;
    return *this;
  }
  InputDef& withConvert(std::function<SelectionItem(const SelectionItem&)> form) {
    convert = std::move(form);
    return *this;
  }
  InputDef& withOnPick(
      std::function<void(const SelectionItem&, CommandState&, const CommandContext&)> call,
      bool keep = true) {
    onPick = std::move(call);
    keepOnRepick = keep;
    return *this;
  }
  InputDef& withManipulator(
      std::function<std::optional<Manipulator>(const CommandState&, const CommandContext&)> handle) {
    manipulator = std::move(handle);
    return *this;
  }
  InputDef& withToggles(
      std::function<std::vector<Manipulator>(const CommandState&, const CommandContext&, const QJsonObject&)> made) {
    toggles = std::move(made);
    return *this;
  }
  InputDef& withGhosts(
      std::function<QStringList(const SelectionItem&, const CommandState&, const CommandContext&)> named) {
    ghosts = std::move(named);
    return *this;
  }
  InputDef& withHoverChain(SelectFilter kinds, const QString& check) {
    chainKinds = kinds;
    chainCheck = check;
    return *this;
  }
  InputDef& withChoices(std::function<QVector<QPair<QString, QString>>(const CommandState&)> options) {
    choicesFor = std::move(options);
    return *this;
  }
  InputDef& withOnChange(std::function<void(CommandState&, const CommandContext&)> call) {
    onChange = std::move(call);
    return *this;
  }
  InputDef& withTabCycle(const QStringList& values) {
    tabCycle = values;
    return *this;
  }
  InputDef& withChoiceImages(std::function<QImage(const QString&)> image) {
    choiceImage = std::move(image);
    return *this;
  }
  InputDef& withCursorBox() {
    cursorBox = true;
    return *this;
  }
  InputDef& withPickRule(std::function<SelectFilter(const CommandState&)> kinds,
                         std::function<bool(const SelectionItem&, const CommandState&)> takes) {
    filterNow = std::move(kinds);
    testNow = std::move(takes);
    return *this;
  }
  InputDef& withHiddenConstruction() {
    hidesConstruction = true;
    return *this;
  }
  InputDef& withChoiceIcons(const QHash<QString, QString>& icons) {
    choiceIcons = icons;
    return *this;
  }
  InputDef& withDisclosure() {
    disclosure = true;
    return *this;
  }
  InputDef& withButton(const QString& text, const QString& tip,
                       std::function<void(const CommandState&, const CommandContext&)> call) {
    buttonText = text;
    buttonTooltip = tip;
    onButton = std::move(call);
    return *this;
  }
  InputDef& withChoiceTile(std::function<ChoiceTile(const CommandState&)> tile) {
    choiceTile = std::move(tile);
    return *this;
  }

  // A choice's options in a state.
  QVector<QPair<QString, QString>> choicesIn(const CommandState& state) const {
    return choicesFor ? choicesFor(state) : choices;
  }
  bool accepts(const SelectionItem& item) const;
  // What a selection input takes in a state (filterNow, testNow).
  SelectFilter filterIn(const CommandState& state) const;
  bool acceptsIn(const SelectionItem& item, const CommandState& state) const;
  bool isRequired() const { return type == Type::Selection && min > 0; }
};

InputDef selectionInput(const QString& id, const QString& label, SelectFilter filter, int min = 1,
                        int max = 1);
InputDef valueInput(const QString& id, const QString& label, ValueKind kind,
                    const QString& defaultText);
InputDef choiceInput(const QString& id, const QString& label,
                     const QVector<QPair<QString, QString>>& choices, const QString& defaultChoice);
InputDef checkInput(const QString& id, const QString& label, bool defaultChecked = false);
InputDef flipInput(const QString& id, const QString& label);
InputDef textInput(const QString& id, const QString& label, const QString& defaultText = QString());
InputDef listInput(const QString& id, const QString& label, std::vector<InputDef> children,
                   const QString& addLabel, const QString& rowLabel, int minRows = 1);

// The current values of a command's inputs. The inputs of a list's rows
// are kept under "<list>.<row>.<input>"; row() gives a row's state with
// its inputs under their own ids.
class CommandState {
public:
  const Selection& items(const QString& id) const;
  Selection& items(const QString& id) { return m_items[key(id)]; }
  void setItems(const QString& id, const Selection& selection) { m_items[key(id)] = selection; }

  // What the user typed in a value input.
  QString text(const QString& id) const { return m_texts.value(key(id)); }
  void setText(const QString& id, const QString& typed) { m_texts[key(id)] = typed; }
  bool hasText(const QString& id) const { return m_texts.contains(key(id)); }
  QJsonObject object(const QString& id) const { return m_objects.value(key(id)); }
  void setObject(const QString& id, const QJsonObject& value) { m_objects[key(id)] = value; }
  // The expression to give the model: the text, with the document's unit
  // added to a bare number ("20" becomes "20 mm").
  QString expression(const QString& id) const {
    return m_expressions.value(key(id), m_texts.value(key(id)).trimmed());
  }
  void setExpression(const QString& id, const QString& normalized) {
    m_expressions[key(id)] = normalized;
  }
  // The evaluated value in mm or radians (NaN until evaluated or when the
  // expression is invalid).
  double value(const QString& id) const {
    return m_values.value(key(id), std::numeric_limits<double>::quiet_NaN());
  }
  void setValue(const QString& id, double number) { m_values[key(id)] = number; }
  // The parameter a value input edits, when the feature owns one (d5):
  // shown with its label (mitcad#231).
  QString parameterName(const QString& id) const { return m_parameters.value(key(id)); }
  void setParameterName(const QString& id, const QString& name) { m_parameters[key(id)] = name; }
  // That parameter as a sketch dimension's tooltip tells of it: "d5 = d1 / 2
  // → 20 mm (Extrude1 distance)" (mitcad#250); empty while invalid.
  QString parameterTooltip(const QString& id) const { return m_parameterTips.value(key(id)); }
  void setParameterTooltip(const QString& id, const QString& text) { m_parameterTips[key(id)] = text; }

  QString choice(const QString& id) const { return m_choices.value(key(id)); }
  void setChoice(const QString& id, const QString& option) { m_choices[key(id)] = option; }
  bool hasChoice(const QString& id) const { return m_choices.contains(key(id)); }

  bool checked(const QString& id) const { return m_checks.value(key(id)); }
  void setChecked(const QString& id, bool on) { m_checks[key(id)] = on; }
  bool hasCheck(const QString& id) const { return m_checks.contains(key(id)); }

  // Lists: the number of rows, a row's state, and taking a row out.
  int rows(const QString& list) const { return m_rows.value(key(list)); }
  void setRows(const QString& list, int count) { m_rows[key(list)] = count; }
  CommandState row(const QString& list, int index) const;
  void removeRow(const QString& list, int index);
  // The key of a row's input in the whole state.
  static QString rowKey(const QString& list, int index, const QString& id) {
    return QStringLiteral("%1.%2.%3").arg(list).arg(index).arg(id);
  }

private:
  QString key(const QString& id) const { return m_prefix.isEmpty() ? id : m_prefix + id; }

  QString m_prefix; // "<list>.<row>." in a row's state
  QHash<QString, Selection> m_items;
  QHash<QString, QString> m_texts;
  QHash<QString, QJsonObject> m_objects;
  QHash<QString, QString> m_expressions;
  QHash<QString, double> m_values;
  QHash<QString, QString> m_parameters;
  QHash<QString, QString> m_parameterTips;
  QHash<QString, QString> m_choices;
  QHash<QString, bool> m_checks;
  QHash<QString, int> m_rows;
};

// The outcome of building a definition from the inputs.
struct Built {
  QJsonObject def;
  QString error; // empty when the definition is complete
  QString input; // the input the error is about, if any
  // Instead of a definition: a feature to edit with its own command, and
  // the input that takes the keyboard there (P6: Press Pull on a fillet's
  // face opens the fillet).
  QString edit;
  QString editInput;

  static Built of(const QJsonObject& definition) { return {definition, QString(), QString(), QString(), QString()}; }
  static Built failure(const QString& message, const QString& input = QString()) {
    return {QJsonObject(), message, input, QString(), QString()};
  }
  static Built editing(const QString& feature, const QString& input) {
    return {QJsonObject(), QString(), QString(), feature, input};
  }
};

// What an inspection command shows instead of a feature (Measure,
// Interference, Section Analysis).
struct Inspection {
  QString text;       // the answer, rich text for the panel
  QString log;        // the answer in a line, for the log (UI tests)
  QString error;      // the model could not answer
  TopoDS_Shape shape; // drawn over the view (overlaps, the section plane)
  bool red = false;   // the shape is a problem (interference)
  QJsonValue section; // a plane reference: the bodies are shown cut there
  // Items highlighted besides the inputs' (orange; a component's bodies for
  // a Component item): BUILD's auto-length target (mitcad#375).
  Selection highlights;
  // Given to the definition's previewMarkers for labelled points over the
  // view (BUILD: the junctions, the new part's length at its free end).
  QJsonObject report;
  // A line shown over the top of the view while the command waits for
  // something there (BUILD's first part: "Click a plane or a face, or
  // press Enter", mitcad#394); none when empty.
  QString viewHint;
};

struct CommandDef {
  enum class Kind {
    Feature, // a panel with inputs that adds or edits a feature
    Action,  // runs at once (Create Sketch, Undo)
    Tool,    // an interactive tool in the view (sketch tools)
  };
  enum class Mode { Model, Sketch, Any }; // when it is available

  QString id;   // "solid.extrude"
  QString name; // "Extrude"
  QString icon; // file name in app/icons without .svg
  QString tooltip;
  QKeySequence shortcut;
  // Further keys of the same command (Delete's Backspace on macOS), active
  // while the shortcut is the default; not listed in the shortcut dialog.
  QList<QKeySequence> alternates;
  QString tab = QStringLiteral("SOLID");
  QString group = QStringLiteral("CREATE");
  bool pinned = false;  // also a button next to the group's menu
  // A submenu of the group's menu (the sketch CREATE group's "Circles",
  // mitcad#222): the commands with the same subgroup, together where the
  // first of them is. Its pinned command is the default of a split button
  // whose arrow opens the submenu; a subgroup of one command is a plain
  // entry.
  QString subgroup;
  QStringList keywords; // other words the command search finds it by
  Kind kind = Kind::Feature;
  Mode mode = Mode::Model;

  // Feature commands.
  QString featureType; // the type of definition it adds and edits
  QStringList editsAlso; // other types it edits (Move/Copy: moves of occurrences)
  // A new feature's inputs "operation" and "objects" (operationInputs)
  // follow what the model chooses from each preview's tool until the user
  // changes either (mitcad#289: Extrude joins the bodies its tool touches
  // and cuts those it goes into); editing keeps the stored operation.
  bool autoOperation = false;
  // Sketch commands (U2): `build` gives a `sketch.*` command (or
  // {"commands": [...]}) instead of a feature definition; the preview runs
  // it and takes it back, OK keeps it as one undo step.
  bool sketchCommand = false;
  // Sketch commands: the preview's answer once it applied, and an empty
  // object when it is taken back or failed, e.g. to draw what the command
  // removes (Clean Up Stale Projections, mitcad#252).
  std::function<void(const QJsonObject&)> sketchPreviewShown;
  // Model commands (U4: materials, New Component): `build` gives
  // {"commands": [...]}, run on OK as one undo step named after the
  // command; nothing is previewed.
  bool modelCommand = false;
  // Inspections (U4: Measure, Interference, Section Analysis): the panel
  // shows the answer while the inputs change, OK closes it.
  std::function<Inspection(const CommandState&, const CommandContext&)> inspect;
  // Whether OK keeps an inspection's section shown (Section Analysis). With
  // `build` it is kept in the document (mitcad#41): `build` gives the
  // analysis' definition, OK adds it (`add_analysis`) or changes the one
  // edited (`edit_analysis`), and `load` fills the inputs from an entry of
  // the `analyses` query to edit it.
  bool keepsSection = false;
  // Application commands with a panel (3D Print, mitcad#234): the inputs
  // are picked and checked as a feature's are (`build` gives an empty
  // definition, or the failure the panel shows), nothing is previewed, and
  // OK calls `accept`, which does the work outside the model's history;
  // false keeps the panel open.
  std::function<bool(const CommandState&, const CommandContext&)> accept;
  // The OK button's text when it is not "OK" (3D Print: Send).
  QString okLabel;
  // Commands that run model commands of their own (BUILD, mitcad#375),
  // with `inspect` as their preview: OK (Enter) calls `apply`, which runs
  // them through the main window (each its own undo step, or one step for
  // several) and may change the inputs; false when the model refused (the
  // panel shows the reason and stays). With `repeats` the panel then stays
  // open for the next one (BUILD's Place, the next profile), its Cancel
  // named Close: what was applied stays; else it closes.
  std::function<bool(CommandState&, const CommandContext&)> apply;
  bool repeats = false;
  QVector<InputDef> inputs;
  // Fills the inputs from the selection made before the command, instead
  // of each item going to the first input that takes it (3D Print: a face
  // stands for its body, an occurrence for the bodies shown in it, nothing
  // for every shown body).
  std::function<void(const Selection&, CommandState&, const CommandContext&)> preselect;
  // Defaults that depend on the model (the newest profile, ...), after the
  // inputs' own defaults and the selection made before the command.
  std::function<void(CommandState&, const CommandContext&)> init;
  // Called once when the command ends (OK, Cancel, Close or discarded),
  // with whether it was committed: what it opened beside its panel goes
  // with it (BUILD's part picker, mitcad#418).
  std::function<void(bool committed)> ended;
  std::function<Built(const CommandState&, const CommandContext&)> build;
  std::function<std::vector<PreviewMarker>(const QJsonObject&)> previewMarkers;
  // As previewMarkers, with the inputs' state (in its place when set): the
  // markers may follow the hover (BUILD's junctions of the profile under
  // the mouse, mitcad#375).
  std::function<std::vector<PreviewMarker>(const QJsonObject&, const CommandState&)> previewMarkersIn;
  // The mouse moved onto another item of the view (an invalid one: off the
  // bodies); true when it changed the state, so that the markers are drawn
  // again (no preview is computed for it).
  std::function<bool(const SelectionItem&, CommandState&, const CommandContext&)> hover;
  // The mouse moved over an item of the view, `at` where it is now (in the
  // item's component's coordinates), or off the items (an invalid item:
  // off the bodies, out of the view): true when it changed the state so
  // that the preview is computed again (BUILD: an end click's part
  // straight or turned before the click, mitcad#417). Not while the model
  // computes or the view is navigated.
  std::function<bool(const SelectionItem&, CommandState&, const CommandContext&)> hoverPreview;
  // Fills the inputs from a definition to edit: the `feature` query's def,
  // with the feature's "uid" added (see CommandContext::valueText).
  std::function<void(const QJsonObject&, CommandState&, const CommandContext&)> load;
  // Whether the inputs can hold all of a definition of `featureType`, so
  // that editing loses nothing (every definition when unset).
  std::function<bool(const QJsonObject&)> canEdit;
  // A log line after OK, such as "Added fillet on 4 edge(s)".
  std::function<QString(const CommandState&, const CommandContext&)> describe;
  // Whether describe reads the model as it was before OK (edges a fillet
  // rounds are gone after it), not after (the datum a feature added).
  bool describeBefore = false;

  // Actions and tools.
  std::function<void()> run;
  std::function<bool()> enabled; // besides the mode; always when unset
  // Also while a command's panel is open (U5: the view's commands, which
  // change no model: Fit, standard views, styles, ...).
  bool duringCommands = false;

  int inputIndex(const QString& inputId) const;
  // The first selection input, which the context menu matches.
  const InputDef* primarySelection() const;
};

} // namespace mitcad
