# Fillets and chamfers of stored designs: 0930–0942

Rounds 13 to 15 of mitcad#343 (a ring's rims, roundings ending on a rod, a
torus's edge interrupted by a rod, a key's top edge) and a design's chamfer
that hung in the kernel (mitcad#495). The earlier rounds are in
[0870 to 0911](fillet-replay-0870-0911.md), the later ones in
[0950 to 0980](fillet-replay-0950-0980.md). Back to the
[port's README](../README.md).

## Overview

- **0930** lets a rounding roll on another's contact line (0016) where the
  two spines run opposite ways: its side on the line runs from the line's
  end to its start (both rims of a ring rounded, the rims' circles turning
  opposite ways, mitcad#343).
- **0931** runs the seam of the face at a corner's end on to the corner's
  cut where the cut crosses the seam's line past the seam's end at the
  corner's vertex (roundings of three bores' curves ending on a rod whose
  seam runs into one of their end vertices, mitcad#343).
- **0932** ends a walk that 0077 cut before a singular section past its
  spine's end at the cut (it was taken up again from there).
- **0933** joins two stripe ends over a narrow face that pokes through the
  ball's path, the contact never leaving its face, one blend holed by that
  face (a rounding of a torus's edge with a plane that a rod interrupts
  twice, mitcad#343).
- **0940** rounds over a narrow face whose far edge is a concave crease on
  that face's surface unbounded, as 0531 chamfers: the face beyond cuts the
  blend's side on the narrow face from where the contact crosses the crease
  up to the other contact, and grows up to the cut; at the spine's ends a
  second face beyond takes over where the two faces' junction, run on along
  its own curve, meets the blend (a key's top edge rounded wider than the
  key's side is high where it runs out against a bore's wall, mitcad#343).
- **0941** takes the crossings of a gap (0142) on a closed line of the other
  contact in the period that follows the piece before the gap, where the gap
  does not run across the spine's start (a boss's foot chamfered past a
  plate corner with a slanted side, the boss's seam just before the gap,
  mitcad#495).
- **0942** stops ShapeFix's repair of intersecting wires from intersecting
  the same two edges for ever where two arcs of one circle in two wires meet
  end to end (the repair of 0941's invalid result never ended, mitcad#495).

## 0930 and 0931: a ring's rims and roundings ending on a rod

The patches 0930 and 0931 come from mitcad#343 (round 13): both rims of a
ring rounded, and three roundings ending on a rod.

### 0930-chfi3d-rounding-riding-anothers-contact-line-against-it

0016 makes the later of two roundings of a face narrower than both roll on
the earlier one's contact line, and `ApplyRides` makes the two blends meet
on that line, the earlier blend's, run its spine's way; the later blend's
side was set on it from its start to its end. A spine follows its edge's
curve, and the two circles of a ring's band, which start at one point, may
turn opposite ways round the axis (a ring 2.5 thick rounded with 1.5 on both
rims, whose bottom circle came from a cylinder standing upside down): the
later blend then used the line against its own direction, and its face came
out reversed (its area −1617 mm²) with a wire not closed. `ApplyRides` now
finds which way the later blend's own line on the support runs along the
earlier one's (`rideRunsAgainst`, the tangents at 30 and 70 % of it); where
against, that side runs from the line's end to its start, its ends and
points are the earlier blend's the other way round, and once the stripes are
in the data structure the line's transition on the later blend's surface is
turned (`turnRidesAgainst`), as for blends cut by each other between two
crossings. On the ring's replayed body the change of volume is the file's
within 1.4e-7 of the stored state in either order of the rims; on the stored
body, whose circles turn the same way, it is unchanged.

### 0931-chfi3d-seam-run-on-to-a-corners-cut-past-its-end

Three roundings of radius 0.5 along the curves where three bores meet end on
a rod of radius 2.5 in their middle, whose cylinder face runs all round with
its seam from its bottom up to the vertex where two of the bores' curves
meet, one rounding's end vertex. The rounding takes material off there, so
the rod's face grows up to the corner's cut (the blend's intersection with
the rod), which crosses the seam's line 0.31 past the seam's end.
`PerformOneCorner` split the cut at the seam only where the crossing lay on
the seam's edge or at one of its vertices; the cut was left whole across the
line, the seam ended at the vertex the rounding removes, and the rod's face
came out with a wire not closed. Where the seam ends at the corner's vertex
and the cut's curve on the face crosses the seam's line beyond that end (the
point on the face's surface, within the tolerances), the cut is now split
there and the seam runs on along its line from the vertex to the crossing: a
curve on the face's iso-line through the seam (the surface's own
parametrization; the seam's curve, a B-spline here, cannot be prolonged), on
both sides of the seam, each the way the seam runs there. The seam stays
whole. On the replayed body the rod's face has the file's area (412.4848
against 412.4839 mm²) and every new face lies within 6e-4 mm of the file's.

## 0932 and 0933: a torus's edge interrupted by a rod

The patches 0932 and 0933 come from mitcad#343 (round 14): a rounding of
radius 0.5 on three pieces of the edge between a large torus and a plane,
interrupted twice by a rod's narrow strips (2.34 and 2.26 mm²), the pieces'
contacts on the plane running past their ends onto a torus of radius 0.6
round the rod's foot.

### 0932-chfi3d-walk-cut-before-a-singular-section-ends-there

0077's `ChFi3d_CutSingularExtension` cuts a piece's walk past its end at
18.0021 (its spine 17.92 long), where the contact on that torus speeds up
from 1.25 to 20.5. The cut end lies on no arc, so `ComputeData` left it open
and the walk was taken up again from the cut into the same sections; the
corners there failed. The function now returns the ends it cut and
`ComputeData` closes them (`intl`/`intf` 1): the cut end is the blend's end
for the corner. The common points at the walk's end are kept (resetting them
breaks a corner the stored ones build: the arc of a cylinder's top cut by
two planes, test `fillet_kernel`).

### 0933-chfi3d-ends-joined-over-a-narrow-face-that-pokes-through-the-blend

In the file each pair of ends is one blend over the strip C between them;
over one strip the contact on the small torus N leaves it across C and comes
back (0700's cut), over the other it stays on N and C pokes through the
blend, a hole of 3.66 mm round a lens of the rod (0.888 mm²).
`JoinOverNarrowFaces` took C from the arc an end's contact ends on; ends cut
short lie on none, and C is now also the face across an edge of N that holds
both vertices. Where the walked contact never leaves N, the join is made
when C cuts the walked blend in a closed curve and lies wholly beyond the
walk's start section (on it at the end's vertex): one piece from the start
to a section beyond C, the curve and the other end, which `PierceBlends`
holes. On the replayed body the rounding, as the three pieces or with the
rod's two edges, builds in every order of its contours within 0.0033 mm³ of
the file's change (7.5193 mm³); before, two corners failed.

## 0940: a key's top edge over its narrow side

The patch 0940 comes from mitcad#343 (round 15): a key's top edge rounded
with 2, the key's side B a strip between that edge and a bore's wall C (a
torus) across a concave crease, 2.2 high in the middle and lower than the
ball towards both ends, where B and C end at the key's end roundings of
0.5 (C2).

### 0940-chfi3d-rounding-over-a-narrow-face-cut-by-the-face-beyond-its-concave-edge

Past the crossing where the ball's contact on B reaches the crease, the ball
rolled on the crease (0011's obstacle), sticking out of the key's side, and
the corners at the ends (four edges each) failed. The file's rounding is B's
cylinder all along, cut where C's wall runs through it, C grown over the
strip up to the cut; at each end C2 cuts the blend from where the junction
of C and C2, run on along its circle, meets it, down to the key's top.
`ChamferOverNarrowFace` (0531) now takes a rounding whose piece rolls on a
concave crease of a face of its edges, or whose contact ends on such a
crease away from the spine's end vertices while the other contact goes on
(over a convex crease the ball still rolls on the edge, 0620).
`ChamferCutByFacesBeyond` takes a crossing where the other contact lies
outside C's material and the contact on B's surface past it inside
(roundings only): C cuts B's side from the crossing up to where the other
contact enters C's material, the piece ending in a point there. At the
spine's ends with two faces beyond B the junction runs on along its own
circle, line or ellipse where that lies on both faces; where it meets the
blend before that point, C2 cuts the blend from there; where it meets the
other face first it bounds the growth instead of the edge's tangent line,
and an edge of the other face and C2 through that point is cut there. Curves
of two faces that the intersection lines miss at a vertex where the faces
touch are found by points, and the cuts' curves are moved into the periods
of the faces beyond. On the design's stored body: valid, every new face on
the file's within 1e-3 mm (the file's sliver of B of 0.0024 mm² at one end
lies 0.017 mm off), the change of volume -9.5996 against the file's -9.5954
mm³; before, two corners failed.

## 0941 and 0942: a chamfer that hung in the kernel

The patches 0941 and 0942 come from mitcad#495: a design's chamfer hung in
the kernel. Four bosses of radius 7.5 under a plate, their feet chamfered
by 13 (each boss 13 long, so its side goes with the chamfer); two of them
stand under the plate's rounded corners (radius 20 about the boss's axis)
whose one side is slanted, tangent to the corner's arc, so that the contact
on the plate's underside (radius 20.5) runs 0.5 beyond the outline round
the corner and along both sides (0142's gap, cut by the chain of the faces
beyond). The bosses' seams lie just before the gap.

### 0941-chfi3d-kpart-chain-crossings-in-the-period-after-the-gaps-start

`ChFi3d_KPartEdgeCrossing` takes a crossing's parameter on the other
contact's line (the boss's circle) in the period around the middle of the
piece before the gap; a long gap ending towards the spine's end got its last
crossing a period early (-0.286 for 5.997), and the last piece of the chain,
from there to the next piece's start (6.218), ran more than once round the
circle: the boss's bottom face got edges covering more than its rim, the
cone's seam a tolerance of 58 mm, an invalid result.
`ChFi3d_KPartChainPieces` now takes each crossing's parameter in the period
that follows the end of the piece before the gap (or the chain's given
start) where the gap does not run across the spine's start; a gap across the
start keeps them as before (0145, 0146). On the design's replayed body:
valid, the change of volume +25112.5803 against the file's +25112.5808 mm³;
before, invalid (two of the four bosses +618 and -733 mm³ off).

### 0942-shapefix-intersecting-wires-cut-reported-only-where-cut

The healing of that invalid result (`ShapeFix_Shape`, as the dressups'
`healed` does) never returned.
`ShapeFix_IntersectionTool::FixIntersectingWires` intersected an arc of one
wire of the boss's bottom face with the next arc of the same circle in
another wire; the intersection was a segment of no length at their common
vertex, both edges were to be cut there and the pair intersected again
without counting a modification. `CutEdge`, for an edge that is not same
parameter with a trimmed curve as its p-curve, cut only a trimmed line but
answered that it had cut any trimmed curve; now it answers so only where it
cut (a line cut to the range it has is no cut either), and the repair goes
on to the next pair. With 0941 alone the design's chamfer builds; with 0942
alone it fails at once ("an invalid shape") instead of hanging.

## Reports and tests

Each of these tests fails without its patch, except where noted.

- 0930: mitcad#343 and a disc 2.5 thick, its rims' circles turning opposite
  ways, rounded with 1.5 on both rims in either order, in
  `geometry/tests/test_fillet_ride_against.cpp` (valid, the volume the
  section's revolution has within 1e-6; invalid, 1322 against 3035, without
  the patch).
- 0931: mitcad#343 and a roof whose ridge runs into a rod standing on it,
  the rod's seam a B-spline along the line through the ridge's end, in
  `geometry/tests/test_fillet_seam_run_on.cpp` (valid, the material removed
  the blend's section along the ridge less the rod within 1e-6). It does not
  fail without the patch: on that body OCCT's topological build closes the
  rod's face without it as well, so the test checks 0931's way only; the
  design's failure is verified on its replayed body.
- 0932 has no such test: its cut needs a contact speeding up some
  twentyfold past the spine's end on a small torus, which the synthetic
  posts of 0933's test did not reach, verified on the design's replayed
  body, where the rounding with the rod's edges fails without it.
- 0933: mitcad#343 and a post on a plate, the edge round its foot broken by
  two caps that poke through its rounding of 0.5, posts of radius 1 and
  0.8, in `geometry/tests/test_fillet_pierced.cpp` (valid, the rounded post
  with the caps fused within 2e-5 mm³, both caps' lenses; invalid without
  the patch).
- 0940: mitcad#343 and a key in a spherical hollow, its top edge rounded
  with 2 where the key's side runs out against the hollow's wall, in
  `geometry/tests/test_fillet_narrow_cut.cpp` (valid, the material taken off
  the corner outside the rounding's cylinder inside the hollow within 1e-6
  mm³; the key whose side runs out before its ends builds without the patch
  as well, the key cut off before that by end walls is invalid without it).
  Its first two cases do not fail without the patch; the key with end walls
  fails without it.
- 0941: mitcad#495 and a boss under a plate corner with a slanted side, its
  foot chamfered past the corner, the boss's seam at 120, 150, 180 and 300
  degrees, 13 and 15 long, in `geometry/tests/test_chamfer_overflow.cpp`
  (valid, the boss with the chamfer's cone cut by the plate's outline within
  1e-8 relative; invalid without the patch for the seams from 120 to 180
  degrees).
- 0942: mitcad#495 and a planar face of two wires whose arcs of one circle
  meet end to end, in `geometry/tests/test_shapefix.cpp` (the repair of
  intersecting wires returns with nothing to repair; it never returns
  without the patch).
