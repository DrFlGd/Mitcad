# SPDX-License-Identifier: MIT
# check-all sources: app/build app/commands/BuildCommands.cpp app/framework app/library app/OcctViewer.cpp tools/cli
# Input-driven Windows UI test of the BUILD tab (mitcad#375) in the build
# VM (tools/ui-windows-lib.ps1 says how the input is posted); the Linux
# test is tools/ui-build-test.sh, with the same steps. A library of small
# synthetic BUILD parts made here (mitcad-cli; no supplier's files), fetched
# from a local git repository; skipped where git is not installed:
#   1. The BUILD tab beside SOLID.
#   2. A new design, Part1 active (mitcad#394): Profile adds to the root,
#      no policy message; the first profile by a pick on the XY plane,
#      then (undone) by Enter at the origin; then only profiles' faces,
#      the planes hidden; a side click flush with an end fills Offset and
#      From (no check box); an end click in the middle straight on (Mitre),
#      the hover near an edge turned, the click there a corner (mitre
#      Auto: shared at offset 0, none at 10; mitcad#417); Close shows the
#      planes again.
#   3. Profile on a design with only its root: a frame of four (two flush
#      with the first's ends by clicks at the flush snaps, From End A and
#      End B; a typed offset stays from end B, From changed keeps the part
#      in place; the fourth's length bound to the third; Tab steps the auto
#      length).
#   4. Connector at a junction (shown on the hover), previewed in place.
#   5. Panel from an inner face.
#   6. Linear Part: a rail, a carriage on it; Ends: a mitred, bored end.
#   7. Save, reopen; Cut List: rows, Copy as CSV, Save as PDF.
#   8. The part picker (mitcad#418), with a 30 profile of another system in
#      the library's next version: opened from Profile's Part, a group, a
#      click choosing the 30 profile that the preview then uses, a star
#      (first in its group), pinned it follows Connector (a click chooses
#      the bracket); Profile again opens it with the panel's System,
#      Favourites, In this design after the placement, its system follows
#      the panel's. Its clicks are posted to the picker's own window.
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
# Expect-Row INPUT shown|hidden description: Profile's panel row INPUT as
# the panel laid it out last ("Panel Profile input <id> at x,y", with
# " (hidden)" when hidden).
function Expect-Row([string]$Row, [string]$State, [string]$Description) {
  $line = $null
  for ($i = 0; $i -lt 30; $i++) {
    $line = Ui-LogLines | Where-Object { $_.Contains("Panel Profile input $Row at ") } | Select-Object -Last 1
    $hidden = "$line".EndsWith('(hidden)')
    if ($line -and (($State -eq 'hidden') -eq $hidden)) { Write-Host "ok   $Description"; return }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "${Description}: Panel Profile input $Row is not $State ($line)"
}
# Marker-At LABEL: where Profile's last preview marker LABEL (a regex; also
# the taken one, "> LABEL") is: "Preview marker Profile flush B world
# x,y,z at x,y". Its point is on the face picked, so a click there clicks
# that face (mitcad#417).
function Marker-At([string]$Label) {
  $line = Ui-LogLines | Where-Object { $_ -match "^Preview marker Profile (> )?$Label world " } | Select-Object -Last 1
  if ("$line" -notmatch ' at (\d+),(\d+)$') { Ui-Fail "no preview marker '$Label' of Profile logged" }
  return @([int]$Matches[1], [int]$Matches[2])
}
# Marker-InMiddle LABEL: where Profile's marker LABEL is once it lies in
# the middle half of the view, the view zoomed out step by step at its
# centre until then: fitted to the parts, a profile's end can come under
# the orientation cube in the view's corner (session 0's window is small),
# where a click turns the view instead of picking.
function Marker-InMiddle([string]$Label) {
  $area = @((Ui-LastMatch 'View area (\d+ \d+ \d+ \d+)') -split ' ' | ForEach-Object { [int]$_ })
  if ($area.Count -lt 4) { Ui-Fail 'the app did not log its view area' }
  $cx = [int]($area[0] + $area[2] / 2)
  $cy = [int]($area[1] + $area[3] / 2)
  $sign = -1
  $last = $null
  for ($i = 0; $i -lt 16; $i++) {
    $at = Marker-At $Label
    $off = [Math]::Max([Math]::Abs($at[0] - $cx) / $area[2], [Math]::Abs($at[1] - $cy) / $area[3])
    if ($off -lt 0.25) { return $at }
    # A step that moved it away zoomed in: the other way then.
    if ($null -ne $last -and $off -gt $last) { $sign = -$sign }
    $last = $off
    [UiInput]::Move($script:UiWindow, $cx, $cy, 0x04)
    Start-Sleep -Milliseconds 80
    [UiInput]::Wheel($script:UiWindow, $cx, $cy, $sign)
    Ui-Sync
    Start-Sleep -Milliseconds 500
  }
  Ui-Fail "Profile's marker '$Label' did not come into the middle of the view"
}
# Expect-Inspect TEXTS description: an "Inspect Profile:" line since the
# mark has every text.
function Expect-Inspect([string[]]$Texts, [string]$Description) {
  for ($i = 0; $i -lt 30; $i++) {
    $found = Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Inspect Profile: ') } |
      Where-Object { $line = $_; @($Texts | Where-Object { -not $line.Contains($_) }).Count -eq 0 } |
      Select-Object -First 1
    if ($found) { Write-Host "ok   $Description"; return }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "${Description}: no 'Inspect Profile:' line since the mark with $($Texts -join ' / ')"
}
# Wheel-Choice INPUT STEPS: a panel drop-down of Profile moved by STEPS
# entries (positive: down the list), one wheel step at a time.
function Wheel-Choice([string]$Choice, [int]$Steps) {
  for ($i = 0; $i -lt [Math]::Abs($Steps); $i++) {
    Ui-WheelLogged "Panel Profile input $Choice" ([Math]::Sign(-$Steps))
    Start-Sleep -Milliseconds 150
  }
}
# No-PolicyMessage: no design-policy refusal reached the log.
function No-PolicyMessage {
  $refused = Ui-LogLines | Where-Object { $_.Contains('does not allow') } | Select-Object -First 1
  if ($refused) { Ui-Fail "a policy refusal: $refused" }
  Write-Host 'ok   no policy message'
}

# A new design (mitcad#394): Part1 active, the root with the Assembly policy.
Ui-StartApp 'build-new'

Write-Host '--- 1. The BUILD tab'
Ui-ExpectLog 'Ribbon tab BUILD at' 'the BUILD tab beside SOLID'
Ui-Mark
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-ExpectNew 'Ribbon BUILD/PROFILE at' 'its PROFILE group'

Write-Host '--- 2. The first profiles of a new design, Part1 active'
Ui-Mark
Ui-Step 'Profile (F3)'                   { Ui-Key 'F3' }
Ui-ExpectNew 'Command Profile started' 'Profile started'
Ui-ExpectNew 'BUILD profile adds to Part1 (a ground placement)' 'BUILD adds to the active Part1 (Hybrid, mitcad#421)'
Ui-ExpectNew 'View hint Profile: Click a plane or a face, or press Enter' "the view's hint for the first part"
Ui-ExpectNew 'Inspect Profile: place grounded, length 1000.00 mm (typed), square ends; adds to Part1' 'at the origin with its stock length'
Expect-Row 'axis=z' 'shown' "the Direction's X, Y and Z buttons"
Expect-Row 'mitre' 'hidden' 'no Mitre before an end pick'
Expect-Row 'offset' 'hidden' 'no Offset before a side pick'
Expect-Row 'place' 'hidden' 'Placement under More'
Ui-Mark
Ui-Step "on the root's XY plane"         { Ui-ClickLogged 'Datum xy' }
Ui-ExpectNew 'Inspect Profile: place grounded, length 1000.00 mm (typed), square ends; adds to Part1' 'on the root''s XY plane, picked from Part1: grounded at the click'
Place 'Profile 20 test'
No-PolicyMessage
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-Step 'undo it (Ctrl+Z)'               { Ui-Key 'ctrl+z' }
Ui-Mark
Ui-Step 'Profile again (F3)'             { Ui-Key 'F3' }
Ui-ExpectNew 'BUILD profile adds to Part1 (a ground placement)' 'a new design again'
Ui-Step '60 mm long'                     { Ui-TypeIn 'Panel Profile input length' '60' }
Ui-Mark
Ui-Step 'along X'                        { Ui-ClickLogged 'Panel Profile input axis=x' }
Ui-ExpectNew '; along x' 'the preview along X'
Ui-Mark
Ui-Step 'standing (Z)'                   { Ui-ClickLogged 'Panel Profile input axis=z' }
Ui-ExpectNew '; along z' 'the preview standing'
Place 'Profile 20 test'
No-PolicyMessage
$b0 = Placed-Body
if (-not $b0) { Ui-Fail 'no body of the first profile' }
Ui-ExpectNew 'Profile Place On takes now: faces' "only a profile's faces once one is there"
Ui-ExpectNew 'Construction geometry hidden for the command' 'the planes hidden for side and end placements'
Refresh-Picks 'Profile'
$sides0 = @(Faces-Of $b0 $side)
if ($sides0.Count -eq 0) { Ui-Fail "no side of the first profile ($b0) to pick" }
Ui-Mark
Ui-Step 'its side, 60 mm long'           { Pick-Face $b0 $sides0[0] }
Ui-ExpectNew 'Inspect Profile: place side' 'on the side'
Ui-ExpectNew '; snap end_' 'flush with the end near the click'
Ui-ExpectNew 'BUILD side click: From End ' 'the click fills From with the end it is flush with'
Expect-Row 'offset' 'shown' 'Offset for a side pick (no check box)'
Expect-Row 'from' 'shown' 'From for a side pick'
Expect-Row 'mitre' 'hidden' 'no Mitre for a side pick'
Place 'Profile 20 test'
$b1 = Placed-Body
Refresh-Picks 'Profile'
# An end of the first profile, or of the second where the first's end is
# out of sight in session 0's smaller window: the step is about an end pick.
$endBody = $b0
$tops0 = @(Faces-Of $b0 ':end_b')
if ($tops0.Count -eq 0 -and $b1) { $endBody = $b1; $tops0 = @(Faces-Of $b1 ':end_b') + @(Faces-Of $b1 ':end_a') }
if ($tops0.Count -eq 0) { Ui-Fail "no end face of the first two profiles ($b0, $b1) to pick" }
# The end face's pick place is in its inner half: straight on (mitcad#417).
Ui-Mark
Ui-Step 'an end face, in the middle'     { Pick-Face $endBody $tops0[0] }
Ui-ExpectNew 'Inspect Profile: place end' 'on the end'
Ui-ExpectNew 'BUILD end click: straight towards ' 'a click in the middle of the end continues straight'
Expect-Inspect @('; end straight towards ', '; least snap ') 'the preview straight on, with the snap distance'
Expect-Row 'direction' 'shown' 'Direction for an end pick'
Expect-Row 'mitre' 'shown' 'Mitre for an end pick'
Expect-Row 'offset' 'hidden' 'no side Offset for an end pick'
Expect-Row 'corner_offset' 'hidden' 'no corner Offset straight on'
Ui-Mark
Ui-Step 'mitre the new part'             { Wheel-Choice 'mitre' 3 }
Ui-ExpectNew 'ends A 45 deg' "the new part's end mitred"
Ui-Mark
Ui-Step 'the mitre Auto again'           { Wheel-Choice 'mitre' -3 }
Expect-Inspect @('; end straight towards ', 'mitre none') 'Auto: no mitre straight on'
# Near an edge of the end face (its zone's marker): the hover shows the
# turned part before the click, the click turns it: a corner.
$turn = Marker-At 'turn .*'
Ui-Mark
# MK_SHIFT keeps the move in session 0 (see the Connector's hover below).
Ui-Step 'the mouse near an edge of the end' { [UiInput]::Move($script:UiWindow, $turn[0], $turn[1], 0x04) }
Ui-ExpectNew 'BUILD hover on ' 'the hover on the end face'
Expect-Inspect @('; end edge towards ', '; on the hover') 'the hover previews the turned part before the click'
Ui-Mark
Ui-Step 'a click there'                  { Ui-Click $turn[0] $turn[1] }
Ui-ExpectNew 'BUILD end click: edge towards ' 'a click near an edge turns the part'
Expect-Inspect @('; end edge towards ', 'mitre shared, offset 0.0 mm') 'a corner, its mitre Auto: shared at offset 0'
Expect-Row 'corner_offset' 'shown' "the corner's Offset"
Ui-Mark
Ui-Step "the corner's offset 10"         { Ui-TypeIn 'Panel Profile input corner_offset' '10' }
Expect-Inspect @('; end edge towards ', 'mitre none, offset 10.0 mm') 'at another offset the mitre is none'
Ui-Mark
Ui-Step "the corner's offset 0"          { Ui-TypeIn 'Panel Profile input corner_offset' '0' }
Expect-Inspect @('; end edge towards ', 'mitre shared, offset 0.0 mm', 'ends A 45 deg') 'shared again at offset 0'
Place 'Profile 20 test'
No-PolicyMessage
Ui-Mark
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-ExpectNew 'Closed Profile after 3' 'Close keeps them'
Ui-ExpectNew 'Construction geometry shown again' 'the planes shown again when the command ends'

# The rest on a design with only its root, the active component.
Ui-StartApp 'build' @('--open', $start)
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }

Write-Host '--- 3. Profile: a frame of four'
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
Refresh-Picks 'Profile'
# The two on a side of the first one that a click can reach, flush with
# its ends by clicks at the flush snaps (their markers): a click fills
# Offset and From (mitcad#417).
$sides1 = @(Faces-Of $b1 $side)
if ($sides1.Count -eq 0) { Ui-Fail "no side of the first profile ($b1) to pick" }
$side1 = $sides1[0]
Ui-Mark
Ui-Step "the first profile's side"       { Pick-Face $b1 $side1 }
Ui-ExpectNew 'Inspect Profile: place side' 'on the side'
Ui-ExpectNew 'BUILD side click: From ' 'the click fills From and Offset'
Expect-Row 'offset' 'shown' 'Offset for a side pick (no check box)'
Expect-Row 'from' 'shown' 'From for a side pick'
Ui-Step 'length 300'                     { Ui-TypeIn 'Panel Profile input length' '300' }
$flushA = Marker-InMiddle 'flush A'
Ui-Mark
Ui-Step 'a click flush with end A'       { Ui-Click $flushA[0] $flushA[1] }
Ui-ExpectNew 'BUILD side click: From End A, Offset 0 mm (end_a)' 'flush with end A: From End A, Offset 0'
Expect-Inspect @('; snap end_a at 0.0 mm from a') 'the preview flush with end A'
Place 'Profile 20 test'
$p2 = Placed-Occurrence
$b2 = Placed-Body
Refresh-Picks 'Profile'
# Flush with end B by a click, then typed offsets stay from end B; From
# changed keeps the part where it is (L 400, width 20).
Ui-Mark
Ui-Step "the first profile's side again" { Pick-Face $b1 $side1 }
Ui-ExpectNew 'Inspect Profile: place side' 'on the side'
$flushB = Marker-InMiddle 'flush B'
Ui-Mark
Ui-Step 'a click flush with end B'       { Ui-Click $flushB[0] $flushB[1] }
Ui-ExpectNew 'BUILD side click: From End B, Offset 0 mm (end_b)' 'flush with end B: From End B, Offset 0'
Expect-Inspect @('; snap end_b at 0.0 mm from b') 'the preview flush with end B'
Ui-Mark
Ui-Step 'offset 30'                      { Ui-TypeIn 'Panel Profile input offset' '30' }
Ui-ExpectNew 'BUILD side offset typed: from end B' 'a typed offset keeps From'
Expect-Inspect @(' at 30.0 mm from b') '30 mm from end B'
Ui-Mark
Ui-Step 'from end A'                     { Wheel-Choice 'from' -1 }
Ui-ExpectNew 'BUILD From End A keeps the part in place: Offset 350 mm' 'From End A keeps the part in place'
Expect-Inspect @(' at 350.0 mm from a') '350 mm from end A: the same place'
Ui-Mark
Ui-Step 'from the middle'                { Wheel-Choice 'from' 2 }
Ui-ExpectNew 'BUILD From Middle keeps the part in place: Offset 160 mm' 'From Middle keeps the part in place'
Expect-Inspect @(' at 160.0 mm from middle') '160 mm from the middle: the same place'
Ui-Mark
Ui-Step 'from end B again'               { Wheel-Choice 'from' -1 }
Ui-ExpectNew 'BUILD From End B keeps the part in place: Offset 30 mm' 'From End B again'
Ui-Mark
Ui-Step 'offset 0'                       { Ui-TypeIn 'Panel Profile input offset' '0' }
Expect-Inspect @(' at 0.0 mm from b') 'offset 0 from end B: flush with end B'
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

Write-Host '--- 4. Connector'
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

Write-Host '--- 5. Panel'
Ui-Mark
Ui-Step 'Panel (F5)'                     { Ui-Key 'F5' }
Ui-ExpectNew 'BUILD parts panel: 1' 'the picker lists the panel'
Ui-Step "the frame's inner face"         { Pick-Face $b1 $side1 }
Ui-ExpectNew 'Inspect Panel: panel Panel test' "the panel's preview"
Ui-Mark
Ui-Step 'Place (Enter)'                  { Ui-Key 'Return' }
Ui-ExpectNew 'Placed panel Panel test as ' 'the panel placed' 30
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }

