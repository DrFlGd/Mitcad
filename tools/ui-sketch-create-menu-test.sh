#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework/Ribbon.cpp app/framework/Ribbon.hpp app/framework/Command.hpp
# check-all sources: app/commands/SketchCommands.cpp
# The sketch CREATE group in submenus (mitcad#222), through the real UI, in
# the Docked and the Floating layout:
#   - the CREATE menu: Line, Rectangles, Circles, Arcs, Other Geometry,
#     Text, Patterns, References (Project, Clean Up Projections, Clean Up
#     Stale Projections) and Sketch Dimension, every sketch creation
#     command exactly once; Offset (and Edit Text) in MODIFY, not in
#     CREATE;
#   - split buttons for Rectangles, Circles and Arcs, none for Other
#     Geometry and Patterns: the Circles button starts Center Diameter
#     Circle, its arrow opens the circle variants, which start from there;
#   - the keyboard: the CREATE menu's Circles submenu with the arrow keys,
#     the shortcut C and the command search;
#   - in a narrow window the groups stay within it;
#   - the SOLID CREATE menu has no submenus.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-sketch-create-menu-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

RECTANGLES="2-Point Rectangle | 3-Point Rectangle | Center Rectangle"
CIRCLES="Center Diameter Circle | 2-Point Circle | 3-Point Circle | 2-Tangent Circle | 3-Tangent Circle"
ARCS="3-Point Arc | Center Point Arc | Tangent Arc"
OTHER="Ellipse | Circumscribed Polygon | Inscribed Polygon | Edge Polygon | Center to Center Slot | Overall Slot"
OTHER="$OTHER | Center Point Slot | Three Point Arc Slot | Center Point Arc Slot | Fit Point Spline"
OTHER="$OTHER | Control Point Spline | Point"
PATTERNS="Rectangular Pattern | Circular Pattern | Mirror"
REFERENCES="Project | Clean Up Projections | Clean Up Stale Projections"
CREATE="Line | Rectangles > [$RECTANGLES] | Circles > [$CIRCLES] | Arcs > [$ARCS] | Other Geometry > [$OTHER]"
CREATE="$CREATE | Text | Patterns > [$PATTERNS] | References > [$REFERENCES] | Sketch Dimension"

# last_line "prefix": the last log line starting with the prefix, without it.
last_line() {
  grep -F -- "$1" "$UI_LOG" | tail -1 | awk -v p="$1" '{ print substr($0, index($0, p) + length(p)) }'
}

# expect_tool "name" "description": the tool started since the mark, then
# Esc ends it.
expect_tool() {
  ui_expect_new "Sketch tool $1" "$2"
  ui_step "end the tool (Esc)"             ui_key Escape
}

# within_window "description": every group of the SKETCH tab logged last
# is inside the window.
within_window() {
  local width group at x
  width=$(xdotool getwindowgeometry --shell "$UI_WINDOW" | sed -n 's/^WIDTH=//p')
  for group in CREATE MODIFY CONSTRAINTS "FINISH SKETCH"; do
    at=$(grep -F "Ribbon SKETCH/$group at " "$UI_LOG" | tail -1 | sed -n 's/.* at \([0-9]*\),[0-9]*$/\1/p')
    [ -n "$at" ] || ui_fail "$1: SKETCH/$group not logged"
    x=$((at * UI_SCALE))
    [ "$x" -gt 0 ] && [ "$x" -lt "$width" ] || ui_fail "$1: SKETCH/$group at x $x, the window is $width wide"
  done
  echo "ok   $1: the groups within the window ($width px)"
}

