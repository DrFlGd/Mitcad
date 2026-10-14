// SPDX-License-Identifier: MIT
// Chamfers wider than a face beside them (mitcad#240, mitcad#263), with the
// OCCT port's patches 0140 and later (third_party/vcpkg-ports/opencascade),
// on synthetic shapes and OCCT's chamfer alone. The cross-sections are
// exact, so are the volumes.

#include <cmath>
#include <cstdio>
#include <functional>
#include <utility>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom_Curve.hxx>
#include <Geom_Surface.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_List.hxx>
#include <Standard_ErrorHandler.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp_Ax2.hxx>
#include <gp_Circ.hxx>
#include <gp_Vec.hxx>

#include "check.hpp"
#include "mitcad/analysis/properties.hpp"

namespace test {
namespace {

double volume_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props, 1.0e-9);
  return props.Mass();
}

// The volume by the analysis library's integrals, which follow the spans
// of the faces' curves (OCCT's adaptive rule leaves 1e-6 on a chamfer's
// cone in one face of eleven edges).
double exact_volume_of(const TopoDS_Shape& shape) {
  return mitcad::analysis::physical_properties(shape).volume;
}

// Whether every edge's p-curve on each of its faces lies on the edge's 3d
// curve (sampled): the checker passes wires whose p-curves on a seam's other
// side were taken in the wrong period by luck of their order.
bool pcurves_fit(const TopoDS_Shape& shape) {
  for (TopExp_Explorer f(shape, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face& face = TopoDS::Face(f.Current());
    const occ::handle<Geom_Surface> surface = BRep_Tool::Surface(face);
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
      const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
      if (BRep_Tool::Degenerated(edge)) {
        continue;
      }
      double f1 = 0.0, l1 = 0.0, f2 = 0.0, l2 = 0.0;
      const occ::handle<Geom2d_Curve> pcurve = BRep_Tool::CurveOnSurface(edge, face, f1, l1);
      const occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, f2, l2);
      if (pcurve.IsNull() || curve.IsNull()) {
        return false;
      }
      for (int k = 0; k <= 4; ++k) {
        const double t = f1 + (l1 - f1) * k / 4.0;
        const gp_Pnt2d uv = pcurve->Value(t);
        if (surface->Value(uv.X(), uv.Y()).Distance(curve->Value(t)) > 1e-4 + BRep_Tool::Tolerance(edge)) {
          return false;
        }
      }
    }
  }
  return true;
}

int faces_of(const TopoDS_Shape& shape) {
  int n = 0;
  for (TopExp_Explorer e(shape, TopAbs_FACE); e.More(); e.Next()) {
    ++n;
  }
  return n;
}

TopoDS_Shape cut(const TopoDS_Shape& a, const TopoDS_Shape& b) {
  BRepAlgoAPI_Cut op(a, b);
  return op.Shape();
}

TopoDS_Shape fuse(const TopoDS_Shape& a, const TopoDS_Shape& b) {
  BRepAlgoAPI_Fuse op(a, b);
  op.SimplifyResult();
  return op.Shape();
}

// The edges of a shape all of whose sampled points satisfy `on`.
std::vector<TopoDS_Edge> edges_where(const TopoDS_Shape& shape, const std::function<bool(const gp_Pnt&)>& on) {
  std::vector<TopoDS_Edge> found;
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges;
  TopExp::MapShapes(shape, TopAbs_EDGE, edges);
  for (int i = 1; i <= edges.Extent(); ++i) {
    const TopoDS_Edge& edge = TopoDS::Edge(edges(i));
    if (BRep_Tool::Degenerated(edge)) {
      continue;
    }
    BRepAdaptor_Curve curve(edge);
    bool all = true;
    for (int k = 0; k <= 8 && all; ++k) {
      all = on(curve.Value(curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * k / 8.0));
    }
    if (all) {
      found.push_back(edge);
    }
  }
  return found;
}

