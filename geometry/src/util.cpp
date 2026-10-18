// SPDX-License-Identifier: MIT
#include "util.hpp"

#include <algorithm>
#include <atomic>
#include <cctype>
#include <cmath>
#include <optional>
#include <sstream>
#include <utility>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepBndLib.hxx>
#include <BRepCheck.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepCheck_Result.hxx>
#include <TopExp_Explorer.hxx>
#include <BRepGProp.hxx>
#include <BRep_Builder.hxx>
#include <BRep_TVertex.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom_Curve.hxx>
#include <Geom_Surface.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_List.hxx>
#include <Precision.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopExp.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Vertex.hxx>
#include <gp_Pnt2d.hxx>

namespace mitcad::geometry::detail {
namespace {

long long rounded(double value) { return std::llround(value * 1.0e6); }

} // namespace

void require_positive(const char* what, double value) {
  if (!(value > 0.0) || !std::isfinite(value)) {
    throw std::invalid_argument(std::string(what) + " must be greater than zero");
  }
}

void require_finite(const char* what, double value) {
  if (!std::isfinite(value)) {
    throw std::invalid_argument(std::string(what) + " must be a finite number");
  }
}

bool has_mesh_faces(const TopoDS_Shape& shape) {
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    TopLoc_Location location;
    if (BRep_Tool::Surface(TopoDS::Face(it.Current()), location).IsNull()) {
      return true;
    }
  }
  return false;
}

void require_surfaces(const TopoDS_Shape& shape, const char* operation) {
  if (has_mesh_faces(shape)) {
    throw std::invalid_argument(std::string(operation) +
                                ": a mesh body (faces without surfaces) cannot be used; the operation "
                                "works on solid and surface bodies");
  }
}

ShapeKey shape_key(const TopoDS_Shape& shape) {
  GProp_GProps props;
  switch (shape.ShapeType()) {
  case TopAbs_SOLID:
  case TopAbs_COMPSOLID:
  case TopAbs_COMPOUND:
    BRepGProp::VolumeProperties(shape, props);
    break;
  case TopAbs_FACE:
  case TopAbs_SHELL:
    BRepGProp::SurfaceProperties(shape, props);
    break;
  case TopAbs_EDGE:
  case TopAbs_WIRE:
    BRepGProp::LinearProperties(shape, props);
    break;
  default:
    break;
  }
  gp_Pnt centre;
  if (shape.ShapeType() == TopAbs_VERTEX) {
    centre = BRep_Tool::Pnt(TopoDS::Vertex(shape));
  } else {
    centre = props.CentreOfMass();
  }
  Bnd_Box box;
  BRepBndLib::Add(shape, box);
  double xmin = 0, ymin = 0, zmin = 0, xmax = 0, ymax = 0, zmax = 0;
  if (!box.IsVoid()) {
    box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
  }
  return {rounded(centre.X()), rounded(centre.Y()), rounded(centre.Z()),
          rounded(xmin),       rounded(ymin),       rounded(zmin)};
}

ShapeKey edge_key(const TopoDS_Shape& edge) {
  const TopoDS_Edge& e = TopoDS::Edge(edge);
  if (BRep_Tool::Degenerated(e)) {
    return shape_key(edge);
  }
  const BRepAdaptor_Curve curve(e);
  const gp_Pnt middle = curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2.0);
  const ShapeKey whole = shape_key(edge);
  return {rounded(middle.X()), rounded(middle.Y()), rounded(middle.Z()),
          whole[0],           whole[1],           whole[2]};
}

