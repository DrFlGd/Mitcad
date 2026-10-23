# SPDX-License-Identifier: MIT
# Part workbench sweeps along and of objects that are not sketches
# (mitcad#547), each 40 mm along Y from the one before:
#
# - Spring: a circle of radius 1 (a sketch, named WireRadius) in the XZ
#   plane at (10, 0, 0) swept along a Part helix of radius 10, pitch 5
#   and height 15 about the Z axis (right-handed), Frenet, a solid.
# - LeftSpring: a Part circle of radius 1 swept along a left-handed Part
#   helix of the same sizes, Frenet, a solid.
# - Bend: a Part hexagon (RegularPolygon, circumradius 2) swept along a
#   quarter of a Part circle of radius 20 in the XY plane, from its start
#   (where the hexagon stands square to it), a solid.
# - Rod: a Part circle of radius 3 swept along a Part line 20 up, a
#   solid.
# - Tip: a Part circle of radius 3 and a Part vertex 20 above its centre
#   swept along a Part line between them (a cone), a solid.
#
# The Part objects are edges or a vertex without faces; the sweeps hide
# them as FreeCAD's commands do.
#
# The variant (part_sweep_objects_changed) has WireRadius 1.5 mm.
#
# Mitcad expects: no fallback
# Mitcad change: WireRadius = 1.5 mm


def sweep(name, sections, spine, frenet=False):
    feature = named(doc.addObject("Part::Sweep", name), name)
    feature.Sections = sections
    feature.Spine = spine
    feature.Solid = True
    feature.Frenet = frenet
    for section in sections:
        hide(section)
    hide(spine)
    return feature


def helix(name, at, left=False):
    coil = named(doc.addObject("Part::Helix", name), name)
    coil.Pitch = 5
    coil.Height = 15
    coil.Radius = 10
    coil.Angle = 0
    coil.LocalCoord = "Left-handed" if left else "Right-handed"
    coil.Placement = place(at)
    return coil


def part_circle(name, radius, at, axis=(0, 0, 1), angle=0.0):
    ring = named(doc.addObject("Part::Circle", name), name)
    ring.Radius = radius
    ring.Placement = place(at, axis, angle)
    return ring


def part_line(name, a, b):
    edge = named(doc.addObject("Part::Line", name), name)
    edge.X1, edge.Y1, edge.Z1 = a
    edge.X2, edge.Y2, edge.Z2 = b
    return edge


# Spring: the sketch's XY plane turned 90 degrees about X (the XZ plane).
wire = doc.addObject("Sketcher::SketchObject", "SpringWire")
wire.Label = "SpringWire"
wire.Placement = place((10, 0, 0), (1, 0, 0), 90)
rim = add(wire, circle((0, 0), 1))
constrain(wire, "Coincident", rim, MID, H_AXIS, START)
constrain(wire, "Radius", rim, 1, name="WireRadius")
sweep("Spring", [wire], helix("SpringHelix", (0, 0, 0)), frenet=True)

sweep(
    "LeftSpring",
    [part_circle("LeftSpringCircle", 1, (10, 40, 0), (1, 0, 0), 90)],
    helix("LeftSpringHelix", (0, 40, 0), left=True),
    frenet=True,
)

# Bend: the arc about (20, 80) from 180 to 270 degrees leaves (0, 80)
# along -Y; the hexagon stands in the XZ plane there.
arc_edge = part_circle("BendArc", 20, (20, 80, 0))
for first, second in (("Angle1", "Angle2"), ("Angle0", "Angle1")):
    if hasattr(arc_edge, first) and hasattr(arc_edge, second):
        setattr(arc_edge, first, 180)
        setattr(arc_edge, second, 270)
        break
hexagon = named(doc.addObject("Part::RegularPolygon", "BendHexagon"), "BendHexagon")
hexagon.Polygon = 6
hexagon.Circumradius = 2
hexagon.Placement = place((0, 80, 0), (1, 0, 0), 90)
sweep("Bend", [hexagon], arc_edge)

sweep("Rod", [part_circle("RodCircle", 3, (0, 120, 0))], part_line("RodLine", (0, 120, 0), (0, 120, 20)))

tip = named(doc.addObject("Part::Vertex", "TipVertex"), "TipVertex")
tip.X, tip.Y, tip.Z = 0, 160, 20
sweep("Tip", [part_circle("TipCircle", 3, (0, 160, 0)), tip], part_line("TipLine", (0, 160, 0), (0, 160, 20)))

doc.recompute()


def wider():
    wire.setDatum("WireRadius", App.Units.Quantity("1.5 mm"))


variant(wider)
