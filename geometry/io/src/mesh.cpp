// SPDX-License-Identifier: MIT
#include "mitcad/io/mesh.hpp"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <limits>
#include <map>
#include <mutex>
#include <unordered_map>
#include <utility>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepMesh_Context.hxx>
#include <BRepMesh_Deflection.hxx>
#include <BRepMesh_DiscretAlgoFactory.hxx>
#include <BRepMesh_DiscretFactory.hxx>
#include <BRepMesh_EdgeDiscret.hxx>
#include <BRepMesh_DelabellaMeshAlgoFactory.hxx>
#include <BRepMesh_FaceDiscret.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepMesh_MeshAlgoFactory.hxx>
#include <BRepMesh_ShapeTool.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_CurveRepresentation.hxx>
#include <BRep_TEdge.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom_Curve.hxx>
#include <Geom_Plane.hxx>
#include <Geom_Surface.hxx>
#include <IMeshData_Curve.hxx>
#include <IMeshData_Edge.hxx>
#include <IMeshData_Face.hxx>
#include <IMeshData_Model.hxx>
#include <IMeshData_PCurve.hxx>
#include <IMeshData_Status.hxx>
#include <IMeshData_Wire.hxx>
#include <IMeshTools_CurveTessellator.hxx>
#include <IMeshTools_MeshAlgo.hxx>
#include <IMeshTools_MeshAlgoFactory.hxx>
#include <IMeshTools_Parameters.hxx>
#include <OSD_Environment.hxx>
#include <TCollection_AsciiString.hxx>
#include <Message_ProgressScope.hxx>
#include <OSD_Parallel.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Precision.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Elips.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>
#include <gp_XY.hxx>

#include "mitcad/io/body.hpp"

