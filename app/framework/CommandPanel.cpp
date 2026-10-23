// SPDX-License-Identifier: MIT
#include "CommandPanel.hpp"

#include <algorithm>
#include <functional>
#include <iterator>
#include <utility>

#include <QAbstractScrollArea>
#include <QCheckBox>
#include <QComboBox>
#include <QCoreApplication>
#include <QEvent>
#include <QFont>
#include <QFormLayout>
#include <QFrame>
#include <QHBoxLayout>
#include <QIcon>
#include <QImage>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QStyle>
#include <QListWidget>
#include <QPixmap>
#include <QPushButton>
#include <QScrollBar>
#include <QSignalBlocker>
#include <QTimer>
#include <QToolButton>
#include <QVBoxLayout>
#include <QtLogging>

#include "ChromeStyle.hpp"
#include "CommandMessage.hpp"
#include "CutTargetsPanel.hpp"
#include "GlassCard.hpp"
#include "Icons.hpp"
#include "ParameterField.hpp"
#include "TestSync.hpp"
#include "Theme.hpp"

namespace mitcad {
namespace {

// Colours of the selection inputs in order, unless an input sets its own.
const QColor kInputColors[] = {QColor(0x1f, 0x7a, 0xff), QColor(0xff, 0x8c, 0x00),
                               QColor(0x2c, 0xa0, 0x2c), QColor(0x9b, 0x30, 0xff)};

QColor paletteColor(int index) {
  return kInputColors[static_cast<std::size_t>(index) % std::size(kInputColors)];
}

QString countText(const InputDef& input, int count) {
  if (count == 0) {
    return input.min > 0 ? QObject::tr("Select") : QObject::tr("None (all)");
  }
  return QObject::tr("%1 selected").arg(count);
}

bool isShown(const InputDef& input, const CommandState& state) {
  return !input.visible || input.visible(state);
}

// "sets.1.edges": the list "sets", row 1.
bool splitKey(const QString& key, QString& list, int& index) {
  const QStringList parts = key.split(QLatin1Char('.'));
  if (parts.size() < 3) {
    return false;
  }
  bool number = false;
  index = parts[1].toInt(&number);
  list = parts[0];
  return number;
}

QToolButton* smallButton(const QString& text, const QString& tip) {
  auto* button = new QToolButton;
  button->setText(text);
  button->setToolTip(tip);
  button->setFocusPolicy(Qt::NoFocus);
  button->setAutoRaise(true);
  if (chromeStyle() == ChromeStyle::Floating) {
    setFlatCardButton(button);
    button->setFixedSize(24, 24);
  }
  return button;
}

// The look of the controls in a floating card: flat and rounded like the
// ribbon's buttons. A checked toggle is outlined in the accent colour, which
// is the system's on macOS and so is not palette(highlight); the styles are
// set again when the palette changes (CommandPanel::applyControlStyles).
void styleToggle(QToolButton* button, const QColor& accent) {
  button->setAutoRaise(true);
  button->setFixedSize(30, 26);
  button->setStyleSheet(QStringLiteral(
      "QToolButton { border: 1px solid rgba(128, 128, 128, 70); background: rgba(128, 128, 128, 25);"
      " border-radius: 7px; padding: 1px; }"
      "QToolButton:hover { background: rgba(128, 128, 128, 55); }"
      "QToolButton:pressed { background: rgba(128, 128, 128, 90); }"
      "QToolButton:checked { border: 1px solid %1; background: rgba(128, 128, 128, 55); }")
                            .arg(accent.name()));
}

// The selection button: a capsule that is filled with the accent colour
// while its input is the one that takes the picks.
void styleSelect(QPushButton* button, const QColor& accent, const QColor& accentText) {
  button->setFixedHeight(26);
  button->setStyleSheet(QStringLiteral(
      "QPushButton { border: 1px solid rgba(128, 128, 128, 70); background: rgba(128, 128, 128, 25);"
      " border-radius: 13px; padding: 0 12px; }"
      "QPushButton:hover { background: rgba(128, 128, 128, 55); }"
      "QPushButton:checked { border: 1px solid %1; background: %1; color: %2; }")
                            .arg(accent.name(), accentText.name()));
}

// The text fields: rounded with a subtle fill like the combo boxes next to
// them, and the accent colour as the focus ring (QMacStyle's own ring does not
// follow a style sheet). The selection colours stay the palette's.
void styleLineEdit(QLineEdit* edit, const QColor& accent, const QColor& error) {
  edit->setStyleSheet(QStringLiteral(
      "QLineEdit { border: 1px solid rgba(128, 128, 128, 70); background: rgba(128, 128, 128, 25);"
      " border-radius: 7px; padding: 3px 6px; min-height: 18px; }"
      "QLineEdit:focus { border: 1.5px solid %1; }"
      "QLineEdit[invalid=\"true\"] { border: 1px solid %2; }"
      "QLineEdit:disabled { background: rgba(128, 128, 128, 12); }")
                          .arg(accent.name(), error.name()));
}

// A choice shown as a tile (InputDef::choiceTile, mitcad#427): the chosen
// option's picture, its name and a smaller line under it. A click, or Enter
// or Space while it has the keyboard, calls `activated`. It takes the
// keyboard by Tab only: after a click Enter still confirms the command.
class TileButton : public QToolButton {
public:
  explicit TileButton(std::function<void()> activated) : m_activated(std::move(activated)) {
    setFocusPolicy(Qt::TabFocus);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Minimum);
    auto* box = new QHBoxLayout(this);
    box->setContentsMargins(6, 4, 6, 4);
    box->setSpacing(8);
    m_image = new QLabel;
    m_image->setFixedSize(kImageSize, kImageSize);
    m_image->setAlignment(Qt::AlignCenter);
    m_title = new QLabel;
    m_title->setWordWrap(true);
    m_detail = new QLabel;
    m_detail->setWordWrap(true);
    m_detail->setForegroundRole(QPalette::PlaceholderText);
    QFont small = m_detail->font();
    if (small.pointSizeF() > 0.0) {
      small.setPointSizeF(small.pointSizeF() * 0.85);
    } else if (small.pixelSize() > 0) {
      small.setPixelSize(std::max(1, small.pixelSize() * 85 / 100));
    }
    m_detail->setFont(small);
    for (QLabel* label : {m_image, m_title, m_detail}) {
      label->setAttribute(Qt::WA_TransparentForMouseEvents);
    }
    auto* texts = new QVBoxLayout;
    texts->setSpacing(0);
    texts->addStretch(1);
    texts->addWidget(m_title);
    texts->addWidget(m_detail);
    texts->addStretch(1);
    box->addWidget(m_image);
    box->addLayout(texts, 1);
    setMinimumHeight(kImageSize + 8);
    connect(this, &QToolButton::clicked, this, [this] {
      if (m_activated) {
        m_activated();
      }
    });
  }

