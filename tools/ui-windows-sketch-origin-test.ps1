# SPDX-License-Identifier: MIT
# check-all sources: app/sketch app/MainWindow.cpp app/MainWindowVisibility.cpp app/browser app/projects tools/cli
# Input-driven Windows UI test of the sketch's own origin and axes in a new
# design, whose Origin folder's light bulb is off (mitcad#739), in the
# build VM (tools/ui-windows-lib.ps1 says how the input is posted); the
# Linux test is tools/ui-sketch-origin-test.sh:
#   1. A new design starts with the design's origin hidden in the view and
#      the Origin folder's light bulb off.
#   2. Create Sketch on XY, XZ and YZ: the origin planes shown for the pick
#      only; in the sketch the design's origin is hidden again, while the
#      sketch draws its origin in the view and its x and y axes across it
#      ("Sketch axes drawn:"), where the sketch view puts the sketch's
#      (0, 0) and its x and y directions ("Sketch view").
#   3. The Line tool clicked a few pixels beside the origin starts the line
#      at the origin exactly (the snap). The XZ and YZ sketches get a line
#      away from the origin instead, so that they are kept (an empty new
#      sketch is taken back when it is finished).
#   4. After each sketch the Origin folder's light bulb is still off and
#      was never switched ("Visibility Root/Origin" is never logged).
#   5. Editing an existing sketch (the XZ one) draws its origin and axes
#      too, and its origin is snapped to.
#   6. New Design in a project (mitcad#844; the project made with
#      mitcad-cli): the new design opens with its Component1 active, Create
#      Sketch shows the origin planes, XY clicked in the view starts the
#      sketch in Component1, and the light bulb stays off.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-sketch-origin-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-sketch-origin'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'sketch.create' = 'F7' } -StepPause 0

$hidden = "Design origin hidden in the view; the Origin folder's light bulb off"
$shown = "Design origin shown in the view; the Origin folder's light bulb off"

# Expect-OriginOff "description": within six seconds the last "Design
# origin hidden|shown in the view; ..." line says the design's origin is
# hidden with the light bulb off, and the Origin folder's light bulb was
# never switched. The match is case-sensitive and names the visibility line
# only: Ui-LastMatch (Select-String) ignores case, and a sketch edit's start
# logs "Sketch origin at ...; the design origin at x ... in the sketch",
# which came after the visibility line on Windows.
function Expect-OriginOff([string]$Description) {
  $last = $null
  for ($i = 0; $i -lt 30; $i++) {
    $line = Ui-LogLines | Where-Object { $_ -cmatch 'Design origin (hidden|shown) in the view; ' } |
      Select-Object -Last 1
    $last = if ("$line" -cmatch '(Design origin (hidden|shown) in the view; .*)$') { $Matches[1] } else { $null }
    if ($last -eq $hidden) { break }
    Start-Sleep -Milliseconds 200
  }
  if ($last -ne $hidden) { Ui-Fail "${Description}: the last origin line is '$last', expected '$hidden'" }
  $switched = Ui-LogLines | Where-Object { $_.Contains('Visibility Root/Origin') } | Select-Object -Last 1
  if ($switched) { Ui-Fail "${Description}: the Origin folder's light bulb was switched: '$switched'" }
  Write-Host "ok   ${Description}: the design's origin hidden, the Origin folder's light bulb off"
}

# Along ax ay bx by dx dy ox oy: the line a-b runs in the direction d and
# passes within two pixels of the point o.
function Along([double]$Ax, [double]$Ay, [double]$Bx, [double]$By, [double]$Dx, [double]$Dy,
               [double]$Ox, [double]$Oy) {
  $lx = $Bx - $Ax
  $ly = $By - $Ay
  $length = [Math]::Sqrt($lx * $lx + $ly * $ly)
  $d = [Math]::Sqrt($Dx * $Dx + $Dy * $Dy)
  if ($length -lt 50 -or $d -lt 1 -or ($lx * $Dx + $ly * $Dy) / ($length * $d) -lt 0.999) { return $false }
  $off = (($Ox - $Ax) * $ly - ($Oy - $Ay) * $lx) / $length
  return [Math]::Abs($off) -le 2
}

