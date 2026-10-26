// SPDX-License-Identifier: MIT
#pragma once

// mitcad-cli mesh-compare (mitcad#644): a design's bodies meshed twice as
// the 3D view's final display tier meshes them, each time with one variant
// of the planes' triangulation (the default insertion, OCCT's Delabella as
// the interactive preview tier, or Delabella checked as the final tier
// now), compared face by face; and the totals over many such reports.

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
