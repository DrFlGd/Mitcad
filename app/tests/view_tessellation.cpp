// SPDX-License-Identifier: MIT
// Exercise the viewport's OCCT meshing path on synthetic STEP round trips.
#include "view/ViewTessellation.hpp"
#include "mitcad/io/step.hpp"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <filesystem>

#include <BRepAdaptor_Curve.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRep_Tool.hxx>
#include <NCollection_Sequence.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangulation.hxx>
#include <StdPrs_DeflectionCurve.hxx>
#include <StdPrs_ToolTriangulatedShape.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>

namespace {
int failures = 0;
void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "view_tessellation.cpp:%d: %s\n", line, expression);
    ++failures;
  }
}
#define CHECK(expr) check((expr), #expr, __LINE__)

double sag(const gp_Pnt& a, const gp_Pnt& b, const gp_Circ& circle) {
  const gp_Pnt middle((a.XYZ() + b.XYZ()) * 0.5);
  return circle.Radius() - middle.Distance(circle.Location());
}

void checkCylinder(const std::string& path, double radius, double height) {
  mitcad::io::Body body;
  body.name = "Cylinder";
  body.shape = BRepPrimAPI_MakeCylinder(radius, height).Shape();
  mitcad::io::write_step(path, {body});
  const auto imported = mitcad::io::read_step(path);
  CHECK(imported.size() == 1);
  if (imported.size() != 1) return;
  const TopoDS_Shape& shape = imported.front().shape;
  const occ::handle<Prs3d_Drawer> defaults = new Prs3d_Drawer();
  defaults->SetupOwnDefaults();
  mitcad::setViewTessellation(defaults);
  // AIS objects inherit the context's drawer instead of owning tolerances.
  const occ::handle<Prs3d_Drawer> drawer = new Prs3d_Drawer();
  drawer->SetLink(defaults);
  CHECK(StdPrs_ToolTriangulatedShape::Tessellate(shape, drawer));
  int circles = 0;
  double boundarySag = 0.0;
  double wireSag = 0.0;
  for (TopExp_Explorer face(shape, TopAbs_FACE); face.More(); face.Next()) {
    TopLoc_Location location;
    const auto mesh = BRep_Tool::Triangulation(TopoDS::Face(face.Current()), location);
    CHECK(!mesh.IsNull());
    if (mesh.IsNull()) continue;
    for (TopExp_Explorer edge(face.Current(), TopAbs_EDGE); edge.More(); edge.Next()) {
      const TopoDS_Edge& boundary = TopoDS::Edge(edge.Current());
      BRepAdaptor_Curve curve(boundary);
      if (curve.GetType() != GeomAbs_Circle) continue;
      ++circles;
      CHECK(std::abs(curve.Circle().Radius() - radius) < radius * 1e-6);
      const auto polygon = BRep_Tool::PolygonOnTriangulation(boundary, mesh, location);
      CHECK(!polygon.IsNull());
      if (polygon.IsNull()) continue;
      for (int n = 2; n <= polygon->NbNodes(); ++n) {
        const gp_Pnt a = mesh->Node(polygon->Node(n - 1)).Transformed(location.Transformation());
        const gp_Pnt b = mesh->Node(polygon->Node(n)).Transformed(location.Transformation());
        // At a 1000-pixel radius the boundary deviates by at most half a
        // pixel, including a small circle on a very long imported body.
        boundarySag = std::max(boundarySag, sag(a, b, curve.Circle()));
      }
      NCollection_Sequence<gp_Pnt> wire;
      StdPrs_DeflectionCurve::Add({}, curve,
          StdPrs_ToolTriangulatedShape::GetDeflection(shape, drawer), drawer, wire, false);
      CHECK(wire.Length() > 2);
      for (int n = 2; n <= wire.Length(); ++n) {
        wireSag = std::max(wireSag, sag(wire.Value(n - 1), wire.Value(n), curve.Circle()));
      }
    }
  }
  CHECK(circles >= 2);
  std::printf("Cylinder radius %g, height %g: boundary deviation %g, wire deviation %g\n",
              radius, height, boundarySag, wireSag);
  CHECK(boundarySag <= radius * 0.0005);
  CHECK(wireSag <= radius * 0.001);
}
} // namespace

int main(int argc, char** argv) {
  if (argc != 2) return 2;
  std::filesystem::create_directories(argv[1]);
  const std::string path = (std::filesystem::path(argv[1]) / "cylinder.step").string();
  try {
    checkCylinder(path, 0.02, 0.08);
    checkCylinder(path, 5.0, 20.0);
    checkCylinder(path, 5000.0, 20000.0);
    checkCylinder(path, 5.0, 50000.0);
  } catch (const Standard_Failure& error) {
    std::fprintf(stderr, "OCCT: %s\n", error.what());
    return 1;
  } catch (const std::exception& error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
  return failures == 0 ? 0 : 1;
}
