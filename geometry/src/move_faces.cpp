// SPDX-License-Identifier: MIT
#include "mitcad/geometry/move_faces.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <optional>
#include <stdexcept>
#include <vector>

#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepTools_Modification.hxx>
#include <BRepTools_Modifier.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_IntCS.hxx>
#include <GeomAPI_IntSS.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <GeomProjLib.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom_Curve.hxx>
#include <Geom_Surface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_DataMap.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_List.hxx>
#include <NCollection_Map.hxx>
#include <Precision.hxx>
#include <TopAbs.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <gp_Mat.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Vec.hxx>
#include <gp_Vec2d.hxx>

#include "face_select.hpp"
#include "history.hpp"
#include "util.hpp"

namespace mitcad::geometry {

namespace {

using Ancestors = NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<TopoDS_Shape>, TopTools_ShapeMapHasher>;
using ShapeSet = NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher>;

// How near a new edge or vertex must lie to where its curve or point is
// found to belong to it (the body's units, mm).
constexpr double kNear = 1.0e-6;

// A face's surface in global coordinates.
occ::handle<Geom_Surface> global_surface(const TopoDS_Face& face) {
  TopLoc_Location location;
  const occ::handle<Geom_Surface> surface = BRep_Tool::Surface(face, location);
  if (surface.IsNull() || location.IsIdentity()) {
    return surface;
  }
  return occ::down_cast<Geom_Surface>(surface->Transformed(location.Transformation()));
}

// An edge's curve in global coordinates, with its range.
occ::handle<Geom_Curve> global_curve(const TopoDS_Edge& edge, double& first, double& last) {
  TopLoc_Location location;
  occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, location, first, last);
  if (curve.IsNull() || location.IsIdentity()) {
    return curve;
  }
  return occ::down_cast<Geom_Curve>(curve->Transformed(location.Transformation()));
}

// The faces of an edge, each once.
std::vector<TopoDS_Face> faces_of(const Ancestors& map, const TopoDS_Shape& edge) {
  std::vector<TopoDS_Face> faces;
  const int index = map.FindIndex(edge);
  if (index == 0) {
    return faces;
  }
  for (const TopoDS_Shape& face : map(index)) {
    if (std::none_of(faces.begin(), faces.end(), [&](const TopoDS_Face& f) { return f.IsSame(face); })) {
      faces.push_back(TopoDS::Face(face));
    }
  }
  return faces;
}

// The parameter on `curve` of the point nearest `p`, within kNear, near
// `hint` where the curve passes `p` more than once.
std::optional<double> parameter_on(const occ::handle<Geom_Curve>& curve, const gp_Pnt& p, double tolerance,
                                   std::optional<double> hint = std::nullopt) {
  GeomAPI_ProjectPointOnCurve projection(p, curve);
  std::optional<double> best;
  double best_score = std::numeric_limits<double>::infinity();
  for (int i = 1; i <= projection.NbPoints(); ++i) {
    if (projection.Distance(i) > tolerance) {
      continue;
    }
    const double w = projection.Parameter(i);
    const double score = hint ? std::abs(w - *hint) : projection.Distance(i);
    if (score < best_score) {
      best_score = score;
      best = w;
    }
  }
  return best;
}

// The point where `curve` meets `surface` nearest `near`.
std::optional<gp_Pnt> meet(const occ::handle<Geom_Curve>& curve, const occ::handle<Geom_Surface>& surface,
                           const gp_Pnt& near) {
  GeomAPI_IntCS intersection(curve, surface);
  if (!intersection.IsDone()) {
    return std::nullopt;
  }
  std::optional<gp_Pnt> best;
  for (int i = 1; i <= intersection.NbPoints(); ++i) {
    const gp_Pnt p = intersection.Point(i);
    if (!best || p.Distance(near) < best->Distance(near)) {
      best = p;
    }
  }
  return best;
}

class MovedFaces : public BRepTools_Modification {
public:
  // An edge between a moved face and one that stays: its new curve (global)
  // and range.
  struct Edge {
    occ::handle<Geom_Curve> curve;
    double first = 0.0;
    double last = 0.0;
  };

