// SPDX-License-Identifier: MIT
#pragma once

#include <string>

namespace mitcad::cli {

// Discover running GUI instances or bridge MCP stdio to one of them.
int mcpListInstances();
int mcpConnect(const std::string& instance);

} // namespace mitcad::cli
