<!-- SPDX-License-Identifier: MIT -->
# Native macOS UI

The AppKit frontend uses the existing CAD model, commands, file formats,
undo history, imports and OCCT viewport. Its outer window and general
controls are AppKit. The shared session owns a single document; enabling
the frontend does not introduce multiple documents or a new renderer.

```bash
cmake --preset dev -DMITCAD_MACOS_NATIVE_UI=ON
cmake --build --preset dev
ctest --preset dev --output-on-failure
```

Run these commands in the isolated macOS VM. The deployment target stays
14.4; newer AppKit APIs need availability guards. In a native-enabled build
the frontend is selected by default. `--ui=native` selects it explicitly;
`--ui=qt` and `--chrome=docked|floating` select the retained Qt frontend.

## Interaction

- The top CAD ribbon retains its command groups and
  coloured CAD icons from the shared command registry, matching the sketch
  palette. Native AppKit buttons, system typography and menu controls supply
  the macOS appearance. Utility controls retain native SF Symbols.
  Ribbon buttons have eight-point gaps, with wider spacing between groups.
  Model and sketch workspaces keep their existing command membership and
  order; grouped menus expose additional commands. Narrow windows collapse
  trailing icons into those menus while keeping Finish Sketch visible.
  Group menus use the same workspace membership as the shared Qt ribbon.
  Repeated action snapshots reuse native artwork and preserve menu objects
  while only availability or checked states change. Artwork is cached by its
  Qt icon key, symbol, palette, template mode and logical size; palette changes
  discard the cached images. Shortcut bindings are parsed with each changed
  action structure rather than on every key event. Overlay layout requests
  repaint the viewport only when its size or backing scale changes.
  Settings, view controls and project utilities keep their intended menu-bar
  locations; search-only settings pages stay available in command search.