  // Shows a tile; false when it shows that already.
  bool setTile(const ChoiceTile& tile) {
    const qint64 key = tile.image.isNull() ? 0 : tile.image.cacheKey();
    if (m_shown && tile.title == m_title->text() && tile.detail == m_detail->text() && key == m_imageKey) {
      return false;
    }
    m_shown = true;
    m_imageKey = key;
    if (tile.image.isNull()) {
      m_image->clear();
    } else {
      const qreal ratio = devicePixelRatioF();
      QPixmap pixmap = QPixmap::fromImage(tile.image.scaled(QSize(kImageSize, kImageSize) * ratio,
                                                            Qt::KeepAspectRatio, Qt::SmoothTransformation));
      pixmap.setDevicePixelRatio(ratio);
      m_image->setPixmap(pixmap);
    }
    m_image->setVisible(!tile.image.isNull());
    m_title->setText(tile.title);
    m_detail->setText(tile.detail);
    m_detail->setVisible(!tile.detail.isEmpty());
    return true;
  }

protected:
  bool event(QEvent* event) override {
    // Enter and Space open it rather than confirming the command (the main
    // window's shortcut) while it has the keyboard.
    if (event->type() == QEvent::ShortcutOverride) {
      const auto* key = static_cast<QKeyEvent*>(event);
      if (key->modifiers() == Qt::NoModifier &&
          (key->key() == Qt::Key_Return || key->key() == Qt::Key_Enter || key->key() == Qt::Key_Space)) {
        event->accept();
        return true;
      }
    }
    return QToolButton::event(event);
  }
  void keyPressEvent(QKeyEvent* event) override {
    if (event->key() == Qt::Key_Return || event->key() == Qt::Key_Enter) {
      event->accept();
      click();
      return;
    }
    QToolButton::keyPressEvent(event); // Space: pressed and released, a click
  }

private:
  static constexpr int kImageSize = 40;
  std::function<void()> m_activated;
  QLabel* m_image = nullptr;
  QLabel* m_title = nullptr;
  QLabel* m_detail = nullptr;
  qint64 m_imageKey = 0;
  bool m_shown = false;
};

} // namespace

CommandPanel::CommandPanel(const CommandDef& def, const CommandState& state, bool editing,
                           QWidget* parent)
    : QWidget(parent), m_def(def) {
  setObjectName(QStringLiteral("commandPanel"));
  // Keep the widget signals for existing test and UI callers, and forward
  // them through the same presenter actions the native inspector uses.
  connect(this, &CommandPanel::textEdited, this, [this](const QString& key, const QString& value) {
    auto call = CommandPresenter::actions().textEdited;
    if (call) {
      call(key, value);
    }
  });
  connect(this, &CommandPanel::valueCommitted, this, [this](const QString& key) {
    auto call = CommandPresenter::actions().valueCommitted;
    if (call) {
      call(key);
    }
  });
  connect(this, &CommandPanel::choiceChanged, this, [this](const QString& key, const QString& value) {
    auto call = CommandPresenter::actions().choiceChanged;
    if (call) {
      call(key, value);
    }
  });
  connect(this, &CommandPanel::checkChanged, this, [this](const QString& key, bool value) {
    auto call = CommandPresenter::actions().checkChanged;
    if (call) {
      call(key, value);
    }
  });
  connect(this, &CommandPanel::selectionActivated, this, [this](const QString& key) {
    auto call = CommandPresenter::actions().selectionActivated;
    if (call) {
      call(key);
    }
  });
  connect(this, &CommandPanel::selectionCleared, this, [this](const QString& key) {
    auto call = CommandPresenter::actions().selectionCleared;
    if (call) {
      call(key);
    }
  });
  connect(this, &CommandPanel::itemMoved, this, [this](const QString& key, int from, int to) {
    auto call = CommandPresenter::actions().itemMoved;
    if (call) {
      call(key, from, to);
    }
  });
  connect(this, &CommandPanel::itemRemoved, this, [this](const QString& key, int index) {
    auto call = CommandPresenter::actions().itemRemoved;
    if (call) {
      call(key, index);
    }
  });
  connect(this, &CommandPanel::rowAdded, this, [this](const QString& key) {
    auto call = CommandPresenter::actions().rowAdded;
    if (call) {
      call(key);
    }
  });
  connect(this, &CommandPanel::rowRemoved, this, [this](const QString& key, int index) {
    auto call = CommandPresenter::actions().rowRemoved;
    if (call) {
      call(key, index);
    }
  });
  connect(this, &CommandPanel::buttonClicked, this, [this](const QString& key) {
    auto call = CommandPresenter::actions().buttonClicked;
    if (call) {
      call(key);
    }
  });
  connect(this, &CommandPanel::accepted, this, [this] {
    auto call = CommandPresenter::actions().accepted;
    if (call) {
      call();
    }
  });
  connect(this, &CommandPanel::rejected, this, [this] {
    auto call = CommandPresenter::actions().rejected;
    if (call) {
      call();
    }
  });

  // The panel is as wide as the page shows it (the scroll area's viewport,
  // which never scrolls sideways), whatever its rows ask for: a row that
  // would want more is squeezed rather than pushing the fields and OK and
  // Cancel out of sight (mitcad#198).
  setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
  auto* layout = new QVBoxLayout(this);
  if (chromeStyle() == ChromeStyle::Floating) {
    layout->setContentsMargins(4, 4, 4, 4); // the card has its own margins
    layout->setSpacing(10);
  }

  auto* header = new QHBoxLayout;
  auto* icon = new QLabel;
  icon->setPixmap(themeIcon(def.icon).pixmap(QSize(24, 24), devicePixelRatioF()));
  auto* title = new QLabel(QStringLiteral("<b>%1</b>%2").arg(
      def.name.toHtmlEscaped(), editing ? tr(" <span style='color:gray'>(edit)</span>") : QString()));
  header->addWidget(icon);
  header->addWidget(title, 1);
  layout->addLayout(header);

  m_form = new QFormLayout;
  m_form->setFieldGrowthPolicy(QFormLayout::AllNonFixedFieldsGrow);
  if (chromeStyle() == ChromeStyle::Floating) {
    m_form->setVerticalSpacing(8);
    m_form->setHorizontalSpacing(10);
    m_form->setRowWrapPolicy(QFormLayout::WrapLongRows);
    m_form->setLabelAlignment(Qt::AlignLeft | Qt::AlignVCenter);
  }
  int selectionIndex = 0;
  for (const InputDef& input : def.inputs) {
    if (input.type == InputDef::Type::Selection) {
      m_colors.insert(input.id, input.color.isValid() ? input.color : paletteColor(selectionIndex));
      ++selectionIndex;
    } else if (input.type == InputDef::Type::List) {
      m_listColors.insert(input.id, selectionIndex);
      ++selectionIndex;
    }
  }
  for (const InputDef& input : def.inputs) {
    createRow(input, input.id, m_form, state, QString(), -1);
  }
  layout->addLayout(m_form);

  // In the panel before it is shown: shown without a parent, the label
  // would be a window of its own for a moment, which takes the activation
  // from the main window, and when it goes the system may hand it to
  // another application's window, so that Enter no longer reaches the
  // command's OK (mitcad#255).
  m_result = new QLabel(this);
  m_result->setObjectName(QStringLiteral("commandResult"));
  m_result->setWordWrap(true);
  m_result->setTextFormat(Qt::RichText);
  m_result->setTextInteractionFlags(Qt::TextSelectableByMouse);
  m_result->setFrameShape(QFrame::StyledPanel);
  m_result->setMargin(4);
  layout->addWidget(m_result);
  m_result->setVisible(static_cast<bool>(def.inspect));

  // Errors and hints (mitcad#198): within the panel's width however long
  // their words, scrolling after a few lines, hidden when there is none.
  m_message = new CommandMessage;
  m_message->setObjectName(QStringLiteral("commandMessage"));
  layout->addWidget(m_message);

  auto* buttons = new QHBoxLayout;
  m_ok = new QPushButton(def.okLabel.isEmpty() ? tr("OK") : def.okLabel);
  m_ok->setObjectName(QStringLiteral("commandOk"));
  // A command that repeats (BUILD's Place, mitcad#375) keeps what it
  // applied: its Cancel only closes it.
  auto* cancel = new QPushButton(def.repeats ? tr("Close") : tr("Cancel"));
  cancel->setObjectName(QStringLiteral("commandCancel"));
  for (QPushButton* button : {m_ok, cancel}) {
    // The keyboard stays in the value fields; Enter and Esc work anywhere.
    button->setFocusPolicy(Qt::NoFocus);
    button->setAutoDefault(false);
  }
  // In a floating card OK is the accent-coloured default button.
  m_ok->setDefault(chromeStyle() == ChromeStyle::Floating);
  m_ok->setToolTip(def.apply                 ? tr("%1 (Enter)").arg(def.okLabel.isEmpty() ? tr("OK") : def.okLabel)
                   : def.inspect             ? tr("Close (Enter)")
                   : def.okLabel.isEmpty()   ? tr("Create the feature (Enter)")
                                             : tr("%1 (Enter)").arg(def.okLabel));
  cancel->setToolTip(def.repeats ? tr("Close; what was placed stays (Esc)") : tr("Close without changes (Esc)"));
  connect(m_ok, &QPushButton::clicked, this, &CommandPanel::accepted);
  connect(cancel, &QPushButton::clicked, this, &CommandPanel::rejected);
  buttons->addStretch();
  buttons->addWidget(m_ok);
  buttons->addWidget(cancel);
  layout->addLayout(buttons);
  layout->addStretch();

  applyControlStyles();
  refresh(state);
}

