#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Check geometry, persisted expressions and bounded delivered drag evidence."""
import json
import math
from pathlib import Path
import re
import sys

log, model, case, output = sys.argv[1:]
text = Path(log).read_text()
design = json.loads(Path(model).read_text())
face = case == "face-ring"
command = "Move Face" if face else "Move/Copy"
key = "tx" if case == "body-arrow" else "px" if case == "point-arrow" else "angle"
drag = re.search(r"^Manipulator " + re.escape(command) + " " + key + r" dragged to ([+-]?[0-9.]+)$", text, re.M)
assert drag, "no completed actual handle drag"
value = float(drag[1])
assert abs(value) > 1e-8 and math.isfinite(value), "drag did not produce a finite nonzero motion"
held = re.search(r"Held drag: begin\n(.*?)Held drag: end, delivered= (\d+) elapsed_ms= (\d+)", text, re.S)
assert held and int(held[2]) == 64 and "Held drag: resumed" in held[1], "incomplete held input"
assert "Held drag: exact preview available before release" not in text, "rigid-proxy wait contract changed"
assert not re.search(r"^Progress dialog shown: preview", text, re.M), "preview blocked the drag"
feedback = re.search(r"^Manipulator feedback " + re.escape(command) + " " + key +
                    r": moves=(\d+) semantic=(\d+) visual=(\d+) paints=(\d+)$", text, re.M)
assert feedback, "missing delivered/semantic/visual/paint counters"
moves, semantic, visual, paints = map(int, feedback.groups())
assert moves == 64 and 1 <= semantic <= visual <= moves and paints > 0, feedback[0]


def captured(prefix):
    match = re.search(r"^TestDriver: shown body " + prefix + r": .*?, volume ([^,]+), bounds (.+)$", text, re.M)
    assert match, f"no {prefix} displayed geometry snapshot"
    return [float(match[1]), *map(float, match[2].split())]


original, preview = captured("original"), captured("preview")
assert any(abs(a - b) > 1e-5 for a, b in zip(original, preview)), "handle did not change displayed geometry"
features = [f for f in design["features"] if f.get("type") in ("move", "move_occurrence", "move_face")]
assert len(features) == 1, "commit did not leave exactly one motion feature"
feature = features[0]
transform = feature["transform"]
parameters = {p["name"]: p["expression"] for p in design.get("parameters", [])}


def scalar(expression):
    expression = parameters.get(str(expression), expression)
    parsed = re.fullmatch(r"\s*([+-]?[0-9.]+(?:[eE][+-]?[0-9]+)?)\s*(mm|deg|°|rad)?\s*", str(expression))
    assert parsed, f"unsupported saved numeric expression: {expression}"
    number = float(parsed[1])
    return math.radians(number) if parsed[2] in ("deg", "°") else number


if case == "body-arrow":
    assert feature["type"] == "move" and feature["bodies"] == ["F2.b0"]
    assert transform["type"] == "free"
    committed = float(transform["matrix"][0][3])
elif case == "point-arrow":
    assert feature["type"] == "move_occurrence" and feature["occurrences"] == ["O2"]
    assert transform["type"] == "point_to_position" and transform["point_occurrence"] == "O2"
    assert "F8:end(" in json.dumps(transform["point"]), "the selected peg rim was not persisted"
    committed = scalar(transform["x"])
else:
    assert feature["type"] == ("move_face" if face else "move_occurrence")
    assert transform["type"] == "rotate"
    if not face:
        assert feature["occurrences"] == ["O2"]
    else:
        assert feature["body"] == "F2.b0" and len(feature["faces"]) == 1
        assert "E{" in json.dumps(transform["axis"]), "the actual top/front edge was not persisted"
    committed = scalar(transform["angle"])
assert abs(committed - value) <= 1e-6, f"saved motion {committed} differs from handle's {value}"
evidence = {"case": case, "delivered_moves": int(held[2]), "elapsed_ms_observed": int(held[3]),
            "dragged_value": value, "committed_value": committed, "original": original, "preview": preview,
            "semantic_changes": semantic, "visual_updates": visual, "overlay_paints": paints}
if face:
    assert abs(value - math.radians(5)) > 1e-6, "ring left the initial tilt unchanged"
    expected = [48000 * (1 + math.tan(value)), 48000 * (1 - math.tan(value))]
    assert min(abs(preview[0] - v) for v in expected) <= max(1e-5, preview[0] * 1e-6), \
        f"tilted block volume {preview[0]} disagrees with the edge rotation {value}"
    assert re.search(r"^Preview installed: Move Face, dragging=1$", held[1], re.M), "no exact held geometry"
    assert "Drag proxy Move Face: shown" not in text, "Move Face unexpectedly used a rigid proxy"
    evidence["exact_held_installs"] = held[1].count("Preview installed: Move Face, dragging=1")
else:
    assert abs(original[0] - preview[0]) <= original[0] * 1e-6, "rigid move changed volume"
    summary = re.search(r"^Drag proxy Move/Copy " + key +
        r": (\d+) inputs, (\d+) proxy updates .*?, (\d+) exact previews .*?, (\d+) frames in", text, re.M)
    assert summary, "no proxy input/update/exact/frame counters"
    inputs, proxies, exacts, frames = map(int, summary.groups())
    minimum = 3 if key == "angle" else 10
    assert minimum <= inputs <= 64 and minimum <= proxies <= 128 and frames > 0, summary[0]
    assert exacts <= 3, "exact recompute per move instead of provisional motion"
    assert f"Drag proxy Move/Copy: shown for {key} " in held[1], "no proxy during held input"
    distance = re.search(r"^Drag proxy Move/Copy: the exact preview of .* replaced it .*?, ([0-9.]+) mm from the proxy$",
                         text, re.M)
    assert distance and float(distance[1]) <= 1e-6, "exact preview differs from last proxy location"
    if case != "body-arrow":
        assert re.search(r"^Timing display: .*, 0 new, [1-9][0-9]* placed", held[1], re.M), "component objects were recreated"
    # Last interactive preview and first final commit reuse moved meshes.
    interval = text.split("Held drag: begin\n", 1)[1].split("command edit.undo", 1)[0]
    for tier in ("interactive", "final"):
        meshes = re.findall(r"^Mesh trace .* " + tier + r": \d+ bodies ready, (\d+) meshed,", interval, re.M)
        assert meshes and int(meshes[-1]) == 0, f"{tier}: moved faces were meshed again"
    evidence.update(inputs=inputs, proxies=proxies, exacts=exacts, frames=frames, proxy_distance=float(distance[1]))
Path(output).write_text(json.dumps(evidence, indent=2) + "\n")
print("Verified held motion:", json.dumps(evidence, sort_keys=True))
