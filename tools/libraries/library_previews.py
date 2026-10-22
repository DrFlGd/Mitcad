# SPDX-License-Identifier: MIT
"""Rendered preview images of the library's parts (mitcad#526), shared by
make-fastener-library.py and make-profile-library.py.

Every fastener's picture is rendered with Mitcad's own renderer
(`mitcad-cli render`, docs/rendering.md) in one studio setup: the part
turned so that its axis (the head's side up) is Z, a three-quarter view
from the front right and above, the same environment, no ground, the
render's transparent pixels laid over the picker tiles' background, and
the part cropped to fill the tile. The setup is STUDIO below: a rebuilt
library gives the same pictures, and a picture whose part and setup did
not change is kept (its PNG carries the key it was made from).

Finishes: a fastener's surface is its `appearance`, one of FINISHES by
name (zinc plated steel, stainless, black oxide, brass) or any appearance
of Mitcad's library; default_finish() picks one from a part's name.

Only the standard library: the images are read and written here
(PNG, 8 bits; no Pillow), so that the generators run wherever Python 3
does, Windows included.
"""

import hashlib
import json
import math
import os
import shutil
import struct
import subprocess
import tempfile
import zlib

# The picker tiles' background behind a part, sRGB 0..255: that of the
# pictures the generators draw (sections, outlines), so that drawn and
# rendered tiles sit side by side.
BACKGROUND = (246, 247, 249)
# The picture's size in the library, and the render's (twice as large,
# averaged down: smooth edges without more samples).
PREVIEW_SIZE = 256
RENDER_SIZE = 512
# The library's limit on a preview (docs/libraries.md).
MAX_BYTES = 512 << 10

# The one studio setup of every fastener's picture. Changing anything
# here renders every picture again (the key below includes it).
STUDIO = {
    "version": 3,
    # Cycles' path tracing: samples per pixel, denoised once at the end;
    # no time limit, so that a slow machine renders the same picture. The
    # render worker leaves Cycles' integrator seed at its default (0), so
    # the samples are the same on every run; `seed` records it.
    "samples": 64,
    "denoise": True,
    "seed": 0,
    # The environment of the render settings (commands.md "Render
    # settings"): the default studio's soft key, fill and top lights in
    # light grey surroundings, which the metals reflect, twice as strong
    # as the default and a quarter stop brighter, so that zinc plating is
    # a light grey; no ground (the parts float on the tile), the neutral
    # view transform (base colours as they are, highlights compressed
    # rather than clipped).
    "environment": {"preset": "studio", "strength": 2.0, "rotation": 0.0},
    "ground": {"shadows": False},
    "film": {"exposure": 0.25, "view_transform": "neutral"},
    # Softboxes (render lights, commands.md "Render lights") whose
    # reflections are the metal's highlights. Each is placed where the
    # camera sees it mirrored in the part: `normal` is the part's surface
    # normal (in its frame, its axis Z) that shows the light, given
    # relative to the camera: `azimuth` degrees round Z from the camera's
    # side (negative: to the left in the picture) and `elevation` degrees
    # over the XY plane. The light lies along the view direction mirrored
    # in that normal, `distance` times the part's size (its bounds'
    # diagonal) from its middle, facing it, `size` (across, along Z)
    # times the part's size, giving `irradiance` W/m2 at the part (its
    # power scaled with the distance, so that every part is lit alike). A
    # vertical normal mirrors a light above; a horizontal one (a shank or
    # a head's side, seen from above) a light below the part. So: a broad
    # key streak along the left of a shank and a head's side, a narrow rim
    # streak near their right edge, and a small softbox in a head's top,
    # tilted so that the top shows it in part (a highlight fading out, not
    # a flat disc).
    "lights": [
        {"name": "Key", "normal": {"azimuth": -35.0, "elevation": 0.0}, "distance": 4.0, "size": [1.6, 4.0],
         "irradiance": 2.5},
        {"name": "Rim", "normal": {"azimuth": 60.0, "elevation": 0.0}, "distance": 4.0, "size": [0.6, 4.0],
         "irradiance": 2.0},
        {"name": "Top", "normal": {"azimuth": -30.0, "elevation": 78.0}, "distance": 4.0, "size": [1.0, 1.0],
         "irradiance": 1.5},
    ],
    # The camera, in the part's frame after its axis is turned to +Z (the
    # head's side up): from the front right (azimuth from -Y towards +X)
    # and above (elevation over the XY plane), `distance` times the part's
    # size away, in perspective (so that a flat face shows a gradient of
    # what it mirrors; the view shows `frame` times the part's projected
    # bounds, as its nearer side looks larger). A long part (longer along
    # its axis than `long` times its width) is seen from lower down, a flat
    # one (thinner than `flat` times its width: nuts, washers) from higher
    # up; a part at least `diagonal` times longer than wide lies across the
    # tile (the camera rolled by `roll`), so that it fills more of it.
    # `margin` is left around the part on each side once it is cropped.
    "camera": {"azimuth": 35.0, "elevation": {"long": 25.0, "compact": 35.0, "flat": 50.0},
               "long": 1.5, "flat": 0.6, "diagonal": 2.2, "roll": 45.0, "margin": 0.06,
               "perspective": True, "distance": 3.0, "frame": 1.35},
    "render_size": RENDER_SIZE,
    "preview_size": PREVIEW_SIZE,
    "background": list(BACKGROUND),
}

