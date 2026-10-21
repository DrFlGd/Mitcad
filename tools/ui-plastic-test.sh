#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands tools/cli
# Plastic Feature (mitcad#459) through the real UI, with the volumes the
# model reports: a boss clicked on a plate's top (a cylinder 6 across and 10
# high: pi 9 10 = 282.743 mm3 more), then a groove along the opening of a
# box shelled through its top (1 wide and 2 deep into the wall:
# (70 40 - 68 38) 2 = 432 mm3 less), the type chosen in the panel.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-plastic-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-plastic.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# A 60 x 60 x 3 plate (Box1 F1, Body1) and an 80 x 50 x 30 box (Box2 F2,
# Body2) shelled 6 thick through its top (Shell1 F3): 57984 mm3 (a rim wide
# enough to click).
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"},
    "corner": [0, 0], "length": 60, "width": 60, "height": 3, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"},
    "corner": [100, 0], "length": 80, "width": 50, "height": 30, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "shell", "body": "F2.b0", "faces": ["F2:top"], "inside": 6}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$WORK/model.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli model"; }

ui_start_display
ui_start_app --open "$WORK/model.mitcad"
ui_step "fit (F6)"                         ui_key F6

echo "--- A boss on the plate"
ui_step "Plastic Feature"                  ui_command "Plastic Feature"
ui_expect_log "Command Plastic Feature started" "the panel opened"
ui_step "click the plate's top"            ui_click_pick "face F1.b0/F1:top"
ui_expect_log "Plastic Feature Face: 1 face" "the face picked"
ui_step "diameter 6"                       ui_type_in "Panel Plastic Feature input diameter" "6"
ui_step "height 10"                        ui_type_in "Panel Plastic Feature input height" "10"
ui_expect_log "Preview Plastic Feature: ok" "previewed"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added plastic feature (Boss)" "boss added"
ui_expect_volume "Body Body1 (F1.b0): volume 10800.000 -> \([0-9.]*\) mm3" 11082.743339 "the boss's post"

echo "--- A groove along the shelled box's opening"
ui_step "Plastic Feature"                  ui_command "Plastic Feature"
ui_expect_log "Command Plastic Feature started" "the panel opened again"
ui_step "Groove"                           ui_choose "Panel Plastic Feature input type" 2
ui_mark
ui_step "the face input"                   ui_click_logged "Panel Plastic Feature input face"
ui_step "click the rim"                    ui_click_pick "face F2.b0/F3:offset_cap(F2:top)"
ui_expect_new "Plastic Feature Face: 1 face" "the rim picked"
ui_expect_log "Preview Plastic Feature: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added plastic feature (Groove)" "groove added"
ui_expect_volume "Body Body2 (F2.b0): volume 57984.000 -> \([0-9.]*\) mm3" 57552 "the groove"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_step "undo the groove (Ctrl+Z)"         ui_key ctrl+z
ui_step "undo the boss (Ctrl+Z)"           ui_key ctrl+z

ui_finish "UI plastic test"
