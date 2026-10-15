#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/render app/view
# The rendered view seen from below the ground (mitcad#320, docs/rendering.md,
# Display), on a red 40 mm box with the ground's shadows on: the ground is
# Cycles' shadow catcher, whose matte would leave only the background when
# the camera looks at it from below. In the orthographic and then the
# perspective view, the camera orbits under the ground (Shift + middle
# drags) and back above it: the worker leaves the ground out below and
# builds it again above ("render-ground off|on"), and every view renders
# the box: its frames' opaque pixels (what the frame's alpha covers) are a
# good part of the view, not none (only the background).
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-render-ground-test.sh
# check-all sources: tools/cli

UI_APP=${UI_APP:-$(cd "$(dirname "$0")/.." && pwd)/build/dev/app/mitcad}
BUILD_DIR=$(cd "$(dirname "$UI_APP")/.." 2> /dev/null && pwd)
source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$BUILD_DIR/tools/cli/mitcad-cli}
WORK=$(mktemp -d "${TMPDIR:-/tmp}/mitcad-ui-render-ground.XXXXXX")
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
# Few samples: the test checks what the frames cover, not how they look.
export MITCAD_RENDER_SAMPLES=8
mkdir -p "$UI_CONFIG/Mitcad"
printf '[render]\ndevice=cpu\n' >> "$UI_CONFIG/Mitcad/Mitcad.conf"

# A red 40 mm box (F1.b0) on the XY plane; the ground (shadows) is on by
# default, under the box.
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-20, -20],
    "length": 40, "width": 40, "height": 40, "operation": "new_body"}},
  {"cmd": "set_body_appearance", "uid": "F1.b0", "appearance": "paint_red"}
]
EOF
"$CLI" run "$WORK/model.json" --save "$WORK/box.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# last_view: the sequence number of the last view sent to the worker.
last_view() { grep -o "Render view [0-9]*: " "$UI_LOG" | tail -1 | tr -dc 0-9; }

# expect_box description: the last view gets all its samples, and its last
# frame's opaque pixels cover at least a twentieth of the view (the box;
# "covers 0,0 0,0" when only the background shows).
expect_box() {
  local view line x0 y0 x1 y1 area vw vh
  for _ in $(seq 1 300); do
    view=$(last_view)
    [ -n "$view" ] && grep -qE "Render view $view: $MITCAD_RENDER_SAMPLES samples in" "$UI_LOG" && break
    ui_crashed && ui_fail "$1: the app crashed"
    sleep 0.2
  done
  grep -qE "Render view $view: $MITCAD_RENDER_SAMPLES samples in" "$UI_LOG" ||
    ui_fail "$1: view $view did not get its $MITCAD_RENDER_SAMPLES samples"
  for _ in $(seq 1 50); do
    line=$(grep -E "^Render frame view $view [0-9]+x[0-9]+ samples $MITCAD_RENDER_SAMPLES( denoised)? covers " \
      "$UI_LOG" | tail -1)
    [ -n "$line" ] && break
    sleep 0.2
  done
  [ -n "$line" ] || ui_fail "$1: no frame of view $view with all samples shown"
  read -r x0 y0 x1 y1 <<< "$(sed -n 's/.* covers \([0-9-]*\),\([0-9-]*\) \([0-9-]*\),\([0-9-]*\)$/\1 \2 \3 \4/p' <<< "$line")"
  read -r _ _ _ _ vw vh <<< "$(grep -o "View area [0-9]* [0-9]* [0-9]* [0-9]*" "$UI_LOG" | tail -1)"
  area=$(((x1 - x0) * (y1 - y0)))
  [ "$area" -gt $((vw * vh / 20)) ] ||
    ui_fail "$1: the frame covers ${x0},${y0} ${x1},${y1} of a ${vw} x ${vh} view: the box is not rendered"
  echo "ok   $1: view $view covers ${x0},${y0} ${x1},${y1}"
}

# camera_z: the z of the camera's last logged direction (from the eye to
# the model; above 0 while it looks up).
camera_z() { grep -o '^Camera direction [-0-9.]* [-0-9.]* [-0-9.]*' "$UI_LOG" | tail -1 | awk '{ print $5 }'; }

# orbit_drag pixels: one orbit, Shift + middle drag down the view by that
# share of its height (negative: up), then the camera at rest.
orbit_drag() {
  local i
  ui_mark
  xdotool keydown shift
  ui_apart $(ui_view_at 50 50)
  xdotool mousemove $(ui_view_at 50 $((50 - $1 / 2))) mousedown 2
  for i in 1 2 3 4 5 6; do
    xdotool mousemove $(ui_view_at 50 $((50 - $1 / 2 + $1 * i / 6)))
    sleep 0.05
  done
  xdotool mouseup 2
  xdotool keyup shift
  ui_sync
  ui_expect_new "Camera direction " "the camera came to rest" 10 > /dev/null
}

# orbit_until off|on description: orbits up or down the view, 30 % of its
# height at a time (the other way when a drag turned the camera the wrong
# way), until the worker logs the ground off (under it) or on (above it).
DRAG=30
orbit_until() {
  local want=$1 before after mark i
  mark=$(wc -l < "$UI_LOG")
  for i in 1 2 3 4 5 6 7 8 9 10; do
    tail -n +$((mark + 1)) "$UI_LOG" | grep -q "Render worker: render-ground $want" && break
    before=$(camera_z)
    orbit_drag "$DRAG"
    after=$(camera_z)
    # Under the ground the camera looks up (z grows), above it down.
    if awk -v b="$before" -v a="$after" -v w="$want" \
      'BEGIN { exit !((w == "off" && a < b) || (w == "on" && a > b)) }'; then
      DRAG=$((-DRAG))
    fi
    sleep 0.5
  done
  tail -n +$((mark + 1)) "$UI_LOG" | grep -q "Render worker: render-ground $want" ||
    ui_fail "$2: the worker did not turn the ground $want (camera direction z $(camera_z))"
  echo "ok   $2: the ground $want (camera direction z $(camera_z))"
}

ui_start_display
ui_start_app --open "$WORK/box.mitcad" --no-native-dialogs
ui_expect_log "Opened $WORK/box.mitcad" "the box opened"
ui_step "fit (F6)"                       ui_key F6

for projection in Orthographic Perspective; do
  echo "--- $projection: under the ground and back"
  ui_step "$projection"                  ui_command "$projection"
  ui_step "Isometric View"               ui_command "Isometric View"
  if ! grep -q "Rendered view on" "$UI_LOG"; then
    ui_step "View > Rendered"            ui_command "Rendered"
    ui_expect_log "Render worker ready: Cycles" "the worker runs Cycles" 30
  fi
  ui_expect_log "Render environment studio, background view, ground shadows" "the ground's shadows are on"
  expect_box "$projection, above the ground"
  orbit_until off "$projection, orbited under the ground"
  expect_box "$projection, from below the ground"
  orbit_until on "$projection, orbited above the ground again"
  expect_box "$projection, above the ground again"
done
grep -q "Render worker error" "$UI_LOG" && ui_fail "the worker reported an error"

ui_finish "the rendered view from below the ground"
