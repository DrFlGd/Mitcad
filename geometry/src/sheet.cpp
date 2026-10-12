// SPDX-License-Identifier: MIT
#include "mitcad/geometry/sheet.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <set>
#include <stdexcept>
#include <utility>

#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <Precision.hxx>
#include <gp_Vec.hxx>

#include "history.hpp"
#include "mitcad/geometry/query.hpp"
#include "sweep.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

constexpr double kAngularTolerance = 1.0e-9;
constexpr double kTwoPi = 6.28318530717958647692;

void finite_point(const gp_Pnt2d& point) {
  detail::require_finite("sheet curve coordinate", point.X());
  detail::require_finite("sheet curve coordinate", point.Y());
}

void check_frame(const Frame& frame) {
  for (int i = 1; i <= 3; ++i) {
    detail::require_finite("sheet frame origin", frame.origin.Coord(i));
    detail::require_finite("sheet frame axis", frame.x_axis.Coord(i));
    detail::require_finite("sheet frame axis", frame.y_axis.Coord(i));
  }
  if (std::abs(frame.x_axis.Dot(frame.y_axis)) > kAngularTolerance) {
    throw std::invalid_argument("sheet frame axes must be orthogonal");
  }
}

// Validate the used data before OCCT receives it. Work on a value copy:
// normalising angular representatives never changes the model's domains.
Region checked_region(const SheetPlanarPatch& patch) {
  Region region = patch.region;
  region.name = patch.feature + ".p" + std::to_string(patch.slot);
  for (Loop& loop : region.loops) {
    throw_if_cancelled();
    for (Segment& segment : loop.segments) {
      if (segment.name.empty()) {
        throw std::invalid_argument("sheet domain has a missing curve identity");
      }
      switch (segment.kind) {
      case SegmentKind::Line:
        finite_point(segment.start);
        finite_point(segment.end);
        break;
      case SegmentKind::Arc:
      case SegmentKind::Circle:
      case SegmentKind::Ellipse:
      case SegmentKind::EllipseArc:
        finite_point(segment.center);
        detail::require_positive("sheet curve radius", segment.radius);
        if (segment.kind == SegmentKind::Ellipse || segment.kind == SegmentKind::EllipseArc) {
          detail::require_positive("sheet curve minor radius", segment.minor_radius);
          detail::require_finite("sheet curve rotation", segment.rotation);
        }
        if (segment.kind == SegmentKind::Arc || segment.kind == SegmentKind::EllipseArc) {
          detail::require_finite("sheet arc start angle", segment.start_angle);
          detail::require_finite("sheet arc end angle", segment.end_angle);
          const double span = segment.end_angle - segment.start_angle;
          if (std::abs(span) > kTwoPi + kAngularTolerance) {
            throw std::invalid_argument("a sheet arc cannot wind more than once");
          }
          // Preserve a full periodic sweep (including equal-angle and -2pi
          // representations). Reducing the endpoints separately can turn a
          // full circle into a tiny arc through rounding at the seam.
          const double turn_roundoff = 8 * std::numeric_limits<double>::epsilon() * kTwoPi;
          double sweep = std::fmod(span, kTwoPi);
          if (span == 0.0 || std::abs(std::abs(span) - kTwoPi) <= turn_roundoff) {
            sweep = kTwoPi;
          } else if (sweep <= 0.0) {
            sweep += kTwoPi;
          }
          segment.start_angle = std::fmod(segment.start_angle, kTwoPi);
          segment.end_angle = segment.start_angle + sweep;
        }
        break;
      case SegmentKind::BSpline:
        for (const gp_Pnt2d& pole : segment.poles) {
          finite_point(pole);
        }
        for (double weight : segment.weights) {
          detail::require_positive("sheet spline weight", weight);
        }
        for (std::size_t i = 0; i < segment.knots.size(); ++i) {
          detail::require_finite("sheet spline knot", segment.knots[i]);
          if (i > 0 && segment.knots[i] <= segment.knots[i - 1]) {
            throw std::invalid_argument("sheet spline knots must be strictly increasing");
          }
        }
        for (int multiplicity : segment.multiplicities) {
          if (multiplicity <= 0) {
            throw std::invalid_argument("sheet spline knot multiplicities must be positive");
          }
        }
        break;
      }
    }
  }
  return region;
}

std::string cap(const SheetPlanarPatch& patch, const char* role) {
  return face_name(patch.feature, role, std::to_string(patch.slot));
}

ShapePtr panel(const SheetPlanarPatch& patch, const SheetShapeSpec& spec) {
  Frame frame = patch.frame;
  if (spec.reference_side == SheetReferenceSide::Back) {
    frame.origin.Translate(-spec.thickness * gp_Vec(frame.normal()));
  }
  const ProfileFace profile = make_profile(frame, checked_region(patch), spec.tolerance);
  // These are fresh faces owned only by this construction. No cached input
  // shape, curve or material-domain record is modified by the prism builder.
  BRepPrimAPI_MakePrism prism(profile.face, spec.thickness * gp_Vec(frame.normal()));
  detail::build(prism);
  if (!prism.IsDone()) {
    throw std::runtime_error("sheet panel extrusion failed");
  }
  detail::FaceNamer namer(prism.Shape());
  namer.add(prism.FirstShape(), cap(patch, "sheet_front"));
  namer.add(prism.LastShape(), cap(patch, "sheet_back"));
  for (const auto& [edge, curve] : profile.edges) {
    namer.generated(prism, edge,
                    face_name(patch.feature, "sheet_side" + std::to_string(patch.slot), curve));
  }
  namer.finish();
  detail::require_valid(namer.result(), "sheet panel");
  return namer.shape();
}

} // namespace

