// SPDX-License-Identifier: MIT
#include "mitcad/geometry/query.hpp"

#include <algorithm>
#include <cmath>
#include <exception>
#include <memory>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepGProp.hxx>
#include <BRepTools.hxx>
#include <BRepTopAdaptor_FClass2d.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BezierCurve.hxx>
#include <Geom_Line.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_SurfaceOfLinearExtrusion.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_Map.hxx>
#include <OSD_Parallel.hxx>
#include <Precision.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <gp.hxx>
#include <gp_Pnt2d.hxx>

#include "mitcad/analysis/common.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

const char* surface_type(const TopoDS_Face& face) {
  // (The surface's type needs no parameter bounds of the face.)
  switch (BRepAdaptor_Surface(face, false).GetType()) {
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

const char* curve_type(const TopoDS_Edge& edge) {
  if (BRep_Tool::Degenerated(edge)) {
    return "degenerate";
  }
  switch (BRepAdaptor_Curve(edge).GetType()) {
  case GeomAbs_Line:
    return "line";
  case GeomAbs_Circle:
    return "circle";
  case GeomAbs_Ellipse:
    return "ellipse";
  case GeomAbs_Hyperbola:
    return "hyperbola";
  case GeomAbs_Parabola:
    return "parabola";
  case GeomAbs_BezierCurve:
    return "bezier";
  case GeomAbs_BSplineCurve:
    return "bspline";
  case GeomAbs_OffsetCurve:
    return "offset";
  default:
    return "other";
  }
}

// BRepBndLib::AddOptimal(shape, box, false, false) with the faces on all
// cores: the optimal box of a spline face is a search over its surface,
// seconds for a body of thousands of faces (through-all extrusions ask for
// the box of each new body). The same box: AddOptimal takes the union of
// its faces' boxes, then adds the edges not in faces and the vertices not
// in edges, which are added here as it adds them.
Bnd_Box optimal_box(const TopoDS_Shape& shape) {
  std::vector<TopoDS_Shape> faces;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    faces.push_back(it.Current());
  }
  std::vector<Bnd_Box> boxes(faces.size());
  std::vector<std::exception_ptr> errors(faces.size());
  OSD_Parallel::For(
      0, static_cast<int>(faces.size()),
      [&](int i) {
        const auto k = static_cast<std::size_t>(i);
        try {
          BRepBndLib::AddOptimal(faces[k], boxes[k], false, false);
        } catch (...) {
          errors[k] = std::current_exception();
        }
      },
      faces.size() < 8);
  Bnd_Box box;
  for (std::size_t k = 0; k < faces.size(); ++k) {
    if (errors[k]) {
      std::rethrow_exception(errors[k]);
    }
    if (!boxes[k].IsVoid()) {
      double xmin = 0, ymin = 0, zmin = 0, xmax = 0, ymax = 0, zmax = 0;
      boxes[k].Get(xmin, ymin, zmin, xmax, ymax, zmax);
      box.Update(xmin, ymin, zmin, xmax, ymax, zmax);
    }
  }
  BRep_Builder builder;
  TopoDS_Compound rest;
  builder.MakeCompound(rest);
  bool any = false;
  for (TopExp_Explorer it(shape, TopAbs_EDGE, TopAbs_FACE); it.More(); it.Next()) {
    builder.Add(rest, it.Current());
    any = true;
  }
  for (TopExp_Explorer it(shape, TopAbs_VERTEX, TopAbs_EDGE); it.More(); it.Next()) {
    builder.Add(rest, it.Current());
    any = true;
  }
  if (any) {
    BRepBndLib::AddOptimal(rest, box, false, false);
  }
  return box;
}

} // namespace

MassProperties mass_properties(const Shape& shape) {
  return detail::run("mass properties", [&] {
    MassProperties result;
    if (shape.occt().IsNull()) {
      return result;
    }
    if (const auto measured = shape.measured()) {
      const auto& m = *measured;
      result.volume = m[0];
      result.area = m[1];
      result.center = gp_Pnt(m[2], m[3], m[4]);
      return result;
    }
    // The analysis library's integrals, as every reported volume.
    // A sheet (open shells, no solid) encloses nothing: only closed shells count
    // when there is no solid.
    bool sheet = !TopExp_Explorer(shape.occt(), TopAbs_SOLID).More();
    if (sheet) {
      bool open = false;
      for (TopExp_Explorer it(shape.occt(), TopAbs_SHELL); it.More() && !open; it.Next()) {
        open = !BRep_Tool::IsClosed(it.Current());
      }
      sheet = open;
    }
    const GProp_GProps volume = analysis::volume_properties(shape.occt(), sheet);
    const GProp_GProps surface = analysis::surface_properties(shape.occt());
    result.volume = volume.Mass();
    result.area = surface.Mass();
    result.center = result.volume > 0.0 ? volume.CentreOfMass() : surface.CentreOfMass();
    shape.set_measured({result.volume, result.area, result.center.X(), result.center.Y(),
                        result.center.Z()});
    return result;
  });
}

