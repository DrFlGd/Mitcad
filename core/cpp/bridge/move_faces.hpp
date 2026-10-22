// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/move_faces.rs. The motion is the
// transform bridge's Affine.

#include <memory>

#include "mitcad/geometry/shape.hpp"
#include "mitcad_bridge/kernel/transform.h"
#include "rust/cxx.h"

namespace mitcad::bridge {

// geometry::move_faces_rigidly (mitcad#343).
std::shared_ptr<geometry::Shape> move_faces(rust::Str feature, const geometry::Shape& body,
                                            rust::Slice<const rust::String> faces, const Affine& motion);

} // namespace mitcad::bridge
