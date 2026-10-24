#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Physical macOS batch acceptance, with actual worker events and per-worker RSS.

Usage: render-metal-acceptance.py <cli> <worker> <output-directory>
Missing hardware skips unless MITCAD_RENDER_REQUIRE_METAL=1; fallback fails.
Synthetic scenes and images are retained with the commands, events and timings.
"""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import signal
import subprocess
import sys
import time


def check(condition, reason):
    if not condition:
        raise RuntimeError(reason)


def scenes():
    """Different material/light/ground paths; no external assets or models."""
    box = {"cmd": "add_feature", "def": {"type": "box", "plane": "xy",
           "corner": [-30, -20], "length": 60, "width": 40, "height": 20,
           "operation": "new_body"}}
    view = {"cmd": "add_named_view", "name": "Home", "eye": [180, -240, 170],
            "target": [15, 0, 15], "up": [0, 0, 1], "height": 150, "perspective": True}
    settings = {"cmd": "set_render_settings", "output": {
        "samples": 64, "width": 320, "height": 240, "aspect": "fixed"}}
    textures = json.loads((Path(__file__).parent / "cli/tests/render_texture_maps.json").read_text())
    mapped = next(x for x in textures if x.get("id") == "mapped")
    common = [box, {"cmd": "add_feature", "def": {"type": "sphere", "plane": "xy",
               "center": [50, 0], "diameter": 30, "operation": "new_body"}}, view, settings]
    product = common + [mapped,
        {"cmd": "set_body_appearance", "uid": "F1.b0", "appearance": "mapped"},
        {"cmd": "set_body_appearance", "uid": "F2.b0", "appearance": "chrome"},
        {"cmd": "add_render_light", "type": "area", "position": [0, -70, 140],
         "direction": [0, 0, -1], "power": 8, "size": 60, "size_y": 40},
        {"cmd": "set_render_settings", "ground": {"shadows": True, "reflections": True}}]
    glass = common + [
        {"cmd": "set_body_appearance", "uid": "F1.b0", "appearance": "paint_red"},
        {"cmd": "set_body_appearance", "uid": "F2.b0", "appearance": "glass_clear"},
        {"cmd": "set_render_settings", "output": {"transparent": True},
         "ground": {"shadows": True, "reflections": False}},
        {"cmd": "add_render_light", "type": "point", "position": [-80, -100, 160],
         "color": [0.7, 0.8, 1], "power": 8}]
    instances = [{"cmd": "create_component", "name": "RepeatedBlock"}, box,
        {"cmd": "set_body_appearance", "uid": "F1.b0", "appearance": "plastic_red"},
        {"cmd": "activate_component", "component": "Root"}]
    instances += [{"cmd": "copy_occurrence", "occurrence": "RepeatedBlock:1",
                   "transform": {"translation": [x * 75, y * 65, 0]}}
                  for y in range(2) for x in range(3) if (x, y) != (0, 0)]
    instances += [{**view, "eye": [360, -430, 300], "target": [75, 30, 10], "height": 300}, settings]
    return {"textured-reflections": product, "glass-transparent": glass, "instances": instances}


def compare(a, b, read_png):
    aw, ah, ac, ar = read_png(str(a))
    bw, bh, bc, br = read_png(str(b))
    check((aw, ah) == (bw, bh), "image dimensions differ")
    # Compare straight RGB and alpha separately, including transparent pixels.
    differences = sorted(abs(x - y) for ra, rb in zip(ar, br)
                         for pa, pb in zip(zip(*[iter(ra)] * ac), zip(*[iter(rb)] * bc))
                         for x, y in zip(pa[:3], pb[:3]))
    mean = sum(differences) / len(differences)
    p99 = differences[(len(differences) - 1) * 99 // 100]
    alpha = 0
    if ac == 4 and bc == 4:
        alpha = max(abs(x - y) for ra, rb in zip(ar, br) for x, y in zip(ra[3::4], rb[3::4]))
    check(mean <= 2 and p99 <= 24, f"RGB comparison failed: mean={mean:.4f}, p99={p99}")
    check(alpha <= 24, f"alpha differs by {alpha}")
    return {"mean_rgb": mean, "p99_rgb": p99, "max_alpha": alpha}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("worker", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--size", default="320x240")
    parser.add_argument("--modes", default="cpu,metal-cpu,metal,raw-cpu,raw-metal")
    parser.add_argument("--scenes", default="textured-reflections,glass-transparent,instances")
    args = parser.parse_args()
    args.cli, args.worker, args.output = args.cli.resolve(), args.worker.resolve(), args.output.resolve()
    width, height = map(int, args.size.split("x"))
    check(width > 0 and height > 0, "positive image dimensions required")
    check(set(args.modes.split(",")) <= {"cpu", "metal-cpu", "metal", "raw-cpu", "raw-metal"}, "unknown mode")
    check(set(args.scenes.split(",")) <= set(scenes()), "unknown scene")
    args.output.mkdir(parents=True, exist_ok=True)
    spec = importlib.util.spec_from_file_location("image_stats", Path(__file__).with_name("ui-image-stats.py"))
    stats = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(stats)
    environment = os.environ.copy()
    for key in ("MITCAD_RENDER_TEST_DENOISER", "MITCAD_RENDER_TEST_COMPARE_DENOISER",
                "MITCAD_RENDER_TEST_DENOISER_FAILURE", "MITCAD_RENDER_TEST_DEVICE_FAILURE",
                "CYCLES_TEST_SINGLE_SAMPLE_UPDATES", "MITCAD_RENDER_REQUIRE_METAL"):
        environment.pop(key, None)
    listed = subprocess.run([str(args.worker), "--list-devices"], env=environment,
                            capture_output=True, text=True, check=True, timeout=30)
    (args.output / "devices.json").write_text(listed.stdout)
    devices = json.loads(listed.stdout)["devices"]
    metal = [d for d in devices if d["type"] == "METAL" and d.get("denoiser_available")]
    if not metal or any("Paravirtual" in d["name"] for d in metal):
        if any(os.environ.get(key, "0") not in ("", "0")
               for key in ("MITCAD_RENDER_REQUIRE_METAL", "MITCAD_RENDER_REQUIRE_GPU")):
            raise RuntimeError("physical Metal/OIDN required")
        print("SKIP: no physical Metal/OIDN device", flush=True)
        sys.exit(77)
    runs, comparisons = [], []
    for scene_name, scene in scenes().items():
        if scene_name not in args.scenes.split(","):
            continue
        scene.append({"cmd": "set_render_settings", "output": {"width": width, "height": height}})
        scene_path = args.output / (scene_name + ".json")
        scene_path.write_text(json.dumps(scene, indent=2) + "\n")
        for mode, device, denoiser in (("cpu", "cpu", "cpu"), ("metal-cpu", "metal", "cpu"),
                                        ("metal", "metal", "metal"), ("raw-cpu", "cpu", "cpu"),
                                        ("raw-metal", "metal", "metal")):
            if mode not in args.modes.split(","):
                continue
            stem = args.output / (scene_name + "-" + mode)
            events_path, metrics_path = Path(str(stem) + ".events"), Path(str(stem) + ".time")
            wrapper = Path(str(stem) + ".sh")
            wrapper.write_text("#!/bin/bash\nset -o pipefail\n/usr/bin/time -l " +
                shlex.quote(str(args.worker.resolve())) + ' "$@" 2>' + shlex.quote(str(metrics_path)) +
                " | /usr/bin/tee " + shlex.quote(str(events_path)) + "\nexit ${PIPESTATUS[0]}\n")
            wrapper.chmod(0o700)
            image = Path(str(stem) + ".png")
            command = [str(args.cli), "render", str(scene_path), "--worker", str(wrapper),
                       "--device", device, "-o", str(image)]
            if mode.startswith("raw-"):
                command.append("--no-denoise")
            env = {**environment, "MITCAD_RENDER_TEST_DENOISER": denoiser}
            start = time.monotonic()
            with subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  text=True, start_new_session=True) as process:
                try:
                    stdout, stderr = process.communicate(timeout=240)
                except subprocess.TimeoutExpired:
                    # The wrapper and actual GPU worker belong to this group.
                    os.killpg(process.pid, signal.SIGKILL)
                    stdout, stderr = process.communicate()
                    Path(str(stem) + ".log").write_text(stdout + stderr)
                    raise RuntimeError(f"{stem.name}: worker timed out") from None
                result = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
            wall = time.monotonic() - start
            Path(str(stem) + ".log").write_text(result.stdout + result.stderr)
            check(result.returncode == 0, f"{stem.name} failed: {result.stdout}{result.stderr}")
            events = [json.loads(line) for line in events_path.read_text().splitlines() if line.startswith("{")]
            if scene_name == "instances":
                check(any(e.get("event") == "status" and e.get("text") == "scene of 6 bodies, 1 meshes"
                          for e in events), "repeated occurrences did not share their mesh")
            check(not any(e.get("event") in ("error", "warning") for e in events), f"{stem.name}: warning/fallback")
            done = [e for e in events if e.get("event") == "done"]
            check(len(done) == 1 and done[0]["samples"] == 64, f"{stem.name}: no full final image")
            actual = done[0]["device"]
            check(actual["type"] == device.upper(), f"{stem.name}: wrong actual device {actual}")
            if device == "metal":
                check(actual["metalrt"] == (actual["metalrt_available"] and actual["metalrt_default"]),
                      f"{stem.name}: wrong MetalRT policy")
            if not mode.startswith("raw-"):
                oidn = actual["denoiser_device"]
                check(oidn["executed"] and oidn["type"] == denoiser.upper(), f"{stem.name}: wrong denoiser {oidn}")
            else:
                check(not actual["denoiser_device"]["executed"], f"{stem.name}: denoising was not disabled")
            w, h, channels, rows = stats.read_png(str(image))
            check((w, h) == (width, height) and len(set(b"".join(rows))) > 16, f"{stem.name}: blank/wrong image")
            covered = sum(sum(pixel[:3]) < 3 * 180 and (channels == 3 or pixel[3] >= 200)
                          for row in rows for pixel in zip(*[iter(row)] * channels))
            check(covered >= 32, f"{stem.name}: no visible synthetic geometry over the light background")
            if scene_name == "glass-transparent":
                check(channels == 4 and min(min(row[3::4]) for row in rows) == 0
                      and max(max(row[3::4]) for row in rows) == 255, "missing transparent silhouette")
            memory = re.search(r"(\d+)\s+maximum resident set size", metrics_path.read_text())
            check(memory, f"{stem.name}: no per-worker peak RSS")
            record = {"scene": scene_name, "mode": mode, "command": command, "worker_seconds": done[0]["seconds"],
                      "wall_seconds": wall, "peak_worker_rss_bytes": int(memory[1]), "device": actual}
            runs.append(record)
            (args.output / "results.json").write_text(json.dumps({"runs": runs, "comparisons": comparisons}, indent=2))
            print(json.dumps(record), flush=True)
        for left, right in (("cpu", "metal-cpu"), ("cpu", "metal"), ("metal-cpu", "metal"),
                            ("raw-cpu", "raw-metal")):
            if left not in args.modes.split(",") or right not in args.modes.split(","):
                continue
            difference = compare(args.output / f"{scene_name}-{left}.png",
                                 args.output / f"{scene_name}-{right}.png", stats.read_png)
            comparisons.append({"scene": scene_name, "left": left, "right": right, **difference})
        (args.output / "results.json").write_text(json.dumps({"runs": runs, "comparisons": comparisons}, indent=2))
    print("PASS: required physical Metal scene/denoiser matrix", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.SubprocessError) as error:
        print("FAIL: " + str(error), file=sys.stderr)
        sys.exit(1)
