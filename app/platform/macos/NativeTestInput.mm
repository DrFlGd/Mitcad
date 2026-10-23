// SPDX-License-Identifier: MIT
#include "platform/NativeTestInput.hpp"

#import <AppKit/AppKit.h>

#include <QApplication>
#include <QComboBox>
#include <QCheckBox>
#include <QDialog>
#include <QElapsedTimer>
#include <QEvent>
#include <QEventLoop>
#include <QAction>
#include <QMenu>
#include <QLabel>
#include <QLineEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QSpinBox>
#include <QStandardItemModel>
#include <QTableView>
#include <QTreeView>
#include <QRegularExpression>
#include <QImage>
#include <QJsonArray>
#include <QJsonObject>
#include <QItemSelectionModel>
#include <QOpenGLWidget>
#include <QPainter>
#include <QPalette>
#include <QRegion>
#include <QToolButton>
#include <QVBoxLayout>
#include <QtLogging>
#include <QtMath>
#include <algorithm>
#include <cmath>
#include <functional>
#include <vector>
#include <unistd.h>

#include "platform/MacChrome.hpp"
#include "platform/NativeDialogs.hpp"
#include "platform/NativeForms.hpp"
#include "platform/NativeMacWindow.hpp"
#include "browser/DocumentSnapshot.hpp"
#include "framework/CommandSession.hpp"
#include "framework/CommandRegistry.hpp"
#include "framework/Icons.hpp"
#include "MainWindow.hpp"

// Public selectors on the same feature controls used by pointer input.
@protocol MITCADNativeHistoryFeatureTest <NSObject>
- (void)selectHistoryFeature;
- (void)activateHistoryFeature;
@end

// NSEvent exposes magnification and phase as read-only properties and has
// no public magnify factory. This in-process test double exercises the real
// document NSWindow dispatch entry without injecting system-wide input.
@interface MITCADTestMagnificationEvent : NSEvent {
@public
  NSWindow* __weak target;
  NSPoint point;
  CGFloat amount;
  NSEventPhase gesturePhase;
  NSTimeInterval eventTime;
}
@end

@implementation MITCADTestMagnificationEvent
- (NSEventType)type { return NSEventTypeMagnify; }
- (NSWindow*)window { return target; }
- (NSInteger)windowNumber { return target.windowNumber; }
- (NSPoint)locationInWindow { return point; }
- (CGFloat)magnification { return amount; }
- (NSEventPhase)phase { return gesturePhase; }
- (NSEventModifierFlags)modifierFlags { return 0; }
- (NSTimeInterval)timestamp { return eventTime; }
// The Cocoa gesture reader uses zero for the default trackpad device.
- (NSUInteger)deviceID { return 0; }
@end