// OCCT's chamfer of the edges with one distance: the result, or a null
// shape when it fails.
TopoDS_Shape chamfered(const TopoDS_Shape& body, const std::vector<TopoDS_Edge>& edges, double distance) {
  BRepFilletAPI_MakeChamfer maker(body);
  for (const TopoDS_Edge& edge : edges) {
    if (maker.Contour(edge) == 0) {
      maker.Add(distance, edge);
    }
  }
  try {
    OCC_CATCH_SIGNALS
    maker.Build();
  } catch (const Standard_Failure& failure) {
    std::fprintf(stderr, "chamfer overflow test: %s\n", failure.what());
    return TopoDS_Shape();
  }
  return maker.IsDone() ? maker.Shape() : TopoDS_Shape();
}

// A block with a hole of radius 1 from its top down to a step 1.5 below,
// and under the step a counterbore of radius 2 from the bottom: the step is
// a ring 1 wide facing down. Its inner rim is a whole circle or, `split`,
// two arcs (a vertex at 188.6 degrees); the counterbore's seam lies at the
// rim's vertex (180 degrees) or, `turned`, at 90 degrees.
TopoDS_Shape step_ring(bool split, bool turned) {
  const gp_Dir up(0, 0, 1);
  TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(-10, -10, -6), gp_Pnt(10, 10, 0)).Shape();
  block = cut(block, BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -2), up, gp_Dir(-1, 0, 0)), 1.0, 3).Shape());
  block = cut(block,
              BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -7), up, turned ? gp_Dir(0, 1, 0) : gp_Dir(-1, 0, 0)), 2.0,
                                       5.5)
                  .Shape());
  if (split) {
    const double a = 188.6 * kPi / 180.0;
    BRepAlgoAPI_Splitter splitter;
    NCollection_List<TopoDS_Shape> arguments;
    NCollection_List<TopoDS_Shape> tools;
    arguments.Append(block);
    tools.Append(BRepBuilderAPI_MakeVertex(gp_Pnt(std::cos(a), std::sin(a), -1.5)).Vertex());
    splitter.SetArguments(arguments);
    splitter.SetTools(tools);
    splitter.Build();
    block = TopExp_Explorer(splitter.Shape(), TopAbs_SOLID).Current();
  }
  return block;
}

// The step's inner rim chamfered by 1, the ring's width (patch 0140): the
// chamfer's cone runs from the hole's wall 1 above the step to the
// counterbore's wall, the ring vanishes. The triangle taken away (legs 1)
// turns about the axis at radius 4/3: 4 pi / 3. Before, the contact along
// the ring's outer circle was hatched in part only and the rest walked
// along the boundary ("Path failed"), or, for a whole circle, the chamfer
// was valid but kept the ring as a face of zero width. With the
// counterbore's seam turned away from the rim's vertex (`turned`), the
// contact along the ring's outer circle passes that circle's vertex, on the
// seam, inside the stripe (patch 0144): its edge was split there with one
// p-curve on the counterbore's wall for both parts, the part past the seam a
// period off, and the parts' p-curves on the chamfer's cone were shifted by
// the vertex's parameter (an unorientable face for the whole circle; for the
// rim in two arcs a valid shape whose p-curves on the cone missed their
// edges by 2.8). Now the stripe is split there in two pieces of one face.
// The fillet of radius 1 likewise: it takes the square of the ring's width
// less the quarter disc about the counterbore's rim, turned about the axis:
// 2 pi (11 / 6 - pi / 2).
void test_chamfer_as_wide_as_a_step_ring() {
  for (const bool fillet : {false, true}) {
    for (const bool split : {false, true}) {
      for (const bool turned : {false, true}) {
        const TopoDS_Shape body = step_ring(split, turned);
        const std::vector<TopoDS_Edge> rim = edges_where(body, [](const gp_Pnt& p) {
          return std::abs(p.Z() + 1.5) < 1e-6 && std::abs(std::hypot(p.X(), p.Y()) - 1.0) < 1e-6;
        });
        CHECK(rim.size() == (split ? 2u : 1u));
        TopoDS_Shape result;
        if (fillet) {
          BRepFilletAPI_MakeFillet maker(body);
          for (const TopoDS_Edge& edge : rim) {
            if (maker.Contour(edge) == 0) {
              maker.Add(1.0, edge);
            }
          }
          try {
            OCC_CATCH_SIGNALS
            maker.Build();
          } catch (const Standard_Failure& failure) {
            std::fprintf(stderr, "chamfer overflow test: %s\n", failure.what());
          }
          if (maker.IsDone()) {
            result = maker.Shape();
          }
        } else {
          result = chamfered(body, rim, 1.0);
        }
        CHECK(!result.IsNull());
        if (result.IsNull()) {
          continue;
        }
        CHECK(BRepCheck_Analyzer(result).IsValid());
        CHECK(pcurves_fit(result));
        const double change = volume_of(result) - volume_of(body);
        CHECK_NEAR(change, fillet ? -2.0 * kPi * (11.0 / 6.0 - kPi / 2.0) : -4.0 * kPi / 3.0);
        // The ring gone, the blend in its place (in two pieces on two arcs).
        CHECK(faces_of(result) == faces_of(body) + (split ? 1 : 0));
      }
    }
  }
}

