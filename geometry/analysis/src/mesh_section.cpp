// SPDX-License-Identifier: MIT
#include "mesh_section.hpp"

#include <array>
#include <cmath>
#include <map>
#include <set>
#include <utility>

#include <BOPAlgo_Tools.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Poly_Triangulation.hxx>
#include <Precision.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <gp.hxx>
#include <gp_Trsf.hxx>

#include "convert.hpp"

namespace mitcad::analysis::detail {

namespace {

// The nodes of a mesh shape, merged where they coincide (the triangles of
// neighbouring faces, or of one face whose nodes repeat, then share them),
// with their side of the plane, and the points where triangle edges cross
// the plane, one per edge, so that the triangles on either side of an edge
// cut it at the same node.
class Slicer {
public:
  explicit Slicer(const Plane& plane)
      : m_origin(pnt(plane.origin)),
        m_normal(gp_Vec(plane.normal.x, plane.normal.y, plane.normal.z)) {
    if (m_normal.Magnitude() <= gp::Resolution()) {
      throw Error("the section plane needs a non-zero normal");
    }
    m_normal.Normalize();
  }

  int node(const gp_Pnt& p) {
    const std::array<double, 3> key{p.X(), p.Y(), p.Z()};
    const auto found = m_nodes.find(key);
    if (found != m_nodes.end()) {
      return found->second;
    }
    const double distance = gp_Vec(m_origin, p).Dot(m_normal);
    // A node within the confusion tolerance of the plane lies in it.
    const int side = std::abs(distance) <= Precision::Confusion() ? 0 : (distance < 0.0 ? -1 : 1);
    return m_nodes[key] = add(p, distance, side);
  }

  // The node where the edge between nodes on opposite sides crosses the plane.
  int crossing(int a, int b) {
    const std::pair<int, int> key = a < b ? std::make_pair(a, b) : std::make_pair(b, a);
    const auto found = m_crossings.find(key);
    if (found != m_crossings.end()) {
      return found->second;
    }
    const gp_Pnt& p = m_points[static_cast<std::size_t>(key.first)];
    const gp_Pnt& q = m_points[static_cast<std::size_t>(key.second)];
    const double dp = m_distances[static_cast<std::size_t>(key.first)];
    const double dq = m_distances[static_cast<std::size_t>(key.second)];
    const gp_Pnt at = p.XYZ() + (q.XYZ() - p.XYZ()) * (dp / (dp - dq));
    return m_crossings[key] = add(at, 0.0, 0);
  }

  int side(int id) const { return m_sides[static_cast<std::size_t>(id)]; }
  const gp_Pnt& point(int id) const { return m_points[static_cast<std::size_t>(id)]; }
  gp_Dir normal() const { return gp_Dir(m_normal); }
  const gp_Pnt& origin() const { return m_origin; }

private:
  int add(const gp_Pnt& p, double distance, int side) {
    m_points.push_back(p);
    m_distances.push_back(distance);
    m_sides.push_back(side);
    return static_cast<int>(m_points.size()) - 1;
  }