Write-Host '--- 6. Linear Part: a rail and a carriage'
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

Write-Host '--- 6b. Ends: an end of the third profile mitred and bored'
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

Write-Host '--- 7. Save, reopen, Cut List'
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
Ui-ExpectNew ' 45 deg (' 'the mitred end'
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

Write-Host '--- 8. The part picker (mitcad#418)'
# A 30 x 30 profile of I-type slot 6 on a 30 mm grid in the library's next
# version, fetched before the design is opened again.
$profile30 = @((Bar 30), (Frame 'slot_x+_1' '[15, 0, 0]' '[1, 0, 0]' '[0, 0, 1]'),
  (Frame 'slot_x-_1' '[-15, 0, 0]' '[-1, 0, 0]' '[0, 0, 1]'), (Frame 'slot_y+_1' '[0, 15, 0]' '[0, 1, 0]' '[0, 0, 1]'),
  (Frame 'slot_y-_1' '[0, -15, 0]' '[0, -1, 0]' '[0, 0, 1]')) -join "`n"
Design 'profiles\p30-test.mitcad' ($profile30 + $end)
$p30Entry = @'
    {"id": "p30-test", "path": "profiles/p30-test.mitcad", "name": "Profile 30 test", "category": "profiles",
     "supplier": "Test supplier", "part_number": "P30-T", "designation": "Profile 30 test L={L}",
     "build": {"role": "profile", "system": {"type": "I", "slot": 6, "grid": 30},
       "length": {"parameter": "L", "min": "10 mm", "max": "3000 mm", "stock": ["1000 mm"]},
       "section": {"width": "30 mm", "height": "30 mm"},
       "slot": {"depth": "8 mm", "opening": "6.2 mm", "inner_width": "16 mm"}, "mass_per_metre": "0.8 kg",
       "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
                  {"name": "slot_x+_1", "kind": "slot"}, {"name": "slot_x-_1", "kind": "slot"},
                  {"name": "slot_y+_1", "kind": "slot"}, {"name": "slot_y-_1", "kind": "slot"}]}},
