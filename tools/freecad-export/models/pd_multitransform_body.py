# SPDX-License-Identifier: MIT
# MultiTransforms of the whole body (TransformMode "Transform body" in 1.0,
# "Whole shape" in 1.1; FreeCAD 0.21 has no such mode, the model is
# skipped there), which Mitcad replays as patterns and mirrors of the body
# joining their copies to it, each one copying the body with the copies
# before it (mitcad#539), one per Body:
#
# - WholeRow: a 30 x 20 x 5 plate with a boss, the whole body Count (3)
#   times along the X axis over 20 mm, then twice along the Y axis over
#   10 mm (every copy overlapping the plate before it).
# - WholeCross: a 40 x 6 x 5 bar (z from -2 to 3) from near the Body's Z
#   axis, the whole body 4 times round the Z axis (a cross), then mirrored
#   about the XY plane (the image overlapping it).
#
# The variant (pd_multitransform_body_changed) has Count 4.
#
# Mitcad expects: no fallback
# Mitcad change: Count = 4

if not at_least(1, 0):
    raise Unsupported("transformations of the whole body came with FreeCAD 1.0")


def multi(container, name, steps):
    """A MultiTransform of the whole body: no originals, the body mode."""
    feature = container.newObject("PartDesign::MultiTransform", name)
    feature.TransformMode = "Whole shape" if at_least(1, 1) else "Transform body"
    made = []
    for kind, options in steps:
        step = container.newObject("PartDesign::" + kind, name + kind + str(len(made)))
        for key, value in options.items():
            setattr(step, key, value)
        made.append(step)
    feature.Transformations = made
    return feature


def axis(container, role):
    return (origin_feature(container, role), [""])


sheet = spreadsheet("Spreadsheet", [("B1", "3", "Count")])

row = body("WholeRow")
plate = block(row, "WholeRowPlate", 30, 20, 5)
sketch = sketch_on_face(row, "WholeRowBossSketch", plate, (0, 0, 1), 5)
add(sketch, circle((5, 10), 3))
pad(row, sketch, 4, "WholeRowBoss")
copies = multi(row, "WholeRowCopies", [
    ("LinearPattern", {"Direction": axis(row, "X_Axis"), "Length": 20, "Occurrences": 3}),
    ("LinearPattern", {"Direction": axis(row, "Y_Axis"), "Length": 10, "Occurrences": 2}),
])
bind(copies.Transformations[0], "Occurrences", "Spreadsheet.Count")

cross = body("WholeCross", place((80, 0, 0)))
sketch = sketch_on(cross, "WholeCrossBarSketch", offset=place((0, 0, -2)))
rectangle(sketch, -5, -3, 40, 6)
pad(cross, sketch, 5, "WholeCrossBar")
multi(cross, "WholeCrossCopies", [
    ("PolarPattern", {"Axis": axis(cross, "Z_Axis"), "Angle": 360, "Occurrences": 4}),
    ("Mirrored", {"MirrorPlane": (origin_feature(cross, "XY_Plane"), [""])}),
])

doc.recompute()
for container in (row, cross):
    for obj in reversed(container.Group):
        if obj.TypeId == "PartDesign::MultiTransform":
            container.Tip = obj
            break
doc.recompute()


def changed():
    sheet.set("B1", "4")


variant(changed)
