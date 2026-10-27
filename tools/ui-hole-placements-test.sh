#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/HoleCommands.cpp app/commands/PlaceInputs.cpp core/model/src/features/hole.rs
# check-all sources: tools/cli
# The Hole panel edits holes placed as the .f3d import writes them
# (mitcad#733) through the real UI, on a design mitcad-cli makes: a
# 60 x 40 x 20 block (Sketch1 F1, Extrude1 F2), a 5 mm flat hole 10 deep on
# its top face 10 and 15 mm from two of its top edges (Hole1 F3,
# `face_offsets`) and one started on the XY plane, flipped up into the
# block (Hole2 F4, `plane`); a 20 mm shaft 40 long on XY at (100, 0)
# (Sketch2 F5, Extrude2 F6) with a 4 mm hole 12 mm up from its foot on its
# -X side (Hole3 F7, `cylinder_offset`):
#   1. Hole1 edited: its placement is From Two Edges with Edge Offset 1 and
#      2 at 10 and 15 (not sketch points); Edge Offset 1 typed 1, OK: the
#      hole is 1 mm from the edge, partly outside the block (the block's
#      volume shows it).
#   2. Hole2 edited: its placement is On a Plane; OK keeps it.
#   3. Hole3 edited: its placement is Along a Shaft with Offset 12; Offset
#      typed 20, OK: the hole moved, its volume the same.
#   4. Hole2 edited again, tapped with NPT (mitcad#733): Tapered is on, the
#      Diameter shown and filled with the size's tap drill (160.311 mm for
#      6-8 NPT, then 10.721 mm for 1/4-18 NPT); OK saves a tapered thread.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-hole-placements-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-hole-placements.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

cat > "$WORK/holes.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$RECT"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "hole", "diameter": 5, "flat": true,
    "extent": {"type": "distance", "depth": 10},
    "placement": {"type": "face_offsets", "body": "F2.b0", "face": "F2:end($RECT)",
      "edge1": "E{F2:end($RECT)|F2:side(c1[c4,c2])}", "offset1": 10,
      "edge2": "E{F2:end($RECT)|F2:side(c4[c3,c1])}", "offset2": 15}}},
  {"cmd": "add_feature", "def": {"type": "hole", "diameter": 5, "flat": true, "flip": true,
    "extent": {"type": "distance", "depth": 10},
    "placement": {"type": "plane", "plane": "xy", "points": [[45, 30, 0]]}}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F5", "center": [100, 0], "diameter": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F5", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 40}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "hole", "diameter": 4, "flat": true,
    "extent": {"type": "distance", "depth": 5},
    "placement": {"type": "cylinder_offset", "body": "F6.b0", "face": "F6:side(c1)",
      "plane": "xy", "offset": 12, "toward": [80, 0, 12]}}}
]
EOF
"$CLI" run "$WORK/holes.json" --save "$WORK/holes.mitcad" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$WORK/holes.mitcad"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Bodies shown: F2.b0" "the design shown"

echo "--- 1. A hole at distances from two edges"
ui_mark
ui_step "double-click Hole1"               ui_double_click_logged "Timeline Hole1"
ui_expect_new "Editing F3 with Hole" "Hole1 opens for editing" 30
ui_expect_new "Hole placement kept: face_offsets" "its placement kept as it is"
ui_expect_new "Panel Hole input kept_offset1 at" "Edge Offset 1 listed"
ui_expect_new "Panel Hole input kept_offset2 at" "Edge Offset 2 listed"
for offset in kept_offset1 kept_offset2; do
  grep "Panel Hole input $offset at" <(tail -n +"$UI_MARK" "$UI_LOG") | tail -1 | grep -q "(hidden)" &&
    ui_fail "$offset hidden"
done
grep "Panel Hole input points at" <(tail -n +"$UI_MARK" "$UI_LOG") | grep -qv "(hidden)" &&
  ui_fail "the placement is taken for sketch points"
ui_mark
ui_step "edge offset 1: 1"                 ui_type_in "Panel Hole input kept_offset1" "1"
ui_expect_new "Preview Hole: ok" "previewed 1 mm from the edge" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F3" "Hole1 edited" 30
# The 5 mm hole's circle 1 mm from the edge: the segment beyond it is
# outside the block (r^2 acos(d/r) - d sqrt(r^2 - d^2)), the rest 10 deep;
# and Hole2's whole circle 10 deep.
ui_expect_volume "Body [^ ]* (F2\.b0): volume [0-9.]* -> \([0-9.]*\) mm3" \
  "$(awk 'BEGIN { r = 2.5; d = 1; pi = 3.14159265358979;
                  out = r * r * atan2(sqrt(r * r - d * d), d) - d * sqrt(r * r - d * d);
                  printf "%.6f", 48000 - 10 * (pi * r * r - out) - 10 * pi * r * r }')" \
  "the hole 1 mm from the edge"

echo "--- 2. A hole started on a plane"
ui_mark
ui_step "double-click Hole2"               ui_double_click_logged "Timeline Hole2"
ui_expect_new "Editing F4 with Hole" "Hole2 opens for editing" 30
ui_expect_new "Hole placement kept: plane" "its plane kept as it is"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F4" "Hole2 saved as it was" 30

echo "--- 3. A hole along a shaft"
ui_mark
ui_step "double-click Hole3"               ui_double_click_logged "Timeline Hole3"
ui_expect_new "Editing F7 with Hole" "Hole3 opens for editing" 30
ui_expect_new "Hole placement kept: cylinder_offset" "its shaft placement kept as it is"
ui_expect_new "Panel Hole input kept_offset at" "Offset listed"
grep "Panel Hole input kept_offset at" <(tail -n +"$UI_MARK" "$UI_LOG") | tail -1 | grep -q "(hidden)" &&
  ui_fail "Offset hidden"
ui_mark
ui_step "offset: 20"                       ui_type_in "Panel Hole input kept_offset" "20"
ui_expect_new "Preview Hole: ok" "previewed 20 mm up" 30
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F7" "Hole3 edited" 30

echo "--- 4. A hole tapped with a taper pipe thread"
ui_mark
ui_step "double-click Hole2"               ui_double_click_logged "Timeline Hole2"
ui_expect_new "Editing F4 with Hole" "Hole2 opens again" 30
ui_mark
ui_step "Tapped"                           ui_choose "Panel Hole input tap" 1
ui_step "Thread Type NPT"                  ui_choose "Panel Hole input standard" 3
# NPT starts at its size 6: the 6-8 NPT tap drill.
ui_expect_new "Hole tap drill: 160.311 mm" "the Diameter filled with the size's tap drill" 30
ui_step "Size 1/4"                         ui_choose "Panel Hole input size" -13
ui_expect_new "Hole tap drill: 10.721 mm" "the 1/4-18 NPT tap drill" 30
for input in diameter taper_thread; do
  grep "Panel Hole input $input at" "$UI_LOG" | tail -1 | grep -q "(hidden)" && ui_fail "$input hidden"
done
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F4" "Hole2 tapped" 30
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI hole placements test"
