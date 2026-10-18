#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/build app/commands/BuildCommands.cpp app/framework
# check-all sources: core/model/src/build core/model/src/api core/vcs/src/library tools/cli
# The BUILD tab (mitcad#375) through the real UI, with a small library of
# synthetic BUILD parts made here (mitcad-cli: a 20 x 20 profile of I-type
# slot 5, a corner block, a panel, a rail and a carriage; no supplier's
# files), fetched from a local git repository:
#   1. The BUILD tab beside SOLID, its groups.
#   2. A new design, Part1 active (mitcad#394): Profile adds to the root
#      without activating it, no policy message; the panel starts with the
#      part, its length (the stock length) and the X / Y / Z buttons, the
#      view's hint; the first profile by a pick on the root's XY plane,
#      then (undone) by Enter at the origin, 60 mm long; then only
#      profiles' faces are picked and the planes are hidden; a click on its
#      side places one flush with an end, a pick on its end face shows
#      Mitre and places one mitred; Close shows the planes again.
#      The same panel in the Floating chrome (More opens Placement).
#   3. Profile on a design with only its root: the first profile on the
#      XY plane (typed length); two on its side flush with its ends (offset
#      0 from A and from B); the fourth on the side of the second at its
#      far end, its length bound to the third (Tab steps face, centre,
#      through; the box at the cursor shows it) - a frame of four.
#   4. Connector at a junction (shown on the hover, the chosen one
#      always), from the fitting connectors, previewed in place.
#   5. Panel in the frame from an inner face.
#   6. Linear Part: a rail on the first profile, a carriage on the rail;
#      Ends: an end of the third profile mitred and bored (end machining).
#   7. Save, reopen: the parts are there (their sources in the design);
#      Cut List: its rows (one with a 45 degree end), Copy as CSV, Save as
#      PDF.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-build-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CLI=${UI_CLI:-$(dirname "$UI_APP")/../tools/cli/mitcad-cli}
mkdir -p "$ROOT/build"
WORK=$(mktemp -d "$ROOT/build/ui-build.XXXXXX")
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
export MITCAD_LIBRARIES_DIR=$WORK/cache
LIB=$WORK/test-profiles
FILE=$WORK/frame.mitcad
PDF=$WORK/cut-list.pdf
g() { git -C "$1" -c user.name=T -c user.email=t@example.invalid "${@:2}" > /dev/null; }

RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
# frame NAME ORIGIN Z X: a mount frame (docs/libraries.md, "BUILD parts").
frame() {
  printf '  {"cmd": "add_feature", "name": "mount:%s", "def": {"type": "joint_origin", "geometry": "origin",
    "frame_override": {"origin": %s, "z_axis": %s, "x_axis": %s}}},\n' "$1" "$2" "$3" "$4"
}
# bar SIZE: a square bar of SIZE mm centred on the Z axis, extruded by L
# from z = 0, with the frames of a cut-to-length part.
bar() {
  local half
  half=$(awk -v s="$1" 'BEGIN { print s / 2 }')
  printf '[\n  {"cmd": "add_parameter", "name": "L", "expression": "500 mm"},\n'
  printf '  {"cmd": "sketch.create", "plane": "xy"},\n'
  printf '  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-%s, -%s], "width": %s, "height": %s},\n' \
    "$half" "$half" "$1" "$1"
  printf '  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "%s"}],
    "extent": {"type": "distance", "distance": "L"}, "operation": "new_body"}},\n' "$RECT"
  frame end_a "[0, 0, 0]" "[0, 0, -1]" "[1, 0, 0]"
  printf '  {"cmd": "add_feature", "name": "mount:end_b", "def": {"type": "joint_origin",
    "geometry": {"body": "F2.b0", "face": "F2:end(%s)"}}},\n' "$RECT"
}
# block X Y Z (and more frames after): a box from the origin.
block() {
  printf '[\n  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"},
    "corner": [0, 0], "length": %s, "width": %s, "height": %s, "operation": "new_body"}},\n' "$1" "$2" "$3"
}
design() { # design FILE: the script on stdin saved as a design
  local script=$WORK/$(basename "$1" .mitcad).json
  cat > "$script"
  mkdir -p "$(dirname "$1")"
  "$CLI" run "$script" --save "$1" > /dev/null || ui_fail "mitcad-cli run $(basename "$script") failed"
}

