// SPDX-License-Identifier: MIT
#pragma once

// A shape's display triangulations as bytes (mitcad#858): what the view's
// meshing (mesh_in_place, mesh_all_in_place) left in a shape's faces and
// edges, kept in the result store (core/model/src/store.rs) under the
// body's version, so that opening the design again gives every face its
// triangulation back instead of meshing it.

#include <cstddef>
#include <memory>
#include <string>
#include <string_view>

#include <TopoDS_Shape.hxx>

namespace mitcad::io {

// The triangulations of the shape's faces, in TopExp order: each face's
// nodes, parameters, triangles, deviation and meshing parameters (not the
// normals a presentation adds), its edges' polygons on it, and what the
// face is checked by when they are given back (its surface's kind, its
// edges and the sum of their end points). A face without a triangulation
// (one OCCT's mesher failed on) is kept as one without; a face met again
// in another placement refers to the first. Throws std::invalid_argument
// for a null shape or a mesh body (a face without a surface).
std::string triangulation_bytes(const TopoDS_Shape& shape);

// A shape's display triangulations taken for triangulation_bytes later, on
// another thread (mitcad#880): each face's triangulation, its edges'
// polygons on it and its signature, as triangulation_bytes would write
// them, held by OCCT's handles. Taking them only walks the shape's faces
// and edges; turning them into bytes, the costly part, reads only the
// triangulations and polygons held, which nothing changes once made (a
// face meshed again gets a new triangulation, and the view adds normals,
// which are not written). So it runs while the shape is drawn, or meshed
// again with a shape that shares its faces, or after the shape is gone.
// Throws as triangulation_bytes.
class TriangulationCapture;
std::shared_ptr<const TriangulationCapture> capture_triangulations(const TopoDS_Shape& shape);
// The bytes of triangulation_bytes for the shape the capture was taken of.
std::string triangulation_bytes(const TriangulationCapture& capture);

// Gives the shape's faces the triangulations of triangulation_bytes, with
// their edges' polygons; a face that has a triangulation already is left
// as it is. A face met again in another placement takes the earlier one's
// through their shared TShape, or a copy of it when the shape has a TShape
// of its own there (a copy of the shape need not share it). Returns how
// many faces (TopExp's, each placement apart) were given one (0 when all
// had one).
// Every face is checked before anything is written: bytes of another
// shape (other faces, a face on another kind of surface, with other edges
// or end points 1e-6 mm apart, indices out of range, data cut short) write
// nothing and throw std::runtime_error. The shape must be the caller's
// alone (nothing draws it meanwhile), as for mesh_in_place.
std::size_t restore_triangulations(const TopoDS_Shape& shape, std::string_view data);

// The same from a capture of another shape's triangulations instead of
// their bytes (mitcad#881): a copy of `shape` (geometry::independent_copy,
// the same faces, edges and placements in new sub-shapes, on the same
// surfaces and curves) meshed on another thread gives the shape its
// triangulations and edge polygons, the same handles, with the same checks
// (another shape's capture writes nothing and throws std::runtime_error).
// The shape must be the caller's alone; the capture is only read.
std::size_t restore_triangulations(const TopoDS_Shape& shape, const TriangulationCapture& capture);

} // namespace mitcad::io
