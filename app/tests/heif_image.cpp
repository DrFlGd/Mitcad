// SPDX-License-Identifier: MIT
// HEIF images of image references (mitcad#850, view/ImageFile.hpp): HEIF
// files told from the other formats, and HEIF files the test makes decoded
// as they are shown (a quarter turn applied), with 8 and 10 bits per
// channel; a file cut short and other bytes refused with the reason.
#include "view/ImageFile.hpp"

#include <cstddef>
#include <cstdio>
#include <cstdlib>

#include <QBuffer>
#include <QByteArray>
#include <QImage>
#include <QString>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "heif_image.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

// The HEVC data of tools/heif-test-fixture.py, which made it from pixels of
// its own: a 64 x 32 picture of flat quarters (top left red, top right
// green, bottom left blue, bottom right white), full range, BT.601; per bit
// depth the hvcC property's content (with the parameter sets) and the image
// item's data (the slice, its length before it).
const unsigned char kHvcc8[] = {
    0x01, 0x01, 0x60, 0x00, 0x00, 0x00, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1e, 0xf0, 0x00, 0xfc,
    0xfd, 0xf8, 0xf8, 0x00, 0x00, 0x0f, 0x03, 0xa0, 0x00, 0x01, 0x00, 0x18, 0x40, 0x01, 0x0c, 0x01,
    0xff, 0xff, 0x01, 0x60, 0x00, 0x00, 0x03, 0x00, 0x90, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00,
    0x1e, 0x95, 0x98, 0x09, 0xa1, 0x00, 0x01, 0x00, 0x2b, 0x42, 0x01, 0x01, 0x01, 0x60, 0x00, 0x00,
    0x03, 0x00, 0x90, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x1e, 0xa0, 0x20, 0x82, 0x16, 0x59,
    0x59, 0xae, 0x4c, 0xae, 0x6e, 0x04, 0x04, 0x0c, 0x08, 0x00, 0x00, 0x03, 0x00, 0x08, 0x00, 0x00,
    0x03, 0x00, 0xc8, 0x40, 0xa2, 0x00, 0x01, 0x00, 0x06, 0x44, 0x01, 0xc1, 0x71, 0xa1, 0x12,
};
const unsigned char kData8[] = {
    0x00, 0x00, 0x00, 0x65, 0x28, 0x01, 0xaf, 0x04, 0x78, 0x3c, 0x86, 0x2f, 0xf4, 0x39, 0x48, 0xb3,
    0x20, 0x28, 0x0c, 0xb8, 0xe4, 0x65, 0xd1, 0xcc, 0x14, 0xb8, 0x94, 0x50, 0x58, 0x2c, 0x9f, 0xaf,
    0x6c, 0x9a, 0xac, 0x2c, 0xff, 0xb1, 0xec, 0x4c, 0x14, 0x9b, 0x6d, 0x7f, 0x70, 0x72, 0xa2, 0x83,
    0xfb, 0x63, 0xc8, 0x90, 0xba, 0x6c, 0xe9, 0xe9, 0x21, 0x69, 0x5a, 0x8f, 0xcb, 0xfd, 0x38, 0xcb,
    0x86, 0xdc, 0xc4, 0xab, 0x7b, 0xa9, 0x7d, 0xad, 0x65, 0x3b, 0x81, 0x52, 0x44, 0x78, 0x68, 0xfc,
    0x34, 0xb4, 0x87, 0x52, 0xef, 0xff, 0x86, 0xa8, 0x72, 0x83, 0x1b, 0x68, 0x7e, 0xf4, 0x80, 0x10,
    0x95, 0x93, 0x05, 0x8a, 0xbf, 0x6a, 0x3d, 0xba, 0x7e,
};
const unsigned char kHvcc10[] = {
    0x01, 0x02, 0x20, 0x00, 0x00, 0x00, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1e, 0xf0, 0x00, 0xfc,
    0xfd, 0xfa, 0xfa, 0x00, 0x00, 0x0f, 0x03, 0xa0, 0x00, 0x01, 0x00, 0x18, 0x40, 0x01, 0x0c, 0x01,
    0xff, 0xff, 0x02, 0x20, 0x00, 0x00, 0x03, 0x00, 0x90, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00,
    0x1e, 0x95, 0x98, 0x09, 0xa1, 0x00, 0x01, 0x00, 0x2a, 0x42, 0x01, 0x01, 0x02, 0x20, 0x00, 0x00,
    0x03, 0x00, 0x90, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x1e, 0xa0, 0x20, 0x82, 0x13, 0x65,
    0x95, 0x9a, 0xe4, 0xca, 0xe6, 0xe0, 0x40, 0x40, 0xc0, 0x80, 0x00, 0x00, 0x03, 0x00, 0x80, 0x00,
    0x00, 0x0c, 0x84, 0xa2, 0x00, 0x01, 0x00, 0x06, 0x44, 0x01, 0xc1, 0x71, 0xa1, 0x12,
};
const unsigned char kData10[] = {
    0x00, 0x00, 0x00, 0x60, 0x28, 0x01, 0xaf, 0x04, 0x78, 0x25, 0xe3, 0x17, 0xf3, 0xe3, 0x3f, 0xff,
    0x08, 0x7e, 0xcb, 0xdb, 0x86, 0x21, 0x57, 0x10, 0x21, 0xef, 0x38, 0x73, 0x1c, 0x47, 0xbc, 0x44,
    0x81, 0x36, 0xf3, 0x92, 0x3c, 0x7c, 0x67, 0x75, 0xd4, 0x82, 0x1f, 0xbd, 0xd7, 0x46, 0xb1, 0x38,
    0xf9, 0x65, 0x1f, 0xe7, 0xab, 0x8a, 0xc5, 0x3f, 0xd7, 0xe3, 0x54, 0x48, 0x07, 0xf3, 0x09, 0x73,
    0xf8, 0x05, 0xbf, 0xfa, 0xd6, 0x63, 0x5a, 0xff, 0xa3, 0x13, 0x50, 0x28, 0x8f, 0xfe, 0xaa, 0x94,
    0xba, 0x6f, 0xe7, 0x90, 0x7b, 0x4f, 0xf4, 0x08, 0xb7, 0x51, 0x6d, 0x4c, 0x7b, 0xab, 0x65, 0x09,
    0xf1, 0xdc, 0x4b, 0x90,
};

