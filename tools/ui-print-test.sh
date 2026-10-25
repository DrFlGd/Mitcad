#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/files
# check-all sources: app/framework/Command.hpp app/framework/CommandSession.cpp app/OcctViewer.cpp
# 3D Print (mitcad#13; its panel mitcad#234) through the real UI, with a
# fake slicer: a script that writes the arguments it was started with, so
# no slicer runs.
#   1. Nothing selected: the three visible bodies as an STL file each
#      (named after the bodies) in <temp>/mitcad-print/<design>, one start
#      of the slicer with the three files.
#   2. The format 3MF (the panel's Format): one file, the folder emptied
#      first (the STL files are gone); the file holds one object of three
#      parts named after the bodies, in their places and with the
#      appearance's colour, and the parts' volumes are the bodies' within
#      the refinement's deviation.
#   3. A body selected: only it is in the panel and sent. Cancel (Esc)
#      sends nothing.
#   4. Preferences' 3D Print group keeps the slicer.
#   5. A body in a component placed twice (mitcad#17): an STL file per
#      occurrence, named with it, each with the body where the design
#      shows it.
#   6. A mesh body in a moved and turned occurrence (mitcad#20): the file
#      opens, the view shows the mesh where it shows the solid it was made
#      of (placed the same), and its STL file has it there.
#   7. Bodies picked in the panel (mitcad#234): with nothing selected the
#      shown solids are in Bodies and highlighted, a surface body left
#      out; a click on a body takes it and its highlight out, a click on
#      the surface body picks nothing, and Send writes the files of the
#      bodies left only.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-print-test.sh
# check-all sources: core/3mf core/zip tools/cli

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-print.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
FILE=$WORK/parts.mitcad
# The app's temporary folder, where the files for the slicer go.
export TMPDIR=$WORK/tmp
mkdir -p "$TMPDIR"
FOLDER=$TMPDIR/mitcad-print/parts
ARGS=$WORK/slicer-args.txt

# The fake slicer: one line per argument, a run after another.
cat > "$WORK/fake-slicer.sh" << EOF
#!/bin/sh
printf '%s\n' "\$@" > "$ARGS.part"
mv "$ARGS.part" "$ARGS"
EOF
chmod +x "$WORK/fake-slicer.sh"
mkdir -p "$XDG_CONFIG_HOME/Mitcad"
# STL to start with (3MF is the first time's default, mitcad#655).
printf '[print]\nslicerName=Fake slicer\nslicerProgram=%s\nformat=stl\n' "$WORK/fake-slicer.sh" \
  > "$XDG_CONFIG_HOME/Mitcad/Mitcad.conf"

# A 20 x 10 x 5 plate, a 10 mm cube in red and a pin of 10 mm diameter, 20
# high: 1000, 1000 and 1570.796 mm3.
cat > "$WORK/model.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
    "length": 20, "width": 10, "height": 5, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [30, 0],
    "length": 10, "width": 10, "height": 10, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "cylinder", "plane": "xy", "center": [55, 5],
    "diameter": 10, "height": 20, "operation": "new_body"}},
  {"cmd": "rename_body", "uid": "F1.b0", "name": "Plate"},
  {"cmd": "rename_body", "uid": "F2.b0", "name": "Cube"},
  {"cmd": "rename_body", "uid": "F3.b0", "name": "Pin"},
  {"cmd": "set_body_appearance", "uid": "F2.b0", "appearance": "paint_red"}
]
EOF
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }

# slicer_ran: waits for the fake slicer's arguments of this run.
slicer_ran() {
  for _ in $(seq 1 50); do
    [ -f "$ARGS" ] && return 0
    sleep 0.2
  done
  ui_fail "the slicer was not started"
}

