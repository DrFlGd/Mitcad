#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/commands app/MainWindow.cpp app/framework core/model/src/sketch core/model/src/api core/solver tools/cli
# Equal spacing in sketches (mitcad#221) through the real UI, on a sketch
# mitcad-cli makes: a fixed point p1 at (-20, 0), a circle c2 (centre
# (45, 30), radius 10), a line c4 from (100, -10) to (120, 10) (its
# midpoint at (110, 0)), an arc c7 (centre (170, 20), a quarter turn, its
# radius dimensioned 12: k1) and a point p11 at (250, -10): x intervals
# 65, 65, 60 and 80.
#   - The five selected (mixed objects), Sketch Dimension (D) opens Equal
#     Spacing Dimension: horizontal by their spread, the mean interval 67.5
#     to type over; 60 and OK give one constraint (k2) and one dimension
#     (k3) in the order selected, one undo step (undone, redone); saved,
#     and read back by mitcad-cli: the chain's order, its mode and its
#     common spacing.
#   - A double-click on the spacing's symbol opens Edit Equal Spacing:
#     Point to Point keeps the stored order and the 60; OK, then undone.
#   - A right click on the symbol (another object selected) selects it;
#     the context menu's Edit Equal Spacing opens the same panel; cancelled.
#   - Equal Spacing on three of them from the command search: refused
#     before OK as implied by the others; Point to Point asks to confirm
#     the selection's order as the chain's, then previews; cancelled.
#   - Deleting objects takes them out of the chain, the rest in its order;
#     with two left the constraint and its dimension go.
#
# Clicks are given in sketch millimetres (ui_sketch_click), from the
# placement the app logs.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-equal-spacing-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-spacing.XXXXXX)
FILE=$WORK/spacing.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

cat > "$WORK/sketch.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [-20, 0], "fixed": true},
  {"cmd": "sketch.circle", "sketch": "F1", "mode": "center", "center": [45, 30], "radius": 10},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [100, -10], "end": [120, 10]},
  {"cmd": "sketch.arc", "sketch": "F1", "mode": "center", "center": [170, 20], "start": [182, 20],
   "end": [170, 40]},
  {"cmd": "sketch.add_dimension", "sketch": "F1", "dimension": {"type": "radius", "curve": "c7"},
   "value": 12},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [250, -10]},
  {"expect": {"query": {"query": "sketch", "uid": "F1"},
    "result": {"constraints": [], "dimensions": [{"id": "k1", "type": "radius", "measured": 12}]}}}
]
EOF
"$CLI" run "$WORK/sketch.json" --save "$FILE" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ANCHORS="p1, c2, c4, c7, p11"

# check "description" expectations...: the saved design opened by mitcad-cli.
check() {
  local description=$1; shift
  local IFS=,
  echo "[$*]" > "$WORK/check.json"
  "$CLI" run "$WORK/check.json" --open "$FILE" > "$WORK/check.log" 2>&1 ||
    { cat "$WORK/check.log"; ui_fail "$description"; }
  echo "ok   $description"
}

# zoom_to_show x1 y1 x2 y2: zooms out (the wheel at the view's middle)
# until the sketch rectangle from (x1, y1) to (x2, y2) mm is in the view.
zoom_to_show() {
  local area vx vy vw vh ax ay bx by
  for _ in $(seq 1 20); do
    area=$(grep -o "View area [0-9]* [0-9]* [0-9]* [0-9]*" "$UI_LOG" | tail -1)
    read -r _ _ vx vy vw vh <<< "$area"
    read -r ax ay <<< "$(ui_sketch_at "$1" "$2")"
    read -r bx by <<< "$(ui_sketch_at "$3" "$4")"
    if [ "$ax" -gt $((X + vx + 20)) ] && [ "$bx" -lt $((X + vx + vw - 20)) ] &&
      [ "$by" -gt $((Y + vy + 20)) ] && [ "$ay" -lt $((Y + vy + vh - 20)) ]; then
      return 0
    fi
    xdotool mousemove $(ui_view_at 50 50) click 5
    ui_sync
  done
  ui_fail "the view did not zoom out to show ($1, $2) to ($3, $4)"
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
}

# select_curve_of "c7" x y [x y ...]: clicks on the curve at the sketch points
# in turn until the app logs exactly that curve selected (a constraint symbol
# or a dimension's text drawn over the first point takes the click; it is
# not a failure of the curve); a failure shows what was selected last.
select_curve_of() {
  local curve=$1 x y
  shift
  while [ "$#" -ge 2 ]; do
    x=$1 y=$2
    shift 2
    ui_mark
    select_at "$x" "$y"
    for _ in $(seq 1 15); do
      tail -n +$((UI_MARK + 1)) "$UI_LOG" |
        grep -qF -- "Selected: 1 sketch curve [sketch curve $curve of F1]" && { echo "ok   $curve selected"; return; }
      sleep 0.2
    done
    echo "note: $curve not selected by the click at $x, $y: $(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "Selected: " | tail -1)"
  done
  ui_fail "$curve could not be selected"
}

