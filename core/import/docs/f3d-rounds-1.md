# The .f3d import: notes on single rounds, part 1

Notes on single rounds of work on the corpus, kept as the record of what
was found and decided. Part 1: the census of the non-parametric items and
the model-by-model rounds. Part of the [.f3d import](f3d.md) notes; the
[import documents](../README.md#documents) list the other topics.

## Census of the non-parametric items (mitcad#178)

What still does not come in parametric, by cause, so that decoding work
goes where it moves the most items. One coverage run of both private
corpora (release build, three files at a time, with the learning dump;
[*Coverage runs in parallel*](../README.md#coverage-runs-in-parallel)); every item whose outcome is a fallback,
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

## Items that fell back after the census: extrusions, patterns, planes, sketches

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

## Datums and extrusions that name other components (model by model)

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

## Designs with several remaining problems (model by model, second round)

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
  mitcad#25 ([*Primitives, coils and option constants*](f3d-learning.md#primitives-coils-and-option-constants-mitcad25)).
- **Extrusions whose profile records name curves the sketch no longer has**
  (stale selections, `inferred`): the regions come from the history.
- **Where a circle starts (its seam)** is read now (see "Where a circle
  starts" under the input decoding audit).

## Profile selections: ellipses, splines and stale holes (model by model)

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

## A large assembly design and its sibling (model by model, third round)

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

## Face profiles and extents up to objects on two sides (model by model)

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

## Four designs whose remaining items had geometric causes (model by model, third round)

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

## A circuit board assembly with a part around it (model by model)

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
  not read ([*Light bulbs*](f3d-appearances.md#light-bulbs)): 51 of the 60 top-level occurrences, the board
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

## Eight designs, third round (model by model)

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

## Twenty-nine designs, third round, second batch (model by model)

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

## Census of 15.10.2026

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