void CommandPanel::applyControlStyles() {
  if (chromeStyle() != ChromeStyle::Floating) {
    return;
  }
  const QColor accent = accentColor(palette());
  // White on the accent colour, as the title bar's prominent button has it.
  const QColor accentText = palette().color(QPalette::HighlightedText);
  m_result->setFrameShape(QFrame::NoFrame);
  m_result->setMargin(10);
  m_result->setStyleSheet(QStringLiteral(
      "QLabel { background: rgba(128, 128, 128, 18); border: none; border-radius: 10px; }"));
  for (const auto& row : m_rows) {
    if (row->select != nullptr) {
      styleSelect(row->select, accent, accentText);
    }
    if (row->edit != nullptr) {
      styleLineEdit(row->edit, accent, errorColor(palette()));
    }
    if (auto* toggle = qobject_cast<QToolButton*>(row->check); toggle != nullptr && !row->input->disclosure) {
      styleToggle(toggle, accent);
    }
    for (QToolButton* button : row->choiceButtons) {
      styleToggle(button, accent);
    }
    if (row->tile != nullptr) {
      // A tile: rounded like the fields, the accent colour as its focus ring.
      row->tile->setStyleSheet(QStringLiteral(
          "QToolButton { border: 1px solid rgba(128, 128, 128, 70); background: rgba(128, 128, 128, 25);"
          " border-radius: 7px; }"
          "QToolButton:hover { background: rgba(128, 128, 128, 55); }"
          "QToolButton:pressed { background: rgba(128, 128, 128, 90); }"
          "QToolButton:focus { border: 1.5px solid %1; }")
                                   .arg(accent.name()));
    }
  }
}

void CommandPanel::changeEvent(QEvent* event) {
  QWidget::changeEvent(event);
  if (event->type() == QEvent::ApplicationPaletteChange || event->type() == QEvent::PaletteChange) {
    applyControlStyles();
  }
}

CommandPanel::~CommandPanel() = default;