  MovedFaces(const TopoDS_Shape& body, const ShapeSet& moved, const gp_Trsf& motion)
      : m_moved(moved), m_motion(motion) {
    TopExp::MapShapesAndAncestors(body, TopAbs_EDGE, TopAbs_FACE, m_edgeFaces);
    TopExp::MapShapesAndAncestors(body, TopAbs_VERTEX, TopAbs_EDGE, m_vertexEdges);
    // The edges between moved faces and faces that stay: the surfaces'
    // intersection line next to the edge.
    for (int i = 1; i <= m_edgeFaces.Extent(); ++i) {
      const TopoDS_Edge edge = TopoDS::Edge(m_edgeFaces.FindKey(i));
      if (BRep_Tool::Degenerated(edge) || kind(edge) != Kind::Between) {
        continue;
      }
      const std::vector<TopoDS_Face> faces = faces_of(m_edgeFaces, edge);
      if (faces.size() != 2) {
        throw std::runtime_error("an edge of the moved faces does not lie between two faces");
      }
      double first = 0.0, last = 0.0;
      const occ::handle<Geom_Curve> old = global_curve(edge, first, last);
      const gp_Pnt middle = old->Value(0.5 * (first + last));
      GeomAPI_IntSS intersection(surface(faces[0]), surface(faces[1]), kNear);
      if (!intersection.IsDone() || intersection.NbLines() == 0) {
        throw std::runtime_error("a moved face no longer meets its neighbour");
      }
      occ::handle<Geom_Curve> best;
      double distance = std::numeric_limits<double>::infinity();
      for (int k = 1; k <= intersection.NbLines(); ++k) {
        const occ::handle<Geom_Curve> line = intersection.Line(k);
        GeomAPI_ProjectPointOnCurve projection(middle, line);
        if (projection.NbPoints() > 0 && projection.LowerDistance() < distance) {
          distance = projection.LowerDistance();
          best = line;
        }
      }
      if (best.IsNull()) {
        throw std::runtime_error("a moved face no longer meets its neighbour");
      }
      if (const auto trimmed = occ::down_cast<Geom_TrimmedCurve>(best)) {
        best = trimmed->BasisCurve();
      }
      // The new curve runs the old one's way.
      gp_Pnt p;
      gp_Vec was;
      old->D1(0.5 * (first + last), p, was);
      GeomAPI_ProjectPointOnCurve on(middle, best);
      gp_Vec now;
      best->D1(on.LowerDistanceParameter(), p, now);
      if (was.Dot(now) < 0.0) {
        best = best->Reversed();
      }
      Edge data;
      data.curve = best;
      m_between.Bind(edge, data);
    }
    // The vertices that move, and the ranges of the edges between.
    for (int i = 1; i <= m_vertexEdges.Extent(); ++i) {
      const TopoDS_Vertex vertex = TopoDS::Vertex(m_vertexEdges.FindKey(i));
      if (const std::optional<gp_Pnt> p = new_point(vertex, m_vertexEdges(i))) {
        m_points.Bind(vertex, *p);
      }
    }
    for (NCollection_DataMap<TopoDS_Shape, Edge, TopTools_ShapeMapHasher>::Iterator it(m_between); it.More();
         it.Next()) {
      const TopoDS_Edge edge = TopoDS::Edge(it.Key());
      Edge& data = m_between.ChangeFind(edge);
      TopoDS_Vertex v1, v2;
      TopExp::Vertices(edge, v1, v2);
      double first = 0.0, last = 0.0;
      global_curve(edge, first, last);
      const double tolerance = std::max(10.0 * BRep_Tool::Tolerance(edge), 1.0e-4);
      const std::optional<double> w1 = parameter_on(data.curve, point(v1), tolerance);
      const std::optional<double> w2 = parameter_on(data.curve, point(v2), tolerance);
      if (!w1 || !w2) {
        throw std::runtime_error("an edge of a moved face does not reach its vertices");
      }
      data.first = *w1;
      data.last = *w2;
      if (data.curve->IsPeriodic()) {
        const double period = data.curve->Period();
        while (data.last <= data.first + Precision::PConfusion()) {
          data.last += period;
        }
        // (A closed edge goes once round; an open one less than once.)
        if (v1.IsSame(v2)) {
          data.last = data.first + period;
        }
      } else if (data.last <= data.first) {
        throw std::runtime_error("an edge of a moved face would turn round");
      }
    }
  }

