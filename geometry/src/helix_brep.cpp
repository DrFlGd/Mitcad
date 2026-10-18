// SPDX-License-Identifier: MIT
#include "helix_brep.hpp"

#include <algorithm>
#include <cmath>
#include <functional>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepLib.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GeomConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_Array2.hxx>
#include <NCollection_HArray1.hxx>
#include <ShapeFix_Shell.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Wire.hxx>
#include <Precision.hxx>
#include <gp_Ax3.hxx>
#include <gp_Lin.hxx>
#include <gp_Pln.hxx>
#include <gp_Trsf.hxx>

#include "history.hpp"
#include "util.hpp"

namespace mitcad::geometry::detail {
namespace {

constexpr double kTwoPi = 6.28318530717958647692;
// How far the helices may lie from the exact screw motion of their points
// (mm): the interpolation's points a turn are chosen for it.
constexpr double kHelixExact = 1.0e-7;
// The edges' tolerance before OCCT measures them.
constexpr double kEdgeTolerance = 1.0e-7;

// The screw motion: about the axis by `sign` turns of `v / 2 pi` while
// rising `pitch` a turn.
struct Screw {
  gp_Ax1 axis;
  gp_Dir x;
  gp_Dir y;
  double pitch = 0.0;
  double sign = 1.0;

  gp_Trsf at(double v) const {
    gp_Trsf turn;
    turn.SetRotation(axis, sign * v);
    gp_Trsf rise;
    rise.SetTranslation(gp_Vec(axis.Direction()) * (pitch * v / kTwoPi));
    return rise * turn;
  }
  // A point's height along the axis, and its radial part in x and y.
  double height(const gp_Pnt& p) const { return gp_Vec(axis.Location(), p).Dot(gp_Vec(axis.Direction())); }
  double along_x(const gp_Pnt& p) const { return gp_Vec(axis.Location(), p).Dot(gp_Vec(x)); }
  double along_y(const gp_Pnt& p) const { return gp_Vec(axis.Location(), p).Dot(gp_Vec(y)); }
};

// The helices of all points as one family of cubic B-splines interpolated
// at the same parameters (the screw's v, `per_turn` points a turn, from two
// turns before the start to two after the end): a point's helix is a
// combination of the helices of the frame's directions (interpolation is
// linear), so the helices of points a pitch apart along the axis are the
// same curve shifted by a turn, to rounding, away from the family's ends.
class Helices {
public:
  Helices(const Screw& screw, double turns, double radius) : m_screw(screw) {
    // The cubic interpolation of a circle of radius r at steps h deviates
    // about 5/384 r h^4.
    const double step = std::pow(kHelixExact * 384.0 / (5.0 * std::max(radius, 1.0e-3)), 0.25);
    const int per_turn = std::clamp(static_cast<int>(std::ceil(kTwoPi / step / 8.0)) * 8, 32, 1024);
    const int from = -2 * per_turn;
    const int to = static_cast<int>(std::ceil(turns * per_turn)) + 2 * per_turn;
    const int count = to - from + 1;
    occ::handle<NCollection_HArray1<double>> params = new NCollection_HArray1<double>(1, count);
    occ::handle<NCollection_HArray1<gp_Pnt>> cos_points = new NCollection_HArray1<gp_Pnt>(1, count);
    occ::handle<NCollection_HArray1<gp_Pnt>> sin_points = new NCollection_HArray1<gp_Pnt>(1, count);
    occ::handle<NCollection_HArray1<gp_Pnt>> rise_points = new NCollection_HArray1<gp_Pnt>(1, count);
    for (int i = 0; i < count; ++i) {
      const double v = kTwoPi * (from + i) / per_turn;
      const double a = screw.sign * v;
      params->SetValue(i + 1, v);
      // Where the screw takes the frame's x and y directions (as points).
      const gp_XYZ x = screw.x.XYZ() * std::cos(a) + screw.y.XYZ() * std::sin(a);
      const gp_XYZ y = screw.y.XYZ() * std::cos(a) - screw.x.XYZ() * std::sin(a);
      cos_points->SetValue(i + 1, gp_Pnt(x));
      sin_points->SetValue(i + 1, gp_Pnt(y));
      rise_points->SetValue(i + 1, gp_Pnt(screw.axis.Direction().XYZ() * (screw.pitch * v / kTwoPi)));
    }
    const auto interpolated = [&](const occ::handle<NCollection_HArray1<gp_Pnt>>& points) {
      GeomAPI_Interpolate interpolate(points, params, false, 1.0e-12);
      interpolate.Perform();
      if (!interpolate.IsDone()) {
        throw std::runtime_error("the helix could not be built");
      }
      return interpolate.Curve();
    };
    m_x = interpolated(cos_points);
    m_y = interpolated(sin_points);
    m_rise = interpolated(rise_points);
    if (m_x->NbPoles() != m_y->NbPoles() || m_x->NbPoles() != m_rise->NbPoles() ||
        m_x->NbKnots() != m_rise->NbKnots()) {
      throw std::runtime_error("the helix could not be built");
    }
  }