# Fasteners' surfaces by name: an appearance of Mitcad's library (its id)
# or one the design gets ({"id", "name", "based_on", parameters}). All
# metals (metalness 1, from steel_satin) and glossy, so that their
# highlights show their shape.
FINISHES = {
    # Zinc plated steel: a light, slightly bluish silver.
    "zinc_plated": {"id": "zinc_plated", "name": "Steel - Zinc plated", "based_on": "steel_satin",
                    "base_color": [0.74, 0.76, 0.79], "roughness": 0.27},
    # Stainless steel (A2, A4): a little darker and smoother than zinc.
    "stainless": {"id": "stainless_steel", "name": "Steel - Stainless", "based_on": "steel_satin",
                  "base_color": [0.64, 0.64, 0.62], "roughness": 0.22},
    # Black oxide, the usual finish of socket screws of property class
    # 8.8 to 12.9: a dark, oiled and glossy metal whose highlights show
    # its shape.
    "black_oxide": {"id": "black_oxide", "name": "Steel - Black oxide", "based_on": "steel_satin",
                    "base_color": [0.24, 0.24, 0.25], "roughness": 0.22},
    "brass": "brass_polished",
}


class PreviewError(Exception):
    """A picture that could not be rendered (the caller draws one)."""


def finish(look):
    """The appearance a part's `appearance` names: a finish of FINISHES by
    name, else what it is (a library id, or an appearance of the design)."""
    if isinstance(look, str) and look in FINISHES:
        return FINISHES[look]
    return look


def default_finish(*texts):
    """A fastener's finish from its name, part number and the like:
    stainless for stainless steel (A2, A4), brass for brass, black oxide
    for black ones, else zinc plated steel."""
    text = " ".join(t for t in texts if isinstance(t, str)).lower()
    words = set(text.replace(",", " ").replace("-", " ").replace("_", " ").split())
    if "stainless" in text or "inox" in words or words & {"a2", "a4"}:
        return "stainless"
    if "brass" in text:
        return "brass"
    if "black" in text or "oxide" in text:
        return "black_oxide"
    return "zinc_plated"


def appearance_commands(look, body_uids, name):
    """Commands that give every body the appearance `look` (a finish's
    name, a library id or a design appearance's definition)."""
    look = finish(look)
    commands = []
    if isinstance(look, dict):
        appearance = look.get("id") or "finish"
        command = {"cmd": "create_appearance", "id": appearance, "name": look.get("name") or name}
        for key, value in look.items():
            if key not in ("id", "name"):
                command[key] = value
        commands.append(command)
    else:
        appearance = look
    for uid in body_uids:
        commands.append({"cmd": "set_body_appearance", "uid": uid, "appearance": appearance})
    return commands


# ---------------------------------------------------------------------
# mitcad-cli

def run(command, timeout=600):
    """Runs a program; its result. Never raises on its exit status."""
    try:
        return subprocess.run([str(c) for c in command], capture_output=True, text=True, encoding="utf-8",
                              errors="replace", timeout=timeout)
    except (OSError, subprocess.TimeoutExpired) as e:
        return subprocess.CompletedProcess(command, -1, "", str(e))


def tail(result, lines=6):
    text = [line.strip() for line in f"{result.stdout}\n{result.stderr}".splitlines() if line.strip()]
    return " | ".join(text[-lines:]) or f"exit status {result.returncode}"


def worker_arguments(worker):
    return ["--worker", worker] if worker else []


