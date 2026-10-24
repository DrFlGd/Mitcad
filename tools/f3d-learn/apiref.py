# SPDX-License-Identifier: MIT
"""Compares the `.f3d` decoder with the API reference dumps.

The reference dumps are the answers of the original application's own
interface for real designs (one JSON dump per design in the schema of
core/import/SCHEMA.md, walked per timeline step). The decoder's dump IR
follows the same schema, so the two are joined per timeline item and every
input group is compared field by field:

    decoder   the dump IR of `mitcad-f3d-inspect design FILE --json`
    census    the answer the history accepted in the learning dump
              (core/import/docs/f3d-learning.md, "Learning the undecoded inputs")
    API       the reference dump, taken as the truth

    apiref.py compare --api DIR --ir DIR --out DIR
                      [--census DIR --index FILE.tsv --root DIR]
                      [--group G] [--max-rows N] [--jobs N]
    apiref.py groups

`--api` holds `<id>.json.gz`, `--ir` holds `<id>.json` (one decoder dump per
design). The learning dump's file names carry a hash of the design's path
(`census_lines`), so `--census` needs `--index`, a TSV whose columns are
`id` and `source` (the reference dumps' `index.tsv`), and `--root`, the
folder the sources are relative to (the learning dump was made with
`ROOT/source` as the path).

Output in `--out`: `summary.txt` (a table per group), `mismatches/<group>.tsv`
(design, item, name, field, source (decoder or census), state, the source's
value, the API's value; of two unequal sets, what only each holds)
and `items.tsv` (how the timelines were joined).

Every field of a group counts once per item (or per sketch element) as

    agree      decoder and API give the same value
    differs    both give a value and they differ
    dec_miss   the API gives a value, the decoder none
    api_miss   the decoder gives a value, the API none

(both absent: not counted), and, where the item's history settled, the
census's accepted answer against the API in the same four classes
(`c_agree`, ...). A census disagreement where the decoder agrees is a
guess that the history accepted although it was wrong. Fields the
accepted answer does not carry are not counted for it.

Values are compared as the file's internal units (cm, rad) with a
tolerance of 1e-4 (edges, by their middle and length: 2e-2, as the edges of
a replayed history approximate the application's curves). Entities are
compared by what they are, not by their tokens or ids: faces and edges by
their geometry, sketch curves by their geometry in the sketch, features by
their names.

Python 3.8+, standard library only.
"""

import argparse
import glob
import gzip
import json
import math
import os
import sys
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor

STATES = ("agree", "differs", "dec_miss", "api_miss")
CSTATES = ("c_agree", "c_differs", "c_miss", "c_extra")
TOL = 1e-4
EDGE_TOL = 2e-2  # cm

# ---------------------------------------------------------------------------
# Loading


def load_json(path):
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as f:
        return json.load(f)


def fnv_hash(text):
    """The 32-bit hash of a design's path in the learning dump's file names
    (FNV-1a over the UTF-8 bytes, the low 32 bits)."""
    h = 0xCBF29CE484222325
    for b in text.encode("utf-8"):
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return "%08x" % (h & 0xFFFFFFFF)


