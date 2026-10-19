// SPDX-License-Identifier: MIT
#include "LiveServer.hpp"

#include <algorithm>
#include <exception>
#include <stdexcept>
#include <vector>

#include <QCoreApplication>
#include <QHostAddress>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPointer>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTimer>
#include <QUuid>

#include "MainWindow.hpp"
#include "LiveErrors.hpp"
#include "LocalEndpoint.hpp"
#include "LogRing.hpp"
#include "mitcad_bridge/lib.h"

namespace mitcad::mcp {
namespace {

QString randomId() {
  return QUuid::createUuid().toString(QUuid::Id128);
}

QByteArray protocolError(int code, const QString& message) {
  return QJsonDocument(QJsonObject{{QStringLiteral("jsonrpc"), QStringLiteral("2.0")},
                                  {QStringLiteral("error"), QJsonObject{{QStringLiteral("code"), code},
                                                                      {QStringLiteral("message"), message}}}})
             .toJson(QJsonDocument::Compact) + '\n';
}

} // namespace

struct LiveServer::State {
  struct Client {
    QPointer<QTcpSocket> socket;
    QByteArray input;
    bool authenticated = false;
    // Initialized an MCP session: an agent is connected (mitcad#265).
    bool initialized = false;
    rust::Box<LiveMcpSession> session = new_live_mcp_session();
  };

  LiveServer* owner;
  QPointer<MainWindow> window;
  QTcpServer listener;
  Endpoint endpoint;
  bool published = false;
  bool handling = false;
  std::vector<std::shared_ptr<Client>> clients;

  State(LiveServer* object, MainWindow* documentWindow) : owner(object), window(documentWindow) {}

  // The window shows whether agents are connected and can disconnect them
  // (mitcad#265).
  void updateAgents() {
    int count = 0;
    for (const auto& client : clients) {
      if (client->initialized && client->socket && client->socket->state() == QAbstractSocket::ConnectedState) {
        ++count;
      }
    }
    if (window) {
      window->setMcpAgents(count, [this] { disconnectAll(); });
    }
  }

  void disconnectAll() {
    for (const auto& client : clients) {
      if (client->initialized && client->socket) {
        client->socket->abort();
      }
    }
    updateAgents();
  }

  void write(Client& client, const QByteArray& response) {
    if (client.socket && client.socket->state() == QAbstractSocket::ConnectedState) {
      // The Rust dispatcher bounds each response to kMaxFrameBytes.
      // This separate queue budget protects against a client that stops reading.
      if (client.socket->bytesToWrite() + response.size() > 4 * (kMaxFrameBytes + 1)) {
        client.socket->abort();
      } else {
        client.socket->write(response);
      }
    }
  }

  void frame(Client& client, const QByteArray& message) {
    if (!client.authenticated) {
      const auto object = QJsonDocument::fromJson(message).object();
      const bool accepted = object.size() == 1 && object.value(QStringLiteral("mitcadAuth")).toString() == endpoint.token;
      write(client, accepted ? QByteArray("{\"authenticated\":true}\n") : QByteArray("{\"authenticated\":false}\n"));
      client.authenticated = accepted;
      if (!accepted && client.socket) {
        client.socket->disconnectFromHost();
      }
      return;
    }
    if (QString::fromUtf8(message).toUtf8() != message) {
      write(client, protocolError(-32700, QStringLiteral("Invalid UTF-8 JSON message")));
      return;
    }
    try {
      const std::string planText(client.session->prepare_mcp(rust::Str(message.constData(), size_t(message.size()))));
      const auto plan = QJsonDocument::fromJson(QByteArray::fromStdString(planText)).object();
      if (plan.contains(QStringLiteral("response"))) {
        const auto response = plan.value(QStringLiteral("response"));
        if (!response.isNull()) {
          // The Rust dispatcher serializes IDs losslessly, including u64.
          write(client, response.toString().toUtf8() + '\n');
          if (!client.initialized &&
              QJsonDocument::fromJson(message).object().value(QStringLiteral("method")).toString() ==
                  QStringLiteral("initialize") &&
              QJsonDocument::fromJson(response.toString().toUtf8()).object().contains(QStringLiteral("result"))) {
            client.initialized = true;
            updateAgents();
          }
        }
        return;
      }
      QJsonObject value;
      bool error = false;
      try {
        if (!window) {
          throw std::runtime_error("The application document window has closed");
        }
        const QString tool = plan.value(QStringLiteral("tool")).toString();
        const auto arguments = plan.value(QStringLiteral("arguments")).toObject();
        // The live catalog (mitcad#265): MainWindowMcp.cpp, MainWindowMcpUi.cpp.
        // An applied model command is an error only when a feature it
        // added or edited fails; finish_mcp decides (mitcad#401).
        value = window->mcpTool(tool, arguments);
      } catch (const Refusal& refusal) {
        // Busy (when to retry), a panel's state.
        error = true;
        value = refusal.fields();
        value.insert(QStringLiteral("error"), QString::fromUtf8(refusal.what()));
      } catch (const std::exception& failure) {
        error = true;
        value = {{QStringLiteral("error"), QString::fromUtf8(failure.what())}};
      }
      const QByteArray result = QJsonDocument(value).toJson(QJsonDocument::Compact);
      const std::string response(client.session->finish_mcp(rust::Str(result.constData(), size_t(result.size())), error));
      write(client, QByteArray::fromStdString(response) + '\n');
    } catch (const std::exception& failure) {
      write(client, protocolError(-32603, QString::fromUtf8(failure.what())));
    }
  }

