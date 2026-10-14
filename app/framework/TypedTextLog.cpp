// SPDX-License-Identifier: MIT
#include "framework/TypedTextLog.hpp"

#include <QApplication>
#include <QDialog>
#include <QDoubleSpinBox>
#include <QFileDialog>
#include <QKeyEvent>
#include <QKeySequence>
#include <QLineEdit>
#include <QSpinBox>
#include <QStringList>
#include <QWidget>
#include <QtLogging>

#include <functional>
#include <utility>

namespace mitcad {
namespace {

constexpr const char* kWatched = "mitcadTypedTextLogged";

// The text field a widget that took the keyboard edits: itself, or the line
// edit of a spin box or an editable combo box.
QLineEdit* textFieldOf(QWidget* widget) {
  if (auto* edit = qobject_cast<QLineEdit*>(widget)) {
    return edit;
  }
  return widget->findChild<QLineEdit*>(QString(), Qt::FindDirectChildrenOnly);
}

// The id of the command panel's input a text field belongs to (its row is
// named "input_<id>", the panel "commandPanel"; framework/CommandPanel.cpp),
// empty if none.
QString panelInputOf(const QWidget* edit) {
  const QString prefix = QStringLiteral("input_");
  for (const QWidget* widget = edit; widget != nullptr; widget = widget->parentWidget()) {
    if (!widget->objectName().startsWith(prefix)) {
      continue;
    }
    for (const QWidget* panel = widget->parentWidget(); panel != nullptr; panel = panel->parentWidget()) {
      if (panel->objectName() == QLatin1String("commandPanel")) {
        return widget->objectName().mid(prefix.size());
      }
    }
    return {};
  }
  return {};
}

// Logs the text of the field a widget that took the keyboard edits, each
// time the user edits it, after the prefix (asked for each time: a dialog
// may change its title). A spin box's text is its number without its
// prefix and suffix; its line edit tells of typing only as textChanged.
void logEdits(QWidget* focused, QLineEdit* edit, std::function<QString()> prefix) {
  const auto log = [focused, edit, prefix = std::move(prefix)] {
    if (edit->echoMode() != QLineEdit::Normal) {
      return;
    }
    QString text = edit->text();
    if (const auto* decimal = qobject_cast<const QDoubleSpinBox*>(focused)) {
      text = decimal->cleanText();
    } else if (const auto* whole = qobject_cast<const QSpinBox*>(focused)) {
      text = whole->cleanText();
    }
    qInfo().noquote() << prefix() + text;
  };
  if (focused == edit) {
    QObject::connect(edit, &QLineEdit::textEdited, edit, log);
  } else {
    QObject::connect(edit, &QLineEdit::textChanged, edit, log);
  }
}

void watchFileDialog(QFileDialog* dialog, QLineEdit* edit) {
  QObject::connect(edit, &QLineEdit::textChanged, edit, [dialog](const QString& text) {
    qInfo().noquote() << QStringLiteral("File dialog %1: %2").arg(dialog->windowTitle(), text);
  });
  if (!dialog->property(kWatched).toBool()) {
    dialog->setProperty(kWatched, true);
    QObject::connect(dialog, &QDialog::finished, dialog, [dialog](int result) {
      qInfo().noquote() << QStringLiteral("File dialog %1: %2")
                               .arg(dialog->windowTitle(), result == QDialog::Accepted ? QStringLiteral("accepted")
                                                                                       : QStringLiteral("rejected"));
    });
  }
}

void focusChanged(QWidget* now) {
  if (now == nullptr) {
    return;
  }
  QLineEdit* edit = textFieldOf(now);
  if (edit == nullptr || edit->property(kWatched).toBool()) {
    return;
  }
  QWidget* window = edit->window();
  if (auto* files = qobject_cast<QFileDialog*>(window)) {
    // Qt's own file dialog names its file name field so.
    if (edit->objectName() == QLatin1String("fileNameEdit")) {
      edit->setProperty(kWatched, true);
      watchFileDialog(files, edit);
    }
    return;
  }
  if (qobject_cast<QDialog*>(window) != nullptr) {
    edit->setProperty(kWatched, true);
    logEdits(now, edit, [window] { return QStringLiteral("Dialog field %1: ").arg(window->windowTitle()); });
    return;
  }
  const QString input = panelInputOf(edit);
  if (!input.isEmpty()) {
    edit->setProperty(kWatched, true);
    logEdits(now, edit, [input] { return QStringLiteral("Panel field %1: ").arg(input); });
  }
}

// Logs each key press once: a shortcut's ShortcutOverride and the key press
// that follows when no shortcut takes the key reach the event filter for
// every widget they pass, so the same key (its time, codes and modifiers)
// is logged only the first time.
class KeyLog : public QObject {
public:
  explicit KeyLog(QObject* parent) : QObject(parent) {}

protected:
  bool eventFilter(QObject* watched, QEvent* event) override {
    if (event->type() == QEvent::ShortcutOverride || event->type() == QEvent::KeyPress) {
      log(*static_cast<const QKeyEvent*>(event));
    }
    return QObject::eventFilter(watched, event);
  }

private:
  struct Press {
    quint64 time = 0;
    int key = 0;
    quint32 native = 0;
    quint32 scanCode = 0;
    int modifiers = 0;
    bool operator==(const Press& other) const {
      return time == other.time && key == other.key && native == other.native && scanCode == other.scanCode &&
             modifiers == other.modifiers;
    }
  };

  void log(const QKeyEvent& event) {
    if (event.isAutoRepeat()) {
      return;
    }
    const Qt::KeyboardModifiers modifiers = event.modifiers();
    const Press press{event.timestamp(), event.key(), event.nativeVirtualKey(), event.nativeScanCode(),
                      static_cast<int>(modifiers.toInt())};
    if (press == m_last) {
      return;
    }
    m_last = press;
    QStringList held;
    const std::pair<Qt::KeyboardModifier, const char*> names[] = {{Qt::ControlModifier, "ctrl"},
                                                                  {Qt::AltModifier, "alt"},
                                                                  {Qt::ShiftModifier, "shift"},
                                                                  {Qt::MetaModifier, "meta"}};
    for (const auto& [modifier, name] : names) {
      if (modifiers.testFlag(modifier)) {
        held << QString::fromLatin1(name);
      }
    }
    // A character the window got without a key (WM_CHAR) has no key code:
    // its text instead.
    const QString key = event.key() != 0 ? QKeySequence(event.key()).toString(QKeySequence::PortableText)
                                         : QStringLiteral("'%1'").arg(event.text());
    qInfo().noquote() << QStringLiteral("Key press: %1%2 (native %3)")
                             .arg(key.isEmpty() ? QString::number(event.key()) : key,
                                  held.isEmpty() ? QString() : QStringLiteral(" with ") + held.join(QLatin1Char('+')))
                             .arg(event.nativeVirtualKey());
  }

  Press m_last;
};

} // namespace

void installKeyLogIfRequested() {
  if (qEnvironmentVariableIntValue("MITCAD_LOG_KEYS") == 0 || qApp == nullptr) {
    return;
  }
  qApp->installEventFilter(new KeyLog(qApp));
  qInfo().noquote() << QStringLiteral("Key log on");
}

void installTypedTextLogIfRequested() {
  if (qEnvironmentVariableIntValue("MITCAD_LOG_TYPED_TEXT") == 0 || qApp == nullptr) {
    return;
  }
  QObject::connect(qApp, &QApplication::focusChanged, qApp, [](QWidget*, QWidget* now) { focusChanged(now); });
}

} // namespace mitcad
