#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser app/print app/MainWindowContextMenu.cpp core/model/src/document/bindings.rs tools/cli
# Bindings (mitcad#809, mitcad#861; the model's mitcad#800) through the real
# UI:
#   - A design (mitcad-cli): Box1, Box2 and Box3 (Body1, Body2, Body3); the
#     text parameters Color ('Black', allowed values 'Black' and 'Red') and
#     Holes ('No'). No menu has Follow Option any more.
#   - Body1's Appearance...: the window's assignment panel shows None and
#     what Body1 has; Varies with lists Color (allowed values) and not Holes.
#     Color chosen: a row per value, prefilled, nothing written; the row
#     Black filled by a double-click on Paint - Black writes switch(Color;
#     ...) at once and makes Red the active row ("Assign to row Red"),
#     which the next double-click (Paint - Red) fills; after the last row no
#     row is active and Assign waits for a click.
#   - Change Parameters: Color = 'Red' turns Body1 red (the saved design);
#     Body1's menu marks Appearance... with fx.
#   - Body2's Material...: the material varies with Color the same way in
#     the Materials window (ABS Plastic, PLA Plastic).
#   - Body1's Appearance... again: the binding is shown (Red marked as the
#     value now); None keeps Paint - Red as the appearance set by hand, in
#     one undo step; undo brings the binding back.
#   - Body1, Body2 and Body3 selected (bound, an appearance from Body2's
#     material, none of its own): the Mixed notice; the first fill writes
#     the shown table to all three in one undo step, and undo restores each.
#   - Box2 in the timeline: Suppress When... Holes == 'No' suppresses it;
#     Holes == 'Yes' in Change Parameters brings it back.
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

# last_menu: the last "Context menu: ..." line since the mark.
last_menu() { tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "Context menu: " | tail -1; }

# no_follow_option description: the last menu has no Follow Option entry.
no_follow_option() {
  ! last_menu | grep -qF "Follow Option" || ui_fail "$1 still has Follow Option: $(last_menu)"
  echo "ok   $1 has no Follow Option"
}

# expect_exact_since line description: that exact line in the log since the
# mark.
expect_exact_since() {
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qxF -- "$1" || ui_fail "$2: '$1' not in the log since the mark"
  echo "ok   $2"
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

SWITCH_1="switch(Color; 'Black'; 'paint_black'; 'Red'; 'paint_red')"

cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-70, -10],
    "length": 40, "width": 20, "height": 20, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-20, -10],
    "length": 40, "width": 20, "height": 20, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [30, -10],
    "length": 40, "width": 20, "height": 20, "operation": "new_body"}},
  {"cmd": "add_parameter", "name": "Color", "expression": "'Black'", "values": ["'Black'", "'Red'"]},
  {"cmd": "add_parameter", "name": "Holes", "expression": "'No'"}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE" --no-native-dialogs
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Body F1.b0 at" "the boxes are in the view"

echo "--- No Follow Option in the menus"
ui_mark
ui_step "right-click Body1 in the view"    ui_click_logged "Body F1.b0" 3
ui_expect_new "Context menu: " "the view's menu"
last_menu | grep -qF "Appearance" || ui_fail "the view's menu has no Appearance...: $(last_menu)"
no_follow_option "the view's menu"
ui_key Escape
ui_focus_main

echo "--- Body1's appearance varies with Color (the Appearances window)"
ui_mark
ui_step "right-click Body1"                ui_click_logged "Browser Root/Bodies/Body1" 3
ui_expect_new "Context menu: " "the row's menu"
last_menu | grep -qF "Show When" || ui_fail "Show When... is not in the body's menu"
echo "ok   the body's menu has Show When..."
no_follow_option "the body's menu"
ui_step "choose Appearance..."             ui_menu_choose "Appearance..."
ui_focus_dialog '^Appearances$'
ui_expect_new "Appearances panel: Assign to Body1; varies with None (Steel - Satin)" \
  "the panel shows None and what Body1 has"
ui_expect_new "Appearances varies with: None | Color" "Varies with lists Color (allowed values), not Holes"
ui_expect_new "Appearances varies at" "Varies with is logged"
ui_mark
ui_step "Varies with: Color"               ui_choose "Appearances varies" 1
ui_expect_new "Appearances varies with Color: 2 rows, not written yet" "Color chosen, nothing written"
ui_expect_new "Appearances value Black: Steel - Satin (now) (active)" "Black prefilled, the value now, active"
ui_expect_new "Appearances value Red: Steel - Satin" "Red prefilled"
ui_expect_new "Appearances assign label: Assign to row Black" "Assign names the active row"
ui_expect_new "Appearances value Red at" "the rows' places are logged"
if tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -q "Appearances binding of"; then
  ui_fail "choosing Color wrote a binding"
