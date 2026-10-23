# SPDX-License-Identifier: MIT
# More Part extrusions (mitcad#541), the operands hidden as FreeCAD's
# commands hide them:
#
# - Wedge: a Part::Face of a 30 x 20 rectangle extruded 8 along its normal
#   with a taper of 5 degrees (wider away from the face).
# - Waist: a Part::Face of a 16 x 12 rectangle extruded 6 forward with a
#   taper of 4 degrees and 3 back with -3 degrees.
# - Leaning: a Part::Face of a circle of radius 5 extruded 10 along the
#   custom direction (1, 0, 2), off its normal.
# - Ramp: a 20 x 10 rectangle sketch extruded 8 along the custom
#   direction (0, 1, 2) with a taper of Slope (a spreadsheet's alias,
#   3 degrees).
# - Twins: a Part::Face of two sketches (two squares apart: a face of two
#   bodies in Mitcad) extruded 5 along their normal.
# - Bend: a quarter arc of radius 10 extruded into faces 12 along Z (a
#   quarter of a cylinder), its curved face then extruded 5 along the
#   custom direction (1, 1, 0).
# - Rod: a Part::Circle of radius 4 (an edge only) extruded 6 along its
#   normal into a solid.
# - Prism: a Part::RegularPolygon (a triangle of circumradius 6, edges
#   only) extruded 7 along its normal into a solid.
# - Fin: a Part::Line extruded into faces 5 along the custom direction
#   (0, 0, 1).
#
# The variant (part_extrusion_more_changed) has Slope 6 degrees.
#
# Mitcad expects: no fallback
# Mitcad change: Slope = 6 deg

dims = spreadsheet("Dims", [("B1", "=3 deg", "Slope")])


def free_sketch(name, at=None):
    """A sketch outside a Body, placed by `at`."""
    sketch = doc.addObject("Sketcher::SketchObject", name)
    sketch.Label = name
    if at is not None:
        sketch.Placement = at
    return sketch


def extrusion(name, base, solid, **options):
    feature = named(doc.addObject("Part::Extrusion", name), name)
    feature.Base = base
    feature.Solid = solid
    for key, value in options.items():
        setattr(feature, key, value)
    hide(base)
    return feature


def face_of(name, sketches):
    feature = named(doc.addObject("Part::Face", name), name)
    feature.Sources = sketches
    for sketch in sketches:
        hide(sketch)
    return feature


wedge_sketch = free_sketch("WedgeSketch")
rectangle(wedge_sketch, 0, 0, 30, 20)
extrusion("Wedge", face_of("WedgeFace", [wedge_sketch]), True, DirMode="Normal", LengthFwd=8,
          TaperAngle=5)

waist_sketch = free_sketch("WaistSketch", place((50, 0, 0)))
rectangle(waist_sketch, 0, 0, 16, 12)
extrusion("Waist", face_of("WaistFace", [waist_sketch]), True, DirMode="Normal", LengthFwd=6,
          LengthRev=3, TaperAngle=4, TaperAngleRev=-3)

leaning_sketch = free_sketch("LeaningSketch", place((90, 0, 0)))
rim = add(leaning_sketch, circle((0, 0), 5))
constrain(leaning_sketch, "Radius", rim, 5)
extrusion("Leaning", face_of("LeaningFace", [leaning_sketch]), True, DirMode="Custom",
          Dir=V(1, 0, 2), LengthFwd=10)

ramp_sketch = free_sketch("RampSketch", place((0, 40, 0)))
rectangle(ramp_sketch, 0, 0, 20, 10)
ramp = extrusion("Ramp", ramp_sketch, True, DirMode="Custom", Dir=V(0, 1, 2), LengthFwd=8,
                 TaperAngle=3)
bind(ramp, "TaperAngle", "Dims.Slope")

left = free_sketch("TwinLeft", place((50, 40, 0)))
rectangle(left, 0, 0, 8, 8)
right = free_sketch("TwinRight", place((65, 40, 0)))
rectangle(right, 0, 0, 8, 8)
extrusion("Twins", face_of("TwinFaces", [left, right]), True, DirMode="Normal", LengthFwd=5)

arc_sketch = free_sketch("BendSketch", place((90, 40, 0)))
add(arc_sketch, arc((0, 0), 10, 0, 90))
wall = extrusion("BendWall", arc_sketch, False, DirMode="Custom", Dir=V(0, 0, 1), LengthFwd=12)
extrusion("Bend", wall, True, DirMode="Custom", Dir=V(1, 1, 0), LengthFwd=5)

ring = named(doc.addObject("Part::Circle", "RodCircle"), "RodCircle")
ring.Radius = 4
ring.Placement = place((0, 80, 0))
extrusion("Rod", ring, True, DirMode="Normal", LengthFwd=6)

triangle = named(doc.addObject("Part::RegularPolygon", "PrismPolygon"), "PrismPolygon")
triangle.Polygon = 3
triangle.Circumradius = 6
triangle.Placement = place((36, 84, 0))
extrusion("Prism", triangle, True, DirMode="Normal", LengthFwd=7)

edge = named(doc.addObject("Part::Line", "FinLine"), "FinLine")
edge.X1, edge.Y1, edge.Z1 = 60, 80, 0
edge.X2, edge.Y2, edge.Z2 = 75, 85, 0
extrusion("Fin", edge, False, DirMode="Custom", Dir=V(0, 0, 1), LengthFwd=5)
doc.recompute()


def steeper():
    dims.set("B1", "=6 deg")


variant(steeper)