"$CLI" library init "$LIB" --id test-profiles --name "Test profiles" --license MIT > /dev/null ||
  ui_fail "library init"
{ bar 20
  frame "slot_x+_1" "[10, 0, 0]" "[1, 0, 0]" "[0, 0, 1]"
  frame "slot_x-_1" "[-10, 0, 0]" "[-1, 0, 0]" "[0, 0, 1]"
  frame "slot_y+_1" "[0, 10, 0]" "[0, 1, 0]" "[0, 0, 1]"
  frame "slot_y-_1" "[0, -10, 0]" "[0, -1, 0]" "[0, 0, 1]"
  printf '  {"expect": {"bodies": 1}}\n]\n'; } | design "$LIB/profiles/p20-test.mitcad"
{ block 18 18 18
  frame mount_a "[9, 9, 0]" "[0, 0, -1]" "[1, 0, 0]"
  frame mount_b "[0, 9, 9]" "[-1, 0, 0]" "[0, 0, 1]"
  printf '  {"expect": {"bodies": 1}}\n]\n'; } | design "$LIB/connectors/bracket-20-test.mitcad"
{ printf '[\n  {"cmd": "add_parameter", "name": "W", "expression": "100 mm"},
  {"cmd": "add_parameter", "name": "H", "expression": "100 mm"},
  {"cmd": "add_parameter", "name": "t", "expression": "3 mm"},\n'
  # Centred on its XY origin: width along X, height along Y, thickness
  # along Z (commands.md, "Joints BUILD makes"). A box's corner takes
  # numbers only: the box from the origin (F1), moved back by half its
  # width and height (F2), so that W and H still size it about the origin.
  block 0 0 0 | tail -n +2 | sed 's/"length": 0, "width": 0, "height": 0/"length": "W", "width": "H", "height": "t"/'
  printf '  {"cmd": "add_feature", "def": {"type": "move", "bodies": ["F1.b0"],
    "transform": {"type": "translate_xyz", "x": "-W / 2", "y": "-H / 2", "z": 0}}},\n'
  frame face "[0, 0, 0]" "[0, 0, -1]" "[1, 0, 0]"
  printf '  {"expect": {"bodies": 1}}\n]\n'; } | design "$LIB/panels/panel-test.mitcad"
{ bar 15
  frame rail "[0, 7.5, 0]" "[0, 1, 0]" "[0, 0, 1]"
  # Its underside on a profile's slot: its first frame of kind face.
  frame base "[0, -7.5, 0]" "[0, -1, 0]" "[0, 0, 1]"
  printf '  {"expect": {"bodies": 1}}\n]\n'; } | design "$LIB/linear/rail-test.mitcad"
{ block 30 20 20
  frame rail "[15, 10, 0]" "[0, 0, -1]" "[1, 0, 0]"
  printf '  {"expect": {"bodies": 1}}\n]\n'; } | design "$LIB/linear/carriage-test.mitcad"

