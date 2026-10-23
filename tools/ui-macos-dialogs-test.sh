#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework/CommandSearch app/framework/TestDriver app/browser/ParametersDialog app/platform/MacCommandSearch.hpp app/platform/MacParameters.hpp
# Focused coverage for the primary Qt command search and parameter table.
# Theme requests are confined to each test process; no system setting changes.
# Usage: tools/ui-macos-dialogs-test.sh <app> [out directory]
# Set MITCAD_UI_REQUIRE_RETINA=1 when collecting Retina acceptance evidence.

set -u
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=tools/ui-macos-lib.sh
. "$here/ui-macos-lib.sh"
if [ $# -lt 1 ] || [ $# -gt 2 ]; then
  echo "usage: $0 <app> [out directory]" >&2
  exit 2
fi
ui_init "$1" "${2:-ui-macos-dialogs}"
UI_TIMEOUT=${UI_DIALOGS_TIMEOUT:-180}
export MITCAD_DIALOG_SHOTS=$UI_OUT

for scheme in light dark; do
  for surface in search parameters-add parameters-editor parameters-error-selected parameters parameters-compact; do
    rm -f "$UI_OUT/$surface-$scheme.png"
  done
  MITCAD_TEST_COLOR_SCHEME=$scheme \
    MITCAD_TEST_INPUT=$here/ui-scripts/macos-dialogs.mitcad-ui \
    ui_run_app "$scheme" --ui=qt --demo --no-recovery --no-mcp
  ui_expect_no_log 'TestDriver: FAIL' "$scheme: all dialog assertions passed"
  ui_expect_log 'TestDriver: done' "$scheme: script completed"
  for surface in search parameters-add parameters-editor parameters-error-selected parameters parameters-compact; do
    if [ ! -s "$UI_OUT/$surface-$scheme.png" ]; then
      ui_fail "$scheme: missing focused $surface capture"
    fi
  done
  if [ "$UI_FAILURES" -gt 0 ]; then
    echo "FAIL: macOS Qt dialogs; logs and captures: $UI_OUT"
    exit 1
  fi
done
echo "PASS: macOS Qt dialogs; logs and captures: $UI_OUT"
