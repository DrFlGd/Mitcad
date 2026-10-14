// SPDX-License-Identifier: MIT
#pragma once

#include <QColor>
#include <QString>
#include <QtGlobal>

class QWidget;

// The Windows-only parts of the Docked chrome's window (mitcad#251), which
// need the Windows API. Portable code calls these functions without #ifdefs:
// on other platforms they are inline stubs that do nothing. The Windows
// implementation is platform/windows/WindowsFrame.cpp.
namespace mitcad::win {

#ifdef Q_OS_WIN

// The Windows 11 window frame, through DwmSetWindowAttribute: the title bar
// in `caption` (the menu bar's colour, so that the top edge reads as one
// surface) with its text in `text`, the Mica backdrop and rounded corners.
// Skipped on Windows before 11 (build 22000), decided at run time. The window
// must have its native handle (once it is shown, or from its show event on).
// Safe to call again, e.g. when the theme changes. Returns what was done, for
// the log: "Windows 11 caption colour, Mica, rounded corners", "skipped
// (Windows build N)", "not applied (no native window)" or "not applied (DWM
// error 0x...)".
QString applyWindowFrame(QWidget* window, const QColor& caption, const QColor& text);

// The interface font of Windows 11, Segoe UI Variable, where it is
// installed: its family name, or empty (the platform's default font stays).
QString uiFontFamily();

#else

inline QString applyWindowFrame(QWidget*, const QColor&, const QColor&) { return {}; }
inline QString uiFontFamily() { return {}; }

#endif

} // namespace mitcad::win
