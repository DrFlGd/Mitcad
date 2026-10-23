# SPDX-License-Identifier: MIT
# Part workbench sweeps of sketches along a sketch (mitcad#543), each 40 mm
# above the one before:
#
# - Swept: a circle of radius 2 (named TubeRadius) swept along a quarter
#   of a circle of radius 20, a solid.
# - SweptFrenet: the same in the Frenet mode.
# - SweptSections: a circle of radius 2 at the arc's start and one of
#   radius 3 at its end, a solid through both.
# - SweptShell: the first circle swept along the arc as a shell (Solid
#   off).
#
# The arc runs in the XY plane from (0, 0) about (20, 0) to (20, -20),
# leaving the origin along -Y and reaching its end along +X; the circles
# lie square to it there. The sketches have fixed placements.
#
# The variant (part_sweep_changed) has TubeRadius 3 mm.
#
# Mitcad expects: no fallback
# Mitcad change: TubeRadius = 3 mm


def fixed_sketch(name, pos, axis=(0, 0, 1), angle=0.0):
    sketch = doc.addObject("Sketcher::SketchObject", name)
    sketch.Label = name
    sketch.Placement = place(pos, axis, angle)
    hide(sketch)
    return sketch


def spine(name, z):
    sketch = fixed_sketch(name, (0, 0, z))
    add(sketch, arc((20, 0), 20, 180, 270))
    return sketch


def ring(name, at, radius, normal_along_x=False, radius_name=None):
    """A circle about `at`, square to the arc: at its start the XZ plane
    (the XY plane turned 90 degrees about X: its normal -Y), at its end
    the YZ plane (turned 90 degrees about Y: its normal +X)."""
    if normal_along_x:
        sketch = fixed_sketch(name, at, (0, 1, 0), 90)
    else:
        sketch = fixed_sketch(name, at, (1, 0, 0), 90)
    rim = add(sketch, circle((0, 0), radius))
    constrain(sketch, "Coincident", rim, MID, H_AXIS, START)
    constrain(sketch, "Radius", rim, radius, name=radius_name)
    return sketch


def sweep(name, sections, path, solid=True, frenet=False):
    feature = named(doc.addObject("Part::Sweep", name), name)
    feature.Sections = sections
    feature.Spine = (path, ["Edge1"])
    feature.Solid = solid
    feature.Frenet = frenet
    return feature


tube = ring("SweptRing", (0, 0, 0), 2, radius_name="TubeRadius")
sweep("Swept", [tube], spine("SweptSpine", 0))
sweep("SweptFrenet", [ring("SweptFrenetRing", (0, 0, 40), 2)], spine("SweptFrenetSpine", 40), frenet=True)
sweep(
    "SweptSections",
    [ring("SweptSectionsStart", (0, 0, 80), 2), ring("SweptSectionsEnd", (20, -20, 80), 3, True)],
    spine("SweptSectionsSpine", 80),
)
sweep("SweptShell", [ring("SweptShellRing", (0, 0, 120), 2)], spine("SweptShellSpine", 120), solid=False)

doc.recompute()


def wider():
    tube.setDatum("TubeRadius", App.Units.Quantity("3 mm"))


variant(wider)
