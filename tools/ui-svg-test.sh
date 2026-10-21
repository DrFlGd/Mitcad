#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/files core/model
# Insert SVG (mitcad#473) through the real UI: the test drawing of
# core/model/data/svg (a square ring and a disc with holes, a stroked
# line, a text) into a new sketch, its stroke as a band; the filled
# regions come selected, and Extrude makes them solids with their holes.
# Then the same drawing into the edited sketch at twice the size.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-svg-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
SOURCE="$(cd "$(dirname "$0")/.." && pwd)/core/model/data/svg/logo.svg"
WORK=$(mktemp -d /tmp/mitcad-ui-svg.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
cp "$SOURCE" "$WORK/logo.svg"

ui_start_display
ui_start_app --no-native-dialogs

echo "--- Insert SVG into a new sketch"
ui_mark
ui_step "Insert SVG (search)"              ui_command "Insert SVG"
ui_type_path "Insert SVG" "$WORK/logo.svg"
ui_expect_new "Insert SVG dialog opened: 100 x 60 mm, 2 filled, 1 stroked, 1 texts" "the drawing's size and contents"
ui_expect_new "Insert SVG OK at" "the dialog is laid out"
ui_focus_dialog '^Insert SVG$'
ui_step "stroke bands"                     ui_click_logged "Insert SVG bands"
ui_step "OK"                               ui_click_logged "Insert SVG OK"
ui_focus_main
ui_expect_new "Insert SVG: scale 1, lower left at (0, 0), bands on" "the choices"
ui_expect_new "Inserted logo.svg into Sketch1: " "inserted"
ui_expect_new ", 3 filled, 1 bands" "the ring, the disc and the band are filled"
ui_expect_new "Selected: 3 profiles" "the filled regions are selected"
[ -n "$SHOT" ] && ui_capture "$SHOT"

echo "--- Extrude the filled regions"
ui_mark
ui_step "extrude (E)"                      ui_key e
ui_expect_new "Extrude Profiles: 3 profiles" "Extrude takes the selection"
ui_step "distance 2"                       ui_type_in "Panel Extrude input distance" "2"
ui_expect_log "Preview Extrude: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
# A body each: the ring (40^2 - 20^2) 2, the band 90 x 4 x 2 and the disc
# pi (15^2 - 7.5^2) 2, with their holes.
ui_expect_volume "New body Body1 (F2.b0): volume \([0-9.]*\) mm3" 2400 "the ring keeps its hole" 1e-5
ui_expect_volume "New body Body2 (F2.b1): volume \([0-9.]*\) mm3" 720 "the band is 4 mm wide" 1e-5
ui_expect_volume "New body Body3 (F2.b2): volume \([0-9.]*\) mm3" 1060.287560 "the disc keeps its hole" 1e-5
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

echo "--- Into the edited sketch, twice the size"
ui_step "double-click Sketch1"             ui_double_click_logged "Timeline Sketch1"
ui_expect_log "Editing sketch F1 (Sketch1)" "the sketch edited"
ui_mark
ui_step "Insert SVG (search)"              ui_command "Insert SVG"
ui_type_path "Insert SVG" "$WORK/logo.svg"
ui_expect_new "Insert SVG OK at" "the dialog"
ui_focus_dialog '^Insert SVG$'
ui_step "scale 2"                          ui_type_in "Insert SVG scale" "2"
ui_step "at Y 100"                         ui_type_in "Insert SVG y" "100"
ui_step "OK"                               ui_click_logged "Insert SVG OK"
ui_focus_main
ui_expect_new "Insert SVG: scale 2, lower left at (0, 100), bands off" "the choices"
ui_expect_new "Inserted logo.svg into Sketch1: " "inserted into the edited sketch"
ui_expect_new ", 2 filled, 0 bands" "without bands the stroke is a line"

ui_finish "UI SVG test"
