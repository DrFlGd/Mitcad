# SPDX-License-Identifier: MIT
# check-all sources: app/commands/PlaceInputs.cpp app/commands/PlaceGeometry.cpp app/commands/PrimitiveCommands.cpp
# check-all sources: app/commands/PulleyCommands.cpp app/commands/GearCommands.cpp app/framework/ManipulatorOverlay.cpp
# check-all sources: app/framework/CommandSession.cpp core/model/src/features/place_at.rs tools/cli
# Input-driven Windows UI test of placement by picked geometry (mitcad#656)
# in the build VM (tools/ui-windows-lib.ps1 says how the input is posted);
# the Linux counterpart is tools/ui-place-test.sh. A design mitcad-cli
# makes as earlier versions wrote them: a 60 x 40 x 20 block (Sketch1 F1,
# Extrude1 F2), a cylinder on XY at (100, 0) by its plane and centre
# (Cylinder1 F3), a GT2 pulley on XY at (0, 80) (Pulley1 F4) and one beside
# it by `belt_with`, 80 mm along X (Pulley2 F5), a spur gear on XY at
# (0, -80) (Gear1 F6) and one meshing it at 90 degrees by
# `mesh_with.angle` (Gear2 F7):
#   1. Cylinder1 edited: it opens on XY, 100 mm from the Y axis and 0 from
#      the X axis, no typed coordinates; 25 high, OK: saved with its plane
#      and centre as before (the placement unchanged).
#   2. Pulley2 edited: placed where it is, with Pulley1 as its Check Belt
#      (200 mm belt, its rows under it); OK keeps `belt_with`.
#   3. Gear2 edited: placed where its mesh had it ((0, -40) on XY), Mesh
#      With Gear1; OK keeps `mesh_with.angle`.
#   4. Box on the block's top face: measured from the two nearest edges,
#      its x along Reference 1, the distances typed 10 and 12; More
#      opened, Angle 10 in the panel shows its value box in the view
#      (hidden at 0), where it is turned to 30 degrees. OK makes a 30 x 20 x 10 box (6000 mm3).
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-place-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-place'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'solid.box' = 'F7' } -StepPause 0

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$rect = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
$earlierJson = Join-Path $work 'earlier.json'
Set-Content -Encoding ascii $earlierJson (@'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "cylinder", "plane": "xy", "center": [100, 0],
    "diameter": 10, "height": 20, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "pulley", "kind": "toothed", "profile": "gt2_2mm", "plane": "xy",
    "center": [0, 80], "teeth": 20, "belt_width": 6, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "pulley", "kind": "toothed", "profile": "gt2_2mm",
    "belt_with": {"pulley": "F4", "center_distance": 80, "angle": 0}, "teeth": 20, "belt_width": 6,
    "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "gear", "kind": "spur", "plane": "xy", "center": [0, -80],
    "module": 2, "teeth": 20, "face_width": 10, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "gear", "kind": "spur", "mesh_with": {"gear": "F6", "angle": 1.5707963267948966},
    "module": 2, "teeth": 20, "face_width": 10, "operation": "new_body"}}
]
'@).Replace('RECT', $rect)
$earlier = Join-Path $work 'earlier.mitcad'
$output = & $cli run $earlierJson --save $earlier 2>&1
if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }

Ui-StartApp 'place' @('--open', $earlier)
Start-Sleep -Milliseconds 800
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F2.b0' 'the design shown'

Write-Host '--- 1. A cylinder written with its plane and centre'
Ui-Mark
Ui-Step 'edit Cylinder1 (double-click)'  { Ui-DoubleClickLogged 'Timeline Cylinder1' }
Ui-ExpectNew 'Editing F3 with Cylinder' 'Cylinder1 opens for editing' 30
Ui-ExpectNew 'Place Cylinder: face plane xy, 100 mm from axis y, 0 mm from axis x' 'on XY from the origin axes'
Ui-Mark
Ui-Step 'height 25'                      { Ui-TypeIn 'Panel Cylinder input height' '25' }
Ui-ExpectNew 'Preview Cylinder: ok' '25 high previewed' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F3' 'Cylinder1 edited, its plane and centre kept' 30
Ui-ExpectVolume 'Body \S+ \(F3\.b0\): volume [0-9.]+ -> ([0-9.]+) mm3' ([Math]::PI * 25 * 25) '25 high, where it was'