CommandPanel::Row* CommandPanel::createRow(const InputDef& input, const QString& key,
                                           QFormLayout* form, const CommandState& state,
                                           const QString& list, int index) {
  auto owned = std::make_unique<Row>();
  Row& row = *owned;
  row.input = &input;
  row.key = key;
  row.list = list;
  row.index = index;
  row.form = form;
  m_byKey.insert(key, owned.get());
  m_rows.push_back(std::move(owned));

  auto* container = new QWidget;
  auto* column = new QVBoxLayout(container);
  column->setContentsMargins(0, 0, 0, 0);
  column->setSpacing(1);
  QWidget* field = nullptr;
  const CommandState scope = list.isEmpty() ? state : state.row(list, index);

  switch (input.type) {
  case InputDef::Type::Selection: {
    auto* line = new QWidget;
    auto* box = new QHBoxLayout(line);
    box->setContentsMargins(0, 0, 0, 0);
    row.select = new QPushButton(swatchIcon(colorOf(key)), QString());
    row.select->setCheckable(true);
    row.select->setFocusPolicy(Qt::NoFocus);
    row.select->setToolTip(input.tooltip.isEmpty() ? tr("Click to pick %1").arg(input.label)
                                                   : input.tooltip);
    row.clear = smallButton(QStringLiteral("✕"), tr("Clear the selection"));
    row.clear->setObjectName(QStringLiteral("clear_") + key);
    connect(row.select, &QPushButton::clicked, this, [this, key] { emit selectionActivated(key); });
    connect(row.clear, &QToolButton::clicked, this, [this, key] { emit selectionCleared(key); });
    box->addWidget(row.select, 1);
    box->addWidget(row.clear);
    field = line;
    if (input.ordered) {
      // The picks in order, with buttons that move them.
      auto* ordered = new QWidget;
      auto* orderedBox = new QHBoxLayout(ordered);
      orderedBox->setContentsMargins(0, 0, 0, 0);
      row.order = new QListWidget;
      row.order->setFocusPolicy(Qt::NoFocus);
      row.order->setMaximumHeight(90);
      row.order->setObjectName(QStringLiteral("order_") + key);
      auto* moves = new QVBoxLayout;
      moves->setSpacing(0);
      QListWidget* items = row.order;
      auto* up = smallButton(QStringLiteral("▲"), tr("Move up"));
      auto* down = smallButton(QStringLiteral("▼"), tr("Move down"));
      auto* remove = smallButton(QStringLiteral("✕"), tr("Remove"));
      up->setObjectName(QStringLiteral("up_") + key);
      down->setObjectName(QStringLiteral("down_") + key);
      connect(up, &QToolButton::clicked, this, [this, key, items] {
        const int at = items->currentRow();
        if (at > 0) {
          emit itemMoved(key, at, at - 1);
        }
      });
      connect(down, &QToolButton::clicked, this, [this, key, items] {
        const int at = items->currentRow();
        if (at >= 0 && at + 1 < items->count()) {
          emit itemMoved(key, at, at + 1);
        }
      });
      connect(remove, &QToolButton::clicked, this, [this, key, items] {
        if (items->currentRow() >= 0) {
          emit itemRemoved(key, items->currentRow());
        }
      });
      moves->addWidget(up);
      moves->addWidget(down);
      moves->addWidget(remove);
      moves->addStretch();
      orderedBox->addWidget(row.order, 1);
      orderedBox->addLayout(moves);
      column->addWidget(line);
      field = ordered;
    }
    break;
  }
  case InputDef::Type::Value:
  case InputDef::Type::Text: {
    row.edit = new QLineEdit(scope.text(input.id));
    if (input.type == InputDef::Type::Value) {
      row.edit->setToolTip(input.tooltip.isEmpty()
                               ? tr("A value or an expression, such as 20, 2 in or d1 * 2")
                               : input.tooltip);
    } else {
      row.edit->setToolTip(input.tooltip);
    }
    connect(row.edit, &QLineEdit::textEdited, this,
            [this, key](const QString& text) { emit textEdited(key, text); });
    if (input.type == InputDef::Type::Value) {
      connect(row.edit, &QLineEdit::editingFinished, this, [this, key] { emit valueCommitted(key); });
      row.edit->installEventFilter(this); // its keyboard focus (mitcad#250)
      // Parameter suggestions while typing, the fx browser, name = value and
      // Create User Parameter (mitcad#567, mitcad#568).
      ParameterFieldOptions suggest;
      suggest.kind = input.valueKind == ValueKind::Angle      ? ParameterKind::Angle
                     : input.valueKind == ValueKind::Unitless ? ParameterKind::Unitless
                                                              : ParameterKind::Length;
      suggest.own = scope.parameterName(input.id);
      suggest.label = QStringLiteral("%1 %2").arg(m_def.name, input.label);
      suggest.naming = true;
      ParameterField::attach(row.edit, suggest);
    }
    field = row.edit;
    break;
  }
  case InputDef::Type::Choice: {
    if (input.choiceTile && input.onButton) {
      // A tile: the chosen option's picture and name; a click on it does
      // what the button would (mitcad#427: BUILD's part picker).
      auto* tile = new TileButton([this, key] { emit buttonClicked(key); });
      tile->setToolTip(input.tooltip);
      row.tile = tile;
      field = tile;
      break;
    }
    if (!input.choiceIcons.isEmpty() && !input.choicesFor) {
      // A row of icon buttons, the chosen one pressed (mitcad#394: BUILD's
      // X / Y / Z).
      auto* line = new QWidget;
      auto* box = new QHBoxLayout(line);
      box->setContentsMargins(0, 0, 0, 0);
      box->setSpacing(4);
      for (const auto& option : input.choices) {
        const QString value = option.first;
        const QString label = option.second;
        auto* button = new QToolButton;
        button->setIcon(themeIcon(input.choiceIcons.value(value)));
        button->setIconSize(QSize(20, 20));
        button->setCheckable(true);
        button->setAutoRaise(true);
        button->setFocusPolicy(Qt::NoFocus);
        button->setToolTip(input.tooltip.isEmpty() ? label : QStringLiteral("%1: %2").arg(label, input.tooltip));
        button->setObjectName(QStringLiteral("input_%1_%2").arg(key, value));
        connect(button, &QToolButton::clicked, this, [this, key, value] {
          emit choiceChanged(key, value);
        });
        row.choiceButtons.insert(value, button);
        box->addWidget(button);
      }
      box->addStretch();
      field = line;
      break;
    }
    row.combo = new QComboBox;
    // No keyboard focus: choosing leaves it in the value field, where the
    // typed value goes.
    row.combo->setFocusPolicy(Qt::NoFocus);
    row.combo->setToolTip(input.tooltip);
    if (input.choiceImage) {
      row.combo->setIconSize(QSize(48, 48)); // the options' pictures
    }
    // Options that follow other inputs are filled by refreshRow.
    if (!input.choicesFor) {
      for (const auto& [value, label] : input.choices) {
        row.combo->addItem(label, value);
      }
    }
    connect(row.combo, &QComboBox::activated, this, [this, key, combo = row.combo](int at) {
      emit choiceChanged(key, combo->itemData(at).toString());
    });
    field = row.combo;
    if (input.onButton) {
      // A button beside the drop-down (mitcad#418: BUILD's part picker).
      auto* line = new QWidget;
      auto* box = new QHBoxLayout(line);
      box->setContentsMargins(0, 0, 0, 0);
      box->setSpacing(4);
      row.button = new QToolButton;
      row.button->setText(input.buttonText);
      row.button->setToolTip(input.buttonTooltip);
      row.button->setFocusPolicy(Qt::NoFocus);
      row.button->setObjectName(QStringLiteral("button_") + key);
      connect(row.button, &QToolButton::clicked, this, [this, key] { emit buttonClicked(key); });
      box->addWidget(row.combo, 1);
      box->addWidget(row.button);
      field = line;
    }
    break;
  }
  case InputDef::Type::Check: {
    if (input.disclosure) {
      // A small button that opens the inputs that follow it ("More",
      // mitcad#394).
      auto* more = new QToolButton;
      more->setCheckable(true);
      more->setAutoRaise(true);
      more->setFocusPolicy(Qt::NoFocus);
      more->setToolButtonStyle(Qt::ToolButtonTextOnly);
      more->setText(QStringLiteral("▸ %1").arg(input.label));
      more->setToolTip(input.tooltip);
      connect(more, &QToolButton::clicked, this, [this, key](bool on) { emit checkChanged(key, on); });
      row.check = more;
      field = more;
      break;
    }
    auto* check = new QCheckBox;
    check->setFocusPolicy(Qt::NoFocus);
    check->setToolTip(input.tooltip);
    // Only as wide as its box, which is where a click toggles it.
    check->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    connect(check, &QCheckBox::clicked, this, [this, key](bool on) { emit checkChanged(key, on); });
    row.check = check;
    field = check;
    break;
  }
  case InputDef::Type::Flip: {
    auto* flip = new QToolButton;
    flip->setIcon(themeIcon(QStringLiteral("flip")));
    flip->setIconSize(QSize(20, 20));
    flip->setCheckable(true);
    flip->setFocusPolicy(Qt::NoFocus);
    flip->setToolTip(input.tooltip.isEmpty() ? tr("Flip the direction") : input.tooltip);
    connect(flip, &QToolButton::clicked, this, [this, key](bool on) { emit checkChanged(key, on); });
    row.check = flip;
    field = flip;
    break;
  }
  case InputDef::Type::CutTargets: {
    row.targets = new CutTargetsPanel;
    row.targets->setScope(scope.object(input.id));
    connect(row.targets, &CutTargetsPanel::scopeChanged, this, [this, key](const QJsonObject& value) {
      auto call = CommandPresenter::actions().objectChanged;
      if (call) { call(key, value); }
    });
    connect(row.targets, &CutTargetsPanel::highlightChanged, this, [this](const Selection& items) {
      auto call = CommandPresenter::actions().highlightRequested;
      if (call) { call(items); }
    });
    field = row.targets;
    break;
  }
  case InputDef::Type::List: {
    auto* box = new QWidget;
    auto* boxLayout = new QVBoxLayout(box);
    boxLayout->setContentsMargins(0, 0, 0, 0);
    boxLayout->setSpacing(2);
    row.rowsLayout = new QVBoxLayout;
    row.rowsLayout->setSpacing(2);
    boxLayout->addLayout(row.rowsLayout);
    if (!input.addLabel.isEmpty()) {
      row.add = new QPushButton(themeIcon(QStringLiteral("add")), input.addLabel);
      row.add->setFocusPolicy(Qt::NoFocus);
      row.add->setObjectName(QStringLiteral("add_") + key);
      connect(row.add, &QPushButton::clicked, this, [this, key] { emit rowAdded(key); });
      boxLayout->addWidget(row.add, 0, Qt::AlignLeft);
    }
    field = box;
    break;
  }
  }
  field->setObjectName(QStringLiteral("input_") + key);
  if (field != container) {
    column->addWidget(field);
  }
  row.note = new QLabel;
  row.note->setWordWrap(true);
  row.note->hide();
  column->addWidget(row.note);
  row.field = container;

  if (input.type == InputDef::Type::List || input.type == InputDef::Type::CutTargets) {
    // A list's rows take the whole width, under its name.
    auto* title = new QLabel(QStringLiteral("<b>%1</b>").arg(input.label.toHtmlEscaped()));
    title->setToolTip(input.tooltip);
    row.title = title;
    form->addRow(title);
    form->addRow(container);
  } else {
    if (!input.heading.isEmpty() && form != nullptr) {
      // The heading of a group of inputs, a row of its own above.
      auto* heading = new QLabel(QStringLiteral("<b>%1</b>").arg(input.heading.toHtmlEscaped()));
      row.title = heading;
      form->addRow(heading);
    }
    // A disclosure button carries its label itself.
    row.label = new QLabel(input.disclosure ? QString() : input.label);
    row.label->setToolTip(input.tooltip);
    if (form != nullptr) {
      form->addRow(row.label, container);
    }
  }
  updateNote(row);
  return &row;
}

