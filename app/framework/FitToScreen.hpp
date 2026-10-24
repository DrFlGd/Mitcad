// SPDX-License-Identifier: MIT
#pragma once

#include <QScrollArea>
#include <QSize>

class QEvent;
class QObject;
class QWidget;

namespace mitcad {

// A QScrollArea whose size hint is as tall (and wide) as its contents need.
// QScrollArea's own hint stops at 36 x 24 text lines, so a dialog that holds
// its form in a scroll area opens at that size however much the form needs
// and scrolls although the screen has room. With this one the dialog's
// hint is the whole form; the scroll bar remains for a screen too low for
// it (see fitWindowToContents). Needs widgetResizable for the contents'
// size hint to count; the minimum size set on it is respected.
class FittingScrollArea : public QScrollArea {
public:
  explicit FittingScrollArea(QWidget* parent = nullptr) : QScrollArea(parent) {}
  QSize sizeHint() const override;

protected:
  // A layout change in the contents (a field shown or hidden) makes the
  // area's hint, which the dialog's layout caches, stale.
  bool eventFilter(QObject* watched, QEvent* event) override;
};

// The size of a window whose contents ask for `hint`, on a screen whose
// available area is `available` (the screen without the task bar and the
// like), for a window frame (title bar and borders) of `frame` around the
// window's own area: the hint, at most what is left of the screen.
QSize fittedWindowSize(const QSize& hint, const QSize& available, const QSize& frame);

enum class FitMode {
  Fit,      // the window gets exactly its fitted size (also smaller)
  GrowOnly  // the window only grows to it: a change while the user is typing
};

// Sizes a top-level window (a dialog) to its size hint, capped by the
// screen's available size minus the window frame (QWidget::adjustSize caps
// at two thirds of the screen instead), and moves it up or left when it
// would stick out of the screen. Before the window is shown its frame is
// not known: room is kept for a typical one, and the next call, from
// showEvent, takes the real one.
void fitWindowToContents(QWidget* window, FitMode mode = FitMode::Fit);

} // namespace mitcad
