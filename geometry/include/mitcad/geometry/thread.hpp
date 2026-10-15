// SPDX-License-Identifier: MIT
#pragma once

// Modelled screw threads: the 60-degree profile (ISO metric, Unified) of
// given diameters built on cylindrical faces.

#include <string>
#include <vector>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// Lengths in millimetres, angles in radians.
//
// The profile is the basic profile of ISO 68-1 (also Unified): flanks at 30
// degrees to the radial direction, the thread exactly half a pitch wide at
// the pitch diameter, cut off by flat crests and roots at the major and
// minor diameters (no rounding). An external thread has its crests on the
// major diameter and its roots on the minor one, an internal thread the
// other way round. The basic diameters (D, D1 = D - 5H/4, D2 = D - 3H/4,
// H = 0.866025 P) give the basic profile; a tolerance class's diameters
// (each the middle of its tolerance) give the class's profile with the
// same flanks. Extra clearance (`radial_clearance`, `flank_clearance`)
// moves and thins that profile for a looser fit.
//
// Along the threaded part of a face the material becomes exactly the
// thread: what lies between the root and the face beyond the profile is
// removed, and where the face lies inside the crest (a shaft thinner than
// the major diameter, a bore wider than the minor one) the teeth are added
// up to the crest. The part runs between planes across the axis; at an end
// of the face with no material beyond it (the free end of a shaft, the
// mouth of a hole) the thread runs out through it.
//
// The helix: take the axis pointing up (+Z; axes across Z towards +Y, the
// X axis towards +X), whichever way the face's surface runs. From the
// face's end at the low end of that axis, at the reference direction (the
// world X axis projected onto the plane across the axis; the Y axis for
// axes within about 25 degrees of X) turned by `angle` about the axis
// (right-hand rule), an external thread's groove spans the first half
// pitch at the pitch diameter, an internal thread's tooth the same half
// pitch (so that a bolt and a nut threaded from the same plane mate).
struct ThreadSpec {
  std::string feature;
  // Cylindrical faces of the body (references; all pieces of each).
  std::vector<std::string> faces;
  double pitch = 0.0;
  // Diameters: the major one, the minor one and the pitch diameter.
  double major = 0.0;
  double minor = 0.0;
  double pitch_diameter = 0.0;
  bool right_handed = true;
  double angle = 0.0;
  // Over the whole face; otherwise `length` from `offset` measured from
  // the end the cylinder's axis points to (`high_end`) or from the other
  // end.
  bool full_length = true;
  double length = 0.0;
  double offset = 0.0;
  bool high_end = true;
  // Extra clearance (mitcad#195), both 0 or more. `radial_clearance` moves
  // the whole profile (crests, roots, flanks and the pitch diameter) along
  // the radius, outwards for an internal thread and inwards for an
  // external one: 0.2 changes each diameter by 0.4. Then
  // `flank_clearance` moves each flank by that distance perpendicular to
  // itself into the tooth (an external thread's tooth thinner, an internal
  // thread's groove wider): the flanks keep their angle and the pitch, and
  // the crests and roots their radii, so each flank moves
  // flank_clearance / cos 30 along the axis and the crests narrow by twice
  // that. A clearance that leaves too narrow a crest (under P/50) or no
  // core is refused.
  double radial_clearance = 0.0;
  double flank_clearance = 0.0;
  // Cut the profile; false: only the end chamfers (a cosmetic thread's).
  bool groove = true;
  // End chamfers (mitcad#304): before the profile, a cone is cut at the
  // chosen ends of each face, so that the first turn starts on the cone and
  // leaves no knife edge. An external thread's cone widens from
  // `chamfer_diameter` at the end into the material at `chamfer_angle` to
  // the axis, and everything outside it goes; an internal thread's (a
  // countersink) narrows from `chamfer_diameter` at the end, and
  // everything inside it goes. `chamfer_ends` holds per face (in the order
  // of `faces`) whether its low and its high end along the axis of the
  // face's cylinder get one; an end gets one only where the thread reaches
  // it and nothing of the body lies beyond it (a shaft's free end, a
  // hole's mouth). The cones' faces are named
  // <feature>:thread_chamfer_low(<face reference>) and
  // <feature>:thread_chamfer_high(<face reference>).
  struct ChamferEnds {
    bool low = false;
    bool high = false;
  };
  bool chamfer = false;
  double chamfer_angle = 0.0;
  double chamfer_diameter = 0.0;
  std::vector<ChamferEnds> chamfer_ends;
};

// The body with the threads. The thread's new faces (flanks, crests and
// roots) are named <feature>:thread(<face reference>).
//
// The work is local: only the body's faces near the thread take part, so
// a thread on a body of thousands of faces takes about as long as on a
// small one.
ShapePtr modeled_thread(const Shape& body, const ThreadSpec& spec);

} // namespace mitcad::geometry
