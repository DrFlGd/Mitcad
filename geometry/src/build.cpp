// SPDX-License-Identifier: MIT
#include "mitcad/geometry/build.hpp"

#include <algorithm>
#include <memory>
#include <vector>

#include "util.hpp"

namespace mitcad::geometry {

ShapePtr rename_faces(const Shape& shape, const std::string& prefix,
                      const std::map<std::string, std::string>& renames) {
  return detail::run("rename faces", [&] {
    std::vector<Shape::NamedFace> faces;
    for (int i = 0; i < shape.face_count(); ++i) {
      NameList names;
      for (const std::string& name : shape.face_names(i)) {
        const auto found = renames.find(name);
        if (found != renames.end()) {
          names.push_back(found->second);
        } else if (prefix.empty()) {
          names.push_back(name);
        } else {
          names.push_back(prefix + '(' + name + ')');
        }
      }
      // Two names of a merged face may take the same new name.
      std::sort(names.begin(), names.end());
      names.erase(std::unique(names.begin(), names.end()), names.end());
      if (!names.empty()) {
        faces.push_back({shape.face(i), names});
      }
    }
    return std::make_shared<Shape>(shape.occt(), faces);
  });
}

} // namespace mitcad::geometry
