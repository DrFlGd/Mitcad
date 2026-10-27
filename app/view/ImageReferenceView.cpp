// SPDX-License-Identifier: MIT
// Image references in the 3D view (mitcad#782): each shown image a
// textured quad on its plane by its corners, see-through by its opacity,
// drawn with the bodies (hidden behind them) or over them; never picked.
#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstring>
#include <utility>

#include <QImage>
#include <QtLogging>

#include <AIS_InteractiveContext.hxx>
#include <AIS_InteractiveObject.hxx>
#include <Aspect_PolygonOffsetMode.hxx>
#include <Graphic3d_AlphaMode.hxx>
#include <Graphic3d_ArrayOfTriangles.hxx>
#include <Graphic3d_AspectFillArea3d.hxx>
#include <Graphic3d_Group.hxx>
#include <Graphic3d_Texture2D.hxx>
#include <Graphic3d_TextureParams.hxx>
#include <Graphic3d_ZLayerId.hxx>
#include <Image_PixMap.hxx>
#include <Prs3d_Presentation.hxx>
#include <PrsMgr_PresentationManager.hxx>
#include <Quantity_Color.hxx>
#include <SelectMgr_Selection.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>

#include "../OcctViewer.hpp"
#include "ImageReferences.hpp"

namespace mitcad {
namespace {

// The longest side of an image's texture (pixels): larger images are
// drawn from a smaller copy.
constexpr int kMaxTexture = 4096;

// A texture of the image, its alpha times the opacity (the view blends
// it); rows top down.
occ::handle<Graphic3d_Texture2D> imageTexture(const QImage& source, double opacity) {
  QImage image = source;
  if (std::max(image.width(), image.height()) > kMaxTexture) {
    image = image.scaled(kMaxTexture, kMaxTexture, Qt::KeepAspectRatio, Qt::SmoothTransformation);
  }
  const QImage straight = image.convertToFormat(QImage::Format_ARGB32);
  occ::handle<Image_PixMap> pixels = new Image_PixMap();
  pixels->InitZero(Image_Format_BGRA, static_cast<std::size_t>(straight.width()),
                   static_cast<std::size_t>(straight.height()));
  pixels->SetTopDown(true);
  const double alpha = std::clamp(opacity, 0.0, 1.0);
  for (int y = 0; y < straight.height(); ++y) {
    // QImage's ARGB32 is B, G, R, A in memory on little-endian machines.
    auto* row = static_cast<unsigned char*>(pixels->ChangeRow(static_cast<std::size_t>(y)));
    std::memcpy(row, straight.constScanLine(y), static_cast<std::size_t>(straight.width()) * 4);
    for (int x = 0; x < straight.width(); ++x) {
      unsigned char& a = row[static_cast<std::size_t>(x) * 4 + 3];
      a = static_cast<unsigned char>(std::lround(a * alpha));
    }
  }
  occ::handle<Graphic3d_Texture2D> texture = new Graphic3d_Texture2D(pixels);
  texture->GetParams()->SetModulate(false);
  texture->GetParams()->SetFilter(Graphic3d_TOTF_BILINEAR);
  return texture;
}

// One image on its plane: two triangles between its corners, both sides,
// unlit, blended; nothing to pick.
class ImageQuad : public AIS_InteractiveObject {
public:
  ImageQuad(const ImageReferenceShown& image, occ::handle<Graphic3d_Texture2D> texture)
      : m_corners(image.corners), m_texture(std::move(texture)) {}

