#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch/SketchPalette.cpp app/sketch/SketchPalette.hpp app/MainWindow.cpp
# check-all sources: app/framework/FloatingLayout.cpp app/framework/FloatingLayout.hpp app/framework/GlassCard.cpp
# check-all sources: core/sketch core/model-api/src/sketch.rs tools/cli
# The Sketch Palette in the Floating layout's command card at the default
# window size, 1280 x 800 (mitcad#470), through the real UI:
#   - A sketch with a rectangle (mitcad-cli), edited from the timeline.
#   - The window is 1280 x 800 and the command card is placed.
#   - The palette's rows (the Options and Finish Sketch) keep at least
#     their size hint and none overlaps the next: the palette sits in a
#     scroll area that scrolls when the card is shorter than the rows (the
#     stacked page was squeezed below its hint before).
#   - The wheel over the palette scrolls it to the end (when the card is
#     shorter than the rows): then the last row, Finish Sketch, is within
#     the card.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-palette-floating-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-palette-floating.XXXXXX)
FILE=$WORK/sketch.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# check_rows description: the palette rows logged since the mark (waiting
# for them): each at least as high as its size hint, in the order of their
# places none overlapping the next, and the palette itself not below its
# minimum hint when it is not scrolling.
check_rows() {
  local rows="" count=0 name y h hh previous="" previous_bottom=-1 palette
  local -a part
  for _ in $(seq 1 40); do
    rows=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -E '^Palette row ' | tail -n 9)
    count=$(grep -c . <<< "$rows")
    [ "$count" -ge 9 ] && break
    sleep 0.2
  done
  [ "$count" -ge 9 ] || ui_fail "$1: $count Palette row lines (wanted 9): $rows"
  # "Palette row NAME at X,Y size WxH, hint WxH": the name, the top y and
  # the height of each, to sort by place.
  local places=""
  while IFS=' ' read -r -a part; do
    name=${part[2]}
    y=${part[4]#*,}
    h=${part[6]#*x}
    h=${h%,}
    hh=${part[8]#*x}
    echo "     $name: y $y, height $h (hint $hh)"
    [ "$h" -ge "$hh" ] || ui_fail "$1: $name is $h high, below its size hint $hh"
    places+="$y $h $name"$'\n'
  done <<< "$rows"
  while read -r y h name; do
    [ -n "$name" ] || continue
    if [ -n "$previous" ] && [ "$y" -lt "$previous_bottom" ]; then
      ui_fail "$1: $name (y $y) overlaps $previous (bottom $previous_bottom)"
    fi
    previous=$name
    previous_bottom=$((y + h))
  done < <(sort -n <<< "$places")
  echo "ok   $1: the rows keep their size hint and do not overlap"
  palette=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -E '^Palette size ' | tail -n 1)
  echo "     $palette"
}

ui_start_display
ui_start_app --chrome=floating --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Timeline Sketch1 at" "the timeline is logged"
ui_expect_log "Floating cards: window 1280x800" "the window is 1280 x 800 in the Floating layout"

echo "--- The palette in the command card"
ui_mark
ui_double_click_expect "Timeline Sketch1" "Editing sketch F1 (Sketch1)" "Sketch1 is edited"
ui_expect_new "Palette size " "the palette's rows are logged"
ui_sync
card=$(grep -E '^Floating cards: ' "$UI_LOG" | tail -n 1 | grep -oE 'commandDock [0-9]+,[0-9]+ [0-9]+x[0-9]+')
[ -n "$card" ] || ui_fail "no command card in the Floating cards line"
echo "     $card"
check_rows "the palette at 1280x800"

echo "--- Scrolled to the end"
ui_mark
ui_step "wheel down over the palette"    xdotool mousemove $(ui_logged_at "Palette paletteGrid") click --repeat 15 --delay 40 5
ui_sync
scrolled=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -E '^Palette scrolled to ' | tail -n 1)
if [ -n "$scrolled" ]; then
  echo "     $scrolled"
  read -r _ _ _ value _ maximum _ <<< "$scrolled"
  [ "$value" = "$maximum" ] || ui_fail "the palette did not scroll to its end: $scrolled"
  echo "ok   the palette scrolled to its end"
else
  echo "ok   the palette fits the card (nothing to scroll)"
fi
finish=$(grep -E '^Palette row paletteFinish at ' "$UI_LOG" | tail -n 1)
[ -n "$finish" ] || ui_fail "no Palette row paletteFinish in the log"
echo "     $finish"
read -r _ _ _ _ at _ size _ <<< "$finish"
fy=${at#*,}
fh=${size#*x}
fh=${fh%,}
read -r _ cxy csize <<< "$card"
cy=${cxy#*,}
ch=${csize#*x}
[ "$fy" -ge "$cy" ] && [ $((fy + fh)) -le $((cy + ch)) ] ||
  ui_fail "Finish Sketch (y $fy, height $fh) is not within the command card (y $cy, height $ch)"
echo "ok   Finish Sketch, the last row, is within the command card"
echo "PASS: UI palette floating test"
