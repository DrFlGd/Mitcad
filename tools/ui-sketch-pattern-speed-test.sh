#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/OcctViewer.cpp app/MainWindow.cpp core/solver tools/cli
# A sketch with many patterned circles (mitcad#670), as a user works on it,
# with the times of each step logged (MITCAD_LOG_TIMING=1) and summed up:
#   - The design: tools/cli/tests/perf_sketch_pattern_<N>.json, one circle
#     and an N x N rectangular pattern of it (N = UI_PATTERN_SIZE: 5, 10
#     (the default) or 20), made with mitcad-cli; the sketch edited from
#     the timeline.
#   - A window selection round the whole pattern selects every circle:
#     the view's pick ("window pick"), the selection applied ("window
#     selection applied") and the frame that shows it ("window selection
#     shown"), each from the release.
#   - The original circle's centre dragged: every copy follows. The drag's
#     steps ("sketch drag step": the model's sketch.drag, the sketch read
#     again, the scene refreshed) and the frames between them.
#   - The original circle's edge dragged in: its radius, every copy's too.
# Only what the steps do is checked; the times are printed ("Times: ...")
# for comparisons (tools/perf-measure.sh, the ticket's table), since a
# debug build and a loaded machine take many times longer.
#
# Runs headless on Xvfb (see ui-test-lib.sh); UI_SCALE=2 runs it at device
# pixel ratio 2; UI_PATTERN_LOG=<file> keeps the application's log there
# (every "Timing" line).
# Usage: [UI_PATTERN_SIZE=5|10|20] tools/ui-sketch-pattern-speed-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

export MITCAD_LOG_TIMING=1
# As a user runs it: without the UI tests' logs of pick places and volumes
# (as tools/perf-measure.sh), which would log every circle after each step.
unset MITCAD_LOG_PICKS MITCAD_LOG_VOLUMES
N=${UI_PATTERN_SIZE:-10}
SCRIPT=$(cd "$(dirname "$0")" && pwd)/cli/tests/perf_sketch_pattern_$N.json
[ -f "$SCRIPT" ] || { echo "FAIL: no fixture for UI_PATTERN_SIZE=$N ($SCRIPT)"; exit 2; }
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-pattern-speed.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

"$CLI" run "$SCRIPT" --save "$WORK/pattern.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli $(basename "$SCRIPT")"; }

# The fixture ends with the original circle (c1, centre p2) at (5, 2.5),
# radius 4.5, the copies 12 apart in x and 10 in y.
CX=5
CY=2.5
R=4.5
XMAX=$(awk -v n="$N" 'BEGIN { print 5 + 12 * (n - 1) + 4.5 }')
YMAX=$(awk -v n="$N" 'BEGIN { print 2.5 + 10 * (n - 1) + 4.5 }')
COUNT=$((N * N))

# zoom_to_show x1 y1 x2 y2: zooms out (the wheel at the view's middle)
# until the sketch rectangle from (x1, y1) to (x2, y2) mm is in the view.
zoom_to_show() {
  local area vx vy vw vh ax ay bx by
  for _ in $(seq 1 30); do
    area=$(grep -o "View area [0-9]* [0-9]* [0-9]* [0-9]*" "$UI_LOG" | tail -1)
    read -r _ _ vx vy vw vh <<< "$area"
    read -r ax ay <<< "$(ui_sketch_at "$1" "$2")"
    read -r bx by <<< "$(ui_sketch_at "$3" "$4")"
    if [ "$ax" -gt $((X + (vx + 20) * UI_SCALE)) ] && [ "$bx" -lt $((X + (vx + vw - 20) * UI_SCALE)) ] &&
      [ "$by" -gt $((Y + (vy + 20) * UI_SCALE)) ] && [ "$ay" -lt $((Y + (vy + vh - 20) * UI_SCALE)) ]; then
      return 0
    fi
    xdotool mousemove $(ui_view_at 50 50) click 5
    ui_sync
  done
  ui_fail "the view did not zoom out to show ($1, $2) to ($3, $4)"
}

# timings "step": the numbers of the "Timing <step>: <ms> ms" lines since
# the mark, one per line.
timings() {
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | sed -n "s/^Timing $1: \([0-9.]*\) ms.*/\1/p"
}

# summary "step": "n steps, median m ms, max x ms" of those lines.
summary() {
  timings "$1" | sort -n | awk '{ v[NR] = $1 } END {
    if (NR == 0) { print "none"; exit }
    printf "%d, median %.1f ms, max %.1f ms", NR, v[int((NR + 1) / 2)], v[NR] }'
}

# slow_drag x1 y1 x2 y2 moves: a left drag between sketch points (mm) in
# that many pointer moves 20 ms apart, as a hand moves a mouse; prints how
# long the moves took (ms).
slow_drag() {
  local i fx fy tx ty start end
  read -r fx fy <<< "$(ui_sketch_at "$1" "$2")"
  read -r tx ty <<< "$(ui_sketch_at "$3" "$4")"
  ui_apart "$fx" "$fy"
  # The pointer rests over the geometry first: what the press grabs is
  # what the view found under it (after a large redisplay, a while).
  xdotool mousemove "$fx" "$fy"
  ui_sync
  sleep 0.5
  ui_sync
  xdotool mousedown 1
  start=$(date +%s%N)
  for i in $(seq 1 "$5"); do
    xdotool mousemove $((fx + (tx - fx) * i / $5)) $((fy + (ty - fy) * i / $5))
    sleep 0.02
  done
  ui_sync
  end=$(date +%s%N)
  xdotool mouseup 1
  echo $(((end - start) / 1000000))
}

