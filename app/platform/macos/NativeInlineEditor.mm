// SPDX-License-Identifier: MIT
#include "platform/NativeInlineEditor.hpp"

#import <AppKit/AppKit.h>

#include <algorithm>
#include <vector>

#include <QAction>
#include <QCoreApplication>
#include <QEvent>
#include <QHash>
#include <QKeyEvent>
#include <QKeySequence>
#include <QLineEdit>
#include <QPointer>
#include <QTimer>
#include <QVariant>
#include <QWidget>

#include "platform/NativeDialogs.hpp"

namespace mitcad { class NativeInlineController; }

@interface MITCADInlinePanel : NSPanel
@end
@implementation MITCADInlinePanel
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

@interface MITCADInlineEditorTarget : NSObject <NSTextFieldDelegate> {
@public
  mitcad::NativeInlineController* owner;
}
@end

namespace mitcad {
namespace {

QHash<const QLineEdit*, NativeInlineController*> editors;
std::vector<NativeInlineController*> editorOrder;

NSWindow* hostWindow(QLineEdit* source) {
  // Ask for the existing viewport root's public handle, never for the
  // hidden QLineEdit's winId (which would create a second native control).
  QWidget* root = source->window();
  NSView* view = (__bridge NSView*)reinterpret_cast<void*>(root->winId());
  return view.window;
}

} // namespace

class NativeInlineController final : public QObject {
public:
  explicit NativeInlineController(QLineEdit* source)
      : QObject(source), m_source(source), m_key(source), m_viewport(source->parentWidget()) {
    m_target = [[MITCADInlineEditorTarget alloc] init];
    m_target->owner = this;
    m_field = [[NSTextField alloc] initWithFrame:NSZeroRect];
    m_field.editable = YES;
    m_field.selectable = YES;
    m_field.bezeled = YES;
    m_field.bezelStyle = NSTextFieldRoundedBezel;
    m_field.font = [NSFont systemFontOfSize:NSFont.systemFontSize];
    m_field.delegate = m_target;
    m_field.accessibilityIdentifier = source->objectName().toNSString();
    m_field.identifier = source->objectName().toNSString();
    m_field.accessibilityLabel = source->placeholderText().toNSString();
    m_field.translatesAutoresizingMaskIntoConstraints = NO;

    m_panel = [[MITCADInlinePanel alloc] initWithContentRect:NSMakeRect(0, 0, 110, 26)
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
        backing:NSBackingStoreBuffered defer:NO];
    m_panel.releasedWhenClosed = NO;
    m_panel.opaque = NO;
    m_panel.backgroundColor = NSColor.clearColor;
    m_panel.hasShadow = YES;
    m_panel.hidesOnDeactivate = YES;
    m_panel.becomesKeyOnlyIfNeeded = YES;
    m_panel.floatingPanel = NO;
    m_panel.excludedFromWindowsMenu = YES;
    m_panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary |
        NSWindowCollectionBehaviorTransient | NSWindowCollectionBehaviorIgnoresCycle;
    m_panel.identifier = (QStringLiteral("native.inline.") + source->objectName()).toNSString();
    NSView* content = [[NSView alloc] initWithFrame:NSZeroRect];
    m_panel.contentView = content;
    [content addSubview:m_field];
    [NSLayoutConstraint activateConstraints:@[
      [m_field.leadingAnchor constraintEqualToAnchor:content.leadingAnchor],
      [m_field.trailingAnchor constraintEqualToAnchor:content.trailingAnchor],
      [m_field.topAnchor constraintEqualToAnchor:content.topAnchor],
      [m_field.bottomAnchor constraintEqualToAnchor:content.bottomAnchor]
    ]];
    m_host = hostWindow(source);
    [m_host addChildWindow:m_panel ordered:NSWindowAbove];
    source->hide();
    source->installEventFilter(this);
    if (m_viewport) m_viewport->installEventFilter(this);
    connect(source, &QLineEdit::textChanged, this, [this] { refresh(true); });
    connect(source, &QLineEdit::selectionChanged, this, [this] { syncSelection(); });
    // A native host move, backing-scale change or a viewport-only resize
    // does not always move the hidden presenter; keep child geometry tied
    // to its public mapToGlobal rectangle as the window is manipulated.
    m_timer.setInterval(50);
    connect(&m_timer, &QTimer::timeout, this, [this] { refresh(false); });
    m_timer.start();
    editorOrder.push_back(this);
    editors.insert(source, this);
    refresh(false);
    [m_panel orderFront:nil];
  }

