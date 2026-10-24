# vcpkg overlay ports

`vcpkg.json` (`vcpkg-configuration.overlay-ports`) makes vcpkg build these
ports instead of its own, on every platform and for every use of the
manifest (the CMake presets, `tools/dev-env/build-cycles.sh`). vcpkg builds
an overlay port from source once per machine and triplet and keeps the
result in its binary cache; a change to the port rebuilds it.

## opencascade

vcpkg's `opencascade` port for OCCT 8.0.1 as of the baseline in
`vcpkg.json`, with patches of Mitcad's.

| Files | Origin | Licence |
|---|---|---|
| `portfile.cmake`, `vcpkg.json`, patches 0001 to 0005 | microsoft/vcpkg, `ports/opencascade` at the baseline; unchanged but for `port-version`, the list of patches and the SPDX line | MIT |
| patches 0006 and later | Mitcad | LGPL-2.1-only WITH OCCT-exception-1.0, as OCCT |

The patches change OCCT, so they are under OCCT's licence (the GNU LGPL
version 2.1 with the Open CASCADE exception), published here with Mitcad's
source, and OCCT stays a set of shared libraries (vcpkg's dynamic
triplets), as the [licence policy](../../docs/development.md#licence-policy)
requires; the third-party notices of the packages name them. Each patch
says at its top what it changes and why. Patches 0006 to 0009 change no
result: each removes repeated work that made an algorithm quadratic in the
edges or wires of a face, which bodies with thousands of holes reach
(mitcad#39, mitcad#72, mitcad#77, mitcad#79). Patches 0010 to 0019 make
OCCT's rolling ball fillet build roundings it failed on (mitcad#121), and
0020 to 0026 fix other failures and crashes of it (mitcad#122,
mitcad#133). Patches 0030 to 0032 make an allocation that fails throw
instead of crashing, let a caller learn of it and leave the collections
valid (mitcad#132), and 0033 makes the checker's parallel mode give the
sequential mode's results (mitcad#203), and 0034 lets a signal raised on Windows
reach OCCT's exception instead of locking its mutex twice. Patch 0060 changes no result
either: it removes repeated work that made ShapeFix's repair of
intersecting edges quadratic in the wires of a face (mitcad#60). Patches 0090 and 0091 fix a heap overflow of
OCCT's chamfer and fillet (mitcad#160). Patches 0130 to 0139 make its
fillet build corners where a rounding ends (mitcad#236), 0140 to 0149
chamfers wider than a face beside them (mitcad#240), 0150 to 0159
roundings whose results were invalid or failed (mitcad#239), 0160 lets
its walks pass over the rounded rim of a hole the ball cannot touch
(mitcad#238), 0170 to 0179 make its blend rolling on an edge a true
rolling ball (mitcad#237), 0180 lets a contour stop where its edges
turn (mitcad#241), 0190 takes 0160's walk round a closed rounding
that consumes a face of its edge (mitcad#261), 0200 lets a walked
blend turn about a contact that stands still (mitcad#239), 0210 and
0211 build more corners where a rounding ends (mitcad#236), 0220
lets the face merge after booleans merge two faces of one cylinder whose
seams lie half a turn apart (mitcad#284), 0240 and 0241 keep the
seam of a blend's extension out of the ball's way and let two roundings
meet across a notch in the rim they round (mitcad#239), 0250 lets
a walk go on along a face of its contour that 0160 took for a rim
(mitcad#308), 0260 carries a fillet's contour across a small notch in
the rim it rounds onto the next edge of that rim (mitcad#307), 0270
lets two roundings meet on one contact line of a face that goes on past
the line's ends and 0271 cuts two roundings by each other where their
contacts cross part of the way (mitcad#239), 0280 to 0283 build more
corners where a rounding ends among several faces (mitcad#236),
0290 and 0291 walk round a corner's vertex through a face met at a point
and end a rounding past the spine's end where one contact ran off its
face (mitcad#236), 0300 gives the pieces of an edge prolonged at a
rounding's end across a seam their p-curves on the right face
(mitcad#239), 0320 tells where a fillet that is not done stopped, 0321
builds a closed rounding of several edges that consumes a face all round
(mitcad#322), 0322 ends an open stripe whose contact runs along a whole
edge at that edge's vertex without corrupting memory (mitcad#331), 0330
tells which corner a fillet failed to build at a vertex and why, 0331
builds a rounding rolled on a straight edge beside a plane as its exact
cylinder (mitcad#323), 0340 to 0342 end roundings wider than a top or an end
face, whose balls roll on that face's edge, where the faces at the ends
cut them (mitcad#236), 0350 to 0352 put a fillet's regularity only on
edges whose faces are tangent and build chamfers and roundings that
consume both faces of a closed chain of edges (mitcad#300), 0360 orders
the edges that leave a vertex tangentially to each other about it when a
boolean splits a face (mitcad#235), 0380 and 0381 end a walked
chamfer at a vertex where the face at the end is tangent to both its
faces and keep a chamfer's guide on its edges (mitcad#263), 0390 and
0391 let a blend rolling on an edge pass an edge turning towards the ball
and let the ball rest on the edges of both faces (mitcad#237), 0400 ends a
rounding that vanishes where its faces come tangent at the vertex where
its contacts reach it and 0401 splits a rounding's end cut where it
crosses the seam's line of a periodic face at the end (mitcad#330), 0410
ends a rounding whose end section lies in the face it runs into
tangentially (mitcad#236), 0420 rounds a boss's rim on a cylinder whose
seam runs in steps (mitcad#337), 0430 to 0432 let a rounding beside a
face narrower than its radius roll on that face's far edge where its
overflow would run across another rounding, and make the crease where two
such roundings meet (mitcad#339), 0440 builds chamfers and roundings
whose contact runs along a chain of edges of the face they consume
(mitcad#334), 0450 ends a rounding whose face turns tangentially into the
face at its end there (mitcad#341), 0460 ends a rounding on the end
edge of a rounding of its radius beside it, consuming the corner face
between them (mitcad#342), 0480 and 0481 keep the parts of an open
intersection line beyond its vertices and split a blend's piece over a gap
at the start of a closed spine (mitcad#263), 0490 ends a rounding at a
face whose trimmed surface its opposite contact crosses only past the
trimmed bounds (mitcad#236), 0500 to 0502 make the rounding between a
cylinder and a cone about one axis a torus, cut it round flats under an
overhanging rim it passes below, and cut a rounding whose contact runs off
its face over a gap by the face at its end (mitcad#237), 0503 joins
two roundings at a corner along their intersection where their ends meet
on an edge within its tolerance (mitcad#378), 0510 to 0512
let booleans cut stored faces and edges that run many turns (a threaded
hole's cylinders, flanks and helices): new curves are put into the copy of
the period inside such a face, points are classified at every copy of them
in its range, and a long B-spline's roots on a quadric are sampled by its
knot spans (mitcad#349), 0530 and 0531 keep a walked chamfer's guide on
its edges beside analytic blends, build a chamfer wider than a face of its
edges along part of its chain, the faces beyond that face growing down to
it, and end a chamfer stopped where its edges turn by its blend run on past
the vertex (mitcad#263), 0540 ends a rounding that runs on into an earlier
rounding of its radius with the section they share (mitcad#236), 0550 ends
a rounding over pieces cut by the face at the end where that face's edge
is in two pieces at a vertex no other
edge meets (mitcad#343), 0560 checks a curve moved on a face spanning
many periods where it is moved to, so that a second boolean cuts the pieces
a first one left of such a face (mitcad#380), and 0580 and 0581 round a
pocket's walls over ribs up to blocks flush with the contact: a contact line
along a concave edge of its face is off that face, and roundings joined over
a rib whose sides cut them along sections meet inside the piece over the
rib's end (mitcad#343), 0590 gives up a merged face's wire that repeats
itself over the same seams instead of building it without end
(mitcad#235), 0591 to 0593 bound the work of booleans on faces running
many turns: a face's samples count the knot spans in its range, the face
merge tests each surface's planarity once, and a projection's grid holds
at most 1001 parameters a direction (mitcad#235), 0600 ends a rounding at a vertex where a wall
starts by its blend run on past the vertex along its other face up to the
wall's edge with that face, cut by the wall along its side (mitcad#343),
0620 rolls a rounding on the convex edge where its contact leaves a
narrow face before the end of the spine, as in its middle, where that
edge is not of a face at the end vertex (mitcad#343), 0650 cuts an
analytic chamfer over a gap by a chain of up to nine faces beyond, along
the arc nearest the blend's piece where the face beyond grows to the cut,
from gap ends at vertices of the contact's face, and keeps the pieces about
a closed spine's start (a flange chamfered past the strip of its top up to
the roots of the teeth standing on it, mitcad#235), 0651 starts a walk
on the seam of a face closed round its surface on the seam's other side
where it leaves the face across it, and ends a spine's extension where its
walk or surface fails (a thread's crests rounded along all their turns,
mitcad#235), 0670 makes the rounding round a wall's corner rounded with the
ball's radius, between two analytic pieces, the piece of the standing
ball's sphere (a tab's top outline on a steep cylinder across it,
mitcad#235), 0671 cuts an analytic chamfer whose contacts both leave
their faces before its edge's ends across its whole width there, by a chain
of faces beyond grown on to it (a thread's groove ending in a flat face, the
edge where it meets the bore chamfered past the flanks, mitcad#235),
0680 merges a ring of cylinder pieces cut through from
end to end, which reaches round the whole period without winding round it,
into one face without a seam (a spiral stair's slabs, treads and helical
stringers, mitcad#400), 0690 carries the seam of a face at a rounding's
end down along an arc that continues it to the rounding's contact (a bar's
top edge rounded where the bar meets a bore's wall at its seam,
mitcad#235), 0691 takes the end of an OnSame rounding whose face at the end
continues the opposite face tangentially where the contact crosses the
prolonged edge, near the corner, and 0692 puts an analytic blend's contact
end found within a vertex's tolerance at the vertex (a recess's rim rounded
up to a rib across it whose bottom edges are rounded, mitcad#235).
0720 drops a face at a rounding's end vertex that the end
passes wholly beyond, the face beyond it cut once, and 0721 prolongs an
OnSame end's edge as its own curve where that lies on both faces' surfaces
(a circle of two coaxial surfaces across a cone's seam; mitcad#343).
0700 joins two stripes whose balls are one ball
rolling over a narrow face between them into one blend that face cuts (a
notch in the blend's side), drops the stripes under such a ball, and cuts a
hole in a blend a face of the solid pokes through, that face keeping its
part outside the blend (mitcad#343).
0740 gives a rounding's end on a face at the end split round the vertex
into faces of one surface, each with its own surface, the curves its arcs
lack on the piece the cut goes on (a bar's top edge rounded into a bore
whose wall is three faces round the corner), and 0741 ends an analytic
blend's contact that leaves its face at a vertex on the edge of the face
from the spine's end vertex also where that edge's curve stops short of
the vertex (the recess's rim of 0691 and 0692), and 0742 extends a
blend's guide past an open end of its spine along the curve the end edge
is a piece of (a thread's root rounded up to the faces that cut it;
mitcad#415).
0750 rounds both edges of a thread's crest narrower than the roundings
with one blend between the flanks, cut by the faces at the thread's ends
along their long oblique curves: a guide along a helix and a B-spline face
swept along one are continued by their screw motion past their ends, a
face at the end met at a kink of a degree cuts the blend, and a stripe
computed over the crest that runs out at both ends takes the other's
place (mitcad#343).
0770 rolls a rounding on an edge that runs into its end vertex tangent to
its other face up to that vertex, where it vanishes, and 0771 stops a
contour at a vertex where the faces of both its edges come tangent, each
side's rounding vanishing there (a block on a post as wide as the block,
mitcad#343). 0772 joins two stripes on the same face and on one plane
whose edges a corner of the solid interrupts into one blend over the
corner, cut by the corner's sides, which grow up to it (a bulge's foot on a
housing's rounded corner, mitcad#343).
0790 ends a rounding whose edge runs on tangentially into an edge not
rounded, the face at the end going on tangentially from the rounding's
face, where that face's surface, followed back along the rounding, cuts it
across, a face on that surface closing the end (a thread's crest rounded
down to its run-out; mitcad#422).
0800 to 0803 take the traits of stored bodies into OCCT's fillet: an OnSame
end's cap points brought into one period however many periods apart (a
cylinder trimmed turns below its basis's parameters), faces taken for
tangent within ten times the tolerance of their edge, a contact ending on
the spine's own edge near the vertex where the spine goes on passed
through that vertex, and the point where a cap leaves the prolongation
tangentially given a tolerance that takes in their crossings near it (a
thread's chamfer and a groove's floor, mitcad#343).
0840 keeps a fillet's contour from taking an edge of its own again, so
that its walk round a closed chain that stops at a turn ends (it went
round for ever; mitcad#434).
0820 makes the blend 0772 joins across a corner one face per stripe, as
the stored designs' roundings are. 0821 rounds both edges of a lens (a
face of two edges, one concave and one convex, narrower than the
roundings) with one S-shaped section, the convex blend's ball touching the
concave blend's from outside. 0822 rolls a rounding on both edges of such a
lens into its ends, where the edges meet and the rounding vanishes in a
point (a key on a shaft, mitcad#343).
0860 carries a fillet's contour on into an edge asked for whose faces meet
at a few hundredths of a radian, and 0861 cuts the spine's own edge from
the face of a contact that ends at the vertex where the blend comes to
nothing, on another edge of that face (a thread's chamfer cone rounded down
to the root; mitcad#441).
0880 starts a rounding along an edge of a face the ball touches at no
point (a strip of an earlier, smaller rounding, convex) across that face,
the strip consumed (an endcap's notch corner; mitcad#343).
0890 ends a rounding at a vertex where a sliver lies between its faces and
the face beyond the edge its contact left its face by as that face's OnSame
end, the sliver consumed (a wall's foot rounded up to an earlier rounding
of its radius past a step's end face; mitcad#343), and 0891 walks a closed
rounding over a hole's rounded mouth where the piece over the gap runs on
round the spine or the gap lies across the spine's start (a disc's rim;
mitcad#343).
0870 keeps a rounding's ball on the surface of a narrow face of its edge
where the contact runs off that face across a tangent edge and comes back,
the face beyond cutting the rounding, as the stored designs' roundings are
(a closed chain round a part, mitcad#343).
0900 to 0909 round the rim and the notch walls of a bar's end (mitcad#343):
0900 names the face of a piece walked again past an OnSame end after the
spine's edges, 0901 ends a walked intersection with the face at the end
where the walk first passes the end point, 0902 ends a rounding that
vanishes where three faces are tangent at its spine's end vertex, 0903
lets a blend rolling on an edge go on where its other contact crosses onto
the face of the spine's next edge, 0904 rolls a blend on along the line or
circle of the edge it rolls on past that edge's vertex, cut by the face
at the end (or ends it in a point where the narrow face ends in the
spine's vertex), 0905 turns a ball rolling on an edge about that edge's
vertex onto the next edge, 0906 takes the next start found as an edge
overflow where the first walk from one fails, 0907 releases such a turn
where a step lands on the point where it ends, 0908 samples the spine
more finely where no overflow start is found, and 0909 ends a blend
rolling into the spine's vertex where its section lost its width a hair
short of it.

0920 keeps a radius law the caller sets on a fillet's contour, which the
spine dropped (a B-spline radius along the edges, mitcad#464).

0910 starts an OnSame end's cap at the opposite contact's crossing with
the prolonged edge itself, not where the contact's approximated curve
crosses the face at the end (a rounding along a thread's chamfer cone
ending on the thread's root cylinder, mitcad#343).

0911 makes an end where the face at the end goes on tangentially from a
face of the spine's edge, behind that edge, an OnSame end: the face at the
end cuts the blend and grows up to it (a pin's foot rounded up to the
slot whose sides are tangent to the pin, mitcad#343).

0930 lets a rounding roll on another's contact line (0016) where the two
spines run opposite ways: its side on the line runs from the line's end to
its start (both rims of a ring rounded, the rims' circles turning opposite
ways, mitcad#343). 0931 runs the seam of the face at a corner's end on to
the corner's cut where the cut crosses the seam's line past the seam's end
at the corner's vertex (roundings of three bores' curves ending on a rod
whose seam runs into one of their end vertices, mitcad#343).

0932 ends a walk that 0077 cut before a singular section past its spine's
end at the cut (it was taken up again from there). 0933 joins two stripe
ends over a narrow face that pokes through the ball's path, the contact
never leaving its face, one blend holed by that face (a rounding of a
torus's edge with a plane that a rod interrupts twice, mitcad#343).

0940 rounds over a narrow face whose far edge is a concave crease on that
face's surface unbounded, as 0531 chamfers: the face beyond cuts the blend's
side on the narrow face from where the contact crosses the crease up to
the other contact, and grows up to the cut; at the spine's ends a second
face beyond takes over where the two faces' junction, run on along its own
curve, meets the blend (a key's top edge rounded wider than the key's side
is high where it runs out against a bore's wall, mitcad#343).

0941 takes the crossings of a gap (0142) on a closed line of the other
contact in the period that follows the piece before the gap, where the gap
does not run across the spine's start (a boss's foot chamfered past a plate
corner with a slanted side, the boss's seam just before the gap,
mitcad#495). 0942 stops ShapeFix's repair of intersecting wires from
intersecting the same two edges for ever where two arcs of one circle in
two wires meet end to end (the repair of 0941's invalid result never
ended, mitcad#495).

0950 lets the roundings of both edges of a face exactly as wide as the two
take the face away: their contacts along each other on it do not clash,
the points their corners make at the contacts' ends are one, and the two
contacts are one edge (window sills in a ring wall rounded with half the
wall's thickness on both edges, mitcad#343).

0951 ends a rounding where its wall steps back into the material by much
less than the radius (a narrow strip, square to the rounding, at the end):
the blend ends at the section through the vertex and a quarter torus, its
ball turning on about its contact's end, closes it, cut by the
stepped-back wall (a rim's wall stepping back by 4.3 micrometres,
mitcad#343).

0961 ends a rounding in a point at its spine's end vertex, where its faces
come tangent, when its walk stopped short of the vertex on the spine's own
edge (the faces meeting only within that edge's tolerance), its surface
bent to the vertex with its contact curves (a bar's outline rounded up to
where an earlier rounding runs tangentially into the bar's side, round 18
of mitcad#343). 0962 builds 0951's quarter torus whichever side of the
blend the wall is on, and only where the recess beyond the step ends in a
strip back to the wall; a later rounding whose recess an earlier rounding
ends runs on across the step (mitcad#343).
0970 rounds a chain past a wall's convex corner rounded with less than the
radius, where no ball rolls round: the chain is split at the corner's arc,
both roundings end where their axes cross, at the ball touching the face
of the chain and both walls, and a vertex blend from that ball's centre to
the corner's arc closes the corner, with a torus where the second wall
leans over the face (an underside outline rounded with 0.5 past corners
rounded with 0.3, mitcad#343).
0980 rounds a chain whose faces turn into their neighbours at creases of a
few degrees, which the fillet takes for tangent, both faces of the blend
changing at nearly one section: a contour runs on over a short edge not
asked for to the next edge asked for, the guide of a walk next to an
analytic piece is built from a stretch of its edge before it, both faces
change at once where the contacts reach their arcs within a hair, and the
ball turns about a concave crease, rolling on its edge, instead of jumping
across it (a fin's chamfered top edges, mitcad#343).
0990 ends a rounding rolling on an edge where the faces at its ends cut it
past the point where its ball comes tangent to that edge's face: the blend
is walked again with the face of its other contact unbounded up to that
point, run on along its tangents and cut by the face its other contact runs
into, the face at the end and the rolled edge's face, which grow up to the
cut (a groove's rim under a slot's end wall rounded over a strip lower than
the radius, cut at a notch's walls, mitcad#343).
1000 lets a chamfer's contact pass a slight concave crease of its face
(0980's turn about the crease is a rounding's), keeps 0671's cut of an
analytic chamfer to the spine's end off the unbounded narrow face of
0531's computation again, and continues the B-spline surface of a face
that grew past its bounds up to the face's bounds, the edges' curves on it
kept (a fin's top chains chamfered over slight creases of its top and over
side strips narrower than the chamfer, mitcad#343). 1001 takes 0980's
guide stretch only where the spine turns at that vertex: along tangent
edges the stretched guide's end lay off the spine's where an analytic
piece's section is, and the walk from it failed (a bar's outline rounded
up to where its blend vanishes, 0961's case, mitcad#343).
1010 lets the faces round a cut-out cut a closed rim's analytic blend over
the gap where the blend's contact leaves its face along a whole edge of the
rim: the gap runs from the only piece of another edge round to that
piece's start, which the search for the piece after the gap did not reach
(a bore's rim in two arcs chamfered past a pocket from below that breaks
through the wall 1 under the top, mitcad#343).
1011 takes the ends of a rounding's intersection with a periodic face at
its end in one period where one end lies past the face's edge across the
face's parametric seam (a pocket's rim rounded up to an earlier rounding's
torus whose seam lies at that end, mitcad#343).
1020 takes a face beyond a contact's kink that goes on smoothly into a
face at the spine's end vertex for a face at the end, as 0750 takes one
that holds that vertex, also where the next piece starts from the arc of
the piece before (a drafted peg's corners rounded down to a block whose
walls go on from the peg's sides at slight creases and whose corners are
rounded with less, the walls and corners cutting the rounding at the
creases instead of 0980's turn about both creases at once, mitcad#343).
1030 corrects the second derivative of the binormal of the guide trihedron
by curvilinear equivalence (`GeomFill_GuideTrihedronAC::D2`), which bent a
pipe shell's surfaces along an auxiliary spine between its sections
(mitcad#539).

- `0006-shapeupgrade-unifysamedomain-seam-search-by-vertex.patch`: the face
  merge after booleans (`ShapeUpgrade_UnifySameDomain`) looks up the edges
  it removed by vertex instead of going through all of them at each step of
  a wire across a seam.
- `0007-brepcheck-analyzer-no-search-in-face.patch`: the checker
  (`BRepCheck_Analyzer`) no longer searches a face for each of its own
  vertices and edges (`InContextOfAncestor`).
- `0008-brepcheck-wire-seam-edges-once-per-face.patch`: the checker finds
  the seam edges of a face once instead of once per wire for the 2d closure
  check, and no longer searches the face for each of its wires
  (`BRepCheck_Wire::InContextOfFace`).
- `0009-bopalgo-face-face-adaptors-once-per-face.patch`: the face/face
  intersections of a boolean build the surface adaptor of each face (its UV
  bounds go through all its edges) once instead of once per pair of faces
  (`IntTools_Context::SetSurfaceAdaptor`).
- `0010-brepblend-surfrst-line-on-both-restrictions.patch`: a blend
  rolling on an edge (`BRepBlend_SurfRstLineBuilder`) whose contacts leave
  the face and reach the end of the edge in the same step took the
  parameter of the wrong arc as its point on the edge and stopped short of
  the end; it now ends on both.
- `0011-chfi3d-fillet-edge-overflow.patch`: edge overflow of the rolling
  ball fillet. Where the ball cannot touch a face of the edge within that
  face (a face narrower than the rounding), it touches the face beyond the
  face's far edge, or, where that face does not let it, it rolls on the far
  edge (a blend on an obstacle edge, which OCCT had only where a walk met
  an edge in the middle of a rounding). OCCT found no start where the
  contact leaves a face along the whole edge; now the start can be on the
  face beyond or on the edge (`StartSolOverflow`). A blend on an edge then
  ends where the spine or the face at its end does (`ObstacleAtSpineEnd`,
  `ChFi3d_ObstacleEndOnFace`, `ChFi3d_ExtendAtSpineEnds`) or goes round a
  closed spine (a periodic guide in the edge/face `ComputeData`), and the
  narrow face it consumes vanishes (`ChFi3d_CutConsumedFaces`). Where the
  contact leaves a face near the end of the spine, OCCT cuts the rounding
  with the face beyond, as before. Fillets only; chamfers are unchanged.
- `0012-chfi3d-closed-stripe-end-on-vertex.patch`: a closed rounding
  whose start lies on a vertex but whose end was not identified with it
  gave the builder a vertex index as a point index (an exception).
- `0013-chfi3d-overflow-across-faces-and-overlapping-roundings.patch`:
  0011's overflow across several narrow faces and on both sides of the
  edge. Each side's support moves on across the edge of its face that the
  section crosses while the contact lies beyond it, also where the ball
  touches a face's surface at no point (a sliver: the face beyond the
  nearest edge, within the ball's diameter); a side rolls on the last edge
  before a face it cannot touch (not both sides: that ball touches no
  face); a start touching the faces it reached must clear the faces it
  passed and those around them. The consumed faces between the spine and
  an edge the blend rolls on can be a chain of strips
  (`ChFi3d_CutConsumedFaces`).
  Two stripes whose contacts on a face between their edges each lie beyond
  the other's are computed again overflowing that face across each
  other's edge (`OverlappingStripes`, `OverlapEdge`); a stripe whose blend
  then lies on another's stays a contour of the builder but gives no faces
  (`RemoveCoincidentStripes`). The builder class gets members for these,
  so code built against the port's headers must be rebuilt with them.
- `0014-chfi3d-contact-along-face-edge.patch`: a fillet or chamfer whose
  contact runs along the far edge of a face (a face exactly as wide as
  the contact distance). The split of an analytic blend into the domains
  of its faces found no domain for a contact line lying on the face's
  boundary, so the blend went to the walking path and failed; such a line
  now takes the domains of the same line moved a little into the face,
  its ends put at the vertices they meet (`SplitKPart`). A blend whose
  contact covers a whole edge of its face is then stored as a blend on
  that edge, as 0011's blends rolling on an edge are, so that the face it
  consumes is dropped instead of being trimmed to nothing
  (`ChFi3d_ContactsAlongFaceEdges`; a blend made 0.1 % smaller than the
  face stays inside it). Walked blends along a face's boundary still fail.
- `0015-brepblend-surfrst-smaller-step-on-failed-reframing.patch`: a
  blend rolling on an edge (`BRepBlend_SurfRstLineBuilder`) stopped at the
  first section it could not reframe on an arc; a step too large lands on
  the other branch of the solution, outside the face. The step is now
  halved first, as the walking between two faces does.
- `0016-chfi3d-rounding-rolls-on-anothers-contact-line.patch`: two
  roundings of a face narrower than both, where no ball touches the faces
  beyond it on both sides (the rims of a plate thinner than the
  roundings' diameter), so that 0013 cannot make them one: the earlier
  rounding is computed on its own faces, the later one rolls on the
  earlier one's contact line on that face (`RideOnOverlap`), the two
  blends meet on that line (`ApplyRides`, after the corners; the caps of
  open roundings meet where the line ends) and the face vanishes. Both
  roundings convex or both concave only. The builder class gets members
  for this (code built against the port's headers must be rebuilt).
- `0017-chfi3d-contact-along-face-edge-ends-at-vertices.patch`: 0014's
  ends at the vertices also where the other face's contact ends first (a
  stripe ending on slanted or curved faces): those ends are filled in the
  processing of the spine's extensions, which did not put them at the
  vertex; the arc was cut a hatching shift from it, which left a tiny edge
  in the face at the end and an invalid shell (`SplitKPart`).
- `0018-chfi3d-corner-where-the-end-face-is-split-in-one-plane.patch`: a
  fillet or chamfer ending at a vertex where the face at the end is split
  in two faces of one plane (a tab fused onto a support): the edge between
  them is no longer counted where `PerformIntersectionAtEnd` chooses, by
  the number of edges, between walking round the vertex (which failed
  with "Failed to find edge" there) and `IntersectMoreCorner`,
  and `IntersectMoreCorner` continues the prolonged edge on the opposite
  face by that face's own edge (before, any other edge of the vertex,
  which oriented the face inconsistently).
- `0019-chfi3d-undone-moresurfdata-corner-fails.patch`: where
  `PerformMoreSurfdata` returns without computing the corner (its
  surfdatas at the end are not as it expects), the vertex now fails with
  an exception. Before, the stripe's end kept no points, the data
  structure referred to point 0, and the topological build threw
  `TopOpeBRepDS_DataStructure::Point` or, without range checks, wrote out
  of bounds (heap corruption).

The patches 0020 to 0026 fix OCCT's fillet (`TKFillet`, `ChFi3d`) where
it crashed, threw or built a wrong shape on fillets of the `.f3d` corpora
(mitcad#122, and from 0024 mitcad#133); each one changes the results only
in the cases it describes, and `geometry/tests/test_fillet_kernel.cpp`
(`test_fillet_ends.cpp` from 0024) rebuilds those it could on synthetic
shapes with OCCT alone.

- `0020-chfi3d-crash-fixes.patch`: three causes of segmentation faults. A
  stripe continued past an obstacle face that does not hold the edge the
  contact left by kept an empty restriction curve (`StartSol`); the seam
  of the face at a corner was intersected with a parallel curve, whose
  points `Extrema_ExtCC` does not give (`PerformOneCorner`,
  `IntersectMoreCorner`); `PerformMoreSurfdata` read the first stripe of
  the builder instead of the stripe at the vertex.
- `0021-chfi3d-corner-of-contour-closed-at-sharp-vertex.patch`: a contour
  of tangent edges that closes on itself at a sharp corner has both ends
  at that vertex; with more than three sharp edges there
  (`PerformMoreThreeCorner`) only its first end was found, and the
  corner threw.
- `0022-chfi3d-round-edges-asked-for-near-tangent.patch`: an edge between
  faces up to 0.1 rad apart counted as tangent and could not be rounded;
  it is refused now only when its faces come within the builder's angular
  tolerance.
- `0023-chfi3d-meeting-point-of-periodic-contact-lines.patch`: where a
  fillet along an arc meets one along a line at a shallow corner, the
  meeting point of their contact lines on the common face was taken on
  the arc's extension, far from the vertex, and the corner face was
  self-intersecting.
- `0024-chfi3d-order-of-periodic-kpart-surfaces.patch`: the analytic
  pieces of a rounding along a whole circle, cut where its contact line
  leaves a face (`SplitKPart`), are sorted along the spine
  (`PerformSetOfKPart`); the sort kept a stale piece, so one piece came out
  repeated and others lost, and the walks between the pieces threw
  (`ChFiDS_CommonPoint::Vector`) (mitcad#133).
- `0025-chfi3d-end-cut-by-faces-around-a-cylinder-corner.patch`: a
  rounding whose end is cut by several faces around a cylinder's corner
  patch (`PerformIntersectionAtEnd`): the crossing with each common edge
  is taken on that edge (two arcs of one circle gave the same one), a
  crossing beyond the edge's end prolongs the faces' common boundary along
  the edge's curve and cuts the corner patch's edge there (not where the
  rounding passes beside a face between the end's first and last faces,
  such as a narrow chamfer at a step's corner), a seam on a face
  at the end only no longer counts as an edge of the vertex, and the end's
  intersection with that face is taken across the seam, split there, and
  the seam cut (mitcad#133).
- `0026-chfi3d-onsame-end-cut-by-several-faces.patch`: the end of a
  rounding whose face runs on tangentially past the vertex (the OnSame
  state) with its contact off any edge, on either side of the rounding,
  cut by several faces (`PerformIntersectionAtEnd` gave up): the face's
  edge at the vertex is
  prolonged to the contact by a piece of its curve on both its faces, the
  spine's edge is cut at the vertex, and the ends on a periodic face are
  taken into its domain (mitcad#133).

The patches 0030 to 0032 make OCCT handle an allocation that fails
(mitcad#132).

- `0030-standard-failed-allocation-throws.patch`:
  `Standard::AllocateOptimal`, under the collections' allocators and the
  strings, returned null when an allocation failed with the native (the
  default), TBB and jemalloc memory managers, and its callers wrote to
  the block at once: under a memory limit the shape checker crashed in
  `CSLib_Class2d`. It throws `Standard_OutOfMemory` now, as
  `Standard::Allocate` and the flexible memory manager do;
  `Standard_Failure`'s message, the one caller that checked for null,
  goes without it as before. `Standard_OutOfMemory::NbRaised` counts the
  exceptions made, so that a caller learns of a failure that the checker
  or the healing caught inside and went on from.
- `0031-osd-threadpool-failed-allocations-to-launching-thread.patch`:
  `Standard_OutOfMemory::NbRaisedInThread` counts the failures of the
  calling thread, and `OSD_ThreadPool::Launcher` hands those of the jobs
  its threads ran over to the thread that launched them (the checker and
  the booleans run on the pool), so a caller knows whether memory ran out
  under its own call rather than under another thread's at the same time.
- `0032-ncollection-containers-intact-when-allocation-fails.patch`: the
  collections stay valid when an allocation in them throws.
  `NCollection_Array1::Resize` and `NCollection_LocalArray` freed their
  block before allocating the new one and kept pointing to it (freed again
  by their destructors); `NCollection_IndexedMap` and
  `NCollection_IndexedDataMap::ReSize` moved their nodes to the new
  buckets before reallocating their index array, and when that threw
  their destructors freed nodes twice (a boolean ended the process so
  under a memory limit). `NCollection_BaseMap::BeginResize` leaked its
  first bucket array when the second failed.

- `0033-brepcheck-edge-curve-adaptor-per-context.patch`: in the checker's
  parallel mode (`BRepCheck_Analyzer`'s `theIsParallel`, which Mitcad's
  checks use since mitcad#181) the faces are checked on several threads,
  and an edge in the contexts of its two faces at the same time
  (`BRepCheck_Edge::InContext`). Both evaluated the edge's one curve
  adaptor, which caches the span of a B-spline it evaluated last, so a
  thread could take another's span, find the edge off its face
  (`BRepCheck_InvalidCurveOnSurface`) and call the face unorientable: a
  valid shape came out valid or invalid from run to run (a round groove
  swept along a helix and cut from a nut, mitcad#203). Each context now
  evaluates its own copy of the adaptor (`ShallowCopy`); the sequential
  mode, the checks and their statuses are unchanged.

- `0034-osd-signal-handler-recursive-mutex.patch`: on Windows the handler
  `OSD::SetSignal` installs for SIGSEGV, SIGFPE and SIGILL
  (`SIGWntHandler`) locked the signal mutex and called `CallHandler`,
  which locked it again on the same thread; MSVC's runtime threw
  `std::system_error` ("resource deadlock would occur") in place of
  `OSD_Exception_ACCESS_VIOLATION`, which the geometry's guard missed (a
  crash in a section's booleans was reported on the body instead of the
  section). The mutex is recursive now.

- `0060-shapefix-intersecting-edges-once-per-face.patch`: the repair of
  a wire's intersecting edges (`ShapeFix_IntersectionTool::FixSelfIntersectWire`,
  once for each wire of a face) applied the healing context to the whole
  face and went through all its vertices for their largest tolerance. The
  face is applied with `ShapeBuild_ReShape::ApplyAgain`, which gives the
  previous call's result and status back while no replacement has been
  recorded since (that call recorded none either: it rebuilt nothing), and
  the largest tolerance is looked up only for a wire with edges that may
  cross, before anything changes. The repair of intersecting wires
  (`FixIntersectingWires`) lists the edges of the second wire of a pair only
  when the boxes of the two wires meet. The healing context's class gets
  members (code built against the port's headers must be rebuilt with them).

The patches 0050 and later continue 0010 to 0019 (mitcad#121): roundings
whose contacts leave their faces near the ends of the spine or part of the
way along it.

- `0050-chfi3d-kpart-end-runs-into-end-face.patch`: an analytic blend
  (`SplitKPart`) whose contact on one face runs into the face at the end
  of the spine, a plane, while the other contact runs on to the end where
  its face continues tangentially into that face at the end (a round slot
  end rounded under a pocket as wide as the slot), keeps the second
  contact up to the end, so that the face at the end cuts the one
  analytic blend (`PerformOneCorner`). Before, the rest was walked on the
  first face's tangent plane and the vertex failed or gave an invalid
  shape.
- `0051-chfi3d-rolling-on-across-surfdatas.patch`: a blend rolling on an
  edge whose other contact changes faces while the ball rolls on along the
  same edge (a bevel narrower than the rounding along the bottom of a
  plate with round corners: the ball's centre stays put at the corners) no
  longer cuts the edge where its pieces meet (`ChFi3d_FilDS`); the cut
  left the edge's piece twice in the face across it, an invalid face.
- `0052-chfi3d-rolling-start-beside-tangent-face.patch`: the start of a
  blend that rolls on the far edge of a narrow face on one side and
  touches, on the other, a face that continues the edge's face
  tangentially (a notch whose straight side runs on into a round corner):
  `StartSolOverflow` lets the side that crossed a sharp edge roll and keeps
  the tangent side's face, and `BRepBlend_SurfRstLineBuilder` solves a
  start that touches the surface outside its face again from points
  across the face (the ball on the far side of a cylinder). Before, no
  start was found or the walk failed at once.
- `0053-chfi3d-corner-after-overflow-onto-face-beyond.patch`: the corner
  at the end of such a blend when the faces beside it run on past the end
  (the notch with a tab under its straight side and round corner).
  `PerformOneCorner` takes the face at the corner's vertex for the face at
  the end, not the face the blend consumes, and, where the far contact's
  face does not reach the corner's vertex, prolongs the face at the end
  to the cork across the face beyond the face of the spine's edge,
  cutting the edge between them where it crosses the face at the end.
  Before, the cap was not built (`OneCorner : bouchon non ecrit`).
- `0054-chfi3d-contact-along-a-part-of-a-face-edge.patch`: a contact that
  runs along a part of an edge of its face (a channel wall exactly as high
  as the rounding at its foot, the floor rounded down at the ends, where
  the contact lies inside the wall) is taken as running along the edge,
  as 0014 does for a whole edge, and the edge is cut at its inner ends
  (`ChFi3d_ContactsAlongFaceEdges`). Before, the contact stayed on the
  face along its boundary and the face's wire intersected itself.
- `0055-chfi3d-kpart-blend-cut-by-face-beyond.patch`: an analytic blend
  whose contact passes over a hole or a cut-out of its face (a hole
  through a plate in front of a wall whose foot is rounded, a hole's rim
  near the part's side) runs on over it, and the face beyond cuts it: a
  piece of the blend whose side on that face is its intersection with the
  face beyond fills the gap between the pieces of `SplitKPart`
  (`PerformSetOfKPart`). Gaps where the face beyond reaches over the
  neighbouring pieces too, or where one of its edges (a seam) ends under
  the blend, keep OCCT's handling. Before, the blend failed, or rolled on
  the hole's edge.
- `0056-chfi3d-corner-cutting-the-last-two-walked-pieces.patch`: the
  corner where the face at the end cuts the last two walked pieces of a
  blend (`PerformMoreSurfdata`): one contact has left its face across an
  edge of the face at the end, the other runs on to another edge of it
  after crossing a face boundary (a rounding round a rod where it meets a
  pipe whose top is flush with the rod's, the rod's side ending in a
  narrow bevel). `MoreSurfdata` recognises the corner whichever order the
  vertex's two edges come in (before, in the other order the corner went
  to `PerformOneCorner`, which drops the last piece and fails). Where the
  face's cut leaves the last piece through its end section instead of
  reaching the point on the edge (next to the contact the piece dips under
  the face, its faces' normals being more than a right angle apart), the
  piece's surface is extended beyond its end and cut again; a cut that
  only touches the piece at that point is not taken. The pieces' ends,
  which meet within their tolerance only, are matched to the nearer
  points and the curves' orientations follow from the nearer ends (before,
  an unorientable shell); the section gets a p-curve on the previous
  piece where its projection fails; a last piece prolonged beyond the
  point keeps its contact edge only up to that point.
- `0057-chfi3d-kpart-gaps-in-order-across-closed-lines.patch`: more of
  the gaps of 0055 are filled (the rim of a cylinder whose top is crossed
  by blind holes or by notches open to a bore; a circle whose contact on a
  ledge crosses round notches). The pieces of a closed spine are taken in
  the order of their starts (SplitKPart lists the two halves of the piece
  across the spine's start first, so the gaps next to it were missed);
  only the face beyond's vertices on its boundary with the contact's face
  count as under the blend (the bottom of a blind hole refused the cut);
  on an intersection line that closes on itself the arc across the line's
  ends is tried too; and where the cut runs along two lines that meet, the
  gap's two pieces are split at the section through that point rather
  than at the gap's middle.
- `0058-chfi3d-rounding-rolls-on-a-convex-far-edge.patch`: a rounding
  whose contact runs off the face of its edge across that face's far edge,
  where that edge is sharp and convex, rolls on the edge (it stays tangent
  to the other side's face and runs through the edge: a bevel narrower
  than the rounding of its edge with a slot's wall, a wall's foot strip
  lower than the rounding with the wall leaning back above it). Before,
  where a ball could touch the face beyond, the rounding ran onto it: a
  larger rounding that cut into the face beyond and whose corners at the
  spine's ends found no face at the end. Tangent and concave far edges,
  edges rounded by the same fillet, and contacts that pass several faces
  keep the earlier handling.
- `0059-chfi3d-kpart-blend-cut-by-the-face-at-the-end.patch`: an analytic
  blend whose contact on one face runs into the face at an end of the
  spine before that end (the foot of a groove's end wall where the floor
  rounds up into the part's side, the wall lower than the rounding there)
  runs on to the end and that face cuts it all along, as over a hole
  (0055), up to where its other contact reaches that face; there the blend
  ends in a point, the stripe's end is a singular free end and no corner
  is made at the spine's end vertex. Before, the rest was walked rolling
  on the wall's top edge and the corner failed.
- `0070-chfi3d-kpart-cut-to-end-where-the-narrow-face-vanishes.patch`: 0059
  where the wall vanishes (a groove's bottom exactly as deep as the
  rounding at its end wall's foot): along the middle of the chain the
  contact runs along the wall's top edge, towards the ends past it. A line
  on its face's boundary that leaves the hatching undone is hatched on the
  boundary (as 0014 does where no domain is found); the cut pieces start
  from such a piece's end on the wall's edge; and the inner ends of that
  piece's contact along a part of the edge (0054) cut the edge with no part
  of it kept for the wall, whose other pieces are gone too. Before, the
  walk got stuck (Path failed), or the edge's parts stayed with the face at
  the end.
- `0071-chfi3d-kpart-cut-by-several-faces-beyond.patch`: an analytic blend
  whose contact passes over a hole or cut-out whose outline is made of
  several faces (a slot through a wall rounded at its top edge: its
  straight sides, then its rounded end) is cut by those faces in turn,
  one piece per face; a face that cuts the blend along a section (a plane
  square to a straight spine) gives no piece, and the pieces beside it
  share only part of that section, the rest being that face's edge with
  the blend (`ChFi3d_FilDS`). A gap across the start of a closed spine is
  filled too, and a cut whose end is the start of a closed intersection
  line is found the other way round. Before, such gaps were left to the
  walking, which rolled on the hole's edge and failed.
- `0072-chfi3d-kpart-cuts-on-closed-spines.patch`: the cuts of 0055 and
  0071 on closed spines and conic cuts (a ring standing near a block's side
  chamfered at its foot as wide as it is high, the side cutting the
  chamfer's cone; a hole's rim near the side bevelled): intersection lines
  without p-curves (a cone and a plane meet in a hyperbola) are used, a
  piece whose lines run across their period's end gets its end a period
  later once the gaps are filled, and a contact along a part of a closed
  edge counts as such (0054), so the face whose far edge it is goes.
  Before, the cut was not found and the chamfer was left to a walking
  that is not implemented for chamfers.
- `0073-chfi3d-kpart-pieces-over-a-hole-are-one-face.patch`: the pieces of
  an edge's analytic blend over a hole or cut-out of its face (cut there by
  the faces beyond, 0055, 0057, 0071) are one face with the hole cut out:
  where a run of pieces returns to the faces it left, all on one surface,
  the pieces share one surface in the data structure and get no sections
  between them. Pieces split where a contact passes from one face to
  another keep their own faces. Before, the blend was a face per piece.
- `0074-chfi3d-walked-blend-cut-by-the-face-at-the-end.patch`: 0059 for a
  walked blend: where the walk stops short of the spine's end vertex
  because the contact on one face leaves it across an edge into the face
  at that vertex, the blend is walked on with that face's surface
  unbounded until the other contact reaches its face's edge at the vertex,
  and that piece is cut by the face at the end (`ChFi3d_KPartCutPiece`);
  the stripe ends in a point there, with no corner at the vertex. The
  case: the rounding of an inside corner between a wall and another wall
  whose top edge is rounded, the first wall's contact leaving it across
  its top edge before the corner's edge reaches the top. Before, the
  corner at the vertex gave an invalid result or failed.
- `0075-blendfunc-constrad-rolling-ball-section.patch`: the walked
  constant-radius blend between two surfaces (`BlendFunc_ConstRad`, and
  `BlendFunc_ConstRadInv` for its ends on restrictions) is a true rolling
  ball: the ball's centre lies at the radius along both surfaces' unit
  normals, and each section is its arc in the plane through the centre
  that holds both contact normals. The guide's parameter only picks the
  ball (the contact points' mid-point in the guide's normal plane). Before,
  both contact points lay in the guide's normal plane with the normals
  projected into it, which is the rolling ball only where the normals are
  square to the edge (0.014 off between a plate and a rod tilted by 40
  degrees, radius 2). Analytic blends were rolling balls already.
- `0076-chfi3d-extension-rolls-on-the-faces-own-surface.patch`: past the
  end of a spine, where a contact leaves its face with no face beyond, the
  blend rolls on the face's own surface, unbounded, when it is a cylinder,
  cone, sphere or torus, instead of on its tangent plane at the edge
  (`ChFi3d_BuildPlane`), so that it meets the piece before it.
- `0077-chfi3d-walk-past-the-spine-ends-before-a-singular-section.patch`:
  the walk into the straight extensions of the guide past the spine's ends
  stops where a rolling ball's contacts run twice as fast along the guide
  as at the spine's end, before the guide's planes come tangent to the
  centre's curve and the sections get singular (where the approximation
  failed).

The patches 0090 and later fix memory errors (mitcad#160).

- `0090-chfi3d-double-interference-at-vertex-kind.patch`: where two
  consecutive pieces of a chamfer or rounding meet on an edge they reach
  with opposite transitions, `ChFi3d_FilDS` adds a second interference on
  that edge. When the pieces meet at a vertex of the edge, the index it
  used is the vertex's, but the interference was made of kind point (the
  other calls there pass the kind); `TopOpeBRepBuild_Builder::BuildEdges`
  then counted that point past the end of its array of points and
  overwrote the heap (`free(): invalid next size` later; a debug build of
  OCCT throws `Standard_OutOfRange`). The interference now takes the
  index's kind, and the existing one is looked up with it. The case: a
  counterbore's circle on its floor chamfered by the floor's width, the
  circle in two arcs whose radii differ by rounding, so that the chamfer
  runs a hair inside the hole along one arc and on the floor along the
  other, the two meeting at the hole circle's vertex.
- `0091-topopebrepbuild-point-uses-within-array.patch`: `BuildEdges`
  counts the uses of a point only for indices within its array, as one of
  its four counts already did: an interference with a wrong index can no
  longer write outside it.

The patches 0100 and later change OCCT's offset (`TKOffset`, mitcad#172);
`geometry/tests/test_shell_vanishing.cpp` rebuilds the cases with exact
volumes, partly with OCCT alone.

- `0100-brepoffset-faces-vanishing-within-the-offset.patch`: a face whose
  offset vanishes within the offset distance (offset inside: a chamfer
  strip narrower than what the offsets of the faces beside it take from
  it, a convex rounding of a radius smaller than the wall; outside, the
  same concave) is dropped, and the offsets of the faces on both sides of
  it are intersected with each other instead, as if they were neighbours
  (`FindVanishingFaces`, `BRepOffset_Inter3d::ConnexIntAcrossVanished` and
  `ConnexIntByArcAcrossVanished`); the walks round the faces' wires skip
  its edges (`BRepOffset_Inter2d::ConnexIntByInt`). It finds faces of four
  edges whose offset turns inside out between two opposite edges all
  along them, also several in a row (a strip and the cones at its
  corners); a face that vanishes only in parts stays, and with the arc
  join one with a tube along an edge. Before, the shell failed (the thick
  solid had fewer faces than the body), a closed offset had no solid, or
  the solid was wrong: the inverted rounding kept, or with the arc join
  the body itself, unshelled. A thick solid to which the offset
  contributes no face is no longer done. The private methods
  `BuildOffsetByArc`, `BuildOffsetByInter`, `Intersection3D`,
  `IntersectEdges` and `ToContext` of `BRepOffset_MakeOffset` and three
  methods of `BRepOffset_Inter2d` and `BRepOffset_Inter3d` get parameters;
  no class gets members.

The patches 0110 and later change OCCT's face merge after booleans
(`ShapeUpgrade_UnifySameDomain`).

- `0110-shapeupgrade-unifysamedomain-chain-not-concatenated.patch`: a chain
  of B-spline edges whose curves cannot be concatenated (the end of one
  lies farther from the start of the next than the tolerance of the vertex
  between them, which `GeomConvert::ConcatC1` refuses with "Courbes non
  jointives") stays as its edges, as chains of other curves that cannot be
  merged do (`MergeSubSeq` returns false). Before, the exception ended the
  whole merge, and a join whose boolean had given a valid result failed
  (`BRepAlgoAPI_BuilderAlgo::SimplifyResult`).

The patches 0120 and 0121 change no result either: they remove repeated
work that made the checker (`BRepCheck_Analyzer`) quadratic in the wires
or edges of a face, on stored bodies of thousands of holes (mitcad#119).

- `0120-brepcheck-face-wire-boxes-of-conics-past-their-range.patch`: the
  check of how a face's wires lie inside one another
  (`BRepCheck_Face::ClassifyWires`) skips pairs of wires whose UV boxes do
  not meet, but gave up a wire's box when an edge's range reached past its
  p-curve's own parameters, which circles and ellipses of stored bodies do
  by a rounding error (an arc ending at 2π + 4 · 10⁻¹⁵): every pair of
  such wires was classified (8.6 million on a face of 2 929 holes, 3.5 s).
  A circle's or an ellipse's box is now the conic's over the edge's range.
- `0121-brepcheck-shell-orientation-edges-listed-once-per-face.patch`: the
  shell's orientation check (`BRepCheck_Shell::Orientation`) searched each
  face at each edge for the edge; a face of 64 edges or more now lists the
  orientations of its edges once (9.6 s → 0.1 s on a shell whose two faces
  hold 23 428 edges each).

The patches 0130 to 0139 make OCCT's rolling ball fillet build the
corners where a rounding ends that it failed on (mitcad#236);
`geometry/tests/test_fillet_corners.cpp` rebuilds the cases with OCCT alone.

- `0130-chfi3d-rounding-vanishing-at-a-tangent-end.patch`: a rounding whose
  two faces come tangent at the vertex where it ends vanishes there. Where
  the faces are tangent there only to within micrometres (a stored face, an
  arch whose radius misses the walls it springs from by a micrometre), or
  the walk stops a little short of the vertex where its sections get
  singular, the stripe ended in a section of micrometres off the vertex,
  and the corner failed or built a cap of micrometres whose wire
  intersects itself. Where both contact points of the end lie within a
  hundredth of the radius of the vertex, both faces hold it and their
  normals there are less than 0.002 rad from parallel, the walked contact
  curves' ends (in 3d and on the faces) are moved to the vertex within
  their tolerance, and the stripe ends in a point there with a degenerated
  edge (`ChFi3d_ShrinkVanishingEnd` in `PerformFilletOnVertex`), as where
  the walk reaches the vertex exactly.

The patches 0150 to 0159 make OCCT's fillet build roundings whose result
the topological build made invalid, or whose computation failed
(mitcad#239); `geometry/tests/test_fillet_results.cpp` rebuilds the cases
with OCCT alone, each test failing without its patch.

- `0150-chfi3d-onsame-corner-beside-a-split-face.patch`: the corner where a
  rounding ends against a face (OnSame) when the opposite face is split, at
  the corner's vertex, into two faces of one surface along an edge that
  lies in the face at the end (a slot cut into the side of a bar whose side
  is two faces split at the slot's back wall). The missing end of the
  prolonged edge went on the opposite face, which does not hold that edge,
  and the split edge stayed whole; the build kept its pieces under the
  rounding (an unorientable shell). Where the opposite contact ends on such
  an edge, the missing end is computed on the other face of the prolonged
  edge, and the split edge is cut where the contact ends.
- `0151-chfi3d-corner-on-an-end-face-split-in-one-plane.patch`: the corner
  where a rounding ends (OnDiff) on a face at the end that is two faces of
  one plane, each holding one of the arcs the ends lie on (a pad on a wall
  with a foot face of its own; the concave edge at the pad's bevelled end
  rounded wider than the bevel, the ball rolling on the pad's edge). No
  face held both arcs, and the intersection at the end built a curve from
  the wrong edge's geometry. Where exactly one arc holds the corner's
  vertex and both faces are planes of one plane facing the same way, the
  cap goes on the face of the other arc and the part of the first arc
  between its point and the vertex becomes the edge between the two faces
  (`ChFi3d_CoplanarSplitEndFace`); a point at a vertex is first given the
  edge of that vertex on the coplanar face.
- `0152-chfi3d-closed-rolling-start-on-a-seam-vertex.patch`: a closed
  rounding rolling on an edge (0011) from that edge's vertex, where a face's
  seam starts too (a disc's bottom edge rounded wider than the disc's side
  and bevel, the ball rolling on the bevel's top edge; disc, bevel and neck
  made of separate solids). The first point was found on the seam, and
  `ChFi3d_FilDS` cut the seam there; the build lost the faces above. A
  point at a vertex on a side rolling on an edge no longer cuts its arc at
  the first surface data, as at the last one already.
- `0153-chfi3d-roundings-meeting-on-one-contact-line.patch`: two roundings
  whose contact lines on the face between their edges are one line, the
  face exactly as wide as the two together (a plate 1.5 thick, both top
  edges rounded with 0.75). 0013 and 0016 handle contacts that overlap;
  coinciding ones failed in the corner (the two caps tangent at their
  common end point, taken as crossing) and in `ChFi3d_StripeEdgeInter`.
  Such a pair on a strip face between their edges is recorded as 0016's
  ride (the face standing for the support), so that the blends meet on the
  line and the face vanishes; caps tangent at their own end are not taken
  as crossing.

The patches 0140 to 0149 continue 0010 to 0019 and 0050 to 0077 for
chamfers (mitcad#240, mitcad#263): chamfers wider than a face beside them;
`geometry/tests/test_chamfer_overflow.cpp` rebuilds the cases with exact
volumes on synthetic shapes with OCCT's chamfer alone.

- `0140-chfi3d-contact-along-a-closed-face-edge.patch`: a fillet or
  chamfer exactly as wide as its face where the contact runs along a
  circular or closed edge (the inner rim of a step ring chamfered by the
  ring's width; a lug inside a bore as wide as the chamfer of its edge with
  the bore's wall). 0014's hatching of a contact on the boundary moved
  circles by translating them, which crosses the boundary: a circle is now
  moved into the face by changing its radius, a whole circle is hatched
  from a point outside the face, and domains across the start of a closed
  line are taken across it (`HatchOnBoundary`). A line on the boundary
  that the hatcher split into partial domains is hatched on the boundary
  too (`LineOnBoundary`). On a closed spine a piece reaching once round
  from before the start of the first edge is no longer dropped
  (`SplitKPart`). A closed contact along a closed edge counts as along it:
  its ends at the edge's vertex are that vertex, its curve is cut at the
  vertex inside it, and the edge is cut where a closed spine starts
  elsewhere on it (`ChFi3d_ContactsAlongFaceEdges`). Before, such chamfers
  failed ("Path failed", or the walking that chamfers do not implement) or
  kept the consumed face with zero width.
- `0141-chfi3d-onsame-end-second-contact-beyond-the-edge.patch`: the end of
  a chamfer in the OnSame state (`PerformIntersectionAtEnd`) whose second
  contact runs on past the vertex to the face at the end, beyond that
  face's edge (the foot of a rib on a wall chamfered wider than the rib's
  bottom, which the rib's sides cut): the contact's point on the face's
  edge prolonged is looked for on the face's unbounded surface, as for the
  first contact. Before, with the chamfer's faces in that order, the point
  outside the face was not found and the corner failed. Fillets keep the
  bounded face (the unbounded one changed a rounding of a corpus design
  that the import matched to one it does not).
- `0142-chfi3d-kpart-cut-by-faces-beyond-that-grow.patch`: 0055 and 0071
  for an analytic blend that adds material (a boss's foot under a plate
  chamfered so wide that the contact on the plate runs a little past the
  plate's rounded corner and sides): the faces beyond grow down to the
  cut instead of being cut back. The crossings of the chain of faces
  beyond are found on the prolongations of the edges between them beyond
  their ends (`ChFi3d_KPartEdgeCrossing`); there the pieces meet at a
  vertex of their own, the edge is prolonged up to it on both its faces
  (`ChFi3d_ProlongEdgeBeyondVertex`), and the edges between the contact's
  face and a face of the chain wholly under the blend are dropped. On a
  closed spine the gap from the last piece round to the first is filled
  too, with the parameters taken across the period's end; a cut that the
  intersection gives in arcs of one closed curve is taken on that curve;
  the other contact at a given end on its face's seam is put on the seam;
  and a contact along a face's edge (0014) finds the side of its blend from
  the other contact's nearest point (a cut piece's lines have other
  parameters). Before, such chamfers failed (their walking is not
  implemented).
- `0143-chfi3d-chamfer-corner-beyond-the-common-face.patch`: the corner of
  two chamfers whose contacts on their common face leave it across arcs
  into one face beyond before they cross (a hexagonal prism under a disc,
  chamfered wider than the disc's underside near the hexagon's corners;
  the disc's cylinder cuts the chamfers there). Where the corner by
  intersection fails, `ChFi3d_ChBuilder::PerformTwoCorner` gives each
  stripe a piece cut by the face beyond, from its contact's point on the
  arc to where the chamfers' intersection meets that face, and computes
  the corner between those pieces, on the face beyond; a seam of the face
  beyond whose prolongation that point lies on is prolonged to it on both
  its sides. While that corner is computed (a flag of the calling thread),
  `ChFi3d_IsInFront` takes the corner vertex's parameters on the face
  beyond from the nearest point of its surface; elsewhere it fails as
  before. Before, the corner failed.
- `0144-chfi3d-closed-contact-split-at-the-edges-vertex.patch`: a closed
  contact along a closed edge (0140) that passes the edge's vertex inside
  the stripe (a step ring's rim chamfered or rounded by the ring's width,
  the counterbore's seam turned away from the rim's vertex) splits the
  SurfData there into two on one blend surface, one face as 0073's pieces,
  each with lines of its own and its p-curve on the face beyond in that
  face's period (`ChFi3d_SplitAtVertexOnLine`). 0140 cut the line at the
  vertex: the part past the seam had its p-curve on the counterbore's wall
  a period beyond the face, and the topological build translated the
  shared p-curve on the blend in place for one part, moving the other's
  too (an unorientable face; for the rim in two arcs a valid shape whose
  p-curves on the cone missed their edges). `ChFi3d_PCurveInFacePeriod`
  moves a face interference's p-curve on its face by whole periods into
  the face's bounds.
- `0145-chfi3d-kpart-pieces-in-their-faces-periods.patch`: on a closed
  spine, the pieces cut by faces beyond over a gap (0055, 0071, 0142) past
  the spine's start held the other contact's p-curve a period beyond its
  face's seam (the boss of 0142 longer than the chamfer, its seam in the
  gap: an unorientable face); once the gaps are filled each piece's p-curve
  on its face is taken into the face's period.
- `0146-chfi3d-kpart-pieces-one-face-round-a-closed-spine.patch`: 0073's
  join of a blend's pieces into one face on a spine closed by one edge: the
  piece across the spine's start holds its spine parameters a period on
  (the gap between pieces is now taken modulo the period), a run of pieces
  cut by faces beyond may go on across the start (runs are followed round
  the spine, each piece taking the surface of the one before it in its run,
  with no run making a loop), and the joined pieces' p-curves on the blend
  go on from the piece before's in its period
  (`ChFi3d_PCurvesOnSurfAfter`). The cone of 0142's boss came in four or
  five faces; now it is one, as in the file's body.
- `0147-chfi3d-onsame-end-prolonged-along-a-seam.patch`: the OnSame end of
  a stripe whose face at the end meets the opposite face along that face's
  seam (a lug in a bore, its top's edge with the wall blended as wide as
  the top, the lug ending at the bore's seam). `PerformOneCorner` takes the
  prolonged edge's p-curve on the opposite face as that face uses the edge
  (an edge left from a seam, used once, keeps both p-curves; the other one
  put the prolongation where the blend covers the wall), and cuts the seam
  where the prolongation along it ends, its part along the prolongation
  kept on neither side (`ChFi3d_CutSeamAlongCurve`). Before, the wall's
  wire intersected itself.
- `0148-chfi3d-chamfer-as-wide-as-a-ledge-along-a-line-and-an-arc.patch`:
  a wall's foot of a line and an arc on a ledge as wide as the chamfer.
  `HatchOnBoundary` takes a moved line as on the boundary when none of its
  samples is outside the face and at least two are on its boundary (the
  arc's contact runs on into the face where the line's begins);
  `ChFi3d_CutConsumedFaces` accepts a consumed face's strip whose vertices
  are those of any obstacle edge of the stripe's side (the ledge's top
  along the line's and the arc's edges); `ChFi3d_ContactsAlongFaceEdges`
  makes a contact's end along a whole open edge, near the edge's vertex,
  that vertex. Before, the chamfer failed or its wires intersected.
- `0149-chfi3d-end-snapped-onto-a-closed-edges-vertex-on-a-periodic-face.patch`:
  `SnapToVertex` on a periodic face (a boss as long as the chamfer on its
  free end, the contact along the closed edge at the boss's foot): when the
  vertex projects onto the line's other end, leaving a piece of no length,
  the vertex's parameters a period on are tried. Before, the chamfer
  failed.

The patch 0160 goes on with the rolling ball fillet's walks (mitcad#238).

- `0160-chfi3d-walked-blend-over-a-rounded-rim.patch`: a walk whose
  contact reaches an edge into a face that continues its face tangentially
  but bends towards the ball more sharply than the ball (a hole's or
  cut-out's rim rounded with a smaller radius) no longer goes onto that
  face (`StartSol`): the ball cannot touch it. Inside the spine the blend
  is walked on over the gap on the face's own surface, unbounded, until
  the contact comes back into the face, and the rim cuts it
  (`WalkedBridgeOverGap`, with 0055's `ChFi3d_KPartCutPiece`; where the
  cut crosses the rim's seam, two pieces, the seam's end under the blend
  dropped); at the spine's ends it rolls on past the face's boundary on
  the face's own surface (0076). Of several arcs of the cut,
  `ChFi3d_KPartCutCurve` takes one on the rim's face first. Before, the
  walk's first step left the rim at once and the fillet failed.

The patches 0170 and later make the blend that rolls on an edge (where a
rounding's face is narrower than the radius) a true rolling ball, as 0075
did for the blend between two faces, and let it turn about the edge's ends
(mitcad#237); `geometry/tests/test_fillet_rolling_ball.cpp` checks the
cases with OCCT alone against the ball's centre curves.

- `0170-brepblend-surfrst-rolling-ball-and-pivot.patch`: the surface/edge
  blend (`BRepBlend_SurfRstConstRad`, and the inverse functions for its
  ends) puts the ball's centre at the radius along the surface's unit
  normal and at the radius from the edge, the edge tangent to the ball;
  the guide only picks the ball (the contact points' mid-point in its
  normal plane, as 0075), and each section is the ball's arc in the plane
  holding both contact directions. Before, both contacts lay in the
  guide's normal plane (6.5 · 10⁻⁴ off on a rib whose far side tapers,
  radius 3). The ball leaves the edge for the face beside it where it
  reaches into that face. Where the edge ends and the edge after it turns
  away from the ball, the walk holds the contact at the end and turns the
  ball about it until it can roll on (`StartPivot`). Where a circle on the
  axis of a surface of revolution is the edge, the contact lies in the
  plane through the axis (also where the ball's centre is on the axis).
- `0171-chfi3d-rolling-ball-turns-about-a-vertex.patch`: where a blend
  rolling on an edge reaches a vertex whose next edge runs away from the
  ball, the next blend is the ball's turn about the vertex, on the old
  edge (`StartSol`), and where the contact stayed at the vertex the same
  edge goes on; a side reduced to a point becomes a degenerated edge
  (`ChFi3d_FilDS`).
- `0172-chfi3d-rolled-blend-cut-by-the-face-at-the-end.patch`: 0074 for a
  blend rolling on an edge: where its contact on its face leaves the face
  into the face beyond the edge before the spine's end (a pocket whose
  floor is a lens narrower than the radius between its wall and a post's
  rim), the blend is walked on, still rolling on the edge, until the ball
  reaches that face, and cut by it (`RolledCutToEnd`); the stripe ends in
  a point on the edge. Before, the corner at the lens's tip was invalid.

The patch 0180 lets a fillet's contour stop where its edges turn although
the faces go on tangentially there (mitcad#241); Mitcad's fillets use it, so
that a tangent chain runs on only where its edges continue each other, as
in the `.f3d` files' roundings, and `geometry/tests/test_fillet_chains.cpp`
checks it with OCCT alone.

- `0180-chfi3d-chains-stop-where-the-edges-turn.patch`: OCCT grows a
  contour into the next edge at a vertex wherever no other sharp edge
  meets there (the faces across a crease of up to 0.1 rad count as
  tangent), as long as the edges turn by less than a right angle; a
  rounding of the top edge of a wall that is drafted by 3 degrees above a
  vertical part, or of a cylinder that meets another at 0.7 degrees, ran on
  over the next edges. `StopChainsAtTurns` (`ChFi3d_Builder`,
  `BRepFilletAPI_MakeFillet`) makes the contours added afterwards stop at
  a vertex where the edges turn by the angular tolerance or more, unless
  the next edge is one of the edges given (the selection, which so still
  makes one contour); the vertices where it stopped one are recorded. Past
  such a vertex, where the faces meet across a concave crease, the walk
  past the stripe's end stays on the face's tangent plane instead of
  changing onto the face beyond (`StartSol`), and that face, its surface
  extended, cuts the blend in the corner at the vertex
  (`PerformOneCorner`), as in the files' roundings; an analytic blend
  whose contact ends on that crease ends there (`SplitKPart`,
  `EndsOnTurnCrease`) and stays analytic instead of being walked (a torus
  with circles for contacts, as in the files). Where the extension is
  purged on both sides and the first purge removed it, the second no
  longer removes the surface data then at that end (the stripe was left
  without its blend). Not done: a convex crease (the face beyond meets the
  rounding only past the vertex; the walk past the end fails as before, and
  so does the fillet); chamfers (their corner at such an end is not the
  face beyond's cut, so `BRepFilletAPI_MakeChamfer` does not offer the
  option). Without the call nothing changes.

The patch 0190 goes on with 0160's walks over a rounded rim (mitcad#261).

- `0190-chfi3d-consumed-face-round-a-closed-stripe.patch`: a closed
  rounding whose start overflowed a face of its edge all round (a disc's
  bottom edge rounded with 1, the disc then turned down by 0.1 and the
  edge between the turned cylinder and that rounding rounded with 1: the
  ball touches the cylinder and the bottom, the earlier rounding lies
  wholly between the contacts) and whose contact on the face beyond
  crosses a hole with a rounded rim. 0160 refused its walk over the rim
  where the face the contact runs on is not a face of the spine's edges,
  and the fillet failed ("Chaining is impossible"). Round a closed spine
  such a face beyond a consumed face (`ChFi3d_ConsumedSpineFaces`: a face
  of the spine's edges that no surface data lies on) is taken; the rim's
  edges with the consumed face vanish with it (no edge to grow at their
  vertices under the blend), and the topological build drops the consumed
  face once its neighbours are cut. Where the other contact crosses a seam
  of its face inside the walk over the rim, the stretch's cut piece ended
  at the section through the spine's point there, not at the walked
  piece's own end section (the rolling ball's sections are not in the
  guide's normal planes: 0.011 apart), and its point on the seam was
  looked up on the seam's first occurrence; the seam kept its piece under
  the blend and with it a part of the cylinder and the consumed face (a
  valid shape 129 mm³ smaller than the file's). A piece now ends at the
  walked piece's own end section with the walk's common point on the arc
  (`ChFi3d_KPartCutPiece`'s `AtFullEnds`, `ChFi3d_KPartSectionCutAt`), and
  the next stretch starts on the arc as a walk goes on across a seam.

The patch 0200 lets a walked blend between two faces turn about a contact
that stands still (mitcad#239).

- `0200-chfi3d-walked-blend-about-a-standing-contact.patch`: where a
  rounding runs past a corner rounded with the ball's own radius (a box's
  vertical edge rounded with r, then its top edges with r; a pocket's
  floor whose inner corners are rounded with r), the ball's centre stays
  on the corner's axis: its contact on the top (the floor) stands at one
  point while the other sweeps round the corner, and the blend there is
  a piece of the ball's sphere whose side on the top is a point. On
  elementary faces OCCT builds that sphere analytically; on other faces
  (stored splines) the walk does it, and there the line got no
  transition on the standing side (its contact has no direction:
  `Standard_DomainError` from `BRepBlend_Line::TransitionOnS1/S2` in
  `StoreData` or `ComputeData`), the standing side's twist flag was the
  sign of numerical noise (the piece was then replaced by a filling
  surface), and `StoreData` took the blend's orientation from its normal
  at the point side, which is null, and failed. A contact whose tangent
  is below a thousandth of the other's now stands still: it is not
  twisted, and its transition is taken with the guide's tangent for its
  direction (`BRepBlend_Walking::TestArret`); where the blend's normal at
  its side 1 is null, the orientation is taken a hundredth of the
  section inside. The point side is stored as 0171 stores a ball turning
  about the end of an edge: a degenerated edge of the blend's face.

The patches 0210 to 0219 continue 0130's corners where a rounding ends
(mitcad#236); the tests in `geometry/tests/test_fillet_corners.cpp` fail
without them.

- `0210-chfi3d-mitred-ends-at-a-short-sharp-edge.patch`: two fillets that
  end alone at the two vertices of a sharp edge E (three sharp edges at
  each), each with a contact on E's face S and its other contact on an
  edge of E's other face (the face at both ends), and whose contacts on E
  overlap (E shorter than their two setbacks on it), were each cut by the
  face at the end as if the other were not there; the second cut crossed
  the first and the corner failed ("fillets have too big radiuses"). The
  two blends are now mitred (`ChFi3d_Builder::PerformMitredEnd`, before the
  one-stripe corner in `PerformFilletOnVertex`; both ends are made at the
  later of the two vertices): each ends in two curves, stored as the big
  stripe's end of `PerformTwoCornerbyInter` is, the blends' intersection
  from where their contacts on S cross up to the face at the end (shared by
  both), and its cut by that face from there to its contact on the face's
  edge. The contacts on S end where they cross, and E vanishes. Two
  cylinders of one radius whose axes cross meet in an ellipse
  (`IntAna_QuadQuadGeo`), whose shorter arc is the mitre, exact; other
  blends are intersected by `ChFi3d_ComputeCurves`' walk.
- `0211-chfi3d-end-cut-by-a-periodic-face-across-its-seam.patch`: where the
  face at a rounding's end (an OnDiff or AllSame corner) is periodic and
  its seam runs between the two arcs that hold the rounding's contacts,
  the arcs' curves on the face lie a period apart, and the cut of the
  blend by the face was sought between them round nearly the whole face
  (it then crossed the other roundings' ends, or the body came out
  invalid). The second point is now taken within half a period of the
  first (`PerformOneCorner`), and the cut crosses the seam, where it is
  split as before.

The patch 0230 keeps a blend's open contact in the adjacent face's
parametric period. Projecting the contact onto a cylinder can place the
whole arc one period beyond the face bounds: its 3D curve is correct, but
the wire does not close in 2D. The existing period adjustment now applies
to every newly projected contact, before the closed-contact seam handling.
The lug-in-bore chamfer and fillet cases in
`geometry/tests/test_chamfer_overflow.cpp` cover this failure.

The patch 0220 makes the `.f3d` import's replay keep one face where the
files have one (mitcad#284).

- `0220-shapeupgrade-unifysamedomain-seams-half-a-period-apart.patch`:
  a join of two coaxial cylinders of one radius, one on the other, leaves
  two faces of one cylindrical surface with their own seams. The face merge
  after the boolean removes the seams and the circle between the faces and
  builds the merged face's seam again from the removed edges
  (`ReconstructMissedSeam`); a removed edge that ends at the current vertex
  was judged by its far end, taken when that end lay more than half a
  period from the current point and dropped when less. Where the seams lie
  half a turn apart, the half circle between them ends exactly half a
  period away: neither test took it, and the faces stayed apart (a
  revolution joined onto an extruded cylinder; the files' bodies have one
  face). On a surface periodic in one direction the edge is now turned to
  start at the current vertex first and its own end there is compared.

The patches 0240 and 0241 go on with roundings whose computation failed
(mitcad#239, third round).

- `0240-chfi3d-extension-on-the-faces-own-surface-away-from-its-seam.patch`:
  past the end of a spine, where a contact leaves its face with no face
  beyond, the blend rolls on the face's own surface (0076), a face made of
  the whole surface within its natural bounds, whose seam is an edge
  without vertices. Where the ball heads for that seam close by (a small
  convex arc whose circle starts 0.08 radians before the arc, after a kink
  from a big arc, both rounded: the small arc's blend is extended back past
  the kink for the corner), the walk stopped on the seam
  (`Standard_NullObject` in `ChFi3d_FilCommonPoint`, then `FindFromKey` in
  the search for the faces around it). Where the ball, leaving the face,
  heads for a seam of the natural bounds less than a quarter period away,
  the bounds are now taken half a period on (`ChFi3d_BuildPlane`); other
  extensions keep the face they had, since moved bounds would part the
  extension's parameters from the walk's on the face before it.
- `0241-chfi3d-roundings-meeting-across-a-notch.patch`: a rim rounded on
  both sides of a small notch whose edges are not rounded (an arc broken
  at its lowest point by a V notch narrower than the rounding, whose
  contact on the top passes above the notch): the edge chain stops on both
  sides, one stripe ends at each side or both ends of one stripe meet
  there. The blends are one torus, over the notch the ball rolls on and
  the notch's faces cut the blend, and the files' bodies have one torus
  face there. OCCT ran each end's contact on the top on to the end of its
  domain on that face (SplitKPart's extension for the corner, here round
  the whole other half of the outline), the corner did not cut it back, and
  the two contacts overlapped (`ChFi3d_StripeEdgeInter`) or the wires
  intersected themselves. `ChFi3d_JoinAcrossNotches`, after the stripes
  and before the corners, finds such pairs of ends (each the only end at
  its vertex, analytic blends of one surface, contacts on one line of a
  common face that stays inside it over the gap, the other contacts on
  arcs of two faces of one surface), cuts the contacts on the common face
  back to the ends' sections, runs one end's blend on over the gap cut by
  the notch's faces in turn (the pieces of `ChFi3d_KPartChainPieces`, which
  now takes given ends where the piece before or after is missing), and
  makes the two ends free ends that share the meeting section's points. The
  blends are one face where their surfaces are one (the joined end's pieces
  take the other's surface, their p-curves mapped where the tori are
  parametrized otherwise); the two ends of one stripe share the section's
  edge (one face all round would have no seam). In `ChFi3d_KPartCutPiece`
  an edge of the face beyond with the other contact's face, from a vertex
  where it meets the contact's face too, no longer counts as an edge ending
  under the blend: it lies between the cut and the other contact and goes
  with both faces.

The patch 0250 keeps 0160's bridge over a rounded rim to faces that are
not along the contour (mitcad#308).

- `0250-chfi3d-walk-on-along-a-face-of-the-spine.patch`: 0160 made
  `StartSol` refuse a face beyond a tangent edge that bends towards the
  ball more sharply than the ball (`ballCannotTouch`), which also refused
  a face the contour itself runs along: a box whose vertical edges are
  rounded with 2, its bottom chamfered round the box, then the edge
  between the chamfer and a side face rounded with 5 (the contour goes on
  round the corners, between the chamfer's cone and the corner's
  cylinder). The spine turns with that face and the blend's sections
  touch it; the walk went on onto it before 0160, and after it no face
  continued the contact and the fillet failed ("Chaining is impossible").
  A face that borders an edge of the spine (`faceAlongSpine`) now
  continues the contact's face whatever its curvature along the spine;
  0160's test applies to the other faces. It also lets a top rounding go
  on round vertical corners rounded with a smaller radius than its own
  (mitcad#297: a rim of tabs and notches with corners rounded with 1 under
  a rounding of 1.5; there the corner's cylinder bends towards the ball
  inside the solid).

The patch 0260 goes on with 0241's roundings across a notch (mitcad#307);
Mitcad's fillets use it.

- `0260-chfi3d-fillet-carried-across-a-notch-in-its-rim.patch`: a rim
  rounded on one side of a small notch whose edges are not rounded, the
  next edge of the rim beyond the notch not given (a plate's top edge along
  an arc of a cylinder, broken by a tapered hole whose top edges are 0.28 to
  0.29 long, 0.25 deep and 0.57 wide, the radius 0.5): the files' bodies
  round that edge too, with one torus face over both edges and the notch,
  cut by the notch's faces. OCCT ended the contour at the notch and left
  the edge beyond it sharp (the result was invalid).
  `BRepFilletAPI_MakeFillet::CarryChainsAcrossNotches`
  (`ChFi3d_FilBuilder::AddAcrossNotches`), called after the contours are
  added, follows the boundary of a face of a contour's end edge from its
  end through edges no contour holds, each within the fillet's contact
  distance on that face of the end edge's line or circle (R tan(a/2), a
  the angle between the faces' normals) and within the ball's diameter of
  the end, to an edge of that face on the same line or circle whose other
  face lies on the surface of the end edge's other face; that edge is added
  as a contour of the same radius, just before the contour it is carried
  from, and 0241 joins the two ends over the notch (the joined face is the
  later stripe's, so it is generated by the given edge). Constant radii, lines and circles only; a notch wider than the
  ball (a groove 1 long and 0.0043 deep beside a rounding of 0.3, which the
  files' roundings do not cross) is left as it is. Without the call nothing
  changes.

The patches 0270 and 0271 go on with 0153 (mitcad#239).

- `0270-chfi3d-roundings-meeting-on-one-contact-line-of-a-face-going-on.patch`:
  two roundings whose contact lines on the face between their edges are
  one line, where that face goes on past the line's ends (a rib as wide as
  the two roundings standing out of a plate or a ring, its top one face
  with theirs; a slot as wide in a wall, its floor one face with the floor
  in front of the wall). 0153 recorded the pair only where the face is a
  strip that vanishes whole, so here `ChFi3d_StripeEdgeInter` failed. The
  pair is now recorded where neither stripe is closed; `ApplyRides` makes
  the blends meet on the line when each end of it is on one arc of the
  face for both (as 0153) or a point inside the face where both corners
  end, having cut the face off there (the edge of the face at the end
  prolonged across the root). At such a point each corner made its own
  point: the later stripe's is made the earlier one's throughout the data
  structure, so that the face's wire closes there. The face keeps its part
  past the line's ends.
- `0271-chfi3d-roundings-cut-by-each-other-where-their-contacts-cross.patch`:
  two roundings whose contacts on the face between their edges cross each
  other twice, the face narrowing to less than their two widths part of
  the way (a block whose long sides are concave arcs, the top 5.8 wide in
  the middle, both top edges rounded with 4; a channel whose walls bulge
  in, its floor edges rounded). The files' bodies cut the two blends by
  each other between the crossings and keep the face in two pieces; OCCT
  failed in `ChFi3d_StripeEdgeInter`, or, where 0013 took the contacts for
  overlapping all along, in the stripes computed again. After the corners,
  `trimCrossingStripes` finds the blends' intersection between the
  crossings (one blend's sections cut by the other's surface, interpolated;
  the blends are tangent to the face at the crossings, a node of the
  surfaces' intersection), splits each SurfData in three on its surface,
  and gives the middle piece the intersection on no face, shared by both
  stripes; `meetOnCurve` makes the two stripes' points at the crossings one
  once both are in the data structure. A pair that 0013 computed again is
  computed on the faces of its edges once more when its contacts as first
  computed crossed twice and it failed or still crosses. Open stripes with
  no spine end in common only; other pairs are left as they were.

The patches 0280 to 0283 build corners where a rounding ends among
several faces (`PerformIntersectionAtEnd`, `PerformOneCorner`,
mitcad#236).

- `0280-chfi3d-onsame-end-where-the-face-at-the-end-turns-tangentially.patch`:
  a convex corner's top edge rounded up to the vertex where the top plane
  turns tangentially into a rim's torus: the wall's contact reaches the
  wall's edge with the torus before the vertex, the top's contact crosses
  the top's tangent edge with the torus past it. The files' bodies cut the
  blend with the torus up to the point where the top's contact meets that
  edge, which runs on to it; OCCT failed at the corner. The top's contact
  is a circle that meets the torus only on its prolongation across its
  parameter origin, which the reframing in `Update` put out of range: such
  a point is now taken where nothing else is found. The missing end of the
  prolonged edge was sought as the intersection of two faces that touch
  along it: it is now the edge's own curve and p-curves continued to the
  contact's end (`ChFi3d_ProlongTangentEdge`).
- `0281-chfi3d-walk-round-the-vertex-past-a-short-edge.patch`: the walk
  round the vertex in `PerformIntersectionAtEnd` takes, at each vertex it
  passes, the common edge of the faces on both sides there (two faces may
  share several edges; the first found was taken, the far end of a
  groove), and an edge it reached past a short edge of the vertex (13
  micrometres, the corner's wall above a rim's torus) is prolonged from
  the vertex where the walk reached it, the short edge in the removed
  corner (before, from the corner's vertex, which is off the faces'
  intersection: self-intersecting wires).
- `0282-chfi3d-end-cut-past-the-bounds-of-the-faces-at-the-end.patch`: a
  wall's foot rounded along the floor up to where a hole in the floor
  beside the wall begins: the floor's contact meets the hole's side, the
  wall's runs on. The files' bodies cut the blend with the hole's side
  and corner chamfer continued up through it; the second contact's end is
  now sought on the last face's unbounded surface where its bounded one
  gives none (fillets; chamfers already since 0143).
- `0283-chfi3d-standing-end-piece-one-face-with-the-one-before.patch`:
  there, the blend's piece along the next edge of the line, whose floor
  contact stands still, is one face with the piece before it
  (`ChFi3d_JoinStandingEnd`: it takes that piece's surface, its p-curves
  mapped by 0241's `ChFi3d_AnalyticParamMap`), and the corner puts no
  section between them; before, the blend was two faces where the files'
  bodies have one.

The patches 0290 and 0291 go on with the corners where a rounding ends
(`PerformIntersectionAtEnd`, `PerformOneCorner`, mitcad#236).

- `0290-chfi3d-walk-round-the-vertex-through-a-face-met-at-a-point.patch`:
  the walk round the vertex steps, at each vertex it passes, from the face
  across one edge of the stripe's face to the face across the next, which
  share an edge where three faces meet. Where more meet (a cavity's
  ceiling and a rib's top under a groove along the wall, coplanar, meeting
  at the point where the groove's side crosses the rib's side) they share
  none and the corner failed ("Failed to find edge."). The faces round
  that vertex between the two are now faces at the end
  (`ChFi3d_FacesBetweenAtVertex`), where the two share no edge at all; a
  face the fillet crosses at a single point gets no curve, the edge beyond
  it is prolonged to that point (the nearest extremum on the faces'
  intersection curve, the short way on a periodic one) with the
  orientation of the edge it prolongs; the edge crossed from the first
  face, its ends on both stripe faces, is cut on the side of the end
  where the walk reached it; and the OnSame edge beside the face holding
  the contact is prolonged up to the contact (as 0026 does).
- `0291-chfi3d-end-past-the-spine-where-a-contact-ran-off-its-face.patch`:
  a rounding along a top face's edge with a wall, where the top narrows
  to nothing at the spine's end and the wall runs on past that vertex
  into a face tangent to it: the top's contact runs off the top before
  the end, and the piece walked from there past the spine's end on the
  top's tangent plane has that side given up (`ChFi3d_Purge`). The corner
  dropped that piece and failed. The files' bodies run the blend on past
  the vertex, cut along that side by the sloped face beyond the top and
  at its end by the face beyond the wall. The given-up piece now takes the
  surface of the elementary blend before it where it lies on it, its
  p-curves projected (`ChFi3d_ExtensionOnNeighbourSurface`, one face up to
  the corner); `PerformOneCorner` keeps it where both its ends at the
  corner lie on edges and the piece before it does not end on edges; and
  `PerformIntersectionAtEnd` takes, as the given-up side's face, the face
  it had before, and bounds such a blend wider along its spine where it
  does not cross an edge between the faces at the end within its bounds.

The patch 0300 goes on with 0150 to 0153 (mitcad#239).

- `0300-chfi3d-prolonged-end-edge-across-the-opposite-faces-seam.patch`:
  a concave rounding of a rib's foot on a rod, its end cut by the rib's
  end face: the end face grows down to the rounding's contact on the rod,
  its edge with the rod prolonged from the vertex to the contact
  (`PerformOneCorner`). Where the rod's seam runs between the foot and the
  contact, the prolonged edge is cut at the seam, and the two pieces'
  p-curves on the end face were computed on the rod instead (edges of
  several mm tolerance, an unorientable end face). They now take the
  intersection's own p-curve on the end face, trimmed at the seam.

The patches 0320 and 0321 come from the split of the fillets the `.f3d`
import's census found failing along their edges (mitcad#322,
`core/import/docs/f3d-rounds-2.md`, *Fillets that fail along their edges*).

- `0320-chfi3d-where-a-fillet-stops.patch`: a fillet that is not done
  told its faulty contours with a status and its faulty vertices, but not
  where it stopped. `ChFi3d_Builder::Failure()` (also
  `BRepFilletAPI_MakeFillet::Failure()`) now gives the stage: a contour's
  blend finding no start, its walk between two faces or rolling on an edge
  stopping, a twisted surface or another failure (an exception in the
  stripe's computation); blends overlapping or crossing on a face that fail
  when computed again; the blends of two contours intersecting each other
  (`ChFi3d_StripeEdgeInter`, whose stripe kept the status Ok); a corner, or
  a corner built in part only. `FailedContour()` is the contour it stopped
  at. The computation is unchanged; Mitcad's dressup names the stage in
  its error.
- `0321-chfi3d-closed-rounding-consuming-its-face-ends-at-the-vertex.patch`:
  a slot's rim (two lines and two half circles) under a counterbore 0.5
  wider, rounded or chamfered with 0.5: the blend consumes the ledge between
  the two all round, each piece's contact running along a whole edge of
  the ledge (0014's blend on the edge). Those ends become the edges'
  vertices, but at the closed stripe's start and end the stripe kept the
  point it had made beside that vertex, and the topological build failed
  (`TopoDS_Builder::Add`). The stripe's point there is now the vertex, as
  0014 does for a contact along a closed edge (a round hole built before).
- `0322-chfi3d-open-stripe-end-at-the-vertex-of-the-edge-its-contact-runs-along.patch`
  (mitcad#331): the same at the ends of an open stripe, where it corrupted
  memory. A fin 1.5 thick on a wall, its top and bottom flush with the
  wall's, the edges round one side rounded with 0.75 and then those round
  the other side: the second rounding's contacts run along the whole edges
  the first one left on the fin's top and bottom, up to the wall, and 0014
  made their ends there the edges' vertices while the stripe kept the
  indices of its own points. `ChFi3d_FilDS` takes the kind of a stripe's
  end from its common point, so it put those points' indices into the DS
  as vertices'; with OCCT's exceptions off (release builds)
  `ChFi3d_Builder::Compute` then read and updated the tolerance of the
  DS's shapes of those indices (the solid's, a face's) as vertices', past
  the ends of those objects, the build failed (`TopoDS_Builder::Add`) and
  the process crashed later on the corrupted heap (an `.f3d` import, in
  freeing the fillet's copy of its input or in comparing faces). Now the
  stripe's index at such an end is the vertex's for open stripes too, and
  what the DS already has at the old point (the end's curves and edges) is
  put at the vertex (`ChFi3d_StripeEndAtVertex`); the fin builds, valid,
  each side's rounding taking off its exact volume.

The patches 0330 and 0331 come from the split of the fillets the `.f3d`
import's census found failing at a vertex (mitcad#323,
`core/import/docs/f3d-rounds-2.md`, *Fillets that cannot end or join at a vertex*).

- `0330-chfi3d-which-corner-a-fillet-fails-at.patch`: a fillet whose
  corner at a vertex failed told the vertex (`FaultyVertex`, with 0320
  `Failure_Corner`), but not which corner it tried there nor why. For
  each faulty vertex `ChFi3d_Builder::FailedCorner(IV)` (also
  `BRepFilletAPI_MakeFillet`) now gives the corner `PerformFilletOnVertex`
  chose (a rounding's end, mitred, over several pieces or where more than
  three sharp edges meet; two or three roundings meeting; more; roundings
  ending in points), `FailedCornerBlends(IV)` and `FailedCornerEdges(IV)`
  the roundings and sharp edges meeting there, and
  `FailedCornerMessage(IV)` the message of the exception it failed with.
  The computation is unchanged; Mitcad's dressup names the corner and the
  cause in its error.
- `0331-chfi3d-rounding-rolled-on-a-straight-edge-exactly.patch`: a
  rounding wider than the face beside its edge rolls on that face's far
  edge (0170's rolling ball). Where the ball touches a plane and rolls on
  a straight edge parallel to it along a straight guide, its centres lie on
  a line and the rounding is a cylinder; OCCT stored the walked
  approximation (a B-spline surface and curves). The stored designs have
  the cylinder, and a later rounding of edges of the result failed on the
  approximation. The SurfData now gets the cylinder and the contacts'
  lines, on the same parameters (`ChFi3d_ExactRollOnLine`, after
  `CompleteData` in the two `PerformSurf` of a constant blend rolling on a
  restriction, only where the walked contacts lie on the exact ones; the
  cylinder trimmed in U to the section, as the walked surface is, so that
  0390's ball at the blend's end is found from its bounds). The
  edge on the rolled edge, where the rounding meets the face beyond at an
  angle, gets no regularity with 0350.

The patches 0340 to 0342 go on with the corners where a rounding ends
(mitcad#236): OnSame ends of roundings wider than the face they end
against or than their top, whose balls roll on that face's edge (0170).

- `0340-chfi3d-onsame-end-cut-by-the-face-of-the-edge-the-ball-rolls-on.patch`:
  a ridge on a rod, its lens-shaped end face (2.5 high) rounded with 5: the
  ball touches the rod and rolls on the lens's edge with the ridge's side,
  an edge from one end of the spine to the other, and past each end rolls
  on until it comes tangent to the ridge's side. OCCT took the lens for
  the face at the end and failed ("bouchon non ecrit", "no point on
  arc"). Now the ridge's side itself is the face at the end
  (`ChFi3d_RolledEndOnItsFace`, `PerformOneCorner`): it cuts the blend from
  the point of tangency to the rod's contact, its edge with the rod
  prolonged; the rolled edge goes from it on both sides of that point.
- `0341-chfi3d-rolling-blend-walked-again-across-a-seam-past-its-ends.patch`:
  the same where the rod's seam runs past an end between the lens's tip and
  the point of tangency: the walk stopped on the seam (0011 then ends a
  rolling blend at the spine's end), short of that point. The blend is
  walked again from its middle with the rod's surface grown, its seam half
  a period away (`RolledEndToEndAcrossSeam`).
- `0342-chfi3d-rounding-rolled-on-past-its-edge-cut-by-the-faces-at-the-ends.patch`:
  an ear on a boss, its top narrower than the radius between the boss and a
  chamfer, the top's edge with the boss rounded: the ball rolls on the
  top's edge with the chamfer, whose ends lie on the ear's sides; OCCT
  turned it about the ends (0171) and rolled it on the sides' edges, and
  the corners failed or the result was invalid. The blend is walked again
  rolling on the edge's line, the edge of a strip of the chamfer's surface,
  up to the spine's ends (`RolledOnProlongedEdge`), and each end is an
  OnSame corner cut by the ear's side from the edge's end
  (`ChFi3d_RolledPastEdgeVertex`), also at a vertex of more than three
  edges where the side meets the boss at the vertex only, their new edge
  the surfaces' intersection from there (`PerformRolledPastEnd`,
  `ChFi3d_CurveFromVertexToContact`).

The patches 0350 to 0352 go on with roundings and chamfers that consume a
face beside their edges (mitcad#300).

- `0350-chfi3d-regularity-only-where-the-faces-are-tangent.patch`: a
  rounding whose contact runs along the far edge of a face it consumes (a
  boss's foot rounded as high as the boss) marked that edge, now between
  the blend and the face beyond (the boss's top, at a right angle), as
  tangent (`SetRegul` took the face beyond for the image of the consumed
  face the blend is tangent to); so did a rounding with the walls of a hole
  cut through it before. A later rounding of such an edge failed: the
  boss's top edge, one closed edge, was taken for a spine ending at its
  vertex, and the walk stopped at once ("Path failed", for any radius); at
  a hole's corner the walk went on past the corner onto the torus. The
  regularity is now encoded only where `ChFi3d::IsTangentFaces` finds the
  faces tangent along the edge.
- `0351-chfi3d-closed-chain-consuming-a-face-all-round.patch`: a pocket's
  wall foot of four lines and four arcs on a ledge as wide as its chamfer
  (or rounding): the contact on the ledge is an unbounded line along the
  ledge's boundary, which the hatcher found inside the ledge only beyond
  the line's ends; `LineOnBoundary` (0148) now looks at such a line
  alongside the spine's edge, and the line is hatched on the boundary. And
  the closed stripe's start, its contacts ending at the vertices of the
  ledge's edges (0148), keeps those vertices as its points
  (`ChFi3d_ContactsAlongFaceEdges`); the topological build took the point
  made before for a vertex (TopoDS_Builder::Add). `IsInput`, asked for
  those vertices, takes a spine tangent square to the face's edges there
  as not running into the face (its angle was measured from a null
  vector). Before, the chamfer failed ("Path failed") or threw.
- `0352-chfi3d-contact-on-a-face-beyond-at-an-acute-angle.patch`: the
  transition of a contact put onto the face beyond its face's edge (0014,
  `ChFi3d_ContactsAlongFaceEdges`) is taken on the side of the edge outside
  that face, from the edge's direction in it; before, the side toward the
  other contact, which lies under the face beyond where the blend folds
  back under it (a convex lip over a wider pocket chamfered as far as its
  wall's top and its underside's far edge): an invalid shape. The same
  wrong side made a wall's foot on a flange chamfered as wide as the
  flange's top, the flange's rim rounded on beyond it tangentially, invalid
  (mitcad#327).

The patches 0380 and 0381 go on with 0140 to 0149 for chamfers
(mitcad#263): a chamfer up a crease between a straight side and an arc
and on over the rim's roundings to the top.

- `0380-chfi3d-chamfer-end-where-the-face-at-the-end-is-tangent-to-both-faces.patch`:
  the walk reaches the vertex on the top where the chain's edge meets the
  two rims' edges, along which the roundings join the top tangentially;
  its contact on the straight side's rounding ends on that rim, the other
  inside the arc's rounding. The corner by intersection with the top found
  nothing, the top being tangent to both faces, and the general corner
  threw in its projection: the chamfer failed. A ruled piece now closes
  such an end (`ChFi3d_ChamferEndOnTangentFace`, after `WalkedCutToEnd`):
  from the end section to the point of the other rim as far from the
  vertex as the section's point on the first, its sides along lines of the
  top's and of the rounding's parameters; the stripe ends in that point,
  and `ChFi3d_FilDS` cuts the rim there although the end has a degenerated
  edge.
- `0381-chfi3d-chamfer-guide-deformed-by-a-hundredth-of-its-distance-at-most.patch`:
  `ChFi3d_PerformElSpine` made the chain's guide C2 by removing knots
  within up to a tenth of its length; along the wall the guide ran 0.03 to
  0.08 off the crease, and the chamfer's contacts, at its distance from
  the guide, moved with it. A chamfer's guide now keeps knots whose
  removal would move it by more than a hundredth of the chamfer's smallest
  distance (`ChFi3d_ChamferGuideDeformation`); fillets are unchanged.

The patch 0360 lets a boolean split a face where two of its new edges
leave a vertex tangentially to each other (mitcad#235).

- `0360-bopalgo-wiresplitter-tangent-edges-measured-at-one-distance.patch`:
  the wire splitter (`BOPAlgo_WireSplitter`) orders the edges at each
  vertex by their directions, each measured at a step of its own from the
  vertex: on a circle about sqrt(2 R tol) away, on a B-spline a step of
  its parameter, often a tenth of that distance. A counterbore cut through
  an internal thread whose crest lies on the hole's circle splits its end
  face by the circle and by the thread's section, which leaves the circle
  tangentially: the circle, measured farther out, had turned more than the
  section beside it though the section turns faster, the two came out in
  the wrong order, and the face between them came out with a negative
  area; the cut left the body empty (a part's counterbores through
  threaded holes). Edges within 0.1 radians of each other at a vertex are
  now measured again by the chord to their points at one distance from it
  (`RefineTangentAngles`).

The patches 0390 and 0391 go on with 0170 to 0172 (mitcad#237): a ball
rolling on an edge that meets an edge turning towards it, and a ball
resting on the edges of both faces; `geometry/tests/test_fillet_rolling_ball.cpp`
checks them with OCCT alone.

- `0390-chfi3d-rolling-ball-touches-the-next-edge-before-the-vertex.patch`:
  where the edge E1 the ball rolls on ends at a vertex whose next edge E2
  of the reference face turns towards the ball (a rib's far side that
  thickens towards the ball, a thin rim's inner edge turning into a spoke),
  the ball touches E2 before the vertex. The walk rolled on through E2 into
  the solid up to the vertex, and the next blend started on E2 at the
  vertex (a rib, radius 3: 2.6 · 10⁻³ off the ball). Now the walk stops
  where the ball touches E2 (`NextEdgeGap`, `FindConcaveTurn`, with the
  next edges' curves from `ChFi3d_NextEdges`), `StartSol` goes on rolling on
  E2 from the ball's contact there (`ChFi3d_ConcaveTurnEdge`) with the same
  ball, and between the two blends the rounding is that ball: pieces of its
  sphere whose side on the other face is the shared contact (a degenerated
  edge, as 0171) and whose other side runs where the sphere cuts the faces
  beyond E1 and E2, across their common edge (`ChFi3d_ConcaveTurnWedge`).
  E1 and E2 end at the blends' contacts. Where the edges' contacts lie more
  than a right angle round the ball from the other contact (the sections
  rising over the ball's top), the sphere cuts neither face between the
  sections and the stripe fails.
- `0391-brepblend-rstrst-rolling-ball-on-two-edges.patch`: where both
  faces of the rounded edge are narrower than the ball's contacts (a ridge
  whose roof faces end at sharp eaves), the ball rests on the far edges of
  both. The overflow start refused it and the fillet failed or became
  another rounding. `StartSolOverflow` now lets both sides roll where their
  contacts lie beyond sharp convex edges, on closed contours only (the
  corners where such a blend meets the faces at an open contour's ends are
  not made: an open contour fails as before), and the blend between two edges
  (`BRepBlend_RstRstConstRad`, `BRepBlend_CurvPointRadInv` at an edge's end)
  is the ball tangent to both (`BallOnTwoCurves`: its centre in both
  edges' normal planes at the radius from both; the guide only picks the
  ball, as 0075), its sections the ball's arcs; on a closed guide the walk's
  period starts where it ended to the left, as for the other walks.

The patches 0400 and 0401 (mitcad#330) fix two exceptions inside a
rounding's computation; `geometry/tests/test_fillet_corners.cpp` checks
each with OCCT alone, and each check fails without its patch.

- `0400-chfi3d-rounding-vanishing-at-its-end-stops-at-the-vertex.patch`:
  where a rounding's two faces come tangent at the end vertex of its spine
  (a tangency extremity), its walk's last piece may end a little short of
  the guide's end, where both contacts reach the edges that end at the
  vertex within the vertex's tolerance (on stored bodies whose edges lie
  within their tolerance of it). The face beyond them continued the
  contour, and the stripe went on with a piece from the vertex whose
  sections, both contacts on that face, had no width; its approximation
  threw (`BlendFunc::GetMinimalWeights` on a zero angle,
  "Geom_TrimmedCurve::U1 == U2"). Now such a piece ends the stripe
  (`ContactsAtTangentEnd`), and the end is the singular corner, a point at
  the vertex, as where the walk reaches it exactly.
- `0401-chfi3d-end-cut-across-the-seam-line-of-a-periodic-face.patch`:
  `PerformOneCorner` split the cut of a rounding's end by a periodic face
  at the end only where the extrema of the cut's and the seam's 3d curves
  met within `Precision::Confusion()`. Where the seam starts at the
  corner's vertex the two curves meet only within the cut's approximation
  (2.4 · 10⁻⁶ on a gable's ridge ending on a cylinder's seam), or the cut
  passes just beyond the seam's start within the vertex's tolerance (a
  stored body): the cut was stored unsplit, running across the seam, the
  seam was lost and the face came out unorientable (or the body without
  its floor). Now the cut is also split where its curve on the face
  crosses the seam's line (`ChFi3d_CutCrossesSeamLine`): at the seam's end
  vertex where it passes within that vertex's tolerance (the seam keeps
  its vertex), else at the seam's point there as before; the pieces'
  curves on the face are the cut's own, each in the period of its arc.

The patch 0410 goes on with the corners where a rounding ends (mitcad#236,
sixth round); `geometry/tests/test_fillet_corners.cpp` checks it with OCCT
alone.

- `0410-chfi3d-rounding-ending-in-the-face-it-runs-into.patch`: a vault's
  end edge rounded with the radius of the block's rounded vertical corners
  under it. The ball overflows the corners onto the end face, and where the
  vault springs tangentially from the side walls the blend's end section is
  the corner's own quarter circle: the blend runs on into the corner
  tangentially. OCCT found no face at the end holding both end points (one
  is the vertex, the other lies on the end face's edge with the corner, on
  no arc), went on to its corner of several faces and failed there
  ("Failed to find edge"). Now `ChFi3d_EndSectionOnFace` (in
  `PerformOneCorner`, not OnSame, where no face at the end is found
  otherwise) takes the face at the vertex that holds the whole end section
  for the face at the end, gives a point on no arc the edge of that face it
  lies on (with the transition of a contact crossing it), and keeps a point
  at the vertex on no arc, so that the faces beyond keep their edges up to
  it; the cap is the section itself, not the intersection of two surfaces
  tangent along it, and the spine's edge is cut at the vertex as at a
  singular end. A walk that reached the spine's end vertex with one contact
  at a section before the guide's end failed on the next step from there
  and invalidated the stripe; the blend now ends there
  (`ChFi3d_ContactAtSpineEnd` in `PerformSetOfSurfOnElSpine`).

The patch 0420 (mitcad#337) rounds a closed edge on a cylinder whose seam
runs in steps; `geometry/tests/test_fillet_kernel.cpp` checks it with OCCT
alone.

- `0420-chfi3d-kpart-round-a-closed-edge-on-a-face-with-a-stepped-seam.patch`:
  two coaxial cylinders of one radius joined with their seams at different
  angles (a boss extruded on in a second step) merge into one face whose
  seam runs up the first seam, along the circle between the parts and up
  the second one. The analytic rounding (a torus) of the boss's end circle
  has the circle's parameters, one period round it; where the end part
  extends before the first seam in the face's parameters (the second seam
  at a smaller angle), the split of the blend into the face's domain
  (`SplitKPart`) found that domain a period off, the piece covered half
  the spine and the walk for the rest failed. The tangency line on the
  face is now moved by those whole periods of the surface, and its
  domains with it (`DomainsIntoPeriod`). Left: with a radius smaller than
  the end part's height (the contact above the step), OCCT's rebuild of
  the cylinder keeps one of the step's two uses, and the result does not
  check valid (it failed before; the dressup rejects it as before).

The patches 0430 to 0432 come from the fillets left failing at a vertex
after mitcad#323 (mitcad#339, `core/import/docs/f3d-rounds-2.md`, *Fillet corners
left after mitcad#323*): roundings beside faces narrower than their radius
whose overflow (0011) ran across another rounding.

- `0430-chfi3d-rounding-rolls-before-another-roundings-face.patch`: the
  two vertical edges at the end of a window through a wall 3.54 thick
  rounded with 2, the end split into a strip 0.5 wide and the rest, which
  meets the strip at 0.35 degrees. The rounding beside the strip ran over
  it onto the other face (0011; at 0.35 degrees OCCT also takes the two
  faces for tangent), up to 2 from its edge, across the other rounding's
  contact there, and the corners at the window's top and bottom failed
  ("fillets have too big radiuses"). The stored designs' rounding rolls on
  the strip's far edge. `ChFi3d_Builder::RollsInsteadOfOverflow`, once the
  stripes are computed: a side on a face reached across a narrow face of its
  edge, whose contact there lies nearer another stripe's edge than that
  stripe's own contact, may not overflow onto that face; the stripe is
  computed again (as before where that fails), its start found as an
  overflow's, and that side rolls on the edge before the face (also where
  the two faces only nearly continue each other: their normals more than
  1e-4 apart, and before a sliver it passed).
- `0431-chfi3d-rolled-onsame-end-at-the-end-of-its-contact-curve.patch`:
  the OnSame end of such a blend rolling on an edge up to where the edge
  ends on the face at the end: the intersection of its contact curve with
  that face came out 6e-15 past the curve's bounds, was not taken, and the
  corner failed ("bouchon non ecrit"). It is now taken within the curve's
  parametric resolution, at the bound.
- `0432-chfi3d-roundings-crease-over-a-narrow-common-face.patch`: a box's
  rim with an inner lip whose bottom edge, lines and corner arcs meeting at
  53 degrees, is rounded with 4 over a ring 1 wide under the lip and a
  chamfer beyond it. Each rounding ran over the ring onto the chamfer, the
  lines' onto its planes and the arcs' onto its cones, and at each corner
  the two had no face in common ("Trying to check orientation of NULL
  face", "Failed to find edge"). The stored designs' roundings roll on the
  ring's far edges and meet in a crease. `RollsInsteadOfOverflow` also lets
  two stripes meeting at a vertex roll before the different faces beyond
  their consumed common face; `ChFi3d_IsInFront` no longer takes two sides
  on no face for a common face; `ChFi3d_FilBuilder::PerformTwoRolledCorner`
  makes the crease (the blends' intersection from their point on the pivot
  edge to the rolled edges' common vertex, the stripes ending on it as in
  the bevel of `PerformTwoCornerbyInter`), and a walked blend rolling on a
  circle about the axis of the cylinder or plane it touches becomes the
  torus it is (`ChFi3d_ExactRollOnCircle`, a B-spline patch of it in the
  walked surface's layout), which reaches the crease past the walk's end.

The patch 0440 goes on with 0148 and 0351 for chamfers and roundings that
consume a strip beside their edges (mitcad#334): a boss on a slab 2 wider,
the boss's foot chamfered by 2 along three sides and its corner arcs, the
slab's top going on along the fourth side.

- `0440-chfi3d-contact-along-a-chain-of-edges-of-a-consumed-strip.patch`:
  `ChFi3d_ContactsAlongFaceEdges` took a contact along a whole edge of its
  face or along a part of one. A contact along several edges end to end
  (the strip's far side split at a vertex) was along none of them, and the
  strip stayed as a face of no width; such a contact's SurfData is now split
  at the chain's inner vertices (`ChFi3d_SplitAtVertexOnLine`, as 0144 does
  at a closed edge's vertex), each part along one edge. And a contact along
  a part of an edge (starting inside it, where the chamfered chain ends)
  kept its own point at its other end, the edge's vertex, where the next
  contact began at the vertex: the slab's walls took two vertices there and
  their wires were not closed. That end is now the vertex, as for a contact
  along a whole edge. Before, the results were invalid; fillets likewise.

The patch 0450 comes from the fillets left failing at a vertex after
mitcad#339 (mitcad#341): a rounding ending where one of its faces turns
tangentially into the face at the end.

- `0450-chfi3d-rounding-face-turning-tangentially-into-the-end-face.patch`:
  a rounding ends at the spine's end vertex, where one of its faces (an
  earlier rounding) runs on tangentially into the face at the end and the
  other meets that face at a sharp edge. At the end of the walk both
  contacts reach an edge of their faces; the sharp one ends the walk, the
  tangent one did not, and `ComputeData` walked that side on along the
  face at the end while the other lay on a tangent plane past its edge:
  the pieces at the end were not as `PerformMoreSurfdata` expects, or the
  corner was invalid. Where one side's edge goes on tangentially into a
  face that holds the spine's end vertex and that the other side's sharp
  edge bounds too, that face is now the face at the end on both sides
  (`TangentIntoEndFace`): the walk ends there and the corner cuts the blend
  with it, the cap running from one contact's crossing of the tangent edge
  to the other's crossing of the sharp edge. The tangent side often
  reaches its edge past the spine's end, where 0077 cut the walk at twice
  the contacts' speed at the spine's end, short of that edge: a line that
  ends on an edge is kept up to it while its contacts run less than four
  times as fast (`ChFi3d_RegularUpToArc`), and the ends above are taken
  only where that holds; past it the sections get singular, and such ends
  fail at the corner as before.

The patch 0460 goes on with 0410 for the corners where a rounding ends
(mitcad#342): a rounding that ends on the end edge of a rounding of its own
radius beside it.

- `0460-chfi3d-onsame-end-on-an-edge-of-a-consumed-corner-face.patch`: a
  boss 1.5 high on a wall, its long top edges rounded with 2 (rolling on the
  foot edges), the feet of those roundings with 2 and the ends' edges with
  the roundings with 1.5; then the ends' feet are rounded with 2. At each
  corner the ball fits the foot rounding beside it, so the end section is
  that rounding's end arc, from the corner's vertex to the arc's other end,
  where the opposite contact ends; the corner torus between them (0.5 wide)
  lies wholly between the contacts. OCCT took the torus for the face at the
  end of its OnSame corner and intersected the blend with it, tangent to it
  all along the arc ("echec calcul intersection").
  `ChFi3d_EndSectionAlongEdge` recognises such an end (the points at the
  vertices of the edge to prolong, the section on its curve, the face at
  the end bounded otherwise by the spine and the opposite face): the face
  beyond that edge becomes the face at the end, and the end is built as
  0410's end section lying in its face, the cap the edge's own curve,
  taking the edge's place there; the consumed face's edges off the spine are cut
  at their vertices, so that the builder drops it (as
  `ChFi3d_CutConsumedFaces` drops consumed strips). No prolongation of the
  opposite contact is made: it ends at the edge's vertex.

The patches 0480 and 0481 go on with 0140 to 0149 and 0380 for chamfers
(mitcad#263): a post's foot on a plate chamfered so wide that the cone's
contact on the plate runs past the plate's outline, the outline's faces
growing up to the cone (0142), where the gaps hold the post's seam.

- `0480-geomint-open-line-parts-beyond-its-vertices.patch` (`TKGeomAlgo`):
  `GeomInt_LineConstructor` kept of an analytic open line (a line, parabola
  or hyperbola of two surfaces) with two or more vertices only the parts
  between them, dropping those before the first and after the last although
  they lie in both domains. A cone meets a plane parallel to its axis in a
  hyperbola whose asymptotes are the generatrices parallel to the plane;
  with the cone's seam one of them within rounding (8.6e-9 rad off a wall
  square to it), the hyperbola crosses the seam twice some 1e8 mm away and
  only a piece of no length between those crossings was left, so the
  chamfer found no cut of its cone by the wall and failed. Those outer parts
  are now tested and kept as the inner ones are, and parts that meet are
  joined (a line wholly in both domains is one part, as with no vertices).
- `0481-chfi3d-kpart-gap-across-a-closed-spines-start-split-there.patch`:
  the piece cut by the face beyond over the gap that holds the start of a
  closed spine ran across that start, its other contact crossing inside it
  the seam of the face closed round the spine (the post's cylinder, whose
  edge's vertex is on the seam). The topological build took the contact's
  edges on the cylinder for a wire of their own and kept the cylinder
  whole: an invalid shape. `ChFi3d_FillKPartGaps` now splits such a piece
  at the spine's start where a line crosses its face's seam there
  (`ChFi3d_SplitPieceAtSection`: both parts on one blend surface, each line
  cut at the plane square to the spine there, the points on the seam on
  each part's side of it), the part before the start at the period's end
  and the part after it at its start, as `SplitKPart` cuts its own pieces.
  A piece whose lines cross no seam there (a rounding's torus over a gap)
  stays whole, its face's seam where it was: split, a later rounding along
  that torus's edges failed in a corpus design.

The patch 0490 goes on with the corners where a rounding ends (mitcad#236,
seventh round): the face at an OnSame end on a trimmed surface.

- `0490-chfi3d-onsame-end-on-a-trimmed-face-past-its-bounds.patch`: a
  plate's bottom edge rounded with 3 up to a conical foot below the plate
  (the foot's rim on the bottom, the cone cut flush with the plate's side).
  The cone is the face at the end of the OnSame corner, which intersects
  the opposite contact (on the side, 3 above the bottom) with that face
  extended and caps the blend there. The cone continued above its rim
  crosses that contact before the edge's end, but stored bodies carry the
  cone on a surface trimmed at the face's bounds, which
  `ChFi3d_ExtendSurface` (B-splines and Beziers only) left bounded: no
  intersection, no cap ("OneCorner : bouchon non ecrit"). Where that
  intersection fails, the face at the end is now taken as its basis surface
  continues (`ChFi3d_UntrimmedEndSurface`: an elementary basis, unbounded
  where not periodic, one period from the trimmed start where periodic, the
  face's own parameters); the cap and the prolonged edge are built there,
  and the face keeps its surface (a new surface given to the face lost its
  seam's second p-curve in the topological build). Ends whose bounded face
  is crossed are unchanged.

The patches 0500 to 0502 go on with 0170 to 0172 and 0390 to 0391
(mitcad#237): a rounding between a cylinder and a cone about one axis whose
contact passes below a ledge over flats cut into the cylinder, which in the
files is not a ball rolling on the ledge's edge but the torus of the
cylinder and the cone, cut round each flat; and a thin rim's outer edge
rounded wider than the rim, which in the files is the outer wall's rounding
cut by the rim's round end caps; `geometry/tests/test_fillet_rolling_ball.cpp`
checks all three with OCCT alone, and each check fails without its patch.
0503 (mitcad#378) mends the corners where 0500's torus meets the next
edge's analytic rounding; its check there fails without it.

- `0500-chfikpart-fillet-between-coaxial-surfaces-of-revolution.patch`: the
  constant-radius fillet between a cylinder and a cone, or two cones, about
  one axis, along a circle about it, is an analytic blend, the torus its
  ball sweeps (`ChFiKPart_MakeFilletCoaxial`, `ChFi3d_KParticular`). OCCT
  had analytic blends only with a plane among the faces; this one was walked
  as a B-spline, and where its contact passed over a cut-out of its face
  the walk rolled on the cut-out's edge and failed. In the meridian
  half-plane the ball's centre lies at the radius from both profiles; the
  section runs from the contact on the first face to the one on the second
  with the torus's v rising, as the other analytic fillets have it. The
  convex and concave roundings of a post's conical top or flare and of two
  cones, whole and part revolutions, agree with the walked ones within the
  walk's approximation (2 · 10⁻⁵ of volume).
- `0501-chfi3d-kpart-cut-round-a-cut-out-under-an-overhanging-rim.patch`:
  an analytic blend whose contact passes below a ledge over a flat cut into
  its face (a post's rim overhanging ten flats, rounded with 0.9 where it
  meets the flare above: the contact 0.073 below the ledge) is cut round
  the flat in turn: up the flat's plane, along the ledge (grown out to the
  cut) and down the flat again (`ChFi3d_KPartCutChain` round from the flat
  back to it, crossings on the prolongations of the edge between the flat
  and the ledge beyond both its ends, 0142). Those crossings lie just beyond
  the gap (0.01 along the spine): the pieces are kept within the gap, the
  ones between its ends and the crossings of no length along the spine,
  their cut on the flat their only side; the cut's arc of the ledge's
  circle is the one round the piece, and a gap across a closed spine's
  start is split there, its closing section the torus's seam. The same
  across the spine's edges (the circle in two edges, its vertices at a
  flat's corner and mid-flat: `ChFi3d_FillKPartRoundGapsAcrossEdges`, the
  pieces at the edges' ends given the arcs SplitKPart leaves them without),
  and the torus is one face round the whole circle: the edges' tori are
  parametrized alike and the pieces that are one face (0073) join across
  the edges (`ChFi3d_JoinKPartRunsAcrossEdges`). Before, the stripe failed
  ("the ball's piece at a concave turn not made", 0390's rolling on the
  ledge's edge); now the result is the post rounded without the flats, the
  flats cut from it after, as the bodies stored in the files are (Mitcad's
  dressup built that with the flats cleared and cut again, mitcad#311).
- `0502-chfi3d-kpart-cut-by-the-end-face-over-a-gap.patch`: an analytic
  blend near an open spine's end whose contact runs off its face N over a
  gap and comes back into N only where the face at the end already cuts
  the blend (a thin rim between an outer wall and a pocket, its ends round
  caps, the outer top edge rounded with 1.2, wider than the rim: the
  contact on the top lies over the pocket and comes back into the top
  inside the cap) is cut by the end face from where the other contact
  reaches the end face's edge to where the blend, the gap's wall and the
  end face meet, and by the gap's wall from there to the first piece whose
  contact comes back (`ChFi3d_KPartCutToEndOverGap`, where 0059's cut at the
  end does not apply); the end face grows up to its cut (its edge with N
  prolonged beyond the spine's end vertex), a new edge bounds it and the
  gap's wall, and the pieces are one face with the rest. Before, the
  stripe failed; now the result is the outer wall rounded alone, the ring
  it loses taken off the rim outside the caps, as the files' bodies are.
- `0503-chfi3d-two-roundings-meeting-on-an-edge-within-its-tolerance.patch`
  (mitcad#378, a regression of 0500): two roundings meeting at a vertex of
  four sharp edges whose ends lie on the same two points are joined along
  their intersection (`PerformTwoCornerSameExt`); the many-edge corner
  took that path only where the ends agreed within 1e-7, and now also
  where both ends are common points on one edge within that edge's
  tolerance (`SameEndPoint`). A slot through a round boss, its rim
  chamfered and the chamfer's lower edges rounded with 1: at each corner
  the wall's rounding (a cylinder) meets 0500's torus round the boss, and
  both end on the crease between the chamfer's plane and cone. On a stored
  body that crease's curves on the two faces differ within its tolerance
  (1e-4) and the ends lay 1.6e-7 apart: the corner was a plate of no width
  whose edges reached tolerances of 0.4, and Mitcad's check found a
  self-intersecting wire once the vertex tolerances were cut down. Walked
  roundings end further apart there, so 0500 exposed it; now the corner is
  the two roundings' intersection, as in the files' bodies.

The patches 0510 to 0512 let booleans cut stored bodies whose faces and
edges run many turns (mitcad#349): a stored body can keep a threaded hole's
minor and major cylinders as one face each between helices running tens of
turns (the face's u range spans as many periods), its flanks as one ruled
B-spline face each, and the helices as one B-spline edge each.

- `0510-boptools-pcurve-into-the-period-inside-a-face-spanning-many.patch`:
  `BOPTools_AlgoTools2D::AdjustPCurveOnSurf` moves a boolean's new curve on
  such a face into the first period of the range and, when the classifier
  finds its middle outside the face, by one period up or down; at a given
  height only one copy of an angle lies inside the face, often many periods
  away. A box cut across the thread got its section curves on the cylinders
  outside the face; the result was invalid and its volume wrong. When the
  curve's middle is outside the face, every copy by whole periods within
  the range is now classified and the curve moved into the first one inside
  the face; the one-period shift remains only when none is.
- `0511-inttools-fclass2d-every-copy-on-a-face-spanning-many-periods.patch`:
  `IntTools_FClass2d::Perform` and `TestOnRestriction` try a point's copies
  upwards from where `GeomInt::AdjustPeriodic` put it, which leaves a point
  already in the face's range where it is; the copies below it were never
  tried. A section curve's point in the surface's first period lay in the
  range of the 32-turn face, the copy inside the face 16 periods below: it
  was classified outside and the parts of section curves on the face were
  dropped (the box cut removed nothing). The copies now start at the first
  one at or above the range's start, as `BRepTopAdaptor_FClass2d` does.
- `0512-intcurvesurface-quadric-roots-sampled-by-knot-spans.patch`
  (`TKGeomAlgo`): the exact intersection of a curve with a plane, cylinder,
  cone or sphere samples its distance to the quadric
  `IntCurveSurface_TheHCurveTool::NbSamples` times, at most 50 for the
  whole curve: every two thirds of a turn on a helix edge of 32 turns, and
  where the helix dips below a cylinder across the hole's end and rises
  again within one step neither crossing was found (`IntTools_EdgeFace`
  found one of three). The section curves on the hole's faces lost their
  ends, and the tool's face over the hole's opening was left whole with
  them inside as internal edges: the cut kept it as an internal face. A
  B-spline is now sampled at least its degree times for each knot span in
  the interval (at most 100000); other curves keep their count.
- `0560-boptools-pcurve-checked-at-its-own-copy-in-the-face.patch`:
  `AdjustPCurveOnSurf` called `GeomInt::AdjustPeriodic` with the curve's
  middle u as both its input and its output, so the classifier of a face
  spanning more than one period (0510's) checked the point moved twice, and
  the shift was corrected from there. After a cross hole splits a 32-turn
  cylinder face into pieces on the one surface, the piece below it lies 16
  periods below the surface's first period: a box cut's section curves on it
  were checked 32 periods down and placed a period above their first one,
  outside the piece, and the cut removed nothing and was invalid. u now
  keeps the curve's own value, so the point checked is where the curve
  goes; faces of at most one period, and curves the shift into the range
  does not move, are not affected (mitcad#380).

The patch 0590 comes from threads whose turns meet in one edge, as the
bodies stored in files have them (mitcad#235):

- `0590-shapeupgrade-unifysamedomain-wire-repeating-itself-given-up.patch`:
  the face merge builds a merged face's wires edge by edge and, where no
  edge left goes on from the current vertex, takes a removed seam there
  (`ReconstructMissedSeam`). The seven pieces of a cylinder cut by the
  helix and the lines along the axis where a thread's turns meet in one
  edge (the threaded part intersected with the thread's crest cylinder
  past a blind bore's bottom) sent the wire back and forth over the same
  removed edges without end, until the memory was gone. A wire runs along
  each edge left at most once and each removed edge at most twice; past
  that count it is given up as when no next edge is found, and the faces
  stay unmerged (mitcad#235).

The patches 0591 to 0593 bound the work of booleans on faces whose
B-spline surfaces run many turns (mitcad#235): a thread built as the
stored bodies keep it has one face per profile edge over all its turns,
its helices interpolated at about 140 points a turn (4000 knot spans for
25 turns), and is split into pieces of a third of a turn on the one
surface for the booleans. The cut of such a thread took 19 s (87 s of
work) and its union with the part 13 s and 7.5 GB; with the three patches
3 s and 3 s, 0.8 GB.

- `0591-adaptor3d-bspline-samples-within-the-adaptors-range.patch`
  (`TKG3d`): `Adaptor3d_HSurfaceTool::NbSamplesU` and `NbSamplesV`, the
  polyhedron of the edge-face intersections, count a B-spline's knot spans
  within the adaptor's range (a face's) instead of all of the surface's;
  for a face of the whole surface the counts are as before.
- `0592-shapeupgrade-unifysamedomain-planarity-once-per-surface.patch`:
  the face merge tests each surface for planarity once
  (`GeomLib_IsPlanarSurface`, which samples the whole surface), not once
  for each pair of neighbouring faces; the pieces of a thread's faces met
  their tangent neighbours with that test each time. Results are the
  test's own.
- `0593-extrema-genextps-grid-of-very-many-knot-spans-sampled-evenly.patch`
  (`TKGeomBase`): the grid of a point's projection onto a B-spline holds
  at most 1001 parameters in a direction (as many as
  `Adaptor3d_TopolTool::BSplSamplePnts` samples at most), evenly spaced,
  where the degree's number of parameters in each knot span would be more;
  every projection computes the distances to all of the grid's nodes,
  edges and cells, and each face's grid was kept in every thread's
  context.

The patches 0530 and 0531 build chamfers wider than a face of their
edges along part of the chain (mitcad#263):

- `0530-chfi3d-chamfer-guide-keeps-to-its-edges-at-the-blends-beside.patch`:
  a walked chamfer's guide takes the tangents of the analytic blends before
  and after it (the cones of arcs, meeting the stretch between at a slight
  angle) within the chamfer's smallest distance of each end only
  (`ChFi3d_AdjustGuideEndsLocally`); before, the adjustment deformed the
  whole guide, up to 0.11 mm off a stretch of 16.6 mm, and the chamfer's
  contacts moved with it. Other blends are unchanged.
- `0531-chfi3d-chamfer-wider-than-a-narrow-face-cut-by-the-faces-beyond.patch`:
  a chamfer whose contact on a face B runs off B across one of B's edges
  into a face beyond at an angle (a fin's tip chamfered wider than the
  strip of its side left by the fin's foot chamfer) is computed again with
  B's surface unbounded (`ChamferOverNarrowFace`) and cut by the faces
  beyond B (`ChamferCutByFacesBeyond`): the material it takes away is kept
  outside those faces' material, their surfaces extended, so the chamfer
  is cut by such a face from where its contact leaves B back across it to
  its other contact, nothing of it is left where its section lies wholly
  inside, and there the faces beyond grow down to the chamfer's other face
  (new curves of them with it and with each other, prolonging their common
  edge; B's and the spine's edges under it bound no face). The parts left
  are stripes of their own ending in points. A chamfer whose contour is
  stopped where its edges turn (`StopChainsAtTurns`, now offered by
  `BRepFilletAPI_MakeChamfer` too) ends there with the end edge's analytic
  blend run on past the vertex, cut by the face beyond the crease, up to
  where its other contact reaches that face (`ChamferEndPastTurnStop`).
  Before, the walk would roll on B's edge, which chamfers do not ("PerformSurf
  Not Implemented"). Only chamfers that failed so, or whose walk left a face
  mid-spine, take the new paths.

The patch 0550 comes from the fillet corners left after mitcad#341
(mitcad#343): a rounding whose end lies over two pieces, where the edge of
the face at the end is in two pieces.

- `0550-chfi3d-end-over-pieces-where-the-end-faces-edge-is-in-two.patch`:
  a bar joined to a cylinder, its long top and bottom edges chamfered and
  the chamfers' edges with its side rounded; the side's concave edge with
  the cylinder is rounded with 3, on tangentially over the bar's roundings
  and chamfers up to the caps. Near each cap the cylinder's contact
  reaches the cap first; the walk goes on until the other contact crosses
  from the bar's rounding onto the chamfer, and its extension over the
  chamfer (the cylinder's side given up) reaches the chamfer's edge with
  the cap: the end lies over two pieces, both cut by the cap
  (`PerformMoreSurfdata`). That end is taken where the piece before the
  last ends on an edge of the face at the end at the vertex; the cap's edge
  with the cylinder is in two pieces here (the profile's vertex where the
  bar's side met the cylinder stays on it after the chamfer and the
  rounding took the side's top away, with only those two pieces at it),
  and the point lies on the piece away from the vertex. The end was not
  taken, `PerformOneCorner` dropped the extension and failed in
  `PerformIntersectionAtEnd`, whose walk round the bar's rounding, a face
  without the vertex, did not come back ("the max number of faces
  reached"). An edge now counts as the arc at the vertex where it goes on
  from it between the same two faces through vertices no other edge meets
  (`ChFi3d_ArcGoesOnAs`, in `MoreSurfdata` and `PerformMoreSurfdata`). The
  result is the one built where the cap's edge is one piece, as the bodies
  stored in the files are (0.3 µm from them in a stored design).

The patches 0580 and 0581 come from the same round (mitcad#343): a pocket's
ceiling rounded with its walls (radius 8), the walls' edges broken by ribs
1 thick standing out 6 from them, the roundings ending at blocks whose side
lies where they touch the ceiling. The bodies stored in the files have one
cylinder along each wall, cut by the ribs and ended by the blocks.

- `0580-chfi3d-kpart-contact-along-a-concave-edge-off-its-face.patch`: past
  a rounding's end the ceiling's contact line runs along the block's
  concave edge with the ceiling. `SplitKPart` hatches the line on its face,
  and the hatcher took that stretch as inside the face or outside it as the
  line lay a rounding error (1e-14) to one side of the edge or the other: on
  one wall the line's domain ended at the block and the corner cut the
  blend with the face at the end, on the mirrored wall it ran on to the
  ceiling's rim, 21.6 past the end, and the corner failed or built faces of
  reversed orientation. No ball of the blend touches a face along a concave
  edge of it (the face beyond rises into the ball), so a straight contact
  line along such an edge is hatched moved off the face across it by the
  shift `HatchOnBoundary` uses, its ends near a vertex snapped to the
  vertex (`HatchOffConcaveEdges`).
- `0581-chfi3d-roundings-joined-across-a-rib-with-square-sides.patch`: 0241's
  `ChFi3d_JoinAcrossNotches` joins the roundings on either side of a rib,
  one stripe's blend running on over the gap cut by the rib's faces. The
  rib's sides cut the blend along sections and give no piece, so the piece
  on the rib's end starts where the blend crosses that face's far edge, not
  at the other stripe's end point, and the stripes met at that point: the
  piece's edge on the rib's end started there, the rib's side lost its edge
  with the blend and the shell was open. The piece is now split at its
  middle, its half at the other stripe's end goes to that stripe, and the
  stripes meet at the split, a whole section on one face; the rest of the
  end section is the side's edge with the blend, which `ChFi3d_FilDS` makes
  between two pieces of one stripe (`PartSection`). Only where the blends
  are one surface with the same sides and the stripes run the same way at
  the gap. And where a stripe joined before is joined again (three edges,
  two ribs), every surface data on the old surface takes the new one, not
  only the extended stripe's: the stripe joined first was left a face of
  its own with no edge at the section between them.

The patch 0600 comes from the next round (mitcad#343): a convex edge
between a band N and a cone Fo rounded with 1, ending at a vertex V where a
notch wall B (a plane at 8 degrees to the edge) starts. Past V, N goes on as
a narrower tongue between the notch walls and Fo goes on beside B (B's edge
with N convex, with Fo concave: an OnSame end). The bodies stored in the
files run the blend on past V along Fo until its contact on Fo reaches B's
edge with Fo (5.9 past V), and B cuts it along its side from there back to
where its contact on N crosses B's plane (4 before V); B grows up to the
cut.

- `0600-chfi3d-onsame-end-cut-along-its-side-by-the-face-at-the-end.patch`:
  OCCT walked on past V until the contact on N left the tongue across its
  edge with a face tangent to it (4 past V, the contact on Fo still short
  of B), and the corner took that edge's face for the face at the end: an
  invalid result (faces of reversed orientation) or two roundings that
  cross. `ChFi3d_Builder::WalkedOnSameToEndFace` now finds such an end
  after the stripe is walked (OnSame, a vertex of three edges, the last
  SurfData past V with its contact on N on an edge that is not B's and its
  contact on Fo inside Fo) and walks that SurfData again from its other
  end, N's surface unbounded and the guide run on past its end, up to where
  the contact on Fo reaches B's edge with Fo; that SurfData replaces the
  last one, and `PerformOneCorner`'s OnSame corner cuts it by B and
  prolongs B's edge with N up to the contact on N, as at an ordinary OnSame
  end. The stripe is left as it was where that walk does not reach the
  edge; analytic blends (not walked) are not changed.

The patch 0620 comes from the round after (mitcad#343): a concave edge
between a revolve's torus and a narrow B-spline face rounded with 0.1 along
its tangent chain, on into the torus's edge with a planar sliver tangent
to the B-spline face. Near the chain's middle the B-spline face is
narrower than the rounding: its contact leaves the face across its convex
edge with another torus, and the ball rolls on that edge and on the
sliver's edge that continues it.

- `0620-chfi3d-contact-off-its-face-before-the-end-rolls-on-the-edge.patch`:
  the chain's one guide reaches both ends of the spine, so the walk's end
  handling (`ComputeData`) took the edge the contact met for the end
  region (it is a few edges from the end vertex, `IsObst`) and went on
  over the B-spline's surface beyond the edge, unclassified, until the
  other contact met the torus's edge with the sliver; the stripe stopped
  there, short of the spine's end, and the corner at the end failed.
  `ChFi3d_ObstacleBeforeEnd` now takes a contact on a convex edge of its
  face more than 10 `tolesp` before the spine's end, whose face beyond
  does not hold the end vertex, for an obstacle: the walk ends there, and
  `PerformSetOfSurfOnElSpine` goes on with a piece rolling on the edge, as
  where a walk meets an edge in the middle of a rounding. Edges of faces
  at the end vertex, concave or tangent edges and edges at the spine's end
  are handled as before.

The patches 0650 and 0651 come from the `.ipt` import's replay gaps
(mitcad#235): a flange's rim chamfered past the strip of its top up to the
roots of the teeth standing on it (a pulley's, a pinion's), and a thread's
crests rounded with 0.15 along all their turns.

- `0650-chfi3d-kpart-cut-by-a-long-chain-of-faces-beyond.patch`: the
  chamfer's cone runs under every tooth, whose seven faces (root and tip
  roundings, flanks, tip) cut it and grow down to it (0055, 0071, 0142).
  The chain of faces beyond from the one the contact leaves its face into
  to the one it comes back from may be nine faces long
  (`ChFi3d_KPartCutChain`, four before). Where no arc of the intersection
  between a cut's ends lies on the face beyond (it grows to the cut), the
  arc whose coordinate along the spine spans the least is the cut: the
  other arc of a root rounding's cylinder with the cone runs round the
  cylinder's far side (`ChFi3d_KPartCutCurve`, the first arc before); and
  ends on two lines of the intersection that meet end to end, or through a
  third (it splits where a rounding tangent to the root circle touches the
  cone), give the arc joined along them. For chamfers, a gap whose ends are
  vertices of the contact's face (the contact runs along the root arcs and
  leaves the face where they end) starts and ends on the face's edges into
  the faces beyond there, which bound no face any more
  (`ChFi3d_FillKPartGaps`; before, the chamfer failed, "Chaining is
  impossible"). On a closed spine the first and the last SurfData are
  neighbours across the spine's start, so a contact along an edge that the
  start splits runs on along it (`ChFi3d_ContactsAlongFaceEdges`; before,
  the edge was cut there, an open wire), and a piece of a spine of several
  edges whose line runs across its period's end into the edge is kept
  (`SplitKPart`; before, it was taken for a piece past the edge's end).
- `0651-chfi3d-walk-from-a-seam-and-extension-ending-where-it-fails.patch`:
  each crest's strip of a turn is a face closed round its cylinder, its
  seam where the thread starts. The walk past the spine's start starts
  where the walk before stopped, the crest's contact on that seam at the end
  of the face's parameters, and runs out across it: its first step left the
  face and was halved until it failed ("Path failed"). It now starts a
  period on, on the seam's other side, where the solution a little way on
  lies beyond the face's bounds (`BRepBlend_Walking::Perform`). The
  extension past the spine's ends (half the spine's length) walked on over
  the thread's run-out, where the rounding vanishes and the guide nearly
  stops while the sections go on: the surface over them had weights below
  zero or strayed far from them. A step wholly past the spine's end whose
  surface fails so (it throws, or strays from its sections by more than
  100 times the approximation's tolerance) fails as a step whose walk
  fails (`CallPerformSurf`), and ends the extension where it started
  (`PerformSetOfSurfOnElSpine`; before, the stripe failed). Steps within the spine fail the stripe as before.

The patch 0670 goes on with the `.ipt` import's replay gaps (mitcad#235): a
tab's top outline rounded with the radius of its rounded corners, the top a
steep cylinder across the tab.

- `0670-chfi3d-standing-ball-sphere-in-a-gap-between-pieces.patch`: round
  each corner the ball's centre stays on the corner's axis, its contact on
  the top at one point, and the rounding is a piece of the ball's sphere.
  The walls' roundings are analytic and end where the ball reaches the
  corner; the walk between them (its sections' mid-points in the guide's
  normal planes, 0075) swept round the corner before the guide reached it,
  as the guide climbs the steep top and its normal planes lean, and then
  found no ball ("Path failed"). A gap between two pieces of the stripe
  whose piece before ends, or whose piece after starts, with its contact on
  an edge into a face on a cylinder of the ball's radius about an axis
  through the ball's centre, the other contact inside its face
  (`ChFi3d_StandingBall`), now gets the piece of that ball's sphere from
  that section to the one where the contact's sweep round the corner
  reaches the corner's other edge (`ChFi3d_StandingBallPiece`, its side on
  the top a degenerated edge of the blend's face); the walk takes the rest
  of the gap, if any, from where its sections put that ball
  (`ChFi3d_StandingBallSection`). Also the gap across the period's end of a
  closed spine.
- `0671-chfi3d-kpart-chamfer-cut-across-its-width-by-faces-beyond-at-both-ends.patch`:
  a thread's groove in a bore ends in a flat face; the edge where that
  face meets the bore is chamfered further than the groove is deep. The
  contact on the flat face leaves it across the tip's edge, the contact on
  the bore leaves it where the next turn begins, both before the edge's
  ends; SplitKPart left one short piece and the corners at both ends
  failed ("OneCorner : bouchon non ecrit"). At each end of an open spine
  of a chamfer that takes material off, `ChFi3d_KPartCutChainToEnd` now
  cuts the chamfer from the end piece's section on by a chain of faces
  beyond (the tip's rounding, then a flank), their edges prolonged past
  their vertices on the flat face along lines, circles or their faces'
  isolines (`ChFi3d_KPartEdgeCrossing`, `ChFi3d_ProlongEdgeAlongIso`), up
  to the face whose surface meets the other contact at Q, where the
  chamfer ends in a point. Q lies inside an edge of the other contact's
  face (that edge's part from Q to the end vertex is replaced by a curve
  bounding its face beyond and the grown face) or on its prolongation
  past the end vertex (the edge is prolonged to Q). The edges under the
  chamfer at the end vertex bound no face any more. `ChFi3d_KPartCutPiece`,
  `ChFi3d_MakeExtremities` and `ChFi3d_FilDS` take an end in a point at a
  vertex of its own; such a stripe is not computed again as a walked
  chamfer over a narrow face. Mitcad's chamfer joins the contacts' pieces
  at the vertices they leave between them (`piece_vertices` in
  `geometry/src/dressup.cpp`): the files keep one edge there, and OCCT's
  roundings of the crests beside them failed at those vertices.

The patch 0680 comes from a spiral stair's join of its slabs, treads and
helical stringers (mitcad#400):

- `0680-shapeupgrade-unifysamedomain-faces-round-a-period-without-winding.patch`:
  the face merge takes faces on a surface periodic in u, none of them with
  a seam, whose u reach round the whole period to wind round the surface,
  and builds their union's wires with seams reconstructed from removed
  edges (`ReconstructMissedSeam`). The rim of the stair's slabs, split by
  the sketch's lines into 80 pieces each, is cut through from its bottom
  to its top by the treads' inner sides rising on it step by step, or by a
  helical stringer's band along it: what is left reaches from one side of
  the cut round to the other, the whole period, without winding round the
  cylinder. Built with seams, the merged face used an edge with two
  pcurves on its surface once (`BRepCheck_InvalidMultiConnexity`) and left
  pieces of the rim under it; the result was invalid, the union was
  computed again without the merge (twice the time), and kept a thousand
  pieces of the rim, which every later boolean met. `IntUnifyFaces` now
  first follows the contours of the merged face's edges, moving each
  pcurve by whole periods onto the end of the one before it
  (`RelocateContoursNotWinding`); when each contour closes in 2d and each
  hole lies inside the outer contour at one place, the face is built
  without a seam, its pcurves spanning more than the period, on a copy of
  the surface (as faces covering less than the period are built on a
  trimmed one). A contour that winds round the surface, a vertex with more
  than one way on, a hole that cannot be placed or a surface periodic in
  both directions take the seams as before.
The patches 0720 and 0721 come from the corners and rounding ends of
mitcad#343 that a design's history fails on its own stored bodies (the
patches 0700 to 0719 are that ticket's notched blends).

- `0720-chfi3d-face-at-the-end-passed-wholly-beyond-is-consumed.patch`: a
  housing's corner edges rounded with 20, each ending at a vertex of a
  plane square to it. A cylinder standing out of that plane leaves of it a
  small piece at the vertex (two sides on the rounded edge's faces, the
  third on the cylinder) and the pieces beyond; the rounding's contacts
  pass the small piece and end on the pieces beyond, and its end is cut by
  them and by the cylinder between them. The walk round the end vertex
  (`PerformIntersectionAtEnd`) met a piece beyond, the cylinder, the small
  piece, the cylinder again and the other piece beyond: the cylinder was
  cut twice, both times up to its edge with the small piece (outside it),
  and the small piece was kept (the shell unorientable). Now a face
  between two visits of one face across the same edge, holding the end
  vertex, whose edge with that face the rounding does not cross (its
  surface about the end, also widened along the spine), lies in the
  removed corner: it leaves the faces at the end with the second visit,
  and its edges are cut at their vertices so that the builder drops it.
- `0721-chfi3d-onsame-prolonged-edge-own-curve-on-both-surfaces.patch`: an
  external thread's flank rounded with 0.1 along its edge with the chamfer
  cone at the thread's start, where the flank comes out of the root
  cylinder: an OnSame end, the cone's edge with the cylinder (a circle of
  both, coaxial) prolonged past the vertex up to the rounding's contact on
  the cone, across the cone's seam. OCCT intersected the cone and the
  cylinder for it: the cone is nearly flat there, the intersection's
  p-curve ran the long way round the cone and its 3d curve strayed 0.1 off
  the circle; the seam was not cut and the cone's face lost its seam and
  its edge with the cylinder. `ChFi3d_ProlongTangentEdge` (the edge's own
  curve continued, so far for faces tangent along it) now also takes faces
  not tangent there whose surfaces both hold the curve continued (within 10
  `tolapp3d` at three points), tried before the surfaces are intersected;
  and its p-curves are parametrized as the 3d curve also when it runs from
  the edge's last vertex on a periodic curve (the trimmed circle's
  parameters come into its first period, the p-curves kept the edge's: the
  pieces split at the seam had p-curves a period off on the face beside).
  Split at the opposite face's seam, such a prolongation's pieces take the
  crossing in the curve's own range and their own p-curves, each moved by
  whole periods into the face's domain (projected anew, the piece from the
  vertex lay a period off: a rib's foot on a rod, rounded across the rod's
  seam, `test_rib_foot_rounded_across_the_rods_seam`). Where the
  prolongation is still the surfaces' intersection (the edge's curve an
  approximation ending at the vertex, as on the replay's modelled thread),
  its ends on the periodic opposite face are taken in one period
  (`ChFi3d_Recale`): a period apart, the intersection ran the long way
  round.

The patches 0740 to 0742 go on with the `.ipt` import's replay gaps
(mitcad#415, after mitcad#235's 0690 to 0692).

- `0740-chfi3d-end-face-split-round-the-vertex-pieces-of-one-surface.patch`:
  a bar's top edge rounded where the bar meets a bore's wall, the wall
  round the corner's vertex three faces of one cylinder (over the bar's
  top, beside the bar below it and beside it above), each on its own
  cylinder object, as booleans that keep faces of different origins apart
  leave it. `PerformIntersectionAtEnd` walks round the vertex through the
  three pieces and, the pieces being tangent along both edges between them,
  puts the cut on one of them (`oneintersection`): the part of the
  contact's arc on another piece from the contact to the vertex becomes the
  edge between the two. Its curve on the piece with the cut, and the arc's
  point there (`Hc1`, `Hc2`), were asked of the arc, which has curves only
  on its own faces' surfaces: null where the pieces do not share the
  surface object, the edge without a curve on that face and the point taken
  at (0, 0), the face invalid or the cut along a wrong curve.
  `ChFi3d_ArcOnSameSurface` now computes such a curve (the edge's curve
  projected on the face's surface, in the face's period at the vertex,
  checked to lie on it) where the arc has none.
- `0741-chfi3d-kpart-contact-end-at-a-vertex-on-the-edge-to-the-spines-end.patch`:
  the recess's rim of 0691 and 0692 rounded up to the rib, the edges
  between the recess's cone and the rib's roundings with curves that stop
  short of their vertices with the slopes within those vertices' tolerance
  (as an imported body's may). The rim's contact on the cone leaves it
  exactly at such a vertex; the hatching found the end there only on the
  cone's edge with the slope, `FillSD` put it on that edge, and the corner
  (OnSame) took the slope for the face at the end instead of the rib's
  rounding: the cut ran across the bottom and the result was invalid. An
  end found at a vertex of an arc that does not hold the spine's end
  vertex is now on the face's arc that holds both, at the vertex, as when
  the hatching finds both arcs there.
- `0742-chfi3d-guide-past-an-open-end-along-the-end-edges-own-curve.patch`:
  a thread's root (the helical edge between its flanks) rounded up to the
  faces that cut it at its ends, as the `.ipt` import's rounding of an
  internal thread's root running out past a blind bore's bottom (an item
  of `d9aec91e`). The walk starts and ends some way past the spine's open
  ends along the guide (`ChFi3d_PerformElSpine`), which
  `GeomLib::ExtendCurveToPoint` extended there smoothly towards a point
  far along the end tangent; on a helix of small lead that leaves the root
  within millimetres, the walk's contacts swerved, and the faces at the
  ends were cut along curves joined by vertices with tolerances of up to
  tens of millimetres (14 mm in that part, where a later cut then merged
  a circle's vertex 12 mm away into one and a rounding of that circle
  failed). The guide now goes on along the end edge's own curve where that
  is a B-spline going on past the edge, else along the iso-line of its
  face's B-spline surface the edge runs on, where either goes on far
  enough (`ChFi3d_OwnCurveExtension`, `ChFi3d_IsoCurveOfEdge`; the guide's
  own part keeps its parameters); else the smooth extension continues from
  as far as one goes.

The patch 0790 goes on with the `.ipt` import's replay gaps (mitcad#422,
after mitcad#415).

- `0790-chfi3d-tangent-end-face-cuts-the-blend-back-along-the-stripe.patch`:
  a thread's crest (between the post's cylinder and the groove's lower
  flank) rounded down to the thread's run-out, a straight cut whose lower
  flank is the plane tangent to the groove's flank along the start
  profile. The crest runs on there into the run-out's crest, which is not
  rounded: an OnSame end whose face at the end goes on tangentially from
  the flank. Its plane only touches the rounding near the vertex; OCCT's
  corner cut the rounding there by that touch, running it on down to the
  run-out. The bodies the `.ipt` files store end such a rounding where the
  plane cuts it across a turn back (331 degrees up, where the crest rises
  out of the plane again), a face of 0.006 mm² on the plane closing it and
  the crest staying sharp from there down. `PerformCapAtTangentEndFace`
  (before the other ends of a lone stripe at a vertex of three edges):
  where the end is OnSame and the face at the end goes on tangentially
  (G1, across an edge at the vertex) from a face of the spine's edge, each
  contact is followed back from the end to where it first passes from that
  face's solid's side of its surface to the outside; where both do so on
  one surfdata more than four radii from the vertex, the surfdatas past the
  cut are dropped, the last one ends on the curve where the surface cuts it,
  and a corner stripe on that surface closes the end up to the point where
  the surface crosses the spine's edge, which is cut there; the spine's end
  becomes a free one. A cut whose two contacts fall on two surfdatas (the
  stripe split there, as at a face's seam) is not made: the corner at the
  vertex is tried as before. Elsewhere nothing changes (the end of the same
  thread's other crest at the run-out, whose flank's plane never cuts its
  rounding across, keeps its corner at the vertex, as the files' bodies
  have a corner patch there).

The patch 0690 goes on with the `.ipt` import's replay gaps (mitcad#235): a
bar's top edge rounded where the bar meets a bore's wall at the wall's
seam.

- `0690-chfi3d-end-on-a-face-along-the-line-of-its-seam.patch`: the
  rounding ends on the wall, whose seam runs up from the corner's vertex
  along the line of the bar's side edge on the wall (the edge continues
  the seam). The wall the rounding uncovers lies on both sides of that
  line from the rounding's contact on the bar's side up to the vertex.
  PerformOneCorner took the cut of the rounding's end a period away from
  the wall's domain and nothing stood for the seam below the vertex: the
  wall's face was unorientable and the solid's volume wrong (where the
  side edge kept both curves of the seam it was split off, the OCC32
  pseudo-seam code made its piece a seam with its curves the wrong way
  round). Where one point of a cut stored whole lies on such an arc
  (`ChFi3d_ArcAlongSeamLine`), the cut's curve on the face is now put in
  the period of the other point's arc, and the arc's part from its point
  to the vertex is stored on the face twice, with the arc's curve and
  translated by the period: a piece of the seam.

The patches 0691 and 0692 go on with the `.ipt` import's replay gaps
(mitcad#235): a recess's rim on a part's bottom rounded up to a rib across
the recess whose bottom is the part's bottom, the rib's bottom edges
rounded up to 45-degree slopes.

- `0691-chfi3d-onsame-end-on-a-tangent-face-the-crossing-nearest-the-corner.patch`:
  each half of the rim's rounding ends on the rib's rounding, which
  continues the bottom (the opposite face) tangentially along the
  prolonged edge (OnSame). The contact on the bottom, a circle round the
  recess's axis, only touches the rounding's surface, at both ends of the
  half, and `IntersUpdateOnSame` took the far end's touch, a diameter
  away. Where the face at the end is tangent to the opposite face along
  the prolonged edge (either way round), the contact's crossing with that
  edge is now computed too and the point nearer the corner's vertex taken.
- `0692-chfi3d-kpart-contact-end-within-a-vertex-tolerance-at-the-vertex.patch`:
  the rim's contact on the recess's cone leaves the cone exactly through
  the vertex where the rib's rounding meets its slope. In a part's own
  body the edge's curve ends a little past that vertex, within its
  tolerance, and the hatching found the contact's end inside the edge:
  a sliver of the edge was left, and the cone's wire intersected itself.
  `FillSD` now puts an end found inside an arc within the tolerance of one
  of its vertices at that vertex (`SnapToVertex`).

The patch 0700 comes from the eighth round of mitcad#343: a big torus's
edges with a plane rounded with 0.5, the chain broken by two narrow strips
of a boss (a cylinder standing on the plane) between the torus and earlier
roundings of 0.6 (tori tangent to the plane), onto which the plane's
contact runs. The bodies stored in the file have one rounding face over
each strip: the ball keeps touching the torus and the earlier rounding,
the strip's edge with the torus lies under it, and the boss cuts the
rounding, of the strip only a lens being left.

- `0700-chfi3d-blends-cut-by-a-narrow-face-they-roll-over.patch` (a new
  file, `ChFi3d_Builder_Trim.cxx`, and two calls in `Compute`):
  - `JoinOverNarrowFaces`: two stripe ends of one radius on the same two
    faces Fo and N, the contact on N of one of them ending on an edge of a
    narrow face C that holds both ends' vertices, and no other stripe at
    those vertices but stripes whose edges all lie between Fo and C (under
    the ball, dropped). The first end's blend is walked again from a
    section some radii before its end (or from its end piece's start), N's
    surface unbounded, along a guide through the ball's centres of the two
    ends' pieces (past the spine's end A's own guide runs straight on and
    its sections slide along the ball's path, `ChFi3d_CutSingularExtension`
    then cut the walk back): its contact leaves N across an edge of C and
    comes back across one. The cut of the walked blend by C between those
    points (`GeomInt_IntSS`, its lines joined end to end) may reach back
    along the blend past both points (item 45: 0.59 and 0.62 past points
    0.48 apart, the cut 3.80 long): the walked blend is three pieces on one
    surface, one face for `ChFi3d_FilDS` (as 0581's joined pieces), the
    middle one's side on C the cut (as `ChFi3d_KPartCutPiece`'s, whose
    pieces hold one span a section and could not hold it). The other end's
    piece is cut back to the section of the same ball beyond the cut, and
    the two ends meet there as free ends with a common section edge (as
    `ChFi3d_JoinAcrossNotches`); A's end piece is cut back to the walk's
    start. Before, each end got a corner at its vertex of C, which failed
    ("Failed to find edge"); analytic ends (a cylinder between two planes)
    are walked the same way. It runs before the stripes' contacts on a
    common face are taken for one (`myRideStripes`): an analytic end's
    contact runs on to the end of its face for the corner.
  - Stripes under another's ball: a stripe whose edges all lie between Fo
    and a narrow face C, along which another stripe's contact on Fo runs
    within one of its pieces (on Fo and a face other than C), on the side
    of that contact its blend covers, is dropped; not where the ball
    touches C (a wall beyond a rib the rounding is joined over goes on from
    the blend's other face: its own short stripe stays).
  - `PierceBlends`, once a stripe is in the data structure: where an edge
    of Fo with a face C lies under the blend, C's surface cuts a piece's
    surface in a closed curve inside it, and every edge of C lies under the
    blend (its points beyond the blend's surface from its ball's centre),
    the curve is a hole of the blend's face and the whole edge of C's lens
    (the part of C outside the blend); C's edges are cut at their vertices
    so that the builder drops them. Before, the blend ran through C.

The patch 0750 comes from the ninth round of mitcad#343: an external
thread's crest (a cylinder band 0.8 wide, three turns) rounded with 0.8 on
both its edges. The two roundings' contacts on the crest cross, and
OverlappingStripes (0013) computes each again overflowing the crest, the
ball touching both flanks; the bodies stored in the file have that one
blend, the crest consumed. At the thread's ends a plane square to the axis
and a chamfer cone at 60 degrees meet the helix at about a degree and cut
the blend along curves 83 long, one flank's contact running on 17 past the
end of its edge, the other's meeting the face 65 before.

- `0750-chfi3d-thread-crest-rounded-on-both-edges-cut-by-the-ends.patch`:
  - The guide of the walks past a spine's end follows the end edge's own
    curve or its face's iso-line as far as these go on (0742), then turns
    towards a far point along its end tangent (`ChFi3d_PerformElSpine`); the
    crest's helices and the flanks' iso-lines stop about where their faces
    do. Past the end of a helix the sections turn away from the ball's
    path (more than a radian 60 past a helix of radius 28) and
    `ChFi3d_CutSingularExtension` cut the walk back before the contacts
    reached the face at the end. A guide that a screw motion maps onto
    itself, within a ten-thousandth of its length (the edges'
    approximation), is now continued by that motion where 0742's extension
    does not reach far enough, by a turn at most
    (`ChFi3d_ScrewContinuedCurve`: a copy of its end moved by the motion,
    joined with a knot of multiplicity the degree, the guide itself
    unchanged).
  - The walks' face surfaces (`ConexFaces`, `StartSolOverflow`): a B-spline
    surface that a screw motion sweeps along one of its parameters (a
    thread's flank; a surface of revolution) is evaluated as continued by
    that motion past both ends of that parameter, by a turn or its own
    length at most (`ChFi3d_ScrewContinued`, `ChFi3d_LoadScrewContinued`:
    its own poles and knots over its range, moved copies of its ends
    beyond, so that the contacts' p-curves are the face's). The flank
    whose contact runs on past the spine's end is stored ending about
    where its face ends; past its bounds the walk ran on the end spans'
    polynomials and lost the ball.
  - A contact that leaves its face onto a face that holds the vertex at
    that end of the spine, the two faces' normals along the edge nowhere
    within the builder's angular tolerance but within the tenth of a
    radian `SearchFace` takes for a face continued (a flank running into
    the chamfer cone at 0.8 degree), now ends on that face, which cuts the
    blend (`KinkIntoEndFace` in `ComputeData`). Before, the next walk
    rolled the ball on the cone.
  - Of two stripes computed again over the face between their edges
    (`myOverlapStripes`), one that is a single piece on the two faces
    beyond that face, both its contacts ending on edges at both its ends,
    takes the other's place, which is dropped with its corners
    (`DropOverlapsRunOutByOther`). The two blends are one, but each stripe
    may run out to the faces at the ends at one end only, or its start
    section lie elsewhere, so that `RemoveCoincidentStripes` kept both and
    their corners failed.
  - A start section of a stripe computed again over such a face is taken
    only where the ball touches the face across the other stripe's edge
    and its own edge's other face (`StartSolOverflow`; near the face's end
    a section ran on to the thread's root and the end plane), and the test
    whether the two contacts overlap (`contactBeyond`) samples the contact
    at its interference's parameters and projects on the other within its
    range (on a contact line of several turns a projection landed a turn
    away, its ends' points off the line where the contact left its face).
  - Walks of more than three turns of a thread still fail where their
    surface's approximation fails ("Failed approximation"), not split.

The patches 0770 and 0771 come from the ninth round of mitcad#343: a block
on a post as wide as the block (the post's top circle touches the block's
long bottom edges at two points, where the block's underside narrows to
nothing and the post's side is tangent to the block's long sides), the
post's foot rounded with the post's radius, then the block's bottom outline
rounded over that rounding.

- `0770-chfi3d-rolling-on-an-edge-into-the-end-vertex-where-it-vanishes.patch`:
  the foot's rounding: its contact on the underside runs off across the
  long edges, and from there the ball rolls on those edges, touching the
  post, up to the two points, where the edges run into the post's top
  tangent to its side and the rounding vanishes. OCCT took those edges,
  which end at the rounding's end vertex, for no obstacle (an arc at the
  end vertex was the face at the end's: `StartSol`), ended each side where
  the contact left the underside and extended it by tangent planes; the
  corners at the points filled the rest wrongly. An arc into the end vertex
  tangent to the blend's other face (`ChFi3d_ArcTangentToFaceAt`, within
  0.01 rad) is now an obstacle, rolled on up to the vertex. Several
  roundings that all vanish at one vertex each end in a point there
  (`ChFi3d_ShrinkVanishingEnd`, so far for a rounding ending alone there:
  the faces' normals taken at the contacts' ends, as a surface may
  degenerate at the vertex, an end of coinciding contacts short of the
  vertex moved too, the end parameters matched within a thousand times the
  parametric confusion, and the contact curves cut by full knots where they
  are twice the end's distance from the vertex before their end poles are
  moved, so that only that last piece bends), and the corner is the
  singular one.
- `0771-chfi3d-contour-stops-where-the-rounding-vanishes.patch`: the bottom
  outline: along each long side its edge runs over the foot's rounding,
  whose face comes tangent to the side at the point (the foot's rounding
  ends in a point there, its surface degenerate at that corner). OCCT
  carried the contour on through the point (the edges continue each other
  there); its walk crawled into the point, or its last section, the
  singular one given at the point, took parameters on the degenerate
  face's iso far from the contact's, and the approximation between failed
  or wandered (p-curve tolerances of tens of millimetres). The faces'
  normals at a vertex where a surface has none are now their limits along
  the edge (`TangentExtremity`), the contour stops at a vertex where the
  faces of both edges come tangent (`PerformElement`), a walk into such a
  vertex also ends where its section has no width within a hundredth of
  the radius (`ContactsAtTangentEnd`), and the line's end there is solved
  again from its last section at least 5e-4 of the radius wide, each
  section from the one before (its step on the faces no longer than the
  walk's before), until narrower within a hundredth of the radius of the
  vertex (`ChFi3d_EndBeforeVanishing`); the corner moves that end to the
  vertex (0770).
- `0772-chfi3d-stripes-joined-across-a-corner-of-the-solid.patch`: from the
  same round, a housing whose top is a plane with a bulge (a tilted
  cylinder) on its rounded vertical corner, the bulge's foot on the top
  rounded. The foot is two edges, each from inside the top to the corner's
  rounding, reached just past its tangent lines; the ball of each rounding
  rolls on between the top's plane and the bulge past that end, its contact
  on the plane leaving the top across the first side's edge and coming
  back across the second's. OCCT ended each stripe at its edge's end, and
  the corners there filled the gap wrongly. `JoinAcrossCorners` (after
  `JoinOverNarrowFaces`) now pairs such free ends (one constant radius, at
  most 8 radii apart, the last pieces on the same face and on faces of one
  plane, their vertices on a third face, the corner) and walks one blend
  from the middle of the first stripe's last piece along a guide through
  both last pieces' ball centres, the plane unbounded. The edges the
  contact crosses give the sides; the edges between the sides and the
  corner's face, run on, meet the blend at the junctions; the blend is cut
  by the three faces between those points (their surfaces' intersection,
  interpolated with its p-curves) and split into five pieces up to a
  section inside the second stripe's last piece, where the two stripes
  share an edge. Where the contact crosses the sides' edges, each edge
  keeps its part beyond the blend (the point's transition set by the side
  its vertex on the corner's face lies on, as the walk may run either way
  along the spine). The edges between the sides run on to new vertices at
  the junctions, and the corner face's edges now under the blend are
  dropped.

The patches 0800 to 0803 come from the tenth round of mitcad#343: the
traits of the bodies stored in files that kept OCCT's fillet from the
roundings round a thread's chamfer cone and up to a groove's floor.

- `0800-chfi3d-onsame-cap-points-in-one-period-of-a-stored-cylinder.patch`:
  a thread's flank rounded round its chamfer cone from where the flank
  comes out of the root cylinder: an OnSame end whose face at the end is
  that cylinder. Stored bodies hold such a cylinder trimmed to one period
  starting turns below the basis surface's parameters (u from -11.82 to
  -5.54), the edges' curves on it in that range; the cap's point on the
  arc came from the arc's curve (u = -11.824), the opposite contact's
  point from its projection on the surface (u = 0.743), two periods apart.
  `ChFi3d_Recale` moved one of them by one period, so the box for the
  blend's intersection with the cylinder spanned a period beside the cap,
  which was not found ("echec calcul intersection"). It now brings the
  point within half a period of the other however many periods apart (for
  points under one and a half periods apart nothing changes).
- `0801-chfi3d-tangent-faces-within-ten-times-the-edge-tolerance.patch`:
  `ChFi3d::IsTangentFaces` compared the points of the edge's curves on the
  two faces within max(0.001, 1.5 times the edge's tolerance); each curve
  may lie within the tolerance of the edge's own curve, so the two up to
  twice that apart, and the approximated helices of stored threads have
  curves on their faces up to 4.8 micrometres off their own (tolerances of
  1.4 to 3.7 micrometres). A crest band and the roundings beside it,
  tangent within 1e-3 radian, were taken for faces meeting at an angle: a
  rounding round the chamfer cone stopped its contour at those edges'
  vertices (`FaceTangency`), three roundings meeting end to end that
  walked past each other, and a walk across such an edge stopped there
  (`IsG1` in `SearchFace`). The positions are compared within ten times
  the edge's tolerance now (at least 0.001; the normals within a tenth of
  a radian as before).
- `0802-chfi3d-contact-on-its-own-edge-near-a-vertex-passes-it.patch`: that
  rounding, one contour now, runs along the cone's edges with an earlier
  rounding of the crest and then with the flank, which goes on
  tangentially from it across an edge ending at the vertex V between the
  contour's two edges. Cone and rounding meet at 1.7 degrees at V, so the
  blend is 0.015 wide there, its contacts 7.5 micrometres from the edge;
  the stored edges lie up to 4 micrometres off their faces. The contact on
  the rounding met the edge with the cone itself 0.034 before V and the
  walk ended there (`SearchFace` found the cone beyond). A contact point on
  the spine's edge between the contact's face and the blend's other face,
  within ten times its tolerance or its vertex's of the vertex ahead where
  the spine goes on into another of its edges, a face tangent to the
  contact's face holding that edge, is taken at that vertex with the
  contact's way as its vector (`ChFi3d_OwnEdgeNearVertex` in
  `ComputeData`): the walk goes on through the vertex into that face
  (`SearchFaceOnV`).
- `0803-chfi3d-onsame-cap-leaving-the-prolongation-tangentially.patch`: a
  thread cut as a groove, its crest's edges rounded up to the groove's
  floor. Each rounding ends where its contact on the crest's cylinder meets
  the floor, the floor's edge with the cylinder prolonged up to there; the
  floor cuts the rounding along a cap that leaves that circle tangentially
  and runs inside it a few micrometres off for a while. The cap's
  approximation crossed the circle 0.053 from their common point (its
  tolerance 5e-5, the circle's 1e-7): the floor's wire was
  self-intersecting. Where the two meet tangentially at that point (within
  a hundredth of a radian), their crossings within a hundredth of the
  cap's length are added to the box the point's tolerance is made from
  (`ChFi3d_TangentCapCrossings`).
The patch 0840 ends a fillet contour's walk that went round a closed chain
for ever (mitcad#434, found in the `.f3d` import after the ninth round of
mitcad#343).

- `0840-chfi3d-contour-never-takes-its-own-edge-again.patch`: a closed
  chain of edges whose edges turn at a vertex W, the faces going on
  tangentially across it, rounded from an edge selected for the turns
  (`StopChainsAtTurns`, 0180) while the edge beyond W is not.
  `PerformElement`'s downstream walk stops at W. Its upstream walk from the
  contour's start goes round the chain the other way and comes to W from
  the other side, where the edge beyond is the downstream walk's last edge,
  selected; `StopsAtTurn` lets it pass, and the walk went on over the
  contour's edges, prepending them to the spine for ever. The downstream
  walk ends where it comes back to its start vertex; the upstream walk had
  no such check, and `BRepFilletAPI_MakeFillet::Add` has no progress range,
  so a request to stop did not reach it either (the import's definition of
  `db73863f` item 88 that named some of a closed chain's edges ran for
  hours). Now neither walk adds an edge already in the contour: it stops
  there as at an edge it does not take, and round such a chain the contour
  is every edge of the chain once, both its ends at W.

The patches 0820 to 0822 come from the tenth round of mitcad#343: the
housing's corner of 0772 again, and a key on a shaft (a cylinder standing
out of the shaft's side), each end of the key a lens between the key's
convex arc and the shaft's concave arc.

- `0820-chfi3d-stripes-joined-across-a-corner-one-face-each.patch`: 0772
  walked the joined blend from the middle of the first stripe's last piece
  to a section inside the second's, so that each corner had three faces
  (the first piece's rest, the walked blend, the second piece's rest), and
  the walked face was generated by no edge. The stored bodies have two,
  one of each edge, the first's holding the cut by the first side and the
  second's the cuts by the corner's face and by the second side; a later
  rounding of the sides' edges names its edges by those faces. Now the
  walk runs from the first stripe's last piece's far end (the other face's
  surface taken within its own period about the face, else widened by its
  size and ten radii, as the contacts at the pieces' far ends lie past
  their faces) to the second's far end; the first stripe takes the walk's
  pieces up to the section through the first side's junction, the second
  the rest, on a copy of the walked surface, its lines and p-curves in its
  own spine parameter and its sides in its own order; the two stripes end
  on that section. The walk starts on the stripe whose edge runs out of
  the corner where the other's runs into it, whichever order the edges are
  given in. The faces generated by the edges are the walked surface and
  its copy. Where a corner at a stripe's far end extends the last piece's
  surface (`PerformIntersectionAtEnd`), the stripe's other pieces on that
  surface take the extended surface too, so that they stay one face.
- `0821-chfi3d-lens-rounded-on-both-edges-s-shaped-section.patch`: both
  edges of the key's lens rounded with a radius wider than the lens. The
  stored designs' concave rounding is a ball touching the shaft and
  rolling on the lens's convex edge; their convex rounding is a ball
  touching the key and that concave ball from outside; the concave face
  lies between its contact on the shaft and the convex blend's contact on
  it, and towards the lens's ends, where the convex blend's contact comes
  down to the concave one's on the shaft, the shaft cuts the convex blend
  up to where its contact on the key reaches the key's sides. OCCT rolled
  each rounding on the other's edge; the concave one found no start. A
  failed stripe of one edge and a constant radius whose face on one side
  has two edges, it concave, the other convex and the edge of another such
  stripe, is now taken out with that stripe while the others are done
  (`DeferLensRoundings`, only where nothing else failed) and joined with it
  after `JoinAcrossCorners` (`FinishLensRoundings`, `JoinLensPair`): the
  concave blend walked rolling on the convex edge from the start the
  stripe's overflow start finds; the balls at the ends Q, touching the
  shaft at its contact and the key from inside; the pipe of the concave
  radius round the concave balls' centres (the concave blend's surface,
  run on past its contacts) up to where those centres turn more sharply
  than the radius; the convex blend walked between the key's surface and
  the pipe, first along the convex edge (where its balls fold back before
  Q) and then along a guide through those balls' centres and on to the
  balls at Q; its cuts by the shaft from where its contact leaves the key
  (E) to Q (`ChFi3d_CutBetween`, 0772's cut shared); the convex stripe in
  three pieces (on the cut, on the key, on the cut), its other side the
  edge the two blends share, the concave stripe in three pieces on the
  pipe, both ending in Q, the lens's edges dropped. The two stripes'
  blends are not intersected with each other (`IsLensPair`). Where the
  join fails, the concave stripe fails as before.
- `0822-chfi3d-rounding-rolling-on-both-edges-into-a-lens-end.patch`: the
  key's other lens, only its convex arc rounded, wider than the lens. The
  ball touches the key and rolls on the lens's concave edge; where its
  contact on the key reaches the key's sides it rolls on those too, up to
  the lens's ends, where the two edges meet and the rounding vanishes in a
  point (the stored designs' faces there: the piece rolling on both edges,
  split where the key's side is split by a vertex, and a three-sided piece
  ending in that point). The walk's first step starts in the middle and
  ends at both sides; OCCT went on rolling on both edges at its last end,
  but then walked back from its first end with the faces and edges of the
  steps at the other end and failed; where a side's edge ended at a vertex
  short of the lens's end, the walk stopped there (the edge's other face
  holds the end vertex), and the corners failed. The walk back now goes on
  with the first step's faces, edges and obstacles (saved after it), its
  contacts at its start taken to be on their edges; a blend rolling on two
  edges, one of which ended at a vertex short of the spine's end vertex
  where another edge of its reference face, not tangent across, takes it
  on, goes on rolling (`RollsOnPastVertex`). The line's sections narrower
  than 5e-4 of the radius at an end where both contacts lie within a
  hundredth of the radius of the spine's end vertex are dropped
  (`ChFi3d_EndRstRstBeforeVanishing`; the approximation fails on sections
  of almost no width), and the corner moves that end to the vertex
  (`ChFi3d_ShrinkVanishingEnd`). Where the side's edge runs into the lens's
  end unsplit, the walk still ends where the contact reaches it and the
  corner cuts the blend by the shaft (within 0.016 mm of the rolling
  ball's faces).

The patches 0860 and 0861 make one rounding of a thread's chamfer cone
along its five edges down to the root, the last of them between faces 0.03
radian apart (mitcad#441, after the tenth round of mitcad#343).

- `0860-chfi3d-contour-runs-on-into-a-near-tangent-edge-asked-for.patch`:
  `ChFi3d::IsTangentFaces` takes faces up to 0.1 rad apart for tangent, and
  0022 rounds an edge between such faces where a contour starts from it
  (its faces farther apart than the angular tolerance, `MinNormalAngle`). A
  contour coming to the edge from the edges before it stopped there:
  `FaceTangency` takes the next edge only when its faces meet at an angle.
  The edge then started a contour of its own, whose walks took the edges
  before it again from the other side, two stripes over the same edges that
  met at no corner. Which happened depended on which edge comes first among
  the body's edges: on the stored body the near-tangent edge came first and
  the rounding was built; on the replay's body another edge did. Now
  `FaceTangency` takes such an edge as one at an angle where it is one of
  the edges asked for (`StopChainsAtTurns`) and its faces meet at more than
  the angular tolerance; without `StopChainsAtTurns` nothing changes.
- `0861-chfi3d-contact-at-the-vertex-where-the-blend-vanishes-cuts-the-own-edge.patch`:
  the one contour's blend comes to nothing at the vertex V where the
  contour passes from the cone's edge with an earlier rounding of the crest
  to its edge with the flank, the flank going on tangentially from the
  rounding across an edge from V. The cone and the rounding meet at 1.7
  degrees at V, the blend there thinner than V's tolerance, and the walk's
  contact on the rounding ended at V on the rounding's edge with the flank
  (`ChFi3d_FilCommonPoint` takes the vertex within its tolerance), not on
  the spine's edge. The data structure held no point of the blend on that
  spine edge, and the rounding's face kept the whole edge beside the
  blend's end, though the blend takes it away: the face's wire was open and
  BRepCheck rejected the result (0802 handles the same contact where it
  ends on the spine's own edge near V). `ChFi3d_OwnEdgesAtVertices`, after
  `ChFi3d_FilDS`, adds for such a contact (on face F, at a vertex V of the
  edge E between F and the blend's other face Fo, on an edge of F across
  which a face Fn goes on tangentially from F, Fn meeting Fo along an edge
  from V) a vertex interference on E at V, with the contact's transition
  carried over to E as E lies in F.

The patch 0880 starts a rounding along an edge of a strip the ball cannot
touch (an endcap's notch corners, round 11 of mitcad#343).

- `0880-chfi3d-start-across-a-strip-the-ball-cannot-touch.patch`: a notch
  cut into a ring's outer cylinder left at one corner a strip 5 µm wide of
  an earlier rounding of radius 0.4 between the notch and the cylinder,
  and the strip's edge with the notch is rounded with 1. The strip is
  convex and bends more sharply than the ball, so no ball inside the solid
  touches it, and the section on the edge's faces has no solution even on
  their surfaces. `StartSolOverflow` (0011, 0013) went on across a face the
  ball touches at no point only after a side had already moved, and gave
  up where the spine's own face is such a face: the rounding had no start
  ("no ball of the radius touches both faces"). Now, at the spine, the side
  whose face is convex there and bends towards the solid more sharply than
  the ball (`ballTouchesNoPointOf`) goes on across the face's edge the
  section crosses, within the rounding's size, as such a side does further
  on: the ball touches the notch and the cylinder, the strip consumed, as
  at the corner without the strip.

The patches 0890 and 0891 come from the eleventh round of mitcad#343:
roundings whose contacts run off a narrow face (the edge overflow of
mitcad#121), one ending at a sliver, one closed round a disc's rim over a
hole's rounded mouth.

- `0890-chfi3d-onsame-end-across-a-sliver-at-the-vertex.patch`: a wall's
  foot rounded with the radius of an earlier rounding round the foot of
  the wall's end, whose ball the new one's becomes where the contact on the
  wall leaves the wall across their common edge (0540's case at one end of
  the foot). At the other end a step's end face is left between the floor,
  the wall and the earlier rounding (a triangle 0.16 by 0.02, the wall
  narrowing to an edge 0.02 long), and the foot edge ends at that sliver,
  not at the earlier rounding. The vertex's three edges are all concave
  (AllSame); the corner found no face holding both contacts' ends and went
  round the vertex (`PerformIntersectionAtEnd`), and the result was
  invalid. `ChFi3d_SliverFaceAtEnd` recognises such an end: one contact
  left its face across an edge not at the vertex (any edge of that face at
  the contact's end where that end is a vertex) whose face beyond Fv
  closes the third face at the vertex, a face smaller than the radius
  whose other edges all lie on Fv, its edges with the blend's faces at the
  vertex running to a vertex of that edge and to a vertex of an edge of Fv
  and the opposite face. `PerformOneCorner` then takes the end as Fv's
  OnSame end: Fv is the face at the end, that edge of Fv and the opposite
  face is prolonged from its vertex at the sliver, the contact's point at a
  vertex is put on the edge left across, and the sliver is consumed (its
  edges cut at their vertices, as for a face consumed at an end along an
  edge).
- `0891-chfi3d-bridge-over-a-gap-round-a-closed-spine-and-across-its-start.patch`:
  a disc's bottom rim rounded with 1 over a narrow rounding of the rim, the
  contact on the bottom face running over the mouth of a blind hole,
  rounded with 0.5, which the ball cannot touch: 0160 walks the blend on
  over the gap and cuts it by the mouth's rounding, in two pieces where the
  cut crosses that rounding's seam. Three faults there. The piece over the
  gap was walked on round the rest of the closed spine, and
  `ChFi3d_KPartLineParam` took the section through the gap's start (its
  other contact line's own start) at the projection's foot on the far side
  of the disc, the line's ends being no feet: the seam's crossing was not
  found and the stripe failed. A bounded line's end nearer than any foot
  is taken now. With the gap across the spine's start, the intersection of
  the walked blend with the mouth's rounding lost the cut's last stretch at
  the surface's end, where the contact comes back (`ChFi3d_KPartCutCurve`
  intersects a walked blend extended by a quarter of its length past both
  ends along the spine), and the stretch past the start took the vertex
  where the cut crossed the seam, just before it, for a vertex of the
  mouth's rounding under the blend (`ChFi3d_KPartCutPiece` passes over a
  vertex whose nearest point on the contact's line is no foot).
The patch 0870 comes from the eleventh round of mitcad#343: a closed chain
of 36 edges round a part rounded with 5, the face on one side of three of
its edges a cylinder band about 2.47 wide, tangent to a torus of radius 10
beyond it, the contact on the band 2.59 from the edge.

- `0870-chfi3d-contact-off-a-narrow-face-across-its-tangent-edge-keeps-its-surface.patch`:
  the rounding's contact on the band N leaves N across its tangent edge
  with the torus C and comes back where N widens. The walk took C for N in
  between, the ball rolling on over C, which curves away from N's surface:
  up to 0.18 deeper, 7.1 mm3 more taken away at each of the three places.
  The stored designs keep the ball on N's surface run on (its centre 5 from
  N's cylinder all along), and C cuts the rounding along their
  intersection, a crease of up to 0.19 rad, the rest of N left as strips
  at the crossings. `KeepFacesAcrossTangentEdges` (in `Compute` after
  `JoinAcrossCorners`, stripes of constant radius) finds a stripe's run of
  pieces on N and Fo, on C (across N's tangent edge where the contact
  crossed it) and on N and Fo again, and walks it again from its first
  section to its last with N's surface unbounded, along a copy of the
  stripe's elementary spine, or the spine interpolated over the run where
  analytic pieces at its ends have none. Where the walk reaches both
  sections, its contact leaves N once and comes back, and the contact lies
  off C's surface by a tenth of the radius at most (deeper overflows are
  left as they were), the run becomes three pieces on the walked surface,
  one face: up to where the contact leaves N, the cut by C, and from where
  it comes back. The cut is found section by section (`ChFi3d_SectionCut`:
  C meets the blend tangentially at the crossings, where an intersection of
  the surfaces leaves gaps), its curves interpolated by chord length. The
  spine's edges generate the walked face (`Generated`). Only runs whose
  pieces on C roll on it (the blend tangent to C along their contact) are
  taken: a blend walked over a rounded hole's mouth and already cut by its
  rim at a crease (patch 0160) is the same construction, exact, and is left
  as it is (`geometry` test "rounding over a hole with a rounded rim").

The patches 0900 to 0906 come from the same round: the roundings of a
bar's end with radius 0.86, the rim chain round a band vanishing at a
vertex where three faces are tangent, and the walls of a notch 1 wide,
strips about 0.06 high between the notch's floor (a cone) and an earlier
rounding, whose balls touch the floor and roll on the walls' far edges.

- `0900-chfi3d-walked-onsame-piece-generated-by-the-spine-edges.patch`:
  the face of a piece walked again past an OnSame end
  (`WalkedOnSameToEndFace`, 0600) is generated by the spine's edges: the
  edges' map to their surfaces (`myEVIMap`) still named the old piece's
  surface, so `Generated()` left the new face out and a caller naming the
  result's faces after their edges found it unnamed.
- `0901-chfi3d-cap-intersection-ends-where-the-walk-first-passes-the-end.patch`:
  `ChFi3d_ComputeCurves` cuts the walked intersection of a blend with the
  face at its end where the walk first passes each end point (a local
  minimum of the distance no larger than the step), not at the points
  nearest to them: at an OnDiff end where the blend touches that face, the
  walk passed onto the other branch, wandered and came back, and the cap
  kept the wandering part (the face's wire self-intersecting). The same
  blend walked the other way round was valid.
- `0902-chfi3d-rounding-vanishing-where-its-contact-crosses-onto-a-tangent-face.patch`:
  a rounding vanishing at its spine's end vertex V where three faces are
  tangent: its contact crossed onto a face tangent to the other face along
  an edge into V a hundredth of the radius before V, and no ball fits
  between those two. `ChFi3d_ContactOntoTangentFaceAtEnd` recognises such
  an end; the walk ends the stripe there, `PerformFilletOnVertex` puts the
  end at V (the blend ending in a point), and `ChFi3d_ComputeArete` returns
  without a p-curve where its projection fails instead of using a null one.
- `0903-chfi3d-rolling-contact-crossing-onto-the-next-spine-face-goes-on.patch`:
  a blend rolling on an edge whose other contact crosses onto the face of
  the spine's next edge goes on along the spine
  (`ChFi3d_ArcOntoSpineFaces`); before, that face held the spine's end
  vertex and was taken for the face at the end, the walk stopped with most
  of the spine unwalked, and `ChFi3d_ExtendAtSpineEnds` extended the piece
  over the next one.
- `0904-chfi3d-rounding-rolled-on-past-one-end-along-a-line-or-circle.patch`:
  a blend rolling on an edge of a face narrower than its radius rolls on
  along that edge's line or circle past the edge's vertex on the face at
  the end, which cuts it (`ChFi3d_RolledToFaceAtEnd`,
  `RolledOnProlongedEdgeAtEnd`; 0342 took lines only and a piece without
  vertices at the spine's ends), at one end or both; where the narrow face
  ends in the spine's vertex, its far edge running into it, the blend ends
  in a point there (`ChFi3d_RolledIntoEndVertex`).
- `0905-chfi3d-ball-turning-about-a-vertex-of-the-edges-it-rolls-on.patch`:
  the piece where the ball turns about the common vertex of two edges it
  rolls on (one contact standing still, 0200) is walked back only over its
  turn (it was completed forward over the piece before it), extended only
  at the end its walk reached, and its side reduced to a point takes the
  transition of a neighbouring piece on the same faces (its face came out
  reversed on its wire).
- `0906-chfi3d-next-overflow-start-where-the-first-walk-fails.patch`:
  `StartSolOverflow` samples the spine from its start and takes the first
  section it finds. On a notch wall narrowing to a point at the spine's
  first vertex, that section (the ball touching the floor's side and the
  face beyond the wall's far edge, an earlier rounding, with its contact
  inside it) made the wall's side roll on that far edge; but the ball
  rolling on the edge and touching that face touches its surface outside
  the face, so the walk failed at its first section and the stripe with
  it. `PerformSetOfSurfOnElSpine` now takes the next start past it
  (`StartSolOverflow`'s `theAfter`) when the first walk from such a start
  finds no section there (`myStartSectionFailed`, set by `ComputeData`); a
  walk that fails further on keeps its failure (0821's lens is built after
  one). The same wall on the notch's other side, its spine running the
  other way, built before.
- `0907-brepblend-pivot-released-where-a-step-lands-on-the-tangency.patch`:
  0170's ball turning about the end of the edge it rolls on is released
  where its side of the next edge is within `Precision::Confusion()` of
  the release, and the release point is compared with the walk's last
  point and the step within `Precision::PConfusion()`. On the replayed body
  a step landed on the release point (side -5e-14): it was not taken, the
  step past it solved the release back at that point and the exact
  comparison rejected it, the steps shrank to nothing, the piece ended on
  no edge and the stripe failed.
- `0908-chfi3d-overflow-start-sampled-finer-where-none-is-found.patch`:
  where none of `StartSolOverflow`'s samples (eleven on a spine of one or
  two edges) finds a start, the spine is sampled again between them at a
  quarter of their spacing. A start of the far notch wall's rounding
  exists only along about a millimetre of its 17.7 long spine; on the
  replayed body no sample fell in it.
- `0909-chfi3d-rounding-vanishing-a-hair-short-of-the-vertex-it-rolls-into.patch`:
  a blend rolling on an edge that runs into the spine's end vertex
  (`ChFi3d_RollsIntoVertex`) ends the stripe where the walk stopped with
  its contacts within ten times the tolerance of each other and within a
  hundredth of the radius of that vertex (`ContactsAtTangentEnd`), and
  `ChFi3d_RolledIntoEndVertex` then takes that contact within a hundredth
  of the radius, so that the end is put at the vertex (0904). On the
  replayed body the contacts met 0.005 before the vertex and the next step,
  with no edge to roll on, failed.

The patch 0920 comes from mitcad#464, a fillet whose radius along its
edges is a B-spline of the position (Mitcad's `spline` fillet size).

- `0920-chfi3d-radius-law-of-the-callers-along-a-contour.patch`:
  `BRepFilletAPI_MakeFillet::SetRadius(Law_Function, IC, IinC)` left the
  contour with no radius: `ChFiDS_FilSpine::SetRadius` put the law in a
  composite it dropped. The other variable radii are interpolated through
  (parameter, radius) points with level ends (`Law_Interpol`), so a
  B-spline radius could only be approximated through them. The spine keeps
  the law now, by its parameter (the abscissa from its first vertex);
  `ComputeLaw` gives every guideline the law over its range, constant at
  the law's end values where the guideline runs past the law (an extension
  at an end), and refuses a closed contour. A spine with a law is constant
  on no edge, its largest radius is sampled from the law, `Reset` keeps it,
  and a radius set at a point or a reset of all data drops it.

The patch 0910 comes from mitcad#343 (round 11), a rounding of radius 0.5
along a thread's chamfer cone that ends where its contact on the cone runs
into the thread's root, an exact cylinder.

- `0910-chfi3d-onsame-cap-from-the-contacts-point-on-the-prolonged-edge.patch`:
  in an OnSame end whose faces are not tangent along the prolonged edge,
  `IntersUpdateOnSame` takes the opposite contact's crossing with that
  edge's curve on the face at the end (unbounded) where the point found by
  intersecting the contact's 3D curve with the face's surface lies within
  the tolerance of it, and `PerformOneCorner` makes the cap start there
  (`ChFi3d_CapFromPoint`: the end poles of the cap's curve and of its
  curves on the blend and on the face moved onto the point, by no more than
  ten times the tolerance). The contact's approximated curve lay 2e-6 off
  the cone and crossed the cylinder at a small angle, its point 1.3e-6
  below the cone's circle on the cylinder. The blend is tangent to the cone
  along its contact, so the cap leaves the circle tangentially, and as it
  is nearly tangent to the cylinder there it rises from it slowly: it
  crossed the circle 0.026 from the vertex, a self-intersecting wire for
  the checker (the circle's tolerance is 1e-7). On the thread swept a turn
  at a time, whose root was a B-spline, the same end had passed only behind
  a cap whose 3D curve lay 1.4 off its curves on the faces, with vertex
  tolerances of 2.

The patch 0911 comes from mitcad#343 (round 12), the foot of a pin of
radius 4.5 standing out of a wall, rounded with 1 along its arc on the
wall, which ends where the wall ends in a slot whose sides are tangent to
the pin.

- `0911-chfi3d-onsame-end-where-the-face-at-the-end-goes-on-tangentially.patch`:
  `ChFi3d_EdgeState` decides an end's state from the sides of the faces at
  the vertex's two other edges, sampled inside each edge; where an edge
  lies between faces tangent to each other, those sides follow no rule.
  At the pin's arc ends the slot's side goes on tangentially from the pin
  across their common edge, and the wall's edge with the slot's side runs
  into the vertex tangent to the arc (the wall a cusp there); the end came
  out OnDiff, the walk past the wall's edge rolled on it into the vertex
  (a blend ending in a point, 0770) or, with the end not taken as that
  edge's, ran round the pin to its seam, and the corner walked round the
  vertex: the result was 0.14 mm off the file's or invalid. Where exactly
  one of the two edges lies between tangent faces and leaves the vertex
  behind the spine's edge (on the side away from the interior of the face
  the two edges share, whose corner there is reflex, so that the blend's
  contact on that face does not meet it: `ChFi3d_TangentEdgeBehindSpine`),
  the end is now OnSame: the face at the end cuts the blend and grows up
  to it along the tangent edge prolonged (`PerformOneCorner`'s OnSame end,
  whose tangent case seeks the contact's crossing with that edge). Where
  such an edge leaves into that face (an earlier rounding of another of its
  edges coming in at the vertex, its contact line the tangent edge), the
  blend's contact runs onto the face at the end before the vertex, and the
  state is decided as before (an OnSame end failed in two such corners of
  another design, which build as before). On the stored body before the
  rounding: the file's change of volume within 2.4e-4 mm³ and every new
  face on the file's within 1e-5 mm.

The patches 0930 and 0931 come from mitcad#343 (round 13): both rims of a
ring rounded, and three roundings ending on a rod.

- `0930-chfi3d-rounding-riding-anothers-contact-line-against-it.patch`:
  0016 makes the later of two roundings of a face narrower than both roll
  on the earlier one's contact line, and `ApplyRides` makes the two blends
  meet on that line, the earlier blend's, run its spine's way; the later
  blend's side was set on it from its start to its end. A spine follows its
  edge's curve, and the two circles of a ring's band, which start at one
  point, may turn opposite ways round the axis (a ring 2.5 thick rounded
  with 1.5 on both rims, whose bottom circle came from a cylinder standing
  upside down): the later blend then used the line against its own
  direction, and its face came out reversed (its area −1617 mm²) with a
  wire not closed. `ApplyRides` now finds which way the later blend's own
  line on the support runs along the earlier one's (`rideRunsAgainst`, the
  tangents at 30 and 70 % of it); where against, that side runs from the
  line's end to its start, its ends and points are the earlier blend's the
  other way round, and once the stripes are in the data structure the
  line's transition on the later blend's surface is turned
  (`turnRidesAgainst`), as for blends cut by each other between two
  crossings. On the ring's replayed body the change of volume is the file's
  within 1.4e-7 of the stored state in either order of the rims; on the
  stored body, whose circles turn the same way, it is unchanged.
- `0931-chfi3d-seam-run-on-to-a-corners-cut-past-its-end.patch`: three
  roundings of radius 0.5 along the curves where three bores meet end on a
  rod of radius 2.5 in their middle, whose cylinder face runs all round
  with its seam from its bottom up to the vertex where two of the bores'
  curves meet, one rounding's end vertex. The rounding takes material off
  there, so the rod's face grows up to the corner's cut (the blend's
  intersection with the rod), which crosses the seam's line 0.31 past the
  seam's end. `PerformOneCorner` split the cut at the seam only where the
  crossing lay on the seam's edge or at one of its vertices; the cut was
  left whole across the line, the seam ended at the vertex the rounding
  removes, and the rod's face came out with a wire not closed. Where the
  seam ends at the corner's vertex and the cut's curve on the face crosses
  the seam's line beyond that end (the point on the face's surface, within
  the tolerances), the cut is now split there and the seam runs on along
  its line from the vertex to the crossing: a curve on the face's iso-line
  through the seam (the surface's own parametrization; the seam's curve, a
  B-spline here, cannot be prolonged), on both sides of the seam, each the
  way the seam runs there. The seam stays whole. On the replayed body the
  rod's face has the file's area (412.4848 against 412.4839 mm²) and every
  new face lies within 6e-4 mm of the file's.

The patches 0932 and 0933 come from mitcad#343 (round 14): a rounding of
radius 0.5 on three pieces of the edge between a large torus and a plane,
interrupted twice by a rod's narrow strips (2.34 and 2.26 mm²), the pieces'
contacts on the plane running past their ends onto a torus of radius 0.6
round the rod's foot.

- `0932-chfi3d-walk-cut-before-a-singular-section-ends-there.patch`: 0077's
  `ChFi3d_CutSingularExtension` cuts a piece's walk past its end at
  18.0021 (its spine 17.92 long), where the contact on that torus speeds
  up from 1.25 to 20.5. The cut end lies on no arc, so `ComputeData` left
  it open and the walk was taken up again from the cut into the same
  sections; the corners there failed. The function now returns the ends it
  cut and `ComputeData` closes them (`intl`/`intf` 1): the cut end is the
  blend's end for the corner. The common points at the walk's end are kept
  (resetting them breaks a corner the stored ones build: the arc of a
  cylinder's top cut by two planes, test `fillet_kernel`).
- `0933-chfi3d-ends-joined-over-a-narrow-face-that-pokes-through-the-blend.patch`:
  in the file each pair of ends is one blend over the strip C between them;
  over one strip the contact on the small torus N leaves it across C and
  comes back (0700's cut), over the other it stays on N and C pokes
  through the blend, a hole of 3.66 mm round a lens of the rod (0.888
  mm²). `JoinOverNarrowFaces` took C from the arc an end's contact ends on;
  ends cut short lie on none, and C is now also the face across an edge of
  N that holds both vertices. Where the walked contact never leaves N, the
  join is made when C cuts the walked blend in a closed curve and lies
  wholly beyond the walk's start section (on it at the end's vertex): one
  piece from the start to a section beyond C, the curve and the other
  end, which `PierceBlends` holes. On the replayed body the rounding, as
  the three pieces or with the rod's two edges, builds in every order of
  its contours within 0.0033 mm³ of the file's change (7.5193 mm³); before,
  two corners failed.

The patch 0940 comes from mitcad#343 (round 15): a key's top edge rounded
with 2, the key's side B a strip between that edge and a bore's wall C (a
torus) across a concave crease, 2.2 high in the middle and lower than the
ball towards both ends, where B and C end at the key's end roundings of
0.5 (C2).

- `0940-chfi3d-rounding-over-a-narrow-face-cut-by-the-face-beyond-its-concave-edge.patch`:
  past the crossing where the ball's contact on B reaches the crease, the
  ball rolled on the crease (0011's obstacle), sticking out of the key's
  side, and the corners at the ends (four edges each) failed. The file's
  rounding is B's cylinder all along, cut where C's wall runs through it,
  C grown over the strip up to the cut; at each end C2 cuts the blend from
  where the junction of C and C2, run on along its circle, meets it, down
  to the key's top. `ChamferOverNarrowFace` (0531) now takes a rounding
  whose piece rolls on a concave crease of a face of its edges, or whose
  contact ends on such a crease away from the spine's end vertices while
  the other contact goes on (over a convex crease the ball still rolls on
  the edge, 0620). `ChamferCutByFacesBeyond` takes a crossing where the
  other contact lies outside C's material and the contact on B's surface
  past it inside (roundings only): C cuts B's side from the crossing up to
  where the other contact enters C's material, the piece ending in a point
  there. At the spine's ends with two faces beyond B the junction runs on
  along its own circle, line or ellipse where that lies on both faces;
  where it meets the blend before that point, C2 cuts the blend from there;
  where it meets the other face first it bounds the growth instead of the
  edge's tangent line, and an edge of the other face and C2 through that
  point is cut there. Curves of two faces that the intersection lines miss
  at a vertex where the faces touch are found by points, and the cuts'
  curves are moved into the periods of the faces beyond. On the design's
  stored body: valid, every new face on the file's within 1e-3 mm (the
  file's sliver of B of 0.0024 mm² at one end lies 0.017 mm off), the
  change of volume -9.5996 against the file's -9.5954 mm³; before, two
  corners failed.

The patches 0941 and 0942 come from mitcad#495: a design's chamfer hung in
the kernel. Four bosses of radius 7.5 under a plate, their feet chamfered
by 13 (each boss 13 long, so its side goes with the chamfer); two of them
stand under the plate's rounded corners (radius 20 about the boss's axis)
whose one side is slanted, tangent to the corner's arc, so that the contact
on the plate's underside (radius 20.5) runs 0.5 beyond the outline round
the corner and along both sides (0142's gap, cut by the chain of the faces
beyond). The bosses' seams lie just before the gap.

- `0941-chfi3d-kpart-chain-crossings-in-the-period-after-the-gaps-start.patch`:
  `ChFi3d_KPartEdgeCrossing` takes a crossing's parameter on the other
  contact's line (the boss's circle) in the period around the middle of
  the piece before the gap; a long gap ending towards the spine's end got
  its last crossing a period early (-0.286 for 5.997), and the last piece
  of the chain, from there to the next piece's start (6.218), ran more than
  once round the circle: the boss's bottom face got edges covering more
  than its rim, the cone's seam a tolerance of 58 mm, an invalid result.
  `ChFi3d_KPartChainPieces` now takes each crossing's parameter in the
  period that follows the end of the piece before the gap (or the chain's
  given start) where the gap does not run across the spine's start; a gap
  across the start keeps them as before (0145, 0146). On the design's
  replayed body: valid, the change of volume +25112.5803 against the
  file's +25112.5808 mm³; before, invalid (two of the four bosses +618 and
  -733 mm³ off).
- `0942-shapefix-intersecting-wires-cut-reported-only-where-cut.patch`:
  the healing of that invalid result (`ShapeFix_Shape`, as the dressups'
  `healed` does) never returned. `ShapeFix_IntersectionTool::FixIntersectingWires`
  intersected an arc of one wire of the boss's bottom face with the next
  arc of the same circle in another wire; the intersection was a segment
  of no length at their common vertex, both edges were to be cut there and
  the pair intersected again without counting a modification. `CutEdge`,
  for an edge that is not same parameter with a trimmed curve as its
  p-curve, cut only a trimmed line but answered that it had cut any
  trimmed curve; now it answers so only where it cut (a line cut to the
  range it has is no cut either), and the repair goes on to the next pair.
  With 0941 alone the design's chamfer builds; with 0942 alone it fails at
  once ("an invalid shape") instead of hanging.

The patch 0950 comes from mitcad#343 (round 17): the floors and ceilings
of windows in a ring wall 3 thick (radii 112 and 115), rounded with 1.5
on both edges, ending at the pillars' full-rounded ends, the windows the
copies of a circular pattern. The two roundings of a sill are halves of
one torus (113.5, 1.5); their contacts on the sill lie along each other at
the wall's middle.

- `0950-chfi3d-roundings-consuming-the-face-between-them-meet-on-one-contact-edge.patch`:
  `ChFi3d_StripeEdgeInter` took the two contacts, which overlap all along,
  for roundings too big for the face; such a pair (one segment covering
  both ranges) now meets along the contacts. After the stripes are in the
  data structure (`joinEndsAlongContacts`), the points two such stripes'
  corners made where the contacts end on an edge of the face are one, the
  edge leaves the face on both sides of them, and where both ends are one
  the second contact edge is replaced by the first (its curve on the second
  blend projected anew, its transition turned where the two run opposite
  ways), the face holding that edge on both sides. Before, the sill came
  out as a sliver of no area, or with the arc's pieces left on it and on
  the pillar's end, depending on the window's copy. On the design's body
  before the item (48 edges, four windows): valid, every face on the stored
  body's within 1e-6 mm both ways, the change of volume -243.0297 against
  the file's -243.0189 mm³; before, two of the four windows failed.

The patch 0951 comes from mitcad#343 (round 17) as well: a rim's edge
rounded with 0.3 up to a vertex where the rim's wall steps back by 4.3
micrometres. The wall's face at the edge ends on a strip that narrow (the
step's face, square to the rounding); the stored bodies end the rounding
at the section through the vertex and close it with a quarter torus.

- `0951-chfi3d-rounding-ending-at-a-step-back-of-its-wall-closes-with-a-torus.patch`:
  OCCT found no face for the rounding's end and left an invalid shape.
  `PerformPivotAtStepEnd`, tried first for the end of a lone stripe, takes
  an end where the contact on a plane ends on its edge with a plane strip
  narrower than a tenth of the radius, the face beyond the strip parallel
  to the first and the strip's width into the material, and the other face
  a plane square to both: the stripe ends at the section through the
  vertex, and a corner stripe of one piece closes it, the torus the ball
  sweeps turning on about its contact's end point until it lies in the
  wall's plane (both radii the rounding's), cut by the stepped-back face.
  The section becomes two edges, the one over the strip bounding it. On
  the design's body before the item (14 edges): valid, every face on the
  stored body's (distance 0 both ways), the change of volume -1.7315
  against the file's -1.7477 mm³; before, the result was invalid.

The patch 0960 comes from mitcad#504: a rounding's corner of more than
three edges hung while its boundary curve was projected onto the initial
surface of a plate. A stack sample of the import of `eecf90bd` stopped in
`DichExactBound` inside `ProjLib_CompProjectedCurve`.

On the replayed body before item 50 (Fillet7, radius 1 on ten edges),
OCCT's four fillet contours reach that projection with a boundary made
from a straight 2D line on a B-spline blend surface. The boundary extends
far beyond the surface's native parameter bounds, so evaluating it
extrapolates the surface and produces plane coordinates of order 1e11
and a projection derivative of order 1e10. The boundary search's last
accepted parameter is 102.90382719900337, the failed one
102.90382719900339: adjacent doubles, 1.4210854715202004e-14 apart. Its
UV-derived stopping tolerance is 1.1242033789204225e-14, smaller than that
gap, and their midpoint rounds to the failed endpoint. A second projection
onto a bounded initial plane reaches the same condition with a tolerance
of 1.1343993885480033e-15.

The replayed input has 98 faces. On the file's stored input (84 faces),
the same ten edges match within 1.1e-14 mm and the same four contours fail
promptly, before and after 0960. On the replayed input, 0960 makes the
boundary search return too; the projection cannot cover the whole corner
curve, and `ProjLib_CompProjectedCurve::D0` rejects a parameter outside its
continuous parts. The OCCT-only probe then reports that the corner failed
instead of hanging. It uses OCCT's normal signal-to-exception support,
as the import already does.

In the diagnostic and final production macOS builds, importing `eecf90bd`
without `--hang-limit` returns: item 50 fails promptly and takes the stored state through the
existing import error handling. Its final report names a zero-length
curve where the rounding's last pieces lie on different faces. All 120
item outcomes stay as before (101 parametric, 7 fallback, 12 skipped),
all other item notes are unchanged, and the five final bodies' volume
differences are identical to the baseline. The watchdog warning is gone.

- `0960-projlib-projection-boundary-bisection-stops-at-adjacent-parameters.patch`:
  the boundary search bisects between a curve parameter that projects and
  one that does not. Its stopping tolerance is the UV tolerance divided
  by the projected curve's derivative; it can be smaller than the spacing
  of the parameters' floating-point values. Once the bracket's endpoints
  are adjacent values, their midpoint is one of the endpoints and the same
  projection is tried for ever. Stop then, retaining the last successfully
  projected point. The projection's tolerances and success criteria stay
  as they were; no failed point is accepted and no iteration limit cuts a
  search short while it can still make progress.
  The OCCT-only test `geometry.projection_boundary` projects a straight
  B-spline segment with knots from 1e6 to 1e6 + 1e-6 onto a bounded plane,
  where its crossings have no representable parameters. Without 0960 the
  constructor hangs in `DichExactBound`. The test checks that the boundary
  points project correctly to 1e-6 mm and that their adjacent parameters
  lie outside the plane's bounds, instead of demanding a curve point at an
  unrepresentable parameter. A segment with the same knot span near 1 is
  the control.
The patch 1001 comes from mitcad#343 (round 19): the bar's outline of
0961's case rounded with 0.3, after 0980 joined the port.

- `1001-chfi3d-guide-stretch-only-where-the-spine-turns-at-the-vertex.patch`:
  each side's chain of tangent edges starts at the vertex where its blend
  vanishes, and the walked piece of its first edge ends 0.018 past the
  next edge's start, where that edge's analytic piece starts. 0980 built
  that guide from a stretch of the next edge as well (0.136), meant for a
  spine that turns at the vertex (a fin's top edges kinked by a degree and
  a half). Here the edges go on tangentially (2e-6 rad): the guide,
  approximated across the vertex, passed its end 1e-5 mm and 5e-4 rad off
  the spine's point and tangent, where the analytic piece's section is,
  and the walk's first step from that section failed on all four chains,
  on the replayed and on the stored body. `ChFi3d_PerformElSpine` takes
  the stretch only where the spine's tangents just before and after the
  vertex lie more than a thousandth of a radian apart. On the design's
  body before the item: valid, the stored body's 174 faces, -2.2689 mm³,
  as with 0961 alone; 0980's fin still builds.

The patch 1000 comes from mitcad#343 (round 19): a fin 1 mm thick whose
top edges, an arc and a profile's spline stretch in turn, meet at vertices
where the top's faces kink by 1.4 and 2.2 degrees (concave creases), both
top chains chamfered by 0.263 at 45 degrees, the side strips between the
top edges and the root's chamfer (a walked B-spline between a plane and a
torus) narrower than the chamfer along the spline stretches (0531's case).
The chamfer failed with an exception before any part of it was made.

- `1000-chfi3d-chamfer-across-a-slight-concave-crease-over-a-narrow-face-grown-faces-continued.patch`:
  0980's crease turn in `StartSol` (a face taken for tangent that meets the
  face before at a concave crease of more than the angular tolerance is no
  continuation: the ball turns about the crease's edge, rolling on it) took
  the chamfer's piece after the arc's analytic one for a blend rolling on
  that edge, which chamfers do not have ("PerformSurf Not Implemented"),
  and left no piece for `ChamferOverNarrowFace` to find the narrow face by;
  a chamfer's contact, at its distance along the face, passes the crease,
  so the turn is a rounding's only. Computed again over the narrow face's
  surface unbounded, 0671's `ChFi3d_KPartCutChainToEnd` asked whether the
  arc's analytic piece takes material off by the spine's edge with its two
  faces, one of them the unbounded face, which does not hold the edge
  (`BRepTools::OriEdgeInFace` threw); it now looks only at faces of the
  spine's edge (`ChFi3d_EdgeOfFaces`), the narrow-face computation cutting
  the chamfer by the faces beyond itself. The root's chamfer, grown up to
  the tip chamfer along its sections continued straight
  (`ChFi3d_WideSurface`), kept its surface, so the face ran past its
  surface's parameter bounds (17 of 82): its geometry where OCCT evaluates
  the B-spline past its bounds, but tools that keep to the bounds
  (`ShapeAnalysis_Surface`'s projections) found its points nowhere on it.
  `ChFi3d_Builder::Compute` now gives each face of the result running past
  its non-rational B-spline surface's bounds in a direction of degree 1
  that surface continued linearly to the face's bounds
  (`ChFi3d_ContinueGrownFaces`: the end row of poles moved along the end
  span's lines, the knot with it, the parameters as they were), each edge's
  curve on the face stored again with the continued surface. On the
  design's body before the item, replayed and stored: valid, every face of
  the file's within 3e-4 mm of the result's and the result's on the file's
  within 1e-3 mm; before, the chamfer failed.

The patch 0990 comes from mitcad#343 (round 18): a groove of radius 5
under a slot's end wall, whose face is a strip 0.86 high between the groove
and the slot's concave roundings of 9 and its ceiling, the groove through a
notch in the wall's 1 mm thickness; the groove's rim on the strip rounded
with 10 rolls on the strip's top edges, and the notch's walls are the faces
at the ends.

- `0990-chfi3d-rolled-end-cut-by-the-faces-at-the-end-past-the-balls-tangency.patch`:
  past the ends the walk went on with the contact on the groove run off
  across its edge onto the wall beside the notch, and both corners failed
  ("Corner OnSame : no point on arc", "OneCorner : bouchon non ecrit").
  `RolledEndCutByEndFace` (tried at each end after `WalkedOnSameToEndFace`)
  takes an end piece rolling on an edge Er of a face B whose other contact,
  on N, ends on N's edge A into a face Fa, at a spine's end vertex of N, the
  face across Er and the face at the end Be: it walks the blend again from
  the piece's middle with N's surface unbounded, rolling on Er up to its end
  (again a little short of it where the approximation of the singular last
  section fails), runs its surface on along its tangents
  (`GeomLib::ExtendSurfByLength`, C1) and cuts it by Fa from where the
  contact on N crosses A, by Be, and by B's surface up to the point where
  that intersection ends on the contact on Er, the ball tangent to B there
  (the branches of the surfaces' intersection lines through the cut's
  points). Be grows up to the cut along its edges with the face across Er
  and with Fa prolonged and its curve with B, which grows up to it as well;
  Er is cut, and the edge of N and Be and the spine's edge at the vertex go
  with the blend. The end is put in the data structure with no corner
  (`PerformFilletOnVertex` makes none at the end of a lone stripe already
  there). `ChFi3d_ExtendAtSpineEnds` no longer looks up arcs of faces made
  of surfaces unbounded in the edge/face map. On the design's body before
  the item: valid, every face on the stored body's within 1.6e-4 mm both
  ways, the material in a box round the rounding 207.3352 against the
  stored body's 207.3360 mm³; before, both corners failed.

The patch 1010 comes from mitcad#343 (round 19): a bore's rim of radius 85
in two arcs chamfered 1 on the top and 2 down the wall, where a pocket from
below (a cylinder whose side breaks through the wall) ends 1 under the top;
along the shorter arc the wall is a strip 1 high over the pocket's
ceiling, the arcs' vertices where the pocket's side meets the wall.

- `1010-chfi3d-kpart-closed-rim-gap-round-to-its-own-first-edge.patch`:
  the chamfer's contact 2 down the wall lies in the pocket all along the
  shorter arc, so `SplitKPart` gives the longer arc one piece, its ends on
  the pocket's side, and the shorter arc none.
  `ChFi3d_FillKPartRoundGapsAcrossEdges` (the faces round from the
  pocket's side back to it, the side, the ceiling and the side, cut the
  blend over the gap) looked for the piece after the gap only on the
  other edges, never back on the gap's own first edge, and left the gap
  to the walking chamfers do not have: the chamfer failed. On a closed
  spine whose other edges have no piece the search now goes round to the
  first edge again, a period on (the pieces before and after the gap may
  be one), and the gap's edges are told apart by their place in it, not by
  their numbers. The ceiling cuts the cone lengthwise and keeps its part
  outside it, up to 84.5 from the axis, and the pocket's side cuts it near
  the arc's ends, both growing up to the cut, as the stored bodies have it.
  On the design's body before the item: valid, every face on the stored
  body's within 2e-6 mm both ways, the change of volume -522.6458 against
  the file's -522.6452 mm³; before, the chamfer failed.

The patch 1011 comes from mitcad#343 (round 19), from the same design once
the chamfer of 1010 is built: the bore's inner rim of radius 82 rounded
with 0.5 where a pocket of radius 25.5 cuts into it (the rim an arc from
one side of the pocket round to the other, its torus' parametric seam at
its start), then the pocket's top rim rounded with 1 up to that torus.

- `1011-chfi3d-intersection-at-end-across-the-end-faces-seam-in-one-period.patch`:
  at the seam end the rounding's contact on the top runs past the torus'
  edge (the torus grows up to the cut), and its point on the torus came
  out at u = 6.263, a period beyond the face's domain [-0.005, 5.692],
  while the other end lay at u = -0.002 in it. `PerformIntersectionAtEnd`
  walked the intersection of the blend with the torus between them the
  long way round: a curve 3.3e6 mm long and a body of 1e12 mm³ that the
  checker took for valid. On a periodic face at the end, an end of the
  intersection outside the face's domain and more than half a period from
  the other end, which lies in it, is now taken in the period next to that
  end (in u and v) before the intersection is computed. On the design's
  replayed body and on the stored body before the item: valid, -7.525 and
  -7.615 mm³ at the seam end (the other side's rounding -7.618 and
  -7.512); the item's three roundings one after the other give the stored
  body after it within 9e-4 mm both ways. Before, the rounding gave the
  1e12 mm³ body, which the import measured as no solid, and the item fell
  back where its three roundings' first definition had given way at its
  share of the item's time.

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

- `1020-chfi3d-kink-into-a-face-going-on-smoothly-into-the-end-face-also-at-a-pieces-start.patch`:
  before 0980 the walls were taken for the sides continued (their normals
  within a tenth of a radian) and the ball rolled on down the block's
  corners, which it rounded with 0.5 (the material taken 1.127 against the
  stored 0.932 mm³; the import accepted the 0.04 mm as corner faces). With
  0980 both contacts reach their creases at one section and the ball
  turned about both creases' edges at once: the fillet failed. The walls
  are faces at the end as `KinkIntoEndFace` (0750) has them, but they hold
  no vertex of the spine: they go on smoothly into the corner's cylinder,
  which holds the corner edge's end vertex. `KinkIntoEndFace` now also
  takes a face beyond the kink that goes on smoothly
  (`ChFi3d_SmoothIntoFace`: the normals within the angular tolerance all
  along an edge of both) into a face at the end vertex other than the face
  before. That check, with 0980's rule that a face of an edge of the spine
  is no face at the end (`ChFi3d_KinkIntoEndFaceOfSpine`), was made only
  where a walk ends on the arc (`ComputeData`); `StartSol` makes its new
  kind too (a face beyond that holds no end vertex itself), where the next
  piece starts from the arc of the piece before: the corner's analytic
  piece, a cylinder between the planar sides, ends at the creases, and for
  a corner edge running from the block up the walk from there to its first
  vertex started over them. That side now returns as at an end before a
  face at an angle, and the end is built as where a walk ends on the arc.
  A face beyond that holds the end vertex itself stays the walk's there,
  which runs on it up to the vertex: taken for a face at the end in
  `StartSol`, 0980's fin top (`5ef2dbd5` item 87) failed at its corner.
  On the design's replayed body before the item:
  valid, the material taken 0.931662 against the stored 0.932227 mm³,
  every face of the result within 1.4e-4 mm of the stored body after the
  item and the stored body's within 8.2e-5 mm of the result.

The patch 1030 comes from mitcad#539 (the FreeCAD import's pipes along an
auxiliary spine): an 8 x 2 bar swept 50 along a line and turned a quarter
turn by an auxiliary spine 10 above it, crossing from one side to the
other, the spine's points at the same fraction of the lengths as the
line's (the sweep's auxiliary `frame` with `curvilinear`; the reference
model `pd_pipe_modes`, whose stored pipe holds 800 mm³: every section is
the bar).

- `1030-geomfill-guide-trihedron-ac-second-derivative-of-the-binormal.patch`:
  the trihedron's values and first derivatives were right (checked against
  differences of D0 and D1), and so were the pipe's sections (`Simulate`)
  and its surfaces at the approximation's sections, but its side faces lay
  up to 0.27 off the turned bar between them: the pipe held 784.9 mm³
  (775.9 at a tolerance of 1e-7). `D2` took the binormal's second
  derivative as a unit vector's with the square of the binormal itself (1)
  where the square of its first derivative belongs, which added `-(1 -
  |d|^2) b`; the binormal's second derivative was off by about the
  binormal. With the square of the first derivative (as the normal's
  second derivative has it) the pipe holds 800 mm³, as by the plane rule.

The patches 0961 and 0962 come from mitcad#343 (round 18): the same bar's
outline rounded with 0.3 on both sides (22 edges, the item before that
rim's rounding). Each side's chain runs along an earlier concave rounding
(radius 3) that goes on tangentially into a corner rounding of 0.4, itself
tangent to the bar's side: the outline edge's two faces come tangent at its
end vertex V, and the blend vanishes there.

- `0961-chfi3d-rounding-vanishing-where-its-walk-stopped-on-its-own-edge.patch`:
  the stored faces meet only within the edge's tolerance (1.5e-3). A gap g
  between faces at an angle a moves the line where they meet sideways by
  g / a, and the ball's contacts, R a / 2 from that line, reach the edge
  itself where a² = 2 g / R: the walk's contact on the earlier rounding met
  the spine's own edge 0.014 to 0.042 short of V (sections 0.004 to 0.011
  wide) at all four ends, and the walk ended there. The end was not taken
  as vanishing (0130 takes contacts within a hundredth of the radius), so
  the corner went to the intersection at the end, whose plate failed at
  one of them (each tangent chain alone gave an invalid shape). Where a
  lone stripe's contact ends on the spine's own edge into V, a tangency extremity, its
  section no wider than sqrt(2 R tol) and both contacts within the radius
  of V (`ChFi3d_ContactOnOwnEdgeAtEnd`), the end is put at V as 0130 does,
  without comparing the faces' normals; the blend left between the stop
  and V is narrower than that section. `ChFi3d_ShrinkVanishingEnd` then
  bends the blend's surface with its contact curves (its boundary isos):
  all three cut by a full knot at one parameter, the surface's end row of
  poles put at V, so that the surface ends in V and the tolerances are the
  curves' sampled deviation from it and from their faces, not the distance
  the end moved (as wide as 0.04, which the dressup's check of inflated
  vertex tolerances took for a corner left open). On the design's body
  before the item: valid, the file's 174 faces, every face on the stored
  body's after it within 0.004 mm both ways.
- `0962-chfi3d-step-end-torus-cut-on-its-direct-frame-and-only-at-a-recess.patch`:
  with the outline rounded, 0951's rim rounding (the next item) met the
  step with the wall on the blend's first side. 0951 reverses the torus in
  u there (the blend's normal asks for it); its cut by the stepped-back
  face, computed on that reversed torus, whose frame is indirect, failed,
  the end went to the other corners, and the side plane's wire came out
  broken (area -642 mm²). The cut is now computed on the torus as built,
  its end points' u negated, and its p-curve mirrored in u onto the
  reversed torus: valid, every face on the stored body's within 0.004 mm.
  With both ends so built, the item after it (radius 0.2 on the walls'
  edges below the step) built quarter tori at both its ends 0.04 off the
  file's: the stored bodies run that rounding on across the step, cutting
  the recess's 4.3 micrometres, until the rim's rounding (whose torus ends
  the recess at its far side) cuts it. The pivot is now taken only where
  the stepped-back face's edge with the other face ends at its far end on
  a strip back to the wall's plane (a recess, 0951's case); elsewhere OCCT
  runs the blend on as before, and that item has the file's 229 faces,
  every face of the result on the file's within 4.3e-4 mm.
The patch 0970 comes from mitcad#343 (round 18): a part's underside (a
plane) rounded with 0.5 along an outline of a side wall square to it, a
corner rounded with 0.3, a front wall leaning 15 degrees over it (its
edge with the underside acute), a second corner and the other side wall.
The ball cannot roll round a convex corner tighter than itself: its centre
would turn back about the corner's axis. OCCT walked the chain over the
corner's arc anyway and threw storing the walk (`StoreData`, the
approximation of a walk running backwards). The stored bodies end both
walls' roundings where their axes cross and close the corner with a vertex
blend (a B-spline patch, its third side the corner's arc turned square to
the underside, a point at the ball's contact with the underside) and a
torus about the front rounding's axis (minor radius the corner's, major
the difference of the radii), which cuts the corner's cylinder.

- `0970-chfi3d-chain-past-a-wall-corner-tighter-than-the-ball-vertex-blend-and-torus.patch`:
  `SplitAtTightCorners` (new, as `Compute` starts) splits a convex contour
  of constant radius R at an arc between a plane U and a cylinder K of
  radius below R, between two lines whose other faces are planes tangent
  to K, one of them (Wp) square to the other line, the other (Wo) square to
  U or leaning over it (an obtuse one is left as before): the runs between
  such arcs become contours whose ends at the arcs have the status
  `ChFiDS_Tangent`, the arcs left out. `PerformTightCorners` (new, before
  the corners at vertices) finds the ball's centre C, R inside U, Wp and
  Wo, and builds the vertex blend C + sin t A(u) + cos t R n (n U's
  outward normal, A(u) the arc of K's radius about C + rho np + rho ep in
  the plane through C parallel to U, rho = R - rk, np Wp's outward normal,
  ep along Wp's rounding towards the corner), an exact rational
  biquadratic patch tangent to both roundings along their ellipses through
  the ball's contact with U; where Wo leans, the torus about Wo's rounding
  axis turns K's section from Wo's normal to ep and meets the vertex blend
  along its arc, Wo's rounding along an arc of radius R, Wp along an arc of
  radius rho, and K along K's section. The roundings end on those curves,
  the vertex blend and the torus are corner stripes on the roundings'
  surfaces and Wp, and K is cut along its section. `SplitKPart` keeps the
  pieces at the last end of such a contour, as at its first end (before,
  it dropped them all and the analytic rounding was walked). On the
  design's body before the item (the outline's five edges): valid, the
  file's 197 faces, every face on the stored body's within 0.002 mm both
  ways, the change of volume -1.5382 against the file's -1.5432 mm³ (the
  file's vertex blends are cubic B-splines);
  before, the fillet failed.
The patch 0980 comes from mitcad#343 (round 18): a fin's top edges rounded
with 0.2, the top an arc and a straight part, the sides chamfered along the
arcs only, slivers joining the chamfers to the straight part's sides. Every
face meets the next at a crease of one to five degrees; the selected edges
are the arcs' and the straight parts', short sliver edges between them.

- `0980-chfi3d-rounding-across-slight-creases-of-its-faces-at-one-section.patch`:
  five changes, each needed by the next. A contour stopped at a turn
  (`StopChainsAtTurns`) runs on over an edge not asked for that leads,
  tangentially, to an edge asked for: each selected edge was a contour of
  its own, ending at a vertex of four sharp edges where the corner failed.
  The guide of a walk that starts a hair short of the end of its edge
  after an analytic piece (less than a hundredth of the guide's length) is
  built from a stretch of that edge before it, its parameters kept: its
  first span was that hair, the spine's kink turned within it, and the
  first section's reframing, looking a little before the start, met the
  guide extrapolated off the spine. A kink onto a face holding the spine's
  end vertex (`KinkIntoEndFace`) ends the blend only where no edge of the
  spine lies on that face. Where one contact crosses onto a tangent face
  and the other lies within ten times the approximation tolerance of such
  an edge ahead, both change faces at once, and the first section starts
  on both arcs where they are reached within ten times the guide's
  tolerance of each other: a walk on one new face and one old was taken
  between, out of order with its neighbours. A face taken for tangent
  across a concave crease of more than the angular tolerance does not
  continue the face before: the ball rolls on the crease's edge from the
  ball of the piece before until it touches that face (its reference face
  there), the edge taken on the face whose part beside it stays, the walk
  not completed back over the piece before, and the crease's point where
  the ball leaves it onto the face it rolled on given that piece's
  transition only. Before, the walk on the next face started some 0.1 mm
  further on and the gap was closed by edge tolerances of 0.1 mm. On the
  design's body before the item (12 edges): valid, every new face on the
  stored body's within 0.0083 mm and the stored body's on the result's
  within 0.0024 mm, the change of volume -0.2909 against the file's
  -0.2888 mm³; before, the corners failed.

The patch 0540 goes on with the corners where a rounding ends (mitcad#236,
eighth round): a rounding that runs on into an earlier rounding's tube.

- `0540-chfi3d-rounding-running-on-into-an-earlier-roundings-tube.patch`: a
  wall's foot rounded with the radius of an earlier rounding round the
  wall's end, whose tube leaves the wall's side tangentially (its spine
  curving away from the side's foot) and was cut back to the wall's plane,
  where its floor contact crosses it: the new rounding's spine ends at that
  crossing, and its section where the two spines are tangent lies in the
  earlier tube. At that OnSame end the earlier rounding is the face at the
  end; the corner intersected the opposite contact with it, which it only
  touches, and the blend with it, tangent to it all along the section ("OneCorner
  : bouchon non ecrit", "OneCorner : echec calcul intersection"). Now
  `PerformOneCorner` takes the point where the contact touches the face's
  surface within the tolerance, the blend tangent to the face there
  (`ChFi3d_ContactTouchesEndFace`), caps the blend with its end section
  where that lies in the face's surface, tangent to it (as an end section
  lying in a face at the end is since 0410), and prolongs the earlier
  rounding's edge with the opposite face along the iso of the face's
  surface the edge runs on where the edge's curve ends at the vertex (a
  stored B-spline; `ChFi3d_ProlongTangentEdgeAlongIso`, as
  `ChFi3d_ProlongTangentEdge` does with a curve that goes on past it). The
  earlier rounding grows up to the section, as in the files' bodies.

Each patch has a report for OCCT, with a reproduction that uses OCCT alone
and measurements: mitcad#84 (0006), mitcad#90 (0007), mitcad#91 (0008) and
mitcad#92 (0009); for 0010 to 0019 and 0050 to 0077, mitcad#121 and the tests in
`geometry/tests/test_fillet_overflow.cpp` (for 0075 to 0077 also
`geometry/tests/test_fillet_rolling_ball.cpp`), which use OCCT's fillet alone
where they can; for 0020 to 0023, mitcad#122 and
`geometry/tests/test_fillet_kernel.cpp`; for 0024 to 0026, mitcad#133
and `geometry/tests/test_fillet_ends.cpp`; for 0030 to 0032, mitcad#132
and `geometry/tests/test_allocation.cpp`; for 0033, mitcad#203 and the
helix cases of `geometry/tests/test_sweeps.cpp`; for 0034, the ctests
`cli.occt_crash` and `cli.occt_crash_section` on Windows; for 0060, mitcad#60 and
`geometry/tests/test_shapefix.cpp`, whose expected results are OCCT's
without the patch; for 0090 and 0091, mitcad#160 and
`geometry/tests/test_chamfer_kernel.cpp`, which aborts without them; for
0120 and 0121, mitcad#119 and `geometry/tests/test_large_bodies.cpp`, whose
statuses are OCCT's without them; for 0130 to 0139, 0210 to 0219,
0280 to 0289, 0290 to 0299, 0340 to 0349 and 0410 to 0419, mitcad#236, for 0430 to 0432, mitcad#339, and for 0450, mitcad#341, and `geometry/tests/test_fillet_corners.cpp`,
whose tests of 0280 to 0283, 0290, 0291, 0340 to 0342, 0410, 0430 to 0432 and 0450 each fail without their patch; for 0360, mitcad#235 and
the split by tangent curves in `geometry/tests/test_boolean.cpp`, which fails without it; for 0140 to 0149, mitcad#240, mitcad#263 and
`geometry/tests/test_chamfer_overflow.cpp`; for 0380 and 0381, mitcad#263
and the rounded crease in `geometry/tests/test_chamfer_overflow.cpp`,
which fails without either; for 0150 to 0159, mitcad#239
and `geometry/tests/test_fillet_results.cpp`; for 0160, mitcad#238 and the
rounded-rim tests in `geometry/tests/test_fillet_overflow.cpp`, which fail
without it; for 0170 to 0179, 0390 to 0399 and 0500 to 0509, mitcad#237
(0503: mitcad#378) and `geometry/tests/test_fillet_rolling_ball.cpp`, which
fails without them;
for 0180, mitcad#241 and `geometry/tests/test_fillet_chains.cpp`; for 0190,
mitcad#261 and the turned-disc test in
`geometry/tests/test_fillet_overflow.cpp`, which fails without it; for
0200, mitcad#239 and the standing-contact tests in
`geometry/tests/test_fillet_results.cpp`, which fail without it; for
0220, mitcad#284 and the coaxial cylinders' join in
`geometry/tests/test_boolean.cpp`, which fails without it; for 0240 and
0241, mitcad#239 and the seam and notch tests in
`geometry/tests/test_fillet_results.cpp`, which fail without them; for
0250, mitcad#308, mitcad#297 and the chamfered box with rounded corners and
the box whose top rounding is larger than its rounded corners in
`geometry/tests/test_fillet_overflow.cpp`, which fail without it; for
0260, mitcad#307 and the rounding carried across a notch in
`geometry/tests/test_fillet_results.cpp`, which fails without it; for
0270 and 0271, mitcad#239 and the ribs and the slot rounded to a full
round and the narrowing top and channel in
`geometry/tests/test_fillet_results.cpp`, which fail without them; for
0300, mitcad#239 and the rib's foot rounded across the rod's seam in
`geometry/tests/test_fillet_results.cpp`, which fails without it; for
0320 and 0321, mitcad#322 and the tests of where a fillet stops and of the
slot's rim rounded as wide as its ledge in
`geometry/tests/test_fillet_kernel.cpp` (the first needs 0320's API to
compile, the second fails without 0321); for 0322, mitcad#331 and the fin
whose sides are rounded one after the other in the same file, which fails
without it; for 0330 and 0331, mitcad#323 and the tests of which corner a
fillet failed at and of the block's top edge rounded wider than its side
in the same file (the first needs 0330's API to compile, the second fails
without 0331); for
0350, mitcad#300 and the boss's top rounded after its foot in
`geometry/tests/test_fillet_overflow.cpp`; for 0351 and 0352, mitcad#300
and the stepped pockets' wall feet in
`geometry/tests/test_chamfer_overflow.cpp` (for 0352 also mitcad#327's
flange with a rounded rim there); for 0400 and 0401, mitcad#330
and the valley whose floor's rounding vanishes short of its end and the
gable's ridge ending on a cylinder's seam in
`geometry/tests/test_fillet_corners.cpp`; for 0420, mitcad#337 and the
boss's rim rounded across a stepped seam in
`geometry/tests/test_fillet_kernel.cpp`; for 0440, mitcad#334 and the
boss on a slab in `geometry/tests/test_chamfer_overflow.cpp`; for 0460,
mitcad#342 and the boss's foot rounding ending on a rounding of its radius
in `geometry/tests/test_fillet_corners.cpp`; for 0480 and 0481, mitcad#263
and the post beyond a square plate in
`geometry/tests/test_chamfer_overflow.cpp` (its turned plate fails without
0480, every seam in a gap without 0481); for 0490, mitcad#236 and the
plate on a conical foot with a trimmed cone in
`geometry/tests/test_fillet_corners.cpp`; for 0540, mitcad#236 and the
wall's foot rounded on into its end's earlier rounding (its floor edges as
circles and as B-splines) in the same file; for 0510 to 0512, mitcad#349
and the threaded block whose cylinders, flanks and helices run 32 turns in
`geometry/tests/test_multi_turn_faces.cpp` (its box cuts fail without 0510
or 0511; its cross hole checks the helices' crossings that 0512 finds);
for 0530 and 0531, mitcad#263 and the fin whose side strip is narrower
than its tip's chamfer in `geometry/tests/test_chamfer_overflow.cpp` (the
chamfer's volume from its sections, cut by the foot's chamfer, swept along
the edge); for 0550, mitcad#343 and the bar on a cylinder rounded where the
cap's edge is in two pieces in `geometry/tests/test_fillet_corners.cpp`;
for 0560, mitcad#380 and the box cuts on the threaded block's cross-holed
result in `geometry/tests/test_multi_turn_faces.cpp`; for 0580 and 0581,
mitcad#343 and the pocket rounded over ribs up to flush blocks in
`geometry/tests/test_fillet_corners.cpp` (the volume added against the
rounding's section); for 0600, mitcad#343 and the plate with an overhanging
tongue in `geometry/tests/test_fillet_corners.cpp` (the tongue's left edge
rounded so that the contact runs off it, against a tongue wide enough for
the contact); for 0590, mitcad#235 and the nut threaded past its bore's
bottom, intersected with the cylinder inside the bore, in
`geometry/tests/test_sweeps.cpp`; for 0591 to 0593, mitcad#235 and the
threaded block's 32-turn flanks in `geometry/tests/test_multi_turn_faces.cpp`
(the samples of a face on a third of a turn of a flank, fail without 0591;
the grid nodes of a projection onto a flank, fail without 0593; 0592's
merge after a cut keeps the result, its work is not observable from
outside the merge); for 0620, mitcad#343 and the wall on a
floor that narrows before the end in `geometry/tests/test_fillet_corners.cpp`
(the volume added against the full rounding's section and the rolling
ball's); for 0650, mitcad#235 and the flange under a ring of teeth in
`geometry/tests/test_chamfer_overflow.cpp` (its rim chamfered a little
past the teeth's roots and exactly up to them, by distance and angle and
by two distances, its top circle in one or two edges: the volume against
the revolved section less the teeth extended down); for 0651, mitcad#235
and the cylinder under two tilted planes in
`geometry/tests/test_fillet_kernel.cpp` (the rounding of one arc of its
top, walked past its end from the cylinder's seam: the stripe is made);
for 0670, mitcad#235 and the boss topped by a steep bowl in
`geometry/tests/test_fillet_results.cpp` (its top's outline rounded past
corners of the ball's radius: each corner one face on the ball's sphere,
its area the spherical triangle's); for 0671, mitcad#235 and the groove's
end wall chamfered past its flanks in
`geometry/tests/test_chamfer_overflow.cpp` (three reaches: the removed
volume against the chamfer's wedge within the groove carried on past the
wall; one edge between the chamfer and the top through Mitcad's chamfer);
for 0680, mitcad#400 and two slabs of four quarters with treads on their
rim in `geometry/tests/test_helical_joins.cpp` (the rim merged into one
face; the slabs alone, a ring with a seam, as before); for 0690, mitcad#235
and the bar across a tube in `geometry/tests/test_fillet_results.cpp` (its
top edge rounded into the bore at the bore's seam, as fused and copied: the
volume and faces of the one with the seam turned away); for 0691 and 0692,
mitcad#235 and the disc whose recess a rib crosses in
`geometry/tests/test_fillet_results.cpp` (its rim rounded up to the rib: the
ends at the contact's crossings with the rib's tangent lines; with the edges
at the rib's roundings carried past their vertices within tolerance, the
result of the exact body);
for 0720 and 0721, mitcad#343 and the bar with a tilted rod standing out of
its end (the bar's edge rounded past the end's corner piece: valid, the
volume removed against the bar's and the rod's within the rounding's
corner) and the post with a rib under a chamfer cone (the rib's side
rounded along the cone just short of the cone's seam and turned away from
it, the post a cylinder or a spline: the same cone face and, on the
cylinder, the same volume removed) in
`geometry/tests/test_fillet_corner_cases.cpp`;
for 0700, mitcad#343 and the corner rounded over a cap of a ball standing
out of it in `geometry/tests/test_fillet_pierced.cpp` (the cap notching
the rounding, with and without the cap's edges with the wall rounded too,
and poking through it: the volume and the cap's area left against the
plain corner's rounding with the ball fused to it);
for 0740 and 0741, mitcad#415 and, in
`geometry/tests/test_fillet_results.cpp`, the bar across a ring of two
layers whose bore's wall is in three pieces round each corner (the bar's
top edges rounded: valid, the pieces kept apart, the volume removed that of
the wall in one face) and the disc whose recess a rib crosses with the
edges at the rib's roundings stopped short of their vertices (the result of
the exact body);
for 0742, mitcad#415 and the ring whose bore is threaded from below it to
above it, the thread's root rounded up to the ring's faces, in
`geometry/tests/test_sweeps.cpp` (valid, its vertices within 1e-3, the
volume added and each face's gain against the section the ball fills);
for 0750, mitcad#343 and the thread whose crest is rounded on both edges
between end planes in `geometry/tests/test_fillet_thread_crests.cpp`
(valid, the crest consumed, and a turn of thread more taking away the
section's volume swept round the axis at its centroid);
for 0770 and 0771, mitcad#343 and the block on a post as wide as the block
in `geometry/tests/test_fillet_lens_overflow.cpp` (the post's foot rounded:
valid, its quarters alike, the material added between the torus within 30
degrees of the block's axis and the whole torus; then the block's bottom
outline rounded: valid, the two points kept, its quarters alike);
for 0772, mitcad#343 and the bulge on a housing's rounded corner in the
same file (valid, the junctions at the height of the blend within 1e-4,
nothing beyond the first side above the top, the material added alike on
both sides of the corner's bisector; and with the two edges given in the
other order, the blend walked the other way);
for 0790, mitcad#422 and the post whose thread runs out along a straight
cut, its lower crest rounded down to the run-out, in
`geometry/tests/test_sweeps.cpp` (valid, one small face on the run-out's
flank plane where it cuts the rounding a turn back, the crest left sharp
from there down to the run-out against the helix's length);
for 0800, 0801 and 0803, mitcad#343 and, in
`geometry/tests/test_fillet_crest_corners.cpp`, the post with a rib under a
chamfer cone whose cylinder is trimmed two turns below its basis's
parameters (the rib's side rounded up to it: the cone's face and the
volume removed as on the plain post), the block of two halves whose top's
split edge has curves on the two halves 1.8e-3 apart (its top front edge
one contour through the split: valid, the volume of the edge's rounding)
and the shaft with a thread cut as a groove whose floor's and ceiling's
rims are circles 2 micrometres inside the shaft (a crest edge rounded up to
them: valid, the volume of the section along the edge);
for 0840, mitcad#434 and the plate whose top outline turns twice in
`geometry/tests/test_fillet_chain_loops.cpp` (the contour from a selected
edge takes the outline's four edges once each; Mitcad's fillet of three of
them ends; without the patch the test fails when the contour does not end
in 30 s);
for 0820, mitcad#343 and the same housing in
`geometry/tests/test_fillet_lens_overflow.cpp` (the blend one face per
edge: the first's with the cut by the first side, the second's with the
cuts by the corner's rounding and the second side, sharing an edge from
the first side's junction; in both orders of the edges);
for 0821, mitcad#343 and the key on a shaft in the same file (both edges
of the key's lens rounded: valid, the convex blend's contact ending on the
key's sides within 1e-3, no face left on the lens's plane, the material
added alike on both sides within 0.01 mm3, the two blends' faces as large
as the stored design's within 0.01 mm2);
for 0822, mitcad#343 and the key on a shaft whose side is split near the
key's other end, in the same file (that end's convex arc rounded, the
split at two heights: valid, the contacts' ends within 1e-3, no face left
on the lens's plane, a face past each split ending in a point at the
lens's end, the material removed alike on both sides within 1e-3 mm3 and
in all as the rounding's within 0.002);
for 0860 and 0861, mitcad#441 and the block whose top meets a wall along
three edges, the first between faces 0.05 radian apart, in
`geometry/tests/test_fillet_near_tangent_contour.cpp` (the contour started
from the second edge is one contour of the three, valid, with the volume of
the one started from the first; with the vertex between the first two edges
at a tolerance of 0.02, above the blend's half width there: valid, no part
of the second edge left);
for 0880, mitcad#343 and the ring's notch with a strip 5 µm wide at one
corner, in `geometry/tests/test_fillet_strip_start.cpp` (the strip's edge
with the notch rounded: valid, the strip gone, the rounding's face as large
as the sharp corner's within 1e-6 and the material removed as there within
1e-4, both corners at once as each alone);
for 0890, mitcad#343 and the wall cut from a half-cylinder's rounded end
whose foot is rounded, ending at a sliver the cut's end face leaves, in
`geometry/tests/test_fillet_end_overflow.cpp` (valid, the sliver consumed,
the volume the result without the sliver has, with the material beyond the
cut, within 1e-7);
for 0900, mitcad#343 and the plate with an overhanging tongue whose left
edge, rounded with 0.3, is rounded with 1 so that its last piece is walked
again past its OnSame end, in `geometry/tests/test_fillet_corners.cpp`
(every face of the result on a new surface is generated by the edge or
its vertices; 0901 to 0909 are verified on the stored design's bodies
only, see core/import/docs/f3d-rounds-3.md, "A bar's rim and notch walls rounded with
0.86");
for 0891, mitcad#343 and the B-spline disc whose rim is rounded over a
hole's rounded mouth, in the same file (valid, the volume of the disc
rounded without the hole less the hole within 1e-7, with the gap away from
the rim's vertex and across it on either side; it fails without the patch
with the gap across the vertex: the first two of the patch's three changes
are verified on the stored design's bodies only, as no synthetic body made
the projection miss its line's end or the intersection lose its last
stretch);
for 0870, mitcad#343 and the block whose top, 2 wide in the middle and 8 at
its ends, goes on tangentially into a rim rounded with 6, its front edge
rounded with 3, in `geometry/tests/test_fillet_tangent_overflow.cpp` (valid,
every point of the rounding's faces 3 from the ball's path on the front
wall and the top's plane, 2.7e-15 off; 0.171 off without the patch);
for 0920, mitcad#464 and a block's edge rounded with a B-spline radius, in
`geometry/tests/test_dressup.cpp` (`test_fillet_spline_radius`: the
material removed is 1 − π/4 times the integral of the radius squared along
the edge, and each cross-section misses 1 − π/4 times the radius there
squared, within 2e-3; without the patch the contour has no radius);
for 0911, mitcad#343 and a pin standing out of a wedge whose sides are
tangent to it, its foot rounded along its arc on the wedge's top, in
`geometry/tests/test_fillet_tangent_end_face.cpp` (valid, one torus, the
material added the ring at the foot within the sides' planes within 1e-6;
4.514 against 4.764 and invalid without the patch);
for 0930, mitcad#343 and a disc 2.5 thick, its rims' circles turning
opposite ways, rounded with 1.5 on both rims in either order, in
`geometry/tests/test_fillet_ride_against.cpp` (valid, the volume the
section's revolution has within 1e-6; invalid, 1322 against 3035, without
the patch);
for 0931, mitcad#343 and a roof whose ridge runs into a rod standing on
it, the rod's seam a B-spline along the line through the ridge's end, in
`geometry/tests/test_fillet_seam_run_on.cpp` (valid, the material removed
the blend's section along the ridge less the rod within 1e-6);
for 0933, mitcad#343 and a post on a plate, the edge round its foot broken
by two caps that poke through its rounding of 0.5, posts of radius 1 and
0.8, in `geometry/tests/test_fillet_pierced.cpp` (valid, the rounded post
with the caps fused within 2e-5 mm³, both caps' lenses; invalid without
the patch);
for 0940, mitcad#343 and a key in a spherical hollow, its top edge rounded
with 2 where the key's side runs out against the hollow's wall, in
`geometry/tests/test_fillet_narrow_cut.cpp` (valid, the material taken off
the corner outside the rounding's cylinder inside the hollow within 1e-6
mm³; the key whose side runs out before its ends builds without the patch
as well, the key cut off before that by end walls is invalid without it);
for 0941, mitcad#495 and a boss under a plate corner with a slanted side,
its foot chamfered past the corner, the boss's seam at 120, 150, 180 and
300 degrees, 13 and 15 long, in `geometry/tests/test_chamfer_overflow.cpp`
(valid, the boss with the chamfer's cone cut by the plate's outline
within 1e-8 relative; invalid without the patch for the seams from 120 to
180 degrees);
for 0942, mitcad#495 and a planar face of two wires whose arcs of one
circle meet end to end, in `geometry/tests/test_shapefix.cpp` (the repair
of intersecting wires returns with nothing to repair; it never returns
without the patch);
for 0950, mitcad#343 and five windows in a ring wall 3 thick, the pillars'
full-rounded ends in two halves each, both edges of every sill rounded
with 1.5, in `geometry/tests/test_fillet_consumed_strip.cpp` (valid, no
face of no area, the material taken within 2e-3 mm³ of the integral over
the sills' outlines; the fillet fails without the patch);
for 0951, mitcad#343 and a block's edge rounded with 0.3 up to where its
wall steps back by 0.0043, the step below and above the edge, one rim and
both, as built and mirrored, in `geometry/tests/test_fillet_step_end.cpp`
(valid, a quarter torus at each end, the material taken within 2e-6 mm³
of the integral; invalid without the patch);
for 0961, mitcad#343 and a tower on a slab flush with its side, the
tower's corner rounded with 0.4 and its foot there with 3, the side's edge
with the foot's torus rounded with 0.3 up to the vertex where the three
are tangent, the torus moved 0.0002 to 0.001 off the side within its
edges' tolerance, in `geometry/tests/test_fillet_vanishing_own_edge.cpp`
(valid, the blend ending in a point at the vertex, the vertices'
tolerances below 0.01, the material taken within 0.5 % of the exact
body's; the fillet fails without the patch);
for 0962, mitcad#343 and 0951's cases with the body's faces in the reverse
order, and the edge below the step rounded with 0.2 after the edge above
it with 0.3, in `geometry/tests/test_fillet_step_end.cpp` (the reversed
cases valid as the others; the second rounding's cylinder runs on past the
step up to the first's torus, with no torus of its own; the reversed cases
invalid or failing without the patch);
for 0970, mitcad#343 and a block's bottom outline rounded with 0.5 past
front corners rounded with 0.3, the front wall square to the bottom and
leaning 15 degrees over it, and the whole outline of a block with all four
corners rounded, in `geometry/tests/test_fillet_tight_corner.cpp` (valid,
the vertex blends on their exact surface within 1e-6 mm, the tori where
the wall leans, the material taken within 1e-6 mm³ of the integral where
the walls are square; without the patch the leaning wall's fillet fails,
and the square corners come out with other corner patches, 0.0065 mm off
the vertex blend);
for 0980, mitcad#343 and a box's top front edges rounded with 1 and 0.5
where the front turns outward by 3 and 5 degrees about a slanted crease
from the top edge, its faces B-splines, the edges given either way, in
`geometry/tests/test_fillet_concave_crease.cpp` (valid, no edge tolerance
above 1e-4, the material taken within 0.5 % of the two edges' sections
along them; the fillet fails at a vertex without the patch; the test
covers the turn over the crease, the other four changes are verified on
the design's body only, as no synthetic body reproduced the creases'
micrometre jumps at one section);
for 0990, mitcad#343 and a block's slot rounded with 9 along its
ceiling's long edges, a groove of radius 5 under its end wall through a
notch, the groove's rim on the wall's strip rounded with 10, in
`geometry/tests/test_fillet_rolled_end_cut.cpp` (valid, both notch walls
grown up to the cut above the groove's rim, the roundings of 9 grown past
the slot's wall, no face of no area; the fillet fails without the patch);
for 1000, mitcad#343 and a fin's tip, a line and a cubic turned up from it
by 2 and 4 degrees, chamfered 0.4 at 45 degrees along both, with the side
whole and with its foot chamfered so high that the side strip is narrower
than the chamfer along the cubic, the foot's face planar and a B-spline of
degree 1, in `geometry/tests/test_chamfer_crease_strip.cpp` (valid, the
material taken the section's triangle along both within 1e-4 where the
side is whole, every face inside its surface's bounds, the B-spline foot's
result the planar one's; without the patch the chamfer fails, without its
second change the narrow cases are invalid, without its third the foot's
face runs past its surface's bounds);
for 1010, mitcad#343 and a disc's rim of radius 85 in two arcs split where
a pocket from below breaks through its wall 1 under the top, chamfered 1
on the top and 2 down the wall, either arc given first and the seam at
either corner, and the rim as one closed edge, in
`geometry/tests/test_chamfer_rim_cutout.cpp` (valid, the material taken
within 1e-6 of the body's part outside the chamfer's revolved section, the
pocket's ceiling cut back to 84.5 from the axis; the two-arc chamfers fail
without the patch, the one-edge rim builds without it as well);
for 1020, mitcad#343 and a square peg drafted by 3, 4 and 5 degrees hanging
from a plate into a block whose walls go on from its sides at the creases
and whose vertical corners are rounded with 0.4, the peg's corner edges
(given either way along the corners) rounded with 0.5, in
`geometry/tests/test_fillet_crease_into_end_walls.cpp` (valid, the block's
corner cylinders from its foot up past the creases, the material taken
within 1e-3 mm³ of the peg rounded alone with the block carried on up into
it; with 0980 and without the patch the fillet fails, before 0980 it
rounds the block's corners with 0.5);
each fails without its patch but 0931's (on that body OCCT's topological
build closes the rod's face without it as well, so the test checks 0931's
way only; the design's failure is verified on its replayed body) and
0940's first two cases (the key with end walls fails without it), and 0802
has no such test: its contact meets the
spine's own edge only through the stored bodies' micrometre deviations at
the walk's steps there, which no synthetic body reproduced; nor has 0910: its
cap crosses the edge only through the contact curve's approximation lying
on the wrong side of the face at the end by 1.3e-6 mm, at a
corner where the blend is nearly tangent to that face, verified on the
design's replayed body; a bolt's thread cut by a chamfer cone and rounded
there failed elsewhere first; nor has 0932: its cut needs a contact
speeding up some twentyfold past the spine's end on a small torus, which
the synthetic posts of 0933's test did not reach, verified on the design's
replayed body, where the rounding with the rod's edges fails without it;
nor has 1001: its stretch needs the next edge's analytic piece to start a
hair past a vertex of tangent edges, as the design's faces split it, which
no synthetic body reproduced; verified on the design's replayed and stored
bodies, where the rounding fails with 0980 and without 1001);
nor has 1011: bodies with a bore's rim rounded where a pocket or a slot
cuts into it and the pocket's or slot's rim rounded up to that torus built
or failed alike with and without it, their contacts not running past the
torus' edge at its seam; verified on the design's replayed and stored
bodies, where the seam end's rounding gives a body of 1e12 mm³ without
it).
Sending them upstream is the
maintainers' step. When the
baseline moves to a newer OCCT, check which of the changes OCCT has made;
update the port from vcpkg's and apply the remaining patches again
(`git apply` in OCCT's source tree, in order), and compare the corpus
results as for any change to the geometry. Building OCCT from the port is
described in [development.md](../../docs/development.md#faster-builds).
