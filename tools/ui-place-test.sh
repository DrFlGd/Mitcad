#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/PlaceInputs.cpp app/commands/PlaceGeometry.cpp app/commands/PrimitiveCommands.cpp
# check-all sources: app/commands/PulleyCommands.cpp app/commands/GearCommands.cpp app/framework/ManipulatorOverlay.cpp
# check-all sources: app/framework/CommandSession.cpp core/model/src/features/place_at.rs tools/cli
# Placement by picked geometry (mitcad#656) through the real UI, on a
# design mitcad-cli makes as earlier versions wrote them: a 60 x 40 x 20
# block (Sketch1 F1, Extrude1 F2), a cylinder on XY at (100, 0) by its plane
# and centre (Cylinder1 F3), a GT2 pulley on XY at (0, 80) (Pulley1 F4) and
# one beside it by `belt_with`, 80 mm along X (Pulley2 F5), a spur gear on
# XY at (0, -80) (Gear1 F6) and one meshing it at 90 degrees by
# `mesh_with.angle` (Gear2 F7). The Windows counterpart is
# tools/ui-windows-place-test.ps1 (ctest app.ui-windows-place):
#   1. Cylinder1 edited: it opens on XY, 100 mm from the Y axis and 0 from
#      the X axis, no typed coordinates; 25 high, OK: saved with its plane
#      and centre as before (the placement unchanged).
#   2. Pulley2 edited: placed where it is, 80 mm along X from Pulley1's
#      centre on XY, with Pulley1 as its Check Belt (200 mm belt, its rows
#      under it); OK keeps `belt_with`.
#   3. Gear2 edited: placed where its mesh had it (the centre distance 40
#      mm at 90 degrees: (0, -40) on XY), Mesh With Gear1; OK keeps
#      `mesh_with.angle`.
#   4. Box on the block's top face: the click measured from the two nearest
#      edges, its x along Reference 1; another top edge clicked becomes a
#      reference; the distances typed 10 and 12; More opened, Angle 10 in
#      the panel shows its value box in the view (hidden at 0), where it is
#      turned to 30 degrees. OK makes a 30 x 20 x 10 box (6000 mm3).
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-place-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-place.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

cat > "$WORK/earlier.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$RECT"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "cylinder", "plane": "xy", "center": [100, 0],
    "diameter": 10, "height": 20, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "pulley", "kind": "toothed", "profile": "gt2_2mm", "plane": "xy",
    "center": [0, 80], "teeth": 20, "belt_width": 6, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "pulley", "kind": "toothed", "profile": "gt2_2mm",
    "belt_with": {"pulley": "F4", "center_distance": 80, "angle": 0}, "teeth": 20, "belt_width": 6,
    "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "gear", "kind": "spur", "plane": "xy", "center": [0, -80],
    "module": 2, "teeth": 20, "face_width": 10, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "gear", "kind": "spur", "mesh_with": {"gear": "F6", "angle": 1.5707963267948966},
    "module": 2, "teeth": 20, "face_width": 10, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/earlier.json" --save "$WORK/earlier.mitcad" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$WORK/earlier.mitcad"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Bodies shown: F2.b0" "the design shown"

echo "--- 1. A cylinder written with its plane and centre"
ui_mark
ui_step "double-click Cylinder1"           ui_double_click_logged "Timeline Cylinder1"
ui_expect_new "Editing F3 with Cylinder" "Cylinder1 opens for editing" 30
ui_expect_new "Place Cylinder: face plane xy, 100 mm from axis y, 0 mm from axis x" "on XY from the origin axes"
grep -q "Panel Cylinder input x at\|Panel Cylinder input y at" <(tail -n +"$UI_MARK" "$UI_LOG") &&
  ui_fail "the panel still has typed coordinates"
ui_mark
ui_step "height 25"                        ui_type_in "Panel Cylinder input height" "25"
ui_expect_new "Preview Cylinder: ok" "25 high previewed" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F3" "Cylinder1 edited, its plane and centre kept" 30
ui_expect_volume "Body [^ ]* (F3\.b0): volume [0-9.]* -> \([0-9.]*\) mm3" \
  "$(awk 'BEGIN { printf "%.6f", 3.14159265358979 * 25 * 25 }')" "25 high, where it was"

