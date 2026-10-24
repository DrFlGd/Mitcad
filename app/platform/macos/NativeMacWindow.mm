// SPDX-License-Identifier: MIT
#include "platform/NativeMacWindow.hpp"
#include "NativeKeys.hpp"

#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

#include <algorithm>
#include <cmath>
#include <cstring>
#include <utility>

#include <QApplication>
#include <QHash>
#include <QKeySequence>
#include <QImage>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QTextEdit>
#include <QAbstractSpinBox>
#include <QNativeGestureEvent>
#include <QOpenGLWidget>
#include <QPointingDevice>
#include <QPointer>
#include <QPalette>
#include <QSet>
#include <QWidget>
#include <QWindow>
#include <QtLogging>

namespace mitcad::mac {
struct NativeMacWindowState;
}

@interface MITCADDocumentWindow : NSWindow {
@public
  mitcad::mac::NativeMacWindowState* state;
}
- (BOOL)performKeyEquivalent:(NSEvent*)event;
- (BOOL)dispatchConfirmation;
- (BOOL)dispatchCancellation;
- (BOOL)dispatchKeyboardEvent:(NSEvent*)event;
@end

@interface MITCADViewportHost : NSView {
@public
  mitcad::mac::NativeMacWindowState* state;
}
@end

@interface MITCADCanvasRoot : NSView {
@public
  mitcad::mac::NativeMacWindowState* state;
}
@end

@interface MITCADCommandButton : NSButton
@property(nonatomic, copy) NSString* representedObject;
@end
@implementation MITCADCommandButton
@end

// The outer view owns the shadow; the system material owns its content.
@interface MITCADFloatingCard : NSView
@property(nonatomic, strong) NSView* body;
@end

@implementation MITCADFloatingCard
- (BOOL)isFlipped { return YES; }
- (NSView*)hitTest:(NSPoint)point {
  NSPoint local = [self convertPoint:point fromView:self.superview];
  if (![[NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:14 yRadius:14] containsPoint:local]) return nil;
  return [super hitTest:point];
}
@end

@interface MITCADModelOutlineView : NSOutlineView {
@public
  mitcad::mac::NativeMacWindowState* state;
}
@end

@interface MITCADHistoryStrip : NSView {
@public
  mitcad::mac::NativeMacWindowState* state;
}
@end

@interface MITCADHistoryFeatureButton : NSButton <NSDraggingSource> {
@public
  mitcad::mac::NativeMacWindowState* state;
  NSPoint pressPoint;
  BOOL pressed;
}
@property(nonatomic, copy) NSString* featureKey;
@property(nonatomic, copy) NSString* statusKey;
- (void)selectHistoryFeature;
- (void)activateHistoryFeature;
- (void)reorderHistoryFeatureToIndex:(NSInteger)index;
@end

@interface MITCADHistoryMarkerButton : NSButton {
@public
  mitcad::mac::NativeMacWindowState* state;
}
- (BOOL)beginHistoryMarkerDrag;
- (void)dragHistoryMarkerToIndex:(NSInteger)index;
- (void)endHistoryMarkerDrag;
@end

@interface MITCADInspectorDocumentView : NSView
@end

@implementation MITCADInspectorDocumentView
- (BOOL)isFlipped { return YES; }
@end

@interface MITCADTreeNode : NSObject
@property(nonatomic, copy) NSString* key;
@property(nonatomic, strong) NSMutableArray<MITCADTreeNode*>* children;
@end

@implementation MITCADTreeNode
@end

@interface MITCADNativeWindowDelegate : NSObject <NSApplicationDelegate, NSWindowDelegate, NSToolbarDelegate,
    NSOutlineViewDataSource, NSOutlineViewDelegate, NSTableViewDataSource, NSTableViewDelegate,
    NSControlTextEditingDelegate, NSTextFieldDelegate, NSSearchFieldDelegate,
    NSMenuItemValidation, NSPopoverDelegate> {
@public
  mitcad::mac::NativeMacWindowState* state;
}
- (void)performAction:(id)sender;
- (void)toggleSidebar:(id)sender;
- (void)toggleInspector:(id)sender;
- (void)toggleHistory:(id)sender;
- (void)openSearch:(id)sender;
- (void)chooseSearch:(id)sender;
- (void)searchChanged:(id)sender;
- (void)openTimelineSearchFilter:(id)sender;
- (void)stepTimelineSearch:(id)sender;
- (void)activateTree:(id)sender;
- (void)clickHistoryFeature:(id)sender;
- (void)stepHistory:(id)sender;
- (void)openHistoryFilter:(id)sender;
- (void)openProjectMenu:(id)sender;
- (void)toggleTreeVisibility:(id)sender;
- (void)renameTree:(id)sender;
- (void)closeDocument:(id)sender;
- (void)quitApplication:(id)sender;
- (void)menuTrackingChanged:(NSNotification*)notification;
@end

namespace mitcad::mac {
namespace {

NSString* ns(const QString& text) { return text.toNSString(); }
QString qt(NSString* text) { return QString::fromNSString(text != nil ? text : @""); }

constexpr CGFloat ribbonButtonWidth = 36;
constexpr CGFloat ribbonButtonHeight = 32;
constexpr CGFloat ribbonButtonGap = 8;
constexpr CGFloat ribbonGroupGap = 24;
constexpr CGFloat ribbonInset = 12;
constexpr CGFloat historyInset = 14;
constexpr CGFloat historySlot = 44;
constexpr CGFloat historyRunSlot = 16;
constexpr CGFloat historyStripHeight = 56;

CGFloat ribbonIconRowWidth(int count) {
  return count > 0 ? count * ribbonButtonWidth + (count - 1) * ribbonButtonGap : 0;
}

CGFloat ribbonGroupWidth(int count, CGFloat captionWidth) {
  return std::max(captionWidth, ribbonIconRowWidth(count) + ribbonInset);
}

NSImage* symbol(const QString& name, NSString* description = nil) {
  NSImage* image = [NSImage imageWithSystemSymbolName:ns(name) accessibilityDescription:description];
  if (image == nil) {
    image = [NSImage imageWithSystemSymbolName:@"square.dashed" accessibilityDescription:description];
  }
  [image setTemplate:YES];
  return image;
}

NSImage* renderActionImage(const NativeMacAction& action) {
  if (!action.symbol.isEmpty()) {
    NSImage* image = [NSImage imageWithSystemSymbolName:ns(action.symbol) accessibilityDescription:ns(action.label)];
    if (image) { [image setTemplate:YES]; return image; }
  }
  if (!action.icon.isNull()) {
    QImage image = action.icon.pixmap(48, 48).toImage().convertToFormat(QImage::Format_RGBA8888);
    // QImage's RGBA8888 stores straight alpha; AppKit must use the same
    // format to preserve the CAD colours along antialiased edges.
    NSBitmapImageRep* bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nil
       pixelsWide:image.width() pixelsHigh:image.height() bitsPerSample:8 samplesPerPixel:4
       hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bitmapFormat:NSBitmapFormatAlphaNonpremultiplied
       bytesPerRow:image.bytesPerLine() bitsPerPixel:32];
    if (bitmap) {
      memcpy(bitmap.bitmapData, image.constBits(), size_t(image.sizeInBytes()));
      NSImage* result = [[NSImage alloc] initWithSize:NSMakeSize(24, 24)];
      [result addRepresentation:bitmap];
      [result setTemplate:action.iconTemplate];
      result.accessibilityDescription = ns(action.label);
      return result;
    }
  }
  return symbol(QStringLiteral("square.dashed"), ns(action.label));
}

QString actionMenuPath(const NativeMacAction& item) {
  if (!item.menu.isEmpty()) return item.menu;
  const bool sketch = item.workspace.compare(QStringLiteral("SKETCH"), Qt::CaseInsensitive) == 0;
  // The BUILD tab's commands (mitcad#375) in a menu of their own; this
  // frontend has no workspace for them on its toolbar.
  if (item.workspace.compare(QStringLiteral("BUILD"), Qt::CaseInsensitive) == 0) return QStringLiteral("Build");
  // Menu-only commands can retain the default CREATE group. Without an
  // explicit menu or CAD workspace they belong to command search only.
  if (!sketch && item.workspace.compare(QStringLiteral("SOLID"), Qt::CaseInsensitive) != 0) return QString();
  if (item.id.startsWith(QStringLiteral("file."))) return QStringLiteral("File");
  if (item.id.startsWith(QStringLiteral("edit."))) return QStringLiteral("Edit");
  if (item.id.startsWith(QStringLiteral("view."))) return QStringLiteral("View");
  if (sketch) return QStringLiteral("Sketch");
  const QString group = item.group.isEmpty() ? QStringLiteral("Tools") : item.group;
  return group.left(1).toUpper() + group.mid(1).toLower();
}

bool isAppAction(const NativeMacAction& item) {
  return item.id.startsWith(QStringLiteral("app.")) ||
         item.menu.section(QLatin1Char('/'), 0, 0) == QStringLiteral("Mitcad");
}

SEL editingSelector(const NativeMacAction* action) {
  if (!action) return nullptr;
  const QKeySequence shortcut(action->shortcut, QKeySequence::PortableText);
  for (const auto& pair : {std::pair<QKeySequence::StandardKey, SEL>(QKeySequence::Cut, @selector(cut:)),
                           std::pair<QKeySequence::StandardKey, SEL>(QKeySequence::Copy, @selector(copy:)),
                           std::pair<QKeySequence::StandardKey, SEL>(QKeySequence::Paste, @selector(paste:)),
                           std::pair<QKeySequence::StandardKey, SEL>(QKeySequence::SelectAll, @selector(selectAll:))}) {
    if (!shortcut.isEmpty() && shortcut == QKeySequence(pair.first)) return pair.second;
  }
  return nullptr;
}

QPoint screenPoint(NSEvent* event) {
  const NSPoint point = [event.window convertPointToScreen:event.locationInWindow];
  const CGFloat top = NSScreen.screens.firstObject.frame.size.height;
  return QPoint(int(std::lround(point.x)), int(std::lround(top - point.y)));
}

QPoint menuPoint(NSView* control) {
  const NSPoint local = [control convertPoint:NSMakePoint(NSMinX(control.bounds), NSMaxY(control.bounds)) toView:nil];
  const NSPoint point = [control.window convertPointToScreen:local];
  const CGFloat top = NSScreen.screens.firstObject.frame.size.height;
  return QPoint(int(std::lround(point.x)), int(std::lround(top - point.y)));
}

NSTextField* label(NSString* text, NSFont* font = nil) {
  NSTextField* field = [NSTextField labelWithString:text];
  field.font = font != nil ? font : [NSFont systemFontOfSize:NSFont.systemFontSize];
  field.translatesAutoresizingMaskIntoConstraints = NO;
  return field;
}

NSScrollView* scroll(NSView* document) {
  NSScrollView* view = [[NSScrollView alloc] initWithFrame:NSZeroRect];
  view.documentView = document;
  view.hasVerticalScroller = YES;
  view.autohidesScrollers = YES;
  view.drawsBackground = NO;
  view.translatesAutoresizingMaskIntoConstraints = NO;
  return view;
}

void fill(NSView* child, NSView* parent, CGFloat inset = 0) {
  child.translatesAutoresizingMaskIntoConstraints = NO;
  [parent addSubview:child];
  [NSLayoutConstraint activateConstraints:@[
    [child.leadingAnchor constraintEqualToAnchor:parent.leadingAnchor constant:inset],
    [child.trailingAnchor constraintEqualToAnchor:parent.trailingAnchor constant:-inset],
    [child.topAnchor constraintEqualToAnchor:parent.topAnchor constant:inset],
    [child.bottomAnchor constraintEqualToAnchor:parent.bottomAnchor constant:-inset]
  ]];
}

MITCADFloatingCard* floatingCard(NSString* identifier) {
  MITCADFloatingCard* card = [[MITCADFloatingCard alloc] initWithFrame:NSZeroRect];
  card.accessibilityIdentifier = identifier;
  card.wantsLayer = YES;
  card.layer.shadowColor = NSColor.blackColor.CGColor;
  card.layer.shadowOpacity = 0.15;
  card.layer.shadowRadius = 12;
  card.layer.shadowOffset = CGSizeMake(0, -3);
  NSView* body = [[MITCADInspectorDocumentView alloc] initWithFrame:NSZeroRect];
  card.body = body;
  if (@available(macOS 26.0, *)) {
    NSGlassEffectView* glass = [[NSGlassEffectView alloc] initWithFrame:NSZeroRect];
    glass.style = NSGlassEffectViewStyleRegular;
    glass.cornerRadius = 14;
    glass.contentView = body;
    fill(glass, card);
    return card;
  }
  NSVisualEffectView* material = [[NSVisualEffectView alloc] initWithFrame:NSZeroRect];
  material.material = NSVisualEffectMaterialPopover;
  material.blendingMode = NSVisualEffectBlendingModeWithinWindow;
  material.state = NSVisualEffectStateFollowsWindowActiveState;
  material.wantsLayer = YES;
  material.layer.cornerRadius = 14;
  material.layer.masksToBounds = YES;
  fill(material, card);
  fill(body, material);
  return card;
}

void setShortcut(NSMenuItem* item, const QString& shortcut) {
  setKeyEquivalent(item, QKeySequence::fromString(shortcut, QKeySequence::PortableText));
}

int searchScore(const NativeMacAction& action, const QString& query) {
  const QString name = action.label.toCaseFolded();
  if (query.isEmpty()) return 5;
  if (name.startsWith(query)) return 0;
  for (const QString& word : name.split(QLatin1Char(' '), Qt::SkipEmptyParts)) {
    if (word.startsWith(query)) return 1;
  }
  if (name.contains(query)) return 2;
  for (const QString& keyword : action.keywords) {
    if (keyword.toCaseFolded().startsWith(query)) return 3;
  }
  int offset = 0;
  for (const QChar character : query) {
    offset = int(name.indexOf(character, offset));
    if (offset < 0) return -1;
    ++offset;
  }
  return 4;
}

NSString* const sidebarID = @"mitcad.sidebar";
NSString* const inspectorID = @"mitcad.inspector";
NSString* const historyID = @"mitcad.history";
NSString* const projectID = @"mitcad.project";
NSString* const projectToolbarMigrationKey = @"com.mitcad.toolbar.floating.v2.projectStatusInstalled";
NSString* const createID = @"mitcad.create";
NSString* const modifyID = @"mitcad.modify";
NSString* const searchID = @"mitcad.search";
NSString* const constraintsID = @"mitcad.constraints";

} // namespace

struct NativeMacWindowState {
  QPointer<QWidget> viewport;
  std::unique_ptr<QWindow> foreignWindow;
  NativeMacWindowCallbacks callbacks;
  QString error;
  QList<NativeMacAction> actions;
  QList<QList<QKeySequence>> actionShortcuts;
  QHash<QString, int> actionIndex;
  quint64 shortcutDispatchCount = 0;
  NSCache<NSString*, NSImage*>* __strong actionImages = [[NSCache alloc] init];
  qint64 imagePaletteKey = 0;
  QList<NativeMacTreeItem> treeItems;
  QHash<QString, int> treeIndex;
  QList<NativeMacHistoryItem> historyItems;
  QList<int> searchResults;
  QList<NativeMacHistoryItem> timelineResults;
  QSet<QString> timelineMatches;
  bool timelineSearch = false;
  NSPoint savedHistoryScroll = NSZeroPoint;
  NSPoint savedBrowserScroll = NSZeroPoint;
  int historyMarker = 0;
  QStringList selectedHistory;
  QString historySelectionAnchor;
  bool historyMarkerDragging = false;
  int historyDragPosition = 0;
  int historyDropBoundary = -1;
  QList<CGFloat> historyBoundaries;
  QString projectText;
  QString projectTooltip;
  bool sketching = false;
  bool updatingSelection = false;
  bool closing = false;
  bool frameInitialized = false;
  bool layingOut = false;
  QSize viewportSize;
  CGFloat viewportScale = 0;
  bool historyAppearanceValid = false;
  bool historyAppearanceCanChange = false;
  bool historyAppearanceBlocked = false;
  bool historyAppearanceDragging = false;
  int historyAppearanceMarker = -1;
  QStringList historyAppearanceSelection;
  QSet<QString> historyAppearanceMatches;
  qint64 historyAppearancePaletteKey = 0;
  QPointer<QWidget> gestureReceiver;
  unsigned gestureKinds = 0;

  NSEvent* __strong lastShortcutEvent = nil;
  bool lastShortcutHandled = false;
  NSWindow* __strong window = nil;
  MITCADNativeWindowDelegate* __strong delegate = nil;
  MITCADViewportHost* __strong viewportHost = nil;
  MITCADCanvasRoot* __strong canvasRoot = nil;
  MITCADFloatingCard* __strong sidebarCard = nil;
  MITCADFloatingCard* __strong inspectorCard = nil;
  MITCADFloatingCard* __strong historyCard = nil;
  MITCADFloatingCard* __strong statusCard = nil;
  NSVisualEffectView* __strong ribbon = nil;
  NSScrollView* __strong ribbonScroll = nil;
  MITCADInspectorDocumentView* __strong ribbonGroups = nil;
  NSMutableDictionary<NSString*, MITCADCommandButton*>* __strong ribbonButtons = nil;
  NSMutableDictionary<NSString*, NSPopUpButton*>* __strong ribbonMenus = nil;
  NSMutableDictionary<NSString*, NSBox*>* __strong ribbonSeparators = nil;
  QStringList ribbonGroupOrder;
  QHash<QString, QStringList> ribbonGroupButtons;
  MITCADModelOutlineView* __strong tree = nil;
  NSMutableArray<MITCADTreeNode*>* __strong treeRoots = nil;
  NSMutableDictionary<NSString*, MITCADTreeNode*>* __strong treeNodes = nil;
  MITCADHistoryStrip* __strong history = nil;
  NSScrollView* __strong historyScroll = nil;
  NSMutableDictionary<NSString*, MITCADHistoryFeatureButton*>* __strong historyButtons = nil;
  MITCADHistoryMarkerButton* __strong historyMarkerButton = nil;
  NSArray<NSButton*>* __strong historySteps = nil;
  NSButton* __strong historyFilterButton = nil;
  NSMutableArray<NSImageView*>* __strong historyRunMarks = nil;
  NSBox* __strong historyDropLine = nil;
  NSView* __strong inspectorHost = nil;
  NSScrollView* __strong inspectorScroll = nil;
  NSView* __strong inspectorContent = nil;
  MITCADInspectorDocumentView* __strong inspectorDocument = nil;
  NSTextField* __strong status = nil;
  NSPopover* __strong searchPopover = nil;
  NSSearchField* __strong searchField = nil;
  NSTableView* __strong searchTable = nil;
  NSStackView* __strong timelineSearchControls = nil;
  NSTextField* __strong timelineSearchCount = nil;
  NSMenu* __strong mainMenu = nil;
  NSMenu* __strong previousMenu = nil;
  NSMenu* __strong previousServicesMenu = nil;
  NSMenu* __strong previousWindowsMenu = nil;
  id<NSApplicationDelegate> __strong previousAppDelegate = nil;