# send [entries]: 3D Print through the search, the panel's Format that many
# entries down (negative: up) when given, then Send (Enter).
send() {
  rm -f "$ARGS"
  ui_mark
  ui_command "3D Print"
  ui_expect_new "Panel 3D Print input format at " "the panel"
  if [ $# -gt 0 ]; then
    ui_step "format"                     ui_choose "Panel 3D Print input format" "$1"
  fi
  ui_key Return
}

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                       ui_key F6

echo "--- Nothing selected: an STL file per visible body"
ui_step "3D print, STL (Enter)"          send
ui_expect_new "3D Print panel: 3 visible: Plate, Cube, Pin; format stl, refinement high, slicer Fake slicer" \
  "the panel: the visible bodies, STL and High by default, the slicer of the settings"
ui_expect_new "3D Print: 3 bodies as stl (high) to $FOLDER: Plate.stl, Cube.stl, Pin.stl" "three STL files"
ui_expect_new "3D Print: started $WORK/fake-slicer.sh $FOLDER/Plate.stl $FOLDER/Cube.stl $FOLDER/Pin.stl" \
  "one start of the slicer"
slicer_ran
[ "$(cat "$ARGS")" = "$(printf '%s\n' "$FOLDER/Plate.stl" "$FOLDER/Cube.stl" "$FOLDER/Pin.stl")" ] ||
  ui_fail "the slicer's arguments: $(cat "$ARGS")"
echo "ok   the slicer got the three files"
for name in Plate Cube Pin; do
  # Binary STL: 84 bytes of header and count, 50 per triangle (a box 12).
  [ "$(stat -c %s "$FOLDER/$name.stl")" -ge 684 ] || ui_fail "$name.stl is too small"
done
echo "ok   the files are there"

echo "--- 3MF: one object with three parts"
ui_step "3D print, 3MF (Format, Enter)"  send 1
ui_expect_new "3D Print panel: 3 visible: Plate, Cube, Pin; format stl" "the panel"
ui_expect_new "3D Print: Format = 3MF" "3MF chosen"
ui_expect_new "3D Print: Format = 3MF" "3MF chosen"
ui_expect_new "3D Print: 3 bodies as 3mf (high) to $FOLDER: Plate " "a 3MF file"
slicer_ran
[ "$(cat "$ARGS")" = "$FOLDER/parts.3mf" ] || ui_fail "the slicer's arguments: $(cat "$ARGS")"
echo "ok   the slicer got the 3MF file"
[ "$(ls "$FOLDER")" = "parts.3mf" ] || ui_fail "the folder was not emptied: $(ls "$FOLDER")"
echo "ok   the STL files of the last send are gone"
python3 - "$FOLDER/parts.3mf" << 'EOF' || ui_fail "the 3MF file"
import sys, zipfile, xml.etree.ElementTree as ET
ns = {"m": "http://schemas.microsoft.com/3dmanufacturing/core/2015/02"}
archive = zipfile.ZipFile(sys.argv[1])
assert "3D/3dmodel.model" in archive.read("_rels/.rels").decode()
model = ET.fromstring(archive.read("3D/3dmodel.model"))
assert model.get("unit") == "millimeter"
items = model.findall("m:build/m:item", ns)
assert len(items) == 1, items
objects = {o.get("id"): o for o in model.findall("m:resources/m:object", ns)}
assembly = objects[items[0].get("objectid")]
assert assembly.get("name") == "parts"
components = assembly.findall("m:components/m:component", ns)
parts = [objects[c.get("objectid")] for c in components]
assert [p.get("name") for p in parts] == ["Plate", "Cube", "Pin"], [p.get("name") for p in parts]
bases = model.findall("m:resources/m:basematerials/m:base", ns)
assert [(b.get("name"), b.get("displaycolor")) for b in bases] == [("Cube", "#C82828FF")], bases
assert parts[1].get("pindex") == "0" and parts[0].get("pid") is None
expected = [1000.0, 1000.0, 3.141592653589793 * 25 * 20]
areas = [2 * (20 * 10 + 20 * 5 + 10 * 5), 600.0, 3.141592653589793 * 10 * 20 + 2 * 3.141592653589793 * 25]
for part, volume, area in zip(parts, expected, areas):
    vertices = [[float(v.get(a)) for a in "xyz"] for v in part.findall("m:mesh/m:vertices/m:vertex", ns)]
    triangles = [[int(t.get(a)) for a in ("v1", "v2", "v3")] for t in part.findall("m:mesh/m:triangles/m:triangle", ns)]
    edges = {}
    six = 0.0
    for t in triangles:
        a, b, c = (vertices[i] for i in t)
        six += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
        for k in range(3):
            edges[(t[k], t[(k + 1) % 3])] = edges.get((t[k], t[(k + 1) % 3]), 0) + 1
    assert all(n == 1 and edges.get((b, a)) == 1 for (a, b), n in edges.items()), part.get("name") + " is not closed"
    print(f"     {part.get('name')}: {len(triangles)} triangles, {six / 6:.4f} mm3 of {volume:.4f}")
    assert abs(six / 6 - volume) <= area * 0.01, part.get("name")
# The bodies' places: the pin's vertices around (55, 5).
pin = [[float(v.get(a)) for a in "xy"] for v in parts[2].findall("m:mesh/m:vertices/m:vertex", ns)]
assert abs(sum(p[0] for p in pin) / len(pin) - 55) < 0.5 and abs(sum(p[1] for p in pin) / len(pin) - 5) < 0.5
EOF
echo "ok   one object of three closed parts, named, coloured, in place, of the bodies' volumes"

echo "--- A selected body; Cancel"
ui_step "select the cube"                ui_click_logged "Body F2.b0"
ui_expect_new "of F2.b0]" "something of the cube selected"
ui_step "3D print (Enter)"               send
ui_expect_new "3D Print panel: 1 selected: Cube; format 3mf" "the panel: the selected body, 3MF remembered"
ui_expect_new "3D Print: 1 bodies as 3mf (high) to $FOLDER: Cube " "the cube alone"
slicer_ran
rm -f "$ARGS"
ui_mark
ui_step "3D print (search)"              ui_command "3D Print"
ui_expect_new "Command 3D Print started" "the panel"
ui_step "cancel (Esc)"                   ui_key Escape
ui_expect_new "Command 3D Print cancelled" "cancelled"
sleep 1
[ -f "$ARGS" ] && ui_fail "the slicer was started after Cancel"
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -q "3D Print: .* bodies as" && ui_fail "files were written after Cancel"
echo "ok   nothing sent"

echo "--- Preferences keep the slicer"
ui_mark
ui_step "preferences (search)"           ui_command "Preferences"
ui_focus_dialog '^Preferences$'
ui_step "OK (Enter)"                     ui_key Return
ui_focus_main
ui_expect_new "Preferences: slicer Fake slicer ($WORK/fake-slicer.sh <files>)" "the slicer stays"

echo "--- STL where the design shows the bodies (mitcad#17)"
# A 20 x 10 x 5 pin in a component placed twice: turned a quarter about z
# and moved 100 mm along x, and 50 mm along y.
cat > "$WORK/assembly.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
    "length": 20, "width": 10, "height": 5, "operation": "new_body"}},
  {"cmd": "rename_body", "uid": "F1.b0", "name": "Pin"},
  {"cmd": "components_from_bodies", "bodies": ["F1.b0"]},
  {"cmd": "set_occurrence_transform", "occurrence": "Pin:1",
    "transform": {"translation": [100, 0, 0], "rotation": {"axis": [0, 0, 1], "angle": 1.5707963267948966}}},
  {"cmd": "copy_occurrence", "occurrence": "Pin:1", "transform": {"translation": [0, 50, 0]}}
]
EOF
"$CLI" run "$WORK/assembly.json" --save "$WORK/assembly.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
ui_stop_app
ui_start_app --open "$WORK/assembly.mitcad"
ASSEMBLY=$TMPDIR/mitcad-print/assembly
ui_step "3D print, STL (Format, Enter)"  send -1
ui_expect_new "3D Print panel: 1 visible: Pin; format 3mf" "the panel: the pin, 3MF remembered"
ui_expect_new "3D Print: 1 bodies as stl (high) to $ASSEMBLY: Pin (Pin_1).stl, Pin (Pin_2).stl" \
  "an STL file per occurrence, named with it"
