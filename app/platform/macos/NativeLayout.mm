// SPDX-License-Identifier: MIT
#include "NativeLayout.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <utility>

#include <QAbstractButton>
#include <QAbstractScrollArea>
#include <QAbstractSpinBox>
#include <QBoxLayout>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QGridLayout>
#include <QKeySequenceEdit>
#include <QLabel>
#include <QLayout>
#include <QLayoutItem>
#include <QLineEdit>
#include <QList>
#include <QMargins>
#include <QProgressBar>
#include <QSizePolicy>
#include <QSlider>
#include <QSpacerItem>
#include <QWidget>

namespace mitcad::mac::layout {
namespace {

// Constraint priorities of the contract. Explicit and Qt minimum/maximum
// sizes hold against window resizing (510); preferred sizes, filling and
// sharing stay below the window's own size (500), so they never resize it.
constexpr NSLayoutPriority kLimit = 999;
constexpr NSLayoutPriority kSoftCap = 740;  // below AppKit's text compression (750)
// Filling stays below every preferred size: a field capped at its hint must
// never pull its column below a neighbour's preferred width.
constexpr NSLayoutPriority kFill = 320;  // above AppKit's default hugging (250)
constexpr NSLayoutPriority kPreferred = 490;
constexpr NSLayoutPriority kShare = 480;
constexpr NSLayoutPriority kStretchedPreferred = 470;  // yields to the stretch ratio
constexpr NSLayoutPriority kLabelCentre = 300;
constexpr NSLayoutPriority kTrackHug = 2;
constexpr NSLayoutPriority kHug = 1;
constexpr CGFloat kDefaultSpacing = 8;
constexpr CGFloat kButtonSpacing = 12;
constexpr CGFloat kMacGap = 36;  // QDialogButtonBox's gap around destructive buttons

char kHiddenToken = 0;  // KVO context of the items' hidden state

using Render = std::function<NSView*(QLayoutItem*)>;

struct Edges {
  NSLayoutAnchor* start;
  NSLayoutAnchor* end;
  NSLayoutAnchor* centre;
  NSLayoutDimension* size;
};

Edges edges(NSView* view, bool horizontal) {
  if (horizontal) { return {view.leadingAnchor, view.trailingAnchor, view.centerXAnchor, view.widthAnchor}; }
  return {view.topAnchor, view.bottomAnchor, view.centerYAnchor, view.heightAnchor};
}

Edges edges(NSLayoutGuide* guide, bool horizontal) {
  if (horizontal) { return {guide.leadingAnchor, guide.trailingAnchor, guide.centerXAnchor, guide.widthAnchor}; }
  return {guide.topAnchor, guide.bottomAnchor, guide.centerYAnchor, guide.heightAnchor};
}

int extent(const QSize& size, bool horizontal) { return horizontal ? size.width() : size.height(); }
bool bounded(int value) { return value >= 0 && value < QLAYOUTSIZE_MAX; }
bool any(Qt::Alignment value, Qt::Alignment mask) { return (value & mask).toInt() != 0; }

Place placement(Qt::Alignment alignment, bool horizontal) {
  if (horizontal) {
    if (any(alignment, Qt::AlignLeft)) { return Place::Leading; }
    if (any(alignment, Qt::AlignRight)) { return Place::Trailing; }
    return Place::Center;  // AlignHCenter, or none: Qt centres a bounded widget
  }
  if (any(alignment, Qt::AlignTop)) { return Place::Leading; }
  if (any(alignment, Qt::AlignBottom)) { return Place::Trailing; }
  return Place::Center;
}

// AppKit's intrinsic width means little for these controls (an empty text
// field, a slider) or differs from the shared text metrics (wrapping
// labels): their widths follow the presenter's size hints.
bool sharedMetrics(const QWidget* widget) {
  return qobject_cast<const QLineEdit*>(widget) != nullptr || qobject_cast<const QAbstractSpinBox*>(widget) != nullptr ||
         qobject_cast<const QSlider*>(widget) != nullptr || qobject_cast<const QProgressBar*>(widget) != nullptr ||
         qobject_cast<const QKeySequenceEdit*>(widget) != nullptr || qobject_cast<const QComboBox*>(widget) != nullptr ||
         qobject_cast<const QLabel*>(widget) != nullptr || qobject_cast<const QAbstractScrollArea*>(widget) != nullptr;
}

bool entryWidth(const QWidget* widget) {
  const auto* combo = qobject_cast<const QComboBox*>(widget);
  return qobject_cast<const QLineEdit*>(widget) != nullptr || qobject_cast<const QAbstractSpinBox*>(widget) != nullptr ||
         qobject_cast<const QSlider*>(widget) != nullptr || qobject_cast<const QProgressBar*>(widget) != nullptr ||
         qobject_cast<const QKeySequenceEdit*>(widget) != nullptr || (combo != nullptr && combo->isEditable());
}

// A widget item's axis, as QWidgetItem/qSmartMinSize/qSmartMaxSize derive
// it, from the widget itself: a hidden widget keeps its contract for when
// it is shown again.
Axis widgetAxis(const QWidget* widget, bool horizontal, Qt::Alignment alignment) {
  Axis axis;
  const QSizePolicy policy = widget->sizePolicy();
  const int flags = int(horizontal ? policy.horizontalPolicy() : policy.verticalPolicy());
  const bool aligned = any(alignment, horizontal ? Qt::AlignHorizontal_Mask : Qt::AlignVertical_Mask);
  // Heights keep AppKit's own control metrics; explicit limits still apply.
  const bool metrics = horizontal && sharedMetrics(widget);
  const int explicitMinimum = extent(widget->minimumSize(), horizontal);
  const int explicitMaximum = extent(widget->maximumSize(), horizontal);
  int minimum = 0;
  int preferred = -1;
  if (metrics) {
    const int hint = extent(widget->sizeHint(), horizontal);
    const int minimumHint = extent(widget->minimumSizeHint(), horizontal);
    if ((flags & QSizePolicy::IgnoreFlag) == 0) {
      minimum = (flags & QSizePolicy::ShrinkFlag) != 0 ? minimumHint : std::max(hint, minimumHint);
    }
    // A wrapping label's hint is a reading width, not a requirement.
    const auto* label = qobject_cast<const QLabel*>(widget);
    if (hint > 0 && (label == nullptr || !label->wordWrap())) { preferred = hint; }
    if (qobject_cast<const QLineEdit*>(widget) != nullptr) {
      // At least QLineEdit's own 17 characters, measured in the AppKit
      // field's font: the hidden presenter's hint can be too small.
      NSFont* font = [NSFont systemFontOfSize:NSFont.systemFontSize];
      const CGFloat letter = [@"x" sizeWithAttributes:@{NSFontAttributeName: font}].width;
      preferred = std::max(preferred, static_cast<int>(std::lround(letter * 17 + 8)));
    }
  }
  minimum = std::min(minimum, explicitMaximum);
  if (explicitMinimum > 0) { minimum = explicitMinimum; }
  minimum = std::max(minimum, 0);
  if (preferred >= 0) { preferred = std::max(minimum, std::min(preferred, explicitMaximum)); }
  const bool canGrow = (flags & QSizePolicy::GrowFlag) != 0;
  axis.minimum = static_cast<CGFloat>(minimum);
  axis.preferred = static_cast<CGFloat>(preferred);
  axis.maximum = bounded(explicitMaximum) ? static_cast<CGFloat>(explicitMaximum) : -1;
  axis.cap = aligned || !canGrow;
  axis.grows = (aligned || canGrow) && explicitMaximum > std::max(minimum, preferred);
  axis.expands = !aligned && axis.grows && (flags & QSizePolicy::ExpandFlag) != 0;
  axis.place = placement(alignment, horizontal);
  // Labels keep Qt's vertically centred text, buttons their own height.
  axis.fill = horizontal || (!aligned && canGrow && qobject_cast<const QLabel*>(widget) == nullptr &&
                             qobject_cast<const QAbstractButton*>(widget) == nullptr);
  return axis;
}

// A nested layout's size hint is its native content's natural size: Qt's
// QLayout::sizeHint is not reliable before its presenter is shown.
Axis layoutAxis(bool horizontal, Qt::Alignment alignment) {
  Axis axis;
  axis.place = placement(alignment, horizontal);
  // An aligned layout keeps its size hint, as QLayout::alignmentRect.
  axis.cap = any(alignment, horizontal ? Qt::AlignHorizontal_Mask : Qt::AlignVertical_Mask);
  return axis;
}

Axis spacerAxis(const QSpacerItem* spacer, bool horizontal) {
  Axis axis;
  const QSizePolicy policy = spacer->sizePolicy();
  const int flags = int(horizontal ? policy.horizontalPolicy() : policy.verticalPolicy());
  const int maximum = extent(spacer->maximumSize(), horizontal);
  axis.minimum = static_cast<CGFloat>(std::max(0, extent(spacer->minimumSize(), horizontal)));
  axis.preferred = static_cast<CGFloat>(std::max(0, extent(spacer->sizeHint(), horizontal)));
  axis.maximum = bounded(maximum) ? static_cast<CGFloat>(maximum) : -1;
  axis.grows = (flags & QSizePolicy::GrowFlag) != 0;
  axis.expands = axis.grows && (flags & QSizePolicy::ExpandFlag) != 0;
  axis.cap = !axis.grows;
  return axis;
}

int widgetStretch(QLayoutItem* source, bool horizontal) {
  const QWidget* widget = source->widget();
  if (widget == nullptr) { return 0; }
  return horizontal ? widget->sizePolicy().horizontalStretch() : widget->sizePolicy().verticalStretch();
}

bool makeItem(QLayoutItem* source, const Render& render, Item& item) {
  const Qt::Alignment alignment = source->alignment();
  if (QWidget* widget = source->widget()) {
    item.view = render(source);
    item.horizontal = widgetAxis(widget, true, alignment);
    item.vertical = widgetAxis(widget, false, alignment);
    // An ignored width hint lets a long title shrink, as in the Qt layout.
    item.entry = entryWidth(widget) || widget->sizePolicy().horizontalPolicy() == QSizePolicy::Ignored;
  } else if (source->layout() != nullptr) {
    item.view = render(source);
    item.nested = true;
    item.horizontal = layoutAxis(true, alignment);
    item.vertical = layoutAxis(false, alignment);
  } else if (QSpacerItem* spacer = source->spacerItem()) {
    item.view = [[NSView alloc] initWithFrame:NSZeroRect];
    item.spacer = true;
    item.horizontal = spacerAxis(spacer, true);
    item.vertical = spacerAxis(spacer, false);
  }
  return item.view != nil;
}

CGFloat spacing(int value) { return value >= 0 ? static_cast<CGFloat>(value) : kDefaultSpacing; }

void addBox(QBoxLayout* box, const Render& render, Grid& grid) {
  const QBoxLayout::Direction direction = box->direction();
  const bool horizontal = direction == QBoxLayout::LeftToRight || direction == QBoxLayout::RightToLeft;
  const bool reversed = direction == QBoxLayout::RightToLeft || direction == QBoxLayout::BottomToTop;
  std::vector<Track>& tracks = horizontal ? grid.columns : grid.rows;
  (horizontal ? grid.rows : grid.columns).assign(1, Track{});
  const int count = box->count();
  for (int i = 0; i < count; ++i) {
    const int index = reversed ? count - 1 - i : i;
    QLayoutItem* source = box->itemAt(index);
    Item item;
    if (source == nullptr || !makeItem(source, render, item)) { continue; }
    Track track;
    // QBoxLayout's own stretch, or else the widget's size policy stretch.
    track.stretch = box->stretch(index);
    if (track.stretch == 0) { track.stretch = widgetStretch(source, horizontal); }
    (horizontal ? item.column : item.row) = static_cast<int>(tracks.size());
    tracks.push_back(track);
    grid.items.push_back(std::move(item));
  }
  (horizontal ? grid.horizontalSpacing : grid.verticalSpacing) = spacing(box->spacing());
}

void addGrid(QGridLayout* layout, const Render& render, Grid& grid) {
  const int rows = std::max(1, layout->rowCount());
  const int columns = std::max(1, layout->columnCount());
  grid.rows.assign(static_cast<std::size_t>(rows), Track{});
  grid.columns.assign(static_cast<std::size_t>(columns), Track{});
  for (int row = 0; row < rows; ++row) {
    grid.rows[static_cast<std::size_t>(row)].stretch = layout->rowStretch(row);
    grid.rows[static_cast<std::size_t>(row)].minimum = static_cast<CGFloat>(layout->rowMinimumHeight(row));
  }
  for (int column = 0; column < columns; ++column) {
    grid.columns[static_cast<std::size_t>(column)].stretch = layout->columnStretch(column);
    grid.columns[static_cast<std::size_t>(column)].minimum = static_cast<CGFloat>(layout->columnMinimumWidth(column));
  }
  for (int i = 0; i < layout->count(); ++i) {
    int row = 0;
    int column = 0;
    int rowSpan = 1;
    int columnSpan = 1;
    layout->getItemPosition(i, &row, &column, &rowSpan, &columnSpan);
    QLayoutItem* source = layout->itemAt(i);
    Item item;
    if (source == nullptr || !makeItem(source, render, item)) { continue; }
    item.row = std::clamp(row, 0, rows - 1);
    item.column = std::clamp(column, 0, columns - 1);
    // A negative span reaches the last row or column.
    item.rowSpan = std::clamp(rowSpan < 0 ? rows - item.row : rowSpan, 1, rows - item.row);
    item.columnSpan = std::clamp(columnSpan < 0 ? columns - item.column : columnSpan, 1, columns - item.column);
    // Without an explicit stretch, a track takes its widgets' policy stretch.
    if (item.columnSpan == 1 && layout->columnStretch(item.column) == 0) {
      Track& track = grid.columns[static_cast<std::size_t>(item.column)];
      track.stretch = std::max(track.stretch, widgetStretch(source, true));
    }
    if (item.rowSpan == 1 && layout->rowStretch(item.row) == 0) {
      Track& track = grid.rows[static_cast<std::size_t>(item.row)];
      track.stretch = std::max(track.stretch, widgetStretch(source, false));
    }
    grid.items.push_back(std::move(item));
  }
  grid.horizontalSpacing = spacing(layout->horizontalSpacing());
  grid.verticalSpacing = spacing(layout->verticalSpacing());
}

void addForm(QFormLayout* form, const Render& render, Grid& grid) {
  grid.columns.assign(2, Track{});
  grid.alwaysGrows = true;
  // The effective policies include the platform style's defaults.
  const QFormLayout::FieldGrowthPolicy policy = form->fieldGrowthPolicy();
  const FieldGrowth growth = policy == QFormLayout::FieldsStayAtSizeHint ? FieldGrowth::None
                             : policy == QFormLayout::ExpandingFieldsGrow ? FieldGrowth::Expanding
                                                                          : FieldGrowth::All;
  const Qt::Alignment labels = form->labelAlignment();
  const Qt::Alignment whole = form->formAlignment();
  const bool wrap = form->rowWrapPolicy() == QFormLayout::WrapAllRows;
  int row = 0;
  for (int i = 0; i < form->rowCount(); ++i) {
    if (QLayoutItem* spanning = form->itemAt(i, QFormLayout::SpanningRole)) {
      Item item;
      if (makeItem(spanning, render, item)) {
        item.row = row;
        item.columnSpan = 2;
        item.regime = false;
        item.horizontal.place = Place::Leading;
        grid.items.push_back(std::move(item));
      }
      ++row;
      continue;
    }
    if (QLayoutItem* label = form->itemAt(i, QFormLayout::LabelRole)) {
      Item item;
      if (makeItem(label, render, item)) {
        item.row = row;
        item.formLabel = true;
        item.horizontal.cap = true;
        item.horizontal.grows = false;
        item.horizontal.expands = false;
        item.horizontal.place = placement(labels, true);
        if (const QWidget* caption = label->widget()) {
          item.labelLimit = static_cast<CGFloat>(caption->sizeHint().height()) * 3 / 8;
        }
        if (wrap) {
          item.columnSpan = 2;
          item.regime = false;
          item.horizontal.place = Place::Leading;
        }
        grid.items.push_back(std::move(item));
        if (wrap) { ++row; }
      }
    }
    if (QLayoutItem* field = form->itemAt(i, QFormLayout::FieldRole)) {
      Item item;
      if (makeItem(field, render, item)) {
        item.row = row;
        item.column = wrap ? 0 : 1;
        item.columnSpan = wrap ? 2 : 1;
        item.formField = true;
        item.regime = !wrap;
        item.growth = growth;
        item.horizontal.place = Place::Leading;
        grid.items.push_back(std::move(item));
      }
    }
    ++row;
  }
  grid.rows.assign(static_cast<std::size_t>(std::max(1, row)), Track{});
  grid.horizontalSpacing = spacing(form->horizontalSpacing());
  grid.verticalSpacing = spacing(form->verticalSpacing());
  grid.content = any(whole, Qt::AlignHCenter) ? Place::Center
                 : any(whole, Qt::AlignRight) ? Place::Trailing
                                              : Place::Leading;
}

// A layout of another kind lists its items from top to bottom.
void addList(QLayout* layout, const Render& render, Grid& grid) {
  grid.columns.assign(1, Track{});
  for (int i = 0; i < layout->count(); ++i) {
    QLayoutItem* source = layout->itemAt(i);
    Item item;
    if (source == nullptr || !makeItem(source, render, item)) { continue; }
    item.row = static_cast<int>(grid.rows.size());
    grid.rows.push_back(Track{});
    grid.items.push_back(std::move(item));
  }
  grid.verticalSpacing = spacing(layout->spacing());
}

// The live axis of a nested layout or widget container, without the
// growth policy of a form field.
Axis baseAxis(const Item& item, bool horizontal) {
  Axis axis = horizontal ? item.horizontal : item.vertical;
  if (![item.view isKindOfClass:MitcadNativeLayoutView.class]) { return axis; }
  auto* inner = (MitcadNativeLayoutView*)item.view;
  const bool expands = [inner expandsAlong:horizontal];
  if (item.nested) {
    // QLayout::expandingDirections and maximumSize for its shown items.
    if (!axis.cap) {
      axis.grows = expands || [inner growsAlong:horizontal];
      axis.expands = expands;
    }
  } else if (axis.grows && expands) {
    axis.expands = true;  // QWidgetItem: a growing widget whose layout expands
  }
  return axis;
}

Axis effectiveAxis(const Item& item, bool horizontal) {
  Axis axis = baseAxis(item, horizontal);
  if (horizontal && item.formField) {
    if (item.growth == FieldGrowth::None) {
      axis.grows = false;
      axis.expands = false;
    } else if (item.growth == FieldGrowth::Expanding) {
      axis.grows = axis.expands;
    }
    axis.cap = !axis.grows;
  }
  return axis;
}

bool shown(const Item& item) {
  if (item.view == nil || item.view.hidden) { return false; }
  if (item.nested && [item.view isKindOfClass:MitcadNativeLayoutView.class]) {
    return ![(MitcadNativeLayoutView*)item.view layoutEmpty];
  }
  return true;
}

std::pair<std::size_t, std::size_t> span(const Item& item, bool horizontal, std::size_t count) {
  const int start = std::max(0, horizontal ? item.column : item.row);
  const int length = std::max(1, horizontal ? item.columnSpan : item.rowSpan);
  const std::size_t first = std::min(static_cast<std::size_t>(start), count - 1);
  const std::size_t last = std::min(first + static_cast<std::size_t>(length) - 1, count - 1);
  return {first, last};
}

struct Guides {
  NSArray<NSLayoutGuide*>* columns;
  NSArray<NSLayoutGuide*>* rows;
  NSArray<NSLayoutGuide*>* cells;
  NSLayoutGuide* content;
  NSLayoutGuide* block;
};

// An item's state in one rebuild: hidden, shown, or taken away by another
// owner (a window's button footer), which leaves its view alone.
constexpr char kHidden = 0;
constexpr char kShown = 1;
constexpr char kDetached = 2;

// Qt's box/grid geometry along one axis: tracks keep their items' minimum
// and preferred sizes; extra space goes to the stretched tracks in their
// ratio, else to the expanding tracks, else to the tracks that can grow,
// as qGeomCalc. Spacing separates tracks with shown content only.
void constrainAxis(NSView* host, const Grid& grid, const Guides& guides, bool horizontal,
                   const std::vector<char>& visible, NSMutableArray<NSLayoutConstraint*>* out) {
  const std::vector<Track>& tracks = horizontal ? grid.columns : grid.rows;
  NSArray<NSLayoutGuide*>* trackGuides = horizontal ? guides.columns : guides.rows;
  const std::size_t count = tracks.size();
  const auto require = [out](NSLayoutConstraint* constraint) { [out addObject:constraint]; };
  const auto prefer = [out](NSLayoutConstraint* constraint, NSLayoutPriority priority) {
    constraint.priority = priority;
    [out addObject:constraint];
  };
  const auto track = [trackGuides, horizontal](std::size_t index) { return edges(trackGuides[index], horizontal); };

  std::vector<char> present(count, 0);
  std::vector<char> occupied(count, 0);
  std::vector<char> expanding(count, 0);
  std::vector<char> growing(count, 0);
  for (std::size_t i = 0; i < grid.items.size(); ++i) {
    if (visible[i] != kShown) { continue; }
    const Item& item = grid.items[i];
    const auto [first, last] = span(item, horizontal, count);
    for (std::size_t t = first; t <= last; ++t) {
      present[t] = 1;
      if (!item.spacer) { occupied[t] = 1; }
    }
    if (first != last || !item.regime) { continue; }
    const Axis axis = effectiveAxis(item, horizontal);
    if (axis.expands) { expanding[first] = 1; }
    if (axis.grows || axis.expands) { growing[first] = 1; }
  }
  int stretch = 0;
  bool anyExpanding = false;
  for (std::size_t t = 0; t < count; ++t) {
    if (present[t] == 0) { continue; }
    stretch += std::max(0, tracks[t].stretch);
    anyExpanding = anyExpanding || expanding[t] != 0;
  }
  std::vector<CGFloat> weight(count, 0);
  std::size_t base = count;
  for (std::size_t t = 0; t < count; ++t) {
    if (present[t] == 0) { continue; }
    if (horizontal && grid.equalColumns) { weight[t] = 1; }
    else if (stretch > 0) { weight[t] = static_cast<CGFloat>(std::max(0, tracks[t].stretch)); }
    else if (anyExpanding) { weight[t] = expanding[t] != 0 ? 1 : 0; }
    else { weight[t] = growing[t] != 0 ? 1 : 0; }
    if (weight[t] > 0 && base == count) { base = t; }
  }
  const bool sharing = base < count;

  // The content box inside the margins, and the block of tracks within it.
  const Edges outer = edges(host, horizontal);
  const Edges inner = edges(guides.content, horizontal);
  const Edges block = edges(guides.block, horizontal);
  const CGFloat startInset = horizontal ? grid.margins.left : grid.margins.top;
  const CGFloat endInset = horizontal ? grid.margins.right : grid.margins.bottom;
  require([inner.start constraintEqualToAnchor:outer.start constant:startInset]);
  require([inner.end constraintEqualToAnchor:outer.end constant:-endInset]);
  require([block.start constraintEqualToAnchor:track(0).start]);
  require([block.end constraintEqualToAnchor:track(count - 1).end]);
  const Place place = horizontal ? grid.content : Place::Leading;
  if (sharing) {
    require([block.start constraintEqualToAnchor:inner.start]);
    require([block.end constraintEqualToAnchor:inner.end]);
  } else if (place == Place::Trailing) {
    require([block.end constraintEqualToAnchor:inner.end]);
    require([block.start constraintGreaterThanOrEqualToAnchor:inner.start]);
  } else if (place == Place::Center) {
    require([block.centre constraintEqualToAnchor:inner.centre]);
    require([block.start constraintGreaterThanOrEqualToAnchor:inner.start]);
  } else {
    require([block.start constraintEqualToAnchor:inner.start]);
    require([block.end constraintLessThanOrEqualToAnchor:inner.end]);
  }
  // Without a size from its parent the layout takes its natural size.
  prefer([outer.size constraintEqualToConstant:0], kHug);

  const CGFloat gap = horizontal ? grid.horizontalSpacing : grid.verticalSpacing;
  const Edges crossHost = edges(host, !horizontal);
  bool seen = false;
  for (std::size_t t = 0; t < count; ++t) {
    const Edges g = track(t);
    const Edges cross = edges(trackGuides[t], !horizontal);
    require([cross.start constraintEqualToAnchor:crossHost.start]);
    require([cross.size constraintEqualToConstant:0]);
    if (t > 0) {
      require([g.start constraintEqualToAnchor:track(t - 1).end constant:(occupied[t] != 0 && seen) ? gap : 0]);
    }
    if (occupied[t] != 0) { seen = true; }
    require([g.size constraintGreaterThanOrEqualToConstant:0]);
    const CGFloat minimum = std::max<CGFloat>(0, tracks[t].minimum);
    if (present[t] == 0) {
      prefer([g.size constraintEqualToConstant:minimum], kLimit);
      continue;
    }
    if (minimum > 0) { prefer([g.size constraintGreaterThanOrEqualToConstant:minimum], kLimit); }
    if (weight[t] <= 0) {
      prefer([g.size constraintEqualToConstant:0], kTrackHug);
    } else if (t != base) {
      prefer([g.size constraintEqualToAnchor:track(base).size multiplier:weight[t] / weight[base]], kShare);
    }
  }

  for (std::size_t i = 0; i < grid.items.size(); ++i) {
    const Item& item = grid.items[i];
    NSView* view = item.view;
    const Edges v = edges(view, horizontal);
    const Edges cell = edges(guides.cells[i], horizontal);
    if (visible[i] != kShown) {
      // A hidden item takes no space; its own content keeps a position.
      if (visible[i] == kHidden) { require([v.start constraintEqualToAnchor:outer.start]); }
      require([cell.start constraintEqualToAnchor:outer.start]);
      require([cell.size constraintEqualToConstant:0]);
      continue;
    }
    const auto [first, last] = span(item, horizontal, count);
    require([cell.start constraintEqualToAnchor:track(first).start]);
    require([cell.end constraintEqualToAnchor:track(last).end]);
    require([v.start constraintGreaterThanOrEqualToAnchor:cell.start]);
    require([v.end constraintLessThanOrEqualToAnchor:cell.end]);
    const Axis axis = effectiveAxis(item, horizontal);
    if (item.formLabel && !horizontal && item.labelLimit > 0) {
      // As QFormLayout: centred beside a one-line field, near the top of a
      // tall row.
      prefer([v.centre constraintEqualToAnchor:cell.centre], kLabelCentre);
      prefer([v.start constraintLessThanOrEqualToAnchor:cell.start constant:item.labelLimit], kLimit);
    } else if (axis.place == Place::Leading) {
      require([v.start constraintEqualToAnchor:cell.start]);
    } else if (axis.place == Place::Trailing) {
      require([v.end constraintEqualToAnchor:cell.end]);
    } else {
      require([v.centre constraintEqualToAnchor:cell.centre]);
    }
    if (axis.minimum > 0) { prefer([v.size constraintGreaterThanOrEqualToConstant:axis.minimum], kLimit); }
    if (axis.maximum >= 0) {
      prefer([v.size constraintLessThanOrEqualToConstant:std::max(axis.maximum, axis.minimum)], kLimit);
    }
    if (axis.cap) {
      if (axis.preferred >= 0) {
        prefer([v.size constraintLessThanOrEqualToConstant:std::max(axis.preferred, axis.minimum)], kSoftCap);
      } else {
        // Without a shared hint the control keeps its own content size.
        const NSLayoutConstraintOrientation orientation =
            horizontal ? NSLayoutConstraintOrientationHorizontal : NSLayoutConstraintOrientationVertical;
        if ([view contentHuggingPriorityForOrientation:orientation] < kSoftCap) {
          [view setContentHuggingPriority:kSoftCap forOrientation:orientation];
        }
      }
    }
    // A capped item without a shared hint keeps its natural size: its
    // control's content, or a nested layout's own items at their hints.
    if (axis.fill && !(axis.cap && axis.preferred < 0)) {
      prefer([v.size constraintEqualToAnchor:cell.size], kFill);
    }
    prefer([v.size constraintEqualToConstant:0], kHug);
    // A stretched item's share starts from its minimum, as Qt's smartSizeHint:
    // its hint gives the layout's natural size but yields to the ratio.
    const bool stretched = stretch > 0 && first == last && tracks[first].stretch > 0;
    if (axis.preferred > 0) {
      const NSLayoutPriority priority = stretched ? kStretchedPreferred : kPreferred;
      if (first == last) {
        prefer([track(first).size constraintGreaterThanOrEqualToConstant:axis.preferred], priority);
      } else {
        prefer([cell.size constraintGreaterThanOrEqualToConstant:axis.preferred], priority);
      }
    }
  }
}

} // namespace
} // namespace mitcad::mac::layout

