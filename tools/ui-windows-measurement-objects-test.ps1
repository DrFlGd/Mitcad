# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/framework app/browser app/view core/model/src/api tools/cli
# Input-driven Windows UI test of measurement objects (mitcad#649) in the
# build VM (tools/ui-windows-lib.ps1 says how the input is posted), as
# tools/ui-measurement-objects-test.sh on Linux, on the design of
# tools/ui-windows-measure-test.ps1: two cylinders of radius 5, 20 high
# (Body1 about Z through the origin, Body2 through (30, 40)) and a block
# (Body3).
#   1. Keep in the Measure panel: the round faces kept as AxisDistance1.
#   2. Keep Row in the history: Body1's top rim kept as Diameter1.
#   3. AxisDistance1 renamed Spacing, recoloured green, hidden.
#   4. Export Snapshot (F9 here) at 4 x the view, transparent: a PNG of
#      four times the view's size with an alpha channel.
#   5. Shown again, saved, opened again: both measured and drawn.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-measurement-objects-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-measurement-objects'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'view.export_snapshot' = 'F9' }

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null

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

# Wait-Enabled "place": until the last "<place> at x,y" line is not
# " (disabled)": the button's place logged once it is enabled.
function Wait-Enabled([string]$Place) {
  $last = $null
  for ($i = 0; $i -lt 30; $i++) {
    $last = Ui-LogLines | Where-Object { $_.Contains("$Place at ") } | Select-Object -Last 1
    if ($last -and -not "$last".EndsWith('(disabled)')) { return }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "'$Place' is not enabled: $last"
}

Ui-StartApp 'measurement-objects' @('--open', $design)
Ui-ExpectLog 'Measurements: none' 'no measurement objects yet'
Ui-Step 'fit (F6)'                         { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F2.b0' 'the cylinders shown'

Write-Host '--- 1. Keep in the Measure panel'
Ui-Mark
Ui-Step 'measure (I)'                      { Ui-Key 'i' }
Ui-ExpectNew 'Command Measure started' 'measure started'
Ui-ExpectNew 'Measure Keep at' 'the panel has Keep'
Ui-Step 'pick Body1''s round face'         { Ui-ClickPick 'face F2.b0/F2:side(c1)' }
Ui-Step 'pick Body2''s round face'         { Ui-ClickPick 'face F4.b0/F4:side(c1)' }
Ui-ExpectNew 'distance 40, angle 0 deg, axis distance 50, axis lines 2' 'the axes 50 apart'
Ui-ExpectNew 'Measure Keep enabled' 'Keep enabled for the model''s picks'
Wait-Enabled 'Measure Keep'
Ui-Mark
Ui-Step 'Keep'                             { Ui-ClickLogged 'Measure Keep' }
Ui-ExpectNew 'Measure Keep clicked' 'Keep clicked'
Ui-ExpectNew 'Measurement kept: AxisDistance1' 'kept as the axes'' distance'
Ui-ExpectNew 'Measurements drawn: AxisDistance1 = 50 mm (#d9480f)' 'drawn in the first colour'
Ui-Step 'close (Enter)'                    { Ui-Key 'Return' }
Ui-ExpectNew 'Closed Measure' 'measure closed'
Ui-ExpectLog 'Browser Root/Measurements/AxisDistance1 at' 'the browser lists it under Measurements'

Write-Host '--- 2. Keep Row in the history'
Ui-Mark
Ui-Step 'measure (I)'                      { Ui-Key 'i' }
Ui-Step 'pick Body1''s top rim'            { Ui-ClickPick 'edge F2.b0/E{F2:end(r{c1})|' }
Ui-ExpectNew 'Inspect Measure: length 31.416, radius 5, diameter 10' 'the rim measured'
Ui-ExpectNew 'Measure history row 2: Edge of Body1' 'its row in the history'
Ui-ExpectNew 'Measure history row 2 at' 'its row laid out'
Ui-Step 'show the rim''s row again'        { Ui-ClickLogged 'Measure history row 2' }
Ui-ExpectNew 'Measure history showed row 2' 'shown again'
Wait-Enabled 'Measure history Keep'
Ui-Mark
Ui-Step 'Keep Row'                         { Ui-ClickLogged 'Measure history Keep' }
Ui-ExpectNew 'Measure history Keep clicked: row 2' 'Keep Row clicked'
Ui-ExpectNew 'Measurement kept: Diameter1' 'kept as the rim''s diameter'
Ui-ExpectNew 'Measurements drawn: AxisDistance1 = 50 mm (#d9480f) | Diameter1 = 10 mm (#1971c2)' 'both drawn'
Ui-Step 'close (Enter)'                    { Ui-Key 'Return' }
Ui-ExpectNew 'Closed Measure' 'measure closed'

Write-Host '--- 3. Renamed, recoloured, hidden in the browser'
Ui-Mark
Ui-Step 'select AxisDistance1''s row'     { Ui-ClickLogged 'Browser Root/Measurements/AxisDistance1' }
Ui-Step 'rename (F2)'                     { Ui-Key 'F2' }
Ui-ExpectNew 'Renaming Root/Measurements/AxisDistance1' 'renamed in place'
Ui-Step 'type Spacing, Enter'             { Ui-Type 'Spacing'; Ui-Key 'Return' }
Ui-ExpectNew 'Renamed AxisDistance1 to Spacing' 'AxisDistance1 is Spacing'
Ui-ExpectNew 'Browser Root/Measurements/Spacing at' 'the row has its new name'
Ui-Mark
Ui-Step 'its menu (right click)'          { Ui-ClickLogged 'Browser Root/Measurements/Spacing' 3 }
Ui-ExpectNew 'Context menu: Colour' 'the menu starts with Colour'
Ui-Step 'Colour (Down, Enter)'            { Ui-Key 'Down'; Ui-Key 'Return' }
Ui-ExpectNew 'Measurement colour dialog opened: Spacing #d9480f (Orange | Blue | Green |' 'the colours offered'
Ui-Step 'Green (Down Down, Enter)' {
  Ui-FocusDialog '^Measurement Colour$'
  Ui-Key 'Down'
  Ui-Key 'Down'
  Ui-Key 'Return'
  Ui-FocusMain
}
Ui-ExpectNew 'Measurement Spacing colour #2f9e44' 'recoloured green'
Ui-ExpectNew 'Measurements drawn: Spacing = 50 mm (#2f9e44) | Diameter1 = 10 mm (#1971c2)' 'drawn green'
Ui-Mark
Ui-Step 'hide it (light bulb)'            { Ui-ClickLogged 'Browser eye Root/Measurements/Spacing' }
Ui-ExpectNew 'Visibility Root/Measurements/Spacing: hidden' 'hidden'
Ui-ExpectNew 'Measurements drawn: Diameter1 = 10 mm (#1971c2)' 'no longer drawn'

Write-Host '--- 4. Export Snapshot: 4 x the view, transparent'
$area = Ui-LastMatch 'View area (\d+ \d+ \d+ \d+)'
if (-not $area) { Ui-Fail 'the app did not log its view area' }
$view = $area -split ' ' | ForEach-Object { [int]$_ }
$width = 4 * $view[2]
$height = 4 * $view[3]
$png = Join-Path $work 'snapshot.png'
Ui-Mark
Ui-Step 'export snapshot (F9)'            { Ui-Key 'F9' }
Ui-ExpectNew 'Snapshot dialog opened: size 2x, transparent off, measurements on (1 shown)' 'the dialog with its defaults'
Ui-Step 'type the file' {
  Ui-FocusDialog '^Export Snapshot$'
  Ui-Key 'ctrl+a'
  Ui-Type $png
}
Ui-Step 'Size: 4 x (Alt+Z, Down)'         { Ui-Key 'alt+z'; Ui-Key 'Down' }
Ui-ExpectNew 'Snapshot dialog: size = 4x' 'four times the view'
Ui-Step 'Transparent background (Alt+T)'  { Ui-Key 'alt+t' }
Ui-ExpectNew 'Snapshot dialog: transparent = on' 'a transparent background'
Ui-Step 'OK (Enter)'                      { Ui-Key 'Return'; Ui-FocusMain }
Ui-ExpectNew "Snapshot saved ${png}: $width x $height, transparent, 1 measurement(s)" 'the snapshot written' 30
if (-not (Test-Path $png)) { Ui-Fail 'no snapshot file' }
$bytes = [System.IO.File]::ReadAllBytes($png)
$pw = ([int]$bytes[16] -shl 24) -bor ([int]$bytes[17] -shl 16) -bor ([int]$bytes[18] -shl 8) -bor [int]$bytes[19]
$ph = ([int]$bytes[20] -shl 24) -bor ([int]$bytes[21] -shl 16) -bor ([int]$bytes[22] -shl 8) -bor [int]$bytes[23]
if ($pw -ne $width -or $ph -ne $height) { Ui-Fail "the snapshot is $pw x $ph, not $width x $height" }
if ($bytes[25] -ne 6) { Ui-Fail "the snapshot has no alpha channel (PNG colour type $($bytes[25]))" }
Write-Host "ok   snapshot.png is $pw x $ph with an alpha channel, $($bytes.Length) bytes"

Write-Host '--- 5. Shown again, saved and opened again'
Ui-Mark
Ui-Step 'show Spacing (light bulb)'       { Ui-ClickLogged 'Browser eye Root/Measurements/Spacing' }
Ui-ExpectNew 'Visibility Root/Measurements/Spacing: shown' 'shown'
Ui-ExpectNew 'Measurements drawn: Spacing = 50 mm (#2f9e44) | Diameter1 = 10 mm (#1971c2)' 'drawn again'
Ui-Step 'save (Ctrl+S)'                   { Ui-Key 'ctrl+s' }
Ui-ExpectNew 'Saved ' 'saved'
if (-not (Select-String -Path $design -Pattern '"measurements"' -Quiet)) { Ui-Fail 'the design file has no measurements' }
Ui-StartApp 'measurement-objects-again' @('--open', $design)
Ui-ExpectLog 'Measurements: Spacing axis_distance | Diameter1 diameter' 'both read back'
Ui-ExpectLog 'Measurements drawn: Spacing = 50 mm (#2f9e44) | Diameter1 = 10 mm (#1971c2)' 'and drawn'

if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }
Ui-Finish 'UI Windows measurement objects test'