def renderer_available(cli, worker=None):
    """Whether `mitcad-cli render` finds a render worker (its devices are
    listed); the reason when it does not."""
    result = run([cli, "render", "--list-devices"] + worker_arguments(worker), timeout=120)
    if result.returncode == 0 and result.stdout.strip():
        return True, ""
    return False, tail(result, 3)


def design_bodies(cli, design):
    """The design's bodies as `mitcad-cli info --json` reports them: uid,
    volume, centre and bounding box."""
    result = run([cli, "info", design, "--json"])
    if result.returncode != 0:
        raise PreviewError(f"mitcad-cli info failed: {tail(result)}")
    try:
        report = json.loads(result.stdout)
    except ValueError as e:
        raise PreviewError(f"mitcad-cli info gave no JSON: {e}")
    bodies = report.get("bodies") if isinstance(report, dict) else None
    if not isinstance(bodies, list) or not bodies:
        raise PreviewError("the design has no bodies")
    return bodies


def apply_appearance(cli, design, look, scratch, name):
    """Gives every body of the design file `design` the appearance `look`,
    saved in place (a finish's name, a library id or a definition)."""
    uids = [b["uid"] for b in design_bodies(cli, design) if b.get("uid")]
    script = os.path.join(scratch, "appearance.json")
    with open(script, "w", encoding="utf-8", newline="\n") as f:
        json.dump(appearance_commands(look, uids, name), f, indent=1)
    finished = os.path.join(scratch, "appearance.mitcad")
    result = run([cli, "run", script, "--open", design, "--save", finished])
    if result.returncode != 0 or not os.path.isfile(finished):
        raise PreviewError(f"the appearance could not be set: {tail(result)}")
    shutil.copyfile(finished, design)
    os.remove(finished)


def design_report(cli, design):
    """`mitcad-cli info --json` of a design: its timeline and bodies."""
    result = run([cli, "info", design, "--json"])
    if result.returncode != 0:
        raise PreviewError(f"mitcad-cli info failed: {tail(result)}")
    try:
        report = json.loads(result.stdout)
    except ValueError as e:
        raise PreviewError(f"mitcad-cli info gave no JSON: {e}")
    return report if isinstance(report, dict) else {}


def failed_features(report, names):
    """The features of a report's timeline named in `names` that failed,
    each `name: error`."""
    timeline = report.get("timeline") or {}
    features = timeline.get("features") if isinstance(timeline, dict) else timeline
    return [f"{f.get('name')}: {f.get('error')}" for f in features or []
            if isinstance(f, dict) and f.get("name") in names and f.get("status") == "error"]


def apply_checked(cli, design, commands, scratch):
    """Runs `commands` on the design file `design` and keeps the result
    when every feature they add is made; otherwise leaves the file as it
    was. None, or why it was left."""
    names = {c["name"] for c in commands if c.get("cmd") == "add_feature" and c.get("name")}
    script = os.path.join(scratch, "features.json")
    with open(script, "w", encoding="utf-8", newline="\n") as f:
        json.dump(commands, f, indent=1)
    finished = os.path.join(scratch, "features.mitcad")
    result = run([cli, "run", script, "--open", design, "--save", finished])
    if result.returncode != 0 or not os.path.isfile(finished):
        return tail(result)
    try:
        failed = failed_features(design_report(cli, finished), names)
    except PreviewError as e:
        failed = [str(e)]
    if failed:
        os.remove(finished)
        return "; ".join(failed)
    shutil.copyfile(finished, design)
    os.remove(finished)
    return None


def design_edges(cli, design):
    """The edges of the design's bodies as `mitcad-cli info --names` lists
    them: {body uid: [{"name", "curve", "length"}]}."""
    result = run([cli, "info", design, "--names"])
    if result.returncode != 0:
        raise PreviewError(f"mitcad-cli info --names failed: {tail(result)}")
    edges = {}
    for line in result.stdout.splitlines():
        if line.startswith("Edges of ") and ": " in line:
            uid, _, data = line[len("Edges of "):].partition(": ")
            try:
                listed = json.loads(data)
            except ValueError:
                continue
            edges[uid.strip()] = [e for e in listed if isinstance(e, dict)] if isinstance(listed, list) else []
    return edges


# Screw threads (mitcad#526): the sizes the thread feature has. A screw
# whose own thread it does not have (a wood screw, a self-forming screw)
# takes the nearest size of a profile it can model, as a stand-in that
# looks right: ISO 1478's forming screw threads (ST) for wood screws, ISO
# metric coarse threads for metric self-forming screws.
FORMING_SIZES = [1.5, 1.9, 2.2, 2.6, 2.9, 3.3, 3.5, 3.9, 4.2, 4.8, 5.5, 6.3, 8.0, 9.5]


