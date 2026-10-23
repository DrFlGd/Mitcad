# SPDX-License-Identifier: MIT
# MultiTransforms with a Scaled transformation in other places than after
# one linear or polar pattern of as many occurrences (mitcad#539). FreeCAD
# scales the copies the transformations before it made (the first one's
# turning fastest) in as many runs as its occurrences, each copy about the
# first original's centre of mass carried with it; first (or alone) it
# makes that many scaled copies of the originals. Mitcad: a `scale_pattern`
# for a Scaled transformation first, else the `scale` (and `scale_groups`)
# of the pattern or mirror before it. One per Body, each Body placed apart:
#
# - ScaledAlone: a boss on a plate scaled 1, 1.25 and 1.5 times about its
#   centre (the larger copies reaching into the plate).
# - ScaledFirst: a boss scaled 1 and 1.5 times, then both 3 along the X
#   axis over 40 mm.
# - ScaledRuns: a boss 4 along the X axis over 45 mm, then scaled in two
#   runs of two copies: the first two as they are, the last two Grow (2)
#   times. FreeCAD 1.0 and 1.1 leave the first run's copy (the second
#   boss) out of their result, 0.21 keeps it: Mitcad's reading for 1.0
#   and 1.1 skips that copy (`skipped_copies`).
# - ScaledMirror: a boss mirrored about the YZ plane, the image 1.5 times
#   the size.
# - ScaledTwoLinear: a boss 3 along the X axis over 40 mm, then 2 along
#   the Y axis over 14 mm, then each of the 6 copies scaled on its own, 1
#   to 1.5 times.
#
# A Scaled transformation after two patterns whose runs hold copies of the
# first pattern's is in pd_scaled_inner.
#
# The variant (pd_scaled_general_changed) has Grow 2.5.
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

alone = body("ScaledAlone", place((0, 0, 0)))
plate = block(alone, "ScaledAlonePlate", 30, 20, 5)
original = boss(alone, plate, "ScaledAloneBoss", (15, 10), 3)
multi(alone, "ScaledAloneBosses", [original], [
    ("Scaled", {"Factor": 1.5, "Occurrences": 3}),
])
bodies.append(alone)

first = body("ScaledFirst", place((0, 40, 0)))
plate = block(first, "ScaledFirstPlate", 60, 20, 5)
original = boss(first, plate, "ScaledFirstBoss", (8, 10), 3)
multi(first, "ScaledFirstBosses", [original], [
    ("Scaled", {"Factor": 1.5, "Occurrences": 2}),
    ("LinearPattern", {"Direction": axis(first, "X_Axis"), "Length": 40, "Occurrences": 3}),
])
bodies.append(first)

runs = body("ScaledRuns", place((0, 80, 0)))
plate = block(runs, "ScaledRunsPlate", 70, 20, 5)
original = boss(runs, plate, "ScaledRunsBoss", (8, 10), 2.5)
grown = multi(runs, "ScaledRunsBosses", [original], [
    ("LinearPattern", {"Direction": axis(runs, "X_Axis"), "Length": 45, "Occurrences": 4}),
    ("Scaled", {"Factor": 2, "Occurrences": 2}),
])
bind(grown.Transformations[1], "Factor", "Spreadsheet.Grow")
bodies.append(runs)

mirror = body("ScaledMirror", place((120, 80, 0)))
plate = block(mirror, "ScaledMirrorPlate", 40, 20, 5, -20, 0)
original = boss(mirror, plate, "ScaledMirrorBoss", (10, 10), 3)
multi(mirror, "ScaledMirrorBosses", [original], [
    ("Mirrored", {"MirrorPlane": (origin_feature(mirror, "YZ_Plane"), [""])}),
    ("Scaled", {"Factor": 1.5, "Occurrences": 2}),
])
bodies.append(mirror)

linear = body("ScaledTwoLinear", place((0, 120, 0)))
plate = block(linear, "ScaledTwoLinearPlate", 60, 30, 5)
original = boss(linear, plate, "ScaledTwoLinearBoss", (8, 8), 2)
multi(linear, "ScaledTwoLinearBosses", [original], [
    ("LinearPattern", {"Direction": axis(linear, "X_Axis"), "Length": 40, "Occurrences": 3}),
    ("LinearPattern", {"Direction": axis(linear, "Y_Axis"), "Length": 14, "Occurrences": 2}),
    ("Scaled", {"Factor": 1.5, "Occurrences": 6}),
])
bodies.append(linear)

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
