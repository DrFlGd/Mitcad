# Fillets and chamfers of stored designs: 0990–1020

Rounds 18 to 20 of mitcad#343: a groove's rim rolled on a strip, a fin's
chamfers over slight creases, a bar's outline after 0980, a bore's rim
chamfered past a pocket, a pocket's rim rounded up to a torus's seam, and a
drafted peg's corners. The earlier rounds are in
[0950 to 0980](fillet-replay-0950-0980.md). Back to the
[port's README](../README.md).

## Overview

- **0990** ends a rounding rolling on an edge where the faces at its ends
  cut it past the point where its ball comes tangent to that edge's face:
  the blend is walked again with the face of its other contact unbounded up
  to that point, run on along its tangents and cut by the face its other
  contact runs into, the face at the end and the rolled edge's face, which
  grow up to the cut (a groove's rim under a slot's end wall rounded over a
  strip lower than the radius, cut at a notch's walls, mitcad#343).
- **1000** lets a chamfer's contact pass a slight concave crease of its face
  (0980's turn about the crease is a rounding's), keeps 0671's cut of an
  analytic chamfer to the spine's end off the unbounded narrow face of
  0531's computation again, and continues the B-spline surface of a face
  that grew past its bounds up to the face's bounds, the edges' curves on it
  kept (a fin's top chains chamfered over slight creases of its top and over
  side strips narrower than the chamfer, mitcad#343).
- **1001** takes 0980's guide stretch only where the spine turns at that
  vertex: along tangent edges the stretched guide's end lay off the spine's
  where an analytic piece's section is, and the walk from it failed (a bar's
  outline rounded up to where its blend vanishes, 0961's case, mitcad#343).
- **1010** lets the faces round a cut-out cut a closed rim's analytic blend
  over the gap where the blend's contact leaves its face along a whole edge
  of the rim: the gap runs from the only piece of another edge round to that
  piece's start, which the search for the piece after the gap did not reach
  (a bore's rim in two arcs chamfered past a pocket from below that breaks
  through the wall 1 under the top, mitcad#343).
- **1011** takes the ends of a rounding's intersection with a periodic face
  at its end in one period where one end lies past the face's edge across
  the face's parametric seam (a pocket's rim rounded up to an earlier
  rounding's torus whose seam lies at that end, mitcad#343).
- **1020** takes a face beyond a contact's kink that goes on smoothly into a
  face at the spine's end vertex for a face at the end, as 0750 takes one
  that holds that vertex, also where the next piece starts from the arc of
  the piece before (a drafted peg's corners rounded down to a block whose
  walls go on from the peg's sides at slight creases and whose corners are
  rounded with less, the walls and corners cutting the rounding at the
  creases instead of 0980's turn about both creases at once, mitcad#343).

## 0990: a rolled end cut past the ball's tangency

The patch 0990 comes from mitcad#343 (round 18): a groove of radius 5
under a slot's end wall, whose face is a strip 0.86 high between the groove
and the slot's concave roundings of 9 and its ceiling, the groove through a
notch in the wall's 1 mm thickness; the groove's rim on the strip rounded
with 10 rolls on the strip's top edges, and the notch's walls are the faces
at the ends.

### 0990-chfi3d-rolled-end-cut-by-the-faces-at-the-end-past-the-balls-tangency

Past the ends the walk went on with the contact on the groove run off across
its edge onto the wall beside the notch, and both corners failed ("Corner
OnSame : no point on arc", "OneCorner : bouchon non ecrit").
`RolledEndCutByEndFace` (tried at each end after `WalkedOnSameToEndFace`)
takes an end piece rolling on an edge Er of a face B whose other contact, on
N, ends on N's edge A into a face Fa, at a spine's end vertex of N, the face
across Er and the face at the end Be: it walks the blend again from the
piece's middle with N's surface unbounded, rolling on Er up to its end
(again a little short of it where the approximation of the singular last
section fails), runs its surface on along its tangents
(`GeomLib::ExtendSurfByLength`, C1) and cuts it by Fa from where the contact
on N crosses A, by Be, and by B's surface up to the point where that
intersection ends on the contact on Er, the ball tangent to B there (the
branches of the surfaces' intersection lines through the cut's points). Be
grows up to the cut along its edges with the face across Er and with Fa
prolonged and its curve with B, which grows up to it as well; Er is cut, and
the edge of N and Be and the spine's edge at the vertex go with the blend.
The end is put in the data structure with no corner (`PerformFilletOnVertex`
makes none at the end of a lone stripe already there).
`ChFi3d_ExtendAtSpineEnds` no longer looks up arcs of faces made of surfaces
unbounded in the edge/face map. On the design's body before the item: valid,
every face on the stored body's within 1.6e-4 mm both ways, the material in
a box round the rounding 207.3352 against the stored body's 207.3360 mm³;
before, both corners failed.

## 1000: a chamfer across a slight concave crease

The patch 1000 comes from mitcad#343 (round 19): a fin 1 mm thick whose
top edges, an arc and a profile's spline stretch in turn, meet at vertices
where the top's faces kink by 1.4 and 2.2 degrees (concave creases), both
top chains chamfered by 0.263 at 45 degrees, the side strips between the
top edges and the root's chamfer (a walked B-spline between a plane and a
torus) narrower than the chamfer along the spline stretches (0531's case).
The chamfer failed with an exception before any part of it was made.

### 1000-chfi3d-chamfer-across-a-slight-concave-crease-over-a-narrow-face-grown-faces-continued

0980's crease turn in `StartSol` (a face taken for tangent that meets the
face before at a concave crease of more than the angular tolerance is no
continuation: the ball turns about the crease's edge, rolling on it) took
the chamfer's piece after the arc's analytic one for a blend rolling on that
edge, which chamfers do not have ("PerformSurf Not Implemented"), and left
no piece for `ChamferOverNarrowFace` to find the narrow face by; a chamfer's
contact, at its distance along the face, passes the crease, so the turn is a
rounding's only. Computed again over the narrow face's surface unbounded,
0671's `ChFi3d_KPartCutChainToEnd` asked whether the arc's analytic piece
takes material off by the spine's edge with its two faces, one of them the
unbounded face, which does not hold the edge (`BRepTools::OriEdgeInFace`
threw); it now looks only at faces of the spine's edge
(`ChFi3d_EdgeOfFaces`), the narrow-face computation cutting the chamfer by
the faces beyond itself. The root's chamfer, grown up to the tip chamfer
along its sections continued straight (`ChFi3d_WideSurface`), kept its
surface, so the face ran past its surface's parameter bounds (17 of 82): its
geometry where OCCT evaluates the B-spline past its bounds, but tools that
keep to the bounds (`ShapeAnalysis_Surface`'s projections) found its points
nowhere on it. `ChFi3d_Builder::Compute` now gives each face of the result
running past its non-rational B-spline surface's bounds in a direction of
degree 1 that surface continued linearly to the face's bounds
(`ChFi3d_ContinueGrownFaces`: the end row of poles moved along the end
span's lines, the knot with it, the parameters as they were), each edge's
curve on the face stored again with the continued surface. On the design's
body before the item, replayed and stored: valid, every face of the file's
within 3e-4 mm of the result's and the result's on the file's within 1e-3
mm; before, the chamfer failed.

## 1001: the guide stretch only where the spine turns

The patch 1001 comes from mitcad#343 (round 19): the bar's outline of
0961's case rounded with 0.3, after 0980 joined the port.

### 1001-chfi3d-guide-stretch-only-where-the-spine-turns-at-the-vertex

Each side's chain of tangent edges starts at the vertex where its blend
vanishes, and the walked piece of its first edge ends 0.018 past the next
edge's start, where that edge's analytic piece starts. 0980 built that guide
from a stretch of the next edge as well (0.136), meant for a spine that
turns at the vertex (a fin's top edges kinked by a degree and a half). Here
the edges go on tangentially (2e-6 rad): the guide, approximated across the
vertex, passed its end 1e-5 mm and 5e-4 rad off the spine's point and
tangent, where the analytic piece's section is, and the walk's first step
from that section failed on all four chains, on the replayed and on the
stored body. `ChFi3d_PerformElSpine` takes the stretch only where the
spine's tangents just before and after the vertex lie more than a thousandth
of a radian apart. On the design's body before the item: valid, the stored
body's 174 faces, -2.2689 mm³, as with 0961 alone; 0980's fin still builds.

## 1010: a closed rim's gap round to its own first edge

The patch 1010 comes from mitcad#343 (round 19): a bore's rim of radius 85
in two arcs chamfered 1 on the top and 2 down the wall, where a pocket from
below (a cylinder whose side breaks through the wall) ends 1 under the top;
along the shorter arc the wall is a strip 1 high over the pocket's
ceiling, the arcs' vertices where the pocket's side meets the wall.

### 1010-chfi3d-kpart-closed-rim-gap-round-to-its-own-first-edge

The chamfer's contact 2 down the wall lies in the pocket all along the
shorter arc, so `SplitKPart` gives the longer arc one piece, its ends on the
pocket's side, and the shorter arc none.
`ChFi3d_FillKPartRoundGapsAcrossEdges` (the faces round from the pocket's
side back to it, the side, the ceiling and the side, cut the blend over the
gap) looked for the piece after the gap only on the other edges, never back
on the gap's own first edge, and left the gap to the walking chamfers do not
have: the chamfer failed. On a closed spine whose other edges have no piece
the search now goes round to the first edge again, a period on (the pieces
before and after the gap may be one), and the gap's edges are told apart by
their place in it, not by their numbers. The ceiling cuts the cone
lengthwise and keeps its part outside it, up to 84.5 from the axis, and the
pocket's side cuts it near the arc's ends, both growing up to the cut, as
the stored bodies have it. On the design's body before the item: valid,
every face on the stored body's within 2e-6 mm both ways, the change of
volume -522.6458 against the file's -522.6452 mm³; before, the chamfer
failed.

## 1011: an intersection at the end across the end face's seam

The patch 1011 comes from mitcad#343 (round 19), from the same design once
the chamfer of 1010 is built: the bore's inner rim of radius 82 rounded
with 0.5 where a pocket of radius 25.5 cuts into it (the rim an arc from
one side of the pocket round to the other, its torus' parametric seam at
its start), then the pocket's top rim rounded with 1 up to that torus.

### 1011-chfi3d-intersection-at-end-across-the-end-faces-seam-in-one-period

At the seam end the rounding's contact on the top runs past the torus' edge
(the torus grows up to the cut), and its point on the torus came out at u =
6.263, a period beyond the face's domain [-0.005, 5.692], while the other
end lay at u = -0.002 in it. `PerformIntersectionAtEnd` walked the
intersection of the blend with the torus between them the long way round: a
curve 3.3e6 mm long and a body of 1e12 mm³ that the checker took for valid.
On a periodic face at the end, an end of the intersection outside the face's
domain and more than half a period from the other end, which lies in it, is
now taken in the period next to that end (in u and v) before the
intersection is computed. On the design's replayed body and on the stored
body before the item: valid, -7.525 and -7.615 mm³ at the seam end (the
other side's rounding -7.618 and -7.512); the item's three roundings one
after the other give the stored body after it within 9e-4 mm both ways.
Before, the rounding gave the 1e12 mm³ body, which the import measured as no
solid, and the item fell back where its three roundings' first definition
had given way at its share of the item's time.

## 1020: a kink into a face going on smoothly into the end face

The patch 1020 comes from mitcad#343 (round 20): a square peg whose sides
are drafted by 3 degrees hangs from a plate and runs, 4 below it, into a
block whose walls go on straight down from the peg's sides there (concave
creases of 3 degrees) and whose vertical corners an earlier rounding made
cylinders of radius 0.4; the peg's sharp corners poke out of those
cylinders down to 6.2355 below the plate, where its corner edges end on
them. The four corner edges are rounded with 0.5. In the stored result the
rounding ends at the creases, and the block's walls and corner cylinders
grow up to where they meet it (to 3.841 and 3.447 below the plate), the
corner below them gone.

### 1020-chfi3d-kink-into-a-face-going-on-smoothly-into-the-end-face-also-at-a-pieces-start

Before 0980 the walls were taken for the sides continued (their normals
within a tenth of a radian) and the ball rolled on down the block's corners,
which it rounded with 0.5 (the material taken 1.127 against the stored 0.932
mm³; the import accepted the 0.04 mm as corner faces). With 0980 both
contacts reach their creases at one section and the ball turned about both
creases' edges at once: the fillet failed. The walls are faces at the end as
`KinkIntoEndFace` (0750) has them, but they hold no vertex of the spine:
they go on smoothly into the corner's cylinder, which holds the corner
edge's end vertex. `KinkIntoEndFace` now also takes a face beyond the kink
that goes on smoothly (`ChFi3d_SmoothIntoFace`: the normals within the
angular tolerance all along an edge of both) into a face at the end vertex
other than the face before. That check, with 0980's rule that a face of an
edge of the spine is no face at the end (`ChFi3d_KinkIntoEndFaceOfSpine`),
was made only where a walk ends on the arc (`ComputeData`); `StartSol` makes
its new kind too (a face beyond that holds no end vertex itself), where the
next piece starts from the arc of the piece before: the corner's analytic
piece, a cylinder between the planar sides, ends at the creases, and for a
corner edge running from the block up the walk from there to its first
vertex started over them. That side now returns as at an end before a face
at an angle, and the end is built as where a walk ends on the arc. A face
beyond that holds the end vertex itself stays the walk's there, which runs
on it up to the vertex: taken for a face at the end in `StartSol`, 0980's
fin top (`5ef2dbd5` item 87) failed at its corner. On the design's replayed
body before the item: valid, the material taken 0.931662 against the stored
0.932227 mm³, every face of the result within 1.4e-4 mm of the stored body
after the item and the stored body's within 8.2e-5 mm of the result.

## Reports and tests

Each of these tests fails without its patch.

- 0990: mitcad#343 and a block's slot rounded with 9 along its ceiling's
  long edges, a groove of radius 5 under its end wall through a notch, the
  groove's rim on the wall's strip rounded with 10, in
  `geometry/tests/test_fillet_rolled_end_cut.cpp` (valid, both notch walls
  grown up to the cut above the groove's rim, the roundings of 9 grown past
  the slot's wall, no face of no area; the fillet fails without the patch).
- 1000: mitcad#343 and a fin's tip, a line and a cubic turned up from it by
  2 and 4 degrees, chamfered 0.4 at 45 degrees along both, with the side
  whole and with its foot chamfered so high that the side strip is narrower
  than the chamfer along the cubic, the foot's face planar and a B-spline of
  degree 1, in `geometry/tests/test_chamfer_crease_strip.cpp` (valid, the
  material taken the section's triangle along both within 1e-4 where the
  side is whole, every face inside its surface's bounds, the B-spline foot's
  result the planar one's; without the patch the chamfer fails, without its
  second change the narrow cases are invalid, without its third the foot's
  face runs past its surface's bounds).
- 1001 has no such test: its stretch needs the next edge's analytic piece
  to start a hair past a vertex of tangent edges, as the design's faces
  split it, which no synthetic body reproduced; verified on the design's
  replayed and stored bodies, where the rounding fails with 0980 and without
  1001.
- 1010: mitcad#343 and a disc's rim of radius 85 in two arcs split where a
  pocket from below breaks through its wall 1 under the top, chamfered 1 on
  the top and 2 down the wall, either arc given first and the seam at either
  corner, and the rim as one closed edge, in
  `geometry/tests/test_chamfer_rim_cutout.cpp` (valid, the material taken
  within 1e-6 of the body's part outside the chamfer's revolved section, the
  pocket's ceiling cut back to 84.5 from the axis; the two-arc chamfers fail
  without the patch, the one-edge rim builds without it as well).
- 1011 has no such test: bodies with a bore's rim rounded where a pocket or
  a slot cuts into it and the pocket's or slot's rim rounded up to that
  torus built or failed alike with and without it, their contacts not
  running past the torus' edge at its seam; verified on the design's
  replayed and stored bodies, where the seam end's rounding gives a body of
  1e12 mm³ without it.
- 1020: mitcad#343 and a square peg drafted by 3, 4 and 5 degrees hanging
  from a plate into a block whose walls go on from its sides at the creases
  and whose vertical corners are rounded with 0.4, the peg's corner edges
  (given either way along the corners) rounded with 0.5, in
  `geometry/tests/test_fillet_crease_into_end_walls.cpp` (valid, the
  block's corner cylinders from its foot up past the creases, the material
  taken within 1e-3 mm³ of the peg rounded alone with the block carried on
  up into it; with 0980 and without the patch the fillet fails, before 0980
  it rounds the block's corners with 0.5).
