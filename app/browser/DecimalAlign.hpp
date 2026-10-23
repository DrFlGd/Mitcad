// SPDX-License-Identifier: MIT
#pragma once

// The layout of numbers on a shared decimal axis (mitcad#566): a column's
// numbers are split at their decimal separator, and the parts are placed
// so that the separators of all rows are on one vertical line, an integer
// has its implied separator there, and the units of all rows start on
// another line after the longest fraction. Plain Qt Core and the standard
// library; the table's delegate (DecimalAlignDelegate) measures the parts
// and paints them.

#include <optional>
#include <vector>

#include <QString>

namespace mitcad {

// A cell's text split into the parts that are placed apart.
struct NumberParts {
  QString integer;  // the sign and the digits before the separator: "-2"
  QString fraction; // the separator with its digits and an exponent: ".75", "e-9"; empty for an integer
  QString unit;     // what follows the number: "mm"; empty for none
};

// The parts of a text that is one number with an optional unit: "3 mm",
// "-2.75 mm", "2,5", ".5 mm", "1e-9 mm", "30deg", white space around it
// allowed. A decimal comma counts as a decimal point (as in
// parsePlainNumber); the separator is kept as written. Nothing for any
// other text, such as a formula ("d1 * 2", "20 mm + 3 mm"), a name, a
// number with a text that is no unit ("20 mm * 2") or an empty text.
std::optional<NumberParts> splitNumber(const QString& text);

// The widths, in pixels, of a number's parts as drawn.
struct NumberWidths {
  int integer = 0;
  int fraction = 0;
  int unit = 0;
};

// Where the parts of a column's numbers go, from the left edge of the
// block that holds them all.
struct DecimalColumn {
  int axis = 0;      // the decimal separator: the widest integer part ends here
  int unitStart = 0; // the units start here: after the widest fraction and the gap
  int width = 0;     // the block, up to the end of the widest unit
};

// The column of the given numbers' widths; `gap` separates the widest
// fraction from the units (none when no number has a unit). All zeros for no
// numbers.
DecimalColumn layoutDecimalColumn(const std::vector<NumberWidths>& numbers, int gap);

// Where a number's integer part starts so that it ends at the axis.
inline int integerStart(const DecimalColumn& column, const NumberWidths& number) {
  return column.axis - number.integer;
}

} // namespace mitcad