namespace mitcad::io {

namespace {
constexpr double kPi = 3.14159265358979323846;
constexpr double kDegree = kPi / 180.0;
}

MeshOptions MeshOptions::from(Refinement refinement) {
  switch (refinement) {
  case Refinement::Low:
    return MeshOptions{0.1, 30.0 * kDegree, false, 5000, false, false};
  case Refinement::Medium:
    return MeshOptions{0.03, 15.0 * kDegree, false, 20000, false, false};
  case Refinement::High:
    return MeshOptions{0.01, 8.0 * kDegree, false, 50000, false, false};
  }
  return MeshOptions{};
}

namespace {

// How many triangles BRepMesh gives a curved face, per unit of the
// integral of the square root of the Gaussian curvature over it, at a
// deviation of 1 mm (inversely proportional to the deviation). Its grid
// on a sphere has steps of 0.7 times the arc whose sag is the deviation,
// about 5 R / d triangles on a hemisphere, whose integral is 2 pi R.
constexpr double kTrianglesPerCurvature = 0.8;
// The integral is at most about this many times the diagonal of the face's
// box unless the face is wavy (a whole sphere: 3.6): smaller faces cannot
// need a larger deviation and are not integrated.
constexpr double kCurvaturePerDiagonal = 4.0;

// Gauss-Legendre points and weights on [-1, 1], one half (symmetric).
constexpr double kGaussX[4] = {0.1834346424956498, 0.5255324099163290, 0.7966664774136267,
                               0.9602898564975363};
constexpr double kGaussW[4] = {0.3626837833783620, 0.3137066458778873, 0.2223810344533745,
                               0.1012285362903763};

// The integral of sqrt(|K|) dA over the face's parameter range (8 x 8
// Gauss points; the trimmed part counts too, which overestimates it).
// Zero for the surfaces whose Gaussian curvature is zero.
double curvature_integral(const TopoDS_Face& face) {
  try {
    const BRepAdaptor_Surface surface(face, false);
    switch (surface.GetType()) {
    case GeomAbs_Plane:
    case GeomAbs_Cylinder:
    case GeomAbs_Cone:
    case GeomAbs_SurfaceOfExtrusion:
      return 0.0;
    default:
      break;
    }
    double u1 = 0.0;
    double u2 = 0.0;
    double v1 = 0.0;
    double v2 = 0.0;
    BRepTools::UVBounds(face, u1, u2, v1, v2);
    if (!(u2 > u1) || !(v2 > v1) || Precision::IsInfinite(u2 - u1) || Precision::IsInfinite(v2 - v1)) {
      return 0.0;
    }
    const double mu = 0.5 * (u1 + u2);
    const double hu = 0.5 * (u2 - u1);
    const double mv = 0.5 * (v1 + v2);
    const double hv = 0.5 * (v2 - v1);
    double sum = 0.0;
    for (int i = 0; i < 8; ++i) {
      const double xu = (i < 4 ? -1.0 : 1.0) * kGaussX[i % 4];
      const double wu = kGaussW[i % 4];
      for (int j = 0; j < 8; ++j) {
        const double xv = (j < 4 ? -1.0 : 1.0) * kGaussX[j % 4];
        const double wv = kGaussW[j % 4];
        gp_Pnt p;
        gp_Vec du;
        gp_Vec dv;
        gp_Vec duu;
        gp_Vec dvv;
        gp_Vec duv;
        surface.D2(mu + hu * xu, mv + hv * xv, p, du, dv, duu, dvv, duv);
        const gp_Vec normal = du.Crossed(dv);
        const double area = normal.Magnitude(); // sqrt(EG - F^2)
        if (!(area > 1e-12)) {
          continue; // a pole
        }
        const gp_Vec unit = normal / area;
        const double k = (duu.Dot(unit) * dvv.Dot(unit) - duv.Dot(unit) * duv.Dot(unit)) / (area * area);
        sum += wu * wv * std::sqrt(std::abs(k)) * area;
      }
    }
    return std::isfinite(sum) ? sum * hu * hv : 0.0;
  } catch (const Standard_Failure&) {
    return 0.0;
  }
}

// How many triangles of the face bound make one straight piece of a turn
// round a cylinder or a cone (mitcad#438): the view's 20 000 give the
// deviation of a turn in 500 pieces, 2e-5 of the radius, which a circle
// filling a 4K screen shows as less than a tenth of a pixel (BRepMesh cuts
// an edge at half its deviation: about 700 points a turn). At the view's
// 0.01 mm a cylinder of metres got thousands (2 600 round a radius of
// 6.8 m), and its edges' points are its triangles' corners.
constexpr double kTrianglesPerTurnPiece = 40.0;

// The deviation of a circle of `radius` cut into a turn's pieces for the
// bound: 0 for a radius of 0, infinite for an infinite one (a line).
double turn_deviation(double radius, std::size_t face_triangles) {
  const double pieces = std::max(8.0, static_cast<double>(face_triangles) / kTrianglesPerTurnPiece);
  return std::abs(radius) * (1.0 - std::cos(kPi / pieces));
}

// The radius of a cylinder face, the largest radius of a cone face over its
// parameter range (BRepMesh's cone splitter steps round it at that radius);
// 0 for other faces.
double round_radius(const TopoDS_Face& face) {
  const BRepAdaptor_Surface surface(face, false);
  switch (surface.GetType()) {
  case GeomAbs_Cylinder:
    return surface.Cylinder().Radius();
  case GeomAbs_Cone: {
    double u1 = 0.0;
    double u2 = 0.0;
    double v1 = 0.0;
    double v2 = 0.0;
    BRepTools::UVBounds(face, u1, u2, v1, v2);
    if (Precision::IsInfinite(v1) || Precision::IsInfinite(v2)) {
      return 0.0;
    }
    const gp_Cone cone = surface.Cone();
    const double grows = std::sin(cone.SemiAngle());
    return std::max(std::abs(cone.RefRadius() + v1 * grows), std::abs(cone.RefRadius() + v2 * grows));
  }
  default:
    return 0.0;
  }
}

// The smallest radius of curvature of an edge's curve: its radius for a
// circle, infinite for a line, sampled for other curves; 0 when it cannot
// be told.
double edge_radius(const TopoDS_Edge& edge) {
  try {
    const BRepAdaptor_Curve curve(edge);
    switch (curve.GetType()) {
    case GeomAbs_Line:
      return std::numeric_limits<double>::infinity();
    case GeomAbs_Circle:
      return curve.Circle().Radius();
    case GeomAbs_Ellipse: {
      const gp_Elips ellipse = curve.Ellipse();
      return ellipse.MajorRadius() > 0.0
                 ? ellipse.MinorRadius() * ellipse.MinorRadius() / ellipse.MajorRadius()
                 : 0.0;
    }
    default:
      break;
    }
    const double first = curve.FirstParameter();
    const double last = curve.LastParameter();
    if (Precision::IsInfinite(first) || Precision::IsInfinite(last) || !(last > first)) {
      return 0.0;
    }
    // |C' x C''| / |C'|^3 at 17 points along it.
    double curvature = 0.0;
    for (int i = 0; i <= 16; ++i) {
      gp_Pnt point;
      gp_Vec d1;
      gp_Vec d2;
      curve.D2(first + (last - first) * i / 16.0, point, d1, d2);
      const double speed = d1.Magnitude();
      if (!(speed > 1e-12)) {
        return 0.0;
      }
      curvature = std::max(curvature, d1.Crossed(d2).Magnitude() / (speed * speed * speed));
    }
    return curvature > 0.0 ? 1.0 / curvature : std::numeric_limits<double>::infinity();
  } catch (const Standard_Failure&) {
    return 0.0;
  }
}

// Discretizes a model's edge again at `deviation`, as BRepMesh_EdgeDiscret
// does (its own deviation being the parameters'): the finest polygon a
// triangulation of one of its faces has for it, when one meets the
// deviation, else a new tessellation of its curve.
void rediscretize(const IMeshData::IEdgeHandle& edge, double deviation, const IMeshTools_Parameters& parameters) {
  edge->SetDeflection(deviation);
  edge->UnsetStatus(IMeshData_Outdated);
  edge->Clear(false);
  double finest = std::numeric_limits<double>::max();
  int found = -1;
  for (int i = 0; i < edge->PCurvesNb(); ++i) {
    const TopoDS_Face& face = edge->GetPCurve(i)->GetFace()->GetFace();
    TopLoc_Location location;
    const occ::handle<Poly_Triangulation>& triangulation = BRep_Tool::Triangulation(face, location);
    if (triangulation.IsNull()) {
      continue;
    }
    const occ::handle<Poly_PolygonOnTriangulation>& polygon =
        BRep_Tool::PolygonOnTriangulation(edge->GetEdge(), triangulation, location);
    if (polygon.IsNull()) {
      continue;
    }
    if (polygon->HasParameters() &&
        BRepMesh_Deflection::IsConsistent(polygon->Deflection(), deviation, parameters.AllowQualityDecrease)) {
      if (polygon->Deflection() < finest) {
        finest = polygon->Deflection();
        found = i;
      }
    } else {
      edge->SetStatus(IMeshData_Outdated); // its faces are meshed again
    }
  }
  occ::handle<IMeshTools_CurveTessellator> tessellator;
  if (found >= 0) {
    edge->SetDeflection(finest);
    tessellator = BRepMesh_EdgeDiscret::CreateEdgeTessellationExtractor(
        edge, IMeshData::IFaceHandle(edge->GetPCurve(found)->GetFace()));
  } else {
    const IMeshData::IPCurveHandle& pcurve = edge->GetPCurve(0);
    tessellator = BRepMesh_EdgeDiscret::CreateEdgeTessellator(
        edge, pcurve->GetOrientation(), IMeshData::IFaceHandle(pcurve->GetFace()), parameters);
  }
  BRepMesh_EdgeDiscret::Tessellate3d(edge, tessellator, true);
  BRepMesh_EdgeDiscret::Tessellate2d(edge, true);
}

// The edges of large cylinders and cones at a deviation that keeps a turn
// round them within the bound's pieces (turn_deviation, mitcad#438): their
// faces' triangles are mostly their edges' points (BRepMesh puts no points
// inside a cylinder), so an edge takes the smallest floor of the faces it
// bounds (face_deviation_floor), planes having no say, and no more than its
// own curve's turn deviation; one that bounds no cylinder or cone, or a
// face whose floor is none, keeps BRepMesh's. In parallel when the meshing
// is: each task changes only its own edge's discretization.
void coarsen_round_edges(const occ::handle<IMeshData_Model>& model, const IMeshTools_Parameters& parameters,
                         std::size_t face_triangles) {
  if (face_triangles == 0 || model.IsNull() || parameters.Relative) {
    return;
  }
  OSD_Parallel::For(
      0, model->EdgesNb(),
      [&model, &parameters, face_triangles](int i) {
        const IMeshData::IEdgeHandle& edge = model->GetEdge(i);
        if (edge->IsFree() || edge->GetDegenerated() || edge->IsSet(IMeshData_Failure)) {
          return;
        }
        const double own = edge->GetDeflection();
        try {
          bool round = false;
          double floor = std::numeric_limits<double>::infinity();
          for (int p = 0; p < edge->PCurvesNb() && floor > own; ++p) {
            const IMeshData::IFacePtr& face = edge->GetPCurve(p)->GetFace();
            const GeomAbs_SurfaceType type = face->GetSurface()->GetType();
            if (type == GeomAbs_Plane) {
              continue;
            }
            round = round || type == GeomAbs_Cylinder || type == GeomAbs_Cone;
            floor = std::min(floor, face_deviation_floor(face->GetFace(), parameters.Deflection, face_triangles));
          }
          if (!round || !(floor > own) || !std::isfinite(floor)) {
            return;
          }
          floor = std::min(floor, turn_deviation(edge_radius(edge->GetEdge()), face_triangles));
          if (!(floor > own)) {
            return;
          }
          rediscretize(edge, floor, parameters);
        } catch (const Standard_Failure&) {
          // As BRepMesh discretized it.
          try {
            rediscretize(edge, own, parameters);
          } catch (const Standard_Failure&) {
            edge->SetStatus(IMeshData_Failure);
          }
        }
      },
      !parameters.InParallel || model->EdgesNb() < 8);
}

using Clock = std::chrono::steady_clock;

double milliseconds_since(Clock::time_point start) {
  return std::chrono::duration<double, std::milli>(Clock::now() - start).count();
}

} // namespace

const char* surface_name(GeomAbs_SurfaceType type) {
  switch (type) {
  case GeomAbs_Plane:
    return "plane";
  case GeomAbs_Cylinder:
    return "cylinder";
  case GeomAbs_Cone:
    return "cone";
  case GeomAbs_Sphere:
    return "sphere";
  case GeomAbs_Torus:
    return "torus";
  case GeomAbs_BezierSurface:
    return "bezier";
  case GeomAbs_BSplineSurface:
    return "bspline";
  case GeomAbs_SurfaceOfRevolution:
    return "revolution";
  case GeomAbs_SurfaceOfExtrusion:
    return "extrusion";
  case GeomAbs_OffsetSurface:
    return "offset";
  default:
    return "other";
  }
}

namespace {

// The faces' triangulation times of one meshing, from BRepMesh's threads.
struct FaceTimes {
  std::mutex mutex;
  std::vector<MeshReport::FaceTime> faces;
};

// BRepMesh's algorithm for a face, timed (MeshReport::slowest).
class TimedAlgo : public IMeshTools_MeshAlgo {
public:
  TimedAlgo(occ::handle<IMeshTools_MeshAlgo> inner, FaceTimes& times) : m_inner(std::move(inner)), m_times(times) {}

