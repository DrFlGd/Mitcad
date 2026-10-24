# SPDX-License-Identifier: MIT
# check-all sources: app/commands/PulleyCommands.cpp app/framework/CommandSession.cpp
# check-all sources: core/model/src/features/pulley.rs core/model/src/features/pulley_profile.rs core/model/src/api/pulleys.rs
# check-all sources: app/commands/SeatInputs.cpp core/model/src/features/seat.rs core/model/src/api/seats.rs tools/cli
# Input-driven Windows UI test of Pulley (mitcad#562) in the build VM
# (tools/ui-windows-lib.ps1 says how the input is posted); the Linux
# counterpart is tools/ui-pulley-test.sh:
#   1. Pulley in an empty design: the panel starts on XY with a GT2 2 mm
#      pulley of 20 teeth; its note shows the pitch diameter (20 x 2 / pi =
#      12.7324 mm). In the preview: flanges 24 across, then 30 teeth (pitch
#      diameter 60 / pi = 19.0986 mm), then a flange on one side only. OK
#      makes Pulley1 (F1.b0).
#   2. Pulley1 edited: it opens with its flange on one side and its 30
#      teeth; 20 teeth again, OK.
#   3. Pulley again (20 teeth): Belt With a face of Pulley1, 80 mm apart:
#      the note shows the belt round both (two equal pulleys: 2 x 80 +
#      20 x 2 = 200 mm, 100 teeth); the catalogue lengths 188, 202 and 210
#      give the nearest one (202 mm). OK makes F2.b0.
#   4. On a shaft (mitcad#578), in a design mitcad-cli makes (an 8 mm
#      cylinder 30 high, Cylinder1, F1.b0): Pulley with Shaft the
#      cylinder's top edge: the Bore Clearance starts as the printed-fit
#      parameter shaft_bore_clearance (0.2 mm, mitcad#623), so the note
#      shows the bore the shaft gives as 8.2 mm; an offset of 2 mm and a
#      typed bore clearance of 0.3 mm make it 8.3 mm; OK makes the pulley
#      on the shaft (F2.b0).
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-pulley-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-pulley'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'solid.pulley' = 'F7' }

# The shaft of part 4, made by mitcad-cli.
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$shaftJson = Join-Path $work 'shaft.json'
Set-Content -Encoding ascii $shaftJson @'
[
  {"cmd": "add_feature", "def": {"type": "cylinder", "plane": "xy", "center": [0, 0],
    "diameter": 8, "height": 30, "operation": "new_body"}}
]
'@
$shaft = Join-Path $work 'shaft.mitcad'
$output = & $cli run $shaftJson --save $shaft 2>&1
if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }

Ui-StartApp 'pulley'
Start-Sleep -Milliseconds 800

Write-Host '--- A GT2 pulley on XY, its teeth and flanges changed in the preview'
Ui-Mark
Ui-Step 'pulley (F7)'                    { Ui-Key 'F7' }
Ui-ExpectNew 'Preview Pulley: ok' 'the pulley previewed' 30
Ui-ExpectNew 'Panel Pulley note: Pitch diameter 12.7324 mm' 'its pitch diameter shown'
Ui-Mark
Ui-Step 'flange diameter 24'             { Ui-TypeIn 'Panel Pulley input flange_diameter' '24' }
Ui-ExpectNew 'Preview Pulley: ok' 'the wider flanges previewed' 30
Ui-Mark
Ui-Step 'teeth 30'                       { Ui-TypeIn 'Panel Pulley input teeth' '30' }
Ui-ExpectNew 'Panel Pulley note: Pitch diameter 19.0986 mm' 'the pitch diameter of 30 teeth' 30
Ui-Mark
# One step towards the far side: the entry before Both.
Ui-Step 'flanges: One'                   { Ui-WheelLogged 'Panel Pulley input flanges' 1 }
Ui-ExpectNew 'Preview Pulley: ok' 'one flange previewed' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added pulley (Toothed), GT2 2 mm, 30 teeth' 'the pulley added' 30
Ui-ExpectNew '(F1.b0): volume ' 'its body made' 30