namespace mlayout = mitcad::mac::layout;

@interface MitcadNativeLayoutView ()
- (void)mitcadRebuildConstraints;
- (void)mitcadItemsChanged;
- (BOOL)mitcadStretchesAlong:(BOOL)horizontal;
- (BOOL)mitcadOwns:(const mitcad::mac::layout::Item&)item;
@end

@implementation MitcadNativeLayoutView {
  mitcad::mac::layout::Grid _grid;
  NSMutableArray<NSLayoutConstraint*>* _active;
  NSMutableArray<NSView*>* _observed;
  NSMutableArray<NSLayoutGuide*>* _columns;
  NSMutableArray<NSLayoutGuide*>* _rows;
  NSMutableArray<NSLayoutGuide*>* _cells;
  NSLayoutGuide* _content;
  NSLayoutGuide* _block;
  __weak NSView* _detaching;  // an item being taken away by another owner
  BOOL _applying;
}

// Another owner may move an item's view elsewhere (a secondary window's
// button footer): its cell closes at once, as a hidden item's does.
- (void)willRemoveSubview:(NSView*)subview {
  [super willRemoveSubview:subview];
  if (_applying) { return; }
  _detaching = subview;
  [self mitcadItemsChanged];
  _detaching = nil;
}

- (BOOL)mitcadOwns:(const mitcad::mac::layout::Item&)item {
  return item.view != nil && item.view.superview == self && item.view != _detaching;
}

