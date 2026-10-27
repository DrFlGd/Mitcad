# The .f3d import: notes on single rounds, part 3

Notes on single rounds of work on the corpus. Part 3: rounds 8 to 11 of
mitcad#343 and the fillet items of mitcad#434 to mitcad#441. Part of the
[.f3d import](f3d.md) notes; the [import documents](../README.md#documents)
list the other topics.

## Corner and end cases of `db73863f`'s roundings (round 8 of mitcad#343)

The roundings of `db73863f` (mitcad#379) that fail on the history's
stored bodies before them, traced again through OCCT's fillet alone on
the dumped inputs (the stored body before each item, its edges found by
their names in the file), with traces of the walks, the corners and the
walk round the end vertex. Two are fixed in the port (0720, 0721); the
design imported again by content id: features parametric 125 → 127,
fallbacks 12 → 10 (the final bodies the file's).

- **Item 74 (Fillet1, radius 20, three edges of the housing's corner):
  parametric, verified.** Each edge ends at a vertex of a plane square to
  it, a side of the housing's triangular end, out of which a coaxial
  cylinder of radius 49.135 stands; the cylinder's trace on the plane cuts
  off a small piece (16.84 mm²) at the vertex. The rounding's contacts pass
  that piece and end on the plane's pieces beyond it, the end cut by them
  and by the cylinder between them. The walk round the end vertex
  (`PerformIntersectionAtEnd`) met: a piece beyond, the cylinder, the small
  piece, the cylinder again, the other piece beyond; the cylinder was cut
  twice up to its edge with the small piece and the small piece kept (its
  edge with the cylinder twice in the faces: unorientable). Port patch 0720
  takes such a face (between two visits of one face across one edge,
  holding the end vertex, its edge not crossed by the rounding) out of the
  faces at the end and drops it. On the stored body the change is
  -3312.88 mm³ against the file's -3312.93, the cylinders and the
  roundings the file's faces; the result valid.
- **Item 142 (Fillet5, radius 0.1, the plug's thread flank along its
  chamfer cone): parametric, verified.** The rounding starts where the
  flank comes out of the root cylinder (an OnSame end): the cone's edge
  with the cylinder (a circle of both) is prolonged past the vertex up to
  the contact on the cone, across the cone's seam, where the cone is a
  ring 0.022 wide and nearly flat. OCCT intersected the two surfaces for
  it: the p-curve ran the long way round the cone (its ends a period
  apart), the 3d curve strayed 0.1 off the circle, and the cone lost its
  seam and its edge with the cylinder (unorientable). Port patch 0721
  prolongs the edge as its own circle where that lies on both surfaces,
  its p-curves parametrized as the circle and split at the seam each in
  the face's domain; on the replay's body (the thread modelled, its root a
  spline, the edge's curve an approximation) the surfaces are still
  intersected, with the ends taken in one period of the cone. On the
  stored body the result has the file's faces
  and vertices (the cone's seam cut at the circle, the rounding starting
  where the file's does).
- **Item 143 (Fillet6, radius 0.5, the five edges round the thread's
  chamfer cone): 0 / 1.** With 0721 the first OnSame corner builds (its
  prolongation, 0.008 long, is the circle continued); the second fails
  where the cap is the intersection of the blend, a surface 95 long along
  the helix, with the root cylinder: not found over the whole blend, found
  with the blend bounded about its end section (a trial, not in the port).
  Then the result is invalid: the thread's crest (a cylinder band 0.36
  wide) is narrower than the rounding, and the stripes of the flank on
  either side of it (one with the crest's edge) each walk past their
  common vertex over the other's spine (10.7 and 1.6 along their guides),
  where the file's rounding leaves the first flank for the second about 5
  before that vertex, in one face. Needed: the two stripes joined into one
  blend between the two flanks over the crest (the crest consumed).
- **Item 133 (Fillet7, radius 0.8, the two helical edges of a thread's
  crest, three turns): 0 / 1.** The crest (a cylinder band of 3.2 turns,
  0.79 wide on average) is narrower than the two roundings: their balls
  touch both flanks over most of the crest (the pieces of the first edge's
  stripe lie between the two flanks), so the two stripes are one blend
  there. At the ends the walk round the vertex follows a face that does
  not hold the vertex and loops (the limit of 15 faces). Needed, as for
  item 143: the stripes of a narrow face's two edges joined into one blend
  between the faces beyond them.
- **Item 27 (Fillet4, radius 0.6, the thread's roots on both flanks, 13
  edges over the per-turn root faces): 0 / 1.** At the thread's start the
  root rounding's flank contact reaches the end face (the ridge's flat
  start, z = 15.432) 6.2 past the vertex while the root contact is still
  above it; the root cylinder there continues as the shaft, so its contact
  meets the end face's plane only about 3.3 further, with no edge of the
  root face there (OnSame: "bouchon non ecrit" at both starts). The walk's
  end on the root face also carries a spurious arc (the root face's seam,
  at another angle). Needed: the walk run on with the flank's surface
  unbounded until the root contact crosses the end face's plane (as port
  patch 0600 does up to an edge), then the OnSame cap. The thread's other
  end fails in `PerformMoreSurfdata` (not traced further).
- **Item 75 (Fillet2, radius 20, the concave edges between the cylinder
  and the plane pieces of item 74): 0 / 1.** Each rounding ends at a vertex
  of item 74's rounding (its end cap there). Its plane contact leaves the
  narrowing piece across its convex edge with the housing's side (OCCT
  rolls on it; the file's ball keeps touching the plane extended past that
  edge, see round 9 below), then at that edge's end OCCT rolls on the next
  edge, item 74's rounding's end circle, whose face holds the end vertex,
  and the corner's update runs the cylinder contact's p-curve out of its
  range (a point 3e12 off). The file's blend runs on and is cut across
  item 74's rounding (that rounding shortened by 0.31). Needed: the walk
  ended where the obstacle edge reaches the face at the end, and a corner
  cutting the blend by that face.
- **Item 137 (Fillet5, radius 2): 0 / 1.** A key inside the bore ends in a
  lens face 0.67 wide between its convex arc and the bore's concave arc;
  both arcs are rounded with 2, wider than the lens. The concave rounding's
  walk fails; the file joins the two roundings tangentially below the lens
  (an S-shaped section), the lens consumed. Needed: two roundings of
  opposite sides of a narrow face meeting tangentially.
- **Items 120 and 121 (edge overflow, mitcad#121):** not taken further in
  this round.

| Item | Achieved / target | Cause of the gap |
|---|---|---|
| `db73863f` 74 | 1 / 1 parametric, verified (0720) | |
| `db73863f` 142 | 1 / 1 parametric, verified (0721) | |
| `db73863f` 143 | 0 / 1 | two flank stripes over a crest narrower than the rounding overlap past their vertex |
| `db73863f` 133 | 0 / 1 | the two crest edges' stripes are one blend between the flanks; the walk round the end loops |
| `db73863f` 27 | 0 / 1 | the root contact meets the end face's plane past the flank's, where the root face has no edge |
| `db73863f` 75 | 0 / 1 | the walk rolls on an edge of the face at the end; the blend is to be cut across item 74's rounding |
| `db73863f` 137 | 0 / 1 | two roundings of a lens's two sides meet tangentially, the lens consumed |
| `db73863f` 120, 121 | 0 / 2 | edge overflow (mitcad#121), not taken |

## Blends cut by a narrow face they roll over (round 8 of mitcad#343)

The owner decided (18.10.2026) that fillets get the larger OCCT change: the
blend trimmed by the boss as a face of the result. Port patch 0700.

**`6418ebb1`, `920a1cef` item 45 (radius 0.5).** On each side of the first
strip the stripes are one rolling ball (round 7). OCCT now joins them
(`JoinOverNarrowFaces`): the blend of one end is walked again, from a
section four radii before its end, over the earlier rounding's surface
unbounded, along a guide through the ball's centres of the two ends'
pieces (A's own guide runs straight on past the spine's end and its
sections slide along the ball's path: the walk ran 6 off and was cut
back). The contact leaves the earlier rounding at the stored body's lens
vertex and comes back at the other one (within 1e-5); the boss cuts the
walked blend between them along a curve 3.8043 long (the stored body's
3.8042) that reaches 0.59 and 0.62 past those points along the blend. The
walk is three pieces on one surface, one face of the result whose side is
that cut (a notch); the other stripe's end piece is cut back to the same
ball's section past the cut and the two meet there. With the file's five
edges, the stripe on the strip's edge with the torus lies under the ball
and is dropped. Over the second strip the boss pokes through the blend in a
closed curve 3.66 long (`PierceBlends`): a hole in the blend's face, the
strip's lens bounded by that curve alone, as in the stored body (its lens
0.8877 against the file's 0.8876, the first strip's 1.1947 against
1.1953). On the stored body before the item the definition now gives a
change of volume of 7.398 (three edges 7.398, five 7.398) against the
file's 7.343; the faces over the strips agree with the stored body's
within 0.002 in area, the rest of the difference is OCCT's roundings
elsewhere on the torus, as before the item's failure. The stored bodies
before and after the item have the same three invalid faces (tolerances
up to 0.46); the result has those and no other.
Tests: a corner rounded over a cap of a ball standing out of it in
`geometry/tests/test_fillet_pierced.cpp` (OCCT only; the cap notching the
rounding, with and without the cap's edges with the wall rounded too, and
poking through it; each fails without the patch).

**`61526c66` item 12's other end: not done.** The input is mirror-symmetric
(the chain's edges continue each other to 1e-14 rad on both sides, the two
narrow B-spline faces 0.10750 each, the two slivers 0.0752 and 0.0753), and
the stored body after the item rounds both B-spline faces alike (0.07268
and 0.07269 left) but ends the rounding differently at the two slivers: one
shrinks to 0.0620 (the rounding runs 0.28 past A, cut by the sliver's
plane), the other grows to 0.0856 (its plane cuts the rounding 0.24 before
A). No rule of the geometry gives the two ends: one rule makes both sides
alike. The item is parametric and verified as it is (round 5: the
rounding goes on along the sliver's edge, 2.8e-6 of the body); its
behaviour is unchanged by 0700 and so is the patch 0342's test.

Imported again by content id (features parametric, before → after):

| Class | Items achieved / target | Designs |
|---|---|---|
| one rounding over narrow boss strips, cut by them | 2 / 2 | `6418ebb1` 40 → 41 of 50, `920a1cef` 40 → 41 of 50 |
| item 12's other end | 0 / 1 | `61526c66` 12 of 14, unchanged |

The other designs with the patch, item by item as without it: `5ef2dbd5` 81
of 98, `a9e82a9a` and `de7b8b9d` 181 of 199 (item 159's short stripes
beyond its ribs are kept: the ball touches the wall beyond a rib), `52ba7a15`
and `0c99fb88` 97 of 110, `db73863f` 126 of 145 (128 with 0720 and 0721,
the port built with 0700 on top of them).

## Thread crests rounded on both edges (round 9 of mitcad#343)

The `db73863f` (mitcad#379) items round 8 left on a thread, traced again
on the history's stored bodies before them through OCCT's fillet alone.
Port patch 0750.

**Item 133 (Fillet7, radius 0.8, both helical edges of the crest, three
turns): parametric, verified.** The crest (a cylinder band 0.81 wide) is
narrower than the two roundings (each 0.46 wide on it): their contacts on
it cross, and port patch 0013 computes each again overflowing the crest,
the ball touching both flanks, one blend, the crest consumed, as in the
stored bodies. At the thread's ends the end plane (square to the axis) and
the chamfer cone (60 degrees) meet the helix at about a degree and cut
that blend along curves 83 long: on the plane the contact on one flank
meets it 17 past the end of its edge, the other's 65 before. The first
deviation: the walk of the joined blend stopped 9 from the end, where the
other flank's stored surface ends (its B-spline about its face), and its
contact rose to the crest's edge. Four things in OCCT, each needed:
- the face surfaces of the walks continued by their screw motion past
  their bounds (the flanks are swept along the helix);
- the guide past the spine's ends continued along the helix (prolonged
  along its end tangent, its sections turned a radian away 60 past the end
  of a helix of radius 28, and the walk was cut back);
- at the cone, which meets the flank at 0.8 degree, the contact's crossing
  onto it taken as the end (it was taken for the flank continued, and the
  ball rolled on the cone);
- of the two stripes the one that ran out to both ends kept, the other
  dropped (in the synthetic thread each ran out at one end only, and the
  corners failed).
On the stored body the definition gives a change of volume of -80.1539
against the file's -80.181 (1.7e-7 of the body), 128 faces as in the
file; the ends cut at the file's points within 0.001.
Test: a thread rounded on both crest edges between end planes in
`geometry/tests/test_fillet_thread_crests.cpp` (OCCT only; valid, the
crest consumed, one more turn taking away the section's volume swept round
the axis within 0.5 %); it fails without the patch.

**Item 27 (Fillet4, radius 0.6, the crest edges of a thread cut as a
groove, two chains of 13 and 12 edges over the per-turn faces): 0 / 1.**
With 0750 the corners at the thread's start now build: the guide past the
chains' ends follows the helix, so the walk runs on until the crest's
contact meets the groove's floor (the plane z = 15.432) where the stored
body's does, the cap from the flank's edge to the floor's rim the stored
body's (its ends within 1e-4 of the file's). Two gaps are left:
- the result is invalid: the floor's wire is reported self-intersecting
  where the cap leaves the rim's prolongation (OnSame: the floor's edge
  with the crest's cylinder prolonged from the vertex to the contact)
  tangentially, at their common vertex (tolerance 2e-6); the cap lies
  inside the rim all along;
- the definition removes 34.73 against the file's 43.92 (both chains),
  while the blend faces' areas are the stored ones' (562 each): the rest of
  the difference lies outside the blends, not traced further.

**Item 143 (Fillet6, radius 0.5, the five edges round the thread's
chamfer cone): 0 / 1, unchanged by 0750.** The OnSame cap at the
chain's end on the root cylinder is still not found over the whole blend
(95 long along the helix), and the two stripes meeting tangentially across
the crest's end (a cylinder band 0.36 wide) at the cone still walk past
their common vertex over each other's spines. Needed (round 7): the cap
found on the blend bounded about its end, and the two stripes joined
across the crest's end into one blend that leaves the first flank for the
second about 5 before that vertex, as the stored body's single face does.

Round 9's items: 1 / 3 (133; 27 and 143 left as above). `db73863f`
imported alone: 128 → 129 parametric of 145 (fallbacks 10 → 9). The other
designs, each imported alone, keep their counts: `6418ebb1` 41 / 50,
`920a1cef` 41 / 50, `5ef2dbd5` 80 / 98 (main gives 80 too), `a9e82a9a`
181 / 199, `de7b8b9d` 181 / 199, `52ba7a15` 97 / 110, `61526c66` 12 / 14.
One fallback there fails differently: `5ef2dbd5` item 86 (Chamfer2, 0.263
at 45 degrees on the edges of a band of faces meeting at 1.4 and 2.2
degrees). Each edge's chamfer now ends at the next face of the band (the
kink rule) where OCCT failed before, but the walk past the end stops at the
guide's extension (half the spine's length, `ExtentOneCorner`) before the
far contact reaches that face's edge, and the corner closes the gap with a
sliver: the plane's wire self-intersects. With an extension of one length
the far contact reaches the edge and the result is valid (on the stored
body, the four edges of the stored state: valid, -1.444 against the file's
-2.2697, so still not the file's).

## Lens, overflow and corner cases of `db73863f` (round 9 of mitcad#343)

Items 75, 120, 121 and 137 of `db73863f`, traced through OCCT's fillet
alone on the stored bodies before them (as in round 8), and item 86, which
follows item 75 and changes with it. Three are fixed in
the port (0770, 0771, 0772), each with an OCCT-only test in
`geometry/tests/test_fillet_lens_overflow.cpp` that fails without its
patch.

- **Item 120 (radius 10, the foot of a post under a block as wide as the
  post): parametric, verified (0770).** The underside's contact runs off
  the underside across the block's long bottom edges; from there the file's
  ball rolls on those edges, touching the post, up to the two points where
  the post's top circle touches them (the post's side tangent to the
  block's long sides there), where the rounding vanishes. OCCT took those
  edges, which end at the rounding's end vertex, for no obstacle and ended
  each side where the contact left the underside; the corners at the points
  filled the rest wrongly (invalid). Now an arc into the end vertex tangent
  to the blend's other face is an obstacle, rolled on up to the vertex,
  and the roundings that all vanish at one vertex end in a point there. On
  the stored body the change is 852.71 against the file's 852.70; 41 faces
  against 40 (the torus split at its seam). Its inputs are read since
  mitcad#439: the file names one arc of the post's top circle (half of it)
  with its tangent chain, and the circle is three arcs, split at the two
  points. There the next arc and the block's long bottom edge both run on
  from an arc in its direction; MitCad's own chain took the long edges
  too, the rounding with them failed, and the fillet fell back to the
  kernel's contour, the one arc (a change of 426.3 against the file's
  852.7), so that the import took the arcs gone in history state 75
  instead (inferred). The arcs are concave and the long
  edges convex, and a rounding cannot run on from one into the other (it
  vanishes where they meet): the chain now continues only into edges of the
  same convexity (`geometry/src/face_select.cpp`, `EdgeContinuations`;
  test `test_mitcad_fillet_chain_through_points_where_the_rounding_vanishes`
  in `geometry/tests/test_fillet_chains.cpp`), and the one arc named gives
  the three. `db73863f` imported again: 132 of 145 items parametric, 6
  fallbacks, inputs inferred for 14 items (15 before); items 120 and 121
  verified with their inputs read.
- **Item 121 (radius 1, the block's bottom outline over item 120's
  rounding): parametric, verified (0771).** Along each long side the edge
  runs over item 120's rounding, whose face comes tangent to the side at
  those points (the rounding ends in a point there, its surface degenerate
  at that corner). OCCT carried the contour on through the points; its walk
  crawled into the point and stopped, or its last section took parameters
  on the degenerate face's iso far from the contact's and no p-curve
  approximated it ("a common point's vector" above). Now the contour stops
  at such a vertex, each side's rounding ending in a point there, and the
  walk's last section before it is solved again step by step from the
  contact's side. On the stored body the change is -26.782 against the
  file's -26.715 (0.25 %, within the import's 1 %), with the file's 54
  faces.
- **Item 75 (radius 20, the cylinder's foot on the plane pieces of item
  74): parametric, verified (0772).** The file's blend is the ball rolling
  between the plane, extended past the housing's side, and the cylinder
  (every centre 20 from the plane): over item 74's rounding it goes on
  beyond the housing, cut by the housing's two sides and item 74's
  rounding, which grow up to it (the edges between them run on to 0.3834
  above the plane). It does not roll on the plane piece's edge with the
  side, as round 8 had it. OCCT ended each stripe at its edge's end; the
  corners there failed. Now two such stripe ends (one radius, on the same
  face and on one plane, their vertices on a third face) are walked as one
  blend over the corner, cut by the three faces and joined to the second
  stripe. On the stored body the change is 822.54 against the file's
  823.08 (0.07 %); the cuts and junctions are the file's edges (the
  junctions at z = 0.3834, the crossings of the sides' edges the file's
  within 1e-4), 60 faces against 57 (the blend split otherwise, its area
  the same within 0.01 per corner). On the replay's body (its faces and
  their surfaces reversed, a thread as B-splines) the walks run the other
  way along the spine; the points where the contact crosses the sides'
  edges now take their transition from the side of the edge's vertex on
  the corner's face, and the change is 822.27.