// A block with a bore of radius 17 and inside it a lug: a ring sector
// between radii 12.5 and 17 over 135 degrees from `start` degrees, 5 high,
// joined to the bore's wall, whose seam lies at `seam` degrees. The concave
// arc where the lug's top meets the wall above it is chamfered by 4.5, as
// wide as the lug's top (patch 0140): the hatching of the circular contact
// on the top's inner arc gave a domain without a first point, and the
// contact was walked. The chamfer adds the triangle (legs 4.5, centroid at
// radius 15.5) turned through 135 degrees; the lug's radial sides cut it.
// With the lug's end on the bore's seam (patch 0147), the end's corner
// prolonged the edge between the lug's end and the wall up to the chamfer
// with the edge's p-curve on the wall's other side (an edge left from the
// seam keeps both), and the seam above the lug was not cut: a
// self-intersecting wire. The fillet of radius 4.5 likewise: it adds the
// square of the lug's width less the quarter disc about the fillet's axis,
// turned through 135 degrees.
void test_chamfer_as_wide_as_a_lug_in_a_bore(double start, double seam, bool fillet) {
  const gp_Dir up(0, 0, 1);
  const double a0 = start * kPi / 180.0, s0 = seam * kPi / 180.0;
  const gp_Dir from(std::cos(a0), std::sin(a0), 0);
  TopoDS_Shape block = BRepPrimAPI_MakeBox(gp_Pnt(-30, -30, -16), gp_Pnt(30, 30, 18)).Shape();
  block = cut(block, BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -17), up, gp_Dir(std::cos(s0), std::sin(s0), 0)),
                                              17.0, 36)
                         .Shape());
  TopoDS_Shape lug = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -6), up, from), 17.0, 5.0, 0.75 * kPi).Shape();
  lug = cut(lug, BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -7), up, from), 12.5, 7.0).Shape());
  const TopoDS_Shape body = fuse(block, lug);
  CHECK(BRepCheck_Analyzer(body).IsValid());
  const std::vector<TopoDS_Edge> arc = edges_where(body, [](const gp_Pnt& p) {
    return std::abs(p.Z() + 1) < 1e-6 && std::abs(std::hypot(p.X(), p.Y()) - 17.0) < 1e-6;
  });
  CHECK(arc.size() == 1u);
  TopoDS_Shape result;
  if (fillet) {
    BRepFilletAPI_MakeFillet maker(body);
    for (const TopoDS_Edge& edge : arc) {
      maker.Add(4.5, edge);
    }
    try {
      OCC_CATCH_SIGNALS
      maker.Build();
    } catch (const Standard_Failure& failure) {
      std::fprintf(stderr, "chamfer overflow test: %s\n", failure.what());
    }
    if (maker.IsDone()) {
      result = maker.Shape();
    }
  } else {
    result = chamfered(body, arc, 4.5);
  }
  CHECK(!result.IsNull());
  if (result.IsNull()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(result).IsValid());
  CHECK(pcurves_fit(result));
  const double r = 4.5;
  const double added = fillet ? 0.75 * kPi * (r * 66.375 - 12.5 * kPi * r * r / 4.0 - r * r * r / 3.0)
                              : 0.75 * kPi * 15.5 * 10.125;
  CHECK_NEAR(volume_of(result) - volume_of(body), added);
  // The lug's top gone, the blend in its place.
  CHECK(faces_of(result) == faces_of(body));
}