'@
$manifestPath = Join-Path $library 'mitcad-library.json'
$manifest = [System.IO.File]::ReadAllText($manifestPath)
$bracketEntry = '    {"id": "bracket-20-test"'
if (-not $manifest.Contains($bracketEntry)) { Ui-Fail "the manifest has no '$bracketEntry'" }
$manifest = $manifest.Replace('"library_version": "1.0.0"', '"library_version": "1.1.0"').Replace(
  $bracketEntry, $p30Entry + "`n" + $bracketEntry)
Set-Content -Encoding ascii $manifestPath $manifest
foreach ($arguments in @(@('add', '-A'), @('commit', '-q', '-m', 'A 30 profile'), @('tag', 'v1.1.0'))) {
  $output = & $git.Source -C $library -c user.name=T -c user.email=t@example.invalid @arguments 2>&1
  if ($LASTEXITCODE -ne 0) { Ui-Fail "git $($arguments -join ' '): $($output -join ' / ')" }
}
Run-Cli 'library fetch of v1.1.0' @('library', 'fetch', $library)

# Picker-Click TEXT: a click on a place of the part picker that it logged
# ("Part picker tile <part> at x,y; in the picker x,y"), posted to the
# picker's own window at its own coordinates (a window of its own, which
# the main window's clicks do not reach).
function Picker-Click([string]$Text) {
  $place = $null
  for ($i = 0; $i -lt 30 -and -not $place; $i++) {
    $line = Ui-LogLines | Where-Object { $_.Contains("$Text at ") } | Select-Object -Last 1
    $match = [regex]::Match("$line", 'in the picker (-?\d+),(-?\d+)$')
    if ($match.Success) { $place = @([int]$match.Groups[1].Value, [int]$match.Groups[2].Value) }
    else { Start-Sleep -Milliseconds 200 }
  }
  if (-not $place) { Ui-Fail "no place of the part picker logged for '$Text'" }
  # Ui-Windows gives the array itself (as one object): no @( ) around it.
  $picker = Ui-Windows '^Part Picker'
  if ($picker.Count -eq 0) { Ui-Fail 'no part picker window' }
  $b = $script:UiButtons[1]
  [UiInput]::Move($picker[0], $place[0], $place[1], 0)
  Start-Sleep -Milliseconds 80
  [UiInput]::Button($picker[0], $b[0], $place[0], $place[1], $b[3])
  Start-Sleep -Milliseconds 60
  [UiInput]::Button($picker[0], $b[1], $place[0], $place[1], 0)
}

