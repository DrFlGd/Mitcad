// SPDX-License-Identifier: MIT
#include "NativeWindowFit.hpp"

#import <objc/runtime.h>

#include <algorithm>

// The content view of a secondary window, with its parts for the fitting.
@interface MitcadNativeWindowContent : NSView
@property(nonatomic, strong) NSView* body;
@property(nonatomic, strong) NSScrollView* scroll;
@property(nonatomic, strong) NSView* footer;
@end

@implementation MitcadNativeWindowContent
@end

// A flipped document: a long body starts at its top, not at the bottom of
// AppKit's scroll range.
@interface MitcadNativeWindowDocument : NSView
@end

@implementation MitcadNativeWindowDocument
- (BOOL)isFlipped { return YES; }
@end

// Follows display changes for one window; the window's associated object.
@interface MitcadNativeWindowFit : NSObject {
@public
  __weak NSWindow* window;
}
- (void)screenChanged:(NSNotification*)notification;
@end

namespace mitcad {
namespace {

NSSize g_testLimit = NSZeroSize;
char g_fitKey = 0;

NSRect availableFrame(NSWindow* window, NSWindow* sheetHost) {
  NSScreen* screen = sheetHost != nil ? sheetHost.screen : window.screen;
  if (screen == nil) {
    screen = NSScreen.mainScreen;
  }
  NSRect area = screen != nil ? screen.visibleFrame : NSMakeRect(0, 0, 1280, 800);
  if (g_testLimit.width > 0 && g_testLimit.height > 0) {
    const CGFloat height = std::min(area.size.height, g_testLimit.height);
    area = NSMakeRect(NSMinX(area), NSMaxY(area) - height, std::min(area.size.width, g_testLimit.width), height);
  }
  if (sheetHost != nil) {
    // A sheet hangs from its host's content area, below the title bar and
    // the toolbar.
    const NSRect content = [sheetHost convertRectToScreen:sheetHost.contentLayoutRect];
    const CGFloat top = std::min(NSMaxY(area), NSMaxY(content));
    area.size.height = std::max<CGFloat>(0, top - NSMinY(area));
  }
  return area;
}

// `value` within `low` and `high`; `low` when the range is empty.
CGFloat within(CGFloat value, CGFloat low, CGFloat high) { return std::max(low, std::min(value, high)); }

// `frame` no larger than `area`, moved inside it.
NSRect inside(NSRect frame, NSRect area) {
  frame.size.width = std::min(frame.size.width, area.size.width);
  frame.size.height = std::min(frame.size.height, area.size.height);
  frame.origin.x = within(frame.origin.x, NSMinX(area), NSMaxX(area) - frame.size.width);
  frame.origin.y = within(frame.origin.y, NSMinY(area), NSMaxY(area) - frame.size.height);
  return frame;
}

// The title bar and borders around a window's content.
NSSize chromeOf(NSWindow* window) {
  const NSRect frame = [window frameRectForContentRect:NSMakeRect(0, 0, 400, 400)];
  return NSMakeSize(std::max<CGFloat>(0, frame.size.width - 400), std::max<CGFloat>(0, frame.size.height - 400));
}

// The content size the body and the buttons need at `width`: the body at
// least as wide as its controls require.
NSSize naturalSize(MitcadNativeWindowContent* content, CGFloat width) {
  NSView* body = content.body;
  NSLayoutConstraint* probe = [body.widthAnchor constraintEqualToConstant:width];
  probe.priority = NSLayoutPriorityRequired - 1;
  probe.active = YES;
  const NSSize size = body.fittingSize;
  probe.active = NO;
  const CGFloat footer = content.footer.hidden ? 0 : content.footer.fittingSize.height;
  return NSMakeSize(size.width, size.height + footer);
}

// A window made smaller and moved inside its available frame where needed.
void keepOnScreen(NSWindow* window) {
  NSWindow* host = window.sheetParent;
  const NSRect area = availableFrame(window, host);
  if (host != nil) {
    // AppKit places a sheet; only its size is ours.
    const NSSize excess = NSMakeSize(std::max<CGFloat>(0, window.frame.size.width - area.size.width),
                                     std::max<CGFloat>(0, window.frame.size.height - area.size.height));
    if (excess.width > 0 || excess.height > 0) {
      const NSSize content = [window contentRectForFrameRect:window.frame].size;
      [window setContentSize:NSMakeSize(std::max<CGFloat>(0, content.width - excess.width),
                                        std::max<CGFloat>(0, content.height - excess.height))];
    }
    return;
  }
  const NSRect frame = inside(window.frame, area);
  if (!NSEqualRects(frame, window.frame)) {
    [window setFrame:frame display:YES];
  }
}

void followScreens(NSWindow* window) {
  if (objc_getAssociatedObject(window, &g_fitKey) != nil) {
    return;
  }
  MitcadNativeWindowFit* fit = [MitcadNativeWindowFit new];
  fit->window = window;
  NSNotificationCenter* center = NSNotificationCenter.defaultCenter;
  [center addObserver:fit selector:@selector(screenChanged:)
                 name:NSApplicationDidChangeScreenParametersNotification object:nil];
  [center addObserver:fit selector:@selector(screenChanged:) name:NSWindowDidChangeScreenNotification object:window];
  objc_setAssociatedObject(window, &g_fitKey, fit, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
}

} // namespace

NSView* nativeScrollingContent(NSView* body, NSView* footer) {
  MitcadNativeWindowContent* content = [[MitcadNativeWindowContent alloc] initWithFrame:NSMakeRect(0, 0, 480, 320)];
  content.accessibilityIdentifier = @"native.secondary.content";
  MitcadNativeWindowDocument* document = [[MitcadNativeWindowDocument alloc] initWithFrame:NSZeroRect];
  document.translatesAutoresizingMaskIntoConstraints = NO;
  body.translatesAutoresizingMaskIntoConstraints = NO;
  [document addSubview:body];
  NSScrollView* scroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
  scroll.translatesAutoresizingMaskIntoConstraints = NO;
  scroll.hasVerticalScroller = YES;
  scroll.hasHorizontalScroller = YES;
  scroll.autohidesScrollers = YES;
  scroll.borderType = NSNoBorder;
  scroll.drawsBackground = NO;
  scroll.accessibilityIdentifier = @"native.secondary.body";
  scroll.documentView = document;
  footer.translatesAutoresizingMaskIntoConstraints = NO;
  [content addSubview:scroll];
  [content addSubview:footer];
  NSClipView* clip = scroll.contentView;
  // As wide as the window unless the controls require more; at least as
  // tall as the window, so that the body fills it as it did unscrolled.
  NSLayoutConstraint* windowWidth = [document.widthAnchor constraintEqualToAnchor:clip.widthAnchor];
  windowWidth.priority = NSLayoutPriorityDefaultHigh;
  // The buttons take their own height, the body the rest.
  NSLayoutConstraint* footerHeight = [footer.heightAnchor constraintEqualToConstant:0];
  footerHeight.priority = NSLayoutPriorityDefaultLow - 50;
  [NSLayoutConstraint activateConstraints:@[
    [body.leadingAnchor constraintEqualToAnchor:document.leadingAnchor],
    [body.trailingAnchor constraintEqualToAnchor:document.trailingAnchor],
    [body.topAnchor constraintEqualToAnchor:document.topAnchor],
    [body.bottomAnchor constraintEqualToAnchor:document.bottomAnchor],
    [document.leadingAnchor constraintEqualToAnchor:clip.leadingAnchor],
    [document.topAnchor constraintEqualToAnchor:clip.topAnchor],
    [document.widthAnchor constraintGreaterThanOrEqualToAnchor:clip.widthAnchor],
    windowWidth,
    [document.heightAnchor constraintGreaterThanOrEqualToAnchor:clip.heightAnchor],
    [scroll.leadingAnchor constraintEqualToAnchor:content.leadingAnchor],
    [scroll.trailingAnchor constraintEqualToAnchor:content.trailingAnchor],
    [scroll.topAnchor constraintEqualToAnchor:content.topAnchor],
    [footer.leadingAnchor constraintEqualToAnchor:content.leadingAnchor],
    [footer.trailingAnchor constraintEqualToAnchor:content.trailingAnchor],
    [footer.topAnchor constraintEqualToAnchor:scroll.bottomAnchor],
    [footer.bottomAnchor constraintEqualToAnchor:content.bottomAnchor],
    footerHeight
  ]];
  if ([footer isKindOfClass:NSStackView.class]) {
    [(NSStackView*)footer setHuggingPriority:NSLayoutPriorityDefaultHigh
                              forOrientation:NSLayoutConstraintOrientationVertical];
  }
  content.body = body;
  content.scroll = scroll;
  content.footer = footer;
  return content;
}

void fitNativeWindowToScreen(NSWindow* window, NSWindow* sheetHost) {
  if (window == nil) {
    return;
  }
  const NSRect placed = window.frame;
  const NSRect area = availableFrame(window, sheetHost);
  const NSSize chrome = chromeOf(window);
  const NSSize most = NSMakeSize(std::max<CGFloat>(0, area.size.width - chrome.width),
                                 std::max<CGFloat>(0, area.size.height - chrome.height));
  const NSSize requested = [window contentRectForFrameRect:placed].size;
  NSSize size = NSMakeSize(std::min(requested.width, most.width), std::min(requested.height, most.height));
  if ([window.contentView isKindOfClass:MitcadNativeWindowContent.class]) {
    auto* content = (MitcadNativeWindowContent*)window.contentView;
    // Wrapping text takes its height at the width it will have.
    [window setContentSize:size];
    [content layoutSubtreeIfNeeded];
    const NSSize natural = naturalSize(content, size.width);
    size = NSMakeSize(std::min(std::max(size.width, natural.width), most.width),
                      std::min(std::max(requested.height, natural.height), most.height));
  }
  if (sheetHost != nil) {
    [window setContentSize:size];
  } else {
    NSRect frame = [window frameRectForContentRect:NSMakeRect(0, 0, size.width, size.height)];
    frame.origin = NSMakePoint(NSMidX(placed) - frame.size.width / 2, NSMidY(placed) - frame.size.height / 2);
    [window setFrame:inside(frame, area) display:YES];
  }
  followScreens(window);
}

NSRect nativeWindowAvailableFrame(NSWindow* window) {
  return window == nil ? NSZeroRect : availableFrame(window, window.sheetParent);
}

void setNativeTestScreenLimit(NSSize limit) { g_testLimit = limit; }

} // namespace mitcad

@implementation MitcadNativeWindowFit
- (void)dealloc {
  [NSNotificationCenter.defaultCenter removeObserver:self];
}
- (void)screenChanged:(NSNotification*)notification {
  (void)notification;
  NSWindow* target = window;
  if (target != nil && target.visible) {
    mitcad::keepOnScreen(target);
  }
}
@end
