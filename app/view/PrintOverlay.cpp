// SPDX-License-Identifier: MIT
// Print Analysis in the 3D view (mitcad#620): the overhang and thin-wall
// triangles over the bodies, the thinnest point's marker and label, and the
// legend.
#include "PrintOverlay.hpp"

#include <algorithm>
#include <array>
#include <cstddef>
#include <initializer_list>

#include <QEvent>
#include <QFont>
#include <QFontMetrics>
#include <QMargins>
#include <QPainter>
#include <QPalette>
#include <QPainterPath>
#include <QPen>
#include <QRectF>
#include <QtLogging>

#include <AIS_InteractiveContext.hxx>
#include <AIS_InteractiveObject.hxx>
#include <Aspect_PolygonOffsetMode.hxx>
#include <Graphic3d_ArrayOfTriangles.hxx>
#include <Graphic3d_AspectFillArea3d.hxx>
#include <Graphic3d_Group.hxx>
#include <Prs3d_Presentation.hxx>
#include <PrsMgr_PresentationManager.hxx>
#include <Quantity_Color.hxx>
#include <SelectMgr_Selection.hxx>
#include <gp_Vec.hxx>

#include "../OcctViewer.hpp"
#include "../commands/PrintAnalysis.hpp"

namespace mitcad {
namespace {

// How far the triangles lie off the body's surface, along their normals
// (mm), over the polygon offset: the analysis' mesh and the view's differ
// on curved faces by their deviations (a few hundredths of a millimetre).
// The thin walls' lie over the overhangs' where a triangle is both.
constexpr double kOverhangLift = 0.05;
constexpr double kThinLift = 0.08;

constexpr qreal kMargin = 12.0;  // from the view's edges and the cards over it
constexpr qreal kPadding = 8.0;  // inside a card
constexpr qreal kSwatch = 12.0;  // a legend colour's square
constexpr qreal kMarker = 7.0;   // the marker's radius

Quantity_Color occtColor(const QColor& color) {
  return Quantity_Color(color.redF(), color.greenF(), color.blueF(), Quantity_TOC_sRGB);
}

// The triangles of one colour as a group of the presentation: flat (light
// does not darken them, as the section caps), both sides, in front of the
// body's own faces they lie on.
void addTriangles(const occ::handle<Prs3d_Presentation>& presentation, const std::vector<float>& xyz,
                  const QColor& color, double lift, float offset) {
  const std::size_t count = xyz.size() / 9;
  if (count == 0) {
    return;
  }
  occ::handle<Graphic3d_ArrayOfTriangles> triangles =
      new Graphic3d_ArrayOfTriangles(static_cast<int>(count * 3));
  for (std::size_t t = 0; t < count; ++t) {
    const float* v = xyz.data() + t * 9;
    const gp_Pnt a(v[0], v[1], v[2]);
    const gp_Pnt b(v[3], v[4], v[5]);
    const gp_Pnt c(v[6], v[7], v[8]);
    const gp_Vec normal = gp_Vec(a, b).Crossed(gp_Vec(a, c));
    const double length = normal.Magnitude();
    const gp_Vec shift = length > 1e-12 ? normal * (lift / length) : gp_Vec(0.0, 0.0, 0.0);
    for (const gp_Pnt& corner : {a, b, c}) {
      triangles->AddVertex(corner.Translated(shift));
    }
  }
  occ::handle<Graphic3d_AspectFillArea3d> aspect = new Graphic3d_AspectFillArea3d();
  aspect->SetInteriorStyle(Aspect_IS_SOLID);
  aspect->SetShadingModel(Graphic3d_TypeOfShadingModel_Unlit);
  aspect->SetInteriorColor(occtColor(color));
  aspect->SetFaceCulling(Graphic3d_TypeOfBackfacingModel_DoubleSided);
  aspect->SetPolygonOffsets(Aspect_POM_Fill, offset, offset);
  const occ::handle<Graphic3d_Group> group = presentation->NewGroup();
  group->SetGroupPrimitivesAspect(aspect);
  group->AddPrimitiveArray(triangles);
}

// The overhang and thin-wall triangles, in design coordinates; not
// pickable.
class PrintTriangles : public AIS_InteractiveObject {
public:
  explicit PrintTriangles(const PrintOverlay& overlay)
      : m_overhangs(overlay.overhangs), m_thin(overlay.thin), m_overhangColor(overlay.overhangColor),
        m_thinColor(overlay.thinColor) {}

