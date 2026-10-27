# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/framework/ModelShapes.cpp
# check-all sources: core/sketch core/model-api/src/sketch.rs core/solver tools/cli
# Input-driven Windows UI test of the sketch's axes and origin as picks of
# Sketch Dimension and the constraint tools (mitcad#500) in the build VM
# (tools/ui-windows-lib.ps1 says how the input is posted), as
# tools/ui-sketch-axes-test.sh on Linux:
#   1. A design (mitcad-cli): Sketch1 with a free point p1 at (20, 12), a
#      free line c2 from (40, 5) to (70, 20) and a free point p5 at
#      (-8, 15); nothing at the origin (8 DOF).
#   2. Sketch Dimension: p1, then the x axis: 12.00 offered, 30 typed: the
#      x axis made for the pick, p1 at 30 mm from it (7 DOF).
#   3. Sketch Dimension: the line, then the x axis, the value between
#      them: 26.57 offered, 25 typed: the line at 25 degrees (6 DOF).
#   4. Coincident (Shift+F5 in the test's settings): p5, then the y axis:
#      the y axis made, p5 on it (5 DOF); undo takes both back, redo makes
#      them again.
#   5. The saved design: the axes' lines, the values, p5 at x = 0.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-sketch-axes-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-sketch-axes'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

# Coincident has no default shortcut (the command search's popup closes at
# once in session 0).
Ui-Init $App $Out $QtBin @{ 'sketch.constraint.coincident' = 'Shift+F5' }
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$script = Join-Path $work 'axes.json'
$design = Join-Path $work 'axes.mitcad'
Set-Content -Encoding ascii $script @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [20, 12]},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [40, 5], "end": [70, 20]},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [-8, 15]}
]
'@
$made = & $cli run $script --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

# Ui-ZoomOut steps: wheel steps towards the user at the view's middle, then
# a moment for the new "Sketch view" line.
function Ui-ZoomOut([int]$Steps) {
  $at = Ui-ViewAt 50 50
  for ($i = 0; $i -lt $Steps; $i++) {
    [UiInput]::Move($script:UiWindow, $at[0], $at[1], 0)
    Start-Sleep -Milliseconds 80
    [UiInput]::Wheel($script:UiWindow, $at[0], $at[1], -1)
    Start-Sleep -Milliseconds 300
  }
  Start-Sleep -Milliseconds 800
}

Ui-StartApp 'axes' @('--open', $design)
Ui-ExpectLog 'axes.mitcad' 'the design opened'
Ui-Mark
Ui-Step 'edit Sketch1 (double-click)'    { Ui-DoubleClickLogged 'Timeline Sketch1' }
Ui-ExpectNew 'Editing sketch F1 (Sketch1)' 'the sketch is edited' 30
Ui-ExpectNew 'Sketch Sketch1: 8 DOF' 'two free points and a free line'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Start-Sleep -Milliseconds 800
Ui-Step 'zoom out to show the axes'      { Ui-ZoomOut 2 }

Write-Host '--- 1. A point 30 mm from the x axis'
Ui-Mark
Ui-Step 'sketch dimension (D)'           { Ui-Key 'd' }
Ui-ExpectNew 'Sketch tool Sketch Dimension' 'Sketch Dimension active'
Ui-Step 'pick p1'                        { Ui-SketchClick 20 12 }
Ui-Step 'pick the x axis'                { Ui-SketchClick 8 0 }
Ui-Step 'place the value'                { Ui-SketchClick 26 6 }
Ui-ExpectNew 'Sketch value editor at' 'the value editor opened'
Ui-ExpectNew ' with 12.00' "p1's distance from the x axis offered"
Ui-Step 'type 30, Enter'                 { Ui-Key 'ctrl+a'; Ui-Type '30'; Start-Sleep -Milliseconds 200; Ui-Key 'Return' }
Ui-ExpectNew 'Made the sketch x axis for a pick: c7' 'the x axis made for the pick'
Ui-ExpectNew 'Added dimension k' 'a driving dimension'
Ui-ExpectNew 'point_line_distance' "the point's distance from the axis"
Ui-ExpectNew 'Sketch Sketch1: 7 DOF' 'p1 held at its distance'