template <std::size_t N>
QByteArray bytesOf(const unsigned char (&data)[N]) {
  return QByteArray(reinterpret_cast<const char*>(data), static_cast<qsizetype>(N));
}

void put16(QByteArray& bytes, unsigned value) {
  bytes.append(static_cast<char>((value >> 8) & 0xffu));
  bytes.append(static_cast<char>(value & 0xffu));
}

void put32(QByteArray& bytes, unsigned value) {
  put16(bytes, (value >> 16) & 0xffffu);
  put16(bytes, value & 0xffffu);
}

QByteArray box(const char* kind, const QByteArray& body) {
  QByteArray bytes;
  put32(bytes, static_cast<unsigned>(8 + body.size()));
  bytes.append(kind, 4);
  bytes.append(body);
  return bytes;
}

// A full box: its version, no flags.
QByteArray fullBox(const char* kind, int version, const QByteArray& body) {
  QByteArray content(4, '\0');
  content[0] = static_cast<char>(version);
  return box(kind, content + body);
}

// A HEIF file of one HEVC image item, 64 x 32 pixels coded, turned `turns`
// quarter turns anticlockwise (an irot property; none for 0): the files
// tools/heif-test-fixture.py writes.
QByteArray heifFile(const QByteArray& hvcc, const QByteArray& data, int turns) {
  QByteArray brands("heic", 4);
  put32(brands, 0);
  brands.append("mif1heic", 8);
  const QByteArray ftyp = box("ftyp", brands);
  QByteArray ispe;
  put32(ispe, 64);
  put32(ispe, 32);
  // Full range BT.601, as coded.
  QByteArray colr("nclx", 4);
  put16(colr, 1);
  put16(colr, 13);
  put16(colr, 6);
  colr.append(static_cast<char>(0x80));
  QByteArray properties = box("hvcC", hvcc) + fullBox("ispe", 0, ispe) + box("colr", colr);
  // The essential bit, then the property's index from 1.
  QByteArray associations;
  associations.append(static_cast<char>(0x81));
  associations.append(static_cast<char>(0x02));
  associations.append(static_cast<char>(0x03));
  if (turns != 0) {
    properties += box("irot", QByteArray(1, static_cast<char>(turns)));
    associations.append(static_cast<char>(0x84));
  }
  const auto metaBox = [&](unsigned offset) {
    QByteArray hdlr(4, '\0');
    hdlr.append("pict", 4);
    hdlr.append(QByteArray(13, '\0'));
    QByteArray pitm;
    put16(pitm, 1);
    // Offsets and lengths of 4 bytes, no base offset: item 1, one extent.
    QByteArray iloc;
    iloc.append(static_cast<char>(0x44));
    iloc.append('\0');
    put16(iloc, 1);
    put16(iloc, 1);
    put16(iloc, 0);
    put16(iloc, 1);
    put32(iloc, offset);
    put32(iloc, static_cast<unsigned>(data.size()));
    QByteArray infe;
    put16(infe, 1);
    put16(infe, 0);
    infe.append("hvc1", 4);
    infe.append('\0');
    QByteArray iinf;
    put16(iinf, 1);
    iinf += fullBox("infe", 2, infe);
    QByteArray ipma;
    put32(ipma, 1);
    put16(ipma, 1);
    ipma.append(static_cast<char>(associations.size()));
    ipma += associations;
    const QByteArray iprp = box("iprp", box("ipco", properties) + fullBox("ipma", 0, ipma));
    return fullBox("meta", 0,
                   fullBox("hdlr", 0, hdlr) + fullBox("pitm", 0, pitm) + fullBox("iloc", 0, iloc) +
                       fullBox("iinf", 0, iinf) + iprp);
  };
  const unsigned offset = static_cast<unsigned>(ftyp.size() + metaBox(0).size() + 8);
  return ftyp + metaBox(offset) + box("mdat", data);
}