  int poles() const { return m_x->NbPoles(); }
  const occ::handle<Geom_BSplineCurve>& basis() const { return m_x; }

  // The poles of the point's helix.
  std::vector<gp_Pnt> of(const gp_Pnt& p) const {
    const gp_XYZ base = m_screw.axis.Location().XYZ() + m_screw.axis.Direction().XYZ() * m_screw.height(p);
    const double a = m_screw.along_x(p);
    const double b = m_screw.along_y(p);
    std::vector<gp_Pnt> out;
    out.reserve(static_cast<std::size_t>(poles()));
    for (int j = 1; j <= poles(); ++j) {
      out.emplace_back(base + m_x->Pole(j).XYZ() * a + m_y->Pole(j).XYZ() * b + m_rise->Pole(j).XYZ());
    }
    return out;
  }

  // The point's helix as a curve.
  occ::handle<Geom_BSplineCurve> curve(const gp_Pnt& p) const {
    const std::vector<gp_Pnt> points = of(p);
    NCollection_Array1<gp_Pnt> poles(1, static_cast<int>(points.size()));
    for (std::size_t j = 0; j < points.size(); ++j) {
      poles.SetValue(static_cast<int>(j) + 1, points[j]);
    }
    return new Geom_BSplineCurve(poles, m_x->Knots(), m_x->Multiplicities(), m_x->Degree());
  }

private:
  Screw m_screw;
  occ::handle<Geom_BSplineCurve> m_x;
  occ::handle<Geom_BSplineCurve> m_y;
  occ::handle<Geom_BSplineCurve> m_rise;
};

// What a profile edge sweeps.
enum class Role { Side, Bottom, Top };

// An edge of the profile loop in the loop's order, as a B-spline from its
// corner to the next.
struct ProfileEdge {
  occ::handle<Geom_BSplineCurve> curve;
  std::string name;
  Role role = Role::Side;
  // A line along the axis: it sweeps a cylinder.
  bool along_axis = false;
  // An edge of the core that replaces the side whose turns meet.
  bool core = false;
};

// An edge of the solid used by a face: oriented along the face's wire, with
// its curve on the face.
struct Use {
  TopoDS_Edge edge;
  bool reversed = false;
  occ::handle<Geom2d_Curve> pcurve;
};

// A piece of the helix through a corner of the profile: the edge, the range
// of the screw's v it spans at this corner, and how much more the edge's own
// parameter is (the edge belongs to the corner a pitch below this one when
// the turns meet there).
struct HelixPiece {
  TopoDS_Edge edge;
  double from = 0.0;
  double to = 0.0;
  double shift = 0.0;
};

occ::handle<Geom_BSplineCurve> as_bspline(const TopoDS_Edge& edge) {
  double first = 0.0;
  double last = 0.0;
  const occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, first, last);
  if (curve.IsNull()) {
    throw std::invalid_argument("a profile edge has no curve");
  }
  occ::handle<Geom_BSplineCurve> spline =
      GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(curve, first, last));
  if (spline.IsNull()) {
    throw std::invalid_argument("a profile edge cannot be swept as a B-spline");
  }
  if (edge.Orientation() == TopAbs_REVERSED) {
    spline->Reverse();
  }
  return spline;
}

occ::handle<Geom_BSplineCurve> segment(const gp_Pnt& a, const gp_Pnt& b) {
  NCollection_Array1<gp_Pnt> poles(1, 2);
  poles.SetValue(1, a);
  poles.SetValue(2, b);
  NCollection_Array1<double> knots(1, 2);
  knots.SetValue(1, 0.0);
  knots.SetValue(2, a.Distance(b));
  NCollection_Array1<int> mults(1, 2);
  mults.SetValue(1, 2);
  mults.SetValue(2, 2);
  return new Geom_BSplineCurve(poles, knots, mults, 1);
}

