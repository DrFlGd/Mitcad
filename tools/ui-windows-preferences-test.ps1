# SPDX-License-Identifier: MIT
# check-all sources: app/view/ViewController.cpp app/files/RemotePreferences.cpp app/files/RemotePreferences.hpp
# check-all sources: app/files/Print3d.cpp core/vcs/src/remote/cli.rs
# Input-driven Windows UI test of Preferences' size (mitcad#690) in the
# build VM (tools/ui-windows-lib.ps1 says how the input is posted), with git
# and without it, as tools/ui-preferences-test.sh does on Linux:
#   1. git found: the Cloud page names it.
#   2. MITCAD_GIT naming a file that is not there: the Cloud page says so.
#   3. No git on PATH nor where Git for Windows installs itself (git not
#      installed): the Cloud page's long message, which once made the
#      window wider than the screen, wraps.
# Each time Preferences opens on its Cloud page: the size it asks for (its
# largest page's) and its minimum size fit a 1366 x 768 screen with a title
# bar and a taskbar, it is resizable, its frame lies within the screen's
# available area, and the page list is as wide as its entries; on its
# General page (Home in the page list) the minimum is that page's, and it
# fits too.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-preferences-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-preferences'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

# Popups close at once in session 0: the Cloud page by a key of the test's
# own.
Ui-Init $App $Out $QtBin @{ 'tools.preferences.cloud' = 'F8' }

$work = Join-Path $script:UiOut 'preferences'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
Remove-Item Env:MITCAD_GIT -ErrorAction SilentlyContinue

# Since-Match "regex": the match of the last line logged since the mark.
function Since-Match([string]$Pattern) {
  $lines = @(Ui-LogLines | Select-Object -Skip $script:UiMark)
  $found = $null
  foreach ($line in $lines) {
    $match = [regex]::Match($line, $Pattern)
    if ($match.Success) { $found = $match }
  }
  return $found
}

# Fits what width height: within 1366 x 690, what a 1366 x 768 screen shows
# below a title bar and above a taskbar (mitcad#25).
function Fits([string]$What, [int]$Width, [int]$Height) {
  if ($Width -le 0 -or $Width -gt 1366 -or $Height -le 0 -or $Height -gt 690) {
    Ui-Fail "Preferences: $What $Width x $Height, more than a 1366 x 768 screen shows"
  }
  Write-Host "ok   Preferences: $What $Width x $Height fits a 1366 x 768 screen"
}

