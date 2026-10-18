// SPDX-License-Identifier: MIT
#include "mitcad/geometry/pattern.hpp"

#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <utility>

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepGProp.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <GeomAbs_SurfaceType.hxx>
#include <Geom_Curve.hxx>
#include <Geom_Plane.hxx>
#include <NCollection_List.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <ShapeFix_Face.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <NCollection_HSequence.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Pln.hxx>

#include "history.hpp"
#include "mitcad/geometry/cancel.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

constexpr double kTolerance = 1.0e-6;

// The first fuzzy value a union within a tolerance tries (mm, mitcad#397;
// as a join's, boolean.cpp).
constexpr double kFuzzyStart = 1.0e-5;

double area(const TopoDS_Shape& face) {
  GProp_GProps props;
  BRepGProp::SurfaceProperties(face, props);
  return props.Mass();
}

gp_Pln plane_of(const TopoDS_Face& face) {
  const Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  const Handle(Geom_Plane) plane = Handle(Geom_Plane)::DownCast(surface);
  if (plane.IsNull()) {
    throw std::invalid_argument("the openings of the faces must lie in planes");
  }
  return plane->Pln();
}

bool coplanar(const TopoDS_Face& a, const TopoDS_Face& b) {
  const gp_Pln pa = plane_of(a);
  const gp_Pln pb = plane_of(b);
  return pa.Axis().Direction().IsParallel(pb.Axis().Direction(), 1.0e-9) &&
         pa.Distance(pb.Location()) <= kTolerance;
}

// True when a vertex of `inner` lies inside `outer`.
bool contains(const TopoDS_Face& outer, const TopoDS_Face& inner) {
  TopExp_Explorer vertex(inner, TopAbs_VERTEX);
  gp_Pnt point;
  if (vertex.More()) {
    point = BRep_Tool::Pnt(TopoDS::Vertex(vertex.Current()));
  } else {
    // A wire of one closed edge without a vertex: a point on the edge.
    TopExp_Explorer edge(inner, TopAbs_EDGE);
    if (!edge.More()) {
      return false;
    }
    double first = 0.0;
    double last = 0.0;
    const Handle(Geom_Curve) curve = BRep_Tool::Curve(TopoDS::Edge(edge.Current()), first, last);
    point = curve->Value(first);
  }
  BRepClass_FaceClassifier classifier(outer, point, kTolerance);
  return classifier.State() == TopAbs_IN;
}

// Caps across openings: a cap inside a larger coplanar cap becomes its hole.
std::vector<TopoDS_Face> nest(std::vector<TopoDS_Face> caps) {
  std::stable_sort(caps.begin(), caps.end(), [](const TopoDS_Face& a, const TopoDS_Face& b) {
    return area(a) > area(b);
  });
  std::vector<TopoDS_Face> result;
  for (const TopoDS_Face& cap : caps) {
    bool hole = false;
    for (TopoDS_Face& outer : result) {
      if (!coplanar(outer, cap) || !contains(outer, cap)) {
        continue;
      }
      BRepBuilderAPI_MakeFace maker(outer);
      for (TopExp_Explorer wire(cap, TopAbs_WIRE); wire.More(); wire.Next()) {
        maker.Add(TopoDS::Wire(wire.Current()));
      }
      ShapeFix_Face fix(maker.Face());
      fix.FixOrientation();
      outer = fix.Face();
      hole = true;
      break;
    }
    if (!hole) {
      result.push_back(cap);
    }
  }
  return result;
}

// The orientation an edge has in a face, as the face uses it.
TopAbs_Orientation orientation_in(const TopoDS_Shape& face, const TopoDS_Shape& edge) {
  for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
    if (it.Current().IsSame(edge)) {
      return it.Current().Orientation();
    }
  }
  return TopAbs_EXTERNAL;
}

// The union of shapes whose boxes are apart, as one compound of their
// solids without a boolean operation (they share nothing to merge); null
// when two boxes meet. A pattern's copies are mostly so, and fusing 90 of
// them took most of a minute.
ShapePtr disjoint_union(const std::vector<const Shape*>& shapes) {
  // Boxes sorted by their low x, each compared with those that start
  // before it ends (all pairs: 12 million for 4900 copies).
  std::vector<Bnd_Box> boxes(shapes.size());
  std::vector<std::size_t> order(shapes.size());
  for (std::size_t i = 0; i < shapes.size(); ++i) {
    BRepBndLib::Add(shapes[i]->occt(), boxes[i]);
    boxes[i].Enlarge(kTolerance);
    if (boxes[i].IsVoid()) {
      return nullptr;
    }
    order[i] = i;
  }
  std::sort(order.begin(), order.end(), [&boxes](std::size_t a, std::size_t b) {
    return boxes[a].CornerMin().X() < boxes[b].CornerMin().X();
  });
  for (std::size_t k = 0; k < order.size(); ++k) {
    const Bnd_Box& box = boxes[order[k]];
    const double end = box.CornerMax().X();
    for (std::size_t m = k + 1; m < order.size() && boxes[order[m]].CornerMin().X() <= end; ++m) {
      if (!box.IsOut(boxes[order[m]])) {
        return nullptr;
      }
    }
  }
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (const Shape* shape : shapes) {
    for (TopExp_Explorer it(shape->occt(), TopAbs_SOLID); it.More(); it.Next()) {
      builder.Add(compound, it.Current());
    }
  }
  detail::FaceNamer namer(compound);
  for (const Shape* shape : shapes) {
    for (int i = 0; i < shape->face_count(); ++i) {
      for (const std::string& name : shape->face_names(i)) {
        namer.add(shape->face(i), name);
      }
    }
  }
  namer.finish();
  return namer.shape();
}

