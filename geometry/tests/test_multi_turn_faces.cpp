// SPDX-License-Identifier: MIT
// Booleans on stored bodies whose faces run many turns: a threaded hole's
// minor and major cylinders kept as one face each, their helix edges'
// pcurves spanning tens of periods, and the thread flanks one ruled
// B-spline face each (mitcad#349). The port's patches 0510 to 0512 and 0560 let
// OCCT's booleans cut them.

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <functional>
#include <string>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <NCollection_IndexedMap.hxx>
#include <TopExp.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <GeomAbs_SurfaceType.hxx>
#include <IntTools_CommonPrt.hxx>
#include <IntTools_EdgeFace.hxx>
#include <IntTools_Range.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Ax2.hxx>
#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>

#include "check.hpp"
#include "mitcad/geometry/brep_import.hpp"
#include "mitcad/geometry/query.hpp"

namespace {

using test::kPi;
namespace brep = mitcad::brep;

double volume(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props);
  return props.Mass();
}

int count(const TopoDS_Shape& shape, TopAbs_ShapeEnum type) {
  int n = 0;
  for (TopExp_Explorer ex(shape, type); ex.More(); ex.Next()) {
    ++n;
  }
  return n;
}

// A square thread in a hole along z through a block, as a stored body
// keeps it: the hole's minor cylinder (radius r1) between the thread's
// turns and its major cylinder (r2) in the groove are one face each, as are
// the two flanks (radial helicoids z = k theta and z = k theta + w, ruled
// surfaces between helices at r1 and r2). The helices are interpolated at
// 64 points a turn; their parameter is the angle, so an edge's range spans
// `turns` periods of the cylinders.
struct ThreadedBlock {
  double r1 = 3.4;
  double r2 = 4.0;
  double pitch = 1.25;
  double groove = 0.6;
  int turns = 32;
  double half = 8.0; // half the block's width

  double height() const { return pitch * turns; }

  // The block's volume: the square prism less the core and the groove,
  // which the helicoids sweep uniformly (groove / pitch of the ring).
  double expected_volume() const {
    const double h = height();
    return 4.0 * half * half * h - kPi * r1 * r1 * h - kPi * (r2 * r2 - r1 * r1) * h * groove / pitch;
  }

  // Whether a point inside the block's square lies in its material: outside
  // the major cylinder, or between the cylinders above the upper flank and
  // below the next turn's lower flank.
  bool material(const gp_Pnt& p) const {
    const double r = std::hypot(p.X(), p.Y());
    if (r < r1) {
      return false;
    }
    if (r > r2) {
      return true;
    }
    const double k = pitch / (2.0 * kPi);
    const double along = p.Z() - k * std::atan2(p.Y(), p.X());
    const double phase = along - pitch * std::floor(along / pitch);
    return phase > groove;
  }

  // The volume of the material in a box where `inside` holds, sampled at
  // the centres of 60^3 cells.
  double material_volume(const gp_Pnt& low, const gp_Pnt& high,
                         const std::function<bool(const gp_Pnt&)>& inside) const {
    constexpr int n = 60;
    const gp_Vec size(low, high);
    int in = 0;
    for (int i = 0; i < n; ++i) {
      for (int j = 0; j < n; ++j) {
        for (int l = 0; l < n; ++l) {
          const gp_Pnt p(low.X() + size.X() * (i + 0.5) / n, low.Y() + size.Y() * (j + 0.5) / n,
                         low.Z() + size.Z() * (l + 0.5) / n);
          in += material(p) && inside(p) ? 1 : 0;
        }
      }
    }
    return size.X() * size.Y() * size.Z() * in / (static_cast<double>(n) * n * n);
  }

