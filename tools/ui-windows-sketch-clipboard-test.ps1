# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/commands/SketchCommands.cpp app/MainWindow.cpp app/framework/ShortcutBindings.hpp tools/cli
# check-all sources: core/sketch core/model-api/src/sketch_clipboard.rs core/model-api/src/sketch.rs core/solver
# Input-driven Windows UI test of Copy, Cut and Paste of sketch geometry
# (mitcad#788) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted), as tools/ui-sketch-clipboard-test.sh on Linux:
#   1. A design (mitcad-cli): Sketch1 on XY with its origin point p1, a line
#      c2 from p3 (10, 10) to p4 (40, 10), horizontal, 30 long and 10 above
#      the origin, and a line c5 parallel to it; Sketch2 on XZ, empty.
#   2. c2 clicked and cut (Ctrl+X): three constraints and dimensions kept,
#      the parallel left out, c2 deleted; its base point is c2's start, held
#      in y by the distance from the origin. Undo and redo name the step
#      "Cut in Sketch1".
#   3. In Sketch2, Paste (Ctrl+V) and a click place it, still 10 above the
#      origin; undo takes it back. Paste and Esc cancels.
#   4. Paste with X 30 typed and Enter (no Y box: y is held): the saved
#      design has the line from (30, 10) to (60, 10) on Sketch2's origin with
#      the three constraints and dimensions, and Sketch1 only c5.
# In session 0 the system clipboard may not take data: the application
# keeps the copy then (app/COMMANDS.md, "Copy, Cut and Paste"), and the
# test passes either way.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-sketch-clipboard-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-sketch-clipboard'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{}
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$script = Join-Path $work 'clipboard.json'
$design = Join-Path $work 'clipboard.mitcad'
Set-Content -Encoding ascii $script @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_point", "sketch": "F1", "at": [0, 0], "fixed": true},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [10, 10], "end": [40, 10]},
  {"cmd": "sketch.add_constraint", "sketch": "F1", "constraint": {"type": "horizontal", "line": "c2"}},
  {"cmd": "sketch.add_dimension", "sketch": "F1", "value": 30,
   "dimension": {"type": "length", "line": "c2"}},
  {"cmd": "sketch.add_dimension", "sketch": "F1", "value": 10,
   "dimension": {"type": "vertical_distance", "a": "p1", "b": "p3"}},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 30], "end": [40, 30]},
  {"cmd": "sketch.add_constraint", "sketch": "F1",
   "constraint": {"type": "parallel", "a": "c2", "b": "c5"}},
  {"cmd": "sketch.create", "plane": "xz"}
]
'@
$made = & $cli run $script --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

# Edit-Sketch "Sketch1" F1: a double-click on its timeline item, then a fit.
# (Not $Name: Ui-Step's own parameter, which its script block would see.)
function Edit-Sketch([string]$Sketch, [string]$Uid) {
  Ui-Mark
  Ui-Step "edit $Sketch (double-click)"    { Ui-DoubleClickLogged "Timeline $Sketch" }
  Ui-ExpectNew "Editing sketch $Uid ($Sketch)" "$Sketch is edited" 30
  Ui-ExpectNew 'Sketch view ' "$Sketch's view is placed"
  Ui-Step 'fit (F6)'                     { Ui-Key 'F6' }
  Ui-Sync
  Start-Sleep -Milliseconds 500
}

Ui-StartApp 'clipboard' @('--open', $design)
Ui-ExpectLog 'Timeline Sketch2 at' 'Sketch1 and Sketch2 in the timeline'