// Whether a pixel is the colour expected, within the coding's rounding.
bool closeTo(QRgb actual, QRgb expected) {
  return std::abs(qRed(actual) - qRed(expected)) <= 24 && std::abs(qGreen(actual) - qGreen(expected)) <= 24 &&
         std::abs(qBlue(actual) - qBlue(expected)) <= 24;
}

const QRgb kRed = qRgb(255, 0, 0);
const QRgb kGreen = qRgb(0, 255, 0);
const QRgb kBlue = qRgb(0, 0, 255);
const QRgb kWhite = qRgb(255, 255, 255);

// The middles of the image's quarters: top left, top right, bottom left,
// bottom right.
bool quarters(const QImage& image, QRgb topLeft, QRgb topRight, QRgb bottomLeft, QRgb bottomRight) {
  const int w = image.width();
  const int h = image.height();
  return closeTo(image.pixel(w / 4, h / 4), topLeft) && closeTo(image.pixel(3 * w / 4, h / 4), topRight) &&
         closeTo(image.pixel(w / 4, 3 * h / 4), bottomLeft) &&
         closeTo(image.pixel(3 * w / 4, 3 * h / 4), bottomRight);
}

QByteArray pngFile() {
  QImage image(4, 2, QImage::Format_RGB32);
  image.fill(Qt::gray);
  QByteArray bytes;
  QBuffer buffer(&bytes);
  if (!buffer.open(QIODevice::WriteOnly) || !image.save(&buffer, "PNG")) {
    return {};
  }
  return bytes;
}

