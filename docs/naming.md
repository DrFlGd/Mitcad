# Naming

This page fixes the words that Mitcad uses for its own concepts, so that
the code, the user interface and the documentation say the same thing.
A new term, or a new meaning for an existing one, is added here in the
same change that introduces it.

## The design hierarchy

```
Project                 a folder; optional
└─ Design               one .mitcad file
   └─ Root              the root component
      ├─ Body
      └─ Occurrence     Component1:1
         └─ Component
            ├─ Body
            └─ Occurrence
               └─ Component
                  └─ …
```

| Term | What it is | In the code |
|---|---|---|
| **Project** | A folder with the marker file `.mitcad/project.json`. It holds designs, in folders of its own choosing, and keeps their versions. A design need not be in a project: outside one it is a single-file design. | `file::Project` (`core/model/src/file/project.rs`), `ProjectsPanel` |
| **Design** | One `.mitcad` file: one timeline and one root component with everything placed in it. When the file as such matters (paths, saving, linking), say **design file**. | `Document` (the design in memory), `file/` |
| **Component** | The thing that owns bodies, sketches, construction geometry and features, all in its own coordinates. The design itself is the **root component**, named `Root` unless renamed. Every other component is placed in the root or in another component by occurrences. | `ComponentDef`, `ComponentUid` (`core/model/src/assembly.rs`) |
| **Occurrence** | One placement of a component in a parent component, with a rigid transform. Its name is the component's with a number, `Component1:2`. Several occurrences of one component share its definition, so a change shows in all of them. Components nest through occurrences: an occurrence of a component that itself holds occurrences places those again (a subassembly). | `Occurrence`, `OccurrenceUid` |
| **Body** | A solid, surface or mesh body of a component. In user-facing text a body made of open faces is a **surface body**. | `BodyUid`, `BodyKind` (`Sheet` is the surface body) |

The structure of components and occurrences is the design's
**assembly**. "Assembly" describes a design that has components; it is
not a level of its own, and Mitcad has no separate assembly file.

### Words that are not synonyms

- **Part** does not mean component. Write *component* for the thing in
  the hierarchy, also for one that comes from a library or that the
  Build tab places. *Part* is kept where it means an item of a bill of
  materials: **Parts List**, **part number**, supplier fields. Existing
  names that use *part* for a component (Library Parts, Part picker,
  Linear Part, Derived Part, "build part", "library part") are known
  deviations: new text does not copy them, and renaming them is a change
  of its own, docs and UI together.
- **Instance** is not used in user-facing text; it is an occurrence.
  Internal code may speak of instances where it means the placements
  computed along each path from the root (`Assembly::instances`).
- **Document** is the Rust type of a design in memory. User-facing text
  says *design*.
- **Model** is not a level of the hierarchy. `core/model` is the name of
  the crate.
- Importers keep the vocabulary of their source format (an `.ipt`
  part, a 3MF `Part`, a SolidCraft `Occurrence`) inside their own
  crate and map it to these terms where the design is built. Import
  reports and messages shown to the user use Mitcad's terms, naming the
  source's term only where it helps to find the item in the source.

## Annotations

