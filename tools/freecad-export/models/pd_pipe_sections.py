# SPDX-License-Identifier: MIT
# PartDesign pipes through several sections in the turning modes, and
# faces as the profiles and sections of pipes and lofts (mitcad#543), one
# per Body (the Bodies 100 mm apart along Y):
#
# - Frenet, Binormal, Auxiliary: a circle of radius 2 at the start of a
#   quarter of a circle of radius 20 in the XY plane and one of radius 3
#   (named SectionRadius in Frenet) at its end, a pipe through both
#   (Transformation Multisection) in the Frenet mode, in the Binormal mode
#   (the binormal (0, 1, 0) in the pipe's own placement, the profile's:
#   the arc's axis, +Z, in the Body) and in the Auxiliary mode along a
#   line 10 above the arc's chord.
# - PipeFace: a 10 x 10 x 5 block whose top face is swept 20 up a line
#   from its centre (Profile = (block, Face<n>)).
# - LoftFace: a 10 x 10 x 5 block whose top face is lofted to a circle of
#   radius 3 20 above it.
# - MultiFace (FreeCAD 1.0 and later: AllowMultiFace): a 30 x 20 x 10
#   block whose top face and the top of a 10 x 10 x 4 boss beside it are
#   padded 3 mm together.
#
# The arc runs from (0, 0) about (20, 0) to (20, -20), leaving the origin
# along -Y and reaching its end along +X; the circles lie square to it
# there. The sketches have fixed placements in their Bodies.
#
# The variant (pd_pipe_sections_changed) has SectionRadius 4 mm.
#
# Mitcad expects: no fallback
# Mitcad change: SectionRadius = 4 mm


def fixed_sketch(container, name, pos, axis=(0, 0, 1), angle=0.0):
    sketch = container.newObject("Sketcher::SketchObject", name)
    sketch.Label = name
    sketch.MapMode = "Deactivated"
    sketch.Placement = place(pos, axis, angle)
    hide(sketch)
    return sketch


def ring(container, name, at, radius, normal_along_x=False, radius_name=None):
    if normal_along_x:
        sketch = fixed_sketch(container, name, at, (0, 1, 0), 90)
    else:
        sketch = fixed_sketch(container, name, at, (1, 0, 0), 90)
    rim = add(sketch, circle((0, 0), radius))
    constrain(sketch, "Coincident", rim, MID, H_AXIS, START)
    constrain(sketch, "Radius", rim, radius, name=radius_name)
    return sketch


def sections_body(name, y, mode, radius_name=None, rail=False, **options):
    """The pipe through the two circles; with `rail` its auxiliary spine,
    made before the pipe (a sketch after it would be the Body's last
    object)."""
    container = body(name, place((0, y, 0)))
    spine = fixed_sketch(container, name + "Spine", (0, 0, 0))
    add(spine, arc((20, 0), 20, 180, 270))
    start = ring(container, name + "Start", (0, 0, 0), 2)
    end = ring(container, name + "End", (20, -20, 0), 3, True, radius_name)
    guide = None
    if rail:
        guide = fixed_sketch(container, name + "Rail", (0, 0, 10))
        add(guide, line((0, 0), (20, -20)))
    feature = container.newObject("PartDesign::AdditivePipe", name + "Pipe")
    if guide is not None:
        if hasattr(feature, "AuxiliarySpine"):
            feature.AuxiliarySpine = (guide, ["Edge1"])
        else:
            feature.AuxillerySpine = (guide, ["Edge1"])
    feature.Profile = start
    feature.Sections = [end]
    feature.Transformation = "Multisection"
    feature.Spine = (spine, ["Edge1"])
    feature.Mode = mode
    for key, value in options.items():
        setattr(feature, key, value)
    return feature, container


sections_body("Frenet", 0, "Frenet", "SectionRadius")
sections_body("Binormal", 100, "Binormal", Binormal=V(0, 1, 0))
sections_body("Auxiliary", 200, "Auxiliary", rail=True)

piped = body("PipeFace", place((0, 300, 0)))
base = block(piped, "PipeFaceBlock", 10, 10, 5)
doc.recompute()
spine = fixed_sketch(piped, "PipeFaceSpine", (0, 5, 0), (1, 0, 0), 90)
add(spine, line((5, 5), (5, 25)))
pipe = piped.newObject("PartDesign::AdditivePipe", "PipeFacePipe")
pipe.Profile = (base, [face_index(base.Shape, (0, 0, 1))])
pipe.Spine = (spine, ["Edge1"])

lofted = body("LoftFace", place((0, 400, 0)))
base = block(lofted, "LoftFaceBlock", 10, 10, 5)
doc.recompute()
top = fixed_sketch(lofted, "LoftFaceTop", (5, 5, 25))
add(top, circle((0, 0), 3))
loft = lofted.newObject("PartDesign::AdditiveLoft", "LoftFaceLoft")
loft.Profile = (base, [face_index(base.Shape, (0, 0, 1))])
loft.Sections = [top]

containers = [piped, lofted]
if at_least(1, 0):
    several = body("MultiFace", place((0, 500, 0)))
    base = block(several, "MultiFaceBlock", 30, 20, 10)
    boss_sketch = sketch_on_face(several, "MultiFaceBossSketch", base, (0, 0, 1), 10)
    rectangle(boss_sketch, 15, 5, 10, 10)
    boss = pad(several, boss_sketch, 4, "MultiFaceBoss")
    doc.recompute()
    multi = several.newObject("PartDesign::Pad", "MultiFacePad")
    multi.AllowMultiFace = True
    multi.Profile = (boss, [face_index(boss.Shape, (0, 0, 1), 14), face_index(boss.Shape, (0, 0, 1), 10)])
    multi.Length = 3
    containers.append(several)

doc.recompute()
for container in doc.Objects:
    if container.TypeId == "PartDesign::Body":
        # The last solid feature (never a sketch: a sketch as the Tip
        # leaves the Body invalid).
        solids = [f for f in container.Group if f.TypeId.startswith("PartDesign::") and f.isDerivedFrom("PartDesign::Feature")]
        container.Tip = solids[-1]
doc.recompute()


def wider():
    sketch = doc.getObject("FrenetEnd")
    sketch.setDatum("SectionRadius", App.Units.Quantity("4 mm"))


variant(wider)