SYSTEM='{"type": "I", "slot": 5, "grid": 20}'
cat > "$LIB/mitcad-library.json" <<EOF
{
  "format": "mitcad-library", "version": 1, "id": "test-profiles", "name": "Test profiles",
  "description": "Synthetic BUILD parts of the UI test.", "library_version": "1.0.0", "license": "MIT",
  "authors": ["Mitcad UI test"], "units": "mm",
  "categories": [{"id": "profiles", "name": "Profiles"}, {"id": "connectors", "name": "Connectors"},
                 {"id": "panels", "name": "Panels"}, {"id": "linear", "name": "Linear parts"}],
  "components": [
    {"id": "p20-test", "path": "profiles/p20-test.mitcad", "name": "Profile 20 test", "category": "profiles",
     "supplier": "Test supplier", "part_number": "P20-T", "designation": "Profile 20 test L={L}",
     "build": {"role": "profile", "system": $SYSTEM,
       "length": {"parameter": "L", "min": "10 mm", "max": "3000 mm", "stock": ["1000 mm"]},
       "section": {"width": "20 mm", "height": "20 mm"},
       "slot": {"depth": "6 mm", "opening": "5.2 mm", "inner_width": "11 mm"}, "mass_per_metre": "0.4 kg",
       "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
                  {"name": "slot_x+_1", "kind": "slot"}, {"name": "slot_x-_1", "kind": "slot"},
                  {"name": "slot_y+_1", "kind": "slot"}, {"name": "slot_y-_1", "kind": "slot"}]}},
    {"id": "bracket-20-test", "path": "connectors/bracket-20-test.mitcad", "name": "Bracket 20 test",
     "category": "connectors", "supplier": "Test supplier", "part_number": "B20-T",
     "build": {"role": "connector", "systems": [$SYSTEM],
       "frames": [{"name": "mount_a", "kind": "face"}, {"name": "mount_b", "kind": "face"}],
       "connector": {"junctions": ["L", "T"], "space": "inner_corner",
                     "mates": [{"frame": "mount_a", "to": "slot"}, {"frame": "mount_b", "to": "slot"}],
                     "fasteners": [], "machining": []}}},
    {"id": "panel-test", "path": "panels/panel-test.mitcad", "name": "Panel test", "category": "panels",
     "build": {"role": "panel", "frames": [{"name": "face", "kind": "face"}],
               "panel": {"mount": "slot", "thickness": "t", "width": "W", "height": "H", "clearance": "0.5 mm"}}},
    {"id": "rail-test", "path": "linear/rail-test.mitcad", "name": "Rail 15 test", "category": "linear",
     "supplier": "Test supplier", "part_number": "R15-T", "designation": "Rail 15 test L={L}",
     "build": {"role": "linear", "systems": [$SYSTEM],
       "length": {"parameter": "L", "min": "20 mm", "max": "2000 mm", "stock": ["1000 mm"]},
       "section": {"width": "15 mm", "height": "15 mm"},
       "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"}, {"name": "rail", "kind": "rail"},
                  {"name": "base", "kind": "face"}],
       "linear": {"kind": "rail"}}},
    {"id": "carriage-test", "path": "linear/carriage-test.mitcad", "name": "Carriage 15 test", "category": "linear",
     "supplier": "Test supplier", "part_number": "C15-T",
     "build": {"role": "linear", "frames": [{"name": "rail", "kind": "rail"}],
       "linear": {"kind": "carriage", "mate": {"frame": "rail", "on": "rail", "joint": "slider", "axis": "x"}}}}
  ]
}
EOF
g "$LIB" add -A
g "$LIB" commit -q -m "Test parts"
g "$LIB" tag v1.0.0
"$CLI" library fetch "$LIB" > /dev/null || ui_fail "library fetch"
# A design with only its root, the active component.
printf '[]\n' > "$WORK/empty.json"
"$CLI" run "$WORK/empty.json" --save "$WORK/start.mitcad" > /dev/null || ui_fail "the empty design"