void CommandPanel::rebuildList(Row& list, const CommandState& state) {
  const int count = state.rows(list.key);
  if (count == list.built) {
    return;
  }
  const bool first = list.built < 0;
  // The rows' frames go; their widgets may be sending the signal that got
  // here (a row's remove button), so they are deleted later.
  for (QWidget* frame : list.frames) {
    frame->hide();
    frame->deleteLater();
  }
  list.frames.clear();
  list.removes.clear();
  for (auto it = m_rows.begin(); it != m_rows.end();) {
    if ((*it)->list == list.key) {
      m_byKey.remove((*it)->key);
      it = m_rows.erase(it);
    } else {
      ++it;
    }
  }
  const InputDef& input = *list.input;
  // Rows of values only (positions) are lines of a table; others are forms.
  const bool table = std::all_of(input.children.begin(), input.children.end(), [](const InputDef& child) {
    return child.type == InputDef::Type::Value || child.type == InputDef::Type::Text;
  });
  for (int i = 0; i < count; ++i) {
    auto* frame = new QFrame;
    frame->setFrameShape(table ? QFrame::NoFrame : QFrame::StyledPanel);
    auto* frameLayout = new QVBoxLayout(frame);
    frameLayout->setContentsMargins(table ? 0 : 4, table ? 0 : 2, table ? 0 : 4, table ? 0 : 2);
    frameLayout->setSpacing(1);
    auto* header = new QHBoxLayout;
    header->setSpacing(3);
    auto* name = new QLabel(QStringLiteral("<span style='color:gray'>%1</span>")
                                .arg(input.rowLabel.arg(i + 1).toHtmlEscaped()));
    header->addWidget(name, table ? 0 : 1);
    QToolButton* remove = smallButton(QStringLiteral("✕"), tr("Remove this row"));
    remove->setObjectName(QStringLiteral("remove_%1.%2").arg(list.key).arg(i));
    remove->setEnabled(count > input.minRows);
    const QString key = list.key;
    connect(remove, &QToolButton::clicked, this, [this, key, i] { emit rowRemoved(key, i); });
    if (table) {
      for (const InputDef& child : input.children) {
        Row* cell = createRow(child, CommandState::rowKey(list.key, i, child.id), nullptr, state, list.key, i);
        header->addWidget(cell->label);
        header->addWidget(cell->field, 1);
      }
      header->addWidget(remove);
      frameLayout->addLayout(header);
    } else {
      header->addWidget(remove);
      frameLayout->addLayout(header);
      auto* form = new QFormLayout;
      form->setFieldGrowthPolicy(QFormLayout::AllNonFixedFieldsGrow);
      for (const InputDef& child : input.children) {
        createRow(child, CommandState::rowKey(list.key, i, child.id), form, state, list.key, i);
      }
      frameLayout->addLayout(form);
    }
    list.rowsLayout->addWidget(frame);
    list.frames.push_back(frame);
    list.removes.push_back(remove);
  }
  list.built = count;
  applyControlStyles();
  setActiveSelection(m_active);
  if (!first) {
    // Where the new rows are, for UI tests.
    scheduleLogLayout();
  }
}

void CommandPanel::refresh(const CommandState& state) {
  refreshRows(state);
  // A row that moves (one above it grew: a tile's picture, a note) logs
  // the places again (mitcad#528; eventFilter). Once per field.
  for (const auto& row : m_rows) {
    if (row->field != nullptr && !row->field->property("mitcadPlaceWatched").toBool()) {
      row->field->setProperty("mitcadPlaceWatched", true);
      row->field->installEventFilter(this);
    }
  }
  // Rows a choice showed or hid move the others: where they are now, for
  // UI tests.
  QStringList shown;
  for (const Row* row : orderedRows()) {
    if (!row->field->isHidden()) {
      shown << row->key;
    }
  }
  const QString layout = shown.join(QLatin1Char(','));
  if (!m_shownRows.isEmpty() && layout != m_shownRows) {
    scheduleLogLayout();
  }
  m_shownRows = layout;
}

void CommandPanel::scheduleLogLayout() {
  if (m_logTimer == nullptr) {
    m_logTimer = new QTimer(this);
    m_logTimer->setSingleShot(true);
    m_logTimer->setInterval(50);
    connect(m_logTimer, &QTimer::timeout, this, &CommandPanel::logLayout);
    TestSync::watch(m_logTimer);
  }
  m_logTimer->start();
}

void CommandPanel::refreshRows(const CommandState& state) {
  for (const InputDef& input : m_def.inputs) {
    Row* row = m_byKey.value(input.id);
    if (row == nullptr) {
      continue;
    }
    refreshRow(*row, state, state);
    if (input.type == InputDef::Type::List) {
      rebuildList(*row, state);
      for (int i = 0; i < row->built; ++i) {
        const CommandState scope = state.row(input.id, i);
        for (const InputDef& child : input.children) {
          if (Row* childRow = m_byKey.value(CommandState::rowKey(input.id, i, child.id))) {
            refreshRow(*childRow, scope, state);
          }
        }
      }
    }
  }
}

