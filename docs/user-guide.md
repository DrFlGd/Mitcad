# Mitcad user guide

The window has the toolbar at the top with tabs (SOLID; SKETCH while
sketching) and groups (CREATE, MODIFY, ASSEMBLE, CONSTRUCT, INSPECT, INSERT,
SELECT; a group's name opens a menu of all its commands), the model tree
(Browser) on the left, the timeline at the bottom, the command panel on
the right and the orientation cube in the view's top right corner. **S**
opens command search: type the beginning of a command's name and press
Enter.

## Quick start

### A part from a sketch to a fillet

1. **Create Sketch** (Alt+S, or command search S) and click the XY plane; the view
   turns to the plane.
2. **R** (2-Point Rectangle): click the first corner at the origin, type
   the width `40`, Tab, the height `25`, Enter. Typed values become
   dimensions and parameters (`d1`, `d2`).
3. **Ctrl+Enter** finishes the sketch.
4. **E** (Extrude) takes the latest profile: type `20` and press Enter.
   The arrow can also be dragged in the view.
5. **F** (Fillet): click the top face (a face = all its edges), type the
   radius and press Enter.
6. Changing a dimension: **Change Parameters** (MODIFY or the Edit menu),
   double-click a value and type `50` or the expression `d2 * 2`. The model
   is recomputed and the fillet stays on the same edges.
7. Editing a feature: double-click it on the timeline (a sketch opens in
   sketch mode). An edit is one undo step (Ctrl+Z).

### Opening an .f3d design

**File › Open** (or Import) a `.f3d` or `.f3z` file. The import replays
the design's timeline as Mitcad features; a progress dialog shows which
item is being replayed. It runs to the end however long it takes, and a
crash of the geometry kernel does not close the window. The dialog can
end it early:

- **Stop and Keep What Is Imported**: the items replayed so far stay,
  and the remaining features come in as the file's bodies (base
  features; sketches and construction geometry still come in as such).
  The document is complete, only less of it is parametric. Stopping takes
  a moment.
- **Cancel Import** (or Esc): the import is discarded.

When a feature does not build on the imported bodies, the import tries it
again on the bodies stored in the file. If that would replace bodies
that were already imported correctly (they match the file's within the
import tolerance), a dialog asks once per import:

- **Keep Imported Bodies** (the default): those bodies stay parametric;
  the feature comes in as the file's bodies if it still fails.
- **Replace with Stored Bodies**: the stored bodies replace them, so that
  the feature can build on those; the features before it then no longer
  shape the bodies replaced.

The import report lists, per feature tried again, the bodies kept and
replaced.

