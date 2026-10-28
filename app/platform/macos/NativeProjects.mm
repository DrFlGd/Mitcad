// SPDX-License-Identifier: MIT
#include "NativeProjects.hpp"
#include "NativeKeys.hpp"

#include <utility>

#include <QKeySequence>
#include <QDebug>

// One row's text field, edited in place and restored when the name ends.
@interface MITCADProjectsNameEditor : NSObject <NSTextFieldDelegate> {
@public
  NSTextField* __weak field;
  NSOutlineView* __weak outline;
  id<NSTextFieldDelegate> __weak previousDelegate;
  NSString* original;
  NSColor* previousBackground;
  NSColor* previousTextColor;
  NSLineBreakMode previousLineBreak;
  BOOL previousEditable;
  BOOL previousSelectable;
  BOOL previousDrawsBackground;
  BOOL previousWraps;
  BOOL previousScrollable;
  BOOL previousSingleLine;
  BOOL finishing;
  BOOL sessionEnded; // the field already lost the keyboard
  std::function<void(const QString&, bool)> finished;
}
- (void)finishTaking:(BOOL)accepted text:(NSString*)text focus:(BOOL)focus notify:(BOOL)notify;
@end

namespace {

// At most one Projects name is typed at a time; the editor lives until its
// name ends, whichever way.
MITCADProjectsNameEditor* __strong g_activeEditor = nil;

QString qt(NSString* text) { return QString::fromNSString(text != nil ? text : @""); }

} // namespace

@implementation MITCADProjectsNameEditor
- (void)finishTaking:(BOOL)accepted text:(NSString*)text focus:(BOOL)focus notify:(BOOL)notify {
  if (g_activeEditor != self) return;
  // The last strong reference goes below; this one keeps the editor until
  // the end of its own method.
  NS_VALID_UNTIL_END_OF_SCOPE MITCADProjectsNameEditor* keep = self;
  g_activeEditor = nil;
  NSTextField* edited = field;
  NSOutlineView* rows = outline;
  NSWindow* window = edited.window;
  finishing = YES;
  // End the field editor's session while its end is ignored here, so that
  // the shared cell never takes the text as a model edit of its own; a
  // session AppKit already ended is left to the control taking the keyboard.
  if (!sessionEnded && edited != nil && edited.currentEditor != nil && window != nil) {
    NSResponder* next = (focus && rows.window == window) ? rows : nil;
    if (![window makeFirstResponder:next]) {
      [window endEditingFor:edited];
      if (next != nil) [window makeFirstResponder:next];
    }
  } else if (focus && rows.window != nil) {
    [rows.window makeFirstResponder:rows];
  }
  if (edited != nil) {
    // The name stays shown until the panel reads the project again.
    edited.stringValue = accepted ? text : original;
    edited.delegate = previousDelegate;
    edited.editable = previousEditable;
    edited.selectable = previousSelectable;
    edited.drawsBackground = previousDrawsBackground;
    edited.backgroundColor = previousBackground;
    edited.textColor = previousTextColor;
    edited.lineBreakMode = previousLineBreak;
    NSCell* cell = edited.cell;
    cell.usesSingleLineMode = previousSingleLine;
    cell.scrollable = previousScrollable;
    cell.wraps = previousWraps;
  }
  finishing = NO;
  qInfo().noquote() << QStringLiteral("Native Projects editor: %1").arg(accepted ? QStringLiteral("taken") : QStringLiteral("cancelled"));
  if (notify && finished) {
    const auto call = std::move(finished);
    finished = nullptr;
    call(qt(text), accepted);
  }
  (void)keep;
}
- (BOOL)control:(NSControl*)control textView:(NSTextView*)textView doCommandBySelector:(SEL)selector {
  (void)control;
  if (finishing || g_activeEditor != self) return NO;
  if (selector == @selector(insertNewline:) || selector == @selector(insertNewlineIgnoringFieldEditor:) ||
      selector == @selector(insertTab:) || selector == @selector(insertBacktab:)) {
    [self finishTaking:YES text:[textView.string copy] focus:YES notify:YES];
    return YES;
  }
  if (selector == @selector(cancelOperation:)) {
    [self finishTaking:NO text:original focus:YES notify:YES];
    return YES;
  }
  return NO;
}
- (void)controlTextDidEndEditing:(NSNotification*)notification {
  (void)notification;
  // The field lost the keyboard to another control: the name is taken, as
  // the default tree's editor takes it when it loses the focus.
  if (finishing || g_activeEditor != self) return;
  sessionEnded = YES;
  NSTextField* edited = field;
  [self finishTaking:YES text:edited != nil ? edited.stringValue : original focus:NO notify:YES];
}
@end

