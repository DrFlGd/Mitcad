// SPDX-License-Identifier: MIT
#include "NativeTableCells.hpp"

#import <objc/runtime.h>

#include <algorithm>
#include <cmath>

#include <QAbstractItemModel>
#include <QAbstractItemView>
#include <QBrush>
#include <QColor>
#include <QFont>
#include <QHash>
#include <QHeaderView>
#include <QModelIndex>
#include <QPalette>
#include <QString>
#include <QStringList>
#include <QTableView>
#include <QTreeView>
#include <QVariant>
#include <QWidget>
#include <QtGlobal>
#include <QtLogging>

#include "../NativeDialogs.hpp"
#include "../../framework/Theme.hpp"

// Refits a header-driven table's stretching columns after the visible area
// or a column changes. Owned by its table (an associated object).
@interface MitcadNativeColumnFit : NSObject {
@public
  __weak NSTableView* table;
  BOOL fitting;
}
- (void)fit:(NSNotification*)notification;
@end

@implementation MitcadNativeColumnFit
- (void)fit:(NSNotification*)notification {
  (void)notification;
  NSTableView* target = table;
  if (target == nil || fitting) { return; }
  fitting = YES; // its own width changes post the same notification
  if (NSTextField* edited = mitcad::mac::editedCellField(target)) {
    mitcad::mac::logNativeCellEdit("the columns are fitted during an edit", edited);
  }
  mitcad::mac::fitNativeColumns(target);
  fitting = NO;
}
- (void)dealloc { [NSNotificationCenter.defaultCenter removeObserver:self]; }
@end

