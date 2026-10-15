#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser tools/cli
# The browser's Collapse All (mitcad#312), through the real UI, on a
# synthetic design with several branches and four nested levels
# (Root > Sketches > Sketch1 > Extrude1):
#   - Several branches are opened with their disclosure arrows, and
#     Collapse All is chosen from the context menu of the deepest row: the
#     whole tree folds, descendants of folded rows too, and the top row stays.
#   - Model contents and visibility are unchanged (nothing is logged for them).
#   - Opening the top row with its arrow shows only its children: folders that
#     open by default and branches that were open before stay folded.
#   - A refresh (a light bulb toggled and back) does not restore the states.
#   - Each arrow click opens one level; the levels below stay folded.
#   - A second Collapse All, also when everything is folded already, leaves
#     the tree folded and still opens one level at a time.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-browser-collapse-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-browser-collapse.XXXXXX)
FILE=$WORK/collapse.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# count_new "text": how many lines logged since the mark contain the text.
count_new() { tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -cF -- "$1"; }

# expect_none "text" "description": nothing logged since the mark contains it.
expect_none() {
  [ "$(count_new "$1")" = 0 ] || ui_fail "$2: '$1' was logged"
  echo "ok   $2"
}

# arrow "path": a click on the row's disclosure arrow.
arrow() { ui_click_logged "Browser arrow $1"; }

# Sketch1 (F1) 30 x 20 on XY; Extrude1 (F2) 10 up: Body1; Plane1 (F3) 30 up;
# Sketch2 (F4) 20 x 10 on it; Loft1 (F5) from Sketch1 to Sketch2: Body2.
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "construction_plane",
    "definition": {"type": "offset", "plane": "xy", "distance": 30}}},
  {"cmd": "sketch.create", "plane": "F3"},
  {"cmd": "sketch.add_rectangle", "sketch": "F4", "corner": [5, 5], "width": 20, "height": 10},
  {"cmd": "add_feature", "def": {"type": "loft", "sections": [
    {"type": "profile", "sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"},
    {"type": "profile", "sketch": "F4", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Browser Root/Sketches/Sketch1 at" "the browser lists Sketch1"
ui_expect_log "Browser arrow Root at" "the top row has a disclosure arrow"
grep -q "Failed features: [^n]" "$UI_LOG" && ui_fail "a feature failed"

echo "--- Several branches open, four levels deep"
ui_step "open Sketch1"                   arrow "Root/Sketches/Sketch1"
ui_expect_log "Browser Root/Sketches/Sketch1/Extrude1 at" "Extrude1, four levels down, is in sight"
ui_step "open Named Views"               arrow "Root/Named Views"
ui_expect_log "Browser Root/Named Views/Top at" "the named views are listed"
ui_expect_log "Browser Root/Bodies/Body1 at" "the Bodies folder is open (by default)"

echo "--- Collapse All from the deepest row"
ui_mark
ui_step "right-click Extrude1's row"     ui_click_logged "Browser Root/Sketches/Sketch1/Extrude1" 3
ui_expect_new "Collapse All" "the menu has Collapse All"
ui_step "choose Collapse All"            ui_menu_choose "Collapse All"
ui_expect_new "Browser collapsed all" "the browser folded"
ui_expect_new "Browser Root/Sketches/Sketch1/Extrude1 out of sight" "the deepest row is folded away"
ui_expect_new "Browser Root/Named Views/Top out of sight" "so are the named views"
ui_expect_new "Browser Root/Bodies/Body1 out of sight" "and the bodies"
ui_expect_new "Browser Root/Sketches out of sight" "and the sketches folder"
ui_place_valid "Browser Root" || ui_fail "the top row is gone"
echo "ok   the top row stays"
expect_none "Visibility " "no light bulb changed"
expect_none "Suppressed" "no feature changed"
expect_none "Removed body" "no body changed"

echo "--- Opening the top row opens one level"
ui_mark
ui_step "open the top row"               arrow "Root"
ui_expect_new "Browser Root/Sketches at" "its children are in sight"
ui_expect_new "Browser Root/Bodies at" "also the folders that open by default"
expect_none "Browser Root/Sketches/Sketch1 at" "the folded Sketches folder stays folded"
expect_none "Browser Root/Bodies/Body1 at" "the Bodies folder stays folded"
expect_none "Browser Root/Named Views/Top at" "Named Views stays folded"

echo "--- A refresh does not restore the states"
ui_mark
ui_step "hide the Bodies folder's bulb"  ui_click_logged "Browser eye Root/Bodies"
ui_expect_new "Visibility Root/Bodies" "the model changed and the browser was rebuilt"
ui_step "show it again"                  ui_click_logged "Browser eye Root/Bodies"
sleep 1
expect_none "Browser Root/Sketches/Sketch1 at" "Sketch1's row did not come back"
expect_none "Browser Root/Bodies/Body1 at" "nor did the bodies"
expect_none "Browser Root/Named Views/Top at" "nor the named views"

echo "--- One level per arrow click"
ui_mark
ui_step "open Sketches"                  arrow "Root/Sketches"
ui_expect_new "Browser Root/Sketches/Sketch1 at" "Sketch1 is in sight"
ui_expect_new "Browser Root/Sketches/Sketch2 at" "and Sketch2"
expect_none "Browser Root/Sketches/Sketch1/Extrude1 at" "Sketch1, open before, is folded"
ui_mark
ui_step "open Sketch1"                   arrow "Root/Sketches/Sketch1"
ui_expect_new "Browser Root/Sketches/Sketch1/Extrude1 at" "Extrude1 is in sight"
expect_none "Browser Root/Sketches/Sketch2/Loft1 at" "Sketch2 stays folded"
ui_mark
ui_step "open Bodies"                    arrow "Root/Bodies"
ui_expect_new "Browser Root/Bodies/Body1 at" "Body1 is in sight"
[ -n "$SHOT" ] && ui_capture "$SHOT"

echo "--- A second Collapse All, also when everything is folded"
ui_mark
ui_step "right-click the top row"        ui_click_logged "Browser Root" 3
ui_step "choose Collapse All"            ui_menu_choose "Collapse All"
ui_expect_new "Browser Root/Sketches/Sketch1 out of sight" "the open branches folded"
ui_expect_new "Browser Root/Bodies/Body1 out of sight" "the bodies too"
ui_mark
ui_step "right-click the top row"        ui_click_logged "Browser Root" 3
ui_step "choose Collapse All again"      ui_menu_choose "Collapse All"
ui_expect_new "Browser collapsed all" "the second one ran"
ui_place_valid "Browser Root" || ui_fail "the top row is gone"
echo "ok   the top row stays"
ui_mark
ui_step "open the top row"               arrow "Root"
ui_expect_new "Browser Root/Sketches at" "one level is in sight"
expect_none "Browser Root/Sketches/Sketch1 at" "no more"
expect_none "Visibility " "no light bulb changed"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI browser collapse test"
