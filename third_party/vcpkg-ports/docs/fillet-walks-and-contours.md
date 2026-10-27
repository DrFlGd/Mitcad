# Fillet walks over rims and contours: 0160, 0180, 0190, 0250, 0260, 0840, 0860, 0861, 0920

How OCCT's fillet walks over a rounded rim it cannot touch (mitcad#238,
mitcad#261, mitcad#307, mitcad#308), and which edges a contour takes and
with what radius (mitcad#241, mitcad#434, mitcad#441, mitcad#464). Back to
the [port's README](../README.md).

## Overview

- **0160** lets the fillet's walks pass over the rounded rim of a hole the
  ball cannot touch (mitcad#238).
- **0190** takes 0160's walk round a closed rounding that consumes a face of
  its edge (mitcad#261).
- **0250** lets a walk go on along a face of its contour that 0160 took for
  a rim (mitcad#308).
- **0260** carries a fillet's contour across a small notch in the rim it
  rounds onto the next edge of that rim (mitcad#307).
- **0180** lets a contour stop where its edges turn (mitcad#241).
- **0840** keeps a fillet's contour from taking an edge of its own again, so
  that its walk round a closed chain that stops at a turn ends (it went
  round for ever; mitcad#434).
- **0860** carries a fillet's contour on into an edge asked for whose faces
  meet at a few hundredths of a radian, and **0861** cuts the spine's own
  edge from the face of a contact that ends at the vertex where the blend
  comes to nothing, on another edge of that face (a thread's chamfer cone
  rounded down to the root; mitcad#441).
- **0920** keeps a radius law the caller sets on a fillet's contour, which
  the spine dropped (a B-spline radius along the edges, mitcad#464).

## 0160: walks over a rounded rim

The patch 0160 goes on with the rolling ball fillet's walks (mitcad#238).

### 0160-chfi3d-walked-blend-over-a-rounded-rim

A walk whose contact reaches an edge into a face that continues its face
tangentially but bends towards the ball more sharply than the ball (a hole's
or cut-out's rim rounded with a smaller radius) no longer goes onto that
face (`StartSol`): the ball cannot touch it. Inside the spine the blend is
walked on over the gap on the face's own surface, unbounded, until the
contact comes back into the face, and the rim cuts it
(`WalkedBridgeOverGap`, with 0055's `ChFi3d_KPartCutPiece`; where the cut
crosses the rim's seam, two pieces, the seam's end under the blend dropped);
at the spine's ends it rolls on past the face's boundary on the face's own
surface (0076). Of several arcs of the cut, `ChFi3d_KPartCutCurve` takes one
on the rim's face first. Before, the walk's first step left the rim at once
and the fillet failed.

## 0190: round a closed stripe

The patch 0190 goes on with 0160's walks over a rounded rim (mitcad#261).

### 0190-chfi3d-consumed-face-round-a-closed-stripe

A closed rounding whose start overflowed a face of its edge all round (a
disc's bottom edge rounded with 1, the disc then turned down by 0.1 and the
edge between the turned cylinder and that rounding rounded with 1: the ball
touches the cylinder and the bottom, the earlier rounding lies wholly
between the contacts) and whose contact on the face beyond crosses a hole
with a rounded rim. 0160 refused its walk over the rim where the face the
contact runs on is not a face of the spine's edges, and the fillet failed
("Chaining is impossible"). Round a closed spine such a face beyond a
consumed face (`ChFi3d_ConsumedSpineFaces`: a face of the spine's edges that
no surface data lies on) is taken; the rim's edges with the consumed face
vanish with it (no edge to grow at their vertices under the blend), and the
topological build drops the consumed face once its neighbours are cut. Where
the other contact crosses a seam of its face inside the walk over the rim,
the stretch's cut piece ended at the section through the spine's point
there, not at the walked piece's own end section (the rolling ball's
sections are not in the guide's normal planes: 0.011 apart), and its point
on the seam was looked up on the seam's first occurrence; the seam kept its
piece under the blend and with it a part of the cylinder and the consumed
face (a valid shape 129 mm³ smaller than the file's). A piece now ends at
the walked piece's own end section with the walk's common point on the arc
(`ChFi3d_KPartCutPiece`'s `AtFullEnds`, `ChFi3d_KPartSectionCutAt`), and the
next stretch starts on the arc as a walk goes on across a seam.

## 0250: a walk on along a face of the spine

The patch 0250 keeps 0160's bridge over a rounded rim to faces that are
not along the contour (mitcad#308).

### 0250-chfi3d-walk-on-along-a-face-of-the-spine

0160 made `StartSol` refuse a face beyond a tangent edge that bends towards
the ball more sharply than the ball (`ballCannotTouch`), which also refused
a face the contour itself runs along: a box whose vertical edges are rounded
with 2, its bottom chamfered round the box, then the edge between the
chamfer and a side face rounded with 5 (the contour goes on round the
corners, between the chamfer's cone and the corner's cylinder). The spine
turns with that face and the blend's sections touch it; the walk went on
onto it before 0160, and after it no face continued the contact and the
fillet failed ("Chaining is impossible"). A face that borders an edge of the
spine (`faceAlongSpine`) now continues the contact's face whatever its
curvature along the spine; 0160's test applies to the other faces. It also
lets a top rounding go on round vertical corners rounded with a smaller
radius than its own (mitcad#297: a rim of tabs and notches with corners
rounded with 1 under a rounding of 1.5; there the corner's cylinder bends
towards the ball inside the solid).

## 0260: a fillet carried across a notch in its rim

The patch 0260 goes on with 0241's roundings across a notch (mitcad#307);
MitCad's fillets use it.

### 0260-chfi3d-fillet-carried-across-a-notch-in-its-rim

A rim rounded on one side of a small notch whose edges are not rounded, the
next edge of the rim beyond the notch not given (a plate's top edge along an
arc of a cylinder, broken by a tapered hole whose top edges are 0.28 to 0.29
long, 0.25 deep and 0.57 wide, the radius 0.5): the files' bodies round that
edge too, with one torus face over both edges and the notch, cut by the
notch's faces. OCCT ended the contour at the notch and left the edge beyond
it sharp (the result was invalid).
`BRepFilletAPI_MakeFillet::CarryChainsAcrossNotches`
(`ChFi3d_FilBuilder::AddAcrossNotches`), called after the contours are
added, follows the boundary of a face of a contour's end edge from its end
through edges no contour holds, each within the fillet's contact distance on
that face of the end edge's line or circle (R tan(a/2), a the angle between
the faces' normals) and within the ball's diameter of the end, to an edge of
that face on the same line or circle whose other face lies on the surface of
the end edge's other face; that edge is added as a contour of the same
radius, just before the contour it is carried from, and 0241 joins the two
ends over the notch (the joined face is the later stripe's, so it is
generated by the given edge). Constant radii, lines and circles only; a
notch wider than the ball (a groove 1 long and 0.0043 deep beside a rounding
of 0.3, which the files' roundings do not cross) is left as it is. Without
the call nothing changes.

## 0180: chains stop where the edges turn

The patch 0180 lets a fillet's contour stop where its edges turn although
the faces go on tangentially there (mitcad#241); MitCad's fillets use it, so
that a tangent chain runs on only where its edges continue each other, as
in the `.f3d` files' roundings, and `geometry/tests/test_fillet_chains.cpp`
checks it with OCCT alone.

### 0180-chfi3d-chains-stop-where-the-edges-turn

OCCT grows a contour into the next edge at a vertex wherever no other sharp
edge meets there (the faces across a crease of up to 0.1 rad count as
tangent), as long as the edges turn by less than a right angle; a rounding
of the top edge of a wall that is drafted by 3 degrees above a vertical
part, or of a cylinder that meets another at 0.7 degrees, ran on over the
next edges. `StopChainsAtTurns` (`ChFi3d_Builder`,
`BRepFilletAPI_MakeFillet`) makes the contours added afterwards stop at a
vertex where the edges turn by the angular tolerance or more, unless the
next edge is one of the edges given (the selection, which so still makes one
contour); the vertices where it stopped one are recorded. Past such a
vertex, where the faces meet across a concave crease, the walk past the
stripe's end stays on the face's tangent plane instead of changing onto the
face beyond (`StartSol`), and that face, its surface extended, cuts the
blend in the corner at the vertex (`PerformOneCorner`), as in the files'
roundings; an analytic blend whose contact ends on that crease ends there
(`SplitKPart`, `EndsOnTurnCrease`) and stays analytic instead of being
walked (a torus with circles for contacts, as in the files). Where the
extension is purged on both sides and the first purge removed it, the second
no longer removes the surface data then at that end (the stripe was left
without its blend). Not done: a convex crease (the face beyond meets the
rounding only past the vertex; the walk past the end fails as before, and so
does the fillet); chamfers (their corner at such an end is not the face
beyond's cut, so `BRepFilletAPI_MakeChamfer` does not offer the option).
Without the call nothing changes.

## 0840: a contour never takes its own edge again

The patch 0840 ends a fillet contour's walk that went round a closed chain
for ever (mitcad#434, found in the `.f3d` import after the ninth round of
mitcad#343).

### 0840-chfi3d-contour-never-takes-its-own-edge-again

A closed chain of edges whose edges turn at a vertex W, the faces going on
tangentially across it, rounded from an edge selected for the turns
(`StopChainsAtTurns`, 0180) while the edge beyond W is not.
`PerformElement`'s downstream walk stops at W. Its upstream walk from the
contour's start goes round the chain the other way and comes to W from the
other side, where the edge beyond is the downstream walk's last edge,
selected; `StopsAtTurn` lets it pass, and the walk went on over the
contour's edges, prepending them to the spine for ever. The downstream walk
ends where it comes back to its start vertex; the upstream walk had no such
check, and `BRepFilletAPI_MakeFillet::Add` has no progress range, so a
request to stop did not reach it either (the import's definition of
`db73863f` item 88 that named some of a closed chain's edges ran for hours).
Now neither walk adds an edge already in the contour: it stops there as at
an edge it does not take, and round such a chain the contour is every edge
of the chain once, both its ends at W.

## 0860 and 0861: a contour into a near-tangent edge

The patches 0860 and 0861 make one rounding of a thread's chamfer cone
along its five edges down to the root, the last of them between faces 0.03
radian apart (mitcad#441, after the tenth round of mitcad#343).

### 0860-chfi3d-contour-runs-on-into-a-near-tangent-edge-asked-for

`ChFi3d::IsTangentFaces` takes faces up to 0.1 rad apart for tangent, and
0022 rounds an edge between such faces where a contour starts from it (its
faces farther apart than the angular tolerance, `MinNormalAngle`). A contour
coming to the edge from the edges before it stopped there: `FaceTangency`
takes the next edge only when its faces meet at an angle. The edge then
started a contour of its own, whose walks took the edges before it again
from the other side, two stripes over the same edges that met at no corner.
Which happened depended on which edge comes first among the body's edges: on
the stored body the near-tangent edge came first and the rounding was built;
on the replay's body another edge did. Now `FaceTangency` takes such an edge
as one at an angle where it is one of the edges asked for
(`StopChainsAtTurns`) and its faces meet at more than the angular tolerance;
without `StopChainsAtTurns` nothing changes.

### 0861-chfi3d-contact-at-the-vertex-where-the-blend-vanishes-cuts-the-own-edge

The one contour's blend comes to nothing at the vertex V where the contour
passes from the cone's edge with an earlier rounding of the crest to its
edge with the flank, the flank going on tangentially from the rounding
across an edge from V. The cone and the rounding meet at 1.7 degrees at V,
the blend there thinner than V's tolerance, and the walk's contact on the
rounding ended at V on the rounding's edge with the flank
(`ChFi3d_FilCommonPoint` takes the vertex within its tolerance), not on the
spine's edge. The data structure held no point of the blend on that spine
edge, and the rounding's face kept the whole edge beside the blend's end,
though the blend takes it away: the face's wire was open and BRepCheck
rejected the result (0802 handles the same contact where it ends on the
spine's own edge near V). `ChFi3d_OwnEdgesAtVertices`, after `ChFi3d_FilDS`,
adds for such a contact (on face F, at a vertex V of the edge E between F
and the blend's other face Fo, on an edge of F across which a face Fn goes
on tangentially from F, Fn meeting Fo along an edge from V) a vertex
interference on E at V, with the contact's transition carried over to E as E
lies in F.

## 0920: the caller's radius law

The patch 0920 comes from mitcad#464, a fillet whose radius along its
edges is a B-spline of the position (MitCad's `spline` fillet size).

### 0920-chfi3d-radius-law-of-the-callers-along-a-contour

`BRepFilletAPI_MakeFillet::SetRadius(Law_Function, IC, IinC)` left the
contour with no radius: `ChFiDS_FilSpine::SetRadius` put the law in a
composite it dropped. The other variable radii are interpolated through
(parameter, radius) points with level ends (`Law_Interpol`), so a B-spline
radius could only be approximated through them. The spine keeps the law now,
by its parameter (the abscissa from its first vertex); `ComputeLaw` gives
every guideline the law over its range, constant at the law's end values
where the guideline runs past the law (an extension at an end), and refuses
a closed contour. A spine with a law is constant on no edge, its largest
radius is sampled from the law, `Reset` keeps it, and a radius set at a
point or a reset of all data drops it.

## Reports and tests

- 0160: mitcad#238 and the rounded-rim tests in
  `geometry/tests/test_fillet_overflow.cpp`, which fail without it.
- 0180: mitcad#241 and `geometry/tests/test_fillet_chains.cpp`.
- 0190: mitcad#261 and the turned-disc test in
  `geometry/tests/test_fillet_overflow.cpp`, which fails without it.
- 0250: mitcad#308, mitcad#297 and the chamfered box with rounded corners
  and the box whose top rounding is larger than its rounded corners in
  `geometry/tests/test_fillet_overflow.cpp`, which fail without it.
- 0260: mitcad#307 and the rounding carried across a notch in
  `geometry/tests/test_fillet_results.cpp`, which fails without it.
- 0840: mitcad#434 and the plate whose top outline turns twice in
  `geometry/tests/test_fillet_chain_loops.cpp` (the contour from a selected
  edge takes the outline's four edges once each; MitCad's fillet of three of
  them ends; without the patch the test fails when the contour does not end
  in 30 s).
- 0860 and 0861: mitcad#441 and the block whose top meets a wall along
  three edges, the first between faces 0.05 radian apart, in
  `geometry/tests/test_fillet_near_tangent_contour.cpp` (the contour
  started from the second edge is one contour of the three, valid, with the
  volume of the one started from the first; with the vertex between the
  first two edges at a tolerance of 0.02, above the blend's half width
  there: valid, no part of the second edge left); it fails without its
  patch.
- 0920: mitcad#464 and a block's edge rounded with a B-spline radius, in
  `geometry/tests/test_dressup.cpp` (`test_fillet_spline_radius`: the
  material removed is 1 − π/4 times the integral of the radius squared
  along the edge, and each cross-section misses 1 − π/4 times the radius
  there squared, within 2e-3; without the patch the contour has no radius).