| Term | What it is | In the code |
|---|---|---|
| **Annotation** | A dimension, a leader note, a datum label or a feature control frame kept in the design and drawn in the view in its plane; listed in the Annotations window. The kept measurements of Measure are annotations: user-facing text no longer calls them *measurement objects*. | `Annotation` (`core/model/src/document/annotations.rs`; `Measurement` is its old name), `AnnotationsWindow` |
| **Dimension** | An annotation that shows a measured value: a distance, an angle, an offset, an axis or centre distance, a radius or a diameter, with its prefix, decimals, own text and tolerance. Not a sketch dimension, which drives a sketch; this one only measures. | `MeasureKind` other than the leaders (`Note`, `Datum`, `Frame`) |
| **Leader note** | An annotation of text on an arrow to a face, edge or vertex. Short: **note**. | `MeasureKind::Note` |
| **Annotation group** | A name annotations share (`Manufacturing`), to show or hide them at once and to take one group into a snapshot. Not a timeline group. Short in the Annotations window: **group**. | `Annotation::group` |
| **Tolerance** | A dimension's permitted deviation: symmetric (±), deviations, limits (written as the largest and smallest size) or an ISO 286 **fit code** (`H7`, `g6`). | `Tolerance`, `iso286.rs` |
| **Datum label** | A letter (A, B, C …; never I, O or Q) in a square on a leader ending in a filled triangle on a planar face, an edge, a round face's axis or a datum plane or axis (ISO 5459): the datum feature frames refer to. Not a datum plane, axis or point of construction geometry. | `MeasureKind::Datum`, `Annotation::letter` |
| **Feature control frame** | A geometric tolerance (ISO 1101) in a row of cells: the characteristic's symbol (flatness ⏥, perpendicularity ⟂, position ⌖ …), the tolerance zone's width with Ø and Ⓜ or Ⓛ where they apply, and the datum letters in order; on a leader to a face or edge, or under a dimension. Short: **frame**. | `MeasureKind::Frame`, `GeometricTolerance`, `Characteristic` |

## Drawings

| Term | What it is | In the code |
|---|---|---|
| **Drawing** | A 2D drawing kept in the design, in the **Drawings** folder of the root or of a component; its sheets and views are definitions, drawn from the model when shown or exported. Not the export file, which is a PDF, SVG or DXF file of a drawing. | `Drawing`, `DrawingUid` (`core/model/src/drawing/def.rs`), `DrawingWorkspace` |
| **Sheet** | A page of a drawing: its size, orientation, projection method (first or third angle), title block and views. | `DrawingSheet` |
| **View** | A projection on a sheet: a **base view** of the design, a component or chosen bodies; a **projected view** from another view; a **section view** cut along a line on a view; a **detail view** enlarging a circle of a view. Not the 3D view, which user-facing text calls *the 3D view* where both could be meant. | `DrawingView`, `ViewKind` |
| **Break** | A band taken out of a view to shorten it, its sides moved together. | `ViewBreak` |
| **Close Drawing** | Leaves the open drawing for the 3D view. It replaces *Back to Model*: the model is not elsewhere, the drawing is a view of it. | `drawing.close`, `DrawingWorkspace::closeDrawing` |
| **Annotation** (on a view) | A dimension, a centre mark or centreline, a leader note, a datum label, a feature control frame or a surface finish symbol on a drawing's view, its picks edges and faces of the model seen in the view; the same kinds of dimension, tolerance, datum and frame as the model's annotations. Broken when what it picks is gone. | `ViewAnnotation`, `AnnotationType` (`core/model/src/drawing/annotate.rs`) |
| **Pick** (on a view) | An edge, a point of an edge or a round face of a body that a click on a view finds, kept by its topological name so that the annotation follows the model. | `ViewPick`, `drawing_pick` |
| **Centre mark** | Crossed centre lines through the centre of a circle seen end on. UI: *Center Mark*. | `AnnotationType::CenterMark` |
| **Centreline** | The axis of a round face seen from the side, or a line through two centres or between two edges. UI: *Centerline*. | `AnnotationType::Centerline` |
| **Surface finish symbol** | ISO 1302's symbol on a surface: any process, material removed or no material removed, with its requirement (`Ra 1.6`). UI: *Surface Finish*. | `AnnotationType::Finish`, `SurfaceFinish` |
| **Text note** | Text on a sheet without a leader. UI: *Text*. Not a leader note (*Note*). | `SheetNote` |
| **Sketch dimensions** (of a view) | The dimensions of the sketches a view sees square on, drawn from the sketches (retrieved). | `DrawingView::retrieved_dimensions` |
| **Datum label** (on a view) | A datum label on a drawing's view; its letter is one of the same set as the model's datum labels. UI: the *Datum Label* tool, not *Datum*. | `AnnotationType::Datum` |
| **Hole table** | A table of the round holes a view sees end on, each tagged in the view (A1, A2, B1 by size, or numbered) and listed with its place from an origin, size, depth and thread. Not File › Export Drawing's sheet-wide hole table of its own. UI: *Hole Table*. | `AnnotationType::HoleTable`, `TableSettings` (`core/model/src/drawing/tables.rs`) |
| **Parts list** (on a sheet) | A numbered table of the components a view shows, with their quantities and what the design says of them; the drawing's own form of the design's **Parts List**. UI: *Parts List*. | `AnnotationType::PartsList` |
| **Balloon** | A component's number in a parts list, in a circle on a leader to the component on a view. UI: *Balloon*. | `AnnotationType::Balloon` |
| **Sheet template** | A sheet's frame, title block and revision table as data: its fields filled in from the design (name, part number, material, mass, author, date, scale, sheet, revision, any property). Built in (ISO, ANSI), the user's or a project's; a sheet keeps a copy. UI: *Template*, *Save as Template…*; *Simple Title Block* for a sheet without one. | `SheetTemplate` (`core/model/src/drawing/template.rs`) |
| **Revision** (of a drawing) | A row of a drawing's revision table: its letter, description, date and author; the title block shows the latest one's letter. UI: *Add Revision*. | `Revision`, `Drawing::revisions` |