namespace mitcad::mac::test {
namespace {

NSWindow* __weak focusedWindow = nil;
NSWindow* __weak keyEventWindow = nil;
NSView* __weak draggedHistoryMarker = nil;
NSPoint historyDragPoint = NSZeroPoint;

QString qt(NSString* text) { return QString::fromNSString(text != nil ? text : @""); }

NSArray<NSWindow*>* nativeWindows() {
  NSMutableOrderedSet<NSWindow*>* windows = [NSMutableOrderedSet orderedSet];
  [windows addObjectsFromArray:NSApp.orderedWindows];
  [windows addObjectsFromArray:NSApp.windows];
  if (NSApp.keyWindow) [windows addObject:NSApp.keyWindow];
  if (NSApp.modalWindow) [windows addObject:NSApp.modalWindow];
  if (void* panel = nativeFileDialogPanel()) [windows addObject:(__bridge NSWindow*)panel];
  if (focusedWindow) [windows addObject:focusedWindow];
  for (NSUInteger index = 0; index < windows.count; ++index) {
    NSWindow* sheet = windows[index].attachedSheet;
    if (sheet) [windows addObject:sheet];
  }
  return windows.array;
}

bool presented(NSWindow* window) {
  return window.visible || window.sheetParent != nil || window == NSApp.modalWindow ||
         window == (__bridge NSWindow*)nativeFileDialogPanel();
}

NSWindow* documentWindow() {
  for (NSWindow* window in NSApp.windows) {
    if ([window.identifier isEqualToString:@"native.documentWindow"]) return window;
  }
  return nil;
}

NSWindow* targetWindow() {
  const auto deepestSheet = [](NSWindow* window) {
    while (window.attachedSheet != nil) window = window.attachedSheet;
    return window;
  };
  if (void* panel = nativeFileDialogPanel()) return deepestSheet((__bridge NSWindow*)panel);
  if (focusedWindow && focusedWindow.visible) return deepestSheet(focusedWindow);
  NSWindow* main = documentWindow();
  if (main.attachedSheet) return deepestSheet(main);
  if (NSApp.modalWindow) return deepestSheet(NSApp.modalWindow);
  return deepestSheet(NSApp.keyWindow != nil ? NSApp.keyWindow : main);
}

NSSavePanel* filePanel() {
  NSWindow* target = targetWindow();
  return [target isKindOfClass:NSSavePanel.class] ? (NSSavePanel*)target : nil;
}

NSView* findView(NSView* root, const std::function<bool(NSView*)>& matches) {
  if (!root || root.hidden) return nil;
  if (matches(root)) return root;
  for (NSView* child in root.subviews) {
    if (NSView* found = findView(child, matches)) return found;
  }
  return nil;
}

// Layout checks need to address the actual card even while it is hidden.
// Unlike findControl, this never materializes or scrolls table rows.
NSView* identifiedView(NSView* root, const QString& identifier) {
  if (!root) return nil;
  if (qt(root.accessibilityIdentifier) == identifier || qt(root.identifier) == identifier) return root;
  for (NSView* child in root.subviews) {
    if (NSView* found = identifiedView(child, identifier)) return found;
  }
  return nil;
}

bool viewVisible(NSView* view) {
  return view && presented(view.window) && ![view isHiddenOrHasHiddenAncestor] &&
         !NSIsEmptyRect(NSIntersectionRect(view.visibleRect, view.bounds));
}

NSMenuItem* findAction(NSMenu* menu, const QString& identifier);

QString commandIdentifier(QAction* action) {
  return action->objectName().startsWith(QStringLiteral("command_"))
      ? action->objectName().mid(8)
      : QStringLiteral("native.action.%1").arg(reinterpret_cast<quintptr>(action), 0, 16);
}

QMenu* retainedRibbonMenu(const QString& workspace, const QString& group) {
  const QString name = QStringLiteral("ribbonMenu_") + workspace + QLatin1Char('/') + group;
  for (QWidget* widget : QApplication::allWidgets()) {
    if (widget->objectName() == name) {
      if (auto* button = qobject_cast<QToolButton*>(widget)) return button->menu();
    }
  }
  return nullptr;
}

bool sameCommandMenu(NSMenu* native, QMenu* retained, bool caption, const QString& path, QString& error) {
  [native update]; // Validate the same current states that opening it presents.
  QList<QAction*> expected;
  for (QAction* action : retained->actions()) {
    if (action->isVisible() && !action->isSeparator()) expected.append(action);
  }
  NSMutableArray<NSMenuItem*>* actual = [NSMutableArray array];
  for (NSUInteger index = caption ? 1 : 0; index < native.itemArray.count; ++index) {
    NSMenuItem* item = native.itemArray[index];
    if (![item isSeparatorItem]) [actual addObject:item];
  }
  if (actual.count != NSUInteger(expected.size())) {
    error = QStringLiteral("native menu '%1' has %2 commands; the retained CAD menu has %3")
        .arg(path).arg(qulonglong(actual.count)).arg(expected.size());
    return false;
  }
  for (int index = 0; index < expected.size(); ++index) {
    QAction* action = expected[index];
    NSMenuItem* item = actual[NSUInteger(index)];
    QString title = action->text();
    title.remove(QLatin1Char('&'));
    const QString identifier = commandIdentifier(action);
    if (qt([item.representedObject isKindOfClass:NSString.class] ? item.representedObject : nil) != identifier ||
        qt(item.title) != title || bool(item.submenu) != bool(action->menu())) {
      error = QStringLiteral("native menu '%1' item %2 differs from CAD command '%3' or its submenu shape")
          .arg(path).arg(index + 1).arg(identifier);
      return false;
    }
    if (action->isCheckable() && item.state != (action->isChecked() ? NSControlStateValueOn : NSControlStateValueOff)) {
      error = QStringLiteral("native menu '%1/%2' has a different checked state").arg(path, title);
      return false;
    }
    if (action->menu() && !sameCommandMenu(item.submenu, action->menu(), false, path + QLatin1Char('/') + title, error)) return false;
  }
  return true;
}

bool nativeMenuPlacement(QString& error) {
  NSMenu* appMenu = nil;
  for (NSMenuItem* top in NSApp.mainMenu.itemArray) {
    if ([top.title isEqualToString:@"Mitcad"]) appMenu = top.submenu;
    if (![top.title isEqualToString:@"Create"] && ![top.title isEqualToString:@"Sketch"]) continue;
    for (NSMenuItem* item in top.submenu.itemArray) {
      const QString id = qt([item.representedObject isKindOfClass:NSString.class] ? item.representedObject : nil);
      if (id.startsWith(QStringLiteral("tools.")) || id.startsWith(QStringLiteral("view.")) ||
          id.startsWith(QStringLiteral("file.")) || id.startsWith(QStringLiteral("help."))) {
        error = QStringLiteral("menu-only command '%1' leaked into native %2").arg(id, qt(top.title));
        return false;
      }
    }
  }
  if (!appMenu || !findAction(appMenu, QStringLiteral("tools.preferences"))) {
    error = QStringLiteral("native Settings is missing from the Mitcad application menu");
    return false;
  }
  if (findAction(NSApp.mainMenu, QStringLiteral("tools.preferences.cache"))) {
    error = QStringLiteral("the search-only Cache preferences page leaked into the native menu bar");
    return false;
  }
  return true;
}

bool ribbonAppearance(NSView* ribbon, QString& error) {
  NSView* caption = findView(ribbon, [](NSView* view) {
    return [view isKindOfClass:NSPopUpButton.class] &&
           qt(view.accessibilityIdentifier).startsWith(QStringLiteral("native.ribbon.group."));
  });
  NSView* document = caption.superview;
  if (!document) {
    error = QStringLiteral("the native ribbon has no command groups");
    return false;
  }
  std::vector<NSView*> buttons;
  std::vector<NSView*> captions;
  std::vector<NSView*> separators;
  for (NSView* child in document.subviews) {
    if (child.hidden) continue;
    if ([child isKindOfClass:NSPopUpButton.class]) captions.push_back(child);
    else if ([child isKindOfClass:NSButton.class]) buttons.push_back(child);
    else if ([child isKindOfClass:NSBox.class] && ((NSBox*)child).boxType == NSBoxSeparator) separators.push_back(child);
  }
  const auto leftToRight = [](NSView* a, NSView* b) { return NSMinX(a.frame) < NSMinX(b.frame); };
  std::sort(buttons.begin(), buttons.end(), leftToRight);
  std::sort(captions.begin(), captions.end(), leftToRight);
  const QString workspace = identifiedView(ribbon, QStringLiteral("native.ribbon.sketch.finish"))
      ? QStringLiteral("SKETCH") : QStringLiteral("SOLID");
  for (NSView* group : captions) {
    const QString name = qt(group.accessibilityIdentifier).mid(QStringLiteral("native.ribbon.group.").size()).toUpper();
    QMenu* retained = retainedRibbonMenu(workspace, name);
    if (!retained) {
      error = QStringLiteral("the retained CAD menu '%1/%2' is missing").arg(workspace, name);
      return false;
    }
    if (!sameCommandMenu(((NSPopUpButton*)group).menu, retained, true, workspace + QLatin1Char('/') + name, error)) return false;
  }
  if (!nativeMenuPlacement(error)) return false;
  for (size_t i = 1; i < buttons.size(); ++i) {
    if (NSMinX(buttons[i].frame) - NSMaxX(buttons[i - 1].frame) < 7.9) {
      error = QStringLiteral("native ribbon buttons '%1' and '%2' lack their 8-point gap")
          .arg(qt(buttons[i - 1].accessibilityIdentifier), qt(buttons[i].accessibilityIdentifier));
      return false;
    }
  }
  for (size_t i = 0; i < captions.size(); ++i) {
    NSView* group = captions[i];
    if (!NSContainsRect(document.bounds, group.frame) ||
        group.frame.size.width + 1 < group.intrinsicContentSize.width ||
        (i > 0 && NSMaxX(captions[i - 1].frame) >= NSMinX(group.frame))) {
      error = QStringLiteral("native ribbon caption '%1' is clipped or overlaps another group")
          .arg(qt(group.accessibilityIdentifier));
      return false;
    }
    for (NSView* separator : separators) {
      if (NSIntersectsRect(group.frame, separator.frame)) {
        error = QStringLiteral("native ribbon separator overlaps caption '%1'").arg(qt(group.accessibilityIdentifier));
        return false;
      }
    }
  }
  return true;
}

bool coloredIconMatches(NSButton* native, const QIcon& sharedIcon, QString& error) {
  if (!native.image || [native.image isTemplate] || sharedIcon.isNull()) {
    error = QStringLiteral("native icon '%1' is missing or monochrome templated").arg(qt(native.accessibilityIdentifier));
    return false;
  }
  NSBitmapImageRep* bitmap = nil;
  for (NSImageRep* representation in native.image.representations) {
    if ([representation isKindOfClass:NSBitmapImageRep.class]) {
      bitmap = (NSBitmapImageRep*)representation;
      break;
    }
  }
  NSData* png = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
  QImage actual = QImage::fromData(static_cast<const uchar*>(png.bytes), int(png.length), "PNG")
                      .convertToFormat(QImage::Format_RGBA8888_Premultiplied);
  if (actual.isNull()) {
    error = QStringLiteral("native icon '%1' has no readable bitmap").arg(qt(native.accessibilityIdentifier));
    return false;
  }
  // These are physical bitmap pixels, including Retina scaling. Render
  // the shared CAD asset at that exact size with DPR 1 so QIcon does not
  // multiply the captured size by the display's scale a second time.
  const QImage expected = sharedIcon.pixmap(actual.size(), 1.0).toImage()
                              .convertToFormat(QImage::Format_RGBA8888_Premultiplied);
  if (actual.size() != expected.size()) {
    error = QStringLiteral("native icon '%1' differs in bitmap size from the shared palette asset")
        .arg(qt(native.accessibilityIdentifier));
    return false;
  }
  bool colored = false;
  for (int y = 0; y < actual.height(); ++y) {
    const uchar* a = actual.constScanLine(y);
    const uchar* e = expected.constScanLine(y);
    for (int x = 0; x < actual.width(); ++x) {
      for (int channel = 0; channel < 4; ++channel) {
        if (std::abs(int(a[x * 4 + channel]) - int(e[x * 4 + channel])) > 2) {
          error = QStringLiteral("native icon '%1' differs from the shared palette asset at %2,%3")
              .arg(qt(native.accessibilityIdentifier)).arg(x).arg(y);
          return false;
        }
      }
      colored = colored || (a[x * 4 + 3] > 128 &&
          std::max({a[x * 4], a[x * 4 + 1], a[x * 4 + 2]}) -
          std::min({a[x * 4], a[x * 4 + 1], a[x * 4 + 2]}) > 16);
    }
  }
  if (!colored) error = QStringLiteral("native icon '%1' lost its CAD colours").arg(qt(native.accessibilityIdentifier));
  return colored;
}

bool sketchIconAppearance(NSView* content, QString& error) {
  const QString id = QStringLiteral("sketch.constraint.horizontal_vertical");
  NSView* palette = identifiedView(content, QStringLiteral("palette_") + id);
  if (!viewVisible(palette)) return true; // Non-sketch inspector or hidden palette.
  NSView* ribbon = identifiedView(content, QStringLiteral("native.ribbon.") + id);
  QToolButton* presenter = nullptr;
  for (QWidget* widget : QApplication::allWidgets()) {
    if (widget->objectName() == QStringLiteral("palette_") + id) {
      presenter = qobject_cast<QToolButton*>(widget);
      break;
    }
  }
  if (![ribbon isKindOfClass:NSButton.class] || ![palette isKindOfClass:NSButton.class] || !presenter) {
    error = QStringLiteral("the sketch constraint icons are missing their shared palette presenter");
    return false;
  }
  return coloredIconMatches((NSButton*)ribbon, presenter->icon(), error) &&
         coloredIconMatches((NSButton*)palette, presenter->icon(), error);
}

NSView* findControlView(NSView* root, const std::function<bool(NSView*)>& matches,
                        NSTableView** rowTable, NSInteger* rowIndex) {
  if (!root || root.hidden) return nil;
  if (matches(root)) return root;
  if ([root isKindOfClass:NSTableView.class]) {
    NSTableView* table = (NSTableView*)root;
    // View-based AppKit tables only instantiate visible cells. Prepare an
    // offscreen row using the public API, then scroll it into view so input
    // and readiness checks can address the same real row below the fold.
    for (NSInteger row = 0; row < table.numberOfRows; ++row) {
      for (NSInteger column = 0; column < table.numberOfColumns; ++column) {
        NSView* cell = [table viewAtColumn:column row:row makeIfNecessary:YES];
        if (NSView* found = findView(cell, matches)) {
          [table scrollRowToVisible:row];
          [table layoutSubtreeIfNeeded];
          if (NSView* live = [table viewAtColumn:column row:row makeIfNecessary:NO]) {
            if (NSView* actual = findView(live, matches)) found = actual;
          }
          if (rowTable) *rowTable = table;
          if (rowIndex) *rowIndex = row;
          return found;
        }
      }
    }
  }
  for (NSView* child in root.subviews) {
    if (NSView* found = findControlView(child, matches, rowTable, rowIndex)) return found;
  }
  return nil;
}

NSView* findControl(const QString& identifier, NSTableView** rowTable = nullptr, NSInteger* rowIndex = nullptr) {
  if (rowTable) *rowTable = nil;
  if (rowIndex) *rowIndex = -1;
  NSWindow* target = targetWindow();
  const auto match = [&](NSView* view) {
    return qt(view.accessibilityIdentifier) == identifier || qt(view.identifier) == identifier;
  };
  const auto inWindow = [&](NSWindow* window) -> NSView* {
    if (NSView* found = findControlView(window.contentView.superview != nil ? window.contentView.superview : window.contentView,
                                       match, rowTable, rowIndex)) return found;
    // AppKit can host toolbar custom views outside the content/titlebar
    // hierarchy, or detach them into overflow. Read the actual inserted
    // items through the public API; palette samples are not in this list.
    for (NSToolbarItem* item in window.toolbar.items) {
      if (qt(item.itemIdentifier) == identifier && item.view) return item.view;
      if (NSView* found = identifiedView(item.view, identifier)) return found;
    }
    return nil;
  };
  if (NSView* found = inWindow(target)) return found;
  // A transient command-search popover has its own AppKit window. It may
  // not become key until its search field starts editing.
  // NSApp.windows also includes NSPopover's panels on macOS versions
  // where those panels do not participate in orderedWindows.
  for (NSWindow* window in nativeWindows()) {
    if (window == target || !presented(window)) continue;
    if (NSView* found = inWindow(window)) return found;
  }
  return nil;
}

NSButton* findButton(const QRegularExpression& expression) {
  const auto match = [&](NSView* view) {
    if (![view isKindOfClass:NSButton.class] || !((NSButton*)view).enabled) return false;
    QString title = qt(((NSButton*)view).title);
    title.remove(QLatin1Char('&'));
    return expression.match(title).hasMatch() || expression.match(qt(view.accessibilityLabel)).hasMatch();
  };
  NSView* content = targetWindow().contentView;
  return (NSButton*)findView(content.superview != nil ? content.superview : content, match);
}

bool windowMatches(NSWindow* window, const QRegularExpression& expression) {
  if (!presented(window) || window == documentWindow()) return false;
  if (expression.match(qt(window.title)).hasMatch()) return true;
  if ([window isKindOfClass:NSSavePanel.class] && expression.match(qt(((NSSavePanel*)window).prompt)).hasMatch()) return true;
  if (expression.match(qt(window.accessibilityIdentifier)).hasMatch()) return true;
  // Native sheets such as NSAlert have no useful window title. Match their
  // visible label text in the same way as the portable QMessageBox route.
  return findView(window.contentView, [&](NSView* view) {
    return [view isKindOfClass:NSTextField.class] && !((NSTextField*)view).editable &&
           expression.match(qt(((NSTextField*)view).stringValue)).hasMatch();
  }) != nil;
}

NSMenuItem* findAction(NSMenu* menu, const QString& identifier) {
  for (NSMenuItem* item in menu.itemArray) {
    if (qt([item.representedObject isKindOfClass:NSString.class] ? item.representedObject : nil) == identifier) return item;
    if (item.submenu) if (NSMenuItem* child = findAction(item.submenu, identifier)) return child;
  }
  return nil;
}

bool isQtResponder(NSResponder* responder) {
  if (!responder) return false;
  const QString name = qt(NSStringFromClass(responder.class));
  return name.startsWith(QStringLiteral("QNS")) || name.startsWith(QStringLiteral("QCocoa"));
}

NSString* characters(int key, const QString& text) {
  if (!text.isEmpty()) return text.toNSString();
  unichar character = 0;
  switch (key) {
  case Qt::Key_Return: case Qt::Key_Enter: character = NSCarriageReturnCharacter; break;
  case Qt::Key_Escape: character = 0x1b; break;
  case Qt::Key_Tab: case Qt::Key_Backtab: character = NSTabCharacter; break;
  case Qt::Key_Backspace: character = NSBackspaceCharacter; break;
  case Qt::Key_Delete: character = NSDeleteFunctionKey; break;
  case Qt::Key_Left: character = NSLeftArrowFunctionKey; break;
  case Qt::Key_Right: character = NSRightArrowFunctionKey; break;
  case Qt::Key_Up: character = NSUpArrowFunctionKey; break;
  case Qt::Key_Down: character = NSDownArrowFunctionKey; break;
  case Qt::Key_Home: character = NSHomeFunctionKey; break;
  case Qt::Key_End: character = NSEndFunctionKey; break;
  case Qt::Key_PageUp: character = NSPageUpFunctionKey; break;
  case Qt::Key_PageDown: character = NSPageDownFunctionKey; break;
  default:
    if (key >= Qt::Key_F1 && key <= Qt::Key_F35) character = NSF1FunctionKey + key - Qt::Key_F1;
    else if (key > 0 && key < 0x10000) character = QChar(ushort(key)).toLower().unicode();
    break;
  }
  return character == 0 ? @"" : [NSString stringWithCharacters:&character length:1];
}

unsigned short virtualKey(int key) {
  switch (key) {
  case Qt::Key_Z: return 6;
  case Qt::Key_Return: return 36;
  case Qt::Key_Enter: return 76;
  case Qt::Key_Tab: case Qt::Key_Backtab: return 48;
  case Qt::Key_Space: return 49;
  case Qt::Key_Backspace: return 51;
  case Qt::Key_Delete: return 117;
  case Qt::Key_F2: return 120;
  case Qt::Key_Escape: return 53;
  case Qt::Key_Left: return 123;
  case Qt::Key_Right: return 124;
  case Qt::Key_Down: return 125;
  case Qt::Key_Up: return 126;
  default: return 0;
  }
}

} // namespace

bool active() { return nativeDialogsActive() && documentWindow() != nil; }

bool nativeFocused() {
  return active() && targetWindow() && !isQtResponder(targetWindow().firstResponder);
}

bool applicationDelegate(QString& error) {
  id<NSApplicationDelegate> delegate = NSApp.delegate;
  const bool native = active();
  const bool resign = [delegate respondsToSelector:@selector(applicationDidResignActive:)];
  const bool become = [delegate respondsToSelector:@selector(applicationDidBecomeActive:)];
  const bool reopen = [delegate respondsToSelector:@selector(applicationShouldHandleReopen:hasVisibleWindows:)];
  const bool secureSelector = [delegate respondsToSelector:@selector(applicationSupportsSecureRestorableState:)];
  const bool secure = secureSelector && [delegate applicationSupportsSecureRestorableState:NSApp];
  const auto identity = [](id object) {
    return object ? QStringLiteral("%1@%2").arg(qt(NSStringFromClass([object class])))
                       .arg(quintptr((__bridge void*)object), 0, 16) : QStringLiteral("nil");
  };
  const QString details = QStringLiteral("native=%1 delegate=%2 resign=%3 become=%4 reopen=%5 secureSelector=%6 secure=%7 "
                                         "forwardResign=%8 forwardSecure=%9 qtState=%10 appActive=%11")
      .arg(native).arg(identity(delegate)).arg(resign).arg(become).arg(reopen).arg(secureSelector).arg(secure)
      .arg(identity([(NSObject*)delegate forwardingTargetForSelector:@selector(applicationDidResignActive:)]))
      .arg(identity([(NSObject*)delegate forwardingTargetForSelector:@selector(applicationSupportsSecureRestorableState:)]))
      .arg(int(qApp->applicationState())).arg(bool(NSApp.active));
  qInfo().noquote() << QStringLiteral("Native application delegate diagnostic: %1").arg(details);
  if (!native || !resign || !become || !reopen || !secureSelector || !secure) {
    error = QStringLiteral("native application delegate lost Qt activation or secure state restoration: %1").arg(details);
    return false;
  }
  QList<Qt::ApplicationState> changes;
  class ActivationEvents : public QObject {
  public:
    int deactivations = 0;
    bool eventFilter(QObject*, QEvent* event) override {
      if (event->type() == QEvent::ApplicationDeactivate) ++deactivations;
      return false;
    }
  } events;
  qApp->installEventFilter(&events);
  QObject::connect(qApp, &QGuiApplication::applicationStateChanged, &events,
                   [&changes](Qt::ApplicationState state) { changes.append(state); });
  const auto settle = [](Qt::ApplicationState expected) {
    QElapsedTimer deadline;
    deadline.start();
    while (qApp->applicationState() != expected && deadline.elapsed() < 2000) {
      QCoreApplication::processEvents(QEventLoop::AllEvents, 25);
    }
    return qApp->applicationState() == expected;
  };
  const auto notification = [](NSNotificationName name) {
    return [NSNotification notificationWithName:name object:NSApp];
  };
  [delegate applicationDidBecomeActive:notification(NSApplicationDidBecomeActiveNotification)];
  if (!settle(Qt::ApplicationActive)) {
    error = QStringLiteral("native application activation did not reach Qt");
    return false;
  }
  changes.clear();
  events.deactivations = 0;
  [delegate applicationDidResignActive:notification(NSApplicationDidResignActiveNotification)];
  if (!settle(Qt::ApplicationInactive)) {
    error = QStringLiteral("native application deactivation did not reach Qt");
    return false;
  }
  [delegate applicationDidBecomeActive:notification(NSApplicationDidBecomeActiveNotification)];
  if (!settle(Qt::ApplicationActive)) {
    error = QStringLiteral("native application reactivation did not reach Qt");
    return false;
  }
  [delegate applicationDidResignActive:notification(NSApplicationDidResignActiveNotification)];
  if (!settle(Qt::ApplicationInactive) ||
      ![delegate applicationShouldHandleReopen:NSApp hasVisibleWindows:YES] ||
      !settle(Qt::ApplicationActive)) {
    error = QStringLiteral("native Dock reopen did not restore Qt's active application state");
    return false;
  }
  const QList<Qt::ApplicationState> expected = {
      Qt::ApplicationInactive, Qt::ApplicationActive, Qt::ApplicationInactive, Qt::ApplicationActive};
  if (changes != expected || events.deactivations != 2) {
    error = QStringLiteral("native application delegate lost or duplicated Qt state signals or deactivate events");
    return false;
  }
  qInfo() << "Native application delegate: activation, deactivation, reopen and secure state passed";
  return true;
}

bool controlExists(const QString& identifier) { return active() && findControl(identifier) != nil; }

bool textOf(const QString& identifier, QString& value) {
  if (identifier == QStringLiteral("native.window.origin") || identifier == QStringLiteral("native.window.size")) {
    NSWindow* window = documentWindow();
    if (!window) return false;
    if (identifier == QStringLiteral("native.window.origin")) {
      const NSPoint origin = window.frame.origin;
      const NSPoint screen = NSScreen.mainScreen.visibleFrame.origin;
      value = QStringLiteral("%1,%2").arg(qRound(origin.x - screen.x)).arg(qRound(origin.y - screen.y));
    } else {
      const NSSize size = window.contentView.frame.size;
      value = QStringLiteral("%1x%2").arg(qRound(size.width)).arg(qRound(size.height));
    }
    qDebug().noquote() << identifier << value;
    return true;
  }
  if (identifier == QStringLiteral("native.file.name") || identifier == QStringLiteral("native.file.directory")) {
    NSSavePanel* panel = filePanel();
    if (!panel) return false;
    value = qt(identifier == QStringLiteral("native.file.name") ? panel.nameFieldStringValue : panel.directoryURL.path);
    return true;
  }
  NSView* view = findControl(identifier);
  NSString* text = nil;
  if ([view isKindOfClass:NSTextField.class]) text = ((NSTextField*)view).stringValue;
  else if ([view isKindOfClass:NSPopUpButton.class]) {
    NSPopUpButton* popup = (NSPopUpButton*)view;
    text = popup.pullsDown ? popup.title : popup.titleOfSelectedItem;
  }
  else if ([view isKindOfClass:NSButton.class]) text = ((NSButton*)view).title;
  if (!text) return false;
  value = qt(text);
  return true;
}

bool textMatches(const QString& identifier, const QRegularExpression& expression) {
  QString text;
  return textOf(identifier, text) && expression.match(text).hasMatch();
}

bool enabled(const QString& identifier, bool expected) {
  NSView* view = findControl(identifier);
  return [view isKindOfClass:NSControl.class] && bool(((NSControl*)view).enabled) == expected;
}

bool visible(const QString& identifier, bool expected) {
  NSWindow* window = documentWindow();
  NSView* root = window.contentView.superview != nil ? window.contentView.superview : window.contentView;
  NSView* view = identifiedView(root, identifier);
  if (view) return viewVisible(view) == expected;
  // Search popovers and secondary controls live in separate AppKit windows.
  // Keep this a visibility query: do not materialize or scroll table rows.
  for (NSWindow* secondary in nativeWindows()) {
    if (secondary == window) continue;
    NSView* content = secondary.contentView;
    NSView* secondaryRoot = content.superview != nil ? content.superview : content;
    if (NSView* found = identifiedView(secondaryRoot, identifier); viewVisible(found)) return expected;
  }
  return !expected;
}

bool present(const QString& identifier, bool expected) {
  NSWindow* window = documentWindow();
  if (!window) return false;
  NSView* root = window.contentView.superview != nil ? window.contentView.superview : window.contentView;
  return (identifiedView(root, identifier) != nil) == expected;
}

bool resizeDocument(int width, int height, QString& error) {
  NSWindow* window = documentWindow();
  if (!window || width < 820 || height < 520) {
    error = QStringLiteral("native-resize requires a document and content size at least 820 x 520");
    return false;
  }
  [window setContentSize:NSMakeSize(width, height)];
  [window.contentView layoutSubtreeIfNeeded];
  return true;
}

bool toolbarItemPresent(const QString& identifier, bool expected) {
  NSToolbar* toolbar = documentWindow().toolbar;
  if (!toolbar) return false;
  bool found = false;
  for (NSToolbarItem* item in toolbar.items) {
    if (qt(item.itemIdentifier) == identifier) found = true;
  }
  return found == expected;
}

bool configureToolbarItem(const QString& identifier, const QString& operation, QString& error) {
  NSToolbar* toolbar = documentWindow().toolbar;
  if (!toolbar) {
    error = QStringLiteral("native toolbar is not installed");
    return false;
  }
  NSString* itemID = identifier.toNSString();
  if (operation == QStringLiteral("palette")) {
    // Customization asks for samples that are never inserted. Keep one
    // alive until the next event-loop turn to expose accidental tracking.
    NSToolbarItem* sample = [toolbar.delegate toolbar:toolbar itemForItemIdentifier:itemID willBeInsertedIntoToolbar:NO];
    if (!sample) { error = QStringLiteral("native toolbar palette item is unavailable"); return false; }
    dispatch_async(dispatch_get_main_queue(), ^{ (void)sample.view; });
    return true;
  }
  if (operation != QStringLiteral("insert") && operation != QStringLiteral("remove")) {
    error = QStringLiteral("native-toolbar-item expects insert, remove or palette");
    return false;
  }
  NSMutableDictionary* configuration = [toolbar.configurationDictionary mutableCopy];
  NSMutableArray* identifiers = [configuration[@"TB Item Identifiers"] mutableCopy];
  if (!identifiers) {
    error = QStringLiteral("native toolbar configuration has no item identifiers");
    return false;
  }
  [identifiers removeObject:itemID];
  if (operation == QStringLiteral("insert")) [identifiers insertObject:itemID atIndex:std::min(NSUInteger(3), identifiers.count)];
  configuration[@"TB Item Identifiers"] = identifiers;
  [toolbar setConfigurationFromDictionary:configuration];
  [NSUserDefaults.standardUserDefaults synchronize];
  if (!toolbarItemPresent(identifier, operation == QStringLiteral("insert"))) {
    error = QStringLiteral("native toolbar configuration did not %1 '%2'").arg(operation, identifier);
    return false;
  }
  return true;
}

bool moveDocument(int x, int y, QString& error) {
  NSWindow* window = documentWindow();
  NSScreen* screen = NSScreen.mainScreen;
  if (!window || !screen) {
    error = QStringLiteral("native-move requires a document and a screen");
    return false;
  }
  const NSPoint origin = screen.visibleFrame.origin;
  [window setFrameOrigin:NSMakePoint(origin.x + x, origin.y + y)];
  return true;
}

bool pinch(QOpenGLWidget* viewport, const QPointF& position, double magnification, QString& error) {
  NSWindow* window = documentWindow();
  if (!active() || !viewport || !std::isfinite(magnification)) {
    error = QStringLiteral("native-pinch requires the native document and a finite magnification");
    return false;
  }
  // The standalone Qt root already owns the embedded native handle.
  // Giving the QOpenGLWidget its own winId would create a new native child
  // and change the gesture routing this regression is meant to exercise.
  QWidget* root = viewport->window();
  const WId rootId = root->internalWinId();
  if (!rootId) {
    error = QStringLiteral("native-pinch requires an existing native Qt root handle");
    return false;
  }
  NSView* native = (__bridge NSView*)reinterpret_cast<void*>(rootId);
  if (native.window != window) {
    error = QStringLiteral("native-pinch Qt root is not embedded in its document window");
    return false;
  }
  const QPointF rootPosition = viewport->mapTo(root, position);
  NSPoint local = NSMakePoint(rootPosition.x(), rootPosition.y());
  if (!native.flipped) local.y = NSMaxY(native.bounds) - local.y;
  const NSPoint point = [native convertPoint:local toView:nil];
  [window makeKeyAndOrderFront:nil];
  @try {
    for (NSEventPhase phase : {NSEventPhaseBegan, NSEventPhaseChanged, NSEventPhaseEnded}) {
      MITCADTestMagnificationEvent* event = [MITCADTestMagnificationEvent new];
      event->target = window;
      event->point = point;
      event->amount = phase == NSEventPhaseChanged ? magnification : 0;
      event->gesturePhase = phase;
      event->eventTime = NSProcessInfo.processInfo.systemUptime;
      [window sendEvent:event];
    }
  } @catch (NSException* exception) {
    error = QStringLiteral("native-pinch AppKit dispatch rejected the test event: %1").arg(qt(exception.reason));
    return false;
  }
  return true;
}

bool floatingLayout(QString& error) {
  NSWindow* window = documentWindow();
  NSView* content = window.contentView;
  [content layoutSubtreeIfNeeded];
  NSView* canvas = identifiedView(content, QStringLiteral("native.viewportHost"));
  if (!viewVisible(canvas)) {
    error = QStringLiteral("the native canvas is not visible");
    return false;
  }
  const NSRect canvasRect = [canvas convertRect:canvas.bounds toView:content];
  const NSRect bounds = content.bounds;
  const auto close = [](CGFloat a, CGFloat b) { return std::abs(a - b) <= 1; };
  NSView* ribbon = identifiedView(content, QStringLiteral("native.ribbon"));
  if (!viewVisible(ribbon)) {
    error = QStringLiteral("the native CAD ribbon is not visible");
    return false;
  }
  if (!ribbonAppearance(ribbon, error) || !sketchIconAppearance(content, error)) return false;
  const NSRect ribbonRect = [ribbon convertRect:ribbon.bounds toView:content];
  const bool vertical = content.flipped
      ? close(NSMinY(canvasRect), NSMaxY(ribbonRect)) && close(NSMaxY(canvasRect), NSMaxY(bounds))
      : close(NSMaxY(canvasRect), NSMinY(ribbonRect)) && close(NSMinY(canvasRect), NSMinY(bounds));
  if (!close(NSMinX(canvasRect), NSMinX(bounds)) || !close(canvasRect.size.width, bounds.size.width) || !vertical) {
    error = QStringLiteral("the native canvas does not fill the document below the ribbon (%1 x %2 versus %3 x %4)")
        .arg(canvasRect.size.width).arg(canvasRect.size.height).arg(bounds.size.width).arg(bounds.size.height);
    return false;
  }
  NSView* sidebar = identifiedView(content, QStringLiteral("native.sidebar"));
  NSView* inspector = identifiedView(content, QStringLiteral("native.inspectorCard"));
  if (!sidebar || !inspector) {
    error = QStringLiteral("the native floating sidebar or inspector card is missing");
    return false;
  }
  NSView* model = identifiedView(content, QStringLiteral("native.modelTree"));
  if (![model isKindOfClass:NSOutlineView.class] || !model.enclosingScrollView ||
      ((NSOutlineView*)model).backgroundColor.alphaComponent > 0 ||
      model.enclosingScrollView.drawsBackground) {
    error = QStringLiteral("the native browser draws an opaque inset background over its card");
    return false;
  }
  for (NSString* identifier in @[@"native.sidebar", @"native.inspectorCard", @"native.historyCard", @"native.ribbon"]) {
    NSView* card = identifiedView(content, qt(identifier));
    if (!card || !viewVisible(card)) continue;
    const NSRect rect = [card convertRect:card.bounds toView:content];
    if (rect.size.width <= 1 || rect.size.height <= 1 ||
        !NSContainsRect(NSInsetRect(bounds, -1, -1), rect)) {
      error = QStringLiteral("native overlay '%1' is empty or outside the document").arg(qt(identifier));
      return false;
    }
    const NSPoint center = NSMakePoint(NSMidX(card.bounds), NSMidY(card.bounds));
    NSView* hit = [content hitTest:[card convertPoint:center toView:content.superview]];
    if (hit != card && ![hit isDescendantOf:card]) {
      error = QStringLiteral("native overlay '%1' cannot receive pointer input above the canvas").arg(qt(identifier));
      return false;
    }
  }
  if (viewVisible(sidebar) && viewVisible(inspector)) {
    const NSRect left = [sidebar convertRect:sidebar.bounds toView:content];
    const NSRect right = [inspector convertRect:inspector.bounds toView:content];
    if (NSMinX(left) <= NSMinX(bounds) || NSMaxX(right) >= NSMaxX(bounds) || NSIntersectsRect(left, right)) {
      error = QStringLiteral("native side cards lack their floating edge insets or overlap each other");
      return false;
    }
  }
  // The edge gaps must still pass pointer input to the full-width canvas.
  for (CGFloat x : {NSMinX(bounds) + 4, NSMaxX(bounds) - 4}) {
    const NSPoint point = NSMakePoint(x, NSMidY(bounds));
    NSView* hit = [content hitTest:[content convertPoint:point toView:content.superview]];
    if (hit != canvas && ![hit isDescendantOf:canvas]) {
      error = QStringLiteral("the canvas edge gap is blocked by an overlay");
      return false;
    }
  }
  return true;
}

bool setAppearance(const QString& name, QString& error) {
  if (name != QStringLiteral("light") && name != QStringLiteral("dark")) {
    error = QStringLiteral("native appearance must be light or dark");
    return false;
  }
  NSAppearance* appearance = [NSAppearance appearanceNamed:(name == QStringLiteral("dark") ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua)];
  if (!appearance) { error = QStringLiteral("the requested native appearance is unavailable"); return false; }
  NSApp.appearance = appearance;
  // Resolve real system roles in this app's appearance, without changing
  // macOS defaults. The application's normal palette watcher must then
  // replace its cached native CAD bitmaps.
  __block QPalette palette = QApplication::palette();
  [appearance performAsCurrentDrawingAppearance:^{
    const auto color = [](NSColor* source) {
      NSColor* rgb = [source colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
      return QColor::fromRgbF(rgb.redComponent, rgb.greenComponent, rgb.blueComponent, rgb.alphaComponent);
    };
    palette.setColor(QPalette::Window, color(NSColor.windowBackgroundColor));
    palette.setColor(QPalette::WindowText, color(NSColor.labelColor));
    palette.setColor(QPalette::Base, color(NSColor.textBackgroundColor));
    palette.setColor(QPalette::Text, color(NSColor.textColor));
    palette.setColor(QPalette::Button, color(NSColor.controlColor));
    palette.setColor(QPalette::ButtonText, color(NSColor.controlTextColor));
    palette.setColor(QPalette::Highlight, color(NSColor.selectedContentBackgroundColor));
    palette.setColor(QPalette::HighlightedText, color(NSColor.selectedTextColor));
    palette.setColor(QPalette::ToolTipBase, color(NSColor.controlBackgroundColor));
    palette.setColor(QPalette::ToolTipText, color(NSColor.labelColor));
    for (QPalette::ColorRole role : {QPalette::WindowText, QPalette::Text, QPalette::ButtonText}) {
      palette.setColor(QPalette::Disabled, role, color(NSColor.disabledControlTextColor));
    }
  }];
  QApplication::setPalette(palette);
  return true;
}

bool historyLayout(const QJsonObject& timeline, const QSet<QString>& filtered, const QStringList& selected,
                   bool checkSelection, QString& error) {
  NSView* content = documentWindow().contentView;
  [content layoutSubtreeIfNeeded];
  NSView* card = identifiedView(content, QStringLiteral("native.historyCard"));
  NSView* strip = identifiedView(content, QStringLiteral("native.historyStrip"));
  NSView* marker = identifiedView(content, QStringLiteral("native.historyMarker"));
  NSScrollView* scroll = strip.enclosingScrollView;
  if (!viewVisible(card) || !viewVisible(strip) || !viewVisible(marker) || !scroll ||
      ![marker isKindOfClass:NSControl.class] || [marker isKindOfClass:NSSlider.class]) {
    error = QStringLiteral("the native history is missing its horizontal feature strip or in-row marker");
    return false;
  }
  if (card.bounds.size.height > 84 || strip.bounds.size.height > 60 ||
      scroll.drawsBackground || scroll.contentView.drawsBackground || strip.opaque ||
      scroll.borderType != NSNoBorder) {
    error = QStringLiteral("native history is oversized or draws an opaque inset surface");
    return false;
  }
  const QJsonArray features = timeline.value(QStringLiteral("features")).toArray();
  const int position = timeline.value(QStringLiteral("marker")).toInt(-1);
  if (position < 0 || position > features.size()) {
    error = QStringLiteral("the model has an invalid timeline marker");
    return false;
  }
  int nativeCount = 0;
  for (NSView* child in strip.subviews) {
    if ([child respondsToSelector:@selector(activateHistoryFeature)]) ++nativeCount;
  }
  if (nativeCount != features.size()) {
    error = QStringLiteral("native history has %1 feature icons for %2 model features").arg(nativeCount).arg(features.size());
    return false;
  }
  struct Cell {
    int first;
    NSRect frame;
    bool hiddenRun;
  };
  std::vector<Cell> cells;
  int runs = 0;
  QStringList actualSelected;
  for (int index = 0; index < features.size(); ++index) {
    const QJsonObject feature = features[index].toObject();
    const QString uid = feature.value(QStringLiteral("uid")).toString();
    NSView* view = identifiedView(strip, QStringLiteral("native.history.") + uid);
    if (![view isKindOfClass:NSButton.class] || ![view respondsToSelector:@selector(activateHistoryFeature)]) {
      error = QStringLiteral("history feature '%1' is not a native CAD icon button").arg(uid);
      return false;
    }
    NSButton* button = (NSButton*)view;
    if (filtered.contains(uid)) {
      if (!button.hidden || button.enabled || button.state == NSControlStateValueOn) {
        error = QStringLiteral("filtered history feature '%1' remains visible, enabled or selected").arg(uid);
        return false;
      }
      const QString previous = index > 0 ? features[index - 1].toObject().value(QStringLiteral("uid")).toString()
                                         : QString();
      if (index > 0 && filtered.contains(previous)) continue;
      NSView* mark = identifiedView(strip, QStringLiteral("native.history.filteredRun.%1").arg(index));
      const NSRect frame = [mark convertRect:mark.bounds toView:strip];
      if (![mark isKindOfClass:NSImageView.class] || !viewVisible(mark) ||
          frame.size.width <= 0 || frame.size.width > 16 || !NSContainsRect(strip.bounds, frame) ||
          (!cells.empty() && NSMaxX(cells.back().frame) >= NSMinX(frame))) {
        error = QStringLiteral("filtered history run at %1 has no compact native mark").arg(index);
        return false;
      }
      int count = 1;
      while (index + count < features.size() &&
             filtered.contains(features[index + count].toObject().value(QStringLiteral("uid")).toString())) ++count;
      if (!qt(mark.toolTip).startsWith(QStringLiteral("%1 feature(s) hidden").arg(count))) {
        error = QStringLiteral("filtered history run at %1 reports the wrong hidden count").arg(index);
        return false;
      }
      cells.push_back({index, frame, true});
      ++runs;
      continue;
    }
    if (button.hidden) {
      error = QStringLiteral("unfiltered history feature '%1' is hidden").arg(uid);
      return false;
    }
    const NSRect frame = [button convertRect:button.bounds toView:strip];
    if (frame.size.width <= 0 || frame.size.height <= 0 || !NSContainsRect(strip.bounds, frame) ||
        (!cells.empty() && (NSMaxX(cells.back().frame) >= NSMinX(frame) ||
                            std::abs(NSMidY(cells.back().frame) - NSMidY(frame)) > 1))) {
      error = QStringLiteral("native history feature '%1' is clipped, overlapping or outside the horizontal row").arg(uid);
      return false;
    }
    cells.push_back({index, frame, false});
    NSView* hit = [content hitTest:[button convertPoint:NSMakePoint(NSMidX(button.bounds), NSMidY(button.bounds))
                                               toView:content.superview]];
    if (hit != button) {
      error = QStringLiteral("native history feature '%1' cannot receive pointer input above the canvas").arg(uid);
      return false;
    }
    if (!coloredIconMatches(button, themeIcon(featureIcon(feature.value(QStringLiteral("type")).toString())), error)) return false;
    const bool dimmed = index >= position || feature.value(QStringLiteral("suppressed")).toBool();
    if (dimmed ? button.alphaValue > 0.6 || button.alphaValue <= 0.1 : button.alphaValue < 0.95) {
      error = QStringLiteral("native history feature '%1' does not visually reflect rollback/suppression").arg(uid);
      return false;
    }
    if (button.state == NSControlStateValueOn) actualSelected.append(uid);
  }
  int nativeRuns = 0;
  for (NSView* child in strip.subviews) {
    if (qt(child.accessibilityIdentifier).startsWith(QStringLiteral("native.history.filteredRun.")) && !child.hidden) ++nativeRuns;
  }
  if (nativeRuns != runs) {
    error = QStringLiteral("native history has %1 run marks for %2 hidden runs").arg(nativeRuns).arg(runs);
    return false;
  }
  const NSRect markerFrame = [marker convertRect:marker.bounds toView:strip];
  NSView* markerHit = [content hitTest:[marker convertPoint:NSMakePoint(NSMidX(marker.bounds), NSMidY(marker.bounds))
                                                 toView:content.superview]];
  if (markerHit != marker) {
    error = QStringLiteral("the native history marker cannot receive pointer input above the canvas");
    return false;
  }
  bool between = NSContainsRect(strip.bounds, markerFrame);
  for (const Cell& cell : cells) {
    const CGFloat left = cell.hiddenRun ? NSMidX(markerFrame) : NSMinX(markerFrame) + 1;
    const CGFloat right = cell.hiddenRun ? NSMidX(markerFrame) : NSMaxX(markerFrame) - 1;
    between = between && (cell.first < position ? left >= NSMaxX(cell.frame) : right <= NSMinX(cell.frame));
  }
  if (!between) {
    error = QStringLiteral("the native marker is not between feature icons at model position %1").arg(position);
    return false;
  }
  if (checkSelection && actualSelected != selected) {
    error = QStringLiteral("native selected feature icons '%1' differ from '%2'")
        .arg(actualSelected.join(QLatin1Char(',')), selected.join(QLatin1Char(',')));
    return false;
  }
  return true;
}

bool historyMarkerEvent(const QString& phase, int index, QString& error) {
  NSView* marker = phase == QStringLiteral("begin")
      ? identifiedView(documentWindow().contentView, QStringLiteral("native.historyMarker")) : draggedHistoryMarker;
  NSView* strip = marker.superview;
  if (!viewVisible(marker) || ![marker isKindOfClass:NSControl.class] || !((NSControl*)marker).enabled ||
      ![marker respondsToSelector:@selector(beginHistoryMarkerDrag)]) {
    error = QStringLiteral("no draggable native history marker");
    return false;
  }
  NSEventType type = NSEventTypeLeftMouseDragged;
  if (phase == QStringLiteral("begin")) {
    draggedHistoryMarker = marker;
    historyDragPoint = [marker convertPoint:NSMakePoint(NSMidX(marker.bounds), NSMidY(marker.bounds)) toView:nil];
    [marker.window makeKeyAndOrderFront:nil];
    type = NSEventTypeLeftMouseDown;
  } else if (phase == QStringLiteral("to")) {
    std::vector<NSView*> features;
    for (NSView* child in strip.subviews) {
      if ([child respondsToSelector:@selector(activateHistoryFeature)]) features.push_back(child);
    }
    std::sort(features.begin(), features.end(), [](NSView* a, NSView* b) { return NSMinX(a.frame) < NSMinX(b.frame); });
    if (features.empty() || index < 0 || index > int(features.size())) {
      error = QStringLiteral("native history drag index %1 is outside the feature row").arg(index);
      return false;
    }
    CGFloat x = 0;
    if (index == 0) {
      const CGFloat gap = features.size() > 1 ? NSMinX(features[1].frame) - NSMaxX(features[0].frame) : marker.bounds.size.width;
      x = NSMinX(features[0].frame) - gap / 2;
    } else if (index == int(features.size())) {
      const CGFloat gap = features.size() > 1 ? NSMinX(features.back().frame) - NSMaxX(features[features.size() - 2].frame)
                                             : marker.bounds.size.width;
      x = NSMaxX(features.back().frame) + gap / 2;
    } else {
      x = (NSMaxX(features[size_t(index - 1)].frame) + NSMinX(features[size_t(index)].frame)) / 2;
    }
    historyDragPoint = [strip convertPoint:NSMakePoint(x, NSMidY(marker.frame)) toView:nil];
  } else if (phase == QStringLiteral("end")) {
    type = NSEventTypeLeftMouseUp;
  } else {
    error = QStringLiteral("native history drag phase must be begin, to or end");
    return false;
  }
  NSEvent* event = [NSEvent mouseEventWithType:type location:historyDragPoint modifierFlags:0
      timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:marker.window.windowNumber
      context:nil eventNumber:0 clickCount:1 pressure:(type == NSEventTypeLeftMouseUp ? 0 : 1)];
  if (type == NSEventTypeLeftMouseDown) [marker mouseDown:event];
  else if (type == NSEventTypeLeftMouseDragged) [marker mouseDragged:event];
  else { [marker mouseUp:event]; draggedHistoryMarker = nil; }
  return true;
}

bool clickImmediately(const QString& identifier, QString& error) {
  // Job-start input must reach the existing document button before a
  // progress sheet or performClick's animation starts another event loop.
  NSView* view = identifiedView(documentWindow().contentView, identifier);
  if (![view isKindOfClass:NSButton.class] || [view isHiddenOrHasHiddenAncestor] || !((NSButton*)view).enabled) {
    error = QStringLiteral("no enabled native document button '%1'").arg(identifier);
    return false;
  }
  NSButton* button = (NSButton*)view;
  if (!button.action || ![button sendAction:button.action to:button.target]) {
    error = QStringLiteral("native button '%1' could not dispatch its action").arg(identifier);
    return false;
  }
  return true;
}

bool click(const QString& identifier, QString& error) {
  NSView* view = findControl(identifier);
  if (!view) { error = QStringLiteral("no native control '%1'").arg(identifier); return false; }
  [view scrollRectToVisible:view.bounds];
  [view.window makeKeyAndOrderFront:nil];
  if ([view isKindOfClass:NSButton.class]) {
    NSButton* button = (NSButton*)view;
    if (!button.enabled) { error = QStringLiteral("native button '%1' is disabled").arg(identifier); return false; }
    [button performClick:nil];
    return true;
  }
  if ([view isKindOfClass:NSTextField.class]) {
    [view.window makeFirstResponder:view];
    [(NSTextField*)view selectText:nil];
    return true;
  }
  if ([view isKindOfClass:NSControl.class]) {
    NSControl* control = (NSControl*)view;
    if (!control.enabled) { error = QStringLiteral("native control '%1' is disabled").arg(identifier); return false; }
    [control sendAction:control.action to:control.target];
    return true;
  }
  error = QStringLiteral("native view '%1' is not an actionable control").arg(identifier);
  return false;
}

bool hover(const QString& identifier, bool enter, QString& error) {
  NSView* view = findControl(identifier);
  if (!view) { error = QStringLiteral("no native control '%1'").arg(identifier); return false; }
  [view.window makeKeyAndOrderFront:nil];
  [view updateTrackingAreas];
  for (NSTrackingArea* area in view.trackingAreas) {
    if ((area.options & NSTrackingMouseEnteredAndExited) == 0 || area.owner != view) { continue; }
    const NSPoint location = [view convertPoint:NSMakePoint(NSMidX(view.bounds), NSMidY(view.bounds)) toView:nil];
    NSEvent* event = [NSEvent enterExitEventWithType:enter ? NSEventTypeMouseEntered : NSEventTypeMouseExited
                                        location:location modifierFlags:0 timestamp:NSProcessInfo.processInfo.systemUptime
                                    windowNumber:view.window.windowNumber context:nil eventNumber:0 trackingNumber:0 userData:nullptr];
    // Exercise the real AppKit tracking owner; it is responsible for
    // forwarding the event to the hidden Qt presenter.
    if (enter) { [view mouseEntered:event]; }
    else { [view mouseExited:event]; }
    return true;
  }
  error = QStringLiteral("native control '%1' has no hover tracking area").arg(identifier);
  return false;
}

bool replaceText(const QString& identifier, const QString& text, QString& error) {
  NSView* view = findControl(identifier);
  if (![view isKindOfClass:NSTextField.class] || !((NSTextField*)view).editable || !((NSTextField*)view).enabled) {
    error = QStringLiteral("no editable native field '%1'").arg(identifier);
    return false;
  }
  NSTextField* field = (NSTextField*)view;
  [field scrollRectToVisible:field.bounds];
  [field.window makeKeyAndOrderFront:nil];
  [field.window makeFirstResponder:field];
  [field selectText:nil];
  NSTextView* editor = (NSTextView*)field.currentEditor;
  if (!editor) { error = QStringLiteral("native field '%1' has no field editor").arg(identifier); return false; }
  // AppKit's own field editor emits the real editing notifications, which
  // drive command state, validation and preview through the presenter.
  [editor insertText:text.toNSString() replacementRange:NSMakeRange(0, editor.string.length)];
  return true;
}

bool choose(const QString& identifier, const QString& title, QString& error) {
  NSView* view = findControl(identifier);
  if (![view isKindOfClass:NSPopUpButton.class] || !((NSPopUpButton*)view).enabled) {
    error = QStringLiteral("no native choice '%1'").arg(identifier); return false;
  }
  NSPopUpButton* popup = (NSPopUpButton*)view;
  const NSInteger index = [popup indexOfItemWithTitle:title.toNSString()];
  if (index < 0) { error = QStringLiteral("native choice '%1' has no '%2' item").arg(identifier, title); return false; }
  [popup selectItemAtIndex:index];
  [popup sendAction:popup.action to:popup.target];
  return true;
}

bool disclosureArtwork(NSButton* button, QString& error) {
  // Compare rendered cells with a fresh native disclosure. Checking only
  // bezelStyle misses a cell that still draws inherited switch artwork.
  NSButton* reference = [[NSButton alloc] initWithFrame:NSMakeRect(0, 0, 24, 24)];
  [reference setButtonType:NSButtonTypePushOnPushOff];
  reference.bezelStyle = NSBezelStyleDisclosure;
  reference.title = @"";
  reference.font = button.font;
  reference.controlSize = button.controlSize;
  reference.appearance = button.effectiveAppearance;
  reference.state = button.state;
  const auto pixels = [](NSButton* control) {
    NSBitmapImageRep* bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nil
        pixelsWide:24 pixelsHigh:24 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO
        colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:96 bitsPerPixel:32];
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
    [NSColor.clearColor setFill];
    NSRectFillUsingOperation(NSMakeRect(0, 0, 24, 24), NSCompositingOperationCopy);
    [control.cell drawWithFrame:NSMakeRect(0, 0, 24, 24) inView:control];
    [NSGraphicsContext restoreGraphicsState];
    NSData* png = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
    return QImage::fromData(static_cast<const uchar*>(png.bytes), int(png.length), "PNG");
  };
  const QImage actual = pixels(button);
  const QImage expected = pixels(reference);
  if (actual.isNull() || expected.isNull() || actual != expected) {
    error = QStringLiteral("native disclosure does not render the system disclosure triangle");
    return false;
  }
  return true;
}

bool formControls(CommandHost& host, QString& error) {
  // BUILD no longer has a More section. Exercise the reusable disclosure
  // contract here, independently of any command's current arrangement.
  CommandDef def;
  def.id = QStringLiteral("test.native_form_controls");
  def.name = QStringLiteral("Native form controls regression");
  def.inspect = [](const CommandState&, const CommandContext&) { return Inspection{}; };
  def.inputs = {
      choiceInput(QStringLiteral("axis"), QStringLiteral("Direction"),
                  {{QStringLiteral("x"), QStringLiteral("Along X")},
                   {QStringLiteral("y"), QStringLiteral("Along Y")},
                   {QStringLiteral("z"), QStringLiteral("Along Z")}}, QStringLiteral("z"))
          .withChoiceIcons({{QStringLiteral("x"), QStringLiteral("build-axis-x")},
                            {QStringLiteral("y"), QStringLiteral("build-axis-y")},
                            {QStringLiteral("z"), QStringLiteral("build-axis-z")}})
          .withTooltip(QStringLiteral("The axis")),
      checkInput(QStringLiteral("more"), QStringLiteral("More")).withDisclosure()};
  for (int index = 0; index < 8; ++index) {
    def.inputs.append(textInput(QStringLiteral("detail%1").arg(index), QStringLiteral("Detail"))
                          .withVisible([](const CommandState& state) {
                            return state.checked(QStringLiteral("more"));
                          }));
  }
  const auto check = [&error](bool passed, const QString& message) {
    if (!passed) error = message;
    return passed;
  };
  {
    CommandSession session(def, host);
    auto form = createNativeCommandForm(session);
    NSView* root = (__bridge NSView*)form->nativeView();
    [root.widthAnchor constraintEqualToConstant:280].active = YES;
    CommandState state;
    state.setChoice(QStringLiteral("axis"), QStringLiteral("z"));
    form->refresh(state);
    const auto button = [root](const QString& identifier) -> NSButton* {
      NSView* view = identifiedView(root, identifier);
      return [view isKindOfClass:NSButton.class] ? (NSButton*)view : nil;
    };
    NSButton* x = button(QStringLiteral("input_axis_x"));
    NSButton* y = button(QStringLiteral("input_axis_y"));
    NSButton* z = button(QStringLiteral("input_axis_z"));
    NSButton* more = button(QStringLiteral("input_more"));
    if (!check(x && y && z && more && x.enabled && y.enabled && z.enabled &&
               x.state == NSControlStateValueOff && z.state == NSControlStateValueOn &&
               qt(x.toolTip) == QStringLiteral("Along X: The axis") &&
               qt(x.accessibilityLabel) == QStringLiteral("Along X") &&
               more.bezelStyle == NSBezelStyleDisclosure && qt(more.accessibilityLabel) == QStringLiteral("More"),
               QStringLiteral("native icon choices or disclosure have the wrong controls, tooltips or initial state"))) return false;
    for (const auto& axis : {QStringLiteral("x"), QStringLiteral("y"), QStringLiteral("z")}) {
      if (!coloredIconMatches(button(QStringLiteral("input_axis_") + axis),
                              themeIcon(QStringLiteral("build-axis-") + axis), error)) return false;
    }
    QString submitted;
    CommandPresenter::Actions actions = form->actions();
    actions.choiceChanged = [&](const QString& key, const QString& value) {
      submitted = value;
      state.setChoice(key, value);
      form->refresh(state);
    };
    actions.checkChanged = [&](const QString& key, bool checked) {
      state.setChecked(key, checked);
      form->refresh(state);
    };
    form->setActions(std::move(actions));
    for (NSButton* selected in @[x, y, y, z]) {
      // Offscreen contract checks dispatch synchronously. performClick's
      // animation spins an event loop and can advance the UI script before
      // this fixture has been installed in the document.
      selected.state = selected.state == NSControlStateValueOn ? NSControlStateValueOff : NSControlStateValueOn;
      [selected sendAction:selected.action to:selected.target];
      if (!check(submitted == qt(selected.identifier).section(QLatin1Char('|'), 2) &&
                 selected.state == NSControlStateValueOn &&
                 (x.state == NSControlStateValueOn) + (y.state == NSControlStateValueOn) +
                     (z.state == NSControlStateValueOn) == 1,
                 QStringLiteral("native icon choice lost its submitted value or exclusive selection"))) return false;
    }
    NSView* detail = identifiedView(root, QStringLiteral("row_detail0"));
    [root layoutSubtreeIfNeeded];
    const CGFloat collapsedHeight = root.fittingSize.height;
    if (!check(detail.hidden && more.state == NSControlStateValueOff,
               QStringLiteral("native disclosure did not initially hide its dependent rows"))) return false;
    if (!disclosureArtwork(more, error)) return false;
    more.state = NSControlStateValueOn;
    [more sendAction:more.action to:more.target];
    [root layoutSubtreeIfNeeded];
    if (!check(state.checked(QStringLiteral("more")) && !detail.hidden &&
               more.state == NSControlStateValueOn && root.fittingSize.height > collapsedHeight + 100,
               QStringLiteral("native disclosure did not expand its dependent rows and form height"))) return false;
    if (!disclosureArtwork(more, error)) return false;
    more.state = NSControlStateValueOff;
    [more sendAction:more.action to:more.target];
    [root layoutSubtreeIfNeeded];
    if (!check(!state.checked(QStringLiteral("more")) && detail.hidden &&
               std::abs(root.fittingSize.height - collapsedHeight) < 1,
               QStringLiteral("native disclosure left empty layout space after collapse"))) return false;
    state.setChoice(QStringLiteral("axis"), QStringLiteral("x"));
    form->refresh(state);
    if (!check(x.state == NSControlStateValueOn && z.state == NSControlStateValueOff,
               QStringLiteral("native icon choice did not follow a programmatic state refresh"))) return false;
    qDebug() << "Native form controls: shared artwork, exclusive choice values and disclosure layout verified";
  }
  // Mount the same fixture through the ordinary command controller so the
  // script also exercises the real inspector's cap and scrolling.
  auto* window = dynamic_cast<MainWindow*>(&host);
  auto* registry = window ? window->findChild<CommandRegistry*>() : nullptr;
  if (!check(registry != nullptr, QStringLiteral("native form fixture has no command registry"))) return false;
  if (!registry->find(def.id)) { registry->add(def); }
  window->startCommand(def.id, {}, {});
  return true;
}

bool disclosureLayout(QString& error) {
  NSView* content = documentWindow().contentView;
  [content layoutSubtreeIfNeeded];
  NSView* card = identifiedView(content, QStringLiteral("native.inspectorCard"));
  NSView* root = identifiedView(card, QStringLiteral("commandPanel"));
  NSView* last = identifiedView(root, QStringLiteral("input_detail7"));
  NSView* more = identifiedView(root, QStringLiteral("input_more"));
  if (!root || !last || ![more isKindOfClass:NSButton.class] || !root.enclosingScrollView) {
    error = QStringLiteral("mounted disclosure fixture is missing");
    return false;
  }
  const bool expanded = ((NSButton*)more).state == NSControlStateValueOn;
  NSView* history = identifiedView(content, QStringLiteral("native.historyCard"));
  const CGFloat floor = !history || history.hidden ? content.bounds.size.height - 12
                                                   : history.frame.origin.y - 8;
  const CGFloat available = std::max(CGFloat(1), floor - card.frame.origin.y);
  const CGFloat expected = std::min(available, std::max(CGFloat(160), root.fittingSize.height + 4));
  if (std::abs(card.frame.size.height - expected) > 1 ||
      static_cast<bool>(last.hiddenOrHasHiddenAncestor) == expanded) {
    error = QStringLiteral("mounted disclosure height %1, expected %2 (form %3, available %4, expanded %5, last hidden %6)")
                .arg(card.frame.size.height).arg(expected).arg(root.fittingSize.height).arg(available)
                .arg(expanded).arg(static_cast<bool>(last.hiddenOrHasHiddenAncestor));
    return false;
  }
  if (expanded) {
    [last scrollRectToVisible:last.bounds];
    [content layoutSubtreeIfNeeded];
    if (NSIsEmptyRect(NSIntersectionRect(last.visibleRect, last.bounds))) {
      error = QStringLiteral("expanded disclosure's final input cannot be reached by scrolling");
      return false;
    }
    [more scrollRectToVisible:more.bounds];
  }
  return true;
}

bool widgetRefreshContract(QString& error) {
  QWidget presenter;
  presenter.setAttribute(Qt::WA_DontShowOnScreen);
  auto* layout = new QVBoxLayout(&presenter);
  const auto named = [layout](QWidget* widget, const QString& name) {
    widget->setObjectName(name);
    layout->addWidget(widget);
  };
  auto* caption = new QLabel(QStringLiteral("<b>Initial label</b>"));
  auto* button = new QPushButton(QStringLiteral("&Initial button"));
  auto* check = new QCheckBox(QStringLiteral("Check"));
  auto* edit = new QLineEdit(QStringLiteral("Initial value"));
  auto* spin = new QSpinBox;
  auto* progress = new QProgressBar;
  auto* combo = new QComboBox;
  auto* table = new QTableView;
  auto* tree = new QTreeView;
  named(caption, QStringLiteral("refresh.caption"));
  named(button, QStringLiteral("refresh.button"));
  named(check, QStringLiteral("refresh.check"));
  named(edit, QStringLiteral("refresh.edit"));
  named(spin, QStringLiteral("refresh.spin"));
  named(progress, QStringLiteral("refresh.progress"));
  named(combo, QStringLiteral("refresh.combo"));
  named(table, QStringLiteral("refresh.table"));
  named(tree, QStringLiteral("refresh.tree"));
  combo->addItems({QStringLiteral("First"), QStringLiteral("Second")});
  QStandardItemModel model(2, 1), replacement(1, 1);
  model.setData(model.index(0, 0), QStringLiteral("Row one"));
  model.setData(model.index(1, 0), QStringLiteral("Row two"));
  replacement.setData(replacement.index(0, 0), QStringLiteral("Replacement"));
  table->setModel(&model);
  tree->setModel(&model);
  auto view = createNativeWidgetView(&presenter);
  view->refresh();
  NSView* root = (__bridge NSView*)view->nativeView();
  const auto control = [&root](const QString& name) { return identifiedView(root, name); };
  const auto fail = [&error](const QString& message) { error = message; return false; };
  const auto unchanged = [&view, &fail]() {
    const quint64 before = nativeWidgetPropertyWrites();
    for (int i = 0; i < 3; ++i) { view->refresh(); }
    return nativeWidgetPropertyWrites() == before || fail(QStringLiteral("unchanged widget refresh assigned AppKit properties"));
  };
  if (!unchanged()) { return false; }

  // All of these signals/events must arrive before the slow fallback.
  caption->setText(QStringLiteral("<b>Changed label</b>"));
  button->setText(QStringLiteral("&Changed button"));
  button->setToolTip(QStringLiteral("Changed button tooltip"));
  edit->setText(QStringLiteral("Changed value"));
  edit->setToolTip(QStringLiteral("Changed tooltip"));
  edit->setEnabled(false);
  check->setChecked(true);
  spin->setValue(17);
  combo->setCurrentIndex(1);
  table->selectionModel()->select(model.index(1, 0), QItemSelectionModel::ClearAndSelect | QItemSelectionModel::Rows);
  QCoreApplication::processEvents(QEventLoop::AllEvents);
  root = (__bridge NSView*)view->nativeView();
  NSTextField* field = (NSTextField*)control(QStringLiteral("refresh.edit"));
  NSButton* nativeCheck = (NSButton*)control(QStringLiteral("refresh.check"));
  NSPopUpButton* popup = (NSPopUpButton*)control(QStringLiteral("refresh.combo"));
  NSTextField* nativeCaption = (NSTextField*)control(QStringLiteral("refresh.caption"));
  NSButton* nativeButton = (NSButton*)control(QStringLiteral("refresh.button"));
  if (qt(field.stringValue) != edit->text() || field.enabled || qt(field.toolTip) != edit->toolTip() ||
      nativeCheck.state != NSControlStateValueOn || popup.indexOfSelectedItem != 1 ||
      qt(nativeCaption.stringValue) != QStringLiteral("Changed label") ||
      qt(nativeButton.title) != QStringLiteral("Changed button") || qt(nativeButton.toolTip) != button->toolTip()) {
    return fail(QStringLiteral("presenter signals/events did not refresh native controls"));
  }
  const auto tableIn = [](NSView* parent, const auto& self) -> NSTableView* {
    if ([parent isKindOfClass:NSTableView.class]) { return (NSTableView*)parent; }
    for (NSView* child in parent.subviews) { if (NSTableView* found = self(child, self)) { return found; } }
    return nil;
  };
  NSTableView* nativeTable = tableIn(control(QStringLiteral("refresh.table")), tableIn);
  if (nativeTable.numberOfRows != 2 || nativeTable.selectedRow != 1) {
    return fail(QStringLiteral("model selection signal did not refresh native rows"));
  }
  model.setData(model.index(0, 0), QStringLiteral("Changed row"));
  model.insertRow(2);
  QCoreApplication::processEvents(QEventLoop::AllEvents);
  if (nativeTable.numberOfRows != 3) { return fail(QStringLiteral("model insertion did not refresh native rows")); }
  if (!unchanged()) { return false; }
  table->setRowHidden(0, true);
  view->refresh();
  if (nativeTable.numberOfRows != 2) { return fail(QStringLiteral("silent row hiding was not reflected by an explicit refresh")); }
  table->setRowHidden(0, false);
  view->refresh();

  auto* selection = new QItemSelectionModel(&model, table);
  table->setSelectionModel(selection);
  view->refresh();
  selection->select(model.index(0, 0), QItemSelectionModel::ClearAndSelect | QItemSelectionModel::Rows);
  QCoreApplication::processEvents(QEventLoop::AllEvents);
  if (nativeTable.selectedRow != 0) { return fail(QStringLiteral("replacement selection model was not observed")); }

  // A model and selection-model swap must rebind safely. Updates from the
  // previous model cannot keep dirtying the new table or stale binding IDs.
  table->setModel(&replacement);
  view->refresh();
  replacement.insertRow(1);
  QCoreApplication::processEvents(QEventLoop::AllEvents);
  root = (__bridge NSView*)view->nativeView();
  nativeTable = tableIn(control(QStringLiteral("refresh.table")), tableIn);
  if (nativeTable.numberOfRows != 2) { return fail(QStringLiteral("replacement model signals were not rebound")); }
  auto* embedded = new QLineEdit(QStringLiteral("Cell value"));
  embedded->setObjectName(QStringLiteral("refresh.cell"));
  table->setIndexWidget(replacement.index(0, 0), embedded);
  view->refresh();
  root = (__bridge NSView*)view->nativeView();
  nativeTable = tableIn(control(QStringLiteral("refresh.table")), tableIn);
  [nativeTable reloadData];
  NSView* cell = [nativeTable viewAtColumn:0 row:0 makeIfNecessary:YES];
  if (identifiedView(cell, QStringLiteral("refresh.cell")) == nil) {
    return fail(QStringLiteral("index widget was lost after a presenter-tree change"));
  }
  if (!unchanged()) { return false; }
  delete embedded;
  QCoreApplication::processEvents(QEventLoop::AllEvents);
  if (!unchanged()) { return false; }
  qDebug() << "Native widget refresh: unchanged properties, event updates, model replacement and index widgets verified";
  return true;
}

bool duplicateChoices(CommandHost& host, QString& error) {
  if (!mac::nativeWindowRefreshContract((__bridge void*)documentWindow(), error) || !widgetRefreshContract(error)) { return false; }
  // Exercise the real presenters with synthetic feature UIDs, independently
  // of the geometry needed to produce two identically named features.
  const auto popupIn = [](NSView* root, const auto& self) -> NSPopUpButton* {
    if ([root isKindOfClass:NSPopUpButton.class]) return (NSPopUpButton*)root;
    for (NSView* child in root.subviews) {
      if (NSPopUpButton* popup = self(child, self)) return popup;
    }
    return nil;
  };
  const auto check = [&error](bool passed, const QString& message) {
    if (!passed) error = message;
    return passed;
  };
  const QString first = QStringLiteral("F101");
  const QString second = QStringLiteral("F202");
  CommandDef def;
  def.name = QStringLiteral("Duplicate choices regression");
  def.inputs = {choiceInput(QStringLiteral("feature"), QStringLiteral("Feature"),
                           {{first, QStringLiteral("Same name")}, {second, QStringLiteral("Same name")}}, first)};
  CommandSession session(def, host);
  auto form = createNativeCommandForm(session);
  NSPopUpButton* formPopup = popupIn((__bridge NSView*)form->nativeView(), popupIn);
  if (!check(formPopup != nil && formPopup.numberOfItems == 2 &&
             [[formPopup itemAtIndex:0].title isEqualToString:[formPopup itemAtIndex:1].title],
             QStringLiteral("command form collapsed same-titled choices"))) return false;
  NSMenuItem* firstItem = [formPopup itemAtIndex:0];
  NSMenuItem* secondItem = [formPopup itemAtIndex:1];
  for (NSInteger index : {1, 0}) {
    [formPopup selectItemAtIndex:index];
    [formPopup sendAction:formPopup.action to:formPopup.target];
    const QString expected = index == 0 ? first : second;
    if (!check(session.state().choice(QStringLiteral("feature")) == expected,
               QStringLiteral("command form submitted the wrong feature UID"))) return false;
    for (int refresh = 0; refresh < 3; ++refresh) form->refresh(session.state());
    if (!check([formPopup itemAtIndex:0] == firstItem && [formPopup itemAtIndex:1] == secondItem &&
               qt((NSString*)formPopup.selectedItem.representedObject) == expected,
               QStringLiteral("command form rebuilt unchanged choices or lost the selected UID"))) return false;
  }

  QDialog dialog;
  auto* layout = new QVBoxLayout(&dialog);
  auto* combo = new QComboBox(&dialog);
  layout->addWidget(combo);
  combo->addItem(QStringLiteral("Same name"), first);
  combo->addItem(QStringLiteral("Same name"), second);
  combo->setCurrentIndex(1);
  auto dialogView = createNativeWidgetView(&dialog);
  dialogView->refresh();
  NSPopUpButton* dialogPopup = popupIn((__bridge NSView*)dialogView->nativeView(), popupIn);
  if (!check(dialogPopup != nil && dialogPopup.numberOfItems == 2 && dialogPopup.indexOfSelectedItem == 1 &&
             [[dialogPopup itemAtIndex:0].title isEqualToString:[dialogPopup itemAtIndex:1].title],
             QStringLiteral("dialog binding collapsed same-titled choices or lost its initial selection"))) return false;
  NSMenuItem* dialogFirst = [dialogPopup itemAtIndex:0];
  NSMenuItem* dialogSecond = [dialogPopup itemAtIndex:1];
  int activated = -1;
  QObject::connect(combo, &QComboBox::activated, &dialog, [&activated](int index) { activated = index; });
  for (NSInteger index : {0, 1}) {
    [dialogPopup selectItemAtIndex:index];
    [dialogPopup sendAction:dialogPopup.action to:dialogPopup.target];
    const QString expected = index == 0 ? first : second;
    if (!check(combo->currentData().toString() == expected && activated == index,
               QStringLiteral("dialog popup submitted the wrong feature UID or activated row"))) return false;
    for (int refresh = 0; refresh < 3; ++refresh) dialogView->refresh();
    if (!check([dialogPopup itemAtIndex:0] == dialogFirst && [dialogPopup itemAtIndex:1] == dialogSecond &&
               dialogPopup.indexOfSelectedItem == index,
               QStringLiteral("dialog binding rebuilt unchanged choices or lost its selection"))) return false;
  }
  // A model insertion shifts the persistent identities. Subsequent input
  // must still activate the row carrying the selected feature's data.
  combo->insertItem(0, QStringLiteral("Same name"), QStringLiteral("F303"));
  [dialogPopup selectItemAtIndex:0];
  [dialogPopup sendAction:dialogPopup.action to:dialogPopup.target];
  if (!check(combo->currentData().toString() == first && activated == 1,
             QStringLiteral("dialog popup used a stale menu row after a model insertion"))) return false;
  combo->setCurrentIndex(2);
  dialogView->refresh();
  if (!check(dialogPopup.numberOfItems == 3 && dialogPopup.indexOfSelectedItem == 2,
             QStringLiteral("dialog popup did not follow an inserted same-titled row"))) return false;
  [dialogPopup selectItemAtIndex:1];
  [dialogPopup sendAction:dialogPopup.action to:dialogPopup.target];
  if (!check(combo->currentData().toString() == first && activated == 1,
             QStringLiteral("dialog popup submitted the wrong UID after a model insertion"))) return false;
  combo->setCurrentIndex(-1);
  dialogView->refresh();
  if (!check(dialogPopup.selectedItem == nil,
             QStringLiteral("dialog popup retained a selection for an unset model index"))) return false;
  qDebug() << "Native duplicate choices: both presenters preserved UIDs, duplicate labels and unchanged menu items";
  return true;
}

bool triggerAction(const QString& identifier, QString& error) {
  NSMenuItem* item = findAction(NSApp.mainMenu, identifier);
  if (!item) { error = QStringLiteral("no native menu action '%1'").arg(identifier); return false; }
  if ([item.target respondsToSelector:@selector(validateMenuItem:)] &&
      ![(id<NSMenuItemValidation>)item.target validateMenuItem:item]) {
    error = QStringLiteral("native menu action '%1' is disabled").arg(identifier); return false;
  }
  if (![NSApp sendAction:item.action to:item.target from:item]) {
    error = QStringLiteral("native menu action '%1' could not dispatch").arg(identifier); return false;
  }
  return true;
}

bool triggerMenu(const QString& path, QString& error) {
  const auto normalized = [](QString text) {
    text.remove(QLatin1Char('&'));
    while (text.endsWith(QLatin1Char('.')) || text.endsWith(QChar(0x2026))) text.chop(1);
    return text.trimmed();
  };
  NSMenu* menu = NSApp.mainMenu;
  const QStringList parts = path.split(QLatin1Char('/'), Qt::SkipEmptyParts);
  NSMenuItem* found = nil;
  for (int index = 0; index < parts.size(); ++index) {
    found = nil;
    [menu update];
    for (NSMenuItem* item in menu.itemArray) {
      if (normalized(qt(item.title)) == normalized(parts[index])) { found = item; break; }
    }
    if (!found || (index + 1 < parts.size() && !found.submenu)) {
      error = QStringLiteral("no native menu entry '%1'").arg(path); return false;
    }
    if (index + 1 < parts.size()) menu = found.submenu;
  }
  if (!found || !found.enabled || !found.action || ![NSApp sendAction:found.action to:found.target from:found]) {
    error = QStringLiteral("native menu entry '%1' is disabled or could not dispatch").arg(path); return false;
  }
  return true;
}

bool activate(const QString& identifier, QString& error) {
  NSTableView* table = nil;
  NSInteger row = -1;
  NSView* view = findControl(identifier, &table, &row);
  if (!view) { error = QStringLiteral("no native item '%1'").arg(identifier); return false; }
  if ([view respondsToSelector:@selector(activateHistoryFeature)]) {
    [view scrollRectToVisible:view.bounds];
    [view.window makeKeyAndOrderFront:nil];
    const NSPoint point = [view convertPoint:NSMakePoint(NSMidX(view.bounds), NSMidY(view.bounds)) toView:nil];
    for (NSEventType type : {NSEventTypeLeftMouseDown, NSEventTypeLeftMouseUp}) {
      NSEvent* event = [NSEvent mouseEventWithType:type location:point modifierFlags:0
          timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:view.window.windowNumber
          context:nil eventNumber:0 clickCount:2 pressure:(type == NSEventTypeLeftMouseDown ? 1 : 0)];
      if (type == NSEventTypeLeftMouseDown) [view mouseDown:event];
      else [view mouseUp:event];
    }
    return true;
  }
  for (NSView* parent = view; !table && parent; parent = parent.superview) {
    if ([parent isKindOfClass:NSTableView.class]) { table = (NSTableView*)parent; break; }
  }
  if (row < 0 && table) row = [table rowForView:view];
  if (!table || row < 0 || !table.doubleAction) {
    error = QStringLiteral("native item '%1' is not an activatable table row").arg(identifier); return false;
  }
  [table selectRowIndexes:[NSIndexSet indexSetWithIndex:NSUInteger(row)] byExtendingSelection:NO];
  [table scrollRowToVisible:row];
  return [NSApp sendAction:table.doubleAction to:table.target from:table];
}
bool trackContextMenu(const QString& choice, const std::function<void()>& open, QString& error) {
  __block NSMenu* tracked = nil;
  __block NSTimer* response = nil;
  __block bool began = false;
  __block bool ended = false;
  __block bool selected = false;
  __block QStringList available;
  const bool cancel = choice == QStringLiteral("cancel");
  NSString* title = choice.toNSString();
  NSNotificationCenter* center = NSNotificationCenter.defaultCenter;
  id beginObserver = [center addObserverForName:NSMenuDidBeginTrackingNotification object:nil queue:nil
      usingBlock:^(NSNotification* notice) {
    if (began || ![notice.object isKindOfClass:NSMenu.class]) return;
    began = true;
    tracked = notice.object;
    // The popup owns a nested AppKit tracking loop. Respond in that loop,
    // after the real menu is displayed, rather than dispatching a QAction
    // directly or allowing the script's next step to race its closure.
    response = [NSTimer timerWithTimeInterval:0.05 repeats:NO block:^(NSTimer* timer) {
      (void)timer;
      if (!cancel) {
        [tracked update];
        for (NSInteger index = 0; index < tracked.numberOfItems; ++index) {
          NSMenuItem* item = [tracked itemAtIndex:index];
          available.append(QStringLiteral("%1 (enabled=%2, action=%3, target=%4, submenu=%5)")
              .arg(qt(item.title)).arg(bool(item.enabled)).arg(qt(NSStringFromSelector(item.action)))
              .arg(qt(NSStringFromClass([item.target class]))).arg(bool(item.submenu)));
          if (qt(item.title).remove(QLatin1Char('&')).trimmed() == qt(title).remove(QLatin1Char('&')).trimmed() &&
              item.enabled && item.action && !item.submenu) {
            selected = true;
            [tracked performActionForItemAtIndex:index];
            break;
          }
        }
      }
      [tracked cancelTracking];
    }];
    [NSRunLoop.currentRunLoop addTimer:response forMode:NSEventTrackingRunLoopMode];
    [NSRunLoop.currentRunLoop addTimer:response forMode:NSRunLoopCommonModes];
  }];
  id endObserver = [center addObserverForName:NSMenuDidEndTrackingNotification object:nil queue:nil
      usingBlock:^(NSNotification* notice) {
    if (notice.object == tracked) ended = true;
  }];
  open();
  // Browser and viewport owners intentionally queue their context-menu
  // slots. Let that real delivery open the popup before removing observers.
  QElapsedTimer deadline;
  deadline.start();
  while (!ended && deadline.elapsed() < 6000) {
    QCoreApplication::processEvents(QEventLoop::AllEvents, 25);
    [NSRunLoop.currentRunLoop runMode:NSDefaultRunLoopMode
                          beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
  }
  [response invalidate];
  [center removeObserver:beginObserver];
  [center removeObserver:endObserver];
  if (!began || !ended) {
    error = QStringLiteral("the real native context menu did not begin and finish tracking"); return false;
  }
  if (!cancel && !selected) {
    error = QStringLiteral("native context menu has no enabled leaf action '%1': %2")
        .arg(choice, available.join(QStringLiteral(" | "))); return false;
  }
  qInfo().noquote() << QStringLiteral("Native test: context menu tracked and %1")
      .arg(cancel ? QStringLiteral("cancelled") : QStringLiteral("selected '%1'").arg(choice));
  return true;
}

bool contextMenu(const QString& identifier, const QString& choice, QString& error) {
  NSTableView* table = nil;
  NSInteger row = -1;
  NSView* view = findControl(identifier, &table, &row);
  if (!view || !viewVisible(view)) {
    error = QStringLiteral("no visible native context menu target '%1'").arg(identifier); return false;
  }
  NSView* receiver = table != nil ? table : view;
  [view scrollRectToVisible:view.bounds];
  [receiver.window makeKeyAndOrderFront:nil];
  const NSPoint point = [view convertPoint:NSMakePoint(NSMidX(view.bounds), NSMidY(view.bounds)) toView:nil];
  NSEvent* event = [NSEvent mouseEventWithType:NSEventTypeRightMouseDown location:point modifierFlags:0
      timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:receiver.window.windowNumber
      context:nil eventNumber:0 clickCount:1 pressure:1];
  return trackContextMenu(choice, [receiver, event] {
    NSMenu* menu = [receiver menuForEvent:event];
    if (menu) [NSMenu popUpContextMenu:menu withEvent:event forView:receiver];
  }, error);
}

bool select(const QString& identifier, QString& error, Qt::KeyboardModifiers modifiers) {
  NSTableView* table = nil;
  NSInteger row = -1;
  NSView* view = findControl(identifier, &table, &row);
  if ([view respondsToSelector:@selector(selectHistoryFeature)]) {
    [view scrollRectToVisible:view.bounds];
    [view.window makeKeyAndOrderFront:nil];
    [view.window makeFirstResponder:view];
    if (!modifiers) {
      [(id<MITCADNativeHistoryFeatureTest>)view selectHistoryFeature];
    } else {
      const NSPoint point = [view convertPoint:NSMakePoint(NSMidX(view.bounds), NSMidY(view.bounds)) toView:nil];
      NSEventModifierFlags flags = 0;
      if (modifiers & Qt::ShiftModifier) flags |= NSEventModifierFlagShift;
      if (modifiers & Qt::ControlModifier) flags |= NSEventModifierFlagCommand;
      for (NSEventType type : {NSEventTypeLeftMouseDown, NSEventTypeLeftMouseUp}) {
        NSEvent* event = [NSEvent mouseEventWithType:type location:point modifierFlags:flags
            timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:view.window.windowNumber
            context:nil eventNumber:0 clickCount:1 pressure:(type == NSEventTypeLeftMouseDown ? 1 : 0)];
        if (type == NSEventTypeLeftMouseDown) [view mouseDown:event];
        else [view mouseUp:event];
      }
    }
    return true;
  }
  for (NSView* parent = view; !table && parent; parent = parent.superview) {
    if ([parent isKindOfClass:NSTableView.class]) { table = (NSTableView*)parent; break; }
  }
  if (row < 0 && table) row = [table rowForView:view];
  if (!table || row < 0) { error = QStringLiteral("native item '%1' is not a visible table row").arg(identifier); return false; }
  bool selectable = true;
  if ([table isKindOfClass:NSOutlineView.class]) {
    NSOutlineView* outline = (NSOutlineView*)table;
    if ([outline.delegate respondsToSelector:@selector(outlineView:shouldSelectItem:)])
      selectable = [outline.delegate outlineView:outline shouldSelectItem:[outline itemAtRow:row]];
  } else if ([table.delegate respondsToSelector:@selector(tableView:shouldSelectRow:)]) {
    selectable = [table.delegate tableView:table shouldSelectRow:row];
  }
  if (!selectable) {
    error = QStringLiteral("native item '%1' is not selectable").arg(identifier); return false;
  }
  [table.window makeKeyAndOrderFront:nil];
  [table.window makeFirstResponder:table];
  [table selectRowIndexes:[NSIndexSet indexSetWithIndex:NSUInteger(row)] byExtendingSelection:NO];
  [table scrollRowToVisible:row];
  return true;
}
bool expand(const QString& identifier, QString& error) {
  NSView* view = findControl(identifier);
  if (![view isKindOfClass:NSOutlineView.class]) {
    error = QStringLiteral("native view '%1' is not an outline").arg(identifier); return false;
  }
  [(NSOutlineView*)view expandItem:nil expandChildren:YES];
  return true;
}

bool buttonExists(const QRegularExpression& expression) { return active() && findButton(expression) != nil; }
bool clickButton(const QRegularExpression& expression, QString& error) {
  NSButton* button = findButton(expression);
  if (!button) { error = QStringLiteral("no native button matching '%1'").arg(expression.pattern()); return false; }
  [button scrollRectToVisible:button.bounds];
  [button performClick:nil];
  return true;
}

bool dialogExists(const QRegularExpression& expression) {
  if (!active()) return false;
  for (NSWindow* window in nativeWindows()) if (windowMatches(window, expression)) return true;
  return false;
}
bool focusDialog(const QRegularExpression& expression) {
  for (NSWindow* window in nativeWindows()) {
    if (windowMatches(window, expression)) {
      focusedWindow = window;
      [NSApp activate];
      [window makeKeyAndOrderFront:nil];
      [window makeKeyWindow];
      return true;
    }
  }
  return false;
}
bool replaceFileName(const QString& name, QString& error) {
  NSSavePanel* panel = filePanel();
  if (!panel || [panel isKindOfClass:NSOpenPanel.class]) {
    error = QStringLiteral("no focused native save panel"); return false;
  }
  // Public panel state identifies its real filename input; no private
  // AppKit class names, selectors or view identifiers are assumed.
  NSTextField* field = (NSTextField*)findView(panel.contentView, [&](NSView* view) {
    return [view isKindOfClass:NSTextField.class] && ![view isKindOfClass:NSSearchField.class] &&
           ((NSTextField*)view).editable && [((NSTextField*)view).stringValue isEqualToString:panel.nameFieldStringValue];
  });
  if (!field) {
    // Sandboxed/remote AppKit choosers keep this field in NSRemoteView's
    // service. The public panel property still edits that native chooser;
    // its normal ok: validation supplies the actual URL and result.
    panel.nameFieldStringValue = name.toNSString();
    if (qt(panel.nameFieldStringValue) == name) return true;
    error = QStringLiteral("native save panel rejected its filename"); return false;
  }
  [panel makeKeyAndOrderFront:nil];
  [panel makeFirstResponder:field];
  [field selectText:nil];
  NSTextView* editor = (NSTextView*)field.currentEditor;
  if (!editor) { error = QStringLiteral("native save filename has no field editor"); return false; }
  [editor insertText:name.toNSString() replacementRange:NSMakeRange(0, editor.string.length)];
  return true;
}
bool setFileDirectory(const QString& directory, QString& error) {
  NSSavePanel* panel = filePanel();
  BOOL isDirectory = NO;
  NSString* path = directory.toNSString();
  if (!panel) { error = QStringLiteral("no focused native file panel"); return false; }
  if (!path.absolutePath || ![NSFileManager.defaultManager fileExistsAtPath:path isDirectory:&isDirectory] || !isDirectory) {
    error = QStringLiteral("native file directory '%1' does not exist").arg(directory); return false;
  }
  panel.directoryURL = [NSURL fileURLWithPath:path isDirectory:YES];
  return true;
}
bool cancelFilePanel(QString& error) {
  NSSavePanel* panel = filePanel();
  if (!panel) { error = QStringLiteral("no focused native file panel"); return false; }
  [panel cancel:nil];
  return true;
}
void focusMain() {
  focusedWindow = nil;
  NSWindow* window = documentWindow();
  [window makeKeyAndOrderFront:nil];
  NSView* canvas = findView(window.contentView, [](NSView* view) {
    return isQtResponder(view) && view.acceptsFirstResponder;
  });
  if (canvas) [window makeFirstResponder:canvas];
}

void requestKeyEventFocus() {
  keyEventWindow = nil;
  if (!active()) return;
  // Whole-event checks require actual AppKit activation, not just the Qt
  // state exercised by applicationDelegate(). Only activate our test app;
  // do not alter other applications or synthesize activation notifications.
  keyEventWindow = targetWindow();
  [NSApp activateIgnoringOtherApps:YES];
  [keyEventWindow makeKeyAndOrderFront:nil];
}

bool keyEventFocusReady() {
  return active() && keyEventWindow != nil && NSApp.active &&
         NSApp.keyWindow == keyEventWindow && targetWindow() == keyEventWindow;
}

namespace {
bool sendNativeKey(int key, Qt::KeyboardModifiers modifiers, const QString& text, bool applicationEvent) {
  if (applicationEvent ? !active() : !nativeFocused()) return false;
  NSWindow* window = applicationEvent ? keyEventWindow : targetWindow();
  // Menu equivalents route to NSApp.keyWindow. Synchronize that public
  // AppKit focus with the script's native target after a child sheet closes.
  if (applicationEvent) {
    if (!keyEventFocusReady()) return false;
  } else {
    [NSApp activate];
    [window makeKeyWindow];
  }
  if (applicationEvent) {
    qInfo().noquote() << QStringLiteral("Native application key event: key=%1 target=%2 keyWindow=%3 active=%4 responder=%5")
        .arg(key).arg(qlonglong(window.windowNumber)).arg(qlonglong(NSApp.keyWindow.windowNumber))
        .arg(bool(NSApp.active)).arg(qt(NSStringFromClass(window.firstResponder.class)));
  }
  NSEventModifierFlags flags = 0;
  if (modifiers.testFlag(Qt::ControlModifier)) flags |= NSEventModifierFlagCommand;
  if (modifiers.testFlag(Qt::MetaModifier)) flags |= NSEventModifierFlagControl;
  if (modifiers.testFlag(Qt::AltModifier)) flags |= NSEventModifierFlagOption;
  if (modifiers.testFlag(Qt::ShiftModifier) || key == Qt::Key_Backtab) flags |= NSEventModifierFlagShift;
  // AppKit ignores Command/Control/Option here, but preserves Shift. A
  // lowercase Command-Shift-Z event can match Undo instead of Redo.
  NSString* ignoringModifiers = characters(key, QString());
  if ((flags & NSEventModifierFlagShift) && key >= Qt::Key_A && key <= Qt::Key_Z) {
    ignoringModifiers = ignoringModifiers.uppercaseString;
  }
  NSString* string = text.isEmpty() ? ignoringModifiers : characters(key, text);
  NSEvent* down = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags
        timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber context:nil
        characters:string charactersIgnoringModifiers:ignoringModifiers isARepeat:NO keyCode:virtualKey(key)];
  // The full-event regression leaves all equivalent/responder routing to
  // NSApplication. Ordinary script keys offer the target its equivalents.
  // A secondary window owns Command-W even when an asynchronous sheet close
  // has temporarily restored NSApp.keyWindow to the document.
  if (!applicationEvent && [window performKeyEquivalent:down]) return true;
  // The shell's validation keeps plain CAD keys out of text field editors.
  if (!applicationEvent && [NSApp.mainMenu performKeyEquivalent:down]) return true;
  // Let the real event loop dequeue these keys and establish currentEvent.
  // Native field delegates and Qt's key mapper read its modifier flags.
  [NSApp postEvent:down atStart:NO];
  NSEvent* up = [NSEvent keyEventWithType:NSEventTypeKeyUp location:NSZeroPoint modifierFlags:flags
        timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber context:nil
        characters:string charactersIgnoringModifiers:ignoringModifiers isARepeat:NO keyCode:virtualKey(key)];
  [NSApp postEvent:up atStart:NO];
  return true;
}
} // namespace

bool sendKey(int key, Qt::KeyboardModifiers modifiers, const QString& text) {
  return sendNativeKey(key, modifiers, text, false);
}

bool sendKeyEvent(int key, Qt::KeyboardModifiers modifiers, const QString& text) {
  return sendNativeKey(key, modifiers, text, true);
}

bool sendText(const QString& text) {
  if (!nativeFocused()) return false;
  NSResponder* responder = targetWindow().firstResponder;
  if ([responder conformsToProtocol:@protocol(NSTextInputClient)]) {
    [(id<NSTextInputClient>)responder insertText:text.toNSString() replacementRange:NSMakeRange(NSNotFound, 0)];
    return true;
  }
  return false;
}

bool screenshot(const QString& path, QOpenGLWidget* viewport) {
  if (!active() || !captureOwnWindows(path)) return false;
  if (!viewport) return true;
  // The window server may omit an embedded Qt/OpenGL surface, especially
  // with the VM's software GL. Keep the real AppKit capture and insert the
  // public framebuffer at the same screen rectangle, preserving any native
  // windows above the document rather than covering their controls.
  QImage shot(path);
  const QImage frame = viewport->grabFramebuffer();
  if (shot.isNull() || frame.isNull()) return false;
  CGRect bounds = CGRectNull;
  QRegion covered;
  bool aboveDocument = true;
  const NSInteger documentNumber = documentWindow().windowNumber;
  NSArray* infos = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID));
  for (NSDictionary* info in infos) {
    if ([info[(id)kCGWindowOwnerPID] intValue] != getpid()) continue;
    CGRect rect;
    if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)info[(id)kCGWindowBounds], &rect) ||
        rect.size.width < 8 || rect.size.height < 8) continue;
    bounds = CGRectUnion(bounds, rect);
    const NSInteger number = [info[(id)kCGWindowNumber] integerValue];
    if (number == documentNumber) aboveDocument = false;
    else if (aboveDocument) covered += QRect(qFloor(rect.origin.x), qFloor(rect.origin.y),
                                           qCeil(rect.size.width), qCeil(rect.size.height));
  }
  if (CGRectIsNull(bounds)) return false;
  bounds = CGRectInset(bounds, -48, -48); // same shadow margin as captureOwnWindows
  const QPoint origin = viewport->mapToGlobal(QPoint());
  const QRect canvas(origin, viewport->size());
  NSView* content = documentWindow().contentView;
  NSView* host = identifiedView(content, QStringLiteral("native.viewportHost"));
  if (host && host.bounds.size.width > 0 && host.bounds.size.height > 0) {
    // The model, inspector and ribbon float inside this same NSWindow.
    // Preserve their real AppKit capture when inserting the underlying
    // OpenGL framebuffer; separate-window exclusion alone is insufficient.
    const CGFloat scaleX = viewport->width() / host.bounds.size.width;
    const CGFloat scaleY = viewport->height() / host.bounds.size.height;
    for (NSString* identifier in @[@"native.sidebar", @"native.inspectorCard", @"native.historyCard", @"native.ribbon", @"native.statusCard"]) {
      NSView* overlay = identifiedView(content, qt(identifier));
      if (!viewVisible(overlay)) continue;
      NSRect rect = [overlay convertRect:overlay.bounds toView:host];
      if (!host.flipped) rect.origin.y = NSMaxY(host.bounds) - NSMaxY(rect);
      covered += QRect(origin.x() + qFloor(rect.origin.x * scaleX), origin.y() + qFloor(rect.origin.y * scaleY),
                       qCeil(rect.size.width * scaleX), qCeil(rect.size.height * scaleY));
    }
  }
  QPainter painter(&shot);
  painter.scale(shot.width() / bounds.size.width, shot.height() / bounds.size.height);
  painter.translate(-bounds.origin.x, -bounds.origin.y);
  painter.setClipRegion(QRegion(canvas).subtracted(covered));
  painter.drawImage(canvas, frame);
  // CAD overlays remain Qt-painted content by design; precision fields are
  // hidden presenters and their visible AppKit child panels were protected
  // by the covered region above.
  for (QWidget* overlay : viewport->findChildren<QWidget*>(Qt::FindDirectChildrenOnly)) {
    if (overlay->isVisibleTo(viewport))
      overlay->render(&painter, overlay->mapToGlobal(QPoint()), QRegion(), QWidget::DrawChildren);
  }
  painter.end();
  return shot.save(path);
}

} // namespace mitcad::mac::test
