// SPDX-License-Identifier: MIT
#pragma once

// Roundings that run over cut-outs or protrusions of their faces
// (mitcad#311).
//
// A rounding's contact on one of its faces may cross a region where the
// face's surface is interrupted: a cut-out (a flat, slot or hole cut into
// the face after it was made: material missing on the face's material
// side) or a protrusion (a lug or rib standing out of the face: material on
// its outer side). OCCT's rolling ball fails there (it rolls on the
// region's sharp rim), where the bodies stored in .f3d files keep the
// rounding on the face's own surface across the region and trim it by the
// region's faces, which grow out to meet it. That is the rounding of the
// body with the cut-outs filled and the protrusions taken away, back to the
// face's surface, with the regions put back afterwards: each as the region
// on the region's side of every face of the body that bounds it (its
// walls), near it; a cut-out cut out again, a protrusion joined again with
// the body's material there.

#include <optional>
#include <vector>

#include <TopoDS_Shape.hxx>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry::detail {

// A body with the cut-outs and protrusions of its selected edges' faces
// cleared.
struct Cutouts {
  // The cleared body, its faces named after the input's.
  ShapePtr body;
  // The sets of the cleared body's edges (as `owner` of the input's).
  std::vector<int> owner;
  // The regions cut out again (one solid per cut-out) and the body's
  // material joined again (one solid per protrusion).
  TopoDS_Shape recut;
  TopoDS_Shape rejoin;
  // The input's faces whose surfaces the cleared body and the result merge
  // faces on: the selected edges' faces and the regions' walls.
  std::vector<int> faces;
};

// The body with the cut-outs filled and the protrusions taken away that a
// rounding of the selected edges (`owner`, by edge: the set index,
// negative for none) runs over: `contact` is the largest distance from an
// edge at which the rounding touches its faces, `reach` the largest from
// an edge at which it changes the body. A cut-out is a region outside the
// body on the material side of the surface of one of the edges' faces, a
// protrusion a region of the body on its outer side, within `reach` of the
// edges, that meets the face along one of its other edges, lies on that
// surface and comes within `contact` of the edges. Each is taken only
// where the region on its side of every face of the body that bounds it
// (planes, cylinders, cones, spheres and tori) holds all of it and, for a
// cut-out, none of the body, for a protrusion, no other part of the body
// past the face's surface. None when no region is found.
std::optional<Cutouts> fill_cutouts(const Shape& body, const std::vector<int>& owner, double contact,
                                    double reach);

// The dressup of the cleared body (`rounded`) with the regions put back,
// its faces named after the dressup's and, where a region's face is new,
// after the input's face it lies on; the pieces of one face of the input,
// or of one selected edge's face, on one surface merged. Null when that is
// not one valid solid.
ShapePtr cut_out_again(const ShapePtr& rounded, const Cutouts& cutouts, const Shape& input);

} // namespace mitcad::geometry::detail
