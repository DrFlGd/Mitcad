# SPDX-License-Identifier: MIT
# check-all sources: app/browser app/framework app/view tools/cli
# Input-driven Windows UI test of the Browser's Rows background in the
# Floating layout (mitcad#419) in the build VM (tools/ui-windows-lib.ps1 says
# how the input is posted); the Linux test is tools/ui-browser-rows-test.sh:
#   1. A 200 x 200 mm plate (mitcad-cli), seen from the top and zoomed in
#      until it fills the view; the Browser starts on its card.
#   2. Preferences > Display > Browser background: Rows (Alt+B, Down, OK):
#      applied at once and saved; a place beside the rows is logged.
#   3. A click there selects the plate's face behind the rows; a click on
#      Body1's row still selects the body; a middle drag beside the rows
#      orbits (the middle-orbit navigation scheme).
#   4. Back to Card: the place is gone and the plate is not picked there.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-browser-rows-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-browser-rows'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'tools.preferences.display' = 'F8'; 'view.top' = 'F7' }
# The middle button orbits (the drag beside the rows).
$ini = Join-Path $script:UiSettings 'Mitcad\Mitcad.ini'
Add-Content -Encoding ascii $ini @('[view]', 'navigationScheme=middle-orbit')

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
# A 200 x 200 x 10 mm plate centred on the origin: Body1 (F2.b0).
$script = Join-Path $work 'plate.json'
$design = Join-Path $work 'plate.mitcad'
Set-Content -Encoding ascii $script @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-100, -100], "width": 200, "height": 200},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}}
]
'@
$output = & $cli run $script --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }

# Set-Background card|rows Up|Down: Preferences > Display (F8), the Browser
# background one entry up or down (Alt+B), OK.
function Set-Background([string]$Background, [string]$Key) {
  Ui-Mark
  Ui-Step 'Preferences: Display (F8)'       { Ui-Key 'F8' }
  Ui-ExpectNew 'Preferences opened: Display, ' 'Preferences on its Display page'
  Ui-FocusDialog '^Preferences$'
  Ui-Step 'the Browser background (Alt+B)'  { Ui-Key 'alt+b' }
  Ui-Step "Browser background: $Background" { Ui-Key $Key }
  Ui-Step 'OK (Enter)'                      { Ui-Key 'Return' }
  Ui-FocusMain
  Ui-ExpectNew "Preferences: browser background $Background" 'the choice is saved'
  Ui-ExpectNew "Browser background: $Background" 'the Browser follows at once'
  if (-not (Select-String -Quiet -Path $ini -Pattern "^browserBackground=$Background$")) {
    Ui-Fail "appearance/browserBackground is not $Background in the settings"
  }
  Write-Host "ok   the setting is $Background"
}

# Since-Mark "text": the lines logged since the mark that contain the text.
function Since-Mark([string]$Text) {
  return @(Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.Contains($Text) })
}

Ui-StartApp 'browser-rows' @('--open', $design, '--chrome=floating')
Ui-ExpectLog 'plate.mitcad' 'the plate is open'
Ui-ExpectLog 'Browser background: card' 'the Browser starts on its card'
Ui-ExpectLog 'Browser Root/Bodies/Body1 at' "Body1's row"

Write-Host '--- The plate fills the view'
Ui-Step 'top view (F7)'                     { Ui-Key 'F7' }
Ui-Step 'fit (F6)'                          { Ui-Key 'F6' }
# Zoomed in about the plate's middle (the body's logged place), which stays
# put: the plate grows on every side and fills the view, also behind the
# Browser. About the view's middle it grew mostly away from the Browser:
# the plate sits in the free area right of the Browser's card, its left
# edge near the view's middle, and stopped short of the rows.
$plate = Ui-LoggedAt 'Body F2.b0'
Ui-Step 'zoom in at the plate''s middle' {
  [UiInput]::Move($script:UiWindow, $plate[0], $plate[1], 0)
  for ($i = 0; $i -lt 14; $i++) {
    Start-Sleep -Milliseconds 60
    [UiInput]::Wheel($script:UiWindow, $plate[0], $plate[1], 1)
  }
}
Ui-WaitIdle

Write-Host '--- Rows'
Set-Background 'rows' 'Down'
Ui-ExpectLog 'Browser view beside the rows at' 'a place beside the rows is logged'
$beside = Ui-LoggedAt 'Browser view beside the rows'
Ui-Mark
Ui-Step 'click beside the rows'             { Ui-ClickLogged 'Browser view beside the rows' }
Ui-ExpectNew 'Selected: 1 face [face F2:' "the click reached the view: the plate's face behind the rows"
Ui-Mark
Ui-Step "click Body1's row"                 { Ui-ClickLogged 'Browser Root/Bodies/Body1' }
Ui-ExpectNew 'Selected: 1 body [body F2.b0]' "a row's box still takes the click"

$beside = Ui-LoggedAt 'Browser view beside the rows'
Ui-Mark
Ui-Step 'middle drag beside the rows' { Ui-Drag $beside[0] $beside[1] ($beside[0] + 120) ($beside[1] + 64) 2 }
Ui-ExpectNew 'Camera direction' 'a middle drag beside the rows orbits' 8
$last = "$(Ui-LogLines | Where-Object { $_.Contains('Camera direction') } | Select-Object -Last 1)"
if ($last.Contains('direction 0 0 -1 ')) { Ui-Fail "the drag did not turn the view: $last" }
Write-Host "ok   orbited to: $last"
if ((Since-Mark 'Selected: ').Count -ne 0) { Ui-Fail 'the drag selected something' }
Write-Host 'ok   the drag selected nothing'

Write-Host '--- Back to Card'
Set-Background 'card' 'Up'
Ui-ExpectNew 'Browser view beside the rows out of sight' 'the place beside the rows is gone'
Ui-Mark
Ui-Step 'click where the rows left room'    { Ui-Click $beside[0] $beside[1] }
# The card's tree may select the row there; the plate behind is not picked.
if ((Since-Mark 'Selected: 1 face').Count -ne 0) { Ui-Fail 'the click went past the card to the view' }
Write-Host 'ok   the card takes the click again'

Ui-Finish 'Windows UI browser rows test'