double volume(const Shape& shape) { return mass_properties(shape).volume; }

MassProperties fixed_point_properties(const Shape& shape) {
  return detail::run("mass properties", [&] {
    MassProperties result;
    if (shape.occt().IsNull()) {
      return result;
    }
    gp_XYZ moment(0.0, 0.0, 0.0);
    for (TopExp_Explorer it(shape.occt(), TopAbs_SOLID); it.More(); it.Next()) {
      GProp_GProps solid;
      BRepGProp::VolumeProperties(it.Current(), solid);
      const double volume = std::abs(solid.Mass());
      result.volume += volume;
      moment += volume * solid.CentreOfMass().XYZ();
    }
    GProp_GProps surface;
    BRepGProp::SurfaceProperties(shape.occt(), surface);
    result.area = surface.Mass();
    if (result.volume > 0.0) {
      result.center = gp_Pnt(moment / result.volume);
    } else {
      result.center = surface.CentreOfMass();
    }
    return result;
  });
}

BoundingBox bounding_box(const Shape& shape) {
  return detail::run("bounding box", [&] {
    BoundingBox result;
    if (shape.occt().IsNull()) {
      return result;
    }
    if (const auto bounds = shape.bounds()) {
      const auto& b = *bounds;
      result.empty = false;
      result.min = gp_Pnt(b[0], b[1], b[2]);
      result.max = gp_Pnt(b[3], b[4], b[5]);
      return result;
    }
    Bnd_Box box = optimal_box(shape.occt());
    if (box.IsVoid()) {
      // A mesh body has only triangulations.
      BRepBndLib::Add(shape.occt(), box, true);
    }
    if (box.IsVoid()) {
      return result;
    }
    double xmin = 0, ymin = 0, zmin = 0, xmax = 0, ymax = 0, zmax = 0;
    box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
    result.empty = false;
    result.min = gp_Pnt(xmin, ymin, zmin);
    result.max = gp_Pnt(xmax, ymax, zmax);
    shape.set_bounds({xmin, ymin, zmin, xmax, ymax, zmax});
    return result;
  });
}

BoundingBox enclosing_box(const Shape& shape) {
  return detail::run("enclosing box", [&] {
    BoundingBox result;
    if (shape.occt().IsNull()) {
      return result;
    }
    if (const auto bounds = shape.bounds()) {
      const auto& b = *bounds;
      result.empty = false;
      result.min = gp_Pnt(b[0], b[1], b[2]);
      result.max = gp_Pnt(b[3], b[4], b[5]);
      return result;
    }
    // The faces' control points, not the optimum of each curved face
    // (OCCT's particle swarm: seconds for a body of thousands of spline
    // faces), and the shape's tolerances: a box that holds the shape. Not
    // kept: the shape's own box is the tight one.
    Bnd_Box box;
    BRepBndLib::Add(shape.occt(), box, false);
    if (box.IsVoid()) {
      return bounding_box(shape);
    }
    double xmin = 0, ymin = 0, zmin = 0, xmax = 0, ymax = 0, zmax = 0;
    box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
    result.empty = false;
    result.min = gp_Pnt(xmin, ymin, zmin);
    result.max = gp_Pnt(xmax, ymax, zmax);
    return result;
  });
}

