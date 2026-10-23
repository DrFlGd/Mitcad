# SPDX-License-Identifier: MIT
# Pads whose profile names edges of its sketch (Profile = (sketch,
# [Edge<n>, ...])), one per Body:
#
# - Dims: PadLength (=5 mm), every pad's length.
# - IgnoredEdges: a sketch of a 20 x 10 rectangle and a 10 x 10 square
#   beside it, the profile naming the rectangle's four edges without
#   AllowMultiFace: FreeCAD ignores them and pads the whole sketch.
# - ChosenEdges (FreeCAD 1.0 and later): the same sketch 40 mm off, with
#   AllowMultiFace: only the rectangle padded.
# - ChosenFrame (1.0 and later): a 30 x 30 square round a 10 x 10 square
#   and a 10 x 10 square beside them (80 mm off), the profile naming the
#   two nested squares' edges with AllowMultiFace: the frame between them
#   padded.
#
# The variant (pd_profile_edges_changed) has PadLength 8 mm.
#
# Mitcad expects: no fallback
# Mitcad change: PadLength = 8 mm

dims = spreadsheet("Dims", [("B1", "=5 mm", "PadLength")])


def edges_padded(name, x, squares, chosen, allow):
    """A Body with a sketch of the squares (x, y, size) padded PadLength,
    the profile naming the edges of the squares numbered in `chosen`."""
    container = body(name)
    sketch = sketch_on(container, name + "Sketch")
    for sx, sy, w, h in squares:
        rectangle(sketch, x + sx, sy, w, h)
    doc.recompute()
    names = []
    for k in chosen:
        sx, sy, w, h = squares[k]
        x0, x1, y0, y1 = x + sx, x + sx + w, sy, sy + h
        names += edges_where(
            sketch.Shape,
            lambda m, e, x0=x0, x1=x1, y0=y0, y1=y1: x0 - 1e-9 <= m.x <= x1 + 1e-9
            and y0 - 1e-9 <= m.y <= y1 + 1e-9
            and (abs(m.x - x0) < 1e-9 or abs(m.x - x1) < 1e-9 or abs(m.y - y0) < 1e-9 or abs(m.y - y1) < 1e-9),
        )
    padded = container.newObject("PartDesign::Pad", name + "Pad")
    padded.Profile = (sketch, names)
    padded.Length = 5
    bind(padded, "Length", "Dims.PadLength")
    if allow:
        padded.AllowMultiFace = True
    hide(sketch)
    return padded


pair = [(0, 0, 20, 10), (25, 0, 10, 10)]
edges_padded("IgnoredEdges", 0, pair, [0], False)
if at_least(1, 0):
    edges_padded("ChosenEdges", 40, pair, [0], True)
    nested = [(0, 0, 30, 30), (10, 10, 10, 10), (35, 0, 10, 10)]
    edges_padded("ChosenFrame", 80, nested, [0, 1], True)
doc.recompute()


def longer():
    dims.set("B1", "=8 mm")


variant(longer)