Write-Host '--- 2. A line at 25 degrees to the x axis'
Ui-Mark
Ui-Step 'pick the line'                  { Ui-SketchClick 55 12.5 }
Ui-Step 'pick the x axis'                { Ui-SketchClick 8 0 }
Ui-Step 'place the value between them'   { Ui-SketchClick 45 3 }
Ui-ExpectNew 'Sketch value editor at' 'the value editor opened'
Ui-ExpectNew ' with 26.57' 'the angle between them offered'
Ui-Step 'type 25, Enter'                 { Ui-Key 'ctrl+a'; Ui-Type '25'; Start-Sleep -Milliseconds 200; Ui-Key 'Return' }
Ui-ExpectNew 'Added dimension k' 'a driving dimension'
Ui-ExpectNew ': 25.00 deg, the arc from 25.0 to 0.0 deg' 'the line turned to 25 degrees'
Ui-ExpectNew 'Sketch Sketch1: 6 DOF' "the line's direction held"
if (@(Ui-LogLines | Where-Object { $_.Contains('Made the sketch x axis') }).Count -ne 1) {
  Ui-Fail 'the x axis was made twice'
}
Ui-Step 'end the tool (Esc)'             { Ui-Key 'Escape' }

Write-Host '--- 3. A point on the y axis'
Ui-Mark
Ui-Step 'coincident (Shift+F5)'          { Ui-Key 'shift+F5' }
Ui-ExpectNew 'Sketch tool Coincident' 'Coincident active'
Ui-Step 'pick p5'                        { Ui-SketchClick -8 15 }
Ui-Step 'pick the y axis'                { Ui-SketchClick 0 25 }
Ui-ExpectNew 'Made the sketch y axis for a pick: c9' 'the y axis made for the pick'
Ui-ExpectNew 'Added constraint coincident k' 'the point on the axis'
Ui-ExpectNew 'on p5, c9' "p5 and the y axis's line"
Ui-ExpectNew 'Sketch Sketch1: 5 DOF' 'p5 on the y axis'
Ui-Step 'end the tool (Esc)'             { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Sketch Sketch1: 6 DOF' 'the constraint and the y axis taken back as one step'
Ui-Mark
Ui-Step 'redo (Ctrl+Y)'                  { Ui-Key 'ctrl+y' }
Ui-ExpectNew 'Sketch Sketch1: 5 DOF' 'both again'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'sketch finished'
Ui-Mark
Ui-Step 'save (Ctrl+S)'                  { Ui-Key 'ctrl+s' }
Ui-ExpectNew 'Saved ' 'saved'

Write-Host '--- 4. The saved design'
# The entities in the order they were made: p1, the line's points and the
# line, p5, then the origin point, the x axis's far end and line, the y
# axis's far end and line.
$check = Join-Path $work 'check.json'
Set-Content -Encoding ascii $check @'
[
  {"expect": {"sketch": "Sketch1", "dof": 5}},
  {"expect": {"query": {"query": "sketch", "uid": "F1"}, "result": {
    "entities": [
      {"id": "p1", "at": [20, 30]},
      {"id": "p3"}, {"id": "p4"}, {"id": "c2"},
      {"id": "p5", "at": [0, 15]},
      {"id": "p6", "at": [0, 0], "fixed": true},
      {"id": "p8", "axis": "x", "fixed": true},
      {"id": "c7", "type": "line", "start": "p6", "end": "p8", "axis": "x", "construction": true, "fixed": true},
      {"id": "p10", "axis": "y", "fixed": true},
      {"id": "c9", "type": "line", "start": "p6", "end": "p10", "axis": "y", "construction": true, "fixed": true}],
    "constraints": [{"type": "coincident", "point": "p5", "entity": "c9"}],
    "dimensions": [{"type": "point_line_distance", "point": "p1", "line": "c7", "measured": 30},
                   {"type": "angle", "a": "c2", "b": "c7", "measured": 0.4363323129985824}],
    "regions": []}}}
]
'@
$checked = & $cli run $check --open $design
if ($LASTEXITCODE -ne 0) { Ui-Fail "the saved axes, dimensions and constraint: $($checked -join ' / ')" }
Write-Host 'ok   the axes, dimensions and constraint saved'

Ui-Finish 'UI Windows sketch axes test'