std::vector<FaceInfo> face_infos(const Shape& shape) {
  return detail::run("faces", [&] {
    // The faces' kinds and areas are measured once per shape (it never
    // changes), on all cores: the area integrals of a body of thousands of
    // faces took seconds a call, and the .f3d import asks for them again
    // and again.
    std::shared_ptr<const Shape::FaceMeasures> measures = shape.face_measures();
    if (!measures) {
      // (The faces read here: the input check's bookkeeping is the
      // calling thread's.)
      std::vector<TopoDS_Face> each;
      for (int i = 0; i < shape.face_count(); ++i) {
        each.push_back(shape.face(i));
      }
      auto measured = std::make_shared<Shape::FaceMeasures>(each.size());
      std::vector<std::exception_ptr> errors(each.size());
      OSD_Parallel::For(
          0, static_cast<int>(each.size()),
          [&](int i) {
            const auto k = static_cast<std::size_t>(i);
            try {
              (*measured)[k] = {surface_type(each[k]), analysis::surface_properties(each[k]).Mass()};
            } catch (...) {
              errors[k] = std::current_exception();
            }
          },
          each.size() < 8);
      for (const std::exception_ptr& error : errors) {
        if (error) {
          std::rethrow_exception(error);
        }
      }
      shape.set_face_measures(measured);
      measures = std::move(measured);
    }
    std::vector<FaceInfo> faces;
    faces.reserve(measures->size());
    for (int i = 0; i < shape.face_count(); ++i) {
      const Shape::FaceMeasure& measure = (*measures)[static_cast<std::size_t>(i)];
      faces.push_back({shape.face_names(i), measure.surface, measure.area});
    }
    return faces;
  });
}

std::vector<EdgeInfo> edge_infos(const Shape& shape) {
  return detail::run("edges", [&] {
    std::vector<EdgeInfo> edges;
    for (int i = 0; i < shape.edge_count(); ++i) {
      GProp_GProps props;
      BRepGProp::LinearProperties(shape.edge(i), props);
      edges.push_back({shape.edge_name(i), curve_type(shape.edge(i)), props.Mass()});
    }
    return edges;
  });
}

std::vector<EdgeMiddle> edge_middles(const Shape& shape) {
  // Kept by the shape once worked out (Shape::kept_edge_middles); kept only
  // when the run did not fail.
  if (const auto kept = std::static_pointer_cast<const std::vector<EdgeMiddle>>(shape.kept_edge_middles())) {
    return *kept;
  }
  auto middles = std::make_shared<const std::vector<EdgeMiddle>>(detail::run("edges", [&] {
    std::vector<EdgeMiddle> middles;
    for (int i = 0; i < shape.edge_count(); ++i) {
      const std::string& name = shape.edge_name(i);
      if (name.empty()) {
        continue;
      }
      TopoDS_Edge edge = shape.edge(i);
      edge.Orientation(TopAbs_FORWARD);
      if (BRep_Tool::Degenerated(edge)) {
        continue;
      }
      const BRepAdaptor_Curve curve(edge);
      const double length = GCPnts_AbscissaPoint::Length(curve);
      const GCPnts_AbscissaPoint at(curve, length / 2.0, curve.FirstParameter());
      if (!at.IsDone()) {
        continue;
      }
      gp_Pnt point;
      gp_Vec tangent;
      curve.D1(at.Parameter(), point, tangent);
      if (tangent.Magnitude() <= gp::Resolution()) {
        continue;
      }
      middles.push_back({name, point, tangent.Normalized(), length});
    }
    return middles;
  }));
  shape.keep_edge_middles(middles);
  return *middles;
}

namespace {

// Whether OCCT's box of the face (BRepBndLib::Add from its geometry) holds
// all of it: the boxes of analytic and polynomial surfaces over the face's
// parameters, and a plane's from its edges' curves, are the curves' or
// surfaces' own or those of their poles; others are sampled.
bool box_holds_face(const TopoDS_Face& face) {
  switch (BRepAdaptor_Surface(face, false).GetType()) {
  case GeomAbs_Cylinder:
  case GeomAbs_Cone:
  case GeomAbs_Sphere:
  case GeomAbs_Torus:
  case GeomAbs_BezierSurface:
  case GeomAbs_BSplineSurface:
    return true;
  case GeomAbs_Plane:
    for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
      const TopoDS_Edge& edge = TopoDS::Edge(it.Current());
      if (!BRep_Tool::IsGeometric(edge)) {
        continue;
      }
      switch (BRepAdaptor_Curve(edge).GetType()) {
      case GeomAbs_Line:
      case GeomAbs_Circle:
      case GeomAbs_Ellipse:
      case GeomAbs_BezierCurve:
      case GeomAbs_BSplineCurve:
        break;
      default:
        return false;
      }
    }
    return true;
  default:
    return false;
  }
}

