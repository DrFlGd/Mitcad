// SPDX-License-Identifier: MIT
#pragma once

#include <QtGlobal>

// The UI tests' sync key on Windows (framework/TestSync.hpp): the Windows UI
// test library (tools/ui-windows-lib.ps1, Ui-Sync) posts the Pause key to the
// application's window. Portable code calls this without #ifdefs: elsewhere
// it is an inline stub. The Windows implementation is
// platform/windows/WindowsTestSync.cpp.
namespace mitcad::win {

#ifdef Q_OS_WIN

// Whether a native message of Qt's event dispatcher ("windows_generic_MSG",
// a MSG) is a key down or up of the Pause key; `released` says which.
bool isSyncKey(const void* message, bool* released);

#else

inline bool isSyncKey(const void*, bool*) { return false; }

#endif

} // namespace mitcad::win
