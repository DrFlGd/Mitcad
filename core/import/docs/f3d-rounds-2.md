# The .f3d import: notes on single rounds, part 2

Notes on single rounds of work on the corpus. Part 2: fillets and chamfers
that fail (mitcad#322 to mitcad#343, up to round 6). Part of the [.f3d
import](f3d.md) notes; the [import documents](../README.md#documents) list
the other topics.

## Fillets that fail along their edges (mitcad#322)

The census's largest cause, "fillet failed; the radius may be too large
for the edges", was the text the dressup gave for any fillet OCCT did not
build where no vertex was faulty. OCCT's fillet now tells where it stopped
(port patch 0320, `ChFi3d_Builder::Failure`), and the dressup names it
(`geometry/src/dressup.cpp`, `fillet_failure`), with the first edge of the
contour it stopped at:

| Where OCCT stops | The import's note (after "fillet failed; ") |
|---|---|
| no start section | the geometry kernel finds no start for the rounding along *edge*: no ball of the radius touches both faces there |
| the walk between the two faces | the rounding along *edge* stops part of the way: the geometry kernel's walk between the two faces fails |
| the walk rolling on an edge | the rounding along *edge* stops part of the way where it rolls on an edge of a neighbouring face |
| a twisted surface | the rounding's surface along *edge* twists: the radius is larger than a face's curvature there |
| an exception in the rounding's computation | the geometry kernel's computation of the rounding along *edge* fails |
| roundings overlapping on a face, computed again across it | the roundings of two edges overlap on the face between them (one along *edge*), and the geometry kernel cannot run them across that face |
| contacts crossing on a face, computed again | the contacts of two roundings cross on a face (one along *edge*), and the geometry kernel cannot cut them by each other |
| the check that two contours' roundings do not intersect | the rounding along *edge* and the rounding of another edge intersect each other |
| a corner | the geometry kernel cannot end or join the rounding at a vertex: *which corner and why* (mitcad#323, *Fillets that cannot end or join at a vertex* below) |
| a corner built in part | the geometry kernel builds the corner at a vertex where the roundings meet only in part |

*Edge* is "edge E{…} (and N more edges of its chain)". The texts of the
failures along edges name no vertex, as before (the import retries those
with more of the chain's edges).

The items were found again with one coverage run of both private corpora
on main `46d8eff3` (the census's code; 95 instances in 53 designs with the
text, the census counted 94 in 56; the reference copies add 14). Each was
replayed from the failure dump (`MITCAD_IMPORT_DRESSUPS`: the input body,
the definition, the stored bodies of the history's states before and after
it) through OCCT's fillet alone with a trace of where it stops, and on the
history's stored body before the item, its selected edges found there by
their geometry (within 0.01 mm; pieces where the two split an edge
otherwise), through the dressup with all its tries and OCCT's fillet alone.
Built on the stored body means the file's change of volume within 1 %.
Content ids are those of the files; a design stored in several files counts
once per file.

| Class | Items / 95 | Builds on stored / on replay | Designs (content id, item) | Cause |
|---|---|---|---|---|
| upstream: an earlier dressup built 0.1 % smaller leaves a sliver | 7 | 7 / 0 | `7b638e0c` 40, 134; `e79e1d36` 40; `a9e82a9a`, `de7b8b9d`, `fc4f1709` 8; `7aa27c24` 7 | at its exact size the earlier rounding or chamfer consumes a face, which OCCT did not build, so it was built 0.1 % smaller and left a strip of 0.5 to 1.5 µm where the file has none; the rounding then runs on the strip's edge (no start, or the walk's approximation fails). A slot's rim rounded as wide as its ledge (3 items): fixed, port patch 0321. A concave chamfer as wide as a ring sector beside a rounded rim (3): OCCT builds it, but with an unorientable face; follow-up. A pocket floor's chamfer taking both its faces (1): mitcad#300 |
| upstream: after an earlier fallback in the component | 7 | 7 / 0 | `5d6ee474`, `81beaf7b` 76; `66684457` 77; `0aed68ff` 79; `db73863f` 106; `3a5fe9db` 13; `e4424061` 6 | the body is the state's within the tolerance (0 to 3e-2 mm³ apart), so no retry replaces it, but its faces are split otherwise around the edges (stored faces of an earlier fallback, or the replay's own split): the walk between the two faces fails on the replay's faces and builds on the stored ones; two of them (`3a5fe9db`, `e4424061`) are mitcad#300's |
| upstream: the replay's body differs where the edges are | 4 | 0 / 0 | `480e591c`, `d498ea9d`, `eecf90bd` 63; `4365a039` 12 | the stored body has no edges where the replay's are (in three, the bodies 0.76 mm³ apart, quarter circles of radius 0.5 on the replay only; in one, an edge of the selection); the earlier item that makes them differ is not traced |
| builds on the stored body, 6.6 % off the file's change | 3 | 0 / 0 | `480e591c`, `d498ea9d`, `eecf90bd` 39 | the replay has 54 faces where the file has 46; on the stored body OCCT's result is invalid and the dressup's 6.6 % off |
| kernel: the walk between two faces stops | 22 | 0 / 0 | e.g. `98a29231` 23, `64052a50` 41, `7dab3af4` 26, 29, `5d6ee474` 88, `a9e82a9a` 29 | 18 fail the same way on both bodies, 14 of them with a face of the edge narrower than the radius (the edge overflow's class, mitcad#121, mitcad#238's bridge); 4 stop at a corner on one of the two bodies |
| kernel: the walk rolling on an edge stops | 18 | 0 / 0 | `818fa4e2`, `48ff426e`, `c6cf5a22` 73; `cfab5b0a`, `e3c34193` 145; `a9e82a9a`, `de7b8b9d` 137, 191; `64bd9e0e`, `ff3baa6d` 7, 8; `5ef2dbd5` 90; `61526c66` 13; `9f94df34` 11; `db73863f` 137; `66684457` 75 | every one has a face of the edge narrower than the radius: the ball rolls on that face's far edge (mitcad#237's class); e.g. an edge of 0.66 mm between a triangle of 0.5 mm² and a large face rounded with 4, a radius of 129 on an edge whose faces are 36 mm wide |
| kernel: an exception in a rounding's computation | 14 | 0 / 0 | `5ef2dbd5` 88, 93; `99ef321f` 23 (twice); `c23a90e7` 12; `6eccefc2` 11; `9c3751cd` 65 (three times); `b596234a` 65; `236b88c6` 7; `61526c66`, `cb955714` 5; `db73863f` 121 | a rational section's weights too small (5, mitcad#239's `GetMinimalWeights`), a trimmed curve of zero length (4, one part), the walked blend's approximation (2), "chaining is impossible" (2), a common point's vector (1) |
| kernel: roundings overlapping on a face fail when computed again | 5 | 0 / 0 | `480e591c`, `d498ea9d`, `eecf90bd` 32; `2d12d69c`, `52ba7a15` 37 | the overflow across the face between the edges (0013) fails rolling on its edge |
| kernel: a twisted surface | 3 | 0 / 0 | `24523ade`, `98a29231` 33; `5ef2dbd5` 38 | on the stored body OCCT builds an invalid shape (an edge's range) or stops at a corner |
| kernel: two contours' roundings intersect | 2 | 0 / 0 | `6418ebb1`, `920a1cef` 37 | "fillets have too big radiuses" (`ChFi3d_StripeEdgeInter`, mitcad#239's crossing contacts) |
| kernel: no start on both | 1 | 0 / 0 | `2d12d69c` 8 | |
| kernel: a stripe given up without a status | 1 | 0 / 0 | `0b758b01` 155 | |
| the note is another definition's | 3 | 0 / 0 | `480e591c`, `d498ea9d`, `eecf90bd` 60 | the definition by the edges' names gives an unorientable face on the replay's body (125 faces; the stored body has 120) and fails at a corner on the stored body |
| not in the dump | 5 | – | `408947b1` 8; `ad0df696` 10; `e75ed14e` 10; `0b758b01` 144, 236 | three whose state in the history cannot be rebuilt (nothing to try on), two whose failing definition is not one fillet definition |

Upstream noise is 21 of the 95 (14 build on the stored body with the
file's change, none on the replay's); kernel gaps 66 (none builds on
either body); 8 are not classified further. No class
but the slivers comes from one cause that a change of the import or one
patch would fix.

Fixed: the slot's rim rounded exactly as wide as its ledge (port patch
0321). Imported again with 0321, the 53 designs (and the reference copies
next to them): only two change, `7b638e0c` 129 → 129 of 140 items
parametric (items 40 and 134 come in; Chamfer4, item 102, now fails and the
thread after it, item 114, comes in 0.7 % off: the exact roundings before it
leave the replay's body one face more than the stored body where a boss of
radius 12 touches a wall tangentially, with edges of zero length, and the
chamfer of the ring beside the boss, as wide as the ring, fails in OCCT's
walk there; on the stored body OCCT builds it) and `e79e1d36` 71 → 72 of 76;
the final bodies as before (all within 2e-7 of the file's). In
`7b638e0c` the three dressups that were built 0.1 % smaller are now built
at their size.

Follow-ups:

| Class | Items | What is needed |
|---|---|---|
| concave chamfer as wide as a ring sector beside a rounded rim | 3 | done: port patch 0352 (mitcad#300) builds it at its size, and the rounding after it comes in (mitcad#327, below) |
| after an earlier fallback, faces split otherwise | 7 (2 in mitcad#300) | the replay's faces where the file has one (the mitcad#284 kind): find the earlier item of each that splits them |
| the replay's body differs where the edges are | 4 | trace the earlier item that adds the replay's rounded quarter circles |
| a boss touching a wall tangentially: edges of zero length | 1 (the chamfer above) | the boolean that leaves them, against the stored body's 149 faces |
| walks between two faces and rolling on an edge past a face narrower than the radius | 40 | the edge overflow's constructions (mitcad#121, #237, #238), one shape at a time |
| exceptions in a rounding's computation | 14 | weights of rational sections (mitcad#239), zero-length trimmed curves, approximations |

## Fillets that cannot end or join at a vertex (mitcad#323)

The census's second cause, "fillet failed; the geometry kernel cannot end
or join the rounding at a vertex", was the text for any fillet OCCT did not
build because of a corner. OCCT's fillet now tells, for each faulty vertex,
which corner it tried there and the exception that stopped it (port patch
0330, `ChFi3d_Builder::FailedCorner`), and the dressup names both
(`geometry/src/dressup.cpp`, `corner_failure`), for the first faulty
vertex, "(and at N more vertices)" after it:

| Corner (`PerformFilletOnVertex`) | The import's note (after "fillet failed; the geometry kernel ") |
|---|---|
| a rounding's end | cannot end the rounding at a vertex: *cause* |
| a rounding's end over pieces on different faces | cannot end the rounding at a vertex where its last pieces lie on different faces: *cause* |
| a rounding's end where more than three sharp edges meet | cannot end the rounding at a vertex where more than three sharp edges meet: *cause* |
| two or three roundings meeting | cannot join the roundings of two (three) edges at a vertex: *cause* |
| more roundings or sharp edges | cannot join the roundings at a vertex where N roundings and M sharp edges meet: *cause* |
| roundings ending in points | cannot join the roundings that end in points at a vertex: *cause* |

| The corner's exception | *Cause* |
|---|---|
| the end section meets no face at the end ("bouchon non ecrit") | the rounding's end does not meet the face at the end |
| the cut on the face at the end crosses another curve there ("fillets have too big radiuses") | where the rounding cuts the face at the end, it crosses another rounding's contact on that face |
| no intersection of the surface and the face ("echec calcul intersection") | the rounding's surface and the face at the end do not intersect |
| the walk round the vertex reaches no end ("the max number of faces reached") | the walk round the vertex along the faces at the end does not come back to the rounding |
| two consecutive faces with no common edge ("Failed to find edge") | two faces met round the vertex share no edge |
| an edge without a p-curve on a face ("Failed to get p-curve", "no parameters on surface") | an edge at the vertex has no curve on a face the corner takes it on |
| no common face of two roundings ("Trying to check orientation of NULL face") | the two roundings end on no common face |
| the surfdatas at the end not as expected ("PerformMoreSurfdata : corner not computed") | the pieces of the rounding at its end are not as the corner needs |
| others | a curve of the corner has no length; a curve of the corner does not project onto a face; the rounding's contact does not reach the edge at the vertex; the corner's computation crashed (caught); the corner's computation gives no result; else the message itself |

The text always names a vertex, as before: the import then tries more of
the chain's edges. The failure dump (`MITCAD_IMPORT_DRESSUPS`) now writes,
past an item's first four failures, each failure with an error not written
before (up to 12), and each definition of a failed candidate of several
definitions (the error is one of theirs).

**The items.** The census's per-item tables (rerun, main `0328b2d2`) have
59 items with the note in 41 files, 8 of them in reference copies; two
more (`a9e82a9a`, `de7b8b9d` item 159) reached the rerun's time limit and
are counted: 61. Each was imported again (two at a time, 6 GB each), the
definition by the edges' names in the file taken from the dump, and run as
mitcad#322's: on the replay's body and on the history's stored body before
the item (its edges found by their geometry within 0.01 mm), through the
dressup with all its tries and OCCT's fillet alone. "Builds on stored"
means the file's change of volume within 1 %. With the code of this
round and main of the same day (0330, 0331, mitcad#236's 0340 to 0342,
mitcad#300's 0350 to 0352 and mitcad#325):

| Class | Items x / 61 | Builds on stored / on replay | Content id, item | Cause |
|---|---|---|---|---|
| now parametric | 9 | – | `36f79fb9`, `5d6ee474`, `81beaf7b` 82 (0331); `09c282ac` 15, 16 (0350, mitcad#300); `480e591c`, `d498ea9d`, `eecf90bd` 89 (0340, mitcad#236); `5ef2dbd5` 81 (main) | the earlier roundings rolled on a straight edge, B-spline approximations on the replay and cylinders in the file, are now the cylinders (0331); their edges on the rolled edges no longer marked tangent (0350); the ridge's lens-shaped end (0340) |
| upstream: a cap's arc left cut in two | 4 | 4 / 0 | `2d12d69c`/`0c99fb88`, `52ba7a15` 94, 98 | the cap's arc of an extrusion stays cut at the vertex where another region of its profile met it, after a chamfer and a rounding took that region's side away; the file's body has one edge. The corner's walk round the vertex stops there ("does not come back"). With the two pieces joined on the replay's body both build (0.02 % and 0.21 % of the file's change) |
| upstream: faces split otherwise, earlier roundings walked | 3 | 3 / 0 | `a9e82a9a`, `de7b8b9d`, `fc4f1709` 44 | a pattern's faces split otherwise and earlier roundings' B-spline faces where the file has cylinders and tori; 0.39 % on the stored body |
| upstream: after an earlier fallback, faces split otherwise | 1 | 1 / 0 | `53790e1a` 63 | the base feature's faces against the replay's (mitcad#322's class C); 0.33 % on the stored body |
| builds on stored, 39 % off the file's change | 1 | 0 / 0 | `3fe35ae9` 21 | where more than three sharp edges meet |
| the file's definition fails along its edges | 7 | 0 / 0 | `0974db2f` 23; `818fa4e2`, `c6cf5a22`, `48ff426e` 84; `9c3751cd`, `b596234a` 63; `916fb62a` 8 | the vertex note is another candidate's; the definition by the names fails in a walk (6) or with overlapping roundings (1), mitcad#322's classes |
| the file's definition is invalid on the stored body | 3 | 0 / 0 | `9c3751cd`, `b596234a` 64; `db73863f` 74 | a self-intersecting wire, an unorientable face |
| kernel: two roundings with no common face | 5 | 0 / 0 | `7b638e0c`, `9d25940d`, `a953a62a`, `e79e1d36`, `f9d49409` 11 | a lip's inner edge rounded with 4 over a ring 1 wide: both roundings run over the ring onto the chamfers beyond it, at the corner a wall and a post meet at an angle; the ends past the spine carry a side on no face, and OCCT's corner of two roundings needs a face both lie on. The file's corner is the crease where the two roundings meet |
| kernel: the end crosses another rounding on the face at the end | 4 | 0 / 0 | `36f79fb9`, `5d6ee474`, `64fab81a`, `81beaf7b` 11 | the two upright edges of a wall end 3.35 thick, each beside a face 0.5 wide, rounded with 2: the roundings run over the narrow faces (a sliver 0.003 wide beyond one), and their ends on the top cross |
| kernel: the end over pieces on different faces | 5 | 0 / 0 | `5ef2dbd5` 63, 89; `a768c6e1`, `be9717f3` 15; `db73863f` 27 | the pieces at the end are not as `PerformMoreSurfdata` expects |
| kernel: the walk round the vertex does not come back | 5 | 0 / 0 | `61526c66` 12; `6418ebb1`, `920a1cef` 45; `a9e82a9a`, `de7b8b9d` 159 | `PerformIntersectionAtEnd`; tiny edges of 0.05 mm at the vertex (45) |
| kernel: the end meets no face at the end | 2 | 0 / 0 | `0aed68ff` 77; `f9d6d78a` 24 | an OnSame end whose other contact runs parallel to the face at the end, 2 mm from it (77) |
| kernel: a curve of the end does not project | 3 | 0 / 0 | `480e591c`, `d498ea9d`, `eecf90bd` 50 | `ProjLib`; the import's watchdog gives the item up first |
| kernel: the surface and the face at the end do not intersect | 2 | 0 / 0 | `5ef2dbd5` 95; `db73863f` 143 | |
| kernel: no curve of an edge on a face | 3 | 0 / 0 | `49ccc315`, `e41d24ac` 35 (two roundings, four sharp edges); `db73863f` 75 | |
| kernel: others | 3 | 0 / 0 | `5ef2dbd5` 87 (no result where more than three sharp edges meet); `99ef321f` 22 (no intersection with a face at the end); `f5e08b18` 23 (the edge beside the end does not cross the face at the end) | |
| not in the dump | 1 | – | `0b758b01` 141 | now "its result is not in the file's history" |

Upstream noise is 8 of the 61 (all build on the stored body, none on the
replay's), kernel gaps at corners 32, the file's definition failing or
invalid otherwise 10, not dumped 1; 9 come in.

**Fixed: a rounding rolled on a straight edge.** A rounding wider than the
face beside its edge rolls on that face's far edge; where its ball touches
a plane and rolls on a line parallel to it, the rounding is a cylinder. OCCT
stored the walked B-spline approximation (port patch 0331 builds the
cylinder; 0350, mitcad#300, leaves its edge on the rolled edge sharp). The
stored designs have the cylinder, and in one design (three files) the
later rounding failed only on the approximation. Imported again with and without 0331
(the 11 of the 41 designs whose items moved since the census; the others
as before), on main with 0350: with 0331 6 items come in (`36f79fb9`,
`5d6ee474`, `81beaf7b` 76 and 82) and 3 go out (the same designs' item 77:
item 76 now comes in, its corners' tori 0.2 % larger in area than the
file's, and item 77's walk then fails on them; on the stored body it
builds, 0.04 % off). Before 0350, 0331 also brought `09c282ac` 15, 16 and
`0aed68ff` 79 in, which 0350 now does. The 249 specs of mitcad#107: MATCH
90, NEAR 60, OFF 1, failing 98 with and without 0331 (before 0350) and
with it on main with 0350, every result line the same but the volume's
last digits (the cylinders exact).

Not kept: joining the pieces an edge is left in after a dressup took a
face away at its vertex, which the files' bodies have as one edge (measured
before main's 0350 to 0352). As a step after every fillet and chamfer it
brings 10 items in among the 41
designs (the cap's arcs above, `480e591c`, `d498ea9d`, `eecf90bd` 24 and
`a9e82a9a`, `de7b8b9d`, `fc4f1709` 18) and takes 7 out (`2d12d69c`,
`52ba7a15` 104 and 109, an extrusion that then cuts no participant; the
same three designs' item 17, whose definition then fails at a vertex);
as a retry only for the failing rounding, the four cap items come in and
104 and 109 go out. Which vertices the files keep needs tracing first.

Follow-ups:

| Class | Items | What is needed |
|---|---|---|
| a cap's arc left cut in two, the next cut | 4 + 4 | where the files keep such vertices; the extrusion after the two roundings that then cuts no participant |
| two roundings with no common face | 5 | the crease where two roundings meet across sharp edges (OCCT's corner of two roundings needs a common face); the ends past the spine with a side on no face |
| the end crossing another rounding on the face at the end | 4 | roundings over faces 0.5 wide with a sliver of 0.003 beyond (the sliver within the import's tolerance, mitcad#299) |
| the end over pieces on different faces; the walk round the vertex | 10 | `PerformMoreSurfdata` and `PerformIntersectionAtEnd` on these ends, one shape at a time (mitcad#236's line) |
| upstream: faces split otherwise | 4 | the earlier items that split them (mitcad#322's classes C and D) |
| item 77 after 0331 | 3 | the walk on the replay's corner tori 0.2 % off the file's |

## Chamfers that consume a flange's top or a strip (mitcad#327, mitcad#334)

- The concave chamfer as wide as a ring sector's flange (`a9e82a9a`,
  `de7b8b9d`, `fc4f1709` item 7): the flange's rim is rounded on beyond
  its top tangentially, and the chamfer's contact runs along the top's edge
  with the rim. OCCT gave the rim the transition on the wrong side, the
  cause of mitcad#300's lip as well; port patch 0352, merged with
  mitcad#300 after the census, builds it at its size (within 2.3e-10 of the
  file's state), and the rounding after it (item 8) is parametric. No
  further patch: the test of a flange with a rounded rim in
  `geometry/tests/test_chamfer_overflow.cpp` fails without 0352.
- `3a5fe9db` Chamfer1: a boss's foot on a slab chamfered by 2, as wide as
  the slab's top round three sides and the corner arcs; the top goes on
  along the fourth side, and its far edge along one side is two edges end
  to end. The contact along those two edges was along neither, so the top
  kept a strip of no width there, and the contacts along the sides, which
  start inside the far edges where the chain ends, kept points of their own
  beside the vertices where the arcs' contacts began: an invalid shape,
  which the dressup then built 0.1 % smaller. Port patch 0440 splits such a
  contact at the chain's vertices and makes those ends the vertices.
  Chamfer1 now gives the file's state (volume and area); Chamfer2, which
  builds exactly on the stored body, now builds on the replay's too.

Checked by content id (items parametric):

| design | before | after |
|---|---|---|
| `3a5fe9db` | 24 / 25 | 25 / 25 |
| `a9e82a9a` | 180 / 199 | 180 / 199 (items 7 and 8 already parametric on main with 0352) |
| `de7b8b9d` | 180 / 199 | 180 / 199 (likewise) |
| `fc4f1709` | 130 / 142 | 130 / 142 (likewise) |

The final bodies are unchanged. The 249 fillets and chamfers of the
mitcad#107 failure dump were not run again (no whole-dataset runs).

## Exceptions in a rounding's computation (mitcad#330)

The 14 items of mitcad#322's class "an exception in a rounding's
computation", found again with the failure dump (`MITCAD_IMPORT_DRESSUPS`,
the ten files imported alone) and traced through OCCT's fillet alone to the
exception and the step before it. Each was also run on the history's
stored body before the item: all 14 fail there in the same way, so none is
upstream noise of the replay. Fixed / items per kind:

| Kind (the exception) | Fixed / items | Content id, item | Cause |
|---|---|---|---|
| a trimmed curve of zero length (`Geom_TrimmedCurve::U1 == U2` in `BlendFunc::GetMinimalWeights`) | 4 / 4 (one part) | `9c3751cd` 65 (three files), `b596234a` 65 | a rounding of radius 1 along a ridge whose faces come tangent at both ends of the chain. At the last vertex the walk's piece ended where both contacts reached the edges ending at the vertex, 0.0015 short of the guide's end and within the vertex's tolerance (0.003): the stored faces' curves of those edges lie that far off them. The face beyond, tangent to both there, continued the contour, and the next piece from the vertex had both contacts on it, sections of no width, whose minimal weights threw. Fixed by port patch 0400: such a piece ends the stripe, the end a point at the vertex. The first vertex is the start of a cylinder's seam, and the cut of the rounding's end by the cylinder passes 5e-5 beyond the seam's start (within the vertex's tolerance); it was not split at the seam, which was lost, and the cylinder face came out unorientable. Fixed by port patch 0401: the cut is split where its curve on the cylinder crosses the seam's line, here at the seam's vertex. The item is parametric in all four files; its blend faces are the file's within 0.05 mm² (the file's two end pieces are spheres, OCCT's walked splines), its change of volume 4.109 against the file's 4.103, and the final bodies of the four files are the file's within 2.8e-7 of their volume |
| a rational section's weights too small (`Geom_BSplineSurface` in `ChFi3d_Builder::StoreData`) | 0 / 5 (two parts) | `5ef2dbd5` 93, `99ef321f` 23 (two files); `c23a90e7` 12, `6eccefc2` 11 | the ball is larger than a concave corner rounding it has to pass: radius 0.5 along a pocket floor whose wall corner is rounded with 0.3 (`5ef2dbd5`, `99ef321f`); radius 1.5 along a rib's foot on a tube whose rim is rounded with 1 (`c23a90e7`, `6eccefc2`). The walk climbs the corner rounding on its far side, where no ball of the radius touches it: the guide's parameter stops while the contacts still move (about 30 of the piece's 52 points at one parameter), and the approximation of that piece swings its weights from -37 to 34. The file builds the corner otherwise: the earlier corner rounding is carried round the new rounding's axis as a torus (tube the corner's radius, major radius the ball's less the corner's) with a spline blend beside it, a construction OCCT's rolling ball has no counterpart for (mitcad#238's bridge class) |
| the walked blend's approximation (`PerformSurf : Failed approximation!`, the orientation in `StoreData`) | 0 / 2 | `5ef2dbd5` 88, `236b88c6` 7 | the same stall of the walk at its last points, the approximation's tolerance then 0.4 and 38 mm: a rounding of 0.4 along a slot's bottom 0.33 wide, which the file consumes (the edge overflow's class, mitcad#121); a rounding of 1 round a slot's end on a large torus, not traced further |
| "chaining is impossible" (`PerformSetOfSurfOnElSpine`) | 0 / 2 (one part) | `61526c66`, `cb955714` 5 | a full rounding of 1 at the foot of a bore whose contact on the floor crosses a small notch in the floor's rim: past the notch's edge the walk finds no face for that contact. The walk over a gap (0160) handles a rim rounded more sharply than the ball only, and the notch's wall is sharp; the file's torus runs on over the notch, which cuts it |
| a common point's vector (`ChFiDS_CommonPoint::Vector`) | 0 / 1 | `db73863f` 121 | the rounding's faces come tangent at a vertex inside its chain, where the walk's piece ends in a point (4e-9 wide); the next piece starts from that point, which has no direction. Needs the stripe ended in a point and begun again at that vertex |

Of mitcad#107's 249 dressups, run with markers in the changed code
(OCCT's rounding alone, the 1e-3 smaller retry and rings off): only the
two copies of the zero-length item reach it (both now within 4.1e-6 of
the file's next state, 40 faces against its 39), and every other dressup
gives the same result as before.

Found on the way, not changed: `GeomFill_QuasiAngularConvertor` (the
sections of rational blends whose angle exceeds 120 degrees) computes its
coefficient b = -1/beta^2 + beta / (3 (tan beta - beta)) directly for
|beta| down to 1e-6, where the two terms cancel (b is wrong in its first
digit below beta = 3e-4 and of the order of 1e6 at 3e-6; the limit is -0.4),
and below 1e-6 its series gives -0.2; the derivatives fare worse. A walked
blend that vanishes at a vertex where its faces come tangent then fails in
the approximation with weights below zero (a ring cut by its tangent plane
at an inner point, its intersection edge rounded with 0.5: 65004 poles,
weights down to -0.73). With b and its derivatives from their series below
|beta| = 0.1 that approximation has 146 poles and weights 0.99 to 1.06, but
the rounding's corner at that vertex still fails; none of the 14 items
reaches it.

## A boss touching a wall tangentially (mitcad#326)

The chamfer of mitcad#322's follow-up (`7b638e0c` and its reference copy
`f9d49409`, item 102) failed on a body with one face more than the stored
one. The first item that made it so is the join of a ring about a boss of
radius 12 (item 92): the ring's sketch runs along a wall into the boss's
circle where the circle touches the wall, and the solver put the line's end
1.17 µm along the wall from the tangent point (noise along a tangent: that
point is 6 · 10⁻¹⁴ mm off the circle). The body before the join already had
an edge at the exact tangent point (the boss's first quarter). The exact
join kept both: a strip of the wall 1.17 µm wide stayed between the two
parts of the boss's cylinder, a face of 1.1 · 10⁻⁴ mm² with edges of
1.17 µm at its ends, carried through every later item; the chamfer of the
ring beside it failed in OCCT's walk there. On the stored body, which has
one cylinder, OCCT builds it.

Joins within the import's tolerance (`Kernel::join_within`, mitcad#298)
now also try again with a fuzzy value when the exact join leaves more edges
shorter than 1e-5 mm than its operands had (`geometry/src/boolean.cpp`,
`tiny_edges`): the smallest fuzzy value from 1e-5 mm up that leaves no more
is taken (here 1e-5 mm: the two vertices merge, the cylinder is one face as
in the file); where none does, the exact join stays. Joins that leave no
such edges, and joins outside the import, are as before.

The thread after it (item 114) had been parametric only through a retry on
the history's exact state, made because the earlier chamfer was built
0.1 % smaller; with exact chamfers there is no retry, and the thread came
in 0.7 % off. Its definition that first moves the bolt's face to the
thread's major diameter failed on the replay's body, which is the stored
body's exactly: a dress-up gives its body as a compound holding one solid,
and OCCT's offset of a compound gives a shell, not a solid. Moving faces
(`geometry/src/faceops.cpp`, `offset_input`) now offsets the solid, as
offsetting a whole body already did.

Checked on the two designs (features parametric, all verified against the
file's history; final bodies):

| design | features before → after | final bodies within 0.001 % before → after | what still falls back, and why |
|---|---|---|---|
| `7b638e0c` | 129 → 131 / 134 | 4 → 3 / 4 | the chamfer and the thread are parametric. The bolt's final body is 0.0057 % off the file's (0.16 mm at the thread), as whenever this thread is built: MitCad's modelled thread is not the file's. Two roundings whose edge no longer exists after an earlier item and one that fails at a vertex, as before |
| `f9d49409` | 123 → 124 / 129 | 4 → 4 / 4 | the chamfer is parametric. The thread (on a bolt of diameter 16.0, moved to 15.822) comes in 0.9 % off the file's change (-549.2 mm³ against -554.3; one face up to 0.063 mm off), and without the move its result is 7.8 mm³ off: the modelled thread's profile, not this ticket's cause. A rounding 20 % off the file's change, and the three of the other design |

The chamfer's result still has one face more than the stored body: the
ring's edge it runs along is split at the tangent point (the stored body's
is too), and the chamfer keeps a face on each side of it; the rounding
items after it build on it.

## Extrusion roots left after mitcad#325 (mitcad#332)

The extrusions the census of 15.10.2026 left as roots, each imported alone
with `MITCAD_IMPORT_TRACE_MISS` (content ids; the second id of a pair is a
copy of the design):

- **Faces of a document without history.** A referenced document whose
  bodies are stored only (an imported part or circuit board: `.smb` blobs,
  no `.smbh`) was never asked: the decoder said "the other document has no
  history", and an extrusion up to such a face went to the guessed planes.
  Its stored bodies are asked now (`inputs.rs`, `find_in_stored`). Each
  stored body names its entities from the operation that imported it, so
  the same names fit faces of many bodies (one design's six references
  fit 6 to 13 bodies each); the face is the one with the most of the faces
  its recipe lists around it (`NamedState::faces_around`), and where
  several bodies tie it stays not found, with the count in `found`.
  `3e4710ff` item 36: the face named by 36 neighbours is one body's plane
  (the other hit, a torus face, has fewer of them): the extrusion is
  parametric (56 of 56 items, was 55). `0aed68ff`/`66684457` item 27: the
  face's names (one name and one neighbour, no operations) fit three
  bodies of the board equally (a plane parallel to the sketch on a
  switch housing's component, which the board places twice, a plane
  facing sideways and a spline face): still a fallback. The reference's
  placement (`_f3d.context`) is the board's turned by 180° about the
  vertical from where the file places it at that item (the joint between
  them stays as-built, turned by 180°), so it names no occurrence either,
  and the extrusion's link to the housing's component cannot pick one of
  its two occurrences. The file holds nothing more that tells the body (the
  reference's second GUID is one per reference and does not occur in the
  board's file).
- **Text profiles whose texts are gone.** A text profile (`92F9A5F4`)
  names its sketch text through its target object; where the design no
  longer has that object (a text deleted from its sketch after the
  extrusions that use it were made: the history keeps their results), the
  decoder lists it in `_f3d_missing_texts`, and the extrusion falls back
  with "its profile is N sketch texts the file no longer has (deleted from
  <sketch>)" instead of probing the sketch's other regions (`b373797d`/
  `56c97389` items 22, 25, 26: two texts deleted from a sketch that has
  none left; the three items name them). Nothing in the file gives the
  texts' glyphs, so they stay fallbacks.
- **A 3D sketch** (`a33cb893`/`ebd77010` item 12): the profile is a
  triangle of the sketch's lines, two of which run from the square's
  corners to a point 226 mm off the sketch's plane; it lies in the plane
  of item 10, which those two lines define. MitCad's sketch entities are
  planar (`EntityKind::Point { at: [f64; 2] }`, `core/sketch`;
  the import keeps a point's x and y, `sketch.rs`), so the lines and the
  plane come out in the sketch's plane. Needed: sketch geometry off the
  sketch's plane.
- **Names in no state** (`a33cb893`/`ebd77010` item 39): the profile face's
  document GUID is the design's own (the third GUID of its manifest), and
  its names (faces of operation 357 that the history's operation 357 did
  not make; it made six others) are in none of the history's 47 states:
  a selection older than the design's last edits.
- **A cut that fails on the replay's body** (`9e53857a` item 10): the
  decoded cut builds an unorientable face on the replay's body, which
  measures as the state's (signature within 0 mm) but carries 0.57 mm³
  (1.8 · 10⁻⁵) from an earlier join of two pipes of one radius (the
  intersection where they meet is degenerate); on the stored body it
  builds. A cut retried per target with fuzzy values from 1e-5 to 0.01 mm
  gave invalid results at every value (tried and left out), so the cause
  is the kernel's cut through that seam, not the tolerance.
- **Regions the profile loops map inexactly** (`0b758b01` item 40,
  `fd2a66f8` item 16): the loops name pieces the replayed sketch splits
  otherwise (`exact=false`): the file's regions are 35.7 mm² where the
  replay's are 31.7 mm² (a circle crossing them), and one region of
  1 537.8 mm² with small ones where the file cuts only the small ones.
- **Up to a face, the file's regions** (`61526c66`/`cb955714` item 2): the
  loops select the profile's two child regions (23.6 and 10.9 mm²), which
  are the file's (its new start face is them merged with the body's
  587.3 mm² face), but the replayed join of them up to the decoded face
  adds no volume where the file's adds 43.2 mm³: the extent, not the
  profile, differs (not traced further).
- **A column left by a cut up to a face** (`64b26f50`/`4d3c4d1b` item 6):
  the loops select 5 of 6 regions exactly, among them the 1.92 mm² one
  whose column the cut leaves (a face of 1.92 mm² 1.13 mm off the file's,
  at the cut's start): that region's prism does not reach the face it
  goes up to (not traced further).
- **A sketch on a face of a fillet whose edges are lost** (`437b1356`
  item 10): the fillet before it (item 8) has lost all 12 edge references
  and leaves its state as it was; the sketch's linked circle (8 mm) finds
  no edge (the replay's rim is 7 mm), and the cut's participant is a body
  of 119 mm³ while the file's state after it is a body of 5 277 mm³ the
  replay does not have at that point (not traced further).

## Roundings OCCT walks as B-splines (mitcad#328, mitcad#329)

Two classes of mitcad#322 are fillets that build on the file's stored state
but not on the replay's: "after an earlier fallback, faces split
otherwise" (5 items besides the 2 of mitcad#300) and "the replay's body
differs where the edges are" (4 items). Each was traced to the first item
whose result deviates from the file's, comparing the replay's bodies with
the history's stored bodies item by item (`MITCAD_IMPORT_DRESSUPS_ITEMS`,
`core/import-replay/src/dressups.rs`: the bodies after the items it names) and trying the
fillet on both.

The largest cause was not the fallback nor a split: where OCCT walks a
rounding (a ball rolling on a narrow face's far edge, the edge overflow of
mitcad#121; a rounding between a plane and a cylinder it does not build in
closed form), it makes the rounding a B-spline surface and its edges
B-spline curves, also where they are exactly a cylinder, lines and arcs
(within 1e-13 mm and 1e-8 mm here). The files have the cylinder, lines and
circles. The faces lie on each other, the measures agree, so the item is
taken; a later rounding along those faces then fails in OCCT's walk on the
B-splines and builds on the analytic faces (checked with OCCT's fillet
alone on the replay's body with its faces and edges converted: it builds,
the change 9.5989 mm³ against the stored body's 9.5980). The dressup now
gives the new faces whose B-splines are a plane, cylinder, cone, sphere or
torus within 1e-7 that surface, and their B-spline edges that are lines or
circles within it those curves (`geometry/src/canonical.cpp`; faces closed
on a seam are left as they are; a converted face that does not check valid
leaves the result as OCCT built it).

| Item (content id) | First deviation | Now |
|---|---|---|
| `5d6ee474`, `81beaf7b` item 76 | item 74 rounds a boss's two outer edges with 2 where its sides are 1.5 wide: the rolling ball's rounding (a cylinder) is a B-spline, so is the foot rounding after it (item 75); the rounding of the boss's end-face loop fails in the walk | parametric (0.44 % off the file's change); item 82, the same on a second boss, too |
| `66684457` item 77, `0aed68ff` item 79 | | parametric on main before this change |
| `db73863f` item 106 | item 105's rounding round a hole is a closed surface of revolution where the file has a torus, and the cylinders' seams lie elsewhere (the stored body splits the selected edges in two); builds on the stored body | falls back: a closed face is not converted; which of the two makes OCCT's walk fail is not traced |
| `4365a039` item 12 | item 10 rounds a notch's two vertical edges between its side plane and the tube's inner cylinder with 0.5: OCCT walks them, their arcs at the bottom are B-splines where the file has circles; the rounding of the bottom's inner circle chain (through the notch) fails in the walk | parametric (0.03 % off); the chamfer after it (item 17) too |
| `480e591c`, `d498ea9d`, `eecf90bd` item 63 | item 61 rounds four edges meeting at two vertices: OCCT's rolling ball corner (edges of quarter circles of radius 0.5) where the file has a corner of more patches with edges of zero length, 0.10 to 0.15 mm apart (taken: corner faces within `geometric::CORNER_DISTANCE`) | falls back: on the history's stored state the file's edges (the 9 edges round its corner face) fail in OCCT's walk too, a kernel gap on the exact state |

Designs imported again by content id (features parametric, before →
after; final bodies as before): `5d6ee474` 83 → 84 of 93 (items 76 and 82
come in; item 77, the rounding of the boss's foot after item 76, now runs
on the replay's item 76 and fails at a vertex: at its exact size it fails
on the stored state too, where it was built only 0.1 % smaller before),
`81beaf7b` 84 of 93, `4365a039` 15 → 17 of 20, `66684457` 71 of 81,
`0aed68ff` 98 of 112, `d498ea9d` 100 of 120 as before; `db73863f` 116 →
119 of 145 (its run ended inside its time limit this time; the three items
at the end that hit the limit before come in).

## Fillets left after the analytic dressup faces (mitcad#337)

The three items mitcad#328 and mitcad#329 left, each replayed from the
dressup dump (`MITCAD_IMPORT_DRESSUPS`, with `MITCAD_IMPORT_DRESSUPS_ITEMS`
for the items before them) through OCCT's fillet alone and through the
dressup, on the replay's body and on the history's stored body before the
item:

| Item (content id) | First deviation and cause | Now |
|---|---|---|
| `db73863f` item 106 | Not item 105's closed rounding: the rounding of the boss's end circle (radius 7 on a cylinder of 12) fails alone too, the other boss's builds. Item 104 extrudes the boss on by 3 mm; the join merges the two coaxial cylinders into one face whose seam runs in steps (up the first part's seam, 67 mm along the circle between the parts, up the second part's seam, which lies at a smaller angle). The stored body has one straight seam and the end circle split at it; both faces are valid. OCCT's analytic rounding (a torus) was split into the face's domain a period off the blend's parameters, the piece covered half the circle and the walk for the rest failed. Fixed in OCCT: port patch 0420 moves the tangency line by whole periods of the surface into the blend's period | parametric, 0.05 % off the file's change (on the stored body 0.07 %) |
| `480e591c`, `d498ea9d`, `eecf90bd` item 63 | Kernel, on the exact state: the definition by the edges' names runs on the history's stored body itself (same volume, every edge found at 0 mm). The chain of 9 edges along a cylinder of radius 22.75 rounded with 1 crosses four B-spline patches 0.25 mm wide (the file's vertex blend of item 61, at the two corners) and planes 1.0 to 1.2 mm wide: the ball overflows faces narrower than the radius, and OCCT's walk stops after 5 of the stripe's surfaces (`WalkingFailure`), on the stored body as on the replay's | falls back: the edge overflow across narrow B-spline patches (mitcad#121's class) |
| `5d6ee474`, `81beaf7b` item 77 | Kernel, at the corners: the rounding (radius 2) of the boss's foot on the wall ends at four vertices where the wall runs on into the foot rounding of the boss's side, of the same radius 2. The ball fits that rounding, so the rounding's end section is that rounding's end edge (a circular arc of 1.79 mm) and the blend touches it all along, and the corner rounding of item 76 between them is consumed (the bodies stored in the file have exactly that: the new face ends on the arc). OCCT's corner (`PerformOneCorner`, OnSame) intersects the blend with the face at the end, which is singular all along the arc (the intersection fails). Taking the arc as the cap gets past it, but the prolongation of the face at the end then degenerates (the opposite contact ends at the arc's other vertex) and the consumed corner face is kept as a sliver: not solved | falls back |

Designs imported again by content id (features parametric, before →
after; final bodies as before, all within 6 · 10⁻⁶ of the file's):
`db73863f` 119 → 120 of 145 (inputs read 68 → 69); `eecf90bd` 100 of 120
and `5d6ee474` 84 of 93 as before.

## Fillet corners left after mitcad#323 (mitcad#339)

The two largest kernel classes of *Fillets that cannot end or join at a
vertex* were traced on the failure dump (`MITCAD_IMPORT_DRESSUPS`) with
OCCT's fillet alone, on the replay's body and on the history's stored body
before the item (both fail the same way, so no upstream noise):

- **The end crossing another rounding on the face at the end** (`36f79fb9`,
  `5d6ee474`, `64fab81a`, `81beaf7b` item 11, one design in four files).
  The four vertical edges at the ends of a window through a wall 3.54 thick
  are rounded with 2. Each end is split into faces: a strip 0.5 wide beside
  one wall and the rest, meeting the strip at 0.35 degrees (at the other
  end two strips, one of them beyond a step of 0.003). Each rounding beside
  a strip ran over the strip onto the face beyond (the edge overflow of
  mitcad#121; at 0.35 degrees OCCT also takes the two faces for tangent), up
  to 2 from its edge, across the other rounding's contact on that face, and
  their ends on the window's top and bottom crossed. The file's roundings
  beside the strips roll on the strips' far edges (the item's change
  -10.3795 mm³ is exactly one ordinary rounding and three rolled ones).
  Fixed in OCCT (port patch 0430): a side that overflowed onto a face across
  another rounding's contact rolls on the edge before that face instead
  (also before a nearly tangent face, and past a sliver to the edge before
  it); 0431 then ends such a rolled blend where its edge ends on the face at
  the end (an intersection 6e-15 past its contact curve was not taken).
- **Two roundings with no common face** (`7b638e0c`, `9d25940d`, `a953a62a`,
  `e79e1d36`, `f9d49409` item 11, one design in five files). A box rim's lip
  is rounded with 4 along its inner bottom edge, lines and corner arcs that
  meet at 53 degrees, over a ring 1 wide under the lip and a chamfer below
  the ring. Each rounding ran over the ring onto the chamfer, the lines'
  onto its planes and the arcs' onto its cones, so at each corner the two
  had no face in common (the null face of "Trying to check orientation of
  NULL face" was two sides on no face taken for one). The file's roundings
  roll on the ring's far edges, the arcs' as exact tori, and meet in a
  crease from the lip's vertical edge down to the ring's corner. Fixed in
  OCCT (0432): two stripes meeting at a vertex that both overflowed their
  common face onto different faces roll on its far edges; their corner is
  the crease (`PerformTwoRolledCorner`), and a walked blend rolling on a
  circle about the axis of the cylinder it touches becomes its torus, which
  reaches the crease past the walk's end. The rim's later cut (item 13)
  then failed: the analytic dressup faces (mitcad#328) made the ring's
  corner circles exact circles, which lie 2.3e-7 off the stored cones below
  them, more than the edges' tolerance of 1.7e-7, and the cones no longer
  checked valid. A new edge's tolerance now covers its distance from every
  face it bounds, and those faces are checked too (`canonical.cpp`).

Each fix has an OCCT-only test in `geometry/tests/test_fillet_corners.cpp`
that fails without it (a window's end split into a strip and a face at
0.35, -0.35, -10 degrees and with strips on both sides; a lipped rim whose
removed volume is the corner regions swept along the edges, within the
body, less what any ball covers).

Imported again by content id (features parametric, before → after; final
bodies within 5e-8 of the file's):

| Class | Items achieved / target | Designs |
|---|---|---|
| the end crossing another rounding | 4 / 4 | `5d6ee474` 84 → 85 of 93; `36f79fb9`, `81beaf7b` 85 of 93; `64fab81a` 27 of 28 |
| two roundings with no common face | 5 / 5 | `7b638e0c` 131 → 132 of 140; `9d25940d` 21 of 22; `a953a62a` 17 of 18; `e79e1d36` 73 of 76; `f9d49409` 126 of 132 |
| the end over pieces on different faces | 0 / 5 | not fixed: e.g. `5ef2dbd5` item 89 ends a rounding of 1 over a face of linear extrusion between tori and cones, where `PerformMoreSurfdata` finds its pieces at the end not as it expects |
| the walk round the vertex does not come back | 0 / 5 | not fixed; the census's items moved: `61526c66` 12 now fails where two faces round the vertex share no edge (a rounding of 0.0012 mm³), `6418ebb1`/`920a1cef` 45 lose two edge references first |
| a cap's arc left cut in two | 0 / 4 | not fixed: in `52ba7a15` 94 and 98 the blend's end lies on the bar's rounding beside the chamfer it starts from; that face does not hold the corner's vertex, and the walk round it cycles without reaching the vertex (the cap's arc split at the vertex where the other profile region met it, on the bar's other side) |
| item 77 after 0331 | 0 / 3 | `36f79fb9`, `5d6ee474`, `81beaf7b`: still "the rounding's surface and the face at the end do not intersect": the corner of *Fillets left after the analytic dressup faces* (mitcad#337), the end section on the next rounding's end arc; fixed by port patch 0460 (mitcad#342, below) |

## A rounding ending on a rounding of its radius (mitcad#342)

Item 77 of `5d6ee474`, `81beaf7b` and `36f79fb9` (one design in three
files) rounds the feet of a boss's two ends with 2. The boss is 1.5 high on
a wall; its long top edges are rounded with 2 (rolling on the foot edges,
item 74), the feet of those roundings with 2 (item 75) and the ends' edges
with the roundings with 1.5 (item 76). At each of the four corners the ball
of item 77 fits item 75's foot rounding beside it: the end section is that
rounding's end arc (1.79 long), from the corner's vertex on the wall to the
arc's other end, where the opposite contact (on item 76's rounding of the
top's corner, past the foot's corner torus) ends. The corner torus between
them (item 76, 0.51 wide) lies wholly between the contacts and is consumed;
the bodies stored in the file have exactly that.

OCCT's corner (OnSame) took the torus for the face at the end and
intersected the blend with it, but the blend is tangent to it, and to the
foot rounding, all along the arc: the intersection failed, on the replay's
body and on the history's stored body before the item alike. Port patch
0460 recognises such an end (the points at the two vertices of the edge to
prolong, the section on its curve, the face at the end bounded otherwise by
the spine and the opposite face) and builds it as the end section lying in
the face beyond that edge (0410): the cap is the edge's own arc, which takes
its place there, and the consumed torus with its other edges goes. On the
stored body before the item OCCT's fillet now gives the stored state's 59
faces; the new faces at a corner are within 0.002 mm² of the stored ones,
the change of volume 4.949 mm³ against the file's 4.925. Test: the boss's
foot rounding in `geometry/tests/test_fillet_corners.cpp` (OCCT only; fails
without 0460).

Imported again by content id (final bodies within 2e-6 of the file's):

| Design | Features parametric | Inputs read | Items matching the history |
|---|---|---|---|
| `5d6ee474` | 85 → 86 / 93 | 59 → 60 (1 inferred) | 56 → 57 |
| `81beaf7b` | 85 → 86 / 93 | 59 → 60 (1 inferred) | 56 → 57 |
| `36f79fb9` | 85 → 86 / 93 | 59 → 60 (1 inferred) | 56 → 57 |

Of mitcad#107's 249 fillets and chamfers, run through OCCT's fillet alone
with a marker in the new code, none reaches it, and every result is as
before.

## Fillet corners left after mitcad#339 (mitcad#341)

The three classes *Fillet corners left after mitcad#323* left, imported
again by content id with the failure dump (`MITCAD_IMPORT_DRESSUPS`) and
each failing definition run through OCCT's fillet alone, on the replay's
body and on the history's stored body before the item. All the items
below fail the same way on both, so the causes are in the geometry kernel
(or, for item 45, also in the file).

**The end over pieces on different faces.** A rounding ends at the
spine's end vertex, where one of its faces (an earlier rounding, a torus
or cylinder) runs on tangentially into the face at the end and the other
meets that face at a sharp edge (e.g. `5ef2dbd5` item 89: radius 1 along a
face of linear extrusion beside a torus 1.1 wide that turns into the torus
at the end; `be9717f3` item 15: radius 2 along a wall's foot whose corner
cylinder turns into the front face). Both contacts reach an edge of their
faces at the end; the sharp side ended the walk, the tangent side walked on
along the face at the end while the other side lay on a tangent plane, and
`PerformMoreSurfdata` found its pieces at the end not as it expects. The
bodies stored in the files end the rounding with a cap on the face at the
end, from the tangent contact's crossing of its edge to the other contact's
crossing of the sharp edge. Fixed in OCCT (port patch 0450): that face is
then the face at the end on both sides and the corner cuts the blend with
it; the walk past the spine's end up to the tangent side's edge is kept
while its sections are regular (0077 cut it at twice the contacts' speed,
short of that edge, and the cap then missed the blend: an invalid shape or
a wrong one). OCCT-only test in `geometry/tests/test_fillet_corners.cpp`
(a footed wall's foot rounded up to the front face, a short and a long
one; fails without the patch, and the long one without the kept walk).

**The walk round the vertex does not come back** (`6418ebb1`/`920a1cef`
item 45, `61526c66` item 12). Item 45's references: 2 of its 7 edge
references are lost in the file itself (the faces they name are not in
the state the item was made from; the file keeps the item's earlier
geometry), and its body is the stored state (items 36 to 44 fall back).
The definition by the 5 references left gives 3 edges (the torus's edges
with the plane); the rounding the file stores runs along those and the
torus's edges with the two narrow cylinders between them (1.3 mm wide), 5
edges, its far side running over the cylinders onto the tori beyond. The
import's definitions found against the history take 12 or 4 edges (the
roundings intersect, or the end over pieces), neither the file's 5. The
file's 5 fail in OCCT too: at the two vertices where the torus's edge with
a cylinder meets its edge with the plane, the two edges leave at 20
degrees (the torus's boundary doubles back along the cylinder), and OCCT's
corner of two roundings fails ("Failed to find edge"); the file's rounding
is one surface there. Item 12 rounds with 0.1 two chains that meet beside
a planar face of 0.076 mm² (edges 0.26 mm long); OCCT's corner of two
roundings fails there on the stored body too ("no parameters on surface").

**A cap's arc in two pieces** (`2d12d69c`/`0c99fb88`, `52ba7a15` items 94
and 98). A rounding of 3 along a cylinder's short edge (1.86 mm) runs on
tangentially over the bar's roundings and ends at a chamfer's vertex on the
extrusion's cap, whose arc is split at the vertex where another region of
the profile met it. At the end the chamfer's side reaches its sharp edge
with the cap first, the cylinder's side then crosses into the bar's
rounding (tangent, and not a face at the end vertex), and the end goes to
`PerformIntersectionAtEnd`, whose walk round the faces at the end does not
come back. Not fixed.

Imported again by content id (features parametric, before → after; final
bodies within 2.7e-6 of the file's, as before; one body of `5ef2dbd5` 1.24e-6 off
where it was 1.09e-6):

| Class | Items achieved / target | Designs |
|---|---|---|
| the end over pieces on different faces | 3 / 5 | `5ef2dbd5` 74 → 77 of 98 (item 63, and items 31 and 39, whose results were invalid shapes before); `be9717f3` 52 → 53 of 56 and `a768c6e1` 53 of 56 (item 15 in both; its result 0.44 mm³ off the file's change of -116.0, a base feature brings the body to the file's state after it). Not: `5ef2dbd5` 89 (its corner now builds; the two roundings of the narrow face between them, 1 wide under a radius of 1, then intersect each other: mitcad#121's edge overflow); `db73863f` 27 (now "the rounding's end does not meet the face at the end", on main too) |
| the walk round the vertex | 0 / 5 | `6418ebb1`, `920a1cef` 45: lost references in the file and the corner of two roundings at 20 degrees; `61526c66` 12: the corner of two roundings beside a sliver; `a9e82a9a`, `de7b8b9d` 159 not looked at |
| a cap's arc in two pieces | 0 / 4 | `2d12d69c`/`0c99fb88`, `52ba7a15` 94, 98: `PerformIntersectionAtEnd` |

## Fillet corners left after mitcad#341 (mitcad#343)

The five items *Fillet corners left after mitcad#339* left, imported again
by content id with the failure dump and each failing definition run
through OCCT's fillet alone (with a trace of the corner code), on the
replay's body and on the history's stored body before the item.

**A cap's arc in two pieces** (`52ba7a15` and `0c99fb88`, the design in
`2d12d69c`, items 94 and 98). A bar 10 x 10 joins a ring's outer cylinder;
its long edges at the side are chamfered and the chamfers' edges rounded
with 5; the side's concave edge with the cylinder is rounded with 3, on
tangentially over those roundings and chamfers up to the caps. Near each
cap the walk is right: the cylinder's contact reaches the cap first, the
walk goes on until the other contact crosses onto the chamfer, and its
extension over the chamfer reaches the chamfer's edge with the cap, so
the end lies over two pieces, both cut by the cap. OCCT takes such an end
(`PerformMoreSurfdata`) only where the piece before the last ends on an
edge of the cap at the corner's vertex, and the cap's edge with the
cylinder is in two pieces: the profile's vertex where the bar's side met
the cylinder stays on it after the chamfer and the rounding took the
side's top away, with only those two pieces at it, and the contact ends
on the piece away from the vertex. The extension was then dropped and the
walk round the bar's rounding, a face without the vertex, did not come
back. Fixed in OCCT (port patch 0550): such a piece counts as the edge at
the vertex. On the replay's body OCCT's fillet now gives the stored body
after the item within 0.3 µm (change of volume 29.6973 mm³ against the
file's 29.7023). Test: a bar on a cylinder in
`geometry/tests/test_fillet_corners.cpp` (OCCT only; fails without 0550;
its result is the one built where the cap's edge is one piece). Of
mitcad#107's 249 fillets and chamfers, run through OCCT's fillet alone
with a marker in the new code, none reaches it.

**Not fixed** (the causes, first deviation first):

- `5ef2dbd5` item 89 (radius 1; the input is the history's stored body,
  the same on the replay): each of the two roundings along a cone's edge
  with a curved band ends at a vertex where a plane strip 0.64 mm² (the
  wall of a notch 1 mm wide) is the face at the end. The bodies stored in
  the file run each rounding on past that vertex along the cone, its side
  cut by the strip's plane, which grows from y 5.40 to 9.36 with it. OCCT
  walks on past the spine's end (an OnSame end, 3.7 mm past it) across the
  notch until the band's contact meets an edge of the band beyond the
  notch's middle; the corner then builds a shape with faces of reversed
  orientation (the change of volume −494 mm³ against the file's −0.66), and
  the two roundings together cross over the notch ("intersect each
  other"). What is needed: the end cut by the strip's plane along the
  stripe's side, a corner OCCT has no case for.
- `6418ebb1`, `920a1cef` item 45 (radius 0.5): two of its seven references
  name faces not in the state it was made from (a loss in the file), so
  the import's definitions take 3, 12 or 4 edges, not the file's 5 (the
  torus's edges with the plane and with two cylinders 1.3 mm wide). The
  file's 5, run on the stored body, fail in OCCT's corner of two
  roundings: at each of the two vertices where the torus's edges with a
  cylinder and with the plane leave at 20 degrees, the two stripes meet
  with the cylinder's edge with the plane sharp between them;
  `PerformTwoCornerbyInter` goes on to `PerformMoreThreeCorner`, which finds
  no edge of a face round the vertex ("Failed to find edge"; at the other
  vertex "Failed to get p-curve of edge"). The stored body's edges there
  have tolerances of 1.2e-3. What is needed: both the edges the file
  rounds (from the lost references) and a corner of two roundings across a
  sharp edge, one surface in the file.
- `61526c66` item 12 (radius 0.1, a change of 0.0012 mm³): fails on the
  stored body too, at two vertices beside a planar sliver of 0.076 mm²
  ("no parameters on surface"; with the two edges of one chain alone "the
  max number of faces reached"); the stored body's vertices there have
  tolerances of 2.7e-3, 2.7 % of the radius. Not traced further.
- `a9e82a9a`, `de7b8b9d` item 159 (radius 8): the file rounds two long
  edges of a pocket's ceiling with its walls (x = −54 and −73, 69 mm
  long, with 1 mm pieces beyond notches 1 mm wide at both ends) and cuts
  the roundings by the walls at the ends; the ceiling's contact runs
  exactly along a wall block's side edge, the radius from the wall. On the
  replay's body (the stored state's) OCCT's result is invalid: at each end
  the walk runs on past the vertex 21.6 mm along the ceiling up to its rim
  instead of ending at the face at the end, and the corner walks round the
  ceiling (more than 15 faces) or builds reversed faces (changes of
  volume of 10⁴ mm³ against the file's 1951); with radii 7.99 to 8.01 it
  is invalid too, so the coincident contact is not the only cause. Not
  solved.

Imported again by content id (features parametric, before → after; final
bodies within 1.9e-8 of the file's, as before):

| Class | Items achieved / target | Designs |
|---|---|---|
| a cap's arc in two pieces | 4 / 4 | `52ba7a15` 94 → 96 of 110 (inputs read 61 → 63), `0c99fb88` 94 → 96 of 110 (61 → 63) |
| the end along a notch's wall | 0 / 1 | `5ef2dbd5` 89 |
| lost references, two roundings across a sharp edge | 0 / 2 | `6418ebb1`, `920a1cef` 45 |
| two roundings beside a sliver | 0 / 1 | `61526c66` 12 |
| roundings over notches, walked past the end | 0 / 2 | `a9e82a9a`, `de7b8b9d` 159 |

## Fillet corners left after round 3 of mitcad#343

The four items the section above left, imported again by content id with
the failure dump and each failing definition run through OCCT's fillet
alone (with traces of the join, the hatching and the corner code), on the
replay's body and on the history's stored body before the item.

**Roundings over ribs, up to blocks flush with the contact**
(`a9e82a9a`, `de7b8b9d` item 159, radius 8). The ribs are 1 thick and
stand out 6 from the pocket's walls (not notches), and the blocks at the
ends have their side where the roundings touch the ceiling. The
definition's six edges (each wall's edge in three pieces between the ribs)
are six stripes; OCCT's corner at a rib joins two of them over it (0241),
the far ends end at the blocks. Two faults, both in OCCT:

- the first deviation: on one wall the ceiling's contact line ran past the
  block along the block's concave edge with the ceiling up to the
  ceiling's rim (21.6 further) while on the mirrored wall it ended at the
  block; the line lay 7e-15 inside the ceiling there and 1.4e-14 outside
  it on the other wall, and the hatcher took the stretch along the edge as
  inside or outside by that. Fixed (port patch 0580): a contact line along
  a concave edge of its face is off the face there;
- the join over a rib met at the other stripe's end point while its first
  piece (on the rib's end) started where the blend crosses the rib's far
  edge (the rib's side cuts the blend along a section): an open shell; and
  where a stripe joined to another was joined again, the one joined first
  kept a surface of its own. Fixed (port patch 0581).

On the stored body OCCT's fillet now gives the file's change of volume
(1951.1 against 1950.99, every face as in the stored body after the item).
Test: a pocket rounded over ribs up to flush blocks in
`geometry/tests/test_fillet_corners.cpp` (OCCT only; fails without the
patches; the volume added against the rounding's section).

**Not fixed** (the causes, first deviation first):

- `6418ebb1`, `920a1cef` item 45 (radius 0.5): the stored bodies before
  and after the item have edge and vertex tolerances up to 0.46. The
  references: the 5 left resolve to the big torus's 3 edges with the plane;
  the 2 lost are the torus's edges with the two narrow cylinders (r 2.5,
  1.3 wide) that join those 3 into one chain of 5. The stored body after
  the item has neither cylinder but a lens of each (the roundings took the
  rest), so the history shows 6 vanished edges round each cylinder, of
  which only the two on the torus are the item's: the file's 5 are the 3
  resolved edges and the edges joining them along their common face (the
  torus), which the import could take, but OCCT fails on those 5 too. Each
  cylinder's rounding (torus to cylinder) stops where its contact reaches
  the cylinder's concave edge with an earlier rounding (a torus of 0.6),
  at the points where the file's rounding leaves the cylinder (within
  4e-5 of the stored body's vertices there); the file's rounding rolls on
  over that earlier rounding to the vertex where the torus's edges with
  the cylinder and with the plane leave at 20 degrees. At that vertex
  OCCT's corner of two roundings needs both ends on the sharp edge between
  the plane and the cylinder (`PerformTwoCornerbyInter`: "not on the same
  arc"), and the general corner (`PerformMoreThreeCorner`) finds no edge
  between two of the faces round the vertex ("Failed to find edge"). What
  is needed: the cylinder's rounding run on over the earlier rounding (the
  edge overflow of mitcad#121) and a corner of the two roundings across
  the sharp edge.
- `61526c66` item 12 (radius 0.1): OCCT's contour of the torus's edge
  with a B-spline face of 0.108 mm² runs on into the torus's edge with a
  planar sliver of 0.077 mm² (the two edges turn by 9.95e-5 rad at their
  vertex, within the fillet's angular tolerance; the vertex's tolerance is
  2.7e-3) up to the sliver's cusp, where its edges with the torus and with
  the plane x = -5.848 leave at 0 degrees; the corner there fails ("Failed
  to find edge", "no parameters on surface"). The stored body after the
  item keeps that edge sharp: the file's rounding ends before it. With the
  contour stopped at that vertex (angular tolerance 5e-5) the end fails in
  turn ("OneCorner: echec calcul intersection"): the rounding takes the
  B-spline face (narrower than its contact), and the file's body has the
  sliver's plane grown along the torus up to the rounding's own face. What
  is needed: the edge overflow over that face (mitcad#121) and the chain
  stopped where the file's stops.
- `5ef2dbd5` item 89 (radius 1): the end is OnSame; OCCT walks past it on
  the band and the cone until the band's contact meets the edge of the
  band's tongue between the notch walls (4 past the end), and the corner
  goes to `PerformIntersectionAtEnd` and `PerformMoreThreeCorner`. The
  stored body's rounding runs on 5.9 past the end along the cone (to
  y = -0.51, where the cone's contact reaches the notch wall's plane),
  cut by that plane along its side from y 9.36 (where the band's contact
  crosses it) to y -0.51; the plane grows up to the cut. What is needed:
  the walk on past the end until the cone's contact reaches the end face,
  the band's contact on its extension, and a corner that cuts the blend by
  the end face along its side, which OCCT has no case for.

Imported again by content id (features parametric, before → after; final
bodies within 4.4e-11 of the file's volumes, as before):

| Class | Items achieved / target | Designs |
|---|---|---|
| roundings over ribs up to flush blocks | 2 / 2 | `a9e82a9a` 180 → 181 of 199, `de7b8b9d` 180 → 181 of 199 |
| roundings onto an earlier rounding across a sharp edge | 0 / 2 | `6418ebb1`, `920a1cef` 45 (40 of 50, unchanged) |
| two roundings beside a sliver's cusp | 0 / 1 | `61526c66` 12 (11 of 14, unchanged) |
| the end cut along its side by a notch wall | 0 / 1 | `5ef2dbd5` 89 (77 of 98, unchanged) |

## Fillet corners left after round 4 of mitcad#343

The three items the section above left, traced again on the history's
stored body before the item and through OCCT's fillet alone.

**The end cut along its side by a notch wall** (`5ef2dbd5` item 89, radius
1). The first deviation: past the OnSame end OCCT's walk stopped where the
band's contact left the band's tongue across its edge with a face tangent
to it (4 past the end), the cone's contact still short of the notch wall,
and the corner took that edge's face for the face at the end. Fixed (port
patch 0600, `WalkedOnSameToEndFace`): that piece is walked again with the
band's surface unbounded up to where the cone's contact reaches the wall's
edge with the cone, and the OnSame corner cuts it by the wall. On the
stored body OCCT's fillet now gives the stored body after the item (volume
within 1.3e-5, 145 faces as there); the next item (90) builds on it too.
Test: a plate with an overhanging tongue in
`geometry/tests/test_fillet_corners.cpp` (OCCT only; fails without the
patch).

**Not fixed** (the causes, first deviation first):

- `61526c66` item 12 (radius 0.1): the rounded edges are two mirrored
  concave edges between a torus and a B-spline face of 0.108 mm², each from
  a vertex where the faces become tangent (the rounding vanishes there) to
  a vertex A where the torus's concave edge with a planar sliver (0.077
  mm², tangent to the B-spline face) goes on to a cusp. Two deviations:
  - the chain: OCCT's tangent chain runs on from A along the sliver's edge
    to the cusp, the stored body after the item keeps that edge sharp and
    ends the rounding at A. On one side the edges turn by 1e-4 rad at A, on
    the other by less than 1e-6 (the stored body before the item even has
    that edge split 6.7 µm from A, along the same faces), and every edge at
    A is tangent or continues the chain: no turn or crease gives the stop,
    so neither the angular tolerance nor 0180's stop at turns can find it
    from the geometry (and the file's references, resolved in a later
    state, name the whole chain). What is needed: the rule that ends the
    chain at A, then a way to give it to OCCT (stop vertices);
  - the end at A (the chain stopped there by hand, an angular tolerance of
    5e-5 on the side that turns): the walk on past A has the B-spline
    face's contact reach that face's convex edge with a torus of the
    revolve 0.19 past A, still below the sliver's plane, and the OnSame
    corner at A fails: the sliver's plane does not cut the blend across.
    The stored body rolls the blend on along that edge (0011's edge
    overflow, within 2–5 µm of the edge) until the sliver's plane, grown,
    cuts it. What is needed: past the spine's end, the walk carried on as a
    blend rolling on the edge it met (as 0011 does in the middle of a
    rounding), and the OnSame corner cutting the two pieces by the plane.
- `6418ebb1`, `920a1cef` item 45: unchanged from the section above (the
  cylinder roundings rolled on over an earlier rounding past the narrow
  cylinder's edge, and a corner of two roundings across the sharp edge at
  20 degrees); not taken further in this round.

Imported again by content id (features parametric, before → after; final
bodies as before, within 4e-4 % of the file's volumes; the other two
designs' items unchanged with the patch):

| Class | Items achieved / target | Designs |
|---|---|---|
| the end cut along its side by a notch wall | 1 / 1 | `5ef2dbd5` 89 (and 90 after it): 78 → 80 of 98 |
| two roundings ending at a sliver's crease | 0 / 1 | `61526c66` 12 (11 of 14, unchanged) |
| roundings onto an earlier rounding across a sharp edge | 0 / 2 | `6418ebb1`, `920a1cef` 45 (40 of 50 each, unchanged) |

## Fillet corners left after round 5 of mitcad#343

The two items the section above left, traced again on the history's stored
bodies before and after the item and through OCCT's fillet alone.

**A rounding whose narrow face runs out before the end of its chain**
(`61526c66` item 12, radius 0.1). The file names, on each side, the
torus's edge with the B-spline face (a), a piece 6.7 µm long (b) and the
torus's edge with the planar sliver up to its cusp (c), as one tangent
chain; the history's stored body before the item has exactly those edges.

- The chain: in the file's own state the edges a, b and c continue each
  other to 1e-14 rad at both vertices on both sides (the turn of 1e-4 rad
  seen before is the replay's, whose sliver edge runs 1e-4 rad off). No
  angle stops the chain, and the file's references take c in. The stored
  body after the item rounds c only near A, differently on the two mirrored
  sides although the input is mirror-symmetric: on one side the blend runs
  0.28 past A until its contact on the torus meets c, cut there by the
  sliver's plane; on the other the sliver's plane, grown, cuts the blend
  0.24 before A. Beyond that the stored bodies keep c sharp. No rule of the
  geometry gives either end; the rounding along the rest of c is left to
  the kernel (below).
- The first deviation was OCCT's: the B-spline face's contact leaves that
  face across its convex edge with another torus 0.0965 before A (inside
  the spine), where the ball has to roll on that edge (as the stored bodies
  do, within 3 µm). The chain's one guide reaches both ends of the spine,
  so the walk's end handling took that edge for the end region and ran the
  blend on over the B-spline's surface beyond it until the torus's contact
  met c; the stripe stopped there and the corner at the end failed. Fixed
  in OCCT (port patch 0620, `ChFi3d_ObstacleBeforeEnd`): such an edge (a
  convex edge whose face beyond does not hold the end vertex) is an
  obstacle, and the walk goes on rolling on it and on the sliver's edge
  that continues it.

Now the file's definition builds on the replay's body: valid, 195 faces
(the stored body after the item 193), its change of volume 0.0097 mm³
against the file's 0.0012 (2.8e-6 of the body): the rounding goes on along
c, the ball touching the torus and rolling on the sliver's far edge, where
the file keeps c sharp. The item is parametric and verified. The next item
(13, a fillet of the item's result) still falls back: its walk between the
two faces fails.
Test: a wall on a floor that narrows before the end in
`geometry/tests/test_fillet_corners.cpp` (OCCT only; without the patch the
rounding stopped where the floor's contact met the floor's far edge).

**Two roundings across a sharp edge at 20 degrees** (`6418ebb1`,
`920a1cef` item 45, radius 0.5). The import now takes the two lost
references as the edges that join the found ones end to end along their
common face (the torus's edges with the two narrow cylinders): the file's
five edges, after the three found ones alone (`joining_edges`, as many as
the references lost, on one face only; unit test in
`core/import-replay/src/features.rs`). OCCT fails on them as before. Its walk of
each narrow cylinder's rounding (the cylinder a strip 1.3 wide between the
big torus, an earlier rounding of 0.6 and the plane at both ends) has its
contact on the cylinder reach the cylinder's short edges with the plane
0.9 before each vertex, where the plane holds the vertex: the end handling
runs the blend on over the cylinder's surface past them up to where the
torus's contact meets an edge; the file's rounding rolls on over the
earlier rounding there. The corner of that stripe with the big one at each
20-degree vertex then fails ("two faces met round the vertex share no
edge"). The stored body before the item is itself invalid (edge and vertex
tolerances up to 0.46). What is needed: the cylinder's rounding run on
over the earlier rounding up to the vertex, and a corner of two roundings
across the sharp edge between the plane and the cylinder.

Imported again by content id (features parametric, before → after; final
bodies as before; with the patch the other designs' items are as without it:
`5ef2dbd5` 80 of 98, `a9e82a9a` and `de7b8b9d` 181 of 199, `52ba7a15` and
`0c99fb88` 97 of 110):

| Class | Items achieved / target | Designs |
|---|---|---|
| a narrow face running out before the end of the chain | 1 / 1 | `61526c66` 12: 11 → 12 of 14 (inputs read 6, inferred 0 → 1) |
| roundings onto an earlier rounding across a sharp edge | 0 / 2 | `6418ebb1`, `920a1cef` 45 (40 of 50 each, unchanged) |

## Fillet corners left after round 6 of mitcad#343

Traced again on the dumped inputs (the history's stored body before the
item) through OCCT's fillet alone, with traces of the walks, the stripes'
ends and the corners. Nothing in the import or the port changed in this
round: no item below was brought to build the file's result.

**`6418ebb1`, `920a1cef` item 45 (radius 0.5): one rounding over two narrow
strips, cut by them in a notch.** The torus's three edges with the plane
are three stripes, broken by two strips of one cylinder (r 2.5, a boss
standing on the plane, 1.3 wide) between the torus and earlier roundings of
0.6 (tori tangent to the plane), the plane's contact running onto those
roundings there. The bodies stored in the file have
one rounding face across each strip: the ball keeps touching the torus and
the earlier rounding over the strip, the strip's edge with the torus lies
under it, and of the strip only a lens is left (area 1.2 of 2.3), between
the rounding and the earlier one.

- The first deviation: the two stripes on either side of the first strip
  are one rolling ball. Walked on from the end of one (the earlier
  rounding's surface unbounded), its contact on the earlier rounding leaves
  that face across its edge with the strip at the file's lens vertex
  (503.058, −289.287, −3.482) and comes back across the same edge at the
  other lens vertex (503.333, −288.959, −3.263), where the other stripe's
  walk starts: the two sections agree within 1.4e-5 and 4e-6. OCCT instead
  cut the first walk back to the spine's end (the walk past it, `0077`'s
  speed rule) and ended each stripe at its own vertex of the strip, a corner
  of one stripe each, which fails ("Failed to find edge" and "Failed to get
  p-curve of edge"; with the file's five edges, the corners of two stripes
  across the strip's sharp edge with the plane fail the same way).
- What the join needs, and why OCCT's stripes cannot hold it: the boss cuts
  the joined blend along a curve 3.80 long between those two vertices
  0.48 apart (the file's edge between the rounding and the lens). From the
  point where the contact leaves the earlier rounding the cut runs back,
  through the band between the contacts, to the section where both
  contacts are still on their faces (it crosses the end section of the
  walk past the spine's end, at least 0.33 back along the guide, where the boss pierces
  the middle of the band), and on to the other vertex. The rounding face is
  one face with a notch in its side: over that stretch each section of the
  blend is two spans, the boss's part between them. A stripe's pieces
  (`ChFiDS_SurfData`, `ChFi3d_KPartCutPiece`) hold one span per section, a
  side cut by a face beyond only where the contact itself is off its face;
  a trial join of the two stripes (walk on, cut by the strip, meet at the
  common section as free ends, as `ChFi3d_JoinAcrossNotches` does for
  analytic blends) found the common section but no cut piece: the
  intersection of the blend with the boss is two lines meeting outside the
  piece. What is needed: a blend face trimmed by the boss as a face of the
  result, not as a piece of a stripe.
- The second strip: the stripe walked on past its start covers the third
  stripe's whole spine and over the strip (that stripe is dropped as
  coincident); its contact on the other earlier rounding passes the strip's
  edge within a step and is not cut, where the file has a lens of that strip
  too.

**`61526c66` item 12's other end**: not taken further. The stored bodies end
the two mirrored sides differently (0.28 past and 0.24 before the vertex),
with no rule in the geometry, and the item is parametric and verified as it
is. The next item (13, radius 0.5 along the edge of two planes, the plane
x = −5.848 narrowing to an edge 0.02 long at its end) fails on the stored
body too ("unorientable"; with the history's edge, the walk stops where it
rolls on an edge of a neighbouring face): mitcad#121's edge overflow at the
end of the rounding, not a consequence of item 12's end.

**`db73863f` (mitcad#379), the corner items**, traced as far as the first
failure:

- item 143 (radius 0.5, the file's five edges on a cone between faces that
  are nearly tangent to it, the rounding a sliver 0.02 wide at its lower
  ends): at both lower ends the walk ends exactly at the file's end section
  (on the cylinder below the cone, its points within 4e-4 of the file's)
  and then runs on past it. At one end the OnSame corner's cap (0.017 long)
  is found, but the prolongation of the cone's edge with the cylinder past
  the vertex, 0.008 long, is not (the intersection of the two surfaces, where
  the edge's own circle continued lies on both); at the other the cap, the
  intersection of the whole blend (a surface 95 long) with the cylinder, is
  not found until the blend is bounded about its end section. With both
  handled (the circle continued, the blend bounded; trials only, not in the
  port) every corner builds, but the result is
  invalid (unorientable faces round the cone's wall faces): the two stripes
  that meet tangentially at the top vertex each walk on past it over the
  other's spine (one by 10.7 along its guide), where the file's rounding
  leaves the first wall face for the next one about 5 before that vertex, in
  one face from there.
- item 27 (along a thread's edges): two OnSame ends find no face at the end
  ("bouchon non ecrit"), the third end's pieces are not taken
  (`PerformMoreSurfdata`).
- items 74 and 75 (radius 20, three and six roundings at a housing corner):
  every corner builds; 74's result has unorientable faces, 75's new faces do
  not close (the shell classification of the rebuilt solid throws in
  `TopoDS_Builder::Add`).
- item 133 (radius 0.8 along a thread's long edges): both ends'
  `PerformIntersectionAtEnd` walks round the faces at the end reach their
  limit of faces.
- item 137: one of the three edges' stripes has no piece (its walk fails).
- item 142 (radius 0.1): the result has an unorientable cone and cylinder.

| Item | Achieved / target | Cause of the gap |
|---|---|---|
| `6418ebb1`, `920a1cef` 45 | 0 / 2 | the rounding over the strips needs a face with a notch cut by the boss, which the stripes cannot hold |
| `61526c66` 12's other end | not taken | no geometric rule; item 13 fails on the stored body (mitcad#121) |
| `db73863f` 143 | 0 / 1 | sliver rounding: corners only with trials; then two tangent stripes overlapping past their common vertex |
| `db73863f` 27, 74, 75, 133, 137, 142 | 0 / 6 | traced to the first failure only (above) |

