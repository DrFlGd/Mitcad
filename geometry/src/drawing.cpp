// SPDX-License-Identifier: MIT
#include "mitcad/geometry/drawing.hpp"

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <HLRAlgo_Projector.hxx>
#include <HLRBRep_Algo.hxx>
#include <HLRBRep_HLRToShape.hxx>
#include <HLRBRep_PolyAlgo.hxx>
#include <HLRBRep_PolyHLRToShape.hxx>
#include <Precision.hxx>
#include <Standard_Failure.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Wire.hxx>
#include <gp.hxx>
#include <gp_Ax2.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Vec.hxx>

#include "mitcad/analysis/section.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kTurn = 2.0 * kPi;

// The view's 2D coordinates of a model point.
struct ViewFrame {
  gp_Dir x;
  gp_Dir y;

  Point2 project(const gp_Pnt& p) const {
    const gp_Vec v(p.XYZ());
    return {v.Dot(gp_Vec(x)), v.Dot(gp_Vec(y))};
  }
};

analysis::Plane analysis_plane(const DrawingSection& section) {
  analysis::Plane plane;
  plane.origin = {section.origin.X(), section.origin.Y(), section.origin.Z()};
  plane.normal = {section.normal.X(), section.normal.Y(), section.normal.Z()};
  return plane;
}

double normalized_angle(double a) {
  a = std::fmod(a, kTurn);
  if (a < 0.0) {
    a += kTurn;
  }
  if (a > kTurn - 1e-12) {
    a = 0.0;
  }
  return a;
}

// Points along a curve within the deflection, in its own direction.
std::vector<gp_Pnt> sample(const BRepAdaptor_Curve& curve, double first, double last, double deflection) {
  std::vector<gp_Pnt> points;
  const GCPnts_TangentialDeflection sampler(curve, first, last, 0.1, deflection, 2);
  for (int i = 1; i <= sampler.NbPoints(); ++i) {
    points.push_back(sampler.Value(i));
  }
  if (points.size() < 2) {
    points = {curve.Value(first), curve.Value(last)};
  }
  return points;
}

// One edge of a projection: its curve lies in the view's plane (x, y, 0).
void add_projected(const TopoDS_Edge& edge, bool visible, DrawnKind kind, double deflection, DrawnView& view) {
  if (BRep_Tool::Degenerated(edge)) {
    return;
  }
  BRepAdaptor_Curve curve(edge);
  const double first = curve.FirstParameter();
  const double last = curve.LastParameter();
  if (!(last > first)) {
    return;
  }
  DrawnCurve drawn;
  drawn.visible = visible;
  drawn.kind = kind;
  const auto flat = [](const gp_Pnt& p) { return Point2{p.X(), p.Y()}; };
  switch (curve.GetType()) {
  case GeomAbs_Line: {
    drawn.type = DrawnCurve::Type::Line;
    drawn.points = {flat(curve.Value(first)), flat(curve.Value(last))};
    if (std::hypot(drawn.points[1][0] - drawn.points[0][0], drawn.points[1][1] - drawn.points[0][1]) <
        Precision::Confusion()) {
      return;
    }
    break;
  }
  case GeomAbs_Circle: {
    const gp_Circ circle = curve.Circle();
    drawn.type = DrawnCurve::Type::Arc;
    drawn.center = flat(circle.Location());
    drawn.radius = circle.Radius();
    const double sweep = last - first;
    if (sweep >= kTurn - 1e-9) {
      drawn.start = 0.0;
      drawn.end = kTurn;
      break;
    }
    const gp_Pnt a = curve.Value(first);
    const gp_Pnt b = curve.Value(last);
    // The parameter runs counter-clockwise in the view when the circle's
    // axis points to the eye (+Z of the view's plane).
    const gp_Pnt from = circle.Axis().Direction().Z() > 0.0 ? a : b;
    drawn.start = normalized_angle(std::atan2(from.Y() - drawn.center[1], from.X() - drawn.center[0]));
    drawn.end = drawn.start + sweep;
    break;
  }
  default: {
    drawn.type = DrawnCurve::Type::Polyline;
    for (const gp_Pnt& p : sample(curve, first, last, deflection)) {
      drawn.points.push_back(flat(p));
    }
    break;
  }
  }
  view.curves.push_back(std::move(drawn));
}