- The OCCT viewport spans the full document window width below the ribbon.
  The model browser floats over its left edge and the inspector floats on
  the right, leaving the canvas width unchanged when either card is hidden.
  The cards use [NSGlassEffectView](https://developer.apple.com/documentation/appkit/nsglasseffectview)
  on macOS 26 and newer, with NSVisualEffectView on earlier supported systems.
  Rounded edges and shadows stay within the document window, with
  unobstructed canvas gaps around the cards.
- Trackpad magnification, rotation and smart zoom enter through the native
  document window and reach the shared CAD viewer with explicit local
  coordinates. A gesture starting on a floating card stays outside the
  canvas; a gesture starting on the canvas keeps its target until it ends.
  The existing viewer supplies zoom sensitivity and navigation settings.
- The model browser shows selections, names and visibility. Its context
  menu uses the shared browser command semantics. The outline and its
  scroll view stay transparent over the rounded native card.
- The inspector stays hidden until a command, sketch or selection needs it.
  It hosts one native command form, selection details or the sketch palette.
  Expressions, decimal commas, choices, repeated inputs, selection lists,
  errors and previews use the existing command definitions. Icon choices use
  native buttons with the shared CAD icons and option labels as tooltips.
  Disclosure rows reveal their dependent inputs while keeping the inspector's
  height cap and scrolling rules.
- View controls show or hide the model, inspector and feature-history
  cards. Feature history is a compact horizontal timeline of the shared
  coloured CAD icons in native buttons, with its marker between features
  and native step controls. It floats above the canvas and supports edit,
  selection, reordering and marker movement. Resizing keeps visible cards
  inside the document and preserves their input routing above the embedded
  viewport. Marker drags use the shared rollback and undo transaction.
- Settings, parameters, appearances, version/library/remote dialogs,
  reports, progress and alerts bind existing workflow presenters to native
  controls. File open/save panels use NSOpenPanel/NSSavePanel, and colours
  use NSColorPanel. AppKit text responders handle normal editing shortcuts.
- Preview, inspection and commit jobs run on the model worker. A slow commit presents a
  cancellable progress sheet. Closing during a command waits for cancel
  compensation before asking to save and terminating.
- Checkpoints for commands in the native inspector restore the original
  definition, saved state and undo/redo history even when Cancel arrives
  just after a computation committed. Accepted commands keep the usual
  single undo step.
- Sketch precision fields are AppKit text fields anchored to the canvas.
  They share the sketch controller's expressions, validation and Tab order.

The sketch editor retains its existing shared marker and undo/merge
workflow. Inspector command checkpoints do not replace that whole sketch
edit transaction.

Keyboard Shortcuts validates commands in their available contexts and keeps
conflicting saved assignments visible for repair. Native menus and key
dispatch use the registry's effective primary and alternate bindings.
Recording, menu equivalents and event dispatch share the Qt/AppKit key
conversion; Command remains Qt's portable Ctrl and physical Control its
Meta. The recorder's Clear button removes a binding, so Backspace and
forward Delete can themselves be assigned.

With Browser or history focus, F2 renames and Backspace or forward Delete
deletes through the shared controllers. Return activates the selected Browser
row, opening its sketch or feature editor; Return in a row's text editor
finishes renaming.
Command+F opens native timeline search, sharing the query, component/status
filters, result navigation and selection restoration with the Qt timeline.
Command+W in command or timeline search dismisses that search and keeps the
document open. Removing Project through toolbar customisation remains in
effect after saves and other project-status changes.
Text editors keep their normal editing keys; document shortcuts do not
intercept input in an attached modal sheet. In Save with Description,
Command+Return saves and plain Return inserts a newline.

The #187 keyboard changes were initially integrated without executing builds
or tests at the owner's request. On 2026-10-23, the physical M1 Max native
workflow passed whole AppKit Undo/Redo events, cancellation preserving the
redo history, search dismissal and the existing command/form scenarios.
The whole-event test bridge now waits for actual application activation and
its own key window before posting events; simulated delegate notifications
alone do not establish AppKit keyboard focus (mitcad#559). This scoped run
does not complete the full keyboard matrix. Outstanding manual scenarios
remain in [the macOS checklist](macos-trackpad-checklist.md).

## Viewport anti-aliasing measurements

The normal shaded viewport retains four MSAA samples on every backing
scale. `MITCAD_VIEWPORT_AA=msaa4|msaa2|off|ray-aa` is an opt-in diagnostic
override, shared by the Qt and native frontends. It changes neither the
rendered view's resolution nor the user's saved settings. Invalid values
warn and retain four samples. No scale-dependent or navigation-dependent
policy is selected without physical-display measurements (mitcad#552).

`MITCAD_LOG_TIMING=1` reports the GL renderer/version, screen refresh rate,
logical and backing dimensions, backing scale, `GL_MAX_SAMPLES` and the
configured OCCT sample count. Each `Timing frame` line also includes its
current backing dimensions and scale, including after a display move.
The configured sample count is a request: the driver or OCCT may reduce it;
`GL_MAX_SAMPLES` alone does not prove the internal framebuffer's allocation.

`Timing frame` measures CPU time inside `paintGL`, including any driver
waits. `Timing frame swap` is the interval between Qt's `frameSwapped`
signals, after composition; it includes input scheduling and is not a GPU
timestamp or a measurement of physical scanout. Optional
`MITCAD_VIEWPORT_SYNC=1` finishes this context's GL work before ending the
paint timer, adding GPU completion waits while excluding Qt composition
and presentation. That diagnostic fence changes pipelining; keep fenced
and unfenced results separate and retain the normal unfenced comparison.

Run the bounded synthetic dome and pointer navigation harness serially on
the physical Mac, using the application's companion CLI:

```sh
bash tools/measure-viewport-aa-macos.sh "$APP" "$CLI" "$OUT/aa-submit"
MITCAD_VIEWPORT_SYNC=1 bash tools/measure-viewport-aa-macos.sh "$APP" "$CLI" "$OUT/aa-sync"
```

The harness starts fresh isolated settings for each mode, fixes the window
at 1280 × 820 logical pixels, fits the same synthetic dome, warms up with
30 pointer moves and measures 60 further moves on the same circular orbit.
The in-process driver sends real viewer pointer events without external
input permissions. It checks that all 90 moves ran, that the camera changed
during the orbit and that the release cleared held buttons. Results include
raw logs, frame/swap samples, median and maximum milliseconds, resting
screenshots, and OS/display metadata. Review the images for line quality
and frame sample counts before interpreting a comparison. Keep identifying
display metadata outside published reports.

The fourth `ray-aa` mode explicitly tests the issue's suggested
`IsAntialiasingEnabled` flag with MSAA disabled. In the pinned OCCT 8.0.1
source that flag controls adaptive FSAA in `OpenGl_View_Raytrace.cxx`;
it does not enable FXAA for this raster shaded viewport. Its result must
be described as a ray-tracer flag control, not evidence for raster FXAA.
Implementing a separate raster postprocess is outside this measurement.

A genuine 1× comparison requires verifying the actual display's backing
scale using its display mode and runtime log. The harness rejects Qt scale
overrides: software scaling does not establish a physical 1× display.
When only 2× hardware is available, record that limitation and leave the
1× acceptance item open.

### Physical 2× result, 2026-10-23

The synthetic fixture was the fully revolved dome shell with 4.45 and
4.55 m radii from `tools/ui-fixtures/viewport-dome.json`, with the half-dome
feature suppressed. The controlled comparison ran on a physical Apple M1 Max
(32 GPU cores), macOS 27.0.1 build 26A434, with OpenGL reporting
`4.1 Metal - 91.7`. The built-in display reported 120 Hz and a 2× backing
scale. After the harness resize, the measured viewport was 2560 × 1380
backing pixels; its initial environment line reported 2560 × 1340 before
that resize. `GL_MAX_SAMPLES` was four. Each mode completed all 30 warmup
and 60 measured pointer moves, with 60 measured paint and swap samples.

Unfenced results, in milliseconds (median / maximum):

| Configured mode | Paint | Qt frame-swap interval |
| --- | ---: | ---: |
| MSAA 4 | 1.37 / 4.44 | 19.73 / 28.39 |
| MSAA 2 | 1.25 / 4.15 | 19.55 / 30.16 |
| MSAA off | 0.80 / 3.26 | 20.24 / 32.47 |
| Ray-tracer AA flag, MSAA off | 2.12 / 3.44 | 19.69 / 32.18 |

The input driver paced these swap intervals; they do not establish
sustained 120 Hz navigation. The ray-tracer flag control also varied from
the equivalent raster MSAA-off configuration, so differences between these
single runs must not be treated as a repeatability estimate.

The separate diagnostic run with `MITCAD_VIEWPORT_SYNC=1` also completed
all 60 measured paint and swap samples per mode. GPU-completion-inclusive
paint durations and Qt swap intervals, in milliseconds (median / maximum):

| Configured mode | Paint with GL completion wait | Qt frame-swap interval |
| --- | ---: | ---: |
| MSAA 4 | 4.43 / 6.42 | 19.66 / 30.87 |
| MSAA 2 | 3.05 / 6.52 | 19.52 / 30.98 |
| MSAA off | 2.87 / 7.32 | 19.32 / 32.92 |
| Ray-tracer AA flag, MSAA off | 2.59 / 4.44 | 19.16 / 31.23 |

Retain MSAA 4. MSAA 2 reduced median completion-inclusive paint duration
by 1.38 ms (31%) in this bounded workload, but its maximum did not improve
and the input-paced swap intervals showed no established cadence gain.
MSAA 4's largest completion-inclusive paint duration was below the
display's nominal 8.33 ms period in this run; that excludes Qt composition
and is not evidence of sustained 120 Hz output or a guarantee for other
designs. These results do not justify lowering resting line quality or
adding a navigation-dependent policy.

No actual 1× display/mode was available at this checkpoint; no Qt scale
override was used as a substitute. The owner subsequently selected a real
non-HiDPI mode, allowing the remaining measurement below. Raster FXAA is
unavailable through the suggested OCCT flag, as described above. Moving
between physical displays remains a separate hardware validation gap.

### Physical 1× result, 2026-10-24

The owner switched the built-in panel to its native 3456 × 2234 non-HiDPI
mode. AppKit reported a 3456 × 2234 logical screen and
`backingScaleFactor=1.0`; CoreGraphics independently reported both logical
and pixel dimensions as 3456 × 2234 at 120 Hz. The panel's marketing
"Retina" label in system metadata does not describe its current backing
scale. No Qt scale override was used.

The physical Apple M1 Max (32 GPU cores), macOS 27.0.1 build 26A434 and
OpenGL `4.1 Metal - 91.7` match the earlier hardware. This run used the
validated arm64 Qt RelWithDebInfo build with C++/Objective-C++ ThinLTO from
`ffe13896`; the earlier 2× checkpoint used a Debug profiling build.
Consequently these are separate within-build mode comparisons: differences
between their absolute times cannot be attributed solely to backing scale.
The harness and synthetic dome fixture are byte-identical to the 2× run.

Each of the four modes completed 30 warmup and 60 measured pointer moves
in both unfenced and fenced runs. Every measured paint recorded scale 1,
a 1280 × 690 backing viewport and the expected GPU-sync setting. Each mode
provided 60 paint and 60 frame-swap samples. The initial environment line
reported 1280 × 670 before the harness resize. The renderer was Apple M1 Max
and `GL_MAX_SAMPLES` was four; configured sample counts retain the
request-versus-allocation limitation described above.

Unfenced results, in milliseconds (median / maximum):

| Configured mode | Paint | Qt frame-swap interval |
| --- | ---: | ---: |
| MSAA 4 | 1.33 / 3.92 | 17.86 / 31.52 |
| MSAA 2 | 1.33 / 3.43 | 18.19 / 29.33 |
| MSAA off | 0.90 / 2.83 | 18.24 / 29.31 |
| Ray-tracer AA flag, MSAA off | 1.28 / 2.84 | 18.46 / 31.13 |

Separate `MITCAD_VIEWPORT_SYNC=1` results, in milliseconds (median / maximum):

| Configured mode | Paint with GL completion wait | Qt frame-swap interval |
| --- | ---: | ---: |
| MSAA 4 | 2.22 / 4.19 | 18.07 / 28.86 |
| MSAA 2 | 2.55 / 4.13 | 18.21 / 30.14 |
| MSAA off | 2.23 / 4.80 | 18.08 / 30.54 |
| Ray-tracer AA flag, MSAA off | 2.02 / 3.93 | 18.38 / 31.40 |

Retain MSAA 4 at 1× as well. MSAA 2 did not lower the unfenced paint median
and its completion-inclusive median was higher in this bounded run. The
small maximum differences and input-paced swap intervals do not justify
lowering line quality. Resting MSAA 4/off captures were visually reviewed;
the four-sample image retains the smooth dome outline and grid lines.
These single runs are not a repeatability estimate, physical scanout or
input-latency measurement, and the ray-tracer control is not raster FXAA.

The real 1× acceptance gap for #552 is now covered. Combined with the
earlier physical 2× result, no adaptive sample policy is justified: normal
MSAA 4 remains unchanged. No application code, user preference or display
mode was changed by the measurement harness. Live moves between different
physical displays remain untested; no new scale-dependent behaviour needs
that transition for this decision.

## Validation

`app.ui-macos-native` starts and cancels Offset Plane through its familiar
pinned CAD ribbon button, checks Extrude stays disabled without profiles,
then creates a 40 × 25 × 20 mm box through the native menu and AppKit controls,
rejects an invalid expression, edits its height to 10 mm through native
history, verifies undo/redo, cancels a preview and exercises command search.
Repeated native inputs retain their values after adding and removing rows,
and their controls unlock after each structural action is applied.
Native outline selection drives worker measurements, canvas Return and
Browser Escape close the inspector, and Physical Properties presents a
native report dialog. Command search checks Command+W dismissal without
closing the document. The native sketch workflow checks Browser Return
activation for sketches and features, row renaming and command isolation.
The toolbar regression upgrades an older saved layout, checks live status
after a customization-palette request, removes Project, changes projects,
and verifies that removal persists in a second application process.
The secondary workflow checks BUILD direction buttons and the reusable
form contract for exclusive icon choices and expanding/collapsing rows.
Model-volume assertions check the persisted result. The native test bridge
uses AppKit target/action and the field editor without external
Accessibility permission. The retained Qt workflow tests still run with
`--ui=qt`.

The native workflow also dispatches activation and deactivation through the
installed application delegate, checking Qt's state signals and deactivate
events. Dock reopen must restore the active state without another activation
notification, and secure state restoration remains provided by Qt's delegate.

The same workflow checks that the viewport keeps the full content width
below the CAD ribbon when the model and inspector cards are shown or
hidden. It checks a compact 900 × 640 point window and the normal 1280 ×
820 point window, card bounds and AppKit hit testing: cards receive pointer
input above the canvas, and their outer gaps pass input to the canvas.
Command and sketch workflows also check the visible inspector layout and
start commands through the real native ribbon buttons. The sketch workflow
checks familiar line/rectangle buttons and finishes through the ribbon.
The layout checks cover ribbon button spacing, group caption bounds,
separators and the transparent browser background. In sketch mode they
compare the actual ribbon and palette constraint bitmaps with the shared
coloured CAD asset at each image's resolution, and reject template tinting.
An idle sketch screenshot shows both icon sets before drawing starts.
Hiding and showing an active inspector is checked with an invalid
expression: its text and disabled confirmation state remain intact.
The actual native ribbon popup menus are compared recursively with the
retained Qt CAD menus for the active workspace, including command order,
submenu structure and checked states. Settings must remain in the Mitcad
application menu, and search-only settings pages must stay out of CAD menus.
The secondary-window and panel tests open the Cache settings page through
native command search.
Native screenshots preserve the captured AppKit cards when compositing the
real OCCT framebuffer beneath them.

`app.ui-macos-native-history` creates three independent bodies and checks
their actual counts and volumes at each marker position. It compares the
native feature bitmaps with the shared CAD assets in light and dark
appearance, verifies the compact transparent strip and in-row marker,
and checks Shift/Cmd selection across appearance and window-size changes.
Native double-click editing, cancellation, step controls and hide/show
use the real feature controls. Separate AppKit mouse-down, drag and
mouse-up events verify that a multi-position marker drag is one undo step,
that undo/redo restores the correct bodies, and that returning to the
starting position adds no undo entry. After clearing cached geometry, the
test sends a new drag target and releases the pointer while the real model
worker is still computing, then checks the final marker and single undo.
Feature and marker hit tests also verify pointer routing above the canvas.
Native feature drag/drop reordering
and physical pointer/trackpad feel remain hardware checklist checks.

`app.ui-macos-native-gestures` dispatches in-process magnification events
through the actual document NSWindow and compares the resulting OCCT
camera height with a direct Qt gesture control. It checks pinch in/out,
single delivery, suppression over the browser and preservation of sketch
precision input during zoom. The test uses a test-only NSEvent subclass
because AppKit exposes magnification as a read-only property. It verifies
event routing and camera behaviour; physical trackpad recognition and
gesture feel still require the [hardware checklist](macos-trackpad-checklist.md).

Native menu tracking, sheet presentation and document focus loss cancel
the viewport's outstanding pointer interaction. The shared viewer also
recovers from a consumed release when it next receives motion with no
button down. Cancellation drops pending pointer deltas and selections,
preserves explicit camera commands, and never turns the interrupted drag
into a click. Context-menu signals are emitted after the release reaches
the view controller.

Shared Qt context-menu definitions are presented by an owned AppKit popup
with explicit action targets, preserving icons, enabled and checked states,
submenus and shortcuts. The selected guarded QAction is triggered once
after tracking ends. The native document's responder chain cannot dispatch
Qt's implicit Cocoa menu-loader target.

`app.ui-macos-native-context-menus` reproduces a consumed right-button
release after an actual orbit and reads the OCCT camera direction and up
vector. It checks recovery and subsequent right dragging, tracks real
canvas, cube, Browser and history menus, and tests both cancellation and
selected actions. A stale viewport press must already be cleared when a
native menu closes, before hover recovery could hide the failure. Browser
visibility and feature suppression actions are verified against owner
logs and actual model volumes.

The native cancellation test exercises stale previews, Return during a
pending preview, cancellation during commit, preservation of a prior redo
step and subsequent command startup. Commit cancellation dispatches the
actual native Cancel button at the matching document job-start signal,
with the model busy, before a progress sheet blocks the command form.
The history drag test similarly submits its final target and mouse release
at job start, checking busy before and after each input. It then verifies
the final marker, body set and single undo step after the queued jobs finish.
The secondary-window test edits parameters and previews a local feedback
report through native controls. The sketch test checks invalid expressions,
decimal commas, Tab/Shift+Tab, Return/Escape and a 20,250 mm³ extrusion.
The panel test configures and cancels native open/save panels and nested
report export, including the real format popup. Actual file acceptance is
a manual check: Apple's separate-process Save panels reject programmatic
`ok:` calls. The test bridge uses public panel properties and Cancel,
and never simulates an accepted file. See
[Apple's Save panel guidance](https://developer.apple.com/videos/play/wwdc2019/701/).

The initial native-frontend validation on 2026-10-09 used an arm64 macOS
27.0.1 VM, SDK 27 and Qt 6.12, with deployment target 14.4:

- Debug AppKit build and release builds with the native option both OFF
  and ON passed.
- The complete registered Mac test set ran in two groups: 124 passed,
  zero failed; seven third-party corpus tests skipped because their external
  datasets were unavailable. All five native UI tests passed.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  the dependency/advisory checks passed.
- The release bundle was signed ad hoc; signature and bundle checks passed
  with 58 Mach-O files and 738 library references checked.
- All five native workflows also passed against the packaged release
  application, including history and search under optimized ARC.
- Windows and Linux tests were not run on this machine, per user request.

The floating-layout follow-up on the same VM passed the five native UI
workflows and both retained Qt Mac UI tests. Debug and Release C++ and
Objective-C++ builds had no compiler warnings. The final Debug workflow
also passed the active inspector hide/show validation check. All five
native workflows passed against the ad hoc signed packaged Release app;
signature and bundle checks again passed with 58 Mach-O files and 738
library references. The full-width canvas, overlay input routing, compact
forms and familiar ribbon icons were also checked in captured images.
Windows and Linux tests were not run for this follow-up, per user request.

The ribbon-appearance follow-up on the same VM passed the Debug sketch
and floating-panel workflows, then all five native workflows against the
ad hoc signed packaged Release application. Debug and Release AppKit
builds passed; signature and bundle checks passed with 58 Mach-O files and
738 library references. The native ribbon and palette constraint bitmaps
matched the shared coloured asset at Retina resolution, including
antialiased alpha edges. Captured images confirmed the even icon spacing,
centred group captions and browser content blending into its floating
card. Windows and Linux tests were not run on this machine, per user request.

The trackpad follow-up on 2026-10-10 used the same macOS 27.0.1 VM.
Before the fix, the AppKit pinch regression left the camera unchanged while
its direct Qt control zoomed correctly. After the fix, the Debug gesture
and sketch tests passed. Debug and Release builds passed, and the gesture,
main workflow and panel tests passed against the ad hoc signed packaged
Release application. Signature and bundle checks passed with 58 Mach-O
files and 738 library references. The gesture test fixes the window at
1280 × 820 points so its browser hit stays deterministic. Windows and Linux
tests were not run on this machine, per user request.

The native-menu follow-up on 2026-10-10 reproduced 80 entries in the
original model Create popup versus 21 in the retained CAD menu. The fixed
Debug sketch test passed its recursive menu comparison, including shared
Select submenus. Debug and Release builds passed. Main, secondary-window
and panel workflows passed against the packaged Release app, checking the
model menus and command-search access to the Cache settings page. Ad hoc
signature and bundle checks passed with 58 Mach-O files and 738 library
references. Windows and Linux tests were not run on this machine, per user
request.

The native-history follow-up on 2026-10-10 used the same macOS 27.0.1 VM.
Debug and Release AppKit builds passed without C++ or Objective-C++
compiler warnings. The registered Mac checks ran in groups: 126 passed,
zero failed; seven external corpus cases skipped because their datasets
were unavailable. All seven native workflows passed against the signed
packaged Release app. The history regression verified real body rollback,
one undo per drag, release while computing, exact selection and shared
icon bitmaps in both appearances. Captured images confirmed the compact
horizontal strip and marker between feature icons. The cancellation test
now dispatches at the actual job-start boundary to avoid a progress-sheet
input race. Source license headers, signature, bundle and DMG checks
passed, with 58 Mach-O files and 738 library references checked.
Windows and Linux tests were not run on this machine, per user request.

The context-menu follow-up on 2026-10-10 used the same macOS 27.0.1 VM.
Disabling hover recovery reproduced camera rotation with no button down;
disabling native tracking cancellation independently left the right button
pressed after a Browser popup closed. The final registered Mac checks ran
in groups: 127 passed, zero failed, and seven external corpus cases skipped.
All eight native workflows and the retained Qt workflow passed. Debug and
Release C++/Objective-C++ builds had no compiler warnings; the Rust
updater's existing `rust-objcopy`/`libLLVM` stripping warning remains.
Source license headers, runner syntax, ad hoc signature, bundle and DMG
checks passed, with 58 Mach-O files and 738 library references checked.
The context-menu regression also passed against the signed Release app
included in the DMG.
Windows and Linux tests were not run on this machine, per user request.

### Physical Mac checklist (mitcad#558)

Qt remains the primary/default macOS build and release frontend; AppKit is
an optional, separately configured test build. The physical acceptance
matrix below records the 2026-10-23 physical run separately from historical
VM results. The normal Qt build has `MITCAD_MACOS_NATIVE_UI=OFF`; the
optional AppKit build enables it in a separate build directory.

The available configuration for the 2026-10-23 run is an Apple M1 Max
(32 GPU cores), 32 GiB memory, macOS 27.0.1 (26A434), with the built-in
Retina display (3456 × 2234 backing pixels, 1728 × 1117 logical points).
The runtime `expect-viewport` output records actual backing scale, screen
size, reported refresh rate and OpenGL version. Both frontends reported
scale 2, 120 Hz, `OpenGL renderer: Apple M1 Max` and OpenGL
`4.1 Metal - 91.7` while the built-in display was awake.
An older virtual-display configuration is not evidence for this panel.
`NSSupportsAutomaticGraphicsSwitching` has no GPU-switching effect on this
single-GPU Apple Silicon configuration.

| Checklist item | Qt evidence | AppKit evidence | Physical result |
| --- | --- | --- | --- |
| Render and face pick after resize, window move, two minimize/restore cycles, fullscreen and return | `app.ui-macos-physical` | `app.ui-macos-native-physical` | Passed in light/dark: twelve real framebuffer/face-selection checkpoints per frontend; actual selected face identity is read back |
| Actual 2x backing, framebuffer dimensions and input coordinates | Physical test with `MITCAD_UI_REQUIRE_RETINA=1`; `app.ui-macos-dialogs` | Native physical test and `app.ui-macos-native-context-menus` | Passed without scale overrides; Qt resized from 1280x690/2560x1380 to 1000x520/2000x1040, AppKit fullscreen reached 1728x968/3456x1936 |
| Synthetic pixel-scroll pan/orbit/zoom, pinch and selection | Physical test reads real camera target, orientation, height and selection | Same plus `app.ui-macos-native-gestures` for AppKit gesture routing, alternate scheme and sketch input | Passed; actual camera and released-button state checked; synthetic input does not establish sensor latency |
| Floating browser/command/history cards, compact ribbon, pointer hit testing and menus | Own-window captures and Qt workflow | Physical layout assertions, native history and context-menu tests | Passed; inspected actual light/dark own-window captures including glass; widget-only composites omit platform glass backgrounds |
| Light/dark appearance, native shortcut names, Tab/Return/Escape, editable fields | Physical test in both appearances, dialogs and Qt workflow | Physical test in both appearances, native main/sketch/secondary/gesture tests | Passed; parameter dialogs also passed 119 driver steps per appearance; requests are confined to the test process |
| File browsing, initial folder/name/filter, cancellation and frame restoration | Qt workflow uses shared file helpers and Qt choosers | Native panels test checks real AppKit panels from document and secondary windows | Passed programmatic configuration/cancellation and saved-frame restoration; native panel acceptance remains manual |
| Save/open, recovery, import/export, version history and local remote fixture | Qt workflow with fixture output outside Git | Native main, secondary and replacement workflows | Passed, including independent Git/history checks and recovery/import assertions; secondary/replacement fixtures also ran outside Git |
| Busy previews, latest edited input, cancel during commit and edit restoration on close | Qt command-cancel test | Native cancel test | Passed real progress/form cancellation, actual body volumes and unchanged undo depth; Qt checks actual window close and Cancel at its save question |
| Hardware Metal rendered-view navigation, resizing and worker shutdown | Dedicated render-metal test | Dedicated native-render-metal test | Covered separately by the [Metal acceptance matrix](rendering.md#physical-metal-acceptance-tools) (#185), outside #558 |
| ProMotion orbit pacing | Opt-in physical-test timing sample | Same optional sample | Both reported 120 Hz and about 121 swap callbacks/s in the unlocked session; see samples below; physical presentation/input latency remains unmeasured |
| Sleep/wake, physical trackpad/Magic Mouse input, Natural scrolling and momentum | Untested | Untested | Requires manual hardware interaction; no system sleep or input preference changes performed |
| Actual 1x non-HiDPI display mode | Four-mode physical AA comparison on 24 October 2026 | Native frontend not exercised in this mode | AppKit scale 1.0 and CoreGraphics logical/pixel size both 3456x2234; Qt measured backing viewport 1280x690; see the 1x result above |
| Live display moves, backing-scale transitions and multiple-display arrangements | Untested | Untested | One built-in panel; the owner changed its actual mode before the run, rather than forcing a Qt scale or moving a live window between displays |
| Increased contrast, reduced transparency/motion and VoiceOver navigation | Untested | Untested | Global accessibility preferences are not changed by the unattended harness |
| Native chooser acceptance, overwrite prompts, native colour acceptance/cancellation, Finder dispatch and Quit variants | Untested where not exercised by the workflows | Untested where not exercised by the workflows | Native save acceptance is in a separate system process; no permission-dependent input injection |
| Oldest supported macOS runtime | Untested | Untested | macOS 14.4 runtime unavailable; SDK/deployment target 14.4 does not provide that coverage |

The Qt command path waits synchronously for its worker while a nested
event loop presents `ComputeProgress`. Its busy-command test cancels
through that real modal dialog, then closes the idle command form with
Escape and checks body volumes and undo depth. AppKit uses asynchronous
command continuations; its existing native test can cancel the command
form at the actual job-start signal. Injecting a Qt form click synchronously
inside that signal would reenter a worker-start stack before work is posted,
which is not an ordinary user-input path. The Qt test's synthetic field
replacement and pending Return additionally stress retention of the latest
requested value; they do not measure physical typing responsiveness.
For a genuine slow Qt commit, the test first cancels the edited value's
preview and then accepts the unchanged value: a cancelled preview is not
cached, so OK computes the command itself. Accepting a successful small
Box preview instead adopts its cached result and does not demonstrate
busy commit cancellation.
Each physical run retains `windowed-ui-light.png` and
`windowed-ui-dark.png` as framebuffer/widget composites, and
`windowed-screen-light.png` and `windowed-screen-dark.png` from the
existing own-window capture API. The latter captures only this process's
on-screen windows and includes the platform glass; use it for inspecting
the floating cards, ribbon and Retina appearance rather than inferring
contrast from a widget grab without its native backdrop.

Use an unlocked console session. Initially the frontmost process was
`com.apple.loginwindow`, AppKit was inactive and its key window was nil.
Native whole-key tests, sketch Tab entry and fullscreen could not establish
their targets. After the owner unlocked the console, the same activation,
focus and fullscreen assertions passed without changing product behavior
or weakening guards. The suspected findings #573 and #574 were closed as
environment preconditions, with locked/unlocked diagnostics retained.
Hardware framebuffer/picking evidence alone does not establish active
AppKit keyboard routing or panel presentation.

The final warmed Alt+pixel-scroll observation ran with no concurrent build
or test load, after unlocking the console. Each frontend retained 180
measured callback intervals after 30 warm-up input events:

| Frontend | Median interval (ms) | Mean interval (ms) | Observed swap callbacks/s |
| --- | --- | --- | --- |
| Qt | 8.11 | 8.27 | 120.97 |
| AppKit | 8.11 | 8.26 | 121.06 |

These callbacks are consistent with the screen's reported 120 Hz pacing;
they are not scanout/presentation timestamps or physical trackpad latency.
An earlier locked-console sample was excluded from the physical pacing
result. The initial synthetic right-button sampler released at a different
point from its last move, opening a context menu; the corrected sample uses
the already-validated pixel-scroll orbit path and waits for its end timer.
The failed harness logs are retained; no product navigation or graphics
policy was changed to obtain the final result.

The physical tests are registered serial CTests and return 77 when no
physical Metal device is available. `MITCAD_UI_REQUIRE_HARDWARE=1` makes
that absence fail; `MITCAD_UI_REQUIRE_GUI=1` similarly requires a live
window server and GL context. Software or paravirtual viewport renderers
are rejected after startup. Settings and model fixtures are private to
each run, and the native test restores its own saved-frame preference.
No global display, appearance or accessibility settings are changed.
The no-device guard was also exercised with a synthetic empty worker
enumeration: both frontend invocations returned 77 with hardware optional
and 1 with hardware required, before launching any GUI. The real physical
runs required GUI, hardware and Retina and had no skips.

Run only the selected checks in the appropriate configured build:

```bash
MITCAD_UI_REQUIRE_GUI=1 MITCAD_UI_REQUIRE_HARDWARE=1 \
MITCAD_UI_REQUIRE_RETINA=1 \
ctest --test-dir <qt-build> -j1 --output-on-failure -R '^app\.ui-macos-physical$'
MITCAD_UI_REQUIRE_GUI=1 MITCAD_UI_REQUIRE_HARDWARE=1 \
MITCAD_UI_REQUIRE_RETINA=1 \
ctest --test-dir <native-build> -j1 --output-on-failure -R '^app\.ui-macos-native-physical$'
```

For the checklist's explicitly authorized frame-time observation, append
`MITCAD_PHYSICAL_MEASURE_FRAMES=1` to one physical test invocation per
frontend. It performs one warmed synthetic orbit with a 1 ms driver tick
(two ticks per Alt+pixel-scroll event), logs `MITCAD_LOG_TIMING=1` frame
swaps and retains raw samples plus `orbit-summary.tsv`. This avoids a
20 ms input pacing cap;
reported display refresh, actual observed swaps, hardware presentation and
physical trackpad latency remain different quantities. No 120 Hz success
is inferred from the panel specification or from input cadence alone.
Log failures and link their follow-up tickets alongside the result cells.