slicer_ran
[ "$(cat "$ARGS")" = "$(printf '%s\n' "$ASSEMBLY/Pin (Pin_1).stl" "$ASSEMBLY/Pin (Pin_2).stl")" ] ||
  ui_fail "the slicer's arguments: $(cat "$ARGS")"
echo "ok   the slicer got both files"
python3 - "$ASSEMBLY" << 'EOF' || ui_fail "the STL files' places"
import struct, sys
for name, low, high in [("Pin (Pin_1)", (90, 0, 0), (100, 20, 5)), ("Pin (Pin_2)", (0, 50, 0), (20, 60, 5))]:
    data = open(f"{sys.argv[1]}/{name}.stl", "rb").read()
    count = struct.unpack_from("<I", data, 80)[0]
    assert len(data) == 84 + 50 * count, name
    points = [struct.unpack_from("<3f", data, 84 + 50 * i + 12 + 12 * v) for i in range(count) for v in range(3)]
    box = [(min(p[k] for p in points), max(p[k] for p in points)) for k in range(3)]
    print(f"     {name}: {count} triangles in {box}")
    assert all(abs(box[k][0] - low[k]) < 1e-4 and abs(box[k][1] - high[k]) < 1e-4 for k in range(3)), name
EOF
echo "ok   each file has the pin where the view shows that occurrence"

