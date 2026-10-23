# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindow.cpp app/browser app/framework tools/cli
# Input-driven Windows UI test of the Docked layout's window height
# (mitcad#560) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted), on a design mitcad-cli makes: a 10 mm cube and a rectangular
# pattern of it, 6 x 5 (30 bodies: Body1 F2.b0, the copies F3.b0 to F3.b28).
#   1. The design opens folded (its rows do not fit the Browser); the
#      window's size, the view's area and the timeline's cell are noted.
#   2. Root/Bodies opened: the 30 body rows scroll (Browser scroll range
#      above 0); the window keeps its size ("Window layout:", which the
#      window logs again whenever its size changes), the view its area, the
#      timeline's cell its place.
#   3. A click on the timeline's RectangularPattern1 selects its 29 bodies:
#      the command panel lists the first 8 with their measurements and
#      scrolls them; the window, the view and the cell stay where they were
#      (in session 0's 1024x768 desktop the window grew from 749 to 967
#      pixels, the cell went below the screen).
#   4. A plain double-click on the cell edits the pattern; cancelled, the
#      window still has its size.
# The Linux counterpart is tools/ui-docked-height-test.sh.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-docked-height-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-docked-height'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null

$modelScript = Join-Path $work 'model.json'
$design = Join-Path $work 'docked-height.mitcad'
Set-Content -Encoding ascii $modelScript @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 10, "height": 10},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "rectangular_pattern",
    "objects": {"type": "bodies", "bodies": ["F2.b0"]},
    "direction1": {"axis": {"type": "origin", "axis": "x"}, "quantity": 6, "distance": 20},
    "direction2": {"axis": {"type": "origin", "axis": "y"}, "quantity": 5, "distance": 20},
    "distance_type": "spacing"}},
  {"expect": {"bodies": 30}}
]
'@
$made = & $cli run $modelScript --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

# Window-Size: "WxH" of the last "Window layout:" line.
function Window-Size { return Ui-LastMatch '^Window layout: window (\d+x\d+),' }
# View-Area: "x y w h" of the last "View area" line.
function View-Area { return Ui-LastMatch '^View area (\d+ \d+ \d+ \d+)$' }
# Scroll-Range: the Browser rows' last logged vertical scroll range.
function Scroll-Range { return [int](Ui-LastMatch '^Browser scroll range (\d+)$') }

# Expect-Unmoved "description": once the app has settled, the window has
# the size, the view the area and the timeline's cell the place noted at
# the start.
function Expect-Unmoved([string]$Description) {
  Ui-WaitIdle
  Start-Sleep -Milliseconds 500
  Ui-Sync
  $size = Window-Size
  if ($size -ne $script:Size0) { Ui-Fail "${Description}: the window is $size, not $($script:Size0)" }
  $view = View-Area
  if ($view -ne $script:View0) { Ui-Fail "${Description}: the view area is $view, not $($script:View0)" }
  $cell = Ui-LoggedAt 'Timeline RectangularPattern1'
  if ($cell[0] -ne $script:Cell0[0] -or $cell[1] -ne $script:Cell0[1]) {
    Ui-Fail "${Description}: the timeline's cell is at $($cell -join ','), not $($script:Cell0 -join ',')"
  }
  Write-Host "ok   ${Description}: the window $size, the view $view, the cell at $($cell -join ',')"
}

# Expect-Scrolls "description": the Browser's rows overflow and scroll.
function Expect-Scrolls([string]$Description) {
  for ($i = 0; $i -lt 25; $i++) {
    if ((Scroll-Range) -gt 0) { break }
    Start-Sleep -Milliseconds 200
  }
  if ((Scroll-Range) -le 0) { Ui-Fail "${Description}: the Browser's rows do not scroll" }
  Write-Host "ok   ${Description} (Browser scroll range $(Scroll-Range))"
}

Write-Host '--- 1. The design opens folded'
Ui-StartApp 'docked-height' @('--open', $design)
Ui-ExpectLog 'docked-height.mitcad' 'the design opened'
Ui-ExpectLog 'Window layout: window ' 'the window logged its layout'
Ui-ExpectLog 'Browser opened folded: ' 'the 30 bodies do not fit: the design opens folded'
Ui-ExpectLog 'Browser arrow Root/Bodies at' "the Bodies folder's arrow"
Ui-ExpectLog 'Timeline RectangularPattern1 at' "the pattern's cell"
Ui-WaitIdle
Ui-Sync
$script:Size0 = Window-Size
$script:View0 = View-Area
$script:Cell0 = Ui-LoggedAt 'Timeline RectangularPattern1'
Write-Host "ok   at the start: the window $($script:Size0), the view $($script:View0), the cell at $($script:Cell0 -join ',')"

Write-Host '--- 2. Root/Bodies opened: the rows scroll, the window keeps its size'
Ui-Mark
Ui-Step 'open Bodies'                       { Ui-ClickLogged 'Browser arrow Root/Bodies' }
Ui-ExpectNew 'Browser Root/Bodies/Body1 at' "Body1's row"
Expect-Scrolls 'the 30 body rows scroll'
Expect-Unmoved 'Bodies open'

Write-Host '--- 3. The pattern''s 29 bodies selected from the timeline'
Ui-Mark
Ui-Step 'click RectangularPattern1'         { Ui-ClickLogged 'Timeline RectangularPattern1' }
Ui-ExpectNew 'Selected: 29 bodies' "the pattern's bodies selected"
Expect-Scrolls 'the rows still scroll'
Expect-Unmoved 'the bodies selected'

Write-Host '--- 4. A plain double-click edits the pattern'
Ui-Mark
Ui-Step 'double-click RectangularPattern1'  { Ui-DoubleClickLogged 'Timeline RectangularPattern1' }
Ui-ExpectNew 'Editing F3 with Rectangular Pattern' 'editing the pattern'
Ui-ExpectNew 'Preview Rectangular Pattern: ok' 'previewed' 30
Ui-Mark
Ui-Step 'cancel (Esc)'                      { Ui-Key 'Escape' }
Ui-ExpectNew 'Command Rectangular Pattern cancelled' 'left as it was'
Ui-WaitIdle
Ui-Sync
if ((Window-Size) -ne $script:Size0) { Ui-Fail "after the edit the window is $(Window-Size), not $($script:Size0)" }
Write-Host "ok   after the edit the window is still $($script:Size0)"

Ui-Finish 'UI Windows docked height test'
