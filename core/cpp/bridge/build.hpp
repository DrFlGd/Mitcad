// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/build.rs.

#include <memory>

#include "bridge/shape.hpp"
#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// geometry::rename_faces: `from[i]` becomes `to[i]`, other names
// "<prefix>(n)".
std::shared_ptr<geometry::Shape> rename_faces(const geometry::Shape& shape, rust::Str prefix,
                                              rust::Slice<const rust::String> from,
                                              rust::Slice<const rust::String> to);

} // namespace mitcad::bridge
