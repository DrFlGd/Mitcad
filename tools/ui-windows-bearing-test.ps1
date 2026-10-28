# SPDX-License-Identifier: MIT
# check-all sources: app/commands/BearingCommands.cpp app/framework/CommandSession.cpp
# check-all sources: core/model/src/features/printable_bearing.rs core/model/src/features/bearing_sizes.rs
# check-all sources: core/model-api/src/bearings.rs tools/cli
# check-all sources: app/commands/PlaceInputs.cpp core/model/src/features/seat.rs core/model-api/src/seats.rs
# Input-driven Windows UI test of Bearing (mitcad#577) in the build VM
# (tools/ui-windows-lib.ps1 says how the input is posted), on a design
# mitcad-cli makes: a shaft 10 mm across and 20 long at (60, 0)
# (Extrude1, F2.b0, its round face F2:side(c1)), an 8 mm cylinder 30 high
# at (0, 60) (Cylinder1, F3.b0, its top edge E{F3:side0|F3:top}) and a
# block 40 x 40 x 10 at (0, 120) with a 22 mm hole through it (Extrude2
# F5.b0, the hole's wall F7:side(c1)). The Linux counterpart is
# tools/ui-bearing-test.sh:
#   1. Bearing, placed by a click on the XY plane and both distances 0
#      (it waits for a pick, mitcad#656): a 608 of dumbbells; its note
#      shows the model's values (15 mm pitch diameter, 10 elements of 3.8
#      mm, walls 1.2 mm, the flanks' gap 0.4 / sqrt(2)). 12 elements are
#      too many (the note says so), 8 are fine; as a New Body, OK makes
#      Bearing1 (F8.b0 ...).
#   2. Bearing1 edited: it opens with its 8 elements; 10 again, OK.
#   3. Bearing again, Groove Into the shaft's round face: the 608's outer
#      race on the shaft, the raceway cut into it (7 dumbbells of 4.9333 mm
#      on a 13.2667 mm pitch circle, the outer race's wall 1.5 mm). OK.
#   4. On a shaft's end (mitcad#578): Place On the cylinder's top edge takes
#      the 608 (its bore fits the 8 mm shaft), 5 mm down the shaft. OK.
#   5. In a hole: Place On the hole's top rim takes the 608 by its outside
#      (22 mm), flush with the block's top face. OK.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-bearing-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-bearing'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'solid.bearing' = 'F7' } -StepPause 0
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null

$modelScript = Join-Path $work 'model.json'
$design = Join-Path $work 'bearing.mitcad'
Set-Content -Encoding ascii $modelScript @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [60, 0], "diameter": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "cylinder", "plane": "xy", "center": [0, 60],
    "diameter": 8, "height": 30, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F4", "corner": [-20, 100], "width": 40, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F4", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F6", "center": [0, 120], "diameter": 22},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F6", "region": "r{c1}"}],
    "extent": {"type": "through_all"}, "operation": "cut", "participants": ["F5.b0"]}}
]
'@
$made = & $cli run $modelScript --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

Ui-StartApp 'bearing' @('--open', $design)
Ui-ExpectLog 'bearing.mitcad' 'the design opened'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F2.b0' 'the shaft shown'


# Answer-Hybrid: after OK of a bearing made as a new component, the
# question that the root already has modelled bodies (the shaft, the block
# and the grooved bearing's bodies) answered "Continue as Is" (Alt+O), when
# it is asked; the bearing is added then.
function Answer-Hybrid {
  for ($i = 0; $i -lt 50; $i++) {
    $since = @(Ui-LogLines | Select-Object -Skip $script:UiMark)
    if ($since | Where-Object { $_.StartsWith('Hybrid assembly question:') }) {
      Ui-FocusDialog '^Hybrid Assembly$'
      Ui-Step 'continue as is (Alt+O)'      { Ui-Key 'alt+o' }
      Ui-FocusMain
      return
    }
    if ($since | Where-Object { $_.StartsWith('Added bearing') }) { return }
    Start-Sleep -Milliseconds 200
  }
}
Write-Host '--- 1. A 608 of dumbbells on XY'
Ui-Mark
Ui-Step 'bearing (F7)'                   { Ui-Key 'F7' }
Ui-PlaceOnXy 'Bearing'
Ui-ExpectNew 'Panel Bearing note: Pitch diameter 15 mm, 10 elements of 3.8 mm, walls 1.2 mm and 1.2 mm, least gap 0.2828 mm' 'its values shown' 30
Ui-Mark
Ui-Step 'elements 12'                    { Ui-TypeIn 'Panel Bearing input element_count' '12' }
Ui-ExpectNew 'Panel Bearing error: too many elements for the pitch circle: at most 10' 'too many elements refused' 30
Ui-Mark
Ui-Step 'elements 8'                     { Ui-TypeIn 'Panel Bearing input element_count' '8' }
Ui-ExpectNew 'Panel Bearing note: Pitch diameter 15 mm, 8 elements of 3.8 mm' '8 elements' 30
Ui-Mark
# One step towards the user: the entry after New Component.
Ui-Step 'operation: New Body'            { Ui-WheelLogged 'Panel Bearing input operation' -1 }
Ui-ExpectNew 'Preview Bearing: ok' 'previewed as new bodies' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added bearing (Dumbbell), 608' 'the bearing added' 30
Ui-ExpectNew '(F8.b0): volume ' 'its inner race made' 30