def forming_thread(diameter):
    """The forming screw thread nearest a diameter: {standard, designation}."""
    size = min(FORMING_SIZES, key=lambda s: (abs(s - diameter), s))
    return {"standard": "forming_screw", "designation": f"ST{size:g}"}


def metric_thread(diameter):
    """The ISO metric coarse thread of a diameter (`M8`)."""
    return {"designation": f"M{diameter:g}"}


def thread_command(faces, size, length=None, location=None, name="Thread", chamfer_diameter=None):
    """A cosmetic thread feature (`modeled` false: listed by the `threads`
    query, no geometry but its end chamfer) on `faces`, chamfered at the
    free end (the tip; the end at a head is not free). `chamfer_diameter`
    (an expression) makes the chamfer follow a configuration row's size,
    which the thread's designation (the default row's) does not."""
    definition = {"type": "thread", "faces": faces, "thread": size, "modeled": False, "chamfer": "both"}
    if length is not None:
        definition["length"] = length
    if location is not None:
        definition["location"] = location
    if chamfer_diameter is not None:
        definition["chamfer_diameter"] = chamfer_diameter
    return {"cmd": "add_feature", "name": name, "def": definition}


def modelled(thread):
    """A cosmetic thread's definition as a modelled one (the preview's
    profile): the same faces, size, length and chamfer."""
    definition = dict(thread["def"] if "def" in thread else thread)
    definition["modeled"] = True
    return definition


# What went wrong with a part that a generator made anyway (a thread or a
# rounding left out, a preview drawn instead of rendered): printed as a
# warning, and with a generator's --strict its exit status.
PROBLEMS = []


def warn(message):
    """A warning about a part (`<id>: …`), kept in PROBLEMS."""
    PROBLEMS.append(message)
    print(f"warning {message}", flush=True)


def strict_exit(strict, what):
    """Ends a generator run with --strict that had PROBLEMS."""
    if strict and PROBLEMS:
        raise SystemExit(f"{what}: {len(PROBLEMS)} problems with --strict:\n" + "\n".join(PROBLEMS))


# ---------------------------------------------------------------------
# The camera

def _dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def _cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def _unit(v):
    n = math.sqrt(_dot(v, v))
    if n < 1e-12:
        raise PreviewError(f"no direction: {v}")
    return [c / n for c in v]


def _bounds(bodies):
    """The bodies' bounds and their centre of mass."""
    lows, highs, centre, volume = [], [], [0.0, 0.0, 0.0], 0.0
    for body in bodies:
        box = body.get("bbox") or {}
        if "min" in box and "max" in box:
            lows.append([float(v) for v in box["min"]])
            highs.append([float(v) for v in box["max"]])
        weight = abs(float(body.get("volume") or 0.0))
        if body.get("center") is not None and weight > 0:
            centre = [c + weight * float(v) for c, v in zip(centre, body["center"])]
            volume += weight
    if not lows:
        raise PreviewError("the bodies have no bounding box")
    low = [min(p[i] for p in lows) for i in range(3)]
    high = [max(p[i] for p in highs) for i in range(3)]
    middle = [(low[i] + high[i]) / 2 for i in range(3)]
    return low, high, ([c / volume for c in centre] if volume > 0 else middle)


def part_axis(low, high, centre):
    """The direction that becomes the picture's up: along a long part (its
    longest extent), across a flat one (its thinnest), else Z; towards the
    side its mass lies on (a screw's head, which is heavier than its tip)."""
    extents = [high[i] - low[i] for i in range(3)]
    order = sorted(range(3), key=lambda i: extents[i])
    shortest, middle, longest = order
    camera = STUDIO["camera"]
    if extents[longest] > camera["long"] * extents[middle]:
        axis = longest
    elif extents[shortest] < camera["flat"] * extents[middle]:
        axis = shortest
    else:
        axis = 2
    offset = centre[axis] - (low[axis] + high[axis]) / 2
    sign = -1.0 if offset < -0.01 * max(extents[axis], 1e-9) else 1.0
    up = [0.0, 0.0, 0.0]
    up[axis] = sign
    return up


def turn_to_z(up):
    """The rotation (rows) that turns `up` to +Z."""
    z = _unit(up)
    seed = [1.0, 0.0, 0.0] if abs(z[0]) < 0.9 else [0.0, 1.0, 0.0]
    x = _unit([seed[i] - _dot(seed, z) * z[i] for i in range(3)])
    return [x, _cross(z, x), z]


