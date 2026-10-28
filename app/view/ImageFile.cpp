// SPDX-License-Identifier: MIT
// Image references' files decoded (view/ImageFile.hpp).
#include "view/ImageFile.hpp"

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <memory>

#include <QImageReader>

#if defined(MITCAD_HEIF)
#include <libheif/heif.h>
#endif

namespace mitcad {
namespace {

#if defined(MITCAD_HEIF)

QString heifText(const heif_error& result) {
  return result.message != nullptr ? QString::fromUtf8(result.message) : QStringLiteral("unknown error");
}

// The primary image as it is shown: libheif applies the file's irot, imir
// and clap unless told not to, and joins a grid's tiles.
QImage decodeHeif(const QByteArray& bytes, QString* error) {
  // Once per process (libheif counts its users; the process' end frees it).
  static const heif_error started = heif_init(nullptr);
  if (started.code != heif_error_Ok) {
    *error = QStringLiteral("the HEIF decoder did not start: %1").arg(heifText(started));
    return {};
  }
  const std::unique_ptr<heif_context, void (*)(heif_context*)> context(heif_context_alloc(), heif_context_free);
  if (!context) {
    *error = QStringLiteral("the HEIF decoder has no memory");
    return {};
  }
  heif_error result = heif_context_read_from_memory_without_copy(context.get(), bytes.constData(),
                                                                 static_cast<std::size_t>(bytes.size()), nullptr);
  if (result.code != heif_error_Ok) {
    *error = QStringLiteral("the HEIF file cannot be read: %1").arg(heifText(result));
    return {};
  }
  heif_image_handle* handle = nullptr;
  result = heif_context_get_primary_image_handle(context.get(), &handle);
  const std::unique_ptr<heif_image_handle, void (*)(const heif_image_handle*)> primary(handle,
                                                                                       heif_image_handle_release);
  if (result.code != heif_error_Ok || !primary) {
    *error = QStringLiteral("the HEIF file has no primary image: %1").arg(heifText(result));
    return {};
  }
  const int width = heif_image_handle_get_width(handle);
  const int height = heif_image_handle_get_height(handle);
  if (width <= 0 || height <= 0) {
    *error = QStringLiteral("the HEIF image has no pixels");
    return {};
  }
  // The limit Qt's own readers keep to (QImageReader::allocationLimit, MB).
  const int limit = QImageReader::allocationLimit();
  if (limit > 0 && static_cast<qint64>(width) * height * 4 > (static_cast<qint64>(limit) << 20)) {
    *error = QStringLiteral("the HEIF image of %1 x %2 pixels needs more than the %3 MB an image may take")
                 .arg(width)
                 .arg(height)
                 .arg(limit);
    return {};
  }
  const bool alpha = heif_image_handle_has_alpha_channel(handle) != 0;
  const int channels = alpha ? 4 : 3;
  heif_image* decoded = nullptr;
  result = heif_decode_image(handle, &decoded, heif_colorspace_RGB,
                             alpha ? heif_chroma_interleaved_RGBA : heif_chroma_interleaved_RGB, nullptr);
  const std::unique_ptr<heif_image, void (*)(const heif_image*)> picture(decoded, heif_image_release);
  if (result.code != heif_error_Ok || !picture) {
    *error = QStringLiteral("the HEIF image cannot be decoded: %1").arg(heifText(result));
    return {};
  }
  const int bits = heif_image_get_bits_per_pixel_range(decoded, heif_channel_interleaved);
  if (bits != 8) {
    *error = QStringLiteral("the HEIF image was decoded with %1 bits per channel, not 8").arg(bits);
    return {};
  }
  int stride = 0;
  const std::uint8_t* plane = heif_image_get_plane_readonly(decoded, heif_channel_interleaved, &stride);
  const int columns = heif_image_get_width(decoded, heif_channel_interleaved);
  const int rows = heif_image_get_height(decoded, heif_channel_interleaved);
  const std::size_t row = static_cast<std::size_t>(columns) * static_cast<std::size_t>(channels);
  if (plane == nullptr || columns <= 0 || rows <= 0 || stride < 0 || static_cast<std::size_t>(stride) < row) {
    *error = QStringLiteral("the HEIF image was decoded without its pixels");
    return {};
  }
  QImage image(columns, rows, alpha ? QImage::Format_RGBA8888 : QImage::Format_RGB888);
  if (image.isNull()) {
    *error = QStringLiteral("there is no memory for the image's %1 x %2 pixels").arg(columns).arg(rows);
    return {};
  }
  for (int y = 0; y < rows; ++y) {
    std::memcpy(image.scanLine(y), plane + static_cast<std::ptrdiff_t>(y) * stride, row);
  }
  return image;
}

#else

QImage decodeHeif(const QByteArray&, QString* error) {
  *error = QStringLiteral("this build of MitCad has no HEIF decoder");
  return {};
}

#endif

} // namespace

bool isHeifFile(const QByteArray& bytes) {
  if (bytes.size() < 16 || bytes.mid(4, 4) != QByteArrayLiteral("ftyp")) {
    return false;
  }
  const auto byte = [&bytes](int at) { return static_cast<quint32>(static_cast<unsigned char>(bytes.at(at))); };
  const quint32 size = (byte(0) << 24) | (byte(1) << 16) | (byte(2) << 8) | byte(3);
  const qsizetype end = qMin(bytes.size(), static_cast<qsizetype>(size));
  static const char* const kBrands[] = {"mif1", "mif2", "msf1", "miaf", "heic", "heix",
                                        "heim", "heis", "hevc", "hevx", "avif", "avis"};
  // The major brand, the minor version (skipped), the compatible brands.
  for (qsizetype at = 8; at + 4 <= end; at += 4) {
    if (at == 12) {
      continue;
    }
    const QByteArray brand = bytes.mid(at, 4);
    for (const char* known : kBrands) {
      if (brand == known) {
        return true;
      }
    }
  }
  return false;
}

bool heifDecoderBuilt() {
#if defined(MITCAD_HEIF)
  return true;
#else
  return false;
#endif
}

QImage decodeImageFile(const QByteArray& bytes, QString* error) {
  QString why;
  QImage image;
  if (isHeifFile(bytes)) {
    image = decodeHeif(bytes, &why);
  } else {
    image = QImage::fromData(bytes);
    if (image.isNull()) {
      why = QStringLiteral("Qt cannot read the image");
    }
  }
  if (error != nullptr) {
    *error = image.isNull() ? why : QString();
  }
  return image;
}

} // namespace mitcad
