// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/face_copy.rs.

#include <cstddef>
#include <memory>

#include "bridge/shape.hpp"
#include "mitcad/geometry/face_copy.hpp"

namespace mitcad::bridge {

class FeatureChanges {
public:
  explicit FeatureChanges(geometry::FeatureChanges changes) : m_changes(std::move(changes)) {}

  std::size_t added_count() const { return m_changes.added.size(); }
  std::shared_ptr<geometry::Shape> added(std::size_t index) const;
  // The seam of the added material, null when it has none.
  std::shared_ptr<geometry::Shape> added_seam(std::size_t index) const;
  std::size_t removed_count() const { return m_changes.removed.size(); }
  std::shared_ptr<geometry::Shape> removed(std::size_t index) const;

private:
  geometry::FeatureChanges m_changes;
};

std::unique_ptr<FeatureChanges> feature_changes(const geometry::Shape& before, const geometry::Shape& after);

// The copies that stand on the body, as the list of their parts (a null
// entry for a copy with nothing under it); seams may be null.
std::unique_ptr<ShapeList> supported_copies(const geometry::Shape& body, const ShapeList& solids,
                                            const ShapeList& seams);

} // namespace mitcad::bridge