fi
echo "ok   nothing written yet"
ui_step "click the search field"           ui_click_logged "Appearances search"
ui_step "type paint"                       xdotool type --delay 60 "paint"
ui_expect_new 'Appearances filtered "paint"' "the paints in sight"
ui_expect_new "Appearances item paint_black at" "Paint - Black is listed"
ui_double_click_expect "Appearances item paint_black" "Appearances filled Black with 'paint_black'" \
  "a double-click fills the row Black"
ui_expect_new "Appearances binding of Body1: switch(Color; 'Black'; 'paint_black'; 'Red'; 'steel_satin')" \
  "written at once"
ui_expect_new "Appearances assign label: Assign to row Red" "Red is the active row now"
ui_expect_new "Appearances value Red: Steel - Satin (active)" "Red marked active"
ui_double_click_expect "Appearances item paint_red" "Appearances filled Red with 'paint_red'" \
  "the next double-click fills the row Red"
ui_expect_new "Appearances binding of Body1: $SWITCH_1" "the whole switch() written again"
ui_expect_new "Appearances active row: none" "after the last row no row is active"
expect_exact_since "Appearances assign label: Assign" "Assign waits for a row to be clicked"
ui_expect_new "Appearances panel: Assign to Body1; varies with Color (bound)" "the panel shows the binding"
ui_step "Close"                            ui_click_logged "Appearances close"
ui_focus_main
save
expect_file '.bodies[0] | [.appearance, .appearance_expr]' "[\"paint_black\",\"$SWITCH_1\"]" \
  "Body1 is black and follows Color"

echo "--- Change Parameters: Color = 'Red'"
set_text Color Red
save
expect_file '.bodies[0].appearance' '"paint_red"' "Body1 turned red"

echo "--- Body2's material varies with Color (the Materials window)"
ui_mark
ui_step "right-click Body2"                ui_click_logged "Browser Root/Bodies/Body2" 3
ui_expect_new "Context menu: " "the row's menu"
if tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -q "Context menu fx"; then
  ui_fail "Body2's menu marks a binding it does not have"
fi
echo "ok   no fx mark without a binding"
ui_step "choose Material..."               ui_menu_choose "Material..."
ui_focus_dialog '^Materials$'
ui_expect_new "Materials panel: Assign to Body2; varies with None (Steel)" "the panel shows what Body2 is made of"
ui_expect_new "Materials varies at" "Varies with is logged"
ui_mark
ui_step "Varies with: Color"               ui_choose "Materials varies" 1
ui_expect_new "Materials varies with Color: 2 rows, not written yet" "Color chosen, nothing written"
ui_expect_new "Materials value Red: Steel (now)" "Red is the value now"
ui_expect_new "Materials value Black at" "the rows' places are logged"
ui_expect_new "Materials item ABS Plastic at" "ABS is listed"
ui_double_click_expect "Materials item ABS Plastic" "Materials filled Black with 'abs'" \
  "a double-click fills the row Black"
ui_expect_new "Materials binding of Body2: switch(Color; 'Black'; 'abs'; 'Red'; 'steel')" "written at once"
ui_expect_new "Materials assign label: Assign to row Red" "Red is the active row now"
ui_double_click_expect "Materials item PLA Plastic" "Materials filled Red with 'pla'" \
  "the next double-click fills the row Red"
ui_expect_new "Materials binding of Body2: switch(Color; 'Black'; 'abs'; 'Red'; 'pla')" "written again"
ui_step "Close"                            ui_click_logged "Materials close"
ui_focus_main
save
expect_file '.bodies[1] | [.material, .material_expr]' \
  "[\"pla\",\"switch(Color; 'Black'; 'abs'; 'Red'; 'pla')\"]" "Body2 is PLA and follows Color"

