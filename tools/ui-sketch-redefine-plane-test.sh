#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/SketchPlaneCommands.cpp app/browser app/MainWindow.cpp app/MainWindowContextMenu.cpp
# check-all sources: app/framework/CommandSession.cpp tools/cli
# check-all sources: core/model/src/document/sketch_plane.rs core/sketch/src/placement.rs core/model-api/src/sketch_plane.rs
# Redefine Sketch Plane (mitcad#787) through the real UI, on a design that
# mitcad-cli makes: Sketch1 (F1) 30 x 20 on XY and Extrude1 (F2) 10 up,
# Body1 (F2.b0); Sketch2 (F3) 10 x 6 at (5, 5) on XY and Extrude2 (F4) 3
# up, Body2 (F4.b0).
#   - The browser: Redefine Sketch Plane on Sketch2's row starts with
#     Sketch2; Body1's top face picked and an offset of 2 typed, OK moves
#     the sketch onto the face, 12 above XY (and Extrude2 with it); one
#     undo takes it back.
#   - A face of Extrude1 for Sketch1 fails the preview with the reason:
#     Extrude1 comes after Sketch1 in the timeline.
#   - In Sketch2 being edited, Redefine Sketch Plane from the sketch's
#     context menu (its Sketch input hidden: the edited sketch is moved)
#     turns it a quarter on its plane.
# The Windows test is tools/ui-windows-sketch-redefine-plane-test.ps1.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-sketch-redefine-plane-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CLI=${UI_CLI:-$(dirname "$UI_APP")/../tools/cli/mitcad-cli}
mkdir -p "$ROOT/build"
WORK=$(mktemp -d "$ROOT/build/ui-sketch-redefine-plane.XXXXXX")
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
FILE=$WORK/design.mitcad

R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [5, 5], "width": 10, "height": 6},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "$R"}],
    "extent": {"type": "distance", "distance": 3}, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# The entries of the last "Context menu: ..." line since the mark, " | "
# around each.
menu_since() {
  printf '| %s |' "$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -F "Context menu: " | tail -1 |
    sed "s/.*Context menu: //" | sed "s/…/.../g")"
}
has() {
  [[ $(menu_since) == *"| $1 |"* ]] || ui_fail "'$1' is not in the context menu: $(menu_since)"
  echo "ok   the context menu has $1"
}

# within "x, y, z" "ex ey ez" tolerance description: each coordinate within
# the tolerance of the expected one.
within() {
  awk -v got="$1" -v want="$2" -v tol="$3" 'BEGIN {
    if (split(got, g, /, */) != 3 || split(want, w, / +/) != 3) exit 1
    for (i = 1; i <= 3; i++) { d = g[i] - w[i]; if (d < 0) d = -d; if (d > tol) exit 1 }
  }' || ui_fail "$4: ($1), expected ($2) within $3"
  echo "ok   $4: ($1)"
}

# frame_of origin|x|y: that part of the frame the last "Redefined the plane
# of" line logged, "x, y, z".
frame_of() {
  local line
  line=$(grep -F "Redefined the plane of" "$UI_LOG" | tail -1)
  case $1 in
    origin) sed -n 's/.*frame origin (\([^)]*\)), x .*/\1/p' <<< "$line" ;;
    x) sed -n 's/.*frame origin ([^)]*), x (\([^)]*\)), y .*/\1/p' <<< "$line" ;;
    y) sed -n 's/.*frame origin ([^)]*), x ([^)]*), y (\([^)]*\)).*/\1/p' <<< "$line" ;;
  esac
}

ui_start_display
ui_start_app --open "$FILE"
# Seen from the top: the origin planes the Plane input shows stand in front
# of Body1's top face in the isometric view (XZ at y = 0 between it and the
# eye) and are the picks there; from the top, XZ and YZ are edge-on and XY
# lies behind the face.
ui_step "top view (search)"              ui_command "Top View"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Browser Root/Sketches/Sketch2 at" "the browser lists Sketch2"
ui_expect_log "Bodies shown: F2.b0, F4.b0" "both bodies shown"

