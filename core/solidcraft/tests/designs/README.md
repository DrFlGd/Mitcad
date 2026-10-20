# Test designs of the `.solidcraft` import

Small SolidCraft designs written by hand for the reader's and the import's
tests (mitcad#450). None was saved by SolidCraft and none is a third-party
model; SolidCraft itself is not built or run for them. Each follows the
file format as SolidCraft's source defines it (`crates/doc/src/document.rs`,
`format.rs`, `crates/sketch/src/model.rs`, at the commit named in
`third_party/solidcraft/README.md`), and the values the tests expect
(timeline, parameters, constraint counts, body count, volume, bounding box)
are worked out from those definitions by hand, as the comments of the tests
show. The files are named `*.solidcraft.json`: plain JSON designs, which
SolidCraft reads as it reads `.solidcraft` files.

Real designs are never committed; the corpus tests read them from the
folders of `MITCAD_SOLIDCRAFT_CORPUS` (a path list, like `PATH`).

| File | What it holds |
|---|---|
| `box-fillet-hole.solidcraft.json` | Format 27: user parameters, a dimensioned rectangle, an extrude by a parameter, a fillet picked by a point on its edge, a circle cut through |
| `motion-links.solidcraft.json` | Format 27: six components placed by occurrences, five joints and three motion links between them: a rack and pinion sized by the pinion's picked round face, a gear pair, a cam on a sketch's circle with a knife follower (mitcad#466) |
| `emboss.solidcraft.json` | Format 27: a rectangle wrapped onto a rod's side and raised, another sunk (deboss), a square raised from the sketch plane on its end (mitcad#458) |
| `format-1.solidcraft.json` | Format 1: bare body colours, an unflipped joint, a component without an occurrence, the old XZ plane, a fillet's stored edge name; exercises every upgrade step that rewrites a design |
| `fillet-sweep-pattern-options.solidcraft.json` | Format 27 (mitcad#464): a fillet whose radius is a cubic B-spline along its edge, a sweep with an end scale given by a user parameter, a sweep between two guide rails, a body patterned along a chain of two sketch lines turning with it |
| `sweep-rail-plane.solidcraft.json` | Format 27 (mitcad#469): a circle swept along a line guided by an arc that bulges away from it, the rail's point where it crosses the plane square to the path |
| `sheet-contour-perforate.solidcraft.json` | Format 27: a sheet rule, a contour flange along two sketch lines by a parameter width, a square perforation field over it (mitcad#461) |
| `sheet-convert.solidcraft.json` | Format 27: a plate and an extruded L of thin walls converted to sheet metal, without and with a bend edge, their thickness taken from the bodies (mitcad#461) |
| `thin-extrude.solidcraft.json` | Format 27: thin extrudes along an open chain of lines (outside), an arc (centred) and a closed outline of lines (two sides), mitcad#463 |
| `bounding-solid.solidcraft.json` | Format 27: two boxes and three bounding solids: around both by a margin parameter, around one without a margin and without a body name, and around every body (mitcad#460) |
| `boundary-fill.solidcraft.json` | Format 27: two overlapping cubes, the cell in both as a new body and the cells in each alone as new bodies with the tools removed; a bar crossed by a wall, the bar's cell outside the wall (two blocks) cut from a plate, the feature's participants, with the tools removed (mitcad#457) |
| `mesh-convert.solidcraft.json` | Format 27: a mesh import of a plate with a 16-cornered round hole, converted to planes and a cylinder within the tolerance parameter `tol` (mitcad#462); `mesh-convert.tol005.expected.json` the same with `tol` 0.05 mm, the hole staying planes |
| `patch-stitch-thicken.solidcraft.json` | Format 27: five sketch squares patched into surfaces and stitched into an open box, its opening patched by edge picks and stitched on into a solid; a sheet thickened to one side, symmetrically and joined to a body by a negative thickness (mitcad#456) |
