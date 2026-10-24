// SPDX-License-Identifier: MIT
#pragma once
#ifdef __APPLE__
#include <QObject>
#include <QJsonObject>
#include <QString>
#include <functional>
#include <memory>

namespace mitcad::render {
struct Frame;
class MacRenderClient : public QObject {
public:
  using Event = std::function<void(const QJsonObject&)>;
  using Failure = std::function<void(const QString&)>;
  explicit MacRenderClient(QObject* parent);
  ~MacRenderClient() override;
  bool start(Event event, Failure failure);
  void stop();
  bool isRunning() const;
  qint64 processId() const;
  void reserve(int width, int height);
  void send(const QJsonObject& command);
  bool takeFrame(Frame& frame);
private:
  struct Impl;
  std::unique_ptr<Impl> m_impl;
};
} // namespace mitcad::render
#endif
