# Commands in the Mitcad application

How to add or change UI commands, sketch tools and panels, the UI
framework's conventions, and the log lines UI tests read. A feature
command is a declarative definition plus a little glue between its inputs
and the model's feature definition (`core/model/src/api/commands.md`);
the framework does the rest. Where the code lives: [Files](#files).

## A feature command

```cpp
CommandDef chamfer(const CommandContext& context) {
  CommandDef def;
  def.id = "solid.chamfer";            // unique, also the settings key of its shortcut
  def.name = QObject::tr("Chamfer");
  def.icon = "chamfer";                // app/icons/chamfer.svg
  def.tooltip = QObject::tr("Bevels edges; a face bevels all its edges");
  def.shortcut = QKeySequence();       // none by default
  def.tab = "SOLID";                   // SOLID, BUILD or SKETCH; empty: menus only
  def.group = "MODIFY";                // see Ribbon::groups()
  def.pinned = true;                   // a button besides the group menu entry
  def.subgroup = "";                   // a submenu of the group's menu, see below
  def.keywords = {"bevel"};            // the command search finds it by these too
  def.featureType = "chamfer";         // the definition type it adds and edits
  def.inputs = {
      selectionInput("edges", tr("Edges"), SelectKind::Edge | SelectKind::Face, 1, 0),
      choiceInput("type", tr("Chamfer Type"), kChamferTypes, "equal_distance"),
      valueInput("distance", tr("Distance"), ValueKind::Length, "1 mm"),
      valueInput("angle", tr("Angle"), ValueKind::Angle, "45 deg")
          .withVisible([](const CommandState& s) { return s.choice("type") == "distance_angle"; }),
      flipInput("flip", tr("Flip")),
      checkInput("tangent_chain", tr("Tangent Chain"), true),
  };
  def.enabled = [&context] { return hasBodies(context); };
  def.build = [](const CommandState& state, const CommandContext&) -> Built { ... };
  def.load = [](const QJsonObject& def, CommandState& state, const CommandContext& context) { ... };
  def.describe = [](const CommandState& state, const CommandContext& context) { ... };
  return def;
}
```

Register it in the family's `register...Commands(CommandRegistry&, const
CommandContext&)` (called from `MainWindow::registerCommands`); the order
of registration is the order of the group menus and buttons. The toolbar,
menus, command search, shortcut dialog and context menu pick it up from
the registry. (The real Chamfer has sets of edges, see `listInput`.)

**Subgroups** (mitcad#222): commands of a group with the same `subgroup`
are one submenu of the group's menu, where the first of them is, in
registration order (`Ribbon::createGroup`). The subgroup's pinned command
is the default of a split button (`QToolButton::MenuButtonPopup`; in the
Mac presentation `RibbonButton` draws the arrow part): the button runs it,
its arrow opens the submenu. A subgroup without a pinned command has no
button; a subgroup of one command is a plain entry. The sketch commands
give it with the group, `"CREATE/Circles"` (`sketchCommand` in
`SketchCommands.cpp`). The native macOS frontend mirrors the submenus as
popup entries of its group menus; the default keeps its plain button there.

Checklist for a new command or feature type:

- An icon in `app/icons` (see [Icons](#icons)); a missing file logs
  `Command <id> has no icon '<name>'`.
- A new feature type also needs an icon in `featureIcon`
  (`browser/DocumentSnapshot.cpp`; the generic `feature` icon otherwise).
- A command that changes the model outside a panel goes through
  `DocumentHost::runModelCommand` (refresh, volume log) and checks
  `canChangeModel`.
- Picks may carry an occurrence: a command whose definition refers to
  bodies of another component must refuse them (the model fails such
  features).
- Every unit a dialog or panel offers starts at millimetres, the thread
  table at ISO metric (`tools/ui-units-test.sh`).

### Inputs

| Builder | Panel | State |
|---|---|---|
| `selectionInput(id, label, filter, min, max)` | a button "N selected" in the input's colour, and a clear button; `max` 0: any number | `state.items(id)`: `Selection` |
| `valueInput(id, label, kind, default)` | a text field taking expressions with units (`20`, `2 in`, `d1 * 2`) | `state.text(id)` as typed, `state.expression(id)` for the model, `state.value(id)` in mm or rad (NaN if invalid) |
| `choiceInput(id, label, {{value, label}}, default)` | a drop-down | `state.choice(id)` |
| `checkInput(id, label, default)` | a check box | `state.checked(id)` |
| `flipInput(id, label)` | a flip button | `state.checked(id)` |
| `textInput(id, label, default)` | a text field, not evaluated (a component's name, a thread's designation) | `state.text(id)` |
| `listInput(id, label, {children}, addLabel, rowLabel, minRows)` | rows of the children with a remove button each and an add button (fillet and chamfer sets); rows of values only are lines of a table (hole positions) | `state.rows(id)`, `state.row(id, i)` (the row's inputs by their own ids), `CommandState::rowKey(id, i, child)` |

Modifiers:

- `withVisible(test)`: the row is shown only while `test(state)` holds;
  hidden inputs are not checked and not highlighted. A list's child gets
  its row's state (`state.row(list, i)`).
- `withHeading(text)`: a bold heading row above the input, shown and
  hidden with it: the first input of a group (Thread's and Hole's Extra
  clearance: Radial clearance and Flank clearance, shown for modelled
  threads only).
- `withAccepts(test)`: narrows a selection beyond its kinds (only straight
  edges and sketch lines: `item.geometry == "line"`); the view then
  highlights and picks only what the input takes.
- `withoutMeshes()` (mitcad#399): a selection that hands bodies or faces to
  a surface operation takes no mesh body and no face of one (triangles
  without surfaces; `item.geometry == "mesh"`: the view sets it on mesh
  bodies and their faces, the browser on bodies the `bodies` query marks
  `mesh`). The view neither highlights nor picks them; one picked in the
  browser is not taken, logged `<command> <label>: body F3.b0 not taken: a
  mesh body` with a hint in the status bar. Combine's Target Body and Tool
  Bodies, the Objects of the operation inputs, an extent's Object,
  Hole's Objects to Cut, Shell's Faces/Body, Draft's, Offset Face's,
  Replace Face's source, Split Face's and Delete Face's Faces, Split
  Body's Bodies and splitting tool, Pattern's Faces and Sweep's Guide
  Surface use it; the model refuses mesh bodies in those features anyway
  (commands.md, "Mesh bodies in surface operations").
- `withColor(color)`: a selection's highlight colour; otherwise blue,
  orange, green, purple in input order.
- `withOrdered()`: the picks' order matters (loft sections); the panel
  lists them with up, down and remove buttons.
- `withChoices(options)`: a choice's options follow other inputs
  (`options(state)`, e.g. thread sizes follow the standard); the
  drop-down is refilled when they change, and a value they lack stays as
  an extra entry. `withOnChange(call)`: called after the user changed a
  choice, to set dependent inputs (a new size takes its coarse
  designation); shown values it leaves NaN (`state.setValue(id, NaN)`)
  are evaluated again (Drive Joint's value of another motion). A value's
  or a text's `onChange` runs when it is typed (a value's also when its
  handle drags it): BUILD's typed length detaches the auto length, its
  search narrows the picker.
- `withTabCycle(values)`: a choice that Tab steps through these values
  while the main window has the keyboard and the choice is shown (BUILD's
  auto length: face, centre, through); the session filters the
  application's key presses while such a command runs (Shift+Tab still
  moves between the fields), logged `<command>: Tab steps <label>`.
- `withChoiceImages(image)`: a choice whose options (from `withChoices`)
  each have a picture beside the label, `image(value)` (a null image for
  none; 48 px in the drop-down). Logged `Panel <command> option pictures
  <key>: <n> of <m>` with the options. BUILD's connectors show the
  library's preview images.
- `withCursorBox()`: a value input also as a box at the cursor in the
  view, as a sketch tool's value boxes are (mitcad#375: BUILD's length).
  It follows the mouse over the view once it has moved there, and is
  shown while the input is; its text is the input's, or the shown
  inspection's `report.cursor.<key>` (the auto length) while that gives
  one. Typing in the box, or a number typed while the view has the
  keyboard (it goes into the box), is the input's text as typing in the
  panel's field is, and the field shows it; Enter confirms, Esc closes.
  Not in the AppKit frontend. Logged `Cursor box <command> <key>: <text>
  at x,y` when its text changes. the form a pick is kept in (Pattern's Features
  keeps the feature that made a face; Move/Copy's Components the
  occurrence that places a body).
- `withPickRule(kinds, test)` (mitcad#394): what a selection takes
  follows the other inputs: `kinds(state)` (within its filter) and
  `test(item, state)`. The view's hover and clicks use them, the origin's
  planes show only while the kinds have planes, and the active input
  takes them again after each change of the inputs (logged `<command>
  <label> takes now: faces`). BUILD's placement: planes and faces in a
  ground placement, a profile's faces otherwise. `withHiddenConstruction()`:
  while the input is active and takes no construction geometry, the view
  hides the construction planes, axes and points (not their visibility in
  the design; `CommandHost::setConstructionHidden`, logged `Construction
  geometry hidden for the command` and `... shown again`).
- `withChoiceIcons({value: icon})`: a choice shown as a row of icon
  buttons, the chosen one pressed (BUILD's X / Y / Z), logged `Panel
  <command> input <key>=<value> at x,y` for each; the AppKit frontend
  shows the drop-down. `withDisclosure()`: a check shown as a small
  `▸ More` button that opens the inputs whose `withVisible` follows it
  (a check box in the AppKit frontend).
- `withButton(text, tooltip, call)` (mitcad#418): a choice with a button
  beside its drop-down; a click calls `call(state, context)` once the
  model is idle (the presenter's `buttonClicked`, logged `<command>:
  <label> button`; the button's place `Panel <command> button_<key> at
  x,y`). It changes no input itself: what it opens chooses later through
  `DocumentHost::chooseInCommand(command, key, value)` (the main window's
  `CommandSession::choose`, as the drop-down chooses: `onChange` runs and
  the preview follows; false when that command does not run). BUILD's
  Part and Connector open the part picker so. The AppKit frontend shows no
  button.
- `withChoiceTile(tile)` (mitcad#427): a choice with a button
  (`withButton`) shown as a tile instead: `tile(state)` gives a
  `ChoiceTile` (the chosen option's picture, its name and a smaller line
  under it, BUILD: the supplier and part number), logged `Panel <command>
  tile <key>: <name> (<line>)` when it changes; the row's place is `Panel
  <command> input <key> at x,y`. A click on it, or Enter or Space while it
  has the keyboard (it takes it by Tab only, so that Enter after a click
  still confirms the command), calls the button's `call`. Frontends
  without tiles (AppKit, the web) show the drop-down and its button.
- `Inspection::viewHint`: a line over the top of the view while the
  command waits for something there (BUILD's first part: `Click a plane
  or a face, or press Enter`), logged `View hint <command>: <text>`.
- `withOnPick(call, keep)`: called after a pick, with `item.at` where the
  click hit, in its component's coordinates. With `keep`, picking an item
  already selected keeps it and still calls back (Hole adds a position
  per click; primitives and Coil put their centre at the click,
  `cmd::placeAt`).
- `withManipulator(handle)`: a value input with a handle in the view.
  `handle(state, context)` returns a `Manipulator` (or none): an arrow
  from `origin` along `direction`, knob at `origin + (value - base) *
  factor` (a diameter: factor 0.5), or a ring about `direction` with
  angle 0 towards `reference` (`cmd::arrow`, `cmd::ring`). Dragging sets
  the value rounded to a zoom-dependent step (whole degrees for rings) in
  the document's unit; a click without a drag picks what lies under it.
- `withToggles(made)`: a text input with dots in the view;
  `made(state, context, preview)` builds them from the preview's report
  (`Manipulator::Kind::Toggle` with `element` and `on`); a click on a dot
  adds or removes its element's number in the text ("2, 5"). Used by the
  patterns' Suppressed input.
- `showsOrigin = false`: the input takes construction geometry without
  showing the origin's planes and axes (Measure).
- `withGhosts(named)`: a selection whose picks let later picks through
  (mitcad#147). `named(item, state, context)` gives the occurrences (paths
  from the root, `O1/O4`) to draw as ghosts for an item of the input:
  their bodies, and those of the occurrences inside them, see-through in
  their own colour and not pickable (`BodyDisplay::Style::Ghost`,
  `OcctViewer::setGhosts`), and the highlights on them let clicks through
  too. The session asks every shown input's items whenever the selections
  or the active input change, and leaves out the active input's own (its
  next pick may lie on them: re-picking shows them normally); deselecting
  the item, OK and Esc take the ghosts away. A list row's input gets its
  row's state. Joint and As-Built Joint ghost side A's occurrence until B
  is picked (`ghostUntilPicked`); the same hook serves other commands that
  pick one component and then another behind it (Rigid Group, assembly
  constraints, mitcad#148). The ghosts of components that are not active
  (mitcad#420, "Inactive components as ghosts" under The view) look the
  same but stay pickable; a body ghosted by both is the command's ghost.
- `withHoverChain(kinds, check)`: while the check input `check` (a
  top-level one) is on, the hover over an edge or face of a body of
  `kinds` shows its whole tangent chain, what the operation takes with
  the pick (mitcad#290): Fillet's and Chamfer's edges, the faces of
  Shell, Draft and Replace Face, all with `tangent_chain`. Picks stay the
  item clicked. The session asks the model's `tangent_chains` query once
  per body and kind in a command run (logged as `Tangent chains of F2.b0
  (edges): 2 in 3 ms`, `(slow)` over a second; a failure is a warning and
  leaves the hover on the item) and looks each hover up; the view draws
  the rest of the chain in the hover's colour (`OcctViewer::setHoverChain`,
  `framework/CommandSessionHover.cpp`). The log line `Hover chain: 3
  edge(s) of F2.b0` (`face(s)`) gives how many the hover shows
  (`tools/ui-hover-chain-test.sh`).

Values:

- Give the model `state.expression(id)`: the typed text with the
  document's unit added to a bare number (`"20"` → `"20 mm"`). A plain
  number would be millimetres or radians (90 as an angle is 90 rad). The
  panel evaluates values with the model's `evaluate` query while typing
  and shows `= 120 mm` or the model's error.
- Decimal commas: the model reads `12,5` as `12.5`; `state.expression`
  has points, and the field shows the expression with points once Enter
  or focus-out. The application's own checks for a bare number use
  `parsePlainNumber` (`framework/Numbers.hpp`); spin boxes are
  `DecimalSpinBox`. Tests: `tools/ui-decimal-test.sh`, ctest `app.unit`.
- A 0 that would only make a parameter (a taper) can be left out of the
  definition (`cmd::nonZero`, as Extrude does).
- Counts and factors in slots the model reads as lengths (pattern
  quantities, scale factors, a construction path's fraction) go as plain
  numbers (`cmd::countValue`) and come back with `cmd::loadCount`;
  fractions of sweeps and pipes and coil revolutions are unitless slots
  and take the expression.

A required selection that a choice reveals and that is still empty
becomes the active one (Extrude's To Object).

### Glue

- `build(state, context)` returns the definition (`Built{def}`) or
  `Built::failure(message, inputId)` for what the inputs cannot express
  (profiles of two sketches, edges of two bodies). It runs only when every
  shown selection has its `min` items and every shown value is valid.
  `Built::editing` makes the command open another feature's edit instead
  (`CommandHost::editInstead`: Press Pull on a fillet face opens the
  fillet for its radius).
- `load(def, state, context)` fills the inputs from the `feature` query's
  `def` (with `uid` added) for editing. Parameter slots hold names:
  `context.valueText(slot, def["uid"])` gives the feature's own dimension
  as its expression and a borrowed parameter by name, so OK changes the
  own parameter or keeps using the borrowed one.
- `canEdit(def)`: whether the inputs can hold all of a definition. A
  feature with more than the panel shows (the importer's fixed planes or
  axes, an asymmetric fillet size, a hole placed by offsets from edges or
  by sketch coordinates, a partial thread) is not offered for editing,
  so OK cannot drop the rest.
- `editsAlso`: other definition types the command edits (Move/Copy edits
  `move` and `move_occurrence`; `build` gives the type).
- `init(state, context)`: defaults that depend on the model, applied after
  the inputs' defaults and the pre-selection (Extrude and Revolve take the
  newest unused profile).
- `ended(committed)`: called once when the command ends (OK, Cancel,
  Close, discarded), before the main window's `finished`: what the
  command opened beside its panel goes with it (BUILD's part picker,
  mitcad#418).
- `preselect(selection, state, context)`: fills the inputs from the
  selection made before the command itself, instead of each item going to
  the first input that takes it (3D Print: a face stands for its body, an
  occurrence for the bodies shown in it, nothing selected for every shown
  body); `init` follows.
- `describe(state, context)`: a log line after OK (UI tests read it);
  with `describeBefore` it reads the model as before OK (the edges a
  fillet rounds), else after.
- `enabled()`: whether the command can start.

Three other kinds of panels:

- `modelCommand`: `build` gives `{"commands": [...], "label": "..."}`, run
  on OK as one undo step through `CommandHost::runCommands` (Physical
  Material, Appearance, New Component); no preview, outside the timeline.
- `accept(state, context)` (mitcad#234): an application command with a
  panel (3D Print). The inputs are picked and checked as a feature's are,
  and the view highlights the selections; `build` gives an empty
  definition or a `Built::failure` the panel shows; nothing is previewed.
  OK calls `accept`, which does the work outside the model's history
  (files, other programs) and returns false to keep the panel open; then
  `Done <command>` is logged. `okLabel` names the OK button (Send).
- `inspect(state, context)` instead of `build`: an `Inspection` (rich
  text, a log line, a shape drawn over the view, red for a problem, a
  section plane) computed while inputs change; OK closes the panel
  (Measure, Interference, Section Analysis). With `keepsSection` the
  section stays after OK; with `build` too, OK keeps it in the document
  as an analysis (mitcad#41): `add_analysis` with `build`'s definition,
  or `edit_analysis` (shown) when the panel edits one
  (`CommandSession::editAnalysis`, its inputs filled by `load` from the
  `analyses` query's entry). While an inspection's panel is open it shows
  its own section (`CommandHost::showSection`); otherwise
  `MainWindow::m_section` follows the model's shown analysis
  (`MainWindow::followAnalyses` in `refreshScene`). Inspections draw with
  `setPreview`, so a command started during one ends it. An inspection's
  `highlights` are drawn orange besides the inputs' (a Component item: the
  bodies of its occurrence), and its `report` goes to the definition's
  `previewMarkers` for labelled points over the view (`previewMarkersIn`
  gets the state too). `hover(item, state, context)` is called when the
  mouse moves onto another item of the view (an invalid item: off the
  bodies); true when it changed the state redraws the markers, nothing
  is computed (BUILD's Connector: the junctions of the profile under the
  mouse). `hoverPreview(item, state, context)` (mitcad#417) is called as
  the point under the mouse moves (`OcctViewer::hoverMoved`: the item with
  its `at`; an invalid item off the bodies or when the mouse leaves the
  view), not while the model computes or the view is navigated; true
  when it changed the state computes the preview again (BUILD's Profile:
  an end click's part straight or turned before the click).
  `CommandContext::pixelSize(point)` is the size of a view pixel in mm
  at a point of the design (NaN on the model's worker), for snap
  distances that keep their size on the screen (BUILD's `at.snap`).
- `apply(state, context)` with `inspect` (mitcad#375, BUILD): a panel with
  model commands of its own. The inspection is the preview; OK (its
  `okLabel`, Place) is refused while the inspection failed, else calls
  `apply`, which runs its commands through the main window
  (`DocumentHost::runModelCommand`, each its own undo step, or
  `runModelCommands` for one) and may change the inputs; false (the model
  refused: its reason is shown) keeps the panel as it is. Without
  `repeats` the panel then closes (`Done <command>`); with `repeats` it
  stays open for the next one (`Applied <command> (<n>)`), its first
  needed selection active again, and its Cancel is **Close**: what was
  applied stays (`Closed <command> after <n>`, also in the AppKit
  frontend, whose checkpoint is then kept).

References (`commands/CommandSupport`):

- `SelectionItem::reference()` gives the JSON reference of an item
  (commands.md, "References to geometry"): `{"sketch", "region"}` for a
  profile, `{"body", "face"}`, `{"body", "edge"}`, `{"body", "vertex"}`,
  `{"body"}`, `{"sketch", "curve"}`, `{"sketch", "point"}`, or a datum id
  (`"xy"`, `"y"`, `"F5"`).
- `cmd::itemOf(ref, context)` is its inverse; invalid for fixed geometry
  (`isShowable`), which no input can show. `pathOf`/`pathItems` do the
  same for paths, `objectOf`/`loadObject` for extents' objects.

### What the framework does

- In Solid mode, `AutoselectPanel` offers the last unused sketch's profile
  groups from the model's `autoselect_profiles` query. Group clicks apply
  the viewport's per-item Ctrl-toggle semantics to the current selection;
  they preserve unrelated items and pass every selected profile to the
  next command as normal preselection. Hover is a separate highlight list.
  Scene refresh clears that preview, replaces the groups and removes idle
  profile selections whose exact keys no longer occur in `profiles`.
- Pre-selection goes to the first input that takes each item. The first
  selection still needed is active (else the first); a click toggles an
  item in the active input, a one-item input takes the new one, and a
  full input passes on to the next. Window selection adds what the window
  holds of one kind. Clicking an input's button activates it. While an
  input takes construction geometry the origin planes and axes are shown.
- Items made by the previewed feature itself cannot be picked: picks are
  checked against the bodies before the feature.
- Previews run 150 ms after the last change and are cached by definition;
  the model caches results too, so OK does not compute again. A failed
  preview shows the model's message and the selections in red and OK is
  refused; missing selections show a hint ("Select edges.").
- OK (Enter, wherever the focus is) runs `add_feature`, or
  `edit_feature` when editing. Esc cancels.
- Editing rolls the timeline back to just before the feature, so picks
  name geometry there and the preview adds the edited definition at that
  point. OK undoes the roll-back and edits (one undo step); Cancel undoes
  it too. A feature type is edited by the command whose `featureType` it
  is (timeline double-click, the context menu's "Edit <feature>").
- The context menu offers the feature commands whose first selection
  input takes everything selected, "Edit <feature>" for an item a feature
  made, and Repeat; in sketch mode, model faces, edges and vertices get
  Project.
- Inputs that take `SelectKind::Feature` get the timeline's and browser's
  clicks on features (`DocumentHost::takesFeatures`); a face clicked in
  the view goes in as the feature that made it (`withConvert`).
- Handles (`ManipulatorOverlay`) are drawn for shown value inputs that
  have them, the active one's knob orange. The overlay follows the camera
  through `OcctViewer::viewChanged` and `toWidgetF`; a new display style
  or section view must keep these right.
- Long panels scroll (`m_panelScroll`); adding or removing a list row logs
  the panel's places again.
- The panel is never wider than its page, which does not scroll sideways
  (mitcad#198): its message (`CommandMessage`) breaks between words and
  inside a word longer than the line (a profile's or face's name), shows
  at most six lines and scrolls beyond them, can be selected and copied
  whole, and is hidden when empty; the notes under the fields break long
  words too (`breakableText`). A row that still asks for more width is
  squeezed rather than pushing the fields and OK and Cancel out of sight.

## Actions and tools

Commands without a panel set `kind` to `Action` (runs at once: Create
Sketch, Undo, Fit, Command Search) or `Tool` (an interactive tool in the
view: the sketch tools), a `mode` (`Model`: outside sketches, `Sketch`,
`Any`) and `run`. No command runs while a feature command is active,
except those with `duringCommands` (Fit, standard views, camera, visual
styles, grid, environment, Preferences, the shortcut overview). A `Model`
feature command whose first input takes profiles (Extrude, Revolve) can
also start in a sketch: the sketch is finished first and its selected
profiles go to the command. Measure is `Mode::Any`: in a sketch, the
sketch pauses as for sketch panels; sketch curves and points are measured
in the application on the shapes the view shows, the rest by the model's
`measure` query.

View and file commands are `Action`s with an empty tab (menus, search,
shortcuts).

**Shortcuts and macOS.** `def.shortcut` is the default key (Restore
Defaults and the saved settings go by it); `def.alternates` are further
keys while the shortcut is the default (Redo's Ctrl+Shift+Z, Fit's F6 and
Delete's Backspace on macOS). A platform's other default is an
`#ifdef Q_OS_MACOS` in the definition; nothing is remapped at run time. Qt
makes `Ctrl` Cmd and `Meta` Control on macOS. Fit is Cmd+0 there (F-keys
are media keys); Backspace as Delete loses to a focused text field (the
browser and timeline handle it in `keyPressEvent`); no default is Cmd+H,
Cmd+M, Cmd+Q, Cmd+Space or Cmd+Tab. Menu actions Qt's text heuristic could
move to the application menu get no role (`MainWindow::createMenus`);
Exit, Preferences (Cmd+,) and About have their roles, and macOS has a
Window menu.

## Sketch mode

