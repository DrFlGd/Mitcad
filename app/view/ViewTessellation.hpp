// SPDX-License-Identifier: MIT
#pragma once

#include <cstddef>

#include <Prs3d_Drawer.hxx>

#include "mitcad/geometry/shape.hpp"
#include "mitcad/io/mesh.hpp"

namespace mitcad {

// The view's meshes: the chordal error in model units (millimetres), the
// angle between neighbouring facets, and about how many triangles a doubly
// curved face gets at most (mitcad#398). OCCT's relative default grows with
// the whole body's bounds; its derived minimum mesh size can then override
// the angular limit on small curves of a long body. So the chordal error is
// absolute, with an angular limit for circles smaller than it; and a face
// whose 0.01 mm would take more triangles than the bound (a sphere of
// metres: millions, which froze the window) meshes with a deviation that
// grows with its size instead (io::face_deviation_floor). Faces smaller
// than about 6 cm (a sphere up to about 20 mm radius), and planes, mesh at
// 0.01 mm. Cylinders and cones mesh at most as finely as a turn in 500
// straight pieces (mitcad#438): those of radii over about half a metre,
// and their edges, with a deviation of 2e-5 of the radius (about 700
// points round a circle, where 6.8 m took 2 600 at 0.01 mm), smaller ones
// at 0.01 mm.
constexpr double kViewDeviation = 0.01;
constexpr double kViewAngle = 5.0 * 3.14159265358979323846 / 180.0;
constexpr std::size_t kViewFaceTriangles = 20000;

inline io::MeshOptions viewMeshOptions() {
  return io::MeshOptions{kViewDeviation, kViewAngle, false, kViewFaceTriangles, false};
}

// The interactive preview tier's meshes (mitcad#635, framework/
// DisplayMeshes): the view's deviation, angle and face bound, with planes
// through Delabella's base triangulation (io::MeshOptions::fast_planes).
// Chosen from test_preview_meshing --compare on the synthetic 83-hole
// block: the same 48 180 triangles and nodes as viewMeshOptions in about a
// tenth of the time (a plane of 84 wires: 0.5 s with the default
// insertion, 30 ms), where a coarser deviation and angle (0.05 mm, 20
// degrees) took as long for a quarter of the triangles. Its deviation is
// the drawers' (setViewTessellation), so a drawer finds the triangulations
// fine enough and never meshes them again.
inline io::MeshOptions previewMeshOptions() {
  return io::MeshOptions{kViewDeviation, kViewAngle, false, kViewFaceTriangles, true};
}

// Shared by shaded face boundaries, wireframe and selection highlights.
inline void setViewTessellation(const occ::handle<Prs3d_Drawer>& drawer) {
  drawer->SetTypeOfDeflection(Aspect_TOD_ABSOLUTE);
  drawer->SetMaximalChordialDeviation(kViewDeviation);
  drawer->SetDeviationAngle(kViewAngle);
}

// Whether the view may mesh a shape it draws (its presentations' and
// selection's auto-triangulation): not when the job meshed it for display
// to the end (DisplayMeshes) and faces were left without a triangulation.
// Those are faces OCCT's mesher fails on, and meshing them again can take
// without end; the view would do it on the UI thread (mitcad#589). The
// view then draws the faces that have a triangulation. Nor a stand-in of
// the interactive tier (mitcad#635): its meshes are the preview's, and
// the final ones are made for its body by a job.
inline bool viewMeshes(const geometry::Shape& shape) {
  return !shape.display_interactive() && (!shape.display_meshed() || io::has_triangulation(shape.occt()));
}

// Makes every presentation mesh with the view's face bound (the meshing
// algorithm of the whole process; OcctViewer sets it up). The view's
// drawers then mesh what io::mesh_in_place(shape, viewMeshOptions())
// meshed the same way, and keep those triangulations.
inline void useViewMeshing() { io::use_bounded_presentation_meshes(kViewFaceTriangles); }

} // namespace mitcad
