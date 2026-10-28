// SPDX-License-Identifier: MIT
#include "DragProxy.hpp"

#include <algorithm>
#include <cmath>
#include <utility>

#include <BRep_Tool.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangulation.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>

namespace mitcad {

std::shared_ptr<const ProxyMesh> proxyMeshOf(const TopoDS_Shape& shape) {
  if (shape.IsNull()) {
    return nullptr;
  }
  auto mesh = std::make_shared<ProxyMesh>();
  // Each edge once (shared by two faces): from the first face that has a
  // polygon of it.
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> outlined;
  for (TopExp_Explorer face(shape, TopAbs_FACE); face.More(); face.Next()) {
    const TopoDS_Face& current = TopoDS::Face(face.Current());
    TopLoc_Location location;
    const occ::handle<Poly_Triangulation> triangulation = BRep_Tool::Triangulation(current, location);
    if (triangulation.IsNull() || triangulation->NbNodes() == 0) {
      continue;
    }
    const gp_Trsf placed = location.Transformation();
    const int offset = static_cast<int>(mesh->nodes.size());
    for (int i = 1; i <= triangulation->NbNodes(); ++i) {
      mesh->nodes.push_back(triangulation->Node(i).Transformed(placed));
    }
    const bool reversed = current.Orientation() == TopAbs_REVERSED;
    for (int i = 1; i <= triangulation->NbTriangles(); ++i) {
      int a = 0;
      int b = 0;
      int c = 0;
      triangulation->Triangle(i).Get(a, b, c);
      if (reversed) {
        std::swap(b, c);
      }
      mesh->triangles.push_back({offset + a - 1, offset + b - 1, offset + c - 1});
    }
    for (TopExp_Explorer edge(current, TopAbs_EDGE); edge.More(); edge.Next()) {
      if (outlined.Contains(edge.Current())) {
        continue;
      }
      const occ::handle<Poly_PolygonOnTriangulation> polygon =
          BRep_Tool::PolygonOnTriangulation(TopoDS::Edge(edge.Current()), triangulation, location);
      if (polygon.IsNull() || polygon->NbNodes() < 2) {
        continue;
      }
      outlined.Add(edge.Current());
      std::vector<int> line;
      line.reserve(static_cast<std::size_t>(polygon->NbNodes()));
      for (int i = 1; i <= polygon->NbNodes(); ++i) {
        line.push_back(offset + polygon->Node(i) - 1);
      }
      mesh->outlines.push_back(std::move(line));
    }
  }
  if (mesh->triangles.empty()) {
    return nullptr;
  }
  return mesh;
}

std::optional<DragProxy> applyDragProxy(const ProxyMesh& mesh, const DragProxyRule& rule,
                                        const std::vector<gp_Dir>& axes) {
  if (mesh.nodes.empty() || !std::isfinite(rule.from) || !std::isfinite(rule.to)) {
    return std::nullopt;
  }
  constexpr double kFlat = 1e-9;
  const bool stretch = rule.kind != DragProxyRule::Kind::Translate;
  if (stretch && (std::abs(rule.from) < kFlat || std::abs(rule.to) < kFlat)) {
    return std::nullopt; // a flat tool, or none to stretch from
  }
  if (rule.kind == DragProxyRule::Kind::StretchSide && (rule.from < 0.0 || rule.to < 0.0)) {
    return std::nullopt; // the side would turn through the fixed plane
  }
  for (const gp_Dir& axis : axes) {
    const gp_XYZ along = axis.XYZ();
    double low = std::numeric_limits<double>::infinity();
    double high = -std::numeric_limits<double>::infinity();
    for (const gp_Pnt& node : mesh.nodes) {
      const double h = node.XYZ().Dot(along);
      low = std::min(low, h);
      high = std::max(high, h);
    }
    if (std::isfinite(rule.length)) {
      const double tolerance = 1e-4 * std::max(1.0, std::abs(rule.length)) + 1e-6;
      if (std::abs((high - low) - std::abs(rule.length)) > tolerance) {
        continue; // not the arrow's axis in this mesh
      }
    }
    DragProxy proxy;
    proxy.axis = axis;
    proxy.fixed = high - rule.reach;
    auto moved = std::make_shared<ProxyMesh>();
    moved->nodes.reserve(mesh.nodes.size());
    const double factor = stretch ? rule.to / rule.from : 1.0;
    const double side = 1e-9 * std::max(1.0, std::abs(high - low));
    for (const gp_Pnt& node : mesh.nodes) {
      const double h = node.XYZ().Dot(along);
      double shift = 0.0;
      switch (rule.kind) {
      case DragProxyRule::Kind::Stretch:
        shift = (h - proxy.fixed) * (factor - 1.0);
        break;
      case DragProxyRule::Kind::StretchSide:
        shift = h > proxy.fixed + side ? (h - proxy.fixed) * (factor - 1.0) : 0.0;
        break;
      case DragProxyRule::Kind::Translate:
        shift = rule.to - rule.from;
        break;
      }
      moved->nodes.push_back(gp_Pnt(node.XYZ() + along * shift));
    }
    moved->triangles = mesh.triangles;
    if (factor < 0.0) {
      // Mirrored through the fixed plane: the triangles turn inside out.
      for (std::array<int, 3>& triangle : moved->triangles) {
        std::swap(triangle[1], triangle[2]);
      }
    }
    moved->outlines = mesh.outlines;
    proxy.mesh = std::move(moved);
    return proxy;
  }
  return std::nullopt;
}

gp_Trsf draggedPlacement(const gp_Trsf& placement, MotionSide side, const gp_Trsf& from, const gp_Trsf& to,
                         const gp_Trsf& frame) {
  // The change: back from where the exact preview put it, then where the
  // dragged value puts it.
  const gp_Trsf change = to * from.Inverted();
  switch (side) {
  case MotionSide::Shape:
    return placement * change;
  case MotionSide::Occurrence:
    return frame * change * frame.Inverted() * placement;
  case MotionSide::None:
    break;
  }
  return placement;
}

bool sameMotion(const gp_Trsf& a, const gp_Trsf& b, double tolerance) {
  for (int row = 1; row <= 3; ++row) {
    for (int column = 1; column <= 4; ++column) {
      if (std::abs(a.Value(row, column) - b.Value(row, column)) > tolerance) {
        return false;
      }
    }
  }
  return true;
}

} // namespace mitcad