// A wall (a block's face x = 95) with a rib on it: a pentagonal prism over
// (95,85) (94,85) (92,84) (94,83) (95,83) from z = 5 to 50. The rib's
// bottom meets the wall below in a concave edge, chamfered by 2.99: on the
// bottom the contact (x = 92.01) lies inside it only within 0.005 of the
// tip, elsewhere it runs off across the bottom's slanted edges, and the
// rib's sides, prolonged down, cut the chamfer; at the ends of the edge the
// rib's flat sides cut it, the edges between the sides prolonged down to
// it. With the rib's bottom as the chamfer's first face (`bottom_first`:
// the shell's faces in another order), the end's corner prolonged the
// wall's edge on the end face's bounded surface, where the contact's point
// beyond that edge's end was not found (patch 0141).
void test_chamfer_of_a_rib_cut_by_its_sides(bool bottom_first) {
  TopoDS_Shape wall = BRepPrimAPI_MakeBox(gp_Pnt(95, 70, 0), gp_Pnt(105, 100, 60)).Shape();
  BRepBuilderAPI_MakePolygon outline;
  outline.Add(gp_Pnt(95, 85, 5));
  outline.Add(gp_Pnt(94, 85, 5));
  outline.Add(gp_Pnt(92, 84, 5));
  outline.Add(gp_Pnt(94, 83, 5));
  outline.Add(gp_Pnt(95, 83, 5));
  outline.Close();
  const TopoDS_Shape rib =
      BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(outline.Wire()).Face(), gp_Vec(0, 0, 45)).Shape();
  TopoDS_Shape body = fuse(wall, rib);
  if (bottom_first) {
    BRep_Builder builder;
    TopoDS_Shell shell;
    builder.MakeShell(shell);
    std::vector<TopoDS_Shape> rest;
    for (TopExp_Explorer f(body, TopAbs_FACE); f.More(); f.Next()) {
      BRepAdaptor_Surface surface(TopoDS::Face(f.Current()));
      if (surface.GetType() == GeomAbs_Plane && std::abs(surface.Plane().Location().Z() - 5) < 1e-9 &&
          std::abs(surface.Plane().Axis().Direction().Z()) > 0.5) {
        builder.Add(shell, f.Current());
      } else {
        rest.push_back(f.Current());
      }
    }
    for (const TopoDS_Shape& face : rest) {
      builder.Add(shell, face);
    }
    TopoDS_Solid solid;
    builder.MakeSolid(solid);
    builder.Add(solid, shell);
    body = solid;
  }
  const std::vector<TopoDS_Edge> foot = edges_where(body, [](const gp_Pnt& p) {
    return std::abs(p.Z() - 5) < 1e-6 && std::abs(p.X() - 95) < 1e-6 && p.Y() > 82.9 && p.Y() < 85.1;
  });
  CHECK(foot.size() == 1u);
  const TopoDS_Shape result = chamfered(body, foot, 2.99);
  CHECK(!result.IsNull());
  if (result.IsNull()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(result).IsValid());
  // At depth s below the rib's bottom the chamfer fills the rib's section
  // beyond x = 92.01 + s: the triangle's part (tip at x = 92) while that
  // lies before x = 94, then the rectangle's.
  const double e = 0.01, s1 = 1.99;
  const double added = 4.0 * s1 - (std::pow(e + s1, 3) - std::pow(e, 3)) / 6.0 + 1.0;
  CHECK_NEAR(volume_of(result) - volume_of(body), added);
  // The chamfer's face; the bottom's sliver at the tip (5e-5) stays.
  CHECK(faces_of(result) == faces_of(body) + 1);
}

