#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/commands/SketchCommands.cpp tools/cli
# check-all sources: core/sketch core/model-api/src/sketch.rs core/solver
# Closed fitted splines and tangents at a chosen end (mitcad#783), through
# the real UI, on a design mitcad-cli makes: Sketch1 (F1) on XY with a
# fitted spline c1 through p2 (-30, -30), p3 (-10, -20) and p4 (10, -30),
# a line c5 from (-70, -60) to its start p2 and a line c7 from its end p4
# to (50, -60), each along the spline's end direction (slope 3/4: the
# tangents hold as drawn, so the spline keeps its shape), and two points
# that widen the fitted view.
#   1. Tangent: the spline picked near its start and the first line, then
#      near its end and the second line: each tangent records the end it
#      was picked at (p2, p4), so one spline is tangent at both ends.
#   2. Fit Point Spline: four points and a click on the first one close it
#      into one smooth (periodic) spline, one profile.
#   3. Extrude makes one body of it; saved, the file has the closed spline
#      and both tangents' points.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-sketch-spline-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-sketch-spline.XXXXXX)
FILE=$WORK/spline.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.spline", "sketch": "F1", "points": [[-30, -30], [-10, -20], [10, -30]]},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [-70, -60], "end": "p2"},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": "p4", "end": [50, -60]},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [-60, 50]},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [60, 50]}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Timeline Sketch1 at" "the timeline is logged"
ui_step "edit Sketch1 (double-click)"    ui_double_click_logged "Timeline Sketch1"
ui_expect_log "Editing sketch F1 (Sketch1)" "Sketch1 is edited"
ui_step "fit (F6)"                       ui_key F6
ui_sync

echo "--- Tangent at each end of the spline"
# The spline is the natural cubic through its points: x = -30 + 40 t,
# y = -30 - 40 t^3 + 30 t on its first half, mirrored on the second; at
# t = 0.2 and 0.8 it is at (-22, -24.32) and (2, -24.32).
ui_mark
ui_step "tangent (search)"               ui_command "Tangent"
ui_step "pick the spline near its start" ui_sketch_click -22 -24.32
ui_step "pick the first line"            ui_sketch_click -50 -45
ui_expect_new "Added constraint tangent k" "a tangent added"
ui_expect_new "Tangent would apply at p2" "while picking, the glyph is at the end it will use"
ui_expect_new " on c1, c5 at p2" "at the spline's start, where it was picked"
ui_mark
ui_step "pick the spline near its end"   ui_sketch_click 2 -24.32
ui_step "pick the second line"           ui_sketch_click 30 -45
ui_expect_new "Tangent would apply at p4" "while picking, the glyph is at the other end"
ui_expect_new " on c1, c7 at p4" "at the spline's end: tangent at both ends"
ui_step "end the tool (Esc)"             ui_key Escape
grep -q "Constraint tangent refused" "$UI_LOG" && ui_fail "a tangent was refused"

echo "--- A closed spline by a click on its first point"
ui_mark
ui_step "spline (search)"                ui_command "Fit Point Spline"
ui_step "spline: 1"                      ui_sketch_click -50 20
ui_step "spline: 2"                      ui_sketch_click -10 10
ui_step "spline: 3"                      ui_sketch_click 30 20
ui_step "spline: 4"                      ui_sketch_click -10 40
ui_step "spline: the first point again"  ui_sketch_click -50 20
ui_expect_new "Fit Point Spline: a click here closes it" "the first point is marked before the click"
ui_expect_new "Added closed spline c11 through 4 points" "the click on the first point closed it"
ui_step "end the tool (Esc)"             ui_key Escape
grep -q "Sketch command sketch.spline refused" "$UI_LOG" && ui_fail "the closed spline was refused"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "sketch finished"

echo "--- One profile, one body"
ui_mark
ui_step "extrude (E)"                    ui_key e
ui_step "type 10"                        xdotool type --delay 40 "10"
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "New body Body1 (F2.b0): volume" "the closed spline extruded as one profile"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_mark
ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
grep -qE '"closed": ?true' "$FILE" || ui_fail "the saved spline is not closed"
echo "ok   the saved spline is closed"
grep -qE '"at": ?"p2"' "$FILE" && grep -qE '"at": ?"p4"' "$FILE" ||
  ui_fail "the saved tangents do not have their points"
echo "ok   the saved tangents have their points"

ui_finish "UI sketch spline test"
