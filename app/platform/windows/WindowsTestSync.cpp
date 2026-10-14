// SPDX-License-Identifier: MIT
#include "../WindowsTestSync.hpp"

#include <windows.h>

namespace mitcad::win {

bool isSyncKey(const void* message, bool* released) {
  const auto* msg = static_cast<const MSG*>(message);
  const bool down = msg->message == WM_KEYDOWN || msg->message == WM_SYSKEYDOWN;
  const bool up = msg->message == WM_KEYUP || msg->message == WM_SYSKEYUP;
  if ((!down && !up) || msg->wParam != VK_PAUSE) {
    return false;
  }
  *released = up;
  return true;
}

} // namespace mitcad::win
