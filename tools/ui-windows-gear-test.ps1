# SPDX-License-Identifier: MIT
# check-all sources: app/commands/GearCommands.cpp app/framework/CommandSession.cpp
# check-all sources: core/model/src/features/gear.rs core/model/src/features/gear_profile.rs geometry/src/gear.cpp
# Input-driven Windows UI test of Gear (mitcad#472) in the build VM
# (tools/ui-windows-lib.ps1 says how the input is posted):
#   1. Gear in an empty design: the panel starts on XY with a spur gear of
#      20 teeth, module 2, 10 wide; its note shows the derived diameters
#      (pitch 40, outside 44, root 35). OK makes Gear1 (F1.b0) of the
#      section's area times the width (12371.968 mm3).
#   2. Gear again: Rack picked by its icon, Mesh With a face of Gear1, 10
#      teeth, 12 high: the note shows its pitch line (20 pi = 62.8319 mm) and
#      the centre distance (20 mm); OK makes the rack below the gear
#      (F2.b0, its section's area times the width: 6218.320 mm3).
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-gear-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-gear'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'solid.gear' = 'F7' }

Ui-StartApp 'gear'
Start-Sleep -Milliseconds 800

Write-Host '--- A spur gear on XY'
Ui-Mark
Ui-Step 'gear (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Preview Gear: ok' 'the spur gear previewed' 30
Ui-ExpectNew 'Panel Gear note: Pitch diameter 40 mm, outside 44 mm, root 35 mm' 'the derived diameters shown'
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added gear (Spur), 20 teeth, module 2' 'the gear added' 30
Ui-ExpectVolume 'New body \S+ \(F1\.b0\): volume ([0-9.]+) mm3' 12371.968 'Gear1' 1e-5

Write-Host '--- A rack meshing it'
Ui-Mark
Ui-Step 'gear (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Preview Gear: ok' 'the panel open' 30
Ui-Step 'Rack'                           { Ui-ClickLogged 'Panel Gear input kind=rack' }
Ui-Step 'teeth 10'                       { Ui-TypeIn 'Panel Gear input teeth' '10' }
Ui-Step 'height 12'                      { Ui-TypeIn 'Panel Gear input height' '12' }
Ui-Mark
Ui-Step 'Mesh With'                      { Ui-ClickLogged 'Panel Gear input mesh_with' }
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-ExpectNew 'Pick face F1.b0/F1:' 'a face of Gear1 can be picked'
$face = Ui-LastMatch 'Pick face F1\.b0/(F1:[a-z_]+\(\d+\)) at'
Ui-Mark
Ui-Step "click $face"                    { Ui-ClickPick "face F1.b0/$face at" }
Ui-ExpectNew 'Panel Gear note: Pitch line 62.8319 mm, centre distance 20 mm' 'the rack''s pitch line and centre distance shown' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added gear (Rack), 10 teeth, module 2' 'the rack added' 30
Ui-ExpectVolume 'New body \S+ \(F2\.b0\): volume ([0-9.]+) mm3' 6218.320 'the rack' 1e-5

Ui-Finish 'UI Windows gear test'
