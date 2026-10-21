#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/build app/commands/BuildCommands.cpp app/framework app/library app/OcctViewer.cpp
# check-all sources: core/model/src/build core/model/src/api core/vcs/src/library tools/cli
# The Timber tool's Frame mode (mitcad#479) through the real UI, with a
# small library of synthetic parts made here (mitcad-cli: a 48 x 98 timber
# member and a wood screw; no supplier's files), fetched from a local git
# repository:
#   1. Timber, Mode Frame: a wall 3600 x 2400 from the origin along +X,
#      studs at 600, previewed with its members counted (the screws found
#      in the library).
#   2. A window typed into Openings (at 600, 900 x 1200, sill at 900): its
#      king, trimmer and cripple studs, header and sill in the preview.
#   3. Enter places the frame: one build_frame, its members and two screws
#      at each junction; the wall's members in the design.
#   4. Cut List: the frame's members with the other timber, grouped by size
#      (the five studs and the two king studs one row).
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-timber-frame-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CLI=${UI_CLI:-$(dirname "$UI_APP")/../tools/cli/mitcad-cli}
mkdir -p "$ROOT/build"
WORK=$(mktemp -d "$ROOT/build/ui-timber-frame.XXXXXX")
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
export MITCAD_LIBRARIES_DIR=$WORK/cache
LIB=$WORK/test-timber
g() { git -C "$1" -c user.name=T -c user.email=t@example.invalid "${@:2}" > /dev/null; }

RECT='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
# frame NAME ORIGIN Z X: a mount frame (docs/libraries.md, "BUILD parts").
frame() {
  printf '  {"cmd": "add_feature", "name": "mount:%s", "def": {"type": "joint_origin", "geometry": "origin",
    "frame_override": {"origin": %s, "z_axis": %s, "x_axis": %s}}},\n' "$1" "$2" "$3" "$4"
}
# member W T: a timber member W wide (along X) and T thick (along Y),
# centred on the Z axis, extruded by L from z = 0, with its end frames and
# a face frame on each side (docs/libraries.md, "Timber parts").
member() {
  local hw ht
  hw=$(awk -v s="$1" 'BEGIN { print s / 2 }')
  ht=$(awk -v s="$2" 'BEGIN { print s / 2 }')
  printf '[\n  {"cmd": "add_parameter", "name": "L", "expression": "600 mm"},\n'
  printf '  {"cmd": "sketch.create", "plane": "xy"},\n'
  printf '  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-%s, -%s], "width": %s, "height": %s},\n' \
    "$hw" "$ht" "$1" "$2"
  printf '  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "%s"}],
    "extent": {"type": "distance", "distance": "L"}, "operation": "new_body"}},\n' "$RECT"
  frame end_a "[0, 0, 0]" "[0, 0, -1]" "[1, 0, 0]"
  printf '  {"cmd": "add_feature", "name": "mount:end_b", "def": {"type": "joint_origin",
    "geometry": {"body": "F2.b0", "face": "F2:end(%s)"}}},\n' "$RECT"
  frame face_xp "[$hw, 0, 0]" "[1, 0, 0]" "[0, 0, 1]"
  frame face_xn "[-$hw, 0, 0]" "[-1, 0, 0]" "[0, 0, 1]"
  frame face_yp "[0, $ht, 0]" "[0, 1, 0]" "[0, 0, 1]"
  frame face_yn "[0, -$ht, 0]" "[0, -1, 0]" "[0, 0, 1]"
}
design() { # design FILE: the script on stdin saved as a design
  local script=$WORK/$(basename "$1" .mitcad).json
  cat > "$script"
  mkdir -p "$(dirname "$1")"
  "$CLI" run "$script" --save "$1" > /dev/null || ui_fail "mitcad-cli run $(basename "$script") failed"
}

"$CLI" library init "$LIB" --id test-timber --name "Test timber" --license MIT > /dev/null ||
  ui_fail "library init"
{ member 98 48
  printf '  {"expect": {"bodies": 1}}\n]\n'; } | design "$LIB/dimensioned-timber/timber-48-test.mitcad"
# A wood screw: a 5 x 5 shank 70 long along +Z, its axis frame at the head.
{ printf '[\n  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"},
    "corner": [0, 0], "length": 5, "width": 5, "height": 70, "operation": "new_body"}},\n'
  frame axis "[2.5, 2.5, 0]" "[0, 0, 1]" "[1, 0, 0]"
  printf '  {"expect": {"bodies": 1}}\n]\n'; } | design "$LIB/fasteners/wood-screw-test.mitcad"

