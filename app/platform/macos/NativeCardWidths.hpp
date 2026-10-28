// SPDX-License-Identifier: MIT
#pragma once

// The widths of the AppKit frontend's side cards (mitcad#818): Projects and
// Browser on the left, the inspector on the right. The user drags a card's
// inner edge; the width is remembered with the shared settings (Projects:
// projects/cardWidth, as the Floating Qt card) and placed within limits
// like FloatingLayout::setCardWidth's. Included by Objective-C++ sources
// only.

#import <AppKit/AppKit.h>

#include <functional>

#include <QString>

namespace mitcad::mac {

enum class NativeCard { Projects, Browser, Inspector };

// "projects", "browser" and "inspector": the names of the cards in
// NativeMacWindowCallbacks::cardWidthChosen and NativeMacWindow::setCardWidth.
QString nativeCardName(NativeCard card);
bool nativeCardNamed(const QString& name, NativeCard& card);

// The widths the cards are placed at in a canvas.
struct NativeCardPlacement {
  CGFloat projects = 0;
  CGFloat browser = 0;
  CGFloat inspector = 0;
};

// The cards' widths as the user dragged them, and where they are placed:
// each at least its minimum and at most two thirds of the canvas less its
// margins; with cards shown on both sides, a free canvas of at least
// freeCanvas between them, for which the inspector narrows first and then
// the left cards, each down to its minimum. A narrow window narrows the
// cards without changing the remembered widths.
class NativeCardWidths {
public:
  static constexpr CGFloat margin = 12;      // the cards' distance from the canvas edges
  static constexpr CGFloat freeCanvas = 160; // the least canvas between the side cards
  // Projects' native toolbar needs its row of fixed buttons; the inspector
  // keeps the width its forms are laid out for.
  static CGFloat minimum(NativeCard card);
  static CGFloat standard(NativeCard card);

  CGFloat width(NativeCard card) const;
  // A remembered width (from the settings): the card asks for it again.
  void restore(NativeCard card, CGFloat width);
  // A drag of a card's edge to `width`: remembered as placed now, within
  // the limits and the other side's cards; returns that width.
  CGFloat drag(NativeCard card, CGFloat width, CGFloat canvasWidth, bool projectsShown, bool browserShown,
               bool inspectorShown);
  NativeCardPlacement place(CGFloat canvasWidth, bool projectsShown, bool browserShown, bool inspectorShown) const;

private:
  CGFloat m_widths[3] = {260, 260, 300};
};

// Adds a handle along the inner edge of `card` (a frame-placed, flipped
// floating card): its right edge, its left one for the inspector. Pointer
// drags on it show the resize cursor and report the width they ask for
// while dragging, and once more with `finished` when released. The
// handle's accessibility identifier is the card's with ".width" appended.
NSView* addCardWidthHandle(NSView* card, NativeCard which, std::function<void(CGFloat width, bool finished)> dragged);
// Drops the handle's report, before its receiver goes away.
void detachCardWidthHandle(NSView* handle);

// Lays out a shown card and lets its outline's first (name) column take the
// width its viewport leaves after the other columns, as the shared panels'
// name columns stretch. AppKit autoresizes columns only when a viewport
// changes size, so an outline made or remade inside a card that is already
// wide (a remembered width) would otherwise keep its first width.
void fitCardOutline(NSView* card, NSOutlineView* outline);

} // namespace mitcad::mac
