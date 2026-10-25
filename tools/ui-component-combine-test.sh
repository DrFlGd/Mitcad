#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/browser app/framework core/model/src/features/combine.rs
# check-all sources: app/OcctViewer.cpp core/model/src/features/moves.rs
# check-all sources: core/model/src/features/point_pattern.rs
# Combine with a tool of another component, through the real UI
# (mitcad#104, mitcad#105, mitcad#544): a block in the root, a block of
# Part:1 beside it and Part:2 on its other side, both overlapping it.
#   - Combine with the root's block as the target and Part's block, picked
#     where Part:1 shows it, as the tool: the tool comes in as a body in
#     its occurrence and joins the target. Consumed, it leaves Part: the
#     preview draws it where Part:2 shows it as a tool overlay, and the
#     panel says it is also gone from Part:2 (mitcad#561).
#   - Edited to a cut: the panel takes the tool back where it was picked.
#   - Edited again, Keep Tools still off, Part's block picked on its tool
#     overlay where Part:2 shows it (mitcad#561): the same body in two
#     occurrences is two tools, listed "Part:1, Part:2", and the target
#     has both cuts.
#   - The project file keeps each tool with its occurrence.
#   - Move/Copy Point to Point between components (mitcad#699), on a second
#     design: Part:1 10 mm and Part:2 50 mm along X, nothing in the root.
#     Part:2 moved from a vertex of its block to the same vertex where
#     Part:1 shows it: the preview puts Part:2 on Part:1, the file keeps
#     each point's occurrence, an edit loads them back.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-component-combine-test.sh [screenshot.png]
# check-all sources: tools/cli

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-component-combine.XXXXXX)
FILE=$WORK/overlap.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# expect_file expression expected description: a Python expression on the
# saved project file (doc).
expect_file() {
  local actual
  actual=$(python3 -c 'import json, sys; doc = json.load(open(sys.argv[1])); print(eval(sys.argv[2]))' \
    "$FILE" "$1") || ui_fail "$3: cannot read $FILE"
  [ "$actual" = "$2" ] || ui_fail "$3: $1 is '$actual', expected '$2'"
  echo "ok   $3"
}

save() {
  ui_mark
  ui_key ctrl+s
  ui_expect_new "Saved $FILE" "saved"
}

