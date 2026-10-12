# SPDX-License-Identifier: MIT
# Input-driven Windows UI test of the Sheet Metal commands (mitcad#210), on
# a 35 x 20 mm rectangle mitcad-cli sketches (corner (-30, 0)):
#   1. Face (a shortcut of the test's) with rules of its own, thickness 2,
#      bend radius 3, K 0.3 typed: a 1400 mm3 plate, the rules and the Face
#      one undo step.
#   2. Flange on the plate's right edge (a click on it), 15 mm: the
#      folded volume 2000 + 44 pi (BA = 1.8 pi, the bend line on the edge).
#      Esc of a second Flange leaves the body as it is.
#   3. Edit Sheet Rules: thickness 3, then K 0.5 and bend radius 4, each one
#      undo step that recomputes the folded body.
#   4. Flat Pattern on the plate's (back) face: the browser's Flat Pattern
#      row, its area 1000 + 27.5 pi; Show Flat Pattern shows it flat, Undo
#      folded again.
#   5. Export Flat Pattern to a DXF file through its dialog: four outline
#      lines and one bend line, in millimetres, read back from the file.
#   6. The Flange edited from the timeline (a double-click): length 20.
#   7. The Floating layout: Edit Sheet Rules there too.
# Mitcad runs with Mesa's software OpenGL (MITCAD_MESA_DIR) in session 0
# over SSH, as tools/ui-windows-workflow-test.ps1 does.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-sheet-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-sheet'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{
  'sheet.face' = 'Shift+F2'; 'sheet.flange' = 'Shift+F3'; 'sheet.edit_rules' = 'Shift+F4'
  'sheet.flat_pattern' = 'Shift+F5'; 'sheet.export_dxf' = 'Shift+F6'; 'sheet.show_flat' = 'Shift+F8' }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$workLogged = $work -replace '\\', '/'
$pi = [Math]::PI

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$sketchScript = Join-Path $work 'sheet.json'
$design = Join-Path $work 'sheet.mitcad'
Set-Content -Encoding ascii $sketchScript @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-30, 0], "width": 35, "height": 20}
]
'@
$made = & $cli run $sketchScript --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

Write-Host '--- Face with rules of its own'
Ui-StartApp 'sheet' @('--open', $design)
Ui-ExpectLog "Opened $workLogged/sheet.mitcad" 'the design opened'
Ui-Step 'fit (F6)'                          { Ui-Key 'F6' }
Ui-Mark
Ui-Step 'face (Shift+F2)'                   { Ui-Key 'shift+F2' }
Ui-ExpectNew 'Command Face started' 'Face started'
Ui-ExpectNew 'Face Profiles: 1 profile' 'the rectangle taken'
Ui-Step 'thickness 2'                       { Ui-TypeIn 'Panel Face input thickness' '2' }
Ui-Step 'bend radius 3'                     { Ui-TypeIn 'Panel Face input bend_radius' '3' }
Ui-Step 'K-factor 0.3'                      { Ui-TypeIn 'Panel Face input k_factor' '0.3' }
Ui-ExpectNew 'Preview Face: ok' 'the face previews'
Ui-Step 'confirm (Enter)'                   { Ui-Key 'Return' }
Ui-ExpectNew 'Added Face as F3' 'the face added after its rules'
Ui-ExpectNew 'Added sheet face of 1 profile(s), new rules, thickness 2' 'the face has rules of its own'
Ui-ExpectVolume 'New body \S+ \(F3\.b0\): volume ([0-9.]+) mm3' 1400 'the plate'

