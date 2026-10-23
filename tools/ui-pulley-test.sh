#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/PulleyCommands.cpp app/framework/CommandSession.cpp
# check-all sources: core/model/src/features/pulley.rs core/model/src/features/pulley_profile.rs core/model/src/api/pulleys.rs
# check-all sources: app/commands/SeatInputs.cpp core/model/src/features/seat.rs core/model/src/api/seats.rs tools/cli
# Pulley (mitcad#562) through the real UI; the Windows counterpart is
# tools/ui-windows-pulley-test.ps1 (ctest app.ui-windows-pulley):
#   1. Pulley in an empty design: the panel starts on XY with a GT2 2 mm
#      pulley of 20 teeth; its note shows the pitch diameter (20 x 2 / pi =
#      12.7324 mm). In the preview: flanges 24 across, then 30 teeth (pitch
#      diameter 60 / pi = 19.0986 mm), then a flange on one side only. OK
#      makes Pulley1 (F1.b0).
#   2. Pulley1 edited: it opens with its flange on one side and its 30
#      teeth; 20 teeth again, OK.
#   3. Pulley again (20 teeth): Belt With a face of Pulley1, 80 mm apart:
#      the note shows the belt round both (two equal pulleys: 2 x 80 +
#      20 x 2 = 200 mm, 100 teeth); the catalogue lengths 188, 202 and 210
#      give the nearest one (202 mm). OK makes F2.b0.
#   4. On a shaft (mitcad#578), in a design mitcad-cli makes (an 8 mm
#      cylinder 30 high, Cylinder1, F1.b0): Pulley with Shaft the
#      cylinder's top edge: the note shows the bore the shaft gives (8 mm);
#      an offset of 2 mm and a bore clearance of 0.2 mm make it 8.2 mm; OK
#      makes the pulley on the shaft (F2.b0).
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-pulley-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-pulley.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

cat > "$WORK/shaft.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "cylinder", "plane": "xy", "center": [0, 0],
    "diameter": 8, "height": 30, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/shaft.json" --save "$WORK/shaft.mitcad" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app

echo "--- A GT2 pulley on XY, its teeth and flanges changed in the preview"
ui_mark
ui_step "Pulley"                           ui_command "Pulley"
ui_expect_new "Command Pulley started" "the panel opened" 30
ui_expect_new "Preview Pulley: ok" "the pulley previewed" 30
ui_expect_new "Panel Pulley note: Pitch diameter 12.7324 mm" "its pitch diameter shown"
ui_mark
ui_step "flange diameter 24"               ui_type_in "Panel Pulley input flange_diameter" "24"
ui_expect_new "Preview Pulley: ok" "the wider flanges previewed" 30
ui_mark
ui_step "teeth 30"                         ui_type_in "Panel Pulley input teeth" "30"
ui_expect_new "Panel Pulley note: Pitch diameter 19.0986 mm" "the pitch diameter of 30 teeth" 30
ui_mark
ui_step "flanges: One"                     ui_choose "Panel Pulley input flanges" -1
ui_expect_new "Preview Pulley: ok" "one flange previewed" 30
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added pulley (Toothed), GT2 2 mm, 30 teeth" "the pulley added" 30
ui_expect_new "(F1.b0): volume " "its body made" 30

echo "--- Pulley1 edited back to 20 teeth"
ui_mark
ui_step "double-click Pulley1"             ui_double_click_logged "Timeline Pulley1"
ui_expect_new "Editing F1 with Pulley" "Pulley1 opens for editing" 30
ui_expect_new "flanges=one" "with its flange on one side"
ui_expect_new "Panel Pulley note: Pitch diameter 19.0986 mm" "and its 30 teeth" 30
ui_mark
ui_step "teeth 20"                         ui_type_in "Panel Pulley input teeth" "20"
ui_expect_new "Panel Pulley note: Pitch diameter 12.7324 mm" "the edit previewed" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F1" "Pulley1 edited" 30

echo "--- A second pulley on the belt"
ui_step "fit (F6)"                         ui_key F6
ui_mark
ui_step "Pulley"                           ui_command "Pulley"
ui_expect_new "Preview Pulley: ok" "the panel open" 30
ui_mark
ui_step "Belt With"                        ui_click_logged "Panel Pulley input belt_with"
ui_expect_new "Pick places:" "the picks are logged"
ui_expect_new "Pick face F1.b0/F1:" "a face of Pulley1 can be picked"
face=$(sed -n 's/^Pick face F1\.b0\/\(F1:[^ ]*\) at [0-9]*,[0-9]*$/\1/p' "$UI_LOG" | tail -1)
[ -n "$face" ] || ui_fail "no face of Pulley1 to pick"
ui_mark
ui_step "click $face"                      ui_click_pick "face F1.b0/$face at"
ui_expect_new "Preview Pulley: ok" "previewed beside Pulley1" 30
ui_mark
ui_step "centre distance 80"               ui_type_in "Panel Pulley input center_distance" "80"
ui_expect_new "belt length 200 mm (100 teeth)" "the belt length logged" 30
ui_mark
ui_step "catalogue lengths"                ui_type_in "Panel Pulley input belt_lengths" "188, 202, 210"
ui_expect_new "nearest catalogue length 202 mm" "the nearest catalogue length logged" 30
grep "^Panel Pulley note: " "$UI_LOG" | tail -1 | sed 's/^/     /'
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added pulley (Toothed), GT2 2 mm, 20 teeth" "the second pulley added" 30
ui_expect_new "(F2.b0): volume " "its body made" 30
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

echo "--- A pulley on a shaft (mitcad#578)"
ui_stop_app
ui_start_app --open "$WORK/shaft.mitcad"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Bodies shown: F1.b0" "the shaft shown"
ui_mark
ui_step "Pulley"                           ui_command "Pulley"
ui_expect_new "Preview Pulley: ok" "the panel open on XY" 30
ui_step "teeth 20"                         ui_type_in "Panel Pulley input teeth" "20"
ui_mark
ui_step "Shaft"                            ui_click_logged "Panel Pulley input shaft"
ui_expect_new "Pick places:" "the picks are logged"
ui_expect_new "Pick edge F1.b0/E{F1:side0|F1:top} at" "the shaft's top edge can be picked"
ui_mark
ui_step "click the shaft's top edge"       ui_click_pick "edge F1.b0/E{F1:side0|F1:top} at"
ui_expect_new "bore 8 mm (shaft 8 mm)" "the bore the shaft gives" 30
ui_mark
ui_step "offset 2"                         ui_type_in "Panel Pulley input seat_offset" "2"
ui_expect_new "Preview Pulley: ok" "2 mm down the shaft previewed" 30
ui_mark
ui_step "bore clearance 0.2"               ui_type_in "Panel Pulley input bore_clearance" "0.2"
ui_expect_new "bore 8.2 mm (shaft 8 mm)" "the clearance widens the bore" 30
grep "^Panel Pulley note: " "$UI_LOG" | tail -1 | sed 's/^/     /'
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added pulley (Toothed), GT2 2 mm, 20 teeth" "the pulley on the shaft added" 30
ui_expect_new "(F2.b0): volume " "its body made" 30
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI pulley test"