- (instancetype)initWithFrame:(NSRect)frame {
  self = [super initWithFrame:frame];
  if (self != nil) {
    _active = [NSMutableArray array];
    _observed = [NSMutableArray array];
    _columns = [NSMutableArray array];
    _rows = [NSMutableArray array];
    _cells = [NSMutableArray array];
    self.translatesAutoresizingMaskIntoConstraints = NO;
  }
  return self;
}

- (void)dealloc {
  // NSView's dealloc removes the subviews: no rebuild of a view going away.
  _applying = YES;
  for (NSView* view in _observed) {
    [view removeObserver:self forKeyPath:@"hidden" context:&mlayout::kHiddenToken];
  }
}

- (void)applyGrid:(const mitcad::mac::layout::Grid&)grid {
  [NSLayoutConstraint deactivateConstraints:_active];
  [_active removeAllObjects];
  for (NSView* view in _observed) {
    [view removeObserver:self forKeyPath:@"hidden" context:&mlayout::kHiddenToken];
  }
  [_observed removeAllObjects];
  for (NSLayoutGuide* old in [self.layoutGuides copy]) { [self removeLayoutGuide:old]; }
  [_columns removeAllObjects];
  [_rows removeAllObjects];
  [_cells removeAllObjects];
  _grid = grid;
  if (_grid.columns.empty()) { _grid.columns.emplace_back(); }
  if (_grid.rows.empty()) { _grid.rows.emplace_back(); }
  NSMutableSet<NSView*>* kept = [NSMutableSet set];
  for (const mlayout::Item& item : _grid.items) {
    if (item.view != nil) { [kept addObject:item.view]; }
  }
  _applying = YES;
  for (NSView* child in [self.subviews copy]) {
    if (![kept containsObject:child]) { [child removeFromSuperview]; }
  }
  _applying = NO;
  const auto guide = [self]() {
    NSLayoutGuide* created = [[NSLayoutGuide alloc] init];
    [self addLayoutGuide:created];
    return created;
  };
  for (std::size_t i = 0; i < _grid.columns.size(); ++i) { [_columns addObject:guide()]; }
  for (std::size_t i = 0; i < _grid.rows.size(); ++i) { [_rows addObject:guide()]; }
  _content = guide();
  _block = guide();
  // Every item keeps a view and a cell guide, so indices stay aligned.
  for (mlayout::Item& item : _grid.items) {
    if (item.view == nil) { item.view = [[NSView alloc] initWithFrame:NSZeroRect]; }
    NSView* view = item.view;
    view.translatesAutoresizingMaskIntoConstraints = NO;
    if (view.superview != self) { [self addSubview:view]; }
    if (item.entry) {
      [view setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
                                     forOrientation:NSLayoutConstraintOrientationHorizontal];
    }
    if (![_observed containsObject:view]) {
      [view addObserver:self forKeyPath:@"hidden" options:0 context:&mlayout::kHiddenToken];
      [_observed addObject:view];
    }
    [_cells addObject:guide()];
  }
  [self mitcadRebuildConstraints];
}

