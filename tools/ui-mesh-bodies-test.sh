#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework/Command.cpp app/framework/CommandSession.cpp app/OcctViewer.cpp app/browser app/commands core/model/src/features/surface_bodies.rs core/model-api/src/lib.rs tools/cli
# Mesh bodies in the inputs of surface operations (mitcad#399). A mesh body
# (triangles without surfaces: STL, OBJ, the meshes of imported models)
# cannot go to the booleans, fillets and face operations; OCCT's boolean
# failed on its faces (an access violation on Windows, mitcad#392). The
# inputs that hand bodies or faces to them do not take one:
#   - Shell's Faces/Body offers the block's faces but not the mesh body's
#     (Measure, which takes any face, offers both);
#   - Combine's Tool Bodies does not take the mesh body picked in the
#     browser, and says why; a solid tool is taken and previewed.
# A block of 30 x 20 x 10 (F2.b0) and a closed mesh cube of 20 mm from an
# STL file at y = 40 (F3.b0, "cube").
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-mesh-bodies-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
WORK=$(mktemp -d /tmp/mitcad-ui-mesh-bodies.XXXXXX)
FILE=$WORK/mesh.mitcad
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
R='r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

cat > "$WORK/model.json" << EOF
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "$R"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}}
]
EOF
# The cube from x = 0 to 20, y = 40 to 60, z = 0 to 20, counter-clockwise
# seen from outside.
facet() { printf 'facet normal 0 0 0\nouter loop\nvertex %s\nvertex %s\nvertex %s\nendloop\nendfacet\n' "$1" "$2" "$3"; }
{
  echo "solid cube"
  facet "0 40 0" "20 60 0" "20 40 0";    facet "0 40 0" "0 60 0" "20 60 0"
  facet "0 40 20" "20 40 20" "20 60 20"; facet "0 40 20" "20 60 20" "0 60 20"
  facet "0 40 0" "20 40 0" "20 40 20";   facet "0 40 0" "20 40 20" "0 40 20"
  facet "20 40 0" "20 60 0" "20 60 20";  facet "20 40 0" "20 60 20" "20 40 20"
  facet "20 60 0" "0 60 0" "0 60 20";    facet "20 60 0" "0 60 20" "20 60 20"
  facet "0 60 0" "0 40 0" "0 40 20";     facet "0 60 0" "0 40 20" "0 60 20"
  echo "endsolid cube"
} > "$WORK/cube.stl"
"$CLI" run "$WORK/model.json" --save "$FILE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli run"; }
"$CLI" import "$WORK/cube.stl" --open "$FILE" --save "$FILE" >> "$WORK/cli.log" 2>&1 ||
  { cat "$WORK/cli.log"; ui_fail "mitcad-cli import"; }
grep -q "1 faces, 0 edges" "$WORK/cli.log" || { cat "$WORK/cli.log"; ui_fail "no mesh body imported"; }

ui_start_display
ui_start_app --open "$FILE"
ui_step "fit (F6)"                         ui_key F6
ui_expect_log "Bodies shown: " "the block and the mesh shown"

# Measure takes any face: the mesh body's face has a pick place.
ui_mark
ui_step "measure (search)"                 ui_command "Measure"
ui_expect_new "Pick face F3.b0/F3:import(0) at" "Measure offers the mesh body's face"
ui_step "cancel (Esc)"                     ui_key Escape

# Shell's Faces/Body does not.
ui_mark
ui_step "shell (search)"                   ui_command "Shell"
ui_expect_new "Pick face F2.b0/" "Shell offers the block's faces"
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "Pick face F3.b0/" &&
  ui_fail "Shell offers the mesh body's face"
echo "ok   Shell does not offer the mesh body's face"
ui_step "cancel (Esc)"                     ui_key Escape

# Combine: the mesh body picked in the browser is not taken.
ui_step "combine (search)"                 ui_command "Combine"
ui_step "pick Body1 (target)"              ui_click_logged "Browser Root/Bodies/Body1"
ui_expect_log "Combine Target Body: 1 body [body F2.b0]" "the block is the target"
ui_mark
ui_step "pick cube (tool)"                 ui_click_logged "Browser Root/Bodies/cube"
ui_expect_new "Combine Tool Bodies: body F3.b0 not taken: a mesh body" "the mesh body refused"
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep "Combine Tool Bodies: 1 body" &&
  ui_fail "the mesh body was taken as a tool"
echo "ok   no tool taken"
ui_step "cancel (Esc)"                     ui_key Escape
ui_expect_log "Command Combine cancelled" "cancelled"

ui_finish "UI mesh bodies test"
