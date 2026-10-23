#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindow.cpp app/MainWindowVisibility.cpp app/MainWindowParameterPicks.cpp
# check-all sources: app/view/CommandSketches.hpp app/OcctViewer.cpp app/framework/CommandSession.cpp
# check-all sources: app/commands/ExtrudeCommands.cpp app/browser tools/cli
# The sketches a command shows for its time (mitcad#546), through the real
# UI; the Windows test is tools/ui-windows-edit-sketches-test.ps1:
#   - A design (mitcad-cli): Sketch1 (F1) with two 20 x 20 squares, the
#     first extruded by Extrude1 (F2), so Sketch1 is hidden as a used
#     sketch is; Sketch2 (F3) with one square extruded by Extrude2 (F4),
#     its light bulb turned on.
#   - Edit Extrude1: Sketch1 shown for the edit, its square can be picked
#     (taken away and picked again), 15 mm, OK: Sketch1 hidden again.
#   - Edit Extrude1, Esc: shown, then hidden again.
#   - Edit Extrude2: Sketch2, shown by its light bulb, is not shown "for the
#     edit" and stays shown after OK.
#   - The saved design: Sketch1's light bulb not set, Sketch2's on, Body1
#     15 mm high: nothing of the edits' showing went to the model.
#   - A new Extrude takes Sketch1's free square: Sketch1 shown for it, Esc
#     hides it again.
#   - Edit Extrude1 with Sketch1's light bulb turned on meanwhile, Esc:
#     Sketch1 stays shown.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-edit-sketches-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-edit-sketches.XXXXXX)
FILE=$WORK/sketches.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 20, "height": 20},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 40], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [40, 0], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "$RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "set_feature_visible", "uid": "F3", "visible": true}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# last_shown: the sketches the view shows, as last logged.
last_shown() {
  local line
  line=$(grep "Sketches shown: " "$UI_LOG" | tail -1)
  echo "${line#*Sketches shown: }"
}

# expect_shown "F3" "description": the view shows those sketches now.
expect_shown() {
  local shown
  shown=$(last_shown)
  [ "$shown" = "$1" ] || ui_fail "$2: the view shows '$shown'"
  echo "ok   $2"
}

# not_since "text" "description": no line since the mark has the text.
not_since() {
  ui_sync
  sleep 0.5
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qF "$1" && ui_fail "$2: '$1' is in the log"
  echo "ok   $2"
}

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Bodies shown: F2.b0, F4.b0" "both blocks shown"
ui_expect_log "Timeline Extrude2 at" "the timeline is logged"
expect_shown "F3" "Sketch2 shown by its light bulb, Sketch1 hidden as it is used"

echo "--- Edit Extrude1: Sketch1 shown for the edit, its square picked; OK"
ui_mark
ui_step "edit Extrude1 (double-click)"   ui_double_click_logged "Timeline Extrude1"
ui_expect_new "Editing F2 with Extrude" "Extrude1 opens for editing" 30
ui_expect_new "Sketch Sketch1 shown for the edit of Extrude1" "Sketch1 shown for the edit"
ui_expect_new "Sketches shown: F1" "the view shows Sketch1 (Sketch2 is after the marker)"
ui_expect_new "Pick profile F1/$RECT at" "Sketch1's square can be picked"
ui_mark
ui_step "take the square away"           ui_click_pick "profile F1/$RECT"
ui_expect_new "Extrude Profiles: nothing" "the square taken away"
ui_mark
ui_step "pick the square again"          ui_click_pick "profile F1/$RECT"
ui_expect_new "Extrude Profiles: 1 profile [profile $RECT of F1]" "the square picked again"
ui_mark
ui_step "distance 15"                    ui_type_in "Panel Extrude input distance" "15"
ui_expect_new "Preview Extrude: ok" "previewed" 30
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F2" "Extrude1 edited" 30
ui_expect_new "Sketch Sketch1 hidden again after the edit of Extrude1" "Sketch1 hidden again"
expect_shown "F3" "the view shows Sketch2 only again"

