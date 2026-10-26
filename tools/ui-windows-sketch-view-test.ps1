# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/OcctViewer.cpp app/OcctViewer.hpp app/view app/MainWindow.cpp tools/cli
# Input-driven Windows UI test of how sketch mode shows its geometry in the
# build VM (tools/ui-windows-lib.ps1 says how the input is posted):
#   1. A sketch on XY: its origin is the design's (mitcad#254).
#   2. A rectangle from the origin, its width and height typed: fully
#      constrained.
#   3. The dark background recolours its fully constrained lines light, the
#      light one dark again (mitcad#253), their halos with them
#      (mitcad#302).
#   4. Clicks at known sketch points (no tool): the view reports their
#      sketch coordinates in mm (mitcad#254).
#   5. Outside sketch editing the shown sketch keeps the fully constrained
#      colour, also when the background changes; editing it again changes
#      no colour (mitcad#286).
#   6. A design (mitcad-cli) whose component's sketch, placed by its
#      occurrence, has a fully constrained rectangle and a free line: the
#      rectangle in the constrained colour, the line blue, in the main mode
#      and in its edit alike (mitcad#286), placed by the occurrence in both
#      (mitcad#505).
#   7. A design (mitcad-cli) with a block and a sketch on its top face with
#      three dimensions: each is drawn over a halo in the background's tone,
#      dark on the dark background and light on the light one, read from
#      the log ("Sketch dimension halos:"; mitcad#633; the pixels are
#      checked by tools/ui-sketch-view-test.sh).
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-sketch-view-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-sketch-view'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'sketch.create' = 'F7'; 'view.environment.dark' = 'F8'; 'view.environment.light' = 'F9' }

# Expect-Recoloured "dark|light": the sketch entities shown were recoloured
# for that background, the rectangle's four lines among the fully
# constrained ones.
function Expect-Recoloured([string]$Background) {
  Ui-ExpectNew "Sketch entities recoloured for a $Background background: " "recoloured for the $Background background"
  $count = Ui-LastMatch "Sketch entities recoloured for a $Background background: (\d+) fully constrained"
  if (-not $count -or [int]$count -lt 4) { Ui-Fail "$Background background: $count fully constrained entities recoloured, expected 4 or more" }
  Write-Host "ok   $count fully constrained entities recoloured for the $Background background"
  # Their halos took the background's tone too (mitcad#302).
  $halos = Ui-LastMatch 'Sketch halos: (\d+ (?:dark|light))$'
  if ($halos -notmatch "^(\d+) $Background$" -or [int]$Matches[1] -lt 4) {
    Ui-Fail "$Background background: the last halos are '$halos', expected 4 or more $Background"
  }
  Write-Host "ok   halos for the $Background background: $halos"
}

# Sketch-Colours "F1" (a regex: "F1 in O1", "F1(?: in O\d+)?"): the counts
# of the sketch's entities in the fully constrained colour and in the unconstrained blue, from the
# last "Sketch colours:" line, or $null when it does not name the sketch.
function Sketch-Colours([string]$Sketch) {
  $line = Ui-LogLines | Where-Object { $_.Contains('Sketch colours: ') } | Select-Object -Last 1
  $pattern = '(?:: |; )' + $Sketch + ' (\d+) fully constrained, (\d+) unconstrained(?:;|\s*$)'
  if ($line -and $line -match $pattern) { return @([int]$Matches[1], [int]$Matches[2]) }
  return $null
}

# Expect-Colours "F1" { param($constrained, $unconstrained) ... } "description":
# within six seconds the sketch's counts pass the check; returns them.
function Expect-Colours([string]$Sketch, [scriptblock]$Check, [string]$Description) {
  $counts = $null
  for ($i = 0; $i -lt 30; $i++) {
    $counts = Sketch-Colours $Sketch
    if ($counts -and (& $Check $counts[0] $counts[1])) {
      Write-Host "ok   ${Description}: $($counts[0]) fully constrained, $($counts[1]) unconstrained"
      return , $counts
    }
    Start-Sleep -Milliseconds 200
  }
  $last = Ui-LogLines | Where-Object { $_.Contains('Sketch colours: ') } | Select-Object -Last 1
  Ui-Fail "${Description}: the last colours are '$last'"
}

