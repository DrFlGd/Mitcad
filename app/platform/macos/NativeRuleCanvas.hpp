// SPDX-License-Identifier: MIT
#pragma once

#include <memory>

#include "../NativeDialogs.hpp"

@class NSEvent;
@class NSWindow;

class QWidget;

namespace mitcad::mac {
// Dedicated AppKit input and drawing for the shared rule block scene and
// palette. The QGraphicsScene is artwork; no QWidget is embedded or drawn.
std::unique_ptr<NativeWidgetView> createNativeRuleView(QWidget* presenter);
// Focused rule input gets its editing/navigation keys before the dialog's
// default and Escape buttons. Other presenters and unhandled keys fall
// through to the ordinary window handling.
bool routeNativeRuleKey(NSWindow* window, NSEvent* event);
}
