# SPDX-License-Identifier: MIT
# check-all sources: app/sketch/EditTools.cpp app/sketch/SketchOverlay.cpp
# check-all sources: core/model/src/sketch core/solver tools/cli
# Input-driven Windows UI test of the gap between two concentric circles
# (mitcad#445) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted):
#   1. A design (mitcad-cli): Sketch1 with a circle of diameter 10 about a
#      fixed centre at the origin and a free circle of radius 25 about the
#      same centre (1 DOF: its radius).
#   2. Sketch Dimension: the inner circle, then the outer one, the value
#      placed between the rims: a concentric_distance offered as 20.00;
#      12 typed makes it driving and the sketch fully constrained.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-concentric-dimension-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-concentric-dimension'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{}
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$script = Join-Path $work 'circles.json'
$design = Join-Path $work 'circles.mitcad'
Set-Content -Encoding ascii $script @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 10},
  {"cmd": "sketch.circle", "sketch": "F1", "mode": "center", "center": "p2", "radius": 25}
]
'@
$made = & $cli run $script --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

Ui-StartApp 'concentric' @('--open', $design)
Ui-ExpectLog 'circles.mitcad' 'the design opened'
Ui-Mark
Ui-Step 'edit Sketch1 (double-click)'    { Ui-DoubleClickLogged 'Timeline Sketch1' }
Ui-ExpectNew 'Editing sketch F1 (Sketch1)' 'the sketch is edited' 30
Ui-ExpectNew 'Sketch Sketch1: 1 DOF' 'the outer radius is free'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Start-Sleep -Milliseconds 800

Write-Host '--- The gap between the two circles'
Ui-Mark
Ui-Step 'sketch dimension (D)'           { Ui-Key 'd' }
Ui-Step 'pick the inner circle'          { Ui-SketchClick -5 0 }
Ui-Step 'pick the outer circle'          { Ui-SketchClick -25 0 }
Ui-Step 'place the value between them'   { Ui-SketchClick 10 10 }
Ui-ExpectNew 'Sketch value editor at' 'the value editor opened'
Ui-ExpectNew ' with 20.00' 'the gap offered'
Ui-Step 'type 12, Enter'                 { Ui-Key 'ctrl+a'; Ui-Type '12'; Start-Sleep -Milliseconds 200; Ui-Key 'Return' }
Ui-ExpectNew 'Added dimension k' 'a driving dimension'
Ui-ExpectNew 'concentric_distance' 'the gap between the circles'
Ui-ExpectNew 'Sketch Sketch1: 0 DOF' 'the outer radius follows the gap'
Ui-Step 'end the tool (Esc)'             { Ui-Key 'Escape' }
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'sketch finished'

Ui-Finish 'UI Windows concentric dimension test'
