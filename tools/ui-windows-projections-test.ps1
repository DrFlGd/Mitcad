# SPDX-License-Identifier: MIT
# check-all sources: app/commands/SketchCommands.cpp app/framework/CommandSession.cpp app/sketch
# check-all sources: core/model/src/sketch core/model/src/api/sketch.rs
# Input-driven Windows UI test of Clean Up Projections (mitcad#232) in the
# build VM (tools/ui-windows-lib.ps1 says how the input is posted):
#   1. A 40 x 25 block 20 high; a sketch on its front face.
#   2. Project (P) the front face twice, linked: two projections of the same
#      four edges.
#   3. Clean Up Projections (a shortcut of the test's): the panel's preview
#      lists the repeated curves it removes; OK keeps it, and running it
#      again has nothing to clean up.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-projections-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-projections'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'sketch.create' = 'F7'; 'sketch.clean_up_projections' = 'F8' }

Write-Host '--- A block and a sketch on its front face'
Ui-StartApp 'projections'
Ui-Step 'create sketch on XY (F7)'       { Ui-CreateSketch 'F7' 'xy' }
Ui-Step 'rectangle tool (R)'             { Ui-Key 'r' }
Ui-Step 'first corner at the origin'     { Ui-SketchClick 0 0 }
Ui-Step 'width 40 (Tab) height 25'       { Ui-Type '40'; Ui-Key 'Tab'; Ui-Type '25' }
Ui-Step 'confirm (Enter)'                { Ui-Key 'Return' }
Ui-ExpectLog 'Added rectangle 40 x 25 mm at (0, 0)' 'rectangle 40 x 25 at the origin'
Ui-Step 'finish sketch (Ctrl+Enter)'     { Ui-Key 'ctrl+Return' }
Ui-ExpectLog 'Sketch finished' 'sketch finished'

Ui-Mark
Ui-Step 'extrude (E)'                    { Ui-Key 'e' }
Ui-ExpectNew 'Command Extrude started' 'Extrude started with the new profile'
Ui-Step 'type the distance 20'           { Ui-Type '20' }
Ui-Step 'confirm (Enter)'                { Ui-Key 'Return' }
Ui-ExpectNew 'Added extrude (New Body), distance 20' 'extrusion built'

Ui-Mark
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-ExpectNew 'Pick face F2.b0/F2:side(c1[c4,c2])' 'the front face can be picked'
Ui-Step 'pick the front face'            { Ui-ClickPick 'face F2.b0/F2:side(c1[c4,c2])' }
Ui-ExpectNew 'Sketch started on {"body":"F2.b0","face":"F2:side(c1[c4,c2])' 'a sketch on the front face'

Write-Host '--- The front face projected twice'
foreach ($time in @('once', 'twice')) {
  Ui-Mark
  Ui-Step "project (P), $time"           { Ui-Key 'p' }
  Ui-ExpectNew 'Pick face F2.b0/F2:side(c1[c4,c2])' 'the front face can be picked'
  Ui-Step 'pick the front face'          { Ui-ClickPick 'face F2.b0/F2:side(c1[c4,c2])' }
  Ui-ExpectNew 'Preview Project: ok' 'projection previewed'
  Ui-Step 'OK (Enter)'                   { Ui-Key 'Return' }
  Ui-ExpectNew 'Projected 1 item(s)' "projected $time"
}

Write-Host '--- Clean Up Projections'
Ui-Mark
Ui-Step 'clean up projections (F8)'      { Ui-Key 'F8' }
Ui-ExpectNew 'Preview Clean Up Projections: ok, warning: curves the same as another removed: ' `
  'the preview lists the repeated curves'
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Cleaned up projections' 'kept'

Ui-Mark
Ui-Step 'clean up projections again (F8)' { Ui-Key 'F8' }
Ui-ExpectNew 'nothing to clean up' 'nothing left to clean up'
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }

Ui-Finish 'UI Windows projections test'
