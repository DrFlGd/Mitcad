// SPDX-License-Identifier: MIT
#include "bridge/surface_edits.hpp"

#include <cstddef>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

#include "bridge/faceops.hpp"
#include "bridge/sweeps.hpp"
#include "mitcad/geometry/surface_edits.hpp"
#include "mitcad_bridge/kernel/surface_edits.h"

namespace mitcad::bridge {
namespace {

std::vector<std::string> strings(rust::Slice<const rust::String> values) {
  std::vector<std::string> result;
  for (const rust::String& value : values) {
    result.emplace_back(value);
  }
  return result;
}

} // namespace

std::shared_ptr<geometry::Shape> trim_surface(const geometry::Shape& body, const ToolSpec& tool,
                                              const ShapeList& tool_shapes, const Frame& frame,
                                              rust::Slice<const Region> regions,
                                              rust::Slice<const SweepCurve> curves, const TrimData& data) {
  geometry::TrimInput input;
  input.extend = data.extend;
  if (data.by_curves) {
    std::size_t next = 0;
    for (const std::size_t count : data.chains) {
      if (count > curves.size() - next) {
        throw std::invalid_argument("a chain of curves runs past the curves given");
      }
      geometry::Path chain;
      for (std::size_t i = 0; i < count; ++i, ++next) {
        chain.push_back(path_curve_of(curves[next]));
      }
      input.chains.push_back(std::move(chain));
    }
    input.direction = gp_Dir(data.direction[0], data.direction[1], data.direction[2]);
  } else {
    input.tool = tool_of(tool, tool_shapes, &frame, regions);
  }
  std::vector<gp_Pnt> keep;
  for (std::size_t i = 0; i + 2 < data.keep.size(); i += 3) {
    keep.emplace_back(data.keep[i], data.keep[i + 1], data.keep[i + 2]);
  }
  return geometry::trim_surface(body, input, keep);
}

std::shared_ptr<geometry::Shape> untrim_surface(rust::Str feature, const geometry::Shape& body, rust::Str face,
                                                bool holes, bool outer, bool in_place) {
  return geometry::untrim_surface(std::string(feature), body, std::string(face), holes, outer, in_place);
}

std::unique_ptr<ShapeList> unstitch(const geometry::Shape& body, rust::Slice<const rust::String> faces) {
  auto [rest, apart] = geometry::unstitch(body, strings(faces));
  auto list = std::make_unique<ShapeList>();
  list->push(rest);
  for (const auto& piece : apart) {
    list->push(piece);
  }
  return list;
}

std::shared_ptr<geometry::Shape> reverse_faces(const geometry::Shape& body, rust::Slice<const rust::String> faces) {
  return geometry::reverse_faces(body, strings(faces));
}

} // namespace mitcad::bridge
