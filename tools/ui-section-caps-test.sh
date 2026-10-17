#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/OcctViewer.cpp app/OcctViewer.hpp app/MainWindow.cpp app/MainWindow.hpp app/browser tools/cli
# The caps of Section Analysis through the real UI (mitcad#382), on a design
# of the root and two components, each with a block:
#   - Each component's caps take its timeline colour (the band under its
#     features); the root's keep the default colour. One group per colour,
#     hatched in a darker shade of its own, in turn at 45 and 135 degrees.
#   - The caps keep their colours and their hatch in other visual styles and
#     with the bodies drawn in component colours.
#   - Remove Section Analysis takes the caps away.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-section-caps-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-section-caps.XXXXXX)
FILE=$WORK/blocks.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

# Part (the timeline's first colour) at y = 0, Other (the second) at y = 30,
# the root's block at y = 60; all three cross x = 10.
cat > "$WORK/model.json" << EOF
[
  {"cmd": "create_component", "name": "Part"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "create_component", "name": "Other"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [0, 30], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F5", "corner": [0, 60], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F5", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# The palette of app/browser/ComponentColors.hpp: the first and second
# component, and the default cap colour.
ROOT=#f29e1a
PART=#e0892a
OTHER=#3aa35b
HATCH='#[0-9a-f]\{6\}'

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Bodies shown: " "the blocks shown"

echo "--- Section Analysis at x = 10"
ui_mark
ui_step "section analysis (search)"        ui_command "Section Analysis"
ui_step "pick YZ"                          ui_click_logged "Datum yz"
ui_expect_new "Section Analysis Faces/Plane: 1 plane [plane yz]" "YZ picked"
ui_step "10 along X"                       ui_type_in "Panel Section Analysis input offset" "10"
ui_expect_log "Section caps: 3 face(s) at" "three blocks cut"
ui_expect_log "Section caps: 3 colour(s) [" "one group of caps per colour"
CAPS=$(grep "Section caps: 3 colour(s)" "$UI_LOG" | tail -1)
echo "$CAPS" | grep -q "$ROOT hatch $HATCH at 45 deg, [1-9][0-9]* lines" ||
  ui_fail "the root's caps are not $ROOT with a hatch at 45 degrees: $CAPS"
echo "$CAPS" | grep -q "$PART hatch $HATCH at 135 deg, [1-9][0-9]* lines" ||
  ui_fail "Part's caps are not $PART with a hatch at 135 degrees: $CAPS"
echo "$CAPS" | grep -q "$OTHER hatch $HATCH at 45 deg, [1-9][0-9]* lines" ||
  ui_fail "Other's caps are not $OTHER with a hatch at 45 degrees: $CAPS"
echo "ok   caps: $CAPS"
# The hatch is darker than the cap, in each group.
echo "$CAPS" | sed 's/.*\[\(.*\)\] in.*/\1/; s/; /\n/g' | while read -r cap hatch colour _; do
  [ "$hatch" = hatch ] || ui_fail "unexpected caps entry: $cap $hatch $colour"
  [ "$cap" != "$colour" ] || ui_fail "the hatch of $cap has the cap's own colour"
done
[ -n "$SHOT" ] && ui_capture "$SHOT"

ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Closed Section Analysis" "closed, the section kept"

echo "--- Other visual styles"
ui_mark
ui_step "wireframe (search)"               ui_command "Wireframe"
ui_expect_new "] in Wireframe, bodies in" "the caps drawn in the wireframe style"
ui_step "shaded (search)"                  ui_command "Shaded"
ui_expect_new "] in Shaded, bodies in" "and in the shaded style"
ui_mark
ui_step "component colours (search)"       ui_command "Component Colors"
ui_expect_new "bodies in component colours" "the bodies in component colours"
CAPS_NOW=$(grep "Section caps: 3 colour(s)" "$UI_LOG" | tail -1 | sed 's/\] in.*/]/')
[ "$CAPS_NOW" = "$(echo "$CAPS" | sed 's/\] in.*/]/')" ] || ui_fail "the caps changed with the bodies' colours: $CAPS_NOW"
echo "ok   the caps keep their colours and hatch"

echo "--- Remove"
ui_mark
ui_step "remove section analysis (search)" ui_command "Remove Section Analysis"
ui_expect_new "Section caps: none" "the caps are gone"

ui_finish "UI section caps test"
