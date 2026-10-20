# SPDX-License-Identifier: MIT
"""Builds the BUILD parts of Mitcad's parts library (mitcad-parts;
mitcad#375): the BUILD tab's slot profiles, connectors, nuts, caps, panels
and linear parts as parametric Mitcad designs, with the library's
manifest, previews, licence, sources and README. With --merge the
library's other parts already in the manifest (the fasteners;
tools/libraries/make-parts-library.py runs both generators) stay. With
--commit the folder becomes a git repository with the version recorded and
tagged.

    python3 make-profile-library.py --cli build/dev/tools/cli/mitcad-cli \\
        --source <folder of the supplier's files> --out ../libraries/mitcad-parts \\
        [--groups tools/libraries/profiles] [--only id,id] [--version 1.0.0] [--merge] [--commit]

The parts are listed in group files (tools/libraries/profiles/*.json; the
format is in docs/libraries.md, "Mitcad's parts library"). Each part is
made one of three ways:

- section: the cross-section at the source's end face, placed on XY at its
  centre, as a sketch extruded by the length parameter along +Z (end A at
  z = 0, end B at z = L);
- solid: the source's solids as a base feature, moved to Mitcad's
  coordinates with `place`;
- model: no source; the design is built from `dimensions` (a plate, an
  angle, a rod with a cosmetic thread) for sheets, plates, shafts and
  screws sold without a CAD file.

Mount frames are joint origins named mount:<name>: found from a profile's
section ({"auto": "profile"}) or listed. The supplier's files are read
from --source and never copied into the library: only the parametric
designs made from them are.

mitcad-cli does the modelling: `import` reads a source, and `mcp
--workspace` (the Model Context Protocol over stdin and stdout, without a
UI) runs the commands and queries the generator needs answers to (the
section's curves, the profile regions' keys).
"""

import argparse
import copy
import glob
import json
import math
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

LIBRARY_ID = "mitcad-parts"
LIBRARY_NAME = "Mitcad parts"
LIBRARY_DESCRIPTION = ("Mitcad's parts: ISO metric screws, nuts and washers, and slot profiles, connectors, nuts, "
                       "caps, panels and linear parts for the BUILD tab.")
LICENSE = "MIT"
PROTOCOL = "2025-11-25"
# Aluminium, kg per mm3: a profile's mass per metre when the group file
# gives none.
ALUMINIUM = 2.7e-6
MAKES = ("section", "solid", "model")
HERE = os.path.dirname(os.path.abspath(__file__))


class PartError(Exception):
    """A part that cannot be made: reported by its id and left out."""


def fail(message):
    """A broken group file or another reason to stop the whole run."""
    sys.exit(f"make-profile-library: {message}")


def mm(value):
    """An expression in millimetres: `8.5 mm`."""
    text = f"{value:.4f}".rstrip("0").rstrip(".")
    if text in ("-0", ""):
        text = "0"
    return f"{text} mm"


def length_of(value, what):
    """A length of a group file (a number in mm or text with a unit), in mm."""
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        text = value.strip()
        for unit, factor in (("mm", 1.0), ("cm", 10.0), ("m", 1000.0), ("in", 25.4)):
            if text.endswith(unit):
                try:
                    return float(text[: -len(unit)].strip().replace(",", ".")) * factor
                except ValueError:
                    break
        try:
            return float(text.replace(",", "."))
        except ValueError:
            pass
    raise PartError(f"{what} is no length: {value!r}")


def run(command, cwd=None, check=True):
    result = subprocess.run([str(c) for c in command], cwd=cwd, capture_output=True, text=True,
                            encoding="utf-8", errors="replace")
    if check and result.returncode != 0:
        # One line: the exit status (a crash's code in hex, as Windows
        # gives it) and the last lines the program wrote.
        status = result.returncode
        code = f"{status} (0x{status & 0xFFFFFFFF:08X})" if status < 0 or status > 255 else str(status)
        lines = [line.strip() for line in f"{result.stdout}\n{result.stderr}".splitlines() if line.strip()]
        raise PartError(f"{' '.join(map(str, command))} failed with exit status {code}: "
                        f"{' | '.join(lines[-8:]) or 'no output'}")
    return result


def write(path, text):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


# ---------------------------------------------------------------------
# mitcad-cli mcp: a session of documents with commands and queries.

