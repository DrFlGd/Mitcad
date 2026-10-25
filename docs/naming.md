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
