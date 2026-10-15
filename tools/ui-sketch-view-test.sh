#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/OcctViewer.cpp app/OcctViewer.hpp app/view app/MainWindow.cpp tools/cli
# How sketch mode shows its geometry, through the real UI:
#   - A sketch on XY: its origin is the design's (mitcad#254).
#   - A rectangle from the origin, its width and height dimensioned: fully
#     constrained.
#   - The dark background recolours its fully constrained lines light, the
#     light one dark again (mitcad#253).
#   - The pointer moved to known sketch points: the view reports their
#     sketch coordinates in mm (mitcad#254).
#   - Outside sketch editing the shown sketch keeps the fully constrained
#     colour, also when the background changes; editing it again changes
#     no colour (mitcad#286).
#   - A design (mitcad-cli) whose component's sketch, placed by its
#     occurrence, has a fully constrained rectangle and a free line: the
#     rectangle in the constrained colour, the line blue, in the main mode
#     and in its edit alike (mitcad#286).
#   - A design (mitcad-cli) with a block and a sketch on its top face: a
#     fully constrained circle and a centre line on the face, a circle
#     beside the block. In the dark theme the lines on the face are light
#     with dark pixels of their halo beside them, in the light theme dark
#     with white ones; the circle over the empty background shows in both
#     (pixels of a screen dump; mitcad#302). Every background change also
#     restyles the halos ("Sketch halos:").
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-sketch-view-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-sketch-view.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# sketch_colours F1 (an extended regex: "F1 in O1", "F1( in O[0-9]+)?"):
# "<constrained> <unconstrained>", the counts of the sketch's entities in
# the fully constrained colour and in the unconstrained blue from the last
# "Sketch colours:" line; empty when it does not name the sketch.
sketch_colours() {
  grep "Sketch colours: " "$UI_LOG" | tail -1 | sed 's/.*Sketch colours: //' | tr ';' '\n' |
    sed 's/^ *//' | grep -E "^$1 [0-9]+ fully constrained, [0-9]+ unconstrained\$" | head -1 |
    sed -E 's/.* ([0-9]+) fully constrained, ([0-9]+) unconstrained$/\1 \2/'
}

# expect_colours F1 "awk condition on c and u" description: within six
# seconds the sketch's counts (c fully constrained, u unconstrained) pass
# the condition; COLOURS keeps them ("c u").
expect_colours() {
  local counts
  for _ in $(seq 1 30); do
    counts=$(sketch_colours "$1")
    if [ -n "$counts" ] && awk -v c="${counts% *}" -v u="${counts#* }" "BEGIN { exit !($2) }"; then
      COLOURS=$counts
      echo "ok   $3: ${counts% *} fully constrained, ${counts#* } unconstrained"
      return
    fi
    sleep 0.2
  done
  ui_fail "$3: the last colours are '$(grep "Sketch colours: " "$UI_LOG" | tail -1)'"
}

# expect_recoloured dark|light: the sketch entities shown were recoloured
# for that background, the rectangle's four lines among the fully
# constrained ones.
expect_recoloured() {
  local count
  ui_expect_new "Sketch entities recoloured for a $1 background: " "recoloured for the $1 background"
  count=$(sed -n "s/.*Sketch entities recoloured for a $1 background: \([0-9]*\) fully constrained.*/\1/p" "$UI_LOG" |
    tail -1)
  [ -n "$count" ] && [ "$count" -ge 4 ] ||
    ui_fail "$1 background: '$count' fully constrained entities recoloured, expected 4 or more"
  echo "ok   $count fully constrained entities recoloured for the $1 background"
  # Their halos took the background's tone too (mitcad#302).
  count=$(grep "Sketch halos: " "$UI_LOG" | tail -1 | sed -n "s/.*Sketch halos: \([0-9]*\) $1\$/\1/p")
  [ -n "$count" ] && [ "$count" -ge 4 ] ||
    ui_fail "$1 background: the last halos are '$(grep "Sketch halos: " "$UI_LOG" | tail -1)'"
  echo "ok   $count halos for the $1 background"
}