  explicit NativeMacWindowState(QWidget* widget, NativeMacWindowCallbacks calls)
      : viewport(widget), callbacks(std::move(calls)) {
    actionImages.countLimit = 1024;
    delegate = [[MITCADNativeWindowDelegate alloc] init];
    delegate->state = this;
    previousAppDelegate = NSApp.delegate;
    NSApp.delegate = delegate;
    buildWindow();
    embedViewport();
    rebuildMenus();
    // AppKit's menu tracking loop can consume a release without Qt's
    // popup/WindowBlocked lifecycle. Observe all menus in this process,
    // including the ribbon and context menus bridged from QMenu.
    NSNotificationCenter* notifications = NSNotificationCenter.defaultCenter;
    [notifications addObserver:delegate selector:@selector(menuTrackingChanged:)
                         name:NSMenuDidBeginTrackingNotification object:nil];
    [notifications addObserver:delegate selector:@selector(menuTrackingChanged:)
                         name:NSMenuDidEndTrackingNotification object:nil];
  }

  ~NativeMacWindowState() {
    finishHistoryMarkerDrag();
    closing = true;
    [NSNotificationCenter.defaultCenter removeObserver:delegate name:NSMenuDidBeginTrackingNotification object:nil];
    [NSNotificationCenter.defaultCenter removeObserver:delegate name:NSMenuDidEndTrackingNotification object:nil];
    delegate->state = nullptr;
    ((MITCADDocumentWindow*)window)->state = nullptr;
    viewportHost->state = nullptr;
    canvasRoot->state = nullptr;
    tree->state = nullptr;
    history->state = nullptr;
    historyMarkerButton->state = nullptr;
    for (MITCADHistoryFeatureButton* button in historyButtons.allValues) button->state = nullptr;
    [searchPopover close];
    window.delegate = nil;
    window.toolbar.delegate = nil;
    tree.delegate = nil;
    tree.dataSource = nil;
    searchTable.delegate = nil;
    searchTable.dataSource = nil;
    // Detach Qt while its foreign parent NSView still exists. Qt owns its
    // child handle, AppKit owns the parent, and neither owns the other.
    if (viewport && viewport->windowHandle()) {
      viewport->hide();
      viewport->windowHandle()->setParent(nullptr);
    }
    foreignWindow.reset();
    if (NSApp.delegate == delegate) NSApp.delegate = previousAppDelegate;
    if (NSApp.mainMenu == mainMenu) {
      NSApp.mainMenu = previousMenu;
      NSApp.servicesMenu = previousServicesMenu;
      NSApp.windowsMenu = previousWindowsMenu;
    }
    [window orderOut:nil];
    [window close];
  }

  const NativeMacAction* action(const QString& id) const {
    const auto found = actionIndex.constFind(id);
    return found == actionIndex.cend() ? nullptr : &actions[*found];
  }

  NSImage* actionImage(const NativeMacAction& item, int size = 24) {
    const qint64 paletteKey = QApplication::palette().cacheKey();
    if (imagePaletteKey != paletteKey) {
      [actionImages removeAllObjects];
      imagePaletteKey = paletteKey;
    }
    // Each cached image has one immutable logical size: ribbon and history
    // controls must not resize artwork shared with a menu or toolbar item.
    NSString* key = ns(QStringLiteral("%1/%2/%3/%4/%5/%6")
                          .arg(item.icon.cacheKey()).arg(paletteKey).arg(item.iconTemplate)
                          .arg(size).arg(item.symbol.size()).arg(item.symbol));
    NSImage* image = [actionImages objectForKey:key];
    if (!image) {
      image = [renderActionImage(item) copy];
      image.size = NSMakeSize(size, size);
      // Labels belong to the controls; one artwork may serve many commands.
      image.accessibilityDescription = nil;
      [actionImages setObject:image forKey:key];
    }
    return image;
  }

  const NativeMacTreeItem* treeItem(NSString* key) const {
    const auto found = treeIndex.constFind(qt(key));
    return found == treeIndex.cend() ? nullptr : &treeItems[*found];
  }

  bool activateBrowserSelection() {
    if (closing || nativeModalBlocked() || tree.editedRow >= 0 || tree.selectedRow < 0 || !callbacks.activate) return false;
    MITCADTreeNode* node = [tree itemAtRow:tree.selectedRow];
    if (!node) return false;
    callbacks.activate(qt(node.key));
    return true;
  }

  void invoke(const QString& id) {
    const NativeMacAction* item = action(id);
    if (item && item->enabled && callbacks.action) callbacks.action(id);
  }

  void cancelViewportPointerInteraction() {
    gestureReceiver.clear();
    gestureKinds = 0;
    if (!closing && callbacks.cancelPointerInteraction) callbacks.cancelPointerInteraction();
  }

  bool nativeModalBlocked() const {
    NSWindow* key = NSApp.keyWindow;
    return NSApp.modalWindow != nil || window.attachedSheet != nil ||
           (key != nil && (key.attachedSheet != nil || key.sheetParent != nil));
  }

  bool textEditing() const {
    if ([window.firstResponder isKindOfClass:NSTextView.class]) return true;
    // Qt editors can live inside the embedded viewport as CAD overlays.
    NSView* responder = [window.firstResponder isKindOfClass:NSView.class] ? (NSView*)window.firstResponder : nil;
    if (responder && [responder isDescendantOf:viewportHost]) {
      QWidget* focus = QApplication::focusWidget();
      return qobject_cast<QLineEdit*>(focus) || qobject_cast<QPlainTextEdit*>(focus) ||
             qobject_cast<QTextEdit*>(focus) || qobject_cast<QAbstractSpinBox*>(focus);
    }
    return false;
  }

  bool dispatchShortcutOnce(NSEvent* event) {
    if (event.type != NSEventTypeKeyDown) return false;
    // AppKit can offer one event as a key equivalent and through sendEvent:.
    // Keep both entry points for plain keys, but inspect each event once.
    if (event == lastShortcutEvent) return lastShortcutHandled;
    const bool handled = dispatchShortcut(event);
    lastShortcutEvent = event;
    lastShortcutHandled = handled;
    return handled;
  }

  bool dispatchShortcut(NSEvent* event) {
    ++shortcutDispatchCount;
    if (event.type != NSEventTypeKeyDown || event.window != window ||
        closing || nativeModalBlocked() || QApplication::activeModalWidget() || textEditing()) return false;
    const QKeySequence pressed = keySequence(event);
    if (pressed.isEmpty()) return false;
    NSResponder* focus = window.firstResponder;
    const bool browserFocus = focus == tree;
    const bool historyFocus = focus == history || [focus isKindOfClass:MITCADHistoryFeatureButton.class];
    const bool inHistory = [focus isKindOfClass:NSView.class] && [(NSView*)focus isDescendantOf:history];
    const auto combination = pressed[0];
    // AppKit offers the default button its key equivalent before keyDown:
    // reaches the outline. Browser activation must win at that stage too.
    if (browserFocus && combination.keyboardModifiers() == Qt::NoModifier &&
        (combination.key() == Qt::Key_Return || combination.key() == Qt::Key_Enter) && activateBrowserSelection()) return true;
    // Other document shortcuts still require the active key window.
    if (NSApp.keyWindow != window) return false;
    if ((browserFocus || historyFocus || inHistory) && pressed == QKeySequence(QKeySequence::Find)) {
      openSearch(true);
      return true;
    }
    if ((browserFocus || historyFocus) && combination.keyboardModifiers() == Qt::NoModifier) {
      const auto key = combination.key();
      if (key == Qt::Key_Delete || key == Qt::Key_Backspace || key == Qt::Key_F2) {
        // Focus belongs to the collection itself, never its field editor.
        if (!historyCanChange()) return true;
        if (browserFocus && tree.selectedRow >= 0) {
          MITCADTreeNode* node = [tree itemAtRow:tree.selectedRow];
          const auto* item = node ? treeItem(node.key) : nullptr;
          if (item && item->enabled) {
            if (key == Qt::Key_F2) {
              if (item->editable) [tree editColumn:0 row:tree.selectedRow withEvent:nil select:YES];
            } else if (callbacks.treeDelete) callbacks.treeDelete(item->id);
          }
        } else if (historyFocus && !selectedHistory.isEmpty()) {
          const QString uid = selectedHistory.back();
          if (key == Qt::Key_F2) { if (callbacks.historyRename) callbacks.historyRename(uid); }
          else if (callbacks.historyDelete) callbacks.historyDelete(uid);
        }
        return true;
      }
    }
    const NativeMacAction* match = nullptr;
    for (int index = 0; index < actions.size(); ++index) {
      const auto& candidate = actions[index];
      if (!candidate.enabled || candidate.id == QStringLiteral("file.close")) continue;
      for (const QKeySequence& shortcut : actionShortcuts[index]) {
        if (shortcut != pressed) continue;
        if (match && match->id != candidate.id) return false;
        match = &candidate;
      }
    }
    if (!match) return false;
    invoke(match->id);
    return true;
  }

  bool historyCanChange() const {
    return !closing && !nativeModalBlocked() && callbacks.historyCanChange && callbacks.historyCanChange();
  }

  int historyIndex(const QString& key) const {
    for (int index = 0; index < historyItems.size(); ++index) {
      if (historyItems[index].id == key) return index;
    }
    return -1;
  }

  bool historyShownBoundary(int index) const {
    return index <= 0 || index >= historyItems.size() ||
           !historyItems[index].filtered || !historyItems[index - 1].filtered;
  }

  int historyStep(int direction) const {
    for (int index = historyMarker + direction; index > 0 && index < historyItems.size(); index += direction) {
      if (historyShownBoundary(index)) return index;
    }
    return direction < 0 ? 0 : int(historyItems.size());
  }

  void layoutHistory() {
    if (!history) return;
    historyBoundaries.clear();
    historyBoundaries.reserve(historyItems.size() + 1);
    CGFloat x = historyInset;
    int run = 0;
    for (int index = 0; index < historyItems.size(); ++index) {
      const bool hidden = historyItems[index].filtered;
      MITCADHistoryFeatureButton* button = historyButtons[ns(historyItems[index].id)];
      button.hidden = hidden;
      historyBoundaries.append(x);
      if (!hidden) {
        button.frame = NSMakeRect(x + 7, 12, 30, 32);
        x += historySlot;
        continue;
      }
      if (index > 0 && historyItems[index - 1].filtered) {
        // Internal boundaries share the run's end; pointer interaction
        // considers only its two outer boundaries.
        continue;
      }
      NSImageView* mark = NSUInteger(run) < historyRunMarks.count ? historyRunMarks[NSUInteger(run)] : nil;
      if (!mark) {
        mark = [[NSImageView alloc] initWithFrame:NSZeroRect];
        mark.image = symbol(QStringLiteral("ellipsis"));
        mark.contentTintColor = NSColor.tertiaryLabelColor;
        [history addSubview:mark positioned:NSWindowBelow relativeTo:historyMarkerButton];
        [historyRunMarks addObject:mark];
      }
      int count = 1;
      while (index + count < historyItems.size() && historyItems[index + count].filtered) ++count;
      mark.hidden = NO;
      mark.frame = NSMakeRect(x + 2, 21, historyRunSlot - 4, 14);
      mark.toolTip = ns(QStringLiteral("%1 feature(s) hidden by the timeline filter").arg(count));
      mark.accessibilityLabel = mark.toolTip;
      mark.accessibilityIdentifier = ns(QStringLiteral("native.history.filteredRun.%1").arg(index));
      ++run;
      x += historyRunSlot;
    }
    historyBoundaries.append(x);
    for (NSUInteger index = NSUInteger(run); index < historyRunMarks.count; ++index) historyRunMarks[index].hidden = YES;
    const CGFloat width = std::max(historyScroll.contentView.bounds.size.width, x + historyInset);
    history.frame = NSMakeRect(0, 0, width, historyStripHeight);
    historyMarkerButton.frame = NSMakeRect(historyBoundaries[historyMarker] - 7, 3, 14, 48);
    historyDropLine.frame = NSMakeRect(historyBoundaries[std::clamp(historyDropBoundary, 0, int(historyItems.size()))] - 1, 8, 2, 42);
    historyDropLine.hidden = historyDropBoundary < 0;
  }

  void updateHistoryAppearance() {
    const bool canChange = historyCanChange();
    const bool blocked = nativeModalBlocked();
    const qint64 paletteKey = QApplication::palette().cacheKey();
    if (historyAppearanceValid && historyAppearanceCanChange == canChange &&
        historyAppearanceBlocked == blocked && historyAppearanceDragging == historyMarkerDragging &&
        historyAppearanceMarker == historyMarker && historyAppearanceSelection == selectedHistory &&
        historyAppearanceMatches == timelineMatches && historyAppearancePaletteKey == paletteKey) return;
    historyAppearanceValid = true;
    historyAppearanceCanChange = canChange;
    historyAppearanceBlocked = blocked;
    historyAppearanceDragging = historyMarkerDragging;
    historyAppearanceMarker = historyMarker;
    historyAppearanceSelection = selectedHistory;
    historyAppearanceMatches = timelineMatches;
    historyAppearancePaletteKey = paletteKey;
    for (int index = 0; index < historyItems.size(); ++index) {
      const auto& item = historyItems[index];
      MITCADHistoryFeatureButton* button = historyButtons[ns(item.id)];
      button.enabled = item.enabled && !item.filtered && !blocked;
      button.state = selectedHistory.contains(item.id) ? NSControlStateValueOn : NSControlStateValueOff;
      button.wantsLayer = YES;
      button.layer.cornerRadius = 5;
      button.layer.borderWidth = timelineMatches.contains(item.id) ? 2 : 0;
      button.layer.borderColor = NSColor.controlAccentColor.CGColor;
      button.alphaValue = item.suppressed || index >= historyMarker ? 0.42 : 1.0;
      QStringList description = {item.title};
      if (item.suppressed) description.append(QStringLiteral("Suppressed"));
      else if (index >= historyMarker) description.append(QStringLiteral("Rolled back"));
      if (item.status == QStringLiteral("error")) description.append(QStringLiteral("Error"));
      else if (item.status == QStringLiteral("warning")) description.append(QStringLiteral("Warning"));
      button.accessibilityLabel = ns(description.join(QStringLiteral(", ")));
    }
    // An accepted drag remains active during recomputes, so its final target
    // and shared undo merge cannot be lost when a snapshot refreshes.
    historyMarkerButton.enabled = historyMarkerDragging || (canChange && !historyItems.isEmpty());
    const int hiddenAfter = int(std::count_if(historyItems.cbegin() + historyMarker, historyItems.cend(),
                                            [](const auto& item) { return item.filtered; }));
    historyMarkerButton.toolTip = ns(QStringLiteral("History marker: %1 of %2\nDrag to roll back or forward%3")
                                      .arg(historyMarker).arg(historyItems.size())
                                      .arg(hiddenAfter > 0 ? QStringLiteral("\n%1 hidden feature(s) after the marker").arg(hiddenAfter) : QString()));
    historyFilterButton.enabled = !blocked && !historyMarkerDragging;
    for (NSButton* button in historySteps) {
      const bool beginning = button.tag < 2;
      button.enabled = canChange && !historyMarkerDragging &&
                       (beginning ? historyMarker > 0 : historyMarker < historyItems.size());
    }
    layoutHistory();
  }

  void setHistorySelected(const QStringList& keys, bool scroll = true) {
    QStringList valid;
    for (const auto& item : historyItems) if (!item.filtered && keys.contains(item.id)) valid.append(item.id);
    const bool changed = valid != selectedHistory;
    selectedHistory = valid;
    if (!selectedHistory.contains(historySelectionAnchor)) {
      historySelectionAnchor = selectedHistory.isEmpty() ? QString() : selectedHistory.front();
    }
    updateHistoryAppearance();
    if (scroll && changed && !historyMarkerDragging && !selectedHistory.isEmpty()) {
      MITCADHistoryFeatureButton* button = historyButtons[ns(selectedHistory.back())];
      [button scrollRectToVisible:NSInsetRect(button.bounds, -8, -4)];
    }
  }

  void selectHistoryFeature(const QString& key, NSEventModifierFlags modifiers) {
    const int index = historyIndex(key);
    if (index < 0 || nativeModalBlocked() || historyItems[index].filtered || !historyItems[index].enabled) return;
    QStringList keys = selectedHistory;
    const int anchor = historyIndex(historySelectionAnchor);
    if ((modifiers & NSEventModifierFlagShift) && anchor >= 0) {
      if (!(modifiers & NSEventModifierFlagCommand)) keys.clear();
      for (int at = std::min(anchor, index); at <= std::max(anchor, index); ++at) {
        if (!historyItems[at].filtered && historyItems[at].enabled && !keys.contains(historyItems[at].id)) keys.append(historyItems[at].id);
      }
    } else if (modifiers & NSEventModifierFlagCommand) {
      if (keys.contains(key)) keys.removeAll(key);
      else keys.append(key);
      historySelectionAnchor = key;
    } else {
      keys = {key};
      historySelectionAnchor = key;
    }
    setHistorySelected(keys);
    [window makeFirstResponder:historyButtons[ns(key)]];
    if (!updatingSelection && callbacks.historySelection) callbacks.historySelection(selectedHistory);
  }

  void activateHistoryFeature(const QString& key) {
    const int index = historyIndex(key);
    if (index < 0 || historyItems[index].filtered || !historyItems[index].enabled || !historyCanChange()) return;
    selectHistoryFeature(key, 0);
    if (callbacks.historyActivated) callbacks.historyActivated(key);
  }

  void navigateHistory(const QString& key, int direction, NSEventModifierFlags modifiers) {
    if (historyItems.isEmpty()) return;
    const int current = std::max(0, historyIndex(key));
    int target = direction == -2 ? 0 : direction == 2 ? int(historyItems.size()) - 1 : current + direction;
    const int step = direction < 0 && direction != -2 ? -1 : direction == 2 ? -1 : 1;
    while (target >= 0 && target < historyItems.size()) {
      if (!historyItems[target].filtered) { selectHistoryFeature(historyItems[target].id, modifiers); return; }
      target += step;
    }
  }

  int historyBoundary(NSPoint windowPoint, bool autoScroll = false) {
    if (autoScroll) {
      NSClipView* clip = historyScroll.contentView;
      const NSPoint inClip = [clip convertPoint:windowPoint fromView:nil];
      NSPoint origin = clip.bounds.origin;
      if (inClip.x < NSMinX(clip.bounds) + 16) origin.x -= historySlot / 2;
      else if (inClip.x > NSMaxX(clip.bounds) - 16) origin.x += historySlot / 2;
      origin.x = std::clamp(origin.x, CGFloat(0), std::max(CGFloat(0), history.bounds.size.width - clip.bounds.size.width));
      [clip scrollToPoint:origin];
      [historyScroll reflectScrolledClipView:clip];
    }
    const NSPoint local = [history convertPoint:windowPoint fromView:nil];
    int nearest = 0;
    CGFloat distance = std::abs(local.x - historyBoundaries.front());
    for (int index = 1; index < historyBoundaries.size(); ++index) {
      if (!historyShownBoundary(index)) continue;
      const CGFloat candidate = std::abs(local.x - historyBoundaries[index]);
      if (candidate < distance) { nearest = index; distance = candidate; }
    }
    return nearest;
  }

  bool beginHistoryMarkerDrag() {
    if (historyMarkerDragging || !historyCanChange() || historyItems.isEmpty() ||
        !callbacks.historyMarkerDragStarted || !callbacks.historyMarkerDragged || !callbacks.historyMarkerDragFinished) return false;
    historyMarkerDragging = true;
    historyDragPosition = historyMarker;
    callbacks.historyMarkerDragStarted();
    updateHistoryAppearance();
    return true;
  }