## Sketches

| Term | What it is | In the code |
|---|---|---|
| **Inactive** (constraint, dimension) | A sketch constraint or dimension kept in the sketch and drawn (pink), but left out of the solve until it is activated; an inactive dimension keeps its value. An import makes those its file does not meet inactive. Not *suppressed*, which is said of features, and not *driven*, which is a dimension that only measures. UI: *Activate*, *Deactivate*; the tooltip *Not applied: …*. | `Constraint::active`, `Dimension::active` (`core/model/src/sketch/mod.rs`), `sketch.set_active` |

## UI names in user-facing text

The user guide, the help, the README, tooltips, status messages and
dialog texts name a control exactly as the user sees it in the default
interface, the Floating chrome. The internal identifier of a control is
not a name for it.

- **Tabs and groups.** Their identifiers are upper case (`SOLID`,
  `BUILD`, `ASSEMBLE`, `SHEET METAL`). The Floating chrome, and the
  native macOS interface, show them in title case (`displayName()` in
  `app/framework/Ribbon.cpp`): write *the Solid tab*, *the Build tab*,
  *the Assemble group*, *the Sheet Metal group*. The upper-case form
  belongs to code and to developer documents that quote the identifier
  in backticks. The Docked chrome shows the group menus in upper case
  (`ASSEMBLE ▾`); the documentation does not follow that.
- **Commands.** The command's label as its `CommandDef` gives it, with
  the same spelling and capitals: *New Component*, *Insert from
  Library*, *Create Components from Bodies*.
- **Menus.** The path as the menu bar shows it, with `›` between the
  levels: *Tools › Preferences*, *Tools › Libraries › Libraries...*.
- **Dialogs, panels, inputs and settings.** Their title or label as
  shown: *the Projects panel*, *the Display page of Tools › Preferences*,
  *Ghost inactive components*.
- **Names in the browser and the timeline** as Mitcad makes them:
  `Root`, `Component1:2`, `Body1`.

A change that renames a control changes the user-facing documents that
name it in the same change. Developer documents (`app/COMMANDS.md`,
`docs/architecture.md`, `commands.md`) may also use the identifiers, in
backticks.

The names **Design Settings**, **Recover Designs**, **Place Component**
and the **Component** design-policy preset follow these rules. The policy's
stored id remains `part`, and the placement command remains
`insert.mount_part`; saved designs, scripts and shortcuts keep working.

Pattern duplicates are **copies**, including the copies omitted from a
pattern. A copy is not necessarily an occurrence: a pattern may copy faces,
bodies or sketch geometry. Use **occurrence** only for a component placement.

Developer examples and diagnostic log keys may retain uppercase identifiers
or historical names. User-facing descriptions use the control's current
label. **Model parameters** remains the name of feature-owned parameters;
**document** is retained for an external format's own document and for general
PDF or text documents.
