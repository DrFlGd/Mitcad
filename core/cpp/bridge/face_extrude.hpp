// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/face_extrude.rs (mitcad#535).

#include <memory>

#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/face_extrude.h.
struct FaceExtrusion;

// geometry::extrude_face.
std::shared_ptr<geometry::Shape> extrude_face(rust::Str feature, const geometry::Shape& body, rust::Str face,
                                              rust::Str key, const FaceExtrusion& extrusion);

} // namespace mitcad::bridge