  void Perform(const IMeshData::IFaceHandle& face, const IMeshTools_Parameters& parameters,
               const Message_ProgressRange& range) override {
    const Clock::time_point start = Clock::now();
    try {
      m_inner->Perform(face, parameters, range);
    } catch (...) {
      record(face, start); // BRepMesh marks the face failed
      throw;
    }
    record(face, start);
  }

private:
  void record(const IMeshData::IFaceHandle& face, Clock::time_point start) {
    MeshReport::FaceTime time;
    time.milliseconds = milliseconds_since(start);
    time.face = face->GetFace();
    time.surface = surface_name(face->GetSurface()->GetType());
    time.wires = face->WiresNb();
    for (int w = 0; w < face->WiresNb(); ++w) {
      const IMeshData::IWireHandle& wire = face->GetWire(w);
      for (int e = 0; e < wire->EdgesNb(); ++e) {
        time.points += std::max(0, wire->GetEdge(e)->GetCurve()->ParametersNb() - 1);
      }
    }
    const std::lock_guard<std::mutex> lock(m_times.mutex);
    m_times.faces.push_back(std::move(time));
  }

  occ::handle<IMeshTools_MeshAlgo> m_inner;
  FaceTimes& m_times;
};

// The area check of MeshOptions::check_planes (plane_area_fits).
constexpr double kPlaneAreaRelative = 1e-3;
constexpr double kPlaneAreaAbsolute = 1e-3; // mm²

// The area of polygons in a plane's parameters, wire by wire: each wire's
// by the shoelace formula over its edges' polylines (relative to the
// wire's first point, so that a plane far from its origin keeps its
// digits), the outer wire taken as the largest and the others as its
// holes. Orientations are not trusted: each wire's area counts as it is.
class LoopAreas {
public:
  void start_wire() {
    finish_wire();
    m_started = false;
    m_twice = 0.0;
  }

  // One directed segment of the wire.
  void add(const gp_XY& from, const gp_XY& to) {
    if (!m_started) {
      m_origin = from;
      m_started = true;
    }
    const gp_XY a = from - m_origin;
    const gp_XY b = to - m_origin;
    m_twice += a.X() * b.Y() - b.X() * a.Y();
    m_open = true;
  }

  double area() {
    finish_wire();
    double largest = 0.0;
    double sum = 0.0;
    for (const double wire : m_wires) {
      largest = std::max(largest, wire);
      sum += wire;
    }
    return std::max(0.0, largest - (sum - largest));
  }

private:
  void finish_wire() {
    if (m_open) {
      m_wires.push_back(0.5 * std::abs(m_twice));
      m_open = false;
    }
  }

  std::vector<double> m_wires;
  gp_XY m_origin;
  double m_twice = 0.0;
  bool m_started = false;
  bool m_open = false;
};

// The area (mm²) a plane's boundary discretization encloses, from the
// mesher's model of it: each wire's edges' points in the plane's
// parameters (lengths on a plane), each edge in the direction the mesher
// links it (its parameter curve's orientation, reversed or not), the
// wires the mesher leaves out (self-intersecting) left out here too.
double boundary_area(const IMeshData::IFaceHandle& face) {
  const IMeshData::IFacePtr self = face.get();
  LoopAreas loops;
  for (int w = 0; w < face->WiresNb(); ++w) {
    const IMeshData::IWireHandle& wire = face->GetWire(w);
    if (wire->IsSet(IMeshData_SelfIntersectingWire)) {
      continue;
    }
    loops.start_wire();
    for (int e = 0; e < wire->EdgesNb(); ++e) {
      const IMeshData::IEdgePtr& edge = wire->GetEdge(e);
      const IMeshData::IPCurveHandle& pcurve = edge->GetPCurve(self, wire->GetEdgeOrientation(e));
      const TopAbs_Orientation orientation = pcurve->GetOrientation();
      if (orientation != TopAbs_FORWARD && orientation != TopAbs_REVERSED) {
        continue; // an edge inside the face, not a boundary
      }
      const bool reversed = orientation == TopAbs_REVERSED;
      for (int i = 0; i + 1 < pcurve->ParametersNb(); ++i) {
        const gp_XY p = pcurve->GetPoint(i).XY();
        const gp_XY q = pcurve->GetPoint(i + 1).XY();
        if (reversed) {
          loops.add(q, p);
        } else {
          loops.add(p, q);
        }
      }
    }
  }
  return loops.area();
}

// The area (mm²) a triangulation covers, each triangle's counted however it
// turns (a fold adds to it): in its parameter nodes when it has them (a
// plane's are lengths), else in its nodes.
double covered_area(const Poly_Triangulation& triangulation) {
  double twice = 0.0;
  const bool uv = triangulation.HasUVNodes();
  for (int t = 1; t <= triangulation.NbTriangles(); ++t) {
    int a = 0;
    int b = 0;
    int c = 0;
    triangulation.Triangle(t).Get(a, b, c);
    if (uv) {
      const gp_XY pa = triangulation.UVNode(a).XY();
      const gp_XY ab = triangulation.UVNode(b).XY() - pa;
      const gp_XY ac = triangulation.UVNode(c).XY() - pa;
      twice += std::abs(ab.X() * ac.Y() - ab.Y() * ac.X());
    } else {
      const gp_Pnt pa = triangulation.Node(a);
      twice += gp_Vec(pa, triangulation.Node(b)).Crossed(gp_Vec(pa, triangulation.Node(c))).Magnitude();
    }
  }
  return 0.5 * twice;
}

// A plane's triangulation made by `first` (Delabella's), checked
// (MeshOptions::check_planes, mitcad#644): when it leaves the face without
// a new triangulation or marks it failed, or the triangulation's area does
// not fit the area its boundary encloses (plane_area_fits), the face is
// triangulated again by `again` (the default insertion), whose result is
// kept as it comes. Each such face counts in `redone`.
class CheckedPlaneAlgo : public IMeshTools_MeshAlgo {
public:
  CheckedPlaneAlgo(occ::handle<IMeshTools_MeshAlgo> first, occ::handle<IMeshTools_MeshAlgoFactory> again,
                   std::atomic<std::size_t>* redone)
      : m_first(std::move(first)), m_again(std::move(again)), m_redone(redone) {}