# glyph_at x y: the screen position of the first constraint symbol of the
# sketch point at (x, y) mm (12 pixels right of it and up, SketchOverlay).
glyph_at() {
  local sx sy
  read -r sx sy <<< "$(ui_sketch_at "$1" "$2")"
  awk -v x="$sx" -v y="$sy" -v s="$UI_SCALE" 'BEGIN { printf "%d %d\n", x + 12 * s + 0.5, y - 12 * s + 0.5 }'
}

# double_click_expect_at x y "log text" "what": a double click at a screen
# position that must log the text; under load the two clicks may come as
# two single clicks, so it is tried once more.
double_click_expect_at() {
  local attempt
  for attempt in 1 2; do
    ui_mark
    ui_wait_idle
    xdotool mousemove "$1" "$2" click --repeat 2 --delay 90 1
    ui_sync
    ui_crashed && ui_fail "crashed during the double-click at $1,$2"
    for _ in $(seq 1 15); do
      tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qF -- "$3" && { echo "ok   $4"; return; }
      sleep 0.2
    done
    echo "note: no '$3' after the double-click at $1,$2 (attempt $attempt)"
    sleep 0.45 # not a double-click with the clicks before
  done
  ui_fail "$4: '$3' not in the log after two double-clicks"
}

# edit_sketch: Sketch1 edited from the timeline, all of it in the view.
# Entering the sketch puts its origin in the middle of the view (only
# sketches shown, the origin in sight; mitcad#738), which leaves most of
# the sketch to the right; the fit brings all of it to the middle, so that
# the zoom keeps the arc large enough to pick it beside its end points.
edit_sketch() {
  ui_mark
  ui_step "double-click Sketch1"           ui_double_click_logged "Timeline Sketch1"
  ui_expect_new "Editing sketch F1 (Sketch1)" "Sketch1 edited"
  ui_sync
  ui_step "fit (F6)"                       ui_key F6
  ui_sync
  ui_step "zoom out"                       zoom_to_show -30 -30 260 50
}

ui_start_display
ui_start_app --open "$FILE"
edit_sketch

echo "--- Five objects selected, then Sketch Dimension: Equal Spacing Dimension"
ui_mark
ui_step "select the fixed point"           select_at -20 0
ui_step "and the circle (Ctrl)"            select_at 55 30 ctrl
ui_step "and the line (Ctrl)"              select_at 110 0 ctrl
ui_step "and the arc (Ctrl)"               select_at 180.39 26 ctrl
ui_step "and the last point (Ctrl)"        select_at 250 -10 ctrl
ui_expect_new "Selected: 3 sketch curves, 2 sketch points" "three curves and two points selected"
ui_mark
ui_step "sketch dimension (D)"             ui_key d
ui_expect_new "Sketch Dimension on 5 objects: Equal Spacing Dimension" "Dimension offers their equal spacing"
ui_expect_new "Command Equal Spacing Dimension started" "its panel"
ui_expect_new "Equal spacing preview: $ANCHORS" "the objects in the order selected"
ui_expect_new "Equal spacing k2 horizontal: $ANCHORS (spacing k3 = 67.50)" \
  "horizontal by their spread, the mean interval previewed"
ui_expect_new "Preview Equal Spacing Dimension: ok" "previewed"
ui_mark
ui_step "spacing 60"                       ui_type_in "Panel Equal Spacing Dimension input spacing" "60"
ui_expect_new "Equal spacing k2 horizontal: $ANCHORS (spacing k3 = 60.00)" "every interval 60"
ui_expect_new "Preview Equal Spacing Dimension: ok" "previewed"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Equal spacing dimension of 5 object(s), horizontal, spacing 60 mm" "the constraint and its dimension"
ui_expect_new "Added Equal Spacing Dimension" "added"
ui_mark
ui_step "undo (Ctrl+Z)"                    ui_key ctrl+z
ui_expect_new "Undo: " "undone"
ui_expect_new "Equal spacing k2 removed" "the constraint and its dimension undone as one step"
ui_mark
ui_step "redo (Ctrl+Y)"                    ui_key ctrl+y
ui_expect_new "Equal spacing k2 horizontal: $ANCHORS (spacing k3 = 60.00)" "redone"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"       ui_key ctrl+Return
ui_expect_new "Sketch finished" "sketch finished"
ui_mark
ui_step "save (Ctrl+S)"                    ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
check "the file keeps the chain's order, its mode and its one common spacing" \
  '{"expect": {"query": {"query": "sketch", "uid": "F1"},
     "result": {"constraints": [{"type": "equal_spacing", "anchors": ["p1", "c2", "c4", "c7", "p11"],
                                 "mode": "horizontal"}],
                "dimensions": [{"id": "k1", "type": "radius", "measured": 12},
                               {"id": "k3", "type": "spacing", "constraint": "k2", "measured": 60}]}}}'

echo "--- A double-click on its symbol: Edit Equal Spacing, Point to Point"
edit_sketch
# p11 is at (220, -10) now: -20 + 4 x 60.
read -r gx gy <<< "$(glyph_at 220 -10)"
double_click_expect_at "$gx" "$gy" "Double-click on k2: sketch.edit_equal_spacing" \
  "the symbol opens Edit Equal Spacing"
