// SPDX-License-Identifier: MIT
#include "render/MacCommandPayload.hpp"

#include <QDir>
#include <algorithm>
#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <new>
#include <sys/stat.h>
#include <unistd.h>
#include <utility>

namespace mitcad::render {
namespace {
struct Descriptor {
  int value = -1;
  explicit Descriptor(int fd = -1) : value(fd) {}
  ~Descriptor() { if (value >= 0) close(value); }
  int release() { return std::exchange(value, -1); }
};
struct TemporaryDirectory {
  QByteArray path;
  Descriptor directory;
  ~TemporaryDirectory() {
    if (directory.value >= 0) unlinkat(directory.value, "payload", 0);
    if (!path.isEmpty()) rmdir(path.constData());
  }
};
std::string systemError(const char* operation) {
  return std::string(operation) + ": " + std::strerror(errno);
}
bool validLength(std::uint64_t length, std::uint64_t maximum) {
  return length > 0 && length <= std::min(maximum, kMacCommandPayloadMaxBytes);
}
bool validFile(int fd, std::uint64_t length, std::string& error) {
  struct stat status {};
  if (fstat(fd, &status) != 0) { error = systemError("payload fstat"); return false; }
  const int flags = fcntl(fd, F_GETFL);
  if (flags < 0) { error = systemError("payload fcntl"); return false; }
  if (!S_ISREG(status.st_mode) || status.st_uid != geteuid() || status.st_nlink != 0 ||
      (status.st_mode & 07777) != 0600 || (flags & O_ACCMODE) != O_RDONLY ||
      status.st_size <= 0 || std::uint64_t(status.st_size) != length) {
    error = "the command payload is not an owned unlinked read-only file of its declared size";
    return false;
  }
  return true;
}
} // namespace

MacCommandPayload::~MacCommandPayload() { reset(); }
MacCommandPayload::MacCommandPayload(MacCommandPayload&& other) noexcept
    : m_descriptor(std::exchange(other.m_descriptor, -1)), m_size(std::exchange(other.m_size, 0)) {}
MacCommandPayload& MacCommandPayload::operator=(MacCommandPayload&& other) noexcept {
  if (this != &other) {
    reset();
    m_descriptor = std::exchange(other.m_descriptor, -1);
    m_size = std::exchange(other.m_size, 0);
  }
  return *this;
}
void MacCommandPayload::reset() {
  if (m_descriptor >= 0) close(m_descriptor);
  m_descriptor = -1;
  m_size = 0;
}

bool MacCommandPayload::create(const QByteArray& json, std::string& error, std::uint64_t maximum) {
  reset();
  error.clear();
  const auto length = std::uint64_t(json.size());
  if (!validLength(length, maximum)) { error = "the command payload exceeds its bounded JSON capacity"; return false; }
  TemporaryDirectory temporary;
  QByteArray pattern = QDir::tempPath().toUtf8() + "/mitcad-command-XXXXXX";
  if (!mkdtemp(pattern.data())) { error = systemError("payload mkdtemp"); return false; }
  temporary.path = pattern;
  temporary.directory.value = open(pattern.constData(), O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW);
  if (temporary.directory.value < 0) { error = systemError("payload directory open"); return false; }
  Descriptor writer(openat(temporary.directory.value, "payload", O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600));
  if (writer.value < 0) { error = systemError("payload create"); return false; }
  if (fchmod(writer.value, 0600) != 0) { error = systemError("payload permissions"); return false; }
  std::size_t written = 0;
  while (written < length) {
    const auto count = write(writer.value, json.constData() + written, std::size_t(length) - written);
    if (count < 0 && errno == EINTR) continue;
    if (count <= 0) { error = count < 0 ? systemError("payload write") : "short command payload write"; return false; }
    written += std::size_t(count);
  }
  Descriptor reader(openat(temporary.directory.value, "payload", O_RDONLY | O_CLOEXEC | O_NOFOLLOW));
  if (reader.value < 0) { error = systemError("payload read-only reopen"); return false; }
  if (unlinkat(temporary.directory.value, "payload", 0) != 0) { error = systemError("payload unlink"); return false; }
  // Close the only writer before publishing even the read-only descriptor.
  close(writer.release());
  if (!validFile(reader.value, length, error)) return false;
  m_descriptor = reader.release();
  m_size = length;
  return true;
}

bool MacCommandPayload::readOwned(int descriptor, std::uint64_t declaredSize, QByteArray& json,
                                  std::string& error, std::uint64_t maximum) {
  Descriptor input(descriptor);
  json.clear();
  error.clear();
  if (!validLength(declaredSize, maximum)) { error = "the command payload exceeds its bounded JSON capacity"; return false; }
  if (!validFile(input.value, declaredSize, error)) return false;
  if (fcntl(input.value, F_SETFD, FD_CLOEXEC) != 0) { error = systemError("payload close-on-exec"); return false; }
  QByteArray bytes;
  try { bytes.resize(qsizetype(declaredSize)); }
  catch (const std::bad_alloc&) { error = "not enough memory for the command payload"; return false; }
  std::size_t received = 0;
  while (received < declaredSize) {
    const auto count = pread(input.value, bytes.data() + received, std::size_t(declaredSize) - received, off_t(received));
    if (count < 0 && errno == EINTR) continue;
    if (count <= 0) { error = count < 0 ? systemError("payload read") : "truncated command payload"; return false; }
    received += std::size_t(count);
  }
  if (!validFile(input.value, declaredSize, error)) return false;
  json = std::move(bytes);
  return true;
}
} // namespace mitcad::render