// A straight 2D curve from `a` at t0 to `b` at t1, parametrised by t.
occ::handle<Geom2d_Curve> line2d(const gp_Pnt2d& a, const gp_Pnt2d& b, double t0, double t1) {
  NCollection_Array1<gp_Pnt2d> poles(1, 2);
  poles.SetValue(1, a);
  poles.SetValue(2, b);
  NCollection_Array1<double> knots(1, 2);
  knots.SetValue(1, t0);
  knots.SetValue(2, t1);
  NCollection_Array1<int> mults(1, 2);
  mults.SetValue(1, 2);
  mults.SetValue(2, 2);
  return new Geom2d_BSplineCurve(poles, knots, mults, 1);
}

// The signed area the wire's curves on the face enclose (positive against
// the clock in the face's parameters).
double uv_area(const std::vector<Use>& wire) {
  std::vector<gp_Pnt2d> points;
  for (const Use& use : wire) {
    double first = 0.0;
    double last = 0.0;
    BRep_Tool::Range(use.edge, first, last);
    constexpr int kSteps = 32;
    for (int i = 0; i < kSteps; ++i) {
      const double s = static_cast<double>(i) / kSteps;
      const double t = use.reversed ? last + (first - last) * s : first + (last - first) * s;
      points.push_back(use.pcurve->Value(t));
    }
  }
  double area = 0.0;
  for (std::size_t i = 0; i < points.size(); ++i) {
    const gp_Pnt2d& a = points[i];
    const gp_Pnt2d& b = points[(i + 1) % points.size()];
    area += a.X() * b.Y() - b.X() * a.Y();
  }
  return area / 2.0;
}

} // namespace

