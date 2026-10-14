// SPDX-License-Identifier: MIT
#pragma once

#include <vector>

#include <QString>

#include <gp_Pnt.hxx>

namespace mitcad {

class OcctViewer;

// Brings points of the design into sight (the Sketch Palette's warnings,
// mitcad#264; live MCP's ui_select, mitcad#265). When none of them is in
// the view, the camera pans to their middle, keeping its zoom unless they
// do not fit; with `fit` it also zooms to them when they are in sight.
// Returns what it did: "none", "pan" or "zoom".
QString revealInView(OcctViewer& viewer, const std::vector<gp_Pnt>& points, bool fit = false);

} // namespace mitcad
