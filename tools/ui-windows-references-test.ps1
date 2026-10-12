# SPDX-License-Identifier: MIT
# check-all sources: app/files app/framework/Dialogs.cpp core/import core/f3d tools/cli
# check-all sources: core/ffi/src/f3d_import.rs core/ffi/src/f3d_references.rs core/tests
# Input-driven Windows UI test of the .f3d import of a design whose
# components come from other documents (mitcad#196), in the build VM
# (tools/ui-windows-lib.ps1 says how the input is posted):
#   1. Open (Ctrl+O) a synthetic assembly (test_exchange
#      --write-f3d-references) whose documents are in a folder that is not
#      next to it: a file dialog for each document the design refers to,
#      naming it; Esc skips each, and the design opens with their
#      components empty.
#   2. Open it again and choose the part's file: its folder is searched for
#      the other document (by the GUID the file lists), so no second dialog
#      opens; the import fills both documents' components, placed bodies
#      and all.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-references-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-references'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{}
$testExchange = Join-Path (Split-Path $script:UiApp) 'test_exchange.exe'
if (-not (Test-Path $testExchange)) { Ui-Fail "no $testExchange" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
& $testExchange --write-f3d-references $work parts
if ($LASTEXITCODE -ne 0) { Ui-Fail 'the test files were not written' }
$assembly = Join-Path $work 'Assembly v1.f3d'
$part = Join-Path $work 'parts\Part v2.f3d'

# A path typed into a file dialog's focused field, then Enter.
function Type-Into-Dialog([string]$Title, [string]$Path) {
  Ui-FocusDialog $Title
  Ui-Key 'ctrl+a'
  Ui-Type $Path
  Start-Sleep -Milliseconds 300
  Ui-Key 'Return'
  Ui-FocusMain
}

function Open-Assembly {
  Ui-Step 'open (Ctrl+O)'                { Ui-Key 'ctrl+o' }
  Ui-Step 'type the path'                { Type-Into-Dialog '^Open$' $assembly }
}

Write-Host '--- Both documents skipped'
Ui-StartApp 'references-skipped'
Ui-Mark
Open-Assembly
Ui-ExpectNew 'References dialog: ? version 2 of Assembly v1.f3d' 'asked for the part'
Ui-Step 'skip (Esc)'                     { Ui-FocusDialog '^Find .*, Version 2$'; Ui-Key 'Escape'; Ui-FocusMain }
Ui-ExpectNew 'References dialog: skipped' 'the part skipped'
Ui-ExpectNew 'References dialog: ? version 1 of Assembly v1.f3d' 'asked for the sub-assembly'
Ui-Step 'skip (Esc)'                     { Ui-FocusDialog '^Find .*, Version 1$'; Ui-Key 'Escape'; Ui-FocusMain }
Ui-ExpectNew 'Import references of Assembly v1.f3d: 0 found, 2 skipped' 'both skipped'
Ui-ExpectNew 'Imported Assembly v1.f3d: 0 bodies' 'imported with the components empty' 60
Ui-ExpectNew 'Imported Assembly v1.f3d: documents referred to:  skipped,  skipped' 'the report says they were skipped'

Write-Host '--- The part chosen, the sub-assembly found next to it'
Ui-StartApp 'references-chosen'
Ui-Mark
Open-Assembly
Ui-ExpectNew 'References dialog: ? version 2 of Assembly v1.f3d' 'asked for the part'
Ui-Step "type the part's path"           { Type-Into-Dialog '^Find .*, Version 2$' $part }
Ui-ExpectNew 'References dialog: chose Part v2.f3d' 'the part chosen'
Ui-ExpectNew 'Import references of Assembly v1.f3d: 2 found, 0 skipped' 'the sub-assembly found in the part''s folder'
if ((Ui-LogLines) | Where-Object { $_.Contains('References dialog: ? version 1') }) { Ui-Fail 'asked for the sub-assembly too' }
Ui-ExpectNew 'Imported Assembly v1.f3d: 2 bodies' 'the documents imported' 60
Ui-ExpectNew 'Imported Assembly v1.f3d: documents referred to: Part linked, Holder linked' 'both linked'

Ui-Finish 'UI Windows references test'
