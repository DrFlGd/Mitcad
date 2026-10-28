# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindow.cpp app/MainWindowVisibility.cpp app/MainWindowParameterPicks.cpp
# check-all sources: app/view/CommandSketches.hpp app/OcctViewer.cpp app/framework/CommandSession.cpp
# check-all sources: app/commands/ExtrudeCommands.cpp app/browser tools/cli
# Input-driven Windows UI test of the sketches a command shows for its time
# (mitcad#546) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted); the Linux test is tools/ui-edit-sketches-test.sh:
#   1. A design (mitcad-cli): Sketch1 (F1) with two 20 x 20 squares, the
#      first extruded by Extrude1 (F2), so Sketch1 is hidden as a used
#      sketch is; Sketch2 (F3) with one square extruded by Extrude2 (F4),
#      its light bulb turned on.
#   2. Edit Extrude1: Sketch1 shown for the edit, its square can be picked
#      (taken away and picked again), 15 mm, OK: Sketch1 hidden again.
#   3. Edit Extrude1, Esc: shown, then hidden again.
#   4. Edit Extrude2: Sketch2, shown by its light bulb, is not shown "for
#      the edit" and stays shown after OK.
#   5. The saved design: Sketch1's light bulb not set, Sketch2's on, Body1
#      15 mm high: nothing of the edits' showing went to the model.
#   6. A new Extrude takes Sketch1's free square: Sketch1 shown for it, Esc
#      hides it again.
#   7. Edit Extrude1 with Sketch1's light bulb turned on meanwhile, Esc:
#      Sketch1 stays shown.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-edit-sketches-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-edit-sketches'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{} -StepPause 0

# Ui-NotSince "text" "description": no line since the mark has the text.
function Ui-NotSince([string]$Text, [string]$Description) {
  Ui-Sync
  Start-Sleep -Milliseconds 500
  if ((Ui-LogLines | Select-Object -Skip $script:UiMark) | Where-Object { $_.Contains($Text) }) {
    Ui-Fail "${Description}: '$Text' is in the log"
  }
  Write-Host "ok   $Description"
}

# Expect-Shown "F3" "description": the sketches the view shows, as last
# logged.
function Expect-Shown([string]$Sketches, [string]$Description) {
  Ui-Sync
  $shown = Ui-LastMatch 'Sketches shown: (.*)$'
  if ($shown -ne $Sketches) { Ui-Fail "${Description}: the view shows '$shown'" }
  Write-Host "ok   $Description"
}

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$rect = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
$design = Join-Path $work 'sketches.mitcad'
$modelJson = Join-Path $work 'model.json'
Set-Content -Encoding ascii $modelJson (@'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 20, "height": 20},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 40], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [40, 0], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "set_feature_visible", "uid": "F3", "visible": true}
]
'@).Replace('RECT', $rect)
$output = & $cli run $modelJson --save $design 2>&1
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }

Ui-StartApp 'edit-sketches' @('--open', $design)
Ui-ExpectLog 'sketches.mitcad' 'the design opened'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F2.b0, F4.b0' 'both blocks shown'
Ui-ExpectLog 'Timeline Extrude2 at' 'the timeline is logged'
Expect-Shown 'F3' 'Sketch2 shown by its light bulb, Sketch1 hidden as it is used'

Write-Host '--- Edit Extrude1: Sketch1 shown for the edit, its square picked; OK'
Ui-Mark
Ui-Step 'edit Extrude1 (double-click)'   { Ui-DoubleClickLogged 'Timeline Extrude1' }
Ui-ExpectNew 'Editing F2 with Extrude' 'Extrude1 opens for editing' 30
Ui-ExpectNew 'Sketch Sketch1 shown for the edit of Extrude1' 'Sketch1 shown for the edit'
Ui-ExpectNew 'Sketches shown: F1' 'the view shows Sketch1 (Sketch2 is after the marker)'
Ui-ExpectNew "Pick profile F1/$rect at" 'Sketch1''s square can be picked'
Ui-Mark
Ui-Step 'take the square away'           { Ui-ClickPick "profile F1/$rect" }
Ui-ExpectNew 'Extrude Profiles: nothing' 'the square taken away'
Ui-Mark
Ui-Step 'pick the square again'          { Ui-ClickPick "profile F1/$rect" }
Ui-ExpectNew "Extrude Profiles: 1 profile [profile $rect of F1]" 'the square picked again'
Ui-Mark
Ui-Step 'distance 15'                    { Ui-TypeIn 'Panel Extrude input distance' '15' }
Ui-ExpectNew 'Preview Extrude: ok' 'previewed' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F2' 'Extrude1 edited' 30
Ui-ExpectNew 'Sketch Sketch1 hidden again after the edit of Extrude1' 'Sketch1 hidden again'
Expect-Shown 'F3' 'the view shows Sketch2 only again'