class Mcp:
    def __init__(self, cli, workspace):
        self.stderr = tempfile.TemporaryFile()
        self.process = subprocess.Popen([cli, "mcp", "--workspace", workspace], stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.stderr)
        self.next_id = 0
        self.request("initialize", {"protocolVersion": PROTOCOL, "capabilities": {},
                                    "clientInfo": {"name": "make-profile-library", "version": "1"}})
        self.notify("notifications/initialized")

    def diagnostics(self):
        self.stderr.seek(0)
        return self.stderr.read().decode("utf-8", "replace")

    def send(self, message):
        self.process.stdin.write((json.dumps(message, separators=(",", ":")) + "\n").encode("utf-8"))
        self.process.stdin.flush()

    def notify(self, method, params=None):
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        self.send(message)

    def request(self, method, params):
        self.next_id += 1
        self.send({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
        while True:
            line = self.process.stdout.readline()
            if not line:
                fail("mitcad-cli mcp stopped:\n" + self.diagnostics())
            message = json.loads(line)
            if message.get("id") == self.next_id:
                break
        if "error" in message:
            raise PartError(f"{method}: {message['error'].get('message', message['error'])}")
        return message["result"]

    def tool(self, name, arguments):
        result = self.request("tools/call", {"name": name, "arguments": arguments})
        content = result.get("structuredContent")
        if content is None:
            content = json.loads(result["content"][0]["text"])
        if result.get("isError"):
            message = content.get("error") if isinstance(content, dict) else None
            if not message and isinstance(content, dict):
                failed = [f"{f.get('name')}: {f.get('error')}" for f in
                          content.get("timeline", {}).get("features", []) if f.get("status") == "error"]
                message = "; ".join(failed) or json.dumps(content)[:400]
            raise PartError(f"{name}: {message}")
        return content

    # A blank design in the workspace, as mitcad-cli makes one: its root
    # takes the features. MCP's document_create makes a new design as File
    # > New does (the root an assembly, the features going into a component
    # Part1), which a library's design must not be.
    BLANK = "blank.mitcad"

    def create(self):
        return self.tool("document_open", {"path": self.BLANK})["document"]

    def open(self, path):
        return self.tool("document_open", {"path": path})["document"]

    def close(self, document):
        try:
            self.tool("document_close", {"document": document, "discard": True})
        except PartError:
            pass

    def command(self, document, command):
        """Runs a command; its result, and the timeline after it."""
        content = self.tool("model_command", {"document": document, "command": command})
        return content.get("result"), content.get("timeline", {})

    def add(self, document, command):
        """Adds a feature; its uid."""
        result, timeline = self.command(document, command)
        if isinstance(result, dict) and result.get("uid"):
            return result["uid"]
        features = timeline.get("features") or []
        if not features:
            raise PartError(f"{command.get('cmd')} added no feature")
        return features[-1]["uid"]

    def query(self, document, query):
        return self.tool("model_query", {"document": document, "query": query})["result"]

    def save(self, document, path):
        self.tool("document_save", {"document": document, "path": path, "overwrite": True})

    def finish(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=60)
        except (OSError, subprocess.TimeoutExpired):
            self.process.kill()


# ---------------------------------------------------------------------
# Geometry of sections: curves in the section's plane, centred.

def dot(a, b):
    return sum(a[i] * b[i] for i in range(len(a)))


def bspline_point(degree, control, weights, knots, t):
    """A point of a (rational) B-spline by de Boor's algorithm."""
    n = len(control)
    if not weights:
        weights = [1.0] * n
    if not knots:
        inner = max(n - degree - 1, 0)
        knots = [0.0] * (degree + 1) + [(i + 1) / (inner + 1) for i in range(inner)] + [1.0] * (degree + 1)
    span = degree
    while span < n - 1 and t >= knots[span + 1]:
        span += 1
    d = [[control[j][0] * weights[j], control[j][1] * weights[j], weights[j]]
         for j in range(span - degree, span + 1)]
    for r in range(1, degree + 1):
        for j in range(degree, r - 1, -1):
            i = span - degree + j
            denominator = knots[i + degree - r + 1] - knots[i]
            alpha = 0.0 if denominator == 0 else (t - knots[i]) / denominator
            d[j] = [(1 - alpha) * d[j - 1][k] + alpha * d[j][k] for k in range(3)]
    w = d[degree][2] or 1.0
    return (d[degree][0] / w, d[degree][1] / w)


def bspline_range(degree, control, knots):
    if not knots:
        return 0.0, 1.0
    return knots[degree], knots[len(control)]


def polyline(curve, samples=48):
    """Points along a curve, for bounds and previews."""
    kind = curve["type"]
    if kind == "line":
        return [curve["start"], curve["end"]]
    if kind in ("circle", "arc"):
        cx, cy = curve["center"]
        r = curve["radius"]
        if kind == "circle":
            a0, a1 = 0.0, 2 * math.pi
        else:
            a0 = math.atan2(curve["start"][1] - cy, curve["start"][0] - cx)
            a1 = math.atan2(curve["end"][1] - cy, curve["end"][0] - cx)
            while a1 <= a0:
                a1 += 2 * math.pi
        steps = max(4, int(samples * (a1 - a0) / (2 * math.pi)))
        return [(cx + r * math.cos(a0 + (a1 - a0) * i / steps), cy + r * math.sin(a0 + (a1 - a0) * i / steps))
                for i in range(steps + 1)]
    if kind == "spline":
        lo, hi = bspline_range(curve["degree"], curve["control"], curve["knots"])
        return [bspline_point(curve["degree"], curve["control"], curve["weights"], curve["knots"],
                              lo + (hi - lo) * i / samples) for i in range(samples + 1)]
    raise PartError(f"a {kind} in the section")


def bounds(curves):
    xs, ys = [], []
    for curve in curves:
        for x, y in polyline(curve):
            xs.append(x)
            ys.append(y)
    if not xs:
        raise PartError("the section has no curves")
    return min(xs), min(ys), max(xs), max(ys)


def transform_curves(curves, function, mirrored):
    """Curves with every point moved by `function`; arcs keep running
    counter-clockwise when the map mirrors."""
    out = []
    for curve in curves:
        c = dict(curve)
        for key in ("start", "end", "center"):
            if key in c:
                c[key] = function(c[key])
        if "control" in c:
            c["control"] = [function(p) for p in c["control"]]
        if c["type"] == "arc" and mirrored:
            c["start"], c["end"] = c["end"], c["start"]
        out.append(c)
    return out


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def unit(vector, what):
    try:
        v = [float(c) for c in vector]
    except (TypeError, ValueError):
        v = []
    if len(v) != 3 or dot(v, v) < 1e-12:
        fail(f"{what} is no direction [x, y, z]")
    n = math.sqrt(dot(v, v))
    return [c / n for c in v]


def section_axes(entry, extents):
    """The part's x, y and z axes in the source's coordinates, and the
    source's axis the part runs along. `place` {"z", "x"} names them;
    without it the part runs along the source's longest extent (or
    `axis`), x and y along the next axes in turn."""
    place = entry.get("place") or {}
    if "z" in place:
        z = unit(place["z"], f"{entry['id']}: place.z")
        axis = max(range(3), key=lambda i: abs(z[i]))
        if abs(abs(z[axis]) - 1) > 1e-6:
            fail(f"{entry['id']}: place.z is not along a model axis")
        x = unit(place.get("x", [1 if i == (axis + 1) % 3 else 0 for i in range(3)]), f"{entry['id']}: place.x")
        if abs(dot(x, z)) > 1e-6:
            fail(f"{entry['id']}: place.x is not square to place.z")
        return axis, x, cross(z, x), z
    axes = {"x": 0, "y": 1, "z": 2}
    axis = axes.get(entry.get("axis", ""), max(range(3), key=lambda i: extents[i]))
    e = [[1.0 if i == j else 0.0 for i in range(3)] for j in range(3)]
    return axis, e[(axis + 1) % 3], e[(axis + 2) % 3], e[axis]


def section_curves(sketch, part_x, part_y):
    """The projected curves of a sketch on an end face, in the section's
    coordinates: along the part's x and y axes (in the source's
    coordinates), so that the part's axis is +z."""
    frame = sketch.get("frame")
    if not frame:
        raise PartError("the sketch on the end face did not evaluate")
    origin, xa, ya = frame["origin"], frame["x_axis"], frame["y_axis"]

    def mapped(p):
        point = [origin[i] + p[0] * xa[i] + p[1] * ya[i] for i in range(3)]
        return (dot(point, part_x), dot(point, part_y))

    mirrored = dot(xa, part_x) * dot(ya, part_y) - dot(ya, part_x) * dot(xa, part_y) < 0
    curves = []
    for entity in sketch.get("entities", []):
        kind = entity.get("type")
        geometry = entity.get("geometry")
        if kind == "point" or geometry is None or entity.get("construction"):
            continue
        if kind == "line":
            curves.append({"type": "line", "start": geometry["start"], "end": geometry["end"]})
        elif kind == "circle":
            curves.append({"type": "circle", "center": geometry["center"], "radius": geometry["radius"]})
        elif kind == "arc":
            curves.append({"type": "arc", "center": geometry["center"], "radius": geometry["radius"],
                           "start": geometry["start"], "end": geometry["end"]})
        elif kind == "spline":
            curves.append({"type": "spline", "degree": geometry["degree"], "control": geometry["control"],
                           "weights": geometry.get("weights") or [], "knots": geometry.get("knots") or []})
        else:
            raise PartError(f"the section has a {kind}, which the generator does not read")
    return transform_curves(curves, mapped, mirrored)


def centred(curves, place):
    """The curves with the centre of their bounds at the origin, then
    turned by place.rotate (degrees) and moved by place.shift; and that
    map of the section's points."""
    x0, y0, x1, y1 = bounds(curves)
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    angle = math.radians(float(place.get("rotate", 0)))
    dx, dy = [float(v) for v in place.get("shift", [0, 0])]
    c, s = math.cos(angle), math.sin(angle)

    def moved(p):
        x, y = p[0] - cx, p[1] - cy
        x, y = c * x - s * y + dx, s * x + c * y + dy
        return (round(x, 9) + 0.0, round(y, 9) + 0.0)

    return transform_curves(curves, moved, False), moved


def sketch_definition(curves):
    """A sketch on XY holding the curves as fixed geometry: ends shared
    where curves meet, so that the loops are closed."""
    entities = []
    ends = {}
    counter = [0]

    def new_id(prefix):
        counter[0] += 1
        return f"{prefix}{counter[0]}"

    def point(p, shared=True):
        key = (round(p[0], 6), round(p[1], 6))
        if shared and key in ends:
            return ends[key]
        uid = new_id("p")
        entities.append({"id": uid, "type": "point", "at": [p[0], p[1]], "fixed": True})
        if shared:
            ends[key] = uid
        return uid

    for curve in curves:
        kind = curve["type"]
        if kind == "line":
            a, b = point(curve["start"]), point(curve["end"])
            if a == b:
                continue
            entities.append({"id": new_id("c"), "type": "line", "start": a, "end": b, "fixed": True})
        elif kind == "circle":
            centre = point(curve["center"], shared=False)
            entities.append({"id": new_id("c"), "type": "circle", "center": centre, "radius": curve["radius"],
                             "fixed": True})
        elif kind == "arc":
            centre = point(curve["center"], shared=False)
            a, b = point(curve["start"]), point(curve["end"])
            entities.append({"id": new_id("c"), "type": "arc", "center": centre, "start": a, "end": b,
                             "fixed": True})
        elif kind == "spline":
            control = curve["control"]
            ids = [point(control[0])] + [point(p, shared=False) for p in control[1:-1]] + [point(control[-1])]
            spline = {"id": new_id("c"), "type": "spline", "degree": curve["degree"], "control": ids,
                      "fixed": True}
            if curve["weights"]:
                spline["weights"] = curve["weights"]
            if curve["knots"]:
                spline["knots"] = curve["knots"]
            entities.append(spline)
    return {"type": "sketch", "plane": "xy", "entities": entities}


# ---------------------------------------------------------------------
# Frames of a profile found from its section.

def profile_frames(curves, tolerance=1e-3):
    """The ends, the core bores and the slots of a centred section, named
    by the layout of docs/libraries.md: (name, kind, origin, z, x,
    extra)."""
    x0, y0, x1, y1 = bounds(curves)
    frames = []
    # Bores: circles inside the section, the centred one `core`, others
    # core_<n> from the lower coordinate.
    circles = [c for c in curves if c["type"] == "circle"]
    smaller = min(x1 - x0, y1 - y0) / 3
    bores = [c for c in circles if c["radius"] < smaller]
    centre = [c for c in bores if math.hypot(*c["center"]) < 0.05]
    if centre:
        bore = min(centre, key=lambda c: c["radius"])
        frames.append(("core", "bore", [0.0, 0.0, 0.0], [0, 0, -1], [1, 0, 0],
                       {"diameter": mm(2 * bore["radius"])}))
    else:
        others = sorted(bores, key=lambda c: (round(c["center"][1], 3), round(c["center"][0], 3)))
        for n, bore in enumerate(others, 1):
            frames.append((f"core_{n}", "bore", [bore["center"][0], bore["center"][1], 0.0], [0, 0, -1], [1, 0, 0],
                           {"diameter": mm(2 * bore["radius"])}))
    # Slots: gaps between the straight pieces of the outline on each side.
    openings = []
    lines = [c for c in curves if c["type"] == "line"]
    for side, along, at, normal in (("x+", 1, x1, [1, 0, 0]), ("x-", 1, x0, [-1, 0, 0]),
                                    ("y+", 0, y1, [0, 1, 0]), ("y-", 0, y0, [0, -1, 0])):
        across = 1 - along
        pieces = sorted((min(l["start"][along], l["end"][along]), max(l["start"][along], l["end"][along]))
                        for l in lines
                        if abs(l["start"][across] - at) < tolerance and abs(l["end"][across] - at) < tolerance)
        merged = []
        for lo, hi in pieces:
            if merged and lo <= merged[-1][1] + tolerance:
                merged[-1][1] = max(merged[-1][1], hi)
            else:
                merged.append([lo, hi])
        gaps = [(a[1], b[0]) for a, b in zip(merged, merged[1:]) if b[0] - a[1] > tolerance]
        for n, (lo, hi) in enumerate(gaps, 1):
            middle = (lo + hi) / 2
            origin = [at, middle, 0.0] if side[0] == "x" else [middle, at, 0.0]
            frames.append((f"slot_{side}_{n}", "slot", origin, normal, [0, 0, 1], {}))
            openings.append(hi - lo)
    return frames, openings


def listed_frames(entries, what):
    out = []
    for i, frame in enumerate(entries):
        if not isinstance(frame, dict) or not isinstance(frame.get("name"), str):
            fail(f"{what}: frames[{i}] has no name")
        name = frame["name"]
        extra = {k: frame[k] for k in ("diameter", "accepts") if k in frame}
        if name in ("end_a", "end_b") and "origin" not in frame:
            out.append((name, frame.get("kind", "end"), None, None, None, extra))
            continue
        try:
            origin = frame["origin"]
            if len(origin) != 3 or not all(isinstance(v, (int, float, str)) for v in origin):
                raise TypeError
            origin = [v if isinstance(v, str) else float(v) for v in origin]
            z = [float(v) for v in frame["z"]]
            x = [float(v) for v in frame.get("x", [1, 0, 0] if abs(z[2]) > 0.9 else [0, 0, 1])]
        except (KeyError, TypeError, ValueError):
            fail(f"{what}: frame {name}: origin is three numbers (or the z one an expression of the length), "
                 "z and x are three numbers")
        if "kind" not in frame:
            fail(f"{what}: frame {name} has no kind")
        if isinstance(origin[0], str) or isinstance(origin[1], str):
            fail(f"{what}: frame {name}: only the z of an origin may name a parameter")
        if isinstance(origin[2], str) and abs(abs(z[2]) - 1) > 1e-6:
            fail(f"{what}: frame {name}: an origin that follows a parameter has its z along +Z or -Z")
        out.append((name, frame["kind"], origin, z, x, extra))
    return out


def add_frames(mcp, document, frames, length):
    """Adds the frames as joint origins mount:<name>, in their order with
    end_a and end_b first. With a length parameter, end_a and end_b
    without an origin are XY turned over and a plane at the length. A
    frame whose origin's z is an expression (`[0, 0, "L"]`) lies on a plane
    offset by it along Z (from XY, or from a fixed plane through its x and
    y), so that it follows the parameter; the others are fixed planes."""
    def follow(name, label, origin, z, x):
        on_axis = abs(origin[0]) < 1e-9 and abs(origin[1]) < 1e-9 and dot(x, [1, 0, 0]) > 1 - 1e-9
        base = "xy" if on_axis else {"origin": [origin[0], origin[1], 0.0], "normal": [0, 0, 1], "x_axis": x}
        plane = mcp.add(document, {"cmd": "add_feature", "name": label, "def": {
            "type": "construction_plane", "definition": {"type": "offset", "plane": base, "distance": origin[2]}}})
        definition = {"type": "joint_origin", "geometry": plane}
        if z[2] < 0:
            definition["flip"] = True
        return definition

    ordered = sorted(frames, key=lambda f: {"end_a": 0, "end_b": 1}.get(f[0], 2))
    for name, _, origin, z, x, _ in ordered:
        if name == "end_a" and length and origin is None:
            definition = {"type": "joint_origin", "geometry": "xy", "flip": True}
        elif name == "end_b" and length and origin is None:
            definition = follow(name, "End B plane", [0.0, 0.0, length], [0, 0, 1], [1, 0, 0])
        elif origin is None:
            raise PartError(f"frame {name} needs an origin (the part has no length)")
        elif isinstance(origin[2], str):
            label = "End B plane" if name == "end_b" else f"Plane of {name}"
            definition = follow(name, label, origin, z, x)
        else:
            definition = {"type": "joint_origin", "geometry": {"origin": origin, "normal": z, "x_axis": x}}
        mcp.add(document, {"cmd": "add_feature", "name": f"mount:{name}", "def": definition})


def set_frames(entry, build, made, listed):
    """The manifest's frames are the frames made in the design, keeping
    the extra fields (accepts, diameter) of those the group file's
    build.frames lists; a warning names the frames that build.frames lists
    and the design has not, or the reverse."""
    expected = [f.get("name") for f in build.get("frames") or [] if isinstance(f, dict)]
    names = [f[0] for f in made]
    dropped = [n for n in expected if n not in names]
    added = [n for n in names if n not in expected]
    if expected and (dropped or added):
        print(f"warning {entry['id']}: build.frames is the frames made: "
              + "; ".join(filter(None, [f"left out (not in the design) {', '.join(dropped)}" if dropped else "",
                                         f"added {', '.join(added)}" if added else ""])))
    build["frames"] = frame_entries(made, listed)


def frame_entries(frames, listed):
    """The manifest's frames: name, kind and extras, with what the group
    file lists for a frame of the same name (accepts, diameter)."""
    extras = {f["name"]: f for f in listed if isinstance(f, dict) and "name" in f}
    out = []
    for name, kind, _, _, _, extra in frames:
        entry = {"name": name, "kind": kind}
        entry.update(extra)
        for key, value in extras.get(name, {}).items():
            if key in ("accepts", "diameter", "kind"):
                entry[key] = value
        out.append(entry)
    return out


# ---------------------------------------------------------------------
# The three ways of making a part.

def import_source(ctx, entry):
    source = entry.get("source")
    if not source:
        raise PartError("no source file")
    path = os.path.join(ctx.source, source)
    if not os.path.isfile(path):
        raise PartError(f"the source {source} is not in --source")
    imported = f"source-{entry['id']}.mitcad"
    run([ctx.cli, "import", path, "--save", os.path.join(ctx.scratch, imported)])
    return imported


def main_body(mcp, document):
    bodies = [b for b in mcp.query(document, {"query": "bodies", "properties": True})
              if b.get("kind", "solid") == "solid"]
    if not bodies:
        raise PartError("the source has no solid")
    return max(bodies, key=lambda b: b.get("volume", 0))


def make_section(ctx, mcp, entry, build):
    """A profile or another cut-to-length part from its section."""
    source = mcp.open(import_source(ctx, entry))
    try:
        body = main_body(mcp, source)
        low, high = body["bbox"]["min"], body["bbox"]["max"]
        extents = [high[i] - low[i] for i in range(3)]
        axis, part_x, part_y, part_z = section_axes(entry, extents)
        source_length = extents[axis]
        ends, start = end_faces(mcp, source, body, part_z, extents, axis)
        # The section: every end face projected into one sketch on the
        # largest (they lie in one plane).
        sketch_uid = mcp.add(source, {"cmd": "sketch.create",
                                      "plane": {"face": ends[0]["names"][0], "body": body["uid"]}})
        for face in ends:
            mcp.command(source, {"cmd": "sketch.project", "sketch": sketch_uid, "source": face["names"][0],
                                 "body": body["uid"]})
        sketch = mcp.query(source, {"query": "sketch", "uid": sketch_uid})
        # A part sold with holes at a pitch (a rail) keeps them: those
        # nearest end A, found in the source.
        pitched = bool((build.get("length") or {}).get("pitch"))
        holes = source_holes(mcp, source, body, part_z, start) if pitched else []
    finally:
        mcp.close(source)
    face_areas = [f.get("area", 0) for f in ends]
    curves, moved = centred(section_curves(sketch, part_x, part_y), entry.get("place") or {})

    def to_part(point):
        x, y = moved((dot(point, part_x), dot(point, part_y)))
        return [x, y, dot(point, part_z) - start]

    for hole in holes:
        tip = to_part([hole["point"][i] + hole["direction"][i] for i in range(3)])
        hole["point"] = to_part(hole["point"])
        hole["direction"] = [tip[i] - hole["point"][i] for i in range(3)]
    source_area = body["volume"] / source_length
    return build_cut_part(ctx, mcp, entry, build, curves, face_areas, source_length, holes, source_area)


def source_holes(mcp, document, body, part_z, start):
    """The holes across a cut-to-length part nearest its end A: circular
    edges whose axes are square to the part's axis, grouped by the line
    they lie on. Each hole: its start point and direction into the
    material, its diameter, a counterbore's diameter and depth, and
    whether it goes through."""
    edges = mcp.query(document, {"query": "edges", "body": body["uid"]})
    stacks = {}
    for edge in edges:
        if edge.get("curve") != "circle" or not edge.get("name"):
            continue
        measured = mcp.query(document, {"query": "measure", "a": {"body": body["uid"], "edge": edge["name"]}})
        circle = (measured.get("a") or {}).get("circle")
        if not circle or not circle.get("axis") or not circle.get("center"):
            continue
        axis = circle["axis"]
        length = math.sqrt(dot(axis, axis))
        if length < 1e-12:
            continue
        axis = [a / length for a in axis]
        if abs(dot(axis, part_z)) > 1e-6:
            continue
        # One direction per line, so that the circles of a hole meet.
        k = max(range(3), key=lambda i: abs(axis[i]))
        if axis[k] < 0:
            axis = [-a for a in axis]
        center = circle["center"]
        across = cross(axis, part_z)
        key = (round(dot(center, part_z), 2), round(dot(center, across), 2), k)
        stacks.setdefault(key, []).append((dot(center, axis), float(circle["radius"]), center, axis))
    if not stacks:
        return []
    low, high = body["bbox"]["min"], body["bbox"]["max"]
    corners = [[(low, high)[(n >> i) & 1][i] for i in range(3)] for n in range(8)]
    nearest = min(key[0] for key in stacks)
    holes = []
    for key, stack in stacks.items():
        if key[0] - nearest > 0.5:
            continue
        stack.sort(key=lambda c: c[0])
        axis = stack[0][3]
        lo, hi = stack[0][0], stack[-1][0]
        small = min(c[1] for c in stack)
        large = max(c[1] for c in stack)
        counterbore = None
        if large - small > 1e-3 and abs(stack[0][1] - large) < 1e-3 and abs(stack[-1][1] - large) > 1e-3:
            # The counterbore at the lower end: the hole goes along +axis.
            begin, direction = lo, axis
            floor = max(c[0] for c in stack if abs(c[1] - large) < 1e-3)
            counterbore = (2 * large, floor - lo)
        else:
            begin, direction = hi, [-a for a in axis]
            if large - small > 1e-3:
                floor = min(c[0] for c in stack if abs(c[1] - large) < 1e-3)
                counterbore = (2 * large, hi - floor)
        # A counterbore without depth (a chamfer's or countersink's edge at
        # the start) is no counterbore: a plain hole.
        if counterbore and counterbore[1] < 1e-3:
            counterbore = None
        extent_lo = min(dot(c, axis) for c in corners)
        extent_hi = max(dot(c, axis) for c in corners)
        through = abs(lo - extent_lo) < 0.05 and abs(hi - extent_hi) < 0.05
        center = stack[0][2]
        point = [center[i] + (begin - stack[0][0]) * axis[i] for i in range(3)]
        holes.append({"point": point, "direction": direction, "diameter": 2 * small,
                      "counterbore": counterbore, "through": through, "depth": hi - lo,
                      "at": dot(point, part_z) - start})
    return holes


def add_holes(mcp, document, entry, holes, parameter, pitch):
    """The holes of a part with a pitch: each at `first` from end A, on a
    plane at its start facing out of the material, then a pattern along
    the axis at `distance` whose count follows the length, so that a
    shorter part keeps the holes that fit."""
    first = length_of(pitch.get("first", 0), "build.length.pitch.first")
    distance = length_of(pitch.get("distance"), "build.length.pitch.distance")
    if distance <= 0:
        raise PartError("build.length.pitch.distance is not above 0")
    uids = []
    for n, hole in enumerate(holes, 1):
        if abs(hole["at"] - first) > 0.5:
            print(f"warning {entry['id']}: the source's hole nearest end A is {hole['at']:.2f} mm from it, "
                  f"the pitch's first {first:.2f} mm; the design puts it at the pitch's")
        x, y, _ = hole["point"]
        normal = [-c for c in hole["direction"]]
        plane = mcp.add(document, {"cmd": "add_feature", "name": f"Hole plane {n}", "def": {
            "type": "construction_plane", "definition": {
                "type": "fixed", "origin": [x, y, first], "x_axis": [0, 0, 1], "y_axis": cross(normal, [0, 0, 1])}}})
        definition = {"type": "hole", "placement": {"type": "plane", "plane": plane, "points": [[x, y, first]]},
                      "diameter": round(hole["diameter"], 6)}
        if hole["counterbore"] and hole["counterbore"][1] > 1e-3 and hole["counterbore"][0] > hole["diameter"]:
            definition["kind"] = {"type": "counterbore", "diameter": round(hole["counterbore"][0], 6),
                                  "depth": round(hole["counterbore"][1], 6)}
        definition["extent"] = ({"type": "through_all"} if hole["through"] or hole["depth"] < 1e-3
                                else {"type": "distance", "depth": round(hole["depth"], 6)})
        uids.append(mcp.add(document, {"cmd": "add_feature", "name": f"Hole {n}", "def": definition}))
    count = f"max(1, floor(({parameter} - {mm(first)}) / ({mm(distance)})) + 1)"
    mcp.add(document, {"cmd": "add_feature", "name": "Holes at the pitch", "def": {
        "type": "rectangular_pattern", "objects": {"type": "features", "features": uids},
        "direction1": {"axis": "z", "quantity": count, "distance": mm(distance)},
        "distance_type": "spacing"}})


def end_faces(mcp, document, body, part_z, extents, axis):
    """The planar faces at the end the part's z starts from, facing along
    -z, largest first, and where that end lies along z: those no larger than the section's bounds are
    tried, a sketch on each telling where it lies and which way it faces."""
    low, high = body["bbox"]["min"], body["bbox"]["max"]
    corners = [[(low, high)[(n >> i) & 1][i] for i in range(3)] for n in range(8)]
    start = min(dot(c, part_z) for c in corners)
    bounds_area = 1.0
    for i in range(3):
        if i != axis:
            bounds_area *= extents[i]
    faces = mcp.query(document, {"query": "faces", "body": body["uid"]})
    planar = [f for f in faces if f.get("surface") == "plane" and f.get("names")]
    candidates = sorted((f for f in planar if f.get("area", 0) <= bounds_area * 1.0001),
                        key=lambda f: -f.get("area", 0))
    tolerance = max(1e-3, 1e-6 * extents[axis])
    facing, seen = [], []
    for face in candidates:
        uid = mcp.add(document, {"cmd": "sketch.create", "plane": {"face": face["names"][0], "body": body["uid"]}})
        frame = mcp.query(document, {"query": "sketch", "uid": uid}).get("frame") or {}
        normal, origin = frame.get("normal") or [0, 0, 0], frame.get("origin") or [0, 0, 0]
        seen.append(f"{face.get('area', 0):.2f} mm2 at {dot(origin, part_z) - start:.3f}")
        if dot(normal, part_z) < -0.999:
            facing.append((dot(origin, part_z), face))
    # The faces facing out of the start that lie lowest along z: the end,
    # also when the bounds are a little larger than the body.
    first = min((at for at, _ in facing), default=None)
    ends = [] if first is None or first - start > max(0.5, 0.01 * extents[axis]) else [
        face for at, face in facing if at - first < tolerance]
    if not ends:
        raise PartError(f"no planar face at the start of its axis ({'xyz'[axis]}) facing out of it: of "
                        f"{len(planar)} planar faces {len(candidates)} fit the section's bounds "
                        f"({bounds_area:.2f} mm2), their areas and distances from that end: "
                        f"{', '.join(seen[:12]) or 'none'}")
    return ends, first


def build_cut_part(ctx, mcp, entry, build, curves, face_areas, source_length, holes, source_area):
    area = sum(face_areas)
    length_block = build.setdefault("length", {})
    parameter = length_block.setdefault("parameter", "L")
    default = length_of(length_block.get("default", source_length), "build.length.default")
    document = mcp.create()
    try:
        mcp.command(document, {"cmd": "add_parameter", "name": parameter, "expression": mm(default),
                               "comment": "Length"})
        sketch = mcp.add(document, {"cmd": "add_feature", "name": "Section", "def": sketch_definition(curves)})
        regions = mcp.query(document, {"query": "sketch", "uid": sketch}).get("regions") or []
        if not regions:
            raise PartError("the section's curves bound no region")
        # One region per end face: the one of the face's area.
        chosen = []
        for face_area in face_areas:
            left = [r for r in regions if r.get("key") not in chosen]
            region = min(left, key=lambda r: abs(r.get("area", 0) - face_area)) if left else None
            if region is None or abs(region.get("area", 0) - face_area) > 0.02 * face_area:
                raise PartError(f"no region of the section has the end face's area ({face_area:.2f} mm2; "
                                f"regions {[round(r.get('area', 0), 2) for r in regions]})")
            chosen.append(region["key"])
        mcp.add(document, {"cmd": "add_feature", "name": "Profile", "def": {
            "type": "extrude", "profiles": [{"sketch": sketch, "region": key} for key in chosen],
            "extent": {"type": "distance", "distance": parameter}, "operation": "new_body"}})
        bodies = mcp.query(document, {"query": "bodies", "properties": True})
        volume = sum(b.get("volume", 0) for b in bodies)
        if not bodies or abs(volume - area * default) > 0.01 * area * default:
            raise PartError(f"the profile's body is {volume:.1f} mm3, not {area * default:.1f}")
        pitch = length_block.get("pitch")
        if pitch and holes:
            solid = volume
            before = {f["uid"] for f in mcp.query(document, {"query": "timeline"}).get("features", [])}
            try:
                add_holes(mcp, document, entry, holes, parameter, pitch)
                bodies = mcp.query(document, {"query": "bodies", "properties": True})
                volume = sum(b.get("volume", 0) for b in bodies)
                if volume > solid * (1 - 1e-6):
                    raise PartError("the holes cut nothing")
            except PartError as e:
                # Holes that cannot be placed from the source: the part
                # without them, rather than no part.
                added = [f["uid"] for f in mcp.query(document, {"query": "timeline"}).get("features", [])
                         if f["uid"] not in before]
                for uid in reversed(added):
                    try:
                        mcp.command(document, {"cmd": "delete_feature", "uid": uid, "dependents": True})
                    except PartError:
                        pass
                reason = " | ".join(line.strip() for line in str(e).splitlines() if line.strip())
                print(f"warning {entry['id']}: its source's holes could not be placed ({reason}); it is made "
                      "without them")
                bodies = mcp.query(document, {"query": "bodies", "properties": True})
                volume = sum(b.get("volume", 0) for b in bodies)
        elif pitch:
            print(f"warning {entry['id']}: build.length.pitch gives holes, but the source has no holes across "
                  "the part; it is made without them")
        # The design against the source, per length: what differs (holes
        # the design does not have, machined ends) is said.
        per_length = volume / default
        if abs(per_length - source_area) > 0.05 * source_area:
            print(f"warning {entry['id']}: the design's volume per length is {per_length:.2f} mm2, the "
                  f"source's {source_area:.2f} mm2")
        frames = entry.get("frames") or {"auto": "profile"}
        openings = []
        if isinstance(frames, dict):
            if frames.get("auto") != "profile":
                fail(f"{entry['id']}: frames.auto is 'profile' or the frames are a list")
            found, openings = profile_frames(curves)
            found = [("end_a", "end", None, None, None, {}), ("end_b", "end", None, None, None, {})] + found
            found += listed_frames(frames.get("add", []), entry["id"])
            listed = (build.get("frames") or []) + frames.get("add", [])
        else:
            found = listed_frames(frames, entry["id"])
            listed = (build.get("frames") or []) + frames
        add_frames(mcp, document, found, parameter)
        set_frames(entry, build, found, listed)
        if build.get("role") == "profile":
            x0, y0, x1, y1 = bounds(curves)
            build.setdefault("section", {"width": mm(x1 - x0), "height": mm(y1 - y0)})
            if openings and "opening" not in build.get("slot", {}):
                build.setdefault("slot", {})["opening"] = mm(min(openings))
            build.setdefault("mass_per_metre", f"{per_length * 1000 * ALUMINIUM:.3f} kg")
        finish_design(mcp, document, entry)
        return save(ctx, mcp, document, entry), {"kind": "section", "curves": curves}
    finally:
        mcp.close(document)


def placement_matrix(place, what):
    """A move's matrix from `place`: `matrix`; or `z` and `x` (the source's
    directions that become the part's z and x axes); or `rotate`
    ({"axis": "x" or [x, y, z], "angle": degrees}); then `translate` [x, y,
    z]."""
    if "matrix" in place:
        return [[float(v) for v in row] for row in place["matrix"]]
    rotation = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    if "z" in place:
        z = unit(place["z"], f"{what}: place.z")
        x = unit(place.get("x", [1, 0, 0] if abs(z[0]) < 0.9 else [0, 0, 1]), f"{what}: place.x")
        if abs(dot(x, z)) > 1e-6:
            fail(f"{what}: place.x is not square to place.z")
        # The part's coordinates of a source point are its components along
        # the part's axes: the rows of the rotation.
        rotation = [x, cross(z, x), z]
    elif "rotate" in place:
        axis = place["rotate"].get("axis", "z")
        axis = {"x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]}.get(axis, axis)
        norm = math.sqrt(dot(axis, axis))
        x, y, z = [float(v) / norm for v in axis]
        a = math.radians(float(place["rotate"].get("angle", 0)))
        c, s, t = math.cos(a), math.sin(a), 1 - math.cos(a)
        rotation = [[t * x * x + c, t * x * y - s * z, t * x * z + s * y],
                    [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
                    [t * x * z - s * y, t * y * z + s * x, t * z * z + c]]
    shift = [float(v) for v in place.get("translate", [0, 0, 0])]
    return [rotation[i] + [shift[i]] for i in range(3)]


def make_solid(ctx, mcp, entry, build):
    """A connector, nut, cap or other part from the source's solids."""
    document = mcp.open(import_source(ctx, entry))
    try:
        bodies = mcp.query(document, {"query": "bodies", "properties": True})
        if not bodies:
            raise PartError("the source has no bodies")
        place = entry.get("place") or {}
        if place:
            mcp.add(document, {"cmd": "add_feature", "name": "Placed", "def": {
                "type": "move", "bodies": [b["uid"] for b in bodies],
                "transform": {"type": "free", "matrix": placement_matrix(place, entry["id"])}}})
        frames = listed_frames(entry.get("frames") or [], entry["id"])
        length = (build.get("length") or {}).get("parameter")
        add_frames(mcp, document, frames, length)
        set_frames(entry, build, frames, build.get("frames") or [])
        finish_design(mcp, document, entry)
        return save(ctx, mcp, document, entry), {"kind": "solid"}
    finally:
        mcp.close(document)


def make_model(ctx, mcp, entry, build):
    """A sheet, plate, angle or rod from its `dimensions`: `shape`
    (`plate`, `angle`, `rod` or `none`), its sizes (a parameter of
    `parameters`, or a value, which becomes a user parameter) and
    `parameters` (user parameters and their defaults); then any
    `commands`. A plate and an angle are centred on x = 0 across their
    `width`: a plate runs `thickness` along +Y and `length` along +Z; an
    angle has its first leg along +Z (in the XZ plane, `thickness` along
    +Y) and its second along +Y (in the XY plane, `thickness` along +Z). A
    rod is a cylinder of `diameter` on the Z axis from z = 0 to `length`;
    with `thread` (a thread size: `standard`, `designation`, `class`) a
    cosmetic thread over its whole side (mitcad#449)."""
    dimensions = entry.get("dimensions") or {}
    if not isinstance(dimensions, dict):
        fail(f"{entry['id']}: dimensions is no object")
    shape = dimensions.get("shape", "none")
    needed = {"plate": ("width", "thickness", "length"), "angle": ("legs", "width", "thickness"),
              "rod": ("diameter", "length"), "none": ()}.get(shape)
    if needed is None:
        fail(f"{entry['id']}: dimensions.shape '{shape}' is none of plate, angle, rod, none")
    missing = [n for n in needed if n not in dimensions]
    if missing:
        fail(f"{entry['id']}: the {shape} needs dimensions.{', dimensions.'.join(missing)}")
    parameters = dimensions.get("parameters") or {}
    if not isinstance(parameters, dict):
        fail(f"{entry['id']}: dimensions.parameters is no object")
    thread = dimensions.get("thread")
    if thread is not None and (shape != "rod" or not isinstance(thread, dict) or not thread.get("designation")):
        fail(f"{entry['id']}: dimensions.thread is a rod's thread size with a designation")
    document = mcp.create()
    try:
        names = set()
        for name, value in parameters.items():
            mcp.command(document, {"cmd": "add_parameter", "name": name,
                                   "expression": mm(length_of(value, f"dimensions.parameters.{name}"))})
            names.add(name)

        def size(value, default_name):
            """The parameter a size names, or a new user parameter of its value."""
            if isinstance(value, str) and value.strip() in names:
                return value.strip()
            millimetres = length_of(value, f"{entry['id']}: dimensions")
            name, n = default_name, 1
            while name in names:
                n += 1
                name = f"{default_name}{n}"
            mcp.command(document, {"cmd": "add_parameter", "name": name, "expression": mm(millimetres)})
            names.add(name)
            return name

        def box(name, length, width, height, operation):
            return mcp.add(document, {"cmd": "add_feature", "name": name, "def": {
                "type": "box", "plane": "xy", "corner": [0, 0], "length": length, "width": width,
                "height": height, "operation": operation}})

        if shape == "plate":
            w = size(dimensions["width"], "W")
            t = size(dimensions["thickness"], "T")
            length = size(dimensions["length"], "L")
            box("Plate", w, t, length, "new_body")
        elif shape == "angle":
            legs = dimensions["legs"]
            if not isinstance(legs, list) or len(legs) != 2:
                fail(f"{entry['id']}: dimensions.legs is a list of two lengths")
            a, b = size(legs[0], "A"), size(legs[1], "B")
            w = size(dimensions["width"], "W")
            t = size(dimensions["thickness"], "T")
            box("Leg A", w, t, a, "new_body")
            box("Leg B", w, b, t, "join")
        elif shape == "rod":
            d = size(dimensions["diameter"], "D")
            length = size(dimensions["length"], "L")
            rod = mcp.add(document, {"cmd": "add_feature", "name": "Rod", "def": {
                "type": "cylinder", "plane": "xy", "center": [0, 0], "diameter": d, "height": length,
                "operation": "new_body"}})
            if thread is not None:
                # Cosmetic, over the whole side: the thread follows the
                # length, and the threads query lists it.
                body = mcp.query(document, {"query": "bodies"})[0]["uid"]
                size_of = {key: thread[key] for key in ("standard", "designation", "class", "right_handed")
                           if key in thread}
                mcp.add(document, {"cmd": "add_feature", "name": "Thread", "def": {
                    "type": "thread", "faces": [{"body": body, "face": f"{rod}:side0"}], "thread": size_of,
                    "modeled": False}})
                threads = mcp.query(document, {"query": "threads"}) or []
                if len(threads) != 1:
                    raise PartError(f"the rod's thread {thread['designation']} is not listed "
                                    f"({len(threads)} threads)")
        if shape in ("plate", "angle"):
            bodies = mcp.query(document, {"query": "bodies"})
            mcp.add(document, {"cmd": "add_feature", "name": "Centred", "def": {
                "type": "move", "bodies": [body["uid"] for body in bodies],
                "transform": {"type": "translate_xyz", "x": f"-{w} / 2", "y": 0, "z": 0}}})
        for command in entry.get("commands") or []:
            mcp.command(document, command)
        if not mcp.query(document, {"query": "bodies"}):
            raise PartError("the model makes no body")
        frames = listed_frames(entry.get("frames") or [], entry["id"])
        length = (build.get("length") or {}).get("parameter")
        add_frames(mcp, document, frames, length)
        set_frames(entry, build, frames, build.get("frames") or [])
        finish_design(mcp, document, entry)
        return save(ctx, mcp, document, entry), {"kind": "model"}
    finally:
        mcp.close(document)


def finish_design(mcp, document, entry):
    """The configuration table of the rows, and a recompute's check."""
    rows = entry.get("rows") or []
    if rows:
        selectors, parameters = [], []
        table_rows = []
        for i, row in enumerate(rows):
            if not isinstance(row, dict) or not row.get("name"):
                fail(f"{entry['id']}: rows[{i}] has no name")
            for key in (row.get("select") or {}):
                if key not in selectors:
                    selectors.append(key)
            for key in (row.get("values") or {}):
                if key not in parameters:
                    parameters.append(key)
            table_row = {"name": row["name"], "values": row.get("values") or {}}
            for key in ("select", "designation", "part_number", "supplier_url"):
                if row.get(key):
                    table_row[key] = row[key]
            table_rows.append(table_row)
        table = {"selectors": selectors, "parameters": parameters,
                 "default": entry.get("default", rows[0]["name"]), "rows": table_rows}
        mcp.command(document, {"cmd": "set_configurations", "configurations": table})
    timeline = mcp.query(document, {"query": "timeline"})
    failed = [f"{f.get('name')}: {f.get('error')}" for f in timeline.get("features", []) if f.get("status") == "error"]
    if failed:
        raise PartError("; ".join(failed))


# Top-level fields of a design that put its parts in components: a
# library's design holds its features in the root.
ASSEMBLY_FIELDS = ("components", "occurrences", "active_component", "root_policy")


def save(ctx, mcp, document, entry):
    name = f"part-{entry['id']}.mitcad"
    mcp.save(document, name)
    path = os.path.join(ctx.scratch, name)
    try:
        with open(path, encoding="utf-8") as f:
            design = json.load(f)
    except (OSError, ValueError) as e:
        raise PartError(f"the saved design cannot be read: {e}")
    found = [key for key in ASSEMBLY_FIELDS if design.get(key) not in (None, [], {}, "C0")]
    if found:
        raise PartError(f"the design has {', '.join(found)}: its features are not all in the root")
    # Names a source file brought along in another language or a broken
    # encoding (German or mis-decoded body names) become the part's own: names
    # are for people, references go by uid.
    renamed = {}

    def plain(value):
        if isinstance(value, dict):
            for key, item in value.items():
                if key == "name" and isinstance(item, str) and not item.isascii():
                    value[key] = renamed.setdefault(item, f"{entry['name']} {len(renamed) + 1}")
                else:
                    plain(item)
        elif isinstance(value, list):
            for item in value:
                plain(item)

    plain(design)
    # Other text from the source (its file name in a base feature) is kept,
    # written as JSON escapes so that the design file is ASCII.
    with open(path, encoding="utf-8") as f:
        is_ascii = f.read().isascii()
    if renamed or not is_ascii:
        with open(path, "w", encoding="utf-8") as f:
            json.dump(design, f, indent=2, ensure_ascii=True)
            f.write("\n")
    return path


# ---------------------------------------------------------------------
# Previews: 256 x 256 PNG images.

SIZE = 256


def png(width, height, pixels):
    raw = b"".join(b"\x00" + bytes(row) for row in pixels)

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9))
            + chunk(b"IEND", b""))


def raster(polygons):
    """Fills polygons (lists of points in a -1..1 square, y up) with their
    shades by the even-odd rule within each polygon group; later groups
    are drawn over earlier ones. Returns the image's rows."""
    background = (246, 247, 249)
    index = [[-1] * SIZE for _ in range(SIZE)]
    shades = [[None] * SIZE for _ in range(SIZE)]
    for number, (segments, shade) in enumerate(polygons):
        for row in range(SIZE):
            y = 1 - 2 * (row + 0.5) / SIZE
            crossings = []
            for (x1, y1), (x2, y2) in segments:
                if (y1 > y) != (y2 > y):
                    crossings.append(x1 + (y - y1) * (x2 - x1) / (y2 - y1))
            crossings.sort()
            for a, b in zip(crossings[0::2], crossings[1::2]):
                first = max(0, int(math.ceil((a + 1) * SIZE / 2 - 0.5)))
                last = min(SIZE - 1, int(math.floor((b + 1) * SIZE / 2 - 0.5)))
                for column in range(first, last + 1):
                    index[row][column] = number
                    shades[row][column] = shade
    pixels = []
    for row in range(SIZE):
        line = []
        for column in range(SIZE):
            shade = shades[row][column]
            if shade is None:
                line.extend(background)
            else:
                # Anodised aluminium, lit from the upper left.
                light = 0.75 + 0.25 * (1 - (row + column) / (2 * SIZE))
                level = max(0.0, min(1.0, shade * light))
                line.extend((int(60 + 170 * level), int(66 + 172 * level), int(76 + 174 * level)))
        pixels.append(line)
    for row in range(1, SIZE - 1):
        for column in range(1, SIZE - 1):
            here = index[row][column]
            if here < 0:
                continue
            around = (index[row - 1][column], index[row + 1][column], index[row][column - 1], index[row][column + 1])
            if any(other != here for other in around):
                pixels[row][3 * column:3 * column + 3] = [38, 42, 50]
    return pixels


def fit(points_lists):
    xs = [x for points in points_lists for x, _ in points]
    ys = [y for points in points_lists for _, y in points]
    span = max(max(xs) - min(xs), max(ys) - min(ys)) or 1.0
    cx, cy = (max(xs) + min(xs)) / 2, (max(ys) + min(ys)) / 2
    scale = 1.7 / span
    return lambda p: ((p[0] - cx) * scale, (p[1] - cy) * scale)


def section_preview(curves):
    """The section seen from end B."""
    chains = [polyline(c, 96) for c in curves]
    to = fit(chains)
    segments = []
    for chain in chains:
        points = [to(p) for p in chain]
        segments.extend(zip(points, points[1:]))
    return png(SIZE, SIZE, raster([(segments, 0.85)]))


def box_preview(low, high):
    """The bounds of the bodies as a box seen from the front right, above."""
    def iso(p):
        x, y, z = p
        return ((x - y) * math.cos(math.radians(30)), (x + y) * math.sin(math.radians(30)) + z)

    x0, y0, z0 = low
    x1, y1, z1 = high
    top = [(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]
    front = [(x0, y0, z0), (x1, y0, z0), (x1, y0, z1), (x0, y0, z1)]
    side = [(x1, y0, z0), (x1, y1, z0), (x1, y1, z1), (x1, y0, z1)]
    faces = [[iso(p) for p in face] for face in (front, side, top)]
    to = fit(faces)
    polygons = []
    for face, shade in zip(faces, (0.65, 0.5, 0.9)):
        points = [to(p) for p in face]
        polygons.append((list(zip(points, points[1:] + points[:1])), shade))
    return png(SIZE, SIZE, raster(polygons))


def preview(ctx, design, made, mcp_bounds):
    if made["kind"] == "section":
        return section_preview(made["curves"])
    if ctx.render:
        target = design + ".png"
        result = run([ctx.cli, "render", design, "-o", target, "--view", "iso", "--size", f"{SIZE}x{SIZE}",
                      "--samples", "16", "--time-limit", "30"], check=False)
        if result.returncode == 0 and os.path.isfile(target) and os.path.getsize(target) <= 512 << 10:
            with open(target, "rb") as f:
                return f.read()
    return box_preview(*mcp_bounds)


def design_bounds(mcp, ctx, design):
    document = mcp.open(os.path.relpath(design, ctx.scratch).replace("\\", "/"))
    try:
        bodies = mcp.query(document, {"query": "bodies", "properties": True})
    finally:
        mcp.close(document)
    lows = [b["bbox"]["min"] for b in bodies if "bbox" in b]
    highs = [b["bbox"]["max"] for b in bodies if "bbox" in b]
    if not lows:
        return [0, 0, 0], [1, 1, 1]
    return [min(p[i] for p in lows) for i in range(3)], [max(p[i] for p in highs) for i in range(3)]


# ---------------------------------------------------------------------
# Group files and the library folder.

def read_groups(folder, only):
    files = sorted(glob.glob(os.path.join(folder, "*.json")))
    if not files:
        fail(f"no group files (*.json) in {folder}")
    entries, categories, seen, left_out = [], {}, set(), []
    for path in files:
        name = os.path.basename(path)
        try:
            with open(path, encoding="utf-8") as f:
                data = json.load(f)
        except (OSError, ValueError) as e:
            fail(f"{name}: {e}")
        if isinstance(data, dict):
            for category in data.get("categories") or []:
                if not isinstance(category, dict) or "id" not in category:
                    fail(f"{name}: a category without an id")
                categories[category["id"]] = category.get("name") or category["id"]
            parts = data.get("parts", data.get("components"))
            # The supplier's files of the group that are no part, and why
            # (a duplicate, an assembly, a wrong file in a product's folder).
            notes = data.get("left_out") or []
            if not isinstance(notes, list) or not all(
                    isinstance(n, dict) and isinstance(n.get("source"), str) and isinstance(n.get("reason"), str)
                    for n in notes):
                fail(f"{name}: left_out is a list of {{\"source\", \"reason\"}}")
            left_out += [dict(n, _group=name) for n in notes]
        else:
            parts = data
        if not isinstance(parts, list):
            fail(f"{name}: a group file is a list of parts, or an object with `parts`")
        for i, entry in enumerate(parts):
            if not isinstance(entry, dict):
                fail(f"{name}: part {i} is no object")
            for key in ("id", "name", "make"):
                if not isinstance(entry.get(key), str) or not entry[key].strip():
                    fail(f"{name}: part {i} has no {key}")
            if entry["make"] not in MAKES:
                fail(f"{name}: {entry['id']}: make '{entry['make']}' is none of {', '.join(MAKES)}")
            if entry["id"] in seen:
                fail(f"{name}: the id {entry['id']} is used twice")
            if entry["make"] != "model" and not entry.get("source"):
                fail(f"{name}: {entry['id']}: a {entry['make']} part names its source")
            if "build" in entry and not isinstance(entry["build"], dict):
                fail(f"{name}: {entry['id']}: build is no object")
            for row in entry.get("rows") or []:
                if isinstance(row, dict) and "source" in row and not isinstance(row["source"], str):
                    fail(f"{name}: {entry['id']}: the source of row {row.get('name')} is no path")
            seen.add(entry["id"])
            entry["_group"] = name
            if not only or entry["id"] in only:
                entries.append(entry)
    return entries, categories, left_out


def category_name(category, categories):
    return categories.get(category) or category.replace("-", " ").replace("_", " ").capitalize()


MIT_TEXT = """MIT License

Copyright (c) 2026 Mitcad contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""


# Letters of the suppliers' file names written in ASCII in SOURCES.md.
ASCII_LETTERS = {"\u00e4": "ae", "\u00f6": "oe", "\u00fc": "ue", "\u00c4": "Ae", "\u00d6": "Oe", "\u00dc": "Ue",
                 "\u00df": "ss", "\u00b0": "deg"}


def ascii_text(text):
    return "".join(c if c.isascii() else ASCII_LETTERS.get(c, "?") for c in text)


def sources_text(components, entries, left_out=()):
    by_id = {e["id"]: e for e in entries}
    lines = ["# Sources of the parts", "",
             "The parts are Mitcad's own parametric designs. A profile's design is the",
             "cross-section of the supplier's CAD file extruded by its length; a connector,",
             "nut or other part keeps the solids of the supplier's CAD file; sheets and",
             "plates sold without a CAD file are modelled from the dimensions the supplier",
             "gives. Each part names its supplier and part number, so that it can be",
             "bought. Check critical dimensions against the supplier's data.", ""]
    for component in components:
        entry = by_id[component["id"]]
        where = entry.get("source") or "the supplier's dimensions"
        if entry["make"] == "model" and entry.get("source"):
            where = f"modelled from the dimensions of {entry['source']}"
        url = entry.get("supplier_url") or ""
        supplier = " ".join(filter(None, [entry.get("supplier", ""), entry.get("part_number", "")]))
        line = f"- {component['id']} ({component['name']}): {where}"
        if supplier:
            line += f"; {supplier}"
        if url:
            line += f"; {url}"
        lines.append(line)
        # The supplier's files of the rows (another thread, colour or stock
        # length of the same part): the design is made from the part's.
        for row in entry.get("rows") or []:
            if isinstance(row, dict) and row.get("source"):
                row_line = f"  - {row['name']}: {row['source']}"
                if row.get("part_number"):
                    row_line += f"; {row['part_number']}"
                if row.get("supplier_url"):
                    row_line += f"; {row['supplier_url']}"
                lines.append(row_line)
    if left_out:
        lines += ["", "The supplier's files not made into parts, and why:", ""]
        lines += [f"- {n['source']}: {n['reason']}" for n in left_out]
    lines += ["", "The generator: tools/libraries/make-profile-library.py in Mitcad's repository.", ""]
    return ascii_text("\n".join(lines))


class Context:
    pass


def make_part(ctx, mcp, entry):
    build = copy.deepcopy(entry.get("build") or {})
    if entry["make"] == "section":
        design, made = make_section(ctx, mcp, entry, build)
    elif entry["make"] == "solid":
        design, made = make_solid(ctx, mcp, entry, build)
    else:
        design, made = make_model(ctx, mcp, entry, build)
    category = entry.get("category") or build.get("role") or "parts"
    path = f"{category}/{entry['id']}.mitcad"
    target = os.path.join(ctx.out, path)
    os.makedirs(os.path.dirname(target), exist_ok=True)
    shutil.copyfile(design, target)
    image = preview(ctx, design, made, design_bounds(mcp, ctx, design) if made["kind"] != "section" else None)
    preview_path = f"previews/{entry['id']}.png"
    os.makedirs(os.path.join(ctx.out, "previews"), exist_ok=True)
    with open(os.path.join(ctx.out, preview_path), "wb") as f:
        f.write(image)
    component = {"id": entry["id"], "path": path, "category": category, "name": entry["name"]}
    for key in ("standard", "description", "keywords", "license", "designation", "supplier", "part_number",
                "supplier_url"):
        if entry.get(key):
            component[key] = entry[key]
    component["preview"] = preview_path
    # A plain part (no build block in its group file) stays one: the frames
    # the making found are not a BUILD part's.
    if build and entry.get("build"):
        component["build"] = build
    rows = entry.get("rows") or []
    if rows and "description" not in component:
        component["description"] = f"{len(rows)} sizes, {rows[0]['name']} to {rows[-1]['name']}."
    return component


def merged(out, components, categories, sources):
    """This run's components, categories and sources, then those of the
    manifest in `out` that this run does not make (as
    make-fastener-library.py's --merge)."""
    path = os.path.join(out, "mitcad-library.json")
    try:
        with open(path, encoding="utf-8") as f:
            old = json.load(f)
    except (OSError, ValueError):
        return components, categories, sources
    if not isinstance(old, dict):
        return components, categories, sources
    made = {c["id"] for c in components}
    kept = [c for c in old.get("components") or [] if isinstance(c, dict) and c.get("id") not in made]
    used = {c.get("category") for c in kept}
    names = {c["id"] for c in categories}
    more = [c for c in old.get("categories") or []
            if isinstance(c, dict) and c.get("id") in used and c.get("id") not in names]
    extra = [s for s in old.get("sources") or [] if s not in sources]
    return components + kept, categories + more, sources + extra


def inside_project(folder):
    folder = os.path.abspath(folder)
    while True:
        if os.path.isfile(os.path.join(folder, ".mitcad", "project.json")):
            return True
        parent = os.path.dirname(folder)
        if parent == folder:
            return False
        folder = parent


def main():
    # The parts' source paths are the supplier's UTF-8 file names; a console
    # in another code page must not stop the build.
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--cli", required=True, help="mitcad-cli")
    parser.add_argument("--source", required=True, help="the folder of the supplier's files (never copied)")
    parser.add_argument("--out", required=True, help="the library's folder (made when missing)")
    parser.add_argument("--groups", default=os.path.join(HERE, "profiles"), help="the folder of the group files")
    parser.add_argument("--only", default="", help="only these part ids, e.g. p30x30-b8,tnut-b8-m6")
    parser.add_argument("--version", default="1.0.0", help="the library's version")
    parser.add_argument("--commit", action="store_true", help="record the folder as a git version and tag it")
    parser.add_argument("--work", help="folder for intermediate files (default: next to --out)")
    parser.add_argument("--no-render", action="store_true",
                        help="draw the previews of solids from their bounds instead of rendering them")
    parser.add_argument("--merge", action="store_true",
                        help="keep the library's other parts of the manifest in --out (the fastener generator's)")
    args = parser.parse_args()

    only = {s.strip() for s in args.only.split(",") if s.strip()}
    entries, categories, left_out_files = read_groups(args.groups, only)
    if not entries:
        fail("no parts to make")
    ctx = Context()
    ctx.cli = os.path.abspath(args.cli) if os.path.exists(args.cli) else args.cli
    ctx.source = os.path.abspath(args.source)
    ctx.out = os.path.abspath(args.out)
    ctx.render = not args.no_render
    # The files the group files name and do not make: a note on one that
    # is not in --source (moved or renamed by the supplier).
    for entry in entries:
        for row in entry.get("rows") or []:
            source = row.get("source") if isinstance(row, dict) else None
            if source and not os.path.isfile(os.path.join(ctx.source, source)):
                print(f"warning {entry['id']}: the source of row {row.get('name')}, {source}, is not in --source")
    for note in left_out_files:
        if not os.path.isfile(os.path.join(ctx.source, note["source"])):
            print(f"warning {note['_group']}: the left-out file {note['source']} is not in --source")
    os.makedirs(ctx.out, exist_ok=True)
    if not os.path.isfile(os.path.join(ctx.out, ".mitcad", "project.json")):
        run([ctx.cli, "project", "init", ctx.out, "--no-history"])
    work = os.path.abspath(args.work or os.path.dirname(ctx.out))
    os.makedirs(work, exist_ok=True)
    components, left_out = [], []
    with tempfile.TemporaryDirectory(prefix=".profile-work-", dir=work) as scratch:
        ctx.scratch = scratch
        if inside_project(scratch):
            fail(f"--work {work} is inside a Mitcad project; give a folder outside it")
        # The blank design each made part starts from (Mcp.create).
        write(os.path.join(scratch, "blank.json"), "[]\n")
        try:
            run([ctx.cli, "run", os.path.join(scratch, "blank.json"), "--save", os.path.join(scratch, Mcp.BLANK)])
        except PartError as e:
            fail(str(e))
        mcp = Mcp(ctx.cli, scratch)
        try:
            for entry in entries:
                try:
                    components.append(make_part(ctx, mcp, entry))
                    print(f"made {entry['id']}")
                except PartError as e:
                    # One line per part, whatever the reason holds.
                    reason = " | ".join(line.strip() for line in str(e).splitlines() if line.strip())
                    left_out.append((entry["id"], reason))
                    print(f"left out {entry['id']}: {reason}", flush=True)
        finally:
            mcp.finish()
    if not components:
        fail("no part could be made")
    made = components
    used = []
    for component in components:
        if component["category"] not in used:
            used.append(component["category"])
    library_categories = [{"id": c, "name": category_name(c, categories)} for c in used]
    sources = ["The suppliers' CAD files and dimensions, made parametric; see SOURCES.md"]
    if args.merge:
        # The library's other parts (the fasteners) stay.
        components, library_categories, sources = merged(ctx.out, components, library_categories, sources)
    manifest = {
        "format": "mitcad-library", "version": 1, "id": LIBRARY_ID, "name": LIBRARY_NAME,
        "description": LIBRARY_DESCRIPTION,
        "library_version": args.version, "license": LICENSE, "authors": ["Mitcad contributors"],
        "sources": sources, "units": "mm",
        "categories": library_categories,
        "components": components,
    }
    write(os.path.join(ctx.out, "mitcad-library.json"), json.dumps(manifest, indent=2) + "\n")
    write(os.path.join(ctx.out, "LICENSE"), MIT_TEXT)
    write(os.path.join(ctx.out, "SOURCES.md"), sources_text(made, entries, [] if only else left_out_files))
    write(os.path.join(ctx.out, "README.md"),
          f"# {LIBRARY_NAME}\n\n"
          "Slot profiles, connectors, nuts, caps, panels and linear parts for Mitcad's BUILD tab.\n"
          "Each part is a parametric design (a profile is cut to any length) and names its supplier\n"
          "and part number. In Mitcad: Libraries, add this repository's URL, Fetch; then the BUILD\n"
          "tab's pickers list the parts. Designs record the version they use.\n\n"
          f"Licence: {LICENSE} (LICENSE). Where the parts come from: SOURCES.md.\n")
    check = run([ctx.cli, "library", "check", ctx.out], check=False)
    sys.stdout.write(check.stdout + check.stderr)
    if check.returncode != 0:
        fail("the library check found errors; nothing was recorded")
    if args.commit:
        identity = ["-c", "user.name=Mitcad developers", "-c", "user.email=devs@mitcad.org"]
        if not os.path.isdir(os.path.join(ctx.out, ".git")):
            run(["git", "init", "-q", "-b", "main", ctx.out])
        run(["git", "-C", ctx.out, "add", "-A"])
        status = run(["git", "-C", ctx.out, "status", "--porcelain"]).stdout
        if status.strip():
            run(["git", "-C", ctx.out] + identity + ["commit", "-q", "-m", f"{LIBRARY_NAME} {args.version}"])
        run(["git", "-C", ctx.out] + identity + ["tag", "-a", "-f", f"v{args.version}", "-m",
                                                  f"{LIBRARY_NAME} {args.version}"])
    print(f"{LIBRARY_ID} {args.version}: {len(made)} components in {ctx.out}")
    if left_out:
        print(f"left out: {len(left_out)} ({', '.join(i for i, _ in left_out)})")


if __name__ == "__main__":
    try:
        main()
    except PartError as e:
        fail(str(e))
