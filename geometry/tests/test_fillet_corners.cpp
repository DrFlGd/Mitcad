// SPDX-License-Identifier: MIT
// Corners where a rounding ends, built here with OCCT's fillet alone on
// synthetic shapes: the cases behind the patches 0130 and later of the OCCT
// port (third_party/vcpkg-ports, mitcad#236).

#include <cmath>
#include <cstdio>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Ax2.hxx>

#include "check.hpp"

namespace test {
namespace {

// The volume, integrated to a relative error of 1e-9 (OCCT's default
// integration errs by more than the differences checked here on the
// roundings' spline faces).
double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props, 1e-9, true);
  return props.Mass();
}

int face_count(const TopoDS_Shape& shape) {
  int n = 0;
  for (TopExp_Explorer e(shape, TopAbs_FACE); e.More(); e.Next()) {
    ++n;
  }
  return n;
}

// Builds the operation; false when OCCT throws or does not finish.
bool build(BRepFilletAPI_MakeFillet& op) {
  try {
    OCC_CATCH_SIGNALS
    op.Build();
  } catch (const Standard_Failure& failure) {
    std::fprintf(stderr, "fillet corner test: %s\n", failure.what());
    return false;
  }
  return op.IsDone();
}

// A block 40 x 20 x 25 whose four vertical edges are rounded with a radius
// of 2, cut to a vault: below z = 10 the block, above it a cylinder of
// radius r along x whose axis lies at y = 10, z = 10. With r = 10 the vault
// springs tangentially from the walls y = 0 and y = 20; a larger r makes it
// cross them at an angle of sqrt(2 (r - 10) / 10).
TopoDS_Shape vaulted_block(double r) {
  const TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(0, 0, 0), gp_Pnt(40, 20, 25)).Shape();
  BRepFilletAPI_MakeFillet corners(block);
  for (TopExp_Explorer e(block, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    if (std::abs(a.X() - b.X()) < 1e-9 && std::abs(a.Y() - b.Y()) < 1e-9) {
      corners.Add(2.0, edge);
    }
  }
  corners.Build();
  const TopoDS_Shape lower = BRepPrimAPI_MakeBox(gp_Pnt(-1, -1, -1), gp_Pnt(41, 21, 10)).Shape();
  const TopoDS_Shape vault =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(-1, 10, 10), gp_Dir(1, 0, 0)), r, 42).Shape();
  const TopoDS_Shape tool = BRepAlgoAPI_Fuse(lower, vault).Shape();
  ShapeUpgrade_UnifySameDomain unify(BRepAlgoAPI_Common(corners.Shape(), tool).Shape(), true, true, true);
  unify.Build();
  return unify.Shape();
}

// The edges of the end x = 0 above the vault's springing: the arc on the end
// face and the two curves where the vault meets the corner roundings.
void add_vault_end(BRepFilletAPI_MakeFillet& fillet, const TopoDS_Shape& body, double radius) {
  for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge));
    const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge));
    const BRepAdaptor_Curve curve(edge);
    const gp_Pnt m = curve.Value(0.5 * (curve.FirstParameter() + curve.LastParameter()));
    const bool vertical = std::abs(a.X() - b.X()) < 1e-9 && std::abs(a.Y() - b.Y()) < 1e-9;
    if (!vertical && m.X() < 2 + 1e-6 && m.Z() > 10 + 1e-6 && a.Z() > 10 - 1e-6 && b.Z() > 10 - 1e-6 &&
        fillet.Contour(edge) == 0) {
      fillet.Add(radius, edge);
    }
  }
}

// The vault's end rounded with a radius of 1 (patch 0130): the curves where
// the vault meets the corner roundings end where both come tangent to the
// wall, and the rounding vanishes there. With a vault that misses tangency by
// a micrometre (radius 10.000001, faces 0.00045 rad apart at the vertex)
// OCCT stopped with an end section of micrometres off the vertex and failed
// at the corner; with radius 10.00001 it built a cap of micrometres whose
// wire intersects itself. Now both end in a point at the vertex, as the
// tangent vault (radius 10) does, and remove the same volume.
void test_rounding_vanishing_where_its_faces_come_tangent() {
  double tangent_removed = 0.0;
  for (const double r : {10.0, 10.000001, 10.00001}) {
    const TopoDS_Shape body = vaulted_block(r);
    BRepFilletAPI_MakeFillet fillet(body);
    add_vault_end(fillet, body, 1.0);
    CHECK(fillet.NbContours() == 1);
    CHECK(build(fillet));
    if (!fillet.IsDone()) {
      continue;
    }
    CHECK(BRepCheck_Analyzer(fillet.Shape()).IsValid());
    CHECK(face_count(fillet.Shape()) == face_count(body) + 5);
    const double removed = volume_of(body) - volume_of(fillet.Shape());
    if (r == 10.0) {
      tangent_removed = removed;
      CHECK(removed > 4.8 && removed < 4.9);
    } else {
      CHECK_NEAR_TOL(removed, tangent_removed, 1e-4);
    }
  }
}

} // namespace

void fillet_corner_tests() {
  guarded("test_rounding_vanishing_where_its_faces_come_tangent",
          test_rounding_vanishing_where_its_faces_come_tangent);
}

} // namespace test