# expect_pointer x y: the pointer moved to the sketch point (x, y) mm; the
# view reports that place, within two pixels.
expect_pointer() {
  local px py view
  ui_mark
  ui_step "pointer to ($1, $2)" xdotool mousemove $(ui_sketch_at "$1" "$2")
  ui_expect_new "Sketch pointer x " "the pointer's place at ($1, $2) logged"
  read -r px py <<< "$(sed -n 's/.*Sketch pointer x \([-0-9.]*\), y \([-0-9.]*\) mm.*/\1 \2/p' "$UI_LOG" | tail -1)"
  [ -n "$py" ] || ui_fail "the pointer's place is not in mm: $(grep 'Sketch pointer' "$UI_LOG" | tail -1)"
  view=$(grep -o "Sketch view [-0-9.,]* [-0-9.,]* [-0-9.,]*" "$UI_LOG" | tail -1)
  awk -v view="$view" -v px="$px" -v py="$py" -v x="$1" -v y="$2" 'BEGIN {
    split(view, part, " "); split(part[3], o, ","); split(part[4], a, ",")
    tolerance = 2 * 100 / sqrt((a[1] - o[1]) ^ 2 + (a[2] - o[2]) ^ 2)
    exit !((px - x) ^ 2 <= tolerance ^ 2 && (py - y) ^ 2 <= tolerance ^ 2) }' ||
    ui_fail "the view reports ($px, $py) for the pointer at ($1, $2)"
  echo "ok   the view reports x $px, y $py for the pointer at ($1, $2)"
}

ui_start_display
ui_start_app

echo "--- A sketch on XY, its origin the design origin (mitcad#254)"
ui_step "light background (search)"      ui_command "Light Background"
ui_step "create sketch on XY"            ui_create_sketch xy
ui_expect_log "Sketch view" "the sketch view is logged"
ui_expect_log "Sketch origin at 0.00, 0.00, 0.00 mm in the design; the design origin at x 0.00, y 0.00 mm in the sketch" \
  "the sketch origin is the design origin"

echo "--- A fully constrained rectangle on a light background"
ui_step "rectangle tool (R)"             ui_key r
ui_step "first corner at the origin"     ui_sketch_click 0 0
ui_step "opposite corner"                ui_sketch_click 40 20
ui_expect_log "Added rectangle 40 x 20 mm at (0, 0)" "rectangle 40 x 20 at the origin"
ui_step "end the tool (Esc)"             ui_key Escape
ui_step "sketch dimension (D)"           ui_key d
ui_step "pick the bottom line"           ui_sketch_click 20 0
ui_step "place the value below it"       ui_sketch_click 20 -10
ui_step "type 40"                        xdotool type --delay 40 "40"
ui_step "Enter"                          ui_key Return
ui_step "pick the right line"            ui_sketch_click 40 10
ui_step "place the value beside it"      ui_sketch_click 52 10
ui_step "type 20"                        xdotool type --delay 40 "20"
ui_step "Enter"                          ui_key Return
ui_expect_log "Sketch Sketch1: 0 DOF, fully constrained" "fully constrained"
ui_step "end the tool (Esc)"             ui_key Escape

echo "--- Fully constrained lines on a dark background (mitcad#253)"
ui_mark
ui_step "dark background (search)"       ui_command "Dark Background"
expect_recoloured dark
if [ -n "$SHOT" ]; then
  ui_sync
  ui_capture "$SHOT"
fi
ui_mark
ui_step "light background (search)"      ui_command "Light Background"
expect_recoloured light

echo "--- The pointer's place in sketch coordinates (mitcad#254)"
expect_pointer 30 15
expect_pointer -15 -10

