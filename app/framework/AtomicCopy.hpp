// SPDX-License-Identifier: MIT
#pragma once

#include <array>

#include <QIODevice>
#include <QSaveFile>
#include <QString>

namespace mitcad {

// Copy an open source without exposing a partial replacement. QSaveFile's
// direct-write fallback stays disabled, so every failure preserves the old
// destination (including a read error after some bytes have been copied).
inline bool copyToFileAtomically(QIODevice& source, const QString& path, QString& error) {
  error.clear();
  QSaveFile output(path);
  if (!output.open(QIODevice::WriteOnly)) {
    error = output.errorString();
    return false;
  }
  std::array<char, 64 * 1024> buffer{};
  for (;;) {
    const qint64 count = source.read(buffer.data(), static_cast<qint64>(buffer.size()));
    if (count < 0) {
      error = source.errorString();
      output.cancelWriting();
      return false;
    }
    if (count == 0) {
      break;
    }
    qint64 written = 0;
    while (written < count) {
      const qint64 chunk = output.write(buffer.data() + written, count - written);
      if (chunk <= 0) {
        error = output.errorString();
        output.cancelWriting();
        return false;
      }
      written += chunk;
    }
  }
  if (!output.commit()) {
    error = output.errorString();
    return false;
  }
  return true;
}

} // namespace mitcad