ui_start_display
ui_start_app --open "$WORK/pattern.mitcad"
ui_step "fit (F6)"                         ui_key F6
ui_step "double-click Sketch1"             ui_double_click_logged "Timeline Sketch1"
ui_expect_log "Editing sketch F1 (Sketch1)" "the sketch edited"
ui_sync
ui_step "zoom out"                         zoom_to_show -12 -12 "$XMAX" "$YMAX"
ui_wait_idle

echo "--- A window round the whole pattern: every circle selected"
ui_mark
ui_step "window selection"                 ui_sketch_drag -10 -10 "$(awk -v x="$XMAX" 'BEGIN { print x + 8 }')" \
                                             "$(awk -v y="$YMAX" 'BEGIN { print y + 8 }')"
ui_expect_new "Selected: $COUNT sketch curves" "the window selected all $COUNT circles" 120
ui_expect_new "Timing window selection shown: " "the selection shown" 120
ui_sync
PICK=$(timings "window pick" | tail -1)
APPLIED=$(timings "window selection applied" | tail -1)
SHOWN=$(timings "window selection shown" | tail -1)
REFRESH_SEL=$(summary "refresh")
ui_step "clear the selection (Esc)"        ui_key Escape

echo "--- The original's centre dragged: the copies follow"
ui_mark
ui_wait_idle
MOVED=$(slow_drag "$CX" "$CY" $((10 + N)) $((5 + N)) 30)
ui_expect_new "Dragged p2 to" "the centre dragged" 120
CENTER_STEPS=$(timings "sketch drag step" | wc -l)
CENTER_STEP=$(summary "sketch drag step")
CENTER_COMMAND=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | sed -n 's/^Timing command: \([0-9.]*\) ms, sketch.drag$/\1/p' |
  sort -n | awk '{ v[NR] = $1 } END { if (NR) printf "median %.1f ms", v[int((NR + 1) / 2)]; else print "none" }')
CENTER_RELOAD=$(summary "sketch reload")
CENTER_REFRESH=$(summary "refresh")
CENTER_FRAME=$(summary "frame")
[ "$CENTER_STEPS" -gt 0 ] || ui_fail "no drag steps logged"
# The sketch read from the model once per step (mitcad#679): the view and
# sketch mode share one answer of the sketch query. Counted up to the last
# step (the drag's end refreshes once more).
CENTER_QUERIES=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" |
  awk '/^Timing sketch query: / { n++ } /^Timing sketch drag step: / { steps = n } END { print steps + 0 }')
[ "$CENTER_QUERIES" -le "$CENTER_STEPS" ] ||
  ui_fail "the sketch was read $CENTER_QUERIES times in $CENTER_STEPS drag steps (once per step expected)"
ui_mark
ui_step "undo the drag (Ctrl+Z)"           ui_key ctrl+z
# A large pattern takes seconds to solve again: the next drag waits for it.
ui_expect_new "Undo: Drag in Sketch1" "the drag undone" 120
ui_expect_new "Timing refresh: " "the sketch shown again" 120
ui_sync

echo "--- The original's edge dragged: every radius follows"
# Inwards, to a radius of 1.5: 3 mm are more pixels than a drag needs
# (4) also in the view of the 20 x 20 pattern, and the circles stay apart.
ui_mark
ui_wait_idle
RMOVED=$(slow_drag "$(awk -v x="$CX" -v r="$R" 'BEGIN { print x + r }')" "$CY" \
  "$(awk -v x="$CX" 'BEGIN { print x + 1.5 }')" "$CY" 30)
ui_expect_new "Dragged c1 to" "the circle's edge dragged" 120
RADIUS_STEPS=$(timings "sketch drag step" | wc -l)
RADIUS_STEP=$(summary "sketch drag step")
RADIUS_FRAME=$(summary "frame")

[ -n "${UI_PATTERN_LOG:-}" ] && cp -f "$UI_LOG" "$UI_PATTERN_LOG"
echo "Times ($N x $N, $COUNT circles, scale $UI_SCALE):"
echo "  window selection: pick ${PICK:-?} ms, applied ${APPLIED:-?} ms, shown ${SHOWN:-?} ms after the release; refreshes: $REFRESH_SEL"
echo "  centre drag: $CENTER_STEPS steps in $MOVED ms of moves; step: $CENTER_STEP; sketch.drag: $CENTER_COMMAND;" \
  "sketch queries: $CENTER_QUERIES; reload: $CENTER_RELOAD; refresh: $CENTER_REFRESH; frames: $CENTER_FRAME"
echo "  radius drag: $RADIUS_STEPS steps in $RMOVED ms of moves; step: $RADIUS_STEP; frames: $RADIUS_FRAME"
ui_finish "sketch pattern of $COUNT circles: window selection and drags"
