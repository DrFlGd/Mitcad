// SPDX-License-Identifier: MIT
#pragma once

// Faces that are pieces of one surface joined into one face, and the edges
// that then run along one curve between the same two faces joined into one
// edge (mitcad#343): a thread's flanks, root and crest, cut a turn at a
// time, made one face each over all the turns, as the bodies stored in
// files keep threads. Unlike OCCT's face merge (ShapeUpgrade_UnifySameDomain)
// a face may run many periods of a periodic surface (a crest about a
// cylinder): its p-curves are unrolled over the periods. Internal to the
// geometry library.

#include <string>
#include <vector>

#include <Geom_Surface.hxx>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry::detail {

// `body` with those of the faces `faces` (indices) that share their surface
// (the same surface or copies of one, in the same location) and are
// connected through their edges each joined into one face holding all
// their names, and then each run of
// two or more edges of the joined faces that meet tangentially at vertices
// of no other edge, between the same two faces, joined into one edge. The
// other faces keep their geometry and names. Null where the result would
// not be valid; the shape unchanged where nothing is to be joined.
ShapePtr join_surface_pieces(const Shape& body, const std::vector<int>& faces);

// The result of an operation on join_surface_pieces' body (of `body` with
// its faces `faces` joined) with its faces split again where the pieces
// met, so that it keeps the pieces as `body` did: the faces over the joined
// pieces and the faces the operation generated along their edges (named
// face_name(feature, role, <edge>)) split by the planes of the edges
// between the pieces, over them and `reach` round them. A piece of a face
// over the joined pieces is named as the piece of `body` it lies on, a
// piece of a generated face as the face generated from the piece's edge it
// runs along; the pieces of any other face it splits are joined again.
// Null where the result would not be valid.
ShapePtr split_joined_pieces(const Shape& result, const Shape& body, const std::vector<int>& faces,
                             const std::string& feature, const std::string& role, double reach);

// Whether two surfaces are one: the same, or copies of one (a cylinder of
// the same frame and radius, a B-spline of the same poles and knots).
bool same_surface(const occ::handle<Geom_Surface>& a, const occ::handle<Geom_Surface>& b);

} // namespace mitcad::geometry::detail
