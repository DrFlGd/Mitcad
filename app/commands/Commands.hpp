// SPDX-License-Identifier: MIT
#pragma once

// Registration of the command definitions (app/COMMANDS.md), one function
// per family of the toolbar.

#include <functional>

#include <QStringList>

#include "../framework/Selection.hpp"

class QWidget;

namespace mitcad {

class CommandContext;
class CommandRegistry;
struct CommandDef;

namespace sketch {
class SketchController;
}

// The SOLID tab's CREATE group (U4), in menu order: Create Sketch (the
// main window's, given here), Extrude, Revolve, Sweep, Loft, Rib, Web,
// Hole, Thread, the primitives, Coil, Pipe, the patterns and Mirror; also
// New Component, which leads the ASSEMBLE group. `context` answers the
// definitions' questions about the model while the application runs.
void registerCreateCommands(CommandRegistry& registry, const CommandContext& context,
                            const CommandDef& createSketch);
// MODIFY: Press Pull, Fillet, Chamfer, Shell, Draft, Scale, Combine,
// Offset Face, Replace Face, Split Face, Split Body, Move/Copy, Align,
// Delete, Physical Material, Appearance.
void registerModifyCommands(CommandRegistry& registry, const CommandContext& context);
// CONSTRUCT: every construction plane, axis and point.
void registerConstructCommands(CommandRegistry& registry, const CommandContext& context);
// ASSEMBLE (mitcad#55): Joint, As-Built Joint, Joint Origin, Rigid Group,
// Drive Joint. New Component comes first (registerCreateCommands); Animate
// Joint, Insert Component and Insert from Library last (the main window).
void registerAssembleCommands(CommandRegistry& registry, const CommandContext& context);
// INSPECT: Measure, Interference, Section Analysis and Physical
// Properties (a dialog over `window` for the bodies `selection` gives).
void registerInspectCommands(CommandRegistry& registry, const CommandContext& context,
                             QWidget* window, std::function<Selection()> selection);

// The SKETCH tab (U2): drawing tools, the sketch dimension, constraints and
// the modify commands, run in the sketch `controller` edits.
void registerSketchCommands(CommandRegistry& registry, sketch::SketchController& controller);
// The constraint commands' ids, in the sketch palette's order.
QStringList sketchConstraintCommands();

} // namespace mitcad
