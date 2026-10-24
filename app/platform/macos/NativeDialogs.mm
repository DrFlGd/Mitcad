// SPDX-License-Identifier: MIT
#include "NativeKeys.hpp"
#include "../NativeDialogs.hpp"

#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

#include <algorithm>
#include <cmath>
#include <functional>
#include <memory>
#include <vector>

#include <QAbstractButton>
#include <QAbstractItemView>
#include <QAbstractSpinBox>
#include <QAction>
#include <QBoxLayout>
#include <QBuffer>
#include <QCheckBox>
#include <QComboBox>
#include <QCoreApplication>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDoubleSpinBox>
#include <QDir>
#include <QEnterEvent>
#include <QEventLoop>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QFrame>
#include <QGridLayout>
#include <QGroupBox>
#include <QHeaderView>
#include <QItemSelectionModel>
#include <QKeyEvent>
#include <QKeySequence>
#include <QShortcut>
#include <QKeySequenceEdit>
#include <QLabel>
#include <QLineEdit>
#include <QListView>
#include <QMenu>
#include <QMessageBox>
#include <QPersistentModelIndex>
#include <QPlainTextEdit>
#include <QPointer>
#include <QProgressBar>
#include <QProgressDialog>
#include <QPushButton>
#include <QRadioButton>
#include <QRegularExpression>
#include <QScrollArea>
#include <QShowEvent>
#include <QSignalBlocker>
#include <QSlider>
#include <QSpinBox>
#include <QSplitter>
#include <QStackedWidget>
#include <QTabWidget>
#include <QTableView>
#include <QTextDocument>
#include <QTextBlock>
#include <QTextFragment>
#include <QTextEdit>
#include <QTimer>
#include <QToolButton>
#include <QTreeView>
#include <QValidator>
#include <QWidget>
#include <QtLogging>

#include "../../report/ReportDialogs.hpp"
#include "../../framework/SettingsWindow.hpp"

namespace mitcad {
class AppKitWidgetView;
class NativeDialogController;
class NativeFilePanel;
class NativeColorPanel;
class NativeMenuPopup;
}

@interface MitcadNativeMenuTarget : NSObject <NSMenuDelegate, NSMenuItemValidation> {
@public
  mitcad::NativeMenuPopup* owner;
}
- (void)perform:(NSMenuItem*)sender;
@end

@interface MitcadNativeModelNode : NSObject {
@public
  QPersistentModelIndex index;
  NSMutableArray<MitcadNativeModelNode*>* children;
}
@end
@implementation MitcadNativeModelNode
@end

@interface MitcadNativeComboItem : NSObject {
@public
  QPersistentModelIndex index;
}
@end
@implementation MitcadNativeComboItem
@end

@interface MitcadNativeWidgetTarget : NSObject <NSTextFieldDelegate, NSComboBoxDelegate, NSTextViewDelegate, NSTabViewDelegate,
                                               NSTableViewDataSource, NSTableViewDelegate,
                                               NSOutlineViewDataSource, NSOutlineViewDelegate> {
@public
  mitcad::AppKitWidgetView* owner;
}
- (void)perform:(id)sender;
- (void)doubleClick:(id)sender;
@end

@interface MitcadNativeModelCell : NSTableCellView <NSTextFieldDelegate> {
@public
  mitcad::AppKitWidgetView* owner;
  int binding;
  QPersistentModelIndex index;
}
- (void)checked:(id)sender;
@end

// Only opted-in models forward hover. Persistent indices become invalid
// when an asynchronous preview replaces the checklist's rows.
@interface MitcadNativeModelHoverButton : NSButton {
@public
  QPointer<QAbstractItemView> presenter;
  QPersistentModelIndex modelIndex;
  NSTrackingArea* hoverArea;
}
- (void)publishHover:(BOOL)entered;
@end

@interface MitcadNativeWindowTarget : NSObject <NSWindowDelegate> {
@public
  mitcad::NativeDialogController* owner;
}
@end

@interface MitcadNativeSecondaryWindow : NSWindow
- (BOOL)performKeyEquivalent:(NSEvent*)event;
- (void)cancelOperation:(id)sender;
@end

@interface MitcadNativeFilePanelTarget : NSObject <NSOpenSavePanelDelegate> {
@public
  mitcad::NativeFilePanel* owner;
}
- (void)filterChanged:(id)sender;
@end

@interface MitcadNativeColorPanelTarget : NSObject <NSWindowDelegate> {
@public
  mitcad::NativeColorPanel* owner;
}
- (void)accept:(id)sender;
- (void)cancel:(id)sender;
@end

@interface MitcadNativeTable : NSTableView
@end
@interface MitcadNativeOutline : NSOutlineView
@end

@interface MitcadNativeShortcutField : NSTextField {
@public
  QPointer<QKeySequenceEdit> edit;
}
- (void)clearShortcut:(id)sender;
@end

// Opt-in hover forwarding for presenters that preview a button's candidates.
@interface MitcadNativeHoverButton : NSButton {
@public
  QPointer<QAbstractButton> presenter;
  NSTrackingArea* hoverArea;
}
@end

@interface MitcadNativeScreenshot : NSView {
@public
  QPointer<mitcad::report::ScreenshotView> presenter;
  NSImage* bitmap;
  qint64 imageKey;
  NSPoint from;
  NSPoint to;
  BOOL dragging;
  QRect drawnCrop;
}
- (void)refresh;
@end

namespace mitcad {
namespace {

__weak NSWindow* g_documentWindow = nil;
std::vector<NSWindow*> g_modalWindows;
quint64 g_widgetPropertyWrites = 0;

NSString* ns(const QString& text) {
  const QByteArray utf8 = text.toUtf8();
  return [[NSString alloc] initWithBytes:utf8.constData() length:static_cast<NSUInteger>(utf8.size())
                               encoding:NSUTF8StringEncoding];
}
QString qt(NSString* text) { return text == nil ? QString() : QString::fromUtf8(text.UTF8String); }
// Compare UTF-16 in place. Converting an unchanged QString to NSString on
// every safety-net pass would keep allocating even with guarded setters.
bool sameText(NSString* current, const QString& value) {
  if (current.length != static_cast<NSUInteger>(value.size())) { return false; }
  for (qsizetype i = 0; i < value.size(); ++i) {
    if ([current characterAtIndex:static_cast<NSUInteger>(i)] != value.at(i).unicode()) { return false; }
  }
  return true;
}
#define NATIVE_SET(object, property, value) do { \
  const auto desired = (value); \
  if ((object).property != desired) { (object).property = desired; ++g_widgetPropertyWrites; } \
} while (false)
#define NATIVE_STRING(object, property, value) do { \
  const QString desiredText = (value); \
  if (!sameText((object).property, desiredText)) { (object).property = ns(desiredText); ++g_widgetPropertyWrites; } \
} while (false)
QString title(QString text) {
  text.replace(QStringLiteral("&&"), QStringLiteral("\x01"));
  text.remove(QLatin1Char('&'));
  text.replace(QChar(1), QLatin1Char('&'));
  return text;
}
QString plain(const QString& text) {
  if (!Qt::mightBeRichText(text)) {
    return text;
  }
  QTextDocument document;
  document.setHtml(text);
  return document.toPlainText();
}
NSImage* image(const QPixmap& pixmap) {
  if (pixmap.isNull()) {
    return nil;
  }
  QByteArray data;
  QBuffer buffer(&data);
  buffer.open(QIODevice::WriteOnly);
  pixmap.save(&buffer, "PNG");
  return [[NSImage alloc] initWithData:[NSData dataWithBytes:data.constData()
                                                    length:static_cast<NSUInteger>(data.size())]];
}
NSStackView* stack(bool horizontal = false) {
  NSStackView* result = [[NSStackView alloc] initWithFrame:NSZeroRect];
  result.orientation = horizontal ? NSUserInterfaceLayoutOrientationHorizontal : NSUserInterfaceLayoutOrientationVertical;
  result.alignment = horizontal ? NSLayoutAttributeCenterY : NSLayoutAttributeLeading;
  result.spacing = 8;
  result.detachesHiddenViews = YES;
  result.translatesAutoresizingMaskIntoConstraints = NO;
  return result;
}
NSTextField* label(const QString& text) {
  NSTextField* result = [NSTextField wrappingLabelWithString:ns(plain(text))];
  result.font = [NSFont systemFontOfSize:NSFont.systemFontSize];
  result.selectable = YES;
  result.translatesAutoresizingMaskIntoConstraints = NO;
  [result setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
                                   forOrientation:NSLayoutConstraintOrientationHorizontal];
  return result;
}
void add(NSStackView* parent, NSView* child) {
  if (child == nil) {
    return;
  }
  child.translatesAutoresizingMaskIntoConstraints = NO;
  [parent addArrangedSubview:child];
  if (parent.orientation == NSUserInterfaceLayoutOrientationVertical) {
    [child.leadingAnchor constraintEqualToAnchor:parent.leadingAnchor constant:parent.edgeInsets.left].active = YES;
    [child.trailingAnchor constraintEqualToAnchor:parent.trailingAnchor constant:-parent.edgeInsets.right].active = YES;
  }
}
NSScrollView* scroll(NSView* document, CGFloat height = 160) {
  NSScrollView* view = [[NSScrollView alloc] initWithFrame:NSMakeRect(0, 0, 420, height)];
  view.borderType = NSBezelBorder;
  view.hasVerticalScroller = YES;
  view.hasHorizontalScroller = YES;
  view.autohidesScrollers = YES;
  view.documentView = document;
  if ([document isKindOfClass:NSStackView.class]) {
    document.translatesAutoresizingMaskIntoConstraints = NO;
    [document.leadingAnchor constraintEqualToAnchor:view.contentView.leadingAnchor].active = YES;
    [document.topAnchor constraintEqualToAnchor:view.contentView.topAnchor].active = YES;
    [document.widthAnchor constraintEqualToAnchor:view.contentView.widthAnchor].active = YES;
    [document.heightAnchor constraintGreaterThanOrEqualToAnchor:view.contentView.heightAnchor].active = YES;
    view.hasHorizontalScroller = NO;
  }
  view.translatesAutoresizingMaskIntoConstraints = NO;
  [view.heightAnchor constraintGreaterThanOrEqualToConstant:height].active = YES;
  return view;
}
NSWindow* currentHost(QWidget* widget);

void pumpNativeEvents(const std::function<bool()>& complete) {
  // A nested QEventLoop::exec can transfer control to NSApplication::run
  // in Qt's Cocoa dispatcher. Explicit bounded flushes keep Qt timers,
  // worker completions and AppKit input progressing together.
  while (!complete()) {
    @autoreleasepool {
      QCoreApplication::processEvents(QEventLoop::AllEvents);
      QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
      if (complete()) { break; }
      NSEvent* event = [NSApp nextEventMatchingMask:NSEventMaskAny
                                        untilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]
                                           inMode:NSDefaultRunLoopMode dequeue:YES];
      if (event != nil) { [NSApp sendEvent:event]; }
      [NSApp updateWindows];
    }
  }
}

void updateButtonPresentation(NSButton* native, QAbstractButton* button) {
  const auto* tool = qobject_cast<QToolButton*>(button);
  const auto style = tool == nullptr ? Qt::ToolButtonTextBesideIcon : tool->toolButtonStyle();
  const bool iconOnly = style == Qt::ToolButtonIconOnly && !button->icon().isNull();
  const bool textOnly = style == Qt::ToolButtonTextOnly;
  NATIVE_STRING(native, title, iconOnly ? QString() : title(button->text()));
  NATIVE_SET(native, imagePosition, textOnly ? NSNoImage : iconOnly || button->text().isEmpty() ? NSImageOnly
                                            : style == Qt::ToolButtonTextUnderIcon ? NSImageAbove : NSImageLeading);
  NATIVE_STRING(native, accessibilityLabel, plain(title(button->text().isEmpty() ? button->toolTip() : button->text())));
  NATIVE_STRING(native, toolTip, plain(button->toolTip().isEmpty() ? button->text() : button->toolTip()));
}

} // namespace

class AppKitWidgetView final : public QObject, public NativeWidgetView {
public:
  explicit AppKitWidgetView(QWidget* source) : m_source(source) {
    m_target = [MitcadNativeWidgetTarget new];
    m_target->owner = this;
    m_root = stack();
    m_root.edgeInsets = NSEdgeInsetsMake(16, 16, 16, 16);
    m_root.accessibilityIdentifier = ns(source->objectName());
    connect(source, &QObject::destroyed, this, [this] { m_root.hidden = YES; });
    m_refreshTimer.setSingleShot(true);
    connect(&m_refreshTimer, &QTimer::timeout, this, [this] { refreshPresenters(); });
    rebuild();
    // Qt has no notify signal for several presenter properties (label and
    // button text, spin ranges, read-only state, model replacement, row
    // hiding and index widgets). Only those gaps need a slow safety net.
    m_timer.setInterval(500);
    connect(&m_timer, &QTimer::timeout, this, [this] { checkModelPresentation(); refreshPresenters(); });
    m_timer.start();
  }
  ~AppKitWidgetView() override {
    m_timer.stop();
    m_refreshTimer.stop();
    m_target->owner = nullptr;
    for (MitcadNativeModelCell* cell in m_cells) { cell->owner = nullptr; }
    detachBindings();
  }
  void detachBindings() {
    for (Binding& binding : m_bindings) {
      for (const auto& connection : binding.connections) { disconnect(connection); }
      for (const auto& connection : binding.modelConnections) { disconnect(connection); }
      for (const auto& connection : binding.selectionConnections) { disconnect(connection); }
      if ([binding.control isKindOfClass:MitcadNativeHoverButton.class]) {
        ((MitcadNativeHoverButton*)binding.control)->presenter.clear();
      }
      if (binding.control != nil) { binding.control.target = nil; binding.control.action = nullptr; }
      if (binding.stepper != nil) { binding.stepper.target = nil; binding.stepper.action = nullptr; }
      if ([binding.control isKindOfClass:NSTextField.class]) {
        ((NSTextField*)binding.control).delegate = nil;
      }
      binding.tabs.delegate = nil;
      binding.text.delegate = nil;
      binding.linkText.delegate = nil;
      if (binding.table != nil) {
        binding.table.delegate = nil;
        binding.table.dataSource = nil;
      }
    }
  }
  void* nativeView() const override { return (__bridge void*)m_root; }
  QWidget* source() const { return m_source; }
  void focusInitial() {
    NSControl* first = nil;
    for (const Binding& binding : m_bindings) {
      if (binding.widget == nullptr || !binding.widget->isVisibleTo(m_source) || !binding.widget->isEnabled()) { continue; }
      if ([binding.control isKindOfClass:NSTextField.class] && ((NSTextField*)binding.control).editable) {
        if (first == nil) { first = binding.control; }
        if (binding.widget->hasFocus()) { first = binding.control; break; }
      }
    }
    if (first != nil) { [first.window makeFirstResponder:first]; [(NSTextField*)first selectText:nil]; }
  }

