#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/print app/browser/BrowserController.cpp app/MainWindow.cpp app/files/MainWindowPrint.cpp
# check-all sources: core/model/src/print_library.rs core/model/src/document/library_items.rs tools/cli
# check-all sources: core/model/src/print_profile.rs core/model/src/print_files/installed.rs core/model/src/document/slicer_presets.rs
# My Library (mitcad#704) through the real UI: the user's materials,
# printers and processes managed without a design, on a design of one box
# that must not change. My library is the test's own folder
# (MITCAD_PRINT_LIBRARY_DIR), which mitcad-cli gives a printer, Bench X.
#   1. Tools › My Library… (the command search): the Materials tab lists
#      the built-in materials under Built-in; Duplicate of PLA Plastic
#      makes PLA Plastic (copy) in My library (its file), its name typed
#      (Bench PLA) and its density changed (1.3), each written to the
#      file; Delete asks ("Delete Bench PLA from My library?") and removes
#      the file.
#   2. The Printers tab: Bench X; Duplicate makes Bench X (copy) in My library,
#      renamed Bench Y; Delete removes it.
#   3. The design did not change: no modified mark in the title, its file
#      as it was; Edit Printers and Processes' My Library… opens the
#      window again.
#   4. A fake Bambu Studio data folder (mitcad#830): Import from Slicer ▾
#      offers its presets; the new process is ticked, the printer named as
#      Bench X is marked and unticked; Import brings the process into My
#      library and leaves Bench X as it was; the line names it and My
#      Library shows it on the Processes tab, selected. Then again, with
#      both names in My library: the keyboard moves to Fine, Space ticks
#      and unticks it, Tab reaches its choice; Override ticks the row and
#      All same names shows no value while the rows differ; Copy again;
#      Import names the count ("Import 1 Preset"); the filter "bench"
#      hides Fine, and Import still brings the ticked Fine in as Fine 2,
#      named in the line and selected.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-my-library-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-my-library.XXXXXX)
trap 'ui_cleanup; rm -rf -- "${WORK:?}"' EXIT
FILE=$WORK/box.mitcad
# The user's library of materials and profiles in the test's own folder.
export MITCAD_PRINT_LIBRARY_DIR=$WORK/print-library
LIB=$MITCAD_PRINT_LIBRARY_DIR

# expect_item path filter expected description: a jq filter on a library
# item's file.
expect_item() {
  local actual
  [ -f "$1" ] || ui_fail "$4: no $1"
  actual=$(jq -c "$2" "$1") || ui_fail "$4: cannot read $1"
  [ "$actual" = "$3" ] || ui_fail "$4: $2 is '$actual', expected '$3'"
  echo "ok   $4"
}

# logged_after "text" sed-expression: the last line since the mark with
# the text, through the expression.
logged_after() {
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -F -- "$1" | tail -1 | sed "$2"
}

# The design: one box. My library: a printer.
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-10, -10],
    "length": 20, "width": 20, "height": 20, "operation": "new_body"}}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
cat > "$WORK/printer.json" << 'EOF'
[
  {"cmd": "create_print_profile", "library": "user", "kind": "printer", "name": "Bench X",
   "values": {"bed_shape": [[0, 0], [220, 0], [220, 220], [0, 220]], "max_print_height": 250,
              "nozzle_diameter": 0.4}}
]
EOF
"$CLI" run "$WORK/printer.json" --save "$WORK/other.mitcad" > "$WORK/cli-printer.log" 2>&1 ||
  { cat "$WORK/cli-printer.log"; ui_fail "mitcad-cli (Bench X into My library)"; }
[ -f "$LIB/print/printers/bench-x.json" ] || ui_fail "Bench X is not in My library"
DESIGN_SUM=$(sha256sum "$FILE" | cut -c1-64)

ui_start_display
ui_start_app --open "$FILE" --no-native-dialogs
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Body F1.b0 at" "the box is in the view"

