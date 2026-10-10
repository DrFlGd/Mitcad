// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/face_mesh.rs: shaded meshes of a body's
// faces for USDZ export (mitcad#123).

#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/face_mesh.h.
struct FaceMeshes;

FaceMeshes face_meshes(const geometry::Shape& shape, double deviation, double angle);

} // namespace mitcad::bridge
