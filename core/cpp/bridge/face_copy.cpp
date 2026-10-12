// SPDX-License-Identifier: MIT
#include "bridge/face_copy.hpp"

#include <utility>

#include "mitcad_bridge/kernel/face_copy.h"

namespace mitcad::bridge {

std::shared_ptr<geometry::Shape> FeatureChanges::added(std::size_t index) const {
  return m_changes.added.at(index).solid;
}

std::shared_ptr<geometry::Shape> FeatureChanges::added_seam(std::size_t index) const {
  return m_changes.added.at(index).seam;
}

std::shared_ptr<geometry::Shape> FeatureChanges::removed(std::size_t index) const {
  return m_changes.removed.at(index);
}

std::unique_ptr<FeatureChanges> feature_changes(const geometry::Shape& before, const geometry::Shape& after) {
  return std::make_unique<FeatureChanges>(geometry::feature_changes(before, after));
}

std::unique_ptr<ShapeList> supported_copies(const geometry::Shape& body, const ShapeList& solids,
                                            const ShapeList& seams) {
  const std::vector<geometry::ShapePtr> parts = geometry::supported_copies(body, solids.items(), seams.items());
  auto list = std::make_unique<ShapeList>();
  for (const geometry::ShapePtr& part : parts) {
    list->push(part);
  }
  return list;
}

} // namespace mitcad::bridge