echo "--- Body1: back to None keeps the appearance shown; undo brings the binding back"
ui_mark
ui_step "right-click Body1"                ui_click_logged "Browser Root/Bodies/Body1" 3
ui_expect_new "Context menu fx: Appearance..." "the menu marks Appearance... (fx)"
ui_step "choose Appearance..."             ui_menu_choose "Appearance..."
ui_focus_dialog '^Appearances$'
ui_expect_new "Appearances panel: Assign to Body1; varies with Color (bound)" "the window shows Body1's binding"
ui_expect_new "Appearances value Red: Paint - Red (now)" "Red is marked as the value now"
ui_expect_new "Appearances varies at" "Varies with is logged"
ui_mark
ui_step "Varies with: None"                ui_choose "Appearances varies" -1
ui_expect_new "Appearances back to None: paint_red for Body1" "None sets the shown appearance by hand"
ui_expect_new "Appearances panel: Assign to Body1; varies with None (Paint - Red)" "the panel shows None"
ui_step "Close"                            ui_click_logged "Appearances close"
ui_focus_main
save
expect_file '.bodies[0] | [.appearance, .appearance_expr]' '["paint_red",null]' \
  "Body1 keeps red, the binding gone"
ui_mark
ui_step "undo"                             ui_key ctrl+z
ui_expect_new "Undo: Set Appearance of Body1" "one undo step"
save
expect_file '.bodies[0] | [.appearance, .appearance_expr]' "[\"paint_red\",\"$SWITCH_1\"]" \
  "undo brings the binding back"

echo "--- Three bodies of different assignments: Mixed, and the first fill writes to all"
# Body1 is still selected from its right-click: a click on it would change
# nothing (and log nothing), so the selection is cleared first.
ui_mark
ui_step "Escape (clear the selection)"     ui_key Escape
ui_expect_new "Browser selected: nothing" "nothing selected"
ui_mark
ui_step "click Body1"                      ui_click_logged "Browser Root/Bodies/Body1"
ui_expect_new "Browser selected: Root/Bodies/Body1" "Body1 selected"
ui_step "Ctrl+click Body2"                 xdotool mousemove $(ui_logged_at "Browser Root/Bodies/Body2") keydown ctrl click 1 keyup ctrl
ui_expect_new "Browser selected: Root/Bodies/Body1, Root/Bodies/Body2" "Body1 and Body2 selected"
ui_step "Ctrl+click Body3"                 xdotool mousemove $(ui_logged_at "Browser Root/Bodies/Body3") keydown ctrl click 1 keyup ctrl
ui_expect_new "Browser selected: Root/Bodies/Body1, Root/Bodies/Body2, Root/Bodies/Body3" "the three bodies selected"
ui_mark
ui_step "right-click Body1"                ui_click_logged "Browser Root/Bodies/Body1" 3
ui_step "choose Appearance..."             ui_menu_choose "Appearance..."
ui_focus_dialog '^Appearances$'
ui_expect_new "Appearances panel: Assign to 3 Bodies; varies with Color (bound)" "the table shows Body1's state"
ui_expect_new "Appearances mixed: Mixed: Body1 follows Color, Body2 has Plastic - White, Body3 has Steel - Satin" \
  "the Mixed notice names each state"
ui_expect_new "Filling a row gives all 3 bodies this table." "and says what the first fill does"
ui_expect_new "Appearances assign label: Assign to row Black (3 bodies)" "Assign names the row and the bodies"
ui_expect_new "Appearances value Black: Paint - Black" "Body1's table"
ui_expect_new "Appearances item paint_white at" "Paint - White is listed"
ui_double_click_expect "Appearances item paint_white" "Appearances filled Black with 'paint_white'" \
  "the first fill"
ui_expect_new "Appearances binding of Body1, Body2 and Body3: switch(Color; 'Black'; 'paint_white'; 'Red'; 'paint_red')" \
  "written to all three"
ui_step "Close"                            ui_click_logged "Appearances close"
ui_focus_main
ui_step "Escape (clear the selection)"     ui_key Escape
save
expect_file '[.bodies[] | .appearance_expr]' \
  "[\"switch(Color; 'Black'; 'paint_white'; 'Red'; 'paint_red')\",\"switch(Color; 'Black'; 'paint_white'; 'Red'; 'paint_red')\",\"switch(Color; 'Black'; 'paint_white'; 'Red'; 'paint_red')\"]" \
  "the three bodies follow the shown table"
expect_file '[.bodies[] | .appearance]' '["paint_red","paint_red","paint_red"]' "all three are red (Color = 'Red')"
ui_mark
ui_step "undo"                             ui_key ctrl+z
ui_expect_new "Undo: Appearance of Body1, Body2 and Body3" "one undo step, named for all three"
save
expect_file '[.bodies[] | .appearance_expr]' "[\"$SWITCH_1\",null,null]" "undo restores each body's state"

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

ui_finish "UI bindings"
