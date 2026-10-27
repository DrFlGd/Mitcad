# SPDX-License-Identifier: MIT
# check-all sources: app/MainWindowImageReferences.cpp app/OcctViewer.cpp app/view app/browser app/framework tools/cli
# check-all sources: core/model/src/document core/model/src/file core/model-api/src core/model-base/src
# Input-driven Windows UI test of image references (mitcad#782) in the
# build VM (tools/ui-windows-lib.ps1 says how the input is posted), as
# tools/ui-image-references-test.sh does on Linux, on a 60 x 60 x 3 plate
# mitcad-cli makes and a 200 x 100 pixel grey BMP file that states 4000
# pixels per metre (0.25 mm each). The command search cannot open in
# session 0: Image Reference (F8) and Calibrate Image Reference (F9) come
# from shortcuts of the test's.
#   1. Image Reference on the plate's top face, the file's path typed,
#      Opacity 40: Image1 drawn 50 x 25 mm, 40 % opaque, behind the bodies,
#      in the browser's Images folder.
#   2. Calibrate: two clicks a tenth in from the image's left and right
#      edges (160 pixels apart), 80 typed: drawn 100 mm wide.
#   3. Its row double-clicked: Opacity 70 and Through Bodies in its panel.
#   3. (Through Bodies: in the view's image layer, before the sketch
#      layers.)
#   4. Its light bulb off: not drawn, Calibrate refused; saved, opened again, its light bulb
#      on: drawn as it was.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-image-references-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows-image-references'),
  [string]$QtBin = '')

. (Join-Path $PSScriptRoot 'ui-windows-lib.ps1')

Ui-Init $App $Out $QtBin @{ 'insert.image_reference' = 'F8'; 'insert.calibrate_image_reference' = 'F9' }
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $script:UiApp) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Ui-Fail "no mitcad-cli.exe beside $($script:UiApp)" }
$work = Join-Path $script:UiOut 'files'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null

# A 60 x 60 x 3 plate (Box1 F1, Body1).
$modelScript = Join-Path $work 'model.json'
$design = Join-Path $work 'plate.mitcad'
Set-Content -Encoding ascii $modelScript @'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": {"type": "origin", "plane": "xy"},
    "corner": [0, 0], "length": 60, "width": 60, "height": 3, "operation": "new_body"}}
]
'@
$made = & $cli run $modelScript --save $design
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $design)) { Ui-Fail "mitcad-cli: $($made -join ' / ')" }

# The BMP file: a 54-byte header (200 x 100, 24 bits, 4000 pixels per
# metre), then 100 rows of 600 grey bytes.
$image = Join-Path $work 'plan.bmp'
$header = [byte[]](0x42, 0x4d, 0x96, 0xea, 0, 0, 0, 0, 0, 0, 0x36, 0, 0, 0,
  0x28, 0, 0, 0, 0xc8, 0, 0, 0, 0x64, 0, 0, 0, 1, 0, 0x18, 0,
  0, 0, 0, 0, 0x60, 0xea, 0, 0, 0xa0, 0x0f, 0, 0, 0xa0, 0x0f, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0)
$bytes = New-Object byte[] 60054
[Array]::Copy($header, $bytes, 54)
for ($i = 54; $i -lt $bytes.Length; $i++) { $bytes[$i] = 0x80 }
[IO.File]::WriteAllBytes($image, $bytes)