# Placed parts as the log names them: "Placed <part> as <name> (<occurrence>), ...".
placed_occurrence() { sed -n 's/^Placed .* as .* (\([O0-9/]*\)), length.*/\1/p' "$UI_LOG" | tail -1; }
placed_name() { sed -n 's/^Placed .* as \(.*\) ([O0-9/]*), length.*/\1/p' "$UI_LOG" | tail -1; }
# The body the placement logged last ("New body <name> (<uid>): volume ...").
placed_body() { sed -n 's/^New body .* (\(F[0-9]*\.b[0-9]*\)): volume.*/\1/p' "$UI_LOG" | tail -1; }
# faces_of BODY TEXT: the faces of a body a click can pick now (after the
# last "Pick places:"), whose names contain the text, one per line: a
# derived part's sides are "<F>:part(F2:side(c1[c4,c2]))", its ends
# "<F>:end_a", "<F>:end_b".
faces_of() {
  local start
  start=$(grep -n "Pick places:" "$UI_LOG" | tail -1 | cut -d: -f1)
  tail -n +"${start:-1}" "$UI_LOG" | grep -F "Pick face $1/" |
    sed -n 's/^Pick face [^/]*\/\(.*\) at [0-9]*,[0-9]*$/\1/p' | grep -F -- "$2"
}
pick_face() { ui_click_pick "$1/$2 at"; }
# refresh_picks COMMAND: the view fitted to the parts, and the places a
# click picks logged again (the panel's pick input activated again).
refresh_picks() {
  ui_step "fit the view (F6)"              ui_key F6
  ui_step "$1's pick again"                ui_click_logged "Panel $1 input at"
}
SIDE=":part(F2:side("
# expect_row INPUT shown|hidden description: Profile's panel row INPUT as
# the panel laid it out last ("Panel Profile input <id> at x,y", with
# " (hidden)" when hidden).
expect_row() {
  local line=""
  for _ in $(seq 1 30); do
    line=$(grep -F "Panel Profile input $1 at " "$UI_LOG" | tail -1)
    if [ "$2" = hidden ]; then
      [[ $line == *"(hidden)" ]] && { echo "ok   $3"; return; }
    else
      [ -n "$line" ] && [[ $line != *"(hidden)" ]] && { echo "ok   $3"; return; }
    fi
    sleep 0.2
  done
  ui_fail "$3: Panel Profile input $1 is not $2 (${line:-never laid out})"
}
# no_policy_message: no design-policy refusal reached the log.
no_policy_message() {
  ! grep -q "does not allow" "$UI_LOG" || ui_fail "a policy refusal: $(grep "does not allow" "$UI_LOG" | head -1)"
  echo "ok   no policy message"
}

ui_start_display
# A new design (mitcad#394): Part1 active, the root with the Assembly policy.
ui_start_app --no-native-dialogs

echo "--- 1. The BUILD tab"
ui_expect_log "Ribbon tab BUILD at" "the BUILD tab beside SOLID"
ui_mark
ui_step "the BUILD tab"                    ui_click_logged "Ribbon tab BUILD"
ui_expect_new "Ribbon BUILD/PROFILE at" "its PROFILE group"
ui_expect_new "Ribbon BUILD/OUTPUT at" "its OUTPUT group"

echo "--- 2. The first profiles of a new design, Part1 active"
ui_mark
ui_step "Profile (command search)"         ui_command "Profile"
ui_expect_new "Command Profile started" "Profile started"
ui_expect_new "BUILD parts profile: 1 (0 in the design)" "the picker lists the library's profile"
ui_expect_new "BUILD profile adds to Root (a ground placement)" "BUILD adds to the root while Part1 is active"
ui_expect_new "View hint Profile: Click a plane or a face, or press Enter" "the view's hint for the first part"
ui_expect_new "Inspect Profile: place grounded, length 1000.00 mm (typed), square ends; adds to Root" \
  "at the origin with its stock length, adding to Root"