  gp_Pnt m_origin;
  gp_Vec m_normal;
  std::map<std::array<double, 3>, int> m_nodes;
  std::map<std::pair<int, int>, int> m_crossings;
  std::vector<gp_Pnt> m_points;
  std::vector<double> m_distances;
  std::vector<int> m_sides;
};

using Triangle = std::array<int, 3>;

// A face's triangles as merged nodes, in the face's own winding.
std::vector<Triangle> triangles(const TopoDS_Face& face, Slicer& slicer) {
  TopLoc_Location location;
  const occ::handle<Poly_Triangulation> mesh = BRep_Tool::Triangulation(face, location);
  if (mesh.IsNull()) {
    throw Error("a mesh face has no triangles");
  }
  const gp_Trsf transform = location.Transformation();
  std::vector<int> nodes(static_cast<std::size_t>(mesh->NbNodes()) + 1, -1);
  for (int i = 1; i <= mesh->NbNodes(); ++i) {
    nodes[static_cast<std::size_t>(i)] = slicer.node(mesh->Node(i).Transformed(transform));
  }
  std::vector<Triangle> found;
  found.reserve(static_cast<std::size_t>(mesh->NbTriangles()));
  for (int i = 1; i <= mesh->NbTriangles(); ++i) {
    int n[3] = {0, 0, 0};
    mesh->Triangle(i).Get(n[0], n[1], n[2]);
    for (const int k : n) {
      if (k < 1 || k > mesh->NbNodes()) {
        throw Error("a mesh triangle refers to a node the mesh does not have");
      }
    }
    found.push_back({nodes[static_cast<std::size_t>(n[0])], nodes[static_cast<std::size_t>(n[1])],
                     nodes[static_cast<std::size_t>(n[2])]});
  }
  return found;
}

using Segment = std::pair<int, int>;

void add_segment(std::set<Segment>& segments, int a, int b) {
  if (a != b) {
    segments.insert(a < b ? std::make_pair(a, b) : std::make_pair(b, a));
  }
}

// Where the plane cuts a triangle: a segment between two nodes in the
// plane (a vertex of it or an edge crossing), or nothing.
void cut(const Triangle& t, Slicer& slicer, std::set<Segment>& segments) {
  const int s[3] = {slicer.side(t[0]), slicer.side(t[1]), slicer.side(t[2])};
  int on = 0;
  for (int k = 0; k < 3; ++k) {
    on += s[k] == 0 ? 1 : 0;
  }
  if (on == 3) {
    return; // in the plane: its edges come from the triangles around it
  }
  if (on == 2) {
    // An edge in the plane; the set keeps it once for both its triangles.
    for (int k = 0; k < 3; ++k) {
      if (s[k] != 0) {
        add_segment(segments, t[(k + 1) % 3], t[(k + 2) % 3]);
      }
    }
    return;
  }
  std::vector<int> ends;
  for (int k = 0; k < 3; ++k) {
    const int a = t[static_cast<std::size_t>(k)];
    const int b = t[static_cast<std::size_t>((k + 1) % 3)];
    if (s[k] == 0) {
      ends.push_back(a);
    } else if (s[k] * s[(k + 1) % 3] < 0) {
      ends.push_back(slicer.crossing(a, b));
    }
  }
  // Two ends: the plane crosses the triangle; one: it only touches a vertex.
  if (ends.size() == 2) {
    add_segment(segments, ends[0], ends[1]);
  }
}

struct Curves {
  TopoDS_Compound edges;
  std::size_t count = 0;
  double length = 0.0;
};

Curves edges_of(const std::set<Segment>& segments, const Slicer& slicer) {
  Curves result;
  BRep_Builder builder;
  builder.MakeCompound(result.edges);
  // One vertex per node, shared by the edges that meet there.
  std::map<int, TopoDS_Vertex> vertices;
  const auto vertex = [&](int id) {
    auto found = vertices.find(id);
    if (found == vertices.end()) {
      found = vertices.emplace(id, BRepBuilderAPI_MakeVertex(slicer.point(id)).Vertex()).first;
    }
    return found->second;
  };
  for (const auto& [a, b] : segments) {
    const double length = slicer.point(a).Distance(slicer.point(b));
    if (length <= Precision::Confusion()) {
      continue; // a crossing next to a node: too short for an edge
    }
    builder.Add(result.edges, BRepBuilderAPI_MakeEdge(vertex(a), vertex(b)).Edge());
    ++result.count;
    result.length += length;
  }
  return result;
}

// The planar faces the section edges of a closed mesh bound, holes
// included, each facing along the plane's normal.
TopoDS_Compound faces_of(const TopoDS_Compound& edges, const gp_Dir& normal) {
  TopoDS_Compound result;
  BRep_Builder builder;
  builder.MakeCompound(result);
  if (!TopExp_Explorer(edges, TopAbs_EDGE).More()) {
    return result;
  }
  TopoDS_Shape wires;
  TopoDS_Shape faces;
  if (BOPAlgo_Tools::EdgesToWires(edges, wires, true) != 0 ||
      !BOPAlgo_Tools::WiresToFaces(wires, faces)) {
    throw Error("the section faces could not be computed");
  }
  for (TopExp_Explorer it(faces, TopAbs_FACE); it.More(); it.Next()) {
    TopoDS_Face face = TopoDS::Face(it.Current());
    const BRepAdaptor_Surface surface(face, false);
    gp_Dir axis = surface.Plane().Axis().Direction();
    if (face.Orientation() == TopAbs_REVERSED) {
      axis.Reverse();
    }
    if (axis.Dot(normal) < 0.0) {
      face.Reverse();
    }
    builder.Add(result, face);
  }
  return result;
}

std::size_t count_faces(const TopoDS_Shape& shape) {
  std::size_t count = 0;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    ++count;
  }
  return count;
}

// A mesh face of the triangles, whose nodes are the slicer's.
TopoDS_Face mesh_face(const std::vector<Triangle>& kept, const Slicer& slicer,
                      TopAbs_Orientation orientation) {
  std::map<int, int> local;
  for (const Triangle& t : kept) {
    for (const int id : t) {
      local.emplace(id, 0);
    }
  }
  const occ::handle<Poly_Triangulation> mesh = new Poly_Triangulation(
      static_cast<int>(local.size()), static_cast<int>(kept.size()), false);
  int index = 0;
  for (auto& [id, at] : local) {
    at = ++index;
    mesh->SetNode(at, slicer.point(id));
  }
  for (std::size_t i = 0; i < kept.size(); ++i) {
    const Triangle& t = kept[i];
    mesh->SetTriangle(static_cast<int>(i) + 1,
                      Poly_Triangle(local[t[0]], local[t[1]], local[t[2]]));
  }
  TopoDS_Face face;
  BRep_Builder().MakeFace(face, mesh);
  face.Orientation(orientation);
  return face;
}

} // namespace

