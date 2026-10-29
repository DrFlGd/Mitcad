#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Summarize paint samples, never input pacing, from the #770 diagnostic."""
import hashlib
import json
from pathlib import Path
import re
import statistics
import sys


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def stats(values):
    ordered = sorted(values)
    return {"count": len(values), "median_ms": statistics.median(values), "mean_ms": statistics.mean(values),
            "min_ms": min(values), "p95_ms": ordered[max(0, (len(ordered) * 95 + 99) // 100 - 1)], "max_ms": max(values)}


def main():
    log, output, frontend, app, cli, sync, revision = sys.argv[1:]
    root = Path(output)
    text = Path(log).read_text()
    assert "TestDriver: done" in text and "TestDriver: FAIL" not in text
    environment = re.findall(r"^Viewport timing environment: (.*)$", text, re.M)
    assert environment, "missing actual renderer and framebuffer environment"
    assert all("Apple" in one and "llvmpipe" not in one.lower() and "software" not in one.lower()
               for one in environment), "this diagnostic requires an actual Apple GPU"
    if frontend == "native":
        assert "Native UI: AppKit shell with embedded OCCT viewport" in text
    order = json.loads((root / "files/order.json").read_text())
    # First and last 'none' calls establish/restore the test state. Every
    # middle call owns exactly two traversals: warmup then measurement.
    segments = re.split(r"^Body highlight diagnostic: ([a-z-]+), 15 bodies, (\d+) overlays\s*$", text, flags=re.M)
    records = []
    for index, mode in enumerate(order):
        at = 1 + 3 * (index + 1)
        assert segments[at] == mode, f"run {index}: wrong mode"
        overlays = int(segments[at + 1])
        assert overlays == (0 if mode == "none" else 15)
        body = segments[at + 2]
        drags = re.findall(r"Held drag: begin\s*\n(.*?)Held drag: end, delivered=\s*(\d+) elapsed_ms=\s*(\d+)",
                           body, re.S)
        assert len(drags) == 2, f"run {index}: expected warmup and measurement"
        gesture, delivered, _elapsed = drags[1]
        assert int(delivered) == 60, f"run {index}: incomplete input"
        # 'Timing frame swap' includes pacing, window presentation and other
        # work. Only the paint's own duration is the comparison metric.
        samples = [float(value) for value in re.findall(r"^Timing frame: ([0-9.]+) ms", gesture, re.M)]
        assert len(samples) >= 30, f"run {index}: too few rendered samples: {len(samples)}"
        navigation = re.findall(r"^View navigation: (\d+) frame\(s\), (\d+) repaint\(s\).*", body, re.M)
        assert navigation, f"run {index}: no navigation/repaint accounting"
        frames, repaints = map(int, navigation[-1])
        assert frames >= 30, f"run {index}: only {frames} navigation frames"
        picture = root / f"files/run-{index}-{mode}.png"
        assert picture.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"), f"run {index}: no PNG"
        records.append({"run": index, "style": mode, "overlays": overlays, "delivered_moves": int(delivered),
                        "navigation_frames": frames, "widget_repaints": repaints,
                        "paint": stats(samples), "paint_samples_ms": samples, "capture": str(picture),
                        "capture_sha256": sha(picture)})
    grouped = {mode: stats([one["paint"]["median_ms"] for one in records if one["style"] == mode])
               for mode in dict.fromkeys(order)}
    baseline = grouped["none"]["median_ms"]
    for mode in grouped:
        grouped[mode]["relative_to_none"] = grouped[mode]["median_ms"] / baseline
        grouped[mode]["delta_ms"] = grouped[mode]["median_ms"] - baseline
    report = {"frontend": frontend, "source_revision_supplied": revision,
              "source_revision_verified_by_harness": False, "gpu_completion_fence": sync != "0",
              "metric": "OCCT viewport paintGL Timing frame; includes GPU completion only when fenced",
              "excludes": "input pacing, Qt composition and display presentation",
              "fixture": "15 synthetic independent boxes; identical empty selection, panel, camera and orbit",
              "renderer": environment, "app_sha256": sha(app), "cli_sha256": sha(cli),
              "log_sha256": sha(log), "fixture_sha256": sha(root / "files/scene.mitcad"),
              "driver_sha256": sha(root / "files/measure.mitcad-ui"),
              "report_script_sha256": sha(__file__), "runs": records, "styles": grouped,
              "visual_review": "pending; inspect default and alternative captures before choosing a change",
              "production_change": "none; this diagnostic does not select an optimization"}
    (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    rows = ["run\tstyle\tpaint_count\tpaint_median_ms\tpaint_p95_ms\tnavigation_frames\twidget_repaints"]
    for one in records:
        rows.append("\t".join(map(str, [one["run"], one["style"], one["paint"]["count"],
            one["paint"]["median_ms"], one["paint"]["p95_ms"], one["navigation_frames"], one["widget_repaints"]])))
    (root / "samples.tsv").write_text("\n".join(rows) + "\n")
    print(json.dumps({"frontend": frontend, "fenced": sync != "0", "styles": grouped}, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, OSError, ValueError, IndexError) as error:
        raise SystemExit(f"FAIL: whole-body highlight evidence: {error}")
