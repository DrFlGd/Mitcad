# SPDX-License-Identifier: MIT
# check-all sources: app/commands app/framework/CommandSession.cpp app/framework/TestDriver.cpp core/model/src/print_fits.rs core/model/src/print_fits
# The same actual-control workflow as the Qt and AppKit Mac tests.
param(
  [string]$App,
  [string]$Cli,
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-print-fits'),
  [string]$QtBin = '')
. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')
Ui-Init $App $Out $QtBin @{}
$design = Join-Path $script:UiOut 'shaft.mitcad'
& $Cli run (Join-Path $PSScriptRoot 'ui-scripts/printed-fits.json') --save $design
if ($LASTEXITCODE -ne 0) { Ui-Fail 'printed-fit fixture creation failed' }
$env:MITCAD_TEST_INPUT = Join-Path $PSScriptRoot 'ui-scripts/printed-fits.mitcad-ui'
$env:MITCAD_TEST_STEP_MS = '70'
Ui-StartApp 'fits' @('--ui=qt', '--no-mcp', '--open', $design)
# The handle read while the app runs: without it ExitCode stays empty after
# the exit (Start-Process -PassThru), which read as a failure on Windows.
$null = $script:UiProcess.Handle
if (-not $script:UiProcess.WaitForExit(300000)) { Ui-Fail 'printed-fit driver timed out' }
$exitCode = $script:UiProcess.ExitCode
if ($null -ne $exitCode -and "$exitCode" -ne '' -and $exitCode -ne 0) {
  Ui-Fail "printed-fit driver failed (exit code $exitCode)"
}
Ui-ExpectLog 'TestDriver: done' 'printed-fit fields, parameters and undo passed'
Ui-StopApp
Write-Host 'PASS: printed fits'
