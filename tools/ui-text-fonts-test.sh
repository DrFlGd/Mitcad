#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/commands app/view core/model tools/cli
# Sketch text in a font file (mitcad#473) through the real UI:
#   - Preferences, Fonts: a folder of fonts added; the settings keep it.
#   - Edit Text: the folder's font in the font chooser, with previews;
#     chosen, the text takes the file's outlines. Then File... chooses a
#     copy of the font in another folder.
#   - The design saved keeps the file's path, its licence note and the
#     outlines, and an extruded letter has the same volume once the font
#     files are gone (mitcad-cli).
#   - The text has a Hebrew word after "MIT": in the font file it is
#     shaped and runs right to left, its first letter rightmost.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-text-fonts-test.sh [screenshot.png]

source "$(dirname "$0")/ui-test-lib.sh"

SHOT=${1:-}
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
SOURCE_FONT="$(cd "$(dirname "$0")/.." && pwd)/core/model/data/fonts/LiberationSans-Regular.ttf"
WORK=$(mktemp -d /tmp/mitcad-ui-fonts.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
FILE="$WORK/text.mitcad"
mkdir -p "$WORK/fonts/sans" "$WORK/other"
cp "$SOURCE_FONT" "$WORK/fonts/sans/"
cp "$SOURCE_FONT" "$WORK/other/"
LABEL="Liberation Sans (LiberationSans-Regular.ttf)"
WORD=$'שלום'

# Sketch1 (F1): "MIT" and a Hebrew word, 10 high in the bundled font
# (which has no Hebrew letters).
cat > "$WORK/text.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_text", "sketch": "F1", "text": "MIT $WORD", "at": [0, 0], "height": 10}
]
EOF
"$CLI" run "$WORK/text.json" --save "$FILE" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli text"; }

# The M extruded 2 from the saved design: its volume (mitcad-cli).
cat > "$WORK/extrude.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{t1.g0.c0}"}],
    "extent": {"type": "distance", "distance": 2}, "operation": "new_body"}}
]
EOF
m_volume() {
  "$CLI" run "$WORK/extrude.json" --open "$FILE" 2>&1 | sed -n 's/.*(F2.b0): volume \([0-9.]*\) mm3.*/\1/p'
}

ui_start_display
ui_start_app --no-native-dialogs --open "$FILE"

echo "--- Preferences, Fonts: a folder of fonts"
ui_mark
ui_step "Preferences: Fonts"               ui_command "Preferences: Fonts"
ui_expect_new "Preferences font folder add at" "the Fonts page"
ui_focus_dialog '^Preferences$'
ui_step "Add..."                           ui_click_logged "Preferences font folder add"
ui_step "choose the folder"                ui_type_path "Font Folder" "$WORK/fonts"
ui_focus_dialog '^Preferences$'
ui_step "OK (Return)"                      ui_key Return
ui_expect_new "Preferences: font folders $WORK/fonts" "the folder is added"
grep -q "^folders=.*$WORK/fonts" "$UI_CONFIG/Mitcad/Mitcad.conf" || ui_fail "the settings do not keep the folder"
echo "ok   the settings keep it"
ui_focus_main

echo "--- Edit Text: the folder's font"
ui_step "double-click Sketch1"             ui_double_click_logged "Timeline Sketch1"
ui_expect_log "Editing sketch F1 (Sketch1)" "the sketch edited"
ui_step "fit (F6)"                         ui_key F6
# Where the text is: Edit Text's pick logs the sketch's entities.
ui_step "edit text (search)"               ui_command "Edit Text"
ui_expect_log "Sketch entity F1/t1 at" "the text's place"
ui_step "cancel (Esc)"                     ui_key Escape
ui_double_click_expect "Sketch entity F1/t1" "Double-click on t1: sketch.edit_text" \
  "the double-click opened Edit Text"
ui_expect_log "Panel Edit Text button_font at" "the font file button"
grep "Panel Edit Text options font:" "$UI_LOG" | tail -1 | grep -qF "| $LABEL" ||
  ui_fail "the font chooser does not list the folder's font"
