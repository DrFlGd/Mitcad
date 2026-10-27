# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/framework core/model-api/src tools/cli
# Input-driven Windows UI test of Measure in the build VM
# (tools/ui-windows-lib.ps1 says how the input is posted), as
# tools/ui-measure-test.sh on Linux, on a design mitcad-cli makes: two
# cylinders of radius 5, 20 high (Body1 about Z through the origin, Body2
# through (30, 40)) and a block 10 x 10 x 20 at (20, -30) (Body3).
# Axes (mitcad#647):
#   1. A round face: its axis through its centre, drawn as a line.
#   2. Two round faces: the surfaces 40 apart, the axes parallel and 50.
#   3. A round face against the block's top: square, no axis distance.
#   4. Two top rims: the axes' and the centres' distance.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-measure-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-measure'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$rect = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

$modelJson = Join-Path $work 'model.json'
$design = Join-Path $work 'axes.mitcad'
Set-Content -Encoding ascii $modelJson @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F3", "center": [30, 40], "diameter": 10},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F5", "corner": [20, -30], "width": 10, "height": 10},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F5", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}
]
'@
$output = & $cli run $modelJson --save $design 2>&1
if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }

Ui-StartApp 'measure' @('--open', $design)
Ui-Step 'fit (F6)'                         { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F2.b0' 'the cylinders shown'

Write-Host '--- 1. A round face''s axis'
Ui-Mark
Ui-Step 'measure (I)'                      { Ui-Key 'i' }
Ui-ExpectNew 'Command Measure started' 'measure started'
Ui-Step 'pick Body1''s round face'         { Ui-ClickPick 'face F2.b0/F2:side(c1)' }
Ui-ExpectNew 'Inspect Measure: area 628.319, radius 5, diameter 10, axis (0, 0, 10) along (0, 0, ' 'its axis through its centre'
Ui-ExpectNew 'axis lines 1' 'the axis drawn through it'

Write-Host '--- 2. Parallel axes'
Ui-Mark
Ui-Step 'pick Body2''s round face'         { Ui-ClickPick 'face F4.b0/F4:side(c1)' }
Ui-ExpectNew 'axis (30, 40, 10) along (0, 0, ' 'the second axis'
Ui-ExpectNew 'distance 40, angle 0 deg, axis distance 50, axis lines 2' 'the surfaces 40 apart, the axes parallel and 50 apart'
Ui-Step 'close (Enter)'                    { Ui-Key 'Return' }
Ui-ExpectNew 'Closed Measure' 'measure closed'

Write-Host '--- 3. A round face against a plane'
Ui-Mark
Ui-Step 'measure (I)'                      { Ui-Key 'i' }
Ui-Step 'pick Body1''s round face'         { Ui-ClickPick 'face F2.b0/F2:side(c1)' }
Ui-ExpectNew 'axis lines 1' 'its axis'
Ui-Step 'pick the block''s top'            { Ui-ClickPick "face F6.b0/F6:end($rect)" }
Ui-ExpectNew 'angle 90 deg, axis lines 1' 'square to the top, no axis distance'
Ui-Step 'close (Enter)'                    { Ui-Key 'Return' }
Ui-ExpectNew 'Closed Measure' 'measure closed'

Write-Host '--- 4. Two rims'
Ui-Mark
Ui-Step 'measure (I)'                      { Ui-Key 'i' }
Ui-Step 'pick Body1''s top rim'            { Ui-ClickPick 'edge F2.b0/E{F2:end(r{c1})|' }
Ui-ExpectNew 'Inspect Measure: length 31.416, radius 5, diameter 10, axis (0, 0, 20) along' 'the rim''s axis'
Ui-Step 'pick Body2''s top rim'            { Ui-ClickPick 'edge F4.b0/E{F4:end(r{c1})|' }
Ui-ExpectNew 'axis distance 50, center distance 50, axis lines 2' 'the axes and the centres 50 apart'
Ui-Step 'close (Enter)'                    { Ui-Key 'Return' }
Ui-ExpectNew 'Closed Measure' 'measure closed'

Write-Host '--- 5. The history (mitcad#648)'
# The three measurements above, newest first, in the next panel.
Ui-Mark
Ui-Step 'measure (I)'                      { Ui-Key 'i' }
Ui-ExpectNew 'Measure history: 3 rows' 'three rows'
Ui-ExpectNew 'Measure history row 3: Edge of Body1' 'the rims'' row first'
Ui-ExpectNew ', Axis distance 50 mm, Centre distance 50 mm' 'with their axes'' and centres'' distances'
Ui-ExpectNew 'Measure history row 2: Face of Body1' 'the round face against the top'
Ui-ExpectNew 'Face of Body3: Radius 5 mm, Diameter 10 mm, Distance' 'with its distance'
Ui-ExpectNew 'Measure history row 1: Face of Body1' 'the parallel faces last'
Ui-ExpectNew 'Face of Body2: Radius 5 mm, Diameter 10 mm, Radius 5 mm, Diameter 10 mm, Distance 40 mm, Angle 0' 'with their values'
Ui-ExpectNew 'Measure history row 1 at' 'its row laid out'
Ui-Mark
Ui-Step 'show the first again'             { Ui-ClickLogged 'Measure history row 1' }
Ui-ExpectNew 'Measure history showed row 1' 'shown again'
Ui-ExpectNew 'Measure Selection 2: 1 face' 'its picks back in the inputs'
Ui-ExpectNew 'distance 40, angle 0 deg, axis distance 50, axis lines 2' 'measured again, its lines drawn'
if (Ui-LogLines | Where-Object { $_.Contains('Measure history: 4 rows') }) { Ui-Fail 'showing a row again added one' }
Ui-Mark
Ui-Step 'copy its row'                     { Ui-ClickLogged 'Measure history Copy' }
Ui-ExpectNew 'Measure history copied row 1: 1 | Face of Body1' 'the row copied as tab-separated text'
Ui-Step 'copy every row'                   { Ui-ClickLogged 'Measure history Copy All' }
Ui-ExpectNew 'Measure history copied 3 rows: 3 | Edge of Body1' 'every row, newest first'
Ui-ExpectNew ' / 1 | Face of Body1' 'down to the first'
Ui-Step 'clear'                            { Ui-ClickLogged 'Measure history Clear' }
Ui-ExpectNew 'Measure history cleared' 'cleared'
Ui-ExpectNew 'Measure history: 0 rows' 'no rows'
Ui-Step 'close (Enter)'                    { Ui-Key 'Return' }
Ui-ExpectNew 'Closed Measure' 'measure closed'

Write-Host '--- 6. The history in the Floating layout'
Ui-StopApp
Ui-StartApp 'measure-floating' @('--open', $design, '--chrome=floating')
Ui-Step 'fit (F6)'                         { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F2.b0' 'the cylinders shown'
# The Floating layout's command card (x from 716 in session 0's 1024x768)
# covers Body2 after the fit: zoomed out about Body1, a wheel step at a
# time, until Body2 lies left of the card.
$body1 = Ui-LoggedAt 'Body F2.b0'
for ($i = 0; $i -lt 12; $i++) {
  $body2 = Ui-LoggedAt 'Body F4.b0'
  if ($body2[0] -lt 680) { break }
  [UiInput]::Move($script:UiWindow, $body1[0], $body1[1], 0x04)
  Start-Sleep -Milliseconds 80
  [UiInput]::Wheel($script:UiWindow, $body1[0], $body1[1], -1)
  Ui-Sync
  Start-Sleep -Milliseconds 300
}
$body2 = Ui-LoggedAt 'Body F4.b0'
if ($body2[0] -ge 680) { Ui-Fail "Body2 stays under the command card at $($body2[0]),$($body2[1])" }
Write-Host "ok   Body2 left of the command card at $($body2[0]),$($body2[1])"
Ui-Mark
Ui-Step 'measure (I)'                      { Ui-Key 'i' }
Ui-ExpectNew 'Measure history: 0 rows' 'a design opened: no rows'
Ui-Step 'pick Body1''s round face'         { Ui-ClickPick 'face F2.b0/F2:side(c1)' }
Ui-Step 'pick Body2''s round face'         { Ui-ClickPick 'face F4.b0/F4:side(c1)' }
Ui-ExpectNew 'axis distance 50, axis lines 2' 'measured'
Ui-Step 'close (Enter)'                    { Ui-Key 'Return' }
Ui-ExpectNew 'Closed Measure' 'measure closed'
Ui-Mark
Ui-Step 'measure (I)'                      { Ui-Key 'i' }
Ui-ExpectNew 'Measure history: 1 rows' 'one row'
Ui-ExpectNew 'Measure history row 1: Face of Body1' 'the parallel faces'
Ui-ExpectNew 'Measure history row 1 at' 'its row laid out'
Ui-Step 'show it again'                    { Ui-ClickLogged 'Measure history row 1' }
Ui-ExpectNew 'Measure history showed row 1' 'shown again'
Ui-ExpectNew 'distance 40, angle 0 deg, axis distance 50, axis lines 2' 'measured again'
Ui-Step 'close (Enter)'                    { Ui-Key 'Return' }
Ui-ExpectNew 'Closed Measure' 'measure closed'

if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }
Ui-Finish 'UI Windows measure test'
