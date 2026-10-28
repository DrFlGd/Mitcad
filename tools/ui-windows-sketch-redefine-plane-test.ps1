# SPDX-License-Identifier: MIT
# check-all sources: app/commands/SketchPlaneCommands.cpp app/browser app/MainWindow.cpp app/MainWindowContextMenu.cpp
# check-all sources: app/framework/CommandSession.cpp tools/cli
# check-all sources: core/model/src/document/sketch_plane.rs core/sketch/src/placement.rs core/model-api/src/sketch_plane.rs
# Input-driven Windows UI test of Redefine Sketch Plane (mitcad#787) in the
# build VM (tools/ui-windows-lib.ps1 says how the input is posted), as
# tools/ui-sketch-redefine-plane-test.sh does on Linux, on a design that
# mitcad-cli makes: Sketch1 (F1) 30 x 20 on XY and Extrude1 (F2) 10 up,
# Body1 (F2.b0); Sketch2 (F3) 10 x 6 at (5, 5) on XY and Extrude2 (F4) 3
# up, Body2 (F4.b0). The context menu stays open in session 0 and takes
# the keys (as in tools/ui-windows-move-copy-menu-test.ps1).
#   1. The browser: Redefine Sketch Plane on Sketch2's row starts with
#      Sketch2; Body1's top face picked and an offset of 2 typed, OK moves
#      the sketch onto the face, 12 above XY; one undo takes it back.
#   2. A face of Extrude1 for Sketch1 fails the preview with the reason.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-sketch-redefine-plane-test.ps1 [-App build\dev\mitcad.exe] [-Cli build\dev\tools\cli\mitcad-cli.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Cli = '',
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-sketch-redefine-plane'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

# Top View by a key: the origin planes the Plane input shows stand in front
# of Body1's top face in the isometric view (XZ at y = 0 between it and the
# eye) and are the picks there; seen from the top, XZ and YZ are edge-on and
# XY lies behind the face.
Ui-Init $App $Out $QtBin @{ 'view.top' = 'F7' } -StepPause 0
if (-not $Cli) {
  $Cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
    ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $Cli -or -not (Test-Path $Cli)) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null

$modelScript = Join-Path $work 'model.json'
$design = Join-Path $work 'design.mitcad'
Set-Content -Encoding ascii $modelScript @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [5, 5], "width": 10, "height": 6},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 3}, "operation": "new_body"}}
]
'@
$made = & $Cli run $modelScript --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

# Menu-Entries: the entries of the context menu logged last (the labels'
# ellipsis as three dots).
$ellipsis = [string][char]0x2026
function Menu-Entries {
  return @("$(Ui-LastMatch '^Context menu: (.*)$')" -split ' \| ' | ForEach-Object { $_.Replace($ellipsis, '...') })
}
function Expect-Entry([string]$Entry) {
  $entries = Menu-Entries
  if (-not ($entries -contains $Entry)) { Ui-Fail "'$Entry' is not in the context menu: $($entries -join ' | ')" }
  Write-Host "ok   the context menu has $Entry"
}
# Menu-Choose "entry": the entry chosen with the keys, as the menu opens
# with nothing active.
function Menu-Choose([string]$Entry) {
  $entries = Menu-Entries
  $index = [array]::IndexOf($entries, $Entry)
  if ($index -lt 0) { Ui-Fail "'$Entry' is not in the context menu: $($entries -join ' | ')" }
  Ui-Step $Entry {
    for ($i = 0; $i -le $index; $i++) { Ui-Key 'Down' }
    Ui-Key 'Return'
  }
}
# Expect-Vector "part" "x y z" "description": the frame part (origin, x, y)
# of the last "Redefined the plane of" line, each coordinate within 1e-6.
function Expect-Vector([string]$Part, [double[]]$Want, [string]$Description) {
  $text = Ui-LastMatch "Redefined the plane of .*[;,] (?:frame )?$Part \(([^)]*)\)"
  if (-not $text) { Ui-Fail "${Description}: no $Part in the last 'Redefined the plane of' line" }
  $got = $text -split ', *' | ForEach-Object { [double]::Parse($_, [Globalization.CultureInfo]::InvariantCulture) }
  for ($i = 0; $i -lt 3; $i++) {
    if ([Math]::Abs($got[$i] - $Want[$i]) -gt 1e-6) { Ui-Fail "${Description}: ($text), expected ($($Want -join ', '))" }
  }
  Write-Host "ok   ${Description}: ($text)"
}

