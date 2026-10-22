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
#   5. Two points from a stud's side to the next stud's (mitcad#509): the
#      sides are not one plane, so the member is free between the points,
#      as long as the studs' distance, its end cut back to the end stud's
#      face (not its end moved onto the start's plane, a member a few
#      millimetres long).
#   6. In a new design, Corner Butt, the default (mitcad#510): a wall, then
#      a wall started by a click on its side near its end A butts from the
#      side there; one by a click on its end B's face (the view from the
#      right) starts from that end, a butt corner.
#   7. In a new design, Corner Lapped plates: a wall started near the
#      first one's end B makes an L corner (mitcad#493); the next Timber
#      start remembers Lapped plates.
#   8. On a member (mitcad#508): the hover over a member's side names only
#      the face a click picks, nothing previewed before the click; once
#      picked, the mouse along the side, off the parts and back previews
#      and inspects nothing, the preview's end A unchanged; a click on
#      another member's side moves it once. Closed without placing.
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
# off_view: the mouse out of the view, over the ribbon above its middle
# (window coordinates of the view: "View area x y w h").
off_view() {
  local area vx vy vw vh
  area=$(grep -o "View area [0-9]* [0-9]* [0-9]* [0-9]*" "$UI_LOG" | tail -1)
  [ -n "$area" ] || ui_fail "the app did not log its view area"
  read -r _ _ vx vy vw vh <<< "$area"
  xdotool mousemove $((X + (vx + vw / 2) * UI_SCALE)) $((Y + vy / 2 * UI_SCALE))
}
# no_inspect DESCRIPTION: a moment after the mouse moved, no "Inspect
# Timber:" line since the mark: the hover runs no placement and no
# inspection (mitcad#508).
no_inspect() {
  sleep 1.5
  ui_sync
  ! tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -q "^Inspect Timber: " ||
    ui_fail "$1: $(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "^Inspect Timber: " | head -1)"
  echo "ok   $1"
}
# last_end_a: where the last "Inspect Timber:" line puts the preview's end
# A ("; end A at x, y, z").
last_end_a() { grep "^Inspect Timber: " "$UI_LOG" | tail -1 | sed -n 's/.*; end A at \([^;]*\);.*/\1/p'; }
# end_a_since: the places of the preview's end A in the "Inspect Timber:"
# lines since the mark, each once.
end_a_since() {
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "^Inspect Timber: " | sed -n 's/.*; end A at \([^;]*\);.*/\1/p' | sort -u
}
# moved_once WAS DESCRIPTION: after a click, the "Inspect Timber:" lines
# since the mark put the preview's end A elsewhere than WAS, all at one
# place: the preview moved once, to the click (mitcad#508).
moved_once() {
  local places=""
  for _ in $(seq 1 30); do
    places=$(end_a_since)
    [ -n "$places" ] && break
    sleep 0.2
  done
  # A moment more: a preview still following the mouse would add places.
  sleep 1
  ui_sync
  places=$(end_a_since)
  [ -n "$places" ] || ui_fail "$2: no 'Inspect Timber:' line with end A since the mark"
  [ "$(printf '%s\n' "$places" | wc -l)" -eq 1 ] || ui_fail "$2: end A at more than one place: $(tr '\n' ' ' <<< "$places")"
  [ "$places" != "$1" ] || ui_fail "$2: end A still at $1"
  echo "ok   $2: end A at $places"
}

# new_bodies: the bodies made since the mark, separated by spaces.
new_bodies() {
  tail -n +$((UI_MARK + 1)) "$UI_LOG" | sed -n 's/^New body .* (\(F[0-9]*\.b[0-9]*\)): volume.*/\1/p' |
    sort -u | tr '\n' ' '
}

# wall_faces TEXT: "x y" of each face of Wall 1's bodies ($WALL1) whose
# name has TEXT that a click on Start picks now (after the last "Pick
# places:"), sorted left to right.
wall_faces() {
  local start line
  start=$(grep -n "Pick places:" "$UI_LOG" | tail -1 | cut -d: -f1)
  tail -n +"${start:-1}" "$UI_LOG" | while IFS= read -r line; do
    if [[ $line =~ ^Pick\ face\ (F[0-9]+\.b[0-9]+)/(.*)\ at\ ([0-9]+),([0-9]+)$ ]] &&
      [[ " $WALL1 " == *" ${BASH_REMATCH[1]} "* && ${BASH_REMATCH[2]} == *"$1"* ]]; then
      echo "${BASH_REMATCH[3]} ${BASH_REMATCH[4]}"
    fi
  done | sort -n
}