# Expect-Pointer x y: a click at the sketch point (x, y) mm with no tool
# active; the view reports that place, within two pixels. A click, not a
# move alone: a press always places the pointer (updateCursor), while a
# move posted in session 0 may be followed by a Leave at once.
function Expect-Pointer([double]$X, [double]$Y) {
  $at = Ui-SketchAt $X $Y
  $o = Ui-SketchAt 0 0
  $a = Ui-SketchAt 100 0
  $dx = $a[0] - $o[0]
  $dy = $a[1] - $o[1]
  $tolerance = 2 * 100 / [Math]::Sqrt($dx * $dx + $dy * $dy)
  Ui-Mark
  Ui-Step "click at ($X, $Y)" { Ui-Click $at[0] $at[1] }
  Ui-ExpectNew 'Sketch pointer x ' "the pointer's place at ($X, $Y) logged"
  $line = Ui-LogLines | Where-Object { $_.Contains('Sketch pointer x ') } | Select-Object -Last 1
  if ($line -notmatch 'Sketch pointer x (-?[0-9.]+), y (-?[0-9.]+) mm') { Ui-Fail "the pointer's place is not in mm: '$line'" }
  $px = [double]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
  $py = [double]::Parse($Matches[2], [Globalization.CultureInfo]::InvariantCulture)
  if ([Math]::Abs($px - $X) -gt $tolerance -or [Math]::Abs($py - $Y) -gt $tolerance) {
    Ui-Fail "the view reports ($px, $py) for the pointer at ($X, $Y) (tolerance $tolerance mm)"
  }
  Write-Host "ok   the view reports x $px, y $py for the pointer at ($X, $Y)"
}

Write-Host '--- A sketch on XY, its origin the design origin (mitcad#254)'
Ui-StartApp 'sketch-view'
Ui-Step 'light background (F9)'          { Ui-Key 'F9' }
Ui-Step 'create sketch on XY (F7)'       { Ui-CreateSketch 'F7' 'xy' }
Ui-ExpectLog 'Create Sketch: origin planes shown' 'a new design (its empty Component1 active): the origin planes shown (mitcad#294)'
Ui-ExpectLog 'Sketch view' 'the sketch view is logged'
Ui-ExpectLog 'Sketch origin at 0.00, 0.00, 0.00 mm in the design; the design origin at x 0.00, y 0.00 mm in the sketch' `
  'the sketch origin is the design origin'

Write-Host '--- A fully constrained rectangle on a light background'
Ui-Step 'rectangle tool (R)'             { Ui-Key 'r' }
Ui-Step 'first corner at the origin'     { Ui-SketchClick 0 0 }
Ui-Step 'width 40 (Tab) height 25'       { Ui-Type '40'; Ui-Key 'Tab'; Ui-Type '25' }
Ui-Step 'confirm (Enter)'                { Ui-Key 'Return' }
Ui-ExpectLog 'Added rectangle 40 x 25 mm at (0, 0)' 'rectangle 40 x 25 at the origin'
Ui-ExpectLog 'Sketch Sketch1: 0 DOF, fully constrained' 'fully constrained'
Ui-Step 'end the tool (Esc)'             { Ui-Key 'Escape' }

Write-Host '--- Fully constrained lines on a dark background (mitcad#253)'
Ui-Mark
Ui-Step 'dark background (F8)'           { Ui-Key 'F8' }
Expect-Recoloured 'dark'
Ui-Mark
Ui-Step 'light background (F9)'          { Ui-Key 'F9' }
Expect-Recoloured 'light'

Write-Host "--- The pointer's place in sketch coordinates (mitcad#254)"
Expect-Pointer 30 20
Expect-Pointer -15 -10

Write-Host '--- Outside sketch editing the sketch keeps its colours (mitcad#286)'
# A new design's first sketch goes into a new component, Component1 (O1;
# mitcad#220): outside its edit that occurrence places it.
$sketch1 = 'F1(?: in O\d+)?'
$editing = Expect-Colours $sketch1 { param($c, $u) $c -ge 4 -and $u -eq 0 } 'in its edit the rectangle is fully constrained'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'sketch finished'
Ui-WaitIdle
$shown = Ui-LastMatch 'Sketches shown: (.*)$'
if ($shown -ne 'F1') { Ui-Fail "the finished sketch is not shown: 'Sketches shown: $shown'" }
Write-Host 'ok   the finished sketch is shown'
Expect-Colours $sketch1 { param($c, $u) $c -eq $editing[0] -and $u -eq $editing[1] } `
  'outside its edit the same colours, fully constrained' | Out-Null
Ui-Mark
Ui-Step 'dark background (F8)'           { Ui-Key 'F8' }
Expect-Recoloured 'dark'
Ui-Mark
Ui-Step 'light background (F9)'          { Ui-Key 'F9' }
Expect-Recoloured 'light'
Ui-ExpectLog 'Timeline Sketch1 at' 'the timeline is laid out'
Ui-Mark
Ui-Step 'edit Sketch1 (double-click)'    { Ui-DoubleClickLogged 'Timeline Sketch1' }
Ui-ExpectNew 'Editing sketch F1 (Sketch1)' 'the sketch is edited again' 30
Ui-WaitIdle
Expect-Colours $sketch1 { param($c, $u) $c -eq $editing[0] -and $u -eq $editing[1] } `
  'entering its edit changes no colour' | Out-Null
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'sketch finished again'

Write-Host "--- A component's sketch placed by its occurrence (mitcad#286)"
# mitcad-cli builds it: Part:1 (O1) 100 mm along X, its Sketch1 (F1) a
# dimensioned 30 x 20 rectangle (fully constrained) and a free line (two
# degrees of freedom at each end).
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$script = Join-Path $work 'component.json'
$design = Join-Path $work 'component.mitcad'
Set-Content -Encoding ascii $script @'
[
  {"cmd": "create_component", "name": "Part"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 30], "end": [20, 40]},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "set_occurrence_transform", "occurrence": "O1", "transform": {"translation": [100, 0, 0]}}
]
'@
$made = & $cli run $script --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

Ui-StartApp 'sketch-view-component' @('--open', $design)
Ui-ExpectLog 'component.mitcad' 'the design opened'
Ui-ExpectLog 'Sketches shown: F1' "the component's sketch is shown"
$placed = Expect-Colours 'F1 in O1' { param($c, $u) $c -ge 4 -and $u -ge 1 } `
  'placed by Part:1, the rectangle fully constrained and the line blue'
