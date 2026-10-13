// SPDX-License-Identifier: MIT
#import <AppKit/AppKit.h>

#include "../NativeForms.hpp"
#include "../NativeDialogs.hpp"

#include <algorithm>
#include <iterator>
#include <vector>

#include <QHash>
#include <QTextDocument>
#include <QTimer>
#include <QtLogging>

#include "../../framework/CommandSession.hpp"
#include "../../framework/CutTargetsPanel.hpp"
#include "../../framework/TestSync.hpp"

namespace mitcad {
class AppKitCommandForm;
}

@interface MitcadCommandFormTarget : NSObject <NSTextFieldDelegate> {
@public
  mitcad::AppKitCommandForm* form;
}
- (void)performAction:(id)sender;
@end

namespace mitcad {
namespace {

NSString* nativeString(const QString& value) {
  const QByteArray utf8 = value.toUtf8();
  return [[NSString alloc] initWithBytes:utf8.constData()
                                length:static_cast<NSUInteger>(utf8.size())
                              encoding:NSUTF8StringEncoding];
}

QString qtString(NSString* value) {
  return value == nil ? QString() : QString::fromUtf8(value.UTF8String);
}

NSStackView* column() {
  NSStackView* stack = [[NSStackView alloc] initWithFrame:NSZeroRect];
  stack.orientation = NSUserInterfaceLayoutOrientationVertical;
  stack.alignment = NSLayoutAttributeLeading;
  stack.spacing = 6;
  stack.detachesHiddenViews = YES;
  stack.translatesAutoresizingMaskIntoConstraints = NO;
  return stack;
}

NSStackView* line() {
  NSStackView* stack = [[NSStackView alloc] initWithFrame:NSZeroRect];
  stack.orientation = NSUserInterfaceLayoutOrientationHorizontal;
  stack.alignment = NSLayoutAttributeCenterY;
  stack.spacing = 6;
  stack.translatesAutoresizingMaskIntoConstraints = NO;
  return stack;
}

void addFullWidth(NSStackView* parent, NSView* child) {
  child.translatesAutoresizingMaskIntoConstraints = NO;
  [parent addArrangedSubview:child];
  [child.leadingAnchor constraintEqualToAnchor:parent.leadingAnchor
                                     constant:parent.edgeInsets.left].active = YES;
  [child.trailingAnchor constraintEqualToAnchor:parent.trailingAnchor
                                      constant:-parent.edgeInsets.right].active = YES;
}

NSTextField* label(const QString& text, bool secondary = false) {
  NSTextField* field = [NSTextField wrappingLabelWithString:nativeString(text)];
  field.font = secondary ? [NSFont systemFontOfSize:[NSFont smallSystemFontSize]]
                         : [NSFont systemFontOfSize:[NSFont systemFontSize]];
  field.textColor = secondary ? NSColor.secondaryLabelColor : NSColor.labelColor;
  field.selectable = YES;
  [field setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
                                 forOrientation:NSLayoutConstraintOrientationHorizontal];
  return field;
}

QString displaySelection(const InputDef& input, int count) {
  if (count == 0) {
    return input.min > 0 ? QObject::tr("Select") : QObject::tr("None (all)");
  }
  return QObject::tr("%1 selected").arg(count);
}

const QColor kSelectionColors[] = {QColor(0x1f, 0x7a, 0xff), QColor(0xff, 0x8c, 0x00),
                                    QColor(0x2c, 0xa0, 0x2c), QColor(0x9b, 0x30, 0xff)};

QColor selectionColor(int index) {
  return kSelectionColors[static_cast<std::size_t>(index) % std::size(kSelectionColors)];
}

} // namespace

class AppKitCommandForm final : public NativeCommandForm {
public:
  explicit AppKitCommandForm(CommandSession& session)
      : m_def(session.def()), m_state(session.state()) {
    m_logTimer.setSingleShot(true);
    m_logTimer.setInterval(0);
    QObject::connect(&m_logTimer, &QTimer::timeout, &session, [this] { logLayout(); });
    TestSync::watch(&m_logTimer);
    m_target = [MitcadCommandFormTarget new];
    m_target->form = this;
    m_root = column();
    m_root.edgeInsets = NSEdgeInsetsMake(16, 16, 16, 16);
    m_root.spacing = 12;
    m_root.accessibilityIdentifier = @"commandPanel";
    NSTextField* title = label(m_def.name + (session.isEditing() ? QObject::tr(" (edit)") : QString()));
    title.font = [NSFont systemFontOfSize:[NSFont systemFontSize] weight:NSFontWeightSemibold];
    addFullWidth(m_root, title);
    m_fields = column();
    m_fields.spacing = 12;
    addFullWidth(m_root, m_fields);
    m_result = label(QString());
    m_result.accessibilityIdentifier = @"commandResult";
    m_result.hidden = YES;
    addFullWidth(m_root, m_result);
    m_message = label(QString(), true);
    m_message.accessibilityIdentifier = @"commandMessage";
    m_message.hidden = YES;
    addFullWidth(m_root, m_message);
    NSStackView* buttons = line();
    NSView* spring = [NSView new];
    [spring setContentHuggingPriority:NSLayoutPriorityDefaultLow
                      forOrientation:NSLayoutConstraintOrientationHorizontal];
    [buttons addArrangedSubview:spring];
    NSButton* cancel = button(QObject::tr("Cancel"), QStringLiteral("cancel"));
    cancel.accessibilityIdentifier = @"commandCancel";
    cancel.keyEquivalent = @"\033";
    [buttons addArrangedSubview:cancel];
    m_ok = button(m_def.inspect             ? QObject::tr("Close")
                  : m_def.okLabel.isEmpty() ? QObject::tr("OK")
                                            : m_def.okLabel,
                  QStringLiteral("accept"));
    m_ok.accessibilityIdentifier = @"commandOk";
    m_ok.keyEquivalent = @"\r";
    [buttons addArrangedSubview:m_ok];
    addFullWidth(m_root, buttons);
    int colorIndex = 0;
    for (const InputDef& input : m_def.inputs) {
      if (input.type == InputDef::Type::Selection) {
        m_colors.insert(input.id, input.color.isValid() ? input.color : selectionColor(colorIndex));
        ++colorIndex;
      } else if (input.type == InputDef::Type::List) {
        m_listColors.insert(input.id, colorIndex++);
      }
    }
    rebuildFields();
    session.setPresenter(*this);
  }

