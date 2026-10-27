# SPDX-License-Identifier: MIT
# check-all sources: app/build app/commands/BuildCommands.cpp app/framework app/library app/OcctViewer.cpp tools/cli core/model/src/build core/model/src/api/build.rs
# check-all sources: app/MainWindowContextMenu.cpp
# Input-driven Windows UI test of the BUILD tab's TIMBER group: Timber
# (mitcad#478, mitcad#485), Wall and Floor / Roof (mitcad#520), each section
# with its own tool, in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted). A library of small synthetic parts made here (mitcad-cli; a
# slot profile, two timber members, one of a less common size, and a wood
# screw), fetched from a local git repository; skipped where git is not
# installed:
#   1. Profile's picker lists no timber, Timber's no profile, its standard
#      size first; Timber's first start (mitcad#485): On a member, a typed
#      length of the part's default (3000 mm), along X, flat, the
#      dimensioned 48x98, resting on the XY plane (mitcad#492).
#   2. On a member: a post standing on the XY plane (Enter at the origin;
#      after Place nothing previewed at the origin on top of it, the panel
#      waiting for a click on a member, mitcad#513),
#      then a member on the post's side: the hover names only the face a
#      click picks, nothing previewed before the click; once picked, the
#      mouse along the side, off the parts and back previews and inspects
#      nothing, the preview's end A unchanged, a click on another face
#      moves it once and a click at the first place back (mitcad#508); the
#      next start takes the length typed last and the direction
#      (mitcad#485).
#   3. Two points (mitcad#485): a member from a click on the second
#      member's side to a click on the post's side, nothing previewed
#      towards the mouse between the clicks; Tab turns it on edge.
#   4. Spacing: members at a centre distance along the second member's
#      side; the hover names only the face a click picks, nothing previewed
#      or inspected before the click (mitcad#489); every member previewed
#      after the click, which stays there while the mouse moves, a click on
#      another face moving it once (mitcad#508), and again when the
#      distance changes their count (mitcad#487) and again when the Offset
#      typed after the click moves the set from end A, chosen in From
#      (mitcad#492), placed as one set
#      of as many; Spacing Set edits its distance.
#   5. Connector at a timber junction: wood screws by default, sized from
#      the members, one click.
#   6. Profile again: the design's timber is not in its picker; Cut List:
#      the timber rows.
#   7. Every size a tile of its own (mitcad#484): the picker lists the 48
#      member's three rows and the 28 one, the families as groups, a row's
#      own picture; starring 48x148 stars that size only (mitcad#491): it
#      comes first and Favourites lists it alone; a search by 48x148 finds
#      its tile, which places that row with its section. The star changes
#      only the star: the tiles stay where they are until the list is made
#      again (mitcad#517). The use count is per size (mitcad#518): only the
#      48x98 tile counts the design's members; once a 48x148 member is
#      placed, that tile counts it, and In this design lists the two sizes.
#   8. A frame of four members with shared mitres (mitcad#485): each on the
#      end of the one before, turned at a corner, lying flat as it does
#      (ends of the same width); the last one's auto length bound to the
#      first closes the corner with a shared mitre, as long as the first
#      one's outer side.
#   9. The same frame of slot profiles with Profile, the last one's length
#      through the first: a shared mitre, not into it.
#  10. Wall's walls (the TIMBER group's tool, mitcad#520; mitcad#493,
#      mitcad#510): a wall along X, Corner
#      Butt (the default); after Place nothing previewed from the origin
#      on top of it and Enter places no second wall there, the view asking
#      where the next wall starts (mitcad#513); a wall started by a click
#      on its end stud's side, a few centimetres from the corner, comes
#      flush with end A (mitcad#514; a backing stud more in the first
#      wall), an Offset typed moving the preview once and 0 bringing it
#      back; one by a click on its end B's face (the view from the right)
#      starts from that end (a butt corner). Then, in a new design, Corner
#      Lapped plates: a click on the side near end B makes an L corner (a
#      junction stud more in the first wall), one mid-side a T; each with a
#      typed length, the panel naming where it starts and the corner, each
#      placed with the members previewed; the next Wall start remembers
#      Lapped plates. Then, in a new design (mitcad#515), two walls flush
#      with the first one's ends and a wall started on the first of them
#      towards the other: no End clicked, it ends butting the other's side.
#  11. The timber frame again, the last one's length typed to the first's
#      outer side: it closes the corner with a shared mitre too (mitcad#492).
#  12. A board along X on the ground rests on it (its axis 24 mm up); a
#      member standing on its top face from end A at offset 0 lies as it
#      does: its width along the board's, its sides flush (mitcad#492).
#  13. Fasteners (mitcad#497; the library adds a wood screw, a ring nail
#      and a pan-head connector screw in two sizes each, with heads to
#      click on): the first time the first countersunk wood screw, not the
#      connector screw whose name comes first (mitcad#516); Wall with a chosen
#      fastener (the picker's), size 5.0x90 and three at each junction;
#      Timber's Spacing on the wall's top plate with Fasteners on (the
#      fastener chosen in Wall, shared by the TIMBER group), each member
#      fastened; two of the wall's screws selected, the context menu's
#      BUILD > Swap Fastener (chosen with the keys) on them makes them
#      3.1x90 ring nails, one undo step. Wall's fastener rows lie below
#      the dock in session 0's low window: the panel scrolls to them
#      (Ui-PanelReveal).
#  14. A wall with Wall, then Timber's Two points from a stud's side to
#      the next stud's (mitcad#509): the
#      sides are not one plane, so the member is free between the points
#      (not its end moved onto the start's plane, a member a few
#      millimetres long), as long as the studs' distance, its end cut back
#      to the next stud's face.
#  15. Floor / Roof (mitcad#520): a floor the first time, no wall's
#      inputs, rim joists all round, placed; Kind Roof waits for its two
#      walls.
#  16. The Openings window (mitcad#519): from Wall's Openings tile, Add
#      window (previewed once, a partition's flat header and no trimmers,
#      marked), Add door, a row typed outside the wall flagged and put
#      back, Load-bearing chosen once (trimmers, headers on edge), placed
#      with both; Timber Frame (MODIFY) on the wall opens the window with
#      them. Clicks, typing and the wheel go to the window itself.
#  17. Each frame a component of its own (mitcad#522): three walls (Wall 2
#      and Wall 3 flush with Wall 1's ends), each logged as a component in
#      Component1, Component1 holding no joint; Timber Frame on a plate of Wall 1 lays
#      it out again in the same component; a gable roof on Wall 2 and Wall
#      3 with ring nails (mitcad#521, the owner's case): two at each
#      junction, each at its junction's members, its head on the entry
#      face, their joints in Roof 1; the cut list by frame.
#  18. A Start on another body's face (mitcad#524, the owner's case): a
#      design of a slab and a solid wall with a door (mitcad-cli); a click
#      on the wall's side face starts a wall square from it on the slab's
#      top; on its end face a corner with Butt, in line with Straight (the
#      Corner's icon buttons), placed in line.
#  19. A wall across a long room (mitcad#536, the owner's case): the room's
#      walls one body (mitcad-cli); a start on a long wall's inner face,
#      Length Auto: it previews and is placed as wide as the room, ending at
#      the opposite face of the same body, its plates longer than the
#      timber's longest spliced over studs; the inspection names what the
#      run met and the length, also when a height its studs cannot have
#      refuses the preview.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-timber-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-timber-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-timber'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'build.profile' = 'F3'; 'build.timber' = 'F4'; 'build.wall' = 'F7';
                            'build.connector' = 'F8'; 'build.cut_list' = 'F9'; 'build.spacing' = 'F11';
                            'build.floor_roof' = 'F5'; 'build.frame' = 'F12' }

$git = Get-Command git -ErrorAction SilentlyContinue
if (-not $git) {
  Write-Host 'note: git is not installed; libraries cannot be fetched, nothing to test'
  Write-Host 'PASS: UI Windows timber test (skipped)'
  exit 0
}
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$gitConfig = Join-Path $work 'gitconfig'
Set-Content -Encoding ascii $gitConfig ''
$env:GIT_CONFIG_GLOBAL = $gitConfig
$env:GIT_CONFIG_NOSYSTEM = '1'
$env:MITCAD_LIBRARIES_DIR = Join-Path $work 'cache'
$library = Join-Path $work 'test-timber'

function Run-Cli([string]$What, [string[]]$Arguments) {
  $output = & $cli @Arguments 2>&1
  if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli ${What}: $($output -join ' / ')" }
}
# Design name script: the commands saved as the library's design <name>.
function Design([string]$Name, [string]$Script) {
  $json = Join-Path $work (($Name -replace '[\\/]', '-') + '.json')
  Set-Content -Encoding ascii $json $Script
  $target = Join-Path $library $Name
  New-Item -ItemType Directory -Force (Split-Path $target) | Out-Null
  Run-Cli "run $Name" @('run', $json, '--save', $target)
}

Run-Cli 'library init' @('library', 'init', $library, '--id', 'test-timber', '--name', 'Test timber', '--license', 'MIT')
$rect = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
function Frame([string]$Name, [string]$Origin, [string]$Z, [string]$X) {
  return ('  {"cmd": "add_feature", "name": "mount:' + $Name + '", "def": {"type": "joint_origin", "geometry": "origin", ' +
          '"frame_override": {"origin": ' + $Origin + ', "z_axis": ' + $Z + ', "x_axis": ' + $X + '}}},')
}
# A bar W x T centred on its XY origin, L long along +Z, with its end frames.
function Bar([double]$W, [double]$T, [string]$Length) {
  return @(
    '[',
    ('  {"cmd": "add_parameter", "name": "L", "expression": "' + $Length + '"},'),
    '  {"cmd": "sketch.create", "plane": "xy"},',
    ('  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-' + ($W / 2) + ', -' + ($T / 2) + '], "width": ' + $W +
     ', "height": ' + $T + '},'),
    ('  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "' + $rect + '"}], ' +
     '"extent": {"type": "distance", "distance": "L"}, "operation": "new_body"}},'),
    (Frame 'end_a' '[0, 0, 0]' '[0, 0, -1]' '[1, 0, 0]'),
    ('  {"cmd": "add_feature", "name": "mount:end_b", "def": {"type": "joint_origin", ' +
     '"geometry": {"body": "F2.b0", "face": "F2:end(' + $rect + ')"}}},')) -join "`n"
}
# A timber member: the bar with a face frame on each side (docs/libraries.md, "Timber parts").
function Member([double]$W, [double]$T) {
  return @((Bar $W $T '600 mm'),
    (Frame 'face_xp' ('[' + ($W / 2) + ', 0, 0]') '[1, 0, 0]' '[0, 0, 1]'),
    (Frame 'face_xn' ('[-' + ($W / 2) + ', 0, 0]') '[-1, 0, 0]' '[0, 0, 1]'),
    (Frame 'face_yp' ('[0, ' + ($T / 2) + ', 0]') '[0, 1, 0]' '[0, 0, 1]'),
    (Frame 'face_yn' ('[0, -' + ($T / 2) + ', 0]') '[0, -1, 0]' '[0, 0, 1]')) -join "`n"
}
$end = "`n" + '  {"expect": {"bodies": 1}}' + "`n" + ']'
$profile20 = @((Bar 20 20 '500 mm'), (Frame 'slot_x+_1' '[10, 0, 0]' '[1, 0, 0]' '[0, 0, 1]'),
  (Frame 'slot_x-_1' '[-10, 0, 0]' '[-1, 0, 0]' '[0, 0, 1]'), (Frame 'slot_y+_1' '[0, 10, 0]' '[0, 1, 0]' '[0, 0, 1]'),
  (Frame 'slot_y-_1' '[0, -10, 0]' '[0, -1, 0]' '[0, 0, 1]')) -join "`n"
Design 'profiles\p20-test.mitcad' ($profile20 + $end)
# The 48 member in three sizes (mitcad#484): W x T from its rows, centred on
# XY, its face frames on planes at W / 2 and T / 2 (the generator's layout).
function Side-Frame([string]$Name, [string]$Normal, [string]$Distance, [int]$Plane) {
  return ('  {"cmd": "add_feature", "name": "Plane of ' + $Name + '", "def": {"type": "construction_plane", ' +
          '"definition": {"type": "offset", "plane": {"origin": [0, 0, 0], "normal": ' + $Normal +
          ', "x_axis": [0, 0, 1]}, "distance": "' + $Distance + '"}}},' + "`n" +
          '  {"cmd": "add_feature", "name": "mount:' + $Name + '", "def": {"type": "joint_origin", "geometry": "F' +
          $Plane + '"}},')
}
$sizes = @(@('48x98', 98), @('48x148', 148), @('48x198', 198)) | ForEach-Object {
  '    {"name": "' + $_[0] + '", "select": {"Size": "' + $_[0] + '"}, "values": {"W": "' + $_[1] + ' mm", "T": "48 mm"}}' }
$member48 = @('[',
  '  {"cmd": "add_parameter", "name": "L", "expression": "600 mm"},',
  '  {"cmd": "add_parameter", "name": "W", "expression": "98 mm"},',
  '  {"cmd": "add_parameter", "name": "T", "expression": "48 mm"},',
  ('  {"cmd": "add_feature", "name": "Section", "def": {"type": "sketch", "plane": "xy", "entities": [' +
   '{"id": "p1", "type": "point", "at": [-49, -24]}, {"id": "p2", "type": "point", "at": [49, -24]}, ' +
   '{"id": "p3", "type": "point", "at": [49, 24]}, {"id": "p4", "type": "point", "at": [-49, 24]}, ' +
   '{"id": "p5", "type": "point", "at": [0, 0], "fixed": true}, ' +
   '{"id": "c6", "type": "line", "start": "p1", "end": "p2"}, {"id": "c7", "type": "line", "start": "p2", "end": "p3"}, ' +
   '{"id": "c8", "type": "line", "start": "p3", "end": "p4"}, {"id": "c9", "type": "line", "start": "p4", "end": "p1"}, ' +
   '{"id": "c10", "type": "line", "start": "p1", "end": "p3", "construction": true}], "constraints": [' +
   '{"id": "k11", "type": "horizontal", "line": "c6"}, {"id": "k12", "type": "vertical", "line": "c7"}, ' +
   '{"id": "k13", "type": "horizontal", "line": "c8"}, {"id": "k14", "type": "vertical", "line": "c9"}, ' +
   '{"id": "k15", "type": "midpoint", "point": "p5", "curve": "c10"}], "dimensions": [' +
   '{"id": "k16", "type": "length", "line": "c6", "value": "W"}, {"id": "k17", "type": "length", "line": "c7", "value": "T"}]}},'),
  ('  {"cmd": "add_feature", "name": "Profile", "def": {"type": "extrude", "profiles": [{"sketch": "F1", ' +
   '"region": "r{c6[c9,c7],c7[c6,c8],c8[c7,c9],c9[c8,c6]}"}], "extent": {"type": "distance", "distance": "L"}, ' +
   '"operation": "new_body"}},'),
  ('  {"cmd": "add_feature", "name": "End B plane", "def": {"type": "construction_plane", ' +
   '"definition": {"type": "offset", "plane": "xy", "distance": "L"}}},'),
  '  {"cmd": "add_feature", "name": "mount:end_a", "def": {"type": "joint_origin", "geometry": "xy", "flip": true}},',
  '  {"cmd": "add_feature", "name": "mount:end_b", "def": {"type": "joint_origin", "geometry": "F3"}},',
  (Side-Frame 'face_xp' '[1, 0, 0]' 'W / 2' 6), (Side-Frame 'face_xn' '[-1, 0, 0]' 'W / 2' 8),
  (Side-Frame 'face_yp' '[0, 1, 0]' 'T / 2' 10), (Side-Frame 'face_yn' '[0, -1, 0]' 'T / 2' 12),
  ('  {"cmd": "set_configurations", "configurations": {"selectors": ["Size"], "parameters": ["W", "T"], ' +
   '"default": "48x98", "rows": [' + "`n" + ($sizes -join ",`n") + ']}},'),
  '  {"expect": {"total_volume": 2822400, "tolerance": 1e-6}},') -join "`n"
