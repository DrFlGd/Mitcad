#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/BearingCommands.cpp app/framework/CommandSession.cpp
# check-all sources: core/model/src/features/printable_bearing.rs core/model/src/features/bearing_sizes.rs
# check-all sources: core/model-api/src/bearings.rs tools/cli
# check-all sources: app/commands/PlaceInputs.cpp core/model/src/features/seat.rs core/model-api/src/seats.rs
# Bearing (mitcad#577) through the real UI, on a design mitcad-cli makes: a
# shaft 10 mm across and 20 long at (60, 0) (Extrude1, F2.b0, its round
# face F2:side(c1)), an 8 mm cylinder 30 high at (0, 60) (Cylinder1, F3.b0,
# its top edge E{F3:side0|F3:top}) and a block 40 x 40 x 10 at (0, 120)
# with a 22 mm hole through it (Extrude2 F5.b0, the hole's wall
# F7:side(c1)). The Windows counterpart is tools/ui-windows-bearing-test.ps1
# (ctest app.ui-windows-bearing):
#   1. Bearing, placed by a click on the XY plane and both distances 0
#      (it waits for a pick, mitcad#656): a 608 of dumbbells; its note
#      shows the model's values (15 mm pitch diameter, 10 elements of 3.8
#      mm, walls 1.2 mm, the flanks' gap 0.4 / sqrt(2)). 12 elements are
#      too many (the note says so), 8 are fine; as a New Body, OK makes
#      Bearing1 (F8.b0 ...).
#   2. Bearing1 edited: it opens with its 8 elements; 10 again, OK.
#   3. Bearing again, Groove Into the shaft's round face: the 608's outer
#      race on the shaft, the raceway cut into it (7 dumbbells of 4.9333 mm
#      on a 13.2667 mm pitch circle, the outer race's wall 1.5 mm). OK.
#   4. On a shaft's end (mitcad#578): Place On the cylinder's top edge takes
#      the 608 (its bore fits the 8 mm shaft), 5 mm down the shaft. OK.
#   5. In a hole: Place On the hole's top rim takes the 608 by its outside
#      (22 mm), flush with the block's top face. OK.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-bearing-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-bearing.XXXXXX)
FILE=$WORK/bearing.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [60, 0], "diameter": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "cylinder", "plane": "xy", "center": [0, 60],
    "diameter": 8, "height": 30, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F4", "corner": [-20, 100], "width": 40, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F4", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F6", "center": [0, 120], "diameter": 22},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F6", "region": "r{c1}"}],
    "extent": {"type": "through_all"}, "operation": "cut", "participants": ["F5.b0"]}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Bodies shown: F2.b0" "the shaft shown"


# answer_hybrid: after OK of a bearing made as a new component, the
# question that the root already has modelled bodies (the shaft, the block
# and the grooved bearing's bodies) answered "Continue as Is" (Alt+O), when
# it is asked; the bearing is added then.
answer_hybrid() {
  local i
  for i in $(seq 50); do
    if tail -n +"$((UI_MARK + 1))" "$UI_LOG" | grep -q '^Hybrid assembly question:'; then
      ui_focus_dialog '^Hybrid Assembly$'
      ui_step "continue as is (Alt+O)"   ui_key alt+o
      ui_focus_main
      return
    fi
    tail -n +"$((UI_MARK + 1))" "$UI_LOG" | grep -q '^Added bearing' && return
    sleep 0.2
  done
}
echo "--- 1. A 608 of dumbbells on XY"
ui_mark
ui_step "Bearing"                          ui_command "Bearing"
ui_expect_new "Command Bearing started" "the panel opened" 30
ui_place_on_xy Bearing
ui_expect_new "Panel Bearing note: Pitch diameter 15 mm, 10 elements of 3.8 mm, walls 1.2 mm and 1.2 mm, least gap 0.2828 mm" \
  "its values shown" 30
ui_mark
ui_step "elements 12"                      ui_type_in "Panel Bearing input element_count" "12"
ui_expect_new "Panel Bearing error: too many elements for the pitch circle: at most 10" "too many elements refused" 30
ui_mark
ui_step "elements 8"                       ui_type_in "Panel Bearing input element_count" "8"
ui_expect_new "Panel Bearing note: Pitch diameter 15 mm, 8 elements of 3.8 mm" "8 elements" 30
ui_mark
ui_step "operation: New Body"              ui_choose "Panel Bearing input operation" 1
ui_expect_new "Preview Bearing: ok" "previewed as new bodies" 30
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added bearing (Dumbbell), 608" "the bearing added" 30
ui_expect_new "(F8.b0): volume " "its inner race made" 30