  void dragHistoryMarker(int position) {
    if (!historyMarkerDragging) return;
    if (closing || nativeModalBlocked() || historyCard.hidden) { finishHistoryMarkerDrag(); return; }
    position = std::clamp(position, 0, int(historyItems.size()));
    if (position == historyDragPosition) return;
    historyDragPosition = historyMarker = position;
    updateHistoryAppearance();
    callbacks.historyMarkerDragged(position);
  }

  void finishHistoryMarkerDrag() {
    if (!historyMarkerDragging) return;
    const int position = historyDragPosition;
    historyMarkerDragging = false;
    if (callbacks.historyMarkerDragFinished) callbacks.historyMarkerDragFinished(position);
    updateHistoryAppearance();
  }

  void requestHistoryMarker(int position) {
    if (!historyCanChange() || historyMarkerDragging || !callbacks.historyMarkerChanged) return;
    position = std::clamp(position, 0, int(historyItems.size()));
    if (position != historyMarker) callbacks.historyMarkerChanged(position);
  }

  void reorderHistoryFeature(const QString& key, int destination) {
    const int from = historyIndex(key);
    if (!historyCanChange() || !callbacks.historyReordered || from < 0 || historyItems.isEmpty() || historyItems[from].filtered) return;
    destination = std::clamp(destination, 0, int(historyItems.size()) - 1);
    if (destination != from) callbacks.historyReordered(key, destination);
  }

  void rebuildHistory(const QList<NativeMacHistoryItem>& items, int marker) {
    if (items.size() != historyItems.size()) historyAppearanceValid = false;
    else for (int index = 0; index < items.size(); ++index) {
      const auto& a = items[index]; const auto& b = historyItems[index];
      if (a.id != b.id || a.title != b.title || a.status != b.status ||
          a.suppressed != b.suppressed || a.enabled != b.enabled || a.filtered != b.filtered) {
        historyAppearanceValid = false;
        break;
      }
    }
    historyItems = items;
    historyMarker = historyMarkerDragging ? std::clamp(historyDragPosition, 0, int(items.size())) :
                    marker < 0 ? int(items.size()) : std::clamp(marker, 0, int(items.size()));
    if (historyMarkerDragging) historyDragPosition = historyMarker;
    NSMutableSet<NSString*>* present = [NSMutableSet set];
    for (const auto& item : items) {
      NSString* key = ns(item.id);
      [present addObject:key];
      MITCADHistoryFeatureButton* button = historyButtons[key];
      if (!button) {
        button = [[MITCADHistoryFeatureButton alloc] initWithFrame:NSZeroRect];
        button->state = this;
        button.featureKey = key;
        button.title = @"";
        button.bezelStyle = NSBezelStyleRounded;
        [button setButtonType:NSButtonTypePushOnPushOff];
        button.imagePosition = NSImageOnly;
        button.imageScaling = NSImageScaleProportionallyDown;
        button.target = delegate;
        button.action = @selector(clickHistoryFeature:);
        button.accessibilityIdentifier = ns(QStringLiteral("native.history.") + item.id);
        [history addSubview:button positioned:NSWindowBelow relativeTo:historyMarkerButton];
        historyButtons[key] = button;
      }
      NativeMacAction artwork;
      artwork.label = item.title;
      artwork.icon = item.icon;
      if (item.icon.isNull()) artwork.symbol = item.symbol;
      NSImage* image = actionImage(artwork, 22);
      if (button.image != image) button.image = image;
      if (qt(button.toolTip) != item.detail) button.toolTip = ns(item.detail);
      // A native system badge communicates failure/warning without replacing
      // or tinting the shared coloured CAD artwork.
      if (qt(button.statusKey) != item.status) {
        button.statusKey = ns(item.status);
        for (NSView* badge in button.subviews.copy) {
          if ([badge.identifier isEqualToString:@"historyStatusBadge"]) [badge removeFromSuperview];
        }
        if (item.status == QStringLiteral("error") || item.status == QStringLiteral("warning")) {
          NSImageView* badge = [[NSImageView alloc] initWithFrame:NSMakeRect(20, 0, 10, 10)];
          badge.image = symbol(item.status == QStringLiteral("error") ? QStringLiteral("xmark.circle.fill") : QStringLiteral("exclamationmark.triangle.fill"));
          badge.contentTintColor = item.status == QStringLiteral("error") ? NSColor.systemRedColor : NSColor.systemOrangeColor;
          badge.identifier = @"historyStatusBadge";
          badge.accessibilityElement = NO;
          [button addSubview:badge];
        }
      }
    }
    for (NSString* key in historyButtons.allKeys) {
      if ([present containsObject:key]) continue;
      MITCADHistoryFeatureButton* button = historyButtons[key];
      button->state = nullptr;
      [button removeFromSuperview];
      [historyButtons removeObjectForKey:key];
    }
    setHistorySelected(selectedHistory, false);
  }

