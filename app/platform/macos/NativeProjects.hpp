// SPDX-License-Identifier: MIT
#pragma once

// The native Projects outline's own name editor (mitcad#816). New Design,
// New Folder and Rename name a row in place, in the AppKit outline that
// presents the shared Projects panel, rather than in its hidden Qt tree.
// The row's own text field becomes editable with the whole name selected,
// over an opaque text background. Return, Enter and Tab take the name,
// Escape keeps the old one, and leaving the field (a click elsewhere) takes
// it as the default tree's editor does. The outline keeps the keyboard
// after Return, Tab and Escape. Only the panel decides what a name does.
// Objective-C++ only (AppKit types); used by NativeMacWindow.mm.

#import <AppKit/AppKit.h>

#include <functional>

#include <QString>

namespace mitcad::mac {

// Starts the editor on the row of `outline` whose cell has `identifier`
// (NativeControlIdentifierRole), with `text`. Its end calls `finished`
// with the text and whether it was taken. False when there is no such row.
bool beginProjectsNameEditor(NSOutlineView* outline, const QString& identifier, const QString& text,
                             std::function<void(const QString&, bool)> finished);
// Ends an open editor without a result: the panel closed the naming.
void closeProjectsNameEditor();
// Return, Enter and Escape of the editor with the keyboard, before any
// button's key equivalent can take them; true when the key was the editor's.
bool projectsNameEditorKey(NSEvent* event);
// Ends an editor whose field has left its window (the outline was replaced)
// without taking its name.
void checkProjectsNameEditor();

} // namespace mitcad::mac