Write-Host '--- 2. Bearing1 edited back to 10 elements'
Ui-Mark
Ui-Step 'edit Bearing1 (double-click)'   { Ui-DoubleClickLogged 'Timeline Bearing1' }
Ui-ExpectNew 'Editing F8 with Bearing' 'Bearing1 opens for editing' 30
Ui-ExpectNew 'Panel Bearing note: Pitch diameter 15 mm, 8 elements of 3.8 mm' 'with its 8 elements' 30
Ui-Mark
Ui-Step 'elements 10'                    { Ui-TypeIn 'Panel Bearing input element_count' '10' }
Ui-ExpectNew 'Panel Bearing note: Pitch diameter 15 mm, 10 elements of 3.8 mm' 'the edit previewed' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F8' 'Bearing1 edited' 30

Write-Host '--- 3. A raceway cut into the shaft'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-Mark
Ui-Step 'bearing (F7)'                   { Ui-Key 'F7' }
Ui-ExpectNew 'Command Bearing started' 'the panel opened, waiting for a pick' 30
Ui-Mark
Ui-Step 'Groove Into'                    { Ui-ClickLogged 'Panel Bearing input groove' }
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-ExpectNew 'Pick face F2.b0/F2:side(c1) at' "the shaft's round face can be picked"
Ui-Mark
Ui-Step 'click the shaft'                { Ui-ClickPick 'face F2.b0/F2:side(c1) at' }
Ui-ExpectNew 'Preview Bearing: ok' 'the raceway previewed' 30
Ui-ExpectNew 'Panel Bearing note: Pitch diameter 13.2667 mm, 7 elements of 4.9333 mm, walls 1.5 mm, least gap 0.2828 mm' 'the values of the bearing on the shaft' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added bearing (Dumbbell), 608, groove into a face' 'the raceway cut' 30
Ui-ExpectNew '(F9.b0): volume ' 'the outer race made' 30

Write-Host "--- 4. On the cylinder's top end, 5 mm down (mitcad#578)"
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-Mark
Ui-Step 'bearing (F7)'                   { Ui-Key 'F7' }
Ui-ExpectNew 'Command Bearing started' 'the panel opened, waiting for a pick' 30
Ui-Mark
Ui-Step 'Place On'                       { Ui-ClickLogged 'Panel Bearing input at' }
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-ExpectNew 'Pick edge F3.b0/E{F3:side0|F3:top} at' "the cylinder's top edge can be picked"
Ui-Mark
Ui-Step 'click the top edge'             { Ui-ClickPick 'edge F3.b0/E{F3:side0|F3:top} at' }
Ui-ExpectNew 'Preview Bearing: ok' 'the bearing previewed on the shaft' 30
Ui-ExpectNew 'on a 8 mm shaft' 'the size whose bore fits the 8 mm shaft' 30
Ui-Mark
Ui-Step 'More'                           { Ui-ClickLogged 'Panel Bearing input at_more' }
Ui-Step 'offset 5'                       { Ui-TypeIn 'Panel Bearing input at_offset' '5' }
Ui-ExpectNew 'Preview Bearing: ok' '5 mm down the shaft previewed' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Answer-Hybrid
Ui-ExpectNew 'Added bearing (Dumbbell), 608, on a shaft or in a hole' 'the bearing on the shaft added' 30

Write-Host "--- 5. In the hole, flush with the block's top face"
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-Mark
Ui-Step 'bearing (F7)'                   { Ui-Key 'F7' }
Ui-ExpectNew 'Command Bearing started' 'the panel opened, waiting for a pick' 30
Ui-Mark
Ui-Step 'Place On'                       { Ui-ClickLogged 'Panel Bearing input at' }
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-ExpectNew 'Pick edge F5.b0/E{' "the block's edges can be picked"
$rim = Ui-LastMatch 'Pick edge F5\.b0/(E\{[^ ]*F5:end\([^ ]*F7:side\(c1\)[^ ]*\}|E\{[^ ]*F7:side\(c1\)[^ ]*F5:end\([^ ]*\}) at'
if (-not $rim) { Ui-Fail 'no top rim of the hole to pick' }
Ui-Mark
Ui-Step "click the hole's top rim"       { Ui-ClickPick "edge F5.b0/$rim at" }
Ui-ExpectNew 'Preview Bearing: ok' 'the bearing previewed in the hole' 30
Ui-ExpectNew 'in a 22 mm hole' 'the size whose outside fits the 22 mm hole' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Answer-Hybrid
Ui-ExpectNew 'Added bearing (Dumbbell), 608, on a shaft or in a hole' 'the bearing in the hole added' 30

Ui-Finish 'UI Windows bearing test'
