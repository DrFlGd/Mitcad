#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/browser app/framework core/model/src/features/combine.rs
# Combine with a tool of another component, through the real UI
# (mitcad#104, mitcad#105, mitcad#544): a block in the root, a block of
# Part:1 beside it and Part:2 on its other side, both overlapping it.
#   - Combine with the root's block as the target and Part's block, picked
#     where Part:1 shows it, as the tool: the tool comes in as a body in
#     its occurrence and joins the target.
#   - Edited to a cut: the panel takes the tool back where it was picked.
#   - Edited again, Part's block where Part:2 shows it added: the same body
#     in two occurrences is two tools, listed "Part:1, Part:2", and the
#     target has both cuts.
#   - The project file keeps each tool with its occurrence.
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
# Keep Tools on first: a consumed tool leaves its own component
# (mitcad#104), so with Part:1's block a tool the preview draws no block
# where Part:2 shows Part, and nothing is there to pick.
ui_mark
ui_step "Keep Tools"                     ui_click_logged "Panel Combine input keep_tools"
ui_expect_new "Combine: Keep Tools = on" "the tools kept"
ui_expect_new "Preview Combine: ok" "previewed with the tools kept"
ui_step "Tool Bodies"                    ui_click_logged "Panel Combine input tools"
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

ui_finish "UI component combine test"