// A box that holds a face of a surface of linear extrusion (mitcad#311),
// where OCCT's box is sampled: the face's parameters along the curve and
// the direction are those of its boundary, so the face lies in the hull of
// the curve's stretch (its ends for a line, the poles of its segment for a
// B-spline or Bezier curve) moved to both ends of the face's stretch along
// the direction. False for other faces and other curves. The parameters
// are widened by a millionth of their span and the box by the face's
// largest tolerance.
bool extrusion_box(const TopoDS_Face& face, Bnd_Box& box) {
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  if (const auto trimmed = Handle(Geom_RectangularTrimmedSurface)::DownCast(surface)) {
    surface = trimmed->BasisSurface();
  }
  const auto extrusion = Handle(Geom_SurfaceOfLinearExtrusion)::DownCast(surface);
  if (extrusion.IsNull()) {
    return false;
  }
  double u0 = 0.0, u1 = 0.0, v0 = 0.0, v1 = 0.0;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  if (!std::isfinite(u0) || !std::isfinite(u1) || !std::isfinite(v0) || !std::isfinite(v1) ||
      u1 < u0 || v1 < v0) {
    return false;
  }
  const double du = (u1 - u0) * 1e-6 + Precision::PConfusion();
  const double dv = (v1 - v0) * 1e-6 + Precision::PConfusion();
  u0 -= du;
  u1 += du;
  v0 -= dv;
  v1 += dv;
  Handle(Geom_Curve) curve = extrusion->BasisCurve();
  while (const auto trimmed = Handle(Geom_TrimmedCurve)::DownCast(curve)) {
    curve = trimmed->BasisCurve();
  }
  std::vector<gp_Pnt> hull;
  if (const auto line = Handle(Geom_Line)::DownCast(curve)) {
    hull = {line->Value(u0), line->Value(u1)};
  } else if (const auto spline = Handle(Geom_BSplineCurve)::DownCast(curve)) {
    const auto copy = Handle(Geom_BSplineCurve)::DownCast(spline->Copy());
    const double first = copy->FirstParameter();
    const double last = copy->LastParameter();
    if (!copy->IsPeriodic()) {
      // The curve's own poles hold all of it.
      const double a = std::max(u0, first);
      const double b = std::min(u1, last);
      if (b - a > Precision::PConfusion() && (a > first || b < last)) {
        copy->Segment(a, b);
      }
    } else if (u1 - u0 < copy->Period()) {
      copy->Segment(u0, u1);
    }
    for (int i = 1; i <= copy->NbPoles(); ++i) {
      hull.push_back(copy->Pole(i));
    }
  } else if (const auto bezier = Handle(Geom_BezierCurve)::DownCast(curve)) {
    const auto copy = Handle(Geom_BezierCurve)::DownCast(bezier->Copy());
    const double a = std::max(u0, copy->FirstParameter());
    const double b = std::min(u1, copy->LastParameter());
    if (b - a > Precision::PConfusion() && (a > copy->FirstParameter() || b < copy->LastParameter())) {
      copy->Segment(a, b);
    }
    for (int i = 1; i <= copy->NbPoles(); ++i) {
      hull.push_back(copy->Pole(i));
    }
  } else {
    return false;
  }
  const gp_Vec along(extrusion->Direction());
  for (const gp_Pnt& p : hull) {
    box.Add(p.Translated(along * v0));
    box.Add(p.Translated(along * v1));
  }
  double tolerance = BRep_Tool::Tolerance(face);
  for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
    tolerance = std::max(tolerance, BRep_Tool::Tolerance(TopoDS::Edge(it.Current())));
  }
  for (TopExp_Explorer it(face, TopAbs_VERTEX); it.More(); it.Next()) {
    tolerance = std::max(tolerance, BRep_Tool::Tolerance(TopoDS::Vertex(it.Current())));
  }
  box.Enlarge(tolerance);
  return !box.IsVoid();
}

} // namespace

