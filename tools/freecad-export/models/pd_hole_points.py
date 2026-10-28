# SPDX-License-Identifier: MIT
# A hole at sketch points, then a fillet on its edges (mitcad#835), in a
# 50 x 30 x 15 block from a sketch on its top face:
#
# - Points: two coincident points at (15, 10), joined by a coincident
#   constraint, and a construction point at (35, 20) that FreeCAD does not
#   drill (its sketch shape leaves construction geometry out).
# - PointHole: diameter 9 through all, an angled (118 degree) drill point,
#   linked to the whole sketch.
# - PointFillet: radius 1 on the hole's two circular edges.
#
# Holes at sketch points are checked from FreeCAD 1.0.
#
# Mitcad expects: no fallback

if not at_least(1, 0):
    raise Unsupported("holes at sketch points are checked from FreeCAD 1.0")


def hole_circle(middle, edge):
    radius = getattr(edge.Curve, "Radius", None)
    return radius is not None and abs(radius - 4.5) < 1e-9


container = body("PointHoles")
base = block(container, "PointBase", 50, 30, 15)
sketch = sketch_on_face(container, "Points", base, (0, 0, 1), 15)
first = add(sketch, point((15, 10)))
second = add(sketch, point((15, 10)))
constrain(sketch, "Coincident", first, 1, second, 1)
add(sketch, point((35, 20)), construction=True)
feature = container.newObject("PartDesign::Hole", "PointHole")
feature.Profile = sketch
# The hole's options read the sketch's shape.
doc.recompute()
feature.Diameter = 9
feature.DepthType = "ThroughAll"
feature.DrillPoint = "Angled"
feature.DrillPointAngle = 118
sketch.Visibility = False
doc.recompute()
fillet = container.newObject("PartDesign::Fillet", "PointFillet")
fillet.Base = (feature, edges_where(feature.Shape, hole_circle))
fillet.Radius = 1
