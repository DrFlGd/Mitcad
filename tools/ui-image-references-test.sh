#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindowImageReferences.cpp app/OcctViewer.cpp app/view app/browser app/framework tools/cli
# check-all sources: core/model/src/document core/model/src/file core/model-api/src core/model-base/src
# Image references (mitcad#782) through the real UI, in the Docked layout,
# on a 60 x 60 x 3 plate and a 200 x 100 pixel grey BMP file that states
# 4000 pixels per metre (0.25 mm each):
#   - Insert > Image Reference (command search): the plate's top face
#     picked, the file's path typed, Opacity 40: Image1 drawn 50 x 25 mm,
#     40 % opaque, behind the bodies, listed in the browser's Images
#     folder.
#   - Calibrate Image Reference: two clicks on the image's middle row, a
#     tenth in from its left and right edges (160 pixels apart), and 80
#     typed as their distance: the image twice as large (100 x 50 mm).
#   - Its row double-clicked: the panel edits it; Opacity 70 and Through
#     Bodies: drawn over the bodies, in the view's image layer before the
#     sketch layers (a sketch traced on it stays on top).
#   - Its light bulb off: not drawn, and Calibrate Image Reference refused
#     ("Show an image reference first."); saved, opened again, its light bulb on:
#     drawn as it was (the image embedded in the design).
# tools/ui-windows-image-references-test.ps1 does the same in the Windows
# VM.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-image-references-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-image-references.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# A 60 x 60 x 3 plate (Box1 F1, Body1).
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"},
    "corner": [0, 0], "length": 60, "width": 60, "height": 3, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$WORK/plate.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli model"; }

# The BMP file: a 54-byte header (200 x 100, 24 bits, 4000 pixels per
# metre), then 100 rows of 600 grey bytes.
{
  printf 'BM\x96\xea\x00\x00\x00\x00\x00\x00\x36\x00\x00\x00'
  printf '\x28\x00\x00\x00\xc8\x00\x00\x00\x64\x00\x00\x00\x01\x00\x18\x00'
  printf '\x00\x00\x00\x00\x60\xea\x00\x00\xa0\x0f\x00\x00\xa0\x0f\x00\x00'
  printf '\x00\x00\x00\x00\x00\x00\x00\x00'
  head -c 60000 /dev/zero | tr '\0' '\200'
} > "$WORK/plan.bmp"
[ "$(wc -c < "$WORK/plan.bmp")" = 60054 ] || ui_fail "the BMP file is not 60054 bytes"

# expect_size "name" width: the image's width in the last "Image references
# drawn" line is within 2 % of the width expected (clicks land on whole
# pixels of the screen).
expect_size() {
  local width=""
  for _ in $(seq 1 30); do
    width=$(grep "^Image references drawn: .*$1 " "$UI_LOG" | tail -1 | sed -n "s/.*$1 \([0-9.]*\) x .*/\1/p")
    [ -n "$width" ] && awk -v w="$width" -v e="$2" 'BEGIN { exit !((w - e) ^ 2 < (0.02 * e) ^ 2) }' && break
    sleep 0.2
  done
  [ -n "$width" ] || ui_fail "$1 is not drawn"
  awk -v w="$width" -v e="$2" 'BEGIN { exit !((w - e) ^ 2 < (0.02 * e) ^ 2) }' ||
    ui_fail "$1 is drawn $width mm wide, not $2 mm"
  echo "ok   $1 drawn $width mm wide"
}

# expect_drawn "pattern" description: a line "Image references drawn: …"
# since the mark matches the extended regular expression.
expect_drawn() {
  for _ in $(seq 1 30); do
    tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qE "^Image references drawn: $1" && { echo "ok   $2"; return; }
    sleep 0.2
  done
  ui_fail "$2: no 'Image references drawn: $1' since the mark"
}

ui_start_display
ui_start_app --open "$WORK/plate.mitcad"
ui_step "fit (F6)"                         ui_key F6

echo "--- Insert > Image Reference on the plate's top"
ui_mark
ui_step "Image Reference"                  ui_command "Image Reference"
ui_expect_new "Command Image Reference started" "the panel opened"
ui_step "click the plate's top"            ui_click_pick "face F1.b0/F1:top"
ui_expect_log "Image Reference Plane: 1 face" "the face picked"
ui_step "the image's file"                 ui_type_in "Panel Image Reference input file" "$WORK/plan.bmp"
ui_step "opacity 40"                       ui_type_in "Panel Image Reference input opacity" "40"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Inserted image reference from plan.bmp" "inserted"
ui_expect_new "Image references drawn: Image1 50.0 x 25.0 mm 40% behind" "drawn 50 x 25 mm, 40 % opaque, behind"
ui_expect_log "Browser Root/Images/Image1 at" "listed in the Images folder"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

echo "--- Calibrate: 160 pixels are 80 mm"
ui_mark
ui_step "Calibrate Image Reference"        ui_command "Calibrate Image Reference"
ui_expect_new "Calibrate Image1 started" "calibration started"
ui_expect_new "Calibrate Image1 right at" "the image's places logged"
ui_step "the first point"                  ui_click_logged "Calibrate Image1 left"
ui_expect_new "Calibrate Image1 first point" "the first point taken"
ui_step "the second point"                 ui_click_logged "Calibrate Image1 right"
ui_expect_new "Calibrate Image1 second point" "the second point taken"
ui_focus_dialog "$(ui_title_regex "Calibrate Image1")"
ui_step "80 mm"                            ui_type_text "80"
ui_step "OK (Enter)"                       ui_key Return
ui_focus_main
ui_expect_new "Calibrated Image1: " "calibrated"
expect_size Image1 100

echo "--- Edited in its panel: 70 % opaque, through the bodies"
ui_mark
ui_double_click_expect "Browser Root/Images/Image1" "Editing image reference Image1 with Image Reference" \
  "the edit panel opened"
ui_step "opacity 70"                       ui_type_in "Panel Image Reference input opacity" "70"
ui_step "Through Bodies"                   ui_click_logged "Panel Image Reference input through"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Edited image reference Image1" "edited"
expect_drawn "Image1 .* 70% through" "drawn 70 % opaque, over the bodies"
ui_expect_new "Image references through the bodies: in the image layer, before the sketch halos" \
  "drawn under the sketches"
expect_size Image1 100

echo "--- Hidden, saved, opened again and shown"
ui_mark
ui_step "Image1's light bulb off"          ui_click_logged "Browser eye Root/Images/Image1"
ui_expect_new "Visibility Root/Images/Image1: hidden" "hidden"
ui_expect_new "Image references drawn: none" "not drawn"
ui_step "Calibrate Image Reference"        ui_command "Calibrate Image Reference"
ui_expect_new "Show an image reference first." "a hidden image is not calibrated"
ui_step "save (Ctrl+S)"                    ui_key ctrl+s
ui_expect_new "Saved $WORK/plate.mitcad" "saved"
ui_stop_app
ui_start_app --open "$WORK/plate.mitcad"
ui_expect_log "Browser eye Root/Images/Image1 at" "Image1 read back"
ui_mark
ui_step "Image1's light bulb on"           ui_click_logged "Browser eye Root/Images/Image1"
ui_expect_new "Visibility Root/Images/Image1: shown" "shown"
expect_drawn "Image1 .* 70% through" "drawn as it was saved"
expect_size Image1 100

ui_finish "UI image references test"