echo "--- Outside sketch editing the sketch keeps its colours (mitcad#286)"
# A new design's first sketch goes into a new component, Part1 (O1;
# mitcad#220): outside its edit that occurrence places it.
SKETCH1="F1( in O[0-9]+)?"
expect_colours "$SKETCH1" "c >= 4 && u == 0" "in its edit the rectangle is fully constrained"
EDITING=$COLOURS
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "sketch finished"
ui_wait_idle
ui_sync
shown=$(grep "Sketches shown: " "$UI_LOG" | tail -1)
[ "${shown#*Sketches shown: }" = "F1" ] || ui_fail "the finished sketch is not shown: '$shown'"
echo "ok   the finished sketch is shown"
expect_colours "$SKETCH1" "c == ${EDITING% *} && u == ${EDITING#* }" "outside its edit the same colours, fully constrained"
ui_mark
ui_step "dark background (search)"       ui_command "Dark Background"
expect_recoloured dark
ui_mark
ui_step "light background (search)"      ui_command "Light Background"
expect_recoloured light
ui_double_click_expect "Timeline Sketch1" "Editing sketch F1 (Sketch1)" "the sketch is edited again"
ui_wait_idle
ui_sync
expect_colours "$SKETCH1" "c == ${EDITING% *} && u == ${EDITING#* }" "entering its edit changes no colour"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "sketch finished again"

echo "--- A component's sketch placed by its occurrence (mitcad#286)"
# Part:1 (O1) 100 mm along X, its Sketch1 (F1) a dimensioned 30 x 20
# rectangle (fully constrained) and a free line.
cat > "$WORK/component.json" << 'EOF'
[
  {"cmd": "create_component", "name": "Part"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 30], "end": [20, 40]},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "set_occurrence_transform", "occurrence": "O1", "transform": {"translation": [100, 0, 0]}}
]
EOF
"$CLI" run "$WORK/component.json" --save "$WORK/component.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
ui_stop_app
ui_start_app --open "$WORK/component.mitcad"
UI_MARK=0
ui_expect_log "Sketches shown: F1" "the component's sketch is shown"
expect_colours "F1 in O1" "c >= 4 && u >= 1" "placed by Part:1, the rectangle fully constrained and the line blue"
PLACED=$COLOURS
ui_mark
ui_step "dark background (search)"       ui_command "Dark Background"
expect_recoloured dark
ui_mark
ui_step "light background (search)"      ui_command "Light Background"
expect_recoloured light
ui_expect_log "Timeline Sketch1 at" "the timeline is laid out"
ui_double_click_expect "Timeline Sketch1" "Editing sketch F1 (Sketch1)" "the component's sketch is edited"
ui_wait_idle
ui_sync
expect_colours F1 "c == ${PLACED% *} && u == ${PLACED#* }" "in its edit the same colours"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "sketch finished"
ui_wait_idle
ui_sync
expect_colours "F1 in O1" "c == ${PLACED% *} && u == ${PLACED#* }" "placed by Part:1 again, the same colours"

echo "--- Fully constrained curves and a centre line over a body's face (mitcad#302)"
# A 60 x 40 x 30 mm block centred on the origin; on its top face (sketch
# origin over the design's) a fully constrained circle on the face, one
# beside the block over the empty background, and a centre line on the
# face. Each is drawn over a halo in the background's tone, which keeps it
# readable over the face in both themes.
cat > "$WORK/face.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-30, -20],
    "length": 60, "width": 40, "height": 30, "operation": "new_body"}},
  {"cmd": "sketch.create", "plane": {"face": "F1:top", "body": "F1.b0"}},
  {"cmd": "sketch.add_circle", "sketch": "F2", "center": [10, 0], "diameter": 16},
  {"cmd": "sketch.add_circle", "sketch": "F2", "center": [50, 0], "diameter": 16},
  {"cmd": "sketch.add_line", "sketch": "F2", "start": [-25, -12], "end": [25, -12], "centerline": true}
]
EOF
"$CLI" run "$WORK/face.json" --save "$WORK/face.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
ui_stop_app
ui_start_app --open "$WORK/face.mitcad"
UI_MARK=0
ui_expect_log "Sketches shown: F2" "the face's sketch is shown"
# The circles and their centres fully constrained, the centre line's free
# ends blue.
expect_colours F2 "c == 4" "both circles fully constrained"
ui_expect_log "Timeline Sketch1 at" "the timeline is laid out"
ui_double_click_expect "Timeline Sketch1" "Editing sketch F2 (Sketch1)" "the face's sketch is edited"
ui_wait_idle
ui_sync

