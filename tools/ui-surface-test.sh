#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/commands
# The SURFACE group (mitcad#455) through the real UI, with the bodies the
# model reports (commands.md, "Surface modelling"):
#   - Surface Extrude: a 60 x 40 rectangle's profile (instead of the newest
#     one) and an open L of two lines beside it (Chain Selection), 10 mm:
#     two surface bodies, no volume; edited, its panel loads it.
#   - Surface Revolve: a line 5 mm off a sketch line, a full turn about it.
#   - Surface Sweep: a line along a line square to its sketch.
#   - Surface Loft: a line to the same line 10 mm above, two sections.
#   - Patch: a 40 x 40 square filled; Thicken: the patch 2 mm thick, a new
#     body of 3200 mm3 (the patch stays).
#   - Offset Surface: the thickened patch's top 3 mm out; Ruled Surface: a
#     strip off a wall's open edges; Extend: that wall past its open edges.
#   - Trim: the walls cut by Plane1 (10 mm up), the piece clicked kept;
#     Unstitch: one of their faces a surface body of its own; Reverse
#     Normal: a face of the revolved surface; Untrim: the offset face with
#     its outer boundary back, a new surface.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-surface-test.sh
# check-all sources: tools/cli core/model

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-surface.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

# Sketch1 (F1): a 60 x 40 rectangle (c1-c4, its points p5-p8) and an open
# L, (80, 0)-(90, 0) (c9) and (90, 0)-(90, 10) (c12); Sketch2 (F2) on YZ:
# the revolve's line (c1) and its axis (c4) apart; Sketch3 (F3) on XZ: the
# sweep's path; Sketch4 (F4): the swept line from the path's start;
# Sketch5 (F5) and Sketch6 (F7, on Plane1 F6 10 mm up): the loft's lines;
# Sketch7 (F8): a 40 x 40 square for the patch.
cat > "$WORK/surfaces.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [80, 0], "end": [90, 0]},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [90, 0], "end": [90, 10]},
  {"cmd": "sketch.create", "plane": "yz"},
  {"cmd": "sketch.add_line", "sketch": "F2", "start": [5, 0], "end": [5, 10]},
  {"cmd": "sketch.add_line", "sketch": "F2", "start": [0, -5], "end": [0, 15]},
  {"cmd": "sketch.create", "plane": "xz"},
  {"cmd": "sketch.add_line", "sketch": "F3", "start": [-40, 0], "end": [-40, 20]},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_line", "sketch": "F4", "start": [-40, 0], "end": [-30, 0]},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_line", "sketch": "F5", "start": [0, 60], "end": [10, 60]},
  {"cmd": "add_feature", "def": {"type": "construction_plane", "definition": {"type": "offset", "plane": "xy", "distance": 10}}},
  {"cmd": "sketch.create", "plane": "F6"},
  {"cmd": "sketch.add_line", "sketch": "F7", "start": [0, 60], "end": [10, 60]},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F8", "corner": [100, 0], "width": 40, "height": 40}
]
EOF
"$CLI" run "$WORK/surfaces.json" --save "$WORK/surfaces.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli surfaces"; }

ui_start_display
ui_start_app --open "$WORK/surfaces.mitcad"
ui_step "fit (F6)"                         ui_key F6

echo "--- Surface Extrude: the rectangle and the open L"
ui_step "surface extrude (search)"         ui_command "Surface Extrude"
ui_step "the profiles input"               ui_click_logged "Panel Surface Extrude input profiles"
ui_step "take the newest profile away"     ui_click_pick "profile F8/"
ui_step "pick the rectangle"               ui_click_pick "profile F1/"
ui_step "pick a line of the L"             ui_click_logged "Sketch entity F1/c9"
ui_expect_log "Preview Surface Extrude: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added surface extrude of 1 profiles and 1 curves" "surface extrude added"
ui_expect_log "(F9.b0): volume 0.000 mm3" "the rectangle's walls, a surface"
ui_expect_log "(F9.b1): volume 0.000 mm3" "the L's walls, a surface"

echo "--- Extrude1 edited: the Surface Extrude panel"
ui_step "double-click Extrude1"            ui_double_click_logged "Timeline Extrude1"
ui_expect_log "Editing F9 with Surface Extrude" "editing it with its own panel"
ui_step "distance 15"                      ui_type_in "Panel Surface Extrude input distance" "15"
ui_expect_log "Preview Surface Extrude: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Edited F9" "Extrude1 edited"

echo "--- Surface Revolve: a line a full turn about another"
ui_step "surface revolve (search)"         ui_command "Surface Revolve"
ui_step "the profiles input"               ui_click_logged "Panel Surface Revolve input profiles"
ui_step "pick the line"                    ui_click_logged "Sketch entity F2/c1"
ui_step "the axis input"                   ui_click_logged "Panel Surface Revolve input axis"
ui_step "pick the axis line"               ui_click_logged "Sketch entity F2/c4"
ui_expect_log "Preview Surface Revolve: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added surface revolve of 0 profiles and 1 curves" "surface revolve added"
ui_expect_log "(F10.b0): volume 0.000 mm3" "a cylinder's wall"