Write-Host '--- Edit Extrude1, Esc'
Ui-Mark
Ui-Step 'edit Extrude1 (double-click)'   { Ui-DoubleClickLogged 'Timeline Extrude1' }
Ui-ExpectNew 'Editing F2 with Extrude' 'Extrude1 opens for editing' 30
Ui-ExpectNew 'Sketch Sketch1 shown for the edit of Extrude1' 'Sketch1 shown for the edit'
Ui-ExpectNew 'Sketches shown: F1' 'the view shows Sketch1'
Ui-Mark
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'the edit cancelled'
Ui-ExpectNew 'Sketch Sketch1 hidden again after the edit of Extrude1' 'Sketch1 hidden again'
Expect-Shown 'F3' 'the view shows Sketch2 only again'

Write-Host '--- Edit Extrude2: Sketch2 is shown by its light bulb'
Ui-Mark
Ui-Step 'edit Extrude2 (double-click)'   { Ui-DoubleClickLogged 'Timeline Extrude2' }
Ui-ExpectNew 'Editing F4 with Extrude' 'Extrude2 opens for editing' 30
Ui-ExpectNew "Pick profile F3/$rect at" 'Sketch2''s square can be picked'
Ui-NotSince 'shown for the edit' 'no sketch shown only for the edit'
Expect-Shown 'F3' 'the view shows Sketch2, not Sketch1'
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F4' 'Extrude2 edited' 30
Ui-NotSince 'hidden again' 'nothing hidden again'
Expect-Shown 'F3' 'Sketch2 stays shown'

Write-Host '--- The saved design: no visibility change'
Ui-Mark
Ui-Step 'save (Ctrl+S)'                  { Ui-Key 'ctrl+s' }
Ui-ExpectNew 'Saved ' 'saved'
$check = Join-Path $work 'check.json'
Set-Content -Encoding ascii $check @'
[
  {"expect": {"query": {"query": "timeline"}, "result": {"features": [
    {"uid": "F1", "visible": false, "visible_set": false}, {"uid": "F2"},
    {"uid": "F3", "visible": true, "visible_set": true}, {"uid": "F4"}]}}},
  {"expect": {"body": "Body1", "volume": 6000, "tolerance": 1e-6}}
]
'@
$checked = & $cli run $check --open $design 2>&1
if ($LASTEXITCODE -ne 0) { Ui-Fail "the saved design: $($checked -join ' / ')" }
Write-Host 'ok   Sketch1''s light bulb not set, Sketch2''s on, Extrude1 15 mm'

Write-Host '--- A new Extrude takes Sketch1''s free square'
Ui-Mark
Ui-Step 'extrude (E)'                    { Ui-Key 'e' }
Ui-ExpectNew 'Command Extrude started' 'Extrude started'
Ui-ExpectNew 'Extrude Profiles: 1 profile [profile ' 'it takes a profile'
Ui-ExpectNew 'Sketch Sketch1 shown for Extrude' 'Sketch1 shown for the command'
Ui-ExpectNew 'Sketches shown: F1, F3' 'the view shows both sketches'
Ui-Mark
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'Extrude cancelled'
Ui-ExpectNew 'Sketch Sketch1 hidden again after Extrude' 'Sketch1 hidden again'
Expect-Shown 'F3' 'the view shows Sketch2 only again'

Write-Host '--- Sketch1''s light bulb turned on during the edit'
Ui-Mark
Ui-Step 'edit Extrude1 (double-click)'   { Ui-DoubleClickLogged 'Timeline Extrude1' }
Ui-ExpectNew 'Editing F2 with Extrude' 'Extrude1 opens for editing' 30
Ui-ExpectNew 'Sketch Sketch1 shown for the edit of Extrude1' 'Sketch1 shown for the edit'
Ui-ExpectLog 'Browser eye Root/Sketches/Sketch1 at' 'Sketch1''s light bulb in the browser'
Ui-Mark
Ui-Step 'show Sketch1 (light bulb)'      { Ui-ClickLogged 'Browser eye Root/Sketches/Sketch1' }
Ui-ExpectNew 'Visibility Root/Sketches/Sketch1: shown (during Extrude)' 'Sketch1''s light bulb on during the edit'
Ui-Mark
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude: 1 visibility change(s) kept' 'the light bulb kept'
Ui-ExpectNew 'Command Extrude cancelled' 'the edit cancelled'
Ui-ExpectNew 'Sketch Sketch1 stays shown after the edit of Extrude1 (its light bulb)' 'Sketch1 stays shown'
Expect-Shown 'F1, F3' 'the view shows both sketches'
if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }

Ui-Finish 'UI Windows edit sketches test'
