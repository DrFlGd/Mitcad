// SPDX-License-Identifier: MIT
#pragma once

#include <QString>
#include <QWidget>

namespace mitcad {

class OcctViewer;

// A card at the top of the 3D view that says what the window waits for
// while a mode lasts (mitcad#474: Create Sketch waiting for a plane), in
// every layout: it is a child of the view itself, so the Docked and the
// Floating layout and the native macOS window show it alike, and no hint
// of the status line replaces it. It lets the mouse through to the view
// and keeps below the cards that float over the view's top
// (OcctViewer::overlayInsets). Logs "View banner: <text>" when it shows a
// text, "View banner at x,y WxH" (in the window) when it moves and "View
// banner hidden" when it goes.
class ModeBanner : public QWidget {
  Q_OBJECT

public:
  explicit ModeBanner(OcctViewer* viewer);

  // Shows the text (empty: hides the banner).
  void setText(const QString& text);
  QString text() const { return m_text; }

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void paintEvent(QPaintEvent* event) override;

private:
  // The card's rectangle in the view's coordinates.
  QRect cardRect() const;
  void place();

  OcctViewer* m_viewer;
  QString m_text;
  QString m_loggedPlace; // "View banner at x,y WxH" in the window, last logged
};

} // namespace mitcad
