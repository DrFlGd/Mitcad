// SPDX-License-Identifier: MIT
#include "../WindowsFrame.hpp"

#include <QFontDatabase>
#include <QOperatingSystemVersion>
#include <QStringList>
#include <QWidget>

#include <windows.h>
#include <dwmapi.h>

namespace mitcad::win {
namespace {

// The attributes by number, so that an SDK older than Windows 11's builds
// too (DWMWINDOWATTRIBUTE).
constexpr DWORD kWindowCornerPreference = 33; // DWMWA_WINDOW_CORNER_PREFERENCE
constexpr DWORD kCaptionColor = 35;           // DWMWA_CAPTION_COLOR
constexpr DWORD kTextColor = 36;              // DWMWA_TEXT_COLOR
constexpr DWORD kSystemBackdropType = 38;     // DWMWA_SYSTEMBACKDROP_TYPE, from build 22621
// Mica before build 22621, which has no backdrop types yet: the attribute
// the system's own applications used then (not in the SDK).
constexpr DWORD kMicaEffect = 1029;
constexpr int kRoundCorners = 2; // DWMWCP_ROUND
constexpr int kMica = 2;         // DWMSBT_MAINWINDOW

constexpr int kWindows11 = 22000;
constexpr int kBackdropTypes = 22621;

COLORREF colorRef(const QColor& color) {
  return RGB(static_cast<BYTE>(color.red()), static_cast<BYTE>(color.green()), static_cast<BYTE>(color.blue()));
}

} // namespace

QString applyWindowFrame(QWidget* window, const QColor& caption, const QColor& text) {
  // The build number (QOperatingSystemVersion asks the system, not the
  // manifest's compatibility).
  const int build = QOperatingSystemVersion::current().microVersion();
  if (build < kWindows11) {
    return QStringLiteral("skipped (Windows build %1)").arg(build);
  }
  if (window == nullptr) {
    return QStringLiteral("not applied (no window)");
  }
  // The native window as it is (winId() would create one).
  const HWND hwnd = reinterpret_cast<HWND>(window->internalWinId());
  if (hwnd == nullptr || !IsWindow(hwnd)) {
    return QStringLiteral("not applied (no native window)");
  }
  HRESULT failed = S_OK;
  const auto set = [hwnd, &failed](DWORD attribute, const void* value, DWORD size) {
    const HRESULT result = DwmSetWindowAttribute(hwnd, attribute, value, size);
    if (FAILED(result) && SUCCEEDED(failed)) {
      failed = result;
    }
  };
  const COLORREF captionColor = colorRef(caption);
  set(kCaptionColor, &captionColor, static_cast<DWORD>(sizeof(captionColor)));
  const COLORREF textColor = colorRef(text);
  set(kTextColor, &textColor, static_cast<DWORD>(sizeof(textColor)));
  if (build >= kBackdropTypes) {
    const int backdrop = kMica;
    set(kSystemBackdropType, &backdrop, static_cast<DWORD>(sizeof(backdrop)));
  } else {
    const BOOL mica = TRUE;
    set(kMicaEffect, &mica, static_cast<DWORD>(sizeof(mica)));
  }
  const int corners = kRoundCorners;
  set(kWindowCornerPreference, &corners, static_cast<DWORD>(sizeof(corners)));
  if (FAILED(failed)) {
    return QStringLiteral("not applied (DWM error 0x%1)")
        .arg(static_cast<quint32>(failed), 8, 16, QLatin1Char('0'));
  }
  return QStringLiteral("Windows 11 caption colour, Mica, rounded corners");
}

QString uiFontFamily() {
  // The Text optical size, as the system's own interface uses it; the font
  // database may list the variable font by its family alone.
  for (const QString& family : QStringList{QStringLiteral("Segoe UI Variable Text"), QStringLiteral("Segoe UI Variable")}) {
    if (QFontDatabase::hasFamily(family)) {
      return family;
    }
  }
  return {};
}

} // namespace mitcad::win
