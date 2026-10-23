#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/ExtrudeCommands.cpp app/commands/SweepCommands.cpp
# check-all sources: app/commands/CommandSupport.cpp app/framework/CommandSession.cpp
# check-all sources: core/model/src/features/extrude.rs core/model/src/features/sweep.rs tools/cli
# Editing keeps what the panels do not show (mitcad#543), on a design
# mitcad-cli makes as the imports make them:
#   - Extrude1: a 20 x 20 square extruded 10 along its own direction (0, 3,
#     4) (an `extrude` with `direction`): 400 x 10 x 0.8 = 3200 mm3. Edited
#     to 12: 3840 mm3, still along the direction (4800 along the normal).
#   - Sweep1: a circle of radius 2 swept 40 along a line about a constant
#     binormal (a `sweep` with `frame`, the FreeCAD import's Binormal
#     pipes). Edited (Flip on and off, OK): its frame stays; saved, the file
#     holds the direction and the frame.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-kept-options-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CLI=${UI_CLI:-$(dirname "$UI_APP")/../tools/cli/mitcad-cli}
mkdir -p "$ROOT/build"
WORK=$(mktemp -d "$ROOT/build/ui-kept-options.XXXXXX")
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
FILE="$WORK/kept.mitcad"

cat > "$WORK/kept.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "direction": [0, 3, 4], "operation": "new_body"}},
  {"expect": {"body": "Body1", "volume": 3200, "tolerance": 1e-9}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_line", "sketch": "F3", "start": [0, 60], "end": [40, 60]},
  {"cmd": "sketch.create", "plane": "yz"},
  {"cmd": "sketch.add_circle", "sketch": "F4", "center": [0, 60], "diameter": 4},
  {"cmd": "add_feature", "def": {"type": "sweep", "profiles": [{"sketch": "F4", "region": "r{c1}"}],
    "path": {"sketch": "F3", "curves": ["c1"]},
    "frame": {"type": "constant_binormal", "binormal": [0, 0, 1]}, "operation": "new_body"}},
  {"expect": {"feature": "Sweep1", "status": "ok"}}
]
EOF
"$CLI" run "$WORK/kept.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli design"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Bodies shown: F2.b0" "the slanted block shown"

echo "--- Extrude1 edited to 12: along its direction still"
ui_mark
ui_step "edit Extrude1 (double-click)"     ui_double_click_logged "Timeline Extrude1"
ui_expect_new "Editing F2 with Extrude" "editing it"
ui_expect_new "Extrude: keeps its direction [0,3,4]" "its direction kept"
ui_step "distance 12"                      ui_type_in "Panel Extrude input distance" "12"
ui_expect_new "Preview Extrude: ok" "the edit previewed"
ui_focus_main
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F2" "edited"
ui_expect_volume "Body [^(]* (F2.b0): volume 3200.000 -> \([0-9.]*\) mm3" 3840 "12 along the direction"

echo "--- Sweep1 edited: its frame kept"
ui_mark
ui_step "edit Sweep1 (double-click)"       ui_double_click_logged "Timeline Sweep1"
ui_expect_new "Editing F5 with Sweep" "editing it"
ui_expect_new "Sweep: keeps {\"frame\":{" "its frame kept"
ui_step "flip on"                          ui_click_logged "Panel Sweep input flip"
ui_step "flip off"                         ui_click_logged "Panel Sweep input flip"
ui_expect_new "Preview Sweep: ok" "the edit previewed"
ui_focus_main
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F5" "edited"

ui_mark
ui_step "save (Ctrl+S)"                    ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
grep -q '"direction"' "$FILE" || ui_fail "the extrusion's direction is in the file"
echo "ok   the extrusion's direction saved"
grep -q '"constant_binormal"' "$FILE" || ui_fail "the sweep's frame is in the file"
echo "ok   the sweep's frame saved"

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_stop_app
ui_finish "UI kept options"