- (void)mitcadRebuildConstraints {
  [NSLayoutConstraint deactivateConstraints:_active];
  [_active removeAllObjects];
  std::vector<char> visible(_grid.items.size(), mlayout::kHidden);
  for (std::size_t i = 0; i < _grid.items.size(); ++i) {
    const mlayout::Item& item = _grid.items[i];
    visible[i] = ![self mitcadOwns:item] ? mlayout::kDetached : mlayout::shown(item) ? mlayout::kShown : mlayout::kHidden;
  }
  const mlayout::Guides guides{_columns, _rows, _cells, _content, _block};
  mlayout::constrainAxis(self, _grid, guides, true, visible, _active);
  mlayout::constrainAxis(self, _grid, guides, false, visible, _active);
  [NSLayoutConstraint activateConstraints:_active];
}

- (void)mitcadItemsChanged {
  // Nothing rebuilds while this view applies a grid or is deallocated, also
  // when a child layout reports a change during that teardown.
  if (_applying) { return; }
  [self mitcadRebuildConstraints];
  // The parent's spacing and shares follow this layout's shown items.
  NSView* parent = self.superview;
  if ([parent isKindOfClass:MitcadNativeLayoutView.class]) { [(MitcadNativeLayoutView*)parent mitcadItemsChanged]; }
}

