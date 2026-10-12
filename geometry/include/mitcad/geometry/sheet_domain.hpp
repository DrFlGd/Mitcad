// SPDX-License-Identifier: MIT
#pragma once

// Value-only transport for named sheet material charts. Tokens belong to one
// kernel invocation; the model owns persistent identities and body allocation.

#include <cstdint>
#include <string>
#include <vector>

#include <gp_Vec2d.hxx>

#include "mitcad/geometry/profile.hpp"

namespace mitcad::geometry {

enum class SheetDomainKind { Panel, Bend };

struct SheetDomainRef {
  SheetDomainKind kind = SheetDomainKind::Panel;
  std::string feature;
  std::uint32_t slot = 0;
  bool operator==(const SheetDomainRef& other) const {
    return kind == other.kind && feature == other.feature && slot == other.slot;
  }
  bool operator!=(const SheetDomainRef& other) const { return !(*this == other); }
};

struct ParamSpan {
  double first = 0.0;
  double last = 1.0;
};

struct CurvePieceRef {
  // Opaque, feature-qualified curve role, shared with the topological namer.
  std::string curve;
  std::uint32_t piece = 0;
  bool operator==(const CurvePieceRef& other) const {
    return curve == other.curve && piece == other.piece;
  }
  bool operator!=(const CurvePieceRef& other) const { return !(*this == other); }
};

struct BoundaryPieceRef {
  SheetDomainRef domain;
  CurvePieceRef piece;
  bool operator==(const BoundaryPieceRef& other) const {
    return domain == other.domain && piece == other.piece;
  }
  bool operator!=(const BoundaryPieceRef& other) const { return !(*this == other); }
};

struct BoundedCurve2 {
  // The actual support of this edge, not its ancestor's parameterization.
  // Line uses [0,1]; conics use unwrapped radians; splines use knot parameters.
  // Descending range reverses traversal. Segment::name is not authoritative.
  Segment geometry;
  ParamSpan range;
};

struct MaterialEdge {
  CurvePieceRef piece;
  BoundedCurve2 curve;
};

struct MaterialLoop {
  std::vector<MaterialEdge> edges;
};

struct MaterialRegion {
  // Exterior first, followed by holes. Every edge follows the wire traversal.
  std::vector<MaterialLoop> loops;
};

struct Isometry2 {
  gp_Pnt2d origin{0.0, 0.0};
  gp_Vec2d x_axis{1.0, 0.0};
  gp_Vec2d y_axis{0.0, 1.0};
};

struct AxialMap {
  double offset = 0.0;
  double scale = 1.0;
};

struct TrimDomain {
  SheetDomainRef domain;
  std::vector<MaterialRegion> regions;
  Isometry2 to_operation;
};

struct TrimSeamSide {
  BoundaryPieceRef boundary;
  ParamSpan span;
  AxialMap axial;
};

struct TrimSeam {
  std::uint32_t token = 0;
  TrimSeamSide first;
  TrimSeamSide second;
  ParamSpan axial_interval;
};

enum class SheetTrimOperation { Union, Difference, Intersection };

struct SheetTrimSpec {
  SheetTrimOperation operation = SheetTrimOperation::Difference;
  std::vector<TrimDomain> domains;
  std::vector<MaterialRegion> tools;
  std::vector<TrimSeam> seams;
  double tolerance = 1.0e-7;
};

enum class TrimSourceKind { Domain, Tool };

struct TrimSource {
  TrimSourceKind kind = TrimSourceKind::Domain;
  // Domain is unused for a tool; piece always identifies the source support.
  SheetDomainRef domain;
  CurvePieceRef piece;
  bool operator==(const TrimSource& other) const {
    return kind == other.kind && domain == other.domain && piece == other.piece;
  }
  bool operator!=(const TrimSource& other) const { return !(*this == other); }
};

struct SourceSpan {
  TrimSource source;
  // Coverage in the INPUT support parameter, not an output-to-input map.
  ParamSpan span;
};

struct TrimEdge {
  std::uint32_t token = 0;
  BoundedCurve2 curve;
  std::vector<SourceSpan> lineage;
};

struct TrimLoop {
  std::vector<TrimEdge> edges;
};

struct TrimRegion {
  std::uint32_t token = 0;
  std::vector<TrimLoop> loops;
};

struct TrimDomainResult {
  SheetDomainRef domain;
  std::vector<TrimRegion> regions;
};

struct TrimEndpoint {
  SheetDomainRef domain;
  std::uint32_t edge_token = 0;
  double parameter = 0.0;
};

enum class SeamEventKind { Crossing, Tangency, Endpoint };

struct SeamSplit {
  std::uint32_t seam_token = 0;
  std::uint32_t split_token = 0;
  double axial = 0.0;
  SeamEventKind kind = SeamEventKind::Endpoint;
  std::vector<TrimEndpoint> first;
  std::vector<TrimEndpoint> second;
};

struct TrimEndpointSpan {
  SheetDomainRef domain;
  std::uint32_t edge_token = 0;
  ParamSpan range;
};

struct TrimPortSpan {
  std::uint32_t seam_token = 0;
  ParamSpan axial_interval;
  TrimEndpointSpan first;
  TrimEndpointSpan second;
};

struct SheetTrimResult {
  std::vector<TrimDomainResult> domains;
  std::vector<SeamSplit> splits;
  std::vector<TrimPortSpan> ports;
  std::vector<std::string> diagnostics;
};

} // namespace mitcad::geometry
