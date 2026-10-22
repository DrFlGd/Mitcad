// SPDX-License-Identifier: MIT
#pragma once

// Faces of a body moved by a rigid motion (the .f3d import's moved faces,
// mitcad#343).

#include <string>
#include <vector>

#include <gp_Trsf.hxx>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// Moves the faces by the rigid motion `motion`: each keeps its surface,
// moved. The neighbouring faces stay on their surfaces and extend or
// shorten up to the moved ones: their common edges become the surfaces'
// intersections next to where the edges were, the vertices between them
// follow. Every face keeps its names; the body's topology stays as it is,
// and a motion that would have to change it (a face that would vanish or
// split, an edge whose faces no longer meet near it) is an error. The
// motion must be rigid (no scaling) and must move something.
ShapePtr move_faces_rigidly(const std::string& feature, const Shape& body,
                            const std::vector<std::string>& faces, const gp_Trsf& motion);

} // namespace mitcad::geometry
