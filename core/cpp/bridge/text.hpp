// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/text.rs.

#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/text.h.
struct TextOutlines;
struct FontFileInfo;

TextOutlines text_outlines(rust::Str family, bool bold, bool italic, rust::Str text);
FontFileInfo font_file(rust::Str family, bool bold, bool italic);

} // namespace mitcad::bridge
