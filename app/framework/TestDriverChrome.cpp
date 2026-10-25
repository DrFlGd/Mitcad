// SPDX-License-Identifier: MIT
#include "TestDriver.hpp"

#include <QAction>
#include <QDebug>
#include <QLayout>
#include <QMenu>
#include <QRegion>
#include <QToolButton>

#include "ChromeStyle.hpp"
#include "Ribbon.hpp"
#include "TitleBar.hpp"

namespace mitcad {

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
