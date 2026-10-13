// SPDX-License-Identifier: MIT
#pragma once

// Planar sections of bodies and the clipped view of section analysis.

#include <cstddef>
#include <vector>

#include "mitcad/analysis/common.hpp"

namespace mitcad::analysis {

// A plane through `origin` with normal `normal` (any length but zero).
struct Plane {
  Vec3 origin;
  Vec3 normal{0.0, 0.0, 1.0};
};

struct Section {
  // Compound of the section edges (the intersection with all faces).
  TopoDS_Shape curves;
  // Compound of the planar faces where the plane cuts the solids, with
  // holes; empty when the shape has no solids.
  TopoDS_Shape faces;
  std::size_t edge_count = 0;
  std::size_t face_count = 0;
  double length = 0.0; // of the curves, mm
  double area = 0.0;   // of the faces, mm^2
};

// Intersects a shape with a plane.
Section section(const TopoDS_Shape& shape, const Plane& plane);

// A face of a clipped shape and what is left of it: its faces in the
// clipped shape, none when it lies wholly on the removed side.
struct ClippedFace {
  TopoDS_Shape face;
  std::vector<TopoDS_Shape> pieces;
};

// The shape with the half-space on the normal side of the plane removed,
// as section analysis shows it (the normal points towards the viewer).
// Faces lying on the plane stay whole: the removed half-space only touches
// them. With `faces`, what became of each face of the shape, in TopExp
// order (the caps on the plane come from none), so that the clipped
// shape's faces can keep the shape's names (mitcad#233).
TopoDS_Shape clip(const TopoDS_Shape& shape, const Plane& plane,
                  std::vector<ClippedFace>* faces = nullptr);

} // namespace mitcad::analysis