  ~AppKitCommandForm() override {
    // AppKit's target and delegates are non-owning. Clear the controller's
    // back pointer before any retained view can send another event.
    m_target->form = nullptr;
    for (const Row& row : m_rows) {
      row.edit.delegate = nil;
    }
  }

  void* nativeView() const override { return (__bridge void*)m_root; }

  void refresh(const CommandState& state) override {
    m_state = state;
    bool structureChanged = false;
    for (const InputDef& input : m_def.inputs) {
      if (input.type == InputDef::Type::List && m_counts.value(input.id, -1) != state.rows(input.id)) {
        structureChanged = true;
      }
    }
    if (structureChanged) {
      rebuildFields();
    }
    for (Row& row : m_rows) {
      const CommandState scope = row.list.isEmpty() ? state : state.row(row.list, row.index);
      const InputDef& input = *row.input;
      bool visible = !input.visible || input.visible(scope);
      if (!row.list.isEmpty()) {
        const InputDef& owner = m_def.inputs[m_def.inputIndex(row.list)];
        visible = visible && (!owner.visible || owner.visible(state));
      }
      row.box.hidden = !visible;
      switch (input.type) {
      case InputDef::Type::Value:
      case InputDef::Type::Text:
        if (qtString(row.edit.stringValue) != scope.text(input.id)) {
          row.edit.stringValue = nativeString(scope.text(input.id));
        }
        break;
      case InputDef::Type::Choice:
        refreshChoices(row, scope);
        break;
      case InputDef::Type::Check:
      case InputDef::Type::Flip:
        row.check.state = scope.checked(input.id) ? NSControlStateValueOn : NSControlStateValueOff;
        break;
      case InputDef::Type::Selection: {
        const Selection& items = scope.items(input.id);
        row.select.title = nativeString(displaySelection(input, static_cast<int>(items.size())));
        row.select.state = row.key == m_active ? NSControlStateValueOn : NSControlStateValueOff;
        row.select.accessibilityValue = nativeString(row.key == m_active ? QObject::tr("Active selection input")
                                                                        : QObject::tr("Inactive selection input"));
        row.clear.enabled = !items.isEmpty();
        if (row.order != nil) {
          refreshOrder(row, items);
        }
        break;
      }
      case InputDef::Type::CutTargets:
        row.targets->setScope(scope.object(input.id));
        break;
      case InputDef::Type::List:
        break;
      }
      updateNote(row);
    }
    refreshControlAvailability();
    [m_root layoutSubtreeIfNeeded];
    m_logTimer.start();
  }