expect_row axis=z shown "the Direction's X, Y and Z buttons"
expect_row mitre hidden "no Mitre before an end pick"
expect_row use_offset hidden "no Offset before a side pick"
expect_row rotate hidden "no Rotate before another part is picked"
expect_row place hidden "Placement under More"
ui_mark
ui_step "on the root's XY plane"           ui_click_logged "Datum xy"
ui_expect_new "Inspect Profile: place geometry, length 1000.00 mm (typed), square ends; adds to Root" \
  "on the XY plane, without activating Root"
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Profile 20 test as " "placed by a pick on the XY plane" 30
no_policy_message
ui_step "Close (Escape)"                   ui_key Escape
ui_mark
ui_step "undo it (Ctrl+Z)"                 ui_key ctrl+z
ui_sync
ui_mark
ui_step "Profile again"                    ui_command "Profile"
ui_expect_new "BUILD profile adds to Root (a ground placement)" "a new design again"
ui_step "60 mm long"                       ui_type_in "Panel Profile input length" "60"
ui_mark
ui_step "along X"                          ui_click_logged "Panel Profile input axis=x"
ui_expect_new "Profile: Direction = Along X" "the X button"
ui_expect_new "; along x" "the preview along X"
ui_mark
ui_step "standing (Z)"                     ui_click_logged "Panel Profile input axis=z"
ui_expect_new "; along z" "the preview standing"
ui_mark
ui_step "Place without a pick (Enter)"     ui_key Return
ui_expect_new "Placed Profile 20 test as " "placed at the origin by Enter" 30
ui_expect_new "Applied Profile (1)" "the panel stays open"
no_policy_message
B1=$(placed_body)
[ -n "$B1" ] || ui_fail "no body of the first profile"
ui_expect_new "Profile Place On takes now: faces" "only a profile's faces once one is there"
ui_expect_new "Construction geometry hidden for the command" "the planes hidden for side and end placements"
ui_expect_new "View hint Profile: Click a profile's side or end" "the view's hint for the next one"
refresh_picks "Profile"
SIDE0=$(faces_of "$B1" "$SIDE" | head -1)
[ -n "$SIDE0" ] || ui_fail "no side of the first profile ($B1) to pick"
ui_mark
ui_step "its side, 60 mm long"             pick_face "$B1" "$SIDE0"
ui_expect_new "Inspect Profile: place side" "on the side"
ui_expect_new "; snap end_" "flush with the end near the click"
expect_row use_offset shown "Offset for a side pick"
expect_row mitre hidden "no Mitre for a side pick"
expect_row rotate shown "Rotate against another part"
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Profile 20 test as " "the second profile placed flush" 30
refresh_picks "Profile"
TOP0=$(faces_of "$B1" ":end_b" | head -1)
[ -n "$TOP0" ] || ui_fail "no end face of the first profile ($B1) to pick"
ui_mark
ui_step "its end face"                     pick_face "$B1" "$TOP0"
ui_expect_new "Inspect Profile: place end" "on the end"
expect_row mitre shown "Mitre for an end pick"
expect_row use_offset hidden "no Offset for an end pick"
ui_mark
ui_step "mitre the new part"               ui_choose "Panel Profile input mitre" 1
ui_expect_new "ends A 45 deg" "the new part's end mitred"
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Profile 20 test as " "the third profile placed on the end, mitred" 30
no_policy_message
ui_mark
ui_step "Close (Escape)"                   ui_key Escape
ui_expect_new "Closed Profile after 3" "Close keeps them"
ui_expect_new "Construction geometry shown again" "the planes shown again when the command ends"
ui_stop_app

echo "--- 2b. The same panel in the Floating chrome"
ui_start_app --no-native-dialogs --chrome=floating
ui_mark
ui_step "Profile (command search)"         ui_command "Profile"
ui_expect_new "BUILD profile adds to Root (a ground placement)" "a new design: adds to the root"
expect_row axis=x shown "the X button in a floating card"
expect_row more shown "More in a floating card"
expect_row place hidden "Placement closed under More"
ui_step "More"                             ui_click_logged "Panel Profile input more"
expect_row place shown "Placement under More, opened"
ui_mark
ui_step "Place without a pick (Enter)"     ui_key Return
ui_expect_new "Placed Profile 20 test as " "placed at the origin by Enter" 30
no_policy_message
ui_step "Close (Escape)"                   ui_key Escape
ui_stop_app

