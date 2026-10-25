// SPDX-License-Identifier: MIT
#pragma once

// mitcad-cli mesh-compare (mitcad#644): a design's bodies meshed as the 3D
// view's final display tier meshes them, and the same with planes through
// OCCT's Delabella triangulation (the interactive preview tier's planes),
// compared face by face; and the totals over many such reports.

#include <functional>
#include <string>
#include <vector>

#include "mitcad_bridge/lib.h"
#include "rust/cxx.h"

namespace mitcad::cli {

extern const char* const kMeshCompareUsage;

// `mitcad-cli mesh-compare ...` (args[0] is the mode); `open` opens a design
// or runs a script (.json) as the other commands do. The exit status.
int mesh_compare(const std::vector<std::string>& args,
                 const std::function<rust::Box<mitcad::Document>(const std::string&)>& open);

} // namespace mitcad::cli