echo "ok   the chooser lists the folder's font"
pictures=$(sed -n 's/.*Panel Edit Text option pictures font: \([0-9]*\) of \([0-9]*\).*/\1 \2/p' "$UI_LOG" | tail -1)
read -r shown all <<< "$pictures"
[ -n "$shown" ] && [ "$shown" -gt 1 ] && [ "$shown" = "$all" ] ||
  ui_fail "the fonts have no previews ($pictures)"
echo "ok   every font has a preview ($shown)"
ui_mark
ui_step "open the font chooser"            ui_click_logged "Panel Edit Text input font"
ui_sync
ui_step "the last font (End, Return)"      ui_key End Return
ui_expect_new "Edit Text: Font = $LABEL" "the folder's font chosen"
ui_expect_log "Preview Edit Text: ok" "previewed"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Edited text t1: \"MIT $WORD\", $LABEL, left" "the text is in the font file's font"

echo "--- Edit Text: File... chooses a font file"
ui_double_click_expect "Sketch entity F1/t1" "Double-click on t1: sketch.edit_text" \
  "Edit Text again"
ui_expect_log "Panel Edit Text button_font at" "the font file button"
ui_mark
ui_step "File..."                          ui_click_logged "Panel Edit Text button_font"
ui_type_path "Font File" "$WORK/other/LiberationSans-Regular.ttf"
ui_focus_main
ui_expect_new "Edit Text font file $WORK/other/LiberationSans-Regular.ttf" "the file chosen"
ui_expect_new "Preview Edit Text: ok" "previewed"
[ -n "$SHOT" ] && ui_capture "$SHOT"
ui_step "OK (Enter)"                       ui_key Return
ui_expect_log "Edited text t1: \"MIT $WORD\", $LABEL, left" "the text is in the chosen file's font"
ui_step "finish sketch (Ctrl+Enter)"       ui_key ctrl+Return
ui_expect_log "Sketch finished" "sketch finished"
ui_mark
ui_key ctrl+s
ui_expect_new "Saved $FILE" "saved"
grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"

echo "--- The design keeps the file's outlines"
text() { jq -r ".features[0].texts[0]$1" "$FILE"; }
[ "$(text .font_file.path)" = "$WORK/other/LiberationSans-Regular.ttf" ] ||
  ui_fail "the font file's path: $(text .font_file.path)"
text .font_file.licence | grep -q "Open Font License" || ui_fail "no licence note: $(text .font_file.licence)"
[ "$(text .font)" = "Liberation Sans" ] && [ "$(text '.glyphs.glyphs | length')" = 8 ] ||
  ui_fail "the outlines are not stored: $(text .font), $(text '.glyphs.glyphs | length')"
echo "ok   path, licence note and outlines"
before=$(m_volume)
[ -n "$before" ] || ui_fail "the M was not extruded"
mv "$WORK/fonts" "$WORK/fonts-gone"
mv "$WORK/other" "$WORK/other-gone"
after=$(m_volume)
[ "$after" = "$before" ] || ui_fail "the M's volume without the font file: $after, with it $before"
echo "ok   the extruded M: $after mm3 without the font file too"

echo "--- The Hebrew word runs right to left"
# Its first two letters (characters 4 and 5) extruded: the first lies
# right of the second.
cat > "$WORK/hebrew.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{t1.g4.c0}"}],
    "extent": {"type": "distance", "distance": 2}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{t1.g5.c0}"}],
    "extent": {"type": "distance", "distance": 2}, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/hebrew.json" --open "$FILE" > "$WORK/hebrew.log" 2>&1 || { cat "$WORK/hebrew.log"; ui_fail "the Hebrew letters"; }
centres=$(sed -n 's/.*center of mass \[\([-0-9.]*\),.*/\1/p' "$WORK/hebrew.log" | tr '\n' ' ')
read -r first second <<< "$centres"
awk -v a="$first" -v b="$second" 'BEGIN { exit !(a > b + 1) }' ||
  ui_fail "the first Hebrew letter at x $first is not right of the second at $second"
echo "ok   the first letter at x $first, the second at $second"

ui_finish "UI text fonts test"