echo "--- Surface Sweep: a line along a line"
ui_step "surface sweep (search)"           ui_command "Surface Sweep"
ui_step "the profiles input"               ui_click_logged "Panel Surface Sweep input profiles"
ui_step "pick the swept line"              ui_click_logged "Sketch entity F4/c1"
ui_step "the path input"                   ui_click_logged "Panel Surface Sweep input path"
ui_step "pick the path"                    ui_click_logged "Sketch entity F3/c1"
ui_expect_log "Preview Surface Sweep: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added surface sweep of 0 profiles and 1 curves" "surface sweep added"
ui_expect_log "(F11.b0): volume 0.000 mm3" "the swept surface"

echo "--- Surface Loft: two lines in order"
ui_step "surface loft (search)"            ui_command "Surface Loft"
ui_step "the lower line"                   ui_click_logged "Sketch entity F5/c1"
ui_step "the upper line"                   ui_click_logged "Sketch entity F7/c1"
ui_expect_log "Preview Surface Loft: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added surface loft through 2 sections" "surface loft added"
ui_expect_log "(F12.b0): volume 0.000 mm3" "the lofted surface"

echo "--- Patch and Thicken"
ui_step "patch (search)"                   ui_command "Patch"
ui_step "pick the square"                  ui_click_pick "profile F8/"
ui_expect_log "Preview Patch: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added patch of 1 items" "patch added"
ui_expect_log "(F13.b0): volume 0.000 mm3" "the patch"
body=$(sed -n 's/^New body \(.*\) (F13\.b0): volume.*/\1/p' "$UI_LOG" | tail -1)
[ -n "$body" ] || ui_fail "the patch's body has no name in the log"
ui_step "thicken (search)"                 ui_command "Thicken"
ui_step "the patch in the browser"         ui_click_logged "Browser Root/Bodies/$body"
ui_step "thickness 2"                      ui_type_in "Panel Thicken input thickness" "2"
ui_expect_log "Preview Thicken: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added thicken (New Body) of 1 bodies" "thicken added"
ui_expect_volume "New body .* (F14.b0): volume \([0-9.]*\) mm3" 3200 "the patch 2 mm thick"

echo "--- Offset Surface, Ruled Surface and Extend"
ui_step "offset surface (search)"          ui_command "Offset Surface"
ui_step "the thickened patch's top"        ui_click_pick "face F14.b0/F14:front("
ui_step "distance 3"                       ui_type_in "Panel Offset Surface input distance" "3"
ui_expect_log "Preview Offset Surface: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added offset surface of 1 faces" "offset surface added"
ui_expect_log "(F15.b0): volume 0.000 mm3" "the offset face, a surface"
ui_step "ruled surface (search)"           ui_command "Ruled Surface"
ui_step "the open edges input"             ui_click_logged "Panel Ruled Surface input open_faces"
ui_step "a wall of the extrusion"          ui_click_pick "face F9.b0/F9:side("
ui_step "distance 3"                       ui_type_in "Panel Ruled Surface input distance" "3"
ui_expect_log "Preview Ruled Surface: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added ruled surface off 0 edges and the open edges of 1 faces" "ruled surface added"
ui_expect_log "(F16.b0): volume 0.000 mm3" "the strips, a surface"
ui_step "extend (search)"                  ui_command "Extend"
ui_step "the open edges input"             ui_click_logged "Panel Extend input open_faces"
ui_step "a wall of the extrusion"          ui_click_pick "face F9.b0/F9:side("
ui_step "distance 2"                       ui_type_in "Panel Extend input distance" "2"
ui_expect_log "Preview Extend: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added extend of 0 edges and the open edges of 1 faces" "extend added"

echo "--- Trim, Unstitch, Reverse Normal and Untrim"
ui_step "trim (search)"                    ui_command "Trim"
ui_expect_log "Command search chose surface.trim" "the surface trim"
walls=$(sed -n 's/^New body \(.*\) (F9\.b0): volume.*/\1/p' "$UI_LOG" | tail -1)
[ -n "$walls" ] || ui_fail "the extrusion's walls have no name in the log"
ui_step "the walls in the browser"         ui_click_logged "Browser Root/Bodies/$walls"
ui_step "the tool input"                   ui_click_logged "Panel Trim input tool"
ui_step "Plane1 (10 mm up) as the tool"    ui_click_logged "Browser Root/Construction/Plane1"
ui_step "the keep input"                   ui_click_logged "Panel Trim input keep"
ui_step "click the walls"                  ui_click_pick "face F9.b0/"
ui_expect_log "Preview Trim: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added trim keeping 1 pieces" "trim added"
ui_step "unstitch (search)"                ui_command "Unstitch"
ui_step "a face of the trimmed walls"      ui_click_pick "face F9.b0/"
ui_expect_log "Preview Unstitch: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added unstitch of 1 faces" "unstitch added"
ui_expect_log "(F19.b0): volume 0.000 mm3" "the face a surface body of its own"
ui_step "reverse normal (search)"          ui_command "Reverse Normal"
ui_step "the revolved surface"             ui_click_pick "face F10.b0/"
ui_expect_log "Preview Reverse Normal: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added reverse normal of 1 faces" "reverse normal added"
ui_step "untrim (search)"                  ui_command "Untrim"
ui_step "the offset face"                  ui_click_pick "face F15.b0/"
ui_step "the outer boundary too"           ui_click_logged "Panel Untrim input outer"
ui_expect_log "Preview Untrim: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Added untrim (holes closed, outer restored)" "untrim added"
ui_expect_log "(F21.b0): volume 0.000 mm3" "the untrimmed face, a surface"

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_stop_app
ui_finish "surface modelling"
