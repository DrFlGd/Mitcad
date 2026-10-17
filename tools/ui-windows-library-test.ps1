# SPDX-License-Identifier: MIT
# check-all sources: app/files tools/cli
# Input-driven Windows UI test of the library browser's Update button
# (mitcad#317) in the build VM (tools/ui-windows-lib.ps1 says how the input
# is posted); the Linux test is tools/ui-library-test.sh (parts 2b and 6).
# Local git repositories only, no network; skipped where git is not
# installed (fetching needs it):
#   1. Libraries...: the default sources removed, a library of one block
#      (mitcad-cli library init/add, one git commit) added, not fetched.
#   2. Insert from Library: empty, its text says to press Update; Update
#      fetches the source and lists the block.
#   3. A source that is no library added: Update fetches the other, says so
#      in one warning, searches again and keeps the selected block.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-library-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-library'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'tools.libraries' = 'F8'; 'insert.library_component' = 'F9' }

$git = Get-Command git -ErrorAction SilentlyContinue
if (-not $git) {
  Write-Host 'note: git is not installed; libraries cannot be fetched, nothing to test'
  Write-Host 'PASS: UI Windows library test (skipped)'
  exit 0
}
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$gitConfig = Join-Path $work 'gitconfig'
Set-Content -Encoding ascii $gitConfig ''
$env:GIT_CONFIG_GLOBAL = $gitConfig
$env:GIT_CONFIG_NOSYSTEM = '1'
# The fetched libraries of the test's own.
$env:MITCAD_LIBRARIES_DIR = Join-Path $work 'cache'

# A block (20 x 20 x 10) in a library of its own, recorded with git.
$script = Join-Path $work 'block.json'
Set-Content -Encoding ascii $script @'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 20, "height": 20},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}}
]
'@
$block = Join-Path $work 'block.mitcad'
$output = & $cli run $script --save $block
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $block)) { Ui-Fail "mitcad-cli run: $($output -join ' / ')" }
$library = Join-Path $work 'blocks'
$output = & $cli library init $library --id test-blocks --name 'Test blocks' --license CC0-1.0
if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli library init: $($output -join ' / ')" }
$output = & $cli library add $library $block --id block --name 'Test block' --category blocks --license CC0-1.0
if ($LASTEXITCODE -ne 0) { Ui-Fail "mitcad-cli library add: $($output -join ' / ')" }
foreach ($arguments in @(@('add', '-A'), @('commit', '-q', '-m', 'Blocks'))) {
  $output = & $git.Source -C $library -c user.name=T -c user.email=t@example.invalid @arguments 2>&1
  if ($LASTEXITCODE -ne 0) { Ui-Fail "git $($arguments -join ' '): $($output -join ' / ')" }
}
$broken = Join-Path $work 'no-such-library'

# Add-Source path: Add URL in Libraries... (open), then back to it.
function Add-Source([string]$Path) {
  Ui-Step 'Add URL (Alt+A)'              { Ui-Key 'alt+a' }
  Ui-FocusDialog '^Add Library$'
  Ui-Step 'the URL'                      { Ui-Type $Path; Ui-Key 'Return' }
  Ui-FocusDialog '^Libraries$'
  Ui-ExpectNew "Library source added: $Path" 'the source added'
}

Ui-StartApp 'library' @('--open', $block)

Write-Host '--- 1. Libraries: only the block library, not fetched'
Ui-Mark
Ui-Step 'Libraries (F8)'                 { Ui-Key 'F8' }
Ui-FocusDialog '^Libraries$'
Ui-ExpectNew 'Library source https://github.com/mitcad/parts.git (on): not fetched' 'the default sources listed'
Ui-Step 'remove a default (Alt+R)'       { Ui-Key 'alt+r' }
Ui-Step 'and the other (Alt+R)'          { Ui-Key 'alt+r' }
Ui-ExpectNew 'Library source removed: https://github.com/mitcad/community-index.git' 'the defaults removed'
Add-Source $library
Ui-Step 'close (Alt+C)'                  { Ui-Key 'alt+c' }
Ui-FocusMain

Write-Host '--- 2. Insert from Library: empty until Update (mitcad#317)'
Ui-Mark
Ui-Step 'Insert from Library (F9)'       { Ui-Key 'F9' }
Ui-FocusDialog '^Insert from Library$'
Ui-ExpectNew "Library search '': 0 components, 0 libraries" 'nothing listed before a fetch'
Ui-ExpectNew 'Library browser empty: nothing fetched yet, press Update' 'the empty list says to press Update'
Ui-Mark
Ui-Step 'Update (Alt+U)'                 { Ui-Key 'alt+u' }
Ui-ExpectNew 'Library browser update: 1 enabled source(s)' 'Update fetches the enabled source'
Ui-ExpectNew "Library fetched: library Test blocks (test-blocks) from $library" 'the library fetched' 30
Ui-ExpectNew 'Library fetch of 1 source(s): 0 failed' 'no failures' 30
Ui-ExpectNew "Library search '': 1 components" 'the search ran again'
Ui-ExpectNew 'Library browser selected test-blocks/block: version ' 'the block listed and selected'
Ui-Step 'close (Escape)'                 { Ui-Key 'Escape' }
Ui-FocusMain

Write-Host '--- 3. A broken source: one warning, the selection kept'
Ui-Mark
Ui-Step 'Libraries (F8)'                 { Ui-Key 'F8' }
Ui-FocusDialog '^Libraries$'
Add-Source $broken
Ui-Step 'close (Alt+C)'                  { Ui-Key 'alt+c' }
Ui-FocusMain
Ui-Mark
Ui-Step 'Insert from Library (F9)'       { Ui-Key 'F9' }
Ui-FocusDialog '^Insert from Library$'
Ui-ExpectNew 'Library browser selected test-blocks/block: version ' 'the block selected'
Ui-Mark
Ui-Step 'Update (Alt+U)'                 { Ui-Key 'alt+u' }
Ui-ExpectNew 'Library browser update: 2 enabled source(s)' 'Update fetches both sources'
Ui-ExpectNew "Library fetch failed: ${broken}: " 'the broken one fails' 30
Ui-ExpectNew 'Library fetch of 2 source(s): 1 failed' 'one failure' 30
Ui-ExpectNew 'Library browser kept the selection component|' 'the search ran again, the block kept'
Ui-FocusDialog '^Fetch Libraries$'
Ui-Step 'close the warning (Return)'     { Ui-Key 'Return' }
Ui-FocusDialog '^Insert from Library$'
Ui-Step 'close (Escape)'                 { Ui-Key 'Escape' }
Ui-FocusMain

Ui-Finish 'UI Windows library test'
