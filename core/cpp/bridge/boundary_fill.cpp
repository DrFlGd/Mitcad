// SPDX-License-Identifier: MIT
#include "bridge/boundary_fill.hpp"

#include <gp_Pnt.hxx>

#include "mitcad_bridge/kernel/boundary_fill.h"

namespace mitcad::bridge {

std::shared_ptr<geometry::Shape> BoundaryCells::cell(std::size_t index) const { return m_cells.at(index).shape; }

rust::Vec<std::size_t> BoundaryCells::cell_holders(std::size_t index) const {
  rust::Vec<std::size_t> holders;
  for (const std::size_t holder : m_cells.at(index).holders) {
    holders.push_back(holder);
  }
  return holders;
}

std::unique_ptr<BoundaryCells> boundary_cells(const ShapeList& tools, rust::Slice<const double> points) {
  std::vector<const geometry::Shape*> shapes;
  for (const auto& tool : tools.items()) {
    shapes.push_back(tool.get());
  }
  std::vector<gp_Pnt> at;
  for (std::size_t i = 0; i + 2 < points.size(); i += 3) {
    at.emplace_back(points[i], points[i + 1], points[i + 2]);
  }
  return std::make_unique<BoundaryCells>(geometry::boundary_cells(shapes, at));
}

} // namespace mitcad::bridge
