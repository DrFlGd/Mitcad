// SPDX-License-Identifier: MIT
#include "NativeNavigationGuide.hpp"

#import <AppKit/AppKit.h>

#include <algorithm>
#include <cmath>

#include <QBuffer>
#include <QImage>
#include <QPainter>
#include <QPointer>

#include "../../view/NavigationPreferences.hpp"

namespace mitcad::mac { class NativeNavigationGuide; }

@interface MitcadNativeNavigationGuide : NSImageView {
@public
  mitcad::mac::NativeNavigationGuide* owner;
}
@end

namespace mitcad::mac {
namespace {
QColor color(NSColor* native, const QColor& fallback) {
  NSColor* rgb = [native colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
  return rgb == nil ? fallback : QColor::fromRgbF(rgb.redComponent, rgb.greenComponent, rgb.blueComponent, rgb.alphaComponent);
}
}

class NativeNavigationGuide final : public NativeWidgetView {
public:
  explicit NativeNavigationGuide(NavigationGuide* presenter) : m_source(presenter) {
    m_view = [[MitcadNativeNavigationGuide alloc] initWithFrame:NSMakeRect(0, 0, 560, 142)];
    m_view->owner = this;
    m_view.identifier = @"navigationGuide";
    m_view.accessibilityIdentifier = presenter->objectName().toNSString();
    m_view.imageScaling = NSImageScaleNone;
    m_view.imageAlignment = NSImageAlignTopLeft;
    m_view.imageFrameStyle = NSImageFrameNone;
    m_view.accessibilityElement = YES;
    m_view.accessibilityRole = NSAccessibilityImageRole;
    [m_view.heightAnchor constraintEqualToConstant:142].active = YES;
    refresh();
  }
  ~NativeNavigationGuide() override { m_view->owner = nullptr; }
  void* nativeView() const override { return (__bridge void*)m_view; }
  void refresh() override {
    if (!m_source) { m_view.hidden = YES; return; }
    const NSSize logical = m_view.bounds.size;
    if (logical.width < 1 || logical.height < 1) return;
    const CGFloat scale = m_view.window.backingScaleFactor > 0 ? m_view.window.backingScaleFactor : 1;
    __block QPalette colors = m_source->palette();
    [m_view.effectiveAppearance performAsCurrentDrawingAppearance:^{
      colors.setColor(QPalette::Text, color(NSColor.labelColor, colors.color(QPalette::Text)));
      colors.setColor(QPalette::Highlight, color(NSColor.controlAccentColor, colors.color(QPalette::Highlight)));
    }];
    const QString key = QStringLiteral("%1|%2|%3|%4|%5|%6|%7")
        .arg(m_source->artworkRevision()).arg(logical.width).arg(logical.height).arg(scale)
        .arg(colors.color(QPalette::Text).rgba()).arg(colors.color(QPalette::Highlight).rgba())
        .arg(m_source->font().toString());
    if (key == m_key) return;
    m_key = key;
    const QSize pixels(std::max(1, int(std::ceil(logical.width * scale))),
                       std::max(1, int(std::ceil(logical.height * scale))));
    QImage image(pixels, QImage::Format_ARGB32_Premultiplied);
    image.setDevicePixelRatio(scale);
    image.fill(Qt::transparent);
    QPainter painter(&image);
    paintNavigationGuide(painter, QSizeF(logical.width, logical.height), m_source->navigation(), colors, m_source->font());
    painter.end();
    QByteArray bytes;
    QBuffer buffer(&bytes);
    buffer.open(QIODevice::WriteOnly);
    image.save(&buffer, "PNG");
    NSImage* native = [[NSImage alloc] initWithData:[NSData dataWithBytes:bytes.constData() length:bytes.size()]];
    native.size = logical;
    native.accessibilityDescription = m_source->accessibleDescription().toNSString();
    m_view.accessibilityLabel = m_source->accessibleName().toNSString();
    m_view.image = native;
  }
private:
  QPointer<NavigationGuide> m_source;
  MitcadNativeNavigationGuide* m_view = nil;
  QString m_key;
};

std::unique_ptr<NativeWidgetView> createNativeNavigationGuide(QWidget* presenter) {
  if (auto* guide = dynamic_cast<NavigationGuide*>(presenter)) return std::make_unique<NativeNavigationGuide>(guide);
  return {};
}

bool navigationGuideArtwork(NSView* view, QString& detail) {
  if (![view isKindOfClass:MitcadNativeNavigationGuide.class]) {
    detail = QStringLiteral("not a native navigation guide"); return false;
  }
  MitcadNativeNavigationGuide* guide = (MitcadNativeNavigationGuide*)view;
  if (guide->owner) guide->owner->refresh();
  NSBitmapImageRep* pixels = [[NSBitmapImageRep alloc] initWithData:guide.image.TIFFRepresentation];
  const CGFloat scale = guide.window.backingScaleFactor > 0 ? guide.window.backingScaleFactor : 1;
  if (pixels == nil || pixels.pixelsWide < guide.bounds.size.width * scale - 1 ||
      pixels.pixelsHigh < guide.bounds.size.height * scale - 1) {
    detail = QStringLiteral("navigation guide has no image at its actual backing scale"); return false;
  }
  // The actual native image must contain both the diagrams and the
  // bottom-row direction arrows. Text metadata alone never passes this.
  NSInteger ink = 0;
  NSInteger arrows = 0;
  const NSInteger arrowTop = NSInteger(std::floor(96 * scale));
  for (NSInteger y = 0; y < pixels.pixelsHigh; ++y) {
    for (NSInteger x = 0; x < pixels.pixelsWide; ++x) {
      NSColor* pixel = [pixels colorAtX:x y:y];
      if (pixel.alphaComponent > 0.3) {
        ++ink;
        if (y >= arrowTop) ++arrows;
      }
    }
  }
  detail = QStringLiteral("navigation guide: %1x%2 backing pixels, %3 painted, %4 in arrow row; %5")
      .arg(pixels.pixelsWide).arg(pixels.pixelsHigh).arg(ink).arg(arrows)
      .arg(QString::fromUtf8(guide.image.accessibilityDescription.UTF8String));
  return ink >= 300 * scale * scale && arrows >= 50 * scale * scale &&
         guide.accessibilityLabel.length > 0;
}
} // namespace mitcad::mac

@implementation MitcadNativeNavigationGuide
- (BOOL)isFlipped { return YES; }
- (NSSize)intrinsicContentSize { return NSMakeSize(560, 142); }
- (void)setFrameSize:(NSSize)size { [super setFrameSize:size]; if (owner) owner->refresh(); }
- (void)viewDidChangeBackingProperties { [super viewDidChangeBackingProperties]; if (owner) owner->refresh(); }
- (void)viewDidChangeEffectiveAppearance { [super viewDidChangeEffectiveAppearance]; if (owner) owner->refresh(); }
- (void)drawRect:(NSRect)dirty { if (owner) owner->refresh(); [super drawRect:dirty]; }
@end