// Whether a face of the shape lies on a free-form surface (not a plane,
// cylinder, cone, sphere or torus): a B-spline, a swept surface, an offset.
bool free_form(const Shape& shape) {
  for (int i = 0; i < shape.face_count(); ++i) {
    switch (BRepAdaptor_Surface(shape.face(i), false).GetType()) {
    case GeomAbs_Plane:
    case GeomAbs_Cylinder:
    case GeomAbs_Cone:
    case GeomAbs_Sphere:
    case GeomAbs_Torus:
      break;
    default:
      return true;
    }
  }
  return false;
}

ShapePtr united(const std::vector<const Shape*>& shapes, double within) {
  if (shapes.size() == 1) {
    return std::make_shared<Shape>(*shapes.front());
  }
  if (ShapePtr apart = disjoint_union(shapes)) {
    return apart;
  }
  // (mitcad#400) The shapes of elementary faces are joined first, and the
  // shapes with free-form faces then with their union. OCCT's union
  // intersects every pair of faces that meet; a sketch's lines split a
  // slab's rim and a tread's sides into many faces, which the face merge
  // after the union makes one again. Joined in one go, each of those pieces
  // met a helical stringer's faces on the same cylinder: a free-form face
  // against a cylinder it lies on is the costliest pair, walked over the
  // whole of both, and the merge after it broke the result, which was then
  // joined again without it (a spiral stair's 40 treads and two stringers:
  // 10 s on 16 cores, 72 s on fewer, 2292 faces). Joined first, the
  // treads' merged faces meet the stringers once each.
  std::vector<const Shape*> elementary;
  std::vector<const Shape*> rest;
  for (const Shape* shape : shapes) {
    (free_form(*shape) ? rest : elementary).push_back(shape);
  }
  if (elementary.size() > 1 && !rest.empty()) {
    const ShapePtr first = united(elementary, within);
    rest.insert(rest.begin(), first.get());
    return united(rest, within);
  }
  const auto fused = [&](bool simplify, double fuzzy) {
    // On a copy of the shapes, as a boolean operation (boolean.cpp): the
    // face merge writes into the sub-shapes the union kept.
    std::vector<TopoDS_Shape> inputs;
    for (const Shape* shape : shapes) {
      inputs.push_back(shape->occt());
    }
    const detail::InputCopy copy(inputs);
    BRepAlgoAPI_Fuse fuse;
    NCollection_List<TopoDS_Shape> arguments;
    arguments.Append(copy.shape(0));
    NCollection_List<TopoDS_Shape> tools;
    for (std::size_t i = 1; i < shapes.size(); ++i) {
      tools.Append(copy.shape(i));
    }
    fuse.SetArguments(arguments);
    fuse.SetTools(tools);
    fuse.SetNonDestructive(true);
    // On all cores, as the booleans (boolean.cpp): the union of a part
    // with a thread insert whose faces run 25 turns took 20 s on one
    // (mitcad#235).
    fuse.SetRunParallel(true);
    if (fuzzy > 0.0) {
      fuse.SetFuzzyValue(fuzzy);
    }
    detail::build(fuse);
    if (!fuse.IsDone() || fuse.HasErrors()) {
      throw std::runtime_error("the union failed");
    }
    if (simplify) {
      fuse.SimplifyResult();
    }
    detail::FaceNamer namer(fuse.Shape());
    for (const Shape* shape : shapes) {
      namer.carry(fuse, *shape, copy);
    }
    namer.finish();
    return namer;
  };
  // As a boolean operation's (boolean.cpp): when merging the faces the
  // union left breaks its result, the faces stay as it left them.
  detail::FaceNamer namer = fused(true, 0.0);
  if (!detail::is_valid(namer.result())) {
    namer = fused(false, 0.0);
    // (mitcad#397) Within the caller's tolerance: the smallest fuzzy
    // value that gives a valid union (faces that lie on each other only
    // within an importer's geometric tolerance).
    if (!detail::is_valid(namer.result()) && within > 0.0) {
      std::vector<double> steps;
      for (double decade = kFuzzyStart; decade < within; decade *= 10.0) {
        steps.push_back(decade);
        if (3.0 * decade < within) {
          steps.push_back(3.0 * decade);
        }
      }
      steps.push_back(within);
      for (const double step : steps) {
        for (const bool simplify : {true, false}) {
          try {
            detail::FaceNamer fuzzy = fused(simplify, step);
            if (detail::is_valid(fuzzy.result())) {
              return fuzzy.shape();
            }
          } catch (const Cancelled&) {
            throw;
          } catch (...) {
            throw_if_cancelled();
          }
        }
      }
    }
    detail::require_valid(namer.result(), "the union");
  }
  return namer.shape();
}

} // namespace

