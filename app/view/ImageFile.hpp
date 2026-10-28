// SPDX-License-Identifier: MIT
// The pixels of an image reference's file (mitcad#782, mitcad#850): PNG,
// JPEG, BMP and WebP through Qt's image readers; HEIF (.heic, .heif)
// through libheif and its HEVC decoder libde265 in builds with MITCAD_HEIF
// (shared libraries under the LGPL; docs/development.md, "Licence
// policy"). A HEIF image comes as it is shown: its rotation, mirroring and
// crop applied, which is the size core/model-base/src/image_header/heif.rs
// reads; 8 bits per channel (a 10-bit image reduced), with its alpha plane
// when it has one.
#pragma once

#include <QByteArray>
#include <QImage>
#include <QString>

namespace mitcad {

// Whether the bytes start as a HEIF file does: a `ftyp` box naming a brand
// of the HEIF family (the brands image_header/heif.rs takes).
bool isHeifFile(const QByteArray& bytes);

// Whether this build decodes HEIF files (MITCAD_HEIF).
bool heifDecoderBuilt();

// The image of a file's bytes, or a null image and why in *error.
QImage decodeImageFile(const QByteArray& bytes, QString* error = nullptr);

} // namespace mitcad
