// SPDX-License-Identifier: MIT
#include "MeasureHistory.hpp"

#include <algorithm>
#include <utility>

namespace mitcad {

const MeasureHistory::Row* MeasureHistory::row(int number) const {
  const auto found =
      std::find_if(m_rows.begin(), m_rows.end(), [number](const Row& row) { return row.number == number; });
  return found != m_rows.end() ? &*found : nullptr;
}

bool MeasureHistory::record(Row row) {
  if (row.key.isEmpty() || row.picked.isEmpty()) {
    return false;
  }
  // The same picks again (a change of the model, the row shown again):
  // the row keeps its place and number.
  for (Row& existing : m_rows) {
    if (existing.key == row.key) {
      if (existing.values == row.values && existing.picked == row.picked && !existing.gone) {
        return false;
      }
      row.number = existing.number;
      existing = std::move(row);
      return true;
    }
  }
  // The second pick of the measurement this panel took last.
  if (!m_rows.empty() && m_runRow != 0 && m_rows.front().number == m_runRow &&
      m_rows.front().picks < row.picks && row.key.startsWith(m_rows.front().key + QLatin1Char('\n'))) {
    row.number = m_runRow;
    m_rows.front() = std::move(row);
    return true;
  }
  row.number = m_next++;
  m_runRow = row.number;
  m_rows.insert(m_rows.begin(), std::move(row));
  return true;
}

bool MeasureHistory::setGone(int number, bool gone) {
  for (Row& row : m_rows) {
    if (row.number == number && row.gone != gone) {
      row.gone = gone;
      return true;
    }
  }
  return false;
}

void MeasureHistory::clear() {
  m_rows.clear();
  m_next = 1;
  m_runRow = 0;
}

QString MeasureHistory::tabSeparated(const Row& row) {
  QStringList cells{QString::number(row.number), row.picked};
  cells << row.values;
  return cells.join(QLatin1Char('\t'));
}

QString MeasureHistory::tabSeparated() const {
  QStringList lines;
  for (const Row& row : m_rows) {
    lines << tabSeparated(row);
  }
  return lines.join(QLatin1Char('\n'));
}

} // namespace mitcad
