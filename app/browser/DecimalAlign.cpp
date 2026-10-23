// SPDX-License-Identifier: MIT
#include "DecimalAlign.hpp"

#include <algorithm>

#include <QChar>

namespace mitcad {
namespace {

constexpr char16_t kMinus = 0x2212; // the minus sign as some locales write it

bool isDigit(QChar c) {
  return c.unicode() >= u'0' && c.unicode() <= u'9';
}

bool isSign(QChar c) {
  return c == QLatin1Char('-') || c == QLatin1Char('+') || c == QChar(kMinus);
}

bool isSeparator(QChar c) {
  return c == QLatin1Char('.') || c == QLatin1Char(',');
}

// A unit starts with a letter or a unit symbol and goes on with those, digits
// and the signs of powers and quotients ("mm", "deg", "°", "mm^2", "m/s",
// "in", "%").
bool isUnitStart(QChar c) {
  return c.isLetter() || c == QChar(0x00b0) || c == QLatin1Char('%') || c == QLatin1Char('"') ||
         c == QLatin1Char('\'');
}

bool isUnitText(const QString& text) {
  if (text.isEmpty() || !isUnitStart(text.front())) {
    return false;
  }
  return std::all_of(text.cbegin(), text.cend(), [](QChar c) {
    return isUnitStart(c) || isDigit(c) || c == QLatin1Char('^') || c == QLatin1Char('/') ||
           c == QChar(0x00b2) || c == QChar(0x00b3) || c.isMark();
  });
}

} // namespace

std::optional<NumberParts> splitNumber(const QString& text) {
  const QString s = text.trimmed();
  qsizetype i = 0;
  const qsizetype n = s.size();
  if (i < n && isSign(s.at(i))) {
    ++i;
  }
  qsizetype digits = 0;
  while (i < n && isDigit(s.at(i))) {
    ++i;
    ++digits;
  }
  const qsizetype integerEnd = i;
  if (i < n && isSeparator(s.at(i))) {
    ++i;
    while (i < n && isDigit(s.at(i))) {
      ++i;
      ++digits;
    }
  }
  if (digits == 0) {
    return std::nullopt; // no number: a name, a formula, nothing
  }
  // An exponent only when digits follow it: "2 em" has none.
  if (i < n && (s.at(i) == QLatin1Char('e') || s.at(i) == QLatin1Char('E'))) {
    qsizetype j = i + 1;
    if (j < n && isSign(s.at(j))) {
      ++j;
    }
    if (j < n && isDigit(s.at(j))) {
      while (j < n && isDigit(s.at(j))) {
        ++j;
      }
      i = j;
    }
  }
  NumberParts parts;
  parts.integer = s.left(integerEnd);
  parts.fraction = s.mid(integerEnd, i - integerEnd);
  const QString rest = s.mid(i).trimmed();
  if (!rest.isEmpty()) {
    if (!isUnitText(rest)) {
      return std::nullopt; // "20 mm * 2", "1.2.3"
    }
    parts.unit = rest;
  }
  return parts;
}

DecimalColumn layoutDecimalColumn(const std::vector<NumberWidths>& numbers, int gap) {
  int integer = 0;
  int fraction = 0;
  int unit = 0;
  for (const NumberWidths& number : numbers) {
    integer = std::max(integer, number.integer);
    fraction = std::max(fraction, number.fraction);
    unit = std::max(unit, number.unit);
  }
  DecimalColumn column;
  column.axis = integer;
  column.unitStart = integer + fraction + (unit > 0 ? gap : 0);
  column.width = column.unitStart + unit;
  return column;
}

} // namespace mitcad
