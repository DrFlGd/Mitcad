// SPDX-License-Identifier: MIT
#include "mitcad/geometry/contact.hpp"

#include <cstddef>

#include <BRepBndLib.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <Bnd_Box.hxx>
#include <Precision.hxx>
#include <TopAbs_State.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Pnt.hxx>

#include "sweep.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

// Shapes closer than this touch (boolean.cpp's kContact).
constexpr double kContact = 1.0e-6;

// A shape's box from its triangulation where it has one (BRepBndLib's default;
// detail::box_of in face_select.hpp reads the geometry only).
Bnd_Box triangulated_box_of(const TopoDS_Shape& shape) {
  Bnd_Box box;
  BRepBndLib::Add(shape, box);
  return box;
}

// Whether the point lies inside a solid of the shape (on its boundary is
// not inside).
bool inside(const TopoDS_Shape& shape, const gp_Pnt& point) {
  for (TopExp_Explorer solid(shape, TopAbs_SOLID); solid.More(); solid.Next()) {
    const BRepClass3d_SolidClassifier classifier(solid.Current(), point, Precision::Confusion());
    if (classifier.State() == TopAbs_IN) {
      return true;
    }
  }
  return false;
}

} // namespace

std::vector<ToolContact> tool_contacts(const Shape& tool, const std::vector<const Shape*>& bodies) {
  return detail::run("tool contacts", [&] {
    std::vector<ToolContact> contacts(bodies.size(), ToolContact::Apart);
    Bnd_Box tool_box = triangulated_box_of(tool.occt());
    if (tool_box.IsVoid()) {
      return contacts;
    }
    tool_box.Enlarge(kContact);
    // A point inside each solid of the tool, found once a body is touched.
    std::vector<gp_Pnt> points;
    bool found = false;
    for (std::size_t i = 0; i < bodies.size(); ++i) {
      // A mesh body is neither joined nor cut (mitcad#399): apart.
      if (detail::has_mesh_faces(bodies[i]->occt())) {
        continue;
      }
      const Bnd_Box box = triangulated_box_of(bodies[i]->occt());
      if (box.IsVoid() || tool_box.IsOut(box) || !detail::shapes_touch(*bodies[i], tool)) {
        continue;
      }
      contacts[i] = ToolContact::Touches;
      if (!found) {
        found = true;
        for (TopExp_Explorer solid(tool.occt(), TopAbs_SOLID); solid.More(); solid.Next()) {
          points.push_back(detail::interior_point(solid.Current()));
        }
      }
      for (const gp_Pnt& point : points) {
        if (!box.IsOut(point) && inside(bodies[i]->occt(), point)) {
          contacts[i] = ToolContact::Inside;
          break;
        }
      }
    }
    return contacts;
  });
}

} // namespace mitcad::geometry
