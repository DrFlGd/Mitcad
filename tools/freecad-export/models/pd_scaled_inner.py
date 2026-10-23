# SPDX-License-Identifier: MIT
# A Scaled transformation after two patterns (mitcad#539): a boss on a disc
# 2 along the X axis over 8 mm, then 4 round the Z axis, then scaled 1 to
# 1.6 times by the turn (the two copies of a turn alike, each about its
# own centre). Mitcad: a circular pattern of the linear one with `scale`,
# which scales each copy of the linear pattern's copies about its own
# centre.
#
# FreeCAD 0.21 builds that. FreeCAD 1.0 and 1.1 leave the copies of the
# first run other than the original out of their result (here the linear
# pattern's copy before the turn: the stored shape has one boss less);
# Mitcad's reading for them has the linear pattern skip that copy
# (`skipped_copies`), which the circular pattern still copies (mitcad#542).
#
# Mitcad expects: no fallback


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


two = body("ScaledAfterTwo")
disc = sketch_on(two, "ScaledAfterTwoDiscSketch")
add(disc, circle((0, 0), 30))
disc_pad = pad(two, disc, 5, "ScaledAfterTwoDisc")
sketch = sketch_on_face(two, "ScaledAfterTwoBossSketch", disc_pad, (0, 0, 1), 5)
add(sketch, circle((10, 0), 2))
original = pad(two, sketch, 4, "ScaledAfterTwoBoss")
multi(two, "ScaledAfterTwoBosses", [original], [
    ("LinearPattern", {"Direction": axis(two, "X_Axis"), "Length": 8, "Occurrences": 2}),
    ("PolarPattern", {"Axis": axis(two, "Z_Axis"), "Angle": 360, "Occurrences": 4}),
    ("Scaled", {"Factor": 1.6, "Occurrences": 4}),
])

doc.recompute()
for obj in reversed(two.Group):
    if obj.TypeId == "PartDesign::MultiTransform":
        two.Tip = obj
        break
doc.recompute()