Write-Host '--- Pulley1 edited back to 20 teeth'
Ui-Mark
Ui-Step 'edit Pulley1 (double-click)'    { Ui-DoubleClickLogged 'Timeline Pulley1' }
Ui-ExpectNew 'Editing F1 with Pulley' 'Pulley1 opens for editing' 30
Ui-ExpectNew 'flanges=one' 'with its flange on one side'
Ui-ExpectNew 'Panel Pulley note: Pitch diameter 19.0986 mm' 'and its 30 teeth' 30
Ui-Mark
Ui-Step 'teeth 20'                       { Ui-TypeIn 'Panel Pulley input teeth' '20' }
Ui-ExpectNew 'Panel Pulley note: Pitch diameter 12.7324 mm' 'the edit previewed' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F1' 'Pulley1 edited' 30

Write-Host '--- A second pulley on the belt'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-Mark
Ui-Step 'pulley (F7)'                    { Ui-Key 'F7' }
Ui-ExpectNew 'Preview Pulley: ok' 'the panel open' 30
Ui-Mark
Ui-Step 'Belt With'                      { Ui-ClickLogged 'Panel Pulley input belt_with' }
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-ExpectNew 'Pick face F1.b0/F1:' 'a face of Pulley1 can be picked'
$face = Ui-LastMatch 'Pick face F1\.b0/(F1:\S+) at'
Ui-Mark
Ui-Step "click $face"                    { Ui-ClickPick "face F1.b0/$face at" }
Ui-ExpectNew 'Preview Pulley: ok' 'previewed beside Pulley1' 30
Ui-Mark
Ui-Step 'centre distance 80'             { Ui-TypeIn 'Panel Pulley input center_distance' '80' }
Ui-ExpectNew 'belt length 200 mm (100 teeth)' 'the belt length logged' 30
Ui-Mark
Ui-Step 'catalogue lengths'              { Ui-TypeIn 'Panel Pulley input belt_lengths' '188, 202, 210' }
Ui-ExpectNew 'nearest catalogue length 202 mm' 'the nearest catalogue length logged' 30
$note = Ui-LastMatch '(Panel Pulley note: .*)'
Write-Host "     $note"
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added pulley (Toothed), GT2 2 mm, 20 teeth' 'the second pulley added' 30
Ui-ExpectNew '(F2.b0): volume ' 'its body made' 30

Write-Host '--- A pulley on a shaft (mitcad#578)'
Ui-StartApp 'pulley-shaft' @('--open', $shaft)
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F1.b0' 'the shaft shown'
Ui-Mark
Ui-Step 'pulley (F7)'                    { Ui-Key 'F7' }
Ui-ExpectNew 'Preview Pulley: ok' 'the panel open on XY' 30
Ui-Step 'teeth 20'                       { Ui-TypeIn 'Panel Pulley input teeth' '20' }
Ui-Mark
Ui-Step 'Shaft'                          { Ui-ClickLogged 'Panel Pulley input shaft' }
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-ExpectNew 'Pick edge F1.b0/E{F1:side0|F1:top} at' "the shaft's top edge can be picked"
Ui-Mark
Ui-Step "click the shaft's top edge"     { Ui-ClickPick 'edge F1.b0/E{F1:side0|F1:top} at' }
Ui-ExpectNew 'bore 8.2 mm (shaft 8 mm)' 'the bore the shaft gives, with the printed fit' 30
Ui-Mark
Ui-Step 'offset 2'                       { Ui-TypeIn 'Panel Pulley input seat_offset' '2' }
Ui-ExpectNew 'Preview Pulley: ok' '2 mm down the shaft previewed' 30
Ui-Mark
Ui-Step 'bore clearance 0.3'             { Ui-TypeIn 'Panel Pulley input bore_clearance' '0.3' }
Ui-ExpectNew 'bore 8.3 mm (shaft 8 mm)' 'a typed clearance replaces the parameter' 30
$note = Ui-LastMatch '(Panel Pulley note: .*)'
Write-Host "     $note"
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added pulley (Toothed), GT2 2 mm, 20 teeth' 'the pulley on the shaft added' 30
Ui-ExpectNew '(F2.b0): volume ' 'its body made' 30

Ui-Finish 'UI Windows pulley test'