  void Perform(const IMeshData::IFaceHandle& face, const IMeshTools_Parameters& parameters,
               const Message_ProgressRange& range) override {
    const TopoDS_Face& shape = face->GetFace();
    TopLoc_Location location;
    const occ::handle<Poly_Triangulation> before = BRep_Tool::Triangulation(shape, location);
    bool fits = false;
    try {
      m_first->Perform(face, parameters, range);
      const occ::handle<Poly_Triangulation> made = BRep_Tool::Triangulation(shape, location);
      fits = !face->IsSet(IMeshData_Failure) && !made.IsNull() && made != before &&
             plane_area_fits(covered_area(*made), boundary_area(face));
    } catch (const Standard_Failure&) {
      fits = false;
    }
    if (fits) {
      return;
    }
    // The default's own start: no triangulation, nothing failed yet. A
    // stopped meshing leaves the face so, as a face it did not reach.
    BRepMesh_ShapeTool::NullifyFace(shape);
    face->UnsetStatus(IMeshData_Failure);
    if (range.UserBreak()) {
      face->SetStatus(IMeshData_UserBreak);
      return;
    }
    ++*m_redone;
    const occ::handle<IMeshTools_MeshAlgo> again = m_again->GetAlgo(GeomAbs_Plane, parameters);
    if (again.IsNull()) {
      face->SetStatus(IMeshData_Failure);
      return;
    }
    again->Perform(face, parameters, range);
  }

private:
  occ::handle<IMeshTools_MeshAlgo> m_first;
  occ::handle<IMeshTools_MeshAlgoFactory> m_again;
  std::atomic<std::size_t>* m_redone;
};

// use_fast_plane_algorithms_for_tests: null for Delabella's.
std::mutex g_fastPlanesMutex;
occ::handle<IMeshTools_MeshAlgoFactory> g_fastPlanesForTests;

occ::handle<IMeshTools_MeshAlgoFactory> fast_plane_algorithms() {
  const std::lock_guard<std::mutex> lock(g_fastPlanesMutex);
  if (!g_fastPlanesForTests.IsNull()) {
    return g_fastPlanesForTests;
  }
  return new BRepMesh_DelabellaMeshAlgoFactory();
}

// The faces' algorithms: BRepMesh_Context's default (CSF_MeshAlgo), with
// planes through Delabella's base triangulation when `fast_planes`
// (MeshOptions::fast_planes), checked and made again by the default when
// `check_planes` too (CheckedPlaneAlgo, counted in `redone`), each timed
// when `times` is given.
class FaceAlgoFactory : public IMeshTools_MeshAlgoFactory {
public:
  FaceAlgoFactory(bool fast_planes, bool check_planes, FaceTimes* times, std::atomic<std::size_t>* redone)
      : m_check(fast_planes && check_planes), m_times(times), m_redone(redone) {
    TCollection_AsciiString algorithm = OSD_Environment("CSF_MeshAlgo").Value();
    algorithm.LowerCase();
    if (algorithm == "delabella" || algorithm == "1") {
      m_default = new BRepMesh_DelabellaMeshAlgoFactory();
    } else {
      m_default = new BRepMesh_MeshAlgoFactory();
    }
    m_planes = fast_planes ? fast_plane_algorithms() : m_default;
  }

  occ::handle<IMeshTools_MeshAlgo> GetAlgo(const GeomAbs_SurfaceType type,
                                           const IMeshTools_Parameters& parameters) const override {
    occ::handle<IMeshTools_MeshAlgo> algorithm =
        (type == GeomAbs_Plane ? m_planes : m_default)->GetAlgo(type, parameters);
    if (!algorithm.IsNull() && m_check && type == GeomAbs_Plane) {
      algorithm = new CheckedPlaneAlgo(algorithm, m_default, m_redone);
    }
    if (algorithm.IsNull() || m_times == nullptr) {
      return algorithm;
    }
    return new TimedAlgo(algorithm, *m_times);
  }

private:
  occ::handle<IMeshTools_MeshAlgoFactory> m_default;
  occ::handle<IMeshTools_MeshAlgoFactory> m_planes;
  bool m_check;
  FaceTimes* m_times;
  std::atomic<std::size_t>* m_redone;
};

// BRepMesh's own context, with the edges of large cylinders and cones
// discretized at their faces' floor (coarsen_round_edges) and each face's
// deviation raised to its floor once the healer has computed the
// deviations from the edges' (BRepMesh's pre-processor then keeps a
// triangulation that meets it). The floors are computed over the faces in
// parallel when the meshing is (each task only reads its own face and sets
// its own deviation); a face whose floor cannot be computed keeps
// BRepMesh's deviation. Planes through Delabella with `fast_planes`,
// checked with `check_planes` (MeshOptions). With a report, each phase is
// timed and so is each face (MeshReport).
class BoundedContext : public BRepMesh_Context {
public:
  BoundedContext(std::size_t face_triangles, bool fast_planes, bool check_planes, MeshReport* report)
      : m_faceTriangles(face_triangles), m_report(report) {
    if (fast_planes || m_report != nullptr) {
      SetFaceDiscret(new BRepMesh_FaceDiscret(
          new FaceAlgoFactory(fast_planes, check_planes, m_report ? &m_times : nullptr, &m_redone)));
    }
  }

  bool BuildModel() override {
    const Clock::time_point start = Clock::now();
    const bool done = BRepMesh_Context::BuildModel();
    add(&MeshReport::build, start);
    return done;
  }

  bool DiscretizeEdges() override {
    const Clock::time_point start = Clock::now();
    if (!BRepMesh_Context::DiscretizeEdges()) {
      return false;
    }
    coarsen_round_edges(GetModel(), GetParameters(), m_faceTriangles);
    add(&MeshReport::edges, start);
    return true;
  }

  bool HealModel() override {
    const Clock::time_point start = Clock::now();
    if (!BRepMesh_Context::HealModel()) {
      return false;
    }
    const occ::handle<IMeshData_Model>& model = GetModel();
    if (m_faceTriangles == 0 || model.IsNull()) {
      add(&MeshReport::heal, start);
      return true;
    }
    const std::size_t limit = m_faceTriangles;
    OSD_Parallel::For(
        0, model->FacesNb(),
        [&model, limit](int i) {
          const IMeshData::IFaceHandle& face = model->GetFace(i);
          try {
            const double floor = face_deviation_floor(face->GetFace(), face->GetDeflection(), limit);
            if (floor > face->GetDeflection()) {
              face->SetDeflection(floor);
            }
          } catch (const Standard_Failure&) {
            // BRepMesh's own deviation.
          }
        },
        !GetParameters().InParallel || model->FacesNb() < 8);
    add(&MeshReport::heal, start);
    return true;
  }

  bool PreProcessModel() override {
    const Clock::time_point start = Clock::now();
    const bool done = BRepMesh_Context::PreProcessModel();
    add(&MeshReport::check, start);
    return done;
  }

  bool DiscretizeFaces(const Message_ProgressRange& range) override {
    const Clock::time_point start = Clock::now();
    const bool done = BRepMesh_Context::DiscretizeFaces(range);
    add(&MeshReport::triangulate, start);
    return done;
  }

  bool PostProcessModel() override {
    const Clock::time_point start = Clock::now();
    const bool done = BRepMesh_Context::PostProcessModel();
    add(&MeshReport::write, start);
    count();
    return done;
  }

private:
  void add(double MeshReport::*phase, Clock::time_point start) {
    if (m_report != nullptr) {
      m_report->*phase += milliseconds_since(start);
    }
  }

  // The faces kept, meshed and failed, by surface kind, and the slowest.
  void count() {
    const occ::handle<IMeshData_Model>& model = GetModel();
    if (m_report == nullptr || model.IsNull()) {
      return;
    }
    std::map<std::string, std::size_t> surfaces;
    for (int i = 0; i < model->FacesNb(); ++i) {
      const IMeshData::IFaceHandle& face = model->GetFace(i);
      ++m_report->faces;
      TopLoc_Location location;
      if (face->IsSet(IMeshData_Reused)) {
        ++m_report->kept;
      } else if (face->IsSet(IMeshData_Failure) || BRep_Tool::Triangulation(face->GetFace(), location).IsNull()) {
        ++m_report->failed;
      } else {
        ++m_report->meshed;
        ++surfaces[surface_name(face->GetSurface()->GetType())];
      }
    }
    for (const auto& [surface, count] : surfaces) {
      m_report->meshed_by_surface.emplace_back(surface, count);
    }
    m_report->planes_redone += m_redone.load();
    if (m_report->every_face) {
      m_report->every_face_time.insert(m_report->every_face_time.end(), m_times.faces.begin(), m_times.faces.end());
    }
    std::vector<MeshReport::FaceTime>& slowest = m_report->slowest;
    slowest.insert(slowest.end(), m_times.faces.begin(), m_times.faces.end());
    std::sort(slowest.begin(), slowest.end(), [](const MeshReport::FaceTime& a, const MeshReport::FaceTime& b) {
      return a.milliseconds > b.milliseconds;
    });
    if (slowest.size() > 5) {
      slowest.resize(5);
    }
  }

