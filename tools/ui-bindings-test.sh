#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser app/MainWindowContextMenu.cpp core/model/src/document/bindings.rs tools/cli
# Bindings (mitcad#809, the model's mitcad#800) through the real UI:
#   - A design (mitcad-cli): Box1 (Body1) and Box2 (Body2); the text
#     parameters Color ('Black', allowed values 'Black' and 'Red') and Holes
#     ('No').
#   - Body1's row: Follow Option > Appearance... lists Color's values; Black
#     gets Paint - Black and Red Paint - Red, typed into the result fields;
#     OK binds Body1's appearance to switch(Color; ...) in one undo step,
#     and the browser marks Body1's row (fx).
#   - Change Parameters: Color = 'Red' turns Body1 red (the saved design).
#   - Box2 in the timeline: Suppress When... Holes == 'No' suppresses it;
#     Holes == 'Yes' in Change Parameters brings it back.
#   - Appearance... of Body1 set by hand (Steel - Satin) ends the binding in
#     one undo step; undo brings the binding back.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-bindings-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-bindings.XXXXXX)
FILE=$WORK/bindings.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

# expect_file filter expected description: a jq filter on the saved file.
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

# set_text name text: the parameter's expression in Change Parameters,
# the table closed again.
set_text() {
  ui_mark
  ui_step "Change Parameters (search)"     ui_command "Change Parameters"
  ui_expect_new "Parameters dialog opened" "the table opened"
  ui_expect_log "Parameters $1 expression at" "$1's cells are logged"
  ui_focus_dialog '^Parameters$'
  ui_double_click_logged "Parameters $1 expression"
  ui_sync
  ui_key ctrl+a
  xdotool type --delay 40 "'$2'"
  ui_key Return
  ui_expect_new "Parameter $1 expression: '$2' = " "$1 = '$2'"
  ui_step "OK (the table)"                 ui_click_logged "Parameters OK"
  ui_focus_main
}

cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-45, -10],
    "length": 40, "width": 20, "height": 20, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [5, -10],
    "length": 40, "width": 20, "height": 20, "operation": "new_body"}},
  {"cmd": "add_parameter", "name": "Color", "expression": "'Black'", "values": ["'Black'", "'Red'"]},
  {"cmd": "add_parameter", "name": "Holes", "expression": "'No'"}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE" --no-native-dialogs
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Body F2.b0 at" "the boxes are in the view"

echo "--- Body1: Follow Option > Appearance..."
ui_mark
ui_step "right-click Body1"                ui_click_logged "Browser Root/Bodies/Body1" 3
ui_expect_new "Context menu Follow Option: Appearance... | Material..." "the Follow Option submenu"
ui_expect_new "Context menu: " "the row's menu"
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "Context menu: " | tail -1 | grep -qF "Show When" ||
  ui_fail "Show When... is not in the body's menu"
echo "ok   the body's menu has Show When..."
ui_step "choose Follow Option > Appearance..." ui_menu_choose "Follow Option" "Appearance..."
ui_focus_dialog '^Appearance of Body1 Follows Option$'
ui_expect_new "Follow Option values of Color: Black, Red" "Color's allowed values fill the table"
ui_expect_new "Follow Option result 2 at" "the result fields are logged"
ui_step "Black: Paint - Black"             ui_type_in "Follow Option result 1" "Paint - Black"
ui_step "Red: Paint - Red"                 ui_type_in "Follow Option result 2" "Paint - Red"
ui_expect_new "Follow Option expression: switch(Color; 'Black'; 'paint_black'; 'Red'; 'paint_red'; 'paint_black')" \
  "the table writes switch()"
ui_mark
ui_step "OK"                               ui_click_logged "Follow Option OK"
ui_focus_main
ui_expect_new "Appearance of Body1 follows switch(Color; 'Black'; 'paint_black'; 'Red'; 'paint_red'; 'paint_black')" \
  "Body1's appearance follows Color"
ui_expect_new "Browser fx Root/Bodies/Body1: Appearance = switch(Color;" "the browser marks Body1's row"
save
expect_file '.bodies[0] | [.appearance, .appearance_expr]' \
  "[\"paint_black\",\"switch(Color; 'Black'; 'paint_black'; 'Red'; 'paint_red'; 'paint_black')\"]" \
  "Body1 is black and follows Color"

echo "--- Change Parameters: Color = 'Red'"
set_text Color Red
save
expect_file '.bodies[0].appearance' '"paint_red"' "Body1 turned red"

echo "--- Box2: Suppress When... Holes == 'No'"
ui_mark
ui_step "right-click Box2"                 ui_click_logged "Timeline Box2" 3
ui_step "choose Suppress When..."          ui_menu_choose "Suppress When..."
ui_focus_dialog '^Suppress Box2 When$'
ui_expect_new "Condition dialog Suppress Box2 When: none" "the dialog opened without a condition"
ui_expect_new "Condition OK at" "its OK is logged"
ui_step "type Holes == 'No'"                ui_type_text "Holes == 'No'"
ui_mark
ui_step "OK"                               ui_click_logged "Condition OK"
ui_focus_main
ui_expect_new "Suppress Box2 when Holes == 'No'" "Box2 follows the condition"
save
expect_file '.features[1] | [.suppressed, .suppressed_when]' "[true,\"Holes == 'No'\"]" "Box2 is suppressed"
set_text Holes Yes
save
expect_file '.features[1] | [(.suppressed // false), .suppressed_when]' "[false,\"Holes == 'No'\"]" \
  "Box2 is back, still following the condition"

echo "--- Appearance... by hand ends Body1's binding"
ui_mark
ui_step "right-click Body1"                ui_click_logged "Browser Root/Bodies/Body1" 3
ui_step "choose Appearance..."             ui_menu_choose "Appearance..."
ui_expect_new "Appearances for bodies: F1.b0" "the Appearances window opened for Body1"
ui_expect_new "Appearances item steel_satin at" "the library is listed"
ui_focus_dialog '^Appearances$'
ui_step "click Steel - Satin"              ui_click_logged "Appearances item steel_satin"
ui_expect_new "Appearance selected: steel_satin" "Steel - Satin selected"
ui_step "Assign to Body1"                  ui_click_logged "Appearances assign"
ui_expect_new "Assigned appearance steel_satin to F1.b0" "assigned"
ui_step "Close"                            ui_click_logged "Appearances close"
ui_focus_main
save
expect_file '.bodies[0] | [.appearance, .appearance_expr]' '["steel_satin",null]' \
  "the appearance set by hand, the binding gone"
ui_mark
ui_step "undo"                             ui_key ctrl+z
# One body: the model's own undo step (several bodies are one step named Appearance).
ui_expect_new "Undo: Set Appearance of Body1" "one undo step"
save
expect_file '.bodies[0] | [.appearance, (.appearance_expr | startswith("switch(Color;"))]' '["paint_red",true]' \
  "undo brings the binding back"

ui_finish "UI bindings"
