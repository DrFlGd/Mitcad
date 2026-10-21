# SPDX-License-Identifier: MIT
# check-all sources: app/build app/commands/BuildCommands.cpp app/framework app/library app/OcctViewer.cpp tools/cli core/model/src/build
# Input-driven Windows UI test of the BUILD tab's Timber tool (mitcad#478,
# mitcad#485) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted). A library of small synthetic parts made here (mitcad-cli; a
# slot profile, two timber members, one of a less common size, and a wood
# screw), fetched from a local git repository; skipped where git is not
# installed:
#   1. Profile's picker lists no timber, Timber's no profile, its standard
#      size first; Timber's first start (mitcad#485): On a member, a typed
#      length of the part's default (3000 mm), along X, flat, the
#      dimensioned 48x98.
#   2. On a member: a post standing on the XY plane (Enter at the origin),
#      then a member on the post's side; the next start takes the length
#      typed last and the direction (mitcad#485).
#   3. Two points (mitcad#485): a member from a click on the second
#      member's side to a click on the post's side, nothing previewed
#      towards the mouse between the clicks; Tab turns it on edge.
#   4. Spacing: members at a centre distance along the second member's
#      side, placed as one set; Spacing Set edits its distance.
#   5. Connector at a timber junction: wood screws by default, sized from
#      the members, one click.
#   6. Profile again: the design's timber is not in its picker; Cut List:
#      the timber rows.
#   7. Every size a tile of its own (mitcad#484): the picker lists the 48
#      member's three rows and the 28 one, the families as groups, a row's
#      own picture; a search by 48x148 finds its tile, which places that
#      row with its section.
#   8. A frame of four members with shared mitres (mitcad#485): each on the
#      end of the one before, turned at a corner, lying flat as it does
#      (ends of the same width); the last one's auto length bound to the
#      first closes the corner with a shared mitre, as long as the first
#      one's outer side.
#   9. The same frame of slot profiles with Profile, the last one's length
#      through the first: a shared mitre, not into it.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-timber-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
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
       "length": {"parameter": "L", "min": "100 mm", "max": "6000 mm", "default": "3000 mm",
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
# Marker-At CMD LABEL: where CMD's last preview marker LABEL (a regex; also
# the taken one, "> LABEL") is; its point is on the face picked, so a click
# there clicks that face (tools/ui-windows-build-test.ps1).
function Marker-At([string]$Cmd, [string]$Label) {
  $line = Ui-LogLines | Where-Object { $_ -match "^Preview marker $Cmd (> )?$Label world " } | Select-Object -Last 1
  if ("$line" -notmatch ' at (\d+),(\d+)$') { Ui-Fail "no preview marker '$Label' of $Cmd logged" }
  return @([int]$Matches[1], [int]$Matches[2])
}
# Pick-Markers CMD AREA: the mouse off the parts (to a corner of the view,
# AREA: x y width height) and the pick's markers logged again, not the
# hover's (tools/ui-windows-build-test.ps1).
function Pick-Markers([string]$Cmd, [int[]]$Area) {
  $inset = 6
  $corners = @(@(($Area[0] + $inset), ($Area[1] + $inset)),
               @(($Area[0] + $inset), ($Area[1] + $Area[3] - $inset)),
               @(($Area[0] + $Area[2] - $inset), ($Area[1] + $Area[3] - $inset)))
  $inspect = -1
  foreach ($corner in $corners) {
    [UiInput]::Move($script:UiWindow, $corner[0], $corner[1], 0x04)
    for ($i = 0; $i -lt 20 -and $inspect -lt 0; $i++) {
      Ui-Sync
      $lines = @(Ui-LogLines)
      $lastLine = -1
      for ($j = $lines.Count - 1; $j -ge 0; $j--) { if ($lines[$j].StartsWith("Inspect ${Cmd}: ")) { $lastLine = $j; break } }
      if ($lastLine -lt 0 -or -not $lines[$lastLine].Contains('; on the hover')) { $inspect = [Math]::Max($lastLine, 0) }
      else { Start-Sleep -Milliseconds 100 }
    }
    if ($inspect -ge 0) { break }
  }
  if ($inspect -lt 0) { Ui-Fail "the hover's preview stayed with the mouse in the view's corners" }
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

# A new design (mitcad#394): Part1 active.
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
Ui-ExpectNew 'BUILD timber adds to Part1, member' 'On a member at first (mitcad#485)'
Ui-ExpectNew "BUILD timber starts: length 3000 mm (the part's default), typed; along x, flat" "a typed length, the part's default, along X, flat"
Expect-Inspect 'Timber' @('place grounded, length 3000.00 mm (typed)', '; along x, flat', '; thickness along Z',
                          '; part Timber 48x98 test') 'the preview: 3000 mm along X, lying flat'

Write-Host '--- 2. On a member, the next start'
Ui-Step '600 mm long'                    { Ui-TypeIn 'Panel Timber input length' '600' }
Ui-Mark
Ui-Step 'standing (Z)'                   { Ui-ClickLogged 'Panel Timber input axis=z' }
Expect-Inspect 'Timber' @('place grounded, length 600.00 mm (typed)', '; along z') 'a post at the origin, standing'
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
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'Timber (F4)'                    { Ui-Key 'F4' }
Ui-ExpectNew 'Command Timber started' 'Timber again'
Ui-ExpectNew 'BUILD timber starts with test-timber/timber-48-test: chosen last in the tool' 'the part chosen last'
Ui-ExpectNew 'BUILD timber adds to Part1, member' 'the mode used last'
Ui-ExpectNew 'BUILD timber starts: length 600 (typed last), typed; along z, flat' 'the length typed last and the direction'

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

# Frame-Of-Four CMD KEY PART L1 L2 [-Through]: a frame on the XY plane, L1
# along X and L2 across, of four members (PART) each on the end of the
# one before with a shared mitre (mitcad#485): the first along X at the
# origin, the second turned to -Y, the third to -X; the view then from the
# front left (the third's end faces -X), the fourth turned to +Y, its auto
# length bound to the first (Tab-stepped to "through" with THROUGH): it
# closes the corner with a shared mitre, L2 long.
function Frame-Of-Four([string]$Cmd, [string]$Key, [string]$Part, [string]$L1, [string]$L2, [switch]$Through) {
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
  Corner $Cmd $m3 '+Y' ''
  if ($Through) {
    Ui-Step 'to the centre (Tab)'        { Ui-Key 'Tab' }
    Ui-Mark
    Ui-Step 'through (Tab)'              { Ui-Key 'Tab' }
    Ui-ExpectNew '(bound, auto: through to ' 'bound through the first'
  }
  Expect-Inspect $Cmd @("length $L2.00 mm", "; closes the corner at end A of $m1Name, shared mitre") 'the last one closes the corner with a shared mitre, as long as the side'
  Place $Part
  Ui-ExpectNew 'length bound, auto: ' 'placed with its length bound to the first'
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

Ui-Finish 'UI Windows timber test'
