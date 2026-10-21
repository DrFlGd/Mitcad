// SPDX-License-Identifier: MIT
#include "bridge/surface_offsets.hpp"

#include <cstddef>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <gp_Dir.hxx>

#include "mitcad/geometry/surface_offsets.hpp"
#include "mitcad_bridge/kernel/surface_offsets.h"

namespace mitcad::bridge {
namespace {

std::vector<std::string> strings(rust::Slice<const rust::String> values) {
  std::vector<std::string> result;
  for (const rust::String& value : values) {
    result.emplace_back(value);
  }
  return result;
}

template <class Openings>
std::vector<geometry::OpeningEdges> openings_of(const Openings& openings) {
  std::vector<geometry::OpeningEdges> result;
  for (const OpeningData& opening : openings) {
    geometry::OpeningEdges edges;
    edges.index = opening.index;
    for (const rust::String& face : opening.faces) {
      edges.faces.emplace_back(face);
    }
    for (std::size_t i = 0; i + 2 < opening.at.size(); i += 3) {
      edges.at.emplace_back(opening.at[i], opening.at[i + 1], opening.at[i + 2]);
    }
    result.push_back(std::move(edges));
  }
  return result;
}

} // namespace

std::shared_ptr<geometry::Shape> offset_surface(rust::Str feature, const geometry::Shape& body,
                                                rust::Slice<const rust::String> faces, double distance,
                                                bool chain) {
  return geometry::offset_surface(std::string(feature), body, strings(faces), distance, chain);
}

std::shared_ptr<geometry::Shape> ruled_surface(rust::Str feature, const geometry::Shape& body,
                                               const RuledData& spec) {
  geometry::RuledSpec ruled;
  ruled.feature = std::string(feature);
  for (const rust::String& edge : spec.edges) {
    ruled.edges.emplace_back(edge);
  }
  ruled.openings = openings_of(spec.openings);
  ruled.face = std::string(spec.face);
  ruled.distance = spec.distance;
  switch (spec.mode) {
  case 0:
    ruled.mode = geometry::RuledMode::Normal;
    break;
  case 1:
    ruled.mode = geometry::RuledMode::Tangent;
    break;
  case 2:
    ruled.mode = geometry::RuledMode::Angle;
    break;
  case 3:
    ruled.mode = geometry::RuledMode::Direction;
    break;
  default:
    throw std::invalid_argument("ruled surface: unknown mode " + std::to_string(spec.mode));
  }
  ruled.angle = spec.angle;
  if (spec.has_direction) {
    ruled.direction = gp_Dir(spec.direction[0], spec.direction[1], spec.direction[2]);
  }
  ruled.flip = spec.flip;
  ruled.corners = spec.corners == 1 ? geometry::RuledCorners::Round : geometry::RuledCorners::Mitre;
  return geometry::ruled_surface(body, ruled);
}

std::shared_ptr<geometry::Shape> extend_surface(const geometry::Shape& body, rust::Slice<const rust::String> edges,
                                                rust::Slice<const OpeningData> openings, double distance,
                                                std::uint8_t mode) {
  const geometry::ExtendMode extend = mode == 1   ? geometry::ExtendMode::Tangent
                                      : mode == 2 ? geometry::ExtendMode::Curvature
                                                  : geometry::ExtendMode::Natural;
  return geometry::extend_surface(body, strings(edges), openings_of(openings), distance, extend);
}

} // namespace mitcad::bridge