# expect_halos "3 dark": within six seconds the last "Sketch halos:" line
# says so.
expect_halos() {
  for _ in $(seq 1 30); do
    [ "$(grep "Sketch halos: " "$UI_LOG" | tail -1 | sed 's/.*Sketch halos: //')" = "$1" ] &&
      { echo "ok   halos: $1"; return; }
    sleep 0.2
  done
  ui_fail "halos: the last line is '$(grep "Sketch halos: " "$UI_LOG" | tail -1)', expected '$1'"
}

# tones name x y "w h": the pixels of the region centred on screen point
# (x, y) in the dump of the screen: "light dark white min max", the counts
# of luminance above 200, below 70 and above 244, and the lowest and
# highest luminance.
tones() {
  local w=${4% *} h=${4#* }
  convert "$WORK/$1.xwd" -crop "${w}x${h}+$(($2 - w / 2))+$(($3 - h / 2))" +repage -depth 8 txt:- |
    sed -n 's/^[0-9]*,[0-9]*: *(\([0-9.]*\),\([0-9.]*\),\([0-9.]*\).*/\1 \2 \3/p' |
    awk 'BEGIN { min = 255; max = 0 }
      { l = 0.299 * $1 + 0.587 * $2 + 0.114 * $3
        light += l > 200; dark += l < 70; white += l > 244
        if (l < min) min = l; if (l > max) max = l }
      END { printf "%d %d %d %d %d\n", light, dark, white, min, max }'
}

# region name sketch-x sketch-y "w h" description: the tones of the region
# around a sketch point, in L (light), D (dark), W (white), MIN and MAX.
region() {
  local px py
  read -r px py <<< "$(ui_sketch_at "$2" "$3")"
  read -r L D W MIN MAX <<< "$(tones "$1" "$px" "$py" "$4")"
  [ -n "$MAX" ] || ui_fail "$5: no pixels at ($px, $py)"
  echo "     $5 at ($px, $py): $L light, $D dark, $W white, luminance $MIN-$MAX"
}

# check name "condition" description: an awk condition on L, D, W, MIN, MAX.
check() {
  awk -v L="$L" -v D="$D" -v W="$W" -v MIN="$MIN" -v MAX="$MAX" "BEGIN { exit !($2) }" ||
    ui_fail "$1: $3 ($L light, $D dark, $W white, luminance $MIN-$MAX)"
  echo "ok   $1: $3"
}

# A strip across each curve, wider than the line and its halo: across the
# right side of each circle and across the centre line (along it, so that
# its dashes are in it).
ACROSS="25 5"
ALONG="41 17"

ui_step "dark background (search)"       ui_command "Dark Background"
expect_halos "3 dark"
ui_sync
sleep 1
xwd -root -silent > "$WORK/dark.xwd" || ui_fail "no screenshot"
[ -z "$SHOT" ] || convert "$WORK/dark.xwd" "${SHOT%.png}-face-dark.png"
region dark 18 0 "$ACROSS" "dark: the circle on the face"
check dark "L >= 2 && D >= 2" "the circle on the face is light over a dark halo"
region dark -10 -12 "$ALONG" "dark: the centre line on the face"
check dark "D >= 2 && MAX >= 140" "the centre line on the face is light over a dark halo"
region dark 58 0 "$ACROSS" "dark: the circle beside the block"
check dark "L >= 2" "the circle over the dark background is light"

ui_step "light background (search)"      ui_command "Light Background"
expect_halos "3 light"
ui_sync
sleep 1
xwd -root -silent > "$WORK/light.xwd" || ui_fail "no screenshot"
[ -z "$SHOT" ] || convert "$WORK/light.xwd" "${SHOT%.png}-face-light.png"
region light 18 0 "$ACROSS" "light: the circle on the face"
check light "MIN <= 110 && W >= 2" "the circle on the face is dark over a light halo"
region light -10 -12 "$ALONG" "light: the centre line on the face"
check light "W >= 2 && MIN <= 160" "the centre line on the face is darker over a light halo"
region light 58 0 "$ACROSS" "light: the circle beside the block"
check light "MIN <= 110" "the circle over the light background is dark"

ui_finish "UI sketch view test"
