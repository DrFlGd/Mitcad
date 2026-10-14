// SPDX-License-Identifier: MIT
#pragma once

#include <Prs3d_Drawer.hxx>

namespace mitcad {

// Shared by shaded face boundaries, wireframe and selection highlights.
// OCCT's relative default grows with the whole body's bounds. Its derived
// minimum mesh size can then override the angular limit on small curves
// of a long body. Bound the chordal error in model units (millimetres),
// with an angular limit for circles smaller than that linear tolerance.
inline void setViewTessellation(const occ::handle<Prs3d_Drawer>& drawer) {
  drawer->SetTypeOfDeflection(Aspect_TOD_ABSOLUTE);
  drawer->SetMaximalChordialDeviation(0.01);
  drawer->SetDeviationAngle(5.0 * 3.14159265358979323846 / 180.0);
}

} // namespace mitcad
