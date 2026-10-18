#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindowMcp.cpp app/MainWindowMcpUi.cpp app/mcp core/mcp core/ffi/src/mcp.rs
# check-all sources: app/OcctViewer.cpp app/view/ViewReveal.cpp app/view/ViewController.cpp tools/cli
# Live MCP's files and camera through the real UI on Xvfb and the actual
# bridge, `mitcad-cli mcp --connect`, driven from the shell (jq):
#   - mitcad#402: an untitled design is saved with document_save's path
#     into the folder the run allows (--mcp-folder); a path outside it, a
#     relative '..' and an existing file without overwrite are refused;
#     document_open opens another design from the folder in place of the
#     open one (a new handle; the old one refused), and refuses while there
#     are unsaved changes unless discard is true;
#   - mitcad#405: ui_camera reads the camera, sets eye, target, up and
#     height (the reply says where it is), frames a box: two equal blocks
#     500 mm apart framed one after the other give the same screenshot,
#     unlike the whole design; ui_select's reveal "fit" moves the view
#     before the next screenshot; view.fit frames only the isolated body;
#     a Home named view's target and height hold after view.home.
# The Python flow tools/ui-mcp-live.py (tools/ui-mcp-test.sh) covers the
# other live tools.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-mcp-files-camera-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

for tool in jq compare base64; do
  command -v "$tool" > /dev/null || { echo "$tool is required"; exit 2; }
done
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-mcp-files.XXXXXX)
BRIDGE=""
trap '[ -n "$BRIDGE" ] && kill "$BRIDGE" 2> /dev/null; ui_cleanup; rm -rf "$WORK"' EXIT
ALLOWED=$WORK/allowed
OUTSIDE=$WORK/outside
mkdir -p "$ALLOWED/sub" "$OUTSIDE"
# The listener's record in a private folder of the test's own.
export MITCAD_MCP_DISCOVERY_DIR=$WORK/discovery

# Two blocks 20 mm on each side, the second 500 mm along x from the first.
cat > "$WORK/blocks.json" << 'EOF'
[{"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"},
  "corner": [0, 0], "length": 20, "width": 20, "height": 20, "operation": "new_body"}},
 {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"},
  "corner": [500, 0], "length": 20, "width": 20, "height": 20, "operation": "new_body"}},
 {"cmd": "set_origin_visible", "visible": false}]
EOF
"$CLI" run "$WORK/blocks.json" --save "$ALLOWED/sub/blocks.mitcad" > "$WORK/cli.log" 2>&1 ||
  ui_fail "mitcad-cli did not make the design: $(cat "$WORK/cli.log")"

# ---------------------------------------------------------------------------
# The bridge: JSON-RPC lines through two FIFOs. mcp_request leaves the answer
# in $WORK/answer.json, mcp_tool the tool's structured result in
# $WORK/result.json; expect checks a jq condition on it.

MCP_ID=0
bridge_open() {
  mkfifo "$WORK/to-bridge" "$WORK/from-bridge"
  "$CLI" mcp --connect < "$WORK/to-bridge" > "$WORK/from-bridge" 2> "$WORK/bridge.err" &
  BRIDGE=$!
  exec 7> "$WORK/to-bridge"
  exec 8< "$WORK/from-bridge"
}

bridge_close() {
  exec 7>&-
  wait "$BRIDGE" || ui_fail "the bridge ended with an error: $(cat "$WORK/bridge.err")"
  BRIDGE=""
  exec 8<&-
}

mcp_send() { printf '%s\n' "$1" >&7; }

mcp_request() {
  local line params=${2:-'{}'}
  MCP_ID=$((MCP_ID + 1))
  mcp_send "$(jq -cn --argjson id "$MCP_ID" --arg method "$1" --argjson params "$params" \
    '{jsonrpc: "2.0", id: $id, method: $method, params: $params}')"
  while :; do
    IFS= read -r -t 120 -u 8 line || ui_fail "no answer to $1 from the bridge: $(cat "$WORK/bridge.err")"
    [ -z "$line" ] && continue
    printf '%s\n' "$line" > "$WORK/answer.json"
    # A notification (a method, no id) is not the answer.
    jq -e 'has("id") or (has("method") | not)' "$WORK/answer.json" > /dev/null || continue
    jq -e --argjson id "$MCP_ID" '.id == $id' "$WORK/answer.json" > /dev/null ||
      ui_fail "the answer has another request's id: $(head -c 2000 "$WORK/answer.json")"
    return 0
  done
}