  void buildWindow() {
    window = [[MITCADDocumentWindow alloc] initWithContentRect:NSMakeRect(0, 0, 1280, 800)
                                       styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                                                 NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
                                         backing:NSBackingStoreBuffered defer:NO];
    ((MITCADDocumentWindow*)window)->state = this;
    window.releasedWhenClosed = NO;
    window.delegate = delegate;
    window.title = @"Untitled";
    window.identifier = @"native.documentWindow";
    // The document and frame have their own recovery. An older system UI
    // snapshot must not overwrite the current frame autosave at launch.
    window.restorable = NO;
    window.minSize = NSMakeSize(820, 560);
    window.tabbingMode = NSWindowTabbingModeDisallowed;
    window.collectionBehavior = NSWindowCollectionBehaviorFullScreenPrimary;
    window.toolbarStyle = NSWindowToolbarStyleUnifiedCompact;

    const NSSize contentSize = [window contentRectForFrameRect:window.frame].size;
    canvasRoot = [[MITCADCanvasRoot alloc] initWithFrame:NSMakeRect(0, 0, contentSize.width, contentSize.height)];
    canvasRoot->state = this;
    window.contentView = canvasRoot;
    viewportHost = [[MITCADViewportHost alloc] initWithFrame:canvasRoot.bounds];
    viewportHost->state = this;
    viewportHost.accessibilityLabel = @"3D canvas";
    viewportHost.accessibilityIdentifier = @"native.viewportHost";
    [canvasRoot addSubview:viewportHost];

    ribbon = [[NSVisualEffectView alloc] initWithFrame:NSZeroRect];
    ribbon.material = NSVisualEffectMaterialHeaderView;
    ribbon.blendingMode = NSVisualEffectBlendingModeWithinWindow;
    ribbon.state = NSVisualEffectStateFollowsWindowActiveState;
    ribbon.accessibilityIdentifier = @"native.ribbon";
    ribbonScroll = scroll(nil);
    ribbonScroll.hasVerticalScroller = NO;
    ribbonScroll.hasHorizontalScroller = YES;
    ribbonScroll.scrollerStyle = NSScrollerStyleOverlay;
    fill(ribbonScroll, ribbon);
    [canvasRoot addSubview:ribbon];

    sidebarCard = floatingCard(@"native.sidebar");
    [canvasRoot addSubview:sidebarCard];
    NSTextField* browserTitle = label(@"Browser", [NSFont systemFontOfSize:13 weight:NSFontWeightSemibold]);
    [sidebarCard.body addSubview:browserTitle];
    NSButton* browserClose = [NSButton buttonWithImage:symbol(QStringLiteral("xmark"), @"Hide Browser")
                                             target:delegate action:@selector(toggleSidebar:)];
    browserClose.bordered = NO;
    browserClose.translatesAutoresizingMaskIntoConstraints = NO;
    browserClose.accessibilityLabel = @"Hide Browser";
    [sidebarCard.body addSubview:browserClose];
    [NSLayoutConstraint activateConstraints:@[
      [browserTitle.leadingAnchor constraintEqualToAnchor:sidebarCard.body.leadingAnchor constant:14],
      [browserTitle.topAnchor constraintEqualToAnchor:sidebarCard.body.topAnchor constant:12],
      [browserClose.trailingAnchor constraintEqualToAnchor:sidebarCard.body.trailingAnchor constant:-10],
      [browserClose.centerYAnchor constraintEqualToAnchor:browserTitle.centerYAnchor],
      [browserClose.widthAnchor constraintEqualToConstant:20],
      [browserClose.heightAnchor constraintEqualToConstant:20]
    ]];

    tree = [[MITCADModelOutlineView alloc] initWithFrame:NSZeroRect];
    tree->state = this;
    NSTableColumn* column = [[NSTableColumn alloc] initWithIdentifier:@"model"];
    [tree addTableColumn:column];
    tree.outlineTableColumn = column;
    tree.headerView = nil;
    tree.style = NSTableViewStyleSourceList;
    tree.rowHeight = 30;
    tree.backgroundColor = NSColor.clearColor;
    tree.usesAlternatingRowBackgroundColors = NO;
    tree.allowsMultipleSelection = YES;
    tree.allowsEmptySelection = YES;
    tree.dataSource = delegate;
    tree.delegate = delegate;
    tree.target = delegate;
    tree.doubleAction = @selector(activateTree:);
    tree.autosaveExpandedItems = YES;
    tree.autosaveName = @"MitcadNativeModelTree";
    tree.accessibilityLabel = @"Model";
    tree.accessibilityIdentifier = @"native.modelTree";
    tree.menu = [[NSMenu alloc] initWithTitle:@"Model"];
    NSMenuItem* edit = [[NSMenuItem alloc] initWithTitle:@"Edit" action:@selector(activateTree:) keyEquivalent:@""];
    edit.target = delegate;
    [tree.menu addItem:edit];
    NSMenuItem* rename = [[NSMenuItem alloc] initWithTitle:@"Rename…" action:@selector(renameTree:) keyEquivalent:@""];
    rename.target = delegate;
    [tree.menu addItem:rename];
    NSMenuItem* visibility = [[NSMenuItem alloc] initWithTitle:@"Show / Hide" action:@selector(toggleTreeVisibility:) keyEquivalent:@""];
    visibility.target = delegate;
    [tree.menu addItem:visibility];
    NSScrollView* treeScroll = scroll(tree);
    treeScroll.backgroundColor = NSColor.clearColor;
    [sidebarCard.body addSubview:treeScroll];
    [NSLayoutConstraint activateConstraints:@[
      [treeScroll.leadingAnchor constraintEqualToAnchor:sidebarCard.body.leadingAnchor constant:6],
      [treeScroll.trailingAnchor constraintEqualToAnchor:sidebarCard.body.trailingAnchor constant:-6],
      [treeScroll.topAnchor constraintEqualToAnchor:browserTitle.bottomAnchor constant:8],
      [treeScroll.bottomAnchor constraintEqualToAnchor:sidebarCard.body.bottomAnchor constant:-8]
    ]];
    treeRoots = [NSMutableArray array];
    treeNodes = [NSMutableDictionary dictionary];

    status = label(@"");
    status.textColor = NSColor.secondaryLabelColor;
    status.lineBreakMode = NSLineBreakByTruncatingTail;
    status.accessibilityIdentifier = @"native.status";
    statusCard = floatingCard(@"native.statusCard");
    statusCard.hidden = YES;
    fill(status, statusCard.body, 8);
    [canvasRoot addSubview:statusCard];

    historyCard = floatingCard(@"native.historyCard");
    historyCard.hidden = YES;
    [canvasRoot addSubview:historyCard];
    NSView* historyView = historyCard.body;
    history = [[MITCADHistoryStrip alloc] initWithFrame:NSMakeRect(0, 0, 28, historyStripHeight)];
    history->state = this;
    [history registerForDraggedTypes:@[@"com.mitcad.feature-history"]];
    history.accessibilityLabel = @"Feature history";
    history.accessibilityIdentifier = @"native.historyStrip";
    historyButtons = [NSMutableDictionary dictionary];
    historyRunMarks = [NSMutableArray array];
    historyScroll = scroll(history);
    historyScroll.hasVerticalScroller = NO;
    historyScroll.hasHorizontalScroller = YES;
    historyScroll.scrollerStyle = NSScrollerStyleOverlay;
    historyScroll.drawsBackground = NO;
    historyScroll.contentView.drawsBackground = NO;
    historyScroll.borderType = NSNoBorder;
    [historyView addSubview:historyScroll];
    historyMarkerButton = [[MITCADHistoryMarkerButton alloc] initWithFrame:NSMakeRect(7, 3, 14, 48)];
    historyMarkerButton->state = this;
    historyMarkerButton.title = @"";
    historyMarkerButton.bordered = NO;
    historyMarkerButton.accessibilityLabel = @"History marker";
    historyMarkerButton.accessibilityIdentifier = @"native.historyMarker";
    NSImageView* markerHandle = [[NSImageView alloc] initWithFrame:NSMakeRect(2, 0, 10, 10)];
    markerHandle.image = symbol(QStringLiteral("arrowtriangle.down.fill"));
    markerHandle.contentTintColor = NSColor.controlAccentColor;
    markerHandle.accessibilityElement = NO;
    [historyMarkerButton addSubview:markerHandle];
    NSBox* markerLine = [[NSBox alloc] initWithFrame:NSMakeRect(6, 9, 2, 38)];
    markerLine.boxType = NSBoxCustom;
    markerLine.borderWidth = 0;
    markerLine.fillColor = NSColor.controlAccentColor;
    markerLine.accessibilityElement = NO;
    [historyMarkerButton addSubview:markerLine];
    [history addSubview:historyMarkerButton];
    historyDropLine = [[NSBox alloc] initWithFrame:NSZeroRect];
    historyDropLine.boxType = NSBoxCustom;
    historyDropLine.borderWidth = 0;
    historyDropLine.fillColor = NSColor.controlAccentColor;
    historyDropLine.hidden = YES;
    historyDropLine.accessibilityElement = NO;
    [history addSubview:historyDropLine];
    NSStackView* playback = [[NSStackView alloc] init];
    playback.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    playback.spacing = 2;
    playback.translatesAutoresizingMaskIntoConstraints = NO;
    NSMutableArray<NSButton*>* steps = [NSMutableArray array];
    const QStringList symbols = {QStringLiteral("backward.end.fill"), QStringLiteral("chevron.backward"),
                                 QStringLiteral("chevron.forward"), QStringLiteral("forward.end.fill")};
    const QStringList names = {QStringLiteral("start"), QStringLiteral("back"), QStringLiteral("forward"), QStringLiteral("end")};
    const QStringList labels = {QStringLiteral("Roll back to start"), QStringLiteral("Previous feature"),
                                QStringLiteral("Next feature"), QStringLiteral("Roll forward to end")};
    for (int index = 0; index < names.size(); ++index) {
      NSButton* button = [NSButton buttonWithImage:symbol(symbols[index], ns(labels[index])) target:delegate action:@selector(stepHistory:)];
      button.bordered = NO;
      button.tag = index;
      button.translatesAutoresizingMaskIntoConstraints = NO;
      button.accessibilityIdentifier = ns(QStringLiteral("native.history.") + names[index]);
      button.accessibilityLabel = ns(labels[index]);
      button.toolTip = ns(labels[index]);
      [button.widthAnchor constraintEqualToConstant:24].active = YES;
      [button.heightAnchor constraintEqualToConstant:28].active = YES;
      [playback addArrangedSubview:button];
      [steps addObject:button];
    }
    historySteps = steps;
    historyFilterButton = [NSButton buttonWithImage:symbol(QStringLiteral("line.3.horizontal.decrease"))
                                           target:delegate action:@selector(openHistoryFilter:)];
    historyFilterButton.bordered = NO;
    [historyFilterButton setButtonType:NSButtonTypePushOnPushOff];
    historyFilterButton.translatesAutoresizingMaskIntoConstraints = NO;
    historyFilterButton.accessibilityIdentifier = @"native.history.filter";
    historyFilterButton.accessibilityLabel = @"Timeline Filter";
    [historyFilterButton.widthAnchor constraintEqualToConstant:28].active = YES;
    [historyFilterButton.heightAnchor constraintEqualToConstant:28].active = YES;
    [playback addArrangedSubview:historyFilterButton];
    [historyView addSubview:playback];
    [NSLayoutConstraint activateConstraints:@[
      [playback.leadingAnchor constraintEqualToAnchor:historyView.leadingAnchor constant:12],
      [playback.centerYAnchor constraintEqualToAnchor:historyView.centerYAnchor],
      [historyScroll.leadingAnchor constraintEqualToAnchor:playback.trailingAnchor constant:10],
      [historyScroll.trailingAnchor constraintEqualToAnchor:historyView.trailingAnchor constant:-8],
      [historyScroll.topAnchor constraintEqualToAnchor:historyView.topAnchor constant:10],
      [historyScroll.bottomAnchor constraintEqualToAnchor:historyView.bottomAnchor constant:-10]
    ]];
    updateHistoryAppearance();
    inspectorCard = floatingCard(@"native.inspectorCard");
    inspectorCard.hidden = YES;
    [canvasRoot addSubview:inspectorCard];
    inspectorHost = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 280, 700)];
    inspectorHost.accessibilityLabel = @"Inspector";
    inspectorHost.accessibilityIdentifier = @"native.inspector";
    inspectorScroll = scroll(nil);
    fill(inspectorScroll, inspectorHost);
    fill(inspectorHost, inspectorCard.body);
    layoutOverlays();
  }

  void layoutOverlays() {
    if (layingOut || !inspectorCard) return;
    layingOut = true;
    constexpr CGFloat ribbonHeight = 76;
    constexpr CGFloat margin = 12;
    constexpr CGFloat gap = 8;
    const CGFloat width = canvasRoot.bounds.size.width;
    const CGFloat height = canvasRoot.bounds.size.height;
    ribbon.frame = NSMakeRect(0, 0, width, ribbonHeight);
    layoutRibbon(width);
    viewportHost.frame = NSMakeRect(0, ribbonHeight, width, std::max(CGFloat(1), height - ribbonHeight));
    const CGFloat canvasHeight = viewportHost.bounds.size.height;
    constexpr CGFloat historyHeight = 76;
    historyCard.frame = NSMakeRect(margin, height - margin - historyHeight,
                                   std::max(CGFloat(1), width - margin * 2), historyHeight);
    [historyCard layoutSubtreeIfNeeded];
    layoutHistory();
    const CGFloat floor = historyCard.hidden ? height - margin : historyCard.frame.origin.y - gap;
    const CGFloat top = ribbonHeight + margin;
    const CGFloat leftHeight = std::min(std::max(CGFloat(100), tree.numberOfRows * tree.rowHeight + 50),
                                       std::max(CGFloat(100), std::min(canvasHeight * 0.55, floor - top)));
    sidebarCard.frame = NSMakeRect(margin, top, 260, leftHeight);
    const CGFloat rightTop = ribbonHeight + 190;
    const CGFloat available = std::max(CGFloat(1), floor - rightTop);
    inspectorCard.frame = NSMakeRect(width - margin - 300, rightTop, 300, available);
    [inspectorHost layoutSubtreeIfNeeded];
    const CGFloat formHeight = inspectorContent ? inspectorContent.fittingSize.height : 180;
    inspectorCard.frame = NSMakeRect(width - margin - 300, rightTop, 300,
                                     std::min(available, std::max(CGFloat(160), formHeight + 4)));
    const CGFloat statusWidth = std::min(std::max(CGFloat(120), status.intrinsicContentSize.width + 24),
                                         std::max(CGFloat(120), width - 2 * (300 + margin + gap)));
    statusCard.frame = NSMakeRect((width - statusWidth) / 2, floor - 34, statusWidth, 34);
    if (callbacks.viewportOverlays) {
      std::vector<QRect> covered;
      for (NSView* card in @[sidebarCard, inspectorCard, historyCard]) {
        if (card.hidden) continue;
        const NSRect rect = card.frame;
        covered.emplace_back(int(std::lround(rect.origin.x)), int(std::lround(rect.origin.y - ribbonHeight)),
                             int(std::lround(rect.size.width)), int(std::lround(rect.size.height)));
      }
      const int bottom = historyCard.hidden ? 0 : int(std::lround(height - floor));
      callbacks.viewportOverlays(QMargins(0, 0, 0, bottom), covered);
    }
    syncViewport();
    layingOut = false;
  }

  void setCardVisible(NSView* card, bool shown) {
    if (card.hidden == !shown) return;
    if (card == historyCard && !shown) finishHistoryMarkerDrag();
    if (!shown) {
      NSResponder* responder = window.firstResponder;
      NSView* editingView = [responder isKindOfClass:NSView.class] ? (NSView*)responder : nil;
      if ([responder isKindOfClass:NSTextView.class] && ((NSTextView*)responder).fieldEditor) {
        id control = ((NSTextView*)responder).delegate;
        if ([control isKindOfClass:NSView.class]) editingView = (NSView*)control;
      }
      if ([editingView isDescendantOf:card] && viewport) {
        NSView* canvas = (__bridge NSView*)reinterpret_cast<void*>(viewport->winId());
        if (![window makeFirstResponder:canvas]) return;
      }
    }
    card.hidden = !shown;
    layoutOverlays();
  }

  void embedViewport() {
    if (!viewport || viewport->parentWidget()) {
      error = QStringLiteral("The native Mac canvas requires a standalone QWidget.");
    } else {
      viewport->setAttribute(Qt::WA_NativeWindow);
      (void)viewport->winId();
      foreignWindow.reset(QWindow::fromWinId(WId((__bridge void*)viewportHost)));
      if (!foreignWindow || !viewport->windowHandle()) {
        error = QStringLiteral("Qt could not wrap the native Mac canvas host.");
      } else {
        viewport->windowHandle()->setParent(foreignWindow.get());
        // A Qt child window owns its existing OpenGL framebuffer; AppKit
        // only positions its NSView. No private Qt platform API is used.
        viewport->show();
        syncViewport();
        NSView* embedded = (__bridge NSView*)reinterpret_cast<void*>(viewport->winId());
        if (![embedded isDescendantOf:viewportHost]) {
          viewport->hide();
          viewport->windowHandle()->setParent(nullptr);
          foreignWindow.reset();
          error = QStringLiteral("Qt did not attach the CAD view to its native Mac host.");
        }
      }
    }
    if (!error.isEmpty()) {
      qCritical().noquote() << "Native Mac canvas:" << error;
      NSTextField* failure = label(ns(error));
      failure.lineBreakMode = NSLineBreakByWordWrapping;
      failure.textColor = NSColor.systemRedColor;
      fill(failure, viewportHost, 24);
    }
  }

  void syncViewport() {
    if (!viewport || !foreignWindow || !viewport->windowHandle()) return;
    const NSSize size = viewportHost.bounds.size;
    const int width = std::max(1, int(std::lround(size.width)));
    const int height = std::max(1, int(std::lround(size.height)));
    const QSize dimensions(width, height);
    const CGFloat scale = viewportHost.window.backingScaleFactor;
    if (viewportSize == dimensions && viewportScale == scale) return;
    if (viewportSize != dimensions) {
      viewport->resize(width, height);
      viewport->windowHandle()->setGeometry(0, 0, width, height);
      viewportSize = dimensions;
    }
    viewportScale = scale;
    viewport->update();
  }

  bool dispatchGesture(NSEvent* event) {
    const NSEventType type = event.type;
    if (type != NSEventTypeMagnify && type != NSEventTypeRotate && type != NSEventTypeSmartMagnify) return false;
    if (event.window != window) return false;
    if (closing || nativeModalBlocked() || !viewport || !viewport->isVisible() || !viewport->isEnabled()) {
      gestureReceiver.clear();
      gestureKinds = 0;
      return true;
    }
    NSView* embedded = (__bridge NSView*)reinterpret_cast<void*>(viewport->internalWinId());
    if (!embedded || ![embedded isDescendantOf:viewportHost]) return false;
    NSPoint point = event.locationInWindow;
    if (!std::isfinite(point.x) || !std::isfinite(point.y)) point = [window convertPointFromScreen:NSEvent.mouseLocation];
    const NSPoint nativeLocal = [embedded convertPoint:point fromView:nil];
    if (!std::isfinite(nativeLocal.x) || !std::isfinite(nativeLocal.y)) return bool(gestureReceiver);
    // NSView coordinates are logical points. OcctViewer applies its backing
    // scale when handling the Qt gesture, just as it does for wheel events.
    const QPointF local(nativeLocal.x, embedded.flipped ? nativeLocal.y : embedded.bounds.size.height - nativeLocal.y);
    const auto receiverAtPoint = [&]() -> QWidget* {
      NSView* parent = canvasRoot.superview;
      const NSPoint hitPoint = parent != nil ? [parent convertPoint:point fromView:nil] : point;
      NSView* hit = [canvasRoot hitTest:hitPoint];
      if (!hit || (hit != viewportHost && ![hit isDescendantOf:viewportHost])) return nullptr;
      QWidget* receiver = viewport->childAt(local);
      if (!receiver) receiver = viewport;
      // Qt's CAD overlays may be under the pointer; their GL parent owns
      // navigation. Native floating cards have already failed the hit test.
      while (receiver && !qobject_cast<QOpenGLWidget*>(receiver)) {
        if (receiver == viewport) return nullptr;
        receiver = receiver->parentWidget();
      }
      return receiver && receiver->isVisible() && receiver->isEnabled() ? receiver : nullptr;
    };
    const auto send = [&](QWidget* receiver, Qt::NativeGestureType gesture, qreal value) {
      if (!receiver) return false;
      const QPointF receiverLocal = receiver->mapFrom(viewport, local);
      QNativeGestureEvent translated(gesture, QPointingDevice::primaryPointingDevice(), 2,
                                     receiverLocal, local, viewport->mapToGlobal(local), value, QPointF());
      translated.setTimestamp(static_cast<ulong>(event.timestamp * 1000));
      QApplication::sendEvent(receiver, &translated);
      return true;
    };
    if (type == NSEventTypeSmartMagnify) return send(receiverAtPoint(), Qt::SmartZoomNativeGesture, 1.0);

    const unsigned kind = type == NSEventTypeMagnify ? 1u : 2u;
    const NSEventPhase phase = event.phase;
    const bool began = phase & NSEventPhaseBegan;
    const bool cancelled = phase & NSEventPhaseCancelled;
    const bool ended = cancelled || (phase & NSEventPhaseEnded);
    const qreal value = type == NSEventTypeMagnify ? event.magnification : -event.rotation;
    if (phase == NSEventPhaseNone) {
      QWidget* receiver = receiverAtPoint();
      return receiver && (!std::isfinite(value) || send(receiver, type == NSEventTypeMagnify ? Qt::ZoomNativeGesture : Qt::RotateNativeGesture, value));
    }
    // A second begin of the same kind starts a fresh sequence. Magnification
    // and rotation can interleave and retain the same original receiver.
    if (began && (gestureKinds & kind)) { gestureReceiver.clear(); gestureKinds = 0; }
    if (!gestureKinds && !ended) gestureReceiver = receiverAtPoint();
    if (!ended) gestureKinds |= kind;
    const bool captured = bool(gestureReceiver);
    if (began) send(gestureReceiver, Qt::BeginNativeGesture, 0.0);
    else if (ended) send(gestureReceiver, Qt::EndNativeGesture, 0.0);
    else if (std::isfinite(value)) send(gestureReceiver, type == NSEventTypeMagnify ? Qt::ZoomNativeGesture : Qt::RotateNativeGesture, value);
    if (ended) {
      gestureKinds = cancelled ? 0 : gestureKinds & ~kind;
      if (!gestureKinds) gestureReceiver.clear();
    }
    // Consume captured events so Qt's Cocoa view cannot deliver them again.
    // A sequence begun on a native card remains AppKit-only until its end.
    return captured;
  }

  NSMenuItem* menuAction(const NativeMacAction& item, bool image = false) {
    if (item.separator) return NSMenuItem.separatorItem;
    NSMenuItem* menuItem = [[NSMenuItem alloc] initWithTitle:ns(item.label) action:@selector(performAction:) keyEquivalent:@""];
    menuItem.target = delegate;
    menuItem.representedObject = ns(item.id);
    menuItem.enabled = item.enabled;
    menuItem.state = item.checked ? NSControlStateValueOn : NSControlStateValueOff;
    menuItem.toolTip = ns(item.tooltip);
    setShortcut(menuItem, item.shortcut);
    if (image) {
      menuItem.image = actionImage(item);
      if (@available(macOS 27.0, *)) menuItem.preferredImageVisibility = NSMenuItemImageVisibilityVisible;
    }
    return menuItem;
  }

  void addResponderItem(NSMenu* menu, NSString* title, SEL selector, NSString* key) {
    NSMenuItem* item = [[NSMenuItem alloc] initWithTitle:title action:selector keyEquivalent:key];
    [menu addItem:item];
  }

  void rebuildMenus() {
    if (!mainMenu) {
      previousMenu = NSApp.mainMenu;
      previousServicesMenu = NSApp.servicesMenu;
      previousWindowsMenu = NSApp.windowsMenu;
    }
    mainMenu = [[NSMenu alloc] initWithTitle:@"Main Menu"];
    NSMenu* appMenu = [[NSMenu alloc] initWithTitle:@"Mitcad"];
    NSMenuItem* appItem = [[NSMenuItem alloc] initWithTitle:@"Mitcad" action:nil keyEquivalent:@""];
    appItem.submenu = appMenu;
    [mainMenu addItem:appItem];
    for (const auto& item : actions) if (isAppAction(item)) [appMenu addItem:menuAction(item)];
    [appMenu addItem:NSMenuItem.separatorItem];
    NSMenu* services = [[NSMenu alloc] initWithTitle:@"Services"];
    NSMenuItem* servicesItem = [[NSMenuItem alloc] initWithTitle:@"Services" action:nil keyEquivalent:@""];
    servicesItem.submenu = services;
    [appMenu addItem:servicesItem];
    NSApp.servicesMenu = services;
    [appMenu addItem:NSMenuItem.separatorItem];
    addResponderItem(appMenu, @"Hide Mitcad", @selector(hide:), @"h");
    NSMenuItem* hideOthers = [[NSMenuItem alloc] initWithTitle:@"Hide Others" action:@selector(hideOtherApplications:) keyEquivalent:@"h"];
    hideOthers.keyEquivalentModifierMask = NSEventModifierFlagCommand | NSEventModifierFlagOption;
    [appMenu addItem:hideOthers];
    addResponderItem(appMenu, @"Show All", @selector(unhideAllApplications:), @"");
    [appMenu addItem:NSMenuItem.separatorItem];
    NSMenuItem* quit = [[NSMenuItem alloc] initWithTitle:@"Quit Mitcad" action:@selector(quitApplication:) keyEquivalent:@"q"];
    quit.target = delegate;
    [appMenu addItem:quit];

    QStringList titles = {QStringLiteral("File"), QStringLiteral("Edit"), QStringLiteral("View")};
    for (const NativeMacAction& item : actions) {
      if (isAppAction(item)) continue;
      const QString title = actionMenuPath(item).section(QLatin1Char('/'), 0, 0);
      if (title.isEmpty()) continue;
      if (!titles.contains(title)) titles.append(title);
    }
    for (const QString& title : titles) {
      NSMenu* menu = [[NSMenu alloc] initWithTitle:ns(title)];
      NSMenuItem* top = [[NSMenuItem alloc] initWithTitle:ns(title) action:nil keyEquivalent:@""];
      top.submenu = menu;
      [mainMenu addItem:top];
      QHash<QString, NSMenu*> submenus;
      submenus.insert(title, menu);
      for (const NativeMacAction& item : actions) {
        if (isAppAction(item)) continue;
        // A popup parent is represented by the submenu created from its
        // children's paths; it must not also become a no-op menu command.
        if (!item.submenu.isEmpty()) continue;
        const QString path = actionMenuPath(item);
        if (path.isEmpty()) continue;
        const QStringList parts = path.split(QLatin1Char('/'), Qt::SkipEmptyParts);
        if (parts.isEmpty() || parts.front() != title || item.id == QStringLiteral("file.close")) continue;
        QString prefix = title;
        NSMenu* target = menu;
        for (int index = 1; index < parts.size(); ++index) {
          prefix += QLatin1Char('/') + parts[index];
          if (submenus.contains(prefix)) { target = submenus.value(prefix); continue; }
          NSMenu* child = [[NSMenu alloc] initWithTitle:ns(parts[index])];
          NSMenuItem* childItem = [[NSMenuItem alloc] initWithTitle:ns(parts[index]) action:nil keyEquivalent:@""];
          childItem.submenu = child;
          [target addItem:childItem];
          target = child;
          submenus.insert(prefix, child);
        }
        [target addItem:menuAction(item)];
      }
      if (title == QStringLiteral("File")) {
        [menu addItem:NSMenuItem.separatorItem];
        NSMenuItem* close = [[NSMenuItem alloc] initWithTitle:@"Close" action:@selector(closeDocument:) keyEquivalent:@"w"];
        close.target = delegate;
        [menu addItem:close];
      } else if (title == QStringLiteral("Edit")) {
        [menu addItem:NSMenuItem.separatorItem];
        addResponderItem(menu, @"Cut", @selector(cut:), @"x");
        addResponderItem(menu, @"Copy", @selector(copy:), @"c");
        addResponderItem(menu, @"Paste", @selector(paste:), @"v");
        addResponderItem(menu, @"Select All", @selector(selectAll:), @"a");
      } else if (title == QStringLiteral("View")) {
        [menu addItem:NSMenuItem.separatorItem];
        for (auto pair : {std::pair<NSString*, SEL>(@"Show Sidebar", @selector(toggleSidebar:)),
                          std::pair<NSString*, SEL>(@"Show Inspector", @selector(toggleInspector:)),
                          std::pair<NSString*, SEL>(@"Show History", @selector(toggleHistory:))}) {
          NSMenuItem* item = [[NSMenuItem alloc] initWithTitle:pair.first action:pair.second keyEquivalent:@""];
          item.target = delegate;
          [menu addItem:item];
        }
        addResponderItem(menu, @"Enter Full Screen", @selector(toggleFullScreen:), @"f");
        menu.itemArray.lastObject.keyEquivalentModifierMask = NSEventModifierFlagCommand | NSEventModifierFlagControl;
      }
    }
    NSMenu* windowMenu = [[NSMenu alloc] initWithTitle:@"Window"];
    addResponderItem(windowMenu, @"Minimize", @selector(performMiniaturize:), @"m");
    addResponderItem(windowMenu, @"Zoom", @selector(performZoom:), @"");
    [windowMenu addItem:NSMenuItem.separatorItem];
    addResponderItem(windowMenu, @"Bring All to Front", @selector(arrangeInFront:), @"");
    NSMenuItem* windowItem = [[NSMenuItem alloc] initWithTitle:@"Window" action:nil keyEquivalent:@""];
    windowItem.submenu = windowMenu;
    [mainMenu addItem:windowItem];
    NSApp.windowsMenu = windowMenu;
    NSApp.mainMenu = mainMenu;
  }

  void installToolbar() {
    NSToolbar* toolbar = [[NSToolbar alloc] initWithIdentifier:@"com.mitcad.toolbar.floating.v2"];
    toolbar.delegate = delegate;
    toolbar.allowsUserCustomization = YES;
    toolbar.autosavesConfiguration = YES;
    toolbar.displayMode = NSToolbarDisplayModeIconOnly;
    window.toolbar = toolbar;
    // Upgrade saved layouts that predate Project once. Later absence is
    // the user's customization, including after the application restarts.
    NSUserDefaults* defaults = NSUserDefaults.standardUserDefaults;
    if (![defaults boolForKey:projectToolbarMigrationKey]) {
      bool hasProject = false;
      for (NSToolbarItem* item in toolbar.items) {
        if ([item.itemIdentifier isEqualToString:projectID]) hasProject = true;
      }
      if (!hasProject) [toolbar insertItemWithItemIdentifier:projectID
                                                   atIndex:std::min(NSUInteger(3), toolbar.items.count)];
      [defaults setBool:YES forKey:projectToolbarMigrationKey];
    }
    buildRibbon();
  }

  NSMenu* groupMenu(const QString& group) {
    NSMenu* menu = [[NSMenu alloc] initWithTitle:ns(group)];
    bool firstEntry = true;
    QString section;
    for (const NativeMacAction* item : groupActions(group)) {
      if (item->toolbarOnly) continue; // listed in its subgroup's popup (mitcad#222)
      // A separator where the section changes, as the ribbon's group menu
      // has it (Ribbon::createGroup, mitcad#579): only between entries.
      if (!firstEntry && item->section != section) [menu addItem:NSMenuItem.separatorItem];
      firstEntry = false;
      section = item->section;
      NSMenuItem* entry = menuAction(*item, true);
      if (!item->submenu.isEmpty()) {
        NSMenu* children = [[NSMenu alloc] initWithTitle:ns(item->label)];
        for (const NativeMacAction& child : actions) {
          if (child.menu == item->submenu) [children addItem:menuAction(child)];
        }
        entry.submenu = children;
      }
      [menu addItem:entry];
    }
    return menu;
  }

  QList<const NativeMacAction*> groupActions(const QString& group) const {
    QList<const NativeMacAction*> result;
    const QString workspace = sketching ? QStringLiteral("SKETCH") : QStringLiteral("SOLID");
    for (const auto& item : actions) {
      if (item.group != group || item.separator) continue;
      const bool inWorkspace = item.workspace.compare(workspace, Qt::CaseInsensitive) == 0;
      // SELECT's shared filter actions are the only ribbon extras supplied
      // without a workspace; other empty-workspace actions are menus-only.
      const bool selectExtra = item.workspace.isEmpty() && group == QStringLiteral("SELECT") &&
                               item.menu == QStringLiteral("Select");
      if (!inWorkspace && !selectExtra) continue;
      result.append(&item);
    }
    std::stable_sort(result.begin(), result.end(), [](const auto* a, const auto* b) {
      return a->toolbarOrder < b->toolbarOrder;
    });
    return result;
  }

  void buildRibbon() {
    ribbonGroups = [[MITCADInspectorDocumentView alloc] initWithFrame:NSZeroRect];
    ribbonButtons = [NSMutableDictionary dictionary];
    ribbonMenus = [NSMutableDictionary dictionary];
    ribbonSeparators = [NSMutableDictionary dictionary];
    ribbonGroupOrder.clear();
    ribbonGroupButtons.clear();
    const QStringList groups = sketching
        ? QStringList{QStringLiteral("CREATE"), QStringLiteral("MODIFY"), QStringLiteral("CONSTRAINTS"),
                      QStringLiteral("INSPECT"), QStringLiteral("INSERT"), QStringLiteral("SELECT"), QStringLiteral("FINISH SKETCH")}
        : QStringList{QStringLiteral("CREATE"), QStringLiteral("MODIFY"), QStringLiteral("SHEET METAL"), QStringLiteral("SURFACE"), QStringLiteral("ASSEMBLE"),
                      QStringLiteral("CONSTRUCT"), QStringLiteral("INSPECT"), QStringLiteral("INSERT"),
                      QStringLiteral("MAKE"), QStringLiteral("SELECT")};
    CGFloat x = ribbonInset;
    for (const QString& group : groups) {
      const auto entries = groupActions(group);
      if (entries.isEmpty()) continue;
      ribbonGroupOrder.append(group);
      QList<const NativeMacAction*> pinned;
      for (const auto* item : entries) if (item->toolbarPinned) pinned.append(item);
      QString captionText = group.toLower();
      captionText[0] = captionText[0].toUpper();
      NSPopUpButton* caption = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:YES];
      caption.bordered = NO;
      caption.font = [NSFont systemFontOfSize:11 weight:NSFontWeightMedium];
      caption.accessibilityLabel = ns(captionText);
      caption.accessibilityIdentifier = ns(QStringLiteral("native.ribbon.group.") + group.toLower());
      NSMenu* menu = groupMenu(group);
      [menu insertItem:[[NSMenuItem alloc] initWithTitle:ns(captionText) action:nil keyEquivalent:@""] atIndex:0];
      caption.menu = menu;
      const CGFloat captionWidth = caption.intrinsicContentSize.width + 12;
      const CGFloat groupWidth = ribbonGroupWidth(int(pinned.size()), captionWidth);
      const CGFloat start = x + (groupWidth - ribbonIconRowWidth(int(pinned.size()))) / 2;
      int index = 0;
      for (const auto* item : pinned) {
        MITCADCommandButton* button = [[MITCADCommandButton alloc] initWithFrame:
            NSMakeRect(start + index++ * (ribbonButtonWidth + ribbonButtonGap), 8, ribbonButtonWidth, ribbonButtonHeight)];
        button.bezelStyle = NSBezelStyleRounded;
        [button setButtonType:NSButtonTypeMomentaryPushIn];
        button.image = actionImage(*item, 18);
        button.imagePosition = NSImageOnly;
        button.imageScaling = NSImageScaleProportionallyDown;
        button.representedObject = ns(item->id);
        button.target = delegate;
        button.action = @selector(performAction:);
        button.accessibilityIdentifier = ns(QStringLiteral("native.ribbon.") + item->id);
        button.accessibilityLabel = ns(item->label);
        button.toolTip = ns(item->tooltip.isEmpty() ? item->label : item->tooltip);
        button.enabled = item->enabled;
        button.state = item->checked ? NSControlStateValueOn : NSControlStateValueOff;
        [ribbonGroups addSubview:button];
        ribbonButtons[ns(item->id)] = button;
        ribbonGroupButtons[group].append(item->id);
      }
      caption.frame = NSMakeRect(x + (groupWidth - captionWidth) / 2, 44, captionWidth, 22);
      [ribbonGroups addSubview:caption];
      ribbonMenus[ns(group)] = caption;
      x += groupWidth + ribbonGroupGap;
      if (group != groups.back()) {
        NSBox* separator = [[NSBox alloc] initWithFrame:NSMakeRect(x - ribbonGroupGap / 2, 12, 1, 46)];
        separator.boxType = NSBoxSeparator;
        [ribbonGroups addSubview:separator];
        ribbonSeparators[ns(group)] = separator;
      }
    }
    ribbonGroups.frame = NSMakeRect(0, 0, x - ribbonGroupGap + ribbonInset, 72);
    ribbonScroll.documentView = ribbonGroups;
    layoutOverlays();
  }

  void layoutRibbon(CGFloat availableWidth) {
    if (!ribbonGroups) return;
    QList<int> counts;
    QList<CGFloat> minimums;
    for (const QString& group : ribbonGroupOrder) {
      counts.append(int(ribbonGroupButtons.value(group).size()));
      minimums.append(ribbonMenus[ns(group)].intrinsicContentSize.width + 12);
    }
    const auto totalWidth = [&] {
      CGFloat total = 2 * ribbonInset + std::max(0, int(counts.size()) - 1) * ribbonGroupGap;
      for (int i = 0; i < counts.size(); ++i) total += ribbonGroupWidth(counts[i], minimums[i]);
      return total;
    };
    // Match the established ribbon: retain every group menu and collapse
    // trailing pinned icons into it when the window becomes narrow.
    for (int group = int(counts.size()) - 1; group >= 0 && totalWidth() > availableWidth; --group) {
      if (ribbonGroupOrder[group] == QStringLiteral("FINISH SKETCH")) continue;
      while (counts[group] > 0 && totalWidth() > availableWidth) --counts[group];
    }
    CGFloat x = ribbonInset;
    for (int i = 0; i < ribbonGroupOrder.size(); ++i) {
      const QString& group = ribbonGroupOrder[i];
      const CGFloat width = ribbonGroupWidth(counts[i], minimums[i]);
      const QStringList ids = ribbonGroupButtons.value(group);
      const CGFloat start = x + (width - ribbonIconRowWidth(counts[i])) / 2;
      for (int buttonIndex = 0; buttonIndex < ids.size(); ++buttonIndex) {
        MITCADCommandButton* button = ribbonButtons[ns(ids[buttonIndex])];
        button.hidden = buttonIndex >= counts[i];
        button.frame = NSMakeRect(start + buttonIndex * (ribbonButtonWidth + ribbonButtonGap), 8,
                                  ribbonButtonWidth, ribbonButtonHeight);
      }
      ribbonMenus[ns(group)].frame = NSMakeRect(x + (width - minimums[i]) / 2, 44, minimums[i], 22);
      ribbonSeparators[ns(group)].frame = NSMakeRect(x + width + ribbonGroupGap / 2, 12, 1, 46);
      x += width + ribbonGroupGap;
    }
    ribbonGroups.frame = NSMakeRect(0, 0, std::max(availableWidth, x - ribbonGroupGap + ribbonInset), 72);
  }

  void updateToolbar(bool structureChanged = false) {
    for (const auto& command : actions) {
      MITCADCommandButton* button = ribbonButtons[ns(command.id)];
      if (!button) continue;
      if (button.enabled != command.enabled) button.enabled = command.enabled;
      const NSControlStateValue state = command.checked ? NSControlStateValueOn : NSControlStateValueOff;
      if (button.state != state) button.state = state;
      NSImage* image = actionImage(command, 18);
      if (button.image != image) button.image = image;
      if (qt(button.accessibilityLabel) != command.label) button.accessibilityLabel = ns(command.label);
      const QString tooltip = command.tooltip.isEmpty() ? command.label : command.tooltip;
      if (qt(button.toolTip) != tooltip) button.toolTip = ns(tooltip);
    }
    // Menu validation reads the latest command snapshot. Replacing popup
    // menus while a user is tracking one would interrupt that interaction.
    for (NSToolbarItem* item in window.toolbar.items) {
      const QString identifier = qt(item.itemIdentifier);
      if (identifier.startsWith(QStringLiteral("command."))) {
        if (const auto* command = action(identifier.mid(8))) {
          if (item.enabled != command->enabled) item.enabled = command->enabled;
          if (qt(item.label) != command->label) item.label = ns(command->label);
          const QString tooltip = command->tooltip.isEmpty() ? command->label : command->tooltip;
          if (qt(item.toolTip) != tooltip) item.toolTip = ns(tooltip);
          NSImage* image = actionImage(*command);
          if (item.image != image) item.image = image;
        }
      } else if (structureChanged && [item isKindOfClass:NSMenuToolbarItem.class]) {
        NSString* group = [item.itemIdentifier isEqualToString:modifyID] ? @"MODIFY" :
                          ([item.itemIdentifier isEqualToString:constraintsID] ? @"CONSTRAINTS" : @"CREATE");
        ((NSMenuToolbarItem*)item).menu = groupMenu(qt(group));
      }
    }
  }

  void rebuildTree(const QList<NativeMacTreeItem>& items) {
    QStringList selected;
    NSMutableSet<NSString*>* expanded = [NSMutableSet set];
    for (NSInteger row = 0; row < tree.numberOfRows; ++row) {
      MITCADTreeNode* node = [tree itemAtRow:row];
      if ([tree isItemExpanded:node]) [expanded addObject:node.key];
      if ([tree.selectedRowIndexes containsIndex:NSUInteger(row)]) selected.append(qt(node.key));
    }
    const bool first = treeItems.isEmpty();
    treeItems = items;
    treeIndex.clear();
    treeRoots = [NSMutableArray array];
    treeNodes = [NSMutableDictionary dictionary];
    for (int index = 0; index < items.size(); ++index) {
      const auto& item = items[index];
      if (item.id.isEmpty() || treeIndex.contains(item.id)) {
        qWarning().noquote() << "Native Mac model tree: duplicate/empty key" << item.id;
        continue;
      }
      treeIndex.insert(item.id, index);
      MITCADTreeNode* node = [[MITCADTreeNode alloc] init];
      node.key = ns(item.id);
      node.children = [NSMutableArray array];
      treeNodes[node.key] = node;
    }
    for (const auto& item : items) {
      MITCADTreeNode* node = treeNodes[ns(item.id)];
      if (node == nil) continue;
      MITCADTreeNode* parent = treeNodes[ns(item.parentId)];
      // Defend a presentation error: cycles must never reach NSOutlineView.
      QSet<QString> visited{item.id};
      QString ancestor = item.parentId;
      bool cycle = false;
      while (!ancestor.isEmpty() && treeIndex.contains(ancestor)) {
        if (visited.contains(ancestor)) { cycle = true; break; }
        visited.insert(ancestor);
        ancestor = items[treeIndex.value(ancestor)].parentId;
      }
      if (parent && !cycle) [parent.children addObject:node];
      else [treeRoots addObject:node];
    }
    updatingSelection = true;
    [tree reloadData];
    if (first) [tree expandItem:nil expandChildren:YES];
    else for (NSString* key in expanded) if (treeNodes[key]) [tree expandItem:treeNodes[key]];
    setSelected(selected);
    updatingSelection = false;
  }

  void setSelected(const QStringList& keys) {
    const bool previous = updatingSelection;
    updatingSelection = true;
    NSMutableIndexSet* rows = [NSMutableIndexSet indexSet];
    for (const QString& key : keys) {
      MITCADTreeNode* node = treeNodes[ns(key)];
      if (node == nil) continue;
      NSInteger row = [tree rowForItem:node];
      if (row >= 0) [rows addIndex:NSUInteger(row)];
    }
    [tree selectRowIndexes:rows byExtendingSelection:NO];
    updatingSelection = previous;
  }

  MITCADTreeNode* contextNode() {
    const NSInteger row = tree.clickedRow >= 0 ? tree.clickedRow : tree.selectedRow;
    return row >= 0 ? [tree itemAtRow:row] : nil;
  }

  void installInspector(NSView* content) {
    if (inspectorContent == content) return;
    inspectorScroll.documentView = nil;
    [inspectorContent removeFromSuperview];
    inspectorContent = content;
    inspectorDocument = nil;
    if (content) {
      // A flipped document starts long command forms at their heading and
      // first input, rather than at the bottom of AppKit's scroll range.
      inspectorDocument = [[MITCADInspectorDocumentView alloc] initWithFrame:NSZeroRect];
      inspectorDocument.translatesAutoresizingMaskIntoConstraints = NO;
      fill(content, inspectorDocument);
      inspectorScroll.documentView = inspectorDocument;
      [NSLayoutConstraint activateConstraints:@[
        [inspectorDocument.leadingAnchor constraintEqualToAnchor:inspectorScroll.contentView.leadingAnchor],
        [inspectorDocument.topAnchor constraintEqualToAnchor:inspectorScroll.contentView.topAnchor],
        [inspectorDocument.widthAnchor constraintEqualToAnchor:inspectorScroll.contentView.widthAnchor]
      ]];
      [inspectorHost layoutSubtreeIfNeeded];
      [inspectorScroll.contentView scrollToPoint:NSZeroPoint];
      [inspectorScroll reflectScrolledClipView:inspectorScroll.contentView];
    }
    layoutOverlays();
  }

  void buildSearch() {
    if (searchPopover) return;
    searchPopover = [[NSPopover alloc] init];
    searchPopover.behavior = NSPopoverBehaviorTransient;
    searchPopover.contentSize = NSMakeSize(420, 350);
    searchPopover.animates = !qEnvironmentVariableIsSet("MITCAD_TEST_INPUT");
    searchPopover.delegate = delegate;
    NSViewController* controller = [[NSViewController alloc] init];
    NSView* view = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 420, 350)];
    controller.view = view;
    searchPopover.contentViewController = controller;
    searchField = [[NSSearchField alloc] initWithFrame:NSZeroRect];
    searchField.translatesAutoresizingMaskIntoConstraints = NO;
    searchField.placeholderString = @"Search commands";
    searchField.delegate = delegate;
    searchField.target = delegate;
    searchField.action = @selector(searchChanged:);
    searchField.sendsSearchStringImmediately = YES;
    searchField.accessibilityIdentifier = @"native.commandSearch";
    searchField.identifier = @"native.commandSearch";
    [view addSubview:searchField];
    searchTable = [[NSTableView alloc] initWithFrame:NSZeroRect];
    NSTableColumn* column = [[NSTableColumn alloc] initWithIdentifier:@"command"];
    [searchTable addTableColumn:column];
    searchTable.headerView = nil;
    searchTable.rowHeight = 30;
    searchTable.style = NSTableViewStyleFullWidth;
    searchTable.dataSource = delegate;
    searchTable.delegate = delegate;
    searchTable.target = delegate;
    searchTable.doubleAction = @selector(chooseSearch:);
    searchTable.action = @selector(chooseSearch:);
    searchTable.accessibilityLabel = @"Commands";
    NSScrollView* results = scroll(searchTable);
    [view addSubview:results];
    timelineSearchCount = label(@"");
    NSButton* filters = [NSButton buttonWithTitle:@"Filters…" target:delegate action:@selector(openTimelineSearchFilter:)];
    NSButton* previous = [NSButton buttonWithTitle:@"Previous" target:delegate action:@selector(stepTimelineSearch:)];
    NSButton* next = [NSButton buttonWithTitle:@"Next" target:delegate action:@selector(stepTimelineSearch:)];
    previous.tag = -1;
    next.tag = 1;
    timelineSearchControls = [NSStackView stackViewWithViews:@[filters, previous, next, timelineSearchCount]];
    timelineSearchControls.translatesAutoresizingMaskIntoConstraints = NO;
    timelineSearchControls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    timelineSearchControls.spacing = 8;
    timelineSearchControls.hidden = YES;
    [view addSubview:timelineSearchControls];
    [NSLayoutConstraint activateConstraints:@[
      [searchField.leadingAnchor constraintEqualToAnchor:view.leadingAnchor constant:12],
      [searchField.trailingAnchor constraintEqualToAnchor:view.trailingAnchor constant:-12],
      [searchField.topAnchor constraintEqualToAnchor:view.topAnchor constant:12],
      [results.topAnchor constraintEqualToAnchor:searchField.bottomAnchor constant:8],
      [results.leadingAnchor constraintEqualToAnchor:view.leadingAnchor constant:6],
      [results.trailingAnchor constraintEqualToAnchor:view.trailingAnchor constant:-6],
      [results.bottomAnchor constraintEqualToAnchor:view.bottomAnchor constant:-42],
      [timelineSearchControls.leadingAnchor constraintEqualToAnchor:view.leadingAnchor constant:12],
      [timelineSearchControls.trailingAnchor constraintEqualToAnchor:view.trailingAnchor constant:-12],
      [timelineSearchControls.bottomAnchor constraintEqualToAnchor:view.bottomAnchor constant:-8]
    ]];
  }

  void updateMenuState(NSMenu* menu) {
    bool commandItems = false;
    for (NSMenuItem* item in menu.itemArray) {
      if (item.target == delegate && item.action == @selector(performAction:)) commandItems = true;
      if (item.submenu) updateMenuState(item.submenu);
    }
    if (commandItems) [menu update];
  }

  void setActions(const QList<NativeMacAction>& snapshot) {
    // Availability changes do not recreate the menu bar while a user has a
    // menu or an editable control open. Validation reads the latest snapshot.
    bool structureChanged = snapshot.size() != actions.size();
    bool menuStateChanged = false;
    if (!structureChanged) {
      for (int index = 0; index < snapshot.size(); ++index) {
        const auto& a = snapshot[index]; const auto& b = actions[index];
        if (a.id != b.id || a.label != b.label || a.menu != b.menu || a.group != b.group ||
            a.workspace != b.workspace || a.shortcut != b.shortcut || a.shortcuts != b.shortcuts || a.submenu != b.submenu ||
            a.separator != b.separator || a.toolbarPinned != b.toolbarPinned || a.toolbarOnly != b.toolbarOnly || a.toolbarOrder != b.toolbarOrder ||
            a.section != b.section) {
          structureChanged = true;
          break;
        }
        if (a.enabled != b.enabled || a.checked != b.checked || a.tooltip != b.tooltip) menuStateChanged = true;
      }
    }
    actions = snapshot;
    if (structureChanged) {
      actionIndex.clear();
      for (int index = 0; index < snapshot.size(); ++index) actionIndex.insert(snapshot[index].id, index);
      actionShortcuts.clear();
      for (const auto& item : snapshot) {
        QList<QKeySequence> parsed;
        for (const QString& shortcut : item.shortcuts) parsed.append(QKeySequence(shortcut, QKeySequence::PortableText));
        actionShortcuts.append(parsed);
      }
    }
    if (structureChanged) rebuildMenus();
    if (!window.toolbar) installToolbar();
    else if (structureChanged) buildRibbon();
    updateToolbar(structureChanged);
    // Async model changes can enable a key equivalent before AppKit next
    // validates its menu. Refresh retained menus without replacing them.
    if (menuStateChanged || structureChanged) {
      updateMenuState(mainMenu);
      for (NSPopUpButton* popup in ribbonMenus.allValues) updateMenuState(popup.menu);
      for (NSToolbarItem* item in window.toolbar.items) {
        if ([item isKindOfClass:NSMenuToolbarItem.class]) updateMenuState(((NSMenuToolbarItem*)item).menu);
      }
    }
    if (historyMarkerDragging && nativeModalBlocked()) finishHistoryMarkerDrag();
    updateHistoryAppearance();
    if (searchPopover.shown) fillSearch();
  }

  void fillSearch() {
    if (timelineSearch) {
      if (callbacks.timelineSearchChanged) callbacks.timelineSearchChanged(qt(searchField.stringValue));
      [searchTable reloadData];
      return;
    }
    searchResults.clear();
    const QString query = qt(searchField.stringValue).trimmed().toCaseFolded();
    for (int index = 0; index < actions.size(); ++index) {
      if (!actions[index].separator && searchScore(actions[index], query) >= 0) searchResults.append(index);
    }
    std::stable_sort(searchResults.begin(), searchResults.end(), [&](int left, int right) {
      const auto& a = actions[left];
      const auto& b = actions[right];
      if (a.enabled != b.enabled) return a.enabled;
      const int rankA = searchScore(a, query), rankB = searchScore(b, query);
      return rankA == rankB ? a.label.localeAwareCompare(b.label) < 0 : rankA < rankB;
    });
    [searchTable reloadData];
    if (!searchResults.isEmpty() && actions[searchResults.front()].enabled) {
      [searchTable selectRowIndexes:[NSIndexSet indexSetWithIndex:0] byExtendingSelection:NO];
    }
  }

  void openSearch(bool timeline = false) {
    if (nativeModalBlocked()) return;
    buildSearch();
    if (searchPopover.shown && timelineSearch && !timeline && callbacks.timelineSearchClosed) callbacks.timelineSearchClosed();
    const bool reopening = searchPopover.shown && timelineSearch == timeline;
    timelineSearch = timeline;
    timelineSearchControls.hidden = !timeline;
    searchField.placeholderString = timeline ? @"Search timeline: name, type, in:component, is:status" : @"Search commands";
    searchField.accessibilityIdentifier = timeline ? @"native.timelineSearch" : @"native.commandSearch";
    searchField.identifier = searchField.accessibilityIdentifier;
    searchTable.accessibilityLabel = timeline ? @"Timeline search results" : @"Commands";
    if (timeline && callbacks.timelineSearchOpened) callbacks.timelineSearchOpened();
    if (timeline) setCardVisible(historyCard, true);
    if (!reopening) searchField.stringValue = @"";
    fillSearch();
    [searchPopover showRelativeToRect:NSMakeRect(window.contentView.bounds.size.width / 2 - 1,
                                               1, 2, 2)
                              ofView:window.contentView preferredEdge:NSRectEdgeMaxY];
    searchField.window.identifier = timeline ? @"native.timelineSearchWindow" : @"native.commandSearchWindow";
    [searchField.window makeKeyWindow];
    [searchField.window makeFirstResponder:searchField];
    [searchField selectText:nil];
    if (!searchPopover.shown || searchField.window == nil)
      qWarning() << "Native command search could not attach to its document window";
  }
};