ui_expect_log "Panel Edit Equal Spacing input mode at" "its panel, filled from the constraint"
ui_mark
ui_step "Point to Point"                   ui_choose "Panel Edit Equal Spacing input mode" 3
ui_expect_new "Equal spacing k2 point_to_point: $ANCHORS (spacing k3 = 60.00)" \
  "a chain in the stored order, its links 60"
ui_expect_new "Preview Edit Equal Spacing: ok" "previewed"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited equal spacing k2 of 5 object(s), point_to_point, spacing 60 mm" "edited"
ui_mark
ui_step "undo (Ctrl+Z)"                    ui_key ctrl+z
ui_expect_new "Equal spacing k2 horizontal: $ANCHORS (spacing k3 = 60.00)" "the edit is one undo step"

echo "--- A right click on its symbol: Edit Equal Spacing in the context menu"
ui_mark
ui_step "select the fixed point"           select_at -20 0
ui_expect_new "Selected: 1 sketch point" "something else selected"
ui_mark
ui_step "right-click the symbol"           eval 'xdotool mousemove $gx $gy click 3'
ui_expect_new "Context menu: " "the context menu opened"
grep "Context menu: " "$UI_LOG" | tail -1 | grep -qE "(:|\|) Edit Equal Spacing( \||$)" ||
  ui_fail "no Edit Equal Spacing in the context menu: $(grep "Context menu: " "$UI_LOG" | tail -1)"
echo "ok   the menu offers Edit Equal Spacing for the symbol right-clicked"
ui_step "edit equal spacing"               ui_menu_choose "Edit Equal Spacing"
ui_expect_new "Command Edit Equal Spacing started" "its panel"
ui_expect_new "Panel Edit Equal Spacing input mode at" "filled from the constraint"
ui_expect_new "Preview Edit Equal Spacing: ok" "previewed as it is"
ui_mark
ui_step "cancel (Esc)"                     ui_key Escape
ui_expect_new "Command Edit Equal Spacing cancelled" "cancelled"
! tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qF "Edited equal spacing" || ui_fail "the cancelled panel edited"
echo "ok   nothing edited"

echo "--- Equal Spacing on three of them: redundant; a chain's order confirmed"
ui_mark
ui_step "select the fixed point"           select_at -20 0
ui_step "and the circle (Ctrl)"            select_at 50 30 ctrl
ui_step "and the line (Ctrl)"              select_at 100 0 ctrl
ui_expect_new "Selected: 2 sketch curves, 1 sketch point" "three objects selected"
ui_mark
ui_step "equal spacing (search)"           ui_command "Equal Spacing"
ui_expect_new "Command Equal Spacing started" "Equal Spacing with the selection"
ui_expect_new "Preview Equal Spacing: failed:" "refused before OK"
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "Preview Equal Spacing: failed:" | tail -1 | grep -q "over-constrain" ||
  ui_fail "not refused as implied by the others"
echo "ok   refused as implied by the others"
ui_mark
ui_step "Point to Point"                   ui_choose "Panel Equal Spacing input mode" 3
ui_expect_new "Preview Equal Spacing: not built: The objects are chained in the listed order" \
  "a selection's order is confirmed before it is a chain"
ui_expect_new "Panel Equal Spacing input order_confirmed at" "the confirmation shown"
# The longer message moves the rows: their places are logged again with it.
ui_expect_new "Panel Equal Spacing message " "the message shown"
ui_sync
ui_mark
ui_step "confirm the listed order"         ui_click_logged "Panel Equal Spacing input order_confirmed"
ui_expect_new "Equal spacing k4 point_to_point: p1, c2, c4" "the chain as listed"
ui_expect_new "Preview Equal Spacing: ok" "previewed"
ui_mark
ui_step "cancel (Esc)"                     ui_key Escape
ui_expect_new "Equal spacing k4 removed" "nothing added"

echo "--- Deleting objects takes them out of the chain"
ui_mark
ui_step "select the last point"            select_at 220 -10
ui_step "delete (Delete)"                  ui_key Delete
ui_expect_new "Equal spacing k2 horizontal: p1, c2, c4, c7 (spacing k3 = 60.00)" \
  "the point left the chain, the others keep their order"
# The arc's centre is at (160, 20) now, its radius 12: 30 degrees round it.
ui_step "select the arc"                   select_curve_of c7 170.39 26 166 30.39 171.59 23.11
ui_mark
ui_step "delete (Delete)"                  ui_key Delete
ui_expect_new "Equal spacing k2 horizontal: p1, c2, c4 (spacing k3 = 60.00)" "three left"
ui_step "select the line"                  select_curve_of c4 100 0 95 -5 105 5
ui_mark
ui_step "delete (Delete)"                  ui_key Delete
ui_expect_new "Equal spacing k2 removed" "two left: the constraint and its dimension go"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"       ui_key ctrl+Return
ui_expect_new "Sketch finished" "sketch finished"

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_finish "UI equal spacing test"