ShapePtr screwed_brep(const NamedWire& loop, const gp_Ax1& axis, double pitch, double revolutions,
                      bool left_handed, const NameList& first, const NameList& last,
                      const std::optional<MeetingSide>& meeting, double piece_turns) {
  Screw screw;
  screw.axis = axis;
  const gp_Ax3 frame(axis.Location(), axis.Direction());
  screw.x = frame.XDirection();
  screw.y = frame.YDirection();
  screw.pitch = pitch;
  screw.sign = left_handed ? -1.0 : 1.0;
  const double end = kTwoPi * revolutions;
  const gp_Vec up(axis.Direction());

  // The profile's edges, the meeting side replaced by the core beyond it:
  // from its lower end (Lo) out to Lo', to the core's middle M, to Hi' and
  // back to its upper end Hi = Lo + pitch along the axis.
  std::vector<ProfileEdge> edges;
  bool met = false;
  std::string meeting_name;
  for (BRepTools_WireExplorer w(loop.wire); w.More(); w.Next()) {
    const TopoDS_Edge& edge = w.Current();
    std::string name;
    for (const auto& [named, n] : loop.edges) {
      if (named.IsSame(edge)) {
        name = n;
      }
    }
    if (meeting && edge.IsSame(meeting->edge)) {
      const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge, true));
      gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge, true));
      const bool rising = screw.height(b) > screw.height(a);
      // The side as long as the pitch exactly: its upper end moved onto the
      // lower one's next turn (it was within 1e-6 of it).
      const gp_Pnt lo = rising ? a : b;
      const gp_Pnt hi = lo.Translated(up * pitch);
      const gp_Vec out = gp_Vec(meeting->away) * meeting->depth;
      const gp_Pnt lo_out = lo.Translated(out);
      const gp_Pnt hi_out = hi.Translated(out);
      const gp_Pnt middle = lo.Translated(up * (pitch / 2.0) + out * 2.0);
      const auto add = [&](const gp_Pnt& p, const gp_Pnt& q, Role role) {
        edges.push_back({segment(p, q), name, role, false, true});
      };
      if (rising) {
        add(lo, lo_out, Role::Bottom);
        add(lo_out, middle, Role::Side);
        add(middle, hi_out, Role::Side);
        add(hi_out, hi, Role::Top);
      } else {
        add(hi, hi_out, Role::Top);
        add(hi_out, middle, Role::Side);
        add(middle, lo_out, Role::Side);
        add(lo_out, lo, Role::Bottom);
      }
      met = true;
      meeting_name = name;
      continue;
    }
    ProfileEdge e;
    e.curve = as_bspline(edge);
    e.name = name;
    const BRepAdaptor_Curve adaptor(edge);
    if (adaptor.GetType() == GeomAbs_Line) {
      const gp_Vec d(e.curve->StartPoint(), e.curve->EndPoint());
      e.along_axis = d.Magnitude() > Precision::Confusion() &&
                     std::abs(std::abs(d.Normalized().Dot(up)) - 1.0) < 1.0e-12 &&
                     gp_Lin(axis).Distance(e.curve->StartPoint()) > Precision::Confusion();
    }
    edges.push_back(e);
  }
  if (meeting && !met) {
    throw std::invalid_argument("the side whose turns meet is not in the profile");
  }
  const std::size_t n = edges.size();
  if (n == 0) {
    throw std::invalid_argument("the profile has no edges");
  }
  // The corners: corner i starts edge i. Edges meet exactly there, at the
  // core's ends where the core meets the profile's other edges.
  std::vector<gp_Pnt> corners(n);
  for (std::size_t i = 0; i < n; ++i) {
    const ProfileEdge& before = edges[(i + n - 1) % n];
    corners[i] = before.core ? before.curve->EndPoint() : edges[i].curve->StartPoint();
  }
  for (std::size_t i = 0; i < n; ++i) {
    const std::size_t next = (i + 1) % n;
    occ::handle<Geom_BSplineCurve>& c = edges[i].curve;
    if (c->StartPoint().Distance(corners[i]) > 1.0e-5 || c->EndPoint().Distance(corners[next]) > 1.0e-5) {
      throw std::invalid_argument("the profile's edges do not join");
    }
    c->SetPole(1, corners[i]);
    c->SetPole(c->NbPoles(), corners[next]);
  }
  if (met && revolutions <= 1.0 + 1.0e-9) {
    throw std::invalid_argument("turns that meet need more than one turn");
  }

  double radius = 0.0;
  for (const ProfileEdge& e : edges) {
    for (int k = 1; k <= e.curve->NbPoles(); ++k) {
      radius = std::max(radius, gp_Lin(axis).Distance(e.curve->Pole(k)));
    }
  }
  const Helices helices(screw, revolutions, radius);

  // The range of v each edge's face spans.
  const auto range = [&](const ProfileEdge& e) {
    switch (e.role) {
    case Role::Bottom:
      return std::make_pair(0.0, kTwoPi);
    case Role::Top:
      return std::make_pair(end - kTwoPi, end);
    case Role::Side:
      break;
    }
    return std::make_pair(0.0, end);
  };

  BRep_Builder builder;
  const gp_Trsf at_end = screw.at(end);
  std::vector<TopoDS_Vertex> start_vertices(n);
  std::vector<TopoDS_Vertex> end_vertices(n);
  for (std::size_t i = 0; i < n; ++i) {
    builder.MakeVertex(start_vertices[i], corners[i], kEdgeTolerance);
    builder.MakeVertex(end_vertices[i], corners[i].Transformed(at_end), kEdgeTolerance);
  }
  // The edges of the caps: each profile edge at the start and at the end.
  std::vector<TopoDS_Edge> start_edges(n);
  std::vector<TopoDS_Edge> end_edges(n);
  std::vector<occ::handle<Geom_BSplineCurve>> end_curves(n);
  for (std::size_t i = 0; i < n; ++i) {
    const std::size_t next = (i + 1) % n;
    const occ::handle<Geom_BSplineCurve>& c = edges[i].curve;
    builder.MakeEdge(start_edges[i], c, kEdgeTolerance);
    builder.Add(start_edges[i], start_vertices[i].Oriented(TopAbs_FORWARD));
    builder.Add(start_edges[i], start_vertices[next].Oriented(TopAbs_REVERSED));
    builder.Range(start_edges[i], c->FirstParameter(), c->LastParameter());
    end_curves[i] = occ::down_cast<Geom_BSplineCurve>(c->Transformed(at_end));
    builder.MakeEdge(end_edges[i], end_curves[i], kEdgeTolerance);
    builder.Add(end_edges[i], end_vertices[i].Oriented(TopAbs_FORWARD));
    builder.Add(end_edges[i], end_vertices[next].Oriented(TopAbs_REVERSED));
    builder.Range(end_edges[i], c->FirstParameter(), c->LastParameter());
  }

  // The faces are split into pieces along the screw's v every `piece_turns`
  // turns, on one grid for all of them (so that the faces whose turns meet
  // split the edge they share at the same points): every piece winds less
  // than half a turn about the axis, so that its intersections with the
  // bodies' faces about the axis are single arcs, which OCCT's booleans
  // find reliably. The pieces of a face share its surface, so that the
  // merge of faces after a boolean makes them one face again.
  const double step = kTwoPi * piece_turns;
  const auto grid_inside = [&](double a, double b) {
    std::vector<double> out;
    if (step <= 0.0) {
      return out;
    }
    for (long k = static_cast<long>(std::floor(a / step)) + 1; k * step < b - 1.0e-9; ++k) {
      if (k * step > a + 1.0e-9) {
        out.push_back(k * step);
      }
    }
    return out;
  };

  // The helix pieces at each corner. A corner of the bottom edge (Lo, Lo')
  // has its own helix up to one turn and, beyond it, the edges where the
  // turns meet; the corner a pitch above it (Hi, Hi', a corner of the top
  // edge) has those edges up to its last turn and its own helix beyond.
  std::vector<std::vector<HelixPiece>> pieces(n);
  const auto chain = [&](const gp_Pnt& p, double from, double to, const TopoDS_Vertex& a, const TopoDS_Vertex& b) {
    const occ::handle<Geom_BSplineCurve> curve = helices.curve(p);
    std::vector<double> at{from};
    for (const double v : grid_inside(from, to)) {
      at.push_back(v);
    }
    at.push_back(to);
    std::vector<HelixPiece> out;
    TopoDS_Vertex start = a;
    for (std::size_t k = 0; k + 1 < at.size(); ++k) {
      TopoDS_Vertex stop = b;
      if (k + 2 < at.size()) {
        builder.MakeVertex(stop, curve->Value(at[k + 1]), kEdgeTolerance);
      }
      TopoDS_Edge edge;
      // Its own piece of the helix (the same parameters).
      const occ::handle<Geom_BSplineCurve> own = occ::down_cast<Geom_BSplineCurve>(curve->Copy());
      own->Segment(at[k], at[k + 1]);
      builder.MakeEdge(edge, own, kEdgeTolerance);
      builder.Add(edge, start.Oriented(TopAbs_FORWARD));
      builder.Add(edge, stop.Oriented(TopAbs_REVERSED));
      builder.Range(edge, at[k], at[k + 1]);
      out.push_back({edge, at[k], at[k + 1], 0.0});
      start = stop;
    }
    return out;
  };
  const auto role_at = [&](std::size_t corner, Role role) {
    return edges[corner].role == role || edges[(corner + n - 1) % n].role == role;
  };
  // The corner whose point is `p`.
  const auto corner_at = [&](const gp_Pnt& p) {
    for (std::size_t i = 0; i < n; ++i) {
      if (corners[i].Distance(p) <= 1.0e-7) {
        return i;
      }
    }
    throw std::runtime_error("the turns' meeting corners do not match");
  };
  for (std::size_t i = 0; i < n; ++i) {
    if (role_at(i, Role::Bottom)) {
      const std::size_t above = corner_at(corners[i].Translated(up * pitch));
      // Its helix at kTwoPi is the start of the corner above.
      pieces[i] = chain(corners[i], 0.0, kTwoPi, start_vertices[i], start_vertices[above]);
      for (const HelixPiece& piece : chain(corners[i], kTwoPi, end, start_vertices[above], end_vertices[i])) {
        pieces[i].push_back(piece);
        pieces[above].push_back({piece.edge, piece.from - kTwoPi, piece.to - kTwoPi, kTwoPi});
      }
    } else if (!role_at(i, Role::Top)) {
      pieces[i] = chain(corners[i], 0.0, end, start_vertices[i], end_vertices[i]);
    }
  }
  for (std::size_t i = 0; i < n; ++i) {
    if (role_at(i, Role::Top)) {
      const std::size_t below = corner_at(corners[i].Translated(-up * pitch));
      if (pieces[i].empty()) {
        throw std::runtime_error("the turns' meeting corners do not match");
      }
      // Its helix from the last turn starts where the corner below ends.
      for (const HelixPiece& piece : chain(corners[i], end - kTwoPi, end, end_vertices[below], end_vertices[i])) {
        pieces[i].push_back(piece);
      }
    }
  }
  // The vertex of a corner's helix at v.
  const auto vertex_at = [&](std::size_t corner, double v) {
    for (const HelixPiece& piece : pieces[corner]) {
      if (std::abs(piece.from - v) <= 1.0e-9) {
        return TopExp::FirstVertex(piece.edge);
      }
      if (std::abs(piece.to - v) <= 1.0e-9) {
        return TopExp::LastVertex(piece.edge);
      }
    }
    throw std::runtime_error("the helix's pieces do not match");
  };

  // The faces.
  std::vector<std::pair<TopoDS_Face, std::string>> named;
  const auto make_face = [&](const occ::handle<Geom_Surface>& surface, std::vector<Use> wire) {
    if (uv_area(wire) < 0.0) {
      std::reverse(wire.begin(), wire.end());
      for (Use& use : wire) {
        use.reversed = !use.reversed;
      }
    }
    TopoDS_Face face;
    builder.MakeFace(face, surface, kEdgeTolerance);
    TopoDS_Wire w;
    builder.MakeWire(w);
    for (std::size_t k = 0; k < wire.size(); ++k) {
      const Use& use = wire[k];
      double a = 0.0;
      double b = 0.0;
      BRep_Tool::Range(use.edge, a, b);
      // An edge the wire runs along twice is a seam of the face: its curve
      // with the edge forward first.
      const Use* other = nullptr;
      for (std::size_t l = 0; l < wire.size(); ++l) {
        if (l != k && wire[l].edge.IsSame(use.edge)) {
          other = &wire[l];
        }
      }
      if (other == nullptr) {
        builder.UpdateEdge(use.edge, use.pcurve, face, kEdgeTolerance);
      } else if (!use.reversed) {
        builder.UpdateEdge(use.edge, use.pcurve, other->pcurve, face, kEdgeTolerance);
      }
      builder.Range(use.edge, face, a, b);
      builder.Add(w, use.edge.Oriented(use.reversed ? TopAbs_REVERSED : TopAbs_FORWARD));
    }
    w.Closed(true);
    builder.Add(face, w);
    return face;
  };

  for (std::size_t i = 0; i < n; ++i) {
    const ProfileEdge& e = edges[i];
    const std::size_t next = (i + 1) % n;
    const auto [from, to] = range(e);
    const double u0 = e.curve->FirstParameter();
    const double u1 = e.curve->LastParameter();
    occ::handle<Geom_Surface> surface;
    // The face's curve of an edge along the profile at v (the edge's own
    // curve's parameter running along it), and of a helix piece at a corner.
    std::function<occ::handle<Geom2d_Curve>(double, bool)> across;
    std::function<occ::handle<Geom2d_Curve>(std::size_t, const HelixPiece&)> along;
    // The curve of the edge between two pieces of the face at v.
    std::function<occ::handle<Geom_Curve>(double)> iso;
    if (e.along_axis) {
      // A cylinder about the axis: u the angle from the frame's x, v the
      // height along the axis.
      const gp_Pnt p0 = e.curve->StartPoint();
      const gp_Pnt p1 = e.curve->EndPoint();
      const double rho = gp_Lin(axis).Distance(p0);
      surface = new Geom_CylindricalSurface(frame, rho);
      const double alpha = std::atan2(screw.along_y(p0), screw.along_x(p0));
      const double h0 = screw.height(p0);
      const double h1 = screw.height(p1);
      across = [=](double v, bool) {
        const double u = alpha + screw.sign * v;
        const double rise = pitch * v / kTwoPi;
        return line2d(gp_Pnt2d(u, h0 + rise), gp_Pnt2d(u, h1 + rise), u0, u1);
      };
      along = [=](std::size_t corner, const HelixPiece& piece) {
        const double h = screw.height(corners[corner]);
        const double a = piece.from + piece.shift;
        const double b = piece.to + piece.shift;
        return line2d(gp_Pnt2d(alpha + screw.sign * piece.from, h + pitch * piece.from / kTwoPi),
                      gp_Pnt2d(alpha + screw.sign * piece.to, h + pitch * piece.to / kTwoPi), a, b);
      };
      const occ::handle<Geom_BSplineCurve> c = e.curve;
      iso = [=](double v) -> occ::handle<Geom_Curve> {
        return occ::down_cast<Geom_Curve>(c->Transformed(screw.at(v)));
      };
    } else {
      // The edge's B-spline with each pole moved along its helix.
      const occ::handle<Geom_BSplineCurve>& c = e.curve;
      const int nu = c->NbPoles();
      const int nv = helices.poles();
      NCollection_Array2<gp_Pnt> poles(1, nu, 1, nv);
      NCollection_Array2<double> weights(1, nu, 1, nv);
      for (int k = 1; k <= nu; ++k) {
        const std::vector<gp_Pnt> helix = helices.of(c->Pole(k));
        for (int j = 1; j <= nv; ++j) {
          poles.SetValue(k, j, helix[static_cast<std::size_t>(j) - 1]);
          weights.SetValue(k, j, c->Weight(k));
        }
      }
      const occ::handle<Geom_BSplineCurve>& basis = helices.basis();
      occ::handle<Geom_BSplineSurface> spline;
      if (c->IsRational()) {
        spline = new Geom_BSplineSurface(poles, weights, c->Knots(), basis->Knots(), c->Multiplicities(),
                                         basis->Multiplicities(), c->Degree(), basis->Degree());
      } else {
        spline = new Geom_BSplineSurface(poles, c->Knots(), basis->Knots(), c->Multiplicities(),
                                         basis->Multiplicities(), c->Degree(), basis->Degree());
      }
      surface = spline;
      across = [=](double v, bool reversed) {
        // A cap edge of the edge itself runs with u; the start cap's edge of
        // the top edge, met at the bottom edge's far end (and the end cap's
        // edge of the bottom edge at the top edge's), runs against it.
        if (reversed) {
          return line2d(gp_Pnt2d(u1, v), gp_Pnt2d(u0, v), u0, u1);
        }
        return line2d(gp_Pnt2d(u0, v), gp_Pnt2d(u1, v), u0, u1);
      };
      along = [=](std::size_t corner, const HelixPiece& piece) {
        const double u = corner == i ? u0 : u1;
        return line2d(gp_Pnt2d(u, piece.from), gp_Pnt2d(u, piece.to), piece.from + piece.shift,
                      piece.to + piece.shift);
      };
      iso = [=](double v) -> occ::handle<Geom_Curve> { return spline->VIso(v); };
    }
    // Along the profile at the face's ends: the edge's own cap edges, or at
    // a ramp's far end the other ramp's cap edge.
    const auto cap_use = [&](double v) -> Use {
      if (v <= 0.0) {
        return {start_edges[i], false, across(0.0, false)};
      }
      if (v >= end) {
        return {end_edges[i], false, across(end, false)};
      }
      // S(2 pi) of the bottom edge is the top edge reversed (both are
      // segments of the same length).
      for (std::size_t k = 0; k < n; ++k) {
        if (e.role == Role::Bottom && edges[k].role == Role::Top) {
          return {start_edges[k], true, across(v, true)};
        }
        if (e.role == Role::Top && edges[k].role == Role::Bottom) {
          return {end_edges[k], true, across(v, true)};
        }
      }
      throw std::runtime_error("the turns' meeting edges do not match");
    };
    // Along the profile between two pieces.
    const auto between = [&](double v) -> Use {
      TopoDS_Edge edge;
      builder.MakeEdge(edge, iso(v), kEdgeTolerance);
      builder.Add(edge, vertex_at(i, v).Oriented(TopAbs_FORWARD));
      builder.Add(edge, vertex_at(next, v).Oriented(TopAbs_REVERSED));
      builder.Range(edge, u0, u1);
      return {edge, false, across(v, false)};
    };
    // The helix pieces of a corner within [a, b], rising.
    const auto rising = [&](std::size_t corner, double a, double b) {
      std::vector<HelixPiece> chosen;
      for (const HelixPiece& piece : pieces[corner]) {
        if (piece.from >= a - 1.0e-9 && piece.to <= b + 1.0e-9) {
          chosen.push_back(piece);
        }
      }
      std::sort(chosen.begin(), chosen.end(),
                [](const HelixPiece& p, const HelixPiece& q) { return p.from < q.from; });
      std::vector<Use> uses;
      for (const HelixPiece& piece : chosen) {
        uses.push_back({piece.edge, false, along(corner, piece)});
      }
      return uses;
    };
    std::vector<double> at{from};
    for (const double v : grid_inside(from, to)) {
      at.push_back(v);
    }
    at.push_back(to);
    Use lower = cap_use(from);
    for (std::size_t k = 0; k + 1 < at.size(); ++k) {
      Use upper = k + 2 < at.size() ? between(at[k + 1]) : cap_use(to);
      std::vector<Use> wire{lower};
      for (const Use& use : rising(next, at[k], at[k + 1])) {
        wire.push_back(use);
      }
      Use top = upper;
      top.reversed = !top.reversed;
      wire.push_back(top);
      std::vector<Use> down = rising(i, at[k], at[k + 1]);
      std::reverse(down.begin(), down.end());
      for (Use& use : down) {
        use.reversed = true;
        wire.push_back(use);
      }
      named.emplace_back(make_face(surface, wire), e.role == Role::Side ? e.name : meeting_name);
      lower = upper;
    }
  }

  // The caps: planes of the profile at the start and at the end.
  const auto cap = [&](const std::vector<TopoDS_Edge>& cap_edges, const gp_Trsf& place) {
    // The profile's plane: through the points of its edges spanning the
    // largest triangle with the first corner.
    std::vector<gp_Pnt> points;
    for (const ProfileEdge& e : edges) {
      for (int k = 0; k < 8; ++k) {
        const double t = e.curve->FirstParameter() + (e.curve->LastParameter() - e.curve->FirstParameter()) * k / 8.0;
        points.push_back(e.curve->Value(t));
      }
    }
    gp_Vec normal;
    for (std::size_t a = 1; a < points.size(); ++a) {
      for (std::size_t b = a + 1; b < points.size(); ++b) {
        const gp_Vec c = gp_Vec(points[0], points[a]).Crossed(gp_Vec(points[0], points[b]));
        if (c.Magnitude() > normal.Magnitude()) {
          normal = c;
        }
      }
    }
    if (normal.Magnitude() < 1.0e-12) {
      throw std::invalid_argument("the profile has no plane");
    }
    const gp_Pln plane = gp_Pln(points[0], gp_Dir(normal)).Transformed(place);
    TopoDS_Face face;
    builder.MakeFace(face, new Geom_Plane(plane), kEdgeTolerance);
    TopoDS_Wire w;
    builder.MakeWire(w);
    for (const TopoDS_Edge& edge : cap_edges) {
      builder.Add(w, edge.Oriented(TopAbs_FORWARD));
    }
    w.Closed(true);
    builder.Add(face, w);
    for (const TopoDS_Edge& edge : cap_edges) {
      BRepLib::BuildPCurveForEdgeOnPlane(edge, face);
    }
    return face;
  };
  const TopoDS_Face start_cap = cap(start_edges, gp_Trsf());
  const TopoDS_Face end_cap = cap(end_edges, at_end);

  TopoDS_Shell shell;
  builder.MakeShell(shell);
  for (const auto& [face, name] : named) {
    builder.Add(shell, face);
  }
  builder.Add(shell, start_cap);
  builder.Add(shell, end_cap);
  BRepLib::SameParameter(shell, 1.0e-5, true);
  BRepLib::UpdateTolerances(shell);
  ShapeFix_Shell fix(shell);
  fix.FixFaceOrientation(shell);
  if (fix.NbShells() != 1) {
    throw std::runtime_error("the helix's faces do not close into one shell");
  }
  TopoDS_Shell fixed = fix.Shell();
  fixed.Closed(true);
  TopoDS_Solid solid;
  builder.MakeSolid(solid);
  builder.Add(solid, fixed);
  BRepLib::OrientClosedSolid(solid);
  if (!is_valid(solid)) {
    throw std::runtime_error("the helix's B-rep is not valid");
  }
  // Names: the faces kept their identity through the orientation fix (the
  // fix may reverse them), so they are found by their surfaces' TShape.
  FaceNamer namer(solid);
  for (TopExp_Explorer f(solid, TopAbs_FACE); f.More(); f.Next()) {
    for (const auto& [face, name] : named) {
      if (f.Current().IsSame(face) && !name.empty()) {
        namer.add(f.Current(), name);
      }
    }
    if (f.Current().IsSame(start_cap)) {
      for (const std::string& name : first) {
        namer.add(f.Current(), name);
      }
    }
    if (f.Current().IsSame(end_cap)) {
      for (const std::string& name : last) {
        namer.add(f.Current(), name);
      }
    }
  }
  return namer.shape();
}

} // namespace mitcad::geometry::detail