# Axes-Agree "axes" "view": the logged origin is in the view where the
# sketch view puts the sketch's (0, 0) (within two pixels), and the x and y
# axes are drawn across the view through it, each along the view's
# direction of the sketch's x or y.
function Axes-Agree([string]$Axes, [string]$View) {
  $pattern = '^origin at (-?\d+),(-?\d+) in the view; x axis (-?\d+),(-?\d+) to (-?\d+),(-?\d+); ' +
             'y axis (-?\d+),(-?\d+) to (-?\d+),(-?\d+)$'
  if ($Axes -notmatch $pattern) { return $false }
  $a = @(1..10 | ForEach-Object { [double]$Matches[$_] })
  $v = @($View -split '[ ,]' | ForEach-Object { [double]::Parse($_, [Globalization.CultureInfo]::InvariantCulture) })
  if ($v.Count -ne 6) { return $false }
  if (($a[0] - $v[0]) * ($a[0] - $v[0]) + ($a[1] - $v[1]) * ($a[1] - $v[1]) -gt 4) { return $false }
  if (-not (Along $a[2] $a[3] $a[4] $a[5] ($v[2] - $v[0]) ($v[3] - $v[1]) $a[0] $a[1])) { return $false }
  return Along $a[6] $a[7] $a[8] $a[9] ($v[4] - $v[0]) ($v[5] - $v[1]) $a[0] $a[1]
}

# Expect-Axes "description": within six seconds the last "Sketch axes
# drawn:" line since the mark agrees with the last "Sketch view" line.
function Expect-Axes([string]$Description) {
  $axes = $null
  $view = $null
  for ($i = 0; $i -lt 30; $i++) {
    Ui-Sync
    $line = Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.Contains('Sketch axes drawn: ') } |
      Select-Object -Last 1
    $axes = if ($line -and $line -match 'Sketch axes drawn: (.*)$') { $Matches[1] } else { $null }
    $view = Ui-LastMatch 'Sketch view ([-0-9.,]+ [-0-9.,]+ [-0-9.,]+)'
    if ($axes -and $view -and (Axes-Agree $axes $view)) {
      Write-Host "ok   ${Description}: $axes"
      return
    }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "${Description}: the sketch's origin and axes are not drawn where the view puts them: '$axes' ($view)"
}

# Line-FromOrigin x y "description": the Line tool from a click four pixels
# right of and three above the sketch's origin to the sketch point (x, y):
# the line starts at the origin exactly.
function Line-FromOrigin([double]$X, [double]$Y, [string]$Description) {
  Ui-Mark
  Ui-Step 'line tool (L)'                  { Ui-Key 'l' }
  Ui-ExpectNew 'Sketch tool Line' 'line tool active'
  $o = Ui-SketchAt 0 0
  Ui-Step 'click beside the origin'        { Ui-Click ($o[0] + 4) ($o[1] - 3) }
  Ui-Step "to ($X, $Y)"                    { Ui-SketchClick $X $Y }
  Ui-ExpectNew 'from (0, 0) to' "${Description}: the line starts at the origin (snapped)"
  Ui-Step 'end the tool (Esc)'             { Ui-Key 'Escape' }
}

# Free-Line "description": a line from the sketch point (20, 10) to
# (40, 30), away from the origin and the axes, so that the sketch is kept
# when it is finished (an empty new sketch is taken back,
# MainWindow::finishSketch) and its origin stays unmade for a later snap.
function Free-Line([string]$Description) {
  Ui-Mark
  Ui-Step 'line tool (L)'                  { Ui-Key 'l' }
  Ui-ExpectNew 'Sketch tool Line' 'line tool active'
  Ui-Step 'from (20, 10)'                  { Ui-SketchClick 20 10 }
  Ui-Step 'to (40, 30)'                    { Ui-SketchClick 40 30 }
  Ui-ExpectNew 'Added line ' "${Description}: a line away from the origin"
  Ui-Step 'end the tool (Esc)'             { Ui-Key 'Escape' }
}

