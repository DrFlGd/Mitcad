// SPDX-License-Identifier: MIT
#pragma once

#include <QJsonObject>
#include <functional>
#include <memory>
#include "render/Renderer.hpp"

namespace mitcad::render {
QJsonObject renderDeviceJson(const DeviceInfo& device);
// One interactive renderer, independent of process stdio and other documents.
// Commands are serialized by its transport; FrameSink events may originate on
// the renderer thread. The sink and callback outlive this session.
class RenderSession {
public:
  using Event = std::function<void(const QJsonObject&)>;
  RenderSession(FrameSink& sink, Event event, const QJsonObject& configuration = {});
  ~RenderSession();
  void hello();
  bool command(const QJsonObject& command);
private:
  struct Impl;
  std::unique_ptr<Impl> m_impl;
};
} // namespace mitcad::render
