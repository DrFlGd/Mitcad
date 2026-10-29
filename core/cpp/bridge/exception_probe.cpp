// SPDX-License-Identifier: MIT
#include "bridge/exception_probe.hpp"

#include <stdexcept>
#include <string>

#include <Standard_ConstructionError.hxx>
#include <Standard_Failure.hxx>

namespace mitcad::bridge {

namespace {

// Thrown by the "other" kind: not derived from std::exception, so only the
// bridge's catch-all (bridge/trycatch.hpp) turns it into an error.
struct NotAnException {
  int code;
};

} // namespace

void throw_for_bridge_test(rust::Str kind, rust::Str message) {
  const std::string which(kind);
  const std::string text(message);
  if (which == "occt") {
    throw Standard_Failure(text.c_str());
  }
  if (which == "occt-empty") {
    throw Standard_ConstructionError();
  }
  if (which == "std") {
    throw std::runtime_error(text);
  }
  if (which == "other") {
    throw NotAnException{937};
  }
}

} // namespace mitcad::bridge
