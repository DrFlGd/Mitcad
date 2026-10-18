// SPDX-License-Identifier: MIT
#include "mitcad/io/mesh.hpp"

#include <algorithm>
#include <cmath>
#include <unordered_map>
#include <utility>

#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepMesh_Context.hxx>
#include <BRepMesh_DiscretAlgoFactory.hxx>
#include <BRepMesh_DiscretFactory.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <IMeshData_Face.hxx>
#include <IMeshData_Model.hxx>
#include <IMeshData_Status.hxx>
#include <IMeshTools_Parameters.hxx>
#include <Precision.hxx>
#include <Standard_Failure.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Pnt.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

#include "mitcad/io/body.hpp"

namespace mitcad::io {

namespace {
constexpr double kDegree = 3.14159265358979323846 / 180.0;
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

// BRepMesh's own context, with each face's deviation raised to its floor
// once the healer has computed the deviations from the edges' (BRepMesh's
// pre-processor then keeps a triangulation that meets it).
class BoundedContext : public BRepMesh_Context {
public:
  explicit BoundedContext(std::size_t face_triangles) : m_faceTriangles(face_triangles) {}

  bool HealModel() override {
    if (!BRepMesh_Context::HealModel()) {
      return false;
    }
    const occ::handle<IMeshData_Model>& model = GetModel();
    if (m_faceTriangles == 0 || model.IsNull()) {
      return true;
    }
    for (int i = 0; i < model->FacesNb(); ++i) {
      const IMeshData::IFaceHandle& face = model->GetFace(i);
      const double floor = face_deviation_floor(face->GetFace(), face->GetDeflection(), m_faceTriangles);
      if (floor > face->GetDeflection()) {
        face->SetDeflection(floor);
      }
    }
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