NativeMacWindow::NativeMacWindow(QWidget* viewport, NativeMacWindowCallbacks callbacks)
    : m_state(std::make_unique<NativeMacWindowState>(viewport, std::move(callbacks))) {
  setInspectorText(QStringLiteral("Inspector"), QStringLiteral("Select an object to inspect it, or start a command."));
}
NativeMacWindow::~NativeMacWindow() = default;
bool NativeMacWindow::isEmbedded() const { return m_state->error.isEmpty() && bool(m_state->foreignWindow); }
bool NativeMacWindow::modalBlocked() const { return m_state->nativeModalBlocked(); }
QString NativeMacWindow::embeddingError() const { return m_state->error; }
void NativeMacWindow::show() {
  [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
  [m_state->window makeKeyAndOrderFront:nil];
  [NSApp activate];
  [m_state->window.contentView layoutSubtreeIfNeeded];
  if (!m_state->frameInitialized) {
    // The first presentation settles AppKit's toolbar and title geometry.
    // Restore afterwards, before enabling automatic frame saves.
    const BOOL restoredFrame = [m_state->window setFrameUsingName:@"MitcadNativeDocumentWindow"];
    if (!restoredFrame) {
      [m_state->window setContentSize:NSMakeSize(1280, 800)];
      [m_state->window center];
    }
    [m_state->window setFrameAutosaveName:@"MitcadNativeDocumentWindow"];
    m_state->frameInitialized = true;
    [m_state->window.contentView layoutSubtreeIfNeeded];
  }
  m_state->syncViewport();
  focusViewport();
}
void NativeMacWindow::close() { [m_state->window performClose:nil]; }
void NativeMacWindow::focusViewport() {
  if (m_state->viewport) {
    m_state->viewport->setFocus(Qt::OtherFocusReason);
    NSView* native = (__bridge NSView*)reinterpret_cast<void*>(m_state->viewport->winId());
    [m_state->window makeFirstResponder:native];
  }
}
void NativeMacWindow::setActions(const QList<NativeMacAction>& actions) { m_state->setActions(actions); }
void NativeMacWindow::setModelItems(const QList<NativeMacTreeItem>& items) {
  m_state->rebuildTree(items);
  m_state->layoutOverlays();
}
void NativeMacWindow::setSelection(const QStringList& keys) { m_state->setSelected(keys); }
void NativeMacWindow::setHistory(const QList<NativeMacHistoryItem>& items, int marker) {
  m_state->rebuildHistory(items, marker);
}
void NativeMacWindow::setDocument(const QString& title, const QString& path, bool edited) {
  m_state->window.title = ns(title.isEmpty() ? QStringLiteral("Untitled") : title);
  m_state->window.representedURL = path.isEmpty() ? nil : [NSURL fileURLWithPath:ns(path)];
  m_state->window.documentEdited = edited;
}
void NativeMacWindow::setHistorySelection(const QStringList& keys) {
  m_state->setHistorySelected(keys);
}
void NativeMacWindow::setHistoryFilter(const QString& label, bool active) {
  m_state->historyFilterButton.state = active ? NSControlStateValueOn : NSControlStateValueOff;
  m_state->historyFilterButton.toolTip = ns(QStringLiteral("Timeline Filter: %1").arg(label));
}
void NativeMacWindow::setProjectStatus(const QString& text, const QString& tooltip) {
  m_state->projectText = text;
  m_state->projectTooltip = tooltip;
  // The toolbar owns its current controls. Palette samples and removed
  // items must neither receive live status nor determine item membership.
  for (NSToolbarItem* item in m_state->window.toolbar.items) {
    if (![item.itemIdentifier isEqualToString:projectID] || ![item.view isKindOfClass:NSButton.class]) continue;
    NSButton* button = (NSButton*)item.view;
    button.title = ns(text);
    button.toolTip = ns(tooltip);
    button.accessibilityLabel = ns(QStringLiteral("Project: %1").arg(text));
  }
}
void NativeMacWindow::setSketchMode(bool sketching) {
  if (m_state->sketching == sketching) return;
  m_state->sketching = sketching;
  m_state->buildRibbon();
  m_state->updateToolbar(true);
}
void NativeMacWindow::setStatus(const QString& message, bool error) {
  m_state->status.stringValue = ns(message);
  m_state->status.textColor = error ? NSColor.systemRedColor : NSColor.secondaryLabelColor;
  m_state->status.toolTip = ns(message);
  m_state->statusCard.hidden = message.isEmpty();
  m_state->layoutOverlays();
}
void NativeMacWindow::setInspectorText(const QString& title, const QString& text) {
  NSStackView* content = [[NSStackView alloc] initWithFrame:NSZeroRect];
  content.orientation = NSUserInterfaceLayoutOrientationVertical;
  content.alignment = NSLayoutAttributeLeading;
  content.spacing = 12;
  content.edgeInsets = NSEdgeInsetsMake(18, 16, 18, 16);
  NSTextField* heading = label(ns(title), [NSFont systemFontOfSize:18 weight:NSFontWeightSemibold]);
  NSTextField* message = [NSTextField wrappingLabelWithString:ns(text)];
  message.textColor = NSColor.secondaryLabelColor;
  message.selectable = YES;
  message.translatesAutoresizingMaskIntoConstraints = NO;
  [content addArrangedSubview:heading];
  [content addArrangedSubview:message];
  NSView* space = [[NSView alloc] initWithFrame:NSZeroRect];
  [content addArrangedSubview:space];
  [message.widthAnchor constraintEqualToAnchor:content.widthAnchor constant:-32].active = YES;
  m_state->installInspector(content);
}
void NativeMacWindow::setInspectorView(void* view) { m_state->installInspector((__bridge NSView*)view); }
void NativeMacWindow::showInspector(bool shown) { m_state->setCardVisible(m_state->inspectorCard, shown); }
void NativeMacWindow::showSidebar(bool shown) { m_state->setCardVisible(m_state->sidebarCard, shown); }
void NativeMacWindow::showHistory(bool shown) { m_state->setCardVisible(m_state->historyCard, shown); }
void NativeMacWindow::showCommandSearch() { m_state->openSearch(); }
void NativeMacWindow::showTimelineSearch() { m_state->openSearch(true); }
void NativeMacWindow::setTimelineSearchResults(const QString& text, const QString& count,
                                               const QList<NativeMacHistoryItem>& results, int selected) {
  auto& state = *m_state;
  state.timelineResults = results;
  state.timelineMatches.clear();
  for (const auto& item : results) state.timelineMatches.insert(item.id);
  state.updateHistoryAppearance();
  if (!state.timelineSearch) return;
  if (qt(state.searchField.stringValue) != text) state.searchField.stringValue = ns(text);
  state.timelineSearchCount.stringValue = ns(count);
  [state.searchTable reloadData];
  if (selected >= 0 && selected < results.size()) {
    [state.searchTable selectRowIndexes:[NSIndexSet indexSetWithIndex:NSUInteger(selected)] byExtendingSelection:NO];
    [state.searchTable scrollRowToVisible:selected];
  } else [state.searchTable deselectAll:nil];
}
void NativeMacWindow::saveTimelineSearchScroll() {
  m_state->savedHistoryScroll = m_state->historyScroll.contentView.bounds.origin;
  m_state->savedBrowserScroll = m_state->tree.enclosingScrollView.contentView.bounds.origin;
}
void NativeMacWindow::restoreTimelineSearchScroll() {
  [m_state->historyScroll.contentView scrollToPoint:m_state->savedHistoryScroll];
  [m_state->historyScroll reflectScrolledClipView:m_state->historyScroll.contentView];
  NSScrollView* browser = m_state->tree.enclosingScrollView;
  [browser.contentView scrollToPoint:m_state->savedBrowserScroll];
  [browser reflectScrolledClipView:browser.contentView];
}
void* NativeMacWindow::nativeWindow() const { return (__bridge void*)m_state->window; }
void* NativeMacWindow::nativeInspectorHost() const { return (__bridge void*)m_state->inspectorHost; }

bool nativeWindowRefreshContract(void* nativeWindow, QString& error) {
  MITCADDocumentWindow* window = (__bridge MITCADDocumentWindow*)nativeWindow;
  if (![window isKindOfClass:MITCADDocumentWindow.class] || !window->state) {
    error = QStringLiteral("Native refresh contract requires a document window");
    return false;
  }
  auto& state = *window->state;
  const QList<NativeMacAction> snapshot = state.actions;
  NSArray<NSToolbarItemIdentifier>* allowed = [window.toolbar.delegate toolbarAllowedItemIdentifiers:window.toolbar];
  for (const auto& action : snapshot) {
    NSString* identifier = ns(QStringLiteral("command.") + action.id);
    if (!action.submenu.isEmpty() && [allowed containsObject:identifier]) {
      error = QStringLiteral("Native toolbar palette offered a popup without a standalone menu control");
      return false;
    }
  }
  state.setActions(snapshot);
  MITCADCommandButton* button = state.ribbonButtons.allValues.firstObject;
  if (!button || !button.image) {
    error = QStringLiteral("Native refresh contract requires a ribbon image");
    return false;
  }
  NSImage* image = button.image;
  NSView* ribbon = state.ribbonGroups;
  NSMenu* mainMenu = state.mainMenu;
  NSMutableArray<NSMenu*>* toolbarMenus = [NSMutableArray array];
  for (NSToolbarItem* item in window.toolbar.items) {
    if ([item isKindOfClass:NSMenuToolbarItem.class]) [toolbarMenus addObject:((NSMenuToolbarItem*)item).menu];
  }
  state.setActions(snapshot);
  if (button.image != image || state.ribbonGroups != ribbon || state.mainMenu != mainMenu) {
    error = QStringLiteral("Identical native actions replaced ribbon artwork or menus");
    return false;
  }
  NSUInteger index = 0;
  for (NSToolbarItem* item in window.toolbar.items) {
    if (![item isKindOfClass:NSMenuToolbarItem.class]) continue;
    if (index >= toolbarMenus.count || ((NSMenuToolbarItem*)item).menu != toolbarMenus[index++]) {
      error = QStringLiteral("Identical native actions replaced a toolbar popup menu");
      return false;
    }
  }
  QList<NativeMacAction> changed = snapshot;
  const int commandIndex = state.actionIndex.value(qt(button.representedObject), -1);
  if (commandIndex < 0) {
    error = QStringLiteral("Native refresh contract ribbon command is missing");
    return false;
  }
  auto& command = changed[commandIndex];
  command.enabled = !command.enabled;
  command.checked = !command.checked;
  command.tooltip = QStringLiteral("Native refresh contract tooltip");
  state.setActions(changed);
  const bool updated = button.image == image && state.ribbonGroups == ribbon &&
                       button.enabled == command.enabled &&
                       button.state == (command.checked ? NSControlStateValueOn : NSControlStateValueOff) &&
                       qt(button.toolTip) == command.tooltip;
  state.setActions(snapshot);
  if (!updated) {
    error = QStringLiteral("Native refresh did not update command state while retaining its image");
    return false;
  }
  NSMenuItem* redoItem = nil;
  for (NSMenuItem* top in state.mainMenu.itemArray) {
    if (![top.title isEqualToString:@"Edit"]) continue;
    for (NSMenuItem* item in top.submenu.itemArray) {
      if ([item.representedObject isEqual:@"edit.redo"]) redoItem = item;
    }
  }
  const int redoIndex = state.actionIndex.value(QStringLiteral("edit.redo"), -1);
  if (!redoItem || redoIndex < 0) {
    error = QStringLiteral("Native refresh contract requires the retained Redo menu item");
    return false;
  }
  changed = snapshot;
  for (bool enabled : {true, false}) {
    changed[redoIndex].enabled = enabled;
    state.setActions(changed);
    // Read the stored state before any test-side validation or key event.
    const bool menuUpdated = redoItem.enabled == enabled && state.mainMenu == mainMenu;
    if (!menuUpdated) {
      state.setActions(snapshot);
      error = QStringLiteral("Native refresh left the retained Redo key equivalent in a stale enabled state");
      return false;
    }
  }
  state.setActions(snapshot);
  // Offer the same unmatched key through both AppKit entry points. Count
  // dispatch attempts, including rejected keys, rather than action effects.
  NSString* characters = [NSString stringWithFormat:@"%C", unichar(NSF35FunctionKey)];
  NSEvent* event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
                             modifierFlags:NSEventModifierFlagCommand | NSEventModifierFlagControl |
                                           NSEventModifierFlagOption | NSEventModifierFlagShift
                                 timestamp:NSProcessInfo.processInfo.systemUptime
                              windowNumber:window.windowNumber context:nil characters:characters
               charactersIgnoringModifiers:characters isARepeat:NO keyCode:0];
  const quint64 before = state.shortcutDispatchCount;
  [window sendEvent:event];
  [window performKeyEquivalent:event];
  if (state.shortcutDispatchCount != before + 1) {
    error = QStringLiteral("One native key event reached shortcut dispatch more than once");
    return false;
  }
  return true;
}

} // namespace mitcad::mac