  void refresh() override {
    // Explicit caller refreshes also honour silent row/index-widget changes.
    checkModelPresentation();
    refreshPresenters();
  }
  void refreshPresenters() {
    if (m_source == nullptr || m_refreshing) {
      return;
    }
    m_refreshTimer.stop();
    m_refreshing = true;
    if (m_structureDirty) {
      const auto widgets = m_source->findChildren<QWidget*>();
      bool changed = widgets.size() != static_cast<qsizetype>(m_widgetTree.size());
      for (qsizetype i = 0; !changed && i < widgets.size(); ++i) {
        changed = widgets[i] != m_widgetTree[static_cast<std::size_t>(i)];
      }
      m_structureDirty = false;
      if (changed) { rebuild(); }
    }
    // Actions may replace/delete controls or models. All presenter access
    // uses QPointer; retained AppKit views never keep raw QWidget pointers.
    for (std::size_t i = 0; i < m_bindings.size(); ++i) {
      Binding& binding = m_bindings[i];
      QWidget* widget = binding.widget;
      if (widget == nullptr) {
        NATIVE_SET(binding.view, hidden, YES);
        continue;
      }
      NATIVE_SET(binding.view, hidden, widget != m_source && widget->isHidden());
      if (qobject_cast<QAbstractButton*>(widget) == nullptr) { NATIVE_STRING(binding.view, toolTip, widget->toolTip()); }
      if (binding.control != nil) {
        NATIVE_SET(binding.control, enabled, widget->isEnabled());
      }
      if (qobject_cast<report::ScreenshotView*>(widget) != nullptr) {
        [(MitcadNativeScreenshot*)binding.view refresh];
      } else if (auto* edit = qobject_cast<QLineEdit*>(widget)) {
        NSTextField* field = (NSTextField*)binding.control;
        NATIVE_SET(field, editable, !edit->isReadOnly());
        NATIVE_SET(field, selectable, YES);
        NATIVE_STRING(field, placeholderString, edit->placeholderText());
        if (field.currentEditor == nil) { NATIVE_STRING(field, stringValue, edit->text()); }
      } else if (auto* button = qobject_cast<QAbstractButton*>(widget)) {
        NSButton* native = (NSButton*)binding.control;
        const auto* tool = qobject_cast<QToolButton*>(button);
        const int style = tool == nullptr ? int(Qt::ToolButtonTextBesideIcon) : int(tool->toolButtonStyle());
        if (!binding.presentationInitialized || binding.labelText != button->text() ||
            binding.buttonTip != button->toolTip() || binding.buttonStyle != style ||
            binding.iconKey != button->icon().cacheKey()) {
          updateButtonPresentation(native, button);
          binding.labelText = button->text();
          binding.buttonTip = button->toolTip();
          binding.buttonStyle = style;
          binding.presentationInitialized = true;
        }
        auto state = button->isChecked() ? NSControlStateValueOn : NSControlStateValueOff;
        if (binding.iconKey != button->icon().cacheKey()) {
          native.image = button->icon().isNull() ? nil : image(button->icon().pixmap(18, 18));
          ++g_widgetPropertyWrites;
          binding.iconKey = button->icon().cacheKey();
        }
        if (auto* check = qobject_cast<QCheckBox*>(widget)) {
          NATIVE_SET(native, allowsMixedState, check->isTristate());
          if (check->checkState() == Qt::PartiallyChecked) {
            state = NSControlStateValueMixed;
          }
        }
        NATIVE_SET(native, state, state);
        if (auto* push = qobject_cast<QPushButton*>(widget)) {
          if (push->isDefault()) { NATIVE_STRING(native, keyEquivalent, QStringLiteral("\r")); }
          else if ([native.keyEquivalent isEqualToString:@"\r"]) { NATIVE_STRING(native, keyEquivalent, QString()); }
        }
      } else if (auto* combo = qobject_cast<QComboBox*>(widget)) {
        observeModel(binding, combo->model(), static_cast<int>(i));
        const bool choicesDirty = binding.dirty || binding.items.size() != combo->count();
        QStringList items = binding.items;
        if (choicesDirty) {
          items.clear();
          for (int j = 0; j < combo->count(); ++j) { items.append(combo->itemText(j)); }
        }
        if (combo->isEditable()) {
          NSComboBox* native = (NSComboBox*)binding.control;
          if (items != binding.items) {
            [native removeAllItems];
            for (const QString& item : items) { [native addItemWithObjectValue:ns(item)]; }
            binding.items = items;
          }
          if (native.currentEditor == nil) { NATIVE_STRING(native, stringValue, combo->currentText()); }
        } else {
          NSPopUpButton* native = (NSPopUpButton*)binding.control;
          bool same = native.numberOfItems == combo->count();
          for (int j = 0; same && j < combo->count(); ++j) {
            NSMenuItem* item = [native itemAtIndex:j];
            MitcadNativeComboItem* value = item.representedObject;
            same = sameText(item.title, items[j]) && value != nil &&
                   value->index == combo->model()->index(j, combo->modelColumn(), combo->rootModelIndex());
          }
          if (!same) {
            [native removeAllItems];
            for (int j = 0; j < combo->count(); ++j) {
              NSMenuItem* item = [[NSMenuItem alloc] initWithTitle:ns(items[j]) action:nil keyEquivalent:@""];
              MitcadNativeComboItem* value = [MitcadNativeComboItem new];
              value->index = combo->model()->index(j, combo->modelColumn(), combo->rootModelIndex());
              item.representedObject = value;
              [native.menu addItem:item];
            }
          }
          if (combo->currentIndex() >= 0 && combo->currentIndex() < native.numberOfItems &&
              native.indexOfSelectedItem != combo->currentIndex()) {
            [native selectItemAtIndex:combo->currentIndex()];
            ++g_widgetPropertyWrites;
          } else if (combo->currentIndex() < 0 && native.selectedItem != nil) {
            [native selectItem:nil];
            ++g_widgetPropertyWrites;
          }
        }
        binding.items = items;
        binding.dirty = false;
      } else if (auto* text = qobject_cast<QLabel*>(widget)) {
        const QPixmap pixmap = text->pixmap();
        NATIVE_SET(binding.image, hidden, pixmap.isNull());
        NATIVE_SET(binding.control, hidden, binding.linkText != nil || !pixmap.isNull());
        if (binding.pixmapKey != pixmap.cacheKey()) {
          binding.image.image = image(pixmap);
          ++g_widgetPropertyWrites;
          binding.pixmapKey = pixmap.cacheKey();
        }
        if (!binding.presentationInitialized || binding.labelText != text->text()) {
          binding.labelText = text->text();
          binding.presentationInitialized = true;
          if (binding.linkText != nil) {
            [binding.linkText.textStorage setAttributedString:richLabel(text->text())];
            ++g_widgetPropertyWrites;
          } else { NATIVE_STRING(((NSTextField*)binding.control), stringValue, plain(title(text->text()))); }
        }
        if (binding.linkText != nil) { NATIVE_SET(binding.linkText, hidden, !pixmap.isNull()); }
      } else if (auto* group = qobject_cast<QGroupBox*>(widget)) {
        if (group->isCheckable() && binding.control != nil) {
          if (!binding.presentationInitialized || binding.labelText != group->title()) {
            binding.labelText = group->title();
            binding.presentationInitialized = true;
            NATIVE_STRING(((NSButton*)binding.control), title, title(group->title()));
          }
          NATIVE_SET(((NSButton*)binding.control), state, group->isChecked() ? NSControlStateValueOn : NSControlStateValueOff);
        }
      } else if (auto* spin = qobject_cast<QSpinBox*>(widget)) {
        NATIVE_SET(binding.stepper, minValue, spin->minimum());
        NATIVE_SET(binding.stepper, maxValue, spin->maximum());
        NATIVE_SET(binding.stepper, increment, spin->singleStep());
        NATIVE_SET(binding.stepper, doubleValue, spin->value());
        NATIVE_SET(binding.stepper, enabled, spin->isEnabled() && !spin->isReadOnly());
        NSTextField* field = (NSTextField*)binding.control;
        NATIVE_SET(field, editable, !spin->isReadOnly());
        if (field.currentEditor == nil) { NATIVE_STRING(field, stringValue, spin->text()); }
      } else if (auto* spin = qobject_cast<QDoubleSpinBox*>(widget)) {
        NATIVE_SET(binding.stepper, minValue, spin->minimum());
        NATIVE_SET(binding.stepper, maxValue, spin->maximum());
        NATIVE_SET(binding.stepper, increment, spin->singleStep());
        NATIVE_SET(binding.stepper, doubleValue, spin->value());
        NATIVE_SET(binding.stepper, enabled, spin->isEnabled() && !spin->isReadOnly());
        NSTextField* field = (NSTextField*)binding.control;
        NATIVE_SET(field, editable, !spin->isReadOnly());
        if (field.currentEditor == nil) { NATIVE_STRING(field, stringValue, spin->text()); }
      } else if (auto* slider = qobject_cast<QSlider*>(widget)) {
        NSSlider* native = (NSSlider*)binding.control;
        NATIVE_SET(native, minValue, slider->minimum());
        NATIVE_SET(native, maxValue, slider->maximum());
        NATIVE_SET(native, integerValue, slider->value());
      } else if (auto* progress = qobject_cast<QProgressBar*>(widget)) {
        const bool indeterminate = progress->minimum() == progress->maximum();
        const bool modeChanged = !binding.presentationInitialized || bool(binding.progress.indeterminate) != indeterminate;
        NATIVE_SET(binding.progress, indeterminate, indeterminate);
        NATIVE_SET(binding.progress, minValue, progress->minimum());
        NATIVE_SET(binding.progress, maxValue, progress->maximum());
        NATIVE_SET(binding.progress, doubleValue, std::clamp<double>(progress->value(), progress->minimum(), progress->maximum()));
        if (modeChanged && indeterminate) { [binding.progress startAnimation:nil]; ++g_widgetPropertyWrites; }
        else if (modeChanged) { [binding.progress stopAnimation:nil]; ++g_widgetPropertyWrites; }
        binding.presentationInitialized = true;
      } else if (auto* tabs = qobject_cast<QTabWidget*>(widget)) {
        if ([binding.tabs indexOfTabViewItem:binding.tabs.selectedTabViewItem] != tabs->currentIndex()) {
          [binding.tabs selectTabViewItemAtIndex:tabs->currentIndex()];
          ++g_widgetPropertyWrites;
        }
      } else if (auto* pages = qobject_cast<QStackedWidget*>(widget)) {
        if (binding.tabs != nil && [binding.tabs indexOfTabViewItem:binding.tabs.selectedTabViewItem] != pages->currentIndex()) {
          [binding.tabs selectTabViewItemAtIndex:pages->currentIndex()];
          ++g_widgetPropertyWrites;
        }
      } else if (auto* edit = qobject_cast<QKeySequenceEdit*>(widget)) {
        if (((NSTextField*)binding.control).currentEditor == nil &&
            (!binding.presentationInitialized || binding.keySequence != edit->keySequence())) {
          binding.keySequence = edit->keySequence();
          binding.presentationInitialized = true;
          NATIVE_STRING(((NSTextField*)binding.control), stringValue, binding.keySequence.toString(QKeySequence::NativeText));
        }
      } else if (auto* text = qobject_cast<QPlainTextEdit*>(widget)) {
        NATIVE_SET(binding.text, editable, !text->isReadOnly());
        if (binding.text.window.firstResponder != binding.text &&
            (binding.document != text->document() || binding.textRevision != text->document()->revision())) {
          NATIVE_STRING(binding.text, string, text->toPlainText());
          binding.document = text->document();
          binding.textRevision = text->document()->revision();
        }
      } else if (auto* text = qobject_cast<QTextEdit*>(widget)) {
        NATIVE_SET(binding.text, editable, !text->isReadOnly());
        if (binding.text.window.firstResponder != binding.text &&
            (binding.document != text->document() || binding.textRevision != text->document()->revision())) {
          NATIVE_STRING(binding.text, string, text->toPlainText());
          binding.document = text->document();
          binding.textRevision = text->document()->revision();
        }
      } else if (auto* view = qobject_cast<QAbstractItemView*>(widget)) {
        refreshModel(binding, view, static_cast<int>(i));
      }
    }
    m_refreshing = false;
  }

  void action(id sender) {
    QPointer<AppKitWidgetView> alive(this);
    const NSInteger tag = [sender tag];
    if (tag < 0 || static_cast<std::size_t>(tag) >= m_bindings.size()) { return; }
    const Binding& binding = m_bindings[static_cast<std::size_t>(tag)];
    if (sender != binding.control && sender != binding.stepper) { return; }
    const QPointer<QWidget> pointer = m_bindings[static_cast<std::size_t>(tag)].widget;
    QWidget* widget = pointer;
    if (widget == nullptr || !widget->isEnabled()) { return; }
    bool rejecting = false;
    if (auto* button = qobject_cast<QAbstractButton*>(widget)) {
      if (auto* box = qobject_cast<QDialogButtonBox*>(button->parentWidget())) {
        rejecting = box->buttonRole(button) == QDialogButtonBox::RejectRole;
      }
    }
    NSWindow* window = [sender window];
    // Accepting actions commit pending name/texture/value edits. Cancel
    // remains usable when a required field contains incomplete input.
    if (!rejecting && window != nil && ![window makeFirstResponder:nil]) { return; }
    if (alive == nullptr || pointer == nullptr) { return; }
    if (auto* button = qobject_cast<QAbstractButton*>(widget)) {
      QMenu* menu = nullptr;
      if (auto* tool = qobject_cast<QToolButton*>(widget)) { menu = tool->menu(); }
      if (auto* push = qobject_cast<QPushButton*>(widget)) { menu = push->menu(); }
      if (menu != nullptr) {
        execNativeMenu(menu, widget->mapToGlobal(QPoint(0, widget->height())));
      } else { button->click(); }
    } else if (auto* combo = qobject_cast<QComboBox*>(widget)) {
      int index = -1;
      if (combo->isEditable()) { index = static_cast<int>([(NSComboBox*)sender indexOfSelectedItem]); }
      else {
        MitcadNativeComboItem* value = ((NSPopUpButton*)sender).selectedItem.representedObject;
        if (value != nil && value->index.isValid() && value->index.model() == combo->model() &&
            value->index.parent() == combo->rootModelIndex()) { index = value->index.row(); }
      }
      combo->setCurrentIndex(index);
      if (alive == nullptr || pointer == nullptr) { return; }
      if (combo->isEditable()) { combo->setEditText(qt([(NSComboBox*)sender stringValue])); }
      if (alive == nullptr || pointer == nullptr) { return; }
      QMetaObject::invokeMethod(combo, "activated", Qt::DirectConnection, Q_ARG(int, index));
    } else if (auto* spin = qobject_cast<QSpinBox*>(widget)) {
      spin->setValue(static_cast<int>([sender doubleValue]));
      if (alive == nullptr || pointer == nullptr) { return; }
      QMetaObject::invokeMethod(spin, "editingFinished", Qt::DirectConnection);
    } else if (auto* spin = qobject_cast<QDoubleSpinBox*>(widget)) {
      spin->setValue([sender doubleValue]);
      if (alive == nullptr || pointer == nullptr) { return; }
      QMetaObject::invokeMethod(spin, "editingFinished", Qt::DirectConnection);
    } else if (auto* slider = qobject_cast<QSlider*>(widget)) {
      slider->setValue(static_cast<int>([sender integerValue]));
      if (alive == nullptr || pointer == nullptr) { return; }
      QMetaObject::invokeMethod(slider, "sliderReleased", Qt::DirectConnection);
    } else if (auto* group = qobject_cast<QGroupBox*>(widget)) {
      group->setChecked([(NSButton*)sender state] == NSControlStateValueOn);
      if (alive == nullptr || pointer == nullptr) { return; }
      QMetaObject::invokeMethod(group, "clicked", Qt::DirectConnection, Q_ARG(bool, group->isChecked()));
    }
    if (alive != nullptr && pointer != nullptr) { refresh(); }
  }

  bool textChanged(NSControl* sender, bool finished) {
    QPointer<AppKitWidgetView> alive(this);
    const NSInteger tag = sender.tag;
    if (tag < 0 || static_cast<std::size_t>(tag) >= m_bindings.size()) { return true; }
    const QPointer<QWidget> pointer = m_bindings[static_cast<std::size_t>(tag)].widget;
    QWidget* widget = pointer;
    const QString value = qt([(NSTextField*)sender stringValue]);
    if (auto* edit = qobject_cast<QLineEdit*>(widget)) {
      if (edit->isReadOnly()) { return true; }
      QString proposed = value;
      int cursor = proposed.size();
      if (const QValidator* validator = edit->validator()) {
        if (validator->validate(proposed, cursor) == QValidator::Invalid) {
          [(NSTextField*)sender setStringValue:ns(edit->text())];
          return false;
        }
      }
      if (!finished) {
        edit->setText(value);
        if (alive == nullptr || pointer == nullptr) { return true; }
        edit->setModified(true);
        QMetaObject::invokeMethod(edit, "textEdited", Qt::DirectConnection, Q_ARG(QString, value));
      } else if (edit->hasAcceptableInput()) {
        QMetaObject::invokeMethod(edit, "editingFinished", Qt::DirectConnection);
      } else { return false; }
    } else if (auto* spin = qobject_cast<QAbstractSpinBox*>(widget)) {
      // Interpret through the presenter's virtual validator/valueFromText
      // path, including DecimalSpinBox's comma support and fixups.
      if (auto* line = spin->findChild<QLineEdit*>()) {
        line->setText(value);
        if (alive == nullptr || pointer == nullptr) { return true; }
        if (spin->keyboardTracking() || finished) { spin->interpretText(); }
        if (alive == nullptr || pointer == nullptr) { return true; }
        if (finished) { QMetaObject::invokeMethod(spin, "editingFinished", Qt::DirectConnection); }
      }
    } else if (auto* combo = qobject_cast<QComboBox*>(widget)) {
      combo->setEditText(value);
    }
    return true;
  }