# The rest on a design with only its root, the active component.
ui_start_app --open "$WORK/start.mitcad" --no-native-dialogs
ui_step "the BUILD tab"                    ui_click_logged "Ribbon tab BUILD"

echo "--- 3. Profile: a frame of four"
ui_mark
ui_step "Profile (command search)"         ui_command "Profile"
ui_expect_new "Command Profile started" "Profile started"
ui_expect_new "BUILD profile adds to Root (a ground placement)" "the root, the active component"
ui_step "a typed length of 400"            ui_type_in "Panel Profile input length" "400"
ui_expect_new "BUILD length typed: detached from the auto length" "typing a length detaches it"
ui_step "on the XY plane"                  ui_click_logged "Datum xy"
ui_expect_new "Inspect Profile: place " "the preview"
ui_mark
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Profile 20 test as " "the first profile placed" 30
ui_expect_new "Applied Profile (1)" "the panel stays open"
P1=$(placed_occurrence)
B1=$(placed_body)
[ -n "$P1" ] && [ -n "$B1" ] || ui_fail "no occurrence or body of the first profile"
echo "ok   the first profile is $P1 ($B1)"

# Two profiles on the first one's side, flush with its ends: offset 0 from
# A, and from B.
refresh_picks "Profile"
# The two on a side of the first one that a click can reach.
SIDE1=$(faces_of "$B1" "$SIDE" | head -1)
[ -n "$SIDE1" ] || ui_fail "no side of the first profile ($B1) to pick"
ui_mark
ui_step "the first profile's side"         pick_face "$B1" "$SIDE1"
ui_expect_new "Inspect Profile: place side" "on the side"
ui_mark
ui_step "Offset from an End"               ui_click_logged "Panel Profile input use_offset"
ui_step "offset 0"                         ui_type_in "Panel Profile input offset" "0"
ui_step "length 300"                       ui_type_in "Panel Profile input length" "300"
ui_expect_new "snap none at 0.0 mm" "offset 0 from end A: flush with end A"
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Profile 20 test as " "the second profile placed" 30
P2=$(placed_occurrence)
B2=$(placed_body)
refresh_picks "Profile"
ui_mark
ui_step "the first profile's side again"   pick_face "$B1" "$SIDE1"
ui_expect_new "Inspect Profile: place side" "on the side"
ui_mark
ui_step "from end B"                       ui_choose "Panel Profile input from" 1
ui_expect_new "snap none at 380.0 mm" "offset 0 from end B: flush with end B"
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Profile 20 test as " "the third profile placed" 30
P3=$(placed_occurrence)
B3=$(placed_body)
P3NAME=$(placed_name)
echo "ok   the second and third profiles are $P2 ($B2) and $P3 ($B3, $P3NAME), on $SIDE1"

# The fourth: on the second one's side that faces the third, at its far
# end, its length bound to the third.
ui_step "the auto length again (Tab)"      ui_key Tab
ui_expect_new "Profile: Tab steps Length" "Tab steps the auto length"
refresh_picks "Profile"
found=""
mapfile -t FACES2 < <(faces_of "$B2" "$SIDE")
for face in "${FACES2[@]}"; do
  ui_mark
  ui_step "the second profile's $face"     pick_face "$B2" "$face"
  for _ in $(seq 1 25); do
    tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -q "^Inspect Profile: " && break
    sleep 0.2
  done
  # Bound to the third: "(bound, auto: to the face to <its face> of <name>)" or "... to <name>)".
  inspect=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "^Inspect Profile: " | tail -1)
  if [[ $inspect == *"(bound, "* && $inspect == *" $P3NAME)"* ]]; then
    found=$face
    break
  fi