void add_compound(const TopoDS_Shape& compound, bool visible, DrawnKind kind, double deflection, DrawnView& view) {
  if (compound.IsNull()) {
    return;
  }
  for (TopExp_Explorer it(compound, TopAbs_EDGE); it.More(); it.Next()) {
    add_projected(TopoDS::Edge(it.Current()), visible, kind, deflection, view);
  }
}

template <class Output>
void collect(Output& out, const DrawingViewSpec& spec, DrawnView& view) {
  add_compound(out.VCompound(), true, DrawnKind::Sharp, spec.deflection, view);
  add_compound(out.OutLineVCompound(), true, DrawnKind::Outline, spec.deflection, view);
  if (spec.smooth) {
    add_compound(out.Rg1LineVCompound(), true, DrawnKind::Smooth, spec.deflection, view);
  }
  if (spec.hidden) {
    add_compound(out.HCompound(), false, DrawnKind::Sharp, spec.deflection, view);
    add_compound(out.OutLineHCompound(), false, DrawnKind::Outline, spec.deflection, view);
    if (spec.smooth) {
      add_compound(out.Rg1LineHCompound(), false, DrawnKind::Smooth, spec.deflection, view);
    }
  }
}

void project_exact(const std::vector<TopoDS_Shape>& parts, const HLRAlgo_Projector& projector,
                   const DrawingViewSpec& spec, DrawnView& view) {
  occ::handle<HLRBRep_Algo> algo = new HLRBRep_Algo();
  for (const TopoDS_Shape& part : parts) {
    algo->Add(part);
  }
  algo->Projector(projector);
  algo->Update();
  algo->Hide();
  HLRBRep_HLRToShape out(algo);
  collect(out, spec, view);
}

// The polygonal algorithm works on triangles: of copies of the shapes, so
// that the shapes themselves stay as they are.
void project_polygonal(const std::vector<TopoDS_Shape>& parts, const HLRAlgo_Projector& projector,
                       const DrawingViewSpec& spec, DrawnView& view) {
  occ::handle<HLRBRep_PolyAlgo> algo = new HLRBRep_PolyAlgo();
  for (const TopoDS_Shape& part : parts) {
    const bool mesh = detail::has_mesh_faces(part);
    TopoDS_Shape copy = BRepBuilderAPI_Copy(part, true, mesh).Shape();
    if (!mesh) {
      Bnd_Box box;
      BRepBndLib::Add(part, box);
      const double size = box.IsVoid() ? 1.0 : std::sqrt(box.SquareExtent());
      BRepMesh_IncrementalMesh mesher(copy, std::max(spec.deflection, size * 1e-3), false, 0.2, false);
    }
    algo->Load(copy);
  }
  algo->Projector(projector);
  algo->Update();
  HLRBRep_PolyHLRToShape out;
  out.Update(algo);
  collect(out, spec, view);
}

// The loops of the faces where the plane cuts the shape, in view
// coordinates.
void add_cuts(const TopoDS_Shape& shape, const DrawingSection& section, const ViewFrame& frame,
              double deflection, DrawnView& view) {
  const analysis::Section cut = analysis::section(shape, analysis_plane(section));
  if (cut.faces.IsNull()) {
    return;
  }
  for (TopExp_Explorer faces(cut.faces, TopAbs_FACE); faces.More(); faces.Next()) {
    const TopoDS_Face face = TopoDS::Face(faces.Current());
    CutFace loops;
    for (TopExp_Explorer wires(face, TopAbs_WIRE); wires.More(); wires.Next()) {
      std::vector<Point2> loop;
      for (BRepTools_WireExplorer edges(TopoDS::Wire(wires.Current()), face); edges.More(); edges.Next()) {
        const TopoDS_Edge& edge = edges.Current();
        if (BRep_Tool::Degenerated(edge)) {
          continue;
        }
        const BRepAdaptor_Curve curve(edge);
        std::vector<gp_Pnt> points = sample(curve, curve.FirstParameter(), curve.LastParameter(), deflection);
        if (edge.Orientation() == TopAbs_REVERSED) {
          std::reverse(points.begin(), points.end());
        }
        for (const gp_Pnt& p : points) {
          const Point2 q = frame.project(p);
          if (loop.empty() || std::hypot(q[0] - loop.back()[0], q[1] - loop.back()[1]) > 1e-9) {
            loop.push_back(q);
          }
        }
      }
      if (loop.size() > 2 &&
          std::hypot(loop.front()[0] - loop.back()[0], loop.front()[1] - loop.back()[1]) <= 1e-9) {
        loop.pop_back();
      }
      if (loop.size() > 2) {
        loops.push_back(std::move(loop));
      }
    }
    if (!loops.empty()) {
      view.cuts.push_back(std::move(loops));
    }
  }
}

