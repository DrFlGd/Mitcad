#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands
# Options of Fillet, Sweep and Pattern on Path (mitcad#464) through the
# real UI, with the volumes the model reports:
#   - Fillet: the demo block's top front edge (60 mm) rounded with a Spline
#     radius of control radii 2, 6 and 4 (a quadratic: the radius's square
#     integrates to 248/15 along the edge's position, so the rounding takes
#     (1 - pi/4) 60 248/15 away); opened again, its middle control radius
#     is a row of Control Radii.
#   - Sweep: a circle of radius 4 swept 50 with End Scale 0.5, a frustum of
#     radii 4 and 2; a 4 x 10 rectangle swept 50 between a Guide Rail and a
#     Second Rail that open from 10 to 14 apart (sections 40 k^2, k = 1 +
#     0.008 x), with the Rail Point shown (Square to Path by default,
#     mitcad#469; the same along these straight rails).
#   - Pattern on Path: the frustum's body three times 25 apart along a chain
#     of two sketch lines (30 along X, then up Y).
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-feature-options-test.sh [screenshot.png]
# check-all sources: tools/cli

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-feature-options.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
TOP_FRONT="E{F2:end($R)|F2:side(c1[c4,c2])}"
PI=3.14159265358979

# Sketch1 (F1): the cone's path c1 (0, 0)-(50, 0), the rail sweep's path c4
# (0, 40)-(50, 40), its rails c7 (0, 35)-(50, 33) and c10 (0, 45)-(50, 47),
# and the pattern's chain c13 (0, -20)-(30, -20), c16 (30, -20)-(30, 10);
# Sketch2 (F2) on YZ (sketch x = -Z, y = +Y): the 4 x 10 rectangle between
# the rails; Sketch3 (F3) on YZ: a circle of radius 4 round the cone's path.
cat > "$WORK/paths.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 0], "end": [50, 0]},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 40], "end": [50, 40]},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 35], "end": [50, 33]},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 45], "end": [50, 47]},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, -20], "end": [30, -20]},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [30, -20], "end": [30, 10]},
  {"cmd": "sketch.create", "plane": "yz"},
  {"cmd": "sketch.add_rectangle", "sketch": "F2", "corner": [-2, 35], "width": 4, "height": 10},
  {"cmd": "sketch.create", "plane": "yz"},
  {"cmd": "sketch.add_circle", "sketch": "F3", "center": [0, 0], "diameter": 8}
]
EOF
"$CLI" run "$WORK/paths.json" --save "$WORK/paths.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display

echo "=== Fillet with a Spline radius on the demo block"
ui_start_app --demo
ui_step "fit (F6)"                         ui_key F6
ui_step "fillet (F)"                       ui_key f
ui_step "pick the top front edge"          ui_click_pick "edge F2.b0/$TOP_FRONT"
ui_expect_log "Fillet Edges: 1 edge" "the edge picked"
ui_step "Spline"                           ui_choose "Panel Fillet input sets.0.size" 4
ui_expect_log "Panel Fillet input sets.0.knots at" "its knots shown"
ui_expect_log "Panel Fillet add control at" "the control radii shown"
ui_step "first control radius 2"           ui_type_in "Panel Fillet input sets.0.radius" "2"
ui_step "last control radius 4"            ui_type_in "Panel Fillet input sets.0.end" "4"
ui_step "add a control radius"             ui_click_logged "Panel Fillet add control"
ui_expect_log "Panel Fillet input control.0.radius at" "the control radius row"
ui_mark
ui_step "6 mm"                             ui_type_in "Panel Fillet input control.0.radius" "6"
ui_expect_new "Preview Fillet: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_volume "Body Body1 (F2.b0): volume 48000.000 -> \([0-9.]*\) mm3" \
  "$(awk -v pi=$PI 'BEGIN { printf "%.6f", 48000 - (1 - pi / 4) * 60 * 248 / 15 }')" \
  "control radii 2, 6, 4 along the edge" 1e-5
ui_mark
ui_step "double-click Fillet1"             ui_double_click_logged "Timeline Fillet1"
ui_expect_new " with Fillet" "editing Fillet1"
ui_expect_new "Panel Fillet input control.0.radius at" "the middle control radius loaded as a row"
ui_step "cancel (Esc)"                     ui_key Escape
ui_expect_new "Command Fillet cancelled" "nothing changed"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_stop_app