// A plate 17 thick (z 0..17) whose corner is rounded with radius 20 about
// (81.5, 20), and under it a boss of radius 7.5 on the same axis (z from
// -13, or -14, to 0). The boss's foot is chamfered by 13: on the plate's
// underside the contact (radius 20.5) runs 0.5 beyond the plate's outline
// over a quarter turn and more (patch 0142). The chamfer adds material: the
// plate's sides there, prolonged down, cut it, the edges between them
// prolonged down to it, and the underside's edges under the chamfer go.
// Before, the chain of faces beyond was not found (the blend crosses the
// edges between them below their ends), and the gap was left to the
// walking, which chamfers do not do. `exact`: the boss as long as the
// chamfer (it vanishes, 0140); `away`: the boss's seam (and the closed
// spine's start) away from the gap, else in it. With the seam in the gap
// the pieces past the spine's start held their p-curves on the boss a
// period on, beyond its seam (patch 0145: an unorientable face for the
// longer boss); and the cone came in a face per piece where the piece
// across the spine's start holds its parameters a period on, or the run of
// pieces cut by the faces beyond goes on across the start (patch 0146), the
// cone being one face in the file's body.
void test_chamfer_of_a_boss_beyond_a_plate_corner(bool exact, bool away) {
  TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(61.5, 0, 0), gp_Pnt(200, 150, 17)).Shape();
  {
    BRepFilletAPI_MakeFillet round(plate);
    for (const TopoDS_Edge& e : edges_where(plate, [](const gp_Pnt& p) {
           return std::abs(p.X() - 61.5) < 1e-6 && std::abs(p.Y()) < 1e-6;
         })) {
      round.Add(20.0, e);
    }
    plate = round.Shape();
  }
  const double length = exact ? 13.0 : 14.0;
  const TopoDS_Shape boss =
      BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(81.5, 20, -length), gp_Dir(0, 0, 1), away ? gp_Dir(1, 1, 0) : gp_Dir(-1, 0, 0)),
                               7.5, length)
          .Shape();
  const TopoDS_Shape body = fuse(plate, boss);
  const std::vector<TopoDS_Edge> foot = edges_where(body, [](const gp_Pnt& p) {
    return std::abs(p.Z()) < 1e-6 && std::abs(std::hypot(p.X() - 81.5, p.Y() - 20) - 7.5) < 1e-6;
  });
  CHECK(foot.size() == 1u);
  const TopoDS_Shape result = chamfered(body, foot, 13.0);
  CHECK(!result.IsNull());
  if (result.IsNull()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(result).IsValid());
  // The chamfer's ring (legs 13 from radius 7.5) less what lies beyond the
  // outline: in the rounded corner's quarter beyond radius 20 (exact), and
  // beyond each side half a circular segment of the ring's radius R
  // (20 to 20.5 over the top 0.5), integrated with Simpson's rule.
  const double ring = 2.0 * kPi * (7.5 + 13.0 / 3.0) * 84.5;
  const double corner = 0.5 * kPi * ((std::pow(20.5, 3) / 6.0 - 200.0 * 20.5) - (8000.0 / 6.0 - 4000.0));
  double sides = 0.0;
  const int n = 2000;
  for (int i = 0; i <= n; ++i) {
    const double r = 20.0 + 0.5 * i / n;
    const double segment = r * r * std::acos(20.0 / r) - 20.0 * std::sqrt(std::max(0.0, r * r - 400.0));
    sides += (i == 0 || i == n ? 1.0 : (i % 2 == 1 ? 4.0 : 2.0)) * segment;
  }
  sides *= 0.5 / n / 3.0;
  CHECK(pcurves_fit(result));
  CHECK_NEAR_TOL(exact_volume_of(result) - exact_volume_of(body), ring - corner - sides, 1e-8);
  // The cone one face; the boss's side gone with the exact length.
  CHECK(faces_of(result) == faces_of(body) + (exact ? 0 : 1));
}