# Expect-Cloud "regex" "description": the Cloud page's line about the git
# program ("Preferences: cloud: <text>") since the mark matches.
function Expect-Cloud([string]$Pattern, [string]$Description) {
  for ($i = 0; $i -lt 30; $i++) {
    $match = Since-Match '^Preferences: cloud: (.*)$'
    if ($match) {
      $text = $match.Groups[1].Value
      if ($text -notmatch "^$Pattern") { Ui-Fail "${Description}: the Cloud page says '$text'" }
      Write-Host "ok   ${Description}: $text"
      return
    }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "${Description}: the Cloud page's git line is not in the log"
}

# Preferences-Fit "Cloud page regex" "description": Preferences opened on
# its Cloud page (F8), the Cloud page's text about git, the window's sizes
# and frame, then its General page, then closed.
function Preferences-Fit([string]$Pattern, [string]$Description) {
  Ui-Mark
  Ui-Step 'Preferences: Cloud (F8)'        { Ui-Key 'F8' }
  Ui-ExpectNew 'Preferences opened: Cloud, ' 'Preferences on its Cloud page'
  Expect-Cloud $Pattern $Description
  Ui-ExpectNew 'Preferences page list ' 'Preferences shown'
  $asked = Since-Match '^Preferences opened: Cloud, (\d+) x (\d+)$'
  if (-not $asked) { Ui-Fail 'the size Preferences asks for is not logged' }
  Fits 'the size asked for' ([int]$asked.Groups[1].Value) ([int]$asked.Groups[2].Value)
  $minimum = Since-Match '^Preferences minimum (\d+) x (\d+), ([a-z]+)$'
  if (-not $minimum) { Ui-Fail "Preferences' minimum size is not logged" }
  Fits 'the minimum size' ([int]$minimum.Groups[1].Value) ([int]$minimum.Groups[2].Value)
  if ($minimum.Groups[3].Value -ne 'resizable') { Ui-Fail "Preferences cannot be resized ($($minimum.Groups[3].Value))" }
  Write-Host 'ok   Preferences can be resized'
  $frame = Since-Match '^Preferences frame (\d+) x (\d+) at (-?\d+),(-?\d+) on a screen of (\d+) x (\d+) at (-?\d+),(-?\d+)$'
  if (-not $frame) { Ui-Fail "Preferences' frame is not logged" }
  $v = @(1..8 | ForEach-Object { [int]$frame.Groups[$_].Value })
  $width = $v[0]; $height = $v[1]; $x = $v[2]; $y = $v[3]
  $screenWidth = $v[4]; $screenHeight = $v[5]; $screenX = $v[6]; $screenY = $v[7]
  if ($x -lt $screenX -or $y -lt $screenY -or $x + $width -gt $screenX + $screenWidth -or
      $y + $height -gt $screenY + $screenHeight) {
    Ui-Fail ("Preferences' frame $width x $height at $x,$y is not within the screen's " +
             "$screenWidth x $screenHeight at $screenX,$screenY")
  }
  if ($width -gt 1366) { Ui-Fail "Preferences' frame is $width wide, more than a 1366 x 768 screen" }
  Write-Host "ok   Preferences' frame $width x $height at $x,$y lies within the screen ($screenWidth x $screenHeight)"
  $list = Since-Match '^Preferences page list (\d+) wide, its entries (\d+)$'
  if (-not $list -or [int]$list.Groups[2].Value -le 0 -or [int]$list.Groups[1].Value -lt [int]$list.Groups[2].Value) {
    Ui-Fail "the page list does not show its entries whole: $(if ($list) { $list.Value } else { 'not logged' })"
  }
  Write-Host "ok   the page list ($($list.Groups[1].Value)) shows its entries whole ($($list.Groups[2].Value))"
  Ui-FocusDialog '^Preferences$'
  Ui-Mark
  Ui-Step 'the General page (Home)'        { Ui-Key 'Home' }
  Ui-ExpectNew 'Preferences page: General, minimum ' 'Preferences on its General page'
  $general = Since-Match '^Preferences page: General, minimum (\d+) x (\d+)$'
  Fits "the General page's minimum size" ([int]$general.Groups[1].Value) ([int]$general.Groups[2].Value)
  Ui-Step 'close it (Escape)'              { Ui-Key 'Escape' }
  Ui-FocusMain
}

Write-Host '--- 1. git found'
Ui-StartApp 'preferences-git'
Preferences-Fit 'git [0-9][^ ]* at ' 'the Cloud page names the git program found'

Write-Host '--- 2. MITCAD_GIT naming a file that is not there'
$env:MITCAD_GIT = Join-Path $work 'no-such-git.exe'
Ui-StartApp 'preferences-mitcad-git'
Remove-Item Env:MITCAD_GIT
Preferences-Fit 'the git program set for Mitcad \(MITCAD_GIT\) is unavailable: ' 'the Cloud page says it is not there'

Write-Host '--- 3. No git on PATH nor where Git for Windows installs itself'
# Mitcad started with a PATH of only the folders without git.exe, and the
# folders where Git for Windows installs itself pointing at an empty one;
# the test's own environment back as it was once it runs.
$saved = @{}
foreach ($name in @('Path', 'ProgramFiles', 'ProgramW6432', 'LOCALAPPDATA')) {
  $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
$empty = Join-Path $work 'no-git-here'
New-Item -ItemType Directory -Force $empty | Out-Null
$env:Path = (@($env:Path -split ';' | Where-Object { $_ -and -not (Test-Path (Join-Path $_ 'git.exe')) })) -join ';'
$env:ProgramFiles = $empty
$env:ProgramW6432 = $empty
$env:LOCALAPPDATA = $empty
try {
  Ui-StartApp 'preferences-no-git'
} finally {
  foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
Preferences-Fit 'git is not installed: ' 'the Cloud page says git is not installed'

Ui-Finish 'UI Windows preferences test'
