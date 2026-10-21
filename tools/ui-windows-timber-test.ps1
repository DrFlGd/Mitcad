# SPDX-License-Identifier: MIT
# check-all sources: app/build app/commands/BuildCommands.cpp app/framework app/library app/OcctViewer.cpp tools/cli
# Input-driven Windows UI test of the BUILD tab's Timber tool (mitcad#478)
# in the build VM (tools/ui-windows-lib.ps1 says how the input is posted).
# A library of small synthetic parts made here (mitcad-cli; a slot profile,
# two timber members, one of a less common size, and a wood screw), fetched
# from a local git repository; skipped where git is not installed:
#   1. Profile's picker lists no timber, Timber's no profile, its standard
#      size first.
#   2. On a member: a post standing on the XY plane (Enter at the origin),
#      then a member on the post's side.
#   3. From a click, two points: a member lying on the second member's
#      side, from a click there to across from the post's side, previewed
#      to where the mouse is before the second click; Tab turns it on edge.
#   4. Spacing: members at a centre distance along the second member's
#      side, placed as one set; Spacing Set edits its distance.
#   5. Connector at a timber junction: wood screws by default, sized from
#      the members, one click.
#   6. From a click, its length as Profile's: a click on the second
#      member's side, the mouse over the post's side: the auto length bound
#      to what the way meets, Tab steps it to the centre and through; then a
#      typed length of 1200 placed that long towards the mouse.
#   7. Profile again: the design's timber is not in its picker; Cut List:
#      the timber rows.
#   8. Every size a tile of its own (mitcad#484): the picker lists the 48
#      member's three rows and the 28 one, the families as groups, a row's
#      own picture; a search by 48x148 finds its tile, which places that
#      row with its section.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-timber-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-timber'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'build.profile' = 'F3'; 'build.timber' = 'F4'; 'build.connector' = 'F8';
                            'build.cut_list' = 'F9'; 'build.spacing' = 'F11' }

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
       "length": {"parameter": "L", "min": "100 mm", "max": "6000 mm", "default": "600 mm",
                  "stock": ["3000 mm", "3600 mm"]},
       "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
                  {"name": "face_xp", "kind": "face"}, {"name": "face_xn", "kind": "face"},
                  {"name": "face_yp", "kind": "face"}, {"name": "face_yn", "kind": "face"}],
       "rows": {"48x148": {"section": {"width": "148 mm", "height": "48 mm"},
                           "nominal": {"width": "148 mm", "height": "48 mm"}},
                "48x198": {"section": {"width": "198 mm", "height": "48 mm"},
                           "nominal": {"width": "198 mm", "height": "48 mm"}, "availability": "less common"}}}},
    {"id": "wood-screw-test", "path": "fasteners/wood-screw-test.mitcad", "name": "Wood screw test",
     "category": "fasteners", "designation": "Wood screw test 5x70",
     "build": {"role": "fastener", "wood": {"kind": "screw", "head": "countersunk", "diameter": "5 mm", "length": "70 mm"},
       "material": {"product": "steel", "density": 7850},
       "frames": [{"name": "axis", "kind": "shaft"}]}}
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
function Place([string]$What) {
  Ui-Mark
  Ui-Step 'Place (Enter)'                { Ui-Key 'Return' }
  Ui-ExpectNew "Placed $What as " "$What placed" 30
}
$side = ':part(F2:side('

# A new design (mitcad#394): Part1 active.
Ui-StartApp 'timber-new'
Ui-Mark
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-ExpectNew 'Ribbon BUILD/PROFILE at' 'its PROFILE group'
Ui-ExpectLog 'Ribbon button build.timber at' 'the Timber button on the BUILD tab'

Write-Host '--- 1. The pickers'
Ui-Mark
Ui-Step 'Profile (F3)'                   { Ui-Key 'F3' }
Ui-ExpectNew 'BUILD parts profile: 1 (0 in the design)' "Profile's picker: the slot profile only, no timber"
Ui-ExpectNew 'Panel Profile tile part: Profile 20 test' 'Profile starts with the slot profile'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'the Timber button'              { Ui-ClickLogged 'Ribbon button build.timber' }
Ui-ExpectNew 'Command Timber started' 'Timber started'
Ui-ExpectNew 'BUILD parts timber: 4 (0 in the design)' "Timber's picker: the members' four sizes (mitcad#484), no profile"
Ui-ExpectNew 'BUILD timber starts with test-timber/timber-48-test' 'the standard size first, the less common one after it'
Ui-ExpectNew 'BUILD timber adds to Part1, points' 'two points at first'