- **Item 86 (Fillet4, the plane pieces' edges with the housing's sides
  beyond item 75's blend): parametric → fallback.** It was parametric on
  item 75's stored body (its 15 edges found by their names in the file).
  On item 75's own result only 7 of them are found (against an earlier
  history state), and the rounding of those stops part of the way ("the
  walk between the two faces fails"; OCCT's fillet alone on that input,
  with a radius of 5, fails the same way with main's port: a change of
  item 86's input, not of the patches' work on it). Item 75's blend is three
  faces per corner where the file has two: OCCT's join splits the first
  stripe where the walk starts (the middle of its last piece) and the
  second where the walk meets it, while the file's two faces meet along an
  edge from the first junction to the cylinder (not a section of the
  ball). A trial walking each stripe from its last piece's far end (two
  faces per corner, meeting at the section through the first junction)
  left an inverted face at the stripes' other ends and was not kept.
- **Item 137 (radius 2): 0 / 1.** The key's end face is a lens 0.67 wide
  between its convex arc and the bore's concave arc, both rounded with 2.
  The file's concave rounding is a ball touching the bore and rolling on
  the lens's convex edge; its convex rounding is a ball touching the key's
  side and externally tangent to that ball (an S-shaped section, checked
  at the middle to 1e-4). The lens is consumed between x = ±2.0449, beyond
  which the key meets the bore in a crease. OCCT rolls each rounding on the
  other's edge (0430), and the concave one's walk fails. Needed: the
  concave rounding rolling on the convex edge first, then the convex
  rounding walked between the key's side and the concave rounding's
  surface, the lens consumed where the two meet; not done in this round.

