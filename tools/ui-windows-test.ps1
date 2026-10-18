# SPDX-License-Identifier: MIT
# Windows UI test for the build VM: runs mitcad.exe with Mesa's software
# OpenGL (llvmpipe, see tools/dev-env/setup-mesa-windows.ps1 and
# MITCAD_MESA_DIR) and checks its screenshots:
#   1. --demo: the demo block is drawn with the body colours; the Docked
#      window got the Windows 11 frame (caption colour, Mica, rounded
#      corners) or skipped it on an older Windows, never refused, and gets
#      it again when the theme switches while it shows (mitcad#251).
#   2. --open of a project file with a fillet: recomputed and drawn.
#   3. the same file with --set d3=35: the block is drawn taller.
#   4. a design (mitcad-cli) whose fillet's edges a sketch change renamed
#      (mitcad#272): opened, the timeline and the warnings badge show the
#      fillet's warning that it found them under their new names.
#   5. MITCAD_TEST_CRASH=app and model-worker: the crash report's stack
#      starts at the crashing function, named from the PDB file (mitcad#76).
#   6. View > Rendered, with the render worker: its frames refine and show
#      the block (mitcad#51); an orbit sends the worker no view while the
#      button is held and one after the release, a burst of wheel steps one
#      view (mitcad#214); the rendered image stays over the view while the
#      camera keeps still, after the first render, an orbit, wheel steps
#      and a click on the orientation cube, and differs from the shaded
#      view (mitcad#216).
#   7. The app ends with Render Image and Render Environment open: no crash
#      at the close (mitcad#226).
# The app renders into an offscreen framebuffer and --screenshot reads it
# back, so this works over SSH, in session 0 without a desktop.
#
# Usage: powershell -ExecutionPolicy Bypass -File tools\ui-windows-test.ps1 [-App build\dev\mitcad.exe] [-Out dir] [-QtBin dir]
# Qt's DLLs are not copied next to the app; -QtBin defaults to the Qt of
# the build directory's CMakeCache.txt.
param(
  [string]$App = (Join-Path $PSScriptRoot '..\build\dev\mitcad.exe'),
  [string]$Out = (Join-Path $env:TEMP 'mitcad-ui-windows'),
  [string]$QtBin = '')

$ErrorActionPreference = 'Stop'
$App = (Resolve-Path $App).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$failures = 0

function Pass([string]$Message) { Write-Host "ok   $Message" }
function Fail([string]$Message) { Write-Host "FAIL: $Message"; $script:failures++ }

if (-not $QtBin) {
  $cache = Join-Path (Split-Path $App) 'CMakeCache.txt'
  $qtDir = if (Test-Path $cache) { Select-String -Path $cache -Pattern '^Qt6_DIR:PATH=(.*)' } else { $null }
  if ($qtDir) { $QtBin = Join-Path $qtDir.Matches[0].Groups[1].Value '..\..\..\bin' }
}
if (-not $QtBin -or -not (Test-Path (Join-Path $QtBin 'Qt6Core.dll'))) {
  Write-Host "FAIL: Qt's bin directory not found; pass -QtBin"
  exit 1
}
$env:Path = (Resolve-Path $QtBin).Path + ';' + $env:Path

if (-not (Test-Path (Join-Path (Split-Path $App) 'opengl32.dll'))) {
  Write-Host "FAIL: no opengl32.dll next to $App. Build Mesa (tools\dev-env\setup-mesa-windows.ps1)"
  Write-Host '      and configure with -DMITCAD_MESA_DIR=C:/dev/mesa/bin.'
  exit 1
}

# Pixel statistics of a screenshot. A "face" pixel lies inside a uniformly
# coloured area (thin grid lines and edges drop out) and is clearly darker
# than the light background; face pixels are grouped by brightness, and
# every group covering at least 1.5 % of the image counts as a face shade.
# The orientation cube in the upper right corner is left out.
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public class Shade {
    public int Luma, Pixels, R, G, B;
}

public class ShotStats {
    public int Width, Height, Background, FacePixels;
    public List<Shade> Shades = new List<Shade>();
    public double Percent(int pixels) { return 100.0 * pixels / (Width * Height); }
}

public static class Shot {
    const int Bucket = 8;