  void drain(const std::shared_ptr<Client>& client) {
    if (!client->socket || client->socket->state() != QAbstractSocket::ConnectedState ||
        (handling && client->authenticated)) {
      return;
    }
    const bool alreadyHandling = handling;
    handling = true;
    int processed = 0;
    while (processed < 32 && client->socket && client->socket->state() == QAbstractSocket::ConnectedState) {
      // Discovery can authenticate a new connection while another model
      // command's progress loop runs. Its tools wait until that call ends.
      if (alreadyHandling && client->authenticated) {
        break;
      }
      const qsizetype limit = client->authenticated ? kMaxFrameBytes : 4096;
      const qsizetype newline = client->input.indexOf('\n');
      if (newline >= 0) {
        if (newline > limit) {
          write(*client, protocolError(-32600, QStringLiteral("Message exceeds the input limit")));
          client->socket->disconnectFromHost();
          break;
        }
        QByteArray message = client->input.left(newline);
        client->input.remove(0, newline + 1);
        frame(*client, message);
        ++processed;
        continue;
      }
      if (client->input.size() > limit) {
        write(*client, protocolError(-32600, QStringLiteral("Message exceeds the input limit")));
        client->socket->disconnectFromHost();
        break;
      }
      if (!client->socket->bytesAvailable()) {
        break;
      }
      client->input += client->socket->read(limit + 1 - client->input.size());
    }
    handling = alreadyHandling;
    if (alreadyHandling) {
      return;
    }
    // Model commands run Qt's progress loop. Other clients' readyRead
    // callbacks can arrive inside it; consume them only once it finishes.
    QTimer::singleShot(0, owner, [this] {
      const auto waiting = clients;
      for (const auto& other : waiting) {
        if (other->socket && (other->socket->bytesAvailable() || other->input.contains('\n') ||
                              other->input.size() > (other->authenticated ? kMaxFrameBytes : 4096))) {
          drain(other);
        }
      }
    });
  }

  void accept() {
    while (listener.hasPendingConnections()) {
      auto* socket = listener.nextPendingConnection();
      clients.erase(std::remove_if(clients.begin(), clients.end(), [](const auto& client) { return !client->socket; }),
                    clients.end());
      if (clients.size() >= 8) {
        socket->abort();
        socket->deleteLater();
        continue;
      }
      socket->setParent(owner);
      socket->setReadBufferSize(kMaxFrameBytes + 1);
      auto client = std::make_shared<Client>();
      client->socket = socket;
      clients.push_back(client);
      QObject::connect(socket, &QTcpSocket::readyRead, owner, [this, client] { drain(client); });
      QObject::connect(socket, &QTcpSocket::disconnected, owner, [this, socket] {
        socket->deleteLater();
        // Once the socket's QPointer has cleared.
        QTimer::singleShot(0, owner, [this] { updateAgents(); });
      });
      QTimer::singleShot(5000, socket, [client] {
        if (!client->authenticated && client->socket) {
          client->socket->abort();
        }
      });
    }
  }
};

LiveServer::LiveServer(MainWindow* window, QObject* parent)
    : QObject(parent), m_state(std::make_unique<State>(this, window)) {}

LiveServer::~LiveServer() {
  m_state->listener.close();
  if (m_state->window) {
    m_state->window->setMcpAgents(0, {});
  }
  for (const auto& client : m_state->clients) {
    if (client->socket) {
      client->socket->disconnect(this);
      client->socket->abort();
    }
  }
  if (m_state->published) {
    removeEndpoint(m_state->endpoint);
  }
}

bool LiveServer::start(QString* error) {
  // ui_log's lines (mitcad#265): main() installs the ring for the whole run
  // (mcp/LogRing.hpp); this keeps it for a server started without it.
  installLogRing();
  if (!prepareDiscoveryDirectory(error)) {
    return false;
  }
  // Removes the records of killed instances (mitcad#437) before a client
  // has to skip them.
  discover();
  if (!m_state->listener.listen(QHostAddress::LocalHost, 0)) {
    if (error) {
      *error = m_state->listener.errorString();
    }
    return false;
  }
  auto& endpoint = m_state->endpoint;
  endpoint.instance = randomId();
  endpoint.token = randomId() + randomId();
  endpoint.pid = QCoreApplication::applicationPid();
  endpoint.port = m_state->listener.serverPort();
  if (!publishEndpoint(endpoint, error)) {
    m_state->listener.close();
    return false;
  }
  m_state->published = true;
  connect(&m_state->listener, &QTcpServer::newConnection, this, [this] { m_state->accept(); });
  qInfo().noquote() << "MCP live instance:" << endpoint.instance << "127.0.0.1:" << endpoint.port;
  return true;
}

} // namespace mitcad::mcp