  bool NewSurface(const TopoDS_Face& F, occ::handle<Geom_Surface>& S, TopLoc_Location& L, double& Tol,
                  bool& RevWires, bool& RevFace) override {
    if (!m_moved.Contains(F)) {
      return false;
    }
    S = surface(F);
    L = TopLoc_Location();
    Tol = BRep_Tool::Tolerance(F);
    RevWires = false;
    RevFace = false;
    return true;
  }

  bool NewCurve(const TopoDS_Edge& E, occ::handle<Geom_Curve>& C, TopLoc_Location& L, double& Tol) override {
    if (BRep_Tool::Degenerated(E)) {
      return false;
    }
    switch (kind(E)) {
    case Kind::Moved: {
      double first = 0.0, last = 0.0;
      C = occ::down_cast<Geom_Curve>(global_curve(E, first, last)->Transformed(m_motion));
      break;
    }
    case Kind::Between:
      C = m_between.Find(E).curve;
      break;
    case Kind::Kept:
      return false;
    }
    L = TopLoc_Location();
    Tol = BRep_Tool::Tolerance(E);
    return true;
  }

  bool NewPoint(const TopoDS_Vertex& V, gp_Pnt& P, double& Tol) override {
    const gp_Pnt* p = m_points.Seek(V);
    if (p == nullptr) {
      return false;
    }
    P = *p;
    Tol = BRep_Tool::Tolerance(V);
    return true;
  }

  bool NewCurve2d(const TopoDS_Edge& E, const TopoDS_Face& F, const TopoDS_Edge&, const TopoDS_Face&,
                  occ::handle<Geom2d_Curve>& C, double& Tol) override {
    const Edge* data = m_between.Seek(E);
    if (data == nullptr) {
      if (!m_moved.Contains(F)) {
        return false;
      }
      // A moved face keeps its parametrization, and so the curves of its
      // edges that move with it (each side's of a seam, by the edge's
      // orientation in it), now on its new surface.
      double f = 0.0, l = 0.0;
      const occ::handle<Geom2d_Curve> old = BRep_Tool::CurveOnSurface(E, F, f, l);
      if (old.IsNull()) {
        return false;
      }
      C = occ::down_cast<Geom2d_Curve>(old->Copy());
      Tol = BRep_Tool::Tolerance(E);
      return true;
    }
    const occ::handle<Geom_Surface> S = surface(F);
    double tolerance = kNear;
    C = GeomProjLib::Curve2d(data->curve, data->first, data->last, S, tolerance);
    if (C.IsNull()) {
      throw std::runtime_error("an edge of a moved face has no curve on its face");
    }
    // Into the period of the old p-curve.
    double f = 0.0, l = 0.0;
    const occ::handle<Geom2d_Curve> old = BRep_Tool::CurveOnSurface(E, F, f, l);
    if (!old.IsNull()) {
      const gp_Pnt2d was = old->Value(0.5 * (f + l));
      const gp_Pnt2d now = C->Value(0.5 * (data->first + data->last));
      gp_Vec2d shift(0.0, 0.0);
      if (S->IsUPeriodic()) {
        shift.SetX(-std::round((now.X() - was.X()) / S->UPeriod()) * S->UPeriod());
      }
      if (S->IsVPeriodic()) {
        shift.SetY(-std::round((now.Y() - was.Y()) / S->VPeriod()) * S->VPeriod());
      }
      if (shift.Magnitude() > 0.0) {
        C->Translate(shift);
      }
    }
    Tol = std::max(BRep_Tool::Tolerance(E), tolerance);
    return true;
  }

  bool NewParameter(const TopoDS_Vertex& V, const TopoDS_Edge& E, double& P, double& Tol) override {
    if (BRep_Tool::Degenerated(E)) {
      return false;
    }
    if (const Edge* data = m_between.Seek(E)) {
      TopoDS_Vertex v1, v2;
      TopExp::Vertices(E, v1, v2);
      // (A closed edge: its vertex at the start as its own curve holds it,
      // forward in the edge taken forward.)
      TopAbs_Orientation own = V.Orientation();
      if (E.Orientation() == TopAbs_REVERSED) {
        own = TopAbs::Reverse(own);
      }
      const bool atStart = V.IsSame(v1) && (!v1.IsSame(v2) || own == TopAbs_FORWARD);
      P = atStart ? data->first : data->last;
      Tol = BRep_Tool::Tolerance(V);
      return true;
    }
    const gp_Pnt* p = m_points.Seek(V);
    if (p == nullptr) {
      return false;
    }
    // The vertex moves along the edge's curve (moved with the edge or not).
    double first = 0.0, last = 0.0;
    occ::handle<Geom_Curve> curve = global_curve(E, first, last);
    if (kind(E) == Kind::Moved) {
      curve = occ::down_cast<Geom_Curve>(curve->Transformed(m_motion));
    }
    const double old = BRep_Tool::Parameter(V, E);
    const std::optional<double> w =
        parameter_on(curve, *p, std::max(10.0 * BRep_Tool::Tolerance(E), 1.0e-4), old);
    if (!w) {
      throw std::runtime_error("a vertex of a moved face leaves its edge");
    }
    P = *w;
    Tol = BRep_Tool::Tolerance(V);
    return true;
  }

