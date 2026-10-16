#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands/SheetCommands.cpp core/model/src/sheet tools/cli
# A double bend edited from the timeline through the real UI (mitcad#288):
# the Bend panel loads a double bend (`port_mode: "double"`, as the `.ipt`
# import makes them) with its type and whole side, and an edit keeps it.
# Design (mitcad-cli, as cli.sheet_double_bend): t = 1, r = 2, K = 0.4;
# panel A (x = -30..0, y = 0..20, z = 0..1) and a separate panel B (x =
# 0..30, z = 5..6) joined at x = 0 by two opposite bends of angle a and the
# straight between them, A's edge fixed: middle radius 2.5, straight L =
# (5 - 5 (1 - cos a)) / sin a, B cut back to 5 sin a + L cos a; volume
# 600 + (30 - run) 20 + 20 L + 2 a (9 - 4) / 2 20.
#   - 45 degrees: 1300 - 100 sqrt 2 + 25 pi = 1237.1184601;
#   - edited to 60 degrees: L = 2.5 / sin 60, run = 5 sin 60 + L cos 60,
#     about 1246.985 (`volume` below);
#   - the whole side edited to B's edge: the same volume by symmetry.
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-sheet-double-bend-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-sheet-double-bend.XXXXXX)
FILE=$WORK/double.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
cat > "$WORK/model.json" << EOF
[
  {"cmd":"add_feature","def":{"type":"sheet_rules","thickness":1,"bend_radius":2,"allowance":{"type":"k_factor","k":0.4},"reference_side":"front","bend_relief":{"type":"rectangular","width":1,"depth":1},"corner_relief":{"type":"round","width":2,"depth":2}}},
  {"cmd":"sketch.create"},
  {"cmd":"sketch.add_rectangle","sketch":"F2","corner":[-30,0],"width":30,"height":20},
  {"cmd":"add_feature","def":{"type":"sheet_face","rules":"F1","profiles":[{"sketch":"F2","region":"$RECT"}]}},
  {"cmd":"sketch.create"},
  {"cmd":"sketch.add_rectangle","sketch":"F4","corner":[0,0],"width":30,"height":20},
  {"cmd":"add_feature","def":{"type":"sheet_face","rules":"F1","profiles":[{"sketch":"F4","region":"$RECT"}]}},
  {"cmd":"add_feature","def":{"type":"move","bodies":["F5.b0"],"copy":false,"transform":{"type":"free","matrix":[[1,0,0,0],[0,1,0,0],[0,0,1,5]]}}},
  {"cmd":"add_feature","def":{"type":"sheet_bend","body":"F3.b0","moving_body":"F5.b0","stationary":{"panel":{"feature":"F3","slot":0},"curve":{"type":"sketch","sketch":"F2","segment":"c2[c1,c3]"}},"moving":{"panel":{"feature":"F5","slot":0},"curve":{"type":"sketch","sketch":"F4","segment":"c4[c3,c1]"}},"angle":"45 deg","port_mode":"double","fixed":"stationary"}},
  {"expect":{"feature":"F7","status":"ok","body":"F3.b0","volume":1237.1184601024353}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# The volume at angle a (degrees), A's or B's edge fixed alike.
volume() {
  awk -v d="$1" 'BEGIN { a = d * atan2(0, -1) / 180; l = (5 - 5 * (1 - cos(a))) / sin(a);
    run = 5 * sin(a) + l * cos(a); printf "%.9f", 600 + (30 - run) * 20 + 20 * l + 2 * a * 5 / 2 * 20 }'
}

# panel_input_shown input: the last layout of the Bend panel shows it.
panel_input_shown() {
  grep "Panel Bend input $1 at" "$UI_LOG" | tail -1 | grep -qv "(hidden)"
}

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Timeline SheetBend1 at" "the double bend is in the timeline"
grep -q "Failed features: [^n]" "$UI_LOG" && ui_fail "a feature failed"

echo "--- The double bend loads as a double bend"
ui_double_click_expect "Timeline SheetBend1" "Editing F7 with Bend" "editing the double bend"
ui_expect_new "Panel Bend input fixed at" "the panel is laid out"
ui_sync
panel_input_shown fixed || ui_fail "the whole side is hidden: the bend did not load as a double bend"
panel_input_shown flip && ui_fail "Flip is shown for a double bend"
echo "ok   Bend Type: Double Bend, its whole side shown, no Flip"
ui_step "angle 60"                         ui_type_in "Panel Bend input angle" "60"
ui_expect_new "Preview Bend: ok" "previewed"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F7" "the double bend edited"
ui_expect_volume "Body [A-Za-z0-9]* (F3.b0): volume [0-9.]* -> \([0-9.]*\) mm3" "$(volume 60)" "a jog of two 60-degree bends"

echo "--- B's edge whole instead"
ui_double_click_expect "Timeline SheetBend1" "Editing F7 with Bend" "editing it again"
ui_expect_new "Panel Bend input fixed at" "the panel is laid out"
ui_sync
panel_input_shown fixed || ui_fail "the edited bend did not stay a double bend"
ui_step "Whole Face: Moving"               ui_choose "Panel Bend input fixed" 1
ui_expect_new "Preview Bend: ok" "previewed"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited F7" "edited to B's edge"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -q "F7.*error" && ui_fail "the edited double bend failed"
echo "ok   B's edge whole: the bend still evaluates"

ui_step "undo (Ctrl+Z)"                    ui_key ctrl+z
ui_step "undo (Ctrl+Z)"                    ui_key ctrl+z

ui_finish "UI sheet double bend test"
