// SPDX-License-Identifier: MIT
#pragma once

// Seats on shafts (mitcad#578; commands.md, "Seats on shafts"): the panel
// inputs every seated feature shares (Pulley, Gear, the printable
// bearing). A part placed by a shaft instead of a plane and a centre takes
// the inputs
//
//   shaft           a shaft's circular end edge or its cylindrical side
//                   face (a hole's rim or wall); a click on a face also
//                   chooses the end nearer it,
//   seat_offset     along the shaft from that end,
//   seat_flip       from the other end, facing back,
//   bore_clearance  added to the shaft's diameter for the bore (optional),
//
// and the definition's `seat` (with the shaft's occurrence link when it is
// another component's) and `bore_clearance`. A shaft cleared again puts
// the plane and centre inputs where the seat had the part.

#include <functional>

#include <QJsonObject>
#include <QString>
#include <QVector>

#include "../framework/Command.hpp"

namespace mitcad::cmd {

struct SeatPanel {
  // Whether the part can sit on a shaft in a state (a gear's kind with a
  // bore, a pulley that does not run with another); the inputs show only
  // then, and a shaft is used only then. Always when unset.
  std::function<bool(const CommandState&)> offered;
  // Whether the panel has Bore Clearance.
  bool clearance = true;
  // The plane and centre inputs a cleared shaft fills; empty: none.
  QString plane = QStringLiteral("plane");
  QString x = QStringLiteral("x");
  QString y = QStringLiteral("y");
};

// The inputs, for a command's input list (after its plane input's place).
QVector<InputDef> seatInputs(const SeatPanel& panel = {});
// Whether a shaft is picked and the part can sit on it.
bool seated(const CommandState& state, const SeatPanel& panel = {});
// For a command's init: where the new feature's component is seen (the
// active component's first occurrence), which a link of another
// component's shaft needs.
void initSeat(CommandState& state, const CommandContext& context);
// The definition's `seat` and `bore_clearance` when seated.
void addSeat(QJsonObject& def, const CommandState& state, const CommandContext& context,
             const SeatPanel& panel = {});
// Fills the inputs from a definition with a `seat` (and its feature's
// component); false when it has none.
bool loadSeat(const QJsonObject& feature, CommandState& state, const CommandContext& context);
// Whether the panel can show the definition's seat (none counts).
bool seatShowable(const QJsonObject& feature, const CommandContext& context);
// What a panel's note says of a seated part from its values query
// (`bore_diameter`, `shaft_diameter`): "bore 8.2 mm (shaft 8 mm)"; empty
// when not seated.
QString seatNote(const QJsonObject& values, const CommandContext& context);

} // namespace mitcad::cmd
