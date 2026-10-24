// SPDX-License-Identifier: MIT
#pragma once
#ifdef __APPLE__
#include <memory>
#include <QColor>
#include "render/MacFrameSurface.hpp"
#include "render/RenderImage.hpp"

class OpenGl_Context;
namespace mitcad::render {
// Composes the shared half-float planes in the viewport's current GL context.
class MacFrameDisplay {
public:
  MacFrameDisplay();
  ~MacFrameDisplay();
  void setFrame(std::shared_ptr<MacFrameLease> frame, const FrameInfo& info,
                const QColor& top, const QColor& bottom, const Look& look);
  void draw();
  void release(OpenGl_Context* context);
private:
  struct Impl;
  std::unique_ptr<Impl> m_impl;
};
} // namespace mitcad::render
#endif
