// SPDX-License-Identifier: MIT
#include "bridge/contact.hpp"

#include <vector>

#include "mitcad/geometry/contact.hpp"
#include "mitcad_bridge/kernel/contact.h"

namespace mitcad::bridge {

rust::Vec<std::uint8_t> tool_contacts(const geometry::Shape& tool, const ShapeList& bodies) {
  std::vector<const geometry::Shape*> shapes;
  shapes.reserve(bodies.size());
  for (const auto& body : bodies.items()) {
    shapes.push_back(body.get());
  }
  rust::Vec<std::uint8_t> contacts;
  for (const geometry::ToolContact contact : geometry::tool_contacts(tool, shapes)) {
    switch (contact) {
    case geometry::ToolContact::Apart:
      contacts.push_back(0);
      break;
    case geometry::ToolContact::Touches:
      contacts.push_back(1);
      break;
    case geometry::ToolContact::Inside:
      contacts.push_back(2);
      break;
    }
  }
  return contacts;
}

} // namespace mitcad::bridge
