// SPDX-License-Identifier: MIT
#pragma once

// Shaded meshes of a shape's faces for display formats (USDZ export,
// mitcad#123): each face keeps its own vertices, with the surface's
// normals, so that edges stay sharp and curved faces smooth.

#include <array>
#include <cstdint>
#include <vector>

#include <TopoDS_Shape.hxx>

#include "mitcad/io/mesh.hpp"

namespace mitcad::io {

struct FaceMeshes {
  std::vector<std::array<double, 3>> vertices;
  // A unit normal per vertex, pointing out of the body.
  std::vector<std::array<double, 3>> normals;
  // Counter-clockwise seen from outside, face by face.
  std::vector<std::array<std::uint32_t, 3>> triangles;
  // Each face's number of triangles, in the order TopExp::MapShapes numbers
  // the faces (that of geometry::Shape::face).
  std::vector<std::uint32_t> face_triangles;
};

// The shape triangulated with `options` on a copy (see triangulate), in the
// shape's own placement (millimetres). A face's normals are its surface's
// at each vertex, turned where the face is reversed (at a point where the
// surface has no normal, such as a cone's apex, the normal of the face's
// triangles around it); a face without a surface (a mesh body) is faceted:
// each triangle with its own three vertices and its own normal. Throws
// Error when a face has no triangulation.
FaceMeshes face_meshes(const TopoDS_Shape& shape, const MeshOptions& options);

} // namespace mitcad::io
