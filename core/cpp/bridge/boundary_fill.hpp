// SPDX-License-Identifier: MIT
#pragma once

// C++ half of core/ffi/src/kernel/boundary_fill.rs.

#include <cstddef>
#include <memory>
#include <utility>
#include <vector>

#include "bridge/shape.hpp"
#include "mitcad/geometry/boundary_fill.hpp"
#include "rust/cxx.h"

namespace mitcad::bridge {

// The cells of geometry::boundary_cells, in the order of the points.
class BoundaryCells {
public:
  explicit BoundaryCells(std::vector<geometry::BoundaryCell> cells) : m_cells(std::move(cells)) {}

  std::size_t cell_count() const { return m_cells.size(); }
  std::shared_ptr<geometry::Shape> cell(std::size_t index) const;
  rust::Vec<std::size_t> cell_holders(std::size_t index) const;

private:
  std::vector<geometry::BoundaryCell> m_cells;
};

// geometry::boundary_cells of the tools at the points (x, y, z of each in
// turn).
std::unique_ptr<BoundaryCells> boundary_cells(const ShapeList& tools, rust::Slice<const double> points);

} // namespace mitcad::bridge