    public static ShotStats Analyze(string path) {
        using (Bitmap bitmap = new Bitmap(path)) {
            int w = bitmap.Width, h = bitmap.Height;
            BitmapData data = bitmap.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly,
                                              PixelFormat.Format24bppRgb);
            int stride = data.Stride;
            byte[] px = new byte[stride * h];
            Marshal.Copy(data.Scan0, px, 0, px.Length);
            bitmap.UnlockBits(data);

            ShotStats stats = new ShotStats();
            stats.Width = w;
            stats.Height = h;
            // The background is a vertical gradient: per row, the median
            // brightness of the rightmost pixels, which the fitted body
            // leaves free (grid lines are thin).
            int[] background = new int[h];
            for (int y = 0; y < h; y++) {
                List<int> edge = new List<int>();
                for (int x = Math.Max(0, w - 9); x < w; x++) edge.Add(Luma(px, stride, x, y));
                edge.Sort();
                background[y] = edge[edge.Count / 2];
            }
            int[] sorted = (int[])background.Clone();
            Array.Sort(sorted);
            stats.Background = sorted[h / 2];

            int cube = Math.Min(w, h) / 3;
            long[,] sums = new long[256 / Bucket + 1, 4];
            for (int y = 2; y < h - 2; y++) {
                for (int x = 2; x < w - 2; x++) {
                    if (x > w - cube && y < cube) continue;
                    if (!Uniform(px, stride, x, y)) continue;
                    int luma = Luma(px, stride, x, y);
                    if (luma > background[y] - 12) continue;
                    int i = y * stride + 3 * x;
                    int bucket = luma / Bucket;
                    sums[bucket, 0]++;
                    sums[bucket, 1] += px[i + 2];
                    sums[bucket, 2] += px[i + 1];
                    sums[bucket, 3] += px[i];
                    stats.FacePixels++;
                }
            }
            for (int bucket = sums.GetLength(0) - 1; bucket >= 0; bucket--) {
                long n = sums[bucket, 0];
                if (n * 1000 < 15L * w * h) continue;
                Shade shade = new Shade();
                shade.Luma = bucket * Bucket + Bucket / 2;
                shade.Pixels = (int)n;
                shade.R = (int)(sums[bucket, 1] / n);
                shade.G = (int)(sums[bucket, 2] / n);
                shade.B = (int)(sums[bucket, 3] / n);
                stats.Shades.Add(shade);
            }
            return stats;
        }
    }

    // The mean difference of two screenshots of the same size, per colour
    // channel (0-255), without the orientation cube's corner; -1 for
    // screenshots of different sizes.
    public static double MeanDifference(string pathA, string pathB) {
        byte[] a, b;
        int w, h, stride, wb, hb, strideB;
        Pixels(pathA, out a, out w, out h, out stride);
        Pixels(pathB, out b, out wb, out hb, out strideB);
        if (wb != w || hb != h) return -1;
        int cube = Math.Min(w, h) / 3;
        long sum = 0, count = 0;
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                if (x > w - cube && y < cube) continue;
                int i = y * stride + 3 * x;
                for (int k = 0; k < 3; k++) sum += Math.Abs(a[i + k] - b[i + k]);
                count += 3;
            }
        }
        return count == 0 ? -1 : (double)sum / count;
    }

    static void Pixels(string path, out byte[] px, out int w, out int h, out int stride) {
        using (Bitmap bitmap = new Bitmap(path)) {
            w = bitmap.Width;
            h = bitmap.Height;
            BitmapData data = bitmap.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly,
                                              PixelFormat.Format24bppRgb);
            stride = data.Stride;
            px = new byte[stride * h];
            Marshal.Copy(data.Scan0, px, 0, px.Length);
            bitmap.UnlockBits(data);
        }
    }

    static int Luma(byte[] px, int stride, int x, int y) {
        int i = y * stride + 3 * x;
        return (px[i + 2] * 299 + px[i + 1] * 587 + px[i] * 114) / 1000;
    }

    static bool Uniform(byte[] px, int stride, int x, int y) {
        int c = y * stride + 3 * x;
        int[] offsets = { -2 * 3, 2 * 3, -2 * stride, 2 * stride };
        foreach (int offset in offsets) {
            for (int k = 0; k < 3; k++) {
                if (Math.Abs(px[c + k] - px[c + offset + k]) > 4) return false;
            }
        }
        return true;
    }
}
'@