  bool AcceptDisplayMode(const int mode) const override { return mode == 0; }

protected:
  void Compute(const occ::handle<PrsMgr_PresentationManager>&, const occ::handle<Prs3d_Presentation>& presentation,
               const int mode) override {
    if (mode != 0 || m_texture.IsNull()) {
      return;
    }
    const auto point = [this](std::size_t i) {
      return gp_Pnt(m_corners[i][0], m_corners[i][1], m_corners[i][2]);
    };
    occ::handle<Graphic3d_ArrayOfTriangles> quad =
        new Graphic3d_ArrayOfTriangles(4, 6, Graphic3d_ArrayFlags_VertexTexel);
    // Bottom left, bottom right, top right, top left: the texture's rows
    // run top down.
    quad->AddVertex(point(3), gp_Pnt2d(0, 0));
    quad->AddVertex(point(2), gp_Pnt2d(1, 0));
    quad->AddVertex(point(1), gp_Pnt2d(1, 1));
    quad->AddVertex(point(0), gp_Pnt2d(0, 1));
    quad->AddEdges(1, 2, 3);
    quad->AddEdges(1, 3, 4);
    occ::handle<Graphic3d_AspectFillArea3d> aspect = new Graphic3d_AspectFillArea3d();
    aspect->SetInteriorStyle(Aspect_IS_SOLID);
    aspect->SetShadingModel(Graphic3d_TypeOfShadingModel_Unlit);
    aspect->SetInteriorColor(Quantity_Color(Quantity_NOC_WHITE));
    aspect->SetFaceCulling(Graphic3d_TypeOfBackfacingModel_DoubleSided);
    aspect->SetAlphaMode(Graphic3d_AlphaMode_Blend);
    // In front of a face it lies on.
    aspect->SetPolygonOffsets(Aspect_POM_Fill, -1.0f, -1.0f);
    aspect->SetTextureMap(m_texture);
    aspect->SetTextureMapOn();
    const occ::handle<Graphic3d_Group> group = presentation->NewGroup();
    group->SetGroupPrimitivesAspect(aspect);
    group->AddPrimitiveArray(quad);
  }
  void ComputeSelection(const occ::handle<SelectMgr_Selection>&, const int) override {}

private:
  std::array<ImagePoint, 4> m_corners;
  occ::handle<Graphic3d_Texture2D> m_texture;
};

} // namespace

void OcctViewer::setImageReferences(const std::vector<ImageReferenceShown>& shown,
                                    const QHash<QString, QImage>& images) {
  for (const occ::handle<AIS_InteractiveObject>& object : m_imageReferences) {
    m_context->Remove(object, false);
  }
  m_imageReferences.clear();
  QHash<QString, occ::handle<Graphic3d_Texture2D>> textures;
  std::vector<ImageReferenceShown> drawn;
  for (const ImageReferenceShown& image : shown) {
    const QImage picture = images.value(image.sha256);
    if (picture.isNull()) {
      continue; // not decoded: nothing to draw
    }
    // A texture of each image and opacity, kept while it is drawn.
    const QString key = QStringLiteral("%1/%2").arg(image.sha256).arg(image.opacity);
    occ::handle<Graphic3d_Texture2D> texture = m_imageTextures.value(key);
    if (texture.IsNull()) {
      texture = imageTexture(picture, image.opacity);
    }
    textures.insert(key, texture);
    occ::handle<AIS_InteractiveObject> quad = new ImageQuad(image, texture);
    m_context->Display(quad, 0, -1, false);
    if (image.through) {
      // Over the bodies, under the sketches traced on it (OcctViewer's
      // image layer, before the sketch halos and the top layer).
      m_context->SetZLayer(quad, m_imageLayer);
    }
    m_imageReferences.push_back(quad);
    drawn.push_back(image);
  }
  m_imageTextures = std::move(textures);
  // For UI tests: the layer the see-through ones are in.
  const bool through = std::any_of(drawn.begin(), drawn.end(), [](const ImageReferenceShown& image) { return image.through; });
  if (through && !m_loggedImageLayer) {
    m_loggedImageLayer = true;
    qDebug().noquote() << QStringLiteral("Image references through the bodies: in the image layer, before the sketch halos");
  }
  const QString logged = imageReferencesLog(drawn);
  if (logged != m_loggedImageReferences) {
    m_loggedImageReferences = logged;
    qDebug().noquote() << logged;
  }
  sceneChanged();
}

} // namespace mitcad
