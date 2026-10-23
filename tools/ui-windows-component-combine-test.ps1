# SPDX-License-Identifier: MIT
# check-all sources: app/commands/MoveCommands.cpp app/framework/Command.hpp
# check-all sources: app/framework/CommandSession.cpp app/framework/CommandPanel.cpp
# check-all sources: core/model/src/features/combine.rs tools/cli
# check-all sources: app/OcctViewer.cpp app/framework/PreviewBodies.cpp
# Input-driven Windows UI test of Combine with the same body in two
# occurrences as tools (mitcad#544) in the build VM (tools/ui-windows-lib.ps1
# says how the input is posted), as the last part of
# tools/ui-component-combine-test.sh on Linux: a block in the root (F4.b0,
# 6000 mm3), Part:1's block overlapping it on one side and Part:2's on the
# other, 2000 mm3 each.
#   1. Combine, the root's block the target, Keep Tools off, Part's block
#      picked where Part:1 shows it: the consumed tool leaves Part, so the
#      preview draws it where Part:2 shows it as a tool to pick there
#      (mitcad#561), and the panel says it is also gone from Part:2; then
#      picked where Part:2 shows it: two tools, listed "Part:1, Part:2";
#      Cut, OK: both cut away, 2000 mm3 left.
#   2. The design file keeps a tool in each occurrence.
#   3. Edited: the panel takes both tools back where they were picked.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-component-combine-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-component-combine'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'solid.combine' = 'F8'; 'view.top' = 'F7' }

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$r = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'

# Part:1 (O1, C1): Sketch1 (F1) 30 x 20, Extrude1 (F2) 10 up: F2.b0.
# Part:2 (O2) 30 along X. The root, active: Sketch2 (F3) 20 x 20 at
# x = 20, Extrude2 (F4) 15 up: F4.b0, overlapping Part:1's block by
# 10 x 20 x 10 mm (x 20 to 30) and Part:2's as much (x 30 to 40).
$modelJson = Join-Path $work 'model.json'
Set-Content -Encoding ascii $modelJson (@'
[
  {"cmd": "create_component", "name": "Part"},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"},
  {"cmd": "copy_occurrence", "occurrence": "Part:1", "transform": {"translation": [30, 0, 0]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [20, 0], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 15}, "operation": "new_body"}}
]
'@).Replace('RECT', $r)
$file = Join-Path $work 'overlap.mitcad'
$output = & $cli run $modelJson --save $file 2>&1
if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }

Ui-StartApp 'component-combine' @('--open', $file)
Ui-Step 'top view (F7)'                  { Ui-Key 'F7' }
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: ' 'the blocks shown'
Ui-ExpectLog 'Body F2.b0 in O2 at ' "Part:2's block shown"

Write-Host '--- 1. The same body in two occurrences as tools'
Ui-Mark
Ui-Step 'combine (F8)'                   { Ui-Key 'F8' }
Ui-ExpectNew 'Command Combine started' 'Combine started'
# The root's block is clicked 20 px (3 mm) left of its logged middle: the
# middle (its box's centre, x 30) lies on the seam where Part:1's and
# Part:2's blocks meet below it, and there their edges won the click in
# session 0; at x 27 only the root's top face is under the cursor.
$root = Ui-LoggedAt 'Body F4.b0'
Ui-Step "pick the root's block"          { Ui-Click ($root[0] - 20) $root[1] }
Ui-ExpectNew 'Combine Target Body: 1 body [body F4.b0]' 'the target picked'
# Keep Tools stays off: a consumed tool leaves its own component
# (mitcad#104), so once Part:1's block is a tool the preview has Part's
# block in no occurrence; it draws it where Part:2 shows it as a tool
# overlay, which a click picks (mitcad#561).
Ui-Step 'Tool Bodies'                    { Ui-ClickLogged 'Panel Combine input tools' }
Ui-Mark
Ui-Step "pick Part:1's block"            { Ui-ClickLogged 'Body F2.b0 in O1' }
Ui-ExpectNew 'Combine Tool Bodies: 1 body [body F2.b0 in O1]' 'the tool picked where Part:1 shows it'
Ui-ExpectNew 'Combine Tool Bodies listed: Part:1' 'listed by its occurrence'
Ui-ExpectNew 'Preview Combine: ok' 'previewed with the tool consumed' 30
Ui-ExpectNew 'F2.b0 in O2 at 30,0,0 tool' "Part's block drawn as a tool where Part:2 shows it"
Ui-ExpectNew 'Panel Combine note: ' 'the panel notes where the tool goes'
Ui-ExpectNew 'leaves Part: it is also gone from Part:2.' 'the tool also gone from Part:2'
Ui-Mark
Ui-Step "pick Part:2's block"            { Ui-ClickLogged 'Body F2.b0 in O2' }
Ui-ExpectNew 'Combine Tool Bodies: 2 bodies [body F2.b0 in O1; body F2.b0 in O2]' 'the same body picked where Part:2 shows it'
Ui-ExpectNew 'Combine Tool Bodies listed: Part:1, Part:2' 'listed by their occurrences'
Ui-Mark
# One wheel step towards the user: the next operation, Cut.
Ui-Step 'Cut (wheel)'                    { Ui-WheelLogged 'Panel Combine input operation' -1 }
Ui-ExpectNew 'Preview Combine: ok' 'the cut previewed' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added combine (cut) of 2 tool(s)' 'the combine added' 30
# 6000 - 2000 - 2000 mm3.
Ui-ExpectVolume 'Body [A-Za-z0-9]* \(F4\.b0\): volume 6000\.000 -> ([0-9.]+) mm3' 2000 'both blocks cut away'
if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }

Write-Host '--- 2. The design file keeps a tool in each occurrence'
Ui-Mark
Ui-Step 'save (Ctrl+S)'                  { Ui-Key 'ctrl+s' }
Ui-ExpectNew 'Saved ' 'saved'
$combine = (Get-Content -Raw $file | ConvertFrom-Json).features | Where-Object { $_.type -eq 'combine' } |
  Select-Object -First 1
if (-not $combine) { Ui-Fail 'no combine in the saved design' }
$tools = @($combine.tools)
$shown = $tools | ConvertTo-Json -Compress
if ($tools.Count -ne 2 -or $tools[0].body -ne 'F2.b0' -or $tools[0].occurrence -ne 'O1' -or
    $tools[1].body -ne 'F2.b0' -or $tools[1].occurrence -ne 'O2') {
  Ui-Fail "the saved tools are $shown, expected F2.b0 in O1 and in O2"
}
if ($combine.PSObject.Properties.Name -contains 'tool_links') { Ui-Fail 'the saved combine has tool_links' }
Write-Host "ok   a tool in each occurrence: $shown"

Write-Host '--- 3. Edited: both tools back where they were picked'
Ui-Mark
Ui-Step 'edit Combine1 (double-click)'   { Ui-DoubleClickLogged 'Timeline Combine1' }
Ui-ExpectNew 'Editing F5 with Combine' 'editing the combine'
Ui-ExpectNew 'Combine Tool Bodies: 2 bodies [body F2.b0 in O1; body F2.b0 in O2]' 'both tools in their occurrences'
Ui-ExpectNew 'Combine Tool Bodies listed: Part:1, Part:2' 'listed by their occurrences again'
Ui-ExpectNew 'Preview Combine: ok' 'previewed with both' 30
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited F5' 'edited' 30
Ui-ExpectVolume 'Body [A-Za-z0-9]* \(F4\.b0\): volume [0-9.]+ -> ([0-9.]+) mm3' 2000 'still both cuts'
if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }

Ui-Finish 'UI Windows component combine test'