A feature whose own definition builds, but not quite the file's result
(a rounding that removes 9 % more than the file's, say), comes in as the
file's bodies too. Once the import report is closed, the dialog **Keep
Deviating Results?** lists such features, one row each, with how far off
each one is at that place: the volume it adds or removes against the
file's, the difference in mm³ and per cent, how far its new faces lie
from the file's and how much of their area is farther than the import
tolerance, and the volume of the bodies it changed against the stored
ones. A rounding that does not build with all its edges may be listed
built without some of them (the row says how many, its tooltip which).
Selecting a row selects the faces where it deviates, so the view
shows where. For each row:

- **Use stored** (the default): the file's geometry stands in for it, as
  imported.
- **Keep parametric** (check the row, or Space): the feature stays
  parametric with its own result, and the features after it are built on
  that result.

**Keep All Parametric** and **Use Stored for All** decide every row at
once; **Apply** runs the import again with the rows checked kept, and the
dialog comes again for the features that still deviate after them. A
feature kept so carries a note on the timeline (*Deviation from the file
accepted at import: …*), and the import report lists it with its numbers.
On the command line: `mitcad-cli import-f3d --accept-deviations all` (or
timeline indices, `88,93`); the default keeps the file's geometry and the
report lists the deviations.

An import report then shows which items came in as parametric features,
which partly (for example a sketch without unsupported constraints),
which as the file's bodies and which were left out (for example assembly
relationships),
and how well the final bodies match those stored in the file. Save the
result as a `.mitcad` design. More in [.f3d import](#f3d-import).

### Import and export

- **File › Import** (also the INSERT group): STEP, IGES and BRep come in
  as a base feature, STL and OBJ as mesh bodies (Insert Mesh asks for the
  unit), DXF into a new sketch on a chosen plane (Insert DXF; in sketch
  mode into the sketch being edited), another Mitcad design as a
  component, linked or as a copy (Insert Component). FreeCAD documents
  and `.f3d` designs open as new documents with their history
  ([FreeCAD import](#freecad-import), [.f3d import](#f3d-import)), `.ipt`
  parts as new documents with their bodies ([.ipt import](#ipt-import)), IFC
  building models as new documents of their storeys and elements ([IFC
  import](#ifc-import)). The
  file dialogs list the CAD formats under one *CAD files* filter. Which
  formats Mitcad reads and writes, and which keep their design history:
  [Support for multiple CAD formats](../README.md#support-for-multiple-cad-formats).
- **File › Export:** STEP (AP214 or AP242), IGES, STL (refinement,
  binary or text), OBJ, BRep, 3MF and USDZ ([USDZ for AR and 3D
  viewers](#usdz-for-ar-and-3d-viewers)) of all or the selected bodies, and a
  sketch as DXF (also Save As DXF in the sketch's context menu). Bodies
  are written where the design shows them, also those of moved components
  (a STEP file is then an assembly with the parts placed as the
  components are). In a design with components, the Coordinates row
  writes each body in its own component's coordinates instead.
- **3D Print** (the MAKE group): sends the selected bodies, or every
  visible one, to a slicer ([3D printing](#3d-printing)).
- A file dropped on the window opens or is imported; Open opens any of
  these types as a new document. On macOS, a double-click in Finder, Open
  With, a drop on the Dock icon and `open -a Mitcad file` do the same as
  Open; the title bar shows the file's proxy icon, and the close button a
  dot while there are unsaved changes. Open Recent remembers the ten
  latest files.

## Commands and shortcuts

The shortcuts below are the defaults; they can be changed (Tools ›
Keyboard Shortcuts), and Help › Keyboard and Mouse Overview lists them
all. Commands without a shortcut are in command search (S) and in the
groups' menus. Tools › Preferences has a page for each kind of setting
(General, Navigation, Display, Cache, Cloud, Updates,
3D Print, Diagnostics, Textures); command search finds each page by its name or by what it
sets ("autosave", "slicer", "git"), as *Preferences: Cache* and so on.

| Command | Key | Action |
|---|---|---|
| Command Search | S | Find a command by name or keyword |
| New Design, Open, Close | Ctrl+N, Ctrl+O, Ctrl+W | A new design: in the current project named in the Projects panel ([Versions](#versions)), outside projects an untitled one; open a design or a file to import as a new document; close |
| New Project, Open Project, Open from Cloud, Project Settings | | File › Project: a Local or Cloud project: make, open, clone, set up ([Versions](#versions)) |
| Projects | | Show or hide the Projects panel ([Versions](#versions)) |
| Save, Save As | Ctrl+S, Ctrl+Shift+S | Save a `.mitcad` design (in a project also a version) |
| Save with Description | Ctrl+Alt+S | Save and record a version with a description you write |
| Version History | Ctrl+Shift+H | The design's versions: compare, open, restore or save a copy of one |
| Sync | Ctrl+Alt+Y | Take the remote's newer versions and send yours (a Cloud project) |
| Undo, Redo | Ctrl+Z, Ctrl+Y | Undo, redo (also a dimension change and a drag of the timeline marker) |
| Fit | F6 | Fit the model to the view |
| OK, Cancel | Enter, Esc | Accept or cancel a command; Esc also clears the selection |
| Create Sketch | Alt+S (Option+S) | A plane or a planar face (or select it first); outside a sketch |
| Finish Sketch | Ctrl+Enter | Back to the 3D view |
| Line | L | Lines one after another; drag from the last point for a tangent arc |
| 2-Point Rectangle | R | Also 3-Point and Center Rectangle |
| Center Diameter Circle | C | Also 2- and 3-Point, 2- and 3-Tangent |
| Sketch Dimension | D | Length, distance (horizontal, vertical, aligned), angle (the one on the side where the value is placed), radius, diameter; also to body edges and corners in the sketch plane |
| Trim | T | Remove the part of a curve between intersections (also Extend) |
| Offset, Project, Move/Copy | O, P, M | In a sketch: offset curve, projection (also in the context menu of a face, edge or vertex), move |
| Construction | X | Selected curves to construction geometry and back |
| Delete | Delete | Delete the selected sketch geometry, constraints and dimensions |
| Extrude | E | Profiles (also straight from sketch mode), direction, extent, taper, thin wall, Operation |
| Hole | H | Click a face for the hole positions; simple, counterbore, countersink, counterdrill, taper, thread |
| Fillet | F | Edges or faces, several radius sets |
| Press Pull | Q | A face is offset, an edge filleted, a profile extruded |
| Move/Copy | M | Bodies or components: free move with handles, along axes, rotation, point to point |
| Measure | I | Distance, angle, length, area, position; also whole bodies |
| Rename | F2 | A browser row (on the timeline from the context menu) |
| Search Timeline | Ctrl+F (Cmd+F) | Where the timeline or the browser has the keys: find features by name, type, component and status ([Timeline search](#browser-timeline-and-parameters)); also Edit › Search Timeline |

The BUILD tab's commands (frames of slot profiles) are in [BUILD
tab](#build-tab). Other commands of the SOLID tab, in menu order:

- **CREATE:** Create Sketch, Extrude, Revolve, Sweep, Loft,
  Rib, Web, Hole, Thread, Box, Cylinder, Sphere, Torus, Coil, Helix, Pipe,
  Rectangular/Circular Pattern, Pattern on Path, Pattern at Points, Mirror. Helix turns
  sketch profiles about an axis while they rise along it, as a screw
  moves (a thread's profile, a twisted ridge): give the revolutions and
  the pitch, the revolutions and the height, or the height and the pitch,
  right- or left-handed; a Growth moves the profiles that far out from
  the axis each turn (negative: in), widening the helix into a cone or a
  spiral. A hole can be counterdrilled (a counterbore
  whose floor is a cone down to the hole) and tapered (Taper Angle: the
  wall leans in as the hole goes deeper). A thread, cosmetic or modelled,
  can have *Chamfer Ends*: a cone cut at the free ends it runs out
  through (a shaft's tip, a hole's mouth), so that the first turn starts
  on the cone without a knife edge. The size comes from the thread size
  (45 degrees, just inside the minor diameter of a shaft, just outside the
  major diameter of a hole) and follows it, or is given as an angle and
  a diameter.
- **MODIFY:** Press Pull, Fillet, Chamfer, Shell, Draft, Scale, Combine,
  Offset Face, Replace Face, Split Face, Split Body, Move/Copy, Align,
  Delete Face, Physical Material, Appearance, Edit Appearances, Change
  Parameters. Edit Appearances (also Appearance... in a body's context
  menu) shows the appearance library with a preview of each: base colour,
  metalness, roughness, specular, transmission, index of refraction, coat,
  emission and opacity. New copies the selected appearance into the design,
  where its values can be changed; Assign gives the bodies it was opened
  for the selected appearance. With faces selected, **Assign to N bodies** (next to
  Assign) gives the appearance, as their body appearance, to the bodies
  that own the selected faces (and the selected bodies), each once, in one
  undo step; the faces' own appearances stay as they are. The shaded view shows an appearance's base
  colour; the rendered view uses all of its values. **Textures:** the
  selector above the list switches between the appearances and the
  texture sources: *Local Folders* are the folders added in Preferences ›
  Textures (Add, Choose... or a folder typed and OK; subfolders are
  searched too). A material there is a set of PNG or JPEG images named by
  their maps: a base colour (`_diff`, `_albedo`, `_basecolor`, `_color`),
  and optionally a normal map in the OpenGL convention (`_nor_gl`,
  `_normal`; DirectX ones, `_nor_dx`, are left out), roughness (`_rough`),
  metalness (`_metal`) and ambient occlusion (`_ao`), with the same name
  before the suffix (`oak_diff_2k.jpg`, `oak_nor_gl_2k.png`), or a folder
  of its own with files named by the suffix alone (`color.jpg`,
  `roughness.png`); a resolution in the name (`_1k`, `_2K`, `_2048`) makes
  that material come in several resolutions. The search field and the
  category (one per texture folder; Library and This Design for the
  appearances) narrow the list; the materials show as thumbnails. Choose
  a resolution (the last one chosen, 2k at first), then **Use**: the
  material becomes a new appearance of the design with all its images
  embedded in the design file (the line next to the resolution says how
  much they take), its texture 100 mm wide (as high as the image's
  proportions) unless the source knows the material's real size, and its
  base colour the image's average for the shaded view; it is selected, so
  Assign gives it to the selection. An image the design embeds already
  is kept once. The design opens on any computer without the folder.
  *Poly Haven* is an online source: the textures of
  [Poly Haven](https://polyhaven.com), all under the CC0 licence
  (`CC0-1.0`: free for any use, no attribution needed, so they may be
  embedded in designs and shared), searched through Poly Haven's public
  API. Mitcad uses the internet only when you allow it: the first time an
  online source is chosen, Mitcad asks; No asks for nothing, and
  Preferences › Textures (*Allow texture sources on the internet*)
  changes the choice later. Choosing a texture shows its resolutions with
  their download size; Use downloads only that resolution's maps (base
  colour, normal, roughness, metalness and ambient occlusion where the
  texture has them) and the texture's size is its real size (Poly Haven
  gives it in metres). The source records the texture's page and its
  licence. The list of textures, the thumbnails and the downloads are kept
  in the texture cache, so Poly Haven is asked again only now and then;
  when it cannot be reached, the browser says why and the other sources
  keep working. Mitcad is not made or endorsed by Poly Haven.
  The dialog lists a texture's maps and where it came from. With *Tangent Chain*
  on (the default), Fillet and Chamfer also take the edges that continue
  a picked edge smoothly, and Shell, Draft and Replace Face the faces
  joined to a picked face through smooth edges; the hover then shows the
  whole chain before the click, and with the option off only the edge
  or face under the pointer.
- **ASSEMBLE:** New Component, Joint, As-Built Joint, Joint Origin,
  Rigid Group, Drive Joint, Constraint, Animate Joint, Insert Component,
  Insert from Library: everything that builds an assembly. Once Joint's origin A (or
  As-Built Joint's component A) is picked, A's component turns
  see-through and lets clicks through, so that origin B can be picked
  behind it; once B is picked the preview shows A where the joint puts
  it. Clicking Origin A's button again shows A normally for another pick.
  **Constraint** holds components against each other in rows, each
  taking away only some motions: pick geometry 1 on the component that
  moves, then geometry 2 on another; the row's type follows the picks
  (Automatic: two circles an Insert, a circle and a hole's face Insert
  (Axis), a cylinder or sphere on a plane Tangent, else Align) and can be
  chosen: Align (faces flush or facing, points together, axes
  concentric, with an offset or limits), Angle, Center (between two
  faces), Tangent, Insert (a pin into a hole; Lock Rotation keeps its
  turn) and Insert (Axis) (the depth stays free). Add Constraint adds
  rows without closing the panel; the rows are solved with the joints,
  and the set is one timeline feature with its rows under it in the
  Joints folder.
- **CONSTRUCT:** Offset Plane, Plane at Angle, Tangent Plane, Midplane,
  Plane Through Two Edges / Three Points / Edge and Point, Plane Tangent
  to Face at Point, Plane Along Path, Plane Normal to Path at Point,
  Offset Plane Through Point, axes (through a cylinder, cone or torus;
  perpendicular at a point; through two planes; through two points;
  through an edge; normal to a face at a point) and points (at a
  vertex, through two edges, through three planes, at the centre of a
  circle or sphere, at an edge and a plane, along a path).
- **INSPECT:** Measure, Interference, Section Analysis, Physical
  Properties.
- **INSERT:** Insert Mesh, Insert DXF, Import.

Outside a command, selecting an edge shows its length in the selection card;
a circular edge also shows its radius and diameter. A selected face shows
its area, and a body shows its area and volume. Values update after edits
and undo. The card lists up to eight items; hover over the text for their
full geometry references. Use Measure for distances and angles between items.

**Section Analysis** (and Hide Above Sketch) draws every cut face as a cap
over the cut, with thin parallel lines at 45 degrees over it. A cap takes the
colour of the component its body belongs to, the same colour as that
component's band in the Timeline, so the parts of an assembly can be told
apart on the plane; the root component's caps are orange. The lines are a
darker shade of the cap's own colour, alternating between 45 and 135 degrees
from one component to the next, and they stay in every visual style and also
when the bodies are drawn in Component Colors, by appearance or rendered.
Small caps get fewer, closer lines, large ones are not filled with them.

**Pattern at Points** repeats Features, Bodies or Components at sketch points,
construction points or vertices. Select the source objects, then the Target
Points: every target receives one additional copy with the source orientation,
and the originals remain. The targets stay linked to their geometry, so moving
a point updates its copy. Components remain instances of the same definition.
Features replay their operations; select a hole and its finishing features
together to repeat that group rather than copy the resulting body.
For Bodies and Features, activate the component that owns the sources first;
use Components to repeat placed occurrences from other components.

The preview marks the shared **Automatic anchor** that goes to each target.
For components it is the centre of the bounding box of their origins. For a
feature group containing holes, it is the centre of the bounding box of all
hole placement points, on their starting surface; finishing features do not
change it. Other feature groups use their tools' bounding box, or the modified
bodies for a group containing only finishing features. Bodies use the bounding
box of all selected bodies. These group rules do not depend on selection order.
Under **Additional options**, change Anchor to **Selected point** to pick a
different anchor, or restore **Automatic**. A group retains its relative
positions. Repeated target references cannot be selected twice; coincident
targets and targets at the anchor report an error instead of silently omitting
a copy. Edit the timeline feature to change sources, targets or the anchor.

On macOS the keys that say Ctrl are Cmd keys (Cmd+N, Cmd+S, Cmd+Z, ...),
and Redo is Shift+Cmd+Z (elsewhere Ctrl+Y and Ctrl+Shift+Z). Fit is also
Cmd+0, Delete also takes the delete key (Backspace), Alt+S (Create Sketch) is
Option+S (⌥S in the menus), Cmd+, opens Settings
and Cmd+Q quits (both in the application menu), the Window menu has
Minimize (Cmd+M), Zoom and Bring All to Front, and About is in the
application menu.

## Using Mitcad

### View and selection

- Middle mouse button + drag: pan; Shift + middle button: orbit; wheel:
  zoom at the cursor. Tools › Preferences offers other mouse schemes:
  Alt with the left, middle or right button (orbit, pan, zoom); middle
  pans and F4 + left orbits; middle orbits, Ctrl + middle pans and
  Shift + middle zooms; right orbits and middle pans.
- **Trackpad** scheme (the default on macOS): a two-finger scroll pans,
  Alt (Option) + scroll orbits, Shift or Ctrl (Cmd) + scroll zooms;
  Alt + left drag orbits, Shift + Alt + left drag and the middle button
  pan, the right button orbits (a right click, or Control + click on a
  Mac, still opens the context menu). In every scheme a pinch zooms about
  the fingers, a two-finger rotation rolls the view (not with a
  constrained orbit) and a two-finger double tap fits everything.
  Two-finger orbit has a 1.3× motion gain; panning, zoom and mouse drags
  retain their own sensitivity.
- **Orientation cube**: a click on a face, edge or corner turns the view
  there, dragging the cube orbits, the house returns to the home view, the
  right button opens the cube's menu (Home, Orthographic, Perspective, Set
  Current View as Home, Reset Home).
- **View menu:** standard views, Look At, Fit, camera, display styles
  (Shaded, Shaded with Visible Edges Only, Shaded with Hidden Edges,
  Wireframe, Wireframe with Hidden Edges), background, Grid and Snaps, and
  named views (stored in the document). The settings are remembered.
- **Rendered view**: View > Visual Style >
  Rendered shows the bodies path traced. View > Render Environment (also
  in Visual Style and Environment) sets how: the light (Studio, White
  Studio, Dark Studio, Outdoor with the sun's height and direction, or an
  HDR image, `.hdr` or `.exr`, with its rotation and strength), what is
  behind the bodies (the view's background, a colour or the environment
  itself), the ground (shadows, reflections, its height) and the film
  (exposure, Standard, Filmic or Neutral). These settings are kept in the
  design and saved with it; each change is a step of Undo.
- **Render Image** (the MAKE group, also View › Visual Style) renders
  the current view, or a named view, to an image file in these
  settings. Choose the size (a preset such as 1920 × 1080, or width and
  height; the aspect from the view, or fixed, when the view shows the
  image's frame), the samples or a time limit, denoising, a transparent
  background and the format: PNG (8 or 16 bits per channel) and JPEG look
  as the rendered view does, OpenEXR keeps the render's linear light
  without the exposure and the view transform. Render works in the
  background while you go on working; the dialog shows the image as it
  refines, the samples and the time left. Cancel stops it; Save...
  writes the finished image. These choices are saved with the design
  too.
- **Selection:** a click selects, Ctrl or Shift adds or removes; a drag
  from left to right selects what lies inside the window, from right to
  left also what the window crosses (in a design with components, a drag
  that starts on a body drags the component or says why it does not:
  see [Components](#components)). The filters of the SELECT group limit
  what can be selected; sketch mode has its own (see [Selection in sketch
  mode](#selection-in-sketch-mode)). A selection made before a command goes into the
  command's input. What a click would pick is highlighted under the
  pointer: a face see-through in light blue (in a command, its input's
  colour) with its boundary drawn, so that its shading still shows, an
  edge or vertex in the same colour; geometry in front of it hides the
  highlight.
- The right mouse button opens the commands that fit the selection, Edit
  (of a feature) and Repeat (the last command).

### Command panels

A command's panel (on the right) shows its inputs; selections are shown
in the view in the input's colour, and the preview follows the inputs.
Distances and angles also have handles in the view. A failing preview is
shown in red, and OK does not accept it. Enter = OK, Esc = Cancel. While
editing a feature, the timeline rolls back to before it.

Value fields take an expression with units (`20`, `2 in`, `d1 * 2`); the
value or an error shows below the field. Numbers take a decimal comma or
a decimal point: `1,5 mm` is `1.5 mm` (and is shown as `1.5 mm`). A comma
directly between two digits is a decimal comma; any other comma separates
a function's arguments, and `;` always does. So `max(a, b)`, `max(1, 5)`
and `max(1; 5)` have two arguments, but `max(1,5)` is `max(1.5)`. There
are no thousands separators (`1,000` is 1, `1 000` is an error).

### Cutting across components

For a new **Extrude** with **Operation: Cut**, **Affected components** lists
the components intersected by the current tool. Names include their assembly
paths; root-owned bodies appear under the root. Uncheck a component to leave
all its bodies unchanged, or expand its row to exclude individual bodies.
Hover or focus a row to identify its geometry. The list scrolls independently
of the other command fields.

The preview follows the checked targets. Exclusions survive distance, start,
direction and profile changes, including targets temporarily outside the cut.
Newly intersected eligible targets are selected automatically. **Select none**
means no targets and disables OK; **Select all** explicitly clears exclusions.
The saved feature retains its choices when edited, reopened or undone/redone.

Repeated instances of a shared component definition and linked read-only
components are shown disabled with a reason. This cut cannot change only one
instance of a shared definition. Older cuts retain their **Local bodies** scope
when edited; opening an existing cut does not add other component targets.

### The Mac layout

On macOS the window follows the system's own apps; `--chrome=docked`
brings back the layout of the other systems.

- **Title bar row:** Undo and Redo, the workspace tabs as a segmented
  control, the document's name, the command search and, in a sketch,
  Finish Sketch. Under it the ribbon's groups as capsules; a group's name
  opens a menu of all its commands.
- **Cards:** the Browser (upper left, collapsible), the Command panel
  (upper right, only while it has something to show) and the Timeline
  (along the bottom) float over the 3D view; the Projects panel, when
  shown, is a card above the Browser. View › Browser, › Timeline and ›
  Projects show and hide them. Fit and Home place the model in the area the cards
  leave free; a click beside a card reaches the view.
- **Messages** appear for a few seconds in a capsule above the Timeline;
  update and remote notices appear under the ribbon. The remote's state is
  not shown; its commands are in File › Project.
- **Glass:** on macOS 26 and later the cards are Liquid Glass; with
  `--no-glass` or on older systems they are translucent panels.
- **Settings** (Mitcad › Settings, Cmd+,) is a window with the same pages
  as Preferences elsewhere, as panes. Changes apply at once; there is no
  OK. The background can follow the system's appearance.
- **Sheets:** questions and dialogs about the document hang from the
  window's title bar.

### Sketch mode

The SKETCH tab has the drawing tools (CREATE), modification (MODIFY:
fillet, chamfer, Trim, Extend, Offset, Move/Copy, Edit Pattern, Edit
Offset, Edit Text, construction, centre line, Convert to Sketch Geometry)
and constraints (CONSTRAINTS). The CREATE menu groups its tools:

| Entry | Tools |
| --- | --- |
| Line | the line tool (lines end to end, with tangent arcs) |
| Rectangles | 2-Point, 3-Point and Center Rectangle |
| Circles | Center Diameter, 2-Point, 3-Point, 2-Tangent and 3-Tangent Circle |
| Arcs | 3-Point, Center Point and Tangent Arc |
| Other Geometry | Ellipse, the polygons, the slots, the splines and Point |
| Text | Text |
| Patterns | Rectangular Pattern, Circular Pattern and Mirror |
| Project | Project (references from the bodies) |
| Sketch Dimension | Sketch Dimension |

A polygon (Circumscribed, Inscribed or Edge Polygon) stays regular: its
construction circles keep the sides equal and the corners evenly spaced,
so dragging a corner or turning it with Move/Copy (Rotate) turns, moves
or resizes the whole polygon without skewing it. A dimension that would
make it irregular is refused. Its centre, size and orientation stay free
until you constrain them (fix or place the centre, dimension a circle or
a side, make a side horizontal).

On the toolbar, Line and Sketch Dimension are buttons; Rectangles, Circles
and Arcs are split buttons: the button starts the default tool (2-Point
Rectangle, Center Diameter Circle, 3-Point Arc) and its arrow opens the
other variants. The keyboard shortcuts and the command search reach every
tool as before.

On the right is the sketch palette: degrees of freedom,
options (construction, snap to grid, showing dimensions, constraints and
profiles, Hide Above Sketch, Look At) and the constraint buttons.
*Hide Above Sketch* cuts the bodies at the sketch plane, so that a sketch
on a face below other parts of a body is not hidden by them (the cut
bodies cannot be picked meanwhile). The faces where it cuts the bodies are
drawn in a colour of their own and hatched, as in Section Analysis. The
bodies stay cut while a sketch command's panel is open (Offset, Mirror,
the patterns, Project, sketch Fillet, ...) and its preview is shown. It
stays on for the next sketch.

- The sketch's X and Y axes run across the view through its origin,
  which is ringed, also when the design's Origin is hidden. Above the
  status badge the view shows where the pointer is in the sketch's
  coordinates, in the document's length unit (where a tool snaps it).
  A sketch whose origin is not the design's (on a face or a construction
  plane, in a component placed elsewhere) names its origin's place in the
  design beside it and marks the design origin as seen along the sketch's
  normal. Fully constrained geometry is drawn dark, or light on a dark
  background, also in the sketches shown outside sketch editing, so a
  glance tells which of them are fully constrained. Fully constrained
  curves and centre lines have a thin outline in the background's tone
  (dark under light lines, light under dark ones), so they stay readable
  where they cross a body's face as well.
- While a tool is active the pointer is a slim cross with the tool's
  icon; a small square in its centre gap shows that the point snaps to
  geometry.
- The cursor snaps to points, centres, midpoints of lines and arcs,
  quadrants, intersections, curves, the origin and (optionally) the grid,
  and the snap adds a constraint. A line gets horizontal, vertical,
  perpendicular, parallel and tangent constraints when it is nearly so.
- **Point on a body:** the Point tool also snaps to the corners of visible
  bodies that lie in the sketch plane and to the middles of their edges
  there (lines and arcs), also of other components' bodies; the marker and
  the hint are those of a point or a midpoint, and the sketch's own
  geometry wins when both are under the pointer. Nothing is added while
  the pointer only hovers; a click projects the corner or the edge into the
  sketch (linked, as Project does it), adds the point and holds it on the
  projected corner or at the projected edge's middle, in one undo step, so
  the point follows the body when the model changes. A projection of the
  same corner or edge the sketch already has is used again; a click on a
  corner that already has a point does nothing.
- Values can be typed while drawing: fields appear next to the cursor,
  Tab moves between them, Enter accepts. Typed values (also expressions)
  become dimensions.
- **Angles:** Sketch Dimension (D) on two lines measures the angle on the
  side where the value is placed: the two lines, extended, make four
  angles at their crossing, and the one the pointer is in is measured (the
  inner corner of a shape with the pointer inside it). The arc and the
  value follow the pointer, so moving across a line switches between the
  angle and its supplement. The dimension keeps that side when its value
  changes; moving the value afterwards moves only the text.
- **Auto constraint:** once the line, circle or arc tool has drawn its
  first entity, the palette shows an *Auto constraint* section that
  constrains each next entity of the same tool to the first one it drew
  (*Link to first*) or to the one drawn just before (*Link to last*, the
  default). Circles and arcs offer *Equal* (radius), *Horizontal/Vertical
  alignment* (the centres level or plumb, whichever they are nearer to) and
  *Concentric*; lines offer *Equal* (length), *Parallel* and
  *Perpendicular*, so that Equal and Perpendicular with Link to last draw
  square corners of equal sides. The new entity is made where the checked
  constraints put it (the preview shows it, with the reference
  highlighted, also while the pointer is over the section) and gets them
  in the same undo step. The choices affect only the entities drawn next
  and last until the tool ends; a constraint the sketch refuses opens a
  dialog that lists it: *Cancel* takes the entity back, *Accept* keeps it
  without that constraint. Tangent circles and tangent arcs take no auto
  constraints.
- **Dimensions to bodies:** Sketch Dimension (D) also picks the edges of
  visible bodies that lie in the sketch plane (lines, arcs, circles,
  ellipses, splines), their corners and the centres of their circles and
  arcs, also of other components' bodies, and also while Hide Above
  Sketch cuts the bodies. They highlight under the pointer as sketch
  geometry does, which wins when both are under it. Nothing is added
  while picking; when the dimension is made, the edge or corner is
  projected into the sketch (linked, as Project does it) and the
  dimension attaches to it, in one undo step. A projection of the same
  edge or corner the sketch already has is used again. Projected geometry
  is fixed: a dimension to sketch geometry moves that geometry, one
  between two projected edges is driven. Edges that are not in the plane
  still need Project (P).
- A double-click on a dimension edits its value; a dimension's text can
  be dragged. A dimension that would over-constrain the sketch is added
  as driven (in parentheses); an over-constraining constraint is refused
  with a message. A driven dimension's value is a read-only parameter
  (`d7`, kind reference): it can be renamed and used in expressions of
  features after the sketch, and a value typed into it makes it driving.
- Dimension names: hovering a dimension shows its parameter (`d3 = d1 / 2
  → 20 mm (Sketch1 distance)`). The value editor shows the name before
  the field (`d3 =`); typing `width = 40` renames the parameter as it
  sets it, and a click on another dimension while the editor is open
  inserts that dimension's name. The palette's *Dimension text* option
  shows each dimension's value, its name, or both (`d3 = 20`). A feature
  panel's value field shows its own parameter's name after the label
  (`Distance (d5)`), its tooltip says what the parameter is as a
  dimension's does (`d5 = d1 / 2 → 20 mm (Extrude2 distance)`), and it
  also takes `name = value`. While a feature panel's value field has the
  keyboard, the view labels the dimensions of the sketches it shows and
  the values of the features before the one the panel adds or edits
  (`depth = 12 mm`, where their handles would be; a fillet's or chamfer's
  sets' values on the faces they made); a click on a label
  inserts its name at the field's cursor.
- Imported designs: a dimension the file keeps driven gets the file's own
  parameter of it as its read-only reference, so expressions that use it
  follow what the sketch measures.
- A command that starts with a selection (Offset, Fillet, Extrude, ...)
  gives the keyboard to its first value field, text selected, as soon as
  the required picks are made, so the number is typed without a click;
  picks in the view still work, and a field you clicked into keeps the
  keyboard.
- A constraint button applies to the selected geometry, or picks it
  when nothing fitting is selected. Equal, Parallel, Perpendicular,
  Collinear, Concentric, Tangent and Coincident on more than two selected
  entities constrain the first one selected with each of the others (for
  example, several circles equal to a dimensioned one); Horizontal/Vertical
  on several lines makes each one horizontal or vertical. The constraints
  are one undo step; ones the sketch refuses (already implied or
  conflicting) are left out with a message.
- Coincident between the ends of two curves makes them one point, as if
  both curves had been drawn from it, so Offset and chain selection see
  the corner joined (a fixed end is the one kept; a distance between the
  two goes). A point and a curve, a centre, a lone point or a projected
  point stay a coincident constraint.
- Extend: an end that reaches another curve's end, or any existing point,
  joins it; reaching the middle of a curve, the end stays on it. A
  projected point holds the extended end with a coincident constraint.
  The end keeps its point, its constraints and its dimensions (those of
  its length do not stay): a distance to it measures the new place.
- **Convert to Sketch Geometry** (MODIFY, and the context menu of
  projected geometry): the selected projected curves and points become
  ordinary sketch geometry, free and no longer following their source.
  A corner that projected curves you do not convert also use stays
  projected, and the converted curve stays joined to it.
- **Equal Spacing** (Constraints) keeps the intervals between three or
  more independent objects equal without copying them: points, circles
  and arcs (their centres) and lines (their midpoints), mixed as needed.
  Choose how the intervals are measured: Horizontal (x), Vertical (y),
  Along a Line (pick the line) or Point to Point (the distances, a chain
  that may bend). Only successive intervals count (A-B, B-C, C-D); nothing
  is aligned and no size is made equal (an arc spaced by its centre keeps
  its radius when the radius is dimensioned). Horizontal, vertical and along
  spacings take the objects in their order along the direction; when two
  are at the same place along it, the panel names them: order the list
  and check Keep Listed Order. A point-to-point chain follows the list's
  order (objects picked in the panel come in the order clicked; for a
  selection made beforehand check Chain in Listed Order once the list is
  right). The preview draws the chain numbered 1, 2, 3.
- Without a dimension the intervals stay equal but their size is free.
  **Common Spacing Dimension** adds one dimension that sets every
  interval; selecting three or more such objects and pressing Dimension
  (D) opens Equal Spacing Dimension directly, with the current mean
  interval to type over. Editing that one value changes every interval;
  deleting it keeps the intervals equal. A double-click on the equal
  spacing's symbol (or **Edit Equal Spacing**, also in the context menu
  of a right-clicked symbol) changes its objects, their
  order, the measurement and the dimension. Deleting one of its objects
  takes it out of the chain; with fewer than three left the constraint
  goes. Copies of a sketch pattern are spaced by the pattern: change them
  with Edit Pattern.
- Fully defined geometry is black, free geometry blue, construction
  geometry an orange dashed line, projected geometry purple. Dragging a
  point or a curve moves it as far as the constraints allow.
- Text letters are profiles (letters as regions, their openings as
  holes), and the copies of sketch patterns follow their originals; Edit
  Text, Edit Pattern and Edit Offset change them later.
- Limits: the quadrant of an angular dimension cannot be chosen, and no
  constraints can be added to the curves of an offset of an ellipse or a
  spline.

#### Selection in sketch mode

While a sketch is edited, the **Selection Filter** button of the SKETCH
tab's SELECT group says what a click and a window select. Each row cycles
when clicked: an empty square (cannot be selected), a filled square (a
click selects it) and a square with a cross (a click and a window select
it, in both drag directions). Each sketch edit starts with:

| Row | Default |
| --- | --- |
| Sketch geometry, construction geometry, projected geometry | cross |
| Dimensions and constraints, profiles, visible body edges | square |
| Occluded body edges, body faces, body vertices, bodies, components, planes/axes/points | empty |

So a window selects the sketch's ordinary, construction and projected
geometry together and nothing of the bodies, and a click on a body edge
in sight selects it, one behind faces does not. Whether an edge is in
sight depends on the view at the moment (orbit and it changes); for an
edge partly hidden, the part clicked or inside the window counts. The
rows only concern the edited sketch: other sketches' geometry cannot be
selected, and hidden bodies neither. *Reset to Defaults* restores the
rows; the SOLID tab's filters are kept apart and apply again after the
sketch.

- **Shift+click** also selects what empty rows hold (for example an edge
  behind a face); the highlight under the pointer shows it while Shift is
  down and goes back when Shift is released. Ctrl+click adds or removes,
  Ctrl+Shift+click does both. Shift with a window adds to the selection
  but does not go beyond the filter. Shift with the other mouse buttons
  and the wheel navigates as before. A right click offers the commands
  for what Shift+click would select, so a face still offers Project.
- **Project** (P) has rows of its own while it runs, shown in the same
  control: visible body edges cross, occluded edges empty, faces and
  vertices square. They can be changed while it runs, and Shift+click
  reaches an edge behind faces; it takes nothing but edges, faces and
  vertices. OK and Cancel bring back the sketch's rows as they were; the
  projected geometry then follows the projected geometry row.

### Browser, timeline and parameters

- **Browser:** Document Settings (unit), Named Views, Origin, and the root
  component and every occurrence with its bodies, sketches and
  construction geometry. The eye before a row's icon shows and hides it
  (undoable). A sketch hides by itself when a feature first uses it and
  shows again when the last such feature is deleted; the eye overrides
  that. The radio button after a component's name activates it; F2
  renames. In the view, the context menu of a face, edge,
  vertex or body has **Hide** with **Hide Body** (the bodies of everything
  selected) and **Hide Component** (the occurrences that place them, left
  out for the root's bodies); the entries count when there are several
  ("Hide 3 Bodies"), do what the eyes do and undo in one step. Context menu: Edit, Show/Hide, Isolate, New Component, Move
  Features into a Component, Copy, Paste, Paste New, Move to... (a
  component: another parent; the joints, constraints, rigid groups and
  moves that place it go with it), Ground, Create Components from Bodies,
  Move to Component (bodies and sketches), Create Sketch, Look At,
  Rename, Delete, Find in Timeline. **Collapse All**, the last entry of
  every row's menu, folds the whole tree whichever row you clicked, with
  everything below folded rows; only the top row stays. Its arrow then
  opens just that row, and each row below opens with its own arrow, so you
  reopen the tree one level at a time. It changes only what the browser
  shows: nothing in the design and no light bulb changes.
- **Timeline:** features as icons in their component's colour, a failed
  one in red with its error, one with a warning in yellow with the
  warning in its tooltip (also what an import says about a feature, see
  [Reading an imported timeline](#reading-an-imported-timeline)). A click selects what the feature made: the
  bodies, sketch or construction geometry it created, or, for a feature
  that changes a body (fillet, chamfer, hole, shell, …), the faces it
  made. Shift+click selects a run of features. A feature after the
  marker, a suppressed one, or one whose faces a later feature removed
  highlights nothing. A double-click edits. Context menu: Edit, Rename,
  Suppress, Delete (dependent features are asked about), Move to
  Component (a component made from bodies: Move the Features That Made
  It into the Component), Roll History Marker Here. Dragging the marker rolls the model back; dragging a
  feature moves it (a forbidden place shows in red with the reason).
- **Timeline filter:** the filter button after the playback buttons shows
  only some components' features: All Components, Active Component (it
  follows the radio button) or a component chosen from the list, with
  Include Subcomponents for the components placed in it. Show in
  Timeline Only on a component's browser row does the same. Joints and
  rigid groups that place an occurrence of a shown component stay. The
  other features take no room; a dotted mark stands for each run of them.
  The marker still rolls back every later feature, shown or not (its
  tooltip says how many hidden ones); the playback buttons step between
  the features shown, and a feature dragged lands just before or after a
  shown neighbour. Find in Timeline on a hidden feature shows all again.
  The filter belongs to the window and is not saved.
- **Timeline search:** the magnifier after the filter button, Ctrl+F
  (Cmd+F on a Mac) while the timeline or the browser has the keys, or
  Edit › Search Timeline opens a field on the timeline. It finds the
  features whose name or type contains every word typed ("fillet2",
  "rounded edge", "fillet", "construction plane"; a "quoted phrase" is
  one word), among those the timeline filter shows. The matches are
  outlined in orange, and listed above the field with their component's
  colour and name, since imported designs repeat names across
  components. Enter or Down goes to the next match, Shift+Enter or Up to
  the previous one (a click on a listed match to it): it is scrolled into
  sight and selected as a click selects it. Filters in the same field,
  also from its filter menu: `in:Part` (the components whose name
  contains it; `in:"Two words"`), `is:failed`, `is:warning` (warnings and
  the import's notes, as the yellow "n warnings" counts them),
  `is:suppressed`, `is:rolled-back`. Filters of one kind are
  alternatives (`is:failed is:warning`: either), different kinds all
  apply (`fillet in:Cover is:warning`). Esc closes the search; clearing
  it brings back the scroll position and the selection from before it,
  unless something else was selected meanwhile.
- **Find in Design:** the magnifier on the browser's title row (also
  Edit › Find in Design and the command search) opens a window beside the
  browser that finds components and bodies by name and acts on all the
  matches at once, which is what designs with thousands of elements need.
  The text is matched case-insensitively anywhere in a name; `*` stands
  for any characters and `?` for one (`bracket*left`, `plat?`). Components
  and Bodies choose what is listed, and "Visible only" or "Hidden only"
  narrows it. The list follows the text as you type (after a short
  pause) and the design as it changes; it shows the name, the kind and the
  place in the assembly, and a count such as "238 bodies, 12 components".
  It searches the design's names, not the browser's rows, so branches that
  are folded or never opened are found too. A component placed twice
  gives one row per place for each of its bodies; the body is one body,
  so hiding it hides it in every place. Selecting rows selects them in
  the view and the browser (Select All and Select None select the
  list); a double-click reveals the row in the browser. The buttons act
  on the selected rows, or on all when none is selected: **Show** and
  **Hide** are what the browser's eyes do, all in one undo step;
  **Appearance...** opens Edit Appearances with the bodies as the targets
  (a component's targets are its bodies); **Select** makes the results
  the selection, for every command that takes one. The debug log
  records each action with its count, such as "hid 238 bodies".
- **Change Parameters:** user and model parameters with expressions and
  comments; a refused change (a cycle, a wrong unit) stays in the cell in
  red with the reason. Renaming updates the expressions.
- The status bar's "n failed" lists the failed features and leads to
  them; beside it, the yellow "n warnings" lists the features with
  warnings, an import's fallbacks, partial items and placeholders among
  them, and leads to them the same way. (In the Mac layout both are in
  the Browser's header.)

### Components

New Component, the New Component operation of an extrude or a primitive
and Create Components from Bodies make components; Paste makes an
occurrence and Paste New a new component. Activating a component (the
radio button) directs new features into it. Ground fixes an occurrence;
Move/Copy moves components as a timeline feature. Insert Component
inserts another design linked (updated when opened) or as a copy.

**Modelled bodies in the root:** a design often starts with sketches
and features in the root, and components come later. When a component
is made in a root that has modelled bodies (New Component, Create
Components from Bodies, the New Component operation of a feature),
Mitcad asks first: **Move into a New Component** puts every modelled
body of the root, with the features, sketches, construction geometry
and parameters that made them, into one new component where it is now
(grounded, nothing moves), so that the root holds only components and
the part can be edited and reused as a component; **Continue as Is**
makes the component and leaves the root's features where they are;
**Cancel** makes nothing. For Create Components from Bodies, moving
gives each selected body's component the features that made it. "Don't
ask again for this design" is saved with the design. A feature being
added that uses what moved (an extrude of a moved sketch) goes into the
new component with it.

The same is at hand later: **Move Features into a Component** on a
component's row moves all its modelled features, **Move to Component**
on a body, a sketch or a feature in the timeline moves that one with the
features that go with it, and **Move the Features That Made It into the
Component** on a Component from Bodies step gives that component its
history and removes the step. A feature that works on bodies that would
end up in different components (a combine, a cut through both, a sketch
on a face of another body) cannot go with only one of them: Mitcad lists
such features and offers to keep those bodies together. Each move is one
undo step.

**Moving a component to another parent:** a component made inside
another one (because that one was active) can be moved out: **Move
to...** on its row in the browser lists the root and the other
components by their full paths and moves the component, with everything
in it, under the one chosen. It stays where it is in the design, also
under turned parents; its history, parameters, name and grounding stay
too, and sketches, links, joints, cut scopes, section analyses and
Isolate that refer to it through its old place follow it. Parents it
cannot go into are listed greyed with the reason: its current parent,
itself and what is placed in it, a component placed more than once (the
move would change every copy of it), a linked component. What the move
cannot keep is named instead of changed: an occurrence placed by a Move,
a captured position, a joint, a constraint or a rigid group in its
parent; a joint whose moved component would change; a link across it
while a parent on the way is moved by the timeline; Pattern at Points.
The move is one undo step; Cancel changes nothing.

**Dragging components:** a component that its joints and constraints
leave free to move can be dragged in the view with the left mouse
button; the joints and constraints hold while it moves, and the drag is one undo step. Over such a body the
pointer turns into the move cursor and the status bar says what the drag
moves, for example "Drag to move Pin:1 (Joint1: rz)". A drag on a body
that cannot be dragged moves nothing and says why in the status bar: the
component is grounded or in a rigid group with a grounded one, its joints
hold it fixed (a rigid joint), it has no joints (components move only
through their joints; Create Components from Bodies makes a component of
each body of a component), the body belongs to the root component, a
command is open, or the design is read-only. In a design with
components, a window selection starts from empty space or with Shift
held.

A sketch of the active component can lie on a face or a construction
plane of another component: Create Sketch on it where an occurrence
shows it. The sketch stays linked to that face and follows it when the
face changes or either component moves. Project in such a sketch, or in
any sketch, also takes edges, faces and vertices of other components,
linked when Projection Link is on. Features other than sketches still
work only on their own component's bodies.

### BUILD tab

The **BUILD** tab, beside SOLID, builds frames of aluminium slot profiles
with their connectors, panels and linear parts: pick a part, click where it
goes, and its length, joints, mitres and connectors follow from the clicks.
What it makes is ordinary design content: each placed part is a component
of its own with its occurrence, its features and its joint on the
timeline, editable with the SOLID tab's commands (a hole sketched on a
profile, a cut), saved in the design and reopened as it was. The parts come
from Mitcad's parts library (Tools › Libraries: add it and fetch it, or
Update in Insert from Library); each lists its supplier and part number. A
part placed once is kept in the design, so placing more of it needs no
library.

**PROFILE**

- **Profile:** the panel's picker finds a profile by its name, size,
  system, slots, supplier or part number (Search), and narrows to one
  system (System: `B8 30` is B-type slot 8 on a 30 mm grid). The first
  profile goes on a plane, a point or a face; later ones on a profile you
  click: near an end on its side it goes on the side, along that slot, at
  the click (or at an Offset from end A or B); on its end face it goes on
  the end, along that profile or turned to an axis (**Direction**: a
  corner, cut on the bisector with a Shared mitre). *Grounded at the
  origin* places a first profile without a pick. Its **length** runs to the profile in its way and stays bound to
  it: when that one moves, the length follows. The preview highlights that
  profile's face and shows the length at the free end and in a box at the
  cursor; **Tab** steps the length to the face, to the profile's centre or
  through it, and typing a length (in the box at the cursor or in the
  panel, or dragging its handle) makes it a plain number. **Mitre**: on an
  end the existing profile, the new one or both (Shared, on the bisector)
  are cut; on a side only the new one. **Rotate** turns it about its axis
  in 90 degree steps, **Flip** puts its other end first. **Place** (Enter)
  places it, and the panel stays open for the next one; **Close** (Esc)
  ends, keeping what you placed.
- **Panel:** click the inner face of a frame's profile and the panel fills
  the frame in its slots, sized to the frame (a sheet sold in fixed widths
  keeps its width and says when it does not fit); an outer face puts it on
  the face, held by clamps. A generic panel takes any thickness.
- **Linear Part:** rails (cut to length, their hole pitch kept from end A),
  carriages (a carriage on its rail slides along it: drag it, or Animate
  Joint), slot-mounted carriage mounts, bearings and motor plates.

**CONNECT**

- **Connector:** with the mouse over a profile, the places where it meets
  others are marked (L, T, cross, inline; connected ones say so; *Show
  All Junctions* marks every one). Click a profile near one, or choose it
  in the list: the panel lists the connectors that fit its systems and the
  room in the corner, each with its picture, supplier and part number,
  and greys out those that nearly fit with the reason; the chosen
  connector is previewed in
  place with its nuts and screws, and choosing another one in the list
  previews that one. **Connect** places the
  chosen one with its nuts and screws and the end machining it needs (a
  bore in the profile's end), and moves on to the next junction. Removing
  a connector removes what it added.
- **Fasteners:** a connector's nuts and screws in another size, taken off,
  or added again.

**MODIFY**

- **Length:** a part's length bound to a face (to it, to the profile's
  centre or through it) or detached as a number.
- **Ends:** an end's mitre angle and the axis it turns about; *Also the
  Part It Meets* cuts the end that meets it too. *End Machining*: a bore
  in the end (its diameter, depth and thread, for a screw or an automatic
  connector), or none; the other end's machining stays as it is.
- **Flip** and **Rotate 90 deg** for the parts you pick (Enter turns them
  again).

**OUTPUT**

- **Cut List:** the parts to cut, grouped where they are the same, with
  their quantity, length, end angles, machining, supplier, part number and
  mass, the stock lengths they come from (with the kerf, the saw cut's
  width) and the other parts. **Copy as CSV** puts the tables on the
  clipboard for a spreadsheet; **Save as PDF...** writes them with the
  design's name and the date.
- **Parts List:** every part with its quantity, supplier and part number,
  also as CSV and PDF.

The BUILD commands work through live MCP as the other panels do
([docs/mcp.md](mcp.md)); the model's commands behind them are in
commands.md, *BUILD: profile assemblies*.

### Long computations

Opening a large design, or changing it, can take a while. The window
keeps drawing, and after a moment a progress dialog shows the feature
being computed, how many there are and the time so far. **Cancel** (or
Esc) stops within a fraction of a second, also inside a long feature, and
undoes what started it: an open leaves the previous document, and a
command, parameter change, Undo or Redo leaves the design as it was (a
command's panel stays open, and OK tries again). While the progress
dialog shows, the window takes no other input.

## Terms

Each of these words has one meaning in Mitcad, its menus and this guide:

| Term | Meaning |
|---|---|
| **Design** | One `.mitcad` file: a part or an assembly |
| **Project** | A folder that is one git repository and holds designs and folders; **Local** or **Cloud** ([Versions](#versions), [Sharing a project](#sharing-a-project)) |
| **Folder** | A folder inside a project |
| **Version** | A recorded state of a design (one commit of the project's repository) |
| **Owner** | A user or an organisation on a git service: the first part of a Cloud project's address |
| **Projects panel** | The panel of projects, folders and designs; the **Browser** stays the design's feature tree |
| **Projects folder** | Where new projects go by default (Documents/Mitcad) |

## Files

### Saving and recovery

A design is saved as a `.mitcad` file. Outside projects it is a
single-file design: its imported bodies are stored inside it (in a
project they are stored in the project's folder instead; see
[Versions](#versions)). A `*` in the window title marks unsaved changes;
New, Open and closing ask first. If another program changed the file
after you opened it, Save asks before writing over it. Opening a file
that is not a valid design names the place of the error.

**Autosave** writes unsaved changes every 5 minutes to a recovery folder
in your user data, never next to your design (Tools › Preferences,
General: on or off, 1 to 60 minutes, the folder with Open Folder).
Saving, closing and ending Mitcad normally remove it. After a crash,
Mitcad offers it at the next start: **Recover Unsaved Work** lists the
documents with the time they were autosaved. **Restore** opens the
selected one as the file it came from, with its changes unsaved and
without undo history; **Discard** deletes the selected ones; **Later**
keeps them for next time or for File › Recover Documents.

### Versions

A **project** is a folder whose designs keep their versions. It is
**Local** (the versions are on this computer) or **Cloud** (they are also
in a git repository that others share; see
[Sharing a project](#sharing-a-project)). Every Save (Ctrl+S) of a design
in a project records a version, named after what you did since the last
save (`Save bracket.mitcad: Add Fillet1, Change d3`); **File › Save
with Description…** (Ctrl+Alt+S) records one with a description you
type. Saving without a change, or after changing only the view (Origin
folder shown, Isolate), records none, and autosave never does.

**File › Project › New Project…** makes one:

- **Folder:** a full path, by default Documents/Mitcad/Project1 (the first
  that is free). A folder that is a project already turns Create into
  **Open It**; designs in the folder become the project's, in its first
  version; other files stay there and are not versioned. A folder inside
  another git repository gets a repository of its own (a warning says so).
- **Storage:** Local, or Cloud with the Cloud section (below).
- **Author:** the name and email address every version records, filled in
  from git's settings, else from Preferences › Cloud. They are kept in the
  project's repository; the email address is part of every version.

**Create** makes the folder, its repository and the project's first
version. The project starts empty (or with the designs its folder had)
and shows in the **Projects panel**, ready for **New Design**. A cancel
leaves the folder as it was.

**The Projects panel** (View › Projects; on the left above the Browser,
on a Mac a card above it) lists your projects, their folders and designs:

```
github.com/acme/gripper        Cloud · ↑2
  finger                       v4 · Locked: Alex
Robot arm                      Local
  parts
    bracket                    v3 · Changed
  base                         v12
```

The panel starts shown when you have a project (made, opened or cloned
here, or in the projects folder) and hidden when you have none, until you
show or hide it yourself (View › Projects); Mitcad then remembers your
choice.

- **Which projects:** those made, opened or cloned on this computer,
  newest first, and those in the projects folder (Documents/Mitcad).
  **Add Existing Project Folder…** adds one from elsewhere; **Remove from
  List** takes one out of the panel and deletes nothing.
- **Rows:** each project is a row of its own, with only its folders and
  designs below it, sorted by name. A project shows the display name you
  gave it in Project Settings; without one, a Cloud project shows its
  repository's address (`github.com/acme/gripper`,
  `gitlab.com/group/sub/part`, a shared folder's path) and a Local one its
  folder's name. The icon and the tooltip tell Local from Cloud. A
  project shows its sync state as the project indicator shows it (`✓`,
  `↑2` to send, `↓1` newer, `Offline`). A design shows its latest
  version, **Changed** when it differs from that version, and its edit
  lock (`Editing`, `Locked: Alex`); the open design is in bold. The panel
  reads the projects again after a save or a sync and when Mitcad gets
  the focus back (changes made with other programs show then).
- **Open** a design with a double-click (or Enter); unsaved changes are
  asked about as with Open.
- **New Design** (the toolbar, the context menu, or **File › New
  Design…** (Ctrl+N), also in the project indicator's menu) adds a design
  to the selected folder of the current project (with no current
  project, File › New Design opens an untitled design instead): type its
  name in the panel and press Enter. It is recorded as a version (`Add parts/bracket.mitcad`) and
  opens. **New Folder** adds a folder the same way (`Add folder parts`).
- **Rename** (F2, or the context menu of a design or a folder) and a
  **drag** onto another folder or the project move
  a design or a folder within its project, as one version (`Move
  parts/a.mitcad to old/a.mitcad`); a design that is open follows its new
  name. Moving between projects is not possible.
- **Delete** (Del) asks first and removes a design or a folder as a new
  version: the project's history keeps it. A design that is open closes;
  changes that no version holds are lost (the question says so).
- **Project Settings…** (the toolbar's settings button) opens the settings
  of the selected project, or of the project of the selected folder or
  design; a double-click on a Cloud project opens them too. Another
  project's settings open without opening the project.
- **Version History…**, **Show in Folder** and **Sync Now** work on the
  selected design or project; for another project it opens first. **New
  Project…** and **Open from Cloud…** are there too.
- **Save** of a new, untitled design in a project starts in the panel's
  selected folder.

A design locked by someone else cannot be moved or deleted; a name that is
taken is refused. Each change is sent as a saved version is.

**File › Project › Open Project…** opens a folder: a project shows in the
Projects panel and opens the design you opened last there, else the
newest; an empty project opens none. An older project without versions
gets them at once. A git repository, a folder of designs and an empty folder go to New
Project with the folder filled in. A project inside another git repository
opens without versions. **Open…** and **Open Recent** (designs listed with
their project, `bracket.mitcad — Robot arm`) recognise the project of a
design the same way.

**The project indicator** shows the current project where the version was
in the status bar (on a Mac beside the document's name in the title bar
row): `Robot arm · Local · v14`, `Robot arm · Cloud · ✓` (with `↑2` to
send, `↓1` newer, `Offline, ↑2 waiting`), or `Not in a project`. A click
opens its menu: Sync Now, Check for Newer Versions, Version History,
Project Settings, Open in Browser and New Design; for a design in no
project **Move to a Project…** (also in File › Project), which moves the
design into a new project (its own folder by default) or one you have,
and records it.

**File › Project › Project Settings…** has the project's display name,
its folder, its storage (Local or Cloud), and on this computer its
author, whether
each version is sent at once and how often the remote is checked
(Preferences › Cloud gives the defaults). The **Display name** is the
name the Projects panel shows for the project on this computer; it never
renames the folder or the repository, and clearing it brings back the
default (a Cloud project's repository address such as
`github.com/acme/gripper`, a Local project's folder name).

When the file changed outside Mitcad since you opened it (another
program, another Mitcad, git), Save asks first: **Save as New Version**
(the outside change is recorded as a version first, so both stay in the
history), **Save As**, or **Compare**. When that outside change cannot be
recorded (HEAD detached, a merge in progress, the repository's index
locked by another program) or the version status cannot be read, Save
stops with an error and leaves the file as it is; Save As still keeps
your design in another file. A design renamed or moved within
its project outside Mitcad keeps its view settings, and its next save
records the rename.

A project is an ordinary git repository, so git tools work on it. A
design file moved out of its project folder lacks its imported bodies;
`mitcad-cli convert` makes a single-file design of it again
([Command line](#command-line)). Internals:
[architecture.md](architecture.md).

**Version History** (File › Version History…, Ctrl+Shift+H) lists the
open design's versions, newest first, each with when it was saved, by
whom, its description and what it changed (`d3 20 mm -> 25 mm, 1 feature
modified`). The selected version shows its whole description, its id and
a preview of the view as it was saved on this computer. Below, it is
compared with the version before or with the open design (including
unsaved changes): parameters, timeline features, sketches, components and
bodies; **Compare Geometry** also compares the bodies' volumes and areas.

- **Open** opens a version as a new, untitled design ("bracket v3");
  the file and its history stay as they are.
- **Restore** makes a version the latest again by recording its design
  as a new version (`Restore v3 of bracket.mitcad (abc1234)`). The
  versions since stay; history is never rewritten. Unsaved changes are
  saved as a version first, or dropped if you choose; a change made to
  the file outside Mitcad is recorded as a version first. When either
  cannot be recorded as a version, Restore stops with an error saying
  why and writes nothing over the file (unsaved changes it saved stay in
  the file, without a version yet).
- **Save Copy As** writes a version to a file of your choice.


### Sharing a project

A **Cloud** project's versions are shared through a git repository: on a
git service (GitHub, GitLab, Forgejo or Gitea, any server) or in a shared
folder. This needs the git program (Git for Windows on Windows).

- **New Project** with Storage **Cloud**, or **Project Settings** of a
  Local project with **Cloud** and **Share** (which sends its whole
  history; the dialog lists the authors whose names and email addresses
  the versions carry), or **File › Project › Open from Cloud…** to open
  one.
- **The Cloud section:** paste the repository's address into **Repository
  address**, the one your server's clone button shows
  (`https://git.example.org/team/robot-arm.git`,
  `git@git.example.org:team/robot-arm.git`, `ssh://git@git.example.org/team/robot-arm.git`),
  or the address of the repository's page in the browser
  (`https://git.example.org/team/robot-arm`, also with `/src/…` or
  `/tree/…` after it), or the `git clone …` line, or a folder. Any server
  works: an address on a host Mitcad does not know is taken as
  `owner/repository` on a Forgejo / Gitea style server, and an address with
  a port, another SSH user or a deeper path is used as it is (**Other
  address**). A malformed address is refused at once ("This is not a valid
  repository address") instead of being handed to git.
  **Details** shows what the address was read as and lets you change it:
  the **Service** (GitHub, GitLab, Forgejo / Gitea with its server, Shared
  folder, Other address), the **Owner** (you or an organisation; on GitLab
  also a group with its subgroups, `group/sub`), the **Repository** (named
  after the project's folder until you type one) and **HTTPS** or
  **SSH**; the address follows what you change there, and an address
  pasted into the Server, Owner or Repository field is read like one
  pasted into Repository address.
  `https://gitlab.com/group/sub/robot-arm.git` gives the owner
  `group/sub`. GitHub and Forgejo / Gitea have one owner level; an
  address with a part starting with `~` stays an Other address. **Create
  Repository in Browser…** (under Details) opens the service's page for a
  new repository (GitHub's form filled in, also for an organisation; on
  GitLab and Forgejo choose the owner on the page). A shared folder that
  is empty is made a repository.
- The address is checked as you type, and again when the dialog gets the
  focus back (after making the repository in the browser): empty and
  ready; files but no project (a README: the project goes beside them, its
  history after theirs); a Mitcad project (**Open It Instead** opens it);
  not found; or a problem of signing in or of the network. When the server
  cannot be reached, **Create as Local for Now** makes a Local project,
  shared later in Project Settings.
- **Open from Cloud** clones a project into a new or empty folder (by
  default Documents/Mitcad/<repository>), with a summary of what is there
  (`Project "robot-arm": 14 versions, latest 2026-10-07 by Your Name`). An
  empty repository gets a new, empty project (**Create Project Here**), one
  with files but no project gets one beside them (**Make It a Project**),
  and a folder that holds a clone of the repository opens as it is (**Open
  It**). The project shows in the Projects panel as its repository's
  address, and opens its design opened last, else the newest.

**Signing in** is git's: Mitcad never asks for, keeps or passes a password
or a token, and refuses addresses with one in them. Over SSH it uses your
SSH key (and its agent); when the server refuses it, **Copy Public Key**
puts your key on the clipboard for the service's **SSH Keys Page**: the key
goes to your own user account, also for an organisation's repository. Over
HTTPS git uses a credential helper (Git Credential Manager comes with Git
for Windows). A server whose identity this computer does not know yet
shows **Trust This Server…**: its keys' fingerprints, checked against the
keys GitHub, GitLab and Codeberg publish (for another server, compare them
with what its administrator gives); trusting adds them to your
`~/.ssh/known_hosts`.

In a Cloud project:

- Every version Save records is sent at once (Project Settings, with
  Preferences › Cloud as the default, can turn that off). Offline, versions
  wait and the indicator shows them (`Offline, ↑2 waiting`); they are sent
  when the repository can be reached again.
- The repository is checked for newer versions when a design opens, every
  10 minutes and at the first change after opening (Project Settings: how
  often, or never). The indicator shows them (`↓1`), and a newer version of
  the open design shows a notice under the toolbar with **Sync Now**.
- **Sync** (File › Project › Sync, Ctrl+Alt+Y) saves the open design,
  takes the remote's newer versions and sends yours after them; the
  history stays one line. The design is reopened when its file changed.
- When you and someone else both changed a design, **Resolve Sync
  Conflicts** asks what to keep: **Keep mine**, **Take theirs**, or
  **Save mine as a copy** (the default: the file becomes theirs, and
  yours is kept next to it as `bracket (conflict copy Your Name
  2026-10-05 14.03).mitcad`); **Compare** lists what differs. Designs are
  never merged, and nothing is lost: versions left out stay in a backup
  of the history.
- **Project Settings** shows the address with **Change…** (a new address
  of the same repository, when it moved; another repository is refused)
  and **Open in Browser**, the state with **Sync Now**, and **Local**
  stops syncing: the remote is removed, the versions stay here and in the
  repository. Under **For everyone in this project** are the settings the
  project's members share, recorded as a version and sent; the others'
  Mitcad follows them after its next sync.
- Designs linked from outside the project are not shared.
- A repository with **several remotes** (set up with git) syncs with the
  one its branch follows, else `origin`, else the only one. When none of
  these applies, the indicator says `No remote chosen`, nothing is sent
  or fetched, and Project Settings asks which remote to follow (**Remote**
  and **Follow**; `mitcad-cli remote follow` does the same).

**Live updates** make what others do arrive at once instead of at the next
check: a version someone sends, and who has a design open. They go
through an MQTT broker that the project names (Project Settings › Live
updates › Broker, with a topic prefix, `mitcad` by default; or **Another
broker** or **Off** for this computer only, under Live updates here). The
versions themselves stay in the git repository; the broker only says
that something changed.

- **Trust:** the first time a project wants to connect to a broker,
  Mitcad asks: "Robot arm uses live updates through broker.example.com.
  Your name and the designs you open are sent there." **Connect** or
  **Not Now**. Anyone who can send versions to the project can change its
  broker, so nothing connects before you answer, the answer is kept for
  that address, and another address asks again. A plain-text broker
  (`mqtt://`) says so in the question: anyone on the network can read its
  messages.
- **Signing in:** a broker that wants a user name and a password shows a
  notice under the toolbar, "Live updates need you to sign in to
  broker.example.com" with **Sign In** and **Not Now**; the design opens
  meanwhile. They are kept in the system's keychain (Windows Credential
  Manager, the macOS Keychain, the Secret Service or KWallet on Linux),
  never in the project or Mitcad's settings, and go only to that broker's
  host and port, and only over an encrypted connection (`mqtts://`; a
  password for `mqtt://` is refused). Without a keychain they are kept
  until Mitcad closes.
- **The indicator** says `· Live` while the connection is up, and
  `· Live offline (polling)` while it is not: Mitcad then checks the
  repository on its own again. While live, a version someone sends shows
  at once (`↓1` and the notice of a newer version of the open design),
  and the repository is not checked on the timer.
- **Test** in Project Settings connects to the broker, subscribes,
  publishes one message under the prefix and waits for it, and lists the
  steps.

**Tools › Preferences › Cloud:** the git program (found on `PATH`, on
Windows also where Git for Windows installs itself, or chosen), the
default author (when git has none), the defaults of projects for sending
each version at once and for how often to check for newer versions,
whether live updates are allowed (off: no project connects to a broker),
the broker New Project offers, and **Known brokers**: the brokers you
answered the trust question for, with the user you signed in as. **Sign
Out** removes a broker's user name and password from the keychain;
**Forget** also removes the answer, so that the next connection asks
again.

#### Edit locks

In a Cloud project a design has one editor at a time. Opening a design
takes its **edit lock** on the remote (a reference of the repository's
own, outside its branches); anyone else who opens it gets it
**read-only** and can ask for it. Locks are on by default; Project
Settings turns them off for everyone, and sets the idle time (1–120
minutes, 10 by default) and how often the remote is read (5 s–10 min, 10 s
by default; every 2 minutes while live updates are connected, which tell
of every change at once). Other git tools do not see them, and a remote
that does not take such references (Project Settings says so) has none.

- **Opening a design:** free, it opens for editing and the indicator says
  `Editing`; if the remote has a newer version of it, that is taken first
  (as Sync does). Someone else editing it: it opens read-only with
  "part.mitcad is being edited by Alex (since 13:40, last active 2 min
  ago)", **Continue Read-Only** or **Request Edit Access…**. Open
  elsewhere by yourself (another computer or window): **Take Over** (the
  other window turns read-only and keeps its changes) or **Open
  Read-Only**. A lock whose holder is gone (its Mitcad ended without
  releasing it, it did not answer, or, with live updates, its connection
  was lost and you said so) is taken as free, and you are told whose it
  was. When the remote cannot be reached the design opens for editing
  with `Edit lock not confirmed (offline)`, and the lock is taken once the
  remote answers (or the window turns read-only if someone else holds it
  by then). **File › Open Read-Only…** opens a design without a lock; an
  older version opened from Version History takes none either.
- **Read-only windows** show and inspect the design (selection, measuring,
  sections, isolating, exporting, 3D printing, rendering, Version History
  and its comparisons) but do not change it: commands, timeline edits,
  sketches and parameters are refused. The banner under the toolbar says
  why, with **Request Edit Access…** (a message of up to 200 characters),
  and shows the holder's answer and, with live updates, who else has the
  design open. When the holder sends a version, the window shows it
  without changing your copy of the file ("Updated to Alex's version
  saved at 14:05."); your copy follows when you get the lock. **Save As**
  keeps a copy, which opens for editing with a lock of its own.
- **Holding a lock:** a command, an edit, a selection or a change of the
  view keeps it active. After the idle time without any: with no unsaved
  changes the design's versions are sent and the lock released; with
  unsaved changes and autosave on (Preferences › General) they are saved
  as a version first ("Saved automatically before releasing the edit
  lock"); with autosave off the lock stays, marked idle, and a request
  gets it at once, your changes staying in your window. The banner then
  says "Your edit lock was released after 10 minutes without activity",
  and **Edit** takes it again. The indicator's menu has **Release Edit
  Lock** and **Edit Lock Details…** (the holder, since when, the requests,
  who else has the design open, the live updates' connection).
- **Requests:** your Mitcad answers with a receipt by itself, and asks
  you: **Release** (unsaved changes are saved first, or **Release Without
  Saving** keeps them in your read-only window; the versions are sent and
  the lock goes straight to the one who asked), **Keep 15 More Minutes**
  (asked again then), or **Decline…** with a message. Without an answer
  within the idle time the lock is released as at the idle time. When
  your Mitcad does not answer at all (it ended, or the computer is off),
  the one who asked takes the lock after two of its polls. The other way
  round, the asking Mitcad renews its request while it waits; a request
  whose Mitcad ended (or whose window closed without withdrawing it) is
  no longer renewed, and after the idle time and two polls nobody is
  asked about it any more and the next Mitcad that sees it removes it.
  A declined request goes the same way unless you ask again.
- **Losing a lock** (taken over, or taken because your Mitcad did not
  answer): the window turns read-only and says who took it; unsaved
  changes stay, and **Save as Copy…** or **Save as New Version** keeps
  them (Sync then asks which version of the file to keep).
- **Closing** a design or quitting sends the versions not sent yet and
  releases its lock and requests; when that sync needs your choice,
  **Resolve Sync**, **Release Without Sending** or **Keep Lock**. Offline,
  the lock stays until it is stale. After a crash it stays too; reopening
  the design (or restoring its autosaved work) checks it as opening does.
- **Sync** asks before it sends a version of a file someone else holds
  the lock of: **Send Anyway** or **Cancel**.
- Turning edit locks off releases your locks; others' stay listed by
  `mitcad-cli lock status` and can be removed with `mitcad-cli lock
  release --force`.


### Component libraries

Fasteners and other standard parts come from component libraries: git
repositories of designs, each with a table of its sizes. Mitcad's own
library of parts has ISO metric screws (ISO 4762, ISO 4017, ISO 10642),
nuts (ISO 4032) and washers (ISO 7089) from M3 to M12, and the BUILD
tab's slot profiles, connectors and other parts. It was the fastener
library up to its version 1.0.0; designs made with that keep their parts. Libraries need the git
program to be fetched; parts already in a design need nothing.

1. **Tools › Libraries › Libraries...** lists the libraries and
   community indexes to use (Mitcad's parts library and the community
   index to start with). Add a library by its URL or a folder, turn one
   off, and **Fetch** to get it or its newer versions. Nothing is fetched
   unless you ask.
2. **Insert from Library** (the ASSEMBLE group, or Tools › Libraries):
   search by name, standard or tag (`4762`, `washer`), choose the version
   (the newest first) and the size (Size, then Length), and insert the part
   **linked** (read only) or as a **copy** (editable, with its history).
   The licence and the attribution are shown before you insert. On a new
   installation the list is empty until you press **Update**, which
   fetches all enabled libraries (the same as Fetch All in Libraries...)
   and searches again; press it later to get newer versions to choose
   from. Parts already in a design keep their versions.
3. Each linked part keeps the library version chosen for it. Opening the
   design reads that version even when the library has moved on, so the
   design always rebuilds the same way. **Library Parts...** shows the
   parts with their versions and sizes: choose another version or size,
   **Show Changes** lists what changes (`M5x16: dk 8.5 mm -> 8.7 mm`),
   and **Update** applies it as one undo step. **Check for Newer
   Versions** fetches the libraries first.
4. Library parts carry their standard designation (`ISO 4762 M5x16`).
   The **Parts List** tab of Library Parts lists every part with its
   quantity, library, version and licence, and copies it as CSV.

A design from another computer whose libraries are not here keeps the
parts' saved bodies; **Get Missing Libraries** in Library Parts fetches
them after showing their addresses.

### Community library

**Tools › Libraries › Community Library...** searches the designs and
components others share in their own git repositories, listed by
community indexes (an index is just another git repository). Items show
their licence and what it asks of you (CC0, CC-BY, CC-BY-SA, MIT, Apache,
BSD, CERN-OHL); items without a licence are hidden unless you ask for
them. **Get Library** fetches a library an index lists; then insert its
parts linked or as copies. The licence and the source are recorded with
every part in the design.

**Tools › Libraries › Publish to Library...** adds the open design to a
library of your own: a folder (made when new, with your choice of
licence) that keeps versions like a project. It records the design with
an image of the view, pushes the library to a remote you give, and
copies the entry to propose to a community index. The format is in
[libraries.md](libraries.md).

### Cache of computed results

Results of costly features are kept on disk in your user cache folder, so
that opening a design again (or a copy, or an older version of it) only
computes what changed. Tools › Preferences, Cache: on or off, the size
(5 GB by default; the least recently used results go first), the folder
with Open Folder, and **Clear**. A new version of Mitcad does not reuse
an older one's results. In memory, computed results take at most a
quarter of the machine's memory by default ("In memory, at most").

**Help › Diagnostics** (also from Preferences, Cache) shows what both
caches hold: size against limit, hits and misses, disk reads and writes,
space by feature, feature type and document, and where each result of
the last recompute came from. It has **Clear Memory**, **Clear Disk** and
**Export Report** (JSON, or text with a `.txt` name).

### Updates

Once a day at start-up, and with **Help › Check for Updates**, Mitcad
asks its release page whether a newer version is out, sending only its
version, the platform and the channel. A newer one shows in a bar under
the toolbar: **Release Notes**, **Install**, **Skip This Version** and
**Later**. Install downloads the update, verifies its signature, closes
Mitcad (asking to save changes first), installs it and starts Mitcad
again. This works for the Windows installation (Windows may ask to allow
the installer) and for a Linux AppImage in a folder of your own; the
portable `.zip`, other Linux installations and own builds get the release
page instead. A download whose signature does not match is deleted, never
run.

Tools › Preferences, Updates: the automatic check on or off, and whether
pre-releases are offered. Administrators turn checks off for everyone
with the environment variable `MITCAD_NO_UPDATE_CHECK=1` or, on Windows,
the registry value `DisableUpdateCheck` (DWORD 1) under
`HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Mitcad\Mitcad`. More:
[updates.md](updates.md).

### Feedback and error reports

**Help › Send Feedback** reports a problem or a wish without leaving
Mitcad: the kind (bug report, wish, other), a summary, a description, a
contact address if you want one (the issue tracker is public), the
diagnostics (Mitcad's version, the system, Qt and Open CASCADE, the
graphics renderer and driver, the window's layout; on by default) and,
if you tick it, a screenshot of the window, which you can crop by
dragging over the part to keep (**Whole Window** undoes it).

**Preview** shows everything the report contains, section by section:
edit any part, untick what should not be sent, change the title, or
Cancel. Folders and file paths, your user name and the computer's name
are masked before you see it (`<path>/<file>.f3d`, `<user>`, `<host>`);
names of program and source files stay, so that a stack stays readable.
Your design and its files are never attached.

**Send** opens Mitcad's issue form in your web browser with the
title and the report filled in; nothing is sent until you submit it
there, and Mitcad holds no account or token. A report too long for the
form's link is on the clipboard: paste it over the shortened text. A
screenshot is saved in Mitcad's data folder (`reports`) and the message
says where: drag it into the form.

**Error reports.** When Mitcad crashes, it writes a crash report (the
version, the platform, the signal or exception, the stack of the
crashed thread and the last commands you used, never design content)
and offers it the next time it starts. A crash of the import or render
process is offered at once, and so is an internal error: the geometry
kernel crashing inside an operation (the operation fails and the
design stays as it was) or an unexpected error from a command. Review
Report shows the same preview: add what you were doing, edit or leave
out parts, Send or Cancel. The report carries a duplicate key
(`mitcad-1a2b3c4d5e6f`, also in the title): the same error gives the same
key, so its earlier reports can be found by searching for it. **Do not
offer error reports again** in the offer turns offers off (setting
`reports/offerErrors`); Help › Send Feedback stays. Crash reports are
kept in the `crashes` folder of Mitcad's data folder (the newest 20).

Administrators and builds of their own point the reports at another
issue tracker with the setting `reports/issueUrl` (an issue list's
address, `https://host/owner/repository/issues`, or a template with
`{title}` and `{body}`).

### Debug log

When something odd happens in normal use (a cursor that stays busy, a
command that does nothing), a debug log shows afterwards what Mitcad did.
**Preferences › Diagnostics › Write a debug log** switches it on at once,
without a restart, and it stays on (also at the next start) until you
turn it off there or in the project indicator's menu (**Turn Off Debug
Log**). While it runs, the project indicator (in the status bar; beside
the document's name in the Mac layout) says **Debug log on**. For one run
only, start Mitcad with `--debug-log` (or `--debug-log trace.log`) or
with the variable `MITCAD_DEBUG_LOG=1` (or a file's path).

The log is written to Mitcad's log folder (**Help › Open Log Folder**;
`logs` in Mitcad's local data folder), one file per start,
`mitcad-YYYYMMDD-HHMMSS-<process>.log`; a file continues in a new one at
20 MB, and the folder keeps the newest 10 files, at most 200 MB. Each
file begins with Mitcad's version and build, the system, the graphics
card and its OpenGL renderer, the command line, the open design's path
and the switches in use; started with Mitcad, the file is written once
the 3D view knows its graphics card (at most a few seconds, at once when
something goes wrong first); when it is switched on, the lines Mitcad kept
in memory from before (the newest 5000) come first. Each line has the
time in milliseconds, the level (debug, info, warning, critical), the
thread when it is not the main one, and the text. It is written at least
once a second and at every warning, so a crash loses at most a second.

**What a log contains.** Commands, sketch operations, computations,
refusals, warnings: file paths, design names and names of features,
bodies and sketch entities. What you type into fields and where clicks
pick are left out unless switched on separately (`MITCAD_LOG_TYPED_TEXT`,
`MITCAD_LOG_PICKS`, for tests). Logs stay on your computer: a report
(Help › Send Feedback, an error report) offers the latest log and the
newest lines kept in memory, both left out unless you tick them; a
ticked log file is not sent but its place is shown, to drag it into the
issue form.

## Import and export

### Base features

An imported file comes onto the timeline as a **base feature**: bodies
without a history. STEP and IGES bodies keep their names and colours;
STL and OBJ bring mesh bodies, which cannot take part in boolean
operations. Later features can still use an imported body's faces and
edges (a fillet, for example).

### 3D printing

**3D Print** sends bodies to a slicer (Bambu Studio, OrcaSlicer,
PrusaSlicer, UltiMaker Cura or any program that opens files given on its
command line). Its panel starts with the selected bodies in **Bodies** (a
face, edge or vertex stands for its body, a component's occurrence in the
browser for the bodies shown in it), or with nothing selected every
visible solid and mesh body. The bodies to be sent are highlighted; while
the panel is open, a click on a body in the view adds it or takes it out
(a window selection adds several). Surface bodies and empty ones cannot
be printed, nor picked; those that were selected are left out. The panel
also chooses:

- **Format:** *STL, one file per body* (the default; a body shown more
  than once, in a component placed several times, gets a file per
  occurrence picked, such as `Pin (Arm_2).stl`) or *3MF, one object
  with parts*: one file in which the bodies are the parts of one object,
  with their names and appearance colours. Both have the bodies where
  the view shows them. 3MF is the reliable way to get one object of
  several parts in the slicer (UltiMaker Cura loads the parts as a
  group).
- **Refinement:** Low, Medium, High (the default) or Custom: how far the
  triangles may lie from the surface (mm) and the angle between
  neighbouring triangles.
- **Slicer:** the one of Preferences (3D Print), found automatically
  where possible, or another program (an AppImage or, on macOS, an
  application in the Applications folder, for example).

**Send** (Enter) writes the files into `mitcad-print/<design>` in the
temporary folder, which is emptied before each send, and gives them to
the slicer. Without a slicer, or when it does not start, the folder
opens instead. The choices are remembered.

Slicers are found in their usual install folders and the list of
installed programs (Windows), in the Applications folders (macOS; an
application gets the files through `open -a`), and on `PATH`, as
Flatpaks and in desktop files (Linux).

### USDZ for AR and 3D viewers

File › Export with the type *USDZ for AR and 3D viewers* (or a file name
ending in `.usdz`) writes a Universal Scene Description package: one file
that phones' and tablets' AR viewers, web viewers, renderers and other 3D
programs open with the design's look.

- **What goes in:** the visible bodies (with *Selected bodies*, those
  selected), where the view shows them; hidden bodies and hidden
  component occurrences are left out. Components stay a hierarchy: each
  occurrence is a group with its placement holding its bodies, so a
  viewer can move them apart. With Coordinates set to the components'
  coordinates, each body is written once as its component has it.
- **How it looks:** each body's appearance becomes a material (colour,
  metal or not, roughness, clear coat, emission, see-through for glass;
  a body without an appearance keeps its colour or the default grey), and
  faces with appearances of their own keep theirs. A texture's image goes
  into the file with the projection the rendered view uses; an image
  that cannot be found is written as its base colour and the log says
  so. Curved faces are smooth and edges sharp; Refinement chooses how
  finely the faces are divided into triangles.
- **Size and orientation:** the file is in millimetres and says so, and
  it stands Y up as viewers expect, so the design appears in AR at its
  real size and upright.
- The file is written with Mitcad's own code; nothing else needs to be
  installed. `mitcad-cli export design.mitcad design.usdz` writes the
  same file from the command line.

### .f3d import

An `.f3d` design file (or an `.f3z` package of them) opens with its
history (File › Open, or `mitcad-cli import-f3d`). Parameters, sketches
with constraints and dimensions, construction planes, extrudes,
revolves, sweeps, lofts, pipes, fillets, chamfers, holes, threads,
patterns, mirrors, splits and Combine are replayed as Mitcad features,
and after every feature the bodies are checked against the file. A
feature that cannot be replayed comes in as a base feature of the file's
bodies at its place, and the timeline continues on them, so the result
always has the file's geometry. Ribs and webs always come in this way.
The whole import is one undoable step.

Components and occurrences come in at the file's placements. Joints,
as-built joints, joint origins, rigid groups, grounded occurrences and
captured positions come in as Mitcad's where they hold
where the file places the parts; a joint that does not hold there comes
in as an as-built joint, so that nothing moves. Parts inserted from other
documents (fasteners, for example) come in as empty components named
after the item that inserted them: their bodies are not in the file, but
their joints and positions are.

Bodies and faces keep the file's appearances: each one the design uses
becomes an appearance of the document (named as in the file, its colour,
roughness, metal or transparency taken over), given to the bodies and to
faces painted on their own; an occurrence's appearance goes to its bodies
where its component is placed once. A body's material becomes Mitcad's
where Mitcad has the same one (steel, aluminium, ...); the report lists
every material with its density. Texture images are not in the file: such
an appearance comes in with its stored colour.

In the application the import has no time limit; see [Opening an .f3d
design](#opening-an-f3d-design) for stopping it early. A geometry kernel
operation that does not return for 90 seconds is given up on (that item
comes in as the file's bodies, and the report warns). How well real
designs import: [core/import/README.md](../core/import/README.md). How
the timeline shows what came in how: [Reading an imported
timeline](#reading-an-imported-timeline).

### Reading an imported timeline

The `.f3d`, `.ipt` and FreeCAD imports leave on the timeline what their
report says, so the document tells it after it has been saved and opened
again:

- **Yellow, a base feature:** a fallback. Its tooltip names the items of
  the file it stands in for and why each did not replay ("Stands in for
  Extrude5 (its result is not in the file's history)"). A base feature
  that brings the bodies that differ to the file's, after an approximate
  item or at the end, says that. Bodies the import made within 0.01 mm
  of the file's are never replaced.
- **Yellow, another feature:** it came in partly; the tooltip says what
  was left out ("Partly imported: left out: 3 constraints ...").
- **Struck through with a yellow outline:** a placeholder, named after an
  item the file had suppressed or that failed in the file ("Suppressed:
  Suppressed in the file"). It builds nothing; it shows where the file
  had the item. Items with no geometry (occurrences, groups) get none.
- **Struck through with a yellow outline, a real feature:** an `.f3d`
  item suppressed in the file, brought in with its definition ("Suppressed:
  Suppressed in the file; not checked against the file's history").
  Unsuppress it to build it, as in the original design; the file keeps
  no result for it, so the import could not check it. Unsuppress the
  suppressed items before it too when it uses them (a fillet of a
  suppressed extrusion's edges). Where its definition did not come in,
  it is a placeholder that says why.
- **Rolled back, a real feature:** an `.f3d` item after the file's timeline
  marker, brought in with its definition ("Rolled back in the file (after
  its timeline marker); not checked against the file's history"). The
  timeline marker is before it, as in the file; move the marker past it to
  build it. Where its definition did not come in, it is a placeholder
  that says why.
- **Red:** a feature that failed in Mitcad, as anywhere else; only these
  count in "n failed". The yellow "n warnings" beside it lists the
  others, and a click goes to them.

Editing such a feature so that it still builds (its definition or one of
its dimensions) takes its yellow away; undo brings it back. On a long
imported timeline the [timeline search](#browser-timeline-and-parameters)
finds them: `is:warning` the yellow ones and the placeholders' notes,
`is:failed` the red ones, `in:<component>` one component's.

### FreeCAD import

A FreeCAD document (`.FCStd`, FreeCAD 0.19 to 1.1) opens as a new
document with its history (File › Open or Import, or `mitcad-cli
import-fcstd`):

- Each PartDesign Body and each final result of the Part workbench is a
  body named after its label, in components for the document's parts
  (App::Part, Assembly) at their placements. Links (App::Link, also
  arrays and links to other documents next to it) are occurrences;
  hidden objects stay hidden.
- Features come in as Mitcad features, each checked against FreeCAD's
  stored result: pads and pockets, revolutions and grooves, fillets,
  chamfers, holes, mirrors, linear and polar patterns, PartDesign
  booleans, drafts, thickness (shell), lofts, pipes (sweeps), helices,
  primitives, datum planes, lines and points; and the Part workbench's
  primitives, extrusions and revolutions of sketches, cuts, unions,
  commons, fillets, chamfers and mirrorings. A feature Mitcad cannot
  rebuild, or whose result differs, comes in as FreeCAD's result (a base
  feature, `fallback` in the report), and later features continue on it.
  A Body's tip before its last feature suppresses the features after it.
- Sketches are editable, with their constraints and dimensions, on the
  plane or face they were attached to. A sketch with something Mitcad
  cannot express (a tangency to an ellipse, Snell's law, a distance to a
  circle) comes in without it and is marked partial.
- Shapes without a history (plain shapes, Draft objects, bodies of other
  documents) are base features with FreeCAD's colours.
- Spreadsheet cells, VarSets and named constraints become parameters, and
  expressions driving imported values become Mitcad expressions.
  Assembly joints are left out.

The import report says how many features were replayed, came in as
FreeCAD's results or were skipped, which parameters and expressions were
carried over (and why some kept FreeCAD's values), and what each object
became or why it was left out. On the timeline, FreeCAD's results are
yellow base features, partial sketches and features yellow, and features
suppressed in FreeCAD placeholders ([Reading an imported
timeline](#reading-an-imported-timeline)).

### .ipt import

An `.ipt` part file opens as a new document with its history (File › Open
or Import, or `mitcad-cli import-ipt`): its parameters with their
expressions (Change Parameters lists them), its sketches with their
constraints and dimensions, offset work planes, and its extrusions,
revolutions, holes, fillets, chamfers, patterns and mirrors as Mitcad
features, each checked against the bodies the file stored after it. A
feature that cannot be replayed (a face draft, a shell, sheet metal, a
mirror Mitcad does not reproduce) comes in as the bodies the file stored
after it, a base feature, so the final bodies are always the file's. The
document takes the part's length unit, and the bodies its material when
Mitcad's material library has one of the same name. The import report
lists each feature and sketch with how it came in, the parameters and
expressions, the bodies (solid or sheet, valid, faces), the part number,
material, units and the release that saved the file. The import runs in a
process of its own, as the other design imports do; Cancel Import ends it.
`mitcad-cli import-ipt --bodies-only` brings in only the stored bodies,
each a base feature. Files of older releases whose bodies are not stored in
the ASM format are not read. The timeline shows the fallbacks, partial
items and suppressed features as an `.f3d` import does ([Reading an
imported timeline](#reading-an-imported-timeline)).

### IFC import

An IFC file (`.ifc`, or `.ifczip`; IFC2X3, IFC4 or IFC4X3, the open
exchange format of building models) opens as a new document (File › Open
or Import, or `mitcad-cli import-ifc`). Each site, building and storey
becomes a component placed where the file places it, a storey's elevation
a construction plane in it, a grid's axes construction axes. Each
element (wall, slab, beam, column, door, window, furniture, ...) comes
into its storey's component, its bodies named by its name and GlobalId
and coloured as the file colours them: walls, slabs, beams and other
swept shapes as sketches and extrusions (or revolutions, sweeps and
pipes) with their clippings and boolean cuts, which can be edited like
any feature; breps as the bodies the file describes; tessellated shapes
as mesh bodies. Openings are cut from their walls and slabs. A shape that
a type gives many elements (doors, windows, columns, furniture) is one
component, and each element an occurrence of it. Spaces are not
imported. The import report has a line for every element of the file:
how it came in, or why it did not, and checks the bodies' volumes
against the volumes the file states for its elements. What did not
build stays on the timeline as a note ([Reading an imported
timeline](#reading-an-imported-timeline)). The import runs in a process
of its own; Cancel Import ends it.

### .rvt import

An `.rvt` building project opens as a new document of reference geometry
(File › Open, or `mitcad-cli import-rvt`): one component per level, named
as the level, and in it a body for each building element (walls, floors,
roofs, columns, beams, doors, windows, furniture, fixtures, ducts and pipes,
…), named `Family:Type:Id` as the project names it. Walls, floors, roofs,
ceilings, columns, beams and stair runs are rebuilt as features; every
other element (doors, windows, furniture, curtain panels and mullions,
railings, …) is a mesh of the shape the file keeps of it. An IFC export
of the project next to it (of the same name, `project.ifc`) is used to
check the elements and stands in for an element whose shape the file does
not keep; without one, each element built as features is checked against
the shape the file keeps of it, and one that differs comes in as that
shape's mesh. Linked models are found next to the project, in a folder of
the project's folder (such as `Links`) or where the project stores them. Only an element of which nothing is kept comes in as its box,
and such a box is hidden unless the element is structural (a wall, floor,
roof, ceiling, stair run, column or beam): show its body in the browser
to see it. Annotations, rooms, levels, grids, types and group definitions
that are placed nowhere stay out; the import report counts them, lists the
bodies per level and category, says for each box why, and names the
release that saved the file. An `.rfa` family file opens with its metadata
only (its forms come later). The timeline's yellow badge notes how many
bodies are meshes and boxes.

## Command line

The application (`mitcad`, on Windows `mitcad.exe`):

- `--open file`: open a design or a file to import at start-up (an
  invalid design: exit code 2)
- `--set d3=35`: set a parameter's value (a number: mm or radians), as
  Change Parameters does (can be repeated)
- `--demo`: start with a 60 × 40 × 20 mm block
- `--screenshot image.png`: save the 3D view after start-up and quit
- `--no-recovery`: do not offer autosaved work at start-up (File ›
  Recover Documents still does)
- `file`: the same as `--open file`, as a file association passes it
  (one file, not together with `--open`)
- `--chrome=docked|floating`: panels in docks or floating over the 3D
  view (default: floating on macOS, docked elsewhere)
- `--no-glass`: floating panels paint their own material instead of
  Liquid Glass (macOS 26 and later)
- `--debug-log [file.log]`, `--debug-log=file`: write a debug log for this
  run ([Debug log](#debug-log)), to the file given or to a new file in
  the log folder

Environment variables: `MITCAD_NO_UPDATE_CHECK=1` turns update checks
off, `MITCAD_DEBUG_LOG=1|<file>` writes a debug log as `--debug-log` does,
`MITCAD_PROJECTS_DIR` sets where New Project puts projects (instead
of Documents/Mitcad), `MITCAD_GIT` the git program when it is not on
`PATH` (Preferences › Cloud sets it for the application), `MITCAD_LIBRARIES_DIR` where fetched component libraries are
kept, `MITCAD_TEXTURES_DIR` where the texture sources' thumbnails and
fetched maps are kept.
Variables for tests: [development.md](development.md).

`mitcad-cli` works without a UI; `mitcad-cli --help` lists all commands
and options.

- `info part.mitcad [--json] [--properties]`: computes the design and
  prints the timeline with its states and the bodies with their volumes,
  areas, centres of mass and bounding boxes.
- `run script.json [--open part.mitcad] [--save result.mitcad]`: runs
  JSON commands and expectations (`{"expect": {"body": "Body1", "volume":
  48000}}`); a failed expectation: exit code 1. Examples:
  [tools/cli/tests](../tools/cli/tests), commands:
  [commands.md](../core/model/src/api/commands.md).
- `import part.step [--unit-mm N] [--save result.mitcad]`: STEP, IGES,
  BRep, STL or OBJ as a base feature.
- `import-f3d part.f3d --save part.mitcad [--report report.json]`: an
  `.f3d` design with its history. `--design` picks a document of an
  `.f3z`, `--time-limit S` brings the remaining items in as the file's
  bodies after S seconds, `--hang-limit S` interrupts a geometry kernel
  that hangs, `--no-verify` skips the comparison with the file,
  `--no-fallback` leaves out items that do not replay, `--bodies-only`
  imports the bodies only, `--json` prints the reports as JSON.
- `import-fcstd part.FCStd --save part.mitcad [--report report.json]
  [--bodies-only]`: a FreeCAD document; `--bodies-only` imports FreeCAD's
  results without history and sketches.
- `import-ipt part.ipt --save part.mitcad [--report report.json]
  [--reference part.stp [--max-relative X] [--deviation]]`: the bodies
  of an `.ipt` part; `--reference` compares its solids with a STEP file of
  the same part (volume and area, 1e-6 relative by default; exit code 1
  when they differ).
- `import-ifc building.ifc --save building.mitcad [--report report.json]
  [--no-openings] [--no-occurrences]`: an IFC building model; the report
  has a line for every product of the file.
- `import-rvt project.rvt --save project.mitcad [--report report.json]
  [--report-only] [--elements <id>,...]`: an `.rvt` project's building
  elements, one component per level (an `.rfa` family: its metadata);
  `--report-only` reads and reports without importing, `--elements`
  imports only the elements listed by id.
- `export part.mitcad result.step [--bodies …] [--schema ap214|ap242]
  [--unit mm|in|…] [--refinement low|medium|high] [--ascii]`; a `.3mf`
  file holds the solid and mesh bodies as the parts of one object, a
  `.usdz` file the shown bodies with their appearances (textures relative
  to the design's folder).
- `export-sketch part.mitcad Sketch1 sketch.dxf [--r12]`.
- `render part.mitcad -o image.png [--view Front] [--size 1920x1080]
  [--samples 128] [--time-limit 60] [--format png|png16|jpeg|exr]
  [--transparent] [--no-denoise]`: renders the
  visible bodies as Render Image does, with the design's render
  settings; `--view` is a named view or front, back, left, right, top,
  bottom or iso (the default: the design's Home view, else iso).
- `project init folder [--author "Name <email>"] [--no-history]`: makes
  a project folder with version history (the author: `--author`, else
  git's `user.name` and `user.email`); `--no-history` makes the folder
  only.
- `convert part.mitcad out.mitcad [--format v2|v3|auto]`: writes a
  design as a single-file design (`v2`) or as a design in a project
  (`v3`, its imported bodies in the project's folder).
- `library fetch|list|show|search|diff|init|add|check|index-entry`:
  component libraries (`mitcad-cli --help`); `parts part.mitcad`: the
  parts list with the library parts' designations, versions and licences.
- Version history: `version save part.mitcad [-m "message"]` records the
  saved file as a new version (none if nothing changed), `history
  part.mitcad` lists its versions, `version show part.mitcad <version>
  [--save old.mitcad]` opens an older version (`<version>`: an id or its
  first digits, or `HEAD~2`), `version changes part.mitcad <version>
  [<version>]` lists the files that differ, and `version restore
  part.mitcad <version>` makes an older version the newest.
- `diff part.mitcad <version> [<version>]` compares two versions of a
  design (without the second: the file as saved); `diff a.mitcad
  b.mitcad` two files. The result is in the model's terms: parameters
  (`d3 (Extrude1 distance): 10 mm -> 15 mm`), features added, deleted,
  moved and changed, sketches, components, placements and bodies.
  `--geometry` also compares the bodies' volumes and areas; `--json`
  answers in JSON.
- Remote repositories (see [Sharing a project](#sharing-a-project) for
  requirements and signing in):
  - `remote add project-folder <url>` connects the project and sends its
    versions; a repository holding another project's history is refused.
  - `push project-folder` sends new versions (never overwriting the
    server's); `fetch project-folder` gets newer versions without
    changing the files; `remote show project-folder` tells how many are
    to send and how many are newer; `clone <url> new-folder` opens a
    project from a server; `remote remove project-folder` disconnects.
  - `sync project-folder` gets the server's newer versions, puts yours
    after them and sends the result. When you and someone else changed
    the same file, it stops without changing anything and lists the
    file; choose per file with `--resolve part.mitcad=mine`, `=theirs`
    or `=copy` (take theirs, keep yours as `part (conflict copy <you>
    <date>).mitcad`) and run it again. `--dry-run` tells what a sync
    would do. Record your changes as versions first: unrecorded changes
    in a file the sync would change stop it. Your history from before
    the sync is kept in a backup for 30 days (the latest five always).
- Local and Cloud projects:
  - `project inspect folder` tells what a folder is (a Local or Cloud
    project, a repository, designs, empty) with its designs and settings;
    `project create folder --author "Name <email>" [--url <url>]` makes a
    project with its first version, and with `--url` a Cloud one (an empty
    server repository gets its versions; one with files such as a README
    is taken in beside the project); `project settings folder [--set
    '{"shared": {"edit_locks": {"enabled": false}}}']` shows or changes the
    project's settings (a change for everyone is recorded as a version).
  - `remote check <url>` tells what a server repository holds;
    `remote share project-folder <url>` connects a Local project to a
    repository that holds files but no project (your versions go after
    its own); `clone <url> new-folder --adopt` makes a repository with
    files a project.
  - `host-keys <server> [--trust]` shows the server's SSH key
    fingerprints, checked against those GitHub, GitLab and Codeberg
    publish, and with `--trust` adds them to `~/.ssh/known_hosts`.