Write-Host '--- Cut c2 from Sketch1'
Edit-Sketch 'Sketch1' 'F1'
Ui-Mark
Ui-Step 'click c2'                       { Ui-SketchClick 33 10 }
Ui-ExpectNew 'Selected: 1 sketch curve' 'c2 selected'
Ui-Mark
Ui-Step 'cut (Ctrl+X)'                   { Ui-Key 'ctrl+x' }
Ui-ExpectNew 'Cut c2 from Sketch1: 3 constraint(s) and dimension(s) kept, 1 left out, base point at (10, 10)' `
  'the cut keeps the horizontal, the length and the distance from the origin'
Ui-ExpectNew 'Deleted c2' 'c2 deleted'
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Cut in Sketch1' 'the cut is one undo step, named Cut'
Ui-Mark
Ui-Step 'redo (Ctrl+Y)'                  { Ui-Key 'ctrl+y' }
Ui-ExpectNew 'Redo: Cut in Sketch1' 'redone'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'Sketch1 finished'

Write-Host '--- Paste into Sketch2 with a click, then undo'
Edit-Sketch 'Sketch2' 'F2'
Ui-Mark
Ui-Step 'paste (Ctrl+V)'                 { Ui-Key 'ctrl+v' }
Ui-ExpectNew 'Paste: geometry of Sketch1 from' "the clipboard (or the application's copy) holds Sketch1's geometry"
if (-not (Ui-LastMatch '(Paste: geometry of Sketch1 from .*, base point at \(10, 10\), held in y)$')) {
  Ui-Fail "not held in y at c2's start"
}
Ui-ExpectNew 'Sketch tool Paste' 'the Paste tool runs'
$hover = Ui-SketchAt 15 15
Ui-Step 'move the pointer'               { [UiInput]::Move($script:UiWindow, $hover[0], $hover[1], 0) }
Ui-Sync
Ui-Mark
# At y 15 (as the pointer): Sketch1's leftover lies edge-on along x in this
# view, so the fit may leave less than 20 mm above the origin.
Ui-Step 'click where it goes'            { Ui-SketchClick 20 15 }
# The pasted ids are the target's own (points first, as the source lists
# them): matched by kind, not assumed.
Ui-ExpectNew ' into Sketch2 at ' 'a click pastes c2 and its points'
$placed = Ui-LastMatch '(Pasted p\d+, p\d+, c\d+ into Sketch2 at \([-0-9.]+, 10\):.*)$'
if (-not $placed) { Ui-Fail 'not two points and a line pasted 10 above the origin (the click moved it in y?)' }
Write-Host "ok   still 10 above the origin: $placed"
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Paste into Sketch2' 'the paste is one undo step'

Write-Host '--- Esc cancels a paste'
Ui-Mark
Ui-Step 'paste (Ctrl+V)'                 { Ui-Key 'ctrl+v' }
Ui-ExpectNew 'Sketch tool Paste' 'the Paste tool runs'
Ui-Step 'Esc'                            { Ui-Key 'Escape' }
Ui-ExpectNew 'Paste cancelled' 'Esc cancels it'
if ((Ui-LogLines | Select-Object -Skip $script:UiMark) | Where-Object { $_.Contains('Pasted ') }) {
  Ui-Fail 'Esc pasted'
}
Write-Host 'ok   nothing pasted'

Write-Host '--- Paste at a typed X'
Ui-Mark
Ui-Step 'paste (Ctrl+V)'                 { Ui-Key 'ctrl+v' }
Ui-ExpectNew 'Sketch tool Paste' 'the Paste tool runs'
Ui-Sync
Ui-Step 'type X 30'                      { Ui-Key 'ctrl+a'; Ui-Type '30' }
Ui-ExpectNew 'Sketch value x: 30' 'X typed'
Ui-Step 'Enter'                          { Ui-Key 'Return' }
Ui-ExpectNew ' into Sketch2 at (30, 10): 3 constraint(s) and dimension(s) kept, 1 left out' `
  'pasted at the typed place'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'Sketch2 finished'
Ui-Mark
Ui-Step 'save (Ctrl+S)'                  { Ui-Key 'ctrl+s' }
Ui-ExpectNew 'Saved ' 'saved'

$check = Join-Path $work 'check.json'
Set-Content -Encoding ascii $check @'
[
  {"expect": {"query": {"query": "sketch", "uid": "F1"}, "result": {
    "entities": [{"id": "p1"}, {"type": "point", "at": [0, 30]}, {"type": "point", "at": [40, 30]},
                 {"id": "c5", "type": "line"}],
    "constraints": [], "dimensions": []}}},
  {"expect": {"query": {"query": "sketch", "uid": "F2"}, "result": {
    "entities": [{"id": "p1", "fixed": true, "at": [0, 0]},
                 {"type": "point", "at": [30, 10]}, {"type": "point", "at": [60, 10]},
                 {"type": "line"}],
    "constraints": [{"type": "horizontal"}],
    "dimensions": [{"type": "length", "measured": 30},
                   {"type": "vertical_distance", "a": "p1", "measured": 10}]}}}
]
'@
$checked = & $cli run $check --open $design
if ($LASTEXITCODE -ne 0) { Ui-Fail "the saved sketches: $($checked -join ' / ')" }
Write-Host 'ok   Sketch1 keeps c5 alone; Sketch2 has the line with its constraints and dimensions'

Ui-Finish 'Windows UI sketch clipboard test'
