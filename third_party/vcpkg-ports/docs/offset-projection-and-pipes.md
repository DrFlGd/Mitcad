# Offset, projection and pipes: 0100, 0960, 1030

OCCT's offset (`TKOffset`), the projection of a curve onto a surface
(`ProjLib`) and the pipe shell's guide trihedron (`GeomFill`). Back to the
[port's README](../README.md).

## Overview

- **0100** drops a face whose offset vanishes within the offset distance
  and intersects the offsets of the faces on both sides of it instead
  (mitcad#172).
- **0960** stops the projection's boundary search once its bracket's
  endpoints are adjacent floating-point values, where it tried the same
  projection for ever (mitcad#504).
- **1030** corrects the second derivative of the binormal of the guide
  trihedron by curvilinear equivalence (`GeomFill_GuideTrihedronAC::D2`),
  which bent a pipe shell's surfaces along an auxiliary spine between its
  sections (mitcad#539).

## 0100: faces vanishing within the offset

The patches 0100 and later change OCCT's offset (`TKOffset`, mitcad#172);
`geometry/tests/test_shell_vanishing.cpp` rebuilds the cases with exact
volumes, partly with OCCT alone.

### 0100-brepoffset-faces-vanishing-within-the-offset

A face whose offset vanishes within the offset distance (offset inside: a
chamfer strip narrower than what the offsets of the faces beside it take
from it, a convex rounding of a radius smaller than the wall; outside, the
same concave) is dropped, and the offsets of the faces on both sides of it
are intersected with each other instead, as if they were neighbours
(`FindVanishingFaces`, `BRepOffset_Inter3d::ConnexIntAcrossVanished` and
`ConnexIntByArcAcrossVanished`); the walks round the faces' wires skip its
edges (`BRepOffset_Inter2d::ConnexIntByInt`). It finds faces of four edges
whose offset turns inside out between two opposite edges all along them,
also several in a row (a strip and the cones at its corners); a face that
vanishes only in parts stays, and with the arc join one with a tube along an
edge. Before, the shell failed (the thick solid had fewer faces than the
body), a closed offset had no solid, or the solid was wrong: the inverted
rounding kept, or with the arc join the body itself, unshelled. A thick
solid to which the offset contributes no face is no longer done. The private
methods `BuildOffsetByArc`, `BuildOffsetByInter`, `Intersection3D`,
`IntersectEdges` and `ToContext` of `BRepOffset_MakeOffset` and three
methods of `BRepOffset_Inter2d` and `BRepOffset_Inter3d` get parameters; no
class gets members.

## 0960: the projection's boundary search

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

### 0960-projlib-projection-boundary-bisection-stops-at-adjacent-parameters

The boundary search bisects between a curve parameter that projects and one
that does not. Its stopping tolerance is the UV tolerance divided by the
projected curve's derivative; it can be smaller than the spacing of the
parameters' floating-point values. Once the bracket's endpoints are adjacent
values, their midpoint is one of the endpoints and the same projection is
tried for ever. Stop then, retaining the last successfully projected point.
The projection's tolerances and success criteria stay as they were; no
failed point is accepted and no iteration limit cuts a search short while it
can still make progress. The OCCT-only test `geometry.projection_boundary`
projects a straight B-spline segment with knots from 1e6 to 1e6 + 1e-6 onto
a bounded plane, where its crossings have no representable parameters.
Without 0960 the constructor hangs in `DichExactBound`. The test checks that
the boundary points project correctly to 1e-6 mm and that their adjacent
parameters lie outside the plane's bounds, instead of demanding a curve
point at an unrepresentable parameter. A segment with the same knot span
near 1 is the control.

## 1030: the guide trihedron's second derivative

The patch 1030 comes from mitcad#539 (the FreeCAD import's pipes along an
auxiliary spine): an 8 x 2 bar swept 50 along a line and turned a quarter
turn by an auxiliary spine 10 above it, crossing from one side to the
other, the spine's points at the same fraction of the lengths as the
line's (the sweep's auxiliary `frame` with `curvilinear`; the reference
model `pd_pipe_modes`, whose stored pipe holds 800 mm³: every section is
the bar).

### 1030-geomfill-guide-trihedron-ac-second-derivative-of-the-binormal

The trihedron's values and first derivatives were right (checked against
differences of D0 and D1), and so were the pipe's sections (`Simulate`) and
its surfaces at the approximation's sections, but its side faces lay up to
0.27 off the turned bar between them: the pipe held 784.9 mm³ (775.9 at a
tolerance of 1e-7). `D2` took the binormal's second derivative as a unit
vector's with the square of the binormal itself (1) where the square of its
first derivative belongs, which added `-(1 - |d|^2) b`; the binormal's
second derivative was off by about the binormal. With the square of the
first derivative (as the normal's second derivative has it) the pipe holds
800 mm³, as by the plane rule.

## Reports and tests

- 0100: mitcad#172 and `geometry/tests/test_shell_vanishing.cpp`, which
  rebuilds the cases with exact volumes, partly with OCCT alone.
- 0960: mitcad#504 and the OCCT-only test `geometry.projection_boundary`
  (described with the patch above).
