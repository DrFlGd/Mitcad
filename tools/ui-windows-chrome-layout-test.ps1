# SPDX-License-Identifier: MIT
# check-all sources: app/framework/ChromeStyle.cpp app/framework/ChromeStyle.hpp app/framework/Restart.cpp
# check-all sources: app/framework/Restart.hpp app/framework/FloatingLayout.cpp app/framework/FloatingLayout.hpp
# check-all sources: app/framework/GlassCard.cpp app/framework/TitleBar.cpp app/framework/Ribbon.cpp
# check-all sources: app/main.cpp app/view/ViewController.cpp app/MainWindow.cpp
# Input-driven Windows UI test of the window's layout chosen in Preferences
# (mitcad#419) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted), as tools/ui-chrome-layout-test.sh does on Linux:
#   1. Without a choice Docked; Preferences > Display > Layout Floating asks
#      Restart Mitcad, Later keeps it in the settings for the next start.
#   2. Docked again is the window's layout: saved, no question.
#   3. Floating, Restart Now with unsaved changes: the window asks about
#      them as quitting does; Cancel keeps Mitcad open and nothing restarts.
#   4. Saved, Floating, Restart Now: Mitcad quits and would start again
#      (MITCAD_TEST_NO_RESTART=1 logs the command) with the run's settings
#      options and the design, without --chrome.
#   5. The next start is Floating by the preference; every card starts
#      below the whole top bar (the menu bar, the title bar row, the
#      ribbon's groups), at the default size, in a narrow and a wide window.
#   6. --chrome=docked overrides the preference.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-chrome-layout-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-chrome-layout'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