// --- Round holes -----------------------------------------------------------

constexpr double kAxisTolerance = 1e-6;

// An axis line in a canonical form: its direction's first non-zero
// component of (z, y, x) positive, through the foot of the origin.
struct Line {
  gp_Dir direction;
  gp_Pnt foot;

  static Line of(const gp_Ax1& axis) {
    gp_Dir d = axis.Direction();
    const double key = std::abs(d.Z()) > 1e-9 ? d.Z() : (std::abs(d.Y()) > 1e-9 ? d.Y() : d.X());
    if (key < 0.0) {
      d.Reverse();
    }
    const gp_Vec v(axis.Location().XYZ());
    const gp_Pnt foot(axis.Location().XYZ() - d.XYZ() * v.Dot(gp_Vec(d)));
    return {d, foot};
  }

  bool same(const Line& other) const {
    return direction.IsParallel(other.direction, 1e-9) && foot.Distance(other.foot) < kAxisTolerance;
  }
  double at(const gp_Pnt& p) const { return gp_Vec(foot, p).Dot(gp_Vec(direction)); }
  gp_Pnt point(double t) const { return foot.Translated(gp_Vec(direction) * t); }
  double distance(const gp_Pnt& p) const {
    const gp_Pnt on = point(at(p));
    return on.Distance(p);
  }
};

// A coaxial cylinder or cone with the material outside it: the faces of one
// surface over one range along the axis.
struct Wall {
  bool cone = false;
  double low = 0.0;  // along the line
  double high = 0.0;
  double radius_low = 0.0;
  double radius_high = 0.0;
  double span = 0.0; // the angle the faces cover together
  double half_angle = 0.0; // a cone's
  int face = -1;     // the first face
};

struct Axis {
  Line line;
  std::vector<Wall> walls;
};

// Whether the material lies outside the surface at the face's middle.
bool material_outside(const TopoDS_Face& face, const Line& line) {
  const BRepAdaptor_Surface surface(face);
  double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  gp_Pnt point;
  gp_Vec du;
  gp_Vec dv;
  surface.D1((u0 + u1) / 2.0, (v0 + v1) / 2.0, point, du, dv);
  const gp_Vec cross = du.Crossed(dv);
  if (cross.Magnitude() < Precision::Confusion()) {
    return false;
  }
  gp_Dir normal(cross);
  if (face.Orientation() == TopAbs_REVERSED) {
    normal.Reverse();
  }
  const gp_Vec radial(line.point(line.at(point)), point);
  return radial.Dot(gp_Vec(normal)) < 0.0;
}

