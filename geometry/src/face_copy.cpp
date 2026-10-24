// SPDX-License-Identifier: MIT
#include "mitcad/geometry/face_copy.hpp"

#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <utility>

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepTools.hxx>
#include <BRepTopAdaptor_FClass2d.hxx>
#include <BRep_Builder.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_List.hxx>
#include <Precision.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Dir.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

#include "face_select.hpp"
#include "mitcad/geometry/boolean.hpp"
#include "mitcad/geometry/pattern.hpp"
#include "mitcad/geometry/query.hpp"
#include "mitcad/geometry/removed.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

// Faces of `after` this far (mm) from every face of `before` are the
// features' own; the others are faces the two share. The bodies come from
// one another by the features' operations, so the shared faces coincide
// to the kernel's tolerance.
constexpr double kSlack = 1.0e-4;

// Pieces smaller than this share of the largest piece, or than this
// volume (mm^3), are slivers along the shared faces.
constexpr double kSliverShare = 1.0e-6;
constexpr double kSliverVolume = 1.0e-9;

// Planar faces lie in one plane to this distance (mm) and face each other
// to this angle (radians).
constexpr double kPlane = 1.0e-5;
constexpr double kAngle = 1.0e-7;

// The part of a seam face that no face lies under counts when it is larger
// than this share of the face and this area (mm^2).
constexpr double kUncoveredShare = 1.0e-6;
constexpr double kUncoveredArea = 1.0e-9;

// How far (mm) the cut that takes the material off an unsupported part
// starts below the seam's plane, and past the material's far end it
// reaches, so that no face of it lies in a face of the material.
constexpr double kMargin = 1.0;

std::vector<ShapePtr> pieces_of(const BooleanResult& result) {
  std::vector<std::pair<double, ShapePtr>> measured;
  double largest = 0.0;
  for (const BooleanPiece& piece : result.pieces) {
    const double v = volume(*piece.shape);
    largest = std::max(largest, v);
    measured.emplace_back(v, piece.shape);
  }
  std::vector<ShapePtr> out;
  for (auto& [v, shape] : measured) {
    if (v > kSliverShare * largest && v > kSliverVolume) {
      out.push_back(std::move(shape));
    }
  }
  return out;
}

// A planar face with its plane, its outward normal (out of the material)
// and its box.
struct Planar {
  TopoDS_Face face;
  gp_Pln plane;
  gp_Dir normal;
  Bnd_Box box;
};

std::vector<Planar> planar_faces(const Shape& body) {
  std::vector<Planar> out;
  for (int i = 0; i < body.face_count(); ++i) {
    const TopoDS_Face& face = body.face(i);
    const BRepAdaptor_Surface surface(face);
    if (surface.GetType() != GeomAbs_Plane) {
      continue;
    }
    Planar planar;
    planar.face = face;
    planar.plane = surface.Plane();
    planar.normal = detail::planar_normal(face, planar.plane);
    BRepBndLib::Add(face, planar.box);
    if (planar.box.IsVoid()) {
      continue;
    }
    planar.box.Enlarge(kPlane);
    out.push_back(std::move(planar));
  }
  return out;
}

// Whether `candidate` lies in the plane of `face` (of the given normal and a
// point on it) and faces it.
bool facing(const Planar& candidate, const gp_Dir& normal, const gp_Pnt& point) {
  return candidate.normal.Dot(normal) < -(1.0 - kAngle) && std::abs(candidate.plane.Distance(point)) <= kPlane;
}

// Points inside the face: of a coarse grid over its parameter range, of a
// fine one for a narrow face; at most `most`, spread over the grid.
std::vector<gp_Pnt> interior_points(const TopoDS_Face& face, std::size_t most) {
  double u0 = 0.0;
  double u1 = 0.0;
  double v0 = 0.0;
  double v1 = 0.0;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  BRepTopAdaptor_FClass2d classifier(face, Precision::PConfusion());
  std::vector<gp_Pnt2d> inside;
  for (const int cells : {5, 24}) {
    for (int a = 0; a < cells; ++a) {
      for (int b = 0; b < cells; ++b) {
        const gp_Pnt2d uv(u0 + (u1 - u0) * (a + 0.5) / cells, v0 + (v1 - v0) * (b + 0.5) / cells);
        if (classifier.Perform(uv) == TopAbs_IN) {
          inside.push_back(uv);
        }
      }
    }
    if (!inside.empty()) {
      break;
    }
  }
  std::vector<gp_Pnt> points;
  const BRepAdaptor_Surface surface(face);
  const std::size_t wanted = std::min(inside.size(), most);
  for (std::size_t k = 0; k < wanted; ++k) {
    const gp_Pnt2d& uv = inside[k * inside.size() / wanted];
    points.push_back(surface.Value(uv.X(), uv.Y()));
  }
  return points;
}

