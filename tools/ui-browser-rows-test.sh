#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser app/framework app/view tools/cli
# The Browser's Rows background in the Floating layout (mitcad#419), through
# the real UI, on a 200 x 200 mm plate zoomed in until it fills the view:
#   - The Browser starts on its card (the default).
#   - Preferences > Display > Browser background: Rows, applied at once and
#     saved (appearance/browserBackground); a place beside the rows, inside
#     the card's old rectangle, is logged.
#   - A click there selects the plate's face behind the rows; a click on a
#     row still selects its body; a middle drag beside the rows orbits.
#   - Back to Card at once: the place beside the rows is gone, and a click
#     there is the card's again (the plate is not picked).
# The Windows counterpart is tools/ui-windows-browser-rows-test.ps1.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-browser-rows-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-browser-rows.XXXXXX)
FILE=$WORK/plate.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# A 200 x 200 x 10 mm plate centred on the origin: Body1 (F2.b0).
cat > "$WORK/plate.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-100, -100], "width": 200, "height": 200},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/plate.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# The middle button orbits (the drag beside the rows).
mkdir -p "$UI_CONFIG/Mitcad"
printf '[view]\nnavigationScheme=middle-orbit\n' > "$UI_CONFIG/Mitcad/Mitcad.conf"

# count_new "text": how many lines logged since the mark contain the text.
count_new() { tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -cF -- "$1"; }

# set_background card|rows Up|Down: Preferences > Display, the Browser
# background one entry up or down (Alt+B), OK.
set_background() {
  ui_mark
  ui_step "Preferences: Display (search)"  ui_command "Preferences: Display"
  ui_expect_new "Preferences browser background at" "Display has the Browser background"
  ui_focus_dialog '^Preferences$'
  ui_step "Browser background: $1"         ui_key alt+b "$2"
  ui_step "OK"                             ui_key Return
  ui_focus_main
  ui_expect_new "Preferences: browser background $1" "the choice is saved"
  ui_expect_new "Browser background: $1" "the Browser follows at once"
  grep -q "^browserBackground=$1" "$UI_CONFIG/Mitcad/Mitcad.conf" ||
    ui_fail "appearance/browserBackground is not $1 in the settings"
  echo "ok   the setting is $1"
}

ui_start_display
ui_start_app --open "$FILE" --chrome=floating
ui_expect_log "Opened $FILE" "the plate is open"
ui_expect_log "Browser background: card" "the Browser starts on its card"
ui_expect_log "Browser Root/Bodies/Body1 at" "Body1's row"

echo "--- The plate fills the view"
ui_step "top view (search)"              ui_command "Top View"
ui_step "fit (F6)"                       ui_key F6
# Zoomed in about the plate's middle (the body's logged place), which stays
# put: the plate grows on every side and fills the view, also behind the
# Browser. About the view's middle it grows mostly away from the Browser:
# the plate sits in the free area right of the Browser's card.
ui_sync
ui_step "zoom in at the plate's middle"  xdotool mousemove $(ui_logged_at "Body F2.b0") click --repeat 14 --delay 60 4
ui_wait_idle

echo "--- Rows"
set_background rows Down
ui_expect_log "Browser view beside the rows at" "a place beside the rows is logged"
read -r BX BY <<< "$(ui_logged_at "Browser view beside the rows")"
ui_mark
ui_step "click beside the rows"          ui_click_logged "Browser view beside the rows"
ui_expect_new "Selected: 1 face [face F2:" "the click reached the view: the plate's face behind the rows"
ui_mark
ui_step "click Body1's row"              ui_click_logged "Browser Root/Bodies/Body1"
ui_expect_new "Selected: 1 body [body F2.b0]" "a row's box still takes the click"
[ -n "$SHOT" ] && ui_capture "$SHOT"

ui_mark
read -r BX BY <<< "$(ui_logged_at "Browser view beside the rows")"
ui_apart "$BX" "$BY"
xdotool mousemove "$BX" "$BY" mousedown 2
for i in 1 2 3 4 5 6 7 8; do
  xdotool mousemove $((BX + 15 * i)) $((BY + 8 * i))
  sleep 0.05
done
xdotool mouseup 2
ui_sync
ui_expect_new "Camera direction" "a middle drag beside the rows orbits" 8
last=$(grep "Camera direction" "$UI_LOG" | tail -1)
case "$last" in *"direction 0 0 -1 "*) ui_fail "the drag did not turn the view: $last" ;; esac
echo "ok   orbited to: ${last#*Camera }"
[ "$(count_new "Selected: ")" = 0 ] || ui_fail "the drag selected something"
echo "ok   the drag selected nothing"

echo "--- Back to Card"
set_background card Up
ui_expect_new "Browser view beside the rows out of sight" "the place beside the rows is gone"
ui_mark
ui_step "click where the rows left room" xdotool mousemove "$BX" "$BY" click 1
sleep 0.5
# The card's tree may select the row there; the plate behind is not picked.
[ "$(count_new "Selected: 1 face")" = 0 ] || ui_fail "the click went past the card to the view"
echo "ok   the card takes the click again"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI browser rows test"
