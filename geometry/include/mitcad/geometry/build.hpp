// SPDX-License-Identifier: MIT
#pragma once

// The BUILD tab's derived parts (mitcad#375): a library part's bodies
// copied into a design under the names of the feature that copies them.

#include <map>
#include <string>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

// The shape with its faces renamed and its B-rep shared: a face name found
// in `renames` (matched whole, "#k" included) takes the name it maps to;
// every other name n becomes "<prefix>(n)", or stays when `prefix` is empty.
ShapePtr rename_faces(const Shape& shape, const std::string& prefix,
                      const std::map<std::string, std::string>& renames);

} // namespace mitcad::geometry