  ~NativeInlineController() override {
    retire(false);
    [m_panel close];
  }

  void retire(bool restoreFocus) {
    if (m_retired) return;
    m_retired = true;
    m_timer.stop();
    editors.remove(m_key);
    editorOrder.erase(std::remove(editorOrder.begin(), editorOrder.end(), this), editorOrder.end());
    m_target->owner = nullptr;
    m_field.delegate = nil;
    if (m_source) {
      m_source->removeEventFilter(this);
      disconnect(m_source, nullptr, this, nullptr);
    }
    if (m_viewport) m_viewport->removeEventFilter(this);
    const bool wasKey = m_panel.keyWindow;
    [m_host removeChildWindow:m_panel];
    [m_panel orderOut:nil];
    if (restoreFocus && wasKey && m_viewport && m_host.visible) {
      [m_host makeKeyWindow];
      NSView* root = (__bridge NSView*)reinterpret_cast<void*>(m_viewport->window()->winId());
      [m_host makeFirstResponder:root];
    }
    m_source = nullptr;
    m_viewport = nullptr;
  }

  bool focused() const { return m_panel.keyWindow && m_field.currentEditor != nil; }

  void focus() {
    if (!m_source) return;
    refresh(false);
    [m_panel makeKeyAndOrderFront:nil];
    [m_panel makeFirstResponder:m_field];
    [m_field selectText:nil];
    m_source->selectAll();
  }

  void edited() {
    if (!m_source || m_refreshing) return;
    QPointer<NativeInlineController> guard(this);
    QPointer<QLineEdit> source = m_source;
    const QString text = QString::fromNSString(m_field.stringValue);
    m_refreshing = true;
    source->setText(text);
    if (!guard || m_retired || !source) return;
    emit source->textEdited(text);
    if (!guard || m_retired) return;
    m_refreshing = false;
    refresh(false);
  }

  void committed() {
    if (m_source && !m_refreshing) emit m_source->editingFinished();
  }

  bool key(int key, Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
    QPointer<QLineEdit> source = m_source;
    if (!source) return true;
    if (key == Qt::Key_Tab || key == Qt::Key_Backtab) {
      // Preserve the sketch controller's Tab behavior, then give the same
      // next presenter its real AppKit field editor.
      QKeyEvent event(QEvent::KeyPress, key, modifiers, QStringLiteral("\t"));
      QPointer<NativeInlineController> guard(this);
      QCoreApplication::sendEvent(source, &event);
      if (!guard || m_retired) return true;
      std::vector<NativeInlineController*> group;
      for (auto* editor : editorOrder) {
        if (editor->m_source && editor->m_viewport == m_viewport &&
            editor->m_source->objectName().startsWith(QStringLiteral("sketchField_"))) group.push_back(editor);
      }
      const auto found = std::find(group.begin(), group.end(), this);
      if (found != group.end() && !group.empty()) {
        const int offset = key == Qt::Key_Backtab ? -1 : 1;
        const int index = int(found - group.begin());
        group[size_t((index + offset + int(group.size())) % int(group.size()))]->focus();
      }
      return true;
    }
    const QKeySequence shortcut(QKeyCombination(modifiers, Qt::Key(key)));
    QWidget* root = source->window();
    for (QAction* action : root->actions()) {
      if (action->isEnabled() && action->shortcuts().contains(shortcut)) {
        action->trigger();
        return true;
      }
    }
    // A caller outside SketchController may use the ordinary QLineEdit
    // signals directly instead of a window-wide confirmation action.
    if (key == Qt::Key_Return || key == Qt::Key_Enter) {
      emit source->returnPressed();
      if (source) emit source->editingFinished();
      return true;
    }
    QKeyEvent event(QEvent::KeyPress, key, modifiers);
    QCoreApplication::sendEvent(source, &event);
    return true;
  }

protected:
  bool eventFilter(QObject* watched, QEvent* event) override {
    if (watched == m_source && event->type() == QEvent::Hide) {
      [m_panel orderOut:nil];
    } else if (event->type() == QEvent::Move || event->type() == QEvent::Resize ||
               event->type() == QEvent::ToolTipChange || event->type() == QEvent::DynamicPropertyChange) {
      refresh(false);
    }
    return false;
  }

private:
  void refresh(bool liveText) {
    if (!m_source || m_refreshing) return;
    m_refreshing = true;
    const QString text = m_source->text();
    if (QString::fromNSString(m_field.stringValue) != text) {
      m_field.stringValue = text.toNSString();
      if (liveText && focused()) [m_field selectText:nil];
    }
    m_field.placeholderString = m_source->placeholderText().toNSString();
    m_field.toolTip = m_source->toolTip().toNSString();
    m_field.enabled = m_source->isEnabled();
    m_field.textColor = m_source->property("nativeInlineInvalid").toBool() ? NSColor.systemRedColor : NSColor.labelColor;
    const QPoint at = m_source->mapToGlobal(QPoint());
    const CGFloat top = NSScreen.screens.firstObject.frame.origin.y + NSScreen.screens.firstObject.frame.size.height;
    const CGFloat width = std::max(60, m_source->width());
    const CGFloat height = std::max(24, m_source->sizeHint().height());
    const NSRect frame = NSMakeRect(at.x(), top - at.y() - height, width, height);
    if (!NSEqualRects(frame, m_panel.frame)) [m_panel setFrame:frame display:YES];
    m_refreshing = false;
  }

