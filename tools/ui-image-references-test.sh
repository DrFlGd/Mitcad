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
#   - A HEIF file (mitcad#850, tools/heif-test-fixture.py's 64 x 32 pixels
#     turned a quarter by its irot property), Width 32: Image2 decoded
#     upright (32 x 64 pixels) and drawn 32 x 64 mm; saved, the file
#     deleted, opened again: decoded and drawn from the design's copy.
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

# The HEIF file of tools/heif-test-fixture.py (quadrants-8-rot1.heic): an
# HEVC image of 64 x 32 pixels (flat red, green, blue and white quarters)
# with an irot of a quarter turn anticlockwise.
{
  printf '\x00\x00\x00\x18\x66\x74\x79\x70\x68\x65\x69\x63\x00\x00\x00\x00\x6d\x69\x66\x31\x68\x65\x69\x63'
  printf '\x00\x00\x01\x4a\x6d\x65\x74\x61\x00\x00\x00\x00\x00\x00\x00\x21\x68\x64\x6c\x72\x00\x00\x00\x00'
  printf '\x00\x00\x00\x00\x70\x69\x63\x74\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00'
  printf '\x0e\x70\x69\x74\x6d\x00\x00\x00\x00\x00\x01\x00\x00\x00\x1e\x69\x6c\x6f\x63\x00\x00\x00\x00\x44'
  printf '\x00\x00\x01\x00\x01\x00\x00\x00\x01\x00\x00\x01\x6a\x00\x00\x00\x69\x00\x00\x00\x23\x69\x69\x6e'
  printf '\x66\x00\x00\x00\x00\x00\x01\x00\x00\x00\x15\x69\x6e\x66\x65\x02\x00\x00\x00\x00\x01\x00\x00\x68'
  printf '\x76\x63\x31\x00\x00\x00\x00\xce\x69\x70\x72\x70\x00\x00\x00\xaf\x69\x70\x63\x6f\x00\x00\x00\x77'
  printf '\x68\x76\x63\x43\x01\x01\x60\x00\x00\x00\x90\x00\x00\x00\x00\x00\x1e\xf0\x00\xfc\xfd\xf8\xf8\x00'
  printf '\x00\x0f\x03\xa0\x00\x01\x00\x18\x40\x01\x0c\x01\xff\xff\x01\x60\x00\x00\x03\x00\x90\x00\x00\x03'
  printf '\x00\x00\x03\x00\x1e\x95\x98\x09\xa1\x00\x01\x00\x2b\x42\x01\x01\x01\x60\x00\x00\x03\x00\x90\x00'
  printf '\x00\x03\x00\x00\x03\x00\x1e\xa0\x20\x82\x16\x59\x59\xae\x4c\xae\x6e\x04\x04\x0c\x08\x00\x00\x03'
  printf '\x00\x08\x00\x00\x03\x00\xc8\x40\xa2\x00\x01\x00\x06\x44\x01\xc1\x71\xa1\x12\x00\x00\x00\x14\x69'
  printf '\x73\x70\x65\x00\x00\x00\x00\x00\x00\x00\x40\x00\x00\x00\x20\x00\x00\x00\x13\x63\x6f\x6c\x72\x6e'
  printf '\x63\x6c\x78\x00\x01\x00\x0d\x00\x06\x80\x00\x00\x00\x09\x69\x72\x6f\x74\x01\x00\x00\x00\x17\x69'
  printf '\x70\x6d\x61\x00\x00\x00\x00\x00\x00\x00\x01\x00\x01\x04\x81\x02\x03\x84\x00\x00\x00\x71\x6d\x64'
  printf '\x61\x74\x00\x00\x00\x65\x28\x01\xaf\x04\x78\x3c\x86\x2f\xf4\x39\x48\xb3\x20\x28\x0c\xb8\xe4\x65'
  printf '\xd1\xcc\x14\xb8\x94\x50\x58\x2c\x9f\xaf\x6c\x9a\xac\x2c\xff\xb1\xec\x4c\x14\x9b\x6d\x7f\x70\x72'
  printf '\xa2\x83\xfb\x63\xc8\x90\xba\x6c\xe9\xe9\x21\x69\x5a\x8f\xcb\xfd\x38\xcb\x86\xdc\xc4\xab\x7b\xa9'
  printf '\x7d\xad\x65\x3b\x81\x52\x44\x78\x68\xfc\x34\xb4\x87\x52\xef\xff\x86\xa8\x72\x83\x1b\x68\x7e\xf4'
  printf '\x80\x10\x95\x93\x05\x8a\xbf\x6a\x3d\xba\x7e'
} > "$WORK/photo.heic"
[ "$(wc -c < "$WORK/photo.heic")" = 467 ] || ui_fail "the HEIF file is not 467 bytes"

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

# HEIF only in builds with its decoder (MITCAD_HEIF, off by default:
# docs/development.md); the build's CMake cache says.
if grep -q '^MITCAD_HEIF:BOOL=ON$' "$(dirname "$UI_APP")/../CMakeCache.txt" 2>/dev/null; then
  echo "--- A HEIF file: decoded upright, saved and read back without the file"
  ui_mark
  ui_step "Image Reference"                  ui_command "Image Reference"
  ui_expect_new "Command Image Reference started" "the panel opened"
  ui_step "click the plate's top"            ui_click_pick "face F1.b0/F1:top"
  ui_expect_new "Image Reference Plane: 1 face" "the face picked"
  ui_step "the HEIF file"                    ui_type_in "Panel Image Reference input file" "$WORK/photo.heic"
  ui_step "width 32"                         ui_type_in "Panel Image Reference input width" "32"
  ui_step "OK (Enter)"                       ui_key Return
  ui_expect_new "Inserted image reference from photo.heic" "inserted"
  ui_expect_new "Image reference Image2 decoded: 32 x 64 pixels" "decoded upright: the quarter turn applied"
  expect_drawn "Image1 .* 70% through, Image2 32.0 x 64.0 mm 50% behind" "drawn 32 x 64 mm"
  grep -q "Image reference Image2 cannot be drawn" "$UI_LOG" && ui_fail "the HEIF image was not decoded"
  ui_step "save (Ctrl+S)"                    ui_key ctrl+s
  ui_expect_new "Saved $WORK/plate.mitcad" "saved"
  ui_stop_app
  rm "$WORK/photo.heic"
  ui_start_app --open "$WORK/plate.mitcad"
  ui_expect_log "Image reference Image2 decoded: 32 x 64 pixels" "decoded from the design's copy"
  # The new run's log from its start.
  UI_MARK=0
  expect_drawn "Image1 .* 70% through, Image2 32.0 x 64.0 mm 50% behind" "drawn as it was saved"
else
  echo "--- A HEIF file: skipped, this build has no HEIF decoder (MITCAD_HEIF off)"
fi

ui_finish "UI image references test"