// The planar faces of `solid` that lie wholly on planar faces of `before`
// (facing them): a compound of those faces; null when there are none.
ShapePtr seam_of(const Shape& solid, const std::vector<Planar>& before) {
  BRep_Builder builder;
  TopoDS_Compound seam;
  builder.MakeCompound(seam);
  bool any = false;
  for (int i = 0; i < solid.face_count(); ++i) {
    const TopoDS_Face& face = solid.face(i);
    const BRepAdaptor_Surface surface(face);
    if (surface.GetType() != GeomAbs_Plane) {
      continue;
    }
    const gp_Dir normal = detail::planar_normal(face, surface.Plane());
    const std::vector<gp_Pnt> points = interior_points(face, 9);
    if (points.empty()) {
      continue;
    }
    const bool on = std::all_of(points.begin(), points.end(), [&](const gp_Pnt& p) {
      return std::any_of(before.begin(), before.end(), [&](const Planar& g) {
        return facing(g, normal, p) && !g.box.IsOut(p) &&
               BRepClass_FaceClassifier(g.face, p, Precision::Confusion()).State() != TopAbs_OUT;
      });
    });
    if (on) {
      builder.Add(seam, face);
      any = true;
    }
  }
  return any ? std::make_shared<Shape>(seam) : nullptr;
}

double area_of(const TopoDS_Shape& shape) {
  GProp_GProps props;
  BRepGProp::SurfaceProperties(shape, props);
  return props.Mass();
}

// The part of the planar face that none of the candidates (planar faces
// in its plane, facing it) covers: its faces.
std::vector<TopoDS_Face> uncovered(const TopoDS_Face& face, const std::vector<const Planar*>& candidates) {
  std::vector<TopoDS_Face> out;
  if (candidates.empty()) {
    out.push_back(face);
    return out;
  }
  // The candidates turned the way the face is, so that the same-domain
  // faces take their part off it.
  NCollection_List<TopoDS_Shape> tools;
  for (const Planar* candidate : candidates) {
    tools.Append(candidate->face.Reversed());
  }
  NCollection_List<TopoDS_Shape> arguments;
  arguments.Append(face);
  BRepAlgoAPI_Cut cut;
  cut.SetArguments(arguments);
  cut.SetTools(tools);
  cut.SetNonDestructive(true);
  detail::build(cut);
  if (!cut.IsDone() || cut.HasErrors()) {
    throw std::runtime_error("the part of a seam no face lies under could not be worked out");
  }
  const double whole = area_of(face);
  for (TopExp_Explorer it(cut.Shape(), TopAbs_FACE); it.More(); it.Next()) {
    const TopoDS_Face piece = TopoDS::Face(it.Current());
    if (area_of(piece) > std::max(kUncoveredShare * whole, kUncoveredArea)) {
      out.push_back(piece);
    }
  }
  return out;
}

// The prism of a seam's uncovered face through the material over it: from
// kMargin below the seam's plane (along the seam's outward normal) to
// `length` above it.
TopoDS_Shape column(const TopoDS_Face& part, const gp_Dir& normal, double length) {
  gp_Trsf down;
  down.SetTranslation(gp_Vec(normal) * kMargin);
  const TopoDS_Shape start = BRepBuilderAPI_Transform(part, down, true).Shape();
  TopoDS_Shape prism = BRepPrimAPI_MakePrism(start, gp_Vec(normal) * -(length + kMargin)).Shape();
  GProp_GProps props;
  BRepGProp::VolumeProperties(prism, props);
  if (props.Mass() < 0.0) {
    prism.Reverse();
  }
  return prism;
}

} // namespace

