// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/tangent_chain.rs.

#include "bridge/shape.hpp"
#include "mitcad/geometry/shape.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// Shared with Rust; defined in the generated mitcad_bridge/kernel/tangent_chain.h.
struct TangentChain;

// The chains of two or more named edges or faces of the shape (a face by
// its first name; unnamed members left out).
rust::Vec<TangentChain> tangent_chains(const geometry::Shape& shape, bool faces);

} // namespace mitcad::bridge
