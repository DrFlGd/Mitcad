#!/bin/bash
# SPDX-License-Identifier: MIT
# check-all sources: app/render app/view app/framework/TestDriver
# Metal/OIDN acceptance through the existing in-process driver. The
# helper isolates settings and touches only the test application's window.
# Usage: ui-macos-render-metal-test.sh <app> [output directory] [qt|native]
# Missing Metal OIDN may skip; either required-GPU switch makes it fail.
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
if [ $# -lt 1 ] || [ $# -gt 3 ]; then
  echo "usage: $0 <app> [output directory] [qt|native]" >&2
  exit 2
fi
frontend=${3:-qt}
case "$frontend" in
  qt|native) ;;
  *) echo "invalid frontend: $frontend (qt or native)" >&2; exit 2 ;;
esac
ui_init "$1" "${2:-ui-macos-render-metal}"
# A failed run must not reuse an earlier run's screenshots or statistics.
for shot in rendered orbited zoomed resized; do
  rm -f "$UI_OUT/$shot.png" "$UI_OUT/$shot.stats.json"
done
worker=${MITCAD_RENDER_WORKER:-$(dirname "$UI_APP")/mitcad-render}
if ! "$worker" --list-devices > "$UI_OUT/devices.json" 2> "$UI_OUT/devices.err"; then
  cat "$UI_OUT/devices.err" >&2
  echo 'FAIL: cannot query the render worker' >&2
  exit 1
fi
python3 - "$UI_OUT/devices.json" "${MITCAD_RENDER_REQUIRE_METAL:-0}" "${MITCAD_RENDER_REQUIRE_GPU:-0}" <<'PY'
import json, sys
with open(sys.argv[1]) as stream:
    devices = json.load(stream)["devices"]
metal = [device for device in devices if device["type"] == "METAL"]
reason = None
if not metal:
    reason = "no Metal render device"
elif not metal[0].get("denoiser_available", False):
    reason = "the Metal device has no OIDN support"
elif "Paravirtual" in metal[0]["name"] and sys.argv[2] not in ("", "0"):
    reason = "required physical-GPU acceptance cannot use a paravirtual device"
if reason:
    required = any(value not in ("", "0") for value in sys.argv[2:])
    print(("FAIL: " if required else "SKIP: ") + reason)
    sys.exit(1 if required else 77)
print("Metal OIDN capability: " + metal[0]["name"] + "; actual execution is checked below")
PY
capability=$?
[ "$capability" = 0 ] || exit "$capability"

# The hardware path must use the window server and GPU, with no software
# or headless override inherited from another test.
unset QT_QPA_PLATFORM QT_OPENGL LIBGL_ALWAYS_SOFTWARE GALLIUM_DRIVER
unset MITCAD_RENDER_TEST_DENOISER MITCAD_RENDER_TEST_COMPARE_DENOISER
unset MITCAD_RENDER_TEST_DENOISER_FAILURE MITCAD_RENDER_TEST_DEVICE_FAILURE
unset CYCLES_TEST_SINGLE_SAMPLE_UPDATES
mkdir -p "$MITCAD_SETTINGS_DIR/nocodo.fi"
printf '[render]\ndevice=metal\n' > "$MITCAD_SETTINGS_DIR/nocodo.fi/Mitcad.ini"
script="$UI_OUT/render.script"
cat > "$script" <<'DRIVER'
expect-ever 'View area'
window-resize 1280 800
command view.rendered
expect-log 'Rendered view on'
wait-log 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples 16 ' 150
screenshot-view '${MITCAD_METAL_SHOTS}/rendered.png'
mark
view-pointer press 40 50 middle shift
view-pointer move 45 48 middle shift
view-pointer move 50 46 middle shift
view-pointer move 55 45 middle shift
sleep 700
expect-no-log 'Render view [0-9]+: (orthographic|perspective) '
view-pointer release 55 45 middle shift
wait-log 'Render view [0-9]+: first frame' 120
wait-log 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples 16 ' 120
screenshot-view '${MITCAD_METAL_SHOTS}/orbited.png'
mark
capture-view-height before_zoom
wheel-burst 50 50 -1 1
wait-log 'Render view [0-9]+: first frame' 120
wait-log 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples 16 ' 120
expect-view-zoom ${before_zoom} out
screenshot-view '${MITCAD_METAL_SHOTS}/zoomed.png'
DRIVER
shots='rendered orbited zoomed resized'
cat >> "$script" <<'DRIVER'
mark
window-resize 1440 900
wait-log 'Render view [0-9]+: first frame' 120
wait-log 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples 16 ' 120
screenshot-view '${MITCAD_METAL_SHOTS}/resized.png'
DRIVER
cat >> "$script" <<'DRIVER'
command view.rendered
expect-log 'Rendered view off'
wait-log 'Render worker ended' 15
DRIVER
MITCAD_TEST_INPUT="$script" MITCAD_METAL_SHOTS="$UI_OUT" \
  MITCAD_RENDER_SAMPLES=16 MITCAD_RENDER_PREVIEWS=4,16 UI_TIMEOUT=420 \
  ui_run_app metal "--ui=$frontend" --no-recovery --demo
