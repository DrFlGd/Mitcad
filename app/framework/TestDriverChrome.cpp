// SPDX-License-Identifier: MIT
#include "TestDriver.hpp"

#include <QAction>
#include <QAccessible>
#include <QDebug>
#include <QLayout>
#include <QMenu>
#include <QRegion>
#include <QScrollArea>
#include <QScrollBar>
#include <QToolButton>
#include <QTabBar>

#include "ChromeStyle.hpp"
#include "Ribbon.hpp"
#include "TitleBar.hpp"

namespace mitcad {

void TestDriver::testRibbonLabels(const QStringList& expected) {
  whenReady([this, expected] {
    auto* ribbon = m_main ? m_main->findChild<Ribbon*>() : nullptr;
    QTabBar* tabs = ribbon ? ribbon->tabBar() : nullptr;
    if (tabs == nullptr || tabs->count() != expected.size()) return false;
    QAccessibleInterface* accessible = QAccessible::queryAccessibleInterface(tabs);
    for (int i = 0; i < tabs->count(); ++i) {
      if (tabs->tabText(i) != expected.at(i)) return false;
      QAccessibleInterface* tab = accessible ? accessible->child(i) : nullptr;
      if (tab == nullptr || tab->text(QAccessible::Name) != expected.at(i)) return false;
    }
    for (QToolButton* button : ribbon->findChildren<QToolButton*>()) {
      if (!button->objectName().startsWith(QLatin1String("ribbonMenu_"))) continue;
      // These captions are ordinary words (Create, Sheet Metal, ...), not
      // file formats or user names. Check the actual text used by painting.
      const QString text = button->text();
      if (text.isEmpty() || text == text.toUpper() || button->font().capitalization() == QFont::AllUppercase) {
        m_waitDetail = QStringLiteral("all-caps group caption: %1").arg(text);
        return false;
      }
      if (!button->isVisible()) continue;
      if (button->width() < button->sizeHint().width() || button->menu() == nullptr || button->menu()->isEmpty()) {
        m_waitDetail = QStringLiteral("clipped caption or missing group menu: %1 (%2 < %3)")
                           .arg(text).arg(button->width()).arg(button->sizeHint().width());
        return false;
      }
      // Overflow may put a group outside the current viewport, but ordinary
      // horizontal scrolling must be able to expose its entire menu button.
      for (QWidget* parent = button->parentWidget(); parent != nullptr && parent != ribbon; parent = parent->parentWidget()) {
        auto* scroll = qobject_cast<QScrollArea*>(parent);
        if (scroll == nullptr || scroll->objectName() != QLatin1String("ribbonPageScroll")) continue;
        const QRect caption(button->mapTo(scroll->widget(), QPoint()), button->size());
        const int viewport = scroll->viewport()->width();
        QScrollBar* bar = scroll->horizontalScrollBar();
        const QRect reachable(0, 0, viewport + bar->maximum(), scroll->viewport()->height());
        QWidget* wrapper = scroll->parentWidget();
        auto* back = wrapper->findChild<QToolButton*>(QStringLiteral("ribbonScrollLeft"));
        auto* next = wrapper->findChild<QToolButton*>(QStringLiteral("ribbonScrollRight"));
        const auto exposed = [ribbon, wrapper](QToolButton* arrow) {
          if (arrow == nullptr || !arrow->isVisible() || arrow->width() <= 0 || arrow->height() <= 0 ||
              arrow->width() < arrow->sizeHint().width() || arrow->height() < arrow->sizeHint().height()) return false;
          return wrapper->rect().contains(QRect(arrow->mapTo(wrapper, QPoint()), arrow->size())) &&
                 ribbon->rect().contains(QRect(arrow->mapTo(ribbon, QPoint()), arrow->size())) &&
                 QRegion(arrow->rect()).subtracted(arrow->visibleRegion()).isEmpty();
        };
        if (caption.width() > viewport || !reachable.contains(caption) || back == nullptr || next == nullptr ||
            (bar->maximum() > 0 && (!exposed(back) || !exposed(next))) ||
            back->isEnabled() != (bar->value() > bar->minimum()) || next->isEnabled() != (bar->value() < bar->maximum())) {
          const auto metrics = [](const QWidget* widget) {
            if (widget == nullptr) return QStringLiteral("missing");
            QString detail;
            QDebug(&detail).nospace() << widget->objectName() << " rect=" << widget->geometry()
                << " hint=" << widget->sizeHint() << " minimumHint=" << widget->minimumSizeHint()
                << " minimum=" << widget->minimumSize() << " visible=" << widget->isVisible()
                << " enabled=" << widget->isEnabled() << " exposed=" << widget->visibleRegion().boundingRect();
            return detail;
          };
          m_waitDetail = QStringLiteral("ribbon overflow cannot expose %1: caption=%2x%3; wrapper={%4}; "
                                       "scroll={%5}; viewport={%6}; content={%7}; left={%8}; right={%9}; range=%10..%11 value=%12")
              .arg(text).arg(caption.width()).arg(caption.height()).arg(metrics(wrapper), metrics(scroll),
                  metrics(scroll->viewport()), metrics(scroll->widget()), metrics(back), metrics(next))
              .arg(bar->minimum()).arg(bar->maximum()).arg(bar->value());
          return false;
        }
      }
    }
    qInfo() << "Ribbon labels: passed" << expected;
    return true;
  }, 6, QStringLiteral("workspace labels/accessibility, complete captions or reachable group menus differ"));
}

void TestDriver::testSketchFinishLayout() {
  whenReady([this] {
    QWidget* window = m_main.data();
    if (window == nullptr || window->findChild<QWidget*>(QStringLiteral("titleFinishSketch")) != nullptr) {
      return false;
    }
    auto* action = window->findChild<QAction*>(QStringLiteral("command_sketch.finish"));
    auto* ribbon = window->findChild<Ribbon*>();
    auto* palette = window->findChild<QToolButton*>(QStringLiteral("paletteFinish"));
    auto* group = ribbon ? ribbon->findChild<QToolButton*>(QStringLiteral("ribbonMenu_SKETCH/FINISH SKETCH")) : nullptr;
    if (action == nullptr || !action->isEnabled() || ribbon == nullptr || group == nullptr ||
        !group->isVisible() || group->menu() == nullptr || !group->menu()->actions().contains(action) ||
        palette == nullptr || !palette->isVisible() || !palette->isEnabled() || palette->defaultAction() != action) {
      return false;
    }
    if (!ribbon->rect().contains(QRect(group->mapTo(ribbon, QPoint()), group->size()))) {
      return false;
    }
    auto* title = window->findChild<TitleBar*>();
    if (chromeStyle() == ChromeStyle::Floating && title == nullptr) {
      return false;
    }
    if (title != nullptr) {
      for (QToolButton* button : title->findChildren<QToolButton*>()) {
        if (button->defaultAction() == action) return false;
      }
      QWidget* search = title->findChild<QWidget*>(QStringLiteral("titleSearch"));
      QLayout* row = title->layout();
      // Search is the final layout item and reaches its normal right margin:
      // removing Finish Sketch must release its whole slot and spacing.
      if (search == nullptr || row == nullptr || row->indexOf(search) != row->count() - 1 ||
          title->width() - search->geometry().right() - 1 != row->contentsMargins().right()) {
        return false;
      }
      if (title->isFloatingRow()) {
        QRegion controls;
        for (const auto& part : title->rowParts()) {
          if (part.first == QLatin1String("titleFinishSketch")) return false;
          controls += part.second;
        }
        // All remaining space passes through to the viewport, including the
        // former Finish Sketch slot. No stale mouse mask may survive.
        if (controls != title->mask()) return false;
      }
    }
    qInfo() << "Sketch finish layout: title none, ribbon and palette enabled, no reserved title slot";
    return true;
  }, 6, QStringLiteral("duplicate title Finish Sketch, stale title slot/mask, or missing ribbon/palette action"));
}

} // namespace mitcad
