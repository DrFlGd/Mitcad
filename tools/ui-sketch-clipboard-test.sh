#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/commands/SketchCommands.cpp app/MainWindow.cpp app/framework/ShortcutBindings.hpp tools/cli
# check-all sources: core/sketch core/model-api/src/sketch_clipboard.rs core/model-api/src/sketch.rs core/solver
# Copy, Cut and Paste of sketch geometry (mitcad#788), on a design
# mitcad-cli makes: Sketch1 (F1) on XY with its origin point p1, a line c2
# from p3 (10, 10) to p4 (40, 10), horizontal (k1), 30 long (k2) and 10
# above the origin (k3, a vertical distance from p1), and a line c5 from
# (0, 30) to (40, 30) parallel to c2 (k4); Sketch2 (F2) on XZ, empty.
#   1. c2 clicked and cut (Ctrl+X): the copy keeps the horizontal, the
#      length and the distance from the origin and leaves the parallel out;
#      c2 is deleted from Sketch1. Its base point is c2's start, and the
#      distance from the origin holds it in y. Undo and redo name the step
#      "Cut in Sketch1".
#   2. In Sketch2, Paste (Ctrl+V) and a click place it, still 10 above the
#      origin wherever the click is in y; undo takes it back as one step.
#   3. Paste and Esc cancels it.
#   4. Paste with X 30 typed and Enter (no Y box: y is held): the line goes
#      to (30, 10) to (60, 10) on Sketch2's own origin, with the three
#      constraints and dimensions; the saved design has it, and Sketch1 has
#      only c5.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-sketch-clipboard-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-sketch-clipboard.XXXXXX)
FILE=$WORK/clipboard.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [0, 0], "fixed": true},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [10, 10], "end": [40, 10]},
  {"cmd": "sketch.add_constraint", "sketch": "F1", "constraint": {"type": "horizontal", "line": "c2"}},
  {"cmd": "sketch.add_dimension", "sketch": "F1", "value": 30,
   "dimension": {"type": "length", "line": "c2"}},
  {"cmd": "sketch.add_dimension", "sketch": "F1", "value": 10,
   "dimension": {"type": "vertical_distance", "a": "p1", "b": "p3"}},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 30], "end": [40, 30]},
  {"cmd": "sketch.add_constraint", "sketch": "F1",
   "constraint": {"type": "parallel", "a": "c2", "b": "c5"}},
  {"cmd": "sketch.create", "plane": "xz"},
  {"expect": {"query": {"query": "sketch_fragment", "sketch": "F1", "items": ["c2"]},
   "result": {"kept": 3, "left_out": 1, "base": [10, 10], "held": [false, true]}}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# edit "Sketch1" F1: a double-click on its timeline item, then a fit.
edit() {
  ui_double_click_expect "Timeline $1" "Editing sketch $2 ($1)" "$1 edited"
  ui_expect_new "Sketch view " "$1's view is placed"
  ui_step "fit (F6)"                     ui_key F6
  ui_sync
}

ui_start_display
ui_start_app --open "$FILE"
ui_expect_log "Timeline Sketch2 at" "Sketch1 and Sketch2 in the timeline"

echo "--- Cut c2 from Sketch1"
edit Sketch1 F1
ui_mark
ui_step "click c2"                       ui_sketch_click 33 10
ui_expect_new "Selected: 1 sketch curve" "c2 selected"
ui_mark
ui_step "cut (Ctrl+X)"                   ui_key ctrl+x
ui_expect_new "Cut c2 from Sketch1: 3 constraint(s) and dimension(s) kept, 1 left out, base point at (10, 10)" \
  "the cut keeps the horizontal, the length and the distance from the origin"
ui_expect_new "Deleted c2" "c2 deleted"
ui_mark
ui_step "undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: Cut in Sketch1" "the cut is one undo step, named Cut"
ui_mark
ui_step "redo (Ctrl+Y)"                  ui_key ctrl+y
ui_expect_new "Redo: Cut in Sketch1" "redone"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "Sketch1 finished"

echo "--- Paste into Sketch2 with a click, then undo"
edit Sketch2 F2
ui_mark
ui_step "paste (Ctrl+V)"                 ui_key ctrl+v
ui_expect_new "Paste: geometry of Sketch1 from" "the clipboard holds Sketch1's geometry"
held=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "Paste: geometry of Sketch1" | tail -1)
case $held in
*", base point at (10, 10), held in y") echo "ok   held in y, its base point c2's start" ;;
*) ui_fail "not held in y at c2's start: '$held'" ;;
esac
ui_expect_new "Sketch tool Paste" "the Paste tool runs"
ui_step "move the pointer"               xdotool mousemove $(ui_sketch_at 15 15)
ui_sync
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_mark
# At y 15 (as the pointer): Sketch1's leftover lies edge-on along x in this
# view, so the fit leaves about 19 mm above the origin; y 20 is on the ribbon.
ui_step "click where it goes"            ui_sketch_click 20 15
# The pasted ids are the target's own (points first, as the source lists
# them): read from the log, not assumed.
ui_expect_new " into Sketch2 at " "a click pastes c2 and its points"
placed=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "Pasted .* into Sketch2 at " | tail -1)
[[ $placed =~ "Pasted "p[0-9]+", "p[0-9]+", "c[0-9]+" into Sketch2 at" ]] ||
  ui_fail "not two points and a line pasted: '$placed'"
