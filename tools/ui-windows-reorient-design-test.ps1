# SPDX-License-Identifier: MIT
# check-all sources: app/browser/ReorientDialog.cpp app/browser/BrowserController.cpp app/MainWindow.cpp tools/cli
# check-all sources: core/model/src/reorient.rs core/model/src/document/reorient.rs core/model/src/api/reorient.rs
# Input-driven Windows UI test of Reorient Design (mitcad#502) in the build
# VM (tools/ui-windows-lib.ps1 says how the input is posted), on a design
# mitcad-cli makes, as tools/ui-reorient-design-test.sh does on Linux:
# Block, 60 x 40 x 20 from a sketch on XY with a hole through its top. The
# root's context menu cannot open in session 0, so the command it runs,
# Reorient Design, comes from a shortcut of the test's.
#   1. Reorient Design (F8) previews 90 degrees about X at once; Z (Alt+Z)
#      previews about Z, X (Alt+X) about X again; OK keeps it: the block's
#      box is turned (y -20..0, z 0..40).
#   2. Undo takes it back as one step, Reorient Design.
#   3. Opened again, 180 degrees (Alt+1) previews; Cancel (Esc) takes it
#      back with the history: Redo still redoes the first turn.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-reorient-design-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-reorient-design'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'assembly.reorient_design' = 'F8' }
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$region = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

$modelScript = Join-Path $work 'model.json'
$design = Join-Path $work 'reorient.mitcad'
Set-Content -Encoding ascii $modelScript (@'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "hole",
    "placement": {"type": "face", "body": "F2.b0", "face": "F2:end(RECT)", "points": [[15, 20, 20]]},
    "diameter": 8, "extent": {"type": "through_all"}}},
  {"cmd": "rename_body", "uid": "F2.b0", "name": "Block"}
]
'@ -creplace 'RECT', $region)
$made = & $cli run $modelScript --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

Ui-StartApp 'reorient-design' @('--open', $design)
Ui-ExpectLog 'Browser Root at' 'the browser shows the root'

Write-Host '--- 1. Previewed, then kept'
Ui-Mark
Ui-Step 'Reorient Design (F8)'           { Ui-Key 'F8' }
Ui-ExpectNew 'Reorient Design dialog opened' 'the dialog opened'
Ui-ExpectNew 'Reorient Design preview: 90 degrees about X: ' '90 about X previewed at once' 30
Ui-FocusDialog '^Reorient Design$'
Ui-Mark
Ui-Step 'about Z (Alt+Z)'                { Ui-Key 'alt+z' }
Ui-ExpectNew 'Reorient Design preview: 90 degrees about Z: ' 'about Z previewed' 30
Ui-Mark
Ui-Step 'about X (Alt+X)'                { Ui-Key 'alt+x' }
Ui-ExpectNew 'Reorient Design preview: 90 degrees about X: ' 'about X again' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-FocusMain
Ui-ExpectNew 'Reorient Design: 90 degrees about X' 'the turn kept'
Ui-ExpectNew 'Reorient Design body Block: box (0, -20, 0) - (60, 0, 40)' 'the block''s box turned about X'

Write-Host '--- 2. Undo: one step'
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Reorient Design' 'one undo step named Reorient Design' 30

Write-Host '--- 3. Cancel takes the preview back'
Ui-Mark
Ui-Step 'Reorient Design (F8)'           { Ui-Key 'F8' }
Ui-ExpectNew 'Reorient Design preview: 90 degrees about X: ' 'previewed again' 30
Ui-FocusDialog '^Reorient Design$'
Ui-Mark
Ui-Step '180 degrees (Alt+1)'            { Ui-Key 'alt+1' }
Ui-ExpectNew 'Reorient Design preview: 180 degrees about X: ' '180 about X previewed' 30
Ui-Mark
Ui-Step 'Cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-FocusMain
Ui-ExpectNew 'Reorient Design cancelled' 'cancelled' 30
Ui-Mark
Ui-Step 'redo (Ctrl+Y)'                  { Ui-Key 'ctrl+y' }
Ui-ExpectNew 'Redo: Reorient Design' 'Cancel left the history as it was' 30

Ui-Finish 'UI Windows reorient design test'