void CommandPanel::refreshRow(Row& row, const CommandState& scope, const CommandState& state) {
  const InputDef& input = *row.input;
  const bool shown = isShown(input, scope);
  if (input.type == InputDef::Type::List || input.type == InputDef::Type::CutTargets) {
    row.form->setRowVisible(row.title, shown);
    row.form->setRowVisible(row.field, shown);
    if (row.targets) { row.targets->setScope(scope.object(input.id)); }
    return;
  }
  if (row.form != nullptr) {
    row.form->setRowVisible(row.field, shown);
    if (row.title != nullptr) {
      row.form->setRowVisible(row.title, shown);
    }
  } else {
    row.label->setVisible(shown);
    row.field->setVisible(shown);
  }
  const QString& id = input.id;
  // A label that follows other inputs (InputDef::labelFor, mitcad#545): a
  // pattern's length row says Spacing or Extent, with the parameter it
  // edits after it as below.
  if (input.labelFor && row.label != nullptr && !input.disclosure) {
    const bool valued = input.type == InputDef::Type::Value || input.type == InputDef::Type::Text;
    const QString name = valued ? scope.parameterName(id) : QString();
    const QString label = input.labelIn(scope);
    const QString text = name.isEmpty() ? label : QStringLiteral("%1 (%2)").arg(label, name);
    if (row.label->text() != text) {
      row.label->setText(text);
      qDebug().noquote() << QStringLiteral("Panel %1 label %2: %3").arg(m_def.name, row.key, text);
    }
  }
  switch (input.type) {
  case InputDef::Type::Selection: {
    const Selection& items = scope.items(id);
    // The picks by name where the input lists them (Combine's tools in
    // occurrences, mitcad#544), else their count.
    const QString listed = items.isEmpty() ? QString() : scope.itemsText(id);
    const QString tip = input.tooltip.isEmpty() ? tr("Click to pick %1").arg(input.label) : input.tooltip;
    if (listed.isEmpty()) {
      row.select->setText(countText(input, static_cast<int>(items.size())));
      row.select->setToolTip(tip);
    } else {
      row.select->setText(row.select->fontMetrics().elidedText(listed, Qt::ElideRight, 220));
      row.select->setToolTip(QStringLiteral("%1\n%2").arg(listed, tip));
    }
    if (row.order != nullptr) {
      QStringList texts;
      for (int i = 0; i < items.size(); ++i) {
        texts << QStringLiteral("%1. %2").arg(i + 1).arg(items[i].describe());
      }
      QStringList shownTexts;
      for (int i = 0; i < row.order->count(); ++i) {
        shownTexts << row.order->item(i)->text();
      }
      if (texts != shownTexts) {
        const int current = row.order->currentRow();
        row.order->clear();
        row.order->addItems(texts);
        row.order->setCurrentRow(std::min(current, static_cast<int>(texts.size()) - 1));
      }
    }
    break;
  }
  case InputDef::Type::Value:
  case InputDef::Type::Text:
    if (row.edit->text() != scope.text(id)) {
      row.edit->setText(scope.text(id));
    }
    // Its own parameter is not offered to it (mitcad#567).
    if (ParameterField* suggest = ParameterField::of(row.edit)) {
      suggest->setOwnParameter(scope.parameterName(id));
    }
    // The parameter the field edits, after its label: "Distance (d5)"
    // (mitcad#231).
    if (const QString name = scope.parameterName(id); row.label != nullptr && !name.isEmpty()) {
      const QString text = QStringLiteral("%1 (%2)").arg(input.labelIn(scope), name);
      if (row.label->text() != text) {
        row.label->setText(text);
        qDebug().noquote() << QStringLiteral("Panel %1 label: %2").arg(m_def.name, text);
      }
      // What the parameter is, as a sketch dimension's tooltip says
      // (mitcad#250): its name, expression, value and comment.
      const QString about = scope.parameterTooltip(id);
      const QString tip = (about.isEmpty() ? QStringLiteral("%1 = %2").arg(name, scope.text(id)) : about) +
                          QLatin1Char('\n') +
                          tr("Type name = value to rename it; a click on a dimension or a value in the view "
                             "inserts its name");
      if (row.edit->toolTip() != tip) {
        row.edit->setToolTip(tip);
        if (!about.isEmpty()) {
          qDebug().noquote() << QStringLiteral("Panel %1 tooltip %2: %3").arg(m_def.name, row.key, about);
        }
      }
    }
    break;
  case InputDef::Type::Choice: {
    if (row.tile != nullptr) {
      // A tile: the chosen option as the input says it is.
      const ChoiceTile tile = input.choiceTile(scope);
      if (static_cast<TileButton*>(row.tile)->setTile(tile)) {
        qDebug().noquote() << QStringLiteral("Panel %1 tile %2: %3%4")
                                  .arg(m_def.name, row.key, tile.title,
                                       tile.detail.isEmpty() ? QString() : QStringLiteral(" (%1)").arg(tile.detail));
      }
      break;
    }
    if (row.combo == nullptr) {
      // Icon buttons: the chosen one pressed; those not offered hidden
      // (InputDef::choiceOffered, mitcad#524).
      for (auto it = row.choiceButtons.cbegin(); it != row.choiceButtons.cend(); ++it) {
        const QSignalBlocker blockButton(it.value());
        it.value()->setChecked(it.key() == scope.choice(id));
        it.value()->setVisible(!input.choiceOffered || input.choiceOffered(scope, it.key()));
      }
      break;
    }
    const QSignalBlocker block(row.combo);
    if (input.choicesFor) {
      QVector<QPair<QString, QString>> options = input.choicesFor(scope);
      // A value the options lack (an edited thread's designation the table
      // writes otherwise) stays shown as it is.
      const QString current = scope.choice(id);
      const bool listed = std::any_of(options.begin(), options.end(),
                                      [&current](const auto& option) { return option.first == current; });
      if (!listed && !current.isEmpty()) {
        options.append({current, current});
      }
      bool same = row.combo->count() == options.size();
      for (int i = 0; same && i < options.size(); ++i) {
        same = row.combo->itemData(i).toString() == options[i].first && row.combo->itemText(i) == options[i].second;
      }
      if (!same) {
        row.combo->clear();
        int pictures = 0;
        for (const auto& [value, label] : options) {
          // A picture beside each option that has one (InputDef::choiceImage).
          const QImage image = input.choiceImage ? input.choiceImage(value) : QImage();
          if (image.isNull()) {
            row.combo->addItem(label, value);
          } else {
            row.combo->addItem(QIcon(QPixmap::fromImage(image)), label, value);
            ++pictures;
          }
        }
        QStringList labels;
        for (const auto& option : options) {
          labels << option.second;
        }
        qDebug().noquote() << QStringLiteral("Panel %1 options %2: %3")
                                  .arg(m_def.name, row.key, labels.join(QStringLiteral(" | ")));
        if (input.choiceImage) {
          qDebug().noquote() << QStringLiteral("Panel %1 option pictures %2: %3 of %4")
                                    .arg(m_def.name, row.key)
                                    .arg(pictures)
                                    .arg(options.size());
        }
      }
    }
    row.combo->setCurrentIndex(qMax(0, row.combo->findData(scope.choice(id))));
    break;
  }
  case InputDef::Type::Check:
  case InputDef::Type::Flip: {
    const QSignalBlocker block(row.check);
    row.check->setChecked(scope.checked(id));
    if (input.disclosure) {
      row.check->setText(QStringLiteral("%1 %2").arg(scope.checked(id) ? QStringLiteral("▾") : QStringLiteral("▸"),
                                                     input.label));
    }
    break;
  }
  case InputDef::Type::List:
  case InputDef::Type::CutTargets:
    break;
  }
  Q_UNUSED(state);
}

