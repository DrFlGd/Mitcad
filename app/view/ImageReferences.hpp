// SPDX-License-Identifier: MIT
#pragma once

#include <array>
#include <optional>
#include <vector>

#include <QJsonArray>
#include <QJsonObject>
#include <QString>

namespace mitcad {

using ImagePoint = std::array<double, 3>;

// An image reference as the view draws it (mitcad#782): an entry of the
// model's `image_references` query whose plane is there at the marker.
struct ImageReferenceShown {
  QString name;
  QString sha256;
  // Top left, top right, bottom right, bottom left, in the design.
  std::array<ImagePoint, 4> corners{};
  double opacity = 1.0;
  bool through = false; // drawn over the bodies
  bool visible = true;
  int width = 0; // pixels
  int height = 0;
};

// One entry of the query; none when its plane is gone (`error`) or it has
// no four corners.
std::optional<ImageReferenceShown> imageReferenceOf(const QJsonObject& entry);
// The entries the view draws, in order: the shown ones with their planes.
std::vector<ImageReferenceShown> shownImageReferences(const QJsonArray& entries);
// The point of the image `across` its width from its left edge and `down`
// its height from its top edge (fractions; 0.5, 0.5 is its middle).
ImagePoint imagePointAt(const ImageReferenceShown& image, double across, double down);
// Where a point of the image's plane is across the image from its left
// edge and down from its top edge, as fractions (0 to 1 on the image).
std::array<double, 2> imageFractions(const ImageReferenceShown& image, const ImagePoint& point);
// Where a ray from `from` along `direction` meets the image's plane; none
// when it runs along the plane.
std::optional<ImagePoint> imagePlaneHit(const ImageReferenceShown& image, const ImagePoint& from,
                                        const ImagePoint& direction);
// The image's width and height in the design (mm).
std::array<double, 2> imageSize(const ImageReferenceShown& image);
// For UI tests, when it changes: "Image references drawn: Image1 50.0 x
// 25.0 mm 50% behind, Plan 20.0 x 10.0 mm 30% through", or "Image
// references drawn: none".
QString imageReferencesLog(const std::vector<ImageReferenceShown>& shown);
// A calibration's distance as the user typed it: a bare number takes the
// document's length unit ("40" in mm is "40 mm"), anything else stays as
// it is for the model to evaluate.
QString calibrationExpression(const QString& typed, const QString& unit);

} // namespace mitcad
