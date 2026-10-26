# SPDX-License-Identifier: MIT
# check-all sources: app/browser app/print app/MainWindow.cpp tools/cli
# Input-driven Windows UI test of the browser's scrolling with a selection
# (mitcad#318) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted); the Linux test is the last part of tools/ui-browser-test.sh:
#   1. A design with 60 bodies and Plane1 above them (mitcad-cli): more
#      rows than the browser shows at once, Plane1's at the bottom.
#   2. Plane1 picked in the view: the tree scrolls to its row.
#   3. Body1 clicked at the top, the rows scrolled down with the wheel until
#      Body1 is out of sight, Body60 Ctrl+clicked: both bodies selected and
#      the scroll position stays where it was.
#   4. Five light bulbs clicked quickly with a small mouse move during each
#      press (mitcad#344): each toggles once and the selection is unchanged
#      (the Linux test is a part of tools/ui-browser-test.sh).
#   5. Material... and Appearance... of Body60's row, Body1 and Body60
#      selected, open for both bodies (mitcad#703; the Linux test, which
#      assigns, is tools/ui-material-test.sh): the right click keeps the
#      selection, Assign names the two bodies.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-browser-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-browser'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin

# Place-Valid "Browser <path>": the row's place was logged and has not gone
# out of sight since (BrowserPanel::logLayout).
function Place-Valid([string]$Text) {
  $last = "$(Ui-LogLines | Where-Object { $_.Contains("$Text at ") -or $_.EndsWith("$Text out of sight") } |
    Select-Object -Last 1)"
  return ($last.Length -gt 0) -and -not $last.EndsWith(' out of sight')
}

# Browser-Wheel steps: wheel steps over the last row place the browser
# logged (negative: down the list).
function Browser-Wheel([int]$Steps) {
  $line = "$(Ui-LogLines | Where-Object { $_ -match '^Browser (eye |radio )?Root.* at \d+,\d+$' } | Select-Object -Last 1)"
  $match = [regex]::Match($line, ' at (\d+),(\d+)$')
  if (-not $match.Success) { Ui-Fail 'no browser row in sight' }
  $x = [int]$match.Groups[1].Value
  $y = [int]$match.Groups[2].Value
  [UiInput]::Move($script:UiWindow, $x, $y, 0)
  Start-Sleep -Milliseconds 80
  [UiInput]::Wheel($script:UiWindow, $x, $y, $Steps)
  Ui-Sync
}

# Browser-Reveal "Browser <path>" steps: wheels until the row is in sight.
function Browser-Reveal([string]$Text, [int]$Steps) {
  for ($i = 0; $i -lt 20 -and -not (Place-Valid $Text); $i++) { Browser-Wheel $Steps }
  if (-not (Place-Valid $Text)) { Ui-Fail "'$Text' cannot be scrolled into sight" }
}

# Browser-Scroll: where the rows are scrolled to now.
function Browser-Scroll { return Ui-LastMatch '^Browser scroll (\d+)$' }

$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
# Body1..Body60 (F2..F61, all from Sketch1's rectangle) and Plane1 (F62)
# 60 mm above them.
$rect = 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'
$lines = @('[', '  {"cmd": "sketch.create"},',
  '  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 30, "height": 20},')
for ($i = 1; $i -le 60; $i++) {
  $lines += '  {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": "' + $rect +
    '"}], "extent": {"type": "distance", "distance": ' + ($i % 10 + 1) + '}, "operation": "new_body"}},'
}
$lines += '  {"cmd": "add_feature", "def": {"type": "construction_plane", "definition": {"type": "offset", "plane": "xy", "distance": 60}}}'
$lines += ']'
$script = Join-Path $work 'many.json'
$design = Join-Path $work 'many.mitcad'
Set-Content -Encoding ascii $script $lines
$output = & $cli run $script --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($output -join ' / ')" }