SheetGeometry sheet_shape(const SheetShapeSpec& spec) {
  return detail::run("sheet shape", [&] {
    detail::require_positive("sheet tolerance", spec.tolerance);
    detail::require_positive("sheet thickness", spec.thickness);
    if (spec.thickness <= std::max(spec.tolerance, Precision::Confusion())) {
      throw std::invalid_argument("sheet thickness must exceed the modelling tolerance");
    }
    if (spec.reference_side != SheetReferenceSide::Front && spec.reference_side != SheetReferenceSide::Back) {
      throw std::invalid_argument("unknown sheet reference side");
    }
    if (spec.planar_patches.empty()) {
      throw std::invalid_argument("sheet shape has no material domains");
    }
    std::set<std::pair<std::string, std::uint32_t>> identities;
    std::vector<ShapePtr> panels;
    double material_volume = 0.0;
    double material_area = 0.0;
    for (const SheetPlanarPatch& patch : spec.planar_patches) {
      throw_if_cancelled();
      if (patch.feature.empty() || !identities.emplace(patch.feature, patch.slot).second) {
        throw std::invalid_argument("sheet shape has missing or duplicate domain identities");
      }
      check_frame(patch.frame);
      const Frame& first = spec.planar_patches.front().frame;
      if (!patch.frame.normal().IsEqual(first.normal(), kAngularTolerance) ||
          std::abs(gp_Vec(first.origin, patch.frame.origin).Dot(gp_Vec(first.normal()))) > spec.tolerance) {
        throw std::invalid_argument("sheet base panels must be coplanar with the same material direction");
      }
      const ShapePtr shape = panel(patch, spec);
      const MassProperties properties = mass_properties(*shape);
      detail::require_positive("sheet material volume", properties.volume);
      detail::require_positive("sheet material area", properties.area);
      material_volume += properties.volume;
      material_area += properties.area;
      panels.push_back(shape);
    }
    // Only fresh construction results enter this union. The named aliases
    // retain the domain-to-solid correspondence even when coplanar caps merge.
    const ShapePtr joined = detail::finished(*detail::fused(panels, "sheet shape"), "sheet shape");
    const double volume_tolerance = std::max(spec.tolerance * material_area, 1.0e-9 * material_volume);
    if (material_volume - volume(*joined) > volume_tolerance) {
      throw std::invalid_argument("sheet material domains overlap");
    }
    SheetGeometry result;
    std::vector<bool> seen(spec.planar_patches.size(), false);
    for (const ShapePtr& shape : solids(*joined)) {
      throw_if_cancelled();
      detail::require_valid(shape->occt(), "sheet solid");
      if (volume(*shape) <= 0.0) {
        throw std::runtime_error("sheet domain did not form a positive-volume solid");
      }
      SheetSolid solid{shape, {}};
      for (std::size_t i = 0; i < spec.planar_patches.size(); ++i) {
        const SheetPlanarPatch& patch = spec.planar_patches[i];
        if (!shape->find_faces(cap(patch, "sheet_front")).empty()) {
          if (seen[i] || shape->find_faces(cap(patch, "sheet_back")).empty()) {
            throw std::runtime_error("sheet domain has inconsistent solid membership");
          }
          seen[i] = true;
          solid.domains.push_back(i);
        }
      }
      if (solid.domains.empty()) {
        throw std::runtime_error("sheet solid has no material domain");
      }
      result.solids.push_back(std::move(solid));
    }
    if (std::find(seen.begin(), seen.end(), false) != seen.end()) {
      throw std::runtime_error("sheet construction lost a material domain");
    }
    // Point/edge contact does not form a manifold sheet. A whole-side join
    // has already become one solid; all remaining islands must be separate.
    for (std::size_t i = 0; i < result.solids.size(); ++i) {
      for (std::size_t j = i + 1; j < result.solids.size(); ++j) {
        detail::Interrupt interrupt;
        BRepExtrema_DistShapeShape distance(result.solids[i].shape->occt(), result.solids[j].shape->occt(),
                                            Extrema_ExtFlag_MINMAX, Extrema_ExtAlgo_Grad, interrupt.range());
        throw_if_cancelled();
        if (!distance.IsDone()) {
          throw std::runtime_error("sheet island separation could not be checked");
        }
        if (distance.Value() <= spec.tolerance) {
          throw std::invalid_argument("sheet islands have zero-width material contact");
        }
      }
    }
    // Input order is the model's canonical domain order, not OCCT traversal
    // or the islands' current positions. Parameter edits cannot swap bodies.
    std::sort(result.solids.begin(), result.solids.end(), [](const SheetSolid& a, const SheetSolid& b) {
      return a.domains < b.domains;
    });
    return result;
  });
}

} // namespace mitcad::geometry