  brep::Body body() const {
    brep::Body b;
    const double k = pitch / (2.0 * kPi);
    const double h = height();
    const double tb = -2.0 * kPi * groove / pitch; // where the upper flank meets z = 0
    const double span = 2.0 * kPi * turns;
    const auto geometry = [&](int kind, const std::vector<std::int32_t>& ints, const std::vector<double>& reals) {
      brep::Geometry g;
      g.kind = kind;
      g.int_offset = static_cast<std::uint32_t>(b.ints.size());
      g.int_count = static_cast<std::uint32_t>(ints.size());
      g.real_offset = static_cast<std::uint32_t>(b.reals.size());
      g.real_count = static_cast<std::uint32_t>(reals.size());
      b.ints.insert(b.ints.end(), ints.begin(), ints.end());
      b.reals.insert(b.reals.end(), reals.begin(), reals.end());
      return g;
    };
    // An interpolated helix's ints and reals: radius r, z = k t + dz for t
    // in [t0, t0 + span].
    const int samples = 64 * turns;
    const auto helix = [&](double r, double t0, double dz, std::vector<std::int32_t>& ints,
                           std::vector<double>& reals) {
      ints.push_back(samples + 1);
      std::vector<double> points;
      for (int i = 0; i <= samples; ++i) {
        const double t = t0 + span * i / samples;
        reals.push_back(t);
        points.insert(points.end(), {r * std::cos(t), r * std::sin(t), k * t + dz});
      }
      reals.insert(reals.end(), points.begin(), points.end());
    };
    const auto add_helix = [&](double r, double t0, double dz) {
      std::vector<std::int32_t> ints;
      std::vector<double> reals;
      helix(r, t0, dz, ints, reals);
      b.curves.push_back(geometry(3, ints, reals));
      return static_cast<std::uint32_t>(b.curves.size() - 1);
    };
    const auto add_circle = [&](double r, double z) {
      b.curves.push_back(geometry(1, {}, {0, 0, z, 0, 0, 1, 1, 0, 0, r, r}));
      return static_cast<std::uint32_t>(b.curves.size() - 1);
    };
    const auto add_line = [&](gp_Pnt from, gp_Pnt to) {
      const double l = from.Distance(to);
      b.curves.push_back(geometry(0, {},
                                  {from.X(), from.Y(), from.Z(), (to.X() - from.X()) / l, (to.Y() - from.Y()) / l,
                                   (to.Z() - from.Z()) / l}));
      return static_cast<std::uint32_t>(b.curves.size() - 1);
    };
    const auto add_vertex = [&](gp_Pnt p) {
      b.vertices.insert(b.vertices.end(), {p.X(), p.Y(), p.Z(), 1e-5});
      return static_cast<std::uint32_t>(b.vertices.size() / 4 - 1);
    };
    const auto add_edge = [&](std::uint32_t curve, std::uint32_t v0, std::uint32_t v1, double t0, double t1) {
      b.edges.push_back({curve, v0, v1, t0, t1, 1e-5});
      return static_cast<std::uint32_t>(b.edges.size() - 1);
    };
    const auto at = [](double r, double t, double z) { return gp_Pnt(r * std::cos(t), r * std::sin(t), z); };
    const auto line_edge = [&](std::uint32_t v0, std::uint32_t v1) {
      const gp_Pnt p0(b.vertices[4 * v0], b.vertices[4 * v0 + 1], b.vertices[4 * v0 + 2]);
      const gp_Pnt p1(b.vertices[4 * v1], b.vertices[4 * v1 + 1], b.vertices[4 * v1 + 2]);
      return add_edge(add_line(p0, p1), v0, v1, 0.0, p0.Distance(p1));
    };

    // Vertices where the flanks meet the end faces.
    const std::uint32_t a1s = add_vertex(at(r1, 0, 0));
    const std::uint32_t a2s = add_vertex(at(r2, 0, 0));
    const std::uint32_t b1s = add_vertex(at(r1, tb, 0));
    const std::uint32_t b2s = add_vertex(at(r2, tb, 0));
    const std::uint32_t a1e = add_vertex(at(r1, span, h));
    const std::uint32_t a2e = add_vertex(at(r2, span, h));
    const std::uint32_t b1e = add_vertex(at(r1, tb + span, h));
    const std::uint32_t b2e = add_vertex(at(r2, tb + span, h));
    // The helices: the lower flank's (A) and the upper flank's (B).
    const std::uint32_t ca1 = add_helix(r1, 0, 0);
    const std::uint32_t ca2 = add_helix(r2, 0, 0);
    const std::uint32_t cb1 = add_helix(r1, tb, groove);
    const std::uint32_t cb2 = add_helix(r2, tb, groove);
    const std::uint32_t a1 = add_edge(ca1, a1s, a1e, 0, span);
    const std::uint32_t a2 = add_edge(ca2, a2s, a2e, 0, span);
    const std::uint32_t b1 = add_edge(cb1, b1s, b1e, tb, tb + span);
    const std::uint32_t b2 = add_edge(cb2, b2s, b2e, tb, tb + span);
    // The hole's outline in the end faces: the minor arc, the flanks'
    // radial lines and the major arc.
    const double tb_turned = tb + 2.0 * kPi;
    const std::uint32_t bottom1 = add_edge(add_circle(r1, 0), a1s, b1s, 0, tb_turned);
    const std::uint32_t bottom2 = add_edge(add_circle(r2, 0), b2s, a2s, tb_turned, 2.0 * kPi);
    const std::uint32_t top1 = add_edge(add_circle(r1, h), a1e, b1e, 0, tb_turned);
    const std::uint32_t top2 = add_edge(add_circle(r2, h), b2e, a2e, tb_turned, 2.0 * kPi);
    const std::uint32_t ra0 = line_edge(a1s, a2s);
    const std::uint32_t rb0 = line_edge(b1s, b2s);
    const std::uint32_t ran = line_edge(a1e, a2e);
    const std::uint32_t rbn = line_edge(b1e, b2e);
    // The block.
    std::uint32_t corner[8];
    for (int i = 0; i < 8; ++i) {
      const double x = (i % 4 == 1 || i % 4 == 2) ? half : -half;
      const double y = (i % 4 >= 2) ? half : -half;
      corner[i] = add_vertex(gp_Pnt(x, y, i < 4 ? 0.0 : h));
    }
    std::uint32_t q[12];
    for (int i = 0; i < 4; ++i) {
      q[i] = line_edge(corner[i], corner[(i + 1) % 4]);
      q[4 + i] = line_edge(corner[4 + i], corner[4 + (i + 1) % 4]);
      q[8 + i] = line_edge(corner[i], corner[4 + i]);
    }

    // Surfaces.
    const auto add_surface = [&](int kind, const std::vector<std::int32_t>& ints, const std::vector<double>& reals) {
      b.surfaces.push_back(geometry(kind, ints, reals));
      return static_cast<std::uint32_t>(b.surfaces.size() - 1);
    };
    const auto add_ruled = [&](double t0, double dz) {
      std::vector<std::int32_t> ints{3};
      std::vector<double> reals;
      helix(r1, t0, dz, ints, reals);
      ints.push_back(3);
      helix(r2, t0, dz, ints, reals);
      return add_surface(7, ints, reals);
    };
    const std::uint32_t minor = add_surface(1, {}, {0, 0, 0, 0, 0, 1, 1, 0, 0, r1, 0});
    const std::uint32_t major = add_surface(1, {}, {0, 0, 0, 0, 0, 1, 1, 0, 0, r2, 0});
    const std::uint32_t flank_a = add_ruled(0, 0);
    const std::uint32_t flank_b = add_ruled(tb, groove);
    const std::uint32_t bottom = add_surface(0, {}, {0, 0, 0, 0, 0, 1, 1, 0, 0});
    const std::uint32_t top = add_surface(0, {}, {0, 0, h, 0, 0, 1, 1, 0, 0});
    const std::uint32_t side[4] = {
        add_surface(0, {}, {0, -half, 0, 0, -1, 0, 1, 0, 0}), add_surface(0, {}, {half, 0, 0, 1, 0, 0, 0, 1, 0}),
        add_surface(0, {}, {0, half, 0, 0, 1, 0, -1, 0, 0}), add_surface(0, {}, {-half, 0, 0, -1, 0, 0, 0, -1, 0})};

    // Faces: loops listed counterclockwise about the surface's own normal
    // (the uv plane); `reversed` faces get their coedges turned, as the
    // data gives them about the material's normal.
    const auto add_face = [&](std::uint32_t surface, bool reversed,
                              const std::vector<std::vector<brep::Coedge>>& loops) {
      brep::Face f;
      f.surface = surface;
      f.reversed = reversed;
      f.first_loop = static_cast<std::uint32_t>(b.loops.size());
      f.loop_count = static_cast<std::uint32_t>(loops.size());
      for (const auto& loop : loops) {
        b.loops.push_back({static_cast<std::uint32_t>(b.coedges.size()), static_cast<std::uint32_t>(loop.size())});
        for (brep::Coedge c : loop) {
          c.forward = c.forward != reversed;
          b.coedges.push_back(c);
        }
      }
      b.shell_faces.push_back(static_cast<std::uint32_t>(b.faces.size()));
      b.faces.push_back(f);
    };
    add_face(minor, true, {{{bottom1, true}, {b1, true}, {top1, false}, {a1, false}}});
    add_face(major, true, {{{bottom2, true}, {a2, true}, {top2, false}, {b2, false}}});
    add_face(flank_a, false, {{{ra0, true}, {a2, true}, {ran, false}, {a1, false}}});
    add_face(flank_b, true, {{{rb0, true}, {b2, true}, {rbn, false}, {b1, false}}});
    add_face(bottom, true,
             {{{q[0], true}, {q[1], true}, {q[2], true}, {q[3], true}},
              {{ra0, true}, {bottom2, false}, {rb0, false}, {bottom1, false}}});
    add_face(top, false,
             {{{q[4], true}, {q[5], true}, {q[6], true}, {q[7], true}},
              {{ran, true}, {top2, false}, {rbn, false}, {top1, false}}});
    for (int i = 0; i < 4; ++i) {
      add_face(side[i], false, {{{q[i], true}, {q[8 + (i + 1) % 4], true}, {q[4 + i], false}, {q[8 + i], false}}});
    }
    b.shells.push_back({0, 0, static_cast<std::uint32_t>(b.faces.size()), true});
    b.lump_count = 1;
    return b;
  }
};

