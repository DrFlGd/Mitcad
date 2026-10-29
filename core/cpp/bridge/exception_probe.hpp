// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/exception_probe.rs: a test of how C++
// exceptions cross the bridges (mitcad#937, bridge/trycatch.hpp).

#include "rust/cxx.h"

namespace mitcad::bridge {

// For tests (core.bridge): throws, outside any geometry operation
// (geometry::run_operation), the exception `kind` names with `message`:
// "occt" a Standard_Failure, "occt-empty" an OCCT exception without a
// message (Standard_ConstructionError), "std" a std::runtime_error, "other"
// an object that is no std::exception; any other kind returns normally.
void throw_for_bridge_test(rust::Str kind, rust::Str message);

} // namespace mitcad::bridge
