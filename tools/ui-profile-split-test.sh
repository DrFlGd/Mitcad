#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/framework/CommandSession.cpp app/framework/Selection.cpp
# check-all sources: app/OcctViewer.cpp core/model/src/sketch/regions.rs core/model/src/features/sketch.rs tools/cli
# Extrude's profile picker with a region a sketch edit split (mitcad#410),
# through the real UI, in the Docked and the Floating chrome:
#   - A bar's tip between two circles is extruded; the bar is lengthened
#     across the outer circle, which splits the tip in two (mitcad#407).
#     Editing the extrusion shows its one reference with both pieces
#     selected (highlighted as two faces).
#   - A click on either piece takes just that piece out: the other piece
#     stays selected under its own key (no piece is added a second time).
#     Docked clicks the main piece (the one that carries the old key in
#     the model), which comes back when clicked again; Floating clicks the
#     other piece.
#   - The extrusion follows: OK extrudes only the pieces left, and the
#     saved definition lists them by their own keys.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-profile-split-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
# In the test library's own folders, which it removes at the end.
WORK=$(mktemp -d "$UI_CACHE/profile-split.XXXXXX")
FILE=$WORK/split.mitcad
ORIGINAL=$WORK/original.mitcad
# Sketch1 (F1): circles of 660 (c1) and 600 (c3) about the origin, a
# vertical bar 40 wide from y = -310 (lines c5..c8, its height d4). TIP is
# the bar's top between the inner circle and the bar's end (y = 310); with
# the bar 700 high (to y = 390) it is split by the outer circle into INNER
# (between the circles) and OUTER (outside them; the main piece, as the
# larger of two with as many curves in common). The extrusion goes down
# and the sketch is shown, so its regions can be clicked from the top.
TIP='r{c3[c6,c8],c6[c3,c7],c7[c6,c8],c8[c7,c3]}'
INNER='r{c1[c6,c8],c3[c6,c8],c6[c3,c1],c8[c1,c3]}'
OUTER='r{c1[c6,c8],c6[c1,c7],c7[c6,c8],c8[c7,c1]}'

cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 660},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 600},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-20, -310], "width": 40, "height": 620},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$TIP"}],
    "extent": {"type": "distance", "distance": -10}, "operation": "new_body"}},
  {"cmd": "set_feature_visible", "uid": "F1", "visible": true},
  {"expect": {"parameter": "d4", "value": 620}},
  {"expect": {"feature": "Extrude1", "status": "ok"}},
  {"cmd": "set_parameter", "name": "d4", "value": 700},
  {"expect": {"feature": "Extrude1", "status": "warning",
    "error_contains": "region $TIP found as $OUTER + $INNER after a change in Sketch1"}},
  {"expect": {"query": {"query": "profile", "sketch": "F1", "region": "$TIP"},
    "result": {"exact": false, "region": "$OUTER", "regions": ["$OUTER", "$INNER"]}}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
cp "$FILE" "$ORIGINAL"

# The pieces' areas: the bar |x| <= 20 between the circles, and above the
# outer circle up to y = 390 (the integral of sqrt(R^2 - x^2) over the bar).
strip() { awk -v r="$1" 'BEGIN { s = 20 / r; printf "%.6f", 20 * sqrt(r * r - 400) + r * r * atan2(s, sqrt(1 - s * s)) }'; }
INNER_VOLUME=$(awk -v a="$(strip 330)" -v b="$(strip 300)" 'BEGIN { printf "%.6f", 10 * (a - b) }')
OUTER_VOLUME=$(awk -v a="$(strip 330)" 'BEGIN { printf "%.6f", 10 * (40 * 390 - a) }')

# edit_extrude: opens Extrude1 for editing; its one reference shows both
# pieces selected.
edit_extrude() {
  ui_step "top view (search)"              ui_command "Top View"
  ui_step "fit (F6)"                       ui_key F6
  ui_mark
  ui_step "double-click Extrude1"          ui_double_click_logged "Timeline Extrude1"
  ui_expect_new "Editing F2 with Extrude" "editing Extrude1"
  ui_expect_new "Extrude Profiles: 1 profile [profile $TIP of F1]" "the split reference, once"
  ui_expect_new "View profile highlights: F1/$TIP 2 faces" "both pieces highlighted as selected"
  ui_mark
  ui_step "the Profiles input"             ui_click_logged "Panel Extrude input profiles"
  ui_expect_new "Pick profile F1/$OUTER" "the outer piece can be picked"
  ui_expect_new "Pick profile F1/$INNER" "the inner piece can be picked"
}

echo "=== Docked"
ui_start_display
ui_start_app --open "$FILE"
edit_extrude

echo "--- The main (outer) piece clicked off, back on, off again"
ui_mark
ui_step "click the outer piece"          ui_click_pick "profile F1/$OUTER"
ui_expect_new "Extrude Profiles: 1 profile [profile $INNER of F1]" "only the outer piece went"
ui_expect_new "View profile highlights: F1/$INNER 1 face" "the inner piece stays highlighted"
ui_expect_new "Preview Extrude: ok" "the inner piece previewed"
ui_mark
ui_step "click the outer piece again"    ui_click_pick "profile F1/$OUTER"
ui_expect_new "Extrude Profiles: 2 profiles [profile $INNER of F1; profile $OUTER of F1]" \
  "the outer piece back: the area selected before"
ui_mark
ui_step "click the outer piece once more" ui_click_pick "profile F1/$OUTER"
ui_expect_new "Extrude Profiles: 1 profile [profile $INNER of F1]" "the outer piece went again"
ui_expect_new "Preview Extrude: ok" "the inner piece previewed"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F2" "Extrude1 edited"
ui_expect_volume "Body Body1 (F2.b0): volume [0-9.]* -> \([0-9.]*\) mm3" "$INNER_VOLUME" \
  "only the inner piece extruded"
ui_mark
ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
saved=$(jq -c '[.features[] | select(.type == "extrude") | .profiles[].region]' "$FILE") ||
  ui_fail "cannot read $FILE"
[ "$saved" = "[\"$INNER\"]" ] || ui_fail "Extrude1's saved profiles are $saved"
echo "ok   Extrude1 lists the inner piece by its own key"
grep -q "is listed more than once" "$UI_LOG" && ui_fail "a profile was listed twice"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_stop_app

echo "=== Floating"
ui_start_app --chrome=floating --open "$ORIGINAL"
edit_extrude
ui_mark
ui_step "click the inner piece"          ui_click_pick "profile F1/$INNER"
ui_expect_new "Extrude Profiles: 1 profile [profile $OUTER of F1]" "only the inner piece went"
ui_expect_new "View profile highlights: F1/$OUTER 1 face" "the outer piece stays highlighted"
ui_expect_new "Preview Extrude: ok" "the outer piece previewed"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F2" "Extrude1 edited"
ui_expect_volume "Body Body1 (F2.b0): volume [0-9.]* -> \([0-9.]*\) mm3" "$OUTER_VOLUME" \
  "only the outer piece extruded"

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_finish "UI profile split test"
