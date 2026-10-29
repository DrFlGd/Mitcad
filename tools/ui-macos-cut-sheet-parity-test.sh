#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/drawings app/browser/DocumentHost app/MainWindowNative app/framework/TestDriver
# check-all sources: app/platform/macos/NativeDialogs app/platform/macos/NativeLayout core/model-api/src/cut_sheets.rs
# Cut Sheet (mitcad#853) through actual Qt/AppKit controls, both themes:
# kind visibility, multiple body selection/count, cancel, material/undo,
# native/Qt part context menus, copy/delete/undo, DXF file panel and output.
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ "$#" -ne 4 ]; then
  echo "usage: $0 <app> <mitcad-cli> <output directory> qt|native" >&2
  exit 2
fi
cli=$2
frontend=$4
case "$frontend" in qt|native) ;; *) exit 2 ;; esac
ui_init "$1" "$3"
UI_TIMEOUT=240
mkdir -p "$UI_OUT/files"
# The real Save panel resolves symlinks in Go to Folder. Use the same
# physical destination for its pre-save path assertion and output checks.
export MITCAD_TEST_WORK=$(cd "$UI_OUT/files" && pwd -P)
for scheme in light dark; do
  rm -rf "$MITCAD_SETTINGS_DIR"
  mkdir -p "$MITCAD_SETTINGS_DIR"
  model=$MITCAD_TEST_WORK/cut-$frontend-$scheme.mitcad
  rm -f "$MITCAD_TEST_WORK/cut-$frontend-$scheme.dxf"
  "$cli" run "$here/ui-scripts/cut-sheet-parity.json" --save "$model" > "$UI_OUT/setup-$scheme.log" 2>&1 || {
    cat "$UI_OUT/setup-$scheme.log" >&2
    exit 1
  }
  script=$UI_OUT/cut-$frontend-$scheme.mitcad-ui
  python3 "$here/ui-scripts/cut-sheet-parity-script.py" "$frontend" > "$script"
  args=(--ui="$frontend" --no-recovery --no-mcp --open "$model")
  [ "$frontend" != qt ] || args+=(--no-native-dialogs)
  MITCAD_TEST_COLOR_SCHEME=$scheme MITCAD_TEST_STEP_MS=90 MITCAD_TEST_INPUT=$script \
    ui_run_app "$frontend-$scheme" "${args[@]}"
  ui_expect_log '^TestDriver: done' "$frontend/$scheme: complete Cut Sheet workflow"
  for problem in 'TestDriver: FAIL' 'Unable to simultaneously satisfy constraints' 'Native Mac canvas:'; do
    ui_expect_no_log "$problem" "$frontend/$scheme: no $problem"
  done
  [ "$UI_FAILURES" = 0 ] || { ui_log_tail; exit 1; }
  python3 - "$model" "$MITCAD_TEST_WORK" "$frontend" "$scheme" <<'PY'
import json
from collections import Counter
from pathlib import Path
import sys

model, work = map(Path, sys.argv[1:3])
frontend, scheme = sys.argv[3:]
drawings = json.loads(model.read_text())["drawings"]
assert len(drawings) == 1, drawings
assert drawings[0]["name"] == "CutSheet1", drawings
cut = drawings[0]["cut_sheet"]
assert (cut["width"], cut["height"]) == (800, 400), cut
parts = cut["parts"]
assert [part["id"] for part in parts] == [1, 2, 3, 4], parts
assert Counter(part["body"] for part in parts) == {"F2.b0": 2, "F4.b0": 2}, parts
assert parts[0]["rotation"] == 90, parts[0]
assert all(part.get("rotation", 0) == 0 for part in parts[1:]), parts
dxf = (work / f"cut-{frontend}-{scheme}.dxf").read_text()
for layer in ["CUT_OUTER", "LABEL"]:
    assert layer in dxf, layer
assert "CIRCLE" in dxf, "disk must remain a circle in the actual export"
for surface in ["create", "parts", "workspace"]:
    image = work / f"cut-{surface}-{frontend}-{scheme}.png"
    assert image.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"), image
print("Verified: saved material/parts/rotation, real DXF layers and circle, and visible-control captures")
PY
  [ "$?" -eq 0 ] || exit 1
done
echo "PASS: Cut Sheet UI parity ($frontend, light and dark)"
