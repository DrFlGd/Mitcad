#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/motion app/MainWindowMotion.cpp app/browser
# check-all sources: core/model/src/motion.rs core/model/src/document/motion.rs tools/cli
# Motion studies (mitcad#454) through the real UI, on a design mitcad-cli
# makes: Plate:1 (a 40 x 40 x 10 block, grounded), Pin:1 (a 10 x 10 x 20
# block) sliding along x on the plate (Joint1), Knob:1 (a 10 x 10 x 5
# block) turning on the plate (Joint2), and the study Open keying both: the
# pin 15 mm and the knob 90 degrees at step 20 of 20.
#   - The browser's Motion Studies folder lists the study.
#   - Motion Study (search) opens its panel; Play plays it to its last
#     step, the joints following (previews only: no undo step).
#   - Add Key at step 0 with a value typed; Keep Position plays the study
#     to the shown step in the design, one undo step, saved and checked by
#     mitcad-cli.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-motion-study-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-motion.XXXXXX)
FILE=$WORK/study.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

# Plate (O1): F1, F2 (Body1); Pin (O2): F3, F4 (Body2); Knob (O3): F5, F6
# (Body3); Joint1 F7 (slider along x), Joint2 F8 (revolute, off the
# plate's middle so that the pin and the knob stand apart).
sed "s/RECT/$RECT/g" > "$WORK/model.json" << 'EOF'
[
  {"cmd": "create_component", "name": "Plate"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 40, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "create_component", "name": "Pin", "transform": {"translation": [100, 0, 0]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [0, 0], "width": 10, "height": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "create_component", "name": "Knob", "transform": {"translation": [0, 100, 0]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F5", "corner": [0, 0], "width": 10, "height": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F5", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "ground_occurrence", "occurrence": "Plate:1"},
  {"cmd": "add_joint", "kind": "slider", "slide_axis": "x", "flip": true, "position": {"tx": 0},
    "a": {"occurrence": "Pin:1", "geometry": {"body": "F4.b0", "face": "F4:start(RECT)"}},
    "b": {"occurrence": "Plate:1", "geometry": {"body": "F2.b0", "face": "F2:end(RECT)"},
          "frame_override": {"origin": [10, 10, 10]}}},
  {"cmd": "add_joint", "kind": "revolute", "flip": true, "position": {"rz": 0},
    "a": {"occurrence": "Knob:1", "geometry": {"body": "F6.b0", "face": "F6:start(RECT)"}},
    "b": {"occurrence": "Plate:1", "geometry": {"body": "F2.b0", "face": "F2:end(RECT)"},
          "frame_override": {"origin": [30, 30, 10]}}},
  {"cmd": "add_motion_study", "name": "Open", "steps": 20, "keys": [
    {"joint": "Joint1", "motion": "tx", "points": [[0, 0], [20, 15]]},
    {"joint": "Joint2", "motion": "rz", "points": [[0, 0], [20, "90 deg"]]}]}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# check "description" '{"expect": ...}': mitcad-cli checks the saved design.
check() {
  printf '[%s]\n' "$2" > "$WORK/check.json"
  "$CLI" run "$WORK/check.json" --open "$FILE" > "$WORK/check.log" 2>&1 ||
    { cat "$WORK/check.log"; ui_fail "$1"; }
  echo "ok   $1"
}

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Browser Root/Motion Studies at" "the browser's Motion Studies folder"
# A design longer than the browser opens folded below the root (mitcad#426).
if ! grep -q "Browser Root/Motion Studies/Open at" "$UI_LOG"; then
  ui_step "open the folder"              ui_click_logged "Browser arrow Root/Motion Studies"
fi
ui_expect_log "Browser Root/Motion Studies/Open at" "the browser lists the study"

echo "--- Motion Study: play the study"
ui_mark
ui_step "motion study (search)"          ui_command "Motion Study"
ui_expect_new "Motion study Open opened: 20 steps, 2 keys" "the study's panel"
ui_focus_dialog "Motion Study"
ui_mark
ui_step "play (Alt+P)"                   ui_key alt+p
ui_expect_new "Playing Open from step 0" "playing"
ui_expect_new "Played Open to step 20: Joint1 tx 15 mm, Joint2 rz 90 deg" "played to the end, both joints keyed" 60
if grep -E "^Undo: " "$UI_LOG" | tail -1 | grep -q "Open"; then
  ui_fail "the playback left an undo step"
fi

echo "--- Keep Position"
ui_mark
ui_step "keep (Alt+T)"                   ui_key alt+t
ui_expect_new "Kept Open at step 20: Joint1 tx 15 mm, Joint2 rz 90 deg" "kept in the design"
ui_expect_new "Motion study Open closed" "the panel closed"
ui_focus_main
ui_mark
ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
check "the joints at the study's last step" \
  '{"expect": {"query": {"query": "joints"}, "result": {"joints": [{"name": "Joint1", "values": {"tx": 15}}, {"name": "Joint2", "values": {"rz": 1.5707963267948966}}]}}}'

echo "--- A key added at step 0"
ui_mark
ui_step "open the study (double-click)"  ui_double_click_logged "Browser Root/Motion Studies/Open"
ui_expect_new "Motion study Open opened: 20 steps, 2 keys" "the panel again"
ui_focus_dialog "Motion Study"
ui_step "the value field (Alt+V)"        ui_key alt+v
ui_type_text "5 mm"
ui_mark
ui_step "add key (Alt+K)"                ui_key alt+k
ui_expect_new "Key added to Open: Joint1 tx at step 0: 5 mm" "the pin keyed at 5 mm at step 0"
ui_step "close (Esc)"                    ui_key Escape
ui_expect_log "Motion study Open closed" "closed"
ui_focus_main
ui_mark
ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
check "the key in the file" \
  '{"expect": {"query": {"query": "motion_studies"}, "result": [{"name": "Open", "keys": [{"motion": "tx", "points": [[0, 5], [20, 15]]}, {"motion": "rz"}]}]}}'

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_finish "UI motion study test"
