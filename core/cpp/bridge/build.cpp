// SPDX-License-Identifier: MIT
#include "bridge/build.hpp"

#include <cstddef>
#include <map>
#include <stdexcept>
#include <string>

#include "mitcad/geometry/build.hpp"
#include "mitcad_bridge/kernel/build.h"

namespace mitcad::bridge {

std::shared_ptr<geometry::Shape> rename_faces(const geometry::Shape& shape, rust::Str prefix,
                                              rust::Slice<const rust::String> from,
                                              rust::Slice<const rust::String> to) {
  if (from.size() != to.size()) {
    throw std::invalid_argument("rename faces: as many new names as old ones are needed");
  }
  std::map<std::string, std::string> renames;
  for (std::size_t i = 0; i < from.size(); ++i) {
    renames[std::string(from[i])] = std::string(to[i]);
  }
  return geometry::rename_faces(shape, std::string(prefix), renames);
}

} // namespace mitcad::bridge