std::vector<Axis> hole_walls(const Shape& shape) {
  std::vector<Axis> axes;
  for (int i = 0; i < shape.face_count(); ++i) {
    const TopoDS_Face& face = shape.face(i);
    const BRepAdaptor_Surface surface(face);
    const GeomAbs_SurfaceType type = surface.GetType();
    if (type != GeomAbs_Cylinder && type != GeomAbs_Cone) {
      continue;
    }
    const gp_Ax1 axis = type == GeomAbs_Cylinder ? surface.Cylinder().Axis() : surface.Cone().Axis();
    const Line line = Line::of(axis);
    if (!material_outside(face, line)) {
      continue;
    }
    double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    Wall wall;
    wall.cone = type == GeomAbs_Cone;
    wall.span = u1 - u0;
    wall.face = i;
    const gp_Pnt a = surface.Value(u0, v0);
    const gp_Pnt b = surface.Value(u0, v1);
    double ta = line.at(a);
    double tb = line.at(b);
    double ra = line.distance(a);
    double rb = line.distance(b);
    if (ta > tb) {
      std::swap(ta, tb);
      std::swap(ra, rb);
    }
    wall.low = ta;
    wall.high = tb;
    wall.radius_low = ra;
    wall.radius_high = rb;
    if (wall.cone) {
      wall.half_angle = std::abs(surface.Cone().SemiAngle());
    }
    auto found = std::find_if(axes.begin(), axes.end(), [&](const Axis& other) { return other.line.same(line); });
    if (found == axes.end()) {
      axes.push_back(Axis{line, {}});
      found = axes.end() - 1;
    }
    // The pieces of a split surface over the same range are one wall.
    auto same = std::find_if(found->walls.begin(), found->walls.end(), [&](const Wall& other) {
      return other.cone == wall.cone && std::abs(other.low - wall.low) < kAxisTolerance &&
             std::abs(other.high - wall.high) < kAxisTolerance &&
             std::abs(other.radius_low - wall.radius_low) < kAxisTolerance &&
             std::abs(other.radius_high - wall.radius_high) < kAxisTolerance;
    });
    if (same != found->walls.end()) {
      same->span += wall.span;
    } else {
      found->walls.push_back(wall);
    }
  }
  for (Axis& axis : axes) {
    axis.walls.erase(std::remove_if(axis.walls.begin(), axis.walls.end(),
                                    [](const Wall& w) { return w.span < kTurn - 1e-6; }),
                     axis.walls.end());
  }
  return axes;
}

// Whether material lies on the line within `reach` beyond `end` along
// `outward`.
bool closed_beyond(const std::vector<TopoDS_Shape>& solids, const gp_Pnt& end, const gp_Dir& outward, double reach) {
  constexpr int kSteps = 16;
  for (int k = 1; k <= kSteps; ++k) {
    const gp_Pnt p = end.Translated(gp_Vec(outward) * (reach * k / kSteps));
    for (const TopoDS_Shape& solid : solids) {
      BRepClass3d_SolidClassifier classifier(solid, p, Precision::Confusion());
      if (classifier.State() == TopAbs_IN) {
        return true;
      }
    }
  }
  return false;
}

// What lies on one side of a drilled wall: a counterbore or a countersink
// meeting it end to end, and where the hole then ends.
struct Side {
  const Wall* wider = nullptr;
  double end = 0.0; // along the line
  bool closed = false;
};

Side side_of(const Axis& axis, const Wall& drill, bool high, const std::vector<bool>& used,
             const std::vector<TopoDS_Shape>& solids) {
  Side side;
  const double at = high ? drill.high : drill.low;
  const double radius = high ? drill.radius_high : drill.radius_low;
  side.end = at;
  for (std::size_t i = 0; i < axis.walls.size(); ++i) {
    const Wall& w = axis.walls[i];
    if (&w == &drill || used[i]) {
      continue;
    }
    const double meet = high ? w.low : w.high;
    const double meet_radius = high ? w.radius_low : w.radius_high;
    const double outer_radius = high ? w.radius_high : w.radius_low;
    if (std::abs(meet - at) > kAxisTolerance) {
      continue;
    }
    // A wider cylinder (a counterbore, its floor between them) or a cone
    // widening from the wall (a countersink).
    const bool wider = w.cone ? std::abs(meet_radius - radius) < 1e-4 && outer_radius > radius + 1e-6
                              : meet_radius > radius + 1e-6;
    if (wider) {
      side.wider = &w;
      side.end = high ? w.high : w.low;
      break;
    }
  }
  gp_Dir outward = axis.line.direction;
  if (!high) {
    outward.Reverse();
  }
  side.closed = closed_beyond(solids, axis.line.point(side.end), outward, 4.0 * radius);
  return side;
}

} // namespace

