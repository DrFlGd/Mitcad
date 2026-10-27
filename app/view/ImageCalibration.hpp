// SPDX-License-Identifier: MIT
#pragma once

#include <functional>
#include <optional>
#include <utility>
#include <vector>

#include <QObject>
#include <QPointF>
#include <QPointer>
#include <QString>
#include <QStringList>

#include "ImageReferences.hpp"

class QEvent;

namespace mitcad {

class ManipulatorOverlay;
class OcctViewer;

// Calibrating an image reference (mitcad#782): two clicks in the view, the
// points where the clicks' rays meet the image's plane. Of several images
// the first click takes the one it lands on (the nearest along the ray);
// one image alone takes a click anywhere on its plane. After the first
// point a cross marks it and a line runs from it to the pointer. While it
// runs the left button is its own (the view picks nothing, the pointer is
// the precision cursor) and Esc cancels it; the middle and right buttons
// still navigate. The callbacks come after the event that caused them, so
// they may delete it.
class ImageCalibration : public QObject {
public:
  struct Callbacks {
    std::function<void(const QString& image, const ImagePoint& first)> first;
    std::function<void(const QString& image, const ImagePoint& first, const ImagePoint& second)> picked;
    std::function<void()> cancelled;
  };

  ImageCalibration(OcctViewer* viewer, std::vector<ImageReferenceShown> images, Callbacks callbacks);
  ~ImageCalibration() override;

  // The images a first click may take; after it, the one it took.
  const std::vector<ImageReferenceShown>& images() const { return m_images; }
  QStringList names() const;
  // Whether the first point was clicked.
  bool hasFirst() const { return m_first.has_value(); }

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  // The point of an image's plane under a place in the view (logical
  // pixels) and its distance from the eye; none when the view looks along
  // the plane.
  std::optional<std::pair<ImagePoint, double>> pointAt(const ImageReferenceShown& image,
                                                       const QPointF& position) const;
  void clicked(const QPointF& position);
  // The cross at the first point and the line to the pointer.
  void drawBand(const QPointF& pointer);

  QPointer<OcctViewer> m_viewer; // null once the view is gone
  std::vector<ImageReferenceShown> m_images;
  Callbacks m_callbacks;
  std::optional<ImagePoint> m_first;
  std::optional<QPointF> m_pressed;
  ManipulatorOverlay* m_band = nullptr;
  bool m_done = false;
};

} // namespace mitcad
