#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser app/framework app/MainWindow.cpp app/MainWindowVisibility.cpp tools/cli
# The browser's light bulbs in every mode (mitcad#295) and Create Sketch's
# origin planes (mitcad#294) through the real UI. A design without
# components: Body1 (20 x 20 x 10) inside Body2 (60 x 60 x 30), and Sketch3
# with a free 20 x 20 square beside them:
#   - Create Sketch shows the origin planes; no face of Body1 can be picked.
#     Body2 hidden by its light bulb while the plane is asked for: the pick
#     goes on, Body1's top face can be picked and the sketch starts on it.
#   - Extrude of the square, Body1 hidden meanwhile: the preview is drawn
#     again without Body1, not computed again; Cancel keeps Body1 hidden
#     (the change is the last undo step).
#   - Extrude again, Body1 hidden meanwhile, OK: one undo step takes the
#     extrusion back and leaves Body1 hidden; the next one shows it.
#   - Edit of Extrude2 (the timeline rolled back), Body1 hidden meanwhile,
#     Cancel: Body1 stays hidden, as the last undo step (no marker move
#     left behind).
#   - A Flat Pattern's or an analysis's light bulb and the other browser
#     actions still wait for the command: Rename says so.
# The same design with a component (Part) and the root active:
#   - Create Sketch does not show the origin planes, says how to get them;
#     a face of Body2 can be picked.
#   - Create Sketch offers the hidden origin planes in the browser
#     (mitcad#694): the Origin folder opens with XY, XZ and YZ pickable, its
#     light bulb unchanged; a click on XZ starts the sketch on it, and the
#     folder folds back.
#   - The Origin folder's light bulb during the pick shows them, and an
#     origin plane can be picked.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-visibility-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-visibility.XXXXXX)
FILE=$WORK/blocks.mitcad
PARTS=$WORK/parts.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
# Sketch1 (F1) 20 x 20 at (40, -60), Extrude1 (F2) 10 up: Body1 (F2.b0).
# Sketch2 (F3) 60 x 60 around it, Extrude2 (F4) 30 up: Body2 (F4.b0),
# Body1 inside it. Sketch3 (F5) 20 x 20 at (100, -140): a profile in front of
# the blocks as the view looks at them.
BLOCKS=$(cat << EOF
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [40, -60], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [20, -80], "width": 60, "height": 60},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "$R"}],
    "extent": {"type": "distance", "distance": 30}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F5", "corner": [100, -140], "width": 20, "height": 20}
