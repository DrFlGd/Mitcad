#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindow.cpp app/browser app/framework tools/cli
# The Docked layout's window height (mitcad#560), through the real UI, on a
# design mitcad-cli makes: a 10 mm cube and a rectangular pattern of it,
# 6 x 5 (30 bodies: Body1 F2.b0, the copies F3.b0 to F3.b28):
#   - The design opens folded (its rows do not fit the Browser); the
#     window's size, the view's area and the timeline's cell are noted.
#   - Root/Bodies opened: the 30 body rows scroll (Browser scroll range
#     above 0); the window keeps its size ("Window layout:", which the
#     window logs again whenever its size changes), the view its area, the
#     timeline's cell its place.
#   - A click on the timeline's RectangularPattern1 selects its 29 bodies:
#     the command panel lists the first 8 with their measurements and
#     scrolls them; the window, the view and the cell stay where they were
#     (the window grew taller than the screen before).
#   - A plain double-click on the cell edits the pattern.
# The Windows counterpart is tools/ui-windows-docked-height-test.ps1.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-docked-height-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-docked-height.XXXXXX)
FILE=$WORK/docked-height.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

cat > "$WORK/model.json" << 'JSON'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 10, "height": 10},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "rectangular_pattern",
    "objects": {"type": "bodies", "bodies": ["F2.b0"]},
    "direction1": {"axis": {"type": "origin", "axis": "x"}, "quantity": 6, "distance": 20},
    "direction2": {"axis": {"type": "origin", "axis": "y"}, "quantity": 5, "distance": 20},
    "distance_type": "spacing"}},
  {"expect": {"bodies": 30}}
]
JSON
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# window_size: "WxH" of the last "Window layout:" line.
window_size() { sed -n 's/^Window layout: window \([0-9]*x[0-9]*\),.*/\1/p' "$UI_LOG" | tail -1; }
# view_area: "x y w h" of the last "View area" line.
view_area() { sed -n 's/^View area \([0-9]* [0-9]* [0-9]* [0-9]*\)$/\1/p' "$UI_LOG" | tail -1; }
# cell_at: the last logged place of the pattern's timeline cell.
cell_at() { sed -n 's/^Timeline RectangularPattern1 at \([0-9]*,[0-9]*\)$/\1/p' "$UI_LOG" | tail -1; }
# scroll_range: the Browser rows' last logged vertical scroll range.
scroll_range() { grep -o '^Browser scroll range [0-9]*$' "$UI_LOG" | tail -1 | grep -o '[0-9]*$'; }

# expect_unmoved description: once the app has settled, the window has the
# size, the view the area and the timeline's cell the place noted at the
# start.
expect_unmoved() {
  ui_wait_idle
  sleep 0.5
  ui_sync
  [ "$(window_size)" = "$SIZE0" ] || ui_fail "$1: the window is $(window_size), not $SIZE0"
  [ "$(view_area)" = "$VIEW0" ] || ui_fail "$1: the view area is $(view_area), not $VIEW0"
  [ "$(cell_at)" = "$CELL0" ] || ui_fail "$1: the timeline's cell is at $(cell_at), not $CELL0"
  echo "ok   $1: the window $SIZE0, the view $VIEW0, the cell at $CELL0"
}

# expect_scrolls description: the Browser's rows overflow and scroll.
expect_scrolls() {
  for _ in $(seq 1 25); do
    [ "$(scroll_range)" -gt 0 ] && break
    sleep 0.2
  done
  [ "$(scroll_range)" -gt 0 ] || ui_fail "$1: the Browser's rows do not scroll"
  echo "ok   $1 (Browser scroll range $(scroll_range))"
}

ui_start_display

echo "--- The design opens folded"
ui_start_app --open "$FILE"
ui_expect_log "Opened $FILE" "the design opened"
ui_expect_log "Window layout: window " "the window logged its layout"
ui_expect_log "Browser opened folded: " "the 30 bodies do not fit: the design opens folded"
ui_expect_log "Browser arrow Root/Bodies at" "the Bodies folder's arrow"
ui_expect_log "Timeline RectangularPattern1 at" "the pattern's cell"
ui_wait_idle
ui_sync
SIZE0=$(window_size)
VIEW0=$(view_area)
CELL0=$(cell_at)
echo "ok   at the start: the window $SIZE0, the view $VIEW0, the cell at $CELL0"

echo "--- Root/Bodies opened: the rows scroll, the window keeps its size"
ui_mark
ui_step "open Bodies"                    ui_click_logged "Browser arrow Root/Bodies"
ui_expect_new "Browser Root/Bodies/Body1 at" "Body1's row"
expect_scrolls "the 30 body rows scroll"
expect_unmoved "Bodies open"

echo "--- The pattern's 29 bodies selected from the timeline"
ui_mark
ui_step "click RectangularPattern1"      ui_click_logged "Timeline RectangularPattern1"
ui_expect_new "Selected: 29 bodies" "the pattern's bodies selected"
expect_scrolls "the rows still scroll"
expect_unmoved "the bodies selected"

echo "--- A plain double-click edits the pattern"
sleep 0.45 # not a double-click with the click before
ui_mark
ui_step "double-click RectangularPattern1" ui_double_click_logged "Timeline RectangularPattern1"
ui_expect_new "Editing F3 with Rectangular Pattern" "editing the pattern"
ui_expect_new "Preview Rectangular Pattern: ok" "previewed" 30
ui_mark
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command Rectangular Pattern cancelled" "left as it was"
ui_wait_idle
ui_sync
[ "$(window_size)" = "$SIZE0" ] || ui_fail "after the edit the window is $(window_size), not $SIZE0"
echo "ok   after the edit the window is still $SIZE0"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI docked height test"
