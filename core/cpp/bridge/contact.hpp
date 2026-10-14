// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/contact.rs.

#include <cstdint>

#include "bridge/shape.hpp"
#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// geometry::tool_contacts, one per body: 0 apart, 1 touching, 2 inside.
rust::Vec<std::uint8_t> tool_contacts(const geometry::Shape& tool, const ShapeList& bodies);

} // namespace mitcad::bridge