Ui-Mark
Ui-Step 'dark background (F8)'           { Ui-Key 'F8' }
Expect-Recoloured 'dark'
Ui-Mark
Ui-Step 'light background (F9)'          { Ui-Key 'F9' }
Expect-Recoloured 'light'
Ui-ExpectLog 'Timeline Sketch1 at' 'the timeline is laid out'
Ui-Mark
Ui-Step 'edit Sketch1 (double-click)'    { Ui-DoubleClickLogged 'Timeline Sketch1' }
Ui-ExpectNew 'Editing sketch F1 (Sketch1)' "the component's sketch is edited" 30
Ui-WaitIdle
# Edited where Part:1 places it too (mitcad#505), not at Part's own
# coordinates.
Expect-Colours 'F1 in O1' { param($c, $u) $c -eq $placed[0] -and $u -eq $placed[1] } `
  'in its edit placed by Part:1, the same colours' | Out-Null
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'sketch finished'
Ui-WaitIdle
Expect-Colours 'F1 in O1' { param($c, $u) $c -eq $placed[0] -and $u -eq $placed[1] } `
  'placed by Part:1 again, the same colours' | Out-Null

Write-Host "--- Dimensions over a body's face have a halo in the background's tone (mitcad#633)"
# mitcad-cli builds a 60 x 40 x 30 mm block, a sketch on its top face (F2)
# with a circle (a diameter dimension) and a free line with a driven length
# dimension: three dimensions, each drawn over a halo (the pixels are
# checked by tools/ui-sketch-view-test.sh; this test reads the log).
$faceScript = Join-Path $work 'face.json'
$faceDesign = Join-Path $work 'face.mitcad'
Set-Content -Encoding ascii $faceScript @'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-30, -20],
    "length": 60, "width": 40, "height": 30, "operation": "new_body"}},
  {"cmd": "sketch.create", "plane": {"face": "F1:top", "body": "F1.b0"}},
  {"cmd": "sketch.add_line", "sketch": "F2", "start": [-28, 10], "end": [-4, 10]},
  {"cmd": "sketch.add_dimension", "sketch": "F2", "dimension": {"type": "length", "line": "c1"},
   "driven": true, "text": [12, 16]},
  {"cmd": "sketch.add_circle", "sketch": "F2", "center": [10, 0], "diameter": 16},
  {"cmd": "sketch.add_circle", "sketch": "F2", "center": [50, 0], "diameter": 16}
]
'@
$made = & $cli run $faceScript --save $faceDesign
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $faceDesign)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

Ui-StartApp 'sketch-view-face' @('--open', $faceDesign)
Ui-ExpectLog 'face.mitcad' 'the design opened'
Ui-ExpectLog 'Sketches shown: F2' "the face's sketch is shown"
Ui-ExpectLog 'Timeline Sketch1 at' 'the timeline is laid out'
Ui-Mark
Ui-Step 'edit Sketch1 (double-click)'    { Ui-DoubleClickLogged 'Timeline Sketch1' }
Ui-ExpectNew 'Editing sketch F2 (Sketch1)' "the face's sketch is edited" 30
Ui-WaitIdle

# Expect-DimensionHalos "dark|light": within six seconds the last "Sketch
# dimension halos:" line says the three dimensions have a halo of that tone.
function Expect-DimensionHalos([string]$Tone) {
  $last = $null
  for ($i = 0; $i -lt 30; $i++) {
    $last = Ui-LastMatch 'Sketch dimension halos: (\d+ (?:dark|light))$'
    if ($last -eq "3 $Tone") {
      Write-Host "ok   dimension halos: $last"
      return
    }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "dimension halos: the last line is '$last', expected '3 $Tone'"
}

Ui-Step 'dark background (F8)'           { Ui-Key 'F8' }
Expect-DimensionHalos 'dark'
Ui-Step 'light background (F9)'          { Ui-Key 'F9' }
Expect-DimensionHalos 'light'

Ui-Finish 'UI Windows sketch view test'
