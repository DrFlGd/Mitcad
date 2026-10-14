// SPDX-License-Identifier: MIT
#pragma once

// How a new feature's tool lies against bodies, for the operation a new
// Extrude chooses until the user picks one (mitcad#289): Join where the
// tool touches a body, Cut where it goes into one, New Body otherwise.

#include <vector>

#include "mitcad/geometry/shape.hpp"

namespace mitcad::geometry {

enum class ToolContact {
  Apart,   // farther from the body than the joins' contact distance
  Touches, // touching or overlapping it, its inside outside the body
  Inside,  // the inside of a solid of the tool is inside the body
};

// The contact of the tool with each body, cheap enough to follow a dragged
// distance: a body whose box the tool's box does not reach is apart without
// more; the others get the test Join uses (detail::shapes_touch), and a
// body the tool touches is classified at a point inside each solid of the
// tool (its middle, or just inside a face where the middle is not in it).
std::vector<ToolContact> tool_contacts(const Shape& tool, const std::vector<const Shape*>& bodies);

namespace detail {

// Whether the shapes touch or overlap within the joins' contact distance
// (a solid inside the other counts): the test Join uses (boolean.cpp).
bool shapes_touch(const Shape& a, const Shape& b);

} // namespace detail

} // namespace mitcad::geometry
