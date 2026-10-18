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
and 0680 merges a ring of cylinder pieces cut through from
end to end, which reaches round the whole period without winding round it,
into one face without a seam (a spiral stair's slabs, treads and helical
stringers, mitcad#400).

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
`core/import/README.md`, *Fillets that fail along their edges*).

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
`core/import/README.md`, *Fillets that cannot end or join at a vertex*).

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
after mitcad#323 (mitcad#339, `core/import/README.md`, *Fillet corners
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
face; the slabs alone, a ring with a seam, as before);
each fails without its patch.
Sending them upstream is the
maintainers' step. When the
baseline moves to a newer OCCT, check which of the changes OCCT has made;
update the port from vcpkg's and apply the remaining patches again
(`git apply` in OCCT's source tree, in order), and compare the corpus
results as for any change to the geometry. Building OCCT from the port is
described in [development.md](../../docs/development.md#faster-builds).