# Invoke-Mitcad name arguments...: runs the app until it exits and returns
# its log (Qt messages go to stderr with QT_FORCE_STDERR_LOGGING), in the
# Docked layout unless the arguments choose one (--chrome; Floating is the
# default since mitcad#429). With -Crash the app is to crash instead of
# exiting.
function Invoke-Mitcad([string]$Name, [string[]]$Arguments, [hashtable]$Environment = @{}, [switch]$Crash) {
  $info = New-Object System.Diagnostics.ProcessStartInfo $App
  if (-not ($Arguments | Where-Object { $_ -like '--chrome*' })) { $Arguments = @('--chrome=docked') + $Arguments }
  $info.Arguments = ($Arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
  $info.UseShellExecute = $false
  $info.RedirectStandardOutput = $true
  $info.RedirectStandardError = $true
  $info.EnvironmentVariables['QT_FORCE_STDERR_LOGGING'] = '1'
  # Autosave's recovery folder of the test's own, not the user's.
  $info.EnvironmentVariables['MITCAD_AUTOSAVE_DIR'] = Join-Path $Out 'autosave'
  # Crash reports of the test's own (mitcad#62): none offered from the user's.
  $info.EnvironmentVariables['MITCAD_CRASH_DIR'] = Join-Path $Out 'crashes'
  # The store of computed results (P7d) off, unless the run turns it on.
  $info.EnvironmentVariables['MITCAD_RESULT_STORE'] = 'off'
  # No update checks (mitcad#9): the tests make no network requests.
  $info.EnvironmentVariables['MITCAD_NO_UPDATE_CHECK'] = '1'
  foreach ($variable in $Environment.GetEnumerator()) {
    $info.EnvironmentVariables[$variable.Key] = $variable.Value
  }
  $process = [System.Diagnostics.Process]::Start($info)
  $stdout = $process.StandardOutput.ReadToEndAsync()
  $stderr = $process.StandardError.ReadToEndAsync()
  if (-not $process.WaitForExit(120000)) {
    $process.Kill()
    Fail "${Name}: Mitcad did not exit within 120 s"
  }
  $process.WaitForExit()
  $log = $stdout.Result + $stderr.Result
  Set-Content -Encoding utf8 (Join-Path $Out "$Name.log") $log
  if ($Crash) {
    if ($process.ExitCode -eq 0) { Fail "${Name}: Mitcad exited instead of crashing"; Write-Host $log }
  } elseif ($process.ExitCode -ne 0) { Fail "${Name}: exit code $($process.ExitCode)"; Write-Host $log }
  return $log
}

function Expect-Log([string]$Log, [string]$Pattern, [string]$Description) {
  if ($Log -match $Pattern) { Pass $Description } else { Fail "${Description}: '$Pattern' not in the log" }
}

# Check-Shot name: checks a screenshot for the body and returns its stats.
function Check-Shot([string]$Name) {
  $path = Join-Path $Out "$Name.png"
  if (-not (Test-Path $path)) { Fail "${Name}: no screenshot"; return $null }
  $stats = [Shot]::Analyze($path)
  $shades = ($stats.Shades | ForEach-Object {
    '{0} rgb({1},{2},{3}) {4:0.0}%' -f $_.Luma, $_.R, $_.G, $_.B, $stats.Percent($_.Pixels) }) -join ', '
  Write-Host ("     {0}: {1}x{2}, background {3}, faces {4:0.0}%: {5}" -f $Name, $stats.Width,
    $stats.Height, $stats.Background, $stats.Percent($stats.FacePixels), $shades)
  if ($stats.Width -lt 300 -or $stats.Height -lt 200) { Fail "${Name}: image too small" }
  elseif ($stats.Background -lt 200) { Fail "${Name}: background is not the light gradient" }
  elseif ($stats.Percent($stats.FacePixels) -lt 20) { Fail "${Name}: no body in the image" }
  elseif ($stats.Shades.Count -lt 2) { Fail "${Name}: fewer than two lit faces" }
  elseif ($stats.Shades | Where-Object { $_.B -lt $_.R -or $_.B - $_.R -gt 40 }) {
    Fail "${Name}: faces are not the bluish grey body colour"
  } else { Pass "${Name}: body drawn with $($stats.Shades.Count) face shades" }
  return $stats
}

# Side faces relative to the top face (the brightest shade).
function Side-Ratio($Stats) {
  $top = $Stats.Shades[0].Pixels
  $sides = ($Stats.Shades | Select-Object -Skip 1 | Measure-Object -Property Pixels -Sum).Sum
  return $sides / $top
}

Write-Host '--- Demo block'
$log = Invoke-Mitcad 'demo' @('--demo', '--screenshot', (Join-Path $Out 'demo.png'))
if ($log -match 'OpenGL renderer: (.*)') { $renderer = $Matches[1].Trim() } else { $renderer = '' }
if ($renderer -like 'llvmpipe*') { Pass "OpenGL renderer: $renderer" }
else { Fail "OpenGL renderer is '$renderer', expected Mesa's llvmpipe" }
$null = Check-Shot 'demo'
# mitcad#251: the Docked window's Windows 11 frame through DWM when it
# shows, or skipped before Windows 11; never refused, also not while the
# window goes at the end. Then the other theme while the window shows: the
# frame is applied again in the new theme's colours.
$applied = '^Window frame: (Windows 11 caption colour, Mica, rounded corners|skipped \(Windows build [0-9]+\))$'
function Frame-Lines([string]$Log) {
  return @($Log -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ -like 'Window frame: *' })
}
# @(...) at every assignment: PowerShell returns a one-line array as the
# line itself, whose [0] is its first character.
$frames = @(Frame-Lines $log)
if ($frames.Count -ge 1 -and $frames[0] -match $applied) { Pass "when the window shows: $($frames[0])" }
else { Fail "the window frame was not applied when the window showed: '$($frames -join ' | ')'" }
$refused = @($frames | Where-Object { $_ -notmatch $applied })
if ($refused.Count -eq 0) { Pass 'the window frame was never refused' }
else { Fail "the window frame was refused: '$($refused -join ' | ')'" }
$theme = if ($log -match 'Window style: dark,') { 'light' } else { 'dark' }
$log = Invoke-Mitcad "frame-$theme" @('--demo', '--screenshot', (Join-Path $Out "frame-$theme.png")) @{
  'MITCAD_TEST_COLOR_SCHEME' = $theme }
