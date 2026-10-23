# SPDX-License-Identifier: MIT
# Tapered pads and pockets along directions off their sketches' normals
# (mitcad#539), one per Body; FreeCAD grows the profile in its plane by
# the length along the direction times the taper's tangent:
#
# - Sizes: Taper (=5 deg).
# - Custom: a 20 x 10 rectangle padded 10 mm along (1, 0, 2), the length
#   along the sketch's normal, tapered Taper.
# - Along: a 20 x 10 rectangle padded 10 mm along (0, 1, 2), the length
#   along the direction, narrowing by 4 degrees.
# - Round: a circle of radius 5 padded 12 mm along (1, 1, 3), tapered
#   Taper: an oblique cone.
# - Symmetric: a 20 x 10 rectangle padded 12 mm symmetric about its sketch
#   along (1, 1, 3), tapered 4 degrees both ways.
# - Pocketed: a 30 x 20 x 10 block with a circle of radius 4 pocketed 6 mm
#   along (1, 0, -2), tapered 3 degrees.
#
# The variant (pd_taper_direction_changed) has Taper 8 deg.
#
# Mitcad expects: no fallback
# Mitcad change: Taper = 8 deg

sheet = spreadsheet("Sizes", [("B1", "=5 deg", "Taper")])


def custom(name, x, y, direction, length, taper=None, along_normal=True, circle_radius=None, **options):
    container = body(name)
    sketch = sketch_on(container, name + "Sketch")
    if circle_radius is None:
        rectangle(sketch, x, y, 20, 10)
    else:
        add(sketch, circle((x, y), circle_radius))
    feature = pad(container, sketch, length, name + "Pad")
    feature.UseCustomVector = True
    feature.Direction = V(*direction)
    feature.AlongSketchNormal = along_normal
    if taper is None:
        feature.TaperAngle = 5
        bind(feature, "TaperAngle", "Sizes.Taper")
    else:
        feature.TaperAngle = taper
    for key, value in options.items():
        if key == "sides":
            sides(feature, value)
        else:
            setattr(feature, key, value)
    return feature


custom("Custom", 0, 0, (1, 0, 2), 10)
custom("Along", 40, 0, (0, 1, 2), 10, taper=-4, along_normal=False)
custom("Round", 85, 5, (1, 1, 3), 12, circle_radius=5)
custom("Symmetric", 120, 0, (1, 1, 3), 12, taper=4, sides="symmetric")

pocketed = body("Pocketed")
base = block(pocketed, "PocketedBase", 30, 20, 10, 0, 40)
sketch = sketch_on_face(pocketed, "PocketedSketch", base, (0, 0, 1), 10)
add(sketch, circle((15, 50), 4))
feature = pocket(pocketed, sketch, "PocketedPocket", length=6)
feature.UseCustomVector = True
feature.Direction = V(1, 0, -2)
feature.TaperAngle = 3

doc.recompute()


def steeper():
    sheet.set("B1", "=8 deg")


variant(steeper)