  void setPreviewReport(const QJsonObject& report) override {
    for (Row& row : m_rows) {
      if (row.targets) {
        row.targets->setPreview(report.value(QStringLiteral("cut_targets")).toObject());
        row.targetsView->refresh();
      }
    }
  }

  void inputActionApplied(const QString& key) override {
    m_pendingStructure.remove(key);
    refreshControlAvailability();
  }

  bool inputEnabled(const QString& key) const override {
    for (auto pending = m_pendingStructure.cbegin(); pending != m_pendingStructure.cend(); ++pending) {
      if (key == pending.value() || key.startsWith(pending.value() + QLatin1Char('.'))) {
        return false;
      }
    }
    return true;
  }

  void setActiveSelection(const QString& key) override {
    m_active = key;
    for (Row& row : m_rows) {
      if (row.select != nil) {
        row.select.state = row.key == key ? NSControlStateValueOn : NSControlStateValueOff;
      }
    }
  }

  void setInputError(const QString& key, const QString& text) override {
    m_notes[key].first = text;
    if (Row* row = findRow(key)) {
      updateNote(*row);
    }
  }

  void setValueHint(const QString& key, const QString& text) override {
    m_notes[key].second = text;
    if (Row* row = findRow(key)) {
      updateNote(*row);
    }
  }

  void setMessage(const QString& text, bool error) override {
    m_message.stringValue = nativeString(text);
    m_message.textColor = error ? NSColor.systemRedColor : NSColor.secondaryLabelColor;
    m_message.hidden = text.isEmpty();
  }

  void setResult(const QString& text) override {
    // The command definitions return simple HTML. AppKit labels preserve
    // readable paragraphs without routing the result through a Qt widget.
    QTextDocument document;
    document.setHtml(text);
    m_result.stringValue = nativeString(document.toPlainText());
    m_result.hidden = text.isEmpty();
  }

  void setOkEnabled(bool enabled) override { m_ok.enabled = enabled; }

  QColor colorOf(const QString& key) const override {
    if (m_colors.contains(key)) {
      return m_colors.value(key);
    }
    const QStringList parts = key.split(QLatin1Char('.'));
    if (parts.size() >= 3 && m_listColors.contains(parts[0])) {
      if (const Row* row = findRow(key); row != nullptr && row->input->color.isValid()) {
        return row->input->color;
      }
      return selectionColor(m_listColors.value(parts[0]) + parts[1].toInt());
    }
    return selectionColor(0);
  }

  bool focusFirstValue() override {
    for (const Row& row : m_rows) {
      if (row.input->type == InputDef::Type::Value && shown(row)) {
        return focusValue(row.key);
      }
    }
    return false;
  }

  bool focusValue(const QString& key) override {
    const Row* row = findRow(key);
    if (row == nullptr || row->edit == nil || !shown(*row) || m_root.window == nil) {
      return false;
    }
    [m_root.window makeFirstResponder:row->edit];
    [row->edit selectText:nil];
    return true;
  }