def studio_view(bodies, up=None):
    """The staged part's turn (a move's matrix, or None when its axis is
    Z already) and the named view of the studio's camera."""
    low, high, centre = _bounds(bodies)
    up = _unit(up) if up is not None else part_axis(low, high, centre)
    rotation = turn_to_z(up)
    identity = all(abs(rotation[i][j] - (1.0 if i == j else 0.0)) < 1e-9 for i in range(3) for j in range(3))
    corners = [[(low, high)[(n >> i) & 1][i] for i in range(3)] for n in range(8)]
    turned = [[_dot(row, c) for row in rotation] for c in corners]
    low = [min(c[i] for c in turned) for i in range(3)]
    high = [max(c[i] for c in turned) for i in range(3)]
    extents = [high[i] - low[i] for i in range(3)]
    across = max(extents[0], extents[1], 1e-9)
    camera = STUDIO["camera"]
    kind = ("long" if extents[2] > camera["long"] * across else
            "flat" if extents[2] < camera["flat"] * across else "compact")
    elevation = math.radians(camera["elevation"][kind])
    azimuth = math.radians(camera["azimuth"])
    # From the target towards the eye.
    back = [math.cos(elevation) * math.sin(azimuth), -math.cos(elevation) * math.cos(azimuth), math.sin(elevation)]
    look = [-c for c in back]
    right = _unit(_cross(look, [0.0, 0.0, 1.0]))
    view_up = _cross(right, look)
    if extents[2] >= camera["diagonal"] * across:
        # The camera rolled: the part's head towards the upper left.
        roll = math.radians(camera["roll"])
        view_up = [math.cos(roll) * view_up[i] + math.sin(roll) * right[i] for i in range(3)]
        right = _unit(_cross(look, view_up))
    target = [(low[i] + high[i]) / 2 for i in range(3)]
    half = max(max(abs(_dot([c[i] - target[i] for i in range(3)], right)),
                   abs(_dot([c[i] - target[i] for i in range(3)], view_up))) for c in
               [[(low, high)[(n >> i) & 1][i] for i in range(3)] for n in range(8)])
    diagonal = max(math.sqrt(sum(e * e for e in extents)), 1e-3)
    if camera["perspective"]:
        distance, frame = camera["distance"] * diagonal, camera["frame"]
    else:
        distance, frame = 4 * diagonal + 10, 1.08
    view = {"eye": [target[i] + back[i] * distance for i in range(3)], "target": target, "up": view_up,
            "perspective": bool(camera["perspective"]), "height": 2 * max(half, 1e-3) * frame}
    move = None if identity else [rotation[i] + [0.0] for i in range(3)]
    return move, view, studio_lights(target, diagonal, back, azimuth)


def studio_lights(target, size, back, azimuth):
    """The studio's softboxes for a part of `size` (mm) round `target`, in
    the staged part's frame, seen from the direction `back` (towards the
    eye) at `azimuth` (radians): `add_render_light` commands."""
    side = [math.sin(azimuth), -math.cos(azimuth), 0.0]  # the camera's side, level
    right = [math.cos(azimuth), math.sin(azimuth), 0.0]  # to the right in the picture, level
    commands = []
    for light in STUDIO["lights"]:
        a = math.radians(light["normal"]["azimuth"])
        e = math.radians(light["normal"]["elevation"])
        normal = [math.cos(e) * (math.cos(a) * side[i] + math.sin(a) * right[i]) for i in range(2)] + [math.sin(e)]
        # The view direction mirrored in the normal: where the light is
        # seen in the surface.
        facing = _dot(normal, back)
        direction = _unit([2 * facing * normal[i] - back[i] for i in range(3)])
        distance = light["distance"] * size
        # A one-sided area light gives P / (pi d^2) on its axis, in W/m2
        # for P in W as if the design were in metres (the worker scales a
        # power by 10^6 mm2 per m2): the power for the irradiance asked.
        power = light["irradiance"] * math.pi * distance * distance / 1e6
        commands.append({"cmd": "add_render_light", "name": light["name"], "type": "area", "space": "world",
                         "shape": "rectangle",
                         "position": [target[i] + direction[i] * distance for i in range(3)],
                         "direction": [-c for c in direction], "power": min(power, 1e6),
                         "size": light["size"][0] * size, "size_y": light["size"][1] * size})
    return commands