| Item | Achieved / target | Cause of the gap |
|---|---|---|
| `db73863f` 120 | 1 / 1 parametric, verified (0770); inputs read 1 / 1 (mitcad#439) | |
| `db73863f` 121 | 1 / 1 parametric, verified (0771) | |
| `db73863f` 75 | 1 / 1 parametric, verified (0772) | |
| `db73863f` 137 | 0 / 1 | the two roundings of a lens's sides meet tangentially (S-shaped), the lens consumed; no such construction in OCCT |
| `db73863f` 86 (kept) | 0 / 1, was parametric | on item 75's result 7 of its 15 edges are found: item 75's blend is three faces per corner, the file's two |

Imported again by content id (features parametric, before → after):
`db73863f` 128 → 130 of 145 (items 120, 121 and 75 parametric, item 86 a
fallback); the others item by item as with main's port: `6418ebb1` and
`920a1cef` 41 of 50, `61526c66` 12 of 14, `5ef2dbd5` 80 of 98 (81 in round
8; 80 with main's port too), `a9e82a9a` and `de7b8b9d` 182 of 199 (181 in
round 8; not attributed), `52ba7a15` and `0c99fb88` 97 of 110.

## A thread's chamfer and a groove's floor (round 10 of mitcad#343)

Items 143 and 27 of `db73863f` (mitcad#379), traced again through OCCT's
fillet alone on the bodies before them (the stored state's for item 143;
for item 27 the replay's, on which its edges' names resolve, within 0.003
mm³ of the stored body in every 3 mm of the thread). Each first deviation
was a trait of the stored bodies OCCT did not allow for; four port patches
(0800 to 0803), each with an OCCT-only test in
`geometry/tests/test_fillet_crest_corners.cpp` that fails without it.

**Item 143 (Fillet6, radius 0.5, the five edges round the thread's chamfer
cone): verified on the stored body (0800, 0801, 0802); a fallback in the
import.** The cone's loop of edges
runs from where one flank comes out of the root cylinder along that flank
(over two faces), the crest band left by an earlier rounding of both crest
edges (0.36 wide, the two roundings tangent to it and to the flanks), the
other rounding and the other flank down to the root again. The stored
result is one rolling ball along the whole loop: a long blend on the first
flank, a torus over the crest band, pieces on the rounding and the second
flank, a sliver 0.015 wide where the cone meets that flank at 1.7 degrees.
- The cap at the start, on the root cylinder, was not found. Not because
  the blend is long (round 7's trial bounded it about its end; the cap,
  15.29 long, a tongue between two points 0.87 apart, is found on the
  whole blend): the stored cylinder is trimmed to one period starting two
  turns below its basis's parameters, and the cap's two points lay two
  periods apart, of which `ChFi3d_Recale` took out one. Port patch 0800
  brings them together however many periods apart.
- OCCT made three stripes of the loop, which met end to end at the crest
  band's vertices and walked past each other (rounds 7 to 9). The edges
  between the crest band, the roundings and the flanks are tangent within
  1e-3 radian, but the stored helices' curves on their faces lie up to 4.8
  micrometres off their own, beyond 1.5 times their tolerances (1.4 to 3.7
  micrometres), where `ChFi3d::IsTangentFaces` compared them: the faces
  were taken for faces at an angle, the contour stopped at those vertices,
  and a walk across such an edge stopped there. Port patch 0801 compares
  the positions within ten times the edge's tolerance.
- The one contour's walk then ended 0.034 before the vertex where the
  rounding gives way to the second flank: the blend is 0.015 wide there,
  and its contact on the rounding met the rounding's own edge with the
  cone, within the stored edges' deviation. Port patch 0802 takes such a
  contact (on the spine's own edge, within ten times the tolerances of the
  vertex ahead, a face tangent to the contact's face holding the spine's
  next edge there) through the vertex.
- On the stored body the definition gives a change of volume of -15.3854
  against the file's -15.3941 (3.5e-7 of the body), with the file's 51
  faces; the blend faces 92.6087, 2.7428, 0.1901, 0.5863 mm² against the
  file's 92.6098, 2.7162, 0.1901, 0.5951 (the sliver as in the file).
  OCCT's result alone keeps the stored helix between the crest band and
  the first rounding, split by the blend, with its tolerance below its
  curves' deviation (BRepCheck); the import's fillet makes it valid.
- In the import the item runs on the replay's body, not the stored one,
  and falls back; the first deviation is the item's edges, not the
  rounding. Their names do not resolve ("missing in the replay"): one
  file edge is 0.03 mm from its replay edge. The edges the item takes away
  in the file then stand in for them, and two are wrong. The replay's
  rounding of item 142 splits the faces near one thread edge differently
  from the file's, so an edge of that rounding is taken although item 143
  does not touch it (the file's states before and after the item are the
  same there). The cone's edge down to the root (its faces 0.03 radian
  apart) is left out, because its blend is 0.015 wide and the file's state
  still has an edge within that of it. With the five right edges, OCCT's
  contour still stops at the vertex before that edge: the walk from the
  other edges takes the next edge only when its faces are at an angle,
  which `ChFi3d::IsTangentFaces` denies within 0.1 radian. The edge then
  starts a contour of its own, which takes the other four edges again from
  the other side. Taking such an edge, as patch 0022 does for the first
  edge of a contour (`MinNormalAngle` above the angular tolerance), gives
  the stored blend faces within 0.01 mm, but a result BRepCheck does not
  accept (a spline face's wire not closed). Not in this round. (The edges
  taken away: *Edges taken away, against the state before the item
  (mitcad#440)* below.)

**Item 27 (Fillet4, radius 0.6, both crest edges of a thread cut as a
groove, two chains over the per-turn faces): parametric when the item's
time allows (0803).** Round 9
left the floor's wire self-intersecting where the cap leaves the rim's
prolongation tangentially, and the change of volume 34.73 against the
file's 43.92.
- The cap and the rim run round the axis there, the cap inside the rim, a
  few micrometres from it for a while: the cap's approximation crossed the
  rim 0.053 from their common point (0.017 at the other chain's), whose
  tolerance was 5e-5, the rim's (an exact circle) 1e-7. Port patch 0803
  gives that point a tolerance taking in such crossings near it, where the
  two meet tangentially. The result is valid.
- The volume: every face of the stored state lies on the result within 4.9
  micrometres, and every face of the result on the state within 5.6 (the
  root's aside, which the distance computation does not find on the
  state's trimmed root cylinder, whose curves run six turns).
  The stored roundings lie inside the result's by 1.5 micrometres on
  average (at 197 of 200 samples), 1.7 mm³ over their 1125 mm²; the flanks
  within 5e-6. The rest of the figure is the stored state's volume, which
  its own integrations do not agree on: 30724.37 as the import measures it,
  30530.55 integrated adaptively (the trimmed root cylinder alone 316 apart,
  each flank of six turns 1 apart), 30728.70 from its triangulation (whose
  value moves by 0.9 with the deflection); the replay's result integrates
  alike all three ways (30733.56, 30733.56, 30733.57). The change is within
  the matching rules' 1e-3 of the body (3e-4 as measured), and the
  geometry within the geometric tolerance.
- The item has 60 s. Its first definition's geometric check agrees
  (within 0.01 mm), but the change matches its state only within 4.0e-4,
  so the import looks for a closer definition and takes the first one
  again after that. Of that time, 11.5 s go to translating the item and
  40.25 s (38.0 s in another run) to the geometric check's first
  measurement: 49 new faces, 16 points each, against the file's body,
  whose thread flanks are splines of 2484 mm² over the whole thread.
  Where the closer definitions end past the 60 s (61.4 s in one run), the
  import does not take the first definition again, and the item falls
  back. Where they end before (57.4 s), it is parametric. So the slow
  measurement itself takes the item's time. The likely cause is the
  boundary index's large-spline path (`analysis.cpp`), which lets go of a
  projection grid after each point; this was read from the code, not
  profiled.

| Item | Achieved / target | Cause of the gap |
|---|---|---|
| `db73863f` 143, the stored body | 1 / 1 verified (0800–0802) | |
| `db73863f` 143, the import | 0 / 1 parametric | the edges taken away stand in for unresolved names, one wrong and one missing; with the right edges, the contour split at a near-tangent edge and an invalid result |
| `db73863f` 27 | parametric in 1 of 2 runs | the item's 60 s: 40 s of it measuring its new faces against the file's thread flanks |

## A fillet contour that went round a closed chain for ever (mitcad#434)

With item 86 a fallback, the import of `db73863f` stopped tracing in item 88
(Fillet6, on item 86's stored body) and ran for over an hour. Two of the
definitions evaluated ahead were inside `BRepFilletAPI_MakeFillet::Add`
(`ChFi3d_Builder::PerformElement`, its upstream walk), prepending the 36
edges of a closed tangent chain to the contour again and again, and the
import waited for the first of them. The chain turns by 0.018 rad at three
vertices (and by 1.3e-3 rad at others, above the tight angular tolerance
of 1e-3). The downstream walk stopped at a turn before an edge not
selected; the upstream walk came round to it from the other side, where the
edge beyond is selected, and `StopsAtTurn` (port patch 0180) let it pass.
`Add` has no progress range, so the item's time did not stop it. The same
loop is there with main's port before the ninth round (a plate whose top
outline turns twice, built with OCCT alone, loops with both builds).

- Port patch 0840: neither walk of a contour adds an edge already in it, so
  round such a chain the contour is the chain's edges once each.
- The import stops waiting for a worker that does not end at its time
  (*A worker that does not stop at the end of its time* above).

`db73863f` now finishes in 5 min 23 s (debug build): 131 parametric, 7
fallbacks, 7 skipped of 145. Item 88 falls back after 27.7 s of its 60 s
(those definitions now fail: "the geometry kernel's computation of the
rounding along edge … (and 35 more edges of its chain) fails"), as its
other definitions do on item 86's stored body. Tests:
`geometry/tests/test_fillet_chain_loops.cpp` (fails without 0840: the
contour does not end in 30 s) and the import test
`a_definition_evaluated_ahead_that_does_not_stop_is_given_up` (a worker
whose kernel call ignores the stop for 15 s; the import ends in 4 s, the
next definition taken; 15 s and a fallback without the change).

## A guessed rounding where the file's item changes nothing (round 10 of mitcad#343)

`db73863f` item 131 (Fillet4, in the nut's component) names one edge,
12.82 long at a radius of 21.13 near the start of the nut's thread. That
edge is in no body of the replay (`missing_in_replay`), and it is in none
of the file's either: the decoder finds it only in an older state of the
nut's stored history, 7 of the item's 113 edge references are lost in the
file, the file's history step for the item leaves every body as it was
(its state is the state before it), and the replay's nut is that state's
body within 2.6e-4 mm. The item failed in the file; nothing before it
deviates.

The import then guessed edges from the next history states ("edges found
against ASM history state 84"): a rounding of the end edge of the
housing's thread crest, on a body of another component. The geometric
check took it: its 5 new faces lay on the file's (unchanged) body but its
blend of 18.74 mm², 0.045 mm off, 0.6 % of the new area, which passed as a
corner patch, and the faces' slack (73.5 mm³) covered its -0.53 mm³ where
the file's change is 0. That blend split the crest's helical edges, so the
edges item 133 (Fillet7) names were not in the replay, and item 133 came
in only by its retry on the file's exact state.

A face off on a body the other side's item left as it was is now never a
corner patch (`geometric::judge`; each new face is measured against the
other side's nearest body, and flagged when that body is one the other
side did not change). Item 131 is skipped as failed in the file, and item
133 is parametric on the replay, its inputs read, its faces the file's
within 0.01 mm. Tests: `no_corner_patch_where_the_file_changes_nothing`
and `a_body_the_file_leaves_is_measured_alone` in `core/import-replay/src/geometric.rs`.

`db73863f` imported again by content id (debug build): 131 parametric,
6 fallbacks, 8 skipped of 145 (before: 132 / 6 / 7, item 131 parametric
by the guess and item 133 only by the retry on the exact state). The
falling back items are those the design check names, and no item is
parametric after a retry on the exact state.

## Lens and corner faces of `db73863f` (round 10 of mitcad#343)

Items 75, 86 and 137 of `db73863f` after round 9, traced through OCCT's
fillet alone on the stored bodies before them; port patches 0820, 0821
and 0822, each with an OCCT-only test in
`geometry/tests/test_fillet_lens_overflow.cpp` that fails without it.

- **Item 75 (0820): two faces per corner, as in the file.** 0772's join
  made three faces per corner, the walked one generated by no edge. Now
  the join is walked from the first stripe's last piece's far end to the
  second's and split at the section through the first side's junction:
  on the stored body 57 faces, the file's count; per corner 428.98 and
  491.81 mm² against the file's 370.58 and 550.26 (the same sum within
  0.05: the file's edge between the two faces is no section of the ball),
  the plane pieces within 0.06, the other faces within 4e-4 mm of the
  file's; the volume 200007.857 against the file's 200007.088 (0772:
  200007.696).
- **Item 86 (Fillet4): fallback → parametric, verified.** With item 75's
  faces named by its two edges, all of its edges are found again on item
  75's result.
- **Item 137 (Fillet5, radius 2, three edges): 0 / 1 → parametric,
  verified (0821, 0822).** The key's end at z = 21.432 is a lens rounded
  on both edges, the S-shaped section of round 9's trace: the concave
  blend rolls on the lens's convex edge, the convex blend touches the key
  and the concave blend's pipe from outside, the shaft cuts the convex
  blend towards the lens's ends (0821). The key's other end is a lens too,
  only its convex arc rounded: the ball rolls on the lens's concave edge,
  then also on the key's side edges (each split by a vertex short of the
  lens's end) up to the lens's ends, where the rounding vanishes in a
  point (0822); OCCT walked back from the first step's start with the
  other end's edges, stopped at the split and failed at both corners. On
  the stored body the new faces are the file's within 3e-5 mm at that end
  (areas 0.0638, 0.1355, 0.2106, 0.2823 as the file's, 6.0214 against
  6.0199) and 2.4e-4 mm at the first (4.8443 and 3.9172 against 4.8439 and
  3.9223); the change in a box round each end -0.7785 against the file's
  -0.7794 and +0.6321 against +0.6325 mm³.

| Item | Achieved / target | Cause of the gap |
|---|---|---|
| `db73863f` 75 | 1 / 1, two faces per corner (0820) | |
| `db73863f` 86 | 1 / 1 parametric, verified | |
| `db73863f` 137 | 1 / 1 parametric, verified (0821, 0822) | |

Where a key's side runs into the lens's end unsplit, 0822 leaves the walk
ending where the contact reaches the side and the corner cutting the blend
by the shaft, within 0.016 mm of the rolling ball's faces.

Imported again by content id (features parametric, before → after):
`db73863f` 132 → 133 of 145 against main (item 137; 4 fallbacks, 8 skipped;
item 137 counts its inputs inferred, the fourth edge reference the file
lost left out, as the design check now names it); the
others unchanged against main: `6418ebb1` and `920a1cef` 41 of 50, `61526c66` 12 of 14,
`5ef2dbd5` 80 of 98, `52ba7a15` and `0c99fb88` 97 of 110; `a9e82a9a` and
`de7b8b9d` 183 of 199 (182 before main's merge, with 0822 alone too: item 11
parametric with main's changes).

## An endcap's notch corners and walls (round 11 of mitcad#343)

The design `4d3c4d1b` is an endcap that `52ba7a15` links (the copy in its
reference folder is the same file). Imported alone, with the assembly's
reference folder for its derived context, its two roundings that fell
back were traced on the history's stored bodies before them through
OCCT's fillet alone (with the dressup's contour stops).

- **Item 8 (Fillet2, radius 1, four corners of two notches in a ring's
  outer cylinder): fallback → parametric.** First deviation in OCCT, on
  the stored body itself (the replay's is the same within 1e-16). Three
  corners are sharp; at the fourth an earlier rounding of radius 0.4 left
  a strip 5 µm wide (0.0757 mm² over 15 mm) between the notch and the
  cylinder, and the file names the strip's edge for that corner. The
  strip is convex and bends more sharply than the ball, so no ball touches
  it and the start failed ("no ball of the radius touches both faces").
  Port patch 0880: at the spine, a side whose face the ball touches at no
  point goes on across that face, as a side that reached such a face
  further on already did; the ball touches the notch and the cylinder and
  the strip is consumed. On the stored body: change of volume −12.010691
  against the file's −12.010691, the file's 27 faces, every face within
  1e-4 mm of the file's; each corner's rounding 21.179018 mm², as the
  file's four. Test: `geometry/tests/test_fillet_strip_start.cpp` (OCCT
  only; fails without 0880).
- **Item 34 (Fillet7, radius 1, the top rims of two walls): fallback →
  parametric.** On the stored body OCCT's fillet with the dressup's
  contour stops gives the file's result (65 faces, every face within
  1e-4 mm, change −17.955960 against −17.955273), so the kernel was not
  the cause. First deviation: Sketch17 (item 24). Its linked arcs do not
  meet exactly (an arc's ends lie 1.5317 and 1.5560 from its centre), so
  the sketch is solved with them free; the solve shared that 24 µm with
  the own circle through the arcs' common ends, moving those ends 9.2 µm
  and the circle's radius from 15.368353 to 15.359863. Extrude13's wall
  then met the ring's cylinder (at 0.74°) 0.67 mm further along the rim,
  leaving two strips of the cylinder up to the top face at the wall's
  ends, so the wall's top edge was three edges and the rounding's end met
  the strips instead of the corner's rounding. Now, solved free, the
  points of the reference curves that hold the sketch's own curves (a
  point a constraint ties to an own curve or to its points, an end an own
  curve shares) stay where the file has them, tried before the free solve
  (`sketch::pinned_reference_points`): the circle keeps the file's radius,
  and the wall is the stored one. A pinned solve is taken only as it is,
  never with constraints the file's positions break left out: where it
  does not keep the file's sketch, the free solve follows as before. Test:
  `an_own_circle_through_a_free_linked_arc_s_end_keeps_the_file_s_radius`.
- **`52ba7a15` Sketch5 (item 18) stays parametric.** With item 34 built,
  the endcap's body after it is the replay's, not the stored state's, and
  two of its edges that Sketch5 (in another component) projects lie
  2.9e-4 mm from the file's curves (earlier micrometre noise of the
  endcap's replay, not traced to its item). A corner of Sketch5 that the
  file holds on a projected curve's end then had to move 2.9e-4 mm, within
  the geometric tolerance, but the check that the solve keeps the file's
  regions (`linked::regions_kept`) placed only points held on a curve as
  the solver does, not points held on a point: with the corner 2.9e-4 mm
  off that end the file's regions differed, and the sketch came in
  partial (the coincidence left out). A point held on a point now counts
  there in the file's regions too, and the sketch is the file's ("solved
  within the geometric tolerance: points up to 2.9e-4 mm"). Test:
  `a_corner_held_on_a_point_counts_as_there_in_the_file_s_regions`.

`4d3c4d1b`, imported alone (and as `52ba7a15`'s linked document): 31 → 33
of 35 items parametric (fallbacks 4 → 2, inputs read 18 → 20), both
roundings verified. The two left are no roundings and were not traced:
item 0 (the derived context, whose source body has no placement in the
file) and item 6 (Extrude3, its result not in the file's history; the
replay's change of volume −301.85 against the file's −330.66 mm³).
Unchanged, each imported alone: `52ba7a15` 97 / 110 (its linked endcap 31
→ 33 / 35), `61526c66` 12 / 14, `6418ebb1` and `920a1cef` 41 / 50,
`5ef2dbd5` 80 / 98, `a9e82a9a` and `de7b8b9d` 183 / 199, `db73863f`
134 / 145.

## Edges taken away, against the state before the item (mitcad#440)

`db73863f` item 143 (Fillet6, radius 0.5 round the thread's chamfer cone,
round 10 above) took its edges from what the next state lost when their
names did not resolve, and two of them were wrong:
- An edge of item 142's rounding (4.26 long) was taken, 0.004 to 0.009 off
  the next state. It is as far off the state before the item: the replay's
  rounding of item 142 splits the faces there otherwise than the file's,
  and item 143 does not touch it. So were ten edges of another body of the
  component, which the replay builds 0.38 mm off the file's, and 17 short
  edges between the replay's crest roundings (tangent, so no dressup's
  anyway), each 0.0002 to 0.004 off both states.
- The cone's edge down to the root (2.96 long, its faces 0.03 radian
  apart) was left out. The rounding there is 0.015 wide and lies 6.0e-5
  mm from the edge's middle, below the gone tolerance (1e-4 of the
  length, 3.0e-4 mm here).

`refs::consumed_edges` now measures the edges' middles against the file's
state before the item too (the cursor's state; its bodies but those the
replay and the next state both have as they are). An edge is gone only
where its middle is farther off the next state than off that one by the
gone tolerance. An edge between faces so nearly tangent that the item's
dressup there (the least of its sections at the edge's wedge) would lie
within the tolerance of its middle is gone where the middle moved off by
half that depth at least (and by more than 1e-6 mm); such edges are looked
for on the faces of the edges found gone, and not between faces within
1e-3 radian (tangent). The cone's edge: 6.06e-5 off the next state, as much
more than off the state before, its rounding 5.99e-5 deep. Where the state
before cannot be measured, the edges are judged by the next state alone,
as before. Test: `edges_gone_with_the_item_and_a_nearly_tangent_one` in
`core/import-replay/src/refs.rs` (a planar body whose quarter-turn edge and nearly tangent
edge are rounded, beside an edge as far off the state before and two
nearly tangent edges left).

The first deviation of round 10, the names, is gone on main: item 131's
guessed rounding of the crest's end (*A guessed rounding where the file's
item changes nothing* above) was what made the crest band's edge 0.03 mm
off the file's. The names now give the file's five edges (six of the
replay's, which splits the long flank edge in two), and the edges the next
state lost are the same six now (before: four of them, the edge of item
142's rounding and the other body's ten). The item still falls back: OCCT's
contour stops at the nearly tangent edge (mitcad#441).

| `db73863f` | Achieved / target | Cause of the gap |
|---|---|---|
| item 143's edges by their names | 5 / 5 | |
| item 143's edges from the next state | 5 / 5 (before: 4 / 5 and 11 others) | |
| item 143 parametric | 0 / 1 | the contour split at the nearly tangent edge (mitcad#441) |
| design, parametric (imported alone, debug build) | 132 / 145, 5 fallbacks, 8 skipped (before the same, item for item) | items 9, 88, 93, 137, 143 |

No other item's edges changed: the gone-edge counts of the other fillets
and chamfers that look at the next states (items in states 19, 51, 83 to
86) are those before, and every outcome is the same.

## A fillet contour into a near-tangent edge (mitcad#441)

With the file's five edges (above), item 143 of `db73863f` (Fillet6,
radius 0.5 round the thread's chamfer cone) fell back only in OCCT's
fillet. Traced through OCCT's fillet alone on the bodies before the item,
two first deviations, each with a port patch and an OCCT-only test in
`geometry/tests/test_fillet_near_tangent_contour.cpp` that fails without
it:
- The contour stopped before the edge down to the root, whose faces are
  0.03 radian apart. OCCT takes faces within 0.1 radian for tangent; port
  patch 0022 rounds such an edge where a contour starts from it, but a
  contour coming to it from the other edges stopped there, and the edge
  started a second contour that took the other four edges again. Which
  happened depended on which of the five edges comes first among the
  body's edges: on the stored body the near-tangent edge did, and the
  rounding was built (round 10); on the replay's body another edge did.
  Started from each of the five edges in turn on the stored body, OCCT now
  gives one contour and the same result (patch 0860).
- On the replay's body the one contour's result was invalid: the earlier
  rounding of the crest kept its whole edge with the cone beside the
  blend's end, its wire open. The blend comes to nothing at the vertex
  where that edge meets the edge down to the root (the cone and the
  rounding meet at 1.7 degrees there), and the walk's contact on the
  rounding ended at that vertex on the rounding's edge with the flank,
  within the vertex's tolerance, so no point of the blend lay on the edge
  with the cone (patch 0861 adds it there).

On the stored body, started from any of the five edges: a change of volume
of -15.3854 against the file's -15.3941, the file's 51 faces, valid after
the import's fillet. On the replay's body: valid, -14.6396 (3e-5 of the
body from the file's change), every new face within 0.01 mm of the stored
state (the faces farther off lie as far off in the body before the item).
In the import the item is parametric and verified: its new faces and the
file's within 0.01 mm, the change 0.115 mm³ from the file's.

`db73863f` imported again by content id (debug build): 133 parametric,
4 fallbacks, 8 skipped of 145 (before: 132 / 5 / 8): item 143 parametric,
items 9, 88, 93 and 137 fall back as before.

## A chamfer that names the rounding's operation before it (round 11 of mitcad#343)

The cap arcs in two pieces of `52ba7a15` and `0c99fb88` (the design in
`2d12d69c`), items 94 and 98, are parametric and verified since port patch
0550 (round 3, [*Fillet corners left after mitcad#341*](f3d-rounds-2.md#fillet-corners-left-after-mitcad341-mitcad343)); each design came
in with 97 of its 110 items parametric, falling back at items 34
(Mirror1) and 61 (Chamfer5) only. Traced on the history's stored bodies:

- **Item 61 (Chamfer5) failed in the file.** Its result number is item
  60's (Fillet9, 417): the history holds no operation of its own, and the
  file's state after it is the state before it (its bodies byte for byte).
  The one edge it names (1.4 long at x = -2.45, z = 0.686) is in an
  older state of the component's history only (the decoder: 7 states
  before 417); the stored state before the item has faces next to it
  (two 45-degree planes from z = 0.686 to 1.686), but not that edge. So no definition is found on the
  replay's body or on the file's exact state (the retry fails the same
  way), and none would give the file's state. The import fell back with
  a base feature; now an item that names an earlier item's operation is
  skipped as failed in the file, translated or not, as an item whose
  operation changed no body is (`StoredGeometry::item_shares_operation`,
  filled by `merge_by_timeline` in `core/ffi/src/f3d_import.rs`). A base
  feature still brings the body to the state there, which the replay's
  differs from by 0.068 mm (below). Test:
  `an_item_naming_an_earlier_item_s_operation_is_skipped_as_failed`.
- **Items 38 and 60 are not item 61's cause.** Item 38 (Fillet7, radius 1
  round the bottom of a block whose upright corners are rounded with 0.4)
  rolls the ball over each corner's arc, smaller than its radius. On the
  stored body before it, OCCT's fillet gives corner patches of 0.900 mm²
  where the file's are 0.666 mm², the straight roundings ending 0.08
  shorter, but every face within 0.023 mm of the file's (a change of
  volume of -9.067 against -9.038 mm³). The import verifies it; it is a
  slightly different corner, not worth the work by the owner's rule of
  18.10.2026. The replay's body at item 61 is 0.068 mm off the file's
  state there.
- **Item 34 (Mirror1) is not a failure of the file's.** Its operations
  (states 13 to 16) add 1.40 mm³; the closest definition, mirroring
  Extrude10, R-Pattern1 and Chamfer3, adds 1.75 mm³ (20 % more). That is
  a feature mirror, outside the fillet corners, and not traced here.

Imported again by content id, each design alone (debug build): `52ba7a15`
and `0c99fb88` 97 parametric, 1 fallback (item 34), 12 skipped of 110
(before 97 / 2 / 11; items 94 and 98 parametric and verified in both).
No other item of the designs below names an earlier item's operation, and
their counts are as before: `db73863f` 134 of 145, `6418ebb1` and
`920a1cef` 41 of 50, `5ef2dbd5` 80 of 98, `a9e82a9a` and `de7b8b9d` 183 of
199, `61526c66` 12 of 14.

## Edge overflows of round 11 of mitcad#343

The items left for this round in the edge-overflow class (mitcad#121),
traced through OCCT's fillet alone on the history's stored bodies before
them and on the replay's bodies (volumes integrated to 1e-7: OCCT's
default integration is up to 10 mm³ off on these bodies).

- **`61526c66` item 13 (Fillet3, radius 0.5 along a wall's foot on a
  floor): parametric, verified, inputs read (port patch 0890).** At both
  ends the wall's contact leaves the wall across its edge with an earlier
  rounding of radius 0.5 round the foot of the wall's end (a B-spline
  between the floor and a torus tangent to the wall), where that
  rounding's ball is the new one's (z = ±5.95). At one end the foot edge
  runs into that rounding's floor contact: 0540's end, the earlier
  rounding growing up to the section they share. At the other a step's end
  face is left between the floor, the wall and the earlier rounding (a
  triangle 0.16 by 0.02: the wall narrows to an edge 0.02 long), and the
  vertex's three edges are all concave. The first deviation was OCCT's,
  the same on the stored body: the corner found no face holding both
  contacts' ends, walked round the vertex, and the result had the blend
  running on to the floor's rim, invalid (a change of 9.1 mm³ against the
  file's 0.71). 0890 takes such an end as the earlier rounding's OnSame end
  across the sliver, which is consumed. On the stored body: valid, the
  file's 193 faces, a change of 0.70767 against 0.70667 mm³, the earlier
  rounding grown at both ends as in the file (2.6318 mm² against 2.6318).
  On the replay's body (which rounds item 12's sliver edge further, 0.0095
  mm³) valid as well.
- **`61526c66` item 5 (Fillet1, radius 1 round the disc's bottom rim):
  parametric, verified, inputs read (port patch 0891).** The edge is the
  rim cylinder's circle with a narrow rounding of the rim (a torus tangent
  to the bottom face); the ball overflows it onto the bottom face, its
  contact there a circle of radius 15.9, which runs over the mouth of a
  blind hole at the rim (rounded with 0.5, sharper than the ball). 0160
  walks the blend on over that gap and the mouth's rounding cuts it,
  in two pieces where the cut crosses that rounding's seam. The first
  deviation was OCCT's, the same on the stored body: the piece over the
  gap is walked on round the rest of the closed spine, and the section
  through its start was taken at the projection's foot on the far side of
  the disc (the line's own start is no foot), so the seam's crossing was
  not found and the stripe failed ("Chaining is impossible"). On the
  replay's body the spine starts in the gap: the cut's intersection lost
  its last stretch at the walked surface's end. On the stored body: valid,
  a change of -7.38905 against the file's -7.38889 mm³, the mouth's
  rounding 5.6721 mm² against 5.6708; on the replay's body -7.38891.
- **`a9e82a9a`, `de7b8b9d` item 159 (Fillet37) and `5ef2dbd5` item 89
  (Fillet29):** parametric and verified already (rounds 4 and 5: 0580,
  0581, 0600); item 89's new faces lie on the file's within 0.0043 mm.
- **`5ef2dbd5` item 90 (Fillet25, radius 0.86): not fixed.** Its edge
  names do not resolve: the file names item 89's cut by the notch wall
  (x = 0.5, 9.97 long), which on the replay ends 0.019 mm short of the
  file's (y -0.491 against -0.510; the replay's body before item 90 is
  0.033 mm off its state; on the stored body OCCT's item 89 ends at the
  file's points). On the stored body the file's edges fail in OCCT too, in
  two contours: the rim chain (seven edges round the band's top) walks to
  0.008 before both ends, where item 89's rounding, item 88's torus and
  the large torus are all tangent at the vertex (normals within 1e-4 rad);
  the contact leaves item 89's rounding for item 88's torus 0.013 from the
  vertex, and the walk between that torus and the large one, tangent to
  each other along their edge into the vertex, fails (the file's rounding
  ends in a point at the vertex). The notch wall's chain (x = 0.5, radius
  0.86 in a notch 1 wide whose walls the file's rounding consumes) fails
  on its own. What is needed: the end of a rounding vanishing at a vertex
  where three faces are tangent, a hair after its contact left its face,
  and the rounding of the notch wall's chain consuming the walls. (Done
  in the next section: port patches 0900 to 0909.)

| Item | Achieved / target | Cause of the gap |
|---|---|---|
| `61526c66` 13 | 1 / 1 parametric, verified, inputs read (0890) | |
| `61526c66` 5 | 1 / 1 parametric, verified, inputs read (0891) | |
| `a9e82a9a`, `de7b8b9d` 159; `5ef2dbd5` 89 | 3 / 3 (already) | |
| `5ef2dbd5` 90 | 0 / 1 | names: item 89's cut 0.019 mm off on the replay; kernel: a rounding vanishing where three faces are tangent, and the notch wall's chain |

Designs imported again by content id, each alone (features parametric
of all, before → after): `61526c66` 12 → 14 of 14 (inputs read for all
but item 12, inferred as before; history states matching 6 → 8 of 10;
the final body 1.6e-5 off the file's, where the fallbacks gave it
exactly); unchanged with 0890 and 0891: `5ef2dbd5` 80 of 98, `a9e82a9a`
and `de7b8b9d` 183 of 199, `6418ebb1` and `920a1cef` 41 of 50, `52ba7a15`
97 of 110, `db73863f` 134 of 145 (falling back items 9, 88 and 93 as its
design line says).
## A rounding kept on a narrow face's surface past its tangent edge (round 11 of mitcad#343)

`db73863f` item 88 (Fillet6, radius 5 along a closed tangent chain of 36
edges round the tetracore, whose contour's endless walk mitcad#434 ended)
was built once item 86 came in parametric (round 10), but fell back: the
import's check found three of the file's new faces (the torus of radius 10
it trims, 234.8 mm² each) up to 0.063 mm off and five of OCCT's up to
0.123 mm off, the change -245.52 against the file's -225.30 mm³. Run
through OCCT's fillet alone on the history's stored body before the item
(its 36 edges found there by their geometry), the same: -246.50 against
-225.30 (the replay's body gives the same result within 1 mm³), so the
first deviation is OCCT's, not upstream noise.

Along three of the chain's edges the face beside it is a cylinder band of
radius 55 about 2.47 wide, tangent to that torus beyond it; the rounding's
contact on the band lies 2.59 from the edge (the faces meet at 54.7
degrees), so it leaves the band across the tangent edge and comes back
where the band widens. OCCT took the torus for the band there and rolled
the ball on over it (the ball's centre 5 from the torus). The file's
rounding keeps the ball on the band's cylinder run on (its centre 5.0000
from that cylinder all along, 4.82 from the torus in the middle), and the
torus cuts the rounding in a crease of 0.19 rad, the band left as strips
1.62 long at the crossings. Port patch 0870 walks such a run again on the
band's surface unbounded and cuts it by the torus (OCCT-only test in
`geometry/tests/test_fillet_tangent_overflow.cpp`, which fails without it).

On the stored body: a change of -226.48 against the file's -225.30 mm³
(before -246.50), the area 37808.30 against 37808.36 mm², and at each place
the file's faces: the torus 234.74 against 234.83 mm², the rounding's face
145.81 against 145.86 mm² with its crease edge 30.590 long at 0.192 rad
(the file's 30.590 at 0.192), the band's strips 1.623 long. In the import
the item is parametric and verified (its state within 4.3e-4), evaluated in
1.8 s.

With item 88 parametric the tetracore is no longer the file's body after
it, and item 133 (Fillet7, radius 0.8 along the two helical edges of the
thread's crest) meets the replay's thread, whose faces are split per turn
where the file's are one face each:
- Its edges, each one edge of 519.5 mm in the file and four pieces in the
  replay, were not found: every chain of the body's shorter edges from the
  edge's start used up the search's steps first. The pieces are now looked
  for first along a face every piece lies on, then along faces each shares
  with the one before (`refs::chains_by_ends`; test
  `a_long_helix_split_per_turn_is_found_along_its_faces`).
- OCCT's fillet then fails at the thread's ends: the stripe's last piece
  lies on the flank's turn before the per-turn split edge, so the face the
  walk round the vertex at the end follows (`PerformIntersectionAtEnd`)
  does not hold that vertex, and the walk goes round it up to its limit of
  faces (the round 8 failure of the item, which port patch 0750 solved
  only where the flank is one face); another definition crashes in the
  corner of two edges (caught). On the stored body the same edges give the
  file's change. Not solved here: item 133 falls back again.

Traced further, the first deviation is the replay's thread itself: the
modelled thread sweeps its groove one turn at a time, so each flank, the
root and the crest are a face a turn, where the file keeps one face each
over all the turns (and one edge along each helix). The fillet's walk
then stops at the first per-turn split edge, where the crest's contact
has already crossed the end plane; its restart takes the crest side for
the extension past the end (a tangent plane), that walk fails and is
dropped as "not serious", so the blend covers nothing over the last turn
and past the end, the overlap of the two crest stripes (patch 0013) is
never found, and the end corner fails. Patching each of these in OCCT
would work around the split, so a rounding along the thread walks its
pieces as one face instead, and the body keeps them (branch
`343-r11-thread-join`):
- The per-turn groove and its boolean stay (a groove swept over many
  turns as one face cuts wrongly). The thread's pieces are numbered as
  before, so that saved references to them still resolve, and put on the
  exact screw surfaces of their section's edges
  (`detail::screwed_surfaces`, the edges' p-curves mapped and projected).
  The helices of those surfaces are interpolated with degree 7 at the
  Greville points of uniform knots, with as few spans a turn as keep them
  within 1e-7 mm, measured (about 15 at a radius of 22 mm, where the bound
  of the interpolation's error asked for 20 and the cubic ones need about
  370), and each piece lies on its own segment
  of the surface, its turn and a quarter of it on either side: booleans
  near pieces on surfaces of many turns of many spans take several times
  longer. On the test bolts the result is valid, its tolerances as
  before, its volume and area within 1e-6.
- A rounding joins, in its own working copy, the pieces of one surface
  its edges run along (`detail::join_surface_pieces`: the faces into one
  face, the edges along one curve between the same two faces into one
  edge, each piece's parameter scaled so that the joined curve keeps its
  speed across the joints; a face closed round its surface is left out),
  rounds them, and splits its result again where the pieces met
  (`detail::split_joined_pieces`): the faces over the pieces along the
  edges between them, each face it generated across its surface where
  those cuts end on it, each new face named as the piece it lies on or as
  the face generated from the piece of the edge it runs along, and put on
  its own segment of its surface. The faces away from the rounding keep
  their pieces and names.
- First deviations found on the way. Item 27 failed on the joined faces
  ("the rounding's end does not meet the face at the end") because the
  joined edges' speed jumped at each joint (pieces of a third and two
  thirds of a turn, each of its own parameter range): the guide's spans
  ran 377, 284, 144, 73, … mm a turn where the turns are 150 mm, and its
  end missed the face at the end. Item 133's joined edges were invalid on
  their faces because `BRepLib::SameParameter` left them alone (a new
  edge is flagged same-parameter), so the join was dropped and the walk
  stopped at the per-turn edges as before.
- Item 143 (Fillet6, radius 0.5 round the thread's chamfer cone). Its
  first deviation from the pipe thread's body: the thread's root is now an
  exact cylinder, so the cut's cone meets it in an exact circle with a
  tolerance of 1e-7 (on the pipe thread a B-spline edge of 5e-6); the
  faces it rounds lie within 1e-6 of the pipe thread's. The rounding along
  the cone and a flank ends where its contact on the cone runs into the
  root (an OnSame end, the root the face at the end). The contact's
  approximated curve lies 2e-6 off the cone and crosses the root at a
  small angle, so its point there lay 1.3e-6 below the circle, and the cap,
  the intersection of the blend with the root, started there too. The
  blend is tangent to the cone along its contact, so the cap leaves the
  circle tangentially, and as it is nearly tangent to the root there (the
  cap runs 15.3 long between points 0.87 apart, as the stored body's does)
  it rises from it slowly: it crossed the circle 0.026 from the vertex, a
  self-intersecting wire. On the pipe thread the same end had passed only
  behind a cap whose 3D curve lay 1.4 off its curves on the faces, with
  vertex tolerances of 2. Port patch 0910 takes the contact's crossing with
  the circle itself and starts the cap there (the end poles of its curves
  moved by 1.3e-5). The item is parametric and verified, inputs read, its
  change 0.063 mm³ from the file's (round 10's on the pipe thread: 0.115),
  its corner vertex 4e-5 from the stored body's.
- The threads took longer than the pipe thread's, the last one (Thread4, a
  60 mm internal thread) 8.7 s against 7.8 s on its own body before it
  (one run each). Traced: its grooves' cut checks every face of the body
  near them, among them the three other threads' pieces, which it does not
  meet; on the screw surfaces each such pair took 2 to 3 ms against 1 ms
  on the pipe thread's (a turn of 20 spans of degree 7 against a third of a
  turn of two spans of degree 10, sampled a point per span and degree).
  The helices now get as few spans as keep them within 1e-7 mm, measured
  (15 a turn there): 8.25 s. The rest, 0.45 s, is those pairs (a turn of
  15 spans) and the roots, exact cylinders the later booleans join into
  one face per thread (0.14 s); fewer spans need a looser surface (1e-6:
  item 133 falls back), a higher degree took longer (9: 8.35 s, 11: 9.0 s),
  and shorter segments do not change the sampling.
- In `db73863f` (imported alone, with main merged): 135 / 145, items 27,
  133 and 143 parametric and verified. The threads take 0.85, 1.87,
  0.77, 1.22, 1.60, 8.24 and 1.41 s where the pipe thread took 0.75, 1.64,
  0.68, 1.10, 1.56, 7.48 and 1.27 s.

| `db73863f` | Achieved / target | Cause of the gap |
|---|---|---|
| item 88 parametric, verified | 1 / 1 (0870) | |
| item 133 parametric, verified | 1 / 1 (thread join) | |
| item 143 parametric, verified | 1 / 1 (0910) | |
| design, parametric (imported alone, debug build) | 135 / 145, 2 fallbacks, 8 skipped (before 134 / 3 / 8) | items 9 (Sweep1) and 93 (Extrude3), not roundings |

## A bar's rim and notch walls rounded with 0.86 (round 11 of mitcad#343)

`5ef2dbd5` item 90 (Fillet25, radius 0.86), traced through OCCT's fillet
alone on the history's stored state before it (state 60) and on the
replay's body before it, and then in the import. The item rounds the
seven edges round a band's rim, which vanish at a vertex where three faces
are tangent, and the walls of a notch 1 wide at the bar's end: plane strips
about 0.06 high between the notch's floor (a cone) and an earlier rounding,
each a near and a far wall on both sides, whose balls touch the floor's
side and roll on the walls' far edges. State 61 has the rim, the near wall
on one side (x = -0.5) and both far walls rounded, 22 new faces; the
near wall on the other side (x = 0.5) is left sharp.

- **First deviation, item 89 on the replay (port patches 0900, 0901).**
  0600's piece walked again past an OnSame end kept the old surface in
  OCCT's map of generated faces, so the face of item 89's cut had no name
  and item 90's edge along it did not resolve (0900). The import adds item
  89's edges in an order that reverses its contour, and its cap's walked
  intersection with the face at the end kept a stretch that wandered past
  the end (0901). With both, item 89 on the replay ends at the file's
  points and its face is named.
- **The rim (0902).** The walk's contact crossed onto a face tangent to
  the other face along an edge into the end vertex a hundredth of the
  radius before it, where no ball fits; the stripe now ends there and the
  end is put at the vertex. On the stored body the rim's faces are the
  file's within about 1e-3 mm².
- **The notch walls (0903 to 0906).** A contact crossing onto the face of
  the spine's next edge no longer ends the walk (0903); a blend rolling on
  a wall's far edge rolls on along its line or circle past the edge's
  vertex and is cut by the face at the end, or ends in a point where the
  wall narrows to the spine's vertex (0904); the ball turning about a
  vertex between two far edges is walked back only over its turn and
  oriented as its neighbours (0905). The far wall at x = 0.5 still failed:
  its first start, 1.2 along its spine, rolls on a spline far edge that
  runs into the spine's first vertex, but the ball rolling on that edge
  and touching the earlier rounding touches that face's surface outside
  the face (u 1.07 against 0 to 1), so the walk failed at once. The wall
  at x = -0.5 built because its spine runs the other way and its first
  start lies at the far end. 0906 takes the next start where the first
  walk fails. The stored body's rounding does not roll on that edge
  either: it ends in the corner where the line edge starts, as OCCT's
  does. On state 60, all eleven edges: valid, the file's 22 new faces
  within 0.02 mm² each (1233.05 against 1233.03 mm² in all), a change of
  -4.249 against -4.053 mm³ (a 2 µm offset over the large faces).
- **The replay's body** (5.8 µm off state 60 by the import's measure,
  0.029 mm³ in volume) failed with the same eleven edges for three
  reasons, each a rounding of a few micrometres or less:
  - a step of the turn about the far wall's vertex landed on the point
    where the turn ends (the ball's side -5e-14 short of it), the release
    was missed and then rejected by an exact comparison (0907);
  - none of the 11 samples of the near wall's spine (17.7 long) fell in
    the millimetre where a start exists; they are taken four times as
    finely where none is found (0908);
  - the near wall's blend, rolling into the spine's first vertex, lost its
    width 0.005 before it (a hundredth of the radius is 0.0086); it now
    ends there and the end is put at the vertex (0909).

  With these, on the replay's body: valid, 22 new faces, 1233.04 against
  1233.03 mm², a change of -4.043 against the file's -4.024 mm³ from that
  body.
- **In the import:** the file's decoded definition (six wall edges, the
  near wall at x = 0.5 among them) cannot give state 61, which left that
  wall sharp; OCCT's contour along it fails as before. The import's guess
  of all eleven edges the item removes ("edges found against ASM history
  state 61") gives state 61 within 2.6e-6, the change exact: parametric,
  verified, inputs inferred, in 6.4 s. (Before 0907 to 0909 the import
  accepted the rim alone, 7 new faces against the file's 22, the walls
  within the corner-face allowance of 0.044 mm.)

| Item | Achieved / target | Cause of the gap |
|---|---|---|
| `5ef2dbd5` 90 | 1 / 1 parametric, verified, inputs inferred (0900 to 0909) | the file's own definition names a wall state 61 leaves sharp |

Designs imported again by content id, each alone, with main's port
patches 0870 and 0880 too (features parametric of all): `5ef2dbd5` 80 →
81 of 98 (history states matching 54 → 55 of 67; item 90 the only
change); unchanged: `61526c66` 14 of 14, `6418ebb1` and `920a1cef` 41 of
50, `52ba7a15` 97 of 110, `a9e82a9a` and `de7b8b9d` 183 of 199, `db73863f`
134 of 145 (falling back items 9, 93 and 133 as its design line says).

An OCCT-only test in `geometry/tests/test_fillet_corners.cpp` covers 0900
(it fails without the patch). 0901 to 0909 are verified on the design's
stored and replayed bodies only: their cases hang on the bodies'
micrometre deviations, on where the spine's samples fall, or on a wall
narrowing to the spine's vertex beside an earlier rounding. Synthetic
bodies tried for 0904 and 0905 (an ear on a tube whose top narrows to a
point under a chamfer, and one whose chamfer edge has a kink) did not
reproduce the design: the narrowing one fails in the blend's approximation
at the vanishing end, the kinked one either turns concavely (0390's piece
of the ball, not made there) or builds an invalid result; they were not
kept. The geometry tests that exercise 0331's ball rolling on a line along
a whole spine, 0390's concave turn and the overflow starts round a closed
spine caught three faults of the first versions of 0904 to 0906, fixed
before this round's import runs.

