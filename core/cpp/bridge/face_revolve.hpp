// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/face_revolve.rs (mitcad#539).

#include <memory>

#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/face_revolve.h.
struct FaceRevolution;

// geometry::revolve_face.
std::shared_ptr<geometry::Shape> revolve_face(rust::Str feature, const geometry::Shape& body, rust::Str face,
                                              rust::Str key, const FaceRevolution& revolution);

} // namespace mitcad::bridge
