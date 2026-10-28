#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/configurations app/browser/ConfigurationRulesPage.cpp app/browser/ConfigurationsDialog.cpp
# check-all sources: app/framework/FormulaEditor.cpp tools/cli core/model-base/src/configurations.rs
# check-all sources: core/model-base/src/configurations core/model/src/document/configurations.rs
# check-all sources: core/model-api/src/configurations.rs
# The rule block editor (mitcad#808) through the real UI, on a design
# mitcad-cli makes: a text option Color ('Black' or 'White', White) and a
# number option Length (300, 800, 1200 or 2500 mm; 1200 mm), no rules.
#   1. Docked: Configurations…, the Rules tab, Add, the name "Black is
#      short"; then with the mouse from the palette: when … then onto the
#      rule, compare into the when, option (Color) and text (Black) into
#      its sockets, max into the then (its option Length), number 1000 mm
#      into it. Each complete edit is saved (one undo step); View as Text
#      shows the rule's expression.
#   2. The browser's Configuration bar: Color = Black moves Length from
#      1200 mm to 800 mm (the rule named), and the bar offers 300 mm and
#      800 mm only.
#   3. Floating, the keyboard only (after a click on the Rules tab): Alt+A
#      adds a rule, named "Not too long"; Alt+P, the palette's max, Enter
#      puts it in the then; Alt+P, number with unit, Enter, 2000 mm; Left
#      to its option, Enter edits it, Esc; forbid put after it and Delete
#      takes it out; Shift+F10 opens the context menu; Alt+K, Check Rules,
#      lists 2500 mm as a value that is never allowed.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-configuration-rules-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-configuration-rules.XXXXXX)
FILE=$WORK/shelf.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_parameter", "name": "Color", "unit": "Text", "expression": "'White'",
   "values": ["'Black'", "'White'"]},
  {"cmd": "add_parameter", "name": "Length", "unit": "mm", "expression": "1200 mm",
   "values": ["300 mm", "800 mm", "1200 mm", "2500 mm"]},
  {"cmd": "set_configurations", "configurations": {"options": ["Color", "Length"]}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# drag "palette id" "canvas place": a palette entry dragged onto the canvas.
drag() {
  ui_drag_logged "Rule palette $1" "Rule canvas $2"
}

# rules_tab: the Configurations window (the command search) on its Rules tab.
rules_tab() {
  ui_mark
  ui_step "Configurations (search)"      ui_command "Configurations"
  ui_expect_new "Configurations window opened" "the window opened"
  ui_focus_dialog '^Configurations$'
  ui_expect_log "Configurations tab Rules at" "the Rules tab's place is logged"
  ui_mark
  ui_step "the Rules tab"                ui_click_logged "Configurations tab Rules"
  ui_expect_new "Configurations page Rules" "the Rules page"
  ui_expect_new "Rule palette compare at" "the palette's places are logged"
  ui_expect_new "Rules add at" "the page's buttons are logged"
}

ui_start_display
ui_start_app --open "$FILE" --no-native-dialogs
ui_expect_log "Configuration bar: Color = White (Black | White); Length = 1200 mm (300 mm | 800 mm | 1200 mm | 2500 mm)" \
  "the bar shows the two options"

echo "--- 1. Black is short, with the mouse (Docked)"
rules_tab
ui_mark
ui_step "Add"                            ui_click_logged "Rules add"
ui_expect_new "Rules saved: Add rule Rule 1" "a new rule, saved"
ui_expect_new "Rules naming Rule 1" "its name to be typed"
ui_step "type its name"                  xdotool type --delay 40 "Black is short"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rules saved: Rename rule Rule 1 to Black is short" "renamed"
ui_expect_new "Rule canvas then.0 slot at" "the canvas shows the rule"
ui_mark
ui_step "when … then onto the rule"      drag when "rule frame"
ui_expect_new "Rule edited Black is short: Rule when" "the rule has a when"
ui_expect_new "Rules not saved yet: Rule when" "an empty when is not saved"
ui_expect_new "Rules row Black is short: enabled, Not saved: fill its empty sockets" "the list marks it"
ui_expect_new "Rule canvas when socket at" "its socket's place is logged"
ui_mark
ui_step "compare into the when"          drag compare "when socket"
ui_expect_new "Rule canvas target when socket" "the socket is highlighted on the way"
ui_expect_new "Rule edited Black is short: Put compare in when" "a compare in the when"
ui_expect_new "Rule canvas when.0 socket at" "its sockets' places are logged"
ui_mark
ui_step "option into its left"           drag option "when.0 socket"
ui_expect_new "Rule canvas editing when.0 block: Color" "the first option, Color, to be confirmed"
ui_step "Enter"                          ui_key Return
ui_mark
ui_step "text into its right"            drag text "when.1 socket"
ui_expect_new "Rule canvas editing when.1 block: " "the text to be typed"
ui_step "type Black"                     xdotool type --delay 40 "Black"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rules saved: Set when.1 to 'Black'" "the condition is complete: saved"
ui_mark
ui_step "max into the then"              drag max "then.0 slot"
ui_expect_new "Rule edited Black is short: Put max in then.0" "max in the then"
ui_expect_new "Rule canvas then.0 option at" "its option's place is logged"
ui_mark
ui_step "its option (a click)"           ui_click_logged "Rule canvas then.0 option"
ui_expect_new "Rule canvas editing then.0 option: Length" "the number option, Length"
ui_step "Enter"                          ui_key Return
ui_mark
ui_step "number into its socket"         drag number "then.0.0 socket"
ui_expect_new "Rule canvas editing then.0.0 block: " "the number to be typed"
ui_step "type 1000 mm"                   xdotool type --delay 40 "1000 mm"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rules saved: Set then.0.0 to 1000 mm" "the rule is complete: saved"
ui_expect_new "Rules row Black is short: enabled, ok" "no mark in the list"
ui_mark
ui_step "View as Text"                   ui_click_logged "Rules text"
ui_expect_new "Rule as text Black is short: when Color == 'Black' / then Length max 1000 mm" \
  "View as Text shows the expression"
[ -n "$SHOT" ] && ui_capture "$SHOT"

echo "--- 2. The bar: Black limits Length"
ui_focus_main
ui_expect_log "Configuration bar Color at" "the Color chooser's place is logged"
ui_mark
ui_step "the bar's Color: Black"         ui_choose "Configuration bar Color" -1
ui_expect_new "Configuration chosen: Color = Black" "Black chosen"
ui_expect_new "Configuration moves: Length 1200 mm → 800 mm: Black is short" "Length moved by the rule"
ui_expect_new "Configuration bar: Color = Black (Black | White); Length = 800 mm (300 mm | 800 mm)" \
  "the bar's Length is limited by the rule"
ui_mark
ui_step "save (Ctrl+S)"                  ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"

echo "--- 3. Not too long, with the keyboard (Floating)"
ui_stop_app
ui_start_app --open "$FILE" --no-native-dialogs --chrome=floating
ui_expect_log "Configuration bar: Color = Black" "the design as saved"
rules_tab
ui_expect_log "Rules row Black is short: enabled, ok" "the saved rule is listed"
ui_mark
ui_step "Alt+A (Add)"                    ui_key alt+a
ui_expect_new "Rules saved: Add rule Rule 1" "a new rule"
ui_expect_new "Rules naming Rule 1" "its name to be typed"
ui_step "type its name"                  xdotool type --delay 40 "Not too long"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rules saved: Rename rule Rule 1 to Not too long" "renamed"
ui_mark
ui_step "Alt+P (the palette)"            ui_key alt+p
ui_step "max (type-ahead)"               ui_type_ahead "max"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rule edited Not too long: Put max in then.0" "max in the then"
ui_expect_new "Rule canvas focus then.0.0 socket" "the focus on its empty socket"
ui_mark
ui_step "Alt+P (the palette)"            ui_key alt+p
ui_step "number (type-ahead)"            ui_type_ahead "number"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rule canvas editing then.0.0 block: " "the number to be typed"
ui_step "type 2000 mm"                   xdotool type --delay 40 "2000 mm"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rules saved: Set then.0.0 to 2000 mm" "saved"
ui_mark
ui_step "Alt+B (the blocks)"             ui_key alt+b
ui_step "Left"                           ui_key Left
ui_expect_new "Rule canvas focus then.0 option" "Left: the option"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rule canvas editing then.0 option: Length" "Enter edits it"
ui_step "Esc"                            ui_key Escape
ui_expect_new "Rule canvas editing then.0 option cancelled" "Esc only cancels the edit"
ui_mark
ui_step "Alt+P (the palette)"            ui_key alt+p
ui_step "forbid (type-ahead)"            ui_type_ahead "forbid"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rule edited Not too long: Put forbid in then.1" "forbid after the max"
ui_expect_new "Rule canvas editing then.1 message: " "its message to be typed"
ui_step "type never"                     xdotool type --delay 40 "never"
ui_step "Enter"                          ui_key Return
ui_expect_new "Rules saved: Set message of then.1 to never" "saved"
ui_expect_new "Rule canvas focus then.1 block" "the focus on the forbid"
ui_mark
ui_step "Delete"                         ui_key Delete
ui_expect_new "Rules saved: Delete then.1" "Delete takes it out"
ui_step "Left"                           ui_key Left
ui_step "Shift+F10"                      ui_key shift+F10
ui_expect_new "Context menu: " "the context menu opens from the keyboard"
ui_step "Esc"                            ui_key Escape
ui_expect_log "Rules row Not too long: enabled, ok" "the rule is listed without a mark"
ui_mark
ui_step "Alt+T (View as Text)"           ui_key alt+t
ui_expect_new "Rule as text Not too long: always / then Length max 2000 mm" "its text"
ui_mark
ui_step "Alt+K (Check Rules)"            ui_key alt+k
ui_expect_new "Check Rules never allowed: Length 2500 mm" "Check Rules lists a value that is never allowed"

ui_finish "UI configuration rules test"