// A hexagonal prism (inradius 4, z 0..4) under a disc of radius 8.5 (z
// 4..6); the edges where the prism's walls meet the disc's underside are
// chamfered 4 on the underside and 3 on the walls (patch 0143). At the
// hexagon's corners the two chamfers' contacts on the underside cross 9.24
// from the axis, beyond the disc's rim: the disc's cylinder, prolonged
// down, cuts the chamfers there, and their intersection runs from the
// prism's edge to the cylinder (at height 3.52). Before, the corner by
// intersection on the common face found no crossing and failed.
// `seam_at_corner`: the cylinder's seam on the hexagon's corner at 180
// degrees (the corner's point on the seam's prolongation), else at 30.
void test_chamfers_crossing_beyond_their_common_face(bool seam_at_corner) {
  BRepBuilderAPI_MakePolygon outline;
  const double r = 8.0 / std::sqrt(3.0);
  for (int k = 0; k < 6; ++k) {
    outline.Add(gp_Pnt(r * std::cos(k * kPi / 3.0), r * std::sin(k * kPi / 3.0), 0));
  }
  outline.Close();
  const TopoDS_Shape prism =
      BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(outline.Wire()).Face(), gp_Vec(0, 0, 4)).Shape();
  const gp_Dir seam = seam_at_corner ? gp_Dir(-1, 0, 0) : gp_Dir(std::sqrt(3.0), 1, 0);
  const TopoDS_Shape disc = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 4), gp_Dir(0, 0, 1), seam), 8.5, 2).Shape();
  const TopoDS_Shape body = fuse(prism, disc);
  const std::vector<TopoDS_Edge> feet = edges_where(body, [](const gp_Pnt& p) {
    return std::abs(p.Z() - 4) < 1e-6 && std::hypot(p.X(), p.Y()) < 4.62;
  });
  CHECK(feet.size() == 6u);
  TopoDS_Face underside;
  for (TopExp_Explorer f(body, TopAbs_FACE); f.More(); f.Next()) {
    BRepAdaptor_Surface surface(TopoDS::Face(f.Current()));
    if (surface.GetType() == GeomAbs_Plane && std::abs(surface.Plane().Location().Z() - 4) < 1e-9 &&
        std::abs(surface.Plane().Axis().Direction().Z()) > 0.5) {
      underside = TopoDS::Face(f.Current());
    }
  }
  BRepFilletAPI_MakeChamfer maker(body);
  for (const TopoDS_Edge& edge : feet) {
    maker.Add(4.0, 3.0, edge, underside);
  }
  try {
    OCC_CATCH_SIGNALS
    maker.Build();
  } catch (const Standard_Failure& failure) {
    std::fprintf(stderr, "chamfer overflow test: %s\n", failure.what());
  }
  CHECK(maker.IsDone());
  if (!maker.IsDone()) {
    return;
  }
  const TopoDS_Shape result = maker.Shape();
  CHECK(BRepCheck_Analyzer(result).IsValid());
  // At height z (1 to 4) the chamfers fill the hexagon of inradius 4 + s,
  // s = (z - 1) 4 / 3, within the disc's cylinder, less the prism's: the
  // hexagon of inradius a within radius R is twelve times the triangle up
  // to where its side leaves the circle and the sector beyond.
  const auto hexagon_in_disc = [](double a, double R) {
    const double t = std::acos(std::min(1.0, a / R));
    if (t >= kPi / 6.0) {
      return 2.0 * std::sqrt(3.0) * a * a;
    }
    return 12.0 * (0.5 * a * a * std::tan(t) + 0.5 * R * R * (kPi / 6.0 - t));
  };
  const int n = 20000;
  double added = 0.0;
  for (int i = 0; i <= n; ++i) {
    const double s = 4.0 * i / n;
    const double area = hexagon_in_disc(4.0 + s, 8.5) - 2.0 * std::sqrt(3.0) * 16.0;
    added += (i == 0 || i == n ? 1.0 : (i % 2 == 1 ? 4.0 : 2.0)) * area;
  }
  added *= 3.0 / n / 3.0;
  CHECK_NEAR_TOL(volume_of(result) - volume_of(body), added, 1e-8);
  // Six chamfers, the underside in six pieces between them and the rim.
  CHECK(faces_of(result) == faces_of(body) + 11);
}