Design 'dimensioned-timber\timber-48-test.mitcad' ($member48 + $end)
Design 'planed-timber\timber-28-test.mitcad' ((Member 120 28) + $end)
# Pictures: the 48 member's (16 px) and its row 48x148's own (24 px).
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force (Join-Path $library 'previews') | Out-Null
foreach ($picture in @(@('timber-48-test', 16), @('timber-48-test-48x148', 24))) {
  $bitmap = New-Object System.Drawing.Bitmap $picture[1], $picture[1]
  $bitmap.Save((Join-Path $library "previews\$($picture[0]).png"), [System.Drawing.Imaging.ImageFormat]::Png)
  $bitmap.Dispose()
}
# A wood screw: a 5 x 5 shank 70 long along +Z, its axis frame at the head.
$screw = @('[',
  '  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"}, "corner": [0, 0], "length": 5, "width": 5, "height": 70, "operation": "new_body"}},',
  (Frame 'axis' '[2.5, 2.5, 0]' '[0, 0, 1]' '[1, 0, 0]')) -join "`n"
Design 'fasteners\wood-screw-test.mitcad' ($screw + $end)
# Fasteners in two sizes (mitcad#497): a head SIZE x SIZE x 10 above the
# axis frame (the entry point), outside the members to click on.
function Head-Fastener([int]$Size, [string]$Row1, [string]$Row2) {
  return @('[',
    ('  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"}, "corner": [-' + ($Size / 2) +
     ', -' + ($Size / 2) + '], "length": ' + $Size + ', "width": ' + $Size + ', "height": 10, "operation": "new_body"}},'),
    (Frame 'axis' '[0, 0, 10]' '[0, 0, 1]' '[1, 0, 0]'),
    ('  {"cmd": "set_configurations", "configurations": {"parameters": [], "default": "' + $Row1 + '", "rows": [{"name": "' +
     $Row1 + '"}, {"name": "' + $Row2 + '"}]}},')) -join "`n"
}
Design 'fasteners\wood-screw-head-test.mitcad' ((Head-Fastener 30 '5.0x70' '5.0x90') + $end)
Design 'fasteners\ring-nail-test.mitcad' ((Head-Fastener 20 '2.8x50' '3.1x90') + $end)
# A pan-head connector screw (mitcad#516): its name comes first, but it
# bears on a plate and is no frame's default.
Design 'fasteners\connector-screw-test.mitcad' ((Head-Fastener 24 '5.0x40' '5.0x60') + $end)

Set-Content -Encoding ascii (Join-Path $library 'mitcad-library.json') @'
{
  "format": "mitcad-library", "version": 1, "id": "test-timber", "name": "Test timber",
  "description": "Synthetic BUILD parts of the timber UI test.", "library_version": "1.0.0", "license": "MIT",
  "authors": ["Mitcad UI test"], "units": "mm",
  "categories": [{"id": "profiles", "name": "Profiles"}, {"id": "dimensioned-timber", "name": "Dimensioned timber"},
                 {"id": "planed-timber", "name": "Planed timber"}, {"id": "fasteners", "name": "Fasteners"}],
  "components": [
    {"id": "p20-test", "path": "profiles/p20-test.mitcad", "name": "Profile 20 test", "category": "profiles",
     "supplier": "Test supplier", "part_number": "P20-T", "designation": "Profile 20 test L={L}",
     "build": {"role": "profile", "system": {"type": "I", "slot": 5, "grid": 20},
       "length": {"parameter": "L", "min": "10 mm", "max": "3000 mm", "stock": ["1000 mm"]},
       "section": {"width": "20 mm", "height": "20 mm"},
       "slot": {"depth": "6 mm", "opening": "5.2 mm", "inner_width": "11 mm"}, "mass_per_metre": "0.4 kg",
       "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
                  {"name": "slot_x+_1", "kind": "slot"}, {"name": "slot_x-_1", "kind": "slot"},
                  {"name": "slot_y+_1", "kind": "slot"}, {"name": "slot_y-_1", "kind": "slot"}]}},
    {"id": "timber-28-test", "path": "planed-timber/timber-28-test.mitcad", "name": "Timber 28x120 test",
     "category": "planed-timber", "designation": "Timber 28x120 test L={L}",
     "build": {"role": "profile", "kind": "timber",
       "material": {"species": "pine", "finish": "planed", "grade": "", "density": 520, "moisture": 20},
       "availability": "less common",
       "section": {"width": "120 mm", "height": "28 mm"}, "nominal": {"width": "120 mm", "height": "28 mm"},
       "length": {"parameter": "L", "min": "100 mm", "max": "6000 mm", "default": "600 mm", "stock": ["3000 mm"]},
       "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
                  {"name": "face_xp", "kind": "face"}, {"name": "face_xn", "kind": "face"},
                  {"name": "face_yp", "kind": "face"}, {"name": "face_yn", "kind": "face"}]}},
    {"id": "timber-48-test", "path": "dimensioned-timber/timber-48-test.mitcad", "name": "Timber 48x98 test",
     "category": "dimensioned-timber", "designation": "Timber 48x98 test L={L}",
     "preview": "previews/timber-48-test.png", "row_previews": {"48x148": "previews/timber-48-test-48x148.png"},
     "build": {"role": "profile", "kind": "timber",
       "material": {"species": "spruce", "finish": "dimensioned", "grade": "", "density": 450, "moisture": 20},
       "availability": "standard",
       "section": {"width": "98 mm", "height": "48 mm"}, "nominal": {"width": "98 mm", "height": "48 mm"},
       "length": {"parameter": "L", "min": "100 mm", "max": "6000 mm", "default": "3000 mm",
                  "stock": ["3000 mm", "3600 mm"]},
       "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
                  {"name": "face_xp", "kind": "face"}, {"name": "face_xn", "kind": "face"},
                  {"name": "face_yp", "kind": "face"}, {"name": "face_yn", "kind": "face"}],
       "rows": {"48x148": {"section": {"width": "148 mm", "height": "48 mm"},
                           "nominal": {"width": "148 mm", "height": "48 mm"}},
                "48x198": {"section": {"width": "198 mm", "height": "48 mm"},
                           "nominal": {"width": "198 mm", "height": "48 mm"}, "availability": "less common"}}}},
    {"id": "connector-screw-test", "path": "fasteners/connector-screw-test.mitcad", "name": "Connector screw test",
     "category": "fasteners", "designation": "Connector screw test {config}",
     "build": {"role": "fastener", "wood": {"kind": "screw", "head": "pan", "seat": "head_underside",
                                            "diameter": "5 mm", "length": "40 mm"},
       "material": {"product": "steel", "density": 7850},
       "frames": [{"name": "axis", "kind": "shaft"}],
       "rows": {"5.0x40": {"wood": {"diameter": "5 mm", "length": "40 mm"}},
                "5.0x60": {"wood": {"diameter": "5 mm", "length": "60 mm"}}}}},
    {"id": "wood-screw-test", "path": "fasteners/wood-screw-test.mitcad", "name": "Wood screw test",
     "category": "fasteners", "designation": "Wood screw test 5x70",
     "build": {"role": "fastener", "wood": {"kind": "screw", "head": "countersunk", "diameter": "5 mm", "length": "70 mm"},
       "material": {"product": "steel", "density": 7850},
       "frames": [{"name": "axis", "kind": "shaft"}]}},
    {"id": "wood-screw-head-test", "path": "fasteners/wood-screw-head-test.mitcad", "name": "Wood screw head test",
     "category": "fasteners", "designation": "Wood screw head test {config}",
     "build": {"role": "fastener", "wood": {"kind": "screw", "head": "countersunk", "diameter": "5 mm", "length": "70 mm"},
       "material": {"product": "steel", "density": 7850},
       "frames": [{"name": "axis", "kind": "shaft"}],
       "rows": {"5.0x70": {"wood": {"diameter": "5 mm", "length": "70 mm"}},
                "5.0x90": {"wood": {"diameter": "5 mm", "length": "90 mm"}}}}},
    {"id": "ring-nail-test", "path": "fasteners/ring-nail-test.mitcad", "name": "Ring nail test",
     "category": "fasteners", "designation": "Ring nail test {config}",
     "build": {"role": "fastener", "wood": {"kind": "nail", "diameter": "3.1 mm", "length": "90 mm"},
       "material": {"product": "steel", "density": 7850},
       "frames": [{"name": "axis", "kind": "shaft"}],
       "rows": {"2.8x50": {"wood": {"diameter": "2.8 mm", "length": "50 mm"}},
                "3.1x90": {"wood": {"diameter": "3.1 mm", "length": "90 mm"}}}}}
  ]
}
'@
foreach ($arguments in @(@('add', '-A'), @('commit', '-q', '-m', 'Test parts'), @('tag', 'v1.0.0'))) {
  $output = & $git.Source -C $library -c user.name=T -c user.email=t@example.invalid @arguments 2>&1
  if ($LASTEXITCODE -ne 0) { Ui-Fail "git $($arguments -join ' '): $($output -join ' / ')" }
}
Run-Cli 'library fetch' @('library', 'fetch', $library)

function Placed-Body { return Ui-LastMatch '^New body .* \((F[0-9]+\.b[0-9]+)\): volume' }
# The faces of a body a click can pick now (after the last "Pick places:")
# whose names contain the text: a derived part's sides are
# "<F>:part(F2:side(c1[c4,c2]))", its ends "<F>:end_a", "<F>:end_b".
function Faces-Of([string]$Body, [string]$Like) {
  $lines = @(Ui-LogLines)
  $start = 0
  for ($i = $lines.Count - 1; $i -ge 0; $i--) { if ($lines[$i].Contains('Pick places:')) { $start = $i; break } }
  $pattern = '^Pick face ' + [regex]::Escape($Body) + '/(.*) at \d+,\d+$'
  return @($lines[$start..($lines.Count - 1)] | ForEach-Object { if ($_ -match $pattern) { $Matches[1] } } |
    Where-Object { $_.Contains($Like) })
}
function Pick-Face([string]$Body, [string]$Face) { Ui-ClickPick "$Body/$Face at" }
function Pick-At([string]$Body, [string]$Face) { return Ui-PickAt "$Body/$Face at" }
# The view fitted to the parts, and the places a click picks logged again.
function Refresh-Picks([string]$Field) {
  Ui-Step 'fit the view (F6)'            { Ui-Key 'F6' }
  Ui-Step "Timber's $Field again"        { Ui-ClickLogged "Panel Timber input $Field" }
}
# Picker-Click TEXT: a click on a place of the part picker that it logged
# ("Part picker tile <part> at x,y; in the picker x,y"), posted to the
# picker's own window (tools/ui-windows-build-test.ps1).
function Picker-Click([string]$Text) {
  $place = $null
  for ($i = 0; $i -lt 30 -and -not $place; $i++) {
    $line = Ui-LogLines | Where-Object { $_.Contains("$Text at ") } | Select-Object -Last 1
    $match = [regex]::Match("$line", 'in the picker (-?\d+),(-?\d+)$')
    if ($match.Success) { $place = @([int]$match.Groups[1].Value, [int]$match.Groups[2].Value) }
    else { Start-Sleep -Milliseconds 200 }
  }
  if (-not $place) { Ui-Fail "no place of the part picker logged for '$Text'" }
  $picker = Ui-Windows '^Part Picker'
  if ($picker.Count -eq 0) { Ui-Fail 'no part picker window' }
  $b = $script:UiButtons[1]
  [UiInput]::Move($picker[0], $place[0], $place[1], 0)
  Start-Sleep -Milliseconds 80
  [UiInput]::Button($picker[0], $b[0], $place[0], $place[1], $b[3])
  Start-Sleep -Milliseconds 60
  [UiInput]::Button($picker[0], $b[1], $place[0], $place[1], 0)
}
# Picker-Type TEXT: typed into the part picker's window (its focused field).
function Picker-Type([string]$Text) {
  $picker = Ui-Windows '^Part Picker'
  if ($picker.Count -eq 0) { Ui-Fail 'no part picker window' }
  [UiInput]::Type($picker[0], $Text, 40)
}
# Picker-Keys @(VK, ...) COUNT: each virtual key COUNT times, to the part
# picker's window (End, BackSpace in its search field).
function Picker-Keys([int[]]$Keys, [int]$Count) {
  $picker = Ui-Windows '^Part Picker'
  if ($picker.Count -eq 0) { Ui-Fail 'no part picker window' }
  foreach ($vk in $Keys) {
    for ($i = 0; $i -lt $Count; $i++) {
      [UiInput]::Key($picker[0], $vk, $false, $false, $false)
      Start-Sleep -Milliseconds 60
    }
  }
}
function Place([string]$What) {
  Ui-Mark
  Ui-Step 'Place (Enter)'                { Ui-Key 'Return' }
  Ui-ExpectNew "Placed $What as " "$What placed" 30
}
$side = ':part(F2:side('
# Set-Previewed TEXT description: a spacing set's preview after the click
# (mitcad#487) in an "Inspect Timber:" line since the mark with TEXT: as
# many bodies drawn as the model lays out members, as many as the panel
# counts, two or more; returns the count.
function Set-Previewed([string]$Text, [string]$Description) {
  $pattern = '; set preview (\d+) member\(s\), (\d+) body\(ies\).*; set of (\d+),'
  $line = $null
  for ($i = 0; $i -lt 50 -and -not $line; $i++) {
    $line = Ui-LogLines | Select-Object -Skip $script:UiMark |
      Where-Object { $_.StartsWith('Inspect Timber: ') -and $_.Contains($Text) -and $_ -match $pattern } |
      Select-Object -Last 1
    if (-not $line) { Start-Sleep -Milliseconds 200 }
  }
  if (-not $line) { Ui-Fail "${Description}: no set preview in an 'Inspect Timber:' line with '$Text' since the mark" }
  $null = "$line" -match $pattern
  $members = [int]$Matches[1]
  $bodies = [int]$Matches[2]
  $counted = [int]$Matches[3]
  if ($members -lt 2 -or $bodies -ne $members -or $counted -ne $members) {
    Ui-Fail "${Description}: the preview shows $bodies body(ies) of $members member(s), the panel counts ${counted}: $line"
  }
  Write-Host "ok   ${Description}: all $members members previewed"
  return $members
}