Write-Host '--- 2. On a member'
Ui-Mark
Ui-Step 'On a member'                    { Ui-ClickLogged 'Panel Timber input how=member' }
Ui-ExpectNew 'Inspect Timber: place grounded, length 600.00 mm (typed)' 'a post at the origin, standing'
Place 'Timber 48x98 test'
$post = Placed-Body
if (-not $post) { Ui-Fail 'no body of the post' }
Refresh-Picks 'at'
$postSides = @(Faces-Of $post $side)
if ($postSides.Count -eq 0) { Ui-Fail "no side of the post ($post) to pick" }
Ui-Mark
Ui-Step "the post's side"                { Pick-Face $post $postSides[0] }
Ui-ExpectNew 'Inspect Timber: place side' "on the post's side, a free offset"
Place 'Timber 48x98 test'
$beam = Placed-Body
if (-not $beam) { Ui-Fail 'no body of the member on the post' }

Write-Host '--- 3. From a click, two points'
Ui-Mark
Ui-Step 'From a click'                   { Ui-ClickLogged 'Panel Timber input how=points' }
Ui-ExpectNew 'View hint Timber: Click where the member starts' "the view's hint for the start"
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Step 'the start pick'                 { Ui-ClickLogged 'Panel Timber input start' }
Ui-Mark
$beamSides = @(Faces-Of $beam $side)
if ($beamSides.Count -eq 0) { Ui-Fail "no side of the member ($beam) to start on" }
Ui-Step "the start on the member's side" { Pick-Face $beam $beamSides[0] }
Ui-ExpectNew 'View hint Timber: Click where it ends' 'the end next'
$postSides = @(Faces-Of $post $side)
if ($postSides.Count -eq 0) { Ui-Fail "no side of the post ($post) to end at" }
$at = Pick-At $post $postSides[0]
Ui-Mark
# MK_SHIFT keeps the move in session 0 (tools/ui-windows-build-test.ps1).
Ui-Step "the mouse over the post's side" { [UiInput]::Move($script:UiWindow, $at[0], $at[1], 0x04) }
Ui-ExpectNew 'Inspect Timber: points, length ' 'previewed to where the mouse is' 10
Ui-ExpectNew '; on the hover' 'the hover'
Ui-Mark
Ui-Step 'the end there'                  { Ui-Click $at[0] $at[1] }
Ui-ExpectNew 'Inspect Timber: points, length ' 'ended there'
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
Ui-Mark
Ui-Step "the member's side"              { Pick-Face $beam $beamSides[0] }
Ui-ExpectNew 'Inspect Timber: place side' 'on its side'
Ui-ExpectNew '; set of ' 'the set counted'
Ui-ExpectNew 'centre distance 400 mm' 'at 400 mm'
Place 'Timber 48x98 test'
Ui-ExpectNew '; a set of ' 'placed as one set'
$set = Ui-LastMatch '; a set of (\d+)'
if ([int]"0$set" -lt 2) { Ui-Fail "the set has $set members, not two or more" }
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
Ui-ExpectNew 'preview Wood screw test with 2 fastener(s)' 'two wood screws by default'
Ui-ExpectNew ', size ' 'sized from the members'
Ui-Mark
Ui-Step 'Connect (Enter)'                { Ui-Key 'Return' }
Ui-ExpectNew 'Connected Wood screw test at junction ' 'one click finishes the joint' 30
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 6. From a click: an auto length, a typed length'
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber again'
Ui-Step 'From a click'                   { Ui-ClickLogged 'Panel Timber input how=points' }
# The start on a side of the member on the post, the mouse over the post's
# side: the member runs from the click towards the mouse. The note of the
# length the preview logged goes to $script:Towards ("bound, auto: to the
# face to ...", "typed"). Without a face: the first side of any member a
# click picks now, away from where the mouse then goes (the members placed
# here may cover the member's sides).
function Start-Towards-Post([string]$Face) {
  Refresh-Picks 'start'
  Ui-Mark
  if ($Face) {
    $postSides = @(Faces-Of $post $side)
    if ($postSides.Count -eq 0) { Ui-Fail "no side of the post ($post) to point at" }
    $at = Pick-At $post $postSides[0]
    Ui-Step "the start on the member's side" { Pick-Face $beam $Face }
  } else {
    # The sides a click picks now: the mouse over the first, the start on
    # one of another member away from it.
    $lines = @(Ui-LogLines)
    $from = 0
    for ($i = $lines.Count - 1; $i -ge 0; $i--) { if ($lines[$i].Contains('Pick places:')) { $from = $i; break } }
    $sides = @()
    foreach ($line in $lines[$from..($lines.Count - 1)]) {
      if ($line -match '^\s*Pick face (\S+)/\S*:part\(F2:side\(\S* at (\d+),(\d+)$') {
        $sides += , @($Matches[1], [int]$Matches[2], [int]$Matches[3])
      }
    }
    if ($sides.Count -lt 2) { Ui-Fail 'no two members'' sides to start on and point at' }
    $at = @($sides[0][1], $sides[0][2])
    $start = $sides | Where-Object {
      $_[0] -ne $sides[0][0] -and ([math]::Abs($_[1] - $at[0]) + [math]::Abs($_[2] - $at[1])) -gt 40 } |
      Select-Object -First 1
    if (-not $start) { Ui-Fail 'no side of another member to start on' }
    Ui-Step "the start on a member's side" { Ui-Click $start[1] $start[2] }
  }
  Ui-ExpectLog 'View hint Timber: Click where it ends, or type its length' 'the end or the length next'
  Ui-Mark
  Ui-Step 'the mouse over the side pointed at' { [UiInput]::Move($script:UiWindow, $at[0], $at[1], 0x04) }
  Ui-ExpectNew '; on the hover' 'previewed towards the mouse' 10
  $script:Towards = Ui-LastMatch '^Inspect Timber: points, .*; length ([^;]*); on the hover'
}
# A side of the member from which the way to the post meets something (its
# top or bottom: a member lying on a side face is beside the post).
$startFace = $null
foreach ($face in @(Faces-Of $beam $side)) {
  Start-Towards-Post $face | Out-Null
  $note = $script:Towards
  Write-Host "     from $face`: length $note"
  if ("$note".StartsWith('bound, auto: to the face')) { $startFace = $face; break }
}
if (-not $startFace) { Ui-Fail 'no side of the member gave an auto length towards the post' }
Write-Host 'ok   the auto length binds to what the way from the click meets'
Ui-Mark
Ui-Step 'to the centre (Tab)'            { Ui-Key 'Tab' }
Ui-ExpectNew 'Timber: Tab steps Length' 'Tab steps the auto length'
Ui-ExpectNew '; length bound, auto: to the centre to ' 'Tab: to the centre'
Ui-Mark
Ui-Step 'through (Tab)'                  { Ui-Key 'Tab' }
Ui-ExpectNew '; length bound, auto: through to ' 'Tab: through'
Ui-ExpectLog 'Cursor box Timber length: ' 'the auto length in the box at the cursor'
Place 'Timber 48x98 test'
Ui-ExpectNew 'length bound, auto: through' 'placed with its length bound'
# Another start (the member placed may cover the first one's place).
Start-Towards-Post '' | Out-Null
Ui-Mark
Ui-Step 'a typed length of 1200'         { Ui-TypeIn 'Panel Timber input length' '1200' }
Ui-ExpectNew 'BUILD length typed: detached from the auto length' 'typing detaches the auto length'
Ui-ExpectNew 'Inspect Timber: points, length 1200.00 mm, ' '1200 mm towards the mouse'
Ui-ExpectNew '; length typed; on the hover' 'typed, its way the mouse''s'
Place 'Timber 48x98 test'
Ui-ExpectNew 'length typed' 'placed 1200 mm long'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 7. Profile again, the cut list'
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

