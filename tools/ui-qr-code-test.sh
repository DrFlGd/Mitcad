#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/sketch core/model
# QR codes in sketches (mitcad#473) through the real UI: QR Code adds a
# code of "https://mitcad.org" (level M: 326 dark modules) with 1 mm
# modules; Extrude takes the whole code at once and makes its modules a
# solid 2 high (326 x 2 mm3); Edit QR Code (a double-click on the code)
# changes its module size.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-qr-code-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-qr.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# Sketch1 (F1) with a point at its origin, nothing else.
cat > "$WORK/sketch.json" << 'EOF'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [-5, -5]}
]
EOF
"$CLI" run "$WORK/sketch.json" --save "$WORK/qr.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli sketch"; }

ui_start_display
ui_start_app --open "$WORK/qr.mitcad"

echo "--- QR Code in Sketch1"
ui_step "double-click Sketch1"             ui_double_click_logged "Timeline Sketch1"
ui_expect_log "Editing sketch F1 (Sketch1)" "the sketch edited"
ui_step "QR Code (search)"                 ui_command "QR Code"
ui_expect_log "Panel QR Code input content at" "the QR Code panel"
ui_step "the content"                      ui_type_in "Panel QR Code input content" "https://mitcad.org"
ui_step "1 mm modules"                     ui_type_in "Panel QR Code input module" "1"
ui_expect_log "Preview QR Code: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log 'Added QR code "https://mitcad.org", module 1, level M' "the code added"
ui_step "fit (F6)"                         ui_key F6
[ -n "$SHOT" ] && ui_capture "$SHOT"

echo "--- Edit QR Code: a double-click on the code"
ui_step "edit text (search) for places"    ui_command "Edit QR Code"
ui_expect_log "Sketch entity F1/t2 at" "the code's place"
ui_step "cancel (Esc)"                     ui_key Escape
ui_double_click_expect "Sketch entity F1/t2" "Double-click on t2: sketch.edit_code" \
  "the double-click opened Edit QR Code"
ui_expect_log "Panel Edit QR Code input module at" "its panel"
ui_step "0.5 mm modules"                   ui_type_in "Panel Edit QR Code input module" "0.5"
ui_expect_log "Preview Edit QR Code: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log 'Edited QR code t2: "https://mitcad.org", module 0.5, level M' "the code edited"
ui_step "finish sketch (Ctrl+Enter)"       ui_key ctrl+Return
ui_expect_log "Sketch finished" "sketch finished"

echo "--- Extrude takes the whole code"
ui_mark
ui_step "extrude (E)"                      ui_key e
ui_expect_new "Extrude Profiles: " "the profiles taken"
taken=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | sed -n 's/.*Extrude Profiles: \([0-9]*\) profiles.*/\1/p' | head -1)
[ -n "$taken" ] && [ "$taken" -gt 10 ] || ui_fail "Extrude took $taken profiles, not the whole code"
echo "ok   every dark region of the code: $taken profiles"
ui_step "distance 2"                       ui_type_in "Panel Extrude input distance" "2"
ui_expect_log "Preview Extrude: ok" "previewed"
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "New body Body1 (F2.b0): volume" "the code extruded"
ui_wait_idle
# 326 modules of 0.5 x 0.5 mm, 2 high: 163 mm3 in the bodies the
# extrusion made (the code's parts that touch only at corners apart).
total=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | sed -n 's/.*New body Body[0-9]* (F2\.b[0-9]*): volume \([0-9.]*\) mm3.*/\1/p' |
  awk '{ s += $1 } END { printf "%.3f", s }')
awk -v t="$total" 'BEGIN { exit !(t > 162.99 && t < 163.01) }' || ui_fail "the code's volume is $total, not 163"
echo "ok   the code's modules: $total mm3"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

ui_finish "UI QR code test"