# start_picks: the view fitted, the places a click on Start picks logged.
start_picks() {
  ui_step "fit the view (F6)"              ui_key F6
  ui_step "Timber's Start"                 ui_click_logged "Panel Timber input start"
}

# click_place "X Y": a click where a pick place was logged.
click_place() {
  local px py
  read -r px py <<< "$1"
  [ -n "$py" ] || ui_fail "no place to click"
  xdotool mousemove $((X + px * UI_SCALE)) $((Y + py * UI_SCALE)) click 1
}

# wall_from NAME "X Y" LENGTH TEXT CORNER: a wall started by a click at a
# place on Wall 1, LENGTH mm long (typed): the panel names where it starts
# (TEXT) and the Corner (CORNER), and the wall is placed so.
wall_from() {
  ui_mark
  ui_step "the start on Wall 1"            click_place "$2"
  ui_expect_new "View hint Timber: Click where the wall ends, or press Enter: Length square to the wall clicked" \
    "the end next, else Length square to the wall"
  ui_mark
  ui_step "$3 mm long"                     ui_type_in "Panel Timber input frame_length" "$3"
  expect_inspect "frame wall $1, wall $3 x 2400 mm" "; $1: $4" "; corner $5" "$1 previewed: $4, corner $5"
  ui_mark
  ui_step "Place (Enter)"                  ui_key Return
  ui_expect_new "Placed frame $1 (wall, lead C" "$1 placed" 60
  ui_expect_new "Frame junction: $1: $4" "$1 placed: $4"
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
WALL1=$(new_bodies)
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

echo "--- 5. Two points from one stud's side to the next one's (mitcad#509)"
ui_mark
ui_step "Timber (command search)"          ui_command "Timber"
ui_expect_new "Command Timber started" "Timber again"
ui_mark
ui_step "Mode: Two points"                 ui_click_logged "Panel Timber input how=points"
ui_expect_new "View hint Timber: Click where the member starts" "the view's hint for the start"
ui_step "fit (F6)"                         ui_key F6
ui_step "the start pick"                   ui_click_logged "Panel Timber input start"
# The studs and king studs (2304 mm, 98 x 48: 10838016 mm3) and, of each,
# its +X side: of its sides in sight, the one furthest right (the view from
# the front right: +X and +Y to the right).
studs=" $(sed -n -E 's/^New body .* \((F[0-9]+\.b[0-9]+)\): volume ([0-9.]+) mm3$/\2 \1/p' "$UI_LOG" |
  awk '$1 > 10838015 && $1 < 10838017 { print $2 }' | tr '\n' ' ')"
start=$(grep -n "Pick places:" "$UI_LOG" | tail -1 | cut -d: -f1)
sides=$(tail -n +"${start:-1}" "$UI_LOG" |
  sed -n -E 's/^Pick face (F[0-9]+\.b[0-9]+)\/[^ ]*:part\(F2:side\([^ ]* at ([0-9]+),([0-9]+)$/\1 \2 \3/p' |
  awk -v studs="$studs" 'index(studs, " " $1 " ") && (!($1 in x) || $2 > x[$1]) { x[$1] = $2; y[$1] = $3 }
    END { for (b in x) print x[b], y[b] }' | sort -n)
[ "$(wc -l <<< "$sides")" -ge 2 ] || ui_fail "fewer than two studs' sides in sight: $sides"
# The two rightmost: the stud at 3000 and the end stud. Their +X sides are
# not one plane. Before mitcad#509 the end was moved onto the start's
# plane, beside the start: a member a few millimetres long, refused.
read -r ax ay <<< "$(tail -2 <<< "$sides" | head -1)"
read -r bx by <<< "$(tail -1 <<< "$sides")"
ui_mark
ui_step "the start on a stud's side"       xdotool mousemove $((X + ax * UI_SCALE)) $((Y + ay * UI_SCALE)) click 1
ui_expect_new "View hint Timber: Click where it ends; Shift+click places at once" "the end next"
ui_mark
ui_step "the end on the next stud's side"  xdotool mousemove $((X + bx * UI_SCALE)) $((Y + by * UI_SCALE)) click 1
expect_inspect "points, length " "; free" "the member free between the studs' sides"
line=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "^Inspect Timber: points, length " | tail -1)
length=$(sed -n 's/^Inspect Timber: points, length \([0-9.]*\) mm.*/\1/p' <<< "$line")
awk -v l="${length:-0}" 'BEGIN { exit !(l > 450 && l < 700) }' ||
  ui_fail "the member between two studs 576 apart is ${length:-no} mm: $line"