Write-Host "--- A new design: the origin hidden, the Origin folder's light bulb off"
Ui-StartApp 'sketch-origin'
Ui-ExpectLog $hidden 'a new design starts with its origin hidden'

foreach ($plane in @('xy', 'xz', 'yz')) {
  $name = $plane.ToUpperInvariant()
  Write-Host "--- Create Sketch on ${name}: the sketch's origin and axes drawn (mitcad#739)"
  Ui-Mark
  Ui-Step "create sketch on $name (F7)"    { Ui-CreateSketch 'F7' $plane }
  Ui-ExpectNew $shown "${name}: the origin planes shown for the pick"
  Ui-ExpectNew 'Sketch view' "${name}: the sketch view is logged"
  Expect-Axes $name
  Expect-OriginOff "$name in the sketch"
  if ($plane -eq 'xy') { Line-FromOrigin 30 20 'XY' } else { Free-Line $name }
  Ui-Mark
  Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
  Ui-ExpectNew 'Sketch finished' "${name}: the sketch finished"
  Ui-WaitIdle
  Expect-OriginOff "$name after the sketch"
}

Write-Host '--- Editing an existing sketch draws its origin and axes too (mitcad#739)'
Ui-ExpectLog 'Timeline Sketch2 at' 'the timeline is laid out'
Ui-Mark
Ui-Step 'edit Sketch2 (double-click)'    { Ui-DoubleClickLogged 'Timeline Sketch2' }
Ui-ExpectNew 'Editing sketch F2 (Sketch2)' 'the XZ sketch is edited again' 30
Ui-WaitIdle
Expect-Axes 'the edited XZ sketch'
Expect-OriginOff 'the edited XZ sketch'
Line-FromOrigin -30 20 'the edited XZ sketch'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'the edited sketch finished'
Ui-WaitIdle
Expect-OriginOff 'after the edit'
if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }

Write-Host '--- New Design in a project: Component1 active, the origin planes for the pick (mitcad#844)'
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$projects = Join-Path $script:UiOut 'projects'
Remove-Item -Recurse -Force $projects -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $projects | Out-Null
# The Projects panel lists the test's own projects, not the user's.
$env:MITCAD_PROJECTS_DIR = $projects
$project = Join-Path $projects 'Shop'
$output = & $cli project create $project --author 'Windows Tester <windows@example.invalid>' --design base.mitcad 2>&1
if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli project create: $($output -join ' / ')" }
Ui-StartApp 'sketch-origin-project' @('--open', (Join-Path $project 'base.mitcad'))
Ui-ExpectLog 'base.mitcad' 'a design of the project opened'
Ui-Mark
Ui-Step 'new design (Ctrl+N)'            { Ui-Key 'ctrl+n' }
Ui-ExpectNew 'New Design: in ' 'a design of the current project, not an untitled one'
Ui-ExpectNew 'Projects panel: naming a new design in ' 'named in the Projects panel' 20
Ui-Step 'take its name (Enter)'          { Ui-Key 'Return' }
Ui-ExpectNew 'New Design: made ' 'made as a version' 20
Ui-ExpectNew 'Opened the new design with Component1 active' 'the new design opens with Component1 active' 20
if (-not (Test-Path (Join-Path $project 'Design1.mitcad'))) { Ui-Fail 'the new design is not in the project' }
Ui-WaitIdle
Expect-OriginOff 'the new design'
Ui-CreateSketch 'F7' 'xy'
Ui-ExpectNew 'Create Sketch: origin planes shown' 'the origin planes shown for the pick'
Ui-ExpectNew $shown 'the design''s origin shown for the pick, its light bulb off'
Ui-ExpectNew 'Sketch Sketch1 is in Component1' 'XY clicked in the view starts the sketch in Component1'
Expect-OriginOff 'the new design''s sketch'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'the new design''s sketch finished'
Ui-WaitIdle
Expect-OriginOff 'after the new design''s sketch'

Ui-Finish 'UI Windows sketch origin test'
