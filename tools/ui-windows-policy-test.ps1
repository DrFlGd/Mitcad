# SPDX-License-Identifier: MIT
# check-all sources: app/browser/PolicyDialog.cpp app/browser/BrowserController.cpp app/MainWindow.cpp
# check-all sources: core/model/src/policy.rs core/model/src/document/policy.rs core/model/src/api/policy.rs
# Input-driven Windows UI test of the design policy (mitcad#220) in the
# build VM (tools/ui-windows-lib.ps1 says how the input is posted):
#   1. A new design has Part1 under the root, active: the first sketch and
#      the extrusion of a 40 x 25 rectangle go into Part1.
#   2. The root (Assembly) activated: Create Sketch is refused with the
#      reason, nothing is redirected.
#   3. Design Policy (a shortcut of the test's) of the root: Assembly with
#      no violations; the Part preset (Alt+P) counts Part1:1 against it; OK
#      sets it, Create Sketch then asks for a plane; Undo takes the policy
#      back and the root refuses sketches again.
#   4. New Component Part2 (a part) in the root: Create Sketch there leaves
#      Part1's faces out of the picks and names Part1 in the hint. With
#      Part1 taking attachments from others (Alt+H) and Part2 attaching to
#      others (Alt+O), Part1's faces can be picked, and the sketch goes
#      into Part2 on it.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-policy-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-policy'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'sketch.create' = 'F7'; 'assembly.policy' = 'F8'; 'solid.new_component' = 'F9' }

# Ui-ExpectNone "text" "description": no line since the mark has the text.
function Ui-ExpectNone([string]$Text, [string]$Description) {
  if ((Ui-LogLines | Select-Object -Skip $script:UiMark) | Where-Object { $_.Contains($Text) }) {
    Ui-Fail "${Description}: '$Text' is in the log"
  }
  Write-Host "ok   $Description"
}

# Design Policy of the active component: opens the dialog, runs the keys
# there and closes it with OK.
function Set-Policy([string]$Name, [string[]]$Keys, [string]$Expected) {
  Ui-Mark
  Ui-Step "design policy of $Name (F8)" { Ui-Key 'F8' }
  Ui-ExpectNew "Design Policy dialog opened for $Name" "the policy of $Name opened"
  Ui-FocusDialog "^Design Policy of $Name$"
  foreach ($key in $Keys) { Ui-Step "press $key" { Ui-Key $key } }
  Ui-Step 'OK (Enter)'                   { Ui-Key 'Return' }
  Ui-FocusMain
  Ui-ExpectNew "Policy of $Name set: $Expected" "the policy of $Name set"
}

Write-Host '--- A new design: Part1 in the root, active'
Ui-StartApp 'policy'
Ui-ExpectLog 'Browser Root/Part1:1 at' 'Part1 is in the browser'
Ui-ExpectLog 'Browser radio Root at' 'the root is not the active component'
Ui-CreateSketch 'F7' 'xy'
Ui-ExpectLog 'Sketch Sketch1 is in Part1' 'the first sketch went into Part1'
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
Ui-ExpectNew 'Added extrude (New Body), distance 20' 'the extrusion built in Part1'

Write-Host '--- The root refuses sketches'
Ui-Mark
Ui-Step 'activate the root'              { Ui-ClickLogged 'Browser radio Root' }
Ui-ExpectNew 'Activated Root' 'the root is active'
Ui-Mark
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Create Sketch refused: Root does not allow sketches' 'refused with the reason'
Ui-ExpectNone 'Create Sketch: select a plane' 'no plane is asked for'
Ui-ExpectNone 'Activated ' 'nothing else is activated'

Write-Host '--- Design Policy of the root'
Ui-Mark
Ui-Step 'design policy (F8)'             { Ui-Key 'F8' }
Ui-ExpectNew 'Policy dialog Root: assembly; sketches blocked (0), features blocked (0), components allowed' 'the root is an assembly'
Ui-FocusDialog '^Design Policy of Root$'
Ui-Step 'Part preset (Alt+P)'            { Ui-Key 'alt+p' }
Ui-ExpectNew 'Policy dialog Root: part; sketches allowed, features allowed, components blocked (1)' 'Part1:1 counted against the Part preset'
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-FocusMain
Ui-ExpectNew 'Policy of Root set: part' 'the root is a part now'
Ui-Mark
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Create Sketch: select a plane' 'the root takes sketches now'
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }
Ui-Mark
Ui-Step 'undo (Ctrl+Z)'                  { Ui-Key 'ctrl+z' }
Ui-ExpectNew 'Undo: Policy of Root' 'the policy change undone'
Ui-Mark
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Create Sketch refused: Root does not allow sketches' 'refused again'

Write-Host '--- Sketch planes of other components'
Ui-Mark
Ui-Step 'new component (F9)'             { Ui-Key 'F9' }
Ui-ExpectNew 'Panel New Component input name at' 'the New Component panel'
Ui-Step 'name it Part2'                  { Ui-TypeIn 'Panel New Component input name' 'Part2' }
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Browser Root/Part2:1 at' 'Part2 in the browser'
Ui-ExpectNew 'Browser radio Root at' 'Part2 is active, not the root'
Ui-Mark
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Create Sketch: policy leaves out Part1' "Part1's faces left out, named"
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Start-Sleep -Milliseconds 500
Ui-ExpectNone 'Pick face F2.b0/' "no face of Part1's body can be picked"
Ui-Step 'cancel (Esc)'                   { Ui-Key 'Escape' }

Ui-Mark
Ui-Step 'activate Part1'                 { Ui-ClickLogged 'Browser radio Root/Part1:1' }
Ui-ExpectNew 'Activated Part1' 'Part1 is active'
Set-Policy 'Part1' @('alt+h') 'custom'
Ui-Mark
Ui-Step 'activate Part2'                 { Ui-ClickLogged 'Browser radio Root/Part2:1' }
Ui-ExpectNew 'Activated Part2' 'Part2 is active'
Set-Policy 'Part2' @('alt+o') 'custom'
Ui-Mark
Ui-Step 'create sketch (F7)'             { Ui-Key 'F7' }
Ui-ExpectNew 'Pick places:' 'the picks are logged'
Ui-ExpectNew 'Pick face F2.b0/F2:side(c1[c4,c2]) in O1 at' "Part1's faces can be picked"
Ui-ExpectNone 'Create Sketch: policy leaves out' 'nothing left out'
Ui-Step "pick a side face of Part1's body" { Ui-ClickPick 'face F2.b0/F2:side(c1[c4,c2])' }
Ui-ExpectNew 'Sketch Sketch2 is in Part2' 'the sketch went into Part2'
Ui-ExpectNew ' in O1, origin ' 'on the face where Part1:1 shows it'

Ui-Finish 'UI Windows policy test'
