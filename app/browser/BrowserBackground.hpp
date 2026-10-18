// SPDX-License-Identifier: MIT
#pragma once

#include <QString>

namespace mitcad {

class BrowserPanel;
class GlassCard;

// The Browser's background in the Floating layout (mitcad#419; Preferences
// › Display, the setting appearance/browserBackground): its card ("card"),
// or no card and a small box of the card's material behind each row and
// behind the header ("rows"), the 3D view showing between and beside them
// and taking the mouse there. The Docked layout has no such background; the
// choice is kept for the Floating layout (mitcad#429: Preferences shows it in
// both layouts).
enum class BrowserBackground { Card, Rows };

// Without a saved choice: Rows on Windows and Linux, the card on macOS
// (mitcad#429).
BrowserBackground defaultBrowserBackground();
// The saved choice, else the default.
BrowserBackground savedBrowserBackground();
// Saves the choice; every Browser card that follows it (followBrowserBackground)
// changes at once.
void saveBrowserBackground(BrowserBackground background);
// "card" or "rows", as saved and logged.
QString browserBackgroundId(BrowserBackground background);

// The card that holds the browser follows the saved choice now and whenever
// it is saved again: with Rows the card is bare (GlassCard::setBare) and
// takes the mouse only where the rows are (BrowserPanel::rowsRegion). A
// windowed card (native glass) is a widget over the view while bare. Logs
// "Browser background: card" (rows).
void followBrowserBackground(GlassCard* card, BrowserPanel* browser);

} // namespace mitcad
