#!/bin/bash
# SPDX-License-Identifier: MIT
# Narrow, opt-in physical-Mac measurement, not an additional CTest run.
# Usage: ui-macos-render-transport-measure.sh <app> <output directory> <cpu|metal>
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ "$#" != 3 ]; then
  echo "usage: $0 <app> <output directory> <cpu|metal>" >&2
  exit 2
fi
device=$3
case "$device" in cpu|metal) ;; *) echo 'device must be cpu or metal' >&2; exit 2 ;; esac
ui_init "$1" "$2"
unset QT_QPA_PLATFORM QT_OPENGL LIBGL_ALWAYS_SOFTWARE GALLIUM_DRIVER
unset MITCAD_RENDER_TEST_DENOISER MITCAD_RENDER_TEST_COMPARE_DENOISER
unset MITCAD_RENDER_TEST_DENOISER_FAILURE MITCAD_RENDER_TEST_DEVICE_FAILURE
unset MITCAD_RENDER_REQUIRE_METAL MITCAD_RENDER_REQUIRE_GPU CYCLES_TEST_SINGLE_SAMPLE_UPDATES
mkdir -p "$MITCAD_SETTINGS_DIR/nocodo.fi"
printf '[render]\ndevice=%s\n' "$device" > "$MITCAD_SETTINGS_DIR/nocodo.fi/Mitcad.ini"
script="$UI_OUT/transport.script"
rm -f "$UI_OUT/rendered.png"
cat > "$script" <<'DRIVER'
expect-ever 'View area'
window-resize 1440 1020
command view.rendered
wait-log 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples 16 denoised covers ' 240
screenshot-view '${MITCAD_TRANSPORT_SHOTS}/rendered.png'
command view.rendered
wait-log 'Render worker ended' 15
DRIVER
MITCAD_TEST_INPUT="$script" MITCAD_TRANSPORT_SHOTS="$UI_OUT" \
  MITCAD_RENDER_TEST_SCENE=1 MITCAD_RENDER_SAMPLES=16 MITCAD_RENDER_PREVIEWS=4,16 \
  MITCAD_LOG_TIMING=1 UI_TIMEOUT=300 \
  ui_run_app transport --ui=qt --no-recovery --demo
ui_expect_log 'TestDriver: done' 'synthetic transport driver completed'
for failure in 'Rendering on the CPU:' 'Render worker error' 'Render failed' 'ERROR:'; do
  ui_expect_no_log "$failure" "no $failure"
done
if [ "$device" = metal ]; then
  ui_expect_log 'Render device: .*METAL.*\(asked for metal\)' 'actual Metal device selected'
else
  ui_expect_log 'Render device: .*\(asked for cpu\)' 'CPU device requested'
  ui_expect_log 'Render denoising diagnostics: backend CPU,.*\(execution confirmed\)' 'actual CPU execution'
fi
ui_expect_log 'OpenGL renderer:.*Apple' 'hardware OpenGL renderer'
if ! python3 - "$UI_LOG_FILE" "$UI_OUT/rendered.png" "$UI_STATS" <<'PY'
import json, re, subprocess, sys
text = open(sys.argv[1]).read()
stats = json.loads(subprocess.check_output([sys.executable, sys.argv[3], "--shot", sys.argv[2]]))
assert stats["width"] >= 2560 and stats["height"] >= 1600, "Retina viewport is too small"
assert stats["colors"] >= 8 and stats["face_percent"] >= 10, "rendered body is missing"
for stage in ("allocate", "copy", "convert", "texture prepare", "upload submit", "diagnostics", "delivery"):
    assert re.search(r"Timing render " + stage + r": [\d.]+ ms, view \d+ samples 16 "
                     + str(stats["width"]) + "x" + str(stats["height"]) + r" denoised 1", text), stage
print("Retina synthetic scene: %s x %s pixels" % (stats["width"], stats["height"]))
PY
then
  ui_fail 'missing Retina image or final-frame timing stages'
fi
echo "Transport timing log: $UI_LOG_FILE"
[ "$UI_FAILURES" = 0 ] || exit 1
echo "PASS: $device synthetic Retina frame measurement"