Create Sketch (`MainWindow::startSketch`) sketches on the selected plane or
planar face; with none, it waits for a pick (`Mode::PickPlane`). It shows
the origin planes for the pick only while the design has no components
(mitcad#294; `sketchShowsOrigin`), not counting the active component and a
root without bodies of its own: a new design's empty Part1 under its empty
root shows them, and so does a design whose bodies are all the active
component's. With another component, or the root's bodies beside the
active component, sketches usually start on their faces or on a
construction plane; the hint says that the browser's Origin folder shows
the origin planes, which can then be picked. Light bulbs work during the
pick (mitcad#295): hiding a body lets the faces behind it be picked, and
the pick places are logged again. Logs: `Create Sketch: origin planes
shown` (`not shown (the design has components)`). Finish Sketch
(Ctrl+Enter) leaves it. Editing a
sketch moves the timeline marker to just after it and back when it is
finished; everything in between, marker moves included, becomes one undo
step `Edit Sketch1` (`merge_undo`, commands.md, "Undo, revisions and the saved state").

`sketch::SketchController` is sketch mode: an event filter on the view's
mouse events; it keeps the edited sketch as the `sketch` query gives it
(`SketchModel`, re-read after every change by `refreshScene`), and owns
`SketchOverlay` (QPainter in view pixels: constraint glyphs, dimensions,
tool previews, the snap marker, DOF). Clicks on glyphs and dimension
values select them (`SelectKind::SketchConstraint`, `SketchDimension`; a
fixed entity's lock is `fix:<entity>`); double-click on a value edits it;
dragging a value moves it; dragging a sketch point or curve drags it with
the solver (`sketch.drag` per move, merged into one step).

### A sketch tool

A drawing or picking tool is a `sketch::SketchTool` (`sketch/SketchTool.hpp`)
made by a factory and started by a `Tool` command:

```cpp
add({"sketch.line", QObject::tr("Line"), "line", "CREATE/Lines",
     [&c] { c.startTool(sketch::lineTool(c)); }, QKeySequence(Qt::Key_L), true, tooltip});
```

The sketch CREATE group's subgroups (mitcad#222), in this order: Lines
(Line), Rectangles (default 2-Point Rectangle), Circles (default Center
Diameter Circle), Arcs (default 3-Point Arc), Other Geometry (Ellipse, the
polygons, the slots, the splines, Point; no default), Text, Patterns
(Rectangular Pattern, Circular Pattern, Mirror; no default), References
(Project); Sketch Dimension follows on its own. Offset, Edit Pattern, Edit
Offset and Edit Text are in MODIFY. The toolbar row has Line, the three
split buttons and Sketch Dimension (`tools/ui-sketch-create-menu-test.sh`).

- `move(snap)`, `press(snap)`, `release`, `doubleClick` get the cursor
  snapped (`SketchSnap`: points, origin, midpoints, centres, quadrants,
  intersections, onto curves, the grid; with `bodySnaps()` also body
  vertices and edge midpoints in the plane, below; tolerances in pixels).
  Picking tools return `snaps() == false` and use
  `controller.entityAt(screen)`.
- The pointer is the precision cursor (`framework/Cursors.hpp`): a cross
  with the tool's icon as a badge (`controller.setToolIcon`, given by the
  registration), a square in its gap while snapped to a point.
  `OcctViewer::setSketchInput(true, icon)` sets it, `setSketchSnapping`
  follows the snap; built for the screen's device pixel ratio (again on
  `QEvent::DevicePixelRatioChange`) and the system's pointer size, cached.
- `paint(ToolPreview&)` gives the rubber band in sketch coordinates (with
  `glyph` for an inferred constraint, `dimension` for one being placed).
- `controller.setFields({{"length", tr("Length"), Kind::Length}, ...})`
  shows value boxes at the cursor (Tab moves, Enter calls `confirm()`);
  `setLive` shows the cursor's value while nothing is typed;
  `fieldValue`/`fieldExpression` give a typed one (evaluated by the
  model's `evaluate` query).
- `confirm()` (Enter) and `cancel()` (Esc: one step back; false at the
  start ends the tool). Esc in the value editor of the Dimension tool also
  steps the tool back; in an editor opened from the context menu (Edit
  Dimension) it only closes the editor and a running tool keeps its state
  (mitcad#354). `name = value` in the Dimension tool's editor adds the
  dimension with that parameter name and closes the editor (mitcad#352);
  when editing an existing dimension the editor stays open if the model
  refuses the name.
- `SketchOp op(controller)` is one undo step of model commands:
  `op.run(cmd)` (refusals keep the model's reason for `refuse`),
  `op.pointInput(snap)` (an existing point where it snapped, the origin's
  fixed point made when needed, else `[x, y]`), `op.constrain(point,
  snap)` (coincident, midpoint, on-curve, intersection constraints for a
  new point), `op.addConstraint`, `op.addDimension(def, typed)` (refusals
  only logged), `op.commit()` (merges the steps, refreshes) or
  `op.rollback()`.
- Typed values become dimensions; snapped points get constraints (every
  drawing tool: a point that snapped onto geometry is held there, mitcad#353;
  the Point tool on the origin makes the sketch's one fixed origin point and
  no constraint, mitcad#355); an Ellipse whose rim click is farther than its
  major end turns its axes, and a typed major radius then dimensions the
  minor one (mitcad#356); the
  line tool infers horizontal, vertical, perpendicular, parallel (to the
  line last passed over) and tangent within a few pixels.

Body edges and vertices in the sketch plane (mitcad#256,
`sketch/BodyGeometry.hpp`): `controller.bodyGeometry()` reads the model's
`plane_geometry` query (commands.md, "Body geometry in a sketch's plane")
once per tool session and again after the document changed (its
revision), logging `Sketch plane geometry: 4 edge(s), 4 vertex(es)`. The
items have ids of their own that never clash with the sketch's: `body:3`
an edge or a vertex, `body:3:start`, `body:3:end` and `body:3:center` a
curve's points. Nothing is projected while a tool picks; the action that
makes something from an item projects it then with
`op.projectBody(item)` (`sketch.project`, `linked: true`, the item's
`source`, `body` and `occurrence`), or takes the entities of an existing
linked projection of the same source, body and occurrence that is not
stale and still lies where the item is (`existingProjection`; logged
`Projected the body edge E{…} of F2.b0: c9` or `Reused the projection of
the body edge E{…} of F2.b0: c9`); the projection is part of the action's
one undo step, named after what the action makes.

- **Sketch Dimension** (`DimensionTool`) picks with
  `entityAt(screen, PickAll | PickBody)`: body vertices and the centres of
  body circles and arcs as points, body edges as curves, with the same
  pixel tolerances as sketch entities, which win ties (a body item counts
  one pixel further than it is). Hover and the picks are highlighted as
  sketch entities are (`SketchOverlay::paintEntityHighlight` looks body
  ids up in `bodyGeometryShown()`). `decide()` and the preview work on the
  tool's view of the sketch, the sketch with the items added as the fixed
  entities they project to (`BodyGeometry::addTo`; `ToolPreview::model`),
  so the dimension types follow as for sketch geometry. A body pick is
  logged `Sketch Dimension picked the body edge E{…} of F2.b0 (body:3)`.
  When the value is entered, one `SketchOp` projects each body pick
  (once each) and adds the dimension on the projected entities (a line's
  end by where the edge's is); a dimension between projected (fixed)
  entities only is added driven, as any over-constraining one. Esc,
  another tool or leaving the sketch before that changes nothing.
- An angle between two lines (mitcad#296) measures the sector of the four
  the lines' extensions make that the cursor is in: `decide()` writes
  `reverse_a`/`reverse_b` from `cursor - corner = s·da + t·db` (a ray
  reversed where its factor is negative), so the preview's arc, arrows and
  value follow the cursor and switch to the supplement across a line;
  while the value is typed, `decide()` keeps the placing click's sector.
  The overlay draws the arc, the extension lines and the default value
  place between the rays `angleRays()` gives (`SketchOverlay.hpp`); a
  dimension stored without the fields gets the sector whose angle is its
  value, of the two the one nearer its text. Moving the value afterwards
  only moves the text. After adding, the tool logs `Angle dimension k5:
  60.00 deg, the arc from 0.0 to 60.0 deg about 0.00,0.00` (the rays'
  directions in degrees, 0 to 360) for `tools/ui-sketch-angle-test.sh`.
- Signed horizontal and vertical distances (mitcad#409; commands.md,
  "Signed point dimensions"): with the palette's **Signed distances**
  (`paletteSignedDistances`, `SketchController::signedDistances()`, not
  saved) checked, `decide()` adds `"signed": true` to a
  `horizontal_distance` or `vertical_distance`, so the preview and the
  value editor show the offset `b - a` from the first pick (`measure()`
  keeps the sign of a signed one). Without it, a value the model refuses
  as negative for a new horizontal or vertical distance is added again
  signed in the same `SketchOp` (the point goes to that side). The
  overlay's text of a signed one has its sign, `+` too (`+25.98`,
  `fx: -15.00`). The sketch's context menu on a selected horizontal or
  vertical distance offers **Make Distance Signed** or **Make Distance
  Unsigned** (`SketchController::setDimensionSigned`, `sketch.set_signed`:
  the point stays, a value on the negative side is negated). Logged:
  `Added dimension k5 horizontal_distance signed d3 = r * cos(a)`,
  `Dimension k5 shows fx: +25.98`, `Signed distances on`, `Dimension k6
  unsigned: fx: 25.98 (-(r * sin(a)))`, and `Dimension texts: …` as a
  sketch with dimensions opens; `tools/ui-sketch-signed-test.sh` steps `a`
  through the four quadrants.
- The picks are geometry the model gives, not the view's shapes: Hide
  Above Sketch, which cuts what the view draws, leaves the edges in the
  plane pickable.
- **Snapping to bodies** (mitcad#258): a tool whose `bodySnaps()` is true
  gives the snapper the body geometry (`SnapSettings::body`), which then
  offers body vertices as `Snap::Kind::BodyVertex` (with the points' rank
  and tolerance) and the middles of body lines and arcs as
  `Snap::Kind::BodyMidpoint` (the midpoints'), the item in
  `Snap::bodyItem`; the sketch's own point or midpoint at the same place
  wins (a body target counts one pixel further). The marker is a point's
  square or a midpoint's triangle, the cursor shows the snap. Only the
  **Point** tool turns it on now; the other drawing tools can opt in
  without changes of their own, as `op.pointInput(snap)` gives a body
  vertex's projected point and `op.constrain(point, snap)` holds a point
  on the projected vertex (coincident) or at the projected edge's middle
  (midpoint), projecting the vertex or edge first (`projectBody`).
- **Point** (`PointTool`): on a body target its hint says what a click
  does; the click is one `SketchOp`: the vertex or the edge projected
  (linked, or an existing projection taken), `sketch.add_point`, and the
  coincident or midpoint constraint. Logged `Point p9 coincident with p8,
  the projected corner V{…}` or `Point p9 at the midpoint of c7, the
  projected edge E{…}` after `Added point p9 at (x, y)`. A point already
  coincident with the corner's existing projection makes the click do
  nothing, as at a point of the sketch.

### Sketch commands with panels

Offset, Mirror, the patterns, Project, sketch Fillet and Chamfer,
Move/Copy and Insert DXF into the edited sketch are feature commands with
`sketchCommand = true`: `build` returns a `sketch.*` command (or
`{"commands": [...]}`), the preview runs it on the document and undoes it
when an input changes, OK keeps it as one undo step, Cancel undoes it.
Their selection inputs take `SelectKind::SketchCurve`/`SketchPoint` of
the edited sketch (`ofSketch`, `linesOf` in `SketchCommands.cpp`; not
texts). While a panel is open, sketch mode's mouse handling pauses
(`SketchController::setPaused`); the window is in `Mode::Command`, but
the sketch is still what the view shows (`MainWindow::sketchShown`), so
Hide Above Sketch keeps the bodies cut through the panel, its previews,
OK and Cancel (mitcad#223). Drive Joint (outside sketches) runs its
`drive_joint` the same way. A sketch command's `warnings` are the panel's
message (not red) and the preview's log line (`Preview <name>: ok,
warning: ...`).

**Clean Up Projections** (`sketch.clean_up_projections`, CREATE's
References, mitcad#232) is such a panel with one input, Merge Overlapping
Lines (checked): its preview runs the command, whose warnings list what it
removes and merges, the links merged curves lose and the regions later
features use that change their names; OK keeps it as one undo step, Cancel
takes it back, and with nothing to clean up the panel says so and OK stays
off. The Sketch Palette lists the sketch's `warnings` (linked projections
that could not follow their sources) under the degrees of freedom
(`sketch::PaletteWarnings`), one label each. A warning the `sketch` query
gives entities for (`warning_entities`, mitcad#264: a projection that keeps
its geometry or lost its source, all its curves and points; else its stale
curves, else those that did not follow) looks like a link, underlined with
a pointing hand and the tooltip "Click to select the curves this is about";
a click selects them with the normal sketch selection
(`SketchController::selectEntities`, in place of the selection, so a second
click or a click on another warning replaces it, and Delete and the other
selection commands take it), and when none of them is in sight the view
pans to them, keeping its zoom unless they do not fit
(`view/ViewReveal.hpp`). Warnings without entities stay plain text.

**Clean Up Stale Projections** (`sketch.clean_up_stale_projections`,
CREATE's References, mitcad#252) is such a panel without inputs: its
preview runs the command and shows the sketch tidied (stale projected
curves that nothing uses gone, stale lines trimmed to the regions later
features use), and its warnings list the curves it removes, trims and keeps
(with why), the projections it unlinks and the regions later features use
that change their names; OK keeps it as one undo step, Cancel takes it
back, and with nothing stale the panel says so and OK stays off. While
the preview lasts the view highlights what it does: the removed curves as
they were and the pieces trimmed off, dashed in the remove colour, and
the trimmed lines in the pick colour (the answer's `stale.preview`, given
to `SketchController::setStalePreview` through the definition's
`sketchPreviewShown`, which a sketch preview calls with its answer and
with nothing when it is taken back; the panel's end clears it too). The
log says `Preview highlights: removed c8; trimmed c3, c7`, and
`Preview highlights: none` once they are gone. The Sketch Palette offers
the command under its warnings while a linked projection does not follow
(`kept`, `lost`, `unfollowed` or `stale` in the `sketch` query): a
Clean Up Stale Projections button (`paletteCleanUpStale`, a capsule in the
Floating layout), whose click starts it as the menu does.

**Convert to Sketch Geometry** (`sketch.convert_projected`, MODIFY after
Centerline, mitcad#257) is an `Action` on the selection, like
Construction: it runs `sketch.convert_projected` with the selected curves
and points of the edited sketch, one undo step, and logs `Converted to
sketch geometry: c9, p7` (and `; kept projected: p8` for shared points
that stay projected); the model's warnings go to the hint line, a refusal
to the error message. It is enabled only while the selection holds
projected geometry (`SketchController::selectionHasProjected`, its
`enabled`; `SketchController::updateConvertAction` refreshes the action on
selection and model changes, since the window refreshes the commands only
on model changes), and the sketch's context menu offers it on projected
geometry.

Points that become one (mitcad#259, mitcad#260): the Coincident tool's
picks of two curve ends merge them in the model; `applyEach` reads the
result's `merged` (`{"kept", "removed"}`), logs `Merged points p5 into p3
(coincident on p3, p5)`, uses the kept point for the remaining pairs of a
multiple selection and replaces the removed point in the selection.
Extend logs the point an end joined (`Extended c1 at (9, 0) (joined
p5)`).

A constraint that replaces one it makes redundant (mitcad#287): when
`sketch.add_constraint` answers with `implied_removed`, `SketchOp::run`
logs `Removed constraint horizontal k12: implied by the new coincident
k17` and the action ends with the hint `Horizontal removed: implied by
the new Coincident.` A constraint or dimension a snap, an inference or a
typed value adds through `SketchOp::attempt` is left out when the model
refuses it, as before, but now logs `Constraint coincident on c27, p40
not added: <reason>` and the action ends with the hint `Coincident not
added: <reason>.` These hints go through `SketchController::notice`
(logged as `Sketch notice: …`), which keeps them in front of the tool's
next prompt hint once.

Texts, patterns and offsets stay editable:

- **Text** (`TextTool`): a click places a text, a drag draws its frame (a
  construction rectangle, top aligned). A text is displayed as its glyph
  outlines (one `SketchEntityDisplay` per text, id `t<n>`, a compound of
  edges, also outside sketch mode); its letters are profiles.
- **Edit Text** (`sketch.edit_text`), **Edit Pattern**
  (`sketch.edit_pattern`) and **Edit Offset** (`sketch.edit_offset`) are
  panels on the picked text, pattern curve or offset curve; `init` fills
  them from `SketchModel::text`, `patternOf`, `offsetOf`. A double-click
  on such a selected curve opens its panel (`SketchHost::trigger`).
- A derived offset spline (of an ellipse or spline) shows only its ends
  as points: the query marks what the offset computes `derived`, and
  `SketchModel` drops the other control points (no snapping to them).
  Dragging or moving it carries its source along at the same distance.

Equal spacing (mitcad#221, `commands/EqualSpacingCommands.cpp`;
commands.md, "Equal spacing in sketches") is a constraint with a panel:

- **Equal Spacing** (`sketch.equal_spacing`, in CONSTRAINTS and the
  palette's constraint grid) and **Equal Spacing Dimension**
  (`sketch.equal_spacing_dimension`, the same panel with its Common
  Spacing Dimension checked) build `sketch.add_equal_spacing`. Sketch
  Dimension (D) with three or more points, circles, arcs or lines selected
  (only those) opens Equal Spacing Dimension instead of its tool
  (`dimensionEqualSpacing`): the spacing field shows the mean interval
  and has the keyboard, OK adds the constraint and its one dimension as
  one undo step.
- Inputs: Objects (an ordered list, `withOrdered`: its order is the
  chain's; points, lines, circles and arcs of the sketch), Measure
  (Horizontal, Vertical, Along a Line, Point to Point; the start chooses
  Horizontal or Vertical by the objects' spread), Direction (a line, for
  Along a Line), Keep Listed Order (directional modes: otherwise the model
  sorts along the direction and refuses ties, naming them), Chain in
  Listed Order (Point to Point: needed unless every object was clicked
  into the list in the panel, one by one, as the list's `onPick` records;
  a selection made before the command or a window has no order of its
  own, so the chain's order is confirmed explicitly), Common Spacing
  Dimension and Spacing.
- The preview runs the command, so the model's refusal (too few or
  unsupported objects, a tie, a pattern's copies, redundant or
  conflicting) is the panel's message before OK. `build` gives the
  objects to `SketchController::setSpacingPreview`; the overlay draws the
  previewed chain dashed, numbered 1, 2, 3 in the order the model keeps
  (`SketchOverlay::paintChain`), and the same for a selected or hovered
  equal spacing. Each anchor has the constraint's glyph (three ticks on a
  line); the `spacing` dimension is drawn on the first interval.
- **Edit Equal Spacing** (`sketch.edit_equal_spacing`) takes the
  constraint's glyph or one of its objects (also by a double-click on the
  selected glyph, and in the sketch's context menu when the glyph is
  selected or right-clicked: a right click on a glyph or a dimension
  selects it, as on geometry), fills the panel from the stored order (Keep Listed
  Order checked), and builds `sketch.edit_equal_spacing` with
  `sketch.add_dimension`, `sketch.set_dimension` or `sketch.remove` for
  the common spacing, one undo step.

## Browser, timeline and parameters

The browser (left) and timeline (bottom) are rebuilt from a
`DocumentSnapshot` after every model change (`MainWindow::refreshScene`,
only when the answers changed). They only show and emit;
`BrowserController` decides and runs model commands through
`DocumentHost::runModelCommand` (refreshes, logs volume changes) or
`runModelCommands` (one undo step). Model changes need the idle mode
(`canChangeModel`); other browser actions in another mode say "Finish the
command or the sketch first."

The light bulbs of bodies, the Bodies, Sketches and Construction folders,
sketches, construction geometry, components and the Origin folder work in
every mode (mitcad#295, `BrowserController::visibilityInEveryMode`): while
Create Sketch asks for a plane, while sketching and during commands; a
Flat Pattern's and an analysis's only where the model can change. They go
through `DocumentHost::runVisibilityCommands`; during a command
`CommandSession::runVisibility` (`framework/CommandSessionVisibility.cpp`)
keeps them out of the command's undo range: an applied sketch preview is
taken back before and applied again after, and what takes the range back
later (Cancel; OK of an edit, whose roll-back is undone; the asynchronous
checkpoint restored) runs the recorded changes again, so a cancelled
command leaves them as the user set them and a committed one is one undo
step of its own after them. The inputs stay selected (one on a body hidden
meanwhile is not highlighted), the preview is drawn again over the bodies
shown now from its cached result (computed again only when a body shown
now was changed by the command and was hidden when it was computed), and
the active input's pick places are logged again. An asynchronous command
refuses a change while it computes. Logs: `Visibility <path>: hidden
(during <command>)` (the command's name, `Create Sketch` while it asks for
a plane, the sketch's name while sketching), `Command <name>: <n>
visibility change(s) kept` (they were run again after its roll-back or
checkpoint was taken back), `Visibility during <command> refused: it is
computing`. Tests: `tools/ui-visibility-test.sh`,
`tools/ui-windows-visibility-test.ps1` (ctest `app.ui-windows-visibility`).

- **Rows:** a row's `SelectionItem` (`BrowserNode::item`) carries the
  occurrence path. Light bulbs: `set_body_visible`,
  `set_occurrence_visible`, `set_feature_visible`, `set_origin_visible`
  (commands.md, "Visibility and display"); a folder sets all of its own as
  one undo step. The model decides what is shown (the `timeline` query's
  `visible`, `DocumentSnapshot::featureShown`): a sketch whose bulb was
  not set is hidden while a feature uses it. The view shows only what is
  shown (`instances` leaves hidden bodies out). A fully constrained
  sketch's icon is `sketch-locked` (the `timeline` query's `dof`).
- **Features under sketches** (mitcad#177): each feature that uses a
  sketch (the `timeline` query's `sketches`) has a row
  (`BrowserNode::Type::Feature`, keyed by feature and sketch) under each
  sketch row it uses, in timeline order; sketches, construction geometry
  and joints keep their own rows only. Sketch rows with such rows start
  collapsed. Feature rows have no light bulb (hiding a sketch hides only
  it), are grey when suppressed or after the marker and red when failed,
  and a feature whose bodies are in another component than its sketch (a
  new component) carries a component badge (`LinkedIconEngine`) with that
  component in its tooltip. Their menu is the timeline's
  (`BrowserController::featureMenu`, shared), with Rename in place and
  Find in Timeline; a double-click edits, Delete deletes the feature.
  Test: `tools/ui-browser-features-test.sh`.
- **Collapse All** (mitcad#312): the last entry of every browser row's
  context menu (`BrowserController::browserMenu`, `featureMenu` for feature
  rows; not the timeline's) calls `BrowserPanel::collapseAll`, whichever row
  was clicked. It folds every row below the top row, those under an already
  folded row too, and remembers them as folded (`foldBelowTop`: `m_expanded`
  is cleared, then filled with `false` for each row), so a rebuild does not
  reopen anything, also not the folders that open by default. The top row
  (the root) is opened and remembered open (mitcad#426): its children stay
  in sight, folded; its own arrow still folds it. An arrow click then opens
  only that row (the tree never opens recursively); rows below stay folded
  until their own arrows are clicked. A presentation action: it runs no
  model command and changes no visibility. Logs: `Browser collapsed all`;
  `logLayout` also logs each row with children's arrow as `Browser arrow
  <path> at x,y`. Test: `tools/ui-browser-collapse-test.sh`.
- **A design as it opens** (mitcad#426): every design that comes to the
  window (`MainWindow::installDocument`: Open, a version, a copy, a
  recovered document, an import of any format, New) opens with the root
  active, whatever active component its file stores: right after reading,
  on the worker, `activate_component` with `"opening": true` (no undo step,
  no new revision, so a design opened unmodified stays unmodified; the
  file stores what is active when it is saved, as before). Logged when the
  file had another one active: `Opened with the root active; C1 was
  active in the file (unmodified)`. Then `BrowserController::designOpened`
  makes the browser rebuild with the rows' default states
  (`BrowserPanel::designOpened` forgets what the design before left open)
  and, once the panel shows with its size (a design opened at the start
  is read before the window shows),
  counts the rows in sight against `BrowserPanel::availableHeight` (the
  rows' viewport; the Floating layout gives the card's largest height,
  `FloatingLayout::maximumContentHeight`, with `setAvailableHeight`; the
  viewport again while that is not known yet). More rows than fit: the top
  row stays open and every row under it is folded and remembered so
  (`foldBelowTop`, as Collapse All), the root's children in sight with their
  arrows closed; otherwise the rows stay as they opened. Only at the
  opening: later rebuilds (a new component, an undo, a recompute) keep the
  rows as the user left them. Logs: `Browser opened folded: <n> rows would
  not fit (<visible> visible)`, `Browser opened expanded: <n> rows
  (<visible> visible)`. Tests: `tools/ui-browser-collapse-test.sh`,
  `tools/ui-windows-browser-open-test.ps1`.
- **Background in the Floating layout** (mitcad#419,
  `browser/BrowserBackground.*`): Preferences › Display › Browser
  background, the setting `appearance/browserBackground`, shown only in
  the Floating layout and applied at once (OK, or as it changes in the
  macOS Settings window). **Card** (`card`, the default): the Browser on
  its card, the rows as a sidebar's (`SidebarDelegate`). **Rows** (`rows`):
  the card is bare (`GlassCard::setBare`): no material and no shadow; its
  header is a small box of the card's material as wide as its chevron,
  title and buttons, which follow the title; each row in sight is on a box
  of its own (`BrowserTree::rowBox`, painted in `drawRow` under the row's
  arrow), from its arrow's place to the end of its name or radio button and
  2 px short of the row at the top and bottom, tinted with the accent colour
  when selected and lightly under the pointer. The card's mask
  (`GlassCard::updateBareMask`) is the header's box and
  `BrowserPanel::rowsRegion` (the rows' boxes, the scroll bars shown, a
  rename editor), kept up to date as the rows scroll, open, fold, change,
  are selected or the card resizes or folds: a click, drag or wheel
  anywhere else in the card's rectangle reaches the 3D view (select,
  orbit, zoom), as beside a card. The view's covered area for fitting and
  pick places stays the card's rectangle. With native glass the card is a
  widget over the view while bare (`GlassCard::setWindowed(false)`), a
  window again with Card. The timeline and the command panel keep their
  cards. Logs: `Browser background: rows` (`card`; at start and on each
  change), `Preferences: browser background rows`, `Preferences browser
  background at x,y`, and with Rows `Browser view beside the rows at x,y`
  (right of every row's box at the middle row in sight; `… out of sight`
  when it goes, as Card comes back). Tests: `tools/ui-browser-rows-test.sh`,
  `tools/ui-windows-browser-rows-test.ps1` (ctest
  `app.ui-windows-browser-rows`).
  With Rows the vertical scroll bar is at the card's left edge, beside the
  rows' start (mitcad#425, `BrowserPanel::setScrollBarOnLeft`, called by
  the Rows switch): a bar of the panel's own that mirrors the tree's (the
  tree's own is off; the wheel and the keys still scroll it), shown while
  the rows overflow, the tree right of it, and in `rowsRegion`; with Card
  the tree's bar stays at the right. Logs: `Browser scroll range 240` (the
  tree's vertical range, 0 while every row fits) and `Browser scroll bar
  at x,y` (the vertical bar in sight, either one; `… out of sight`).
- **The Browser card's height** (mitcad#425, the Floating layout on every
  platform): the card is as high as its rows in sight
  (`BrowserPanel::contentHeight`) and grows down as rows open, up to the
  card below it (the timeline, or the view's bottom margin when the
  timeline is hidden) less the gap, and only then scrolls; it shrinks as
  rows fold, and is placed again when the window resizes (the Rows mask
  follows). `FloatingLayout::maximumContentHeight(content)` gives the
  height the content of a card may take at its place before it scrolls
  (here `m_floating->maximumContentHeight(m_controller->browser())`; -1
  before the first layout). Its height is in the `Floating cards:` line
  (`browserDock`).
- **Rename in place:** F2, the menu's Rename, or a slow second click on
  the name of the selected, current row (`BrowserTree`: after the
  double-click interval, not in a double-click or a drag; the timer is
  watched by `TestSync`). Bodies, components, sketches, datums, joints,
  analyses and named views rename this way too.
- **Paste / Paste New** place the copy where the original is and open
  Move/Copy on it (`DocumentHost::startCommand` with the occurrence
  selected); OK keeps the moved copy as its own placement
  (`set_occurrence_transform`, not a `move_occurrence` feature), paste and
  placement one undo step; Cancel takes the paste back.
- **Hybrid assemblies** (mitcad#171, `framework/HybridAssembly.cpp`):
  before a component is made in the root while the root has modelled
  bodies, `askHybridAssembly` asks (title `Hybrid Assembly`): Move into a
  New Component (default; `move_to_component` with everything), Continue
  as Is, Cancel, and "Don't ask again for this design"
  (`set_hybrid_assembly`). It is called by the browser's New Component
  and Create Components from Bodies (`BrowserController`), and on OK of a
  `modelCommand` whose commands make a component (`create_component`,
  `components_from_bodies`) or of a new feature with the `new_component`
  operation (`CommandSession::askBeforeComponent`, both the synchronous
  and the asynchronous commit). Cancel leaves the panel open. For Create
  Components from Bodies, Move moves each of the root's selected bodies
  with its features into its own component (`groups`) and the command is
  done; a new feature whose references moved (`feature_references`
  query) is added to the new component (`add_feature`'s `component`).
  Move to Component (a body's row, a sketch's row (mitcad#218), a
  timeline feature: `bodies` or `features`) and Move Features into a
  Component (a component's row) run `moveToComponent`. Its dialog always asks for an existing destination or
  Create New, with a name and parent, before running a command. Component
  paths and occurrence identifiers distinguish repeated names; browser
  rows also supply `source_occurrence`. The plan lists expanded sketches,
  features and bodies, shows target problems, and offers Keep Them Together
  (`together`) for conflicts. Cancel makes no model change; a failed move
  leaves the dialog open. These explicit moves send `destination`; Hybrid
  Assembly keeps its automatic new-component behavior. Move the Features
  That Made It into the Component uses `move_history_into_component` for a
  `component_from_bodies` feature. Log lines: `Hybrid assembly question: Root
  has Body1`, `Hybrid assembly: move|continue as is[, don't ask again]`,
  `Moved Sketch1, Extrude1 into Component1`. Test:
  `tools/ui-hybrid-test.sh`; explicit destination dialog checks: `app.unit`.
- **Move to...** on a component occurrence's row (not the root; mitcad#219,
  `framework/MoveOccurrenceDialog.cpp`, `BrowserController::moveOccurrence`):
  another parent for the occurrence with everything in it, where it stays
  in the design. The dialog (title `Move Component`) reads the
  `reparent_occurrence` query: the root and every placed component by full
  path, the current parent marked, unavailable parents grey with their
  reason (selectable, so that the reason shows); what is in the occurrence
  and, for the chosen parent, the references that follow it or the
  blockers, and the features that place it and go with it into the new
  parent (mitcad#301: `Joint1 moves from Box to Root`, or `Part:1's part
  of Move1 moves to Move2, a new feature of Root` when a feature is split).
  Move runs the `reparent_occurrence` command (one undo step) and
  selects the occurrence's row at its new path; a refused command leaves
  the dialog open; Cancel changes nothing. Log lines: `Move Component
  parents of Part:1: Root | Root/Assembly:1 (unavailable) | …`, `Move
  Component features: Joint1 -> Root` (`Move1 -> Move2 in Root`), `Move
  Component parent: Root, ok` (or `…, unavailable: <reason>`), `Moved
  Part:1 to Root`, `Move of Holder:1 cancelled`. Tests: `app.unit`
  (`tests/move_occurrence.cpp`), `tools/ui-move-test.sh` (also the sketch
  row's Move to Component, and a component a joint places).
- **Selection:** rows set the selection outside commands
  (`DocumentHost::pickItems`); in a command a row goes to the active input
  like a click in the view; while Create Sketch waits for a plane, a plane
  row starts the sketch. Browser and timeline follow each other
  (mitcad#177): the features of the selected rows (sketches, datums,
  joints, feature rows) are selected in the timeline through
  `TimelineWidget::setSelectedFeatures`, which emits nothing; a feature
  row selects what its timeline cell does (`featureItems`); a click in the
  timeline selects the feature's rows (`BrowserPanel::showSelection` with
  the features, also silent; `BrowserController::pick`). A click in the
  view leaves the timeline as it is.
- **Occurrences in the view:** a body placed by an occurrence is displayed
  with the occurrence's transformation on its AIS object
  (`BodyDisplay::placement`), so picks name the body's own faces, edges
  and vertices and carry `SelectionItem::occurrence` ("O1/O4"; empty in
  the root). Sketch curves, profiles and datums of a component are placed
  by its first occurrence (`MainWindow::placingOccurrence`).
  `MainWindow::itemShape` places highlights by the occurrence;
  `SelectKind::Component` and `SelectKind::Sketch` are highlighted as
  their bodies and curves.
- **Timeline:** status `warning` (message in `error`) is yellow, failed
  red. A feature that made no body of its own (fillet, hole, join) is
  selected as a `SelectKind::Feature` item placed by its component's first
  occurrence; `itemShape` gives the faces whose names start with `<uid>:`
  on the bodies at the marker. `pickItems` and `keepSelected` leave out a
  feature without such faces. Delete asks with the `dependents` query.
  Dragging the marker runs `set_marker` per crossed feature and merges the
  steps into one `Move Timeline Marker` (none when it ends where it
  began). Dragging a feature checks `can_reorder`. Groups are the model's
  (`group_features`); folding is the window's (`TimelineWidget`
  `m_collapsed`), not saved. So is the filter (mitcad#98,
  `TimelineWidget::setFilter`, the filter button and the browser's Show
  in Timeline Only): hidden features take no room (`Item::filtered`), the
  marker and drops land on `shownBoundary` places, and the playback
  buttons step between them; it logs `Timeline filter: <label>: n of m
  features`. Test: `tools/ui-component-sketch-test.sh`. Colours of the marker, buttons and strike
  line come from the palette and are repainted on a palette or style
  change (`TimelineWidget::changeEvent`). Test: `tools/ui-theme-test.sh`
  (the scheme switched with `MITCAD_TEST_COLOR_SCHEME`).
- **The import's outcomes** (mitcad#170): what an importer said about a
  feature is the `timeline` query's `import` (also first in `error`,
  `DocumentSnapshot::Feature::importNote`). A fallback's base feature and
  an item that came in partly have the status `warning`, so they are
  yellow, the note in the tooltip ("Stands in for Extrude5 (its result is
  not in the file's history)", "Partly imported: left out: ..."). A
  placeholder (type `placeholder`, the `feature` icon) of an item the file
  had suppressed or failed is drawn suppressed with a yellow outline, its
  tooltip "Suppressed: <the file's state>"; so is an `.f3d` item
  suppressed in the file that came in as its own feature (mitcad#180). The failures badge counts only
  features that failed; beside it a yellow badge (`BrowserController::
  warnings`, in the status bar in the docked layout and the browser card's
  header in the floating one) counts the features of
  `DocumentSnapshot::warnings` (status `warning` or an import note, not
  failed) and goes to them like the failures badge: to the one, or
  through a menu of them. An edit of the feature that builds clears its
  note (the model's). Test: `tools/ui-import-status-test.sh`.
- **Timeline search** (mitcad#179): the timeline's search button, Ctrl+F
  (`QKeySequence::Find`, Cmd+F on macOS) in `keyPressEvent` of the
  timeline's strip and the browser's tree (`BrowserPanel::searchRequested`;
  no window shortcut, so it acts only where the timeline or the browser
  has the keys), and `view.search_timeline` (Edit › Search Timeline, the
  command search; disabled in the native macOS UI, whose history is its
  own) call `TimelineWidget::openSearch`: a field after the filter button
  (`m_searchBar`: field, "n of m", previous and next), in both layouts
  (the dock and the floating card hold the same widget). The text is read
  by `TimelineQuery` (`browser/TimelineSearch.*`, plain Qt Core, unit
  tested in `app.unit`): words the name or the type must contain, `in:`
  (`component:`) and `is:` (`status:`) filters with abbreviations, an
  unknown status matching nothing. It searches the features the
  component filter shows (`Item::filtered`); `is:warning` counts as the
  warnings badge does (`DocumentSnapshot::warnings`), `is:rolled-back` is
  after the marker. Matches are outlined (`TimelineStrip::setFound`,
  `kFoundBorder`) and listed with their component's colour and name in
  `m_searchList`, a `QTreeWidget` child of the host window
  (`GlassCard::hostWindow`) placed over the field, without focus. The
  field takes Enter, Esc, Up and Down before the window's OK and Cancel
  (ShortcutOverride): Enter and Down step forward, Shift+Enter and Up
  back (wrapping), Esc closes; a step is `selectFeature` (scroll, a folded
  group opened) plus `featuresClicked`, as a click. When the text becomes
  non-empty the scroll position and the timeline's selection are kept and
  `searchStarted` lets the controller keep the window's selection; when
  it becomes empty after a step and the step's match is still the
  selection, both come back and `searchRestored` makes the controller
  pick its kept selection again (outside commands). Test:
  `tools/ui-timeline-search-test.sh` (both layouts).
- **Find in Design** (mitcad#350): `view.find_in_design` (Edit › Find in
  Design..., the command search) and the magnifier button
  (`BrowserController::findButton`) on the browser's title row (in the
  floating layout the first widget of the card's header; in the docked
  one a title bar of the dock's own, `MainWindow::createPanels`) call
  `BrowserController::openFind`: `FindDialog` (`browser/FindDialog.*`), a
  non-modal window over the main window. The search is
  `findInDesign` (`browser/FindInDesign.*`, plain Qt Core) on the
  `DocumentSnapshot` (occurrences, components, bodies and their names), not
  on the browser's rows: the root's bodies, then each occurrence with its
  bodies and children; a body of a component placed twice is a result per
  place (`FindResult::key`). `NameMatcher`: a case-insensitive substring,
  `*` and `?` as wildcards (still unanchored). Text, kinds and visibility
  filter the results after a 150 ms pause (`TestSync::watch`'d timer); the
  window keeps the selected results through a re-search
  (`BrowserController::refresh` calls `setSnapshot`, which ignores an equal
  snapshot key). The buttons act on the selected rows, or on all:
  Show and Hide run `set_body_visible` per body uid and
  `set_occurrence_visible` per occurrence path, each once, through
  `DocumentHost::runVisibilityCommands` as one undo step ("Show Found
  Items" / "Hide Found Items"); Appearance... calls `openAppearances` with
  the bodies (a component's: its definition's bodies); Select and
  selecting rows call `DocumentHost::pickItems` with body items (uid and
  occurrence) and component items (the component and its occurrence; not
  during a command); a double-click calls `findInBrowser`. All of them
  wait for a running job (`whenIdle`). Test:
  `tools/ui-find-in-design-test.sh`.
- **Analysis folder** (mitcad#41): the analyses kept in the document (the
  `analyses` query, `DocumentSnapshot::analyses`), after the root's other
  rows. Light bulb: `set_analysis_visible`; showing one hides the others,
  as one section cuts the bodies at a time. Double-click and Edit Section
  Analysis open its panel with its plane, offset and flip
  (`DocumentHost::editAnalysis`); Rename, Delete (`rename_analysis`,
  `delete_analysis`); the folder's menu starts a new Section Analysis. An
  analysis whose plane cannot be found at the marker is red with the
  reason in its tooltip, and cuts nothing. Remove Section Analysis hides
  the shown analysis (it stays in the folder); Flip Section Analysis edits
  it with `flip` toggled.
- **Section coordinates** (mitcad#204): preview, the manipulator and saved
  analyses share the model's `section_plane` resolver. A selected component
  face or construction plane keeps its occurrence path, including when an
  analysis is reopened. Distance is applied along the world normal before
  Flip reverses it. Each displayed body converts the same world plane to
  its local coordinates before clipping and keeps its usual placement;
  caps use those clipped faces. Section area and outline length count
  visible occurrences. Regression: `tools/ui-macos-section-test.sh` checks
  the actual retained volumes and world bounds, plus screenshots.
- **Edit Appearances** (`solid.appearances`, mitcad#46; also Appearance...
  in a body's browser menu and in the view's menu of bodies, faces, edges
  and vertices): the `appearances` query's library and the design's own
  appearances with swatches, the selected one's preview and parameters
  (`AppearanceDialog`, modeless, refreshed with the browser). The
  library's are read-only; New is `create_appearance` with `based_on` the
  selected one (named "<name> Copy"), each field change one
  `edit_appearance` undo step (colours as `#rrggbb` or from a colour
  dialog), Delete `delete_appearance`, Assign `set_body_appearance` for the
  bodies it was opened for (the selection's) as one undo step, and
  `set_face_appearance` for its faces (mitcad#53: the dialog opened on
  selected faces assigns to them; on edges and vertices, to their
  bodies). Assign to N bodies (mitcad#346), shown beside Assign while the
  selection holds faces: `set_body_appearance` for each body that owns a
  selected face or is selected, once, as one undo step; the faces' own
  appearances are untouched. The texture (mitcad#53): Image (typed, or Choose... with a file
  dialog; made relative to the design's folder when it is saved), Size,
  Rotation (degrees), Projection (Box, Planar), Embed the image in the
  design (reads the file into `data`; off again needs the file at its
  path), Remove; each one `edit_appearance` with the texture as the
  `appearances` query lists it, so an embedded image stays; a line says
  when the image is missing. "Faces with their own appearance" lists the
  face appearances of the bodies it was opened for (`bodies` query's
  `face_appearances`; a face not found is marked), and Clear gives the
  selected ones their body's again (`set_face_appearance` with null, one
  undo step). The Appearance panel (`solid.appearance`) lists the same
  appearances (`withChoices`) and has a Faces input besides Bodies
  (Default clears a face's own). The view colours each body with the
  appearance's `display_color` and faces with theirs
  (`MainWindow::modelBodies`, `BodyDisplay::faceLooks`, an
  `AIS_ColoredShape` in the shaded styles); `BodyDisplay::appearance`
  carries the parameters to the rendered view, and a change of them alone
  emits `bodiesChanged` without redrawing the shaded body.
  Texture sources (mitcad#315, `app/materials/`): a selector above the
  list chooses the appearances (the list; the search field and the
  category, All, Library or This Design, hide what does not match) or a
  registered `TextureSource` (`textureSourceRegistry()`; now Local
  Folders, `FolderTextureSource` over the folders of Preferences ›
  Textures, the setting `textures/folders`). A source's search fills
  `TextureBrowser`'s grid (thumbnails asked of the source as they come
  into sight and kept in the texture cache); its categories fill the
  filter; the chosen asset's resolutions (`textures/resolution`
  remembers the choice, 2k at first) show the bytes they take; its
  attribution line, when it has one, is under the grid. **Use** fetches
  the asset's maps in the resolution and sends one `create_appearance`
  (`textureUseCommand`): the asset's name (made unique), the base colour
  image's average as `base_color`, and a texture with each map's file
  name and `data`, or `{"path", "embedded": true, "image_sha256"}` for an
  image the design embeds already (`embeddedImageDigests` of the
  `appearances` query), `size` the asset's or 100 mm wide in the image's
  proportions, and `source` (`id`, the asset's recorded id, `resolution`,
  `license`, `url` when known); the new appearance is selected for
  Assign. The model never sees a source or the cache. A source that uses
  the network is asked only once the user allows it (asked on its first
  selection; No goes back to the previous source and asks nothing;
  `textures/allowNetwork`, Preferences › Textures). Choosing an asset
  asks the source for its details once (`TextureSource::details`: an
  online source's real resolutions and their bytes).
  The texture's own lines list the set's maps and its source.
  **Poly Haven** (mitcad#313, `PolyHavenTextureSource`): the CC0 textures
  of https://polyhaven.com through its public API
  (`https://api.polyhaven.com`): `/assets?t=textures` gives the list
  (name, tags, categories, `dimensions` in mm as the texture's size,
  `max_resolution`, the thumbnail's address, `files_hash`; kept in the
  cache as `polyhaven/textures.json` for a day, and used however old when
  the API cannot answer), `/files/<id>` the chosen asset's maps per
  resolution (`Diffuse`, `nor_gl`, `Rough`, `Metal`, `AO`; JPEG where
  there is one, else PNG; kept as `polyhaven/<id>/files-<files_hash>.json`),
  the thumbnails come as PNG in the size shown (`thumbnail-96.png`, a
  month), and Use downloads only the chosen resolution's maps into
  `polyhaven/<id>/<resolution>/`, checked by size and MD5 (kept for a
  later Use). `source` records `polyhaven`, the id, the resolution,
  `CC0-1.0` and `https://polyhaven.com/a/<id>`. The requests go through a
  `TextureFetcher` (`NetworkTextureFetcher`: Qt Network, HTTPS only,
  User-Agent `Mitcad/<version>`, no cookies, 30 s without data ends a
  request, nothing while the network is not allowed); failures (no
  network, a timeout, an HTTP error, an unexpected answer) are sentences
  in the browser and leave the other sources working.
  `MITCAD_TEXTURE_FIXTURES` answers the requests from files
  (`<folder>/<host>/<path>`) for UI tests.
- **Change Parameters** (`solid.parameters`): each cell change is one
  model command and undo step; a bare number gets the parameter's unit;
  refusals stay in the cell in red with the reason. Favourites are
  `set_parameter` `favorite`. `--set name=value` sets a value the same way.

## Joints

The ASSEMBLE group of the SOLID tab (mitcad#55, `commands/JointCommands.cpp`;
the model's side in commands.md, "Joints"). A joint goes into the active
component; an edit keeps the joint's own (a hidden `component` input). Its
origins are geometry of occurrences inside that component: picks carry
paths from the root (`O1/O4`), which `build` makes paths from the
component, and `load` makes them paths from the root again through the
component's first occurrence. A pick outside the component fails the
build with a message.

- **Joint** (`assemble.joint`, J): Origin A and Origin B (faces with a
  plane, axis or centre; straight and circular edges and sketch curves;
  points, axes and planes: `givesFrame`), Type, Slide Along (sliders),
  Offset, Angle, Flip, and per free motion of the type a Limits check
  (Minimum, Maximum) and a Rest check (Rest Value). The origin inputs
  leave the origin's planes hidden (`showsOrigin = false`), as they would
  cover the components. Offset's arrow and Angle's ring stand on the frame
  the model resolves origin B to (else A): the `joint_frame` query, so a
  pick shows where its origin snaps. A driven `position` is kept through
  an edit (hidden input) for the motions the type still has; limits with
  a minimum but no maximum cannot be edited (`canEdit`).
- **As-Built Joint** (`assemble.as_built_joint`): Component A and B (in the
  browser, or bodies they place), Type, Motion Origin (optional), limits.
  `relative` is recorded from the `components` query's placements (B's
  inverse times A's); an edit keeps the recorded one while A and B stay.
- **Joint Origin** (`assemble.joint_origin`): geometry of the active
  component itself, Offset, Angle, Flip; a `frame_override` is kept.
- **Rigid Group** (`assemble.rigid_group`): two or more components; a pick
  inside a sub-assembly stands for the occurrence placed in the component.
- **Drive Joint** (`assemble.drive_joint`): a joint (a feature: the
  timeline, the Joints folder, or the newest drivable one), its Motion and
  the value (Angle or Distance, with a ring or an arrow on the joint's
  frame B). It runs `drive_joint` while the inputs change, as sketch
  commands do (`sketchCommand`), and OK keeps it: one undo step `Drive
  Joint1`. Choosing another motion or joint takes its value where it is
  (a choice's `onChange` may set values; those it leaves NaN are evaluated
  again).
- **Constraint** (`assemble.constraint`, mitcad#148; the model's side in
  commands.md, "Assembly constraints"): a `constraint_set` in the active
  component, its rows a `listInput` (`rows`, Add Constraint): Geometry 1
  and Geometry 2 (as Joint's origins: `givesFrame`), Type, and per type
  Second Face (center), Offset (align, insert), Angle (angle), Flip,
  Lock Rotation (inserts), Limits with Minimum, Maximum and Rest (align:
  lengths, angle: angles). Type's first option is Automatic, labelled
  with the type the picks fit best (`Automatic (Insert)`), from the
  model's `constraint_row` query (remembered per pair of picks), which
  `build` writes; the other options set it. Geometry 1's occurrence is a
  ghost until the row's Geometry 2 is picked (`ghostUntilPicked`, the
  row's state). A locked insert records the turn where the picks are (the
  query's `twist`); an edit keeps the recorded one (hidden
  `lock_angle`). The preview solves the rows with the joints and
  constraints before them and shows the moved components; OK logs
  `Added constraint Constraint1: insert placed, moved O2; DOF 1`. In the
  browser a set is a row of the Joints folder (icon `constraint`) with
  its rows under it (`Insert (Pin:1, Plate:1)`, red when a row does not
  hold at the marker); the folder's menu has New Constraint.
  `DocumentSnapshot::joint` gives a set as a joint of the kind
  `constraint`, so drags name it. Test: `tools/ui-constraint-test.sh`.
- **Animate Joint** (`assemble.animate_joint`, `MainWindowJoints.cpp`): the
  joint selected in the timeline, else the newest with free motions, its
  first motion through its limits (from where it is to one end, the other
  and back) or once round without limits (a slide ±25 mm): 36 frames, each
  the `preview` of an `edit_feature` with the joint's `position` there,
  shown through the preview's `placements`; the preview is cleared at the
  end and nothing changes. Starting a command stops it.
- **Ghost of side A** (mitcad#147): once Joint's origin A or As-Built
  Joint's component A is picked, and while B is not, the occurrence placed
  directly in the joint's component that A lies in is drawn as a ghost
  (`withGhosts`), so that B can be picked behind it; A's highlight stays
  shown but takes no clicks (A is deselected by activating its input, which
  shows A normally again, or with the input's clear button). Once B is
  picked the preview shows A where the joint puts it, normally. Logged as
  `View ghosts: O2 (1 body)` and `View ghosts: none`.
- **Previews of placements:** a feature command's preview shows the
  bodies of the occurrences it places elsewhere where the preview's
  `placements` put them (joints, Move/Copy of components; `Preview <name>
  moves O2`).
- **Dragging components** (`OcctViewer::setOccurrenceDrag`,
  `MainWindowJoints.cpp`): in idle mode, a left drag without keys that
  starts on a body moves the innermost occurrence of its path whose unit
  in its parent component has joints, is not grounded and can move (the
  `joints` query's unit: `dof` above 0, or `mobile`, as a link of a
  four-bar linkage); a click on such a body still picks. The pointer's
  point is on the plane through the grabbed point facing the viewer; per
  move the `joint_drag` query gives the occurrences' placements, which
  move the shown bodies; the release runs `drag_occurrence` (one undo
  step `Drag Pin:1`, after the mouse event: `whenIdle`). A dialog that
  takes the mouse cancels it.
  One function decides what a drag on an item does (`dragAnswer`,
  mitcad#150), so the hint and the refusal agree:
  - **Hint:** the pointer over a body that would drag shows the move
    cursor (`OcctViewer::setDragCursor`) and the status bar hint `Drag to
    move Pin:1 (Joint1: rz)` (its unit's joints and their free motions;
    log `Drag hint: …`, `Drag hint: none` when it goes).
  - **Refusal:** in a design with components, a left drag without keys
    that starts on a body that does not drag is neither a drag nor a
    window selection: the status bar says why (log `Drag refused: …`).
    The reasons, the innermost occurrence's first: `Base:1 is grounded`;
    `Clip:1 is in a rigid group with Base:1, which is grounded`; held
    fixed by its joints (`dof` 0 and not `mobile`): `Fixed:1 cannot move:
    Joint2 is rigid` (several: `Joint2 and Joint3 are rigid`, other kinds
    `Joint2 (revolute) and Joint3 (slider) hold it`); no joints of its own
    in its parent component (components do not drag freely): `Plate:1 has
    no joints in its parent component`, for a body of a component with
    several bodies `Body3 is a body of Plate: bodies move only with their
    component, and Plate:1 has no joints in its parent component; Create
    Components from Bodies makes one component per body`; a body of the
    root: `Bodies of the root component do not move`. A body that would
    drag but not now: the read-only window's reason, `Finish the sketch to
    drag components`, `Close the command to drag components`, `Wait for
    the computation to finish to drag components`. In a command or a
    sketch, a drag on a body that would not drag anyway selects with a
    window as before; so does a drag with keys (Shift), and every drag in
    a design without components. The test: `tools/ui-drag-test.sh`.
- **Browser:** each component's **Joints** folder lists its joints,
  as-built joints, rigid groups and joint origins before the marker
  (joint origins are datum rows with light bulbs; their datum is a plane).
  A joint's tooltip has its kind, state and values, the folder's the
  component's degrees of freedom (the `joints` query's `dof`), an
  occurrence's what its joints leave it. A joint's menu: Edit, Drive,
  Animate, Suppress, Rename, Delete, Find in Timeline; the folder's: New
  Joint, New As-Built Joint, New Rigid Group. A joint row stands for its
  feature (`SelectKind::Feature`), which Drive Joint takes.

## Sheet metal

The SHEET METAL group of the SOLID tab (mitcad#210,
`commands/SheetCommands.cpp`, `MainWindowSheet.cpp`; the model's side in
commands.md, "Sheet rules and base Face" to "Sheet-metal command UI and Flat
Pattern DXF"). Picks on a sheet body are faces and edges; `build` asks the
model's `sheet_reference` (and for two panels `sheet_seam`) what they stand
for, and the definitions keep the semantic references (panels, bends,
boundaries, corners); `load` makes the faces and edges that carry those names
again (a wall `F3:sheet_side0(F2:sheet_curve(c2[c1,c3]))`, a panel's
`sheet_front(<slot>)`, a bend's `sheet_bend_front(<slot>)`, a corner's edge
between two walls), so an edit highlights what the feature uses.

- **Face** (`sheet.face`): Profiles (the newest unused one), Rules: New Rules
  (the default while the design has none: Thickness, Bend Radius, K-Factor,
  Sketch Side; reliefs at round 1 x 2 mm) or one of the design's rules
  features (the newest is preselected once there are some). New Rules is an
  inline definition the model adds as a `sheet_rules` feature just before the
  Face, one undo step; an edit lists only existing rules.
- **Flange** (`sheet.flange`): a straight edge of a flat face or its side face,
  Angle with Flip (the angle's sign), Length and where it is Measured
  (tangent, inner, outer), Position (bend line, inside, outside), Offsets from
  the Ends, Own Bend Radius, Own Bend Relief.
- **Bend** (`sheet.bend`): Fixed and Moving: two flat faces of one sheet (their
  common seam, `virtual_sharp`; where the kernel joined their caps into one
  face, a click on each side of the seam) or an edge of each at the bend
  (placed sheets, `tangent_ports`; another body becomes `moving_body`), Angle,
  Flip, overrides. Bend Type Double Bend (mitcad#288): an edge each of two
  sheets in parallel planes joined by a jog (`port_mode: "double"`), Whole
  Face (`fixed`: the Fixed or the Moving edge's face keeps all its material),
  the angle of each of the two bends without Flip; editing a double bend
  loads it as one.
- **Fold** (`sheet.fold`): the flat face, a sketch line on it, Fixed Side,
  Angle, Flip, Position, overrides.
- **Corner Round**, **Corner Chamfer** (`sheet.corner_round`,
  `sheet.corner_chamfer`): edges through the thickness at corners (between two
  walls), Radius or Distance 1 and 2.
- **Sheet Cut** (`sheet.cut`): Profiles (of one or several sketches on the
  face), the flat Face they lie on, Across Bends (bends opened for the cut).
- **Corner Relief** (`sheet.corner_relief`, mitcad#209): faces of the two
  bends, the Face they leave when they leave several (for bends on different
  faces, the face held, one next to either bend; mitcad#125), Own Corner
  Relief (shape, size; a tear has none).
- **Unfold** (`sheet.unfold`): the Fixed Face and faces of the bends to open.
  **Refold** (`sheet.refold`): the Unfold (a choice of the design's, the newest
  at the start) and the bends to close, none for all.
- **Flat Pattern** (`sheet.flat_pattern`, a model command): the fixed face (or
  the body: keep its fixed face), Seen From (front, back), Show Flat;
  `set_flat_pattern` on OK. Opened on a body with a pattern, it starts with
  the pattern's settings.
- **Sheet Rules** (`sheet.rules`): a rules feature of its own (Thickness, Bend
  Radius, K-Factor, Sketch Side, bend and corner reliefs); its edit is
  **Edit Sheet Rules** (`sheet.edit_rules`, an action: the rules of the
  selected sheet, or of the only one), which every sheet using them follows.
- **Show Flat Pattern** (`sheet.show_flat`): `set_flat_pattern` with `shown`
  toggled for the selected sheet (or the first with a pattern). The view shows
  the developed solid (`analysis_shape` `flat_pattern`) in place of the body,
  without a section's clip (`MainWindow::modelBodies`).
- **Export Flat Pattern** (`sheet.export_dxf`): a save dialog ("Export Flat
  Pattern", the body's name `.dxf`), then `export_flat_pattern_dxf` in the
  document's unit when it is inches, else millimetres; a refusal shows the
  model's reasons. Allowed in a read-only window.

In the browser a sheet-metal body's row has two rows under it (expanded by
default; `BrowserNode::Type::SheetRules`, `FlatPattern`): its rules
(`SheetRules1 (2 mm, R 3 mm, K 0.3)`; double-click and Edit Sheet Rules edit
the feature, Find in Timeline) and, once set, its Flat Pattern (light bulb:
Show Flat; double-click and Edit Flat Pattern open Flat Pattern on it; Show
Flat/Folded, Export DXF..., Delete). A Flat Pattern that is not valid is red
with the reasons in its tooltip. The body's menu adds Edit Sheet Rules and
Create Flat Pattern or Export Flat Pattern. The rows come from the
`sheet_bodies` query (`DocumentSnapshot::sheets`). Test:
`tools/ui-windows-sheet-test.ps1` (ctest `app.ui-windows-sheet`: Face with its
own rules, Flange, rule edits of thickness, K and radius, Cancel, Flat
Pattern, Show Flat and Undo, the DXF read back, an edit from the timeline, the
Floating layout) and `tools/ui-windows-sheet-ops-test.ps1` (ctest
`app.ui-windows-sheet-ops`: Fold with Cancel, Bend at a seam, Corner Round and
Chamfer in the Floating layout, Corner Relief (also of flanges on different
panels), Sheet Cut across a bend, Unfold and Refold, each with its picks,
preview, analytic volumes or flat areas, an edit from the timeline and Undo).

## BUILD

The BUILD tab (mitcad#375, `commands/BuildCommands.cpp`, `build/`; the
model's side in commands.md, "BUILD: profile assemblies"; the parts'
`build` blocks in [docs/libraries.md](../docs/libraries.md#build-parts))
builds frames of slot profiles with their connectors, panels and linear
parts. It is a tab of its own beside SOLID (`Ribbon::groups("BUILD")`:
PROFILE, CONNECT, MODIFY, OUTPUT), in both chrome styles; SKETCH comes
after it while a sketch is edited, and Finish Sketch goes back to the tab
the sketch started from. The AppKit frontend has no workspace for it: its
commands are in a Build menu there. Everything BUILD makes is ordinary
design content (components, occurrences, features, joints), so SOLID's
commands edit it as any other.

The commands are panels of the framework with their own model commands
(`CommandDef::apply`, `inspect` as their preview, [Glue](#glue)): the app
calls only the model's `build_place`, `build_edit`, `build_connect`,
`build_panel`, `build_remove` and the queries `build_preview`,
`build_parts`, `build_junctions`, `build_cut_list`, `library_parts`,
`components`, and the library commands `library_build_candidates`,
`library_search`, `library_show`. Live MCP fills them as any panel
(`ui_command` with `inputs`); an agent's OK of a command that repeats
places one and closes it.

| Command | Panel | Model |
|---|---|---|
| `build.profile` Profile | grows as the placement needs it (mitcad#394). At first: Part, a tile with the chosen part's picture, name, supplier and part number (`withChoiceTile`, mitcad#427); a click on it, or Enter or Space, opens the part picker with the role's parts, filtered to the system chosen last (mitcad#418); Search and System are hidden inputs (for live MCP); Place On (none needed for the first part); Length (the part's stock length, else 500 mm; also a box at the cursor, `withCursorBox`, and a handle at the free end while the length is a number); for a ground placement Direction, three icon buttons X, Y, Z (`withChoiceIcons`, default Z: standing; `at.direction`). Once a part is placed, Length's mode as four icon buttons (mitcad#427, `withChoiceIcons`: Auto: to the face it meets, Auto: to its centre, Auto: through it, Typed length; their labels the tooltips) and, by the pick: an **end pick** adds Direction (mitcad#417: Auto, the click's zone of the end face, `Auto: by the click (+X)`; Straight; the four turns square to the part it meets, `end.turns`, named by the host axis they run along, `+X`, else `Side 1`..`Side 4`), on a corner Offset (the new part's end from the outer side, positive towards the turn, default 0) and Mitre (Auto, `Auto (none)`: the panel's turns are square, and a square corner lies on the end face, mitcad#427; `Auto (<cut>)` when the click's preview says another; None; Shared, not offered at a corner offset other than 0; Cut the new part; Cut the existing part) with Mitre Angle; a **side pick** adds Offset and From (End A, End B, Middle), which the click fills; either adds Rotate (0, 90, 180, 270 deg) and Flip. Placement, last, once the design has a part to go on: four icon buttons (mitcad#427, no More; Automatic: where the click is, the default; On a profile's side; On a profile's end; On a plane (as the first part): a ground placement again; their labels the tooltips). The result names where the part goes (`Adds to: Root`, the `build_host` query). **Place** (Enter) places it, also without a pick in a ground placement (at the origin), and the panel stays for the next one; **Close** (Esc) keeps what was placed | `build_host`, `build_preview` while the inputs change, `build_place` on Place |
| `build.panel` Panel | Part, the tile that opens the part picker (panels; mitcad#427); Frame Face (an inner face of a frame puts it in the slots, an outer face on the surface with clamps); Thickness when the panel's thickness is a parameter. Place repeats | `build_preview`, `build_panel`; `fits` false is shown with its reason, not refused |
| `build.linear` Linear Part | as Profile (its part picker too) without the mitre (rails are cut to length; a carriage on its rail gets the joint its `mate` says) | `build_preview`, `build_place` |
| `build.connector` Connector | Near (a click on a profile chooses the nearest junction), Junction (every junction, `L junction of Part1 and Part2`, `, connected`), Connector (the parts that fit, best first, each with the library's picture of it (`library_preview`, `withChoiceImages`), its supplier and part number; shown as a tile, `withChoiceTile`, mitcad#427, that opens the part picker with the fitting connectors first and, greyed with the reason, those that nearly fit; a new junction gives it new ones), With Nuts and Screws, With End Machining, Show All Junctions. The junctions of the profile under the mouse are marked in the view at their `at_frame` (`L`, `T (connected)`; `CommandDef::hover`, `previewMarkersIn`), the chosen one always (`> L`), every one with Show All Junctions; the panel lists the fitting connectors and, greyed, those that nearly fit with the reason; the two profiles are highlighted. The chosen connector is previewed where it goes (`build_preview` with `build_connect`'s fields; its shape `analysis_shape` `{"shape": "build_preview", "junction", "part", ...}`), its fasteners as markers, the machining it needs in the panel; choosing another in the list previews that one. **Connect** repeats and moves on to the nearest junction not connected | `build_junctions`, `library_build_candidates` with the junction, `build_preview`, `build_connect` |
| `build.fasteners` Fasteners | Connector (a connector, or one of its nuts or screws), With Nuts and Screws, Fastener (the kinds it has), Size (the rows of its library component, `library_show`) | `build_edit` with `config`; off: `build_remove` of each; on for a connector without them: `build_remove` and `build_connect` again with them; one undo step `Fasteners of <name>` |
| `build.length` Length | Part, Length (Detached: this length, Bound: to the face, to the centre, through), Value, Bound To (none: the face it is bound to now); the bound target highlighted | `build_edit` with `length` (a value detaches, a `bind` binds) |
| `build.ends` Ends | Part (the end nearer the click; an end face by its name), End, Angle, About (x, -x, y, -y), Also the Part It Meets (a shared mitre: the end that meets this one in an `L` or `inline` junction); End Machining (As it is, A bore: Diameter, Depth, Thread; None takes the part's end machining away) | `build_edit` with `ends` and `machining`, the part's whole list (`[{end, hole: {diameter, depth, thread}}]`): `build_parts`' list with this end's entries replaced by the bore, or left out for None; one undo step for both parts |
| `build.flip` Flip, `build.rotate` Rotate 90 deg | Parts (any number); Rotate repeats (Enter turns them again) | `build_edit` with `flip` / `rotate`, one undo step |
| `build.cut_list` Cut List, `build.parts_list` Parts List | an action: `BuildListsDialog` (`build/BuildLists.cpp`) with the tabs Cut List (Kerf; the cut parts grouped as the model groups them: quantity, part, length, end A and B, machining, own features, supplier, part number, mass each; the stock: pieces in cutting order, stock pieces, offcut; the other parts) and Parts List (`parts_list`: quantity, part, supplier, part number, library, version, licence); **Copy as CSV** (Alt+C) copies the shown tab's tables, **Save as PDF...** (Alt+P) writes them with `QPdfWriter` (Qt Gui; A4 landscape) under the list's title and the design's name with the date | `build_cut_list` (`kerf`), `parts_list` |

- **The picker** (`build/BuildCatalog`): the parts of the role from
  `library_build_candidates` (`library_search` with the `role` when the
  former is refused), each configuration row an entry, and the design's
  sources (`library_parts` entries with `source`), so that a part placed
  once is placed again from its source (`part.source`) without its
  library. Read when the command starts and after each placement. Every
  part of the role is a choice of Part (live MCP's `part`). Search and
  System are hidden inputs (mitcad#427; the part picker has both), kept
  for live MCP: Search words must all be in the name, designation, ids,
  supplier, part number, system (`B8 30`), size (`30x30`) or slots (`4
  slots`), and a part chosen that they do not match gives way to the
  first they match; System likewise to the first part of that system. The part chosen or placed last is the next
  start's, and the system of the part chosen last the next start's System.
- **The part picker** (mitcad#418, [Part picker](#part-picker)): the Part
  tile (Connector's Connector; `withChoiceTile`, mitcad#427) opens the main
  window's `PartPicker` for the command: Profile, Panel and Linear Part
  ask with their catalog's parts (`pickerParts(QVector<build::Part>)`,
  the drop-down's keys) and the panel's System as the picker's system;
  Connector with the junction's fitting connectors (the catalog's
  `forJunction`) and, greyed with the reason, the nearly fitting ones
  (`buildCandidates` with the junction's `refused`). A tile clicked is
  chosen in the command (`DocumentHost::chooseInCommand` with the key)
  and previewed at once; the open picker follows the panel (the part,
  another junction; live MCP's Search and System). The part chosen sets
  the hidden System to its own system (unless it fits several); the
  picker leaves its own filter as it is until it is opened again. The picker closes with the command
  (`CommandDef::ended`, `commandEnded`) unless pinned; an open, pinned
  picker follows the next BUILD command that chooses parts (its `init`).
  In this design reads `usedLibraryParts`; the main window's
  `refreshScene` calls `designChanged`. Not in the AppKit frontend (its
  inspector has no button).
- **Where parts go** (mitcad#394): no component needs to be activated.
  The model's `build_host` query names the BUILD host, the active
  component when it takes components, else the nearest one up that does
  (in a new design Part1 itself, which takes components, mitcad#421; the
  root when Part1 is set to Part); the panel's result says
  `Adds to: <name>`, and no policy refusal comes from where the parts go.
- **Where a pick is:** `withOnPick` keeps the click in the design (`at`
  is in the item's component; `build::recordPick`); an item given
  otherwise (live MCP) stands for its middle. `at.occurrence` is the
  item's path from the host (`build::pathFromHost`), `at.point` in the
  host's coordinates (`build::inHost`), as joints resolve their picks
  from their component: the root's origin planes, construction planes and
  faces are picked while Part1 is active.
- **Only usable targets** (`InputDef::withPickRule`): a **ground
  placement** (no profile in the design yet, or Placement On a plane)
  takes planes, points and faces that are no BUILD part's, the origin's
  planes shown; with nothing picked the view shows `Click a plane or a
  face, or press Enter` (`Inspection::viewHint`) and Enter places the
  part at the host's origin along the chosen axis (no `at.geometry`).
  Otherwise only faces of the parts it can go on highlight (a profile's
  sides and ends; for Linear Part also a rail's), the view says `Click a
  profile's side or end`, and the origin's and construction planes are
  hidden (`withHiddenConstruction`, the host's `setConstructionHidden`)
  until the placement is a ground one again or the command ends; their
  visibility in the design is not changed.
- **Snap distance** (mitcad#417): every placement with a click sends
  `at.snap`, 12 pixels of the view at the click in mm
  (`build::kSnapPixels`, `CommandContext::pixelSize`, kept with the
  click by `build::recordPick`); the model snaps within the larger of it
  and its own distances, so a zoomed-out view still snaps.
- **Side snaps:** a click on a side goes flush with an end within one
  section width of it, or to the middle or a grid point (the model's
  rules, commands.md, *Side placements*); the result says where (`0.0
  mm from end B (flush B)`), and the markers show the ends and the middle
  (`flush A`, `flush B`, `middle`) and the snap taken (`> flush B`).
- **Side offsets** (mitcad#417): Offset and From are always there on a
  side. A click fills them from the preview's `side.from` and
  `side.offset` (flush with an end: that end and 0; the middle: Middle
  and 0; else the nearer end), and the placement is left to the click
  (`at.point`), which the model binds to that reference. Typing an
  offset sends `at.from` and `at.offset` from then on: the part moves
  from its reference and never snaps; only a new click replaces both.
  Changing From keeps the part where it is: the offset from the new
  reference comes from the side shown last (`side.offset`, `side.width`,
  the target's length from end B's snap) and is sent as typed.
- **Ends** (mitcad#417): a click on an end face continues the part
  straight in the face's inner half and turns it towards an edge in the
  band along it; the click fills Direction's choices from the preview's
  `end` (its `turns`; Auto again). Straight sends the end's axis
  (`at.direction`, the turns' cross product), a turn its vector; on a
  corner the Offset goes as `at.offset`. Mitre Auto sends no `mitre`
  (the model's default, mitcad#427: none on a square corner, the panel's
  turns; shared at offset 0 only on a corner at another angle), None sends
  `{"cut": "none"}`. The markers show the end face's zones (`straight`
  in its middle, `turn +X` in the middle of each band; the one taken
  `> turn +X`). With the mouse on an end face of a part it can go on
  (`hoverPreview`), the preview is what a click there places, Direction
  Auto (the result says so); a move of under 4 pixels shows the same,
  and leaving the face or the view shows the pick again.
- **Auto length:** the preview without a length names the face the ray
  meets (`length.target`); the panel then binds to it (`{"bind": target,
  "mode", "add": 0}`) and previews again. Nothing in the way (the first
  part): the typed length. **Tab** steps face, centre, through wherever
  the main window has the keyboard (`InputDef::tabCycle`); typing a length
  or dragging its handle switches to Typed length (`onChange`), and
  switching to it starts from the auto length shown.
- **The preview:** the new part's shape is `analysis_shape` `{"shape":
  "build_preview", ...}` (`build_place`'s fields); without one, the
  section along `frame`'s z for `length` (`build::boxAlong`). The target
  face is highlighted orange (`Inspection::highlights`); the length is a
  label at the free end and the slot's `snaps` are markers
  (`Inspection::report`, `previewMarkers`). `frame` is `{"origin",
  "x_axis", "y_axis", "z_axis"}` in the design, the new part's own axes at end A
  with z from end A to end B (the junctions' frames too). The replies'
  lengths are millimetres and their angles radians.
- **Enabled:** Profile, Panel, Linear Part and Parts List always (an empty
  picker says to fetch the parts library); the others while
  `build_parts` lists parts.

Logs: `BUILD parts <role>: <n> (<m> in the design)`, `BUILD profile adds
to Root (a ground placement)`, `BUILD connectors for
junction <id> (<kind>): <n> fit, <m> nearly`, `Inspect Profile: place
side, length 742.50 mm (bound, auto: to the face to F8:part(...) of
Part5), square ends; adds to Root; snap end_b at 0.0 mm from b; least
snap 1.30 mm; part Profile 30x30` (the part previewed, last, mitcad#418; the side's snap, `none` between snaps, its offset from
its reference `a`, `b` or `middle`; `at.snap`), `Inspect Profile: place
end, ...; end edge towards +X, mitre none, offset 0.0 mm; least snap
1.30 mm; on the hover` (the end's zone `straight`, `edge` or `given`,
the direction, the mitre the placement takes, a corner's offset; the
hover's preview), `BUILD side click: From End B, Offset 0 mm (end_b)`,
`BUILD side offset typed: from end B`, `BUILD From End A keeps the part
in place: Offset 350 mm`, `BUILD end click: edge towards +X, mitre
none; turns +X, +Y, -X, -Y`, `BUILD corner offset: the shared mitre
goes (it needs offset 0)`, `BUILD hover on <face>`, `BUILD hover off the
end faces`, `BUILD click not placed: <reason>`, `Preview marker Profile
turn +X world x,y,z at x,y` (an end face's zone; `straight`; the taken
one with `> `), `Inspect
Profile: waiting: Click a profile's side or end.; adds to Root; part <part>`, `View
hint Profile: Click a plane or a face, or press Enter`, `Profile Place On
takes now: faces`, `Construction geometry hidden for the command`,
`Construction geometry shown again`, `Panel Profile input axis=x at x,y`
(an icon button of a choice), `Placed <part> as <name> (<occurrence>), length
<how>`, `Applied Profile (2)`, `Closed Profile after 2`, `Profile: Tab
steps Length`, `BUILD length typed: detached from the auto length`,
`Cursor box Profile length: 742.5 mm at x,y` (the box's text, when it
changes while shown), `Inspect Connector: junction J3 (L): 1 fit, 0
nearly, preview <connector> with 2 fastener(s), machining D6.8 x 15 M8
at end a`,
`Preview marker Connector > L world x,y,z at x,y` (a junction's marker,
the chosen one with `> `; `Preview marker Profile 742.5 mm ...` the
length at the free end), `Placed panel <part> as <name> (<occurrence>), fits; 0
clamp(s)` (`, does not fit: <reason>`), `Connected <part> at junction <id>
(<kind>) as <component>: <n> added`, `Fasteners of <name>: <n>
command(s)`, `Length of <name>: <value>` (`bound, face`), `Ends of <name>:
end a 45 deg about x, shared, machining D6.8 mm x 15 mm M8 at end a`, `Flip <names>`, `Rotate <names> 90 deg`,
`Cut list row: 2 x <part>, 500.0 mm, A 45 deg (x), B square`, `Cut list
stock: <part>, 6000.0 mm stock x 1, pieces 500.0, ..., offcut <mm>`, `Cut
list: <n> row(s), <n> stock, <n> other part(s), kerf 3 mm`, `Build parts
list: <n> row(s)`, `Build list CSV: <line>`, `Build list copied as CSV:
<n> line(s)`, `Build list PDF written: <path> (<bytes> bytes; Cut list of
<design>, <date>)`, `Ribbon tab BUILD at x,y`, `Panel Profile input part at x,y` (the Part tile), `Panel Profile tile part: Profile 30 test (Test supplier P30-T)` (what it shows), `Profile: Part button` (clicked), `BUILD picker: <part> not chosen, <command> does not run` (a pinned picker's tile after its command ended), and the picker's own ([Part picker](#part-picker)). Tests:
`tools/ui-build-test.sh`, `tools/ui-windows-build-test.ps1` (ctest
`app.ui-windows-build`).

## The view

`ViewController` (`view/`) registers the view commands and keeps display
settings in the user's settings (`ViewSettings`, group `view`);
`OcctViewer` draws them.

- **Orientation cube** (OCCT's `AIS_ViewCube`): a drag that starts on it
  orbits (passed to OCCT with its own key flag `kCubeDrag`); sketch mode
  leaves presses on the cube to the view (`OcctViewer::cubeInteraction`).
  Create Sketch's plane pick points keep away from the cube
  (`cubeArea`, `planePickPoint`), and the datums' logged pick places are
  where no other datum lies. The arrows shown when looking straight at a
  face: `OcctViewer::turnCube`.
- **Camera:** `turnTo(direction, up)` fits and animates
  (`AIS_AnimationCamera`, 0.4 s; Preferences can turn it off);
  `camera()`/`setCamera()` take a `CameraState` (eye, target, up,
  projection, height), which named views store; `lookAlong` and
  `lookHome` serve Look At and named views. Fit (F6) and sketch mode's
  look at the plane are instant. Fit (`MainWindow::fitView`, mitcad#405)
  frames the selection when something with a shape is selected outside
  commands and sketches ("Fit: the selection"), the shown bodies when the
  design isolates some ("Fit: the isolated bodies", not the origin, datums
  or sketches; `OcctViewer::frameBox`, `bodiesBox`), and otherwise
  everything shown. The camera is logged once it has kept
  still for 250 ms.
- **Visual styles:** hidden edges use two z-layers after the default one:
  the bodies' edges dashed without a depth test, then the visible edges
  against the faces' depth. Wireframe with Hidden Edges draws the faces
  unlit in the background's middle colour, because OCCT draws see-through
  faces after every layer sharing the depth buffer. Bodies keep
  `BodyDisplay::color`; previews, highlights and overlays are untouched.
- **Component Colors** (`view.component_colors`, View menu, mitcad#321):
  draws every body in a stable colour of its owning component
  (`BodyDisplay::component`; hues of consecutive component ids a golden
  angle apart, other ids hashed) and leaves out face appearances, in the
  shaded and wireframe styles; new bodies of a preview keep their style.
  A view setting (`componentColors`), not part of the document. Turning
  it on leaves the rendered view; entering the rendered view turns it off.
  Logs `Component colors on` / `Component colors off`; checked in
  `tools/ui-windows-test.ps1` (`app.ui-windows`).
- **Layout grid** (`LayoutGrid`): on XY, or the sketch plane in sketch
  mode, in a layer under everything, left out of Fit. Automatic spacing:
  the largest 1 or 5 × 10ⁿ mm whose lines are at most 35 px apart
  (`layoutGridStep`), major line every fifth. Snap to Grid snaps to the
  shown minor lines (`OcctViewer::gridStep`).
- **Navigation schemes** (`NavigationScheme`, saved by id as
  `view/navigationScheme`; the old index `view/navigation` is read and
  replaced): `middle-pan` (default), `alt-buttons`, `middle-pan-f4`,
  `middle-orbit`, `right-orbit`, `trackpad` (the default on macOS:
  two-finger scroll pans, Alt + scroll orbits, Shift or Ctrl + scroll
  zooms, Alt + left orbits), and `trackpad-shift` (Shift + scroll orbits,
  Alt or Ctrl + scroll zooms; the same pan, pinch and mouse buttons).
  Pinch, rotation and two-finger double tap
  work in every scheme (`OcctViewer::event`).
- **Preferences** (`tools.preferences`): one page per group (General,
  Navigation, Display with the window's Layout, Cache, Cloud, Updates, 3D
  Print, Diagnostics, Textures: `TexturePreferencesBox`, the texture
  folders, mitcad#315) so the window fits 1366 × 768; each page is also a command
  `tools.preferences.<page>` that the search finds by its settings. OK
  saves every page. On macOS the pages are the panes of `SettingsWindow`
  (Settings, Cmd+,), whose changes apply as they are made. In builds with
  the render worker, Display also has the **render device** (mitcad#50,
  the setting `render/device`: Automatic, the CPU or a GPU the worker
  lists with `mitcad-render --list-devices`; a device of an earlier choice
  that is not there stays as "Not found"): a change starts a running
  rendered view's worker again on it, and Render Image renders on it. In
  the Floating layout Display also has the **Browser background** (Card
  or Rows, mitcad#419, [above](#browser-timeline-and-parameters)), which
  applies without a restart.
  Display ends with **Ghost inactive components** (`view/ghostInactive`)
  and **Pick the active component first** (`view/activePickPriority`),
  both on by default (mitcad#420, below).
- **Layout** (mitcad#419; Preferences › Display › Layout, after the render
  device; not in the native macOS frontend: `layoutChoosable()`):
  Docked or Floating, kept as `appearance/layout` (`docked`, `floating`;
  absent: the platform's default, `defaultChromeStyle()`: Floating on
  macOS, Docked elsewhere). The ticket's "Appearance" page is this Display
  page. `main()` reads it before the window is made (`layoutPreference()`,
  `framework/ChromeStyle`); `--chrome=docked|floating` overrides it for
  that run, and the start-up's `Chrome:` line says which decided. A change
  to a layout other than the window's asks *Restart Mitcad* (Restart Now,
  Later) once the dialog is gone (on macOS's Settings window, as it is
  made); Later keeps it for the next start. Restart Now
  (`MainWindow::restartApplication`, once no model job runs) closes the
  window as quitting does (`maybeSave`: Cancel keeps Mitcad open and
  nothing restarts) and starts Mitcad again after this process has quit
  (`framework/Restart`: a post routine of `QCoreApplication`, as an
  installed update starts; the AppImage itself when it runs from one) with
  this run's settings options (`--ui`, `--no-native-dialogs`,
  `--no-recovery`, `--no-glass`, `--no-mcp`, `--mcp-folder`, a plain
  `--debug-log`), without `--chrome` and the one-off start actions, and
  with `--open <file>` when the design has a file. `MITCAD_TEST_NO_RESTART=1`
  logs the command instead of starting it (UI tests).
- **The Floating layout's top bar** (mitcad#419, mitcad#425): no card
  starts above the bar at the top. On macOS (`floatingTopCards()` false)
  that bar is `FloatingLayout`'s header: the title bar row with the
  workspace switch, the ribbon's groups, the notices; the upper cards' top
  is at least the header's bottom plus the margin, and the header's size
  and place are followed (resized, moved, a notice shown), so a narrow
  window whose groups collapse, or a taller bar, places them again. On
  Windows and Linux only the menu bar is a bar: the 3D view starts right
  below it, and the title row and the toolbar's groups float over the
  view's top as cards (`FloatingLayout::addTopRow`): the title bar row
  (Undo and Redo, the Solid/Build switch, the document's title with the
  project indicator, the command search; left, centre and right as before,
  the title elided when narrow) is one card (`titleCard`, a `GlassCard`
  with the `TitleBar` as its content), as wide as the view less the
  margins; below it a row of the ribbon's groups, each a card of its own
  (`Ribbon::setGroupCards`: the `CapsuleFrame` capsules in the cards'
  material, named `ribbonGroup_<TAB>/<GROUP>`), with the same buttons,
  split buttons and drop-downs, collapsing from the right in a narrow
  window as before; below them the update, remote, live and edit-lock
  notices in a card (`noticeCard`) shown while one of them is. The ribbon
  takes the mouse only on its group cards (a mask of the shown page's
  cards), each card only inside its own rectangle: a click, drag or wheel
  between and beside them reaches the view (select, orbit, pan, zoom), and
  the view picks there (`OcctViewer::setTopCards`: picks are refused only
  inside the cards, not in a margin around them). The Projects, Browser
  and Command cards start below the last row (the gap), the orientation
  cube and its buttons too, and Fit, Home and the named views fit the
  model below the rows. Docked is unchanged everywhere.
- **Inactive components as ghosts** (mitcad#420): while a component other
  than the root is active (Browser › Activate, its radio button; also its
  undo and redo; a design opens with the root active, mitcad#426),
  `MainWindow::refreshScene` gives the view the active component's
  occurrences (`OcctViewer::setActiveScope`, from the `DocumentSnapshot`:
  every occurrence of the active component, its sub-components with them),
  the sketch being edited and the origin's datums (the active component's
  own). Everything else is an inactive ghost: bodies are drawn as
  `BodyDisplay::Style::Ghost` (see-through, their own colour) but stay
  pickable, with the hover and highlights of any item; sketch curves and
  points, profiles and datums are pale (faded towards the background's
  middle, `inactiveColor`; profiles and planes more see-through). The root
  active, or the setting off (`OcctViewer::setGhostInactive`): the view as
  before. A command's ghosts (`withGhosts`, Joint's side A) and previews
  keep working on top: a body ghosted by both is drawn once, as the
  command's ghost (not pickable). Inactive ghosts hide nothing for the
  edge sight test (`occluded`). The rendered view, Render Image and
  `shownBodies()` ignore the ghosting (every body renders normally);
  Section Analysis cuts the ghosts like any body (their caps as usual);
  screenshots and the view's captures show what the viewport shows.
  **Pick priority:** while the scope is on and the setting
  `view/activePickPriority` is on (`OcctViewer::setActivePickPriority`,
  independent of the ghosting), a click, a command's pick and the hover take
  the nearest pickable item of the active component under the cursor before
  nearer items of other components (`setPickPriority` in `PickContext.hpp`:
  the selector orders its point picks with the preferred owners first, after
  its edges-first order; what is not the model's, the orientation cube,
  keeps its place). Where nothing of the active component is under the
  cursor, the ghosts are picked as usual; there is no layered pick through
  to the next item under the cursor, so a ghost behind the active
  component is picked where nothing of the active component covers it, or
  with the setting off. Logs: `View inactive ghosts: 2 bodies (active
  Bracket:1)` (the bodies drawn as inactive ghosts, a command's ghosts
  among them; the active component's occurrence names) and `View inactive
  ghosts: none`, when that changes; `Pick priority: active Bracket:1` when
  a click took the active component's item before a nearer one of another
  component; `Preferences: ghost inactive components on|off`, `Preferences:
  active component first in picks on|off`. Tests:
  `tools/ui-inactive-ghost-test.sh`, `tools/ui-windows-inactive-ghost-test.ps1`
  (ctest `app.ui-windows-inactive-ghost`).
- **Named views** are the document's (`add_named_view` and friends); a
  view named `Home` is the document's home.
- **Section Analysis** and **Hide Above Sketch:** the cut faces are drawn
  as caps (`MainWindow::showSectionCaps`: the clipped bodies' faces in the
  plane, without those the whole body has there, as under a sketch on a
  body's face), one group per component, in the component's timeline colour
  (mitcad#382: `componentColor` / `componentColorIndex` in
  `browser/ComponentColors.hpp`, shared with the timeline; the root's bodies
  keep the default orange) in every visual style, however the bodies
  themselves are coloured. `OcctViewer::setSectionCaps` takes the groups
  (`OcctViewer::SectionCap`) and hatches each over its fill at 45 degrees
  to the plane's x axis (the sketch's in Hide Above Sketch), the next group
  at 135, in a darker shade of its own colour (`capHatchColor`), 1.5 px
  wide, with a spacing per cap face (about eight lines across a small
  face, at most 40 across all the caps of a group, at least 400th of
  them); the lines lie off the caps toward the removed side, in the layer of
  the edges in sight, so that they are drawn over the fill. The clip is the
  model's `analysis_shape {"shape": "clip"}`, on the UI thread; a body
  whose clip fails is shown whole, also when OCCT crashes in the boolean
  (OCCT's handlers on the UI thread turn the crash into the clip's error,
  mitcad#392). A crash in the section the panel measures is likewise the
  panel's error, and the bodies stay whole
  (`tools/ui-section-crash-test.sh`). What is left of
  the bodies keeps their names, so its faces, edges and vertices are picked
  as the bodies' own (Project while Hide Above Sketch is on, also the face
  the sketch lies on, which stays whole); the caps cannot be picked
  (mitcad#233). The viewer logs `Section caps: 3 colour(s) [#e0892a hatch
  #4b2d0b at 135 deg, 12 lines; ...] in <style>, bodies in <component|appearance> colours`
  (again when the style or Component Colors change). Tests:
  `tools/ui-section-caps-test.sh` (the colours per component and the
  hatch from the log), `tools/ui-windows-section-pick-test.ps1` (ctest
  `app.ui-windows-section-pick`).

- **Rendered** (`view.rendered`, in View > Visual Style; only when the
  render worker `mitcad-render`, which every build has, is next to the
  application, [docs/rendering.md](../docs/rendering.md)): a
  toggle beside the visual styles, an ordinary entry, not a developer
  setting. `RenderMode` (`render/`) starts the render worker, sends it the shown bodies as a scene update whenever
  `OcctViewer::bodiesChanged` (once the model is idle: every body with
  its mesh's content hash, placement and material, and only the meshes
  the worker does not hold) and the camera and size whenever a frame was
  drawn with another one, and hands its frames to
  `OcctViewer::setRenderedImage`: the frame covers the view in a layer
  over the bodies (which stay drawn, for their depth, and pickable) and
  under the overlays, so sketches and other overlays behind a body are
  hidden as in the shaded view (`OcctViewer::setRenderedMode`). A
  render's first frame is denoised. Until the first frame of a new camera
  comes, the bodies are drawn shaded. If the
  worker dies, a message over the view says so and the mode turns off; a
  worker of another protocol version is stopped with a message too.
- **Render Environment...** (`view.render_environment`, in View > Visual
  Style after Rendered and in View > Environment; with the render worker
  only; mitcad#47): a non-modal dialog (`render/RenderEnvironmentDialog`)
  beside the main window, with the document's render settings (the
  model's `render_settings`, [commands.md](../core/model/src/api/commands.md#render-settings)):
  light (Studio, White Studio, Dark Studio, Outdoor, HDR Image with its
  path and Browse...), strength, rotation, the sun's elevation and
  direction (degrees), background (View Background, Colour, Environment)
  and its colour, ground shadows, reflections, under the lowest body or at
  a height, exposure and view transform on the Environment page, a
  Rendered view check box, Reset and Close. Fields that the light or the
  ground does not use are disabled. The Lights page (mitcad#54) lists the
  document's lights (the chosen one's fields below): Add adds a light of
  the kind beside it (Point, Spot, Area, Sun) at the camera, shining where
  it looks (`add_render_light`); Delete deletes the chosen one
  (`delete_render_light`); its name, kind, On, Follows the camera (the
  numbers change so that the light stays where it is), position and
  direction (x, y, z), colour, power (W, a sun's W/m²), size, an area
  light's height and shape, a spot's cone and blend and a sun's size each
  change it (`edit_render_light`, an undo step); Aim at Face takes the
  next click on a body's face in the view (`render::surfaceAt`: the
  bodies' display triangles under the mouse) and puts the light the
  distance beside it out along the face's normal, shining at the point
  (a sun only turns; Esc cancels); At Camera puts it at the camera,
  shining where it looks. While the dialog is open the lights are drawn
  as glyphs over the view (`render::RenderLightsOverlay`, a child widget
  of the view that takes no input: a point light a dot with rays, a spot
  its cone, an area light its outline, a sun an arrow toward the view's
  middle; the chosen one orange, those off grey). Each change is one `set_render_settings` (Reset:
  `reset_render_settings`), an undo step; not while a command panel or a
  sketch is open. `MainWindow::refreshScene` hands the settings to
  `ViewController::setRenderSettings` after every change (also undo, redo
  and opening), which passes them to `RenderMode` and refreshes the
  dialog. `RenderMode` sends the worker an `environment` command when the
  environment, the background's visibility or the ground changed, with a
  new view, so the render starts again; the exposure, the view transform
  and a background colour re-display the last frame, as a change of the
  view's background does.
- **Render Image...** (`file.render_image`, a button of the ribbon's MAKE
  group before 3D Print (mitcad#175) and in View > Visual Style after
  Render Environment; with the render worker only; mitcad#48): a
  non-modal dialog (`render/RenderImageDialog`) beside the main window
  with the render settings' `output` section (Camera: Current View or a named view; Size
  presets, which fix the aspect; Width, Height, Aspect From the View or
  Fixed; Samples, Time limit, Denoise; Format PNG, PNG 16 bits, JPEG,
  OpenEXR; JPEG quality; Transparent background), a preview, a progress
  bar and Render, Cancel, Save... and Close. Settings change as in Render
  Environment (one `set_render_settings` each, an undo step; Height only
  with a fixed aspect, JPEG quality only for JPEG, transparency not for
  JPEG). Render waits until the model is idle (`DocumentHost::whenIdle`),
  writes the shown bodies as a scene and the job (`render/RenderBatch`,
  shared with `mitcad-cli render`) into a temporary folder and runs
  `mitcad-render --batch <job> --control` (`render::FinalRender`); the
  window stays usable meanwhile. Cancel (and Close or Escape during a
  render) writes `stop` to the worker and ends it after two seconds;
  Save... copies the finished image through a file dialog filtered to
  its format, named after the design. With a fixed aspect and the
  current view, the view shows the image's frame (a child widget that
  dims the rest and takes no input) while the dialog is shown.
  `ViewController::setRenderSettings` refreshes the dialog after every
  change of the model, and the main window gives it the design's folder
  and name.

**What the view keeps between refreshes.** `refreshScene` runs after every
model change; the view keeps what did not change: bodies with the same
shape, placement (compared by value, `sameTransformation`: a new
`TopLoc_Location` is never `IsEqual` to an old one), style and colour;
profiles whose face the main window passes again (`m_profileFaces`, kept
while the `profiles` query's region `hash` and the placement stay);
sketch entities with the same `signature`. A new display path must keep
to this: give the view the same shape object for the same geometry. The
commands' availability checks share query answers while actions are
updated (`m_queryMemo`).

**Curve display quality.** The interactive context's default drawer uses
`view/ViewTessellation.hpp`: absolute chordal deviation `0.01 mm` and a
5-degree angular limit, inherited by body, wireframe and highlight drawers.
The absolute limit prevents a long body's bounds from inflating OCCT's
minimum mesh size; the angle limit preserves small circular details.
`app.view_tessellation` checks shaded boundaries and wire curves after
synthetic STEP round trips at several scales, including a long thin body.

**Windows session 0** (the VM's tests over SSH): the window is never
exposed, so `update()` paints nothing; `OcctViewer::requestFrame` (used
instead of `update()`) then draws the frame directly. Popups close at
once there, so the Windows tests start commands by shortcuts.

## Background computation

Opening, every model command (also undo, redo, `recompute`, the browser's,
timeline's and Change Parameters' commands) and every preview
(`CommandHost::modelPreview`, sketch mode's `CommandHost::modelCommand`)
run as jobs on the model's worker thread (`MainWindow::runJob`,
`installDocument`, `command()`; design in
[docs/architecture.md](../docs/architecture.md)). Callers stay
synchronous; a job under 50 ms behaves as inline. Rules:

- The caller waits in a nested event loop where timers and queued
  signals still run. Everything that reads the document goes through
  `MainWindow::idleDocument()`, which asserts (debug) and throws
  `ModelBusy` during a job. What can come from the nested loop waits or
  is refused: preview and drag timers restart, picks, the command search
  and the browser's and timeline's queued calls wait (`whenIdle`;
  `BrowserController::follow`), context menus are dropped, `runCommand`
  refuses, autosave skips.
- Drags end when the progress dialog blocks the window
  (`QEvent::WindowBlocked`: the view, sketch drags, manipulators, the
  timeline marker).
- While a job computes the window's document, the UI thread must not read
  or triangulate its shapes, only draw what the view already made of
  them.
  `busyChanged` suspends viewport picking before the worker starts: OCCT's
  `MoveTo` can run during a paint in the nested wait loop, and command
  selection predicates read the document. Finishing, failing or cancelling
  the job restores picking and invalidates cached hover detection.
- A cancel takes back what started it as a whole: the command is rejected
  with `the computation was cancelled` (`ComputationCancelled`;
  `lastCancelled()`), `runModelCommands` undoes the commands before it,
  Change Parameters keeps the old value, OK leaves its panel open, Finish
  Sketch stays in the sketch, Cancel of an edit panel whose roll-forward
  is cancelled closes with the timeline rolled back. A cancelled preview
  is not cached; OK then computes the command itself. A cancel inside a
  single kernel call ends when it returns ("Cancelling...").
- A cancelled open keeps the previous document; closing the window during
  a job cancels it and closes when it is back.
- The wait cursor (`framework/WaitCursor.hpp`, mitcad#268) is a guard of
  the wait: set once when the wait starts (after 50 ms), restored once,
  when the progress dialog shows or when the wait ends (also early, and
  for nested waits), each logged (`Wait cursor on: <job> (depth N)`,
  `Wait cursor off: …`). When the window is idle (no job, no mouse button
  held, no modal dialog or popup) an override cursor left on is cleared
  (right after a job and every second), with a warning: a stuck busy
  cursor fixes itself and the debug log says after which job.
- Test switches (`framework/Diagnostics.hpp`):
  `MITCAD_TEST_RECOMPUTE_DELAY_MS` (`400,undo=800`: per job name),
  `MITCAD_COMPUTE_INLINE=1` (jobs on the UI thread);
  `MITCAD_TEST_WAIT_CURSOR=1` (the commands Nested Wait (Test), a wait
  waited for again inside it, and Leave Wait Cursor On (Test));
  `tools/ui-compute-test.sh`.

Limitation: queries and analyses (Measure, Interference, Physical
Properties), Save's commit and the status bar's history still run on the
UI thread.

## Files

### Source files

| File | What |
|---|---|
| `framework/Command.hpp` | `CommandDef`, `InputDef`, `CommandState`, `Built`, `CommandContext` |
| `framework/Selection.hpp` | `SelectKind`, `SelectFilter`, `SelectionItem` (a picked entity by its model name) |
| `framework/SelectionScope.*`, `framework/SelectionFilterPanel.*`, `MainWindowSelection.cpp` | sketch selection filters (mitcad#217): the three-state rows, the sketch's and Project's contexts, the Selection Filter control, sketch mode's picks through them |
| `framework/CommandSession.*` | a running feature command: state, panel, selections in the view, previews, OK and Cancel, editing |
| `framework/CommandPanel.*` | the panel generated from the inputs |
| `framework/CommandRegistry.*` | every command with its `QAction`; shortcuts saved in the settings |
| `framework/Ribbon.*` | the toolbar: tabs, groups, group menus |
| `framework/CommandSearch.*` | the command search (S) |
| `framework/ShortcutDialog.*` | Tools > Keyboard Shortcuts |
| `AboutDialog.*` | Help > About Mitcad: version, licence, third-party credits (OCCT and Qt versions) |
| `framework/ModelShapes.*` | display shapes of datums and sketch geometry from JSON |
| `framework/ManipulatorOverlay.*` | handles a command draws over the view: distance arrows, angle rings |
| `framework/Appearances.*` | appearances as the model's `appearances` query lists them (`Appearance`: the physically based parameters and the display colour), a preview swatch (`appearanceSwatch`), and the physical materials |
| `materials/TextureSource.*`, `materials/FolderTextureSource.*`, `materials/TextureBrowser.*`, `materials/TextureUse.*`, `materials/TexturePreferences.*` | texture sources (mitcad#315): the `TextureSource` interface, its registry, `TextureSettings` and the texture cache's folders; local texture folders (map detection in file names, the scan, thumbnails); Edit Appearances' grid; Use's `create_appearance`; Preferences' Textures group |
| `materials/PolyHavenTextureSource.*`, `materials/TextureFetcher.*` | Poly Haven as an online texture source (mitcad#313): reading the API's lists, the cached list, files and thumbnails, the downloads; the requests of online sources (Qt Network, or fixture files for UI tests) |
| `framework/Cursors.*`, `framework/Icons.*`, `framework/Numbers.*` | the sketch tools' pointer; icons from `app/icons`; numbers with decimal commas |
| `framework/AppSettings.*` | general settings (autosave) |
| `framework/DesktopEntry.*` | the Linux desktop file and icons an AppImage installs for itself |
| `framework/ModelWorker.*`, `framework/ComputeProgress.*`, `MainWindowJobs.cpp` | background computation: the worker thread, a job's control and stage (`ModelJob`, `AttachedJob`), the progress dialog, `MainWindow::runJob` |
| `framework/ResultCache.*`, `framework/CachePreferences.*`, `framework/CacheDiagnostics.*` | the result store's folder, build id and settings, its clean-up; Preferences' Cache group; Help › Diagnostics |
| `framework/Diagnostics.hpp`, `framework/TestSync.*` | environment switches for test logs and delays; the UI tests' sync |
| `commands/CommandSupport.*` | shared by the families: references to and from items, paths, operations, extents' objects, values, plane frames and axes for manipulators |
| `commands/CommandFactories.hpp` | the SOLID tab's feature commands, made in the family files below |
| `commands/CreateCommands.cpp` | `registerCreateCommands`: the CREATE group in menu order |
| `commands/ExtrudeCommands.cpp` | Extrude and Revolve |
| `commands/SweepCommands.cpp` | Sweep, Loft, Rib, Web, Coil, Pipe, Helix |
| `commands/HoleCommands.cpp` | Hole, Thread |
| `commands/PrimitiveCommands.cpp` | New Component, Box, Cylinder, Sphere, Torus |
| `commands/PatternCommands.cpp` | Rectangular, Circular and Path Pattern, Mirror |
| `commands/PointPatternCommands.cpp` | Pattern at Points: sources and referenced targets, optional anchor, model-reported preview marker |
| `commands/ModifyCommands.cpp` | `registerModifyCommands`: the MODIFY group in menu order |
| `commands/FilletCommands.cpp` | Press Pull, Fillet and Chamfer |
| `commands/FaceCommands.cpp` | Shell, Draft, Offset Face, Replace Face, Split Face, Split Body, Delete Face |
| `commands/MoveCommands.cpp` | Scale, Combine, Move/Copy, Align, Physical Material, Appearance |
| `commands/ConstructCommands.cpp` | `registerConstructCommands`: every construction plane, axis and point, from a table |
| `commands/InspectCommands.cpp` | `registerInspectCommands`: Measure, Interference, Section Analysis, Physical Properties |
| `commands/JointCommands.cpp` | `registerAssembleCommands`: the ASSEMBLE group: Joint, As-Built Joint, Joint Origin, Rigid Group, Drive Joint (mitcad#55); New Component (`PrimitiveCommands.cpp`) leads the group, Animate Joint, Insert Component and Insert from Library (the main window) end it |
| `MainWindowJoints.cpp` | dragging components the joints move, Animate Joint |
| `commands/SheetCommands.cpp` | `registerSheetMetalCommands`: the SHEET METAL group (mitcad#210): Face, Flange, Bend, Fold, Corner Round, Corner Chamfer, Sheet Cut, Corner Relief, Unfold, Refold, Flat Pattern, Sheet Rules |
| `MainWindowSheet.cpp` | Edit Sheet Rules, Show Flat Pattern, Export Flat Pattern and its dialog |
| `commands/BuildCommands.cpp` | `registerBuildCommands`: the BUILD tab (mitcad#375, [BUILD](#build)): Profile, Panel, Linear Part, Connector, Fasteners, Length, Ends, Flip, Rotate 90 deg, Cut List, Parts List |
| `build/BuildCatalog.*`, `build/BuildModel.*`, `build/BuildLists.*` | BUILD's parts for the pickers (library commands, the design's sources); what it reads of the model (paths, placements, picks as `at`, BUILD parts, junctions, preview frames); the Cut List and Parts List dialog with CSV and PDF |
| `library/PartPicker.*`, `library/PartFavourites.*`, `library/UsedParts.*` | the part picker (mitcad#418, [Part picker](#part-picker)): the grid of the library's parts (`PartGrid`), the floating window of the part-choosing commands (`PartPicker`), Insert from Library's results; the favourites and their order; the design's library parts |
| `commands/SketchCommands.cpp` | the SKETCH tab: drawing tools, Sketch Dimension, constraints, Offset, Mirror, patterns, Project, sketch Fillet and Chamfer, Move/Copy, Trim, Extend, Construction, Convert to Sketch Geometry, Delete |
| `sketch/SketchController.*` | sketch mode: mouse and keys, typed values, drags, `SketchOp` |
| `sketch/SketchTool.hpp`, `DrawTools.cpp`, `EditTools.cpp` | the tools (`SketchTool`) and their factories |
| `sketch/SketchOverlay.*` | drawn over the view: constraint glyphs, dimensions, previews, snaps, DOF |
| `sketch/SketchSnap.*`, `SketchGeometry.*`, `SketchModel.*` | snapping; plane curves; the `sketch` query parsed |
| `sketch/SketchPalette.*` | the sketch palette (options, constraints, DOF, the drawing tool's auto constraints) in the command panel's place |
| `sketch/AutoConstraint.*` | auto constraints while drawing (mitcad#189): the types a tool offers, their definitions, the geometry that satisfies them, the tool session's first and last entities |
| `browser/DocumentSnapshot.*` | the document's structure read once per change; feature icons; body volume logs |
| `browser/BrowserPanel.*` | the browser (model tree) |
| `browser/TimelineWidget.*` | the timeline with the marker and playback buttons |
| `browser/TimelineSearch.*` | the timeline search's words and filters (mitcad#179) |
| `browser/ParametersDialog.*` | Change Parameters |
| `browser/AppearanceDialog.*` | Edit Appearances |
| `browser/BrowserController.*` | what the browser, timeline, parameters dialog and failed-features summary do |
| `browser/DocumentHost.hpp` | what they ask of the main window |
| `view/ViewController.*` | the View menu: standard views, Look At, camera, visual styles, grid and snaps, environment, named views, the cube's menu, Preferences, the keyboard and mouse overview; `ViewSettings` |
| `render/` | [docs/rendering.md](../docs/rendering.md): View > Rendered (`RenderMode`, `RenderClient`, `FrameMemory`), Render Environment, Render Image (`RenderImageDialog`, `RenderBatch`: the final render's scene, camera, job and worker process); the worker `mitcad-render` (`RenderWorker`, `CyclesRenderer`, `RenderOutput`) |
| `LayoutGrid.*` | the layout grid |
| `files/MainWindowFiles.cpp` | the File menu (mitcad#175; groups: New Design, Open, Open Recent, Open Read-Only, Close; Save, Save As, Save with Description, Version History; the Project submenu with New Project, Open Project, Open from Cloud, Sync, Check for Newer Versions, Open in Browser, Move to a Project, Project Settings; Import, Export; Recover Documents; Exit. 3D Print, Insert Component and Render Image are on the ribbon only): open, recent files, close, import, export, Insert Component, drops |
| `files/FileFormats.*`, `files/FileDialogs.*` | file kinds by extension and dialog filters; Export, Insert DXF, Insert Mesh, linked or copy, the import report |
| `files/F3dImport.*` | `.f3d`, FreeCAD, `.ipt` and `.iam` import in a worker process behind a progress dialog |
| `files/Autosave.*`, `files/Recovery.*`, `files/MainWindowAutosave.cpp` | autosave and recovery |
| `files/MainWindowVersions.cpp`, `files/VersionDialogs.*` | saving versions: Save and Save with Description in a versioned project, the author, changes and renames made outside Mitcad, the indicator's version |
| `files/VersionHistory.*` | the Version History window |
| `files/MainWindowProjects.cpp`, `files/Projects.*` | Local and Cloud projects (mitcad#89): the window's current project (`ProjectState`), New Project, Open Project, Open from Cloud, Open Read-Only, Project Settings, Move to a Project |
| `files/ProjectDialogs.*`, `files/ProjectSettings.*`, `files/CloudSection.*`, `files/CloudAddress.*` | their dialogs: New Project, Open from Cloud, Change Address, Project Settings; the shared Cloud section (service, address, the check, Trust This Server); addresses composed and read back (Qt Core, `app.unit`) |
| `files/ProjectIndicator.*` | the project indicator and its menu |
| `projects/` | the Projects panel (mitcad#145, [below](#projects-panel)): `ProjectsPanel` (the tree, names typed in place, drags), `ProjectsTree` (the projects read with `project_tree`, their names), `MainWindowProjectsPanel.cpp` (its dock or card, View › Projects, File › New Design, what the panel asks for) |
| `files/Remote*.*` | the current Cloud project's remote: sync, check for newer versions, open in browser, sending versions, Resolve Sync Conflicts, Preferences' Cloud page; `RemoteTask` runs one operation (a project's or a projects command) on a thread of its own |
| `files/Print3d.*`, `files/MainWindowPrint.cpp` | 3D Print: slicers found, settings, file names, the start bodies and the Slicer row's choices, the Preferences group; the command's panel (`print3dCommand`) and its send (`sendToSlicer`) |
| `update/` | automatic updates ([docs/updates.md](../docs/updates.md)) |
| `report/` | feedback and error reports ([below](#feedback-and-error-reports)): `ReportCenter` (Help › Send Feedback, the offers, the delivery), `ReportDialogs` (the form, the screenshot's crop, the preview), `ReportText` (masking, crash files, duplicate keys, the issue form's address; Qt Core, `app.unit`), `CrashHandler` (the signal and exception handlers, plain C++, also in `mitcad-render`) |
| `MainWindow.*` | the host: document, modes (idle, picking a sketch plane, sketch, command), selection, context menu, actions |
| `OcctViewer.*` | the 3D view: bodies in the visual style, previews, highlights, picking, camera, orientation cube, navigation |
| `icons/*.svg` | own icons (see [Icons](#icons)) |
| `web/` | the web viewer's pages (mitcad#423, [Web page](#web-page)): `WebPage` (`writePage` of the open design, `writePageOfFile` of a design file without a window, `pageHtml`: the viewer's template with its import map, module and data inlined), `WebGlb` (the bodies as the viewer's glb: a node per placed body, appearances and textures; the component tree); the viewer itself in `web/viewer/` |
| `web/MainWindowWeb.cpp`, `web/ProjectWeb.*` | a project's web viewer (mitcad#423): Web Viewer, Update Web Viewer, the Save that writes a design's page; the `web/` folder's slugs, `designs.json` and `index.html` (Qt Core, `app.unit`) |

### Import

By extension (`files/FileFormats`); `--open` and dropped files take the
same ways without questions (mm, the XY plane, linked).

| Kind | How |
|---|---|
| `.f3d`, `.f3z` | a new document with the design's history (worker process, below) |
| `.FCStd` | a new document with the bodies and PartDesign/Part history (`import_fcstd`, same worker; commands.md, "FreeCAD import (.FCStd)"); no Stop |
| `.ipt` | a new document with the part's stored bodies, one base feature each (`import_ipt`, same worker, `hang_limit` 90 s; commands.md, ".ipt import"); no Stop; the report lists the bodies, part number, material and units |
| `.iam` | a new document with the assembly's components and occurrences, each part imported once with its features (`import_ipt`, which imports an assembly with `import_iam`; same worker; commands.md, ".iam import"); no Stop; the report lists the occurrences, how each file was found, missing files and the checks against the file |
| `.rvt`, `.rte`, `.rfa`, `.rft` | a new document with a project's building elements, one component per level (`import_rvt`, same worker, mitcad#274; commands.md, ".rvt import"); a family file: its metadata only; no Stop; the report lists the bodies per level, what was left out and why, and the components are logged (`Imported <file>: components <name> (<n> bodies), …`) |
| STEP, IGES, BRep | a base feature (`import_file`, one undo step `Import part.step`) |
| STL, OBJ | mesh bodies, after asking the unit (Insert Mesh) |
| DXF | a new sketch on a plane (`sketch.create` + `sketch.import_dxf`, one undo step `Insert drawing.dxf`), or into the edited sketch; the dialog uses the `dxf_info` query (origin placement, layers) |
| `.mitcad` | Insert Component, linked or as a copy (`insert_component`); in a project with versions the design is chosen in the [project picker](#project-picker) (Another File... the file dialog; mitcad#176) |

**The `.f3d` worker** (`files/F3dImport`): the application runs itself as
`mitcad --import-worker <file> --output <design> --result <json>` (no
window; `main.cpp`), which runs `import_f3d` with `hang_limit` 90 s and
no time limit on a new document under a job (`attach_job`) and writes the
design file and result; it also gets `--result-store` and
`--persist-min-ms` and stores the design's results. It hands the
import's evaluation over to the window (mitcad#413): with
`--hand-off-store <folder>` (in the import's temporary folder, whether
the user's store is on or not) it writes every result a store takes,
however quick (`hand_off_results`), and prints `import-handed-over <n>`;
the window reads the design with that store (`installDocument`'s
`handedOver`, logged `Results handed over for <file>: restored <n>,
evaluated <m>`), so it restores those results instead of evaluating
every feature again, then gives the document the user's store. A kernel crash or
hang ends only the worker. While import threads the hang watchdog gave
up still run, the worker ends with `std::_Exit` after writing its result
(`abandoned_imports()`), without the static destructors that would crash
under them (mitcad#82). Progress: the worker runs with
`MITCAD_IMPORT_TRACE`, and its per-item lines (`import: [t] item <n>:
<name>` when an item starts, `import: [t] <item>: <outcome> in <s> s`
when done; core/import) are counted against the timeline's length
(`import_f3d`'s `list`); a change of that format only loses the count.
**Stop and Keep What Is Imported** writes `stop` to the worker's stdin;
the worker cancels its job, which `import_f3d` takes as a stop request
(`core/import/README.md`, *Stop*). **Cancel Import** kills the worker;
nothing of it is kept. The window then loads the design unsaved (`*`)
and shows the import report. For tests,
`MITCAD_TEST_RECOMPUTE_DELAY_MS=import_f3d=<ms>` slows every definition
the worker tries.

**Keep the imported bodies?** (mitcad#283) The worker runs `import_f3d`
with `retry_bodies` `ask`: the first time an item is to be tried again on
the file's stored bodies before it, and that would replace replayed
bodies that already match them within the geometric tolerance, the
import asks through the job (`JobControl::question`). A thread of the
worker prints it as `import-ask <question>`; the window shows it in a
question dialog over the progress, **Keep the Imported Bodies?**
(`importRetryBodies`), whose buttons are **Keep Imported Bodies** (the
default, also Esc) and **Replace with Stored Bodies**, and writes
`answer keep` or `answer replace` to the worker's stdin, which passes it
to the job (`JobControl::answer`). The answer holds for the rest of the
import. The import report's **Retries on the file's stored bodies**
section names the answer and, per item retried, the bodies kept and
replaced. Log lines: `Import of <file> asks: <question>`, `Import of
<file> answer: keep` (`replace`), `Import report retry: <file>: <line>`.
For tests, `MITCAD_TEST_IMPORT_QUESTION=1` makes the worker ask at the
start of the import (`tools/ui-import-retry-test.sh`).

**Keep Deviating Results?** (mitcad#379; `files/ImportDeviations`) An item
of an `.f3d` design whose decoded definition builds a result that deviates
from the bodies stored in the file comes in as the file's stored geometry,
and the import report lists it with its numbers (`deviation`,
core/model/src/api/commands.md). Once the import report is closed, with
the design in view, the window asks about those items in one dialog
(`importDeviations`), a row each (table `importDeviationsTable`): **Keep
Parametric** (a check box, Space toggles the selected row; unchecked, *Use
stored*, is the default), the component, the item's name (*without n
edges* for a rounding offered without the edges the kernel fails on,
named in its tooltip) and type, the
volume it adds or removes against the file's (mm³), the difference (mm³
and per cent), the largest distance of its new faces from the file's
(mm), the share of their area beyond the geometric tolerance, and the
volume of the bodies it changed against the stored ones. Selecting a row
selects the faces of the design where the item deviates (the report's
`faces`: of the stored geometry standing in for it), so the view shows
where. Buttons: **Keep All Parametric** (Alt+K), **Use Stored for All**
(Alt+U), **Apply** (the default, Enter: the rows checked) and **Cancel**
(Esc: the design stays as imported). Items kept make the worker run the
import again on the same file and options with `accept_deviations` (the
items kept before and now), which replaces the document: those items are
parametric with their results (the report's `accepted_deviation`, their
features' timeline note *Deviation from the file accepted at import: …*)
and the items after them are replayed on those results. Then the dialog
comes again for the items still deviating that were not decided, and for
those after the first item kept in that run (replayed on its result, their
numbers can change); items left as the stored geometry before are not
asked again otherwise. A stopped import does not ask. Log lines: `Import
deviations: <file>: <n> items`, `Import deviation: <file>: <index> <name>
(<type>): volume change … mm3 against the file's …`, `Import deviation
shown: <file>: <name>, <n> faces`, `Import deviation choice: <file>:
<name> keep|stored`, `Import deviations answer: <file>: keep <name>
(<index>), …` (or `stored for all`), `Import of <file> runs again keeping
<n> items parametric`. UI test: `tools/ui-import-deviations-test.sh`.

**Documents the design refers to** (mitcad#196): before the worker starts,
the window lists the documents an `.f3d` design's components come from
(`import_f3d_timeline` with `list_references`) and, for each one whose
file is not found (no reference folder next to the design, or no file
there for it), opens a file dialog titled with the document's name (where
a reference folder gives one) and version, `Find Part, Version 2`, whose
buttons are **Use** and **Skip**. A chosen file is used for that document
and its folder is searched for the others (by the GUIDs their files
list), so the list is read again after each; a skipped one leaves its
components empty, the report saying `skipped`. The worker gets the chosen
files and the skipped documents as `reference_files` and
`skipped_references` (`--import-options <json>`); the import report lists
the documents referred to, where each came from (core/model/src/api/commands.md,
*Components of other documents*). Test: `tools/ui-windows-references-test.ps1`
(`app.ui-windows-references`).

### Export and 3D Print

**Export:** STEP AP214/AP242, IGES, STL, OBJ, BRep, 3MF (appearance
colours as `colors`), USDZ for all or the selected bodies (faces, edges and
vertices stand for their bodies), and a sketch to DXF (`export_sketch`).
STEP and IGES are written in millimetres unless the Unit row says
otherwise. Bodies are where the design shows them (the model's default;
a STEP of moved bodies is an assembly); with occurrences, Coordinates can
give `coordinates: component`. USDZ (mitcad#123) takes the Refinement
row; the model writes the bodies' appearances itself, and a saved design
passes its folder as `design_folder` for relative texture paths; the log
line adds the materials (`…, 2 material(s)`) and each texture written in
its base colour is a warning (`Export: Wood: the texture image …`). A web
page (`.html`): [Web page](#web-page).

**3D Print** (`make.print3d`; `files/MainWindowPrint.cpp`,
`files/Print3d`) is a panel (mitcad#234; an `accept` command, see
[Glue](#glue)):

- **Bodies** (`SelectKind::Body`, any number): starts with what the
  selection stands for (`printStartBodies`: a face, edge or vertex its
  body, an occurrence the bodies shown in it), or with nothing of that
  selected the bodies with a shown instance (`instances`); a body item per
  placement, with its occurrence. A click on a body in the view adds or
  removes it (Ctrl and Shift as in other inputs, a window adds several);
  the bodies in it are highlighted whole in the input's colour. Its
  `withAccepts` takes solids and mesh bodies only (`geometry::body_kind`):
  sheet and empty bodies cannot be picked, and those of the start
  selection are left out with a warning.
- **Format** (STL, 3MF), **Refinement** (Low, Medium, High, Custom with
  **Deviation** and **Angle**, checked in `build`), **Slicer** (none: open
  the folder; the settings' slicer, the ones found, Other program... with
  a file dialog in its `withOnChange`; a choice is the slicer as JSON,
  `slicerChoice`). They start with the settings, group `print`: `format`,
  `refinement`, `deviation`, `angle`, `slicerName`, `slicerProgram`,
  `slicerArguments`, `slicerArgumentsAfter`; Send (Enter) saves them.
- Send writes the files and starts the slicer (`sendToSlicer`): each body
  once; of a body shown more than once, STL files for the placements
  picked (all when none is); 3MF takes every shown placement of a body
  picked (one `export`). Left room for filament assignment (mitcad#21):
  further rows of the panel.

- Files go to `<QDir::tempPath()>/mitcad-print/<design>`
  (`preparePrintFolder`: emptied before each send; `mitcad-print` is made
  for the user alone and refused when it is a link or another user's,
  since /tmp is shared on Linux), through the model's `export`: 3MF one
  file named after the design with `colors`; STL one per body
  (`printFileNames`: safe and unique, `Body1.stl`, `Body1_2.stl`) in
  design coordinates, and one per occurrence of a body shown more than
  once (`printPieces` from `instances`, `export` with `occurrence`:
  `Pin (Arm_2).stl`).
- The slicer starts once: `QProcess::startDetached(program, arguments +
  files + argumentsAfter, folder)`; without one, or when it fails, the
  folder opens (`QDesktopServices`).
- `findSlicers`: on Windows Program Files and the user's Programs (`Bambu
  Studio\bambu-studio.exe`, `OrcaSlicer\orca-slicer.exe`,
  `Prusa3D\PrusaSlicer\prusa-slicer.exe`, the newest `UltiMaker Cura
  *\UltiMaker-Cura.exe`) and the registry's Uninstall lists (read only);
  on Linux `PATH`, Flatpaks (`flatpak run --file-forwarding <id> @@
  <files> @@`: the sandbox has its own /tmp) and desktop files
  (AppImages). `SlicerPreferencesBox` is the Preferences group.
- Tests: `tools/ui-print-test.sh` (a fake slicer script), the Windows
  workflow test's parts 10 and 15 (the format chosen with the wheel,
  `Ui-WheelLogged`: drop-down lists do not open in session 0), `app.unit`.

### Web page

**Export › Web page** (`.html`, mitcad#423; `web/WebPage`, `web/WebGlb`)
writes one self-contained page: the web viewer and the design, opened
from the disk in any browser without a server or the internet (browsers
block module scripts and `fetch()` from `file://`, so nothing is a file
of its own). A project's `web/` folder holds one such page per design
(`writePageOfFile`).

- **Dialog:** the type *Web page (\*.html)*, also by a `.html` or `.htm`
  name; the Refinement row as for STL and OBJ (Low, Medium, High: the
  deviation and angle of `io::MeshOptions::from`, a doubly curved face
  about 20 000 triangles at most); no Bodies or Coordinates row: the page
  shows every visible body where the design places it, as the view does.
- **Writing** (`exportFile`, on the UI thread under a wait cursor, the
  model idle): `web::writePage(model(), path, options)` with the title of
  the design's name and its folder for relative texture paths. The log
  says `Exported <path>: html, <n> body(ies), <bytes> bytes`, the status
  bar the size as Qt formats it; a failure is a sheet (`Could not
  export:`) and `Export of <path> failed: <reason>` in the log.
- **Bodies** (`instances`, the visible ones): each body meshed once on a
  copy of its shape (`io::triangulate`, then `render::meshOf`: smooth
  normals per face, faces in the shape's order), placed by each of its
  occurrences; faces with appearances of their own by their names
  (`face_appearances`, `find_faces`). A body that cannot be meshed is left
  out with a warning.
- **glb** (`pageGlb`): one node per placed body named `n1`, `n2`, … (its
  extras `body`, `body_name`, `occurrence`: not `name`, where a glTF loader keeps the node's name), with its placement as the node's
  matrix; bodies of the same mesh and look share a glTF mesh, a body's
  faces with their own look are primitives of their own. Materials are
  metallic-roughness ones from the appearances (`render::materialOf`:
  linear colours, metalness, roughness, emission; see-through as opacity
  × (1 − 0.9 × transmission), `BLEND`; double sided). Textures go into
  the glb (PNG or JPEG as they are, others and images over 2048 pixels as
  PNG), with texture coordinates by the projection the rendered view uses
  (planar along Z, else box by each triangle's facing, turned and
  scaled), the normal and occlusion maps as glTF's, the roughness and
  metalness maps combined into its one image; an image that cannot be
  read is left out with a warning. Positions are in the design's unit
  (`mm`, `cm`, `m`, `in`, `ft`; other units: mm).
- **Data** (`#mitcad-data`, format 1; `web/WebPage.hpp`): `title`,
  `units`, `unit_scale` (metres per unit), `up` (`[0, 0, 1]`), `glb`
  (base64), `tree` (one root item `{"id": "root", "name", "nodes",
  "children"}`, the occurrences of `components` below it by their uid
  paths, each with the nodes of its own bodies; occurrences without
  visible bodies left out), `views` (`named_views`: `name`, `eye`,
  `target`, `up`, `orthographic`, `height`), `home` (the named view Home,
  else from the front, right and above), `created`, `generator`.
- **Page** (`pageHtml`): the viewer's `template.html` with
  `<!--MITCAD:IMPORTMAP-->` replaced by an import map of `modules.txt`'s
  modules as `data:text/javascript;base64,…` URLs, `<!--MITCAD:VIEWER-->`
  by `viewer.js` as a module script, `<!--MITCAD:DATA-->` by the data
  (`<` written `<`), `{{TITLE}}` by the title, HTML-escaped. The
  viewer's files are resources under `:/web-viewer/<repository path>`
  (`app/web/viewer/*`, `third_party/three/**`, a glob in
  `app/CMakeLists.txt`); a missing one, or a template without a
  placeholder, fails the export with its name.
- **`writePageOfFile`:** the design file on disk (its newest version) read
  with `load_design`, its links followed, computed with the result store,
  as opening it computes it, without a window; any thread.
- Tests: `app.unit` (`tests/web_page.cpp`: `pageHtml` from fixture files,
  the glb's nodes against the tree), `tools/ui-import-test.sh` and the
  Windows workflow test's part 7 (the log line, the file's size, its
  `mitcad-data`).

### Unsaved changes

The title's `*` and the question on New Design, Open, Close and exit
follow the model's `modified` (`document` query), which compares the
revision with the one marked saved (`mark_saved` after Open and Save; a
new document starts saved, an imported `.f3d` design does not). Undo back
to the saved state clears it (`tools/ui-file-test.sh`).

### Autosave

`files/Autosave`. Every 5 minutes by default (settings `autosave/enabled`,
`autosave/minutes`, 1–60), a document the model calls modified whose
revision was not written yet goes to the recovery folder `autosave` in the
user's local application data (`~/.local/share/Mitcad/Mitcad/autosave`,
`%LOCALAPPDATA%\Mitcad\Mitcad\autosave`; `MITCAD_AUTOSAVE_DIR` moves it),
never a project's folder and never a version.

Each run is a session with a UUID. Its files:

- `<id>.lock`: a `QLockFile` from the first write to the end; stale only
  when the process is gone.
- `<id>.<generation UUID>.mitcad`: an immutable design snapshot.
- `<id>.json`, atomically published last: `"format": "mitcad-autosave"`, `version` 2,
  `session`, `pid`, `application`, `application_version`, `document`,
  `path` and `base_digest` of the file it was opened from or saved to,
  `saved_at`, `revision`, `project` (the snapshot's name; the key keeps
  its old name), and the design file's `size` and `digest`. Digests are
  FNV-1a 64 in 16 hex digits.

The design file is made on the UI thread, never while a job has the
model (`Autosave skipped: busy`, retried in 5 s) or while a sketch
command's preview is applied; an edit that rolled the timeline back is
written with the marker it restores (`to_json_with_marker`). Both files
are written on a thread of their own with `QSaveFile`. The metadata points
to a complete generation; earlier bytes stay until its replacement is
published, so interrupted updates retain the last recoverable snapshot.
Successful publication cleans obsolete snapshots and interrupted writes.
Version 1 metadata with `<id>.mitcad` is still recovered and cleaned up.
Save, Save As, a
new document (New, Open, Close, an import) and undo back to the saved
state remove the session's files; a normal end removes them and the lock;
a crash leaves them. An imported `.f3d` design is written at once.

### Recovery

`files/Recovery`, `files/MainWindowAutosave.cpp`. When the window shows
(`main.cpp`; not with `--no-recovery` or `--screenshot`; after an `.f3d`
given with `--open` has come in) and by File › Recover Documents
(`file.recover`), sessions whose lock `QLockFile::tryLock(0)` takes are
found and held until the user decides (a running instance's sessions are
never offered). A lock without files is removed. A session whose metadata
is missing, unreadable or of another version, or whose design file is
missing or does not match `size` and `digest`, is damaged and can only be
discarded.

Restore reads the design file, follows links relative to it and
computes it as a job (`installDocument`), not marked saved (modified, no
undo history), with the file's path or its untitled name. Autosave writes
it into this session at once, and only then removes the earlier
session's files (`AutosaveManager::recovered`). Later keeps the sessions,
unlocked. Save of a recovered document compares its file with
`base_digest` first (`askOverwriteChanged`) and asks before overwriting a
changed file.

Limitation: only a recovered document's Save checks the file on disk; a
document opened normally does not notice a file changed by another
program (one in a versioned project does at Save); there is no
`QFileSystemWatcher`.

### Result store

`framework/ResultCache`, `framework/CachePreferences`; the model's side in
commands.md, "Result store". Every document the window gets
(`installDocument`: open, recovery, import, New, Close, start-up) uses
`resultStoreDirectory()` (`QStandardPaths::CacheLocation` + `/results`;
`MITCAD_RESULT_STORE` for tests, `off` for none; empty when Preferences
turns it off), with the build id `geometry::kernel_build_id()` and the
document's name. The open job writes the results that took at least
100 ms (`MITCAD_RESULT_STORE_MIN_MS`) in the same job; Save writes them as
a job after the file (`MainWindow::afterSaved` → `persistResults`), under
the saved name; autosave and recovery do not. At start-up and after
writes, a pool thread removes the least recently used files beyond the
size (`result_store_gc`). Settings: `cache/memoryMegabytes` (default a
quarter of the machine's memory, `set_memory_cache_budget`),
`cache/disk`, `cache/diskMegabytes` (5 GB); Clear is `result_store_clear`.

**Help › Diagnostics** (`help.diagnostics`, `framework/CacheDiagnostics`):
the model's `cache` query with the store's files and the process's
memory; Clear Memory is `clear_cache` as a job; Export Report writes the
query's JSON, or text for a `.txt` name.

### Versions

`files/MainWindowVersions.cpp`, `files/VersionDialogs`; the model's side
in commands.md, "Version history".

- `writeFile` asks `open_project` whether the file is in a project with
  version history (`versionedProject`; open a `Project` per use, it is
  not shared between threads); there Save writes the file and then
  records it (`commit` of its path, the author always given) on the UI
  thread. `MainWindow::afterSaved` is where a save's follow-ups go.
- The message is `automaticVersionMessage` of the `changes_since_saved`
  query, read before saving marks the state: `Save <file>: <steps>`, the
  steps' labels joined while the line stays within 72 characters, then
  `and N more` with the whole list in the body.
- Author (`versionAuthor`): the project's (`identity`: the repository's
  own `user.name` and `user.email`, which New Project and Project Settings
  write, else git's configured user), else Preferences' default author
  (`versions/name`, `versions/email`); asked (`askVersionAuthor`) only when
  none has one, and kept as the default.
- Before Save overwrites its file, `askVersionConflict` compares the
  `status` command's blob ids with those of the open or last save
  (`m_versionBase`, by path). After anything that writes the file
  (restore), call `rememberVersionBase` (or reopen) and
  `updateVersionStatus`.
- Opening runs `follow_rename` first: a file renamed or moved outside
  Mitcad within its project takes its display state along, and Save
  records the rename first.
- Projects are made in [Projects](#projects) below; an untitled design of
  a project remembers its folder for Save As (`m_projectDir`, else the
  Projects panel's selection, else the current project's root). In a
  project with versions Save As chooses the project, folder and name in
  the [project picker](#project-picker) (`saveAsProjectPlace`; Another
  Folder... the file dialog; mitcad#176); a design in no such project keeps
  the file dialog.
- Versions are per file: `v3` is the file's history length (`history`),
  not the project's commits.
- Tests: `tools/ui-version-test.sh`; the Windows workflow test's part 8.

**Version History** (`file.version_history`; `files/VersionHistory`;
commands.md, "Version history"): reads the history on its own
thread (`QThread::create` with its own `open_project`; the window waits
for it when it closes): `history`, then `history` with `summaries`.
Compare with the previous version uses `diff` on the UI thread; with the
open design `load_version` and `diff_documents`; Compare Geometry is a job
`compare` (`compareVersionGeometry`). Open reads the version as an
untitled document `part v3` (`openVersion`, marked saved). Restore records
unsaved changes or a change in the folder first, then `restore` and
`loadDesign`. Save Copy As: the place chosen in the project picker
(the version's project and folder; Another Folder... the file dialog;
mitcad#176), then `load_version` and `save_design` `auto`, plus a version
in a versioned project. Previews: after `writeFile`,
`openVersion` and `restoreVersion`, the view is grabbed
(`QOpenGLWidget::grabFramebuffer`) and scaled to 256 px when the blob has
none (`saveVersionThumbnail`), stored as `thumbnails/<blob>.png` in
`AppLocalDataLocation` (`MITCAD_THUMBNAIL_DIR` for the Windows tests).

Limitations: previews exist only for versions saved, opened or restored
here, and are never removed; linked components' relative paths are not
rewritten when a design is saved or copied to another folder or moved
into a new project; a recovered document in a project is saved without
the external-change check; closing the window with a very long history
waits until the summaries are read.

### Remote repositories

`files/RemoteController.cpp`: File › Project › Sync, Check for Newer
Versions, Open in Browser, the automatic push and the remote's state in
the project indicator, for the window's current project
(`Host::projectRoot`). The model's side: commands.md, "Remote
repositories".

| Model command / bridge function | Used by |
|---|---|
| `push` | after each version Save records (Project Settings, else Preferences; on by default); a network failure, `not_found` or `other` retries after 60 s, doubling up to 15 minutes |
| `fetch` | on opening a design in a Cloud project, every 10 minutes (Project Settings, else Preferences; 0: never), at the first change after opening when the last fetch is over 2 minutes old, after a refused push, and Check for Newer Versions; then `push` if versions wait and the remote has none newer |
| `incoming` | the status bar tooltip: who saved the remote's newest version, and when |
| `incoming` with `path` | after a fetch and after Save: the notice of a newer version of the open file (`file_versions`, `differs`, `changed_here`) |
| `sync` (`author`; after a conflict `resolutions` and `fetch` false) | Sync and the notice's Sync Now; `changed_paths` reopens the design |
| `diff` (`from`: the conflict's `theirs.version.id`, `to`: `mine.version.id`) | Compare in Resolve Sync Conflicts |
| `commit` (`Save <paths> before sync`) | a sync refused for `local_changes`: records the files listed in `uncommitted` first |
| `remote_info` | the indicator's state, Project Settings' Status row |
| `git_info_at(program)` | Preferences › Cloud: the chosen git program (a path, or "" for the one found) and its version; the app sets `MITCAD_GIT` from it |

Tests: `tools/ui-sync-test.sh` and the Windows workflow test.

### Projects

Local and Cloud projects (mitcad#89; `files/MainWindowProjects.cpp`,
`files/ProjectDialogs`, `files/ProjectSettings`, `files/CloudSection`,
`files/ProjectIndicator`; commands.md, "Projects"). The window keeps the
current project (`ProjectState`: kind `None`, `Local`, `Cloud` or
`NoHistory`, root, remote) as state of its own; the indicator, the
remote and Project Settings use its root. The commands below, with Sync,
Check for Newer Versions and Open in Browser, make the File menu's
Project submenu (mitcad#175); Open Read-Only is in the File menu itself.

| Command | What |
|---|---|
| `file.new_project` New Project... | the New Project dialog: folder, Local or Cloud, author, the Cloud section; Create (an empty project, or one of the designs its folder has, shown in the Projects panel), Open It, Open It Instead, Create as Local for Now |
| `file.open_project` Open Project... | a project chosen in the [project picker](#project-picker) (the Projects panel's projects, also those without versions; mitcad#176), or a folder (Another Folder...: Qt's folder dialog) recognised with `inspect_folder`: a project shows in the Projects panel and opens its design opened last (`last_design`), else the newest, none when it has none; an older one gets its history (`init_project_history`), anything else goes to New Project with the folder fixed |
| `file.open_read_only` Open Read-Only... | a design opened without an edit lock (`m_openingReadOnly` for the lock controller) |
| `file.open_remote` Open from Cloud... | the Open from Cloud dialog: clone, Create Project Here, Make It a Project, Open It |
| `file.project_settings` Project Settings... | the current project's settings (a loose design: Move to a Project offered); a **Display name** first, the name the Projects panel shows on this computer (`set_project_settings`' `local.name`; empty: the default; never renames the folder or the repository) |
| `file.move_to_project` Move to a Project... | a loose design into a new project (New Project, its own folder by default) or an existing one, chosen in the [project picker](#project-picker) with a folder in it and the design's name (a clash shown there; Another Folder... the folder dialog, the project then a known one); an untitled design's name always asked in the picker; the moved design shown selected in the Projects panel (mitcad#174); also in the indicator's menu and Save with Description's and Version History's questions |
| `file.web_viewer` Web Viewer..., `file.update_web_viewer` Update Web Viewer | the current project's web viewer (mitcad#423, [Project web viewer](#project-web-viewer)): every design's page and the index written into `web/` and recorded as a version; Web Viewer turns it on, Update writes it again; only in a project with versions |

| Model command | Used by |
|---|---|
| `inspect_folder` | the current project of a design opened or saved (`projectOfFile`); Open Project; the folder checks of New Project and Open from Cloud (a thread of their own, after a pause while typing) |
| `check_remote` | the Cloud section of New Project and Open from Cloud (a thread of its own, after a pause, and when the dialog gets the focus back) |
| `remote_check` (a project's) | the Cloud section of Project Settings' Share and of Change Address (`related`) |
| `create_project` (no `design`: the project starts empty, mitcad#145; `url` for Cloud; `shared` with New Project's live updates) | New Project's Create, Create as Local for Now, Open from Cloud's Create Project Here; with `init_bare` first for a new shared folder |
| `clone_project` (`adopt` for Make It a Project) | Open from Cloud |
| `connect` with `onto_files` | Project Settings' Share (conflicts in Resolve Sync Conflicts) |
| `remote_set`, `remote_remove` | Change Address, Stop Syncing |
| `project_settings`, `set_project_settings`, `set_identity`, `identity` | Project Settings (a change for everyone is a version, sent as a saved one is); the sending and checking of the remote |
| `remember_design` | each design opened in a project (Open Project opens it next time) |
| `project_settings`' `authors` | Share lists the authors of the versions to publish |
| `git_info` (`credential_helper`, `author`) | HTTPS or SSH by default; the author New Project offers; Cloud off without git |
| `ssh_public_key`, `host_keys`, `trust_host_key` | Copy Public Key; Trust This Server |

Tests: `tools/ui-project-test.sh`, `tools/ui-cloud-test.sh`,
`tools/ui-cloud-address-test.sh` (a pasted git address), `tools/ui-sync-test.sh`,
`tools/ui-version-test.sh`, the Windows and macOS workflow tests.

### Projects panel

The projects, their folders and designs in one panel (mitcad#145;
`projects/ProjectsPanel`, `projects/ProjectsTree`,
`projects/MainWindowProjectsPanel.cpp`): a dock above the browser's in the
Docked layout, a card above the browser's in the Floating one (none in the
AppKit frontend). The tree is project → folders → designs (mitcad#173):
a project row is the repository's root, nothing above it, below it only
the folders and designs it holds; it has Local or Cloud (its icon and
tooltip too) and its sync state as the indicator shows it, a design row
its version, Changed and its edit lock, the open design in bold. The
projects are the known ones (`ProjectsSettings`: made, opened or cloned
here, newest first; `projects/known`) and those of the projects
folder (`projectsDirectory()`) that Remove from List did not take out
(`projects/hidden`). Whether the panel shows is `projects/panel`, the
user's last show or hide (View › Projects, the dock's close button, New
Project and Open Project showing a project); before the first one it is
never written, and the panel starts shown when a project is configured (a
known project whose folder is there, or one of the projects folder's that
was not removed from the list; `anyProjectConfigured`, folder entries
only), hidden otherwise (mitcad#151), in both layouts. The
trees are read on a thread (`project_tree` per project, a `RemoteTask`),
again after a save, a sync, versions sent, the window's focus back and
each change made in the panel; while a name is typed or a row dragged the
rows wait.

A project row shows its name (mitcad#152, #173), and nothing else: the
display name set in Project Settings (`project_tree`'s `display_name`,
`local.name` of the project's settings, kept in `.mitcad/local/name.json`
on this computer), else a Cloud project's repository address without the
scheme (`repositoryAddressOf`: `github.com/owner/repository`, GitLab with
its subgroups, Forgejo, another server's host and path without `.git`, a
shared folder's path), else the folder's name. The projects are sorted
by it, case-insensitive. The rows' keys (the logs, the expanded rows, the
selection) start with the folder's name (`Shop`, `Shop/parts/`; the
whole path when two listed projects' folders have the same name), which a
display name does not change.

| Command | What |
|---|---|
| `view.projects` Projects | shows or hides the panel (View menu, checkable; the dock's close button too) |
| `file.new_design` New Design... (Ctrl+N) | File menu and the indicator's menu: a design named in place in the panel's selected folder of the current project (its top when another project or none is selected), recorded (`new_design`) and opened (the open design's versions sent and its edit lock released first: a refused release makes nothing); without a current project (also one without versions) an untitled design, as `newDocument` (mitcad#175: New and New Design are one command) |

In the panel (toolbar, context menu, keys): **Open** (double-click, Enter;
unsaved changes asked as with Open), **New Design** and **New Folder**
(named in place), **Rename** (F2 and the context menu of a design or a
folder; no toolbar button), **Delete** (Del; confirmed, with the
designs it holds, changes no version holds and whether it is open), a
**drag** onto a folder, a project or a design (its folder) of the same
project (`move`), **Version History...** (the design opened first),
**Show in Folder**, **Project Settings...** (mitcad#152, #153: the
toolbar's button for the selected project, folder or design's project;
a double-click on a Cloud project; the current project's as
File › Project Settings, another's without opening it, through a
`ProjectSettingsHost` that refreshes the panel, sends a recorded version
in the background and has no lock probe), **Sync Now** (another
project opened first, as Open Project), **Remove from List** (nothing is
deleted), **New Project...**, **Open from Cloud...**, **Add Existing
Project Folder...** (a folder that is no project: Make It a Project). An
open design that is moved or renamed follows its new path (its version
base and edit lock: `openDesignMoved`); a deleted open design closes. Save
of an untitled design starts in the panel's selected folder (the project
picker's preselection, `saveAsProjectPlace`; the file dialog's
`fileDialogDirectory` after Another Folder...).

| Model command | Used by |
|---|---|
| `project_tree` (with this run's `session`) | the panel's rows, on a thread of their own |
| `new_design`, `create_folder`, `move`, `delete` (`author`, `session`, `push: false`) | New Design, New Folder, Rename and drags, Delete (`force` only after a second question when the core answers `local_changes`); a refusal (`exists`, `locked`, `invalid_path`, `not_found`, `unsupported`) is shown and changes nothing. Each is a version, sent as Save's are: the current project's by `RemoteController::versionRecorded`, another Cloud project's by a `push` task |

Tests: `tools/ui-projects-test.sh`, `tools/ui-projects-settings-test.sh`
(the default shown or hidden, the remembered choice, the flat tree, the
repository's address, the double-click, the display name, the settings
button; mitcad#151–#153, #173)
and the project tests above.

### Project picker

`projects/ProjectPicker` (mitcad#174): instead of a file or folder dialog,
a place, a design or a project chosen among the projects the Projects
panel lists (`readProjects` on a thread of its own: the known ones and
those of the projects folder), each by the name the panel shows
(`ProjectRow::name`), with its Local or Cloud icon and its kind, remote
address and folder in the tooltip. `pickInProjects(parent, options)`:

- **Place** (`Mode::Place`): a project with versions and one of its real
  folders (`project_tree`'s `folders`), the preselected project first and
  expanded (`root`, `folder`), and the design's name typed in the dialog
  (`.mitcad` added; `ownFile` is no clash); a clash with a file there, or
  a name no file can have, is shown under the name and the accept button
  waits. Move to a Project's existing project ("Choose a Project", the
  Projects panel's selection preselected), an untitled design moved into
  a project and a clash of the moved design's name ("Save in a Project",
  "Choose a Project").
  Save As in a project ("Save in a Project": the design's project and
  folder, else the panel's selection; its own file no clash unless the
  window is read-only) and Version History's Save Copy As ("Save a Copy in
  a Project"), mitcad#176.
- **Design** (`Mode::Design`): a design of a project with versions, the
  given project first and expanded (`ownFile` not offered): Insert
  Component in a project ("Insert from a Project"; the linked or copy
  question as before), mitcad#176.
- **Project** (`Mode::Project`): a project, also one without versions:
  Open Project ("Open a Project"), mitcad#176.

The secondary button (`otherText`: Another Folder..., Another File...)
returns `other`: the caller asks with its file or folder dialog, whose
title differs from the picker's. Missing and unreadable projects are never
offered. `ProjectsPanel::revealDesign` selects a design once a read lists
it (`MainWindow::showDesignInPanel`).

Tests: `tools/ui-project-test.sh` (Move to a Project, Open Project, Save As
with Another Folder..., Insert Component), `tools/ui-version-test.sh` (an
untitled design moved into a new project, Save Copy As, Save As of an
opened version, Open Project's Another Folder...),
`tools/ui-projects-test.sh` (Save in the selected folder, Open Project),
`tools/ui-locks-test.sh` (Save As of a read-only window).

### Edit locks

`files/LockController.cpp`, `files/MainWindowLocks.cpp` (mitcad#89; the
user's side in [docs/user-guide.md](../docs/user-guide.md#edit-locks),
the core's in commands.md, "Edit locks"): the open design's edit lock in a
Cloud project with edit locks on, and the window's read-only mode. The
lock commands run as `RemoteTask`s, one at a time, with a project of
their own; `MITCAD_LOCK_TIME_SCALE` speeds up their timers as it does the
core's clock (UI tests). What others should learn at once (locks,
requests, receipts, answers, withdrawals, who has the design open) goes to
the live controller through `LiveLink`, and its events come back through
`LockEvents` (`files/MainWindowLive.cpp` joins them): a lock, request or
version event polls at once, open events list who else has the design
open, a holder's offline will asks whether to take the lock.

| Model command | Used by |
|---|---|
| `lock_take` (`poll_seconds`, `mqtt`; `take_over` with the lock's commit), then `lock_status` (`network` false: `newer`) | a design of a Cloud project opened (not by Open Read-Only), Edit in the banner, Take Over, a request whose holder did not answer, the remote out of reach (tried again every poll) |
| `lock_poll` (`mqtt`) | every `poll_seconds` (2 minutes while live updates are connected) while the design has a lock or is read-only; at once on a live lock, request or version event |
| `lock_refresh` (`active_at`, `state`, `idle_minutes`, `poll_seconds`, `mqtt`) | every half idle time, when the lock turns idle or active, after Save |
| `lock_request` (`message`), `lock_answer` (`declined`, `keep` for 15 minutes) | Request Edit Access; the holder's Decline and Keep 15 More Minutes |
| `lock_hand_over` (`to`) | the holder's Release, a request while the lock is idle, a request not answered within the idle time |
| `lock_release` (`all`) | closing the design or quitting, the idle time, Release Edit Lock, edit locks turned off, Stop Syncing |
| `lock_probe` | Project Settings of a Cloud project, once per remote: a remote that refuses lock refs disables the Edit locks row |
| `sync_plan` (`fetch` false, `locks` `last`, `session`) | before Sync sends versions: files someone else holds (Send Anyway) |
| `sync` (`RemoteController::syncNow`) | the versions sent before a release or a hand-over |
| `load_version` (bridge) | a read-only window shows the editor's version after a fetch (`incoming`) |

Read-only windows: `MainWindow::isAvailable` disables all but the view's
commands, inspections and `file.export`, `file.version_history`,
`file.sync`, the project and opening commands, `make.print3d`, the
libraries' browsing; `MainWindow::command` refuses every model command
but `set_isolation`, `set_origin_visible`, the analyses' commands,
`export`, `export_sketch` and the reading ones (`ReadOnlyError`); Save
offers Save as Copy and Save as New Version for unsaved changes kept from
editing; sketch mode, command panels and Restore are refused.

Tests: `tools/ui-locks-test.sh` (two and three instances), the Windows
workflow test (Take Over after a killed session).

### Live updates

Live updates through an MQTT broker (mitcad#89; `files/LiveController`,
`files/LiveDialogs`, `files/LiveBrokers`, `files/Keychain`,
`files/MainWindowLive.cpp`; commands.md, "Live updates"). One `LiveHub`
for the application: its commands on the UI thread, its events read on a
thread of the controller's own and handled on the UI thread. The window's
current Cloud project subscribes when it opens with live updates set
(Project Settings, for everyone or on this computer) and Preferences ›
Cloud allows them, and unsubscribes when another project opens or it stops
syncing; quitting closes the hub with `wait_ms` (a clean leave).

| Hub command / model command | Used by |
|---|---|
| `project_settings` (`live_updates`), `project_id`, `identity` | the broker and prefix this computer uses, the project's id on the broker (its first commit), the name others see |
| `subscribe` (`broker`, `prefix`, `session`: autosave's, `user` and `password` of the keychain over TLS only) | a project opens (after the trust question and the keychain), its live settings change, a sign-in; a new connection for another user unsubscribes the old one |
| `unsubscribe` | another project opens, the live settings change, Stop Syncing, Preferences' Forget and Sign Out, live updates turned off |
| `publish` | the lock controller's `LiveLink` calls; `version` after a push or a sync that pushed (`RemoteController::versionsSent`); `open` and `closed` for the open design while no lock controller does it |
| `connect` with a password over `mqtt://` | the sign-in dialog: the core's refusal (`insecure`) shown in it |
| `test` (on a thread of its own) | Project Settings' Test: its steps in a dialog |
| `close` (`wait_ms` 2000) | quitting |
| events: `state`, `subscribed` | the indicator's `Live` / `Live offline (polling)`; `auth_failed` shows the sign-in notice; a subscription after a lost connection checks the remote |
| events: `version` (not this session's) | the remote checked at once (`RemoteController::versionAnnounced`): the indicator's newer versions and the notice of a newer version of the open file |
| events: all of a project's | `LockEvents::liveEvent` of the lock controller, and `LockEvents::liveStateChanged` when the connection comes or goes |

While the project's connection is up the remote is not checked on the
timer (`RemoteController::setLive`). The trust answers are the settings
`live/brokers` (address, trusted); credentials are QtKeychain entries in
the service `Mitcad live updates`, keyed by host and port, holding the user
name and the password; without a keychain they are kept for the session.

Tests: `tools/ui-live-test.sh` (against a mosquitto it starts; skipped
without one).

### Feedback and error reports

`report/` (mitcad#61, mitcad#62; the user's side in
[docs/user-guide.md](../docs/user-guide.md#feedback-and-error-reports),
the design in [docs/architecture.md](../docs/architecture.md)).

- **Help › Send Feedback** (`help.send_feedback`, an action in the Help
  menu and the command search): `FeedbackDialog` with the window grabbed
  before it shows; Preview builds a `report::Report` (sections
  description, contact, the recent actions for a bug, diagnostics) and
  shows `ReportPreviewDialog` over the form, which closes once Send is
  done.
- **Every report** goes through `ReportCenter::previewAndSend`: the title
  and the sections marked `masked` are masked (`report::mask`; the contact
  address is not), the preview edits them, and `deliver` builds the
  Markdown (`report::reportBody`) and the address
  (`report::issueLink`, at most 8000 characters: longer, the body is cut
  at a line with a note and the whole goes to the clipboard), saves a
  screenshot under `reports` in the local app data and opens the address
  (`openExternalUrl`). Nothing goes over the network from Mitcad.
- **The tracker:** `MITCAD_ISSUE_URL` (CMake cache variable, default
  `https://github.com/mitcad/Mitcad/issues` from README.md), overridden
  by the setting `reports/issueUrl`: an issue list's address (`/new` and
  `title`, `body` added) or a template with `{title}` and `{body}`.
- **Crashes:** `crash::install` at the start of `main` (the application
  and the import worker; `mitcad-render`'s `main` too) and
  `ReportCenter::prepareCrashReports` once the settings are known: the
  folder (`crashes` in the local app data, `MITCAD_CRASH_DIR`), the
  workers' environment (`MITCAD_CRASH_DIR`, `MITCAD_CRASH_PARENT`) and the
  Rust panic hook (`mitcad_set_panic_sink`, `core/ffi/src/panic_note.rs`).
  `ReportCenter::startUp` watches the folder: a report of this process's
  worker is offered at once, the others' at the next start (the newest,
  counting the rest); offered ones become `*.crash.offered` (20 kept).
- **Internal errors:** `MainWindow::showError` passes every message to
  `ReportCenter::errorShown`, which offers the kernel's converted crashes
  (`report::isKernelCrashMessage`); `runCommand` offers an exception that
  is not the model's (`internalError`). Once per message and run.
- **Recent actions:** `MainWindow::trigger` (`command <id>`) and
  `runCommand` (`model <cmd>`), the last 32, also in the crash handler's
  buffer; never arguments or names from the design.
- Offers wait for a free window (`offerNext`: no modal dialog or popup,
  no job, no import); `reports/offerErrors` false turns them off.

Test switches: `MITCAD_TEST_LOG_URLS=1` (set by `tools/ui-test-lib.sh`)
logs an address instead of opening it; `MITCAD_TEST_CRASH=app`,
`model-worker` (once the window shows), `import-worker`, `render-worker`
(at the worker's start) crash there; `MITCAD_TEST_OCCT_CRASH=<operation>`
makes a kernel crash for an internal error. `tools/ui-report-test.sh`.

### Component libraries

`files/MainWindowLibraries.cpp`, `files/LibraryDialogs.cpp`,
`files/Libraries.cpp` (mitcad#64, mitcad#63; the formats in
[docs/libraries.md](../docs/libraries.md), the model's side in
commands.md, "Component libraries"). The commands are in Tools ›
Libraries and the command search; Insert from Library is also in the
SOLID tab's ASSEMBLE group.

| Command | What it does | Model command / bridge |
|---|---|---|
| `insert.library_component` Insert from Library | `LibraryBrowser`: search (words, a licence filter, items without a licence only when asked), the results as the part picker's grid (`PartGrid`, [Part picker](#part-picker): groups with Favourites and In this design, stars, the tiles' size, the hover's larger picture; an index's libraries not fetched in "Libraries to get"; a double click or Return inserts), details with the licence and the attribution, the version (newest first) and the size (cascaded combo boxes per selector, `SizeChooser`), linked or a copy; **Update** fetches every enabled source (as Fetch All, one warning for the failures) and searches again, keeping the selected item; while no enabled source is fetched the empty list says to press Update (mitcad#317) | `library_search`, `library_show`, `library_preview`, `library_list`, `library_fetch`; then `insert_component` with `library` |
| `tools.library_parts` Library Parts... | `LibraryPartsDialog`: each part's recorded version and size, another version or size, Show Changes, Update; Check for Newer Versions and Get Missing Libraries fetch (the latter after showing the addresses, which come from the design); the Parts List tab (Copy as CSV) | `library_parts`, `parts_list`, `library_show`, `library_diff`, `library_fetch`; then `update_library_parts` |
| `tools.libraries` Libraries... | `LibrariesDialog`: the sources (settings `libraries/sources`: URL or folder, on or off; the defaults are Mitcad's fastener library and the community index), Add URL, Add Folder, Remove, Fetch Selected, Fetch All | `library_list`, `library_fetch` |
| `tools.community_library` Community Library... | the browser with the indexes' libraries first; Get Library adds a library an index lists to the sources and fetches it after a question with its address and licence | as Insert from Library |
| `tools.publish_library` Publish to Library... | `PublishLibraryDialog`: the open design (a single file) added to a library folder of one's own (made when new), an image of the view as its preview, recorded as a version of the folder's project, pushed (`connect` with a remote URL, else `push`), and the index entry (copied) | `library_init`, `library_add`, the project's `commit`, `connect`/`push`, `index_entry` |

- Where fetched libraries are kept: `MITCAD_LIBRARIES_DIR`, else
  `libraries` in the local app data; `configureLibraries()` tells the
  bridge at start (`configure_libraries`).
- Fetches run as `RemoteTask::library` on a thread of their own with a
  progress dialog whose Cancel ends git; only when the user asks
  (nothing checks for newer versions on its own). The other library
  commands are local and run at once.

Logs: `Library fetched: <kind> <name> (<id>) from <url>; versions ...`,
`Library fetch failed: <url>: ...`, `Library fetch of <n> source(s): <m>
failed` (Fetch All, Update), `Library browser update: <n> enabled
source(s)`, `Library browser empty: nothing fetched yet, press Update` (or
`nothing found`), `Library browser kept the selection <key>`, `Library source <url> (on|off):
<what>; versions ...`, `Library source added|removed: <url>`, `Library
search '<text>': <n> components, <m> libraries, <k> hidden by the
licence`, `Library browser selected <library>/<component>: version ...,
size ..., licence ...` (or `selected library <id> (<url>), licence ...,
fetched|not fetched`), `Library size <row>`, `Inserted component <name>
(linked|copy) from <library> <version>, licence <spdx|none>`, `Library
part <name>: <library> <version>, size <row>, <status>`, `Library changes:
...`, `Updated library parts: ...`, `Parts list row: <qty> x <part>
(...)`, `Publish: ...`. Tests: `tools/ui-library-test.sh`,
`tools/ui-windows-library-test.ps1` (ctest `app.ui-windows-library`).

### Part picker

`library/PartPicker.*`, `library/PartFavourites.*`, `library/UsedParts.*`
(mitcad#418). The library's parts as pictures, for Insert from Library
(the browser's results) and the commands that choose parts (BUILD's
Profile, Panel, Linear Part and Connector).

- **`PartGrid`** (a widget): the groups on the left (Alt+O): Favourites
  first, In this design (only while the design uses one of the parts
  listed), All parts, then the library's categories (`category`, shown
  with its id's words: `linear-parts` as "Linear parts"), Other (no
  category) and Libraries to get (an index's libraries not fetched, in
  the browser). Above the grid: Search (Alt+S; the words `library_search`
  matches: name, id, standard, description, category, keywords,
  supplier, part number, library), the slot System (Alt+Y; only when a
  part has one), Star and the tiles' size (Tiles, Alt+T, 48 to 192
  pixels, the setting `partPicker/tileSize`). A tile: the part's preview
  (`library_preview`, read a few at a time while the application is
  idle, only for tiles a view paints, kept for the session), its name,
  its main sizes (a BUILD part's section and row, else its standard),
  `×N` when the design uses it, a star in the corner. Refused parts
  (`library_build_candidates`' `refused`) are greyed and cannot be
  chosen. Hovering a tile shows a larger picture with the sizes, the
  supplier, the part number, the library and its version, the licence
  and the reason a refused part does not fit. Up and Down in the search
  move through the tiles; a double click or Return activates a tile.
- **Favourites**: a click on a tile's star (or Star, Ctrl+D) stars the
  part; a click on a group's star (Ctrl+Shift+D) stars the group. Stars
  are the user's (`PartFavourites`, settings group `partPicker`, keys
  `starredParts` and `starredGroups`), keyed by the library's id and the
  component's id, so they survive library updates; a starred part that
  is not listed is left out of Favourites. Order (`pickerOrder`,
  `groupOrder`): starred first, then the rest, each in the given order;
  starred groups first after Favourites, In this design and All parts;
  in Favourites the parts of starred groups first. Search and the system
  filter work inside every group.
- **In this design**: `usedLibraryParts` (the model's `library_usage`
  query): the BUILD sources and inserted library components with how
  many times the design uses each.
- **`PartPicker`** (a non-modal dialog of the main window,
  `PartPicker::of(window)`, shown without taking the focus from the
  view; Docked and Floating chrome alike): `ask(PickerRequest)` with the
  command's id, a title ("Part Picker - Profile"), the parts
  (`buildCandidates(fields)` runs `library_build_candidates`;
  `pickerParts(QVector<build::Part>)` turns a BUILD catalog's parts into
  tiles with their categories and licences from one `library_search`),
  the current key and the system to start with. A click (or Return) on a
  tile emits `chosen(part, command)`; `commandEnded(command)` hides it
  unless **Pin** (Alt+P) is on; a pinned, open picker follows the next
  `ask`. `setDesignParts(reader)` and `designChanged()` keep In this
  design current. Its place and size: the setting `partPicker/geometry`.

Logs: `Part picker opened: <n> parts for <command or dialog>`, `Part
picker follows <command>: <n> parts`, `Part picker group <group>: <n>
parts, first <library>/<component>` (every time the tiles change: a
group, a search, the system, a star), `Part picker search '<text>'`
(searched by the grid), `Part picker system <system|all>`, `Part picker
in this design: <n> parts: <library>/<component> x<count>, ...`, `Part
picker star <component> on|off`, `Part picker group star <group>
on|off`, `Part picker chose <library>/<component>[ (<row>)]`, `Part
picker refused <library>/<component>: <reason>`, `Part picker pinned
on|off`, `Part picker closed with <command>`, `Part picker stays pinned
after <command>`. The window's places, once laid out after it shows,
moves, resizes or its tiles change (for UI tests): `Part picker place
search|star|pin at x,y; in the picker x,y`, `Part picker group row
<group> at ...` (the middle of the row, clear of its star) and `Part
picker tile <library>/<component> at ...` (the tiles in sight, up to
24): in the main window's coordinates as the panel's places, then in
the picker's own (the Windows tests post their clicks to its window).
Tests: the data in `tests/part_picker_data.cpp` (test_app), Insert from
Library in `tools/ui-library-test.sh` (part 7) and
`tools/ui-windows-library-test.ps1` (part 4), BUILD in
`tools/ui-build-test.sh` (part 8) and `tools/ui-windows-build-test.ps1`
(part 8).

### Design policy

`browser/PolicyDialog.cpp`, `BrowserController::openPolicy`,
`MainWindow::startSketch` (mitcad#220; the model's side in commands.md,
"Design policy").

| Command | What it does | Model command / query |
|---|---|---|
| `assembly.policy` Design Policy... (command search; the browser's component menu, last entry, opens it for that component) | `PolicyDialog`: the nine settings of the component or the root, each with what it allows in words (incoming and outgoing links told apart; the first-sketch pair, Alt+I and Alt+K, next to the sketch attachment pair, mitcad#293), the presets (Alt+H Hybrid, everything allowed, the defaults; Alt+P Part; Alt+A Assembly; mitcad#421), Alt+T for the incoming sketch attachment setting, and next to a blocked setting the number of objects that break it, a button that lists them; counts follow the edited settings; OK sets them as one undo step, Cancel changes nothing | `policy` (with the proposed `policy`), then `set_policy` |

- A new design (File > New, the window's first design) has the root and
  an empty Part1, active, both Hybrid (mitcad#421): Part1 takes sketches,
  features and components, and other components' sketches may lie on its
  faces. Opened and imported designs are not rearranged; a stored policy
  keeps its values, an owner without one is Hybrid.
- New Component (the command and the browser's) makes a component with
  the defaults, Hybrid. Made in a component that takes no components (one
  set to Part), or by a feature's New Component operation there, the
  model places it
  beside the part, in the nearest owner above that takes components
  (mitcad#292): the reply's `placement_warning` is shown as a hint, also
  when the command's panel closes (instead of "Done.").
- Create Sketch asks the model (`sketch_supports`) first: an active
  component that takes no sketches refuses with the reason and the user
  activates another one or changes the policy (nothing is redirected).
  While a plane is picked, the faces and planes of owners the policy
  leaves out (either side refuses the attachment) are not picked or
  highlighted, the hint names them, and a browser row, a selected face or
  Create Sketch on a plane of theirs is refused with the reason. The
  active component's origin planes are always its own. Linked projections
  are not filtered: the model checks the reference settings and refuses
  with the reason (a projection without Projection Link is always allowed).
- Commands that the policy refuses (features in an Assembly, components in
  a Part with no owner above that takes them, a move into a component that refuses what moves) show the
  model's reason in their panel's preview or when OK is pressed. Under the
  defaults nothing is refused and no hint names left-out owners.

Logs: `Placement warning: <text>`, `Create Sketch refused: <reason>`, `Create Sketch: policy leaves out
<owners>`, `Sketch <name> is in <component>`, `Design Policy dialog opened
for <name>`, `Policy dialog <name>: <preset|custom>; <setting>
allowed|blocked (<count>), ...`, `Policy dialog preset <name>`, `Policy
violations of <name> <setting>: <objects>`, `Policy of <name> set:
<preset|custom>`, `Design Policy dialog closed without a change`; with
`MITCAD_LOG_PICKS`, Create Sketch's `Pick places:` lists the faces it can
pick (`Pick face <body>/<face> in <occurrence> at x,y`). Test:
`tools/ui-windows-policy-test.ps1` (ctest `app.ui-windows-policy`).

### Project web viewer

`web/MainWindowWeb.cpp`, `web/ProjectWeb` (mitcad#423); the pages
themselves are `web/WebPage` (`writePage`, `writePageOfFile`), the model's
side is commands.md, "The web viewer". A project's `web/` folder:

| File | What |
|---|---|
| `web/index.html` | plain HTML and CSS, no scripts (light and dark by `prefers-color-scheme`): the project's folder name, a card per design (its thumbnail or "No preview", name, path, "Saved by <name>, <UTC time>") linking its page |
| `web/<slug>.html` | the page of the design's newest version (one self-contained file) |
| `web/<slug>.png` | its thumbnail: the version's preview (`versionThumbnailPath`, 256 px), when there is one; else one written before stays |
| `web/designs.json` | `[{"name", "file", "page", "thumbnail", "saved_by", "saved_at"}]`, sorted by `file` ignoring case; null where unknown |

`<slug>` is the design's path in the project with `/` as `--`, other
characters outside `[A-Za-z0-9._-]` as `-`, without `.mitcad`
(`designSlug`); a clash ignoring case, and `index`, get `-2`, `-3`, ... in
the order of the paths (`designSlugs`).

| Command / moment | What it does | Model commands |
|---|---|---|
| `file.web_viewer` Web Viewer... (File › Project) | every design `inspect_folder` lists: its newest version's author, time and blob (`history`); its page with `writePageOfFile` in a job (`runJob`, the progress dialog per design, Cancel between designs: nothing recorded); the thumbnails (the open design's preview saved first when it is unchanged); `writeIndex`, which also removes the pages and thumbnails the previous `designs.json` listed for designs gone (only plain file names of the folder). Off before: one version with the setting, `Change project settings: web viewer on`; on: as Update | `inspect_folder`, `history`, `set_project_settings` with `shared.web_viewer` and `paths` |
| `file.update_web_viewer` Update Web Viewer (File › Project) | the same when the viewer is on (a hint when it is not), recorded as `Update the web viewer` | `commit` |
| Save (`writeFile`, before `recordVersion`) | when the viewer is on and updated on each Save (`webViewerOnSave`): unless the file is as its latest version (no version follows), the open design's page (`writePage`, the design file's folder for textures), its thumbnail (`saveVersionThumbnail` of the file's blob, copied), and the index with the other designs as `designs.json` had them (while there with the same slug); these files and those removed go into the design's version | `info`, `status`, `inspect_folder`, then Save's `commit` |
| Project Settings › Web viewer | **Update the web viewer on each Save** (Alt+U; off and disabled until Web Viewer turned it on): for everyone, a version `Change project settings: web viewer (not) updated on each save` | `set_project_settings` with `shared.web_viewer.update_on_save` |

Both commands are enabled in a project with versions; a page that could
not be written is left out of the index and named in a warning. Logs:
`Web viewer: page of <path>: <slug>.html (<n> bytes[, thumbnail])`,
`Web viewer: wrote <n> page(s) and the index in <folder>[, removed
<files>]`, `Web viewer: recorded as version <id>` (or `no new version`,
`not recorded: <why>`), `Web viewer: not turned on in <root>`, `Web
viewer: wrote the page of <path> and the index in <folder> (<n> bytes)`,
`Web viewer: not updated on Save of <path> (off in Project Settings)`,
`Web viewer: <path> unchanged, its page kept`, `Project Settings web
viewer: not turned on | on, updated on each Save | on, not updated on
Save`, `Project Settings: web viewer updated on each Save on|off,
recorded: <id>`. Tests: `tests/project_web.cpp` (test_app),
`tools/ui-web-viewer-test.sh`, `tools/ui-windows-web-viewer-test.ps1`
(ctest `app.ui-windows-web-viewer`).

Limitations: a design that was never saved, opened or restored here has
no preview, so its card shows "No preview" until a Save; two people who
save different designs both change `index.html` and `designs.json`, which
a sync then asks about (take either; Update Web Viewer writes them again);
the pages are whole files in the history (each version of a page is a new
blob).

## Selection

`OcctViewer` names picks through the bodies' `geometry::Shape`
(`names_of_face`, `name_of_edge`, `name_of_vertex`) and reports them as
`SelectionItem`s; it keeps no selection of its own. Its pick filter
(`setPickFilter(kinds, test)`) also limits the hover highlight; a
selection scope (`setPickScope`, sketch mode and Project, see [Sketch
selection filters](#sketch-selection-filters)) also tells its test how an
item is picked. Selection outside commands is the main window's
(`m_selection`). Outside sketch mode a window selection keeps one kind
(profiles, bodies, edges, faces, vertices, sketch curves, points, datums,
in that order of preference), and Shift with a click adds or removes as
Ctrl does. Faces win over bodies, so the bodies filter is off by default.

The highlights look alike (mitcad#149): the hover (`setHoverColor`,
OCCT's dynamic highlight styles, set up by `setHighlightStyle`) and the
selections (`setHighlights`) cover a face half see-through (the hover;
selections 35 %, whole bodies 55 %) in a colour that light does not
change (`flatMaterial`: emission only, no specular), so the face's own
shading shows through and the highlight does not wash out to white
where the light falls on the face head-on. The face's boundary, edges and
vertices are drawn opaque in the colour, edges 3 px wide. The highlights
are in the top layer, which keeps the scene's depth: nearer geometry
hides them (OCCT's own default put the hover of a face, edge or vertex in
the topmost layer, which clears the depth), and a polygon offset brings a
face's highlight just in front of the face. Test:
`tools/ui-hover-test.sh`.

### Selection inputs

A command's selection input keeps each model item once (mitcad#199,
`sameEntry` in `framework/Selection.hpp`). A profile is a region of a
sketch: the occurrence that places the sketch in the view is not part of
it, and a definition keeps the key it stored for a region after the
sketch changed while the view names the region by its current key; the
model's `profile` query names the region a key means now
(`CommandSession::resolver`, asked once per item and pick). So:

- A click toggles: a click on a selected region takes out every entry for
  it (`toggleEntry`), whatever key or occurrence it was put there with;
  clicking a region twice gives back the selection before, in any order.
  The model composes the selected regions odd/even ([commands.md, Profile
  selections](../core/model/src/api/commands.md#profile-selections)):
  clicks on nested squares' rings and centre leave exactly the regions
  clicked an odd number of times.
- Selections filled from elsewhere than clicks (the newest profile of
  `init`, the selection made before the command, a definition loaded for
  editing) are made canonical when the command starts
  (`canonicalizeSelections`): profiles placed by the occurrence the view
  shows the sketch in (`CommandHost::profileOccurrence`), so that their
  highlights lie on the shown profiles and a click on one is on its entry,
  and each region once (`uniqueProfiles`). Before, a click on the newest
  profile of a placed component's sketch added the region a second time,
  and the feature failed with "profile … is listed more than once".
- A region a sketch edit split (mitcad#407) is all its pieces: the
  `profile` query's `regions` lists them, the main one (which keeps the
  old key in the model) first, and the resolver returns that list. Every
  piece is part of the entry (`containsEntry`): the highlight shows them
  all (`profile_shape` draws every piece), and a window over a piece adds
  nothing. A click on one piece takes out just that piece (mitcad#410,
  `removeEntry`): the entry is replaced, in its place, by the other
  pieces under their own current keys, the main one first. The pieces are
  separate regions in the view, so a click works on the region clicked,
  as everywhere else: taking the whole reference out would also unselect
  pieces that were not clicked, and a second click on the same piece
  would then add back only that piece instead of giving back the
  selection before. The old key cannot stay for the rest: it
  would still mean every piece. Before, toggling compared only the main
  region, so a click on another piece added it instead (the model
  deduplicated it), and the piece could not be clicked off. A definition
  that lists a piece after its split reference keeps the reference only
  (`uniqueProfiles`).
- An empty selection is the usual missing-selection hint ("Select
  profiles.") with OK refused.

Tests: `app.unit` (`selectionEntryTests`), `tools/ui-profile-toggle-test.sh`,
`tools/ui-profile-split-test.sh` (split pieces, Docked and Floating),
part 14 of `tools/ui-windows-workflow-test.ps1`.

### Sketch selection filters

While a sketch is edited, picks go through the sketch's three-state
filter (mitcad#217, `framework/SelectionScope.hpp`, `MainWindowSelection.cpp`)
instead of the SOLID tab's filters (`m_filterActions`), which stay as they
are and decide again once sketch mode ends. Each row cycles empty (cannot
be selected) -> square (a click) -> cross (a click and a window, either
direction: inside, or crossing). The rows and their defaults, which every
sketch edit starts with (`SelectionScope::sketch`):

| Row | Default |
| --- | --- |
| Sketch geometry (the edited sketch's ordinary curves and points; centre lines too, they bound profiles) | cross |
| Construction geometry | cross |
| Projected geometry (reference entities) | cross |
| Dimensions and constraints (sketch mode's overlay) | square |
| Profiles | square |
| Visible body edges | square |
| Occluded body edges | empty |
| Body faces, body vertices, bodies, components, planes/axes/points | empty |

- Another sketch's geometry has no row and is never selectable
  (`filterRowOf`). A window collects every kind the filter lets a window
  take, not one preferred kind; dimensions and constraints come from the
  overlay (`SketchController::annotationsIn`) when their row is a cross.
- Visible or occluded: for a body edge, the view looks from the picked
  point of the edge (a click: the edge's point nearest where it hit; a
  window: its samples inside the window) towards the eye, along the view
  in an orthographic camera, and asks whether a face of a shown body lies
  in between (`OcctViewer::occluded`, `IntCurvesFace_ShapeIntersector` per
  body, made once; 0.01 mm tolerance). An edge partly hidden is one item:
  a window takes it when the part it holds is allowed. Bodies that are
  hidden are not shown, see-through ones (previews, ghosts) hide nothing.
  The camera of the moment decides, so an orbit changes the answer.
- Shift: a click with Shift also picks what empty rows hold (the view's
  kinds widen while Shift is down, `setPickScope`'s `expanded`); faces win
  over whole bodies as outside sketch mode. Shift pressed or released
  picks again under the pointer, so the hover shows what the click would
  take (`OcctViewer::eventFilter`, an application event filter for the
  key). Ctrl adds and removes; Ctrl+Shift does both; Shift with a window
  adds it as before without widening it. Other buttons and the wheel keep
  Shift's navigation. A face, a plane or a body is not picked where an
  edge the filter offers is within the edge's pick width, also one behind
  it (with Shift the sketch's own plane lies in front of the bodies). A
  right click opens the context menu for what a Shift+click would take,
  so a body's face in sketch mode still offers Project.
- The control: the SKETCH tab's SELECT group has a Selection Filter
  button whose menu holds `SelectionFilterPanel`: the symbols' legend, a
  button per row (its action `command_select.filter.<row>`, the symbol as
  its icon, the state in its tooltip and accessible description) and
  Reset to Defaults (`command_select.filter.reset`).
- Project (`commandSelectionScope("sketch.project")`, `CommandHost::commandScope`)
  has its own context while it runs: visible edges cross, occluded edges
  empty, faces and vertices square; the control shows and changes it, and
  Shift widens a click to occluded edges. The input's own kinds (edges,
  faces, vertices) always hold. On OK and on Cancel the sketch's rows are
  as they were before. Other commands' inputs decide with their own
  filters as before.

Logs: `Selection filter sketch defaults: <row>=<state>, ...` (entering a
sketch), `Selection filter <scope> <row> <state>: ...` (a row clicked),
`... reset: ...`, `Selection filter project defaults: ...`, `Selection
filter sketch restored: ...`, `Selection filter widened (Shift)` and
`Selection filter as set`; with `MITCAD_LOG_PICKS` also `Sketch hover:
<item>|nothing`, and with `MITCAD_LOG_EDGE_PLACES` once the camera rests `Sketch pick places:` then
`Sketch pick edge F1.b0/<edge> visible|occluded at x,y`. Tests: `app.unit`
(`selectionScopeTests`), `tools/ui-sketch-select-test.sh`.

## Icons

24 x 24 SVG, drawn for Mitcad (MIT, SPDX line first): dark outline
`#2b3a4a` 1.3 px with round joins, solids light blue `#9cc0ec` (tops
`#c9ddf6`, sides `#7aa6dc`), the action in orange `#e07b1a`, sketch
geometry blue `#1f5fc0`, construction planes `#f3c48a`. Rendered with Qt
SVG at several sizes (`themeIcon`).

## Notes on particular commands

- **Helix** (`solid.helix`, `SweepCommands.cpp`): the model's `helix`
  holds only `pitch` and `revolutions`, so a height goes in as a ratio:
  `revolutions` "<height> / <pitch>" or `pitch` "<height> /
  <revolutions>", each operand in parentheses unless it is a name, a
  number with its unit or a call (as the FreeCAD import writes it).
  `load` reads the type back from that form; a ratio in another form
  opens as Revolution and Pitch with the expression as is. Growth goes in
  as `growth` unless it is a plain zero; a new helix is of Mitcad's
  construction, and an edited one keeps its `construction` (FreeCAD's for
  the FreeCAD import's growing helices, mitcad#83). A fixed axis (the
  importer's) is not editable.
- **Sweep** (`solid.sweep`, `SweepCommands.cpp`): Type is Single Path,
  Path + Guide Rail (`guide_rail`, Profile Scaling) or Path + Guide
  Surface (mitcad#157): faces picked in Guide Surface go into
  `guide_surfaces`, the path must lie on them, and Orientation, Taper
  and Twist are hidden. A definition with guide surfaces and a twist or
  taper (which the model ignores) is not editable.
- **Hole**: Counterdrill is `kind` `counterdrill` with `cd_diameter`,
  `cd_depth`, `cd_angle` (the cone's full angle); Taper Angle is `taper`
  (the wall's angle to the axis), left out when 0 and hidden for tapped
  holes, which the model refuses to taper. Thread and Hole take sizes
  from the model's `thread_sizes`.
- **Press Pull** on one face of a fillet opens the fillet for the radius
  of the set that rounds its edge (`Built::editing`).
- **Combine** (`solid.combine`, `MoveCommands.cpp`): a tool body picked
  in another occurrence than the target (a body of another component, in
  an assembly context) goes in with a link, `tool_links` (`source` the
  tool's occurrence path, `target` the target's when it has one;
  commands.md, *combine*, mitcad#104, mitcad#105): the tool is read where
  it was picked and follows when either occurrence moves. `load` gives
  the picks back their occurrences from the links. Test:
  `tools/ui-component-combine-test.sh`.
- **Patterns and Mirror** of features take fillets and chamfers among the
  features (the timeline or a face they made picks them, as other
  features); the model repeats them on the copies' edges (commands.md,
  *Fillets and chamfers among the features*, mitcad#105).
- **Extrude** chooses its operation until the user picks one (mitcad#289,
  `CommandDef::autoOperation`, `framework/CommandSessionAuto.cpp`): each
  preview asks the model for it (`auto_operation`, commands.md,
  *Extrude's automatic operation*), which tries the extrusion's tool
  against the visible bodies of its component: Join with the bodies it
  touches, Cut with those it goes into (a negative distance or Flip from
  a profile on a face), New Body when it touches none. The answer sets
  Operation and Objects (logged `Extrude: automatic Operation = Cut`) and
  their own preview follows, so the view never shows the old operation on
  the new inputs. A change of Operation or Objects by the user (the
  panel, a pick, live MCP) turns it off for the rest of the command;
  editing an Extrude never changes its stored operation. Tests:
  `tools/ui-auto-operation-test.sh`, `app.ui-windows-auto-operation`.

## Known limitations

- Sketch mode works in the component's own coordinates, so a placed
  component's sketch is edited where the component's origin is; the
  browser's Create Sketch refuses planes of placed components. A
  component placed twice shows its sketches and datums once.
- A sketch's Show Profile/Points/Dimensions/Constraints options are the
  palette's (`SketchController::setShow…`), not saved.
- Offset Face after Delete Face on the same body fails in the model ("the
  offset produced no solid").
- Bodies the section cuts away cannot be picked while Section Analysis is
  open (the clipped shape is drawn instead).
- Named views keep no visual style; Insert Mesh does not convert meshes
  to solids.
- File drops are not UI-tested (xdotool cannot drag files).

## Live MCP

An agent connected through `mitcad-cli mcp --connect` ([docs/mcp.md](../docs/mcp.md),
mitcad#265) runs these commands by their ids with `ui_command`
(`MainWindowMcpUi.cpp`): a command with a panel opens as from the menu, its
inputs are given by their keys through the panel's own actions
(`CommandSession::scriptInputs`: what typing, choosing and picking do), its
preview runs, then OK, Cancel or the panel stays open for the user; an
action runs as its shortcut would. `edit` opens a feature as a
double-click in the timeline does. The availability rules are the user's
(`isAvailable`, read-only windows); commands that open a file dialog or
another window (`file.*`, `insert.*`, `tools.*`, `help.*`, 3D Print, DXF
import and export, Parameters, the render environment and grid settings)
are refused, and any other modal dialog is closed as cancelled
(`mcp::ModalGuard`). Their steps are labelled "(agent)" in the undo text,
and while an agent is connected the project indicator says "Agent
connected", its menu offering Disconnect Agent.

Save As and Open have tools of their own instead (mitcad#402):
`document_save` with a `path` and `document_open`, into and from the
folders the user allows agents only (`mcp/AgentFolders`: Preferences ›
Agents, the setting `mcp/folders`, or `--mcp-folder DIR` for a run; none by
default), without dialogs; Open replaces the document as File › Open does
and refuses unsaved changes unless the agent discards them. `ui_camera`
sets the view's camera or frames a box, the shown bodies or the selection
at once (`OcctViewer::frameBox`, mitcad#405). A `view.*` command's reply,
`ui_select`'s reveal and `ui_camera` end the view's animation and a pending
fit before they answer (`OcctViewer::settleCamera`): an animation of
0.4 s otherwise left the camera where it started in the reply and in the
next screenshot.

## Logs that UI tests read

| Line | When |
|---|---|
| `View area x y w h` | the 3D view's place in the window (`ui_view_click`) |
| `Ribbon SOLID/CREATE at x,y`, `Ribbon tab BUILD at x,y` | group menus and the workspace tabs (wherever the tab bar is: the ribbon's, the title bar's), when the ribbon or its tab bar is shown, moves or is resized (`PlaceWatcher`, when the places changed), and when another tab is shown (`ui_click_logged`) |
| `Window layout: window 1280x800, minimum 484x315; menu 6,0 1268x22; central 252,126 716x593; QStatusBar 6,771 1268x29; browserDock 6,126 240x593; commandDock 974,126 300x593; ribbonToolbar 6,22 1268x104; timelineDock 6,725 1268x46` | the main window's size and minimum size and the geometry of its menu bar, central widget (the 3D view) and visible docks, toolbars and status bar (by object name, else class), once the window shows (not with the native UI); `tools/ui-layout.sh` prints it and `tools/check-all.sh` reruns every UI test when it changes (mitcad#412) |
| `Ribbon split SKETCH/CREATE/Circles at x,y`, `Ribbon split SKETCH/CREATE/Circles arrow at x,y` | a subgroup's split button: the part that starts its default, the arrow that opens the submenu (mitcad#222; logged with the group menus) |
| `Ribbon menu SKETCH/CREATE: Line \| Rectangles > [2-Point Rectangle \| …] \| … \| Sketch Dimension`, `Ribbon menu SKETCH/CREATE/Circles: Center Diameter Circle \| …` | a group menu or a subgroup's submenu as it opens (`-` a separator, a submenu as `Name > [its entries]`) |
| `Panel <command> input <id> at x,y [(hidden)]`, `Panel <command> OK at x,y` | a panel opened |
| `Datum xy at x,y`, `Sketch entity F1/c4 at x,y` | an input that takes them became active |
| `Command <name> started`, `Editing F2 with <name>`, `Editing analysis Section1 with <name>`, `Command <name> cancelled` | |
| `<command> <input>: 2 edges [edge E{…} of F2.b0; …]` | a selection input changed |
| `<command> <input>: d1*2 = 120 mm`, `… is invalid: <reason>` | a value was evaluated |
| `Extrude: automatic Operation = Join` (with `Extrude Local bodies: 1 body [body F2.b0]`), `Extrude: operation chosen by hand` | a new Extrude's operation and objects the model chose from its preview, and the user's own choice of either, which stays (mitcad#289) |
| `<command>: <input> = <choice>` / `= on` | a choice or check changed |
| `Preview <command>: ok`, `Preview <command>: failed: <reason>`, `Preview <command>: not built: <reason>` (the inputs make no definition), `<command>: OK refused` | |
| `Added <name> as F5`, `Edited F2`, the command's `describe` line | OK |
| `Press Pull opened Fillet1 for its radius` | a command's inputs asked for another feature's edit, which opens next |
| `Body Body1 (F2.b0): volume a -> b mm3`, `New body …`, `Removed body …` | the bodies OK, Undo or Redo changed, measured by the model (with `MITCAD_LOG_VOLUMES=1`, which the tests set: the first change after opening a large design would measure every body) |
| `Cosmetic threads shown: 1` | the cosmetic threads drawn as rings changed |
| `Sketches shown: F1, …` | the sketches the view shows |
| `Timing refresh: 36.5 ms`, `Timing display: 1093 ms, 166 bodies, 166 new`, `Timing hover: …`, `Timing frame: …`, `Timing open: …` | with `MITCAD_LOG_TIMING=1` (`framework/Diagnostics.hpp`): how long the steps take, for `tools/perf-measure.sh` |
| `Selected: 1 face [face F2:end(…) of F2.b0]`, `Context menu: A \| B` | outside commands |
| `Eye click Root/Bodies/Body1 (press)` (`(double-click)`) | a click on a light bulb in the browser, before its `Visibility …` line; the press, its moves and its release belong to the light bulb and never reach the tree's selection (mitcad#344; `tools/ui-browser-test.sh`, `tools/ui-windows-browser-test.ps1`) |
| `Context menu Hide: Hide Body \| Hide Component` (`Hide 2 Bodies`, `Hide 2 Components`; the submenu of the view's context menu, mitcad#347), `Hid body Body2`, `Hid component Component1:1` | Hide > Hide Body / Hide Component: `set_body_visible` for the bodies of the selected faces, edges, vertices and bodies, `set_occurrence_visible` for their occurrences (none for the root's bodies, so no Hide Component), each once, as the browser's light bulbs do (`BrowserController::hideBodies`, `hideOccurrences`), one undo step; `tools/ui-hide-menu-test.sh` |
| `Find in Design opened`, `Find in Design found: 5 bodies, 0 components`, `Find in Design: hid 3 bodies` (`showed`, `1 component`; `selected …`, `appearance for N bodies`), `Find in Design button at x,y`, `Find in Design field\|components\|bodies\|visibility\|show\|hide\|appearance\|select\|all\|none\|close at x,y`, `Find in Design row N at x,y` | the Find in Design window (mitcad#350): the results after each search, what each button did (bodies by uid and components by occurrence, each once), and where the controls and the rows in sight are; the field is also logged as `Dialog field Find in Design: <text>`; `tools/ui-find-in-design-test.sh` |
| `Create Sketch: select a plane`, `Create Sketch: origin planes shown` (`not shown (the design has components)`), `Create Sketch picked …`, `Sketch started on xy, origin (0, 0, 0)`, `Editing sketch F1 (Sketch1)`, `Sketch finished` | sketch mode |
| `Sketch view x0,y0 x1,y1 x2,y2` | where sketch points (0, 0), (100, 0), (0, 100) are in the window, when it changed (`ui_sketch_click x y`) |
| `Sketch origin at 0.00, 0.00, 20.00 mm in the design; the design origin at x 0.00, y 0.00 mm in the sketch` | a sketch edit started: where its origin lies in the design and where the design origin lies on its plane, in the document's length unit (mitcad#254) |
| `Sketch pointer x 30.00, y -12.50 mm` | in sketch mode, the pointer's place in sketch coordinates that the view shows above the status badge (where a tool snaps it), once the pointer rests at a new place (mitcad#254) |
| `Selection filter sketch defaults: sketch-geometry=cross, …`, `Selection filter sketch visible-edges cross: …`, `… reset: …`, `Selection filter project defaults: …`, `Selection filter sketch restored: …`, `Selection filter button at x,y`, `Selection filter row visible-edges at x,y`, `Selection filter reset at x,y`, `Selection filter widened (Shift)`, `Selection filter as set` | sketch selection filters (mitcad#217): the rows when a sketch edit or Project starts, a row clicked, the sketch's rows back after Project, where the control and its rows are (the rows when its menu opens), Shift down and up |
| `Sketch hover: edge E{…} of F1.b0` / `nothing`, `Sketch pick places:` then `Sketch pick edge F1.b0/E{…} visible at x,y` (`occluded`) | with `MITCAD_LOG_PICKS`: what a click would take under the pointer in sketch mode and Project; with `MITCAD_LOG_EDGE_PLACES` (`tools/ui-sketch-select-test.sh`; every edge is tried) where a click hits each body edge in sight and behind faces once the camera rests in sketch mode |
| `Sketch Sketch1: 2 DOF`, `… 0 DOF, fully constrained`, `…, conflicts k3+k5` | the sketch's status changed |
| `Sketch entities recoloured for a dark background: 8 fully constrained, 1 centre lines` (`light`) | the view's background turned dark or light while sketch entities are shown: fully constrained geometry and centre lines take the background's colours (mitcad#253), their halos too (mitcad#302) |
| `Sketch halos: 5 dark` (`light`) | the halos shown changed in number or tone: fully constrained curves and centre lines are drawn over a wider line in the background's tone (`OcctViewer::styleSketchHalo`), dark on a dark background and light on a light one, so that they stay readable over a body's face; points, construction and projected geometry and unconstrained curves have none (mitcad#302) |
| `Sketch colours: F1 8 fully constrained, 0 unconstrained; F3 in O1 4 fully constrained, 3 unconstrained` (`none`) | the shown sketches changed how they are coloured, in sketch mode and outside it: per sketch (with the occurrence that places a component's sketch) the entities drawn in the fully constrained colour of the background and those in the unconstrained blue; construction, centre lines and projected geometry are not counted (mitcad#286) |
| `Sketch tool Line`, `Added line c2 from (0, 0) to (40, 0) (horizontal)`, `Line chain closed`, `Added rectangle 40 x 25 mm at (0, 0) [c1, …]`, `Added circle, diameter 20 mm at (x, y) [c5]`, `Added arc …`, `Added polygon (6 sides) …`, `Added slot, width 12 mm …`, `Added ellipse …`, `Added spline c9 through 3 points`, `Added point p4 at …`, `Added text t7 "…" at …` (or `in a frame from (x, y) to (x, y)`), `Trimmed c3 at (x, y)`, `Extended c1 at (x, y)` (with ` (joined p5)` when the end joined an existing point, mitcad#259) | the tools |
| `Sketch cursor: bitmap 32x32, hot spot 10,10, ratio 1, icon line`, `Sketch cursor: arrow` | the view's pointer when a sketch tool starts or ends, or the screen's ratio changes (logical pixels) |
| `Sketch value length: 40 = 40 mm`, `Sketch value editor at x,y with 40.00` (`with d1 = 40` for a dimension's, its name first), `Sketch value editor tooltip: d1 = 40.00 mm (Sketch1 width)`, `Sketch value editor: inserted d2, now d2 * 2`, `Sketch value editor kept open with d2 = 30` | typed values, a dimension's value editor, a dimension clicked while it is open, a refused name (mitcad#231) |
| `Sketch plane geometry: 4 edge(s), 4 vertex(es)` (`Sketch plane geometry not read: <reason>`), `Sketch Dimension picked the body edge E{…} of F2.b0 (body:3)`, `Projected the body vertex V{…} of F2.b0: p8`, `Reused the projection of the body edge E{…} of F2.b0: c9`, `Point p9 coincident with p8, the projected corner V{…}`, `Point p9 at the midpoint of c7, the projected edge E{…}`, `Sketch Sketch2: 1 linked projection(s)` | body edges and vertices in the sketch plane (mitcad#256, mitcad#258): what the tools can pick and snap to, read for a tool session or after a change; a body pick; what a dimension or a point projected or reused; the sketch's linked projections, when their number changes |
| `Added dimension k5 length d1 = 40 mm`, `Added driven dimension k7 length`, `Editing dimension k5 (40 mm)`, `Dimension k5 = 50 mm (d1)`, `Moved dimension k5 to (x, y)`, `Dimension k5 text at x,y` (with each `Sketch view`), `Dimension tooltip k5: d1 = 40.00 mm (Sketch1 width)`, `Dimension text name`, `Dimension texts: k5 d1, k6 d2`, `Palette paletteDimensionText at x,y`, `Panel Extrude label: Distance (d3)` | dimensions; their tooltip, the palette's Dimension text option and what each shows, a feature panel's field with its parameter's name (mitcad#231) |
| `Signed distances on` (`off`), `Added dimension k5 horizontal_distance signed d3 = r * cos(a)`, `Dimension k5 shows fx: +25.98`, `Dimension k6 unsigned: fx: 25.98 (-(r * sin(a)))` (`signed: …`), `Dimension texts: k5 fx: +25.98, k6 fx: +15.00` (as a sketch with dimensions opens), `Palette paletteSignedDistances at x,y` | signed horizontal and vertical distances (mitcad#409): the palette option, a signed one added and the text it shows, a switch from the context menu with the text and expression after it |
| `Panel Extrude tooltip distance: d5 = 2*d1 → 80 mm (Extrude2 distance)`, `Parameter labels for Extrude: Sketch1 k5 d1, Sketch1 k7 d3, Extrude1 distance d4` (or `none`), `Parameter label Sketch1 k5 d1 at x,y`, `Parameter label Extrude1 distance d4 at x,y`, `Parameter label Fillet1 sets.0.radius d5 at x,y` (a list row's value), `Parameter label picked: d1 (Sketch1 k5)`, `Panel Extrude input distance: inserted d1, now 2*d1`, `Parameter labels for Extrude hidden`, `Native inline editor sketchValueEditor name: d3 =` (macOS) | a feature panel's value field whose parameter the feature owns: its tooltip; while a value field has the keyboard, the labels of the shown sketches' dimensions and of the earlier features' values (`framework/ParameterPicker.hpp`), where each is, the one clicked and the name it put into the field, the labels gone a moment after the field lost the keyboard; the dimension's name beside the native inline editor (mitcad#250) |
| `Added constraint perpendicular k6 on c1, c2`, `Constraint perpendicular refused: <reason>`, `Fixed …`, `Construction on: [...]`, `Merged points p5 into p3 (coincident on p3, p5)` (two curve ends made one, mitcad#260), `Converted to sketch geometry: c9, p7; kept projected: p8` (mitcad#257), `Deleted c4, k2` | constraints and modify commands |
| `MCP live instance: <id> 127.0.0.1: <port>`, `Agent connected`, `Agent disconnected`, `Agent disconnected by the user`, `Project indicator: …, agent connected`, `MCP: an agent ran sketch.add_line in sketch F4`, `MCP: an agent ran the UI command sketch.finish`, `MCP: an agent selected 2 sketch curves`, `MCP: an agent saved <path>`, `MCP: screenshot of the view, 640 x 480, 12345 bytes`, `Agent change: Add Line (agent)`, `MCP refused: <reason>`, `MCP: closed the dialog <title> (an agent cannot answer it)` | live MCP (mitcad#265): the listener, connected agents, what an agent ran, refusals |
| `Palette warning 0 at x,y` (each warning shown as a link), `Palette: warning 0 selects 8 entities: p1, p2, c3, …`, `Sketch view to the selection: pan` (or `zoom`; nothing when they were in sight) | the Sketch Palette's warnings (mitcad#264): where to click them, what a click selects (the `Selected: …` line follows), how the view moved |
| `Palette Auto constraint: Link to first ( ), Link to last (x); Equal (x), Horizontal/Vertical alignment ( ), Concentric ( )` (or `hidden`), `Palette paletteAutoLast at x,y` (`paletteAutoFirst`, `paletteAuto_equal`, …, the section `paletteAutoConstraint`), `Auto constraint link: first`, `Auto constraint equal: on (checked: equal)`, `Auto constraint target c2`, `Auto constraint reference highlighted: c2` / `highlight off`, `Auto constraint equal k7 on c2, c4` (the reference first; the alignment as `horizontal_points`/`vertical_points` of the curves' centres), `Auto constraint equal on c2, c4 refused: <reason>`, `Auto constraint dialog: Equal: <reason>`, `Auto constraint accepted: c4 kept without them` / `cancelled: c4 taken back`, `Auto constraint entities: c2, c4` | the palette's Auto constraint section (mitcad#189): its state when it changes, its places (in the docked layout the palette scrolls, and the section is scrolled into view when it shows; every `Palette … at x,y` line is logged again when the palette scrolls, mitcad#412), the choices, the reference the preview targets, the constraints each new entity gets, a refused one's dialog and answer, the tool session's entities |
| `Dragged p5 to (x, y)`, `Dragged c7 to (x, y)`, `Drag of p5 did not move it` | a drag ended: where the point is, or the curve's point nearest the cursor |
| `Double-click on t26: sketch.edit_text`, `Edited text t26: "MIT", Droid Sans, center`, `Edited pattern k8: 4 in all`, `Edited offset k13: 3 mm` | the edit panels of texts, patterns and offsets |
| `Equal spacing k4 horizontal: p1, c2, c5 (spacing k6 = 12.00)` (`… along c9`; without the spacing when it is free), `Equal spacing k4 removed`, `Equal spacing preview: p1, c5, c2` (the panel's objects in its order), `Sketch Dimension on 3 objects: Equal Spacing Dimension`, `Double-click on k4: sketch.edit_equal_spacing`, `Equal spacing of 3 object(s), horizontal, spacing 12 mm`, `Edited equal spacing k4 of 4 object(s), vertical` | equal spacings (mitcad#221): each chain in the order the model keeps, when it changes (also during a preview), the panels |
| `Timeline Sketch1 at x,y`, `Timeline marker at x,y`, `Timeline button back at x,y` (start, back, forward, end) | the timeline's items, when they changed, also when the timeline moves in the window (a banner or another panel makes room) |
| `Timeline search opened` / `closed`, `Timeline search field at x,y`, `Timeline button search at x,y` (`search-previous`, `search-next`), `Timeline search "fillet in:cover": 2 matches: Fillet2 (Cover), Fillet7 (Cover)` (`no matches`, `unknown filter is:x`), `Timeline search result Fillet2 (Cover) at x,y`, `Timeline search results hidden`, `Timeline search match 1 of 2: Fillet2 (Cover)`, `Timeline search cleared` / `cleared: selected Plane1 again, scrolled to 0`, `Selection restored after the timeline search: 1 item(s)` | the timeline search (mitcad#179): its field, the matches (when they change), the listed ones in sight, a step, the end and what it restored |
| `Timeline colours: dark, marker #c8d0da, buttons #f0f0f0, window #1f2024, handle #5aa2ff` (`light, marker #2b3a4a, buttons the style's, …`), `Asking the platform for the dark colour scheme` | the timeline's palette colours at start and when the theme changes (the marker's handle: the Docked window's accent colour, orange in the Floating chrome); the switch of `MITCAD_TEST_COLOR_SCHEME` |
| `Window style: dark, window #141518, panels #1f2024, accent #5aa2ff, font Segoe UI Variable Text` (`light, …`; `font the platform's`), `Window frame: Windows 11 caption colour, Mica, rounded corners` (`skipped (Windows build 19045)`, `not applied (DWM error 0x…)`) | the Docked window's style at start and when the theme changes (mitcad#251, `framework/WindowStyle`); on Windows what the window frame got, when the window shows and at each change of theme |
| `Feature faces highlighted: F3 1, F4 2` / `none` | the faces of the features selected on the timeline (uid and number of faces), when they change |
| `Timeline group Group1 at x,y` (its band, or its folded cell), `Timeline group Group1 collapsed` / `expanded`, `Timeline selected: F1, F2, F4` (a Shift+click run, or what the browser's rows selected; `nothing`), `Grouped Sketch1, Extrude1 as Group1`, `Renamed group Group1 to Body`, `Ungrouped Body` | timeline groups |
| `Timeline tooltip: Fillet2: <error>`, `Marker at 2 of 4`, `Moved Plane1 to 0 in the timeline`, `Move of Fillet1 refused: <reason>`, `Suppressed Fillet1`, `Renamed Plane1 to Datum`, `Delete Sketch1: also Extrude1, Fillet1?`, `Deleted Sketch1, Extrude1, Fillet1`, `Delete Sketch1 cancelled` | timeline actions |
| `Browser width 240` (`, scrolls sideways` with a horizontal scroll bar), `Browser Root/Bodies/Body1 at x,y` (its name), `Browser eye <path> at x,y`, `Browser radio <path> at x,y`, `Browser linked Root/Sketches/Sketch1/Extrude2 to Component1` (a feature row whose result is in another component) | the browser's width and the rows in sight (names from the root; a row whose middle is in the view), when they changed, also when the tree scrolls or the panel moves in the window; a place logged before whose row has left sight (scrolled away, folded, gone) is logged once more as `Browser <path> out of sight` (also `Browser eye <path>`, `Browser radio <path>`), so that a test never clicks a place that is not valid; `tools/ui-test-lib.sh` scrolls such a row back into sight before it clicks it (`ui_browser_reveal`) |
| `Browser background: rows` (`card`), `Preferences: browser background rows`, `Preferences browser background at x,y`, `Browser view beside the rows at x,y` (`… out of sight`) | the Browser's background in the Floating layout (mitcad#419): at start and on each change, the choice saved in Preferences, its drop-down in the dialog, and with Rows a place right of every row's box where a click reaches the 3D view (`tools/ui-browser-rows-test.sh`) |
| `Browser scroll range 240`, `Browser scroll bar at x,y` (`… out of sight`) | the rows' vertical scroll range (0 while every row fits; mitcad#425: the Floating Browser card grows to the timeline before it scrolls) and the vertical scroll bar in sight, the tree's or, with Rows, the one at the card's left (`BrowserPanel::setScrollBarOnLeft`), with the rows' places (`tools/ui-browser-rows-test.sh`, `app.ui-windows-browser-rows`) |
| `Browser selected: <paths>`, `Visibility <path>: hidden`, `Visibility <path>: hidden (during Extrude)`, `Command Extrude: 1 visibility change(s) kept`, `Renaming <path>`, `Renamed Body1 to Plate`, `Renamed body F3.b0 with its component to Bracket` (mitcad#291), `Activated Bracket`, `Isolated …`, `Copied …`, `Pasted Bracket:1 as Bracket:2`, `Placed Bracket:2 at (90, 0, 0)`, `Paste of Bracket:3 cancelled`, `Ground Bracket:2`, `Deleted Bracket:2`, `New component in Root`, `Placement warning: <text>` (mitcad#292), `Look at plane F4`, `View top`, `Units: in`, `Found … in the browser` | browser actions |
| `Bodies shown: F2.b0, F3.b0 in O1`, `Body F3.b0 in O1 at x,y` | the shown bodies (with their occurrences) and their middles in the window |
| `Failed features: Fillet2: <error>` / `none`, `Failures button at x,y`, `Revealed Fillet2` | the status bar's summary |
| `Feature warnings: Fillet1: <warning>` / `none`, `Preview Fillet: ok, warning: <warning>` | features that succeeded with warnings, when they change; a preview's warning |
| `Warnings badge: Gear: <message>; Helix: <message>` / `none`, `Warnings button at x,y` | the yellow badge's features (with warnings or an import note, mitcad#170), when they change |
| `Browser sketch DOF: Sketch1 0, Sketch2 4` | the sketches' degrees of freedom the browser shows, when they change |
| `Browser joints: Joint1 placed, AsBuiltJoint1 satisfied, DOF Root 1` | the joints' states and each component's degrees of freedom (mitcad#55), when they change |
| `Added joint Joint1 (revolute): placed, rz 30 deg, moved O2; DOF 7` (a failure: `failed: <error>`), `Added as-built joint …`, `Added joint origin JointOrigin1: origin (5, 5, 10), normal (0, 0, 1)`, `Added rigid group RigidGroup1: Pin:1, Cap:1`, `Drove joint Joint1 (revolute): placed, rz 45 deg, …`, `Added constraint Constraint1: insert placed, moved O2; DOF 1` (each row's type, `(does not hold)` after one that does not), `Preview Joint moves O2` | the ASSEMBLE group's commands (mitcad#55, mitcad#148; the joint as the `joints` query has it after OK) and a preview that places occurrences elsewhere |
| `Drag of Pin:1 started at (x, y, z)`, `Dragged Pin:1 to (x, y, z): Joint1 rz 63.2 deg`, `Drag of Pin:1 cancelled`, `Animating Joint1 rz: 36 frames from 45 deg to 45 deg`, `Animated Joint1: 36 frames`, `Animation of Joint1 stopped: <why>` | dragging a component the joints move (points in its parent's coordinates) and Animate Joint |
| `Parameters favorites: width` / `none`, `Parameters width favorite at x,y`, `Parameter width favorite: on = 30 mm` | favourites |
| `Parameters dialog opened`, `Parameters: d1 = width + 5 mm (35 mm); …`, `Parameters d1 expression at x,y` (name, unit, expression, comment), `Parameters add/delete/OK at x,y`, `Added parameter width = 30 mm`, `Parameter d1 expression: width + 5 mm = 35 mm`, `Parameter width expression: d2 / 2 refused: <reason>`, `Delete parameter w refused: <reason>` | Change Parameters (places in the main window's coordinates) |
| `Panel Fillet input sets.1.radius at x,y`, `Panel Fillet input add sets at x,y`, `… input remove sets.1 at x,y`, `… input up_sections.2 at x,y`, `… input down_sections.0 at x,y` | a list's rows and buttons, an ordered selection's buttons |
| `Panel Profile button_part at x,y` (a frontend without tiles), `Panel Profile tile part: <name>`; `Part picker place search at x,y; in the picker x,y` (also `star`, `pin`), `Part picker group row Profiles at …`, `Part picker tile <library>/<component> at …` | the button beside a choice (`withButton`, mitcad#418); the part picker's places, in the main window and in the picker ([Part picker](#part-picker)) |
| `Panel Extrude width 284 of 284` (after its places), `Panel Extrude message 284x36, panel width 284 of 284` (`message hidden`; `284x96 scrolling`) | the panel's width and its page's (mitcad#198): with its places, and once laid out after its message or a note under a field changed (`tools/ui-panel-message-test.sh`) |
| `Pick places:` then `Pick face F2.b0/F2:end(…) at x,y`, `Pick edge …`, `Pick vertex …`, `Pick profile F1/r{…} at x,y` | an input became active (with `MITCAD_LOG_PICKS=1`, which the tests set): where a click picks each item it takes (`ui_click_pick`) |
| `View ghosts: O2 (1 body)`, `View ghosts: none` | the occurrences a command draws as ghosts changed (`withGhosts`, mitcad#147); logged before the pick places of the input that picks through them |
| `View inactive ghosts: 2 bodies (active Bracket:1)`, `View inactive ghosts: none`, `Pick priority: active Bracket:1` | the bodies drawn as ghosts outside the active component changed (mitcad#420; `none`: the root active or the setting off); a click took the active component's item before a nearer one of another component |
| `View body highlights: F1.b0, F2.b0 in O1`, `View body highlights: none` | the bodies highlighted whole changed (a selection input of bodies, such as 3D Print's, mitcad#234) |
| `Hover chain: 3 edge(s) of F2.b0` (`face(s)`), `Tangent chains of F2.b0 (edges): 2 in 3 ms` | the hover of an input with `withHoverChain` over an edge or face changed how many it shows (the tangent chain with its check on, else 1); a body's chains asked the first time (mitcad#290) |
| `Manipulator <command> <input> at x,y`, `Manipulator <command> <input> dragged to <value>` | a handle's knob, a drag ended |
| `Manipulator Circular Pattern suppressed.2 at x,y` (`… (off) at` when suppressed), `Manipulator Circular Pattern suppressed.2 toggled`, `Circular Pattern Suppressed: 2` | a pattern's instance dots |
| `Inspect <command>: <log>`, `Closed <command>` | an inspection's result (Measure: `area 2400, distance 45, angle 90 deg`, of a circle or arc `length 31.416, radius 5, diameter 10`; Interference: `Body1 x Body2: <volume>` or `none`; Section Analysis: `area …, length …`) |
| `Kept Section Analysis as Section1`, `Edited analysis Section1`, `Deleted analysis Section1`, `Section analysis at {"plane":{"normal":[1,0,0],"origin":[10,0,0]}}`, `Section analysis off` | Section Analysis kept in the document (mitcad#41); the plane the bodies are cut at changed |
| `Physical properties: Body1 volume … mass … center (…)` | the Properties dialog opened |
| `Done <command>` | a model command panel's OK |
| `Camera direction -0.577 0.577 -0.577 up … (orthographic)` | the camera came to rest after it moved (direction from the eye to the model) |
| `Layout grid 5 mm, major 25 mm, 18.2 px apart` | the layout grid's spacing changed, logged when the camera rests |
| `Orientation cube places:` then `Orientation cube front at x,y`, `Orientation cube front-top at …`, `Orientation cube front-right-top at …` | with `MITCAD_LOG_ORIENTATION_CUBE=1`, before each camera line: where a click picks each face, edge and corner of the cube in sight |
| `Orientation cube pressed on front at x,y`, `View home at x,y`, `View home`, `View top`, `Look at …` | the cube, the house, standard views |
| `Orientation cube arrow up at x,y` (up, down, left, right, cw, ccw), `Orientation cube arrow cw`, `Orientation cube arrows hidden` | the cube's arrows: where they are when the view comes to rest face on, a click |
| `Visual style Shaded with Hidden Edges`, `Camera perspective`, `Background #525761 #1f2126`, `Layout grid off`, `Snap to grid on`, `Grid spacing 10 mm` | display settings |
| `Saved named view NamedView1`, `View NamedView1`, `Deleted named view …` | named views |
| `Rendered view on`/`off`, `No render worker (mitcad-render) next to the application: no rendered view`, `Render worker started (pid <n>): <path of mitcad-render>`, `Render frame memory 1: 910 x 735` (debug; the frame memory's id and capacity), `Render worker ready: Cycles 5.3.0 on <CPU>`, `Render scene 1: 2 bodies, 1 meshes sent (F4.b0), 1 meshed, 0 released`, `Render scene 1 applied in 2.1 ms: received F4.b0; added -; changed F4.b0; moved -; removed -; kept 1; meshes 3; instanced 0`, `Render view 3: orthographic 728 x 588` (debug; a view sent to the worker, once a navigation of the view ends, mitcad#214), `Render view 3: first frame (145 x 117) after 26 ms`, `Render view 3: first denoised frame (145 x 117, 1 samples) after 60 ms`, `Render frame view 3 728x588 samples 4 denoised covers x0,y0 x1,y1` (debug; what the frame's alpha covers, in view pixels; `denoised` for a denoised frame), `Render image dropped: camera moved` (debug; `size changed`, `animation`: the view draws the bodies shaded from this frame, once per navigation, mitcad#216), `Render view 3: 64 samples in 2.91 s`, `Render mesh F2.b0: 12 of 12 triangles turned to face outwards` (debug; a triangulation wound inwards, mitcad#319), `Render material swap F2.b0: default (transmission 0, opacity 1) -> glass_clear (transmission 1, opacity 1); mesh reused (the worker holds it)` (debug; `meshed again`, `sent again`), `Render brightness view 3 samples 16: 0.412 over 5321 pixels` (debug; the mean linear brightness of what the frame covers, `tools/ui-render-glass-test.sh`), `Render worker: render-ground off: the camera looks at the ground from below` (debug; `render-ground on: the camera is above the ground again`: the ground dropped and built again as the camera crosses its plane, mitcad#320, `tools/ui-render-ground-test.sh`), `Render worker stopped: <reason>`, `View message: <text>`, `Render worker ended` | View > Rendered (`tools/ui-render-test.sh`) |
| `Preferences opened: General, 640 x 330` (the page shown and the size the window asks for), `Preferences: navigation middle-orbit, zoom to cursor on, …, autosave every 5 min` (or `…, autosave off`), `Shortcut overview: 23 command shortcuts` | the dialogs |
| `Section caps: 2 face(s) at {"normal":[1,0,0],"origin":[30,…]}`, `Section caps: none` | the caps of Section Analysis or Hide Above Sketch drawn (when they change), and gone again (the bodies whole) |
| `Section: F2.b0 shown whole: section: clip: SIGSEGV 'segmentation violation' detected. …` (warning) | a body whose clip failed, shown uncut (mitcad#392) |
| `Section caps: 3 colour(s) [#e0892a hatch #4b2d0b at 135 deg, 12 lines; …] in Shaded, bodies in appearance colours` | the viewer's caps (mitcad#382): per colour group the cap colour (the component's timeline colour, the default orange for the root), the hatch's colour, direction and line count; again when the visual style or Component Colors change |
| `Hide Above Sketch on`/`off`, `In front of the sketch plane: F2.b0 5.0, F5.b0 10.0` | the sketch palette's option; in sketch mode (also while a sketch command's panel is open), how far each shown body reaches in front of the sketch plane (mm, when it changes; 0 or less once cut, a body cut away entirely is left out) |
| `Exported <path>: dxf flat pattern of Body1, 5 entities (CUT_OUTER 4, BEND_DOWN 1), mm`, `Export of the flat pattern of Body1 refused: <reasons>`, `Flat pattern of Body1 shown` (`folded`), `Flat pattern of Body1: valid, area 1086.39 mm2, 1 bend(s)`, `Deleted the flat pattern of Body1`, `Added sheet face of 1 profile(s), new rules, thickness 2`, `Added flange, length 15 (tangent), angle 90 deg`, `Sheet rules: thickness …` | sheet metal (mitcad#210): the Flat Pattern's export, display and item, the commands' describe lines |
| `Export dialog: STEP AP214 (*.step) to <path>`, `Exported <path>: step, 1 body(ies), mm` (with occurrences `…, mm, design coordinates` or `component coordinates`), `Exported <path>: usdz, 1 body(ies), 1 material(s)`, `Exported <path>: dxf, 4 entities` | export |
| `3D Print panel: 3 visible: Plate, Cube, Pin; format stl, refinement high, slicer Fake slicer (/path <files>); slicers None: … \| …` (`1 selected: Cube`), `3D Print Bodies: 2 bodies [body F1.b0; body F2.b0]` (the input, as every selection input logs), `3D Print: Format = 3MF, one object with parts`, `3D Print: left out Surface1 (a surface body)`, `3D Print: no solid or mesh bodies`, `3D Print: 3 bodies as stl (high) to <folder>: Plate.stl, Cube.stl, Pin.stl` (a body shown twice: `Pin (Pin_1).stl, Pin (Pin_2).stl`; 3MF: `parts.3mf`… per body `Plate 12 triangles, 1000.000 mm3`), `3D Print: started <program> <arguments>`, `3D Print: <slicer> did not start`, `3D Print: opened <folder>`, `Done 3D Print`, `Command 3D Print cancelled`, `Preferences: slicer <slicer>` (or `automatic`) | 3D Print (mitcad#234: the panel replaced the dialog, whose `3D Print dialog: …` and `3D Print cancelled` lines are gone) |
| `Export dialog opened: unit mm (mm \| cm \| m \| in \| ft)` (with occurrences also `, coordinates design (design \| component)`), `Insert Mesh dialog opened: unit …`, `Insert DXF dialog opened: unit …`, `Units dialog opened: mm (…)`, `Add User Parameter dialog opened: unit mm`, `Grid settings dialog opened: automatic, fixed spacing 10 mm` | a dialog that offers units opened, with its default first |
| `Panel Hole choices: placement=face, …, standard=iso_metric, size=6, designation=M6x1, class=6H`, `Panel Hole options size: 1 \| 1.1 \| …` | a panel's choices when it starts; the options of a `withChoices` input when they change |
| `Imported block.step as Base1: 1 body(ies)`, `Inserted drawing.dxf into Sketch2: 4 curves, 4 points, 0 texts, 1 profiles, 0 warnings`, `Inserted component block (linked) from block.mitcad` | import |
| `Insert DXF drawing: unit mm, layers Outline (4) \| Holes (1) off`, `Insert DXF place lower_left at x,y`, `Insert DXF x at …`, `Insert DXF layer Holes at …`, `Insert DXF OK at …`, `Insert DXF: lower left corner at (10, 5), layers Outline` | the Insert DXF dialog (places in the main window's coordinates) and what it chose |
| `Import of part.f3d started`, `Import item 3: Extrude2 parametric`, `Import of part.f3d: stop requested after 12 s`, `Import of part.f3d stopping: the remaining items take the file's bodies`, `Import of part.f3d cancelled`, `Import of part.f3d failed: …`, `Imported part.f3d: 2 bodies; items …[; stopped at Extrude3 (item 5)]`, `Import report: part.f3d: 28 items (25 parametric, …), 2 bodies, 2 of the file's 2 solids match[, stopped at Extrude3 (item 5)]`, `Import report summary: <the report's text above its list>`, `Import report kept: Pad.Length = Sheet.w * 2: <why>` | an `.f3d` or FreeCAD import (`stopped after the last item` when only the final comparison was left; `kept`: a FreeCAD expression kept as its value) |
| `References dialog: Part version 2 of Assembly v1.f3d` (`?` without a name), `References dialog: chose Part v2.f3d`, `References dialog: skipped`, `Import references of Assembly v1.f3d: 2 found, 0 skipped`, `Imported Assembly v1.f3d: documents referred to: Part linked, Holder skipped` | an `.f3d` design's documents referred to (mitcad#196): a file dialog for one not found, the choice, the documents found before the import, how each came in |
| `Recent files: a.mitcad \| b.step`, `Dropped <paths>`, `Document closed`, `File menu: New Design \| Open \| Open Recent > \| … \| - \| Project > [New Project \| … \| Project Settings] \| … \| Exit` (debug, as it opens: `-` a separator, names without underlines and dots, mitcad#175) | the File menu |
| `Remote status: synced` (`ahead N`, `behind N`, `push`, `fetch`, `sync`, `conflict`, `offline`, `sign-in needed`, `remote not found`), `Remote task <command> started\|done\|failed (<class>): <message>`, `Remote push: sent N version(s) to origin/main`, `Remote check: ahead N, behind M`, `Remote notice: <text> [Sync Now, Dismiss]`, `Sync: <case>[ (<class>)], N replayed, sent\|nothing sent`, `Sync conflict: <path> (<kind>): <choice>`, `Sync done: <text>` | the current Cloud project's remote: its state (the indicator shows it), a remote task, a push, a check, the notice, a sync and its conflicts |
| `Computing part.mitcad started`, `Computing part.mitcad: 57 of 196, Extrude6` (when the feature changes, at most 5 a second, after the first 50 ms), `Progress dialog shown: part.mitcad`, `Computing part.mitcad: cancel requested after <ms> ms`, `Computing part.mitcad cancelled after <ms> ms`, `Computing part.mitcad done in <ms> ms (<n> evaluated)`, `Opening part.mitcad cancelled` | a job on the model's worker thread (opening a design, an imported design, New, Close) |
| `Computing set_parameter started` … `Computing set_parameter done in <ms> ms (<n> evaluated)` (a model command's job is named by its `cmd` and logged only when it takes over 50 ms; `Computing preview …` for a panel's preview), `Command set_parameter cancelled`, `Command undo refused: the model is busy computing`, `Preview Extrude: cancelled`, `Extrude: OK cancelled`, `Extrude: OK not done`, `Parameter d1 expression: 70 cancelled`, `Finish Sketch cancelled: Sketch1 stays open`, `Command Extrude: the timeline stays rolled back`, `Drag of p3 ended by a dialog`, `Manipulator Extrude distance dragged to 25 (ended by a dialog)`, `Context menu dropped: the model is computing` | commands and previews as jobs: a cancel, a call that came while a job ran (refused, or waiting until it is done), a drag the progress dialog ended |
| `Display meshes of part.mitcad: 2 bodies in <ms> ms on the model's thread` (also `of <cmd>`, `of preview`; only when it meshed any) | the view's meshes of the shown, previewed and tool bodies made at the end of the job (`framework/DisplayMeshes`, mitcad#398; the job's stage reads "Meshing the bodies for display") |
| `Autosave every 300 s to <folder>`, `Autosave off`, `Autosaved block.mitcad: 2865 bytes to <folder>`, `Autosave removed for block.mitcad`, `Autosave skipped: busy` (or `previewing`), `Autosave of block.mitcad failed: <reason>` | autosave: the timer set at start and by Preferences, a write, the session's files removed, a write put off (`MITCAD_TEST_AUTOSAVE_SECONDS` for tests; `tools/ui-autosave-test.sh`) |
| `Recovery: none`, `Recovery: 2 document(s) found`, `Recoverable block.mitcad: /path/block.mitcad, autosaved 2026-10-05T03:12:00Z, 2865 bytes` (`never saved`; `, the file has changed since`, `, the file is gone`, `, damaged: <why>`), `Recovery: removed the lock of session <id>`, `Recovery: later, 1 document(s) kept`, `Discarded recovery of block.mitcad`, `Recovered block.mitcad from autosave of 2026-10-05T03:12:00Z`, `Recovered session <id> removed`, `Recovering block.mitcad cancelled`, `Recovery of block.mitcad failed: <reason>`, `Save conflict: <path> changed since it was opened`, `Save conflict: overwrite` (`save as`, `cancelled`) | recovery: what was found, each session, the dialog's choices, the earlier session's files removed once this session wrote the document, and Save over a recovered document's changed file |
| `Result store: restored 4, evaluated 2; stored 0 (0 bytes) in 0.1 ms`, `Result store: <error>`, `Result store: removed 12 old results (310.5 MB), 4800.0 MB kept`, `Result store cleared: 4 files, 0.2 MB`, `Import stored 17 results`, `Preferences: results on disk on, at most 5120 MB; in memory at most 7726 MB` | the result store: an open's job (restored from the store, evaluated, written) and a Save's, a write that failed, the clean-up beyond the size, Preferences' Clear, the `.f3d` import's worker, the Cache settings saved |
| `Diagnostics: memory 307830 of 8101298176 bytes (11 results), disk 11238 bytes (4 files), process 355545088 bytes; last recompute 5 evaluated, 0 from the store, 1 cached`, `Diagnostics: cleared memory: 7 results, 120000 bytes`, `Diagnostics: cleared disk: 4 files, 11238 bytes`, `Diagnostics report written to <path>` | Help › Diagnostics: shown or refreshed, Clear Memory, Clear Disk, Export Report |
| `Version recorded: part.mitcad abc1234 v3: <summary>`, `Version unchanged: part.mitcad (v3)`, `Version not recorded: <why>`, `Version failed: <error>`, `Version warning: <warning>`, `Version status: bracket, main, v3` (`…, no version`; `none` outside projects with history; when it changes), `Version author: Name <email> (git)` (or `(settings)`), `Version author dialog: git has none` (an older project without an author), `Version author dialog cancelled`, `Save with Description dialog: <automatic message's summary>`, `Save with Description cancelled`, `Save with Description: no version history for part.mitcad`, `Save conflict: <path> changed outside Mitcad (the file)` (or `(a newer version)`), `Save conflict: compare: <the diff's lines joined by " \| ">`, `Save conflict: new version` (`save as`, `cancelled`), `Renamed from a.mitcad: its display state moved along` | saving versions: a commit and what it did, the indicator's version (`Version status`), the author, Save with Description, a change outside Mitcad before Save, a rename followed at opening |
| `Version History dialog: part.mitcad, reading its versions`, `Version History: 3 versions of part.mitcad: v3 abc1234 <summary> \| v2 … (renamed from a.mitcad)` (at most 30), `Version History changes: v3 <changes> \| v2 … \| v1 first version`, `Version History selected v2 (abc1234): preview` (or `no preview`), `Version History compare v2 with v1: <heading and text joined by " \| ">` (`with the open design`), `Version History geometry v2 with v1: computing`, `…: <text>`, `…: cancelled`, `Version History open v2 (abc1234)`, `Version History restore v2 (abc1234)`, `Version History save copy v2 (abc1234)`, `Version History closed`, `Version History failed: <error>`, `Version History: no version history for part.mitcad`, `Restore dialog: v2 (abc1234) of part.mitcad` (` (unsaved changes)`), `Restore cancelled`, `Restore: the unsaved changes dropped`, `Version restored: part.mitcad v2 (abc1234) as v4 (def5678)`, `Opened version v2 (abc1234) of part.mitcad as part v2`, `Save Copy As dialog: part v2.mitcad`, `Save Copy As cancelled`, `Saved a copy of part.mitcad v2 (abc1234) as <path>`, `Version preview saved: <blob's 7 digits> (256 x 160)` | Version History: the list, the changes, the selection and its comparison, the geometry, the choice; Restore, Open and Save Copy As; a version's preview |
| `Appearances dialog opened`, `Appearances for bodies: F1.b0 (custom1)` (the bodies' shared appearance; `none` without bodies), `Appearances item chrome at x,y` (the list's rows in sight), `Appearances field roughness at x,y` (also `name`, `base_color`, `emission_color`: their text fields), `Appearances new at x,y` (`delete`, `assign`, `close`), `Appearance selected: chrome`, `Created appearance custom1 (Chrome Copy) from chrome`, `Appearance custom1 roughness = 0.35` (`base_color = #d02020`; `… refused: <why>`), `Assigned appearance custom1 to F1.b0` (faces: `F1.b0 F1:top`), `Appearances assign_bodies at x,y` and `Appearances assign_bodies label: Assign to 2 bodies` (the button next to Assign, only while the selection holds faces; `Assign to 1 body`), `Appearance Red Copy assigned to 2 bodies (from faces)` (mitcad#346), `Deleted appearance custom1`; mitcad#53: `Appearances for faces: F1.b0 F1:top (paint_red)`, `Appearances field texture_path at x,y` (also `texture_width`, `texture_height`, `texture_rotation`, `texture_projection`, `texture_embed`, `texture_remove`), `Appearances face F1.b0 F1:top paint_red at x,y` (the listed face appearances), `Appearances clear_faces at x,y`, `Cleared face appearances of F1.b0 F1:top`, `Appearance checker texture embedded` (`not embedded`, `removed`, `path <path>`, `size [...]`), `Appearance lost texture missing: <why>` | Edit Appearances (`tools/ui-appearance-test.sh`, `tools/ui-appearance-bodies-test.sh`, `tools/ui-render-faces-test.sh`); places in the main window's coordinates |
| `Render body F1.b0: appearance plastic_red` (`default`; with faces of their own `…, 1 faces paint_red`), `Render textures not drawn: <appearance>: <why>, its base colour is shown` (a map of its set, mitcad#314: `<appearance>: the normal map is left out, <why>`) | the rendered view's scene: each body's appearance and its faces' (`tools/ui-render-materials-test.sh`, `tools/ui-render-faces-test.sh`) |
| `Appearances texture sources: folder, polyhaven (1 folders)`, `Appearances source at x,y` (also `search`, `category`; with a texture source `resolution`, `use`), `Appearances source: folder` (`appearances`), `Appearances filtered "brick" (All Categories): 2 shown`, `Texture search folder "brick" (all categories): 1 found` (`…: <error>`), `Appearances texture Red bricks at x,y` (the grid's tiles in sight), `Texture thumbnail Red bricks shown`, `Texture selected: Red bricks (2k)`, `Texture fetching folder red_bricks 2k`, `Texture used: folder red_bricks 2k as custom1 (3 maps; 3 images embedded, 0 shared)`, `Texture not used: <why>`, `Appearance custom1 texture: embedded, maps normal roughness, source folder red_bricks 2k`, `Appearances: texture sources on the internet allowed`, `Preferences texture folder at x,y`, `Preferences: texture folders <folders>` (`none`) | texture sources in Edit Appearances and Preferences › Textures (mitcad#315, `tools/ui-appearance-textures-test.sh`) |
| `Texture source polyhaven attribution: Powered by …`, `Texture request https://api.polyhaven.com/assets?t=textures` (every request of an online source; `Texture request refused: <why>`, `Texture request <host> failed: <why> (HTTP <status>)`), `Texture details: Red Tiles 01: 1k 292 bytes, 2k 1.2 kB`, `Poly Haven: 2 textures` (`… from the cache`; `the cached list of textures, as <error>`), `Poly Haven: red_tiles_01 2k in the cache (3 maps)` | Poly Haven as a texture source (mitcad#313, `tools/ui-appearance-polyhaven-test.sh`) |
| `Render image dialog opened`, `Render image field output.width at x,y` (every output field; also `camera`, `size`, `render`, `cancel`, `save`, `close`), `Render setting output.width = 320` (`output.size = 3840 x 2160` for a preset), `Render image camera Front` (`current view`), `Render image frame x y w h` (the image's frame in the view, main window coordinates; `Render image frame hidden`), `Render image started: 320 x 240, 8 samples, png, current view, 1 bodies (pid <n>)`, `Render image progress 4/8` (debug), `Render image preview (4 samples)` (debug), `Render image done: 320 x 240, 8 samples in 0.21 s`, `Render image cancelled`, `Render image failed: <reason>`, `Render image saved <path>` | Render Image (`tools/ui-render-image-test.sh`) |
| `Render environment studio_dark, background view, ground shadows` (`environment`; `none`, `shadows and reflections`), `Render worker: environment studio_dark, 1 lights` (debug), `Render environment dialog opened`, `Render settings field film.exposure at x,y` (every field by its section and name, also `environment.image`, `background.color`, `ground.lowest`; a check box at its box), `Render settings rendered at x,y` (`reset`, `close`, `page environment`, `page lights`), `Render settings page lights` (the page shown; the places are logged again), `Render setting environment.preset = studio_dark` (`… refused: <why>`), `Render settings reset`, `Render light field power at x,y` (`list`, `new`, `add`, `delete`, `name`, `type`, `enabled`, `camera`, `position.0`…`direction.2`, `color`, `size`, `size_y`, `shape`, `spot_angle`, `spot_blend`, `angle`, `distance`, `aim`, `from_camera`), `Render light light1 added (point)`, `Render light light1 power = 25` (`… refused: <why>`; `space = camera`, `placed at the camera`, `aimed at a face`), `Render light light1: click a face to aim at`, `Render light light1 aimed at x, y, z (normal x, y, z)`, `Render light aim: no face there`, `Render light light1 deleted`, `Render light glyph light1 at x,y` (`tools/ui-render-lights-test.sh`), the environment's ending `, 1 lights` (`, 2 lights (1 off)`), `View message: Rendering: cannot read the environment image <path>: <why>; the studio lights the scene instead.` | the render settings: an environment sent to the worker, Render Environment's places (main window coordinates) and changes, an image the worker cannot read (`tools/ui-render-environment-test.sh`) |
| `Send Feedback: the form shows`, `Feedback screenshot 1280x800`, `Feedback screenshot image at x,y`, `Feedback screenshot image size w h`, `Report screenshot cropped to 568x402`, `Feedback form: kind bug, summary '<s>', description 42 characters, contact given, diagnostics on, screenshot 568x402`, `Send Feedback cancelled`, `Report preview: <title>`, `Report section <id>: <text, lines joined by " \| ">`, `Report section <id> at x,y` (its check box), `Report text <id> at x,y` (its text), `Report key: mitcad-<key>`, `Report section <id> included` (`left out`), `Report not sent: the preview was cancelled`, `Report sent: '<title>', sections description, contact; link 812 characters`, `Report on the clipboard: <n> characters (too long for the link)`, `Report screenshot saved: <path> 568x402`, `Open URL (test, not opened): <url>`; `Internal error (<context>): <masked message>; key mitcad-<key>`, `Crash report of this application's import-worker: SIGSEGV <file>`, `Crash reports of earlier runs: 1, the newest SIGSEGV of the app`, `Error report offered (crash): <message>; key mitcad-<key>` (`worker-crash`, `error`), `Error report no more offers at x,y`, `Error reports: no more offers`, `Error report declined`, `Error report not offered (turned off): <message>`, `Crashing for a test (MITCAD_TEST_CRASH=app)`; on standard error of a crashed process `Mitcad crashed: SIGSEGV; crash report: <file>` | feedback and error reports (`tools/ui-report-test.sh`); places in the main window's coordinates |
| `Sync 12` | with `MITCAD_TEST_SYNC=1` (the Linux and Windows UI tests): the answer to the Pause key (`ui_sync`, `Ui-Sync`), once the input before it is handled, at least 20 ms after it and when no watched timer runs (`framework/TestSync.hpp`) |
| `Project: local Robot arm at <root>` (`cloud … at <root> (origin <url>)`, `no versions … at <root> (inside <repository>)`, `local … at <root> (remotes first, second, none followed)`, `none`), `Project indicator: Robot arm, local, v3` (`…, cloud, synced` / `ahead 2` / `behind 1` / `offline` / `remote not found` / `sign-in needed` / `conflict` / `not synced` / a running task `fetch`, `push`, `sync`; `…, cloud, no remote chosen`: several remotes and none followed; `not in a project`; `…, no versions (inside <repository>)`), `Project indicator at x,y`, `Project indicator menu: Sync Now \| Check for Newer Versions \| …`, `Project indicator menu chose file.sync` | the current project (mitcad#89): when it changes; the indicator's text, its place and its menu (entries as shown) |
| `New Project dialog: <folder offered>`, `New Project dialog: folder <folder>: <kind>[ (inside <repository>)]` (the kinds of `inspect_folder`), `New Project dialog: storage cloud` (`local`), `New Project: creating <folder> (local)` (`(cloud <url>)`), `New Project: created <root>: version abc1234 on main[, sent \| , not sent (<class>)]`, `New Project failed (<class>): <message>`, `New Project: Open It <folder>`, `New Project: Open It Instead <url> into <folder>`, `New Project: Create as Local for Now`, `New Project cancelled`, `New project <root>` | New Project (also with the folder fixed by Open Project, and as Move to a Project's new project) |
| `Cloud check started: <url>`, `Cloud check <url>: <what was found>` (`<class>: <message>` for a failure), `Cloud check again: <url>`, `Cloud section: open the new repository page <url>` (`SSH keys page`), `Copied the public key <path>`, `Trust This Server dialog: host: SHA256:… (verified \| mismatch \| not published)`, `Trust This Server cancelled`, `Trusted the keys of <host> in <known_hosts>`, `Trust This Server: <host> refused: <why>` | the Cloud section of New Project, Open from Cloud, Project Settings and Change Address |
| `Open from Cloud dialog: <location>`, `Open from Cloud dialog: clone, enabled` (`create`, `adopt`, `open`, `none`; `disabled: <why>`), `Open from Cloud: <action> <url> into <folder>`, `Open from Cloud: <action> done into <root>[, not sent (<class>)]`, `Open from Cloud failed (<class>): <message>`, `Open from Cloud cancelled`, `Open from Cloud: opened <root> (<action>): <designs>` | Open from Cloud |
| `Open Project: <folder>: <kind>`, `Open Project cancelled`, `Open Project: no designs in <root>`, `Open Project: parts/a.mitcad (opened last)` (`(newest)`), `Version history started in <root>: version abc1234 on main`, `Open Read-Only: <path>` | Open Project and the design it opens, Open Read-Only |
| `Projects panel: shown` (`hidden`), `Projects panel: hidden at the next start` (`shown`; the user's choice written), `Projects panel at the start: shown (no choice yet, projects configured)` (`hidden (no choice yet, no projects)`, `shown (as chosen)`, `hidden (as chosen)`), `Projects panel: 2 projects: gripper \| Shop` (when it changed), `Projects top rows: gripper \| Shop` (the keys of the rows at the top of the tree, which are the projects, in their order; when they changed), `Projects names: gripper = github.com/acme/gripper \| Shop = Shop` (the names shown, `(set)` for a display name; when they changed), `Projects panel: settings of <root> (<key> selected)` (the settings button; `no project`), `Projects panel: settings of <root> (double-click on <key>)`, `Projects panel: Project Settings of the current project <root>` (`of <root> (not the current project)`, `no Project Settings for <root>`), `Project Settings: display name <name>` (`display name cleared: <default>`, `display name refused: <why>`), `Projects tree: Shop (local, current): parts/ \| parts/plate.mitcad v3, changed, open \| base.mitcad v1, locked by Alex` (per project when it changed; states `local`, `cloud, synced` / `ahead 2` / `behind 1` / `offline` / `not synced` / `conflict` / `sign-in needed`, `cloud, no remote chosen`, `no versions`, `missing`, `unreadable`; designs `vN` or `not recorded`, `changed`, `editing`, `editing elsewhere`, `locked by <name>`, `open`; `no designs`), `Projects row <key> at x,y` (rows in sight: `Shop`, `Shop/parts/`, `Shop/parts/plate.mitcad`), `Projects button <name> at x,y`, `Projects panel selected <key>`, `Projects panel: naming a new design in <key>` (`folder`; logged once the editor is open), `Projects panel: renaming <key>`, `Projects panel: new design parts/a.mitcad in <root>` (`new folder`), `Projects panel: rename <from> to <to>`, `Projects panel: <what> cancelled`, `Projects panel: <what> refused: the name "<name>"`, `Projects panel: dragging <key>`, `Projects panel: drag <from> to <to>`, `Projects panel: drop of <key> refused[ on <key>]`, `Projects panel: open <key>`, `Projects panel: delete <key>`, `Delete dialog: <path> (<n> designs[, open][, changes no version holds])`, `Delete cancelled`, `Delete: changes no version holds: deleted all the same` (`cancelled`), `Projects panel: <command> recorded as abc1234` (`(no new version)`), `Projects panel: <command> refused (<class>): <message>`, `Projects panel: the open design follows: <from> -> <to>`, `Projects panel: closed the deleted design <path>`, `Projects panel: sent the versions of <root>` (`the versions of <root> wait (<class>): <message>`), `Projects panel: show in folder <folder>`, `Projects panel: added <root>`, `Projects panel: removed <root> from the list`, `New Design: in <root>[, folder <folder>]`, `New Design: no project, an untitled design` (then `New document`), `New Design: made <path>`, `New Design cancelled` | the Projects panel (mitcad#145, `tools/ui-projects-test.sh`); places in the main window's coordinates |
| `Project Settings dialog: <name>, local; edit locks on, idle 10 min, poll 10 s; live updates none; author Name <email>; send at once on; check <choice>; live here project` (`cloud <url>`), `Project Settings: storage cloud (share)`, `Project Settings: authors to publish: …`, `Project Settings: share to <url>`, `Project Settings: shared <name> to <url>: versions sent` (`nothing sent`; ` (<class>)`), `Project Settings: share failed (<class>): <message>`, `Project Settings: stop syncing asked` (`cancelled`), `Project Settings: stopped syncing <root> (was <url>)`, `Change Address dialog: <url>`, `Change Address refused: <why>`, `Change Address cancelled`, `Project Settings: address changed to <url>`, `Project Settings: several remotes, none followed: first, second`, `Project Settings: following <name> (<url>), <name>/main` (`follow <name> failed: <message>`), `Project Settings: shared settings recorded: abc1234` (`no change`; `… refused: <why>`), `Project Settings: here: send at once on, check <choice>, live project`, `Project Settings: author Name <email>`, `Project Settings: Sync Now`, `Project Settings closed` | Project Settings |
| `Edit lock: taking part.mitcad[ over]`, `Edit lock taken: part.mitcad[ from Alex (unchanged \| no_receipt \| unanswered \| take_over)]`, `Edit lock granted: part.mitcad by Alex`, `Edit lock held: part.mitcad by Alex[, another session] (since <time>)`, `Edit lock not confirmed (<class>): part.mitcad: <message>`, `Edit locks: part.mitcad: <message>` (a remote that refuses lock refs), `Edit lock: none for part.mitcad (<why>)`, `Edit lock: part.mitcad opened read-only, without an edit lock`, `Edit lock state: editing, part.mitcad` (`editing, idle`, `read-only`, `not confirmed`, `taking`, `none`), `Read-only: on (<reason>)`, `Read-only: off`, `Read-only: refused <command or what>`, `Read-only: unsaved changes of part.mitcad asked about`, `Lock banner: <text> [Request Edit Access..., Edit, Save as Copy..., Save as New Version]`, `Lock banner <button> at x,y`, `Lock banner: <button>` (clicked), `Lock banner: hidden` | the open design's edit lock (mitcad#89, `tools/ui-locks-test.sh`): taken, held by someone else, not confirmed, the window's read-only mode and its banner |
| `Edit lock dialog: <text> [Continue Read-Only, Request Edit Access...]` (`[Take Over, Open Read-Only]`, `[Take the Edit Lock, Not Now]`), `Edit lock dialog: <button>`, `Request Edit Access dialog: part.mitcad, Alex`, `Request Edit Access cancelled`, `Edit lock request sent: part.mitcad to Alex[: "<message>"]`, `Edit lock request seen by Alex`, `Edit lock request declined by Alex[: <message>]`, `Edit lock request: Alex keeps the edit lock until <time>`, `Edit lock request: Alex did not answer (no_receipt): taking the lock`, `Edit lock of part.mitcad is free: taking it`, `Edit lock of part.mitcad released by its holder`, `Edit lock request refreshed: part.mitcad` (a poll kept the waiting request alive) | opening a design someone else holds, Take Over, a holder's offline will (live updates), the requester's side of a request |
| `Edit lock receipt: part.mitcad for Sam`, `Edit lock request: Sam asks to edit part.mitcad[: "<message>"] [Release, Keep 15 More Minutes, Decline...]` (`Release (Save First), Release Without Saving` with unsaved changes), `Edit lock request: also waiting Kim`, `Edit lock request: Release[ Without Saving]` (`Keep 15 More Minutes`, `Decline`), `Decline dialog: Sam`, `Edit lock request of Sam declined[: <message>]`, `Edit lock request of Sam: kept until <time>`, `Edit lock request withdrawn: part.mitcad`, `Edit lock: handing part.mitcad over to Sam (released \| idle \| no answer)`, `Edit lock handed over: part.mitcad to Sam`, `Edit lock handed over: the unsaved changes of part.mitcad stay`, `Edit lock: Sam asks while part.mitcad is idle: handing it over`, `Edit lock request of Sam not answered within 10 min: handing over` | the holder's side of a request |
| `Edit lock idle: part.mitcad after 10 min without activity (no unsaved changes \| unsaved changes)`, `Edit lock idle: saving part.mitcad`, `Edit lock idle: part.mitcad kept, marked idle (autosave off \| waits)`, `Edit lock idle: part.mitcad kept, marked idle (sync: <class>)`, `Edit lock: part.mitcad active again`, `Edit lock refreshed: part.mitcad (idle \| active)`, `Edit lock released: part.mitcad`, `Edit lock request withdrawn: <path>`, `Edit lock release failed (<class>): <message>`, `Edit lock stays: part.mitcad (<class>)`, `Edit lock: the sync of part.mitcad needs a choice before the release: asked`, `Edit lock: Resolve Sync` (`Release Without Sending`, `Keep Lock`), `Edit lock lost: part.mitcad to Alex (<reason> \| removed)`, `Edit lock lost: the unsaved changes of part.mitcad stay`, `Edit locks off: part.mitcad`, `Edit locks off: released <paths>`, `Edit lock: the project stops syncing`, `Edit lock: part.mitcad saved as copy.mitcad`, `Save as New Version: <path> (read-only)` | holding, idle time, releasing, losing a lock |
| `Edit lock poll failed (<class>): <message>`, `Edit locks: the remote answers again`, `Edit lock: a stale request removed: part.mitcad by Sam` (a request its Mitcad no longer refreshed, removed by this poll), `Edit lock: a newer version on the remote: checking part.mitcad`, `Updated to Alex's version saved at 14:05.`, `Edit lock: bringing part.mitcad up to date (sync)`, `Edit lock: the remote has newer versions: checking part.mitcad`, `Edit lock: also open: <entries>`, `Edit locks: live updates connected: the remote is read every 120 s`, `Edit locks probe: the remote accepts Mitcad's lock references` (`<message> (<reason>)`), `Project Settings: edit locks accepted by the remote`, `Project Settings: edit locks: <message>`, `Edit lock details: <lines joined by " \| ">`, `Project indicator menu chose Release Edit Lock` (`Edit Lock Details`), `Sync: files someone else holds the edit lock of: part.mitcad (Alex): asked`, `Sync: Send Anyway`, `Sync: not sent (locked files)` | polls, a read-only window following the editor's versions, the lock just taken brought up to date, live updates, the remote's probe, the details, Sync's question |
| `Move to a Project dialog: <design>`, `Move to a Project cancelled`, `Move to a Project: <path> into a new project`, `Move to a Project: <design> saved in <root>`, `Moved <path> to <path>`, `Recorded <path> in the project <root>`, `Preferences: cloud: git 2.47.0 at <path>…`, `Preferences: versions by git's user, else Name <email>` (or `nobody`), `Preferences: git …, remote checks every 10 min, each version sent on, live updates on, default broker none` | Move to a Project; Preferences' Cloud page |
| `Project picker <title>: a place, reading the projects` (`a design`, `a project`), `Project picker project <key>: local, <root>` (`cloud`, `no versions`; one per project offered; the key is its name, or its folder when another listed project has the same name), `Project picker <title>: 2 projects: Shop (local) \| acme/gripper (cloud)` (once read), `Project picker row <key> at x,y` (rows in sight: `<key>`, `<key>/<folder>/`, `<key>/<design path>`), `Project picker name at x,y`, `Project picker button Another Folder at x,y` (each button), `Project picker selected <key>`, `Project picker target <key>/parts/bolt.mitcad: free` (`exists`, `invalid`; when it changes), `Project picker <title>: chose <path>` (`another folder`, `another file`, `cancelled`) | the project picker (mitcad#174; places in the main window's coordinates) |
| `Updates: Mitcad 0.0.0 for linux-x64, the AppImage <path>` (`installed in <folder>`, `announce only: <why>`), `Automatic update checks are off`, `Update checks are turned off by the administrator`, `Update check: the last was at <time>`, `Update check: manifest at <url>` (`releases at <url>`), `Update request redirected to https://<host>`, `Update manifest: Mitcad 0.1.0 of 2026-10-06 (available), 214 bytes for linux-x64`, `Update check: Mitcad 0.0.1 is up to date`, `Update notice (offer): <text> [Release Notes, Install, Skip This Version, Later]` (`(progress)`, `(message)`, `(error)`), `Update notice closed`, `Update: skipping Mitcad 0.1.0`, `Update download: <url> (<n> bytes) to <path>`, `Update verified: <path>`, `Update rejected: <reason> (deleted <path>)`, `Update staged: 0.0.0 -> 0.1.0, <program> when Mitcad has quit`, `Update: replaced <AppImage>`, `Update: started <program> (process <pid>)`, `Update installed: 0.0.0 -> 0.1.0` (`Update not installed: …`), `Update failed: <reason>`, `Preferences: update checks on, channel stable` | automatic updates ([docs/updates.md](../docs/updates.md); `tools/ui-update-test.sh`) |
| `Live: <project>: off (no broker)` (`not allowed in Preferences`, `no versions`), `Live trust question: <project> through <address> [Connect, Not Now]`, `Live trust: <address> connect` (`not now`), `Live: <project>: <address> not trusted (Not Now): not connected`, `Keychain: read <host:port>: found, user <user>` (`not found`, `unavailable (<why>)`), `Keychain: saved <host:port>` (`save … failed (<why>)`), `Keychain: removed <host:port>`, `Live: <project> subscribes through <address>, prefix <prefix>, as <user>` (`anonymous`; `(<connection>, <state>)`), `Live: <project>: cannot connect to <address>: <why>`, `Live: connection c1 (<address>) connected` (`connecting`, `offline, again in <ms> ms`, `failed (auth_failed: <why>)`, `closed`), `Live: <project> subscribed`, `Live: <project> unsubscribed from c1[, which closed]`, `Live: <project>: the remote is checked (version abc1234)` (`connected again`), `Remote check: announced by live updates`, `Remote check: no timer while live updates are connected` (`the timer again (every 10 min)`), `Live: <project> published version abc1234` (`open <path>`, `closed <path>`, `lock <path>`), `Live event: <project>: open <file id> by <name> (editing) in session <id>` (`open … closed by session <id>[ (cleared: offline)]`, `version abc1234 on main by <name>`, `lock <file id> held by <name> (active)`, `lock … released`, `request <kind> <file id> from <name>`, `session <id> online` / `offline` / `left`, `dropped <reason> (<n> so far)`; ` (retained)`; ids are their first 8 characters), `Live notice: Live updates need you to sign in to <host> [Sign In, Not Now]`, `Live notice Sign In at x,y` (`Not Now`), `Live notice: Sign In` (`not now`), `Live sign-in dialog: <address>, user <user>, keychain on` (`off`), `Live sign-in refused: <why>`, `Live sign-in: <address> as <user>[, remembered]` (`cancelled`), `Live: signed out of <host:port>`, `Live: <address> forgotten: <project> disconnected`, `Live test dialog: <address>, prefix <prefix>[, as <user>]`, `Live test: <address>: ok; steps connect ok, sign_in ok, subscribe ok, publish ok, receive ok` (`failed (<class>: <why>)`), `Live: closed[, every connection left cleanly]`, `Preferences: known brokers: <address> (trusted, user <user>), …` (`none`), `Preferences: sign out of <host:port>`, `Preferences: forget <address>` | live updates (mitcad#89, `tools/ui-live-test.sh`); places in the main window's coordinates |
| `Debug log on: <file> (--debug-log)` (`MITCAD_DEBUG_LOG`, `Preferences`), `Debug log off: <file>`, `Debug log not started: <why>`, `Debug log turned off from the project indicator`, `Preferences: debug log on` (`off`), `Project indicator: …, debug log on`, `Open Log Folder: <folder>`, `Report section log: 150 lines`, `Report section logfile: <file>`, `Report section logfile included` (`left out`), `Report debug log: <file>` | the debug log (mitcad#269): started and stopped (the file's own first and last lines too), Preferences' switch, the indicator, Help > Open Log Folder (with `MITCAD_TEST_LOG_URLS` the folder's address is logged, not opened), a report's recent log lines and log file |
| `Wait cursor on: set_parameter (depth 1)`, `Wait cursor off: set_parameter (depth 1)`, `Wait cursor at idle: none set`, `Wait cursor left on after <job>: cleared` (a warning; ` (2 override cursors)`), `Wait cursor left on for a test: test hook` | the wait cursor of a job the window waits for (mitcad#268): set, and restored once by the progress dialog or at the wait's end (N counts the waits holding one, nested waits too); the first idle check after a wait; the idle recovery of one left on; the test hook (`MITCAD_TEST_WAIT_CURSOR=1`) |
| `Style: <style> Chrome: floating (the preference)` (`(--chrome)`, `(--chrome; the preference floating)`, `(the platform's default)`), `Layout preference: floating (restart asked)` (`(in use)`: the window's layout already), `Layout restart: now` (`later (floating at the next start)`), `Restart: closing the window`, `Restart cancelled: Mitcad stayed open`, `Restart (test, not started): <program> <arguments>` (`MITCAD_TEST_NO_RESTART=1`), `Restart: <program> <arguments>`, `Restart: started <program> (process <pid>)` | the layout preference and the restart (mitcad#419; `tools/ui-chrome-layout-test.sh`, `tools/ui-windows-chrome-layout-test.ps1`) |
| `Floating cards: window 1028x749, top bar bottom 21, view top 21, floating rows bottom 172; titleCard 12,33 1004x52; ribbonGroup_SOLID/CREATE 12,93 180x67; ribbonGroup_SOLID/MODIFY 200,93 120x67; projectsDock 12,180 260x200; browserDock 12,388 260x300; timelineDock 12,688 1004x48; commandDock 716,370 300x200`, `Floating view between the cards at 196,126` | the Floating layout's visible cards (main window coordinates, also for cards that are windows), the rows' cards first (mitcad#425: the title card, the group cards with spaces in their names as `_`, the notice card), the bottom of the bar above the view (the menu bar, the header on macOS), the view's top and the lowest row's bottom (the bar's bottom without rows), when it changed (mitcad#419: no card's top above the bar's bottom; mitcad#425: on Windows and Linux the view's top is the menu bar's bottom, and no other card's top is above the rows' bottom); and the middle of the gap between the first two cards of a row, where a click reaches the view (`tools/ui-chrome-layout-test.sh`, `app.ui-windows-chrome-layout`) |
| `Export dialog: Web page (*.html) to <path>`, `Exported <path>: html, 1 body(ies), 123456 bytes` (the page's size; each texture image left out a warning, `Export: …`), `Export of <path> failed: <reason>` | Export › Web page (mitcad#423, [Web page](#web-page)) |

Test logs are switched on by environment variables
(`framework/Diagnostics.hpp`): `MITCAD_LOG_PICKS`, `MITCAD_LOG_VOLUMES`
(the UI test libraries set both), `MITCAD_LOG_TIMING`,
`MITCAD_LOG_ORIENTATION_CUBE`; `MITCAD_TEST_SYNC` answers the sync key;
`MITCAD_LOG_TYPED_TEXT` logs typed-in text fields (`File dialog Save As:
/path/part.mitcad`, `File dialog Save As: accepted`, `Dialog field
Parameters: 25 mm`, `Panel field sets.0.radius: 2`;
`framework/TypedTextLog.hpp`); `MITCAD_LOG_KEYS` logs the key presses the
application takes, with the modifiers it saw (`Key log on`, `Key press: F9
with shift (native 120)`; the Windows UI tests hold the modifiers until
then, mitcad#271).
`MITCAD_SETTINGS_DIR` keeps the settings in an INI file of a test's own
(on Windows they are otherwise in the registry).

### The debug log

Every message (`qDebug`, `qInfo`, `qWarning`, `qCritical`) goes through
one handler, the log ring (`mcp/LogRing.hpp`), which `main()` installs
for the whole run after the UI test driver's and which passes every
message on to it (and so to stderr). It keeps the newest 5000 lines for
live MCP's `ui_log` and the problem reports, and passes each line to the
debug log (`framework/DebugLog.hpp`, mitcad#269) while that writes a
file. Preferences' Diagnostics page switches the debug log on and off at
once (saved as `diagnostics/debugLog`, so also at the next start);
`--debug-log [FILE]` (`--debug-log=FILE`, or a following argument ending
in `.log`) and `MITCAD_DEBUG_LOG=1|<file>` switch it on for one run. A
line of the file: `2026-10-14 09:30:12.345 debug    [model worker]
Computing set_parameter started` (local time in ms, the level, the thread
when it is not the GUI thread, the text; further lines of a message
indented). Each file begins with a header (`Mitcad debug log`,
`Version:`, `Build:`, `System:`, `Graphics:`, `Command line:`, `Design:`,
`Switches:`, `Started: …, process <pid>, file <n>`), the first file of a
start then with the ring's earlier lines between `--- The newest <n>
lines from before the debug log started:` and `--- The debug log
starts`. A log switched on before the 3D view exists (`--debug-log`,
`MITCAD_DEBUG_LOG`, the setting at start-up) makes its file at once and
writes it when the view logs its renderer (the fact `Graphics`, set with
`OpenGL renderer: …`), so its header has `Graphics: <renderer>, OpenGL
<version>`; after 5 s without it, at the first warning or critical
message, or when stopped, the file is written with `Graphics: not known
yet (…)`, padded with spaces that the renderer overwrites if it comes
later. The files are `mitcad-YYYYMMDD-HHMMSS-<pid>.log` in
`AppLocalDataLocation/logs` (`MITCAD_DEBUG_LOG_DIR` moves the folder,
for tests), 20 MB each (`--- Continued in <next>`), at most 10 files and
200 MB in the folder, the oldest removed first; written at least once a
second and at every warning. A log may contain file paths, design names
and entity names; typed text and pick places only with
`MITCAD_LOG_TYPED_TEXT` and `MITCAD_LOG_PICKS`. The test variables keep
their environment switches (no check boxes in Preferences).

**A new delayed log line** that tests read must come from a watched timer:
`TestSync::watch(timer)`, or `TestSync::singleShot(ms, context, function)`
instead of `QTimer::singleShot`, so that `Sync <n>` waits for it (how the
sync works: [docs/development.md](../docs/development.md)).

### Test helpers

`tools/ui-test-lib.sh`:

- `ui_command "Name"` runs a command through the search.
- `ui_view_click x% y%`, `ui_view_drag`.
- `ui_click_logged "Panel Extrude input operation"`,
  `ui_double_click_logged`, `ui_drag_logged`.
- `ui_menu_choose "Entry"` (in the context menu just opened).
- `ui_create_sketch xy` (Create Sketch and the plane);
  `ui_sketch_click x y`, `ui_sketch_drag x1 y1 x2 y2`, `ui_sketch_at x y`
  (sketch millimetres).
- `ui_focus_dialog "title"`, `ui_focus_main` (Xvfb has no window
  manager). After a dialog that works on a thread before it closes (New
  Project's Create, Open from Cloud), call `ui_focus_main` once its result
  is logged: the main window focused while the modal dialog is still open
  is left without Qt's activation when the dialog closes, and its
  shortcuts (S) do not work.
- `ui_mark` and `ui_expect_new "text" [seconds]` (only lines logged since
  the mark).
- `ui_click_pick "face F2.b0/F2:end"` (clicks where the last `Pick
  places:` puts the first item whose line starts so), `ui_pick_at`
  (prints the place).
- `ui_drag_from "Manipulator Extrude distance" dx dy`.
- `ui_type_in "Panel Hole input diameter" "8"` (selects the field's text
  and types; `--` lets "-25" through).
- `ui_choose "Panel Extrude input operation" 2` (the entry two below the
  current one; -1 the one above).
- `ui_expect_volume "sed pattern with a group" volume "description"
  [tolerance]` (waits for the line, relative tolerance).
- `ui_step`, `ui_sync [window]`, `ui_wait_idle`: wait until the app has
  handled the input sent so far, or until no job runs.
- `ui_start_app` (Windows `Ui-StartApp`) starts with `--no-recovery`;
  `UI_RECOVERY=1` (`-Recovery`) lets it ask.

Tests run with their own settings (`XDG_CONFIG_HOME`), data
(`XDG_DATA_HOME`, so the recovery folder is theirs), and git
configuration (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM`), so no version
is recorded with the user's identity; a test that records versions sets
its author (`tools/ui-version-test.sh` also sets `HOME`). On Windows:
`MITCAD_SETTINGS_DIR`, `MITCAD_AUTOSAVE_DIR`.

Other tools: test models can be made with `mitcad-cli run script.json
--save model.mitcad` (`tools/ui-browser-test.sh`);
`tools/ui-image-stats.py` counts pixels of a region of an `xwd` dump
(objects, dark, grey lines, red, the section caps' yellow, changes against
another dump) for `tools/ui-view-test.sh`; `tools/ui-import-test.sh`
drives the File menu by accelerators (Alt+F, then E Export, I Import, R
Open Recent) and imports a corpus design when
`MITCAD_F3D_CORPUS` is set.

**Windows** (the build VM): `tools/ui-windows-lib.ps1` has the same
helpers in PowerShell (`Ui-Key "ctrl+z"`, `Ui-Type`, `Ui-ViewClick`,
`Ui-SketchClick`, `Ui-ClickLogged`, `Ui-ClickPick`, `Ui-TypeIn`,
`Ui-FocusDialog`, `Ui-ExpectNew`, `Ui-ExpectVolume`, `Ui-WheelLogged`,
...). They post mouse and keyboard messages to the window, read the same
log lines, and give commands without a default shortcut one in the
test's settings (`Ui-Init ... @{'sketch.create' = 'F7'}`), since popups
close at once in session 0; a panel's drop-down changes with the wheel
(`Ui-WheelLogged "Panel 3D Print input format" -1`: the next entry). `tools/ui-windows-workflow-test.ps1` (ctest
`app.ui-windows-workflow`, with `MITCAD_MESA_DIR`) is the workflow test.
