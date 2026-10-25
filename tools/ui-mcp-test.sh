#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindowMcp.cpp app/MainWindowMcpUi.cpp app/mcp core/mcp core/ffi/src/mcp.rs
# check-all sources: app/framework/CommandSessionScript.cpp app/framework/CommandPanelState.cpp
# check-all sources: app/sketch/SketchReveal.cpp app/view/ViewReveal.cpp app/files/ProjectIndicator.cpp
# check-all sources: core/model/src/api/classes.rs tools/cli tools/ui-mcp-live.py
# check-all sources: app/framework/TestDriverAssistant.cpp app/tests/assistant_client.cpp tools/ui-assistant-setup-test.py
# Live MCP for debugging (mitcad#265) through the real UI on Xvfb and the
# actual bridge, `mitcad-cli mcp --connect`: the application opens a design
# whose sketch F4 holds a projection that no longer follows its face
# (tools/ui-mcp-live.py makes it with mitcad-cli), and the flow of
# tools/ui-mcp-live.py runs against it: queries with the sketch open,
# sketch.add_line through the sketch session and undo, ui_state with the
# palette's warning, ui_select, ui_screenshot, ui_command Clean Up Stale
# Projections and Finish Sketch, document_save, ui_log. The project
# indicator names the initialized client while the bridge is connected.
# Then the shared setup-dialog flow checks isolated file and stub-CLI
# registration, failures, concurrent edits, folder consent and both layouts.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-mcp-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
ASSISTANT_CLIENT=${UI_ASSISTANT_CLIENT:-$(dirname "$UI_APP")/test_assistant_client}
LIVE="$(dirname "$0")/ui-mcp-live.py"
WORK=$(mktemp -d /tmp/mitcad-ui-mcp.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
[ -x "$ASSISTANT_CLIENT" ] || ui_fail "assistant client stub is missing: build the test_assistant_client target"

# The listener's record in a private folder of the test's own; the app makes
# it (it must not exist yet).
export MITCAD_MCP_DISCOVERY_DIR=$WORK/discovery
FILE=$(python3 -I -c 'import importlib.util, sys
spec = importlib.util.spec_from_file_location("live", sys.argv[1])
live = importlib.util.module_from_spec(spec)
spec.loader.exec_module(live)
print(live.make_design(sys.argv[2], sys.argv[3]))' "$LIVE" "$CLI" "$WORK") || ui_fail "mitcad-cli did not make the design"

ui_start_display
ui_start_app --mcp-folder "$WORK" --open "$FILE"
ui_expect_log "MCP live instance:" "the application's MCP listener is up"
ui_expect_log "Timeline Sketch2 at" "the design is open"

echo "--- The live debugging flow through the bridge"
ui_mark
python3 -I "$LIVE" "$CLI" "$FILE" || ui_fail "the live MCP flow failed (above)"
ui_expect_new "Agent connected" "the window said an agent was connected"
ui_expect_new "Project indicator: not in a project, connected: mitcad-live-debug" "the project indicator named the client"
ui_expect_new "Agent disconnected" "the bridge's end disconnected it"
ui_expect_new "Saved $FILE" "the agent saved the design"
ui_crashed && ui_fail "the application crashed"

ui_finish "UI live MCP test"

# Keep this test's private display while the harness starts its own isolated
# application instances. The normal Mitcad build also builds the client stub.
ui_stop_app
ASSISTANT_LOG_DIR=${MITCAD_UI_LOG_DIR:-$WORK}/assistant-setup
python3 -I "$(dirname "$0")/ui-assistant-setup-test.py" "$UI_APP" "$CLI" "$ASSISTANT_CLIENT" "$ASSISTANT_LOG_DIR" ||
  ui_fail "assistant setup UI failed; see $ASSISTANT_LOG_DIR"