$lines = @($log -split "`r?`n" | ForEach-Object { $_.Trim() })
$switched = [Array]::IndexOf($lines, ($lines | Where-Object { $_ -like "Window style: $theme,*" } | Select-Object -First 1))
$after = @(if ($switched -ge 0) { $lines[$switched..($lines.Count - 1)] | Where-Object { $_ -like 'Window frame: *' } })
if ($switched -lt 0) { Fail "the window's style did not turn $theme (MITCAD_TEST_COLOR_SCHEME)" }
elseif ($after.Count -ge 1 -and $after[0] -match $applied) { Pass "after the switch to ${theme}: $($after[0])" }
else { Fail "the window frame was not applied again after the switch to ${theme}: '$($after -join ' | ')'" }
$refused = @(Frame-Lines $log | Where-Object { $_ -notmatch $applied })
if ($refused.Count -eq 0) { Pass "the window frame was never refused ($theme)" }
else { Fail "the window frame was refused ($theme): '$($refused -join ' | ')'" }

Write-Host '--- Component colors and appearance restoration'
$colorScript = Join-Path $Out 'component-colors.script'
@"
command view.component_colors
expect-log 'Component colors on'
screenshot-view '$((Join-Path $Out 'component-colors.png') -replace "'", "''")'
command view.component_colors
expect-log 'Component colors off'
screenshot-view '$((Join-Path $Out 'component-restored.png') -replace "'", "''")'
"@ | Set-Content -Encoding ascii $colorScript
$log = Invoke-Mitcad 'component-colors' @('--demo') @{ 'MITCAD_TEST_INPUT' = $colorScript }
Expect-Log $log 'TestDriver: done' 'Component colors: the script ran'
$coloredPath = Join-Path $Out 'component-colors.png'
if (Test-Path $coloredPath) {
  $colored = [Shot]::Analyze($coloredPath)
  if ($colored.Shades | Where-Object { $_.R -gt $_.B + 40 }) {
    Pass 'Component colors: root component drawn in its identification color'
  } else { Fail 'Component colors: no identification color in the view' }
} else { Fail 'Component colors: no screenshot' }
$null = Check-Shot 'component-restored'

