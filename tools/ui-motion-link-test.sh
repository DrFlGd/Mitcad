#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/browser app/MainWindowJoints.cpp
# check-all sources: core/model/src/motion_links.rs core/model/src/features/motion_link.rs core/solver tools/cli
# Motion links (mitcad#466) through the real UI, on a design mitcad-cli
# makes: Plate:1 (a 40 x 40 x 10 block, grounded), Pinion:1 (a disc of
# radius 10 about (20, 30) on it, turning on Joint1) and Rack:1 (a 40 x 10
# x 5 block below it, y from 10 to 20, sliding along x on Joint2):
#   - Motion Link's panel links them (Rack and Pinion, radius 10), keeping
#     them where they are; the browser lists the link in Motion Links.
#   - Drive Joint moves the rack 5 mm: the pinion turns half a radian.
#   - The link's menu edits it; saved and checked by mitcad-cli.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-motion-link-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-motion-link.XXXXXX)
FILE=$WORK/links.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

# Plate (O1): F1, F2 (Body1); Pinion (O2): F3, F4 (Body2); Rack (O3): F5,
# F6 (Body3); Joint1 F7, Joint2 F8.
sed "s/RECT/$RECT/g" > "$WORK/model.json" << 'EOF'
[
  {"cmd": "create_component", "name": "Plate"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 40, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "create_component", "name": "Pinion", "transform": {"translation": [20, 30, 10]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F3", "center": [0, 0], "diameter": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "create_component", "name": "Rack", "transform": {"translation": [0, 10, 10]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F5", "corner": [0, 0], "width": 40, "height": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F5", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "ground_occurrence", "occurrence": "Plate:1"},
  {"cmd": "add_joint", "kind": "revolute", "flip": true,
    "a": {"occurrence": "Pinion:1", "geometry": {"body": "F4.b0", "face": "F4:start(r{c1})"}},
    "b": {"occurrence": "Plate:1", "geometry": {"body": "F2.b0", "face": "F2:end(RECT)"},
          "frame_override": {"origin": [20, 30, 10]}}},
  {"cmd": "add_joint", "kind": "slider", "slide_axis": "x", "flip": true,
    "a": {"occurrence": "Rack:1", "geometry": {"body": "F6.b0", "face": "F6:start(RECT)"}},
    "b": {"occurrence": "Plate:1", "geometry": {"body": "F2.b0", "face": "F2:end(RECT)"},
          "frame_override": {"origin": [20, 15, 10]}}},
  {"expect": {"occurrence": "Rack:1", "body": "Body3", "center": [20, 15, 12.5], "tolerance": 1e-6}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# unfold "Root/<folder>": opens a browser folder whose rows are out of sight.
unfold() {
  local last
  last=$(grep -F "Browser $1/" "$UI_LOG" | grep -v "^Browser arrow\|^Browser eye" | tail -1)
  if [[ $last == *" out of sight" ]]; then
    ui_step "open $1"                    ui_click_logged "Browser arrow $1"
  fi
}

check() {
  printf '[%s]\n' "$2" > "$WORK/check.json"
  "$CLI" run "$WORK/check.json" --open "$FILE" > "$WORK/check.log" 2>&1 ||
    { cat "$WORK/check.log"; ui_fail "$1"; }
  echo "ok   $1"
}

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Browser Root/Joints/Joint2 at" "the joints in the browser"

echo "--- Motion Link: a rack and pinion"
ui_mark
ui_step "motion link (search)"           ui_command "Motion Link"
ui_expect_new "Command Motion Link started" "Motion Link started"
ui_step "rack and pinion"                ui_choose "Panel Motion Link input coupling" 1
# A design longer than the browser opens folded below the root (mitcad#426).
unfold "Root/Joints"
ui_step "the pinion's joint drives"      ui_click_logged "Browser Root/Joints/Joint1"
ui_expect_new "Motion Link Driver: 1" "the driver picked"
ui_step "the rack's joint follows"       ui_click_logged "Browser Root/Joints/Joint2"
ui_expect_new "Motion Link Follower: 1" "the follower picked"
# The pinion's radius is 10 mm unless given.
ui_expect_new "Preview Motion Link: ok" "previewed"
ui_expect_log "Motion Link Pinion Radius: 10 mm" "radius 10"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Added motion link MotionLink1: rack_pinion, Joint1 rz to Joint2 tx, satisfied" "added: nothing moved"
ui_expect_log "Browser Root/Motion Links at" "the browser's Motion Links folder"
unfold "Root/Motion Links"
ui_expect_log "Browser Root/Motion Links/MotionLink1 at" "the browser lists the link"
ui_expect_log "MotionLink1 rack_pinion satisfied" "the browser's joints say it holds"

echo "--- Drive Joint: the rack moves, the pinion turns"
ui_mark
ui_step "drive joint (search)"           ui_command "Drive Joint"
ui_expect_new "Command Drive Joint started" "Drive Joint started"
ui_mark
ui_step "5 mm"                           ui_type_in "Panel Drive Joint input distance" "5"
ui_expect_new "Preview Drive Joint: ok" "previewed"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Drove joint Joint2" "kept"
ui_mark
ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
check "the rack moved and the pinion turned" \
  '{"expect": {"occurrence": "Rack:1", "body": "Body3", "center": [25, 15, 12.5], "tolerance": 1e-6}},
   {"expect": {"query": {"query": "joints"},
     "result": {"links": [{"name": "MotionLink1", "ratio": 10, "solved": true, "values": {"a": 0.5, "b": 5}}]}}}'

echo "--- The link edited from the browser: ratio 2 mm per radian"
ui_mark
ui_step "link's menu (right click)"      ui_click_logged "Browser Root/Motion Links/MotionLink1" 3
ui_expect_new "Context menu: Edit MotionLink1" "the link's menu"
ui_step "edit"                           ui_menu_choose "Edit MotionLink1"
ui_expect_new "Editing F9 with Motion Link" "the link in its panel"
ui_mark
ui_step "radius 2"                       ui_type_in "Panel Motion Link input radius" "2"
ui_expect_new "Preview Motion Link: ok" "previewed"
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Edited F9" "edited"
ui_mark
ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
check "the pinion turns 2.5 radians for the rack's 5 mm" \
  '{"expect": {"occurrence": "Rack:1", "body": "Body3", "center": [25, 15, 12.5], "tolerance": 1e-6}},
   {"expect": {"query": {"query": "joints"},
     "result": {"links": [{"name": "MotionLink1", "ratio": 2, "solved": true, "values": {"a": 2.5, "b": 5}}]}}}'

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_finish "UI motion link test"
