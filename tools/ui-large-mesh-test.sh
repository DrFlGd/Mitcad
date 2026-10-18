#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/view app/OcctViewer.cpp app/framework/DisplayMeshes.cpp app/framework/DocumentSession.cpp
# check-all sources: geometry/io tools/cli
# Building-scale curved faces in the window (mitcad#398), through the real
# UI on a synthetic design: a hemispherical dome shell 9 m across (sphere
# faces of 4.45 and 4.55 m radius, a full revolve of two quarter arcs in
# metres) and a half revolve of the same profile, suppressed. At the view's
# 0.01 mm each sphere face took millions of triangles on the UI thread:
# the window never appeared, and MCP calls got no answer.
#   - The design opens: the window appears within the usual wait, the dome
#     is meshed on the model's thread in the opening job ("Display meshes
#     of ..."), and the view keeps those meshes (it draws within the
#     bounded triangles, kViewFaceTriangles a face).
#   - The dome can be hovered and picked.
#   - Unsuppressing the half revolve (a model command, as an agent's
#     add_feature over MCP is) computes it and meshes it in the job too.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-large-mesh-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-large-mesh.XXXXXX)
FILE=$WORK/dome.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# The profile on xz (sketch y = -Z): quarter arcs of 4.55 and 4.45 m about
# a centre 40.12 m up, closed by two lines; revolved fully about z (F2,
# Body1) and by half a turn (F3, Body2, suppressed).
REGION='r{c7[c8,c10],c8[c7,c9],c9[c8,c10],c10[c9,c7]}'
cat > "$WORK/model.json" << EOF
[
  {"cmd": "add_feature", "name": "Dome sketch", "def": {"type": "sketch", "plane": "xz",
   "entities": [{"id": "p1", "type": "point", "at": [0, 0], "fixed": true},
    {"id": "p2", "type": "point", "at": [0, -40120]}, {"id": "p3", "type": "point", "at": [4550, -40120]},
    {"id": "p4", "type": "point", "at": [0, -44670]}, {"id": "p5", "type": "point", "at": [0, -44570]},
    {"id": "p6", "type": "point", "at": [4450, -40120]},
    {"id": "c7", "type": "arc", "center": "p2", "start": "p4", "end": "p3"},
    {"id": "c8", "type": "line", "start": "p4", "end": "p5"},
    {"id": "c9", "type": "arc", "center": "p2", "start": "p5", "end": "p6"},
    {"id": "c10", "type": "line", "start": "p6", "end": "p3"}],
   "constraints": [{"id": "k1", "type": "vertical_points", "a": "p1", "b": "p2"},
    {"id": "k5", "type": "vertical_points", "a": "p1", "b": "p4"},
    {"id": "k7", "type": "vertical_points", "a": "p1", "b": "p5"}],
   "dimensions": [{"id": "k2", "type": "vertical_distance", "a": "p1", "b": "p2", "value": "40.12 m"},
    {"id": "k3", "type": "horizontal_distance", "a": "p1", "b": "p3", "value": "4.55 m"},
    {"id": "k4", "type": "vertical_distance", "a": "p1", "b": "p3", "value": "40.12 m"},
    {"id": "k6", "type": "vertical_distance", "a": "p1", "b": "p4", "value": "44.67 m"},
    {"id": "k8", "type": "vertical_distance", "a": "p1", "b": "p5", "value": "44.57 m"},
    {"id": "k9", "type": "horizontal_distance", "a": "p1", "b": "p6", "value": "4.45 m"},
    {"id": "k10", "type": "vertical_distance", "a": "p1", "b": "p6", "value": "40.12 m"}]}},
  {"cmd": "add_feature", "name": "Dome revolve", "def": {"type": "revolve",
   "profiles": [{"sketch": "F1", "region": "$REGION"}],
   "axis": "z", "extent": {"type": "full"}, "operation": "new_body"}},
  {"cmd": "add_feature", "name": "Half dome", "def": {"type": "revolve",
   "profiles": [{"sketch": "F1", "region": "$REGION"}],
   "axis": "z", "extent": {"type": "angle", "angle": 3.14159265358979}, "operation": "new_body"}},
  {"cmd": "suppress_feature", "uid": "F3"},
  {"expect": {"bodies": 1}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

echo "--- Opening the dome"
ui_start_display
# The usual wait for the window (a debug build on a software renderer).
UI_WINDOW_WAIT=30 ui_start_app --open "$FILE"
ui_expect_log "Display meshes of dome.mitcad: 1 bodies in" "the dome was meshed on the model's thread while opening"
ui_expect_log "Timeline Dome revolve at" "the timeline lists the revolve"
grep -q "Failed features: [^n]" "$UI_LOG" && ui_fail "a feature failed"
ui_step "fit (F6)"                        ui_key F6

echo "--- Hover and pick"
ui_mark
ui_step "hover over the dome"             xdotool mousemove $(ui_logged_at "Body F2.b0")
ui_step "click the dome"                  ui_click_logged "Body F2.b0"
ui_expect_new "Selected: 1 " "a click on the dome selects it"

echo "--- A model command computes a large body"
ui_mark
ui_step "right-click Half dome"           ui_click_logged "Timeline Half dome" 3
ui_step "choose Unsuppress Features"      ui_menu_choose "Unsuppress Features"
ui_expect_new "Unsuppressed Half dome" "unsuppressed"
ui_expect_new "Display meshes of suppress_feature: 1 bodies in" "the half dome was meshed in the command's job"
ui_step "hover over the half dome"        xdotool mousemove $(ui_logged_at "Body F3.b0")
ui_crashed && ui_fail "the application crashed"

ui_finish "UI large mesh test"
