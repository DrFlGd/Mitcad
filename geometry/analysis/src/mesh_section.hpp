// SPDX-License-Identifier: MIT
#pragma once

// Sections and clips of mesh bodies (faces with triangles and no surface:
// STL and OBJ files, the meshes of imported building models). OCCT's
// booleans need surfaces: given a face without one, the pave filler reads
// a null handle while it intersects the faces and crashes (mitcad#392), so
// these work on the triangles instead.

#include <vector>

#include <TopoDS_Shape.hxx>

#include "mitcad/analysis/section.hpp"

namespace mitcad::analysis::detail {

// True when every face of the shape is a mesh face (no surface); throws
// Error when the shape mixes mesh faces with faces on surfaces.
bool mesh_only(const TopoDS_Shape& shape);

// The section of a mesh shape: one straight edge for each piece of a
// triangle the plane cuts (edges of triangles lying in the plane count
// once, triangles lying in it add nothing), and, when the triangles close
// up (closed_mesh), the planar faces they bound in the plane, with holes.
Section mesh_section(const TopoDS_Shape& shape, const Plane& plane);

// A mesh shape without the half-space on the plane's normal side: the
// triangles behind the plane, those it crosses cut along it. Triangles in
// the plane stay. A closed mesh gets the section faces as caps, facing
// along the normal. `faces` as clip's: each face's new mesh face (the face
// itself when nothing of it is removed, none when all of it is).
TopoDS_Shape mesh_clip(const TopoDS_Shape& shape, const Plane& plane,
                       std::vector<ClippedFace>* faces);

} // namespace mitcad::analysis::detail
