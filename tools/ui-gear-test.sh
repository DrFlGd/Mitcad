#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/GearCommands.cpp app/framework/CommandSession.cpp
# check-all sources: core/model/src/features/gear.rs core/model/src/features/gear_profile.rs geometry/src/gear.cpp
# check-all sources: app/commands/SeatInputs.cpp core/model/src/features/seat.rs core/model/src/api/seats.rs tools/cli
# check-all sources: core/model/src/print_fits.rs core/model/src/api/gears.rs
# Gear (mitcad#472) through the real UI; the Windows counterpart is
# tools/ui-windows-gear-test.ps1 (ctest app.ui-windows-gear). The Backlash
# and the Bore Clearance start as the printed-fit parameters gear_backlash
# and shaft_bore_clearance (0.2 mm each, mitcad#623):
#   1. Gear in an empty design: the panel starts on XY with a spur gear of
#      20 teeth, module 2, 10 wide; its note shows the derived diameters
#      (pitch 40, outside 44, root 35). OK makes Gear1 (F1.b0), its teeth
#      thinner by the backlash, and creates gear_backlash.
#   2. Gear again: Rack picked by its icon, Mesh With a face of Gear1, 10
#      teeth, 12 high: the note shows its pitch line (20 pi = 62.8319 mm) and
#      the centre distance (20 mm); OK makes the rack below the gear
#      (F2.b0).
#   3. On a shaft (mitcad#578), in a design mitcad-cli makes (an 8 mm
#      cylinder 30 high, Cylinder1, F1.b0): Gear (Spur, 20 teeth) with
#      Shaft the cylinder's top edge: the note shows the bore the shaft
#      gives with the printed fit (8.2 mm); an offset of 5 mm; OK makes the
#      gear on the shaft (F2.b0: Gear1's volume less the 8.2 mm bore
#      through its 10 mm) and creates shaft_bore_clearance.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-gear-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-gear.XXXXXX)
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

echo "--- 1. A spur gear on XY"
ui_mark
ui_step "Gear"                             ui_command "Gear"
ui_expect_new "Preview Gear: ok" "the spur gear previewed" 30
ui_expect_new "Gear Backlash: gear_backlash = 0.2 mm" "the backlash is the printed-fit parameter"
ui_expect_new "Panel Gear note: Pitch diameter 40 mm, outside 44 mm, root 35 mm" "the derived diameters shown"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added gear (Spur), 20 teeth, module 2" "the gear added" 30
ui_expect_new "Created user parameter gear_backlash = 0.2 mm" "gear_backlash created with the gear" 30
ui_expect_volume 'New body [^ ]* (F1\.b0): volume \([0-9.]*\) mm3' 12283.094 "Gear1" 1e-5

echo "--- 2. A rack meshing it"
ui_mark
ui_step "Gear"                             ui_command "Gear"
ui_expect_new "Preview Gear: ok" "the panel open" 30
ui_step "Rack"                             ui_click_logged "Panel Gear input kind=rack"
ui_step "teeth 10"                         ui_type_in "Panel Gear input teeth" "10"
ui_step "height 12"                        ui_type_in "Panel Gear input height" "12"
ui_mark
ui_step "Mesh With"                        ui_click_logged "Panel Gear input mesh_with"
ui_expect_new "Pick places:" "the picks are logged"
ui_expect_new "Pick face F1.b0/F1:" "a face of Gear1 can be picked"
face=$(sed -n 's/^Pick face F1\.b0\/\(F1:[a-z_]*([0-9]*)\) at [0-9]*,[0-9]*$/\1/p' "$UI_LOG" | tail -1)
[ -n "$face" ] || ui_fail "no face of Gear1 to pick"
ui_mark
ui_step "click $face"                      ui_click_pick "face F1.b0/$face at"
ui_expect_new "Panel Gear note: Pitch line 62.8319 mm, centre distance 20 mm" \
  "the rack's pitch line and centre distance shown" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added gear (Rack), 10 teeth, module 2" "the rack added" 30
ui_expect_volume 'New body [^ ]* (F2\.b0): volume \([0-9.]*\) mm3' 6173.320 "the rack" 1e-5
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

echo "--- 3. A spur gear on a shaft (mitcad#578)"
ui_stop_app
ui_start_app --open "$WORK/shaft.mitcad"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Bodies shown: F1.b0" "the shaft shown"
ui_mark
ui_step "Gear"                             ui_command "Gear"
ui_expect_new "Preview Gear: ok" "the panel open on XY" 30
ui_expect_new "Gear Bore Clearance: shaft_bore_clearance = 0.2 mm" "the bore clearance is the printed-fit parameter"
ui_step "teeth 20"                         ui_type_in "Panel Gear input teeth" "20"
ui_mark
ui_step "Shaft"                            ui_click_logged "Panel Gear input shaft"
ui_expect_new "Pick places:" "the picks are logged"
ui_expect_new "Pick edge F1.b0/E{F1:side0|F1:top} at" "the shaft's top edge can be picked"
ui_mark
ui_step "click the shaft's top edge"       ui_click_pick "edge F1.b0/E{F1:side0|F1:top} at"
ui_expect_new "bore 8.2 mm (shaft 8 mm)" "the bore the shaft gives, with the printed fit" 30
ui_mark
ui_step "offset 5"                         ui_type_in "Panel Gear input seat_offset" "5"
ui_expect_new "Preview Gear: ok" "5 mm down the shaft previewed" 30
grep "^Panel Gear note: " "$UI_LOG" | tail -1 | sed 's/^/     /'
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added gear (Spur), 20 teeth, module 2" "the gear on the shaft added" 30
ui_expect_new "Created user parameter shaft_bore_clearance = 0.2 mm" "shaft_bore_clearance created with the gear" 30
ui_expect_volume 'New body [^ ]* (F2\.b0): volume \([0-9.]*\) mm3' 11754.992 "the gear with the shaft's bore" 1e-5
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI gear test"