void CommandPanel::setActiveSelection(const QString& key) {
  m_active = key;
  for (const auto& row : m_rows) {
    if (row->select != nullptr) {
      row->select->setChecked(row->key == key);
    }
  }
}

void CommandPanel::setInputError(const QString& key, const QString& message) {
  m_notes[key].first = message;
  if (Row* row = m_byKey.value(key)) {
    updateNote(*row);
  }
}

void CommandPanel::setValueHint(const QString& key, const QString& hint) {
  m_notes[key].second = hint;
  if (Row* row = m_byKey.value(key)) {
    updateNote(*row);
  }
}

void CommandPanel::updateNote(Row& row) {
  const auto [errorText, hint] = m_notes.value(row.key);
  const bool error = !errorText.isEmpty();
  const QString text = error ? errorText : hint;
  // A long name in an error (an unknown parameter) breaks rather than
  // widening the panel (mitcad#198).
  const QString shown = breakableText(text);
  if (row.note->text() != shown) {
    row.note->setText(shown);
    if (!m_shownRows.isEmpty()) {
      scheduleLogFit();
    }
  }
  setErrorStyleSheet(row.note, error ? QStringLiteral("color: %1;") : QStringLiteral("color: gray;"));
  if (row.note->isHidden() != text.isEmpty()) {
    // The rows below move.
    row.note->setVisible(!text.isEmpty());
    if (!m_shownRows.isEmpty()) {
      scheduleLogLayout();
    }
  }
  if (row.edit != nullptr && chromeStyle() == ChromeStyle::Floating) {
    // The field's style sheet is the card's (styleLineEdit), with the error
    // border for the property.
    row.edit->setProperty("invalid", error);
    row.edit->style()->unpolish(row.edit);
    row.edit->style()->polish(row.edit);
  } else if (row.edit != nullptr) {
    setErrorStyleSheet(row.edit, error ? QStringLiteral("border: 1px solid %1;") : QString());
  }
}

void CommandPanel::setMessage(const QString& message, bool error) {
  setErrorStyleSheet(m_message, error ? QStringLiteral("QTextEdit { color: %1; background: transparent; }")
                                      : QStringLiteral("QTextEdit { color: gray; background: transparent; }"));
  if (message == m_message->message()) {
    return;
  }
  m_message->setMessage(message);
  // What the panel says in red, for UI tests (mitcad#531: the model's
  // refusal of a preview shown on the panel).
  if (error && !message.isEmpty()) {
    qDebug().noquote() << QStringLiteral("Panel %1 error: %2").arg(m_def.name, message);
  }
  scheduleLogFit();
}

void CommandPanel::scheduleLogFit() {
  if (m_fitTimer == nullptr) {
    m_fitTimer = new QTimer(this);
    m_fitTimer->setSingleShot(true);
    m_fitTimer->setInterval(50);
    connect(m_fitTimer, &QTimer::timeout, this, &CommandPanel::logFit);
    TestSync::watch(m_fitTimer);
  }
  m_fitTimer->start();
}

void CommandPanel::logFit() const {
  QCoreApplication::sendPostedEvents(nullptr, QEvent::LayoutRequest);
  // What the page shows of the panel: the scroll area's viewport.
  const QWidget* page = parentWidget() != nullptr ? parentWidget() : this;
  QString shown = QStringLiteral("hidden");
  if (!m_message->isHidden()) {
    shown = QStringLiteral("%1x%2").arg(m_message->width()).arg(m_message->height());
    const QScrollBar* bar = m_message->verticalScrollBar();
    if (bar != nullptr && bar->maximum() > 0) {
      shown += QStringLiteral(" scrolling");
    }
  }
  qDebug().noquote() << QStringLiteral("Panel %1 message %2, panel width %3 of %4")
                            .arg(m_def.name, shown)
                            .arg(width())
                            .arg(page->width());
  // A message that changed its height moved the rows: their new places.
  const int messageHeight = m_message->isHidden() ? 0 : m_message->height();
  if (messageHeight != m_loggedMessageHeight) {
    logLayout();
  }
}

void CommandPanel::setResult(const QString& text) { m_result->setText(text); }

void CommandPanel::setOkEnabled(bool enabled) { m_ok->setEnabled(enabled); }

void CommandPanel::setPreviewReport(const QJsonObject& report) {
  for (const auto& row : m_rows) {
    if (row->targets != nullptr) { row->targets->setPreview(report.value(QStringLiteral("cut_targets")).toObject()); }
  }
}

QColor CommandPanel::colorOf(const QString& key) const {
  const auto own = m_colors.constFind(key);
  if (own != m_colors.cend()) {
    return own.value();
  }
  QString list;
  int index = 0;
  if (splitKey(key, list, index) && m_listColors.contains(list)) {
    if (const Row* row = m_byKey.value(key); row != nullptr && row->input->color.isValid()) {
      return row->input->color;
    }
    return paletteColor(m_listColors.value(list) + index);
  }
  return paletteColor(0);
}

std::vector<const CommandPanel::Row*> CommandPanel::orderedRows() const {
  std::vector<const Row*> rows;
  for (const InputDef& input : m_def.inputs) {
    const Row* row = m_byKey.value(input.id);
    if (row == nullptr) {
      continue;
    }
    rows.push_back(row);
    if (input.type == InputDef::Type::List) {
      for (int i = 0; i < row->built; ++i) {
        for (const InputDef& child : input.children) {
          if (const Row* childRow = m_byKey.value(CommandState::rowKey(input.id, i, child.id))) {
            rows.push_back(childRow);
          }
        }
      }
    }
  }
  return rows;
}

bool CommandPanel::focusFirstValue() {
  for (const Row* row : orderedRows()) {
    if (row->edit != nullptr && row->input->type == InputDef::Type::Value &&
        row->field->isVisibleTo(this)) {
      row->edit->setFocus();
      row->edit->selectAll();
      return true;
    }
  }
  return false;
}

bool CommandPanel::focusValue(const QString& key) {
  const Row* row = m_byKey.value(key);
  if (row == nullptr || row->edit == nullptr || !row->field->isVisibleTo(this)) {
    return false;
  }
  row->edit->setFocus();
  row->edit->selectAll();
  return true;
}

