// SPDX-License-Identifier: MIT
#pragma once

// How a C++ exception under a bridge call becomes the Rust side's Err
// (mitcad#937). Every #[cxx::bridge] with an `extern "C++"` block includes
// this header (`include!("bridge/trycatch.hpp")`), so the shims cxx
// generates for its `Result` functions use this `trycatch` instead of cxx's
// own, which catches only `std::exception`: an exception that is not one
// would reach the shim's `noexcept` and end the process.
//
// OCCT's exceptions (Standard_Failure) derive from std::exception in the
// OCCT this build uses (8.0.1), but they are caught by their own type
// first, so the boundary does not depend on that: an OCCT exception
// thrown outside `geometry::run_operation` (a helper of a bridge function
// that calls OCCT directly) is an Err with OCCT's message, or with the
// exception's type when it has none. Any other exception is an Err too.
//
// cxx looks this function up by its name and signature: it must be a
// template `rust::behavior::trycatch(Try&&, Fail&&)` declared before the
// generated code, which follows the bridge's includes.

#include <exception>
#include <string>

#include <Standard_Failure.hxx>

namespace rust {
namespace behavior {

template <typename Try, typename Fail>
static void trycatch(Try&& func, Fail&& fail) noexcept try {
  func();
} catch (const Standard_Failure& failure) {
  const char* text = failure.what();
  if (text != nullptr && *text != '\0') {
    fail(text);
  } else {
    fail(std::string("OCCT exception ") + failure.ExceptionType());
  }
} catch (const std::exception& error) {
  fail(error.what());
} catch (...) {
  fail("unknown C++ exception");
}

} // namespace behavior
} // namespace rust