echo "--- 1. My Library: a built-in material duplicated, renamed, its density changed, deleted"
ui_mark
ui_command "My Library"
ui_expect_new "My Library opened" "the window opened"
ui_expect_new "My Library materials row: plastics/pla PLA Plastic (mitcad)" "PLA is listed, built in"
ui_expect_new "My Library tab printers at" "the tabs have their places"
ui_expect_new "My Library import at" "Import from Slicer… has its place"
ui_expect_new "My Library materials item PLA Plastic at" "its row has its place"
ui_focus_dialog '^My Library$'
ui_mark
ui_step "PLA Plastic"                      ui_click_logged "My Library materials item PLA Plastic"
ui_expect_new "Material selected: PLA Plastic (mitcad)" "PLA is selected"
ui_expect_new "My Library materials duplicate at" "Duplicate under the list"
ui_step "Duplicate"                        ui_click_logged "My Library materials duplicate"
ui_expect_new "Material duplicated: PLA Plastic as " "a copy made"
id=$(logged_after "Material duplicated: PLA Plastic as " 's/.* as \(.*\) in \(.*\)$/\1/')
place=$(logged_after "Material duplicated: PLA Plastic as " 's/.* as .* in \(.*\)$/\1/')
[ "$place" = user ] || ui_fail "the copy went to '$place', not My library"
echo "ok   in My library, not in the design"
MATERIAL=$LIB/materials/$id.json
expect_item "$MATERIAL" '.material.name' '"PLA Plastic (copy)"' "its file, named after PLA"
ui_expect_new "My Library materials row: plastics/$id PLA Plastic (copy) (user)" "the list shows it"
ui_sync
ui_mark
xdotool type --delay 40 "Bench PLA"
ui_key Return
ui_expect_new "Material PLA Plastic (copy) (user) name = Bench PLA" "Enter renamed it"
expect_item "$MATERIAL" '.material.name' '"Bench PLA"' "the file has the new name"
ui_expect_new "My Library materials density at" "Density has its place"
ui_step "Density"                          ui_click_logged "My Library materials density"
ui_key ctrl+a
xdotool type --delay 40 "1.3"
ui_key Return
ui_expect_new "Material Bench PLA (user) density = 1.3" "Enter saved the density"
expect_item "$MATERIAL" '.material.density' '1.3' "the file has the new density"
ui_mark
ui_step "Delete"                           ui_click_logged "My Library materials delete"
ui_expect_new "My Library materials delete: Delete Bench PLA from My library?" \
  "Delete asks first, naming the library"
ui_focus_dialog '^Delete$'
ui_step "Delete (Alt+D)"                   ui_key alt+d
ui_expect_new "Material deleted: Bench PLA (user)" "deleted"
[ ! -e "$MATERIAL" ] || ui_fail "the file is still there"
echo "ok   its file is gone"

echo "--- 2. Printers: Bench X duplicated, renamed, deleted"
ui_focus_dialog '^My Library$'
ui_mark
ui_step "the Printers tab"                 ui_click_logged "My Library tab printers"
ui_expect_new "My Library tab: printers" "the Printers tab"
ui_expect_new "My Library printers row: printer/bench-x Bench X (user)" "Bench X is listed"
ui_expect_new "My Library printers item Bench X at" "its row has its place"
ui_step "Bench X"                          ui_click_logged "My Library printers item Bench X"
ui_expect_new "Profile selected: printer Bench X (user)" "Bench X is selected"
ui_expect_new "My Library printers duplicate at" "Duplicate under the list"
ui_mark
ui_step "Duplicate"                        ui_click_logged "My Library printers duplicate"
ui_expect_new "Profile duplicated: Bench X as " "a copy made"
pid=$(logged_after "Profile duplicated: Bench X as " 's/.* as \(.*\) in \(.*\)$/\1/')
place=$(logged_after "Profile duplicated: Bench X as " 's/.* as .* in \(.*\)$/\1/')
[ "$place" = user ] || ui_fail "the copy went to '$place', not My library"
PRINTER=$LIB/print/printers/$pid.json
expect_item "$PRINTER" '.profile.name' '"Bench X (copy)"' "its file, named after Bench X"
expect_item "$PRINTER" '.profile.values.max_print_height == 250' 'true' "with Bench X's values"
ui_sync
ui_mark
xdotool type --delay 40 "Bench Y"
ui_key Return
ui_expect_new "Profile Bench X (copy) (user) name = Bench Y" "Enter renamed it"
expect_item "$PRINTER" '.profile.name' '"Bench Y"' "the file has the new name"
ui_mark
ui_step "Delete"                           ui_click_logged "My Library printers delete"
ui_expect_new "My Library printers delete: Delete Bench Y from My library?" "Delete asks first"
ui_focus_dialog '^Delete$'
ui_step "Delete (Alt+D)"                   ui_key alt+d
ui_expect_new "Profile deleted: Bench Y (user)" "deleted"
[ ! -e "$PRINTER" ] || ui_fail "the printer's file is still there"
[ -f "$LIB/print/printers/bench-x.json" ] || ui_fail "Bench X went too"
echo "ok   its file is gone, Bench X stays"
ui_focus_dialog '^My Library$'
ui_mark
ui_step "close"                            ui_click_logged "My Library printers close"
ui_expect_new "My Library closed after changes" "closed"

