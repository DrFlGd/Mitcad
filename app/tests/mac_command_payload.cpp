// SPDX-License-Identifier: MIT
#include "render/MacCommandPayload.hpp"
#include "render/MacRenderProtocol.hpp"

#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTemporaryFile>
#include <cerrno>
#include <cstdio>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#include <utility>

namespace {
using namespace mitcad::render;
bool check(bool value, const char* stage) {
  if (!value) std::fprintf(stderr, "FAIL: %s\n", stage);
  return value;
}
bool closed(int fd) { errno = 0; return fcntl(fd, F_GETFD) == -1 && errno == EBADF; }

bool rejected(int fd, std::uint64_t size, std::uint64_t maximum = kMacCommandPayloadMaxBytes) {
  QByteArray output("old output must be cleared");
  std::string error;
  const bool accepted = MacCommandPayload::readOwned(fd, size, output, error, maximum);
  return !accepted && !error.empty() && output.isEmpty() && closed(fd);
}

bool invalidFiles() {
  QTemporaryFile file;
  if (!check(file.open() && file.write("{}", 2) == 2 && file.flush(), "temporary validation fixture")) return false;
  const auto path = file.fileName().toUtf8();
  if (!check(rejected(open(path.constData(), O_RDONLY | O_CLOEXEC), 2), "linked payload rejected")) return false;
  const int writable = dup(file.handle());
  const int readonly = open(path.constData(), O_RDONLY | O_CLOEXEC);
  const int permissions = open(path.constData(), O_RDONLY | O_CLOEXEC);
  if (!check(writable >= 0 && readonly >= 0 && permissions >= 0 && file.remove(), "unlinked validation fixture")) return false;
  if (!check(rejected(writable, 2), "writable descriptor rejected")) return false;
  if (!check(rejected(readonly, 3), "declared and actual sizes must match")) return false;
  // QTemporaryFile::remove closes the original handle, but duplicated handles
  // still own the anonymous inode and can validate its mode safely.
  if (!check(fchmod(permissions, 0640) == 0 && rejected(permissions, 2), "non-private file mode rejected")) return false;
  int pipefd[2] = {-1, -1};
  if (!check(pipe(pipefd) == 0, "pipe fixture")) return false;
  close(pipefd[1]);
  if (!check(rejected(pipefd[0], 2), "non-regular descriptor rejected")) return false;
  return check(rejected(-1, 2), "invalid descriptor rejected");
}

bool bounds() {
  MacCommandPayload payload;
  std::string error;
  if (!check(!payload.create(QByteArray(), error) && payload.descriptor() == -1, "empty payload rejected")) return false;
  if (!check(!payload.create(QByteArray("{}"), error, 1) && payload.descriptor() == -1, "sender byte cap")) return false;
  if (!check(payload.create(QByteArray("{}"), error), "bounded fixture creation")) return false;
  if (!check(rejected(dup(payload.descriptor()), payload.size(), 1), "receiver cap before allocation")) return false;
  if (!check(rejected(dup(payload.descriptor()), 0), "zero declared size rejected")) return false;
  if (!check(rejected(dup(payload.descriptor()), kMacCommandPayloadMaxBytes + 1), "unrepresentable JSON size rejected")) return false;
  MacCommandPayload moved(std::move(payload));
  if (!check(payload.descriptor() == -1 && payload.size() == 0 && moved.descriptor() >= 0, "move transfers ownership")) return false;
  const int descriptor = moved.descriptor();
  moved.reset();
  return check(closed(descriptor) && moved.size() == 0, "reset closes its descriptor");
}

bool largeScene() {
  // An ordinary valid scene command with real-sized mesh content keys in its
  // release list, rather than a large ignored JSON padding property. The bulk
  // transport must not inherit the control message's 16 MiB limit.
  constexpr int keys = 270000;
  QByteArray input("{\"cmd\":\"scene\",\"ground\":false,\"release\":[");
  input.reserve(keys * 67 + 64);
  for (int index = 0; index < keys; ++index) {
    if (index) input.append(',');
    input.append('"');
    input.append(QByteArray::number(index, 16).rightJustified(64, '0'));
    input.append('"');
  }
  input.append("]}");
  if (!check(std::uint64_t(input.size()) > kMacRenderMessageBytes, "scene exceeds inline XPC limit")) return false;
  MacCommandPayload payload;
  std::string error;
  if (!check(payload.create(input, error), "large anonymous payload creation")) return false;
  struct stat status {};
  if (!check(fstat(payload.descriptor(), &status) == 0 && status.st_nlink == 0 &&
             (status.st_mode & 07777) == 0600 && status.st_uid == geteuid(), "owned private unlinked payload")) return false;
  errno = 0;
  if (!check(write(payload.descriptor(), "x", 1) == -1 && errno == EBADF, "sender exposes only a read-only descriptor")) return false;
  const int receiver = dup(payload.descriptor());
  const auto size = payload.size();
  payload.reset(); // The receiver alone keeps the already unlinked file alive.
  QByteArray output;
  if (!check(MacCommandPayload::readOwned(receiver, size, output, error) && closed(receiver), "large payload receive and close")) return false;
  if (!check(output == input, "bulk bytes are exact")) return false;
  const auto document = QJsonDocument::fromJson(output);
  return check(document.isObject() && document.object().value(QStringLiteral("cmd")).toString() == QLatin1String("scene") &&
               document.object().value(QStringLiteral("release")).toArray().size() == keys,
               "greater-than-16-MiB payload remains a valid complete scene command");
}
} // namespace

int main(int argc, char** argv) {
  QCoreApplication application(argc, argv);
  if (!invalidFiles() || !bounds() || !largeScene()) return 1;
  std::puts("owned bulk command payload: descriptor validation, bounds and >16-MiB scene passed");
  return 0;
}
