#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""No-options entry, numeric Other, New fill and an actual picked face."""
import sys

native = sys.argv[1] == "native"
frontend = "native" if native else "qt"
lines = ["# SPDX-License-Identifier: MIT", "wait-idle", "window-resize 1280 820",
         "expect-model-bodies 1 48000", "key F6"]


def add(*steps):
    lines.extend(steps)


def press(label):
    add(f"native-pointer-button '^{label}$'" if native else f"button '^{label}$'")


def shot(state):
    add(f"screenshot-dialog '${{MITCAD_TEST_WORK}}/{state}-{frontend}-light.png'",
        f"screenshot-screen '${{MITCAD_TEST_WORK}}/{state}-{frontend}-light-own-windows.png'")


def field(identifier, value):
    add(f"native-pointer-click {identifier}" if native else f"focus-widget {identifier}",
        "key Ctrl+a", f"type '{value}'", "key Tab")


def fill(identifier):
    add(f"native-pointer-dblclick appearance.{identifier}" if native else
        f"dialog-dblclick-logged 'Appearances item {identifier} at '")


def length(value):
    add("focus-main", "mark",
        "native-pointer-click configurationField_Length" if native else "focus-widget configurationField_Length",
        "key Ctrl+a", f"type '{value} mm'", "key Return",
        f"expect-log '^Configuration chosen: Length = {value}'", "wait-idle",
        "expect-model-bodies 1 48000")


if native:
    add("expect-ever 'Native UI: AppKit shell with embedded OCCT viewport'", "native-appearance light",
        "native-context-menu native.model.body:F1.b0@ 'Appearance…'")
else:
    add("context-menu-logged '^Browser Root/Bodies/Body1 at ' 'Appearance…'")
add("focus-dialog '^Appearances$'", "expect-log 'Appearances varies with: None \\| Add Option…'",
    "capture-undo-depth beforeOption", "mark")

# Add Option is an action entry, not a retained choice: the shared combo
# resets to None while it opens Configurations. Drive its real popup.
if native:
    add("native-pointer-choice assignmentVaries None Down Return")
else:
    add("qt-choice assignmentVaries 'Add Option…'")
add("expect-log 'Appearances: Add Option…'", "focus-dialog '^Configurations$'")
if native:
    add("native-pointer-click configurationsAddOption")
else:
    add("focus-widget configurationsAddOption", "key Space")
add("focus-dialog '^Add Option$'")
field("addOptionName", "Length")
field("addOptionValue", "20")
shot("assignment-add-option")
press("OK")
add("expect-log 'Configurations added option Length'", "expect-undo-depth ${beforeOption} 1",
    "focus-dialog '^Configurations$'")
press("Close")
add("focus-dialog '^Appearances$'", "expect-log 'Appearances varies with: None \\| Length'", "mark")
add("native-pointer-choice assignmentVaries Length Down Return" if native else
    "qt-choice assignmentVaries Length")
add("expect-log 'Appearances varies with Length: 2 rows, not written yet'",
    "expect-log 'Appearances value Other: Steel - Satin'", "expect-no-log 'Appearances binding of'",
    "expect-undo-depth ${beforeOption} 1")
if native:
    add("expect-native-text assignment.value.1 '^Other$'", "native-pointer-click assignment.value.1")
else:
    add("dialog-click-logged 'Appearances value Other at '")
add("expect-log 'Appearances active row: Other'", "capture-undo-depth beforeNew")
add("native-pointer-click appearance.paint_red" if native else
    "dialog-click-logged 'Appearances item paint_red at '")
if native:
    add("native-pointer-click appearanceNew")
else:
    add("focus-widget appearanceNew", "key Space")
add("expect-log '^Created appearance .* \\(Paint - Red Copy\\) from paint_red$'",
    "expect-log '^Appearances filled Other with '",
    "expect-log '^Appearances binding of Body1: switch\\(Length;'",
    "expect-log 'Appearances active row: none'",
    # Shared New makes a design copy, then the normal fill is its own step.
    "expect-undo-depth ${beforeNew} 2")
shot("assignment-new-other")
press("Close")
length(30)

# The viewport helper probes a currently visible OCCT face and clicks it
# through the real input path. It also asserts the exact shared selection;
# the saved model is checked against that logged topological face name.
add("key Escape", "click-visible-face", "command solid.appearances", "focus-dialog '^Appearances$'",
    "expect-log 'Appearances panel: Assign to Face; varies with None'", "capture-undo-depth beforeFace", "mark")
add("native-pointer-choice assignmentVaries Length Down Return" if native else
    "qt-choice assignmentVaries Length")
add("expect-log 'Appearances varies with Length: 2 rows, not written yet'",
    "expect-undo-depth ${beforeFace} 0")
fill("paint_black")
add("expect-log \"^Appearances filled .* with 'paint_black'$\"",
    "expect-log 'Appearances assign label: Assign to row Other'", "expect-undo-depth ${beforeFace} 1")
fill("paint_white")
add("expect-log \"Appearances filled Other with 'paint_white'\"",
    "expect-log 'Appearances active row: none'", "expect-undo-depth ${beforeFace} 2")
if native:
    add("expect-native-text assignmentTitle '^Assign to Face$'",
        "expect-native-text assignment.result.0 '^Paint - Black$'",
        "expect-native-text assignment.result.1 '^Paint - White$'")
shot("assignment-face-bound")
press("Close")
length(40)
add("key Ctrl+s", "expect-log 'Saved .*bindings-open[.]mitcad'",
    "expect-model-bodies 1 48000", "expect-never 'Native Mac canvas[:]|OCCT view event failed|Recompute failed'")
print("\n".join(lines))