void formatTests() {
  CHECK(mitcad::isHeifFile(heifFile(bytesOf(kHvcc8), bytesOf(kData8), 0)));
  CHECK(!mitcad::isHeifFile(pngFile()));
  // An MP4 file's ftyp names no HEIF brand; too short a file is none.
  QByteArray mp4("\0\0\0\x18" "ftypisom\0\0\0\0isomiso2", 24);
  CHECK(!mitcad::isHeifFile(mp4));
  CHECK(!mitcad::isHeifFile(QByteArray("\0\0\0\x18" "ftyp", 8)));
  // An AVIF file is of the family (image_header refuses it with the reason).
  QByteArray avif("\0\0\0\x18" "ftypavif\0\0\0\0mif1avif", 24);
  CHECK(mitcad::isHeifFile(avif));

  // Other formats through Qt's readers as before.
  QString why;
  const QImage png = mitcad::decodeImageFile(pngFile(), &why);
  CHECK(!png.isNull() && png.width() == 4 && png.height() == 2 && why.isEmpty());
  CHECK(mitcad::decodeImageFile(QByteArray("no image"), &why).isNull());
  CHECK(why == QStringLiteral("Qt cannot read the image"));
}

void decodeTests() {
  QString why;
  const QByteArray file = heifFile(bytesOf(kHvcc8), bytesOf(kData8), 0);
  if (!mitcad::heifDecoderBuilt()) {
    // A build without the decoder (MITCAD_HEIF off) says so.
    CHECK(mitcad::decodeImageFile(file, &why).isNull());
    CHECK(why.contains(QStringLiteral("no HEIF decoder")));
    return;
  }
  const QImage upright = mitcad::decodeImageFile(file, &why);
  CHECK(why.isEmpty());
  if (!why.isEmpty()) {
    std::fprintf(stderr, "heif_image.cpp: %s\n", qPrintable(why));
  }
  CHECK(upright.width() == 64 && upright.height() == 32);
  CHECK(!upright.isNull() && quarters(upright, kRed, kGreen, kBlue, kWhite));

  // A quarter turn anticlockwise: 32 x 64, the right side on top.
  const QImage turned = mitcad::decodeImageFile(heifFile(bytesOf(kHvcc8), bytesOf(kData8), 1), &why);
  CHECK(turned.width() == 32 && turned.height() == 64);
  CHECK(!turned.isNull() && quarters(turned, kGreen, kWhite, kRed, kBlue));
  // Three quarters: the left side on top.
  const QImage back = mitcad::decodeImageFile(heifFile(bytesOf(kHvcc8), bytesOf(kData8), 3), &why);
  CHECK(back.width() == 32 && back.height() == 64);
  CHECK(!back.isNull() && quarters(back, kBlue, kRed, kWhite, kGreen));

  // 10 bits per channel, drawn with 8.
  const QImage deep = mitcad::decodeImageFile(heifFile(bytesOf(kHvcc10), bytesOf(kData10), 0), &why);
  CHECK(why.isEmpty());
  if (!why.isEmpty()) {
    std::fprintf(stderr, "heif_image.cpp: 10 bits: %s\n", qPrintable(why));
  }
  CHECK(deep.width() == 64 && deep.height() == 32);
  CHECK(!deep.isNull() && quarters(deep, kRed, kGreen, kBlue, kWhite));

  // Cut inside the image's data, or its data not HEVC: refused with the
  // decoder's reason.
  CHECK(mitcad::decodeImageFile(file.left(file.size() - 40), &why).isNull());
  CHECK(why.startsWith(QStringLiteral("the HEIF")));
  QByteArray garbled = file;
  garbled.replace(garbled.size() - 60, 56, QByteArray(56, '\0'));
  const QImage broken = mitcad::decodeImageFile(garbled, &why);
  CHECK(broken.isNull() == !why.isEmpty());
}

} // namespace

int heifImageTests() {
  failures = 0;
  formatTests();
  decodeTests();
  return failures;
}
