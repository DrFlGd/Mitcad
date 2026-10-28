// SPDX-License-Identifier: MIT
#include "mitcad/io/mesh_store.hpp"

#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include <BRep_Builder.hxx>
#include <BRep_CurveRepresentation.hxx>
#include <BRep_TEdge.hxx>
#include <BRep_Tool.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <Geom_Surface.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangle.hxx>
#include <Poly_Triangulation.hxx>
#include <Poly_TriangulationParameters.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_XYZ.hxx>

namespace mitcad::io {
namespace {

// "Mitcad triangulations", then the format version. Stored meshes of
// another format are not read (the store's build id changes with the
// program anyway).
constexpr char kMagic[4] = {'M', 'T', 'R', 'I'};
constexpr std::uint32_t kFormat = 2; // 2: a repeated face with its own signature
// How far a face's edges' end points may be from the stored ones (mm),
// per point summed: the baseline tolerance of geometric comparisons.
constexpr double kPointTolerance = 1e-6;

// What a face's entry holds.
enum Kind : std::uint8_t {
  kNone = 0,  // no triangulation (OCCT's mesher failed on the face)
  kOwn = 1,   // its triangulation and its edges' polygons
  kAgain = 2, // the same face as an earlier one, in another placement
};

class Writer {
public:
  void bytes(const void* data, std::size_t size) { m_out.append(static_cast<const char*>(data), size); }
  void u8(std::uint8_t value) { bytes(&value, 1); }
  void u32(std::uint32_t value) {
    std::array<unsigned char, 4> le{};
    for (std::size_t i = 0; i < le.size(); ++i) {
      le[i] = static_cast<unsigned char>(value >> (8 * i));
    }
    bytes(le.data(), le.size());
  }
  void f64(double value) {
    std::uint64_t bits = 0;
    std::memcpy(&bits, &value, sizeof bits);
    std::array<unsigned char, 8> le{};
    for (std::size_t i = 0; i < le.size(); ++i) {
      le[i] = static_cast<unsigned char>(bits >> (8 * i));
    }
    bytes(le.data(), le.size());
  }
  std::string take() { return std::move(m_out); }

private:
  std::string m_out;
};

class Reader {
public:
  explicit Reader(std::string_view data) : m_data(data) {}