  void logLayout() const override {
    [m_root layoutSubtreeIfNeeded];
    for (const Row& row : m_rows) {
      NSView* control = row.edit != nil ? static_cast<NSView*>(row.edit)
                          : row.select != nil ? static_cast<NSView*>(row.select)
                          : row.popup != nil ? static_cast<NSView*>(row.popup)
                          : row.check != nil ? static_cast<NSView*>(row.check)
                                             : static_cast<NSView*>(row.box);
      const NSPoint at = [control convertPoint:NSMakePoint(NSMidX(control.bounds), NSMidY(control.bounds))
                                        toView:nil];
      NSView* content = m_root.window.contentView;
      const int x = static_cast<int>(at.x);
      const int y = static_cast<int>(content.bounds.size.height - at.y);
      qDebug().noquote() << QStringLiteral("Panel %1 input %2 at %3,%4%5")
                                .arg(m_def.name, row.key).arg(x).arg(y)
                                .arg(shown(row) ? QString() : QStringLiteral(" (hidden)"));
    }
  }

  void action(NSControl* sender) {
    // Copy the callback before invoking it: accepting a command may cause
    // the controller to remove the form and destroy its session.
    const QStringList parts = qtString(sender.identifier).split(QLatin1Char('|'));
    if (parts.isEmpty()) {
      return;
    }
    const QString key = parts.value(1);
    const QString& kind = parts[0];
    if (!key.isEmpty() && !inputEnabled(key)) { return; }
    const bool structural = kind == QStringLiteral("add") || kind == QStringLiteral("removeRow") ||
        kind == QStringLiteral("move") || kind == QStringLiteral("removeItem") ||
        kind == QStringLiteral("clear");
    if (structural) {
      if (![sender.window makeFirstResponder:nil]) { return; }
      const Row* row = findRow(key);
      const QString group = row && !row->list.isEmpty() ? row->list : key;
      m_pendingStructure.insert(key, group);
      refreshControlAvailability();
    }
    if (kind == QStringLiteral("accept")) {
      auto call = actions().accepted;
      if (call) { call(); }
    } else if (kind == QStringLiteral("cancel")) {
      auto call = actions().rejected;
      if (call) { call(); }
    } else if (kind == QStringLiteral("select")) {
      auto call = actions().selectionActivated;
      if (call) { call(key); }
    } else if (kind == QStringLiteral("clear")) {
      auto call = actions().selectionCleared;
      if (call) { call(key); }
    } else if (kind == QStringLiteral("choice")) {
      NSPopUpButton* popup = static_cast<NSPopUpButton*>(sender);
      const QString value = qtString(static_cast<NSString*>(popup.selectedItem.representedObject));
      auto call = actions().choiceChanged;
      if (call) { call(key, value); }
    } else if (kind == QStringLiteral("check")) {
      const bool checked = static_cast<NSButton*>(sender).state == NSControlStateValueOn;
      auto call = actions().checkChanged;
      if (call) { call(key, checked); }
    } else if (kind == QStringLiteral("add")) {
      auto call = actions().rowAdded;
      if (call) { call(key); }
    } else if (kind == QStringLiteral("removeRow")) {
      auto call = actions().rowRemoved;
      if (call) { call(key, parts.value(2).toInt()); }
    } else if (kind == QStringLiteral("removeItem")) {
      auto call = actions().itemRemoved;
      if (call) { call(key, parts.value(2).toInt()); }
    } else if (kind == QStringLiteral("move")) {
      auto call = actions().itemMoved;
      if (call) { call(key, parts.value(2).toInt(), parts.value(3).toInt()); }
    }
  }

  void textChanged(NSTextField* field) {
    if (!inputEnabled(qtString(field.identifier))) { return; }
    auto call = actions().textEdited;
    if (call) { call(qtString(field.identifier), qtString(field.stringValue)); }
  }

  void valueCommitted(NSTextField* field) {
    if (!inputEnabled(qtString(field.identifier))) { return; }
    auto call = actions().valueCommitted;
    if (call) { call(qtString(field.identifier)); }
  }