namespace mitcad::mac {

bool beginProjectsNameEditor(NSOutlineView* outline, const QString& identifier, const QString& text,
                             std::function<void(const QString&, bool)> finished) {
  closeProjectsNameEditor();
  if (outline == nil || outline.window == nil || identifier.isEmpty()) return false;
  NSString* wanted = identifier.toNSString();
  NSInteger row = -1;
  for (NSInteger index = 0; index < outline.numberOfRows && row < 0; ++index) {
    NSView* cell = [outline viewAtColumn:0 row:index makeIfNecessary:YES];
    if ([cell.accessibilityIdentifier isEqualToString:wanted]) row = index;
  }
  if (row < 0) {
    qWarning().noquote() << QStringLiteral("Native Projects editor: no row %1").arg(identifier);
    return false;
  }
  if (outline.selectedRow != row) {
    [outline selectRowIndexes:[NSIndexSet indexSetWithIndex:NSUInteger(row)] byExtendingSelection:NO];
  }
  // The row in sight, and its live cell rather than one prepared offscreen.
  [outline scrollRowToVisible:row];
  [outline layoutSubtreeIfNeeded];
  NSView* view = [outline viewAtColumn:0 row:row makeIfNecessary:YES];
  NSTextField* field = [view isKindOfClass:NSTableCellView.class] ? ((NSTableCellView*)view).textField : nil;
  if (field == nil) {
    qWarning().noquote() << QStringLiteral("Native Projects editor: row %1 has no text").arg(identifier);
    return false;
  }
  MITCADProjectsNameEditor* editor = [MITCADProjectsNameEditor new];
  editor->field = field;
  editor->outline = outline;
  editor->previousDelegate = field.delegate;
  editor->original = [field.stringValue copy];
  editor->previousBackground = field.backgroundColor;
  editor->previousTextColor = field.textColor;
  editor->previousLineBreak = field.lineBreakMode;
  editor->previousEditable = field.editable;
  editor->previousSelectable = field.selectable;
  editor->previousDrawsBackground = field.drawsBackground;
  NSCell* cell = field.cell;
  editor->previousWraps = cell.wraps;
  editor->previousScrollable = cell.scrollable;
  editor->previousSingleLine = cell.usesSingleLineMode;
  editor->finished = std::move(finished);
  // One line scrolling under the caret, on an opaque text background in
  // both appearances, as the default tree's editor, within its row.
  field.lineBreakMode = NSLineBreakByClipping;
  cell.wraps = NO;
  cell.scrollable = YES;
  cell.usesSingleLineMode = YES;
  field.drawsBackground = YES;
  field.backgroundColor = NSColor.textBackgroundColor;
  field.textColor = NSColor.textColor;
  field.editable = YES;
  field.selectable = YES;
  field.stringValue = text.toNSString();
  field.delegate = editor;
  g_activeEditor = editor;
  if (![outline.window makeFirstResponder:field] || field.currentEditor == nil) {
    qWarning().noquote() << QStringLiteral("Native Projects editor: row %1 did not take the keyboard").arg(identifier);
    [editor finishTaking:NO text:editor->original focus:YES notify:NO];
    return false;
  }
  [field.currentEditor selectAll:nil];
  qInfo().noquote() << QStringLiteral("Native Projects editor: %1 \"%2\"").arg(identifier, text);
  return true;
}

void closeProjectsNameEditor() {
  if (MITCADProjectsNameEditor* editor = g_activeEditor) {
    [editor finishTaking:NO text:editor->original focus:YES notify:NO];
  }
}

bool projectsNameEditorKey(NSEvent* event) {
  MITCADProjectsNameEditor* editor = g_activeEditor;
  NSTextField* field = editor != nil ? editor->field : nil;
  if (field == nil || field.currentEditor == nil || event.window != field.window ||
      field.window.firstResponder != field.currentEditor) {
    return false;
  }
  const QKeySequence pressed = keySequence(event);
  if (pressed.count() != 1 || pressed[0].keyboardModifiers() != Qt::NoModifier) return false;
  const Qt::Key key = pressed[0].key();
  if (key == Qt::Key_Return || key == Qt::Key_Enter) {
    [editor finishTaking:YES text:[field.currentEditor.string copy] focus:YES notify:YES];
    return true;
  }
  if (key == Qt::Key_Escape) {
    [editor finishTaking:NO text:editor->original focus:YES notify:YES];
    return true;
  }
  return false;
}

void checkProjectsNameEditor() {
  MITCADProjectsNameEditor* editor = g_activeEditor;
  if (editor == nil) return;
  NSTextField* field = editor->field;
  NSOutlineView* outline = editor->outline;
  if (field == nil || field.window == nil || outline == nil || outline.window == nil) {
    qInfo().noquote() << QStringLiteral("Native Projects editor: its row left the window");
    [editor finishTaking:NO text:editor->original focus:NO notify:YES];
  }
}

} // namespace mitcad::mac