void CommandPanel::logLayout() const {
  const QStringList lines = layoutLines();
  m_loggedMessageHeight = m_message->isHidden() ? 0 : m_message->height();
  m_loggedLines = lines;
  for (const QString& line : lines) {
    qDebug().noquote() << line;
  }
}

void CommandPanel::scheduleMovedCheck() {
  if (m_movedTimer == nullptr) {
    m_movedTimer = new QTimer(this);
    m_movedTimer->setSingleShot(true);
    m_movedTimer->setInterval(50);
    // Where the inputs are now, when it is not what was logged last
    // (mitcad#528): a place logged before the layout settled (a tile's
    // picture read, the dock or the window laid out again) would else stay
    // the last one, and a test would click where the input was.
    connect(m_movedTimer, &QTimer::timeout, this, [this] {
      if (!m_loggedLines.isEmpty() && isVisible() && layoutLines() != m_loggedLines) {
        logLayout();
      }
    });
    TestSync::watch(m_movedTimer);
  }
  m_movedTimer->start();
}

void CommandPanel::showEvent(QShowEvent* event) {
  QWidget::showEvent(event);
  // The panel and every widget it is in, up to the window: a move or a
  // resize of any of them moves the inputs (mitcad#528).
  for (QWidget* widget = this; widget != nullptr; widget = widget->parentWidget()) {
    widget->installEventFilter(this);
  }
  scheduleMovedCheck();
}

QStringList CommandPanel::layoutLines() const {
  QStringList lines;
  // Rows made or shown just now are placed first.
  QCoreApplication::sendPostedEvents(nullptr, QEvent::LayoutRequest);
  const auto position = [](const QWidget* widget) {
    const QPoint center = GlassCard::mapToHost(widget, widget->rect().center());
    return QStringLiteral("%1,%2").arg(center.x()).arg(center.y());
  };
  for (const Row* row : orderedRows()) {
    const QWidget* widget = row->select != nullptr  ? static_cast<QWidget*>(row->select)
                            : row->edit != nullptr  ? static_cast<QWidget*>(row->edit)
                            : row->combo != nullptr ? static_cast<QWidget*>(row->combo)
                            : row->check != nullptr ? static_cast<QWidget*>(row->check)
                            : row->add != nullptr   ? static_cast<QWidget*>(row->add)
                                                    : row->field;
    const bool hidden = !row->field->isVisibleTo(this);
    lines << QStringLiteral("Panel %1 input %2 at %3%4")
                              .arg(m_def.name, row->key, position(widget),
                                   hidden ? QStringLiteral(" (hidden)") : QString());
    if (!hidden) {
      // Each icon button of a choice (InputDef::choiceIcons).
      for (auto it = row->choiceButtons.cbegin(); it != row->choiceButtons.cend(); ++it) {
        if (!it.value()->isVisibleTo(this)) {
          continue; // not offered now (InputDef::choiceOffered)
        }
        lines << QStringLiteral("Panel %1 input %2=%3 at %4")
                                  .arg(m_def.name, row->key, it.key(), position(it.value()));
      }
      if (row->button != nullptr) {
        // The button beside a choice (InputDef::onButton).
        lines << QStringLiteral("Panel %1 button_%2 at %3")
                                  .arg(m_def.name, row->key, position(row->button));
      }
    }
    if (row->order != nullptr && !hidden) {
      for (const char* button : {"up_", "down_"}) {
        if (const auto* move = findChild<QToolButton*>(QString::fromLatin1(button) + row->key)) {
          lines << QStringLiteral("Panel %1 %2%3 at %4")
                                    .arg(m_def.name, QString::fromLatin1(button), row->key,
                                         position(move));
        }
      }
    }
    if (row->rowsLayout != nullptr && !hidden) {
      if (row->add != nullptr) {
        lines << QStringLiteral("Panel %1 add %2 at %3")
                                  .arg(m_def.name, row->key, position(row->add));
      }
      for (std::size_t i = 0; i < row->removes.size(); ++i) {
        lines << QStringLiteral("Panel %1 remove %2.%3 at %4")
                                  .arg(m_def.name, row->key)
                                  .arg(i)
                                  .arg(position(row->removes[i]));
      }
    }
  }
  lines << QStringLiteral("Panel %1 OK at %2").arg(m_def.name, position(m_ok));
  // Wider than the page would cut its right side off (mitcad#198).
  const QWidget* page = parentWidget() != nullptr ? parentWidget() : this;
  lines << QStringLiteral("Panel %1 width %2 of %3").arg(m_def.name).arg(width()).arg(page->width());
  // What a long panel's page shows of it: a place outside the view is
  // scrolled away, and a wheel over the bar scrolls it (UI tests in a low
  // window; MainWindow logs the places again when it scrolls).
  const auto* area = parentWidget() != nullptr
                         ? qobject_cast<const QAbstractScrollArea*>(parentWidget()->parentWidget())
                         : nullptr;
  if (area != nullptr && area->viewport() == parentWidget()) {
    const QPoint top = GlassCard::mapToHost(page, QPoint(0, 0));
    const QScrollBar* bar = area->verticalScrollBar();
    QString line = QStringLiteral("Panel %1 view y %2 to %3, scrolled %4 of %5")
                       .arg(m_def.name)
                       .arg(top.y())
                       .arg(top.y() + page->height())
                       .arg(bar->value())
                       .arg(bar->maximum());
    if (bar->isVisible()) {
      line += QStringLiteral(", bar at %1").arg(position(bar));
    }
    lines << line;
  }
  return lines;
}

bool CommandPanel::eventFilter(QObject* watched, QEvent* event) {
  switch (event->type()) {
  case QEvent::Move:
  case QEvent::Resize:
  case QEvent::Show:
  case QEvent::Hide:
    // The panel, a row's field or a widget the panel is in moved: its
    // inputs' places again once it settles, when they changed (mitcad#528).
    if (watched->isWidgetType()) {
      scheduleMovedCheck();
    }
    break;
  default:
    break;
  }
  if (event->type() == QEvent::FocusIn || event->type() == QEvent::FocusOut) {
    for (const auto& row : m_rows) {
      if (row->edit == watched && row->input->type == InputDef::Type::Value) {
        auto call = CommandPresenter::actions().valueFocused;
        if (call) {
          call(row->key, event->type() == QEvent::FocusIn);
        }
        break;
      }
    }
  }
  return QWidget::eventFilter(watched, event);
}

bool CommandPanel::insertIntoValue(const QString& key, const QString& text) {
  Row* row = m_byKey.value(key);
  if (row == nullptr || row->edit == nullptr || !row->field->isVisibleTo(this)) {
    return false;
  }
  // Back where it was: the cursor stays where the view's click left it.
  row->edit->setFocus(Qt::OtherFocusReason);
  row->edit->insert(text);
  const QString now = row->edit->text();
  qDebug().noquote() << QStringLiteral("Panel %1 input %2: inserted %3, now %4").arg(m_def.name, key, text, now);
  emit textEdited(key, now);
  return true;
}

} // namespace mitcad
