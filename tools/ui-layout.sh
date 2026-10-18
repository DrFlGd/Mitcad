#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The main window's layout as the built application lays it out, for
# tools/check-all.sh (mitcad#412). Starts Mitcad with the demo design on a
# hidden Xvfb display (ui-test-lib.sh) and prints its "Window layout: ..."
# line: the window's size and minimum size, and where the menu bar, the
# toolbars, the docks, the status bar and the 3D view are. Every UI test
# clicks and takes screenshots in that window, so check-all reruns every
# UI test when this line changes, whatever areas the tests declare: a row
# added to a panel that raised the window's minimum height once made it
# taller and broke a test that declared none of the files changed.
#
# Not a test (check-all runs tools/ui-*-test.sh only). Exit status 1 when
# the application did not log its layout.
# Usage: tools/ui-layout.sh

source "$(dirname "$0")/ui-test-lib.sh"

ui_start_display
UI_GDB=0 ui_start_app --demo >&2
LAYOUT=$(grep -a '^Window layout: ' "$UI_LOG" | tail -1)
ui_stop_app
if [ -z "$LAYOUT" ]; then
  echo "the application logged no window layout" >&2
  exit 1
fi
echo "$LAYOUT"
