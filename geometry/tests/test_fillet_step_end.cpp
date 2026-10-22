// SPDX-License-Identifier: MIT
// A block's edge rounded up to a vertex where its wall steps back into the
// material by much less than the radius, built with OCCT's fillet alone:
// the wall's face at the edge ends on a narrow strip (the step's face,
// square to the rounding), beyond which the wall goes on a few micrometres
// further in. OCCT found no face to end the rounding on and left the shape
// invalid. The rounding now ends at the section through the vertex and its
// ball turns on about its contact's end until it lies in the wall's plane,
// a quarter torus cut by the stepped-back wall. A stored design with a
// rim whose wall steps back by 4.3 micrometres is the case behind the patch
// 0951 of the OCCT port (third_party/vcpkg-ports, mitcad#343).

#include <cmath>
#include <cstdio>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <gp_Pnt.hxx>

#include "check.hpp"

namespace test {
namespace {

constexpr double kStep = 0.0043; // how far the wall steps back
constexpr double kRound = 0.3;   // the rounding's radius
constexpr double kLength = 8.0;  // the rounded edge's length

// The volume, integrated to a relative error of 1e-9.
double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props, 1e-9, true);
  return props.Mass();
}

// Simpson's rule on [a, b] in n (even) steps.
template <typename F> double simpson(F&& f, double a, double b, int n) {
  const double h = (b - a) / n;
  double sum = f(a) + f(b);
  for (int i = 1; i < n; ++i) {
    sum += f(a + i * h) * (i % 2 == 1 ? 4 : 2);
  }
  return sum * h / 3;
}

// The material one rounded rim loses: the rounding's section, the square
// of the radius less the quarter disk, along the edge, and at the end what
// the turning ball takes. At the distance d from the wall's plane of the
// ball's centres, the torus leaves a quarter disk of radius
// a = R - sqrt(R^2 - d^2) about the contact's end point, of which only the
// part beyond the stepped-back wall is material.
double rim_loss() {
  const double along = kLength * (1 - kPi / 4) * kRound * kRound;
  const double end = simpson(
    [](double d) {
      const double a = kRound - std::sqrt(std::max(0.0, kRound * kRound - d * d));
      if (a <= kStep) {
        return 0.0;
      }
      // (The quarter disk's area beyond y = kStep.)
      const double t = std::acos(kStep / a);
      return 0.5 * a * a * (t - std::sin(t) * std::cos(t));
    },
    0.0, kRound, 4000);
  return along + end;
}

// The block x in [-7, 7], y in [0, 10], z in [15, 30] (mirrored: z in
// [-30, -15]), its wall y = 0 stepped back by kStep over z in [21, 22]
// (above: [23, 24]); the edges of the wall with the side x = 7 (both: and
// x = -7) on the longer side of the step rounded.
struct Case {
  bool above;  // the step above the rounded edge, at its other end
  bool both;   // both sides' edges rounded
  bool mirror; // the block mirrored in z = 0
};

void check_case(const Case& c) {
  const double s = c.mirror ? -1.0 : 1.0;
  const TopoDS_Shape block =
    BRepPrimAPI_MakeBox(gp_Pnt(-7, 0, c.mirror ? -30 : 15), gp_Pnt(7, 10, c.mirror ? -15 : 30)).Shape();
  const double z0 = c.above ? 23 : 21;
  const double n0 = std::min(s * z0, s * (z0 + 1)), n1 = std::max(s * z0, s * (z0 + 1));
  const TopoDS_Shape notch = BRepPrimAPI_MakeBox(gp_Pnt(-8, -1, n0), gp_Pnt(8, kStep, n1)).Shape();
  const TopoDS_Shape body = BRepAlgoAPI_Cut(block, notch).Shape();
  BRepFilletAPI_MakeFillet fillet(body);
  int added = 0;
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges;
  TopExp::MapShapes(body, TopAbs_EDGE, edges);
  for (int i = 1; i <= edges.Extent(); ++i) {
    BRepAdaptor_Curve e(TopoDS::Edge(edges(i)));
    const gp_Pnt a = e.Value(e.FirstParameter()), b = e.Value(e.LastParameter());
    const bool side = std::abs(a.X() - 7) < 1e-9 || (c.both && std::abs(a.X() + 7) < 1e-9);
    const bool along = std::abs(a.X() - b.X()) < 1e-9 && std::abs(a.Y()) < 1e-9 && std::abs(b.Y()) < 1e-9;
    const double zmid = s * 0.5 * (a.Z() + b.Z());
    const bool longer = c.above ? zmid < z0 : zmid > z0;
    if (side && along && longer) {
      fillet.Add(kRound, TopoDS::Edge(edges(i)));
      ++added;
    }
  }
  CHECK(added == (c.both ? 2 : 1));
  try {
    OCC_CATCH_SIGNALS
    fillet.Build();
  } catch (const Standard_Failure& e) {
    std::printf("fillet threw: %s\n", e.what());
  }
  std::printf("step %s, %s, %s:", c.above ? "above" : "below", c.both ? "both rims" : "one rim",
              c.mirror ? "mirrored" : "as built");
  CHECK(fillet.IsDone());
  if (!fillet.IsDone()) {
    std::printf(" not done\n");
    return;
  }
  const TopoDS_Shape rounded = fillet.Shape();
  CHECK(BRepCheck_Analyzer(rounded).IsValid());
  int tori = 0;
  for (TopExp_Explorer ex(rounded, TopAbs_FACE); ex.More(); ex.Next()) {
    GProp_GProps props;
    BRepGProp::SurfaceProperties(ex.Current(), props);
    CHECK(std::abs(props.Mass()) > 1e-9); // no face of no area
    if (BRepAdaptor_Surface(TopoDS::Face(ex.Current())).GetType() == GeomAbs_Torus) {
      ++tori;
    }
  }
  CHECK(tori == (c.both ? 2 : 1)); // each rounding's end its quarter torus
  const double loss = volume_of(body) - volume_of(rounded);
  const double want = (c.both ? 2 : 1) * rim_loss();
  std::printf(" material taken %.7f, %.7f wanted\n", loss, want);
  CHECK(std::abs(loss - want) < 2e-6);
}

void test_rounding_ends_at_a_step_back_of_its_wall() {
  for (const bool above : {false, true}) {
    for (const bool both : {false, true}) {
      for (const bool mirror : {false, true}) {
        check_case({above, both, mirror});
      }
    }
  }
}

} // namespace
} // namespace test

int main() {
  test::guarded("test_rounding_ends_at_a_step_back_of_its_wall",
                test::test_rounding_ends_at_a_step_back_of_its_wall);
  if (test::failures == 0) {
    std::puts("test_fillet_step_end: all checks passed");
  }
  return test::failures == 0 ? 0 : 1;
}
