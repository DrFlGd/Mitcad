#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands
# Blend Surface, Boundary Surface, Match Surface and Fair Surface of the
# SURFACE group (mitcad#455) through the real UI (commands.md, "Surface
# modelling"):
#   - Blend Surface: from a box's top edge along y = 0 to the bottom edge
#     there of a box 10 above, through the edges: a new surface body.
#   - Boundary Surface: a 10 x 10 square of sketch lines filled (one line,
#     Chain Selection); two parallel lines spanned as a network.
#   - Match Surface: a surface wall's face matched to a box's bottom edge
#     above it.
#   - Fair Surface: the filled square faired.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-surface-fill-test.sh
# check-all sources: tools/cli core/model

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-surface-fill.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# F1: a 20 x 10 x 5 box at the origin; F2: a plane 15 up and F3 the same
# box on it; F4 (Sketch1): a 10 x 10 square at (40, 0) (c1-c4); F5
# (Sketch2): the lines (60, 0)-(70, 0) (c1) and (60, 10)-(70, 10) (c4);
# F6 (Sketch3): the line (80, 0)-(90, 0) (c1); F7: it extruded 10 as a
# surface (F7:side(c1)); F8: a plane 12 up and F9 a 20 x 5 x 3 box on it
# at (80, -5): its front bottom edge runs 12 up along y = -5. A sketch a feature
# uses is hidden, so each step has a sketch of its own.
cat > "$WORK/fills.json" << EOF
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0], "length": 20, "width": 10,
   "height": 5, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "construction_plane", "definition": {"type": "offset", "plane": "xy", "distance": 15}}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "F2", "corner": [0, 0], "length": 20, "width": 10,
   "height": 5, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F4", "corner": [40, 0], "width": 10, "height": 10},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_line", "sketch": "F5", "start": [60, 0], "end": [70, 0]},
  {"cmd": "sketch.add_line", "sketch": "F5", "start": [60, 10], "end": [70, 10]},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_line", "sketch": "F6", "start": [80, 0], "end": [90, 0]},
  {"cmd": "add_feature", "def": {"type": "extrude", "surface": true, "operation": "new_body",
   "curves": {"sketch": "F6", "curves": ["c1"]}, "extent": {"type": "distance", "distance": 10}}},
  {"cmd": "add_feature", "def": {"type": "construction_plane", "definition": {"type": "offset", "plane": "xy", "distance": 12}}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "F8", "corner": [80, -5], "length": 20, "width": 5,
   "height": 3, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/fills.json" --save "$WORK/fills.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli fills"; }

# The name of the last logged edge pick whose name holds both faces.
edge_of() {
  grep -F "Pick edge" "$UI_LOG" | grep -F -- "$1" | grep -F -- "$2" | tail -1 |
    sed -n 's/^Pick edge [^/]*\/\(.*\) at [0-9]*,[0-9]*$/\1/p'
}

ui_start_display
ui_start_app --open "$WORK/fills.mitcad"
ui_step "fit (F6)"                         ui_key F6

echo "--- Blend Surface: between two boxes' edges along y = 0"
ui_step "blend surface (search)"           ui_command "Blend Surface"
low=$(edge_of "F1:side0" "F1:top")
[ -n "$low" ] || ui_fail "no pick place logged for the low box's top edge"
ui_step "the low box's top edge"           ui_click_pick "$low"
ui_step "through the edge"                 ui_choose "Panel Blend Surface input from_continuity" -1
ui_step "the second edges input"           ui_click_logged "Panel Blend Surface input to_edges"
high=$(edge_of "F3:side0" "F3:bottom")
[ -n "$high" ] || ui_fail "no pick place logged for the high box's bottom edge"
ui_step "the high box's bottom edge"       ui_click_pick "$high"
ui_step "through that edge too"            ui_choose "Panel Blend Surface input to_continuity" -1
ui_expect_log "Preview Blend Surface: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added blend surface (g0, g0)" "blend surface added"
ui_expect_log "(F10.b0): volume 0.000 mm3" "the blend, a surface"

echo "--- Boundary Surface: a loop and a network"
ui_step "boundary surface (search)"        ui_command "Boundary Surface"
ui_step "the boundary curves input"        ui_click_logged "Panel Boundary Surface input curves"
ui_step "a side of the square (chained)"   ui_click_logged "Sketch entity F4/c1"
ui_expect_log "Preview Boundary Surface: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added boundary surface filling 1 curves and the open edges of 0 faces" "the square filled"
ui_expect_log "(F11.b0): volume 0.000 mm3" "the filled square, a surface"
ui_step "boundary surface again"           ui_command "Boundary Surface"
ui_step "network"                          ui_choose "Panel Boundary Surface input mode" 1
ui_step "the curves one way"               ui_click_logged "Panel Boundary Surface input u"
ui_step "the first line"                   ui_click_logged "Sketch entity F5/c1"
ui_step "the second line"                  ui_click_logged "Sketch entity F5/c4"
ui_expect_log "Preview Boundary Surface: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added boundary surface through 2 by 0 curves" "the lines spanned"
ui_expect_log "(F12.b0): volume 0.000 mm3" "the spanned surface"

echo "--- Match Surface: the wall's top onto the shelf's bottom edge"
ui_step "match surface (search)"           ui_command "Match Surface"
ui_step "the wall"                         ui_click_pick "face F7.b0/F7:side("
ui_step "the target edges input"           ui_click_logged "Panel Match Surface input target_edges"
shelf=$(edge_of "F9:side0" "F9:bottom")
[ -n "$shelf" ] || ui_fail "no pick place logged for the shelf's bottom edge"
ui_step "the shelf's bottom edge"          ui_click_pick "$shelf"
ui_step "through the edge"                 ui_choose "Panel Match Surface input continuity" -2
ui_expect_log "Preview Match Surface: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added match surface (g0)" "match surface added"

echo "--- Fair Surface: the filled square"
square=$(sed -n 's/^New body \(.*\) (F11\.b0): volume.*/\1/p' "$UI_LOG" | tail -1)
[ -n "$square" ] || ui_fail "the filled square has no name in the log"
ui_step "fair surface (search)"            ui_command "Fair Surface"
ui_step "the square in the browser"        ui_click_logged "Browser Root/Bodies/$square"
ui_step "tolerance 0.2"                    ui_type_in "Panel Fair Surface input tolerance" "0.2"
ui_expect_log "Preview Fair Surface: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added fair surface of a body" "fair surface added"

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_stop_app
ui_finish "surface fills"