echo "--- The browser: Sketch2 onto Body1's top face, 2 above it"
ui_mark
ui_step "right-click Sketch2's row"      ui_click_logged "Browser Root/Sketches/Sketch2" 3
ui_expect_new "Context menu: " "the row's menu"
has "Edit Sketch"
has "Redefine Sketch Plane"
ui_step "choose Redefine Sketch Plane"   ui_menu_choose "Redefine Sketch Plane"
ui_expect_new "Command Redefine Sketch Plane started" "the panel started"
ui_expect_new "Redefine Sketch Plane Sketch: 1 sketch [sketch F3]" "with Sketch2"
ui_expect_new "Panel Redefine Sketch Plane input plane at" "the panel is laid out"
ui_mark
ui_step "activate Plane"                 ui_click_logged "Panel Redefine Sketch Plane input plane"
ui_expect_new "Pick face F2.b0/F2:end(" "Body1's top face can be picked"
ui_step "pick Body1's top face"          ui_click_pick "face F2.b0/F2:end("
ui_expect_new "Redefine Sketch Plane Plane: 1 face [face F2:end(" "the top face is the plane"
ui_step "offset 2 mm"                    ui_type_in "Panel Redefine Sketch Plane input offset" "2"
ui_expect_new "Preview Redefine Sketch Plane: ok" "previewed"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Redefined the plane of Sketch2: face F2:end(" "Sketch2 is on the face"
within "$(frame_of origin)" "0 0 12" 1e-6 "the frame's origin 2 above the face"
within "$(frame_of x)" "1 0 0" 1e-6 "its x axis"
within "$(frame_of y)" "0 1 0" 1e-6 "its y axis"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_mark
ui_step "undo"                           ui_key ctrl+z
ui_expect_new "Undo: " "one undo"
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -F "Undo: " | tail -1 | grep -qF "Redefine Plane of Sketch2" ||
  ui_fail "the undo is not the redefine: $(grep -F "Undo: " "$UI_LOG" | tail -1)"
echo "ok   the undo takes the redefine back"
ui_step "Escape (clear the selection)"   ui_key Escape

echo "--- A face of a later feature is refused with the reason"
ui_mark
ui_step "right-click Sketch1's row"      ui_click_logged "Browser Root/Sketches/Sketch1" 3
ui_expect_new "Context menu: " "the row's menu"
ui_step "choose Redefine Sketch Plane"   ui_menu_choose "Redefine Sketch Plane"
ui_expect_new "Redefine Sketch Plane Sketch: 1 sketch [sketch F1]" "with Sketch1"
ui_expect_new "Panel Redefine Sketch Plane input plane at" "the panel is laid out"
ui_mark
ui_step "activate Plane"                 ui_click_logged "Panel Redefine Sketch Plane input plane"
ui_expect_new "Pick face F2.b0/F2:end(" "Body1's top face can be picked"
ui_step "pick Body1's top face"          ui_click_pick "face F2.b0/F2:end("
ui_expect_new "Preview Redefine Sketch Plane: failed: Sketch1 cannot lie on a plane or face of Extrude1" \
  "the preview fails"
ui_expect_new "Extrude1 comes after Sketch1 in the timeline" "with the reason"
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command Redefine Sketch Plane cancelled" "cancelled"
ui_step "Escape (clear the selection)"   ui_key Escape

echo "--- In the sketch: its context menu turns Sketch2 a quarter"
ui_mark
ui_double_click_expect "Timeline Sketch2" "Editing sketch F3 (Sketch2)" "Sketch2 is edited"
ui_expect_new "Sketch view " "the sketch view is logged"
ui_sync
ui_mark
# The middle of the rectangle's bottom line, (10, 5) in the sketch.
ui_step "right-click Sketch2's bottom line" xdotool mousemove $(ui_sketch_at 10 5) click 3
ui_expect_new "Context menu: " "the sketch's menu"
has "Redefine Sketch Plane"
ui_step "choose Redefine Sketch Plane"   ui_menu_choose "Redefine Sketch Plane"
ui_expect_new "Command Redefine Sketch Plane started" "the panel started"
ui_expect_new "Panel Redefine Sketch Plane input sketch at" "the panel is laid out"
grep -F "Panel Redefine Sketch Plane input sketch at" "$UI_LOG" | tail -1 | grep -qF "(hidden)" ||
  ui_fail "the Sketch input is shown in the edited sketch"
echo "ok   the Sketch input is hidden: the edited sketch is moved"
ui_step "angle 90 deg"                   ui_type_in "Panel Redefine Sketch Plane input angle" "90"
ui_expect_new "Preview Redefine Sketch Plane: ok" "previewed"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Redefined the plane of Sketch2: the same plane" "Sketch2 stays on XY"
within "$(frame_of origin)" "0 0 0" 1e-6 "the frame's origin"
within "$(frame_of x)" "0 1 0" 1e-6 "its x axis turned to +Y"
within "$(frame_of y)" "-1 0 0" 1e-6 "its y axis turned to -X"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "the sketch is finished"

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_crashed && ui_fail "the application crashed"
ui_finish "UI Redefine Sketch Plane"
