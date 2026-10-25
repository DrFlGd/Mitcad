// SPDX-License-Identifier: MIT
#pragma once

#include <optional>

#include <QString>

#include "../framework/Command.hpp"

namespace mitcad::cmd {

// Extrude's provisional drag presentation (CommandDef::dragProxy,
// mitcad#636): dragging a distance's arrow stretches the exact preview's
// tool along the arrow about the start (one side), the profile plane
// (symmetric: both halves) or only its own side (two sides); dragging the
// start offset's arrow moves the tool along it. Only for extents of a
// distance on every side without a taper, from the profile plane or an
// offset, not wrapped: where the tool is a straight prism whose length the
// values give. None as well when another input differs from the exact
// preview's (`shown`), or for a value that leaves no tool.
std::optional<DragProxyRule> extrudeDragProxy(const CommandState& shown, const CommandState& now, const QString& key,
                                              double value);

} // namespace mitcad::cmd