done
[ -n "$found" ] || ui_fail "no side of the second profile a click reaches has its auto length to the third"
echo "ok   $found of the second profile runs to the third: bound"
ui_mark
ui_step "to the centre (Tab)"              ui_key Tab
ui_expect_new "Profile: Length = Auto: to the centre" "Tab: to the centre"
ui_step "through (Tab)"                    ui_key Tab
ui_expect_new "Profile: Length = Auto: through" "Tab: through"
ui_step "to the face (Tab)"                ui_key Tab
ui_expect_new "Profile: Length = Auto: to the face" "Tab: to the face again"
ui_expect_new "Cursor box Profile length: " "the auto length in the box at the cursor"
ui_expect_new "(bound, auto: to the face to " "bound to the third profile's face"
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Profile 20 test as " "the fourth profile placed, bound" 30
ui_expect_new "length bound, auto: to the face" "its length is bound"
ui_mark
ui_step "Close (Escape)"                   ui_key Escape
ui_expect_new "Closed Profile after 4" "Close keeps the four profiles"

echo "--- 4. Connector"
ui_step "fit the view (F6)"                ui_key F6
ui_mark
ui_step "Connector (command search)"       ui_command "Connector"
ui_expect_new "Command Connector started" "Connector started"
ui_expect_new "BUILD connectors for junction " "the connectors of a junction asked"
ui_expect_new "Inspect Connector: junction " "the junction and its connectors"
ui_expect_new ", preview Bracket 20 test with " "the chosen connector previewed in place"
ui_expect_new "Panel Connector option pictures part: " "the connectors listed with their pictures"
ui_expect_new "BUILD preview image of test-profiles/bracket-20-test: " "its picture asked of the library"
ui_mark
FACE3=$(faces_of "$B3" "$SIDE" | head -1)
[ -n "$FACE3" ] || ui_fail "no side of the third profile ($B3) to hover"
ui_step "the mouse over the third profile" xdotool mousemove $(ui_pick_at "$B3/$FACE3 at")
for _ in $(seq 1 25); do
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qE "^Preview marker Connector [^>]" && break
  sleep 0.2
done
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qE "^Preview marker Connector [^>]" ||
  ui_fail "no junction of the third profile shown on its hover"
echo "ok   the hovered profile's junctions shown"
ui_expect_new "Preview marker Connector > " "the junctions marked, the chosen one first"
ui_mark
ui_step "Connect (Enter)"                  ui_key Return
ui_expect_new "Connected Bracket 20 test at junction " "the connector placed" 30
ui_step "Close (Escape)"                   ui_key Escape

echo "--- 5. Panel"
ui_mark
ui_step "Panel (command search)"           ui_command "Panel"
ui_expect_new "BUILD parts panel: 1" "the picker lists the panel"
ui_step "the frame's inner face"           pick_face "$B1" "$SIDE1"
ui_expect_new "Inspect Panel: panel Panel test" "the panel's preview"
ui_mark
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed panel Panel test as " "the panel placed" 30
ui_step "Close (Escape)"                   ui_key Escape

echo "--- 6. Linear Part: a rail and a carriage"
ui_mark
ui_step "Linear Part (command search)"     ui_command "Linear Part"
ui_expect_new "BUILD parts linear: 2" "the rail and the carriage"
ui_step "search rail"                      ui_type_in "Panel Linear Part input search" "rail"
ui_step "a typed length of 200"            ui_type_in "Panel Linear Part input length" "200"
# Another side of the first profile than the frame's, else that one.
RAILFACE=$(faces_of "$B1" "$SIDE" | grep -vxF -- "$SIDE1" | head -1)
[ -n "$RAILFACE" ] || RAILFACE=$SIDE1
ui_step "the first profile's $RAILFACE"   pick_face "$B1" "$RAILFACE"
ui_expect_new "Inspect Linear Part: place " "the rail's preview"
ui_mark
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Rail 15 test as " "the rail placed" 30
BRAIL=$(placed_body)
ui_step "search carriage"                  ui_type_in "Panel Linear Part input search" "carriage"
refresh_picks "Linear Part"
RAILTOP=$(faces_of "$BRAIL" "$SIDE" | grep -F "side(c3[" | head -1)
[ -n "$RAILTOP" ] || RAILTOP=$(faces_of "$BRAIL" "$SIDE" | head -1)
[ -n "$RAILTOP" ] || ui_fail "no side of the rail ($BRAIL) to pick"
ui_mark
ui_step "the rail's $RAILTOP"              pick_face "$BRAIL" "$RAILTOP"
ui_expect_new "Inspect Linear Part: " "the carriage's preview"
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Carriage 15 test as " "the carriage placed on the rail" 30
ui_step "Close (Escape)"                   ui_key Escape