  std::size_t m_faceTriangles;
  MeshReport* m_report;
  FaceTimes m_times;
  std::atomic<std::size_t> m_redone{0}; // CheckedPlaneAlgo
};

// Faces the presentations' algorithm triangulated (presentation_faces_meshed).
std::atomic<std::size_t> g_presentationFaces{0};

// BRepMesh_IncrementalMesh with that context. A presentation's meshing
// (BoundedFactory) counts the faces it meshed and traces itself.
class BoundedMesh : public BRepMesh_IncrementalMesh {
public:
  explicit BoundedMesh(const MeshOptions& options, MeshReport* report = nullptr, bool presentation = false)
      : m_faceTriangles(options.face_triangles), m_fastPlanes(options.fast_planes),
        m_checkPlanes(options.check_planes), m_report(report),
        m_presentation(presentation) {}

  using BRepMesh_IncrementalMesh::Perform;
  void Perform(const Message_ProgressRange& range = Message_ProgressRange()) override {
    MeshReport own;
    MeshReport* report = m_presentation ? &own : m_report;
    const Clock::time_point start = Clock::now();
    BRepMesh_IncrementalMesh::Perform(new BoundedContext(m_faceTriangles, m_fastPlanes, m_checkPlanes, report),
                                      range);
    if (report != nullptr) {
      report->total += milliseconds_since(start);
    }
    if (m_presentation) {
      g_presentationFaces += own.meshed;
      if (mesh_tracing()) {
        std::fprintf(stderr, "mesh-trace: presentation %s\n", own.line().c_str());
      }
    }
  }

private:
  std::size_t m_faceTriangles;
  bool m_fastPlanes;
  bool m_checkPlanes;
  MeshReport* m_report;
  bool m_presentation;
};

// The presentations' meshing algorithm (use_bounded_presentation_meshes).
class BoundedFactory : public BRepMesh_DiscretAlgoFactory {
public:
  static constexpr const char* kName = "MitcadBounded";

  explicit BoundedFactory(std::size_t face_triangles)
      : BRepMesh_DiscretAlgoFactory(kName), m_faceTriangles(face_triangles) {}

  occ::handle<BRepMesh_DiscretRoot> CreateAlgorithm(const TopoDS_Shape& shape, double deviation,
                                                    double angle) override {
    const MeshOptions options{deviation, angle, false, m_faceTriangles, false, false};
    occ::handle<BoundedMesh> mesher = new BoundedMesh(options, nullptr, true);
    mesher->ChangeParameters().Deflection = deviation;
    mesher->ChangeParameters().Angle = angle;
    mesher->ChangeParameters().InParallel = BRepMesh_IncrementalMesh::IsParallelDefault();
    mesher->SetShape(shape);
    return mesher;
  }

private:
  std::size_t m_faceTriangles;
};

IMeshTools_Parameters parameters_of(const MeshOptions& options) {
  if (!(options.linear_deflection > 0.0) || !(options.angular_deflection > 0.0)) {
    throw Error("mesh deflections must be greater than zero");
  }
  IMeshTools_Parameters parameters;
  parameters.Deflection = options.linear_deflection;
  parameters.Angle = options.angular_deflection;
  parameters.Relative = options.relative;
  parameters.InParallel = true;
  return parameters;
}

} // namespace

bool mesh_tracing() {
  static const bool on = [] {
#ifdef _MSC_VER
    char* value = nullptr;
    std::size_t length = 0;
    if (_dupenv_s(&value, &length, "MITCAD_MESH_TRACE") != 0 || value == nullptr) {
      return false;
    }
    std::free(value);
    return true;
#else
    return std::getenv("MITCAD_MESH_TRACE") != nullptr;
#endif
  }();
  return on;
}

std::size_t presentation_faces_meshed() { return g_presentationFaces.load(); }

std::string MeshReport::line() const {
  char text[256];
  std::snprintf(text, sizeof text,
                "faces=%zu kept=%zu meshed=%zu failed=%zu build=%.1f edges=%.1f heal=%.1f check=%.1f "
                "triangulate=%.1f write=%.1f total=%.1f ms",
                faces, kept, meshed, failed, build, edges, heal, check, triangulate, write, total);
  std::string out = text;
  if (!meshed_by_surface.empty()) {
    out += "; meshed";
    for (std::size_t i = 0; i < meshed_by_surface.size(); ++i) {
      out += (i == 0 ? " " : ", ") + meshed_by_surface[i].first + " " + std::to_string(meshed_by_surface[i].second);
    }
  }
  if (!slowest.empty()) {
    out += "; slowest";
    for (std::size_t i = 0; i < slowest.size(); ++i) {
      std::snprintf(text, sizeof text, "%s %s wires=%d points=%d %.1f ms", i == 0 ? "" : ",",
                    slowest[i].surface.c_str(), slowest[i].wires, slowest[i].points, slowest[i].milliseconds);
      out += text;
    }
  }
  if (planes_redone > 0) {
    out += "; planes redone " + std::to_string(planes_redone);
  }
  return out;
}

bool plane_area_fits(double triangulated, double boundary) {
  return std::isfinite(triangulated) && std::isfinite(boundary) &&
         std::abs(triangulated - boundary) <= std::max(kPlaneAreaRelative * std::abs(boundary), kPlaneAreaAbsolute);
}

PlaneMeshArea plane_mesh_area(const TopoDS_Face& face) {
  PlaneMeshArea area;
  if (face.IsNull() || BRepAdaptor_Surface(face, false).GetType() != GeomAbs_Plane) {
    return area;
  }
  TopLoc_Location location;
  const occ::handle<Poly_Triangulation> triangulation = BRep_Tool::Triangulation(face, location);
  if (triangulation.IsNull() || !triangulation->HasUVNodes()) {
    return area;
  }
  // Each wire's edges in the face's orientation, as the mesher links them
  // (a reversed face's wires run the other way round, which the areas'
  // own signs do not mind).
  const TopoDS_Face forward = TopoDS::Face(face.Oriented(TopAbs_FORWARD));
  LoopAreas loops;
  for (TopExp_Explorer wires(forward, TopAbs_WIRE); wires.More(); wires.Next()) {
    loops.start_wire();
    for (TopExp_Explorer edges(wires.Current(), TopAbs_EDGE); edges.More(); edges.Next()) {
      const TopoDS_Edge& edge = TopoDS::Edge(edges.Current());
      if (edge.Orientation() != TopAbs_FORWARD && edge.Orientation() != TopAbs_REVERSED) {
        continue;
      }
      const occ::handle<Poly_PolygonOnTriangulation> polygon =
          BRep_Tool::PolygonOnTriangulation(edge, triangulation, location);
      if (polygon.IsNull()) {
        return PlaneMeshArea{};
      }
      const bool reversed = edge.Orientation() == TopAbs_REVERSED;
      for (int i = 1; i < polygon->NbNodes(); ++i) {
        const gp_XY p = triangulation->UVNode(polygon->Node(i)).XY();
        const gp_XY q = triangulation->UVNode(polygon->Node(i + 1)).XY();
        if (reversed) {
          loops.add(q, p);
        } else {
          loops.add(p, q);
        }
      }
    }
  }
  area.boundary = loops.area();
  area.triangles = covered_area(*triangulation);
  return area;
}

void use_fast_plane_algorithms_for_tests(const occ::handle<IMeshTools_MeshAlgoFactory>& factory) {
  const std::lock_guard<std::mutex> lock(g_fastPlanesMutex);
  g_fastPlanesForTests = factory;
}

double face_deviation_floor(const TopoDS_Face& face, double deviation, std::size_t face_triangles) {
  if (face_triangles == 0 || face.IsNull()) {
    return 0.0;
  }
  // A cylinder or a cone: a turn round it in the bound's pieces (mitcad#438),
  // whatever its size, so that the faces of one cylinder agree.
  try {
    const double radius = round_radius(face);
    if (radius > 0.0) {
      const double floor = turn_deviation(radius, face_triangles);
      return floor > deviation ? floor : 0.0;
    }
  } catch (const Standard_Failure&) {
    return 0.0;
  }
  const auto limit = static_cast<double>(face_triangles);
  Bnd_Box box;
  BRepBndLib::Add(face, box, true);
  if (box.IsVoid() || box.IsOpen()) {
    return 0.0;
  }
  if (kTrianglesPerCurvature * kCurvaturePerDiagonal * std::sqrt(box.SquareExtent()) / limit <= deviation) {
    return 0.0;
  }
  return kTrianglesPerCurvature * curvature_integral(face) / limit;
}

bool has_triangulation(const TopoDS_Shape& shape) {
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    TopLoc_Location location;
    if (BRep_Tool::Triangulation(TopoDS::Face(it.Current()), location).IsNull()) {
      return false;
    }
  }
  return true;
}

