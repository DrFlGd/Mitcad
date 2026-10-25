#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/platform/MacDialogs.hpp app/files/LibraryDialogs app/library/PartPicker app/projects/ProjectBrowser app/render/RenderImageDialog app/browser/AppearanceDialog app/framework/ShortcutDialog app/materials/TexturePreferences app/framework/TestDriverDialogs app/framework/CommandSearch app/files/ProjectSettings app/print/MaterialsDialog app/print/SettingsForm
# Local synthetic library and actual Qt dialog controls, light/dark at 2x.
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "usage: $0 <app> [out directory]" >&2
  exit 2
fi
ui_init "$1" "${2:-ui-macos-dialog-gallery}"
UI_TIMEOUT=240
cli=${MITCAD_UI_CLI:-$(dirname "$UI_APP")/../../../../tools/cli/mitcad-cli}
export MITCAD_STYLE_LIBRARY=$UI_OUT/library
export MITCAD_LIBRARIES_DIR=$UI_OUT/library-cache
export MITCAD_STYLE_SHOTS=$UI_OUT
if [ ! -f "$MITCAD_STYLE_LIBRARY/mitcad-library.json" ]; then
  python3 "$here/libraries/make-fastener-library.py" --cli "$cli" --out "$MITCAD_STYLE_LIBRARY" \
    --sizes M5 --standards iso4762,iso4032 --version 1.0.0 --no-render --commit > "$UI_OUT/setup.log" 2>&1 || exit 1
fi
"$cli" library fetch "$MITCAD_STYLE_LIBRARY" --libraries "$MITCAD_LIBRARIES_DIR" --json > "$UI_OUT/fetch.log" 2>&1 || exit 1
script=$here/ui-scripts/macos-dialog-gallery.mitcad-ui
if [ "${MITCAD_DIALOG_BASELINE:-0}" = 1 ]; then
  script=$UI_OUT/baseline.mitcad-ui
  sed '/^test-dialog-controls /d' "$here/ui-scripts/macos-dialog-gallery.mitcad-ui" > "$script"
fi
for scheme in light dark; do
  QT_SCALE_FACTOR=2 MITCAD_UI_REQUIRE_RETINA=1 MITCAD_TEST_COLOR_SCHEME=$scheme \
    MITCAD_TEST_INPUT=$script ui_run_app "$scheme" --ui=qt --demo --no-recovery --no-mcp
  ui_expect_no_log 'TestDriver: FAIL' "$scheme: dialog controls passed"
  ui_expect_log 'TestDriver: done' "$scheme: gallery completed"
  for surface in libraries insert community parts picker publish projects render appearances shortcuts settings; do
    [ -s "$UI_OUT/$surface-$scheme.png" ] || ui_fail "$scheme: missing $surface capture"
  done
  [ "$UI_FAILURES" = 0 ] || exit 1
  if [ "${MITCAD_DIALOG_BASELINE:-0}" != 1 ]; then
    QT_SCALE_FACTOR=2 MITCAD_TEST_COLOR_SCHEME=$scheme \
      MITCAD_TEST_INPUT=$here/ui-scripts/macos-dialog-controls.mitcad-ui \
      ui_run_app "$scheme-controls" --ui=qt --demo --no-recovery --no-mcp
    ui_expect_no_log 'TestDriver: FAIL' "$scheme: Return/Escape and list interaction assertions passed"
    ui_expect_log 'TestDriver: done' "$scheme: list interaction script completed"
    [ "$UI_FAILURES" = 0 ] || exit 1
  fi
done
echo "PASS: Qt dialog gallery at requested 2x scale; actual dialog DPR is recorded in logs; $UI_OUT"
