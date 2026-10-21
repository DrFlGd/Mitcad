#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser app/framework/Appearances.cpp app/framework/Appearances.hpp app/MainWindow.cpp tools/cli app/MainWindowContextMenu.cpp
# Edit Appearances: Assign to N bodies (mitcad#346) through the real UI, on
# three boxes side by side (F1.b0, F2.b0, F3.b0):
#   - With bodies only selected there is no Assign to N bodies button.
#   - With two faces of Body1 and a face of Body2 selected, the dialog shows
#     Assign to 2 bodies; using it gives both bodies the appearance as their
#     body appearance in one undo step, the selected faces get no face
#     appearance of their own, and Body3 keeps the default look.
#   - Undo takes both bodies' appearance back at once.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-appearance-bodies-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-appearance-bodies.XXXXXX)
FILE=$WORK/boxes.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

# expect_file filter expected description: a jq filter on the saved project
# file.
expect_file() {
  local actual
  actual=$(jq -c "$1" "$FILE") || ui_fail "$3: cannot read $FILE"
  [ "$actual" = "$2" ] || ui_fail "$3: $1 is '$actual', expected '$2'"
  echo "ok   $3"
}

save() {
  ui_mark
  ui_key ctrl+s
  ui_expect_new "Saved $FILE" "saved"
}

# ctrl_click x y: a click with Ctrl held, adding to the selection.
ctrl_click() { xdotool mousemove "$1" "$2" keydown ctrl click 1 keyup ctrl; }

# Three 40 x 20 x 20 boxes 10 mm apart along X, and a copy of Paint - Red
# in the design.
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-75, -10],
    "length": 40, "width": 20, "height": 20, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-20, -10],
    "length": 40, "width": 20, "height": 20, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [35, -10],
    "length": 40, "width": 20, "height": 20, "operation": "new_body"}},
  {"cmd": "create_appearance", "id": "red_copy", "name": "Red Copy", "based_on": "paint_red"}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE" --no-native-dialogs
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Body F1.b0 at" "the first box is in the view"
ui_expect_log "Body F3.b0 at" "and the third"

echo "--- Bodies selected: no Assign to N bodies"
ui_mark
ui_step "right-click Body1's row"        ui_click_logged "Browser Root/Bodies/Body1" 3
ui_step "choose Appearance..."           ui_menu_choose "Appearance..."
ui_expect_new "Appearances for bodies: F1.b0" "the dialog opened for the body"
ui_focus_dialog '^Appearances$'
ui_sync
if grep -qF "Appearances assign_bodies" "$UI_LOG"; then
  ui_fail "Assign to N bodies is shown for a body"
fi
echo "ok   there is no Assign to N bodies button for a body"
ui_step "Close"                          ui_click_logged "Appearances close"
ui_focus_main
ui_step "Escape (clear the selection)"   ui_key Escape

echo "--- Faces of two bodies selected"
ui_mark
read -r bx by <<< "$(ui_logged_at "Body F1.b0")"
read -r cx cy <<< "$(ui_logged_at "Body F2.b0")"
# The logged place is where the body's corner nearest the camera is: its top
# face is above it, a side face below.
ui_step "click the top face of Body1"    xdotool mousemove "$bx" $((by - 20)) click 1
ui_expect_new "Selected: 1 face [face" "a face of Body1 selected"
ui_step "Ctrl+click a side face of Body1" ctrl_click $((bx + 12)) $((by + 40))
ui_step "Ctrl+click the top face of Body2" ctrl_click "$cx" $((cy - 20))
ui_expect_new "Selected: 3 faces" "three faces selected"
ui_mark
ui_step "right-click Body2's selected face"    xdotool mousemove "$cx" $((cy - 20)) click 3
ui_step "choose Appearance..."           ui_menu_choose "Appearance..."
ui_expect_new "Appearances for faces:" "the dialog opened for the faces"
ui_focus_dialog '^Appearances$'
ui_expect_new "Appearances assign_bodies label: Assign to 2 bodies" "the dialog offers Assign to 2 bodies"
# The design's own appearances are last in the list.
ui_step "the list"                       ui_click_logged "Appearances item plastic_white"
ui_step "Red Copy (End)"                 ui_key End
ui_expect_new "Appearance selected: red_copy" "Red Copy is selected"
ui_mark
ui_step "Assign to 2 bodies"             ui_click_logged "Appearances assign_bodies"
ui_expect_new "Appearance Red Copy assigned to 2 bodies (from faces)" "both bodies got it"
ui_step "Close"                          ui_click_logged "Appearances close"
ui_focus_main
save
expect_file '[.bodies[] | .appearance]' '["red_copy","red_copy",null]' "Body1 and Body2 have it as their body appearance"
expect_file '[.bodies[] | has("face_appearances")]' '[false,false,false]' "no face appearance of its own"
ui_mark
ui_step "undo"                           ui_key ctrl+z
ui_expect_new "Undo: Appearance" "one undo step"
save
expect_file '[.bodies[] | .appearance]' '[null,null,null]' "both bodies are back to the default look"

ui_finish "UI Edit Appearances, Assign to bodies"
