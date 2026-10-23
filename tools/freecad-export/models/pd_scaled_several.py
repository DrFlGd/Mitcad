# SPDX-License-Identifier: MIT
# MultiTransforms with Scaled transformations the earlier models leave out
# (mitcad#542), one per Body, each Body placed apart:
#
# - ScaledThenLinear: a boss 4 along the X axis over 45 mm, scaled in two
#   runs (the last two Grow (2) times), then the row 2 along the Y axis
#   over 20 mm: a transformation after a Scaled one copies the scaled row.
# - TwoScaled: a boss 4 along the X axis over 45 mm, scaled in two runs
#   (the last two 1.5 times), then 2 along the Y axis over 20 mm, then
#   scaled in two runs again (the second row 1.2 times more): the factors
#   multiply.
# - MoreRuns: a boss on a disc turned half round the Z axis, then both 2
#   along the X axis over 8 mm, then scaled 1 to 1.6 times in four runs,
#   more than the linear pattern has elements (each of the four copies on
#   its own).
#
# FreeCAD 1.0 and 1.1 leave the copies of a Scaled transformation's first
# run other than the original out of their result (0.21 keeps them).
#
# The variant (pd_scaled_several_changed) has Grow 2.5.
#
# Mitcad expects: no fallback
# Mitcad change: Grow = 2.5


def multi(container, name, originals, steps):
    feature = container.newObject("PartDesign::MultiTransform", name)
    feature.Originals = originals
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


def boss(container, base, name, at, radius, top=5, height=4):
    sketch = sketch_on_face(container, name + "Sketch", base, (0, 0, 1), top)
    add(sketch, circle(at, radius))
    return pad(container, sketch, height, name)


sheet = spreadsheet("Spreadsheet", [("B1", "2", "Grow")])
bodies = []

then = body("ScaledThenLinear", place((0, 0, 0)))
plate = block(then, "ScaledThenLinearPlate", 70, 40, 5)
original = boss(then, plate, "ScaledThenLinearBoss", (8, 10), 2.5)
grown = multi(then, "ScaledThenLinearBosses", [original], [
    ("LinearPattern", {"Direction": axis(then, "X_Axis"), "Length": 45, "Occurrences": 4}),
    ("Scaled", {"Factor": 2, "Occurrences": 2}),
    ("LinearPattern", {"Direction": axis(then, "Y_Axis"), "Length": 20, "Occurrences": 2}),
])
bind(grown.Transformations[1], "Factor", "Spreadsheet.Grow")
bodies.append(then)

two = body("TwoScaled", place((0, 60, 0)))
plate = block(two, "TwoScaledPlate", 70, 40, 5)
original = boss(two, plate, "TwoScaledBoss", (8, 10), 2.5)
multi(two, "TwoScaledBosses", [original], [
    ("LinearPattern", {"Direction": axis(two, "X_Axis"), "Length": 45, "Occurrences": 4}),
    ("Scaled", {"Factor": 1.5, "Occurrences": 2}),
    ("LinearPattern", {"Direction": axis(two, "Y_Axis"), "Length": 20, "Occurrences": 2}),
    ("Scaled", {"Factor": 1.2, "Occurrences": 2}),
])
bodies.append(two)

runs = body("MoreRuns", place((120, 0, 0)))
disc = sketch_on(runs, "MoreRunsDiscSketch")
add(disc, circle((0, 0), 30))
disc_pad = pad(runs, disc, 5, "MoreRunsDisc")
original = boss(runs, disc_pad, "MoreRunsBoss", (10, 0), 2)
multi(runs, "MoreRunsBosses", [original], [
    ("PolarPattern", {"Axis": axis(runs, "Z_Axis"), "Angle": 360, "Occurrences": 2}),
    ("LinearPattern", {"Direction": axis(runs, "X_Axis"), "Length": 8, "Occurrences": 2}),
    ("Scaled", {"Factor": 1.6, "Occurrences": 4}),
])
bodies.append(runs)

doc.recompute()
for container in bodies:
    for obj in reversed(container.Group):
        if obj.TypeId == "PartDesign::MultiTransform":
            container.Tip = obj
            break
doc.recompute()


def changed():
    sheet.set("B1", "2.5")


variant(changed)
