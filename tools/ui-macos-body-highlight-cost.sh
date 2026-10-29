#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Diagnostic only (mitcad#770): compare the whole-body overlay's GPU cost
# without changing the model, command panel or camera between variants.
# No performance threshold and no production setting is changed here.
# Usage: ui-macos-body-highlight-cost.sh <app> <cli> <new-output> [qt|native]
# Run the frontends serially. MITCAD_VIEWPORT_SYNC=1 (default) includes a
# GPU-completion fence in Timing frame; 0 records CPU submission instead.
# check-all sources: app/OcctViewer app/framework/TestDriver tools/cli
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
  echo "usage: $0 <app> <cli> <new-output> [qt|native]" >&2
  exit 2
fi
[ ! -e "$3" ] || { echo "FAIL: preserve existing diagnostic evidence: $3" >&2; exit 2; }
cli=$2
frontend=${4:-qt}
case "$frontend" in qt|native) ;; *) echo 'FAIL: frontend must be qt or native' >&2; exit 2 ;; esac
ui_init "$1" "$3"
UI_TIMEOUT=240
export MITCAD_TEST_WORK=$UI_OUT/files
mkdir -p "$MITCAD_TEST_WORK"
python3 - "$MITCAD_TEST_WORK" <<'PY'
import json
from pathlib import Path
import sys
root = Path(sys.argv[1])
commands = [{"comment": "SPDX-License-Identifier: MIT"}]
for y in range(3):
    for x in range(5):
        commands.append({"cmd": "add_feature", "name": f"Block {y * 5 + x + 1}", "def": {
            "type": "box", "plane": "xy", "corner": [x * 45, y * 45],
            "length": 35, "width": 35, "height": 20 + (x + y) * 4, "operation": "new_body"}})
commands.append({"expect": {"bodies": 15}})
(root / "scene.json").write_text(json.dumps(commands, indent=2) + "\n")
# Two traversals reverse the variant order, so a cold start or changing
# machine load is visible in individual results, not hidden in one average.
order = ["none", "default", "opaque", "no-boundaries", "no-boundaries", "opaque", "default", "none"]
lines = ["# SPDX-License-Identifier: MIT", "wait-idle 60", "window-resize 1280 820", "focus-main",
         "expect-viewport", "test-body-highlights none", "command view.orthographic", "command view.home", "command view.fit"]
for run, mode in enumerate(order):
    lines += [f"test-body-highlights {mode}", "command view.home", "command view.fit", "sleep 500",
              # An unreported warmup traversal builds this variant's GPU
              # presentation. Restore the same camera before measuring.
              "drag 50 52 68 62 middle", "sleep 500", "command view.home", "command view.fit", "sleep 500",
              "capture-view-orientation before", "expect-selection 0 0", "drag 50 52 68 62 middle", "sleep 500",
              "expect-view-orientation ${before} changed", "expect-view-buttons none",
              f"screenshot-view '${{MITCAD_TEST_WORK}}/run-{run}-{mode}.png'"]
lines += ["test-body-highlights none", "expect-selection 0 0",
          "expect-never 'OCCT view event failed|Recompute failed|Native Mac canvas:'"]
(root / "measure.mitcad-ui").write_text("\n".join(lines) + "\n")
(root / "order.json").write_text(json.dumps(order) + "\n")
PY
"$cli" run "$MITCAD_TEST_WORK/scene.json" --save "$MITCAD_TEST_WORK/scene.mitcad" > "$UI_OUT/setup.log" 2>&1 || {
  cat "$UI_OUT/setup.log" >&2
  exit 1
}
MITCAD_LOG_TIMING=1 MITCAD_TEST_STEP_MS=16 MITCAD_TEST_SETTLE_MS=0 \
  MITCAD_TEST_DRAG_STEPS=60 MITCAD_TEST_DRAG_PAUSE_TICKS=0 \
  MITCAD_VIEWPORT_SYNC=${MITCAD_VIEWPORT_SYNC:-1} MITCAD_TEST_INPUT=$MITCAD_TEST_WORK/measure.mitcad-ui \
  ui_run_app highlights --ui="$frontend" --no-recovery --no-mcp --open "$MITCAD_TEST_WORK/scene.mitcad"
ui_expect_log '^TestDriver: done' 'the diagnostic completed'
ui_expect_no_log 'TestDriver: FAIL' 'no driver failure'
[ "$UI_FAILURES" = 0 ] || { ui_log_tail; exit 1; }
python3 "$here/ui-body-highlight-cost-report.py" "$UI_LOG_FILE" "$UI_OUT" "$frontend" "$UI_APP" "$cli" \
  "${MITCAD_VIEWPORT_SYNC:-1}" "${MITCAD_TEST_SOURCE_REVISION:-unrecorded}" || {
  ui_fail 'invalid highlight diagnostic evidence'
  exit 1
}
echo "PASS: isolated whole-body highlight cost evidence ($frontend): $UI_OUT/report.json"
