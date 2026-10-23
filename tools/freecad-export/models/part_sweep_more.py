# SPDX-License-Identifier: MIT
# Part workbench sweeps of the last missing forms (mitcad#548), each 40 mm
# along Y from the one before:
#
# - ConeSpring: a circle of radius 1 (a sketch, named WireRadius) in the
#   XZ plane at (10, 0, 0) swept along a conical Part helix of radius 10,
#   pitch 5, height 15 and a cone angle of 10 degrees, Frenet, a solid.
# - Knot: a Part circle of radius 1 swept along a spline that lies in no
#   plane (a plain shape of one B-spline edge), a solid.
# - Open: a Part arc (a third of a circle of radius 4, open) swept 20 up
#   along a Part line: a surface (Solid off).
# - Warped: a closed Part polygon whose four corners lie in no plane swept
#   20 up along a Part line: a surface (Solid off).
# - Edge: a Part circle of radius 1 swept along the top edge of a
#   PartDesign Body's 20 x 20 x 10 block (Spine = (Pad, Edge<n>)), a
#   solid.
#
# The variant (part_sweep_more_changed) has WireRadius 1.5 mm.
#
# Mitcad expects: no fallback
# Mitcad change: WireRadius = 1.5 mm


def sweep(name, sections, spine, solid=True, frenet=False):
    feature = named(doc.addObject("Part::Sweep", name), name)
    feature.Sections = sections
    feature.Spine = spine
    feature.Solid = solid
    feature.Frenet = frenet
    for section in sections:
        hide(section)
    if not isinstance(spine, tuple):
        hide(spine)
    return feature


def part_circle(name, radius, at, axis=(0, 0, 1), angle=0.0, arc=None):
    ring = named(doc.addObject("Part::Circle", name), name)
    ring.Radius = radius
    ring.Placement = place(at, axis, angle)
    if arc is not None:
        for first, second in (("Angle1", "Angle2"), ("Angle0", "Angle1")):
            if hasattr(ring, first) and hasattr(ring, second):
                setattr(ring, first, arc[0])
                setattr(ring, second, arc[1])
                break
    return ring


def part_line(name, a, b):
    edge = named(doc.addObject("Part::Line", name), name)
    edge.X1, edge.Y1, edge.Z1 = a
    edge.X2, edge.Y2, edge.Z2 = b
    return edge


# ConeSpring: the sketch's XY plane turned 90 degrees about X (the XZ
# plane) at the helix's start.
wire = doc.addObject("Sketcher::SketchObject", "ConeSpringWire")
wire.Label = "ConeSpringWire"
wire.Placement = place((10, 0, 0), (1, 0, 0), 90)
rim = add(wire, circle((0, 0), 1))
constrain(wire, "Coincident", rim, MID, H_AXIS, START)
constrain(wire, "Radius", rim, 1, name="WireRadius")
coil = named(doc.addObject("Part::Helix", "ConeSpringHelix"), "ConeSpringHelix")
coil.Pitch = 5
coil.Height = 15
coil.Radius = 10
coil.Angle = 10
sweep("ConeSpring", [wire], coil, frenet=True)

# Knot: a spline through points that leave every plane, starting along +X.
spline = Part.BSplineCurve()
spline.interpolate([V(0, 40, 0), V(10, 40, 2), V(20, 45, 8), V(25, 55, 10)])
knot_curve = named(doc.addObject("Part::Feature", "KnotCurve"), "KnotCurve")
knot_curve.Shape = spline.toShape()
start = spline.tangent(spline.FirstParameter)[0]
knot_ring = part_circle("KnotCircle", 1, (0, 40, 0))
knot_ring.Placement = App.Placement(V(0, 40, 0), App.Rotation(V(0, 0, 1), start))
sweep("Knot", [knot_ring], knot_curve)

# Open: a third of a circle in the XY plane, swept up.
sweep(
    "Open",
    [part_circle("OpenArc", 4, (0, 80, 0), arc=(0, 120))],
    part_line("OpenLine", (0, 80, 0), (0, 80, 20)),
    solid=False,
)

# Warped: a closed polygon about (0, 120, 0) whose corners rise and fall.
warped = named(doc.addObject("Part::Polygon", "WarpedPolygon"), "WarpedPolygon")
warped.Nodes = [V(-3, 117, 0), V(3, 117, 1), V(3, 123, 0), V(-3, 123, 1)]
warped.Close = True
sweep("Warped", [warped], part_line("WarpedLine", (0, 120, 0), (0, 120, 20)), solid=False)

# Edge: along the block's top edge from (0, 160, 10) to (20, 160, 10); the
# circle square to it (the XY plane turned 90 degrees about Y).
container = body("EdgeBody")
pad_feature = block(container, "EdgeBlock", 20, 20, 10, 0, 160)
doc.recompute()
edge_name = edge_index(pad_feature.Shape, (0, 160, 10), (20, 160, 10))
sweep("Edge", [part_circle("EdgeCircle", 1, (0, 160, 10), (0, 1, 0), 90)], (pad_feature, [edge_name]))

doc.recompute()


def wider():
    wire.setDatum("WireRadius", App.Units.Quantity("1.5 mm"))


variant(wider)
