# SPDX-License-Identifier: MIT
"""learn.py rule: a right-handed modelled thread on a whole cylinder whose
file axis points up starts its helix at the seam of the cylinder's surface
(core/import/docs/f3d-learning.md, "Twin edges and a thread's start angle"): the thread's angle is
the seam's direction about the axis, from the reference direction.

    learn.py test --kind ThreadFeature --rule tools/f3d-learn/rules/thread_seam_phase.py --unsettled

check(ex) compares the accepted answer's angle with the prediction (within
2.5 degrees: the angle found in later states has that resolution); None
where the rule does not apply (not modelled, the axis points down, no
seam in the dump: a dump made before the decoder read it, a part of a
cylinder).
"""
import math


def unit(v):
    n = math.sqrt(sum(x * x for x in v))
    return [x / n for x in v]


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def upwards(d):
    if abs(d[2]) > 1e-9:
        return d[2] > 0
    if abs(d[1]) > 1e-9:
        return d[1] > 0
    return d[0] > 0


def phase(axis, seam):
    along = unit(axis)
    if not upwards(along):
        return None
    x = [0.0, 1.0, 0.0] if abs(along[0]) > 0.9 else [1.0, 0.0, 0.0]
    ref = unit([a - dot(x, along) * b for a, b in zip(x, along)])
    across = cross(along, ref)
    return math.degrees(math.atan2(dot(seam, across), dot(seam, ref)))


def check(ex):
    d = ex.known
    if not isinstance(d, dict) or not d.get("detail", d).get("isModeled"):
        return None
    detail = d.get("detail", d)
    if detail.get("isRightHanded") is False:
        return None
    faces = detail.get("inputCylindricalFaces") or []
    defs = [x for x in ((ex.answer or {}).get("defs") or []) if x.get("type") == "thread"]
    if not faces or len(defs) != len(faces):
        return None
    result = None
    for face, df in zip(faces, defs):
        seam = (face.get("_f3d") or {}).get("seam")
        axis = (face.get("geometry") or {}).get("axis")
        if seam is None or axis is None:
            return None
        pred = phase(axis, seam)
        if pred is None:
            return None
        angle = float(str(df.get("angle") or "0 deg").split()[0])
        diff = (angle - pred + 540.0) % 360.0 - 180.0
        if abs(diff) > 2.5:
            return False
        result = True
    return result
