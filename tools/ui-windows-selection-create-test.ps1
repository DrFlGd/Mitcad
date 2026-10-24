# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindowSelectionCreate.cpp app/framework/SelectionCreate.cpp
# check-all sources: app/framework/SelectionCreate.hpp app/framework/WindowStyle.cpp app/MainWindow.cpp
# check-all sources: app/framework/Command.cpp app/framework/CommandSession.cpp app/commands tools/cli
# Input-driven Windows UI test of the selection card's Create row
# (mitcad#619) in the build VM (tools/ui-windows-lib.ps1 says how the
# input is posted), in the Docked layout, as tools/ui-selection-create-test.sh
# on Linux (which also runs it in the Floating layout), on a design
# mitcad-cli makes: a cylinder 20 mm across and 30 high at the origin
# (Extrude1, F2.b0, its round face F2:side(c1)) and an unused 20 mm square
# beside it (Sketch2, F3):
#   1. The square's profile selected: the row offers Extrude, Revolve and
#      Sweep ("Selection card create: Extrude | Revolve | Sweep").
#   2. Its Extrude button clicked: Extrude starts with that profile in
#      Profiles. Esc cancels.
#   3. The cylinder's round face selected: Thread first.
#   4. The selection cleared (Esc): "Selection card create: none".
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-selection-create-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-selection-create'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

# Thread by a shortcut of the test's own: session 0 has no command search.
Ui-Init $App $Out $QtBin @{ 'solid.thread' = 'F7' }

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$rect = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
$side = 'F2:side(c1)'

$modelJson = Join-Path $work 'model.json'
$design = Join-Path $work 'create.mitcad'
Set-Content -Encoding ascii $modelJson @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1}"}],
    "extent": {"type": "distance", "distance": 30}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [30, -10], "width": 20, "height": 20}
]
'@
$output = & $cli run $modelJson --save $design 2>&1
if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }

# Expect-Create "A | B" "description": the row's log line is exactly that,
# since the mark (up to 6 s).
function Expect-Create([string]$Text, [string]$Description) {
  for ($i = 0; $i -lt 30; $i++) {
    $lines = Ui-LogLines | Select-Object -Skip $script:UiMark
    if ($lines | Where-Object { $_ -ceq "Selection card create: $Text" }) { Write-Host "ok   $Description"; return }
    Start-Sleep -Milliseconds 200
  }
  $last = Ui-LogLines | Where-Object { $_.StartsWith('Selection card create: ') } | Select-Object -Last 1
  Ui-Fail "${Description}: the row is not '$Text' ($last)"
}

Ui-StartApp 'selection-create' @('--open', $design)
Ui-Step 'fit (F6)'                         { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F2.b0' 'the cylinder shown'

# Extrude once for the log's pick places of the profiles: it takes the
# square, the newest profile. Cancelled.
Ui-Mark
Ui-Step 'extrude (E)'                      { Ui-Key 'e' }
Ui-ExpectNew "Extrude Profiles: 1 profile [profile $rect of F3]" 'with nothing selected, the square'
Ui-ExpectNew "Pick profile F3/$rect at" 'the square can be picked'
Ui-Step 'cancel (Esc)'                     { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'cancelled'

Write-Host '--- 1. A profile selected'
Ui-Mark
Ui-Step 'select the square'                { Ui-ClickPick "profile F3/$rect" }
Ui-ExpectNew "Selected: 1 profile [profile $rect of F3]" 'the square selected'
Expect-Create 'Extrude | Revolve | Sweep' 'the row: Extrude, Revolve, Sweep'
Ui-ExpectNew 'Selection card create Extrude at ' 'its Extrude button laid out'

Write-Host '--- 2. Its Extrude button'
Ui-Mark
Ui-Step 'click Extrude on the card'        { Ui-ClickLogged 'Selection card create Extrude' }
Ui-ExpectNew 'Selection card create chose Extrude' 'the card''s button clicked'
Ui-ExpectNew 'Command Extrude started' 'Extrude started'
Ui-ExpectNew "Extrude Profiles: 1 profile [profile $rect of F3]" 'with the selected profile'
Ui-ExpectNew 'Preview Extrude: ok' 'previewed' 30
Ui-Mark
Ui-Step 'cancel (Esc)'                     { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'cancelled'

Write-Host '--- 3. A cylinder''s round face selected'
# Thread once for the log's pick places of the round face. Cancelled.
Ui-Mark
Ui-Step 'thread (F7)'                      { Ui-Key 'F7' }
Ui-ExpectNew 'Command Thread started' 'Thread started'
Ui-ExpectNew "Pick face F2.b0/$side at" 'the round face can be picked'
Ui-Step 'cancel (Esc)'                     { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Thread cancelled' 'cancelled'
Ui-Mark
Ui-Step 'select the round face'            { Ui-ClickPick "face F2.b0/$side" }
Ui-ExpectNew "Selected: 1 face [face $side of F2.b0" 'the round face selected'
Ui-ExpectNew 'Selection card create: Thread' 'Thread first'
$last = Ui-LogLines | Where-Object { $_.StartsWith('Selection card create: ') } | Select-Object -Last 1
if ("$last".Contains('Extrude')) { Ui-Fail "Extrude offered for a round face: $last" }
Write-Host 'ok   no Extrude for the round face'

Write-Host '--- 4. The selection cleared'
Ui-Mark
Ui-Step 'clear the selection (Esc)'        { Ui-Key 'Escape' }
Ui-ExpectNew 'Selected: nothing' 'nothing selected'
Expect-Create 'none' 'the row: none'

if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }
Ui-Finish 'UI Windows selection create test'
