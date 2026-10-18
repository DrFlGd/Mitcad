// SPDX-License-Identifier: MIT
#pragma once

// Triangulation of B-rep bodies for mesh export and mesh bodies for display.

#include <array>
#include <cstddef>
#include <cstdint>
#include <vector>

#include <Message_ProgressRange.hxx>
#include <Poly_Triangulation.hxx>
#include <Standard_Handle.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>

namespace mitcad::io {

// Mesh refinement presets for export.
enum class Refinement {
  Low,    // 0.1 mm deviation, 30 degrees between normals, 5 000 triangles a face
  Medium, // 0.03 mm, 15 degrees, 20 000 triangles
  High,   // 0.01 mm, 8 degrees, 50 000 triangles
};

// Tolerances of a triangulation.
struct MeshOptions {
  // Largest distance between the mesh and the surface (surface deviation),
  // in mm; with `relative`, a fraction of each edge's size instead.
  double linear_deflection = 0.03;
  // Largest angle between the normals of neighbouring facets, in radians.
  double angular_deflection = 0.2617993877991494; // 15 degrees
  bool relative = false;
  // About how many triangles a doubly curved face may get at most (0: no
  // limit, `linear_deflection` everywhere). A face whose deviation would
  // give it more meshes with a larger deviation instead, which grows with
  // the face's size (face_deviation_floor), so that a sphere of metres
  // gets about as many triangles as one of centimetres (mitcad#398). The
  // edges keep `linear_deflection`, so neighbouring faces still share
  // their nodes. Small faces are meshed as without a limit.
  std::size_t face_triangles = 0;

  static MeshOptions from(Refinement refinement);
};

// The smallest surface deviation (mm) that keeps a face's mesh within
// about `face_triangles` triangles: proportional to the integral of the
// square root of the surface's Gaussian curvature over the face (a
// sphere's or torus's, zero for planes, cylinders, cones and extrusions,
// whose triangle count does not grow with their size), estimated over
// the face's parameter range. 0 without a limit or for a face too small
// to need one (its box's diagonal below the deviation times
// face_triangles divided by 3.2).
double face_deviation_floor(const TopoDS_Face& face, double deviation, std::size_t face_triangles);

// A triangulated copy of the shape; the shape itself is not modified (OCCT
// keeps triangulations in the faces, so meshing the original would change
// what other users of the shape see). A mesh body, or any shape with a face
// that has no surface, is returned as it is.
TopoDS_Shape triangulate(const TopoDS_Shape& shape, const MeshOptions& options);

// Triangulates the shape's own faces, as OCCT's viewer does, and keeps
// triangulations that meet the options (each face's deviation raised to its
// face_deviation_floor). For the shapes the 3D view shows: meshed where
// nothing else uses them (the model's worker thread, mitcad#398), the view
// then finds them meshed. Returns false when `range` was stopped (the faces
// meshed so far keep their triangulations); a mesh body is left as it is.
bool mesh_in_place(const TopoDS_Shape& shape, const MeshOptions& options,
                   const Message_ProgressRange& range = Message_ProgressRange());

// Whether every face of the shape has a triangulation.
bool has_triangulation(const TopoDS_Shape& shape);

// Makes OCCT's presentations (StdPrs, AIS) mesh shapes with mesh_in_place's
// algorithm: the drawer's deviation and angle, and each face's deviation
// raised to its floor for `face_triangles` (BRepMesh_DiscretFactory's
// default algorithm, for the whole process).
void use_bounded_presentation_meshes(std::size_t face_triangles);

// Triangle and node count over the faces' triangulations.
struct MeshStats {
  std::size_t faces = 0;
  std::size_t triangles = 0;
  std::size_t nodes = 0;
};
MeshStats mesh_stats(const TopoDS_Shape& shape);

// A mesh body: one face that carries the triangulation and no surface.
// OCCT's viewer displays it like any other face.
TopoDS_Shape mesh_body(const occ::handle<Poly_Triangulation>& triangulation);

// True when the shape has faces and none of them has a surface.
bool is_mesh(const TopoDS_Shape& shape);

// A shape as one triangle mesh for 3D printing (3MF, mitcad#13): the
// vertices of all faces with those that coincide (within 1e-6 mm) merged,
// so the triangles of neighbouring faces share their vertices along the
// edges, and a closed solid gives a closed mesh; triangles that collapse
// in the merge (slivers at degenerate edges) are left out. Triangles are
// counter-clockwise seen from outside (a reversed face's are turned
// round). Millimetres, in the shape's own placement.
struct IndexedMesh {
  std::vector<std::array<double, 3>> vertices;
  std::vector<std::array<std::uint32_t, 3>> triangles;
};

// The shape triangulated with `options` on a copy (see triangulate); a
// mesh body's own triangles. Throws Error when a face has no triangulation.
IndexedMesh indexed_mesh(const TopoDS_Shape& shape, const MeshOptions& options);

} // namespace mitcad::io
