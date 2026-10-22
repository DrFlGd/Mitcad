# Component libraries

Mitcad's component libraries (fasteners and other standard parts) and the
community library are plain git repositories. No server is needed: a
library is published by pushing it to any git host (or a shared folder),
and a community index is just another repository that lists libraries.
This page documents the formats, so that anyone can make and publish a
library; the application's side is in the
[user guide](user-guide.md#component-libraries) and the commands in
[commands.md](../core/model/src/api/commands.md#component-libraries).

## A library repository

```
my-parts/                        a git repository, also a Mitcad project
  mitcad-library.json            the manifest (required)
  LICENSE                        the licence's text or a note naming it
  README.md
  SOURCES.md                     where data such as dimension tables came from
  .mitcad/project.json           Mitcad's project marker
  screws/iso4762.mitcad          the components' designs
  previews/iso4762.png           preview images (PNG, at most 512 KB)
```

The designs are ordinary Mitcad designs (`.mitcad` files). A design may have a
configuration table (below) so that one design stands for a family of
sizes. A library holds data only: nothing in it is run, symbolic links
are refused, and paths stay inside the repository.

### `mitcad-library.json`

```json
{
  "format": "mitcad-library",
  "version": 1,
  "id": "mitcad-parts",
  "name": "Mitcad parts",
  "description": "ISO metric screws, nuts and washers, and the BUILD tab's parts.",
  "library_version": "2.0.0",
  "license": "MIT",
  "authors": ["Mitcad contributors"],
  "homepage": "",
  "sources": ["ISO 4762", "ISO 4032", "Tabulated for this library; see SOURCES.md"],
  "units": "mm",
  "categories": [{"id": "screws", "name": "General fasteners/Screws"},
                 {"id": "nuts", "name": "General fasteners/Nuts"}],
  "components": [
    {"id": "iso4762", "path": "screws/iso4762.mitcad", "category": "screws",
     "name": "Hexagon socket head cap screw", "standard": "ISO 4762",
     "keywords": ["socket head", "cap screw", "DIN 912"],
     "preview": "previews/iso4762.png", "license": "CC0-1.0", "designation": "ISO 4762 {Size}x{Length}",
     "description": "36 sizes, M3x5 to M12x120."}
  ]
}
```

| Field | Meaning |
|---|---|
| `format`, `version` | `mitcad-library`, 1 |
| `id` | 2 to 64 of `a-z`, `0-9` and `-`; the same for every version |
| `name`, `description` | for people |
| `library_version` | the library's own version, matching its tag (`v1.0.0`) |
| `license` | an SPDX licence expression for every component without its own |
| `authors`, `homepage`, `sources` | for the attribution and the data's origin |
| `units` | `mm` |
| `categories` | `id` and `name` of the groups components belong to, and optionally `parent` ([Groups](#groups)) |
| `components` | `id`, `path` (a `.mitcad` file), `name` (required); `category` (one id or a list, [Groups](#groups)), `categories`, `standard`, `description`, `keywords` (or `tags`), `preview` (a PNG), `row_previews`, `license` (instead of the library's), `designation`, `supplier`, `part_number`, `supplier_url`, `build`, `mounts` ([Mount frames](#mount-frames)) |

`supplier` (who sells the part, `Motedis`), `part_number` (the
supplier's article number) and `supplier_url` (the product page, https
only; a configuration row's own `part_number` and `supplier_url` come
first, as a sheet sold in widths has a number and a page per width) are shown where
the part is chosen and listed: Insert from Library, Library Parts, the
parts list and the BUILD tab's cut list. `build` makes the component a
part of the BUILD tab ([below](#build-parts)).

`row_previews` gives single configuration rows a picture of their own,
`{"<row name>": "previews/<id>-<row>.png"}` (mitcad#484: each timber size
shows its section from its end); a row without one shows `preview`. The
BUILD tab's pickers list every row as an entry of its own and show a row's picture in
its tile (`library_preview` with `config`). `library check` checks that
every key names a configuration row of the design and every path a PNG
image of the folder.

`designation` is how a bill of materials names a part: `{standard}`,
`{name}`, `{config}` and the selectors' names in braces (`{Size}`) are
replaced; without it `{standard} {config}`. A configuration row's own
`designation` comes first.

Unknown fields are ignored, so newer libraries stay readable.

### Groups

The categories make the tree of groups that Insert from Library and the
BUILD tab's part picker show (mitcad#527):

```json
"categories": [
  {"id": "wood", "name": "Wood"},
  {"id": "dimensioned-timber", "name": "Dimensioned timber", "parent": "wood"},
  {"id": "sheets", "name": "Wood/Sheets"},
  {"id": "slot-screws", "name": "Slot profile/Screws"},
  {"id": "general-screws", "name": "General fasteners/Screws"}],
"components": [
  {"id": "hammer-screw-b8", "path": "slot-screws/hammer-screw-b8.mitcad", "name": "Hammer-head screw B8",
   "category": ["slot-screws", "general-screws"]}]
```

- A category's `parent` is another category's `id` in the same library;
  its `name` may be a path of names, `Wood/Sheets` (the names between the
  slashes, trimmed). Both together put the path under the parent. A
  category with neither is a group at the top, so libraries made before
  the tree, and those of one's own, show as they did.
- Groups are known by their names: the `Wood` of two libraries is one
  group, whether one names it by a parent and the other by a path, and
  in any case (the first spelling shows). A group lists its own parts and
  those of its subgroups.
- A component's `category` is one id or a list of them (or `category`
  and a list `categories`): the first is its home, the folder `library
  add` puts it in and the group a view of one group shows it in; it is
  listed in each of the others too. It stays one part: one tile with one
  star and one count of uses, once in Favourites and In this design.
- The groups show in the order of the libraries' `categories` (a group
  at the place of the first category that names it), then by name; a
  category the list lacks is a group of its id at the top, after them.
- `library check` warns of a `parent` that is no category of the
  library (the group is shown at the top) and of categories that are each
  other's parents, a cycle (the path is cut where it closes), and of a
  component's category the list lacks. These are warnings: the library
  stays usable.
- `library_show` and `library_search` give each component its
  `category` (the home), `categories` (every one, the home first) and
  `groups`: per category `{"category", "path": [names from the top],
  "order": [each name's place]}`. `library_search`'s `category` finds a
  component by any of its categories. `library_add` takes `category` as
  one id or a list, and for a new home category `category_name` (a path
  name) and `category_parent`.

### Configuration tables

A design's top-level `configurations` (the `set_configurations` command
writes it):

```json
"configurations": {
  "selectors": ["Size", "Length"],
  "parameters": ["d", "dk", "k", "s", "t", "L"],
  "default": "M5x16",
  "rows": [
    {"name": "M5x16", "select": {"Size": "M5", "Length": "16"},
     "values": {"d": "5 mm", "dk": "8.5 mm", "k": "5 mm", "s": "4 mm", "t": "2.5 mm", "L": "16 mm"}}
  ]
}
```

Each row sets the expressions of the table's parameters (parameters of the
design, usually user parameters that its features use). Selectors are the
cascaded choices the application shows (Size, then Length); each row's
combination is unique. Applying a row only changes parameter
expressions, as Change Parameters does.

### Mount frames

Any component may say where it mounts (mitcad#525): a light fitting on a
ceiling or a wall, a screw in a hole, a bin on the cells of a baseplate.
Its design marks a Joint Origin as a **mount frame** (Joint Origin's
Mount Frame check, [app/COMMANDS.md](../app/COMMANDS.md#joints); the
`mount` of a `joint_origin`,
[commands.md](../core/model/src/api/commands.md#mount-frames)): the
frame's name, what it mounts on and how the part may move once placed.
A joint origin named `mount:<name>`, as BUILD's parts name theirs
([below](#build-parts)), is a face frame of that name, fixed.

| `kind` | The frame | Placed on | `joint` (how it moves) |
|---|---|---|---|
| `face` | on the face the part lies with, z out of the part | a flat face or a plane, where clicked, face to face | `rigid` (fixed), `planar` (free on the face: slides and turns), `revolute` (turns) |
| `shaft` | on a screw's or pin's axis at the face that bears on the hole's rim (under a head), z along the shank towards the tip | a hole: its round face or rim, at the end towards the viewer, coaxial | `rigid`, `revolute`, `cylindrical` (turns and slides), `slider` (slides along the axis) |
| `hole` | on the axis of the part's hole, z out of the face that goes on | a shaft: its round face or end rim | as `shaft` |
| `grid` | at a corner of a grid of cells on the part's face, x and y along the cells, z out of the part; `pitch` (two lengths) and `cells` (two counts) | (other parts stand on it) | `rigid` |
| `foot` | in the middle of the face a part stands on a grid with, z out of the part; `footprint` (the cells it covers along its x and y) and optionally `pitch` (the grid it fits) | a grid's cells: those under the click, its footprint turned in quarter turns | `rigid` |

**Publishing writes them.** `library_add` (Publish to Library, `mitcad-cli
library add`) writes the design's mount frames into the component's
`mounts` (unless the command gives them, and not for a BUILD part, whose
frames are `build.frames`); the design stays what counts, the block tells
the application before it reads the design:

```json
{"id": "bin1x1", "path": "bins/bin1x1.mitcad", "name": "Bin 1x1",
 "mounts": [{"name": "foot", "kind": "foot", "joint": "rigid", "footprint": [1, 1], "pitch": [42, 42]}]}
{"id": "baseplate", "path": "baseplates/baseplate.mitcad", "name": "Baseplate 3x2",
 "mounts": [{"name": "grid", "kind": "grid", "joint": "rigid", "pitch": [42, 42], "cells": [3, 2]}]}
{"id": "lamp", "path": "lamps/lamp.mitcad", "name": "Ceiling lamp",
 "mounts": [{"name": "ceiling", "kind": "face", "joint": "planar"},
            {"name": "wall", "kind": "face", "joint": "rigid"}]}
```

A length is a number in mm or text with a unit, as in BUILD's blocks.
`library check` checks the block: names (letters, digits, `_`, `-`, `+`,
`.`) each once, a known kind, a joint the kind takes, a grid's pitch and
cells and a foot's footprint (counts of 1 or more), no cells or footprint
on other kinds; and the design: every listed frame is a mount frame of
the design that says the same (kind, joint, cells, footprint, pitch),
every mount of the design is a sound one, and a mount frame of the design
the block leaves out is a warning.

**Placing a part.** Insert from Library drops a part in as it always did
(at the origin, to be joined later with Joint); a part's mount frames
then are joint origins of its component, ready for Joint. A part with
mount frames opens Place Part instead, where a click on a face, a hole or
a grid snaps the chosen frame to it and makes the joint the frame asks for
(Enter without a click still drops it in;
[app/COMMANDS.md](../app/COMMANDS.md#component-libraries)). On a grid the
foot goes on the cells under the click, its footprint turned with Rotate
in quarter turns; a cell another part stands on is refused, with that
part's name.

**Gridfinity** (an open system of bins on baseplates, 42 mm cells and
7 mm height units) is made this way: a baseplate's top with a `grid`
frame at the corner of its first cell (`pitch` [42, 42], `cells` its
size), each bin's bottom with a `foot` frame in its middle (`footprint` its
size in cells, `pitch` [42, 42]). The tests use synthetic parts of those
sizes; ready-made Gridfinity models are third-party files and stay out of
Mitcad's repositories.

### BUILD parts

A component with a `build` block is a part the BUILD tab places
(mitcad#375): a slot profile, a connector, a nut, a panel or a linear
part. The design stays an ordinary design; `build` says how it is cut,
where it mounts and what it fits. Its rules, checked by `library check`:

```json
{"id": "p30x30-b8", "path": "profiles/p30x30-b8.mitcad", "name": "Profile 30x30 B-type slot 8",
 "category": "profiles", "supplier": "Motedis", "part_number": "P30_N8_B",
 "supplier_url": "https://www.motedis.com/en/Profile-30x30-B-Type-Slot-8",
 "designation": "Profile 30x30 B8 L={L}",
 "build": {
   "role": "profile",
   "system": {"type": "B", "slot": 8, "grid": 30},
   "length": {"parameter": "L", "min": "10 mm", "max": "6000 mm", "stock": ["6000 mm"]},
   "section": {"width": "30 mm", "height": "30 mm"},
   "slot": {"depth": "9 mm", "opening": "8.2 mm", "inner_width": "16.5 mm"},
   "mass_per_metre": "0.9 kg",
   "frames": [
     {"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
     {"name": "core", "kind": "bore", "diameter": "6.8 mm"},
     {"name": "slot_x+_1", "kind": "slot"}, {"name": "slot_x-_1", "kind": "slot"},
     {"name": "slot_y+_1", "kind": "slot"}, {"name": "slot_y-_1", "kind": "slot"}]}}
```

A value is a number (mm, kg) or text with a unit: `"9 mm"`, `"6 m"`,
`"0.9 kg"`, `"900 g"`. Unknown fields are kept and ignored.

- `role`: `profile`, `connector`, `fastener`, `panel`, `linear` or
  `cap`. Every role but `fastener` has `frames`.
- `system`: `type` (`I` or `B`), `slot` (the slot width in mm: 5, 6, 8,
  10) and `grid` (the series: 20, 30, 40, 45, …). A connector, nut or cap
  lists the systems it fits as `systems` (an array of the same; a system
  without `grid` fits every series of its type and slot). A profile has
  `system`; a connector and a cap have `systems`.
- `length` (cut-to-length parts: profiles, rails, shafts, screws,
  racks, cut plates): the design's user parameter that sets the length
  (`parameter`), its limits (`min`, `max`), the stock lengths it is sold
  in (`stock`) and optionally the length the design has (`default`).
  `pitch` (rails, racks): `{"distance": "60 mm", "first": "20 mm"}`, the
  hole spacing kept from end A: the design's holes are a pattern from
  `first` at `distance`, as many as fit its length. A profile has `length`.
- Profiles: `section` (outer size), `slot` (depth from the face,
  opening, inner width, used to size panels and fit nuts), and
  `mass_per_metre`.
- `frames` names the design's **mount frames**: each is a `joint_origin`
  feature of the design named `mount:<name>` (`mount:slot_x+_1`), or one
  with a `mount` of that name ([Mount frames](#mount-frames)). A frame
  is where another part mates; its x axis runs along the slot or rail,
  its z axis as below. `name`: letters, digits, `_`, `-`, `+` and
  `.`; `kind`: `end`, `slot`, `bore`, `face`, `rail`, `shaft`, `hole`,
  `tongue`, `groove` (a profiled board's edges, [Timber parts](#timber-parts));
  `accepts` (optional) lists the roles that may mate there
  (`["connector", "panel", "linear"]`, default anything that fits the
  system); `diameter` (optional) a bore's or hole's.
- Caps carry `section` (`width`, `height`) to match their profile's.
  Nuts (role `fastener`) list `systems` without `grid` (a nut fits every
  series of its slot) and have one `slot` frame.

**Which way a frame's z points.** Two frames that mate face each other:
their z axes are opposite, so every joint BUILD makes between them is
flipped ([Joints BUILD makes](../core/model/src/api/commands.md#placement)).

- A profile's `end_a`, `end_b` and slot frames, a bracket's or plate's
  `face` frames, a cap's `end` frame, a carriage's `rail` frame and a
  rail's `rail` frame point out of their part, towards what mates there.
- A part that sits inside a slot (a nut, an inner bracket, a slot-insert
  arm of a connector) points into the profile: opposite the slot frame it
  mates with, as a bracket's face frame does.
- A `bore` or `shaft` frame lies on its axis; a profile's `core` points
  out of end A, a bearing's or nut's bore along the axis the part turns
  or slides about.

**Profile designs** follow one layout so that every profile is placed and
cut the same way: the cross-section's sketch lies on the XY plane with
the profile's centre at the origin, one extrusion of `L` along +Z makes
the body, end A is at z = 0 and end B at z = L. Slot frames sit on the
face at the slot's centre line at z = 0, named `slot_<side>_<n>` with
`side` the outward direction (`x+`, `x-`, `y+`, `y-`) and `n` counting
the slots of that side from the side's lower coordinate (1 for a square
profile; 1 and 2 on the long sides of a 30x60). A closed side has no
frame. `end_a` points along −Z at the origin, `end_b` along +Z at z = L,
both with x along +X; `core` is the central bore's axis at z = 0,
pointing out of end A like `end_a`. A profile without a central bore
(a 40x80 with two) names its bores `core_1`, `core_2`, … from the lower
y (then x). `end_b` follows the length: its joint origin lies on a
construction plane offset from XY by `L`, while `end_a` is XY turned over
and the slots' frames are fixed planes at z = 0.

**Connectors** add `connector`:

```json
"connector": {
  "junctions": ["L", "T"],
  "space": "inner_corner",
  "mates": [{"frame": "mount_a", "to": "slot"}, {"frame": "mount_b", "to": "slot"}],
  "fasteners": [{"frame": "mount_a", "nut": {"library": "mitcad-parts", "component": "tnut-b8", "config": "M6"},
                 "screw": {"library": "mitcad-parts", "component": "iso4762", "config": "M6x12"}}],
  "machining": [{"on": "b", "frame": "end", "hole": {"diameter": "6.8 mm", "depth": "15 mm", "thread": "M8"}}]}
```

- `junctions`: where it can go: `L` (a corner), `T` (an end on a side),
  `cross`, `inline` (end to end on one axis), `surface` (on a face,
  panels' clamps and plates).
- `space`: `inner_corner` (inside the angle of an L or T), `outer`
  (on the outside faces), `end` (in the profile's end, as an automatic
  connector), `through` (through both, as a cube connector's arms).
- `mates`: which of its frames mate with which frame kind of the two
  profiles (`a` the one whose end meets, `b` the other).
- `fasteners`: the nut and screw (and optionally `washer`) each mate
  brings, as library references (configuration rows by name).
- `machining`: what the connector needs cut into a profile (`on` `a` or
  `b`, at its `end` or `slot` frame): a hole with `diameter`, `depth` and
  an optional `thread`.
- `envelope` (optional): `{"x", "y", "z"}`, the room an `inner_corner`
  connector takes; a junction whose inner corner is smaller does not take
  it.

`library_build_candidates` ([commands.md](../core/model/src/api/commands.md#library-commands))
offers a junction the connectors whose `junctions` hold its kind, whose
`systems` fit both profiles' and whose `envelope` fits its free room;
those wrong in one of these come apart as `refused`, each `{"id",
"name", "library", "reason"}` (greyed in the list). A connector that
names the profiles' series, and one made for one kind of junction only,
come first.

**Panels** add `panel`: `mount` (`slot`: in the slots of the frame
around it; `surface`: on the faces, held by clamps), `thickness` (a value,
or the parameter that sets it), `width` and `height` (the parameters that
size it; a fixed width, as a sheet sold in widths, is a configuration
row's value and not a parameter), `clearance` (left in the slot's depth,
default 0.5 mm), and `clamp` (a library reference to the clamp a surface
panel uses).

**Linear parts** add `linear`: `kind` (`rail`, `carriage`, `shaft`,
`bearing`, `screw`, `nut`, `pulley`, `gear`, `rack`, `belt`, `plate`),
and for a part that moves on another, `mate`: `{"frame": "rail", "on":
"rail", "joint": "slider", "axis": "x"}` (the joint kind and its axis, as
[joints](../core/model/src/api/commands.md#kinds-and-frames) have them).

- `size` (text or a number) is what parts that go together share: a
  carriage and its rail (`HGW15`), a bearing and its shaft (`8 mm`), a
  lead screw nut and its spindle (`Tr8x1.5`), a pulley and its belt
  (`GT2`). Sizes are the same when both are lengths of the same value
  (`8`, `"8 mm"`), or else the same text without case.
- Drive data: `lead` and `pitch` (screws; lengths), `module` (a length)
  and `teeth` (a count) for gears, racks and pulleys.
- A part whose `mate.on` is `slot` (a slider, a motor plate) goes on a
  profile's side as a profile does: on the slot of the side clicked, at
  the click's snaps (mitcad#448).
- A lead screw nut mates with a `cylindrical` joint for now (it slides
  and turns on its spindle). A rack is a solid of its length, not a
  cut-to-length part: its teeth cannot be made from an end section.

`library_build_candidates` and `library_search` take a picker's `kind`
and `size`: a carriage for a rail is `{"role": "linear", "kind":
"carriage", "size": "HGW15"}`; a part of another size comes apart with
the reason, and one without a size fits any.

`library check` checks these rules, and in the design: that the
parameters the block names (`length.parameter`, a panel's `thickness`,
`width`, `height`) are user parameters, that every listed frame is a
`mount:<name>` joint origin (an unlisted one is a warning), that the
rows' own `supplier_url`s are https addresses, and a profile's layout (an
extrusion by its length along +Z of a section on XY, `end_a`, `end_b`
and the slots' frames where the layout puts them; `end_b` on a plane
parallel to XY offset by the length). Library references must name a
component (and row) of the library itself, or of a fetched library; one
not fetched is a warning.

**Not expressible yet** (the parts are in the library with the nearest
form):

- A profile with a different system on one side (a 30x45 B-type slot 8
  with one slot-10 side): `system` is one for the whole profile.
- A junction of three profiles at a corner (a cube connector 3D, a corner
  connector 3D): junctions are of two profiles, so these list `L`.
- Slot frames on angled faces (the 45° sides of an R40/80): the layout
  names the `x+`, `x-`, `y+` and `y-` sides only, and automatic frames
  find only those.

### Timber parts

Timber construction (mitcad#478) uses BUILD parts of its own: members
(sawn, planed and dimensioned timber), wood screws and nails, brackets,
joist hangers and post bases fitted by the members' sections, and sheets
laid on a member's face. None of them has a slot system, and the slot
path does not offer them: the pickers list them only where timber is
asked for (below). How the BUILD tab places and lists them is in
[commands.md](../core/model/src/api/commands.md#build-timber-construction).

```json
{"id": "timber-dimensioned", "path": "dimensioned-timber/timber-dimensioned.mitcad", "name": "Dimensioned timber",
 "category": "dimensioned-timber", "license": "MIT", "designation": "Dimensioned timber {Thickness}x{Width} L={L}",
 "build": {
   "role": "profile", "kind": "timber",
   "material": {"species": "spruce", "finish": "dimensioned", "grade": "", "density": 450, "moisture": 20},
   "availability": "standard",
   "section": {"width": "98 mm", "height": "48 mm"}, "nominal": {"width": "98 mm", "height": "48 mm"},
   "tolerance": {"thickness": [-1.0, 1.0], "width": [-1.0, 1.0]},
   "length": {"parameter": "L", "min": 100, "max": 6000, "default": 3000,
              "stock": [2700, 3000, 3300, 3600, 3900, 4200, 4500, 4800, 5100, 5400]},
   "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
              {"name": "face_xp", "kind": "face"}, {"name": "face_xn", "kind": "face"},
              {"name": "face_yp", "kind": "face"}, {"name": "face_yn", "kind": "face"}],
   "rows": {"48x98": {"section": {"width": "98 mm", "height": "48 mm"}, "availability": "standard", "...": "..."},
            "48x248": {"section": {"width": "248 mm", "height": "48 mm"},
                       "nominal": {"width": "248 mm", "height": "48 mm"},
                       "availability": "less common", "material": {"species": "spruce", "density": 450},
                       "tolerance": {"thickness": [-1.0, 1.0], "width": [-1.5, 1.5]}}}}}
```

**Configuration rows' own blocks.** A part sold in sizes of different
data (a timber family's sizes differ in section, species and
availability; a wood screw's in length) gives each row a block of its own
in `build.rows`: `{"<row name>": {a partial build block}}`. Whoever reads
a row merges its block over the part's: objects key by key, other values
(numbers, texts, lists) replaced. The part's own fields are those of its
default row, so a reader that knows nothing of rows sees a whole part.
`library check` checks that every key names a configuration row of the
design and checks the rows' fields as the part's.

**Values.** A length or mass as elsewhere in the block; a `density` in
kg/m³ (a number, or text with the unit `kg/m3`), a `moisture` in %.

- **A member** (`role: "profile"`, `kind: "timber"`): the layout of a
  profile ([above](#build-parts)): the section on XY centred at the
  origin, its width W along X and its thickness T along Y, extruded by `L`
  along +Z. `section` `{width, height}` is the actual size (W, T),
  `nominal` the nominal one (at 20 % moisture; a re-sawn size is 2 mm
  narrower than its nominal width). `material`: `species` (`spruce`,
  `pine`, …), `finish` (`sawn`, `planed`, `dimensioned`), `grade` (`""`
  by default, `C24`, …), `density`, `moisture`. `availability`
  (`standard` or `less common`: the picker lists the standard sizes
  first). `tolerance` `{thickness, width}`, each `[minus, plus]` in mm.
  `length` as a profile's, its `stock` the lengths sold. Its frames are
  `end_a` and `end_b`, and a `face` frame on each side, `face_xp`,
  `face_xn`, `face_yp`, `face_yn` (the side facing +X, −X, +Y, −Y): at z =
  0 on the side's centre line, z out of the side and x along +Z, as a
  profile's slot frames; they follow the width and thickness of the row.
  No `system`, no `slot`, no slot frames and no `mass_per_metre`: the mass
  is the density times the volume.
- **A profiled board** (mitcad#477): a member as above whose section is
  shaped, with a `board` block (each row's own, through `build.rows`):

  ```json
  "board": {"profile": "tongue_and_groove", "cover": "112 mm", "area": 3123.5,
            "tongue": {"length": "8 mm", "thickness": "8 mm"}}
  ```

  `profile` is the shape (below); `cover` the width the board covers in a
  floor, wall or deck (left out: its width); `area` the section's area in
  mm², from which its mass and volume come; `tongue` `{length,
  thickness}`, `overlap`, `edge_radius` and `grooves` (a count) the sizes
  of its joint or edges. The section lies as a member's: its box centred
  on the origin, `section.width` the full width (over a tongue) along X,
  the thickness along Y, the visible face towards +Y. Its face frames are
  on the box's sides. A board that joins the next has two more frames,
  `groove` (kind `groove`) on the box's −X side with z −X, and `tongue`
  (kind `tongue`) the cover width on from it with z +X, both at y = 0,
  x along +Z: the edge with the tongue goes under the next board, so a
  board whose `groove` frame mates another's `tongue` lies beside it
  tongue into groove, and a row of them steps by the cover width. The
  profiles are Mitcad's own parametric sections (no manufacturer's
  drawing); their sizes and cover widths are those of public product
  lists:

  | `profile` | the section | joins the next |
  |---|---|---|
  | `tongue_and_groove` | a tongue `TL` long and `TT` thick on the +X edge, its centre `TO` from the board's middle (below it in a floorboard); a groove on the −X edge `GC` (1 mm) deeper and `GW` (0.5 mm) wider; panel boards with a V-joint, the face's edges chamfered `VB` wide at the angle `VA` to the face. Cover: W − TL | yes |
  | `shiplap` | rebated: a lip `LT` thick and `LW` wide on the back of the +X edge, the −X edge's front lapping over the lip of the board before. Cover: W − LW | yes |
  | `feather_edge` | a wedge from the thick edge `T` to the thin edge `TN`, overlapping the board under it by `O`; the section lies as the board is laid on a wall, turned by the angle at which its thick edge rests on the face of the board under it, so that the boards of a row, the cover width (`sqrt((W - O)² + v²)`, v the board's thickness `O` from its thin edge) apart, touch along that line only | yes |
  | `deck` | the four long edges rounded by `R`; grooved: `grooves` grooves `GG` wide and `GD` deep on the face, evenly over the width. Cover: its width | no: a deck keeps a gap |
- **A wood fastener** (`role: "fastener"`): `wood` `{kind: "screw" |
  "nail", head: "countersunk" | "flat" | "pan" | "washer", diameter,
  length}` (each row's diameter and length in its own block), `material`
  `{product, density}`, and the frame `axis` (kind `shaft`): on the axis at
  the head's bearing surface, z along the shank towards the tip, which is
  at z = `length`. A countersunk head lies under that surface (z > 0), the
  others on it (z < 0).
  Timber's generated fasteners seat by `wood.seat`: `head_top` (wood
  screws and nails by default), `head_underside` (pan, hex, washer, button
  and socket heads by default), or a named mount frame whose z points
  along the shaft. A `head_top` frame, when present, is exact even for an
  oblique source; an older axis-aligned source needs no new frame, as the
  body bounds supply its head top. `head_underside` uses the existing axis
  bearing frame unless the source lists a frame with that name. Selecting
  a row can override `seat`. A head behind the axis drives the tip deeper
  by its height; nominal length still refers to the axis-to-tip distance.
  Metal connector hole frames retain bearing placement. Generic non-wood
  bolts and washers use their existing connector frames unchanged.
  `wood.default: true` (mitcad#516; `library check`: true or false) marks
  the fastener Timber's frames and spacing sets, and a timber junction's
  Connector, start with where the user has not chosen one: the wood
  picker (`library_build_candidates` with `kind: wood`) lists it first.
  Mitcad's library marks **Wood screw, countersunk** (driven flush,
  `seat: head_top`), not the pan-head **Connector screw**, which bears on
  a metal connector's plate and would stand proud of bare wood. Without
  a mark the tools take the first screw driven flush (`seat: head_top`,
  or a countersunk or bugle head without a seat), then the first screw,
  then the first wood fastener, in the picker's order; a choice the user
  made stays the tool's while the application runs.

- **A bracket, joist hanger or post base** (`role: "connector"`):
  `connector.placement: "outer"` (on the members' outer faces; `space`
  `outer`), `junctions` as other connectors (`L`, `T`; a post base
  `surface`), and `fit` `{kind, section, width}`: `kind` `angle`, `hanger`
  or `post_base`; `section` `[a, b]`, the actual section of the member it
  carries (the joist, the post), either way round; `width` `[min, max]`,
  the range of that member's smaller dimension (a bracket). No `systems`.
  `mates`: its first frame, `mount`, places it, and the design is built in
  that frame's coordinates (the frame is the design's origin and axes):
  - a hanger: at the middle of the joist's end on the carrier's face, z
    into the carrier, x up the joist's section (the seat half the joist's
    height below, at −x);
  - an angle bracket: on the carrier's face at the middle of the edge
    where the chosen side of member a meets it, z into the carrier, x
    that side's outward normal (one arm on the carrier in the plane z = 0
    towards +x, one on member a in the plane x = 0 towards −z);
  - a post base: at the middle of the post's end, z into the post along
    its axis, x its +X (its base plate under it, at z < 0).

  `fasteners`: `[{"on": "a" | "b", "frames": [hole frames], "count": n,
  "screw" (or "nail"): {library reference with its row}}]`, one entry per
  member they go into (`a` the member whose end meets, the joist, the
  post; `b` the carrier). A hole frame (kind `hole`, with its `diameter`)
  lies on the connector's surface the fastener's head bears on, z out of
  it towards the head, so that it faces the fastener's `axis` frame.
- **A sheet** (`role: "panel"`): `panel.mount: "face"` (laid on a
  member's face), `thickness` (its parameter), `width` and `height` (the
  parameters the BUILD tab sets from the faces it covers), `stock`
  (`[[width, height], …]`, the sheet sizes sold, mm); `material`
  `{product: "plywood" | "osb" | "chipboard", species, finish, grade,
  density}`; thicknesses as rows. The design is a box from the origin:
  width along X, height along Y, thickness along +Z, its back on z = 0;
  its `face` frame at the origin, z +Z.

**Pickers.** `library_search` and `library_build_candidates` take `kind`
for timber too: `timber` (members), `wood` (wood fasteners), `outer`,
`angle`, `hanger`, `post_base` (outer connectors), and `mount` (`face`:
sheets). A timber part is listed only when it is asked for: `role:
"profile"` without `kind` (the Profile picker) lists no timber, `role:
"panel"` without `mount` no sheets, `role: "fastener"` without `kind` no
wood fasteners, and a slot system's picker or junction none of them. A
junction of two members (`build_junctions` gives it `wood`, with the
members' `sections`) is offered wood fasteners and the outer connectors
whose `junctions` hold its kind and whose `fit` holds member a's section
(within 1 mm) or smaller dimension, from the part or one of its rows;
one that does not is greyed with the reason (`it fits 48 x 148, the
member is 48 x 123`). `mitcad-cli library search` and `library
candidates` take `--kind` and `--mount`.

### Versions

A library's versions are its commits; released ones are tags
(`v1.0.0`). Mitcad lists the tags, newest first, and the default branch's
tip as "latest, unreleased". A design records, for each part, the commit
of the version the user chose; opening the design reads that commit even
after the library has moved on, and only Library Parts › Update changes
it. Keep old tags: designs refer to their commits.

## A community index

```
community-index/
  mitcad-index.json              {"format": "mitcad-index", "version": 1, "name": "..."}
  libraries/<id>.json            one entry per library
```

```json
{
  "format": "mitcad-index-entry", "version": 1,
  "id": "my-parts", "name": "My parts", "description": "...",
  "url": "https://example.org/you/my-parts.git",
  "homepage": "", "license": "CC-BY-4.0", "maintainers": ["You"],
  "tags": ["brackets"],
  "components": [{"id": "bracket-20", "name": "Bracket 20", "tags": ["corner"]}],
  "reviewed": [{"rev": "<40 hex digits>", "label": "v1.0.0", "date": "2026-10-05"}]
}
```

`components` lets a library be found before it is fetched; `reviewed`
lists the versions the index's maintainers looked at. An index only
lists: the libraries stay where their authors keep them, and an index's
checks (if it has any) only check entries. `mitcad-cli library
index-entry <folder> --url <url>` writes an entry for a library.

## Licences

Every item needs a licence; items without one are hidden unless the user
asks for them. A community index accepts these (not legal advice):

| SPDX | What users should know |
|---|---|
| CC0-1.0 | no conditions |
| CC-BY-4.0 | credit the authors when sharing designs that contain it |
| CC-BY-SA-4.0 | credit the authors; designs shared with it may need the same licence |
| MIT, BSD-2-Clause, BSD-3-Clause | keep the copyright and licence notice |
| Apache-2.0 | keep the copyright, licence and notice files |
| CERN-OHL-P-2.0, -W-2.0, -S-2.0 | permissive, weakly and strongly reciprocal hardware licences |

The licence, the library, its URL and the version are recorded with each
part in the design (also for a copied part), and Library Parts › Parts
List shows them.

## Making and publishing a library

In the application: Tools › Libraries › Publish to Library adds the open
design to a library folder of your own (made when new: its manifest, a
licence note and a git repository), with an image of the view as its
preview, records it as a version, pushes it to a remote you give, and
writes the entry for a community index. From the command line:

```
mitcad-cli library init my-parts --id my-parts --name "My parts" --license CC-BY-4.0 --author "You"
mitcad-cli library add my-parts bracket.mitcad --id bracket-20 --name "Bracket 20" --category brackets \
    --preview bracket.png
mitcad-cli library check my-parts
mitcad-cli version save my-parts/mitcad-library.json my-parts/brackets/bracket-20.mitcad ...
git -C my-parts tag v1.0.0
mitcad-cli remote add my-parts https://example.org/you/my-parts.git
mitcad-cli library index-entry my-parts --url https://example.org/you/my-parts.git
```

## Mitcad's parts library

`mitcad-parts` ("Mitcad parts", MIT) is the one library of the parts
Mitcad offers, kept in a repository of its own
(`https://github.com/mitcad/parts.git`):

- the metric fasteners: ISO 4762, ISO 4017 and ISO 10642 screws, ISO 4032
  nuts and ISO 7089 washers, M3 to M12 in their preferred lengths,
  simplified for assemblies (cosmetic threads, the heads' top edges
  rounded or chamfered, no other chamfers or fillets; [Previews](#previews)). Each names
  CC0-1.0 as its own licence;
- the BUILD tab's slot profiles, connectors, nuts, caps, panels and
  linear parts (mitcad#375), made parametric from the supplier's CAD
  files; every part names its `supplier` and, when it has one, its
  `part_number`. Parts the supplier sells without a CAD file are modelled
  from their dimensions: sheets and plates, the round precision shafts h6
  (6 to 25 mm), the trapezoidal lead screws (Tr8x1.5 to Tr16x4) and the
  threaded rods DIN 976 (M5 to M12), the screws and rods with their
  threads as cosmetic thread features (mitcad#449);
- the timber parts (mitcad#478), Mitcad's own in industry-standard sizes
  and naming no supplier: sawn, planed and dimensioned timber (one part
  each, its sizes as rows, the less common ones marked), wood screws,
  connector screws, ring shank nails and structural screws, angle
  brackets, joist hangers and post bases, and spruce and birch plywood,
  OSB and chipboard sheets ([Timber parts](#timber-parts); their group
  files' notes give the densities' grounds);
- the profiled boards (mitcad#477), Mitcad's own parametric sections in
  industry-standard sizes and cover widths, naming no supplier: tongue and
  groove floorboards and panel boards with a V-joint (category
  `tongue-and-groove`), shiplap and feather-edge cladding boards
  (`cladding`), and deck boards with rounded edges, smooth or grooved
  (`decking`); each one part with its sizes as rows.

Its categories are those of both, in six main groups ([Groups](#groups),
mitcad#527), in this order: **Wood** (sawn, planed and dimensioned
timber, tongue and groove, cladding and deck boards, sheets, wood
fasteners, timber connectors), **Slot profile** (profiles, connectors,
mounting plates, panels, caps, covers, tubes and spacers, accessories,
and the nuts and screws made for the slot: `slot-nuts`, `slot-screws`),
**Motion** (the linear parts: rails, carriages, shafts, lead screws and
threaded rods, belts, gears; ball bearings, motors), **General
fasteners** (the ISO screws, nuts and washers: `screws`, `nuts`,
`washers`), **Hardware** (handles and knobs, feet and base plates,
castors and wheels) and **Workshop**. `make-profile-library.py` names
each category under its main group (`MAIN_GROUPS`: `Wood/Dimensioned
timber`) and lists them in that order; `make-fastener-library.py` names
its own `General fasteners/…`. `LICENSE` says which licence
applies to what, `SOURCES.md` where both kinds of part come from.

**The fastener library before it.** Up to 1.0.0 the repository was the
fastener library, `mitcad-fasteners` (`https://github.com/mitcad/fasteners.git`,
which redirects to the parts repository); at 2.0.0 it became the parts
library, with the new id. Designs made with `mitcad-fasteners` keep
working: a part is read at the commit its design recorded, from any clone
that has that commit with the recorded id in its manifest there, so a
clone of the parts repository reads them under their old id and address.
Library Parts › Update moves such a part to the parts library's versions.

Three generators make it with `mitcad-cli`:

```
tools/libraries/make-parts-library.py --cli <mitcad-cli> --source <folder of the supplier's files>
    --out <clone of the parts repository> [--groups <folder>] [--version 2.3.0] [--work <folder>]
    [--render-worker <mitcad-render>] [--no-render] [--rerender] [--commit]
tools/libraries/make-fastener-library.py --cli <mitcad-cli> --out <folder> [--sizes M3,M5]
    [--standards iso4762,iso4032] [--version 1.0.0] [--render-worker <mitcad-render>] [--no-render]
    [--rerender] [--merge] [--commit]
tools/libraries/make-profile-library.py --cli <mitcad-cli> --source <folder of the supplier's files>
    --out <folder> [--groups <folder>] [--only id,id] [--version 1.0.0] [--work <folder>]
    [--render-worker <mitcad-render>] [--no-render] [--rerender] [--merge] [--commit]
tools/libraries/profiles/*.json       the BUILD parts, one file per group
tools/libraries/library_previews.py   the fasteners' studio and finishes (both generators use it)
```

- `make-parts-library.py` makes the library: the fasteners, then the
  BUILD parts into the same manifest (`--merge`), then one `LICENSE`,
  `README.md` and `SOURCES.md`, and checks the folder. `--out` is a clone
  of the parts repository, so that its versions stay: `--commit` records
  the result on top of them and tags it (`v2.3.0` by default, the next version to publish).
- `make-fastener-library.py` builds the fasteners from the standards'
  dimension tables, each with its finish as its appearance and a
  rendered preview ([Previews](#previews));
  `make-profile-library.py` the BUILD parts (below). Each makes a library
  `mitcad-parts` of its own parts on its own (their tests do);
  with `--merge` each keeps the other's parts in the manifest at
  `--out`.

### Previews

How the library's pictures are made (mitcad#526):

- **Fasteners** (the ISO screws, nuts and washers; the BUILD parts whose
  `build.role` is `fastener` or whose category is one of nuts, screws,
  washers, bolts, nails or fasteners, such as `nuts`, `screws` and
  `wood-fasteners`) are rendered with `mitcad-cli render`
  ([rendering.md](rendering.md#final-render)) in one studio setup,
  `STUDIO` in `tools/libraries/library_previews.py`: a copy of the design
  is turned so that the part's axis is Z with its head up (the axis along
  a long part's longest extent, across a flat one's thinnest, its head on
  the side its mass lies on; a group file's `"preview": {"up": [x, y,
  z]}` names it instead), and seen in perspective from three times its
  size away, from the front right and above (25 degrees for a long part,
  35 for a compact one, 50 for a nut or washer), a part at least 2.2
  times longer than wide lying across the tile. The render settings are
  the `studio` environment at twice its strength, three softboxes (area
  lights in the part's frame, sized by the part and powered so that each
  gives the same light on every part, each placed where the camera sees
  it mirrored: a broad key streak along the left of a shank and a head's
  side and a narrow rim streak near their right edge, from lights below
  the part, as a cylinder seen from above mirrors, and a small softbox
  mirrored in part of a head's top), no ground, the `neutral` view
  transform a quarter stop brighter, 64 samples denoised with Cycles'
  fixed seed and no time limit, 512 x 512 pixels with a transparent
  background; the generator lays the render over the tiles' background
  (sRGB 246, 247, 249, that of the drawn pictures), crops it to the part
  with 6 % around it and averages it down to 256 x 256 (an RGB PNG of a
  few tens of KB).
- **Finishes.** A fastener's surface is its `appearance`: the ISO socket
  screws (ISO 4762, ISO 10642) black oxide, the other ISO parts zinc
  plated steel; a BUILD fastener without an `appearance` gets one by what
  its name, part number or id says (`stainless`, `A2`, `A4`: stainless
  steel; `brass`; `black`, `oxide`: black oxide), else zinc plated steel.
  An `appearance` may name a finish: `zinc_plated` (the design's
  `zinc_plated`, a light silver, roughness 0.27), `stainless` (the
  design's `stainless_steel`, 0.22), `black_oxide` (the design's
  `black_oxide`, a dark grey metal, 0.22), `brass` (`brass_polished`);
  the designs keep it, so a placed part shows it too.
- **Threads and heads.** Every screw's design has a cosmetic thread
  feature on its shank (`modeled: false`: the `threads` query lists it,
  the view shows it, no geometry), whose cone chamfers the tip (the
  thread's `chamfer`, at the free end only: about the thread's depth at
  45 degrees), and its head's top edge rounded or chamfered, one feature
  each, so that assemblies stay light:

  | Screw | Thread | Head |
  |---|---|---|
  | ISO 4762 | ISO metric coarse, over b = 2d + 12 mm from the tip (the whole shank when shorter) | the top edge rounded, radius 0.06 dk |
  | ISO 10642 | ISO metric coarse, over b = 2d + 12 mm, at most the shank under the head | the top edge rounded, radius 0.03 dk |
  | ISO 4017 | ISO metric coarse, to the head | the hexagon's six top edges chamfered 0.06 s |
  | Wood screws, connector and structural screws | the nearest ISO 1478 forming screw thread (`ST4.8` for a 5 mm screw), a stand-in for the wood screw's own thread, which the thread feature has no profile of; over 60 % of the length from the tip (the connector screw to the head) | the head's outer edge rounded, radius 0.08 of its diameter |
  | The slot systems' screws (made from the supplier's solids) | ISO metric coarse of the size in the name (`M8x20`), on the solid's cylindrical faces of that diameter; a self-forming screw (`S12x30`) the metric coarse thread of its diameter as a stand-in | the supplier's |

  A design's thread has its default row's size: a configuration row sets
  parameters only, so another row's shank follows its size but keeps
  that designation. A feature that cannot be made (an edge or face not
  found) is left out with a warning; the part stays as it was. The tip's
  chamfer diameter is an expression of the row's own size (ISO: `d -
  1.182532 * P`, the minor diameter less a tenth of a pitch; wood screws
  `0.58 * D`), so that it follows the rows the designation does not. Nails,
  nuts and washers have none. The **previews** make the part's cosmetic
  thread modelled (`edit_feature` with `modeled: true`: the same faces,
  size, length and chamfer) on the rendered copy only, so that the
  picture shows the thread's profile; the generator prints `rendered
  without its thread (why)` when the copy cannot model it. With
  `--strict` (all three generators) a generator fails, after making the
  library, when a fastener's preview was drawn, rendered without its
  thread, or a thread or rounding was left out; the render-step test
  runs with it.
- **Only what changed is rendered.** A rendered PNG carries a `tEXt`
  chunk `Mitcad preview` with the key it was made from (a digest of the
  studio setup and the design file); a generator keeps the picture in
  `--out` whose key matches and renders the others. `--rerender` renders
  them all.
- **Without a render worker** (`--no-render`, or none found: `mitcad-cli
  render --list-devices` fails) the generators say so once and draw the
  pictures as before: an ISO fastener's outline from the side, drawn
  from its dimensions; a BUILD fastener's bounds as a box. A render that
  fails draws that part's picture with a warning naming it.
- **Other parts**: a section part's picture is its section seen from end
  B, drawn from its curves; a `board`'s rows are rendered from their ends
  (`row_previews`, below); the other BUILD parts are rendered from the
  iso view with the design's own render settings. Fasteners have no
  `row_previews`: their rows differ in size only.

The render worker is `--render-worker`, else `MITCAD_RENDER_WORKER`, else
`mitcad-render` next to `mitcad-cli` (a package); in a build folder it is
`app/mitcad-render` beside `tools/cli/mitcad-cli`. To rebuild the library
with rendered previews, from a build folder and a clone of the parts
repository (Windows; elsewhere the same without `.exe`):

```
python tools\libraries\make-parts-library.py --cli build\dev\tools\cli\mitcad-cli.exe
    --render-worker build\dev\app\mitcad-render.exe --source <folder of the supplier's files>
    --out <clone of the parts repository> --work <folder outside it>
```

It prints `preview <id>: rendered|kept|drawn (<finish>)` for every
fastener and ends with the library check; `--commit` records and tags
the result (publishing a version is the maintainers' step).

### The BUILD parts

`make-profile-library.py` makes them from the group files.


The supplier's files are not part of Mitcad; `--source` points at a copy
of them, and only the designs made from them go into the library. A
group file is a JSON array of parts (the generator also reads
`{"categories": [{"id", "name"}], "parts": [...], "left_out": [...]}`,
which names the categories for the manifest (under their main group,
[above](#mitcads-parts-library); a name with a slash is kept as it is)
and the supplier's files the
group makes no part of, each `{"source", "reason"}`: an assembly, a copy
of another part's file, a file that holds another product); the files are
read in the order of their names. Every supplier file is the `source` of
a part or of a row, or left out with its reason. Each entry is one
library component:

```json
{"id": "p30x30-b8", "name": "Profile 30x30 B-type slot 8", "category": "profiles",
 "source": "aluminium-profiles/Profile-30x30-B-Type-Slot-8/P30_N8_B.stp",
 "make": "section",
 "supplier": "Motedis", "part_number": "P30_N8_B",
 "supplier_url": "https://www.motedis.com/en/Profile-30x30-B-Type-Slot-8",
 "designation": "Profile 30x30 B8 L={L}",
 "rows": [],
 "build": {"role": "profile", "system": {"type": "B", "slot": 8, "grid": 30}, "...": "as above"},
 "frames": {"auto": "profile"}}
```

- `id`, `name`, `make` (required); `category` (the folder and category,
  default the role), `source` (a path in `--source`; required but for
  `model`, where it names the file the dimensions come from and is not
  imported), `supplier`, `part_number`, `supplier_url`, `designation`,
  `description`, `keywords`, `standard`, `license` go to the manifest as
  they are, `build` with what the generator fills in (below).
- `make`:
  - `section` (profiles and other cut-to-length parts: rails, shafts,
    belts, spindles): the cross-section at the source's end face, placed
    on XY at the centre of its bounds, as a sketch of fixed lines, arcs,
    circles and splines extruded by the length parameter along +Z.
    `place` `{"z": [0, 1, 0], "x": [0, 0, 1]}` names the source's
    directions that become the part's z (the profile's axis, along a
    model axis) and x; y is z × x. Without it the part runs along the
    source's longest extent (or `axis`: `x`, `y`, `z`), x and y along the
    source's next axes in turn (y and z for an x axis). The section is
    taken at the end the part's z starts from: the planar faces lowest
    along z that face −z (several are projected into one sketch, each
    its own region), found without assuming their area, and extruded
    along the part's whole length. A part with `build.length.pitch` (a
    rail) keeps its holes: the holes across it nearest end A in the
    source (circular edges whose axes are square to the part's, a
    counterbore's two diameters and depth among them, through or to
    their depth) become `hole` features at `first` from end A, and a
    rectangular pattern along Z at `distance` whose count is
    `max(1, floor((L - first) / distance) + 1)`, so that a shorter part
    keeps the holes that fit. A counterbore without depth (a chamfer's
    edge) is a plain hole. Without holes in the source, or with holes
    that cannot be placed (one that cuts nothing), the part is made
    without them, with a warning. The generator warns when the
    design's volume per length and the source's differ (holes or
    machining the design does not have). `place` may also turn the
    centred section about Z (`rotate`, degrees) and move it (`shift`, [x, y]). The length parameter
    (`build.length.parameter`, default `L`) is the source's length unless
    `build.length.default` gives another.
  - `solid` (connectors, nuts, caps, carriages, racks, …): the source's
    bodies as a base feature, in the source's coordinates, or moved by
    `place`: `{"z", "x"}` as for a section, `{"matrix": [[...], [...],
    [...]]}` (rows of a rotation and a translation), or `{"rotate":
    {"axis": "z" or [x, y, z], "angle": degrees}}`; then `"translate": [x,
    y, z]`.
  - `model` (no source: sheets, plates, shafts and screws sold without a
    CAD file): `dimensions`: `shape` (`plate`, `angle`, `rod` or `none`), its sizes (each a
    parameter named in `parameters`, or a value, which becomes a user
    parameter `W`, `T`, `L`, `A` or `B`) and `parameters` (`{name:
    default}`: user parameters, such as a sheet's `W` its rows set). A
    `plate` (`width`, `thickness`, `length`) is centred on x = 0 across
    its width and runs from y = 0 to its thickness and from z = 0 to its
    length. An `angle` (`legs` [a, b], `width`, `thickness`) is centred
    on x = 0 across its width, its first leg in the XZ plane up to z = a
    (its thickness along +Y), its second in the XY plane up to y = b (its
    thickness along +Z). A `rod` (`diameter`, `length`) is a cylinder on
    the Z axis from z = 0 to its length, the layout of a section part, so
    that its `end_a`, `end_b` and `shaft` frames are those of a shaft;
    with `thread` (a thread size as the [thread
    feature](../core/model/src/api/commands.md#thread) takes it:
    `standard`, `designation`, `class`, `right_handed`) a cosmetic thread
    over its whole side, which follows the length and which the `threads`
    query lists (`{"standard": "trapezoidal", "designation": "Tr8x1.5",
    "class": "7e"}` for a lead screw, `{"designation": "M8"}` for a
    threaded rod). `commands` (optional, Mitcad commands as
    `mitcad-cli run` takes them) add anything more.
    The [timber parts](#timber-parts)' shapes: a `board` (`width`,
    `thickness`, `length`) is a rectangle centred on XY, its width along
    X, sketched with its sides dimensioned by the parameters and
    extruded by the length along +Z (the profile layout); without
    `frames` it has `end_a`, `end_b` and the four face frames, which lie on
    planes offset by `W / 2` and `T / 2` and so follow a row's size. A
    `board` with a `profile` (mitcad#477: `tongue_and_groove`, with
    `"v_joint": true` for panel boards; `shiplap`; `feather_edge`; `deck`,
    with `"grooves": n` for a grooved face) is a profiled board ([Timber
    parts](#timber-parts)): its `width` and `thickness` are the parameters
    `W` and `T`, its profile's parameters (`TL`, `TT`, `TO`, `GC`, `GW`,
    `VB`, `VA`; `LW`, `LT`; `TN`, `O`; `R`, `GG`, `GD`) user parameters
    with defaults of their own that `parameters` may replace (an angle as
    `"45 deg"`), and a feather-edge board's turn, cover and box (`FA`,
    `COV`, `HX`, `HY`, `YC`) parameters derived from them. The section
    is a sketch of its points (and a deck board's corner arcs), each
    dimensioned from a fixed point at the origin by signed horizontal and
    vertical distances whose values are expressions of those parameters,
    so a row, or a changed tongue or radius, reshapes it; its face frames
    follow the box and its `groove` and `tongue` frames the cover width.
    The generator checks the design's volume against the area it computes
    for the `board` block. A
    `fastener` (`diameter`, `length`, `head_diameter`, `head_height`,
    `head`: `countersunk`, `flat`, `pan`, `washer`) is a shank on the Z
    axis from z = 0 to its length and a cylindrical head (under z = 0 but
    countersunk, in z > 0), with an `axis` frame at the origin. A `sheet`
    (`width`, `height`, `thickness`) is a box from the origin, with a
    `face` frame there. A `bracket` (`legs` [on the carrier, on the
    member], `width`, `thickness`, `hole`, `holes` `{"b": [[x, y], …],
    "a": [[depth, y], …]}`), a `hanger` (`width` and `height` of the
    joist, `hanger_height`, `depth`, `flange`, `thickness`, `hole`) and a
    `post_base` (`width` of the post, `height`, `plate`,
    `plate_thickness`, `side`, `thickness`, `hole`) are steel plates of
    those fixed sizes (boxes joined), built in their `mount` frame's
    coordinates, with their screw holes cut and made frames `hole_a_<n>`
    (into member a) and `hole_b_<n>` (into b). A frame's origin may name a
    parameter in its x or y too, when its z is along X or Y.
  - `sizes` (instead of `rows`, for a `board`, `fastener` or `sheet`):
    the rows made from a compact table. A board's `{"species": {name:
    density}, "species_default", "resawn": 2, "tolerance": {"thickness":
    [[up_to, minus, plus], …], "width": …}, "rows": [{"thickness",
    "widths": [...], "species", "availability", "resawn": true}]}` makes a
    row `<thickness>x<width>` (selectors Thickness and Width, values W and
    T) for each width, with its own block (section, nominal, availability,
    material, tolerance: the first rule whose `up_to` is the size or more,
    or null); a profiled board's rows also take `profile` (its profile's
    parameters, in `sizes` for every row and in a row for its widths) and
    `covers` (one per width: a tongue and groove board's or shiplap's cover
    width, which sets `TL` or `LW` to the width less it), every profile
    parameter a value of the row, and its block's `board` (profile, cover,
    area, joint) computed from them, sizes that make no section refused
    (a tongue over half the width, a groove outside the thickness, a
    V-joint reaching the groove, a radius over half the thickness, grooves
    that do not fit); a fastener's `{"rows": [{"diameter", "head": [diameter,
    height], "lengths": [...]}]}` a row `<d>x<length>` (Diameter, Length;
    D, L, DK, K; `wood.diameter` and `wood.length`); a sheet's `{"rows":
    [{"thickness", "availability"}]}` a row of each thickness (T).
  - A row's `build` (as `sizes` make them, or written) is the row's own
    block: the manifest's `build.rows`, and the part's block takes its
    default row's.
  - `density` (a part's or the group file's, kg/m³): a section's mass per
    metre is its volume per length times it (aluminium, 2.7 g/cm³, when
    neither names one); a part's `build.material.density` defaults to it.
  - `appearance`: a library appearance's id (`steel_satin`), a finish's
    name (`zinc_plated`, `stainless`, `black_oxide`, `brass`;
    [Previews](#previews); a fastener without one gets one), or one of the
    design's own `{"id", "name", "based_on", "base_color", "roughness",
    "texture", "size"}` whose texture the generator draws and embeds:
    `grain` (wood, the grain along the image's height), `strands` (OSB),
    `particles` (chipboard), repeated every `size` mm and projected as a
    box in the body's coordinates, so that a board's grain runs along its
    length and a sheet's along its height. Every body of the design takes
    it. A `board` with a `grain` gets a second appearance of its own,
    `<id>-end`, on the two caps of its extrusion: growth rings (a 400 mm
    tile, the pith off the section), which the members placed from it show
    on their ends (mitcad#484).
  - `data_source` (a part of no supplier and no source file): what its
    sizes are, for SOURCES.md (`industry-standard sizes of sawn timber`).
- `rows`: configuration rows (`name`, `select`, `values`, `part_number`,
  `supplier_url`, `designation`, `source`) for parts sold in sizes;
  `values` may be empty (one solid for every thread size, a rail's stock
  lengths that only carry their part numbers). The row's `part_number`,
  `supplier_url` and `designation` go into the design's table. A row's
  `source` is the supplier's file of that row's product (another thread,
  colour or stock length of the same part): the design is made from the
  part's `source` and stands for every row; SOURCES.md lists the row's
  file. `default`
  names the default row (the first when left out).
- `frames`: `{"auto": "profile"}` (the default for `section`) finds the
  ends, bores and slots of a profile from its section and names them by
  the layout above: a slot is a gap between the straight pieces of the
  outline on a side, a bore a circle inside it; `{"auto": "profile",
  "add": [...]}` adds frames to those; a frame found that `build.frames`
  does not list, or one listed and not found, is reported as a warning.
  Otherwise a list of `{"name", "kind", "origin", "z", "x"}` (in the
  part's coordinates after `place`; `x` defaults to +X, or +Z when `z`
  is along X or Y), with `diameter` and `accepts` as in the manifest.
  The origin is in mm; its z may instead name the length parameter
  (`[0, 0, "L"]` for `end_b`, `[10, 0, "L / 2"]`) when the frame's z is
  along ±Z: the frame then lies on a plane offset by it from XY (or from
  a plane through its x and y), so that it follows the length. `end_a`
  and `end_b` without an origin, in a part with a length, are XY turned
  over and the plane at the length. The manifest's `build.frames` are the
  frames made in the design, no more and no fewer (a warning names those
  the group file's `build.frames` listed that the design has not, and
  those it adds). `accepts`, `diameter` and `kind` of a frame listed
  in `build.frames` are kept for the frame of the same name.
- What the generator fills in: `build.frames`; for a profile
  `build.section` (the section's bounds), `build.slot.opening` (the
  narrowest slot found) and `build.mass_per_metre` (the design's
  volume per length in aluminium, 2.7 g/cm³), each only when the group file has none; `build.length.parameter`
  for a section; `build.rows` (the rows' own blocks) and the default
  row's fields; an outer connector's `fasteners` `frames` and `count`
  when an entry names only the member (`on`) they go into; a profiled
  board's `build.board` (its rows' and, without rows, the part's).
- The designs hold their features in the root, as a part: no components,
  occurrences, active component or root policy (each starts from a blank
  design `mitcad-cli` makes, not from MCP's `document_create`, whose root
  is an assembly; a design that has them is left out). Their timelines: a
  section part has `Section` (the sketch) and
  `Profile` (the extrusion), and with a pitch `Hole plane <n>`, `Hole
  <n>` and `Holes at the pitch`; a solid its base feature and `Placed` (the
  move, with `place`); a model its boxes and `Centred`, a rod its `Rod` and,
  with a thread, `Thread`. Then the joint
  origins: `mount:end_a`, `End B plane` and `mount:end_b`, then the
  others in their order, one that follows a parameter after its plane
  (`Plane of <name>`). The configuration table comes last.
- The generator imports a source with `mitcad-cli import` and models
  with `mitcad-cli mcp --workspace` (commands and the queries it needs
  answers to: the end face, the section's curves, its regions) in a
  temporary folder in `--work` (default the folder above `--out`; not in
  a Mitcad project). It draws a section's preview (the section seen from
  end B), renders a fastener's in the studio ([Previews](#previews)) and
  the others with `mitcad-cli render` (`--no-render`,
  or a render that fails: their bounds drawn as a box); a `board`'s rows
  each from beyond its end B, a piece 1.2 times its larger side long, so
  that the tiles show the sections (`row_previews`; the part's picture its
  default row's; mitcad#484), writes the
  manifest, an MIT `LICENSE`, `README.md` and `SOURCES.md` (the source
  file, supplier, part number and URL of each part), checks the folder
  with `library check` (errors stop it) and, with `--commit`, records and
  tags it (`v<version>`). A part it cannot make (a missing file, a
  source `mitcad-cli import` cannot read, no end face, a section with an
  ellipse, regions that do not make the end faces' areas, a body that
  fails) is reported by id on one line (`left out <id>: <reason>`, a
  program's failure with its exit status and last lines of output) and
  left out; the run fails only on a broken group file (not JSON, a part
  without `id`, `name` or a known `make`, an id used twice, a frame
  without a name, kind or numbers).
- Mitcad lists the library among its default libraries (Libraries…),
  as Mitcad's one library of parts; it is fetched when the user asks.
- Tests: ctest `cli.profile-library` (`tools/cli/profile-library-test.cmake`)
  runs the generator on two slot profiles (one turned to run along Y and
  placed back, with a frame at half its length) and a bracket the test
  makes and writes as STEP, a rail with two counterbored holes at a
  pitch (its holes counted at three lengths by the volume), a panel
  and an angle modelled from their dimensions, and a shaft and a Tr8x1.5
  lead screw modelled as rods (their length, end B and thread at two
  lengths), with a row's own file and a file left out in SOURCES.md.
  ctest `build.profiles` (`tools/cli/build-profiles-test.cmake`, on the
  generated library named by `MITCAD_PROFILES_LIBRARY`) places a profile
  of each system, linear parts on a frame, slot nuts in their M5 rows and
  a cover cap, and a precision shaft with its bearing and support and a
  lead screw with its nut. ctest `cli.timber-library`
  (`tools/cli/timber-library-test.cmake`) runs the generator on small
  timber group files (a board family with a re-sawn and a less common
  size, wood screws, a bracket, a hanger, a post base and a sheet): their
  manifest fields and rows' blocks, the board's section and face frames
  following a row, the fastener's head, the connectors' holes, the
  pickers; and profiled boards (tongue and groove with and without a
  V-joint, shiplap, feather-edge, a grooved deck board): their `board`
  blocks per row, their volumes and `tongue` and `groove` frames
  following a row and a changed parameter. ctest `build.timber`
  (`tools/cli/build-timber-test.cmake`, on `MITCAD_PROFILES_LIBRARY`)
  checks every timber part of the generated library: the sizes and their
  marking, sections, densities and masses of rows of each family, the
  fasteners' and connectors' frames, and the profiled boards' rows,
  covers, areas (the designs' volumes) and frames, a floorboard placed
  on another's tongue and a shiplap row stepping by its cover. ctest
  `cli.library-previews` (`tools/cli/library-previews-test.py`, mitcad#526)
  renders an ISO 4762 screw (black oxide, the fastener generator) and the
  countersunk wood screw of `timber-fasteners.json` (zinc plated, the
  profile generator) in the studio: each a 256 x 256 PNG within 512 KB,
  not blank, reading as metal: the zinc plated screw a light silvery grey
  with near-white highlights, the black oxide one a dark grey (only its
  thread grooves in deep shadow may be black) with visible highlights, neither flat (the
  lumas inside their edges); `library check` passes; a second run keeps the picture;
  without a worker the generator draws it. It is skipped where the render
  worker cannot render.

## Where libraries are kept

Fetched libraries are bare clones in the user's local data folder
(`libraries`; `MITCAD_LIBRARIES_DIR` overrides it), one per URL, made and
updated with the system's git only when the user asks (Libraries ›
Fetch, Update in the library browser, Library Parts › Check for Newer Versions, Get Library). Updates
add branches and tags without removing anything, and git's automatic
garbage collection is off there, so the versions designs use stay
readable. A design whose library is not on the computer keeps the bodies
saved with it and says so; Library Parts › Get Missing Libraries fetches
them after showing their addresses.
