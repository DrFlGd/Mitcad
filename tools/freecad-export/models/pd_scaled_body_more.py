# SPDX-License-Identifier: MIT
# More Scaled transformations of the whole body (mitcad#547; TransformMode
# "Transform body" in 1.0, "Whole shape" in 1.1; FreeCAD 0.21 has no such
# mode, the model is skipped there), one per Body, each Body placed apart.
# The bodies are a 30 x 20 x 5 plate from the Body's origin with a boss off
# its middle; FreeCAD scales each copy about the Body's origin carried
# with it. Mitcad: a pattern of a pattern of the body, each joining its
# copies to it.
#
# - BodyTwoSteps: the whole body 2 along the X axis over 20 mm, then 2
#   along the Y axis over 10 mm, then each of the four copies scaled on its
#   own, 1 to Grow (1.25) times (more runs than the second pattern has
#   elements: both patterns scale their copies).
# - BodyRuns: the whole body 4 along the X axis over 30 mm, then scaled in
#   two runs, the last two copies 1.25 times. FreeCAD 1.0 and 1.1 leave the
#   first run's copy (the second body, its boss showing) out of their
#   result: Mitcad's pattern of the body skips that copy.
#
# The variant (pd_scaled_body_more_changed) has Grow 1.5.
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


def axis(container, role):
    return (origin_feature(container, role), [""])


sheet = spreadsheet("Spreadsheet", [("B1", "1.25", "Grow")])

two = body("BodyTwoSteps")
plate_with_boss(two, "BodyTwoSteps")
grown = multi(two, "BodyTwoStepsCopies", [
    ("LinearPattern", {"Direction": axis(two, "X_Axis"), "Length": 20, "Occurrences": 2}),
    ("LinearPattern", {"Direction": axis(two, "Y_Axis"), "Length": 10, "Occurrences": 2}),
    ("Scaled", {"Factor": 1.25, "Occurrences": 4}),
])
bind(grown.Transformations[2], "Factor", "Spreadsheet.Grow")

runs = body("BodyRuns", place((0, 60, 0)))
plate_with_boss(runs, "BodyRuns")
multi(runs, "BodyRunsCopies", [
    ("LinearPattern", {"Direction": axis(runs, "X_Axis"), "Length": 30, "Occurrences": 4}),
    ("Scaled", {"Factor": 1.25, "Occurrences": 2}),
])

doc.recompute()
for container in (two, runs):
    for obj in reversed(container.Group):
        if obj.TypeId == "PartDesign::MultiTransform":
            container.Tip = obj
            break
doc.recompute()


def changed():
    sheet.set("B1", "1.5")


variant(changed)