Write-Host '--- 8. Every size a tile of its own, the families as groups'
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber again'
Ui-Mark
Ui-Step 'the part picker (the Part tile)' { Ui-ClickLogged 'Panel Timber input part' }
Ui-ExpectNew 'Part picker opened: 4 parts for build.timber' 'the picker lists every size'
Ui-ExpectNew 'Part picker group All parts: 4 parts, first test-timber/timber-48-test (48x98)' 'all four sizes, the standard ones first'
foreach ($tile in @('test-timber/timber-48-test (48x98)', 'test-timber/timber-48-test (48x148)',
                    'test-timber/timber-48-test (48x198)', 'test-timber/timber-28-test')) {
  Ui-ExpectNew "Part picker tile $tile at " "a tile of $tile"
}
Ui-ExpectNew 'Part picker group row Dimensioned timber at ' 'the dimensioned timber a group of its own'
Ui-ExpectNew 'Part picker group row Planed timber at ' 'the planed timber a group of its own'
Ui-ExpectLog 'BUILD preview image of test-timber/timber-48-test: 16x16' "the part's picture in the panel's tile"
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
Ui-ExpectNew 'BUILD preview image of test-timber/timber-48-test (48x148): 24x24' "the row's own picture in the tile"
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

Ui-Finish 'UI Windows timber test'
