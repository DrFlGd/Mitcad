# SPDX-License-Identifier: MIT
# Grooves and revolutions by their types beyond an angle (mitcad#539), one
# per Body (FreeCAD 1.0 and later: the types are new in 1.0):
#
# - GrooveThrough: a 60 x 30 x 10 block centred on the Z axis with a
#   ring groove of the type ThroughAll about the Z axis, its section 4 wide
#   (named GrooveWidth) and 3 high, 20 mm from the axis, 3 mm up, and an
#   Angle of 120. FreeCAD 1.1 turns it a full turn; 1.0 turns it by its
#   angle whatever its type.
# - RevolutionReversed: a block in the third quadrant, its face at x = 0
#   facing +X, and a rectangle on the XZ plane turned about the Z axis the
#   other way (Reversed) up to that face: a quarter turn.
#
# (FreeCAD computes no revolution up to the first or the last face and no
# groove up to the first face: "not yet supported" in 1.0 and 1.1, so these
# cannot be reference models. Mitcad translates a revolution up to the last
# face as one through the Body's body, see core/import-fcstd/docs/freecad.md.)
#
# The sketches have fixed placements in their Bodies (no attachment), the
# same in every FreeCAD version.
#
# The variant (pd_revolution_upto_changed) has GrooveWidth 6 mm.
#
# Mitcad expects: no fallback
# Mitcad change: GrooveWidth = 6 mm

if not at_least(1, 0):
    raise Unsupported("the revolutions' and grooves' types are new in FreeCAD 1.0")


def upright_sketch(container, name):
    """An empty sketch on the XZ plane at a fixed placement (the XY plane
    turned 90 degrees about X: its x along X, its y along Z)."""
    sketch = container.newObject("Sketcher::SketchObject", name)
    sketch.Label = name
    sketch.MapMode = "Deactivated"
    sketch.Placement = place((0, 0, 0), (1, 0, 0), 90)
    return sketch


def placed_rectangle(sketch, x, y, width, height, width_name=None):
    """A fully constrained rectangle from (x, y); its width named
    `width_name` when given."""
    lines = rectangle(sketch, x, y, width, height)
    constrain(sketch, "Horizontal", lines[0])
    constrain(sketch, "Horizontal", lines[2])
    constrain(sketch, "Vertical", lines[1])
    constrain(sketch, "Vertical", lines[3])
    constrain(sketch, "DistanceX", H_AXIS, START, lines[0], START, x)
    constrain(sketch, "DistanceY", H_AXIS, START, lines[0], START, y)
    constrain(sketch, "DistanceX", lines[0], START, lines[0], END, width, name=width_name)
    constrain(sketch, "DistanceY", lines[1], START, lines[1], END, height)
    return lines


grooved = body("GrooveThrough")
block(grooved, "GrooveThroughBlock", 60, 30, 10, -30, -15)
section = upright_sketch(grooved, "GrooveThroughSketch")
placed_rectangle(section, 20, 3, 4, 3, "GrooveWidth")
groove = grooved.newObject("PartDesign::Groove", "GrooveThroughGroove")
groove.Profile = section
groove.ReferenceAxis = (section, ["V_Axis"])
groove.Angle = 120
groove.Type = "ThroughAll"
hide(section)

reversed_body = body("RevolutionReversed", place((100, 0, 0)))
stop = block(reversed_body, "RevolutionReversedStop", 20, 20, 10, -20, -20)
doc.recompute()
profile = upright_sketch(reversed_body, "RevolutionReversedSketch")
placed_rectangle(profile, 5, 0, 3, 5)
turned = reversed_body.newObject("PartDesign::Revolution", "RevolutionReversedTurn")
turned.Profile = profile
turned.ReferenceAxis = (origin_feature(reversed_body, "Z_Axis"), [""])
turned.Type = "UpToFace"
turned.UpToFace = (stop, [face_index(stop.Shape, (1, 0, 0))])
turned.Reversed = True
hide(profile)

doc.recompute()
for container in (grooved, reversed_body):
    container.Tip = container.Group[-1]


def wider():
    sketch = doc.getObject("GrooveThroughSketch")
    sketch.setDatum("GrooveWidth", App.Units.Quantity("6 mm"))


variant(wider)
