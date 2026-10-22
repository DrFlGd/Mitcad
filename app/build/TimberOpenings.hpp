// SPDX-License-Identifier: MIT
#pragma once

// A timber frame's openings (mitcad#519; app/COMMANDS.md, "Timber",
// Openings): the rows the Openings window edits, their text form (the
// command's data: Wall's, Floor / Roof's and Timber Frame's
// `frame_openings`, the model's `openings` of build_frame and build_edit),
// and the checks of the rows as they are typed. Plain Qt Core.

#include <limits>
#include <optional>
#include <vector>

#include <QJsonArray>
#include <QString>
#include <QStringList>

namespace mitcad::build {

// One opening: a wall's window or door, or a floor's opening. Lengths in
// mm.
struct OpeningRow {
  enum class Kind { Window, Door, Floor };
  Kind kind = Kind::Window;
  // A wall's: `offset` measured from its end B to the opening's far side
  // (the Openings window's From End B), else from its start (end A) to the
  // near side, as the text and the model keep it.
  bool fromEnd = false;
  double offset = 0.0; // a wall's along it; a floor's across the joists (a)
  double across = 0.0; // a floor's along the joists (b)
  double width = 0.0;
  double height = 0.0; // a wall's opening's height; a floor's length
  double sill = 0.0;   // a window's sill above the wall's base; a door has none

  bool operator==(const OpeningRow& other) const;
};

constexpr double kUnknown = std::numeric_limits<double>::quiet_NaN();

// A number of the text or a field: "900", "900 mm", "12,5" (a decimal
// comma); none when it is no number.
std::optional<double> openingNumber(QString text);

// The rows of an Openings text, separated by ';': a wall's "offset
// widthxheight [sill]" (no sill, or 0: a door), a floor's "a b
// widthxlength" (from the outline's corner, across and along the joists).
// Empty rows, with `problem` set (the format), when an entry is not one.
std::vector<OpeningRow> openingRows(const QString& text, bool floor, QString* problem = nullptr);
// The text of rows (the reverse): a wall's offsets from its start, a row
// from its end B turned into one with the wall's length (unknown: its
// number kept as from the start). Numbers as short as they are exact.
QString openingsText(const std::vector<OpeningRow>& rows, double wallLength = kUnknown);
// The model's `openings` of rows (a wall's `offset`, `width`, `height` and
// a window's `sill`; a floor's `offset` [a, b] and `size`), and the rows of
// a definition's `openings`.
QJsonArray openingsJson(const std::vector<OpeningRow>& rows, double wallLength = kUnknown);
std::vector<OpeningRow> openingRowsOf(const QJsonArray& openings);
// A row's offset from the wall's start (as the model takes it).
double offsetFromStart(const OpeningRow& row, double wallLength);

// "2 openings: 1 window, 1 door", "1 opening", "No openings".
QString openingsSummary(const std::vector<OpeningRow>& rows);

// What a wall's rows are checked against; unknown sizes are not checked.
struct WallSize {
  double length = kUnknown;
  double height = kUnknown;
  int bottomPlates = 1;
  int topPlates = 1;
  double studThick = kUnknown; // the stud's (and plate's) smaller size
  double studDepth = kUnknown; // its larger: the wall's depth, an on-edge header's
  bool loadBearing = false;    // trimmers and a header on edge, else a flat header
};

// Each row's problem as the model would refuse the opening (empty: none):
// no width or height, a window without its sill, outside the wall or too
// near its end studs with its framing, overlapping another one's framing,
// its header above the top plate, its sill below the top of the bottom
// plate.
QStringList openingProblems(const std::vector<OpeningRow>& rows, const WallSize& wall);
// A floor's: no size, a negative offset, beyond the floor's length (across
// the joists) or width (along them) when they are known.
QStringList floorOpeningProblems(const std::vector<OpeningRow>& rows, double length = kUnknown,
                                 double width = kUnknown);

} // namespace mitcad::build
