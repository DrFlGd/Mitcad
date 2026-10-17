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
  "categories": [{"id": "screws", "name": "Screws"}, {"id": "nuts", "name": "Nuts"}],
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
| `categories` | `id` and `name` of the groups components belong to |
| `components` | `id`, `path` (a `.mitcad` file), `name` (required); `category`, `standard`, `description`, `keywords` (or `tags`), `preview` (a PNG), `license` (instead of the library's), `designation`, `supplier`, `part_number`, `supplier_url`, `build` |

`supplier` (who sells the part, `Motedis`), `part_number` (the
supplier's article number) and `supplier_url` (the product page, https
only; a configuration row's own `part_number` and `supplier_url` come
first, as a sheet sold in widths has a number and a page per width) are shown where
the part is chosen and listed: Insert from Library, Library Parts, the
parts list and the BUILD tab's cut list. `build` makes the component a
part of the BUILD tab ([below](#build-parts)).

`designation` is how a bill of materials names a part: `{standard}`,
`{name}`, `{config}` and the selectors' names in braces (`{Size}`) are
replaced; without it `{standard} {config}`. A configuration row's own
`designation` comes first.

Unknown fields are ignored, so newer libraries stay readable.

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
  feature of the design named `mount:<name>` (`mount:slot_x+_1`). A frame
  is where another part mates; its x axis runs along the slot or rail,
  its z axis as below. `name`: letters, digits, `_`, `-`, `+` and
  `.`; `kind`: `end`, `slot`, `bore`, `face`, `rail`, `shaft`, `hole`;
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
  simplified for assemblies (no threads, chamfers or fillets). Each names
  CC0-1.0 as its own licence;
- the BUILD tab's slot profiles, connectors, nuts, caps, panels and
  linear parts (mitcad#375), made parametric from the supplier's CAD
  files; every part names its `supplier` and `part_number`.

Its categories are those of both (screws, nuts, washers, profiles,
connectors, caps, panels, linear, …). `LICENSE` says which licence
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
    --out <clone of the parts repository> [--groups <folder>] [--version 2.0.0] [--work <folder>]
    [--no-render] [--commit]
tools/libraries/make-fastener-library.py --cli <mitcad-cli> --out <folder> [--sizes M3,M5]
    [--standards iso4762,iso4032] [--version 1.0.0] [--merge] [--commit]
tools/libraries/make-profile-library.py --cli <mitcad-cli> --source <folder of the supplier's files>
    --out <folder> [--groups <folder>] [--only id,id] [--version 1.0.0] [--work <folder>]
    [--no-render] [--merge] [--commit]
tools/libraries/profiles/*.json       the BUILD parts, one file per group
```

- `make-parts-library.py` makes the library: the fasteners, then the
  BUILD parts into the same manifest (`--merge`), then one `LICENSE`,
  `README.md` and `SOURCES.md`, and checks the folder. `--out` is a clone
  of the parts repository, so that its versions stay: `--commit` records
  the result on top of them and tags it (`v2.0.0` by default).
- `make-fastener-library.py` builds the fasteners from the standards'
  dimension tables, with preview images drawn from the dimensions;
  `make-profile-library.py` the BUILD parts (below). Each makes a library
  `mitcad-parts` of its own parts on its own (their tests do);
  with `--merge` each keeps the other's parts in the manifest at
  `--out`.

### The BUILD parts

`make-profile-library.py` makes them from the group files.


The supplier's files are not part of Mitcad; `--source` points at a copy
of them, and only the designs made from them go into the library. A
group file is a JSON array of parts (the generator also reads
`{"categories": [{"id", "name"}], "parts": [...]}`, which names the
categories for the manifest); the files are read in the order of their
names. Each entry is one library component:

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
  `model`), `supplier`, `part_number`, `supplier_url`, `designation`,
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
  - `model` (no source: sheets and plates sold without a CAD file):
    `dimensions`: `shape` (`plate`, `angle` or `none`), its sizes (each a
    parameter named in `parameters`, or a value, which becomes a user
    parameter `W`, `T`, `L`, `A` or `B`) and `parameters` (`{name:
    default}`: user parameters, such as a sheet's `W` its rows set). A
    `plate` (`width`, `thickness`, `length`) is centred on x = 0 across
    its width and runs from y = 0 to its thickness and from z = 0 to its
    length. An `angle` (`legs` [a, b], `width`, `thickness`) is centred
    on x = 0 across its width, its first leg in the XZ plane up to z = a
    (its thickness along +Y), its second in the XY plane up to y = b (its
    thickness along +Z). `commands` (optional, Mitcad commands as
    `mitcad-cli run` takes them) add anything more.
- `rows`: configuration rows (`name`, `select`, `values`, `part_number`,
  `supplier_url`, `designation`) for parts sold in sizes; `values` may be
  empty (one solid for every thread size, a rail's stock lengths that
  only carry their part numbers). The row's `part_number`,
  `supplier_url` and `designation` go into the design's table. `default`
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
  for a section.
- The designs hold their features in the root, as a part: no components,
  occurrences, active component or root policy (each starts from a blank
  design `mitcad-cli` makes, not from MCP's `document_create`, whose root
  is an assembly; a design that has them is left out). Their timelines: a
  section part has `Section` (the sketch) and
  `Profile` (the extrusion), and with a pitch `Hole plane <n>`, `Hole
  <n>` and `Holes at the pitch`; a solid its base feature and `Placed` (the
  move, with `place`); a model its boxes and `Centred`. Then the joint
  origins: `mount:end_a`, `End B plane` and `mount:end_b`, then the
  others in their order, one that follows a parameter after its plane
  (`Plane of <name>`). The configuration table comes last.
- The generator imports a source with `mitcad-cli import` and models
  with `mitcad-cli mcp --workspace` (commands and the queries it needs
  answers to: the end face, the section's curves, its regions) in a
  temporary folder in `--work` (default the folder above `--out`; not in
  a Mitcad project). It draws a section's preview (the section seen from
  end B) and renders the others with `mitcad-cli render` (`--no-render`,
  or a render that fails: their bounds drawn as a box), writes the
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
  pitch (its holes counted at three lengths by the volume), and a panel
  and an angle modelled from their dimensions.

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
