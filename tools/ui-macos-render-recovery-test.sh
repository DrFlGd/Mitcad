#!/bin/bash
# SPDX-License-Identifier: MIT
# Physical Metal recovery through each real frontend. All settings and
# worker kills belong only to this test's isolated application process.
# Usage: ui-macos-render-recovery-test.sh <app> <output> [qt|native]
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$here/ui-macos-lib.sh"
frontend=${3:-qt}
case "$frontend" in qt|native) ;; *) exit 2 ;; esac
ui_init "$1" "$2"
worker=${MITCAD_RENDER_WORKER:-$(dirname "$UI_APP")/mitcad-render}
"$worker" --list-devices > "$UI_OUT/devices.json" || exit 1
python3 - "$UI_OUT/devices.json" <<'PY'
import json, os, sys
devices = json.load(open(sys.argv[1]))['devices']
if not any(d['type'] == 'METAL' and d.get('denoiser_available') and 'Paravirtual' not in d['name']
           for d in devices):
    required = any(os.environ.get(key, '0') not in ('', '0')
                   for key in ('MITCAD_RENDER_REQUIRE_METAL', 'MITCAD_RENDER_REQUIRE_GPU'))
    print(('FAIL' if required else 'SKIP') + ': physical Metal/OIDN required')
    sys.exit(1 if required else 77)
PY
capability=$?
[ "$capability" = 0 ] || exit "$capability"
unset QT_QPA_PLATFORM QT_OPENGL LIBGL_ALWAYS_SOFTWARE GALLIUM_DRIVER
unset MITCAD_RENDER_REQUIRE_METAL MITCAD_RENDER_TEST_DEVICE_FAILURE
unset MITCAD_RENDER_TEST_DENOISER MITCAD_RENDER_TEST_DENOISER_FAILURE CYCLES_TEST_SINGLE_SAMPLE_UPDATES
unset MITCAD_RENDER_TEST_NATIVE_PASS_COMPARE MITCAD_RENDER_TEST_COMMAND_TRACE
mkdir -p "$MITCAD_SETTINGS_DIR/nocodo.fi"
for scenario in missing failure crash; do
  choice=auto
  [ "$scenario" != missing ] || choice=METAL_missing_acceptance
  printf '[render]\ndevice=%s\n' "$choice" > "$MITCAD_SETTINGS_DIR/nocodo.fi/Mitcad.ini"
  script=$UI_OUT/$scenario.script
  cat > "$script" <<'DRIVER'
expect-ever 'View area'
window-resize 900 650
command view.rendered
DRIVER
  case "$scenario" in
    missing)
      cat >> "$script" <<'DRIVER'
wait-log 'Render device fallback: There is no render device "METAL_missing_acceptance"; rendering on the CPU' 30
expect-log 'Render diagnostics: backend CPU,'
DRIVER
      ;;
    failure)
      cat >> "$script" <<'DRIVER'
wait-log 'Render device: .*METAL.*\(asked for auto\)' 30
wait-log 'Render device fallback: Rendering on the CPU: .*failed \(acceptance mid-render failure\)' 120
expect-log 'Render diagnostics after fallback: backend CPU,'
DRIVER
      ;;
    crash)
      cat >> "$script" <<'DRIVER'
wait-log 'Render device: .*METAL.*\(asked for auto\)' 30
wait-log 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples 8 ' 120
wait-log 'View message: The renderer stopped:' 30
expect-model-bodies 1 48000
expect-viewport
mark
command view.rendered
wait-log 'Render device: .*METAL.*\(asked for auto\)' 30
DRIVER
      ;;
  esac
  cat >> "$script" <<'DRIVER'
wait-log 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples 8 ' 120
expect-model-bodies 1 48000
expect-viewport
click-visible-face
expect-selection 1 0
screenshot-view '${MITCAD_RECOVERY_SHOTS}/recovered.png'
mark
command view.rendered
wait-log 'Render worker ended' 15
DRIVER
  watcher=""
  if [ "$scenario" = crash ]; then
    # Clear old evidence before the watcher starts. Verify the executable path
    # and UID as well as the fresh PID logged by this isolated application.
    rm -f "$UI_OUT/crash.log"
    crash_worker=$worker
    if [ -z "${MITCAD_RENDER_WORKER:-}" ]; then
      crash_worker=$(dirname "$UI_APP")/../XPCServices/org.mitcad.renderer.xpc/Contents/MacOS/mitcad-render-service
    fi
    python3 - "$UI_OUT/crash.log" "$crash_worker" > "$UI_OUT/crash-injection.log" 2>&1 <<'PY' &
