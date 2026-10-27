// SPDX-License-Identifier: MIT
#pragma once

// mitcad-cli export-drawing (mitcad#471): a design's bodies as a drawing
// with views, dimensions and a hole table, written as PDF, SVG, DXF or DWG by
// the model's export_drawing command.

#include <string>
#include <vector>

namespace mitcad::cli {

// `mitcad-cli export-drawing <file.mitcad> <out> [options]` (args[0] is the
// mode); the exit status.
int export_drawing(const std::vector<std::string>& args);

} // namespace mitcad::cli