ShapePtr unite(const std::vector<const Shape*>& shapes, double within) {
  if (shapes.empty()) {
    throw std::invalid_argument("there are no shapes to unite");
  }
  detail::require_finite("union tolerance", within);
  for (const Shape* shape : shapes) {
    detail::require_surfaces(shape->occt(), "union");
  }
  return detail::run("union", [&] { return united(shapes, within); });
}

FaceTool face_tool(const Shape& body, const std::vector<std::string>& names) {
  if (names.empty()) {
    throw std::invalid_argument("no faces selected");
  }
  detail::require_surfaces(body.occt(), "faces");
  return detail::run("faces", [&] {
    std::vector<int> indices;
    for (const std::string& name : names) {
      const std::vector<int> found = body.find_faces(name);
      if (found.empty()) {
        throw std::invalid_argument("the body has no face " + name);
      }
      for (int face : found) {
        if (std::find(indices.begin(), indices.end(), face) == indices.end()) {
          indices.push_back(face);
        }
      }
    }
    // A copy of the faces as the body orients them (outward from its
    // material), so that the caps do not add to the body's edges.
    BRep_Builder builder;
    TopoDS_Compound selection;
    builder.MakeCompound(selection);
    for (int face : indices) {
      builder.Add(selection, body.face(face));
    }
    BRepBuilderAPI_Copy copier(selection);
    std::vector<TopoDS_Face> faces;
    for (TopoDS_Iterator it(copier.Shape()); it.More(); it.Next()) {
      faces.push_back(TopoDS::Face(it.Value()));
    }
    if (faces.size() != indices.size()) {
      throw std::runtime_error("the faces could not be copied");
    }
    TopoDS_Shell shell;
    builder.MakeShell(shell);
    for (const TopoDS_Face& face : faces) {
      builder.Add(shell, face);
    }

    // The openings: edges on one face only (a seam is on its face twice).
    NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<TopoDS_Shape>, TopTools_ShapeMapHasher>
        owners;
    TopExp::MapShapesAndUniqueAncestors(shell, TopAbs_EDGE, TopAbs_FACE, owners);
    Handle(NCollection_HSequence<TopoDS_Shape>) open_edges = new NCollection_HSequence<TopoDS_Shape>;
    for (int i = 1; i <= owners.Extent(); ++i) {
      const TopoDS_Edge& edge = TopoDS::Edge(owners.FindKey(i));
      const NCollection_List<TopoDS_Shape>& at = owners(i);
      if (at.Extent() == 1 && !BRep_Tool::Degenerated(edge) &&
          !BRep_Tool::IsClosed(edge, TopoDS::Face(at.First()))) {
        open_edges->Append(edge);
      }
    }
    std::vector<TopoDS_Face> caps;
    if (!open_edges->IsEmpty()) {
      const Handle(NCollection_HSequence<TopoDS_Shape>) wires =
          ShapeAnalysis_FreeBounds::ConnectEdgesToWires(open_edges, kTolerance, true);
      for (int i = 1; i <= wires->Length(); ++i) {
        const TopoDS_Wire wire = TopoDS::Wire(wires->Value(i));
        if (!BRep_Tool::IsClosed(wire)) {
          throw std::invalid_argument("the openings of the faces are not closed");
        }
        BRepBuilderAPI_MakeFace maker(wire, true);
        if (!maker.IsDone()) {
          throw std::invalid_argument("the openings of the faces must lie in planes");
        }
        caps.push_back(maker.Face());
      }
      caps = nest(std::move(caps));
    }
    // Each cap uses its edges the other way round from the face next to it.
    for (TopoDS_Face& cap : caps) {
      TopExp_Explorer edge(cap, TopAbs_EDGE);
      if (edge.More() && owners.Contains(edge.Current())) {
        const TopoDS_Shape& neighbour = owners.FindFromKey(edge.Current()).First();
        if (orientation_in(cap, edge.Current()) == orientation_in(neighbour, edge.Current())) {
          cap.Reverse();
        }
      }
      builder.Add(shell, cap);
    }

    TopoDS_Solid solid;
    builder.MakeSolid(solid);
    builder.Add(solid, shell);
    GProp_GProps props;
    BRepGProp::VolumeProperties(solid, props);
    FaceTool result;
    // The faces point away from the body's material: outward of a boss,
    // into a pocket.
    result.material = props.Mass() > 0.0;
    if (!result.material) {
      builder.MakeSolid(solid);
      builder.Add(solid, shell.Reversed());
    }
    detail::require_valid(solid, "the solid of the faces");
    detail::FaceNamer namer(solid);
    for (std::size_t k = 0; k < faces.size(); ++k) {
      for (const std::string& name : body.face_names(indices[k])) {
        namer.add(faces[k], name);
      }
    }
    namer.finish();
    result.tool = namer.shape();
    return result;
  });
}

} // namespace mitcad::geometry
