# SPDX-License-Identifier: MIT
# The last Part extrusion cases and an oblique face Pad (mitcad#547), the
# operands hidden as FreeCAD's commands hide them:
#
# - Ribbon: a Part::Helix (pitch 4, height 12, radius 5) extruded into
#   faces 2 along the custom direction (0, 0, 1): a ribbon wound round a
#   cylinder.
# - Washer: a plain shape (Part::Feature) of two concentric circles, radii
#   10 and 4, extruded 3 along its normal into a solid: FreeCAD's face
#   maker fills the ring between them.
# - Kinked: a plain shape of a polyline that leaves its plane ((0, 0, 0),
#   (10, 0, 0), (10, 10, 5)) extruded into faces 6 along the custom
#   direction (0, 0, 1).
# - Squares: a Part::Face of a sketch of two squares apart made by
#   FreeCAD's simple face maker, extruded 4 along its normal.
# - Leaning: a Body's 30 x 20 x 10 block, its top face padded Rise (a
#   spreadsheet's alias, 6 mm) along the custom direction (1, 0, 2).
#
# The variant (part_extrusion_last_changed) has Rise 8 mm.
#
# Mitcad expects: no fallback
# Mitcad change: Rise = 8 mm

dims = spreadsheet("Dims", [("B1", "=6 mm", "Rise")])


def extrusion(name, base, solid, **options):
    feature = named(doc.addObject("Part::Extrusion", name), name)
    feature.Base = base
    feature.Solid = solid
    for key, value in options.items():
        setattr(feature, key, value)
    hide(base)
    return feature


def plain(name, shape):
    """A plain shape (Part::Feature) without a definition."""
    feature = named(doc.addObject("Part::Feature", name), name)
    feature.Shape = shape
    return feature


helix = named(doc.addObject("Part::Helix", "RibbonHelix"), "RibbonHelix")
helix.Pitch, helix.Height, helix.Radius = 4, 12, 5
extrusion("Ribbon", helix, False, DirMode="Custom", Dir=V(0, 0, 1), LengthFwd=2)

rings = Part.Compound([Part.makeCircle(10, V(40, 0, 0)), Part.makeCircle(4, V(40, 0, 0))])
extrusion("Washer", plain("WasherRings", rings), True, DirMode="Normal", LengthFwd=3)

kink = Part.makePolygon([V(70, 0, 0), V(80, 0, 0), V(80, 10, 5)])
extrusion("Kinked", plain("KinkedLine", kink), False, DirMode="Custom", Dir=V(0, 0, 1), LengthFwd=6)

squares_sketch = doc.addObject("Sketcher::SketchObject", "SquaresSketch")
squares_sketch.Label = "SquaresSketch"
squares_sketch.Placement = place((0, 40, 0))
rectangle(squares_sketch, 0, 0, 8, 8)
rectangle(squares_sketch, 15, 0, 8, 8)
squares = named(doc.addObject("Part::Face", "SquaresFace"), "SquaresFace")
squares.Sources = [squares_sketch]
squares.FaceMakerClass = "Part::FaceMakerSimple"
hide(squares_sketch)
extrusion("Squares", squares, True, DirMode="Normal", LengthFwd=4)

leaning = body("Leaning", place((0, 80, 0)))
block_pad = block(leaning, "LeaningBlock", 30, 20, 10)
doc.recompute()
slanted = leaning.newObject("PartDesign::Pad", "LeaningPad")
slanted.Profile = (block_pad, [face_index(block_pad.Shape, (0, 0, 1))])
slanted.Length = 6
slanted.UseCustomVector = True
slanted.Direction = V(1, 0, 2)
bind(slanted, "Length", "Dims.Rise")
doc.recompute()
leaning.Tip = slanted
if not slanted.isValid():
    raise Unsupported("a face Pad along a custom direction fails in this version")
doc.recompute()


def higher():
    dims.set("B1", "=8 mm")


variant(higher)
