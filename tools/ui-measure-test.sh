#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/framework core/model-api/src tools/cli
# check-all sources: app/MainWindowMeasureHistory.cpp
# Measure through the real UI, in the Docked layout, on two cylinders of
# radius 5, 20 high (Body1 about Z through the origin, Body2 through
# (30, 40)) and a block 10 x 10 x 20 at (20, -30) (Body3). Axes
# (mitcad#647):
#   - A round face: its axis through its centre in the panel, drawn as a
#     line ("axis lines 1").
#   - Two round faces: parallel, the surfaces 40 apart, the axes 50.
#   - A round face against the block's top: square to it, no axis
#     distance.
#   - Two top rims: the axes' and the centres' distance.
# The history (mitcad#648): the three measurements as three rows, newest
# first, in the next panel; the first shown again (its picks back, its
# lines drawn, no new row); one row and all rows copied; cleared. In the
# Floating layout, after the design is opened again: no rows, then one
# measurement's row shown again in the next panel.
# Tools/ui-windows-measure-test.ps1 does the same in the Windows VM.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-measure-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-measure.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

cat > "$WORK/axes.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F3", "center": [30, 40], "diameter": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F5", "corner": [20, -30], "width": 10, "height": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F5", "region": "$R"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/axes.json" --save "$WORK/axes.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$WORK/axes.mitcad"
ui_step "fit (F6)"                         ui_key F6

echo "--- A round face's axis"
ui_mark
ui_step "measure (I)"                      ui_key i
ui_expect_new "Command Measure started" "measure started"
ui_step "pick Body1's round face"          ui_click_pick "face F2.b0/F2:side(c1)"
ui_expect_new "Inspect Measure: area 628.319, radius 5, diameter 10, axis (0, 0, 10) along (0, 0, " \
  "its axis through its centre"
ui_expect_new "axis lines 1" "the axis drawn through it"

echo "--- Parallel axes"
ui_mark
ui_step "pick Body2's round face"          ui_click_pick "face F4.b0/F4:side(c1)"
ui_expect_new "axis (30, 40, 10) along (0, 0, " "the second axis"
ui_expect_new "distance 40, angle 0 deg, axis distance 50, axis lines 2" \
  "the surfaces 40 apart, the axes parallel and 50 apart, both drawn"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_step "close (Enter)"                    ui_key Return
ui_expect_new "Closed Measure" "measure closed"

echo "--- A round face against a plane"
ui_mark
ui_step "measure (I)"                      ui_key i
ui_step "pick Body1's round face"          ui_click_pick "face F2.b0/F2:side(c1)"
ui_expect_new "axis lines 1" "its axis"
ui_step "pick the block's top"             ui_click_pick "face F6.b0/F6:end($R)"
ui_expect_new "angle 90 deg, axis lines 1" "square to the top, no axis distance"
ui_step "close (Enter)"                    ui_key Return
ui_expect_new "Closed Measure" "measure closed"

echo "--- Two rims"
ui_mark
ui_step "measure (I)"                      ui_key i
ui_step "pick Body1's top rim"             ui_click_pick "edge F2.b0/E{F2:end(r{c1})|"
ui_expect_new "Inspect Measure: length 31.416, radius 5, diameter 10, axis (0, 0, 20) along" "the rim's axis"
ui_step "pick Body2's top rim"             ui_click_pick "edge F4.b0/E{F4:end(r{c1})|"
ui_expect_new "axis distance 50, center distance 50, axis lines 2" "the axes and the centres 50 apart"
ui_step "close (Enter)"                    ui_key Return
ui_expect_new "Closed Measure" "measure closed"

echo "--- The history (mitcad#648)"
# The three measurements above, each a row (a pick's own row became the
# pair's when the second came), newest first, kept across the panels.
ui_mark
ui_step "measure (I)"                      ui_key i
ui_expect_new "Measure history: 3 rows" "three rows"
ui_expect_new "Measure history row 3: Edge of Body1" "the rims' row first"
ui_expect_new "Edge of Body2: Radius 5 mm, Diameter 10 mm, Radius 5 mm, Diameter 10 mm, Distance 40 mm, Angle 0" \
  "with their values"
