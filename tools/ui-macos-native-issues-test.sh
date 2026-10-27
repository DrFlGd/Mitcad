#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Real model warnings/failures, compact history and native Browser badges.
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ $# -ne 3 ]; then
  echo "usage: $0 <app> <mitcad-cli> <output>" >&2
  exit 2
fi
ui_init "$1" "$3"
UI_TIMEOUT=180
export MITCAD_TEST_WORK=$UI_OUT/files
mkdir -p "$MITCAD_TEST_WORK"
python3 - "$here/cli/tests/p9_warnings.json" "$MITCAD_TEST_WORK/model.json" <<'PY'
import json
import sys

commands = []
for entry in json.load(open(sys.argv[1])):
    if entry.get("expect", {}).get("status") == "warning":
        commands.append({"expect": {"feature": "Fillet1", "status": "warning"}})
        break
    if "cmd" in entry:
        commands.append(entry)
commands.extend([
    {"cmd": "add_feature", "name": "Missing edge", "def": {
        "type": "fillet", "body": "F2.b0", "edges": ["E{F2:side0|F2:side1}"], "radius": 100}},
    {"expect": {"feature": "Missing edge", "status": "error"}},
])
for distance in range(12):
    commands.append({"cmd": "add_feature", "def": {"type": "construction_plane",
        "definition": {"type": "offset", "plane": "xy", "distance": distance + 10}}})
with open(sys.argv[2], "w") as output:
    json.dump(commands, output)
PY
"$2" run "$MITCAD_TEST_WORK/model.json" --save "$MITCAD_TEST_WORK/issues.mitcad" > "$UI_OUT/setup.log" 2>&1 || {
  cat "$UI_OUT/setup.log" >&2
  exit 1
}
MITCAD_TEST_INPUT=$here/ui-scripts/native-macos-issues.mitcad-ui \
  ui_run_app issues --ui=native --no-recovery --open "$MITCAD_TEST_WORK/issues.mitcad"
ui_expect_log 'TestDriver: done' 'native issue workflow reached its end'
ui_expect_no_log 'TestDriver: FAIL|Native Mac canvas:|OCCT view event failed' 'native issues retain input and rendering'
if [ "$UI_FAILURES" -gt 0 ]; then exit 1; fi
echo "PASS: compact native history, issue colours, messages and Browser navigation"