  std::string_view bytes(std::size_t size) {
    if (size > remaining()) {
      throw std::runtime_error("stored display meshes: the data ends early");
    }
    const std::string_view out = m_data.substr(m_at, size);
    m_at += size;
    return out;
  }
  std::uint8_t u8() { return static_cast<std::uint8_t>(bytes(1)[0]); }
  std::uint32_t u32() {
    const std::string_view le = bytes(4);
    std::uint32_t value = 0;
    for (std::size_t i = 0; i < 4; ++i) {
      value |= static_cast<std::uint32_t>(static_cast<unsigned char>(le[i])) << (8 * i);
    }
    return value;
  }
  double f64() {
    const std::string_view le = bytes(8);
    std::uint64_t bits = 0;
    for (std::size_t i = 0; i < 8; ++i) {
      bits |= static_cast<std::uint64_t>(static_cast<unsigned char>(le[i])) << (8 * i);
    }
    double value = 0.0;
    std::memcpy(&value, &bits, sizeof value);
    return value;
  }
  // A count of items of `size` bytes each that the data must still hold.
  int count(std::size_t size) {
    const std::uint32_t value = u32();
    if (value > 0x7fffffffu || static_cast<std::size_t>(value) > remaining() / (size > 0 ? size : 1)) {
      throw std::runtime_error("stored display meshes: a count beyond the data");
    }
    return static_cast<int>(value);
  }
  std::size_t remaining() const { return m_data.size() - m_at; }
  bool done() const { return m_at == m_data.size(); }

private:
  std::string_view m_data;
  std::size_t m_at = 0;
};

using FaceMap = NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher>;

// The face's edges as BRepMesh keeps their polygons on its triangulation:
// in the explorer's order, each sub-shape (TShape and placement) once, so a
// seam's second use is left out (as reuse_triangulations takes them).
std::vector<TopoDS_Edge> face_edges(const TopoDS_Face& face) {
  std::vector<TopoDS_Edge> edges;
  for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
    const TopoDS_Edge& edge = TopoDS::Edge(it.Current());
    bool seen = false;
    for (const TopoDS_Edge& known : edges) {
      if (known.TShape().get() == edge.TShape().get() && known.Location().IsEqual(edge.Location())) {
        seen = true;
        break;
      }
    }
    if (!seen) {
      edges.push_back(edge);
    }
  }
  return edges;
}

// What a face's stored triangulation is checked by: the kind of its
// surface, its edges and the sum of their end points.
struct Signature {
  std::int32_t surface = -1;
  std::uint32_t edges = 0;
  gp_XYZ ends{0.0, 0.0, 0.0};
};

Signature signature_of(const TopoDS_Face& face, const std::vector<TopoDS_Edge>& edges) {
  Signature signature;
  TopLoc_Location location;
  const occ::handle<Geom_Surface>& surface = BRep_Tool::Surface(face, location);
  signature.surface = surface.IsNull() ? -1 : static_cast<std::int32_t>(GeomAdaptor_Surface(surface).GetType());
  signature.edges = static_cast<std::uint32_t>(edges.size());
  for (const TopoDS_Edge& edge : edges) {
    TopoDS_Vertex first;
    TopoDS_Vertex last;
    TopExp::Vertices(edge, first, last);
    if (!first.IsNull()) {
      signature.ends.Add(BRep_Tool::Pnt(first).XYZ());
    }
    if (!last.IsNull()) {
      signature.ends.Add(BRep_Tool::Pnt(last).XYZ());
    }
  }
  return signature;
}

void write_signature(Writer& out, const Signature& signature) {
  out.u32(static_cast<std::uint32_t>(signature.surface));
  out.u32(signature.edges);
  out.f64(signature.ends.X());
  out.f64(signature.ends.Y());
  out.f64(signature.ends.Z());
}

Signature read_signature(Reader& in) {
  Signature signature;
  signature.surface = static_cast<std::int32_t>(in.u32());
  signature.edges = in.u32();
  const double x = in.f64();
  const double y = in.f64();
  const double z = in.f64();
  signature.ends = gp_XYZ(x, y, z);
  return signature;
}

bool same_face(const Signature& stored, const Signature& found) {
  if (stored.surface != found.surface || stored.edges != found.edges) {
    return false;
  }
  const double tolerance = kPointTolerance * (1.0 + 2.0 * static_cast<double>(found.edges));
  return std::abs(stored.ends.X() - found.ends.X()) <= tolerance &&
         std::abs(stored.ends.Y() - found.ends.Y()) <= tolerance &&
         std::abs(stored.ends.Z() - found.ends.Z()) <= tolerance;
}

// The edge's polygons on the triangulation of the face (two for a seam),
// or false when it has none there (as reuse_triangulations finds them).
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

void write_polygon(Writer& out, const occ::handle<Poly_PolygonOnTriangulation>& polygon) {
  const int nodes = polygon->NbNodes();
  out.u32(static_cast<std::uint32_t>(nodes));
  out.f64(polygon->Deflection());
  out.u8(polygon->HasParameters() ? 1 : 0);
  for (int i = 1; i <= nodes; ++i) {
    out.u32(static_cast<std::uint32_t>(polygon->Node(i)));
  }
  if (polygon->HasParameters()) {
    for (int i = 1; i <= nodes; ++i) {
      out.f64(polygon->Parameter(i));
    }
  }
}

// A polygon whose nodes are among the triangulation's `nodes`.
occ::handle<Poly_PolygonOnTriangulation> read_polygon(Reader& in, int nodes) {
  const int count = in.count(4);
  const double deflection = in.f64();
  const bool parameters = in.u8() != 0;
  occ::handle<Poly_PolygonOnTriangulation> polygon = new Poly_PolygonOnTriangulation(count, parameters);
  for (int i = 1; i <= count; ++i) {
    const std::uint32_t node = in.u32();
    if (node < 1 || node > static_cast<std::uint32_t>(nodes)) {
      throw std::runtime_error("stored display meshes: a polygon's node is not in its triangulation");
    }
    polygon->SetNode(i, static_cast<int>(node));
  }
  if (parameters) {
    for (int i = 1; i <= count; ++i) {
      polygon->SetParameter(i, in.f64());
    }
  }
  polygon->Deflection(deflection);
  return polygon;
}

void write_triangulation(Writer& out, const occ::handle<Poly_Triangulation>& triangulation) {
  const int nodes = triangulation->NbNodes();
  const int triangles = triangulation->NbTriangles();
  const bool uv = triangulation->HasUVNodes();
  out.u32(static_cast<std::uint32_t>(nodes));
  out.u8(uv ? 1 : 0);
  out.f64(triangulation->Deflection());
  const occ::handle<Poly_TriangulationParameters>& parameters = triangulation->Parameters();
  out.u8(parameters.IsNull() ? 0 : 1);
  if (!parameters.IsNull()) {
    out.f64(parameters->Deflection());
    out.f64(parameters->Angle());
    out.f64(parameters->MinSize());
  }
  for (int i = 1; i <= nodes; ++i) {
    const gp_Pnt p = triangulation->Node(i);
    out.f64(p.X());
    out.f64(p.Y());
    out.f64(p.Z());
  }
  if (uv) {
    for (int i = 1; i <= nodes; ++i) {
      const gp_Pnt2d p = triangulation->UVNode(i);
      out.f64(p.X());
      out.f64(p.Y());
    }
  }
  out.u32(static_cast<std::uint32_t>(triangles));
  for (int i = 1; i <= triangles; ++i) {
    int a = 0;
    int b = 0;
    int c = 0;
    triangulation->Triangle(i).Get(a, b, c);
    out.u32(static_cast<std::uint32_t>(a));
    out.u32(static_cast<std::uint32_t>(b));
    out.u32(static_cast<std::uint32_t>(c));
  }
}

occ::handle<Poly_Triangulation> read_triangulation(Reader& in) {
  const std::uint32_t stored = in.u32();
  const bool uv = in.u8() != 0;
  const double deflection = in.f64();
  occ::handle<Poly_TriangulationParameters> parameters;
  if (in.u8() != 0) {
    const double linear = in.f64();
    const double angle = in.f64();
    const double size = in.f64();
    parameters = new Poly_TriangulationParameters(linear, angle, size);
  }
  const std::size_t node_size = uv ? 40 : 24;
  if (stored > 0x7fffffffu || static_cast<std::size_t>(stored) > in.remaining() / node_size) {
    throw std::runtime_error("stored display meshes: a count beyond the data");
  }
  const int nodes = static_cast<int>(stored);
  std::vector<gp_Pnt> points;
  points.reserve(static_cast<std::size_t>(nodes));
  for (int i = 1; i <= nodes; ++i) {
    const double x = in.f64();
    const double y = in.f64();
    const double z = in.f64();
    points.emplace_back(x, y, z);
  }
  std::vector<gp_Pnt2d> params;
  if (uv) {
    params.reserve(static_cast<std::size_t>(nodes));
    for (int i = 1; i <= nodes; ++i) {
      const double u = in.f64();
      const double v = in.f64();
      params.emplace_back(u, v);
    }
  }
  const int triangles = in.count(12);
  occ::handle<Poly_Triangulation> triangulation = new Poly_Triangulation(nodes, triangles, uv, false);
  for (int i = 1; i <= nodes; ++i) {
    triangulation->SetNode(i, points[static_cast<std::size_t>(i - 1)]);
    if (uv) {
      triangulation->SetUVNode(i, params[static_cast<std::size_t>(i - 1)]);
    }
  }
  for (int i = 1; i <= triangles; ++i) {
    std::array<int, 3> corner{};
    for (int& index : corner) {
      const std::uint32_t node = in.u32();
      if (node < 1 || node > stored) {
        throw std::runtime_error("stored display meshes: a triangle's node is not in its triangulation");
      }
      index = static_cast<int>(node);
    }
    triangulation->SetTriangle(i, Poly_Triangle(corner[0], corner[1], corner[2]));
  }
  triangulation->Deflection(deflection);
  if (!parameters.IsNull()) {
    triangulation->Parameters(parameters);
  }
  return triangulation;
}

// A copy of a triangulation (nodes, parameters, triangles, deviation and
// meshing parameters), for a face of its own.
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

// A face's entry read back.
struct StoredFace {
  Kind kind = kNone;
  std::uint32_t again = 0; // kAgain: the earlier face's index
  Signature signature;
  occ::handle<Poly_Triangulation> triangulation;
  // Each edge's polygons in face_edges' order; first null when it had none.
  std::vector<std::pair<occ::handle<Poly_PolygonOnTriangulation>, occ::handle<Poly_PolygonOnTriangulation>>> edges;
};

} // namespace