# mcp_tool name arguments [true]: true when the tool must refuse (isError).
mcp_tool() {
  mcp_request tools/call "$(jq -cn --arg name "$1" --argjson arguments "$2" '{name: $name, arguments: $arguments}')"
  jq -e 'has("error") | not' "$WORK/answer.json" > /dev/null ||
    ui_fail "$1: protocol error: $(head -c 2000 "$WORK/answer.json")"
  [ "$(jq -r '.result.isError // false' "$WORK/answer.json")" = "${3:-false}" ] ||
    ui_fail "$1 $2: unexpected result: $(head -c 2000 "$WORK/answer.json")"
  jq -c '.result.structuredContent' "$WORK/answer.json" > "$WORK/result.json"
}

# expect 'jq condition' description [--arg name value ...]: on the last result.
expect() {
  local condition=$1 description=$2
  shift 2
  jq -e "$@" "$condition" "$WORK/result.json" > /dev/null ||
    ui_fail "$description: $(head -c 2000 "$WORK/result.json")"
  echo "ok   $description"
}

# A number within a tolerance: near(value; expected; tolerance).
NEAR='def near(v; e; t): ((v - e) | if . < 0 then -. else . end) <= t;'

# screenshot file: the 3D view as a PNG.
screenshot() {
  mcp_tool ui_screenshot '{"max_size": 640}'
  jq -r '.png_base64' "$WORK/result.json" | base64 -d > "$1"
  [ -s "$1" ] || ui_fail "no screenshot in $1"
}

# differing a b: how many pixels of two screenshots differ by more than 10 %.
differing() { compare -metric AE -fuzz 10% "$1" "$2" null: 2>&1 | sed 's/[^0-9.].*//'; }

# ---------------------------------------------------------------------------

ui_start_display
ui_start_app --mcp-folder "$ALLOWED"
ui_expect_log "MCP live instance:" "the application's MCP listener is up"
bridge_open
mcp_request initialize '{"protocolVersion": "2025-11-25", "capabilities": {},
  "clientInfo": {"name": "mitcad-files-camera", "version": "1"}}'
jq -e '.result.protocolVersion == "2025-11-25"' "$WORK/answer.json" > /dev/null || ui_fail "initialize failed"
mcp_send '{"jsonrpc": "2.0", "method": "notifications/initialized"}'
mcp_request tools/list
jq -e '[.result.tools[].name] | index("document_open") and index("ui_camera")' "$WORK/answer.json" > /dev/null ||
  ui_fail "the live catalog lacks document_open or ui_camera: $(jq -c '[.result.tools[].name]' "$WORK/answer.json")"

echo "--- Save an untitled design into an allowed folder (mitcad#402)"
mcp_tool document_list '{}'
expect '.documents[0].path == null' "the window's design is untitled"
expect '.folders | length == 1 and (.[0] | endswith("/allowed"))' "document_list lists the allowed folder"
DOC=$(jq -r '.documents[0].document' "$WORK/result.json")
mcp_tool model_command "$(jq -cn --arg d "$DOC" '{document: $d, command: {cmd: "add_feature", def: {type: "box",
  plane: {type: "origin", plane: "xy"}, corner: [0, 0], length: 30, width: 20, height: 10, operation: "new_body"}}}')"
