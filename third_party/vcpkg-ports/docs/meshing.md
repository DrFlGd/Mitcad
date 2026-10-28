# Meshing: 1070

A patch to the triangulation BRepMesh takes a plane's base triangulation
from with its Delabella algorithms. Back to the
[port's README](../README.md).

## Overview

- **1070** makes Delabella's in-circle and orientation signs exact, so
  that nearly cocircular boundaries no longer crash it or loop it forever,
  and ends a hull that still comes out inconsistent as a failed
  triangulation (mitcad#846).

## 1070: Delabella on nearly cocircular points

### 1070-brepmesh-delabella-exact-signs

Delabella (`delabella.cpp`, the sweep-hull Delaunay triangulation of
`BRepMesh_DelabellaBaseMeshAlgo`) lifts a plane's boundary points, with
four corners of their enlarged box, onto the paraboloid z = x² + y² and
builds their convex hull a point at a time: the faces the new point sees
are deleted, and new faces join the point to the silhouette of the deleted
ones. MitCad's view meshes planes through it in the interactive tier and,
checked, in `mitcad-cli mesh-compare`'s `checked` variant (mitcad#635,
mitcad#644).

Whether a face sees a point is the sign of an in-circle determinant, which
Delabella computed in `long double`: 64 bits of mantissa with GCC and
Clang on x86-64, 53 with MSVC and on ARM. A building's plate, a square of
1 650 mm with a concave arc of 1 200 mm radius cut from a corner, never
finished meshing on Windows: on the arc's nearly cocircular points the
rounded signs contradicted each other, and the faces a point sees were no
longer one disk (a vertex twice on the silhouette, an unseen face inside
it). The silhouette then had fewer edges than the new faces need, the face
taken as the last one added had not been written in that step, and the
walk round the silhouette followed `next` and `sew` links left from earlier
steps or never set: a null or stray pointer, or a loop that never came back
to its start (`while (pr != entry)`). With Delabella's `long double` as
`double`, the plate's points crashed it in three of the plate's four
copies; with 64 bits, a sweep of the radius from 400 to 1 550 mm at
0.01 mm deviation crashed on 12 of 230 radii and looped on 2, through
BRepMesh alone and through MitCad's meshing alike.

- `incircleSign` and `orientSign` give the determinants' signs exactly: the
  floating-point value when it exceeds Shewchuk's error bound for it
  (doubled, for fused multiply-adds), else the exact value as an expansion
  of non-overlapping doubles (J. R. Shewchuk, "Adaptive Precision
  Floating-Point Arithmetic and Fast Robust Geometric Predicates", 1997),
  with `std::fma` for the products' errors. `Face::dot` is the in-circle
  sign of the input coordinates (the lifted z no longer rounded), and the
  z of `Face::cross` the orientation's sign, which picks the Delaunay
  faces. With exact signs every point is a vertex of the lifted hull and
  the faces it sees are a disk.
- Guards, for a hull that still comes out inconsistent: a silhouette vertex
  met twice in one step, new faces other than the deleted ones plus two, a
  silhouette that is not one loop through every new face's first vertex
  (checked before anything is sewn), a missing back link, and no face that
  sees a point end the triangulation with an `[ERR]` message and 0, as
  when out of memory. The walk round the last point is bounded by the
  number of faces, the fall-back search runs over the faces on the hull,
  and vertices and faces are allocated zeroed.
- All points on one circle: the point retracted to become the cone's top is
  unlinked from the start's contour (the cone took more faces than there
  are; BRepMesh's box corners keep its input from this case).
- `BRepMesh_DelabellaBaseMeshAlgo::buildBaseTriangulation` throws
  `Standard_Failure` when Delabella returns 0: the face fails (MitCad's
  checked planes then triangulate it by the default insertion) instead of
  going on without a base triangulation.

## Reports and tests

- 1070: mitcad#846 and `test_nearly_cocircular_planes` in
  `geometry/io/tests/test_io.cpp`: the plate with discs of 1 200 mm and
  three radii that crashed or looped on Linux, meshed by both of the
  view's Delabella tiers, each plane's triangles covering its boundary and
  none made again. Without the patch the test crashes on Linux at the
  second radius. The patch's own validation (not committed): random,
  integer-grid and arc point sets, each triangulation checked for its area
  against the convex hull's and for empty circumcircles, with 64- and
  53-bit `long double`.
