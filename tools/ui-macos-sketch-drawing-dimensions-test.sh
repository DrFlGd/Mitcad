#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/commands/SketchCommands app/framework/TestDriver
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "usage: $0 <app> [output directory]" >&2
  exit 2
fi
ui_init "$1" "${2:-ui-macos-sketch-drawing-dimensions}"
UI_TIMEOUT=240
for chrome in floating docked; do
  export MITCAD_TEST_WORK=$UI_OUT/$chrome/files
  mkdir -p "$MITCAD_TEST_WORK"
  MITCAD_TEST_INPUT=$here/ui-scripts/sketch-drawing-dimensions.mitcad-ui \
    ui_run_app "$chrome" --ui=qt --chrome="$chrome" --no-recovery --no-mcp
  ui_expect_log 'TestDriver: done' "$chrome: drawing field geometry, labels, locking and Tab passed"
  ui_expect_no_log 'TestDriver: FAIL' "$chrome: no failed drawing assertion"
done
[ "$UI_FAILURES" = 0 ] || exit 1
echo 'PASS: sketch drawing dimension fields'