echo "--- 3. The design did not change; Printers and Processes links to My Library"
ui_focus_main
title=$(xdotool getwindowname "$UI_WINDOW")
case $title in
  *"*"*) ui_fail "the design is marked changed: $title" ;;
esac
[ "$(sha256sum "$FILE" | cut -c1-64)" = "$DESIGN_SUM" ] || ui_fail "the design's file changed"
echo "ok   the design is unchanged ($title)"
ui_mark
ui_command "Edit Printers and Processes"
ui_expect_new "Printers and processes opened" "Printers and Processes opened"
ui_expect_new "Profiles my_library at" "My Library… beside Close"
ui_focus_dialog '^Printers and Processes$'
ui_mark
ui_step "My Library…"                      ui_click_logged "Profiles my_library"
ui_expect_new "Profiles: My Library…" "the link"
ui_expect_new "My Library opened" "My Library opened again"

echo "--- 4. Presets of an installed slicer (mitcad#830): a fake Bambu Studio folder"
# The slicer's data folder in the test's own config folder: a process of
# the user's, and a printer named as My library's Bench X.
PRESETS=$XDG_CONFIG_HOME/BambuStudio/user/default
mkdir -p "$PRESETS/process" "$PRESETS/machine"
printf '{"name": "Fine", "from": "User", "layer_height": "0.12"}\n' > "$PRESETS/process/Fine.json"
printf '{"name": "Bench X", "from": "User", "printable_height": "180"}\n' > "$PRESETS/machine/Bench X.json"
BENCH_SUM=$(sha256sum "$LIB/print/printers/bench-x.json" | cut -c1-64)
ui_focus_dialog '^My Library$'
ui_mark
ui_step "Import from Slicer"               ui_click_logged "My Library import"
ui_expect_new "My Library import menu: From a file… | Presets of Bambu Studio…" "a file or the slicer's presets"
ui_key Down Down Return
ui_expect_new "Slicer presets opened: Bambu Studio, 2 presets" "the slicer's presets listed"
ui_expect_new "Slicer presets row: process Fine, ticked" "a new one ticked"
ui_expect_new "Slicer presets row: printer Bench X, unticked, same name: My library has a printer of this name, copy" \
  "the name My library has: marked, unticked, Copy"
ui_expect_new "Slicer presets ticked: 1, Import 1 Preset" "Import says how many"
ui_expect_new "Slicer presets all same names: copy" "All same names shows the rows' Copy"
ui_focus_dialog '^Import Presets$'
ui_step "Import (Alt+I)"                   ui_key alt+i
ui_expect_new "My Library import: Fine imported into My library" "the ticked one imported, by its name"
ui_expect_new "My Library tab: processes" "My Library shows the Processes tab"
ui_expect_new "Profile selected: process Fine (user)" "with Fine selected"
expect_item "$LIB/print/processes/fine.json" '.profile.values.layer_height' '0.12' "Fine is in My library"
# Its number as a number: the file keeps it as the model wrote it (250.0).
expect_item "$LIB/print/printers/bench-x.json" '.profile.values.max_print_height == 250' 'true' \
  "Bench X keeps its printable height"
