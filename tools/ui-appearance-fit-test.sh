#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser/AppearanceDialog.cpp app/browser/AppearanceDialog.hpp app/framework/FitToScreen.cpp
# check-all sources: app/framework/FitToScreen.hpp app/framework/TestDriverWindowFit.cpp tools/ui-scripts/appearance-fit.mitcad-ui
# Appearances on a screen lower than its contents (mitcad#818), on a
# private Xvfb display 600 logical pixels high: the dialog opens within the
# screen's available area, its body scrolls and its buttons stay visible
# below it, also in a compact dialog; in the Floating and the Docked layout
# (tools/ui-scripts/appearance-fit.mitcad-ui).
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-test-lib.sh"
trap 'ui_cleanup' EXIT
UI_SCREEN_HEIGHT=600
ui_start_display
for chrome in floating docked; do
  MITCAD_TEST_INPUT=$here/ui-scripts/appearance-fit.mitcad-ui \
    ui_start_app --ui=qt --no-mcp --demo --chrome=$chrome
  for _ in $(seq 1 600); do
    kill -0 "$UI_RUNNER" 2> /dev/null || break
    sleep 0.2
  done
  kill -0 "$UI_RUNNER" 2> /dev/null && { ui_fail "the Appearances fit script ($chrome) timed out"; exit 1; }
  wait "$UI_RUNNER" || { cat "$UI_LOG" >&2; exit 1; }
  ui_expect_log 'TestDriver: done' "Appearances fits the low screen with its buttons visible ($chrome)"
done
ui_finish 'Appearances within the screen'