Ui-StartApp 'browser' @('--open', $design)
Ui-ExpectLog 'many.mitcad' 'the design opened'
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }
# A tree longer than the browser opens folded below the root (mitcad#426):
# Construction, then Bodies are opened (Bodies' place stays where it is).
Ui-ExpectLog 'Browser opened folded: ' 'the long tree opened folded'
Ui-Sync
Ui-Step 'open Construction'              { Ui-ClickLogged 'Browser arrow Root/Construction' }
Ui-Step 'open Bodies'                    { Ui-ClickLogged 'Browser arrow Root/Bodies' }
Ui-ExpectLog 'Browser Root/Bodies/Body1 at' 'the rows from the top'
# Plane1's pick place is logged once the camera rests in the laid-out view
# (the one logged when the design opened, before the view had its size, is
# 0,0): the last one is a place in the view.
$plane = $null
for ($i = 0; $i -lt 50; $i++) {
  $plane = Ui-LastMatch '^Datum F62 at (\d+,\d+)$'
  if ($plane -and $plane -ne '0,0') { break }
  Start-Sleep -Milliseconds 200
}
if (-not $plane -or $plane -eq '0,0') { Ui-Fail "Plane1's place in the view is not logged ('$plane')" }
Write-Host "ok   Plane1 in the view at $plane"
if (Place-Valid 'Browser Root/Construction/Plane1') { Ui-Fail 'every row in sight: nothing to scroll' }
Write-Host 'ok   Plane1''s row is below the rows in sight'

Write-Host '--- A pick in the view scrolls the tree to its row'
Ui-Mark
Ui-Step 'pick Plane1 in the view'        { Ui-ClickLogged 'Datum F62' }
Ui-ExpectNew 'Selected: 1 plane [plane F62]' 'Plane1 picked in the view'
Ui-ExpectNew 'Browser scrolled to Root/Construction/Plane1' 'the tree scrolled to its row'
Ui-ExpectNew 'Browser Root/Construction/Plane1 at' 'its row in sight'
Ui-ExpectNew 'Browser selected: Root/Construction/Plane1' 'and selected'

Write-Host '--- Ctrl+click below, the first selected row scrolled away (mitcad#318)'
Browser-Reveal 'Browser Root/Bodies/Body1' 3
Ui-Mark
Ui-Step 'click Body1'                    { Ui-ClickLogged 'Browser Root/Bodies/Body1' }
Ui-ExpectNew 'Selected: 1 body [body F2.b0]' 'Body1 selected'
Write-Host '--- Quick clicks on light bulbs (mitcad#344)'
# Five light bulbs clicked one after the other with no pause, the mouse
# moving a little while each button is down: each toggles its row once and
# the selection stays Body1 (the view's pressed row must not start a range).
function Quick-Eyes([string[]]$Paths) {
  $places = @()
  foreach ($path in $Paths) { $places += , (Ui-LoggedAt "Browser eye $path") }
  foreach ($p in $places) {
    [UiInput]::Move($script:UiWindow, $p[0], $p[1], 0)
    [UiInput]::Button($script:UiWindow, 0x201, $p[0], $p[1], 0x01)
    [UiInput]::Move($script:UiWindow, $p[0] + 2, $p[1] + 1, 0x01)
    [UiInput]::Button($script:UiWindow, 0x202, $p[0] + 2, $p[1] + 1, 0)
  }
  Ui-Sync
}
$quick = @('Root/Bodies/Body2', 'Root/Bodies/Body3', 'Root/Bodies/Body4', 'Root/Bodies/Body5', 'Root/Bodies/Body6')
$selected = Ui-LastMatch '^Browser selected: (.*)$'
Ui-Mark
Ui-Step 'click five light bulbs quickly' { Quick-Eyes $quick }
foreach ($path in $quick) { Ui-ExpectNew "Visibility ${path}: hidden" "$path hidden" }
$new = @(Ui-LogLines | Select-Object -Skip $script:UiMark)
$toggles = @($new | Where-Object { $_.StartsWith('Visibility ') }).Count
if ($toggles -ne 5) { Ui-Fail "five quick clicks gave $toggles visibility changes" }
if ((Ui-LastMatch '^Browser selected: (.*)$') -ne $selected) { Ui-Fail 'the quick clicks changed the selection' }
Write-Host "ok   each light bulb toggled once, the selection is unchanged ($selected)"
Ui-Mark
Ui-Step 'click them again' { Quick-Eyes $quick }
foreach ($path in $quick) { Ui-ExpectNew "Visibility ${path}: shown" "$path shown again" }
if ((Ui-LastMatch '^Browser selected: (.*)$') -ne $selected) { Ui-Fail 'the second round changed the selection' }