# Part:1 (O1, C1): Sketch1 (F1) 30 x 20, Extrude1 (F2) 10 up: F2.b0.
# Part:2 (O2) 30 along X. The root, active: Sketch2 (F3) 20 x 20 at
# x = 20, Extrude2 (F4) 15 up: F4.b0, overlapping Part:1's block by
# 10 x 20 x 10 mm (x 20 to 30) and Part:2's as much (x 30 to 40).
R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
cat > "$WORK/model.json" << EOF
[
  {"cmd": "create_component", "name": "Part"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "copy_occurrence", "occurrence": "Part:1", "transform": {"translation": [30, 0, 0]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [20, 0], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "$R"}],
    "extent": {"type": "distance", "distance": 15}, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "top view (search)"              ui_command "Top View"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Bodies shown: " "the blocks shown"
ui_expect_log "Body F2.b0 in O2 at " "Part:2's block shown"

echo "--- Combine with Part's block as the tool"
ui_mark
ui_step "combine (search)"               ui_command "Combine"
ui_step "pick the root's block"          ui_click_logged "Body F4.b0"
ui_expect_new "Combine Target Body: 1 body [body F4.b0]" "the target picked"
ui_step "pick Part's block"              ui_click_logged "Body F2.b0 in O1"
ui_expect_new "Combine Tool Bodies: 1 body [body F2.b0 in O1]" "the tool picked where Part:1 shows it"
ui_expect_new "Combine Tool Bodies listed: Part:1" "the tool listed by its occurrence"
ui_expect_new "Preview Combine: ok" "previewed"
ui_expect_new "F2.b0 in O2 at 30,0,0 tool" "Part's block drawn as a tool where Part:2 shows it"
ui_expect_new "leaves Part: it is also gone from Part:2." "the panel says the tool is also gone from Part:2"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_step "OK (Enter)"                     ui_key Return
# 6000 + 6000 - 2000 mm3.
ui_expect_volume "Body [A-Za-z0-9]* (F4.b0): volume 6000.000 -> \([0-9.]*\) mm3" 10000 "joined with Part's block"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
save
expect_file 'doc["features"][4]["type"], doc["features"][4]["tools"], "tool_links" in doc["features"][4]' \
  "('combine', [{'body': 'F2.b0', 'occurrence': 'O1'}], False)" "the tool in Part:1"

echo "--- Edited to a cut"
ui_mark
ui_step "double-click Combine1"          ui_double_click_logged "Timeline Combine1"
ui_expect_new "Editing F5 with Combine" "editing the combine"
ui_expect_new "Combine Tool Bodies listed: Part:1" "the tool back in Part:1"
ui_step "Cut"                            ui_choose "Panel Combine input operation" 1
# The tool where it was picked again: the preview reads it in Part:1.
ui_expect_new "Preview Combine: ok" "previewed with the tool in Part:1"
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F5" "edited"
ui_expect_volume "Body [A-Za-z0-9]* (F4.b0): volume [0-9.]* -> \([0-9.]*\) mm3" 4000 "Part:1's block cut away"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
save
expect_file 'doc["features"][4]["operation"], doc["features"][4]["tools"]' \
  "('cut', [{'body': 'F2.b0', 'occurrence': 'O1'}])" "a cut, still in Part:1"

echo "--- Part:2's block added: the same body in two occurrences"
ui_mark
ui_step "double-click Combine1"          ui_double_click_logged "Timeline Combine1"
ui_expect_new "Editing F5 with Combine" "editing the combine again"
ui_expect_new "Preview Combine: ok" "previewed"
ui_expect_new "leaves Part: it is also gone from Part:2." "the edit says the tool is also gone from Part:2"
# Keep Tools stays off: a consumed tool leaves its own component
# (mitcad#104), so with Part:1's block a tool the preview has Part's block
# in no occurrence; with Tool Bodies active it draws it where Part:2 shows
# it as a tool overlay, which a click picks (mitcad#561).
ui_mark
ui_step "Tool Bodies"                    ui_click_logged "Panel Combine input tools"
ui_expect_new "F2.b0 in O2 at 30,0,0 tool" "Part's block drawn as a tool where Part:2 shows it"
ui_mark
ui_step "pick Part:2's block"            ui_click_logged "Body F2.b0 in O2"
ui_expect_new "Combine Tool Bodies: 2 bodies [body F2.b0 in O1; body F2.b0 in O2]" \
  "the same body picked where Part:2 shows it"
ui_expect_new "Combine Tool Bodies listed: Part:1, Part:2" "listed by their occurrences"
ui_expect_new "Preview Combine: ok" "previewed with both"
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F5" "edited"
# 6000 - 2000 - 2000 mm3.
ui_expect_volume "Body [A-Za-z0-9]* (F4.b0): volume [0-9.]* -> \([0-9.]*\) mm3" 2000 "both blocks cut away"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
save
expect_file 'doc["features"][4]["tools"]' \
  "[{'body': 'F2.b0', 'occurrence': 'O1'}, {'body': 'F2.b0', 'occurrence': 'O2'}]" \
  "a tool in each occurrence"

echo "--- Move/Copy Point to Point between copies of a component (mitcad#699)"
# Part:1 (O1, C1) 10 along X: Sketch1 (F1) 30 x 20, Extrude1 (F2) 10 up:
# F2.b0. Part:2 (O2) 50 along X. Nothing in the root.
ui_stop_app
FILE=$WORK/copies.mitcad
cat > "$WORK/copies.json" << EOF
[
  {"cmd": "create_component", "name": "Part", "transform": {"translation": [10, 0, 0]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "copy_occurrence", "occurrence": "Part:1", "transform": {"translation": [50, 0, 0]}}
]
EOF
"$CLI" run "$WORK/copies.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
ui_start_app --open "$FILE"
ui_step "top view (search)"              ui_command "Top View"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Body F2.b0 in O2 at " "Part:2's block shown"
ui_mark
ui_step "select Part:2 in the browser"   ui_click_logged "Browser Root/Part:2"
ui_expect_new "Browser selected: Root/Part:2" "Part:2 selected"
ui_step "move (M)"                       ui_key m
ui_expect_new "Command Move/Copy started" "Move/Copy started"
ui_expect_new "Panel Move/Copy choices: move_type=components" "Part:2 moves as a component"
ui_step "Point to Point"                 ui_choose "Panel Move/Copy input type" 3
ui_expect_new "Move/Copy: Move Type = Point to Point" "Point to Point"
ui_step "Origin Point"                   ui_click_logged "Panel Move/Copy input from"
ui_expect_new "Pick places:" "the places of the picks logged"
ui_sync
# Where a click picks the vertices of Part's block ("<vertex> <x> <y>"),
# both copies: in the top view X points right, so of the two places of a
# vertex Part:2's is the one on the right.
start=$(grep -n "Pick places:" "$UI_LOG" | tail -1 | cut -d: -f1)
places=$(tail -n +"$start" "$UI_LOG" |
  sed -n 's/^Pick vertex F2\.b0\/\(V{[^ ]*}\) at \([0-9]*\),\([0-9]*\)$/\1 \2 \3/p')
vertex=$(awk '{print $1}' <<< "$places" | sort | uniq -d | head -1)
[ -n "$vertex" ] || ui_fail "no vertex of Part's block can be picked in both copies: $places"
read -r left_x left_y <<< "$(awk -v v="$vertex" '$1 == v {print $2, $3}' <<< "$places" | sort -n | head -1)"
read -r right_x right_y <<< "$(awk -v v="$vertex" '$1 == v {print $2, $3}' <<< "$places" | sort -n | tail -1)"
[ "$right_x" -gt "$left_x" ] || ui_fail "the two places of $vertex are not side by side"
echo "ok   $vertex at $left_x,$left_y (Part:1) and $right_x,$right_y (Part:2)"
ui_mark
ui_step "pick it where Part:2 shows it"  xdotool mousemove $((X + right_x * UI_SCALE)) $((Y + right_y * UI_SCALE)) click 1
ui_expect_new "Move/Copy Origin Point: 1 vertex [vertex $vertex of F2.b0 in O2]" "the origin picked in Part:2"
ui_step "Target Point"                   ui_click_logged "Panel Move/Copy input to"
ui_step "pick it where Part:1 shows it"  xdotool mousemove $((X + left_x * UI_SCALE)) $((Y + left_y * UI_SCALE)) click 1
ui_expect_new "Move/Copy Target Point: 1 vertex [vertex $vertex of F2.b0 in O1]" "the target picked in Part:1"
ui_expect_new "Preview Move/Copy: ok" "previewed"
ui_expect_new "Preview Move/Copy moves O2" "the preview moves Part:2"
# From 50 mm to 10 mm along X: onto Part:1.
ui_expect_new "F2.b0 in O2 at 10,0,0" "Part:2 previewed on Part:1"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Moved 1 components (Point to Point)" "Part:2 moved"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
save
expect_file '[(f["transform"].get("from_occurrence"), f["transform"].get("to_occurrence")) for f in doc["features"] if f["type"] == "move_occurrence"]' \
  "[('O2', 'O1')]" "each point kept with its occurrence"
ui_mark
ui_step "double-click Move1"             ui_double_click_logged "Timeline Move1"
ui_expect_new "Editing F3 with Move/Copy" "editing the move"
ui_expect_new "Move/Copy Origin Point: 1 vertex [vertex $vertex of F2.b0 in O2]" "the origin back in Part:2"
ui_expect_new "Move/Copy Target Point: 1 vertex [vertex $vertex of F2.b0 in O1]" "the target back in Part:1"
ui_expect_new "Preview Move/Copy: ok" "previewed again"
ui_step "cancel (Esc)"                   ui_key Escape
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI component combine test"
