// SPDX-License-Identifier: MIT
// Image references (mitcad#782): the model's entries as the view draws
// them, points on an image and on its plane, and the log line UI tests
// read. Plain Qt Core.
#include "ImageReferences.hpp"

#include <cmath>
#include <cstddef>

#include <QJsonValue>
#include <QLocale>
#include <QStringList>

namespace mitcad {
namespace {

ImagePoint minus(const ImagePoint& a, const ImagePoint& b) { return {a[0] - b[0], a[1] - b[1], a[2] - b[2]}; }

double dot(const ImagePoint& a, const ImagePoint& b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }

ImagePoint cross(const ImagePoint& a, const ImagePoint& b) {
  return {a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]};
}

double length(const ImagePoint& a) { return std::sqrt(dot(a, a)); }

} // namespace

std::optional<ImageReferenceShown> imageReferenceOf(const QJsonObject& entry) {
  const QJsonArray corners = entry.value(QStringLiteral("corners")).toArray();
  if (entry.contains(QStringLiteral("error")) || corners.size() != 4) {
    return std::nullopt;
  }
  ImageReferenceShown image;
  image.name = entry.value(QStringLiteral("name")).toString();
  image.sha256 = entry.value(QStringLiteral("sha256")).toString();
  for (int i = 0; i < 4; ++i) {
    const QJsonArray point = corners.at(i).toArray();
    if (point.size() != 3) {
      return std::nullopt;
    }
    for (int k = 0; k < 3; ++k) {
      image.corners[static_cast<std::size_t>(i)][static_cast<std::size_t>(k)] = point.at(k).toDouble();
    }
  }
  image.opacity = entry.value(QStringLiteral("opacity")).toDouble(1.0);
  image.through = entry.value(QStringLiteral("through")).toBool();
  image.visible = entry.value(QStringLiteral("visible")).toBool(true);
  const QJsonArray pixels = entry.value(QStringLiteral("pixels")).toArray();
  image.width = pixels.at(0).toInt();
  image.height = pixels.at(1).toInt();
  return image;
}

std::vector<ImageReferenceShown> shownImageReferences(const QJsonArray& entries) {
  std::vector<ImageReferenceShown> shown;
  for (const QJsonValue& value : entries) {
    if (const std::optional<ImageReferenceShown> image = imageReferenceOf(value.toObject());
        image && image->visible) {
      shown.push_back(*image);
    }
  }
  return shown;
}

ImagePoint imagePointAt(const ImageReferenceShown& image, double across, double down) {
  const ImagePoint& topLeft = image.corners[0];
  const ImagePoint right = minus(image.corners[1], topLeft);
  const ImagePoint below = minus(image.corners[3], topLeft);
  return {topLeft[0] + right[0] * across + below[0] * down, topLeft[1] + right[1] * across + below[1] * down,
          topLeft[2] + right[2] * across + below[2] * down};
}

std::array<double, 2> imageFractions(const ImageReferenceShown& image, const ImagePoint& point) {
  const ImagePoint offset = minus(point, image.corners[0]);
  const ImagePoint right = minus(image.corners[1], image.corners[0]);
  const ImagePoint below = minus(image.corners[3], image.corners[0]);
  const double across = dot(right, right);
  const double down = dot(below, below);
  return {across > 0.0 ? dot(offset, right) / across : 0.0, down > 0.0 ? dot(offset, below) / down : 0.0};
}

std::optional<ImagePoint> imagePlaneHit(const ImageReferenceShown& image, const ImagePoint& from,
                                        const ImagePoint& direction) {
  const ImagePoint& origin = image.corners[0];
  const ImagePoint normal = cross(minus(image.corners[1], origin), minus(image.corners[3], origin));
  const double along = dot(direction, normal);
  if (std::abs(along) < 1e-12 * length(direction) * length(normal) || length(normal) == 0.0) {
    return std::nullopt;
  }
  const double t = dot(minus(origin, from), normal) / along;
  return ImagePoint{from[0] + direction[0] * t, from[1] + direction[1] * t, from[2] + direction[2] * t};
}

std::array<double, 2> imageSize(const ImageReferenceShown& image) {
  return {length(minus(image.corners[1], image.corners[0])), length(minus(image.corners[3], image.corners[0]))};
}

QString imageReferencesLog(const std::vector<ImageReferenceShown>& shown) {
  if (shown.empty()) {
    return QStringLiteral("Image references drawn: none");
  }
  QStringList parts;
  for (const ImageReferenceShown& image : shown) {
    const std::array<double, 2> size = imageSize(image);
    parts << QStringLiteral("%1 %2 x %3 mm %4% %5")
                 .arg(image.name, QString::number(size[0], 'f', 1), QString::number(size[1], 'f', 1))
                 .arg(std::lround(image.opacity * 100.0))
                 .arg(image.through ? QStringLiteral("through") : QStringLiteral("behind"));
  }
  return QStringLiteral("Image references drawn: ") + parts.join(QStringLiteral(", "));
}

QString calibrationExpression(const QString& typed, const QString& unit) {
  const QString text = typed.trimmed();
  bool number = false;
  static_cast<void>(QLocale::c().toDouble(text, &number));
  return number ? QStringLiteral("%1 %2").arg(text, unit) : text;
}

} // namespace mitcad