import ctypes, os, pathlib, re, signal, subprocess, sys, time
path = pathlib.Path(sys.argv[1])
expected = pathlib.Path(sys.argv[2]).resolve(strict=True)
libproc = ctypes.CDLL('/usr/lib/libproc.dylib')
libproc.proc_pidpath.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
libproc.proc_pidpath.restype = ctypes.c_int
deadline = time.monotonic() + 150
while time.monotonic() < deadline:
    text = path.read_text(errors='replace') if path.exists() else ''
    started = re.search(r'^Render worker started \(pid (\d+)\):', text, re.M)
    frame = re.search(r'^Render frame view \d+ \d+x\d+ samples 8 ', text, re.M)
    if started and frame:
        pid = int(started[1])
        executable = ctypes.create_string_buffer(4096)
        if pid <= 1 or libproc.proc_pidpath(pid, executable, len(executable)) <= 0:
            sys.exit('cannot verify the owned renderer executable')
        actual = pathlib.Path(os.fsdecode(executable.value)).resolve(strict=True)
        uid = int(subprocess.check_output(['ps', '-p', str(pid), '-o', 'uid=']).strip())
        if actual != expected or uid != os.geteuid():
            sys.exit('renderer identity does not match the fresh owned test bundle')
        os.kill(pid, signal.SIGKILL)
        print(f'Injected worker crash into own PID {pid}', flush=True)
        break
    time.sleep(.1)
else:
    sys.exit('no own completed Metal worker to stop')
PY
    watcher=$!
  fi
  export MITCAD_TEST_INPUT=$script MITCAD_RECOVERY_SHOTS=$UI_OUT
  if [ "$scenario" = failure ]; then
    MITCAD_RENDER_TEST_DEVICE_FAILURE='acceptance mid-render failure' MITCAD_RENDER_SAMPLES=8 \
      UI_TIMEOUT=240 ui_run_app "$scenario" "--ui=$frontend" --no-recovery --demo
  else
    MITCAD_RENDER_SAMPLES=8 UI_TIMEOUT=240 ui_run_app "$scenario" "--ui=$frontend" --no-recovery --demo
  fi
  if [ -n "$watcher" ]; then
    wait "$watcher" || ui_fail 'own worker crash injection failed'
  fi
  ui_expect_log 'TestDriver: done' "$scenario: recovery script completed"
  ui_expect_no_log 'TestDriver: FAIL' "$scenario: all assertions passed"
  ui_expect_no_log 'Recompute failed' "$scenario: model stayed usable"
  if [ -z "${MITCAD_RENDER_WORKER:-}" ] || [ "${MITCAD_RENDER_REQUIRE_IOSURFACE:-0}" = 1 ]; then
    ui_expect_log 'Render transport: IOSurface, private XPC session' "$scenario: private native surface transport"
    if [ "$scenario" = crash ]; then
      if ! python3 - "$UI_LOG_FILE" <<'PROOF'
import pathlib, sys
text = pathlib.Path(sys.argv[1]).read_text()
for proof in ('Render transport: IOSurface, private XPC session',
              'Render frame transport: Metal buffer interop',
              'Render frame transport: native auxiliary GPU, composite '):
    assert text.count(proof) >= 2, 'native GPU transport did not restart: ' + proof
PROOF
      then ui_fail 'crash: native light and auxiliary transport did not restart'; fi
    else
      ui_expect_log 'Render frame transport: native auxiliary CPU, composite [01]' "$scenario: native CPU fallback output"
    fi
  fi
  [ "$UI_FAILURES" = 0 ] || exit 1
done
echo "PASS: $frontend missing device, actual Metal failure/CPU recovery and worker crash/restart"
