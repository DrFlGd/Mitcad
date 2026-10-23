# SPDX-License-Identifier: MIT
# PartDesign pipes through several sections in the Fixed mode
# (mitcad#547): the sections do not turn, they only move along the spine.
# One per Body (the Bodies 100 mm apart along Y):
#
# - Fixed: a level circle of radius 2 (named SectionRadius) at the start
#   of an eighth of a circle of radius 30 rising from the origin in the XZ
#   plane (about (-30, 0, 0), leaving the origin along +Z) and a level
#   circle of radius 3 at its end, a pipe through both (Transformation
#   Multisection, Mode Fixed).
# - FixedThree: the same arc through three level circles, of radius 2 at
#   its start, 3 halfway (a sixteenth of the circle up) and 2 at its end.
#
# The sketches have fixed placements in their Bodies, level (in planes
# parallel to XY) where the arc passes.
#
# The variant (pd_pipe_fixed_sections_changed) has SectionRadius 2.5 mm.
#
# Mitcad expects: no fallback
# Mitcad change: SectionRadius = 2.5 mm


def fixed_sketch(container, name, pos):
    sketch = container.newObject("Sketcher::SketchObject", name)
    sketch.Label = name
    sketch.MapMode = "Deactivated"
    sketch.Placement = place(pos)
    hide(sketch)
    return sketch


def on_arc(degrees):
    """The point of the arc `degrees` up from the origin."""
    a = math.radians(degrees)
    return (30 * math.cos(a) - 30, 0, 30 * math.sin(a))


def ring(container, name, degrees, radius, radius_name=None):
    sketch = fixed_sketch(container, name, on_arc(degrees))
    rim = add(sketch, circle((0, 0), radius))
    constrain(sketch, "Coincident", rim, MID, H_AXIS, START)
    constrain(sketch, "Radius", rim, radius, name=radius_name)
    return sketch


def fixed_body(name, y, rings):
    """The pipe through the level circles `rings` ((degrees, radius,
    name) each) along the arc."""
    container = body(name, place((0, y, 0)))
    spine = container.newObject("Sketcher::SketchObject", name + "Spine")
    spine.Label = name + "Spine"
    spine.MapMode = "Deactivated"
    # The XZ plane: the sketch's XY plane turned 90 degrees about X.
    spine.Placement = place((0, 0, 0), (1, 0, 0), 90)
    hide(spine)
    add(spine, arc((-30, 0), 30, 0, 45))
    sections = [ring(container, "%sRing%d" % (name, i), d, r, n) for i, (d, r, n) in enumerate(rings)]
    feature = container.newObject("PartDesign::AdditivePipe", name + "Pipe")
    feature.Profile = sections[0]
    feature.Sections = sections[1:]
    feature.Transformation = "Multisection"
    feature.Spine = (spine, ["Edge1"])
    feature.Mode = "Fixed"
    return feature


fixed_body("Fixed", 0, [(0, 2, "SectionRadius"), (45, 3, None)])
fixed_body("FixedThree", 100, [(0, 2, None), (22.5, 3, None), (45, 2, None)])

doc.recompute()


def wider():
    sketch = doc.getObject("FixedRing0")
    sketch.setDatum("SectionRadius", App.Units.Quantity("2.5 mm"))


variant(wider)