echo "--- 6b. Ends: an end of the third profile mitred and bored"
ui_step "fit the view (F6)"                ui_key F6
ui_mark
ui_step "Ends (command search)"            ui_command "Ends"
ui_expect_new "Command Ends started" "Ends started"
END3=$({ faces_of "$B3" ":end_b"; faces_of "$B3" ":end_a"; faces_of "$B3" "$SIDE"; } | head -1)
[ -n "$END3" ] || ui_fail "no face of the third profile ($B3) to pick"
ui_step "the third profile's $END3"        pick_face "$B3" "$END3"
ui_expect_new "Inspect Ends: " "the part's ends"
ui_step "a bore in the end"                ui_choose "Panel Ends input machining" 1
ui_mark
ui_step "OK (Enter)"                       ui_key Return
ui_expect_new "Ends of " "the end edited"
ui_expect_new "machining D6.8" "with its bore"

echo "--- 7. Save, reopen, Cut List"
ui_mark
ui_step "save as (Ctrl+Shift+S)"           ui_key ctrl+shift+s
ui_type_path "Save As" "$FILE"
ui_focus_main
ui_expect_new "Saved $FILE" "saved"
ui_stop_app
ui_start_app --open "$FILE" --no-native-dialogs
ui_expect_log "Opened $FILE" "reopened"
ui_mark
ui_step "Profile (command search)"         ui_command "Profile"
ui_expect_new "BUILD parts profile: 1 (1 in the design)" "the profile's source is in the design"
ui_step "Close (Escape)"                   ui_key Escape
ui_mark
ui_step "Cut List (command search)"        ui_command "Cut List"
ui_focus_dialog '^Cut List$'
ui_expect_new "Cut list row: " "the cut list's rows"
ui_expect_new "x Profile 20 test" "the profiles in it"
ui_expect_new " 45 deg (" "the mitred end"
ui_expect_new "x Rail 15 test" "the rail in it"
ui_expect_new "machining D6.8" "the bored end"
rows=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | sed -n 's/^Cut list: \([0-9]*\) row(s).*/\1/p' | tail -1)
[ "${rows:-0}" -ge 3 ] || ui_fail "the cut list has ${rows:-no} rows, not three or more"
echo "ok   $rows rows: the profiles by length and ends, and the rail"
ui_mark
ui_step "Copy as CSV (Alt+C)"              ui_key alt+c
ui_expect_new "Build list CSV: Quantity,Part,Length (mm),End A,End B,Machining,Own features,Supplier,Part number,Mass each (kg)" \
  "the CSV's header"
ui_expect_new ",Test supplier,P20-T," "the profiles' supplier and part number"
ui_expect_new "Build list copied as CSV: " "copied"
ui_step "Save as PDF (Alt+P)"              ui_key alt+p
ui_type_path "Save Cut List as PDF" "$PDF"
ui_focus_dialog '^Cut List$'
ui_expect_new "Build list PDF written: $PDF (" "the PDF written"
ui_expect_new "Cut list of frame" "with the design's name"
[ "$(head -c 5 "$PDF")" = "%PDF-" ] || ui_fail "$PDF is no PDF"
echo "ok   $PDF is a PDF"
ui_step "close (Escape)"                   ui_key Escape
ui_focus_main

ui_stop_app
ui_finish "the BUILD tab"
