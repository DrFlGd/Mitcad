// SPDX-License-Identifier: MIT
#pragma once

// Secondary AppKit windows within the visible screen (mitcad#818): a
// scrolling body above the dialog's buttons, sized to the content and kept
// within the visible frame of the window's screen, title bar included.
// Required Auto Layout sizes of the body's controls cannot make the window
// larger than that frame: they enlarge the scrolled document instead.
// Included by Objective-C++ sources only.

#import <AppKit/AppKit.h>

namespace mitcad {

// The content view of a secondary window: `body` in a scroll view, which
// scrolls it vertically (and horizontally beyond the width its controls
// require), above `footer` (the dialog's buttons), which stays visible at
// the bottom. The scroll view's accessibility identifier is
// native.secondary.body.
NSView* nativeScrollingContent(NSView* body, NSView* footer);

// Sizes a window whose content view is nativeScrollingContent's: its
// current (requested) width, or the width its controls require; the height
// its body and buttons need, or the requested height when that is more; all
// within nativeWindowAvailableFrame (for a sheet, that of `sheetHost`, which
// it will be presented on). A window that is no sheet is moved inside that
// frame around its current centre. Afterwards the window follows display
// changes and its moves to another screen: made smaller and moved inside
// again where needed, never larger.
void fitNativeWindowToScreen(NSWindow* window, NSWindow* sheetHost);

// The frame (in screen coordinates) a window has to fit in: its screen's
// visible frame (without the menu bar and the Dock); for an attached sheet,
// the part of its host's screen below the host's title bar and toolbar.
NSRect nativeWindowAvailableFrame(NSWindow* window);

// Tests: each screen's visible frame cut to at most `limit` at its top left
// corner, as on a smaller display; NSZeroSize for none. Windows follow it on
// the next NSApplicationDidChangeScreenParametersNotification.
void setNativeTestScreenLimit(NSSize limit);

} // namespace mitcad