Write-Host '--- 2. A pulley written beside another (belt_with)'
Ui-Mark
Ui-Step 'edit Pulley2 (double-click)'    { Ui-DoubleClickLogged 'Timeline Pulley2' }
Ui-ExpectNew 'Editing F5 with Pulley' 'Pulley2 opens for editing' 30
Ui-ExpectNew 'Place Pulley: face plane xy, 80 mm from axis y, 80 mm from axis x' 'where the belt had it'
Ui-ExpectNew 'Pulley Check Belt: 1 feature' 'Pulley1 checks the belt'
Ui-ExpectNew 'Pulley belt check with F4: Belt length 200 mm (100 teeth)' 'the belt round both' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F5' 'Pulley2 saved as it was (belt_with)' 30

Write-Host '--- 3. A gear written meshing another at an angle (mesh_with.angle)'
Ui-Mark
Ui-Step 'edit Gear2 (double-click)'      { Ui-DoubleClickLogged 'Timeline Gear2' }
Ui-ExpectNew 'Editing F7 with Gear' 'Gear2 opens for editing' 30
Ui-ExpectNew 'Place Gear: face plane xy, 0 mm from axis y, -40 mm from axis x' 'where the mesh had it'
Ui-ExpectNew 'Gear Mesh With: 1 feature' 'still meshing Gear1'
Ui-ExpectNew 'Preview Gear: ok' 'previewed where it was' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F7' 'Gear2 saved as it was (mesh_with.angle)' 30

Write-Host "--- 4. A box on the block's top face, measured from its edges"
Ui-Mark
Ui-Step 'box (F7)'                       { Ui-Key 'F7' }
Ui-ExpectNew 'Command Box started' 'the panel opened' 30
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-Mark
Ui-Step 'click the top face'             { Ui-ClickPick "face F2.b0/F2:end($rect)" }
Ui-ExpectNew 'Box Place On: 1 face' 'the top face picked'
Ui-ExpectNew 'Place Box: face face F2:end(' 'measured from two of its edges'
Ui-ExpectNew 'Value box Box at_offset1 at ' 'Distance 1 a dimension in the view' 30
Ui-Mark
Ui-Step 'distance 1: 10'                 { Ui-TypeIn 'Panel Box input at_offset1' '10' }
Ui-Step 'distance 2: 12'                 { Ui-TypeIn 'Panel Box input at_offset2' '12' }
Ui-ExpectNew 'Preview Box: ok' 'previewed at the distances' 30
if ((Ui-LogLines | Select-Object -Skip ($script:UiMark + 1)) -match '^Value box Box at_angle at ') {
  Ui-Fail "the angle's value box shown at 0"
}
Ui-Mark
Ui-Step 'More'                           { Ui-ClickLogged 'Panel Box input at_more' }
Ui-Step 'angle 10'                       { Ui-TypeIn 'Panel Box input at_angle' '10' }
Ui-ExpectNew 'Value box Box at_angle at ' "the angle's value box at its ring, not 0" 30
Ui-Mark
Ui-Step "the angle's value box"          { Ui-ClickLogged 'Value box Box at_angle' }
Ui-ExpectNew 'Value box Box at_angle opened: 10' 'a field over the view edits it'
Ui-Step 'type 30'                        { Ui-Type '30' }
Ui-Step 'Enter keeps it'                 { Ui-Key 'Return' }
Ui-ExpectNew 'Value box Box at_angle closed: 30' 'turned 30 degrees in the view'
Ui-ExpectNew 'Preview Box: ok' 'the turned box previewed' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added box (New Body)' 'the box added' 30
Ui-ExpectVolume 'New body \S+ \([A-Z0-9]+\.b0\): volume ([0-9.]+) mm3' 6000 '30 x 20 x 10'

Ui-Finish 'UI Windows place test'