// A floor (z 0..3) with a ledge `ledge_height` high and on it a wall 20
// high; the wall's outline is a line 30 long along y = 37.5 and a quarter
// arc of radius 20 about (57.5, 57.5) meeting it tangentially, the ledge's
// the same 2 further out. The wall's foot on the ledge (the line and the
// arc) is chamfered by 2, as wide as the ledge (patch 0148): the ledge's top
// vanishes and the chamfer's contact runs along its outer edges. Before,
// the arc's contact, running on into the ledge's top where the line's
// begins, was hatched to a point; the ledge's top, along both edges, was
// not taken as consumed; and the line's and the arc's pieces ended at two
// distinct vertices where they meet: the chamfer failed, or its wires
// intersected. It adds the triangle (legs 2) along the line and, about the
// arc's axis, at radius 20 + 2 / 3 over a quarter turn.
void test_chamfer_as_wide_as_a_ledge_along_a_line_and_an_arc(double ledge_height) {
  const auto outline = [](double offset, double z, double height) {
    const gp_Pnt a(87.5, 37.5 - offset, z), b(57.5, 37.5 - offset, z), c(37.5 - offset, 57.5, z), e(87.5, 57.5, z);
    BRepBuilderAPI_MakeWire wire;
    wire.Add(BRepBuilderAPI_MakeEdge(a, b).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(gp_Circ(gp_Ax2(gp_Pnt(57.5, 57.5, z), gp_Dir(0, 0, 1)), 20.0 + offset), c, b)
                 .Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(c, e).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(e, a).Edge());
    return BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(wire.Wire()).Face(), gp_Vec(0, 0, height)).Shape();
  };
  const TopoDS_Shape floor = BRepPrimAPI_MakeBox(gp_Pnt(20, 20, 0), gp_Pnt(100, 70, 3)).Shape();
  const TopoDS_Shape body =
      fuse(fuse(floor, outline(2.0, 3.0, ledge_height)), outline(0.0, 3.0 + ledge_height, 20.0));
  const double z = 3.0 + ledge_height;
  const std::vector<TopoDS_Edge> foot = edges_where(body, [z](const gp_Pnt& p) {
    return std::abs(p.Z() - z) < 1e-6 && ((std::abs(p.Y() - 37.5) < 1e-6 && p.X() > 57.4) ||
                                          std::abs(std::hypot(p.X() - 57.5, p.Y() - 57.5) - 20.0) < 1e-6);
  });
  CHECK(foot.size() == 2u);
  const TopoDS_Shape result = chamfered(body, foot, 2.0);
  CHECK(!result.IsNull());
  if (result.IsNull()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(result).IsValid());
  CHECK(pcurves_fit(result));
  const double d = 2.0;
  CHECK_NEAR_TOL(exact_volume_of(result) - exact_volume_of(body),
                 d * d / 2.0 * 30.0 + d * d / 2.0 * kPi / 2.0 * (20.0 + d / 3.0), 1e-8);
  // The ledge's top gone, the chamfer's plane and cone in its place.
  CHECK(faces_of(result) == faces_of(body) + 1);
}