std::string triangulation_bytes(const TopoDS_Shape& shape) {
  if (shape.IsNull()) {
    throw std::invalid_argument("display meshes of a null shape");
  }
  FaceMap faces;
  TopExp::MapShapes(shape, TopAbs_FACE, faces);
  Writer out;
  out.bytes(kMagic, sizeof kMagic);
  out.u32(kFormat);
  out.u32(static_cast<std::uint32_t>(faces.Extent()));
  std::unordered_map<const TopoDS_TShape*, std::uint32_t> first;
  for (int i = 1; i <= faces.Extent(); ++i) {
    const TopoDS_Face& face = TopoDS::Face(faces(i));
    TopLoc_Location location;
    if (BRep_Tool::Surface(face, location).IsNull()) {
      throw std::invalid_argument("display meshes of a mesh body");
    }
    const auto found = first.emplace(face.TShape().get(), static_cast<std::uint32_t>(i - 1));
    const std::vector<TopoDS_Edge> edges = face_edges(face);
    if (!found.second) {
      out.u8(kAgain);
      out.u32(found.first->second);
      write_signature(out, signature_of(face, edges));
      continue;
    }
    const occ::handle<Poly_Triangulation>& triangulation = BRep_Tool::Triangulation(face, location);
    out.u8(triangulation.IsNull() ? kNone : kOwn);
    write_signature(out, signature_of(face, edges));
    if (triangulation.IsNull()) {
      continue;
    }
    write_triangulation(out, triangulation);
    for (const TopoDS_Edge& edge : edges) {
      occ::handle<Poly_PolygonOnTriangulation> one;
      occ::handle<Poly_PolygonOnTriangulation> two;
      if (!polygons_of(edge, triangulation, location, one, two)) {
        out.u8(0);
        continue;
      }
      out.u8(two.IsNull() ? 1 : 2);
      write_polygon(out, one);
      if (!two.IsNull()) {
        write_polygon(out, two);
      }
    }
  }
  return out.take();
}