// The threaded block builds as a valid solid of the expected volume, with
// its hole's cylinders and flanks one face each.
TopoDS_Shape threaded_block(const ThreadedBlock& spec) {
  const brep::BuildResult built = brep::build_body(spec.body());
  CHECK(built.report.built && built.report.solid && built.report.valid);
  CHECK_NEAR_TOL(volume(built.shape), spec.expected_volume(), 1e-6);
  CHECK(count(built.shape, TopAbs_FACE) == 10);
  return built.shape;
}

// A cut that crosses the thread: done, valid, one solid; what it has in
// common with the block is the material inside the tool (`inside`, sampled
// within `low`..`high`); and what it removed is that, as the faces the cut
// changed bound it.
TopoDS_Shape check_cut(const ThreadedBlock& spec, const TopoDS_Shape& block, const TopoDS_Shape& tool, const gp_Pnt& low,
                       const gp_Pnt& high, const std::function<bool(const gp_Pnt&)>& inside) {
  BRepAlgoAPI_Common common(block, tool);
  CHECK(common.IsDone());
  const double removed = common.IsDone() ? volume(common.Shape()) : 0.0;
  CHECK_NEAR_TOL(removed, spec.material_volume(low, high, inside), 0.01);
  BRepAlgoAPI_Cut cut(block, tool);
  CHECK(cut.IsDone());
  if (!cut.IsDone()) {
    return TopoDS_Shape();
  }
  CHECK(count(cut.Shape(), TopAbs_SOLID) == 1);
  CHECK(BRepCheck_Analyzer(cut.Shape()).IsValid());
  // What it removed, integrated over the faces the cut changed: the
  // integration over the whole body is not exact enough on faces running
  // 32 turns.
  const mitcad::geometry::ChangedVolume changed = mitcad::geometry::changed_volume({block}, {cut.Shape()});
  CHECK_NEAR_TOL(-changed.volume, removed, 1e-5);
  return cut.Shape();
}