Ui-StartApp 'build-picker' @('--open', $file)
Ui-ExpectLog "Opened $fileLogged" 'reopened'
Ui-Step 'the BUILD tab'                  { Ui-ClickLogged 'Ribbon tab BUILD' }
Ui-Mark
Ui-Step 'Profile (F3)'                   { Ui-Key 'F3' }
Ui-ExpectNew 'BUILD parts profile: 2 (1 in the design)' "the 30 profile listed beside the design's"
Ui-Step 'search p20 in the panel'        { Ui-TypeIn 'Panel Profile input search' 'p20' }
Ui-Step 'More'                           { Ui-ClickLogged 'Panel Profile input more' }
Expect-Row 'place' 'shown' 'Placement under More'
Ui-Mark
Ui-Step 'Placement: on a plane'          { Wheel-Choice 'place' 3 }
Expect-Inspect @('place grounded', '; part Profile 20 test') 'a ground placement of the 20 profile'
Ui-Mark
Ui-Step 'the part picker (... beside Part)' { Ui-ClickLogged 'Panel Profile button_part' }
Ui-ExpectNew 'Profile: Part button' 'the button beside Part'
Ui-ExpectNew 'Part picker opened: 2 parts for build.profile' 'the picker opened with the two profiles'
Ui-ExpectNew 'Part picker in this design: ' "the design's library parts read"
Ui-ExpectNew 'test-profiles/p20-test x' 'the placed 20 profiles in this design, with their count'
Ui-ExpectNew 'Part picker tile test-profiles/p30-test at ' "the tiles' places"
Ui-Mark
Ui-Step 'the group Profiles'             { Picker-Click 'Part picker group row Profiles' }
Ui-ExpectNew 'Part picker group Profiles: 2 parts' 'the group of profiles'
Ui-ExpectNew 'Part picker tile test-profiles/p30-test at ' "the tiles' places in the group"
Ui-Mark
Ui-Step 'a click on the 30 profile'      { Picker-Click 'Part picker tile test-profiles/p30-test' }
Ui-ExpectNew 'Part picker chose test-profiles/p30-test' 'chosen in the picker'
Expect-Inspect @('place grounded', '; part Profile 30 test') 'the preview uses the chosen profile at once'
Ui-Mark
Ui-Step 'star it (Star)'                 { Picker-Click 'Part picker place star' }
Ui-ExpectNew 'Part picker star p30-test on' 'the 30 profile starred'
Ui-ExpectNew 'Part picker group Profiles: 2 parts, first test-profiles/p30-test' 'the starred profile first'
Ui-Mark
Ui-Step 'pin the picker'                 { Picker-Click 'Part picker place pin' }
Ui-ExpectNew 'Part picker pinned on' 'pinned'
Ui-Mark
Ui-Step 'close Profile (Escape)'         { Ui-Key 'Escape' }
Ui-ExpectNew 'Part picker stays pinned after build.profile' 'the pinned picker stays when Profile ends'
Ui-Mark
Ui-Step 'Connector (F8)'                 { Ui-Key 'F8' }
Ui-ExpectNew 'Part picker follows build.connector: ' 'the pinned picker follows Connector'
Ui-ExpectNew 'Part picker tile test-profiles/bracket-20-test at ' "the fitting connector's tile"
Ui-Mark
Ui-Step 'a click on the bracket'         { Picker-Click 'Part picker tile test-profiles/bracket-20-test' }
Ui-ExpectNew 'Part picker chose test-profiles/bracket-20-test' 'chosen in the picker'
Ui-ExpectNew 'Connector: Connector = Bracket 20 test' 'chosen in Connector'
Ui-Mark
Ui-Step 'unpin the picker'               { Picker-Click 'Part picker place pin' }
Ui-ExpectNew 'Part picker pinned off' 'unpinned'
Ui-Mark
Ui-Step 'close Connector (Escape)'       { Ui-Key 'Escape' }
Ui-ExpectNew 'Part picker closed with build.connector' 'the picker closes with the command'
Ui-Mark
Ui-Step 'Profile again (F3)'             { Ui-Key 'F3' }
Ui-ExpectNew 'BUILD parts profile: 2 (1 in the design)' 'Profile again'
Ui-Mark
Ui-Step 'System I6 30 in the panel'      { Wheel-Choice 'system' 2 }
Ui-ExpectNew 'Profile: System = I6 30' "the panel's system"
Ui-Step 'More'                           { Ui-ClickLogged 'Panel Profile input more' }
Expect-Row 'place' 'shown' 'Placement under More'
Ui-Mark
Ui-Step 'Placement: on a plane'          { Wheel-Choice 'place' 3 }
Expect-Inspect @('place grounded', '; part Profile 30 test') 'the 30 profile, the only one of I6 30'
Ui-Mark
Ui-Step 'the part picker again'          { Ui-ClickLogged 'Panel Profile button_part' }
Ui-ExpectNew 'Part picker opened: 2 parts for build.profile' 'the picker opened again'
Ui-ExpectNew 'Part picker group All parts: 1 parts, first test-profiles/p30-test' "with the panel's system"
Ui-ExpectNew 'Part picker group row Favourites at ' "the groups' places"
Ui-Mark
Ui-Step 'Favourites'                     { Picker-Click 'Part picker group row Favourites' }
Ui-ExpectNew 'Part picker group Favourites: 1 parts, first test-profiles/p30-test' 'Favourites has the starred profile'
Place 'Profile 30 test'
Ui-ExpectNew 'test-profiles/p30-test x1' 'In this design has it after the placement'
Ui-ExpectNew 'Part picker group row In this design at ' "the groups' places again"
Ui-Mark
Ui-Step 'In this design'                 { Picker-Click 'Part picker group row In this design' }
Ui-ExpectNew 'Part picker group In this design: 1 parts, first test-profiles/p30-test' 'In this design lists the placed profile (System I6 30)'
Ui-Mark
Ui-Step 'System I5 20 in the panel'      { Wheel-Choice 'system' -1 }
Ui-ExpectNew 'Part picker group In this design: 1 parts, first test-profiles/p20-test' "the picker's system follows the panel's"
Expect-Inspect @('; part Profile 20 test') "the panel's part follows its system"
Ui-Mark
Ui-Step 'Close (Escape)'                 { Ui-Key 'Escape' }
Ui-ExpectNew 'Part picker closed with build.profile' 'the picker closes with Profile'

Ui-Finish 'UI Windows BUILD test'