std::size_t restore_triangulations(const TopoDS_Shape& shape, std::string_view data) {
  if (shape.IsNull()) {
    throw std::runtime_error("stored display meshes: the shape is empty");
  }
  Reader in(data);
  if (in.bytes(sizeof kMagic) != std::string_view(kMagic, sizeof kMagic)) {
    throw std::runtime_error("stored display meshes: not display meshes");
  }
  if (const std::uint32_t format = in.u32(); format != kFormat) {
    throw std::runtime_error("stored display meshes: format " + std::to_string(format) + " is not " +
                             std::to_string(kFormat));
  }
  FaceMap faces;
  TopExp::MapShapes(shape, TopAbs_FACE, faces);
  if (in.u32() != static_cast<std::uint32_t>(faces.Extent())) {
    throw std::runtime_error("stored display meshes: another number of faces");
  }
  // Everything read and checked first: nothing is written into a shape the
  // bytes are not of.
  std::vector<StoredFace> stored(static_cast<std::size_t>(faces.Extent()));
  std::vector<std::vector<TopoDS_Edge>> edges(stored.size());
  for (int i = 1; i <= faces.Extent(); ++i) {
    StoredFace& entry = stored[static_cast<std::size_t>(i - 1)];
    const TopoDS_Face& face = TopoDS::Face(faces(i));
    const std::uint8_t kind = in.u8();
    if (kind == kAgain) {
      // The same face as an earlier one, in another placement: the same
      // TShape, or a copy of it (a copy of the shape need not share it
      // between placements), whose edges and end points it is checked by.
      entry.kind = kAgain;
      entry.again = in.u32();
      entry.signature = read_signature(in);
      if (entry.again >= static_cast<std::uint32_t>(i - 1) || stored[entry.again].kind == kAgain) {
        throw std::runtime_error("stored display meshes: a face repeats one that is not before it");
      }
      std::vector<TopoDS_Edge>& own = edges[static_cast<std::size_t>(i - 1)];
      own = face_edges(face);
      const StoredFace& source = stored[entry.again];
      const bool shared = faces(static_cast<int>(entry.again) + 1).TShape().get() == face.TShape().get();
      if (!same_face(entry.signature, signature_of(face, own)) ||
          (!shared && source.kind == kOwn && source.edges.size() != own.size())) {
        throw std::runtime_error("stored display meshes: face " + std::to_string(i) +
                                 " is not the one it repeats");
      }
      continue;
    }
    if (kind != kNone && kind != kOwn) {
      throw std::runtime_error("stored display meshes: a face of an unknown kind");
    }
    entry.kind = static_cast<Kind>(kind);
    entry.signature = read_signature(in);
    std::vector<TopoDS_Edge>& own = edges[static_cast<std::size_t>(i - 1)];
    own = face_edges(face);
    if (!same_face(entry.signature, signature_of(face, own))) {
      throw std::runtime_error("stored display meshes: face " + std::to_string(i) + " is another face");
    }
    if (entry.kind == kNone) {
      continue;
    }
    entry.triangulation = read_triangulation(in);
    const int nodes = entry.triangulation->NbNodes();
    entry.edges.resize(own.size());
    for (auto& polygons : entry.edges) {
      const std::uint8_t count = in.u8();
      if (count > 2) {
        throw std::runtime_error("stored display meshes: an edge with more than two polygons");
      }
      if (count >= 1) {
        polygons.first = read_polygon(in, nodes);
      }
      if (count == 2) {
        polygons.second = read_polygon(in, nodes);
      }
    }
  }
  if (!in.done()) {
    throw std::runtime_error("stored display meshes: data after the faces");
  }
  BRep_Builder builder;
  std::size_t given = 0;
  std::vector<bool> written(stored.size(), false);
  for (int i = 1; i <= faces.Extent(); ++i) {
    const StoredFace& entry = stored[static_cast<std::size_t>(i - 1)];
    const bool again = entry.kind == kAgain;
    const StoredFace& source = again ? stored[entry.again] : entry;
    if (source.kind != kOwn) {
      continue;
    }
    const TopoDS_Face& face = TopoDS::Face(faces(i));
    // (The face's placement, which its edges' polygons are relative to.)
    TopLoc_Location location;
    if (!BRep_Tool::Triangulation(face, location).IsNull()) {
      // Meshed already: a face this shape shares with another, or the same
      // face in another placement, which counts when it was given one here.
      if (again && written[entry.again] &&
          faces(static_cast<int>(entry.again) + 1).TShape().get() == face.TShape().get()) {
        written[static_cast<std::size_t>(i - 1)] = true;
        ++given;
      }
      continue;
    }
    // A copy of the face in another placement takes copies: the same
    // triangulation in the face's own coordinates.
    const occ::handle<Poly_Triangulation> triangulation =
        again ? copy_triangulation(source.triangulation) : source.triangulation;
    builder.UpdateFace(face, triangulation);
    const std::vector<TopoDS_Edge>& own = edges[static_cast<std::size_t>(i - 1)];
    for (std::size_t k = 0; k < own.size() && k < source.edges.size(); ++k) {
      const auto& polygons = source.edges[k];
      if (polygons.first.IsNull()) {
        continue;
      }
      const occ::handle<Poly_PolygonOnTriangulation> first = again ? polygons.first->Copy() : polygons.first;
      if (polygons.second.IsNull()) {
        builder.UpdateEdge(own[k], first, triangulation, location);
      } else {
        const occ::handle<Poly_PolygonOnTriangulation> second =
            again ? polygons.second->Copy() : polygons.second;
        builder.UpdateEdge(own[k], first, second, triangulation, location);
      }
    }
    written[static_cast<std::size_t>(i - 1)] = true;
    ++given;
  }
  return given;
}

} // namespace mitcad::io