Browser-Reveal 'Browser Root/Bodies/Body60' -3
if (Place-Valid 'Browser Root/Bodies/Body1') { Ui-Fail 'Body1''s row is still in sight' }
$before = Browser-Scroll
if (-not $before -or [int]$before -le 0) { Ui-Fail "the rows are not scrolled down ('$before')" }
Write-Host "ok   scrolled down to $before, Body1 out of sight"
$at = Ui-LoggedAt 'Browser Root/Bodies/Body60'
Ui-Mark
Ui-Step 'Ctrl+click Body60'              { Ui-ClickWith 'ctrl' $at[0] $at[1] }
Ui-ExpectNew 'Selected: 2 bodies' 'two bodies selected'
Ui-ExpectNew 'body F2.b0' 'Body1 still selected'
Ui-ExpectNew 'body F61.b0' 'and Body60'
Ui-ExpectNew 'Browser selected: Root/Bodies/Body1, Root/Bodies/Body60' 'both rows selected'
Ui-Sync
$after = Browser-Scroll
if ($after -ne $before) { Ui-Fail "Ctrl+click scrolled the tree from $before to $after" }
if ((Ui-LogLines | Select-Object -Skip $script:UiMark) | Where-Object { $_.Contains('Browser scrolled to') }) {
  Ui-Fail 'the tree scrolled to a row'
}
if (-not (Place-Valid 'Browser Root/Bodies/Body60')) { Ui-Fail 'Body60''s row left the sight' }
Write-Host "ok   the scroll position stays at $after"

Write-Host '--- Material... and Appearance... of a selected row are for every selected body (mitcad#703)'
# Menu-Choose "entry": the entry of the context menu logged last ("Context
# menu: A | B"), chosen with the keys as the menu opens with nothing active
# (the labels' ellipsis matches three dots).
$ellipsis = [string][char]0x2026
function Menu-Choose([string]$Entry) {
  $entries = @("$(Ui-LastMatch '^Context menu: (.*)$')" -split ' \| ' | ForEach-Object { $_.Replace($ellipsis, '...') })
  $index = [array]::IndexOf($entries, $Entry)
  if ($index -lt 0) { Ui-Fail "'$Entry' is not in the context menu: $($entries -join ' | ')" }
  for ($i = 0; $i -le $index; $i++) { Ui-Key 'Down' }
  Ui-Key 'Return'
}
# Expect-NewMatch "regex" "description": Ui-ExpectNew with a regular
# expression (the two bodies in either order).
function Expect-NewMatch([string]$Pattern, [string]$Description) {
  for ($i = 0; $i -lt 30; $i++) {
    if (Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_ -match $Pattern }) {
      Write-Host "ok   $Description"
      return
    }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "${Description}: /$Pattern/ not in the log since the mark"
}
# Changed-Selection: a "Browser selected" line since the mark.
function Changed-Selection {
  return [bool](Ui-LogLines | Select-Object -Skip $script:UiMark | Where-Object { $_.StartsWith('Browser selected: ') })
}
$both = '(F2\.b0, F61\.b0|F61\.b0, F2\.b0)'
Ui-Mark
Ui-Step 'right-click Body60'             { Ui-ClickLogged 'Browser Root/Bodies/Body60' 3 }
Ui-ExpectNew 'Context menu: ' 'the body''s context menu'
Ui-Step 'choose Material...'             { Menu-Choose 'Material...' }
Expect-NewMatch "^Materials opened for bodies $both$" 'the Materials window opened for both selected bodies'
Ui-ExpectNew 'Materials assign label: Assign to 2 bodies' 'Assign names the two bodies'
if (Changed-Selection) { Ui-Fail 'the right click on a selected row changed the selection' }
Write-Host 'ok   the right click kept both rows selected'
Ui-FocusDialog '^Materials$'
Ui-Step 'close the Materials window (Escape)' { Ui-Key 'Escape' }
Ui-FocusMain
Ui-Mark
Ui-Step 'right-click Body60 again'       { Ui-ClickLogged 'Browser Root/Bodies/Body60' 3 }
Ui-ExpectNew 'Context menu: ' 'the body''s context menu'
Ui-Step 'choose Appearance...'           { Menu-Choose 'Appearance...' }
Expect-NewMatch "^Appearances for bodies: $both( \(|$)" 'the Appearances window opened for both selected bodies'
if (Changed-Selection) { Ui-Fail 'the second right click changed the selection' }
Ui-FocusDialog '^Appearances$'
Ui-Step 'close the Appearances window (Escape)' { Ui-Key 'Escape' }
Ui-FocusMain

if (Ui-LogLines | Where-Object { $_.Contains('Recompute failed') }) { Ui-Fail 'a recompute failed' }

Ui-Finish 'UI Windows browser test'