bool mesh_only(const TopoDS_Shape& shape) {
  bool mesh = false;
  bool surfaces = false;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    TopLoc_Location location;
    if (BRep_Tool::Surface(TopoDS::Face(it.Current()), location).IsNull()) {
      mesh = true;
    } else {
      surfaces = true;
    }
  }
  if (mesh && surfaces) {
    throw Error("cannot section a body that has both mesh faces and faces on surfaces");
  }
  return mesh;
}

Section mesh_section(const TopoDS_Shape& shape, const Plane& plane) {
  Slicer slicer(plane);
  std::set<Segment> segments;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    for (const Triangle& t : triangles(TopoDS::Face(it.Current()), slicer)) {
      cut(t, slicer, segments);
    }
  }
  Curves curves = edges_of(segments, slicer);
  Section result;
  result.curves = curves.edges;
  result.edge_count = curves.count;
  result.length = curves.length;
  // Only a closed mesh encloses something for the plane to cut through.
  if (closed_mesh(shape)) {
    result.faces = faces_of(curves.edges, slicer.normal());
    result.face_count = count_faces(result.faces);
    result.area = surface_properties(result.faces).Mass();
  } else {
    TopoDS_Compound none;
    BRep_Builder().MakeCompound(none);
    result.faces = none;
  }
  return result;
}

TopoDS_Shape mesh_clip(const TopoDS_Shape& shape, const Plane& plane,
                       std::vector<ClippedFace>* faces) {
  Slicer slicer(plane);
  std::set<Segment> segments;
  std::vector<ClippedFace> clipped;
  bool changed = false;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    const TopoDS_Face& face = TopoDS::Face(it.Current());
    ClippedFace piece{face, {}};
    std::vector<Triangle> kept;
    bool cut_here = false;
    for (const Triangle& t : triangles(face, slicer)) {
      cut(t, slicer, segments);
      const int s[3] = {slicer.side(t[0]), slicer.side(t[1]), slicer.side(t[2])};
      if (s[0] <= 0 && s[1] <= 0 && s[2] <= 0) {
        kept.push_back(t); // behind the plane or in it
        continue;
      }
      cut_here = true;
      if (s[0] >= 0 && s[1] >= 0 && s[2] >= 0) {
        continue; // on the removed side
      }
      // Crossed: the part behind the plane, a triangle or a quadrilateral
      // in the triangle's winding, as a fan.
      std::vector<int> polygon;
      for (int k = 0; k < 3; ++k) {
        const int next = (k + 1) % 3;
        if (s[k] <= 0) {
          polygon.push_back(t[static_cast<std::size_t>(k)]);
        }
        if (s[k] * s[next] < 0) {
          polygon.push_back(slicer.crossing(t[static_cast<std::size_t>(k)],
                                            t[static_cast<std::size_t>(next)]));
        }
      }
      for (std::size_t k = 1; k + 1 < polygon.size(); ++k) {
        kept.push_back({polygon[0], polygon[k], polygon[k + 1]});
      }
    }
    if (!cut_here) {
      piece.pieces.push_back(face);
    } else if (!kept.empty()) {
      piece.pieces.push_back(mesh_face(kept, slicer, face.Orientation()));
    }
    changed = changed || cut_here;
    clipped.push_back(std::move(piece));
  }
  if (faces) {
    *faces = clipped;
  }
  if (!changed) {
    return shape;
  }
  TopoDS_Compound result;
  BRep_Builder builder;
  builder.MakeCompound(result);
  for (const ClippedFace& face : clipped) {
    for (const TopoDS_Shape& piece : face.pieces) {
      builder.Add(result, piece);
    }
  }
  if (closed_mesh(shape)) {
    const TopoDS_Compound caps = faces_of(edges_of(segments, slicer).edges, slicer.normal());
    for (TopExp_Explorer it(caps, TopAbs_FACE); it.More(); it.Next()) {
      builder.Add(result, it.Current());
    }
  }
  return result;
}

} // namespace mitcad::analysis::detail
