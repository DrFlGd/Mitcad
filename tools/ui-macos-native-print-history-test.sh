#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser/PropertySetsDialog app/platform/macos/NativeMacWindow app/platform/macos/NativeTestInput
# Native print-cell actions and the horizontal history's wheel input.
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
ui_init "$1" "$3"
cli=$2
UI_TIMEOUT=240
export MITCAD_TEST_WORK=$UI_OUT/files
export MITCAD_LIBRARIES_DIR=$UI_ROOT/libraries
mkdir -p "$MITCAD_TEST_WORK" "$MITCAD_LIBRARIES_DIR"
python3 - "$MITCAD_TEST_WORK/setup.json" <<'PY'
import json, sys
commands = [
    {'cmd': 'add_feature', 'def': {'type': 'box', 'plane': 'xy', 'corner': [0, 0],
        'length': 10, 'width': 10, 'height': 10, 'operation': 'new_body'}},
    {'cmd': 'create_print_profile', 'kind': 'process', 'name': 'Standard', 'values': {'perimeters': 3}},
    {'cmd': 'create_print_profile', 'kind': 'process', 'name': 'Alternate', 'values': {'perimeters': 5}},
    {'cmd': 'set_property', 'owner': 'C0', 'set': 'Print Process', 'key': 'profile', 'value': 'standard'},
    {'cmd': 'set_property', 'owner': 'F1.b0', 'set': 'Print Process', 'key': 'perimeters', 'value': 6},
]
# Empty sketches add real timeline features without extra meshing work.
commands += [{'cmd': 'sketch.create'} for _ in range(40)]
with open(sys.argv[1], 'w', encoding='utf-8') as target:
    json.dump(commands, target)
PY
"$cli" run "$MITCAD_TEST_WORK/setup.json" --save "$MITCAD_TEST_WORK/print-history.mitcad" > "$UI_OUT/setup.log" 2>&1 || {
  cat "$UI_OUT/setup.log" >&2
  exit 1
}
for scheme in light dark; do
  export MITCAD_SETTINGS_DIR=$UI_ROOT/settings-$scheme
  mkdir -p "$MITCAD_SETTINGS_DIR"
  MITCAD_TEST_COLOR_SCHEME=$scheme MITCAD_TEST_INPUT=$here/ui-scripts/native-macos-print-history.mitcad-ui \
    ui_run_app "$scheme" --ui=native --no-recovery --no-mcp --open "$MITCAD_TEST_WORK/print-history.mitcad"
  ui_expect_log 'TestDriver: done' "$scheme: native history wheel and print-cell pointer actions completed"
  ui_expect_log 'Native history wheel: mouse and precise trackpad, both endpoints' "$scheme: actual history scroll responders exercised"
  ui_expect_no_log 'TestDriver: FAIL' "$scheme: no failed native assertions"
  [ "$UI_FAILURES" = 0 ] || exit 1
done
echo 'PASS: native print-cell editors, pencils, reset and timeline wheel input'
