// SPDX-License-Identifier: MIT
#include "mitcad/io/mesh.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
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
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <IMeshData_Edge.hxx>
#include <IMeshData_Face.hxx>
#include <IMeshData_Model.hxx>
#include <IMeshData_PCurve.hxx>
#include <IMeshData_Status.hxx>
#include <IMeshTools_CurveTessellator.hxx>
#include <IMeshTools_Parameters.hxx>
#include <Message_ProgressScope.hxx>
#include <OSD_Parallel.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Precision.hxx>
#include <Standard_Failure.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Elips.hxx>
#include <gp_Pnt.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

#include "mitcad/io/body.hpp"

namespace mitcad::io {

namespace {
constexpr double kPi = 3.14159265358979323846;
constexpr double kDegree = kPi / 180.0;
}

MeshOptions MeshOptions::from(Refinement refinement) {
  switch (refinement) {
  case Refinement::Low:
    return MeshOptions{0.1, 30.0 * kDegree, false, 5000};
  case Refinement::Medium:
    return MeshOptions{0.03, 15.0 * kDegree, false, 20000};
  case Refinement::High:
    return MeshOptions{0.01, 8.0 * kDegree, false, 50000};
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

// BRepMesh's own context, with the edges of large cylinders and cones
// discretized at their faces' floor (coarsen_round_edges) and each face's
// deviation raised to its floor once the healer has computed the
// deviations from the edges' (BRepMesh's pre-processor then keeps a
// triangulation that meets it). The floors are computed over the faces in
// parallel when the meshing is (each task only reads its own face and sets
// its own deviation); a face whose floor cannot be computed keeps
// BRepMesh's deviation.
class BoundedContext : public BRepMesh_Context {
public:
  explicit BoundedContext(std::size_t face_triangles) : m_faceTriangles(face_triangles) {}

  bool DiscretizeEdges() override {
    if (!BRepMesh_Context::DiscretizeEdges()) {
      return false;
    }
    coarsen_round_edges(GetModel(), GetParameters(), m_faceTriangles);
    return true;
  }

  bool HealModel() override {
    if (!BRepMesh_Context::HealModel()) {
      return false;
    }
    const occ::handle<IMeshData_Model>& model = GetModel();
    if (m_faceTriangles == 0 || model.IsNull()) {
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
    return true;
  }

private:
  std::size_t m_faceTriangles;
};

// BRepMesh_IncrementalMesh with that context.
class BoundedMesh : public BRepMesh_IncrementalMesh {
public:
  explicit BoundedMesh(std::size_t face_triangles) : m_faceTriangles(face_triangles) {}

  using BRepMesh_IncrementalMesh::Perform;
  void Perform(const Message_ProgressRange& range = Message_ProgressRange()) override {
    BRepMesh_IncrementalMesh::Perform(new BoundedContext(m_faceTriangles), range);
  }

private:
  std::size_t m_faceTriangles;
};

// The presentations' meshing algorithm (use_bounded_presentation_meshes).
class BoundedFactory : public BRepMesh_DiscretAlgoFactory {
public:
  static constexpr const char* kName = "MitcadBounded";

  explicit BoundedFactory(std::size_t face_triangles)
      : BRepMesh_DiscretAlgoFactory(kName), m_faceTriangles(face_triangles) {}

  occ::handle<BRepMesh_DiscretRoot> CreateAlgorithm(const TopoDS_Shape& shape, double deviation,
                                                    double angle) override {
    occ::handle<BoundedMesh> mesher = new BoundedMesh(m_faceTriangles);
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
  BoundedMesh mesher(options.face_triangles);
  mesher.SetShape(copy);
  mesher.ChangeParameters() = parameters;
  mesher.Perform();
  if (!mesher.IsDone()) {
    throw Error("triangulation failed");
  }
  return copy;
}

bool mesh_in_place(const TopoDS_Shape& shape, const MeshOptions& options, const Message_ProgressRange& range) {
  if (shape.IsNull()) {
    throw Error("cannot triangulate a null shape");
  }
  const IMeshTools_Parameters parameters = parameters_of(options);
  if (has_surfaceless_face(shape)) {
    return true;
  }
  BoundedMesh mesher(options.face_triangles);
  mesher.SetShape(shape);
  mesher.ChangeParameters() = parameters;
  mesher.Perform(range);
  return (mesher.GetStatusFlags() & IMeshData_UserBreak) == 0;
}

std::size_t mesh_all_in_place(const std::vector<TopoDS_Shape>& shapes, const MeshOptions& options,
                              const Message_ProgressRange& range) {
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
    BoundedMesh mesher(options.face_triangles);
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