  bool textCommand(NSTextField* field, SEL command) {
    if (command == @selector(insertNewline:)) {
      valueCommitted(field);
      auto call = actions().accepted;
      if (call) { call(); }
      return true;
    }
    if (command == @selector(cancelOperation:)) {
      auto call = actions().rejected;
      if (call) { call(); }
      return true;
    }
    return false;
  }

private:
  struct Row {
    const InputDef* input = nullptr;
    QString key;
    QString list;
    int index = -1;
    NSStackView* __strong box = nil;
    NSTextField* __strong note = nil;
    NSTextField* __strong edit = nil;
    NSPopUpButton* __strong popup = nil;
    NSButton* __strong check = nil;
    NSButton* __strong select = nil;
    NSButton* __strong clear = nil;
    NSStackView* __strong order = nil;
    QStringList orderDescriptions;
    std::shared_ptr<CutTargetsPanel> targets;
    std::shared_ptr<NativeWidgetView> targetsView;
  };

  NSButton* button(const QString& text, const QString& action) {
    NSButton* control = [NSButton buttonWithTitle:nativeString(text)
                                           target:m_target
                                           action:@selector(performAction:)];
    control.identifier = nativeString(action);
    control.accessibilityIdentifier = nativeString(QStringLiteral("native.action.") + action);
    control.bezelStyle = NSBezelStyleRounded;
    return control;
  }

  void refreshControlAvailability() {
    for (const Row& row : m_rows) {
      // A list owner's view contains all its row fields and actions.
      // Visiting children separately could undo their owner's lock.
      if (row.list.isEmpty() && !row.targets) {
        setControlAvailability(row.box, inputEnabled(row.key));
      }
    }
  }

  void setControlAvailability(NSView* view, bool groupEnabled) {
    if ([view isKindOfClass:[NSControl class]]) {
      NSControl* control = static_cast<NSControl*>(view);
      const QStringList parts = qtString(control.identifier).split(QLatin1Char('|'));
      const QString kind = parts.value(0);
      const QString key = parts.value(1);
      bool enabled = groupEnabled;
      if (kind == QStringLiteral("clear")) {
        enabled = enabled && !m_state.items(key).isEmpty();
      } else if (kind == QStringLiteral("removeRow")) {
        const int index = m_def.inputIndex(key);
        enabled = enabled && index >= 0 && m_state.rows(key) > m_def.inputs[index].minRows;
      } else if (kind == QStringLiteral("move") || kind == QStringLiteral("removeItem")) {
        const int count = m_state.items(key).size();
        const int from = parts.value(2).toInt();
        enabled = enabled && from >= 0 && from < count;
        if (kind == QStringLiteral("move")) {
          const int to = parts.value(3).toInt();
          enabled = enabled && to >= 0 && to < count;
        }
      }
      control.enabled = enabled;
    }
    for (NSView* child in view.subviews) {
      setControlAvailability(child, groupEnabled);
    }
  }

  void rebuildFields() {
    // A row-count change may remove an active field editor. End editing
    // before removing its view, then restore the same key when it survives.
    QString focused;
    for (const Row& row : m_rows) {
      if (row.edit != nil && row.edit.currentEditor != nil) {
        focused = row.key;
        row.edit.delegate = nil;
      }
    }
    if (!focused.isEmpty()) {
      [m_root.window makeFirstResponder:nil];
    }
    for (NSView* child in [m_fields.arrangedSubviews copy]) {
      [m_fields removeArrangedSubview:child];
      [child removeFromSuperview];
    }
    m_rows.clear();
    m_counts.clear();
    for (const InputDef& input : m_def.inputs) {
      if (input.type != InputDef::Type::List) {
        createField(input, input.id, QString(), -1, m_fields);
        continue;
      }
      Row owner;
      owner.input = &input;
      owner.key = input.id;
      owner.box = column();
      owner.box.spacing = 10;
      NSTextField* heading = label(input.label);
      heading.font = [NSFont systemFontOfSize:[NSFont systemFontSize] weight:NSFontWeightMedium];
      addFullWidth(owner.box, heading);
      addFullWidth(m_fields, owner.box);
      m_rows.push_back(owner);
      const int count = m_state.rows(input.id);
      m_counts.insert(input.id, count);
      for (int index = 0; index < count; ++index) {
        NSStackView* group = column();
        group.edgeInsets = NSEdgeInsetsMake(0, 8, 4, 0);
        NSStackView* header = line();
        NSTextField* name = label(input.rowLabel.arg(index + 1), true);
        [header addArrangedSubview:name];
        NSButton* remove = button(QObject::tr("Remove"), QStringLiteral("removeRow|%1|%2").arg(input.id).arg(index));
        remove.enabled = count > input.minRows;
        remove.toolTip = nativeString(QObject::tr("Remove %1").arg(input.rowLabel.arg(index + 1)));
        [header addArrangedSubview:remove];
        addFullWidth(group, header);
        for (const InputDef& child : input.children) {
          createField(child, CommandState::rowKey(input.id, index, child.id), input.id, index, group);
        }
        addFullWidth(owner.box, group);
      }
      if (!input.addLabel.isEmpty()) {
        NSButton* add = button(input.addLabel, QStringLiteral("add|") + input.id);
        add.accessibilityIdentifier = nativeString(QStringLiteral("add_") + input.id);
        [owner.box addArrangedSubview:add];
      }
    }
    if (!focused.isEmpty()) {
      focusValue(focused);
    }
  }