std::vector<std::array<double, 6>> face_boxes(const Shape& shape) {
  if (const std::shared_ptr<const Shape::FaceBoxes> kept = shape.kept_face_boxes()) {
    return *kept;
  }
  auto boxes = std::make_shared<const Shape::FaceBoxes>(detail::run("face boxes", [&] {
    std::vector<TopoDS_Face> each;
    for (int i = 0; i < shape.face_count(); ++i) {
      each.push_back(shape.face(i));
    }
    constexpr double kUnbounded = 1e100;
    Shape::FaceBoxes found(each.size(), {-kUnbounded, -kUnbounded, -kUnbounded, kUnbounded, kUnbounded,
                                         kUnbounded});
    OSD_Parallel::For(
        0, static_cast<int>(each.size()),
        [&](int i) {
          const auto k = static_cast<std::size_t>(i);
          try {
            Bnd_Box box;
            if (extrusion_box(each[k], box)) {
              // (Its own box, above.)
            } else if (!box_holds_face(each[k])) {
              return;
            } else {
              BRepBndLib::Add(each[k], box, false);
            }
            if (box.IsVoid() || box.IsOpen()) {
              return;
            }
            box.Enlarge(Precision::Confusion());
            double x0 = 0, y0 = 0, z0 = 0, x1 = 0, y1 = 0, z1 = 0;
            box.Get(x0, y0, z0, x1, y1, z1);
            found[k] = {x0, y0, z0, x1, y1, z1};
          } catch (...) {
            // (An unbounded box: the face is measured as before.)
          }
        },
        each.size() < 8);
    return found;
  }));
  shape.keep_face_boxes(boxes);
  return *boxes;
}

namespace {

// Up to `count` points inside a face, spread over it: the middles of the
// cells of a coarse grid over its parameters that lie inside it, of a fine
// one for narrow faces.
std::vector<gp_Pnt> inside_points(const TopoDS_Face& face, int count) {
  double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  BRepTopAdaptor_FClass2d classifier(face, Precision::PConfusion());
  std::vector<gp_Pnt2d> inside;
  for (const int cells : {6, 24}) {
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
  const std::size_t wanted = std::min(inside.size(), static_cast<std::size_t>(std::max(count, 0)));
  for (std::size_t k = 0; k < wanted; ++k) {
    const gp_Pnt2d& uv = inside[k * inside.size() / wanted];
    points.push_back(surface.Value(uv.X(), uv.Y()));
  }
  return points;
}

// What tells a face from another for new_faces: its kind of surface, area
// and centre.
struct FaceKey {
  GeomAbs_SurfaceType surface = GeomAbs_OtherSurface;
  double area = 0.0;
  gp_Pnt center;
};

FaceKey face_key(const TopoDS_Face& face) {
  GProp_GProps props;
  BRepGProp::SurfaceProperties(face, props);
  return {BRepAdaptor_Surface(face, false).GetType(), props.Mass(), props.CentreOfMass()};
}

// The same face rebuilt from the same data: the measures agree to within
// what integrating them again may change.
bool same_face(const FaceKey& a, const FaceKey& b) {
  constexpr double kArea = 1e-7;
  constexpr double kCenter = 1e-6;
  return a.surface == b.surface && std::abs(a.area - b.area) <= kArea * std::max(std::abs(a.area), 1.0) &&
         a.center.Distance(b.center) <= kCenter;
}

} // namespace

std::vector<FacePoints> face_points(const Shape& shape, int count) {
  return detail::run("faces", [&] {
    std::vector<FacePoints> result;
    for (int i = 0; i < shape.face_count(); ++i) {
      if (shape.face_names(i).empty()) {
        continue;
      }
      result.push_back({shape.face_names(i).front(), inside_points(shape.face(i), count)});
    }
    return result;
  });
}

std::vector<NewFace> new_faces(const std::vector<TopoDS_Shape>& before, const TopoDS_Shape& after,
                               int count) {
  return detail::run("new faces", [&] {
    NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> kept;
    for (const TopoDS_Shape& shape : before) {
      for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
        kept.Add(it.Current());
      }
    }
    // The faces of `after` that are not the same faces as those of
    // `before`; the measures of `before`'s only when there are such.
    std::vector<TopoDS_Face> others;
    NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> seen;
    for (TopExp_Explorer it(after, TopAbs_FACE); it.More(); it.Next()) {
      if (!kept.Contains(it.Current()) && seen.Add(it.Current())) {
        others.push_back(TopoDS::Face(it.Current()));
      }
    }
    std::vector<FaceKey> keys;
    if (!others.empty()) {
      for (NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(kept); it.More(); it.Next()) {
        keys.push_back(face_key(TopoDS::Face(it.Key())));
      }
      std::sort(keys.begin(), keys.end(), [](const FaceKey& a, const FaceKey& b) { return a.area < b.area; });
    }
    std::vector<NewFace> result;
    for (const TopoDS_Face& face : others) {
      const FaceKey key = face_key(face);
      // Those of about the same area, by the sorted areas.
      const double slack = 1e-7 * std::max(std::abs(key.area), 1.0);
      auto it = std::lower_bound(keys.begin(), keys.end(), key.area - slack,
                                 [](const FaceKey& k, double area) { return k.area < area; });
      bool found = false;
      for (; it != keys.end() && it->area <= key.area + slack && !found; ++it) {
        found = same_face(*it, key);
      }
      if (!found) {
        result.push_back({key.area, inside_points(face, count)});
      }
    }
    return result;
  });
}