echo "--- Edit Extrude1, Esc"
ui_mark
ui_step "edit Extrude1 (double-click)"   ui_double_click_logged "Timeline Extrude1"
ui_expect_new "Editing F2 with Extrude" "Extrude1 opens for editing" 30
ui_expect_new "Sketch Sketch1 shown for the edit of Extrude1" "Sketch1 shown for the edit"
ui_expect_new "Sketches shown: F1" "the view shows Sketch1"
ui_mark
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command Extrude cancelled" "the edit cancelled"
ui_expect_new "Sketch Sketch1 hidden again after the edit of Extrude1" "Sketch1 hidden again"
expect_shown "F3" "the view shows Sketch2 only again"

echo "--- Edit Extrude2: Sketch2 is shown by its light bulb"
ui_mark
ui_step "edit Extrude2 (double-click)"   ui_double_click_logged "Timeline Extrude2"
ui_expect_new "Editing F4 with Extrude" "Extrude2 opens for editing" 30
ui_expect_new "Pick profile F3/$RECT at" "Sketch2's square can be picked"
not_since "shown for the edit" "no sketch shown only for the edit"
expect_shown "F3" "the view shows Sketch2, not Sketch1"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F4" "Extrude2 edited" 30
not_since "hidden again" "nothing hidden again"
expect_shown "F3" "Sketch2 stays shown"

echo "--- The saved design: no visibility change"
ui_mark
ui_step "save (Ctrl+S)"                  ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
cat > "$WORK/check.json" << 'EOF'
[
  {"expect": {"query": {"query": "timeline"}, "result": {"features": [
    {"uid": "F1", "visible": false, "visible_set": false}, {"uid": "F2"},
    {"uid": "F3", "visible": true, "visible_set": true}, {"uid": "F4"}]}}},
  {"expect": {"body": "Body1", "volume": 6000, "tolerance": 1e-6}}
]
EOF
"$CLI" run "$WORK/check.json" --open "$FILE" > "$WORK/check.log" 2>&1 || {
  cat "$WORK/check.log"
  ui_fail "the saved design"
}
echo "ok   Sketch1's light bulb not set, Sketch2's on, Extrude1 15 mm"

echo "--- A new Extrude takes Sketch1's free square"
ui_mark
ui_step "extrude (E)"                    ui_key e
ui_expect_new "Command Extrude started" "Extrude started"
ui_expect_new "Extrude Profiles: 1 profile [profile " "it takes a profile"
ui_expect_new "Sketch Sketch1 shown for Extrude" "Sketch1 shown for the command"
ui_expect_new "Sketches shown: F1, F3" "the view shows both sketches"
ui_mark
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command Extrude cancelled" "Extrude cancelled"
ui_expect_new "Sketch Sketch1 hidden again after Extrude" "Sketch1 hidden again"
expect_shown "F3" "the view shows Sketch2 only again"

echo "--- Sketch1's light bulb turned on during the edit"
ui_mark
ui_step "edit Extrude1 (double-click)"   ui_double_click_logged "Timeline Extrude1"
ui_expect_new "Editing F2 with Extrude" "Extrude1 opens for editing" 30
ui_expect_new "Sketch Sketch1 shown for the edit of Extrude1" "Sketch1 shown for the edit"
ui_expect_log "Browser eye Root/Sketches/Sketch1 at" "Sketch1's light bulb in the browser"
ui_mark
ui_step "show Sketch1 (light bulb)"      ui_click_logged "Browser eye Root/Sketches/Sketch1"
ui_expect_new "Visibility Root/Sketches/Sketch1: shown (during Extrude)" "Sketch1's light bulb on during the edit"
ui_mark
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command Extrude: 1 visibility change(s) kept" "the light bulb kept"
ui_expect_new "Command Extrude cancelled" "the edit cancelled"
ui_expect_new "Sketch Sketch1 stays shown after the edit of Extrude1 (its light bulb)" "Sketch1 stays shown"
expect_shown "F1, F3" "the view shows both sketches"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI edit sketches test"