Write-Host '--- Open a project file, then change a parameter'
$file = Join-Path $Out 'block.mitcad'
@'
{
  "format": "mitcad",
  "version": 1,
  "parameters": [
    { "name": "d1", "value": 60.0 }, { "name": "d2", "value": 40.0 },
    { "name": "d3", "value": 20.0 }, { "name": "d4", "value": 3.0 }
  ],
  "features": [
    { "type": "sketch", "name": "Sketch1", "shapes": [
      { "type": "rectangle", "corner": [0.0, 0.0], "width": "d1", "height": "d2" } ] },
    { "type": "extrude", "name": "Extrude1", "sketch": "Sketch1", "profile": 0,
      "distance": "d3", "operation": "new_body" },
    { "type": "fillet", "name": "Fillet1", "body": "Extrude1", "edges": [
      { "extrude": "Extrude1", "role": "side", "index": 0 },
      { "extrude": "Extrude1", "role": "side", "index": 1 },
      { "extrude": "Extrude1", "role": "side", "index": 2 },
      { "extrude": "Extrude1", "role": "side", "index": 3 } ], "radius": "d4" }
  ]
}
'@ | Set-Content -Encoding ascii $file
$log = Invoke-Mitcad 'block20' @('--open', $file, '--screenshot', (Join-Path $Out 'block20.png'))
Expect-Log $log 'Opened .*block\.mitcad' 'opened with --open'
Expect-Log $log 'Recomputed 3 feature\(s\)' 'sketch, extrude and fillet recomputed'
if ($log.Contains('Recompute failed')) { Fail 'a recompute failed' }
$low = Check-Shot 'block20'

$log = Invoke-Mitcad 'block35' @('--open', $file, '--set', 'd3=35', '--screenshot', (Join-Path $Out 'block35.png'))
Expect-Log $log 'Recomputed 2 feature\(s\)' 'd3=35 recomputed extrude and fillet'
$high = Check-Shot 'block35'
if ($low -and $high -and $low.Shades.Count -ge 2 -and $high.Shades.Count -ge 2) {
  $lowRatio = Side-Ratio $low
  $highRatio = Side-Ratio $high
  $message = 'side faces / top face {0:0.00} -> {1:0.00}' -f $lowRatio, $highRatio
  if ($highRatio -gt 1.3 * $lowRatio) { Pass "block drawn taller with d3=35: $message" }
  else { Fail "block not drawn taller with d3=35: $message" }
}

# The result store (P7d): the first open stores the extrusion and the fillet
# (however quick), the second takes them from the store.
$results = Join-Path $Out 'results'
Remove-Item -Recurse -Force $results -ErrorAction SilentlyContinue
$store = @{ 'MITCAD_RESULT_STORE' = $results; 'MITCAD_RESULT_STORE_MIN_MS' = '0' }
$log = Invoke-Mitcad 'store1' @('--open', $file, '--screenshot', (Join-Path $Out 'store1.png')) $store
Expect-Log $log 'Result store: restored 0, evaluated 3; stored 2 ' 'the first open stored two results'
$log = Invoke-Mitcad 'store2' @('--open', $file, '--screenshot', (Join-Path $Out 'store2.png')) $store
Expect-Log $log 'Result store: restored 2, evaluated 1; stored 0 ' 'the second open took them from the store'
Expect-Log $log 'Recomputed 1 feature\(s\)' 'only the sketch computed'
$stored = Check-Shot 'store2'
if ($low -and $stored -and $stored.Shades.Count -eq $low.Shades.Count) {
  Pass 'the stored block is drawn as the computed one'
}