  GeomAbs_Shape Continuity(const TopoDS_Edge& E, const TopoDS_Face& F1, const TopoDS_Face& F2, const TopoDS_Edge&,
                           const TopoDS_Face&, const TopoDS_Face&) override {
    return BRep_Tool::Continuity(E, F1, F2);
  }

private:
  enum class Kind { Moved, Between, Kept };

  // Whether an edge's faces all move, some, or none.
  Kind kind(const TopoDS_Edge& edge) const {
    int moved = 0, kept = 0;
    for (const TopoDS_Face& face : faces_of(m_edgeFaces, edge)) {
      (m_moved.Contains(face) ? moved : kept) += 1;
    }
    return kept == 0 ? Kind::Moved : (moved == 0 ? Kind::Kept : Kind::Between);
  }

  // A face's surface (global), moved when the face moves.
  occ::handle<Geom_Surface> surface(const TopoDS_Face& face) const {
    const occ::handle<Geom_Surface> s = global_surface(face);
    return m_moved.Contains(face) ? occ::down_cast<Geom_Surface>(s->Transformed(m_motion)) : s;
  }

  // A vertex's point, moved or not.
  gp_Pnt point(const TopoDS_Vertex& vertex) const {
    const gp_Pnt* p = m_points.Seek(vertex);
    return p != nullptr ? *p : BRep_Tool::Pnt(vertex);
  }

  // Where a vertex goes (none: it stays): with its faces where they all
  // move; on an edge that stays, where it meets a moved face; on a moved
  // edge, where it meets a face that stays; else where an edge between
  // meets another face of the vertex.
  std::optional<gp_Pnt> new_point(const TopoDS_Vertex& vertex, const NCollection_List<TopoDS_Shape>& edges) const {
    std::vector<TopoDS_Face> faces;
    for (const TopoDS_Shape& e : edges) {
      for (const TopoDS_Face& face : faces_of(m_edgeFaces, e)) {
        if (std::none_of(faces.begin(), faces.end(), [&](const TopoDS_Face& f) { return f.IsSame(face); })) {
          faces.push_back(face);
        }
      }
    }
    const gp_Pnt old = BRep_Tool::Pnt(vertex);
    const auto movedFace = [&](bool want) -> std::optional<TopoDS_Face> {
      for (const TopoDS_Face& face : faces) {
        if (m_moved.Contains(face) == want) {
          return face;
        }
      }
      return std::nullopt;
    };
    const std::optional<TopoDS_Face> aMoved = movedFace(true);
    const std::optional<TopoDS_Face> aKept = movedFace(false);
    if (!aMoved) {
      return std::nullopt;
    }
    if (!aKept) {
      return old.Transformed(m_motion);
    }
    const double reach = 1.0e3;
    for (const Kind wanted : {Kind::Kept, Kind::Moved}) {
      for (const TopoDS_Shape& e : edges) {
        const TopoDS_Edge edge = TopoDS::Edge(e);
        if (BRep_Tool::Degenerated(edge) || kind(edge) != wanted) {
          continue;
        }
        double first = 0.0, last = 0.0;
        occ::handle<Geom_Curve> curve = global_curve(edge, first, last);
        gp_Pnt near = old;
        occ::handle<Geom_Surface> other = surface(*aMoved);
        if (wanted == Kind::Moved) {
          curve = occ::down_cast<Geom_Curve>(curve->Transformed(m_motion));
          near = old.Transformed(m_motion);
          other = surface(*aKept);
        }
        if (const std::optional<gp_Pnt> p = meet(curve, other, near); p && p->Distance(near) < reach) {
          return p;
        }
      }
    }
    for (const TopoDS_Shape& e : edges) {
      const Edge* data = m_between.Seek(e);
      if (data == nullptr) {
        continue;
      }
      const std::vector<TopoDS_Face> own = faces_of(m_edgeFaces, e);
      for (const TopoDS_Face& face : faces) {
        if (std::any_of(own.begin(), own.end(), [&](const TopoDS_Face& f) { return f.IsSame(face); })) {
          continue;
        }
        if (const std::optional<gp_Pnt> p = meet(data->curve, surface(face), old); p && p->Distance(old) < reach) {
          return p;
        }
      }
    }
    throw std::runtime_error("a vertex of the moved faces has no place on the moved surfaces");
  }

