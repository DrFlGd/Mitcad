// SPDX-License-Identifier: MIT
#pragma once

#include <map>
#include <optional>
#include <vector>

#include <QFont>
#include <QString>
#include <QStyledItemDelegate>

#include "DecimalAlign.hpp"

class QTreeWidget;

namespace mitcad {

// Paints the numbers of some columns of a tree widget on one decimal axis
// per column (mitcad#566): the integer part ends at the axis, the decimal
// separator and the fraction follow it, and the units start on a line of
// their own after the longest fraction, in tabular digits. The cell's text
// stays what the model holds: selection, the focus frame, the tool tips and
// the in-place editor are the standard ones, which show the whole text.
// Texts that are not one number with an optional unit (names, formulas,
// refused entries) are painted as the standard delegate paints them, at the
// block's left edge. When a column is narrower than the block of its
// numbers, the cells are painted as standard, with an ellipsis.
//
// The widths of a column's numbers are taken from all rows of the tree
// (hidden ones too, so that the axis stays when the filter changes) once per
// font; `invalidate` after the rows or their texts changed.
class DecimalAlignDelegate : public QStyledItemDelegate {
public:
  // `columns` are the columns this delegate is set for.
  DecimalAlignDelegate(QTreeWidget* tree, std::vector<int> columns);

  // The rows or their texts changed: measure again at the next paint.
  void invalidate();

  // Where the parts of a cell's number are, in pixels from the cell's left
  // edge.
  struct Placement {
    int integerX = 0; // the first character of the integer part
    int axisX = 0;    // the decimal separator (the implied one of an integer)
    int unitX = -1;   // the unit; -1 for a number without one
  };

  // The placement of `text` in `column` for `font`; nothing when the text is
  // not a number, the column is not one of this delegate's, or its block
  // does not fit `cellWidth` (the cell is painted as standard then).
  std::optional<Placement> placement(const QString& text, int column, const QFont& font,
                                     int cellWidth) const;

  void paint(QPainter* painter, const QStyleOptionViewItem& option, const QModelIndex& index) const override;

protected:
  void initStyleOption(QStyleOptionViewItem* option, const QModelIndex& index) const override;

private:
  // The tabular-digit font of `font`, and the columns' layouts measured with it.
  void measure(const QFont& font) const;
  int margin() const;

  QTreeWidget* m_tree;
  std::vector<int> m_columns;
  mutable bool m_valid = false;
  mutable QString m_fontKey;
  mutable std::map<int, DecimalColumn> m_layouts;
};

} // namespace mitcad