namespace {

std::string status_name(BRepCheck_Status status) {
  std::ostringstream text;
  BRepCheck::Print(status, text);
  std::string name = text.str();
  while (!name.empty() && std::isspace(static_cast<unsigned char>(name.back()))) {
    name.pop_back();
  }
  return name;
}

// The first problem the analyzer found, as " (<kind>: BRepCheck_...)" for
// messages: of a sub-shape itself, or of one in the context of another.
std::string first_problem(const BRepCheck_Analyzer& analyzer, const TopoDS_Shape& shape) {
  static const std::pair<TopAbs_ShapeEnum, const char*> kinds[] = {
      {TopAbs_VERTEX, "vertex"}, {TopAbs_EDGE, "edge"},   {TopAbs_WIRE, "wire"},
      {TopAbs_FACE, "face"},     {TopAbs_SHELL, "shell"}, {TopAbs_SOLID, "solid"}};
  for (const auto& [type, kind] : kinds) {
    for (TopExp_Explorer it(shape, type); it.More(); it.Next()) {
      const occ::handle<BRepCheck_Result>& result = analyzer.Result(it.Current());
      if (result.IsNull()) {
        continue;
      }
      for (const BRepCheck_Status status : result->Status()) {
        if (status != BRepCheck_NoError) {
          return std::string(" (") + kind + ": " + status_name(status) + ")";
        }
      }
      // Sub-shapes checked in this one's context (wires of a face).
      for (const auto& [sub_type, sub_kind] : kinds) {
        for (TopExp_Explorer sub(it.Current(), sub_type); sub.More(); sub.Next()) {
          if (!result->IsStatusOnShape(sub.Current())) {
            continue;
          }
          for (const BRepCheck_Status status : result->StatusOnShape(sub.Current())) {
            if (status != BRepCheck_NoError) {
              return std::string(" (") + sub_kind + " of a " + kind + ": " + status_name(status) + ")";
            }
          }
        }
      }
    }
  }
  return "";
}

} // namespace

// (The checker catches an allocation that fails and calls the sub-shape
// invalid, mitcad#132: that is no answer. It runs on OCCT's thread pool,
// whose jobs count their failed allocations as the launching thread's:
// on a large body the check took as long as the operation, mitcad#181.)
bool is_valid(const TopoDS_Shape& shape) {
  throw_if_cancelled();
  const std::size_t failed = failed_allocations_in_thread();
  const bool valid = BRepCheck_Analyzer(shape, true, true).IsValid();
  throw_if_allocation_failed(failed);
  return valid;
}

void require_valid(const TopoDS_Shape& shape, const char* operation) {
  throw_if_cancelled();
  const std::size_t failed = failed_allocations_in_thread();
  const BRepCheck_Analyzer analyzer(shape, true, true);
  throw_if_allocation_failed(failed);
  if (!analyzer.IsValid()) {
    throw std::runtime_error(std::string(operation) + " produced an invalid shape" + first_problem(analyzer, shape));
  }
}

