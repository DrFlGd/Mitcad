// SPDX-License-Identifier: MIT
#pragma once

// Move/Copy's motion as its inputs give it (mitcad#838), for its
// provisional drag motion (CommandDef::dragMotion): the same motion as the
// transform its definition gives the model, worked out without the model.

#include <optional>

#include <gp_Pnt.hxx>
#include <gp_Trsf.hxx>

#include "../framework/Command.hpp"

namespace mitcad::cmd {

// A free move's rotation: about X, then Y, then Z (radians).
gp_Trsf freeRotation(double rx, double ry, double rz);

// The motion of Move/Copy's inputs: Free Move turns about `pivot` (the
// middle of what moves, or an edit's kept pivot) and then moves by its X,
// Y and Z distances; Translate moves by its distances. None for the other
// move types, whose motions depend on picked geometry, and while a value
// is not a number.
std::optional<gp_Trsf> moveMotion(const CommandState& state, const gp_Pnt& pivot);

} // namespace mitcad::cmd
