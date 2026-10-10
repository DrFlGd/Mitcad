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
  errors and previews use the existing command definitions.
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

## Validation

`app.ui-macos-native` starts and cancels Offset Plane through its familiar
pinned CAD ribbon button, checks Extrude stays disabled without profiles,
then creates a 40 × 25 × 20 mm box through the native menu and AppKit controls,
rejects an invalid expression, edits its height to 10 mm through native
history, verifies undo/redo, cancels a preview and exercises command search.
Repeated native inputs retain their values after adding and removing rows,
and their controls unlock after each structural action is applied.
Native outline selection drives worker measurements, Return/Escape close
the inspector, and Physical Properties presents a native report dialog.
Model-volume assertions check the persisted result. The native test bridge
uses AppKit target/action and the field editor without external
Accessibility permission. The retained Qt workflow tests still run with
`--ui=qt`.

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

The VM can validate software OpenGL and native control behavior. Before
release, verify these on supported physical Macs and the oldest supported
macOS installation:

- Canvas rendering and picking after resize, repeated minimize/restore,
  fullscreen transitions, window movement and sleep/wake.
- Floating model, inspector and history card visibility and pointer input;
  CAD ribbon groups, menus and icon legibility at compact window sizes.
- Retina/non-Retina display moves, backing-scale changes and multiple
  display arrangements; input coordinates and orientation cube scaling.
- Trackpad pan, zoom, orbit and selection; focus traversal between canvas,
  fields, sidebar, history and sheets; Return, Escape and saved shortcuts.
- Light/dark appearance, increased contrast, reduced transparency and
  reduced motion; VoiceOver labels, selection and navigation order.
- File panel acceptance, filename extensions, overwrite prompts and native
  colour acceptance/cancellation; Finder open requests; save/discard/cancel
  on window close and Quit; recovery, imports, exports, version history
  and remote work.
- Long commands: cancel while previewing, cancel during commit, changing
  inputs while previewing and closing during edit-marker restoration.

Do not infer hardware GL or macOS 14.4 runtime coverage from an SDK 27 build
with deployment target 14.4. Retain the Qt frontend while completing those
checks.
