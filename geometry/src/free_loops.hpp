// SPDX-License-Identifier: MIT
#pragma once

// The open boundaries of a body as edge indices (surfaces.cpp), for the
// surface modelling features that take free edges (mitcad#455). Internal
// to the geometry library.

#include <vector>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry::detail {

// The free edges of a body (edges of one face, not a seam of it) grouped
// into connected loops, in the order of open_boundaries (surfaces.hpp).
std::vector<std::vector<int>> free_edge_loops(const Shape& body);

} // namespace mitcad::geometry::detail
