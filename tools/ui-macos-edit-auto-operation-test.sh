#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework/CommandSessionAuto app/commands/ExtrudeCommands.cpp core/model/src/document/auto_operation.rs
# Edits keep their stored operation (mitcad#868) through the Qt panel:
# a Join going into its bodies, deliberate choices, target scope, an
# input while the first preview computes.
# Usage: bash tools/ui-macos-edit-auto-operation-test.sh <app> [out directory]
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "usage: $0 <app> [out directory]" >&2
  exit 2
fi
ui_init "$1" "${2:-ui-macos-edit-auto-operation}"
UI_TIMEOUT=150
cli=${MITCAD_UI_CLI:-$(dirname "$UI_APP")/../../../../tools/cli/mitcad-cli}
export MITCAD_TEST_WORK=$UI_OUT/files
mkdir -p "$MITCAD_TEST_WORK"

for phase in automatic new-body participants overrides scope early; do
  # Variants keep identical geometry; only the stored Operation/Objects
  # differ, and each edit keeps them.
  python3 - "$here/ui-scripts/edit-auto-operation.json" "$MITCAD_TEST_WORK/setup.json" "$phase" <<'PY'
import json, sys
with open(sys.argv[1], encoding='utf-8') as source:
    script = json.load(source)
feature = next(c['def'] for c in script if c.get('cmd') == 'add_feature'
               and c['def']['type'] == 'extrude')
phase = sys.argv[3]
if phase in ('new-body', 'early'):
    feature['operation'] = 'new_body'
    del feature['participants']
elif phase == 'participants':
    feature['participants'] = ['F1.b0']
elif phase == 'scope':
    feature['operation'] = 'cut'
    feature['extent']['distance'] = -15
    del feature['participants']
    feature['cut_scope'].pop('initialize_on_cut')
expected = script[-1]['expect']
expected['instances'] = {'automatic': 2, 'new-body': 4, 'participants': 3,
                         'overrides': 2, 'scope': 3, 'early': 4}[phase]
expected['world_volume'] = {'automatic': 27000, 'overrides': 27000,
                            'scope': 34000}.get(phase, 39000)
with open(sys.argv[2], 'w', encoding='utf-8') as target:
    json.dump(script, target)
PY
  [ "$?" = 0 ] || exit 1
  "$cli" run "$MITCAD_TEST_WORK/setup.json" --save "$MITCAD_TEST_WORK/edit-auto.mitcad" \
    > "$UI_OUT/setup-$phase.log" 2>&1 || { cat "$UI_OUT/setup-$phase.log" >&2; exit 1; }
  case "$phase" in
    automatic) driver=edit-auto-operation; delay=0 ;;
    new-body|participants) driver=edit-auto-operation-retained; delay=0 ;;
    early) driver=edit-auto-operation-early; delay=0,preview=1500 ;;
    *) driver=edit-auto-operation-$phase; delay=0 ;;
  esac
  MITCAD_TEST_STEP_MS=20 MITCAD_TEST_RECOMPUTE_DELAY_MS=$delay \
    MITCAD_TEST_INPUT=$here/ui-scripts/$driver.mitcad-ui \
    ui_run_app "$phase" --ui=qt --no-recovery --no-mcp --open "$MITCAD_TEST_WORK/edit-auto.mitcad"
  ui_expect_log '^TestDriver: done' "$phase: completed every driver assertion"
  ui_expect_no_log 'TestDriver: FAIL' "$phase: no driver failure"
  ui_expect_no_log 'Native UI: AppKit shell with embedded OCCT viewport' "$phase: primary Qt frontend"
  # Ignore echoed driver expressions; expect-never checks runtime messages
  # in a separate recording that excludes TestDriver lines.
  for message in 'The document was read while a job computes it' 'OCCT view event failed' 'Recompute failed'; do
    if awk -v text="$message" 'index($0,"TestDriver:") != 1 && index($0,text) { found=1 }
        END { exit !found }' "$UI_LOG_FILE"; then
      ui_fail "$phase: $message"
    fi
  done
  [ "$UI_FAILURES" = 0 ] || exit 1
  if [ "$phase" = automatic ] || [ "$phase" = new-body ] || [ "$phase" = participants ] || [ "$phase" = scope ]; then
    python3 - "$MITCAD_TEST_WORK/edit-auto.mitcad" "$phase" <<'PY'
import json, sys
with open(sys.argv[1], encoding='utf-8') as source:
    feature = next(f for f in json.load(source)['features'] if f['uid'] == 'F4')
phase = sys.argv[2]
assert feature['operation'] == {'automatic': 'join', 'participants': 'join', 'scope': 'cut'}.get(phase, 'new_body'), feature
assert feature.get('participants', []) == {'automatic': ['F1.b0', 'F2.b0'], 'participants': ['F1.b0']}.get(phase, []), feature
assert not set(feature.get('participants', [])).intersection(('F4.b0', 'F5.b0')), feature
if phase == 'scope':
    targets = feature['cut_scope']['targets']
    assert targets['type'] == 'automatic' and not targets['excluded'], targets
    assert targets['excluded_bodies'] == [{'component': 'C0', 'occurrence': '', 'body': 'F2.b0'}], targets
PY
    [ "$?" = 0 ] || { ui_fail "$phase: persisted policy changed"; exit 1; }
  fi
done
echo "PASS: Qt edit automatic operation; logs in $UI_OUT"