FeatureChanges feature_changes(const Shape& before, const Shape& after) {
  return detail::run("feature faces", [&] {
    FeatureChanges changes;
    const std::vector<Planar> bottom = planar_faces(before);
    for (const ShapePtr& solid : pieces_of(removed_material(after, before, kSlack))) {
      changes.added.push_back({solid, seam_of(*solid, bottom)});
    }
    changes.removed = pieces_of(removed_material(before, after, kSlack));
    return changes;
  });
}

std::vector<ShapePtr> supported_copies(const Shape& body, const std::vector<ShapePtr>& solids,
                                       const std::vector<ShapePtr>& seams) {
  if (solids.size() != seams.size()) {
    throw std::invalid_argument("every copy needs its seam");
  }
  return detail::run("supported copies", [&] {
    const std::vector<Planar> faces = planar_faces(body);
    std::vector<ShapePtr> out;
    out.reserve(solids.size());
    for (std::size_t k = 0; k < solids.size(); ++k) {
      throw_if_cancelled();
      if (!seams[k]) {
        out.push_back(solids[k]);
        continue;
      }
      // The columns of material over the parts of the seam nothing lies
      // under.
      BRep_Builder builder;
      TopoDS_Compound columns;
      builder.MakeCompound(columns);
      bool cut_off = false;
      // Whether nothing lies under any of the seam's faces.
      bool bare_all = true;
      Bnd_Box extent;
      BRepBndLib::Add(solids[k]->occt(), extent);
      for (int i = 0; i < seams[k]->face_count(); ++i) {
        const TopoDS_Face& face = seams[k]->face(i);
        const BRepAdaptor_Surface surface(face);
        if (surface.GetType() != GeomAbs_Plane) {
          bare_all = false;
          continue;
        }
        const gp_Dir normal = detail::planar_normal(face, surface.Plane());
        const gp_Pnt origin = surface.Plane().Location();
        Bnd_Box box;
        BRepBndLib::Add(face, box);
        box.Enlarge(kPlane);
        std::vector<const Planar*> under;
        for (const Planar& candidate : faces) {
          if (!candidate.box.IsOut(box) && facing(candidate, normal, origin)) {
            under.push_back(&candidate);
          }
        }
        const std::vector<TopoDS_Face> bare = uncovered(face, under);
        double bare_area = 0.0;
        for (const TopoDS_Face& part : bare) {
          bare_area += area_of(part);
        }
        if (bare_area < (1.0 - kUncoveredShare) * area_of(face)) {
          bare_all = false;
        }
        if (bare.empty()) {
          continue;
        }
        // How far the material reaches from the seam's plane.
        double length = 0.0;
        for (const double x : {extent.CornerMin().X(), extent.CornerMax().X()}) {
          for (const double y : {extent.CornerMin().Y(), extent.CornerMax().Y()}) {
            for (const double z : {extent.CornerMin().Z(), extent.CornerMax().Z()}) {
              length = std::max(length, -gp_Vec(origin, gp_Pnt(x, y, z)).Dot(gp_Vec(normal)));
            }
          }
        }
        for (const TopoDS_Face& part : bare) {
          builder.Add(columns, column(part, normal, length + kMargin));
          cut_off = true;
        }
      }
      if (!cut_off) {
        out.push_back(solids[k]);
        continue;
      }
      if (bare_all) {
        // Nothing under the seam: nothing of the copy stands.
        out.push_back(nullptr);
        continue;
      }
      const BooleanResult left = boolean(BooleanOp::Cut, {solids[k].get()}, Shape(columns));
      std::vector<ShapePtr> pieces;
      for (const BooleanPiece& piece : left.pieces) {
        pieces.push_back(piece.shape);
      }
      if (!left.touched.front()) {
        out.push_back(solids[k]);
      } else if (pieces.empty()) {
        out.push_back(nullptr);
      } else if (pieces.size() == 1) {
        out.push_back(pieces.front());
      } else {
        std::vector<const Shape*> apart;
        for (const ShapePtr& piece : pieces) {
          apart.push_back(piece.get());
        }
        out.push_back(unite(apart));
      }
    }
    return out;
  });
}

} // namespace mitcad::geometry