echo "=== Sweep with an end scale and between two rails"
ui_start_app --open "$WORK/paths.mitcad"
ui_step "fit (F6)"                         ui_key F6
ui_step "sweep (search)"                   ui_command "Sweep"
ui_expect_log "Sweep Profiles: 1 profile [profile r{c1} of F3]" "the newest profile, the circle"
ui_step "the path input"                   ui_click_logged "Panel Sweep input path"
ui_step "pick the first line"              ui_click_logged "Sketch entity F1/c1"
ui_expect_log "Sweep Path: 1 sketch curve [sketch curve c1 of F1]" "path picked"
ui_mark
ui_step "End Scale 0.5"                    ui_type_in "Panel Sweep input end_scale" "0.5"
ui_expect_new "Preview Sweep: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added sweep (New Body) along 1 curve(s)" "sweep added"
ui_expect_volume "New body Body1 (F4.b0): volume \([0-9.]*\) mm3" \
  "$(awk -v pi=$PI 'BEGIN { printf "%.6f", pi * 50 / 3 * (16 + 8 + 4) }')" "a frustum of radii 4 and 2" 1e-5

# Sweep1 uses Sketch1, so it is hidden (mitcad#7): its light bulb shows it
# again for the rails and the chain.
ui_mark
ui_step "show Sketch1 (Browser)"           ui_click_logged "Browser eye Root/Sketches/Sketch1"
ui_expect_new "Visibility Root/Sketches/Sketch1: shown" "the paths' sketch shown"
ui_mark
ui_step "sweep (search)"                   ui_command "Sweep"
ui_step "Path + Guide Rail"                ui_choose "Panel Sweep input type" 1
ui_expect_new "Sweep: Type = Path + Guide Rail" "guide rail type"
ui_expect_new "Sweep Profiles: 1 profile [profile $R of F2]" "the rectangle, the newest unused profile"
ui_expect_new "Panel Sweep input rail2 at" "the second rail shown"
ui_expect_new "Panel Sweep input rail_point at" "the rail point shown (mitcad#469)"
ui_step "the path input"                   ui_click_logged "Panel Sweep input path"
ui_step "pick the middle line"             ui_click_logged "Sketch entity F1/c4"
ui_expect_new "Sweep Path: 1 sketch curve [sketch curve c4 of F1]" "path picked"
ui_step "the rail input"                   ui_click_logged "Panel Sweep input rail"
ui_step "pick the first rail"              ui_click_logged "Sketch entity F1/c7"
ui_expect_new "Sweep Guide Rail: 1 sketch curve [sketch curve c7 of F1]" "rail picked"
ui_step "the second rail input"            ui_click_logged "Panel Sweep input rail2"
ui_step "pick the second rail"             ui_click_logged "Sketch entity F1/c10"
ui_expect_new "Sweep Second Rail: 1 sketch curve [sketch curve c10 of F1]" "second rail picked"
ui_expect_new "Preview Sweep: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_volume "New body Body2 (F5.b0): volume \([0-9.]*\) mm3" \
  "$(awk 'BEGIN { printf "%.6f", 40 * (1.4 ^ 3 - 1) / (3 * 0.008) }')" "between the rails" 1e-5

echo "=== Pattern on Path along a chain of two lines"
ui_mark
ui_step "pattern on path (search)"         ui_command "Pattern on Path"
ui_step "pick Body1"                       ui_click_logged "Body F4.b0"
ui_step "the path input"                   ui_click_logged "Panel Pattern on Path input path"
ui_step "pick the chain's first line"      ui_click_logged "Sketch entity F1/c13"
ui_step "pick its second line"             ui_click_logged "Sketch entity F1/c16"
ui_expect_new "Pattern on Path Path: 2 sketch curves" "two lines picked"
ui_step "distance 25"                      ui_type_in "Panel Pattern on Path input distance" "25"
ui_expect_new "Preview Pattern on Path: ok" "previewed"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_volume "New body [A-Za-z0-9]* (F6.b1): volume \([0-9.]*\) mm3" \
  "$(awk -v pi=$PI 'BEGIN { printf "%.6f", pi * 50 / 3 * (16 + 8 + 4) }')" "the copy on the second line" 1e-5
ui_double_click_expect "Timeline PathPattern1" "Editing F6 with Pattern on Path" "editing the pattern"
ui_expect_new "Pattern on Path Path: 2 sketch curves" "the chain loaded"
ui_step "cancel (Esc)"                     ui_key Escape
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_finish "UI feature options test"