check_layout() {
  local layout=$1 modify
  echo "=== $layout"
  ui_mark
  ui_create_sketch
  ui_expect_new "Ribbon SKETCH/CREATE at" "$layout: the CREATE group"
  for subgroup in Rectangles Circles Arcs; do
    ui_expect_new "Ribbon split SKETCH/CREATE/$subgroup at" "$layout: a $subgroup split button"
    ui_expect_new "Ribbon split SKETCH/CREATE/$subgroup arrow at" "$layout: its arrow"
  done
  for subgroup in "Other Geometry" Patterns Lines Text References; do
    tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qF "Ribbon split SKETCH/CREATE/$subgroup " &&
      ui_fail "$layout: a split button for $subgroup"
  done
  echo "ok   $layout: no split buttons for the groups without a default"

  echo "--- The CREATE menu"
  ui_mark
  ui_step "open the CREATE menu"           ui_click_logged "Ribbon SKETCH/CREATE"
  ui_expect_new "Ribbon menu SKETCH/CREATE: " "$layout: the CREATE menu opened"
  [ "$(last_line "Ribbon menu SKETCH/CREATE: ")" = "$CREATE" ] ||
    ui_fail "$layout: the CREATE menu is '$(last_line "Ribbon menu SKETCH/CREATE: ")'"
  echo "ok   $layout: the CREATE menu has every tool once, in its groups"
  # The keyboard: the third entry (Circles), its submenu, its second entry.
  ui_step "Circles > 2-Point Circle"       ui_key Down Down Down Right Down Return
  expect_tool "2-Point Circle" "$layout: 2-Point Circle from the menu by the keyboard"

  echo "--- Offset in MODIFY"
  ui_mark
  ui_step "open the MODIFY menu"           ui_click_logged "Ribbon SKETCH/MODIFY"
  ui_expect_new "Ribbon menu SKETCH/MODIFY: " "$layout: the MODIFY menu opened"
  ui_step "close it (Esc)"                 ui_key Escape
  modify=$(last_line "Ribbon menu SKETCH/MODIFY: ")
  case " | $modify | " in
  *" | Offset | "*) echo "ok   $layout: Offset in MODIFY" ;;
  *) ui_fail "$layout: no Offset in MODIFY: '$modify'" ;;
  esac
  case " | $modify | " in
  *" | Edit Text | "*) echo "ok   $layout: Edit Text in MODIFY" ;;
  *) ui_fail "$layout: no Edit Text in MODIFY: '$modify'" ;;
  esac

  echo "--- The Circles split button"
  ui_mark
  ui_step "click the Circles button"       ui_click_logged "Ribbon split SKETCH/CREATE/Circles"
  expect_tool "Center Diameter Circle" "$layout: the button starts Center Diameter Circle"
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qF "Ribbon menu SKETCH/CREATE/Circles: " &&
    ui_fail "$layout: the button opened the menu"
  ui_mark
  ui_step "click its arrow"                ui_click_logged "Ribbon split SKETCH/CREATE/Circles arrow"
  ui_expect_new "Ribbon menu SKETCH/CREATE/Circles: $CIRCLES" "$layout: the arrow shows the circle variants"
  ui_step "choose 3-Point Circle"          ui_key Down Down Down Return
  expect_tool "3-Point Circle" "$layout: a variant from the arrow's menu"
  ui_mark
  ui_step "the Arcs arrow"                 ui_click_logged "Ribbon split SKETCH/CREATE/Arcs arrow"
  ui_expect_new "Ribbon menu SKETCH/CREATE/Arcs: $ARCS" "$layout: the arc variants"
  ui_step "close it (Esc)"                 ui_key Escape

  echo "--- Shortcut and search"
  ui_mark
  ui_step "C"                              ui_key c
  expect_tool "Center Diameter Circle" "$layout: the shortcut C"
  ui_mark
  ui_step "search 3-Point Arc"             ui_command "3-Point Arc"
  expect_tool "3-Point Arc" "$layout: the command search"
  ui_mark
  ui_step "search Offset"                  ui_command "Offset"
  ui_expect_new "Command Offset started" "$layout: Offset from the search"
  ui_step "cancel (Esc)"                   ui_key Escape
  ui_expect_new "Command Offset cancelled" "$layout: Offset cancelled"
  ui_mark
  ui_step "finish the sketch (Ctrl+Enter)" ui_key ctrl+Return
  ui_expect_new "Sketch finished" "$layout: sketch left"
}

# narrow_window: the Floating layout's groups in a narrow window, the ones
# at the right as their names only (Ribbon::fitGroups), all within it.
narrow_window() {
  echo "--- A narrow window"
  ui_step "a narrow window"                xdotool windowsize "$UI_WINDOW" 900 800
  ui_mark
  ui_create_sketch
  ui_expect_new "Ribbon SKETCH/FINISH SKETCH at" "the groups logged again"
  within_window "Floating, narrow"
  ui_mark
  ui_step "open the CREATE menu"           ui_click_logged "Ribbon SKETCH/CREATE"
  ui_expect_new "Ribbon menu SKETCH/CREATE: $CREATE" "the CREATE menu in a narrow window"
  ui_step "close it (Esc)"                 ui_key Escape
}

ui_start_display
ui_start_app

echo "--- The SOLID CREATE menu"
ui_mark
ui_step "open the SOLID CREATE menu"       ui_click_logged "Ribbon SOLID/CREATE"
ui_expect_new "Ribbon menu SOLID/CREATE: " "the SOLID CREATE menu opened"
ui_step "close it (Esc)"                   ui_key Escape
case "$(last_line "Ribbon menu SOLID/CREATE: ")" in
*" > ["*) ui_fail "the SOLID CREATE menu has submenus" ;;
*"Create Sketch | Extrude"*) echo "ok   the SOLID CREATE menu as before" ;;
*) ui_fail "the SOLID CREATE menu: '$(last_line "Ribbon menu SOLID/CREATE: ")'" ;;
esac

check_layout "Docked"
ui_stop_app
ui_start_app --chrome=floating
check_layout "Floating"
narrow_window

ui_finish "UI sketch create menu test"