mcp_tool document_save "$(jq -cn --arg d "$DOC" '{document: $d}')" true
expect '.error | contains("path")' "saving without a path asks for one"
mcp_tool document_save "$(jq -cn --arg d "$DOC" --arg p "$OUTSIDE/box.mitcad" '{document: $d, path: $p}')" true
expect '.error | contains("not in a folder agents may use")' "a folder not allowed is refused"
mcp_tool document_save "$(jq -cn --arg d "$DOC" '{document: $d, path: "../outside/box.mitcad"}')" true
expect '.error | contains("..")' "'..' is refused"
mcp_tool document_save "$(jq -cn --arg d "$DOC" '{document: $d, path: "box.step"}')" true
expect '.error | contains(".mitcad")' "only a .mitcad file is saved"
[ -e "$OUTSIDE/box.mitcad" ] && ui_fail "a refused save wrote a file outside the folder"
ui_mark
mcp_tool document_save "$(jq -cn --arg d "$DOC" '{document: $d, path: "box.mitcad"}')"
expect '.saved == true and (.path | endswith("/allowed/box.mitcad"))' "a relative path saves into the allowed folder"
[ -s "$ALLOWED/box.mitcad" ] || ui_fail "box.mitcad was not written"
ui_expect_new "Saved " "the window says it saved"
mcp_tool ui_state '{}'
expect '.modified == false and (.path | endswith("/allowed/box.mitcad")) and .name == "box.mitcad"' \
  "the window's design is that file now"
mcp_tool model_command "$(jq -cn --arg d "$DOC" '{document: $d, command: {cmd: "add_feature", def: {type: "box",
  plane: {type: "origin", plane: "xy"}, corner: [40, 0], length: 10, width: 10, height: 10, operation: "new_body"}}}')"
mcp_tool document_save "$(jq -cn --arg d "$DOC" --arg p "$ALLOWED/box.mitcad" '{document: $d, path: $p}')" true
expect '.error | contains("overwrite")' "an existing file needs overwrite"
mcp_tool document_save "$(jq -cn --arg d "$DOC" --arg p "$ALLOWED/box.mitcad" '{document: $d, path: $p, overwrite: true}')"
expect '.saved == true' "overwrite: true replaces it"
mcp_tool document_save "$(jq -cn --arg d "$DOC" '{document: $d}')"
expect '.saved == true and (.path | endswith("/allowed/box.mitcad"))' "a plain save goes to the design's file now"

echo "--- Open a design from the allowed folder (mitcad#402)"
mcp_tool model_command "$(jq -cn --arg d "$DOC" '{document: $d, command: {cmd: "add_feature", def: {type: "box",
  plane: {type: "origin", plane: "xy"}, corner: [60, 0], length: 10, width: 10, height: 10, operation: "new_body"}}}')"
mcp_tool document_open '{"path": "sub/blocks.mitcad"}' true
expect '.error | contains("unsaved changes")' "unsaved changes keep the design open"
mcp_tool document_open "$(jq -cn --arg p "$OUTSIDE/box.mitcad" '{path: $p}')" true
expect '.error | contains("not in a folder agents may use")' "opening outside the folder is refused"
ui_mark
mcp_tool document_open '{"path": "sub/blocks.mitcad", "discard": true}'
expect '.opened == true and (.path | endswith("/allowed/sub/blocks.mitcad")) and .document != $old' \
  "document_open opens the design with a new handle" --arg old "$DOC"
ui_expect_new "MCP: an agent opened" "the window opened it"
OLD=$DOC
DOC=$(jq -r '.document' "$WORK/result.json")
mcp_tool model_query "$(jq -cn --arg d "$OLD" '{document: $d, query: {query: "bodies"}}')" true
expect '.error | contains("document_list")' "the old handle is refused"
mcp_tool model_query "$(jq -cn --arg d "$DOC" '{document: $d, query: {query: "bodies"}}')"
expect '.result | length == 2' "the opened design has its two blocks"

echo "--- Aim the camera (mitcad#405)"
mcp_tool ui_camera '{}'
expect '.changed == false and (.camera.eye | length == 3) and .camera.height > 0' "ui_camera reads the camera"
screenshot "$WORK/whole.png"
mcp_tool ui_camera '{"eye": [10, -990, 10], "target": [10, 10, 10], "up": [0, 0, 1], "height": 50,
  "projection": "orthographic"}'
expect "$NEAR"' near(.camera.target[1]; 10; 0.01) and near(.camera.eye[1]; -990; 0.01) and near(.camera.height; 50; 0.01)
  and .camera.projection == "orthographic" and near(.camera.direction[1]; 1; 1e-6)' "eye, target, up and height are set"
