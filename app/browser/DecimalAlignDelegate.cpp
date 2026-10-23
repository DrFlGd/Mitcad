// SPDX-License-Identifier: MIT
#include "DecimalAlignDelegate.hpp"

#include <algorithm>
#include <utility>

#include <QApplication>
#include <QColor>
#include <QFontMetrics>
#include <QPainter>
#include <QPalette>
#include <QRect>
#include <QStyle>
#include <QStyleOptionViewItem>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>

#include "../platform/MacParameters.hpp"

namespace mitcad {
namespace {

// Digits of one width, so that the digits of different rows line up too
// (where the font has them; the separators are placed by the layout anyway).
QFont tabularFont(QFont font) {
  font.setFeature("tnum", 1);
  return font;
}

NumberWidths measureParts(const QFontMetrics& metrics, const NumberParts& parts) {
  NumberWidths widths;
  widths.integer = metrics.horizontalAdvance(parts.integer);
  widths.fraction = metrics.horizontalAdvance(parts.fraction);
  widths.unit = metrics.horizontalAdvance(parts.unit);
  return widths;
}

} // namespace

DecimalAlignDelegate::DecimalAlignDelegate(QTreeWidget* tree, std::vector<int> columns)
    : QStyledItemDelegate(tree), m_tree(tree), m_columns(std::move(columns)) {}

void DecimalAlignDelegate::invalidate() {
  m_valid = false;
}

void DecimalAlignDelegate::initStyleOption(QStyleOptionViewItem* option, const QModelIndex& index) const {
  QStyledItemDelegate::initStyleOption(option, index);
  // This also runs through the standard delegate's formula/narrow fallback.
  mac::styleParameterCell(option, index);
}

int DecimalAlignDelegate::margin() const {
  // The text's margin in the cell, as the standard delegate keeps it.
  return m_tree->style()->pixelMetric(QStyle::PM_FocusFrameHMargin, nullptr, m_tree) + 1;
}

void DecimalAlignDelegate::measure(const QFont& font) const {
  const QFont digits = tabularFont(font);
  const QString key = digits.key();
  if (m_valid && key == m_fontKey) {
    return;
  }
  const QFontMetrics metrics(digits);
  const int gap = metrics.horizontalAdvance(QLatin1Char(' '));
  m_layouts.clear();
  for (const int column : m_columns) {
    std::vector<NumberWidths> numbers;
    for (QTreeWidgetItemIterator it(m_tree); *it != nullptr; ++it) {
      if (const std::optional<NumberParts> parts = splitNumber((*it)->text(column))) {
        numbers.push_back(measureParts(metrics, *parts));
      }
    }
    m_layouts[column] = layoutDecimalColumn(numbers, gap);
  }
  m_fontKey = key;
  m_valid = true;
}

std::optional<DecimalAlignDelegate::Placement> DecimalAlignDelegate::placement(const QString& text, int column,
                                                                               const QFont& font,
                                                                               int cellWidth) const {
  if (std::find(m_columns.cbegin(), m_columns.cend(), column) == m_columns.cend()) {
    return std::nullopt;
  }
  const std::optional<NumberParts> parts = splitNumber(text);
  if (!parts) {
    return std::nullopt;
  }
  measure(font);
  const DecimalColumn layout = m_layouts[column];
  const int inset = margin();
  if (layout.width + 2 * inset > cellWidth) {
    return std::nullopt;
  }
  const QFontMetrics metrics(tabularFont(font));
  Placement place;
  place.integerX = inset + integerStart(layout, measureParts(metrics, *parts));
  place.axisX = inset + layout.axis;
  place.unitX = parts->unit.isEmpty() ? -1 : inset + layout.unitStart;
  return place;
}

void DecimalAlignDelegate::paint(QPainter* painter, const QStyleOptionViewItem& option,
                                 const QModelIndex& index) const {
  QStyleOptionViewItem opt = option;
  initStyleOption(&opt, index);
  const std::optional<NumberParts> parts = splitNumber(opt.text);
  const std::optional<Placement> place =
      parts ? placement(opt.text, index.column(), opt.font, opt.rect.width()) : std::nullopt;
  if (!place) {
    QStyledItemDelegate::paint(painter, option, index);
    return;
  }
  // The background, the selection and the focus frame as standard, without
  // the text.
  const QWidget* widget = opt.widget;
  QStyle* style = widget != nullptr ? widget->style() : QApplication::style();
  opt.text.clear();
  opt.features.setFlag(QStyleOptionViewItem::HasDisplay, false);
  style->drawControl(QStyle::CE_ItemViewItem, &opt, painter, widget);

  const QFont font = tabularFont(opt.font);
  const QFontMetrics metrics(font);
  const NumberWidths widths = measureParts(metrics, *parts);
  QPalette::ColorGroup group = QPalette::Inactive;
  if (!(opt.state & QStyle::State_Enabled)) {
    group = QPalette::Disabled;
  } else if (opt.state & QStyle::State_Active) {
    group = QPalette::Active;
  }
  const QColor color =
      opt.palette.color(group, (opt.state & QStyle::State_Selected) ? QPalette::HighlightedText : QPalette::Text);
  painter->save();
  painter->setClipRect(opt.rect);
  painter->setFont(font);
  painter->setPen(color);
  const int flags = static_cast<int>(Qt::AlignVCenter) | static_cast<int>(Qt::AlignLeft) |
                    static_cast<int>(Qt::TextSingleLine);
  const auto draw = [&](int x, int width, const QString& text) {
    if (!text.isEmpty()) {
      painter->drawText(QRect(opt.rect.left() + x, opt.rect.top(), width, opt.rect.height()), flags, text);
    }
  };
  draw(place->integerX, widths.integer, parts->integer);
  draw(place->axisX, widths.fraction, parts->fraction);
  if (place->unitX >= 0) {
    draw(place->unitX, widths.unit, parts->unit);
  }
  painter->restore();
}

} // namespace mitcad
