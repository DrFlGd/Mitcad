# SPDX-License-Identifier: MIT
# Scaled transformations of the whole body (TransformMode "Transform body"
# in 1.0, "Whole shape" in 1.1; FreeCAD 0.21 has no such mode, the model is
# skipped there; mitcad#542), one per Body, each Body placed apart. The
# bodies are a 30 x 20 x 5 plate from the Body's origin with a boss off its
# middle, so that the scale's centre (the Body's origin, the body's centre
# of mass) shows in the result:
#
# - BodyScaled: the whole body scaled 1 and Grow (1.25) times.
# - BodyLinearScaled: the whole body 2 along the X axis over 20 mm, the
#   copy 1.25 times the size.
#
# The variant (pd_scaled_body_changed) has Grow 1.5.
#
# Mitcad expects: no fallback
# Mitcad change: Grow = 1.5

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


def plate_with_boss(container, name):
    plate = block(container, name + "Plate", 30, 20, 5)
    sketch = sketch_on_face(container, name + "BossSketch", plate, (0, 0, 1), 5)
    add(sketch, circle((8, 10), 3))
    pad(container, sketch, 4, name + "Boss")


sheet = spreadsheet("Spreadsheet", [("B1", "1.25", "Grow")])

scaled = body("BodyScaled")
plate_with_boss(scaled, "BodyScaled")
grown = multi(scaled, "BodyScaledCopies", [
    ("Scaled", {"Factor": 1.25, "Occurrences": 2}),
])
bind(grown.Transformations[0], "Factor", "Spreadsheet.Grow")

linear = body("BodyLinearScaled", place((0, 60, 0)))
plate_with_boss(linear, "BodyLinearScaled")
multi(linear, "BodyLinearScaledCopies", [
    ("LinearPattern", {"Direction": (origin_feature(linear, "X_Axis"), [""]), "Length": 20,
                       "Occurrences": 2}),
    ("Scaled", {"Factor": 1.25, "Occurrences": 2}),
])

doc.recompute()
for container in (scaled, linear):
    for obj in reversed(container.Group):
        if obj.TypeId == "PartDesign::MultiTransform":
            container.Tip = obj
            break
doc.recompute()


def changed():
    sheet.set("B1", "1.5")


variant(changed)