# Refresh-At CMD: the view fitted to the parts and CMD's Place On picked
# again: the places a click picks logged again.
function Refresh-At([string]$Cmd) {
  Ui-Step 'fit the view (F6)'            { Ui-Key 'F6' }
  Ui-Step "$Cmd's Place On again"        { Ui-ClickLogged "Panel $Cmd input at" }
}
# Expect-Inspect CMD TEXTS description: an "Inspect CMD:" line since the
# mark has every text.
function Expect-Inspect([string]$Cmd, [string[]]$Texts, [string]$Description) {
  for ($i = 0; $i -lt 30; $i++) {
    $found = Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith("Inspect ${Cmd}: ") } |
      Where-Object { $line = $_; @($Texts | Where-Object { -not $line.Contains($_) }).Count -eq 0 } |
      Select-Object -First 1
    if ($found) { Write-Host "ok   $Description"; return }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "${Description}: no 'Inspect ${Cmd}:' line since the mark with $($Texts -join ' / ')"
}
# No-Inspect CMD description: a moment after the mouse moved, no "Inspect
# CMD:" line since the mark: the hover runs no placement and no inspection
# (mitcad#508).
function No-Inspect([string]$Cmd, [string]$Description) {
  Start-Sleep -Milliseconds 1500
  Ui-Sync
  $found = @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith("Inspect ${Cmd}: ") })
  if ($found.Count -gt 0) { Ui-Fail "${Description}: $($found[0])" }
  Write-Host "ok   $Description"
}
# No-Ghost CMD TEXT description: a moment after Place, no "Inspect CMD:
# TEXT" line since the mark: nothing previewed at the default place on top
# of what was just placed (mitcad#513).
function No-Ghost([string]$Cmd, [string]$Text, [string]$Description) {
  Start-Sleep -Milliseconds 1500
  Ui-Sync
  $found = @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith("Inspect ${Cmd}: $Text") })
  if ($found.Count -gt 0) { Ui-Fail "${Description}: $($found[0])" }
  Write-Host "ok   $Description"
}
# Last-EndA CMD: where the last "Inspect CMD:" line puts the preview's end
# A ("; end A at x, y, z").
function Last-EndA([string]$Cmd) {
  $line = Ui-LogLines | Where-Object { $_.StartsWith("Inspect ${Cmd}: ") } | Select-Object -Last 1
  if ("$line" -match '; end A at ([^;]*);') { return $Matches[1] }
  return $null
}
# EndA-Since CMD: the places of the preview's end A in the "Inspect CMD:"
# lines since the mark, each once.
function EndA-Since([string]$Cmd) {
  return @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith("Inspect ${Cmd}: ") } |
    ForEach-Object { if ($_ -match '; end A at ([^;]*);') { $Matches[1] } } | Select-Object -Unique)
}
# Moved-Once CMD WAS description: after a click, the "Inspect CMD:" lines
# since the mark put the preview's end A elsewhere than WAS, all at one
# place: the preview moved once, to the click (mitcad#508).
function Moved-Once([string]$Cmd, [string]$Was, [string]$Description) {
  $places = @()
  for ($i = 0; $i -lt 30 -and $places.Count -eq 0; $i++) {
    $places = @(EndA-Since $Cmd)
    if ($places.Count -eq 0) { Start-Sleep -Milliseconds 200 }
  }
  # A moment more: a preview still following the mouse would add places.
  Start-Sleep -Milliseconds 1000
  Ui-Sync
  $places = @(EndA-Since $Cmd)
  if ($places.Count -eq 0) { Ui-Fail "${Description}: no 'Inspect ${Cmd}:' line with end A since the mark" }
  if ($places.Count -gt 1) { Ui-Fail "${Description}: end A at more than one place: $($places -join ' / ')" }
  if ($places[0] -eq $Was) { Ui-Fail "${Description}: end A still at $Was" }
  Write-Host "ok   ${Description}: end A at $($places[0])"
}
# Hover-Names CMD BODY FACE AT: before a click, the mouse over BODY's FACE
# at AT (its pick place) names the face a click picks and previews
# nothing: no "Inspect CMD:" line (mitcad#489, mitcad#508).
function Hover-Names([string]$Cmd, [string]$Body, [string]$Face, [int[]]$At) {
  Ui-Sync
  Start-Sleep -Milliseconds 500
  Ui-Mark
  # MK_SHIFT keeps the move in session 0 (tools/ui-windows-build-test.ps1).
  Ui-Step "the mouse over $Body's $Face"  { [UiInput]::Move($script:UiWindow, $At[0], $At[1], 0x04) }
  Ui-ExpectNew "BUILD hover pick: face $Face of $Body" 'the hover names the face a click picks'
  Ui-Step 'the mouse a little further'   { [UiInput]::Move($script:UiWindow, ($At[0] + 6), $At[1], 0x04) }
  No-Inspect $Cmd 'no preview and no inspection while hovering before the click'
}
# Keeps-Place CMD AT OTHER_BODY OTHER_FACE: after a click at AT (a face's
# pick place), the preview stays there while the mouse moves along the
# face, off the parts and back: no "Inspect CMD:" line, its end A
# unchanged, the hover still logged; a click on OTHER_BODY's OTHER_FACE
# moves it once, a click at AT again puts it back (mitcad#508).
function Keeps-Place([string]$Cmd, [int[]]$At, [string]$OtherBody, [string]$OtherFace) {
  Ui-Sync
  $endA = Last-EndA $Cmd
  if (-not $endA) { Ui-Fail "no end A in the inspection of the click" }
  Ui-Mark
  Ui-Step 'the mouse along the face'     { [UiInput]::Move($script:UiWindow, ($At[0] + 12), $At[1], 0x04) }
  Ui-Step 'the mouse off the parts'      { Off-Parts (View-Area) }
  Ui-ExpectNew "BUILD hover pick: off the parts' faces" 'the hover off the parts'
  Ui-Step 'the mouse on the face again'  { [UiInput]::Move($script:UiWindow, $At[0], $At[1], 0x04) }
  No-Inspect $Cmd 'no preview and no inspection while the mouse moves after the click'
  if ((Last-EndA $Cmd) -ne $endA) { Ui-Fail "the preview's end A moved from $endA" }
  Write-Host "ok   the preview's end A still at $endA"
  Ui-Mark
  Ui-Step "a click on $OtherBody's $OtherFace" { Pick-Face $OtherBody $OtherFace }
  Moved-Once $Cmd $endA 'a click on another face moves the preview once'
  Ui-Mark
  Ui-Step 'a click at the first place again' { Ui-Click $At[0] $At[1] }
  Ui-ExpectNew "Inspect ${Cmd}: " 'previewed at the first click again'
  Ui-Sync
  if ((Last-EndA $Cmd) -ne $endA) { Ui-Fail "the click at the first place again: end A not at $endA" }
  Write-Host "ok   the click at the first place again: end A at $endA"
}
# Marker-At CMD LABEL: where CMD's last preview marker LABEL (a regex; also
# the taken one, "> LABEL") is; its point is on the face picked, so a click
# there clicks that face (tools/ui-windows-build-test.ps1).
function Marker-At([string]$Cmd, [string]$Label) {
  $line = Ui-LogLines | Where-Object { $_ -match "^Preview marker $Cmd (> )?$Label world " } | Select-Object -Last 1
  if ("$line" -notmatch ' at (\d+),(\d+)$') { Ui-Fail "no preview marker '$Label' of $Cmd logged" }
  return @([int]$Matches[1], [int]$Matches[2])
}
# Off-Parts AREA: the mouse off the parts, to the view's top left corner
# (AREA: x y width height of the view; tools/ui-windows-build-test.ps1).
function Off-Parts([int[]]$Area) {
  [UiInput]::Move($script:UiWindow, ($Area[0] + 6), ($Area[1] + 6), 0x04)
}
function View-Area {
  $area = @((Ui-LastMatch 'View area (\d+ \d+ \d+ \d+)') -split ' ' | ForEach-Object { [int]$_ })
  if ($area.Count -lt 4) { Ui-Fail 'the app did not log its view area' }
  return , $area
}
# Pick-Markers CMD AREA: the mouse off the parts (Off-Parts) and the pick's
# markers logged (the hover previews nothing, mitcad#508).
function Pick-Markers([string]$Cmd, [int[]]$Area) {
  Off-Parts $Area
  Ui-Sync
  $lines = @(Ui-LogLines)
  $inspect = 0
  for ($j = $lines.Count - 1; $j -ge 0; $j--) { if ($lines[$j].StartsWith("Inspect ${Cmd}: ")) { $inspect = $j; break } }
  for ($i = 0; $i -lt 25; $i++) {
    if (Ui-LogLines | Select-Object -Skip $inspect | Where-Object { $_.StartsWith("Preview marker $Cmd ") }) { return }
    Start-Sleep -Milliseconds 100
  }
}
# Marker-InMiddle CMD LABEL: where CMD's marker LABEL is once it lies in the
# middle half of the view, the view zoomed step by step at its centre until
# then (tools/ui-windows-build-test.ps1).
function Marker-InMiddle([string]$Cmd, [string]$Label) {
  $area = @((Ui-LastMatch 'View area (\d+ \d+ \d+ \d+)') -split ' ' | ForEach-Object { [int]$_ })
  if ($area.Count -lt 4) { Ui-Fail 'the app did not log its view area' }
  $cx = [int]($area[0] + $area[2] / 2)
  $cy = [int]($area[1] + $area[3] / 2)
  $sign = -1
  $last = $null
  for ($i = 0; $i -lt 16; $i++) {
    Pick-Markers $Cmd $area
    $at = Marker-At $Cmd $Label
    $off = [Math]::Max([Math]::Abs($at[0] - $cx) / $area[2], [Math]::Abs($at[1] - $cy) / $area[3])
    if ($off -lt 0.25) { return $at }
    if ($null -ne $last -and $off -gt $last) { $sign = -$sign }
    $last = $off
    [UiInput]::Move($script:UiWindow, $cx, $cy, 0x04)
    Start-Sleep -Milliseconds 80
    [UiInput]::Wheel($script:UiWindow, $cx, $cy, $sign)
    Ui-Sync
    Start-Sleep -Milliseconds 500
  }
  Ui-Fail "${Cmd}'s marker '$Label' did not come into the middle of the view"
}
# Wheel-Choice CMD INPUT STEPS: a panel drop-down moved by STEPS entries
# (positive: down the list), one wheel step at a time.
function Wheel-Choice([string]$Cmd, [string]$Choice, [int]$Steps) {
  for ($i = 0; $i -lt [Math]::Abs($Steps); $i++) {
    Ui-WheelLogged "Panel $Cmd input $Choice" ([Math]::Sign(-$Steps))
    Start-Sleep -Milliseconds 150
  }
}
# Corner CMD BODY TURN [LENGTH]: a part on BODY's end B turned towards TURN
# (a host axis: "-Y") by a click on the end face's band along that edge (its
# zone's marker), with a shared mitre, LENGTH mm long (none: the auto length
# to the face, mitcad#485).
function Corner([string]$Cmd, [string]$Body, [string]$Turn, [string]$Length) {
  Refresh-At $Cmd
  $ends = @(Faces-Of $Body ':end_b')
  if ($ends.Count -eq 0) { Ui-Fail "no end B of $Body to pick" }
  Ui-Mark
  Ui-Step "$Body's end B, in the middle"  { Pick-Face $Body $ends[0] }
  Ui-ExpectNew 'BUILD end click: straight towards ' 'the end picked, straight on'
  $at = Marker-InMiddle $Cmd ('turn ' + [regex]::Escape($Turn))
  Ui-Mark
  Ui-Step "near its edge towards $Turn"  { Ui-Click $at[0] $at[1] }
  Ui-ExpectNew "BUILD end click: edge towards $Turn" "turned towards $Turn"
  # The panel keeps the mitre chosen for the corner before.
  $line = $null
  for ($i = 0; $i -lt 30 -and -not $line; $i++) {
    $line = Ui-LogLines | Select-Object -Skip $script:UiMark |
      Where-Object { $_.StartsWith("Inspect ${Cmd}: ") -and $_.Contains("; end edge towards $Turn") } | Select-Object -Last 1
    if (-not $line) { Start-Sleep -Milliseconds 200 }
  }
  if (-not "$line".Contains('mitre shared')) {
    Ui-Mark
    Ui-Step 'Mitre: Shared'              { Wheel-Choice $Cmd 'mitre' 2 }
  }
  Expect-Inspect $Cmd @("; end edge towards $Turn", 'mitre shared, offset 0.0 mm') 'a shared mitre at the corner'
  Ui-Mark
  if ($Length) {
    Ui-Step "$Length mm long"            { Ui-TypeIn "Panel $Cmd input length" $Length }
    Expect-Inspect $Cmd @("length $Length.00 mm (typed)", "; end edge towards $Turn", 'mitre shared') "$Length mm, the corner mitred"
  } else {
    Ui-Step 'Auto: to the face'          { Ui-ClickLogged "Panel $Cmd input length_mode=face" }
  }
}

# A new design (mitcad#394): Component1 active.
Ui-StartApp 'timber-new'
Ui-Mark
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-ExpectNew 'Ribbon BUILD/PROFILE at' 'its PROFILE group'
Ui-ExpectLog 'Ribbon button build.timber at' 'the Timber button on the BUILD tab'