  void createField(const InputDef& input, const QString& key, const QString& list, int index,
                   NSStackView* parent) {
    Row row;
    row.input = &input;
    row.key = key;
    row.list = list;
    row.index = index;
    row.box = column();
    row.box.spacing = 4;
    row.box.accessibilityIdentifier = nativeString(QStringLiteral("row_") + key);
    if (!input.heading.isEmpty()) {
      // The heading of a group of inputs, hidden with the row.
      NSTextField* heading = label(input.heading);
      heading.font = [NSFont boldSystemFontOfSize:[NSFont systemFontSize]];
      addFullWidth(row.box, heading);
    }
    addFullWidth(row.box, label(input.label));
    NSView* control = nil;
    switch (input.type) {
    case InputDef::Type::Value:
    case InputDef::Type::Text:
      row.edit = [[NSTextField alloc] initWithFrame:NSZeroRect];
      row.edit.editable = YES;
      row.edit.selectable = YES;
      row.edit.bezeled = YES;
      row.edit.bezelStyle = NSTextFieldRoundedBezel;
      row.edit.font = [NSFont systemFontOfSize:[NSFont systemFontSize]];
      row.edit.identifier = nativeString(key);
      row.edit.accessibilityIdentifier = nativeString(QStringLiteral("input_") + key);
      row.edit.delegate = m_target;
      row.edit.toolTip = nativeString(input.tooltip.isEmpty() && input.type == InputDef::Type::Value
                                         ? QObject::tr("A value or an expression, such as 20, 2 in or d1 * 2")
                                         : input.tooltip);
      control = row.edit;
      break;
    case InputDef::Type::Choice:
      row.popup = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
      row.popup.identifier = nativeString(QStringLiteral("choice|") + key);
      row.popup.accessibilityIdentifier = nativeString(QStringLiteral("input_") + key);
      row.popup.target = m_target;
      row.popup.action = @selector(performAction:);
      control = row.popup;
      break;
    case InputDef::Type::Check:
    case InputDef::Type::Flip:
      row.check = button(input.type == InputDef::Type::Flip ? QObject::tr("Reverse direction") : input.label,
                         QStringLiteral("check|") + key);
      row.check.accessibilityIdentifier = nativeString(QStringLiteral("input_") + key);
      [row.check setButtonType:input.type == InputDef::Type::Flip ? NSButtonTypeToggle : NSButtonTypeSwitch];
      if (input.type == InputDef::Type::Flip) {
        row.check.image = [NSImage imageWithSystemSymbolName:@"arrow.up.arrow.down" accessibilityDescription:nil];
        row.check.imagePosition = NSImageLeft;
      }
      control = row.check;
      break;
    case InputDef::Type::Selection: {
      NSStackView* selection = line();
      row.select = button(QString(), QStringLiteral("select|") + key);
      [row.select setButtonType:NSButtonTypePushOnPushOff];
      row.select.toolTip = nativeString(input.tooltip.isEmpty() ? QObject::tr("Click to pick %1").arg(input.label)
                                                               : input.tooltip);
      row.select.accessibilityIdentifier = nativeString(QStringLiteral("input_") + key);
      row.select.accessibilityLabel = nativeString(input.label);
      [selection addArrangedSubview:row.select];
      row.clear = button(QObject::tr("Clear"), QStringLiteral("clear|") + key);
      row.clear.accessibilityLabel = nativeString(QObject::tr("Clear %1").arg(input.label));
      [selection addArrangedSubview:row.clear];
      control = selection;
      if (input.ordered) {
        addFullWidth(row.box, selection);
        row.order = column();
        row.order.spacing = 4;
        control = row.order;
      }
      break;
    }
    case InputDef::Type::CutTargets:
      row.targets = std::make_shared<CutTargetsPanel>();
      row.targets->setObjectName(QStringLiteral("input_") + key);
      row.targets->setScope(m_state.object(key));
      QObject::connect(row.targets.get(), &CutTargetsPanel::scopeChanged, row.targets.get(),
                       [this, key](const QJsonObject& value) {
        auto call = actions().objectChanged;
        if (call) { call(key, value); }
      });
      QObject::connect(row.targets.get(), &CutTargetsPanel::highlightChanged, row.targets.get(),
                       [this](const Selection& items) {
        auto call = actions().highlightRequested;
        if (call) { call(items); }
      });
      row.targetsView = createNativeWidgetView(row.targets.get());
      control = (__bridge NSView*)row.targetsView->nativeView();
      break;
    case InputDef::Type::List:
      // Declarative lists contain fields; nesting is not supported by the
      // shared CommandState row representation either.
      return;
    }
    if (control != nil) {
      if ([control isKindOfClass:[NSControl class]]) {
        control.accessibilityIdentifier = nativeString(QStringLiteral("input_") + key);
      }
      control.accessibilityLabel = nativeString(input.label);
      if (input.type != InputDef::Type::Value && input.type != InputDef::Type::Text) {
        control.toolTip = nativeString(input.tooltip);
      }
      addFullWidth(row.box, control);
    }
    row.note = label(QString(), true);
    row.note.hidden = YES;
    addFullWidth(row.box, row.note);
    addFullWidth(parent, row.box);
    m_rows.push_back(std::move(row));
  }