  bool AcceptDisplayMode(const int mode) const override { return mode == 0; }

protected:
  void Compute(const occ::handle<PrsMgr_PresentationManager>&, const occ::handle<Prs3d_Presentation>& presentation,
               const int mode) override {
    if (mode != 0) {
      return;
    }
    addTriangles(presentation, m_overhangs, m_overhangColor, kOverhangLift, -1.0f);
    addTriangles(presentation, m_thin, m_thinColor, kThinLift, -2.0f);
  }
  void ComputeSelection(const occ::handle<SelectMgr_Selection>&, const int) override {}

private:
  std::vector<float> m_overhangs;
  std::vector<float> m_thin;
  QColor m_overhangColor;
  QColor m_thinColor;
};

// A card over the view, as the view's banner draws one.
void drawCard(QPainter& painter, const QRectF& rect, const QPalette& palette) {
  QColor background = palette.color(QPalette::Window);
  background.setAlpha(235);
  QPainterPath path;
  path.addRoundedRect(rect, 6.0, 6.0);
  painter.fillPath(path, background);
  painter.setPen(QPen(palette.color(QPalette::Mid), 1.0));
  painter.drawPath(path);
}

} // namespace

void OcctViewer::setPrintOverlay(const PrintOverlay& overlay) {
  if (!m_printTriangles.IsNull()) {
    m_context->Remove(m_printTriangles, false);
    m_printTriangles.Nullify();
  }
  if (!overlay.overhangs.empty() || !overlay.thin.empty()) {
    m_printTriangles = new PrintTriangles(overlay);
    m_context->Display(m_printTriangles, 0, -1, false);
  }
  if (m_printLabels == nullptr && !overlay.isEmpty()) {
    m_printLabels = new PrintOverlayLabels(this);
  }
  if (m_printLabels != nullptr) {
    m_printLabels->setOverlay(overlay);
  }
  // For UI tests: what is drawn, when it changes.
  const QString logged = printOverlayLog(overlay);
  if (logged != m_loggedPrintOverlay) {
    m_loggedPrintOverlay = logged;
    qDebug().noquote() << logged;
  }
  sceneChanged();
}

PrintOverlayLabels::PrintOverlayLabels(OcctViewer* viewer) : QWidget(viewer), m_viewer(viewer) {
  setObjectName(QStringLiteral("printOverlayLabels"));
  setAttribute(Qt::WA_TransparentForMouseEvents);
  setAttribute(Qt::WA_NoSystemBackground);
  setAttribute(Qt::WA_TranslucentBackground);
  setFocusPolicy(Qt::NoFocus);
  hide();
  viewer->installEventFilter(this);
  // The camera moved (a frame was drawn): the marker follows its point.
  connect(viewer, &OcctViewer::viewChanged, this, [this] {
    if (isVisible()) {
      follow();
    }
  });
}

void PrintOverlayLabels::setOverlay(const PrintOverlay& overlay) {
  m_point.reset();
  if (overlay.thinnest) {
    const std::array<double, 3>& p = *overlay.thinnest;
    m_point = gp_Pnt(p[0], p[1], p[2]);
  }
  m_label = overlay.thinnestLabel;
  m_title = overlay.title;
  m_legend = overlay.legend;
  m_markerColor = overlay.thinColor;
  if (overlay.isEmpty()) {
    hide();
    return;
  }
  show();
  lower(); // under the view's own buttons and cards
  follow();
  update();
}

void PrintOverlayLabels::follow() {
  if (geometry() != m_viewer->rect()) {
    setGeometry(m_viewer->rect());
    update();
  }
  if (m_point) {
    const QPointF at = m_viewer->toWidgetF(*m_point);
    if (at != m_drawnAt) {
      m_drawnAt = at;
      update();
    }
  }
}

bool PrintOverlayLabels::eventFilter(QObject* watched, QEvent* event) {
  if (watched == m_viewer && event->type() == QEvent::Resize && isVisible()) {
    follow();
  }
  return QWidget::eventFilter(watched, event);
}

void PrintOverlayLabels::paintEvent(QPaintEvent* /*event*/) {
  QPainter painter(this);
  painter.setRenderHint(QPainter::Antialiasing);
  const QFontMetrics metrics(font());
  const qreal line = metrics.height();
  // The thinnest point: a ring in the thin walls' colour with a dot, and
  // its label beside it, over everything (it may lie inside the body).
  if (m_point) {
    const QPointF at = m_viewer->toWidgetF(*m_point);
    m_drawnAt = at;
    painter.setBrush(Qt::NoBrush);
    painter.setPen(QPen(QColor(255, 255, 255, 220), 4.0));
    painter.drawEllipse(at, kMarker, kMarker);
    painter.setPen(QPen(m_markerColor, 2.0));
    painter.drawEllipse(at, kMarker, kMarker);
    painter.setPen(Qt::NoPen);
    painter.setBrush(m_markerColor);
    painter.drawEllipse(at, 2.5, 2.5);
    if (!m_label.isEmpty()) {
      const QRectF card(at.x() + kMarker + 6.0, at.y() - line / 2.0 - kPadding / 2.0,
                        metrics.horizontalAdvance(m_label) + 2.0 * kPadding, line + kPadding);
      drawCard(painter, card, palette());
      painter.setPen(palette().color(QPalette::WindowText));
      painter.drawText(card, Qt::AlignCenter, m_label);
    }
  }
  // The legend in the lower left corner, clear of the cards over the view.
  if (m_legend.empty()) {
    return;
  }
  QFont bold = font();
  bold.setBold(true);
  const QFontMetrics boldMetrics(bold);
  qreal textWidth = boldMetrics.horizontalAdvance(m_title);
  for (const auto& entry : m_legend) {
    textWidth = std::max(textWidth, kSwatch + 6.0 + metrics.horizontalAdvance(entry.second));
  }
  const qreal cardHeight =
      line * static_cast<qreal>(m_legend.size() + (m_title.isEmpty() ? 0 : 1)) + 2.0 * kPadding;
  const QMargins insets = m_viewer->overlayInsets();
  const QRectF card(insets.left() + kMargin, m_viewer->height() - insets.bottom() - kMargin - cardHeight,
                    textWidth + 2.0 * kPadding, cardHeight);
  drawCard(painter, card, palette());
  qreal row = card.top() + kPadding;
  const qreal textLeft = card.left() + kPadding;
  if (!m_title.isEmpty()) {
    painter.setFont(bold);
    painter.setPen(palette().color(QPalette::WindowText));
    painter.drawText(QRectF(textLeft, row, textWidth, line), Qt::AlignLeft | Qt::AlignVCenter, m_title);
    painter.setFont(font());
    row += line;
  }
  for (const auto& [color, words] : m_legend) {
    painter.setPen(QPen(palette().color(QPalette::Mid), 1.0));
    painter.setBrush(color);
    painter.drawRect(QRectF(textLeft, row + (line - kSwatch) / 2.0, kSwatch, kSwatch));
    painter.setPen(palette().color(QPalette::WindowText));
    painter.drawText(QRectF(textLeft + kSwatch + 6.0, row, textWidth - kSwatch - 6.0, line),
                     Qt::AlignLeft | Qt::AlignVCenter, words);
    row += line;
  }
}

} // namespace mitcad
