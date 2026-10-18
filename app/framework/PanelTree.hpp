// SPDX-License-Identifier: MIT
#pragma once

namespace mitcad {
namespace panel {

// The indentation per level of the side panels' trees (the Browser and the
// Projects panel), which is also the width the style centres a branch's
// disclosure arrow in. The styles' own (20 to 30 pixels: Qt's cross-platform
// style, Windows 11) waste a narrow panel's width and leave the arrows far
// from the names. One definition for both trees, so that their rows keep the
// same offset per level (mitcad#393).
constexpr int kTreeIndentation = 16;

} // namespace panel
} // namespace mitcad
