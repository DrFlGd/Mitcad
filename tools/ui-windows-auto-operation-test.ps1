# SPDX-License-Identifier: MIT
# check-all sources: app/commands/ExtrudeCommands.cpp app/commands/CommandSupport.cpp
# check-all sources: app/framework/CommandSession.cpp app/framework/CommandSessionAuto.cpp
# check-all sources: app/framework/AutoOperationPolicy.hpp app/framework/Command.hpp
# check-all sources: core/model/src/document/auto_operation.rs core/model/src/api/mod.rs
# check-all sources: geometry/src/contact.cpp geometry/src/boolean.cpp tools/cli
# Input-driven Windows UI test of Extrude's automatic operation
# (mitcad#289, mitcad#575) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted), on a design mitcad-cli makes, as tools/ui-auto-operation-test.sh
# does on Linux: a 40 x 30 block 20 deep under the XY plane (Body1, F2.b0;
# its top face lies in XY), a square on XY over the block (Sketch2, F3) and
# a square on XY far from it (Sketch3, F4).
#   1. Extrude of the far square: New Body, nothing chosen automatically.
#   2. Extrude of the square on the block's face: Join at once; a negative
#      distance makes it Cut, a positive one Join again; OK joins.
#   3. Undone; Extrude of the same square with New Body chosen by hand (the
#      drop-down's wheel: its list cannot open in session 0): the choice
#      stays when the distance goes into the block; OK adds a new body.
#   2a. Editing the committed Join follows Cut/Join until Operation is chosen.
#   2b. A new edit follows geometry until Objects is chosen.
#   4. Editing the deliberate touching New Body keeps its stored choice.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-auto-operation-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-auto-operation'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{}
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$region = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

$modelScript = Join-Path $work 'model.json'
$design = Join-Path $work 'auto.mitcad'
Set-Content -Encoding ascii $modelScript (@'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 40, "height": 30},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "RECT"}],
    "extent": {"type": "distance", "distance": -20}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [10, 10], "width": 10, "height": 10},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F4", "corner": [100, 0], "width": 10, "height": 10}
]
'@ -creplace 'RECT', $region)
$made = & $cli run $modelScript --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

# Expect-NoNew "text" "description": no such line since the mark.
function Expect-NoNew([string]$Text, [string]$Description) {
  if ((Ui-LogLines | Select-Object -Skip $script:UiMark) | Where-Object { $_.Contains($Text) }) {
    Ui-Fail $Description
  }
  Write-Host "ok   $Description"
}

Ui-StartApp 'auto-operation' @('--open', $design)
Ui-ExpectLog 'auto.mitcad' 'the design opened'
Ui-ExpectLog 'Bodies shown: F2.b0' 'the block shown'

Write-Host '--- 1. A profile away from the bodies: New Body'
Ui-Mark
Ui-Step 'extrude (E)'                    { Ui-Key 'e' }
Ui-ExpectNew "Extrude Profiles: 1 profile [profile $region of F4]" 'the far square taken'
Ui-ExpectNew 'Preview Extrude: ok' 'previewed' 30
Ui-Step 'distance 15'                    { Ui-TypeIn 'Panel Extrude input distance' '15' }
Ui-ExpectNew 'Extrude Distance: 15 = 15 mm' '15 mm up'
Ui-WaitIdle
Start-Sleep -Milliseconds 500
Expect-NoNew 'Extrude: automatic Operation' 'no operation chosen: it stays New Body'
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added extrude (New Body), distance 15' 'a new body added' 30