echo "--- 2. Bearing1 edited back to 10 elements"
ui_mark
ui_step "double-click Bearing1"            ui_double_click_logged "Timeline Bearing1"
ui_expect_new "Editing F8 with Bearing" "Bearing1 opens for editing" 30
ui_expect_new "Panel Bearing note: Pitch diameter 15 mm, 8 elements of 3.8 mm" "with its 8 elements" 30
ui_mark
ui_step "elements 10"                      ui_type_in "Panel Bearing input element_count" "10"
ui_expect_new "Panel Bearing note: Pitch diameter 15 mm, 10 elements of 3.8 mm" "the edit previewed" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F8" "Bearing1 edited" 30

echo "--- 3. A raceway cut into the shaft"
ui_step "fit (F6)"                         ui_key F6
ui_mark
ui_step "Bearing"                          ui_command "Bearing"
ui_expect_new "Command Bearing started" "the panel opened, waiting for a pick" 30
ui_mark
ui_step "Groove Into"                      ui_click_logged "Panel Bearing input groove"
ui_expect_new "Pick places:" "the picks are logged"
ui_expect_new "Pick face F2.b0/F2:side(c1) at" "the shaft's round face can be picked"
ui_mark
ui_step "click the shaft"                  ui_click_pick "face F2.b0/F2:side(c1) at"
ui_expect_new "Preview Bearing: ok" "the raceway previewed" 30
ui_expect_new "Panel Bearing note: Pitch diameter 13.2667 mm, 7 elements of 4.9333 mm, walls 1.5 mm, least gap 0.2828 mm" \
  "the values of the bearing on the shaft" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added bearing (Dumbbell), 608, groove into a face" "the raceway cut" 30
ui_expect_new "(F9.b0): volume " "the outer race made" 30

echo "--- 4. On the cylinder's top end, 5 mm down (mitcad#578)"
ui_step "fit (F6)"                         ui_key F6
ui_mark
ui_step "Bearing"                          ui_command "Bearing"
ui_expect_new "Command Bearing started" "the panel opened, waiting for a pick" 30
ui_mark
ui_step "Place On"                         ui_click_logged "Panel Bearing input at"
ui_expect_new "Pick places:" "the picks are logged"
ui_expect_new "Pick edge F3.b0/E{F3:side0|F3:top} at" "the cylinder's top edge can be picked"
ui_mark
ui_step "click the top edge"               ui_click_pick "edge F3.b0/E{F3:side0|F3:top} at"
ui_expect_new "Preview Bearing: ok" "the bearing previewed on the shaft" 30
ui_expect_new "on a 8 mm shaft" "the size whose bore fits the 8 mm shaft" 30
ui_mark
ui_step "More"                             ui_click_logged "Panel Bearing input at_more"
ui_step "offset 5"                         ui_type_in "Panel Bearing input at_offset" "5"
ui_expect_new "Preview Bearing: ok" "5 mm down the shaft previewed" 30
grep "^Panel Bearing note: " "$UI_LOG" | tail -1 | sed 's/^/     /'
ui_mark
ui_step "OK (Enter)"                       ui_key Return
answer_hybrid
ui_expect_new "Added bearing (Dumbbell), 608, on a shaft or in a hole" "the bearing on the shaft added" 30

echo "--- 5. In the hole, flush with the block's top face"
ui_step "fit (F6)"                         ui_key F6
ui_mark
ui_step "Bearing"                          ui_command "Bearing"
ui_expect_new "Command Bearing started" "the panel opened, waiting for a pick" 30
ui_mark
ui_step "Place On"                         ui_click_logged "Panel Bearing input at"
ui_expect_new "Pick places:" "the picks are logged"
ui_expect_new "Pick edge F5.b0/E{" "the block's edges can be picked"
rim=$(sed -n 's/^Pick edge F5\.b0\/\(E{[^ ]*F7:side(c1)[^ ]*}\) at [0-9]*,[0-9]*$/\1/p' "$UI_LOG" | grep -F "F5:end(" | tail -1)
[ -n "$rim" ] || ui_fail "no top rim of the hole to pick"
ui_mark
ui_step "click the hole's top rim"         ui_click_pick "edge F5.b0/$rim at"
ui_expect_new "Preview Bearing: ok" "the bearing previewed in the hole" 30
ui_expect_new "in a 22 mm hole" "the size whose outside fits the 22 mm hole" 30
grep "^Panel Bearing note: " "$UI_LOG" | tail -1 | sed 's/^/     /'
ui_mark
ui_step "OK (Enter)"                       ui_key Return
answer_hybrid
ui_expect_new "Added bearing (Dumbbell), 608, on a shaft or in a hole" "the bearing in the hole added" 30
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI bearing test"
