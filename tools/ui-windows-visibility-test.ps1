# SPDX-License-Identifier: MIT
# check-all sources: app/browser app/framework app/MainWindow.cpp app/MainWindowVisibility.cpp tools/cli
# Input-driven Windows UI test of the browser's light bulbs in every mode
# (mitcad#295) and Create Sketch's origin planes (mitcad#294) in the build
# VM (tools/ui-windows-lib.ps1 says how the input is posted); the Linux test
# is tools/ui-visibility-test.sh:
#   1. A design without components (mitcad-cli): Body1 (20 x 20 x 10)
#      inside Body2 (60 x 60 x 30), Sketch3 with a free 20 x 20 square
#      beside them. Create Sketch shows the origin planes, no face of Body1
#      can be picked; Body2 hidden by its light bulb while the plane is
#      asked for, Body1's top face can be picked and the sketch starts on
#      it.
#   2. Extrude of the square, Body1 hidden meanwhile: the preview drawn
#      again without Body1, not computed again; Cancel keeps Body1 hidden
#      (the change is the last undo step).
#   3. Extrude again, Body1 hidden meanwhile, OK: one undo step takes the
#      extrusion back and leaves Body1 hidden; the next one shows it.
#   4. Edit of Extrude2 (the timeline rolled back), Body1 hidden meanwhile,
#      Cancel: Body1 stays hidden, the last undo step (no marker move left).
#   5. The same design with a component and the root active: Create Sketch
#      does not show the origin planes, a face of Body2 can be picked; the
#      component's radio button (another browser action) says "Finish the
#      command or the sketch first." during the pick and during Extrude,
#      where the component's light bulb works; the Origin folder's light
#      bulb during the pick shows the planes, and XY can be picked.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-visibility-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-visibility'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'sketch.create' = 'F7' }

# Ui-ExpectNone "text" "description": no line since the mark has the text.
function Ui-ExpectNone([string]$Text, [string]$Description) {
  Ui-Sync
  Start-Sleep -Milliseconds 500
  if ((Ui-LogLines | Select-Object -Skip $script:UiMark) | Where-Object { $_.Contains($Text) }) {
    Ui-Fail "${Description}: '$Text' is in the log"
  }
  Write-Host "ok   $Description"
}

# Last-Line "text": the last line logged that has the text.
function Last-Line([string]$Text) {
  return "$(Ui-LogLines | Where-Object { $_.Contains($Text) } | Select-Object -Last 1)"
}

# Extrude, which takes the newest free profile (the square): its preview
# computed with both blocks drawn.
function Extrude-Square {
  Ui-Mark
  Ui-Step 'extrude (E)'                  { Ui-Key 'e' }
  Ui-ExpectNew 'Command Extrude started' 'Extrude started'
  Ui-ExpectNew 'Preview Extrude: ok' 'the square''s preview computed'
  Ui-ExpectNew 'Preview bodies: ' 'the preview drawn'
  if (-not (Last-Line 'Preview bodies: ').Contains('F2.b0 at')) { Ui-Fail 'the preview does not draw Body1' }
  Write-Host 'ok   the preview draws Body1'
}

