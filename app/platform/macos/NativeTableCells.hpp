// SPDX-License-Identifier: MIT
#pragma once

// How native AppKit tables present a shared Qt item view's semantics
// (mitcad#814): its cells' model roles, single-line elision and the
// header's column sizes, resize modes and hidden sections. Objective-C++
// (ARC) only; the generic presenter bridge and other native views share it.

#import <AppKit/AppKit.h>

#include <cstddef>
#include <vector>

#include <QtCore/qnamespace.h>

class QAbstractItemView;
class QHeaderView;
class QModelIndex;
class QWidget;

namespace mitcad::mac {

// A cell's font and colours from the shared roles: NativeRefusedRole (the
// error text and fill), Qt::FontRole, Qt::ForegroundRole and
// Qt::BackgroundRole. Shared semantic colours (error, warning, the
// palette's disabled and placeholder text) become AppKit's dynamic system
// colours, so they follow the appearance. nil text: the label colour; nil
// fill: none.
struct NativeCellStyle {
  NSFont* font = nil;
  NSColor* text = nil;
  NSColor* fill = nil;
};
NativeCellStyle nativeCellStyle(const QModelIndex& index, const QAbstractItemView* view);

// One line, elided as the view's text elide mode says, never wrapped into a
// fixed-height row.
void elideNativeCell(NSTextField* field, Qt::TextElideMode mode);

// A label's tone for its kSemanticTextToneProperty (Theme.hpp): 0 none,
// 1 error, 2 warning; and its text colour.
int nativeTextToneKind(const QWidget* widget);
NSColor* nativeTextTone(int kind);

// The horizontal header of a tree or table view; nullptr for a list.
QHeaderView* nativeHeader(const QAbstractItemView* view);

// What applyNativeColumns took from the shared header last: a signature of
// its state and the section sizes. AppKit's own values are not compared,
// so an unchanged refresh writes nothing.
struct NativeColumnState {
  std::size_t signature = 0;
  std::vector<int> sizes;
};

// Applies the header to the table's columns when its state changed:
// titles and header tooltips, hidden sections, the shared section sizes,
// and which columns stretch (QHeaderView::Stretch and a stretched last
// section). A section's width is set when its shared size changes, so a
// native user resize stays until then. True when it applied anything.
bool applyNativeColumns(NSTableView* table, const QAbstractItemView* view, NativeColumnState& state);

// Gives the stretching columns the width the others leave in the visible
// area, down to their minimum width; beyond that the table scrolls.
void fitNativeColumns(NSTableView* table);
// Refits when the visible area or a column's width changes.
void followNativeColumns(NSTableView* table);

// The table a view is in, and the text field of a table cell that has the
// field editor now (nil when none).
NSTableView* owningTable(NSView* view);
NSTextField* editedCellField(NSTableView* table);
// Selects a field's whole text. A field that took the focus already has its
// field editor: selectText: would end that edit and begin another, which
// commits the cell (mitcad#814).
void selectEditedText(NSTextField* field);

// MITCAD_LOG_NATIVE_EDITS=1 logs how native cell edits and table focus
// change, with the calling frames ("Native cell edit: ..."), so that a test
// shows what ended an edit; nothing otherwise.
void logNativeCellEdit(const char* what, NSView* view);

} // namespace mitcad::mac