TopoDS_Shape check_box_cut(const ThreadedBlock& spec, const TopoDS_Shape& block, const gp_Pnt& low, const gp_Pnt& high) {
  return check_cut(spec, block, BRepPrimAPI_MakeBox(low, high).Shape(), low, high,
                   [](const gp_Pnt&) { return true; });
}

void test_cut_across_a_thread_whose_faces_run_many_turns() {
  // Boxes of 1 mm across the thread's minor and major cylinders and its
  // flanks: near the bottom, in the middle and on another side. Without
  // 0510 the section curves on the cylinders lay a period or more outside
  // the faces and the results were invalid; without 0511 the parts of them
  // inside the faces were classified as outside and the first cut removed
  // nothing.
  const ThreadedBlock spec;
  const TopoDS_Shape block = threaded_block(spec);
  check_box_cut(spec, block, gp_Pnt(3.0, -0.5, 2.0), gp_Pnt(4.5, 0.5, 3.0));
  check_box_cut(spec, block, gp_Pnt(3.0, -0.5, 20.2), gp_Pnt(4.5, 0.5, 21.2));
  check_box_cut(spec, block, gp_Pnt(-0.5, -4.5, 30.0), gp_Pnt(0.5, -3.0, 31.0));
}

// The crossings of an edge with a face's surface found by sampling the
// signed distance to a cylinder about the x axis densely.
int sampled_crossings(const TopoDS_Edge& edge, double radius, double z) {
  BRepAdaptor_Curve curve(edge);
  const int n = 100000;
  int crossings = 0;
  double previous = 0.0;
  for (int k = 0; k <= n; ++k) {
    const double t = curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * k / n;
    const gp_Pnt p = curve.Value(t);
    const double d = std::hypot(p.Y(), p.Z() - z) - radius;
    if (k > 0 && (d > 0) != (previous > 0)) {
      ++crossings;
    }
    previous = d;
  }
  return crossings;
}