# Body1's light bulb during a command: the preview drawn again without it,
# not computed again.
function Hide-Body1([string]$Command) {
  Ui-Mark
  Ui-Step 'hide Body1'                   { Ui-ClickLogged 'Browser eye Root/Bodies/Body1' }
  Ui-ExpectNew "Visibility Root/Bodies/Body1: hidden (during $Command)" "Body1 hidden during $Command"
  Ui-ExpectNew 'Preview bodies: ' 'the preview drawn again'
  $drawn = Last-Line 'Preview bodies: '
  if ($drawn.Contains('F2.b0 at')) { Ui-Fail "the preview still draws Body1: $drawn" }
  if (-not $drawn.Contains('F4.b0 at')) { Ui-Fail "the preview lost Body2: $drawn" }
  Write-Host 'ok   the preview without Body1, with Body2'
  Ui-ExpectNone "Preview ${Command}: " 'the preview not computed again'
}

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
# Sketch1 (F1) 20 x 20 at (40, -60), Extrude1 (F2) 10 up: Body1 (F2.b0).
# Sketch2 (F3) 60 x 60 around it, Extrude2 (F4) 30 up: Body2 (F4.b0), Body1
# inside it. Sketch3 (F5) 20 x 20 at (100, -140): a profile in front of the
# blocks as the view looks at them, away from the origin planes.
$blocks = @'
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [40, -60], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [20, -80], "width": 60, "height": 60},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 30}, "operation": "new_body"}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F5", "corner": [100, -140], "width": 20, "height": 20}
'@
# The same with Part:1 (O1, C1) beside them: Sketch4 (F6), Extrude3 (F7);
# the root active again.
$part = @'
  {"cmd": "create_component", "name": "Part", "transform": {"translation": [0, 120, 0]}},
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F6", "corner": [0, 0], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F6", "region": "RECT"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "activate_component", "component": "Root"}
'@
$rect = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
$design = Join-Path $work 'blocks.mitcad'
$parts = Join-Path $work 'parts.mitcad'
foreach ($made in @(@{ Script = "[`n$blocks`n]"; File = $design }, @{ Script = "[`n$blocks,`n$part`n]"; File = $parts })) {
  $script = [IO.Path]::ChangeExtension($made.File, '.json')
  Set-Content -Encoding ascii $script ($made.Script -creplace 'RECT', $rect)
  $output = & $cli run $script --save $made.File
  if ($LASTEXITCODE -ne 0 -or -not (Test-Path $made.File)) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }
}

Ui-StartApp 'visibility' @('--open', $design)
Ui-ExpectLog 'blocks.mitcad' 'the design opened'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-ExpectLog 'Bodies shown: F2.b0, F4.b0' 'both blocks shown'

Write-Host '--- Create Sketch: the origin planes; Body2 hidden while a plane is asked for'
Ui-Mark
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Create Sketch: select a plane' 'Create Sketch asks for a plane'
Ui-ExpectNew 'Create Sketch: origin planes shown' 'no components: the origin planes shown (mitcad#294)'
Ui-ExpectNew 'Datum xy at' 'the XY plane can be picked'
Ui-ExpectNew 'Pick places:' 'the faces'' pick places logged'
Ui-ExpectNone 'Pick face F2.b0/' 'Body1, inside Body2, cannot be picked'
Ui-Mark
Ui-Step 'hide Body2'                     { Ui-ClickLogged 'Browser eye Root/Bodies/Body2' }
Ui-ExpectNew 'Visibility Root/Bodies/Body2: hidden (during Create Sketch)' 'Body2 hidden during the plane pick'
Ui-ExpectNew 'Pick face F2.b0/F2:end(' 'Body1''s top face can be picked now'
Ui-ExpectNone 'Finish the command or the sketch first.' 'the light bulb was not refused'
Ui-Step 'pick Body1''s top face'         { Ui-ClickPick 'face F2.b0/F2:end(' }
Ui-ExpectNew 'Sketch started on {"body":"F2.b0","face":"F2:end(' 'the sketch started on Body1''s top face'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'the empty sketch finished'
Ui-Mark
Ui-Step 'show Body2'                     { Ui-ClickLogged 'Browser eye Root/Bodies/Body2' }
Ui-ExpectNew 'Visibility Root/Bodies/Body2: shown' 'Body2 shown again'
Ui-ExpectNone '(during' 'no command runs'

Write-Host '--- Extrude, Body1 hidden meanwhile, Cancel'
Extrude-Square
Hide-Body1 'Extrude'
Ui-Mark
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'Extrude cancelled'
Ui-ExpectNone 'Bodies shown: F2.b0' 'Body1 stays hidden'
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Hide Body1' 'the light bulb is the last undo step, kept by Cancel'
Ui-ExpectNew 'Bodies shown: F2.b0, F4.b0' 'Body1 shown by the undo'

Write-Host '--- Extrude, Body1 hidden meanwhile, OK: one undo step for the extrusion'
Extrude-Square
Hide-Body1 'Extrude'
Ui-Mark
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Added Extrude as F' 'Extrude3 added'
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Add Extrude3' 'one undo step takes the extrusion back'
Ui-ExpectNone 'Bodies shown: F2.b0' 'Body1 stays hidden after that undo'
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Hide Body1' 'the light bulb is an undo step of its own'
Ui-ExpectNew 'Bodies shown: F2.b0, F4.b0' 'Body1 shown by it'

