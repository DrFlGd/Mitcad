// SPDX-License-Identifier: MIT
#include "mitcad/analysis/section.hpp"

#include <algorithm>
#include <cmath>
#include <utility>

#include <BRep_Builder.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_List.hxx>
#include <Precision.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Pln.hxx>

#include "convert.hpp"
#include "mesh_section.hpp"

namespace mitcad::analysis {

namespace {

// Builds the boolean of `object` with `tool` without changing either: the
// shape is a body of the model, which other results share.
void build_keeping_inputs(BRepAlgoAPI_BooleanOperation& operation, const TopoDS_Shape& object,
                          const TopoDS_Shape& tool) {
  NCollection_List<TopoDS_Shape> objects;
  objects.Append(object);
  NCollection_List<TopoDS_Shape> tools;
  tools.Append(tool);
  operation.SetArguments(objects);
  operation.SetTools(tools);
  operation.SetNonDestructive(true);
  operation.Build();
}

gp_Dir direction(const Plane& plane) {
  const gp_Vec normal(plane.normal.x, plane.normal.y, plane.normal.z);
  if (normal.Magnitude() <= gp::Resolution()) {
    throw Error("the section plane needs a non-zero normal");
  }
  return gp_Dir(normal);
}

// The shape's box; an empty shape cannot be sectioned.
Bounds box_of(const TopoDS_Shape& shape) {
  const Bounds box = bounds(shape);
  if (box.empty) {
    throw Error("cannot section an empty shape");
  }
  return box;
}

// A square face in the plane, centred where the plane meets the line from
// the box centre along the normal, large enough to cover the box.
// `distance` is the signed distance of the box centre from the plane.
TopoDS_Face cover(const Bounds& box, const Plane& plane, double& size, double& distance) {
  const Vec3 extent = box.size();
  size = std::sqrt(extent.x * extent.x + extent.y * extent.y + extent.z * extent.z) + 1.0;
  const gp_Dir normal = direction(plane);
  const gp_Pnt center(0.5 * (box.min.x + box.max.x), 0.5 * (box.min.y + box.max.y),
                      0.5 * (box.min.z + box.max.z));
  distance = gp_Vec(detail::pnt(plane.origin), center).Dot(gp_Vec(normal));
  const gp_Pnt on_plane = center.Translated(gp_Vec(normal) * -distance);
  return BRepBuilderAPI_MakeFace(gp_Pln(on_plane, normal), -size, size, -size, size).Face();
}

} // namespace

PlaneReach plane_reach(const Bounds& box, const Plane& plane) {
  const gp_Vec normal(direction(plane));
  PlaneReach reach;
  if (box.empty) {
    return reach;
  }
  // Along each axis the nearer and the farther end of the box.
  const gp_Pnt origin = detail::pnt(plane.origin);
  const double low[3] = {box.min.x, box.min.y, box.min.z};
  const double high[3] = {box.max.x, box.max.y, box.max.z};
  const double at[3] = {origin.X(), origin.Y(), origin.Z()};
  const double n[3] = {normal.X(), normal.Y(), normal.Z()};
  for (int i = 0; i < 3; ++i) {
    const double a = (low[i] - at[i]) * n[i];
    const double b = (high[i] - at[i]) * n[i];
    reach.min += std::min(a, b);
    reach.max += std::max(a, b);
  }
  return reach;
}

ClipSide clip_side(const Bounds& box, const Plane& plane) {
  const PlaneReach reach = plane_reach(box, plane);
  if (box.empty) {
    return ClipSide::Cut;
  }
  // Behind first: a face lying on the plane stays (a sheet in the plane).
  if (reach.max <= Precision::Confusion()) {
    return ClipSide::Kept;
  }
  if (reach.min >= -Precision::Confusion()) {
    return ClipSide::Removed;
  }
  return ClipSide::Cut;
}

Section section(const TopoDS_Shape& shape, const Plane& plane) {
  if (shape.IsNull()) {
    throw Error("cannot section a null shape");
  }
  if (detail::mesh_only(shape)) {
    return detail::mesh_section(shape, plane);
  }
  Section result;
  BRepAlgoAPI_Section curves(shape, gp_Pln(detail::pnt(plane.origin), direction(plane)), false);
  curves.Approximation(true);
  // The shape is a body of the model, which other results share: the
  // booleans here leave its sub-shapes as they are.
  curves.SetNonDestructive(true);
  curves.Build();
  if (!curves.IsDone() || curves.HasErrors()) {
    throw Error("the section could not be computed");
  }
  result.curves = curves.Shape();
  for (TopExp_Explorer it(result.curves, TopAbs_EDGE); it.More(); it.Next()) {
    ++result.edge_count;
  }
  GProp_GProps linear;
  BRepGProp::LinearProperties(result.curves, linear);
  result.length = linear.Mass();

  const TopoDS_Shape solids = oriented_solids(shape);
  if (!TopExp_Explorer(solids, TopAbs_SOLID).More()) {
    return result;
  }
  double size = 0.0;
  double distance = 0.0;
  const TopoDS_Face disk = cover(box_of(solids), plane, size, distance);
  // The part of the plane inside the solids, holes included.
  BRepAlgoAPI_Common common;
  build_keeping_inputs(common, disk, solids);
  if (!common.IsDone() || common.HasErrors()) {
    throw Error("the section faces could not be computed");
  }
  result.faces = common.Shape();
  for (TopExp_Explorer it(result.faces, TopAbs_FACE); it.More(); it.Next()) {
    ++result.face_count;
  }
  result.area = surface_properties(result.faces).Mass();
  return result;
}

TopoDS_Shape clip(const TopoDS_Shape& shape, const Plane& plane, std::vector<ClippedFace>* faces) {
  if (shape.IsNull()) {
    throw Error("cannot clip a null shape");
  }
  if (detail::mesh_only(shape)) {
    return detail::mesh_clip(shape, plane, faces);
  }
  const Bounds box = box_of(shape);
  // A shape the plane does not reach is kept or removed as a whole without
  // a boolean (mitcad#438): Section Analysis clips every body shown, and
  // most lie on one side.
  const ClipSide side = clip_side(box, plane);
  if (side != ClipSide::Cut) {
    if (faces) {
      NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> own;
      TopExp::MapShapes(shape, TopAbs_FACE, own);
      faces->clear();
      for (int i = 1; i <= own.Extent(); ++i) {
        ClippedFace face{own(i), {}};
        if (side == ClipSide::Kept) {
          face.pieces.push_back(face.face);
        }
        faces->push_back(std::move(face));
      }
    }
    if (side == ClipSide::Kept) {
      return shape;
    }
    TopoDS_Compound nothing;
    BRep_Builder().MakeCompound(nothing);
    return nothing;
  }
  double size = 0.0;
  double distance = 0.0;
  const TopoDS_Face disk = cover(box, plane, size, distance);
  // A slab from the plane over the whole shape on the normal side.
  const double depth = std::abs(distance) + size;
  const TopoDS_Shape removed =
      BRepPrimAPI_MakePrism(disk, gp_Vec(direction(plane)) * depth).Shape();
  BRepAlgoAPI_Cut cut;
  build_keeping_inputs(cut, shape, removed);
  if (!cut.IsDone() || cut.HasErrors()) {
    throw Error("the shape could not be clipped");
  }
  const TopoDS_Shape& result = cut.Shape();
  if (faces) {
    // Each face's images; a face the cut leaves as it is is its own.
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> kept;
    TopExp::MapShapes(result, TopAbs_FACE, kept);
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> own;
    TopExp::MapShapes(shape, TopAbs_FACE, own);
    faces->clear();
    for (int i = 1; i <= own.Extent(); ++i) {
      ClippedFace face{own(i), {}};
      if (!cut.IsDeleted(face.face)) {
        for (const TopoDS_Shape& image : cut.Modified(face.face)) {
          if (image.ShapeType() == TopAbs_FACE && kept.Contains(image)) {
            face.pieces.push_back(image);
          }
        }
        if (face.pieces.empty() && kept.Contains(face.face)) {
          face.pieces.push_back(face.face);
        }
      }
      faces->push_back(std::move(face));
    }
  }
  return result;
}

} // namespace mitcad::analysis
