#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/print/SettingField app/print/MaterialsDialog app/platform/NativeDialogs app/platform/macos/NativeDialogs app/platform/macos/NativeColorField app/framework/TestDriver app/platform/NativeTestInput
# Actual Materials colour fields: explicit colour, inherited appearance and
# Slicer default; native pixels/placeholder, pointer open and Cancel, same
# model and undo depth, both appearances. Qt keeps its custom field painting.
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ $# -ne 4 ] || { [ "$3" != qt ] && [ "$3" != native ]; }; then
  echo "usage: $0 <app> <cli> qt|native <new-output>" >&2
  exit 2
fi
cli=$2
frontend=$3
ui_init "$1" "$4"
UI_TIMEOUT=180
export MITCAD_TEST_WORK=$UI_OUT/files
export MITCAD_LIBRARIES_DIR=$UI_ROOT/libraries
mkdir -p "$MITCAD_TEST_WORK" "$MITCAD_LIBRARIES_DIR"
"$cli" run "$here/ui-scripts/material-color-parity.json" --save "$MITCAD_TEST_WORK/colors.mitcad" > "$UI_OUT/setup.log" 2>&1 || {
  cat "$UI_OUT/setup.log" >&2
  exit 1
}
python3 "$here/ui-scripts/material-color-parity-script.py" "$frontend" > "$UI_OUT/colors.mitcad-ui" || exit 1
for scheme in light dark; do
  export MITCAD_SETTINGS_DIR=$UI_ROOT/settings-$scheme
  mkdir -p "$MITCAD_SETTINGS_DIR"
  # Qt's supported test option keeps its QColorDialog controllable through
  # the actual Qt buttons. Native uses the production AppKit colour panel.
  if [ "$frontend" = qt ]; then
    MITCAD_TEST_COLOR_SCHEME=$scheme MITCAD_TEST_INPUT=$UI_OUT/colors.mitcad-ui \
      ui_run_app "$scheme" --ui=qt --no-native-dialogs --no-recovery --no-mcp --open "$MITCAD_TEST_WORK/colors.mitcad"
  else
    MITCAD_TEST_COLOR_SCHEME=$scheme MITCAD_TEST_INPUT=$UI_OUT/colors.mitcad-ui \
      ui_run_app "$scheme" --ui=native --no-recovery --no-mcp --open "$MITCAD_TEST_WORK/colors.mitcad"
  fi
  ui_expect_log 'TestDriver: done' "$frontend/$scheme: explicit, inherited and unset colour workflows completed"
  for problem in 'TestDriver: FAIL' 'Unable to simultaneously satisfy constraints' 'Native Mac canvas:' 'OCCT view event failed:'; do
    ui_expect_no_log "$problem" "$frontend/$scheme: no $problem"
  done
  [ "$UI_FAILURES" = 0 ] || { ui_log_tail; exit 1; }
done
echo "PASS: $frontend Materials colour fields and cancelled colour selection in light/dark; $UI_OUT"
