# SPDX-License-Identifier: MIT
# The last rare Part extrusion forms (mitcad#548), the operands hidden as
# FreeCAD's commands hide them:
#
# - Cone: a conical Part::Helix (pitch 5, height 10, radius 4, 10 degrees)
#   extruded into faces 2 along the custom direction (0, 0, 1): a ribbon
#   wound round a cone, along the curve the file stores of the helix.
# - Mixed: a plain shape (Part::Feature) of a helix (pitch 3, height 6,
#   radius 3) and a straight edge beside it, extruded into faces 2 along
#   the custom direction (0, 0, 1).
# - Arch: a plain shape of a spline through (0, 0, 0), (10, 0, 3),
#   (10, 10, 0) and (20, 10, 4), in no plane, extruded into faces 3 along
#   the custom direction (0, 0, 1).
# - Rings: a Part::Face of a 20 x 20 square with a circle of radius 5 in
#   it made by FreeCAD's simple face maker (two overlapping faces, one
#   filling each wire), extruded Depth (a spreadsheet's alias, 3 mm) along
#   its normal into two overlapping solids.
#
# The variant (part_extrusion_rare_changed) has Depth 5 mm.
#
# Mitcad expects: no fallback
# Mitcad change: Depth = 5 mm

dims = spreadsheet("Dims", [("B1", "=3 mm", "Depth")])


def extrusion(name, base, solid, **options):
    feature = named(doc.addObject("Part::Extrusion", name), name)
    feature.Base = base
    feature.Solid = solid
    for key, value in options.items():
        setattr(feature, key, value)
    hide(base)
    return feature


def plain(name, shape):
    """A plain shape (Part::Feature) without a definition."""
    feature = named(doc.addObject("Part::Feature", name), name)
    feature.Shape = shape
    return feature


cone = named(doc.addObject("Part::Helix", "ConeHelix"), "ConeHelix")
cone.Pitch, cone.Height, cone.Radius, cone.Angle = 5, 10, 4, 10
extrusion("Cone", cone, False, DirMode="Custom", Dir=V(0, 0, 1), LengthFwd=2)

coil = Part.makeHelix(3, 6, 3)
coil.translate(V(30, 0, 0))
beside = Part.LineSegment(V(40, 0, 0), V(40, 8, 0)).toShape()
extrusion("Mixed", plain("MixedEdges", Part.Compound([coil, beside])), False, DirMode="Custom",
          Dir=V(0, 0, 1), LengthFwd=2)

spline = Part.BSplineCurve()
spline.interpolate([V(60, 0, 0), V(70, 0, 3), V(70, 10, 0), V(80, 10, 4)])
extrusion("Arch", plain("ArchSpline", spline.toShape()), False, DirMode="Custom", Dir=V(0, 0, 1),
          LengthFwd=3)

rings_sketch = doc.addObject("Sketcher::SketchObject", "RingsSketch")
rings_sketch.Label = "RingsSketch"
rings_sketch.Placement = place((0, 40, 0))
rectangle(rings_sketch, 0, 0, 20, 20)
hole = add(rings_sketch, circle((10, 10), 5))
constrain(rings_sketch, "Radius", hole, 5)
rings_face = named(doc.addObject("Part::Face", "RingsFace"), "RingsFace")
rings_face.Sources = [rings_sketch]
rings_face.FaceMakerClass = "Part::FaceMakerSimple"
hide(rings_sketch)
rings = extrusion("Rings", rings_face, True, DirMode="Normal", LengthFwd=3)
bind(rings, "LengthFwd", "Dims.Depth")
doc.recompute()


def deeper():
    dims.set("B1", "=5 mm")


variant(deeper)