Write-Host '--- A fillet whose edges a sketch change renamed (mitcad#272)'
# mitcad-cli builds it (as tools/cli/tests/renamed_face_refs.json): an
# outline whose corner at (0, 40) is the arc c11, circles at the three other
# corners, extruded; Fillet1 rounds the edges where the circles meet the
# lines next to the arc; then the arc goes and the circle c15 takes its
# place, which renames the lines' faces. Opened, the fillet finds its edges
# under their new names and the timeline and the warnings badge show its
# warning.
$cli = @('mitcad-cli.exe', '..\tools\cli\mitcad-cli.exe', 'tools\cli\mitcad-cli.exe') |
  ForEach-Object { Join-Path (Split-Path $App) $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { Fail "no mitcad-cli.exe beside $App" }
else {
  $commands = Join-Path $Out 'renamed.json'
  $renamed = Join-Path $Out 'renamed.mitcad'
  Remove-Item -Force $renamed -ErrorAction SilentlyContinue
  @'
[
  {"cmd": "add_feature", "def": {"type": "sketch", "plane": "xy", "entities": [
    {"id": "p1", "type": "point", "at": [0, 0]},
    {"id": "p2", "type": "point", "at": [60, 0]},
    {"id": "p3", "type": "point", "at": [60, 40]},
    {"id": "p4", "type": "point", "at": [5, 40]},
    {"id": "p5", "type": "point", "at": [0, 35]},
    {"id": "p6", "type": "point", "at": [5, 35]},
    {"id": "c7", "type": "line", "start": "p1", "end": "p2"},
    {"id": "c8", "type": "line", "start": "p2", "end": "p3"},
    {"id": "c9", "type": "line", "start": "p3", "end": "p4"},
    {"id": "c10", "type": "line", "start": "p5", "end": "p1"},
    {"id": "c11", "type": "arc", "center": "p6", "start": "p4", "end": "p5"},
    {"id": "c12", "type": "circle", "center": "p1", "radius": 5},
    {"id": "c13", "type": "circle", "center": "p2", "radius": 5},
    {"id": "c14", "type": "circle", "center": "p3", "radius": 5}]}},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c7[c12,c13],c8[c13,c14],c9[c14,c11],c10[c11,c12],c11[c9,c10],c12[c7,c10],c13[c8,c7],c14[c9,c8]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}},
  {"cmd": "add_feature", "def": {"type": "fillet", "body": "F2.b0", "radius": 1, "edges": [
    "E{F2:side(c14[c9,c8])|F2:side(c9[c14,c11])}",
    "E{F2:side(c10[c11,c12])|F2:side(c12[c7,c10])}"]}},
  {"expect": {"feature": "Fillet1", "status": "ok"}},
  {"cmd": "sketch.remove", "sketch": "F1", "items": ["c11"]},
  {"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 40], "diameter": 12},
  {"expect": {"feature": "Fillet1", "status": "warning"}}
]
'@ | Set-Content -Encoding ascii $commands
  $made = & $cli run $commands --save $renamed
  if ($LASTEXITCODE -ne 0 -or -not (Test-Path $renamed)) { Fail "mitcad-cli: $($made -join ' / ')" }
  else {
    $driver = Join-Path $Out 'renamed.script'
    @'
expect-log 'Feature warnings: .*Fillet1: edge E\{F2:side\(c14\[c9,c8\]\)\|F2:side\(c9\[c14,c11\]\)\} found as E\{F2:side\(c14\[c9,c8\]\)\|F2:side\(c9\[c14,c15\]\)\} after a change in Sketch1' 30
expect-log 'Warnings badge: .*Fillet1: edge E\{F2:side\(c14\[c9,c8\]\)\|F2:side\(c9\[c14,c11\]\)\} found as ' 30
expect-log 'found as E\{F2:side\(c10\[c15,c12\]\)\|F2:side\(c12\[c7,c10\]\)\} after a change in Sketch1' 30
'@ | Set-Content -Encoding ascii $driver
    $log = Invoke-Mitcad 'renamed' @('--no-recovery', '--open', $renamed) @{ 'MITCAD_TEST_INPUT' = $driver }
    Expect-Log $log 'Opened .*renamed\.mitcad' 'the design with the renamed faces opened'
    if ($log.Contains('Recompute failed')) { Fail 'a recompute of the design with the renamed faces failed' }
    Expect-Log $log 'TestDriver: done' 'the timeline and the warnings badge show the fillet that found its renamed edges'
  }
}