  void refreshChoices(Row& row, const CommandState& scope) {
    QVector<QPair<QString, QString>> options = row.input->choicesIn(scope);
    const QString value = scope.choice(row.input->id);
    if (!value.isEmpty() && std::none_of(options.cbegin(), options.cend(), [&value](const auto& option) {
          return option.first == value;
        })) {
      options.append({value, value});
    }
    bool same = row.popup.numberOfItems == options.size();
    for (int index = 0; same && index < options.size(); ++index) {
      NSMenuItem* item = [row.popup itemAtIndex:static_cast<NSInteger>(index)];
      same = qtString(item.title) == options[index].second &&
             qtString(static_cast<NSString*>(item.representedObject)) == options[index].first;
    }
    if (!same) {
      [row.popup removeAllItems];
      QStringList labels;
      for (const auto& option : options) {
        [row.popup addItemWithTitle:nativeString(option.second)];
        row.popup.lastItem.representedObject = nativeString(option.first);
        labels.append(option.second);
      }
      qDebug().noquote() << QStringLiteral("Panel %1 options %2: %3")
                                .arg(m_def.name, row.key, labels.join(QStringLiteral(" | ")));
    }
    NSInteger selected = 0;
    for (int index = 0; index < options.size(); ++index) {
      if (options[index].first == value) {
        selected = static_cast<NSInteger>(index);
        break;
      }
    }
    if (!options.isEmpty()) {
      [row.popup selectItemAtIndex:selected];
    }
  }