  bool textCanEnd(NSControl* sender) {
    const NSInteger tag = sender.tag;
    if (tag < 0 || static_cast<std::size_t>(tag) >= m_bindings.size()) { return true; }
    if (auto* edit = qobject_cast<QLineEdit*>(m_bindings[static_cast<std::size_t>(tag)].widget)) {
      return edit->hasAcceptableInput();
    }
    return true;
  }

  void textViewChanged(NSTextView* sender) {
    const int id = qt(sender.identifier).toInt();
    if (id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
    QWidget* widget = m_bindings[static_cast<std::size_t>(id)].widget;
    if (auto* edit = qobject_cast<QPlainTextEdit*>(widget)) { edit->setPlainText(qt(sender.string)); }
    else if (auto* edit = qobject_cast<QTextEdit*>(widget)) { edit->setPlainText(qt(sender.string)); }
  }

  bool linkClicked(NSTextView* sender, id link) {
    const int id = qt(sender.identifier).toInt();
    if (id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return false; }
    auto* caption = qobject_cast<QLabel*>(m_bindings[static_cast<std::size_t>(id)].widget);
    if (caption == nullptr) { return false; }
    NSString* destination = [link isKindOfClass:NSURL.class] ? [(NSURL*)link absoluteString] : (NSString*)link;
    if (caption->openExternalLinks()) { [NSWorkspace.sharedWorkspace openURL:[NSURL URLWithString:destination]]; }
    else { QMetaObject::invokeMethod(caption, "linkActivated", Qt::DirectConnection, Q_ARG(QString, qt(destination))); }
    return true;
  }

  void tabChanged(NSTabView* sender, NSTabViewItem* item) {
    if (m_refreshing) { return; }
    const int id = qt(sender.identifier).toInt();
    if (id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
    QWidget* widget = m_bindings[static_cast<std::size_t>(id)].widget;
    const int index = qt(item.identifier).toInt();
    if (auto* tabs = qobject_cast<QTabWidget*>(widget)) { tabs->setCurrentIndex(index); }
    else if (auto* pages = qobject_cast<QStackedWidget*>(widget)) {
      if (auto* settings = qobject_cast<SettingsWindow*>(pages->parentWidget())) { settings->showPane(index); }
      else { pages->setCurrentIndex(index); }
    }
  }

  int modelBinding(NSTableView* table) const { return qt(table.identifier).toInt(); }
  NSMutableArray<MitcadNativeModelNode*>* children(int id, MitcadNativeModelNode* node) {
    if (id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return nil; }
    return node == nil ? m_bindings[static_cast<std::size_t>(id)].nodes : node->children;
  }
  MitcadNativeModelNode* nodeAt(NSTableView* table, NSInteger row) {
    if (row < 0) { return nil; }
    if ([table isKindOfClass:NSOutlineView.class]) { return [(NSOutlineView*)table itemAtRow:row]; }
    auto* nodes = children(modelBinding(table), nil);
    return row < static_cast<NSInteger>(nodes.count) ? nodes[static_cast<NSUInteger>(row)] : nil;
  }
  NSView* modelCell(NSTableView* table, NSTableColumn* column, MitcadNativeModelNode* node) {
    const int id = modelBinding(table);
    if (node == nil || id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return nil; }
    auto* view = qobject_cast<QAbstractItemView*>(m_bindings[static_cast<std::size_t>(id)].widget);
    if (view == nullptr || !node->index.isValid()) { return nil; }
    const int col = qt(column.identifier).toInt();
    const QModelIndex index = QModelIndex(node->index).siblingAtColumn(col);
    if (QWidget* widget = view->indexWidget(index)) {
      for (auto& embedded : m_cellViews) {
        if (embedded.first == widget) {
          embedded.second->refresh();
          return (__bridge NSView*)embedded.second->nativeView();
        }
      }
      auto embedded = std::make_unique<AppKitWidgetView>(widget);
      NSView* native = (__bridge NSView*)embedded->nativeView();
      embedded->refresh();
      m_cellViews.emplace_back(QPointer<QWidget>(widget), std::move(embedded));
      return native;
    }
    MitcadNativeModelCell* cell = [MitcadNativeModelCell new];
    cell->owner = this;
    cell->binding = id;
    cell->index = index;
    [m_cells addObject:cell];
    if (index.flags().testFlag(Qt::ItemIsUserCheckable) && index.data(Qt::CheckStateRole).isValid()) {
      NSButton* check;
      if (view->property("nativeModelHoverEvents").toBool()) {
        MitcadNativeModelHoverButton* hover = [MitcadNativeModelHoverButton new];
        hover->presenter = view;
        hover->modelIndex = index;
        [hover setButtonType:NSButtonTypeSwitch];
        hover.title = ns(index.data().toString());
        hover.target = cell;
        hover.action = @selector(checked:);
        check = hover;
      } else {
        check = [NSButton checkboxWithTitle:ns(index.data().toString()) target:cell action:@selector(checked:)];
      }
      const int state = index.data(Qt::CheckStateRole).toInt();
      check.allowsMixedState = state == Qt::PartiallyChecked;
      check.state = state == Qt::PartiallyChecked ? NSControlStateValueMixed
                    : state == Qt::Checked ? NSControlStateValueOn : NSControlStateValueOff;
      check.enabled = view->isEnabled() && index.flags().testFlag(Qt::ItemIsEnabled);
      check.toolTip = ns(plain(index.data(Qt::ToolTipRole).toString()));
      check.accessibilityIdentifier = ns(index.data(NativeControlIdentifierRole).toString());
      check.translatesAutoresizingMaskIntoConstraints = NO;
      [cell addSubview:check];
      [check.leadingAnchor constraintEqualToAnchor:cell.leadingAnchor constant:3].active = YES;
      [check.trailingAnchor constraintLessThanOrEqualToAnchor:cell.trailingAnchor constant:-4].active = YES;
      [check.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor].active = YES;
    } else {
      cell.accessibilityIdentifier = ns(index.data(NativeControlIdentifierRole).toString());
      NSTextField* field = label(index.data().toString());
      const QVariant permission = index.data(NativeEditableRole);
      field.editable = permission.isValid() ? permission.toBool()
                                           : index.flags().testFlag(Qt::ItemIsEditable) && view->editTriggers() != QAbstractItemView::NoEditTriggers;
      field.delegate = cell;
      field.enabled = index.flags().testFlag(Qt::ItemIsEnabled);
      field.toolTip = ns(plain(index.data(Qt::ToolTipRole).toString()));
      cell.textField = field;
      [cell addSubview:field];
      CGFloat inset = 4;
      const QVariant decoration = index.data(Qt::DecorationRole);
      QPixmap icon;
      if (decoration.canConvert<QIcon>()) { icon = decoration.value<QIcon>().pixmap(std::max(18, std::min(80, view->iconSize().height())), std::max(18, std::min(80, view->iconSize().height()))); }
      else if (decoration.canConvert<QPixmap>()) { icon = decoration.value<QPixmap>(); }
      if (!icon.isNull()) {
        NSImageView* preview = [NSImageView new];
        preview.image = image(icon);
        preview.imageScaling = NSImageScaleProportionallyUpOrDown;
        preview.translatesAutoresizingMaskIntoConstraints = NO;
        cell.imageView = preview;
        [cell addSubview:preview];
        const CGFloat side = std::max<CGFloat>(18, std::min<CGFloat>(80, table.rowHeight - 4));
        [preview.leadingAnchor constraintEqualToAnchor:cell.leadingAnchor constant:3].active = YES;
        [preview.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor].active = YES;
        [preview.widthAnchor constraintEqualToConstant:side].active = YES;
        [preview.heightAnchor constraintEqualToConstant:side].active = YES;
        inset = side + 8;
      }
      [field.leadingAnchor constraintEqualToAnchor:cell.leadingAnchor constant:inset].active = YES;
      [field.trailingAnchor constraintEqualToAnchor:cell.trailingAnchor constant:-4].active = YES;
      [field.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor].active = YES;
    }
    return cell;
  }
  void modelEdit(int id, const QPersistentModelIndex& index, const QVariant& value, int role) {
    if (id < 0 || static_cast<std::size_t>(id) >= m_bindings.size() || !index.isValid()) { return; }
    auto* view = qobject_cast<QAbstractItemView*>(m_bindings[static_cast<std::size_t>(id)].widget);
    if (view != nullptr && view->isEnabled() && index.flags().testFlag(Qt::ItemIsEnabled) && view->model() == index.model()) {
      view->model()->setData(index, value, role);
    }
  }
  void modelSelection(NSTableView* table) {
    QPointer<AppKitWidgetView> alive(this);
    const int id = modelBinding(table);
    if (m_refreshing || id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
    if (table.window != nil && ![table.window makeFirstResponder:nil]) { return; }
    if (alive == nullptr) { return; }
    auto* view = qobject_cast<QAbstractItemView*>(m_bindings[static_cast<std::size_t>(id)].widget);
    if (view == nullptr || view->selectionModel() == nullptr) { return; }
    QItemSelection selection;
    const NSInteger currentRow = table.clickedRow >= 0 && [table.selectedRowIndexes containsIndex:static_cast<NSUInteger>(table.clickedRow)]
                                     ? table.clickedRow : table.selectedRow;
    const QModelIndex clicked = modelIndex(table, currentRow, std::max<NSInteger>(0, table.clickedColumn));
    for (NSUInteger row = table.selectedRowIndexes.firstIndex; row != NSNotFound;
         row = [table.selectedRowIndexes indexGreaterThanIndex:row]) {
      MitcadNativeModelNode* node = nodeAt(table, static_cast<NSInteger>(row));
      if (node != nil && node->index.isValid()) { selection.select(node->index, node->index); }
    }
    QPointer<QAbstractItemView> presenter(view);
    view->selectionModel()->select(selection, QItemSelectionModel::ClearAndSelect | QItemSelectionModel::Rows);
    if (alive == nullptr || presenter == nullptr) { return; }
    const QModelIndex current = clicked.isValid() ? clicked : modelIndex(table, table.selectedRow, 0);
    if (current.isValid()) {
      view->selectionModel()->setCurrentIndex(current, QItemSelectionModel::NoUpdate);
      if (alive == nullptr || presenter == nullptr) { return; }
      QMetaObject::invokeMethod(view, "clicked", Qt::DirectConnection, Q_ARG(QModelIndex, current));
    }
  }
  QModelIndex modelIndex(NSTableView* table, NSInteger row, NSInteger col) {
    MitcadNativeModelNode* node = nodeAt(table, row);
    return node == nil ? QModelIndex() : QModelIndex(node->index).siblingAtColumn(static_cast<int>(col));
  }
  void modelActivated(NSTableView* table) {
    QPointer<AppKitWidgetView> alive(this);
    if (table.window != nil && ![table.window makeFirstResponder:nil]) { return; }
    if (alive == nullptr) { return; }
    const int id = modelBinding(table);
    if (id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
    auto* view = qobject_cast<QAbstractItemView*>(m_bindings[static_cast<std::size_t>(id)].widget);
    const QModelIndex index = modelIndex(table, table.clickedRow >= 0 ? table.clickedRow : table.selectedRow,
                                        std::max<NSInteger>(0, table.clickedColumn));
    if (view != nullptr && index.isValid()) {
      if (index.data(NativeEditableRole).isValid() && index.data(NativeEditableRole).toBool()) {
        const NSInteger row = table.clickedRow >= 0 ? table.clickedRow : table.selectedRow;
        const NSInteger column = std::max<NSInteger>(0, table.clickedColumn);
        NSView* cell = [table viewAtColumn:column row:row makeIfNecessary:YES];
        if ([cell isKindOfClass:NSTableCellView.class]) {
          NSTextField* field = ((NSTableCellView*)cell).textField;
          if (field.editable) { [table.window makeFirstResponder:field]; [field selectText:nil]; return; }
        }
      }
      QPointer<QAbstractItemView> presenter(view);
      QMetaObject::invokeMethod(view, "doubleClicked", Qt::DirectConnection, Q_ARG(QModelIndex, index));
      if (alive == nullptr || presenter == nullptr) { return; }
      QMetaObject::invokeMethod(view, "activated", Qt::DirectConnection, Q_ARG(QModelIndex, index));
    }
  }
  void modelContext(NSTableView* table, const NSPoint& point) {
    QPointer<AppKitWidgetView> alive(this);
    const int id = modelBinding(table);
    if (id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
    auto* view = qobject_cast<QAbstractItemView*>(m_bindings[static_cast<std::size_t>(id)].widget);
    const NSInteger row = [table rowAtPoint:point];
    const QModelIndex index = modelIndex(table, row, std::max<NSInteger>(0, [table columnAtPoint:point]));
    if (view != nullptr && index.isValid()) {
      QPointer<QAbstractItemView> presenter(view);
      view->selectionModel()->setCurrentIndex(index, QItemSelectionModel::ClearAndSelect | QItemSelectionModel::Rows);
      if (alive == nullptr || presenter == nullptr) { return; }
      // Existing custom-context handlers resolve indexAt against the
      // presenter view's layout, which remains the authoritative model.
      view->doItemsLayout();
      const QPoint local = view->visualRect(index).center();
      QMetaObject::invokeMethod(view, "customContextMenuRequested", Qt::DirectConnection, Q_ARG(QPoint, local));
    }
  }
  void modelExpanded(NSOutlineView* table, MitcadNativeModelNode* node, bool expanded) {
    const int id = modelBinding(table);
    if (m_refreshing || node == nil || id < 0 || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
    if (auto* view = qobject_cast<QTreeView*>(m_bindings[static_cast<std::size_t>(id)].widget)) {
      view->setExpanded(node->index, expanded);
    }
  }

protected:
  bool eventFilter(QObject* watched, QEvent* event) override {
    switch (event->type()) {
    case QEvent::ChildAdded:
    case QEvent::ChildRemoved:
    case QEvent::ChildPolished:
      m_structureDirty = true;
      scheduleRefresh();
      break;
    case QEvent::Show:
    case QEvent::Hide:
    case QEvent::EnabledChange:
    case QEvent::ToolTipChange:
    case QEvent::DynamicPropertyChange:
    case QEvent::LayoutRequest:
    case QEvent::StyleChange:
    case QEvent::FontChange:
    case QEvent::ReadOnlyChange:
    case QEvent::UpdateRequest:
      // A view's row visibility and index widgets have no notify signal.
      // Its update/layout events provide an early check before the fallback.
      if (qobject_cast<QAbstractItemView*>(watched) != nullptr) { checkModelPresentation(); }
      scheduleRefresh();
      break;
    default: break;
    }
    return false;
  }

private:
  struct Binding {
    QPointer<QWidget> widget;
    NSView* view = nil;
    NSControl* control = nil;
    NSStepper* stepper = nil;
    NSImageView* image = nil;
    NSProgressIndicator* progress = nil;
    NSTextView* text = nil;
    NSTextView* linkText = nil;
    NSTabView* tabs = nil;
    NSTableView* table = nil;
    NSMutableArray<MitcadNativeModelNode*>* nodes = nil;
    QPointer<QAbstractItemModel> model;
    QPointer<QItemSelectionModel> selection;
    QPointer<QTextDocument> document;
    QPersistentModelIndex rootIndex;
    std::vector<QMetaObject::Connection> connections;
    std::vector<QMetaObject::Connection> modelConnections;
    std::vector<QMetaObject::Connection> selectionConnections;
    QStringList items;
    QString labelText;
    QString buttonTip;
    QKeySequence keySequence;
    int buttonStyle = -1;
    int textRevision = -1;
    quint64 presentation = 0;
    qint64 pixmapKey = 0;
    qint64 iconKey = 0;
    bool presentationInitialized = false;
    bool dirty = true;
    bool selectionDirty = true;
  };
  void scheduleRefresh() {
    if (m_source != nullptr && !m_refreshTimer.isActive()) { m_refreshTimer.start(0); }
  }
  void observeModel(Binding& binding, QAbstractItemModel* model, int id) {
    if (binding.model == model) { return; }
    for (const auto& connection : binding.modelConnections) { disconnect(connection); }
    binding.modelConnections.clear();
    binding.model = model;
    binding.dirty = true;
    binding.selectionDirty = true;
    if (model == nullptr) { return; }
    const auto generation = m_generation;
    auto dirty = [this, id, generation, observed = QPointer<QAbstractItemModel>(model)] {
      if (generation != m_generation || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
      if (m_bindings[static_cast<std::size_t>(id)].model != observed) { return; }
      m_bindings[static_cast<std::size_t>(id)].dirty = true;
      m_bindings[static_cast<std::size_t>(id)].selectionDirty = true;
      scheduleRefresh();
    };
    auto& c = binding.modelConnections;
    c.push_back(connect(model, &QAbstractItemModel::dataChanged, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::headerDataChanged, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::modelReset, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::rowsInserted, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::rowsRemoved, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::rowsMoved, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::columnsInserted, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::columnsRemoved, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::columnsMoved, this, dirty));
    c.push_back(connect(model, &QAbstractItemModel::layoutChanged, this, dirty));
    c.push_back(connect(model, &QObject::destroyed, this, dirty));
  }
  void observeSelection(Binding& binding, QAbstractItemView* view, int id) {
    if (binding.selection == view->selectionModel()) { return; }
    for (const auto& connection : binding.selectionConnections) { disconnect(connection); }
    binding.selectionConnections.clear();
    binding.selection = view->selectionModel();
    binding.selectionDirty = true;
    if (!binding.selection) { return; }
    const auto generation = m_generation;
    auto changed = [this, id, generation, observed = binding.selection] {
      if (generation != m_generation || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
      if (m_bindings[static_cast<std::size_t>(id)].selection != observed) { return; }
      m_bindings[static_cast<std::size_t>(id)].selectionDirty = true;
      scheduleRefresh();
    };
    binding.selectionConnections.push_back(connect(binding.selection, &QItemSelectionModel::selectionChanged, this, changed));
    binding.selectionConnections.push_back(connect(binding.selection, &QItemSelectionModel::currentChanged, this, changed));
  }
  quint64 modelPresentation(QAbstractItemView* view) const {
    // No model data, strings, node allocation or AppKit messages. The
    // fallback only checks otherwise silent row hiding/index-widget swaps.
    quint64 value = 1469598103934665603ULL;
    const auto mix = [&value](quint64 part) { value = (value ^ part) * 1099511628211ULL; };
    mix(view->isEnabled());
    const auto describe = [&](auto&& self, const QModelIndex& parent) -> void {
      const int count = view->model()->rowCount(parent);
      mix(static_cast<quint64>(count));
      for (int row = 0; row < count; ++row) {
        const QModelIndex index = view->model()->index(row, 0, parent);
        if (auto* tree = qobject_cast<QTreeView*>(view)) {
          mix(tree->isRowHidden(row, parent));
          mix(tree->isExpanded(index));
        }
        if (auto* table = qobject_cast<QTableView*>(view)) { mix(table->isRowHidden(row)); }
        if (auto* list = qobject_cast<QListView*>(view)) { mix(list->isRowHidden(row)); }
        for (int col = 0; col < view->model()->columnCount(parent); ++col) {
          mix(reinterpret_cast<quintptr>(view->indexWidget(index.siblingAtColumn(col))));
        }
        self(self, index);
      }
    };
    if (view->model() != nullptr) { describe(describe, view->rootIndex()); }
    return value;
  }
  void checkModelPresentation() {
    for (auto& binding : m_bindings) {
      auto* view = qobject_cast<QAbstractItemView*>(binding.widget.data());
      if (view == nullptr) { continue; }
      const quint64 value = modelPresentation(view);
      if (binding.presentation != value) { binding.dirty = true; binding.presentation = value; }
    }
  }
  int bind(QWidget* widget, NSView* view, NSControl* control = nil) {
    Binding binding;
    binding.widget = widget;
    binding.view = view;
    binding.control = control;
    const int id = static_cast<int>(m_bindings.size());
    m_bindings.push_back(std::move(binding));
    auto changed = [this] { scheduleRefresh(); };
    auto& c = m_bindings.back().connections;
    if (auto* button = qobject_cast<QAbstractButton*>(widget)) { c.push_back(connect(button, &QAbstractButton::toggled, this, changed)); }
    if (auto* edit = qobject_cast<QLineEdit*>(widget)) { c.push_back(connect(edit, &QLineEdit::textChanged, this, changed)); }
    if (auto* combo = qobject_cast<QComboBox*>(widget)) {
      c.push_back(connect(combo, &QComboBox::currentIndexChanged, this, changed));
      c.push_back(connect(combo, &QComboBox::editTextChanged, this, changed));
    }
    if (auto* group = qobject_cast<QGroupBox*>(widget)) { c.push_back(connect(group, &QGroupBox::toggled, this, changed)); }
    if (auto* spin = qobject_cast<QSpinBox*>(widget)) { c.push_back(connect(spin, &QSpinBox::valueChanged, this, changed)); }
    if (auto* spin = qobject_cast<QDoubleSpinBox*>(widget)) { c.push_back(connect(spin, &QDoubleSpinBox::valueChanged, this, changed)); }
    if (auto* slider = qobject_cast<QSlider*>(widget)) { c.push_back(connect(slider, &QSlider::valueChanged, this, changed)); }
    if (auto* progress = qobject_cast<QProgressBar*>(widget)) { c.push_back(connect(progress, &QProgressBar::valueChanged, this, changed)); }
    if (auto* tabs = qobject_cast<QTabWidget*>(widget)) { c.push_back(connect(tabs, &QTabWidget::currentChanged, this, changed)); }
    if (auto* pages = qobject_cast<QStackedWidget*>(widget)) { c.push_back(connect(pages, &QStackedWidget::currentChanged, this, changed)); }
    if (auto* edit = qobject_cast<QKeySequenceEdit*>(widget)) { c.push_back(connect(edit, &QKeySequenceEdit::keySequenceChanged, this, changed)); }
    if (auto* edit = qobject_cast<QPlainTextEdit*>(widget)) { c.push_back(connect(edit, &QPlainTextEdit::textChanged, this, changed)); }
    if (auto* edit = qobject_cast<QTextEdit*>(widget)) { c.push_back(connect(edit, &QTextEdit::textChanged, this, changed)); }
    if (auto* tree = qobject_cast<QTreeView*>(widget)) {
      const auto generation = m_generation;
      auto expanded = [this, id, generation] {
        if (generation != m_generation || static_cast<std::size_t>(id) >= m_bindings.size()) { return; }
        m_bindings[static_cast<std::size_t>(id)].dirty = true;
        scheduleRefresh();
      };
      c.push_back(connect(tree, &QTreeView::expanded, this, expanded));
      c.push_back(connect(tree, &QTreeView::collapsed, this, expanded));
    }
    if (control == nil || control == view) { view.accessibilityIdentifier = ns(widget->objectName()); }
    else { control.accessibilityIdentifier = ns(widget->objectName()); }
    view.toolTip = ns(widget->toolTip());
    if (control != nil) { control.tag = id; }
    return id;
  }
  void target(NSControl* control) {
    control.target = m_target;
    control.action = @selector(perform:);
  }
  NSView* renderLayout(QLayout* layout) {
    if (layout == nullptr) { return nil; }
    if (auto* grid = qobject_cast<QGridLayout*>(layout)) {
      NSStackView* rows = stack();
      for (int row = 0; row < grid->rowCount(); ++row) {
        NSStackView* line = stack(true);
        if (m_source->objectName() == QStringLiteral("sketchPalette") && grid->columnCount() == 6) {
          line.distribution = NSStackViewDistributionFillEqually;
        }
        for (int col = 0; col < grid->columnCount(); ++col) {
          QLayoutItem* item = grid->itemAtPosition(row, col);
          if (item == nullptr) { continue; }
          int itemIndex = -1;
          for (int i = 0; i < grid->count(); ++i) { if (grid->itemAt(i) == item) { itemIndex = i; break; } }
          int itemRow = row, itemCol = col, rowSpan, colSpan;
          if (itemIndex >= 0) { grid->getItemPosition(itemIndex, &itemRow, &itemCol, &rowSpan, &colSpan); }
          if (itemRow == row && itemCol == col) {
            add(line, renderItem(item));
          }
        }
        add(rows, line);
      }
      return rows;
    }
    if (auto* form = qobject_cast<QFormLayout*>(layout)) {
      NSStackView* rows = stack();
      for (int row = 0; row < form->rowCount(); ++row) {
        QLayoutItem* field = form->itemAt(row, QFormLayout::FieldRole);
        QLayoutItem* caption = form->itemAt(row, QFormLayout::LabelRole);
        QLayoutItem* spanning = form->itemAt(row, QFormLayout::SpanningRole);
        if (spanning != nullptr) { add(rows, renderItem(spanning)); }
        else {
          NSStackView* line = stack(true);
          if (caption != nullptr) {
            NSView* nativeCaption = renderItem(caption);
            if (auto* text = qobject_cast<QLabel*>(caption->widget()); text && !text->wordWrap()) {
              [nativeCaption.widthAnchor constraintGreaterThanOrEqualToConstant:text->sizeHint().width()].active = YES;
            }
            add(line, nativeCaption);
          }
          if (field != nullptr) { add(line, renderItem(field)); }
          add(rows, line);
        }
      }
      return rows;
    }
    auto* box = qobject_cast<QBoxLayout*>(layout);
    const bool horizontal = box != nullptr && (box->direction() == QBoxLayout::LeftToRight || box->direction() == QBoxLayout::RightToLeft);
    NSStackView* result = stack(horizontal);
    const QMargins margins = layout->contentsMargins();
    result.edgeInsets = NSEdgeInsetsMake(margins.top(), margins.left(), margins.bottom(), margins.right());
    result.spacing = std::max(6, layout->spacing());
    for (int i = 0; i < layout->count(); ++i) { add(result, renderItem(layout->itemAt(i))); }
    return result;
  }
  NSView* renderItem(QLayoutItem* item) {
    if (item == nullptr) { return nil; }
    if (item->widget() != nullptr) { return renderWidget(item->widget()); }
    if (item->layout() != nullptr) { return renderLayout(item->layout()); }
    NSView* spring = [NSView new];
    [spring setContentHuggingPriority:NSLayoutPriorityDefaultLow forOrientation:NSLayoutConstraintOrientationHorizontal];
    return spring;
  }
  NSView* renderWidget(QWidget* widget) {
    if (widget == nullptr) { return nil; }
    if (auto* settings = qobject_cast<SettingsWindow*>(widget)) {
      return renderWidget(settings->findChild<QStackedWidget*>(QString(), Qt::FindDirectChildrenOnly));
    }
    if (auto* screenshot = qobject_cast<report::ScreenshotView*>(widget)) {
      MitcadNativeScreenshot* view = [MitcadNativeScreenshot new];
      view->presenter = screenshot;
      [view.heightAnchor constraintGreaterThanOrEqualToConstant:240].active = YES;
      view.accessibilityLabel = ns(tr("Screenshot. Drag to choose the area to include."));
      const int id = bind(widget, view);
      m_bindings[static_cast<std::size_t>(id)].connections.push_back(
        connect(screenshot, &report::ScreenshotView::cropped, this, [view] { [view refresh]; }));
      [view refresh];
      return view;
    }
    if (auto* text = qobject_cast<QLabel*>(widget)) {
      NSStackView* contents = stack();
      NSTextField* caption = label(title(text->text()));
      if (text->text().contains(QStringLiteral("<b>"))) { caption.font = [NSFont systemFontOfSize:NSFont.systemFontSize weight:NSFontWeightSemibold]; }
      NSImageView* preview = [NSImageView new];
      preview.imageScaling = NSImageScaleProportionallyUpOrDown;
      preview.hidden = text->pixmap().isNull();
      caption.hidden = !text->pixmap().isNull();
      add(contents, caption);
      add(contents, preview);
      [preview.heightAnchor constraintGreaterThanOrEqualToConstant:std::max(80, std::min(240, text->minimumHeight()))].active = YES;
      const int id = bind(widget, contents, caption);
      m_bindings[static_cast<std::size_t>(id)].image = preview;
      if (text->text().contains(QStringLiteral("<a"), Qt::CaseInsensitive)) {
        caption.hidden = YES;
        NSTextView* links = [[NSTextView alloc] initWithFrame:NSMakeRect(0, 0, 440, 55)];
        links.editable = NO;
        links.selectable = YES;
        links.drawsBackground = NO;
        links.delegate = m_target;
        links.identifier = ns(QString::number(id));
        links.autoresizingMask = NSViewWidthSizable;
        links.textContainer.widthTracksTextView = YES;
        [links.heightAnchor constraintGreaterThanOrEqualToConstant:55].active = YES;
        add(contents, links);
        m_bindings[static_cast<std::size_t>(id)].linkText = links;
      }
      return contents;
    }
    if (auto* edit = qobject_cast<QLineEdit*>(widget)) {
      NSTextField* field = edit->echoMode() != QLineEdit::Normal
                               ? [[NSSecureTextField alloc] initWithFrame:NSZeroRect]
                               : [[NSTextField alloc] initWithFrame:NSZeroRect];
      field.stringValue = ns(edit->text());
      field.delegate = m_target;
      field.editable = !edit->isReadOnly();
      field.placeholderString = ns(edit->placeholderText());
      bind(widget, field, field);
      return field;
    }
    if (auto* edit = qobject_cast<QKeySequenceEdit*>(widget)) {
      MitcadNativeShortcutField* field = [[MitcadNativeShortcutField alloc] initWithFrame:NSZeroRect];
      field->edit = edit;
      field.editable = NO;
      field.selectable = NO;
      field.stringValue = ns(edit->keySequence().toString(QKeySequence::NativeText));
      field.placeholderString = ns(tr("Press a shortcut"));
      NSButton* clear = [NSButton buttonWithTitle:ns(tr("Clear")) target:field action:@selector(clearShortcut:)];
      clear.toolTip = ns(tr("Clear shortcut; Delete and Backspace can be recorded as keys"));
      NSStackView* row = [NSStackView stackViewWithViews:@[field, clear]];
      row.orientation = NSUserInterfaceLayoutOrientationHorizontal;
      row.spacing = 6;
      [field setContentHuggingPriority:NSLayoutPriorityDefaultLow forOrientation:NSLayoutConstraintOrientationHorizontal];
      bind(widget, row, field);
      return row;
    }
    if (auto* combo = qobject_cast<QComboBox*>(widget)) {
      NSControl* view;
      if (combo->isEditable()) {
        NSComboBox* field = [[NSComboBox alloc] initWithFrame:NSZeroRect];
        field.delegate = m_target;
        view = field;
      } else { view = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO]; }
      target(view);
      bind(widget, view, view);
      return view;
    }
    if (auto* button = qobject_cast<QAbstractButton*>(widget)) {
      NSButton* view;
      if (button->property("nativeHoverEvents").toBool()) {
        MitcadNativeHoverButton* hover = [[MitcadNativeHoverButton alloc] initWithFrame:NSZeroRect];
        hover->presenter = button;
        hover.title = ns(title(button->text()));
        hover.target = m_target;
        hover.action = @selector(perform:);
        view = hover;
      } else {
        view = [NSButton buttonWithTitle:ns(title(button->text())) target:m_target action:@selector(perform:)];
      }
      view.bezelStyle = NSBezelStyleRounded;
      if (qobject_cast<QCheckBox*>(widget) != nullptr) { [view setButtonType:NSButtonTypeSwitch]; }
      else if (qobject_cast<QRadioButton*>(widget) != nullptr) { [view setButtonType:NSButtonTypeRadio]; }
      else if (button->isCheckable()) { [view setButtonType:NSButtonTypePushOnPushOff]; }
      if (!button->icon().isNull()) {
        view.image = image(button->icon().pixmap(18, 18));
      }
      updateButtonPresentation(view, button);
      if (widget->objectName().startsWith(QStringLiteral("palette_"))) {
        [view.widthAnchor constraintGreaterThanOrEqualToConstant:34].active = YES;
        [view.heightAnchor constraintGreaterThanOrEqualToConstant:30].active = YES;
      }
      if (auto* box = qobject_cast<QDialogButtonBox*>(widget->parentWidget())) {
        const auto role = box->buttonRole(button);
        if (role == QDialogButtonBox::RejectRole) { view.keyEquivalent = @"\033"; }
      }
      bind(widget, view, view);
      return view;
    }
    if (qobject_cast<QSpinBox*>(widget) != nullptr || qobject_cast<QDoubleSpinBox*>(widget) != nullptr) {
      NSStackView* row = stack(true);
      NSTextField* field = [[NSTextField alloc] initWithFrame:NSZeroRect];
      field.delegate = m_target;
      NSStepper* stepper = [[NSStepper alloc] initWithFrame:NSZeroRect];
      add(row, field);
      add(row, stepper);
      const int id = bind(widget, row, field);
      m_bindings[static_cast<std::size_t>(id)].stepper = stepper;
      stepper.tag = id;
      target(stepper);
      [field.widthAnchor constraintGreaterThanOrEqualToConstant:80].active = YES;
      return row;
    }
    if (auto* slider = qobject_cast<QSlider*>(widget)) {
      NSSlider* view = [NSSlider sliderWithValue:slider->value() minValue:slider->minimum() maxValue:slider->maximum()
                                        target:m_target action:@selector(perform:)];
      view.continuous = YES;
      bind(widget, view, view);
      return view;
    }
    if (qobject_cast<QProgressBar*>(widget) != nullptr) {
      NSProgressIndicator* view = [[NSProgressIndicator alloc] initWithFrame:NSZeroRect];
      view.style = NSProgressIndicatorStyleBar;
      const int id = bind(widget, view);
      m_bindings[static_cast<std::size_t>(id)].progress = view;
      return view;
    }
    if (auto* group = qobject_cast<QGroupBox*>(widget)) {
      NSStackView* body = stack();
      if (group->isCheckable()) {
        NSButton* check = [NSButton checkboxWithTitle:ns(title(group->title())) target:m_target action:@selector(perform:)];
        bind(widget, body, check);
        add(body, check);
      } else if (!group->title().isEmpty()) {
        NSTextField* caption = label(title(group->title()));
        caption.font = [NSFont systemFontOfSize:NSFont.systemFontSize weight:NSFontWeightSemibold];
        add(body, caption);
        bind(widget, body);
      }
      add(body, renderLayout(group->layout()));
      return body;
    }
    if (auto* buttons = qobject_cast<QDialogButtonBox*>(widget)) {
      NSStackView* row = stack(true);
      NSView* spring = [NSView new];
      [spring setContentHuggingPriority:NSLayoutPriorityDefaultLow forOrientation:NSLayoutConstraintOrientationHorizontal];
      add(row, spring);
      for (QAbstractButton* button : buttons->buttons()) { add(row, renderWidget(button)); }
      bind(widget, row);
      return row;
    }
    if (auto* view = qobject_cast<QAbstractItemView*>(widget)) { return renderModel(view); }
    if (qobject_cast<QPlainTextEdit*>(widget) != nullptr || qobject_cast<QTextEdit*>(widget) != nullptr) {
      NSTextView* view = [[NSTextView alloc] initWithFrame:NSMakeRect(0, 0, 440, 130)];
      view.font = [NSFont systemFontOfSize:NSFont.systemFontSize];
      view.richText = NO;
      view.delegate = m_target;
      view.verticallyResizable = YES;
      view.horizontallyResizable = NO;
      view.autoresizingMask = NSViewWidthSizable;
      view.textContainer.widthTracksTextView = YES;
      NSScrollView* box = scroll(view, std::max(90, std::min(260, widget->minimumHeight())));
      const int id = bind(widget, box);
      m_bindings[static_cast<std::size_t>(id)].text = view;
      view.identifier = ns(QString::number(id));
      return box;
    }
    if (auto* tabs = qobject_cast<QTabWidget*>(widget)) { return renderTabs(widget, tabs->count(), [tabs](int i) { return tabs->widget(i); }, [tabs](int i) { return tabs->tabText(i); }); }
    if (auto* pages = qobject_cast<QStackedWidget*>(widget)) {
      const bool named = pages->count() > 0 && !pages->widget(0)->property("nativePaneTitle").toString().isEmpty();
      if (named) { return renderTabs(widget, pages->count(), [pages](int i) { return pages->widget(i); }, [pages](int i) { return pages->widget(i)->property("nativePaneTitle").toString(); }); }
      NSStackView* body = stack();
      bind(widget, body);
      for (int i = 0; i < pages->count(); ++i) { add(body, renderWidget(pages->widget(i))); }
      return body;
    }
    if (auto* area = qobject_cast<QScrollArea*>(widget)) {
      NSView* content = renderWidget(area->widget());
      NSScrollView* view = scroll(content, std::max(100, std::min(340, area->minimumHeight())));
      bind(widget, view);
      return view;
    }
    if (auto* split = qobject_cast<QSplitter*>(widget)) {
      NSStackView* view = stack(split->orientation() == Qt::Horizontal);
      bind(widget, view);
      for (int i = 0; i < split->count(); ++i) { add(view, renderWidget(split->widget(i))); }
      return view;
    }
    if (auto* frame = qobject_cast<QFrame*>(widget)) {
      if (frame->frameShape() == QFrame::HLine || frame->frameShape() == QFrame::VLine) {
        NSBox* view = [NSBox new];
        view.boxType = NSBoxSeparator;
        bind(widget, view);
        return view;
      }
    }
    NSView* view = renderLayout(widget->layout());
    if (view == nil) {
      NSStackView* contents = stack();
      const auto children = widget->findChildren<QWidget*>(QString(), Qt::FindDirectChildrenOnly);
      for (QWidget* child : children) { add(contents, renderWidget(child)); }
      view = contents;
    }
    bind(widget, view);
    return view;
  }
  NSAttributedString* richLabel(const QString& html) {
    QTextDocument document;
    document.setHtml(html);
    NSMutableAttributedString* result = [[NSMutableAttributedString alloc] initWithString:ns(document.toPlainText())
      attributes:@{NSFontAttributeName: [NSFont systemFontOfSize:NSFont.systemFontSize], NSForegroundColorAttributeName:NSColor.labelColor}];
    for (QTextBlock block = document.begin(); block.isValid(); block = block.next()) {
      for (auto it = block.begin(); !it.atEnd(); ++it) {
        const QTextFragment fragment = it.fragment();
        if (!fragment.isValid() || !fragment.charFormat().isAnchor()) { continue; }
        const NSRange range = NSMakeRange(static_cast<NSUInteger>(fragment.position()), static_cast<NSUInteger>(fragment.length()));
        if (NSMaxRange(range) <= result.length) {
          [result addAttribute:NSLinkAttributeName value:ns(fragment.charFormat().anchorHref()) range:range];
        }
      }
    }
    return result;
  }
  NSView* renderTabs(QWidget* widget, int count, std::function<QWidget*(int)> page, std::function<QString(int)> name) {
    NSTabView* tabs = [[NSTabView alloc] initWithFrame:NSMakeRect(0, 0, 500, 340)];
    const int id = bind(widget, tabs);
    m_bindings[static_cast<std::size_t>(id)].tabs = tabs;
    tabs.identifier = ns(QString::number(id));
    int selected = 0;
    if (auto* source = qobject_cast<QTabWidget*>(widget)) { selected = source->currentIndex(); }
    if (auto* source = qobject_cast<QStackedWidget*>(widget)) { selected = source->currentIndex(); }
    for (int i = 0; i < count; ++i) {
      NSTabViewItem* item = [[NSTabViewItem alloc] initWithIdentifier:ns(QString::number(i))];
      item.label = ns(title(name(i)));
      item.view = renderWidget(page(i));
      [tabs addTabViewItem:item];
    }
    if (selected >= 0 && selected < count) { [tabs selectTabViewItemAtIndex:selected]; }
    tabs.delegate = m_target;
    return tabs;
  }
  NSView* renderModel(QAbstractItemView* view) {
    const bool hierarchical = qobject_cast<QTreeView*>(view) != nullptr;
    NSTableView* table = hierarchical ? [[MitcadNativeOutline alloc] initWithFrame:NSMakeRect(0, 0, 520, 180)]
                                     : [[MitcadNativeTable alloc] initWithFrame:NSMakeRect(0, 0, 520, 180)];
    table.rowHeight = std::max(25, std::min(100, view->iconSize().height() + 4));
    table.usesAlternatingRowBackgroundColors = YES;
    table.allowsMultipleSelection = view->selectionMode() == QAbstractItemView::ExtendedSelection || view->selectionMode() == QAbstractItemView::MultiSelection;
    table.delegate = m_target;
    table.dataSource = m_target;
    table.target = m_target;
    table.doubleAction = @selector(doubleClick:);
    table.columnAutoresizingStyle = NSTableViewUniformColumnAutoresizingStyle;
    const int count = view->model() == nullptr ? 1 : std::max(1, view->model()->columnCount(view->rootIndex()));
    for (int i = 0; i < count; ++i) {
      NSTableColumn* column = [[NSTableColumn alloc] initWithIdentifier:ns(QString::number(i))];
      column.title = ns(view->model() == nullptr ? QString() : view->model()->headerData(i, Qt::Horizontal).toString());
      column.width = i == 0 ? 220 : 130;
      column.minWidth = 70;
      [table addTableColumn:column];
      if (hierarchical && i == 0) { ((NSOutlineView*)table).outlineTableColumn = column; }
    }
    auto* tree = qobject_cast<QTreeView*>(view);
    if (qobject_cast<QListView*>(view) != nullptr || (tree != nullptr && tree->isHeaderHidden())) { table.headerView = nil; }
    // The native default must fit within the presenter's required upper bound.
    const int minimumHeight = std::min(view->maximumHeight(), std::max(120, std::min(280, view->minimumHeight())));
    NSScrollView* box = scroll(table, minimumHeight);
    if (view->maximumHeight() < QWIDGETSIZE_MAX) {
      [box.heightAnchor constraintLessThanOrEqualToConstant:view->maximumHeight()].active = YES;
    }
    const int id = bind(view, box);
    m_bindings[static_cast<std::size_t>(id)].table = table;
    table.identifier = ns(QString::number(id));
    return box;
  }
  NSMutableArray<MitcadNativeModelNode*>* modelNodes(QAbstractItemView* view, const QModelIndex& parent) {
    auto* nodes = [NSMutableArray<MitcadNativeModelNode*> new];
    for (int row = 0; row < view->model()->rowCount(parent); ++row) {
      if (auto* tree = qobject_cast<QTreeView*>(view)) { if (tree->isRowHidden(row, parent)) { continue; } }
      if (auto* table = qobject_cast<QTableView*>(view)) { if (table->isRowHidden(row)) { continue; } }
      if (auto* list = qobject_cast<QListView*>(view)) { if (list->isRowHidden(row)) { continue; } }
      MitcadNativeModelNode* node = [MitcadNativeModelNode new];
      node->index = view->model()->index(row, 0, parent);
      node->children = modelNodes(view, node->index);
      [nodes addObject:node];
    }
    return nodes;
  }
  void refreshModel(Binding& binding, QAbstractItemView* view, int id) {
    observeModel(binding, view->model(), id);
    observeSelection(binding, view, id);
    if (binding.rootIndex != view->rootIndex()) {
      binding.rootIndex = view->rootIndex();
      binding.dirty = true;
    }
    if (binding.dirty) {
      binding.selectionDirty = true;
      if (view->model() == nullptr) {
        binding.nodes = nil;
        binding.dirty = false;
        [binding.table reloadData];
        ++g_widgetPropertyWrites;
        return;
      }
      binding.presentation = modelPresentation(view);
      binding.nodes = modelNodes(view, view->rootIndex());
      binding.dirty = false;
      [binding.table reloadData];
      ++g_widgetPropertyWrites;
      if ([binding.table isKindOfClass:NSOutlineView.class]) {
        NSOutlineView* outline = (NSOutlineView*)binding.table;
        auto expand = [&](auto&& self, NSArray<MitcadNativeModelNode*>* nodes) -> void {
          auto* tree = qobject_cast<QTreeView*>(view);
          for (MitcadNativeModelNode* node in nodes) {
            if (tree->isExpanded(node->index)) { [outline expandItem:node]; }
            self(self, node->children);
          }
        };
        expand(expand, binding.nodes);
      }
    }
    if (binding.selectionDirty && view->selectionModel() != nullptr) {
      binding.selectionDirty = false;
      NSMutableIndexSet* selected = [NSMutableIndexSet new];
      for (NSInteger row = 0; row < binding.table.numberOfRows; ++row) {
        MitcadNativeModelNode* node = nodeAt(binding.table, row);
        if (node != nil && view->selectionModel()->isRowSelected(node->index.row(), node->index.parent())) {
          [selected addIndex:static_cast<NSUInteger>(row)];
        }
      }
      if (![selected isEqualToIndexSet:binding.table.selectedRowIndexes]) {
        [binding.table selectRowIndexes:selected byExtendingSelection:NO];
        ++g_widgetPropertyWrites;
      }
    }
  }
  void rebuild() {
    detachBindings();
    for (MitcadNativeModelCell* cell in m_cells) { cell->owner = nullptr; }
    m_cellViews.clear();
    for (NSView* child in [m_root.arrangedSubviews copy]) { [m_root removeArrangedSubview:child]; [child removeFromSuperview]; }
    m_bindings.clear();
    ++m_generation;
    m_structureDirty = false;
    for (const auto& object : m_observed) { if (object) { object->removeEventFilter(this); } }
    m_observed.clear();
    add(m_root, renderWidget(m_source));
    m_widgetTree.clear();
    for (QWidget* widget : m_source->findChildren<QWidget*>()) { m_widgetTree.emplace_back(widget); }
    // ChildAdded arrives before QWidget construction finishes. Observe the
    // completed tree in this coalesced rebuild, never cast that child early.
    m_observed.push_back(m_source.data());
    const auto objects = m_source->findChildren<QObject*>();
    for (QObject* object : objects) { m_observed.push_back(object); }
    for (const auto& object : m_observed) { if (object) { object->installEventFilter(this); } }
  }
  QPointer<QWidget> m_source;
  QTimer m_timer;
  QTimer m_refreshTimer;
  MitcadNativeWidgetTarget* m_target = nil;
  NSStackView* m_root = nil;
  std::vector<Binding> m_bindings;
  std::vector<std::pair<QPointer<QWidget>, std::unique_ptr<AppKitWidgetView>>> m_cellViews;
  NSHashTable<MitcadNativeModelCell*>* m_cells = [NSHashTable weakObjectsHashTable];
  std::vector<QPointer<QObject>> m_observed;
  std::vector<QPointer<QWidget>> m_widgetTree;
  quint64 m_generation = 0;
  bool m_structureDirty = false;
  bool m_refreshing = false;
};

#undef NATIVE_SET
#undef NATIVE_STRING

quint64 nativeWidgetPropertyWrites() { return g_widgetPropertyWrites; }

class NativeDialogController final : public QObject {
public:
  NativeDialogController(QWidget* widget, bool modal)
      : m_widget(widget), m_modal(modal),
        m_presentAsSheet(modal || widget->windowModality() != Qt::NonModal || widget->property("nativePresentAsSheet").toBool()),
        m_presenterModality(widget->windowModality()) {
    m_target = [MitcadNativeWindowTarget new];
    m_target->owner = this;
    m_view = std::make_unique<AppKitWidgetView>(widget);
    NSView* view = (__bridge NSView*)m_view->nativeView();
    const QSize requested = widget->size().expandedTo(widget->sizeHint());
    const CGFloat width = std::clamp<CGFloat>(requested.width(), 380, 1000);
    const CGFloat height = std::clamp<CGFloat>(requested.height(), 140, 740);
    m_window = [[MitcadNativeSecondaryWindow alloc] initWithContentRect:NSMakeRect(0, 0, width, height)
                                         styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable
                                           backing:NSBackingStoreBuffered defer:NO];
    m_window.releasedWhenClosed = NO;
    m_window.title = ns(widget->windowTitle());
    m_window.identifier = @"native.secondary";
    m_window.delegate = m_target;
    m_window.contentView = view;
    m_window.minSize = NSMakeSize(360, 160);
    if (auto* dialog = qobject_cast<QDialog*>(widget)) {
      connect(dialog, &QDialog::finished, this, [this](int result) { finish(result); });
    }
    connect(widget, &QObject::destroyed, this, [this] { finish(QDialog::Rejected); });
    widget->installEventFilter(this);
  }
  ~NativeDialogController() override {
    if (m_widget != nullptr) {
      m_widget->removeEventFilter(this);
      m_widget->setWindowModality(m_presenterModality);
    }
    m_target->owner = nullptr;
    m_window.delegate = nil;
    if (m_window.sheetParent != nil) { [m_window.sheetParent endSheet:m_window]; }
    [m_window orderOut:nil];
    m_window.contentView = nil;
    m_view.reset();
  }
  NSWindow* window() const { return m_window; }
  QWidget* widget() const { return m_widget; }
  void show() {
    // Presenter show events initialize dialogs and settings without showing
    // Qt controls. The live interface is composed entirely of AppKit views.
    m_host = currentHost(m_widget);
    m_widget->setWindowModality(Qt::NonModal);
    m_widget->setAttribute(Qt::WA_DontShowOnScreen, true);
    m_widget->setAttribute(Qt::WA_ShowWithoutActivating, true);
    m_widget->show();
    m_view->refresh();
    NSWindow* host = m_host;
    if (m_presentAsSheet && host != nil) {
      [host beginSheet:m_window completionHandler:nil];
    } else { [m_window center]; [m_window makeKeyAndOrderFront:nil]; }
    m_view->focusInitial();
  }
  int exec() {
    show();
    g_modalWindows.push_back(m_window);
    pumpNativeEvents([this] { return m_finished; });
    g_modalWindows.pop_back();
    return m_result;
  }
  bool dispatchShortcut(NSEvent* event) {
    if (!m_widget || m_finished || m_window.attachedSheet || !m_window.isKeyWindow) return false;
    const QKeySequence sequence = mac::keySequence(event);
    if (sequence.isEmpty() || sequence[0].keyboardModifiers() == Qt::NoModifier) return false;
    // Only this presenter's explicit shortcuts participate. Text-view
    // Return remains a newline; Command+Return reaches the save shortcut.
    QPointer<QShortcut> match;
    for (QShortcut* shortcut : m_widget->findChildren<QShortcut*>()) {
      if (!shortcut->isEnabled() || !shortcut->keys().contains(sequence) ||
          shortcut->context() == Qt::WidgetShortcut || shortcut->context() == Qt::WidgetWithChildrenShortcut) continue;
      if (match) return false; // Do not choose arbitrarily between aliases.
      match = shortcut;
    }
    if (!match) return false;
    if (![m_window makeFirstResponder:nil]) return true;
    QTimer::singleShot(0, match, [match] { if (match && match->isEnabled()) emit match->activated(); });
    return true;
  }
  bool closeRequested() {
    m_restoreHost = m_restoreHost || m_window.isKeyWindow;
    if (qobject_cast<QProgressDialog*>(m_widget) != nullptr) { m_widget->close(); }
    else if (auto* dialog = qobject_cast<QDialog*>(m_widget)) { dialog->reject(); }
    else if (m_widget != nullptr) { m_widget->close(); }
    if (!m_finished && m_widget == nullptr) { finish(QDialog::Rejected); }
    return NO; // finish() handles removal or the presenter vetoes closing.
  }
  void finish(int result) {
    if (m_finished) { return; }
    m_finished = true;
    m_result = result;
    NSWindow* key = NSApp.keyWindow;
    const bool sheet = m_window.sheetParent != nil;
    const bool restore = m_restoreHost || m_window.isKeyWindow ||
        (sheet && (key == nil || key == m_host || key == g_documentWindow));
    if (m_window.sheetParent != nil) { [m_window.sheetParent endSheet:m_window returnCode:result]; }
    [m_window orderOut:nil];
    if (restore && m_host != nil) {
      // Qt's hidden presenter can queue parent activation while hiding.
      // Restore its visible AppKit host after that work, without moving
      // focus away from a different native window or a later user event.
      __weak NSWindow* host = m_host;
      __weak NSWindow* closed = m_window;
      __weak NSWindow* previousKey = key;
      const NSTimeInterval eventTime = NSApp.currentEvent.timestamp;
      QTimer::singleShot(0, QCoreApplication::instance(), [host, closed, previousKey, eventTime] {
        if (host == nil || !host.visible || host.attachedSheet != nil) { return; }
        NSWindow* current = NSApp.keyWindow;
        if (current != nil && current != host && current != closed &&
            current != previousKey && current != g_documentWindow) { return; }
        NSEvent* event = NSApp.currentEvent;
        const bool input = event.type == NSEventTypeKeyDown || event.type == NSEventTypeLeftMouseDown ||
                           event.type == NSEventTypeRightMouseDown || event.type == NSEventTypeOtherMouseDown;
        if (input && event.timestamp > eventTime) { return; }
        [host makeKeyWindow];
      });
    }
    if (!m_modal) { QTimer::singleShot(0, this, [this] { delete this; }); }
  }
protected:
  bool eventFilter(QObject* watched, QEvent* event) override {
    if (watched == m_widget && event->type() == QEvent::Hide && !m_finished) {
      m_restoreHost = m_restoreHost || m_window.isKeyWindow;
      // QDialog::done hides before emitting finished; let that signal
      // carry the accepted/rejected result before handling a bare hide.
      QTimer::singleShot(0, this, [this] {
        if (!m_finished && (m_widget == nullptr || !m_widget->isVisible())) {
          auto* dialog = qobject_cast<QDialog*>(m_widget);
          finish(dialog == nullptr ? QDialog::Rejected : dialog->result());
        }
      });
    }
    if (watched == m_widget && event->type() == QEvent::WindowTitleChange) { m_window.title = ns(m_widget->windowTitle()); }
    return QObject::eventFilter(watched, event);
  }
private:
  QPointer<QWidget> m_widget;
  bool m_modal = false;
  bool m_presentAsSheet = false;
  Qt::WindowModality m_presenterModality = Qt::NonModal;
  bool m_finished = false;
  int m_result = QDialog::Rejected;
  std::unique_ptr<AppKitWidgetView> m_view;
  MitcadNativeWindowTarget* m_target = nil;
  NSWindow* m_window = nil;
  __weak NSWindow* m_host = nil;
  bool m_restoreHost = false;
};

namespace {
std::vector<NativeDialogController*> g_modeless;
NSWindow* currentHost(QWidget* widget) {
  for (QWidget* parent = widget == nullptr ? nullptr : widget->parentWidget(); parent != nullptr; parent = parent->parentWidget()) {
    for (NativeDialogController* controller : g_modeless) {
      if (controller->widget() == parent) { return controller->window(); }
    }
  }
  if (!g_modalWindows.empty()) { return g_modalWindows.back(); }
  for (NativeDialogController* controller : g_modeless) {
    if (controller->window() == NSApp.keyWindow) { return controller->window(); }
  }
  return g_documentWindow;
}

int messageBox(QMessageBox* box) {
  QPointer<QMessageBox> presenter = box;
  NSAlert* alert = [NSAlert new];
  alert.messageText = ns(plain(box->text()));
  alert.informativeText = ns(plain(box->informativeText()));
  alert.alertStyle = box->icon() == QMessageBox::Critical ? NSAlertStyleCritical
                    : box->icon() == QMessageBox::Warning ? NSAlertStyleWarning : NSAlertStyleInformational;
  const auto buttons = box->buttons();
  NSButton* implicitDefault = nil;
  bool hasDefault = false;
  for (QAbstractButton* button : buttons) {
    NSButton* native = [alert addButtonWithTitle:ns(title(button->text()))];
    native.enabled = button->isEnabled();
    const auto role = box->buttonRole(button);
    if (implicitDefault == nil && (role == QMessageBox::AcceptRole || role == QMessageBox::YesRole)) { implicitDefault = native; }
    if (button == box->defaultButton()) { native.keyEquivalent = @"\r"; hasDefault = true; }
    else if (button == box->escapeButton() || box->standardButton(button) == QMessageBox::Cancel) { native.keyEquivalent = @"\033"; }
    else { native.keyEquivalent = @""; }
  }
  if (!hasDefault && implicitDefault != nil) { implicitDefault.keyEquivalent = @"\r"; }
  std::unique_ptr<NativeWidgetView> check;
  NSStackView* accessories = stack();
  if (!box->detailedText().isEmpty()) { add(accessories, label(box->detailedText())); }
  if (box->checkBox() != nullptr) {
    check = createNativeWidgetView(box->checkBox());
    add(accessories, (__bridge NSView*)check->nativeView());
  }
  if (accessories.arrangedSubviews.count > 0) {
    [accessories.widthAnchor constraintEqualToConstant:420].active = YES;
    alert.accessoryView = accessories;
  }
  struct Completion { NSModalResponse response = NSModalResponseCancel; bool done = false; };
  const auto completion = std::make_shared<Completion>();
  NSWindow* host = currentHost(box);
  QObject lifetime;
  const auto destroyed = QObject::connect(box, &QObject::destroyed, &lifetime, [&, completion] {
    if (alert.window.sheetParent != nil) { [alert.window.sheetParent endSheet:alert.window]; }
    [alert.window orderOut:nil];
    completion->done = true;
  });
  if (host != nil) {
    [alert beginSheetModalForWindow:host completionHandler:^(NSModalResponse result) {
      completion->response = result;
      completion->done = true;
    }];
    g_modalWindows.push_back(alert.window);
    pumpNativeEvents([completion] { return completion->done; });
    g_modalWindows.pop_back();
  } else { completion->response = [alert runModal]; }
  QObject::disconnect(destroyed);
  if (presenter == nullptr) { return QDialog::Rejected; }
  const NSInteger index = completion->response - NSAlertFirstButtonReturn;
  if (index >= 0 && index < buttons.size()) {
    buttons[static_cast<int>(index)]->click();
  } else { box->reject(); }
  return box->result();
}
} // namespace

// Native panels use the visible AppKit window explicitly. Hidden presenter
// QWindows are not sheet hosts, including when a file chooser is opened from
// a material, render, report or settings window.
class NativeFilePanel final : public QObject {
public:
  explicit NativeFilePanel(QFileDialog* dialog) : m_dialog(dialog) {
    m_target = [MitcadNativeFilePanelTarget new];
    m_target->owner = this;
    m_panel = dialog->acceptMode() == QFileDialog::AcceptSave ? [NSSavePanel savePanel] : [NSOpenPanel openPanel];
    m_panel.delegate = m_target;
    m_panel.title = ns(dialog->windowTitle());
    m_panel.canCreateDirectories = !dialog->testOption(QFileDialog::ReadOnly);
    m_panel.showsHiddenFiles = dialog->filter().testFlag(QDir::Hidden);
    m_panel.canSelectHiddenExtension = YES;
    m_panel.extensionHidden = NO;
    m_panel.prompt = ns(title(dialog->labelText(QFileDialog::Accept)));
    m_panel.nameFieldLabel = ns(title(dialog->labelText(QFileDialog::FileName)));
    m_panel.directoryURL = [NSURL fileURLWithPath:ns(dialog->directory().absolutePath()) isDirectory:YES];
    const QStringList selected = dialog->selectedFiles();
    if (!selected.isEmpty()) {
      const QFileInfo initial(selected.first());
      if (initial.isAbsolute()) {
        const QString directory = initial.isDir() ? initial.absoluteFilePath() : initial.absolutePath();
        m_panel.directoryURL = [NSURL fileURLWithPath:ns(directory) isDirectory:YES];
      }
      if (!initial.isDir()) { m_panel.nameFieldStringValue = ns(initial.fileName()); }
    }
    if ([m_panel isKindOfClass:NSOpenPanel.class]) {
      NSOpenPanel* open = (NSOpenPanel*)m_panel;
      const bool directory = dialog->fileMode() == QFileDialog::Directory || dialog->testOption(QFileDialog::ShowDirsOnly);
      open.canChooseDirectories = directory;
      open.canChooseFiles = !directory;
      open.allowsMultipleSelection = dialog->fileMode() == QFileDialog::ExistingFiles;
      open.resolvesAliases = !dialog->testOption(QFileDialog::DontResolveSymlinks);
    }
    m_filters = dialog->nameFilters();
    m_filterIndex = std::max<qsizetype>(0, m_filters.indexOf(dialog->selectedNameFilter()));
    if (m_filters.size() > 1 && dialog->fileMode() != QFileDialog::Directory) {
      NSStackView* accessory = stack(true);
      add(accessory, label(title(dialog->labelText(QFileDialog::FileType))));
      m_popup = [[NSPopUpButton alloc] initWithFrame:NSMakeRect(0, 0, 280, 26) pullsDown:NO];
      for (QString filter : m_filters) {
        if (dialog->testOption(QFileDialog::HideNameFilterDetails)) {
          filter.remove(QRegularExpression(QStringLiteral("\\s*\\([^()]*\\)\\s*$")));
        }
        [m_popup addItemWithTitle:ns(filter)];
      }
      [m_popup selectItemAtIndex:m_filterIndex];
      m_popup.target = m_target;
      m_popup.action = @selector(filterChanged:);
      m_popup.accessibilityIdentifier = @"native.file.filter";
      add(accessory, m_popup);
      [accessory.widthAnchor constraintEqualToConstant:380].active = YES;
      accessory.frame = NSMakeRect(0, 0, 380, std::max<CGFloat>(40, accessory.fittingSize.height));
      m_panel.accessoryView = accessory;
      if ([m_panel isKindOfClass:NSOpenPanel.class]) { ((NSOpenPanel*)m_panel).accessoryViewDisclosed = YES; }
    }
    updateFilter(false);
    connect(dialog, &QObject::destroyed, this, [this] { cancel(); });
    connect(dialog, &QDialog::finished, this, [this] { if (!m_complete) { cancel(); } });
  }
  ~NativeFilePanel() override {
    m_target->owner = nullptr;
    m_popup.target = nil;
    m_panel.delegate = nil;
    m_panel.accessoryView = nil;
    if (m_panel.sheetParent != nil) { [m_panel.sheetParent endSheet:m_panel]; }
    [m_panel orderOut:nil];
  }
  QStringList patterns() const {
    if (m_filters.isEmpty()) { return {}; }
    QString filter = m_filters.value(m_filterIndex);
    const auto match = QRegularExpression(QStringLiteral("\\(([^()]*)\\)\\s*$")).match(filter);
    if (match.hasMatch()) { filter = match.captured(1); }
    return filter.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
  }
  void updateFilter(bool changed) {
    if (m_dialog == nullptr) { return; }
    if (m_popup != nil) { m_filterIndex = m_popup.indexOfSelectedItem; }
    NSMutableArray<UTType*>* types = [NSMutableArray new];
    const QStringList wildcards = patterns();
    bool unrestricted = wildcards.isEmpty();
    for (const QString& pattern : wildcards) {
      if (!pattern.startsWith(QStringLiteral("*.")) || pattern.count(QLatin1Char('*')) != 1
          || pattern.contains(QLatin1Char('?')) || pattern.mid(2).contains(QLatin1Char('.'))) {
        unrestricted = true;
        break;
      }
      UTType* type = [UTType typeWithFilenameExtension:ns(pattern.mid(2))];
      if (type == nil) { unrestricted = true; break; }
      [types addObject:type];
    }
    m_panel.allowedContentTypes = unrestricted ? @[] : types;
    m_panel.allowsOtherFileTypes = unrestricted;
    [m_panel validateVisibleColumns];
    if (changed && !m_filters.isEmpty()) {
      const QPointer<NativeFilePanel> alive = this;
      m_dialog->selectNameFilter(m_filters.value(m_filterIndex));
      if (alive == nullptr || m_dialog == nullptr) { return; }
      QMetaObject::invokeMethod(m_dialog, "filterSelected", Qt::DirectConnection,
                                Q_ARG(QString, m_filters.value(m_filterIndex)));
    }
  }
  bool shouldEnable(NSURL* url) const {
    if (m_dialog == nullptr) { return false; }
    const QFileInfo file(qt(url.path));
    if (file.isDir() && (m_panel.treatsFilePackagesAsDirectories || ![NSWorkspace.sharedWorkspace isFilePackageAtPath:url.path])) {
      return true;
    }
    const QStringList wildcards = patterns();
    return wildcards.isEmpty() || wildcards.contains(QStringLiteral("*.*")) || QDir::match(wildcards, file.fileName());
  }
  void currentChanged() {
    if (m_dialog == nullptr || ![m_panel isKindOfClass:NSOpenPanel.class]) { return; }
    const QPointer<NativeFilePanel> alive = this;
    const QUrl url = QUrl::fromLocalFile(qt(m_panel.URL.path));
    QMetaObject::invokeMethod(m_dialog, "currentUrlChanged", Qt::DirectConnection, Q_ARG(QUrl, url));
    if (alive == nullptr || m_dialog == nullptr) { return; }
    QMetaObject::invokeMethod(m_dialog, "currentChanged", Qt::DirectConnection, Q_ARG(QString, url.toLocalFile()));
  }
  void directoryChanged() {
    if (m_dialog == nullptr) { return; }
    const QPointer<NativeFilePanel> alive = this;
    const QString directory = qt(m_panel.directoryURL.path);
    m_dialog->setDirectory(directory);
    if (alive == nullptr || m_dialog == nullptr) { return; }
    QMetaObject::invokeMethod(m_dialog, "directoryEntered", Qt::DirectConnection, Q_ARG(QString, directory));
    if (alive == nullptr || m_dialog == nullptr) { return; }
    const QUrl url = QUrl::fromLocalFile(directory);
    QMetaObject::invokeMethod(m_dialog, "directoryUrlEntered", Qt::DirectConnection, Q_ARG(QUrl, url));
  }
  QString fileName(QString path) const {
    if (m_dialog == nullptr || m_dialog->acceptMode() != QFileDialog::AcceptSave
        || QFileInfo(path).fileName().contains(QLatin1Char('.'))) { return path; }
    QString suffix = m_dialog->defaultSuffix();
    if (suffix.isEmpty()) {
      const auto wildcards = patterns();
      if (!wildcards.isEmpty() && wildcards.first().startsWith(QStringLiteral("*."))
          && !wildcards.first().mid(2).contains(QLatin1Char('*')) && !wildcards.first().contains(QLatin1Char('?'))) {
        suffix = wildcards.first().mid(2);
      }
    }
    if (!suffix.isEmpty()) { path += QLatin1Char('.') + suffix; }
    return path;
  }
  int exec() {
    NSWindow* host = currentHost(m_dialog);
    if (host != nil) {
      const QPointer<NativeFilePanel> owner = this;
      [m_panel beginSheetModalForWindow:host completionHandler:^(NSModalResponse response) {
        if (owner != nullptr) { owner->completed(response); }
      }];
      g_modalWindows.push_back(m_panel);
      pumpNativeEvents([this] { return m_complete; });
      g_modalWindows.pop_back();
    } else { completed([m_panel runModal]); }
    if (m_dialog == nullptr) { return QDialog::Rejected; }
    if (m_response != NSModalResponseOK) { m_dialog->reject(); return QDialog::Rejected; }
    QList<QUrl> urls;
    NSArray<NSURL*>* selected = [m_panel isKindOfClass:NSOpenPanel.class] ? ((NSOpenPanel*)m_panel).URLs
                                                                      : (m_panel.URL == nil ? @[] : @[m_panel.URL]);
    for (NSURL* url in selected) {
      urls += QUrl::fromLocalFile(fileName(qt(url.path).normalized(QString::NormalizationForm_C)));
    }
    if (urls.isEmpty()) { m_dialog->reject(); return QDialog::Rejected; }
    // Public QFileDialog setters preserve the existing single-file callers.
    // The result property also carries all files for the shared get-file helpers.
    m_dialog->setOption(QFileDialog::DontUseNativeDialog, true);
    m_dialog->setDirectory(qt(m_panel.directoryURL.path));
    if (!m_filters.isEmpty()) { m_dialog->selectNameFilter(m_filters.value(m_filterIndex)); }
    m_dialog->selectFile(urls.first().toLocalFile());
    m_dialog->setProperty("nativeSelectedUrls", QVariant::fromValue(urls));
    const QPointer<QFileDialog> presenter = m_dialog;
    QStringList paths;
    for (const QUrl& url : urls) { paths += url.toLocalFile(); }
    QMetaObject::invokeMethod(presenter, "urlsSelected", Qt::DirectConnection, Q_ARG(QList<QUrl>, urls));
    if (presenter == nullptr) { return QDialog::Rejected; }
    QMetaObject::invokeMethod(presenter, "filesSelected", Qt::DirectConnection, Q_ARG(QStringList, paths));
    if (presenter == nullptr) { return QDialog::Rejected; }
    if (urls.size() == 1) {
      const QUrl url = urls.first();
      QMetaObject::invokeMethod(presenter, "urlSelected", Qt::DirectConnection, Q_ARG(QUrl, url));
      if (presenter == nullptr) { return QDialog::Rejected; }
      const QString path = paths.first();
      QMetaObject::invokeMethod(presenter, "fileSelected", Qt::DirectConnection, Q_ARG(QString, path));
      if (presenter == nullptr) { return QDialog::Rejected; }
    }
    presenter->QDialog::done(QDialog::Accepted);
    return QDialog::Accepted;
  }
  void completed(NSModalResponse response) {
    if (m_complete) { return; }
    m_complete = true;
    m_response = response;
  }
  void cancel() {
    if (m_panel.sheetParent != nil) { [m_panel.sheetParent endSheet:m_panel returnCode:NSModalResponseCancel]; }
    [m_panel orderOut:nil];
    completed(NSModalResponseCancel);
  }
private:
  QPointer<QFileDialog> m_dialog;
  NSSavePanel* m_panel = nil;
  NSPopUpButton* m_popup = nil;
  MitcadNativeFilePanelTarget* m_target = nil;
  QStringList m_filters;
  qsizetype m_filterIndex = 0;
  NSModalResponse m_response = NSModalResponseCancel;
  bool m_complete = false;
};

class NativeColorPanel final : public QObject {
public:
  NativeColorPanel(const QColor& initial, QWidget* parent, const QString& text,
                   QColorDialog::ColorDialogOptions options) : m_parent(parent) {
    m_panel = NSColorPanel.sharedColorPanel;
    m_oldDelegate = m_panel.delegate;
    m_oldAccessory = m_panel.accessoryView;
    m_oldAlpha = m_panel.showsAlpha;
    m_oldTitle = m_panel.title;
    m_target = [MitcadNativeColorPanelTarget new];
    m_target->owner = this;
    m_panel.delegate = m_target;
    [m_panel setTarget:nil];
    [m_panel setAction:nil];
    m_panel.title = ns(text);
    m_panel.showsAlpha = options.testFlag(QColorDialog::ShowAlphaChannel);
    m_panel.color = [NSColor colorWithSRGBRed:initial.redF() green:initial.greenF() blue:initial.blueF() alpha:initial.alphaF()];
    NSStackView* buttons = stack(true);
    NSButton* cancel = [NSButton buttonWithTitle:ns(QColorDialog::tr("Cancel")) target:m_target action:@selector(cancel:)];
    cancel.keyEquivalent = @"\033";
    cancel.accessibilityIdentifier = @"native.color.cancel";
    add(buttons, cancel);
    NSButton* accept = [NSButton buttonWithTitle:ns(QColorDialog::tr("OK")) target:m_target action:@selector(accept:)];
    accept.keyEquivalent = @"\r";
    accept.accessibilityIdentifier = @"native.color.accept";
    add(buttons, accept);
    buttons.edgeInsets = NSEdgeInsetsMake(8, 8, 8, 8);
    buttons.frame = NSMakeRect(0, 0, std::max<CGFloat>(240, buttons.fittingSize.width),
                               std::max<CGFloat>(50, buttons.fittingSize.height));
    m_panel.accessoryView = buttons;
    if (parent != nullptr) { connect(parent, &QObject::destroyed, this, [this] { finish(false); }); }
  }
  ~NativeColorPanel() override {
    m_target->owner = nullptr;
    if (m_panel.sheetParent != nil) { [m_panel.sheetParent endSheet:m_panel]; }
    [m_panel orderOut:nil];
    m_panel.delegate = m_oldDelegate;
    [m_panel setTarget:nil];
    [m_panel setAction:nil];
    m_panel.accessoryView = m_oldAccessory;
    m_panel.showsAlpha = m_oldAlpha;
    m_panel.title = m_oldTitle;
  }
  QColor exec() {
    NSWindow* host = currentHost(m_parent);
    if (host != nil) { [host beginSheet:m_panel completionHandler:nil]; }
    else { [m_panel center]; [m_panel makeKeyAndOrderFront:nil]; }
    g_modalWindows.push_back(m_panel);
    pumpNativeEvents([this] { return m_complete; });
    g_modalWindows.pop_back();
    if (!m_accepted) { return {}; }
    NSColor* color = [m_panel.color colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    return color == nil ? QColor() : QColor::fromRgbF(color.redComponent, color.greenComponent, color.blueComponent, color.alphaComponent);
  }
  void finish(bool accepted) {
    if (m_complete) { return; }
    if (accepted && ![m_panel makeFirstResponder:nil]) { return; }
    m_complete = true;
    m_accepted = accepted;
    if (m_panel.sheetParent != nil) { [m_panel.sheetParent endSheet:m_panel returnCode:accepted ? NSModalResponseOK : NSModalResponseCancel]; }
    [m_panel orderOut:nil];
  }
private:
  QPointer<QWidget> m_parent;
  NSColorPanel* m_panel = nil;
  MitcadNativeColorPanelTarget* m_target = nil;
  id<NSWindowDelegate> m_oldDelegate = nil;
  NSView* m_oldAccessory = nil;
  NSString* m_oldTitle = nil;
  BOOL m_oldAlpha = NO;
  bool m_complete = false;
  bool m_accepted = false;
};

void setNativeDialogWindow(void* window) { g_documentWindow = (__bridge NSWindow*)window; }
bool nativeDialogsActive() { return g_documentWindow != nil; }
bool nativeDialogModalActive() { return !g_modalWindows.empty() || NSApp.modalWindow != nil; }
void waitNativeUntil(const std::function<bool()>& completed) { pumpNativeEvents(completed); }
std::unique_ptr<NativeWidgetView> createNativeWidgetView(QWidget* widget) {
  if (widget == nullptr) { return {}; }
  auto view = std::make_unique<AppKitWidgetView>(widget);
  view->refresh();
  return view;
}
int execNativeDialog(QDialog* dialog) {
  if (dialog == nullptr) { return QDialog::Rejected; }
  if (auto* file = qobject_cast<QFileDialog*>(dialog)) { return execNativeFileDialog(file); }
  if (auto* box = qobject_cast<QMessageBox*>(dialog)) { return messageBox(box); }
  NativeDialogController controller(dialog, true);
  return controller.exec();
}
int execNativeFileDialog(QFileDialog* dialog) {
  if (dialog == nullptr) { return QDialog::Rejected; }
  dialog->setProperty("nativeSelectedUrls", QVariant());
  NativeFilePanel panel(dialog);
  return panel.exec();
}
void* nativeFileDialogPanel() {
  for (auto it = g_modalWindows.rbegin(); it != g_modalWindows.rend(); ++it) {
    if ([*it isKindOfClass:NSSavePanel.class]) { return (__bridge void*)*it; }
  }
  return nullptr;
}
QList<QUrl> nativeFileDialogUrls(const QFileDialog* dialog) {
  if (dialog == nullptr) { return {}; }
  const QVariant result = dialog->property("nativeSelectedUrls");
  return result.isValid() ? result.value<QList<QUrl>>() : dialog->selectedUrls();
}
QColor getNativeColor(const QColor& initial, QWidget* parent, const QString& text, QColorDialog::ColorDialogOptions options) {
  NativeColorPanel panel(initial, parent, text, options);
  return panel.exec();
}
void showNativeWidget(QWidget* widget) {
  if (widget == nullptr) { return; }
  for (NativeDialogController* controller : g_modeless) {
    if (controller->widget() == widget) { [controller->window() makeKeyAndOrderFront:nil]; return; }
  }
  auto* controller = new NativeDialogController(widget, false);
  g_modeless.push_back(controller);
  QObject::connect(controller, &QObject::destroyed, [controller] {
    g_modeless.erase(std::remove(g_modeless.begin(), g_modeless.end(), controller), g_modeless.end());
  });
  controller->show();
}

// QMenu's Cocoa items dispatch through Qt's window responder chain. The
// native document window has its own chain, so popup actions need an
// explicit target. Keep Qt actions guarded and invoke the chosen action
// after tracking, when it may safely rebuild or destroy the source menu.
class NativeMenuPopup {
public:
  explicit NativeMenuPopup(QMenu* menu) : m_source(menu) {
    m_target = [[MitcadNativeMenuTarget alloc] init];
    m_target->owner = this;
    m_native = menuFor(menu);
  }

  ~NativeMenuPopup() {
    m_target->owner = nullptr;
    for (const auto& binding : m_menus) binding->native.delegate = nil;
  }

  QAction* exec(const QPoint& point) {
    update(m_native);
    if (m_source && m_native.numberOfItems > 0) {
      const NSRect screen = NSScreen.screens.firstObject.frame;
      [m_native popUpMenuPositioningItem:nil atLocation:NSMakePoint(point.x(), NSMaxY(screen) - point.y()) inView:nil];
    }
    const std::size_t menuCount = m_menus.size();
    for (std::size_t i = 0; i < menuCount; ++i) close(m_menus[i]->native);
    if (m_source && m_chosen && m_chosen->isEnabled() && m_chosen->isVisible()) m_chosen->trigger();
    return m_chosen;
  }

  void choose(NSMenuItem* item) {
    const QPointer<QAction> action = actionFor(item);
    if (m_source && action && action->isEnabled() && action->isVisible() && !action->menu()) m_chosen = action;
  }

  bool enabled(NSMenuItem* item) const {
    const QPointer<QAction> action = actionFor(item);
    return m_source && action && action->isEnabled() && action->isVisible();
  }

  void update(NSMenu* native, bool announce = true) {
    MenuBinding* binding = bindingFor(native);
    if (!binding || binding->refreshing) return;
    binding->refreshing = true;
    if (announce && binding->source && !binding->announced) {
      binding->announced = true;
      emit binding->source->aboutToShow();
    }
    [native removeAllItems];
    const QPointer<QMenu> source = binding->source;
    if (m_source && source) {
      const auto actions = source->actions();
      const bool collapseSeparators = source->separatorsCollapsible();
      bool separatorPending = false;
      for (QAction* raw : actions) {
        QPointer<QAction> action(raw);
        if (!action || !action->isVisible()) continue;
        if (action->isSeparator()) {
          if (collapseSeparators) separatorPending = native.numberOfItems > 0;
          else [native addItem:NSMenuItem.separatorItem];
          continue;
        }
        if (separatorPending) {
          [native addItem:NSMenuItem.separatorItem];
          separatorPending = false;
        }
        NSMenuItem* item = [[NSMenuItem alloc] initWithTitle:ns(title(action->text()))
                                                    action:@selector(perform:) keyEquivalent:@""];
        item.target = m_target;
        item.tag = actionIndex(action);
        item.enabled = action->isEnabled();
        item.state = action->isCheckable() && action->isChecked() ? NSControlStateValueOn : NSControlStateValueOff;
        item.toolTip = ns(plain(action->toolTip()));
        setShortcut(item, action->shortcut());
        if (action->isIconVisibleInMenu() && !action->icon().isNull()) {
          item.image = image(action->icon().pixmap(QSize(32, 32)));
          item.image.size = NSMakeSize(16, 16);
          if (@available(macOS 27.0, *)) item.preferredImageVisibility = NSMenuItemImageVisibilityVisible;
        }
        if (QMenu* submenu = action->menu()) item.submenu = menuFor(submenu);
        [native addItem:item];
      }
    }
    binding->refreshing = false;
  }

  void close(NSMenu* native) {
    MenuBinding* binding = bindingFor(native);
    if (!binding || !binding->announced) return;
    binding->announced = false;
    if (binding->source) emit binding->source->aboutToHide();
  }

private:
  struct MenuBinding {
    QPointer<QMenu> source;
    NSMenu* __strong native = nil;
    bool announced = false;
    bool refreshing = false;
  };

  MenuBinding* bindingFor(NSMenu* native) const {
    for (const auto& binding : m_menus) if (binding->native == native) return binding.get();
    return nullptr;
  }

  NSMenu* menuFor(QMenu* source) {
    for (const auto& binding : m_menus) if (binding->source == source) return binding->native;
    auto binding = std::make_unique<MenuBinding>();
    binding->source = source;
    binding->native = [[NSMenu alloc] initWithTitle:ns(title(source->title()))];
    binding->native.delegate = m_target;
    NSMenu* native = binding->native;
    m_menus.push_back(std::move(binding));
    // Submenus need their current items before AppKit validates their
    // parent, but dynamic aboutToShow callbacks run only when opened.
    update(native, false);
    return native;
  }

  NSInteger actionIndex(QAction* action) {
    for (std::size_t i = 0; i < m_actions.size(); ++i) if (m_actions[i] == action) return static_cast<NSInteger>(i);
    m_actions.emplace_back(action);
    return static_cast<NSInteger>(m_actions.size() - 1);
  }

  QPointer<QAction> actionFor(NSMenuItem* item) const {
    const NSInteger index = item.tag;
    return item.target == m_target && index >= 0 && static_cast<std::size_t>(index) < m_actions.size()
               ? m_actions[static_cast<std::size_t>(index)] : QPointer<QAction>();
  }

  static void setShortcut(NSMenuItem* item, const QKeySequence& sequence) {
    mac::setKeyEquivalent(item, sequence);
  }

  QPointer<QMenu> m_source;
  QPointer<QAction> m_chosen;
  MitcadNativeMenuTarget* __strong m_target = nil;
  NSMenu* __strong m_native = nil;
  std::vector<std::unique_ptr<MenuBinding>> m_menus;
  std::vector<QPointer<QAction>> m_actions;
};

QAction* execNativeMenu(QMenu* menu, const QPoint& point) {
  if (menu == nullptr) { return nullptr; }
  NativeMenuPopup popup(menu);
  return popup.exec(point);
}

} // namespace mitcad

@implementation MitcadNativeMenuTarget
- (void)perform:(NSMenuItem*)sender { if (owner) owner->choose(sender); }
- (BOOL)validateMenuItem:(NSMenuItem*)item { return owner && owner->enabled(item); }
- (void)menuNeedsUpdate:(NSMenu*)menu { if (owner) owner->update(menu); }
- (void)menuDidClose:(NSMenu*)menu { if (owner) owner->close(menu); }
@end

@implementation MitcadNativeWidgetTarget
- (void)perform:(id)sender { if (owner != nullptr) { owner->action(sender); } }
- (void)doubleClick:(id)sender { if (owner != nullptr) { owner->modelActivated(sender); } }
- (void)controlTextDidChange:(NSNotification*)notification { if (owner != nullptr) { owner->textChanged(notification.object, false); } }
- (void)controlTextDidEndEditing:(NSNotification*)notification { if (owner != nullptr) { owner->textChanged(notification.object, true); } }
- (BOOL)control:(NSControl*)control textShouldEndEditing:(NSText*)editor {
  (void)editor;
  return owner == nullptr || owner->textCanEnd(control);
}
- (void)textDidChange:(NSNotification*)notification { if (owner != nullptr) { owner->textViewChanged(notification.object); } }
- (BOOL)textView:(NSTextView*)view clickedOnLink:(id)link atIndex:(NSUInteger)index {
  (void)index;
  return owner != nullptr && owner->linkClicked(view, link);
}
- (void)tabView:(NSTabView*)view didSelectTabViewItem:(NSTabViewItem*)item { if (owner != nullptr) { owner->tabChanged(view, item); } }
- (BOOL)tabView:(NSTabView*)view shouldSelectTabViewItem:(NSTabViewItem*)item {
  (void)item;
  return view.window == nil || [view.window makeFirstResponder:nil];
}
- (NSInteger)numberOfRowsInTableView:(NSTableView*)view { return owner == nullptr ? 0 : owner->children(owner->modelBinding(view), nil).count; }
- (NSView*)tableView:(NSTableView*)view viewForTableColumn:(NSTableColumn*)column row:(NSInteger)row {
  return owner == nullptr ? nil : owner->modelCell(view, column, owner->nodeAt(view, row));
}
- (void)tableViewSelectionDidChange:(NSNotification*)notification { if (owner != nullptr) { owner->modelSelection(notification.object); } }
- (NSInteger)outlineView:(NSOutlineView*)view numberOfChildrenOfItem:(id)item {
  return owner == nullptr ? 0 : owner->children(owner->modelBinding(view), item).count;
}
- (id)outlineView:(NSOutlineView*)view child:(NSInteger)index ofItem:(id)item {
  return owner == nullptr ? nil : owner->children(owner->modelBinding(view), item)[static_cast<NSUInteger>(index)];
}
- (BOOL)outlineView:(NSOutlineView*)view isItemExpandable:(id)item {
  return owner != nullptr && owner->children(owner->modelBinding(view), item).count > 0;
}
- (NSView*)outlineView:(NSOutlineView*)view viewForTableColumn:(NSTableColumn*)column item:(id)item {
  return owner == nullptr ? nil : owner->modelCell(view, column, item);
}
- (void)outlineViewSelectionDidChange:(NSNotification*)notification { if (owner != nullptr) { owner->modelSelection(notification.object); } }
- (void)outlineViewItemDidExpand:(NSNotification*)notification {
  if (owner != nullptr) { owner->modelExpanded(notification.object, notification.userInfo[@"NSObject"], true); }
}
- (void)outlineViewItemDidCollapse:(NSNotification*)notification {
  if (owner != nullptr) { owner->modelExpanded(notification.object, notification.userInfo[@"NSObject"], false); }
}
@end

@implementation MitcadNativeModelCell
- (void)controlTextDidEndEditing:(NSNotification*)notification {
  if (owner != nullptr && index.isValid()) { owner->modelEdit(binding, index, QString::fromUtf8([(NSTextField*)notification.object stringValue].UTF8String), Qt::EditRole); }
}
- (void)checked:(id)sender {
  if (owner != nullptr && index.isValid()) { owner->modelEdit(binding, index, [(NSButton*)sender state] == NSControlStateValueOn ? Qt::Checked : Qt::Unchecked, Qt::CheckStateRole); }
}
@end

@implementation MitcadNativeModelHoverButton
- (BOOL)becomeFirstResponder {
  const BOOL accepted = [super becomeFirstResponder];
  if (accepted) { [self publishHover:YES]; }
  return accepted;
}
- (BOOL)resignFirstResponder {
  const BOOL accepted = [super resignFirstResponder];
  if (accepted) { [self publishHover:NO]; }
  return accepted;
}
- (void)updateTrackingAreas {
  if (hoverArea != nil) { [self removeTrackingArea:hoverArea]; }
  hoverArea = [[NSTrackingArea alloc] initWithRect:NSZeroRect
      options:NSTrackingMouseEnteredAndExited | NSTrackingActiveInActiveApp | NSTrackingInVisibleRect
      owner:self userInfo:nil];
  [self addTrackingArea:hoverArea];
  [super updateTrackingAreas];
}
- (void)mouseEntered:(NSEvent*)event {
  (void)event;
  [self publishHover:YES];
}
- (void)mouseExited:(NSEvent*)event {
  (void)event;
  [self publishHover:NO];
}
- (void)publishHover:(BOOL)entered {
  if (presenter != nullptr && (!entered || modelIndex.isValid())) {
    const QModelIndex index = entered ? QModelIndex(modelIndex) : QModelIndex();
    QMetaObject::invokeMethod(presenter.data(), "entered", Qt::DirectConnection,
                              Q_ARG(QModelIndex, index));
  }
}
@end

@implementation MitcadNativeWindowTarget
- (BOOL)windowShouldClose:(NSWindow*)sender { (void)sender; return owner == nullptr ? YES : owner->closeRequested(); }
@end

@implementation MitcadNativeSecondaryWindow
- (BOOL)performKeyEquivalent:(NSEvent*)event {
  if ([self.firstResponder isKindOfClass:MitcadNativeShortcutField.class])
    return [(MitcadNativeShortcutField*)self.firstResponder performKeyEquivalent:event];
  const QKeySequence sequence = mitcad::mac::keySequence(event);
  if (!sequence.isEmpty() && sequence[0].keyboardModifiers() == Qt::NoModifier &&
      (sequence[0].key() == Qt::Key_Return || sequence[0].key() == Qt::Key_Enter) &&
      [self.firstResponder isKindOfClass:NSTextView.class]) {
    NSTextView* text = (NSTextView*)self.firstResponder;
    if (text.editable && !text.isFieldEditor) {
      [text insertNewline:self];
      return YES;
    }
  }
  if ([self.delegate isKindOfClass:MitcadNativeWindowTarget.class]) {
    MitcadNativeWindowTarget* target = (MitcadNativeWindowTarget*)self.delegate;
    if (target->owner && target->owner->dispatchShortcut(event)) return YES;
  }
  const NSEventModifierFlags modifiers = event.modifierFlags &
      (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagShift);
  if (modifiers == NSEventModifierFlagCommand &&
      [event.charactersIgnoringModifiers.lowercaseString isEqualToString:@"w"]) {
    // This window owns its close key even while application activation
    // or the hidden Qt presenter's focus bookkeeping is still pending.
    if ([self makeFirstResponder:nil]) { [self performClose:self]; }
    return YES;
  }
  return [super performKeyEquivalent:event];
}
- (void)keyDown:(NSEvent*)event {
  const NSEventModifierFlags modifiers = event.modifierFlags &
      (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagShift);
  if (modifiers == 0 && event.keyCode == 53) { [self cancelOperation:self]; return; }
  [super keyDown:event];
}
- (void)cancelOperation:(id)sender {
  (void)sender;
  if ([self.delegate isKindOfClass:MitcadNativeWindowTarget.class]) {
    MitcadNativeWindowTarget* target = (MitcadNativeWindowTarget*)self.delegate;
    if (target->owner != nullptr) { target->owner->closeRequested(); }
  }
}
@end

@implementation MitcadNativeFilePanelTarget
- (void)filterChanged:(id)sender { (void)sender; if (owner != nullptr) { owner->updateFilter(true); } }
- (BOOL)panel:(id)sender shouldEnableURL:(NSURL*)url { (void)sender; return owner != nullptr && owner->shouldEnable(url); }
- (void)panelSelectionDidChange:(id)sender { (void)sender; if (owner != nullptr) { owner->currentChanged(); } }
- (void)panel:(id)sender didChangeToDirectoryURL:(NSURL*)url { (void)sender; (void)url; if (owner != nullptr) { owner->directoryChanged(); } }
- (NSString*)panel:(id)sender userEnteredFilename:(NSString*)filename confirmed:(BOOL)confirmed {
  (void)sender; (void)confirmed;
  return owner == nullptr ? filename : mitcad::ns(owner->fileName(mitcad::qt(filename)));
}
@end

@implementation MitcadNativeColorPanelTarget
- (void)accept:(id)sender { (void)sender; if (owner != nullptr) { owner->finish(true); } }
- (void)cancel:(id)sender { (void)sender; if (owner != nullptr) { owner->finish(false); } }
- (BOOL)windowShouldClose:(NSWindow*)sender { (void)sender; if (owner != nullptr) { owner->finish(false); } return NO; }
@end

@implementation MitcadNativeTable
- (void)rightMouseDown:(NSEvent*)event {
  MitcadNativeWidgetTarget* target = (MitcadNativeWidgetTarget*)self.delegate;
  if (target != nil && target->owner != nullptr) { target->owner->modelContext(self, [self convertPoint:event.locationInWindow fromView:nil]); }
}
@end

@implementation MitcadNativeOutline
- (void)rightMouseDown:(NSEvent*)event {
  MitcadNativeWidgetTarget* target = (MitcadNativeWidgetTarget*)self.delegate;
  if (target != nil && target->owner != nullptr) { target->owner->modelContext(self, [self convertPoint:event.locationInWindow fromView:nil]); }
}
@end

@implementation MitcadNativeScreenshot
- (BOOL)isFlipped { return YES; }
- (NSRect)shownRect {
  if (presenter == nullptr || presenter->sourceImage().isNull()) { return NSZeroRect; }
  const QImage& image = presenter->sourceImage();
  const CGFloat scale = std::min(self.bounds.size.width / image.width(), self.bounds.size.height / image.height());
  const NSSize size = NSMakeSize(image.width() * scale, image.height() * scale);
  return NSMakeRect((self.bounds.size.width - size.width) / 2, (self.bounds.size.height - size.height) / 2,
                    size.width, size.height);
}
- (void)refresh {
  if (presenter == nullptr) {
    if (bitmap != nil) { bitmap = nil; self.needsDisplay = YES; ++mitcad::g_widgetPropertyWrites; }
    return;
  }
  const QImage& source = presenter->sourceImage();
  bool changed = false;
  if (imageKey != source.cacheKey()) {
    imageKey = source.cacheKey();
    bitmap = mitcad::image(QPixmap::fromImage(source));
    changed = true;
  }
  if (drawnCrop != presenter->cropRect()) {
    drawnCrop = presenter->cropRect();
    changed = true;
  }
  if (changed) { self.needsDisplay = YES; ++mitcad::g_widgetPropertyWrites; }
}
- (void)drawRect:(NSRect)dirty {
  (void)dirty;
  const NSRect shown = [self shownRect];
  if (NSIsEmptyRect(shown) || bitmap == nil) { return; }
  [bitmap drawInRect:shown fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1
     respectFlipped:YES hints:nil];
  NSRect keep = NSZeroRect;
  if (dragging) {
    keep = NSIntersectionRect(shown, NSMakeRect(std::min(from.x, to.x), std::min(from.y, to.y),
                                               std::abs(to.x - from.x), std::abs(to.y - from.y)));
  } else if (presenter != nullptr && !presenter->cropRect().isNull()) {
    const QRect crop = presenter->cropRect();
    const CGFloat scale = shown.size.width / presenter->sourceImage().width();
    keep = NSMakeRect(shown.origin.x + crop.x() * scale, shown.origin.y + crop.y() * scale,
                      crop.width() * scale, crop.height() * scale);
  }
  if (!NSIsEmptyRect(keep)) {
    NSBezierPath* outside = [NSBezierPath bezierPathWithRect:shown];
    [outside appendBezierPathWithRect:keep];
    outside.windingRule = NSWindingRuleEvenOdd;
    [[NSColor colorWithWhite:0 alpha:0.45] setFill];
    [outside fill];
    [NSColor.controlAccentColor setStroke];
    NSBezierPath* border = [NSBezierPath bezierPathWithRect:keep];
    border.lineWidth = 2;
    [border stroke];
  }
}
- (void)mouseDown:(NSEvent*)event {
  if (presenter == nullptr || presenter->sourceImage().isNull()) { return; }
  from = to = [self convertPoint:event.locationInWindow fromView:nil];
  dragging = YES;
  self.needsDisplay = YES;
}
- (void)mouseDragged:(NSEvent*)event {
  if (!dragging) { return; }
  to = [self convertPoint:event.locationInWindow fromView:nil];
  self.needsDisplay = YES;
}
- (void)mouseUp:(NSEvent*)event {
  if (!dragging || presenter == nullptr) { return; }
  to = [self convertPoint:event.locationInWindow fromView:nil];
  dragging = NO;
  const NSRect shown = [self shownRect];
  if (!NSIsEmptyRect(shown)) {
    const CGFloat scale = presenter->sourceImage().width() / shown.size.width;
    const auto point = [&](NSPoint point) {
      return QPoint(static_cast<int>((point.x - shown.origin.x) * scale),
                    static_cast<int>((point.y - shown.origin.y) * scale));
    };
    const QRect crop = QRect(point(from), point(to)).normalized().intersected(presenter->sourceImage().rect());
    if (crop.width() >= 8 && crop.height() >= 8) { presenter->setCropRect(crop); }
  }
  self.needsDisplay = YES;
}
@end

@implementation MitcadNativeHoverButton
- (void)updateTrackingAreas {
  if (hoverArea != nil) { [self removeTrackingArea:hoverArea]; }
  hoverArea = [[NSTrackingArea alloc] initWithRect:NSZeroRect
                                       options:NSTrackingMouseEnteredAndExited | NSTrackingActiveInKeyWindow | NSTrackingInVisibleRect
                                         owner:self userInfo:nil];
  [self addTrackingArea:hoverArea];
  [super updateTrackingAreas];
}
- (void)mouseEntered:(NSEvent*)event {
  (void)event;
  if (presenter != nullptr && self.enabled && !self.hidden) {
    QEnterEvent enter{QPointF(), QPointF(), QPointF()};
    QCoreApplication::sendEvent(presenter, &enter);
  }
}
- (void)mouseExited:(NSEvent*)event {
  (void)event;
  if (presenter != nullptr) {
    QEvent leave(QEvent::Leave);
    QCoreApplication::sendEvent(presenter, &leave);
  }
}
@end

@implementation MitcadNativeShortcutField
- (void)clearShortcut:(id)sender {
  (void)sender;
  if (edit) edit->clear();
  self.stringValue = @"";
}
- (BOOL)becomeFirstResponder { return YES; }
- (BOOL)acceptsFirstResponder { return YES; }
- (void)mouseDown:(NSEvent*)event { (void)event; [self.window makeFirstResponder:self]; }
- (BOOL)performKeyEquivalent:(NSEvent*)event {
  if (self.window.firstResponder != self) { return NO; }
  [self keyDown:event];
  return YES;
}
- (void)keyDown:(NSEvent*)event {
  if (edit == nullptr) { return; }
  const QKeySequence sequence = mitcad::mac::keySequence(event);
  if (sequence.isEmpty()) return;
  edit->setKeySequence(sequence);
  self.stringValue = sequence.toString(QKeySequence::NativeText).toNSString();
}
@end