cat > "$LIB/mitcad-library.json" <<'EOF'
{
  "format": "mitcad-library", "version": 1, "id": "test-timber", "name": "Test timber",
  "description": "Synthetic BUILD parts of the timber frame UI test.", "library_version": "1.0.0", "license": "MIT",
  "authors": ["Mitcad UI test"], "units": "mm",
  "categories": [{"id": "dimensioned-timber", "name": "Dimensioned timber"}, {"id": "fasteners", "name": "Fasteners"}],
  "components": [
    {"id": "timber-48-test", "path": "dimensioned-timber/timber-48-test.mitcad", "name": "Timber 48x98 test",
     "category": "dimensioned-timber", "designation": "Timber 48x98 test L={L}",
     "build": {"role": "profile", "kind": "timber",
       "material": {"species": "spruce", "finish": "dimensioned", "grade": "", "density": 450, "moisture": 20},
       "availability": "standard",
       "section": {"width": "98 mm", "height": "48 mm"}, "nominal": {"width": "98 mm", "height": "48 mm"},
       "length": {"parameter": "L", "min": "100 mm", "max": "6000 mm", "default": "600 mm",
                  "stock": ["3000 mm", "3600 mm", "4200 mm"]},
       "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
                  {"name": "face_xp", "kind": "face"}, {"name": "face_xn", "kind": "face"},
                  {"name": "face_yp", "kind": "face"}, {"name": "face_yn", "kind": "face"}]}},
    {"id": "wood-screw-test", "path": "fasteners/wood-screw-test.mitcad", "name": "Wood screw test",
     "category": "fasteners", "designation": "Wood screw test 5x70",
     "build": {"role": "fastener", "wood": {"kind": "screw", "head": "countersunk", "diameter": "5 mm", "length": "70 mm"},
       "material": {"product": "steel", "density": 7850},
       "frames": [{"name": "axis", "kind": "shaft"}]}}
  ]
}
EOF
g "$LIB" add -A
g "$LIB" commit -q -m "Test parts"
g "$LIB" tag v1.0.0
"$CLI" library fetch "$LIB" > /dev/null || ui_fail "library fetch"

# expect_inspect TEXT... DESCRIPTION: an "Inspect Timber:" line since the
# mark has every TEXT.
expect_inspect() {
  local description=${!#} line text ok
  for _ in $(seq 1 50); do
    while IFS= read -r line; do
      ok=1
      for text in "${@:1:$#-1}"; do
        [[ $line == *"$text"* ]] || ok=0
      done
      [ $ok = 1 ] && { echo "ok   $description"; return; }
    done < <(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "^Inspect Timber: ")
    sleep 0.2
  done
  ui_fail "$description: no 'Inspect Timber:' line since the mark with: ${*:1:$#-1}"
}

ui_start_display
# A new design (mitcad#394): Part1 active.
ui_start_app --no-native-dialogs

echo "--- 1. Timber, Mode Frame: a wall"
ui_mark
ui_step "the BUILD tab"                    ui_click_logged "Ribbon tab BUILD"
ui_expect_new "Ribbon BUILD/PROFILE at" "its PROFILE group"
ui_mark
ui_step "Timber (command search)"          ui_command "Timber"
ui_expect_new "Command Timber started" "Timber started"
ui_expect_new "BUILD parts timber: 1 (0 in the design)" "Timber's picker: the member"
ui_mark
ui_step "Mode: Frame"                      ui_click_logged "Panel Timber input how=frame"
ui_expect_new "BUILD parts wood: 1 (0 in the design)" "the library's wood screw for the frame's junctions"
expect_inspect "frame wall Wall 1, wall 3600 x 2400 mm, 9 member(s)" "1 bottom plate, 7 stud, 1 top plate" \
  "fasteners yes" "the wall from the origin along +X: plates and seven studs at 600, screws at the junctions"

echo "--- 2. A window"
ui_mark
ui_step "a window at 600"                  ui_type_in "Panel Timber input frame_openings" "600 900x1200 900"
expect_inspect "frame wall Wall 1, wall 3600 x 2400 mm, 16 member(s)" \
  "1 bottom plate, 2 cripple stud, 2 header, 2 king stud, 1 sill, 5 stud, 1 top plate, 2 trimmer stud" \
  "the window: kings, trimmers, a header of two plies, a sill and cripple studs"

echo "--- 3. Place"
ui_mark
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed frame Wall 1 (wall, lead C" "the wall frame placed" 60
line=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "^Placed frame Wall 1 " | tail -1)
[[ $line == *": 16 member(s), "* ]] || ui_fail "the frame's members: $line"
fastened=$(sed -n 's/.*member(s), \([0-9]*\) junction(s) fastened$/\1/p' <<< "$line")
[ "${fastened:-0}" -gt 0 ] || ui_fail "no junction fastened: $line"
echo "ok   16 members, $fastened junction(s) with two screws each"
ui_step "close Timber (Escape)"            ui_key Escape

echo "--- 4. Cut List"
ui_mark
ui_step "Cut List (command search)"        ui_command "Cut List"
ui_focus_dialog '^Cut List$'
ui_expect_new "Cut list row: " "the cut list's rows"
ui_expect_new "Cut list timber: " "the timber columns and totals"
ui_expect_new "Cut list timber row: Timber 48x98 test" "the members' material and sizes"
rows=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -E "^Cut list row: 7 x Timber 48x98 test L=2304, 2304(\.0*)? mm" || true)
[ -n "$rows" ] || ui_fail "no row of the seven 2304 mm studs: $(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep '^Cut list row: ')"
echo "ok   the studs and king studs one row of seven, by size"
ui_step "close the lists (Escape)"         ui_key Escape
ui_focus_main

ui_finish "UI timber frame test"