using namespace mitcad::mac;

@implementation MITCADDocumentWindow
- (BOOL)performKeyEquivalent:(NSEvent*)event {
  if (state && state->dispatchShortcutOnce(event)) return YES;
  return [super performKeyEquivalent:event];
}
- (void)sendEvent:(NSEvent*)event {
  if (state && (state->dispatchGesture(event) || state->dispatchShortcutOnce(event))) return;
  [super sendEvent:event];
}
- (BOOL)dispatchConfirmation {
  if (!state || state->nativeModalBlocked()) return NO;
  if ([self.firstResponder isKindOfClass:NSTextView.class] && ((NSTextView*)self.firstResponder).editable) return NO;
  return state->callbacks.confirm && state->callbacks.confirm();
}
- (BOOL)dispatchCancellation {
  if (!state || state->nativeModalBlocked()) return NO;
  if ([self.firstResponder isKindOfClass:NSTextView.class] && ((NSTextView*)self.firstResponder).editable) return NO;
  return state->callbacks.cancel && state->callbacks.cancel();
}
- (BOOL)dispatchKeyboardEvent:(NSEvent*)event {
  // AppKit sends unhandled keys up the responder chain. Text delegates and
  // table row editing get first refusal before the document's CAD actions.
  const NSEventModifierFlags modifiers = event.modifierFlags &
      (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagShift);
  const unichar key = event.charactersIgnoringModifiers.length ? [event.charactersIgnoringModifiers characterAtIndex:0] : 0;
  if (modifiers == 0 && (key == NSCarriageReturnCharacter || key == NSEnterCharacter)) return [self dispatchConfirmation];
  if (modifiers == 0 && key == 0x1b) return [self dispatchCancellation];
  return NO;
}
- (void)keyDown:(NSEvent*)event {
  if ([self dispatchKeyboardEvent:event]) return;
  [super keyDown:event];
}
- (void)insertNewline:(id)sender {
  if (![self dispatchConfirmation]) [self.nextResponder tryToPerform:_cmd with:sender];
}
- (void)cancelOperation:(id)sender {
  if (![self dispatchCancellation]) [self.nextResponder tryToPerform:_cmd with:sender];
}
@end

@implementation MITCADViewportHost
- (BOOL)isFlipped { return YES; }
- (void)setFrameSize:(NSSize)size {
  [super setFrameSize:size];
  if (state) state->syncViewport();
}
- (void)viewDidChangeBackingProperties {
  [super viewDidChangeBackingProperties];
  if (state) state->syncViewport();
}
@end

@implementation MITCADCanvasRoot
- (BOOL)isFlipped { return YES; }
- (void)layout {
  [super layout];
  if (state) state->layoutOverlays();
}
- (void)setFrameSize:(NSSize)size {
  [super setFrameSize:size];
  if (state) state->layoutOverlays();
}
@end

@implementation MITCADModelOutlineView
- (void)keyDown:(NSEvent*)event {
  if (self.editedRow < 0 && [(MITCADDocumentWindow*)self.window dispatchKeyboardEvent:event]) return;
  [super keyDown:event];
}
- (void)cancelOperation:(id)sender {
  if (self.editedRow < 0 && [(MITCADDocumentWindow*)self.window dispatchCancellation]) return;
  [self.nextResponder tryToPerform:_cmd with:sender];
}
- (NSMenu*)menuForEvent:(NSEvent*)event {
  if (!state || !state->callbacks.treeContextMenu) return [super menuForEvent:event];
  const NSInteger row = [self rowAtPoint:[self convertPoint:event.locationInWindow fromView:nil]];
  MITCADTreeNode* node = row >= 0 ? [self itemAtRow:row] : nil;
  if (node && ![self.selectedRowIndexes containsIndex:NSUInteger(row)])
    [self selectRowIndexes:[NSIndexSet indexSetWithIndex:NSUInteger(row)] byExtendingSelection:NO];
  state->callbacks.treeContextMenu(node ? qt(node.key) : QString(), screenPoint(event));
  return nil;
}
@end