namespace {

// What makes a face's triangulation valid for another face
// (reuse_triangulations): its surface and each edge's geometry, compared
// by identity (the copies an operation makes share them), and the edges'
// end points by value.
struct EdgeKey {
  // Which edge it is in its face (not compared): an edge may bound a face
  // twice in other placements (a prism's side between its base's edge
  // and the same edge moved to the top), with a polygon for each.
  const TopoDS_TShape* tshape = nullptr;
  TopLoc_Location placement; // compared
  TopAbs_Orientation orientation = TopAbs_FORWARD;
  const Geom_Curve* curve = nullptr;
  TopLoc_Location location;
  double first = 0.0;
  double last = 0.0;
  const Geom2d_Curve* pcurve = nullptr;
  const Geom2d_Curve* pcurve2 = nullptr; // a seam's other
  gp_Pnt start;
  gp_Pnt end;
  bool degenerated = false;

  bool same(const EdgeKey& other) const {
    return orientation == other.orientation && placement.IsEqual(other.placement) && curve == other.curve &&
           location.IsEqual(other.location) &&
           first == other.first && last == other.last && pcurve == other.pcurve && pcurve2 == other.pcurve2 &&
           start.X() == other.start.X() && start.Y() == other.start.Y() && start.Z() == other.start.Z() &&
           end.X() == other.end.X() && end.Y() == other.end.Y() && end.Z() == other.end.Z() &&
           degenerated == other.degenerated;
  }
};

struct FaceKey {
  TopoDS_Face face; // located as explored
  const Geom_Surface* surface = nullptr;
  TopLoc_Location location;
  bool natural = false;
  std::vector<EdgeKey> edges; // in the face's order, each sub-shape once

  bool same(const FaceKey& other) const {
    if (surface != other.surface || !location.IsEqual(other.location) || natural != other.natural ||
        edges.size() != other.edges.size()) {
      return false;
    }
    for (std::size_t i = 0; i < edges.size(); ++i) {
      if (!edges[i].same(other.edges[i])) {
        return false;
      }
    }
    return true;
  }
};

FaceKey face_key(const TopoDS_Face& face) {
  FaceKey key;
  key.face = face;
  TopLoc_Location location;
  key.surface = BRep_Tool::Surface(face, location).get();
  key.location = location;
  key.natural = BRep_Tool::NaturalRestriction(face);
  for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(it.Current());
    if (std::any_of(key.edges.begin(), key.edges.end(), [&edge](const EdgeKey& seen) {
          return seen.tshape == edge.TShape().get() && seen.placement.IsEqual(edge.Location());
        })) {
      continue; // a seam's second use
    }
    EdgeKey e;
    e.tshape = edge.TShape().get();
    e.placement = edge.Location();
    e.orientation = edge.Orientation();
    TopLoc_Location curve_location;
    e.curve = BRep_Tool::Curve(edge, curve_location, e.first, e.last).get();
    e.location = curve_location;
    // The edge's parameter curve on the face; none on a plane, where it is
    // the curve's, compared already (OCCT makes one anew from it when the
    // edge stores none, and a copy may store it).
    const bool plane = key.surface != nullptr && key.surface->IsKind(STANDARD_TYPE(Geom_Plane));
    const auto stored = [&edge, &face, plane](TopAbs_Orientation orientation) -> const Geom2d_Curve* {
      if (plane) {
        return nullptr;
      }
      double f = 0.0;
      double l = 0.0;
      bool kept = false;
      const occ::handle<Geom2d_Curve> pcurve =
          BRep_Tool::CurveOnSurface(TopoDS::Edge(edge.Oriented(orientation)), face, f, l, &kept);
      return kept ? pcurve.get() : nullptr;
    };
    e.pcurve = stored(TopAbs_FORWARD);
    e.pcurve2 = BRep_Tool::IsClosed(edge, face) ? stored(TopAbs_REVERSED) : nullptr;
    TopoDS_Vertex v1;
    TopoDS_Vertex v2;
    TopExp::Vertices(edge, v1, v2);
    if (!v1.IsNull()) {
      e.start = BRep_Tool::Pnt(v1);
    }
    if (!v2.IsNull()) {
      e.end = BRep_Tool::Pnt(v2);
    }
    e.degenerated = BRep_Tool::Degenerated(edge);
    key.edges.push_back(e);
  }
  return key;
}

// The edge's polygons on the triangulation of the face (two for a seam),
// or false when it has none there.
bool polygons_of(const TopoDS_Edge& edge, const occ::handle<Poly_Triangulation>& triangulation,
                 const TopLoc_Location& location, occ::handle<Poly_PolygonOnTriangulation>& first,
                 occ::handle<Poly_PolygonOnTriangulation>& second) {
  const auto* tedge = static_cast<const BRep_TEdge*>(edge.TShape().get());
  const TopLoc_Location relative = location.Predivided(edge.Location());
  for (const occ::handle<BRep_CurveRepresentation>& representation : tedge->Curves()) {
    if (representation->IsPolygonOnTriangulation(triangulation, relative)) {
      first = representation->PolygonOnTriangulation();
      if (representation->IsPolygonOnClosedTriangulation()) {
        second = representation->PolygonOnTriangulation2();
      }
      return !first.IsNull();
    }
  }
  return false;
}

