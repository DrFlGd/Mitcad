// SPDX-License-Identifier: MIT
#pragma once

#include <memory>

#include "../framework/CommandPresenter.hpp"

namespace mitcad {

class CommandSession;

// Owns an AppKit form. The window retains its NSView while installed; remove
// it from the inspector before destroying the form. Destroying the session
// first is also safe: it clears the presenter's actions.
class NativeCommandForm : public CommandPresenter {
public:
  virtual void* nativeView() const = 0;
};

#ifdef Q_OS_MACOS
std::unique_ptr<NativeCommandForm> createNativeCommandForm(CommandSession& session);
#else
inline std::unique_ptr<NativeCommandForm> createNativeCommandForm(CommandSession&) { return {}; }
#endif

} // namespace mitcad
