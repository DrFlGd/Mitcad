// SPDX-License-Identifier: MIT
#pragma once

#include <QByteArray>
#include <cstdint>
#include <limits>
#include <string>

namespace mitcad::render {
// Matches the existing renderer's int-sized JSON byte count. Control messages
// keep their much smaller independent transport budget.
constexpr std::uint64_t kMacCommandPayloadMaxBytes = std::numeric_limits<int>::max();

// A completed, unlinked, read-only payload. Only its descriptor and exact byte
// count cross XPC; no peer is asked to open a filesystem path.
class MacCommandPayload {
public:
  MacCommandPayload() = default;
  ~MacCommandPayload();
  MacCommandPayload(const MacCommandPayload&) = delete;
  MacCommandPayload& operator=(const MacCommandPayload&) = delete;
  MacCommandPayload(MacCommandPayload&& other) noexcept;
  MacCommandPayload& operator=(MacCommandPayload&& other) noexcept;

  bool create(const QByteArray& json, std::string& error,
              std::uint64_t maximum = kMacCommandPayloadMaxBytes);
  int descriptor() const { return m_descriptor; }
  std::uint64_t size() const { return m_size; }
  void reset();

  // Consumes the descriptor, including on malformed input. Read into owned
  // storage so concurrent truncation cannot fault a shared mmap with SIGBUS.
  // Call on the service's serial command queue, under its existing watchdog.
  static bool readOwned(int descriptor, std::uint64_t declaredSize, QByteArray& json,
                        std::string& error, std::uint64_t maximum = kMacCommandPayloadMaxBytes);

private:
  int m_descriptor = -1;
  std::uint64_t m_size = 0;
};
} // namespace mitcad::render