ui_expect_log 'TestDriver: done' 'GPU navigation script completed'
if [ "$frontend" = native ]; then
  ui_expect_log 'Native UI: AppKit shell with embedded OCCT viewport' 'AppKit frontend was used'
else
  ui_expect_no_log 'Native UI: AppKit shell with embedded OCCT viewport' 'Qt frontend was used'
fi
renderer=$(ui_log_value 'OpenGL renderer: *\(.*\)')
if [ -z "$renderer" ] || printf '%s' "$renderer" | grep -Eiq 'software|llvmpipe|swiftshader'; then
  ui_fail "no hardware OpenGL renderer: $renderer"
elif [ -n "${MITCAD_UI_RENDERER:-}" ] && ! printf '%s' "$renderer" | grep -Eq -- "$MITCAD_UI_RENDERER"; then
  ui_fail "OpenGL renderer is '$renderer', expected '$MITCAD_UI_RENDERER'"
else
  ui_pass "OpenGL renderer: $renderer"
fi
ui_expect_log 'Render device: .*METAL.*\(asked for metal\)' 'explicit Metal selected'
ui_expect_log 'Render denoising diagnostics: backend METAL,.*OIDN METAL.*\(execution confirmed\)' 'Metal OIDN executed'
ui_expect_no_log 'Rendering on the CPU:|Denoising on the CPU:|Render device: .*CPU|OIDN CPU.*\(execution confirmed\)|Render worker error|Render failed|Recompute failed|ERROR:' \
  'no fallback, worker error or shutdown pipeline error'
ui_expect_log 'Render worker ended' 'leaving rendered mode ended the worker'
if ! python3 - "$UI_LOG_FILE" <<'PY'
import re, sys
text = open(sys.argv[1]).read()
states = re.findall(r'backend METAL, MetalRT (on|off) \(available (yes|no), default (on|off)\); OIDN METAL[^\n]*\(execution confirmed\)', text)
assert states, "no completed Metal execution diagnostics"
for actual, available, default in states:
    assert (actual == "on") == (available == "yes" and default == "on"), "actual MetalRT violates automatic policy"
PY
then
  ui_fail 'actual MetalRT state did not match the automatic policy'
fi
for shot in $shots; do
  if [ ! -s "$UI_OUT/$shot.png" ]; then
    ui_fail "$shot: no rendered image"
  elif ! python3 "$UI_STATS" --shot "$UI_OUT/$shot.png" > "$UI_OUT/$shot.stats.json"; then
    ui_fail "$shot: image analysis failed"
  elif ! python3 - "$UI_OUT/$shot.stats.json" <<'PY'
import json, sys
with open(sys.argv[1]) as stream:
    stats = json.load(stream)
print(json.dumps({key: stats[key] for key in ("width", "height", "colors", "face_percent")}))
assert stats["width"] >= 300 and stats["height"] >= 200
assert stats["colors"] >= 8 and stats["face_percent"] >= 10
PY
  then
    ui_fail "$shot: image is too small, blank or missing the body"
  else
    ui_pass "$shot: rendered image contains the body"
  fi
done
if ! python3 - "$UI_OUT/zoomed.stats.json" "$UI_OUT/resized.stats.json" <<'PY'
import json, sys
with open(sys.argv[1]) as stream:
    before = json.load(stream)
with open(sys.argv[2]) as stream:
    after = json.load(stream)
assert (before["width"], before["height"]) != (after["width"], after["height"]), "window resize did not change the rendered framebuffer"
PY
then
  ui_fail 'window resize did not produce a resized rendered image'
else
  ui_pass 'window resize produced a new framebuffer size'
fi
echo "Metal UI logs and screenshots: $UI_OUT"
[ "$UI_FAILURES" = 0 ] || exit 1
echo "PASS: $frontend Metal/OIDN preview, orbit, zoom, resize and shutdown"