- (void)observeValueForKeyPath:(NSString*)keyPath
                      ofObject:(id)object
                        change:(NSDictionary<NSKeyValueChangeKey, id>*)change
                       context:(void*)context {
  if (context != &mlayout::kHiddenToken) {
    [super observeValueForKeyPath:keyPath ofObject:object change:change context:context];
    return;
  }
  [self mitcadItemsChanged];
}

- (BOOL)layoutEmpty {
  for (const auto& item : _grid.items) {
    if (!item.spacer && [self mitcadOwns:item] && mlayout::shown(item)) { return NO; }
  }
  return YES;
}

- (BOOL)mitcadStretchesAlong:(BOOL)horizontal {
  const std::vector<mlayout::Track>& tracks = horizontal ? _grid.columns : _grid.rows;
  for (const mlayout::Item& item : _grid.items) {
    if (![self mitcadOwns:item] || !mlayout::shown(item)) { continue; }
    const auto [first, last] = mlayout::span(item, horizontal, tracks.size());
    for (std::size_t t = first; t <= last; ++t) {
      if (tracks[t].stretch > 0) { return YES; }
    }
  }
  return NO;
}

- (BOOL)growsAlong:(BOOL)horizontal {
  if (_grid.alwaysGrows || (horizontal && _grid.equalColumns)) { return YES; }
  for (const mlayout::Item& item : _grid.items) {
    if (![self mitcadOwns:item] || !mlayout::shown(item)) { continue; }
    const mlayout::Axis axis = mlayout::effectiveAxis(item, horizontal);
    if (axis.grows || axis.expands) { return YES; }
  }
  return [self mitcadStretchesAlong:horizontal];
}