namespace {

// The faces is_valid_where_changed gives the checker (in a compound), or
// none where the whole shape is to be checked: the result is not solids of
// one shell each, a face is not forward or reversed or is there twice, an
// edge of a shell does not have exactly one forward and one reversed use in
// it (a seam: both in its face), a shell is not connected, or more than half
// of the faces changed. In any of those cases the checker on the whole
// shape decides, so what it would say is never guessed.
std::optional<TopoDS_Compound> faces_to_check(const TopoDS_Shape& result, const ShapeMap& kept) {
  if (kept.IsEmpty()) {
    return std::nullopt;
  }
  // The shells, as the solids hold them.
  std::vector<TopoDS_Shape> shells;
  const auto add_solid = [&](const TopoDS_Shape& solid) {
    int count = 0;
    for (TopoDS_Iterator it(solid); it.More(); it.Next()) {
      if (it.Value().ShapeType() != TopAbs_SHELL) {
        return false;
      }
      shells.push_back(it.Value());
      ++count;
    }
    return count == 1;
  };
  if (result.ShapeType() == TopAbs_SOLID) {
    if (!add_solid(result)) {
      return std::nullopt;
    }
  } else if (result.ShapeType() == TopAbs_COMPOUND) {
    for (TopoDS_Iterator it(result); it.More(); it.Next()) {
      if (it.Value().ShapeType() != TopAbs_SOLID || !add_solid(it.Value())) {
        return std::nullopt;
      }
    }
  } else {
    return std::nullopt;
  }
  if (shells.empty()) {
    return std::nullopt;
  }

  using Uses = NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<int>, TopTools_ShapeMapHasher>;
  ShapeMap all;
  std::vector<TopoDS_Shape> faces;
  std::vector<int> parent;
  const auto find = [&parent](int i) {
    while (parent[static_cast<std::size_t>(i)] != i) {
      parent[static_cast<std::size_t>(i)] = parent[static_cast<std::size_t>(parent[static_cast<std::size_t>(i)])];
      i = parent[static_cast<std::size_t>(i)];
    }
    return i;
  };
  for (const TopoDS_Shape& shell : shells) {
    const int first = static_cast<int>(faces.size());
    // Each edge's forward and reversed uses in the shell, and the first
    // face that uses it.
    NCollection_IndexedDataMap<TopoDS_Shape, std::array<int, 3>, TopTools_ShapeMapHasher> edges;
    for (TopExp_Explorer f(shell, TopAbs_FACE); f.More(); f.Next()) {
      const TopoDS_Shape& face = f.Current();
      if (face.Orientation() != TopAbs_FORWARD && face.Orientation() != TopAbs_REVERSED) {
        return std::nullopt;
      }
      const int before = all.Extent();
      if (all.Add(face) <= before) {
        return std::nullopt; // twice in the result
      }
      const int index = static_cast<int>(faces.size());
      faces.push_back(face);
      parent.push_back(index);
      for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
        const TopoDS_Shape& edge = e.Current();
        if (BRep_Tool::Degenerated(TopoDS::Edge(edge))) {
          continue;
        }
        const TopAbs_Orientation orientation = edge.Orientation();
        if (orientation != TopAbs_FORWARD && orientation != TopAbs_REVERSED) {
          return std::nullopt;
        }
        int k = edges.FindIndex(edge);
        if (k == 0) {
          k = edges.Add(edge, {0, 0, index});
        }
        std::array<int, 3>& use = edges.ChangeFromIndex(k);
        use[orientation == TopAbs_FORWARD ? 0 : 1] += 1;
        const int a = find(index);
        const int b = find(use[2]);
        if (a != b) {
          parent[static_cast<std::size_t>(a)] = b;
        }
      }
    }
    for (int k = 1; k <= edges.Extent(); ++k) {
      const std::array<int, 3>& use = edges(k);
      if (use[0] != 1 || use[1] != 1) {
        return std::nullopt;
      }
    }
    const int root = find(first);
    for (int i = first; i < static_cast<int>(faces.size()); ++i) {
      if (find(i) != root) {
        return std::nullopt;
      }
    }
  }

  // The faces the result does not share with the inputs, and those that
  // meet them at a vertex.
  std::vector<bool> check(faces.size(), false);
  std::size_t changed = 0;
  Uses vertices;
  for (std::size_t i = 0; i < faces.size(); ++i) {
    const int k = kept.FindIndex(faces[i]);
    if (k > 0 && kept(k).IsEqual(faces[i])) {
      continue;
    }
    check[i] = true;
    ++changed;
  }
  if (2 * changed > faces.size()) {
    return std::nullopt;
  }
  for (std::size_t i = 0; i < faces.size(); ++i) {
    ShapeMap own;
    TopExp::MapShapes(faces[i], TopAbs_VERTEX, own);
    for (int v = 1; v <= own.Extent(); ++v) {
      int k = vertices.FindIndex(own(v));
      if (k == 0) {
        k = vertices.Add(own(v), NCollection_List<int>());
      }
      vertices.ChangeFromIndex(k).Append(static_cast<int>(i));
    }
  }
  std::vector<bool> near = check;
  for (int k = 1; k <= vertices.Extent(); ++k) {
    const NCollection_List<int>& users = vertices(k);
    bool touched = false;
    for (const int i : users) {
      touched = touched || check[static_cast<std::size_t>(i)];
    }
    if (touched) {
      for (const int i : users) {
        near[static_cast<std::size_t>(i)] = true;
      }
    }
  }
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (std::size_t i = 0; i < faces.size(); ++i) {
    if (near[i]) {
      builder.Add(compound, faces[i]);
    }
  }
  return compound;
}