# A crash (mitcad#76), of the window's thread and of the model's worker
# thread (where OCCT's handlers are, the C runtime's signal): the report's
# stack is walked from the fault, so it starts at the crashing function and
# not in the handlers, and names it from the PDB file next to the build's
# executable.
foreach ($where in @('app', 'model-worker')) {
  $crashes = Join-Path $Out "crashes-$where"
  Remove-Item -Recurse -Force $crashes -ErrorAction SilentlyContinue
  $log = Invoke-Mitcad "crash-$where" @('--no-recovery', '--demo') @{
    'MITCAD_TEST_CRASH' = $where; 'MITCAD_CRASH_DIR' = $crashes } -Crash
  Expect-Log $log "MITCAD_TEST_CRASH=$where\)" "${where}: crashed for the test"
  $report = Get-ChildItem -Path $crashes -Filter 'crash-*.crash' -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $report) { Fail "${where}: no crash report written"; continue }
  $lines = Get-Content $report.FullName
  $stack = $lines[([Array]::IndexOf($lines, 'stack:') + 1)..([Array]::IndexOf($lines, 'actions:') - 1)]
  $stack | Select-Object -First 4 | ForEach-Object { Write-Host "     $_" }
  if ($lines -notcontains 'signal: EXCEPTION_ACCESS_VIOLATION') { Fail "${where}: the report is not of an access violation" }
  elseif ($stack.Count -lt 3) { Fail "${where}: the report's stack has $($stack.Count) frames" }
  elseif ($stack[0] -match '\\mitcad\.exe!mitcad::crash::crashNow\+0x[0-9a-f]+$') {
    Pass "${where}: the report's stack starts at the crashing function, named"
  } else { Fail "${where}: the report's stack starts at $($stack[0]), not at mitcad::crash::crashNow" }
}

