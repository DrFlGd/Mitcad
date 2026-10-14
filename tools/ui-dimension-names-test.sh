#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework/ParameterPicker.cpp app/framework/ParameterPicker.hpp
# check-all sources: app/framework/CommandSessionPicks.cpp app/framework/CommandSession.cpp
# check-all sources: app/framework/CommandPanel.cpp app/framework/CommandPresenter.hpp
# check-all sources: app/framework/Command.hpp app/MainWindowParameterPicks.cpp
# check-all sources: app/sketch/SketchOverlay.cpp app/sketch/SketchModel.cpp
# check-all sources: app/commands/ExtrudeCommands.cpp app/commands/FilletCommands.cpp app/commands/CommandSupport.cpp tools/cli
# Dimension names in feature panels (mitcad#250), through the real UI:
#   - A design (mitcad-cli): a 40 x 25 rectangle, its width k5 (d1) and
#     height k6 (d2) dimensioned, its left side measured by the driven k7
#     (its reference d3); Extrude1 10 (d4, F2) of it, its corner at (40, 0)
#     rounded by Fillet1 (r 3, d5, F3), and Extrude2 -4 (d6, F4) of it as a
#     second body; the sketch's light bulb on.
#   - Edit Extrude1: the distance field's tooltip says what d4 is, as a
#     sketch dimension's does.
#   - Edit Extrude2: its tooltip; while its distance field has the keyboard,
#     the shown sketch's dimensions, Extrude1's distance and Fillet1's
#     radius (a value of its edge set, a list row) are labelled in the
#     view; a click on the width's label inserts d1 at the field's cursor,
#     one on Extrude1's label d4, one on Fillet1's label d5; the tooltip
#     follows, and OK keeps 2*d1+d4+d5.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-dimension-names-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-dimension-names.XXXXXX)
FILE=$WORK/names.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 40, "height": 25},
  {"cmd": "sketch.set_dimension_text", "sketch": "F1", "dimension": "k5", "text": [20, -10]},
  {"cmd": "sketch.set_dimension_text", "sketch": "F1", "dimension": "k6", "text": [50, 12]},
  {"cmd": "sketch.add_dimension", "sketch": "F1", "dimension": {"type": "length", "line": "c4"},
   "driven": true, "text": [-10, 12]},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "fillet", "body": "F2.b0",
    "edges": ["E{F2:side(c1[c4,c2])|F2:side(c2[c1,c3])}"], "radius": 3}},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$RECT"}],
    "extent": {"type": "distance", "distance": -4}, "operation": "new_body"}},
  {"cmd": "set_feature_visible", "uid": "F1", "visible": true}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Timeline Extrude2 at" "the timeline is logged"
grep -q "Failed features: [^n]" "$UI_LOG" && ui_fail "a feature failed"

echo "--- A feature panel field's tooltip (mitcad#250)"
ui_mark
ui_step "edit Extrude1 (double-click)"   ui_double_click_logged "Timeline Extrude1"
ui_expect_new "Editing F2 with Extrude" "Extrude1 opens for editing" 30
ui_expect_new "Panel Extrude label: Distance (d4)" "the distance field names d4"
ui_expect_new "Panel Extrude tooltip distance: d4 = 10 mm (Extrude1 distance)" "its tooltip says what d4 is"
ui_step "cancel (Esc)"                   ui_key Escape

echo "--- Names picked from the view into the field (mitcad#250)"
ui_mark
ui_step "edit Extrude2 (double-click)"   ui_double_click_logged "Timeline Extrude2"
ui_expect_new "Editing F4 with Extrude" "Extrude2 opens for editing" 30
ui_expect_new "Panel Extrude tooltip distance: d6 = -4" "its tooltip says what d6 is"
ui_expect_new "(Extrude2 distance)" "with its comment"
# The field has the keyboard (the panel gives it to the first value's
# field; the click of ui_type_in does too).
ui_step "type 2* into the distance"      ui_type_in "Panel Extrude input distance" "2*"
ui_expect_new "Parameter label Sketch1 k5 d1 at" "the shown sketch's width is labelled"
ui_expect_new "Parameter label Sketch1 k7 d3 at" "and its driven dimension, by its reference"
ui_expect_new "Parameter label Extrude1 distance d4 at" "and Extrude1's distance"
ui_expect_new "Parameter label Fillet1 sets.0.radius d5 at" "and Fillet1's radius, a value of its edge set"
ui_mark
ui_step "click the width's label"        ui_click_logged "Parameter label Sketch1 k5 d1"
ui_expect_new "Parameter label picked: d1 (Sketch1 k5)" "the width's label picked"
ui_expect_new "Panel Extrude input distance: inserted d1, now 2*d1" "d1 inserted at the cursor"
ui_expect_new "Extrude Distance: 2*d1 = 80" "the distance follows the width"
ui_mark
ui_step "type +"                         xdotool type --delay 40 "+"
ui_expect_new "Extrude Distance: 2*d1+" "the field kept the keyboard"
ui_step "click Extrude1's label"         ui_click_logged "Parameter label Extrude1 distance d4"
ui_expect_new "Parameter label picked: d4 (Extrude1 distance)" "Extrude1's value label picked"
ui_expect_new "Panel Extrude input distance: inserted d4, now 2*d1+d4" "d4 inserted after it"
ui_expect_new "Extrude Distance: 2*d1+d4 = 90" "the distance follows both"
ui_mark
ui_step "type +"                         xdotool type --delay 40 "+"
ui_expect_new "Extrude Distance: 2*d1+d4+" "the field kept the keyboard again"
ui_step "click Fillet1's label"          ui_click_logged "Parameter label Fillet1 sets.0.radius d5"
ui_expect_new "Parameter label picked: d5 (Fillet1 sets.0.radius)" "the fillet set's radius label picked"
ui_expect_new "Panel Extrude input distance: inserted d5, now 2*d1+d4+d5" "d5 inserted after it"
ui_expect_new "Extrude Distance: 2*d1+d4+d5 = 93" "the distance follows all three"
ui_expect_new "Panel Extrude tooltip distance: d6 = 2*d1+d4+d5" "the tooltip shows the expression"
ui_expect_new "93 mm (Extrude2 distance)" "with its value and comment"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F4" "Extrude2 edited" 30

ui_finish "UI dimension names test"