namespace {

// The faces of `of` (each as it is used, with its orientation) that are
// not faces of `in`, as new_faces tells them: not the same face, nor one on
// the same kind of surface with the same area and centre.
std::vector<TopoDS_Face> faces_not_in(const std::vector<TopoDS_Shape>& of, const std::vector<TopoDS_Shape>& in) {
  NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> kept;
  for (const TopoDS_Shape& shape : in) {
    for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
      kept.Add(it.Current());
    }
  }
  std::vector<TopoDS_Face> others;
  for (const TopoDS_Shape& shape : of) {
    for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
      if (!kept.Contains(it.Current())) {
        others.push_back(TopoDS::Face(it.Current()));
      }
    }
  }
  if (others.empty()) {
    return others;
  }
  std::vector<FaceKey> keys;
  for (NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(kept); it.More(); it.Next()) {
    keys.push_back(face_key(TopoDS::Face(it.Key())));
  }
  std::sort(keys.begin(), keys.end(), [](const FaceKey& a, const FaceKey& b) { return a.area < b.area; });
  std::vector<TopoDS_Face> result;
  for (const TopoDS_Face& face : others) {
    const FaceKey key = face_key(face);
    const double slack = 1e-7 * std::max(std::abs(key.area), 1.0);
    auto it = std::lower_bound(keys.begin(), keys.end(), key.area - slack,
                               [](const FaceKey& k, double area) { return k.area < area; });
    bool found = false;
    for (; it != keys.end() && it->area <= key.area + slack && !found; ++it) {
      found = same_face(*it, key);
    }
    if (!found) {
      result.push_back(face);
    }
  }
  return result;
}

// A third of each edge's length times its tolerance beyond 1e-7 mm times
// its distance from `at`: the volume a strip as wide as the tolerance along
// the edge may add to a face's integral about `at`.
double edge_slack(const std::vector<TopoDS_Face>& faces, const gp_Pnt& at) {
  constexpr double kTight = 1e-7;
  double slack = 0.0;
  for (const TopoDS_Face& face : faces) {
    for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
      const TopoDS_Edge& edge = TopoDS::Edge(it.Current());
      const double loose = BRep_Tool::Tolerance(edge) - kTight;
      if (loose <= 0.0 || BRep_Tool::Degenerated(edge)) {
        continue;
      }
      const BRepAdaptor_Curve curve(edge);
      const double length = GCPnts_AbscissaPoint::Length(curve);
      const gp_Pnt middle = curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2.0);
      slack += length * loose * middle.Distance(at) / 3.0;
    }
  }
  return slack;
}

} // namespace

ChangedVolume changed_volume(const std::vector<TopoDS_Shape>& before, const std::vector<TopoDS_Shape>& after) {
  return detail::run("changed volume", [&] {
    Bnd_Box box;
    for (const TopoDS_Shape& shape : after) {
      BRepBndLib::Add(shape, box);
    }
    for (const TopoDS_Shape& shape : before) {
      BRepBndLib::Add(shape, box);
    }
    const gp_Pnt at = box.IsVoid() ? gp::Origin() : gp_Pnt((box.CornerMin().XYZ() + box.CornerMax().XYZ()) / 2.0);
    const std::vector<TopoDS_Face> added = faces_not_in(after, before);
    const std::vector<TopoDS_Face> removed = faces_not_in(before, after);
    ChangedVolume result;
    result.volume = analysis::faces_volume(added, at).Mass() - analysis::faces_volume(removed, at).Mass();
    result.slack = edge_slack(added, at) + edge_slack(removed, at);
    return result;
  });
}

} // namespace mitcad::geometry