# ---------------------------------------------------------------------
# PNG images

def read_png(data):
    """A PNG image (8 or 16 bits; grey, RGB, grey with alpha or RGBA; not
    interlaced): (width, height, rows of RGBA bytes)."""
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise PreviewError("no PNG image")
    at, header, chunks = 8, None, []
    while at + 8 <= len(data):
        length, kind = struct.unpack(">I4s", data[at:at + 8])
        body = data[at + 8:at + 8 + length]
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            chunks.append(body)
        elif kind == b"IEND":
            break
        at += 12 + length
    if header is None:
        raise PreviewError("a PNG image without a header")
    width, height, depth, colour, _, _, interlace = header
    channels = {0: 1, 2: 3, 4: 2, 6: 4}.get(colour)
    if channels is None or depth not in (8, 16) or interlace:
        raise PreviewError(f"a PNG image of colour type {colour}, {depth} bits, interlace {interlace}")
    size = depth // 8
    stride = width * channels * size
    raw = zlib.decompress(b"".join(chunks))
    rows, previous, at = [], bytearray(stride), 0
    for _ in range(height):
        kind, line = raw[at], bytearray(raw[at + 1:at + 1 + stride])
        at += 1 + stride
        step = channels * size
        if kind == 1:
            for i in range(step, stride):
                line[i] = (line[i] + line[i - step]) & 0xFF
        elif kind == 2:
            for i in range(stride):
                line[i] = (line[i] + previous[i]) & 0xFF
        elif kind == 3:
            for i in range(stride):
                left = line[i - step] if i >= step else 0
                line[i] = (line[i] + ((left + previous[i]) >> 1)) & 0xFF
        elif kind == 4:
            for i in range(stride):
                a = line[i - step] if i >= step else 0
                b = previous[i]
                c = previous[i - step] if i >= step else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 0xFF
        elif kind != 0:
            raise PreviewError(f"a PNG row with filter {kind}")
        previous = line
        eight = line[0::2] if size == 2 else line
        if channels == 4:
            rows.append(bytearray(eight))
            continue
        out = bytearray(width * 4)
        for x in range(width):
            pixel = eight[x * channels:(x + 1) * channels]
            if channels == 1:
                out[4 * x:4 * x + 4] = bytes((pixel[0], pixel[0], pixel[0], 255))
            elif channels == 2:
                out[4 * x:4 * x + 4] = bytes((pixel[0], pixel[0], pixel[0], pixel[1]))
            elif channels == 3:
                out[4 * x:4 * x + 4] = bytes((pixel[0], pixel[1], pixel[2], 255))
            else:
                out[4 * x:4 * x + 4] = pixel
        rows.append(out)
    return width, height, rows


TEXT_KEY = "Mitcad preview"


def write_png(width, height, rows, text=None):
    """An RGB PNG image of rows of RGB bytes, with a tEXt chunk of
    TEXT_KEY when given (the key the picture was made from)."""

    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)

    raw = bytearray()
    for row in rows:
        # Each row filtered by its left neighbour: smooth shading packs small.
        raw.append(1)
        line = bytearray(row)
        for i in range(len(line) - 1, 2, -1):
            line[i] = (line[i] - line[i - 3]) & 0xFF
        raw += line
    out = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    if text is not None:
        out += chunk(b"tEXt", TEXT_KEY.encode("latin-1") + b"\x00" + text.encode("latin-1"))
    return out + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")


def png_text(data):
    """The TEXT_KEY text of a PNG image, or None."""
    if not isinstance(data, (bytes, bytearray)) or not data.startswith(b"\x89PNG\r\n\x1a\n"):
        return None
    at = 8
    while at + 8 <= len(data):
        length, kind = struct.unpack(">I4s", data[at:at + 8])
        if kind == b"tEXt":
            key, _, value = bytes(data[at + 8:at + 8 + length]).partition(b"\x00")
            if key == TEXT_KEY.encode("latin-1"):
                return value.decode("latin-1")
        if kind == b"IDAT":
            return None
        at += 12 + length
    return None


def _weights(start, step, count, limit):
    """Box filter weights: for each of `count` output pixels, the source
    pixels (index, weight) its span [start + i step, start + (i + 1)
    step] covers; an index outside 0..limit-1 is the background."""
    out = []
    for i in range(count):
        a, b = start + i * step, start + (i + 1) * step
        taps, j = [], math.floor(a)
        while j < b:
            w = min(b, j + 1) - max(a, j)
            if w > 1e-9:
                taps.append((j, w / step))
            j += 1
        out.append(taps)
    return out


