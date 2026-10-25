# SPDX-License-Identifier: MIT
# check-all sources: app/files core/ipt core/import core/f3d core/ffi/src/iam_import.rs core/ffi/src/ipt_import.rs
# check-all sources: core/ffi/src/ipt_history.rs core/model/src/document/components.rs core/tests
# Input-driven Windows UI test of an .iam assembly's parts with their
# design history (mitcad#507), in the build VM (tools/ui-windows-lib.ps1
# says how the input is posted), as tools/ui-iam-history-test.sh on Linux:
#   1. The test project of test_exchange --write-iam opened (Ctrl+O):
#      Import Assembly asks how the parts come in, Parts with their design
#      history on by default. On, the progress follows the parts (part n of
#      3), the cube part's sketch and extrusion are replayed into its
#      component, the report says per part whether it was replayed or came
#      in as its stored bodies and why.
#   2. Off (Alt+H): every part as its stored bodies, one base feature per
#      part; the components' volumes are those of the import with the
#      history. The next dialog shows the choice made last; Esc cancels.
#   3. With the test files' assembly 1dc60c2a (seven parts; never
#      committed): MITCAD_IAM_UI_ASSEMBLY, else found by its content id
#      under the folders of MITCAD_IPT_CORPUS (an INDEX.tsv or index.tsv
#      naming it by its SHA-256, else every .iam hashed): the same both
#      ways, each part's replayed feature count listed. Skipped without it.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-iam-history-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-iam-history'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{}
$testExchange = Join-Path (Split-Path $script:UiApp) 'test_exchange.exe'
if (-not (Test-Path $testExchange)) { Ui-Fail "no $testExchange" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
& $testExchange --write-iam (Join-Path $work 'iam')
if ($LASTEXITCODE -ne 0) { Ui-Fail 'the test assemblies were not written' }
$top = Join-Path $work 'iam\top.iam'
# How long an import may take (a real assembly's parts replayed).
$importWait = 1200

# A path typed into a file dialog's focused field, then Enter.
function Type-Into-Dialog([string]$Title, [string]$Path) {
  Ui-FocusDialog $Title
  Ui-Key 'ctrl+a'
  Ui-Type $Path
  Start-Sleep -Milliseconds 300
  Ui-Key 'Return'
}

# A new app, File > Open (Ctrl+O) of the assembly; the mark set.
function Open-Assembly([string]$Path, [string]$Run) {
  Ui-StartApp $Run
  Ui-Mark
  Ui-Step "open $(Split-Path -Leaf $Path) (Ctrl+O)" { Ui-Key 'ctrl+o' }
  Ui-Step 'type the path' { Type-Into-Dialog '^Open$' $Path }
}

# The lines logged since the mark.
function New-Lines { return @(Ui-LogLines | Select-Object -Skip $script:UiMark) }

# The assembly opened, Import Assembly set to the history wanted (on, off),
# the import to its end and its report closed; the lines it logged in
# $script:ImportLines (the helpers' output would mix with a return value).
function Import-Assembly([string]$Path, [string]$Want, [string]$Run) {
  $name = Split-Path -Leaf $Path
  Open-Assembly $Path $Run
  Ui-ExpectNew "Import Assembly dialog opened: $name, parts with their design history " 'Import Assembly asks how the parts come in'
  $shown = (New-Lines | Where-Object { $_.Contains("Import Assembly dialog opened: $name") } | Select-Object -Last 1) -replace '.*design history ', ''
  Ui-FocusDialog '^Import Assembly$'
  if ($shown -ne $Want) {
    Ui-Step "parts with their design history $Want (Alt+H)" { Ui-Key 'alt+h' }
    Ui-ExpectNew "Import Assembly: parts with their design history $Want" 'the box toggled'
  }
  Ui-Step 'import (Enter)' { Ui-Key 'Return' }
  Ui-ExpectNew "Import Assembly: $name, parts with their design history $Want" "imported with the history $Want"
  Ui-ExpectNew "Import of ${name}: parts with their design history $Want" 'the import runs with that choice'
  Ui-ExpectNew "Import report: ${name}: " 'the import finished with a report' $importWait
  Ui-FocusDialog '^Import Report$'
  Ui-Step 'close the report (Enter)' { Ui-Key 'Return' }
  Ui-FocusMain
  Ui-ExpectNew "Imported ${name}: " "a new document of the assembly's components"
  $script:ImportLines = New-Lines
}

# Each component the import logged: name -> @{features; bodies; volume}.
function Components([string[]]$Lines) {
  $components = @{}
  foreach ($line in $Lines) {
    if ($line -match 'Imported [^:]*: component (.*): ([0-9]+) features, ([0-9]+) bodies, volume ([0-9.eE+-]+) mm3$') {
      $components[$Matches[1]] = @{
        features = [int]$Matches[2]
        bodies = [int]$Matches[3]
        volume = [double]::Parse($Matches[4], [Globalization.CultureInfo]::InvariantCulture)
      }
    }
  }
  return $components
}

# Every component of the import with the history has the volume of the one
# without within the import's tolerance (1e-3 relative, as the import
# report matches bodies; at least 1e-6 mm3).
function Compare-Volumes([hashtable]$On, [hashtable]$Off) {
  if ($On.Count -eq 0) { Ui-Fail 'no components to compare' }
  if ($On.Count -ne $Off.Count) { Ui-Fail "$($On.Count) components with the history, $($Off.Count) without" }
  $worst = 0.0
  foreach ($name in $On.Keys) {
    if (-not $Off.ContainsKey($name)) { Ui-Fail "the component $name only with the history" }
    $a = $On[$name].volume
    $b = $Off[$name].volume
    $limit = [Math]::Max(1e-3 * [Math]::Abs($b), 1e-6)
    if ([Math]::Abs($a - $b) -gt $limit) { Ui-Fail "${name}: $a mm3 with the history, $b mm3 without" }
    if ([Math]::Abs($b) -gt 0) { $worst = [Math]::Max($worst, [Math]::Abs($a - $b) / [Math]::Abs($b)) }
  }
  Write-Host "ok   $($On.Count) components' volumes agree both ways (largest relative difference $worst)"
}

function Part-Lines([string[]]$Lines) { return @($Lines | Where-Object { $_.Contains('Import report part: ') }) }

Write-Host '--- The test project, parts with their design history (the default)'
Import-Assembly $top 'on' 'iam-on'
$on = $script:ImportLines
if (-not ($on | Where-Object { $_.Contains('Import Assembly dialog opened: top.iam, parts with their design history on') })) {
  Ui-Fail 'the history is not on by default'
}
Write-Host 'ok   on by default'
if (-not ($on | Where-Object { $_ -match 'Import of top\.iam: part [123] of 3: cube\.ipt: replayed in ' })) {
  Ui-Fail 'the progress did not follow the parts'
}
Write-Host 'ok   the progress followed the parts (of 3)'
if (-not ($on | Where-Object { $_.Contains('Import report part: cube.ipt: replayed, 2 items (2 parametric, 0 partial, 0 fallback, 0 skipped)') })) {
  Ui-Fail "the cube was not replayed: $((Part-Lines $on) -join '; ')"
}
if (-not ($on | Where-Object { $_.Contains('Import report part: pair.ipt: stored bodies: the part has no design history') })) {
  Ui-Fail "the part without a design is not reported so: $((Part-Lines $on) -join '; ')"
}
if (-not ($on | Where-Object { $_.Contains('Components with their design history: 1 replayed, 2 as their stored bodies.') })) {
  Ui-Fail 'the report does not sum the parts up'
}
Write-Host 'ok   the report: the cube replayed, the parts without a design as their stored bodies'
$onComponents = Components $on
if (-not $onComponents.ContainsKey('cube') -or $onComponents['cube'].features -lt 2) {
  Ui-Fail "the cube's component does not have its features"
}
Write-Host "ok   the cube's component has its $($onComponents['cube'].features) features"

Write-Host '--- The test project, the parts'' stored bodies'
Import-Assembly $top 'off' 'iam-off'
$off = $script:ImportLines
$parts = @(Part-Lines $off)
if ($parts.Count -ne 3) { Ui-Fail "the report lists $($parts.Count) parts, not 3" }
if ($parts | Where-Object { -not $_.EndsWith(': stored bodies: the stored bodies chosen') }) {
  Ui-Fail "a part came in otherwise than as its stored bodies: $($parts -join '; ')"
}
$offComponents = Components $off
if (-not $offComponents.ContainsKey('cube') -or $offComponents['cube'].features -ne 1) {
  Ui-Fail "the cube's component is not its bodies' base feature"
}
Write-Host 'ok   every part as its stored bodies, one base feature each'
Compare-Volumes $onComponents $offComponents

Write-Host '--- The choice remembered'
Open-Assembly $top 'iam-remembered'
Ui-ExpectNew 'Import Assembly dialog opened: top.iam, parts with their design history off' 'the dialog shows the choice made last'
Ui-FocusDialog '^Import Assembly$'
Ui-Step 'cancel (Esc)' { Ui-Key 'Escape' }
Ui-ExpectNew 'Import Assembly cancelled' 'cancelled: nothing imported'
Ui-FocusMain
Start-Sleep -Seconds 1
if (New-Lines | Where-Object { $_.Contains('Import of top.iam started') -or $_.Contains('Import report') }) {
  Ui-Fail 'the cancelled import started'
}
Write-Host 'ok   the import did not start'

# The test files' seven-part assembly (content id 1dc60c2a), never committed.
$assembly = $env:MITCAD_IAM_UI_ASSEMBLY
if (-not $assembly -and $env:MITCAD_IPT_CORPUS) {
  $folders = @($env:MITCAD_IPT_CORPUS -split ';' | Where-Object { $_ -and (Test-Path -PathType Container $_) })
  foreach ($folder in $folders) {
    foreach ($index in @('INDEX.tsv', 'index.tsv')) {
      $file = Join-Path $folder $index
      if ($assembly -or -not (Test-Path $file)) { continue }
      foreach ($row in Get-Content -Encoding UTF8 $file) {
        $cells = $row -split "`t"
        if (@($cells | Select-Object -Skip 1 | Where-Object { $_ -like '1dc60c2a*' }).Count -gt 0) {
          $candidate = Join-Path $folder ($cells[0] -replace '/', '\')
          if (Test-Path $candidate) { $assembly = $candidate; break }
        }
      }
    }
  }
  if (-not $assembly) {
    foreach ($folder in $folders) {
      $found = Get-ChildItem -Recurse -File -Filter '*.iam' $folder -ErrorAction SilentlyContinue |
        Where-Object { (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLowerInvariant().StartsWith('1dc60c2a') } |
        Select-Object -First 1
      if ($found) { $assembly = $found.FullName; break }
    }
  }
}
if ($assembly -and (Test-Path $assembly)) {
  $name = Split-Path -Leaf $assembly
  Write-Host '--- The assembly 1dc60c2a, parts with their design history'
  Import-Assembly $assembly 'on' 'real-on'
  $realOn = $script:ImportLines
  if (-not ($realOn | Where-Object { $_.Contains("Import of ${name}: part 1 of ") })) { Ui-Fail 'the progress did not follow the parts' }
  $parts = @(Part-Lines $realOn)
  $replayed = @($parts | Where-Object { $_ -match ': replayed, ' })
  Write-Host "ok   $($parts.Count) parts, $($replayed.Count) replayed:"
  $parts | ForEach-Object { Write-Host "       $($_ -replace '.*Import report part: ', '')" }
  if ($replayed.Count -eq 0) { Ui-Fail 'no part of 1dc60c2a was replayed' }
  $realOnComponents = Components $realOn
  $featured = @($realOnComponents.Keys | Where-Object { $realOnComponents[$_].features -gt 1 })
  if ($featured.Count -eq 0) { Ui-Fail 'no component of 1dc60c2a has its features' }
  Write-Host "ok   $($featured.Count) components with their parts' features"
  Write-Host '--- The assembly 1dc60c2a, the parts'' stored bodies'
  Import-Assembly $assembly 'off' 'real-off'
  $realOff = $script:ImportLines
  if (Part-Lines $realOff | Where-Object { -not $_.EndsWith(': stored bodies: the stored bodies chosen') }) {
    Ui-Fail 'a part of 1dc60c2a came in otherwise than as its stored bodies'
  }
  Compare-Volumes $realOnComponents (Components $realOff)
} else {
  Write-Host 'skip the assembly 1dc60c2a: neither MITCAD_IAM_UI_ASSEMBLY nor MITCAD_IPT_CORPUS has it'
}

Ui-Finish 'UI Windows .iam history test'
