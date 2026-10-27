#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework app/commands app/OcctViewer.cpp app/OcctViewer.hpp
# check-all sources: geometry/src/tangent_chain.cpp geometry/src/face_select.cpp core/model-api/src tools/cli
# The hover of a tangent chain (mitcad#290) through the real UI, on a
# 60 x 40 x 30 mm box with one vertical edge rounded (8 mm):
#   - Fillet: hovering the top front edge shows the whole tangent chain,
#     that edge, the rounding's top edge and the top right edge (3 edges);
#     with Tangent Chain off only the edge.
#   - Chamfer: the same.
#   - Shell: hovering the front face shows the faces the rounding joins
#     (3 faces); with Tangent Chain off only the face.
#   - The pick stays the edge or face clicked.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-hover-chain-test.sh

UI_APP=${UI_APP:-$(cd "$(dirname "$0")/.." && pwd)/build/dev/app/mitcad}
BUILD_DIR=$(cd "$(dirname "$UI_APP")/.." 2> /dev/null && pwd)
source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$BUILD_DIR/tools/cli/mitcad-cli}
WORK=$(mktemp -d "$BUILD_DIR/ui-hover-chain.XXXXXX")
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT
TOP_FRONT='edge F1.b0/E{F1:side0|F1:top}'
FRONT='face F1.b0/F1:side0'

cat > "$WORK/box.json" << EOF
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
    "length": 60, "width": 40, "height": 30, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "fillet", "body": "F1.b0", "edges": ["E{F1:side0|F1:side1}"],
    "radius": 8}},
  {"expect": {"feature": "Fillet1", "status": "ok"}}
]
EOF
"$CLI" run "$WORK/box.json" --save "$WORK/box.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# off: the pointer off the box (the hover shows nothing).
off() { xdotool mousemove $(ui_view_at 3 50); }

# chain command input "pick text" kind count: in the command's panel
# (opened, its pick places logged), hovering the item shows `count` items
# of the kind with Tangent Chain on and one with it off; Tangent Chain is
# turned on again after.
chain() {
  local command=$1 input=$2 item=$3 kind=$4 count=$5
  ui_step "pointer off the box" off
  ui_mark
  ui_step "hover the $kind" xdotool mousemove $(ui_pick_at "$item")
  ui_expect_new "Hover chain: $count $kind(s) of F1.b0" "$command: the hover shows the tangent chain ($count ${kind}s)"
  ui_step "pointer off the box" off
  ui_mark
  ui_step "Tangent Chain off" ui_click_logged "Panel $command input tangent_chain"
  ui_step "hover the $kind again" xdotool mousemove $(ui_pick_at "$item")
  ui_expect_new "Hover chain: 1 $kind(s) of F1.b0" "$command: without Tangent Chain the hover shows the $kind alone"
  ui_step "pointer off the box" off
  ui_mark
  ui_step "Tangent Chain on" ui_click_logged "Panel $command input tangent_chain"
  ui_step "hover the $kind once more" xdotool mousemove $(ui_pick_at "$item")
  ui_expect_new "Hover chain: $count $kind(s) of F1.b0" "$command: with Tangent Chain on again the whole chain"
  # The pick stays the item clicked.
  ui_mark
  ui_step "pick the $kind" xdotool click 1
  ui_expect_new "$command $input: 1 $kind [" "$command: the pick is the one $kind"
  ui_step "cancel (Esc)" ui_key Escape
  ui_expect_log "Command $command cancelled" "$command cancelled"
}

ui_start_display
ui_start_app --open "$WORK/box.mitcad" --no-native-dialogs
ui_expect_log "Opened $WORK/box.mitcad" "the box opened"
ui_step "fit (F6)" ui_key F6

echo "--- Fillet"
ui_mark
ui_step "fillet (F)" ui_key f
ui_expect_new "Pick $TOP_FRONT at" "Fillet: the pick places are logged"
ui_expect_log "Panel Fillet input tangent_chain at" "Fillet: the panel has Tangent Chain"
chain Fillet Edges "$TOP_FRONT" edge 3
ui_expect_log "Tangent chains of F1.b0 (edges): 2 in " "the box's edge chains asked once, with the time"
[ "$(grep -c "Tangent chains of F1.b0 (edges)" "$UI_LOG")" = 1 ] ||
  ui_fail "the chains were asked more than once in the command run"
echo "ok   the chains asked once in the command run"

echo "--- Chamfer"
ui_mark
ui_step "chamfer" ui_command "Chamfer"
ui_expect_new "Pick $TOP_FRONT at" "Chamfer: the pick places are logged"
ui_expect_log "Panel Chamfer input tangent_chain at" "Chamfer: the panel has Tangent Chain"
chain Chamfer Edges "$TOP_FRONT" edge 3

echo "--- Shell"
ui_mark
ui_step "shell" ui_command "Shell"
ui_expect_new "Pick $FRONT at" "Shell: the pick places are logged"
ui_expect_log "Panel Shell input tangent_chain at" "Shell: the panel has Tangent Chain"
chain Shell "Faces/Body" "$FRONT" face 3
ui_expect_log "Tangent chains of F1.b0 (faces): 1 in " "the box's face chains asked"

ui_finish "the hover of a tangent chain"