mcp_tool ui_camera '{"box": {"min": [500, 0, 0], "max": [520, 20, 20]}}'
expect "$NEAR"' .framed == "box" and near(.camera.direction[1]; 1; 1e-6) and .camera.height < 80' \
  "box frames the second block along the view direction"
screenshot "$WORK/second.png"
mcp_tool ui_camera '{"box": {"min": [0, 0, 0], "max": [20, 20, 20]}}'
screenshot "$WORK/first.png"
SAME=$(differing "$WORK/first.png" "$WORK/second.png")
OTHER=$(differing "$WORK/whole.png" "$WORK/second.png")
echo "     pixels differing: the two framed blocks $SAME, the whole design and a block $OTHER"
awk -v same="$SAME" -v other="$OTHER" 'BEGIN { exit !(same * 20 < other && other > 2000) }' ||
  ui_fail "framing a block does not show the block as framed (differing pixels $SAME, against the whole $OTHER)"
echo "ok   the next screenshot shows the framed box"

mcp_tool ui_select "$(jq -cn --arg d "$DOC" '{document: $d, items: [{kind: "body", owner: "F2.b0"}], reveal: "fit"}')"
expect "$NEAR"' .view == "zoom" and near(.camera.target[0]; 510; 1)' "ui_select's fit reports the camera on the block"
screenshot "$WORK/selected.png"
SELECTED=$(differing "$WORK/first.png" "$WORK/selected.png")
awk -v n="$SELECTED" 'BEGIN { exit !(n > 500) }' || ui_fail "the screenshot after ui_select's fit is the earlier view"
echo "ok   the screenshot after ui_select's fit shows the new view ($SELECTED pixels differ)"
mcp_tool ui_select "$(jq -cn --arg d "$DOC" '{document: $d, items: [], reveal: "none"}')"

mcp_tool model_command "$(jq -cn --arg d "$DOC" '{document: $d, command: {cmd: "set_isolation", items: [{body: "F1.b0"}]}}')"
mcp_tool ui_command "$(jq -cn --arg d "$DOC" '{document: $d, id: "view.fit"}')"
expect "$NEAR"' near(.camera.target[0]; 10; 2) and .camera.height < 100' "view.fit frames only the isolated block"
mcp_tool model_command "$(jq -cn --arg d "$DOC" '{document: $d, command: {cmd: "set_isolation", items: []}}')"
mcp_tool ui_command "$(jq -cn --arg d "$DOC" '{document: $d, id: "view.fit"}')"
expect '.camera.width > 500' "without isolation view.fit frames both blocks"

mcp_tool model_command "$(jq -cn --arg d "$DOC" '{document: $d, command: {cmd: "add_named_view", name: "Home",
  eye: [510, -300, 400], target: [510, 10, 10], up: [0, 0, 1], height: 40}}')"
mcp_tool ui_command "$(jq -cn --arg d "$DOC" '{document: $d, id: "view.home"}')"
expect "$NEAR"' near(.camera.target[0]; 510; 0.01) and near(.camera.target[2]; 10; 0.01) and near(.camera.height; 40; 0.01)
  and near(.camera.eye[1]; -300; 0.01)' "view.home takes the Home view's eye, target and height"
mcp_tool ui_camera '{}'
expect "$NEAR"' near(.camera.target[0]; 510; 0.01) and near(.camera.height; 40; 0.01)' "and the view stays there"

mcp_tool ui_camera '{"projection": "perspective", "fov": 30, "frame": "shown"}'
expect "$NEAR"' .camera.projection == "perspective" and near(.camera.fov; 30; 0.01) and .framed == "shown"' \
  "perspective with a field of view, framing the shown bodies"
mcp_tool ui_camera '{"frame": "selection"}' true
expect '.error | contains("selected")' "framing an empty selection is refused"
mcp_tool ui_camera '{"eye": [0, 0, 0], "target": [0, 0, 0]}' true
expect '.error | contains("apart")' "an eye on the target is refused"

bridge_close
ui_crashed && ui_fail "the application crashed"
ui_finish "UI live MCP files and camera test"