echo "--- 2. A pulley written beside another (belt_with)"
ui_mark
ui_step "double-click Pulley2"             ui_double_click_logged "Timeline Pulley2"
ui_expect_new "Editing F5 with Pulley" "Pulley2 opens for editing" 30
ui_expect_new "Place Pulley: face plane xy, 80 mm from axis y, 80 mm from axis x" "where the belt had it"
ui_expect_new "Pulley Check Belt: 1 feature" "Pulley1 checks the belt"
ui_expect_new "Pulley belt check with F4: Belt length 200 mm (100 teeth)" "the belt round both" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F5" "Pulley2 saved as it was (belt_with)" 30

echo "--- 3. A gear written meshing another at an angle (mesh_with.angle)"
ui_mark
ui_step "double-click Gear2"               ui_double_click_logged "Timeline Gear2"
ui_expect_new "Editing F7 with Gear" "Gear2 opens for editing" 30
ui_expect_new "Place Gear: face plane xy, 0 mm from axis y, -40 mm from axis x" "where the mesh had it"
ui_expect_new "Gear Mesh With: 1 feature" "still meshing Gear1"
ui_expect_new "Preview Gear: ok" "previewed where it was" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F7" "Gear2 saved as it was (mesh_with.angle)" 30
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

echo "--- 4. A box on the block's top face, measured from its edges"
ui_mark
ui_step "Box"                              ui_command "Box"
ui_expect_new "Command Box started" "the panel opened" 30
ui_expect_new "Pick places:" "the picks are logged"
ui_mark
ui_step "click the top face"               ui_click_pick "face F2.b0/F2:end($RECT)"
ui_expect_new "Box Place On: 1 face" "the top face picked"
ui_expect_new "Place Box: face face F2:end(" "measured from two of its edges"
# The edges the face's place is measured from are the references' own
# until one is picked (mitcad#656): listed, not picked.
ui_expect_new "Box Reference 1 listed: Edge" "a reference edge"
ui_expect_new "Value box Box at_offset1 at " "Distance 1 a dimension in the view" 30
first=$(grep "^Place Box: face face F2:end(" "$UI_LOG" | tail -1 |
  sed -n 's/^Place Box: face face .* of F2\.b0, [0-9.-]* mm from \(edge .*\) of F2\.b0, [0-9.-]* mm from edge .* of F2\.b0 (point.*/\1/p')
echo "     reference 1: $first"
# A top edge that is neither reference: clicked, it takes the place of the
# one it runs with. The last one listed: the previewed box lies against the
# other, over the place logged for it before the box was shown.
other=$(grep "^Pick edge F2\.b0/E{" "$UI_LOG" | grep -F "F2:end(" | sed -n 's/^Pick edge F2\.b0\/\(E{[^ ]*}\) at .*/\1/p' |
  tac | while read -r edge; do grep -qF "$edge" <(grep "^Place Box: " "$UI_LOG" | tail -1) || { echo "$edge"; break; }; done)
[ -n "$other" ] || ui_fail "no other top edge to pick"
ui_mark
ui_step "click $other"                     ui_click_pick "edge F2.b0/$other at"
ui_expect_new "Place Box: edge $other of F2.b0 is a reference now" "the edge a reference now"
ui_expect_new "Box Place On: 1 face" "Place On keeps the face"
ui_mark
ui_step "distance 1: 10"                   ui_type_in "Panel Box input at_offset1" "10"
ui_step "distance 2: 12"                   ui_type_in "Panel Box input at_offset2" "12"
ui_expect_new "Preview Box: ok" "previewed at the distances" 30
grep -q "^Value box Box at_angle at " <(tail -n +"$UI_MARK" "$UI_LOG") && ui_fail "the angle's value box shown at 0"
ui_mark
ui_step "More"                             ui_click_logged "Panel Box input at_more"
ui_step "angle 10"                         ui_type_in "Panel Box input at_angle" "10"
ui_expect_new "Value box Box at_angle at " "the angle's value box at its ring, not 0" 30
ui_mark
ui_step "the angle's value box"            ui_click_logged "Value box Box at_angle"
ui_expect_new "Value box Box at_angle opened: 10" "a field over the view edits it"
ui_step "type 30"                          xdotool type --delay 40 "30"
ui_step "Enter keeps it"                   ui_key Return
ui_expect_new "Value box Box at_angle closed: 30" "turned 30 degrees in the view"
ui_expect_new "Preview Box: ok" "the turned box previewed" 30
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Added box (New Body)" "the box added" 30
ui_expect_volume "New body [^ ]* ([A-Z0-9]*\.b0): volume \([0-9.]*\) mm3" 6000 "30 x 20 x 10"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI place test"