cut_end=', end on [^;]* cut;'
[[ $line =~ $cut_end ]] || ui_fail "its end not cut back to the stud's face: $line"
echo "ok   $length mm between the studs, its end on the end stud's face"
ui_mark
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed Timber 48x98 test as " "the member placed" 30
ui_expect_new "length by two points, " "a member by two points"
ui_step "close Timber (Escape)"            ui_key Escape

echo "--- 6. Butting walls (mitcad#510): from Wall 1's side near its end A, from its end B's face"
# A new design, the orientation cube's places logged (the view from the
# right for the end face).
ui_stop_app
export MITCAD_LOG_ORIENTATION_CUBE=1
ui_start_app --no-native-dialogs
ui_mark
ui_step "the BUILD tab"                    ui_click_logged "Ribbon tab BUILD"
ui_expect_new "Ribbon BUILD/PROFILE at" "its PROFILE group"
ui_mark
ui_step "Timber (command search)"          ui_command "Timber"
ui_expect_new "Command Timber started" "Timber started"
ui_mark
ui_step "Mode: Frame"                      ui_click_logged "Panel Timber input how=frame"
expect_inspect "frame wall Wall 1, wall 3600 x 2400 mm" "; corner butt" "a wall, Corner Butt by default"
ui_mark
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed frame Wall 1 (wall, lead C" "the wall frame placed" 60
WALL1=$(new_bodies)
start_picks
# Its leftmost side face is near its end A (+X to the right in the view).
side=$(wall_faces ":part(F2:side(" | head -1)
wall_from "Wall 2" "$side" 3000 "from Wall 1's side at " butt
at=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" |
  sed -n "s/^Frame junction: Wall 2: from Wall 1's side at \([0-9]*\) mm (butt)$/\1/p" | tail -1)
[ -n "$at" ] || ui_fail "Wall 2 placed without its butt from the side"
[ "$at" -le 600 ] || ui_fail "Wall 2 starts $at mm along Wall 1, not near its end A"
echo "ok   Wall 2 butts on Wall 1's side $at mm from its end A"
# Seen from the right (+X), only Wall 1's end B shows of it: the plates'
# ends (too small to pick from the front right) first, else the end stud.
ui_step "the view from the right"          ui_click_logged "Orientation cube right"
start_picks
end=$( { wall_faces ":end_b"; wall_faces ""; } | head -1)
wall_from "Wall 3" "$end" 2000 "from Wall 1's end B (butt corner)" butt
ui_step "close Timber (Escape)"            ui_key Escape