Write-Host '--- Flange'
Ui-Mark
Ui-Step 'flange (Shift+F3)'                 { Ui-Key 'shift+F3' }
Ui-ExpectNew 'Pick places:' 'Flange logs where its picks are'
Ui-Step 'pick the right edge'               { Ui-ClickPick 'edge F3.b0/E{F3:sheet_front(0)|F3:sheet_side0(F1:sheet_curve(c2[c1,c3]))}' }
Ui-ExpectNew 'Flange Edge: 1 edge' 'the edge picked'
Ui-Step 'length 15'                         { Ui-TypeIn 'Panel Flange input length' '15' }
Ui-ExpectNew 'Preview Flange: ok' 'the flange previews'
Ui-Step 'confirm (Enter)'                   { Ui-Key 'Return' }
Ui-ExpectNew 'Added flange, length 15' 'the flange added'
Ui-ExpectVolume 'Body \S+ \(F3\.b0\): volume 1400\.000 -> ([0-9.]+) mm3' (2000 + 44 * $pi) 'the folded flange'
# Cancel leaves the body as it is.
Ui-Mark
Ui-Step 'flange again (Shift+F3)'           { Ui-Key 'shift+F3' }
Ui-ExpectNew 'Command Flange started' 'Flange started again'
Ui-Step 'cancel (Esc)'                      { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Flange cancelled' 'Flange cancelled'

Write-Host '--- Rules: thickness, K and bend radius'
Ui-Mark
Ui-Step 'edit sheet rules (Shift+F4)'       { Ui-Key 'shift+F4' }
Ui-ExpectNew 'Editing F2 with Sheet Rules' 'the rules open for editing'
Ui-Step 'thickness 3'                       { Ui-TypeIn 'Panel Sheet Rules input thickness' '3' }
Ui-ExpectNew 'Preview Sheet Rules: ok' 'the thicker sheet previews'
Ui-Step 'confirm (Enter)'                   { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F2' 'the rules edited'
Ui-ExpectVolume 'Body \S+ \(F3\.b0\): volume [0-9.]+ -> ([0-9.]+) mm3' (3000 + 76.5 * $pi) 'thickness 3'
Ui-Mark
Ui-Step 'edit sheet rules (Shift+F4)'       { Ui-Key 'shift+F4' }
Ui-ExpectNew 'Editing F2 with Sheet Rules' 'the rules open again'
Ui-Step 'K-factor 0.5'                      { Ui-TypeIn 'Panel Sheet Rules input k_factor' '0.5' }
Ui-Step 'bend radius 4'                     { Ui-TypeIn 'Panel Sheet Rules input bend_radius' '4' }
Ui-ExpectNew 'Preview Sheet Rules: ok' 'K and radius preview'
Ui-Step 'confirm (Enter)'                   { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F2' 'K and radius edited'
Ui-ExpectVolume 'Body \S+ \(F3\.b0\): volume [0-9.]+ -> ([0-9.]+) mm3' (3000 + 82.5 * $pi) 'K 0.5, radius 4'

Write-Host '--- Flat Pattern'
Ui-Mark
Ui-Step 'flat pattern (Shift+F5)'           { Ui-Key 'shift+F5' }
Ui-ExpectNew 'Pick places:' 'Flat Pattern logs where its picks are'
Ui-Step 'pick the plate'                    { Ui-ClickPick 'face F3.b0/F3:sheet_back(0)' }
Ui-ExpectNew 'Flat Pattern Fixed Face: 1 face' 'the plate picked'
Ui-Step 'confirm (Enter)'                   { Ui-Key 'Return' }
$area = 1000 + 27.5 * $pi
Ui-ExpectNew 'Flat pattern of Body1: valid, area 1086.39' 'the flat pattern made'
Ui-ExpectNew 'Browser Root/Bodies/Body1/Flat Pattern at' 'the Flat Pattern row under the body'
Ui-Mark
Ui-Step 'show flat (Shift+F8)'              { Ui-Key 'shift+F8' }
Ui-ExpectNew 'Flat pattern of Body1 shown' 'shown flat'
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                     { Ui-Key 'ctrl+z' }
Ui-Step 'show flat again (Shift+F8)'        { Ui-Key 'shift+F8' }
Ui-ExpectNew 'Flat pattern of Body1 shown' 'Undo folded it, shown flat again'
Ui-Step 'undo (Ctrl+Z)'                     { Ui-Key 'ctrl+z' }

Write-Host '--- Export Flat Pattern'
$dxf = Join-Path $work 'flat.dxf'
Ui-Mark
Ui-Step 'export flat pattern (Shift+F6)'    { Ui-Key 'shift+F6' }
Ui-Step 'type the path'                     {
  Ui-FocusDialog 'Export Flat Pattern'
  Ui-Key 'ctrl+a'
  Ui-Type $dxf
  Start-Sleep -Milliseconds 300
  Ui-Key 'Return'
  Ui-FocusMain
}
Ui-ExpectNew "Exported $workLogged/flat.dxf: dxf flat pattern of Body1, 5 entities (CUT_OUTER 4, BEND_" 'the drawing written' 30
if (-not (Test-Path $dxf)) { Ui-Fail 'no DXF file' }
$text = (Get-Content -Raw $dxf) -split "`r?`n" | ForEach-Object { $_.Trim() }
$lines = @(for ($i = 0; $i -lt $text.Count - 1; $i++) { if ($text[$i] -eq '0' -and $text[$i + 1] -eq 'LINE') { $i } })
if ($lines.Count -ne 5) { Ui-Fail "the DXF file has $($lines.Count) lines, expected 5" }
$units = [array]::IndexOf($text, '$INSUNITS')
if ($units -lt 0 -or $text[$units + 2] -ne '4') { Ui-Fail 'the DXF file is not in millimetres' }
foreach ($layer in 'CUT_OUTER', 'CUT_INNER', 'BEND_UP', 'BEND_DOWN') {
  if ($text -notcontains $layer) { Ui-Fail "the DXF file has no layer $layer" }
}
Write-Host 'ok   the DXF file: 5 lines, millimetres, the four layers'

Write-Host '--- Flange edited from the timeline'
Ui-Mark
Ui-Step 'edit the flange (double-click)'    { Ui-DoubleClickLogged 'Timeline SheetFlange1' }
Ui-ExpectNew 'Editing F4 with Flange' 'the flange opens for editing'
Ui-ExpectNew 'Flange Edge: 1 face' 'its side face loaded'
Ui-Step 'length 20'                         { Ui-TypeIn 'Panel Flange input length' '20' }
Ui-ExpectNew 'Preview Flange: ok' 'the longer flange previews'
Ui-Step 'confirm (Enter)'                   { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F4' 'the flange edited'
Ui-ExpectVolume 'Body \S+ \(F3\.b0\): volume [0-9.]+ -> ([0-9.]+) mm3' (3300 + 82.5 * $pi) 'a 20 mm flange'
Ui-Step 'save (Ctrl+S)'                     { Ui-Key 'ctrl+s' }
Ui-ExpectNew "Saved $workLogged/sheet.mitcad" 'saved'

Write-Host '--- Floating layout'
Ui-StartApp 'sheet-floating' @('--chrome=floating', '--open', $design)
Ui-ExpectLog "Opened $workLogged/sheet.mitcad" 'the design opened floating'
Ui-Mark
Ui-Step 'edit sheet rules (Shift+F4)'       { Ui-Key 'shift+F4' }
Ui-ExpectNew 'Editing F2 with Sheet Rules' 'the rules open in the Command card'
Ui-Step 'thickness 2'                       { Ui-TypeIn 'Panel Sheet Rules input thickness' '2' }
Ui-ExpectNew 'Preview Sheet Rules: ok' 'the thinner sheet previews'
Ui-Step 'confirm (Enter)'                   { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F2' 'the rules edited floating'

Ui-Finish 'Windows sheet metal UI'
