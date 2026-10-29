// SPDX-License-Identifier: MIT
#pragma once

#include <memory>

#include "../NativeDialogs.hpp"

@class NSView;
class QWidget;

namespace mitcad::mac {
// The navigation guide's shared artwork, in a native image view rather
// than the empty generic conversion of a custom-painted QWidget.
std::unique_ptr<NativeWidgetView> createNativeNavigationGuide(QWidget* presenter);
// Tests read the actual native image and its logical/backing dimensions.
bool navigationGuideArtwork(NSView* view, QString& detail);
}
