# SPDX-License-Identifier: MIT
# A profile of more than eight faces (mitcad#547; AllowMultiFace, FreeCAD
# 1.0 and later): a 110 x 10 x 5 block with ten 6 x 6 bosses 4 high in a
# row on it (one pad of a sketch of ten squares, 11 mm apart), whose ten
# top faces are padded together by Rise (a spreadsheet's alias, 2 mm).
#
# The variant (pd_many_faces_changed) has Rise 3 mm.
#
# Mitcad expects: no fallback
# Mitcad change: Rise = 3 mm

if not at_least(1, 0):
    raise Unsupported("AllowMultiFace needs FreeCAD 1.0")

dims = spreadsheet("Dims", [("B1", "=2 mm", "Rise")])

container = body("ManyFaces")
base = block(container, "ManyFacesBlock", 110, 10, 5)
bosses = sketch_on_face(container, "ManyFacesBosses", base, (0, 0, 1), 5)
for i in range(10):
    rectangle(bosses, 2 + 11 * i, 2, 6, 6)
boss = pad(container, bosses, 4, "ManyFacesBoss")
doc.recompute()
tops = [
    "Face%d" % (i + 1)
    for i, face in enumerate(boss.Shape.Faces)
    if face.Surface.TypeId == "Part::GeomPlane"
    and (face.normalAt(0, 0) - V(0, 0, 1)).Length < 1e-9
    and abs(face.CenterOfMass.z - 9) < 1e-9
]
assert len(tops) == 10, tops
multi = container.newObject("PartDesign::Pad", "ManyFacesPad")
multi.AllowMultiFace = True
multi.Profile = (boss, tops)
bind(multi, "Length", "Dims.Rise")
doc.recompute()


def higher():
    dims.set("B1", "=3 mm")


variant(higher)