def tile(width, height, rows, size=PREVIEW_SIZE, margin=None, background=BACKGROUND):
    """A rendered RGBA image (straight alpha) laid over the background,
    cropped to a square round what covers it with `margin` on each side,
    and averaged down to `size`: rows of RGB bytes. None when nothing
    covers it (an empty render)."""
    margin = STUDIO["camera"]["margin"] if margin is None else margin
    x0, y0, x1, y1 = width, height, -1, -1
    for y, row in enumerate(rows):
        alphas = row[3::4]
        covered = [x for x, a in enumerate(alphas) if a > 10]
        if covered:
            x0, x1 = min(x0, covered[0]), max(x1, covered[-1])
            y0, y1 = min(y0, y), max(y1, y)
    if x1 < 0:
        return None
    side = max(x1 - x0 + 1, y1 - y0 + 1) * (1 + 2 * margin)
    cx, cy = (x0 + x1 + 1) / 2, (y0 + y1 + 1) / 2
    columns = _weights(cx - side / 2, side / size, size, width)
    lines = _weights(cy - side / 2, side / size, size, height)
    bg = [float(c) for c in background]

    def blended(y):
        row = rows[y]
        out = [0.0] * (3 * width)
        for x in range(width):
            a = row[4 * x + 3] / 255.0
            for c in range(3):
                out[3 * x + c] = row[4 * x + c] * a + bg[c] * (1 - a)
        return out

    cache = {}
    out_rows = []
    for taps in lines:
        for y, _ in taps:
            if 0 <= y < height and y not in cache:
                line = blended(y)
                averaged = [0.0] * (3 * size)
                for i, row_taps in enumerate(columns):
                    for x, w in row_taps:
                        source = line[3 * x:3 * x + 3] if 0 <= x < width else bg
                        for c in range(3):
                            averaged[3 * i + c] += w * source[c]
                cache[y] = averaged
    background_row = [bg[i % 3] for i in range(3 * size)]
    for taps in lines:
        out = [0.0] * (3 * size)
        for y, w in taps:
            source = cache.get(y, background_row)
            for i in range(3 * size):
                out[i] += w * source[i]
        out_rows.append(bytes(max(0, min(255, int(round(v)))) for v in out))
    return out_rows


def luminance(colour):
    """The luma of an sRGB colour (0..255), Rec. 709 weights."""
    return 0.2126 * colour[0] + 0.7152 * colour[1] + 0.0722 * colour[2]


def content(data, background=BACKGROUND, threshold=12, inset=2):
    """What a preview shows, as a dict: `width`, `height`, `share` (of its
    pixels that differ from the background: the part), `mean` (the part's
    mean colour, sRGB 0..255), and of the part's inside (its pixels
    `inset` pixels or more from the background, so that edges blended
    with the background do not count) `dark` (the luma below which 5 %
    of them lie), `bright` (the luma above which 2 % lie) and `luma`
    (its sorted lumas)."""
    width, height, rows = read_png(data)
    part = [[False] * width for _ in range(height)]
    count, total = 0, [0, 0, 0]
    for y, row in enumerate(rows):
        for x in range(width):
            pixel = row[4 * x:4 * x + 3]
            if max(abs(pixel[c] - background[c]) for c in range(3)) > threshold:
                part[y][x] = True
                count += 1
                for c in range(3):
                    total[c] += pixel[c]
    mean = [t / count for t in total] if count else [float(c) for c in background]
    lumas = []
    for y in range(inset, height - inset):
        for x in range(inset, width - inset):
            if all(part[y + dy][x + dx] for dy in range(-inset, inset + 1) for dx in range(-inset, inset + 1)):
                lumas.append(luminance(rows[y][4 * x:4 * x + 3]))
    lumas.sort()
    dark = lumas[int(0.05 * (len(lumas) - 1))] if lumas else 0.0
    bright = lumas[int(0.98 * (len(lumas) - 1))] if lumas else 0.0
    return {"width": width, "height": height, "share": count / float(width * height), "mean": mean,
            "dark": dark, "bright": bright, "luma": lumas}


# ---------------------------------------------------------------------
# The render step

def preview_key(design, up=None, threads=None):
    """What a picture is made from: the studio setup, the design file and
    the threads modelled for it (a picture whose key is the same is
    kept)."""
    digest = hashlib.sha256()
    digest.update(json.dumps(STUDIO, sort_keys=True).encode("utf-8"))
    digest.update(json.dumps([up, threads or []], sort_keys=True).encode("utf-8"))
    with open(design, "rb") as f:
        digest.update(f.read())
    return f"studio{STUDIO['version']}:{digest.hexdigest()}"