def census_lines(census_dir, path):
    """The learning dump's lines of one design (by its path's hash), each
    with `status` (settled/unsettled) set; [] when there are none."""
    if not census_dir or not path:
        return []
    tag = "-%s.jsonl" % fnv_hash(path)
    out = []
    for status in ("settled", "unsettled"):
        for name in sorted(glob.glob(os.path.join(census_dir, status, "*" + tag))):
            with open(name, encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line:
                        d = json.loads(line)
                        d["status"] = status
                        out.append(d)
    return out


# ---------------------------------------------------------------------------
# Joining the timelines
#
# The decoder and the application number the timeline alike, except that the
# decoder lists a few items the application does not (`DerivedContext`, group
# items) and names occurrence items by their class; the display names of
# occurrences and construction geometry differ too. The items are therefore
# aligned by a global alignment on (type family, name, index).

FAMILY = {
    "Occurrence": "occurrence",
    "ComponentInsert": "occurrence",
    "ComponentFromBodies": "occurrence",
    "DerivedInstance": "occurrence",
    "Fastener": "occurrence",
    "CopyPasteOccurrence": "occurrence",
    "AssemblyConstraint": "assembly",
    "GeometricRelationship": "assembly",
    "CutPasteBody": "body_copy",
    "CopyPasteBody": "body_copy",
    "CopyPasteBodies": "body_copy",
    "CutPasteBodies": "body_copy",
    "RectangularPatternFeature": "rect",
    "RectangularOccurrencePattern": "rect",
    "CircularPatternFeature": "circ",
    "CircularOccurrencePattern": "circ",
    "MirrorFeature": "mirror",
    "MirrorComponent": "mirror",
    "Feature": "other",
    "Context": "other",
    "PCBFeature": "other",
    "MeshFeature": "other",
}


def family(t):
    return FAMILY.get(t, t)


def base_name(name):
    """A display name without the occurrence counter and spacing."""
    name = (name or "").strip()
    if ":" in name and name.rsplit(":", 1)[1].isdigit():
        name = name.rsplit(":", 1)[0]
    return name.replace(" ", "").lower()


def pair_score(a, b):
    """The alignment score of two items, None when they cannot be the same."""
    if family(a.get("objectType")) != family(b.get("objectType")):
        return None
    na, nb = base_name(a.get("name")), base_name(b.get("name"))
    s = 2
    if na == nb:
        s += 3
    elif na and nb and (na in nb or nb in na):
        s += 1
    if a.get("index") == b.get("index"):
        s += 1
    return s


def align(api_items, ir_items):
    """[(api item or None, decoder item or None)] in timeline order."""
    n, m = len(api_items), len(ir_items)
    gap = -1
    score = [[0] * (m + 1) for _ in range(n + 1)]
    back = [[None] * (m + 1) for _ in range(n + 1)]
    for i in range(1, n + 1):
        score[i][0] = i * gap
        back[i][0] = "u"
    for j in range(1, m + 1):
        score[0][j] = j * gap
        back[0][j] = "l"
    for i in range(1, n + 1):
        for j in range(1, m + 1):
            best, how = score[i - 1][j] + gap, "u"
            if score[i][j - 1] + gap > best:
                best, how = score[i][j - 1] + gap, "l"
            p = pair_score(api_items[i - 1], ir_items[j - 1])
            if p is not None and score[i - 1][j - 1] + p > best:
                best, how = score[i - 1][j - 1] + p, "d"
            score[i][j], back[i][j] = best, how
    out = []
    i, j = n, m
    while i > 0 or j > 0:
        how = back[i][j]
        if how == "d":
            out.append((api_items[i - 1], ir_items[j - 1]))
            i, j = i - 1, j - 1
        elif how == "u":
            out.append((api_items[i - 1], None))
            i -= 1
        else:
            out.append((None, ir_items[j - 1]))
            j -= 1
    out.reverse()
    return out


# ---------------------------------------------------------------------------
# Values and their comparison


def is_num(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool)


class Bag(object):
    """An unordered list of values, compared as a multiset."""

    def __init__(self, items=()):
        self.items = list(items)

    def __len__(self):
        return len(self.items)

    def __repr__(self):
        return "Bag(%r)" % (self.items,)


class Within(Bag):
    """A set the other side must hold (and may exceed): the bodies an
    operation visibly changed against the bodies its record selects."""


def qkey(x):
    """A hashable, coarsely rounded form of a value (bucket of `same`)."""
    if is_num(x):
        return round(float(x), 3) + 0.0
    if isinstance(x, Bag):
        return ("bag", tuple(sorted(repr(qkey(v)) for v in x.items)))
    if isinstance(x, (tuple, list)):
        return tuple(qkey(v) for v in x)
    return x


def same(a, b):
    """Equality with a tolerance on numbers, up to order in a Bag."""
    if a is None or b is None:
        return a is None and b is None
    if is_num(a) and is_num(b):
        return abs(a - b) <= TOL + TOL * max(abs(a), abs(b))
    if isinstance(a, Within):
        if not isinstance(b, Bag):
            return False
        left_a, _ = bag_match(a.items, b.items)
        return not left_a
    if isinstance(a, Bag) or isinstance(b, Bag):
        if not (isinstance(a, Bag) and isinstance(b, Bag)) or len(a) != len(b):
            return False
        left_a, left_b = bag_match(a.items, b.items)
        return not left_a and not left_b
    if isinstance(a, (tuple, list)) and isinstance(b, (tuple, list)):
        if (len(a) in (3, 4) and len(b) in (3, 4) and a[0] == "edge" and b[0] == "edge"
                and not isinstance(a[1], Bag) and not isinstance(b[1], Bag)):
            return edge_same(a, b)
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    return a == b


def on_circle(p, circle):
    """Whether the point lies on the circle `("circle", center, normal,
    radius, closed)` (within EDGE_TOL)."""
    _, center, normal, radius, _ = circle
    d = [p[i] - center[i] for i in range(3)]
    height = sum(d[i] * normal[i] for i in range(3))
    flat = math.sqrt(max(0.0, sum(x * x for x in d) - height * height))
    return abs(height) <= EDGE_TOL and abs(flat - radius) <= EDGE_TOL + TOL * radius


def on_segment(p, line):
    """Whether the point lies on the segment `("line", start, end)`."""
    _, a, b = line
    ab = [b[i] - a[i] for i in range(3)]
    ap = [p[i] - a[i] for i in range(3)]
    n2 = sum(x * x for x in ab)
    if n2 <= 0:
        return False
    t = sum(ap[i] * ab[i] for i in range(3)) / n2
    off = math.sqrt(max(0.0, sum(x * x for x in ap) - t * t * n2))
    return -EDGE_TOL / math.sqrt(n2) <= t <= 1 + EDGE_TOL / math.sqrt(n2) and off <= EDGE_TOL


def on_geometry(p, geom):
    return on_circle(p, geom) if geom[0] == "circle" else on_segment(p, geom)


def edge_same(a, b):
    """Two edges by their middle and length: the edges of a replayed history
    are approximations of the application's (curves of splines), so these
    compare to EDGE_TOL. A closed circle (the API's edge carries its circle
    as a fourth element) has no middle of its own: the file's lies opposite
    its seam, a replay's where its seam is, so the other edge is the same
    when it is as long and its middle is on the circle."""
    for geom, other in ((a[3:], b), (b[3:], a)):
        if geom and geom[0][0] == "circle" and geom[0][4]:
            return (
                abs(a[2] - b[2]) <= EDGE_TOL + TOL * max(a[2], b[2])
                and isinstance(other[1], tuple)
                and on_circle(other[1], geom[0])
            )
    if any(g and g[0][0] == "curve" for g in (a[3:], b[3:])):
        # a spline or ellipse: as long, and its middle a tenth of its length
        # from the other's at most
        return (
            abs(a[2] - b[2]) <= 1e-3 * max(a[2], b[2]) + 1e-3
            and isinstance(a[1], tuple)
            and isinstance(b[1], tuple)
            and math.dist(a[1], b[1]) <= 0.1 * max(a[2], b[2]) + EDGE_TOL
        )
    for x, y in zip(a[1:3], b[1:3]):
        xs = x if isinstance(x, tuple) else (x,)
        ys = y if isinstance(y, tuple) else (y,)
        if len(xs) != len(ys) or any(abs(p - q) > EDGE_TOL + TOL * max(abs(p), abs(q)) for p, q in zip(xs, ys)):
            return False
    return True


def bag_match(a_items, b_items):
    """(unmatched of a, unmatched of b) after pairing equal values."""
    b_left = list(b_items)
    buckets = defaultdict(list)
    for k, y in enumerate(b_left):
        buckets[qkey(y)].append(k)
    used = set()
    a_left = []
    for x in a_items:
        hit = None
        for k in buckets.get(qkey(x), ()):
            if k not in used and same(x, b_left[k]):
                hit = k
                break
        if hit is None:
            a_left.append(x)
        else:
            used.add(hit)
    rest = [y for k, y in enumerate(b_left) if k not in used]
    a_final = []
    for x in a_left:
        hit = None
        for k, y in enumerate(rest):
            if same(x, y):
                hit = k
                break
        if hit is None:
            a_final.append(x)
        else:
            del rest[hit]
    return a_final, rest


def on_face(p, key):
    """Whether a point lies on the plane or cylinder `key` (a canonical
    `("face", geometry_key)` entity's key), None for other surfaces."""
    try:
        if key[0] == "plane":
            return abs(dot(key[1], p) - key[2]) <= EDGE_TOL
        if key[0] == "cylinder":
            axis, radius, base = key[1], key[2], key[3]
            d = [p[i] - base[i] for i in range(3)]
            along = dot(d, axis)
            return abs(math.sqrt(max(0.0, dot(d, d) - along * along)) - radius) <= EDGE_TOL + TOL * radius
    except (IndexError, TypeError):
        pass
    return None


def fold_faces(api, other):
    """`other`, a Bag of (edge, size), with the edges of a face the API
    selects made the face: a selection of a face is that of the edges
    round it, which the file's input names as edges. The face replaces the
    edges that lie on it (they are all, or every edge of a face is taken
    when the face is selected)."""
    left_api, left_other = bag_match(api.items, other.items)
    gone = []
    folded = []
    for x in left_api:
        e = x[0]
        if not (isinstance(e, tuple) and e[:1] == ("face",) and len(e) == 2 and isinstance(e[1], tuple)):
            continue
        on = [
            y for y in left_other
            if not any(y is g for g in gone) and isinstance(y[0], tuple) and y[0][:1] == ("edge",)
            and len(y[0]) >= 3 and not isinstance(y[0][1], Bag) and same(x[1], y[1])
            and on_face(y[0][1], e[1])
        ]
        if on:
            gone.extend(on)
            folded.append(x)
    if not folded:
        return other
    return Bag([y for y in other.items if not any(y is g for g in gone)] + folded)


def dressup_edges(fa, *others):
    """Brings the edges of a fillet's or chamfer's input of the decoder and
    of the accepted answer to the API's form, in place: where the API lists
    none (a failed or lost reference) there is nothing to compare; where it
    selects a face, the edges on the face are the face; the pieces of an
    edge are the edge. Where it selects faces its count of edges is not
    comparable."""
    api = fa.get("edges")
    if not isinstance(api, Bag):
        return
    if not api.items:
        fa.pop("edges", None)
        fa.pop("edges.count", None)
        return
    has_faces = any(isinstance(x[0], tuple) and x[0][:1] == ("face",) for x in api.items)
    for o in others:
        if o is None or not isinstance(o.get("edges"), Bag):
            continue
        o["edges"] = fold_faces(api, fold_pieces(api, o["edges"]))
        o["edges.count"] = len(o["edges"])
    if has_faces:
        fa.pop("edges.count", None)
        for o in others:
            if o is not None:
                o.pop("edges.count", None)


def fold_pieces(api, other):
    """`other`, a Bag of (edge, size), with the pieces of an API edge made
    one: a replay splits an edge (at its seam, at a vertex) into pieces on
    the same curve whose lengths add up to its. The API's edge replaces them
    when they are all of the edges that lie on its circle or line without
    matching another edge; a selection of the pieces is the selection of
    the edge."""
    left_api, left_other = bag_match(api.items, other.items)
    taken = set()
    folded = []
    for x in left_api:
        e = x[0]
        if not (isinstance(e, tuple) and e[0] == "edge" and len(e) == 4 and e[3][0] in ("circle", "line")):
            continue
        pieces = [
            k for k, y in enumerate(left_other)
            if k not in taken and isinstance(y[0], tuple) and y[0][:1] == ("edge",) and len(y[0]) >= 3
            and not isinstance(y[0][1], Bag) and same(x[1], y[1])
            and y[0][2] < e[2] - EDGE_TOL and on_geometry(y[0][1], e[3])
        ]
        total = sum(left_other[k][0][2] for k in pieces)
        if len(pieces) >= 2 and abs(total - e[2]) <= EDGE_TOL * len(pieces) + TOL * e[2]:
            taken.update(pieces)
            folded.append(x)
    if not folded:
        return other
    gone = [left_other[k] for k in taken]
    items = []
    for y in other.items:
        hit = next((g for g in gone if g is y), None)
        if hit is None:
            items.append(y)
    return Bag(items + folded)


def uniq(items):
    """The distinct values of a list (by `same`), first occurrences."""
    out = []
    for x in items:
        if not any(same(x, y) for y in out):
            out.append(x)
    return out


def jsonable(x):
    if isinstance(x, Bag):
        return {"bag": [jsonable(v) for v in x.items]}
    if isinstance(x, (tuple, list)):
        return [jsonable(v) for v in x]
    if isinstance(x, float):
        return round(x, 6)
    return x


def show(x, limit=300):
    s = json.dumps(jsonable(x), ensure_ascii=False, separators=(",", ":"))
    return s if len(s) <= limit else s[: limit - 3] + "..."


def vec(v):
    return tuple(float(t) for t in v)


def edge_geometry(edge):
    """The API edge's circle (`("circle", center, normal, radius, closed)`;
    arcs too) or line (`("line", start, end)`), else None."""
    g = edge.get("geometry")
    s, e = edge.get("start_point"), edge.get("end_point")
    if not isinstance(g, dict) or s is None or e is None:
        return None
    if g.get("type") in ("Circle3D", "Arc3D") and all(g.get(k) is not None for k in ("center", "normal", "radius")):
        closed = all(abs(p - q) <= 1e-6 for p, q in zip(s, e))
        return ("circle", vec(g["center"]), vec(g["normal"]), float(g["radius"]), closed)
    if g.get("type") == "Line3D":
        return ("line", vec(s), vec(e))
    # (A spline's or ellipse's middle is its parameter's, which a replay's
    # curve puts elsewhere along it.)
    return ("curve",)


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def unit(v):
    n = math.sqrt(dot(v, v))
    return tuple(t / n for t in v) if n > 0 else tuple(v)


def sign_free(v):
    """A direction without its sign (the first non-zero component positive)."""
    v = unit(v)
    for t in v:
        if abs(t) > 1e-9:
            return tuple(-x for x in v) if t < 0 else v
    return v


def foot(origin, axis):
    """The point of the line through `origin` along `axis` nearest to 0."""
    a = unit(axis)
    t = -dot(origin, a)
    return tuple(o + t * x for o, x in zip(origin, a))


# ---------------------------------------------------------------------------
# Canonical entities
#
# A dump (API or decoder) is read through a Canon: it turns references into
# values that agree when both name the same entity.

ENTITY_KINDS = {
    "face", "edge", "vertex", "body", "feature", "construction_plane",
    "construction_axis", "construction_point", "sketch_entity", "profile",
    "occurrence", "component",
}
SKIP_KEYS = {"entityToken", "tempId", "parentFeature", "parentConstructionPlane",
             "parentConstructionAxis", "parentConstructionPoint"}


def curve_endpoints(g):
    """The two end points of a sketch curve's geometry, or None."""
    if "startPoint" in g and "endPoint" in g:
        return vec(g["startPoint"]), vec(g["endPoint"])
    if g.get("type") == "Arc3D" and all(
            k in g for k in ("center", "normal", "referenceVector", "radius", "startAngle", "endAngle")):
        c, n, r = vec(g["center"]), unit(vec(g["normal"])), unit(vec(g["referenceVector"]))
        b = cross(n, r)

        def pt(a):
            return tuple(c[i] + g["radius"] * (math.cos(a) * r[i] + math.sin(a) * b[i]) for i in range(3))
        return pt(g["startAngle"]), pt(g["endAngle"])
    return None


def curve_key(c):
    """A sketch curve by its geometry in the sketch (not by id or type name)."""
    g = c.get("geometry") or {}
    t = g.get("type")
    ends = curve_endpoints(g)
    if t == "Line3D" and ends:
        return ("L", Bag([ends[0], ends[1]]))
    if t == "Circle3D" and "center" in g:
        return ("O", vec(g["center"]), g.get("radius"))
    if t == "Arc3D" and ends and "center" in g:
        return ("A", vec(g["center"]), g.get("radius"), Bag([ends[0], ends[1]]))
    if t in ("Ellipse3D", "EllipticalArc3D") and "center" in g:
        return ("E", vec(g["center"]), g.get("majorRadius"), g.get("minorRadius"),
                g.get("startAngle"), g.get("endAngle"))
    n = g.get("nurbs") if isinstance(g.get("nurbs"), dict) else None
    if n and n.get("controlPoints"):
        cps = n["controlPoints"]
        return ("N", Bag([vec(cps[0]), vec(cps[-1])]), n.get("controlPointCount", len(cps)))
    return ("X", c.get("type"), c.get("id"))


def geometry_key(g):
    """A surface or line by its geometry, independent of where on it the
    stored origin lies."""
    t = g.get("type")
    try:
        if t == "Plane":
            n = unit(vec(g["normal"]))
            return ("plane", n, dot(n, vec(g["origin"])))
        if t == "Cylinder":
            return ("cylinder", sign_free(vec(g["axis"])), g["radius"], foot(vec(g["origin"]), g["axis"]))
        if t == "Sphere":
            return ("sphere", vec(g["origin"] if "origin" in g else g["center"]), g["radius"])
        if t == "InfiniteLine3D":
            return ("axis", sign_free(vec(g["direction"])), foot(vec(g["origin"]), g["direction"]))
        if t == "Line3D":
            return ("line", Bag([vec(g["startPoint"]), vec(g["endPoint"])]))
    except (KeyError, TypeError, ValueError):
        pass
    items = []
    for k in sorted(g):
        v = g[k]
        if k == "type" or k.startswith("_"):
            continue
        if is_num(v):
            items.append((k, float(v)))
        elif isinstance(v, list) and all(is_num(x) for x in v):
            items.append((k, vec(v)))
    return ("geometry", t, tuple(items))


class Canon(object):
    """Reads one dump (`side` is "api" or "ir") into comparable values."""

    def __init__(self, dump, side):
        self.dump = dump
        self.side = side
        self.items = list(((dump.get("timeline") or {}).get("items")) or [])
        self.by_index = {it.get("index"): it for it in self.items}
        self.sketches = {}
        for it in self.items:
            if it.get("objectType") == "Sketch":
                self.sketches.setdefault(it.get("name"), it)
        self._tokens = {}
        self._step_info = None
        self.index_map = {}  # decoder timeline index -> the application's
        params = dump.get("parameters") or {}
        self.params = {}
        self.param_list = []
        for k in ("user", "model"):
            for p in params.get(k) or []:
                if isinstance(p, dict) and "name" in p:
                    self.params.setdefault(p["name"], p)
                    self.param_list.append(p)

    # --- bodies the steps show

    def _steps(self):
        """(creator of each body, changed bodies per timeline index): the
        application's step snapshots give the bodies after each item; a body
        is created where its name first appears (the k-th new one), and an
        item changed the bodies whose volume differs from the step before."""
        if self._step_info is not None:
            return self._step_info
        creators, changed = {}, {}
        prev = {}
        for st in ((self.dump.get("steps") or {}).get("items")) or []:
            cur = {}
            for b in st.get("bodies") or []:
                if isinstance(b, dict) and b.get("name") is not None:
                    cur[(b.get("occurrence") or "", b["name"])] = b.get("volume")
            new = [k for k in cur if k not in prev]
            for k, key in enumerate(new):
                creators.setdefault(key, (st.get("timeline_index"), k))
            changed[st.get("timeline_index")] = [
                key for key, v in cur.items()
                if key in prev and is_num(v) and is_num(prev[key]) and abs(v - prev[key]) > 1e-7 * (1 + abs(v))]
            prev = cur
        self._step_info = (creators, changed)
        return self._step_info

    def changed_bodies(self, index):
        """The (creating item, k) of the bodies the item at `index` changed."""
        creators, changed = self._steps()
        return [creators[k] for k in changed.get(index, ()) if k in creators]

    # --- sketch entities

    def sketch_tokens(self, name):
        """{sketch-local id: token} of a sketch's points and curves."""
        if name in self._tokens:
            return self._tokens[name]
        toks = {}
        it = self.sketches.get(name)
        if it:
            d = it.get("detail") or {}
            for p in d.get("points") or []:
                if "xyz" in p and p["xyz"] is not None:
                    toks[p["id"]] = ("P", vec(p["xyz"]))
            for c in d.get("curves") or []:
                toks[c["id"]] = curve_key(c)
        self._tokens[name] = toks
        return toks

    def sketch_id(self, name, id_):
        return self.sketch_tokens(name).get(id_, ("id", id_))

    # --- references

    def entity(self, ref):
        kind = ref.get("kind")
        g = ref.get("geometry") if isinstance(ref.get("geometry"), dict) else None
        if kind == "face":
            if g:
                return ("face", geometry_key(g))
            if ref.get("point_on_face"):
                return ("face", "at", vec(ref["point_on_face"]))
            return None
        if kind == "edge":
            if ref.get("mid_point") is not None and ref.get("length") is not None:
                edge = ("edge", vec(ref["mid_point"]), float(ref["length"]))
                geom = edge_geometry(ref)
                if geom:
                    edge += (geom,)
                return edge
            if ref.get("start_point") is not None and ref.get("end_point") is not None:
                return ("edge", Bag([vec(ref["start_point"]), vec(ref["end_point"])]))
            return None
        if kind == "vertex":
            return ("vertex", vec(ref["point"])) if ref.get("point") else ("vertex",)
        if kind == "body":
            # A body by the item that made it and its place among that
            # item's bodies (the names are the application's, the decoder's
            # bodies come with their producer).
            if self.side == "api":
                creators = self._steps()[0]
                made = creators.get((ref.get("occurrence") or "", ref.get("name")))
                if made is None:
                    # a reference made in the body's own component names no
                    # occurrence: the body by its name, when only one has it
                    same_name = [v for (_, n), v in creators.items() if n == ref.get("name")]
                    made = same_name[0] if len(same_name) == 1 else None
                if made is not None:
                    return ("body", made)
            else:
                f3d = ref.get("_f3d") or {}
                if f3d.get("producer") is not None and f3d.get("body_index") is not None:
                    return ("body", (self.index_map.get(f3d["producer"], f3d["producer"]), f3d["body_index"]))
            return ("body", ref.get("name"))
        if kind == "feature":
            return ("feature", ref.get("name"))
        if kind in ("construction_plane", "construction_axis", "construction_point"):
            if ref.get("origin"):
                return (kind, "origin", ref["origin"])
            if ref.get("timeline_index") is not None:
                return (kind, "item", self.index_map.get(ref["timeline_index"], ref["timeline_index"]))
            if g:
                return (kind, geometry_key(g))
            return (kind, ref.get("name"))
        if kind == "sketch_entity":
            return ("sketch_entity", ref.get("sketch"), self.sketch_id(ref.get("sketch"), ref.get("id")))
        if kind == "profile":
            return ("profile", ref.get("sketch"))
        if kind == "component":
            return ("component", ref.get("name"))
        if kind == "occurrence":
            return ("occurrence", ref.get("name") or ref.get("fullPathName"))
        return None

    def value(self, v, bag=False):
        """A field value as a comparable value (None: absent)."""
        if v is None:
            return None
        if isinstance(v, bool):
            return v
        if isinstance(v, (int, float)):
            if isinstance(v, float) and not math.isfinite(v):
                return None
            return v
        if isinstance(v, str):
            # an object handle the application's interface could not convert
            return None if v.startswith("<") and " proxy of " in v else v
        if isinstance(v, dict):
            if "_error" in v:
                return None
            kind = v.get("kind")
            if kind == "parameter":
                return self.value(v.get("value"))
            if kind in ENTITY_KINDS:
                return self.entity(v)
            if v.get("type") in GEOMETRY_TYPES:
                return geometry_key(v)
            return tuple((k, self.value(x)) for k, x in sorted(v.items())
                         if not k.startswith("_") and k not in SKIP_KEYS)
        if isinstance(v, list):
            items = [self.value(x) for x in v if not (isinstance(x, dict) and "_truncated" in x)]
            if bag:
                return Bag(items)
            return tuple(items)
        return None

    def flatten(self, detail, skip=(), bags=()):
        """The fields of an item's detail: a dict-valued field is split one
        level (`extentOne.distance`), references and lists are values."""
        out = {}
        for k, v in (detail or {}).items():
            if k.startswith("_") or k in SKIP_KEYS or k in skip:
                continue
            if isinstance(v, dict) and "_error" not in v and v.get("kind") not in ENTITY_KINDS \
                    and v.get("kind") != "parameter" and v.get("type") not in GEOMETRY_TYPES:
                for kk, vv in v.items():
                    if (kk.startswith("_") and kk != "_type") or kk in SKIP_KEYS:
                        continue
                    out["%s.%s" % (k, kk)] = self.value(vv, bag=("%s.%s" % (k, kk)) in bags)
            else:
                out[k] = self.value(v, bag=k in bags)
        return out


GEOMETRY_TYPES = {"Plane", "Cylinder", "Cone", "Sphere", "Torus", "InfiniteLine3D", "Line3D"}

# ---------------------------------------------------------------------------
# The fields of each input group
#
# A group function reads one item of a dump (the application's or the
# decoder's: the same schema) into {field: comparable value}.

SIZE_KEYS = ("radius", "startRadius", "endRadius", "distance", "distanceOne", "distanceTwo", "angle")


def region_of(canon, sketch, loops):
    """A profile region as ("region", outer curves, holes): `loops` is
    [(is_outer, [sketch curve ids])]."""
    outer, holes = [], []
    for is_outer, ids in loops:
        toks = uniq([canon.sketch_id(sketch, i) for i in ids])
        if is_outer:
            outer = uniq(outer + toks)
        else:
            holes.append(Bag(toks))
    return ("region", Bag(outer), Bag(holes))


def api_loop_ids(loop):
    ids = [c.get("curve") if isinstance(c, dict) else c for c in loop.get("curves") or []]
    return [i for i in ids if i is not None]


def loops_ir(ls):
    return [(bool(l.get("outer")), [c["id"] for c in l.get("curves") or [] if "id" in c])
            for l in ls if isinstance(l, dict)]


def profile_fields(canon, detail, side):
    """`profile.sketch`, `profile.count`, `profile.regions` (and the private
    `_regions`, the same as a list) of a feature that takes profiles. The
    decoder's profile record lists the regions of a selection as its
    children when it has them, else as its own loops."""
    prof = detail.get("profile")
    if prof is None or (isinstance(prof, dict) and "_error" in prof):
        return {}
    prof = [prof] if isinstance(prof, dict) else list(prof)
    refs = [p for p in prof if isinstance(p, dict) and p.get("sketch") is not None]
    out = {"profile.sketch": Bag(uniq([p.get("sketch") for p in refs]))}
    regions = None
    if side == "api":
        regions = [region_of(canon, p.get("sketch"), [(bool(l.get("isOuter")), api_loop_ids(l))
                                                      for l in p.get("loops") or []])
                   for p in refs if p.get("kind") == "profile" and "loops" in p]
        areas = [p["area"] for p in refs if p.get("kind") == "profile" and is_num(p.get("area"))]
        if areas:
            out["profile.areas"] = Bag(areas)
        # profiles whose curves the application does not name (projected or
        # linked geometry) cannot be compared by curves
        unresolved = [r for r in regions if not r[1].items]
        if unresolved:
            out["profile.unresolved"] = len(unresolved)
            out["profile.count"] = len(regions)
            out["_regions"] = [r for r in regions if r[1].items]
            return out
    else:
        entries = detail.get("_f3d_profile_loops")
        if entries is not None:
            regions = []
            for k, e in enumerate(entries):
                sk = refs[k].get("sketch") if k < len(refs) else None
                if e.get("children"):
                    for ch in e["children"]:
                        regions.append(region_of(canon, sk, loops_ir(ch)))
                else:
                    regions.append(region_of(canon, sk, loops_ir(e.get("loops") or [])))
    if regions is not None:
        out["profile.count"] = len(regions)
        out["profile.regions"] = Bag(regions)
        out["_regions"] = regions
    return out


def region_covers(ra, rd):
    """The decoder's region `rd` holds the API's region `ra`: its outer
    curves hold ra's and each of ra's holes is among its holes' supersets."""
    if not tokens_within(ra[1], rd[1]):
        return False
    return all(any(tokens_within(h, g) for g in rd[2].items) for h in ra[2].items)


def flat_curves(region):
    """The distinct curves of a region, outer and holes together."""
    items = list(region[1].items)
    for h in region[2].items:
        items.extend(h.items)
    return uniq(items)


def region_units(acc, design, idx, name, group, fa, fd):
    """One unit per region of the API's profile list: found among the
    decoder's regions (equal, or the decoder's holding it), or not."""
    ra, rd = fa.get("_regions"), fd.get("_regions")
    if ra is None and rd is None:
        return
    ra, rd = ra or [], rd or []
    free = list(range(len(rd)))
    for pas in (0, 1):
        pending = []
        for r in (ra if pas == 0 else pending_a):
            hit = None
            for j in free:
                if (same(r, rd[j]) if pas == 0 else region_covers(r, rd[j])):
                    hit = j
                    break
            if hit is None:
                pending.append(r)
            else:
                free.remove(hit)
                acc.unit(design, idx, name, group, "profile.region" + (".exact" if pas == 0 else ".relaxed"),
                         True, True)
        pending_a = pending
    # The same curves in another loop structure: the application merges
    # holes that touch along a curve into one loop, the file lists them
    # apart (and the reverse).
    pending = []
    for r in pending_a:
        hit = None
        for j in free:
            if same(Bag(flat_curves(r)), Bag(flat_curves(rd[j]))):
                hit = j
                break
        if hit is None:
            pending.append(r)
        else:
            free.remove(hit)
            acc.unit(design, idx, name, group, "profile.region.loops", True, True)
    pending_a = pending
    # Regions the decoder's one outline holds as a union: the file keeps
    # the outline of the selected regions, the application lists each
    # region (so one outline holds any number of them, also where an
    # earlier region already matched it).
    pending, used = [], set()
    for r in pending_a:
        mine = flat_curves(r)
        hit = None
        for j in range(len(rd)):
            if tokens_within(Bag(mine), Bag(flat_curves(rd[j]))):
                hit = j
                break
        if hit is None:
            pending.append(r)
        else:
            used.add(hit)
            acc.unit(design, idx, name, group, "profile.region.union", True, True)
    free = [j for j in free if j not in used]
    pending_a = pending
    for r in pending_a:
        acc.unit(design, idx, name, group, "profile.region.missing", r, None)
    if "profile.unresolved" in fa:
        return  # some of the application's regions are unnamed: the decoder's extras cannot be told
    for j in free:
        acc.unit(design, idx, name, group, "profile.region.extra", None, rd[j])


def direction_of(extent):
    """"positive" or "negative" along the sketch normal, from an extent."""
    if not isinstance(extent, dict):
        return None
    t = extent.get("_type")
    if t == "DistanceExtentDefinition":
        p = extent.get("distance")
        if isinstance(p, dict) and is_num(p.get("value")):
            return "negative" if p["value"] < 0 else "positive"
    if t == "ThroughAllExtentDefinition" and isinstance(extent.get("isPositiveDirection"), bool):
        return "positive" if extent["isPositiveDirection"] else "negative"
    return None


def participant_field(canon, it, side):
    """`participants`: the application's bodies the operation changed (it
    gives no list), against the producing item and body index of each body
    the decoder's record selects."""
    d = it.get("detail") or {}
    if "NewBody" in str(d.get("operation")):
        return {}
    if side == "api":
        bodies = canon.changed_bodies(it.get("index"))
        return {"participants": Within(bodies)} if bodies else {}
    pb = d.get("participantBodies")
    if not isinstance(pb, list):
        return {}
    out = []
    for b in pb:
        f3d = (b or {}).get("_f3d") or {}
        if f3d.get("producer") is not None and f3d.get("body_index") is not None:
            out.append((canon.index_map.get(f3d["producer"], f3d["producer"]), f3d["body_index"]))
    # an empty list says nothing: the decoder found no body
    return {"participants": Bag(out)} if out else {}


def f_extrusion(canon, it, side):
    d = it.get("detail") or {}
    out = canon.flatten(d, skip=("profile", "participantBodies"))
    out.update(participant_field(canon, it, side))
    out.update(profile_fields(canon, d, side))
    if d.get("isThinExtrude") is False:
        # the wall's location means nothing on a solid extrusion: the
        # application keeps its default there
        out.pop("thinExtrudeWallLocationOne", None)
        out.pop("thinExtrudeWallLocationTwo", None)
    for k in ("extentOne.entity", "extentTwo.entity"):
        # a face's plane has no side: the same plane with either normal
        if k in out:
            out[k] = plane_sign_free(out[k])
    dr = direction_of(d.get("extentOne"))
    if dr:
        out["direction"] = dr
    if side == "ir":
        # The decoder's raw extrude record where the dump leaves the field out.
        ex = (it.get("_f3d") or {}).get("extrude") or {}
        if "extentType" not in d and ex.get("extent_a") in EXTENT_CODES:
            out["extentType"] = EXTENT_CODES[ex["extent_a"]]
        if not isinstance(d.get("extentOne"), dict) and (ex.get("sides") or [None])[0] == 4:
            out["extentOne._type"] = "ThroughAllExtentDefinition"
            if is_num(ex.get("flag")):
                # the flag byte after the extent codes: 1 against the sketch normal
                out["extentOne.isPositiveDirection"] = ex["flag"] == 0
                out["direction"] = "positive" if ex["flag"] == 0 else "negative"
    return out


EXTENT_CODES = {1: "OneSideFeatureExtentType", 2: "TwoSidesFeatureExtentType", 3: "SymmetricFeatureExtentType"}


def f_revolve(canon, it, side):
    d = it.get("detail") or {}
    out = canon.flatten(d, skip=("profile", "participantBodies"))
    out.update(participant_field(canon, it, side))
    out.update(profile_fields(canon, d, side))
    return out


def axis_line_of(ref):
    """An axis reference (an origin or construction axis, a cylindrical face,
    a circular or straight edge) as the line it stands for: its direction
    without sign and the point of it nearest to the origin; None when the
    reference does not tell."""
    if not isinstance(ref, dict):
        return None
    g = ref.get("geometry") if isinstance(ref.get("geometry"), dict) else {}
    try:
        if ref.get("kind") == "construction_axis" and ref.get("origin") in ORIGIN_AXIS_DIRS:
            return ("axisline", ORIGIN_AXIS_DIRS[ref["origin"]], (0.0, 0.0, 0.0))
        if g.get("type") == "InfiniteLine3D":
            return ("axisline", sign_free(vec(g["direction"])), foot(vec(g["origin"]), g["direction"]))
        if g.get("type") == "Cylinder":
            return ("axisline", sign_free(vec(g["axis"])), foot(vec(g["origin"]), g["axis"]))
        if g.get("type") == "Circle3D":
            return ("axisline", sign_free(vec(g["normal"])), foot(vec(g["center"]), g["normal"]))
        if g.get("type") == "Line3D":
            d = tuple(e - b for b, e in zip(vec(g["startPoint"]), vec(g["endPoint"])))
            return ("axisline", sign_free(d), foot(vec(g["startPoint"]), d))
    except (KeyError, TypeError, ValueError):
        return None
    return None


ORIGIN_AXIS_DIRS = {"X": (1.0, 0.0, 0.0), "Y": (0.0, 1.0, 0.0), "Z": (0.0, 0.0, 1.0)}


def plane_sign_free(v):
    """A plane value without the side its normal points to."""
    if isinstance(v, tuple) and len(v) == 2 and isinstance(v[1], tuple) and v[1] and v[1][0] == "plane":
        _, n, d = v[1]
        for t in n:
            if abs(t) > 1e-9:
                if t < 0:
                    return (v[0], ("plane", tuple(-x for x in n), -d))
                break
    return v


def f_pattern(canon, it, side):
    d = it.get("detail") or {}
    out = canon.flatten(d, bags=("inputEntities",))
    line = axis_line_of(d.get("axis"))
    if line:
        out["axis.line"] = line
    for n in ("One", "Two"):
        v = d.get("direction" + n)
        if isinstance(v, list) and len(v) == 3 and all(is_num(x) for x in v):
            out["direction%s.line" % n] = ("dir", sign_free(vec(v)))
    if "mirrorPlane" in out:
        out["mirrorPlane"] = plane_sign_free(out["mirrorPlane"])
    return out


def f_dressup(canon, it, side):
    d = it.get("detail") or {}
    out = canon.flatten(d, skip=("edgeSets", "edges"))
    sets = [s for s in d.get("edgeSets") or [] if isinstance(s, dict) and "_truncated" not in s]
    if d.get("edgeSets") is None:
        return out
    edges = []
    for s in sets:
        size = tuple((k, canon.value(s.get(k))) for k in SIZE_KEYS if s.get(k) is not None)
        for e in s.get("edges") or []:
            if isinstance(e, dict) and "_truncated" not in e:
                edges.append((canon.value(e), size))
    out["edgeSets.count"] = len(sets)
    out["edges.count"] = len(edges)
    out["edges"] = Bag(edges)
    for k in ("_type", "isTangentChain", "continuity", "tangencyWeight", "isFlipped"):
        vals = [canon.value(s.get(k)) for s in sets if s.get(k) is not None]
        if vals:
            out["edgeSets.%s" % k] = Bag(vals)
    return out


def f_thread(canon, it, side):
    out = canon.flatten(it.get("detail") or {}, skip=("hole", "inputCylindricalFace"), bags=("inputCylindricalFaces",))
    if out.get("isFullLength") is True:
        out.pop("threadLength", None)
        out.pop("threadOffset", None)
    return out


def f_construction(canon, it, side):
    out = canon.flatten(it.get("detail") or {}, skip=("transform",))
    for k in ("definition.planarEntity", "definition.planarEntityOne", "definition.planarEntityTwo"):
        # a face's plane has no side: the same plane with either normal
        if k in out:
            out[k] = plane_sign_free(out[k])
    paths = out.pop("definition.pathEntities", None)
    if side == "ir" and paths is not None:
        # the decoder lists the path's edges, the application gives the path
        # as one edge or, for several curves, a list of its own
        out["definition.pathEntity"] = paths[0] if len(paths) == 1 else tuple(() for _ in paths)
    return out


def f_shell(canon, it, side):
    out = canon.flatten(it.get("detail") or {}, bags=("inputEntities",))
    faces = out.get("inputEntities")
    if isinstance(faces, Bag):
        # a face's plane has no side: the same plane with either normal
        out["inputEntities"] = Bag([plane_sign_free(f) for f in faces.items])
    return out


def f_combine(canon, it, side):
    return canon.flatten(it.get("detail") or {}, bags=("toolBodies",))


def f_hole(canon, it, side):
    """A hole's fields. The application gives its thread as a reference to
    the thread feature; the decoder reflects the thread feature's fields in
    the hole's record: both only say that the hole has a thread."""
    d = it.get("detail") or {}
    out = canon.flatten(d, skip=("thread",))
    if d.get("thread") is not None:
        out["thread"] = ("thread",)
    return out


def f_generic(canon, it, side):
    return canon.flatten(it.get("detail") or {})


def path_fields(canon, detail):
    """A path's entities (a sweep's or a pipe's): the application also gives
    each path entity its curve as a spline, its type, its direction and its
    parent path, which are the entity again."""
    path = detail.get("path")
    if path is None or (isinstance(path, dict) and "_error" in path):
        return {}
    path = [path] if isinstance(path, dict) else list(path)
    ents = [canon.value(p.get("entity")) for p in path if isinstance(p, dict) and p.get("entity") is not None]
    return {"path": Bag(ents)} if ents else {}


def sketch_of_profiles(canon, detail):
    """The sketches whose profiles a feature takes (the application lists
    every region of a sketch picked whole; the decoder names the sketch)."""
    prof = detail.get("profile")
    if prof is None or (isinstance(prof, dict) and "_error" in prof):
        return {}
    prof = [prof] if isinstance(prof, dict) else list(prof)
    names = uniq([p.get("sketch") for p in prof if isinstance(p, dict) and p.get("sketch") is not None])
    return {"profile.sketch": Bag(names)} if names else {}


def f_sweep(canon, it, side):
    d = it.get("detail") or {}
    out = canon.flatten(d, skip=("path", "profile"))
    out.update(path_fields(canon, d))
    out.update(sketch_of_profiles(canon, d))
    return out


def f_loft(canon, it, side):
    """A loft's sections as (index, the section's entity, the end condition's
    type): the application's end conditions also point back at their section."""
    d = it.get("detail") or {}
    out = canon.flatten(d, skip=("loftSections",))
    secs = d.get("loftSections")
    if isinstance(secs, list):
        out["loftSections"] = tuple(
            (s.get("index"), canon.value(s.get("entity")),
             (s.get("endCondition") or {}).get("_type") if isinstance(s.get("endCondition"), dict) else None)
            for s in secs if isinstance(s, dict))
    return out


def f_move(canon, it, side):
    """A move's definition repeats the item's transform and names the item."""
    out = canon.flatten(it.get("detail") or {})
    for k in ("definition.parentMoveFeature", "definition.transform"):
        out.pop(k, None)
    return out


def timeline_name(it):
    """The name of a timeline item as compared. An occurrence's name in the
    application is its component's name with a counter (` body:1`), the
    decoder's is the component's name or the generic `Component Insert<n>`
    (the item's own display name), so occurrence items compare by the base
    name, the generic ones as one name."""
    name = it.get("name")
    if family(it.get("objectType")) != "occurrence" or not isinstance(name, str):
        return name
    base = base_name(name)
    if base.startswith("componentinsert") or base.startswith("componentfrombodies"):
        return "Component Insert"
    return base


def f_timeline(canon, it, side):
    """The item itself: its type, name and states."""
    out = {"type": family(it.get("objectType")), "name": timeline_name(it)}
    if side == "api":
        out["isSuppressed"] = bool(it.get("isSuppressed"))
        out["healthState"] = it.get("healthState")
        out["isRolledBack"] = bool(it.get("isRolledBack"))
        if it.get("errorOrWarningMessage"):
            out["errorOrWarningMessage"] = it["errorOrWarningMessage"]
    else:
        # the decoder writes both flags when true and nothing otherwise
        out["isSuppressed"] = bool(it.get("isSuppressed"))
        out["isRolledBack"] = bool(it.get("isRolledBack"))
    return out


# (group, objectType of the application's item) -> field function
ITEM_GROUPS = {
    "extrusion": ("ExtrudeFeature", f_extrusion),
    "revolve": ("RevolveFeature", f_revolve),
    "pattern.circular": ("CircularPatternFeature", f_pattern),
    "pattern.rectangular": ("RectangularPatternFeature", f_pattern),
    "pattern.path": ("PathPatternFeature", f_pattern),
    "mirror": ("MirrorFeature", f_pattern),
    "fillet": ("FilletFeature", f_dressup),
    "chamfer": ("ChamferFeature", f_dressup),
    "hole": ("HoleFeature", f_hole),
    "thread": ("ThreadFeature", f_thread),
    "shell": ("ShellFeature", f_shell),
    "combine": ("CombineFeature", f_combine),
    "construction.plane": ("ConstructionPlane", f_construction),
    "construction.axis": ("ConstructionAxis", f_construction),
    "construction.point": ("ConstructionPoint", f_construction),
    "other.loft": ("LoftFeature", f_loft),
    "other.sweep": ("SweepFeature", f_sweep),
    "other.pipe": ("PipeFeature", f_sweep),
    "other.split_body": ("SplitBodyFeature", f_generic),
    "other.move": ("MoveFeature", f_move),
    "other.offset_faces": ("OffsetFacesFeature", f_generic),
    "other.base_feature": ("BaseFeature", f_generic),
    "other.copy_paste_body": ("CopyPasteBody", f_generic),
    "other.cut_paste_body": ("CutPasteBody", f_generic),
    "other.draft": ("DraftFeature", f_generic),
    "other.delete_face": ("DeleteFaceFeature", f_generic),
    "other.remove": ("RemoveFeature", f_generic),
}
GROUP_OF_TYPE = {t: g for g, (t, _) in ITEM_GROUPS.items()}
SPECIAL_GROUPS = ("timeline", "sketch", "sketch.curves", "sketch.points", "sketch.constraints",
                  "sketch.dimensions", "parameters")
ALL_GROUPS = list(ITEM_GROUPS) + list(SPECIAL_GROUPS)

# ---------------------------------------------------------------------------
# Counting

NOCEN = object()


def classify(api, other):
    """The state of `other` against the API's value, None when both are
    absent."""
    if api is None and other is None:
        return None
    if other is None:
        return "dec_miss"
    if api is None:
        return "api_miss"
    return "agree" if same(api, other) else "differs"


CMAP = {"agree": "c_agree", "differs": "c_differs", "dec_miss": "c_miss", "api_miss": "c_extra"}


def diff_show(a, b):
    """Two values for the mismatch list; of two unequal bags, what only
    each holds."""
    if isinstance(a, Bag) and isinstance(b, Bag):
        left_a, left_b = bag_match(a.items, b.items)
        return "only:" + show(left_a, 240), "only:" + show(left_b, 240)
    return show(a), show(b)


class Acc(object):
    """The counts and mismatch rows of a run (picklable)."""

    def __init__(self):
        self.counts = defaultdict(Counter)   # (group, field) -> state counts
        self.api_n = Counter()               # (group, field) -> API values
        self.api_vals = defaultdict(Counter)  # (group, field) -> short API values
        self.rows = defaultdict(list)        # (group, field) -> mismatch rows
        self.notes = Counter()

    def unit(self, design, item, name, group, field, api, dec, cen=NOCEN):
        key = (group, field)
        ds = classify(api, dec)
        cs = None if cen is NOCEN else classify(api, cen)
        if ds is None and cs is None:
            return
        if api is not None:
            self.api_n[key] += 1
            if not isinstance(api, (Bag, tuple, list)) or len(show(api, 60)) < 40:
                self.api_vals[key][show(api, 40)] += 1
        if ds:
            self.counts[key][ds] += 1
        if cs:
            self.counts[key][CMAP[cs]] += 1
        if ds and ds != "agree":
            a, d = diff_show(api, dec)
            self.rows[key].append((design, item, name, field, "decoder", ds, d, a))
        if cs and cs != "agree":
            a, c = diff_show(api, cen)
            self.rows[key].append((design, item, name, field, "census", cs, c, a))

    def merge(self, other, max_rows):
        for k, c in other.counts.items():
            self.counts[k].update(c)
        self.api_n.update(other.api_n)
        for k, c in other.api_vals.items():
            self.api_vals[k].update(c)
        for k, rows in other.rows.items():
            room = max_rows - len(self.rows[k]) if max_rows else len(rows)
            self.rows[k].extend(rows[:max(room, 0)])
            self.notes["rows_dropped"] += max(len(rows) - max(room, 0), 0)
        self.notes.update(other.notes)


# ---------------------------------------------------------------------------
# Sketches and parameters: elements are matched by what they are


def sketch_ids(v):
    """The point and curve ids a constraint's or dimension's refs name."""
    out = []
    if isinstance(v, str):
        if len(v) > 1 and v[0] in "pc" and v[1:].isdigit():
            out.append(v)
    elif isinstance(v, list):
        for x in v:
            out.extend(sketch_ids(x))
    elif isinstance(v, dict):
        for x in v.values():
            out.extend(sketch_ids(x))
    return out


def element_tokens(canon, sketch, e):
    return Bag([canon.sketch_id(sketch, i) for i in sketch_ids(e.get("refs"))])


def tokens_within(a, b):
    """All of bag a is in bag b."""
    left_a, _ = bag_match(a.items, b.items)
    return not left_a


def match_elements(a_items, d_items, tok_a, tok_d, type_of):
    """Pair an API list with a decoder list: same type and the same entities,
    then the API's entities among the decoder's; returns (pairs, api only,
    decoder only, differing pairs) as index lists."""
    free = list(range(len(d_items)))
    pairs, rest_a = [], []
    for pas in (0, 1):
        todo = rest_a if pas else list(range(len(a_items)))
        keep = []
        for i in todo:
            hit = None
            for j in free:
                if type_of(a_items[i]) != type_of(d_items[j]):
                    continue
                ok = same(tok_a[i], tok_d[j]) if pas == 0 else tokens_within(tok_a[i], tok_d[j])
                if ok:
                    hit = j
                    break
            if hit is None:
                keep.append(i)
            else:
                pairs.append((i, hit))
                free.remove(hit)
        rest_a = keep
    differing, api_only = [], []
    for i in rest_a:
        hit = next((j for j in free if type_of(a_items[i]) == type_of(d_items[j])), None)
        if hit is None:
            api_only.append(i)
        else:
            differing.append((i, hit))
            free.remove(hit)
    return pairs, api_only, free, differing


def sketch_units(acc, design, api_c, ir_c, ai, ii):
    da, di = ai.get("detail") or {}, ii.get("detail") or {}
    sk, idx = ai.get("name"), ai.get("index")
    for f in ("referencePlane", "origin", "xDirection", "yDirection"):
        acc.unit(design, idx, sk, "sketch", f, api_c.value(da.get(f)), ir_c.value(di.get(f)))
    acc.unit(design, idx, sk, "sketch", "counts.curves", len(da.get("curves") or []), len(di.get("curves") or []))
    acc.unit(design, idx, sk, "sketch", "counts.points", len(da.get("points") or []), len(di.get("points") or []))

    # curves, by their geometry
    ac, dc = da.get("curves") or [], di.get("curves") or []
    ka, kd = [curve_key(c) for c in ac], [curve_key(c) for c in dc]
    pairs, api_only, dec_only, _ = _match_geometry(ac, dc, ka, kd)
    for i, j in pairs:
        c_a, c_d = ac[i], dc[j]
        t = c_a.get("type")
        acc.unit(design, idx, "%s %s" % (sk, c_a.get("id")), "sketch.curves", "geometry:" + t, True, True)
        acc.unit(design, idx, "%s %s" % (sk, c_a.get("id")), "sketch.curves", "type", t, c_d.get("type"))
        # A flag the decoder leaves out is false (it writes the true ones).
        for f in ("isConstruction", "isCenterLine", "isFixed", "isReference", "isLinked"):
            acc.unit(design, idx, "%s %s" % (sk, c_a.get("id")), "sketch.curves", f,
                     c_a.get(f), bool(c_d.get(f)) if c_a.get(f) is not None else c_d.get(f))
        for f in ("radius", "majorAxisRadius", "minorAxisRadius"):
            acc.unit(design, idx, "%s %s" % (sk, c_a.get("id")), "sketch.curves", f,
                     api_c.value(c_a.get(f)), ir_c.value(c_d.get(f)))
    for i in api_only:
        acc.unit(design, idx, "%s %s" % (sk, ac[i].get("id")), "sketch.curves",
                 "geometry:" + ac[i].get("type"), ka[i], None)
    for j in dec_only:
        acc.unit(design, idx, "%s %s" % (sk, dc[j].get("id")), "sketch.curves",
                 "geometry:" + dc[j].get("type"), None, kd[j])

    # points, by position
    ap, dp = da.get("points") or [], di.get("points") or []
    pa = [("P", vec(p["xyz"])) for p in ap if p.get("xyz") is not None]
    pd = [("P", vec(p["xyz"])) for p in dp if p.get("xyz") is not None]
    only_a, only_d = bag_match(pa, pd)
    for _ in range(len(pa) - len(only_a)):
        acc.unit(design, idx, sk, "sketch.points", "position", True, True)
    for p in only_a:
        acc.unit(design, idx, sk, "sketch.points", "position", p, None)
    for p in only_d:
        acc.unit(design, idx, sk, "sketch.points", "position", None, p)

    # constraints
    cons_a, cons_d = da.get("constraints") or [], di.get("constraints") or []
    tok_a = [element_tokens(api_c, sk, c) for c in cons_a]
    tok_d = [element_tokens(ir_c, ii.get("name"), c) for c in cons_d]
    pairs, api_only, dec_only, differing = match_elements(
        cons_a, cons_d, tok_a, tok_d, lambda c: c.get("type"))
    for i, j in pairs:
        acc.unit(design, idx, "%s %s" % (sk, cons_a[i].get("id")), "sketch.constraints", cons_a[i]["type"],
                 True, True)
    for i, j in differing:
        acc.unit(design, idx, "%s %s" % (sk, cons_a[i].get("id")), "sketch.constraints", cons_a[i]["type"],
                 tok_a[i], tok_d[j])
    for i in api_only:
        acc.unit(design, idx, "%s %s" % (sk, cons_a[i].get("id")), "sketch.constraints", cons_a[i]["type"],
                 tok_a[i], None)
    for j in dec_only:
        acc.unit(design, idx, "%s %s" % (sk, cons_d[j].get("id")), "sketch.constraints",
                 cons_d[j].get("type") or "unknown", None, tok_d[j])

    # dimensions
    dim_a, dim_d = da.get("dimensions") or [], di.get("dimensions") or []
    tok_a = [element_tokens(api_c, sk, c) for c in dim_a]
    tok_d = [element_tokens(ir_c, ii.get("name"), c) for c in dim_d]
    pairs, api_only, dec_only, differing = match_elements(
        dim_a, dim_d, tok_a, tok_d, lambda c: c.get("type"))
    for i, j in pairs + differing:
        a, d = dim_a[i], dim_d[j]
        label = "%s %s" % (sk, a.get("id"))
        g = "sketch.dimensions"
        t = a["type"]
        if (i, j) in differing:
            acc.unit(design, idx, label, g, t, tok_a[i], tok_d[j])
        else:
            acc.unit(design, idx, label, g, t, True, True)
        pa_, pd_ = a.get("parameter"), d.get("parameter")
        acc.unit(design, idx, label, g, "parameter.name",
                 pa_.get("name") if isinstance(pa_, dict) else None, pd_.get("name") if isinstance(pd_, dict) else None)
        if a.get("isDriving") is False:
            # A driven dimension shows the measurement of the sketch as it is
            # now; the file keeps the value of the last time it was solved.
            acc.notes["driven_dimension_values"] += 1
        else:
            acc.unit(design, idx, label, g, "parameter.value", api_c.value(pa_), ir_c.value(pd_))
            acc.unit(design, idx, label, g, "parameter.expression", expr_of(pa_), expr_of(pd_))
        acc.unit(design, idx, label, g, "isDriving", a.get("isDriving"),
                 d.get("isDriving") if "isDriving" in d else (True if a.get("isDriving") is not None else None))
        acc.unit(design, idx, label, g, "textPosition", api_c.value(a.get("textPosition")),
                 ir_c.value(d.get("textPosition")))
    for i in api_only:
        acc.unit(design, idx, "%s %s" % (sk, dim_a[i].get("id")), "sketch.dimensions", dim_a[i]["type"],
                 tok_a[i], None)
    for j in dec_only:
        acc.unit(design, idx, "%s %s" % (sk, dim_d[j].get("id")), "sketch.dimensions",
                 dim_d[j].get("type") or "unknown", None, tok_d[j])


def expr_of(p):
    if isinstance(p, dict) and isinstance(p.get("expression"), str):
        return "".join(p["expression"].split())
    return None


def _match_geometry(ac, dc, ka, kd):
    """Pair sketch curves by their geometry key; leftovers of one type
    pair up as differing."""
    free = list(range(len(dc)))
    buckets = defaultdict(list)
    for j in free:
        buckets[qkey(kd[j])].append(j)
    used, pairs, rest = set(), [], []
    for i, k in enumerate(ka):
        hit = None
        for j in buckets.get(qkey(k), ()):
            if j not in used and same(k, kd[j]):
                hit = j
                break
        if hit is None:
            rest.append(i)
        else:
            used.add(hit)
            pairs.append((i, hit))
    left_d = [j for j in free if j not in used]
    api_only = []
    for i in rest:
        hit = next((j for j in left_d if same(ka[i], kd[j])), None)
        if hit is None:
            api_only.append(i)
        else:
            left_d.remove(hit)
            pairs.append((i, hit))
    return pairs, api_only, left_d, []


def creator_index(p, index_map=None):
    """The timeline index of the item that created a parameter (the sketch
    of a dimension, the feature), in the application's numbering."""
    c = (p or {}).get("createdBy") or {}
    i = c.get("timeline_index", c.get("sketch_timeline_index"))
    if isinstance(i, int) and index_map is not None:
        return index_map.get(i, i)
    return i if isinstance(i, int) else None


def pair_parameters(la, ld, index_map):
    """Pairs of (API, decoder) parameters of one name. The application lists
    the parameters of every component design of the file (an inserted
    component has its own table, whose names `d1`, `d2`, ... repeat the root's),
    so the parameters of one name are paired in stages, the strictest key
    first: the creator's kind, timeline index and the parameter's role; kind
    and index; the index; what the parameter says (expression, unit, role);
    and last by order among those of one role. Unpaired ones pair with None."""
    def keys(p, imap):
        c = (p or {}).get("createdBy") or {}
        idx = creator_index(p, imap)
        return (
            (owner_kind(c), idx, p.get("role")),
            (owner_kind(c), idx),
            (idx,) if idx is not None else None,
            (expr_of(p), p.get("unit"), p.get("role")),
        )
    ka = [keys(a, None) for a in la]
    kd = [keys(d, index_map) for d in ld]
    free_a, free_d = list(range(len(la))), list(range(len(ld)))
    pairs = []
    for stage in range(4):
        for i in list(free_a):
            if ka[i][stage] is None:
                continue
            hit = next((j for j in free_d if kd[j][stage] == ka[i][stage]), None)
            if hit is not None:
                free_a.remove(i)
                free_d.remove(hit)
                pairs.append((la[i], ld[hit]))
    # the remaining ones in their order when they share the role (a wrong
    # creator then shows as a difference); the others are not the same parameter
    for i in free_a:
        hit = next((j for j in free_d if ld[j].get("role") == la[i].get("role")), None)
        if hit is None:
            pairs.append((la[i], None))
        else:
            free_d.remove(hit)
            pairs.append((la[i], ld[hit]))
    pairs.extend((None, ld[j]) for j in free_d)
    return pairs


def parameter_units(acc, design, api_c, ir_c):
    """Parameters by name; a name that several components use pairs the
    parameters in their order."""
    def by_name(canon):
        out = defaultdict(list)
        for p in canon.param_list:
            out[p["name"]].append(p)
        return out
    if any(isinstance(e, dict) and e.get("section") == "parameters" for e in api_c.dump.get("errors") or []):
        acc.notes["parameters_sections_failed"] += 1  # the reference has no parameters to compare with
        return
    pa, pd = by_name(api_c), by_name(ir_c)
    # The application also lists the parameters of the other designs a file
    # links to (the components of an assembly): an unpaired parameter whose
    # creator is no item of this design's timeline is not in the file.
    own = {(it.get("index"), it.get("name")) for it in api_c.items}
    driven = {  # parameters of the application's driven dimensions
        ((d.get("parameter") or {}).get("name"))
        for it in api_c.items for d in ((it.get("detail") or {}).get("dimensions") or [])
        if isinstance(d, dict) and d.get("isDriving") is False}

    def linked(p):
        c = p.get("createdBy") or {}
        idx = c.get("timeline_index")
        return bool(c.get("kind")) and isinstance(idx, int) and (idx, c.get("name")) not in own
    g = "parameters"
    for name in sorted(set(pa) | set(pd)):
        for a, d in pair_parameters(pa.get(name, []), pd.get(name, []), ir_c.index_map):
            if a and not d and linked(a):
                acc.notes["parameters_linked_designs"] += 1
                continue
            acc.unit(design, name, name, g, "exists", True if a else None, True if d else None)
            if not (a and d):
                continue
            if name not in driven:  # a driven dimension's value is a measurement (see sketch.dimensions)
                acc.unit(design, name, name, g, "expression", expr_of(a), expr_of(d))
                acc.unit(design, name, name, g, "value", api_c.value(a.get("value")), ir_c.value(d.get("value")))
            acc.unit(design, name, name, g, "unit", a.get("unit"), d.get("unit"))
            acc.unit(design, name, name, g, "role", a.get("role"), d.get("role"))
            acc.unit(design, name, name, g, "comment", a.get("comment") or None, d.get("comment") or None)
            acc.unit(design, name, name, g, "isFavorite", a.get("isFavorite"), bool(d.get("isFavorite")))  # written when true
            acc.unit(design, name, name, g, "dependents",
                     Bag(a.get("dependents") or []) if a.get("dependents") else None,
                     Bag(d.get("dependents") or []) if d.get("dependents") else None)
            ca, cd = a.get("createdBy") or {}, d.get("createdBy") or {}
            acc.unit(design, name, name, g, "createdBy.kind", owner_kind(ca), owner_kind(cd))
            acc.unit(design, name, name, g, "createdBy.owner", ca.get("name") or ca.get("sketch"),
                     cd.get("name") or cd.get("sketch"))


def owner_kind(c):
    k = c.get("kind")
    if k in ("sketch", "sketch_dimension"):
        return "sketch"
    if k == "feature" and c.get("objectType") == "Sketch":
        return "sketch"  # a sketch's own parameter (text height, pattern distance)
    if k == "feature" and c.get("objectType") == "ConstructionPlane":
        return "construction_plane"
    return k


# ---------------------------------------------------------------------------
# The accepted answers of the learning dump (census), as API fields
#
# A line's `answer.defs` are Mitcad feature definitions in the file's terms:
# timeline items as `T<index>` (the decoder's numbering), parameters by name,
# lengths in mm. Each function gives the fields the accepted answer carries,
# named and valued as the API's, so that the two compare directly. A field the
# answer does not carry is left out (not counted).

OPERATIONS = {"new_body": "NewBodyFeatureOperation", "join": "JoinFeatureOperation",
              "cut": "CutFeatureOperation", "intersect": "IntersectFeatureOperation"}
COMPUTE = {"adjust": "AdjustPatternCompute", "identical": "IdenticalPatternCompute",
           "optimized": "OptimizedPatternCompute"}
ORIGIN_AXES = {"x": "X", "y": "Y", "z": "Z"}
ORIGIN_PLANES = {"xy": "XY", "xz": "XZ", "yz": "YZ"}


def cen_param(name, api_c, ir_c):
    """The value (cm, rad) of a named parameter."""
    p = ir_c.params.get(name) or api_c.params.get(name)
    return p.get("value") if isinstance(p, dict) and is_num(p.get("value")) else None


def cen_item(ref, ir_c):
    """The decoder's item of a `T<index>` reference."""
    if isinstance(ref, str) and ref.startswith("T") and ref[1:].split(".")[0].isdigit():
        return ir_c.by_index.get(int(ref[1:].split(".")[0]))
    return None


def cen_def(line, kind):
    for d in (line.get("answer") or {}).get("defs") or []:
        if d.get("type") == kind:
            return d
    return None


def cen_profiles(line, d, ir_c):
    """`profile.*` fields of the answer's profiles: the regions Mitcad chose,
    by the sketch curves around them."""
    regions, sketches, areas = [], [], []
    ctx = (line.get("context") or {}).get("sketches") or {}
    for p in d.get("profiles") or []:
        it = cen_item(p.get("sketch"), ir_c)
        if it is None:
            continue
        sk = it.get("name")
        sketches.append(sk)
        reg = (ctx.get(p["sketch"]) or {}).get("regions") or []
        if 0 <= p.get("region", -1) < len(reg):
            r = reg[p["region"]]
            regions.append(region_of(ir_c, sk, [(True, r.get("outer") or [])]
                                     + [(False, h) for h in r.get("holes") or []]))
            if is_num(r.get("area")):
                areas.append(r["area"] / 100.0)
    out = {"profile.sketch": Bag(uniq(sketches)), "profile.count": len(regions),
           "profile.regions": Bag(regions)}
    if len(areas) == len(regions) and areas:
        out["profile.areas"] = Bag(areas)
    return out


def cen_axis(a, ir_c):
    """An axis reference of the answer as the API's entity."""
    if isinstance(a, str) and a in ORIGIN_AXES:
        return ("construction_axis", "origin", ORIGIN_AXES[a])
    if isinstance(a, dict) and "curve" in a and "sketch" in a:
        it = cen_item(a["sketch"], ir_c)
        if it is not None:
            return ("sketch_entity", it.get("name"), ir_c.sketch_id(it.get("name"), a["curve"]))
    return None


def cen_extrusion(line, api_c, ir_c):
    d = cen_def(line, "extrude")
    if d is None:
        return None
    out = {"operation": OPERATIONS.get(d.get("operation"))}
    e = d.get("extent") or {}
    t = e.get("type")
    kinds = {"distance": "DistanceExtentDefinition", "through_all": "ThroughAllExtentDefinition",
             "to_object": "ToEntityExtentDefinition", "symmetric": "SymmetricExtentDefinition"}
    if t in kinds:
        out["extentOne._type"] = kinds[t]
        out["extentType"] = {"symmetric": "SymmetricFeatureExtentType"}.get(t, "OneSideFeatureExtentType")
    if t == "two_sides":
        out["extentType"] = "TwoSidesFeatureExtentType"
        s1, s2 = e.get("side1") or {}, e.get("side2") or {}
        out["extentOne._type"] = kinds.get(s1.get("type"))
        out["extentTwo._type"] = kinds.get(s2.get("type"))
        out["extentOne.distance"] = cen_param(s1.get("distance"), api_c, ir_c)
        out["extentTwo.distance"] = cen_param(s2.get("distance"), api_c, ir_c)
    if t == "distance":
        out["extentOne.distance"] = cen_param(e.get("distance"), api_c, ir_c)
    out["taperAngleOne"] = cen_param(e.get("taper"), api_c, ir_c) if e.get("taper") else 0.0
    if t == "distance" and out.get("extentOne.distance") is not None:
        # a distance's sign is its side (the accepted `flip` does not add to it)
        out["direction"] = "negative" if out["extentOne.distance"] < 0 else "positive"
    if t == "through_all":
        out["direction"] = "negative" if d.get("flip") else "positive"
        out["extentOne.isPositiveDirection"] = not d.get("flip")
    out.update(cen_profiles(line, d, ir_c))
    out.update(cen_participants(d, ir_c))
    return out


def cen_participants(d, ir_c):
    """`participants`, where the answer names the bodies (`T<item>.b<k>`)."""
    parts = d.get("participants")
    if not parts or "new_body" in str(d.get("operation")):
        return {}
    out = []
    for r in parts:
        if isinstance(r, str) and r.startswith("T") and ".b" in r:
            item, k = r[1:].split(".b", 1)
            if item.isdigit() and k.isdigit():
                out.append((ir_c.index_map.get(int(item), int(item)), int(k)))
    return {"participants": Bag(out)} if out else {}


def cen_revolve(line, api_c, ir_c):
    d = cen_def(line, "revolve")
    if d is None:
        return None
    out = {"operation": OPERATIONS.get(d.get("operation")), "axis": cen_axis(d.get("axis"), ir_c)}
    e = d.get("extent") or {}
    if e.get("type") == "angle":
        out["extentDefinition.angle"] = cen_param(e.get("angle"), api_c, ir_c)
    out.update(cen_profiles(line, d, ir_c))
    out.update(cen_participants(d, ir_c))
    return {k: v for k, v in out.items() if v is not None or k == "axis"}


def cen_objects(d, ir_c):
    objs = d.get("objects") or {}
    if objs.get("type") == "features":
        names = [(cen_item(r, ir_c) or {}).get("name") for r in objs.get("features") or []]
        return {"inputEntities": Bag([("feature", n) for n in names]),
                "patternEntityType": "FeaturesPatternType"}
    if objs.get("type") == "bodies":
        return {"patternEntityType": "BodiesPatternType"}
    return {}


def cen_circular(line, api_c, ir_c):
    d = cen_def(line, "circular_pattern")
    if d is None:
        return None
    out = {"patternComputeOption": COMPUTE.get(d.get("compute")), "isSymmetric": d.get("symmetric"),
           "quantity": float(d["quantity"]) if is_num(d.get("quantity")) else None,
           "totalAngle": cen_param(d.get("angle"), api_c, ir_c), "axis": cen_axis(d.get("axis"), ir_c)}
    a = d.get("axis")
    if isinstance(a, str) and a in ORIGIN_AXES:
        out["axis.line"] = ("axisline", ORIGIN_AXIS_DIRS[ORIGIN_AXES[a]], (0.0, 0.0, 0.0))
    elif isinstance(a, dict) and "direction" in a and "origin" in a:
        out["axis.line"] = ("axisline", sign_free(vec(a["direction"])),
                            foot(tuple(x / 10.0 for x in a["origin"]), a["direction"]))
    out.update(cen_objects(d, ir_c))
    return {k: v for k, v in out.items() if v is not None or k == "axis"}


def cen_rectangular(line, api_c, ir_c):
    d = cen_def(line, "rectangular_pattern")
    if d is None:
        return None
    out = {"patternComputeOption": COMPUTE.get(d.get("compute")),
           "patternDistanceType": {"spacing": "SpacingPatternDistanceType",
                                   "extent": "ExtentPatternDistanceType"}.get(d.get("distance_type"))}
    for n, key in (("One", "direction1"), ("Two", "direction2")):
        dd = d.get(key)
        if not isinstance(dd, dict):
            continue
        out["quantity" + n] = float(dd["quantity"]) if is_num(dd.get("quantity")) else None
        out["distance" + n] = cen_param(dd.get("distance"), api_c, ir_c)
        if "symmetric" in dd:
            out["isSymmetricInDirection" + n] = dd["symmetric"]
        ax = cen_axis(dd.get("axis"), ir_c)
        if ax is not None:
            out["direction%sEntity" % n] = ax
        if isinstance(dd.get("axis"), str) and dd["axis"] in ORIGIN_AXES:
            out["direction%s.line" % n] = ("dir", ORIGIN_AXIS_DIRS[ORIGIN_AXES[dd["axis"]]])
    out.update(cen_objects(d, ir_c))
    return {k: v for k, v in out.items() if v is not None}


def cen_mirror(line, api_c, ir_c):
    d = cen_def(line, "mirror")
    if d is None:
        return None
    out = {"patternComputeOption": COMPUTE.get(d.get("compute"))}
    if "combine" in d:
        out["isCombine"] = d["combine"]
    pl = d.get("plane")
    if isinstance(pl, str) and pl in ORIGIN_PLANES:
        out["mirrorPlane"] = ("construction_plane", "origin", ORIGIN_PLANES[pl])
    elif isinstance(pl, str):
        it = cen_item(pl, ir_c)
        g = ((it or {}).get("detail") or {}).get("geometry")
        if isinstance(g, dict):
            out["mirrorPlane"] = ("construction_plane", geometry_key(g))
    out.update(cen_objects(d, ir_c))
    if "mirrorPlane" in out:
        out["mirrorPlane"] = plane_sign_free(out["mirrorPlane"])
    return {k: v for k, v in out.items() if v is not None}


def cen_edge(name, line):
    e = ((line.get("context") or {}).get("edges") or {}).get(name)
    if isinstance(e, dict) and e.get("mid") is not None and e.get("length") is not None:
        return ("edge", tuple(x / 10.0 for x in e["mid"]), e["length"] / 10.0)
    return None


def cen_dressup_sets(line, kind):
    """The sets of every definition of the answer: an answer may be a
    feature per tangent chain (or per body), and these are one input of the
    file's feature. Sets of the same size are joined across definitions."""
    defs = [d for d in (line.get("answer") or {}).get("defs") or [] if d.get("type") == kind]
    if len(defs) <= 1:
        return defs[0] if defs else None, (defs[0].get("sets") or []) if defs else []
    sets, order = {}, []
    for d in defs:
        for s in d.get("sets") or []:
            key = json.dumps([s.get("size"), s.get("tangent_chain"), s.get("flip"), s.get("continuity")], sort_keys=True)
            if key not in sets:
                sets[key] = dict(s, edges=[])
                order.append(key)
            sets[key]["edges"].extend(s.get("edges") or [])
    return defs[0], [sets[k] for k in order]


def cen_dressup(kind):
    def f(line, api_c, ir_c):
        d, sets = cen_dressup_sets(line, kind)
        if d is None:
            return None
        edges, chains, flips = [], [], []
        for s in sets:
            sz = s.get("size") or {}
            if kind == "fillet":
                size = (("radius", cen_param(sz.get("radius"), api_c, ir_c)),)
            elif sz.get("type") == "equal_distance":
                size = (("distance", cen_param(sz.get("distance"), api_c, ir_c)),)
            elif sz.get("type") == "two_distances":
                size = (("distanceOne", cen_param(sz.get("distance1"), api_c, ir_c)),
                        ("distanceTwo", cen_param(sz.get("distance2"), api_c, ir_c)))
            elif sz.get("type") == "distance_angle":
                size = (("distance", cen_param(sz.get("distance"), api_c, ir_c)),
                        ("angle", cen_param(sz.get("angle"), api_c, ir_c)))
            else:
                size = ()
            for n in s.get("edges") or []:
                edges.append((cen_edge(n, line), size))
            chains.append(s.get("tangent_chain"))
            if sz.get("type") == "two_distances":
                flips.append(bool(s.get("flip")))
        out = {"edgeSets.count": len(sets), "edges.count": len(edges), "edges": Bag(edges),
               "edgeSets.isTangentChain": Bag([c for c in chains if c is not None])}
        if flips:
            out["edgeSets.isFlipped"] = Bag(flips)
        if kind == "fillet" and "rolling_ball_corners" in d:
            out["isRollingBallCorner"] = d["rolling_ball_corners"]
        return out
    return f


LOCATIONS = {"high_end": "HighEndThreadLocation", "low_end": "LowEndThreadLocation"}


def cen_thread(line, api_c, ir_c):
    d = cen_def(line, "thread")
    if d is None:
        return None
    th = d.get("thread") or {}
    out = {}
    if th.get("designation"):
        out["threadInfo.threadDesignation"] = th["designation"]
    if th.get("class"):
        out["threadInfo.threadClass"] = th["class"]
    if "modeled" in d:
        out["isModeled"] = d["modeled"]
    if d.get("location") in LOCATIONS:
        out["threadLocation"] = LOCATIONS[d["location"]]
    if d.get("length"):
        out["threadLength"] = cen_param(d["length"], api_c, ir_c)
    if d.get("offset"):
        out["threadOffset"] = cen_param(d["offset"], api_c, ir_c)
    dm = d.get("diameters") or {}
    for k, f in (("major", "majorDiameter"), ("minor", "minorDiameter"), ("pitch", "pitchDiameter")):
        if is_num(dm.get(k)):
            out["threadInfo." + f] = dm[k] / 10.0
    return out


def cen_hole(line, api_c, ir_c):
    d = cen_def(line, "hole")
    if d is None:
        return None
    e = d.get("extent") or {}
    out = {"holeDiameter": cen_param(d.get("diameter"), api_c, ir_c)}
    if e.get("type") == "through_all":
        out["extentDefinition._type"] = "AllExtentDefinition"
    elif e.get("type") == "distance":
        out["extentDefinition._type"] = "DistanceExtentDefinition"
    return out


def cen_combine(line, api_c, ir_c):
    d = cen_def(line, "combine")
    if d is None:
        return None
    return {"operation": OPERATIONS.get(d.get("operation")), "isKeepToolBodies": d.get("keep_tools")}


def cen_shell(line, api_c, ir_c):
    d = cen_def(line, "shell")
    if d is None:
        return None
    return {"insideThickness": cen_param(d.get("inside"), api_c, ir_c),
            "isTangentChain": d.get("tangent_chain"),
            "shellType": "RoundedOffsetShellType" if d.get("rounded") else "SharpOffsetShellType"}


CENSUS_FIELDS = {
    "extrusion": cen_extrusion, "revolve": cen_revolve, "pattern.circular": cen_circular,
    "pattern.rectangular": cen_rectangular, "mirror": cen_mirror, "fillet": cen_dressup("fillet"),
    "chamfer": cen_dressup("chamfer"), "thread": cen_thread, "hole": cen_hole, "combine": cen_combine,
    "shell": cen_shell,
}


def census_for(group, line, api_c, ir_c):
    """{field: value} of the accepted answer of a settled line, or None."""
    fn = CENSUS_FIELDS.get(group)
    if fn is None or line is None or line.get("status") != "settled" or not line.get("answer"):
        return None
    return fn(line, api_c, ir_c)


# ---------------------------------------------------------------------------
# One design


def compare_design(task):
    id_, api_path, ir_path, source, census_dir, root, only = task
    acc = Acc()
    items_rows = []
    api = load_json(api_path)
    ir = load_json(ir_path)
    api_c, ir_c = Canon(api, "api"), Canon(ir, "ir")
    lines = census_lines(census_dir, os.path.join(root, source) if source else None)
    by_item = defaultdict(list)
    for ln in lines:
        by_item[ln.get("item")].append(ln)
    pairs = align(api_c.items, ir_c.items)
    ir_c.index_map = {ii.get("index"): ai.get("index") for ai, ii in pairs if ai and ii}
    for ai, ii in pairs:
        items_rows.append((id_, ai.get("index") if ai else "", ai.get("objectType") if ai else "",
                           ai.get("name") if ai else "", ii.get("index") if ii else "",
                           ii.get("objectType") if ii else "", ii.get("name") if ii else ""))
        if ai is None or ii is None:
            acc.notes["items_unpaired_api" if ai else "items_unpaired_decoder"] += 1
            continue
        acc.notes["items_paired"] += 1
        idx, name = ai.get("index"), ai.get("name")
        if not only or "timeline" in only:
            for f, v in f_timeline(api_c, ai, "api").items():
                acc.unit(id_, idx, name, "timeline", f, v, f_timeline(ir_c, ii, "ir").get(f))
        t = ai.get("objectType")
        group = GROUP_OF_TYPE.get(t)
        if group and (not only or group in only):
            fn = ITEM_GROUPS[group][1]
            fa, fd = fn(api_c, ai, "api"), fn(ir_c, ii, "ir")
            cen_lines = [l for l in by_item.get(ii.get("index"), ()) if l.get("type") == ii.get("objectType")]
            cen = census_for(group, cen_lines[0], api_c, ir_c) if cen_lines else None
            if group in ("fillet", "chamfer"):
                dressup_edges(fa, fd, cen)
            if "profile.unresolved" in fa:
                # the application names no curves of some profiles: the regions cannot be compared by curves
                fd.pop("profile.regions", None)
                (cen or {}).pop("profile.regions", None)
            # values the application's interface could not convert (an object handle)
            unconverted = {k for k, v in (ai.get("detail") or {}).items()
                           if isinstance(v, str) and v.startswith("<") and " proxy of " in v}
            for f in sorted(k for k in set(fa) | set(fd) | set(cen or ()) if not k.startswith("_")):
                if f in unconverted and fa.get(f) is None:
                    acc.notes["api_unconverted"] += 1
                    continue
                acc.unit(id_, idx, name, group, f, fa.get(f), fd.get(f),
                         (cen[f] if f in cen else NOCEN) if cen is not None else NOCEN)
            if "_regions" in fa or "_regions" in fd:
                region_units(acc, id_, idx, name, group, fa, fd)
        elif t == "Sketch" and (not only or any(g.startswith("sketch") for g in only)):
            sketch_units(acc, id_, api_c, ir_c, ai, ii)
    if not only or "parameters" in only:
        parameter_units(acc, id_, api_c, ir_c)
    return id_, acc, items_rows


# ---------------------------------------------------------------------------
# The run and its report


def designs_of(args):
    index = {}
    if args.index:
        with open(args.index, encoding="utf-8") as f:
            for line in f.read().splitlines()[1:]:
                cols = line.split("\t")
                if len(cols) > 1:
                    index[cols[0]] = cols[1]
    tasks = []
    for p in sorted(glob.glob(os.path.join(args.api, "*.json.gz"))):
        id_ = os.path.basename(p)[: -len(".json.gz")]
        ir = os.path.join(args.ir, id_ + ".json")
        if os.path.exists(ir) and os.path.getsize(ir) > 0:
            tasks.append((id_, p, ir, index.get(id_), args.census, args.root or "",
                          set(args.group) if args.group else None))
    return tasks


def table(acc, group, with_census):
    keys = sorted(k for k in set(acc.counts) | set(acc.api_n) if k[0] == group)
    if not keys:
        return []
    head = ["field", "API", "agree", "differs", "dec_miss", "api_miss"]
    if with_census:
        head += ["c_agree", "c_differs", "c_miss", "c_extra"]
    head.append("API values")
    rows = [head]
    for k in keys:
        c = acc.counts[k]
        vals = acc.api_vals.get(k)
        top = ", ".join("%s x%d" % (v, n) for v, n in vals.most_common(3)) if vals else ""
        row = [k[1], str(acc.api_n[k])] + [str(c[s]) for s in STATES]
        if with_census:
            row += [str(c[s]) if any(c[x] for x in CSTATES) else "" for s in CSTATES]
        row.append(top)
        rows.append(row)
    return rows


def render(rows):
    widths = [max(len(r[i]) for r in rows) for i in range(len(rows[0]) - 1)]
    out = []
    for n, r in enumerate(rows):
        out.append("  " + "  ".join(c.rjust(w) if i else c.ljust(w) for i, (c, w) in enumerate(zip(r, widths)))
                   + "  " + r[-1])
        if n == 0:
            out.append("  " + "  ".join("-" * w for w in widths))
    return "\n".join(out)


def write_report(acc, items_rows, out_dir, n_designs, with_census, groups):
    os.makedirs(os.path.join(out_dir, "mismatches"), exist_ok=True)
    lines = ["API reference comparison: %d designs, %d items paired, %d only in the reference, "
             "%d only in the decoder's dump" % (n_designs, acc.notes["items_paired"],
                                                acc.notes["items_unpaired_api"],
                                                acc.notes["items_unpaired_decoder"]), ""]
    over = [["group", "fields", "API values", "agree", "differs", "dec_miss", "api_miss", ""]]
    if with_census:
        over[0][-1:] = ["c_agree", "c_differs", ""]
    for g in groups:
        keys = [k for k in acc.counts if k[0] == g]
        if not keys:
            continue
        tot = Counter()
        for k in keys:
            tot.update(acc.counts[k])
        row = [g, str(len(keys)), str(sum(acc.api_n[k] for k in keys))] + [str(tot[x]) for x in STATES]
        if with_census:
            row += [str(tot["c_agree"]), str(tot["c_differs"])]
        over.append(row + [""])
    lines += ["== overview (every field counted once per item or element)", render(over), ""]
    for g in groups:
        t = table(acc, g, with_census)
        if t:
            lines += ["== %s" % g, render(t), ""]
    if acc.notes["api_unconverted"]:
        lines.append("(%d fields of the reference are object handles its interface could not convert and are "
                     "not compared)" % acc.notes["api_unconverted"])
    if acc.notes["parameters_sections_failed"]:
        lines.append("(%d designs whose reference has no parameters section (the dump failed there) are left "
                     "out of the parameters group)" % acc.notes["parameters_sections_failed"])
    if acc.notes["driven_dimension_values"]:
        lines.append("(%d driven dimensions' values and expressions are measurements of the sketch as it is "
                     "now, not compared)" % acc.notes["driven_dimension_values"])
    if acc.notes["parameters_linked_designs"]:
        lines.append("(%d parameters of the reference belong to other designs the file links to and are "
                     "left out of the parameters group)" % acc.notes["parameters_linked_designs"])
    if acc.notes["rows_dropped"]:
        lines.append("(%d mismatch rows beyond --max-rows per field are not listed)" % acc.notes["rows_dropped"])
    with open(os.path.join(out_dir, "summary.txt"), "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    by_group = defaultdict(list)
    for (g, _), rows in sorted(acc.rows.items()):
        by_group[g].extend(rows)
    for g, rows in by_group.items():
        with open(os.path.join(out_dir, "mismatches", g + ".tsv"), "w", encoding="utf-8") as f:
            f.write("design\titem\tname\tfield\tsource\tstate\tsource_value\tapi_value\n")
            for r in rows:
                f.write("\t".join(str(c).replace("\t", " ").replace("\n", " ") for c in r) + "\n")
    with open(os.path.join(out_dir, "items.tsv"), "w", encoding="utf-8") as f:
        f.write("design\tapi_index\tapi_type\tapi_name\tdecoder_index\tdecoder_type\tdecoder_name\n")
        for r in items_rows:
            f.write("\t".join(str(c).replace("\t", " ") for c in r) + "\n")
    return "\n".join(lines)


def cmd_compare(args):
    tasks = designs_of(args)
    if not tasks:
        sys.exit("no designs: %s with %s" % (args.api, args.ir))
    acc = Acc()
    items_rows = []
    jobs = args.jobs or max(1, (os.cpu_count() or 2) - 4)
    if jobs > 1:
        with ProcessPoolExecutor(max_workers=jobs) as ex:
            results = list(ex.map(compare_design, tasks, chunksize=2))
    else:
        results = [compare_design(t) for t in tasks]
    for _, a, rows in results:
        acc.merge(a, args.max_rows)
        items_rows.extend(rows)
    text = write_report(acc, items_rows, args.out, len(tasks), bool(args.census),
                        [g for g in ALL_GROUPS if not args.group or g in args.group])
    print(text)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("compare", help="compare the decoder with the reference dumps")
    c.add_argument("--api", required=True, help="folder of <id>.json.gz reference dumps")
    c.add_argument("--ir", required=True, help="folder of <id>.json decoder dumps")
    c.add_argument("--out", required=True)
    c.add_argument("--census", help="the learning dump (settled/, unsettled/)")
    c.add_argument("--index", help="TSV with id and source columns (the reference dumps' index.tsv)")
    c.add_argument("--root", help="folder the index's sources are relative to")
    c.add_argument("--group", action="append", help="only this group (repeatable)")
    c.add_argument("--max-rows", type=int, default=400, help="mismatch rows kept per field (0: all)")
    c.add_argument("--jobs", type=int, default=0)
    c.set_defaults(fn=cmd_compare)
    g = sub.add_parser("groups", help="list the input groups")
    g.set_defaults(fn=lambda a: print("\n".join(ALL_GROUPS)))
    args = ap.parse_args(argv)
    args.fn(args)


if __name__ == "__main__":
    main()