// A copy of what a triangulation holds of the face (nodes, parameters,
// triangles, deviation), without the normals a presentation may be adding
// to the original meanwhile.
occ::handle<Poly_Triangulation> copy_triangulation(const occ::handle<Poly_Triangulation>& source) {
  occ::handle<Poly_Triangulation> copy =
      new Poly_Triangulation(source->NbNodes(), source->NbTriangles(), source->HasUVNodes(), false);
  for (int i = 1; i <= source->NbNodes(); ++i) {
    copy->SetNode(i, source->Node(i));
    if (source->HasUVNodes()) {
      copy->SetUVNode(i, source->UVNode(i));
    }
  }
  for (int i = 1; i <= source->NbTriangles(); ++i) {
    copy->SetTriangle(i, source->Triangle(i));
  }
  copy->Deflection(source->Deflection());
  copy->Parameters(source->Parameters());
  return copy;
}

// Gives `face` a copy of the donor face's triangulation and of its edges'
// polygons; false (nothing written) when an edge has no polygon there.
bool reuse_face(const FaceKey& target, const FaceKey& donor) {
  TopLoc_Location donor_location;
  const occ::handle<Poly_Triangulation>& source = BRep_Tool::Triangulation(donor.face, donor_location);
  if (source.IsNull()) {
    return false;
  }
  struct Polygons {
    occ::handle<Poly_PolygonOnTriangulation> first;
    occ::handle<Poly_PolygonOnTriangulation> second;
  };
  std::vector<Polygons> polygons(donor.edges.size());
  std::vector<TopoDS_Edge> donor_edges(donor.edges.size());
  std::vector<TopoDS_Edge> target_edges(target.edges.size());
  const auto edges_of = [](const FaceKey& key, std::vector<TopoDS_Edge>& edges) {
    for (TopExp_Explorer it(key.face, TopAbs_EDGE); it.More(); it.Next()) {
      for (std::size_t i = 0; i < key.edges.size(); ++i) {
        const EdgeKey& e = key.edges[i];
        const TopoDS_Shape& edge = it.Current();
        if (e.tshape == edge.TShape().get() && e.placement.IsEqual(edge.Location()) && edges[i].IsNull()) {
          edges[i] = TopoDS::Edge(it.Current());
        }
      }
    }
  };
  edges_of(donor, donor_edges);
  edges_of(target, target_edges);
  for (std::size_t i = 0; i < donor_edges.size(); ++i) {
    if (donor_edges[i].IsNull() || target_edges[i].IsNull() ||
        !polygons_of(donor_edges[i], source, donor_location, polygons[i].first, polygons[i].second)) {
      return false;
    }
  }
  const occ::handle<Poly_Triangulation> copy = copy_triangulation(source);
  BRep_Builder builder;
  builder.UpdateFace(target.face, copy);
  TopLoc_Location location;
  BRep_Tool::Triangulation(target.face, location);
  for (std::size_t i = 0; i < target_edges.size(); ++i) {
    if (polygons[i].second.IsNull()) {
      builder.UpdateEdge(target_edges[i], polygons[i].first->Copy(), copy, location);
    } else {
      builder.UpdateEdge(target_edges[i], polygons[i].first->Copy(), polygons[i].second->Copy(), copy, location);
    }
  }
  return true;
}

} // namespace

std::size_t reuse_triangulations(const TopoDS_Shape& shape, const std::vector<TopoDS_Shape>& donors) {
  if (shape.IsNull() || donors.empty()) {
    return 0;
  }
  std::vector<FaceKey> wanted;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    TopLoc_Location location;
    const TopoDS_Face& face = TopoDS::Face(it.Current());
    if (BRep_Tool::Triangulation(face, location).IsNull() && !BRep_Tool::Surface(face, location).IsNull()) {
      wanted.push_back(face_key(face));
    }
  }
  if (wanted.empty()) {
    return 0;
  }
  // The donors' meshed faces by their surface.
  std::unordered_map<const Geom_Surface*, std::vector<FaceKey>> offered;
  std::unordered_map<const Geom_Surface*, bool> needed;
  for (const FaceKey& key : wanted) {
    needed[key.surface] = true;
  }
  for (const TopoDS_Shape& donor : donors) {
    if (donor.IsNull()) {
      continue;
    }
    for (TopExp_Explorer it(donor, TopAbs_FACE); it.More(); it.Next()) {
      const TopoDS_Face& face = TopoDS::Face(it.Current());
      TopLoc_Location location;
      const Geom_Surface* surface = BRep_Tool::Surface(face, location).get();
      if (surface != nullptr && needed.count(surface) > 0 && !BRep_Tool::Triangulation(face, location).IsNull()) {
        offered[surface].push_back(face_key(face));
      }
    }
  }
  std::size_t reused = 0;
  std::vector<const TopoDS_TShape*> done;
  for (const FaceKey& key : wanted) {
    if (std::find(done.begin(), done.end(), key.face.TShape().get()) != done.end()) {
      continue; // the same face again in another placement
    }
    const auto found = offered.find(key.surface);
    if (found == offered.end()) {
      continue;
    }
    for (const FaceKey& candidate : found->second) {
      if (key.same(candidate) && reuse_face(key, candidate)) {
        done.push_back(key.face.TShape().get());
        ++reused;
        break;
      }
    }
  }
  return reused;
}

void use_bounded_presentation_meshes(std::size_t face_triangles) {
  BRepMesh_DiscretAlgoFactory::UnregisterFactory(BoundedFactory::kName);
  BRepMesh_DiscretAlgoFactory::RegisterFactory(new BoundedFactory(face_triangles), true);
  BRepMesh_DiscretFactory::Get().SetDefaultName(BoundedFactory::kName);
}

namespace {

bool has_surfaceless_face(const TopoDS_Shape& shape) {
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    TopLoc_Location location;
    if (BRep_Tool::Surface(TopoDS::Face(it.Current()), location).IsNull()) {
      return true;
    }
  }
  return false;
}

} // namespace

bool is_mesh(const TopoDS_Shape& shape) {
  bool any = false;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    TopLoc_Location location;
    if (!BRep_Tool::Surface(TopoDS::Face(it.Current()), location).IsNull()) {
      return false;
    }
    any = true;
  }
  return any;
}

TopoDS_Shape triangulate(const TopoDS_Shape& shape, const MeshOptions& options) {
  if (shape.IsNull()) {
    throw Error("cannot triangulate a null shape");
  }
  const IMeshTools_Parameters parameters = parameters_of(options);
  if (has_surfaceless_face(shape)) {
    return shape;
  }
  // A topology copy without triangulations: the original keeps its own.
  const TopoDS_Shape copy = BRepBuilderAPI_Copy(shape, false, false).Shape();
  BoundedMesh mesher(options);
  mesher.SetShape(copy);
  mesher.ChangeParameters() = parameters;
  mesher.Perform();
  if (!mesher.IsDone()) {
    throw Error("triangulation failed");
  }
  return copy;
}

bool mesh_in_place(const TopoDS_Shape& shape, const MeshOptions& options, const Message_ProgressRange& range,
                   MeshReport* report) {
  if (shape.IsNull()) {
    throw Error("cannot triangulate a null shape");
  }
  const IMeshTools_Parameters parameters = parameters_of(options);
  if (has_surfaceless_face(shape)) {
    return true;
  }
  BoundedMesh mesher(options, report);
  mesher.SetShape(shape);
  mesher.ChangeParameters() = parameters;
  mesher.Perform(range);
  return (mesher.GetStatusFlags() & IMeshData_UserBreak) == 0;
}

