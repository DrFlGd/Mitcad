#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands core/model geometry
# Extrude's Wrap Onto (mitcad#473) through the real UI: a 6 x 6 rectangle
# on a plane through a rod's axis extruded 1, wrapped onto the rod's side
# and joined, then edited to flipped and cut (engraved); the volumes of
# tools/cli/tests/wrap.json.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-wrap-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-wrap.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

# A rod of diameter 20, 30 long (F1, F2), and on XZ (through its axis) a
# 6 x 6 rectangle from z = 2 to 8 (F3).
cat > "$WORK/rod.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 30}, "operation": "new_body"}},
  {"cmd": "sketch.create", "plane": "xz"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [-3, -8], "width": 6, "height": 6}
]
EOF
"$CLI" run "$WORK/rod.json" --save "$WORK/rod.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli rod"; }

ui_start_display
ui_start_app --open "$WORK/rod.mitcad"
ui_step "fit (F6)"                         ui_key F6

echo "--- Extrude wrapped onto the rod"
ui_step "extrude (E)"                      ui_key e
ui_expect_log "Extrude Profiles: 1 profile [profile $RECT of F3]" "the newest profile taken"
ui_expect_log "Panel Extrude input wrap at" "the Wrap Onto input"
ui_step "the wrap input"                   ui_click_logged "Panel Extrude input wrap"
ui_step "pick the rod's side"              ui_click_pick "face F2.b0/F2:side(c1)"
ui_expect_log "Extrude Wrap Onto: 1 face" "the cylindrical face picked"
ui_step "distance 1"                       ui_type_in "Panel Extrude input distance" "1"
ui_expect_log "Preview Extrude: ok" "previewed"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log ", distance 1, wrapped onto a cylindrical face" "the extrusion is wrapped"
# 36 mm2 wrapped onto radius 10 and raised to 11: 36 * 21 / 20 more.
ui_expect_volume "Body Body1 (F2.b0): volume 9424.778 -> \([0-9.]*\) mm3" 9462.577961 "raised on the rod"

echo "--- Edited: flipped and cut, engraved"
ui_mark
ui_step "double-click Extrude2"            ui_double_click_logged "Timeline Extrude2"
ui_expect_new "Editing F4 with Extrude" "editing Extrude2"
ui_expect_new "Extrude Wrap Onto: 1 face" "the face loaded"
ui_step "flip"                             ui_click_logged "Panel Extrude input flip"
# The stored Join was the automatic choice, so the edit follows the
# geometry: flipped into the rod, it cuts (779aed86).
ui_expect_new "Extrude: automatic Operation = Cut" "Cut chosen automatically"
ui_expect_new "Preview Extrude: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F4" "Extrude2 edited"
# Sunk to radius 9: 36 * 19 / 20 less than the rod.
ui_expect_volume "Body Body1 (F2.b0): volume [0-9.]* -> \([0-9.]*\) mm3" 9390.577961 "engraved into the rod"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI wrap test"
