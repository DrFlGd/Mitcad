#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser/ReorientDialog.cpp app/browser/BrowserController.cpp app/MainWindow.cpp tools/cli
# check-all sources: core/model/src/reorient.rs core/model/src/document/reorient.rs core/model/src/api/reorient.rs
# Reorient Design (mitcad#502) through the real UI, on a design mitcad-cli
# makes: Block, 60 x 40 x 20 from a sketch on XY with a hole through its
# top.
#   - The root's context menu opens Reorient Design, which shows the design
#     turned 90 degrees about X at once (a preview of the model command on
#     a checkpoint); Z (Alt+Z) previews about Z, X (Alt+X) about X again,
#     and OK keeps it: the block's box is turned (y -20..0, z 0..40).
#   - Undo takes it back as one step, "Reorient Design".
#   - Opened again, 180 degrees (Alt+1) previews and Cancel (Esc) takes it
#     back with the history: Redo still redoes the first turn.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-reorient-design-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-reorient.XXXXXX)
FILE=$WORK/design.mitcad
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT

R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$R"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "hole",
    "placement": {"type": "face", "body": "F2.b0", "face": "F2:end($R)", "points": [[15, 20, 20]]},
    "diameter": 8, "extent": {"type": "through_all"}}},
  {"cmd": "rename_body", "uid": "F2.b0", "name": "Block"}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE" --no-native-dialogs
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Browser Root at" "the browser shows the root"

echo "--- The root's menu: Reorient Design, previewed"
ui_mark
ui_step "right-click Root"               ui_click_logged "Browser Root" 3
ui_expect_new "Context menu: " "its menu opened"
ui_step "choose Reorient Design..."      ui_menu_choose "Reorient Design..."
ui_expect_new "Reorient Design dialog opened" "the dialog opened"
ui_expect_new "Reorient Design preview: 90 degrees about X: " "90 about X previewed at once"
ui_focus_dialog '^Reorient Design$'
ui_mark
ui_step "about Z (Alt+Z)"                ui_key alt+z
ui_expect_new "Reorient Design preview: 90 degrees about Z: " "about Z previewed"
ui_mark
ui_step "about X (Alt+X)"                ui_key alt+x
ui_expect_new "Reorient Design preview: 90 degrees about X: " "about X again"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_focus_main
ui_expect_new "Reorient Design: 90 degrees about X" "the turn kept"
ui_expect_new "Reorient Design body Block: box (0, -20, 0) - (60, 0, 40)" "the block's box turned about X"

echo "--- Undo: one step"
ui_mark
ui_step "undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: Reorient Design" "one undo step named Reorient Design"

echo "--- Cancel takes the preview back"
ui_mark
ui_step "right-click Root"               ui_click_logged "Browser Root" 3
ui_expect_new "Context menu: " "its menu opened"
ui_step "choose Reorient Design..."      ui_menu_choose "Reorient Design..."
ui_expect_new "Reorient Design preview: 90 degrees about X: " "previewed again"
ui_focus_dialog '^Reorient Design$'
ui_mark
ui_step "180 degrees (Alt+1)"            ui_key alt+1
ui_expect_new "Reorient Design preview: 180 degrees about X: " "180 about X previewed"
ui_mark
ui_step "Cancel (Esc)"                   ui_key Escape
ui_focus_main
ui_expect_new "Reorient Design cancelled" "cancelled"
ui_mark
ui_step "redo (Ctrl+Shift+Z)"            ui_key ctrl+shift+z
ui_expect_new "Redo: Reorient Design" "Cancel left the history as it was"

ui_finish "UI reorient design test"
