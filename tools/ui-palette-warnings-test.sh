#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch/PaletteWarnings.cpp app/sketch/PaletteWarnings.hpp app/sketch/SketchPalette.cpp
# check-all sources: app/sketch/SketchReveal.cpp app/sketch/SketchModel.cpp app/view/ViewReveal.cpp
# check-all sources: core/model-api/src/sketch.rs core/sketch core/model/src/features/sketch.rs tools/cli
# The Sketch Palette's warnings select what they are about (mitcad#264),
# through the real UI:
#   - A design (mitcad-cli): a 60 x 40 block 20 high, a fillet of 5 on its
#     vertical edge at (60, 0); a sketch on XY holds the block's bottom face
#     as an earlier version recorded it (no by_geometry), 70 wide from
#     x = -10 with sharp corners, and a line of the user's at x = 20. The
#     face has an arc now: the projection keeps its geometry and warns.
#   - Edit Sketch2: the palette shows the warning as a link (underlined, a
#     pointing hand, the tooltip "Click to select the curves this is
#     about").
#   - The view zoomed in far from the sketch; a click on the warning selects
#     the projection's 8 curves and points (the selection's count matches
#     the warning's entities) and the view moves to them.
#   - A line of the sketch that has no length (mitcad#365) is a second warning,
#     also a link: its click selects that line.
#   - A second click replaces the selection with the same; Delete takes the
#     selection (Deleted ...), Undo brings it back.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-palette-warnings-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-palette-warnings.XXXXXX)
FILE=$WORK/stale.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$RECT"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "fillet", "body": "F2.b0",
    "edges": ["E{F2:side(c1[c4,c2])|F2:side(c2[c1,c3])}"], "radius": 5}},
  {"cmd": "add_feature", "def": {"type": "sketch", "plane": "xy",
    "entities": [
      {"id": "p1", "type": "point", "at": [-10, 0], "fixed": true, "reference": true},
      {"id": "p2", "type": "point", "at": [60, 0], "fixed": true, "reference": true},
      {"id": "c3", "type": "line", "start": "p1", "end": "p2", "fixed": true, "reference": true},
      {"id": "p4", "type": "point", "at": [60, 40], "fixed": true, "reference": true},
      {"id": "c5", "type": "line", "start": "p2", "end": "p4", "fixed": true, "reference": true},
      {"id": "p6", "type": "point", "at": [-10, 40], "fixed": true, "reference": true},
      {"id": "c7", "type": "line", "start": "p4", "end": "p6", "fixed": true, "reference": true},
      {"id": "c8", "type": "line", "start": "p6", "end": "p1", "fixed": true, "reference": true},
      {"id": "p9", "type": "point", "at": [20, 0]},
      {"id": "p10", "type": "point", "at": [20, 40]},
      {"id": "c11", "type": "line", "start": "p9", "end": "p10"},
      {"id": "p14", "type": "point", "at": [30, 20]},
      {"id": "p15", "type": "point", "at": [30, 20]},
      {"id": "c16", "type": "line", "start": "p14", "end": "p15"}],
    "constraints": [
      {"id": "k12", "type": "coincident", "point": "p9", "entity": "c3"},
      {"id": "k13", "type": "coincident", "point": "p10", "entity": "c7"}],
    "projections": [{"source": "F2:start($RECT)",
                     "entities": ["p1", "p2", "c3", "p4", "c5", "p6", "c7", "c8"]}]}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Timeline Sketch2 at" "the timeline is logged"

echo "--- The warning as a link"
ui_double_click_expect "Timeline Sketch2" "Editing sketch F4 (Sketch2)" "Sketch2 is edited"
ui_expect_log "Palette warning 0 at" "the palette shows the warning"
look=$(grep -F "Palette warning 0 at " "$UI_LOG" | tail -1)
case $look in
  *': a link (pointing hand), tooltip "Click to select the curves this is about"') echo "ok   the warning looks like a link" ;;
  *) ui_fail "the warning does not look like a link: $look" ;;
esac

echo "--- A click selects what it is about, the view goes there"
# Zoomed in at the view's corner: the sketch leaves the view.
read -r cx cy <<< "$(ui_view_at 4 6)"
for _ in $(seq 1 15); do
  xdotool mousemove "$cx" "$cy" click 4
  sleep 0.05
done
ui_sync
ui_mark
ui_step "click the warning"              ui_click_logged "Palette warning 0"
ui_expect_new "Palette: warning 0 selects 8 entities: p1, p2, c3, p4, c5, p6, c7, c8" \
  "the click selects the projection's curves and points"
ui_expect_new "Selected: 4 sketch curves, 4 sketch points" "the selection holds them"
selects=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -o 'Palette: warning 0 selects [0-9]* entities' | tail -1 |
  sed 's/.* selects \([0-9]*\) entities/\1/')
selected=0
for n in $(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep '^Selected: ' | tail -1 | sed 's/^Selected: //; s/ \[.*//' |
  grep -o '[0-9][0-9]*'); do
  selected=$((selected + n))
done
[ "$selected" = "$selects" ] || ui_fail "the selection has $selected entities, the warning $selects"
echo "ok   the selection's count matches the warning's entities ($selected)"
ui_expect_new "Sketch view to the selection: " "the view moves to them"

echo "--- Again, then Delete and Undo"
ui_mark
ui_step "click the warning again"        ui_click_logged "Palette warning 0"
ui_expect_new "Palette: warning 0 selects 8 entities" "a second click selects them again"
grep -q "^Selected: nothing" <(tail -n +$((UI_MARK + 1)) "$UI_LOG") && ui_fail "the second click cleared the selection"
ui_mark
ui_step "Delete"                         ui_key Delete
ui_expect_new "Deleted p1, p2, c3, p4, c5, p6, c7, c8" "Delete takes the selection"
ui_mark
ui_step "Undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: " "Undo brings them back"
ui_expect_new "Palette warning 0 at" "the warning is back"

echo "--- A collapsed line (mitcad#365)"
ui_expect_log "Palette warning 1 at" "the collapsed line has a warning of its own"
ui_mark
ui_step "click the collapsed line's warning" ui_click_logged "Palette warning 1"
ui_expect_new "Palette: warning 1 selects 1 entities: c16" "the click selects the collapsed line"

ui_finish "UI palette warnings test"