Ui-StartApp 'sketch-redefine-plane' @('--open', $design)
Ui-Mark
Ui-Step 'top view (F7)'                  { Ui-Key 'F7' }
Ui-ExpectNew 'Camera direction ' 'the view looks down'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-ExpectLog 'Browser Root/Sketches/Sketch2 at' 'the browser lists Sketch2'
Ui-ExpectLog 'Bodies shown: F2.b0, F4.b0' 'both bodies shown'

Write-Host '--- The browser: Sketch2 onto Body1''s top face, 2 above it'
Ui-Mark
Ui-Step 'right-click Sketch2''s row'      { Ui-ClickLogged 'Browser Root/Sketches/Sketch2' 3 }
Ui-ExpectNew 'Context menu: ' 'the row''s menu'
Expect-Entry 'Redefine Sketch Plane'
Menu-Choose 'Redefine Sketch Plane'
Ui-ExpectNew 'Command Redefine Sketch Plane started' 'the panel started'
Ui-ExpectNew 'Redefine Sketch Plane Sketch: 1 sketch [sketch F3]' 'with Sketch2'
Ui-ExpectNew 'Panel Redefine Sketch Plane input plane at' 'the panel is laid out'
Ui-Mark
Ui-Step 'activate Plane'                 { Ui-ClickLogged 'Panel Redefine Sketch Plane input plane' }
Ui-ExpectNew 'Pick face F2.b0/F2:end(' 'Body1''s top face can be picked'
Ui-Step 'pick Body1''s top face'          { Ui-ClickPick 'face F2.b0/F2:end(' }
Ui-ExpectNew 'Redefine Sketch Plane Plane: 1 face [face F2:end(' 'the top face is the plane'
Ui-Step 'offset 2 mm'                    { Ui-TypeIn 'Panel Redefine Sketch Plane input offset' '2' }
Ui-ExpectNew 'Preview Redefine Sketch Plane: ok' 'previewed'
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Redefined the plane of Sketch2: face F2:end(' 'Sketch2 is on the face'
Expect-Vector 'origin' @(0, 0, 12) 'the frame''s origin 2 above the face'
Expect-Vector 'x' @(1, 0, 0) 'its x axis'
Expect-Vector 'y' @(0, 1, 0) 'its y axis'
Ui-Mark
Ui-Step 'undo'                           { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: ' 'one undo'
Ui-ExpectNew 'Redefine Plane of Sketch2' 'the undo takes the redefine back'
Ui-Step 'Escape (clear the selection)'   { Ui-Key 'Escape' }

Write-Host '--- A face of a later feature is refused with the reason'
Ui-Mark
Ui-Step 'right-click Sketch1''s row'      { Ui-ClickLogged 'Browser Root/Sketches/Sketch1' 3 }
Ui-ExpectNew 'Context menu: ' 'the row''s menu'
Menu-Choose 'Redefine Sketch Plane'
Ui-ExpectNew 'Redefine Sketch Plane Sketch: 1 sketch [sketch F1]' 'with Sketch1'
Ui-ExpectNew 'Panel Redefine Sketch Plane input plane at' 'the panel is laid out'
Ui-Mark
Ui-Step 'activate Plane'                 { Ui-ClickLogged 'Panel Redefine Sketch Plane input plane' }
Ui-ExpectNew 'Pick face F2.b0/F2:end(' 'Body1''s top face can be picked'
Ui-Step 'pick Body1''s top face'          { Ui-ClickPick 'face F2.b0/F2:end(' }
Ui-ExpectNew 'Preview Redefine Sketch Plane: failed: Sketch1 cannot lie on a plane or face of Extrude1' 'the preview fails'
Ui-ExpectNew 'Extrude1 comes after Sketch1 in the timeline' 'with the reason'
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Redefine Sketch Plane cancelled' 'cancelled'

if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }

Ui-Finish 'UI Windows Redefine Sketch Plane'