void test_cross_hole_through_a_thread_whose_edges_run_many_turns() {
  // A hole of radius 1 across the threaded hole at mid-height: each helix
  // edge of 32 turns crosses it twice where it passes the hole, several
  // times within a few turns. OCCT's intersection of a curve with a
  // quadric sampled the whole edge 50 times, every two thirds of a turn,
  // and found one crossing of each such pair or none (0512).
  const ThreadedBlock spec;
  const TopoDS_Shape block = threaded_block(spec);
  const double z = 20.0;
  const double radius = 1.0;
  const TopoDS_Shape drill =
      BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(-9, 0, z), gp_Dir(1, 0, 0)), radius, 18).Shape();
  TopoDS_Face side;
  for (TopExp_Explorer ex(drill, TopAbs_FACE); ex.More(); ex.Next()) {
    if (BRepAdaptor_Surface(TopoDS::Face(ex.Current())).GetType() == GeomAbs_Cylinder) {
      side = TopoDS::Face(ex.Current());
    }
  }
  int long_edges = 0;
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges;
  TopExp::MapShapes(block, TopAbs_EDGE, edges);
  for (int i = 1; i <= edges.Extent(); ++i) {
    const TopoDS_Edge& edge = TopoDS::Edge(edges(i));
    double first = 0.0;
    double last = 0.0;
    BRep_Tool::Range(edge, first, last);
    if (last - first < 100.0) {
      continue;
    }
    ++long_edges;
    IntTools_EdgeFace intersection;
    intersection.SetEdge(edge);
    intersection.SetFace(side);
    intersection.SetRange(IntTools_Range(first, last));
    intersection.Perform();
    CHECK(intersection.IsDone());
    const int expected = sampled_crossings(edge, radius, z);
    CHECK(expected >= 4);
    CHECK(intersection.IsDone() && intersection.CommonParts().Length() == expected);
  }
  CHECK(long_edges == 4);
  const auto in_drill = [&](const gp_Pnt& p) { return std::hypot(p.Y(), p.Z() - z) < radius; };
  check_cut(spec, block, drill, gp_Pnt(-8, -radius, z - radius), gp_Pnt(8, radius, z + radius), in_drill);
}

void test_second_cut_across_a_thread_whose_faces_run_many_turns() {
  // Box cuts on the result of a cross hole through the threaded hole at
  // mid-height (mitcad#380). The hole splits the thread's cylinders into
  // pieces below and above it, each on the one surface: the piece below
  // ends well short of the surface's first period. OCCT moved a box's
  // section curves from that period into such a piece's range, but checked
  // the copy at twice the shift, found it outside the piece and moved the
  // curves on from there, out of the piece (0560): the box cut removed
  // nothing and its result was invalid.
  const ThreadedBlock spec;
  const TopoDS_Shape block = threaded_block(spec);
  const double z = 20.0;
  const double radius = 1.0;
  const TopoDS_Shape drill =
      BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(-9, 0, z), gp_Dir(1, 0, 0)), radius, 18).Shape();
  const auto in_drill = [&](const gp_Pnt& p) { return std::hypot(p.Y(), p.Z() - z) < radius; };
  const TopoDS_Shape drilled =
      check_cut(spec, block, drill, gp_Pnt(-8, -radius, z - radius), gp_Pnt(8, radius, z + radius), in_drill);
  CHECK(!drilled.IsNull());
  if (drilled.IsNull()) {
    return;
  }
  // Below the cross hole and above it, on two sides of the hole.
  check_box_cut(spec, drilled, gp_Pnt(3.0, -0.5, 2.0), gp_Pnt(4.5, 0.5, 3.0));
  check_box_cut(spec, drilled, gp_Pnt(-0.5, -4.5, 30.0), gp_Pnt(0.5, -3.0, 31.0));
}

} // namespace

int main() {
  test::guarded("cut across a thread whose faces run many turns", test_cut_across_a_thread_whose_faces_run_many_turns);
  test::guarded("cross hole through a thread whose edges run many turns",
                test_cross_hole_through_a_thread_whose_edges_run_many_turns);
  test::guarded("second cut across a thread whose faces run many turns",
                test_second_cut_across_a_thread_whose_faces_run_many_turns);
  if (test::failures == 0) {
    std::puts("test_multi_turn_faces: all checks passed");
  }
  return test::failures == 0 ? 0 : 1;
}