std::atomic<std::size_t> g_local_checks{0};
std::atomic<std::size_t> g_local_faces{0};

std::optional<TopoDS_Compound> counted(std::optional<TopoDS_Compound> faces) {
  if (faces) {
    g_local_checks += 1;
    std::size_t n = 0;
    for (TopoDS_Iterator it(*faces); it.More(); it.Next()) {
      ++n;
    }
    g_local_faces += n;
  }
  return faces;
}

} // namespace

bool is_valid_where_changed(const TopoDS_Shape& result, const ShapeMap& kept) {
  throw_if_cancelled();
  const std::optional<TopoDS_Compound> faces = counted(faces_to_check(result, kept));
  return is_valid(faces ? *faces : result);
}

void require_valid_where_changed(const TopoDS_Shape& result, const ShapeMap& kept, const char* operation) {
  throw_if_cancelled();
  const std::optional<TopoDS_Compound> faces = counted(faces_to_check(result, kept));
  require_valid(faces ? *faces : result, operation);
}

ChecksWhereChanged checks_where_changed() { return {g_local_checks.load(), g_local_faces.load()}; }

int inflated_vertices(const TopoDS_Shape& shape, bool tighten) {
  // Above this (mm) a tolerance may be inflated.
  constexpr double kInflated = 1.0;
  using Ancestors = NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<TopoDS_Shape>, TopTools_ShapeMapHasher>;
  Ancestors edges_of;
  Ancestors faces_of;
  TopExp::MapShapesAndUniqueAncestors(shape, TopAbs_VERTEX, TopAbs_EDGE, edges_of);
  int count = 0;
  for (int i = 1; i <= edges_of.Extent(); ++i) {
    const TopoDS_Vertex& vertex = TopoDS::Vertex(edges_of.FindKey(i));
    const double tolerance = BRep_Tool::Tolerance(vertex);
    if (tolerance <= kInflated) {
      continue;
    }
    if (faces_of.IsEmpty()) {
      TopExp::MapShapesAndUniqueAncestors(shape, TopAbs_EDGE, TopAbs_FACE, faces_of);
    }
    // What the edges need: the vertex's distance from their ends, on their
    // curves and on their p-curves' surfaces, and their own tolerances.
    const gp_Pnt at = BRep_Tool::Pnt(vertex);
    double needed = 0.0;
    for (const TopoDS_Shape& e : edges_of(i)) {
      const TopoDS_Edge& edge = TopoDS::Edge(e);
      needed = std::max(needed, BRep_Tool::Tolerance(edge));
      if (BRep_Tool::Degenerated(edge)) {
        continue;
      }
      const double t = BRep_Tool::Parameter(vertex, edge);
      double first = 0.0;
      double last = 0.0;
      if (const occ::handle<Geom_Curve> curve = BRep_Tool::Curve(edge, first, last); !curve.IsNull()) {
        needed = std::max(needed, curve->Value(t).Distance(at));
      }
      const int index = faces_of.FindIndex(edge);
      if (index == 0) {
        continue;
      }
      for (const TopoDS_Shape& f : faces_of(index)) {
        const TopoDS_Face& face = TopoDS::Face(f);
        const occ::handle<Geom2d_Curve> pcurve = BRep_Tool::CurveOnSurface(edge, face, first, last);
        if (!pcurve.IsNull()) {
          const gp_Pnt2d uv = pcurve->Value(t);
          needed = std::max(needed, BRep_Tool::Surface(face)->Value(uv.X(), uv.Y()).Distance(at));
        }
      }
    }
    needed = 1.01 * needed + Precision::Confusion();
    if (tolerance <= 10.0 * needed) {
      continue;
    }
    ++count;
    if (tighten) {
      // (Set directly: BRep_Builder only widens a tolerance.)
      occ::down_cast<BRep_TVertex>(vertex.TShape())->Tolerance(needed);
    }
  }
  return count;
}

} // namespace mitcad::geometry::detail
