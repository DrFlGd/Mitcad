// SPDX-License-Identifier: MIT
#pragma once

// Boundary Fill (mitcad#457): the cells that tool bodies enclose. The
// tools' faces divide space into cells, each inside some of the tools and
// outside the others; a point inside a cell picks it.

#include <cstddef>
#include <vector>

#include <gp_Pnt.hxx>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// The cell a point picks: its solids, and the tools that hold the point
// (indices into the tools, in increasing order).
struct BoundaryCell {
  ShapePtr shape;
  std::vector<std::size_t> holders;
};

// For each point, the cell it picks among the tools: all of the space
// inside every tool that holds the point and outside every other tool, as
// one shape of one or more solids. The tools' faces divide space once
// (OCCT's general fuse, BOPAlgo_CellsBuilder), so cells that meet share
// their faces; the pieces of one face that a cell has side by side are
// merged again. Each face keeps the names of the tool faces it is a piece
// of (several where faces of tools lie on each other), pieces of one face
// numbered "#k" (naming.hpp). Two points in one cell give the same shape.
// Throws std::runtime_error naming the point when no tool holds it, when it
// lies on a tool's boundary, or when its cell comes out empty, and when a
// tool has no solid. The tools are not changed.
std::vector<BoundaryCell> boundary_cells(const std::vector<const Shape*>& tools,
                                         const std::vector<gp_Pnt>& points);

} // namespace mitcad::geometry
