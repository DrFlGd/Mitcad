// SPDX-License-Identifier: MIT
#include "BrowserBackground.hpp"

#include <algorithm>
#include <vector>

#include <QObject>
#include <QPointer>
#include <QRect>
#include <QSettings>
#include <QtLogging>

#include "../framework/ChromeStyle.hpp"
#include "../framework/GlassCard.hpp"
#include "BrowserPanel.hpp"

namespace mitcad {
namespace {

const QString kKey = QStringLiteral("appearance/browserBackground");

struct Follower {
  QPointer<GlassCard> card;
  QPointer<BrowserPanel> browser;
};

std::vector<Follower>& followers() {
  static std::vector<Follower> list;
  return list;
}

void apply(GlassCard* card, BrowserPanel* browser, BrowserBackground background) {
  const bool rows = background == BrowserBackground::Rows;
  // Native glass is a window's, behind the whole card: a bare card is a
  // widget over the view, which draws its boxes itself.
  const auto glass = [card](bool windowed) {
    if (!glassActive() || card->isWindowed() == windowed) {
      return;
    }
    const bool shown = card->isShown();
    const QRect slot = card->slot();
    card->setWindowed(windowed);
    card->setSlot(slot);
    card->setVisible(shown);
  };
  if (rows) {
    glass(false);
    browser->setRowsBackground(true);
    card->setBare(true);
    card->setContentRegion(browser->rowsRegion());
  } else {
    card->setBare(false);
    browser->setRowsBackground(false);
    glass(true);
  }
  qDebug().noquote() << QStringLiteral("Browser background: %1").arg(browserBackgroundId(background));
}

} // namespace

BrowserBackground savedBrowserBackground() {
  return QSettings().value(kKey).toString() == QLatin1String("rows") ? BrowserBackground::Rows
                                                                       : BrowserBackground::Card;
}

void saveBrowserBackground(BrowserBackground background) {
  QSettings().setValue(kKey, browserBackgroundId(background));
  std::vector<Follower>& list = followers();
  const auto gone = [](const Follower& follower) { return follower.card.isNull() || follower.browser.isNull(); };
  list.erase(std::remove_if(list.begin(), list.end(), gone), list.end());
  for (const Follower& follower : list) {
    if (follower.card->isBare() != (background == BrowserBackground::Rows)) {
      apply(follower.card, follower.browser, background);
    }
  }
}

QString browserBackgroundId(BrowserBackground background) {
  return background == BrowserBackground::Rows ? QStringLiteral("rows") : QStringLiteral("card");
}

void followBrowserBackground(GlassCard* card, BrowserPanel* browser) {
  followers().push_back({card, browser});
  // The rows scrolled, opened, folded, changed or were selected: where the
  // card takes the mouse follows.
  QObject::connect(browser, &BrowserPanel::rowsRegionChanged, card, [card, browser] {
    if (card->isBare()) {
      card->setContentRegion(browser->rowsRegion());
    }
  });
  apply(card, browser, savedBrowserBackground());
}

} // namespace mitcad
