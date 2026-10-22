// SPDX-License-Identifier: MIT
#include "mitcad/geometry/face_extrude.hpp"

#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

#include <BRepAdaptor_Surface.hxx>
#include <BRep_Tool.hxx>
#include <GeomAbs_SurfaceType.hxx>
#include <Geom_Surface.hxx>
#include <NCollection_IndexedMap.hxx>
#include <TopAbs.hxx>
#include <TopExp.hxx>
#include <TopLoc_Location.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Dir.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

#include "history.hpp"
#include "mitcad/geometry/naming.hpp"
#include "sweep.hpp"
#include "util.hpp"

namespace mitcad::geometry {

ShapePtr extrude_face(const Shape& body, const FaceExtrudeSpec& spec) {
  detail::require_positive("extrude distance", spec.distance1);
  if (spec.distance2) {
    detail::require_positive("extrude distance 2", *spec.distance2);
  }
  detail::require_finite("start offset", spec.start_offset);
  return detail::run("extrude face", [&] {
    const std::vector<int> found = body.find_faces(spec.face);
    if (found.empty()) {
      throw std::invalid_argument("the body has no face " + spec.face);
    }
    if (found.size() > 1) {
      throw std::invalid_argument(spec.face + " names " + std::to_string(found.size()) +
                                  " faces; an extrusion takes one face");
    }
    const TopoDS_Face& face = body.face(found.front());
    TopLoc_Location location;
    if (BRep_Tool::Surface(face, location).IsNull()) {
      throw std::invalid_argument("face " + spec.face +
                                  " has no surface (a mesh body's triangles): an extrusion takes a "
                                  "planar face");
    }
    const BRepAdaptor_Surface surface(face);
    if (surface.GetType() != GeomAbs_Plane) {
      throw std::invalid_argument("face " + spec.face + " is not planar: an extrusion takes a planar face");
    }
    // Out of the material: the plane's normal as the face is oriented.
    gp_Dir normal = surface.Plane().Axis().Direction();
    if (face.Orientation() == TopAbs_REVERSED) {
      normal.Reverse();
    }
    const gp_Dir along = spec.reversed ? normal.Reversed() : normal;

    // The face copied, so that the prism shares nothing with the body, its
    // edges named after the body's.
    const detail::InputCopy copy(face);
    detail::SweepFace sweep{TopoDS::Face(copy.shape()), {}, {spec.key}};
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges;
    TopExp::MapShapes(face, TopAbs_EDGE, edges);
    for (int i = 1; i <= edges.Extent(); ++i) {
      const TopoDS_Edge& edge = TopoDS::Edge(edges(i));
      const std::optional<std::string> name = body.name_of_edge(edge);
      const std::string key = name ? *name : std::to_string(i - 1);
      sweep.edges.emplace_back(TopoDS::Edge(copy.of(edge)), face_name(spec.feature, "side", key));
    }

    // Both sides in one prism, from the far end of side two; with two
    // sides the far cap of side one is the start and that of side two the
    // end, as an extrusion of sketch profiles names them.
    const bool two = spec.distance2.has_value();
    const double low = two ? -*spec.distance2 : 0.0;
    const double length = spec.distance1 - low;
    gp_Trsf place;
    place.SetTranslation(gp_Vec(normal) * spec.start_offset + gp_Vec(along) * low);
    const NameList start{face_name(spec.feature, "start", spec.key)};
    const NameList end{face_name(spec.feature, "end", spec.key)};
    const ShapePtr prism = detail::named_prism(detail::transformed(sweep, place), gp_Vec(along) * length,
                                               two ? end : start, two ? start : end);
    return detail::finished(*prism, "the extrusion of the face");
  });
}

} // namespace mitcad::geometry