# View > Rendered (mitcad#51): the app starts mitcad-render.exe next to it
# (every build has it), whose frames (through the named shared memory)
# refine to all samples; the rendered view shows the block.
$worker = Join-Path (Split-Path $App) 'mitcad-render.exe'
if (-not (Test-Path $worker)) {
  Fail "View > Rendered: no render worker $worker next to the app"
} else {
  $script = Join-Path $Out 'render.script'
  function Shot-Path([string]$Name) { return (Join-Path $Out "$Name.png") -replace "'", "''" }
  # mitcad#216: once the render is done and the camera keeps still, the
  # rendered image stays over the view (`Render image dropped` would say
  # the view drew the bodies shaded instead).
  function Still([string]$Name) {
    return @"
wait-log 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples 16 ' 60
sleep 500
mark
sleep 1500
screenshot-view '$(Shot-Path $Name)'
mark
"@
  }
  @"
screenshot-view '$(Shot-Path 'shaded')'
command view.rendered
expect-log 'Rendered view on'
$(Still 'rendered')
# mitcad#214: no view goes to the worker while a button is held, however
# long it keeps still; one when it goes up.
view-pointer press 40 50 middle shift
view-pointer move 45 48 middle shift
view-pointer move 50 46 middle shift
view-pointer move 55 45 middle shift
sleep 700
expect-no-log 'Render view [0-9]+: (orthographic|perspective) '
view-pointer release 55 45 middle shift
wait-log 'Render view [0-9]+: first frame' 60
$(Still 'orbited')
# A burst of wheel steps 50 ms apart: one view, after the quiet time.
wheel-burst 50 50 -1 5
wait-log 'Render view [0-9]+: first frame' 60
$(Still 'zoomed')
# The orientation cube turns the view to the front, animated.
click-logged 'Orientation cube front at'
wait-log 'Render view [0-9]+: first frame' 60
$(Still 'front')
command view.rendered
expect-log 'Rendered view off'
"@ | Set-Content -Encoding ascii $script
  $log = Invoke-Mitcad 'rendered' @('--no-recovery', '--demo') @{
    'MITCAD_TEST_INPUT' = $script; 'MITCAD_RENDER_SAMPLES' = '16'; 'MITCAD_LOG_ORIENTATION_CUBE' = '1' }
  Expect-Log $log 'TestDriver: done' 'View > Rendered: the script ran'
  $frames = ([regex]::Matches($log, 'Render frame view [0-9]+ [0-9]+x[0-9]+ samples ')).Count
  if ($frames -ge 2) { Pass "View > Rendered: the view refined in $frames frames" }
  else { Fail "View > Rendered: $frames frames shown" }
  # The part of the log from a step of the script to the next mark.
  function Log-Part([string]$Step) {
    $part = [regex]::Match($log, "TestDriver: line [0-9]+: $Step[\s\S]*?TestDriver: line [0-9]+: mark")
    if ($part.Success) { return $part.Value } else { return $null }
  }
  # The views sent between a step of the script and the next mark.
  function Views-Sent([string]$Step) {
    $part = Log-Part $Step
    if ($null -eq $part) { return -1 }
    return ([regex]::Matches($part, 'Render view [0-9]+: (orthographic|perspective) ')).Count
  }
  $sent = Views-Sent 'view-pointer press'
  if ($sent -eq 1) { Pass 'View > Rendered: an orbit sent one view, after the release' }
  else { Fail "View > Rendered: an orbit sent $sent views (1 wanted)" }
  $sent = Views-Sent 'wheel-burst'
  if ($sent -eq 1) { Pass 'View > Rendered: a burst of 5 wheel steps sent one view' }
  else { Fail "View > Rendered: a burst of 5 wheel steps sent $sent views (1 wanted)" }
  # mitcad#216: the image stays while the camera keeps still.
  foreach ($still in @('rendered', 'orbited', 'zoomed', 'front')) {
    # From the pause before the screenshot to the screenshot itself.
    $part = [regex]::Match($log, "TestDriver: line [0-9]+: sleep 1500((?:(?!TestDriver: line)[\s\S])*)" +
      "TestDriver: line [0-9]+: screenshot-view [^\r\n]*[\\/]$still\.png")
    if (-not $part.Success) { Fail "View > Rendered ($still): the script did not get there"; continue }
    $dropped = ([regex]::Matches($part.Groups[1].Value, 'Render image dropped')).Count
    if ($dropped -eq 0) { Pass "View > Rendered ($still): the rendered image stays while the camera keeps still" }
    else { Fail "View > Rendered ($still): the rendered image was dropped $dropped times while the camera kept still" }
  }
  $path = Join-Path $Out 'rendered.png'
  if (-not (Test-Path $path)) { Fail 'View > Rendered: no screenshot' }
  else {
    $stats = [Shot]::Analyze($path)
    Write-Host ("     rendered: {0}x{1}, background {2}, body {3:0.0}%" -f $stats.Width, $stats.Height,
      $stats.Background, $stats.Percent($stats.FacePixels))
    if ($stats.Percent($stats.FacePixels) -ge 10) { Pass 'View > Rendered: the block is in the rendered view' }
    else { Fail 'View > Rendered: no block in the rendered view' }
    # The rendered view is not the shaded one (mitcad#216: it was, as the
    # image was dropped before every frame).
    $difference = [Shot]::MeanDifference((Join-Path $Out 'shaded.png'), $path)
    Write-Host ("     rendered vs shaded: mean difference {0:0.00}" -f $difference)
    if ($difference -ge 3) { Pass 'View > Rendered: the view shows the render, not the shaded bodies' }
    else { Fail "View > Rendered: the view looks shaded (mean difference $difference from the shaded view)" }
  }

  # mitcad#226: the app ends with Render Image and Render Environment open:
  # the main window deletes the 3D view before the dialogs that watch it,
  # which crashed at the close.
  $script = Join-Path $Out 'close-dialogs.script'
  @"
command file.render_image
expect-log 'Render image dialog opened'
command view.render_environment
expect-log 'Render environment dialog opened'
"@ | Set-Content -Encoding ascii $script
  $crashes = Join-Path $Out 'crashes-close'
  if (Test-Path $crashes) { Remove-Item -Recurse -Force $crashes }
  $log = Invoke-Mitcad 'close-dialogs' @('--no-recovery', '--demo') @{
    'MITCAD_TEST_INPUT' = $script; 'MITCAD_CRASH_DIR' = $crashes }
  Expect-Log $log 'TestDriver: done' 'Closing with the render dialogs open: the script ran'
  if ((Test-Path $crashes) -and (Get-ChildItem -Recurse -File $crashes)) {
    Fail 'Closing with the render dialogs open: a crash report was written'
  } else { Pass 'Closing with the render dialogs open: no crash report' }
}

Write-Host "Screenshots and logs: $Out"
if ($failures -gt 0) { Write-Host "FAIL: UI Windows test ($failures failures)"; exit 1 }
Write-Host 'PASS: UI Windows test'