std::size_t mesh_all_in_place(const std::vector<TopoDS_Shape>& shapes, const MeshOptions& options,
                              const Message_ProgressRange& range, MeshReport* report) {
  const IMeshTools_Parameters parameters = parameters_of(options);
  std::size_t meshed = 0;
  // The shapes in one compound: it holds their own sub-shapes (TShapes), so
  // BRepMesh triangulates the shapes' faces themselves, each face once
  // (BRepMesh_ShapeExplorer skips a face met again in another placement,
  // and commits the edges' polygons on one thread).
  std::vector<const TopoDS_Shape*> joined;
  BRep_Builder builder;
  TopoDS_Compound all;
  builder.MakeCompound(all);
  for (const TopoDS_Shape& shape : shapes) {
    if (shape.IsNull()) {
      continue;
    }
    if (has_surfaceless_face(shape)) {
      ++meshed; // left as it is, as mesh_in_place leaves it
      continue;
    }
    builder.Add(all, shape);
    joined.push_back(&shape);
  }
  if (joined.empty()) {
    return meshed;
  }
  Message_ProgressScope scope(range, nullptr, 2);
  bool failed = false;
  try {
    BoundedMesh mesher(options, report);
    mesher.SetShape(all);
    mesher.ChangeParameters() = parameters;
    mesher.Perform(scope.Next());
    if ((mesher.GetStatusFlags() & IMeshData_UserBreak) == 0 && !scope.UserBreak()) {
      return meshed + joined.size();
    }
  } catch (const Standard_Failure&) {
    failed = true;
  } catch (const std::exception&) {
    failed = true;
  }
  if (!failed) {
    // Stopped: the shapes whose faces were all meshed by then.
    for (const TopoDS_Shape* shape : joined) {
      meshed += has_triangulation(*shape) ? 1 : 0;
    }
    return meshed;
  }
  // The joint meshing threw: the shapes it did not finish one by one, so
  // that one shape OCCT fails on leaves only itself without triangulations.
  Message_ProgressScope each(scope.Next(), nullptr, static_cast<double>(joined.size()));
  for (const TopoDS_Shape* shape : joined) {
    if (each.UserBreak()) {
      break;
    }
    const Message_ProgressRange step = each.Next();
    if (has_triangulation(*shape)) {
      ++meshed;
      continue;
    }
    try {
      if (mesh_in_place(*shape, options, step)) {
        ++meshed;
      }
    } catch (const Standard_Failure&) {
      // Left as it is.
    } catch (const std::exception&) {
    }
  }
  return meshed;
}

MeshStats mesh_stats(const TopoDS_Shape& shape) {
  MeshStats stats;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    ++stats.faces;
    TopLoc_Location location;
    const occ::handle<Poly_Triangulation> mesh =
        BRep_Tool::Triangulation(TopoDS::Face(it.Current()), location);
    if (!mesh.IsNull()) {
      stats.triangles += static_cast<std::size_t>(mesh->NbTriangles());
      stats.nodes += static_cast<std::size_t>(mesh->NbNodes());
    }
  }
  return stats;
}

TopoDS_Shape mesh_body(const occ::handle<Poly_Triangulation>& triangulation) {
  if (triangulation.IsNull() || triangulation->NbTriangles() == 0) {
    throw Error("empty triangulation");
  }
  TopoDS_Face face;
  BRep_Builder().MakeFace(face, triangulation);
  return face;
}

namespace {

// Vertices merged within a tolerance: each goes into a grid cell of the
// tolerance's size and is looked up in the cells around its own.
class VertexMerger {
public:
  VertexMerger(double tolerance, std::vector<std::array<double, 3>>& vertices)
      : m_tolerance(tolerance), m_vertices(vertices) {}

  std::uint32_t add(const gp_Pnt& p) {
    const Cell cell{key(p.X()), key(p.Y()), key(p.Z())};
    for (std::int64_t dx = -1; dx <= 1; ++dx) {
      for (std::int64_t dy = -1; dy <= 1; ++dy) {
        for (std::int64_t dz = -1; dz <= 1; ++dz) {
          const auto found = m_cells.find(Cell{cell.x + dx, cell.y + dy, cell.z + dz});
          if (found == m_cells.end()) {
            continue;
          }
          for (const std::uint32_t index : found->second) {
            const std::array<double, 3>& q = m_vertices[index];
            const double d2 = (p.X() - q[0]) * (p.X() - q[0]) + (p.Y() - q[1]) * (p.Y() - q[1]) +
                              (p.Z() - q[2]) * (p.Z() - q[2]);
            if (d2 <= m_tolerance * m_tolerance) {
              return index;
            }
          }
        }
      }
    }
    const auto index = static_cast<std::uint32_t>(m_vertices.size());
    m_vertices.push_back({p.X(), p.Y(), p.Z()});
    m_cells[cell].push_back(index);
    return index;
  }

private:
  struct Cell {
    std::int64_t x;
    std::int64_t y;
    std::int64_t z;
    bool operator==(const Cell& other) const { return x == other.x && y == other.y && z == other.z; }
  };
  struct CellHash {
    std::size_t operator()(const Cell& c) const {
      const auto mix = [](std::uint64_t h, std::int64_t v) {
        return (h ^ static_cast<std::uint64_t>(v)) * 0x100000001b3ull;
      };
      return static_cast<std::size_t>(mix(mix(mix(0xcbf29ce484222325ull, c.x), c.y), c.z));
    }
  };

  std::int64_t key(double v) const { return static_cast<std::int64_t>(std::floor(v / m_tolerance)); }

  double m_tolerance;
  std::vector<std::array<double, 3>>& m_vertices;
  std::unordered_map<Cell, std::vector<std::uint32_t>, CellHash> m_cells;
};

} // namespace

IndexedMesh indexed_mesh(const TopoDS_Shape& shape, const MeshOptions& options) {
  const TopoDS_Shape meshed = triangulate(shape, options);
  IndexedMesh mesh;
  VertexMerger merger(1e-6, mesh.vertices);
  std::vector<std::uint32_t> nodes;
  for (TopExp_Explorer it(meshed, TopAbs_FACE); it.More(); it.Next()) {
    const TopoDS_Face& face = TopoDS::Face(it.Current());
    TopLoc_Location location;
    const occ::handle<Poly_Triangulation> triangulation = BRep_Tool::Triangulation(face, location);
    if (triangulation.IsNull()) {
      throw Error("a face could not be triangulated");
    }
    const gp_Trsf placement = location.Transformation();
    nodes.resize(static_cast<std::size_t>(triangulation->NbNodes()));
    for (int i = 1; i <= triangulation->NbNodes(); ++i) {
      gp_Pnt p = triangulation->Node(i);
      if (!location.IsIdentity()) {
        p.Transform(placement);
      }
      nodes[static_cast<std::size_t>(i - 1)] = merger.add(p);
    }
    // A face's triangles follow its surface's normal: turned round where
    // the face is reversed, or mirrored by its placement.
    const bool turned = (face.Orientation() == TopAbs_REVERSED) != placement.IsNegative();
    for (int t = 1; t <= triangulation->NbTriangles(); ++t) {
      int a = 0;
      int b = 0;
      int c = 0;
      triangulation->Triangle(t).Get(a, b, c);
      if (turned) {
        std::swap(b, c);
      }
      const std::array<std::uint32_t, 3> triangle{nodes[static_cast<std::size_t>(a - 1)],
                                                  nodes[static_cast<std::size_t>(b - 1)],
                                                  nodes[static_cast<std::size_t>(c - 1)]};
      if (triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[2] == triangle[0]) {
        continue; // collapsed by the merge
      }
      mesh.triangles.push_back(triangle);
    }
  }
  return mesh;
}

} // namespace mitcad::io