namespace mitcad::mac {
namespace {

char g_columnFitKey = 0;

std::size_t mixed(std::size_t seed, std::size_t value) {
  return seed ^ (value + static_cast<std::size_t>(0x9e3779b97f4a7c15ULL) + (seed << 6) + (seed >> 2));
}

// The tint of a semantic fill behind a cell's text, as the shared delegate
// mixes about a tenth of the error colour into the base.
constexpr CGFloat kFillAlpha = 0.12;

QPalette shade(bool dark) {
  QPalette palette;
  palette.setColor(QPalette::Window, dark ? QColor(Qt::black) : QColor(Qt::white));
  return palette;
}

QColor roleColor(const QVariant& value) {
  if (value.typeId() == QMetaType::QBrush) {
    const QBrush brush = value.value<QBrush>();
    return brush.style() == Qt::NoBrush ? QColor() : brush.color();
  }
  if (value.typeId() == QMetaType::QColor) { return value.value<QColor>(); }
  return {};
}

NSColor* srgb(const QColor& color) {
  const QColor rgb = color.toRgb();
  return [NSColor colorWithSRGBRed:rgb.redF() green:rgb.greenF() blue:rgb.blueF() alpha:rgb.alphaF()];
}

// The theme's error and warning colours, on a light or a dark palette, are
// AppKit's dynamic red and orange; other colours stay what they are.
NSColor* semanticColor(const QColor& color, const QPalette& palette, bool text) {
  if (!color.isValid()) { return nil; }
  for (const bool dark : {false, true}) {
    if (color == errorColor(shade(dark))) {
      return text ? NSColor.systemRedColor : [NSColor.systemRedColor colorWithAlphaComponent:kFillAlpha];
    }
    if (color == warningColor(shade(dark))) {
      return text ? NSColor.systemOrangeColor : [NSColor.systemOrangeColor colorWithAlphaComponent:kFillAlpha];
    }
  }
  if (text) {
    if (color == palette.color(QPalette::Disabled, QPalette::Text) ||
        color == palette.color(QPalette::Disabled, QPalette::WindowText)) {
      return NSColor.disabledControlTextColor;
    }
    if (color == palette.color(QPalette::PlaceholderText)) { return NSColor.placeholderTextColor; }
    if (color == palette.color(QPalette::Text) || color == palette.color(QPalette::WindowText)) { return nil; }
  }
  return srgb(color);
}

NSFont* cellFont(const QModelIndex& index, const QAbstractItemView* view) {
  const CGFloat base = NSFont.systemFontSize;
  const QVariant value = index.data(Qt::FontRole);
  if (value.typeId() != QMetaType::QFont) { return [NSFont systemFontOfSize:base]; }
  const QFont font = value.value<QFont>();
  CGFloat size = base;
  const qreal reference = view->font().pointSizeF();
  if (font.pointSizeF() > 0 && reference > 0 && std::abs(font.pointSizeF() - reference) > 0.01) {
    size = base * font.pointSizeF() / reference; // a smaller or larger variant of the view's font
  }
  const int weight = static_cast<int>(font.weight());
  const NSFontWeight native = weight >= static_cast<int>(QFont::Bold)       ? NSFontWeightBold
                              : weight >= static_cast<int>(QFont::DemiBold) ? NSFontWeightSemibold
                              : weight >= static_cast<int>(QFont::Medium)   ? NSFontWeightMedium
                                                                            : NSFontWeightRegular;
  NSFont* result = [NSFont systemFontOfSize:size weight:native];
  if (font.italic()) { result = [NSFontManager.sharedFontManager convertFont:result toHaveTrait:NSItalicFontMask]; }
  return result;
}

} // namespace

NativeCellStyle nativeCellStyle(const QModelIndex& index, const QAbstractItemView* view) {
  NativeCellStyle style;
  style.font = cellFont(index, view);
  if (index.data(NativeRefusedRole).toBool()) {
    // As styleParameterCell: the error text on a tenth of the error colour.
    style.text = NSColor.systemRedColor;
    style.fill = [NSColor.systemRedColor colorWithAlphaComponent:kFillAlpha];
    return style;
  }
  const QPalette palette = view->palette();
  style.text = semanticColor(roleColor(index.data(Qt::ForegroundRole)), palette, true);
  style.fill = semanticColor(roleColor(index.data(Qt::BackgroundRole)), palette, false);
  return style;
}

void elideNativeCell(NSTextField* field, Qt::TextElideMode mode) {
  field.usesSingleLineMode = YES;
  field.maximumNumberOfLines = 1;
  field.preferredMaxLayoutWidth = 0;
  field.cell.wraps = NO;
  field.cell.scrollable = NO;
  field.cell.truncatesLastVisibleLine = YES;
  // Last: a truncating mode keeps the cell from wrapping.
  field.lineBreakMode = mode == Qt::ElideLeft     ? NSLineBreakByTruncatingHead
                        : mode == Qt::ElideMiddle ? NSLineBreakByTruncatingMiddle
                        : mode == Qt::ElideNone   ? NSLineBreakByClipping
                                                  : NSLineBreakByTruncatingTail;
}

int nativeTextToneKind(const QWidget* widget) {
  const QString tone = widget == nullptr ? QString() : widget->property(kSemanticTextToneProperty).toString();
  return tone == QLatin1String("error") ? 1 : tone == QLatin1String("warning") ? 2 : 0;
}

NSColor* nativeTextTone(int kind) {
  return kind == 1 ? NSColor.systemRedColor : kind == 2 ? NSColor.systemOrangeColor : NSColor.labelColor;
}

QHeaderView* nativeHeader(const QAbstractItemView* view) {
  if (const auto* tree = qobject_cast<const QTreeView*>(view)) { return tree->header(); }
  if (const auto* table = qobject_cast<const QTableView*>(view)) { return table->horizontalHeader(); }
  return nullptr;
}

bool applyNativeColumns(NSTableView* table, const QAbstractItemView* view, NativeColumnState& state) {
  QHeaderView* header = nativeHeader(view);
  if (table == nil || header == nullptr || view->model() == nullptr) { return false; }
  NSArray<NSTableColumn*>* columns = table.tableColumns;
  // A stretched last section is the last one shown, in visual order.
  int last = -1;
  for (int visual = header->count() - 1; visual >= 0 && last < 0; --visual) {
    const int logical = header->logicalIndex(visual);
    if (logical >= 0 && !header->isSectionHidden(logical)) { last = logical; }
  }
  // The shared state of each column, read from Qt only.
  struct Shared {
    int section = -1;
    QString title;
    QString tip;
    bool hidden = false;
    bool stretch = false;
    QHeaderView::ResizeMode mode = QHeaderView::Interactive;
    int size = 0;
    int minimum = 16;
  };
  std::vector<Shared> shared(static_cast<std::size_t>(columns.count));
  std::size_t signature = mixed(0, static_cast<std::size_t>(columns.count));
  for (std::size_t i = 0; i < shared.size(); ++i) {
    Shared& s = shared[i];
    const int section = QString::fromNSString(columns[i].identifier).toInt();
    if (section < 0 || section >= header->count()) { continue; }
    s.section = section;
    s.title = view->model()->headerData(section, Qt::Horizontal).toString();
    s.tip = view->model()->headerData(section, Qt::Horizontal, Qt::ToolTipRole).toString();
    s.hidden = header->isSectionHidden(section);
    s.mode = header->sectionResizeMode(section);
    s.stretch = s.mode == QHeaderView::Stretch || (header->stretchLastSection() && section == last);
    // A stretching section's own size follows the hidden Qt view's layout;
    // its native width comes from fitting instead.
    s.size = s.stretch ? 0 : header->sectionSize(section);
    const int smallest = header->minimumSectionSize();
    s.minimum = std::max(16, s.stretch || s.size <= 0 ? smallest : std::min(smallest, s.size));
    for (const std::size_t part : {static_cast<std::size_t>(section), qHash(s.title), qHash(s.tip),
                                   static_cast<std::size_t>(s.hidden), static_cast<std::size_t>(s.stretch),
                                   static_cast<std::size_t>(s.mode), static_cast<std::size_t>(s.size),
                                   static_cast<std::size_t>(s.minimum)}) {
      signature = mixed(signature, part);
    }
  }
  if (signature == state.signature && state.sizes.size() == shared.size()) { return false; }
  state.signature = signature;
  if (state.sizes.size() != shared.size()) { state.sizes.assign(shared.size(), -1); }
  for (std::size_t i = 0; i < shared.size(); ++i) {
    const Shared& s = shared[i];
    if (s.section < 0) { continue; }
    NSTableColumn* column = columns[i];
    column.title = s.title.toNSString();
    column.headerToolTip = s.tip.isEmpty() ? nil : s.tip.toNSString();
    column.hidden = s.hidden;
    // The autoresizing flag marks a stretching column for fitNativeColumns;
    // AppKit's own column autoresizing is off. As in the shared header, a
    // stretching or fixed section is not resized by hand.
    column.resizingMask = s.stretch                             ? NSTableColumnAutoresizingMask
                          : s.mode == QHeaderView::Interactive ? NSTableColumnUserResizingMask
                                                               : NSTableColumnNoResizing;
    column.minWidth = s.minimum;
    // A section's width follows a change of its shared size; a native user
    // resize stays until then. A stretching column's comes from fitting.
    const int taken = s.stretch ? 0 : s.size;
    if (taken > 0 && state.sizes[i] != taken) { column.width = taken; }
    state.sizes[i] = taken;
  }
  table.columnAutoresizingStyle = NSTableViewNoColumnAutoresizing;
  fitNativeColumns(table);
  return true;
}

void fitNativeColumns(NSTableView* table) {
  NSScrollView* scroll = table.enclosingScrollView;
  if (scroll == nil || table.columnAutoresizingStyle != NSTableViewNoColumnAutoresizing) { return; }
  const CGFloat available = scroll.contentView.bounds.size.width;
  if (available <= 0) { return; }
  CGFloat used = 0;
  NSMutableArray<NSTableColumn*>* stretching = [NSMutableArray new];
  for (NSTableColumn* column in table.tableColumns) {
    if (column.hidden) { continue; }
    used += table.intercellSpacing.width;
    if ((column.resizingMask & NSTableColumnAutoresizingMask) != 0) { [stretching addObject:column]; }
    else { used += column.width; }
  }
  if (stretching.count == 0) { return; }
  const CGFloat share = std::floor(std::max<CGFloat>(0, available - used) / static_cast<CGFloat>(stretching.count));
  for (NSTableColumn* column in stretching) {
    const CGFloat width = std::max(column.minWidth, share);
    if (std::abs(column.width - width) >= 0.5) { column.width = width; }
  }
}

void followNativeColumns(NSTableView* table) {
  NSClipView* clip = table.enclosingScrollView.contentView;
  if (clip == nil || objc_getAssociatedObject(table, &g_columnFitKey) != nil) { return; }
  MitcadNativeColumnFit* fit = [MitcadNativeColumnFit new];
  fit->table = table;
  clip.postsFrameChangedNotifications = YES;
  NSNotificationCenter* center = NSNotificationCenter.defaultCenter;
  [center addObserver:fit selector:@selector(fit:) name:NSViewFrameDidChangeNotification object:clip];
  [center addObserver:fit selector:@selector(fit:) name:NSTableViewColumnDidResizeNotification object:table];
  [center addObserver:fit selector:@selector(fit:) name:NSOutlineViewColumnDidResizeNotification object:table];
  objc_setAssociatedObject(table, &g_columnFitKey, fit, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
}

NSTableView* owningTable(NSView* view) {
  for (NSView* parent = view.superview; parent != nil; parent = parent.superview) {
    if ([parent isKindOfClass:NSTableView.class]) { return (NSTableView*)parent; }
  }
  return nil;
}

void logNativeCellEdit(const char* what, NSView* view) {
  static const bool enabled = qEnvironmentVariableIntValue("MITCAD_LOG_NATIVE_EDITS") == 1;
  if (!enabled) { return; }
  // The cell's or the table's identifier: the nearest one up the views.
  QString name;
  for (NSView* parent = view; parent != nil && name.isEmpty(); parent = parent.superview) {
    name = QString::fromNSString(parent.accessibilityIdentifier);
  }
  QStringList frames;
  NSArray<NSString*>* stack = NSThread.callStackSymbols;
  for (NSUInteger i = 1; i < stack.count && i < 18; ++i) { frames << QString::fromNSString(stack[i]).simplified(); }
  qInfo().noquote() << QStringLiteral("Native cell edit: %1 (%2), focus %3; from %4")
                           .arg(QString::fromLatin1(what), name,
                                QString::fromNSString(NSStringFromClass(view.window.firstResponder.class)),
                                frames.join(QStringLiteral(" | ")));
}

void selectEditedText(NSTextField* field) {
  if (field.currentEditor != nil) { [field.currentEditor selectAll:nil]; }
  else { [field selectText:nil]; }
}

NSTextField* editedCellField(NSTableView* table) {
  NSResponder* responder = table.window.firstResponder;
  if (![responder isKindOfClass:NSTextView.class] || !((NSTextView*)responder).isFieldEditor) { return nil; }
  id edited = ((NSTextView*)responder).delegate;
  if (![edited isKindOfClass:NSTextField.class]) { return nil; }
  NSTextField* field = (NSTextField*)edited;
  return [field isDescendantOf:table] ? field : nil;
}

} // namespace mitcad::mac
