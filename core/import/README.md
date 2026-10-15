 (by its `DesignDescription.json`, mitcad#229)# mitcad-import: .f3d and FreeCAD designs with their history

`core/import` replays a design's history as Mitcad features, checks each
step against the bodies stored in the file, and brings in what it cannot
replay as the stored bodies. Two imports live here:

- **.f3d** (`src/*.rs`): the design dump IR
  (`mitcad_f3d::design::ir::Dump`, [SCHEMA.md](SCHEMA.md)), checked
  against the file's ASM history.
- **FreeCAD .FCStd** (`src/freecad/`), checked against every feature's
  stored result.

Both are generic over the [`Kernel`](../model/src/kernel.rs); the tests run
them on the model's mock kernel. The JSON commands and report fields are in
`core/model/src/api/commands.md` (*.f3d import with the timeline*,
*FreeCAD import (.FCStd)*).

## .f3d import

Entry points:

- `mitcad-cli import-f3d part.f3d [--save part.mitcad] [--report
  report.json] [--design <doc>] [--dump dump.json] [--no-verify]
  [--no-fallback] [--no-compare] [--time-limit S] [--hang-limit S]
  [--threads N] [--references DIR]... [--no-references] [--json]`;
  `--bodies-only` imports the bodies without the history;
- the document command `{"cmd": "import_f3d", "path": …}` and
  `Document::import_f3d_timeline(path, json)`, implemented in
  `core/ffi/src/f3d_import.rs`;
- Rust: `mitcad_import::import_design(&mut doc, &dump, &mut geometry,
  &options)`.

### Modules

| Module | |
|---|---|
| `params.rs` | User and model parameters with the file's names and expressions, in dependency order. Expressions that do not evaluate become values; names Mitcad reserves get `_1`. A model parameter is adopted by the feature or sketch that made it (`Document::adopt_parameters`). |
| `sketch.rs` | Points, lines, circles, arcs (counter-clockwise), ellipses, splines (control points, weights, full knots), texts (style, flips, multi-line frame and alignment, text along or fitted to a path), constraints and dimensions by type. Ids are renumbered into Mitcad's one id space (points first). Positions are the file's stored solution; the sketch is accepted when Mitcad's solve keeps them (1e-4 mm), else retried with driven dimensions, then without constraints (`partial`). `sketch/groups.rs`: polygons, sketch patterns, offsets and concentric circle dimensions (*Sketch patterns, offsets and polygons* below). |
| `features.rs` | Sketch planes (origin planes; imported construction planes with the exact frame as `frame`; planar faces found geometrically; else a fixed construction plane), construction planes (from the entities the decoder reads, mitcad#178: offset, angle, tangent, mid plane, two edges, along a path; through three points, tangent at a point, offset through a point, mitcad#186; else offset from an origin plane or a face found by distance; each checked against the file's plane; else fixed), construction points (at a vertex, at the centre of a circle, where three planes meet) and axes (through two points, along an edge or sketch line, through a round face, where two planes meet) checked against where the file has them (mitcad#186; else fixed), extrude, revolve, fillet, chamfer. |
| `ops.rs` | Combine, mirror, circular and rectangular patterns, shell, offset faces, move, split body, hole, thread, replace face — from external dumps (SCHEMA.md §2), or from the history when the stream decoder gives no inputs (see below). |
| `sweeps.rs` | Sweeps, pipes and lofts (*Sweeps, pipes and lofts* below). |
| `components.rs` | Components and occurrences. |
| `joints.rs` | Joints, as-built joints, joint origins, rigid groups and ground items (*Joints* below). |
| `relationships.rs` | Assembly relationships as constraint sets, tangent relationships as tangent rows (*Assembly relationships* below). |
| `refs.rs` | Fingerprints resolved against the replay (bodies by volume and name, or by points on their edges, surface bodies by name or area, faces through `point_on_face`, edges by midpoint and length), planar faces, the edges a fillet consumed, the faces a replace face replaced, the final comparison. |
| `history.rs` | The bodies stored in the file (`StoredGeometry`: the ASM history states and the stored design) and matching of body sets (volume, area, centre of mass, face count). |
| `report.rs` | The import report (JSON and text). |
| `timeline_notes.rs` | The report's outcomes on the timeline (mitcad#170): import notes on fallbacks' base features, partial items and placeholders of items suppressed or failed in the file (*The outcomes on the timeline* below). |
| `suppressed.rs` | Items suppressed in the file as suppressed features with their definitions (mitcad#180, *Items suppressed in the file* below). |
| `references.rs` | Components of other documents filled from those documents' own imports, and the captured positions inside them (mitcad#196, *Components of other documents* below). |
| `reference_folder.rs` | Where the files of the documents a design refers to are: the reference folder next to it, folders and files given, an `.f3z` package's designs (by its `DesignDescription.json`, mitcad#229), the GUIDs the files' manifests list (mitcad#196). |
| `appearances.rs` | The file's appearances and physical materials on the replayed bodies and faces (*Appearances and materials* below, mitcad#228). |

### The history as the reference

`core/ffi` reads the ASM history of the `.smbh` blobs and rolls it back
operation by operation. A timeline item's `_f3d.result_no` is the number of
the ASM state its operation made. Each component's history counts its own
operations (two components both have an operation 3), so the number names
the operation of the blob of the component that owns the item
(`_f3d.component`, *Components*) when that blob has it; else (a feature
that changes another component's bodies) of the other blobs with that
number that no item of their own component names, those whose bodies it
changed first. A history blob belongs to the component whose items name
most of its operations (mitcad#156), where they name more of them than
another component's items do and more than of another blob; the other
blobs go by the bodies they share with a component's `.smb` blob (the
numbers count per component, so inserted parts, whose base features are
all their component's operation 3, name every such blob alike). (By the bodies
alone, a body moved into a new component, which is in that component's
`.smb` too, linked the root's history to the new component; then neither
history was its component's, every item of the design went into one
component, and the items of the other components missed their states.)
A component made from bodies has no result number of its own; its
operation is that of the base feature that put the bodies into the new
component's history (`_f3d.made_result_no`). The states of all blobs are
merged in timeline order into one sequence, oldest first:

- an operation no item names gives one state, before the blob's next named
  one;
- components inserted from other designs bring their own histories, which
  come last;
- without item numbers (external dumps) states are merged by their
  numbers;
- the last state is the stored design.

Matching rules:

- An item with a known state must give exactly that state (measures within
  1e-3). First the replay is brought to the state before it, by a base
  feature for what earlier items changed without being replayed.
- Operations no item names that come right before an item's own in its
  blob, after the operation of the item before, may be the item's own: a
  pattern makes one per copy and names the last (mitcad#156). Where the
  replay is at the state before them, the item is tried from there first,
  in half its time; else as above, from the state right before its own.
  Before, a base feature brought every such pattern to its last copy but
  one (149 base features in the corpora, most before patterns).
- An item whose operation the history holds but which left every body as
  it was, and which none of its definitions reproduces (each changes the
  bodies or fails; an item without a translation, such as a base feature
  that brings no body, stays as before), failed in the file: an extrusion
  that found no body to cut. It is
  skipped with a placeholder (`failed in the file: …`), and the bodies
  stay as the items before left them, with no base feature (mitcad#156);
  only a body that differs from the state by more than the geometric
  tolerance is brought to it (mitcad#285).
  The file stores no failure state of its own for it: the three state
  bytes of the item tail and its health object are those of a healthy
  item. An operation that changed a body (a feature that computed with a
  warning) is replayed as any other.
- An extrusion, fillet or chamfer that gives its known state only
  approximately (beyond 1e-5) tries up to eight more definitions and
  keeps the closest: a set of regions next to the right one, or the
  wrong reading of a two-distance chamfer or asymmetric fillet (the
  other reading is tried right after each definition; which face takes
  which distance is decoded for two-distance chamfers only, *Named
  inputs*), can come within 1e-3, and the
  difference grows in later items. A fillet or chamfer is replaced only
  by one that comes closer by a quarter at least: another edge set's
  rounding can come a little closer to a stored spline rounding by
  chance (4.0e-4 against 4.4e-4) and keep later items from building,
  while a right one came closer by half or more in every design looked
  at. A difference the bodies before it
  carried (from an earlier approximate item, such as a fillet stored as
  splines) is not the extrusion's own: one that comes within 1.5 times
  that difference of its state is taken as it is (no definition comes
  closer).
- When an extrusion, or copies the history guessed (patterns and mirrors
  without decoded inputs), still gives its state only approximately, a
  base feature right after it brings the bodies that differ from the
  state by more than the geometric tolerance (0.01 mm) to it (a warning
  names it): the item stays parametric, but its difference does not
  reach later items, where it keeps a later state (a split, a mirror, the
  stored design) from matching. A body within the tolerance, the item's
  own among them, stays as the replay made it (mitcad#285). Other
  approximations (fillets stored as splines) are carried on.
- An item that gives no state on bodies carrying such an approximation
  (beyond 1e-5 from the state before it) is tried again, in half the
  time, after a base feature brings them to that state exactly (a warning
  names it); an item without a state of its own tries the replay's
  current state that way too. So does a fillet or chamfer with a state
  of its own that failed on the replay's bodies for another reason than a
  result not in the history (OCCT failed or crashed, an edge was not
  found), even on bodies within 1e-5 of the state: every solid comes from
  the stored state, whose edges are split otherwise (an `.ipt` part's
  rounding that crashed OCCT on the replay's bodies built on the stored
  ones). Before, only items without a state of their own were tried on
  the stored state, so an `.ipt` part whose features came to know their
  states lost such a rounding.
- An item without a known state must equal one of the next states before
  the state of the next item that has one (that state is the later
  item's; so are the fallbacks tried for it): exactly (measures within
  1e-5 and the same face count), else the closest within 1e-3. As it may match any of them, a join is not taken when its
  result, or the state it matches, has less volume on as many bodies than
  the bodies before it, nor a cut when it has more (a join can come
  within the tolerance of a later fillet's state by chance).
- A combine's tools that the file says it consumed (`isKeepToolBodies`
  false) stay in the ASM history as they were (the design no longer has
  them; its components' body lists leave them out). Where every tool is
  found by the item that made it and the combine's state holds each tool
  unchanged (the same stored body as in the state before it, with the
  replay's measures of the tool), those stored bodies are left out of the
  history from that state on and out of the
  stored design (`Oracle::retire`, mitcad#96): the combine is checked
  with its tools consumed, and they do not come back with a later base
  feature or the final comparison. Before, only a combine keeping its
  tools gave such a state, and the import kept bodies the design does
  not have.
- The volume an item adds or removes must be the file's too (mitcad#121,
  `history::Change`): its measures within the tolerance relative to the
  bodies let a wrong rounding of a large body pass (a few per cent of a
  small change are below 1e-5 of the body). Its own difference is the
  smaller of its change's from the file's change (the item's state
  minus the state the bodies before it stand for) and its result's
  volume from the state's (the item may take away a difference the bodies
  carried), relative to the larger change; above 1 % the result is not
  taken (unless its faces settle it, below) and the next definition is
  tried (the note of an item that falls
  back says by how much its last one was off). A result that is the state
  exactly (the same faces, measures within 1e-5) and differences below
  1e-6 of the bodies' volume (the measures' noise) are not compared. The
  report gives the difference of each item taken (`change_difference`).
  The change is only as good as the measures: a through-all cut of two
  holes through a cylinder's wall was 2.6 % off (106.6 mm³) because the
  state's holes, bounded by cubics of 132 spans fitted to the
  intersection, were integrated with too few fixed Gauss points; every
  face is integrated span by span now (mitcad#139, mitcad#140,
  `docs/architecture.md`).
- Where the volumes do not settle an item (its own difference above
  0.5 %, `history::UNSETTLED`), its faces do (mitcad#138,
  `src/geometric.rs`): a wrong rounding (its surface 0.15 mm off along its
  whole length), differences at the corners of a rounding only and the
  file's spline approximations (every face within 1e-4 mm, a thin layer
  over the body) all come to a few per cent of a small change. The faces
  the result made or changed (`Kernel::new_faces`: the faces of the
  changed bodies that the bodies before the item do not have, also as the
  same surface, area and centre) are measured against the state's faces,
  and the faces the file's operation made or changed (the state's against
  the state before it) against the result, at up to 16 points spread over
  each (`Kernel::boundary_distances`; 3000 a side at most, so a pattern's
  thousands of faces get fewer each). A face is off when a point lies
  farther than the geometric tolerance (`geometric::tolerance`, the
  option `geometric_tolerance`, 0.01 mm by default; mitcad#224). The result is taken when
  every face off on either side is a corner patch, at most 2 % of that
  side's new area (`geometric::CORNER`) and at most 0.3 mm off
  (`geometric::CORNER_DISTANCE`: the corners of roundings that are the
  file's lay 0.03–0.25 mm off, while corner faces 0.83 and 2.5 mm off were
  other corners), and together they hold at most
  5 % (`geometric::CORNERS`), and when the faces account for the volume
  the change is off: at most twice their area times their distances
  (1e-5 mm at least each; the points are a sample, and a part of a face
  they miss shows in the volume: a cut whose end lay elsewhere on a face
  of 145 000 mm² was 67 mm³ off with every point on the file's faces). A main face off rejects the result,
  also within 1 %. A change more than 10 % off the file's is rejected
  without asking the faces (`history::DIFFERENT`: the differences they let
  pass came to 2–4 %). Where the faces cannot be measured (a state not
  rebuilt) the 1 % rule stays. The largest faces are measured first, and
  the measuring ends at a main face off (then the file's side is not
  measured): a point off a face takes the kernel milliseconds on a large
  body, and a rejected rounding of a design's large body took 10 s a
  definition before. The report gives the faces' verdict
  (`geometric_check`), the trace each side's new area and the faces off.
- Lofts with end conditions or rails match within 5e-3, with a warning
  beyond 1e-3. Mitcad builds their weights, angles and free ends by its
  own rule (`commands.md`, `loft`), while the file's surfaces may be
  fitted otherwise (between round sections the stored ones are not round,
  0.3–1.2 % below that rule; stored rails with end conditions differ too),
  so most of those fall back.

### Named inputs

Feature inputs in the streams name faces, edges and bodies as the body
blobs name theirs: every face and body of the ASM history carries
`generic_tag_attrib_def` names (a tag chosen by the operation that made
it and the operations that made and changed it), and an input's recipe
lists entities by such names (`core/f3d/src/design/recipe.rs`,
`core/f3d/src/names.rs`). The decoder rolls the `.smbh` blobs back to
just before the item (its `_f3d.result_no`) and finds them there
(`core/f3d/src/design/inputs.rs`):

- **Fillet and chamfer edges**: the edge between the faces named first
  and second; where several are, the one whose start and end touch the
  faces named third and fourth. Faces not in that state are looked for up
  to 24 states earlier (some recipes name a face as it was before later
  operations renamed it). The edge's middle point, length, ends and its
  own direction (`_f3d.direction`) go into its fingerprint; the import
  finds it by them and tries those edges first, then the history's
  guesses (*What the history settles*). What the learning dump settled
  (mitcad#96, *Learning the undecoded inputs*):
  - an item's tail ends at its last reference to a health object (some
    fillets refer to one near their start too, and lost their name,
    state and edges);
  - a recipe whose first entity has a negative tag (`"-1029"`) names the
    edge itself first, then its faces; the blob gives edges such names
    too, which pick among the edges between the faces where they are one,
    else the faces after it decide;
  - a recipe may have a second entity list after the first (read over);
  - a set input says what its group selects (the `u32` after `ref
    2DF7DA30 | u32 0`): 8 edges, 16 faces (every edge between the face
    and another is rounded), 9 a face the next group's edges repeat
    (left out); a group without parameter holders belongs to the next
    group that has some;
  - n inputs of one item with the same names, where exactly n edges fit
    them (n edges between the same two faces, without end faces to tell
    them apart), take one edge each;
  - each edge input stores the edge's curve as the input saw it (a line,
    arc, elliptical arc or spline, `_f3d.curves`): where the names fit
    several edges, the curve picks the one that is the stored edge
    (mitcad#306, *Stored edge curves* below), before the tail's order and
    the n-of-n rule;
  - the import finds a closed edge by its circle or ellipse and length
    (the file gives its middle half way along its parameter, opposite its
    seam, which the replay puts elsewhere; an ellipse's axes follow from
    its two ends of an axis, its direction at the middle and its length), an open one whose middle is up to
    0.05 mm off by its length (the parameter middle of a B-spline or
    ellipse edge), an open one whose middle is within 0.01 mm and whose
    length is up to 2 % off (a blend's approximated length), an open one by
    its two ends (within 1e-3 mm) and a length up to 2 % off where its
    middle is further off (a blend's or spline's parameter middle lies
    elsewhere on the replay's curve), and an edge the replay splits into
    pieces: two or more edges of one body on its segment or circle whose
    lengths add up to its and, on an open edge, follow each other from its
    start to its end; an open edge that is not a line (an elliptical arc,
    a spline) by pieces that follow each other by their ends from its
    start to its end, when their lengths add up to its and the file's
    middle lies within 2 % of its length of them
    (`refs::dressup_edge_match`). Two edges between the
    same ends of the same length are not told apart;
  - the geometric tolerance (mitcad#224, the import option
    `geometric_tolerance`, 0.01 mm by default): the replay's geometry
    differs from the file's by the kernels' noise (a chamfer built 1 µm
    otherwise moves the circle a later fillet names from z = 33.100 to
    33.099 mm). Where nothing is within the exact 1e-3 mm, the edge
    matcher looks again within the tolerance (the middle, the points on
    the file's circle, segment or ellipse, the ends, the sums of pieces),
    as do the lookups of faces through a point (`refs::resolve_face_where`,
    and `refs::face_resolved_uniquely` with the same tolerance), of edges
    by their middle (`refs::resolve_edge_where`) and of vertices
    (`refs::resolve_vertex`). The uniqueness rules stay: a decoded input
    found once within the tolerance is `read`; the item's report gives the
    largest such distance (`geometric_deviation`), the design's report the
    tolerance. Tolerances of 1e-3 mm or less turn the second look off;
  - the two edges of a sliver face (mitcad#224): a dressup that takes a
    face away exactly is built 1e-3 smaller (below), which leaves a strip
    that narrow where the file's two faces meet in one edge (two chamfers
    of 1 mm on the edges of a 2 mm band meet in a circle; the replay keeps
    a band 2 µm high). Where the edge found has a twin of its length on
    the file's curve within the geometric tolerance that shares a face
    with it (the strip), the file's edge is both (`refs::with_sliver_twin`;
    two such closed edges within 1e-3 are both, not ambiguous). A 0.2 mm
    fillet of the circle where such chamfers meet came `inferred` from the
    history's guess of both edges; it is `read` now;
  - the edge's body: the edge refers to its body's record like a body
    input does, so `_f3d.producer` and `_f3d.body_index` name the item that
    made the body. Bodies that touch have edges in the same place (the slot
    of one is the wedge of another); the import matches the edge among the
    bodies that item made first (`Importer::match_dressup_edge`), and among
    all bodies only where those hold none;
  - replay matches with midpoint-plus-length scores tied within
    `1e-9` mm in one body are refused (mitcad#106); coincident copies in
    different bodies of other items keep the existing first-body rule. Repeated aliases
    of the same body and edge name count once. Matcher failures distinguish
    `decoder_unresolved` (no decoded edge geometry),
    `missing_fingerprint` (no external fingerprint geometry),
    `missing_in_replay`, and `ambiguous_in_replay`. Decoder input failure
    notes retain the edge set, input position and decoder's `found` reason.
    Recipe tails remain
    unused for replay matching until their semantics are proved;
  - a two-distance chamfer's first distance lies on the face left of the
    edge's own direction in the file (the face whose coedge runs along
    it, seen against the face's normal): the import sets `flip` so that
    the replay's first distance lands there (per tangent chain, by the
    most of its edges; sets of equal distances keep theirs), and the
    other reading stays a guess after it;
  - the edges found by their names are also tried one tangent chain at a
    time (a feature per chain), right after all at once.
- **Bodies** of moves, splits, body patterns and mirrors: by their tags;
  the middle points of some of their edges let the import find them among
  its bodies (`refs::resolve_body`), else as the only body whose faces
  pass through those points (a replayed sweep or pipe has the stored
  surfaces but its seams and edges elsewhere). A mirror's joining is not
  decoded: separate copies are tried first, then joined ones.
- **Bodies by the item that made them** (mitcad#96): every body input
  also refers to the body's record, which names the timeline item that
  made the body (`_f3d.producer`) and, by the record's position among
  that item's own records, which of its bodies (`_f3d.body_index`;
  `core/f3d/src/design/build/producers.rs`). The import keeps what each
  item made (`producers.rs`): the bodies of the features made for it (a
  split's pieces from the bodies it split, which keep their ids; a
  combine's own bodies, the pieces a cut makes, without its target, which
  keeps the record of the item that made it: the combine's records are
  its new pieces only, so a removal that names the
  combine's first record takes the first new piece, not what is left of the
  target). A body
  input is found there first, then by its names (for an item with several
  bodies the names choose among them, else the index). Where base
  features took the item's state (a fallback), they bring every body that
  differs from the state, and a new body can take the id of one it
  replaces: there the names come first, then the base features' new
  bodies in the item's component. This also names bodies whose recipe
  does not decode (pipes' bodies, bodies later features changed: a
  split's bodies, combine tools) and the copies of a pattern, which the
  names alone cannot tell apart. Bodies made by items off the timeline
  (base features of other designs) are found by their names only.
- **Participants** of joins and cuts of extrusions: their body inputs,
  by the item that made each body (mitcad#96).
- **Thread faces**: the face named first (a `bounded_face` recipe names
  the faces around it next, which pick one of several); its cylinder
  (origin, axis as the file has it, radius) and a point inside it go into
  the fingerprint, and the import finds the face through the point.
- Moves also get their transform, splits an origin or construction plane
  as the tool.
- **Inputs on other components' bodies** (mitcad#161): an extrusion's
  extent or profile face, a sweep's, pipe's or loft's path, rail and
  guide surfaces and a pattern's, mirror's or split's inputs that are in
  no state of the item's own history are looked for in the other
  components' histories at the item's point of the timeline: the state
  after the last item of that history's component before it, else up to
  24 states earlier. A history blob belongs to the component whose items
  name most of its operations (where that blob is also the one whose
  operations the component's items name most). Result numbers follow
  the timeline only within a component, and the names' operation numbers
  need not be states of the history they are in (a component's history
  may start later in the numbering than its names). Such an input gets
  `_f3d.component` (the component's object id): its geometry is in that
  component's coordinates, and the import looks for it among that
  component's bodies. Fillets', chamfers', threads' and shells' inputs
  are on the bodies they change and are not looked for elsewhere. Backup
  corpus: 21 items of 17 files get such inputs.
- **Faces of any surface type** (mitcad#161): besides planes and
  cylinders, a face gets its surface type (`Cone`, `Sphere`, `Torus`,
  else `NurbsSurface`, with the analytic data) and a point inside it: on
  a free-form surface (a B-spline, a thread flank's ruled surface) the
  point of its parameters farthest inside the face's edges taken into
  them, on one of the rulings a ruled surface was built from, where it is
  exact; on a cone, sphere or torus half way across the face as for a
  cylinder. A free-form face also gets `_f3d.face_points`, points spread
  over it. The import takes any free-form face of the replay for a
  free-form face of the file.
- **Helical edges** (mitcad#161): an edge on a helix the decoder knows by
  samples gets its middle, ends, direction and length from the cubic
  through the samples (the straight segments between them were up to
  0.1 mm off a thread's helix and 0.15 % short).
- Combines, splits, patterns, mirrors and holes (mitcad#67): their
  selections (list inputs of bodies, faces, edges, features and planes),
  as SCHEMA.md §5.3 lists; the import builds them first and then, where
  the history checks the item, tries what the history suggests without
  them, so an input decoded wrongly or not found never makes the item
  worse.

Sketch curves carry their kind (construction, centre line) in their
sketch-curve part; the decoder gives `isConstruction` and `isCenterLine`
where they are set, so construction curves no longer split profiles.
Splines (their NURBS data), ellipses and elliptical arcs are decoded
since mitcad#154 (*The published format descriptions*); splines come in
as fixed splines, the decoder not telling fitted from control-point
ones, and a spline out of its sketch's plane is refused.

Parameters of the 2026 writers (class version 8) have their own layout;
before it was decoded their features had no parameters, and the import
fell back for most fillets, chamfers, revolves and patterns of such files.
A dimension's parameter also says whether the dimension is driven
(`isDriving: false`), and an offset dimension's flags whether it measures
a diameter about a line (`SketchLinearDiameterDimension`): sketches with
such dimensions keep their points once the parameters are known.

A driven dimension whose parameter the file has (and the import created)
gets that parameter as its read-only reference (mitcad#250, `reference`
in commands.md's *Values*): the parameter becomes the sketch's, with the
value the sketch measures, and expressions that use it follow; no
reference of the dimension's own is made beside it. The .ipt import's
driven dimensions between projected geometry with a parameter in the
part's table, and the FreeCAD import's named reference constraints, come
in the same way. Where the model refuses it (a feature before the sketch
uses the parameter), the sketch is added without these references.

### Sketch patterns, offsets and polygons

The decoder reads what these constraints keep beyond their entity lists
(SCHEMA.md, *Constraint*): a circular pattern's quantity and total angle,
a rectangular pattern's quantities, distances and directions (the copies
are the entities after the originals), and an offset's parent and child
chains, distance and dimension, from the offset object that refers to the
constraint. `sketch/groups.rs` translates them into what Mitcad's own
sketch tools make, each checked against the stored geometry:

- **Polygon**: a construction circle about the centre through the
  corners, and equal sides (`sketch.polygon`).
- **Circular and rectangular patterns**: a `patterns` record whose copies
  are the file's copies, matched to the originals instance by instance
  (a full turn spreads the instances, a partial angle ends at it; a
  rectangular distance is the spacing or the whole extent, whichever the
  copies show). Mitcad's second direction is its first turned a quarter
  and its spacings are positive: a negative distance turns the direction
  and its spacing becomes `-(d8)`, and the two directions are swapped
  where that keeps them at a right angle. The angle and spacings refer to
  the file's parameters, so a parameter edit moves the copies.
- **Offset** of lines, arcs and circles: each offset curve parallel to its
  source at the distance (a `line_distance`), or concentric with it with a
  radius gap (below), all with one parameter, and an `offsets` record
  (curves, results, side as `sketch.offset` orders the chain). The file's
  offset dimension, whose parameter is signed by the side, stands for the
  offset: a negative one gets a parameter of its own,
  `<name>_offset = -(<name>)`, which the sketch adopts.
- **Concentric circle dimension**, and the radius of an offset arc: a
  construction line along a radius from one circle to the other,
  horizontal or vertical toward the dimension's text, its ends on the
  circles and the centre on it, whose length is the dimension.

### Threads, tapped holes and sheets

The decoder reads a thread's object (SCHEMA.md, *ThreadFeature*): its
type, size, designation, class, side, diameters and pitch, whether it is
modelled and over the whole face, and which end a partial one is
measured from; a tapped hole's thread is the hole's sub-item, read alike.
The translation (`ops.rs`, `commands.md` *From the dump IR*):

- **Threads**: a `thread` on the faces found through the history, with
  the size Mitcad's table lists (ISO metric, the ISO 261 series up to
  M300, above M64 with fine pitches only, also the `M` sizes of other
  metric types; the forming (tapping) screw threads `ST3.5x1.3`, also
  stored without the `ST`; Unified; the parallel pipe threads `G <size>-<tpi>` as
  Mitcad's `G <size>`). A class Mitcad does not list for the face's side
  is left out (it only labels the thread), with a note. A partial thread
  starts at the end the file's cylinder axis points to (or the other):
  where Mitcad's cylinder of the face runs the other way, the ends swap.
  Sizes Mitcad lacks fall back. A cylindrical face the file keeps whole
  may be in pieces in the replay (a join across a bore splits it where
  the file's face goes on): the thread is tried first on the face with the
  pieces of the same cylinder end to end with it (a modelled thread one
  per piece, each piece's start angle measured), then on the face alone.
- **Sized faces**: a thread in the file, a cosmetic one too, also sizes
  its whole faces, an external one to the thread's major diameter, an
  internal one to its minor diameter (the reference models: a bolt of 10
  mm becomes 9.85 mm with an M10 thread). The translation first puts an
  `offset_face` of each face to that diameter before the `thread`; where
  that does not build (offsets of faces of stored bodies often do not),
  a tube of the material between the two radii along the face (two
  `cylinder`s, one combined out of the other) is cut from or joined to
  the body, and the thread goes on the tube's side; then the thread alone
  is tried. The offset is given 2 s (mitcad#68: offsetting a face of a
  large or stored body took 38 s where the tube then took 1.3 s). For a
  cosmetic thread the volume the sizing adds ranks them against the
  history's change, so a thread whose state shows no sizing is tried
  alone first.
- **Tapped holes**: the hole's own history state has the bore of its
  diameter parameter; the file's thread widens it to the thread's minor
  diameter in an operation of its own, which the import leaves out (about
  1e-4 of the volume). The hole comes in with that bore and a `thread`
  feature on its walls (`hole<i>.wall`, from the hole's start, the wall's
  low end, for a partial thread), not with the hole's own `thread`, which
  would make the bore Mitcad's basic minor diameter; then the minor
  diameter and the hole without its thread are tried. Holes placed by the
  history are tried on all bodies, then on the bodies they go into only,
  and through all where the depth is not decoded.
- **Modelled threads** (mitcad#57): a `thread` with `modeled`, the
  file's `diameters` (`majorDiameter`, `minorDiameter`, `pitchDiameter`
  of `threadInfo`) and an `angle`, one per face, tried first on the faces
  as they are (the thread sizes its part of the face itself), then after
  the sizing above. The profile was found by measuring the bodies stored
  in the files: the basic profile of ISO 68-1 (flanks at 30° to the
  radius, the thread half a pitch wide at the pitch diameter) cut off by
  flat crests and roots at the class's major and minor diameters (the
  middles of the tolerances; no rounding), along the whole face and ended
  by the planes of the face's ends. The reference model's M10x1.5 6g bolt
  (9.85/8.141/8.928 mm) has 3.2e-5 less volume than that exact profile
  (its flanks are fitted splines), and the thread replaces the material:
  an internal M10x1.5 6H thread on a 10 mm bore in a corpus design adds
  the teeth inside the bore down to the minor diameter, the exact
  profile's 234.334 mm³ against the stored 234.334 mm³. Where the helix
  starts is not decoded; the import measures it on the next history
  states (which half of a pitch at the pitch diameter is empty, at four
  angles about the axis) and turns the thread to it, else angle 0. It
  measures where segments along the axis cross the stored bodies' faces
  (`Kernel::segment_crossings`, intersecting only the faces near them):
  classifying the points against every spline face of a stored thread
  took about 5 s a thread, the crossings take milliseconds (mitcad#68). The
  reference bolt and a corpus design's nut, both on axes along +Z, start
  where Mitcad's angle 0 does. The groove's middle at each angle is the
  middle of the stretch between the crossings where the segment leaves
  and enters the material, and the angle is kept to 1e-4° (mitcad#161:
  the 48 samples a pitch and tenths of a degree placed a thread's flanks
  up to 0.015 mm off, too far for a sweep guided by the flank to find
  it; now within a micrometre). Region probes (*What the history
  settles*) classify their points the same way, by where a segment from
  each point along the sketch's normal out of the body crosses it (they
  took a minute for a sketch of 80 regions beside a stored thread of 720
  spline faces).
  Whitworth and NPT threads fall back (Mitcad models only 60° threads).
  A tapped hole with a modelled thread gets a modelled `thread` on its
  walls (angle 0: the walls do not exist before the hole).
- **Sheets** (surface bodies): the history's matching compares solids
  only. The sheets of a state come in with the fallbacks that take it, and
  those of the last state at the end, as stored bodies (base features);
  joins, cuts and intersections without participants then name their
  component's solids, so that the sheets are left alone as the file's
  solid operations leave them. Without a history the file's sheets are not
  told from the feature tools and sketch faces its blobs also keep, and
  stay out.

### Sweeps, pipes and lofts

The decoder reads their inputs (SCHEMA.md, *SweepFeature*, *PipeFeature*,
*LoftFeature*; `core/f3d/src/design/build/sweeps.rs`):

- **Paths, guide rails, centre lines and rails**: lists of sketch curves,
  named by the ids the sketch's curves carry (`crv_primary_id`,
  `crv_secondary_id`), or of edges, found in the history as fillet edges
  are (*Named inputs*), as the dumps' `PathEntity` items.
- **Profiles**: a sweep's and a loft section's sketch, from its profile
  source; which of the sketch's regions is not stored in a form decoded.
- **Loft sections** in order: a sketch profile, a sketch point (by its
  `pt_tag`) or a path of a body's edges; the first and last sections'
  end conditions (free, tangent, smooth, direction, tangent point) with
  their weight and angle parameters.
- **Operations** of all three (a pipe's from one design only).

Not decoded: a sweep's direction and rail scaling and participant
bodies. Edges of other components (through their occurrences) are
recognised and make the item fall back. Guide surfaces are found in the
history as other face inputs are (mitcad#161, *Named inputs*): a point
inside each, and more points spread over a free-form one; `sweeps.rs`
translates them into `guide_surfaces` (mitcad#157): the face through the
point among the bodies of the component the decoder found it in, and
every piece of it where the replay splits it (the faces through the
other points, and pieces of the same face that go on smoothly across an
edge). A path or guide surface on another component's body (the file's
assembly context: a nut's thread swept along the mating part's flank)
links that body (`body_links`, as a combine's `tool_links`), and the
sweep goes into its profile's component. A path edge the replay splits
(a thread's helix, one edge per turn) is found as the chain of edges
that go on smoothly from its start to its end with its length and
middle (`refs::resolve_edge_chain`). A guide rail given beside guide
surfaces is not used.

The corpus case (backup f262, item 9: a profile across a nut's thread
swept along the crest of the mating part's thread, guided by its flank)
now gets its inputs, links and definitions, and still falls back: the
profile is taller than the thread's pitch, so the sweep passes over
itself six times, and the geometry does not build it (the pipe shell
makes a self-intersecting solid; swept in pieces, the pieces' union
fails or loses pieces on the real thread, `sweep:
NCollection_IndexedDataMap::FindFromKey`, invalid wires). The
definitions take the item's time (about 50 s of the import).

`sweeps.rs` translates them with what the history settles: a profile's
region (a sweep's sets as for extrusions; a loft's one region per
section, probed ones first, at most twelve choices of all sections), a
second fraction 0 as none first (a profile at the path's end), both
directions where a partial extent or a rail makes them differ (with a
rail the file's direction flag reads the other way: the reference
models' rail sweep, flipped, is Mitcad's sweep without `flip`), the
perpendicular orientation before the parallel one, a rail's scaling
before stretching and none, a solid circular pipe before a hollow one
and the square and triangular sections, and, without participants, a
join or cut on all bodies, then on those the history changed, then a
join as a new body. The reference models'
sweeps and lofts come in from their streams as from their dumps
(`test_f3d_import --models`). The corpus's sweep, a cut swept only
backwards from a profile at the path's end along a path with a slight
kink between its curves, comes in parametric since Mitcad's sweep rounds
corners whose mitre does not build (mitcad#56).

### What the history settles

The history picks what the stream decoder cannot tell, by trying
candidates against the next state:

- **Face profiles.** An extrusion without a decoded profile may extrude a
  planar face: faces whose area times the distance equals the state's
  volume change become a sketch on the face with its boundary projected
  (linked), and that region is extruded (outwards first; inwards first for
  a cut; on the side of the state's new body). These are tried before the
  last sketch before the item. A join that touches no body is also tried
  as a new body when the state has one more. The faces the decoder reads
  in the profile (*Parts of items, face profiles and sketch heights*
  below) come before these guesses.
- **Profiles.** The decoder reads the loops of the profiles an extrusion
  or revolution selected (`_f3d_profile_loops`, mitcad#96: the file's
  curve ids, which piece of each curve, outer loop or hole, the pieces a
  profile is split into). `src/profiles.rs` maps them onto Mitcad's
  regions: a listed profile is a face (its outer loops less its holes),
  and a region is selected when its inside point lies inside one by the
  even-odd rule against the listed curve pieces, with the file's curve
  geometry (a curve cut where the other curves of its loop cross it, the
  pieces numbered along its parameter; lines, arcs, circles, ellipses,
  elliptical arcs and splines, the last three met numerically); where a
  numerically cut curve's pieces contradict the piece count the file gives, or
  it selects nothing, its outer
  loops select the regions whose outer loops share most of their curves.
  Over both private corpora the history accepted the decoded regions for
  1509 of the 1532 settled extrusions that have the loops (most others
  are sets it cannot tell apart from them) and for all 97 settled
  revolutions. Those regions are tried first, before any ranking
  (`Candidate::first`), with the decoded extent; without a history they
  are the ones used. Checked against the item's own state, their
  definitions (the decoded extent, each choice of participants) are
  translated and tried on their own first, and the other sets below only
  when none of them gave the state (mitcad#181: probing the next states
  for those sets took 10–30 s an extrusion on bodies of thousands of
  faces, mostly in vain); they keep their order, and the others come in
  the order they would have had among all. If a later pattern or mirror
  may copy the extrusion, all definitions are translated together so an
  exact match still keeps alternatives for *Profiles a pattern or mirror
  needs*. This conservatively includes every later pattern and mirror:
  decoded body selections can also fall back to feature copies. The learning dump
  (`MITCAD_IMPORT_LEARN`) still gets all of them at once. Then, as before
  the loops were read:
  1. regions whose material the next states change, found by probing a
     point inside each region (in its outline, not its holes:
     `Region::holes`) halfway along the extent on both sides
     (`Kernel::points_inside`); this also merges regions split by
     undecoded construction lines. Such a set, extruded to the side where
     the state changes it, is taken to make the state's change and is
     tried first (a cut deeper than the body removes less than its prism,
     so its prism would rank it behind smaller sets);
  2. sets of any number of regions whose area is within 5 % of the state's
     change over a distance extent's length (bounded subset-sum search,
     nearest six);
  3. subsets of the same count, ranked by how close their prism volume
     comes to the state's change.

  Not tried: a distance extrusion without taper whose prism holds less
  than the state adds or removes (overlaps only take from a prism; left
  out before the likeliest are chosen), and one limited to participants
  that include every body the same definition without them changes (same
  result).
- **Participants** of a join, cut or intersection: the decoded ones first
  when every one is found (all bodies when they are all the solids of the
  sketch's component), then all bodies; without decoded ones all bodies
  first, then the bodies of the sketch's component that the state
  changed (a cut in the file may leave bodies in its way alone).
- **Direction:** the decoded direction vector against the sketch's normal
  (one side through all: the stored vector points away from the
  extrusion; a vector oblique to the normal, along the normal: all 36
  such settled extrusions went that way, mitcad#104), else the decoder's
  ±1, then the other way. What the file does not decide (mitcad#104):
  a one-sided cut through all whose flag byte after the extent codes is
  0 goes against the stored vector in 15 of the 22 the history settled
  over both private corpora and along it in 7, and no byte of its record
  or its inputs, nor the side of the sketch plane where the bodies' volume
  lies (along it in 4, against it in 9 of those with the volume on the
  vector's side), nor the material just off the profile tells which;
  some distance extrusions (13 of about 1400 one-sided ones, most with a
  join or cut whose other side gives a state too) and 4 two-sided ones
  with equal or nearly equal sides likewise. The history settles those
  (the other way is the next definition); without a history the first
  is taken; **symmetric
  extents** (the decoded length first, then the other; without one, half
  the length each way first, then the whole length); **up to an object**
  (the decoded object first, towards the side of the sketch its point lies
  on, else by the stream's flag; then, and without a decoded object,
  planar faces parallel to the sketch, nearest first; a two-sided
  extrusion has an input slot for each side up to an object, decoded in
  order onto the sides the parameters `Side1Offset` and `Side2Offset`
  make extents up to an entity, each side up to its face or, where the
  replay lacks the face as it is, its plane, side one towards the side of
  the sketch its object lies on: mitcad#104), **revolve axes**
  (sketch lines and origin axes, ranked by Pappus' volume). A
  revolution's operation (join, cut, new body: the first `u32` after its
  root part, an extrusion's codes) and its axis
  when it is a sketch line (the axis input's curve ids, as a sweep's
  path names its curves; a line of another sketch too) are decoded
  (mitcad#96): the decoded axis comes first, the guessed ones after it;
  a revolution whose profiles, axis and extent are all decoded is tried
  first. Axes that are faces stay guesses.
- **Fillet and chamfer edges** the decoder did not find by their names
  (and after those it found, should they not give the state). Candidates
  are the replay's edges whose midpoints are off the next state's faces
  (`Kernel::boundary_distances`, seams left out; each body of the state
  is asked only about the points not on another's faces yet, those whose
  boxes hold the most first, and keeps its faces' projections and the
  distances found for later items; analytic faces are asked first, other
  faces project a point from the nearest point of a grid over them, and a
  point within 1e-5 mm of a face has its distance to it without a search
  of all faces). An edge is assigned to an edge set
  when a rounding or bevel of the set's size fits: the points where it
  would meet the edge's two faces (projected onto them) and the middle of
  its cross section (convex or concave by the side of the nearer of the
  edge's faces a point just off the edge lies on; the solid classifier
  where that does not tell) lie on the state's faces (within 5 % of the
  size and 1e-3 mm), and the edge's
  midpoint is as far from the state as that size leaves at its corner
  angle (within 10 %). Edges between faces that meet tangentially are
  no dressup's. Tried in order: all those edges at once; one tangent
  chain after another (sets and edges in order); those edges with the
  gone edges that continue them tangentially (a rounding that ends where
  its edge runs on tangentially takes the next edge's middle away too,
  and Mitcad's fillet follows the chain: also where a third sharp edge
  meets the vertex, at which OCCT's own chain stops, mitcad#224,
  `commands.md`, fillet); the edges whose dressup the
  state has whatever their distance, with their continuations; then
  looser guesses (the edges at that distance, all, the largest group at
  one distance). A definition refused because it holds only a part of a
  tangent chain does not rule out the definitions with more edges. The next four states are looked at; for an item whose state
  is known, one at a time (`Guesses`): the edges found by their names
  first, then its own state's guesses, the next state's only when those
  gave no state, in the same order as all at once (finding the edges a
  state lost takes seconds on large bodies, and the item's own state or
  the names usually give it). Once the item's own state lost edges of the
  replay, the later states are not looked at (mitcad#181): their guesses
  add only edges the item's state did not lose, which no definition that
  gives that state rounds, or the same edges again (a fillet of a design at
  the time limit found the same 32 edges in three later states, 13 s).
  - A rounding or bevel that removes a neighbouring face exactly (full
    round, chamfer as wide as a step), which OCCT cannot build, is built
    1e-3 smaller (`geometry/src/dressup.cpp`). Its body then differs
    from the file's by about 1e-6 to 1e-4 of its volume, and so do the
    final bodies
    built on it: most final bodies that are close but not exact come from
    such dressups.
  - A result BRepCheck rejects is repaired by shape healing when the
    repair keeps the volume (OCCT turns some roundings on stored bodies
    inside out). A result it rejects only on faces of the stored body that
    were invalid already, away from the rounding, is taken as it is
    (mitcad#107).
  - Where OCCT's rounding fails at the size asked, it is tried again on
    the body with the pieces of its edges merged (stored circles in two
    arcs), then with tighter tolerances of OCCT's fillet (knife edges),
    then with the strips narrower than the import's tolerance (0.01) at
    the selected edges' vertices collapsed into edges (mitcad#299), before
    the retries below (mitcad#107).
  - A rounding or bevel of a whole circle that runs over a neighbouring
    face (the rim of a hole near a side), which OCCT stops at, is built
    as a ring about the circle's axis when both faces are planes,
    cylinders or cones on it ([fillet](../model/src/api/commands.md#fillet)).
- **Combine** target and tools, when the decoded ones are not found: the
  bodies the next state changed (a body with a stored body's measures
  counts as unchanged even where its faces are split otherwise), the
  decoded operation first. A join or intersection that consumes its tools
  gives the same solid whichever of its bodies is the target: once one
  gave bodies the history rejects, the same combine with another target
  is not built (mitcad#181). On bodies that carry an earlier item's
  approximation (*Matching rules*) only the decoded combine is tried; its
  guesses come in the try on the state before it exactly, where they are
  as good: on the near-coincident faces of such bodies each boolean took
  10–15 s, on the state's 0.2 s (two combines of a design took a minute
  each; where the bodies cannot be brought to that state, the guesses are
  tried on the bodies as they are after all).
- **Split bodies** the tool does not divide: the file's split leaves such
  a selected body as it is, Mitcad's fails, so each body alone is tried
  after the decoded set (mitcad#96).
- **Replace face's replaced faces** (dumps do not record them): faces of a
  body the next state changed none of whose interior points
  (`Kernel::face_points`, up to nine) lie on the state's faces (1e-3 mm);
  neighbours that extend or shorten keep some. They are listed without the
  tangent chain, and the result is matched within 0.5 % (Mitcad's own
  conventions for curved faces and targets). The target comes from the
  dump.
- **Holes** whose item's through-all flag is set while the file keeps a
  depth for them (`_f3d_through_all`: a byte after the root part, the
  second in class version 4 and the sixth in version 7; seen in two
  designs only, so a lead, mitcad#96): through all first, as a guess the
  history checks, then the depth.
- **Holes** without decoded inputs: each piece of material the state
  removes is a hole along its cylinder's axis, starting at the plane of a
  planar face across the axis at either end of the piece (where the axis
  meets it); simple, counterbore or countersink by the sizes the item has;
  to its depth or through all. The removed material is the replayed body
  minus the state's, cut only where the two differ (mitcad#85,
  `removed.rs`, `Kernel::removed_material`): around the faces of the
  state farther than 0.1 mm from the replay's faces (a hole's walls) and
  the replay's faces with every point that far from the state's. Both
  bodies are cut down to boxes around those faces first. The two are near
  copies (a replay's free-form faces lie up to 0.07 mm from the stored
  ones in the `.ipt` test parts), and a boolean of the whole bodies
  intersects every such pair of faces: minutes for one hole, often with a
  wrong result. `MITCAD_NO_REMOVED_REGIONS=1` cuts the whole bodies (for
  comparisons).
- **Patterns** without decoded inputs: copies of one of the last six
  extrusions about the decoded axis (else the line the axis' direction
  input stores, `_f3d_axis`, mitcad#96, then the origin axes; else the
  origin axes), as many as the
  decoded quantities (else as the state's volume change asks for); the
  extrusion and quantity whose copies come closest to the change first;
  computed `adjust`, then, for an extrusion up to an object, `identical`
  (same result by distances). A rectangular pattern whose item stores
  objects but no directions (the oldest item version, mitcad#74) copies
  the decoded features. Its first direction is guessed along each origin
  axis, the second across it towards another origin axis at 90° (first),
  60° and 120° (a hexagonal lattice: a plate of hexagonal holes with even
  walls), each with both directions symmetric too. For a cut, a copy
  changes the bodies only where its centre (the centre of the material
  the extrusion removed, from its history states) lies in the replay's
  material, so the copies counted there make the change ranked against
  the state's: the copies of a large pattern over a round plate are
  mostly off it, and a lattice of the wrong angle puts fewer on it.
- **Copies of features** act only on the bodies the original feature
  changed where it has no participants (`original_bodies`, mitcad#74): a
  pattern of a hole in a plate leaves a disc of another extrusion that
  its copies reach as it is.
- **Profiles a pattern or mirror needs.** Several sets of regions can give
  an extrusion's state exactly (regions in material a cut already
  removed, or a join adds to), but not the same copies. An extrusion that
  gave its state exactly keeps up to four definitions ranked after the
  one taken. When a pattern or mirror of features gives no state, each
  extrusion it copies is given those definitions in turn, kept only where
  the bodies stay exactly as they are (the extrusion and the items after
  it give their states), and the pattern's candidates of it are tried
  again (at most eight such edits per item). The extrusion's note says
  which pattern chose its profiles.
- **Pattern and mirror objects** decoded as features with fillets and
  chamfers among them (mitcad#105): without them, as before (the copies
  of the other features carry them where those made the edges they
  round), then with them, where every one of them came in parametric (all
  the features made for each, one per tangent chain where the import split
  it), so that the pattern repeats them on its copies (`commands.md`,
  *Fillets and chamfers among the features*; the report's note says so).
  Where the history checks the item, the definitions with them come
  last, after the history's guesses too: repeating roundings on tens of
  copies of a large body takes tens of seconds, and most such items'
  states do not need it (a design at the time limit lost items to them
  otherwise); without a history, with them first. Where the file's compute
  option is optimized (read from the pattern's data object, above) the
  definition with them copies the faces the features made instead of
  rounding at each copy (`commands.md`, *Optimized: copying the faces*):
  the pattern of a pin and its two roundings on 14 × 77 copies, which
  OCCT's rounding failed on at copy 133 to 146 (71 s to fail), is replayed
  in 7 s and matches the file's state to 2e-5 in volume (14 copies where
  the support under the pin's foot ends at an opening's edge differ by
  15 mm³, the file keeping a little more of them than the copies' own
  faces give; 3 faces). A circular
  pattern whose axis entity (a face, an edge) is not found in the replay
  turns about the line its direction input stores (a construction
  axis' stores its direction times its length); a construction axis that
  was not imported is that line too. A mirror's `combine` is decoded
  (none of the bodies it makes is new: after its first two references
  `u32 0 | u32 n | n` body records), the other way after it as a guess.
- **Mirrors** without decoded inputs: sets of up to four preceding
  features whose changes in the history add up to the state's change.
  Bodies the state has twins of become new bodies; a body is joined with
  its image where the state has a body of twice its volume; else each
  body. The plane is the decoded one, else the origin planes and planes
  through the bodies' and images' centres (as a planar face on them when
  there is one). A body joined with its image is symmetric about the
  plane and holds the body: it is tried only where the state has a new
  body of between the body's volume and twice it whose centre lies on
  the plane (mitcad#88; the join of a nearly symmetric body with its
  image was the slowest guess, 119 s in an `.ipt` test part, and a mirror
  of a hole can never be one). The join itself works only where the image
  differs from the body (`Kernel::join_near_copy`, `commands.md`,
  *mirror*).

### Fallback

- A modelling item suppressed in the file (its result number is -1 and
  the first or second of the three state bytes before its health
  reference is set) comes in as a suppressed feature with its
  definition, unchecked, or as a placeholder (*Items suppressed in the
  file*, mitcad#180). One whose third byte is set made its change in
  other components only (its parts there, mitcad#169) and is skipped at
  once, as an `.ipt` part's feature whose state the history does not
  hold is (mitcad#96).
- A modelling item whose operation the history holds but which changed no
  body, and which none of its definitions reproduces, failed in the file
  and is skipped too (*Matching rules*, mitcad#156).
- An item with a known state that cannot be translated, fails, or does not
  give its state takes that state at once: one base feature (`base` with
  `replaces`; the changed bodies keep their ids, so later features
  continue on the stored bodies).
- Any other failing item waits. The next feature is tried on the bodies as
  they are and on each of the next history states put in by one base
  feature. The first combination that reaches a state settles it: the
  waiting items become `fallback` (they made that state, or added bodies
  to the state the next feature reaches) or `skipped` (the history shows
  no change by them). Items still waiting at the end take the last state.
- Without a history the first failure replaces every body with the stored
  design and later modelling items are skipped.
- **Final comparison:** the replay is compared with the stored bodies by
  volume; with `compare`, bodies whose measures differ by more than 1e-4
  also by surface deviation and boolean differences
  (`Kernel::compare_shapes`), each for at most ten seconds and all for at
  most a minute (`CompareOptions::seconds`: the deviations are sampled
  first, and the booleans, which on coincident or nearly coincident
  shapes can take minutes or not return, stop at the limit and are then
  missing). When every sample lies within 0.01 mm of the other body the
  booleans are left out (`CompareOptions::booleans_above`, mitcad#69), and
  boolean differences larger than the deviations allow (four times the
  volume they span over both surfaces: the booleans took nearly coincident
  bodies as apart, a relative difference of 2) are unknown, not a
  difference. Bodies that still differ beyond the matching tolerance are
  replaced by the stored ones (a warning names the base feature).

**States that do not rebuild.** A rolled-back ASM state whose conversion
loses faces or does not build is unknown (the report counts them): no base
feature takes it, and an item with that state is taken unchecked (its
first definition that builds). `mitcad-f3d-inspect history` names the
problem edges. Two ASM quirks are handled by the conversion:

- ASM puts the seam of a full cylinder on the line where it touches planar
  faces (a hole tangent to a wall), so the edge is used twice in each
  direction; it is taken as closed.
- B-spline edges whose ends are up to 0.004 mm off their vertices build
  with wider tolerances (gaps up to 0.01 mm accepted).

**Bodies the history already holds.** An item whose state lies behind the
replay (a base feature took the bodies past it; ASM states of reordered
items or other components can precede items the timeline puts earlier)
continues on those bodies: its first definition that leaves them unchanged
(a join of material they already hold) is taken, verified by them. Without
a history the same holds after the fallback to the stored design.

### The outcomes on the timeline

The report's outcomes stay with the features (mitcad#170, the model's
import notes, [commands.md](../model/src/api/commands.md#the-imports-outcomes-on-the-timeline)),
made from the report itself at the end of the import, so that the
timeline and the report say the same:

- Each fallback records the base features it made (one per component that
  changed, `Importer::note_bases`) and the items they stand in for
  (`Importer::stands_in`: the item that settled it and the waiting items
  it took). Their note is `Stands in for <item> (<the item's note>)` for
  each item, the waiting items' `; in the fallback of <base>` left out.
  A fallback item whose base feature no call site recorded is found by
  its `features`. Base features whose uid a later undone try reused are
  left alone (only base features get these notes).
- A base feature made for another reason (the bodies brought to a state
  exactly, the stored bodies at the end) gets the report's warning about
  it (`Importer::base_warning`, which writes both).
- A `partial` item's features get `Partly imported: <its note>`.
- An item skipped as suppressed in the file (one whose definition did not
  come in, mitcad#180), as one the file keeps no result for, or as
  failed in the file gets a suppressed `placeholder` at its place in its
  component (`Importer::placeholder`), its note the item's note; items
  of no geometry (`Class::NoGeometry`) get none. The placeholder is not
  in the item's `features` (it builds nothing), so the report and the
  coverage counts are as before.
- An item suppressed in the file that came in as suppressed features
  (mitcad#180): its features get its note (`suppressed in the file; not
  checked against the file's history`); a suppressed sketch's says
  `suppressed in the file` after what it says otherwise.

The FreeCAD import does the same from its report
(`timeline_notes::freecad_notes`: fallbacks with their notes, partial
sketches and features) and adds a placeholder for a feature suppressed in
FreeCAD (`suppressed in FreeCAD; not replayed: ...`).

### Items suppressed in the file

mitcad#180 (`suppressed.rs`). **The state bytes.** The three bytes before
an item tail's health reference (`_f3d.flags`) are set only on items with
result number -1 (both corpora, 9043 items). With result -1:

| Bytes | Items in the corpora | What they mean |
|---|---|---|
| `000100` | 50 (extrusions, fillets, sketches, revolves, threads, a chamfer, a canvas) | suppressed itself (the user's action: the design API, the timeline menu, a configuration's row or a suppressed group's member give the same bytes) |
| `010000` | 64 (fillets, extrusions, sketches, revolves, threads, a chamfer) | suppressed through an item it depends on (a fillet of a suppressed hole's rim, the extrusion of a suppressed sketch) |
| `000001` | 5 (4 extrusions, a revolution) | the change is all in other components, in the item's parts there (mitcad#169); each has its part |
| `000000` | sketches, construction geometry, assembly items; modelling items at the end of the timeline | no result to keep, or after the timeline marker (never computed) |

Which of the two suppressed bytes an item has was settled with the
controlled variants: an item and its dependent are suppressed
together by suppressing the first, and then the dependent has `010000`
and the first `000100` (suppressing the dependent too afterwards leaves
`010000`); suppress the dependent alone and it has `000100`;
two independent items suppressed one by one both have `000100`. A
dependent's other bytes are the same as when it is suppressed itself
(the two models of a fillet differ in these two bytes alone). In the
corpora 24 of the 27 designs with suppressed items begin with a `000100`
item, as an item suppressed through another cannot come first; the other
three are versions of one design, whose first suppressed item is a fillet
with `010000` that follows a coil (its parent is not seen: unexplained).
Of the suppressed fillets, 32 of the 39 with `010000` have a face of an
edge that no history state holds (the parent's, or no edge between
the faces named), and none of the 9 with
`000100` has (the census). Items with these bytes also have
`01 01 00` where the others have `01 00 00` after the health reference
(the middle byte is 1 for either), and -1 where the others have the
number of their start state (the result number before theirs). An item
that failed in the file has the bytes and numbers of a healthy item; the
history tells it (mitcad#156, *Fallback*). `decode::item_state` reads the bytes,
`decode::item_suppression` tells the two apart (`Suppression::Direct`,
`Indirect`), the stream decoder writes `isSuppressed: true` for a
suppressed item, and `TimelineItem::file_state` and
`TimelineItem::suppression` give the import the state from either (an
external dump's `isSuppressed` has no bytes and counts as direct).

**After the timeline marker**. Two more state bytes stand in
front of an item's last reference (`_f3d.after`: the two bytes before the
`6CCF41D5` reference that follows the health reference's `5FBE538E`
one). They are `00 00` on every ordinary item; the first is 1 on a
suppressed item (the middle one of `01 01 00` above) and on most items
after the timeline marker; the second is 1 on every item after the
marker (rolled back in the file). The controlled variant of a rolled-back
hole and fillet has `00 01` on both (the items keep their result
numbers, which the history no longer holds). In the corpora 52 items have
the second byte (10 of them with a result number), among them every one
of the 26 modelling items that have result -1 and no state byte (the 13
removal items, which have no result of their own, are no such items),
and they are the end of the timeline in 15 of the 16 designs that have
any (the sixteenth lists a component inserted from another design after
the root's last item). The decoder writes `isRolledBack: true` on such
items and the timeline's `original_marker_position` and
`restored_marker_position` (the position of the first of them).

**Suppressed features.** A modelling item suppressed in the file is
translated as any other (its inputs are decoded as on other items), its
definitions tried in their order (those a proven rule decoded first), no
guesses: no history state checks them, as the file keeps no result for
the item. The first that builds is taken: `parametric`, not verified,
note `suppressed in the file; not checked against the file's history`;
then it is suppressed. Items suppressed in a row (an extrusion and the
fillets of its edges, a sketch and its extrusion) stay built until the
next item that is not suppressed, so that each is translated on the
bodies the ones before it make, as in the file; then the run is
suppressed, the last first, and the bodies are those before it (a
warning says so if not). An item whose definitions do not build on the
run's bodies is tried on the bodies without them (it may have been made
while they were suppressed); when it builds there, the run before it is
suppressed and a new one starts with it. Suppressed sketches and
construction geometry come in as usual and are suppressed with their run.
Unsuppressing a feature in Mitcad builds it where it stands (the items
before it in its run unsuppressed too, where it uses them, as in the
original application).

**Placeholders.** Where no definition builds (no translation, no inputs
decoded, only guesses, every definition fails, or the import gave up on
the item: stopped, out of time or memory, after the fallback to the
stored bodies), the item is `skipped` with `suppressed in the file; its
definition did not come in: <why>` and gets the placeholder of mitcad#170.
Suppressed assembly items get a placeholder, suppressed items of no
geometry none.

The report counts the suppressed modelling items
(`suppressed.suppressed_as_features`, `suppressed_as_placeholders`; the
text: `suppressed in the file: n modelling items as suppressed features,
m as placeholders`, with `(k of them through another suppressed item)`
for those suppressed through another, `suppressed_indirectly`, whose
placeholders' notes end `(suppressed through an item it depends on)`);
the corpus test prints their totals.

**Rolled-back items**. Where the timeline ends with items
after the file's marker (*After the timeline marker* above; not when
an item that is not rolled back comes after them), a modelling item after
it is taken as a suppressed one is: its definitions in their order, no
guesses, the first that builds, `parametric`, not verified, note `rolled
back in the file (after its timeline marker); not checked against the
file's history`, else a `skipped` placeholder `rolled back in the file
(after its timeline marker); its definition did not come in: <why>`. The
items stay built on each other while the import runs; at its end the
marker is moved before the first of them (`close_rolled_back`), so they
come in rolled back (status `rolled_back`) and the final bodies, which
the import then compares with the file's, are those before them.
Sketches and construction geometry after the marker come in as usual,
after it. The inputs are looked for in the state after the last item
before the item, as a suppressed item's (`inputs::no_result_state`).
The report counts them (`suppressed.rolled_back_as_features`,
`rolled_back_as_placeholders`; the text `rolled back in the file: n
modelling items as features after the marker, m as placeholders`). Before
this the items were tried against states the file does not keep and ended
as placeholders or fallbacks; Mitcad moves the marker with its timeline
commands, so unrolling them builds them where they stand.

**Inputs.** The decoder finds a suppressed item's named inputs (fillet
and chamfer edges, faces, bodies) as any item's, in the state just
before it; the file keeps no state of its own for it, so that is the
state after the last item of its component before it
(`inputs::starts`, `Timeline::state_at`), and the notes say `before item
n (suppressed in the file)`. A fillet's edges between faces that a
suppressed item before it made (a fillet of a suppressed extrusion) are
in no state the file keeps: such a fillet stays a placeholder (`a face
of the edge is not in the history state`). A placeholder of a fillet or
chamfer gives why its edges found by their names did not build; the
history's guesses of its edges are not worked out (none could be taken).

Limits: a suppressed item after a pending fallback is translated on the
bodies before the pending items, and a later base feature that replaces
those bodies overrides what it builds when unsuppressed; a feature
suppressed in the file is not checked, so a definition that builds but
differs from the file's (another direction, other regions) is not seen.

### Time limit, stop and hangs

**Time limit.** `time_limit` (`--time-limit`) seconds after the start, the
remaining items take their history states without trying definitions, and
the final bodies are compared by volume only. The application sets none:
its imports run to the end unless the user stops them.

**Item time.** An item's definitions are tried for `item_seconds` (60 s;
the part of it on bodies brought to the state before it exactly gets half
again). The definitions are evaluated with a monitor whose deadline is the
end of that time (`RecomputeMonitor::within`, mitcad#69), so that the
kernel's long operations stop inside one definition (a rectangular pattern
of 35 × 35 copies took 113 s): the definition fails ("the item's time was
over before its definition was evaluated") and the item gives up like one
out of time.

**A definition's share** (mitcad#78). One slow definition must not use up
the item's time before those ranked after it are tried (a join onto a plate
of a thousand holes took a minute where the next definitions took a second
or two). Checked against the history (an item with a state of its own, and
each set of bodies of one without), the definitions are tried in two
rounds (`Turns` in `lib.rs`):

1. In rank order, each with a third of the item's time (`SHARE`); the
   last one, when none waits, with the rest. One cut short at its share
   gives way and waits; the same definition limited to participants waits
   behind it untried (the boolean on fewer bodies takes as long).
2. Those that gave way, in rank order, with the rest of the item's time,
   each only while more time is left than it already ran (it cannot finish
   in less).

Definitions that give a state take seconds (up to about 15 s on bodies of
thousands of faces), so one that runs longer is mostly a wrong guess (on a
plate of a thousand holes the wrong ones took one to two minutes each); one
that would give the state within the item's time still does when those
after it are quick. A smaller share would reach more definitions, but each
one cut short and tried again costs its share. When the item gives no
state, its note names the definition cut short after the longest time
("cut short after 20.0 s: definition 3 of 12 (extrude, join, 1 profile)
(and 2 others)"; the trace has the whole definition).

**Stop.** `Options::stop` is a `RecomputeMonitor` that another thread
cancels. `mitcad-ffi` passes the document's job (`import_f3d` under
`attach_job`); the application's import worker cancels it on Stop and Keep
What Is Imported (`app/files/F3dImport.cpp`). Once cancelled:

- the item being replayed and those after it are given up as after the
  time limit (note "the import was stopped"): with a history they take
  their states, without one the stored bodies stand in; sketches and
  construction geometry still come in;
- while an item's definitions are tried, the request is the document's
  monitor, so the kernel's long operations stop inside evaluation and the
  definition fails at once; nothing else (fallbacks, undo) runs with it;
- the final bodies are compared by volume only;
- the report says where it stopped (`stopped`: the item's `index` and
  `name`; no `item` when the stop came after the last one) and warns.

The monitor's test delay slows each definition tried
(`tools/ui-import-test.sh`). A try that hung after a stop is rerun with
`Options::stop_at`: items before the one it stopped at
(`Progress::stopped`, or the one it hung on if it had not seen the stop
yet) are replayed, the rest given up.

**Hangs.** A kernel call cannot be interrupted. With `hang_limit`
(`--hang-limit`; `test_f3d_import --corpus` defaults to 600 s) the import
runs on its own thread (`core/ffi/src/f3d_import.rs`) with a 256 MiB stack
(OCCT recurses deeply; on a default 2 MiB stack features failed from
overflows) and shares its progress (`Options::progress`: a step counter and
the current item). The step moves between kernel calls, also in the
import's own long loops, which tick it after each call
(`mitcad_import::tick`, for the import running on the thread): building
and measuring bodies (a call per body; `Sig::of` ticks), finding faces
and edges by their fingerprints or for a joint's frame, comparing
bodies. Building a body from the file heals it with OCCT's ShapeFix,
which reports its progress face by face (`brep::BuildOptions::progress`,
`f3d_build_body`), and that moves the step too, as does the progress of
OCCT's long algorithms in the definitions tried (booleans, fillets,
sweeps: those a request to stop stops,
`geometry::CancelSource::progressed`). So a long item is no hang; a
kernel call that does not return is (mitcad#82: a large design's first
item, a base feature, healed two bodies of 15 000 and 23 000 faces for
110 s and looked hung under a limit of 60 s). A call can still take
longer than the limit and return: healing one face of that body took
74 s. The tries share such builds (`StoredFile::builds`): a try waits
while another builds a body, and when the watchdog gave the builder up
meanwhile, the builder hands its body over as B-rep data and the next
try reads it instead of building it again (which would take as long and
look hung again; waiting shows no progress either, so a build that does
not return still does). `MITCAD_IMPORT_STALL=slow` makes the file's
first body take 1.5 s to build, for the tests.

When the step does not move for that many seconds, the
thread is abandoned and the import reruns on a new document with the item
in `Options::hung_items`: it takes its history state without trying
definitions (note "the geometry kernel did not finish a definition of
it"), and the report warns. The items before it replay as before, so those
whose definitions gave no state (`Progress::failed_items`) take their
states at once in the next try, with the same note
(`Options::failed_items`): a pattern of a thousand copies took minutes in
every try. A hang while comparing the final bodies makes
the next try compare volumes only; a hang elsewhere brings in the stored
bodies without the timeline. The try given up is abandoned
(`Progress::abandon`, mitcad#71): when its kernel call returns it stops
before its next item or definition (a definition's long kernel operations
stop inside, as on a stop), compares nothing and its result never reaches
the report or the document, so it does not compete with the next try for
the processors and memory. Until then the thread keeps its stack and
memory, so under an address-space limit
(`ulimit -v`) the next thread's 256 MiB stack may not fit: it then starts
on the largest of 64, 16 or 8 MiB that does (a smaller stack can overflow
in OCCT's deepest recursions, which fails that feature, and the report
warns), and when none fits the import ends with an error saying so (it
used to fail with "Resource temporarily unavailable", mitcad#58).
A process that ends while such a thread still runs must not run its
static destructors: on Linux the shared libraries' finalizers destroyed
OCCT's statics under a thread healing a body it built, which crashed
(`mitcad-cli` exited with a segmentation fault after the error that the
import hung, mitcad#82). `abandoned_imports()` (`mitcad-ffi`) counts the
threads given up that still run; `mitcad-cli` and the application's
import worker then end with `std::_Exit` once their output is written.
`MITCAD_IMPORT_STALL=compare` simulates a hang for the tests
(`core/tests/test_f3d_import.cpp`, also under a tight `RLIMIT_AS` on
Linux); `MITCAD_IMPORT_STALL=busy` makes every try build the file's
bodies over and over without progress, so that the import ends hung
while the threads given up are in the kernel
(`cli.import_f3d_hang_exit`, `tools/cli/hang-exit-test.cmake`). A
tapered extrusion whose arcs shrink past their apex, on
which OCCT's draft loops, is refused before the draft ("the taper closes
the profile", `geometry/src/sweep.cpp`). The file is read once, before
the first try, on an import thread of its own and without the watchdog
(reading and converting a large design takes minutes and calls no
kernel): the tries share its decoded design and its bodies converted to
the neutral model (`StoredFile`), and each builds the OCCT shapes it asks
for (`F3dGeometry`), so a try run again neither reads the file again nor
holds a second copy of it beside the abandoned one (mitcad#80).

### Memory

An allocation that fails ends the process: Rust's cannot be caught, and
the import used to lose everything, also the items done (mitcad#80). So
the import guards its memory (`core/ffi/src/f3d_import.rs`,
`MemoryGuard`), on the watchdog's loop or, without one, on a thread of
its own, ten times a second: it measures the process against the
tightest limit it runs under (`core/ffi/src/memory.rs`,
`core/cpp/bridge/memory.cpp`): an address-space (`ulimit -v`) or data
limit, or the memory the system has left (available memory and free
swap) on Linux; a job's memory limit or the commit the system has left
on Windows; the physical memory on macOS; and the `memory_limit` option
(MiB, against the resident memory). Near a Linux limit the memory the C
library's allocator holds free (`mallinfo2`) does not count: it is used
again before more is mapped.

- At 70 % of the limit (and each 5 % more) memory is *tight*
  (`Progress::tight_on_memory`): before its next item the import drops
  the cached results of the definitions it tried and the bodies of the
  history states behind the replay (`Oracle::release_behind`,
  `StoredGeometry::release_before`), which are built again if they are
  asked for; what it imports stays the same.
- At 85 % it is *low* (`Progress::low_memory`): the definition being
  evaluated is cut short as on a stop and its item takes its history
  state (note "the import ran low on memory"), as do the modelling
  items after it until the memory is back under 70 %
  (`Progress::memory_recovered`); the import drops what it can as above,
  and from then on caches at most 256 MiB of results
  (`CACHE_AFTER_LOW_MEMORY`). A definition's own memory goes when it is
  cut short, so the import usually goes on with the next item (a
  rectangular pattern of a thousand copies took 8 GB in one definition).
  The final bodies are compared by volume only if the memory is still
  low then. The report says where it first ran low, how much was in use
  and how many items it gave up (`low_memory`, and a warning).
- A kernel operation whose allocation fails (OCCT's
  `Standard_OutOfMemory`, `std::bad_alloc`) fails with "<operation>:
  out of memory" (`geometry/src/util.hpp`, `KernelError::OUT_OF_MEMORY`)
  instead of ending the process; the import takes it as low memory.
  `MITCAD_TEST_OCCT_OUT_OF_MEMORY=<operation>` makes one fail for the
  tests.
- OCCT's allocator throws `Standard_OutOfMemory` instead of returning
  null, and counts its failures, per process and per thread with the
  jobs of OCCT's thread pool it ran (patches 0030 and 0031 of the port,
  mitcad#132):
  OCCT's collections wrote to the null block, and a large design under
  the corpus runner's 4 GB limit crashed in the checker
  (`CSLib_Class2d`) while one of the file's bodies was healed. The
  checker and the healing catch OCCT's failures inside and go on (the
  checker then calls a sub-shape invalid), so a moved count is what
  tells: a kernel operation and the checks whose thread's count moved
  (`geometry::failed_allocations_in_thread`) fail "out of memory", a
  body of the file whose build moved it does not build ("building a
  body of the file: out of memory", the import is low on memory), and
  the memory guard takes a process count that moved since its last
  measure (`geometry::failed_allocations`) as low memory ("an
  allocation in the geometry kernel failed"; when the operation that
  failed reports it later, as it does when the guard polls first, the
  report tells the operation's words), also for failures on threads of
  no import. An operation on another thread at the same time is not
  failed by them.
  `MITCAD_TEST_OCCT_ALLOCATION_FAILS=<operation>` (or `heal`) makes one
  fail and be caught inside for the tests.

The history's bodies are converted keeping only the distinct ones: each
state holds the bodies that differ from the state after it, not a copy of
every body (a design of 256 items took 5.8 GB before the first item that
way, and the memory the allocator kept afterwards left 40 MB of a 12 GB
address-space limit once OCCT's threads started).

### Definitions evaluated in parallel

Most of an import's time goes to definitions the history rejects: each
candidate (*What the history settles*) is a full geometry evaluation. With
`Options::threads` above one (`--threads`, the `threads` option of
`import_f3d`; `mitcad-cli` and the application use the logical cores, at
most 8; `test_f3d_import` one unless `--threads`), the definitions ranked
after the one being evaluated are evaluated ahead by workers (`ahead.rs`,
mitcad#95), and the import is the same as with one thread, only faster:

- **Rank order.** The import still goes through the candidates one by
  one and decides as before; it evaluates candidate *k* on its document
  while workers evaluate *k+1*, *k+2*, … (up to `threads` − 1 at once, as
  far as the item's attempts and a loose match's closer tries reach), from
  the second candidate of a list on: the first is usually the one taken,
  and workers beside it would only take processors and memory from it.
  When
  it comes to a candidate a worker has evaluated (or waits for it), it
  takes the worker's result instead of evaluating it again. So candidate
  *k* is taken only when every one ranked before it was rejected,
  whichever finished first, and everything that depends on earlier
  outcomes (participants that give what the definition without them gave,
  edges of a fillet that failed, `Turns`) is decided as before. Once the
  import leaves the list (one was taken, or none is left) the workers
  still evaluating are cut short through their monitors, as on a stop; the
  import does not wait for them.
- **Copies of the document.** A worker evaluates on its own copy of the
  document as it is before the item (`Document::fork`: the definition, the
  cached results and the last recompute, shared, with a kernel of its own,
  `Kernel::fork`; no undo history). A feature added there gets the uid it
  would get in the document, and its result the same version. A rejected
  candidate leaves the document as it is (it was undone anyway); its
  results go into the document's cache, as if it had been evaluated there.
  The candidate taken is added to the document again and finds the
  worker's results in the cache (`Document::adopt_results`): the same
  shapes, topological names, uids and notes.
- **Shared shapes.** The workers read the same bodies (the results before
  the item) at once. The geometry's operations never modify their inputs
  (T0e): those whose OCCT algorithms write into their inputs' sub-shapes
  (booleans, fillets, chamfers, shells, healing) work on copies
  (`InputCopy`), a `Shape` keeps what it measured of itself behind a lock,
  and OCCT's handles count references atomically. The cancellation scope
  of the geometry is per thread, and each worker installs the kernel's
  crash handlers and ticks the import's progress (`mitcad-ffi`).
- **Time.** A worker's definition is given the first round's share of the
  item's time (`SHARE`) from when it starts, and the item's deadline. One
  that gave way at its share is evaluated again on the document when the
  import would give it the rest of the item's time (the last one);
  otherwise its result stands. Results that depend on the time (an item
  out of time, an import past its time limit) can differ, as between two
  runs with one thread: with workers more definitions fit into the time.
- **Memory.** Each worker holds its definition's memory and a stack of
  64 MiB (the import's own thread has 256 MiB). No worker starts while the
  process uses half of its tightest memory limit or more (*Memory*;
  `Progress::set_memory_share`), and at 60 % the definitions being
  evaluated ahead are cut short and evaluated on the import's thread
  instead when the import comes to them; the import's own definition keeps
  the memory, as with one thread. A worker's definition that runs out of
  memory is evaluated again on the import's thread too.
- **Stop and hangs.** While the import waits for a worker it looks for a
  stop, the watchdog's give-up and low memory every 20 ms: a worker that
  does not return does not keep the stop from stopping. The watchdog
  counts the workers' progress as the import's; a worker the import waits
  for that hangs leaves no progress once the others are done, so the item
  is given up as before (`Options::hung_items`). Workers whose result is
  no longer wanted may still be in a kernel call when the import ends:
  `running_workers()` counts them, and `mitcad-ffi`'s
  `abandoned_imports()` includes them, so that the process ends with
  `std::_Exit` as after a hang.
- **What stays on the import's thread.** The second round of definitions
  that gave way, the approximate definition taken after a loose match,
  the next history state's fillet and chamfer edge guesses (`Guesses`,
  found between the definitions), the edits of a pattern's extrusions
  (*Profiles a pattern or mirror needs*), and the definitions tried on
  bodies the history holds already (*Bodies the history already holds*).

**Where the time goes.** Workers help only where an item has several
definitions queued and the first ones are rejected; the rest of an import
runs on its own thread as before. On a 199-item design of the private
corpus (time limit 300 s) 46 % of the time went to evaluating definitions
on one thread, 8 % to translating items and the rest to finding fillet
and chamfer edges between the definitions, the edits of a pattern's
extrusions, sketches, the history's bodies and the final comparison;
on a 76-item design 52 % went to definitions and 36 % to finding fillet
edges. Most items take their first definition (sketches, construction
geometry, fillets of edges found by their names, extrusions whose first
profile set is right), and a fillet's or chamfer's edge guesses come one
history state at a time, so its list has one or two definitions when the
workers could start. So an import with eight threads keeps about 1.5 to
2.5 cores busy where a single-threaded one keeps 1 to 2 (the geometry
kernel's own parallel booleans), and designs whose time goes to rejected
extrusions, combines and patterns gain most (up to about twice as fast).
A worker's definition also slows the import's own one where the
processors are all busy: on a machine loaded already, an import with
eight threads can take longer than one with one. *Work beside the
replay* below runs more of an import in parallel.

**The time limit on large designs** (mitcad#181). The five designs of
the private corpora that lost the most items to the 300 s limit (one
thread, as the coverage runs import) spent it, by their traces and
stack samples, on: probing the next states for an extrusion's region
sets while its decoded profiles gave the state (10–30 s an extrusion on
bodies of thousands of faces), the same combine built with each of its
bodies as the target on bodies carrying an approximation (15 s each),
a fillet's edge guesses against later states that gave the same edges
(5 s a state), OCCT's checker on large bodies (37 s for a stored body of
23 000 faces, half of each body built from the file), finding faces by
name on such bodies (each lookup read every face's names) and the
pattern edits of extrusions' other definitions. Each is cut
(*What the history settles*, `docs/architecture.md`); the items the
limit took went from 81 to none on a 199-item design (in both corpora),
from 48 to none on a 142-item one (the import 385 → 252 s and
348 → 246 s), from 50 to 45 and from 123 to 86–117 (the load of the
machine decides) on the other two, with no item's outcome changed but
those the limit took. What the two keep (one thread):

- healing the stored bodies of every history state (OCCT's ShapeFix,
  mostly its wires' self-intersection checks: 6–7 s a state for a body
  of 5 000 faces, about 80 such states in one design), which threads
  build ahead of the replay (*Work beside the replay*);
- definitions that do not give the state and use the item's whole time:
  guessed patterns (classifying their copies' points against a body of
  4 500 faces took 10 s a guess), through-all cuts of every body that
  reach stored bodies of 14 000–23 000 faces (OCCT rebuilding the cut
  solid, 30 s a definition), a decoded join that OCCT leaves invalid;
- the first base feature's bodies of 14 662 and 23 430 faces (45 s).

A second round on the six designs that still hit the limit (traces of
each import, stack samples of the slowest items) cut, in the same way:

- **Patterns that are not searched.** A circular or rectangular pattern
  whose decoded definition has read inputs, or whose copied feature did not
  come in, is no longer given the history's suggestions (the other features,
  axes and counts of `guessed_copies`): of the settled patterns with read
  inputs none took a suggestion, while on the rejected ones twenty
  definitions of a large body's copies used the item's whole time (125 s
  and more a pattern, and the rest of the import with it). Two designs went
  from 71 and 37 items lost to the limit to 6 and none (110 → 168 and
  100 → 127 parametric items) with this change alone.
- **Sweeps** take the decoded profiles' regions first (`_f3d_profile_loops`,
  as extrusions do) and no longer probe the next history states for other
  region sets: a sweep along a thread took 12 s the first and 30 s the
  second time to translate, now 0.3 s.
- **Boxes that hold a body.** A pattern decides which of its copies reach a
  body by their boxes, and the name lookups skip bodies by theirs; both
  used the optimal box (`BRepBndLib::AddOptimal`, a particle swarm on each
  spline face: a third of the active time of a pattern over a body of 4 400
  faces). `Kernel::enclosing_box` gives the box of the control points and
  tolerances instead (the tight one where it was measured already).
- **Fillets and chamfers repeated on copies** are applied in groups of 128
  copies: OCCT compares every pair of the roundings of one operation
  (`OverlappingStripes`, a minute for a thousand copies), and the faces at
  the contours' edges are found once per operation, not once per edge.
- **No other extrusion definitions after the time.** A pattern whose
  candidates used the item's time does not go on to edit the extrusions it
  copies (each edit would evaluate the same long definition again).

What keeps these designs from importing within the limit (not import
mistakes, the geometry kernel's time or failure): booleans on stored bodies
of 4 400–14 700 faces (15–70 s a definition, the pattern's or extrusion's
cut), a guide-surface sweep along a stored thread of 720 spline faces that
the kernel fails after 8 s and the variants of which take 10 s each, the
roundings repeated on a thousand copies (the kernel cannot end the rounding
at a vertex on some copies), and the healing of the stored bodies (45 s for
the 14 662-face base feature). An item that fails after such time leaves the
later items on stored states, and each item still takes the item time while
its definitions fail.

A third round, on the two designs that still went past the limit (a
116-item design whose body grows to 5 000 faces over 60 history states, and
the 256-item design of the 14 662- and 23 430-face base feature), by the
import's traces, stack samples and the reference dumps' bodies after each
item:

- **Healed faces reused between history states.** Each state's stored body
  was built and healed with ShapeFix as a whole (25–45 s a state for 5 000
  faces, built again after the limit for every item). A build of a large
  body (256 faces or more) now keeps its healed faces, edges and vertices by
  the content of their data (`brep::Healed`: two 64-bit hashes of a face's
  surface, its loops' edges, their curves and vertices; a part whose key
  the body has twice is left out), as healing left them (what its
  `ShapeBuild_ReShape` put in their place, an edge it split as the chain of
  its pieces, a seam it added to a closed face with the face). The build
  of the body in the next state that changes it takes the faces it shares
  with that build (its base: the same ASM record in the blob's state
  before, `StoredFile::bases`) as copies of those healed there, builds the
  others from its data,
  heals only those with ShapeFix (their edges made same-parameter and their
  vertex tolerances fixed where they meet the copies) and checks the whole
  body: when fewer than half of the faces are shared or the result is not a
  valid solid, the body is built and healed as a whole as before. The
  states' signatures stay the same (103 of 104 identical in the 116-item
  design, the other within 5 · 10⁻³ mm³ of 600 000), and a state of 4 500–
  5 200 faces takes 2–7 s instead of 25–45 s (4 300–4 900 faces taken).
  The base is fixed by the file, not by which builds exist (mitcad#243):
  a build waits for its base's build on another thread, and builds the
  base itself when no build of it kept anything yet or any more (a state
  the replay skipped, or released); what a build kept goes when its heir
  is built or no later state has it. Before, the next build took the
  blob's last three builds, which with threads depended on the timing of
  the builds ahead: a body could be healed as a whole in one run and from
  shared faces in another, an edge apart.
- **The body the file continues keeps its id.** A base feature that takes a
  history state paired each new body with the nearest old one; stored bodies
  now carry their ASM record (`StoredBody::lineage`, the same in every state
  of the blob), and a new body with the record of an old one replaces it. A
  split that came in as base features gave its first new piece the id of the
  body it split, so a later cut that names the split's first new body (its
  `body_index` 0) took another piece. The base feature that brings the
  bodies to the state before an item from the item's own start
  (`item_start`: the operations no item names right before its own) is now
  counted as the item's (`Importer::start_bases`): a split whose history
  holds two operations made the new bodies of both.
- **Named participants alone.** A join, cut or intersection with decoded
  participants was also tried on all bodies; where the item's history state
  changed only bodies it names, that is not tried (on all bodies the
  operation gives those the same and must leave the others as they are),
  which took 30 s a definition on the 14 000-face bodies.
- **No inverted-solid check in booleans.** OCCT checked each solid of a
  boolean for being inverted by building a point classifier of all its
  faces on one thread; Mitcad's solids are oriented, so the check is off
  (`SetCheckInverted(false)`).
- **A pattern's consecutive cuts in one operation.** A pattern of features
  cut each place (a feature at one of the places a pattern of patterns gives
  it) with its own copies: a pattern of a hole and of a circular pattern of
  21 of it, 28 times along an axis, was 44 cuts of 28 copies each from a
  body of 4 000 faces (more than a minute). With Identical and Optimized,
  consecutive places that cut the same bodies are cut together with all
  their copies (10 s, and the pattern matches its state).
- **Probes in one call.** Probing a sketch's regions against the next
  states (*What the history settles*) crossed each body with one segment
  per region and side, each call setting up every face the segment reaches
  (its intersector and the classifier of its wires: 1.5 s a probe on a body
  of 5 000 faces whose large faces hold hundreds of holes, 66 probes and
  98 s for one extrusion). `Kernel::segments_crossings` takes all the
  segments and sets each face up once.

What these designs keep: the 256-item design imports within the limit (213
items parametric, from 104–111; 106 inputs read, from 10–14); its fallbacks
included a rectangular pattern of a pin and its two roundings on 14 × 77
copies, whose second rounding the geometry kernel cannot end at a vertex on
copy 146 (its definition with the read axis takes 71 s to fail; the
optimized option's face copy replays it since, see *Pattern and mirror
objects*), and still include a cleanup
cut whose boolean OCCT gets wrong (the cut of its decoded profiles leaves
the body 2 mm³ larger where the file removes 31 mm³), and three splits
whose tool is not decoded. The 116-item design gets further (69 items parametric, from 62;
the pattern above and six joins and roundings after it now replayed) but
still reaches the limit near its 50th item: its joins and roundings on the
5 000-face body take 20–60 s an item (OCCT's general fuse, its checker on
the whole result and the face merge after it), and three joins whose
decoded definitions OCCT leaves invalid try their other definitions.

A fourth round, on the 116-item design (stack samples of the import's
threads, the trace of every item):

- **Faces measured once.** An extrusion up to a face looked the face up on
  each body of the definitions it tried (`refs::resolve_face_where`,
  `face_resolved_uniquely`), and each lookup measured the area of every
  face of the 5 000-face body (`Kernel::faces`, one face after another):
  88 s for one extrusion, which then fell back because its time was over.
  A shape now keeps its faces' kinds and areas once measured, measured on
  all cores (`Shape::face_measures`, geometry); the extrusion takes 6–9 s
  and is parametric.
- **A rounding's edges mapped to faces once.** After OCCT failed a
  rounding as asked, the retries mapped the whole body's edges to faces
  for each selected edge (`faces_at_edge`): 15 s of a rounding of a few
  dozen edges on the large body. The map is made once per try.
- **Checks only where the body changed.** A boolean whose targets the
  checker found valid checks only the faces it changed and those meeting
  them at a vertex, and counts the shells' closure, orientation and
  connection over the whole result (`is_valid_where_changed`,
  `docs/architecture.md`); a join near a few faces of the large body
  checked about 200 of its 5 000 faces. Booleans, roundings, chamfers and
  the bodies built from the file say when the checker found them valid
  (`Shape::checked_valid`).
- **The optimal box on all cores.** An extrusion through all asks for the
  box of the bodies; the box of each new body was the particle search over
  every spline face on one thread (3 s on the large body). The faces'
  boxes are now found on all cores, the same box.
- **A face merge that cannot concatenate a chain keeps it.** OCCT's face
  merge after a boolean (`ShapeUpgrade_UnifySameDomain`) threw "Courbes
  non jointives" when the p-curves or curves of a chain of edges it merges
  did not meet within their vertex's tolerance, and the join or cut whose
  boolean had given a valid result failed (two extrusions of the design,
  one of them its decoded definition). The port's patch 0110 keeps such a
  chain as its edges.

The design goes from 70 to 77 items parametric (27 → 34 inputs read) at
300 s on a loaded machine; its first component (57 items) now finishes
within the limit, and with the limit at 600 s 87 items are parametric (43
read). What it keeps: an extrusion whose decoded profile names curve
pieces the sketch's regions do not give cuts 3 % more than the file
(OCCT's cut of the right region, also without the far features: 2052
against 1985 mm³), a join that OCCT leaves invalid on bodies carrying an
earlier pattern's approximation (1.8 · 10⁻⁵ of the volume) before it is
tried on the history's state, a cut through all whose read side does not
cut (the other side gives the state), and the second component's items
after the limit. A 145-item design of the same round keeps its outcomes
(116 parametric) and no longer reaches the limit (261 s); its fallbacks
are roundings the geometry kernel fails with inputs as the file's
(mitcad#121) and a guide-surface sweep along a 720-face thread that OCCT
fails.

### Large designs, fifth round (mitcad#119)

A profile of a 256-item design with stored bodies of 14 662 and 23 430
faces and what it changed are in [docs/performance.md](../../docs/performance.md):
the checker's two quadratic steps on those bodies (port patches 0120 and
0121), face lookups through a point that pass over faces by their boxes
(`Kernel::face_boxes`), the edges of bodies kept within a budget of edges,
sketch snaps that project only the edges near each linked curve, face
counts from the shape's own map, and a quicker hash of entity indices and
circle seams worked out in parallel while reading. The design imports in
82 s instead of 149 s with eight threads and 169 s instead of 301 s with
one, with the same reports.

### Fallbacks without a B-rep round trip (mitcad#244)

A fallback's base feature holds the stored bodies as B-rep data. It used
to write each body's data and read it back to evaluate the feature, which
also named every face again and measured and checked the read body anew:
on the large design above about 2 s per fallback on its two large bodies.
Now the base feature's first evaluation adopts the shape the data was
written from (`Kernel::adopt_shape`; `Document::add_base_features` hands
the shapes to the evaluation beside their data): the same faces in the
same order with the names the data would get, sharing the stored body's
B-rep instead of a copy, and keeping what was measured of it that does
not depend on names (volume, box, face measures and boxes, edge curves,
the checker's verdict). The data is still written, for the design file;
an evaluation later (a design opened from its file) reads it. On that
design the reports are byte for byte the same; the import took 156 s
instead of 170 s with one thread and 76 s instead of 86 s with eight, and
its peak memory went from 2.5 to 1.9 GB and from 3.5 to 2.9 GB.

A retry of an item on the exact state before it replaced every body of
the replay, also those it already had; it keeps them now (next section).

### Retries on the file's exact state (mitcad#283)

An item whose definitions give no state on the replay's bodies is tried
again on the history's state before it exactly (in half its time), when
the bodies before it carry an earlier item's approximation (they are the
state's only within the matching tolerance), or when it is a fillet or
chamfer the geometry kernel failed on them (the stored state's edges are
split otherwise, and a rounding that fails on one builds on the other);
also an item the history names no state for, on bodies that carry an
approximation (`retry.rs`).

The replay's bodies that are already the state's were imported
correctly, and they stay: those that measure as a body of the state
(`Sig::same`), and those near one (`Sig::matches`) whose faces lie on its
faces within the geometric tolerance (`geometric_tolerance`, 0.01 mm:
the faces each has and the other lacks, `Kernel::new_faces`, measured
against the other's faces, `geometric::body_distance`). Only the bodies
that differ from the state (and the state's bodies the replay lacks, and
its sheets) are brought to it by a base feature, and the item is tried
there. Where nothing differs there is no retry: an item that still fails
takes the file's bodies as any item that fails (*Fallback*), and the
bodies before it stay parametric. Before mitcad#283 the retry replaced the
bodies the replay had within the tolerance (and, for a fillet or chamfer
the kernel failed on bodies that were the state's exactly, every body).

Replacing those bodies too is the user's choice (`import_f3d`'s
`retry_bodies`, `mitcad-cli import-f3d --retry-bodies keep|replace`;
`Options::retry_bodies`): `keep` (the default, also of the corpus tests),
`replace` (as before), or, in the application, `ask`: the first time a
retry would replace a body that matches, after the retry keeping them
gave no state, the import asks through the document's job
(`RecomputeMonitor::ask`, `RetryChoice`), and the answer holds for the
rest of the import, its later tries (after a hang) and the documents it
refers to. While the question waits the import ticks its watchdog.

The report's item gets `exact_retry`: the state, `keep` or `replace`,
the replay's bodies `kept` and `replaced` (`<name> (<uid>)`), and whether
the item `matched` there (else the replacement was undone, or none was
made); the text report a line under the item. The design's
`retry_question` names the item asked at and the answer.

Checked on a design of 256 items (content id `0b758b01`): its six
retries gave no state before either (all fell back), so the outcomes are
the same (220 parametric of 256, 31 of 31 final bodies exact); the retries
now keep the bodies within 0.01 mm (0.2–2 µm here) and replace only those
off by more (0.17 mm, 2.1 mm). On five smaller designs whose fillet
gave its state only on the stored bodies (content ids `59a1e52b`,
`7aa27c24`, `3a5fe9db`, `e4424061`, `4fe294cf`), the fillet now takes the
file's bodies by default, the bodies before it staying parametric: those
bodies are the file's within 0–3.4 µm, but an earlier item split their
faces otherwise (a revolution's or extrusion's face in two, a chamfer's
faces; one more face than the stored body, nine more on one), or the
fillet runs over an earlier fillet's own surfaces (one design), and the
geometry kernel fails the fillet on the replay's edges. What these need is
the fillet building on the replay's split faces (or the replay's faces
merged where the file has one), not the stored bodies.

### Fallbacks keep the bodies imported correctly (mitcad#285)

Every other base feature that brings the replay's bodies to a history
state or to the stored design keeps the bodies that match it as the
retry above does (`Stay::Matching`, `Importer::pair_within`): an item's
fallback, the pending items' bodies (one at a time or in one step with an
item), the state after an approximate extrusion or a guessed pattern or
mirror, the state at an item that failed in the file, and the stored
bodies at the end. A body that is the state's body exactly, or whose
faces lie on it within the geometric tolerance, was imported correctly and
stays as the replay made it, parametric; only the bodies that differ by
more (the bodies of the items that failed among them) are replaced.
Before, each of these replaced every body that was not the state's
exactly. The bodies kept carry their difference on: the distance the
replay carries from the state is measured after such a fallback.

What found bodies of the next states by their measures exactly now
treats a body kept within the tolerance as its stored body
(`Importer::stored_counterparts`), which a later state that leaves it
alone holds exactly: the combines, splits, removals and fillet edges
guessed from the bodies the next states change (a combine took a kept
body for its target before). Moving bodies into a new component takes
the same body, else the nearest within the matching tolerance.

Checked on 27 designs whose items take these paths (content ids, items
parametric before → after): `0b758b01` 220 → 220 of 256 (final bodies
31 of 31 within the tolerance, 29 of them exactly; the two others are
the replay's own, kept), `b596234a` 53 → 55 of 66 (the component move
and an extrusion after it now find the replay's bodies), `a1c980ab`
22 → 22 of 26 (a combine that fell back is now parametric, an extrusion
falls back, below), `f9d6d78a` 68 → 67 of 78, `9e53857a` 24 → 23 of 28,
`ad0df696` 13 → 12 of 14, and the other 21 the same; 1278 → 1277 of
1502 items in all, with 29 base features that bring bodies instead of 44.
What now falls back:

- An extrusion that joins a small cylinder to a body a chamfer made 2.6 µm
  off the file's (the chamfer's body has one face more than the stored
  one, 1.5 · 10⁻⁵ of its volume): on the replay's body the tool stays
  apart from it (it comes in as a solid of its own); the join's near-contact
  fusing reaches 1 · 10⁻⁵ mm only. Needs the join to fuse within the
  import's geometric tolerance where the tool touches nothing, or the
  chamfer's face as the file splits it.
- A join of an extrusion on a body whose faces each lie on the file's (no
  face off; its volume 1.8 · 10⁻⁵ off, the stored faces' measure): the
  geometry kernel's boolean gives an invalid face (intersecting wires) on
  the replay's body and a valid result on the stored one. Needs the join
  retried with a fuzzy value when its result is invalid.
- A base feature of the file whose state is the one before it: it was
  counted parametric because its fallback replaced a body of the replay
  that was 1.4 µm off (one face more than the stored body); nothing
  replaces that body now, and the item is skipped as one that changes
  nothing in the history.

### Joins within the geometric tolerance (mitcad#298)

A body kept within the geometric tolerance (above) can make a join that
works on the stored body fail on the replay's. The import therefore gives
the document its geometric tolerance as the join tolerance
(`Document::set_join_tolerance`, kept in the design file; commands.md
*Join tolerance of imported designs*), and the joins of its features go
through `Kernel::join_within`: where the exact join fails, a tool that
touches no target joins the targets within the tolerance, a tool solid
left on its own beside a target within it is merged, and an invalid
result is tried again, each with the smallest fuzzy value that works
(from 1e-5 mm up). Joins that work exactly are as before. Checked on the
designs of the section above (content ids, items parametric before →
after):

- `a1c980ab` 22 → 23 of 26: an extrusion joining a small cylinder to a
  body that a chamfer made a few micrometres off the file's (the chamfer
  is built 0.1 % smaller than asked: at its exact size it takes a face
  away, which the geometry kernel cannot build, so the body keeps one face
  more). The cylinder's start face lies within 1e-6 mm of the body's face,
  so it touches, but not on it for the exact boolean, which left the
  cylinder a solid of its own; it now merges with a fuzzy value of
  1e-5 mm. The joined body is the replay's own (4.4 · 10⁻⁵ of its volume
  off the file's, the chamfer's difference), where a base feature brought
  the stored body before. One linked curve of a later sketch no longer
  lies within the snap distance (2 µm) of the replay's edge, which
  carries the chamfer's difference, and is kept as fixed reference
  geometry (one projection fewer).
- `9e53857a` 23 → 24 of 28 and `ad0df696` 12 → 13 of 14: a join of a
  plate whose face lies on a face of the body, against the line where
  two pipes of the same radius meet (the body's faces all lie on the
  file's; its volume 1.8 · 10⁻⁵ off). The exact join's result had an
  invalid face (intersecting wires); with a fuzzy value of 1e-5 mm it is
  valid and matches the file's history. A later cut in the first design
  still falls back as before: its result on the replay's body is invalid
  (an unorientable face), a cut, which keeps its exact behaviour.
- `0b758b01` 220 → 220 of 256, the final bodies as before.

### Faces the replay split where the files have one (mitcad#284)

The extra faces of those five designs had three causes. A revolution or
extrusion joined onto a cylinder of the same radius left two faces of one
cylinder where their seams lay half a turn apart: the face merge after the
boolean failed to rebuild the merged face's seam (OCCT port patch 0220);
the faces now merge, as in the files. A chamfer as wide as a ring beside
its edge gave the edges along the ring's far circle p-curves half a turn
off (a tolerance of the ring's diameter) and a rounding of them failed;
and a rounding as high as a boss's wall left a sliver of the wall (built
0.1 % smaller): both are fixed by the port's patches 0144 to 0149
(mitcad#263). Checked by content id (features parametric, all verified
against the file's history; final bodies within 1e-5 of the file's):

| design | before | after | what still falls back, and why |
|---|---|---|---|
| `59a1e52b` | 12 / 13 | 13 / 13 | |
| `4fe294cf` | 23 / 24 | 24 / 24 | |
| `3a5fe9db` | 24 / 25 | 23 / 25 | the boss's foot is now rounded exactly (its wall vanishes, as in the file); rounding its top edge then, between the top and the foot's torus, fails in OCCT's walk for any radius (a boss with an exact foot rounding reproduces it); a chamfer as wide as a face is still built 0.1 % smaller, and a later rounding fails at a vertex beside its sliver |
| `7aa27c24` | 12 / 13 | 12 / 13 | the chamfer of a pocket's floor edges takes away both faces of its edges (the wall and a ledge, both 1.5 wide): OCCT builds it only 0.1 % smaller, and the rounding after it fails on the slivers |
| `e4424061` | 7 / 8 | 7 / 8 | the replay is the file's within 0.5 µm (the file's edges there carry that as their tolerance); the rounding of a square hole's vertical edges fails in OCCT's walk at their upper ends on an earlier rounding's corner torus, and builds on the stored body |

### Opening a design with many face appearances (mitcad#311)

A design of a circuit board with a pole round it (content id `e3c34193`,
150 items, 78 occurrences) opened in 12 s before the appearances
(mitcad#228) and in 190–205 s after, with the same outcome. The commit
that made it slow is `9e165ea9` (points inside faces of extruded and
revolved curves, merged with mitcad#228 in `4dd9265d`): its 127 face
appearances name 5 074 stored faces, most of them faces of B-spline
curves extruded, and those then had points inside them. Two steps took
the time, neither of them the replay (8 s):

- **Reading** (25 s): the point inside a free-form face matched each edge
  point to the nearest point of a grid over the surface by going through
  the whole grid (up to 2000 × 2000 points), and built the grid evaluating
  the face's curve once per grid point. The nearest grid point is now
  found through cubic cells (`names/surface.rs`, `Nearest`: the same
  point as the full search, the first of equally near ones), the grid
  takes each curve point once (`Patch::grid`, the same points bit for
  bit), and the faces are worked out on the reading threads
  (`appearance::fill_faces`).
- **Applying them** (159 s): a face of a surface of linear extrusion had
  an unbounded box (OCCT's box of it is sampled), so every lookup through a
  point measured its distance from every such face of the body, with an
  extremum search that builds a grid each time. Such a face now gets a box
  of its own (`geometry/src/query.cpp`, `extrusion_box`: the hull of the
  curve's poles over the face's stretch, moved to both ends of its stretch
  along the direction, widened by its tolerance); a point beyond a face's
  box is not measured against it; and each body's faces, boxes and edge
  names are read once for all its faces (`refs::PieceIndex`).

The reports are byte for byte the same. Times of this design (release
build, default threads; seconds; reading, replay, applying the
appearances from the import trace):

| build | open | reading | replay | appearances |
|---|---|---|---|---|
| `b7e68d3c` (before mitcad#228) | 11.9–13.2 | 0.1 | 9.7–11.1 | — |
| `7aff9023` (main) | 189.6–204.8 | 24.7–25.7 | 8.0–8.2 | about 160 |
| this change | 11.1–11.9 | 1.2 | 7.7 | 0.9–1.0 |

The design's items: 68 parametric, 1 fallback, 81 skipped (the
occurrences, the component insert and two items suppressed in the
file); the file's own base features come in as its bodies. No parametric
item goes through a retry on the exact state, and all 54 modelling inputs
are read. What fell back or was partial on 8.10.2026 (main `7fe8bac`, 38
parametric, 2 partial, 27 fallback) and came in parametric since:

| item | 8.10.2026 | first deviation then | now |
|---|---|---|---|
| Revolve1 | fallback: "crosses the revolve axis" | the axis not decoded (guessed as the sketch's y axis), the profile guessed among regions that straddle it | parametric, the decoded axis and profile (revolve axes and flags decoded, mitcad#186; straddling profiles) |
| Fillet5 | fallback: not in the history | on Revolve1's fallback body, its edges taken from the stored body's faces | parametric, edges found by their names |
| Extrude8 | fallback: cuts no participant | its sketch on a fixed plane (the face was on Revolve1's fallback body) | parametric |
| Sketch12 | partial: a curve type not decoded | one curve left out | parametric (sketch curve kinds decoded, mitcad#186) |
| Revolve2 | fallback: its sketch has no profiles | Sketch12's missing curve left no region | parametric |
| WorkPlane1 (now Plane1) | partial: a fixed plane | the plane's inputs not decoded | parametric (construction planes from their inputs, mitcad#178) |
| Revolve3 | fallback: not in the history | the two touching regions fused into a wrong face | parametric (mitcad#227) |
| Extrude11 | fallback: not in the history | single regions tried where the file selects two | parametric, the decoded profiles |

Left: **Fillet9** (a 0.9 mm rounding of the edge between Chamfer3's cone
and the pole's cylinder, r 35.1). The geometry kernel fails it on the
replay's body and on the file's own stored body (`--retry-bodies
replace`): its walk stops (ChFiDS_WalkingFailure). Ten flats cut the
cylinder below z 0.8, so above them the band is 0.3 mm high, and the
rounding's contact on the cylinder lies r · tan 22.5° = 0.373 mm below the
edge: over each flat it crosses the flat's ceiling. Up to 0.724 mm (a
contact of 0.300 mm) it builds. A body of the same shape made of
primitives (a revolved profile with the 45° bevel, ten boxes cut from its
side) fails the same way above 0.72 mm, and builds at 0.9 mm with the
flats 0.6 mm below the edge. The file's result is one torus round the
whole edge, its contact at z 0.727 also over the flats, cut there by the
flats' ceilings (which grow to meet it, 19.477 → 19.537 mm² each): the
rounding rolls on over each flat on the cylinder's own surface, as port
patch 0160 does over a gap whose rim is rounded, here across a sharp rim.
That needs an OCCT port patch of its own (a walked blend carried over a
cut-out with a sharp rim on its face's surface, the rim's face extended to
the blend).

`f3d.corpus_designs` (`tools/cli/f3d-designs.cmake`) checks the design
item by item where it is found by its content id under
`MITCAD_F3D_CORPUS` or `MITCAD_F3D_BACKUP`: the items above parametric,
only Fillet9 falling back, no parametric item after a retry on the exact
state, every input read, and reading and applying the appearances taking
less time than the replay. Without the design it is skipped.

### Work beside the replay

The timeline's items are replayed one after another; with `threads`
above one the work that does not depend on the replay runs beside it
(mitcad#103), and the import is the same as with one thread:

- **Reading the file.** The design streams of every design of the file
  are decoded first (`mitcad_f3d::design::decode_streams`); only the
  chosen design's named inputs are then looked up in the bodies' history
  (`resolve_inputs`), its items on several threads (each item reads the
  history's blobs only, read once for all). That runs at the same time
  as the conversion of the bodies of every history state to the neutral
  model (`StoredFile::new`), whose states are converted on several
  threads and taken in order as before (`mitcad_import::in_order`), so
  that the same bodies are kept once each. The two share the threads
  half and half. On a large design of the private corpus reading took
  about 140 s on one thread (80 s finding the inputs, 60 s converting);
  with eight threads about 20 s. An item's inputs are looked up in one
  named history state at a time (the state before the item, then up to
  24 earlier ones per blob, until each input is found): naming all of
  them at once on every thread ran the largest design of the backup
  corpus out of a 4 GB limit before the import began (an allocation of
  Rust's, which ends the process, mitcad#132). A named state is shared
  (mitcad#246): the items and threads that look in it take one naming
  while it is kept, the two or three states used last are kept, the
  whole circles the circle seams read after the items are taken when
  their states are named, and a fillet's face selections name the
  earlier states one at a time. Naming each state only once would mean
  keeping about twelve states of a large design, half a gigabyte more at
  the import's peak ([History states named once](../../docs/performance.md#history-states-named-once-mitcad246)).
- **The first state's large bodies healed while reading** (mitcad#245).
  The replay's first item asks for a history's oldest state with bodies,
  and healing its large bodies took as long as reading the file. The
  conversion of a history blob therefore converts that state first, and
  each of its bodies of 1 000 faces or more (largest first, 100 000 faces
  in all) is healed on a thread taken from the converting ones
  (`mitcad_import::Share`; at most half of them, while the conversion
  goes on with the others: `in_order_sharing` starts an item only on a
  thread left) while the rest of the file is read. The healing is the
  replay's build of that body: from the same data, without a base (no
  older state has the body), keeping what its heirs take (which leaves
  the body as it is; what it kept is handed on only when the body has
  heirs), measured as a body built ahead. Once the file's variants and
  bases are known, the body goes to its variant as a build in progress or
  built ahead (`StoredFile::take_early`); a try asking for it waits for it
  and ticks the watchdog with its progress, and its thread counts in
  `abandoned_imports()`. A body that does not build, or whose variant is
  not the converted one, is left to the replay. With one converting
  thread nothing is healed while reading.
- **The history's bodies built ahead.** When the replay asks for a
  history state, the bodies of the next four states that it has not built
  yet are built (healed) and measured on other threads
  (`F3dGeometry::build_ahead`, `mitcad-ffi`): the replay finds them
  built when it comes to them (the same bodies; a shape keeps what it
  measured of itself). A try that asks for a body being built ahead waits
  for it and ticks the watchdog while the build makes progress, so a
  build that does not return still looks hung; the threads count in
  `abandoned_imports()`. Bodies built ahead that no try took are dropped
  with the states behind the replay when memory gets tight. On the large
  design above, building two bodies of 15 000 and 23 000 faces took the
  first item 142 s on one thread.
- **One budget.** The workers that evaluate definitions ahead and the
  threads that build bodies ahead take their threads from the same
  `Options::threads` (`Progress::helper`: the import's own thread is one of
  them), and none starts while the process uses half of its tightest
  memory limit or is low on memory.
- **Planar faces kept per body.** Every sketch looks for its plane among
  the planar faces of all bodies (`refs::planar_faces`); on bodies of
  thousands of faces that took seconds per sketch (12 s on a body of
  23 000 faces, where most of a 256-item design's time went to its 73
  sketches). An import keeps the planar faces it found by the body's
  version (`refs::KeptPlanes`), which names one shape, so a sketch finds
  them again until the body changes. This is quicker with one thread too.

Coverage runs before and after these changes, at the same time with six
threads (time limit 300 s): `MITCAD_F3D_CORPUS` 575 s → 520 s in all
(1.11×, the slowest designs 1.08–1.39×), the backup corpus 4759 s →
3287 s (1.45×; the 256-item design above 1005 s → 341 s, others up to
3×). The reports with one and with eight threads are the same except
where a time limit decides (`MITCAD_F3D_CORPUS` 47 of 48 identical,
the backup corpus 318 of 325, each difference first at an item whose
time ran out or in the time-limited final comparison).

**Why the items are not replayed in parallel.** A profile of the five
slowest designs of each corpus (four and eight threads, time limit
300 s, before the changes above) kept 1.0 to 3.1 cores busy (mostly 1.4
to 2.3). Evaluating definitions took most of the time of the corpus
designs (the workers' share), finding fillet and chamfer edges 10–30 %
of the fillet-heavy ones, sketches up to 70 % of one large design
(the planar faces above), building the history's bodies up to 240 s of
another, and reading the file up to 140 s. Replaying independent items
at the same time would need items that neither read nor change what an
earlier one changes. Most designs are one component (in the slowest
ones 62–100 % of the items with a state change the same component), and
most of a part's items change the same body. Counting only the bodies
each item's state changes (an item reads at least those; a join or cut
with all bodies as participants reads more), the longest chain of
dependent items holds 42–66 % of the items' time on the slowest corpus
designs (a bound of 1.5–2.4× at best); with the items without a state of
their own (sketches, construction geometry, which find their planes on
the bodies) as dependencies of what follows, the chain is 82–100 % of
the time. A speculative item would also have to
run on a copy of the document and the importer's state (report, history
cursor, alternatives, sketches) and be replayed into the document as
the definitions evaluated ahead are, and the features' uids and
topological names of later items depend on every earlier item's. So the
replay stays sequential, and the work beside it runs ahead instead.

Not done: finding the fillet and chamfer edges of several history
states at once (10–30 % of the time of fillet-heavy designs). The search
asks the stored bodies' boundary index (`Kernel::boundary_distances`),
which keeps the projections it set up between calls; asked in another
order on other threads, the same answers are not assured, so it stays on
the import's thread.

### Components

`components.rs` makes the dump's components and places them by the
occurrence tree before the replay:

- placements: the decoder's top-level `transform` (relative to the root)
  and nested `_f3d.local_transform` (relative to the parent), or an
  external dump's `transform2` (full path) made relative to the parent
  occurrence; cm become mm; grounded and hidden occurrences stay so (the
  decoder reads the light bulbs: *Light bulbs*);
- a component used several times comes in once and is placed by each
  occurrence;
- occurrences of components of other documents (`isReferencedComponent`:
  inserted parts, fasteners) are occurrences of empty components, one per
  component of each document, named after the item that inserted it
  (`Screw1`, `Component Insert2`: the decoder gives each occurrence item
  the occurrence it made), else `Inserted component` (mitcad#75). Their
  bodies are not in the file, but joints, captured positions and ground
  items name them. The report counts them apart (`external`). Once the
  replay is done, those documents' own files fill them where they are
  found (*Components of other documents* below).

The file has one timeline for the whole design; every item goes into a
component:

- an external dump names it;
- for the decoder, the component whose ASM blob the item's operation
  changed (`StoredGeometry::item_components`; each component keeps its
  bodies in its own blob, in its own coordinates), else the component that
  owns the item: every component has a list of its items (class
  `3A6D1E62`, `classes::FEATURE_LIST`, which refers to the component's
  feature manager), and the decoder writes the owner as the item's
  `component` and `_f3d.component`. A feature that changes another
  component's bodies than its owner's (the file's assembly context) goes
  where they are;
- sketches and construction geometry follow the first feature that uses
  them, unless their owner is placed elsewhere than that feature's
  component at that feature's point of the timeline (*Occurrence
  placements*): their geometry is in their owner's coordinates, so they
  stay in the owner and the feature gets a copy (*Across components*).
  One no feature uses goes into its owner;
- other items without an owner follow the next item with a component
  (else the one before);
- a definition goes where the bodies, sketches and construction geometry
  it names are when they agree, else into the item's component; a sketch
  whose owner is known goes on a face of its own component only.

**Components made from bodies** (`ComponentFromBodies`, mitcad#156). The
component is placed by the occurrence tree before the replay, like any
other. The item's state is the new component's first one (its
`_f3d.made_result_no`, *The history as the reference*): the bodies that
state gives the new component, each the same solid as a body of the
replay's in the item's own component, move there by a
`component_from_bodies` feature in that component, whose component is
then the placed one (`Document::move_made_component`; the one the
feature made goes). The history of the item's own component keeps the
moved bodies as they were, though the design no longer has them there:
they leave its states from the item's on and the stored design
(`Oracle::retire`). Later features of the new component work on the
moved bodies. Before, the stored bodies stood in for the item, and the
moved bodies stayed in the root too (two copies in the design).

New-component operations become new bodies of that component. Fallback
base features are made per component, and the final comparison wants
every body in its component (a body left in another one is replaced
there). The report counts components, occurrences and items per component
(`components`) and names each item's component.

A component whose bodies are stored only in an `.smb` blob (no `.smbh`
history) has no features; its solids join the stored design unless one of
the last state's bodies is the same, and come in with the final
comparison. The `.smb` blobs of components with a history also hold
feature tools and sketch faces, which stay out. Occurrence items and
other assembly items change no body and are skipped (occurrence
placements come from the tree; *Items without a translation* below);
joints, as-built joints, joint origins and ground items come in as
Mitcad's (*Joints* below).

**Across components.** Mitcad's features work only on their own
component's bodies, so a feature of the file that changes another
component's bodies (the file's assembly context: a part cut by a sketch of
the assembly) goes into the component whose bodies it changes, and what it
uses of other components comes along:

- a sketch of another component is imported again into the feature's
  component, moved by the two occurrences' placements at the feature's
  point of the timeline onto a planar face of its component or a fixed
  construction plane, so it stays parametric (its dimensions keep the
  file's parameters); features at points where the placements differ get
  copies of their own;
- an extrusion up to a face of another component goes up to a fixed plane
  where that face's occurrence places it in the sketch's component at the
  extrusion's point of the timeline.

- a combine whose tools are bodies of another component than its target
  (a part cut by another part) goes into the target's component and
  takes those tools by their occurrence links (`tool_links`,
  `commands.md`, *combine*; mitcad#104): the model reads each tool in its
  component and moves it by the placements at the combine's point of the
  timeline. Such a combine that consumes its tools lists body removals
  in their components instead of body records (`selections.rs`); the
  consumed tools leave their components. Before, these combines fell
  back (5 items of `MITCAD_F3D_CORPUS` and 17 of the backup corpus, all
  but one cuts keeping their tools); all now give their states.

These need each component placed once (one occurrence on the way from the
root); else the feature falls back. A feature that changes the bodies of
two components at once is one feature per component in the file (*Parts
of items, face profiles and sketch heights* below), each replayed in its
own component. Other features that work on bodies of two components at
once fall back: the base feature takes the item's history state in each
component, and later items continue parametrically.

**Occurrence placements.** The transform the file stores for an
occurrence is where the item that made it put it; joints, captured
positions (`Snapshot` items) and rigid groups after it move it without
changing that transform. The decoder reads the occurrence's placement
after each item that placed it (mitcad#81), so the file places an
occurrence at the end of its timeline at its last one (else at its last
captured position, else at its stored transform), and that is the
occurrence's own placement in Mitcad; the captured positions also come in
as `capture_position` features at their points of the timeline
(*Joints*). The history's states are in each component's own
coordinates, so the matching does not depend on placements; only the
copies above do. Those take the placements the file has at the item's
point of the timeline (`Captures::placement_at`, mitcad#86): where the
items before it put the occurrence (else, without its placements, its
last captured position before the item, else its stored transform), not
where the occurrence starts in Mitcad. Where a joint or a move places an
occurrence elsewhere at an item's point in the timeline than the
placements say, the copy does not follow it and the item falls back.
Joints come in only where they hold where the file places the
occurrences (*Joints*); occurrence moves are not replayed (*Limits*).

The placements are given per path, in the root component. An occurrence
inside a component placed several times is one occurrence in Mitcad (in
that component) but has a path per placement of its parent: each of its
steps is made relative to the parent's placement along the step's own path
at the step's point of the timeline (`Captures::path_steps`), and those are
its placements in the parent. Before, the last path's root placement was
taken as its placement in the parent, so every copy of the parent moved it
there: in a design of 20 copies of a part (each a component holding one
occurrence of the part's body component), all 20 bodies stood one copy's
offset away from their places (41 of the design's 113 occurrence paths
misplaced; none after).

### Components of other documents (mitcad#196)

A design can place components of other documents (inserted designs,
fasteners). The `.f3d` file of the design does not hold those documents,
and a multi-design package of such a design cannot always be made, so
their files are kept beside it: next to `<design> vN.f3d` a folder
`<design> vN.refs/` holds one `.f3d` per referenced document at the
version the design uses (`<name> v<version>.f3d`), for the whole tree of
references, and `references.json`, whose tree maps each document's
version id (the URN the design names it by) to its file (format:
`core/model/src/api/commands.md`, *Components of other documents*).

**What the design stores about them** (the decoder's reading,
`core/import/SCHEMA.md` §5.2):

- The MetaStream's tail lists the external documents: per document a key
  (`u64`), two GUIDs (`str16`), the URN (`str16`,
  `urn:…:fs.file:vf.<id>?version=N`), a GUID (`str16`) and a `u32`.
  References into another document carry the key: `01 id [str8 guid] 01
  key 00 str8(guid) 00`, where `id` is the object's id in *that* document
  and the GUID names its class.
- An occurrence (`CE2913AA`) of a component of another document refers
  to the component that way: the root component of the other document
  (its `id` there). The reader took such a reference only when the id
  happened to be an object of this design as well (a root component has
  the same small id in many designs), so occurrences whose component id
  was none of the design's own were missing from the occurrence tree (one
  of five in the first design looked at). The reader now takes it by the
  key and the class GUID alone. The transform after it is the
  occurrence's placement, as for any occurrence.
- The occurrence object carries GUIDs (`str16`); the paths of captured
  positions and relationships name each level by one of them
  (`path_guids`). A level inside a component of another document is an
  occurrence of that document, named by a GUID its own occurrence object
  carries in its own file (checked on a bearing whose balls' positions an
  assembly captured).
- The external document's first GUID is in the objects of its
  occurrences too; the last one is one of the GUIDs the referenced file's
  `Manifest.dat` lists (the same for every version of that document in
  the files read), which tells the file without its name. The design
  stores no name of the document.
- Joints, as-built joints and relationships name occurrences of these
  components as of any other; their geometry inside such a component is
  named through the key as above.

**The import.** The caller (`mitcad-ffi`, `f3d_references.rs`) first finds
each document's file (`reference_folder.rs`: a file given for it; the
reference folder next to the design or one given, by `references.json`,
else another version of the same document there with a note; another
design of an `.f3z` package, see below; a `.f3d` of those folders whose
manifest lists the document's GUID) and imports it on its own with the
same options and its own references (the same folders; a document that
refers back to one being imported is not imported again; 16 levels at
most), before the design is replayed. Each document (file and design) is
imported once per import: the documents imported so far are shared by
the imports of the documents it refers to, so a document placed by
several occurrences, or referred to on several levels, is replayed once
and copied from that import for each of them (mitcad#229).

**Packages.** An `.f3z` package holds the design and the documents it
refers to, one `.f3d` per design. `Manifest.json` names the root design
(`{"root": "<file>.f3d"}`; imported when no design is chosen, else the
one with the longest timeline), and `DesignDescription.json` lists every
design with the URN designs refer to it by (`about`, with its version),
its `.f3d` in the package (`relativePath`) and its document's name
(`friendlyName`, `displayName`; `mitcad_f3d::design::package`). A
reference not given a file is resolved through that list before the
reference folders (the exact URN, then the same document at another
version, with a note), and the occurrences take the document's name
from it. Without the list the design of the package whose `Manifest.dat`
lists the document's GUID is taken (of several, the one listing the
fewest GUIDs), but never the root design or a design being imported:
an assembly's `Manifest.dat` lists the GUIDs of the documents it refers
to as well as its own, so the first design listing the GUID was often
the assembly itself (a root assembly with four fasteners replayed four
times, its bodies placed for three of the fasteners).

The replay places each occurrence of such a component as an occurrence
of an empty component (*Components* above), records them (`ComponentReport::inserted`, with the document's URN and
GUID), the GUIDs of all occurrences, and the captured positions whose
paths go inside them (`positions_inside`); then, before its first item
(`prefill.rs`), `references.rs` copies each imported document with its
history into the component standing for it
(`Document::fill_component_copy`, named after the document). Occurrences
of one document share a copy where the captured positions arrange their
insides alike (every inner occurrence either placed the same by the
positions of both, or placed by neither: each level's placement is
compared in the occurrence's coordinates, with the document's own
placement where only one has a position, `split_variants`); the others
get a component of their own, made for them first
(`Document::give_occurrence_own_component`, `place_other_component`), so
each arrangement has its own copy. Components placed more than once
inside a copy (a bearing in an assembly) have one set of inner
occurrences: where the positions put one of those differently by
different paths, the occurrence of the later path gets a copy of its
component (`Linker::separate`, `Document::make_unique`, which copies the
component with its features and the components inside it) and the
positions then name the occurrences of the copy (a GUID names one
occurrence per copy, `ComponentReport::occurrence_guids` lists them; the
one whose parent the path is at is meant). The occurrences inside start
where the design's captured positions put them: each path's levels
inside the document by their GUIDs, each placement in its parent worked
out from the placements the earlier positions gave the levels above it
(a position names the levels it moves and leaves those it does not), the
last one where the paths agree, and every placement kept for the items
between (`Captures::placement_at`). The captured positions come in as
`capture_position` features at the end. It is all one undo step with the
import.

The replay matches the design's own history, which holds none of the
copies' bodies, so while it runs the document's body view leaves the
filled components (and the components inside them) out
(`Document::set_import_scopes`, `refs::bodies_for`): matching, the final
comparison and the fallbacks see the design's own bodies only. What the
design names in another document is looked for among that document's
bodies (its `_f3d.document` GUID): the reference resolution in `refs.rs`
takes the bodies of the scope. So the items that name things inside the
copies find them as they find those of any other component:

- **Joints, as-built joints, relationships, grounded occurrences.** A
  path level inside another document is an occurrence of that document,
  named by a GUID (`Importer::occurrence_path`); a side whose frame lies
  on the copy's faces and edges goes on them (with the frame override
  where it does not), else on the copy's origin (a joint) or fixed
  geometry (a relationship) as before, counted as sides on a component of
  another document. A rigid group whose members are all inside one
  occurrence has nothing to join (it says so).
- **Bodies, faces and edges of another document named by an item**
  (decoder: *Entities of other documents* in SCHEMA.md). A body is the
  body of the copy that matches the stored body the decoder found in the
  other document's history (`_f3d.stored`, the other import's
  `bodies[].file`; `ComponentReport::external_bodies`, one per copy): a
  combine whose tools are bodies of another document
  (`Importer::external_body`) takes them by their occurrence links like
  any tool of another component. A face or edge is found by its geometry
  among the scope's bodies, so extrusions up to or from faces of other
  documents' bodies and planes on them find the copy's face.
- **Which occurrence.** A document placed more than once has an
  occurrence per placement, and the file names the entity by its names in
  the document only; what tells the occurrence is the placement of it that
  the reference's slot carries (`_f3d.context`, SCHEMA.md). Once the
  components are filled, `focus.rs` finds for each such reference the
  occurrence path of the design whose placement at the item is that
  (before the item or right after it, the design's own occurrences as the
  captured positions have them there, those inside the copies as the
  positions that placed them did): the reference is looked for among the
  bodies of that occurrence's component
  (`Document::set_import_focus`, `refs::bodies_for`), and the links of
  the features that use the body (a combine's tools, an extrusion's
  objects, a plane's faces) lead through that path, not through the
  component's only path. Where one item names two occurrences that share
  a component (a plane between the end faces of two copies of one bar),
  the later one gets a copy of its own (`Document::make_unique`), so that
  the body each reference is found on tells the occurrences apart.
  Documents that share a GUID are told apart by the decoder the same way
  (`resolve_external`). A reference that no occurrence is at (the
  occurrence is inside a document the design does not name, or the file
  has it moved since) is found as before.
- **A plane between faces of components.** A construction plane's
  definition takes faces, edges and vertices of other components' bodies
  through `links` (`commands.md`, *construction_plane*; one link per body,
  as an extrusion's `object_links`), so that a midplane between two
  placed faces is made in the plane's component's coordinates where the
  design places them, and follows when they move; before, such a plane
  stayed a fixed plane.

**Not found.** A document whose file is not found leaves its components
empty, as before; the report lists it (`missing`, or `skipped` where the
user declined to give it) and a warning names it. The application asks
for each one with a file dialog naming the document (its name where a
reference folder gives one, else the design's, and its version), whose
Skip leaves it out; a chosen file's folder is searched for the others
([app/COMMANDS.md](../../app/COMMANDS.md), *Files*). `mitcad-cli
import-f3d` never asks: `--references DIR` adds folders,
`--no-references` leaves the components empty (and the things inside them
as before: joints on the origin, sides inside left out).

**The report** (`references`): per document its URN, name, version,
file and how it was found, `status` (`linked`, `missing`, `skipped`,
`failed`), a note, Mitcad's components, the occurrences, its own import's
items by outcome and its own references; `components.linked` counts the
occurrences filled, `components.placed_inside` the captured positions
inside them that placed copied occurrences.

**Tests.** `mitcad_f3d::testreferences` writes a synthetic assembly that
places a part once and a sub-assembly (which places the part) twice, and
its reference folder; `core.f3d` reads its occurrences and documents,
`core.import` the folder's lookups, the filling on the mock kernel (a
two-document dump with a captured position inside the part), a joint
inside a copy translated in place, and a combine whose tool is a body of
the part, the occurrences of a part that the positions arrange
differently (a copy each), a component placed twice inside a copy whose
inner occurrences the positions place differently (a copy of it, the
pins where the file has them), a tool read through the occurrence its
context names, two occurrences named by one item (a copy each), and a
removal that names a cut's first record (the new piece, not the target),
`core.f3d` the slot's placement and the choice among documents that
share a GUID, `core.model` the import scopes, the operations that give an
occurrence a component, make it unique, or place another one, and a
construction plane between faces of components, and
`cli.import_f3d_references*` the import from the folder, without it (the
documents missing) and from a folder given (found by their GUIDs), and
`app.ui-windows-references` the application's dialogs (skipped, then a
file chosen whose folder holds the other).

**Corpus counts.** The 53 designs of the backup corpus that have a
reference folder next to them, imported one by one (release build; the
counts of a run without references of the same build first, then with the
folders; 128 referenced documents linked, 2 not in their folders):

| | without references | with the folders |
|---|---|---|
| items parametric / partial / fallback / skipped | 2089 / 49 / 196 / 633 | 2126 / 47 / 187 / 607 |
| captured positions inside other documents' components left out | 1794 (in 20 items) | 0 placed inside the copies: 385 |
| of them, left out because occurrences of the component put the occurrence in different places (one copy for all) | | 0 (112 in one design before a copy per arrangement) |
| components made for occurrences arranged differently, copies of components placed twice inside a copy, copies for occurrences one item names | | 9 (in 5 designs) |
| joints with a side on the origin of an empty component | 31 | 0 (on the copies' geometry) |
| relationship rows with a side inside a component of another document, left out | 61 | 0 |
| relationship sets / their rows translated | 54 / 76 | 67 / 91 |
| joints skipped | 40 | 29 |

All designs import without an error and with their final bodies as
before. What the folders gave: the 4 combines whose tools are bodies of
another document, 2 planes and 2 extrusions on faces of other
documents' bodies, joints, relationships and rigid groups inside the
copies (as joints where they hold where the file places the occurrences,
else as as-built joints or constraint rows left out with the reason).
What the copies per arrangement and the occurrence a reference is in gave
on top: the captured positions of designs that place one assembly six
times, or twice, each arranged differently inside, are placed (273
positions placed inside the copies of one design, 26 before), the
midplane between the end faces of two occurrences of a bar is a
parametric plane (it was a fixed one), and the removal after a cut that
splits its target takes the new piece: the fillets after it are
parametric (2 designs; the two bodies of the cut were counted target
first, the file counts the new piece alone). The rest, with the cause:

- Rows and joints that do not hold where the file places the occurrences
  (87 rows): the placements of occurrences inside a copy are known only
  where a captured position names them.
- An assembly of printed circuit boards names 201 of its inner
  occurrences' positions by GUIDs that no stream of the board's file
  holds (checked as text and as bytes in all of the file's streams and in
  the design's other referenced files; they stand in the assembly, its
  earlier versions and another design that embeds it): 29 of the 189
  GUIDs of the board's components are the second GUID of an occurrence of
  the board, the others name nothing the board has, and of those 157
  positions only 18 put an occurrence where one of the board's
  components is (140 do not; the board is dense, so a few millimetres of
  tolerance would pair many by chance). The assembly's positions are
  from a state of the board that the referenced version does not have, so
  they stay left out ("the other document does not have it").
- Planes and extents between faces of components of other documents
  where the file's slot carries no placement that matches an occurrence
  (an occurrence of a document the design does not name, or placed in the
  file by a step the import does not follow): found as before, among all
  the copies' bodies.
- Derived designs: the three derive items of one design name a document
  that no occurrence places, which is not in its reference folder (the
  folder lists only the placed documents), and their selections are not
  decoded; they stay skipped.
- Primitives placed on a face (cylinders, spheres): the placement is not
  decoded in any design.
- Two designs of an enclosure whose documents are placed once each keep
  two extrusion joins that change the bodies of two of the design's own
  components (a lid and a base): the replayed result has other faces in
  both components than the file's state, so they fall back. They name no
  entity of another document; the fix is in the extrusion, not in the
  references.

### Sketches on other components' faces, linked curves as projections (mitcad#225)

A sketch may lie on a face of a body of another component (a sketch of
the root on a circuit board's face), and its linked curves (projected
edges and faces, *Sketch curve flags* below) follow edges of its own
component or of others. The model keeps both through occurrence links
(mitcad#100, `commands.md`, *Geometry of other components*); the import
uses them (`linked.rs`):

- **The plane.** A sketch whose component the decoder gives, and that no
  planar face of its own component carries, lies on the planar face of
  another component's body, seen through one of its occurrences (those of
  components of other documents, mitcad#196, included), that is in the
  sketch's plane: the only one looking along the sketch's normal, else
  the only one against it. It comes in on that face with a `plane_link`
  (the occurrence path from the root to the face's component, and the one
  the sketch's component is seen at: its first path) instead of a fixed
  plane; the note names the component and occurrence. Two such faces
  (two parts' faces at one height) leave the fixed plane.
- **Projections.** The file does not name what a linked curve follows,
  so the replay is searched: the edges of the sketch's own component and
  those of every other component's bodies through each of their
  occurrences (bodies whose box does not come near the sketch left out),
  moved into the sketch's component. A linked line, circle, arc or
  ellipse that exactly one edge projects onto (within the snap's 2e-3 mm;
  the edges in the sketch's plane first, as the file projects the face a
  sketch lies on) becomes a linked projection of that edge: with a `link`
  when the edge is another component's, without in its own. The edge's
  name must give one curve, and the curve must already be where the
  model's projection puts it (the snap put it there: the choice of edge is
  made once, and the snap puts the curve on the chosen edge's projection
  first, mitcad#227; a line the edge runs the other way round is turned),
  so following it moves nothing on import. The sketch is tried with the
  projections first, then, at each level, without them.
- **Kept fixed.** A linked curve no edge or several edges project onto
  (a wall's top and bottom edges seen along it), a spline, or one whose
  point the snap could not move (shared with a curve put elsewhere) stays
  fixed reference geometry, and the sketch's note says so: "n linked
  curves as projections (k of other components); m linked curves kept as
  fixed reference geometry (no single edge of the replay they follow)".
  A copy of a sketch in another component (*Components*) keeps its linked
  curves fixed.

A linked circle keeps where the file starts it (its `start_angle`, the
seam of what is made from it) when it follows its edge (`commands.md`).
A sketch with such links stays in its own component (the bodies its
links name are not its component's).

Six assembly designs imported one at a time (with their reference
folders): sketches that stood on a fixed plane because their face was not
found, now on a face of another component; linked curves now projections
(of other components / of their own component) and those kept fixed. The
item outcomes and modelling inputs are the same as before in all six.

| Design | Sketches on another component's face | Linked curves as projections | Other / own component | Kept fixed |
| --- | ---: | ---: | ---: | ---: |
| `e3c34193` (a circuit board with a part around it) | 1 / 1 | 525 / 537 | 499 / 26 | 12 |
| `c3e08097` | 5 / 10 | 192 / 266 | 58 / 134 | 74 |
| `48ff426e` | 9 / 9 | 428 / 470 | 126 / 302 | 42 |
| `52ba7a15` | 5 / 8 | 400 / 450 | 192 / 208 | 50 |
| `bd57969c` | 4 / 9 | 191 / 265 | 57 / 134 | 74 |
| `db73863f` | 2 / 4 | 92 / 193 | 32 / 60 | 101 |
| all | 26 / 41 | 1828 / 2181 | 964 / 864 | 353 |

The sketches left on fixed planes have no planar face of any replayed body
in their plane at their point of the timeline (in the two related
designs `c3e08097` and `bd57969c` the replay has no bodies of other
components there yet). The linked curves left fixed have
several edges projecting onto them (a wall's top and bottom edges seen
along it), none (splines, edges of bodies not replayed), or a point the
snap put elsewhere.

### A revolution's touching regions, and a snap that keeps the sketch's regions (mitcad#227)

The example design's Revolve3 (a 360° cut of two thin regions that share
an edge: the wedge between a wall of a cylinder and a cone and the line
across both) fell back with 1077 mm³ cut where the file cuts 1050 mm³. The
ticket named the snap of linked curves (a line put on an edge 2 µm away,
splitting the profile); on main after mitcad#224/#225 that no longer
held: the solved sketch is the file's numbers to 1e-15 mm (the snap moved
nothing), its two regions are the file's two profile regions exactly
(1.454 and 4.846 mm²), and the cut failed on the file's own stored body
too. The cause was the revolution: each region revolved on its own gives
the file's volume (245.655 + 804.642 = 1050.297 mm³), but the two solids
fused and simplified (`BRepAlgoAPI_Fuse::SimplifyResult`) merged the two
parts of the cone face (the line the shared edge cut in two) into one
face built wrong (the fused solid alone was 1077.658 mm³, its centre of
mass off the axis). Regions that touch are now united as one planar face
before the turn (`geometry/src/revolve.cpp`; the straddling profiles did
so already), so no shared face is left for the fuse to merge; geometry
test `test_touching_regions`.

The snap (`snap.rs`) now follows the ticket's rules:

- **One decision per curve.** The projection rule's choice of edge
  (`linked::choices`: the distinct edges whose projection the curve lies
  on, those in the sketch's plane first, the curve follows the only one)
  is made once; the snap puts the curve on that edge's projection first,
  then on any other fitting edge whose projection keeps the points
  already placed (two edges' projections may differ by nanometres), and
  the projection follows the same edge.
- **The regions stay the file's.** A curve is moved only when the curves
  it meets, and how (a crossing inside both, an end of one on the other),
  stay those of the file's numbers, for the curve and for every curve the
  move touches: points the file holds coincident move along, points held
  on the curve go with it (the solver takes them), corners two curves
  share are the solver's and not counted. A curve whose contacts would
  change stays where the file stored it; the note says "n left where the
  file stored them: a snap onto the model's edge would change the
  sketch's regions" and the trace names the contacts. (Checking every
  curve a point of which had moved in any earlier snap, instead of the
  curves this move touches, cost 0.5 s on a 166-curve sketch; scoped, it
  costs nothing measurable.)

The example design (150 items, 78 of them occurrences and one a component
insert that place components without geometry of their own):

| Group | Target | Achieved |
| --- | --- | --- |
| Occurrences | placed as in the file | 78 / 78 (the final bodies the file's, volumes within 0.002 %) |
| Modelling items (71) | parametric | 68 / 71 (before 67 / 71) |
| their inputs | read from the bytes | 54 / 54 (0 inferred, 0 unknown; before 53) |
| Sketches (14) | imported, on the right plane | 14 / 14 |
| linked curves as projections | | 525 / 537 (12 kept fixed: several edges project onto them) |
| Suppressed in the file (4) | definition imported as suppressed | 2 / 4 |

Left, each with its cause:

- **Fillet9** (fallback, also on the file's own stored state): a 0.9 mm
  rounding of the 135° edge between Chamfer3 (0.5 mm) and a cylindrical
  band beside it only 0.300 mm high (its seam; C-Pattern2's slots cut it).
  The rounding needs 0.373 mm of the band (r·tan 22.5°), so the ball must
  run over the band's far edge onto the blend face beyond; OCCT's rolling
  ball with the port's overflow patches fails ("the radius may be too
  large"); 0.7 mm builds. The overflow class of mitcad#121 (a narrow band
  bounded by a blend face and cut by pattern slots), not this ticket.
- **Extrude4, Extrude5** (suppressed in the file, placeholders): the
  decoder gives Extrude4's two-sided extent only as guesses (its extent
  records are not decoded), and a suppressed item has no history state to
  check guesses against; Extrude5 cuts the body Extrude4 would make.
  Decoding the two-sided extent records would bring both in.

The 37 designs whose earlier reports had an extrusion or revolution not
parametric (the designs the snap could touch), imported with main's
import and with this one, item outcomes and the sketches' notes compared:

| Group | Target | Achieved |
| --- | --- | --- |
| Sketches whose regions are the file's | all | no sketch's outcome or regions changed (502 sketches; 0 curves left where the file stored them) |
| Curves snapped onto the model's edges | as before or more | 2162 / 2159 before (one design 42 / 49: edges the projection rule prefers place a shared point elsewhere, its projections 150 / 149) |
| Linked curves as projections | as before or more | 3858 / 3857 |
| Items parametric | as before or more | 1575 / 1574 (Revolve3 of the example design's other copy; nothing lost) |

### Parts of items, face profiles and sketch heights

Three readings of mitcad#169 (a design whose base body is moved into a
component of its own after a lid's first body was extruded from a sketch
on it):

- **Parts of items in other components.** A feature that changes the
  bodies of several components (screw holes cut through a lid and the
  base under it) is one timeline item, owned by one component, and one
  feature more of the item's class in the feature list of each other
  component it changes, with its own inputs (the same profile, that
  component's bodies), its own item tail and its own operation in that
  component's history, which no timeline item names. The design's table
  of item parts (class `12D00AC0`, `classes::ITEM_PARTS`) lists each such
  item with its features. The decoder gives every part with a result as
  an item of its own right after its item, numbered after the timeline's
  items, named after the item, with `_f3d.part_of` (the item's index) and
  its own `component` and `result_no`; the import replays it in its
  component like any item, at its item's point of the timeline
  (`Captures::timeline_at`: placements, stops). The item itself, when its
  own component's bodies did not change (result number -1), is skipped
  with a note naming its parts. Before, the parts' operations were states
  no item names, which a base feature brought the replay through before
  the next item of that component. Corpora: every off-timeline extrusion
  with a result of its own is such a part (`mitcad-f3d-inspect
  off-timeline`): `MITCAD_F3D_CORPUS` 5 extrusions in 2 designs, the
  backup corpus 69 in 11 designs, of which 12 (7 designs) are parts of
  extrusions and given as items, and 57 (4 printed-circuit designs) parts
  of a PCB item, which has no translation; besides, a fillet, a chamfer
  and a revolution are given as parts too, and the threads of holes
  (parts of another class) are not.
- **Faces in an extrusion's profile.** The profile slot (role 65) can
  select planar faces of bodies besides or instead of sketch profiles; the
  decoder gives each as a face reference among the extrusion's
  `profile`s (it takes one of the selected profile entries), found in the
  history by its names like the face of an extent. Faces together with
  sketch profiles are projected into the extrusion's sketch (linked; a
  face of another component through the occurrence placing it once,
  which needs the face found in that component's history, mitcad#161),
  and the sketch's regions on them join the selected regions; only into a
  sketch no other feature uses yet, since the projections split its
  regions. Faces alone go into one sketch on the first face, before the
  faces guessed by their areas. Corpora: `MITCAD_F3D_CORPUS` 28
  extrusions of faces only and 3 with sketch profiles too, the backup
  corpus 87 and 10.
- **Sketches lying off their frame.** A sketch on a face can keep its
  points at the face's height in its own coordinates while its stored
  frame lies in a parallel plane through the origin (in the design
  above, 1.6 mm below the base's top face, so the lid's first body came
  out 1.6 mm too low). Where more than half of a sketch's points share
  one height off the frame, the frame moves there (`plane_height`). Four
  files of the corpora have such a sketch, all the same design's.

Coverage, the 56 files of both corpora these touch, before and after
(`--time-limit 300`; one large design left out): parametric 2734 → 2777,
fallbacks 647 → 626, history matches 1642 → 1685, base features that
bring the bodies to a state 89 → 81, no item that came in parametric
before falls back. One part falls back where its operation was a state
no item named before ("does not cut into any participant body"). The
design above: its two parts parametric and the base feature before the
base's second fillet gone; the lid's first body still falls back until
the face its profile selects, on the root's body, is found in another
component's history (mitcad#161); with that lookup it comes in
parametric (no fallback left, 66 items parametric, 38 history matches).

### Joins that merge bodies, tapers that close, symmetric sketch patterns

Fixes from re-importing designs the last census left non-parametric. Over
the 37 re-imports of the designs these touch: parametric items 1522 →
1713, fallbacks 304 → 121, partial items 13 → 5; 25 imports improved and
none got worse (the final bodies of all still agree with the file's).

- **Bodies a join merged stay in the history.** A join that merges bodies
  into another (an extrusion, revolution or sweep joining several
  participant bodies) leaves the merged bodies in the ASM history's state
  as they were, as a combine's consumed tools do (above): the same stored
  body id, the same solid, though the design no longer has them (the API
  reference dumps of such designs list one body less after the join). The
  candidate's result is matched against the state without such bodies too:
  those of the state before it that are unchanged and that no body of the
  candidate's result is (`history::without_merged`). A match retires them
  from that state on (`Oracle::retire`), so that the later items and the
  final comparison do not expect them; the merged volume must be in the
  result body, so a candidate that did not merge them does not match this
  way. Before, such an item fell back to the stored bodies, and the items
  after it that built on them often with it: one design 112 → 173
  parametric items (77 → 16 fallbacks), another 92 → 127, a third 104 → 106.
- **A face profile keeps its taper.** An extrusion of a body's faces
  (*Parts of items, face profiles and sketch heights* above) gave the
  sketch of the face's projected boundary an extent without the file's
  taper; the decoded faces' candidate now takes the taper too (the
  candidates found by the faces' areas are for untapered prisms, and stay
  so). Its provenance is `read` where the stream's extent kinds say the
  distance goes along the sketch's normal.
- **Tapers that close the profile** (geometry kernel, `tapered`): the
  stored designs end a tapered extrusion where its sides meet when that is
  before the end of its distance, instead of failing: a hexagonal socket
  becomes a pyramid, a round pocket a cone, and the volume agrees with the
  API reference dumps (the pyramid's `A h / 3`). The sides of a
  convex profile of lines are intersected as half-spaces (a pyramid, or a
  roof with a ridge for a rectangle), a circle closes into a cone; the far
  cap is gone. OCCT's draft fails on the first and loops on the second. 13
  imports of 3 models (one keeps a thread that falls back for another cause)
  went from a fallback to a parametric extrusion; a profile with holes or
  arcs among its lines that a taper closes stays an error.
- **Three points on a line.** A collinear constraint whose entities are
  three points (a centre line's ends and the origin it passes) is a
  coincidence of the third point with the sketch's line through the other
  two. The API reference dumps cannot read such a constraint (an
  assertion on the geometry count), six have it, all the same sketch
  shape; 4 imports' sketches came in whole.
- **Symmetric rectangular patterns.** The stream's rectangular pattern
  flags are (distance kind, symmetric in direction one, symmetric in
  direction two): all 41 patterns of the API reference dumps agree with
  `distanceType`, `isSymmetricInDirectionOne` and `…Two` (two are symmetric
  in direction one, with 5 and 11 instances; none is in direction two). A
  symmetric direction has the originals as its middle instance, so the
  copies lie on both sides: `patterns[].symmetric` in the sketch (model,
  [commands.md](../model/src/api/commands.md), *sketch*), the copies'
  indices counting from the end against the direction. An even count
  stays an error. 3 imports' sketches came in whole.
- **An offset without curves** (the stored list of its offset curves is
  empty: the curves were deleted) binds nothing and no longer makes its
  sketch partial (1 import).

### Joints

`joints.rs` translates what the decoder reads of joints, as-built joints,
joint origins, rigid groups and ground items (mitcad#55, mitcad#81,
SCHEMA.md §5.3) into Mitcad's joint features (`commands.md`, *Joints*),
and `captures.rs` the captured positions (mitcad#75). Their occurrence
paths (object ids from the joint's component down) become paths of the
occurrences the tree made, those of components of other documents
included (fasteners, inserted parts: empty components, *Components*); a
joint of a level inside a component of another document is left out with
that reason.

**Where the file places the occurrences.** The transform the file stores
for an occurrence is where the item that made it put it; joints, captured
positions and rigid groups after it move it without changing it. The
decoder reads each occurrence's placements after every item that placed
it (`_f3d.placements`, mitcad#81): the occurrences start at their last
ones (else at their last captured positions, else at their stored
transforms), and after each joint item every occurrence must be where it
was before the item, where the file places it after the item, or where it
ends (1e-6 of the size, 1e-7 in rotation), else the definition is taken
back and the next one tried.

- **Joints.** Side one is origin `a` (it moves onto side two, as `a` does
  onto `b`), side two `b`. The file's relation is Mitcad's `frame_a =
  frame_b · motion(values) · Rz(angle) · Tz(offset) · flip` with the
  angle and z offset parameters as they are, `flip` the file's flip (its
  `opposed` byte: the frames' z axes opposed), and the x and y offsets
  moving `b`'s origin within its plane (numbers). The joint test model
  (the reference model `joint_kinds` under `MITCAD_F3D_MODELS`: every kind at known values, flipped and
  not, mitcad#81) settles the senses: Mitcad's values are the file's
  (turns, slides along x, y or z by the motion's slot, a cylindrical
  joint's turn and slide, a pin-slot's turn and slide along x, a planar
  joint's x and y slides and turn); a ball joint's file values are
  `Rz(pitch) · Rx(yaw) · Rz(roll)`, Mitcad's `Rz · Ry · Rx`, so its
  values differ while the placements agree.
- **Checked before it is added** (mitcad#87). The values the motions have
  where the file places the occurrences (at the marker, else right after
  the item) come from the relation (`mitcad_model::joints::motion_values`)
  with the sides' frames; where it does not hold there, no definition is
  added (the note says how far side one is from side two) and the joint is
  kept as an as-built joint at once. Where it holds, one definition is
  added: with the file's limits (rotation limits bound `rz`, slide limits
  the slide; planar and ball limits are left out, which motion they bound
  is not decoded) when they hold the values, with the values as the
  joint's `position` where the rest value is another; else without the
  limits (`its limits left out (outside them: rz is … there)`). A joint
  item takes one recompute instead of one per definition tried and its
  undo.
- **Offsets the file stores rounded** (the owner's decision in mitcad#81):
  where the relation holds but for the offset along the joint's z, by at
  most half a unit of the last digit of the offset parameter's expression
  (`-2.646 mm`: 0.0005 mm; expressions with decimals only), the parameter
  keeps its name and takes the offset the placements give (`its offset …
  from the placements`). The joints of the corpus that seemed rounded
  hold with the stored offset at the placements after them (the stored
  transforms are older): none needs it.
- **Sides.** The frames are the file's stored ones, in the components the
  paths end in. A side goes on geometry of that component at the joint's
  point of the timeline that gives the frame, with a `frame_override` for
  the parts it does not give (a planar face's origin is its middle, so a
  key point elsewhere on it is an override): a joint origin the side
  names, construction geometry its key point or direction names (the
  component's origin point, planes and axes, imported construction
  features), circular edges centred on the frame's origin along its z
  axis, planar faces through it across z, faces of revolution about z
  (edges first when the key point is on an edge; at most eight). The
  faces and edges come from one pass over each body
  (`Kernel::face_geometries`, `edge_geometries`, mitcad#87) instead of
  finding each by its name. Each is checked by resolving it as the joint
  will (`joint_frame`). Else the side is a fixed plane with the frame, and
  the joint is `partial`. A side on a component of another document (a
  fastener's joint origin) goes on the origin of the empty component
  standing for it, with a `frame_override` where the file's frame there is
  not the identity.
- **Kept as as-built joints.** A joint that does not hold becomes an
  as-built joint of its kind at the placements at its point of the
  timeline (`relative` from them; the motion frame side two's), else a
  rigid one, with a warning and the reason; the item is `partial`. A
  joint whose motion is not decoded is left out.
- **As-built joints.** `relative` from the placements at its point of the
  timeline. The file records the joint's frame in each occurrence's
  component; where those frames disagree with the placements a warning
  says so (the placements are kept). Kinds with motions take the recorded
  frame as their motion frame, on geometry of occurrence two's component
  as for a joint's sides, else of occurrence one's, else fixed on
  occurrence two; their limits where the recorded placement is the
  file's (Mitcad's values are 0 there) and the limits hold 0, with 0 as
  the position where the rest value is another.
- **Rigid groups** (mitcad#81; the file stores them as as-built joints of
  motion type 11). A `rigid_group` of the occurrences of the group's
  component its members are or are in: members below the top level
  ("include children" lists the members' children too) and inside
  components of other documents move with the occurrence they are in. A
  group with the component's own geometry (an empty path) joins its
  occurrences to it by rigid as-built joints.
- **Joint origins.** A `joint_origin` of the owning component with the
  file's angle and z offset parameters on geometry that gives its frame
  before them (the component's origin point for the designs read, a face
  or edge as above, else a fixed plane), checked against the file's frame
  (`datums`); else the file's frame fixed without the offsets (`partial`).
  Later items can use it as a joint's side.
- **Ground items.** The occurrence's ground flag (the decoder also sets it
  in the occurrence tree); the item is `parametric` without a feature.
- **Captured positions.** A `capture_position` feature in each component
  whose occurrences the item places, each occurrence at its placement in
  its parent: the file gives the placement of each path in the item's
  component, made relative to the parent path's placement (the same
  item's, else the last captured or stored one). A captured position of
  joints' values stores parameters instead; its positions are those the
  occurrences' placements name at its index (so are those of positions
  with an empty path). A path level the decoder
  does not find (some designs name a sub-assembly's occurrence by an id no
  occurrence has) is the one occurrence placing the component the next
  level sits in; its id then stands for that occurrence in the item's
  other paths. Positions inside components of other documents and of
  grounded occurrences (Mitcad does not move those) are left out with a
  note (`partial`). The features are not checked against the end
  placements: they are the file's placements at their points.

The report counts them (`joints`: as joints, their sides on fixed frames
and on components of other documents, kept as as-built joints, as-built
joints, joint origins, grounded, captured positions, rigid groups, left
out). With `MITCAD_IMPORT_TRACE` each joint item traces its time (`a
joint in 0.005 s (sides 0.004 s, 1 definitions added)`).

In the corpus (10 designs with joint items or captured positions; every
joint is rigid, nearly every one joins a fastener or another part
inserted from another document):

| | Joints | As-built joints | Rigid groups | Captured positions | Joint origins |
|---|---|---|---|---|---|
| mitcad#81 | 9: 8 in (6 sides on fasteners and inserted parts), 1 inside another document | 3: all in | 10: 7 in, 3 whose other members are inside another document | 15: 6 in, 8 in without positions inside other documents, 1 left out | 6: all in |
| Before | 9: 5 in, 3 kept as as-built joints, 1 inside another document | 13 (the rigid groups among them, as rigid as-built joints of their first two members): 10 in, 3 inside another document | not decoded | 15: 4 in, 8 in without positions inside other documents, 3 left out | 6: all in |

The joints kept before as as-built joints held where the file places the
occurrences after them: two seemed 4.5e-4 mm off (a rounded offset), one
fastener 12 mm off, against the transforms the file stores, which are
older than the joints. In the corpus a joint item now takes at most 0.02
s (0.05 to 0.18 s before); on a large design (199 timeline items, a body
of 1239 faces and 2981 edges) with twelve rigid joints onto its own
geometry added to it, the first that holds took 0.22 s (reading the
component's faces and edges), the others that hold 3 to 5 ms and those
that do not 1 to 3 ms (2.3 to 3.0 s each before: each definition tried
added, recomputed and taken back, and every face and edge found by its
name).

### Assembly relationships

`relationships.rs` (mitcad#155) brings the file's assembly relationships
in as Mitcad's constraint sets (`commands.md`, *Assembly constraints*):
what the decoder reads of them (`core/f3d/src/design/build/relationships.rs`,
SCHEMA.md §5.3 `GeometricRelationship`, `TangentRelationship`).

**What the file stores** *(read from the corpora's relationship items: the
occurrence placements the file stores after each item satisfy the decoded
rows)*. A relationship item holds one or more constraints, each a row: the
two sides' occurrences (by their placements), the faces or edges picked
on each side, the points picked on them, an `Offset` or `Angle`
parameter, and a constraint whose data gives the kind of each picked
entity (a point, an axis, a plane or a circle) with its frame in the
component the side's path ends in, and a sense (the frames' z axes the
same way, opposed, or either). Older files keep the same data in the
constraint itself (its class versions 3 to 6) with type codes one
higher. Where the file places the occurrences, two planes hold `(o2 −
o1)·z1 = offset` (side two's plane that far along side one's normal), a
point or an axis on a plane `(o1 − o2)·z2 = offset`, two axes are
collinear, two circles share their axis with their planes `offset` apart,
a centring's two pairs of faces share their middle plane, and an angle's
axes are at its angle. Of the 282 rows of the corpora (75 in the 37 items
of `MITCAD_F3D_CORPUS`, 207 in 109 items of a second corpus, a backup of
designs), 183 have both sides in the occurrence tree: 125 hold where the
file places the occurrences right after their item, 24 after a later
item that moved them, 34 never (in four large assemblies whose parts
later items moved or replaced). The other 99 have a side inside a
component of another document or on an occurrence that is not in the
occurrence tree. A frame stored as none is the identity (a circle
centred on its component's origin holds so).

**Translation.** One `constraint_set` per item in its component, a row per
constraint (side two `a`, side one `b`; a point or an axis on a plane is
`a`, the plane `b`): two planes or a plane and a circle `align`, a point
or an axis on a plane `align` with the offset above it, two axes (or an
axis and a circle) `align` (concentric), two circles `insert`, a
centring `center` (`a`, `a2` side one's faces, `b`, `b2` side two's), an
angle `angle`. Each frame goes on geometry of the side's component at the
item's point of the timeline that gives its plane, axis, circle or point
(planar faces through its origin across z, cylindrical and conical faces
about z, circular edges centred on it, spheres centred on it; the one
nearest the point picked on it first), else on fixed geometry with the
frame (the item `partial`). A side on a component of another document
(an empty component here) is fixed geometry of it with the file's frame;
a circle there has no edge to go on and the row is left out, as is a
side inside a component of another document. `flip` and the sign of the
offset come from the references as Mitcad resolves them where the file
places the occurrences (the `constraint_row` query's measures): planes,
circles and axes must be parallel there (within 1e-5 rad) and the offset
the parameter or its negative (within 1e-4 mm); the offset stays the
file's parameter.

**Checked when added.** The set is added; every occurrence must stay
where it was, where the file places it after the item, or where it ends
(within 1e-4 mm and 1e-5 in rotation: the file's placements and frames
agree to a few 1e-5 mm), else the rows are added one by one and those
that move an occurrence, or that the joint solver cannot hold together
with the earlier ones, are left out with the reason. The report counts
the sets, rows, rows translated, their sides on fixed geometry and on
components of other documents, the tangent relationships and the rows
left out by reason (`relationships`).

**Tangent relationships** (the decoder's `TangentRelationship`, a contact
between two bodies). A `tangent` row between the faces of the sides'
components that the file's points lie on, where they are planar,
cylindrical or spherical (Mitcad's tangent rows take those), `flip`
tried for a contact inside; else left out with the reason. The three of
the corpora pick a plane and a cylinder: in one design they touch where
the file places the occurrences right after the item (gap 0), in the
other (two versions) they overlap by 0.36 mm there; later items move the
parts apart (31 mm and 6.6 mm at the end), so none holds where the file
places them at the end and all three are left out with those gaps in
the note.

In the corpora (every relationship file, `--time-limit 60`):

| | Items | Rows | As rows | Left out: inside another document | does not hold at the end placements | occurrence not in the tree | circle on another document | the solver cannot hold with the others |
|---|---|---|---|---|---|---|---|---|
| `MITCAD_F3D_CORPUS` | 37 relationships (15 sets: 11 parametric, 4 partial), 1 tangent | 75 | 24 (29 sides on components of other documents) | 30 | 21 | 0 | 0 | 0 |
| Backup corpus | 109 relationships (53 sets: 35 parametric, 18 partial), 2 tangents | 207 | 74 (100 sides on components of other documents, 2 on fixed geometry) | 52 | 51 | 17 | 5 | 8 |

Before mitcad#155 every relationship item was skipped. The rows the
solver cannot hold with the others fix one rotation twice where the
file's frames differ by a few 1e-7 rad (the solver's tolerance is 1e-10
of the size).

### Items without a translation

The decoder names timeline items whose own data it does not read by
their class (`core/f3d/src/design/classes.rs`, `ITEM_TYPES` and
`ITEM_CLASSES`), and the report says what each does:

- **No geometry**, skipped at once (they never wait for a fallback):
  occurrences (`Occurrence`, the item of a new or placed occurrence,
  named after its component), components inserted from other designs
  (`ComponentInsert`, `Fastener`), pasted and derived occurrences
  (`CopyPasteOccurrence`, `DerivedInstance`), patterns of occurrences
  (`RectangularOccurrencePattern`, `CircularOccurrencePattern`: their
  copies are occurrences of the tree, each placed by its stored transform;
  Mitcad has no pattern of occurrences, so they do not follow the
  pattern's quantities and distances), motion links and contact sets
  (assembly relationships and tangent relationships come in as
  constraint sets: *Assembly relationships*), assembly contexts
  (`Context`),
  canvases, timeline groups (`Group`, named by their own name; their items
  follow them) and mesh bodies (`MeshFeature`; meshes are left out).
- **Changes bodies, not translated**: the history states stand in for
  them (fallback), with the reason in the report. Sheet metal flanges
  (`FlangeFeature`) the decoder does not read (*Sheet metal* below),
  embosses (`EmbossFeature`), electronics boards
  (`PCBFeature`), moved faces (`MoveFaceFeature`) and drafts
  (`DraftFeature`), whose inputs are not decoded; components mirrored into new ones (`MirrorComponent`) and bodies derived
  from other designs (`DerivedContext`). Copies and cuts of bodies
  (`CopyPasteBodies`) come in as Mitcad's except a cut whose body the file
  lost. Bodies moved into a new component
  (`ComponentFromBodies`) come in as Mitcad's where the history settles
  them (*Components*), else so.
- Lofts of another class are `LoftFeature`s, translated (*Sweeps, pipes
  and lofts*); points of another class are `ConstructionPoint`s, skipped
  without decoded geometry.

Items of classes still unknown keep the type `?<default name>`
("unknown timeline item").

### Sheet metal

Sheet-metal items (mitcad#211) of the `.f3d` and the `.ipt` import
(`src/sheet.rs`) come from the decoders as the dump's sheet items
(`SCHEMA.md`, *Sheet metal*) and replay as Mitcad's native sheet
features (`commands.md`, *Sheet rules and base Face*, *Folded sheet
features*), each checked against its own history state as any other item:

- **A face** (the `.f3d` base flange, the `.ipt` face that makes a new
  body): `sheet_face` of its sketch's regions (those its decoded loops
  select, else those of its measured profile), the material on the side of
  the sketch's plane the decoder gives (the reference side `front` along
  Mitcad's sketch normal, `back` against it), with the rules: the first
  face of a rule of the file makes a `sheet_rules` feature named
  `Sheet rules <rule>` in the face's item (its own timeline row, before
  the face), and later faces of that rule and side use it. The rules'
  values are the file's parameters (the thickness parameter, the bend
  radius and reliefs as expressions of it), so a changed thickness
  rebuilds the sheet. The rule's inputs the decoder did not read (the
  relief shapes; the corner relief's depth, which the files have no field
  for and Mitcad does not use) make the item `inferred`, with a note.
- **A flange on one edge** (`sheet_flange`): the `.f3d` edge flange's edge
  is found in the replay by its geometry (its names found in the
  history), and `sheet_reference` names the panel boundary it lies on;
  the flange turns towards the side of the face the edge was selected on,
  to the other side when its flip byte is set; position (`inside`,
  `outside`), length datum (`inner`, `outer`, `tangent`), angle, length
  and radius are read (`read`). The `.ipt` flange's edges are read by
  their topological names (mitcad#242, `core/ipt/README.md`, *Topological
  names*): the decoder finds each in the ASM history state the flange
  starts from and gives its ends; the import finds the edge of its replay
  there and `sheet_reference` names its boundary. Its position and length
  datum are told from the stored path (which must start towards one of
  the edges read) and the length's points (`inferred`: the file's own
  fields for them are not found); a width extent of the file (offsets from
  the edge's two ends) becomes the flange's `offsets`, the start offset
  the one at the boundary's axial start.
  An `.f3d` flange on several edges is one `sheet_flange` per edge in its
  item. Bend position `tangent` (4, the bend tangent to the side face) is
  Mitcad's position `side_tangent` and height datum 3 its length mode
  `outer_tangent` (mitcad#242: both measured from the bend's outer extent,
  defined up to the 180-degree hems) *(verified: the hems on two edges and
  the 90-degree flanges of both designs give their states)*. Widths are
  measured from the edge's midpoint: `EdgeWidth` both ways, `EdgeWidth_1`
  towards the file's edge's start point and `EdgeWidth_2` towards its end,
  as the flange's `width` (`middle`), which may be wider than the edge
  *(verified: the partial flanges of both designs, one wider than its
  edge, give their states)*.
- **A cut** (`.ipt`, mitcad#242): `sheet_cut` of mode `straight` (the
  file's cut with neither of its two sheet options set goes straight along
  the sketch's normal, through a bend too) with the file's extent and
  direction, when it goes into the material at least the thickness
  *(verified: the two cuts reaching a bend give their states, one within
  7e-7 of the volume)*. Where it crosses a bend the model keeps it as an
  overlay of the sheet. A cut with its slot-31 option (across the bends,
  mitcad#280) is a developed `sheet_cut` with every bend opened, the panel
  whose surface holds the sketch staying *(verified: the one such cut)*;
  one with the slot-33 option is not translated.
- **Ordinary items on a sheet** (mitcad#242): an extrusion, hole, mirror,
  library feature or fallback whose result is the sheet's material changed
  through its thickness keeps the sheet valid (the model's sheet-domain
  adapter, `commands.md`), so the sheet items after it replay natively.
- **Corner rounds and chamfers** (`.ipt`): each edge set's edges read by
  their names (mitcad#242) and found in the replay as an external dump's
  fingerprints are, each a corner of a panel's contour by
  `sheet_reference`, as `sheet_corner_round` / `sheet_corner_chamfer`
  (`read`). An edge not found by its name makes the item fall back with
  the edge and the reason; the history state is not searched for others.
- **Bends** (`.ipt`, mitcad#242, mitcad#280): its two edges read by their
  names and found in the replay. Edges of parallel panels of two sheet
  bodies with the bend record's kinds 3 and 5 are a double bend:
  `sheet_bend` of port mode `double`, two 45-degree bends, the radius its
  parameter, the fixed edge the one the stored axis and direction point
  away from its panel's material, the body the file's bend works on kept
  *(verified: the corpus's four bends, each joining a strip of a joined
  face to a flange's wall)*. Edges of panels of one sheet body that meet at
  no seam would close a loop and are refused; other kinds, and a single
  bend between two bodies (its angle and sides not decoded: none in the
  test files), are not translated.
- **Folds** (`.ipt`, mitcad#280): enumeration 1 with both first booleans
  false is `sheet_fold` at position `edge` on the sketch line lying on the
  decoded line, of the panel whose surface holds it, the material right of
  the line (seen along the sketch's normal) staying and the other side
  turning against the sketch's normal; other options are not decoded
  (`core/ipt/README.md`).
- **A face joined to the sheet** (`.ipt`, mitcad#242): its material lies
  along the decoded direction from its sketch's plane, against it for the
  plate's side 1. Where that slab is a panel's (the sketch on the panel's
  front or back surface), the face is a `sheet_face` joined to the sheet
  body, which adds its regions to the panel it touches (`read`) *(verified:
  the two joined faces the corpora reach with a valid sheet; #211 took
  them for double bends because it did not read the side)*. A face whose
  bend part lists nothing and whose material lies in no panel's plane
  touches no material of the sheet: its regions are new sheet bodies with
  the sheet's rules (mitcad#280) *(verified: the corpus's joined face of
  two strips, which double bends join to the sheet after it)*. A face bent
  onto the sheet (its bend part's items not decoded) falls back with that
  reason. No pairs of edges are offered as candidates.
- **Holes** through a sheet: Mitcad's simple hole through a planar panel
  cuts the sheet's material domain (`commands.md`, *hole*), so the body
  stays a sheet for the items after it.
- A sheet item on a body whose sheet is no longer valid (an earlier item
  fell back and its base feature replaced the body, or an ordinary
  feature changed it) is not translated: it falls back, its note naming
  the item that first took the sheet meaning away (`its sheet body … is no
  longer a valid sheet: <item> took its sheet meaning away`; the sheet
  state keeps that first item, `commands.md`, *sheet_state*). It is never
  replayed as an ordinary cut, rounding or chamfer instead.

The stream's records (*From the descriptions to the decoder*, cadmpeg's
*BaseFlange* and *EdgeFlange* layouts; `core/f3d/src/design/build/sheet.rs`):

| Record | Fields read |
|---|---|
| Base flange (`7AFA0A65…`, `Face<n>`) | its profile's sketch and loops; its thickness (f64 at offset 123, the rule's) *(verified: both designs)* |
| Edge flange (`7AFA0A65…`, `EdgeFlange<n>`) | found by the marked angle and height owners `p` (the holders of `FlangeAngle` and `FlangeHeight`): before them `u32` bend position (1 outside, 2 inside, 3 adjacent, 4 tangent to the side face), `u32` edge count, a wrapper reference per edge, the settings reference, `u32` height datum (1 inner, 2 outer, 3 tangent to the bend's outer surface *(verified, mitcad#242)*); after them a `u32`, at `p + 26` a flip byte (0, 1) *(verified: two flanges of each design, one flipped, give their states)*, at `p + 37` the inside bend radius (f64, cm); width owners `EdgeWidth`, `EdgeWidth_1`/`_2` (distances from the edge's midpoint, mitcad#242) |
| Sheet-metal rule (`B42ED662…`) | expressions with their values in order (bend radius, relief width, relief depth, two more), five GUID texts, the corner relief size, the K-factor (f64, four bytes after the corner relief size's value), the material, the thickness text and a reference to the thickness parameter *(seen: one rule; the order of the relief fields is taken from their default expressions)* |

Edge recipes of some flanges have names with a nonzero word after their
operations (`0x04000102`, `0x04000022`, `0x04000042`): the recipe reader
reads it over, as the blobs' own names have a word there
(`core/f3d/src/design/recipe.rs`; the design corpus comparison is
unchanged).

A flange whose trim reaches an earlier bend's seam (flanges on adjacent
edges, a flange on the side edge of a wall) trims that bend too, developed
(`commands.md`, *Folded sheet features*) *(verified: the `.f3d` designs'
two-edge flange on the walls' side edges and the `.ipt` multi-edge
flanges below give their states)*. An `.ipt` flange on several edges is
one `sheet_flange` per edge read by its name, in the file's order, with
the inputs its path gives (the edges on the surface the path's edge is
on); every edge must be read, or the flange is not translated
(mitcad#242; the candidate search among the panel's boundaries #211 used
until then is gone). A rounding or chamfer of the file whose every edge is a
corner of a valid sheet's panel is tried first as sheet corners (the same
geometry, the body stays a sheet).

Not translated yet, with the reason in each item's note:
- faces bent onto the sheet (their bend part's items not decoded);
- `.ipt` flanges on several edges with corner settings of their own (the
  corners their walls make, `core/ipt/README.md`, *Corner settings*);
- `.ipt` bends closing a loop of material, bends of other kinds, folds
  with other options or across holes (Mitcad's fold needs one
  uninterrupted line), cuts with the slot-33 option;
- `.f3d` widths other than those above (an `EdgeOffset_1`), the `.f3d`
  hem items (`Hem…` roles), the `adjacent` position,
  bends and folds.

### Light bulbs

`Importer::light_bulbs`, after the replay: each imported sketch and
construction plane, axis and point keeps the file's light bulb only where
it differs from Mitcad's default (sketch hidden while a feature uses it,
construction geometry shown) — a used sketch the file shows, an unused one
it hides, a hidden plane. A sketch that follows the default keeps nothing,
so deleting its last consumer shows it again.

The light bulb is the item's `props.isLightBulbOn` (external dumps and the
decoder), else a sketch's `detail.isVisible` (external dumps); otherwise
the default decides. The decoder reads it for sketches of the current file
format (sketch class version 18, most of 17) and for construction planes;
older sketches, construction axes and points stay unknown. Copies of a
sketch imported into another component follow the default. The report
counts sketches and construction features, those with unknown light bulb
and those kept (`light_bulbs`).

Occurrences: the file keeps one object (class `2BB3BB69`,
`classes::HIDDEN_OCCURRENCE`) per occurrence path whose light bulb is off
and none for those shown. Its last reference list is the path (`u32 n | n
× ref occurrence`, from the root component down); a top-level occurrence
of a component of another document has an empty list and the object refers
to that one occurrence of the file only; a path that goes on inside another
document names its levels there by context references and is that
document's own (its file holds it). The decoder turns `isLightBulbOn` off
on the occurrence node at the path; the others stay unknown (shown).
Against the API reference dumps: the hidden occurrences of all 35 designs
that have any agree by count per component (two designs name the
components differently), and in one design of 113 occurrences one by one.
An occurrence the file hides is hidden in Mitcad
(`set_occurrence_visible`); one inside a component placed several times
(one occurrence in Mitcad, a path per placement in the file) follows its
first path.

### Appearances and materials

The file's appearances and physical materials (mitcad#228): the decoder
reads them into `_f3d.appearance` (`mitcad_f3d::design::appearance`,
`mitcad_f3d::protein`; SCHEMA.md §2), and `appearances.rs` gives them to
the replayed bodies and faces after the replay (commands.md, *Appearances
and materials of the file*). `mitcad-f3d-inspect assets <file> [-v]` lists
a document's assets, `appearances <file> [--json]` what the design gives
its bodies and faces, `blob-bodies <file> <blob>` a blob's bodies with
their keys and their faces' appearance GUIDs.

**The asset packages.** `ProteinAssets.BlobParts/*.protein` is a ZIP
archive (deflated) in the document's ZIP, one folder per asset set:
`Schemas/*.xml` (the property schemas) and `AssetData/`
(`InstanceProperties.bin`, `DefinitionIteratorProperties.bin`,
`AssetTableOfContents.bin`). Both `.bin` streams are paged, as the
published description has it:

```text
stream   u32 page size P (136) | 12 bytes | pages of P bytes
page     u32 | u16 P − 8 | u16 1    the first page of a record (its body whole)
         u32 | u16 P − 8 | u16 0    a further page
         ffffffff | u16 used | u16  the last page: the first `used` bytes
```

An instance record is `str schema | str asset GUID | str asset name | str
library id` (`str`: u32 length, ASCII) and then the values of the
schema's properties — its own and those of the schemas it is based on
(`<Base val=…>`), a derived schema's property in place of the base's of
the same id — in byte order of their ids, without the library id (in the
header), those marked `readonly` and those marked `definitionIteratorData`.
Values: Boolean u8; Integer and Choice u32; Float f64, with a `unit`
attribute u32 unit tag and f64; Distance u32 unit tag and f64; String,
Uuid and URL `str`; Color four f64 (RGBA in [0, 1]); Reference nothing;
TextureURI u8 kind, then kind 0 u32 n and n `str` (library resources),
kind 1 one `str` (a file name); a property with `allowmultiplevalues`
(not TextureURI) u32 n and n values. A property with
`allowconnectedassets` ends with u8 0, or u8 1, u8 1, u32 n and n `str`
GUIDs of the texture assets connected to it. A unit tag is `class << 12 |
unit`: lengths `0x200d` cm, `0x200e` mm, `0x2016` inch; density `0x11001`
kg/m³. A texture's schema (based on `TextureMap2dSchema`) writes one of
its two map channel integers less: the record decodes to its end without
`texture_MapChannel_ID_Advanced`. A description record is `str schema |
u8 | str name given | str asset name | u32 version` and, for version 2,
`str category | str | str description`, then u32 n keywords and u32 n
preview paths; the descriptions come in the instances' order (an edited
copy of a library appearance names another asset in its description, so
they are joined by position). Every record of every package of the
corpora and the reference models decodes to its exact end (5 558 assets
in 16 schemas: the opaque, metal and transparent shaders and their older
generic, metal, plastic, ceramic and glazing forms, physical materials,
structural and thermal aspects, bitmap and bump textures).

The name a person gave an appearance (`Custom Blue`) is the description's
name where it is no library preset; a library appearance's display name is
not in the file (the library resolves its preset), so the import names it
by its description (`Paint - enamel red glossy`, `Steel - satin`). A
texture's image is a library resource, not in the file either.

**The design stream.** A body's record (class `D3937028`, its object id
the body's entity `E`, its name `0_E`) names the body's physical material
and its appearance, and lists the faces with appearances of their own;
names are

```text
name     str16 token | str16 library | (u32 0 | str16 name given) | str16 preset
body     … u8 1 | u32 n | n × face | … | str16 browser node GUID | … |
         material name | ref (CD57BC48) | ref E + 1 | str16 body name |
         … | appearance name | f32 0 | f32 1 | …
face     str16 face GUID | 4 × f32 RGBA | u8 1 | u8 1 | 9 × u8 0 | u8 form |
         appearance name | f32 0 | f32 1
```

The token is the asset's GUID; the asset it names is the token's own or
the one of the token with `_Post2015` suffixes (an appearance kept for the
newer shaders beside the older one under the token), the one whose name is
the preset first. Libraries are upper-case GUIDs (`BA5EE55E-…`
appearances, `C1EEA57C-…` physical materials and the appearances they
bring, `314DE259-…` the generic one, older ones). A name whose asset is a
physical material (`PhysMatSchema`) is the body's material; the other one
not in a face entry its appearance. Face references (class `BFB592FE`,
a face input) that carry an appearance give it as `… | appearance name |
f32 0 | f32 1 | str16 face GUID`; those of faces the body's record does
not list are added to the body whose face carries the GUID. An override
(class `10506394`) names its component and occurrence (references) before
`4 × f32 | u8 1 | u8 1 | 10 bytes | appearance name`; one without an
occurrence is the component's own (the root's).

**Joins.** The body map (classes `74CA4562`, `40DCC3F4`: `u32 n | n ×
(u64 selector, u64 entity) | ref | 0–4 zero bytes | str16 blob name` to
the record's end) gives each entity its blob and selector. The selector is
the key a body's record carries (its header's second field) where a body
of the blob has it, else its position among the blob's last top-level
bodies, as many as the body map names: a snapshot blob (`.smb`) saves the
faces the design refers to as bodies before the design's (the faces' GUIDs
put the design's bodies there in all 38 snapshot blobs that showed it).
The body's ASM record in the blob is the stored body's (`StoredBody::source`,
`blob#record`), which the final comparison matched with a replayed body. A
face's GUID is the string after `NEUTRON_Material_attrib_def` in an
`ATTRIB_CUSTOM` attribute of the face in the blob's stored state
(`NamedState::face_appearance_guid`); every face of the body carrying it
(copies and pieces carry it too) gives a fingerprint as a feature's face
input does (points inside faces of extruded and revolved curves since
mitcad#228, `names/surface.rs`; a planar face's point no longer falls into
a hole where the line through the mean of its edge points runs through
the seam of a closed edge, whose ends met only to rounding:
`names::point_inside` closes such gaps).

**Parameters.** The base colour is the shader's colour property
(`opaque_albedo`, `metal_f0`, `transparent_color`, `generic_diffuse`, …;
`common_Tint_color` where its tint is on), as stored (its colour space
sRGB; a linear one encoded); roughness `surface_roughness` (generic: 1 −
glossiness); metals metalness 1; an opaque surface's index of refraction
from its reflectance `f0 = ((n − 1)/(n + 1))²`; transparent: transmission
1 and its own index. Translucency, emission, bump, normal and cut-out
images and texture images are listed as notes.

**Materials.** A physical material's density is its structural aspect's
`structural_Density` (kg/m³); Mitcad's material is set where its name is
the file material's description up to the first comma or the last part of
its classification (also with a grade after it, `Aluminum 6061`; ABS
written out), and its density the file's within 2 %.

#### Against the reference models

The appearance variant models (`MITCAD_F3D_MODELS`, `diff_appearance_*`,
13 designs, 15 bodies) against their API dumps:

| | achieved / target |
|---|---|
| bodies with the file's appearance | 15 / 15 |
| faces with their own appearance | 1 / 1 |
| occurrence overrides applied | 1 / 1 |
| bodies with the file's material recorded | 15 / 15 |
| bodies with Mitcad's material set (steel, aluminium) | 15 / 15 |
| colours within ΔE 1 (CIE76) of the dump's colour property | 15 / 16 |

The one colour off is the textured oak: its colour is the image (the
dump's colour property is null; the stored colour, white, comes in).
The largest difference elsewhere is ΔE 0.44 (the stored values against the
dump's 8-bit ones).

The body and face colours of all reference models' STEP exports (475
models) against the decoded appearances: 574 / 576 within ΔE 1, the two
others the textured oak and the occurrence override (its body's own
appearance is decoded; the import gives it the override); 10 entries
have no counterpart (models without bodies, and body records of bodies the
export leaves out, which the import leaves out too).

#### On the corpora

The corpus (`MITCAD_F3D_CORPUS`, 48 designs) and the owner's design
backup (499 designs), imported with `--time-limit 60` (items after it
take the file's bodies; the appearances join the stored bodies either
way):

| | achieved / target |
|---|---|
| the stored design's solids with the file's appearance | 5 726 / 5 741 |
| faces with their own appearance | 3 088 / 3 088 |
| solids with the file's material recorded (with its density) | 5 719 / 5 719 |
| solids with Mitcad's material set | 4 482 / 5 719 |
| occurrence overrides applied | 40 / 58 |

- The 15 solids without: no body record names them. 7 are bodies of a
  history blob's stored state without a key, which the body map does not
  name (each beside a keyed body of the same faces, in 5 designs); 8 are
  bodies of a snapshot blob whose body map is empty (one design and its
  copy). Whether the stored design holds them at all is open: the import
  takes every top-level body of the stored state.
- The materials not set are ones Mitcad's library does not have (a generic
  electronic component material of 6 020 kg/m³ on 1 120 bodies,
  polyoxymethylene, PC/ABS blends, …); they are listed with their density.
- 18 overrides are on components placed several times (Mitcad keeps one
  appearance per body).
- 4 face references name faces the stored design no longer has; 4 566
  body records are of bodies the stored design does not hold (a snapshot
  blob's copies, bodies taken by later operations).
- Notes: translucency (18 appearances), bump and normal maps (20),
  texture images for the colour (8) and the roughness (5).

### Crashes

OCCT can crash on unusual input (a fillet on imported geometry); the
importer turns crashes into errors (`mitcad::geometry::catch_occt_crashes`,
OCCT's signal handlers). On Linux that needs `OCC_CONVERT_SIGNALS`, which
OCCT's CMake config sets only for Debug and Release; the root
`CMakeLists.txt` sets it for every configuration. A crash that corrupts
memory cannot be caught: a fillet with a strip 1e-4 wide did, hence the
1e-3 of the dressup retry.

### Limits

- Inputs the streams name (*Named inputs* below) are found only where
  the history state before the item has them by those names: not for
  items without an ASM state of their own (suppressed, or after the
  timeline marker), faces named as they were more than 24 states earlier,
  or edges between the same two faces whose ends do not tell them apart.
  Those edges are left to the history.
- Selections decoded from the streams (mitcad#67) and tried before the
  history's guesses: a combine's target and tools, a split's tool (one
  plane, face or body) and bodies, a pattern's or mirror's objects
  (bodies, features or faces) and a mirror's plane (also a planar face), a
  pattern's axis or directions (origin or construction axes, sketch
  lines, edges, faces), a hole's points, type and an extent through all.
  Not decoded: a hole's extent up to an object and its direction (the
  history settles both), a split with several tool bodies (it falls
  back), a rectangular pattern's direction vectors where the entity is an
  axis (the axis gives them; whether a direction is reversed is not
  stored apart from the distance's sign). Sketch texts are not decoded
  from the streams at all (content, font, frame: a text's placement is a
  matrix in some writers' objects and absent in others), so neither is a
  text's construction flag; texts come in only from external dumps. Planar
  faces named as mirror planes or split tools are not found in the
  history yet (only cylinders and edges are), so those items still fall
  back or take the history's guesses.
- Extrusions: the decoder gives the extent codes (one side, two sides,
  symmetric; per side a distance, an object or through all), the
  distances, tapers and start offset, the direction as a vector, the
  object of an extent up to an object (a face by its names, or a plane;
  mitcad#96) and whether a symmetric distance is the whole length. Not
  decoded: which profiles (only how many, see *What the history settles*),
  a two-sided extent's object side (it falls back), which way a one-sided
  extent through all goes when the stream's flag is not set (the vector is
  the sketch's normal; the history settles it), and thin walls. Participant
  bodies come from their explicit input slot when that slot and its body
  references decode (mitcad#186); unresolved selections retain the older
  reading and history alternatives.
- Features that use bodies of two components at once fall back, and so
  do features across components whose components are placed more than
  once (*Across components*).
- Joints (*Joints*): a joint comes in as a joint only where it holds
  where the file places the occurrences (*Occurrence placements*);
  occurrence moves are not replayed as Mitcad's `move_occurrence`.
  Components of other documents are empty (their bodies, joint origins
  and own occurrences are not in the file): joints of occurrences inside
  them are left out, and a side on one stands on its origin (the corpus'
  fasteners' joints hold there at the placements after them). Geometric
  and assembly relationships, motion links and contact sets are left
  out. Not decoded or not settled: which joint origin of another document
  a side names (the decoder gives its id in the other document only), the
  names of components of other documents, captured positions of top-level
  occurrences named by ids no occurrence has where nothing inside them is
  captured too (left out), motion type codes 3 and 5 (none seen), the
  parameter roles of planar and ball joints' limits (left out), and the
  order of a joint origin's angle and offsets (each joint origin is
  checked against the file's frame). The sides' faces and edges are found
  by their geometry, not by the names the decoder gives (joints have no
  history state of their own to find them in).
- Fillets with G2 continuity or mid radii, asymmetric fillets, setback
  corners, and chamfers with miter or blend corners are translated, but
  Mitcad builds them by its own rules (`commands.md`, `fillet`,
  `chamfer`), so they match the history within the looser conventions
  tolerance (a match is kept with a warning). Which face takes which
  offset of an asymmetric fillet is the set's flip (decoded from the
  streams since mitcad#154, *The published format descriptions*); the
  other reading is still tried after it. The stream decoder gives these
  options, a fillet's corners, each set's continuity, tangency weight and
  tangent chain, and a chamfer's corners since mitcad#154; before, it gave
  constant radii only and the import built the defaults.
- Replace face needs the history (for its faces) and its target in the
  replay: a surface body the replay lacks makes it fall back unless the
  target face is planar (then a fixed plane).
- Threads of older writers (thread class versions before 5) are not
  decoded, nor whether a thread is right- or left-handed (right-handed is
  taken).
- Modelled threads: no reference has a partial one, so how a partial
  thread ends and whether the file sizes the rest of the face are not
  settled (the plain thread is tried, then the sized face). On a face
  whose ends are not planes across its axis (a hole into a curved face)
  the geometry kernel's booleans of the helical flanks can give an
  invalid shape: the corpus's one such thread falls back. A modelled
  thread has four faces a turn (the two flanks, the root and the crest:
  the groove is swept a turn per spine edge, as a spine of one edge over
  many turns makes the cut fail), about 60 for an M10 thread 20 mm long,
  which later operations on the body have to take into account.
- Sweeps, pipes and lofts (*Sweeps, pipes and lofts*): sections that are
  faces or sketch curves, guide surfaces and inputs of other components
  are not translated; a loft whose sections' sketches have
  many regions is tried with twelve choices of them.
- Not translated: ribs and webs. A coil's plane, frame and choices (type,
  section, position, rotation, operation) are decoded since mitcad#25; a
  coil comes in where it rises along its plane's normal and starts on its
  x axis. No design of the corpus has a rib or a web.
- Surface extrusions, revolutions and sweeps (`isSolid` false) are not
  translated: their sheets come in as stored bodies (*Threads, tapped holes
  and sheets*). The stream decoder tells surface extrusions since
  mitcad#154 (none in the corpora).
- Sketch patterns, offsets and polygons (*Sketch patterns, offsets and
  polygons*) are left out, and their copies and offset curves come in as
  plain curves, where Mitcad has no counterpart: patterns with instances
  on both sides of the originals (symmetric), offsets of splines and
  ellipses (Mitcad computes those curves itself, so the file's cannot be
  kept), offset curves without a source curve at the distance (e.g. extra
  corner arcs), and copies that are not where the pattern puts them
  (suppressed or moved instances). Mitcad's pattern counts are numbers and
  its directions vectors: the file's quantity parameters stay unused, and
  a direction along a sketch line keeps its imported direction. External
  dumps do not record these constraints' entities, so from them they are
  left out.

### Corpus test

Real designs are kept outside the repository (`MITCAD_F3D_CORPUS`, default
`~/f3d-corpus`). `test_f3d_import` (`core/tests/test_f3d_import.cpp`):

```bash
test_f3d_import --corpus [dir] [--every N] [--max-items N] \
    [--time-limit S] [--hang-limit S] [--reports DIR] [--threads N] \
    [--jobs N] [--memory SIZE] [--file-timeout S]       # build/rel/core
mitcad-cli import-f3d part.f3d --time-limit 300 --report report.json
```

- `--corpus` imports every design of every `.f3d`/`.f3z` under the
  directory, requires each import to finish without error, and prints
  per feature type how items came in (parametric, partial, fallback,
  skipped) and how the final bodies agree with the stored ones. Exits 77
  (skipped) without a corpus. Files are named by position (f01, …), as in
  `core/f3d/CORPUS_REPORT.md`.
- `--reports DIR` keeps each JSON report; `--hang-limit` defaults to 600 s.
- `--threads N` imports with N threads (*Definitions evaluated in
  parallel*, *Work beside the replay*; default 1). Keep `--jobs` ×
  `--threads` within the cores; the reports are those of `--threads 1`
  except where the time decides.
- `--models [dir]` replays the reference models' external dumps
  (`MITCAD_F3D_MODELS`, default `~/f3d-models`) of lofts and sweeps,
  compares with the stored volumes, and imports each model from its own
  streams too.
- The ctest `f3d.corpus_timeline` (`core/CMakeLists.txt`) runs `--every 4
  --max-items 30 --time-limit 60` to stay quick in debug builds. Run the
  full corpus with the release build, e.g. `--time-limit 300 --hang-limit
  300` per design.
- Each file (each model with `--models`) is imported in a child process of
  its own, `--jobs N` at a time (default: the cores / 4, bounded by the
  memory available / `--memory`, 4G per child by default), the largest
  first; a crash, a hang past `--file-timeout` (3600 s) or a file over its
  memory ends only that file, which counts as failed. The lines come in
  the files' order and the totals are those of `--jobs 1` (every file in
  the one process, as before); [docs/development.md](../../docs/development.md#f3d-corpus-tests)
  has the options.

#### Coverage runs in parallel

A full coverage run, for numbers before and after a change: build both
states with the release preset (keep the binaries of the first), then run
each with every file and compare. The lines are those of a run in one
process, so only the times differ:

```bash
B=build/rel/core   # the binaries of one state
$B/test_f3d_import --corpus --time-limit 300 --hang-limit 300 --jobs 8 \
    --reports before-reports > before.txt
$B/test_brep_import --corpus --jobs 8 > before-bodies.txt
$B/test_exchange --corpus --jobs 8 > before-import.txt
$B/test_f3d_import --models --jobs 8 > before-models.txt
# ... the same with the other state's binaries into after*.txt, then:
diff <(sed -E 's/[0-9.]+ m?s$//' before.txt) <(sed -E 's/[0-9.]+ m?s$//' after.txt)
```

The children's memory adds up: keep `--jobs` × `--memory` within what
the machine (or `systemd-run --user --scope -p MemoryMax=...`) allows. The
line on standard error at the end gives the largest peak memory of a
child and the longest child, e.g. to choose `--memory`.

Known failure classes:

- extrusions whose result is not the state ("not in the file's
  history"): the right regions are not among those tried, mostly in
  sketches that come in partial (left-out constraints that moved curves,
  undecoded curves) or whose regions the stored profile cuts differently;
  results with the state's volume but another area or centre (a region of
  the same area elsewhere); joins and cuts across bodies of several
  components;
- extrusions that "do not cut into any participant body" or find "no
  bodies in the extrude direction": the replay's bodies before them
  already differ (an earlier fallback took a later state, or the sketch is
  placed on a face the replay does not have), or every region set tried
  lies outside the material;
- an extrusion that matches its state only approximately (within 1e-3)
  stays parametric, with a base feature after it (see *Matching rules*),
  so the items after it do not depend on its parameters;
- fillets and chamfers OCCT does not build (`fillet failed` / `chamfer
  failed`), mostly on the right edges: the stored rounding runs over a
  neighbouring face, which OCCT does only where it runs over that face
  along the whole edge (the port's edge overflow, mitcad#121) and around
  whole circles (built as rings), or the body is a stored one with tolerances
  up to 0.05 mm (the classes and counts: *Fillets and chamfers whose
  by-name definition fails in the kernel*);
- items after the time limit (very large designs);
- patterns whose input is not one of the last six extrusions, or copies
  features other than extrusions (with their fillets, for example);
- items whose state an earlier fallback took the replay past, mostly
  fillets and cuts, which cannot leave the bodies unchanged, so they stay
  skipped or fall back;
- items without a translation (*Items without a translation*), items of
  unknown classes (`?…`), construction axes, and the untranslated types
  under *Limits*, which are skipped or fall back.

### Learning the undecoded inputs

Every item whose definition the history settles (*What the history
settles*) is a labelled example of what the stream decoder does not read
yet: the item's raw record and the answer the history accepted
(mitcad#96). The learning dump collects them, and a tester checks rules
from the record's bytes to the answer on them, without geometry.

**Dump.** `mitcad-cli import-f3d part.f3d --learn DIR`, the `learn` field
of the `import_f3d` command, or `MITCAD_IMPORT_LEARN=DIR` (read by
`mitcad-ffi`, so `test_f3d_import --corpus` writes it for every design):

```bash
MITCAD_IMPORT_LEARN=$HOME/f3d-learn build/rel/core/test_f3d_import --corpus \
    --time-limit 300 --hang-limit 300 --jobs 5 --memory 4G
```

The lines are derived from the designs: keep the directory outside the
repository, like the corpora. Per design, `DIR/settled/<design>.jsonl`
holds the items accepted against their known history state and
`DIR/unsettled/<design>.jsonl` the others that had candidates (none
accepted, or accepted unchecked), plus fillet and chamfer items whose
translation failed or offered no candidates (mitcad#106). Their decoded
edge match diagnostics remain available even without history fallback;
`candidates` is empty and `accepted` and `answer` are null when none were
offered or accepted. A rule must not contradict unsettled examples either.
A rerun replaces a design's files. The dump's own work (the
bodies, inputs and sketches it records when an item's definitions are
translated: seconds on a large item) is left out of the item's time and
the import's time limit (`learn::Clock`; the deadlines move by it), so
that a dump run gives no item up that a run without it keeps; candidates
are kept once by a hash of their definitions, and at most 1000 per item
(an item can have tens of thousands, which took gigabytes). The import reports to
`src/learn.rs` at four points: the candidates translated
(`Importer::translate`), ranked (`Importer::rank`), the one accepted
(`Importer::accepted`) and the end of the import; the raw records come
from `mitcad_f3d::design::learn`.

One line (`schema` 1):

| Key | |
|---|---|
| `file`, `item`, `name`, `type`, `class`, `class_version`, `object_id`, `f3d` | the design, the item's timeline index, its type and the decoder's raw item fields |
| `status`, `outcome`, `note` | `settled` or `unsettled`; how the item came in |
| `translation_error` | present only when a fillet or chamfer translation failed: the most recent failure text, including the input position and decoder/replay reason when available. Retained if a later translation offers candidates |
| `known` | what the decoder made of the item (the dump IR's item without `_f3d`) |
| `record` | the item's object and the input objects it refers to, three references deep: `objects[]` with `path` (`item`, `item>0897AF07#0>C46D3EEB#0`: the class's first eight digits and the position among the parent's references to that class), `id`, `class`, `module`, `version`, `len`, `hex` (up to 16 KiB), `sub` (sub-chunk start) and `tokens` (`header`, `root` with its references and attributes, `ref` with `id` and `class`, `str16` with `text`, `sub`). Other timeline items, sketches and their entities, parameters, components and the like are not followed |
| `context.sketches` | per sketch the candidates use (`T<index>`): `regions[]` as Mitcad computes them, in the file's ids (`key`, `outer` and `holes` curve lists, `area` mm², `centroid` and an `inside` point in sketch mm), the `frame` the import placed the sketch on (`origin`, `x_axis`, `y_axis`, `normal`; model mm, the space of an extrusion's stored direction) and `curves` by the file's ids (`object`, `class`, the `u64` root attributes such as `crv_primary_id`, `crv_secondary_id`, `pt_tag`; `type`, `construction`, `geometry` from the dump, sketch space in cm). Curves Mitcad made itself appear as `m<id>` |
| `context.bodies`, `context.edges` | the bodies before the item (`uid`, `volume`, `area`, `center`, `faces`; at most 200) and the edges the candidates name (`mid`, `length`, `body`), mm |
| `context.resolved` | the bodies, faces and edges the decoder gave for the item (by JSON pointer into `known`, e.g. `/detail/edgeSets/0/edges/2`) as the replay's (`{"kind": "body", "uid"}`, `{"kind", "body", "name"}`; `null`: not found), so that a decoded input can be compared with the answer |
| `context.edge_matches` | additive dressup matcher diagnostics for decoded edge inputs at those same pointers (mitcad#106): `status` (`matched`, `decoder_unresolved`, `missing_in_replay`, `ambiguous_in_replay`), `decoder_found` (the decoder's `found` text), `count` and either `matches[]` (`body`, `name`, `mid` mm, `direction`) or ambiguity `candidates[]` (`body`, `name`). A matched result also has `chosen_from_multiple`: at least one match selected among multiple distinct replay edges (absent in older dumps). The first 64 decoded inputs are considered, with at most 16 matches or candidates per edge; `count` retains the full number. Bodies and names use the file's timeline indices. This uses the actual dressup matcher, including circles and split edges; `context.resolved` retains its older midpoint matching for compatibility. The recipe tail's order of the faces round the edge's ends (`_f3d.end_faces`) tells equal mirror-image edges apart in the decoder (EDGE_ORDER.md) |
| `candidates[]` | in the order tried, the first 1000 (`cost.candidates` counts all, `accepted` is the rank among all): `defs` (Mitcad feature definitions in the file's terms: feature uids as timeline indices `T<i>`, profile regions as `{"sketch": "T<i>", "region": <index in regions>}`, sketch curves by the file's ids), `note`, `guess`, `input_provenance` (`read`, `inferred`, `unknown`), `predicted` |
| `accepted`, `answer` | the accepted candidate's rank and the candidate itself |
| `cost` | `candidates`, `rank`, `seconds` from the translation to the acceptance |

**Tester.** `tools/f3d-learn/learn.py` (Python 3, standard library;
`--data DIR`, default `MITCAD_IMPORT_LEARN`) reads only the dump:

- `stats`: settled and unsettled items per type with their cost;
- `show` / `fields`: examples with their objects, tokens and the gaps
  between them, and one object's fields over all examples;
- `test`: a rule as a Python expression (`--expr`, compared with a target
  such as `regions`, `curves`, `edges`, `flip`, `operation`, `extent`,
  `participants`, `axis`, `def.<key>`), a check (`--check`) or a file
  defining `predict(ex)` or `check(ex)` (`--rule`); prints how many
  examples it explains, contradicts or does not apply to, the
  counterexamples, and with `--unsettled` whether its predictions for
  the unsettled items are among their candidates;
- `search`: rule families over every field of every object (fields are
  addressed `gK+N` / `gK-N` in the K-th gap, `@N`, or `rK` for the K-th
  reference, read as `u8` … `f64` or `sign`): `eq` (the field is the
  value, or the number of selected entities), `map` (each value stands
  for one answer class), `idin` (the field holds an id of an answer
  entity and of no other), `idset` (the object holds the ids of the
  answer's entities), `bits` (bit i selects region i);
- `survey`: per type the record's objects, their lengths and gaps, and
  the fields that vary with their values and how purely they go with a
  target.

Its unit tests (`tools/f3d-learn/test_learn.py`, the ctest
`tools.f3d_learn`) run on a synthetic dataset; `src/learn.rs` and
`mitcad_f3d::design::learn` have their own.

A rule that explains every settled example (or every counterexample with
a reason) and agrees with the unsettled ones goes into `core/f3d` with a
unit test on a synthetic record; the import then tries the decoded
answer first, the guesses after it.

Decoded so far (mitcad#96, *What the history settles* says how the import
uses them): the profile loops of extrusions and revolutions
(`_f3d_profile_loops`), a revolution's operation, profile count and
sketch-line axis, a circular pattern's axis line (`_f3d_axis`) and the
direction of each rectangular pattern direction from a construction axis,
a mirror's `isCombine`, a hole's through-all flag where the file keeps
a depth (`_f3d_through_all`, a lead), and fillet and chamfer edges
(the item tail's anchor, recipes that name the edge itself or carry a
second list, face selections, inputs of the same names, the replay's
edges where the middle does not tell, a two-distance chamfer's sides:
*Named inputs*), and items without a result (*Fallback*). A mirror's plane and the patterns'
quantities the decoder read already (the dump confirms them on every
settled item). For extrusions also the flag byte after the extent codes,
the input slots' roles, the object of an extent up to an object and a
symmetric extent's length (`ExtrudeFields`, `decode::extent_slots`; in
the dump `_f3d.extrude.flag`, `slot_roles`, `full_length` and
`extentOne.entity`). Then (mitcad#104): both sides of a two-sided
extrusion up to objects, a combine's body removals in other components
(*Components*) and an oblique stored extrusion vector.

#### Input decoding audit (mitcad#186)

The audit starts from the existing 11 October 2026 learning snapshot on
`bd34bc6b`: the complete main corpus and the 78 largest backup designs.
The snapshot is retained unchanged while rules and corrections are
checked. Its counts include unsettled dressups with no candidates; those
records still contain useful input and translation-failure evidence.

| Input group | Settled / unsettled examples | Reading or remaining uncertainty |
| --- | ---: | --- |
| Circular patterns | 83 / 14 | All 97 have decoded seeds. The direct path now resolves sketch-line axes as well as origin/construction axes. Compute/symmetry flags have no contrasting accepted labels here, so this snapshot cannot establish their byte meanings. |
| Rectangular patterns and mirrors | 18 / 20; 18 / 6 | All 38 rectangular patterns have seeds and a first direction; use those on the direct path, including body/face seeds. Keep either decoded mirror input when the other is missing. A stored-vector priority rule matches 13 settled axes, contradicts one and cannot compare four; flipping by the distance sign contradicts ten. Neither rule is adopted. Spacing/extent and symmetry also lack contrasting labels. |
| Fillet/chamfer edge sets | 808 / 176; 194 / 9 | Bind size-parameter holders to their set records, then the counted selection list, rather than flat input order; bind fillet options by the same holder. The input/option consistency check explains 802 settled + 173 unsettled fillets and 191 + 9 chamfers, with no contradictions; nine settled and three unsettled examples are not comparable. Of 1,191 known sets, 1,178 have the checked framing: 1,170 retain their selections and eight previously missing sets in four items are recovered. Independently checked on 43 sets in 42 reference features. This verifies decoded inputs, not successful kernel replay. |
| Extrusion direction, extent and participants | 1054 / 95 | Role-8 slots exist in 925 examples. They exclude termination references wrongly included in 80 old lists; six extras identify distinct replay bodies. Comparing against explicit accepted participants gives 254 matches and five existing resolver disagreements (795 not comparable); alternatives remain for unresolved replay. No new direction/extent rule was established. |
| Extrusion profiles and sketch selection | Same 1054 / 95 extrusions | Exact loop-to-region curve membership explains 572 settled choices, contradicts nine and abstains on 473; it is insufficient for curve pieces and child loops and is not adopted. A previous-sketch fallback is marked inferred. The groups overlap and must not be added together. |
| Thread inputs | 42 / 1 | All 43 records agree with their decoded ThreadInfo values and modeled/full/location flags. Candidate thread parameters are unchanged across all alternatives; 12 tube and two offset acceptances change geometric representation only. Fourteen items have a nonzero phase obtained from later history, an inferred input. Unsupported layouts remain undecoded; handedness is decoded from the controlled variants (below). |

The old candidate `guess` flag controls replay policy: some candidates
need history, including alternative geometric representations. Accepted
rank also describes the search, not whether an input was read. In this
snapshot, rank greater than zero or `guess` selects 72 circular patterns,
18 rectangular patterns, 18 mirrors, 64 fillets, 14 chamfers, 80 extrusions
and 14 threads. Those are screening counts, not counts of decoding
failures: the 14 threads comprise 12 tube and two offset alternatives.

The completed baseline later supplied all 48 main-corpus and 325 backup
designs. Rules were rechecked on that larger existing dump without another
geometry run. Counts below include every record, including empty candidate
lists; design versions are separate observations.

| Group in the full baseline | Settled / unsettled | Independent recheck |
| --- | ---: | --- |
| Circular patterns | 117 / 22 | All 139 retain decoded seeds. Compute and symmetry labels still provide no contrasting evidence for a byte rule. |
| Rectangular patterns and mirrors | 26 / 30; 22 / 8 | All 56 rectangular patterns and 30 mirrors have decoded seeds. Stored-axis prediction: 18 matches, one contradiction, seven unscored; distance-sign prediction: seven matches, 12 contradictions, seven unscored. Both remain rejected. No sketch-axis examples occur here; synthetic tests cover that path. |
| Fillets and chamfers | 1379 / 282; 294 / 17 | Input/option checks explain 1368 + 278 fillets and 290 + 17 chamfers, with zero contradictions. Of 1978 sets, 1947 have comparable framing: 1939 retain their selection, eight recover a missing selection. All 1578 previously read fillet-option sets retain their holder link; 1563 support an unambiguous option comparison, all matching. |
| Extrusion participants | 1730 / 163 | Role 8 occurs in 1485 items; 122 old lists include termination references, still only six distinct extra replay bodies. Prediction gives 331 matches, the same five resolver disagreements and 1394 unscored settled items; 35 unsettled predictions occur among candidates, 96 do not and 32 are unscored. |
| Extrusion profiles | Same 1730 / 163 | Exact curve membership gives 1041 matches, the same nine contradictions and 680 unscored settled items; unsettled membership is 60 among, one outside and 102 unscored. The proposed replacement remains refuted; no `isThinExtrude` is decoded. |
| Threads | 77 / 3 | All 80 raw field/option checks and cross-candidate parameter comparisons agree. Twenty-one nonzero phases came from history; those on whole cylinders with the axis up are now read from the seam. The legacy screening count of 31 is 29 tube and two offset alternatives. |

Full-baseline settled legacy screening counts are 106 circular, 24 rectangular,
22 mirror, 143 fillet, 23 chamfer, 100 extrusion and 31 thread items.
The conservative fillet field/raw-option guard described below extends
to 1237 settled and 217 unsettled items, with no contradictory raw options.
Reading counted participant references directly also recovers three lists
that the learning dump's diagnostic tokenizer mislabels as string data;
this changes applicability counts, not the prediction results or decoder.

The report and learning candidates carry separate `input_provenance`:
`read` requires accounting for every needed input, `inferred` records at
least one heuristic or history-derived input, and `unknown` leaves an
incomplete audit explicit. A stored-body fallback has no accepted-input
provenance. This metadata does not change candidate ordering, the `guess`
policy or the history comparison.

The initial `read` path is deliberately narrow: constant-radius G1
fillets with an explicit positive radius, tangent-chain flag, rolling-ball
corners and complete native edge references, all uniquely resolved in
the replay. Choosing the best of several possible replay edges is
`inferred`, even if the history accepts it. Splitting the same selected
edges into tangent-chain kernel operations preserves their provenance.
The field and raw-option guard holds on 703 settled and 148 unsettled
fillets, with no contradictory raw options. These are eligibility counts,
not final `read` counts: the old dump does not retain every considered
replay edge needed to establish uniqueness.

Other paths remain `unknown` unless a known inference applies. In
particular, no native extrusion in the snapshot has a decoded
`isThinExtrude`, and the profile mapper combines exact and heuristic
paths. `unknown` is not counted as read merely because rank zero worked.
Omitting dressups from a decoded mixed pattern selection, using the only
solid for an unresolved shell body, and taking a thread phase from later
history are examples of inputs reported as inferred. The corrections
above use decoded selections directly while retaining alternatives for
inputs or replay mappings that remain open.

Verification used an immutable optimized macOS build on `bb45f056` plus
the decoding batch, the original 48+78 design selection, 300-second
limits and two workers with 6 GiB each. All 126 design reports completed.
The historical Linux baseline is older, so these differences also include
intervening main changes and platform effects:

| Outcome | Main corpus before / after | Selected backup designs before / after |
| --- | ---: | ---: |
| Parametric | 1132 / 1182 | 2733 / 2832 |
| Partial | 38 / 38 | 93 / 93 |
| Fallback | 210 / 161 | 699 / 602 |
| Skipped | 261 / 260 | 705 / 703 |

Final bodies remain within 0.1% of their stored volumes, with none off,
missing or extra. Three versions of one fillet become fallback relative
to the old Linux run. A clean same-platform `bb45f056` control reproduces
the same self-intersecting-wire failure at `a9e82a9a` item 45, without
this change. In contrast, `64052a50` item 16 is fallback on that control
and parametric with the decoded rectangular inputs, matching history and
retaining the final bodies. Paired learning comparisons exclude three duplicate
(file label, item, type) keys rather than overwrite them. On items settled
in both runs, legacy screening gives the following before/after counts;
this is not a causal measurement of input decoding alone:

| Group | Paired settled items | Rank > 0 or legacy guess, before / after |
| --- | ---: | ---: |
| Circular | 83 | 72 / 72 |
| Rectangular and mirror | 36 | 36 / 36 |
| Fillet and chamfer | 997 | 75 / 74 |
| Extrusion (both input groups) | 1052 | 80 / 72 |
| Thread | 42 | 14 / 14 |

The subsequent provenance batch passes 180 import unit tests, scoped
clippy and formatting, and the C++ import test. Three targeted models
(`64052a50`, `11d7acd2`, `d03dcc34`) retain identical per-item results and
final-body reports after removing only the new provenance field: 16
accepted items are read, four inferred and 34 unknown; 47 other items
have no accepted-candidate provenance. This includes a read tangent-chain
representation, a history-selected fillet and a fillet missing options.
These are targeted counts, not extrapolated totals for the 126 designs.

After integrating the performance changes of mitcad#181, a separate six-design
run on `2c0c22b5` exercises normal import without learning's eager candidate
collection. Five retain their outcomes; `a9e82a9a` gains 21 parametric items
from previous fallbacks. All 117 final bodies match within the exact-volume
threshold; no item outcome worsens. The original frozen-run counts above
remain separate. A regression test also preserves alternative extrusion
profiles needed by a later pattern, with and without learning enabled.

Two held-out body patterns (`143d219b` item 60, `b2b33a5b` item 3) also
produce direct candidates from resolved named inputs. The existing volume
ranking can still accept a history-derived candidate first. That remaining
candidate-ordering limitation is separate from input decoding; a direct
candidate's presence does not establish that it was tried or accepted.

The learning tool keeps records with no candidates. `test --unsettled`
also evaluates `check(ex)` rules, honours the selected class version and
fails on rule errors. A prediction for an item with no candidates is
reported as unscored, rather than silently omitted or treated as a
contradiction. A candidate match on an unsettled item is a consistency
check, not a history-verified answer.

Accepted suppressed definitions (mitcad#180) retain their input provenance
and learning answer too. They stay unverified and in the unsettled dataset;
suppressed placeholders have no accepted-candidate provenance.

The final integration with mitcad#178, #180 and #181 passes 186 import
tests, 98 f3d unit tests, the f3d corpus/reference-model tests, formatting,
scoped clippy, the warning-free optimized macOS build and the C++ import
test. The geometry ctest passed after the performance merge. These scoped
checks do not replace or relabel the frozen coverage evidence above.

##### Extrusion options from one-option variants (mitcad#186)

Thirty-four reference models each differ from their siblings in one
extrusion option (sides, extent, through all, thin wall, profile selection,
direction, operation, distance). Their records were lined up byte by byte
and the findings tested as rules on the census learning dump (1730 settled
and 163 unsettled extrusions of the main corpus and the backup designs;
design versions counted separately) and on the variants' own dump (45
settled). The decoder reads them in `core/f3d` (`decode::extrude_fields`,
dump keys `_f3d.extrude.*`); the import takes them as the first candidate.

| Option | Encoding | Census (settled: explained / contradicted / n/a; unsettled) | Variants |
| --- | --- | --- | --- |
| Operation | the first `u32` after the root part: 1 join, 2 cut, 3 intersect, 4 new body. Code 3 was not read before: an intersection came in as a new body and its item was a fallback | 1730 / 0 / 0; all 163 unsettled among the candidates | explained |
| Extent kind per side | after the seven reference slots `u32 one`, then (unless one is 2, up to an object, which has none) nine bytes and `u32 two`: 1 distance, 2 up to an object, 4 through all; with the codes `extent_a` 1 one side, 2 two sides, 3 symmetric. The newer writer keeps `u8 0 \| u32 a \| u32 b` in the nine bytes (`a` 2 for a side through all, `b` 1 on two sides and symmetric extents); earlier the decoder dropped `sides` for those records | 1729 / 1 / 0; the one is a one-sided through all whose accepted answer was both sides; unsettled: 161 among the candidates, 2 not | 45 / 0 / 0; `sides` is read on every variant |
| Symmetric length | the byte `LL` of `00 LL 00 00 00` before the thin-wall block: 1 the whole length, 2 half of it each way | 14 / 0 / 1716 | 6 / 0 |
| One-sided distance direction | not a byte: the sign of the distance parameter. The negative-direction, the negative-distance and the UI-typed variants store the same direction vector and flag as the positive one; only the parameter's expression and value differ (`-( 20 mm )`, `-20 mm`, both -2 cm). The two encodings are one geometry. So a one-sided distance goes along the sketch's normal times the sign of its parameter, whatever the stored vector says | 1485 / 1 / 244; the one is a class version 6 extrusion | explained |
| Two-sided distances | each side by its parameter's sign, along the normal | 22 of 23 settled `distance/distance` went along the normal | explained |
| Through all and up to an object | the flag byte decides the side in every writer: 1 against the sketch's normal, 0 along it. The census had 76 of 77 settled one-sided through-alls with flag 1 going against the normal, and every up-to-an-object extrusion with flag 1; with flag 0 the history's accepted answer went against in 15 of 22, which the API reference dumps refute: `isPositiveDirection` is the flag byte 0 on all 78 through-all extents (see the round-2 subsection below). The through-all variants store a different vector sign for the two directions but build the same body, so the vector does not decide it | as stated | positive and negative through all both: flip false, accepted after the history |
| Thin wall | class version 8, after the side kinds: `[6 × 0] [slot] [00 LL 00 00 00] [u8 thin] [u32 location] [slot] [5 × 0] [u32 1] [ref input slot]`, found back from the first input slot. The first `slot` is the second side's parameter holder or one absent byte; the second is the wall thickness's parameter holder (a thin extrusion) or one absent byte (a solid). `thin` 0/1; `location` 0 side one, 1 centre, 2 side two; the thickness is the parameter of role `Side1WallThickness` (its holder is the second slot) | 1714 solid explained, 0 contradicted, 16 n/a (class versions 5 and 6 have no block); no thin extrusion in the census, so it cannot contradict a thin reading | all 5 explained: solid, side one 2 and 3 mm, side two, centre |
| Thin wall side | the file's side one lies inside the profile (60 × 40 × 20 mm, 2 mm: 7.68 cm³ volume), side two outside (8.32), the centre half each way (8.00). The model's `Thin.location` side one is away from the material, so the import swaps side one and side two | | volumes match |
| Profile selection | `_f3d_profile_loops`: per profile its loops are the outline of the union of the selected regions (the outer loop, holes where unselected material is enclosed); with more than one region selected, its children are the individual regions' loops (two or more are listed even where the union is one face). The curve pieces numbered by the cuts of the curves within the loop. The even-odd rule on the pieces selects the regions | 1612 of 1634 settled items with a decoded set agree with the accepted regions; of the 22 others 11 add only regions below 1 mm², 5 name curves the sketch no longer has (a stale profile), 4 miss or differ and 2 add larger regions | all six selections (the left region with and without its hole, the right region, the disc, and their unions) agree |

The refuted exact rule of the earlier audit compared curve membership
instead of the pieces; the pieces and the even-odd rule above explain the
selections, the remaining differences are tiny regions the file may or may
not select (the history cannot tell) and profiles older than the sketch's
last edit, whose records name curves the sketch no longer has.

*Read provenance.* An extrusion candidate is `read` when its profile
selection is exact (every record of its loops found in the sketch with its
piece numbered, no curve-overlap fallback), its operation code, start
support and thin-wall block are read, the participants are read (none for a
new body, else the decoded slot's bodies), and its extent comes from the
stream: a distance, symmetric (with its length byte), two-side or two-sided
through-all extent by the side kinds and the parameters' signs, a
one-sided through all with its flag byte (0 or 1), or an up-to-an-object extent
whose object the stream names and whose side comes from where the object lies
(or the flag). Alternatives stay `inferred`, every other extent `unknown`.
On the 45 extrusions of the variants (before: 5 fallbacks, the four thin
variants and an intersection, with 39 `unknown` and one `inferred`
accepted) every one comes in parametric, 42 `read`, and the three
through-all cuts without the flag byte stay `unknown` or `inferred`. A thin extrusion of a two-sided extent is not
read (its second wall is not decoded: no variant).

Not decoded: the second wall of a two-sided thin extrusion (location two
and thickness two, no variant), the
meaning of the words `a` and `b` beyond the correlations above, and the
two doubles near the end of the record (differ on every file: not geometry).

##### Threads, suppression and the timeline marker (mitcad#186)

Decoded from the controlled one-option variants (15 thread and 16
suppression models, each differing from its siblings in the option alone,
checked against the design API's dumps) and verified on the census (363
designs, 1,132 + 5,169 parametric items).

| Option | Encoding found | Census |
| --- | --- | --- |
| Thread side (external / internal) | the flag byte after the angle (1 external, 0 internal), as read before; the class (`6g`/`6H`) and diameters follow it | 78 + 3 threads agree with their decoded ThreadInfo |
| Thread kind (cosmetic / modelled) | the modelled byte after the diameters, and a list of one `(1, id)` pair before the class (`u32 n`, n × 16 bytes, `(1, id)` of another object of the file; none on a cosmetic thread, one per face on a modelled one) | as before |
| Thread designation, class, length | strings (`M10x1.5`, `6g` / `4g6g`), the full-length byte and the thread length and offset parameters (`10 mm` / `3 mm`; a full thread keeps the default `10 mm` and `0 mm`) | all agree |
| Thread handedness | one byte after the object's two references following the pitch diameter (1 right-handed, 0 left-handed; the hole reference, a null byte for a thread item, comes before them): the left-handed variants of a cosmetic and a modelled thread differ from their right-handed ones in this byte (and the list) alone | 81 threads (versions 4 and 5) have 1, none 0; the census has no left-handed thread, so the rule cannot be contradicted. Version 1 and 2 layouts are not read. Implemented: `ThreadInfo.isRightHanded`, read first by the import; a thread that is not modelled, whose faces the decoder named, whose handedness is read and whose class Mitcad lists has `input_provenance` `read` (a modelled thread keeps `inferred` for its start angle) |
| Thread start angle (phase) | not found: no field of the thread's object, its sub-objects or the body object its list names holds it (checked for the angle in degrees and radians on the 20 census threads with a nonzero phase); the variants all start at 0. It stays inferred from the history | n/a |
| Suppressed itself or through a dependent | `000100` or `010000`, see *Items suppressed in the file* | 24 of 27 designs consistent, 3 (one design) not explained; 32 of 39 `010000` fillets have a face no state holds, 0 of 9 `000100` |
| Suppression by API, timeline menu, configuration, group | the same bytes (the hole's object in the API, UI and configuration variants differs in ids and a random value only) | n/a |
| Configuration table | `DesignConfigurationTable.<id>.dsgcfg` is JSON: columns (a part number property and one per configured object: `configuredObject` with the feature's object id and the title, a boolean column for a suppression), rows (`Configuration n`, a cell per column) and the table view's order; the active row's id is a string in the design stream, in the object named `DesignConfigurationRoot` (two models that differ only in the active row differ in this string and in the item bytes of what the row changes). The active row's effect is in the items' own bytes | no census file has a configuration table, so the import does not read it |
| Rolled back | the second of the two bytes in front of an item's last reference, see *After the timeline marker* | 52 items, 15 of the 16 designs have them as the end of the timeline |

##### Patterns and mirrors: the data object's settings (mitcad#186)

The audit above found no contrasting examples for a pattern's compute
option, symmetry, distance type and suppressed elements. One-option variant
models (a design per value of each option, made from scripts; the flip of a
rectangular pattern also by hand) show them in the bytes of the pattern's
data object, the item's first reference after its root part
(`core/f3d/src/design/build/patterns.rs` has the layout; the decoder gives
the dump IR's `isSymmetric`, `isSymmetricInDirectionOne/Two`,
`patternComputeOption`, `patternDistanceType`, `suppressedElementsIds` and
a mirror's `isCombine`):

| Option | Encoding (found by the bytes that differ between the siblings) |
| --- | --- |
| Compute option | `u32 compute \| u32 kind \| i32 -1` ends the head of the circular, rectangular and mirror data objects: 0 optimized, 1 identical, 2 adjust; `kind` 0 faces, 1 features, 2 bodies. Four or eight bytes lie before it (after the `u32 n \| n` body records); the oldest layout has neither them nor the `-1` |
| Circular pattern symmetric | the byte after the object's root part, 1 for a symmetric pattern |
| Mirror joined | the same byte of a mirror's data object: `isCombine`; always 0 for a mirror of features (the old reading, no new body, gave 1 for 12 of the corpora's feature mirrors) |
| Total angle (360, 180, 90) | no byte: the angle is the parameter `TotalAngle` and the item's float, as before |
| Rectangular distance type | the byte after the `-1`: 0 spacing, 1 extent |
| Rectangular symmetric | one byte each for direction one and two, right after the two distances' parameter holders |
| One or two directions | one or two direction inputs, a null byte instead of the second. A one-direction pattern keeps the second direction's quantity and distance (defaults), as the API reports them |
| Axis kind | the direction input names the entity: an origin axis, an edge, or a sketch line (whose entity input has no target object: decoded from the sketch curve's tags, new in this change) |
| Flip | no flag, and the stored vector does not change: the sign of the distance parameter (`-20 mm` in the API's and in the UI's pattern) |
| Suppressed elements | `u32 s \| s` `u32` at the object's end. A circular pattern's numbers count the original as 1: the API's id 3 is stored as 4 (one variant). A rectangular pattern's element numbers follow no order the variants explain, so only an empty list is read |
| Objects (features, bodies, faces, with fillets or chamfers) and mirror planes | already decoded; the variants confirm the object type, the plane kind (origin, construction, face) and the object list |

The first four rows hold in the corpora too: the layout parses in all 259
circular patterns, rectangular patterns and mirrors of both private corpora
(a decoder run over every file); `kind` agrees with the decoder's own reading
of the objects in 137 of 137 circular patterns, 54 of 54 rectangular patterns
and 30 of 30 mirrors of the census learning dump, and a rectangular pattern's
second direction agrees with the decoded directions in 54 of 54. Settled and
unsettled items, learning dump (before the import used the settings):

| Rule | Explained | Contradicted | Not comparable |
| --- | ---: | ---: | ---: |
| Circular symmetric flag = accepted answer's `symmetric` | 110 | 2 (one design, two versions, 180 degrees: run again, the import accepts `symmetric` true there, so the old answer was ambiguous) | 24 (22 unsettled, one with the flag set, which is a design the import now settles; 2 unread by the tool) |
| Circular compute (0 / 1 / 2) | the code is never another in 112 settled and 22 unsettled: the accepted answer is the default (`adjust`), so the history cannot test it; 15 of the 22 unsettled have a non-default code against 17 of the 112 settled | | |
| Rectangular distance type = accepted `distance_type` | 25 | 1 (extent with four elements; the answer was a spacing) | 0 |
| Rectangular symmetric, second direction | 3 unsettled have the symmetry set and 3 a second direction; none of the 26 settled | | |
| Rectangular extent | 17 of the 28 unsettled are extent, against 2 of 26 settled (one with two elements, where extent and spacing agree) | | |
| Mirror joined flag = the old reading (no new body) | 14 of 14 mirrors of bodies, 4 of 16 mirrors of features | 12 of 16 mirrors of features (the flag is 0, the old reading 1) | |
| Suppressed list | empty in every one of the 134 circular and 54 rectangular patterns: only the variant gives a non-empty one | | |

Against the original application's own dumps of the real designs (the
API reference dumps; `mitcad-f3d-inspect design --json` joined by
design and timeline index), every decoded setting agrees and none is
contradicted: 90 circular patterns (symmetric 3 true / 87 false; compute
78 adjust / 9 identical / 3 optimized; suppressed lists all empty), 22
rectangular patterns (distance type 14 spacing / 8 extent; symmetric in
direction one 1 true / 21 false, in direction two 2 false; compute 14 adjust /
6 identical / 2 optimized; suppressed lists all empty) and 13 mirrors (joined
6 true / 7 false; compute 11 adjust / 2 identical). The compute option of a
mirror, which the variants cannot show, is therefore read from the layout it
shares with the patterns on real data too. The same dumps confirm the
flip: in all 30 rectangular patterns the decoded signed distance equals the
API's `distanceOne` and the stored vector equals its `directionOne` (never
flipped by the sign), a second direction is present exactly where the API has
one (2 of 30), and the entity kinds agree (2 construction axes, 27 edges; one
edge not resolved by the decoder).

The rules the audit found rejected were not rules of a stored flag: the
distance's sign *is* the flip, so "the axis flipped by the distance sign"
(10 or 12 contradictions) counts the flip twice, and the stored vector is the
entity's direction (an axis' or an edge's) without it. Of the 26 settled
rectangular patterns, 19 answers with an origin axis or a stored line have
that axis equal to the stored vector (the signed distance flips them); the
one that does not is a pattern whose edge direction stored as -x was accepted
as +x with a negative distance (history-ambiguous or a reversed stored edge:
undecided). A direction named by an edge, face, sketch line or construction
axis is therefore also tried by its stored vector after the entity (an edge's
direction in the replay need not point where the file's does).

A symmetric pattern does not mean what the model assumed: the variants show
that a symmetric rectangular pattern of three elements puts them at -1, 0 and
+1 and a symmetric circular one of four over 90 degrees at -60, -30, 0 and
+30 degrees (the same spacing, the quantity in total, the extra element of an
even quantity before the original). The model's `symmetric` now means that
(it was the quantity on each side, 2q - 1 elements: an assumption of the
study, K41, never checked); the guesses for patterns without stored
directions follow.

The import tries a pattern or mirror whose objects, axis or plane, counts
and settings are all read first, with `read` provenance (not the dressup
variant, which stays after the history's suggestions); the guesses follow.
A rectangular pattern whose direction names an edge, face, sketch line or
construction axis gets a second definition with the stored vectors as fixed
directions (read too), after the first.

On the 35 pattern and mirror items of this group's variant models: before,
21 were accepted as inferred and 11 as unknown (none read), two were
fallback and one skipped; after, all 35 are parametric with `read`
provenance (the two with fillets or chamfers repeated on the copies at rank
2, the rest at rank 0). Of the 14 census designs whose patterns or mirrors
were unsettled with a non-default option (the unsettled items with the
extent distance type, the symmetry flag or a second direction set, and a
symmetric circular pattern) and that were imported again, 31 pattern and
mirror items are comparable with the learning dump: 24 were unsettled
(fallback or skipped) before and 8 are after (one design's import stopped:
its debug build overflows in the solver's analysis, as before this change);
the 16 that came in are 15 rectangular patterns and the circular one, and
of the 23 settled in all, 20 are read and 3 inferred. The ones that stay unsettled
are four mirrors (other causes) and four rectangular patterns (a copy
reaches no body in two, the history does not hold the result in two; one
of them keeps defaults of a second direction without an entity).

#### Primitives, coils and option constants (mitcad#25)

Controlled one-option variants (`MITCAD_F3D_MODELS`: 19 primitives, 13 coils,
and chamfer, hole, loft, sweep, pipe, thin-wall, through-all and font
variants) of the newer writer (2705; its class version 9 parameters carry
one more `u32 0`, without which no parameter of such a file was read).
Every field below was read against the dump of the same model.

| Group | Bytes | Variants / real designs |
| --- | --- | --- |
| Box, cylinder, sphere, torus: operation | `u32 operation` (1 join, 2 cut, 3 intersect, 4 new body, 5 new component), `u8 1`, then a run of references (holders, the frame input, a definition) | 19 variants; 8 primitives of one design |
| Their placement | after the run `u8 flag`: 1 the identity, 0 and a row-major 4x4 matrix in cm. The frame input (`7B6C3C2D`) holds the matrix of the plane's or face's frame *with its origin* (none: the XY plane). A point of the primitive's own frame is at `frame * placement * p`; a box's width (`DistanceX`) is along x, its length (`DistanceY`) along y, its height along z | box on the XY plane, on an offset plane, on the XZ plane and on a face at another point; sizes; join and cut; cylinders and spheres likewise; all verified against the stored bodies (the bounding boxes of the dumps' bodies) |
| Torus position | `u32` after the placement: 0 on the centre line, 1 outside, 2 inside | 4 variants |
| Coil | `u8 flip` (1: left-handed), `u32 operation`, `u32 type` (1 revolutions and height, 2 revolutions and pitch, 3 height and pitch, 4 spiral), then the holders and the frame input, the placement, then `u32 position` (1 inside, 2 on centre, 3 outside) and, after one more reference, `u32 section` (1 circular, 2 square, 3 triangular outside, 4 triangular inside) | 13 variants; 1 real design (4 copies): all read |
| Chamfer set | tangent chain and flip bytes (already read) agree with the dumps on the six variants; the flip means which face takes the first distance, the face of the coedge that runs the edge's way, which the replay's topology has, so the import still tries both readings | 6 variants |
| Hole direction | the first null reference behind the header, then `u32 type` (0 simple, 1 counterbore, 2 countersink) at +9 and `u32 flipped` at +13 (class version 7 records) | 39 holes of the dumps: all agree; none of the 17 holes of older writers is decoded |
| Loft | after the rails `u8 isClosed`, `u32 isSolid`, `u8` | 9 variants; 14 real lofts all open and solid |
| Sweep | `u32 orientation` after the ten slots: 1 parallel, 2 perpendicular | 8 sweeps with dumps; 18 real sweeps perpendicular |
| Pipe | `u8 isHollow` before `u32 operation`; `u32` after the first input slot: 0 circular, 1 square, 2 triangular | 5 variants; 2 real pipes circular |
| Thin extrusion, two sides | `u8 1 | u32 location | holder | u32 location two | holder two | u32 1 | ref` | 6 variants |
| Through all, newer writer | the flag byte of the older writers is 1 exactly where `isPositiveDirection` is false; the direction given to `setOneSideExtent` changes nothing | 4 variants |
| Text style | the second flag byte after the horizontal alignment: 1 italic (bit 2) | 4 variants |

What the variants showed about geometry: the stored coil's section lies in
the plane through the axis (its size is the circle's diameter, the square's
side, the triangle's height; the radial extent is placed on the diameter),
which Mitcad's coil now builds by the helix's screw motion; a pipe's square
size is its side and a triangle's its height with the path at its middle.

What the loft, sweep and font variants showed, and the kernel work that
followed (each checked against the stored bodies, import report
*verified*):

- *Lofts through three or more sections without end conditions or rails*
  (open or closed) are piecewise cubic Hermite surfaces: each point of a
  section follows a cubic piece through its matches in the sections, whose
  tangent at a section is the derivative of the parabola through it and
  its neighbours, the first and last pieces being the parabolas that
  arrive with the inner tangents; a closed loft is the periodic form.
  The stored surfaces' centre lines and volumes agree to 7 digits (an open
  ring of four circles 9503.318 mm3; closed 12880.53, which the kernel
  built as no solid before the closing curve and the corner matching were
  right; the old C2 interpolation gave a ring 12 % fuller and an open one
  17 % too large). The matching of the sections' corners takes a section
  turned a quarter round the axis a corner off, so each section is turned
  to lie nearest to the one before it.
- *Surface lofts* (`isSolid` false) are sheet bodies of the side faces
  (`surface`), a new body; the volume of an open shell is 0.
- *Direction end conditions*: the angle is measured along the loft, so at
  the last section a positive angle arrives moving outward (the takeoff
  into the loft tilts inward); the end section's loft was 49 % too large
  with the other sign and is 1.6 % small now, the start's 1.2 % large.
  What stays: the stored takeoff fields (the derivative across the
  section, row by row of the circle's polynomial form) carry a fourfold
  ripple (the free end's 8 % down at the diagonals, its direction wobbling
  by 2.6 degrees; the direction end's 1.5 %), a property of the stored
  fit that no rule of the section's geometry gives, so these lofts stay
  fallbacks of the history (0.5 % accepted).
- *A tapered sweep* leans each side of the profile by the angle (the
  outline offset by the distance times the tangent; a square stays a
  square, a circle is a cone), as an extrusion's taper does; Mitcad scaled
  the whole profile (10650.14 mm3 for the reference model, exact now).
- *Sketch text height*: the file's capital height is that of the glyph A
  (the apex is flat in a sans-serif face but reaches the roman face's
  overshoot, 1387 against 1356 units of 2048 for a roman face's H), which
  explains the 0.9776 scale of one roman face (exact for the sans-serif
  ones); all three bodies of the reference text agree to 1e-6 now.

Not translated, with the cause found: a chamfer without its tangent chain
where a round continues the edge (the kernel always follows chains), and
four primitives that stand on faces of a component inserted from another
design.

#### Twin edges and a thread's start angle (mitcad#106, mitcad#186)

Two groups the corpus alone could not teach were read from controlled
one-option variant models (a shaft with a milled flat, a block with a bore
breaking through a face, each with the fillet or chamfer on either mirror-image
edge or both; a revolved shaft with its seam turned 0, 45 and 90 degrees, a
thread with offsets 0, 1 and 2.5 mm) and checked on the census.

- **Twin edges.** Edges between the same two faces with the same faces at their
  ends fit the same recipe names. The recipe's tail lists the faces round each
  end vertex counter-clockwise seen from outside the body
  ([`core/f3d/EDGE_ORDER.md`](../f3d/EDGE_ORDER.md): layout, proof). The
  decoder writes them as `_f3d.end_faces` and takes, among edges that fit
  equally, those whose faces are in that order (`NamedState::edges_fitting_ordered`),
  or one of edges at the same place; the edge then has its geometry in the
  dump and its input is `read`, not chosen by the history. The layout holds
  for all 1,559 distinct tails of the corpora
  (`tools/f3d-learn/rules/edge_tail_grammar.py`: 1,932 fillet and chamfer items
  explained, none contradicted, unsettled ones included). Of 157 edge inputs
  in the 538 designs that did not fit one edge, 29 now do; no edge found
  before changed, and the 13 new edges in the 321 reference designs that the reference
  dumps list are the selected edges, not their mirror images. The other 128
  remain: end faces the recipe names as an earlier state had them, closed
  edges, vertex fans that repeat a face, patterned copies with the same order,
  and the n inputs of one item with the same names (each would choose alone and
  two could take the same edge; they take the n edges that fit in order, as
  before).
- **A thread's start angle.** The variants differ in the seam of the threaded
  cylinder only, and the angle found in later states follows it (0, 45, 90
  degrees) while the offset (0, 1, 2.5 mm), length and location change
  nothing. The file's helix starts at the seam of the cylinder's surface at
  the end of the face the file's axis starts from. The decoder gives a whole
  cylinder's seam direction (`_f3d.seam`); with the file's axis pointing up
  (Mitcad's helix starts at the low end of the axis pointing up) the import's
  angle is the seam's direction about the axis from the world X (Y) reference
  (`thread_phase`), and the measurement in later states is skipped. Of the
  census' 20 modelled threads of this kind 18 agree exactly and two within
  2.3 degrees; with a right-handed thread only (a left-handed one, a part of a
  cylinder and an axis pointing down go on being measured). For an axis
  pointing down the file's helix starts at the other end; the angle there
  follows the face's length and the pitch by a formula that agrees with the
  census threads of this kind whose length is a whole number of turns
  (a half-turn from the seam) but not with the two that are not (the
  face is cut obliquely there), so it is not adopted. The learning rule
  `tools/f3d-learn/rules/thread_seam_phase.py` checks a dump made with the
  decoder that writes the seam. A modelled thread whose angle was read, whose
  faces were named and whose handedness was read has `read` provenance.

#### Stored edge curves (mitcad#306)

An edge input (`5662F619`) refers, besides its recipe, to the curve of the
edge as it was when the input was made: after the reference's two GUIDs a
list `u32 n | n refs` of curve objects (classes deriving from `518AD6D6`),
in the component's coordinates, cm and rad
(`core/f3d/src/design/curve.rs`). After the root part:

| Class | Layout |
|---|---|
| `50AEE8B4` line | `f64[3] start, f64[3] vector` (start to end) |
| `B0B3FA59` arc | `f64[3] centre, f64[3] axis, f64[3] reference direction, f64 radius, f64 start, f64 end` (angles from the reference direction towards axis × reference) |
| `98E8921B` elliptical arc | `f64[3] centre, f64[3] major direction, f64[3] minor direction, f64 major radius, f64 minor radius, f64 start, f64 end` (the parameter t of centre + major cos t · major direction + minor sin t · minor direction) |
| `2319971D` spline | `u8 a, u8 b, u32 degree, f64, knots, weights, poles, f64, u8, u8, f64 start, f64 end` (the edge's parameter range); knots and weights `u32 n, u32 n, u32 8, n × f64` (no weights: 0), poles `u32 n, u32 n, u32 8, n × f64[3]`; `a`, `b` and the two `f64` (small tolerances) are not read |

Every curve ends with its base classes' `u8 0, ref, u8 0, u8 1, ref`, both
references null (one byte 0) in edge inputs (a sketch profile's curves refer
to the profile there). On the reference models (`MITCAD_F3D_MODELS`) the
117 curves the fillet and chamfer edge inputs store are all edges their
sets list in the reference dumps, ends and middle
(`stored_edge_curves_match_the_reference_dumps` in
`core/f3d/tests/design_models.rs`). On the corpora (`MITCAD_F3D_CORPUS`, the backup
corpus and `MITCAD_F3D_MODELS`, 856 distinct designs) the edge inputs store
15,636 curves, all of which decode (7,145 lines, 6,527 arcs, 459 elliptical
arcs, 1,505 splines); 57 edge inputs store none, and 139 store two or three
(an edge the input saw in pieces, each piece a curve: the stored curves of
one spline then have ranges far apart on one parameter).

The decoder writes the curves as `_f3d.curves` (`Line3D`, `Arc3D`,
`EllipticalArc3D` with `normal` = major × minor direction, `NurbsCurve3D`)
and uses them in two ways:

- **Check.** The edge found is compared with the curves (mm, within 0.01 mm)
  and `_f3d.curve_fit` says how it lies: `the stored edge` (its ends are the
  curve's), `a piece of the stored edge` (on it, within its ends: a later
  item cut it), `on the stored curve, … beyond its ends`, or `off the stored
  curve by …` (with `(curve k of n)` for an input of several). Of the 13,952
  edge inputs found in the corpora 13,763 lie on their curves within their
  ends (13,710 the stored edge, a closed edge on its closed curve wherever
  its seam lies; 53 a piece of it), 45 on them beyond their
  ends and 144 off them: 68 for items suppressed in the file (the design
  changed after the item was suppressed: mostly the same edge, moved by 1 or
  10 mm), 54 found a few states before their item (the edge moved since),
  22 in the state before their item (splines whose names fit an edge
  4 to 40 mm away, and elliptical arcs 0.1 to 0.4 mm off; not settled).
  The check does not reject an edge on its own: an edge that moved is still
  the one the names give.
- **Decide.** Where the names fit several edges, the edge that is the stored
  edge, else the only piece of it, is the input's (`found` ends with `(by
  its stored curve)`), before the recipe's tail, the n inputs of the same
  names and the inputs of a set that fit alike: those take the edges in list
  order, and on the corpora 52 inputs had taken another of the edges (most
  the mirror image or the next copy of a pattern of the edge they name); 6
  inputs the names left open are now found. The affected designs imported
  with the decoder's dumps before and after: three items whose edges the
  history had to settle (`inferred`) are `read`, no other outcome and no
  body or history check changed.
- **Several edges** (mitcad#106). An input stores a curve per edge it stood
  for; where two or more edges its names fit lie on its curves within
  their ends (on one body; edges on one curve only as its pieces, together
  no longer than it), a fillet's or chamfer's input is all of them: the
  first is the input's, the others edge inputs of their own after it in its
  set (`found` ends with `(one of the edges its stored curves give)`; an
  edge the set has already is not added again). This comes before the one
  edge above. On the corpora it found the 86 inputs the names and curves
  had left ambiguous (none is left; `core/f3d/EDGE_ORDER.md`, *The curves
  an input stores*), the sets taking 139 edges more; 30 inputs of rings and
  of the same names that had taken the set's edges one each in list order
  now take those their own curves give (the sets' edges are the same). An
  input still ambiguous says how each edge that fits lies to its curves.
  Eleven affected designs imported with the decoder's dumps before and
  after: nine items whose edges the history had to settle (`inferred`)
  are `read`, no other outcome and no history check changed.
- **Inputs that found one edge.** Inputs of one fillet or chamfer with other
  names and other stored curves that found the same edge cannot both have
  found theirs. Those whose curve the edge does not lie within (it runs
  beyond its ends, or lies off it) are left not found with the reason; an
  input whose stored edge was longer than the edge now is (mitcad#305) keeps
  it, and two inputs of the same stored curve (two names of one edge) keep
  theirs. On the corpora 9 inputs are left out this way: 7 arcs of a
  pattern's copies whose names fit one edge 11 states earlier (1 mm off all
  of them), and two inputs whose names fit another input's edge, 28 and
  40 mm from their own curves. An item suppressed in the file keeps such
  edges: it is looked for in the state after the item before it, which the
  file never had, and there pieces of one edge its inputs saw can be one
  edge (on the corpora two or three pieces of one line or elliptical arc,
  each input taking the whole edge, in five items of five designs, versions
  of three; leaving them out took the definitions of such items away).

The inspector's `edge-curves` command lists the curve classes of a file, each
edge input with its fit, and the inputs of other names that found one edge.

#### Where a circle starts (its seam)

A sketch circle record always names the sketch's x axis as its reference
direction, but the closed edge the file's bodies hold for it starts
elsewhere: on the x axis, on the opposite side or at another angle, with no
rule of the sketch's frame, the extrusion's direction or its operation (in
the designs checked the opposite side and the x axis split about 60 to 40;
every edge's parameter runs from -pi, so the start vertex is opposite the
curve's major axis). The replay started every circle on the x axis, so the
cylinder's seam edge and the vertices at it were on the other side: a plane
through three seam vertices, a tangent plane at one, a seam edge in a
fillet's set and the items after such an item failed.

The decoder reads the start from the bodies (`_f3d_seam`, SCHEMA.md): for
each whole sketch circle an extrusion uses, the closed edges of the state the
first such extrusion made are searched for one on the circle's axis and in a
plane across the sketch's normal with its radius; at the sketch's plane
first, else at the other end of the extrusion if the ends agree; its start
vertex's angle from the sketch's x axis is the circle's start. The model's
sketch circle has `start_angle` (radians, default 0, so Mitcad's own
sketches are unchanged), carried into the profile (`SegmentGeometry::Circle`
`start`), the path curves of sweeps and the kernel's circle (the axes of the
circle are turned by it, so the one edge, the cylinder's seam and its
vertices are there). Of the 5722 sketch circles of the 170 designs that had
problems in the census, 3508 are found in the bodies (3146 start elsewhere
than the x axis, in 128 designs); the rest are circles no extrusion uses or
that the bodies hold only split. An extrusion up to a surface that the
sweep crosses twice (a cylinder entered from outside) now ends at its first
crossing.

Run again on the 125 of those 128 designs that finished both runs (one run
with the seams read, one without, the same build): 3 items went from
fallback or partial to parametric (two dressups whose edge sets name a
seam edge, a plane through seam vertices), 3 got worse where the
geometry kernel is sensitive to where the seam lies (a chamfer that fails,
a modelled thread whose volume is 0.7 % off, and a fillet after an item
that now reaches the kernel and fails there instead of being skipped
unchecked); parametric items read from the file rose by 2.

#### Construction datums, sketch constraints and dimension classes (mitcad#186)

Read from one-option variant models (`diff_cplane_*`, `diff_caxis_*`,
`diff_conline_*`, `diff_constraint_*`, `diff_dim_*`, plus the reference
models `cons_axes`, `cons_points`, `sk_text`, `sk_spline_control`,
`con_smooth`), each with the API dump of the original application as ground
truth, and checked on the real designs against their API dumps (`f3d-api-ref`).
The learning dump holds no datum items, so there are no `learn.py` rules; the
checks are the reference-model tests in `core/f3d/tests/design_models.rs`
(`construction_definitions_match_the_reference_dumps`: 123 definitions agree,
none differ or are left undecoded; `sketch_dimension_and_constraint_types_
match_the_reference_dumps`: 1212 sketch lists agree) and counts against the real
designs' API dumps, below.

| Option | Where it is stored | Verified |
| --- | --- | --- |
| Plane definition | the `u32` after a one byte flag behind the item's root part (`_f3d.datum_code`): 0 offset (also parallel through a point), 1 angle, 2 three points, 3 mid plane, 5 and 10 tangent, 6 along a path, 7 two edges, 9 tangent at a point. Items made by older writers store a tangent plane as 1 or 5; the face as the line of an angle makes it a tangent plane | codes against definitions in the real designs: 164 planes, every code one kind (1 and 5 tangent or angle) |
| Vertex input | a `FACE_REF` input whose recipe is `vertex_recipe_data`: the faces that meet there, one entity each; the position is found in the history state at the item's place (`NamedState::find_vertex`) | 3 vertices of the three-point plane models and the point models: all positions as the API's |
| Input wrapper | class `2CA5A1CD` before each wrapped input, the role of the input after its slot key object (not read) | skipped in the item's inputs |
| Point definition | code 0 at a vertex or point (one input), 3 at the centre of a circle (an edge), 2 where three planes meet; its position from the key point input | `cons_points`, the vertex variants, the corpus' one point |
| Axis definition | no code: by the inputs: an edge or a sketch line, a face (round: through it; planar, with no other input: perpendicular to it at the axis' own origin), a face and a point (normal to it at the point), two points, two planes | `cons_axes`, ten `diff_caxis_*`, 4 of 8 axes of real designs (the others: normal to a sketch profile at a point) |
| Constraint bits | `0x200` smooth; bit 31 the control polygon of a control point spline; bit 40 the box of a sketch text; bit 41 a text and a curve (unverified). The last three constrain no geometry (the API lists none) and leave the sketch whole | `con_smooth`, `sk_spline_control`, `sk_text`; real designs' sketches: 1259 agree with the API's constraints by type |
| Rectangular pattern constraint of newer writers | the same record, the sketch reference over 200 bytes into it | 2 constraints, values as the API's |
| Line kind | construction and centre line flags, read already | 36 `diff_conline_*` and `diff_constraint_*` models: every type on normal, construction and centre lines decodes, with the flags |
| Dimension classes | ellipse radii: the one set byte of the last eight flag bytes, 1 or 4 major and 2 or 8 minor; class `A9E23439` the distance of a line from a circle or arc; a point or line with a reference to a face or plane outside the sketch | all `diff_dim_*` models; real designs: 1261 sketches' dimension types agree with the API's (two sketches of one design differ by index) |

Not decoded: the point of a perpendicular axis is not an input, only the
axis' origin; four axes normal to a sketch profile at a point; the face or
plane of a dimension outside the sketch (an `error` reference). Mitcad has
no line-to-circle or point-to-surface dimension, so those dimensions are left
out of the sketch (`partial`). A smooth join with a fitted spline cannot be
kept either (the spline's control points are fixed, not shared with the
arc's end). A vertex at a circle's seam is found now that the replay's circles start where the file's do (see "Where a circle starts" below), so a tangent plane at such a vertex and a plane through three of them are parametric.

The import takes a definition's first reading as `read` where the file names
every entity outright, the others as `inferred`, and keeps a datum only
where it lies where the file has it (planes: normal and distance; points;
axes: line and direction, the reverse direction as a second candidate).
A planar face is its whole plane for the point of an axis perpendicular to
it (`commands.md`), so that a point off the face projects as the original
does. New plane definition `offset_through_point`.

On the 27 designs of both private corpora that hold the planes left fixed,
the axes and the one point (a targeted run, not a coverage run): of 33 fixed
planes 9 are now parametric and read, of 8 fixed axes 4 (inferred: the stored
direction is the reverse of Mitcad's rule) and the point (read); 40 planes
of 93 datum items are `read` (none was before). What stays fixed: items with no ASM state to
find a vertex in, faces the replay cannot name, planes along several sketch
curves, a mid plane and an offset whose inputs the file does not name, and
four axes normal to a sketch profile.

##### Against the API reference dumps

The reference dumps hold the original application's own answer for 321 of
the corpus designs (5,224 timeline items; the per-step walk, so dressup
edges and every input are as the application read them). The decoder's dump
IR has the schema of the dumps, so `tools/f3d-learn/apiref.py` joins the
two per timeline item (a global alignment on type, name and index: 5,184
items paired, 40 only in the reference, 64 only in the decoder's dump,
mostly items it names by their class) and the learning dump's accepted
answer by item index. It counts each field of each input group once per
item or sketch element as agree, differs, decoder missing or reference
missing, and the accepted answer against the reference in the same way;
entities compare by what they are (faces and edges by geometry, sketch
curves by their geometry in the sketch, features by name), numbers with a
tolerance of 1e-4 cm. Output: a table per group, and per field the
mismatches with the design, item, the decoder's or the accepted value and the
reference's.

```bash
python3 tools/f3d-learn/apiref.py compare --api DUMPS --ir IR --census LEARN \
    --index DUMPS/index.tsv --root CORPUS_ROOT --out OUT
```

Decoder against the reference (items or elements; measured on the build with
the one-option decoding of extrusions, patterns, threads and datums):

| Group | Reference values | Decoder agrees | Remaining |
| --- | ---: | ---: | --- |
| Timeline items | 5,184 | names 4,735, types 5,082 | Names: 449 differ (368 occurrences, whose names carry the component and a counter; 43 sketches, 34 of them renamed by the user under a name the file keeps outside the item; 20 joints, as-built joints and rigid groups and 14 constraint sets, which the application numbers by kind). 165 construction planes, axes and joint origins were shown under the file's internal names and now agree. Suppression (80 items) and the rolled-back marker (43 items) 5,184 of 5,184. Health (12 errors, 52 warnings) is not stored in the items |
| Extrusions | 1,281 | operation 1,281, extent type 1,281 (symmetric included), distances 1,148 (their sign is the side), taper 1,281, direction 1,219, extent kinds (distance, through all, symmetric, up to an object) 1,281 and 32 second sides, through-all direction 78 of 78 (and 4 of 4 second sides), symmetric length 7 of 7, `hasTwoExtents` 1,281, start kind 1,281 and the profile plane 1,188 of 1,192, up to an object: chain 55 of 55, nearest solution 55 of 55, direction hint 55 of 55 (and 2 of 2 second sides) | up to an object: the face 52 of 55 and 2 of 2 second sides (the three others are one design whose faces the name lookup resolves to other faces); no thin extrusion occurs in the corpus (the thin flag is read on 1,265 of 1,281, the 16 others are class versions without the block); the accepted answers: through-all side 62 of 68 |
| Extrusion profiles | 2,623 regions | exactly 2,114, held by a decoder region with extra curves 295, by one outline 2 and in another loop structure 1 | 211 missing, 149 extra regions: 134 and 95 in 37 items whose stored loops name curves the sketch no longer has (822 of 27,589 records), 77 and 54 in 33 items whose records all resolve (the application splits and merges loops differently; see the round-2 subsection); 211 profiles name no curves in the reference; the participants the operation visibly changed are among the decoder's in 913 of 968 |
| Revolutions | 101 | operation, angle 101, axis 101 (the accepted answers 71 of 81), solid 101, symmetric 101, project axis 94, participants 34 of 39 | project axis of the 7 class version 1 revolutions (no byte); regions 63 exact, 7 held, 10 missing |
| Circular patterns | 105 | axis 101 of 104, quantity, angle, compute option (87 adjust, 10 identical, 8 optimized) and symmetry (3) 104 of 105 | the accepted answers say adjust and not symmetric: compute agrees 73 of 84, symmetry 82 of 84 |
| Rectangular patterns, mirrors | 39; 17 | direction, quantity, spacing, compute option, distance type, symmetry 31 of 39; mirror plane, compute option 15 of 17, `isCombine` 15 of 15 | 8 patterns of occurrences carry no inputs; the accepted answers: rectangular compute 7 of 12 |
| Fillets | 1,158 | edge sets 1,158, edges 1,026 (the reference lists no edge for 46 sets), options: tangent chain, continuity and corner flag 1,158 | 86 edge lists differ: 43 other edges (27 are equally long edges elsewhere on a symmetric or patterned part, which the names and the recipe's tail do not settle), 21 whose names fit several edges equally or none, 11 in items whose references the application reports lost and 11 in items rolled back after the timeline marker, for which the file keeps no state (*Fillet and chamfer edges against the reference*) |
| Chamfers | 193 | edge sets 193, edges 186 (the reference lists none for 2), sizes in the sets | 5 edge lists differ; tangent-chain and flipped options are not decoded (the reference has none false); the accepted answers' edges differ in 6 of 176 sets, a two-distance chamfer's side in 11 of 46 |
| Holes, threads | 10; 77 | threads: every decoded field of 77 (designation, class, diameters, flags, length, offset, location) | holes: extent (all 10 through all), position (face, both offsets, edges 7 of 7 and 7 of 7) agree; handedness 75 of 77 and 2 of 10 holes (the class version 1 thread object has no handedness byte; all are right-handed); no direction, default direction, taper or hub fields (constants in all 10 holes and 77 threads) |
| Shells, combines | 4; 33 | operation, keep-tools 33, thickness 4; shells: tangent chain, type, faces 4 of 4; combines: target and tools 25 of 31 | combines: 6 of 31 whose bodies have no producer in the decoder or share a name with another body; `isNewComponent` (false in all 33) has no contrasting byte |
| Construction planes, axes | 123; 8 | geometry 123 and 8, definition type 123 and 8 of 8, offsets, angles, distances, line inputs 8 of 8, path inputs 21 of 21, plane faces 42 of 48, profile inputs, point inputs of the axes 4 of 4 | 6 plane faces, the vertices of 5 three-point planes and 2 tangent faces are not found by name in the history (ambiguous or stale names; the same designs' faces); `isParametric` (true in all) is not stored per item |
| Sketch curves | 29,762 | geometry of every curve (4,394 of 4,394 circles); type 29,762 (the 108 control-point and fitted splines included); construction 29,762, centre-line 16,237, fixed (717 set) and linked (18,577 set) flags 29,762 of 29,762 each | none left: see [Sketch curve flags, spline kinds and sketches without a link](#sketch-curve-flags-spline-kinds-and-sketches-without-a-link-mitcad186) |
| Sketch points, constraints, dimensions | 39,658; 9,919; 3,867 | points 39,622; constraints 9,888; dimensions 3,859 | 31 constraints (23 of them rectangular-pattern constraints); 36 points, the corners of text boxes; driving flag 3,862 of 3,862 (63 offset dimensions without a parameter are driving, not driven) |
| Parameters | 12,332 | 10,509 by name; expressions 10,372, values 10,383, units 10,457, roles 10,174 | half of the 1,823 missing are in five assembly designs |

The accepted answers (settled items only) mostly agree where they carry a
field: extrusion operation and extent 1,116, distances 995, regions by area
1,038 of 1,066; circular axis 74 of 74 comparable, quantity and angle 84 of
84, revolution operation and angle 81 of 81, axis 71 of 75. Where the history
cannot tell, the guess is wrong-but-accepted: compute option 18 of 108
patterns and mirrors, a through-all extrusion's side 6 of 68; the edge list
of 63 of 906 fillet sets and 6 of 176 chamfer sets (the first comparison
counted 252 and 63: it compared a closed circle by its middle, which the
file puts opposite its seam and a replay elsewhere, read only the first
definition of an answer made of one feature per tangent chain and counted
the pieces a replay splits an edge into as other edges; the comparison now
accepts the circle, joins the definitions and the pieces and takes a face
selection and a spline's middle as they are meant). Participants are not exposed by the
reference (its value is an object handle): they are derived from the
bodies whose volume the step changed, so a participant that changes nothing
is not counted.

##### Round 2: extents, holes, revolutions, shells and datums (the API reference dumps)

The comparison above was run on the extrusion, hole, revolution, combine,
shell and datum groups, and each mismatch classified as the comparison
tool's own noise, an input the decoder reads wrongly or not at all, or an
input the file does not hold. Every rule below was found on bytes (the
controlled one-option models and the corpus' records), checked against all
the reference dumps, and read by the import before the history's guess.
Fields agreeing, before and after (the tool's own fixes, below, are part of
the change):

| Group | Agree before | Agree after | Differ / decoder missing after |
| --- | ---: | ---: | --- |
| Extrusions | 16,784 | 21,956 | 281 / 1,836 (mostly the 1,194 profile areas, which are computed, not stored; the regions; 45 participants; 16 thin flags) |
| Holes | 182 | 256 | 0 / 118 (constants of all 10 holes: default direction, taper and hub fields) |
| Revolutions | 721 | 1,057 | 18 / 147 (99 profile areas, 7 project-axis bytes, 26 unresolved profiles) |
| Combines | 66 | 116 | 8 / 37 |
| Shells | 6 | 16 | 0 / 0 |
| Construction planes | 448 | 482 | 17 / 129 (123 `isParametric`) |
| Construction axes | 15 | 27 | 1 / 8 |

| Input | Encoding found | Against the reference dumps |
| --- | --- | --- |
| Through-all direction (every writer) | the flag byte after the extent codes belongs to the side through all, whichever side that is: 0 along the sketch's normal (`isPositiveDirection` true), 1 against it; also on a symmetric through all (flag 0) and a two-sided extrusion's second side | 78 of 78 first sides and 4 of 4 second sides; the accepted answers had 62 of 68. The import's first candidate follows the byte, the opposite side and both sides stay guesses, and the extent is `read` with either byte value |
| Symmetric extent | extent code 3 with the side kinds `[1, 1]` (a distance, whose length byte says whole or half) or `[4, 4]` (through all, the whole length by definition); the dump gives `SymmetricExtentDefinition` as `extentOne` and `symmetricExtent` | 11 of 11 types, 7 of 7 lengths. The import keeps the other length as a guess |
| Up to an object | the slot role 17 (18 where the faces join a chain) names the object: `isChained` is the role; the byte after the object's reference is `isMinimumSolution`; `directionHint` is the stored vector turned round by the flag byte; two sides both up to an object keep their slots second side first; a role 5 slot with bodies as its input is an extent up to a body | chain 55 of 55, nearest solution 55 of 55 (one is 1), hint 55 of 55, the faces 52 of 55, the two second sides 2 of 2, up to a body 2 of 2 |
| Extents' other fields | `isSymmetric` false on a distance; `hasTwoExtents` by the extent code; the start kind byte 0 is a `ProfilePlaneStartDefinition` whose plane is the sketch's x-y plane in sketch coordinates | 1,167 of 1,167 distances, 1,281 of 1,281, 1,188 of 1,192 |
| Hole extent | a distance unless the hole has a slot of role 5: bodies as its input mean through all, a plane or face means up to it (the controlled models: a distance `[4]`, through all `[5, 4]`, up to a plane `[5, 4]`). The byte the earlier decoder read as the through-all flag is 0 on four of the ten through-all holes of the dumps; it is 1 where only one of the two position edges is referenced (6 of 6) | 10 of 10 through all (the accepted answers 6 of 10) |
| Hole position | the direction input's face, the two edges (the slots of role 33, in order) and the two reference dimensions | face 10 of 10, offsets 10 of 10, edges 7 of 7 and 7 of 7 (three first edges are stale in the application, which gives an error there) |
| Revolution record | after the operation `u32 2 | u8 | u8 project axis | u8 solid | u8 symmetric | u8 two sides | [second angle holder or 00] | angle holder` (class version 1 has no project-axis byte); the axis input may be a cylindrical or conical face or an edge; the participants are the slot of role 4 or 8 | solid 101, symmetric 101 (1 symmetric), project axis 94 (the 7 others are version 1), axis 101 (the accepted answers 71 of 81), participants 34 of 39. The import reads the extent of a full turn, a symmetric one or two angles, and a face axis is fixed where the face was found |
| Shell options | the two bytes after the root part: tangent-chain flag, shell type (1 rounded, 0 sharp) | 4 of 4 shells and the 4 controlled models |
| Datum profiles | a profile source among a plane's or axis' inputs is a profile of its sketch | planes 2 of 2, axes 4 of 4 |

Tool noise found and fixed in `apiref.py` (and its unit tests): a face's
plane has no side (the same plane with either normal: 12 extent faces, 2
shell faces, 11 plane inputs); bodies are compared by the item that made them
and their place among its bodies, not by the application's names (50 combine
bodies and 2 extent bodies); an empty list of participants says nothing; a plane's
`pathEntity` is the decoder's list of path edges; a hole's thread is a
reference in the application and a reflected record in the dump; a solid
extrusion's wall location is the application's default; two regions over
the same curves in another loop structure, and regions an outline holds, are
counted apart.

Profiles. Of the 211 regions missing and 149 extra: 134 and 95 are in 37
items whose stored loops contain records for curves the sketch no longer has
(822 of the 27,589 loop-curve records: 727 ids exist nowhere in the file,
95 name a curve that was split afterwards under another secondary id; the
profile is older than the sketch's last edit, so nothing in the record
names the current curves). The other 77 and 54, in 33 items, have every
record resolved; in the ones examined the application's loops merge holes that
touch along a curve, or list the regions of a union one by one where the file
keeps the union's outline, or list a curve whole that the records split into
pieces. The decoder reads the stored loops as they are; the import's even-odd
rule on the pieces (above) selects the regions.

Not read. Combine `isNewComponent` (a byte after the keep-tools byte is 0 on
all 33; no contrast). A hole's direction and default direction (the stored
vector is the start face's outward normal on all 10 holes and the application's
direction its opposite; a flipped hole would go with it, and no byte for that
is known). A revolution's direction of a partial angle (two partial
revolutions in the dumps). Thread fields of a tapered or hub thread, and the
handedness of the class version 1 thread object (all constants in the dumps).
The faces of 6 planes, 5 three-point planes' vertices and 2 tangent faces,
three extent faces of one design: the names are found in no state or in
several bodies (a face named by tag and operation only, in a history of six
bodies that share the names).
##### Sketch curve flags, spline kinds and sketches without a link (mitcad#186)

Every sketch curve class ends with the sketch-curve part (class `9FF71F46`),
whose last bytes are the block `01 | u32 n | n references | 13 bytes | f32
1.0, f32 1.0`. The block holds offset chains (`n` is 0 for all but the
curves an offset touches) and moves with them; the flags lie a fixed
number of bytes before its `01`, by the class's version (`curve_flags` in
`core/f3d/src/design/sketch.rs` has the table; versions 7, 8 and 9 occur).
The earlier rule (kind 22 bytes before the pair) was right for version 9 and
no references only, and gave nothing for curves with a reference (the
construction lines of offsets) and the older layouts. Against the API
reference dumps, 29,762 curves of 321 designs:

| Field | Set in the dumps | Before | After |
| --- | ---: | ---: | ---: |
| `isConstruction` | 585 | 29,729 agree | 29,762 |
| `isCenterLine` | 26 | 16,237 of 16,237 lines | 16,237 |
| `isFixed` | 717 | not read | 29,762 (717 set) |
| `isLinked`, `isReference` (always equal) | 18,577 | not read | 29,762 (18,577 set) |

A linked curve comes in as a fixed reference entity (Mitcad's `fixed` and
`reference` flags, as the model gives its own projections) and the sketch's
import note counts them. The model has linked projections (`Projection`:
a topological name and the entities made from it), but the curve does not
say what it follows: no source reference was found in the curve, the
sketch or any object that refers to the curves (the API's referenced entity
is a face for 1,491 of 1,512 linked curves of five sampled designs, an edge
for 20 and a sketch entity for one). Importing them as projections needs that source in the
file and the face or edge resolved to a replayed body's name; neither is
decoded, so the source is found in the replay by where the curve is
(*Sketches on other components' faces, linked curves as projections*,
mitcad#225).

Linked curves on the model's edges. The numbers stored for a linked curve
are single-precision rounded, up to 1e-5 mm and more off the geometry it
follows, while the replay's edge is exact: a join along the wall the
curve follows then no longer touched it, and a tool face tangent to the
circle gave an invalid boolean. Before the sketch is added, each linked
line, circle, arc and ellipse that an edge of the replay's bodies projects
onto within 2e-3 mm goes to that projection (`snap.rs`; its shared points
with it; the note counts them: "(n on the model's edges)"). A snap must
not change the sketch's regions (mitcad#227): the curve is moved only when
the curves it meets, and how (a crossing, an end on a curve), stay those
of the file's numbers, for it and for every curve that moves with it (one
sharing a point, or whose point the file holds coincident; a point the
file holds on the curve goes with the curve, as the solver takes it);
corners two curves share are the solver's and not counted. A curve whose
contacts would change stays where the file stored it, and the note says
"n left where the file stored them". Elliptical edges
and splines are not matched (the kernel lists no curve for them). Of the
35 designs this was tried on, it made 9 extrusions in 9 designs
parametric, and the sketches of four designs that could not be solved
(fixed elliptical arcs whose stored end was a few millionths of a
millimetre off the ellipse; the sketch solve now retries at 1e-7 of the
sketch size when the first does not converge).

Spline kinds. The comparison pairs 404 splines of the dumps (102
control-point, 6 fitted, 296 fixed); the type agrees for all. A fitted
spline has a filled fit-point list (`F81C561A`: the sketch points between
its ends, ending with the reference back to the spline) or handle list
(`4224E565`: its tangent handle lines): 6 of 6, and no other spline has
either list filled. A control-point spline is one an offset made (a child
of the offset's chain) or one with the fixed flag; of the 96 splines that
a join by their control points pairs, 10 are offset children and 86 have
the flag, and no fixed spline has it. Every other spline is a fixed spline. The
dump IR gives the type and, for a fitted spline, `fitPoints` (the ends and
the points between); the import keeps all three as the stored NURBS curve
(a fitted spline of the file is a degree 5 curve, which the model's cubic
fitted spline does not reproduce). The handle lines are no curves of the
sketch (the API lists none), so they are left out and a constraint on one
names the line itself, as the API does.

Sketches without a link. A sketch whose feature names it through a recipe
object (`7ACC2A03`) was not found from the sketch; the feature's own
reference to the sketch now finds it (seven circles and eight constraints
of the dumps, in one design). Remaining against the dumps: the reference
plane of 913 sketches (a face the decoder does not name; not a curve
flag), 36 points that are the corners of text boxes, 31 constraints.
##### Parameters, timeline names and the small input groups against the API reference dumps

What was the comparison's own noise and what the decoder's. The comparison
now pairs parameters by their creating item, leaves out what the reference
cannot answer, and compares by family where the two name one thing twice.

| Noise in the comparison | Count | Handling |
| --- | ---: | --- |
| Parameters of the other designs a file links to (the reference lists every component design's table, whose names `d1`, `d2`, ... repeat the root's; the file holds only its own) | 1,840 | left out when no decoder parameter pairs with them (they had inflated "missing" and, paired by name order, 431 wrong expressions, values, units and roles) |
| Parameter sections the reference failed to dump | 43 designs | left out |
| Driven dimensions: the reference gives the measurement of the sketch as it is now, the file the value of the last solve | 187 | values and expressions not compared |
| Creator kind written differently (a sketch's text or pattern distance is `feature` of type `Sketch`, a plane's offset `feature` of type `ConstructionPlane`) | 221 | normalised |
| Item types named by class (`ComponentInsert`, `GeometricRelationship`, ...) and occurrence names (the component's name with a counter, or `Component Insert <name>`) | 102; 368 | compared by family and by base name |
| Object handles the reference's interface could not convert (participants, guide surfaces, offset faces) | 147 | not compared |
| A sweep's or pipe's path entities carry their spline, type and parent path; a loft's end conditions point back at their section; a move's definition repeats the item | 44 | the entities and the section's own fields are compared |

Decoder errors found and read from the file (all against every reference
dump; the learning dump's accepted answers do not carry these fields):

| Cause | Bytes and rule | Before | After |
| --- | --- | ---: | ---: |
| Sketch names given by the user | the sketch object holds `ref anchor`, `str16 name`, `u8 0`, `u32 n`, `ref[n]` in that order; the item's tail never holds the name a user gave a sketch, and a custom name in a tail is one the sketch no longer has (an empty name in the sketch means the default `Sketch<n>`); some sketch items refer to their sketch where the sketch has no reference back | 43 sketch names, 34 renamed | 2 (the default number of a sketch in a copied component) |
| Unnamed components (`Component<n>`) | a component's name is the text after its last identifier, empty while it has the default name; the text before it is its material, which was read as the name; the default number is the one in its occurrence item's tail | 5 garbled names | 0 |
| Count parameters of patterns and sketch texts without an owner | the count's holder (`A085449A`) and the text's holder (`E5460B7B`) have the layout of a parameter holder, the owner is its root reference | creator kind 265 missing, owner 386 | creator kind 10,260 of 10,260, owner 10,134 (125 differ by the item names below, 1 missing) |
| Features of a component's own list (outside the timeline) as creators | named by their item tail, sketches by their sketch object | owner names missing for about 110 | 0 |
| Favorites | the third byte from the end of the flags after the expression | 10,509 missing | 10,475 of 10,475 (6 favorites) |
| Dependents | the reference lists every parameter that depends on one, through the others (`d3 = d2 / 2`, `d2 = width + 1 mm` give `width` the dependents `d2`, `d3`) | 4 differ | 307 of 307 |
| Text parameters | a text keeps a default `1 mm` where the other parameters keep nothing: not a comment, and no number | comment 2, value 2 extra | 0 |
| Sketches whose item refers to the sketch only | `sketch.dimensions`: 22,639 of 22,639 compared values agree (before 22,949 of 22,988, 5 missing, 34 differ; the 187 driven dimensions' values are left out, they were 31 of the differences), and the sketches' circles and points followed | 8 | 0 |

The import creates the file's parameters with their names, expressions,
units, comments and favorites (`params.rs`); the count parameters now name
their pattern, and the star of a favorite is kept.

After: parameters 82,964 of 83,107 values agree (125 differ, 18 missing,
174 only in the decoder). The 125 differences are the owner names of joints,
constraint sets and rigid groups; the 174 are parameters the reference does
not list (thread offsets and lengths 90, the counts and stitch tolerance of
mirrors 30, the joints' offsets and angles of one assembly 32, a sweep's
distances and angles 13, others). Timeline: names 5,140 of 5,184 agree, types 5,184.

Still open: the default names of joints, as-built joints, constraint sets
and rigid groups (30 items): `Rigid`/`Revolute`/... from the joint's motion,
and one number shared by joints, constraint sets and rigid groups in
creation order, a single constraint numbered as the constraints so far
counting the members of sets; designs whose earlier items live in other
files cannot be numbered from the file. Canvas names (2), the names of
fasteners and pasted occurrences, which are their component's name in a file
the design links to (4), the health state and warning text (not stored in
the item: computed when the application recomputes; 5,161 and 64), the
names of the bodies a move, split, removal or copy takes (32 inputs; the
body names the user gave are in the file but not read, the default
`Body<n>` ones are numbered by the application), a sweep's guide rail (the
decoder reads one in 3 of 4 sweeps, the reference gives none), the constants
of lofts, sweeps and pipes (closed, solid, edge alignments, orientation,
profile scaling: 90 values, the same in every design, no variant model to
read them from) and the one draft.
##### Fillet and chamfer edges against the reference (mitcad#186)

The decoder's and the accepted answers' edge sets, compared with the API
reference dumps (1,158 fillet and 193 chamfer sets):

| Field (fillets) | Before | After |
| --- | ---: | ---: |
| Tangent chain, continuity (agree / not decoded) | 1,081 / 77 | 1,158 / 0 |
| Corner flag (agree / not decoded) | 1,057 / 101 | 1,158 / 0 |
| Edges (agree / differ / the reference lists none) | 1,026 / 86 / 46 | unchanged |

*Options of older records.* The 77 sets without options are in items of
class version 2 (and a few of 3). The set's options follow its root part: version
10 of the set class `u8 asymmetric | u8 0 | u8 constant | u8 flipped | u8
curvature | u8 tangent chain | u8 0 | u8 0`, version 9 `u8 asymmetric | u8
constant | u8 flipped | u8 tangent chain | u8 0 | u8 0` and version 4 the
same without the last byte, then the set input; five sets have a block of 20
zero bytes in between. The tangent chain byte agrees on all 1,158 sets (24
are off; the 12 off among the older records too); the fifth byte of the older
layouts sits where version 10 keeps the curvature flag and is 0 on all, but
no reference set has a curvature continuity, so a set with it is not read. The
corner flag (`u32 0 | u8`) was dropped by a guard against a following
health reference: the flag, a zero and a one read as a reference to the
object with id 256 in every design that has such a health object, which
removed 27 sets; the flag is read there now (the variant models'
rolling-ball and setback corners differ in this byte alone). With the
options the history-accepted answers' tangent chain agrees on all 196 settled
sets of 32 designs (183 before; the unset option was taken as on).

*The edges the replay lacks.* Of the accepted answers' edge lists that
differ from the reference (items, old learning dump, comparison folded as
above): 35 have a decoded edge, which agrees with the reference, that
`refs::dressup_edge_match` did not find in the replay, so no decoded
candidate was tried; 25 have a decoded selection that is itself another
edge than the reference's (below); 8 had the decoded candidate tried first
and the history contradicts it (the kernel fails, or the file's feature
rounds a neighbouring edge too, a tangent chain beyond what the kernel
follows), so a guess was accepted; the rest are partly unresolved decoder
edges. The decoded candidate was first wherever it existed (*Named inputs*:
the ranking only reorders candidates that predict a volume change), so
no ordering changed. The matcher changed instead: an edge the replay splits
into any number of pieces, a curve found by its ends and a length up to 2 %
off, and a middle within 0.01 mm with a length up to 2 % off (*Named
inputs*). On 32 designs chosen from the mismatch list, imported one by
one, the accepted answers' edge lists agree with the reference in 175 of 207
sets (158 before, the same learning dump comparison). One item gets worse: a
later fillet whose edge the replay cannot name now builds on the file's nine
edges of its predecessor instead of the seven the history accepted, and
falls back.

*What stays open (decoder).* 86 fillet edge lists differ from the reference.
Edges the decoder lists without geometry: 21 (the names fit two, three or
eight edges equally where the recipe's tail cannot be compared with the
state's faces, one end of the edge having been renamed since; or no edge
between the faces named), 11 in items whose references the application
reports lost, 11 in items rolled back after the timeline marker, which the
file keeps no state for. Other edges: 43, of which 27 are equally long
edges elsewhere on a symmetric or patterned part (twin copies, closed
edges, copies of a mirrored component), and a tangent chain the reference
lists in full where the file names one edge. The tail picked the right edge
on 6 of 6 edges it decided outside one item. Chamfers: 5 of 193 sets differ;
the chamfers' tangent-chain and flipped options are not decoded (the
reference has no chain off and no flipped set, so no byte can be told
from it).

*Comparison.* `apiref.py` accepts a closed circle by its circle, folds the
definitions of an answer (one per tangent chain or body) into one input,
takes pieces of an edge for the edge, the edges on a selected face for the
face, a spline's middle within a tenth of its length, and counts a set the
reference lists no edge for apart.

##### Fillets and chamfers that fall back for reading reasons

Fillets and chamfers that fell back although their edges were decoded were
read item by item against the API reference dumps (edge count, positions
and lengths, sizes) and the import's trace. Where the decoded set agrees
with the reference (edges, sizes, the state before the item) and the item
still falls back, the geometry kernel is the cause (below). Three readings
were wrong or missing:

- *The body of an edge.* Bodies that touch have edges in the same place,
  and the matcher kept the first body of the coincident copies. An edge's
  input refers to its body's record, so the decoder gives it
  `_f3d.producer` and `_f3d.body_index` like a body input, and the import
  matches the edge on the bodies of that item first
  (`Importer::match_dressup_edge`). A fillet of the edge of a thin wedge
  that sits in a slot of another body was rounded on the slot's body (the
  wrong body: volume and faces never matched, and the items after it
  needed the file's bodies): it builds now, and the edge counts as read.
  On the 50 designs imported again (those that reach the import's time
  limit left out), 17 fillet and chamfer items that were `inferred` (the
  first-body choice) are `read` or `unknown` now, one fillet more is
  parametric, and no item got worse.
- *Closed ellipses.* A closed edge that is not a circle (a round hole cut
  at an angle) was found by nothing, a split replay copy of it least. Its
  axes follow from the file's start (one end of an axis), its middle
  (the other end), its direction at the middle and its length (the
  perimeter fixes the second axis); the replay's edge, or its pieces, are
  found on that ellipse (`FileCurve::Ellipse`). The edge of one design (an
  ellipse the replay splits in two at the ends of its axis) is found by its
  name now.
- *Elliptical arcs and splines the replay splits.* A circle through the
  three points the file gives can have the arc's length without the arc's
  pieces lying on it (an ellipse near its minor axis), so the pieces were
  never taken for the edge. An open edge that is not a line is found, when
  nothing else finds it, by pieces of one body that follow each other by
  their ends from its start to its end, whose lengths add up to its and
  that pass within 2 % of its length of the file's middle
  (`refs::chains_by_ends`); two such chains are ambiguous.

What the kernel does, seen on the items whose decoded sets agree with the
reference (counts are items):

- a rounding or bevel as wide as the face next to it takes the face away
  exactly. A chamfer of the width of the annulus beside it is built 0.1 %
  smaller, which moves the edges the next items name by a few micrometres,
  so they are not found and the items after it fall back (1 item). At the
  exact width the bevel of another item came out as a solid that lost 14 %
  of the volume instead of gaining 2 %: valid for the checks, so not built
  again smaller (1 item). A fillet wider than the face beside it builds a
  shape with four faces more than the file's (1 item);
- a bevel or fillet ends at a vertex where a neighbouring bevel meets it,
  or along the junction of two blend faces that become tangent to a plane
  at its end: the volume differs by 0.1 to 5 % of the body, with the same
  faces in two of the 3 items;
- fillets on both sides of a slot narrower than twice the radius (1 item),
  and a rounding of 0.1 mm that fails where 0.05 mm builds (1 item).

##### Chamfer options, moves, sweep rails and face profiles (round 2, mitcad#186)

Chamfers, moves, sweeps and extrusions of faces were read item by item
against the API reference dumps and the controlled models.

- *Chamfer sets.* The byte right after the root part of a chamfer's set
  record (class version 2) is the set's tangent-chain option. It is 1 on the
  309 sets of the API reference dumps (all `isTangentChain` true) and 0 on
  the 15 sets of the controlled models made by script (all false), so the
  decoder gives `isTangentChain` per set (and per feature where the sets
  agree) and the decoded value agrees on 324 of 324 sets. The record has no
  byte for `isFlipped` of a two-distance chamfer (every dump set has it off,
  whatever the bytes), so the sides keep coming from the edges' directions
  in the file (*Named inputs*), the other reading staying a guess. An equal
  or two-distance chamfer whose corner type, sizes, tangent-chain option and
  edges are all decoded fields is `read` when its decoded definition is
  the one the history accepts (`read_chamfer_inputs`, as for fillets); a
  distance-and-angle chamfer's angle side is not decoded, so it stays
  `unknown`. On the 11 designs imported again that have chamfers, 19 items
  went from `unknown` to `read` and no outcome changed.
- *Moves.* A move's transform object that stores no matrix (its `u32 5`
  followed by `u8 1 | u16 0 | u8 1 | u32 0 | ref` where the 128 bytes of a
  matrix go) is the identity: 2 of the 27 moves of the API reference
  dumps, both identity there; the other 25 store their matrix and all 27
  agree with the dumps. A move whose bodies and matrix are decoded is
  `read`.
- *Sweep rails.* The rail slot of a sweep (class version 6) holds curves of
  the profile's own sketch in 3 of the 4 sweeps of the API reference dumps
  (the fourth has none), where the dumps list no rail; the one controlled
  model with a rail has it in the path's sketch. Curves of the profile's own
  sketch are not taken for a rail now, so the 4 sweeps agree (3 differed).
  The two designs whose sweep fell back because the rail was in its way (the
  volume it made was a third of the file's) are parametric now.
- *Two-sided extrusions with a negative distance.* A cut through all one
  way with a negative second distance (one case) removes what a cut through
  all one way plus the same cut the other way by that distance removes,
  which is the file's state exactly, so the sign of a two-sided distance
  does not turn its side; before, the model refused it.
- *Extrusions of a face up to an object.* A profile that is only a face of
  a body (no sketch region) was made into a sketch on the face and
  extruded by a distance only; an extent up to a decoded face or plane is
  made the same way now, the side chosen by the history.

- *Tangent circles.* A region whose hole is a circle touching its outer
  boundary at a point (two circles that meet at one point)
  failed to build as a face (`intersecting
  wires`), and the prisms of such regions did not fuse with the regions beside
  them (no body at all). The hole's wire shares the outer boundary's vertex
  now and touching regions are united as faces before they are extruded; the
  design's three regions come out as the file's body (volume, area and faces
  equal), and the design is parametric.
- *Combines, shells and pipe threads.* A combine whose bodies, operation and
  keep flag are decoded, and a shell whose body or faces, thickness, type and
  tangent-chain flag are, are `read` (the dumps agree on all 33 combines'
  operation and keep flag and on all 7 shells' type and chain). A pipe
  thread's class `A` is on the internal threads of the stored designs (6 of
  6), which Mitcad's table listed for the external ones only, so the class was
  left out with a note and the thread stayed `unknown`; the table lists it for
  both. On one design with 94 items, 7 items (a shell and six threads) went from `unknown` to `read`.

What the file does not give in these items (counts): the profile records of
a sweep or revolution whose sketch curves were edited after the feature
(their ids name curves that are gone: `unknown`), and profile curves that
are splines, which the region matching does not follow (`unknown`).

##### Fillets and chamfers that fall back: a second pass (mitcad#186)

98 fillet and chamfer items of 26 designs (some are versions of one another)
fell back with `its result is not in the file's history`, a volume that is
off, `edge … no longer exists after …` or an invalid shape. Each was read
against the API reference dumps (the sets' edge counts, lengths, middles and
sizes). 9 of the items have no row there (8 are of a design the reference
application could not walk); of the other 89 the decoded input agrees in 83
(82 before the change below): 4 are references the file lost (below), one
set lists the same edge twice (the same set of edges) and one input names two
edges equally (the history's guesses decide). So the kernel is the cause of
the rest, as before. Two readings were new:

- *Inputs that fit the same edges.* The recipes of the edges of one set that
  lie between the same two faces can differ in the faces at their ends only
  (the 8 edges between a plane and a free-form face), so each fits all of
  them and none was found. Where the inputs of a set that fit the same edges
  are as many as those edges, each takes one (`core/f3d/EDGE_ORDER.md`,
  *Inputs that fit the same edges*). Of the 92 inputs of the 358 reference
  designs the names fit several edges for, 36 are found now; 3 edge sets that
  differed from the reference agree, no set that agreed changed. One of the
  98 items is such a set (8 edges, a free-form patch): its decoded definition
  is the first candidate now (`read`), and it fails in the kernel (an invalid
  shape, 16 s a try). A fillet of another design that was `inferred` from the
  history's next state is `read` and verified now.
- *Edge references the file lost.* The health of an item is not in the file,
  but its inputs show it: a name whose faces are not in the history state the
  item was made from, or between whose faces no edge lies. Of the 25 fillets
  and chamfers of 16 reference designs with such an input, 23 are items the
  reference reports as having lost edge references, using the geometry they
  had cached (7 errors, none of them lists an edge; 16 warnings: the edges
  the reference lists are the edges found in 8 of the 16, in the others the
  decoder finds more edges or fewer than it lists); the other 2 are one item
  in two versions of a design. Such an item gets its result from the file,
  not from its inputs, so no definition of its inputs can rebuild it: its
  note says `N of its M edge references are lost in the file …` before the
  last kernel failure (4 of the 98 items).

What the kernel does with the rest: the decoded definition was built on the
replay and on the stored state before the item; then the size scaled by 0.5
and by 0.1 in a replay of the decoded design (tells a size too large for the
part from a case the kernel cannot do at any size); `isTangentChain` off
(tells whether the kernel's chain is longer than the selection); the
narrowest face beside the edges, the reference's face areas divided by the
edge lengths, against the radius. The classes of the 98 items (the same case
in versions of a design counts each):

| Class | Items |
| --- | ---: |
| the file's edge references are lost (above) | 4 |
| the kernel builds a result that is not the file's | 16 |
| fails at the full size, builds at a smaller one, a face beside the edges narrower than two radii | 20 |
| the same, no narrow face (or no reference row) | 16 |
| fails at every size scaled: the rounding cannot end at a vertex 17, the radius may be too large 15, an invalid shape 8 | 40 |
| the time limit (the kernel's tries of 16 to 100 s) | 1 |
| not scaled (no reference row) | 1 |

Of the 16 that build something else: in one the face beside the edge is as
wide as the rounding (a closed edge, a ring face 2.48 mm wide rounded by
2.5 mm: the result has the volume it had before and 15 faces fewer than the
file's); in two the kernel's contour follows a tangent continuation the
file's rounding did not take (the file's next state lost exactly the
selected edges; the kernel cannot round the selection alone; done in
mitcad#241, below); in three the
volume is within 1.5 % of the file's with other new faces (blend surfaces
and corners); in eight it is 4 to 77 % off; in two (chamfers, with the file's
number of faces) the volume removed is too large.
These go to the kernel work (mitcad#121); none is decoding.

*The longer tangent chain (mitcad#241).* Both items (two designs of one
project: a ring-shaped part, and a post in an assembly) round
edges whose next edges turn from them (0.74 degrees along the top of a
ring where a corner cylinder of radius 1 meets it; 4.2 degrees up the
corners of a post whose walls are drafted by 3 degrees above a vertical
part), while the faces across the vertex go on tangentially within OCCT's
0.1 rad, so OCCT's contour ran on over the turned edges. The file's chain
stops there and the face beyond the crease, extended, cuts the rounding.
With the OCCT port's patch 0180 Mitcad's fillets stop their contours at
such turns unless the next edge is rounded too, the face beyond cuts the
rounding where the crease is concave (both items), and the vertex left
between two pieces of that face's edge is removed: both items are
parametric and verified against the file's next state, the ring's
rounding a torus with circles for its contacts as in the file, and the
final bodies as before (the ring's within 5e-9 of the file's volume). In
the assembly a later fillet of the post (a slot's edges up the vertical
wall, turning at the same crease) became parametric too. A chamfer after
it that the file shows no change for, whose edges now resolve on that
replayed rounding and fail, falls back where it was skipped (its stored
body is the same state).

#### Sketch texts (mitcad#186)

The decoder reads a sketch text (class `E0618268`) into the dump's `texts`:
the string, the font family, the height, the weight (bold from 600), the
alignments, and for a text in a box the box's four lines and, in some files,
the placement matrix (the box corner and the baseline's direction: `position`
and `angle`; the API gives neither). The lines come from the text's own
`TextBoxConstraint` (its curves in order, a corner point among them left
out); the curve of a text on a path from its `TextPathConstraint`. The corner
points a box makes (class `00D157C2`) keep their coordinates in the sub-chunk:
before, the box's points had none. Checked against the API reference dumps:
the string, height, font, style, alignments and kind (box or path) of all 10
texts of the 6 dumps with texts (3 designs, 9 in a box and 1 on a path) agree,
and so do the reference model's; 11 more texts of a design without a dump
decode (all in a box).

The file's texts are measured as follows (derived from the API dumps'
bounding boxes of three texts: the reference model's, a rotated one and a
wrapped one, to within a font's glyph shapes; `cap_height` and `wrap` of the
sketch text):

- the height is the height of the capital letters, not the font's em;
- a line's outline, not its pen advance, is aligned: left to the box's left
  side (without the first letter's side bearing), right, or centred;
- a text in a box aligns vertically by its capitals: the top of the first
  line's capitals at the box's top, the last baseline at the bottom;
- lines are one ascender plus descender apart (no line gap);
- a line breaks at its spaces to fit the box's width (a word wider than the
  box stays whole and overflows).

Without the box, the first line's direction gives the text's angle.

The font: the glyph outlines come from the installed font of that family
(OCCT's font manager); a family that is not installed gives the bundled
Droid Sans and the sketch a warning ("the font Arial is not installed;
Droid Sans is used"). The glyphs then differ from the file's, so a text
extrusion's volume differs from the history's and the extrusion stays a
fallback on a machine without the font (nothing is substituted by shape).
A text on a path is placed from its start; the file's per-glyph matrices
(where on a closed path the text starts and which way it runs) are not
decoded yet.

#### What the file does not settle yet (mitcad#104)

Counted over both private corpora with the learning dump and per-file
traces of the import (the backup corpus repeats some designs of
`MITCAD_F3D_CORPUS`, so these are instances, not distinct items):

- **Flips no byte decides** (*What the history settles*, *Direction*):
  one-sided cuts through all with the flag byte 0, a few distance and
  two-sided extrusions. The history settles them with the next
  definition; files without a history get the first.
- **Patterns and mirrors of features with fillets or chamfers among
  their objects** (the case of a pattern's join whose volume differed
  from its state). A rectangular pattern of a boss, a cut and a chamfer
  between the boss and the plate gave 3979 mm³ less than its state: the
  file's six copies carry the chamfer (755 mm³ each, the five copies
  added come to about that), Mitcad's copies of the features do not (a
  chamfer leaves no tool body to copy; the decoded objects leave
  dressups out). The order of the copies does not matter there (applying
  each element's copies in the features' order gave the same volume). 36
  unsettled patterns and mirrors have such objects (35 settled ones too,
  where the dressups made no difference to the state). Done in
  mitcad#105: patterns and mirrors repeat fillets and chamfers among their
  features on the copies' edges (`commands.md`, *Fillets and chamfers
  among the features*), and the import tries them (*What the history
  settles*). Over both private corpora the patterns and mirrors with such
  objects came in parametric 30 → 39 of 72 (the rest: the copies'
  roundings OCCT does not build, mostly where copies overlap or meet the
  original's rounding, mitcad#107; copies that reach no body after an
  earlier fallback; the time limit), the final bodies as before, the
  import's time unchanged.
- **Fillets and chamfers whose by-name definition fails in the kernel**:
  273 fall back with every edge found by its name. 218 of them round
  edges of stored faces (bodies a base feature brought in, the faces
  named `import`: an earlier fallback, or the file's own base
  features), whose tolerances (up to 0.05 mm) and approximations OCCT's
  dressups often do not build on; 25 round replayed faces after an
  earlier fallback; 29 bodies replayed throughout (radius or distance too
  large 10, rounding cannot end at a vertex 6, invalid shape 3, others
  not the state or names gone). In the samples of the last class the
  rounding is wider than a face next to it (a 90 mm edge rounded wider
  than the 4 mm face beside it; a rounding along a chamfer wider than
  the chamfer), so it runs over or removes that face, which OCCT's
  rolling-ball fillet does not do. These are geometry kernel limits, not
  decoding: fewer fallbacks before them is the cure for the first class.

  Classified in mitcad#107 on the 249 such dressups of both private
  corpora (192 distinct inputs), each built again on its stored input
  body by OCCT's rounding alone (the 1e-3 smaller retry and the rings
  off) and compared with the file's next state (volume within 1e-5 and
  the same face count):

  | Class | Dressups (distinct) | Built by the real rounding |
  |---|---|---|
  | OCCT's result rejected only for faults of the input away from the rounding | 12 (12) | yes: taken as it is |
  | a whole circle stored in two arcs | 3 (3) | yes: the pieces merged first |
  | knife edges (the faces' normals opposite) | 3 (2) | yes: tighter tolerances of OCCT's fillet |
  | built, but not the file's body (faces split otherwise; an input with 11 mm tolerances) | 3 (2) | built, not matching |
  | rounding wider than a face next to it: the face gone in the file's body | 54 (43) | no |
  | the same, the face partly gone | 59 (40) | no |
  | the same, the face kept or not known (state unreadable) | 44 (37) | no |
  | two roundings on the two sides of a face narrower than both | 16 (11) | no |
  | rounding cannot end at a vertex (7: the face it ends on runs on tangentially into a face of the other convexity) | 18 (13) | no |
  | OCCT's result invalid and not the file's body | 11 (10) | no |
  | no start for the rolling ball, or the walk fails, on wide faces | 8 (4) | no |
  | a bevel exactly as wide as its face, contours whose corners fail | 7 (5) | no |
  | OCCT finds no edge to round (faces 1° from tangent), OCCT exceptions | 4 (4) | no |
  | OCCT crashes | 7 (6) | no |

  Where the rounding is wider than a neighbouring face, the file's
  rounding runs over that face or rides its far edge, while OCCT's
  rolling ball stays on the two faces of the edge: it finds no start, the
  walk fails, or it cannot end. Deleting the narrow face first
  (`BRepAlgoAPI_Defeaturing`, its neighbours extended; one attempt, the
  faces next to the edges that the rounding is wider than all along) and
  rounding the edge that takes its place gives the file's body in none
  of them: it applies to 91 of the 249, the deletion fails in 49, the
  rounding in 25, 10 leave no edge where the face was, and the 7 it
  builds all take more material than the
  file's rounding (up to twice: the ball touches the extended neighbour
  instead of riding the narrow face's edge); it costs 6 s a case on
  average, up to 127 s on large bodies. It is not done. OCCT's other
  options (polynomial or quasi-angular sections, its continuity and
  tolerance parameters other than the knife edges', one contour after
  another, healing or merging the input's faces first) build none of
  the rest.

  Coverage with the 1e-3 smaller retry and the rings off in both builds
  (the dressups OCCT's rounding builds), before and after, both corpora:
  fillets parametric 237 → 238 and 967 → 991, chamfers 39 → 39 and
  162 → 164, all items 1049 → 1050 and 4677 → 4702; final bodies as
  before except one body of a design stored twice, exact before and now
  4e-6 from the file's (three of its fillets on a stored body with faulty
  faces come in parametric, matching their states within 1e-5).

  The OCCT port's edge overflow (mitcad#121, patches 0010 to 0012 of
  [the port](../../third_party/vcpkg-ports/README.md)) lets the rolling
  ball run onto the face beyond a narrow face or roll on its far edge
  where the narrow face's contact leaves it along the whole edge (OCCT
  found no start there), and drops the narrow face it consumes. Of the
  249, measured the same way: the face gone in the file's body 6 now the
  file's body (MATCH) and 11 within 1e-3 (NEAR: corner patches or
  approximated blend surfaces other than the file's), 1 off; the face
  kept 2 NEAR; the face partly gone none (OCCT's walks there reach the
  far edge and roll on it as before, then fail at the corners or build
  invalid shapes; where the contact leaves a face near the end of the
  spine OCCT cuts the rounding with the face beyond, as before, which the
  file's body agreed with in the samples). The rest fail in OCCT's
  corners at vertices, in walks that switch to a tangent neighbour on the
  wrong branch of its surface, on narrow faces on both sides of the edge,
  or are chamfers (OCCT has no chamfer rolling on an edge). The other
  classes build as before: where two roundings overlap on a face the
  file's one surface replaces both and the band between them, which
  would merge two stripes (OCCT computes both and fails at the corner
  between them); a chamfer exactly as wide as a face gets no start; and
  where roundings of one fillet consume each other's faces, OCCT stops
  because the two stripes' surfaces intersect.
  Coverage with OCCT's and the patched fillet, both corpora: fillets
  parametric 251 → 253 and 1088 → 1101, all items 1070 → 1072 and
  4874 → 4886, circular patterns 89 → 88 in the backup corpus; final
  bodies exact 525 → 523 and 3169 → 3162, the others within 0.1 %, none
  off. The fillets the patch builds mostly failed before by building
  a wrong body, so the replay took the file's state before them and
  went on exactly; now they come in parametric on the chain's earlier
  approximation (an approximate chamfer before them: up to 2e-4), so
  the final bodies are as far from the file's as that chain, and one
  pattern after such a chain no longer comes within the tolerance. One
  rounding next to a 0.0015 mm sliver now builds onto the face beyond
  it, removing 6 % more or less than the file's (its body 7e-6 off),
  which the import takes; and one fillet after three that now come in
  fails on their result.

  Second round (mitcad#121, patch 0013): the overflow runs over several
  narrow faces in a row (a strip and a sliver beyond it) and on both sides
  of the edge, and two roundings whose contacts cross on a face between
  their edges are one rounding. Of the 249, measured the same way: the
  face gone 6 MATCH / 11 NEAR → 9 / 8 (one family of three now the file's
  body: the ball reaches the face beyond a 0.0005 mm sliver instead of
  rolling on the sliver's edge), the face kept 2 → 3 NEAR, two roundings
  overlapping on a face 0 → 3 NEAR (the volume within 1e-7; the file
  merges two coplanar faces the rounding leaves next to each other). Of
  the NEARs, 9 change the volume as the file's within 1 %; 9 do not (2 to
  56 % off) and are not taken by the import's change comparison. With
  the patches 0020 to 0023 after them, the face partly gone builds 3 NEAR
  too. The rest still fail: corners where a rounding that runs onto faces
  beyond ends (`PerformIntersectionAtEnd`, `PerformOneCorner`), walks that
  fail rolling on an edge, roundings partly wider than the narrow face
  near the spine's ends, chamfers exactly as wide as their face (the
  contact lies on the face's boundary, which the hatching of the analytic
  chamfer leaves out), the second rounding riding on the first one's torus
  and earlier roundings the file's rounding replaces.
  Coverage before (main, with OCCT's overflow of 0010 to 0012 and without
  the change comparison) and after, both corpora at the same time: items
  parametric 1061 → 1034 and 4912 → 4796, fillets 252 → 231 and
  1101 → 1035; final bodies exact 523 → 527 and 3162 → 3180, within 0.1 %
  12 → 8 and 72 → 54, off as before (0 and 1). Nearly all the items that
  no longer come in parametric are results the change comparison rejects
  (above; the smallest 1.0 to 1.5 % off with other faces than the file's)
  or items after them that the replay no longer reaches the same way; the
  rest: the time limit in three large designs (as in other runs), the
  fillet after the sliver rounding above, now the file's, whose edges
  then resolve to other edges (the design is in both corpora), and one
  rounding next to earlier roundings stored as splines that the file
  rounds again (built within 1.7e-4 before, now off). The patch alone
  (the same import before and after): 1 fillet lost and 1 gained in the
  main corpus, 3 lost and 4 gained in the backup corpus.

  Third round (mitcad#121, patches 0014 to 0016): an analytic rounding or
  chamfer exactly as wide as its face (its contact line on the face's far
  edge) is split on that face and stored as a blend on the edge, so that
  the face goes (0014); a rounding rolling on an edge halves its step
  where a section cannot be reframed (0015); and of two roundings of the
  same kind overlapping on a face narrower than both, which 0013 cannot
  make one, the later rolls on the earlier one's contact line and the two
  meet there (0016). Of the 249, measured the same way: two roundings
  overlapping on a face 0 MATCH / 4 NEAR → 6 / 4; the other classes as
  before. The chamfers exactly as wide as their face now get their exact
  surface but fail in the corners at their ends; walked (non-analytic)
  blends along a face's boundary, corners after an overflow onto
  different faces beyond, rolling that pivots on a point or turns at a
  vertex and the ball leaving the edge it rolls on at the start still
  fail. Coverage before (main) and after, both corpora at the same time:
  items parametric 1059 → 1060 and 4905 → 4907, fillets 244 → 245 and
  1092 → 1094, chamfers 56 and 236 as before; final bodies exact
  527 → 529 and 3176 → 3182, within 0.1 % 8 → 6 and 58 → 52, off as
  before (0 and 1). Lost: a two-distance chamfer exactly as wide as its
  face whose ends meet a cylinder (the exact result has a face of
  inconsistent orientation; before, the 0.1 % smaller retry built it),
  and a fillet whose exactly consumed face takes an edge the file keeps.
  The results rejected at 2 to 4 % with the file's faces were compared
  with the file's bodies: one is a different rounding (0.15 mm from the
  file's along its whole length, 2.1 % off), others differ only at the
  corner patches at the ends (2.4 %) or by spline approximations spread
  over the body (2.4 %): no face count or percentage separates them, so
  the tolerance stays 1 %.

  Fourth round (mitcad#121, patches 0017 to 0019): the ends of a contact
  along a face's far edge are put at the edge's vertices also where the
  other face's contact ends first (0017: the two-distance chamfer above,
  whose exact result now is valid and the file's); a fillet or chamfer
  ending where the face at the end is split in two faces of one plane
  builds its corner (0018); and a corner that OCCT's
  `PerformMoreSurfdata` leaves undone fails cleanly instead of leaving
  point 0 in the data structure (0019, before an exception from the
  topological build or, without range checks, a write out of bounds).
  The dressups' naming gives a face that a rounding or chamfer consumed,
  where the blend runs along the face's far edge, to the blend's face, so
  that the edges the consumed face had keep their names: the fillet above
  whose later tangent chain named such an edge resolves again. Of the
  249, measured the same way: chamfers failing in their end corners
  0 → 3 NEAR (the file's volume, 1e-12; the file merges the split faces);
  three crashing specs now fail cleanly; the other classes as before.
  Still failing, with their causes: a rounding rolling on an edge stops
  where the ball pivots on a point of it (its contact on the edge stops
  moving) or at the edge's end, where it would turn onto the next edge,
  and the walk between the faces from there fails; corners after an
  overflow onto a face beyond that reaches past the face at the end (the
  rounding's end lies inside that face, not on an edge both share);
  starts where the ball leaves the edge it rolls on; and contacts leaving
  their face near the spine's end, where a walked piece with one side
  reduced to a point leaves the corner without an arc. Coverage before
  (main) and after, both corpora at the same time (three files at a time
  each): items parametric 1060 → 1061 and 4908 → 4914, fillets
  245 → 246 and 1095 → 1100, chamfers 56 → 56 and 236 → 237; final
  bodies the same (529 exact and 6 within 0.1 %; 3182 exact, 52 within
  0.1 %, 1 off). No item was lost.

  The other classes were traced inside OCCT's fillet (mitcad#122, patches
  0020 to 0023 of [the port](../../third_party/vcpkg-ports/README.md)).
  Measured the same way, on OCCT with the edge overflow, before and after:

  | Class (dressups) | Cause found | File's body before → after |
  |---|---|---|
  | OCCT crashes (7) | an empty restriction curve after a stripe met an obstacle face that does not hold its edge; a seam intersected with a parallel curve; the corner code reading another stripe's surface data | 0 → 2; none crashes now, 3 fail cleanly (no start past the obstacle), 2 build invalid shapes |
  | OCCT exceptions (2) | the corner of a contour closed by its sharp corner, at a vertex of more than three sharp edges, found only one of the contour's ends | 0 → 2 (3 more of the narrow-face classes now come within 1e-3) |
  | faces 1° to 6° from tangent (2) | an edge OCCT's loose tangency test (0.1 rad) calls tangent was refused although asked for | 0 → 1; the other fails in the walk at the ends of a circle between faces 1° apart |
  | OCCT's result invalid (11) | at a shallow corner of an arc and a line, the meeting point of the contact lines taken on the arc's extension | 0 → 1; the others: corner caps at vertices where a stripe ends on its face's boundary, contact lines reset to the far end of a periodic hatch at the spine's start (`SplitKPart`) |
  | rounding cannot end at a vertex (18) | the contact leaves the face near the end of the spine (a face narrower than the rounding there), and the corner (`PerformMoreThreeCorner`) takes an edge that does not touch the vertex for the continuation of another; correcting that gives invalid corners | 0 → 0 |
  | two roundings on the two sides of a narrow face (16), a bevel exactly as wide as its face (7), consumed earlier roundings (3) | the file's rounding consumes the face or the earlier roundings: edge overflow across two stripes, which OCCT does not merge, and chamfers, which the overflow does not cover | 0 → 0 |
  | no start or the walk fails on wide faces (8) | the walk stops where the contact meets a torus' seam or switches to a tangent neighbour | 0 → 0 |
  | built, not the file's body (3) | OCCT's blend split into slivers at a torus seam (the same volume, more faces) | 0 → 0 |

  None of the 249 crashes now (7 before). Coverage before and after the
  patches (both with the edge overflow), both corpora: fillets parametric
  253 → 256 and 1101 → 1112, chamfers 55 → 55 and 231 → 232, all items
  1072 → 1075 and 4881 → 4898; final bodies as before (523 and 3162
  exact). The backup numbers leave out one large design that reaches the
  coverage runner's 4 GB memory limit in both runs (before: the import's
  low-memory stop; after: OCCT's shape checker failing to allocate, a
  segmentation fault in `CSLib_Class2d`); without the limit it imports
  the same before and after.

  The classes left after mitcad#122 were traced further (mitcad#133, patches
  0024 to 0026 of [the port](../../third_party/vcpkg-ports/README.md)), measured
  the same way on OCCT with patches 0010 to 0023, before and after:

  | Class (dressups) | Cause found | File's body (within 1e-3) before → after |
  |---|---|---|
  | no start or the walk fails on wide faces (8) | a rounding along a whole circle whose contact line on a plane crosses six slots: SplitKPart's pieces were sorted with a stale piece, repeated and lost, and the walks between them threw (3, one design, fixed by 0024); a contact line crossing a notch in its plane (2) or a near-tangent spline edge whose approximation fails (2); a rounding wider than the earlier roundings on both sides of its edge (1, the edge overflow's class) | 0 (0) → 0 (3); the file keeps one torus across the slots and trims it with the slots' faces, OCCT runs the rounding along the slot walls (0.7 % of the rounding's volume) |
  | rounding cannot end at a vertex, the face continuing tangentially (7) | the rounding's face turns tangentially into a rounded wall at the end and the contact on it ends off any edge (the OnSame state), the end cut by the rounded wall and the faces beyond it: OCCT's intersection at end gave up on a contact off an edge (5, two designs, the face on either side of the rounding, fixed by 0026: the face's edge at the vertex is prolonged to the contact, the spine's edge cut there, the ends on the cylinder taken in its period); the rounded wall at the end is an earlier rounding that the file extends along the new one in a way OCCT's corners do not build (2) | 0 → 5 (the file's body) |
  | rounding cannot end at a vertex, other (11) | an end cut by a sliver, a cylinder's corner patch and the next sliver (2, one design, fixed by 0025: the intersection at end took the other sliver's crossing of their shared circle, did not prolong the slivers' edges to crossings beyond their ends nor cut the corner patch's edges, counted the cylinder's seam as an edge at one corner, and did not cut the end's intersection at the seam); two roundings meeting on a face narrower than both (5, the edge overflow's class; mitcad#121's patch 0013 does not build them yet); the file extending a cone at the end (2); end corners of a radius of 100 crossing on the face at the end (1); an edge of 0.0006 mm at the vertex (1) | 0 → 2 (the file's body) |
  | OCCT's result invalid (10) | the edge chain ends on both sides of a notch the file's rounding runs across (6, two designs: the file continues the torus across the notch and trims it with the notch's faces; SplitKPart also resets the contact point to the far end of the periodic hatch there; the chains end at faces 0.17 to 0.57 mm wide under roundings of 0.3 to 0.5 mm, so the rounding overflows them as in the narrow face class); side faces narrower than the rounding (2, the edge overflow's class); a face of 2e-5 mm² in the input (1); a chamfer, not traced (1) | 0 → 0 |
  | built, not the file's body (3) | a contact circle tangent to two edges of the plate (the hatch sees two crossings 0.008 mm long, which OCCT walks as slivers; merging them gives the file's faces but a plane face touching itself, which the file splits); an input with 11 mm tolerances (2) | 0 → 0 |
  | faces 1° from tangent (1), crashes that now fail cleanly (3) | the rolling ball's radius exceeds the convex curvature radius of a face (10 on a circle of 8, 1 past a corner of 0.5): the walk reports a twist and stops | 0 → 0 |
  | crashes that now build invalid shapes (2) | the end of a concave rounding where the chain turns convex tangentially (1); not traced (1) | 0 → 0 |

  Over both corpora (fallbacks on, with patches 0024 to 0026 against
  without) fillets came in parametric 4 and 15 times more and as
  fallbacks 4 and 21 times less; no final body is off. A few final bodies
  move from exact to within 0.1 %: a fillet that now builds runs on a body
  where an earlier fillet failed and was accepted within the history's
  tolerance, so the earlier fillet's missing rounding reaches the final
  body instead of being replaced by the stored state.

  Where the edge overflow stands at the close of mitcad#121 (the port's
  patches up to 0077, the later rounds' details in the [port's
  README](../../third_party/vcpkg-ports/README.md)), measured the same way:
  of the 249, 63 are the file's body (MATCH), 41 within 1e-3 (NEAR), 1 is
  off and 144 (117 distinct inputs) fail. By class (MATCH / NEAR of all):
  the narrow face gone 15 / 12 of 54, partly gone 6 / 13 of 59, kept
  2 / 6 of 39, not known 0 / 0 of 5; two roundings overlapping on a face
  6 / 4 of 16; the others (built before, crashes, wide faces) 34 / 6 of
  76. Where OCCT stops for the 144, traced in the fillet builder
  (distinct inputs in brackets):

  | Where it stops | Fail (distinct) | Cause |
  |---|---|---|
  | the corner at a vertex where a rounding ends | 48 (40) | the end face's intersection with the blend fails (`PerformOneCorner`, 8), no face at the end for the cap (5), the vertex's edge not found (8), the cap crosses another rounding's contact line on the face at the end (6), the intersection at the end through several faces (7), an end on a tangent continuation with no point on the arc (3), others (11) |
  | an invalid result | 27 (22) | the topological build gives self-intersecting wires (15) or unorientable faces (12): pieces that end on face boundaries, corner patches |
  | rolling on an edge after an overflow start | 15 (13) | the blend rolling on the far edge stops where the ball pivots on the edge or turns at its end; its sections are OCCT's plane sections, not the rolling ball of 0075 |
  | the walk between two faces | 16 (12) | after a contact reaches its face's edge, or from a start on it, the first step leaves the next face (11 distinct; traced: a wall's concave rounding of a smaller radius than the ball, which the ball cannot touch and should bridge); one walk whose steps stay too large |
  | the computation of a rounding | 14 (11) | exceptions in the stripe's computation, mostly without a message; two of a fillet's roundings whose surfaces intersect (they consume each other's faces) |
  | two roundings whose contact lines cross on a face | 8 (5) | the contacts of two roundings of one fillet cross on a face they share, not all along it as 0013 and 0016 handle (`ChFi3d_StripeEdgeInter`) |
  | no start | 8 (8) | among them a wall's top rounded on both sides with one radius, the top 0.02 mm narrower than two radii: no ball touches both the bore and the other side's rounding (two equal radii) |
  | chamfers wider than a face | 7 (5) | the overflow is for fillets only: no chamfer runs over a face or along an edge |
  | no edge to round | 1 (1) | faces 1 degree from tangent |

  No single cause covers more than a few designs: each class above is a
  new construction in OCCT's fillet (corner patches after an overflow,
  rolling-ball sections on an edge, a blend bridging a face the ball
  cannot touch, chamfer overflow), which the follow-up tickets of
  mitcad#121 take one by one.

  The 48 that stop at the corner where a rounding ends were traced one by
  one in mitcad#236 (patches 0130, 0210, 0211, 0280 to 0283, 0290 and 0291
  of [the port](../../third_party/vcpkg-ports/README.md)). Grouped by where the
  corner code stops, with the file's body reached (within 1e-5, the same
  faces) / specs:

  | Where the corner stops | File's body / specs | Cause |
  |---|---|---|
  | the end face's intersection with the blend (`PerformOneCorner`) | 1 / 5 | a rounding whose faces come tangent at its end vertex stopped a little short of it and could not be capped (1, fixed by 0130: the end is moved to the vertex and the rounding ends in a point there, as in the file); the wall's top rounded where the wall turns tangentially into a concave corner of the rounding's radius, the floor's contact running into that corner's foot before the end (3, one part in three files); the blend's face turning tangentially into an earlier rounding at the end (1) |
  | the vertex's edge not found in the walk round the vertex | 2 / 7 (fourth round) | the foot of a rib against a bore wall rounded up to the ceiling, where a groove runs along the ceiling's junction with the wall and the rib's top under it is a sliver of 0.05 mm² beside the ceiling in its plane, the two meeting only at the point where the groove's side crosses the rib's side (1): the walk round the vertex now passes the faces between the two there, the groove's side crossed at a point (0290); the file's body (1.1e-7, the same 12 faces). A top face tapering to the vertex between a wall and a sloped face, the wall running on past the vertex into a face tangent to it (1): the top's contact runs off the top before the end, and the piece walked from there past the spine's end on the top's plane had that side given up and was dropped at the corner; it now continues the torus before it, cut by the sloped face and the face beyond the wall (0291); the file's body (the boolean differences with its state empty both ways, 39 faces). Left: two roundings of 5 on both edges of a top 3.5 wide, each rolling on the other's edge (one part in two files, 2): the corner's walk follows the face the blend rolls on, which does not hold the corner's vertex, and loops up to its face limit; the file's two tori meet each other and are both cut by the slot's wall, a corner of two blends rolling on each other's edges that OCCT does not build. A rounding of 20 whose narrow end face it consumes, beside a boss whose foot passes through the end of the contact's edge (1): its end section lies under the boss for 16 mm; with 0290 OCCT cuts the blend with the boss's wall, which gives an invalid body removing 453 mm³ more than the file's volume change (the spec stores no next state); a blend ending in the end face's plane under the boss would need a face of the boss's underside that the body does not have. A concave rounding at a five-edge vertex (one part, 2): the piece before the end ran 0.43 mm past the point where its contact left the wall (its recorded end point lies on the wall's bottom edge, its last section's contact 0.3 below it), and the file cuts both that piece's last part and the extension with the bottom face, grown along the ridge circle to where the cut meets the other contact; a corner over two surfdata with that prolonged edge, not built |
  | the cap crosses another rounding's cap on the face at the end | 5 / 6 (second round) | two roundings end at the two ends of a short edge that is not rounded, and their caps cross on the face at the end (4, one part in two files): the file intersects the two blends with each other and trims both with that face, an end of two curves; now built so (0210, the mitre an exact ellipse arc, all 4 the file's body); a radius of 100 on a body of 20 (1): the face at the end is a cylinder whose seam starts at the corner's vertex, and the cap was sought round the whole cylinder between the two sides of the seam, crossing the next rounding's cap (0211, now the file's body); edges of 0.6 micrometres at the vertex (1): two vertices half a micrometre apart where the file has one, so two roundings end at two vertices that are one within the import's geometric tolerance; with the edges shorter than 0.01 mm merged first the rounding builds within 1e-3 (the same faces) but removes 5 % less than the file, whose corner patches at those vertices differ; the merge belongs to the healing of stored bodies with the import's tolerance (mitcad#224), not done here |
  | no cap written (`OnSame` end) | 0 / 5 | the contact on the face at the side ends half a radius short of the face at the end (the walk stops where the other contact meets that face's edge, 1); concave roundings of ribs narrower than the radius whose wall contact runs into a window of the wall (1); a conical face at the end (2, one part in two files); not traced further (1) |
  | the intersection at the end through several faces | 5 / 7 (third round) | a tab's rounded corners whose top edges are rounded up to where the top turns tangentially into a rim's torus (3, one part in three files): the walk round one corner's vertex passed an edge of 13 micrometres and took the far one of the two edges a torus and a cylinder share (0281: the edge at the walk's vertex, prolonged from there), and at the other corners the top's contact crosses the tangent edge past the vertex, the blend cut by the torus up to that point (0280); all three the file's body (5.2e-6, the same 64 faces). A wall's foot rounded up to a hole in the floor beside the wall (2, one part in two files): the blend is cut by the hole's side and corner chamfer continued past their faces (0282) and is one face with its piece along the next edge (0283); both the file's body (3.8e-12, the same 96 faces). Left: a rounding of 0.5 whose contact on a boss reaches the boss's concave edge with a bore just before the bore's rim rounding, where the file pivots the ball about that edge in a corner patch of its own (1, not built); a rounding of 0.1 over a plane face of 0.075 mm² narrower than the radius, whose overflow onto the face beyond leaves no face of the stripe at the vertex (the walk round that face cycles), where the file's rounding vanishes 1.3 short of the vertex and leaves the rest of the edge sharp (1; its edge of 6.7 micrometres lies between the same two faces, so merging it changes nothing, and the strips of mitcad#299 do not apply) |
  | an `OnSame` end with no point on an edge | 0 / 3 | a concave rounding at the foot of a small cylinder's end face (a lens 2.5 high) wider than the face: it overflows the face, and neither contact ends on an edge (one part in three files) |
  | others | 0 / 15 (1 now within 1e-3) | corners of two roundings that both overflow a 1 mm rim (3); corner patches of several roundings after an overflow, whose face beyond holds neither the vertex nor the edge (3, and 2 where a consumed face leaves a constraint of zero length); ends degenerate to 1e-10 at a vertex whose 4-edge corners then fail (3); a notch of 0.1 mm (1); exceptions inside OCCT's intersections and projections (2, one of them now built by 0130 but 15 % off the file's change); the edge beside the end not crossing the face at the end (1) |

  Every group left needs a new construction in OCCT's corners, mostly
  after an edge overflow; none covers more than one part.

  The invalid results (27), the contact lines crossing on a face (8) and
  the exceptions in a rounding's computation (14) were traced in
  mitcad#239 (patches 0150 to 0153 of [the
  port](../../third_party/vcpkg-ports/README.md)); the first failing step
  is found with a dump of the fillet's data structure before the
  topological build. With the file's body reached (within 1e-5, the same
  faces) / within 1e-3 / specs:

  | Group | Reached / near / specs | Cause |
  |---|---|---|
  | invalid results | 1 / 2 / 27 | fixed: a closed rounding rolling on an edge from a vertex where a seam starts, whose seam was cut (0152, the file's body); a corner on an end face split in two faces of one plane (0151, the volume exact, the file splits the two faces along another line: one face more); a corner beside a side face split along an edge in the face at the end (0150, the file's four roundings exactly, its stored state is a different body). Left: the tip of an outline where the chain ends at both vertices of a 0.57 mm edge and runs over a corner arc of the rounding's radius (3, one part); walked blends on stored spline faces whose approximation degenerates (edges of 8.8 mm tolerance, 3, one part); two roundings meeting at the vertices at both ends (two-stripe corners) that leave blend faces of 0.04 mm² (2, not traced further); walked blends of short edges at 4-edge vertices extended far past their spine (2); corners where one contact point is not on an edge (2, and a rib outline with 0.12 mm edges at the corners, 3, one part); concave and convex roundings on both sides of a rib narrower than both, the file's concave one riding on the convex one's torus (1); a concave arc turning tangentially into a convex one at the rounding's end (2); a face of zero width (two coincident circles) at the chain's vertices (1); a radius of 100 between two cylinders ending at 4-edge vertices (1); a corner over several surface data leaving a blend face of zero area (1); chamfers, not traced (3) |
  | contact lines crossing on a face | 0 / 0 / 8 | two roundings 5.8 apart with radius 4, whose blend cylinders the file trims against each other (2); a corner's projection exception, then one contact ending on another's (3, one part); contacts coinciding where a rib is exactly two radii wide: 0153 makes the blends meet where the face between is a strip (a plate's two edges rounded to a full round), but here the rib's top is one face with the ring's top, which would need the ring's edge across the rib's root (2, the file's states unreadable in the dump); coinciding over part of a full circle and an arc (1) |
  | exceptions in a rounding's computation | 0 / 0 / 14 | the contact on one face stands still while the ball pivots about it, where the rounding meets a convex corner rounded with the same radius (the file puts a sphere patch there): the walk sets no transition on that side (11, 9 distinct); a walked piece of zero width along the chain's last edge, whose faces are tangent there, inside the faces (not an extension, so the stripe fails; it would have to end where the faces become tangent, 2); a rational section's weights (1) |

  The pivoting ball is the largest group: OCCT has no blend for a contact
  standing still, and the file's sphere patch replaces part of the
  earlier rounding.

  Chamfers wider than a face (mitcad#240, patches 0140 to 0143 of [the
  port](../../third_party/vcpkg-ports/README.md)): of the 7 specs (5
  distinct inputs), all 7 build now, 4 the file's body (MATCH: a step
  ring's rim chamfered as wide as the ring, the rim in two arcs; a
  hexagonal prism's foot under a disc whose chamfers cross beyond the
  disc's rim, twice; a lug in a bore as wide as its chamfer) and 3 within
  1e-3 (NEAR) where the stored next state differs from the stored input
  elsewhere too (a rib's top; other features' chamfers), the chamfer faces
  themselves the file's (the cone areas of four boss feet within 0.5 mm²
  of the file's). Causes, each fixed in the port: the hatching of a contact
  lying on a circular or closed face edge (moved across the boundary,
  domains across the start of a closed line), a closed contact along a
  closed edge not stored on that edge; a chamfer's OnSame end whose second
  contact meets the face at the end beyond its edge, looked for on the
  bounded face; a chamfer that adds material running a little past its
  face's outline, where the faces beyond must grow down to it (their edges
  prolonged) instead of being cut back; and two chamfers whose contacts
  cross beyond their common face, where the face beyond cuts both.
  Imported again with the port before and after (the ten files of the
  specs' designs and of those whose chamfers fell back with "chamfer
  failed" in a recent run): 5 of the 8 such chamfers and 2 fillets now
  parametric, nothing lost, the final bodies as before; 3 chamfers still
  fail (not among the specs, not traced).

  Rolling on an edge (mitcad#237, patches 0170 to 0172 of [the
  port](../../third_party/vcpkg-ports/README.md)): the blend rolling on
  an edge is now the rolling ball (as 0075 for two faces), turns about
  the edge's ends, and where its contact leaves its face before the end
  is walked on and cut by the face at the end. Of the 15 specs of the
  class (13 distinct inputs) none reaches the file's body; traced, most
  are not a ball rolling on an edge in the file: a rounding of a narrow
  step that the file builds as two tangent tori of opposite curvature
  eating the floor (3); a thin rim whose other rounding fails in the walk
  between two faces (the bridge of mitcad#238) and whose rolling part
  reaches a vertex where the next edge turns towards the ball, so that
  the ball touches it before the vertex (3); a notched rim, the file's
  torus kept over the notches and cut by their faces (2); a ledge
  narrower than the radius at a corner of smaller radius, the file's
  straight blend extended and cut by the corner's wall (2); a floor with
  a hole under the contact, the file's blend cut by the hole (1); a start
  where the ball does not rest on the edge but on the face beside it, with
  no start between the two faces either (3, one of radius 129); a fine
  rim whose rolling piece fails in the approximation (1). Of the 38 specs
  that built through the blend on an edge, one more reaches the file's
  body (a corner whose rounding is coaxial with the circle it rolls on),
  one is closer (7.9e-5 to 3.7e-7, the file's face in three pieces) and
  the others are as before.

  The walks between two faces were traced further (mitcad#238, patch 0160
  of the [port](../../third_party/vcpkg-ports/README.md): a walk over a
  hole whose rim is rounded more sharply than the ball goes on over it on
  the face's own surface and the rim cuts it). Of the 16 (12): in 9 (8)
  the ball would have to pass a face it cannot touch onto the face beyond
  it (a corner rounded with 0.5 or 1 under a rounding of 1 or 1.2 along
  the top edges, a corner pocket of radius 1.25 under a rounding of 3,
  roundings of 0.5 at both ends of the edge under a rounding of 1): at one
  section the ball touches three faces, the pieces before and after it end
  on different arcs of that ball, and the rolling ball leaves the gap
  between them over the face it cannot touch open (the file's body closes
  it with faces of its own), so there is no rolling-ball construction to
  make; they still fail, the last one now at its end corner. In 2 (1) the
  contact crosses such a hole, but the stripe's start overflowed an earlier
  rounding all round a closed edge, a face that nothing cuts loose (the
  result kept it), so the walk over the hole is not taken there. In 3 (1)
  both contacts leave their faces at one section; 2 (2) are chamfers whose
  contact crosses a cylinder's seam where the other contact lies on its
  face's edge. The 249 measured the same way give the same classes with and
  without 0160 (on top of 0130–0153 and 0170–0172: 70 MATCH, 45 NEAR, 1 OFF,
  133 failing); nothing is lost.

  The 2 (1) whose start overflowed an earlier rounding all round a closed
  edge (mitcad#261, patch 0190 of the
  [port](../../third_party/vcpkg-ports/README.md)) now reach the file's body
  within 1e-3 (NEAR: 7.3889 mm³ removed where the file removes 7.3902, the
  walked blend in four pieces where the file has one torus). The earlier
  rounding vanishes once its neighbours are cut; what kept it was the
  walk over the hole's rim, whose piece ended 0.011 mm off the walked
  section where the contact on the cylinder crosses the cylinder's seam,
  so that the seam kept its piece under the blend, and with it the
  cylinder's lower part and the earlier rounding. The other 6 specs of
  mitcad#107's class "no start or the walk fails on wide faces" give the
  same results with and without 0190 (3 the file's body, 3 failing at
  their end corners).

  The ball turning about a standing contact (mitcad#239, patch 0200 of the
  [port](../../third_party/vcpkg-ports/README.md)): where a rounding runs
  past a corner rounded with the ball's own radius, its contact on the
  face across the corner stands still and the blend is a piece of the
  ball's sphere, as the file's sphere patches. On stored (spline) faces
  OCCT's walk did not build it: the line got no transition on the
  standing side (`Standard_DomainError`), that side's twist flag was
  numerical noise, and the blend's orientation was taken at its point
  side. With 0200 the sphere pieces build in all 11 specs (9 distinct
  inputs; in one, three of its four), where a result can be compared the
  file's spheres (four of radius 0.1, 0.011 mm² each).
  Reached the file's body / within 1e-3 / specs: 0 / 1 / 11 (0 / 1 / 9
  distinct). The others stop after the sphere pieces, at causes of other
  classes:

  | Specs (distinct) | Where they stop now |
  |---|---|
  | 1 (1) | within 1e-3: the file's four spheres; a corner of three roundings at another vertex is OCCT's corner patch, not the file's (8.6e-5 mm³, 3 % of the change) |
  | 3 (3) | one part: a later stripe's extension, rolling past its face on a cylinder's own unbounded surface (0076), stops on that surface's seam, an edge without vertices (`Standard_NullObject` in `ChFi3d_FilCommonPoint`); handled in a trial, the result has self-intersecting wires: the contact on the top of the analytic piece before the sphere starts 24 mm off, where the torus' contact circle, which the mirror-image half of the outline shares, first enters the top face (SplitKPart's hatch domain, not cut at the piece's start) |
  | 3 (2) | one part with two pointed tips: the check for crossing contact lines (`ChFi3d_StripeEdgeInter`) finds two stripes' contacts on the top overlapping; one stripe's extension for the corner where the two meet runs 15.9 along the whole other side and is not cut back on the top |
  | 2 (1) | a pocket whose inner corner, drafted, is rounded with 0.42 at the floor under a ball of 0.5: the ball cannot touch that rounding and should bridge it (the class of mitcad#238); the walks stop on both sides ("Chaining is impossible") |
  | 1 (1) | the corners where two stripes meet next to their sphere pieces (`PerformTwoCorner`, `PerformMoreThreeCorner`): a contact p-curve on the common face has no tangent there (`LProp_NotDefined` in `CalculBatten`) |
  | 1 (1) | three of four corners build; at the fourth the chain ends just before the corner and the extension past its end starts inside the turn with both contacts on their faces' boundaries; the walk makes no step |

  The 84 specs that built before give the same results with 0200.

  Two of those groups were traced further (mitcad#239, third round,
  patches 0240 and 0241 of the
  [port](../../third_party/vcpkg-ports/README.md)). Both are one part, an
  outline of arcs of one cylinder whose rim is broken at its tips by small
  V notches (0.4 wide, 0.1 deep) whose edges are not rounded, the rounding
  (0.3 and 0.5) wider than the notch is deep: its contact on the top passes
  above the notch, the file's torus runs on over it, cut by the notch's
  faces, as one face. The edge chain stops on both sides of each notch;
  OCCT ran each end's contact on the top on to the end of its domain on
  that face, round the other half of the outline, and the corner did not
  cut it back. 0241 joins the two ends over the notch: one blend runs on,
  cut by the notch's faces, up to the other's end section, and the two are
  one face. Reached the file's body (within 1e-5, the same faces) / within
  1e-3 / specs:

  | Group | Reached / near / specs | Cause |
  |---|---|---|
  | contacts of two stripes overlapping on the top | 3 / 3 / 3 (2 distinct) | fixed by 0241 (6.1e-6 of the body's volume; the boolean differences of the result and the file's state both ways are empty, so the remainder is the volume integration on the input's spline faces) |
  | an extension stopping on its own surface's seam | 0 / 0 / 3 (one part) | 0240 keeps the seam of the face the extension rolls on (0076) out of the ball's way; 0241 joins the ends over two notches. The rounding still fails as recorded: the file rounds the lower outline's both arcs, but the decoded edge list has one of them only (two of the fillet's 23 edge references in the file resolve in the design dump to two short arcs the list already holds; not traced further), so the recorded rounding ends at a notch whose other side is sharp, with no rolling-ball end. With that arc added the three specs give the file's next state (2.7e-6, the same 53 faces, the boolean differences empty) |

  Of the specs that built before, the six that reach the changed code (an
  extension on a face's own surface, a cut piece at a vertex of the
  contact's face) give the same results with 0240 and 0241.

  The second group's missing arc (mitcad#305, mitcad#307, patch 0260 of
  the [port](../../third_party/vcpkg-ports/README.md)): the decoded edge
  list is the file's own (both references the arc seemed to need name, in
  the file, edges the list already holds). The file's rounding carries on
  from the lower outline's left arc across the three top edges of a small
  tapered hole (0.28 to 0.29 long, 0.25 deep, 0.57 wide at the rim; the
  radius 0.5) onto the right arc between the same top face and the same
  cylinder: one torus face covers both arcs and the notch's edges, which no
  input names. Mitcad's fillets now carry a contour of constant radius on
  across such a notch (its edges within the rounding's contact on the top
  and within the ball's diameter of the contour's end) onto the edge beyond
  it on the same line or circle between faces of the same surfaces; 0241
  then joins the two over the notch. Reached the file's body: 3 / 3 (the
  volume within 2.7e-6, the same 53 faces, the boolean differences with
  the file's next state empty both ways). Of the other specs, one more
  reached the carry before the width bound was added (a groove 1 long and
  0.0043 deep beside a rounding of 0.3, which the file's rounding does not
  cross: the edge beyond it stays sharp in the file's state); with the
  bound none does, so their results are unchanged.

  The group of contact lines crossing on a face was taken up again
  (mitcad#239, fourth round, patches 0270 and 0271 of the
  [port](../../third_party/vcpkg-ports/README.md)), on synthetic bodies
  built after the classes above; the specs themselves were not rerun in
  this round.
  - Contacts coinciding where a rib is exactly two radii wide and its top
    is one face with the top it stands out of (2): 0270 makes the blends
    meet on the line where the face goes on past the line's ends; the
    corners at the rib's root cut the top off along the prolonged edge of
    the face at the root (the ring's edge across the rib's root), and the
    two corners' points there are made one. Ribs out of a plate, between
    two plates and out of a ring, and the concave slot, build with the
    exact volumes.
  - Contacts crossing part of the way (the two roundings 5.8 apart with
    radius 4, 2): where the contacts cross twice on a face that narrows
    to less than the two roundings' width, 0271 cuts the two blends by
    each other between the crossings and leaves the face in two pieces,
    as the files' bodies do; the result has the volume and the faces of
    the body rounded at each edge alone, the two in common (fused, for
    concave roundings), also with different radii and on spline faces.
    Contacts that overlap all along (a plate thinner than the roundings'
    diameter) stay with 0016's ride, whose blend is not the files' cut;
    where such a pair also ends against a face that goes on (a rib), the
    corner at the rib's tip still fails.
  - Not changed: contacts coinciding over part of a full circle and an
    arc (1), which needs the closed stripe's line split where the arc's
    ends cut the face; and the corner's projection exception before one
    contact ends on another's (3, one part), not reproduced.

  Fifth round (mitcad#239, patch 0300 of the
  [port](../../third_party/vcpkg-ports/README.md)), measured the same way on
  the 249 with the port as of this round, before and after: MATCH 88, NEAR
  51 → 54, OFF 1, failing 109 → 106; every other spec gives the same result
  line. The three are one part (a rib of radius 2.5 along a rod of 4.5,
  its foot rounded with 3, the rib's end a spline face): the rib's end face
  grows down to the rounding's contact on the rod, and the rod's seam runs
  between the foot and the contact, so the prolonged edge is cut at the
  seam; its pieces' curves on the end face were the ones computed on the
  rod (the edges of 8.8 mm tolerance that the first round saw). Now valid
  and within 4e-5 of the body (the same 8 faces); the change, 4.885 mm³
  against the file's 4.839, is off by the walked blend's extension to the
  end face (the contact ends 0.07 mm from the file's on one side, 0.008 on
  the other): the file's blends are cylinders, OCCT walks a blend between
  two cylinders and extends its spline. Of this ticket's classes, what is
  left (specs, distinct inputs) is one part each:

  | Class | Specs | Cause |
  |---|---|---|
  | invalid results | 3 (1) | a crescent top whose middle, at a notched tip, is narrower than two radii, rounded all round in one contour ending on both sides of the notch: the contacts from both sides overlap within one stripe and the top keeps its original edge (self-intersecting wire) |
  | | 2 (1) | knife edges (the faces' normals opposite) 0.5 long rounded with 0.2: walked blends wrapping 166 degrees at 4-edge vertices, approximated with 0.05 mm tolerances |
  | | 2 (1) | a fin 1.5 thick rounded on both sides with 0.75 along a chain of faces 1.5 wide (its edge, two earlier roundings, end faces): a full round over several faces that ends on faces that go on (0153 and 0270 handle one strip face) |
  | | 2 (1) each | two-stripe corners leaving blend faces of 0.04 mm² and corners where one contact point is not on an edge (as the first round traced them); an input whose edges have 11 mm tolerances |
  | | 3 (2) | chamfers, not traced |
  | | 7 (7) | not traced further (single specs) |
  | | 1 (1) | a radius of 100 between two cylinders ending at 4-edge vertices |
  | contacts crossing on a face (`ChFi3d_StripeEdgeInter`) | 3 (1) | a boss on a lug whose foot circle touches the lug's rounded end at one point, the lug's face pinched into two faces there: the boss's foot, in two arcs, rounded with 1.5 runs out over the lug's edge at that point (the corner's filling surface fails in a projection, the retry's two stripes then cross) |
  | | 1 (1) | coinciding over part of a full circle and an arc |
  | exceptions in a rounding's computation | 2 (1) | a walked piece along the chain's last edge whose faces become tangent: the section's opening angle reaches zero (`BlendFunc::GetMinimalWeights`) |
  | | 1 (1) | the walked blend's approximation gives knots OCCT's spline surface refuses (`StoreData`) |
  | | 1 (1) | no start for one stripe, the others fail in the walk |

  Strips narrower than the import's tolerance (mitcad#299): in one spec
  the stored body has a band of 0.5 micrometres between two chamfers of a
  flange (and a ring of 0.5 micrometres under the lower one) where the
  design has one edge, and the roundings of the chamfers' edges end at the
  band's two vertices; their caps crossed and the corner failed. The file's
  body has no band: its two roundings meet in one mitre on the crease
  between the chamfers (two faces of 0.256 and 0.281 mm² at each of the 8
  corners, no corner patch). The dressup now, after its other tries, takes
  such strips at the selected edges' vertices (four edges, two of them at
  most 0.01 long, the other two within 0.01 of each other) as the crease
  they stand for: the faces beyond meet in the curve where their surfaces
  meet, each end's two vertices become one, and OCCT's own corner of two
  roundings at a vertex builds the file's mitre (faces of 0.256 and 0.281
  mm², 73 faces as the file, removed volume 1.8915 against the file's
  1.8962 mm³; the earlier probe that merged short edges only, keeping the
  band, built 0.295 mm² faces running past the crease and lacked the lower
  roundings). A step between parallel faces is no crease and is left as
  it is. Of the 249 measured the same way: MATCH 70 → 72 (this spec, and
  one whose chain ends at a face of zero width, two coincident half
  circles, which mitcad#239 listed among the invalid results), NEAR 46, OFF 1,
  failing 132 → 130; every other spec gives the same result line.

- **Edge names several edges fit** ("n edges fit the names"): 118 edges
  in 25 items. The recipe's tail after its type string, `i32 -1 | i32 -1
  | u32 n` then per vertex of the edge a record of entity indices
  separated by `-1`, appears to list the faces around each end of the
  edge in cyclic order (the edge's two faces and the end face, in an
  orientation that differs between otherwise equal recipes), followed by
  two words (`0 0`) or longer index lists; it would tell mirror-image
  edges apart, but the decoder's B-rep walk has no cyclic face order
  around vertices yet. Not decoded in this round.
- **Named edges the replay lacks**: of the settled fillets whose every
  edge the decoder found but whose answer is a guess, 85 have named edges
  that are not in the replay's bodies (no by-name definition); 21 more
  had the by-name definition rejected, the answer differing from the
  names both ways (the replay's bodies differ from the file's there).
- **One tangent chain at a time** (`by_chains`): building the
  definition takes no measurable time (0.4 s over 1547 items); its
  evaluation does. Over the 125 files with fillets that fell back or took
  a guess, it was evaluated 300 times for 72 s (5 % of those fillets'
  1488 s): after an approximate match of the edges at once 68 times
  (23 gave the state or came closer, 12 s), after a kernel failure 218
  times (6 gave the state, 57 s), after another state 14 times (1). No
  failure kind, chain count or whether the chains meet at a vertex
  separates the 6 from the 212, so it stays after every failure.

### The published format descriptions (mitcad#154)

Two public descriptions of the `.f3d` streams were compared with what
`core/f3d/src/design/` reads: the cadmpeg project's format description
and its offset tables (CC BY 4.0, <https://github.com/cadmpeg/cadmpeg>,
`docs/formats/f3d.md`, `docs/layouts/f3d.md`, `docs/formats/asm.md`) and
the ezf3d project's format notes (MIT licence, copyright its authors,
<https://github.com/AlexSabaka/ezf3d>, `docs/format/`). They were used as
information only: the layouts below are written in our own words, no
text or code was taken over, and nothing depends on either project. Each
decode taken over was checked on the reference models' dumps
(`MITCAD_F3D_MODELS`, `core/f3d/tests/design_models.rs`) and, where the
history settles the input, on the corpora (the learning dump, *Learning
the undecoded inputs*). Where a layout below was found by comparing the
reference models' records with their dumps and neither description gives
it, the table says so.

Status: **read** (and verified as the last column says), **read
differently** (we read the same bytes with another meaning or another
layout), **not read**. "Ref. models" means checked on every reference
model that has the input; "history" means the corpora's history settled
it (*What the history settles*).

#### From the descriptions to the decoder

| What the descriptions give (source) | Decoder | Notes |
|---|---|---|
| Container: ZIP with stored, deflated and zstd entries, `.f3z` packages (both) | read | `core/f3d/src/container.rs`; `.f3z` members as separate designs |
| Segment registry `Manifest.dat`, external references `RedirectionsStream.dat`, component reference slot (cadmpeg) | not read | the import names inserted documents' occurrences from the design stream only |
| `MetaStream.dat`: header, type table (GUID, base GUID, version, module, ids), primary and secondary record index, named ids, version contexts, property block (both) | read | the whole stream (`MetaStream::parse`); the secondary index is our sub-chunk table, the version contexts our external references, the property block our feature versions; the two `u32` after the module name, which we called a segment version, are a segment type code by cadmpeg (1 design) |
| Reference encoding: present flag, id, same/other segment, cross-document form with link name and version; the inline type GUID of one writer generation (cadmpeg) | read | `stream::ref_at`; the cross-document form as an assembly context (`Ref::context`) |
| Members written one class level at a time in name order; leading block and property block flags (cadmpeg) | read differently | we read the root part (references, attributes) and then fixed layouts per class and version found on the files; the name ordering explains why a member added in a later version shifts a layout, which matches the version-keyed layouts we keep |
| Property block keys (`EntityGenesis`, `pt_tag`, `crv_primary_id`, `crv_secondary_id`, operation id flags) (both) | read | the root part's attributes |
| Timeline record: context reference and ordered items; items without a scope; order = authored order (both) | read | the only non-empty timeline is the design's (mitcad#37 reads each component's list) |
| Parameter records: expression, role, unit, name, value in cm/rad, owner; the three header forms; owner frames and companions with time stamps (both) | read differently | we read every class version found (including the 2026 writers' version 8) by locating the strings; the owner frames and companions are not read, the holder object gives the owner (section 7 of the format study) |
| Sketch points (versions 0, 8, 10, 11), their companion with incident curves (cadmpeg) | read | coordinates and the curves that use each point |
| Sketch lines: start, displacement, direction, auxiliary normal (both) | read | **subclasses now** (mitcad#178): the subclass `16DEFC4D` keeps the line in its sub-chunk, `AE42BAB6` after a reference of its own; read at the base class's place before, 158 lines of 20 sketches came out as denormal numbers |
| Sketch arcs and circles (ezf3d: by their point count) | read | |
| Sketch NURBS curves: nullable carrier, nested header, degree, fit tolerance, counted knots, weights, control points (cadmpeg) | **read now** (mitcad#154) | `sketch::splines`; ref. models' fitted and control-point splines exact; fitted and control-point splines are not told apart (both come in as fixed splines) |
| Sketch ellipses and elliptical arcs (neither) | **read now** (mitcad#154) | `sketch::ellipses`, read on the reference model and the corpus' arcs; centre, unit major and minor directions, radii, angles |
| Sketch texts (`textex_tag`, `txt_tag`): font, height, width factor, colour, string, frame transform or path (cadmpeg) | not read | 21 texts in 7 corpus files; texts come in from external dumps only |
| Sketch relations: member runs, relation ordinals, constraint bits (cadmpeg) | read | constraint types by bit (format study); the ordinal meaning is new to us and not needed |
| Dimension frames (paired, counted, null-locus, annotated) and the rules that bind a parameter to geometry by measurement (cadmpeg) | read differently | we read each dimension object's entities and holder directly; the measurement rules are the description's way to recover what we read; a driven dimension has a null holder reference, read since mitcad#178 |
| Sketch container, member runs, placement matrix, visibility flag (cadmpeg) | read | the transform object; the light bulb (mitcad#6) |
| Profile regions: loops by endpoint incidence, containment, arrangement (both) | read differently | Mitcad computes the regions itself; the decoder reads the selected loops (`_f3d_profile_loops`, mitcad#96) |
| Construction recipes (body, face, bounded face, edge, vertex), generic tag attributes on the B-rep, recipe resolution against the history (cadmpeg) | read | `recipe.rs`, `names.rs`, `inputs.rs`; vertex recipes not read |
| Parameter scopes: state identities, ordered reference table, family name and ordinal (cadmpeg) | read differently | our item tail (result number, base name, index, sub-items, state bytes) |
| Extrude: operation, direction (one side, two sides, symmetric), face-extend option, direction reversal, geometry kind, start support, profile normal, per-side extent kinds (both; ezf3d the first three) | read; **solid, per-side kinds, thin-wall block and the intersect code now** | the reversal is our flag (mitcad#96), the normal our stored vector; `_f3d.extrude.solid`, `start`, `sides` (mitcad#154): every corpus extrusion where read is a solid, sides agree with the parameters and slots; corpora have no "up to next"; `isSolid` checked on 243 ref. model extrusions (4 surfaces); `thin`, `thin_location`, `thin_holder` (mitcad#186: see *Extrusion options from one-option variants*) |
| Extrude: groups by role (profile 0x41, existing bodies 0x04/0x08, termination 0x11/0x12, start face, shape target 0x05) (cadmpeg) | read differently | our input slots and their role numbers (65, 8, 5, 17, 18) are these roles' high words |
| Extrude profile selection by points and history transitions (cadmpeg) | read differently | we read the loops directly (mitcad#96) |
| Revolve: operation, extent kind, direction kind, profile and axis groups, angle scalars (cadmpeg) | read (operation, profile, sketch-line axis) | axes that are faces or edges stay history guesses |
| Fillet and chamfer sizes, fixed scalar lanes, full round, variable law, chamfer forms (cadmpeg) | **read now** except fixed lanes and full round | see the next table |
| Hole: point and direction carrier, support face selection, parameters (cadmpeg) | read (points, type, sizes) | the direction and the support face are not read; two corpus holes, both parametric |
| Thread: designation, size, profile, pitch, modelled flag (cadmpeg) | read | mitcad#35 |
| Loft: section groups, rails, centre line, operation (cadmpeg) | read | mitcad#34 |
| Sweep: profile, path, guide rail, guide surface group, operation (cadmpeg) | read; guide surfaces recognised | orientation, direction and scaling are not given by the descriptions either |
| Pipe: path group and four scalars, settings (cadmpeg) | read (path, operation, sizes) | section type and hollow are in the settings record, not read |
| Combine: operation, keep tools, target and tool pairs (cadmpeg) | read | mitcad#67 |
| Move, Scale: transform record, body group; scale factor and centre (cadmpeg) | read (move) | no corpus scale |
| Shell: thickness, outward flag, removed faces or target bodies (cadmpeg) | **faces, bodies, outside thickness now** | inside or outside is the parameter's role; faces checked on the ref. models; tangent chain not read |
| Offset faces: signed distance, face group (cadmpeg) | **faces now** | |
| Thicken, surface patch, extend, offset, stitch, trim, ruled, boundary fill, delete face (cadmpeg) | not read | not translated by the import |
| Draft: angle, drafted faces, neutral plane or parting tool (cadmpeg) | not read | one corpus draft |
| Split body, split face: tool and target groups (cadmpeg) | read (split body) | |
| Cylinder, sphere, box, torus primitives: operation, placement matrix, sizes (cadmpeg) | **read** (mitcad#25) | the matrix is in the frame of the plane or face the primitive stands on (that frame carries the plane's origin): `frame * placement`; two cylinders and both spheres of the corpus' one design come in, four cylinders on faces of an inserted component do not |
| Coil: operation, clockwise, extent kind, section, position, placement (cadmpeg) | **read** (mitcad#25) | type, section, position, rotation and placement; the corpus' coil (four copies) comes in parametric |
| Base feature: result bodies by entity suffix, state pair (cadmpeg) | read differently | the history's states stand in for base features |
| Component insert, local occurrences, copy-paste, derived instances, occurrence patterns (cadmpeg) | read | occurrence placements (mitcad#75, mitcad#81) |
| Work planes, axes, points: placement frames, solved axis, point data (cadmpeg) | read (planes: frame, definition and, since mitcad#178, the entities they are measured from; axes partly) | construction axes without decoded geometry are skipped; axes are fixed (their inputs are not read) |
| Joint origins, joints, as-built joints, assembly operand frames, alignment owners (cadmpeg) | read | mitcad#66; the description's alignment lanes agree with our owner order |
| Mirror and patterns: seed groups, counts, angle, axis record (origin and displacement), plane group (cadmpeg) | read | the axis record is our `_f3d_axis` (mitcad#96) |
| Appearance and material assets (`.protein` packages: page framing, instance records by schema, descriptions), body presentation records, per-face assignments, body map (cadmpeg) | **read** (mitcad#228) | *Appearances and materials*; every record of the corpora decodes to its end |
| Canvas, decal, mesh bodies, body visibility, unit systems, bounding box caches, display meshes (both) | not read (display meshes: read) | not needed by the timeline import |
| Sheet metal base flange, edge flange, hem (cadmpeg) | base and edge flanges read (mitcad#211); hems not | *Sheet metal*: the edge flange's flip byte and height datum 3 are ours |
| ACT segment, browser segment (both) | not read | |

#### Fillets and chamfers against the descriptions

The descriptions name a fillet's and a chamfer's sizes by the roles of
the parameters a set's holders hold (`Radius`, `StartRadius`,
`EndRadius`, `MidRadius` with `MidParams`, `ChordLen`, `EdgeOffset1`
with `EdgeOffset2`, `TangencyWeight`; `Distance`, `Distance 1`,
`Distance 2`, `Rotate Angle` and older spellings), sizes stored without
parameters as fixed values in the item, a full round as a face group, and
how edge recipes resolve against the history. They do not give the
options a fillet or chamfer stores beside its sizes; those were read on
the reference models (one model per option, against their dumps):

| Input | Where the file keeps it | Decoder before | Now |
|---|---|---|---|
| constant radius per set | `Radius` parameters | read | read |
| variable radius (start, end, mid radii and positions) | parameter roles | not read (the item fell back: no sets) | read, verified on the reference models |
| chord length | `ChordLen` | not read | read (no reference model, no corpus item) |
| asymmetric offsets | `EdgeOffset1`, `EdgeOffset2` | not read (fell back) | read, verified |
| which face takes which asymmetric offset | the set's options object (`4A557CE2`): flipped byte | not read (both tried) | read, verified (flipped and not) |
| G2 continuity | the set's options object: curvature byte | not read (built G1) | read, verified |
| tangency weight | `TangencyWeight` parameter, one per set | not given to the import | read, verified |
| tangent chain | the set's options object: tangent chain byte | not read (the import chained) | read, verified (on and off) |
| rolling-ball or setback corners | the fillet object: the byte after its set list | not read (built rolling) | read, verified |
| what a set selects (edges, faces, a face's edges) | the set input (mitcad#96) | read | read |
| full round, rule fillet | face groups (description) | not read | not read (no corpus item) |
| chamfer corners (chamfer, miter, blend) | the chamfer object: the `u32` after its root part | not read (built with chamfer corners) | read, verified |
| chamfer tangent chain and flip | the chamfer set object (`0D383DB2`): `u8 tangent chain`, `u8 flipped` after its root part | not read | read from the API reference dumps' 312 sets (the flipped byte agrees on all; the chain byte 324 of 324 with dumps and models); older class versions not read |
| options of the older set records (class version 9 and earlier) | another layout | not read | not read (no reference model) |

Over both private corpora every fillet set is a constant radius with G1
continuity and every chamfer has chamfer corners; 12 fillet sets have
their tangent chain off (the import chained them before), and the
options of 92 sets in older records are not read. Of the 249 dressups
classified in mitcad#107 and rebuilt in mitcad#121 (at its close: 63
MATCH, 41 NEAR, 1 OFF, 144 failing), every one is a constant-radius G1
fillet with rolling-ball corners and its tangent chain on, or an equal or
two-distance chamfer with chamfer corners, as the import built them (15
of them in records whose options are read only in part, none of the
corpus' options differing from the defaults there): their inputs are
complete and honoured, and what remains of them is the geometry
kernel's.

#### Our other problems against the descriptions

| Problem (where it is described here) | Answered by the descriptions? | Done |
|---|---|---|
| Sketches left partial by curves not decoded (96 + 25 partial sketches; *Corpus test*) | NURBS curves: yes (cadmpeg); ellipses: no | both read now: 125 sketches of the backup corpus and 25 of the main corpus had splines (`D82E012F`) or ellipses (`FF23079A`) |
| Extrusions whose regions are wrong (*What the history settles*) | the descriptions infer the selection from points and the history as we did before mitcad#96; our stored loops are more direct | — |
| Left-out sketch constraints and dimensions (*Corpus test*) | partly: relation bits and dimension frames | not done |
| One-sided cuts through all whose flag byte is 0 go either way (*What the file does not settle yet*) | no: the description's direction reversal is our flag and its profile normal our vector; nothing else | — |
| Two-sided extrusions up to objects (mitcad#104) | the termination groups in reference order, as we read them | — |
| Surface extrusions tried as solids | yes: the geometry kind | read now |
| Extents up to the next face | yes: per-side kind 3 | read now (none in the corpora) |
| Rectangular patterns: spacing or whole extent | the description takes the stored distance for the extent; our reference models (spacing) store the spacing: see *Where the descriptions and our reading disagree* | not done |
| Patterns and revolves about edges or faces | yes: the analytic carrier's axis in the history | not done (the history settles them) |
| Revolves about construction axes not imported (5 in the backup corpus) | yes: the solved axis carrier | read now: construction axes come in as fixed axes |
| A hole's direction and extent up to an object | direction yes (point-and-direction carrier), extent by parameters | not done (two corpus holes, both parametric) |
| Sweeps' orientation, direction and scaling; a pipe's section and hollow | no | — |
| Closed lofts, participant bodies of lofts and sweeps | no | — |
| Sketch texts | yes (`textex_tag`) | not done (21 texts) |
| Shells whose body or faces were not found (5 + 1) | yes (face group, outward flag) | read now |
| Offset faces whose faces were not decoded (2 + 2) | yes | read now |
| Cylinder and sphere primitives (6 + 2) | operation and sizes yes; the placement matrix does not place the corpus' primitives | placement read in the frame of the plane (mitcad#25): 4 of 8 come in |
| Coils (3) | yes (operation, handedness, extent kind, section) | read (mitcad#25) |
| Drafts (1), moved faces, embosses, sheet metal | drafts yes; sheet metal yes; the others no | not done (Mitcad's draft exists; one corpus item) |
| Unknown item classes (`?…`) | — | none in the corpora |

#### Coverage before and after

Both private corpora with the release build before and after the change
(the same base, three files at a time, *Coverage runs in parallel*;
items parametric / partial / fallback / skipped):

| Type | `MITCAD_F3D_CORPUS` before → after | backup corpus before → after (one design left out, below) |
|---|---|---|
| all items | 1066 / 61 / 237 / 270 → 1095 / 34 / 236 / 269 | 4798 / 269 / 1031 / 1149 → 4948 / 168 / 992 / 1139 |
| sketches | 335 / 46 → 363 / 18 | 1632 / 161 → 1740 / 53 |
| extrusions | 295 / 76 fallback → 295 / 76 (one gained, one lost) | 1354 / 325 → 1373 / 307 |
| fillets | 248 / 77 → 248 / 77 | 1084 / 312 → 1088 / 308 |
| revolutions | 20 / 7 → 21 / 6 | 81 / 20 → 87 / 14 |
| circular patterns | 14 / 13 → 14 / 13 | 80 / 50 → 84 / 46 |
| mirrors | 6 / 2 → 6 / 2 | 18 / 9 → 22 / 5 |
| moves | 6 / 0 → 6 / 0 | 11 / 8 → 15 / 6 |
| shells | 0 / 1 → 0 / 1 | 0 / 5 → 1 / 4 |
| construction axes | 1 skipped → 1 partial (fixed) | 7 skipped → 7 partial (fixed) |

Final bodies: `MITCAD_F3D_CORPUS` 529 exact and 6 within 0.1 % before and
after; backup corpus without the design below 3182 exact and 52 within
0.1 % after (3187 and 52 with it before). The sketches with splines and
ellipses account for most of the gain (and the items after them that now
replay instead of following a fallback, among them items that reached
the import's time limit before); two revolves of the backup corpus come
in about construction axes. Extrusions: 23 gained and 5 lost in the
backup corpus, 1 and 1 in the other. The lost ones come after items
that now replay differently: a boolean giving `BadOrientationOfSubshape`
on a profile with an elliptical arc (the same design in both corpora),
a cut of one design (stored twice) whose first reading leaves out a body
of its state and whose other reading OCCT fails ("Courbes non
jointives"), and a cut whose target no longer lies across the whole
profile. One revolution of the backup corpus no longer gives its state.

One design of the backup corpus (140 items) now ends the import process:
OCCT corrupts its heap (`free(): invalid next size`) in a chamfer's
second definition on bodies that a fallback before it brought in; the
items before it come in differently now that their sketches are
complete. It is left out of the backup numbers above; it is a kernel
crash, not decoding (open).

#### Shells after the descriptions (mitcad#158)

With the removed faces read (above), the shells whose body was not found
came in as follows:

- the labelled example (a box hollowed from its top face, sharp, inside,
  wall 200 mm) is parametric and matches its history state;
- a removed face with a hole round its middle (a planar ring) was not
  found: the decoder gave the mean of the face's edge points as its
  `point_on_face`, which lies in the hole. A planar face's point is now
  the mean only where that lies in the face, else the point of the face
  farthest from its edges along lines across it
  (`mitcad_f3d::names::point_inside`);
- stored bodies (after a fallback) with only C0 continuous B-spline
  faces: the kernel's offset refused them; the shell now splits such
  faces at their creases first (`geometry/src/faceops.cpp`,
  `split_at_creases`);
- the rest fail in the kernel's offset, which cannot drop a face that
  vanishes at the wall's thickness: a 0.17 mm chamfer strip left between
  a 3 mm rounding and the bottom under a 0.8 mm wall (two designs), and a
  convex rounding of about 0.45 mm under a 2 mm wall (one design, stored
  in both corpora). The same shells come out at thinner walls (0.3 mm and
  0.4 mm). On two designs whose bodies before the shell are stored ones
  (after fallbacks), the offset raises an exception extending an edge of
  the stored B-spline roundings, or gives a shape whose edges fail the
  check (that design's state before the shell cannot be rebuilt, so it
  could not be confirmed either way). The shell's error now says which
  of these it met.

With the OCCT port's patch 0100 (mitcad#172) the kernel's offset drops
such faces and joins the faces beside them: the shells with vanishing
faces (two designs of one family, and the design stored in both corpora)
are parametric. Three of the four match their history states; the
other's state cannot be rebuilt (its input comes after a fallback).

#### Where the descriptions and our reading disagree

- **Symmetric extrusions' side kinds.** The cadmpeg description gives
  `(3, 1, 0)` for a symmetric distance and `(3, 1, 1)` only for two
  class-keyed forms; every symmetric extrusion of the corpora whose sides
  are read stores `(3, 1, 1)`.
- **Two-sided extrusions with a side through all.** The description lists
  two-sided distances and two-sided faces only; the corpora have `(2, 1,
  4)`, `(2, 4, 1)` and `(2, 4, 4)`, which our input slots already read as
  a side through all.
- **Rectangular pattern distances.** The description takes the stored
  direction distances for the seed-to-last extent (spacing = extent / (n
  - 1)); the reference models' patterns, of the spacing type, store the
  spacing. The file's choice between the two is not read.
- **Primitive placement.** The description reads the cylinder's and
  sphere's matrix as their placement; it is placed in the frame of the
  plane or face the primitive stands on, whose own matrix carries the
  origin (mitcad#25).
- **The bytes after a spline's degree.** One paragraph of the cadmpeg
  description calls them a constant token, another a fit tolerance: they
  are an `f64` (1e-9 in the records looked at).
- **The two `u32` after the meta stream's module name**: a segment type
  code by the description; we had called them a segment version (they are
  read, not used).
- **ezf3d**: the sketch's placement is in the design stream (the sketch's
  transform object, and the cadmpeg description agrees), and an extrusion
  names its profile's sketch (its profile source input, mitcad#96); ezf3d
  finds neither. ezf3d gives a parameter to the feature before it by
  position; we follow the parameter's holder, which names it.
- **Scopes no timeline item lists.** The cadmpeg description calls them
  internal operation operands or history carriers whose state chain
  bridges listed scopes. The off-timeline extrusions of the corpora are
  real features: the parts of timeline items in other components, which
  the design's table of item parts names (mitcad#169, *Parts of items,
  face profiles and sketch heights*). Their records also hold the number
  of the operation before theirs in their component (seen on the parts
  looked at; not decoded), which may be the chain the description means.

#### Removals, deleted faces, copies, splits and cut profiles (model by model)

Found by importing real designs one at a time; each rule is general.

- **Removals** (`RemoveFeature`): the item's body input is decoded
  (`itemToRemove`) and imported as `remove_body`. The history keeps the
  removed body as it was, so it cannot check the item against a state: the
  body is the stored body of the state the replay stands for that has its
  measures, and that stored body leaves the history from the replay's state
  on (`removal.rs`, as the tools a combine consumed do). A split's new piece
  is found by its index among the split's own new bodies, not among the
  bodies the split divides as well (`producers.rs`).
- **Deleted faces** (`DeleteFaceFeature`): the face inputs are decoded
  (`deletedFaces`) and imported as `delete_face`, one feature per body.
  Face recipes with a second entity list (faces and bounded faces; the first
  list names the face as it does for edges) now decode. A face whose point
  lies on its surface but off the replay's trimmed face is not found.
- **Copies and cut and paste of bodies** (`CopyPasteBodies`): the source
  bodies are decoded (`sourceBody`). A copy stays in its component: a
  `move` with `copy` and an identity transform. A cut and paste into
  another component is the Move to Component command with an existing
  destination (`pasting.rs`): the bodies and the features that made them go
  into the component the item is in, an existing one placed once like the
  source, and the bodies keep their identity and their place in the design.
  The item has no state of its own, so the check is that the replay's
  bodies are unchanged and the destination's afterwards; a second command
  that also takes the bodies a feature ties to the first ones is tried when
  the first is refused. Bodies that went with an earlier cut (the features
  that made them also made the bodies of that cut) are in the destination
  already: the item is satisfied. A cut whose item has no body input (the
  file keeps the cached result of a reference that failed there) says so.
  A copy, a cut and a removal use no sketch of another component (the
  copies of the last sketch made for items that use none are gone).
- **Split tools**: a plane the file no longer has (the construction plane
  was deleted after the split used it) is stored with the entity input as an
  origin and two unit directions (`PLANE_FRAME`); it is decoded as the
  "reconstructed" plane of the API dumps (the two designs checked agree
  exactly). A tool selection may hold several tools (`splittingTools`): the
  split's `more_tools` divide the pieces in turn. A tool body (or the body of
  a tool face) of another component than the split bodies' is read there by
  the split's `tool_links`, as a combine's tools are (the paths to the two
  components, each placed once), and the split goes where its bodies are.
- **Cut profiles**: the regions the faces a profile selects cover leave out
  slivers (below a millionth of the face's area, left where the file's lines
  nearly meet); with the sketch region selected too, a sliver made the XOR
  of the regions an empty tool. The sketch's region tolerance is at least
  1e-6 mm: lines of a file that lie 4e-8 mm apart on each other no longer
  join the regions either side of them. A cut's pieces below a millionth of
  the largest piece's volume stay in its body as lumps, as the file keeps
  them: an extrusion's, a revolution's and a combine's cut alike.
- **Sketch regions at tangent contacts**: the bounds that decide which
  curves of a sketch can meet were those of the sampled points, so an arc's
  top fell short of the line that ends tangent on it and the contact was
  never tried (the lens between the line and the arc was no region, and the
  extrusion that selected it fell back). Round curves now bound their
  extremes.
- **Tangent distance** (`SketchTangentDistanceDimension`): the gap between
  a circle or arc and a line (the centre's distance from the line less the
  radius) is the model's `tangent_distance` dimension, solved with the
  sketch's other dimensions.
- **Reference curves that do not meet**: curves the file keeps linked to
  the model are locked in the sketch; where two of them share ends that lie
  off one of them (projected ellipses differ by micrometres) no solution
  keeps all of them, and the sketch did not solve. Each level of the
  sketch's import is now tried with those curves locked, then free (their
  ends settle where the curves meet), before it gives up constraints.
- **Tangent handles**: a fitted spline's fit points are held at the middle
  of their tangent handles by midpoint constraints whose curve is a line the
  sketch does not list. The spline's curve is kept as the file has it, so
  these constraints bind nothing and no longer count as left out.
- **Offsets with round corners**: an offset of a chain whose offset curves
  include arcs of the offset distance about a vertex of the chain (the
  corners the file rounds where the chain turns away from the offset side)
  has no copy of each curve alone: Mitcad's exact offset of lines and arcs
  meets its curves at the corners. It is imported as the derived offset
  (computed from the source chain, as an offset of ellipses and splines is):
  the chain in the file's order, each curve's offset, and the corner arcs
  by the join they close (the offsets of the chain's curves `join` and
  `join + 1`). Nothing else can constrain the computed curves. Mitcad
  rounds a corner where the chain turns away from the offset side and trims
  it where the chain turns toward it; where the file does otherwise (in the
  two designs seen, one corner is extended to meet the next offset curve
  while another is rounded), the model refuses the offset and it is left
  out as before, the curves staying as the file has them, after the
  sketch's levels with it (the sketch is partial).
- **Midpoint of two lines**: a midpoint constraint that names two curves
  (the API gives its `midPointCurve` only: the other is no point) puts the
  middle of the first line on the second; a point at that middle holds the
  two constraints.
- **Curves drawn twice**: two curves that lie on each other along a stretch
  (the same edge projected twice) were cut at every sampled piece of it, so
  that the pieces of the two curves did not pair and left slivers that kept
  the regions either side of the curves from being separate. Their
  intersection now stops at the ends of the stretch, where the arrangement
  keeps one of the two. A revolved cut of a sketch with several such edges
  selected too few profiles before.
- **Splines the decoder calls periodic**: a spline with an unclamped knot
  vector is flagged periodic; one whose control polygon does not wrap is an
  open B-spline and imports as one (the projected arcs of a cylinder came
  with such knot vectors).
- **Not translated** (causes found): extrusion profiles that are sketch
  text (the sketch's texts are not decoded); a tapered cut whose tool lies
  away from the material (the replay's tool does not reach the body); cuts
  whose boolean the geometry kernel reports as invalid; a cut across
  components whose parts differ in two of seven bodies; primitives' placement
  (the record's entity inputs are not decoded).

### Census of the non-parametric items (mitcad#178)

What still does not come in parametric, by cause, so that decoding work
goes where it moves the most items. One coverage run of both private
corpora (release build, three files at a time, with the learning dump;
*Coverage runs in parallel*); every item whose outcome is a fallback,
partial, or skipped with geometry (items suppressed or failed in the
file, items of no geometry and waiting items the history shows no
change by are left out) is grouped by its report note, normalised (names
and numbers taken out), and by its item type. A fallback is a **root**
where it is the first of its component, or where its note is not a
symptom of the replay's bodies differing; the symptoms after a root in
its component ("its result is not in the file's history", "does not cut
into any participant body", "no copy ... reaches a body", "no body
changes ...") are counted as **downstream** of it, since they mostly
follow from it. Extrusions and revolutions with a symptom were traced
to their profile sketch through the learning dump (a partial sketch, a
sketch on a fixed plane, an earlier fallback). The backup corpus repeats
some designs of `MITCAD_F3D_CORPUS`, so the counts are instances.

Totals (parametric / partial / fallback / skipped), before this ticket's
work and after its batch (the "after" also has mitcad#161, merged into
main meanwhile):

| | `MITCAD_F3D_CORPUS` | backup corpus |
|---|---|---|
| before | 1132 / 38 / 210 / 261 | 5169 / 185 / 936 / 1112 |
| after | 1147 / 28 / 206 / 260 | 5249 / 120 / 923 / 1110 |

Final bodies as before: 492 and 3086 exact, 9 and 61 within 0.1 %, none
off.

Non-parametric items by category, both corpora (before → after):

| Category | Items | |
|---|---|---|
| the import's time limit (6 designs; mitcad#181) | 418 → 405 | |
| reading and translation: fallbacks | 303 → 297 | the causes below |
| geometry kernel: fillets, chamfers, tapers, booleans that fail or come out off the file's | 220 → 221 | mitcad#121 and others |
| the file's own base features (bodies without a parametric history in the file) | 166 → 166 | nothing to read |
| construction planes as fixed planes | 96 → 40 | done below |
| no translation in Mitcad (bodies of other designs 20, sheet metal 18, copied bodies 14, deleted faces 8, primitives 8, forming threads 5, removed items 4, coils, electronics boards, mirrored components 3 each) | 93 → 93 | |
| sketches partial | 72 → 53 | done below |
| positions inside components of other documents (captured positions, relationships) | 47 → 47 | not in the file |
| after a fallback to the stored bodies (no history to continue from) | 18 → 18 | |
| construction axes as fixed axes | 8 → 8 | their inputs are not read |

Reading causes, ranked (roots + downstream, before; and what was done):

| Cause | Roots | Downstream | Done |
|---|---|---|---|
| construction planes whose inputs the decoder did not read (fixed planes) | 96 (partial) | – | read: 96 → 40 |
| extrusions "not in the file's history" | 36 | 24 | three of the five whose sketch was partial and on a face of another component come in (mitcad#161 with the driven dimensions), and two on sketches on tangent planes; the rest are single cases (profiles the decoder reads but whose prism is not the state, participants, a sketch with three profiles and one region) |
| sketches with dimensions "not decoded" | 28 sketches (137 dimensions) | – | driven dimensions read: 28 → 7 sketches (11 dimensions left: 8 linear dimensions to geometry outside the sketch, 3 of a class not read) |
| fillets "not in the file's history" | 19 | 7 | not reading: the edges are found by their names (tiny edges, results the kernel builds otherwise) |
| sketches with constraints "not decoded" | 13 sketches (23) | – | not done: a constraint type bit (`0x100_0000_0000`) on four construction lines in 5 designs, and 4 constraints whose record is not read |
| combines "no body changes in the next history states" | 6 | 10 | not done |
| revolutions "not in the file's history" or without profiles | 9 | 14 | one revolution (lines of a subclass; one design, stored in both corpora) comes in |
| sweeps "not in the file's history" | 3 | 7 | not done |
| extrusions "do not cut into any participant body" | 5 | 13 | not done |
| rectangular patterns "not in history", "no copy reaches a body" | 9 | 4 | not done |
| sketch lines of two subclasses read as denormal numbers | 158 lines in 20 sketches | – | read |

What was read (each checked before the import relies on it):

- **Driven dimensions.** A dimension without a parameter stores a null
  reference, one zero byte, where the parameter holder is, then its
  entities as any other (`sketch::parse_dimension`). The decoder gave
  such a dimension no type (130 in 22 designs, all between two lines or
  a point and a line); it now reads them as driven dimensions with the
  value they show (`_f3d.expression`). Checked against the sketch's
  geometry: all 130 shown values are the distances the decoded lines
  and points have.
- **Sketch lines of subclasses.** Two subclasses of the sketch line keep
  the line elsewhere: `16DEFC4D` has its own two point references after
  the root part and the line (12 doubles and the point references) in
  its sub-chunk after the chunk's header; `AE42BAB6` has one reference of
  its own before the line. Read at the base class's place, 158 lines of
  20 sketches in 14 files came out as denormal or huge numbers (lines
  without points, construction rectangles of nonsense); now every such
  line's ends are at its points.
- **Construction planes' inputs.** A plane item lists its parameter
  holder, a frame input and the entities it is measured from. The
  decoder gives them by the plane's definition: an offset's face or
  plane; an angle's line (a construction or origin axis, an edge, a
  sketch line), and where it is a cylindrical or conical face, the plane
  is tangent to it; a distance along a path's edges or sketch curve
  (a fraction where the parameter has no unit, else a length); and for
  planes without a parameter, two planar faces a mid plane, two lines a
  plane through both (a line and a point, three points likewise). Faces
  and edges are looked for in the history at the plane's place in the
  timeline (a plane has no state of its own), as other components'
  inputs are (mitcad#161). The import builds each reading, with the
  previous guesses after it (offsets from planar faces at the distance,
  the origin planes for an angle without a plane, both sides of a
  tangent, both ends of a path), and keeps only a plane that lies where
  the file's is: every definition but a fixed one is now checked (before,
  offset planes were taken unchecked). Left fixed: 20 planes whose
  inputs are vertices (vertex recipes are not read) or not decoded, 12
  along paths of several sketch curves (Mitcad's path is one curve or
  connected edges), 8 offsets from faces the file's history has no
  names for (faces of inserted components or of base features, named by
  operations before the history's first), 7 tangent planes and 5 offsets whose
  readings do not give the file's plane.

Items moved, both corpora: 56 planes and 20 sketches partial →
parametric; extrusions 8, revolutions 2, fillets 4, moves 4 and mirrors 2
fallback or skipped → parametric (13 of these are items of three large
designs that the time limit reached later). The one item lost: an
extrusion of the backup corpus whose earlier extrusions now come in
parametric (two of them fell back before) finds no participant body
where the file's body was taken before.

### Items that fell back after the census: extrusions, patterns, planes, sketches

A second pass over the designs whose extrusions, patterns, datums and
sketches fell back (98 designs, imported one by one, release build) found
these causes; each is a rule that holds for any design:

- **Sketch edges a little apart.** Regions are cut from the arrangement of
  the curves with a tolerance of 1e-9 of the sketch's size; in a sketch of
  some millimetres that is 2e-8 mm, and edges that the file meant to
  coincide but stores 2e-7 mm apart (a small rectangle on the edge of a
  bigger one) made a region with a hole whose wall touches the outline: an
  extrusion with a face too many. The tolerance is now at least 1e-6 mm
  (`sketch/regions.rs`). Designs: 1 extrusion → parametric.
- **Patterns of extrusions up to an object.** `adjust` moved the finished
  tool to each copy, which ends where the object would be if it moved too;
  the file builds each copy again against the unmoved object. An extrusion
  whose end depends on bodies (up to an object, through all, a start
  object) now rebuilds its tool with the profile moved (`placed_tool`,
  `features/extrude.rs`). Designs: 1 circular pattern → parametric.
- **A rectangular pattern's second direction.** A second direction with no
  entity has its vector in the pattern's data object, 24 bytes after a
  `-1` marker, before the two distances' parameter holders (the API
  reference dumps: 4 of 4 patterns with a second direction of more than
  one element agree; one-direction patterns store zeros). The importer
  tried positive origin axes only and fell back. Designs: 1 pattern →
  parametric, the inputs read.
- **A plane's face found in an earlier history state.** A plane taken from
  the faces of a body (a midplane between two faces) can name a face the
  decoder found in an earlier state, with a point on a part that a later
  cut removed; the point then lies on no face of the replay. A plane
  reference now falls back to the replay's face in the same plane
  (`refs::resolve_face_by_plane`). Designs: 1 plane → parametric.
- **Ellipse dimensions that hold the whole axis.** An ellipse's major or
  minor dimension with flag 4 or 8 (the reference models have 1 and 2)
  holds twice the radius, as in the API dumps (4 of 4); the import used
  the value as the radius and the sketch did not keep the stored
  positions. Designs: 1 sketch → parametric.
- **Constraints the stored positions break.** A sketch that was not solved
  again after the geometry it is linked to changed has a constraint its
  own positions do not satisfy (a midpoint of a linked line at a point
  that is not the midpoint); the solve then moved the whole sketch and the
  import dropped all constraints. The solver names the constraints a
  position does not satisfy (`System::violated`, `stored_violations` of the
  sketch status) and the import leaves out only those (a dimension among
  them stays as a driven one), keeping the rest; the sketch is partial
  with the number left out in its note. Designs: 1 sketch, from "without
  constraints" to one constraint left out.

Of the 98 designs 15 had items change outcome (fallbacks, partials
and skipped items became parametric, among them items of designs whose other items
came in because a sketch did), none got worse; final bodies as before
(one design's final body is 0.007 % off, the chamfer after the pattern,
which the file's state accepts at 7e-5).

Found and left: extrusions up to an object whose face the decoder gives as
many candidate entities (a face of an imported body: the extrusion runs
along the face's plane); (an extrusion up to a body of another component
was left here until it got `object_links`, see the next section); planes
and axes defined by sketch lines
that the file stores in three dimensions, and by faces of components of
other documents (the item has no history state); sketch texts (not decoded
from the streams, and an extrusion of a text needs the font); a taper that
closes a profile before the end (a circular segment 0.5 mm deep at 0.3°
over 50 mm: the draft fails where the file's solid ends in an edge).

### Datums and extrusions that name other components (model by model)

Re-imports of the 19 designs that still had a fixed plane or axis, an
extrusion up to another component's body or a mirror falling back (run
one by one, release build; the API reference dumps for the comparison):

- **A path of several sketch curves.** A plane along a path whose
  `pathEntities` are lines and arcs of one sketch had no definition: the
  path was tried as one curve or as edges of a body. A path is now also
  `{"sketch", "curves"}` (`PathRef::SketchCurves`): the curves joined end
  to end as a sweep's path is, from the free end of the first listed one
  (the file's order), a corner taking the tangent of the curve before it.
  Planes: 2 in each of 2 designs (the API reference dumps: 21 planes along a
  path of edges or sketch curves, all agree on the stored plane).
- **An axis normal to a profile at a point.** The profile is the plane of
  its sketch; an axis normal to a face at a point now takes a plane too
  (`Want::FaceOrPlane`), the profile as the fixed plane of its sketch as
  the other definitions on profiles have it, and the axis is `read`
  (4 of 4 in the API reference dumps).
- **A cylinder cut away by later features.** The face a tangent plane
  touches was found by its point, which a later feature had removed; a
  datum needs the surface only, so the replay's face on the same cylinder
  is taken (`refs::resolve_cylinder_surface`). Planes: 4 of 4 in 1 design.
- **Faces that keep their names in pieces.** A recipe whose names fit
  several faces on one plane or cylinder (a face the history split) gave
  no face at all; for a plane's or point's inputs the first of them is
  taken (`NamedState::find_face_surface`), since only the surface counts.
  The API reference dumps: 6 fewer undecoded fields of the 166 on
  planes, no field changed.
- **Extrusions up to a body or face of another component.** An extrusion
  has `object_links` (as a combine's `tool_links`, mitcad#104) for the
  bodies its objects are or are faces of; the import adds them and the
  extrusion goes where its sketch is. Through a body from a profile that
  starts inside it, each part of the profile now runs to the body's far
  side (columns over the parts, as for the first face). Designs: 1
  extrusion fallback → parametric, verified.

Planes and axes still fixed: 24 → 14 over the 19 designs (14 are the ones
below); no design got worse (outcomes and provenance counts
of every re-imported design equal or better).

Found and left, each with its cause:

- **Planes through sketch lines in three dimensions** (2 designs: a plane
  through two edges of a pyramid drawn in a sketch): the import flattens
  a sketch's points into its plane, and Mitcad's sketches are planar;
  the lines are not representable. 11 of 358 designs have such a sketch.
- **Three vertices at tangent contacts** (1 design: a plane through three
  vertices of a cylinder, where a plate's face touches it along a line):
  the file has an edge and vertices there, the replay's boolean has none.
- **Vertices of a body moved into a component of its own** (4 designs):
  the recipe names faces by operations the history blobs do not have
  (their last operation is the move's), and one of the three vertices
  names two equally good corners. Stripping the move's operation matched
  the wrong corners (checked against the API dumps) and was not kept.
- **Faces of components of other documents** (6 designs): the replay has
  no such component; with the referenced documents imported as linked
  components (mitcad#196) the offset's base face would be found by the
  plane the file stores, as the guesses already do for a face of the
  replay.
- **A midplane to a profile of a sketch that is not in the file** (1
  design): the input names a sketch object the file does not hold.
- **At-angle planes without usable geometry** (1 design): the stored
  frame of one is the angle-0 plane, of the other absent.
- **Sketch regions of fixed splines crossing a circle** (1 design): the
  region's hole loops intersect its outer loop (`BRepCheck_IntersectingWires`
  after healing).
- **A mirror whose copy is 0.0103 mm³ off** (1 design): the mirror of an
  extrusion and its chamfer gives twice the file's new faces and a volume
  1e-5 off; not diagnosed further.

### Designs with several remaining problems (model by model, second round)

Fifteen designs (three of them in two versions) that still had several
items outside `parametric` and `read` were imported one by one (release
build) and each remaining item read against the API reference dumps and the
import's trace. Rules that hold for any design:

- **Pieces of an edge that is an ellipse's arc.** An edge the replay splits
  in two or more pieces (a blend's or an intersection's curve) was found by
  the chain of pieces only where the file's middle lies within 2 % of the
  edge's length of the polyline through the pieces' ends and middles
  (`refs::chains_by_ends`). A long piece curves away from its polyline (4 %
  on one edge), so the chain also counts when the middle lies within 2 %
  of the arc through a piece's ends and middle (`refs::arc_distance`). One
  design's edge set that stopped at its fifth edge reaches its 25th now.
- **A sketch circle or arc as a pattern's axis.** A circular pattern about
  a circle or an arc of a sketch (the line through its centre normal to the
  sketch) fell back with `curve … is not a line`: the model's axis from a
  sketch curve (`GeomRef::SketchCurves`, `Want::Axis`) takes circles and
  arcs now, and the import uses the file's stored axis line where the
  sketch faces the other way (`Importer::circular_axis`), so that a partial
  pattern turns the file's way. Designs: 1 pattern → parametric.
- **Chamfer sets' options.** A chamfer set's record (class version 2) holds
  `u8 tangent chain | u8 flipped` after its root part, then its parameter
  holder (a `u8 0` more before it in the sets of equal and of two
  distances). Of the 312 sets of the API reference dumps the flipped byte
  agrees on all (the one flipped set, a distance and an angle, and 311
  that are not); the tangent chain byte is the application's own rule (1 on
  the sets made through its user interface, 0 on the controlled models made
  by script, 324 of 324 against dumps and models). Older class versions
  keep their options elsewhere and are not read.
  A chamfer whose sets are equal distances, or two distances whose sides the
  edges' directions give, with the size in millimetres, the options, the
  corner type and its edges found by their names is `read`
  (`read_chamfer_inputs`); a set of a distance and an angle stays unread (the
  face the angle belongs to is not decoded). On the 15 designs 17 of the 18
  chamfer items that came in parametric are `read` (all were `unknown`).
- **Combines and splits.** A combine whose operation and keep-tools flag are
  decoded and whose target and tools are the bodies the items that made
  them gave (`produced_body`, not bodies found by names or measures alone)
  is `read`; so is a split whose bodies are produced that way, whose tools
  are planes of the file's geometry, faces the decoder named or such bodies,
  and whose extend flag is decoded.
- **A tool a few nanometres from a body.** A circle projected from an edge
  into a sketch keeps the file's geometry, which is 1e-6 mm off the edge's
  own (a ring around a tube wall: its inner circle 0.0000012 mm outside the
  wall). The join then "did not touch any body" (contact within 1e-6 mm) and
  the extrusion fell back. A join that touches nothing is fused again with a
  fuzzy value of 1e-5 mm (`geometry/src/boolean.cpp`, only on that failure
  path, for the targets within 1e-5 mm), and a target joins where the tool
  merged with it. Designs: 1 extrusion → parametric.

- **Participants of an extrusion of faces.** The bodies an extrusion joins or
  cuts were taken from the file (and from the history after them) only for
  extrusions of a sketch's profiles; an extrusion of faces joined every body
  its tool touches, merging bodies the file's join leaves apart. Its
  candidates have the participants now (`with_decoded_participants`), and
  where the file names no body that is in the replay (a stale reference),
  the bodies the history's state changed, and each of them alone where
  several changed (one of them may differ from the history's state only by
  the replay's small errors). Designs: 6 extrusions of faces in 2 designs → parametric,
  `inferred`.

Over the 15 designs (the census re-import before, and this round's build
with the rules of the branches merged meanwhile, counting items):
parametric 631 → 652, fallback 99 → 83, `read` 337 → 377, `unknown` 71 →
42, `inferred` 35 → 40 (planes that came in as parametric from fixed ones,
which were `unknown`, are `inferred`: a tangent plane has no reference
plane in the file or in the API reference dumps).

Found and left, each with its cause:

- **Faces of components of other documents** (planes offset from such a
  face, cuts and copies of their bodies: 4 designs): see the section above
  (mitcad#196).
- **Fillets and chamfers the kernel fails** (the rounding cannot end at a
  vertex, the radius may be too large, an invalid shape, a volume off; 10
  designs, 35 items, and the patterns that repeat them): their decoded
  inputs agree with the API reference dumps where a dump exists, as in the
  classes above (mitcad#121).
- **Sketch texts** (1 design: 5 extrusions): the profile loops of a text
  have no curve records, and the regions come from the glyph outlines of
  the font the file names, which is not installed.
- **Primitives and coils** (a cylinder, a sphere and a coil: 9 items in 2
  designs): the API dumps give no detail for them; the sizes are in the
  parameter table and the placement and choices are decoded since
  mitcad#25 (*Primitives, coils and option constants*).
- **Extrusions whose profile records name curves the sketch no longer has**
  (stale selections, `inferred`): the regions come from the history.
- **Where a circle starts (its seam)** is read now (see "Where a circle
  starts" under the input decoding audit).

### Profile selections: ellipses, splines and stale holes (model by model)

A profile selection is *exact* (and so `read`) when every record of its
loops is found in the sketch with its piece numbered. 74 designs that had
extrusions or revolutions with `unknown` inputs were imported one by one
and each such item's reason read from the trace (the *extrude read* and
*revolve read* lines, and `profile …` lines naming the records that did
not resolve). Of the 133 `unknown` extrusions and revolutions of the
census re-import 55 were extrusions of faces (a different input), 18 had
ellipses or splines in their loops, 43 had records that name curves the
sketch no longer has (7 of them only in holes), 17 had a different cause.

- **Ellipses, elliptical arcs and splines take part in the piece
  matching** (`src/profiles/curves.rs`). An ellipse is parameterised from
  its major axis end, an arc by its start and end angles, a NURBS curve
  (rational or not, clamped or not) by its knot domain. Where each of them
  meets another curve is found on the curves flattened to polylines and
  refined on the curves themselves (Newton's method in both parameters);
  the ends of a curve that lie on another cut it there, and the meeting
  points inside a stretch where two curves lie on each other (a spline
  along an arc) count as the ends of the stretch only. The even-odd ray
  test is exact for ellipses and numeric (bisection on the curve) for
  splines. Before, a profile that listed such a curve selected its regions
  by the share of curves (`unknown`).
- **The file's own piece count validates the numbering.** A record names
  piece `piece` of `pieces` of its curve. Where the cuts of the record's
  loop give another count (the curve is cut by curves in other loops of
  the profile, or by curves of the sketch the profile does not list), the
  cuts of all curves of the profile and then those of the sketch (without
  construction curves) are tried, and the set that gives the file's count
  numbers the curve (3 records of the 149 that disagree). For a curve
  that is an ellipse or spline, or is cut by one, where no set agrees the
  numbering is not trusted and the loops are left to the curve-overlap
  rule (a spline that wavers across an arc meets it where the file
  counts no cut: 1 profile). For lines, arcs and circles the loop's
  numbering stays as it was where no set agrees (duplicated, overlapping
  lines of a sketch number their pieces in a way the cuts do not give:
  about 150 records in 27 designs).
- **A hole whose records name no curve of the sketch any more is
  dropped.** The sketch was edited after the profile and the hole's
  curves were deleted; the profile is what its other loops enclose, the
  regions inside the former hole are selected with the rest. The
  application's profile list agrees with this on all 5 of the 7 items (in
  the 74 designs) whose records are stale only
  in holes that have one; the selection is exact.
- **What the file does not give: the curves a stale record stands for.**
  The curve objects carry the root attributes `EntityGenesis`,
  `crv_primary_id` and `crv_secondary_id` only (the numeric ones listed on
  all designs, the raw bytes of a replacement curve inspected on one);
  nothing names the curve that replaced a deleted one.
  Of 231 stale records 213 name a primary id the sketch no longer has,
  18 a primary id that other curves still carry under other secondary
  ids (a curve split afterwards: the pieces' secondary ids are new, a
  record's secondary id of the old state is not among them).
  There is no mapping to read, so a selection with a stale record in an
  outer loop (35 items) stays `unknown`: the regions come out as before
  (agreeing with the API reference dumps' profile lists in 28 of 36
  where a dump exists, counting each design version; in the others the
  application's profile is one
  region that Mitcad's sketch splits, or the reverse).

On the 74 designs (the extrusion and revolve items; the before column is
the branch's base, and the 5 fallback items fewer come from other changes
merged into it meanwhile):

| | before | after |
|---|---|---|
| `unknown` | 92 | 65 |
| `read` | 1057 | 1085 |
| `inferred` | 68 | 72 |
| fallback items (all kinds) | 316 | 311 |

The 28 items that became `read` have selections that agree with the
application's profile lists (areas, 20 of 23 where a dump exists; the
other three differ by 0.2–0.6 % in the area of a spline-bounded region,
the same regions). The remaining `unknown` ones: 35 stale outer loops, 4
with ellipses or splines whose cuts the file's counts contradict, 26 whose
profiles are exact but whose participants (6) or extent (the rest) are not
read, which this section does not change.

### A large assembly design and its sibling (model by model, third round)

Two related designs (one of them stored in both corpora) that still had 16
and 10 items falling back after the earlier rounds were imported one by one
(release build) and each remaining item and each `inferred` or `unknown`
input read against the API reference dumps and the import's trace. Rules
that hold for any design:

- **Faces extruded through all or up to an object.** An extrusion whose
  profile selects faces of bodies (mitcad#169) took a distance only; one
  through all or up to an object fell back to the last sketch before it.
  The decoded faces now take any one-sided extent as the sketch's regions
  do (`face_profiles_without_distance`): one sketch on the faces, the side
  read where the file gives it (through all: the direction flag; up to an
  object: the side of the faces' plane the object lies on) and the other
  side after it as a guess, with the bodies it joins or cuts as decoded
  first (also for a distance). A planar face the replay does not have as it
  is goes by its plane.
- **Curves on the edges of the sketch's face.** The file's geometry is
  exact to 1e-5 mm. A sketch on a face whose curves lie on the face's edges
  in the file can lie 1e-6 mm off them in the replay: the file's stored
  radius is 85.0000013 mm where its dimension says 170 mm, the replay solves
  that sketch to 85 mm, and a later sketch drawn on the extruded face
  without a dimension keeps 85.0000013 mm. Walls extruded from it then stood
  1e-6 mm off the body's walls: the join left them apart or split edges in
  two. Every line, circle and arc of a sketch on a face, drawn or linked,
  that lies on one of the face's edges within 1e-5 mm now takes that
  edge's curve before the sketch is solved (`sketch/edges.rs`; a point two
  such curves share goes where their curves meet; a curve whose points
  cannot all move within the tolerance stays). The sketch's note counts
  them. The snap of linked curves onto any edge of the model (`snap.rs`)
  runs after it: it moves a point two curves share where the first one
  puts it and leaves the second curve as it was, so on its own it left an
  arc whose end moved off its circle (an extrusion of such a profile
  failed, a sketch lost a constraint); with the face's edges first, those
  points are already where both curves meet.
- **Slivers are no pieces.** OCCT's union of walls that continue a body's
  walls exactly can leave strips 1e-6 mm wide as solids of two faces beside
  the result; a solid of up to six faces whose volume is under its area
  times 1e-6 mm is now left out of a boolean's pieces (where it is not the
  only solid), so it no longer becomes a body.
- **A pattern's copies find a face the original took away.** A pattern
  (Adjust) builds each copy of an extrusion up to an object again; an
  extrusion that joins a body up to that body's own face takes the face
  away, so the copies found no face. The copies now take the face from the
  body as it was before the original feature where the current body lacks
  it (`EvalContext::objects_before`, `body_before`).
- **Up to a body, the other solution too.** An extrusion up to a body
  stores the far side's flag in the file but ends at the body's first face
  (it fills a pocket of its participant up to the body above); the other
  solution is tried after the decoded one, as a guess.
- **Moves as defined.** The decoder gives the move's definition from its
  parameters' roles (`RotateAngle` with the entity input as the axis;
  `TranslateX`, `TranslateY`, `TranslateZ`) or, for a move with neither
  parameters nor point, direction or entity inputs, the free move of its
  matrix. The import makes a rotation or translation with the parameters
  (inputs `read`), a free move `read`, and keeps the fixed matrix after it.
  Against the API reference dumps: 19 free moves and 3 rotations agree, none
  differs; 2 point-to-point moves are not decoded.

Outcomes (parametric / partial / fallback; inputs read / inferred /
unknown), before → after, final bodies exact before and after:

| | items | inputs |
|---|---|---|
| first design | 176 / 0 / 16 → 178 / 1 / 13 | 94 / 6 / 20 → 109 / 6 / 8 |
| its sibling | 127 / 1 / 10 → 130 / 1 / 7 | 70 / 3 / 14 → 83 / 3 / 4 |

(The partial item is a sketch whose linked arc the snap onto the model's
edges moves 7.5e-4 mm, off the tangency the file keeps. Chamfers,
combines and splits became `read` through other work merged
meanwhile.) One item went the other way: a two-distance chamfer on the
circle now split in two arcs of the extruded walls (the replay's edges),
which the geometry kernel refuses with the 2 mm distance on the wall side
and builds 1 % off the file's with the other reading; before, it rounded the
stored body that the failing extrusion left.

Left, each with its cause:

- **A face the replay merged with its neighbour** (an extrusion through all
  of ten lens-shaped faces): the file keeps the faces where a revolution's
  face meets an earlier body's coplanar face; the replay's union merges
  them into one face, so the profile's faces resolve to that whole face.
- **Fillets and chamfers the kernel fails or builds otherwise, with inputs
  that agree with the dumps** (mitcad#121): full rounds of 1.5 mm on 3 mm
  faces (two items), roundings that end at vertices (six items), two
  roundings overlapping on a 2.5 mm face, a patterned chamfer at one copy
  (a self-intersecting wire), and the chamfer above.
- **A fillet whose references the file itself lost** (16 of 48 edges).
- **Moved faces**: no translation; the reference dumps give no inputs.
- **A cut of stored bodies 1.8 mm³ off** the file's change on the exact
  history state (OCCT's boolean).
- **Inputs not read**: decoded profile loops that name a curve the sketch
  no longer has or a piece its cuts do not give (four extrusions), the
  side of a revolution by a negative angle, a mirror and three extrusions
  whose decoded definitions do not give the state (the history picks
  another), positions and relationships.

### Face profiles and extents up to objects on two sides (model by model)

Of the extrusions with `unknown` or `inferred` inputs left on the 58
designs that had any, 7 take a face of a body in their profile together
with a sketch's regions (the face's boundary is projected into the sketch)
and 8 take faces only. Their faces were never part of the read condition.

- **A profile face is read when the decoder found it by the file's own
  names and the replay has exactly one such face.** The decoder resolves
  the face's recipe (the names of the entities that bound it) in the
  history and gives a point on it and its plane
  (`refs::face_resolved_uniquely`): no other face of the surface type
  passes through the point and faces the same way, and the face is the
  one the replay resolved. As for the dressup edges, a reference the
  decoder could not complete (no point: the history lacks the face), a
  face without the file's names, and a face the replay has twice are not
  read. With a sketch beside them the regions of the sketch on the faces
  must make up the faces' area, as before. The other inputs (operation,
  start, thin wall, participants, extent) follow the rules of sketch
  profiles; an extrusion of faces only is read when the stream's side
  kinds say one distance along the normal or one extent up to an object
  that lies on the side the extrusion goes to.
- **The faces agree with the API reference dumps.** The profile faces of
  all 71 extrusions whose faces the decoder found were compared with the
  dumps' profile faces by plane: 68 are the same plane in the item's
  coordinates, 3 lie in a placed occurrence and are the same plane after
  its placement (the decoder's normal there points the other way than the
  dump's after the placement; the replay's face is chosen by the side of
  its own normal and the check passes).
- **An extent up to objects on two sides is read** when the stream's side
  code names side one's object (2), side two's object block is in the
  stream (its nearest-solution byte, which the side codes do not name
  after an object) and the extrusion goes towards the side of the sketch
  that side one's object lies on. Before, only two distances and sides
  through all were read.

What stays unread among the exact selections: the participants of 4
extrusions and 2 suppressed revolutions (the stream's body recipes of
bodies that the history state lacks, or a body recipe with a second list
whose meaning is not known: the import finds no body for them) and the
angle's direction of a partial revolution (the stream gives no direction
byte; 1 item).

On the 58 designs (single-design imports, the extrusion and revolve items):

| | before | after |
|---|---|---|
| `unknown` | 65 | 59 |
| `read` | 802 | 812 |
| `inferred` | 59 | 55 |

The outcomes of all 3541 items are unchanged (2890 parametric or partial,
407 skipped, 244 fallback items before and after): only the provenance
changed. Of the 15 face items 9 became `read`; the other 6 stay: 2 with a
selection that is not exact, 4 whose participants the history chose.

### Four designs whose remaining items had geometric causes (model by model, third round)

Four designs with one to four items outside `parametric` were imported one by
one and each item traced:

- **Splines that follow a circle or a line.** A projected edge kept as a
  spline lies within a micron of the circle or arc drawn on it, crossing it
  in places. The sketch's arrangement cut the circle at every crossing, so
  the regions around such curves carried slivers and holes of 1e-3 mm²
  between the two, and the profile faces failed (`a profile loop is not
  closed`). Two pieces with the same ends that run along each other within
  1000 tolerances (`Arrangement::run_along`: points along the one lie on the
  other's stretch, both ways) are one edge now, and of a spline and another
  curve the spline stays (the faces next to a projected edge follow it
  exactly: with the circle in its place the next extrusion, a micron off,
  did not merge with the body). Designs: 1 extrusion → parametric.
- **Up to a chained face.** The face and the faces next to it were the
  target; the chain of faces joined tangentially (`tangent_closure`) is now
  (`isChained`). Designs: 1 extrusion → parametric, and the extrusion that
  follows it, which sat on the old base feature, is parametric.
- **Splitting a sweep with its target.** A profile whose edges are splines
  near a circle gave a splitting that missed the target's first crossing
  (the sweep ran through the body to the far wall); the splitting takes
  features within a micron (1e-3 mm) as one (`kSplitFuzzy`).
- **A revolution of a profile that straddles the axis line.** A face with
  an edge on the axis and a part beyond it was refused (`crosses the
  revolve axis`). It is parted at the line, its regions that touch united
  first (fusing the solids of regions that share a surface lost one of
  them, by the order of the arguments), and each part is swept on its own
  side. 1 revolution: the profile builds now; its result still differs
  from the file's (below).
- **A sketch used in a component placed several times.** A sketch of the
  parent component made the extrusion of a patterned component (an
  occurrence pattern follows it) land in the parent: the sketch copy took
  the parent's construction plane, and the placement of a component that is
  placed more than once was unknown. The copy takes planes of its own
  component and the first occurrence's placement
  (`Importer::original_placement_at`, and the links of an extrusion's
  objects). Designs: 2 extrusions → parametric (`read`).
- **The join pre-check** (the volume a prism can add) did not count the
  bodies a join merges, which the history keeps as they were: the check
  counts them (`feasible`). Designs: 1 extrusion → parametric.

Over the four designs (items; the queue run before, this round after):
parametric 133 → 142, fallback 15 → 7, partial 2 → 1 (the 13 items without
geometry of the largest design stay skipped). Left, with causes:

- A revolution about an edge line of a profile that straddles it: the
  result is 2 % larger than the file's (107173 against 104719 mm³); which
  input differs is not known (no API dump of this design).
- Fillets and chamfers the kernel fails (2 in one design, 1 pattern of
  fillets in another): see above.
- A pattern's time out, an extrusion of 12 profiles out of time and a
  split whose plane is not decoded, in the design with 91 items.
- Profile loops that name spline curves are not mapped to regions (`profile
  curve … is not a line, arc or circle`): the extrusions of such sketches
  are parametric but not `read`.

### A circuit board assembly with a part around it (model by model)

A design whose first 98 items place a circuit board brought in from an
electronics tool (20 components of parts, each its bodies in one file base
feature, the board's and the parts' 60 occurrences, most of them hidden),
then model a part around the board in the root component and place a
component of another document; an older version of it has the board only.
It opened with the parts scattered and everything shown:

- **Occurrences inside components placed several times** stood one copy's
  offset off (*Occurrence placements*): each part is a component holding
  one occurrence of the part's body component, and a part placed 20 times
  put all 20 bodies where the 20th copy should be (41 of 113 occurrence
  paths misplaced, 6 of them shown; none after).
- **Hidden occurrences were shown**: the light bulbs of occurrences were
  not read (*Light bulbs*): 51 of the 60 top-level occurrences, the board
  among them, are hidden in the file; all 96 occurrence paths whose
  visibility differed now agree.
- **The file's own base features** (bodies a tool brought in, without a
  history in the file) were fallbacks with a timeline warning each ("stands
  in for Base Feature1"). Such a base feature is its stored bodies: where
  the item's state changes its own component only and nothing waits
  before it, its base feature is its translation, `parametric` with its
  inputs `read`, and its feature gets no import note. A base feature whose
  state also brings other components' bodies stays a fallback.
- **Captured positions** read their positions from the file (its captured
  matrices, else the placements it stores): `read`, before `unknown`.
- **The final comparison** paired two parts' near-identical bodies across
  their components (a deviation of 0.05 mm reported for two exact bodies):
  a body of the file is matched in its own component first
  (`refs::compare_bodies`).

Before → after (items; the references folder in place; without it the same
outcomes, the other document's component empty): parametric 47 → 67,
fallback 22 → 2, skipped 81 → 81 (78 occurrence items, the component
insert and 2 suppressed extrusions); modelling inputs `read` 29 → 51,
`inferred` 2 → 2, `unknown` 2 → 0. The older version: 20 fallbacks → 20
parametric, 41 → 0 misplaced occurrence paths. Final bodies as before: all
exact.

Left, with causes:

- A revolution that cuts a taper groove with two adjacent profiles about a
  construction line (360°): its inputs agree with the API reference dump
  (profiles, axis, angle), but the cut removes 1077 mm³ where its tool's
  toroids hold 1050 mm³, the file's (the tool's faces lie on the body's,
  some candidates report `BRepCheck_BadOrientationOfSubshape`): a geometry
  kernel failure. The fillet after it names an edge of the revolution's
  result, which the stored bodies standing in for it no longer name, and
  falls back with it.
- The first sketch of the root lies on a face of the board, a body of
  another component. It stood on a fixed plane in the face's place, and
  its 166 projected edges of the board (and the projections of later
  sketches) were fixed reference geometry; since mitcad#225 it lies on the
  board's face through its occurrence, and 525 of the design's 537 linked
  curves are projections (499 of the board's and other components' edges,
  26 of the root's own), the other 12 fixed where no single edge of the
  replay projects onto them (*Sketches on other components' faces, linked
  curves as projections*). The board is a base feature, so nothing that
  could move it is lost.
- A suppressed extrusion up to a face: the decoder does not read the face
  of an extent up to an object, and a suppressed item has no history state
  to settle it; it stays a placeholder.

### Eight designs, third round (model by model)

Eight designs (four of them also stored in a second copy) that still had
items outside `parametric` and `read` were imported one by one (release
build) and every remaining item traced. Two rules that hold for any design:

- **Components made together.** One history state can hold several new
  components that a row of `ComponentFromBodies` items made (three
  items, three components, one state). The first item of the row moves
  the bodies into every component, one feature per component
  (`component_from_bodies`), and the next items of the row take the
  next components' features and notes (`Importer::made_ahead`); the
  state is matched once, with all the moved bodies and the retired
  copies of the owner's history. Before, such an item needed exactly one
  component in its history and the stored bodies stood in. Designs: 1
  item → parametric and its two siblings from skipped → parametric.
- **Linked curves the file keeps inconsistent.** The curves a sketch
  links to the model are locked in place, and tried free when their ends
  do not meet exactly. Some are inconsistent by a few hundredths of a
  millimetre among themselves (an arc whose end lies 0.024 mm off its
  circle; a short stub whose end lies on such a curve), so no solution
  moves less than 1e-4 mm and the sketch was skipped, and with it the
  extrusions of its regions. With the reference curves free the points of
  the linked curves, and those that constraints tie to them, may move up
  to 0.05 mm (`sketch::reference_points`, `position_error_slack`); every
  other point keeps the 1e-4 mm. The sketch keeps its constraints where it
  can (its note says the reference curves are not locked). Designs: 2
  sketches, and 2 extrusions that followed them.

Over the 8 designs (items; the queue run before, this round after; parametric
/ partial / fallback / skipped, then inputs read / inferred / unknown):

| design | before | after |
|---|---|---|
| a bearing | 19 / 0 / 6 / 3, 6 / 2 / 6 | 25 / 0 / 2 / 1, 10 / 1 / 6 |
| an end cap | 28 / 0 / 5 / 2, 16 / 0 / 1 | 32 / 0 / 3 / 0, 18 / 1 / 0 |
| a lamp holder | 43 / 3 / 5 / 2, 17 / 7 / 16 | 45 / 2 / 5 / 1, 35 / 3 / 3 |
| a pin holder | 13 / 1 / 9 / 1, 8 / 2 / 0 | 14 / 1 / 8 / 1, 9 / 1 / 1 |
| a tube end | 127 / 0 / 8 / 16, 68 / 7 / 16 | 127 / 0 / 8 / 16, 78 / 6 / 7 |
| a tube assembly | 88 / 0 / 9 / 13, 43 / 7 / 11 | 88 / 0 / 9 / 13, 52 / 6 / 3 |
| a pyramid side | 48 / 5 / 5 / 8, 18 / 5 / 7 | 53 / 2 / 3 / 8, 20 / 6 / 5 |
| a pyramid | 35 / 4 / 9 / 2, 12 / 3 / 6 | 40 / 1 / 8 / 1, 14 / 5 / 3 |

(Much of the change comes from rules merged since the queue run; the two
rules above account for the bearing's three component items and the end
cap's two sketches, two extrusions and their notes. The copies of the
designs give the same counts.)

Left, each with its cause:

- **Fillets and chamfers the kernel fails** (the rounding cannot end at a
  vertex, the radius may be too large, an invalid shape, a volume off, a
  result 1 % off; about 30 items): the decoded inputs agree with the API
  reference dumps where a dump exists.
- **Components of other documents** (a tube assembly: an extrusion up to a
  face of such a component, a cut and a mirror in its parts, joints on
  their origins).
- **Sketch lines in three dimensions** (a lamp holder: a sketch whose
  lines slope out of its plane and an extrusion and a plane taken from
  them): Mitcad's sketches are planar.
- **Stale profile records** (a pyramid: the profile curve of a revolution
  is no curve of its sketch any more; the bodies after it, cut and
  rounded on the stored one, make invalid wires in the boolean): no
  mapping from the old curve to the new one is stored.
- **Stale linked sketch constraints** (a pyramid side: an offset whose
  source curve is none of the sketch's).
- **Sketch text** (a bearing: an extrusion of text along a circle; the
  glyph regions have no font here) and a cut and paste of a body whose
  reference failed in the file itself (the file keeps the cached result).
- **A cut whose decoded profiles the boolean cannot build** (an end cap:
  the cut of two profiles lying within hundredths of a millimetre of the
  body's faces gives an invalid shape; the extrusion comes in `inferred`
  through another choice of regions, then the stored state): not
  diagnosed further; a retry with a fuzzy value of 1e-5 and 1e-4 mm did
  not build it.
- **A tool body the file's own history no longer holds** (a pin holder:
  two combines whose tool body is not in any state, the API reference
  dump lists no tool either) and a sketch on a face of the body of the
  base feature that no state keeps.
- **Inputs not decoded**: sweeps, pipes and lofts have no `read` rule
  (profile region, orientation and rail scaling are not stored in a
  decoded form), dressup edges named as an edge of a bounded face, a
  revolution whose profile has a stale record, joints, relationships and
  positions (captured positions are `read` since: *A circuit board
  assembly with a part around it*; the sweeps' and lofts' rules, the
  bounded faces' edges, the revolution's record and the joints' and
  relationships' provenance are in *Twenty-nine designs, third round,
  second batch*).

### Twenty-nine designs, third round, second batch (model by model)

Twenty-nine designs (one of them stored in both corpora) with items outside
`parametric` or `read` were imported one by one (release build; two at a
time) and each remaining item was traced. Rules that hold for any design:

- **Profile records of curves the sketch no longer has.** A profile older
  than the sketch's last edit can list curves the sketch has since
  replaced; its loops then carried a record without a curve and the
  selection was never exact. A record left out where the loop's other
  pieces close up without it (every end of a piece at the end of exactly
  one other, a whole circle on its own: `profiles::closes`) leaves the
  loop's inside what they enclose, as the original application reads it
  (its dumps list the closed loop's curves only). A loop that does not
  close without the record stays inexact. A sketch with one region has
  one choice: whatever the records say, the selection is that region and
  exact. Designs: 3 extrusions (one suppressed in the file) and 1
  revolution → `read`.
- **Edges of faces a set selects.** The decoder turns a face a dressup's
  set selects (`bounded_face` recipe) into its edges ("an edge of a
  face"); those edges are named by the file as the edges of an `edge`
  recipe are, and count as read. Designs: 1 chamfer → `read` (the rest of
  this kind came in earlier).
- **Edges the file names that its history no longer has.** An edge the
  decoder left unresolved because both faces are found with no edge
  between them, or because a face of it is gone from the history, is no
  input of the file's operation where the others resolve: the API
  reference dumps list the resolved edges only: of the 79 dressups of the
  dumps with edges the decoder left unresolved, 41 list exactly the edges
  that resolved (all of them for these two reasons), 22 list them all
  (suppressed items, and 6 of the second reason, where the decoder just
  did not find the face) and 16 differ (names that fit several edges).
  The definition without them comes
  before the history's guesses, noted ("3 edges the file no longer has
  left out") and `inferred`: the history checks it, a face the decoder
  just did not find fails the check and the guesses follow as before. A
  set of which none resolves is not reduced. Designs: 1 fillet by its
  names instead of by the history's lost edges.
- **A side through all of two sides.** The side through all of a
  two-sided extrusion (sides 4 and 1 in the stream) goes the way its flag
  says, as a one-sided extent does (`isPositiveDirection` of the dump:
  1 against the sketch's normal); it was the first choice only after the
  stored vector. The extent is read where the flag byte decides it.
  Designs: 1 extrusion → `read`.
- **Assembly items say where their inputs came from.** Rigid groups
  (members and placements the file's), joint origins (the first
  definition: the file's geometry, angle, offset and flip) and as-built
  joints (the file's recorded placement and motion frame, limits kept)
  are `read`; a joint is `read` where its sides' frames are found (not
  fixed), its offset is the stored one and its limits are kept, and
  `inferred` where it is not; a joint kept as an as-built one,
  a relationship set and a tangent (rows whose flip and offset sign the
  file's placements decide, sides found by their frames) are `inferred`.
  Before, none of them said. Designs: 2 rigid groups and 1 joint origin
  → `read`, 7 relationship sets `unknown` → `inferred`.
- **Sweeps** are `read` only with an exact profile selection (the decoded
  loops select the regions), like extrusions: a first set found by
  probing is not read. (Sweeps, pipes and lofts have `read` rules for the
  whole path, a pipe's section and a loft's one region per section; a
  sweep's partial extent has its direction undecoded.)

Over the 29 designs (items; the queue run before, this round after;
parametric / partial / fallback / skipped, then inputs read / inferred /
unknown):

| | before | after |
|---|---|---|
| items of the 29 designs | 894 / 10 / 102 / 140, 488 / 34 / 86 | 948 / 4 / 55 / 139, 605 / 45 / 6 |

Without the rules above (the same build without them, the merged earlier
branches only) the 29 designs stood at 925 / 4 / 78 / 139, 565 / 39 / 29:
the rules above account for read +17 and unknown −23 (the 7 relationship
sets went to `inferred`); the rest of the change comes from the rules
merged since the queue run (other batches and the components of other
documents).

Left, each with its cause (80 items outside `parametric`, apart from the
items that place occurrences and have no geometry):

- **Fillets and chamfers the kernel fails** (the rounding cannot end at a
  vertex, the radius may be too large for the edges, an invalid wire, a
  volume 0.4 to 138 % off, a rounding the watchdog gave up; about 45
  items): the decoded edges and radii agree with the API reference dumps
  where a dump exists. Some are radii as large as a rib is wide (two
  roundings of 1 mm on a rib 1.5 mm wide) or distances as large as a
  pocket's wall, where the kernel retries 0.1 % smaller (the feature's
  warning says so; the mirror of such a rounding then differs from the
  file's by about 1e-3 of the area: one design where it was traced).
  Others come after
  one of those (an edge of the stored face that a fallback left, an edge
  the replay does not have: `missing_in_replay`), or follow a fillet
  whose state the history shows as a variable blend the file stores
  without an approximation (`VBL_SURF`: the state cannot be rebuilt, so
  the items after it are not checked against it).
- **Components of other documents** (extrusions and cuts whose bodies are
  the other document's, a pattern direction taken from an edge of one, a
  cut and paste of its bodies, a derived context): another batch.
- **Stale profiles** (the dump itself cannot read the extrusion's profile:
  an extrusion after the sketch's edit that its history has as a cached
  result; a profile whose only record names a curve the sketch no longer
  has; piece counts of curves that overlap each other in the loop, where
  the file's count is not the one the crossings give).
- **Sketch text** (extrusions of a sketch's text: the profile list holds
  the glyph profiles without loops, and the font the file names is not
  installed here, so a text extrusion comes in `inferred` on Droid Sans
  or stays a fallback).
- **Planes without their reference** (9 tangent planes and planes at an
  angle, `inferred`: the file stores no plane the angle is measured from,
  the API lists none either, and the 13 tangent planes of the dumps tell
  no rule; 2 more are offsets of faces of components of other
  documents), a plane whose inputs the file itself cannot resolve and
  one whose stored geometry does not follow its angle (a fixed plane).
- **Mirrors and patterns whose copies the replay does not build the same
  way** (a mirror of features among which a fillet that comes in 0.1 %
  smaller; a circular pattern whose copies reach nothing after a
  fallback).
- **Time** (an extrusion of 12 profiles out of time).

### Census of 15.10.2026

The census of mitcad#178 again, on main `f085226d`: one coverage run of
both private corpora (release build, `--time-limit 300 --hang-limit 300
--jobs 6 --memory 6G`, the learning dump), items grouped as in *Census of
the non-parametric items (mitcad#178)* (the same selection and categories;
applied to #178's own reports the grouping gives its counts exactly).
Three designs ran out of the 6G per-file memory with the learning dump
(peaks of 9–11 GB when imported alone) and one child process crashed once
and did not again alone; all four were imported again alone and are
counted.

`MITCAD_F3D_CORPUS` is unchanged since #178. The backup corpus keeps its
325 designs and has 174 files more: the local copies of components of
other documents (mitcad#196), which the corpus run reads as designs of
their own; they are counted apart. Since #178 the file's own base
features (bodies the file keeps without a history) are counted parametric
instead of fallback: 23 and 125 items of the totals below.

Totals (parametric / partial / fallback / skipped):

| | `MITCAD_F3D_CORPUS` | backup corpus (325 designs) | reference copies (174) |
|---|---|---|---|
| #178 after | 1147 / 28 / 206 / 260 | 5249 / 120 / 923 / 1110 | – |
| 15.10.2026 | 1311 / 20 / 74 / 236 | 5947 / 76 / 357 / 1022 | 1786 / 38 / 86 / 365 |

Final bodies: 447 and 3010 exact, 12 and 81 within 0.1 %, none off (the
reference copies 2168 and 22). Fewer bodies are compared than in #178 (459
against 501, 3091 against 3147) on the same designs: a stored body of
several solids is now paired as one body, within its component.

Non-parametric items by category, both corpora, the files of #178:

| Category | #178 after | 15.10.2026 | reference copies |
|---|---|---|---|
| the import's time limit | 405 | 40 (30 of them one design) | 0 |
| reading and translation: fallbacks | 297 | 154 | 31 |
| geometry kernel: fillets, chamfers, booleans that fail or come out off the file's | 221 | 226 (fillets 206) | 30 |
| the file's own base features (where the state before them cannot be rebuilt) | 166 | 17 | 29 |
| construction planes as fixed planes | 40 | 11 | 3 |
| no translation in Mitcad (bodies of other designs 20, electronics boards 3, mirrored components 3, emboss, faces moved, Whitworth threads 2 each, a draft) | 93 | 33 | 44 |
| sketches partial | 53 | 36 | 13 |
| assembly positions (relationships, captured positions, as-built joints that do not hold) | 47 | 49 | 22 |
| after a fallback to the stored bodies | 18 | 0 | 0 |
| construction axes as fixed axes | 8 | 0 | 0 |
| references lost in the file itself | – | 11 | 2 |
| all | 1348 | 577 | 174 |

Items suppressed or rolled back in the file whose definition did not come
in (mitcad#180) are left out as in #178: 64 (fillets most of them).

The geometry kernel is now the largest group (39 % of the items): fillets
that fail in OCCT's builder (94 items in 56 designs with the builder's
generic failure, 57 in 37 at a vertex, 29 with invalid or off results).
Fillet fallbacks went from 361 to 261, but fillets that the time limit or
an earlier fallback hid before now reach the kernel. Of the 32 extrusions
and revolutions whose result is not in the history or cuts nothing, 17
follow an earlier fallback in their component, 15 have a complete sketch
and bodies that are the file's (single cases), one a sketch on a fixed
plane. Input provenance of the accepted candidates (mitcad#186), both
corpora: 4668 read, 332 inferred, 113 unknown.

## FreeCAD import (.FCStd)

A FreeCAD document is a zip with `Document.xml` (every object and its
properties), `GuiDocument.xml` (visibility, colours) and the OCCT B-rep of
every shape-bearing object's result as FreeCAD last computed it, in its
container's coordinates. `core/freecad` reads the archive and XML
(`roxmltree`, DTDs refused) into typed objects without a kernel.
Enumerations are stored as indices, so it carries each FreeCAD version's
value lists (`data/enums.json`, read from FreeCAD by
`tools/freecad-export/enums.py`).

Entry points: `mitcad-cli import-fcstd <file.FCStd> [--save out.mitcad]
[--report report.json] [--bodies-only] [--reference dump.json] [--set
name=expression]... [--json]`; the `import_fcstd` command, run with OCCT by
`core/ffi/src/fcstd_import.rs`. The application runs it in the import
process, like the `.f3d` import.

### Stage 1: structure and stored bodies

`src/freecad/mod.rs`: App::Part and Assembly → components; Bodies and the
Part workbench's leaf objects → base features of the stored shapes (one
per component); App::Link → occurrences (arrays, links of links, links to
other files read next to it); visibility, colours and the document's unit
system. Placements come from the file, not from attachments. Datums and
joints are left out; features inside Bodies and consumed operands are part
of their results. The report says why for each object. `--bodies-only`
stops here.

### Stage 2: sketches

`core/freecad/src/sketch.rs` reads geometry, constraints and external
geometry; `src/freecad/sketch.rs` translates, `sketches.rs` places and
checks.

- FreeCAD's curves own their points and Coincident constraints join them;
  joined points become one Mitcad point (union-find), as in the `.f3d`
  import.
- Hyperbolas and parabolas become exact rational splines; a B-spline's
  control-point circles become its control points; an ellipse's axis
  lines stay construction lines held on it.
- The plane is an origin plane, a face of an imported body
  (`<feature>:import(j)`) or a fixed construction plane; the definition's
  `frame` keeps FreeCAD's exact placement.
- External geometry on imported bodies becomes linked projections by
  element name. The kernel's `indexed_element` gives a shape's face, edge
  or vertex by FreeCAD's index (OCCT `TopExp::MapShapes` order); every link
  is checked against the same element of the referenced object's stored
  shape.
- A sketch is accepted when Mitcad's solution keeps FreeCAD's points
  within 1e-4 mm (else with driven dimensions, else without constraints)
  and is compared with its stored shape. What Mitcad cannot express
  (tangency with ellipses, Snell's law, distances to circles,
  intermediate features' elements) is listed, and the sketch is
  `partial`.

### Stage 3: history

`history.rs` drives it, `features.rs` translates PartDesign features,
`part.rs` Part workbench objects, `primitives.rs` objects without a Mitcad
feature of their own, `elements.rs` references.

- Every PartDesign Body and every Part workbench result stage 1 made a
  body of (extrusion or revolution of a sketch, primitive, operation) is
  rebuilt as Mitcad features, with the Bodies its booleans use and the
  operands of its tree. All objects, sketches included, come in
  dependency order (ties in document order).
- The file keeps every feature's own result, so each replayed feature is
  checked against FreeCAD's stored shape: volume, area and centre of mass
  of the Body's Mitcad bodies within 1e-6 relative, and the number of
  solids. Where FreeCAD's conventions are uncertain a few readings are
  tried (directions, taper signs, a chamfer's first face, which sketch
  regions are material).
- A feature that cannot be translated, fails, or does not give FreeCAD's
  shape becomes a base feature of its stored shape that replaces the
  Body's body (`replaces`, keeping its id); later features continue on
  it. One whose stored shape equals its base's is skipped.
- References (`Pad.Face6`, `Edge12`) are taken from the referenced
  object's stored shape and found by geometry in the Mitcad body holding
  its result — first as it is now, else as it was right after the object
  (the name then carries through later features). An edge FreeCAD merged
  is all of Mitcad's pieces along it.
- A Body's tip before its last feature suppresses the features after it.
- Bodies are named and shown as in FreeCAD; colours are not kept (Mitcad's
  appearances are a library).
- `primitives.rs` builds: a cone, a prism, a part of a turn as a section
  sketch on a fixed plane turned or extruded (a cone's and a partial
  cylinder's sizes are dimensions of that sketch); a wedge as a ruled
  loft; an ellipsoid (or a part of one: its section turned, then scaled)
  as a scaled sphere; a pad along a slanted direction as a sweep along a
  fixed line; a MultiTransform as a chain of patterns and mirrors, each
  one a pattern or mirror of the one before (the model's patterns of
  patterns); a linear or polar pattern followed by a Scaled transformation
  of as many occurrences as that pattern with `scale` (its copies scaled
  about the original's centre of mass, mitcad#59). The model's `helix`
  feature and the hole's counterdrill and taper exist for FreeCAD's
  helices and holes.
- Conical and growing helices are helices with a `growth` per turn
  (`Pitch * tan(Angle)`, or Growth) and `"construction": "freecad"`
  (mitcad#83; helices made in Mitcad use Mitcad's own construction),
  built as FreeCAD 1.0 and 1.1 build them (the profile follows the Frenet
  frame of a spiral a hundred times as far from the axis): they match
  those versions' stored shapes to 1e-11. FreeCAD 0.21 built them along an auxiliary spine; its conical
  ones still match (1.8e-7), its growing ones are 1e-6 off and its
  reversed narrowing and left-handed growing ones are other shapes
  (5.9e-3, and 0.21's left-handed growing helix holds 518 mm³ where 1.0
  and 1.1 hold 429), so those fall back. A subtractive helix cutting
  outside the profile (Outside) is a helix that intersects.
- A threaded hole is a hole plus a cosmetic thread (ISO metric, Unified,
  Whitworth incl. BSP `G 1/4`, NPT, tyre valve threads of ISO 4570). A
  modelled ISO metric or Unified thread is first rebuilt as FreeCAD cuts
  it (its `makeThread`): the groove's section (60° flanks, the root P/8
  wide at the major diameter plus the class's clearance) on a plane
  through the axis, swept by a `helix` from a pitch above the top as deep
  as FreeCAD's thread runs, then the hole; then as a modelled `thread`
  (#57's profile with the bore as the minor diameter).
- Datum planes: attached flat with only a normal offset → `offset`
  plane; attached to an origin plane and turned a quarter (or not), with
  the offset along its normal → `offset` from the parallel origin plane;
  turned about the support's x or y axis → `angle` plane; otherwise
  `fixed`.
- Sketches and datum planes whose attachment turn an expression drives
  about a line in their support's plane (its x or y axis or any line
  between) become `angle` planes about that line.

### Stage 4: parameters and expressions

`params.rs`; `core/freecad/src/expression.rs` parses FreeCAD expressions
into a tree whose references the import resolves; `spreadsheet.rs` reads
cells.

- Before the timeline, user parameters are made from spreadsheets'
  aliased cells (and cells expressions name by address), VarSets'
  properties, sketches' named constraints, and feature properties other
  expressions refer to — in dependency order, with FreeCAD's names where
  Mitcad accepts them.
- Every expression bound to a value the import carries over (a feature's
  length or count, a sketch's dimension, an attachment offset along a
  normal as an offset construction plane, a sketch or datum plane turned
  about its support's x or y axis as an `angle` construction plane, a
  side offset that moves a quarter-turned plane along its normal as an
  `offset` from the parallel origin plane) becomes that value's Mitcad
  expression.
- The translation maps references, units, constants, the conditional and
  functions (written out where Mitcad has no equivalent) and keeps
  FreeCAD's precedence. An integer property's expression is rounded as
  FreeCAD rounds it (`round(Count / 2)`).
- Each translated expression is evaluated and compared with FreeCAD's
  stored value; one that differs or does not translate keeps FreeCAD's
  value, with the reason in the report. Not translatable: a shape's
  measure (e.g. bounding box), side offsets that only move a sketch or
  plane within itself (kept as numbers), turns about other axes or with
  shifts (a fixed plane). Cells of quantities
  Mitcad lacks (e.g. kilograms) are left out.
- Expressions on objects kept as stored shapes (e.g. BIM objects) stay
  unused.

### Not translated (fallback)

MultiTransforms of the whole body, and Scaled transformations other than
after one linear or polar pattern of as many occurrences; profiles that
are faces or chosen parts of a sketch; revolutions up to the last face;
tapers along custom directions; pipes with an auxiliary spine or binormal
mode; Part extrusions of faces and Draft objects; Part fillets of varying
radius; binders; Draft and Python objects (`Part::Part2DObjectPython`).

Translated, but measured off FreeCAD's stored shapes (mitcad#59; the
check's limit is 1e-6, the reference models `pd_helix_more`,
`pd_helix_outside`, `pd_hole_modelled`, `expr_datum_turns`):

| Case | 0.21.2 | 1.0.2 | 1.1.4 | Why |
|---|---|---|---|---|
| Conical helix | 1.8e-7 ✓ | 2e-14 ✓ | 2e-14 ✓ | |
| Growing helix | 1.4e-6 | 5e-14 ✓ | 5e-14 ✓ | 0.21 builds it along an auxiliary spine; its centre is 1.5e-6 of the size from 1.0's |
| Reversed narrowing, left-handed growing helices | 5.9e-3, 1.9 | ✓ | ✓ | 0.21's are other shapes (its left-handed growing helix holds a fifth more) |
| Helix cut outside the profile | 3.4e-6, 2.9e-4 | 1.4e-4, 1.2e-4 | 1.4e-4, 1.2e-4 | FreeCAD refines the intersection (Refine): its stored shape lies 1e-5 of the volume and 1e-4 of the size in the centre from the plain intersection of its own helix and support (measured in FreeCAD 1.1); without Refine the conical one matches (9.9e-7) |
| Modelled ISO metric thread (M6, blind) | 1.3e-6 | FreeCAD fails it | 1.3e-6 | The groove's flanks: FreeCAD sweeps the section by the Frenet frame along a helix approximated turn by turn, Mitcad's helix keeps the axis as its binormal (the area differs by 1.3e-6; volumes agree to 3e-7) |
| Modelled Unified thread (1/4 UNC, through) | 2.0e-6 | 1.9e-6 | 2.0e-6 | As above |
| Modelled tyre valve, Whitworth, NPT threads | | | | FreeCAD rounds the tyre valve crests and cuts Whitworth's 55° profile; Mitcad's modelled thread is the straight 60° one, and does not taper |
| Attachment turns about an axis out of the support's plane, or with a shift | | | | Mitcad's planes turn about lines in them (an `angle` plane); a turn that also turns the plane within itself has no parametric plane: a fixed plane, FreeCAD's value kept |

Tyre valve threads exist from FreeCAD 1.1; their holes import parametric.

FreeCAD itself fails a revolution up to the first face, a groove up to a
face, and (0.21, 1.0) a draft pulled along an origin axis; the reference
models leave these out.

### Tests

Documents are kept outside the repository (`MITCAD_FCSTD_CORPUS`).
`tools/freecad-export` makes reference models and dumps with FreeCAD 0.21,
1.0 and 1.1 in the isolated distro
([development.md](../../docs/development.md#freecad-reference-models-and-corpus)).
The ctest `freecad.corpus` (`tools/cli/CMakeLists.txt`) imports every
document and compares those with a dump; for models with a variant it
changes a parameter in Mitcad (`import-fcstd --set`) and compares with the
variant FreeCAD recomputed after the same change. Each document is checked
in a child process of its own, several at once (`mitcad_run_parallel`;
`MITCAD_CORPUS_JOBS`, `MITCAD_CORPUS_MEMORY`, `MITCAD_CORPUS_TIMEOUT` as
for the `.f3d` corpus; `MITCAD_CORPUS_JOBS=1` checks them one after
another), and the lines come in the documents' order. A full run for
numbers before and after a change:
`MITCAD_CORPUS_JOBS=8 ctest --test-dir build/dev -R freecad.corpus -V`,
or with a release build's CLI
`cmake -DCLI=build/rel/tools/cli/mitcad-cli -DRUNNER=build/rel/core/mitcad_run_parallel -DWORK=build/fcstd-corpus -P tools/cli/fcstd-corpus.cmake`.
Some models need FreeCAD
1.0 (VarSets, extents up to shapes) or 1.1 (Whitworth and NPT threads). Ellipsoids and skewed cylinders are
B-spline and extrusion surfaces that FreeCAD's own measures integrate less
exactly (up to 4e-4) than Mitcad's; the reference check does not compare
those. Nor does it compare the measures of shapes brought in as FreeCAD
stored them where FreeCAD's are those of OCCT's plain fixed Gauss points
(`Kernel::fixed_point_properties`, `placed[].fixed`): Mitcad integrates
faces bounded by B-splines of many spans more exactly (mitcad#139; 2e-6 of
the volume on the hydraulic cylinders of the assembly example).

## .ipt import

mitcad#60. The code is not in this crate: [core/ipt](../ipt/README.md)
reads the file (compound file, property sets, segment database, the B-rep
record, the definitions segment) and decodes the part's design into this
crate's dump IR (`mitcad_ipt::design`); `core/ffi/src/ipt_import.rs` runs
the import and `core/ffi/src/ipt_history.rs` gives the bodies of the ASM
history the B-rep record carries (`StoredGeometry`), state by state, as
the `.f3d` import's `F3dGeometry` does for the `.smbh` blobs. Entry
points: `mitcad-cli import-ipt part.ipt [--save part.mitcad] [--report
report.json] [--reference part.stp [--max-relative X] [--deviation]]
[--bodies-only] [--no-verify] [--no-fallback] [--no-compare] [--time-limit
S] [--hang-limit S] [--dump design.json] [--design design.json] [--json]`, the `import_ipt`
command ([commands.md](../model/src/api/commands.md#ipt-import)) and File ›
Open or Import in the application (the import process, as for `.f3d` and
FreeCAD documents; Cancel only).

- **With the history** (stages 2 and 3, the default): `import_design`
  replays the dump as it replays an `.f3d` design: the part's parameters
  with their expressions (`params.rs`), sketches, work planes and
  features, each checked against the state of the ASM history its
  operation made (the definitions segment's state table names it), with
  the fallbacks, the guesses the history settles (profiles, fillet and
  chamfer edges, hole positions, mirror planes) and the final comparison
  of this crate. The items are those of the browser, in its order.
  With `Options::validate` a definition that gives an item's state but
  leaves a body OCCT's checker finds invalid is not taken: the item takes
  the file's bodies (a cut checked only near its change made a part's
  body invalid).
  An item whose state the history does not hold, between states it does
  (a feature suppressed in the file, `StoredGeometry::item_without_result`),
  is skipped as one the file keeps no result for, as an `.f3d` item with
  result number -1 is: before, its definitions were tried against the
  states up to the next item's, and it was skipped or taken into a later
  fallback, often with a reason that did not apply (*its result is not in
  the file's history*, *the extrusion does not cut into any participant
  body*, *its edges were not decoded …*).
  A stored body of several lumps (disjoint solids in one body, as a join
  of a piece that touches nothing makes it in the file) comes in as one
  body per lump in the history states and the stored design, as the
  replay makes them (Mitcad keeps disjoint solids apart); before, such a
  join's replay had one body more than its state and fell back. With `hang_limit` (`--hang-limit`; the application 90 s,
  `ipt.corpus` 300 s) the replay runs on its own thread, watched as the
  `.f3d` import's (*Hangs* above): an item whose kernel call does not
  return (a fuse of two revolved regions that never ended) takes the
  file's bodies in the next try.
- **Bodies only** (`--bodies-only`, and files without a definitions
  segment): one base feature per body, solids and sheet bodies alike; a
  body that OCCT cannot build is reported (`skipped`) and left out.
- The document's length unit becomes the part's (the model settings'
  unit code); the material is set when Mitcad's library has it.
- The report lists the part number, material, units, the release that
  saved the file, each B-rep record (ASM version, bodies, the states of
  the ASM history it carries) and each body (solid or sheet, valid,
  volume, area, faces); with the history also the parameters (records,
  model, user, those not the part's), the expressions (translated,
  agreeing with the stored values), the features and the replay's report.
- Assemblies (`.iam`, stage 4) are an import of their own, which imports
  each part this way (*.iam import* below).
- Not yet: body names and colours, files whose B-rep record is not an ASM
  file (older releases); what the
  definitions segment does not tell (core/ipt/README.md, *What the files
  do not tell*) comes in as the bodies of the history states.
- A hole whose decoded position does not give its state also tries the
  history's guess (each piece of material the state removes); that guess
  cuts the replay's body with the state's, and when the two are nearly
  coincident (a body a fallback took to the previous state) OCCT's boolean
  can take minutes (two minutes for one hole of the test files): the item
  then has no time left for its definitions and falls back.

### Corpus test

Real part files are kept outside the repository (`MITCAD_IPT_CORPUS`, a
path list of folders). The ctest `ipt.corpus`
([tools/cli/ipt-corpus.cmake](../../tools/cli/ipt-corpus.cmake)) imports
every `.ipt` with `mitcad-cli import-ipt` (with the history), each in a
child process of its own, several at once (`mitcad_run_parallel`;
`MITCAD_CORPUS_JOBS`, `MITCAD_CORPUS_MEMORY`, `MITCAD_CORPUS_TIMEOUT`,
`MITCAD_CORPUS_JOBS=1` checks them in the script's process). Every body
must be built and valid, every expression of the part's parameters must
agree with its stored value (in the file and in Mitcad), and the final
bodies must be the file's; the line per file gives the features' outcomes.
A file with a STEP file of the same part next to it (`<name>.stp` or
`.step`) is compared with it: as many solids, each one's volume and area
within 1e-6 relative, unless the folder's `references.tsv` gives the file
a limit of its own (`<name>.ipt`, a tab, the limit, a tab, why). Such a
limit is for a reference that was not exported from the file and models
the part a little differently: it is the difference measured, rounded up
by a few per cent, so that a change in what the import reads still fails.
`core/ipt/tests/corpus.rs` checks the same files without OCCT: the B-rep
records split out of their segments and every body converts cleanly, the
parameters and their expressions, the features' history states.

The test corpus: 16 public NIST MBE PMI test parts
(11 saved by a 2021 release, ASM 226 and one 227; 5 by a 2024 release, ASM
229), each with STEP files of the same test case that the institute
published separately, made from other CAD systems' models of the case
(sources, terms and checksums: the test-file repository's `ipt/README.md`).
Results (dev build, 2026-10-08):

- Every file reads; every B-rep record splits out of its segment; all 22
  bodies (16 solids, 6 one-face sheets) convert without issues and are
  valid for OCCT's checker.
- The STEP references: one part matches within 1e-6 (3.3e-7). The others
  differ by 5e-4 to 4e-2 in volume or area, because the references model
  the parts differently (different face counts: 270 against 255 faces,
  6 against 8; one plate 3.0 mm thick in the reference, 3.02 mm in the
  part); no published STEP file was exported from these part files (each
  part was compared with all of its case's STEP files, AP203 and AP242:
  none matched better). Their limits are in `references.tsv`.
- Three parts saved by both releases (two saves made independently, ASM
  226 and 229) give the same solids: volume and area within 5e-7 through
  a STEP round trip, 3e-9 between the build reports; two parts were
  changed between the saves.
- With the history (2026-10-09): 2,785 parameters with their expressions,
  all agreeing; 344 features with their history states; of 607 timeline
  items 450 parametric, 53 partial, 92 fallback and 12 skipped (per kind:
  [core/ipt/README.md](../ipt/README.md#results-on-the-test-files)); the
  final bodies are the stored ones in every part.
- Import times: bodies only 0.12 to 0.83 s per file and 45 to 63 MB (dev
  build, which optimises dependencies); with the history 0.2 s to 3 min
  (most under 20 s; the slowest has the hole above). The corpus test
  takes about 3 minutes with 6 files at once.

## .iam import

mitcad#60, stage 4. [core/ipt](../ipt/README.md#assemblies) reads the
assembly (referenced files, occurrences, placements, the range boxes and
display transforms the file stores); `core/ffi/src/iam_import.rs` makes
Mitcad's components and occurrences of them and imports each part with
the `.ipt` import above, in a document of its own: its bodies become a
base feature of a component, or with `--history` its replayed design is
copied into one (`Document::add_component_copy`). Entry points: `mitcad-cli
import-iam assembly.iam [--save out.mitcad] [--report report.json]
[--search folder]... [--history [--no-verify] [--no-fallback]
[--no-compare] [--time-limit S]] [--json]` (`import-ipt` takes an `.iam`
too), the `import_iam` command
([commands.md](../model/src/api/commands.md#iam-import)) and File › Open
or Import in the application (the import process, as for parts).

- Each part file comes in once, its stored bodies as base features (with
  `--history` its features as the `.ipt` import replays them), and each
  occurrence is an occurrence of its component;
  sub-assemblies are components with their own occurrences.
- Referenced files are found as saved, relative to the assembly as it was
  saved, by the saved path's tail, or by name in the search folders; a
  part found so is checked to be the referenced document by the version
  id both files store. Files that are not found are empty components,
  placed, and reported.
- Checks against the file: each placement against the transform the file
  displays the occurrence with, each part's bodies within the range box
  the file stores for it (parts saved again after the assembly only
  reported).
- Not yet: constraints and joints (the stored placements are taken as
  they are), design views and positional representations, the model state
  an occurrence uses (the part comes in in its active state).

### .iam corpus test

`iam.corpus` ([tools/cli/iam-corpus.cmake](../../tools/cli/iam-corpus.cmake))
imports every `.iam` under `MITCAD_IPT_CORPUS` with `mitcad-cli
import-iam`, the corpus folders searched by name, each in a child process
of its own, several at once (as `ipt.corpus`). Every occurrence that is
not suppressed must be placed, every part file found must import, every
placement must agree with the displayed one and every part's bodies must
lie within their stored range box; files that are not in the corpus
(library parts, files of other projects) are reported, not failed.

Results (dev build, 2026-10-09) on a private project tree of 166
assemblies: see [core/ipt/README.md](../ipt/README.md#results-on-the-assemblies).
