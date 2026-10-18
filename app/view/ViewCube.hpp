// SPDX-License-Identifier: MIT
#pragma once

#include <QToolButton>

#include <AIS_ViewCube.hxx>

class QEnterEvent;
class QEvent;
class QMouseEvent;
class QPaintEvent;

namespace mitcad {

// The orientation cube (mitcad#430): AIS_ViewCube's picking, hover, turns
// and logs, drawn in glass of its own. Over a dark background dark smoked
// glass with light names, over a light one frosted light glass with dark
// names; the faces a little see-through (only those in front are drawn, the
// names are in their textures), the edges and corners AIS_ViewCube's flat
// bevels, a shade apart, the axes in colour (X red, Y green, Z blue), the
// hover in the accent.
occ::handle<AIS_ViewCube> makeViewCube();

// The axes' thickness and letters for the cube's size, after
// AIS_ViewCube::SetSize (which resets the facets, padding and font).
void scaleViewCube(AIS_ViewCube& cube);

// Whether the view's background is dark: the cube's glass follows it. The
// caller redisplays the cube.
void setViewCubeBackdrop(AIS_ViewCube& cube, bool dark);

// The Home button at the cube: a small capsule of the cube's glass, its
// house in the colour of the cube's names, the accent on hover.
class ViewCubeHomeButton : public QToolButton {
public:
  explicit ViewCubeHomeButton(QWidget* parent);
  // The glass over a dark background or a light one.
  void setDark(bool dark);

protected:
  void paintEvent(QPaintEvent* event) override;
  void enterEvent(QEnterEvent* event) override;
  void leaveEvent(QEvent* event) override;
  void mouseMoveEvent(QMouseEvent* event) override;

private:
  bool m_dark = false;
  bool m_hovered = false;
};

} // namespace mitcad
