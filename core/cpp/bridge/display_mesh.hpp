// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/display_mesh.rs: display triangulations
// kept in the result store (mitcad#858).

#include <cstdint>
#include <memory>
#include <string>

#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

std::unique_ptr<std::string> display_mesh_bytes(const geometry::Shape& shape);
std::uint64_t restore_display_mesh(const geometry::Shape& shape, rust::Slice<const std::uint8_t> data);

} // namespace mitcad::bridge
