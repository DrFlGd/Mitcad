// SPDX-License-Identifier: MIT
#include "bridge/tangent_chain.hpp"

#include <string>
#include <utility>
#include <vector>

#include "mitcad/geometry/tangent_chain.hpp"
#include "mitcad_bridge/kernel/tangent_chain.h"

namespace mitcad::bridge {

rust::Vec<TangentChain> tangent_chains(const geometry::Shape& shape, bool faces) {
  rust::Vec<TangentChain> result;
  const std::vector<std::vector<int>> chains =
      faces ? geometry::face_tangent_chains(shape) : geometry::edge_tangent_chains(shape);
  for (const std::vector<int>& chain : chains) {
    TangentChain named;
    for (const int i : chain) {
      if (faces) {
        const geometry::NameList& names = shape.face_names(i);
        if (!names.empty()) {
          named.names.push_back(rust::String(names.front()));
        }
      } else if (!shape.edge_name(i).empty()) {
        named.names.push_back(rust::String(shape.edge_name(i)));
      }
    }
    if (named.names.size() > 1) {
      result.push_back(std::move(named));
    }
  }
  return result;
}

} // namespace mitcad::bridge