// A plate (z 0..10) with under it a boss of radius 12.5, 3 long, its seam
// at `seam` degrees; the boss's free end's edge is chamfered by 3, as long
// as the boss (or by distance 3 and 45 degrees on the end, `by_angle`): the
// boss's wall vanishes and the chamfer's contact runs along the closed edge
// where the boss meets the plate (patch 0149). Before, the contact's end
// snapped onto that edge's vertex was projected onto the line's other end
// (the vertex's parameters a period away on the boss's wall), and the
// chamfer failed. It takes the triangle (legs 3) turned about the axis at
// radius 12.5 - 1.
void test_chamfer_as_long_as_a_boss(double seam, bool by_angle) {
  const double s0 = seam * kPi / 180.0;
  const TopoDS_Shape plate = BRepPrimAPI_MakeBox(gp_Pnt(-30, -30, 0), gp_Pnt(30, 30, 10)).Shape();
  const TopoDS_Shape body =
      fuse(plate, BRepPrimAPI_MakeCylinder(
                      gp_Ax2(gp_Pnt(0, 0, -3), gp_Dir(0, 0, 1), gp_Dir(std::cos(s0), std::sin(s0), 0)), 12.5, 3)
                      .Shape());
  const std::vector<TopoDS_Edge> rim = edges_where(body, [](const gp_Pnt& p) {
    return std::abs(p.Z() + 3) < 1e-6 && std::abs(std::hypot(p.X(), p.Y()) - 12.5) < 1e-6;
  });
  CHECK(rim.size() == 1u);
  if (rim.size() != 1u) {
    return;
  }
  TopoDS_Shape result;
  if (by_angle) {
    TopoDS_Face end;
    for (TopExp_Explorer f(body, TopAbs_FACE); f.More(); f.Next()) {
      BRepAdaptor_Surface surface(TopoDS::Face(f.Current()));
      if (surface.GetType() == GeomAbs_Plane && std::abs(surface.Plane().Location().Z() + 3) < 1e-9) {
        end = TopoDS::Face(f.Current());
      }
    }
    BRepFilletAPI_MakeChamfer maker(body);
    maker.AddDA(3.0, kPi / 4.0, rim[0], end);
    try {
      OCC_CATCH_SIGNALS
      maker.Build();
    } catch (const Standard_Failure& failure) {
      std::fprintf(stderr, "chamfer overflow test: %s\n", failure.what());
    }
    if (maker.IsDone()) {
      result = maker.Shape();
    }
  } else {
    result = chamfered(body, rim, 3.0);
  }
  CHECK(!result.IsNull());
  if (result.IsNull()) {
    return;
  }
  CHECK(BRepCheck_Analyzer(result).IsValid());
  CHECK(pcurves_fit(result));
  CHECK_NEAR_TOL(exact_volume_of(result) - exact_volume_of(body), -2.0 * kPi * 11.5 * 4.5, 1e-8);
  // The boss's wall gone, the chamfer's cone in its place.
  CHECK(faces_of(result) == faces_of(body));
}

} // namespace

void chamfer_overflow_tests() {
  guarded("test_chamfer_as_wide_as_a_step_ring", test_chamfer_as_wide_as_a_step_ring);
  // (The lug's ends away from the bore's seam, its end and its start on it.)
  for (const bool fillet : {false, true}) {
    for (const auto& [start, seam] : {std::pair<double, double>{112.5, 0.0}, {225.0, 0.0}, {0.0, 0.0}}) {
      guarded("test_chamfer_as_wide_as_a_lug_in_a_bore", [&, start = start, seam = seam] {
        test_chamfer_as_wide_as_a_lug_in_a_bore(start, seam, fillet);
      });
    }
  }
  for (const bool bottom_first : {false, true}) {
    guarded("test_chamfer_of_a_rib_cut_by_its_sides", [&] { test_chamfer_of_a_rib_cut_by_its_sides(bottom_first); });
  }
  for (const bool exact : {true, false}) {
    for (const bool away : {true, false}) {
      guarded("test_chamfer_of_a_boss_beyond_a_plate_corner",
              [&] { test_chamfer_of_a_boss_beyond_a_plate_corner(exact, away); });
    }
  }
  for (const bool seam_at_corner : {false, true}) {
    guarded("test_chamfers_crossing_beyond_their_common_face",
            [&] { test_chamfers_crossing_beyond_their_common_face(seam_at_corner); });
  }
  for (const double ledge_height : {0.5, 1.0}) {
    guarded("test_chamfer_as_wide_as_a_ledge_along_a_line_and_an_arc",
            [&] { test_chamfer_as_wide_as_a_ledge_along_a_line_and_an_arc(ledge_height); });
  }
  for (const double seam : {180.0, 90.0}) {
    for (const bool by_angle : {false, true}) {
      guarded("test_chamfer_as_long_as_a_boss", [&] { test_chamfer_as_long_as_a_boss(seam, by_angle); });
    }
  }
}

} // namespace test
