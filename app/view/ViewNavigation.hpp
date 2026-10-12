// SPDX-License-Identifier: MIT
#pragma once

#include <cstdint>

namespace mitcad {

// Whether the user is navigating the 3D view (orbit, pan, zoom), from the
// input that moves its camera (mitcad#214, docs/rendering.md "Navigation"):
// what watches the camera (the rendered view) waits for the navigation to
// end instead of following every step.
// - A mouse button held on the view (an orbit, a pan, a drag) navigates
//   until the last button goes up.
// - A trackpad scroll that pans or orbits navigates until its drag ends.
// - Input without a release (a wheel step, a pinch, a roll, a resize) is a
//   step: the navigation lasts until the camera has kept still for
//   kQuietMilliseconds after the last step or camera change.
// - An animated camera move (the orientation cube, a named view, Home)
//   navigates while it runs.
// Plain logic over times in milliseconds (the view's clock); the view
// evaluates it with update() whenever it draws a frame and when the quiet
// time ends (quietLeft()).
class ViewNavigation {
public:
  // A burst of wheel steps or a gesture ends after this much quiet.
  static constexpr int kQuietMilliseconds = 250;

  enum class Change { None, Started, Ended };

  // A button is (still) held on the view, or none any more.
  void setButtons(bool held) { m_buttons = held; }
  bool buttons() const { return m_buttons; }
  // A trackpad scroll's drag (a pan or an orbit) is on, or ended.
  void setScrollDrag(bool on) { m_scrollDrag = on; }
  // A step of input without a release, at `now`.
  void stepped(std::int64_t now) { m_quietUntil = now + kQuietMilliseconds; }
  // The camera moved at `now`: within a step's navigation, the quiet time
  // starts again.
  void cameraMoved(std::int64_t now) {
    if (now < m_quietUntil) {
      m_quietUntil = now + kQuietMilliseconds;
    }
  }
  // Evaluates the state at `now`, with whether a camera animation runs;
  // says whether the navigation started or ended since the last update.
  Change update(std::int64_t now, bool animating) {
    const bool active = m_buttons || m_scrollDrag || animating || now < m_quietUntil;
    if (active == m_active) {
      return Change::None;
    }
    m_active = active;
    return active ? Change::Started : Change::Ended;
  }
  // As of the last update().
  bool active() const { return m_active; }
  // How long the quiet time still lasts at `now` (0: over).
  int quietLeft(std::int64_t now) const {
    return now < m_quietUntil ? static_cast<int>(m_quietUntil - now) : 0;
  }

private:
  bool m_buttons = false;
  bool m_scrollDrag = false;
  bool m_active = false;
  std::int64_t m_quietUntil = 0;
};

} // namespace mitcad
