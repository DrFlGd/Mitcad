# SPDX-License-Identifier: MIT
# Part workbench fillets and chamfers with two sizes on an edge (the
# operands hidden, as FreeCAD's commands hide them):
#
# - Dims: BoxLength (=40 mm).
# - Varied: a BoxLength x 20 x 10 box (Part::Fillet): its top front edge
#   rounded from 1 mm at the start of the edge to 3 mm at its end, its top
#   back edge from 3 mm to 1.5 mm, its bottom front edge a constant 2 mm
#   (no two of them meet).
# - Slanted: a 30 x 20 x 10 box 60 mm off (Part::Chamfer): its top front
#   edge chamfered 1 mm and 2.5 mm.
#
# The variant (part_fillet_variable_changed) has BoxLength 50 mm.
#
# Mitcad expects: no fallback
# Mitcad change: BoxLength = 50 mm

dims = spreadsheet("Dims", [("B1", "=40 mm", "BoxLength")])

box = named(doc.addObject("Part::Box", "VariedBox"), "VariedBox")
box.Length, box.Width, box.Height = 40, 20, 10
bind(box, "Length", "Dims.BoxLength")
doc.recompute()


def edge_number(shape, test):
    """The number of the one edge of a shape whose middle passes the test."""
    names = edges_where(shape, test)
    if len(names) != 1:
        raise RuntimeError("expected one edge, found %s" % names)
    return int(names[0][4:])


def along_x(edge):
    """Whether a straight edge runs along x."""
    return abs(edge.Vertexes[0].Point.x - edge.Vertexes[1].Point.x) > 1e-9


top_front = edge_number(box.Shape, lambda m, e: along_x(e) and abs(m.y) < 1e-9 and abs(m.z - 10) < 1e-9)
top_back = edge_number(box.Shape, lambda m, e: along_x(e) and abs(m.y - 20) < 1e-9 and abs(m.z - 10) < 1e-9)
bottom_front = edge_number(box.Shape, lambda m, e: along_x(e) and abs(m.y) < 1e-9 and abs(m.z) < 1e-9)
fillet = named(doc.addObject("Part::Fillet", "Varied"), "Varied")
fillet.Base = box
fillet.Edges = [(top_front, 1.0, 3.0), (top_back, 3.0, 1.5), (bottom_front, 2.0, 2.0)]
hide(box)

plain = named(doc.addObject("Part::Box", "SlantedBox"), "SlantedBox")
plain.Length, plain.Width, plain.Height = 30, 20, 10
plain.Placement = place((60, 0, 0))
doc.recompute()
chamfer = named(doc.addObject("Part::Chamfer", "Slanted"), "Slanted")
chamfer.Base = plain
chamfer.Edges = [
    (edge_number(plain.Shape, lambda m, e: along_x(e) and abs(m.y) < 1e-9 and abs(m.z - 10) < 1e-9), 1.0, 2.5)
]
hide(plain)
doc.recompute()


def longer():
    dims.set("B1", "=50 mm")


variant(longer)
