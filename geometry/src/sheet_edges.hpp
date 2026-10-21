// SPDX-License-Identifier: MIT
#pragma once

// Edges of surface bodies as the surface features take them (mitcad#455;
// defined in surface_offsets.cpp).

#include <string>
#include <vector>

#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Dir.hxx>

#include "mitcad/geometry/shape.hpp"
#include "mitcad/geometry/surface_offsets.hpp"

namespace mitcad::geometry::detail {

// The named edges and the free edges of the openings, each once, as edge
// indices of the body; throws when there are none.
std::vector<int> sheet_edges(const Shape& body, const std::vector<std::string>& names,
                             const std::vector<OpeningEdges>& openings);

// The direction that continues the face across its edge at the edge's
// curve parameter t: in the face's tangent plane, square to the edge,
// away from the face.
gp_Dir across_edge(const TopoDS_Edge& edge, const TopoDS_Face& face, double t);

} // namespace mitcad::geometry::detail
