# SPDX-License-Identifier: MIT
# Pipes in the orientation modes the import translates into a sweep's
# fixed binormal or frame (mitcad#539), one per Body (the Bodies 150 mm
# apart along Y):
#
# - Banded: a 20 x 2 band swept along a quarter of a circle of radius 50
#   in a plane tilted 20 degrees about X in the Binormal mode, the binormal
#   (0, -sin 20, cos 20). FreeCAD sweeps a pipe in its own placement, which
#   1.0 and 1.1 take from the profile (here turned -70 degrees about X), and
#   takes the binormal there as it is: in the Body it is (0, sin 50,
#   cos 50), the band turning only about it. (The binormal (0, 0, 1) was
#   the arc's tangent at its start in that placement: 1.0 and 1.1 left that
#   pipe invalid.)
# - Frenet: the same band along the same arc in the Frenet mode (the band
#   keeps its angle to the arc's plane).
# - Auxiliary: an 8 x 2 bar (its 8 named BarWidth) swept 50 along X,
#   turned by an auxiliary spine 10 above it that crosses from y = 10 to
#   y = -10 (a quarter turn), its points where it crosses the planes square
#   to the spine (the auxiliary spine's curvilinear option off).
# - AuxiliaryCurvilinear: the same with the points at the same fraction of
#   the two lengths (FreeCAD's default).
#
# The sketches have fixed placements in their Bodies (no attachment), the
# same in every FreeCAD version; the spines are their sketches' edges.
# FreeCAD 1.1 renamed the auxiliary spine's properties (AuxillerySpine,
# AuxilleryCurvelinear before; AuxiliarySpine, AuxiliaryCurvilinear since).
#
# The variant (pd_pipe_modes_changed) has BarWidth 10 mm: the bar widens
# from its corner at (-4, -1).
#
# Mitcad expects: no fallback
# Mitcad change: BarWidth = 10 mm


def fixed_sketch(container, name, pos, axis=(0, 0, 1), angle=0.0):
    """An empty sketch at a fixed placement in its container."""
    sketch = container.newObject("Sketcher::SketchObject", name)
    sketch.Label = name
    sketch.MapMode = "Deactivated"
    sketch.Placement = place(pos, axis, angle)
    return sketch


def centred_rectangle(sketch, width, height):
    """A width x height rectangle centred on the sketch's origin, fully
    constrained."""
    lines = rectangle(sketch, -width / 2.0, -height / 2.0, width, height)
    constrain(sketch, "Horizontal", lines[0])
    constrain(sketch, "Horizontal", lines[2])
    constrain(sketch, "Vertical", lines[1])
    constrain(sketch, "Vertical", lines[3])
    constrain(sketch, "Symmetric", lines[0], START, lines[2], START, H_AXIS, START)
    constrain(sketch, "DistanceX", lines[0], START, lines[0], END, width)
    constrain(sketch, "DistanceY", lines[1], START, lines[1], END, height)
    return lines


def cornered_rectangle(sketch, width, height, name=None):
    """A width x height rectangle centred on the sketch's origin, fully
    constrained by its lower left corner and its sizes; its width named
    `name` when given (a new width moves only its right side)."""
    x, y = -width / 2.0, -height / 2.0
    lines = rectangle(sketch, x, y, width, height)
    constrain(sketch, "Horizontal", lines[0])
    constrain(sketch, "Horizontal", lines[2])
    constrain(sketch, "Vertical", lines[1])
    constrain(sketch, "Vertical", lines[3])
    constrain(sketch, "DistanceX", H_AXIS, START, lines[0], START, x)
    constrain(sketch, "DistanceY", H_AXIS, START, lines[0], START, y)
    constrain(sketch, "DistanceX", lines[0], START, lines[0], END, width, name=name)
    constrain(sketch, "DistanceY", lines[1], START, lines[1], END, height)
    return lines


def pipe(container, name, profile, spine, mode, **options):
    feature = container.newObject("PartDesign::AdditivePipe", name)
    feature.Profile = profile
    feature.Spine = (spine, ["Edge1"])
    feature.Mode = mode
    for key, value in options.items():
        setattr(feature, key, value)
    hide(profile)
    hide(spine)
    return feature


def arc_body(name, y, mode, **options):
    """The band along the sloped arc: the arc's plane is the XY plane turned
    20 degrees about X, so it leaves (50, 0, 0) along (0, cos 20, sin 20);
    the band's plane is square to that (the XY plane turned -70 degrees
    about X), its 20 along X."""
    container = body(name, place((0, y, 0)))
    spine = fixed_sketch(container, name + "Spine", (0, 0, 0), (1, 0, 0), 20)
    add(spine, arc((0, 0), 50, 0, 90))
    profile = fixed_sketch(container, name + "Band", (50, 0, 0), (1, 0, 0), -70)
    centred_rectangle(profile, 20, 2)
    return pipe(container, name + "Pipe", profile, spine, mode, **options)


def bar_body(name, y, curvilinear, width_name=None):
    """The bar along X, turned by the rail 10 above it: the bar's plane is
    the YZ plane (the XY plane turned 90 degrees about Y: its x along -Z),
    its 8 along Z."""
    container = body(name, place((0, y, 0)))
    spine = fixed_sketch(container, name + "Spine", (0, 0, 0))
    add(spine, line((0, 0), (50, 0)))
    rail = fixed_sketch(container, name + "Rail", (0, 0, 10))
    add(rail, line((0, 10), (50, -10)))
    profile = fixed_sketch(container, name + "Bar", (0, 0, 0), (0, 1, 0), 90)
    cornered_rectangle(profile, 8, 2, width_name)
    feature = pipe(container, name + "Pipe", profile, spine, "Auxiliary")
    # The auxiliary spine's properties by the version's names.
    if hasattr(feature, "AuxiliarySpine"):
        feature.AuxiliarySpine = (rail, ["Edge1"])
        feature.AuxiliaryCurvilinear = curvilinear
    else:
        feature.AuxillerySpine = (rail, ["Edge1"])
        feature.AuxilleryCurvelinear = curvilinear
    hide(rail)
    return feature


# The binormal in the pipe's placement (see above).
tilt = math.radians(20)
arc_body("Banded", 0, "Binormal", Binormal=V(0, -math.sin(tilt), math.cos(tilt)))
arc_body("Frenet", 150, "Frenet")
bar_body("Auxiliary", 300, False, "BarWidth")
bar_body("AuxiliaryCurvilinear", 450, True)

doc.recompute()
for container in doc.Objects:
    if container.TypeId == "PartDesign::Body":
        container.Tip = container.Group[-1]


def wider():
    sketch = doc.getObject("AuxiliaryBar")
    sketch.setDatum("BarWidth", App.Units.Quantity("10 mm"))


variant(wider)
