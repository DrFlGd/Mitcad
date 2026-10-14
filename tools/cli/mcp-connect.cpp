// SPDX-License-Identifier: MIT
#include "mcp-connect.hpp"

#include <condition_variable>
#include <cstdio>
#include <deque>
#include <iostream>
#include <memory>
#include <mutex>
#include <thread>

#include <QCoreApplication>
#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTcpSocket>
#include <QTimer>

#ifdef Q_OS_WIN
#include <qt_windows.h>
#else
#include <cerrno>
#include <unistd.h>
#endif

#include "mcp/LocalEndpoint.hpp"

namespace mitcad::cli {
namespace {

using mcp::kMaxFrameBytes;
constexpr int kConnectTimeoutMs = 5000;

int connectionError(const QString& message) {
  std::cerr << "mitcad-cli mcp: " << message.toStdString() << '\n';
  return 1;
}

bool authenticate(QTcpSocket& socket, const mcp::Endpoint& endpoint, int timeoutMs) {
  socket.setReadBufferSize(kMaxFrameBytes + 1);
  socket.connectToHost(QStringLiteral("127.0.0.1"), endpoint.port);
  if (!socket.waitForConnected(timeoutMs)) {
    return false;
  }
  const QByteArray authentication = mcp::encodeAuth(endpoint);
  if (socket.write(authentication) != authentication.size()) {
    return false;
  }
  QElapsedTimer waiting;
  waiting.start();
  while (!socket.canReadLine()) {
    const qint64 remaining = timeoutMs - waiting.elapsed();
    if (remaining <= 0 || !socket.waitForReadyRead(static_cast<int>(remaining)) ||
        socket.bytesAvailable() > kMaxFrameBytes) {
      return false;
    }
  }
  const QJsonDocument answer = QJsonDocument::fromJson(socket.readLine(kMaxFrameBytes + 1));
  return answer.isObject() && answer.object().value(QStringLiteral("authenticated")) == QJsonValue(true);
}

QList<mcp::Endpoint> liveEndpoints(QString* error, int timeoutMs = 500) {
  const QList<mcp::Endpoint> records = mcp::discover(error);
  QList<mcp::Endpoint> endpoints;
  for (const mcp::Endpoint& record : records) {
    QTcpSocket probe;
    // Private records can survive a crash. Only the instance owning the
    // record's token may authenticate it; a reused port is not sufficient.
    if (authenticate(probe, record, timeoutMs)) {
      endpoints.append(record);
    }
  }
  return endpoints;
}

struct Input {
  std::mutex mutex;
  std::condition_variable room;
  std::deque<QByteArray> frames;
  qsizetype bytes = 0;
  bool ended = false;
  bool stop = false;
  QString error;
};

// The reader never touches Qt objects on the event-loop thread. Its queue
// and one incomplete frame are bounded, including when the peer is slow.
qint64 readStdin(char* bytes, std::size_t size) {
#ifdef Q_OS_WIN
  DWORD count = 0;
  if (ReadFile(GetStdHandle(STD_INPUT_HANDLE), bytes, static_cast<DWORD>(size), &count, nullptr)) {
    return count;
  }
  const DWORD error = GetLastError();
  return error == ERROR_BROKEN_PIPE || error == ERROR_HANDLE_EOF ? 0 : -1;
#else
  ssize_t count;
  do {
    count = ::read(STDIN_FILENO, bytes, size);
  } while (count < 0 && errno == EINTR);
  return count;
#endif
}

void readInput(const std::shared_ptr<Input>& input) {
  QByteArray frame;
  // A client's writer may start its stream with a UTF-8 byte order mark
  // (Windows PowerShell's process input with a UTF-8 console code page,
  // mitcad#265): not part of the first message, so it is dropped there.
  bool first = true;
  const auto publish = [&] {
    if (first) {
      first = false;
      if (frame.startsWith(QByteArrayLiteral("\xEF\xBB\xBF"))) {
        frame.remove(0, 3);
      }
    }
    frame.append('\n');
    std::unique_lock<std::mutex> lock(input->mutex);
    input->room.wait(lock, [&] { return input->stop || input->bytes + frame.size() <= kMaxFrameBytes + 1; });
    if (input->stop) {
      return false;
    }
    input->bytes += frame.size();
    input->frames.push_back(std::move(frame));
    frame.clear();
    return true;
  };
  char bytes[4096];
  for (;;) {
    const qint64 count = readStdin(bytes, sizeof(bytes));
    if (count <= 0) {
      if (count == 0 && !frame.isEmpty()) {
        publish();
      }
      std::lock_guard<std::mutex> lock(input->mutex);
      if (count < 0) {
        input->error = QStringLiteral("Could not read MCP stdin");
      }
      input->ended = true;
      return;
    }
    for (qint64 index = 0; index < count; ++index) {
      if (bytes[index] == '\n') {
        if (publish()) {
          continue;
        }
        std::lock_guard<std::mutex> lock(input->mutex);
        input->ended = true;
        return;
      }
      if (frame.size() == kMaxFrameBytes) {
        std::lock_guard<std::mutex> lock(input->mutex);
        input->error = QStringLiteral("Message exceeds the 8 MiB input limit");
        input->ended = true;
        return;
      }
      frame.append(bytes[index]);
    }
  }
}

bool expectsResponse(const QByteArray& frame) {
  if (QString::fromUtf8(frame).toUtf8() != frame) {
    return true;
  }
  const QJsonDocument message = QJsonDocument::fromJson(frame);
  if (!message.isObject()) {
    return true;
  }
  const QJsonObject object = message.object();
  if (object.value(QStringLiteral("jsonrpc")) != QStringLiteral("2.0")) {
    return true;
  }
  if (!object.contains(QStringLiteral("method")) &&
      (object.contains(QStringLiteral("result")) || object.contains(QStringLiteral("error")))) {
    // Client replies are valid protocol envelopes, not another request.
    return false;
  }
  return object.contains(QStringLiteral("id")) || !object.value(QStringLiteral("method")).isString();
}

} // namespace

int mcpListInstances() {
  int argc = 1;
  char name[] = "mitcad-cli";
  char* argv[] = {name, nullptr};
  QCoreApplication app(argc, argv);
  QString error;
  const QList<mcp::Endpoint> endpoints = liveEndpoints(&error);
  if (!error.isEmpty()) {
    return connectionError(error);
  }
  QJsonArray instances;
  for (const mcp::Endpoint& endpoint : endpoints) {
    instances.append(QJsonObject{{QStringLiteral("instance"), endpoint.instance},
                                 {QStringLiteral("pid"), endpoint.pid},
                                 {QStringLiteral("host"), QStringLiteral("127.0.0.1")},
                                 {QStringLiteral("port"), endpoint.port}});
  }
  const QByteArray output = QJsonDocument(QJsonObject{{QStringLiteral("instances"), instances}})
                                .toJson(QJsonDocument::Compact);
  std::cout << output.constData() << '\n';
  return 0;
}

int mcpConnect(const std::string& instance) {
  int argc = 1;
  char name[] = "mitcad-cli";
  char* argv[] = {name, nullptr};
  QCoreApplication app(argc, argv);
  QString error;
  // Connecting allows the GUI to finish a busy event-loop turn; a quick
  // listing probe must not classify that live instance as absent.
  const QList<mcp::Endpoint> endpoints = liveEndpoints(&error, kConnectTimeoutMs);
  if (!error.isEmpty()) {
    return connectionError(error);
  }
  const QString requested = QString::fromUtf8(instance.data(), static_cast<qsizetype>(instance.size()));
  mcp::Endpoint endpoint;
  if (requested.isEmpty()) {
    if (endpoints.isEmpty()) {
      return connectionError(QStringLiteral("No running Mitcad instance; open the desktop application first"));
    }
    if (endpoints.size() != 1) {
      return connectionError(QStringLiteral("Several Mitcad instances are running; use --list-instances and --instance ID"));
    }
    endpoint = endpoints.front();
  } else {
    bool found = false;
    for (const mcp::Endpoint& candidate : endpoints) {
      if (candidate.instance == requested) {
        endpoint = candidate;
        found = true;
        break;
      }
    }
    if (!found) {
      return connectionError(QStringLiteral("No running Mitcad instance '%1'").arg(requested));
    }
  }

  QTcpSocket socket;
  if (!authenticate(socket, endpoint, kConnectTimeoutMs)) {
    return connectionError(QStringLiteral("Mitcad refused authentication; rediscover its running instances"));
  }

  auto input = std::make_shared<Input>();
  std::thread reader(readInput, input);
  int result = 0;
  int pending = 0;
  QByteArray response;
  const auto fail = [&](const QString& message) {
    if (result == 0) {
      result = connectionError(message);
    }
    app.quit();
  };
  QObject::connect(&socket, &QTcpSocket::readyRead, &app, [&] {
    while (socket.bytesAvailable() > 0) {
      const QByteArray bytes = socket.read(qMin<qint64>(socket.bytesAvailable(), kMaxFrameBytes + 1 - response.size()));
      if (bytes.isEmpty()) {
        fail(QStringLiteral("Message exceeds the 8 MiB response limit"));
        return;
      }
      response.append(bytes);
      for (;;) {
        const qsizetype newline = response.indexOf('\n');
        if (newline < 0) {
          break;
        }
        const QByteArray line = response.left(newline + 1);
        response.remove(0, newline + 1);
        const QJsonDocument message = QJsonDocument::fromJson(line);
        if (!message.isObject() || message.object().value(QStringLiteral("jsonrpc")) != QStringLiteral("2.0")) {
          fail(QStringLiteral("Mitcad sent an invalid MCP message"));
          return;
        }
        const QJsonObject envelope = message.object();
        if (!envelope.contains(QStringLiteral("method")) &&
            (envelope.contains(QStringLiteral("result")) || envelope.contains(QStringLiteral("error"))) && pending > 0) {
          --pending;
        }
        if (std::fwrite(line.constData(), 1, static_cast<std::size_t>(line.size()), stdout) !=
                static_cast<std::size_t>(line.size()) || std::fflush(stdout) != 0) {
          fail(QStringLiteral("Could not write MCP stdout"));
          return;
        }
      }
      if (response.size() > kMaxFrameBytes) {
        fail(QStringLiteral("Message exceeds the 8 MiB response limit"));
        return;
      }
    }
  });
  QObject::connect(&socket, &QTcpSocket::disconnected, &app, [&] {
    fail(QStringLiteral("The Mitcad instance disconnected"));
  });
  QTimer pump;
  QObject::connect(&pump, &QTimer::timeout, &app, [&] {
    std::unique_lock<std::mutex> lock(input->mutex);
    if (!input->error.isEmpty()) {
      fail(input->error);
      return;
    }
    while (!input->frames.empty() && socket.bytesToWrite() < kMaxFrameBytes) {
      const QByteArray frame = std::move(input->frames.front());
      input->frames.pop_front();
      input->bytes -= frame.size();
      input->room.notify_one();
      if (expectsResponse(frame)) {
        ++pending;
      }
      if (socket.write(frame) != frame.size()) {
        fail(QStringLiteral("Could not send an MCP message to Mitcad"));
        return;
      }
    }
    if (input->ended && input->frames.empty() && socket.bytesToWrite() == 0 && pending == 0) {
      // EOF belongs to this bridge only, not to the desktop application's
      // document or lifetime. All requested responses have been forwarded.
      app.quit();
    }
  });
  pump.start(5);
  app.exec();
  QObject::disconnect(&socket, nullptr, &app, nullptr);
  socket.abort();
  {
    std::lock_guard<std::mutex> lock(input->mutex);
    input->stop = true;
    input->room.notify_one();
    if (input->ended) {
      reader.join();
    } else {
      // Raw OS reads hold no stdio FILE lock during process-exit cleanup.
      // A disconnected GUI must not wait for a client holding stdin open;
      // only shared input state survives until this CLI process exits.
      reader.detach();
    }
  }
  return result;
}

} // namespace mitcad::cli
