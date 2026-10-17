# SPDX-License-Identifier: MIT
# check-all sources: app/build app/commands/BuildCommands.cpp app/framework tools/cli
# Input-driven Windows UI test of the BUILD tab (mitcad#375) in the build
# VM (tools/ui-windows-lib.ps1 says how the input is posted); the Linux
# test is tools/ui-build-test.sh, with the same steps. A library of small
# synthetic BUILD parts made here (mitcad-cli; no supplier's files), fetched
# from a local git repository; skipped where git is not installed:
#   1. The BUILD tab beside SOLID.
#   2. Profile: a frame of four (one new end mitred, the fourth's length
#      bound to the third; Tab steps the auto length).
#   3. Connector at a junction (shown on the hover), previewed in place.
#   4. Panel from an inner face.
#   5. Linear Part: a rail, a carriage on it; Ends: a mitred, bored end.
#   6. Save, reopen; Cut List: rows, Copy as CSV, Save as PDF.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-build-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-build'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'build.profile' = 'F3'; 'build.panel' = 'F5'; 'build.linear' = 'F7';
                            'build.connector' = 'F8'; 'build.cut_list' = 'F9'; 'build.ends' = 'F11' }

$git = Get-Command git -ErrorAction SilentlyContinue
if (-not $git) {
  Write-Host 'note: git is not installed; libraries cannot be fetched, nothing to test'
  Write-Host 'PASS: UI Windows BUILD test (skipped)'
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
$library = Join-Path $work 'test-profiles'
$file = Join-Path $work 'frame.mitcad'
$pdf = Join-Path $work 'cut-list.pdf'
# The log gives paths with forward slashes.
$fileLogged = $file -replace '\\', '/'
$pdfLogged = $pdf -replace '\\', '/'

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

Run-Cli 'library init' @('library', 'init', $library, '--id', 'test-profiles', '--name', 'Test profiles', '--license', 'MIT')
$rect = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
function Frame([string]$Name, [string]$Origin, [string]$Z, [string]$X) {
  return ('  {"cmd": "add_feature", "name": "mount:' + $Name + '", "def": {"type": "joint_origin", "geometry": "origin", ' +
          '"frame_override": {"origin": ' + $Origin + ', "z_axis": ' + $Z + ', "x_axis": ' + $X + '}}},')
}
function Bar([double]$Size) {
  $half = $Size / 2
  return @(
    '[',
    '  {"cmd": "add_parameter", "name": "L", "expression": "500 mm"},',
    '  {"cmd": "sketch.create", "plane": "xy"},',
    ('  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-' + $half + ', -' + $half + '], "width": ' + $Size +
     ', "height": ' + $Size + '},'),
    ('  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "' + $rect + '"}], ' +
     '"extent": {"type": "distance", "distance": "L"}, "operation": "new_body"}},'),
    (Frame 'end_a' '[0, 0, 0]' '[0, 0, -1]' '[1, 0, 0]'),
    ('  {"cmd": "add_feature", "name": "mount:end_b", "def": {"type": "joint_origin", ' +
     '"geometry": {"body": "F2.b0", "face": "F2:end(' + $rect + ')"}}},')) -join "`n"
}
function Block([string]$X, [string]$Y, [string]$Z) {
  return ('[' + "`n" + '  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"}, ' +
          '"corner": [0, 0], "length": ' + $X + ', "width": ' + $Y + ', "height": ' + $Z + ', "operation": "new_body"}},')
}
$end = "`n" + '  {"expect": {"bodies": 1}}' + "`n" + ']'
$profile20 = @((Bar 20), (Frame 'slot_x+_1' '[10, 0, 0]' '[1, 0, 0]' '[0, 0, 1]'),
  (Frame 'slot_x-_1' '[-10, 0, 0]' '[-1, 0, 0]' '[0, 0, 1]'), (Frame 'slot_y+_1' '[0, 10, 0]' '[0, 1, 0]' '[0, 0, 1]'),
  (Frame 'slot_y-_1' '[0, -10, 0]' '[0, -1, 0]' '[0, 0, 1]')) -join "`n"
Design 'profiles\p20-test.mitcad' ($profile20 + $end)
$bracket = @((Block '18' '18' '18'), (Frame 'mount_a' '[9, 9, 0]' '[0, 0, -1]' '[1, 0, 0]'),
  (Frame 'mount_b' '[0, 9, 9]' '[-1, 0, 0]' '[0, 0, 1]')) -join "`n"
Design 'connectors\bracket-20-test.mitcad' ($bracket + $end)
$panel = @('[',
  '  {"cmd": "add_parameter", "name": "W", "expression": "100 mm"},',
  '  {"cmd": "add_parameter", "name": "H", "expression": "100 mm"},',
  '  {"cmd": "add_parameter", "name": "t", "expression": "3 mm"},',
  # Centred on its XY origin: width along X, height along Y, thickness along
  # Z. A box's corner takes numbers only: the box from the origin (F1),
  # moved back by half its width and height (F2), W and H still sizing it.
  ((Block '"W"' '"H"' '"t"') -split "`n")[1],
  ('  {"cmd": "add_feature", "def": {"type": "move", "bodies": ["F1.b0"], ' +
   '"transform": {"type": "translate_xyz", "x": "-W / 2", "y": "-H / 2", "z": 0}}},'),
  (Frame 'face' '[0, 0, 0]' '[0, 0, -1]' '[1, 0, 0]')) -join "`n"
Design 'panels\panel-test.mitcad' ($panel + $end)
$rail15 = @((Bar 15), (Frame 'rail' '[0, 7.5, 0]' '[0, 1, 0]' '[0, 0, 1]'), (Frame 'base' '[0, -7.5, 0]' '[0, -1, 0]' '[0, 0, 1]')) -join "`n"
Design 'linear\rail-test.mitcad' ($rail15 + $end)
$carriage = @((Block '30' '20' '20'), (Frame 'rail' '[15, 10, 0]' '[0, 0, -1]' '[1, 0, 0]')) -join "`n"
Design 'linear\carriage-test.mitcad' ($carriage + $end)

Set-Content -Encoding ascii (Join-Path $library 'mitcad-library.json') @'
{
  "format": "mitcad-library", "version": 1, "id": "test-profiles", "name": "Test profiles",
  "description": "Synthetic BUILD parts of the UI test.", "library_version": "1.0.0", "license": "MIT",
  "authors": ["Mitcad UI test"], "units": "mm",
  "categories": [{"id": "profiles", "name": "Profiles"}, {"id": "connectors", "name": "Connectors"},
                 {"id": "panels", "name": "Panels"}, {"id": "linear", "name": "Linear parts"}],
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
    {"id": "bracket-20-test", "path": "connectors/bracket-20-test.mitcad", "name": "Bracket 20 test",
     "category": "connectors", "supplier": "Test supplier", "part_number": "B20-T",
     "build": {"role": "connector", "systems": [{"type": "I", "slot": 5, "grid": 20}],
       "frames": [{"name": "mount_a", "kind": "face"}, {"name": "mount_b", "kind": "face"}],
       "connector": {"junctions": ["L", "T"], "space": "inner_corner",
                     "mates": [{"frame": "mount_a", "to": "slot"}, {"frame": "mount_b", "to": "slot"}],
                     "fasteners": [], "machining": []}}},
    {"id": "panel-test", "path": "panels/panel-test.mitcad", "name": "Panel test", "category": "panels",
     "build": {"role": "panel", "frames": [{"name": "face", "kind": "face"}],
               "panel": {"mount": "slot", "thickness": "t", "width": "W", "height": "H", "clearance": "0.5 mm"}}},
    {"id": "rail-test", "path": "linear/rail-test.mitcad", "name": "Rail 15 test", "category": "linear",
     "supplier": "Test supplier", "part_number": "R15-T", "designation": "Rail 15 test L={L}",
     "build": {"role": "linear", "systems": [{"type": "I", "slot": 5, "grid": 20}],
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
'@
foreach ($arguments in @(@('add', '-A'), @('commit', '-q', '-m', 'Test parts'), @('tag', 'v1.0.0'))) {
  $output = & $git.Source -C $library -c user.name=T -c user.email=t@example.invalid @arguments 2>&1
  if ($LASTEXITCODE -ne 0) { Ui-Fail "git $($arguments -join ' '): $($output -join ' / ')" }
}
Run-Cli 'library fetch' @('library', 'fetch', $library)
# A design with only its root, the active component.
$empty = Join-Path $work 'empty.json'
Set-Content -Encoding ascii $empty '[]'
$start = Join-Path $work 'start.mitcad'
Run-Cli 'the empty design' @('run', $empty, '--save', $start)

# The occurrence, the name and the body of the part placed last: "Placed <part> as <name> (<occurrence>),
# length ...", and the body the placement logged ("New body <name> (<uid>): volume ...").
function Placed-Occurrence { return Ui-LastMatch '^Placed .* as .* \((O[0-9/]*)\), length' }
function Placed-Name { return Ui-LastMatch '^Placed .* as (.*) \(O[0-9/]*\), length' }
function Placed-Body { return Ui-LastMatch '^New body .* \((F[0-9]+\.b[0-9]+)\): volume' }
# The faces of a body a click can pick now (after the last "Pick places:"),
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
# The view fitted to the parts, and the places a click picks logged again
# (the panel's pick input activated again).
function Refresh-Picks([string]$Command) {
  Ui-Step 'fit the view (F6)'            { Ui-Key 'F6' }
  Ui-Step "$Command's pick again"        { Ui-ClickLogged "Panel $Command input at" }
}
function Place([string]$What) {
  Ui-Mark
  Ui-Step 'Place (Enter)'                { Ui-Key 'Return' }
  Ui-ExpectNew "Placed $What as " "$What placed" 30
}
$side = ':part(F2:side('

Ui-StartApp 'build' @('--open', $start)

Write-Host '--- 1. The BUILD tab'
Ui-ExpectLog 'Ribbon tab BUILD at' 'the BUILD tab beside SOLID'
Ui-Mark
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-ExpectNew 'Ribbon BUILD/PROFILE at' 'its PROFILE group'

Write-Host '--- 2. Profile: a frame of four'
Ui-Mark
Ui-Step 'Profile (F3)'                   { Ui-Key 'F3' }
Ui-ExpectNew 'Command Profile started' 'Profile started'
Ui-ExpectNew 'BUILD parts profile: 1 (0 in the design)' 'the picker lists the profile'
Ui-Step 'a typed length of 400'          { Ui-TypeIn 'Panel Profile input length' '400' }
Ui-ExpectNew 'BUILD length typed: detached from the auto length' 'typing a length detaches it'
Ui-Step 'on the XY plane'                { Ui-ClickLogged 'Datum xy' }
Ui-ExpectNew 'Inspect Profile: place ' 'the preview'
Place 'Profile 20 test'
Ui-ExpectNew 'Applied Profile (1)' 'the panel stays open'
$p1 = Placed-Occurrence
$b1 = Placed-Body
if (-not $p1 -or -not $b1) { Ui-Fail 'no occurrence or body of the first profile' }
Ui-Step 'Offset from an End'             { Ui-ClickLogged 'Panel Profile input use_offset' }
Ui-Step 'offset 0'                       { Ui-TypeIn 'Panel Profile input offset' '0' }
Ui-Step 'mitre the new part'             { Ui-WheelLogged 'Panel Profile input mitre' -1 }
Ui-ExpectNew 'Profile: Mitre = Cut the new part' 'the new part mitred'
Ui-Step 'length 300'                     { Ui-TypeIn 'Panel Profile input length' '300' }
Refresh-Picks 'Profile'
# The two on a side of the first one that a click can reach.
$sides1 = @(Faces-Of $b1 $side)
if ($sides1.Count -eq 0) { Ui-Fail "no side of the first profile ($b1) to pick" }
$side1 = $sides1[0]
Ui-Mark
Ui-Step "the first profile's side"       { Pick-Face $b1 $side1 }
Ui-ExpectNew 'Inspect Profile: place side' 'on the side'
Place 'Profile 20 test'
$p2 = Placed-Occurrence
$b2 = Placed-Body
Ui-Step 'no mitre'                       { Ui-WheelLogged 'Panel Profile input mitre' 1 }
Ui-Step 'from end B'                     { Ui-WheelLogged 'Panel Profile input from' -1 }
Refresh-Picks 'Profile'
Ui-Mark
Ui-Step "the first profile's side again" { Pick-Face $b1 $side1 }
Ui-ExpectNew 'Inspect Profile: place side' 'on the side'
Place 'Profile 20 test'
$p3 = Placed-Occurrence
$b3 = Placed-Body
$p3Name = Placed-Name
Write-Host "ok   the profiles are $p1 ($b1), $p2 ($b2), $p3 ($b3, $p3Name), on $side1"
Ui-Mark
Ui-Step 'the auto length again (Tab)'    { Ui-Key 'Tab' }
Ui-ExpectNew 'Profile: Tab steps Length' 'Tab steps the auto length'
Refresh-Picks 'Profile'
$found = $null
foreach ($face in @(Faces-Of $b2 $side)) {
  Ui-Mark
  Ui-Step "the second profile's $face"   { Pick-Face $b2 $face }
  $inspect = $null
  for ($i = 0; $i -lt 25; $i++) {
    $inspect = Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Inspect Profile: ') } |
      Select-Object -Last 1
    if ($inspect) { break }
    Start-Sleep -Milliseconds 200
  }
  # Bound to the third: "(bound, auto: to the face to <its face> of <name>)" or "... to <name>)".
  if ("$inspect".Contains("(bound, ") -and "$inspect".Contains(" $p3Name)")) { $found = $face; break }
}
if (-not $found) { Ui-Fail 'no side of the second profile a click reaches has its auto length to the third' }
Write-Host "ok   $found of the second profile runs to the third"
Ui-Mark
Ui-Step 'to the centre (Tab)'            { Ui-Key 'Tab' }
Ui-ExpectNew 'Profile: Length = Auto: to the centre' 'Tab: to the centre'
Ui-Step 'through (Tab)'                  { Ui-Key 'Tab' }
Ui-Step 'to the face (Tab)'              { Ui-Key 'Tab' }
Ui-ExpectNew 'Profile: Length = Auto: to the face' 'Tab: to the face again'
Ui-ExpectNew 'Cursor box Profile length: ' 'the auto length in the box at the cursor'
Place 'Profile 20 test'
Ui-ExpectNew 'length bound, auto: to the face' 'its length is bound'
Ui-Mark
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-ExpectNew 'Closed Profile after 4' 'Close keeps the four profiles'

Write-Host '--- 3. Connector'
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Mark
Ui-Step 'Connector (F8)'                 { Ui-Key 'F8' }
Ui-ExpectNew 'Inspect Connector: junction ' 'the junction and its connectors'
Ui-ExpectNew ', preview Bracket 20 test with ' 'the chosen connector previewed in place'
Ui-ExpectNew 'Panel Connector option pictures part: ' 'the connectors listed with their pictures'
$faces3 = @(Faces-Of $b3 $side)
if ($faces3.Count -eq 0) { Ui-Fail "no side of the third profile ($b3) to hover" }
Ui-Mark
$over = Ui-PickAt "$b3/$($faces3[0]) at"
# A move with nothing in wParam is taken by Qt for the "enter" of a window
# shown under the cursor and dropped while no window is under the pointer,
# which in session 0 (no cursor) is the case after the panels changed:
# MK_SHIFT in wParam keeps the move (Qt reads the modifiers from the key
# state, so it is a plain move over the view).
Ui-Step 'the mouse over the third profile' { [UiInput]::Move($script:UiWindow, $over[0], $over[1], 0x04) }
$shown = $false
for ($i = 0; $i -lt 25 -and -not $shown; $i++) {
  $shown = [bool](Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_ -match '^Preview marker Connector [^>]' })
  if (-not $shown) { Start-Sleep -Milliseconds 200 }
}
if (-not $shown) { Ui-Fail 'no junction of the third profile shown on its hover' }
Write-Host "ok   the hovered profile's junctions shown"
Ui-ExpectNew 'Preview marker Connector > ' 'the junctions marked'
Ui-Mark
Ui-Step 'Connect (Enter)'                { Ui-Key 'Return' }
Ui-ExpectNew 'Connected Bracket 20 test at junction ' 'the connector placed' 30
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 4. Panel'
Ui-Mark
Ui-Step 'Panel (F5)'                     { Ui-Key 'F5' }
Ui-ExpectNew 'BUILD parts panel: 1' 'the picker lists the panel'
Ui-Step "the frame's inner face"         { Pick-Face $b1 $side1 }
Ui-ExpectNew 'Inspect Panel: panel Panel test' "the panel's preview"
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed panel Panel test as ' 'the panel placed' 30
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 5. Linear Part: a rail and a carriage'
Ui-Mark
Ui-Step 'Linear Part (F7)'               { Ui-Key 'F7' }
Ui-ExpectNew 'BUILD parts linear: 2' 'the rail and the carriage'
Ui-Step 'search rail'                    { Ui-TypeIn 'Panel Linear Part input search' 'rail' }
Ui-Step 'a typed length of 200'          { Ui-TypeIn 'Panel Linear Part input length' '200' }
# Another side of the first profile than the frame's, else that one.
$outer = @(Faces-Of $b1 $side | Where-Object { $_ -ne $side1 })
$railFace = if ($outer.Count -gt 0) { $outer[0] } else { $side1 }
Ui-Step "the first profile's $railFace"  { Pick-Face $b1 $railFace }
Ui-ExpectNew 'Inspect Linear Part: place ' "the rail's preview"
Place 'Rail 15 test'
$bRail = Placed-Body
Ui-Step 'search carriage'                { Ui-TypeIn 'Panel Linear Part input search' 'carriage' }
Refresh-Picks 'Linear Part'
$railFaces = @(Faces-Of $bRail $side)
if ($railFaces.Count -eq 0) { Ui-Fail "no side of the rail ($bRail) to pick" }
$top = @($railFaces | Where-Object { $_.Contains('side(c3[') })
$railTop = if ($top.Count -gt 0) { $top[0] } else { $railFaces[0] }
Ui-Mark
Ui-Step "the rail's $railTop"            { Pick-Face $bRail $railTop }
Ui-ExpectNew 'Inspect Linear Part: ' "the carriage's preview"
Place 'Carriage 15 test'
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 5b. Ends: an end of the third profile mitred and bored'
Ui-Step 'fit the view (F6)'              { Ui-Key 'F6' }
Ui-Mark
Ui-Step 'Ends (F11)'                     { Ui-Key 'F11' }
Ui-ExpectNew 'Command Ends started' 'Ends started'
$ends3 = @(Faces-Of $b3 ':end_b') + @(Faces-Of $b3 ':end_a') + @(Faces-Of $b3 $side)
if ($ends3.Count -eq 0) { Ui-Fail "no face of the third profile ($b3) to pick" }
Ui-Step "the third profile's $($ends3[0])" { Pick-Face $b3 $ends3[0] }
Ui-ExpectNew 'Inspect Ends: ' "the part's ends"
Ui-Step 'a bore in the end'              { Ui-WheelLogged 'Panel Ends input machining' -1 }
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Ends of ' 'the end edited'
Ui-ExpectNew 'machining D6.8' 'with its bore'

Write-Host '--- 6. Save, reopen, Cut List'
Ui-Mark
Ui-Step 'save as (Ctrl+Shift+S)'         { Ui-Key 'ctrl+shift+s' }
Ui-FocusDialog '^Save As$'
Ui-Step 'the path'                       { Ui-Key 'ctrl+a'; Ui-Type $file; Ui-Key 'Return' }
Ui-FocusMain
Ui-ExpectNew "Saved $fileLogged" 'saved'
Ui-StartApp 'build-reopened' @('--open', $file)
Ui-ExpectLog "Opened $fileLogged" 'reopened'
Ui-Mark
Ui-Step 'Profile (F3)'                   { Ui-Key 'F3' }
Ui-ExpectNew 'BUILD parts profile: 1 (1 in the design)' "the profile's source is in the design"
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'Cut List (F9)'                  { Ui-Key 'F9' }
Ui-FocusDialog '^Cut List$'
Ui-ExpectNew 'Cut list row: ' "the cut list's rows"
Ui-ExpectNew 'x Profile 20 test' 'the profiles in it'
Ui-ExpectNew 'A 45 deg' 'the mitred end'
Ui-ExpectNew 'x Rail 15 test' 'the rail in it'
Ui-ExpectNew 'machining D6.8' 'the bored end'
$rows = Ui-LastMatch '^Cut list: (\d+) row\(s\)'
if ([int]"0$rows" -lt 3) { Ui-Fail "the cut list has $rows rows, not three or more" }
Ui-Mark
Ui-Step 'Copy as CSV (Alt+C)'            { Ui-Key 'alt+c' }
Ui-ExpectNew 'Build list CSV: Quantity,Part,Length (mm),End A,End B,Machining,Own features,Supplier,Part number,Mass each (kg)' "the CSV's header"
Ui-ExpectNew ',Test supplier,P20-T,' "the profiles' supplier and part number"

Ui-Step 'Save as PDF (Alt+P)'            { Ui-Key 'alt+p' }
Ui-FocusDialog '^Save Cut List as PDF$'
Ui-Step 'the PDF path'                   { Ui-Key 'ctrl+a'; Ui-Type $pdf; Ui-Key 'Return' }
Ui-FocusDialog '^Cut List$'
Ui-ExpectNew "Build list PDF written: $pdfLogged (" 'the PDF written'
Ui-ExpectNew 'Cut list of frame' "with the design's name"
$head = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($pdf)[0..4])
if ($head -ne '%PDF-') { Ui-Fail "$pdf is no PDF" }
Write-Host "ok   $pdf is a PDF"
Ui-Step 'close (Escape)'                 { Ui-Key 'Escape' }
Ui-FocusMain

Ui-Finish 'UI Windows BUILD test'
