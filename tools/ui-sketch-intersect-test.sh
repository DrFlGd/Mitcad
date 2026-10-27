#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/SketchCommands.cpp app/framework/CommandSession.cpp app/sketch
# check-all sources: core/sketch core/model-api/src/sketch.rs core/model/src/document.rs tools/cli
# Sketch Intersect (mitcad#736) through the real UI:
#   - A 40 x 25 block 20 high, from (-20, -10), and a sketch on XZ, which
#     cuts it through (mitcad-cli builds them).
#   - Sketch2 edited; Intersect (the command search) takes Body1 picked in
#     the browser: the preview shows the section, OK keeps it, linked
#     (Projection Link is on by default).
#   - Extrude1 edited to 30: the section follows the block. Sketch2 has
#     no warning about it, its corner is at the new top (a drag there finds
#     the fixed, projected point), and nothing is left at the old top.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-sketch-intersect-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-sketch-intersect.XXXXXX)
FILE=$WORK/intersect.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# Sketch1 (F1): the rectangle; Extrude1 (F2): Body1 (F2.b0), z from 0 to
# 20; Sketch2 (F3) on XZ: x along X, y along -Z.
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-20, -10], "width": 40, "height": 25},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "sketch.create", "plane": "xz"}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# no_new "text" description: no log line with the text since the mark.
no_new() {
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qF -- "$1" && ui_fail "$2"
  echo "ok   $2"
}

# edit_sketch2: Sketch2 edited from the timeline.
edit_sketch2() {
  ui_mark
  ui_step "edit Sketch2 (double-click)"  ui_double_click_logged "Timeline Sketch2"
  ui_expect_new "Editing sketch F3 (Sketch2)" "Sketch2 edited"
  ui_step "fit (F6)"                     ui_key F6
  ui_sync
}

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Bodies shown: F2.b0" "the block shown"

echo "--- Intersect: the section of Body1"
edit_sketch2
ui_mark
ui_step "intersect (search)"             ui_command "Intersect"
ui_expect_new "Command search chose sketch.intersect" "Intersect started"
ui_sync
ui_expect_log "Browser Root/Bodies/Body1 at" "Body1 in the browser"
ui_step "pick Body1 in the browser"      ui_click_logged "Browser Root/Bodies/Body1"
ui_expect_new "Intersect Geometry: 1 body" "the body went to Intersect"
ui_expect_new "Preview Intersect: ok" "the section previewed"
ui_wait_idle
ui_focus_main
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Intersected 1 item(s)" "the section kept"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "Sketch2 finished"

echo "--- The block grows to 30: the section follows"
ui_mark
ui_step "edit Extrude1 (double-click)"   ui_double_click_logged "Timeline Extrude1"
ui_expect_new " with Extrude" "editing Extrude1"
ui_step "distance 30"                    ui_type_in "Panel Extrude input distance" "30"
ui_expect_new "Preview Extrude: ok" "the edit previewed"
ui_wait_idle
ui_focus_main
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F2" "Extrude1 edited"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
echo "ok   the design recomputes"

edit_sketch2
no_new "Palette warning" "Sketch2 has no warning: the section followed"
# The section's top right corner is at x = 20, z = 30 now: (20, -30).
ui_mark
ui_step "drag the section's new corner"  ui_sketch_drag 20 -30 26 -36
ui_sync
tail -n +$((UI_MARK + 1)) "$UI_LOG" |
  grep -qE "Drag of p[0-9]+ did not move it|Dragged p[0-9]+ to \(20\.000, -30\.000\)" ||
  ui_fail "no section point at the block's new top corner"
echo "ok   the section's corner is where the block is now"
# The old top line ran along z = 20: nothing of the section is there now.
ui_mark
ui_step "drag at the old top"            ui_sketch_drag 0 -20 6 -26
ui_sync
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qE "Drag of |Dragged " &&
  ui_fail "a section curve stayed at the old top"
echo "ok   nothing of the section is left at the old top"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "Sketch2 finished"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI sketch intersect test"