  ShapeSet m_moved;
  gp_Trsf m_motion;
  Ancestors m_edgeFaces;
  Ancestors m_vertexEdges;
  NCollection_DataMap<TopoDS_Shape, Edge, TopTools_ShapeMapHasher> m_between;
  NCollection_DataMap<TopoDS_Shape, gp_Pnt, TopTools_ShapeMapHasher> m_points;
};

// Whether the motion is rigid: its rotation part orthonormal, no scale.
// (A matrix given by its values is a compound transformation in OCCT's
// terms, also when it is a rotation and a translation.)
bool rigid(const gp_Trsf& motion) {
  return std::abs(motion.ScaleFactor() - 1.0) < 1.0e-9;
}

// Whether the motion moves nothing (within 1e-12 of the identity).
bool still(const gp_Trsf& motion) {
  if (motion.TranslationPart().Modulus() > 1.0e-12) {
    return false;
  }
  const gp_Mat m = motion.VectorialPart();
  for (int r = 1; r <= 3; ++r) {
    for (int c = 1; c <= 3; ++c) {
      if (std::abs(m.Value(r, c) - (r == c ? 1.0 : 0.0)) > 1.0e-12) {
        return false;
      }
    }
  }
  return true;
}

} // namespace

ShapePtr move_faces_rigidly(const std::string& feature, const Shape& body,
                            const std::vector<std::string>& faces, const gp_Trsf& motion) {
  (void)feature;
  if (!rigid(motion)) {
    throw std::invalid_argument("the faces' motion must be rigid");
  }
  if (still(motion)) {
    throw std::invalid_argument("the faces' motion moves nothing");
  }
  detail::require_surfaces(body.occt(), "move faces");
  const std::vector<int> chosen = detail::resolve_faces(body, faces, "faces to move");
  return detail::run("move faces", [&] {
    // (The faces and edges that do not change are shared with the result,
    // whose tolerances SameParameter may widen: a copy of the input's own.)
    const ShapePtr copy = detail::working_copy(body);
    ShapeSet moved;
    for (const int f : chosen) {
      moved.Add(copy->face(f));
    }
    const occ::handle<MovedFaces> modification = new MovedFaces(copy->occt(), moved, motion);
    BRepTools_Modifier modifier(copy->occt(), modification);
    if (!modifier.IsDone()) {
      throw std::runtime_error("the neighbouring faces cannot follow the moved faces");
    }
    const TopoDS_Shape result = modifier.ModifiedShape(copy->occt());
    BRepLib::SameParameter(result, kNear, true);
    if (!BRepCheck_Analyzer(result, true, true).IsValid()) {
      try {
        detail::require_valid(result, "moving the faces");
      } catch (const std::exception& error) {
        throw std::runtime_error(std::string("the neighbouring faces cannot follow the moved faces: ") +
                                 error.what());
      }
    }
    GProp_GProps props;
    BRepGProp::VolumeProperties(result, props);
    if (!(props.Mass() > 0.0)) {
      throw std::runtime_error("moving the faces leaves no body");
    }
    std::vector<Shape::NamedFace> named;
    named.reserve(static_cast<std::size_t>(body.face_count()));
    for (int i = 0; i < body.face_count(); ++i) {
      named.push_back({modifier.ModifiedShape(copy->face(i)), body.face_names(i)});
    }
    auto out = std::make_shared<Shape>(result, named);
    out->set_checked_valid(true);
    return ShapePtr(out);
  });
}

} // namespace mitcad::geometry
