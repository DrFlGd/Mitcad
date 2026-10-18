// SPDX-License-Identifier: MIT
#pragma once

// Helices built directly as B-reps (mitcad#235): a profile loop moved by a
// screw motion, each of its edges sweeping one face over all the turns, as
// the bodies stored in files keep threads. A thread whose profile has a side
// on a cylinder as long as the pitch has turns that meet in one edge there;
// its solid then also holds a core beyond that side, so that the turns join
// along a face instead of touching along an edge. Internal to the geometry
// library.

#include <optional>
#include <string>

#include <TopoDS_Edge.hxx>
#include <gp_Ax1.hxx>
#include <gp_Dir.hxx>

#include "mitcad/geometry/shape.hpp"
#include "spine.hpp"

namespace mitcad::geometry::detail {

// A side of the profile along the axis, as long as the pitch, whose turns
// meet: `away` lies in the profile's plane, across the side, away from the
// profile; the core reaches `depth` beyond the side that way (and twice as
// far at the side's middle).
struct MeetingSide {
  TopoDS_Edge edge;
  gp_Dir away;
  double depth = 0.0;
};

// The loop moved by the screw motion about `axis` that turns once (against
// the clock about the axis' direction, or with it when `left_handed`) while
// it rises `pitch` along it, for `revolutions` turns from where it lies. Its
// edges sweep one face each, named as the loop names them; the caps are
// named `first` and `last`. With `meeting` the side's turns meet: the side
// is replaced by the core beyond it (faces named as the side), and the
// faces either side of it meet in one edge between the turns. The faces are
// split every `piece_turns` turns (0: not split) into pieces that share
// their surface. Throws when the solid cannot be built: a profile edge
// without a B-spline form, edges that do not join, or a result that is not
// one valid solid.
ShapePtr screwed_brep(const NamedWire& loop, const gp_Ax1& axis, double pitch, double revolutions,
                      bool left_handed, const NameList& first, const NameList& last,
                      const std::optional<MeetingSide>& meeting, double piece_turns);

} // namespace mitcad::geometry::detail