@implementation MITCADHistoryStrip
- (BOOL)isFlipped { return YES; }
- (BOOL)isOpaque { return NO; }
- (BOOL)acceptsFirstResponder { return YES; }
- (void)mouseDown:(NSEvent*)event {
  (void)event;
  if (!state || state->nativeModalBlocked()) return;
  [self.window makeFirstResponder:self];
  state->setHistorySelected({});
  if (state->callbacks.historySelection) state->callbacks.historySelection({});
}
- (NSDragOperation)draggingEntered:(id<NSDraggingInfo>)info { return [self draggingUpdated:info]; }
- (NSDragOperation)draggingUpdated:(id<NSDraggingInfo>)info {
  if (!state || !state->historyCanChange() || ![info.draggingSource isKindOfClass:MITCADHistoryFeatureButton.class] ||
      ((MITCADHistoryFeatureButton*)info.draggingSource)->state != state) return NSDragOperationNone;
  const QString key = qt([info.draggingPasteboard stringForType:@"com.mitcad.feature-history"]);
  if (state->historyIndex(key) < 0) return NSDragOperationNone;
  state->historyDropBoundary = state->historyBoundary(info.draggingLocation, true);
  state->layoutHistory();
  return NSDragOperationMove;
}
- (void)draggingExited:(id<NSDraggingInfo>)info {
  (void)info;
  if (state) { state->historyDropBoundary = -1; state->layoutHistory(); }
}
- (BOOL)prepareForDragOperation:(id<NSDraggingInfo>)info {
  return [self draggingUpdated:info] == NSDragOperationMove;
}
- (BOOL)performDragOperation:(id<NSDraggingInfo>)info {
  if ([self draggingUpdated:info] != NSDragOperationMove) return NO;
  const QString key = qt([info.draggingPasteboard stringForType:@"com.mitcad.feature-history"]);
  const int from = state->historyIndex(key);
  const int boundary = state->historyDropBoundary;
  state->historyDropBoundary = -1;
  state->layoutHistory();
  state->reorderHistoryFeature(key, boundary - (boundary > from ? 1 : 0));
  return YES;
}
- (void)concludeDragOperation:(id<NSDraggingInfo>)info {
  (void)info;
  if (state) { state->historyDropBoundary = -1; state->layoutHistory(); }
}
- (void)keyDown:(NSEvent*)event {
  if (!state) return;
  if (!state->historyItems.isEmpty() && event.charactersIgnoringModifiers.length) {
    const unichar key = [event.charactersIgnoringModifiers characterAtIndex:0];
    if (key == NSLeftArrowFunctionKey || key == NSRightArrowFunctionKey || key == NSHomeFunctionKey || key == NSEndFunctionKey) {
      const QString current = state->selectedHistory.isEmpty() ? state->historyItems.front().id : state->selectedHistory.back();
      state->navigateHistory(current, key == NSHomeFunctionKey ? -2 : key == NSEndFunctionKey ? 2 : key == NSLeftArrowFunctionKey ? -1 : 1,
                             event.modifierFlags);
      return;
    }
  }
  if ([(MITCADDocumentWindow*)self.window dispatchKeyboardEvent:event]) return;
  [super keyDown:event];
}
@end

@implementation MITCADHistoryFeatureButton
- (BOOL)isFlipped { return YES; }
- (BOOL)acceptsFirstResponder { return self.enabled; }
- (NSView*)hitTest:(NSPoint)point {
  return NSPointInRect([self convertPoint:point fromView:self.superview], self.bounds) ? self : nil;
}
- (void)selectHistoryFeature {
  if (state) state->selectHistoryFeature(qt(self.featureKey), 0);
}
- (void)activateHistoryFeature {
  if (state) state->activateHistoryFeature(qt(self.featureKey));
}
- (void)reorderHistoryFeatureToIndex:(NSInteger)index {
  if (state) state->reorderHistoryFeature(qt(self.featureKey), int(index));
}
- (void)mouseDown:(NSEvent*)event {
  if (!state || !self.enabled || state->nativeModalBlocked()) return;
  [self.window makeFirstResponder:self];
  if (event.clickCount > 1) { [self activateHistoryFeature]; return; }
  pressed = YES;
  pressPoint = event.locationInWindow;
  [self highlight:YES];
}
- (void)mouseDragged:(NSEvent*)event {
  if (!pressed || !state || !state->historyCanChange() || !state->callbacks.historyReordered) return;
  if (std::hypot(event.locationInWindow.x - pressPoint.x, event.locationInWindow.y - pressPoint.y) < 5) return;
  pressed = NO;
  [self highlight:NO];
  NSPasteboardItem* pasteboard = [[NSPasteboardItem alloc] init];
  [pasteboard setString:self.featureKey forType:@"com.mitcad.feature-history"];
  NSDraggingItem* item = [[NSDraggingItem alloc] initWithPasteboardWriter:pasteboard];
  [item setDraggingFrame:self.bounds contents:self.image];
  [self beginDraggingSessionWithItems:@[item] event:event source:self];
}
- (void)mouseUp:(NSEvent*)event {
  const BOOL select = pressed;
  pressed = NO;
  [self highlight:NO];
  if (select && state && NSPointInRect([self convertPoint:event.locationInWindow fromView:nil], self.bounds))
    state->selectHistoryFeature(qt(self.featureKey), event.modifierFlags);
}
- (NSDragOperation)draggingSession:(NSDraggingSession*)session sourceOperationMaskForDraggingContext:(NSDraggingContext)context {
  (void)session;
  return context == NSDraggingContextWithinApplication && state && state->historyCanChange() ? NSDragOperationMove : NSDragOperationNone;
}
- (void)draggingSession:(NSDraggingSession*)session endedAtPoint:(NSPoint)point operation:(NSDragOperation)operation {
  (void)session; (void)point; (void)operation;
  if (state) { state->historyDropBoundary = -1; state->layoutHistory(); }
}
- (BOOL)ignoreModifierKeysForDraggingSession:(NSDraggingSession*)session { (void)session; return YES; }
- (NSMenu*)menuForEvent:(NSEvent*)event {
  if (!state || state->nativeModalBlocked() || !state->callbacks.historyContextMenu) return nil;
  const QString key = qt(self.featureKey);
  if (!state->selectedHistory.contains(key)) state->selectHistoryFeature(key, 0);
  state->callbacks.historyContextMenu(key, screenPoint(event));
  return nil;
}
- (void)selectAll:(id)sender {
  (void)sender;
  if (!state || state->nativeModalBlocked()) return;
  QStringList keys;
  for (const auto& item : state->historyItems) if (!item.filtered && item.enabled) keys.append(item.id);
  state->setHistorySelected(keys);
  if (state->callbacks.historySelection) state->callbacks.historySelection(state->selectedHistory);
}
- (void)keyDown:(NSEvent*)event {
  if (!state) return;
  if (event.charactersIgnoringModifiers.length) {
    const unichar key = [event.charactersIgnoringModifiers characterAtIndex:0];
    if (key == NSLeftArrowFunctionKey || key == NSRightArrowFunctionKey || key == NSHomeFunctionKey || key == NSEndFunctionKey) {
      state->navigateHistory(qt(self.featureKey), key == NSHomeFunctionKey ? -2 : key == NSEndFunctionKey ? 2 : key == NSLeftArrowFunctionKey ? -1 : 1,
                             event.modifierFlags);
      return;
    }
    if (key == NSCarriageReturnCharacter || key == NSEnterCharacter) { [self activateHistoryFeature]; return; }
    if (key == ' ') { state->selectHistoryFeature(qt(self.featureKey), event.modifierFlags); return; }
  }
  if ([(MITCADDocumentWindow*)self.window dispatchKeyboardEvent:event]) return;
  [super keyDown:event];
}
@end

@implementation MITCADHistoryMarkerButton
- (BOOL)isFlipped { return YES; }
- (BOOL)acceptsFirstResponder { return self.enabled; }
- (NSView*)hitTest:(NSPoint)point {
  return NSPointInRect([self convertPoint:point fromView:self.superview], self.bounds) ? self : nil;
}
- (BOOL)beginHistoryMarkerDrag {
  if (!state || !state->beginHistoryMarkerDrag()) return NO;
  [self.window makeFirstResponder:self];
  return YES;
}
- (void)dragHistoryMarkerToIndex:(NSInteger)index { if (state) state->dragHistoryMarker(int(index)); }
- (void)endHistoryMarkerDrag { if (state) state->finishHistoryMarkerDrag(); }
- (void)mouseDown:(NSEvent*)event { (void)event; [self beginHistoryMarkerDrag]; }
- (void)mouseDragged:(NSEvent*)event {
  if (state && state->historyMarkerDragging) state->dragHistoryMarker(state->historyBoundary(event.locationInWindow, true));
}
- (void)mouseUp:(NSEvent*)event {
  if (!state || !state->historyMarkerDragging) return;
  state->dragHistoryMarker(state->historyBoundary(event.locationInWindow));
  [self endHistoryMarkerDrag];
}
- (NSString*)accessibilityRole { return NSAccessibilitySliderRole; }
- (id)accessibilityValue { return state ? @(state->historyMarker) : @0; }
- (id)accessibilityMinValue { return @0; }
- (id)accessibilityMaxValue { return state ? @(state->historyItems.size()) : @0; }
- (void)setAccessibilityValue:(id)value {
  if (state && [value respondsToSelector:@selector(integerValue)]) state->requestHistoryMarker(int([value integerValue]));
}
- (BOOL)accessibilityPerformIncrement {
  if (!state || !state->historyCanChange() || state->historyMarker >= state->historyItems.size()) return NO;
  state->requestHistoryMarker(state->historyStep(1));
  return YES;
}
- (BOOL)accessibilityPerformDecrement {
  if (!state || !state->historyCanChange() || state->historyMarker <= 0) return NO;
  state->requestHistoryMarker(state->historyStep(-1));
  return YES;
}
- (void)keyDown:(NSEvent*)event {
  if (!state) return;
  if (event.charactersIgnoringModifiers.length) {
    const unichar key = [event.charactersIgnoringModifiers characterAtIndex:0];
    if (key == NSLeftArrowFunctionKey || key == NSRightArrowFunctionKey || key == NSHomeFunctionKey || key == NSEndFunctionKey) {
      state->requestHistoryMarker(key == NSHomeFunctionKey ? 0 : key == NSEndFunctionKey ? int(state->historyItems.size()) :
                                  state->historyStep(key == NSLeftArrowFunctionKey ? -1 : 1));
      return;
    }
  }
  if ([(MITCADDocumentWindow*)self.window dispatchKeyboardEvent:event]) return;
  [super keyDown:event];
}
- (void)cancelOperation:(id)sender {
  if (state && state->historyMarkerDragging) { [self endHistoryMarkerDrag]; return; }
  [self.nextResponder tryToPerform:_cmd with:sender];
}
@end