DrawnView project_view(const std::vector<ShapePtr>& shapes, const DrawingViewSpec& spec) {
  return detail::run("drawing view", [&] {
    if (std::abs(spec.toward_eye.Dot(spec.x_axis)) > 1e-9) {
      throw std::invalid_argument("a view's x axis must be at right angles to its direction");
    }
    const gp_Ax2 axes(gp::Origin(), spec.toward_eye, spec.x_axis);
    const ViewFrame frame{axes.XDirection(), axes.YDirection()};
    const HLRAlgo_Projector projector(axes);
    DrawnView view;
    std::vector<TopoDS_Shape> parts;
    bool meshes = false;
    for (const ShapePtr& shape : shapes) {
      if (!shape) {
        continue;
      }
      TopoDS_Shape part = shape->occt();
      if (spec.section) {
        part = analysis::clip(part, analysis_plane(*spec.section));
        add_cuts(shape->occt(), *spec.section, frame, spec.deflection, view);
      }
      bool empty = true;
      for (TopExp_Explorer it(part, TopAbs_EDGE); it.More(); it.Next()) {
        empty = false;
        break;
      }
      if (part.IsNull() || empty) {
        continue;
      }
      meshes = meshes || detail::has_mesh_faces(part);
      parts.push_back(part);
    }
    if (parts.empty()) {
      return view;
    }
    if (spec.exact && !meshes) {
      DrawnView exact = view;
      try {
        project_exact(parts, projector, spec, exact);
        return exact;
      } catch (const Standard_Failure&) {
        // The polygonal algorithm below.
      }
    }
    project_polygonal(parts, projector, spec, view);
    view.polygonal = true;
    return view;
  });
}

std::vector<RoundHole> round_holes(const Shape& shape) {
  return detail::run("round holes", [&] {
    std::vector<TopoDS_Shape> solids;
    for (TopExp_Explorer it(shape.occt(), TopAbs_SOLID); it.More(); it.Next()) {
      solids.push_back(it.Current());
    }
    std::vector<RoundHole> holes;
    if (solids.empty()) {
      return holes;
    }
    for (const Axis& axis : hole_walls(shape)) {
      // The narrowest cylinders first: each is a drilled wall unless a
      // narrower one took it as its counterbore.
      std::vector<std::size_t> order(axis.walls.size());
      for (std::size_t i = 0; i < order.size(); ++i) {
        order[i] = i;
      }
      std::sort(order.begin(), order.end(), [&](std::size_t a, std::size_t b) {
        return axis.walls[a].radius_low < axis.walls[b].radius_low;
      });
      std::vector<bool> used(axis.walls.size(), false);
      for (const std::size_t i : order) {
        const Wall& drill = axis.walls[i];
        if (used[i] || drill.cone) {
          continue;
        }
        const Side low = side_of(axis, drill, false, used, solids);
        const Side high = side_of(axis, drill, true, used, solids);
        if (low.closed && high.closed) {
          continue; // a cavity inside the body
        }
        used[i] = true;
        // The side it opens to: the open one; of a hole open at both ends
        // the side of its counterbore or countersink, else the high end
        // (towards +Z, +Y or +X).
        bool from_high = !high.closed;
        if (!low.closed && !high.closed) {
          from_high = high.wider != nullptr || low.wider == nullptr;
        }
        const Side& open = from_high ? high : low;
        const Side& other = from_high ? low : high;
        RoundHole hole;
        const NameList& names = shape.face_names(drill.face);
        hole.face = names.empty() ? std::string() : names.front();
        hole.opening = axis.line.point(open.end);
        hole.direction = from_high ? axis.line.direction.Reversed() : axis.line.direction;
        hole.diameter = 2.0 * drill.radius_low;
        hole.through = !other.closed;
        const double bottom = from_high ? drill.low : drill.high;
        hole.depth = std::abs(open.end - bottom);
        if (open.wider != nullptr) {
          const Wall& wide = *open.wider;
          used[static_cast<std::size_t>(&wide - axis.walls.data())] = true;
          if (wide.cone) {
            hole.csink_diameter = 2.0 * std::max(wide.radius_low, wide.radius_high);
            hole.csink_angle = 2.0 * wide.half_angle;
          } else {
            hole.cbore_diameter = 2.0 * wide.radius_low;
            hole.cbore_depth = wide.high - wide.low;
          }
        }
        holes.push_back(hole);
      }
    }
    std::sort(holes.begin(), holes.end(), [](const RoundHole& a, const RoundHole& b) {
      const auto key = [](const RoundHole& h) {
        return std::array<long long, 4>{std::llround(h.opening.X() * 1e6), std::llround(h.opening.Y() * 1e6),
                                        std::llround(h.opening.Z() * 1e6), std::llround(h.diameter * 1e6)};
      };
      return key(a) < key(b);
    });
    return holes;
  });
}

} // namespace mitcad::geometry