[[ $placed =~ " at ("[-0-9.]+", 10):" ]] || ui_fail "the click moved it in y: '$placed'"
echo "ok   still 10 above the origin: $placed"
ui_mark
ui_step "undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: Paste into Sketch2" "the paste is one undo step"

echo "--- Esc cancels a paste"
ui_mark
ui_step "paste (Ctrl+V)"                 ui_key ctrl+v
ui_expect_new "Sketch tool Paste" "the Paste tool runs"
ui_step "Esc"                            ui_key Escape
ui_expect_new "Paste cancelled" "Esc cancels it"
grep -q "Pasted " <(tail -n +$((UI_MARK + 1)) "$UI_LOG") && ui_fail "Esc pasted"
echo "ok   nothing pasted"

echo "--- Paste at a typed X"
ui_mark
ui_step "paste (Ctrl+V)"                 ui_key ctrl+v
ui_expect_new "Sketch tool Paste" "the Paste tool runs"
ui_sync
ui_step "type X 30"                      ui_key ctrl+a
xdotool type --delay 60 "30"
ui_expect_new "Sketch value x: 30" "X typed"
ui_step "Enter"                          ui_key Return
ui_expect_new " into Sketch2 at (30, 10): 3 constraint(s) and dimension(s) kept, 1 left out" \
  "pasted at the typed place"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "Sketch2 finished"

echo "--- The saved design"
ui_mark
ui_step "save (Ctrl+S)"                  ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
cat > "$WORK/check.json" << 'EOF2'
[
  {"expect": {"query": {"query": "sketch", "uid": "F1"}, "result": {
    "entities": [{"id": "p1"}, {"type": "point", "at": [0, 30]}, {"type": "point", "at": [40, 30]},
                 {"id": "c5", "type": "line"}],
    "constraints": [], "dimensions": []}}},
  {"expect": {"query": {"query": "sketch", "uid": "F2"}, "result": {
    "entities": [{"id": "p1", "fixed": true, "at": [0, 0]},
                 {"type": "point", "at": [30, 10]}, {"type": "point", "at": [60, 10]},
                 {"type": "line"}],
    "constraints": [{"type": "horizontal"}],
    "dimensions": [{"type": "length", "measured": 30},
                   {"type": "vertical_distance", "a": "p1", "measured": 10}]}}}
]
EOF2
"$CLI" run "$WORK/check.json" --open "$FILE" > "$WORK/check.log" 2>&1 ||
  { cat "$WORK/check.log"; ui_fail "the saved sketches"; }
echo "ok   Sketch1 keeps c5 alone; Sketch2 has the line with its constraints and dimensions"

ui_finish "UI sketch clipboard test"
