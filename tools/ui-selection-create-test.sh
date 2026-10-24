#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindowSelectionCreate.cpp app/framework/SelectionCreate.cpp
# check-all sources: app/framework/SelectionCreate.hpp app/framework/WindowStyle.cpp app/MainWindow.cpp
# check-all sources: app/framework/Command.cpp app/framework/CommandSession.cpp app/commands tools/cli
# The selection card's Create row (mitcad#619) through the real UI, in the
# Docked and the Floating layout, on a design mitcad-cli makes: a cylinder
# 20 mm across and 30 high at the origin (Extrude1, F2.b0, its round face
# F2:side(c1)) and an unused 20 mm square beside it (Sketch2, F3):
#   1. The square's profile selected: the row offers Extrude, Revolve and
#      Sweep ("Selection card create: Extrude | Revolve | Sweep"; no Loft
#      for one profile).
#   2. Its Extrude button clicked: Extrude starts with that profile in
#      Profiles, as from the ribbon with the selection. Esc cancels.
#   3. The cylinder's round face selected: Thread first (no Extrude: the
#      face is not planar).
#   4. The selection cleared (Esc): "Selection card create: none".
# The Windows counterpart is tools/ui-windows-selection-create-test.ps1
# (ctest app.ui-windows-selection-create).
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-selection-create-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CLI=${UI_CLI:-$(dirname "$UI_APP")/../tools/cli/mitcad-cli}
mkdir -p "$ROOT/build"
WORK=$(mktemp -d "$ROOT/build/ui-selection-create.XXXXXX")
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
FILE=$WORK/create.mitcad
RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
SIDE='F2:side(c1)'

cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 30}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [30, -10], "width": 20, "height": 20}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# expect_create "A | B": the row's log line is exactly that, since the mark
# (up to 6 s).
expect_create() {
  for _ in $(seq 1 30); do
    tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qxF -- "Selection card create: $1" &&
      { echo "ok   the row: $1"; return; }
    sleep 0.2
  done
  ui_fail "the row is not '$1': $(grep -F 'Selection card create: ' "$UI_LOG" | tail -1)"
}

# round LAYOUT [app arguments...]: the four steps in a layout.
round() {
  local layout=$1
  shift
  echo "=== $layout"
  ui_start_app --open "$FILE" "$@"
  ui_step "fit (F6)"                       ui_key F6
  ui_expect_log "Bodies shown: F2.b0" "the cylinder shown"

  # Extrude once for the log's pick places of the profiles: it takes the
  # square, the newest profile. Cancelled.
  ui_mark
  ui_step "extrude (E)"                    ui_key e
  ui_expect_new "Extrude Profiles: 1 profile [profile $RECT of F3]" "with nothing selected, the square"
  ui_expect_new "Pick profile F3/$RECT at" "the square can be picked"
  ui_step "cancel (Esc)"                   ui_key Escape
  ui_expect_new "Command Extrude cancelled" "cancelled"

  echo "--- 1. A profile selected"
  ui_mark
  ui_step "select the square"              ui_click_pick "profile F3/$RECT"
  ui_expect_new "Selected: 1 profile [profile $RECT of F3]" "the square selected"
  expect_create "Extrude | Revolve | Sweep"
  ui_expect_new "Selection card create Extrude at " "its Extrude button laid out"

  echo "--- 2. Its Extrude button"
  ui_mark
  ui_step "click Extrude on the card"      ui_click_logged "Selection card create Extrude"
  ui_expect_new "Selection card create chose Extrude" "the card's button clicked"
  ui_expect_new "Command Extrude started" "Extrude started"
  ui_expect_new "Extrude Profiles: 1 profile [profile $RECT of F3]" "with the selected profile"
  ui_expect_new "Preview Extrude: ok" "previewed"
  ui_mark
  ui_step "cancel (Esc)"                   ui_key Escape
  ui_expect_new "Command Extrude cancelled" "cancelled"

  echo "--- 3. A cylinder's round face selected"
  # Thread once for the log's pick places of the round face. Cancelled.
  ui_mark
  ui_step "thread (search)"                ui_command "Thread"
  ui_expect_new "Command Thread started" "Thread started"
  ui_expect_new "Pick face F2.b0/$SIDE at" "the round face can be picked"
  ui_step "cancel (Esc)"                   ui_key Escape
  ui_expect_new "Command Thread cancelled" "cancelled"
  ui_mark
  ui_step "select the round face"          ui_click_pick "face F2.b0/$SIDE"
  ui_expect_new "Selected: 1 face [face $SIDE of F2.b0" "the round face selected"
  ui_expect_new "Selection card create: Thread" "Thread first"
  grep -F "Selection card create: " "$UI_LOG" | tail -1 | grep -qF "Extrude" &&
    ui_fail "Extrude offered for a round face: $(grep -F 'Selection card create: ' "$UI_LOG" | tail -1)"
  echo "ok   no Extrude for the round face"

  echo "--- 4. The selection cleared"
  ui_mark
  ui_step "clear the selection (Esc)"      ui_key Escape
  ui_expect_new "Selected: nothing" "nothing selected"
  expect_create "none"

  grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
  ui_stop_app
}

ui_start_display
round "Docked"
round "Floating" --chrome=floating
ui_finish "UI selection create"