echo "--- A mesh body in a moved and turned occurrence (mitcad#20)"
# The pin and its STL copy (a mesh body), each in a component of its own,
# both occurrences turned a quarter about z and moved 100 mm along x.
cat > "$WORK/mesh.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
    "length": 20, "width": 10, "height": 5, "operation": "new_body"}},
  {"cmd": "rename_body", "uid": "F1.b0", "name": "Pin"},
  {"cmd": "export", "path": "pin.stl", "bodies": ["Pin"]},
  {"cmd": "import_file", "path": "pin.stl"},
  {"cmd": "rename_body", "uid": "F2.b0", "name": "Mesh"},
  {"cmd": "components_from_bodies", "bodies": ["F1.b0", "F2.b0"]},
  {"cmd": "set_occurrence_transform", "occurrence": "Pin:1",
    "transform": {"translation": [100, 0, 0], "rotation": {"axis": [0, 0, 1], "angle": 1.5707963267948966}}},
  {"cmd": "set_occurrence_transform", "occurrence": "Mesh:1",
    "transform": {"translation": [100, 0, 0], "rotation": {"axis": [0, 0, 1], "angle": 1.5707963267948966}}}
]
EOF
(cd "$WORK" && "$CLI" run mesh.json --save mesh.mitcad) > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
ui_stop_app
ui_start_app --open "$WORK/mesh.mitcad"
MESH=$TMPDIR/mitcad-print/mesh
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Bodies shown: F1.b0 in O1, F2.b0 in O2" "the solid and the mesh shown in their occurrences"
pin_at=$(ui_logged_at "Body F1.b0 in O1")
mesh_at=$(ui_logged_at "Body F2.b0 in O2")
[ "$pin_at" = "$mesh_at" ] || ui_fail "the view shows the mesh at $mesh_at, the solid at $pin_at"
echo "ok   the view shows the mesh where it shows the solid ($mesh_at)"
ui_step "3D print (Enter)"               send
ui_expect_new "3D Print panel: 2 visible: Pin, Mesh; format stl" "the panel: both bodies, STL remembered"
ui_expect_new "3D Print: 2 bodies as stl (high) to $MESH: Pin.stl, Mesh.stl" "an STL file each"
slicer_ran
python3 - "$MESH" << 'EOF' || ui_fail "the STL files' places"
import struct, sys
for name in ["Pin", "Mesh"]:
    data = open(f"{sys.argv[1]}/{name}.stl", "rb").read()
    count = struct.unpack_from("<I", data, 80)[0]
    points = [struct.unpack_from("<3f", data, 84 + 50 * i + 12 + 12 * v) for i in range(count) for v in range(3)]
    box = [(min(p[k] for p in points), max(p[k] for p in points)) for k in range(3)]
    print(f"     {name}: {count} triangles in {box}")
    assert all(abs(box[k][0] - (90, 0, 0)[k]) < 1e-4 and abs(box[k][1] - (100, 20, 5)[k]) < 1e-4 for k in range(3)), name
