#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/browser/Bindings app/browser/BindingList app/browser/Browser app/browser/Timeline app/platform/macos app/MainWindowNative
# Synthetic binding-card identities/actions in both macOS frontends/themes.
# AppKit hover uses actual WindowServer movement and existing event access.
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ "$#" -ne 4 ]; then
  echo "usage: $0 <app> <mitcad-cli> <output> qt|native" >&2
  exit 2
fi
cli=$2
frontend=$4
case "$frontend" in qt|native) ;; *) exit 2 ;; esac
ui_init "$1" "$3"
UI_TIMEOUT=180
for scheme in light dark; do
  rm -rf "$MITCAD_SETTINGS_DIR"
  mkdir -p "$MITCAD_SETTINGS_DIR"
  export MITCAD_TEST_WORK=$UI_OUT/files/$frontend-$scheme
  mkdir -p "$MITCAD_TEST_WORK"
  file=$MITCAD_TEST_WORK/cards.mitcad
  "$cli" run "$here/ui-scripts/binding-card-parity.json" --save "$file" > "$UI_OUT/setup-$scheme.log" 2>&1 || {
    cat "$UI_OUT/setup-$scheme.log" >&2
    exit 1
  }
  cp "$file" "$MITCAD_TEST_WORK/cards-before.mitcad"
  script=$UI_OUT/binding-cards-$frontend-$scheme.mitcad-ui
  python3 "$here/ui-scripts/binding-card-parity-script.py" "$frontend" > "$script"
  MITCAD_TEST_COLOR_SCHEME=$scheme MITCAD_TEST_STEP_MS=90 MITCAD_TEST_INPUT=$script \
    ui_run_app "binding-cards-$frontend-$scheme" --ui="$frontend" --no-recovery --no-mcp --open "$file"
  ui_expect_log '^TestDriver: done' "$frontend/$scheme: binding cards completed"
  for problem in 'TestDriver: FAIL' 'Unable to simultaneously satisfy constraints' 'Native Mac canvas:' 'OCCT view event failed'; do
    ui_expect_no_log "$problem" "$frontend/$scheme: no $problem"
  done
  [ "$UI_FAILURES" = 0 ] || { ui_log_tail; exit 1; }
  python3 - "$MITCAD_TEST_WORK" <<'PY'
import json
from pathlib import Path
import sys
work = Path(sys.argv[1])
before = json.loads((work / "cards-before.mitcad").read_text())
after = json.loads((work / "cards.mitcad").read_text())
assert before == after, "reading/editing then cancelling the cards changed the saved model"
print("Verified: binding card reads and cancelled editors preserve the complete model")
PY
done
echo "PASS: binding card UI parity ($frontend, light and dark)"