echo "--- 7. Lapped plates (mitcad#493, mitcad#510): an L corner near Wall 1's end B, remembered"
ui_stop_app
unset MITCAD_LOG_ORIENTATION_CUBE
ui_start_app --no-native-dialogs
ui_mark
ui_step "the BUILD tab"                    ui_click_logged "Ribbon tab BUILD"
ui_expect_new "Ribbon BUILD/PROFILE at" "its PROFILE group"
ui_mark
ui_step "Timber (command search)"          ui_command "Timber"
ui_expect_new "Command Timber started" "Timber started"
ui_mark
ui_step "Mode: Frame"                      ui_click_logged "Panel Timber input how=frame"
expect_inspect "frame wall Wall 1, wall 3600 x 2400 mm" "; corner butt" "a wall, Corner Butt the first time"
ui_mark
ui_step "Corner: Lapped plates"            ui_choose "Panel Timber input frame_corner" 1
expect_inspect "frame wall Wall 1, wall 3600 x 2400 mm" "; corner lapped" "Corner Lapped plates"
ui_mark
ui_step "Place (Enter)"                    ui_key Return
ui_expect_new "Placed frame Wall 1 (wall, lead C" "the wall frame placed" 60
WALL1=$(new_bodies)
start_picks
# Its rightmost side face is near its end B.
side=$(wall_faces ":part(F2:side(" | tail -1)
wall_from "Wall 2" "$side" 3000 "an L corner at Wall 1's end B (lapped plates)" lapped
ui_step "close Timber (Escape)"            ui_key Escape
ui_mark
ui_step "Timber (command search)"          ui_command "Timber"
ui_expect_new "Command Timber started" "Timber started again"
expect_inspect "frame wall Wall 3, " "; corner lapped" "the next start remembers Lapped plates"
ui_step "close Timber (Escape)"            ui_key Escape

echo "--- 8. On a member: the preview stays where the click put it"
ui_mark
ui_step "Timber (command search)"          ui_command "Timber"
ui_expect_new "Command Timber started" "Timber again"
ui_step "Mode: On a member"                ui_click_logged "Panel Timber input how=member"
ui_step "fit the view (F6)"                ui_key F6
ui_mark
ui_step "Timber's Place On"                ui_click_logged "Panel Timber input at"
ui_expect_new "Pick places:" "the places a click picks"
# Two sides of the wall's members a click can reach, of two members.
start=$(grep -n "Pick places:" "$UI_LOG" | tail -1 | cut -d: -f1)
sides=$(tail -n +"${start:-1}" "$UI_LOG" | grep -F "Pick face " | grep -F ":part(F2:side(" |
  sed -n 's/^Pick face \(.*\) at [0-9]*,[0-9]*$/\1/p')
PICK1=$(printf '%s\n' "$sides" | sed -n 1p)
[ -n "$PICK1" ] || ui_fail "no side of the wall's members to pick"
BODY1=${PICK1%%/*}
FACE1=${PICK1#*/}
PICK2=$(printf '%s\n' "$sides" | grep -vF -- "$BODY1/" | head -1)
[ -n "$PICK2" ] || PICK2=$(printf '%s\n' "$sides" | grep -vxF -- "$PICK1" | head -1)
[ -n "$PICK2" ] || ui_fail "no second side of the wall's members to pick"
AT1=$(ui_pick_at "$PICK1 at")
# Before the click the hover names the face a click picks and previews
# nothing (mitcad#508).
ui_sync
ui_mark
ui_step "the mouse over a member's side"   xdotool mousemove $AT1
ui_expect_new "BUILD hover pick: face $FACE1 of $BODY1" "the hover names the face a click picks"
ui_step "the mouse a little further"       xdotool mousemove_relative -- 6 0
no_inspect "no preview and no inspection while hovering before the click"
ui_mark
ui_step "a click on the side"              xdotool mousemove $AT1 click 1
expect_inspect "place side" "; end A at " "the member on the side picked"
# Once picked the preview stays there: the mouse along the side, off the
# parts and back runs no placement and no inspection, its end A unchanged;
# a click on another member's side moves it once (mitcad#508).
ui_sync
END_A1=$(last_end_a)
[ -n "$END_A1" ] || ui_fail "no end A in the side pick's inspection"
ui_mark
ui_step "the mouse along the side"         xdotool mousemove_relative -- 12 0
ui_step "the mouse off the parts"          off_view
ui_expect_new "BUILD hover pick: off the parts' faces" "the hover off the parts"
ui_step "the mouse on the side again"      xdotool mousemove $AT1
no_inspect "no preview and no inspection while the mouse moves after the click"
[ "$(last_end_a)" = "$END_A1" ] || ui_fail "the preview's end A moved from $END_A1"
echo "ok   the preview's end A still at $END_A1"
ui_mark
ui_step "a click on another member's side" ui_click_pick "$PICK2 at"
moved_once "$END_A1" "a click on another member's side moves the preview once"
ui_step "close Timber (Escape)"            ui_key Escape

ui_finish "UI timber frame test"
