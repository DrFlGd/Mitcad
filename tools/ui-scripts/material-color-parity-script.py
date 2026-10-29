#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Open and cancel the real Materials colour field in both frontends."""
import sys

native = sys.argv[1] == "native"
lines = ["# SPDX-License-Identifier: MIT", "wait-idle", "window-resize 1280 820",
         "expect-model-bodies 1 1000", "capture-undo-depth baseline"]
if native:
    lines += ["expect-ever 'Native UI: AppKit shell with embedded OCCT viewport'",
              "native-appearance '${MITCAD_TEST_COLOR_SCHEME}'"]
lines += ["command solid.materials", "focus-dialog '^Materials$'"]

for identifier, name, color, words, state, accessible in [
        ("explicit-color", "Audit Explicit", "#2a70b8", "-", "explicit", "Filament colour: #2a70b8"),
        ("inherited-color", "Audit Inherited", "#cc8844", "Swatch source", "placeholder",
         "Filament colour: #cc8844, inherited from Swatch source"),
        ("unset-color", "Audit Slicer Default", "none", "Slicer default", "placeholder", "Filament colour: Slicer default")]:
    lines += ["mark"]
    if native:
        lines += [f"native-pointer-click material.design/{identifier}"]
    else:
        # Incremental search selects the actual visible tree row; then use
        # its logged pointer target and the real field, not a setter.
        lines += ["focus-widget materialsList", "key Home", f"type '{name}'"]
    lines += [f"expect-log '^Material selected: {name} \\(design\\)'", "wait-idle"]
    if not native:
        lines += [f"dialog-click-logged 'Materials item {name} at '"]
    lines += ["native-type settingsSearch 'Filament colour'" if native else "focus-widget settingsSearch"]
    if not native:
        lines += ["key Ctrl+a", "type 'Filament colour'"]
    probe = f"expect-native-color-field printSetting_filament_colour '{color}' '{words}' {state} '{accessible}'"
    # Preserve the actual visible field even when its pixel assertion fails.
    lines += [f"screenshot-dialog '${{MITCAD_TEST_WORK}}/{identifier}-${{MITCAD_TEST_COLOR_SCHEME}}.png'"]
    if native:
        lines += [probe, "expect-native-bounds printSetting_filament_colour 180"]
    else:
        # The Qt paint remains custom; its image is reviewed alongside the
        # native pixels. Verify its field state also survives cancellation.
        lines += [f"expect-widget-property printSetting_filament_colour nativeColorFieldPlaceholder {'true' if state == 'placeholder' else 'false'}"]
        lines += [f"expect-widget-property printSetting_filament_colour accessibleName '{accessible}'"]
    lines += ["native-pointer-click printSetting_filament_colour" if native else
              "dialog-click-logged 'Materials settings field filament_colour at '",
              "focus-dialog '^Filament colour$'",
              f"screenshot-screen '${{MITCAD_TEST_WORK}}/{identifier}-picker-${{MITCAD_TEST_COLOR_SCHEME}}.png'"]
    lines += ["native-pointer-click native.color.cancel" if native else "button '^Cancel$'",
              "focus-dialog '^Materials$'", "expect-undo-depth ${baseline} 0", "expect-model-bodies 1 1000"]
    if native:
        lines += [probe]
    else:
        lines += [f"expect-widget-property printSetting_filament_colour nativeColorFieldPlaceholder {'true' if state == 'placeholder' else 'false'}"]
        lines += [f"expect-widget-property printSetting_filament_colour accessibleName '{accessible}'"]
    lines += [f"screenshot-dialog '${{MITCAD_TEST_WORK}}/{identifier}-cancelled-${{MITCAD_TEST_COLOR_SCHEME}}.png'"]

lines += ["native-pointer-button '^Close$'" if native else "button '^Close$'", "focus-main",
          "expect-undo-depth ${baseline} 0", "expect-model-bodies 1 1000",
          "expect-never 'Native Mac canvas[:]|OCCT view event failed|Recompute failed'"]
print("\n".join(lines))
