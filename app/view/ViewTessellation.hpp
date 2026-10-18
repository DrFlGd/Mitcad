// SPDX-License-Identifier: MIT
#pragma once

#include <cstddef>

#include <Prs3d_Drawer.hxx>

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
// than about 6 cm (a sphere up to about 20 mm radius), and planes,
// cylinders and cones, mesh at 0.01 mm.
constexpr double kViewDeviation = 0.01;
constexpr double kViewAngle = 5.0 * 3.14159265358979323846 / 180.0;
constexpr std::size_t kViewFaceTriangles = 20000;

inline io::MeshOptions viewMeshOptions() {
  return io::MeshOptions{kViewDeviation, kViewAngle, false, kViewFaceTriangles};
}

// Shared by shaded face boundaries, wireframe and selection highlights.
inline void setViewTessellation(const occ::handle<Prs3d_Drawer>& drawer) {
  drawer->SetTypeOfDeflection(Aspect_TOD_ABSOLUTE);
  drawer->SetMaximalChordialDeviation(kViewDeviation);
  drawer->SetDeviationAngle(kViewAngle);
}

// Makes every presentation mesh with the view's face bound (the meshing
// algorithm of the whole process; OcctViewer sets it up). The view's
// drawers then mesh what io::mesh_in_place(shape, viewMeshOptions())
// meshed the same way, and keep those triangulations.
inline void useViewMeshing() { io::use_bounded_presentation_meshes(kViewFaceTriangles); }

} // namespace mitcad