  void syncSelection() {
    if (!m_source || m_refreshing || m_field.currentEditor == nil) return;
    const int start = m_source->selectionStart();
    NSTextView* editor = (NSTextView*)m_field.currentEditor;
    editor.selectedRange = start < 0 ? NSMakeRange(NSUInteger(m_source->cursorPosition()), 0) :
                                      NSMakeRange(NSUInteger(start), NSUInteger(m_source->selectedText().size()));
  }

  QPointer<QLineEdit> m_source;
  const QLineEdit* m_key;
  QPointer<QWidget> m_viewport;
  QTimer m_timer;
  bool m_refreshing = false;
  bool m_retired = false;
  MITCADInlineEditorTarget* __strong m_target = nil;
  NSTextField* __strong m_field = nil;
  MITCADInlinePanel* __strong m_panel = nil;
  NSWindow* __weak m_host = nil;
};

bool showNativeInlineEditor(QLineEdit* presenter) {
  if (!presenter || !nativeDialogsActive() || hostWindow(presenter) == nil) return false;
  if (!editors.contains(presenter)) new NativeInlineController(presenter);
  return true;
}
bool focusNativeInlineEditor(QLineEdit* presenter) {
  if (!showNativeInlineEditor(presenter)) return false;
  editors.value(presenter)->focus();
  return true;
}
bool nativeInlineEditorFocused(const QLineEdit* presenter) {
  const auto found = editors.constFind(presenter);
  return found != editors.cend() && found.value()->focused();
}
void hideNativeInlineEditor(QLineEdit* presenter) {
  if (auto* editor = editors.value(presenter)) {
    // Enter/Escape can synchronously remove the model's fields while an
    // AppKit delegate method is on the stack. Detach immediately but leave
    // the native editor alive until that event has returned.
    editor->retire(true);
    editor->deleteLater();
  }
}

} // namespace mitcad

@implementation MITCADInlineEditorTarget
- (void)controlTextDidChange:(NSNotification*)notification { (void)notification; if (owner) owner->edited(); }
- (void)controlTextDidEndEditing:(NSNotification*)notification { (void)notification; if (owner) owner->committed(); }
- (BOOL)control:(NSControl*)control textView:(NSTextView*)view doCommandBySelector:(SEL)command {
  (void)control; (void)view;
  if (!owner) return NO;
  if (command == @selector(insertNewline:) || command == @selector(insertNewlineIgnoringFieldEditor:)) {
    const NSEventModifierFlags flags = NSApp.currentEvent.modifierFlags;
    Qt::KeyboardModifiers modifiers = Qt::NoModifier;
    if (flags & NSEventModifierFlagCommand) modifiers |= Qt::ControlModifier;
    if (flags & NSEventModifierFlagControl) modifiers |= Qt::MetaModifier;
    if (flags & NSEventModifierFlagOption) modifiers |= Qt::AltModifier;
    if (flags & NSEventModifierFlagShift) modifiers |= Qt::ShiftModifier;
    return owner->key(Qt::Key_Return, modifiers);
  }
  if (command == @selector(cancelOperation:)) return owner->key(Qt::Key_Escape);
  if (command == @selector(insertTab:)) return owner->key(Qt::Key_Tab);
  if (command == @selector(insertBacktab:)) return owner->key(Qt::Key_Backtab, Qt::ShiftModifier);
  return NO;
}
@end
