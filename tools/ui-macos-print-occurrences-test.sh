#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/files/MainWindowPrint app/files/Print3d app/browser/BrowserPanel app/framework/CommandSession app/platform/macos/NativeForms app/platform/macos/NativeMacWindow
# check-all sources: core/3mf core/model/src/exchange core/model-api/src/exchange tools/cli
# 3MF occurrence and bed face UI parity (mitcad#856, #857), with local boxes:
# one picked occurrence, two separate print-bed objects, then a selected
# component sent as assembled by its button or put on a picked green face.
# Escape ends only the question and sends nothing. Send starts a recording
# executable that copies the actual 3MF; it opens no external GUI or network.
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ "$#" -ne 4 ]; then
  echo "usage: $0 <app> <mitcad-cli> <output directory> qt|native" >&2
  exit 2
fi
cli=$2
frontend=$4
case "$frontend" in qt|native) ;; *) exit 2 ;; esac
ui_init "$1" "$3"
UI_TIMEOUT=180
# QDir::tempPath and the exported files belong to this evidence directory.
# HOME, global GUI locking and the user's installed slicers stay untouched.
export TMPDIR=$UI_OUT/tmp
mkdir -p "$TMPDIR"
for scheme in light dark; do
  rm -rf "$MITCAD_SETTINGS_DIR"
  # QSettings uses the macOS organization domain even with IniFormat.
  mkdir -p "$MITCAD_SETTINGS_DIR/nocodo.fi"
  export MITCAD_TEST_WORK=$UI_OUT/files/$frontend-$scheme
  # A repeat must wait for this run's detached launches, never old receipts.
  rm -rf "$MITCAD_TEST_WORK"
  mkdir -p "$MITCAD_TEST_WORK/receipts"
  cp "$here/ui-scripts/print-occurrences-record-slicer.py" "$MITCAD_TEST_WORK/record-slicer.py"
  chmod +x "$MITCAD_TEST_WORK/record-slicer.py"
  printf '[print]\nslicerName=Fixture slicer\nslicerProgram=%s\nformat=3mf\nrefinement=high\n' \
    "$MITCAD_TEST_WORK/record-slicer.py" > "$MITCAD_SETTINGS_DIR/nocodo.fi/Mitcad.ini"
  for case_name in plain bed; do
    if ! "$cli" run "$here/ui-scripts/print-occurrences-$case_name.json" \
        --save "$MITCAD_TEST_WORK/$case_name.mitcad" > "$UI_OUT/setup-$scheme-$case_name.log" 2>&1; then
      cat "$UI_OUT/setup-$scheme-$case_name.log" >&2
      exit 1
    fi
    cp "$MITCAD_TEST_WORK/$case_name.mitcad" "$MITCAD_TEST_WORK/$case_name-before.mitcad"
  done
  script=$UI_OUT/print-occurrences-$frontend-$scheme.mitcad-ui
  python3 "$here/ui-scripts/print-occurrences-parity-script.py" "$frontend" > "$script"
  MITCAD_TEST_COLOR_SCHEME=$scheme MITCAD_TEST_STEP_MS=90 MITCAD_TEST_INPUT=$script \
    ui_run_app "print-occurrences-$frontend-$scheme" --ui="$frontend" --no-recovery --no-mcp \
      --open "$MITCAD_TEST_WORK/plain.mitcad"
  ui_expect_log 'TestDriver: done' "$frontend/$scheme: four sends and the zero-send Escape/cancel completed"
  for problem in 'TestDriver: FAIL' 'Unable to simultaneously satisfy constraints' 'Native Mac canvas:' 'OCCT view event failed:'; do
    ui_expect_no_log "$problem" "$frontend/$scheme: no $problem"
  done
  [ "$UI_FAILURES" = 0 ] || { ui_log_tail; exit 1; }
  python3 "$here/ui-scripts/print-occurrences-verify.py" "$MITCAD_TEST_WORK"
  for case_number in 1 2 3 4; do
    [ -s "$MITCAD_TEST_WORK/print-occurrences-$case_number-$frontend-$scheme.png" ] || {
      echo "FAIL: missing $frontend/$scheme case $case_number capture" >&2
      exit 1
    }
  done
  for case_number in 3 escape 4; do
    [ -s "$MITCAD_TEST_WORK/print-bed-choice-$case_number-$frontend-$scheme.png" ] || {
      echo "FAIL: missing $frontend/$scheme bed face choice $case_number capture" >&2
      exit 1
    }
  done
  [ -s "$MITCAD_TEST_WORK/print-bed-escape-$frontend-$scheme.png" ] || {
    echo "FAIL: missing $frontend/$scheme panel still open after Escape capture" >&2
    exit 1
  }
done
echo "PASS: 3MF occurrence UI parity ($frontend, light and dark)"
