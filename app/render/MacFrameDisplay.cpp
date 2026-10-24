// SPDX-License-Identifier: MIT
#ifdef __APPLE__
// Keep OCCT's OpenGL declarations before Qt's, as in OcctViewer.cpp.
#include <OpenGl_Context.hxx>
#include <OpenGl_GlCore30.hxx>
#include <OpenGl_Resource.hxx>
#include <OpenGL/OpenGL.h>
#include <QOpenGLContext>
#include <QOpenGLFunctions_4_1_Core>
#include <QOpenGLVersionFunctionsFactory>
#include <QOpenGLShaderProgram>
#include <QtLogging>
#include <cmath>
#include "render/MacFrameDisplay.hpp"
#include "framework/Diagnostics.hpp"

namespace mitcad::render {
namespace {
constexpr const char* kVertex = R"GLSL(#version 410 core
out vec2 uv;
void main() {
  vec2 p = vec2(float(gl_VertexID & 1), float((gl_VertexID >> 1) & 1));
  uv = p;
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
)GLSL";
constexpr const char* kFragment = R"GLSL(#version 410 core
uniform sampler2DRect lightPlane;
uniform sampler2DRect catcherPlane;
uniform ivec2 frameSize;
uniform int surfaceWidth;
uniform int hasCatcher;
uniform int transformKind;
uniform float exposure;
uniform vec3 topColor;
uniform vec3 bottomColor;
in vec2 uv;
out vec4 color;
vec4 finiteHalf(vec4 value) {
  return vec4(isnan(value.x) || isinf(value.x) ? 0.0 : value.x,
              isnan(value.y) || isinf(value.y) ? 0.0 : value.y,
              isnan(value.z) || isinf(value.z) ? 0.0 : value.z,
              isnan(value.w) || isinf(value.w) ? 0.0 : value.w);
}
vec4 source(sampler2DRect plane, ivec2 p) {
  int index = p.y * frameSize.x + p.x;
  return finiteHalf(texelFetch(plane, ivec2(index % surfaceWidth, index / surfaceWidth)));
}
vec3 hable(vec3 v) {
  return (v * (0.15 * v + 0.05) + 0.004) / (v * (0.15 * v + 0.5) + 0.06) - 0.02 / 0.3;
}
vec3 curve(vec3 light) {
  if (transformKind == 1) {
    return min(hable(max(light, vec3(0.0)) * 3.2) / hable(vec3(20.0)), vec3(1.0));
  }
  vec3 rgb = max(light, vec3(0.0));
  float lowest = min(rgb.r, min(rgb.g, rgb.b));
  float offset = lowest < 0.08 ? lowest - 6.25 * lowest * lowest : 0.04;
  rgb -= offset;
  float peak = max(rgb.r, max(rgb.g, rgb.b));
  if (peak < 0.76) { return rgb; }
  float newPeak = 1.0 - 0.24 * 0.24 / (peak + 0.24 - 0.76);
  float g = 1.0 - 1.0 / (0.15 * (peak - newPeak) + 1.0);
  return rgb * newPeak / peak * (1.0 - g) + vec3(newPeak * g);
}
vec3 encode(vec3 linear) {
  // Match displayImage's 4096-step linear lookup and 8-bit quantisation.
  vec3 v = floor(clamp(linear, 0.0, 1.0) * 4096.0 + 0.5) / 4096.0;
  vec3 srgb = mix(1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055, v * 12.92,
                   lessThanEqual(v, vec3(0.0031308)));
  return floor(clamp(srgb, 0.0, 1.0) * 255.0 + 0.5) / 255.0;
}
vec3 decode(vec3 v) {
  return mix(pow((v + 0.055) / 1.055, vec3(2.4)), v / 12.92,
             lessThanEqual(v, vec3(0.04045)));
}
vec3 srgb(vec3 v) {
  return mix(1.055 * pow(max(v, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055, v * 12.92,
             lessThanEqual(v, vec3(0.0031308)));
}
vec3 pixel(ivec2 p) {
  vec4 raw = source(lightPlane, p);
  float alpha = clamp(raw.a, 0.0, 1.0);
  vec3 light = raw.rgb * exp2(exposure);
  if (transformKind != 0 && alpha > 0.0001) { light = curve(light / alpha) * alpha; }
  vec3 factor = hasCatcher != 0 ? max(source(catcherPlane, p).rgb, vec3(0.0)) : vec3(1.0);
  float along = frameSize.y > 1 ? float(frameSize.y - 1 - p.y) / float(frameSize.y - 1) : 0.0;
  // OCCT's existing image is an sRGB texture: hardware bilinear filtering
  // interpolates its decoded linear values, after the image's 8-bit rounding.
  return decode(encode(light + (1.0 - alpha) * factor * mix(topColor, bottomColor, along)));
}
void main() {
  vec2 p = uv * vec2(frameSize) - 0.5;
  ivec2 first = ivec2(floor(p));
  // The existing Graphic3d_Texture2D repeats at the rectangle's boundary.
  ivec2 a = (first + frameSize) % frameSize;
  ivec2 b = (first + 1 + frameSize) % frameSize;
  vec2 f = fract(p);
  color = vec4(srgb(mix(mix(pixel(a), pixel(ivec2(b.x, a.y)), f.x),
                        mix(pixel(ivec2(a.x, b.y)), pixel(b), f.x), f.y)), 1.0);
}
)GLSL";
class SurfaceGpuResources final : public OpenGl_Resource {
public:
  SurfaceGpuResources(GLuint first, GLuint second, GLuint array) : textures{first, second}, vao(array) {}
  ~SurfaceGpuResources() override = default;
  std::size_t EstimatedDataSize() const override { return 0; }
  void Release(OpenGl_Context* context) override {
    if (context != nullptr && context->core30 != nullptr) {
      context->core30->glDeleteTextures(2, textures);
      context->core30->glDeleteVertexArrays(1, &vao);
    }
    textures[0] = textures[1] = vao = 0;
  }
private:
  GLuint textures[2];
  GLuint vao;
};
float linear(float value) {
  return value <= 0.04045f ? value / 12.92f : std::pow((value + 0.055f) / 1.055f, 2.4f);
}
} // namespace

struct MacFrameDisplay::Impl {
  std::shared_ptr<MacFrameLease> frame;
  FrameInfo info;
  QColor top;
  QColor bottom;
  Look look;
  std::unique_ptr<QOpenGLShaderProgram> program = std::make_unique<QOpenGLShaderProgram>();
  GLuint textures[2]{};
  GLuint vao = 0;
  IOSurfaceID bound[2]{};
  bool failed = false;
};
MacFrameDisplay::MacFrameDisplay() : m_impl(std::make_unique<Impl>()) {}
MacFrameDisplay::~MacFrameDisplay() = default;
void MacFrameDisplay::setFrame(std::shared_ptr<MacFrameLease> frame, const FrameInfo& info,
                               const QColor& top, const QColor& bottom, const Look& look) {
  m_impl->frame = std::move(frame);
  m_impl->info = info;
  m_impl->top = top;
  m_impl->bottom = bottom;
  m_impl->look = look;
}
void MacFrameDisplay::draw() {
  auto& state = *m_impl;
  if (!state.frame || state.failed) { return; }
  ScopedTiming timing("render surface draw");
  if (timingLogged()) {
    timing.setDetail(QStringLiteral("view %1 samples %2 %3x%4 denoised %5")
                         .arg(state.info.view).arg(state.info.samples).arg(state.info.width)
                         .arg(state.info.height).arg(state.info.denoised));
  }
  auto* gl = QOpenGLVersionFunctionsFactory::get<QOpenGLFunctions_4_1_Core>(QOpenGLContext::currentContext());
  if (gl == nullptr || !gl->initializeOpenGLFunctions()) {
    qWarning("IOSurface display requires a current OpenGL 4.1 context");
    state.failed = true;
    return;
  }
  GLint program = 0, vao = 0, active = 0;
  GLint bindings[2]{}, samplers[2]{}, polygonMode[2]{};
  GLboolean depthMask = GL_TRUE, colorMask[4]{};
  gl->glGetIntegerv(GL_CURRENT_PROGRAM, &program);
  gl->glGetIntegerv(GL_VERTEX_ARRAY_BINDING, &vao);
  gl->glGetIntegerv(GL_ACTIVE_TEXTURE, &active);
  gl->glGetIntegerv(GL_POLYGON_MODE, polygonMode);
  gl->glGetBooleanv(GL_DEPTH_WRITEMASK, &depthMask);
  gl->glGetBooleanv(GL_COLOR_WRITEMASK, colorMask);
  const GLenum capabilities[] = {GL_DEPTH_TEST, GL_BLEND, GL_CULL_FACE, GL_SCISSOR_TEST, GL_STENCIL_TEST,
                                  GL_FRAMEBUFFER_SRGB};
  GLboolean enabled[6]{};
  for (int i = 0; i < 6; ++i) {
    enabled[i] = gl->glIsEnabled(capabilities[i]);
    gl->glDisable(capabilities[i]);
  }
  for (int i = 0; i < 2; ++i) {
    gl->glActiveTexture(GL_TEXTURE0 + static_cast<GLenum>(i));
    gl->glGetIntegerv(GL_TEXTURE_BINDING_RECTANGLE, &bindings[i]);
    gl->glGetIntegerv(GL_SAMPLER_BINDING, &samplers[i]);
    gl->glBindSampler(static_cast<GLuint>(i), 0);
  }
  if (state.vao == 0) {
    gl->glGenVertexArrays(1, &state.vao);
    gl->glGenTextures(2, state.textures);
    if (!state.program->addShaderFromSourceCode(QOpenGLShader::Vertex, kVertex) ||
        !state.program->addShaderFromSourceCode(QOpenGLShader::Fragment, kFragment) || !state.program->link()) {
      qWarning().noquote() << "IOSurface display shader:" << state.program->log();
      state.failed = true;
    }
  }
  for (int plane = 0; plane < 2 && !state.failed; ++plane) {
    gl->glActiveTexture(GL_TEXTURE0 + static_cast<GLenum>(plane));
    gl->glBindTexture(GL_TEXTURE_RECTANGLE, state.textures[plane]);
    const IOSurfaceRef surface = state.frame->surface(static_cast<std::uint32_t>(plane));
    if (state.bound[plane] != IOSurfaceGetID(surface)) {
      const CGLError error = CGLTexImageIOSurface2D(CGLGetCurrentContext(), GL_TEXTURE_RECTANGLE, GL_RGBA16F,
          static_cast<GLsizei>(IOSurfaceGetWidth(surface)), static_cast<GLsizei>(IOSurfaceGetHeight(surface)),
          GL_RGBA, GL_HALF_FLOAT, surface, 0);
      if (error != kCGLNoError) {
        qWarning().noquote() << "IOSurface texture binding failed:" << CGLErrorString(error);
        state.failed = true;
        break;
      }
      state.bound[plane] = IOSurfaceGetID(surface);
      gl->glTexParameteri(GL_TEXTURE_RECTANGLE, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
      gl->glTexParameteri(GL_TEXTURE_RECTANGLE, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
      gl->glTexParameteri(GL_TEXTURE_RECTANGLE, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
      gl->glTexParameteri(GL_TEXTURE_RECTANGLE, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    }
  }
  if (!state.failed) {
    state.program->bind();
    state.program->setUniformValue("lightPlane", 0);
    state.program->setUniformValue("catcherPlane", 1);
    gl->glUniform2i(state.program->uniformLocation("frameSize"), static_cast<GLint>(state.info.width),
                    static_cast<GLint>(state.info.height));
    state.program->setUniformValue("surfaceWidth", static_cast<int>(IOSurfaceGetWidth(state.frame->surface())));
    state.program->setUniformValue("hasCatcher", state.info.catcher != 0 ? 1 : 0);
    state.program->setUniformValue("transformKind", static_cast<int>(state.look.transform));
    state.program->setUniformValue("exposure", state.look.exposure);
    state.program->setUniformValue("topColor", linear(state.top.redF()), linear(state.top.greenF()), linear(state.top.blueF()));
    state.program->setUniformValue("bottomColor", linear(state.bottom.redF()), linear(state.bottom.greenF()), linear(state.bottom.blueF()));
    gl->glBindVertexArray(state.vao);
    gl->glPolygonMode(GL_FRONT_AND_BACK, GL_FILL);
    gl->glDepthMask(GL_FALSE);
    gl->glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);
    gl->glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
    // The lease can be replaced immediately after this call. CPU publication
    // must not reuse its slot before all GPU sampling has completed.
    gl->glFinish();
  }
  gl->glUseProgram(static_cast<GLuint>(program));
  gl->glBindVertexArray(static_cast<GLuint>(vao));
  gl->glPolygonMode(GL_FRONT_AND_BACK, static_cast<GLenum>(polygonMode[0]));
  gl->glDepthMask(depthMask);
  gl->glColorMask(colorMask[0], colorMask[1], colorMask[2], colorMask[3]);
  for (int i = 0; i < 2; ++i) {
    gl->glActiveTexture(GL_TEXTURE0 + static_cast<GLenum>(i));
    gl->glBindTexture(GL_TEXTURE_RECTANGLE, static_cast<GLuint>(bindings[i]));
    gl->glBindSampler(static_cast<GLuint>(i), static_cast<GLuint>(samplers[i]));
  }
  gl->glActiveTexture(static_cast<GLenum>(active));
  for (int i = 0; i < 6; ++i) {
    if (enabled[i]) { gl->glEnable(capabilities[i]); }
  }
}
void MacFrameDisplay::release(OpenGl_Context* context) {
  if (m_impl->vao != 0) {
    occ::handle<SurfaceGpuResources> resources =
        new SurfaceGpuResources(m_impl->textures[0], m_impl->textures[1], m_impl->vao);
    if (context != nullptr && !context->IsCurrent()) {
      context->DelayedRelease(resources);
    } else if (context != nullptr) {
      resources->Release(context);
    } else {
      auto* gl = QOpenGLVersionFunctionsFactory::get<QOpenGLFunctions_4_1_Core>(QOpenGLContext::currentContext());
      if (gl != nullptr && gl->initializeOpenGLFunctions()) {
        gl->glDeleteTextures(2, m_impl->textures);
        gl->glDeleteVertexArrays(1, &m_impl->vao);
      }
    }
  }
  m_impl->textures[0] = m_impl->textures[1] = m_impl->vao = 0;
  m_impl->bound[0] = m_impl->bound[1] = 0;
  // A replacement context may belong to a different Qt sharing group.
  // Recreate the program owner as well as its OpenGL names on the next draw.
  m_impl->program = std::make_unique<QOpenGLShaderProgram>();
  m_impl->failed = false;
}
} // namespace mitcad::render
#endif