  void refreshOrder(Row& row, const Selection& items) {
    QStringList descriptions;
    for (const SelectionItem& item : items) {
      descriptions.append(item.describe());
    }
    if (descriptions == row.orderDescriptions) {
      return;
    }
    row.orderDescriptions = descriptions;
    for (NSView* child in [row.order.arrangedSubviews copy]) {
      [row.order removeArrangedSubview:child];
      [child removeFromSuperview];
    }
    for (int index = 0; index < descriptions.size(); ++index) {
      NSStackView* item = line();
      NSTextField* text = label(QStringLiteral("%1. %2").arg(index + 1).arg(descriptions[index]), true);
      [item addArrangedSubview:text];
      for (int direction : {-1, 1}) {
        NSButton* move = button(direction < 0 ? QStringLiteral("↑") : QStringLiteral("↓"),
                                QStringLiteral("move|%1|%2|%3").arg(row.key).arg(index).arg(index + direction));
        move.enabled = index + direction >= 0 && index + direction < descriptions.size();
        move.accessibilityLabel = nativeString(direction < 0 ? QObject::tr("Move %1 up").arg(descriptions[index])
                                                           : QObject::tr("Move %1 down").arg(descriptions[index]));
        [item addArrangedSubview:move];
      }
      NSButton* remove = button(QStringLiteral("−"), QStringLiteral("removeItem|%1|%2").arg(row.key).arg(index));
      remove.accessibilityLabel = nativeString(QObject::tr("Remove %1").arg(descriptions[index]));
      [item addArrangedSubview:remove];
      addFullWidth(row.order, item);
    }
  }

  void updateNote(Row& row) {
    if (row.note == nil) {
      return;
    }
    const auto texts = m_notes.value(row.key);
    const bool error = !texts.first.isEmpty();
    const QString text = error ? texts.first : texts.second;
    row.note.stringValue = nativeString(text);
    row.note.textColor = error ? NSColor.systemRedColor : NSColor.secondaryLabelColor;
    row.note.hidden = text.isEmpty();
    row.box.accessibilityHelp = nativeString(text);
    if (row.edit != nil) {
      row.edit.accessibilityHelp = nativeString(text);
    }
  }

  bool shown(const Row& row) const {
    return !row.box.hidden && !row.box.hiddenOrHasHiddenAncestor;
  }

  Row* findRow(const QString& key) {
    const auto row = std::find_if(m_rows.begin(), m_rows.end(), [&key](const Row& candidate) {
      return candidate.key == key;
    });
    return row == m_rows.end() ? nullptr : &*row;
  }

  const Row* findRow(const QString& key) const {
    const auto row = std::find_if(m_rows.cbegin(), m_rows.cend(), [&key](const Row& candidate) {
      return candidate.key == key;
    });
    return row == m_rows.cend() ? nullptr : &*row;
  }

  const CommandDef m_def;
  CommandState m_state;
  MitcadCommandFormTarget* __strong m_target;
  NSStackView* __strong m_root;
  NSStackView* __strong m_fields;
  NSTextField* __strong m_result;
  NSTextField* __strong m_message;
  NSButton* __strong m_ok;
  std::vector<Row> m_rows;
  QHash<QString, int> m_counts;
  QHash<QString, QString> m_pendingStructure;
  QHash<QString, QColor> m_colors;
  QHash<QString, int> m_listColors;
  QHash<QString, QPair<QString, QString>> m_notes;
  QString m_active;
  QTimer m_logTimer;
};

std::unique_ptr<NativeCommandForm> createNativeCommandForm(CommandSession& session) {
  return std::make_unique<AppKitCommandForm>(session);
}

} // namespace mitcad

@implementation MitcadCommandFormTarget
- (void)performAction:(id)sender {
  if (form != nullptr && [sender isKindOfClass:[NSControl class]]) {
    form->action(static_cast<NSControl*>(sender));
  }
}
- (void)controlTextDidChange:(NSNotification*)notification {
  if (form != nullptr) {
    form->textChanged(static_cast<NSTextField*>(notification.object));
  }
}
- (void)controlTextDidEndEditing:(NSNotification*)notification {
  if (form != nullptr) {
    form->valueCommitted(static_cast<NSTextField*>(notification.object));
  }
}
- (BOOL)control:(NSControl*)control textView:(NSTextView*)textView doCommandBySelector:(SEL)command {
  (void)textView;
  return form != nullptr && form->textCommand(static_cast<NSTextField*>(control), command);
}
@end