Write-Host '--- 2. A profile on the block''s face: Join, Cut going in, Join coming out'
Ui-Mark
Ui-Step 'extrude (E)'                    { Ui-Key 'e' }
Ui-ExpectNew "Extrude Profiles: 1 profile [profile $region of F3]" 'the square on the face taken'
Ui-ExpectNew 'Extrude: automatic Operation = Join' 'Join at once (10 mm up from the face)' 30
Ui-ExpectNew 'Extrude Local bodies: 1 body [body F2.b0]' 'the block is the object'
Ui-ExpectNew 'Preview Extrude: ok' 'the join previewed' 30
Ui-Mark
Ui-Step 'distance -5'                    { Ui-TypeIn 'Panel Extrude input distance' '-5' }
Ui-ExpectNew 'Extrude: automatic Operation = Cut' 'Cut going into the block' 30
Ui-ExpectNew 'Preview Extrude: ok' 'the cut previewed' 30
Ui-Mark
Ui-Step 'distance 5'                     { Ui-TypeIn 'Panel Extrude input distance' '5' }
Ui-ExpectNew 'Extrude: automatic Operation = Join' 'Join again coming out' 30
Ui-ExpectNew 'Preview Extrude: ok' 'the join previewed again' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added extrude (Join), distance 5' 'joined' 30
Ui-ExpectVolume 'Body Body1 \(F2\.b0\): volume 24000\.000 -> ([0-9.]+) mm3' 24500 'the block grew by the square''s 500 mm3'
Write-Host '--- 2a. Editing an automatic Join follows geometry until Operation is chosen'
Ui-Mark
Ui-Step 'edit automatic Extrude3'         { Ui-DoubleClickLogged 'Timeline Extrude3' }
Ui-ExpectNew 'stored operation matches automatic choice; editing follows geometry' 'the stored Join matches the initial probe' 30
Ui-ExpectNew 'Preview Extrude: ok' 'the stored Join previewed' 30
Ui-Mark
Ui-Step 'edited distance -5'              { Ui-TypeIn 'Panel Extrude input distance' '-5' }
Ui-ExpectNew 'Extrude: automatic Operation = Cut' 'the edit cuts going into the block' 30
Ui-ExpectNew 'Preview Extrude: ok' 'the edited Cut previewed' 30
Ui-Mark
Ui-Step 'edited distance 5'               { Ui-TypeIn 'Panel Extrude input distance' '5' }
Ui-ExpectNew 'Extrude: automatic Operation = Join' 'the edit joins coming out again' 30
Ui-ExpectNew 'Preview Extrude: ok' 'the edited Join previewed' 30
Ui-Mark
Ui-Step 'manual New Body in edit (wheel)' { Ui-WheelLogged 'Panel Extrude input operation' 1 }
Ui-ExpectNew 'Extrude: Operation = New Body' 'New Body explicitly chosen in the edit'
Ui-ExpectNew 'Extrude: operation chosen by hand' 'Operation stopped edit automation'
Ui-Mark
Ui-Step 'distance -6 after override'      { Ui-TypeIn 'Panel Extrude input distance' '-6' }
Ui-ExpectNew 'Extrude Distance: -6 = -6 mm' 'the manual edit goes into the block'
Ui-ExpectNew 'Preview Extrude: ok' 'the manual New Body previewed' 30
Ui-WaitIdle
Start-Sleep -Milliseconds 500
Expect-NoNew 'Extrude: automatic Operation' 'manual Operation stays chosen for the rest of the edit'
Ui-Step 'cancel manual edit'              { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'the original stored Join retained' 30

Write-Host '--- 2b. Editing Objects also stops automatic operation'
Ui-Mark
Ui-Step 'edit automatic Extrude3 again'   { Ui-DoubleClickLogged 'Timeline Extrude3' }
Ui-ExpectNew 'stored operation matches automatic choice; editing follows geometry' 'a fresh edit probes the stored Join' 30
Ui-ExpectNew 'Preview Extrude: ok' 'the Join previewed before the body pick' 30
Ui-Mark
Ui-Step 'activate Local bodies'           { Ui-ClickLogged 'Panel Extrude input objects' }
Ui-Step 'toggle the selected block out'   { Ui-ClickLogged 'Browser Root/Bodies/Body1' }
Ui-ExpectNew 'Extrude Local bodies: nothing' 'the real body pick cleared Objects'
Ui-ExpectNew 'Extrude: operation chosen by hand' 'Objects stopped edit automation'
Ui-Mark
Ui-Step 'distance -7 after Objects edit'  { Ui-TypeIn 'Panel Extrude input distance' '-7' }
Ui-ExpectNew 'Extrude Distance: -7 = -7 mm' 'changed geometry after the explicit target pick'
Ui-ExpectNew 'Preview Extrude: ok' 'the fixed Join operation previewed' 30
Ui-WaitIdle
Start-Sleep -Milliseconds 500
Expect-NoNew 'Extrude: automatic Operation' 'Objects keeps the operation fixed for the rest of the edit'
Ui-Step 'cancel Objects edit'             { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'the original stored targets retained' 30
Ui-FocusMain

Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: ' 'the join undone' 30

Write-Host '--- 3. New Body chosen by hand stays when the distance goes in'
Ui-Mark
Ui-Step 'extrude (E)'                    { Ui-Key 'e' }
Ui-ExpectNew "Extrude Profiles: 1 profile [profile $region of F3]" 'the square on the face again'
Ui-ExpectNew 'Extrude: automatic Operation = Join' 'Join at once' 30
Ui-ExpectNew 'Preview Extrude: ok' 'previewed' 30
Ui-Mark
# A wheel step away from the user chooses the entry above: New Body.
Ui-Step 'New Body (wheel)'               { Ui-WheelLogged 'Panel Extrude input operation' 1 }
Ui-ExpectNew 'Extrude: Operation = New Body' 'New Body chosen'
Ui-ExpectNew 'Extrude: operation chosen by hand' 'the automatic choice is off'
Ui-Mark
Ui-Step 'distance -5'                    { Ui-TypeIn 'Panel Extrude input distance' '-5' }
Ui-ExpectNew 'Extrude Distance: -5 = -5 mm' 'into the block'
Ui-ExpectNew 'Preview Extrude: ok' 'previewed' 30
Ui-Step 'distance 5'                     { Ui-TypeIn 'Panel Extrude input distance' '5' }
Ui-ExpectNew 'Extrude Distance: 5 = 5 mm' 'out of the block'
Ui-Step 'distance -5 again'              { Ui-TypeIn 'Panel Extrude input distance' '-5' }
Ui-WaitIdle
Start-Sleep -Milliseconds 500
Expect-NoNew 'Extrude: automatic Operation' 'the hand-picked New Body stays'
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added extrude (New Body), distance -5' 'a new body in the block' 30

Write-Host '--- 4. Editing a deliberate touching New Body keeps its stored operation'
Ui-Mark
Ui-Step 'edit Extrude3 (double-click)'   { Ui-DoubleClickLogged 'Timeline Extrude3' }
Ui-ExpectNew ' with Extrude' 'editing Extrude3' 30
Ui-ExpectNew 'operation=new_body' 'it opens with its New Body'
Ui-ExpectNew 'stored operation differs from automatic choice; editing keeps it' 'the deliberate New Body fails the automatic-policy match' 30
Ui-Mark
Ui-Step 'distance -8'                    { Ui-TypeIn 'Panel Extrude input distance' '-8' }
Ui-ExpectNew 'Preview Extrude: ok' 'the edit previewed' 30
Ui-WaitIdle
Start-Sleep -Milliseconds 500
Expect-NoNew 'Extrude: automatic Operation' 'editing preserves the deliberate New Body'
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F' 'Extrude3 edited' 30
Ui-Mark
Ui-Step 'edit Extrude3 again'            { Ui-DoubleClickLogged 'Timeline Extrude3' }
Ui-ExpectNew ' with Extrude' 'editing Extrude3 again' 30
Ui-ExpectNew 'operation=new_body' 'still New Body'
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'edit cancelled'

if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }
Ui-Finish 'UI Windows auto operation test'
