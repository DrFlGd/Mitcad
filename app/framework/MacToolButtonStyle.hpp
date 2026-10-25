// SPDX-License-Identifier: MIT
#pragma once

class QWidget;
class QToolButton;

namespace mitcad {

// Shared borderless tool controls on macOS, including buttons created after
// the main window and buttons in its owned menus and dialogs. Other platforms
// retain their current style. Native widgets and custom-painted buttons keep
// their own painting; this replaces only QToolButton's platform bezel.
// Initial enrollment is immediate; an external setStyle replacement is
// repaired by a coalesced queued call after Qt finishes its style transaction.
void installMacToolButtonStyle(QWidget* root);
void styleMacToolButton(QToolButton* button);

// UI diagnostics: sample the button's effective style (including a style
// sheet wrapper) without its icon, text, menu or interactive state.
bool macToolButtonPaintsIdleSurface(const QToolButton* button);

} // namespace mitcad