- (BOOL)expandsAlong:(BOOL)horizontal {
  for (const mlayout::Item& item : _grid.items) {
    // A form field expands its form even while it stays at its size hint.
    if ([self mitcadOwns:item] && mlayout::shown(item) && mlayout::baseAxis(item, horizontal).expands) { return YES; }
  }
  return [self mitcadStretchesAlong:horizontal];
}

@end

namespace mitcad::mac::layout {

NSView* build(QLayout* layout, const std::function<NSView*(QLayoutItem*)>& render, bool equalColumns) {
  if (layout == nullptr) { return nil; }
  Grid grid;
  if (auto* form = qobject_cast<QFormLayout*>(layout)) { addForm(form, render, grid); }
  else if (auto* table = qobject_cast<QGridLayout*>(layout)) { addGrid(table, render, grid); }
  else if (auto* box = qobject_cast<QBoxLayout*>(layout)) { addBox(box, render, grid); }
  else { addList(layout, render, grid); }
  const QMargins margins = layout->contentsMargins();
  grid.margins = NSEdgeInsetsMake(margins.top(), margins.left(), margins.bottom(), margins.right());
  grid.equalColumns = equalColumns;
  MitcadNativeLayoutView* view = [[MitcadNativeLayoutView alloc] initWithFrame:NSZeroRect];
  [view applyGrid:grid];
  return view;
}

std::vector<int> macButtonOrder(const std::vector<int>& roles) {
  std::vector<int> order;
  const auto group = [&roles](QDialogButtonBox::ButtonRole role, bool reverse) {
    std::vector<int> indices;
    for (std::size_t i = 0; i < roles.size(); ++i) {
      if (roles[i] == int(role)) { indices.push_back(static_cast<int>(i)); }
    }
    if (reverse) { std::reverse(indices.begin(), indices.end()); }
    return indices;
  };
  const auto append = [&order](const std::vector<int>& indices) {
    order.insert(order.end(), indices.begin(), indices.end());
  };
  // QDialogButtonBox's MacLayout, including its gaps around destructive
  // buttons. Within a role, buttons keep the box's order (some reversed).
  for (const QDialogButtonBox::ButtonRole role : {QDialogButtonBox::HelpRole, QDialogButtonBox::ResetRole,
                                                 QDialogButtonBox::ApplyRole, QDialogButtonBox::ActionRole}) {
    append(group(role, false));
  }
  const bool helpers = !order.empty();
  order.push_back(kStretch);
  const std::vector<int> destructive = group(QDialogButtonBox::DestructiveRole, true);
  if (!destructive.empty()) {
    if (helpers) { order.push_back(kGap); }
    append(destructive);
    order.push_back(kGap);
  }
  const std::vector<int> accepting = group(QDialogButtonBox::AcceptRole, false);
  // Accepting buttons after the first are alternates, left of rejection.
  if (accepting.size() > 1) { append(std::vector<int>(accepting.rbegin(), accepting.rend() - 1)); }
  append(group(QDialogButtonBox::RejectRole, true));
  if (!accepting.empty()) { order.push_back(accepting.front()); }
  append(group(QDialogButtonBox::NoRole, true));
  append(group(QDialogButtonBox::YesRole, true));
  return order;
}

QString buttonRoleKey(const QDialogButtonBox* box) {
  QString key = box->orientation() == Qt::Vertical ? QStringLiteral("v") : QStringLiteral("h");
  for (QAbstractButton* button : box->buttons()) {
    key += QStringLiteral(";%1:%2").arg(reinterpret_cast<quintptr>(button), 0, 16).arg(int(box->buttonRole(button)));
  }
  return key;
}

NSView* buttonRow() { return [[MitcadNativeLayoutView alloc] initWithFrame:NSZeroRect]; }

void arrangeButtons(NSView* row, const QDialogButtonBox* box, const std::vector<NSView*>& buttons) {
  if (![row isKindOfClass:MitcadNativeLayoutView.class]) { return; }
  const QList<QAbstractButton*> sources = box->buttons();
  std::vector<int> roles;
  roles.reserve(static_cast<std::size_t>(sources.size()));
  for (QAbstractButton* button : sources) { roles.push_back(int(box->buttonRole(button))); }
  const bool horizontal = box->orientation() == Qt::Horizontal;
  Grid grid;
  std::vector<Track>& tracks = horizontal ? grid.columns : grid.rows;
  (horizontal ? grid.rows : grid.columns).assign(1, Track{});
  for (const int entry : macButtonOrder(roles)) {
    Item item;
    if (entry >= 0) {
      const std::size_t index = static_cast<std::size_t>(entry);
      if (index >= buttons.size() || buttons[index] == nil) { continue; }
      const QAbstractButton* button = sources.at(entry);
      item.view = buttons[index];
      item.horizontal = widgetAxis(button, true, {});
      item.vertical = widgetAxis(button, false, {});
    } else {
      // The stretch between helpers and responses, or a fixed gap.
      Axis along;
      along.preferred = entry == kGap ? kMacGap : 0;
      along.minimum = along.preferred;
      along.maximum = entry == kGap ? kMacGap : -1;
      along.grows = entry != kGap;
      along.expands = along.grows;
      along.cap = !along.grows;
      Axis across;
      across.preferred = 0;
      across.maximum = 0;
      across.grows = false;
      across.cap = true;
      item.view = [[NSView alloc] initWithFrame:NSZeroRect];
      item.spacer = true;
      item.horizontal = horizontal ? along : across;
      item.vertical = horizontal ? across : along;
    }
    (horizontal ? item.column : item.row) = static_cast<int>(tracks.size());
    tracks.emplace_back();
    grid.items.push_back(std::move(item));
  }
  (horizontal ? grid.horizontalSpacing : grid.verticalSpacing) = kButtonSpacing;
  auto* view = (MitcadNativeLayoutView*)row;
  [view applyGrid:grid];
  view.contentKey = buttonRoleKey(box).toNSString();
}

} // namespace mitcad::mac::layout
