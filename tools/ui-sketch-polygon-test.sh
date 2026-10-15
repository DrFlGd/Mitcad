#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/commands/SketchCommands.cpp tools/cli
# check-all sources: core/model/src/sketch core/model/src/api/sketch.rs core/solver
# Regular polygons stay regular (mitcad#303), through the real UI, on a
# design mitcad-cli makes with an empty Sketch1 (F1) on XY:
#   - Circumscribed Polygon: a hexagon from the origin (its centre joins
#     the origin) to a side's middle at (20, 0): two degrees of freedom
#     left, the orientation and the size (a hexagon whose sides only touch
#     one circle had three: it could skew).
#   - Move/Copy, Rotate, of the six sides by 90 degrees about the origin.
#   - A drag of a corner around the centre: the hexagon turns (and
#     follows the pointer's distance) without skewing.
#   - Saved, and checked with mitcad-cli: every corner where a regular
#     hexagon about the origin through the dragged corner has it.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-sketch-polygon-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-sketch-polygon.XXXXXX)
FILE=$WORK/polygon.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "sketch.create"}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# dof: the degrees of freedom of Sketch1 the app logged last.
dof() { sed -n 's/^Sketch Sketch1: \([0-9]*\) DOF.*/\1/p' "$UI_LOG" | tail -1; }

expect_dof() {
  local now
  ui_sync
  now=$(dof)
  [ "$now" = "$1" ] || ui_fail "$2: $now DOF, expected $1"
  echo "ok   $2 ($1 DOF)"
}

# select_at x y [ctrl]: a click on sketch geometry at sketch mm.
select_at() {
  if [ "${3:-}" = ctrl ]; then
    xdotool keydown ctrl
    ui_sketch_click "$1" "$2"
    xdotool keyup ctrl
  else
    ui_sketch_click "$1" "$2"
  fi
  ui_sync
}

# polar r degrees: the sketch point at that distance and angle.
polar() { awk -v r="$1" -v a="$2" 'BEGIN { t = a * atan2(0, -1) / 180; printf "%.3f %.3f\n", r * cos(t), r * sin(t) }'; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Timeline Sketch1 at" "the timeline is logged"
ui_step "edit Sketch1 (double-click)"    ui_double_click_logged "Timeline Sketch1"
ui_expect_log "Editing sketch F1 (Sketch1)" "Sketch1 is edited"

echo "--- A circumscribed hexagon from the origin"
ui_mark
ui_step "polygon (search)"               ui_command "Circumscribed Polygon"
ui_step "polygon: centre at the origin"  ui_sketch_click 0 0
ui_step "polygon: a side's middle"       ui_sketch_click 20 0
ui_expect_new "Added polygon (6 sides)" "hexagon"
ui_step "end the tool (Esc)"             ui_key Escape
expect_dof 2 "only its orientation and size are free"

echo "--- Rotate: Move/Copy of the six sides by 90 degrees about the origin"
# The sides' middles: 20 from the centre at 0, 60, ..., 300 degrees.
read -r sx sy <<< "$(polar 20 0)"
ui_step "select a side"                  select_at "$sx" "$sy"
for a in 60 120 180 240 300; do
  read -r sx sy <<< "$(polar 20 "$a")"
  ui_step "and the side at $a deg (Ctrl)" select_at "$sx" "$sy" ctrl
done
ui_mark
ui_step "move/copy (search)"             ui_command "Move/Copy"
ui_expect_new "Command Move/Copy started" "Move/Copy started"
ui_expect_new "Panel Move/Copy input type at" "the panel is laid out"
ui_step "Rotate"                         ui_choose "Panel Move/Copy input type" 1
ui_expect_new "Preview Move/Copy: ok" "previewed"
ui_step "OK (Enter)"                     ui_key Return
ui_wait_idle
grep -q "Command Move/Copy refused" "$UI_LOG" && ui_fail "Rotate was refused"
expect_dof 2 "still two degrees of freedom"

echo "--- A drag of a corner around the centre"
# The corner at 30 degrees (23.094 from the centre) is now at 120.
R=23.094
read -r fx fy <<< "$(polar "$R" 120)"
read -r tx ty <<< "$(polar "$R" 145)"
ui_mark
ui_step "drag the corner"                ui_sketch_drag "$fx" "$fy" "$tx" "$ty"
ui_sync
line=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -E "^Dragged p[0-9]+ to " | tail -1)
[ -n "$line" ] || ui_fail "the corner did not drag"
read -r corner dx dy <<< "$(sed -n 's/^Dragged \(p[0-9]*\) to (\([-0-9.]*\), \([-0-9.]*\)).*/\1 \2 \3/p' <<< "$line")"
echo "ok   dragged $corner to ($dx, $dy)"
[ "$corner" = p8 ] || ui_fail "dragged $corner, expected the hexagon's second corner p8"
expect_dof 2 "the drag kept the constraints"

ui_mark
ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"

echo "--- The corners of a regular hexagon through the dragged corner"
# The corners p7 ... p12 are the sketch's 7th to 12th entities (after the
# six sides; the centre, the two construction circles and the point the
# centre joined at the origin follow); p8 is the dragged one.
awk -v x="$dx" -v y="$dy" 'BEGIN {
  pi = atan2(0, -1); r = sqrt(x * x + y * y); t = atan2(y, x)
  printf "[{\"expect\": {\"query\": {\"query\": \"sketch\", \"uid\": \"F1\"}, \"tolerance\": 0.002,\n"
  printf "  \"result\": {\"dof\": 2, \"entities\": [{}, {}, {}, {}, {}, {}"
  for (k = 0; k < 6; ++k) {
    a = t + (k - 1) * pi / 3
    printf ",\n    {\"id\": \"p%d\", \"at\": [%.4f, %.4f]}", 7 + k, r * cos(a), r * sin(a)
  }
  printf ", {}, {}, {}, {}]}}}]\n"
}' > "$WORK/check.json"
"$CLI" run "$WORK/check.json" --open "$FILE" > "$WORK/check.log" 2>&1 ||
  { cat "$WORK/check.json" "$WORK/check.log"; ui_fail "the hexagon is not regular"; }
echo "ok   the hexagon is regular after Rotate and the drag"

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_finish "UI sketch polygon test"