# The window resized as a user would drag its frame (the outer size).
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class LayoutTestWindow {
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy,
                                                               uint flags);
    const uint SWP_NOMOVE = 0x2, SWP_NOZORDER = 0x4, SWP_NOACTIVATE = 0x10;

    public static bool Resize(IntPtr hwnd, int width, int height) {
        return SetWindowPos(hwnd, IntPtr.Zero, 0, 0, width, height, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}
'@

# Popups close at once in session 0: the Display page by a key of the
# test's own; Create Sketch by its default Alt+S.
Ui-Init $App $Out $QtBin @{ 'tools.preferences.display' = 'F8' }

$work = Join-Path $script:UiOut 'chrome-layout'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$file = Join-Path $work 'part.mitcad'
$fileLogged = $file -replace '\\', '/'
$ini = Join-Path $script:UiSettings 'Mitcad\Mitcad.ini'
# The restart is logged, not started: the test starts the next run itself.
$env:MITCAD_TEST_NO_RESTART = '1'

# Choose-Layout key: Preferences on its Display page (F8), the Layout
# (Alt+L) set with the key (Home: Docked, End: Floating), OK.
function Choose-Layout([string]$Key) {
  Ui-Mark
  Ui-Step 'Preferences: Display (F8)'      { Ui-Key 'F8' }
  Ui-ExpectNew 'Preferences opened: Display, ' 'Preferences on its Display page'
  Ui-FocusDialog '^Preferences$'
  Ui-Step "Layout (Alt+L, $Key)"           { Ui-Key 'alt+l'; Ui-Key $Key }
  Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
  Ui-FocusMain
}

function Has-Line([string]$Text) { return [bool]((Ui-LogLines) | Where-Object { $_.Contains($Text) }) }

# Cards-BelowBar description min max: the last "Floating cards:" line, of a
# window min to max wide (waited for), has the Browser's card, and each
# card's top is at or below the top bar's bottom.
function Cards-BelowBar([string]$Description, [int]$Min, [int]$Max) {
  $line = $null
  $width = -1
  for ($i = 0; $i -lt 25; $i++) {
    $line = [string](Ui-LogLines | Where-Object { $_.StartsWith('Floating cards: ') } | Select-Object -Last 1)
    if ($line -match '^Floating cards: window (\d+)x') { $width = [int]$Matches[1] }
    if ($width -ge $Min -and $width -le $Max) { break }
    Start-Sleep -Milliseconds 200
  }
  if ($width -lt $Min -or $width -gt $Max) {
    Ui-Fail "${Description}: no Floating cards line of a window $Min to $Max wide (last: '$line')"
  }
  if ($line -notmatch '^Floating cards: window \d+x\d+, top bar bottom (\d+); (.*)$') { Ui-Fail "${Description}: '$line'" }
  $bar = [int]$Matches[1]
  $rest = $Matches[2]
  if ($bar -le 0) { Ui-Fail "${Description}: no top bar in '$line'" }
  if (-not $rest.Contains('browserDock')) { Ui-Fail "${Description}: no Browser card in '$line'" }
  $count = 0
  foreach ($entry in ($rest -split ';')) {
    if ($entry.Trim() -notmatch '^(\S+) (-?\d+),(-?\d+) \d+x\d+$') { continue }
    if ([int]$Matches[3] -lt $bar) {
      Ui-Fail "${Description}: $($Matches[1])'s top $($Matches[3]) is above the top bar's bottom $bar ($line)"
    }
    $count++
  }
  Write-Host "ok   ${Description}: $count cards below the top bar's bottom $bar (window $width wide)"
}

Write-Host '--- 1. Docked without a choice; Floating chosen, Later'
Ui-StartApp 'chrome-layout'
Ui-ExpectLog "Chrome: docked (the platform's default)" 'Docked without a choice'
if (Has-Line 'Floating cards: ') { Ui-Fail 'Floating cards in the Docked layout' }
Choose-Layout 'End'
Ui-ExpectNew 'Layout preference: floating (restart asked)' 'Floating chosen, a restart asked'
Ui-FocusDialog '^Restart Mitcad$'
Ui-Step 'later (Escape)'                   { Ui-Key 'Escape' }
Ui-FocusMain
Ui-ExpectNew 'Layout restart: later (floating at the next start)' 'kept for the next start'
if (-not (Select-String -Path $ini -Pattern '^layout=floating$' -Quiet)) { Ui-Fail 'the layout is not saved' }
Write-Host 'ok   saved as appearance/layout'

Write-Host '--- 2. Docked again: the window''s layout, no question'
Choose-Layout 'Home'
Ui-ExpectNew 'Layout preference: docked (in use)' 'Docked is the window''s layout'
if (-not (Select-String -Path $ini -Pattern '^layout=docked$' -Quiet)) { Ui-Fail 'Docked is not saved' }
Write-Host 'ok   saved'

Write-Host '--- 3. Floating, Restart Now with unsaved changes: Cancel keeps Mitcad open'
Ui-Step 'create sketch on XY (Alt+S)'      { Ui-CreateSketch 'alt+s' 'xy' }
Ui-Step 'rectangle tool (R)'             { Ui-Key 'r' }
Ui-Step 'first corner at the origin'     { Ui-SketchClick 0 0 }
Ui-Step 'width 40 (Tab) height 25'       { Ui-Type '40'; Ui-Key 'Tab'; Ui-Type '25' }
Ui-Step 'confirm (Enter)'                { Ui-Key 'Return' }
Ui-ExpectLog 'Added rectangle 40 x 25 mm at (0, 0)' 'a rectangle: a change to save'
Ui-Step 'finish the sketch (Ctrl+Enter)'   { Ui-Key 'ctrl+Return' }
Ui-ExpectLog 'Sketch finished' 'a change to save'
Choose-Layout 'End'
Ui-ExpectNew 'Layout preference: floating (restart asked)' 'a restart asked'
# The message boxes titled "Mitcad" are new windows (Ui-FocusDialog).
$before = Ui-Windows '^Mitcad$'
Ui-FocusDialog '^Restart Mitcad$'
Ui-Step 'restart now (Enter)'              { Ui-Key 'Return' }
Ui-ExpectNew 'Layout restart: now' 'Restart Now'
Ui-ExpectNew 'Restart: closing the window' 'the window closes first'
Ui-FocusDialog '^Mitcad$' -Except $before
Ui-Step 'cancel (Escape)'                  { Ui-Key 'Escape' }
Ui-FocusMain
Ui-ExpectNew 'Restart cancelled: Mitcad stayed open' 'cancelled as quitting would be'
if (Has-Line 'Restart (test, not started)') { Ui-Fail 'a restart after the cancel' }
Write-Host 'ok   Mitcad still runs, nothing restarted'

Write-Host '--- 4. Saved; Floating, Restart Now: Mitcad quits and starts again'
Ui-Mark
Ui-Step 'save as (Ctrl+Shift+S)'           { Ui-Key 'ctrl+shift+s' }
Ui-FocusDialog '^Save As$'
Ui-Step 'the path'                         { Ui-Key 'ctrl+a'; Ui-Type $file; Ui-Key 'Return' }
Ui-FocusMain
Ui-ExpectNew "Saved $fileLogged" 'saved'
Choose-Layout 'Home'
Ui-ExpectNew 'Layout preference: docked (in use)' 'Docked again'
Choose-Layout 'End'
Ui-ExpectNew 'Layout preference: floating (restart asked)' 'a restart asked'
Ui-FocusDialog '^Restart Mitcad$'
# No step: the process ends.
Ui-Key 'Return'
Ui-ExpectNew 'Restart (test, not started): ' "the restart's command" 20
$command = ' ' + (Ui-LastMatch '^Restart \(test, not started\): (.*)$') + ' '
if ($command.Contains(' --chrome')) { Ui-Fail "the restart keeps --chrome: $command" }
foreach ($part in @('--no-native-dialogs', '--no-recovery', "--open $fileLogged")) {
  if (-not $command.Contains(" $part ")) { Ui-Fail "the restart has no '$part': $command" }
}
Write-Host "ok   the run's settings options and the design, no --chrome:$command"
if (-not $script:UiProcess.WaitForExit(10000)) { Ui-Fail 'Mitcad did not exit to restart' }
# ExitCode is empty when the process handle was not read before it ended;
# the log tells a crash (Ui-Crashed and the crash folder are checked at the end).
$exitCode = $script:UiProcess.ExitCode
if ($null -ne $exitCode -and "$exitCode" -ne '' -and $exitCode -ne 0) { Ui-Fail "Mitcad exited with $exitCode" }
Write-Host 'ok   Mitcad quit to restart'

Write-Host '--- 5. The next start: Floating by the preference, the cards below the top bar'
Ui-StartApp 'chrome-layout-floating' @('--open', $file)
Ui-ExpectLog 'Chrome: floating (the preference)' 'Floating by the preference'
Ui-ExpectLog 'Floating cards: window ' 'the cards placed'
# The default size is the session's (1028 wide in session 0 of the VM).
Cards-BelowBar 'at the default size' 1 100000
# As narrow as the window gets: the ribbon's groups collapse.
Ui-Step 'a narrow window'                  { if (-not [LayoutTestWindow]::Resize($script:UiWindow, 800, 640)) { Ui-Fail 'the window was not resized' } }
Cards-BelowBar 'in a narrow window' 1 900
Ui-Step 'a wide window'                    { if (-not [LayoutTestWindow]::Resize($script:UiWindow, 1500, 900)) { Ui-Fail 'the window was not resized' } }
# As wide as the desktop lets it (session 0's desktop may be narrower).
Cards-BelowBar 'in a wide window' 901 1500
Ui-StopApp

Write-Host '--- 6. --chrome=docked overrides the preference'
Ui-StartApp 'chrome-layout-docked' @('--chrome=docked')
Ui-ExpectLog 'Chrome: docked (--chrome; the preference floating)' '--chrome decides, the preference named'
if (Has-Line 'Floating cards: ') { Ui-Fail 'Floating cards in the Docked layout' }
Write-Host 'ok   no cards'

Ui-Finish 'UI Windows chrome layout test'