EOF
)
printf '[\n%s\n]\n' "$BLOCKS" > "$WORK/blocks.json"
# The same with Part:1 (O1, C1) beside them: Sketch4 (F6) 20 x 20, Extrude3
# (F7) 10 up; the root active again.
cat > "$WORK/parts.json" << EOF
[
$BLOCKS,
  {"cmd": "create_component", "name": "Part", "transform": {"translation": [0, 120, 0]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F6", "corner": [0, 0], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F6", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"}
]
EOF
"$CLI" run "$WORK/blocks.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
"$CLI" run "$WORK/parts.json" --save "$PARTS" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# since_mark "text": whether a line logged after the mark has the text.
since_mark() { tail -n +$((${UI_MARK:-0} + 1)) "$UI_LOG" | grep -qF -- "$1"; }

# expect_none "text" description: no line since the mark has the text.
expect_none() {
  ui_sync
  since_mark "$1" && ui_fail "$2: '$1' logged"
  echo "ok   $2"
}

# last_line "prefix": the last line logged that starts with the prefix.
last_line() { grep -F -- "$1" "$UI_LOG" | tail -1; }

# extrude_square: Extrude, which takes the newest free profile (the
# square), its preview computed with both blocks drawn.
extrude_square() {
  ui_mark
  ui_step "extrude (E)"                  ui_key e
  ui_expect_new "Command Extrude started" "Extrude started"
  ui_expect_new "Preview Extrude: ok" "the square's preview computed"
  ui_expect_new "Preview bodies: " "the preview drawn"
  last_line "Preview bodies: " | grep -qF "F2.b0 at" || ui_fail "the preview does not draw Body1"
  echo "ok   the preview draws Body1"
}

# hide_body1 command: Body1's light bulb during the command; the preview
# drawn again without it, not computed again.
hide_body1() {
  ui_mark
  ui_step "hide Body1"                   ui_click_logged "Browser eye Root/Bodies/Body1"
  ui_expect_new "Visibility Root/Bodies/Body1: hidden (during $1)" "Body1 hidden during $1"
  ui_expect_new "Preview bodies: " "the preview drawn again"
  last_line "Preview bodies: " | grep -qF "F2.b0 at" && ui_fail "the preview still draws Body1"
  last_line "Preview bodies: " | grep -qF "F4.b0 at" || ui_fail "the preview lost Body2"
  echo "ok   the preview without Body1, with Body2"
  expect_none "Preview $1: " "the preview not computed again"
}

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Bodies shown: F2.b0, F4.b0" "both blocks shown"

echo "--- Create Sketch: the origin planes; Body2 hidden while a plane is asked for"
ui_mark
ui_step "create sketch (search)"         ui_command "Create Sketch"
ui_expect_new "Create Sketch: select a plane" "Create Sketch asks for a plane"
ui_expect_new "Create Sketch: origin planes shown" "no components: the origin planes shown (mitcad#294)"
ui_expect_new "Datum xy at" "the XY plane can be picked"
ui_expect_new "Pick places:" "the faces' pick places logged"
expect_none "Pick face F2.b0/" "Body1, inside Body2, cannot be picked"
ui_mark
ui_step "hide Body2"                     ui_click_logged "Browser eye Root/Bodies/Body2"
ui_expect_new "Visibility Root/Bodies/Body2: hidden (during Create Sketch)" "Body2 hidden during the plane pick"
ui_expect_new "Bodies shown: F2.b0" "only Body1 shown"
ui_expect_new "Pick face F2.b0/F2:end(" "Body1's top face can be picked now"
expect_none "Create Sketch is waiting for a plane" "the light bulb was not refused"
ui_step "pick Body1's top face"          ui_click_pick "face F2.b0/F2:end("
ui_expect_new 'Sketch started on {"body":"F2.b0","face":"F2:end(' "the sketch started on Body1's top face"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "the empty sketch finished"
ui_mark
ui_step "show Body2"                     ui_click_logged "Browser eye Root/Bodies/Body2"
ui_expect_new "Visibility Root/Bodies/Body2: shown" "Body2 shown again"
expect_none "(during" "no command runs"

echo "--- Extrude, Body1 hidden meanwhile, Cancel"
extrude_square
hide_body1 Extrude
ui_mark
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command Extrude cancelled" "Extrude cancelled"
expect_none "Bodies shown: F2.b0" "Body1 stays hidden"
ui_mark
ui_step "undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: Hide Body1" "the light bulb is the last undo step, kept by Cancel"
ui_expect_new "Bodies shown: F2.b0, F4.b0" "Body1 shown by the undo"

echo "--- Extrude, Body1 hidden meanwhile, OK: one undo step for the extrusion"
extrude_square
hide_body1 Extrude
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Added Extrude as F" "Extrude3 added"
ui_mark
ui_step "undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: Add Extrude3" "one undo step takes the extrusion back"
expect_none "Bodies shown: F2.b0" "Body1 stays hidden after that undo"
ui_mark
ui_step "undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: Hide Body1" "the light bulb is an undo step of its own"
ui_expect_new "Bodies shown: F2.b0, F4.b0" "Body1 shown by it"

echo "--- Edit of Extrude2 (rolled back), Body1 hidden meanwhile, Cancel"
ui_expect_log "Timeline Extrude2 at" "Extrude2 in the timeline"
ui_mark
ui_step "edit Extrude2 (double-click)"   ui_double_click_logged "Timeline Extrude2"
ui_expect_new "Editing F4 with Extrude" "editing Extrude2"
ui_expect_new "Bodies shown: F2.b0" "the timeline rolled back to before Body2"
ui_mark
ui_step "hide Body1"                     ui_click_logged "Browser eye Root/Bodies/Body1"
ui_expect_new "Visibility Root/Bodies/Body1: hidden (during Extrude)" "Body1 hidden during the edit"
ui_mark
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command Extrude: 1 visibility change(s) kept" "the change run again after the roll-back"
ui_expect_new "Command Extrude cancelled" "the edit cancelled"
ui_expect_new "Bodies shown: F4.b0" "Body2 back, Body1 hidden"
ui_mark
ui_step "undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: Hide Body1" "the light bulb is the last undo step, no marker move left"
ui_expect_new "Bodies shown: F2.b0, F4.b0" "Body1 shown by the undo"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_stop_app

echo "--- A design with a component: no origin planes by themselves (mitcad#294)"
ui_start_app --open "$PARTS"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Bodies shown: " "the blocks shown"
ui_mark
ui_step "create sketch (search)"         ui_command "Create Sketch"
ui_expect_new "Create Sketch: select a plane" "Create Sketch asks for a plane"
ui_expect_new "Create Sketch: origin planes not shown (the design has components)" "the origin planes not shown"
ui_expect_new "Pick face F4.b0/F4:end(" "Body2's top face can be picked"
expect_none "Datum xy at" "the XY plane is not in the view"
ui_mark
ui_step "activate Part (radio)"          ui_click_logged "Browser radio Root/Part:1"
ui_expect_new "Create Sketch is waiting for a plane: pick one or press Esc." "other browser actions wait for the plane pick"
expect_none "Activated Part" "Part not activated"
ui_step "pick Body2's top face"          ui_click_pick "face F4.b0/F4:end("
ui_expect_new 'Sketch started on {"body":"F4.b0","face":"F4:end(' "the sketch started on Body2's top face"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "the empty sketch finished"

echo "--- The hidden origin planes offered in the browser (mitcad#694)"
ui_mark
ui_step "create sketch (search)"         ui_command "Create Sketch"
ui_expect_new "Create Sketch: origin planes not shown (the design has components)" "the origin planes not shown"
ui_expect_new "Browser offers the origin planes: Root/Origin open; XY, XZ, YZ pickable" \
  "the Origin folder opened, its planes pickable"
ui_expect_new "Browser Root/Origin/XZ at" "the XZ row in sight"
expect_none "Visibility Root/Origin" "the Origin's light bulb not changed"
expect_none "Datum xz at" "the XZ plane is not in the view"
ui_mark
ui_step "click XZ in the browser"        ui_click_logged "Browser Root/Origin/XZ"
ui_expect_new "Sketch started on xz" "the sketch started on the origin's XZ plane"
ui_expect_new "Browser no longer offers the origin planes: Root/Origin folded" "the Origin folder folded back"
ui_expect_new "Browser Root/Origin/XZ out of sight" "the XZ row folded away"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "the empty sketch finished"
expect_none "Browser Root/Origin/XZ at" "the Origin folder stays folded"

echo "--- The Origin folder's light bulb during the pick"
ui_mark
ui_step "create sketch (search)"         ui_command "Create Sketch"
ui_expect_new "Create Sketch: origin planes not shown (the design has components)" "the origin planes not shown"
ui_mark
ui_step "show the origin"                ui_click_logged "Browser eye Root/Origin"
ui_expect_new "Visibility Root/Origin: shown (during Create Sketch)" "the Origin folder shown during the pick"
ui_expect_new "Datum xy at" "the XY plane in the view"
ui_step "pick XY"                        ui_click_logged "Datum xy"
ui_expect_new "Sketch started on xy" "the sketch started on the origin's XY plane"
ui_mark
ui_step "finish sketch (Ctrl+Enter)"     ui_key ctrl+Return
ui_expect_new "Sketch finished" "the empty sketch finished"

echo "--- During Extrude: a component's light bulb works, other browser actions wait"
ui_mark
ui_step "extrude (E)"                    ui_key e
ui_expect_new "Command Extrude started" "Extrude started"
ui_expect_new "Preview Extrude: ok" "the square's preview computed"
ui_mark
ui_step "hide Part:1"                    ui_click_logged "Browser eye Root/Part:1"
ui_expect_new "Visibility Root/Part:1: hidden (during Extrude)" "Part:1 hidden during Extrude"
ui_mark
ui_step "activate Part (radio)"          ui_click_logged "Browser radio Root/Part:1"
ui_expect_new "Finish the command or the sketch first." "other browser actions wait for the command"
expect_none "Activated Part" "Part not activated"
ui_mark
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command Extrude cancelled" "Extrude cancelled"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI visibility test"
