#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework app/browser core/model
# Body transfers (mitcad#465) through the real UI: a block modelled in the
# root and an empty component Bracket placed 40 mm along x. Move to
# Component on the block's browser row, with "Move Only the Bodies", into
# Bracket: a BodyTransfer1 step in Bracket at the timeline marker; the
# block stays where it was on screen, now shown in Bracket:1, while the
# sketch and the extrude that made it stay in the root. Undo takes it back;
# the design saved and opened again keeps it.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-body-transfer-test.sh [screenshot.png]
# check-all sources: tools/cli

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-body-transfer.XXXXXX)
FILE=$WORK/block.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT

save() {
  ui_mark
  ui_key ctrl+s
  ui_expect_new "Saved $FILE" "saved"
}

# The block's place in the window, from the last "Body <uid>[ in <path>] at
# x,y" line.
place() {
  grep -E "Body $1( in O[0-9]+)? at " "$UI_LOG" | tail -1 | sed -n 's/.* at \([0-9-]*,[0-9-]*\).*/\1/p'
}

# expect_info pattern description: the saved design, opened by mitcad-cli,
# has a line matching the pattern.
expect_info() {
  "$CLI" info "$FILE" > "$WORK/info.txt" 2>&1 || { cat "$WORK/info.txt"; ui_fail "$2: mitcad-cli info"; }
  grep -qE "$1" "$WORK/info.txt" || { cat "$WORK/info.txt"; ui_fail "$2: no line '$1'"; }
  echo "ok   $2"
}

# Sketch1 (F1) 30 x 20, Extrude1 (F2) 10 up: Body1 (F2.b0) in the root;
# Bracket (C1) placed at x 40 in the root, empty.
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "set_hybrid_assembly"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "create_component", "name": "Bracket", "transform": {"translation": [40, 0, 0]},
   "activate": false}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Bodies shown: F2.b0" "the block shown in the root"
ui_sync
BEFORE=$(place F2.b0)
[ -n "$BEFORE" ] || ui_fail "no place logged for the block"

echo "--- Move to Component on the body's row: the body only"
ui_mark
ui_step "right-click Body1"              ui_click_logged "Browser Root/Bodies/Body1" 3
ui_step "choose Move to Component..."    ui_menu_choose "Move to Component..."
ui_focus_dialog '^Move to Component$'
ui_step "only the bodies (Alt+O)"        ui_key alt+o
ui_step "destination list (Alt+D)"       ui_key alt+d
ui_step "Bracket (Home, Down)"           ui_key Home Down
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_step "move (Alt+M)"                   ui_key alt+m
ui_focus_main
ui_expect_new "Transferred Body1 into Bracket (BodyTransfer1)" "the body transferred into Bracket"
ui_expect_new "Bodies shown: F2.b0 in O1" "the block shown in Bracket:1"
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -q "^Moved " && ui_fail "features moved instead"
ui_sync
AFTER=$(place F2.b0)
[ "$AFTER" = "$BEFORE" ] || ui_fail "the block moved on screen: $BEFORE -> $AFTER"
echo "ok   the block stays where it was ($AFTER)"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

echo "--- Undo and redo"
ui_mark
ui_step "undo (Ctrl+Z)"                  ui_key ctrl+z
ui_expect_new "Undo: Move Body1 into Bracket" "one undo step"
ui_expect_new "Bodies shown: F2.b0" "the block in the root again"
ui_mark
ui_step "redo (Ctrl+Y)"                  ui_key ctrl+y
ui_expect_new "Bodies shown: F2.b0 in O1" "the block in Bracket:1 again"

echo "--- Saved"
save
expect_info "BodyTransfer1 \(body_transfer\): ok" "the transfer computes in the saved design"
expect_info "Bracket:1/Body1 \(F2.b0\)" "the block is Bracket's"
expect_info "C0 Root: 2 features" "the sketch and the extrude stay in the root"

ui_finish "UI body transfer test"