ui_expect_new ", Axis distance 50 mm, Centre distance 50 mm" "and their axes' and centres' distances"
ui_expect_new "Measure history row 2: Face of Body1" "the round face against the top"
ui_expect_new "Face of Body3: Radius 5 mm, Diameter 10 mm, Distance" "with its distance"
ui_expect_new ", Angle 90" "and its angle"
ui_expect_new "Measure history row 1: Face of Body1" "the parallel faces last"
ui_expect_new "Face of Body2: Radius 5 mm, Diameter 10 mm, Radius 5 mm, Diameter 10 mm, Distance 40 mm, Angle 0" \
  "with their values"
grep "Measure history row 1: " "$UI_LOG" | tail -1 | grep -q ", Axis distance 50 mm" ||
  ui_fail "row 1 has no axis distance"
ui_expect_new "Measure history row 1 at" "its row laid out"
ui_mark
ui_step "show the first again"             ui_click_logged "Measure history row 1"
ui_expect_new "Measure history showed row 1" "shown again"
ui_expect_new "Measure Selection 2: 1 face" "its picks back in the inputs"
ui_expect_new "distance 40, angle 0 deg, axis distance 50, axis lines 2" "measured again, its lines drawn"
grep -q "Measure history: 4 rows" "$UI_LOG" && ui_fail "showing a row again added one"
[ -n "$SHOT" ] && ui_capture "${SHOT%.png}-history.png"
ui_mark
ui_step "copy its row"                     ui_click_logged "Measure history Copy"
ui_expect_new "Measure history copied row 1: 1 | Face of Body1" "the row copied as tab-separated text"
ui_step "copy every row"                   ui_click_logged "Measure history Copy All"
ui_expect_new "Measure history copied 3 rows: 3 | Edge of Body1" "every row, newest first"
ui_expect_new " / 1 | Face of Body1" "down to the first"
ui_step "clear"                            ui_click_logged "Measure history Clear"
ui_expect_new "Measure history cleared" "cleared"
ui_expect_new "Measure history: 0 rows" "no rows"
ui_step "close (Enter)"                    ui_key Return
ui_expect_new "Closed Measure" "measure closed"

echo "--- The history in the Floating layout"
ui_stop_app
ui_start_app --open "$WORK/axes.mitcad" --chrome=floating
ui_step "fit (F6)"                         ui_key F6
# The Floating layout's command card (x from about 990) covers Body2 after
# the fit: zoomed out about Body1, a wheel notch at a time, until Body2
# lies left of the card (as the Windows test does, mitcad#648).
read -r b1x b1y <<< "$(ui_logged_at 'Body F2.b0')"
for _ in $(seq 1 12); do
  read -r b2x _ <<< "$(ui_logged_at 'Body F4.b0')"
  [ $((b2x - X)) -lt $((960 * UI_SCALE)) ] && break
  xdotool mousemove "$b1x" "$b1y" click 5
  ui_sync
  sleep 0.3
done
read -r b2x b2y <<< "$(ui_logged_at 'Body F4.b0')"
[ $((b2x - X)) -lt $((960 * UI_SCALE)) ] || ui_fail "Body2 stays under the command card at $b2x,$b2y"
echo "ok   Body2 left of the command card"
ui_mark
ui_step "measure (I)"                      ui_key i
ui_expect_new "Measure history: 0 rows" "a design opened: no rows"
ui_step "pick Body1's round face"          ui_click_pick "face F2.b0/F2:side(c1)"
ui_step "pick Body2's round face"          ui_click_pick "face F4.b0/F4:side(c1)"
ui_expect_new "axis distance 50, axis lines 2" "measured"
ui_step "close (Enter)"                    ui_key Return
ui_expect_new "Closed Measure" "measure closed"
ui_mark
ui_step "measure (I)"                      ui_key i
ui_expect_new "Measure history: 1 rows" "one row"
ui_expect_new "Measure history row 1: Face of Body1" "the parallel faces"
ui_expect_new "Measure history row 1 at" "its row laid out"
ui_step "show it again"                    ui_click_logged "Measure history row 1"
ui_expect_new "Measure history showed row 1" "shown again"
ui_expect_new "distance 40, angle 0 deg, axis distance 50, axis lines 2" "measured again"
ui_step "close (Enter)"                    ui_key Return
ui_expect_new "Closed Measure" "measure closed"

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_finish "UI measure test"