# Expect-Drawn "regex" "description": a line "Image references drawn: ..."
# since the mark matches.
function Expect-Drawn([string]$Pattern, [string]$Description) {
  for ($i = 0; $i -lt 30; $i++) {
    $lines = Ui-LogLines | Select-Object -Skip $script:UiMark
    if ($lines | Where-Object { $_ -match "^Image references drawn: $Pattern" }) { Write-Host "ok   $Description"; return }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "${Description}: no 'Image references drawn: $Pattern' since the mark"
}
# Expect-Width "name" width: its width in the last "Image references
# drawn" line within 2 % of the one expected (clicks land on whole pixels).
function Expect-Width([string]$Name, [double]$Expected) {
  $width = $null
  for ($i = 0; $i -lt 30; $i++) {
    $found = Ui-LastMatch "^Image references drawn: .*$Name ([0-9.]+) x "
    if ($found) {
      $width = [double]::Parse($found, [Globalization.CultureInfo]::InvariantCulture)
      if ([Math]::Abs($width - $Expected) -lt 0.02 * $Expected) { Write-Host "ok   $Name drawn $width mm wide"; return }
    }
    Start-Sleep -Milliseconds 200
  }
  Ui-Fail "$Name is drawn $width mm wide, not $Expected mm"
}

Ui-StartApp 'image-references' @('--open', $design)
Ui-Step 'fit (F6)'                       { Ui-Key 'F6' }

Write-Host '--- 1. Image Reference on the plate''s top'
Ui-Mark
Ui-Step 'Image Reference (F8)'           { Ui-Key 'F8' }
Ui-ExpectNew 'Command Image Reference started' 'the panel opened'
Ui-Step 'click the plate''s top'         { Ui-ClickPick 'face F1.b0/F1:top' }
Ui-ExpectNew 'Image Reference Plane: 1 face' 'the face picked'
Ui-Step 'the image''s file'              { Ui-TypeIn 'Panel Image Reference input file' $image }
Ui-Step 'opacity 40'                     { Ui-TypeIn 'Panel Image Reference input opacity' '40' }
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Inserted image reference from plan.bmp' 'inserted' 30
Ui-ExpectNew 'Image references drawn: Image1 50.0 x 25.0 mm 40% behind' 'drawn 50 x 25 mm, 40 % opaque, behind'
Ui-ExpectLog 'Browser Root/Images/Image1 at' 'listed in the Images folder'

Write-Host '--- 2. Calibrate: 160 pixels are 80 mm'
Ui-Mark
Ui-Step 'Calibrate Image Reference (F9)' { Ui-Key 'F9' }
Ui-ExpectNew 'Calibrate Image1 started' 'calibration started'
Ui-ExpectNew 'Calibrate Image1 right at' 'the image''s places logged'
Ui-Step 'the first point'                { Ui-ClickLogged 'Calibrate Image1 left' }
Ui-ExpectNew 'Calibrate Image1 first point' 'the first point taken'
Ui-Step 'the second point'               { Ui-ClickLogged 'Calibrate Image1 right' }
Ui-ExpectNew 'Calibrate Image1 second point' 'the second point taken'
Ui-Step '80 mm' {
  Ui-FocusDialog '^Calibrate Image1$'
  Ui-Key 'ctrl+a'
  Ui-Type '80'
  Start-Sleep -Milliseconds 300
  Ui-Key 'Return'
  Ui-FocusMain
}
Ui-ExpectNew 'Calibrated Image1: ' 'calibrated' 30
Expect-Width 'Image1' 100

Write-Host '--- 3. Edited in its panel: 70 % opaque, through the bodies'
Ui-Mark
Ui-Step 'Image1''s row (double-click)'   { Ui-DoubleClickLogged 'Browser Root/Images/Image1' }
Ui-ExpectNew 'Editing image reference Image1 with Image Reference' 'the edit panel opened'
Ui-Step 'opacity 70'                     { Ui-TypeIn 'Panel Image Reference input opacity' '70' }
Ui-Step 'Through Bodies'                 { Ui-ClickLogged 'Panel Image Reference input through' }
Ui-Step 'OK (Enter)'                     { Ui-Key 'Return' }
Ui-ExpectNew 'Edited image reference Image1' 'edited' 30
Expect-Drawn 'Image1 .* 70% through' 'drawn 70 % opaque, over the bodies'
Ui-ExpectNew 'Image references through the bodies: in the image layer, before the sketch halos' 'drawn under the sketches'

Write-Host '--- 4. Hidden, saved, opened again and shown'
Ui-Mark
Ui-Step 'Image1''s light bulb off'       { Ui-ClickLogged 'Browser eye Root/Images/Image1' }
Ui-ExpectNew 'Visibility Root/Images/Image1: hidden' 'hidden'
Ui-ExpectNew 'Image references drawn: none' 'not drawn'
Ui-Step 'Calibrate Image Reference (F9)' { Ui-Key 'F9' }
Ui-ExpectNew 'Show an image reference first.' 'a hidden image is not calibrated'
Ui-Step 'save (Ctrl+S)'                  { Ui-Key 'ctrl+s' }
Ui-ExpectNew 'Saved ' 'saved' 30
Ui-StartApp 'image-references-again' @('--open', $design)
Ui-ExpectLog 'Browser eye Root/Images/Image1 at' 'Image1 read back'
Ui-Mark
Ui-Step 'Image1''s light bulb on'        { Ui-ClickLogged 'Browser eye Root/Images/Image1' }
Ui-ExpectNew 'Visibility Root/Images/Image1: shown' 'shown'
Expect-Drawn 'Image1 .* 70% through' 'drawn as it was saved'
Expect-Width 'Image1' 100

Ui-Finish 'UI Windows image references test'