@implementation MITCADNativeWindowDelegate
- (BOOL)respondsToSelector:(SEL)selector {
  return [super respondsToSelector:selector] ||
      (state && [state->previousAppDelegate respondsToSelector:selector]);
}
- (id)forwardingTargetForSelector:(SEL)selector {
  // Qt still owns activation, hover updates and secure state restoration.
  // Keep every delegate method the shell does not override in that chain.
  if (state && [state->previousAppDelegate respondsToSelector:selector]) return state->previousAppDelegate;
  return [super forwardingTargetForSelector:selector];
}
- (void)application:(NSApplication*)application openFiles:(NSArray<NSString*>*)files {
  bool handled = state && bool(state->callbacks.openFile);
  if (handled) for (NSString* file in files) state->callbacks.openFile(qt(file));
  [application replyToOpenOrPrint:handled ? NSApplicationDelegateReplySuccess : NSApplicationDelegateReplyFailure];
}
- (void)application:(NSApplication*)application openURLs:(NSArray<NSURL*>*)urls {
  (void)application;
  if (!state || !state->callbacks.openFile) return;
  for (NSURL* url in urls) if (url.fileURL) state->callbacks.openFile(qt(url.path));
}
- (BOOL)applicationShouldHandleReopen:(NSApplication*)application hasVisibleWindows:(BOOL)visible {
  // Qt updates its application state here even without an activation event.
  if (state && [state->previousAppDelegate respondsToSelector:_cmd]) {
    [state->previousAppDelegate applicationShouldHandleReopen:application hasVisibleWindows:visible];
  }
  if (!state || state->closing) return NO;
  if (state->window.miniaturized) [state->window deminiaturize:nil];
  [state->window makeKeyAndOrderFront:nil];
  return YES;
}
- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication*)application {
  (void)application;
  if (!state) return NSTerminateNow;
  state->finishHistoryMarkerDrag();
  if (!state->closing && state->callbacks.requestClose && !state->callbacks.requestClose()) return NSTerminateCancel;
  // End QApplication's sole event loop so C++ document/session destructors
  // run normally. AppKit must not call exit() before that cleanup finishes.
  if (!state->closing) [state->window close];
  QCoreApplication::quit();
  return NSTerminateCancel;
}
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication*)application {
  (void)application;
  // windowWillClose invokes the application's closed callback, preserving
  // the existing single-document quit behavior through QApplication.
  return NO;
}
- (BOOL)windowShouldClose:(NSWindow*)window {
  (void)window;
  if (!state || state->closing) return YES;
  state->finishHistoryMarkerDrag();
  return !state->callbacks.requestClose || state->callbacks.requestClose();
}
- (void)windowWillClose:(NSNotification*)notification {
  (void)notification;
  if (!state || state->closing) return;
  state->finishHistoryMarkerDrag();
  state->closing = true;
  if (state->callbacks.closed) state->callbacks.closed();
}
- (void)windowWillBeginSheet:(NSNotification*)notification {
  (void)notification;
  if (state) {
    state->finishHistoryMarkerDrag();
    state->cancelViewportPointerInteraction();
    state->updateHistoryAppearance();
  }
}
- (void)windowDidEndSheet:(NSNotification*)notification {
  (void)notification;
  if (state) state->updateHistoryAppearance();
}
- (void)windowDidResignKey:(NSNotification*)notification {
  (void)notification;
  if (state) {
    state->finishHistoryMarkerDrag();
    state->cancelViewportPointerInteraction();
  }
}
- (void)menuTrackingChanged:(NSNotification*)notification {
  (void)notification;
  if (!state || state->closing) return;
  state->finishHistoryMarkerDrag();
  state->cancelViewportPointerInteraction();
}
- (void)windowDidResize:(NSNotification*)notification {
  (void)notification;
  if (state) state->layoutOverlays();
}
- (void)windowDidEnterFullScreen:(NSNotification*)notification {
  (void)notification;
  if (state) state->syncViewport();
}
- (void)windowDidExitFullScreen:(NSNotification*)notification {
  (void)notification;
  if (state) state->syncViewport();
}
- (void)outlineViewItemDidExpand:(NSNotification*)notification {
  (void)notification;
  if (state) state->layoutOverlays();
}
- (void)outlineViewItemDidCollapse:(NSNotification*)notification {
  (void)notification;
  if (state) state->layoutOverlays();
}
- (void)performAction:(id)sender {
  if (!state) return;
  NSString* identifier = [sender isKindOfClass:NSToolbarItem.class] ? [sender itemIdentifier] : [sender representedObject];
  QString key = qt(identifier);
  if (key.startsWith(QStringLiteral("command."))) key = key.mid(8);
  if (const NativeMacAction* action = state->action(key); action && !action->submenu.isEmpty() && action->enabled) {
    if (state->nativeModalBlocked()) return;
    if ([sender isKindOfClass:NSView.class]) {
      NSMenu* menu = [[NSMenu alloc] initWithTitle:ns(action->label)];
      for (const NativeMacAction& child : state->actions) {
        if (child.menu == action->submenu) [menu addItem:state->menuAction(child)];
      }
      NSView* view = (NSView*)sender;
      [menu popUpMenuPositioningItem:nil atLocation:NSMakePoint(0, view.bounds.size.height + 4) inView:view];
    }
    return;
  }
  if ([NSApp.keyWindow.firstResponder isKindOfClass:NSTextView.class]) {
    if (key == QStringLiteral("edit.undo")) { [NSApp sendAction:@selector(undo:) to:nil from:sender]; return; }
    if (key == QStringLiteral("edit.redo")) { [NSApp sendAction:@selector(redo:) to:nil from:sender]; return; }
    if (SEL selector = editingSelector(state->action(key))) {
      [NSApp sendAction:selector to:nil from:sender];
      return;
    }
  }
  if (state->nativeModalBlocked()) return;
  // End secondary/native field editing before an action changes document
  // state. A control that rejects validation keeps its editor and focus.
  if ([NSApp.keyWindow.firstResponder isKindOfClass:NSTextView.class] &&
      ![NSApp.keyWindow makeFirstResponder:nil]) return;
  if (key == QStringLiteral("tools.search")) state->openSearch();
  else state->invoke(key);
}
- (void)toggleSidebar:(id)sender { (void)sender; if (state) state->setCardVisible(state->sidebarCard, state->sidebarCard.hidden); }
- (void)toggleInspector:(id)sender { (void)sender; if (state) state->setCardVisible(state->inspectorCard, state->inspectorCard.hidden); }
- (void)toggleHistory:(id)sender { (void)sender; if (state) state->setCardVisible(state->historyCard, state->historyCard.hidden); }
- (void)openSearch:(id)sender { (void)sender; if (state) state->openSearch(); }
- (void)closeDocument:(id)sender {
  if (!state) return;
  NSWindow* window = NSApp.keyWindow != nil ? NSApp.keyWindow : state->window;
  // A menu equivalent can make the document key before dispatching while
  // its search popover is still shown. Both windows dismiss that search.
  if (state->searchPopover.shown && (window == state->searchField.window || window == state->window)) {
    [state->searchPopover close];
    // Return keyboard routing to the document's existing responder. A
    // hidden popover's field editor must not disable plain CAD shortcuts.
    [state->window makeKeyWindow];
    return;
  }
  if ([window.identifier hasPrefix:@"native.inline."]) window = state->window;
  // Settings and other native secondary windows own Command-W while key.
  // Ending the native field editor commits values and honors validation.
  if ([window makeFirstResponder:nil]) [window performClose:sender];
}
- (void)quitApplication:(id)sender {
  (void)sender;
  if (!state) return;
  [state->window performClose:nil];
  if (state && state->closing) QCoreApplication::quit();
}
- (BOOL)validateMenuItem:(NSMenuItem*)item {
  if (!state) return NO;
  if (item.action == @selector(performAction:)) {
    const auto* action = state->action(qt(item.representedObject));
    if (!action) return NO;
    const NSControlStateValue value = action->checked ? NSControlStateValueOn : NSControlStateValueOff;
    if (item.state != value) item.state = value;
    if (qt(item.toolTip) != action->tooltip) item.toolTip = ns(action->tooltip);
    if (item.image) {
      NSImage* image = state->actionImage(*action);
      if (item.image != image) item.image = image;
    }
    // Plain CAD tool keys must leave the native field editor free to type.
    const bool textEditor = [NSApp.keyWindow.firstResponder isKindOfClass:NSTextView.class];
    if (textEditor && action->id == QStringLiteral("edit.undo")) return NSApp.keyWindow.firstResponder.undoManager.canUndo;
    if (textEditor && action->id == QStringLiteral("edit.redo")) return NSApp.keyWindow.firstResponder.undoManager.canRedo;
    if (textEditor) if (SEL selector = editingSelector(action))
      return [NSApp targetForAction:selector to:nil from:item] != nil;
    if (state->nativeModalBlocked()) return NO;
    const bool plainShortcut = item.keyEquivalent.length > 0 &&
        !(item.keyEquivalentModifierMask & (NSEventModifierFlagCommand | NSEventModifierFlagControl));
    return action->enabled && !(textEditor && plainShortcut);
  }
  if (item.action == @selector(toggleSidebar:)) item.title = state->sidebarCard.hidden ? @"Show Sidebar" : @"Hide Sidebar";
  if (item.action == @selector(toggleInspector:)) item.title = state->inspectorCard.hidden ? @"Show Inspector" : @"Hide Inspector";
  if (item.action == @selector(toggleHistory:)) item.title = state->historyCard.hidden ? @"Show History" : @"Hide History";
  if (item.action == @selector(activateTree:) || item.action == @selector(renameTree:) || item.action == @selector(toggleTreeVisibility:)) {
    MITCADTreeNode* node = state->contextNode();
    const auto* modelItem = node ? state->treeItem(node.key) : nullptr;
    if (!modelItem || !modelItem->enabled) return NO;
    if (item.action == @selector(renameTree:)) return modelItem->editable && bool(state->callbacks.rename);
    if (item.action == @selector(toggleTreeVisibility:)) {
      const bool visible = modelItem->hasVisibility ? modelItem->checked : modelItem->visible;
      item.title = visible ? @"Hide" : @"Show";
      return modelItem->canToggleVisibility &&
             (bool(state->callbacks.visibilityChanged) || bool(state->callbacks.treeVisibility));
    }
    return bool(state->callbacks.activate) && modelItem->kind != QStringLiteral("folder");
  }
  return YES;
}
- (NSArray<NSToolbarItemIdentifier>*)toolbarDefaultItemIdentifiers:(NSToolbar*)toolbar {
  (void)toolbar;
  return @[sidebarID, @"command.edit.undo", @"command.edit.redo",
           projectID, NSToolbarFlexibleSpaceItemIdentifier, searchID, historyID, inspectorID];
}
- (NSArray<NSToolbarItemIdentifier>*)toolbarAllowedItemIdentifiers:(NSToolbar*)toolbar {
  (void)toolbar;
  NSMutableArray* identifiers = [NSMutableArray arrayWithArray:@[
    sidebarID, createID, modifyID, constraintsID, searchID, historyID, inspectorID, projectID,
    NSToolbarFlexibleSpaceItemIdentifier, NSToolbarSpaceItemIdentifier
  ]];
  if (state) for (const auto& item : state->actions) {
    // Popup parents need their ribbon view to present a menu; the palette
    // offers only actions that work as standalone toolbar commands.
    if (!item.separator && !item.id.isEmpty() && item.submenu.isEmpty()) {
      [identifiers addObject:ns(QStringLiteral("command.") + item.id)];
    }
  }
  return identifiers;
}
- (NSToolbarItem*)toolbar:(NSToolbar*)toolbar itemForItemIdentifier:(NSToolbarItemIdentifier)identifier
         willBeInsertedIntoToolbar:(BOOL)inserted {
  (void)toolbar; (void)inserted;
  if (!state) return nil;
  NSString* title = nil;
  NSString* image = nil;
  SEL selector = nil;
  if ([identifier isEqualToString:projectID]) {
    NSToolbarItem* item = [[NSToolbarItem alloc] initWithItemIdentifier:identifier];
    item.label = @"Project";
    item.paletteLabel = @"Project Status";
    NSButton* button = [NSButton buttonWithTitle:ns(state->projectText) target:self action:@selector(openProjectMenu:)];
    button.translatesAutoresizingMaskIntoConstraints = NO;
    button.bordered = NO;
    button.font = [NSFont systemFontOfSize:11 weight:NSFontWeightMedium];
    button.lineBreakMode = NSLineBreakByTruncatingTail;
    button.toolTip = ns(state->projectTooltip);
    button.accessibilityIdentifier = @"native.projectIndicator";
    button.accessibilityLabel = ns(QStringLiteral("Project: %1").arg(state->projectText));
    [button setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow forOrientation:NSLayoutConstraintOrientationHorizontal];
    [button.widthAnchor constraintLessThanOrEqualToConstant:340].active = YES;
    [button.heightAnchor constraintEqualToConstant:26].active = YES;
    item.view = button;
    return item;
  }
  if ([identifier isEqualToString:sidebarID]) { title = @"Sidebar"; image = @"sidebar.left"; selector = @selector(toggleSidebar:); }
  else if ([identifier isEqualToString:inspectorID]) { title = @"Inspector"; image = @"sidebar.right"; selector = @selector(toggleInspector:); }
  else if ([identifier isEqualToString:historyID]) { title = @"History"; image = @"clock.arrow.circlepath"; selector = @selector(toggleHistory:); }
  else if ([identifier isEqualToString:searchID]) { title = @"Search Commands"; image = @"magnifyingglass"; selector = @selector(openSearch:); }
  else if ([identifier isEqualToString:createID] || [identifier isEqualToString:modifyID] || [identifier isEqualToString:constraintsID]) {
    NSMenuToolbarItem* item = [[NSMenuToolbarItem alloc] initWithItemIdentifier:identifier];
    const bool modify = [identifier isEqualToString:modifyID];
    const bool constrain = [identifier isEqualToString:constraintsID];
    item.label = constrain ? @"Constrain" : (modify ? @"Modify" : @"Create");
    item.paletteLabel = item.label;
    item.image = symbol(qt(constrain ? @"link" : (modify ? @"wrench.and.screwdriver" : @"plus")), item.label);
    item.menu = state->groupMenu(constrain ? QStringLiteral("CONSTRAINTS") : (modify ? QStringLiteral("MODIFY") : QStringLiteral("CREATE")));
    item.toolTip = item.label;
    return item;
  } else {
    const QString key = qt(identifier);
    if (!key.startsWith(QStringLiteral("command."))) return nil;
    const auto* action = state->action(key.mid(8));
    if (!action || !action->submenu.isEmpty()) return nil;
    NSToolbarItem* item = [[NSToolbarItem alloc] initWithItemIdentifier:identifier];
    item.label = ns(action->label);
    item.paletteLabel = item.label;
    item.toolTip = ns(action->tooltip.isEmpty() ? action->label : action->tooltip);
    item.image = state->actionImage(*action);
    item.target = self;
    item.action = @selector(performAction:);
    item.enabled = action->enabled;
    return item;
  }
  NSToolbarItem* item = [[NSToolbarItem alloc] initWithItemIdentifier:identifier];
  item.label = title;
  item.paletteLabel = title;
  item.toolTip = title;
  item.image = symbol(qt(image), title);
  item.target = self;
  item.action = selector;
  return item;
}
- (NSInteger)outlineView:(NSOutlineView*)outline numberOfChildrenOfItem:(id)item {
  (void)outline;
  if (!state) return 0;
  return item ? ((MITCADTreeNode*)item).children.count : state->treeRoots.count;
}
- (id)outlineView:(NSOutlineView*)outline child:(NSInteger)index ofItem:(id)item {
  (void)outline;
  return item ? ((MITCADTreeNode*)item).children[NSUInteger(index)] : state->treeRoots[NSUInteger(index)];
}
- (BOOL)outlineView:(NSOutlineView*)outline isItemExpandable:(id)item {
  (void)outline; return ((MITCADTreeNode*)item).children.count > 0;
}
- (id)outlineView:(NSOutlineView*)outline persistentObjectForItem:(id)item { (void)outline; return ((MITCADTreeNode*)item).key; }
- (id)outlineView:(NSOutlineView*)outline itemForPersistentObject:(id)object { (void)outline; return state ? state->treeNodes[object] : nil; }
- (NSView*)outlineView:(NSOutlineView*)outline viewForTableColumn:(NSTableColumn*)column item:(id)object {
  (void)column;
  MITCADTreeNode* node = object;
  const auto* item = state ? state->treeItem(node.key) : nullptr;
  if (!item) return nil;
  NSTableCellView* cell = [outline makeViewWithIdentifier:@"modelCell" owner:self];
  if (!cell) {
    cell = [[NSTableCellView alloc] initWithFrame:NSZeroRect];
    cell.identifier = @"modelCell";
    NSImageView* icon = [[NSImageView alloc] initWithFrame:NSZeroRect];
    icon.translatesAutoresizingMaskIntoConstraints = NO;
    cell.imageView = icon;
    [cell addSubview:icon];
    NSTextField* title = label(@"");
    title.delegate = self;
    title.lineBreakMode = NSLineBreakByTruncatingTail;
    cell.textField = title;
    [cell addSubview:title];
    NSButton* eye = [NSButton buttonWithImage:symbol(QStringLiteral("eye")) target:self action:@selector(toggleTreeVisibility:)];
    eye.bordered = NO;
    eye.translatesAutoresizingMaskIntoConstraints = NO;
    eye.identifier = @"visibility";
    [cell addSubview:eye];
    [NSLayoutConstraint activateConstraints:@[
      [icon.leadingAnchor constraintEqualToAnchor:cell.leadingAnchor constant:2],
      [icon.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor],
      [icon.widthAnchor constraintEqualToConstant:18],
      [icon.heightAnchor constraintEqualToConstant:18],
      [title.leadingAnchor constraintEqualToAnchor:icon.trailingAnchor constant:7],
      [title.trailingAnchor constraintEqualToAnchor:eye.leadingAnchor constant:-4],
      [title.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor],
      [eye.trailingAnchor constraintEqualToAnchor:cell.trailingAnchor constant:-4],
      [eye.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor],
      [eye.widthAnchor constraintEqualToConstant:20],
      [eye.heightAnchor constraintEqualToConstant:20]
    ]];
  }
  cell.textField.stringValue = ns(item->title);
  cell.textField.identifier = node.key;
  cell.textField.editable = item->editable && bool(state->callbacks.rename);
  cell.textField.selectable = cell.textField.editable;
  cell.textField.textColor = item->enabled ? NSColor.labelColor : NSColor.tertiaryLabelColor;
  cell.imageView.image = symbol(item->symbol.isEmpty() ? QStringLiteral("cube") : item->symbol, ns(item->title));
  const bool showsVisibility = item->canToggleVisibility || item->hasVisibility;
  const bool visible = item->hasVisibility ? item->checked : item->visible;
  for (NSView* child in cell.subviews) if ([child isKindOfClass:NSButton.class]) {
    NSButton* eye = (NSButton*)child;
    eye.identifier = node.key;
    eye.hidden = !showsVisibility;
    eye.enabled = item->enabled && item->canToggleVisibility;
    eye.image = symbol(visible ? QStringLiteral("eye") : QStringLiteral("eye.slash"));
    eye.toolTip = visible ? @"Hide" : @"Show";
    eye.accessibilityLabel = [NSString stringWithFormat:@"%@ %@", eye.toolTip, ns(item->title)];
  }
  cell.alphaValue = showsVisibility && !visible ? 0.5 : 1.0;
  cell.toolTip = ns(item->detail);
  cell.accessibilityLabel = ns(item->title);
  cell.accessibilityIdentifier = ns(QStringLiteral("native.model.") + item->id);
  return cell;
}
- (BOOL)outlineView:(NSOutlineView*)outline shouldSelectItem:(id)object {
  (void)outline;
  const auto* item = state ? state->treeItem(((MITCADTreeNode*)object).key) : nullptr;
  return item && item->enabled;
}
- (void)outlineViewSelectionDidChange:(NSNotification*)notification {
  (void)notification;
  if (!state || state->updatingSelection || !state->callbacks.selection) return;
  QStringList keys;
  NSUInteger row = state->tree.selectedRowIndexes.firstIndex;
  while (row != NSNotFound) {
    MITCADTreeNode* node = [state->tree itemAtRow:NSInteger(row)];
    if (node) keys.append(qt(node.key));
    row = [state->tree.selectedRowIndexes indexGreaterThanIndex:row];
  }
  state->callbacks.selection(keys);
}
- (void)activateTree:(id)sender {
  (void)sender;
  if (!state || !state->callbacks.activate) return;
  MITCADTreeNode* node = state->contextNode();
  if (node) state->callbacks.activate(qt(node.key));
}
- (void)renameTree:(id)sender {
  (void)sender;
  if (!state) return;
  MITCADTreeNode* node = state->contextNode();
  const auto* item = node ? state->treeItem(node.key) : nullptr;
  if (item && item->editable) [state->tree editColumn:0 row:[state->tree rowForItem:node] withEvent:nil select:YES];
}
- (void)toggleTreeVisibility:(id)sender {
  if (!state) return;
  MITCADTreeNode* node = [sender isKindOfClass:NSButton.class] ? state->treeNodes[[sender identifier]] : state->contextNode();
  const auto* item = node ? state->treeItem(node.key) : nullptr;
  if (item && item->canToggleVisibility) {
    if (state->callbacks.treeVisibility) state->callbacks.treeVisibility(item->id);
    else if (state->callbacks.visibilityChanged) state->callbacks.visibilityChanged(item->id, !(item->hasVisibility ? item->checked : item->visible));
  }
}
- (NSInteger)numberOfRowsInTableView:(NSTableView*)table {
  return state && table == state->searchTable ? (state->timelineSearch ? state->timelineResults.size() : state->searchResults.size()) : 0;
}
- (NSView*)tableView:(NSTableView*)table viewForTableColumn:(NSTableColumn*)column row:(NSInteger)row {
  (void)column;
  if (!state || table != state->searchTable || row < 0) return nil;
  if (state->timelineSearch) {
    if (row >= state->timelineResults.size()) return nil;
    const auto& result = state->timelineResults[int(row)];
    NSTextField* field = label(ns(result.title + QStringLiteral("   ") + result.detail));
    field.lineBreakMode = NSLineBreakByTruncatingTail;
    field.toolTip = ns(result.title + QStringLiteral("\n") + result.detail);
    field.accessibilityIdentifier = ns(QStringLiteral("native.timelineSearchResult.") + result.id);
    return field;
  }
  if (row >= state->searchResults.size()) return nil;
  const NativeMacAction* command = &state->actions[state->searchResults[int(row)]];
  NSTableCellView* cell = [table makeViewWithIdentifier:@"nativeListCell" owner:self];
  if (!cell) {
    cell = [[NSTableCellView alloc] initWithFrame:NSZeroRect];
    cell.identifier = @"nativeListCell";
    // NSTableCellView's outlet is non-owning. Retain the field locally until
    // the cell owns it as a subview, including under optimized ARC builds.
    NSTextField* text = label(@"");
    text.lineBreakMode = NSLineBreakByTruncatingTail;
    fill(text, cell, 6);
    cell.textField = text;
  }
  QString title = command->label;
  if (!command->shortcut.isEmpty()) title += QStringLiteral("   ") + QKeySequence(command->shortcut).toString(QKeySequence::NativeText);
  cell.textField.stringValue = ns(title);
  cell.textField.textColor = command->enabled ? NSColor.labelColor : NSColor.tertiaryLabelColor;
  cell.toolTip = ns(command->tooltip);
  cell.accessibilityIdentifier = ns(QStringLiteral("native.command.") + command->id);
  return cell;
}
- (BOOL)tableView:(NSTableView*)table shouldSelectRow:(NSInteger)row {
  if (state && table == state->searchTable && state->timelineSearch) return row >= 0 && row < state->timelineResults.size();
  return state && table == state->searchTable && row >= 0 && row < state->searchResults.size() &&
         state->actions[state->searchResults[int(row)]].enabled;
}
- (void)clickHistoryFeature:(id)sender {
  if ([sender isKindOfClass:MITCADHistoryFeatureButton.class]) [(MITCADHistoryFeatureButton*)sender selectHistoryFeature];
}
- (void)stepHistory:(id)sender {
  if (!state || ![sender isKindOfClass:NSButton.class]) return;
  const NSInteger step = [sender tag];
  state->requestHistoryMarker(step == 0 ? 0 : step == 3 ? int(state->historyItems.size()) :
                             state->historyStep(step == 1 ? -1 : 1));
}
- (void)openHistoryFilter:(id)sender {
  if (!state || state->nativeModalBlocked() || state->historyMarkerDragging || !state->callbacks.historyFilterMenu) return;
  state->callbacks.historyFilterMenu(menuPoint((NSView*)sender));
}
- (void)openProjectMenu:(id)sender {
  if (!state || state->nativeModalBlocked() || !state->callbacks.projectMenu) return;
  state->callbacks.projectMenu(menuPoint((NSView*)sender));
}
- (void)chooseSearch:(id)sender {
  (void)sender;
  if (!state) return;
  NSInteger row = state->searchTable.selectedRow;
  if (state->timelineSearch) {
    if (row >= 0 && row < state->timelineResults.size() && state->callbacks.timelineSearchSelected)
      state->callbacks.timelineSearchSelected(int(row));
    [state->searchField.window makeFirstResponder:state->searchField];
    return;
  }
  // Command search keeps its existing double-click/Return activation.
  if ([sender isKindOfClass:NSTableView.class] && NSApp.currentEvent.clickCount < 2) return;
  if (row < 0 || row >= state->searchResults.size()) return;
  const auto& command = state->actions[state->searchResults[int(row)]];
  if (!command.enabled) return;
  const QString id = command.id;
  [state->searchPopover close];
  state->invoke(id);
}
- (void)openTimelineSearchFilter:(id)sender {
  if (state && state->timelineSearch && state->callbacks.timelineSearchFilterMenu)
    state->callbacks.timelineSearchFilterMenu(menuPoint((NSView*)sender));
  if (state && state->searchPopover.shown) [state->searchField.window makeFirstResponder:state->searchField];
}
- (void)stepTimelineSearch:(id)sender {
  if (state && state->timelineSearch && state->callbacks.timelineSearchStepped)
    state->callbacks.timelineSearchStepped(int([sender tag]));
  if (state) [state->searchField.window makeFirstResponder:state->searchField];
}
- (void)popoverDidClose:(NSNotification*)notification {
  if (!state || notification.object != state->searchPopover || !state->timelineSearch) return;
  state->timelineSearch = false;
  state->timelineMatches.clear();
  state->updateHistoryAppearance();
  if (!state->closing && state->callbacks.timelineSearchClosed) state->callbacks.timelineSearchClosed();
}
- (void)searchChanged:(id)sender { (void)sender; if (state) state->fillSearch(); }
- (void)controlTextDidChange:(NSNotification*)notification {
  if (state && notification.object == state->searchField) state->fillSearch();
}
- (void)controlTextDidEndEditing:(NSNotification*)notification {
  if (!state || notification.object == state->searchField || !state->callbacks.rename) return;
  NSTextField* field = notification.object;
  const auto* item = state->treeItem(field.identifier);
  if (item && item->editable && qt(field.stringValue) != item->title) {
    const QString key = item->id;
    const QString text = qt(field.stringValue).trimmed();
    if (!text.isEmpty()) state->callbacks.rename(key, text);
    else field.stringValue = ns(item->title);
  }
}
- (BOOL)control:(NSControl*)control textView:(NSTextView*)textView doCommandBySelector:(SEL)selector {
  (void)textView;
  if (!state) return NO;
  if (control != state->searchField) {
    if (selector == @selector(cancelOperation:) && [control isKindOfClass:NSTextField.class]) {
      const auto* item = state->treeItem(control.identifier);
      if (item) {
        ((NSTextField*)control).stringValue = ns(item->title);
        [control.window makeFirstResponder:state->tree];
        return YES;
      }
    }
    return NO;
  }
  if (selector == @selector(cancelOperation:)) { [state->searchPopover close]; return YES; }
  if (state->timelineSearch && (selector == @selector(insertNewline:) || selector == @selector(moveDown:) ||
                                selector == @selector(moveUp:) || selector == @selector(insertNewlineIgnoringFieldEditor:))) {
    const bool previous = selector == @selector(moveUp:) || (NSApp.currentEvent.modifierFlags & NSEventModifierFlagShift);
    if (state->callbacks.timelineSearchStepped) state->callbacks.timelineSearchStepped(previous ? -1 : 1);
    return YES;
  }
  if (selector == @selector(insertNewline:)) { [self chooseSearch:control]; return YES; }
  if (selector == @selector(moveDown:) || selector == @selector(moveUp:)) {
    const int step = selector == @selector(moveDown:) ? 1 : -1;
    const NSInteger selected = state->searchTable.selectedRow;
    for (NSInteger row = selected + step; row >= 0 && row < state->searchResults.size(); row += step) {
      if (state->actions[state->searchResults[int(row)]].enabled) {
        [state->searchTable selectRowIndexes:[NSIndexSet indexSetWithIndex:NSUInteger(row)] byExtendingSelection:NO];
        [state->searchTable scrollRowToVisible:row];
        break;
      }
    }
    return YES;
  }
  return NO;
}
@end
