#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework/CommandSessionProxy.cpp app/framework/DragProxy.cpp
# check-all sources: app/commands/MoveCommands.cpp app/commands/MoveDragMotion.cpp
# check-all sources: app/framework/CommandSession.cpp app/framework/ManipulatorOverlay.cpp app/OcctViewer.cpp
# check-all sources: app/framework/DisplayMeshes.cpp geometry/src/transform.cpp core/model/src/features/moves.rs
# check-all sources: tools/cli
# Move/Copy's drags (mitcad#838) through the real UI, with held drags of its
# handles: 180 mouse moves 16 ms apart (xdotool on the hidden Xvfb
# display), a short pause half-way, then the release. On a design mitcad-cli
# makes: Body1 (F2.b0), a cylinder of 30 mm diameter, 20 high, in the root
# (curved faces), and Component1:1 (O1), a 30 x 20 x 10 block 60 mm along X.
#   - Body1's X arrow dragged: the body follows the handle at once ("Drag
#     proxy Move/Copy: shown for tx"), with no model preview per move
#     (only when the pointer rests or the button goes up); the exact
#     preview of the last value replaces it after the release; the moved
#     body is drawn from the meshes it had (no face meshed again for the
#     preview nor for OK), and OK moves it by the last value.
#   - Body1's Z ring dragged: the same for a turn.
#   - Component1:1's Z arrow dragged: its body follows, only placed
#     elsewhere in the view (its drawn objects kept: "0 new, 1 placed"),
#     and OK moves the component.
# Each drag's line is printed: inputs (snapped values), proxy updates,
# exact previews while held, frames drawn and the time held.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-move-drag-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CLI=${UI_CLI:-$(dirname "$UI_APP")/../tools/cli/mitcad-cli}
mkdir -p "$ROOT/build"
WORK=$(mktemp -d "$ROOT/build/ui-move-drag.XXXXXX")
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
FILE=$WORK/design.mitcad
R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
export MITCAD_LOG_TIMING=1
export MITCAD_MESH_TRACE=1

# new_line "regex": the first line since the mark that matches.
new_line() { tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -m1 -E -- "$1"; }

# held_drag "logged text" dx dy: a held left drag from the last "<text> at
# x,y" by (dx, dy) pixels in 180 moves 16 ms apart, six ticks' pause
# half-way (shorter than the rest that asks for an exact preview), then the
# release.
held_drag() {
  local fx fy i steps=180 args=()
  read -r fx fy <<< "$(ui_logged_at "$1")"
  ui_apart "$fx" "$fy"
  args=(mousemove "$fx" "$fy" mousedown 1)
  for ((i = 1; i <= steps; i++)); do
    [ "$i" = $((steps / 2)) ] && args+=(sleep 0.096)
    args+=(mousemove $((fx + $2 * i / steps)) $((fy + $3 * i / steps)) sleep 0.016)
  done
  xdotool "${args[@]}" mouseup 1
}

# drag_summary label key [min]: checks and prints the drag's line; at
# least min inputs (default 10), the bodies follow (proxy updates) and the
# model previewed only a few times while the button was held.
drag_summary() {
  local line inputs proxies exacts
  ui_expect_new " proxy updates (every " "$1: the drag's counts logged" 20 > /dev/null
  line=$(new_line "Drag proxy Move/Copy $2: [0-9]+ inputs, ")
  [[ $line =~ :\ ([0-9]+)\ inputs,\ ([0-9]+)\ proxy\ updates.*,\ ([0-9]+)\ exact\ previews ]] ||
    ui_fail "$1: drag line not understood: $line"
  inputs=${BASH_REMATCH[1]} proxies=${BASH_REMATCH[2]} exacts=${BASH_REMATCH[3]}
  echo "Drag $1: ${line#*: }"
  [ "$inputs" -ge "${3:-10}" ] || ui_fail "$1: only $inputs inputs delivered"
  [ "$proxies" -ge "${3:-10}" ] || ui_fail "$1: the bodies followed only $proxies times"
  # No model preview per move: one when the pointer rested (a loaded
  # machine may rest it more than once), none otherwise.
  [ "$exacts" -le 3 ] || ui_fail "$1: $exacts exact previews while the button was held ($inputs inputs)"
  echo "ok   $1: $proxies proxy updates, $exacts exact previews while held"
}

# no_meshing label: the last display meshing since the mark of each tier
# made no face anew (the moved bodies kept their meshes).
no_meshing() {
  local line
  line=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -E "Mesh trace .* $2: " | tail -1)
  [ -n "$line" ] || ui_fail "$1: no $2 display meshing logged"
  [[ $line =~ bodies\ ready,\ 0\ meshed ]] || ui_fail "$1: faces meshed again: $line"
  echo "ok   $1: nothing meshed again (${line#*: })"
}

cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 30},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_component"}},
  {"cmd": "set_occurrence_transform", "occurrence": "Component1:1", "transform": {"translation": [60, 0, 0]}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Browser Root/Bodies/Body1 at" "the browser lists Body1"
ui_expect_log "Browser Root/Component1:1 at" "and Component1:1"

echo "=== Body1's X arrow"
ui_mark
ui_step "right-click Body1's row"        ui_click_logged "Browser Root/Bodies/Body1" 3
ui_step "choose Move/Copy"               ui_menu_choose "Move/Copy"
ui_expect_new "Move/Copy Objects: 1 body [body F2.b0]" "Move/Copy on Body1"
ui_wait_idle
ui_expect_new "Preview Move/Copy: ok" "previewed" 30
ui_expect_log "Manipulator Move/Copy tx at " "the X arrow" > /dev/null
ui_focus_main
ui_mark
held_drag "Manipulator Move/Copy tx" 120 0
ui_expect_new "Manipulator Move/Copy tx dragged to " "released" 20
value=$(new_line "Manipulator Move/Copy tx dragged to " | sed 's/.* dragged to //')
drag_summary "X arrow" tx
ui_expect_new "Drag proxy Move/Copy: shown for tx " "the body follows the arrow"
line=$(new_line "Drag proxy Move/Copy: shown for tx ")
[[ $line =~ at\ [0-9.-]+,0\.000,10\.000$ ]] || ui_fail "the body is not shown along X from its place: $line"
ui_wait_idle
ui_expect_new "Drag proxy Move/Copy: the exact preview of $value mm replaced it " \
  "the exact preview of the last value $value mm replaces it" 30
no_meshing "the exact preview" interactive
ui_focus_main
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Moved 1 bodies (Free Move)" "OK moves Body1" 30
ui_wait_idle
no_meshing "the move" final
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -q "Drag proxy Move/Copy: shown" && ui_fail "a proxy after OK"

echo "=== Body1's Z ring"
ui_mark
ui_step "right-click Body1's row"        ui_click_logged "Browser Root/Bodies/Body1" 3
ui_step "choose Move/Copy"               ui_menu_choose "Move/Copy"
ui_expect_new "Move/Copy Objects: 1 body [body F2.b0]" "Move/Copy on Body1 again"
ui_wait_idle
ui_expect_new "Preview Move/Copy: ok" "previewed" 30
ui_expect_log "Manipulator Move/Copy rz at " "the Z ring" > /dev/null
ui_focus_main
ui_mark
# Across the ring and along it: the angle changes whichever way the ring
# is seen.
held_drag "Manipulator Move/Copy rz" 70 40
ui_expect_new "Manipulator Move/Copy rz dragged to " "released" 20
drag_summary "Z ring" rz 3
ui_expect_new "Drag proxy Move/Copy: shown for rz " "the body turns with the ring"
ui_wait_idle
ui_expect_new "Drag proxy Move/Copy: the exact preview of " "the exact preview replaces it" 30
no_meshing "the turned preview" interactive
ui_focus_main
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Moved 1 bodies (Free Move)" "OK turns Body1" 30
ui_wait_idle
no_meshing "the turn" final

echo "=== Component1:1's Z arrow"
ui_mark
ui_step "click Component1:1"             ui_click_logged "Browser Root/Component1:1"
ui_expect_new "Selected: 1 component" "Component1:1 selected"
ui_mark
ui_step "right-click Component1:1"       ui_click_logged "Browser Root/Component1:1" 3
ui_step "choose Move/Copy"               ui_menu_choose "Move/Copy"
ui_expect_new "Panel Move/Copy choices: move_type=components" "moving the component"
ui_wait_idle
ui_expect_new "Preview Move/Copy: ok" "previewed" 30
ui_expect_log "Manipulator Move/Copy tz at " "the Z arrow" > /dev/null
ui_focus_main
ui_mark
held_drag "Manipulator Move/Copy tz" 0 -80
ui_expect_new "Manipulator Move/Copy tz dragged to " "released" 20
drag_summary "component's Z arrow" tz
ui_expect_new "Drag proxy Move/Copy: shown for tz " "the component's body follows the arrow"
line=$(new_line "Drag proxy Move/Copy: shown for tz ")
[[ $line =~ at\ 75\.000,10\.000,[0-9.-]+$ ]] || ui_fail "the component's body is not shown along Z from its place: $line"
# Only placed elsewhere: its drawn objects kept and moved.
ui_expect_new " 0 new, 1 placed" "the body's drawn objects only placed elsewhere"
ui_wait_idle
ui_expect_new "Drag proxy Move/Copy: the exact preview of " "the exact preview replaces it" 30
ui_focus_main
ui_mark
ui_step "OK (Enter)"                     ui_key Return
ui_expect_new "Moved 1 components (" "OK moves Component1:1" 30
ui_wait_idle
grep -q "faces that could not be meshed" "$UI_LOG" && ui_fail "faces failed to mesh"
grep -qE "The document was read while a job computes it|OCCT view event failed" "$UI_LOG" &&
  ui_fail "the model or the view was used while a job computed"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_stop_app
echo "PASS: Move/Copy drags"
