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
sequential mode's results (mitcad#203). Patch 0060 changes no result
either: it removes repeated work that made ShapeFix's repair of
intersecting edges quadratic in the wires of a face (mitcad#60). Patches 0090 and 0091 fix a heap overflow of
OCCT's chamfer and fillet (mitcad#160).

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

Each patch has a report for OCCT, with a reproduction that uses OCCT alone
and measurements: mitcad#84 (0006), mitcad#90 (0007), mitcad#91 (0008) and
mitcad#92 (0009); for 0010 to 0019 and 0050 and later, mitcad#121 and the tests in
`geometry/tests/test_fillet_overflow.cpp` (for 0075 to 0077 also
`geometry/tests/test_fillet_rolling_ball.cpp`), which use OCCT's fillet alone
where they can; for 0020 to 0023, mitcad#122 and
`geometry/tests/test_fillet_kernel.cpp`; for 0024 to 0026, mitcad#133
and `geometry/tests/test_fillet_ends.cpp`; for 0030 to 0032, mitcad#132
and `geometry/tests/test_allocation.cpp`; for 0033, mitcad#203 and the
helix cases of `geometry/tests/test_sweeps.cpp`; for 0060, mitcad#60 and
`geometry/tests/test_shapefix.cpp`, whose expected results are OCCT's
without the patch; for 0090 and 0091, mitcad#160 and
`geometry/tests/test_chamfer_kernel.cpp`, which aborts without them. Sending them upstream is the
maintainers' step. When the
baseline moves to a newer OCCT, check which of the changes OCCT has made;
update the port from vcpkg's and apply the remaining patches again
(`git apply` in OCCT's source tree, in order), and compare the corpus
results as for any change to the geometry. Building OCCT from the port is
described in [development.md](../../docs/development.md#faster-builds).
