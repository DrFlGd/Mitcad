# SPDX-License-Identifier: MIT
# PartDesign features whose profile is a planar face of the Body's solid
# (Profile = (feature, Face<n>)), one per Body:
#
# - Dims: BlockHeight (=10 mm).
# - FacePad: a 30 x 20 block BlockHeight high, its top face padded 5 mm.
# - FacePocket: a 30 x 20 x 10 block (40 mm off) with a 10 x 10 boss 4 mm
#   high on it; the boss's top face pocketed 6 mm (the boss and 2 mm of the
#   block).
# - FaceTwoSides: a 30 x 20 x 10 block (80 mm off), its face at y = 0
#   padded 4 mm out of it and 2 mm into it (two lengths).
# - FaceRevolution: a Body placed 60 mm along y, a 20 x 10 x 10 block in
#   it from x = 10 to 30, its face at y = 0 (on the Body's XZ plane)
#   turned 90 degrees about the Body's Z axis.
#
# The variant (pd_profile_faces_changed) has BlockHeight 15 mm.
#
# Mitcad expects: no fallback
# Mitcad change: BlockHeight = 15 mm

dims = spreadsheet("Dims", [("B1", "=10 mm", "BlockHeight")])


def face_pad(container, name, feature, face, length, kind=None):
    """A Pad of a face of `feature` (`face`: its Face<n>)."""
    padded = container.newObject(kind or "PartDesign::Pad", name)
    padded.Profile = (feature, [face])
    padded.Length = length
    return padded


padded = body("FacePad")
base = block(padded, "FacePadBlock", 30, 20, 10)
bind(base, "Length", "Dims.BlockHeight")
doc.recompute()
face_pad(padded, "FacePadPad", base, face_index(base.Shape, (0, 0, 1)), 5)

pocketed = body("FacePocket")
base = block(pocketed, "FacePocketBlock", 30, 20, 10, 40, 0)
boss_sketch = sketch_on_face(pocketed, "FacePocketBossSketch", base, (0, 0, 1), 10)
rectangle(boss_sketch, 50, 5, 10, 10)
boss = pad(pocketed, boss_sketch, 4, "FacePocketBoss")
doc.recompute()
face_pad(pocketed, "FacePocketPocket", boss, face_index(boss.Shape, (0, 0, 1), 14), 6, "PartDesign::Pocket")

two = body("FaceTwoSides")
base = block(two, "FaceTwoSidesBlock", 30, 20, 10, 80, 0)
doc.recompute()
feature = face_pad(two, "FaceTwoSidesPad", base, face_index(base.Shape, (0, -1, 0)), 4)
sides(feature, "two")
feature.Length2 = 2

turned = body("FaceRevolution", place((0, 60, 0)))
base = block(turned, "FaceRevolutionBlock", 20, 10, 10, 10, 0)
doc.recompute()
# The block's shape is in the Body's coordinates (60 mm along y in the
# document's).
revolution = turned.newObject("PartDesign::Revolution", "FaceRevolutionTurn")
revolution.Profile = (base, [face_index(base.Shape, (0, -1, 0))])
revolution.ReferenceAxis = (origin_feature(turned, "Z_Axis"), [""])
revolution.Angle = 90

doc.recompute()
for container in (padded, pocketed, two, turned):
    container.Tip = container.Group[-1]
    for feature in container.Group:
        if feature.TypeId.startswith("PartDesign::") and not feature.isValid():
            raise Unsupported("%s fails in this version" % feature.Name)
doc.recompute()


def taller():
    dims.set("B1", "=15 mm")


variant(taller)