Write-Host '--- 1. The pickers, the first start'
Ui-Mark
Ui-Step 'Profile (F3)'                   { Ui-Key 'F3' }
Ui-ExpectNew 'BUILD parts profile: 1 (0 in the design)' "Profile's picker: the slot profile only, no timber"
Ui-ExpectNew 'Panel Profile tile part: Profile 20 test' 'Profile starts with the slot profile'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'the Timber button'              { Ui-ClickLogged 'Ribbon button build.timber' }
Ui-ExpectNew 'Command Timber started' 'Timber started'
Ui-ExpectNew 'BUILD parts timber: 4 (0 in the design)' "Timber's picker: the members' four sizes (mitcad#484), no profile"
Ui-ExpectNew "BUILD timber starts with test-timber/timber-48-test: the first time's dimensioned 48x98" 'the dimensioned 48x98 the first time'
Ui-ExpectNew 'BUILD timber adds to Component1, member' 'On a member at first (mitcad#485)'
Ui-ExpectNew "BUILD timber starts: length 3000 mm (the part's default), typed; along x, flat" "a typed length, the part's default, along X, flat"
Expect-Inspect 'Timber' @('place grounded, length 3000.00 mm (typed)', '; along x, flat', '; thickness along Z',
                          '; end A at 0.0, 0.0, 24.0', '; part Timber 48x98 test') `
  'the preview: 3000 mm along X, lying flat, resting on the XY plane (mitcad#492)'

Write-Host '--- 2. On a member, the next start'
Ui-Step '600 mm long'                    { Ui-TypeIn 'Panel Timber input length' '600' }
Ui-Mark
Ui-Step 'standing (Z)'                   { Ui-ClickLogged 'Panel Timber input axis=z' }
Expect-Inspect 'Timber' @('place grounded, length 600.00 mm (typed)', '; along z') 'a post at the origin, standing'
Place 'Timber 48x98 test'
$post = Placed-Body
if (-not $post) { Ui-Fail 'no body of the post' }
# After Place no ghost (mitcad#513): the next member waits for a click on a
# member, nothing previewed at the origin on top of the post.
Expect-Inspect 'Timber' @('failed: Click a member''s side or end.') 'after Place the panel waits for a click on a member'
No-Ghost 'Timber' 'place grounded' 'no member previewed at the origin on top of the post'
Refresh-Picks 'at'
$postSides = @(Faces-Of $post $side)
if ($postSides.Count -eq 0) { Ui-Fail "no side of the post ($post) to pick" }
# Nothing follows the mouse (mitcad#508): the hover names the face a click
# picks; once the side is picked, the mouse moving runs no placement and
# no inspection, and a click on another face moves the preview once.
$postAt = Pick-At $post $postSides[0]
Hover-Names 'Timber' $post $postSides[0] $postAt
Ui-Mark
Ui-Step "the post's side"                { Ui-Click $postAt[0] $postAt[1] }
Ui-ExpectNew 'Inspect Timber: place side' "on the post's side, a free offset"
$postOther = @($postSides | Select-Object -Skip 1) + @(Faces-Of $post ':end_b') | Select-Object -First 1
if (-not $postOther) { Ui-Fail "no other face of the post ($post) to click" }
Keeps-Place 'Timber' $postAt $post $postOther
Place 'Timber 48x98 test'
$beam = Placed-Body
if (-not $beam) { Ui-Fail 'no body of the member on the post' }
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber again'
Ui-ExpectNew 'BUILD timber starts with test-timber/timber-48-test: chosen last in the tool' 'the part chosen last'
Ui-ExpectNew 'BUILD timber adds to Component1, member' 'the mode used last'
Ui-ExpectNew 'BUILD timber starts: length 600 mm (typed last), typed; along z, flat' 'the length typed last and the direction'

Write-Host '--- 3. Two points'
Ui-Mark
Ui-Step 'Two points'                     { Ui-ClickLogged 'Panel Timber input how=points' }
Ui-ExpectNew 'View hint Timber: Click where the member starts' "the view's hint for the start"
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Step 'the start pick'                 { Ui-ClickLogged 'Panel Timber input start' }
Ui-Mark
$beamSides = @(Faces-Of $beam $side)
if ($beamSides.Count -eq 0) { Ui-Fail "no side of the member ($beam) to start on" }
Ui-Step "the start on the member's side" { Pick-Face $beam $beamSides[0] }
Ui-ExpectNew 'View hint Timber: Click where it ends; Shift+click places at once' 'the end next'
$postSides = @(Faces-Of $post $side)
if ($postSides.Count -eq 0) { Ui-Fail "no side of the post ($post) to end at" }
$at = Pick-At $post $postSides[0]
Ui-Mark
# MK_SHIFT keeps the move in session 0 (tools/ui-windows-build-test.ps1).
Ui-Step "the mouse over the post's side" { [UiInput]::Move($script:UiWindow, $at[0], $at[1], 0x04) }
Start-Sleep -Milliseconds 1500
Ui-Sync
$towards = @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Inspect Timber: points') })
if ($towards.Count -gt 0) { Ui-Fail "a member previewed towards the mouse before the second click: $($towards[0])" }
Write-Host 'ok   nothing follows the mouse before the second click'
Ui-Mark
Ui-Step 'the end there'                  { Ui-Click $at[0] $at[1] }
Ui-ExpectNew 'Inspect Timber: points, length ' 'two clicks give the member'
Ui-ExpectNew '; length by two points' 'its length from the points'
Ui-Mark
Ui-Step 'Tab: on edge'                   { Ui-Key 'Tab' }
Ui-ExpectNew 'Inspect Timber: points, length ' 'previewed again'
Ui-ExpectNew ' mm, edge; ' 'on edge'
Place 'Timber 48x98 test'
Ui-ExpectNew 'by two points, ' 'a member by two points'

Write-Host '--- 4. Spacing'
Ui-Mark
Ui-Step 'Spacing'                        { Ui-ClickLogged 'Panel Timber input how=spacing' }
Ui-ExpectNew 'Panel Timber input distance at' 'its distance'
Ui-Step '200 mm long'                    { Ui-TypeIn 'Panel Timber input length' '200' }
Refresh-Picks 'at'
$beamSides = @(Faces-Of $beam $side)
if ($beamSides.Count -eq 0) { Ui-Fail "no side of the member ($beam) for the set" }
# The hover shows only the face a click picks (mitcad#489): no set laid out
# while the mouse moves, no inspection asked.
$at = Pick-At $beam $beamSides[0]
Hover-Names 'Timber' $beam $beamSides[0] $at
Ui-Mark
Ui-Step "the member's side"              { Ui-Click $at[0] $at[1] }
Ui-ExpectNew 'Inspect Timber: place side' 'on its side'
Ui-ExpectNew '; set of ' 'the set counted'
Ui-ExpectNew 'centre distance 400 mm' 'at 400 mm'
$at400 = Set-Previewed 'centre distance 400 mm' 'at 400 mm'
# Once picked the set stays there while the mouse moves (mitcad#508); a
# click on another face moves it once, a click at the first place back.
$setOther = $null
$setOtherBody = $beam
if ($beamSides.Count -gt 1) { $setOther = $beamSides[1] }
else {
  $setOtherBody = $post
  $setOther = @(Faces-Of $post $side) | Select-Object -First 1
}
if (-not $setOther) { Ui-Fail "no other face of the member ($beam) or the post ($post) to click" }
Keeps-Place 'Timber' $at $setOtherBody $setOther
$back400 = Set-Previewed 'centre distance 400 mm' 'at the first place again'
if ($back400 -ne $at400) { Ui-Fail "the set at the first place again previewed $back400 members, not the $at400 before" }
Ui-Mark
Ui-Step 'a distance of 200'              { Ui-TypeIn 'Panel Timber input distance' '200' }
$at200 = Set-Previewed 'centre distance 200' 'at 200'
if ($at200 -le $at400) { Ui-Fail "200 mm previewed $at200 members, not more than the $at400 at 400 mm" }
# The Offset typed after the click moves the set (mitcad#492): from its
# reference it starts 300 mm in, fewer members; back to 0, as before.
# The reference is chosen first: the click's depends on where on the side
# the pick place lies (the middle here: 300 mm from it ran off the beam,
# clamped to end B, mitcad#740). From End A, the list's first entry: a
# drop-down's list cannot open in session 0, so wheel steps up the list,
# one message a step (tools/ui-windows-mount-test.ps1); at the top a step
# changes nothing.
Ui-Step 'From End A'                     {
  Ui-WheelLogged 'Panel Timber input from' 1
  Start-Sleep -Milliseconds 150
  Ui-WheelLogged 'Panel Timber input from' 1
}
Ui-Sync
Ui-Mark
Ui-Step 'an offset of 300'               { Ui-TypeIn 'Panel Timber input offset' '300' }
Expect-Inspect 'Timber' @(' at 300.0 mm from a', 'centre distance 200') 'the set previewed 300 mm from end A'
$at300 = Set-Previewed ' at 300.0 mm from a' 'offset 300'
if ($at300 -ge $at200) { Ui-Fail "offset 300 previewed $at300 members, not fewer than the $at200 at offset 0" }
Ui-Mark
Ui-Step 'the offset back to 0'           { Ui-TypeIn 'Panel Timber input offset' '0' }
$again0 = Set-Previewed ' at 0.0 mm from a' 'offset 0 again'
if ($again0 -ne $at200) { Ui-Fail "offset 0 previewed $again0 members, not the $at200 before" }
Place 'Timber 48x98 test'
Ui-ExpectNew '; a set of ' 'placed as one set'
$set = Ui-LastMatch '; a set of (\d+)'
if ([int]"0$set" -ne $at200) { Ui-Fail "the set placed $set members, the preview showed $at200" }
Write-Host "ok   placed the $set members previewed"
$setBody = Placed-Body
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'Spacing Set (F11)'              { Ui-Key 'F11' }
Ui-ExpectNew 'Command Spacing Set started' 'Spacing Set started'
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Step "Spacing Set's member"           { Ui-ClickLogged 'Panel Spacing Set input part' }
$setFaces = @(Faces-Of $setBody $side)
if ($setFaces.Count -eq 0) { Ui-Fail "no face of a member of the set ($setBody)" }
Ui-Mark
Ui-Step 'a member of the set'            { Pick-Face $setBody $setFaces[0] }
Ui-ExpectNew 'Inspect Spacing Set: set ' 'the set'
Ui-Step 'a distance of 150'              { Ui-TypeIn 'Panel Spacing Set input distance' '150' }
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Spacing set of ' 'laid out again'
$again = Ui-LastMatch '^Spacing set of .*: (\d+) member'
if ([int]"0$again" -le [int]"0$set") { Ui-Fail "150 mm gave $again members, not more than the $set at 400 mm" }
Write-Host "ok   $set members at 400 mm, $again at 150 mm"

Write-Host '--- 5. Connector at a timber junction'
Ui-Mark
Ui-Step 'Connector (F8)'                 { Ui-Key 'F8' }
Ui-ExpectNew 'Inspect Connector: junction ' 'a junction chosen'
Ui-ExpectNew 'preview Wood screw head test with 2 fastener(s)' 'two sized countersunk wood screws by default (mitcad#516)'
Ui-ExpectNew ', size ' 'sized from the members'
Ui-Mark
Ui-Step 'Connect (Enter)'                { Ui-Key 'Return' }
Ui-ExpectNew 'Connected Wood screw head test at junction ' 'one click finishes the joint' 30
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 6. Profile again, the cut list'
Ui-Mark
Ui-Step 'Profile (F3)'                   { Ui-Key 'F3' }
Ui-ExpectNew 'BUILD parts profile: 1 (0 in the design)' "the design's timber is not in Profile's picker"
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'Cut List (F9)'                  { Ui-Key 'F9' }
Ui-FocusDialog '^Cut List$'
Ui-ExpectNew 'Cut list row: ' "the cut list's rows"
Ui-ExpectNew 'x Timber 48x98 test' 'the members in it'
Ui-ExpectNew 'Cut list timber: ' 'the timber columns and totals'
Ui-ExpectNew 'Cut list timber row: Timber 48x98 test' "a member's material and sizes"
Ui-ExpectNew 'columns Quantity, Part, Length (mm)' 'the timber columns after the others'
Ui-Step 'close (Escape)'                 { Ui-Key 'Escape' }
Ui-FocusMain

Write-Host '--- 7. Every size a tile of its own, the families as groups'
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber again'
Ui-Mark
Ui-Step 'the part picker (the Part tile)' { Ui-ClickLogged 'Panel Timber input part' }
Ui-ExpectNew 'Part picker opened: 4 parts for build.timber' 'the picker lists every size'
Ui-ExpectNew 'Part picker group All parts: 4 parts, first test-timber/timber-48-test (48x98)' 'all four sizes, the standard ones first'
# The use count is per size (mitcad#518): the design's members are all
# 48x98, so only that tile has a count, not every size of the family.
$tiles = $null
for ($i = 0; $i -lt 30 -and "$tiles" -notmatch 'timber-48-test \(48x98\) x\d+'; $i++) {
  $tiles = Ui-LastMatch '^Part picker tiles: (.*)$'
  if ("$tiles" -notmatch 'timber-48-test \(48x98\) x\d+') { Start-Sleep -Milliseconds 200 }
}
if ("$tiles" -notmatch 'timber-48-test \(48x98\) x\d+') { Ui-Fail "the 48x98 tile has no count: '$tiles'" }
if ("$tiles" -match '\(48x148\) x|\(48x198\) x|timber-28-test x') {
  Ui-Fail "a size the design does not use has a count: '$tiles'"
}
Write-Host "ok   only the size the design uses has a count: $tiles"
foreach ($tile in @('test-timber/timber-48-test (48x98)', 'test-timber/timber-48-test (48x148)',
                    'test-timber/timber-48-test (48x198)', 'test-timber/timber-28-test')) {
  Ui-ExpectNew "Part picker tile $tile at " "a tile of $tile"
}
Ui-ExpectNew 'Part picker group row Dimensioned timber at ' 'the dimensioned timber a group of its own'
Ui-ExpectNew 'Part picker group row Planed timber at ' 'the planed timber a group of its own'
Ui-ExpectLog 'BUILD preview image of test-timber/timber-48-test: 16x16' "the part's picture in the panel's tile"
# A size's star stars that size only (mitcad#491), not its family's rows.
Ui-Mark
Ui-Step 'the 48x148 tile'                { Picker-Click 'Part picker tile test-timber/timber-48-test (48x148)' }
Ui-ExpectNew 'Part picker chose test-timber/timber-48-test (48x148)' '48x148 chosen'
# The panel reads a picture once a session: the row's own, now.
Ui-ExpectNew 'BUILD preview image of test-timber/timber-48-test (48x148): 24x24' "the row's own picture in the tile"
$tilesBefore = Ui-LastMatch '^Part picker tiles: (.*)$'
Ui-Mark
Ui-Step 'star it (Star)'                 { Picker-Click 'Part picker place star' }
Ui-ExpectNew 'Part picker star timber-48-test (48x148) on' 'the size 48x148 starred'
# A star changes only the star (mitcad#517): the tiles stay where they are
# and the selection on the starred one; the size comes first once the list
# is made again (Favourites, then All parts below).
Ui-ExpectNew "Part picker tiles kept: $tilesBefore; current test-timber/timber-48-test (48x148)" 'the tiles stay in their places, the selection on the size'
Ui-Mark
Ui-Step 'Favourites'                     { Picker-Click 'Part picker group row Favourites' }
Ui-ExpectNew 'Part picker group Favourites: 1 parts, first test-timber/timber-48-test (48x148)' 'Favourites lists that size alone'
Ui-Mark
Ui-Step 'All parts'                      { Picker-Click 'Part picker group row All parts' }
Ui-ExpectNew 'Part picker group All parts: 4 parts, first test-timber/timber-48-test (48x148)' 'every size again'
Ui-ExpectNew 'Part picker tile test-timber/timber-48-test (48x98) at ' "48x98's tile after the starred one"
Ui-Mark
Ui-Step 'the 48x98 tile again'           { Picker-Click 'Part picker tile test-timber/timber-48-test (48x98)' }
Ui-ExpectNew 'Part picker chose test-timber/timber-48-test (48x98)' '48x98 chosen again'
Ui-Step 'the search field'               { Picker-Click 'Part picker place search' }
Ui-Mark
Ui-Step 'search 48x148'                  { Picker-Type '48x148' }
Ui-ExpectNew "Part picker search '48x148'" 'searched by the size'
Ui-ExpectNew ': 1 parts, first test-timber/timber-48-test (48x148)' 'the search finds the size'
Ui-ExpectNew 'Part picker tile test-timber/timber-48-test (48x148) at ' 'its tile'
Ui-Mark
Ui-Step 'the 48x148 tile'                { Picker-Click 'Part picker tile test-timber/timber-48-test (48x148)' }
Ui-ExpectNew 'Part picker chose test-timber/timber-48-test (48x148)' '48x148 chosen'
Ui-ExpectNew 'Panel Timber tile part: Timber 48x98 test (48x148)' "the panel's tile names the size"
Ui-ExpectLog 'BUILD preview image of test-timber/timber-48-test (48x148): 24x24' "the row's own picture (read when first chosen)"
Ui-Step 'On a member'                    { Ui-ClickLogged 'Panel Timber input how=member' }
Refresh-Picks 'at'
# A side of any member a click picks now.
$lines = @(Ui-LogLines)
$from = 0
for ($i = $lines.Count - 1; $i -ge 0; $i--) { if ($lines[$i].Contains('Pick places:')) { $from = $i; break } }
$onSide = $lines[$from..($lines.Count - 1)] | ForEach-Object {
  if ($_ -match '^\s*Pick face (\S+)/(\S*:part\(F2:side\(\S*) at \d+,\d+$') { , @($Matches[1], $Matches[2]) } } |
  Select-Object -First 1
if (-not $onSide) { Ui-Fail 'no side of a member to place on' }
Ui-Mark
Ui-Step "a member's side"                { Pick-Face $onSide[0] $onSide[1] }
Ui-ExpectNew 'Inspect Timber: place side' "on a member's side"
Place 'Timber 48x98 test'
$length = [double](Ui-LastMatch '^Placed Timber 48x98 test as .* L=([0-9.]+)')
$volume = [double](Ui-LastMatch '^New body .* \(F[0-9]+\.b[0-9]+\): volume ([0-9.]+) mm3')
$wanted = 148 * 48 * $length
if ([math]::Abs($volume - $wanted) -gt 1) { Ui-Fail "the 48x148 member is $volume mm3, not 148 x 48 x $length = $wanted" }
Write-Host "ok   placed as 48x148: $volume mm3 (148 x 48 x $length)"
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
# Two sizes of one family in the design: each tile counts its own uses
# (mitcad#518), the sizes not used have none.
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber again'
Ui-Mark
Ui-Step 'the part picker (the Part tile)' { Ui-ClickLogged 'Panel Timber input part' }
Ui-ExpectNew 'Part picker opened: 4 parts for build.timber' 'the picker again'
# The search 48x148 above stays in the picker (it shows the part chosen):
# cleared, every size is a tile again.
Ui-Step 'the search field'               { Picker-Click 'Part picker place search' }
Ui-Mark
Ui-Step 'clear the search'               { Picker-Keys @(0x23) 1; Picker-Keys @(0x08) 6 }
Ui-ExpectNew "Part picker search ''" 'the search cleared'
Ui-ExpectNew 'Part picker group All parts: 4 parts' 'every size again'
$tiles = $null
for ($i = 0; $i -lt 30 -and "$tiles" -notmatch 'timber-48-test \(48x148\) x1'; $i++) {
  $tiles = Ui-LastMatch '^Part picker tiles: (.*)$'
  if ("$tiles" -notmatch 'timber-48-test \(48x148\) x1') { Start-Sleep -Milliseconds 200 }
}
if ("$tiles" -notmatch 'timber-48-test \(48x148\) x1') { Ui-Fail "the 48x148 tile does not count its one member: '$tiles'" }
if ("$tiles" -notmatch 'timber-48-test \(48x98\) x\d+') { Ui-Fail "the 48x98 tile lost its count: '$tiles'" }
if ("$tiles" -match '\(48x198\) x|timber-28-test x') { Ui-Fail "a size the design does not use has a count: '$tiles'" }
Write-Host "ok   per-size counts: $tiles"
Ui-Mark
Ui-Step 'In this design'                 { Picker-Click 'Part picker group row In this design' }
Ui-ExpectNew 'Part picker group In this design: 2 parts, first test-timber/timber-48-test (48x148)' 'In this design lists only the two sizes the design uses'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

# Frame-Of-Four CMD KEY PART L1 L2 [-Through]: a frame on the XY plane, L1
# along X and L2 across, of four members (PART) each on the end of the
# one before with a shared mitre (mitcad#485): the first along X at the
# origin, the second turned to -Y, the third to -X; the view then from the
# front left (the third's end faces -X), the fourth turned to +Y, its auto
# length bound to the first (Tab-stepped to "through" with THROUGH), or
# with TYPED its length typed as L2 (mitcad#492): it closes the corner with
# a shared mitre, L2 long.
function Frame-Of-Four([string]$Cmd, [string]$Key, [string]$Part, [string]$L1, [string]$L2, [switch]$Through,
                       [switch]$Typed) {
  Ui-Mark
  Ui-Step "$Cmd ($Key)"                  { Ui-Key $Key }
  Ui-ExpectNew "Command $Cmd started" "$Cmd started"
  Ui-Step "$L1 mm long"                  { Ui-TypeIn "Panel $Cmd input length" $L1 }
  Ui-Mark
  Ui-Step 'along X'                      { Ui-ClickLogged "Panel $Cmd input axis=x" }
  Expect-Inspect $Cmd @("place grounded, length $L1.00 mm (typed)", '; along x') "the first along X"
  Place $Part
  $m1 = Placed-Body
  $m1Name = Ui-LastMatch '^Placed .* as (.*) \(O[0-9/]*\), length'
  if (-not $m1) { Ui-Fail 'no body of the first member' }
  Corner $Cmd $m1 '-Y' $L2
  if ($Cmd -eq 'Timber') {
    Expect-Inspect $Cmd @('place end', '; thickness along Z') 'on the end lying flat as the first: ends of the same width'
  }
  Place $Part
  $m2 = Placed-Body
  Corner $Cmd $m2 '-X' $L1
  Place $Part
  $m3 = Placed-Body
  # The third one's end faces -X: the view from the front left (the cube's
  # places logged, MITCAD_LOG_ORIENTATION_CUBE).
  Ui-Step 'the view from the front left' { Ui-ClickLogged 'Orientation cube front-left-top' }
  Ui-Sync
  Start-Sleep -Milliseconds 800
  Corner $Cmd $m3 '+Y' $(if ($Typed) { $L2 } else { '' })
  if ($Through) {
    Ui-Step 'to the centre (Tab)'        { Ui-Key 'Tab' }
    Ui-Mark
    Ui-Step 'through (Tab)'              { Ui-Key 'Tab' }
    Ui-ExpectNew '(bound, auto: through to ' 'bound through the first'
  }
  Expect-Inspect $Cmd @("length $L2.00 mm", "; closes the corner at end A of $m1Name, shared mitre") 'the last one closes the corner with a shared mitre, as long as the side'
  Place $Part
  if (-not $Typed) {
    Ui-ExpectNew 'length bound, auto: ' 'placed with its length bound to the first'
  }
  Write-Host "ok   a frame of $Cmd closed: $m1, $m2, $m3 and the last one"
  Ui-Step 'Close (Escape)'               { Ui-Key 'Escape' }
}

Write-Host '--- 8. A frame of four timber members, mitred'
$env:MITCAD_LOG_ORIENTATION_CUBE = '1'
Ui-StartApp 'timber-frame'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Frame-Of-Four 'Timber' 'F4' 'Timber 48x98 test' '400' '300'

Write-Host '--- 9. A frame of four slot profiles, mitred, the last one through the first'
Ui-StartApp 'timber-profile-frame'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Frame-Of-Four 'Profile' 'F3' 'Profile 20 test' '200' '150' -Through

# Wall-Sides BODIES: the side faces of these bodies a click on Start picks
# now (after the last "Pick places:"), each @(body, face, x, y).
function Wall-Sides([string[]]$Bodies) {
  $lines = @(Ui-LogLines)
  $start = 0
  for ($i = $lines.Count - 1; $i -ge 0; $i--) { if ($lines[$i].Contains('Pick places:')) { $start = $i; break } }
  return @($lines[$start..($lines.Count - 1)] | ForEach-Object {
      if ($_ -match '^Pick face (F[0-9]+\.b[0-9]+)/(\S*:part\(F2:side\(\S*) at (\d+),(\d+)$' -and $Bodies -contains $Matches[1]) {
        , @($Matches[1], $Matches[2], [int]$Matches[3], [int]$Matches[4])
      } })
}
# New-Bodies: the bodies made since the mark.
function New-Bodies {
  return @(Ui-LogLines | Select-Object -Skip $script:UiMark |
    ForEach-Object { if ($_ -match '^New body .* \((F[0-9]+\.b[0-9]+)\): volume') { $Matches[1] } })
}
# Wall-Ends BODIES: in the view from the right (+X), the faces of these
# bodies a click on Start picks now: their end B's face, the plates' ends
# (end_b) first, then the end stud's side; each @(body, face, x, y)
# (mitcad#510). The plates' ends are too small to pick in the fitted view
# from the front right.
function Wall-Ends([string[]]$Bodies) {
  $lines = @(Ui-LogLines)
  $start = 0
  for ($i = $lines.Count - 1; $i -ge 0; $i--) { if ($lines[$i].Contains('Pick places:')) { $start = $i; break } }
  $faces = @($lines[$start..($lines.Count - 1)] | ForEach-Object {
      if ($_ -match '^Pick face (F[0-9]+\.b[0-9]+)/(\S*) at (\d+),(\d+)$' -and $Bodies -contains $Matches[1]) {
        , @($Matches[1], $Matches[2], [int]$Matches[3], [int]$Matches[4])
      } })
  return @(@($faces | Where-Object { $_[1].Contains(':end_b') }) + @($faces | Where-Object { -not $_[1].Contains(':end_b') }))
}
# Wall-From NAME AT LENGTH TEXT STUDS [CORNER]: a wall started by a click
# at AT (x, y) on Wall 1, LENGTH mm long (typed): the panel names where it
# starts (TEXT) and the Corner chosen (CORNER, default butt), the wall is
# placed with as many members as previewed, and Wall 1 gets at least STUDS
# more (a T's studs may take a stud's place).
function Wall-From([string]$Name, [int[]]$At, [string]$Length, [string]$Text, [int]$Studs,
                   [string]$Corner = 'butt') {
  Ui-Mark
  Ui-Step "the start on Wall 1"          { Ui-Click $At[0] $At[1] }
  Ui-ExpectNew 'View hint Wall: Click where the wall ends, or press Enter: Length square to the wall clicked' `
    'the end next, else Length square to the wall'
  Ui-Mark
  Ui-Step "$Length mm long"              { Ui-TypeIn 'Panel Wall input frame_length' $Length }
  Expect-Inspect 'Wall' @("frame wall $Name, wall $Length x 2400 mm", "; $Name`: $Text", "; corner $Corner") `
    "$Name previewed: $Text, corner $Corner"
  $line = Ui-LogLines | Select-Object -Skip $script:UiMark |
    Where-Object { $_.StartsWith('Inspect Wall: ') -and $_.Contains("; $Name`: $Text") } | Select-Object -Last 1
  if ("$line" -notmatch ', (\d+) member\(s\) \(') { Ui-Fail "no member count in: $line" }
  $previewed = [int]$Matches[1]
  Ui-Mark
  Ui-Step 'Place (Enter)'                { Ui-Key 'Return' }
  Ui-ExpectNew "Placed frame $Name (wall, lead " "$Name placed" 60
  Ui-ExpectNew "Frame junction: $Name`: $Text" "$Name placed: $Text"
  $placed = [int](Ui-LastMatch "^Placed frame $Name \(wall, lead [^)]*\): (\d+) member")
  if ($placed -ne $previewed) { Ui-Fail "$Name placed $placed member(s), the preview showed $previewed" }
  $made = @(New-Bodies).Count
  if ($made -lt $placed + $Studs) {
    Ui-Fail "$($made) new bodies after $Name, not its $placed members and at least $Studs junction stud(s) of Wall 1"
  }
  Write-Host "ok   $Name`: $placed members, Wall 1 $($made - $placed) body(ies) more"
}

