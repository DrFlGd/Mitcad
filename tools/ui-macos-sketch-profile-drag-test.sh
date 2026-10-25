#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindow.cpp app/OcctViewer.cpp app/sketch app/framework/TestDriver
# Held geometry drags defer filled profiles; release/undo restore picking.
set -u
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ $# -lt 2 ] || [ $# -gt 3 ]; then
  echo "usage: $0 <app> <cli> [output directory]" >&2
  exit 2
fi
cli=$2
ui_init "$1" "${3:-ui-macos-sketch-profile-drag}"
export MITCAD_TEST_WORK=$UI_OUT/files
mkdir -p "$MITCAD_TEST_WORK"
"$cli" run "$here/cli/tests/perf_sketch_pattern_10.json" --save "$MITCAD_TEST_WORK/pattern.mitcad" \
  > "$UI_OUT/setup.log" 2>&1 || { cat "$UI_OUT/setup.log" >&2; exit 1; }
UI_TIMEOUT=240
MITCAD_TEST_DRAG_STEPS=30 MITCAD_TEST_STEP_MS=20 MITCAD_TEST_SETTLE_MS=50 \
MITCAD_LOG_TIMING=1 MITCAD_TEST_INPUT=$here/ui-scripts/sketch-profile-drag.mitcad-ui \
  ui_run_app profiles --ui=qt --no-native-dialogs --no-recovery --no-mcp --open "$MITCAD_TEST_WORK/pattern.mitcad"
ui_expect_log 'TestDriver: done' 'held centre/radius drags, final picking and grouped undo'
ui_expect_no_log 'TestDriver: FAIL line' 'all profile drag assertions passed'
ui_expect_no_log 'OCCT view event failed:' 'no viewport event failures'
# Each completed drag step ends after its refresh. Its profiles section
# must neither build/display faces nor exceed the issue's 10 ms budget.
python3 - "$UI_OUT/profiles.log" <<'PY'
import re, sys
from pathlib import Path
steps = []
profile_ms = None
displayed = False
last_refresh_displayed = False
for line in Path(sys.argv[1]).read_text().splitlines():
    match = re.match(r'Timing refresh profiles: ([0-9.]+) ms', line)
    if match:
        profile_ms = float(match[1])
    if line.startswith('Timing display profiles:'):
        displayed = True
    if line.startswith('Timing sketch drag step:'):
        assert profile_ms is not None and profile_ms < 10, ('profiles budget', profile_ms)
        assert not last_refresh_displayed, 'drag step rebuilt profile presentations'
        steps.append(profile_ms)
    if line.startswith('Timing refresh:'):
        last_refresh_displayed = displayed
        displayed = False
    if line.startswith('Timing defer sketch profiles:'):
        displayed = False
assert len(steps) >= 4, ('missing drag steps', steps)
print(f'PASS: {len(steps)} drag steps, profiles max {max(steps):.3f} ms, no profile redisplay')
text = Path(sys.argv[1]).read_text()
held = text.split('Held drag: begin')[-1].split('Held drag: end,')[0]
end = re.search(r'Held drag: end, delivered= \d+ elapsed_ms= (\d+)', text)
count = len(re.findall(r'^Timing sketch drag step:', held, re.M))
assert end and count > 0, 'continuous drag did not finish'
print(f'Times: continuous 10 x 10 drag, {count} applied steps in {end[1]} ms, '
      f'{count * 1000 / int(end[1]):.2f} steps/s including final profile rebuild')
PY
[ $? = 0 ] || exit 1
cat > "$MITCAD_TEST_WORK/fixed.json" <<'JSON'
[
  {"cmd":"sketch.create"},
  {"cmd":"sketch.circle","sketch":"F1","mode":"center","center":[5,2.5],"radius":4.5},
  {"cmd":"sketch.set_fixed","sketch":"F1","entities":["c1"],"fixed":true}
]
JSON
"$cli" run "$MITCAD_TEST_WORK/fixed.json" --save "$MITCAD_TEST_WORK/fixed.mitcad" \
  > "$UI_OUT/fixed-setup.log" 2>&1 || { cat "$UI_OUT/fixed-setup.log" >&2; exit 1; }
MITCAD_TEST_INPUT=$here/ui-scripts/sketch-profile-drag-fixed.mitcad-ui \
  ui_run_app fixed --ui=qt --no-recovery --no-mcp --open "$MITCAD_TEST_WORK/fixed.mitcad"
ui_expect_log 'TestDriver: done' 'fixed-radius drag restores the original profile'
ui_expect_no_log 'TestDriver: FAIL line' 'fixed-profile assertions passed'
ui_expect_no_log 'OCCT view event failed:' 'no fixed-profile viewport event failures'
[ "$UI_FAILURES" = 0 ] || exit 1
echo "PASS: sketch profile drag; evidence in $UI_OUT"
