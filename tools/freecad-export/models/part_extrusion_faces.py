# SPDX-License-Identifier: MIT
# Part extrusions of faces and into faces (mitcad#539), the operands
# hidden as FreeCAD's commands hide them:
#
# - Fence: an open polyline sketch turned upright (its normal along -Y)
#   extruded into faces (Solid off) 15 mm along its normal: walls without
#   caps.
# - Tube: a closed rectangle sketch extruded into faces along a custom
#   direction (0, 0, 1), 6 mm forward and 4 mm back.
# - Plate: a Part::Face of a sketch of a 30 x 20 rectangle with a circular
#   hole of radius 4 (the face keeps the hole).
# - Slab: Plate extruded Depth (a spreadsheet's alias, 10 mm) along its
#   normal into a solid.
# - Sheet: Plate extruded with Solid off (FreeCAD extrudes a face into a
#   solid all the same) along a custom direction (0, 0, -1), 6 mm forward
#   and 2 mm back, reversed.
# - Disc: a Part::Face of a circle sketch of radius 6 tilted 30 degrees
#   about Y; Bar: Disc extruded 12 mm symmetric about it along its normal.
#
# Plate's face is extruded twice: the first extrusion takes a copy of it.
#
# The variant (part_extrusion_faces_changed) has Depth 14 mm.
#
# Mitcad expects: no fallback
# Mitcad change: Depth = 14 mm

dims = spreadsheet("Dims", [("B1", "=10 mm", "Depth")])


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
    return feature


def face_of(name, sketch):
    feature = named(doc.addObject("Part::Face", name), name)
    feature.Sources = [sketch]
    hide(sketch)
    return feature


fence_sketch = free_sketch("FenceSketch", place((0, 60, 0), (1, 0, 0), 90))
polyline(fence_sketch, [(0, 0), (10, 0), (15, 8), (25, 8)])
extrusion("Fence", fence_sketch, False, DirMode="Normal", LengthFwd=15)
hide(fence_sketch)

tube_sketch = free_sketch("TubeSketch", place((40, 0, 0)))
rectangle(tube_sketch, 0, 0, 12, 8)
extrusion("Tube", tube_sketch, False, DirMode="Custom", Dir=V(0, 0, 1), LengthFwd=6, LengthRev=4)
hide(tube_sketch)

plate_sketch = free_sketch("PlateSketch", place((0, 0, 30)))
rectangle(plate_sketch, 0, 0, 30, 20)
hole = add(plate_sketch, circle((15, 10), 4))
constrain(plate_sketch, "Radius", hole, 4)
plate = face_of("Plate", plate_sketch)
slab = extrusion("Slab", plate, True, DirMode="Normal", LengthFwd=10)
bind(slab, "LengthFwd", "Dims.Depth")
extrusion("Sheet", plate, False, DirMode="Custom", Dir=V(0, 0, -1), LengthFwd=6, LengthRev=2, Reversed=True)
hide(plate)

disc_sketch = free_sketch("DiscSketch", place((80, 0, 0), (0, 1, 0), 30))
rim = add(disc_sketch, circle((0, 0), 6))
constrain(disc_sketch, "Radius", rim, 6)
disc = face_of("Disc", disc_sketch)
extrusion("Bar", disc, True, DirMode="Normal", LengthFwd=12, Symmetric=True)
hide(disc)
doc.recompute()


def deeper():
    dims.set("B1", "=14 mm")


variant(deeper)
