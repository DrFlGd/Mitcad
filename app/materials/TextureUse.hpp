// SPDX-License-Identifier: MIT
#pragma once

// Use in Edit Appearances (mitcad#315): a texture source's maps as a new
// appearance of the design, its images embedded (create_appearance with
// each map's `data`, mitcad#313), so the design never needs the source or
// its cache to open.

#include <QColor>
#include <QJsonArray>
#include <QJsonObject>
#include <QSet>
#include <QString>

#include "TextureSource.hpp"

namespace mitcad {

struct TextureUse {
  QJsonObject command; // create_appearance; empty with an error
  int sent = 0;        // images sent in the command
  int shared = 0;      // images the design embeds already, referred to
  QString error;       // a sentence for the user
};

// The largest image the design file embeds (core/model's
// MAX_EMBEDDED_IMAGE).
constexpr qint64 kMaxEmbeddedImage = qint64(32) << 20;

// The create_appearance command for an asset's files: named `name`, its
// base colour the base colour image's average (what the shaded view
// shows), and a texture whose images are embedded: each map `{"path"
// (the file's name), "data"}`, or `{"path", "embedded": true,
// "image_sha256"}` for an image whose digest is in `embedded` (one the
// design has already); its size the asset's, else 100 mm wide and as high
// as the image's proportions; its `source` the source's id, the asset's
// recorded id, the resolution, the licence and the web page where known.
// An image that is no PNG or JPEG or larger than `maxBytes` is an error.
TextureUse textureUseCommand(const QString& sourceId, const TextureAsset& asset, const TextureFiles& files,
                             const QString& name, const QSet<QString>& embedded,
                             qint64 maxBytes = kMaxEmbeddedImage);

// The digests of the images a design embeds: the `appearances` query's
// textures and their maps listed with `embedded` and `image_sha256`.
QSet<QString> embeddedImageDigests(const QJsonArray& appearances);

// The average colour of an image file (sRGB); invalid when it cannot be
// read.
QColor averageImageColor(const QString& file);

} // namespace mitcad
