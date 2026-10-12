// SPDX-License-Identifier: MIT
#pragma once

// The faces a run of features made, as solids to copy: a pattern's
// "optimized" compute option copies the result faces of the features it
// repeats instead of working the features out again at each element.
//
// What features did to a body is what lies between the body before them
// and the body after them: the material they added (a boss with the
// roundings at its foot and on its top) and the material they removed (a
// pocket with its chamfer). Each piece is a closed solid whose faces are
// the result faces the features made, with the faces of the body they
// stood on as its seam; copied to an element and joined to (or cut from)
// the body there it gives the body the features' faces give, without a
// rounding or chamfer worked out again on the copy.
//
// A copy of added material stands on the body only where its seam lies
// on the body's faces: where the faces of the body under the seam end (a
// groove beside the boss, the edge of a plate) the material over that
// part has nothing to stand on and is left out (supported_copies).

#include <vector>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// Material that `after` has and `before` lacks: the solid and its seam, the
// planar faces of the solid that lie on faces of `before` (a compound of
// those faces, as the solid has them; null when there are none).
struct AddedMaterial {
  ShapePtr solid;
  ShapePtr seam;
};

struct FeatureChanges {
  // after minus before, to join to a copy's body.
  std::vector<AddedMaterial> added;
  // before minus after: the material `after` lacks, to cut from it.
  std::vector<ShapePtr> removed;
};

// The material `after` has and `before` lacks and the material `before`
// has and `after` lacks, as closed solids (a body of one operation's
// results next to the body it came from: their unchanged faces coincide).
// The boolean runs only around the faces where the two differ
// (removed.hpp), so that a large body costs what the features' own
// neighbourhood does. Both empty when the bodies do not differ. Face names
// carry through from both bodies. Slivers along faces the two share are
// left out.
FeatureChanges feature_changes(const Shape& before, const Shape& after);

// The part of each copy of added material that stands on `body`: `solids[i]`
// with its `seams[i]` (null: none) moved to the same place. The material
// over a part of a planar seam face that no planar face of the body lies
// under (coplanar, facing it) is cut off, along the seam's normal; a copy
// whose seam lies wholly on the body's faces is as it is, one with nothing
// under it is null. Copies without a seam are as they are.
std::vector<ShapePtr> supported_copies(const Shape& body, const std::vector<ShapePtr>& solids,
                                       const std::vector<ShapePtr>& seams);

} // namespace mitcad::geometry