# First-Wall DESIGN CORNER: in a new design, Wall (mitcad#520) without
# fasteners, Corner CORNER (butt, the default, or lapped: chosen in the
# panel), Wall 1 placed from the origin along +X; its bodies in $Wall1.
function First-Wall([string]$Design, [string]$Corner) {
  Ui-StartApp $Design
  Ui-Step 'the BUILD tab'                { Ui-ClickLogged 'Ribbon tab BUILD' }
  Ui-ExpectLog 'Ribbon BUILD/TIMBER at' 'its TIMBER group (mitcad#520)'
  Ui-ExpectLog 'Ribbon button build.wall at' 'the Wall button'
  Ui-Mark
  Ui-Step 'Wall (F7)'                    { Ui-Key 'F7' }
  Ui-ExpectNew 'Command Wall started' 'Wall started'
  Expect-Inspect 'Wall' @('frame wall Wall 1, wall 3600 x 2400 mm', 'fasteners yes', '; corner butt') `
    'a wall from the origin along +X, Corner Butt the first time'
  Ui-Mark
  Ui-Step 'no fasteners'                 { Ui-ClickLogged 'Panel Wall input frame_fasteners' }
  Expect-Inspect 'Wall' @('frame wall Wall 1, wall 3600 x 2400 mm', 'fasteners no') 'without fasteners'
  if ($Corner -eq 'lapped') {
    Ui-Mark
    Ui-Step 'Corner: Lapped plates'      { Ui-ClickLogged 'Panel Wall input frame_corner=lapped' }
    Expect-Inspect 'Wall' @('frame wall Wall 1, wall 3600 x 2400 mm', '; corner lapped') 'Corner Lapped plates'
  }
  Ui-Mark
  Ui-Step 'Place (Enter)'                { Ui-Key 'Return' }
  Ui-ExpectNew 'Placed frame Wall 1 (wall, lead ' 'the first wall placed' 60
  $script:Wall1 = @(New-Bodies)
  $script:Studs1 = @(Stud-Bodies)
  if ($script:Wall1.Count -lt 4) { Ui-Fail "Wall 1 made $($script:Wall1.Count) bodies" }
}
# Stud-Bodies: the 2304 mm studs (98 x 48: 10838016 mm3) made since the
# mark (mitcad#514).
function Stud-Bodies {
  return @(Ui-LogLines | Select-Object -Skip $script:UiMark | ForEach-Object {
      if ($_ -match '^New body .* \((F[0-9]+\.b[0-9]+)\): volume ([0-9.]+) mm3$' -and
          [Math]::Abs([double]$Matches[2] - 10838016) -lt 1) { $Matches[1] } } | Select-Object -Unique)
}
# Stud-Face BODIES WHICH: @(x, y) of a side face of these studs a click on
# Start picks now (Wall-Sides): WHICH 'first' the leftmost face of all (the
# leftmost stud's front face), 'last' the leftmost face of the rightmost
# stud (its front face, not its end's), 'middle' the rightmost face of the
# middle stud; the studs ordered by their rightmost face (the view from
# the front right: +X and +Y to the right, a stud's +X face right of its
# -Y face).
function Stud-Face([string[]]$Bodies, [string]$Which) {
  $faces = @(Wall-Sides $Bodies)
  if ($faces.Count -lt 1) { Ui-Fail 'no side face of the studs to click' }
  $studs = @($faces | Group-Object { $_[0] } | ForEach-Object {
      $sorted = @($_.Group | Sort-Object { $_[2] })
      [pscustomobject]@{ Lo = $sorted[0]; Hi = $sorted[-1] }
    } | Sort-Object { $_.Hi[2] })
  switch ($Which) {
    'first' { $face = $studs[0].Lo }
    'last' { $face = $studs[-1].Lo }
    default { $face = $studs[[int][Math]::Floor(($studs.Count - 1) / 2)].Hi }
  }
  return @($face[2], $face[3])
}
# Start-Picks: the view fitted, the places a click on Start picks logged.
function Start-Picks {
  Ui-Step 'fit the view (F6)'            { Ui-Key 'F6' }
  Ui-Step "Wall's Start"               { Ui-ClickLogged 'Panel Wall input start' }
}

Write-Host "--- 10. Walls: butting from a wall's side and its end, Lapped plates an L and a T (mitcad#493, mitcad#510)"
First-Wall 'timber-walls' 'butt'
$wall1 = $script:Wall1
# After Place no ghost (mitcad#513): the panel keeps its values, Start and
# End are empty, nothing is previewed from the origin on top of Wall 1 and
# Enter places no second wall there; a click on a start previews the next
# wall there (Wall-From below).
Ui-ExpectNew 'View hint Wall: Click where the next wall starts' 'the view asks where the next wall starts'
Expect-Inspect 'Wall' @('failed: Click where the next wall starts.') 'the panel waits for the next wall''s start'
No-Ghost 'Wall' 'frame wall ' 'no wall previewed at the origin on top of Wall 1 after Place'
Ui-Mark
Ui-Step 'Enter without a click'          { Ui-Key 'Return' }
Ui-ExpectNew 'Wall: OK refused' 'Enter places no second wall at the origin'
No-Ghost 'Wall' 'frame wall ' 'still nothing previewed'
# The owner's case (mitcad#514): a click on its end stud's front face, a
# few centimetres from the corner (+X to the right in the view), comes
# flush with end A, butting, no L: its outer face in the plane of the end
# face. An Offset typed moves the preview once (nothing follows the
# mouse), 0 brings it back flush; placed, a backing stud more in Wall 1.
Start-Picks
$corner = Stud-Face $script:Studs1 'first'
Ui-Mark
Ui-Step "the start near Wall 1's corner" { Ui-Click $corner[0] $corner[1] }
Ui-ExpectNew 'BUILD frame start click: From End A, Offset 0 mm (side)' 'the click fills From and Offset: flush'
Ui-ExpectNew 'View hint Wall: Click where the wall ends, or press Enter: Length square to the wall clicked' `
  'the end next, else Length square to the wall'
Ui-Mark
Ui-Step '3000 mm long'                   { Ui-TypeIn 'Panel Wall input frame_length' '3000' }
Expect-Inspect 'Wall' @('frame wall Wall 2, wall 3000 x 2400 mm', "; Wall 2: from Wall 1's side, flush with end A (butt)",
  '; corner butt', '; length typed') 'Wall 2 previewed flush with end A, the length typed'
Ui-Mark
Ui-Step 'Offset 600'                     { Ui-TypeIn 'Panel Wall input frame_offset' '600' }
Ui-ExpectNew 'BUILD frame offset typed: from End A' 'the offset typed'
Expect-Inspect 'Wall' @("; Wall 2: from Wall 1's side, 600 mm from end A (butt)") 'the preview 600 mm from end A'
Ui-Sync
Ui-Mark
Ui-Step 'the mouse over the wall'        { [UiInput]::Move($script:UiWindow, ($corner[0] + 40), $corner[1], 0x04) }
Ui-Step 'the mouse off the parts'        { Off-Parts (View-Area) }
No-Inspect 'Wall' 'the preview stays where the offset put it while the mouse moves'
Ui-Mark
Ui-Step 'Offset 0'                       { Ui-TypeIn 'Panel Wall input frame_offset' '0' }
Expect-Inspect 'Wall' @("; Wall 2: from Wall 1's side, flush with end A (butt)") 'Offset 0: flush with end A again'
$line = Ui-LogLines | Select-Object -Skip $script:UiMark |
  Where-Object { $_.StartsWith('Inspect Wall: ') -and $_.Contains('; Wall 2: ') } | Select-Object -Last 1
if ("$line" -notmatch ', (\d+) member\(s\) \(') { Ui-Fail "no member count in: $line" }
$previewed = [int]$Matches[1]
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Wall 2 (wall, lead ' 'Wall 2 placed' 60
Ui-ExpectNew "Frame junction: Wall 2: from Wall 1's side, flush with end A (butt)" 'Wall 2 placed flush with end A'
$placed = [int](Ui-LastMatch '^Placed frame Wall 2 \(wall, lead [^)]*\): (\d+) member')
if ($placed -ne $previewed) { Ui-Fail "Wall 2 placed $placed member(s), the preview showed $previewed" }
if (@(New-Bodies).Count -lt $placed + 1) { Ui-Fail "no backing stud more in Wall 1 after Wall 2" }
Write-Host "ok   Wall 2 flush with Wall 1's end A: $placed members, a backing stud more in Wall 1"
# A click on end B's face, seen from the right (+X: only Wall 1's end B
# shows of it): from that end, a butt corner.
Ui-Step 'the view from the right'        { Ui-ClickLogged 'Orientation cube right' }
Ui-Sync
Start-Sleep -Milliseconds 800
Start-Picks
$ends = @(Wall-Ends $wall1)
if ($ends.Count -lt 1) { Ui-Fail "no end face of Wall 1 to click" }
$end = $ends[0]
Wall-From 'Wall 3' @($end[2], $end[3]) '2000' "from Wall 1's end B (butt corner)" 0
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

# Lapped plates: an L corner near end B, a T mid-side (mitcad#493).
First-Wall 'timber-walls-lapped' 'lapped'
$wall1 = $script:Wall1
Start-Picks
$sides = @(Wall-Sides $wall1)
if ($sides.Count -lt 3) { Ui-Fail "only $($sides.Count) side face(s) of Wall 1 to click" }
$right = $sides | Sort-Object { $_[2] } | Select-Object -Last 1
Wall-From 'Wall 2' @($right[2], $right[3]) '3000' "an L corner at Wall 1's end B (lapped plates)" 1 'lapped'
Start-Picks
$sides = @(Wall-Sides $wall1)
if ($sides.Count -lt 3) { Ui-Fail "only $($sides.Count) side face(s) of Wall 1 to click" }
$xs = @($sides | ForEach-Object { $_[2] } | Sort-Object)
$middle = ($xs[0] + $xs[-1]) / 2
$mid = $sides | Sort-Object { [Math]::Abs($_[2] - $middle) } | Select-Object -First 1
Wall-From 'Wall 3' @($mid[2], $mid[3]) '2000' 'a T on Wall 1, ' 1 'lapped'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
# The next start remembers Lapped plates.
Ui-Mark
Ui-Step 'Wall (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Command Wall started' 'Wall started again'
Expect-Inspect 'Wall' @('frame wall Wall 4, ', '; corner lapped') 'Lapped plates remembered'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

# The owner's case of mitcad#515: Wall 2 and Wall 3 flush with Wall 1's
# ends, both 3000 mm to -Y (its front side), 3502 mm apart; a click on the
# +X face of Wall 2's middle stud starts Wall 4 on its side towards Wall
# 3: no End clicked, Length Auto, it previews and is placed between the
# two, butting Wall 3's side, 3453 mm from Wall 2's centre line, 3404 mm
# between the faces.
First-Wall 'timber-walls-run' 'butt'
$studs1 = $script:Studs1
Start-Picks
Wall-From 'Wall 2' (Stud-Face $studs1 'first') '3000' "from Wall 1's side, flush with end A (butt)" 1
$studs2 = @(Stud-Bodies)
Start-Picks
Wall-From 'Wall 3' (Stud-Face $studs1 'last') '3000' "from Wall 1's side, flush with end B (butt)" 1
Start-Picks
$side = Stud-Face $studs2 'middle'
Ui-Mark
Ui-Step "the start on Wall 2's side"     { Ui-Click $side[0] $side[1] }
Ui-ExpectNew 'BUILD frame start click: From ' 'the click fills From and Offset'
Ui-ExpectNew "View hint Wall: Click where the wall ends, or press Enter: to Wall 3's side" `
  'the end next, else to the wall the run meets'
Expect-Inspect 'Wall' @('frame wall Wall 4, wall 3453 x 2400 mm', "; Wall 4: from Wall 2's side, ",
  ", to Wall 3's side (butt), 3404 mm", '; length auto') 'Wall 4 previewed between Wall 2 and Wall 3, no End clicked'
$line = Ui-LogLines | Select-Object -Skip $script:UiMark |
  Where-Object { $_.StartsWith('Inspect Wall: ') -and $_.Contains("to Wall 3's side") } | Select-Object -Last 1
if ("$line" -notmatch ', (\d+) member\(s\) \(') { Ui-Fail "no member count in: $line" }
$previewed = [int]$Matches[1]
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Wall 4 (wall, lead ' 'Wall 4 placed' 60
Ui-ExpectNew ", to Wall 3's side (butt), 3404 mm" 'Wall 4 placed butting Wall 3'
$placed = [int](Ui-LastMatch '^Placed frame Wall 4 \(wall, lead [^)]*\): (\d+) member')
if ($placed -ne $previewed) { Ui-Fail "Wall 4 placed $placed member(s), the preview showed $previewed" }
# Backing studs in both walls it meets.
if (@(New-Bodies).Count -lt $placed + 2) { Ui-Fail "no backing studs in Wall 2 and Wall 3 for Wall 4" }
Write-Host "ok   Wall 4 between Wall 2 and Wall 3: $placed members, no End clicked"
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 11. A frame of four timber members, the last one typed to the first one''s outer side'
Ui-StartApp 'timber-typed-frame'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Frame-Of-Four 'Timber' 'F4' 'Timber 48x98 test' '400' '300' -Typed

Write-Host '--- 12. A board on the ground rests on it; one standing on its top face lies as it does'
Ui-StartApp 'timber-lie'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber started'
Ui-Step '600 mm long'                    { Ui-TypeIn 'Panel Timber input length' '600' }
Ui-Mark
Ui-Step 'along X'                        { Ui-ClickLogged 'Panel Timber input axis=x' }
Expect-Inspect 'Timber' @('place grounded, length 600.00 mm (typed)', '; along x, flat', '; thickness along Z',
                          '; end A at 0.0, 0.0, 24.0') 'flat along X, resting on the XY plane: its axis 24 mm up'
Place 'Timber 48x98 test'
$board = Placed-Body
if (-not $board) { Ui-Fail 'no body of the board' }
Refresh-Picks 'at'
# Its section's side at +Y (the sketch's line c8) faces up when it lies flat.
$tops = @(Faces-Of $board ':part(F2:side(c8[')
if ($tops.Count -eq 0) { Ui-Fail "no top face of the board ($board) to pick" }
Ui-Mark
Ui-Step "the board's top face"           { Pick-Face $board $tops[0] }
Ui-ExpectNew 'Inspect Timber: place side' 'on its top face'
Ui-Mark
Ui-Step 'from end A'                     { Wheel-Choice 'Timber' 'from' -2 }
Ui-Step 'offset 0'                       { Ui-TypeIn 'Panel Timber input offset' '0' }
# Standing up from end A at offset 0, not rotated: its 48 along the board,
# its 98 across the board's 98, its sides flush (it came turned a quarter).
Expect-Inspect 'Timber' @('place side', ' at 0.0 mm from a', '; thickness along X', '; end A at 24.0, 0.0, 48.0') `
  'standing on the top face from end A at 0: its width along the board''s width, flush'
Place 'Timber 48x98 test'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 13. Fasteners: a frame''s and a set''s, Swap Fastener (mitcad#497)'
# Heads-Since VOLUME: the fasteners' head bodies made since the mark (a
# head's volume: 9000 the screw's, 4000 the nail's).
function Heads-Since([string]$Volume) {
  return @(Ui-LogLines | Select-Object -Skip $script:UiMark |
    ForEach-Object { if ($_ -match "^New body .* \((F[0-9]+\.b[0-9]+)\): volume $Volume\b") { $Matches[1] } })
}
# Choose-Fastener CMD INPUT PART: the input's tile opens the part picker, a
# click on the part's tile chooses it.
function Choose-Fastener([string]$Cmd, [string]$Field, [string]$Part) {
  Ui-Mark
  Ui-Step "the $Field tile"              { Ui-ClickLogged "Panel $Cmd input $Field" }
  Ui-ExpectNew 'Part picker opened: ' "the part picker for $Cmd's $Field"
  Ui-ExpectNew "Part picker tile $Part at " "$Part's tile in the picker"
  Ui-Mark
  Ui-Step "$Part in the picker"          { Picker-Click "Part picker tile $Part" }
  Ui-ExpectNew "Part picker chose $Part" "$Part chosen in the picker"
  # The picker stays open beside the panel (input is posted to the main
  # window, which it does not cover) and closes with the command.
}
Ui-StartApp 'timber-fasteners'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-Mark
Ui-Step 'Wall (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Command Wall started' 'Wall started'
Expect-Inspect 'Wall' @('frame wall Wall 1', 'fastener 2 x Wood screw head test at each junction, sized from the members') `
  ('the first time (fresh settings): two of the first countersunk wood screw, sized from the members, not the ' +
   'pan-head connector screw whose name comes first (mitcad#516)')
# The frame: three 5.0x90 screws with heads at each junction.
Choose-Fastener 'Wall' 'fastener' 'test-timber/wood-screw-head-test'
Ui-Mark
Ui-Step 'Size 5.0x90'                    { Wheel-Choice 'Wall' 'fastener_size' 2 }
Ui-Step 'three at each junction'         { Ui-TypeIn 'Panel Wall input fastener_count' '3' }
Ui-Step '1800 mm long'                   { Ui-TypeIn 'Panel Wall input frame_length' '1800' }
Ui-Step '600 mm high'                    { Ui-TypeIn 'Panel Wall input frame_height' '600' }
Expect-Inspect 'Wall' @('frame wall Wall 1, wall 1800 x 600 mm', 'fastener 3 x Wood screw head test 5.0x90 at each junction') `
  'a chosen fastener and size, three at each junction'
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Wall 1 (wall, lead ' 'the wall placed with its screws' 60
$fastened = [int](Ui-LastMatch '^Placed frame Wall 1 \(wall, lead [^)]*\): \d+ member\(s\), (\d+) junction')
$screws = @(Heads-Since '9000')
if ($fastened -lt 1 -or $screws.Count -ne 3 * $fastened) {
  Ui-Fail "$($screws.Count) screw(s) at $fastened junction(s), not three at each"
}
Write-Host "ok   $fastened junction(s), three 5.0x90 screws each"
$seats = @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_ -match '^Timber fastener .*: head top ' })
if ($seats.Count -ne $screws.Count -or @($seats | Where-Object { $_ -notmatch 'head top 0 mm from entry face$' }).Count -gt 0) {
  Ui-Fail "the physical screw heads are not flush within 1e-6 mm: $($seats -join ' / ')"
}
Write-Host 'ok   every frame screw head is flush within 1e-6 mm'
$wall = @(New-Bodies | Where-Object { $screws -notcontains $_ })

# A set on the wall's top plate (its highest side face), fastened to it:
# Timber's Spacing, with the fastener chosen in Wall (shared by the TIMBER
# group's tools, mitcad#520).
Ui-Step 'Close Wall (Escape)'            { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber started'
Ui-Step 'Mode: Spacing'                  { Ui-ClickLogged 'Panel Timber input how=spacing' }
Ui-Step '300 mm long'                    { Ui-TypeIn 'Panel Timber input length' '300' }
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Step "Timber's Place On again"        { Ui-ClickLogged 'Panel Timber input at' }
$top = @(Wall-Sides $wall) | Sort-Object { $_[3] } | Select-Object -First 1
if (-not $top) { Ui-Fail 'no side face of the wall to set the members on' }
Ui-Mark
Ui-Step 'the top plate''s top'           { Ui-Click $top[2] $top[3] }
Expect-Inspect 'Timber' @('place side', '; set of ', '; fasteners no') 'a set on the plate, no fasteners by default'
Ui-Mark
Ui-Step 'Fasteners on'                   { Ui-ClickLogged 'Panel Timber input set_fasteners' }
Expect-Inspect 'Timber' @('place side', '; set of ', '; fasteners 3 x Wood screw head test 5.0x90 at each junction') `
  'a set on the plate, fastened with the fastener chosen last'
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed Timber 48x98 test as ' 'the set placed' 60
$set = Ui-LastMatch '^Placed Timber 48x98 test as .*; a set of (\d+, \d+) junction'
$counts = "$set" -split ', '
if ($counts.Count -ne 2 -or [int]$counts[0] -lt 2 -or $counts[0] -ne $counts[1]) {
  Ui-Fail "not every member of the set fastened to the plate: $set"
}
Write-Host "ok   a set of $($counts[0]), each member fastened"
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

# Swap Fastener: two of the frame's screws, the head highest in the view and
# the next highest at another junction. In session 0's low window a head is
# a few pixels wide and the three of a junction overlap, so a click on the
# middle of one there may pick another's corner: two clicks at one junction
# can take the same screw.
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Sync
$placed = @($screws | ForEach-Object {
    $body = $_
    $line = Ui-LogLines | Where-Object { $_ -match "^Body $([regex]::Escape($body)) in [O\d/]+ at \d+,\d+$" } | Select-Object -Last 1
    if ("$line" -match '^Body (.*) at (\d+),(\d+)$') { , @($Matches[1], [int]$Matches[2], [int]$Matches[3]) }
  } | Sort-Object { $_[2] })
$heads = @()
if ($placed.Count -gt 0) {
  $first = $placed[0]
  $other = $placed | Where-Object { [Math]::Abs($_[1] - $first[1]) + [Math]::Abs($_[2] - $first[2]) -gt 40 } | Select-Object -First 1
  if ($other) { $heads = @($first, $other) }
}
if ($heads.Count -ne 2) { Ui-Fail 'no two screw heads at two junctions in the view' }
Ui-Step 'click a screw'                  { Ui-Click $heads[0][1] $heads[0][2] }
Ui-Step 'Ctrl+click another'             { Ui-ClickWith 'ctrl' $heads[1][1] $heads[1][2] }
# The context menu is filled and logged: its Build has Swap Fastener for
# the screws. It stays open and takes the keys (as in
# tools/ui-windows-browser-open-test.ps1), so it is chosen with them, as on
# Linux with the mouse: Build is its third entry, Swap Fastener the first of
# its submenu.
Ui-Mark
Ui-Step 'right-click it'                 { Ui-Click $heads[1][1] $heads[1][2] 3 }
Ui-ExpectNew 'Context menu Build: ' 'the context menu''s Build'
$menu = Ui-LastMatch '^Context menu Build: (.*)$'
if (-not "$menu".StartsWith('Swap Fastener')) { Ui-Fail "Build has no Swap Fastener first: $menu" }
$entries = Ui-LastMatch '^Context menu: (.*)$'
if (-not "$entries".StartsWith('Repeat Timber | Solid | Build |')) { Ui-Fail "Build is not the menu's third entry: $entries" }
Write-Host 'ok   the context menu''s Build offers Swap Fastener'
Ui-Mark
Ui-Step 'BUILD > Swap Fastener'          { Ui-Key 'Down'; Ui-Key 'Down'; Ui-Key 'Down'; Ui-Key 'Right'; Ui-Key 'Return' }
Ui-ExpectNew 'Command Swap Fastener started' 'Swap Fastener started'
Ui-ExpectNew 'BUILD swap fastener starts with 2 fastener(s), test-timber|wood-screw-head-test' `
  'with the two screws selected, their own part'
Choose-Fastener 'Swap Fastener' 'part' 'test-timber/ring-nail-test'
Ui-Mark
Ui-Step 'Size 3.1x90'                    { Wheel-Choice 'Swap Fastener' 'fastener_size' 2 }
Expect-Inspect 'Swap Fastener' @('swap 2 fastener(s) (wood, 2 x Wood screw head test 5.0x90)', 'for Ring nail test 3.1x90') `
  'two screws to become 3.1x90 ring nails'
Ui-Mark
Ui-Step 'Swap (Enter)'                   { Ui-Key 'Return' }
Ui-ExpectNew 'Swapped 2 fastener(s) for Ring nail test 3.1x90 (3.1x90)' 'the two screws are ring nails' 30
Ui-ExpectNew 'head top 0 mm from entry face' 'the swapped nails are seated flush'
$seats = @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_ -match '^Timber fastener .*: head top ' })
if ($seats.Count -eq 0 -or @($seats | Where-Object { $_ -notmatch 'head top 0 mm from entry face$' }).Count -gt 0) {
  Ui-Fail "a swapped fastener stands proud: $($seats -join ' / ')"
}
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Swap 2 fastener(s) for Ring nail test 3.1x90' 'one undo step takes the swap back'

Write-Host '--- 14. Two points from one stud''s side to the next one''s (mitcad#509)'
Ui-StartApp 'timber-between'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-Mark
Ui-Step 'Wall (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Command Wall started' 'Wall started'
Expect-Inspect 'Wall' @('frame wall Wall 1, wall 3600 x 2400 mm', 'fasteners yes') 'a wall from the origin along +X'
Ui-Mark
Ui-Step 'no fasteners'                   { Ui-ClickLogged 'Panel Wall input frame_fasteners' }
Expect-Inspect 'Wall' @('frame wall Wall 1, wall 3600 x 2400 mm', 'fasteners no') 'without fasteners'
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Wall 1 (wall, lead ' 'the wall placed' 60
# Its studs: the bodies of the volume most of its members have.
$volumes = @{}
foreach ($line in @(Ui-LogLines | Select-Object -Skip $script:UiMark)) {
  if ($line -match '^New body .* \((F[0-9]+\.b[0-9]+)\): volume ([0-9.]+) mm3$') {
    $volumes[$Matches[1]] = [string][Math]::Round([double]$Matches[2])
  }
}
$studVolume = ($volumes.Values | Group-Object | Sort-Object Count -Descending | Select-Object -First 1).Name
$studs = @($volumes.Keys | Where-Object { $volumes[$_] -eq $studVolume })
if ($studs.Count -lt 3) { Ui-Fail "only $($studs.Count) stud(s) of one size in the wall" }
Ui-Step 'Close Wall (Escape)'            { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber started'
Ui-Mark
Ui-Step 'Mode: Two points'               { Ui-ClickLogged 'Panel Timber input how=points' }
Ui-ExpectNew 'View hint Timber: Click where the member starts' "the view's hint for the start"
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Step 'the start pick'                 { Ui-ClickLogged 'Panel Timber input start' }
# Each stud's +X side: of its sides in sight, the one furthest right (the
# view from the front right: +X and +Y to the right).
$plusX = @{}
foreach ($s in @(Wall-Sides $studs)) {
  if (-not $plusX.ContainsKey($s[0]) -or $s[2] -gt $plusX[$s[0]][2]) { $plusX[$s[0]] = $s }
}
$studSides = @($plusX.Values | Sort-Object { $_[2] })
if ($studSides.Count -lt 3) { Ui-Fail "only $($studSides.Count) stud side(s) in sight" }
# The second stud's +X side, facing the third, and the third's: not one
# plane. Before mitcad#509 the end was moved onto the start's plane, beside
# the start: a member a few millimetres long, refused.
$first = $studSides[1]
$second = $studSides[2]
Ui-Mark
Ui-Step "the start on a stud's side"     { Ui-Click $first[2] $first[3] }
Ui-ExpectNew 'View hint Timber: Click where it ends; Shift+click places at once' 'the end next'
Ui-Mark
Ui-Step "the end on the next stud's side" { Ui-Click $second[2] $second[3] }
Ui-ExpectNew 'Inspect Timber: points, length ' 'two clicks give the member'
$line = Ui-LogLines | Select-Object -Skip $script:UiMark |
  Where-Object { $_.StartsWith('Inspect Timber: points, length ') } | Select-Object -Last 1
if ("$line" -notmatch '^Inspect Timber: points, length ([0-9.]+) mm') { Ui-Fail "no length in: $line" }
$length = [double]$Matches[1]
if (-not "$line".Contains('; free')) { Ui-Fail "the member between the studs' sides (two planes) is not free: $line" }
if ($length -lt 450 -or $length -gt 700) { Ui-Fail "the member between two studs 600 apart is $length mm: $line" }
if ("$line" -notmatch '; start on (?!none)' -or "$line" -notmatch ', end on (?!none)') { Ui-Fail "a point not on a stud: $line" }
# The end on the next stud's far side: cut back to its near face.
if ("$line" -notmatch ', end on [^;]* cut;') { Ui-Fail "its end not cut back to the stud's face: $line" }
Write-Host "ok   $length mm between the studs, free, its end on the next stud's face"
Place 'Timber 48x98 test'
Ui-ExpectNew 'by two points, ' 'a member by two points'
$placed = [double](Ui-LastMatch '^Placed Timber 48x98 test as .*, length by two points, ([0-9.]+) mm')
if ([Math]::Abs($placed - $length) -gt 0.1) { Ui-Fail "placed $placed mm long, previewed $length mm" }
Write-Host "ok   placed $placed mm long, as previewed"
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 15. Floor / Roof: a floor (mitcad#520)'
Ui-StartApp 'timber-floor'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-ExpectLog 'Ribbon button build.floor_roof at' 'the Floor / Roof button'
Ui-Mark
Ui-Step 'Floor / Roof (F5)'              { Ui-Key 'F5' }
Ui-ExpectNew 'Command Floor / Roof started' 'Floor / Roof started'
Ui-ExpectNew 'BUILD floor_roof kind floor' 'a floor the first time'
Ui-ExpectNew 'Panel Floor / Roof input frame_kind=roof at' 'the Kind''s Roof button'
$wallInputs = @(Ui-LogLines | Select-Object -Skip $script:UiMark |
  Where-Object { $_ -match '^Panel Floor / Roof input (frame_height|frame_corner|frame_top) at \d+,\d+$' })
if ($wallInputs.Count -gt 0) { Ui-Fail "Floor / Roof shows a wall's inputs: $($wallInputs -join ' / ')" }
Expect-Inspect 'Floor / Roof' @('frame floor Floor 1, floor 4000 x 3000 mm', '4 rim joist') `
  'a floor from the origin: rim joists all round, joists between them'
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Floor 1 (floor, lead ' 'the floor placed' 60
Ui-Mark
Ui-Step 'Kind: Roof'                     { Ui-ClickLogged 'Panel Floor / Roof input frame_kind=roof' }
Ui-ExpectNew 'View hint Floor / Roof: Click a member of each of the two walls the roof stands on' `
  'a roof waits for its two walls'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

# Largest-Bodies: the bodies of the largest volume made since the mark (a
# wall's plates).
function Largest-Bodies {
  $made = @(Ui-LogLines | Select-Object -Skip $script:UiMark | ForEach-Object {
      if ($_ -match '^New body .* \((F[0-9]+\.b[0-9]+)\): volume ([0-9.]+) mm3$') {
        [pscustomobject]@{ Body = $Matches[1]; Volume = [double]$Matches[2] } } })
  if ($made.Count -eq 0) { return @() }
  $most = ($made | Measure-Object -Property Volume -Maximum).Maximum
  return @($made | Where-Object { $_.Volume -eq $most } | ForEach-Object { $_.Body })
}
# Pickable BODIES: the first of these bodies a click picks a face of now (a
# "Pick face" place after the last "Pick places:"); $null for none.
function Pickable([string[]]$Bodies) {
  $lines = @(Ui-LogLines)
  $start = 0
  for ($i = $lines.Count - 1; $i -ge 0; $i--) { if ($lines[$i].Contains('Pick places:')) { $start = $i; break } }
  $picks = @($lines[$start..($lines.Count - 1)])
  foreach ($body in $Bodies) {
    if (@($picks | Where-Object { $_.StartsWith("Pick face $body/") }).Count -gt 0) { return $body }
  }
  return $null
}

Write-Host '--- 16. The Openings window: a window and a door, load-bearing or not (mitcad#519)'
# Openings-Place NAME: the place the Openings window logged for NAME, in
# the window ("Openings place offset.2 at x,y; in the window x,y").
function Openings-Place([string]$Name) {
  for ($i = 0; $i -lt 30; $i++) {
    $line = Ui-LogLines | Where-Object { $_.StartsWith("Openings place $Name at ") } | Select-Object -Last 1
    $match = [regex]::Match("$line", 'in the window (-?\d+),(-?\d+)$')
    if ($match.Success) { return @([int]$match.Groups[1].Value, [int]$match.Groups[2].Value) }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "no place of the Openings window logged for '$Name'"
}
function Openings-Window {
  $window = Ui-Windows '^Openings$'
  if ($window.Count -eq 0) { Ui-Fail 'no Openings window' }
  return $window[0]
}
# Openings-Click NAME: a click on that place, posted to the window itself.
function Openings-Click([string]$Name) {
  $place = Openings-Place $Name
  $window = Openings-Window
  $b = $script:UiButtons[1]
  [UiInput]::Move($window, $place[0], $place[1], 0)
  Start-Sleep -Milliseconds 80
  [UiInput]::Button($window, $b[0], $place[0], $place[1], $b[3])
  Start-Sleep -Milliseconds 60
  [UiInput]::Button($window, $b[1], $place[0], $place[1], 0)
}
# Openings-Type NAME TEXT: the field's text replaced and committed (Enter);
# the keys go to the window, then to the main window again.
function Openings-Type([string]$Name, [string]$Text) {
  Openings-Click $Name
  Ui-FocusDialog '^Openings$'
  Ui-Key 'ctrl+a'
  Ui-Type $Text
  Ui-Key 'Return'
  Ui-FocusMain
}
# Openings-Wheel NAME STEPS: a drop-down of the window moved by wheel steps
# (its list cannot open in session 0): -1 chooses the next entry.
function Openings-Wheel([string]$Name, [int]$Steps) {
  $place = Openings-Place $Name
  $window = Openings-Window
  [UiInput]::Move($window, $place[0], $place[1], 0)
  Start-Sleep -Milliseconds 80
  [UiInput]::Wheel($window, $place[0], $place[1], $Steps)
}
Ui-StartApp 'timber-openings'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-Mark
Ui-Step 'Wall (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Command Wall started' 'Wall started'
Ui-Mark
Ui-Step 'no fasteners'                   { Ui-ClickLogged 'Panel Wall input frame_fasteners' }
Expect-Inspect 'Wall' @('frame wall Wall 1, wall 3600 x 2400 mm', 'fasteners no') 'a wall from the origin, no fasteners'
Ui-Mark
Ui-Step 'Openings (its tile)'            { Ui-ClickLogged 'Panel Wall input frame_openings_window' }
Ui-ExpectNew 'Openings window opened for build.wall: 0 row(s), non-load-bearing' 'the Openings window, a partition by default'
Ui-ExpectNew 'Openings place add_window at ' 'its buttons'' places'
Ui-Mark
Ui-Step 'Add window'                     { Openings-Click 'add_window' }
Ui-ExpectNew 'Openings rows: 600 900x1200 900' 'a window written into Wall'
Expect-Inspect 'Wall' @('frame wall Wall 1, wall 3600 x 2400 mm, 13 member(s)', '1 flat header', '2 king stud',
  '; openings 1, non-load-bearing, opening 1 marked') 'a partition''s window: one flat header, no trimmers, marked'
Ui-Sync
Start-Sleep -Milliseconds 1500
$previews = @(Ui-LogLines | Select-Object -Skip $script:UiMark |
  Where-Object { $_.StartsWith('Inspect Wall: ') -and $_.Contains('; openings 1, non-load-bearing') })
if ($previews.Count -ne 1) { Ui-Fail "the window's change previewed $($previews.Count) times" }
Write-Host 'ok   one preview for the change'
Ui-Mark
Ui-Step 'Add door'                       { Openings-Click 'add_door' }
Ui-ExpectNew 'Openings rows: 600 900x1200 900; 2100 900x2100' 'a door after the window'
Expect-Inspect 'Wall' @('16 member(s)', '2 flat header', '4 king stud', '; openings 2, non-load-bearing') `
  'the door framed too'
Ui-Mark
Ui-Step 'row 2 at 4000'                  { Openings-Type 'offset.2' '4000' }
Ui-ExpectNew 'Openings row 2: outside the wall (3600 mm long)' 'the row outside the wall flagged'
Ui-Mark
Ui-Step 'row 2 at 2100 again'            { Openings-Type 'offset.2' '2100' }
Ui-ExpectNew 'Openings rows ok' 'the row fits again'
Ui-Mark
Ui-Step 'Load-bearing'                   { Openings-Wheel 'bearing' -1 }
Ui-ExpectNew 'Openings bearing: load-bearing' 'the wall made load-bearing'
Expect-Inspect 'Wall' @('22 member(s)', '4 header', '4 trimmer stud', '; openings 2, load-bearing') `
  'load-bearing: trimmer studs and headers on edge'
# The Openings window takes the keyboard when it opens (its Enter commits
# a field): closed (Close; the openings stay), it gives the main window
# the activation back (mitcad#532), and Enter places the wall.
Ui-Mark
Ui-Step 'close the window (Close)'       { Openings-Click 'close' }
# (Logged "Openings window hidden: the main window active again" when it
# still had the activation as it hid.)
Ui-FocusMain
Ui-Sync
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Wall 1 (wall, lead ' 'the wall placed with both' 60
$placed = [int](Ui-LastMatch '^Placed frame Wall 1 \(wall, lead [^)]*\): (\d+) member')
if ($placed -ne 22) { Ui-Fail "Wall 1 placed with $placed member(s), not 22" }
# Its plates (the largest bodies): one a click picks is chosen below.
$plates = @(Largest-Bodies)
# (The window closes with Wall when it is open: the Linux test checks it.)
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'Timber Frame (F12)'             { Ui-Key 'F12' }
Ui-ExpectNew 'Command Timber Frame started' 'Timber Frame started'
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Step 'its Member'                     { Ui-ClickLogged 'Panel Timber Frame input part' }
Ui-Sync
Ui-Mark
$plate = Pickable $plates
if (-not $plate) { Ui-Fail "no plate of the wall ($($plates -join ', ')) to pick" }
Ui-Step 'a member of the wall'           { Ui-ClickPick "$plate/" }
Expect-Inspect 'Timber Frame' @('frame Wall 1 laid out again: 22 member(s)', '; openings 2, load-bearing') `
  'the wall''s values, load-bearing as placed'
Ui-Mark
Ui-Step 'Openings (its tile)'            { Ui-ClickLogged 'Panel Timber Frame input frame_openings_window' }
Ui-ExpectNew 'Openings window opened for build.frame: 2 row(s), load-bearing' 'the window with the wall''s window and door'
# Closed with its Close button first: it has the keyboard (above).
Ui-Step 'close the window (Close)'       { Openings-Click 'close' }
Ui-FocusMain
Ui-Step 'Cancel (Escape)'                { Ui-Key 'Escape' }

Write-Host '--- 17. Each frame a component of its own; a gable roof''s ring nails at its junctions (mitcad#522, mitcad#521)'
First-Wall 'timber-roof' 'butt'
$studs1 = $script:Studs1
Ui-ExpectNew 'Frame component Wall 1 (' 'Wall 1 a component of its own (mitcad#522)'
$frame1 = Ui-LastMatch '^Frame component Wall 1 \((C\d+)\) in Component1$'
if (-not $frame1) { Ui-Fail 'Wall 1 is no component of its own in Component1' }
Ui-ExpectNew 'Joints by component: Component1 0' 'no joint of Wall 1 in Component1'
$plates1 = @(Largest-Bodies)
Start-Picks
Wall-From 'Wall 2' (Stud-Face $studs1 'first') '3000' "from Wall 1's side, flush with end A (butt)" 1
Ui-ExpectNew 'Frame component Wall 2 (' 'Wall 2 a component of its own'
$studs2 = @(Stud-Bodies)
Start-Picks
Wall-From 'Wall 3' (Stud-Face $studs1 'last') '3000' "from Wall 1's side, flush with end B (butt)" 1
Ui-ExpectNew 'Frame component Wall 3 (' 'Wall 3 a component of its own'
$studs3 = @(Stud-Bodies)
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
# Timber Frame (MODIFY) on a plate of Wall 1 lays it out again in place, in
# the same component.
Ui-Mark
Ui-Step 'Timber Frame (F12)'             { Ui-Key 'F12' }
Ui-ExpectNew 'Command Timber Frame started' 'Timber Frame started'
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Step 'its Member'                     { Ui-ClickLogged 'Panel Timber Frame input part' }
Ui-Sync
Ui-Mark
# Of Wall 1's plates the one a click picks now (the bottom plate may be out
# of sight).
$plate1 = Pickable $plates1
if (-not $plate1) { Ui-Fail "no plate of Wall 1 ($($plates1 -join ', ')) to pick" }
Ui-Step 'a plate of Wall 1'              { Ui-ClickPick "$plate1/" }
Expect-Inspect 'Timber Frame' @('frame Wall 1 laid out again: ') "Wall 1's values in Timber Frame"
Ui-Mark
Ui-Step 'Spacing 400'                    { Ui-TypeIn 'Panel Timber Frame input frame_spacing' '400 mm' }
Expect-Inspect 'Timber Frame' @('frame Wall 1 laid out again: ') 'the studs at 400'
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Timber frame Wall 1: ' 'Wall 1 laid out again' 60
Ui-ExpectNew "Frame component Wall 1 ($frame1) in Component1" 'in its own component, the same one'
Ui-ExpectNew 'Joints by component: Component1 0' 'still no joint in Component1'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
# A gable roof on Wall 2 and Wall 3 with ring nails (the owner's case of
# mitcad#521), two at each junction.
Ui-Mark
Ui-Step 'Floor / Roof (F5)'              { Ui-Key 'F5' }
Ui-ExpectNew 'Command Floor / Roof started' 'Floor / Roof started'
Ui-Step 'Kind: Roof'                     { Ui-ClickLogged 'Panel Floor / Roof input frame_kind=roof' }
Ui-ExpectNew 'View hint Floor / Roof: Click a member of each of the two walls the roof stands on' `
  'a roof waits for its two walls'
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Step 'its Walls'                      { Ui-ClickLogged 'Panel Floor / Roof input frame_walls' }
Ui-Sync
$at2 = Stud-Face $studs2 'middle'
$at3 = Stud-Face $studs3 'middle'
Ui-Mark
Ui-Step 'a stud of Wall 2'               { Ui-Click $at2[0] $at2[1] }
Ui-Step 'a stud of Wall 3'               { Ui-Click $at3[0] $at3[1] }
Expect-Inspect 'Floor / Roof' @('frame roof Roof 1, gable roof', 'rafter', 'ridge board', 'fasteners yes') `
  'a gable roof on Wall 2 and Wall 3, with fasteners'
Choose-Fastener 'Floor / Roof' 'fastener' 'test-timber/ring-nail-test'
Ui-Mark
Ui-Step 'two at each junction'           { Ui-TypeIn 'Panel Floor / Roof input fastener_count' '2' }
Expect-Inspect 'Floor / Roof' @('frame roof Roof 1, gable roof', 'fastener 2 x Ring nail test') `
  'two ring nails at each junction'
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Roof 1 (roof, lead ' 'the roof placed' 60
$fastened = [int](Ui-LastMatch '^Placed frame Roof 1 \(roof, lead [^)]*\): \d+ member\(s\), (\d+) junction')
if ($fastened -lt 1) { Ui-Fail 'no junction of the roof fastened' }
Ui-ExpectNew 'Frame component Roof 1 (' 'Roof 1 a component of its own'
Ui-ExpectNew 'Joints by component: Component1 0, ' 'Component1 holds no joint, the roof its nails'''
$joints = Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Joints by component: ') } |
  Select-Object -Last 1
if (-not "$joints".Contains("Roof 1 $(2 * $fastened)")) { Ui-Fail "the roof's joints not in Roof 1: $joints" }
Write-Host "ok   $joints"
# No nail beside the roof: each one at its junction, in its members (the
# test nails are heads only: through the member entered), its head on the
# entry face.
$nails = @(Heads-Since '4000')
if ($nails.Count -ne 2 * $fastened) { Ui-Fail "$($nails.Count) nail(s) at $fastened junction(s), not two each" }
Ui-ExpectNew 'Fastener shaft ' 'the nails measured'
$shafts = @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Fastener shaft ') })
$off = @($shafts | Where-Object { $_ -match ' through 0 of ' })
if ($shafts.Count -ne $nails.Count -or $off.Count -gt 0) {
  Ui-Fail "a nail beside its junction's members: $($shafts -join ' / ')"
}
$seats = @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_ -match '^Timber fastener .*: head top ' })
if (@($seats | Where-Object { $_ -notmatch 'head top 0 mm from entry face$' }).Count -gt 0) {
  Ui-Fail "a nail's head off the entry face: $($seats -join ' / ')"
}
Write-Host "ok   $($nails.Count) nails at their junctions, flush"
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
# The cut list by frame (mitcad#522).
Ui-Mark
Ui-Step 'Cut List (F9)'                  { Ui-Key 'F9' }
Ui-FocusDialog '^Cut List$'
Ui-ExpectNew 'Cut list row: ' "the cut list's rows"
foreach ($name in @('Wall 1', 'Wall 2', 'Wall 3', 'Roof 1')) {
  $rows = @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Cut list row: ') -and $_.EndsWith(", frame $name") })
  if ($rows.Count -lt 1) { Ui-Fail "no cut list row of $name" }
}
Write-Host 'ok   the cut list by frame: Wall 1, Wall 2, Wall 3, Roof 1'
Ui-Step 'close (Escape)'                 { Ui-Key 'Escape' }
Ui-FocusMain

Write-Host '--- 18. A Start on another body''s face (mitcad#524): a solid wall with a door on a slab'
# A slab 5000 x 4000 x 200 (F1), a solid wall 3000 x 200 x 2700 on it (F2)
# and a door 900 x 2100 cut through the wall only (F3).
$solidJson = Join-Path $work 'solid-wall.json'
$solidFile = Join-Path $work 'solid-wall.mitcad'
Set-Content -Encoding ascii $solidJson @'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-1000, -2000],
    "length": 5000, "width": 4000, "height": 200, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
    "length": 3000, "width": 200, "height": 2700, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [1000, -10],
    "length": 900, "width": 220, "height": 2100, "operation": "cut", "participants": ["F2.b0"]}}
]
'@
Run-Cli 'run solid-wall' @('run', $solidJson, '--save', $solidFile)
Ui-StartApp 'timber-solid-wall' @('--open', $solidFile)
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-Mark
Ui-Step 'Wall (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Command Wall started' 'Wall started'
# Solid-Face TEXT DESCRIPTION: a click on each face of the solid wall
# (F2.b0) a click on Start picks, Start picked again before each, until the
# preview's inspection has TEXT.
function Solid-Face([string]$Text, [string]$Description) {
  Start-Picks
  Ui-Sync
  $lines = @(Ui-LogLines)
  $start = 0
  for ($i = $lines.Count - 1; $i -ge 0; $i--) { if ($lines[$i].Contains('Pick places:')) { $start = $i; break } }
  $places = @($lines[$start..($lines.Count - 1)] | ForEach-Object {
      if ($_ -match '^Pick face F2\.b0/\S* at (\d+),(\d+)$') { , @([int]$Matches[1], [int]$Matches[2]) } })
  if ($places.Count -lt 1) { Ui-Fail 'no face of the solid wall to click' }
  $first = $true
  foreach ($at in $places) {
    if (-not $first) { Start-Picks; Ui-Sync }
    $first = $false
    Ui-Mark
    Ui-Step 'a face of the solid wall'   { Ui-Click $at[0] $at[1] }
    for ($k = 0; $k -lt 15; $k++) {
      $line = Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Inspect Wall: ') } |
        Select-Object -Last 1
      if ("$line".Contains($Text)) { Write-Host "ok   ${Description}: $line"; return }
      Start-Sleep -Milliseconds 200
    }
  }
  Ui-Fail "${Description}: no face of the solid wall gave '$Text'"
}
# A side face: square from it on its outward (visible) side, on the slab.
Solid-Face 'from a face of ' 'a wall from a face of the solid wall'
Expect-Inspect 'Wall' @(', base on ', '(z 200 mm)', '; corner butt') 'on the slab''s top, the bottom plate at z 200'
# Its end face: a corner with Butt, in line with Straight (the Corner's
# icon buttons).
Solid-Face 'from the end face of ' 'a corner on the solid wall''s end face'
Ui-Mark
Ui-Step 'Corner: Straight'               { Ui-ClickLogged 'Panel Wall input frame_corner=straight' }
Expect-Inspect 'Wall' @('in line beyond the end face of ', '(straight)', '; corner straight') 'Straight: in line beyond it'
Ui-Mark
Ui-Step 'Corner: Butt'                   { Ui-ClickLogged 'Panel Wall input frame_corner=butt' }
Expect-Inspect 'Wall' @('from the end face of ', '(butt corner)') 'Butt again: the corner'
Ui-Mark
Ui-Step 'Corner: Straight again'         { Ui-ClickLogged 'Panel Wall input frame_corner=straight' }
Expect-Inspect 'Wall' @('in line beyond the end face of ') 'Straight again'
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Wall 1 (wall, lead ' 'the wall placed in line with the solid wall' 60
Ui-ExpectNew 'Frame junction: Wall 1: in line beyond the end face of ' 'placed in line beyond its end face'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 19. The owner''s case of mitcad#536: across a long room, Length Auto, its plates spliced'
# A slab (F1) and a room's walls as one body (F2): a box 10400 x 8219 x
# 2700 with the room, 10000 x 7819 inside, cut out of it (F3). A start on
# a long wall's inner face, Length Auto: the run meets the opposite wall,
# a face of the same body, 7819 mm on; 7819 mm is longer than the timber's
# longest (6000 mm), so each plate is three pieces of at most 3600 mm (its
# longest stock length), the joints over the studs at 3600 and 7200.
$roomJson = Join-Path $work 'long-room.json'
$roomFile = Join-Path $work 'long-room.mitcad'
Set-Content -Encoding ascii $roomJson @'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-700, -700],
    "length": 11400, "width": 9219, "height": 200, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-200, -200],
    "length": 10400, "width": 8219, "height": 2700, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
    "length": 10000, "width": 7819, "height": 2800, "operation": "cut", "participants": ["F2.b0"]}}
]
'@
Run-Cli 'run long-room' @('run', $roomJson, '--save', $roomFile)
Ui-StartApp 'timber-long-room' @('--open', $roomFile)
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-Mark
Ui-Step 'Wall (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Command Wall started' 'Wall started'
# The faces of the room's body clicked in turn until one runs across the
# room (a long wall's inner face; the others run 10000 mm or meet nothing).
Solid-Face 'frame wall Wall 1, wall 7819 x 2400 mm' 'a wall from a long wall''s inner face, across the room'
Expect-Inspect 'Wall' @('frame wall Wall 1, wall 7819 x 2400 mm', '3 bottom plate', '3 top plate', '; length auto',
  '; Length Auto: the run met a face of ', ', the wall 7819 mm',
  '; spliced top plate 3600 7200, bottom plate 3600 7200') `
  'previewed across the room, the run''s end named, each plate three pieces over the studs at 3600 and 7200'
Expect-Inspect 'Wall' @(', to a face of ', ', 7819 mm', ', base on ') 'the junction: to the opposite wall''s face, on the slab'
# Refused (its studs 6904 mm, longer than the timber's longest): the panel
# still names the wall, its length and what the run met.
Ui-Mark
Ui-Step '7000 mm high'                   { Ui-TypeIn 'Panel Wall input frame_height' '7000' }
Ui-ExpectNew 'Frame preview refused: frame wall Wall 1, wall 7819 x 7000 mm, 0 member(s)' `
  'the refused preview names the wall and the length worked out'
$line = Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Frame preview refused: ') } |
  Select-Object -Last 1
if (-not "$line".Contains('; Length Auto: the run met a face of ')) { Ui-Fail "the refused preview without the run's end: $line" }
if (-not "$line".Contains("longer than the part's longest")) { Ui-Fail "the refused preview without its reason: $line" }
Write-Host 'ok   refused, with what the run met and why'
Ui-Mark
Ui-Step '2400 mm high again'             { Ui-TypeIn 'Panel Wall input frame_height' '2400' }
Expect-Inspect 'Wall' @('frame wall Wall 1, wall 7819 x 2400 mm', '; spliced top plate 3600 7200, bottom plate 3600 7200') `
  '2400 mm high again: previewed'
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed frame Wall 1 (wall, lead ' 'the wall placed across the room' 60
Ui-ExpectNew 'Frame spliced: top plate 3600 7200, bottom plate 3600 7200' 'placed with its plates spliced'
$line = Ui-LogLines | Select-Object -Skip $script:UiMark |
  Where-Object { $_.StartsWith('Frame junction: Wall 1: from a face of ') } | Select-Object -Last 1
if (-not ("$line".Contains(', to a face of ') -and "$line".Contains(', 7819 mm'))) {
  Ui-Fail "Wall 1 placed without the face its run met: $line"
}
Write-Host "ok   $line"
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Ui-Finish 'UI Windows timber test'