[ "$(sha256sum "$LIB/print/printers/bench-x.json" | cut -c1-64)" = "$BENCH_SUM" ] ||
  ui_fail "the unticked Bench X's file changed"
echo "ok   Bench X's file as it was"

echo "--- 4b. Both names in My library now: the keyboard, a choice that ticks its row, Fine as Fine 2"
FINE_SUM=$(sha256sum "$LIB/print/processes/fine.json" | cut -c1-64)
ui_focus_dialog '^My Library$'
ui_mark
ui_step "Import from Slicer"               ui_click_logged "My Library import"
ui_expect_new "My Library import menu: From a file… | Presets of Bambu Studio…" "the menu again"
ui_key Down Down Return
ui_expect_new "Slicer presets row: process Fine, unticked, same name: My library has a process of this name, copy" \
  "Fine is now a name My library has: unticked, Copy"
ui_expect_new "Slicer presets ticked: 0, Import" "nothing ticked: plain Import"
ui_expect_new "Slicer presets all same names: copy" "All same names: Copy, as both rows"
ui_focus_dialog '^Import Presets$'
# The list has the keyboard on its first preset, Bench X: down past the
# Processes heading to Fine.
ui_mark
ui_step "Down Down"                        ui_key Down Down
ui_expect_new "Slicer presets current: Fine" "the arrows move to Fine"
ui_mark
ui_step "Space"                            ui_key space
ui_expect_new "Slicer presets tick: Fine ticked" "Space ticks it"
ui_mark
ui_step "Space"                            ui_key space
ui_expect_new "Slicer presets tick: Fine unticked" "Space again unticks it"
ui_mark
ui_step "Tab"                              ui_key Tab
ui_expect_new "Slicer presets focus: Fine choice" "Tab reaches Fine's Copy or Override"
ui_mark
ui_step "Down (Override)"                  ui_key Down
ui_expect_new "Slicer presets choice: Fine override" "Override chosen"
ui_expect_new "Slicer presets tick: Fine ticked" "the choice ticked its unticked row"
ui_expect_new "Slicer presets ticked: 1, Import 1 Preset" "only Fine ticked: Import 1 Preset"
ui_expect_new "Slicer presets all same names: none" "the rows differ: All same names shows no value"
ui_mark
ui_step "Up (Copy)"                        ui_key Up
ui_expect_new "Slicer presets choice: Fine copy" "Copy chosen again"
ui_expect_new "Slicer presets all same names: copy" "the rows agree again: Copy"
# The filter (mitcad#871 review): Shift+Tab back to the row, again to the
# filter; "bench" hides Fine, which stays ticked and is imported.
ui_mark
ui_step "Shift+Tab Shift+Tab (filter)"     ui_key shift+Tab shift+Tab
ui_step "type bench"                       xdotool type --delay 40 "bench"
ui_expect_new "Slicer presets filter: bench, 1 shown" "the filter shows Bench X alone"
ui_mark
ui_step "Import (Alt+I)"                   ui_key alt+i
ui_expect_new "My Library import: Fine imported into My library as Fine 2" "the line names the copy's name"
ui_expect_new "Profile selected: process Fine 2 (user)" "My Library shows Fine 2 selected"
expect_item "$LIB/print/processes/fine-2.json" '.profile.name' '"Fine 2"' "Fine 2 is in My library"
[ "$(sha256sum "$LIB/print/processes/fine.json" | cut -c1-64)" = "$FINE_SUM" ] ||
  ui_fail "Fine's file changed by a Copy"
[ "$(sha256sum "$LIB/print/printers/bench-x.json" | cut -c1-64)" = "$BENCH_SUM" ] ||
  ui_fail "the unticked Bench X's file changed"
echo "ok   Fine and Bench X as they were"
ui_focus_dialog '^My Library$'
ui_step "close (Esc)"                      ui_key Escape
ui_expect_new "My Library closed" "closed"

ui_finish "UI My Library"
