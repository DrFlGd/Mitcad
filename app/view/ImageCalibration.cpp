// SPDX-License-Identifier: MIT
// Two points clicked on an image reference's plane (mitcad#782).
#include "ImageCalibration.hpp"

#include <array>
#include <cmath>
#include <cstddef>
#include <utility>

#include <QCoreApplication>
#include <QEvent>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QTimer>

#include <Graphic3d_Camera.hxx>
#include <gp_Pnt.hxx>

#include "../OcctViewer.hpp"
#include "../framework/ManipulatorOverlay.hpp"

namespace mitcad {
namespace {

// A press and release this close (logical pixels) are a click.
constexpr double kClickSlop = 5.0;
// Half the first point's cross (logical pixels).
constexpr double kCross = 6.0;
// How far outside its edges a first click still takes an image (a
// fraction of its size).
constexpr double kMargin = 0.02;

gp_Pnt occt(const ImagePoint& point) { return gp_Pnt(point[0], point[1], point[2]); }

} // namespace

ImageCalibration::ImageCalibration(OcctViewer* viewer, std::vector<ImageReferenceShown> images, Callbacks callbacks)
    : QObject(viewer), m_viewer(viewer), m_images(std::move(images)), m_callbacks(std::move(callbacks)) {
  m_viewer->setSketchInput(true);
  m_viewer->installEventFilter(this);
  QCoreApplication::instance()->installEventFilter(this);
}

ImageCalibration::~ImageCalibration() {
  QCoreApplication::instance()->removeEventFilter(this);
  if (m_viewer) {
    delete m_band; // a child of the view
    m_viewer->removeEventFilter(this);
    m_viewer->setSketchInput(false);
  }
}

QStringList ImageCalibration::names() const {
  QStringList names;
  for (const ImageReferenceShown& image : m_images) {
    names << image.name;
  }
  return names;
}

std::optional<std::pair<ImagePoint, double>> ImageCalibration::pointAt(const ImageReferenceShown& image,
                                                                       const QPointF& position) const {
  if (!m_viewer || m_viewer->width() <= 0 || m_viewer->height() <= 0) {
    return std::nullopt;
  }
  // As a sketch's points: the camera's ray through the place.
  const double x = 2.0 * position.x() / m_viewer->width() - 1.0;
  const double y = 1.0 - 2.0 * position.y() / m_viewer->height();
  const auto& camera = m_viewer->viewCamera();
  const gp_Pnt from = camera->UnProject(gp_Pnt(x, y, -1.0));
  const gp_Pnt to = camera->UnProject(gp_Pnt(x, y, 1.0));
  const std::optional<ImagePoint> hit = imagePlaneHit(image, {from.X(), from.Y(), from.Z()},
                                                      {to.X() - from.X(), to.Y() - from.Y(), to.Z() - from.Z()});
  if (!hit) {
    return std::nullopt;
  }
  return std::make_pair(*hit, from.Distance(occt(*hit)));
}

void ImageCalibration::clicked(const QPointF& position) {
  if (m_done) {
    return;
  }
  if (!m_first) {
    // The image the click lands on, the nearest one; one image alone takes
    // any point of its plane.
    std::optional<std::size_t> taken;
    std::optional<ImagePoint> point;
    double nearest = 0.0;
    for (std::size_t i = 0; i < m_images.size(); ++i) {
      const auto hit = pointAt(m_images[i], position);
      if (!hit) {
        continue;
      }
      const std::array<double, 2> at = imageFractions(m_images[i], hit->first);
      const bool on = at[0] >= -kMargin && at[0] <= 1.0 + kMargin && at[1] >= -kMargin && at[1] <= 1.0 + kMargin;
      if ((on || m_images.size() == 1) && (!taken || hit->second < nearest)) {
        taken = i;
        point = hit->first;
        nearest = hit->second;
      }
    }
    if (!taken) {
      return; // off the images: the hint stays
    }
    m_images = {m_images[*taken]};
    m_first = point;
    drawBand(position);
    if (m_callbacks.first) {
      const QString name = m_images.front().name;
      const ImagePoint first = *point;
      QTimer::singleShot(0, this, [this, name, first] { m_callbacks.first(name, first); });
    }
    return;
  }
  const auto hit = pointAt(m_images.front(), position);
  if (!hit) {
    return;
  }
  m_done = true;
  const QString name = m_images.front().name;
  const ImagePoint first = *m_first;
  const ImagePoint second = hit->first;
  QTimer::singleShot(0, this, [this, name, first, second] {
    if (m_callbacks.picked) {
      m_callbacks.picked(name, first, second);
    }
  });
}

void ImageCalibration::drawBand(const QPointF& pointer) {
  if (!m_first || !m_viewer) {
    return;
  }
  if (m_band == nullptr) {
    m_band = new ManipulatorOverlay(*m_viewer, QStringLiteral("Calibrate"));
  }
  const ImageReferenceShown& image = m_images.front();
  // The cross: four points of the plane a few pixels from the first.
  const QPointF centre = m_viewer->toWidgetF(occt(*m_first));
  std::vector<std::vector<gp_Pnt>> lines;
  for (const auto& [from, to] : {std::pair{QPointF(-kCross, -kCross), QPointF(kCross, kCross)},
                                 std::pair{QPointF(-kCross, kCross), QPointF(kCross, -kCross)}}) {
    const auto a = pointAt(image, centre + from);
    const auto b = pointAt(image, centre + to);
    if (a && b) {
      lines.push_back({occt(a->first), occt(b->first)});
    }
  }
  if (const auto end = pointAt(image, pointer)) {
    lines.push_back({occt(*m_first), occt(end->first)});
  }
  m_band->setLines(std::move(lines));
}

bool ImageCalibration::eventFilter(QObject* watched, QEvent* event) {
  // Esc is the calibration's, before any shortcut takes it.
  const bool key = event->type() == QEvent::KeyPress || event->type() == QEvent::ShortcutOverride;
  if (key && static_cast<QKeyEvent*>(event)->key() == Qt::Key_Escape) {
    if (event->type() == QEvent::ShortcutOverride) {
      event->accept();
      return true;
    }
    if (!m_done) {
      m_done = true;
      QTimer::singleShot(0, this, [this] {
        if (m_callbacks.cancelled) {
          m_callbacks.cancelled();
        }
      });
    }
    return true;
  }
  if (watched != m_viewer.data()) {
    return false;
  }
  if (event->type() == QEvent::MouseMove) {
    if (m_first && !m_done) {
      drawBand(static_cast<QMouseEvent*>(event)->position());
    }
    return false; // the view still follows the pointer (navigation)
  }
  if (event->type() == QEvent::MouseButtonPress) {
    const auto* mouse = static_cast<QMouseEvent*>(event);
    if (mouse->button() == Qt::LeftButton) {
      m_pressed = mouse->position();
      return true;
    }
  } else if (event->type() == QEvent::MouseButtonRelease) {
    const auto* mouse = static_cast<QMouseEvent*>(event);
    if (mouse->button() == Qt::LeftButton) {
      const std::optional<QPointF> pressed = std::exchange(m_pressed, std::nullopt);
      if (pressed && (mouse->position() - *pressed).manhattanLength() <= kClickSlop) {
        clicked(mouse->position());
      }
      return true;
    }
  }
  return false;
}

} // namespace mitcad