def render_preview(cli, design, scratch, worker=None, up=None, old=None, force=False, threads=None):
    """The design's picture in the studio: (PNG bytes, "rendered",
    "rendered without its thread (why)" or "kept"). `old` is the
    library's current picture of the part: kept when it was made from the
    same design and setup (unless `force`). `up` is the direction of the
    part that is up in the picture (default: part_axis). `threads` are the
    commands that added the design's cosmetic threads (thread_command): on
    the rendered copy only, each of those features is made modelled (the
    same faces, size, length and chamfer, on the faces as the part has
    them), so that the picture shows the thread's profile. Raises
    PreviewError when the render fails."""
    threads = [(t.get("name", "Thread"), modelled(t)) for t in threads or []]
    key = preview_key(design, up, [definition for _, definition in threads])
    if not force and old is not None and png_text(old) == key:
        return old, "kept"
    report = design_report(cli, design)
    bodies = report.get("bodies") or []
    if not bodies:
        raise PreviewError("the design has no bodies")
    move, view, lights = studio_view(bodies, up)
    timeline = report.get("timeline") or {}
    features = timeline.get("features") if isinstance(timeline, dict) else timeline
    uids = {f.get("name"): f.get("uid") for f in features or []
            if isinstance(f, dict) and f.get("type") == "thread"}
    commands, missing = [], []
    for name, definition in threads:
        if uids.get(name):
            commands.append({"cmd": "edit_feature", "uid": uids[name], "def": definition})
        else:
            missing.append(f"{name}: no such thread in the design")
    if move is not None:
        commands.append({"cmd": "add_feature", "name": "Preview turn", "def": {
            "type": "move", "bodies": [b["uid"] for b in bodies if b.get("uid")],
            "transform": {"type": "free", "matrix": move}}})
    commands.append({"cmd": "set_render_settings",
                     "environment": STUDIO["environment"],
                     "background": {"mode": "color", "color": [c / 255.0 for c in BACKGROUND]},
                     "ground": STUDIO["ground"], "film": STUDIO["film"],
                     "output": {"width": RENDER_SIZE, "height": RENDER_SIZE, "aspect": "fixed",
                                "samples": STUDIO["samples"], "time_limit": 0, "denoise": STUDIO["denoise"],
                                "transparent": True, "format": "png"}})
    commands += lights
    commands.append(dict(view, cmd="add_named_view", name="Preview", replace=True))
    work = tempfile.mkdtemp(prefix="preview-", dir=scratch)
    script = os.path.join(work, "studio.json")
    staged = os.path.join(work, "staged.mitcad")
    image = os.path.join(work, "render.png")
    with open(script, "w", encoding="utf-8", newline="\n") as f:
        json.dump(commands, f, indent=1)
    result = run([cli, "run", script, "--open", design, "--save", staged])
    if result.returncode != 0:
        raise PreviewError(f"the studio could not be set up: {tail(result)}")
    how = "rendered"
    if threads:
        # A thread the copy could not model leaves the shank plain.
        failed = missing + failed_features(design_report(cli, staged), {name for name, _ in threads})
        if failed:
            how = f"rendered without its thread ({'; '.join(failed)})"
    result = run([cli, "render", staged, "-o", image, "--view", "Preview", "--size", f"{RENDER_SIZE}x{RENDER_SIZE}",
                  "--samples", str(STUDIO["samples"]), "--transparent", "--format", "png"]
                 + ([] if STUDIO["denoise"] else ["--no-denoise"]) + worker_arguments(worker))
    if result.returncode != 0 or not os.path.isfile(image):
        raise PreviewError(f"mitcad-cli render failed: {tail(result)}")
    with open(image, "rb") as f:
        width, height, rows = read_png(f.read())
    pixels = tile(width, height, rows)
    if pixels is None:
        raise PreviewError("the render shows nothing")
    data = write_png(PREVIEW_SIZE, PREVIEW_SIZE, pixels, key)
    if len(data) > MAX_BYTES:
        raise PreviewError(f"the picture has {len(data)} bytes, more than {MAX_BYTES}")
    return data, how


def read_old(path):
    """The library's current picture at `path`, or None."""
    try:
        with open(path, "rb") as f:
            return f.read()
    except OSError:
        return None