Write-Host '--- Edit of Extrude2 (rolled back), Body1 hidden meanwhile, Cancel'
Ui-ExpectLog 'Timeline Extrude2 at' 'Extrude2 in the timeline'
Ui-Mark
Ui-Step 'edit Extrude2 (double-click)'   { Ui-DoubleClickLogged 'Timeline Extrude2' }
Ui-ExpectNew 'Editing F4 with Extrude' 'editing Extrude2'
Ui-ExpectNew 'Bodies shown: F2.b0' 'the timeline rolled back to before Body2'
Ui-Mark
Ui-Step 'hide Body1'                     { Ui-ClickLogged 'Browser eye Root/Bodies/Body1' }
Ui-ExpectNew 'Visibility Root/Bodies/Body1: hidden (during Extrude)' 'Body1 hidden during the edit'
Ui-Mark
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude: 1 visibility change(s) kept' 'the change run again after the roll-back'
Ui-ExpectNew 'Command Extrude cancelled' 'the edit cancelled'
Ui-ExpectNew 'Bodies shown: F4.b0' 'Body2 back, Body1 hidden'
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Hide Body1' 'the light bulb is the last undo step, no marker move left'
Ui-ExpectNew 'Bodies shown: F2.b0, F4.b0' 'Body1 shown by the undo'
if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }
Ui-StopApp

Write-Host '--- A design with a component: no origin planes by themselves (mitcad#294)'
Ui-StartApp 'visibility-parts' @('--open', $parts)
Ui-ExpectLog 'parts.mitcad' 'the design opened'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-ExpectLog 'Browser radio Root/Part:1 at' 'Part:1 in the browser'
Ui-Mark
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Create Sketch: select a plane' 'Create Sketch asks for a plane'
Ui-ExpectNew 'Create Sketch: origin planes not shown (the design has components)' 'the origin planes not shown'
Ui-ExpectNew 'Pick face F4.b0/F4:end(' 'Body2''s top face can be picked'
Ui-ExpectNone 'Datum xy at' 'the XY plane is not in the view'
Ui-Mark
Ui-Step 'activate Part (radio)'          { Ui-ClickLogged 'Browser radio Root/Part:1' }
Ui-ExpectNew 'Finish the command or the sketch first.' 'other browser actions wait for the plane pick'
Ui-ExpectNone 'Activated Part' 'Part not activated'
Ui-Step 'pick Body2''s top face'         { Ui-ClickPick 'face F4.b0/F4:end(' }
Ui-ExpectNew 'Sketch started on {"body":"F4.b0","face":"F4:end(' 'the sketch started on Body2''s top face'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'the empty sketch finished'

Write-Host '--- The Origin folder''s light bulb during the pick'
Ui-Mark
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Create Sketch: origin planes not shown (the design has components)' 'the origin planes not shown'
Ui-Mark
Ui-Step 'show the origin'                { Ui-ClickLogged 'Browser eye Root/Origin' }
Ui-ExpectNew 'Visibility Root/Origin: shown (during Create Sketch)' 'the Origin folder shown during the pick'
Ui-ExpectNew 'Datum xy at' 'the XY plane in the view'
Ui-Step 'pick XY'                        { Ui-ClickLogged 'Datum xy' }
Ui-ExpectNew 'Sketch started on xy' 'the sketch started on the origin''s XY plane'
Ui-Mark
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectNew 'Sketch finished' 'the empty sketch finished'

Write-Host '--- During Extrude: a component''s light bulb works, other browser actions wait'
Ui-Mark
Ui-Step 'extrude (E)'                    { Ui-Key 'e' }
Ui-ExpectNew 'Command Extrude started' 'Extrude started'
Ui-ExpectNew 'Preview Extrude: ok' 'the square''s preview computed'
Ui-Mark
Ui-Step 'hide Part:1'                    { Ui-ClickLogged 'Browser eye Root/Part:1' }
Ui-ExpectNew 'Visibility Root/Part:1: hidden (during Extrude)' 'Part:1 hidden during Extrude'
Ui-Mark
Ui-Step 'activate Part (radio)'          { Ui-ClickLogged 'Browser radio Root/Part:1' }
Ui-ExpectNew 'Finish the command or the sketch first.' 'other browser actions wait for the command'
Ui-ExpectNone 'Activated Part' 'Part not activated'
Ui-Mark
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Extrude cancelled' 'Extrude cancelled'
if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }

Ui-Finish 'UI Windows visibility test'