EOF
echo "ok   the mesh's file has it where the view shows it"

echo "--- Bodies picked in the panel (mitcad#234)"
# The plate (F1) and the cube (F2) as above, and a surface body: an open
# tube of 10 mm diameter, 20 high, lofted between two circles (F3, F5 on
# Plane1 F4; Loft1 F6).
cat > "$WORK/panel.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
    "length": 20, "width": 10, "height": 5, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [30, 0],
    "length": 10, "width": 10, "height": 10, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F3", "center": [55, 5], "diameter": 10},
  {"cmd": "add_feature", "def": {"type": "construction_plane", "definition": {"type": "offset", "plane": "xy", "distance": 20}}},
  {"cmd": "sketch.create", "plane": "F4"},
  {"cmd": "sketch.add_circle", "sketch": "F5", "center": [55, 5], "diameter": 10},
  {"cmd": "add_feature", "def": {"type": "loft", "sections": [
    {"type": "profile", "sketch": "F3", "region": "r{c1}"}, {"type": "profile", "sketch": "F5", "region": "r{c1}"}],
    "surface": true, "operation": "new_body"}},
  {"cmd": "rename_body", "uid": "F1.b0", "name": "Plate"},
  {"cmd": "rename_body", "uid": "F2.b0", "name": "Cube"},
  {"cmd": "rename_body", "uid": "F6.b0", "name": "Surface"}
]
EOF
"$CLI" run "$WORK/panel.json" --save "$WORK/panel.mitcad" > "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
ui_stop_app
ui_start_app --open "$WORK/panel.mitcad"
PANEL=$TMPDIR/mitcad-print/panel
ui_step "fit (F6)"                       ui_key F6
ui_expect_log "Body F6.b0 at " "the bodies placed in the view"
rm -f "$ARGS"
ui_mark
ui_step "3D print (search)"              ui_command "3D Print"
ui_expect_new "3D Print: left out Surface (a surface body)" "the surface body left out"
ui_expect_new "3D Print panel: 2 visible: Plate, Cube; format stl" "the panel: the plate and the cube, STL remembered"
ui_expect_new "3D Print Bodies: 2 bodies [body F1.b0; body F2.b0]" "both solids in Bodies"
ui_expect_new "View body highlights: F1.b0, F2.b0" "both highlighted"
ui_mark
ui_step "click the cube"                 ui_click_logged "Body F2.b0"
ui_expect_new "3D Print Bodies: 1 body [body F1.b0]" "the click took the cube out"
ui_expect_new "View body highlights: F1.b0" "and its highlight"
ui_mark
ui_step "click the surface"              ui_click_logged "Body F6.b0"
sleep 0.5
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qE "3D Print Bodies:|View body highlights:" &&
  ui_fail "the click on the surface body changed Bodies"
echo "ok   the surface body cannot be picked"
ui_mark
ui_step "send (Enter)"                   ui_key Return
ui_expect_new "3D Print: 1 bodies as stl (high) to $PANEL: Plate.stl" "the plate's file alone"
slicer_ran
[ "$(cat "$ARGS")" = "$PANEL/Plate.stl" ] || ui_fail "the slicer's arguments: $(cat "$ARGS")"
[ "$(ls "$PANEL")" = "Plate.stl" ] || ui_fail "the folder has: $(ls "$PANEL")"
echo "ok   the slicer got the plate's file alone"

ui_crashed && ui_fail "crashed"
ui_finish "UI 3D print test"
