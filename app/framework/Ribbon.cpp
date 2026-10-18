// SPDX-License-Identifier: MIT
#include "Ribbon.hpp"

#include <algorithm>
#include <utility>
#include <vector>

#include <QAction>
#include <QCursor>
#include <QEvent>
#include <QFrame>
#include <QHBoxLayout>
#include <QMargins>
#include <QMenu>
#include <QMouseEvent>
#include <QResizeEvent>
#include <QShowEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPointer>
#include <QRegion>
#include <QStackedWidget>
#include <QStyleOptionToolButton>
#include <QTabBar>
#include <QToolButton>
#include <QVBoxLayout>
#include <QtLogging>

#include "CommandRegistry.hpp"
#include "PlaceWatcher.hpp"
#include "TestSync.hpp"
#include "Theme.hpp"
#include "WindowStyle.hpp"
#include "platform/MacChrome.hpp"

namespace mitcad {
namespace {

const QString kSolid = QStringLiteral("SOLID");
const QString kSketch = QStringLiteral("SKETCH");
// Modular slot-profile assemblies (mitcad#375): a tab of its own beside
// SOLID, always shown.
const QString kBuild = QStringLiteral("BUILD");
// The tabs' data: the index of their page.
constexpr int kSolidPage = 0;
constexpr int kSketchPage = 1;
constexpr int kBuildPage = 2;
constexpr int kIconSize = 28;

QString key(const QString& tab, const QString& group) { return tab + QLatin1Char('/') + group; }

constexpr int kMacIconSize = 26;
constexpr int kMacButtonSize = kMacIconSize + 10;
constexpr int kMacCaptionHeight = 20;
constexpr int kHeight = 30;
// The arrow part of a split button (mitcad#222) in the Mac presentation; the
// logged arrow position of the classic one is as far in from its right edge.
constexpr int kSplitArrowWidth = 14;

// A menu's entries for the UI tests' log: "-" a separator, a submenu as
// "Name > [its entries]".
QString menuEntries(const QMenu* menu) {
  QStringList entries;
  for (const QAction* action : menu->actions()) {
    if (action->isSeparator()) {
      entries << QStringLiteral("-");
      continue;
    }
    if (!action->isVisible()) {
      continue;
    }
    QString name = action->text();
    name.remove(QLatin1Char('&'));
    if (const QMenu* submenu = action->menu()) {
      name += QStringLiteral(" > [%1]").arg(menuEntries(submenu));
    }
    entries << name;
  }
  return entries.join(QStringLiteral(" | "));
}

// The palette's text colour at a fraction of full opacity: the hover and pressed
// highlights and the quiet fills, which are right on a light and a dark palette.
QColor textTint(const QPalette& palette, int alpha) {
  QColor color = palette.color(QPalette::WindowText);
  color.setAlpha(alpha);
  return color;
}

// A small chevron pointing down, centred on `center`.
void drawChevron(QPainter& painter, const QPointF& center, const QColor& color) {
  painter.save();
  painter.setRenderHint(QPainter::Antialiasing);
  QPen pen(color, 1.4);
  pen.setCapStyle(Qt::RoundCap);
  pen.setJoinStyle(Qt::RoundJoin);
  painter.setPen(pen);
  painter.setBrush(Qt::NoBrush);
  QPainterPath path;
  path.moveTo(center.x() - 3.2, center.y() - 1.6);
  path.lineTo(center.x(), center.y() + 1.6);
  path.lineTo(center.x() + 3.2, center.y() - 1.6);
  painter.drawPath(path);
  painter.restore();
}

// The group's name under its buttons, with a chevron: opens the menu of all the
// group's commands.
class GroupCaption : public QToolButton {
public:
  GroupCaption(const QString& text, QWidget* parent = nullptr) : QToolButton(parent) {
    setText(text);
    setAttribute(Qt::WA_Hover);
  }

  QSize sizeHint() const override {
    QFont small = font();
    small.setPointSizeF(small.pointSizeF() * 0.92);
    return QSize(QFontMetrics(small).horizontalAdvance(text()) + 34, kMacCaptionHeight);
  }

protected:
  void paintEvent(QPaintEvent*) override {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    const QPalette pal = palette();
    if (isDown()) {
      painter.setPen(Qt::NoPen);
      painter.setBrush(textTint(pal, 36));
      painter.drawRoundedRect(QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5), height() / 2.0, height() / 2.0);
    } else if (underMouse()) {
      painter.setPen(Qt::NoPen);
      painter.setBrush(textTint(pal, 20));
      painter.drawRoundedRect(QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5), height() / 2.0, height() / 2.0);
    }
    QFont small = font();
    small.setPointSizeF(small.pointSizeF() * 0.92);
    painter.setFont(small);
    const QColor color = textTint(pal, 190);
    painter.setPen(color);
    const int textWidth = QFontMetrics(small).horizontalAdvance(text());
    const int total = textWidth + 5 + 8;
    const int left = (width() - total) / 2;
    painter.drawText(QRect(left, 0, textWidth, height()), Qt::AlignVCenter | Qt::AlignLeft, text());
    drawChevron(painter, QPointF(left + textWidth + 5 + 4, height() / 2.0 + 0.5), color);
  }
};

// The workspace tabs as a macOS segmented control: a capsule track with the
// selected segment as a raised pill. It stays a QTabBar so that the tabs keep
// their behaviour and signals.
class SegmentedTabBar : public QTabBar {
public:
  explicit SegmentedTabBar(QWidget* parent = nullptr) : QTabBar(parent) {
    setDrawBase(false);
    setExpanding(false);
    setFocusPolicy(Qt::NoFocus);
    setUsesScrollButtons(false);
    setMouseTracking(true);
    setAttribute(Qt::WA_Hover);
    setFixedHeight(kHeight);
  }

protected:
  QSize tabSizeHint(int index) const override {
    return QSize(fontMetrics().horizontalAdvance(displayName(tabText(index))) + 30, kHeight);
  }
  QSize minimumTabSizeHint(int index) const override { return tabSizeHint(index); }

  void leaveEvent(QEvent* event) override {
    QTabBar::leaveEvent(event);
    update();
  }
  void mouseMoveEvent(QMouseEvent* event) override {
    QTabBar::mouseMoveEvent(event);
    update();
  }

  void paintEvent(QPaintEvent*) override {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    const QPalette pal = palette();
    const bool dark = isDarkPalette(pal);
    const QRectF track = QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5);
    painter.setPen(Qt::NoPen);
    painter.setBrush(textTint(pal, dark ? 24 : 17));
    painter.drawRoundedRect(track, track.height() / 2, track.height() / 2);
    const int hovered = tabAt(mapFromGlobal(QCursor::pos()));
    for (int i = 0; i < count(); ++i) {
      const QRectF pill = QRectF(tabRect(i)).adjusted(2.5, 2.5, -2.5, -2.5);
      const bool selected = i == currentIndex();
      const bool sketch = tabData(i).toInt() == 1;
      if (selected) {
        painter.setPen(Qt::NoPen);
        QColor shadow = cardShadowColor(pal);
        shadow.setAlpha(shadow.alpha() / 2);
        painter.setBrush(shadow);
        painter.drawRoundedRect(pill.translated(0, 0.8), pill.height() / 2, pill.height() / 2);
        painter.setPen(QPen(cardBorderColor(pal), 1));
        painter.setBrush(dark ? textTint(pal, 56) : pal.color(QPalette::Base));
        painter.drawRoundedRect(pill, pill.height() / 2, pill.height() / 2);
      } else if (i == hovered && isEnabled()) {
        painter.setPen(Qt::NoPen);
        painter.setBrush(textTint(pal, 18));
        painter.drawRoundedRect(pill, pill.height() / 2, pill.height() / 2);
      }
      QFont f = font();
      f.setWeight(selected ? QFont::DemiBold : QFont::Medium);
      painter.setFont(f);
      painter.setPen(sketch ? (dark ? accentColor(pal).lighter(135) : accentColor(pal)) : (selected ? pal.color(QPalette::WindowText) : textTint(pal, 200)));
      painter.drawText(tabRect(i), Qt::AlignCenter, displayName(tabText(i)));
    }
  }
};

// The workspace tabs of the Classic presentation, the Docked chrome's
// (mitcad#251): small semibold names, the selected one on a pill of the
// window's accent colour; SKETCH's name in the accent colour.
class PillTabBar : public QTabBar {
public:
  explicit PillTabBar(QWidget* parent = nullptr) : QTabBar(parent) {
    setDrawBase(false);
    setExpanding(false);
    setFocusPolicy(Qt::NoFocus);
    setUsesScrollButtons(false);
    setMouseTracking(true);
    setAttribute(Qt::WA_Hover);
  }

protected:
  QSize tabSizeHint(int index) const override {
    const QFontMetrics metrics(tabFont());
    return QSize(metrics.horizontalAdvance(tabText(index)) + 24, metrics.height() + 10);
  }
  QSize minimumTabSizeHint(int index) const override { return tabSizeHint(index); }

  void leaveEvent(QEvent* event) override {
    QTabBar::leaveEvent(event);
    update();
  }
  void mouseMoveEvent(QMouseEvent* event) override {
    QTabBar::mouseMoveEvent(event);
    update();
  }

  void paintEvent(QPaintEvent*) override {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    const WindowColors colors = windowColors(palette());
    const int hovered = tabAt(mapFromGlobal(QCursor::pos()));
    painter.setFont(tabFont());
    for (int i = 0; i < count(); ++i) {
      const QRectF pill = QRectF(tabRect(i)).adjusted(1.5, 2.5, -1.5, -2.5);
      const bool selected = i == currentIndex();
      painter.setPen(Qt::NoPen);
      if (selected) {
        QColor fill = colors.accent;
        fill.setAlphaF(0.16f);
        painter.setBrush(fill);
        painter.drawRoundedRect(pill, pill.height() / 2, pill.height() / 2);
      } else if (i == hovered && isEnabled()) {
        QColor fill = colors.text;
        fill.setAlpha(18);
        painter.setBrush(fill);
        painter.drawRoundedRect(pill, pill.height() / 2, pill.height() / 2);
      }
      const bool sketch = tabData(i).toInt() == 1;
      QColor text = colors.secondaryText;
      if (sketch || selected) {
        // The accent on the pill, lighter on a dark theme to be read there.
        text = colors.dark ? colors.accent.lighter(130) : colors.accent;
      }
      if (selected && !sketch && colors.dark) {
        text = QColor(0xd6, 0xe7, 0xff);
      }
      painter.setPen(text);
      painter.drawText(tabRect(i), Qt::AlignCenter, tabText(i));
    }
  }

private:
  QFont tabFont() const {
    QFont small = font();
    if (small.pointSizeF() > 0) {
      small.setPointSizeF(small.pointSizeF() * 0.9);
    }
    small.setWeight(QFont::DemiBold);
    small.setLetterSpacing(QFont::PercentageSpacing, 105);
    return small;
  }
};

// A line between two groups of the Classic presentation: a hairline of the
// surface's (WindowStyle) instead of a sunken frame.
class GroupSeparator : public QWidget {
public:
  explicit GroupSeparator(QWidget* parent = nullptr) : QWidget(parent) {
    setFixedWidth(5);
    setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
  }

protected:
  void paintEvent(QPaintEvent*) override {
    QPainter painter(this);
    painter.fillRect(QRect(width() / 2, 4, 1, height() - 8), windowColors(palette()).line);
  }
};

QToolButton* commandButton(QAction* action, bool mac) {
  QToolButton* button = nullptr;
  if (mac) {
    button = new RibbonButton(action, kMacIconSize);
  } else {
    button = new QToolButton;
    button->setDefaultAction(action);
    button->setIconSize(QSize(kIconSize, kIconSize));
    button->setToolButtonStyle(Qt::ToolButtonIconOnly);
    button->setAutoRaise(true);
  }
  button->setFocusPolicy(Qt::NoFocus);
  if (action->menu() != nullptr) {
    button->setPopupMode(QToolButton::InstantPopup);
  }
  return button;
}

} // namespace

QString displayName(const QString& name) {
  QStringList words = name.toLower().split(QLatin1Char(' '), Qt::SkipEmptyParts);
  for (QString& word : words) {
    word[0] = word.at(0).toUpper();
  }
  return words.join(QLatin1Char(' '));
}

RibbonButton::RibbonButton(QAction* action, int iconSize, QWidget* parent)
    : QToolButton(parent), m_iconSize(iconSize) {
  setDefaultAction(action);
  setIconSize(QSize(iconSize, iconSize));
  setToolButtonStyle(Qt::ToolButtonIconOnly);
  setAutoRaise(true);
  setFocusPolicy(Qt::NoFocus);
  setAttribute(Qt::WA_Hover);
  if (action->menu() != nullptr) {
    setPopupMode(QToolButton::InstantPopup);
  }
}

QSize RibbonButton::sizeHint() const {
  const int arrow = popupMode() == QToolButton::MenuButtonPopup ? kSplitArrowWidth : 0;
  return QSize(m_iconSize + 10 + arrow, m_iconSize + 10);
}

void RibbonButton::mousePressEvent(QMouseEvent* event) {
  // A split button's arrow part opens its menu; the rest runs the action.
  if (popupMode() == QToolButton::MenuButtonPopup && event->button() == Qt::LeftButton &&
      event->position().x() >= width() - kSplitArrowWidth) {
    event->accept();
    showMenu();
    return;
  }
  QToolButton::mousePressEvent(event);
}

void RibbonButton::changeEvent(QEvent* event) {
  QToolButton::changeEvent(event);
  if (event->type() == QEvent::ApplicationPaletteChange || event->type() == QEvent::PaletteChange) {
    update();
  }
}

void RibbonButton::paintEvent(QPaintEvent*) {
  QPainter painter(this);
  painter.setRenderHint(QPainter::Antialiasing);
  const QPalette pal = palette();
  const QRectF box = QRectF(rect()).adjusted(1, 1, -1, -1);
  const bool active = isEnabled();
  painter.setPen(Qt::NoPen);
  if (isChecked()) {
    QColor tint = accentColor(pal);
    tint.setAlpha(active ? 51 : 25);
    painter.setBrush(tint);
    painter.drawRoundedRect(box, 8, 8);
  } else if (isDown() && active) {
    painter.setBrush(textTint(pal, 36));
    painter.drawRoundedRect(box, 8, 8);
  } else if (underMouse() && active) {
    painter.setBrush(textTint(pal, 20));
    painter.drawRoundedRect(box, 8, 8);
  }
  if (isChecked() && active) {
    painter.setPen(QPen(accentColor(pal), 1));
    painter.setBrush(Qt::NoBrush);
    painter.drawRoundedRect(box.adjusted(0.5, 0.5, -0.5, -0.5), 7.5, 7.5);
  }
  // A split button (mitcad#222): the icon left of the arrow part, a chevron
  // in it, divided from the icon while hovered.
  const int arrow = popupMode() == QToolButton::MenuButtonPopup ? kSplitArrowWidth : 0;
  if (arrow > 0) {
    const QColor color = textTint(pal, active ? 190 : 80);
    drawChevron(painter, QPointF(width() - arrow / 2.0, height() / 2.0 + 0.5), color);
    if (underMouse() && active) {
      painter.setPen(QPen(textTint(pal, 40), 1));
      painter.drawLine(QPointF(width() - arrow + 0.5, 8), QPointF(width() - arrow + 0.5, height() - 8));
    }
  }
  const QIcon::Mode mode = active ? QIcon::Normal : QIcon::Disabled;
  const QPixmap pixmap = icon().pixmap(QSize(m_iconSize, m_iconSize), devicePixelRatioF(), mode,
                                       isChecked() ? QIcon::On : QIcon::Off);
  const QSizeF size = pixmap.deviceIndependentSize();
  painter.drawPixmap(QPointF((width() - arrow - size.width()) / 2, (height() - size.height()) / 2), pixmap);
}

CapsuleFrame::CapsuleFrame(int radius, QWidget* parent) : QWidget(parent), m_radius(radius) {}

void CapsuleFrame::setCardMaterial(bool card) {
  m_cardMaterial = card;
  update();
}

void CapsuleFrame::paintEvent(QPaintEvent*) {
  QPainter painter(this);
  painter.setRenderHint(QPainter::Antialiasing);
  const QPalette pal = palette();
  const QRectF box = QRectF(rect()).adjusted(1.5, 1.5, -1.5, -2.5);
  const qreal radius = m_radius < 0 ? box.height() / 2 : m_radius;
  QColor shadow = cardShadowColor(pal);
  shadow.setAlpha(shadow.alpha() / 3);
  if (m_cardMaterial) {
    // The card's fill is translucent: the shadow only where the card is not.
    QPainterPath outside;
    outside.addRect(QRectF(rect()));
    QPainterPath card;
    card.addRoundedRect(box, radius, radius);
    painter.setClipPath(outside.subtracted(card));
  }
  painter.setPen(Qt::NoPen);
  painter.setBrush(shadow);
  painter.drawRoundedRect(box.translated(0, 1), radius, radius);
  painter.setClipping(false);
  painter.setPen(QPen(cardBorderColor(pal), 1));
  if (m_cardMaterial) {
    painter.setBrush(cardFillColor(pal)); // as GlassCard's material
  } else {
    painter.setBrush(isDarkPalette(pal) ? textTint(pal, 20) : pal.color(QPalette::Base));
  }
  painter.drawRoundedRect(box, radius, radius);
}

Ribbon::Ribbon(CommandRegistry& registry, Presentation presentation, QWidget* parent)
    : QWidget(parent), m_registry(registry), m_presentation(presentation) {
  setObjectName(QStringLiteral("ribbon"));
  auto* layout = new QVBoxLayout(this);
  layout->setSpacing(0);
  m_pages = new QStackedWidget;
  if (presentation == Presentation::Mac) {
    layout->setContentsMargins(0, 0, 0, 1);
    // The tab bar is the title bar's; it is a child of the ribbon until that
    // takes it over. The groups clip rather than make the window wider.
    m_tabs = new SegmentedTabBar(this);
    setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
  } else {
    layout->setContentsMargins(4, 2, 4, 2);
    // The Docked chrome's pills (mitcad#251).
    m_tabs = new PillTabBar;
    layout->addWidget(m_tabs);
    // In the toolbar the ribbon takes the width there is and clips the
    // groups at its right rather than asking for all of them: a toolbar
    // moves a widget that does not fit into its overflow menu, which hid
    // the whole ribbon in a narrow window (mitcad#375, the Windows VM's).
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    layout->setSizeConstraint(QLayout::SetNoConstraint);
  }
  layout->addWidget(m_pages);
  // The group cards' mask follows the page shown (setGroupCards).
  connect(m_pages, &QStackedWidget::currentChanged, this, [this] { updateGroupMask(); });
  connect(m_tabs, &QTabBar::currentChanged, this, [this](int index) {
    if (index >= 0) {
      m_pages->setCurrentIndex(m_tabs->tabData(index).toInt());
      // The shown page's group menus, for UI tests (BUILD, mitcad#375).
      TestSync::singleShot(0, this, [this] { logLayout(); });
    }
  });
  // The places logged again when the ribbon or the tab bar is shown or
  // moves in the window (mitcad#375: the window shows after the start's
  // log, a banner makes room, the title bar takes the tabs), when they
  // changed.
  new PlaceWatcher(this, [this] { logLayoutIfChanged(); });
  // The title bar may take the tab bar (Floating) and outlive the ribbon.
  new PlaceWatcher(m_tabs, [ribbon = QPointer<Ribbon>(this)] {
    if (ribbon) {
      ribbon->logLayoutIfChanged();
    }
  });
}

const QStringList& Ribbon::groups(const QString& tab) {
  // MAKE: 3D Print (mitcad#13). ASSEMBLE: joints (mitcad#55). SHEET METAL:
  // mitcad#210.
  static const QStringList solid = {QStringLiteral("CREATE"),  QStringLiteral("MODIFY"),
                                    QStringLiteral("SHEET METAL"), QStringLiteral("ASSEMBLE"), QStringLiteral("CONSTRUCT"),
                                    QStringLiteral("INSPECT"), QStringLiteral("INSERT"),
                                    QStringLiteral("MAKE"),    QStringLiteral("SELECT")};
  static const QStringList sketch = {QStringLiteral("CREATE"),       QStringLiteral("MODIFY"),
                                     QStringLiteral("CONSTRAINTS"),  QStringLiteral("INSPECT"),
                                     QStringLiteral("INSERT"),       QStringLiteral("SELECT"),
                                     QStringLiteral("FINISH SKETCH")};
  // mitcad#375: placing parts, connecting them, changing them, the lists.
  static const QStringList build = {QStringLiteral("PROFILE"), QStringLiteral("CONNECT"), QStringLiteral("MODIFY"),
                                    QStringLiteral("OUTPUT")};
  if (tab == kBuild) {
    return build;
  }
  return tab == kSketch ? sketch : solid;
}

void Ribbon::addGroupExtras(const QString& tab, const QString& group, const QList<QAction*>& menu,
                            const QList<QAction*>& pinned) {
  Extras& extras = m_extras[key(tab, group)];
  extras.menu += menu;
  extras.pinned += pinned;
}

void Ribbon::build() {
  m_pages->addWidget(createPage(kSolid));
  m_sketchPage = createPage(kSketch);
  m_pages->addWidget(m_sketchPage);
  m_pages->addWidget(createPage(kBuild));
  const int solid = m_tabs->addTab(kSolid);
  m_tabs->setTabData(solid, kSolidPage);
  m_tabs->setTabToolTip(solid, tr("Solid modelling"));
  // BUILD (mitcad#375) beside SOLID; SKETCH comes after them while sketching.
  const int build = m_tabs->addTab(kBuild);
  m_tabs->setTabData(build, kBuildPage);
  m_tabs->setTabToolTip(build, tr("Build: frames of slot profiles with their connectors, panels and linear parts"));
}

int Ribbon::tabOfPage(int page) const {
  for (int i = 0; i < m_tabs->count(); ++i) {
    if (m_tabs->tabData(i).toInt() == page) {
      return i;
    }
  }
  return -1;
}

QWidget* Ribbon::createPage(const QString& tab) {
  auto* page = new QWidget;
  auto* row = new QHBoxLayout(page);
  const bool mac = m_presentation == Presentation::Mac;
  row->setContentsMargins(mac ? QMargins(12, 4, 12, 6) : QMargins(0, 2, 0, 0));
  if (mac) {
    row->setSpacing(8);
  }
  bool first = true;
  QList<GroupInfo>& infos = m_groupsOfPage[page];
  for (const QString& group : groups(tab)) {
    if (!first && !mac) {
      row->addWidget(new GroupSeparator);
    }
    first = false;
    GroupInfo info;
    row->addWidget(createGroup(tab, group, &info));
    if (mac) {
      infos.push_back(info);
    }
  }
  row->addStretch();
  return page;
}

QWidget* Ribbon::createGroup(const QString& tab, const QString& group, GroupInfo* info) {
  const bool mac = m_presentation == Presentation::Mac;
  QWidget* widget = mac ? new CapsuleFrame : new QWidget;
  auto* column = new QVBoxLayout(widget);
  column->setContentsMargins(mac ? QMargins(8, 6, 8, 5) : QMargins(2, 0, 2, 0));
  column->setSpacing(0);
  if (mac) {
    // Every capsule is as tall as one with buttons, so that a group of names
    // only (or a collapsed one) keeps the row aligned.
    widget->setMinimumHeight(6 + kMacButtonSize + kMacCaptionHeight + 5);
  }

  auto* row = new QWidget;
  // The same height in every group, so that the group names line up.
  row->setFixedHeight(mac ? kMacButtonSize : kIconSize + 10);
  auto* buttons = new QHBoxLayout(row);
  buttons->setContentsMargins(0, 0, 0, 0);
  buttons->setSpacing(0);
  QToolButton* menuButton = mac ? new GroupCaption(displayName(group)) : new QToolButton;
  auto* menu = new QMenu(menuButton);
  // The group's commands in registration order; a subgroup's together,
  // where its first command is (CommandDef::subgroup, mitcad#222).
  struct Entry {
    QString subgroup; // empty: a command of its own
    QList<const CommandDef*> defs;
  };
  QList<Entry> entries;
  for (const auto& def : m_registry.commands()) {
    if (def->tab != tab || def->group != group || m_registry.action(def->id) == nullptr) {
      continue;
    }
    const auto same = std::find_if(entries.begin(), entries.end(), [&def](const Entry& entry) {
      return !def->subgroup.isEmpty() && entry.subgroup == def->subgroup;
    });
    if (same != entries.end()) {
      same->defs.append(def.get());
    } else {
      entries.append(Entry{def->subgroup, {def.get()}});
    }
  }
  int pinnedCount = 0;
  for (const Entry& entry : entries) {
    if (entry.defs.size() == 1) {
      // A command of its own, also a subgroup of one (Line, Text, Project).
      const CommandDef* def = entry.defs.first();
      QAction* action = m_registry.action(def->id);
      menu->addAction(action);
      if (def->pinned) {
        QToolButton* button = commandButton(action, mac);
        button->setObjectName(QStringLiteral("ribbon_") + def->id);
        buttons->addWidget(button);
        ++pinnedCount;
      }
      continue;
    }
    // A submenu; its pinned command is the default of a split button whose
    // arrow opens the submenu. Without one (Other Geometry, Patterns) it is
    // in the group's menu only.
    const QString path = key(tab, group) + QLatin1Char('/') + entry.subgroup;
    QMenu* submenu = menu->addMenu(entry.subgroup);
    submenu->setObjectName(QStringLiteral("ribbonSubmenu_") + path);
    const CommandDef* primary = nullptr;
    for (const CommandDef* def : entry.defs) {
      submenu->addAction(m_registry.action(def->id));
      if (def->pinned && primary == nullptr) {
        primary = def;
      }
    }
    submenu->setIcon(m_registry.action((primary != nullptr ? primary : entry.defs.first())->id)->icon());
    connect(submenu, &QMenu::aboutToShow, submenu, [submenu, path] {
      qDebug().noquote() << QStringLiteral("Ribbon menu %1: %2").arg(path, menuEntries(submenu));
    });
    if (mac) {
      mac::showMenuImages(submenu); // also when the split button's arrow opens it alone
    }
    if (primary != nullptr) {
      QAction* action = m_registry.action(primary->id);
      QToolButton* button = commandButton(action, mac);
      button->setObjectName(QStringLiteral("ribbon_") + primary->id);
      button->setMenu(submenu);
      button->setPopupMode(QToolButton::MenuButtonPopup);
      button->setAccessibleDescription(tr("%1; its arrow opens %2").arg(action->text(), entry.subgroup));
      buttons->addWidget(button);
      m_splitButtons.insert(path, button);
      ++pinnedCount;
    }
  }
  connect(menu, &QMenu::aboutToShow, menu, [menu, name = key(tab, group)] {
    qDebug().noquote() << QStringLiteral("Ribbon menu %1: %2").arg(name, menuEntries(menu));
  });
  const Extras extras = m_extras.value(key(tab, group));
  if (!extras.menu.isEmpty()) {
    if (!menu->isEmpty()) {
      menu->addSeparator();
    }
    menu->addActions(extras.menu);
  }
  for (QAction* action : extras.pinned) {
    buttons->addWidget(commandButton(action, mac));
    ++pinnedCount;
  }
  if (menu->isEmpty()) {
    menu->addAction(tr("No commands yet"))->setEnabled(false);
  }
  if (pinnedCount == 0) {
    row->setMinimumWidth(kIconSize + 8);
  }

  if (!mac) {
    menuButton->setText(group + QStringLiteral(" ▾"));
    menuButton->setToolButtonStyle(Qt::ToolButtonTextOnly);
    menuButton->setAutoRaise(true);
    QFont font = menuButton->font();
    font.setPointSizeF(font.pointSizeF() * 0.85);
    menuButton->setFont(font);
  } else {
    // Since macOS 27 AppKit hides the images of menu items unless asked.
    mac::showMenuImages(menu);
  }
  menuButton->setObjectName(QStringLiteral("ribbonMenu_") + key(tab, group));
  menuButton->setPopupMode(QToolButton::InstantPopup);
  menuButton->setFocusPolicy(Qt::NoFocus);
  menuButton->setMenu(menu);
  m_groupButtons.insert(key(tab, group), menuButton);

  // The name is centred in the capsule when there are no buttons above it.
  column->addStretch();
  column->addWidget(row, 0, Qt::AlignHCenter);
  column->addWidget(menuButton, 0, Qt::AlignHCenter);
  column->addStretch();
  if (mac) {
    // A card of its own in the Floating layout's group cards (mitcad#425).
    widget->setObjectName(QStringLiteral("ribbonGroup_") + key(tab, group));
    const int caption = menuButton->sizeHint().width() + 16;
    // The capsule is as wide as its caption at least.
    widget->setMinimumWidth(caption);
    info->capsule = widget;
    info->buttons = row;
    info->collapsible = pinnedCount > 0;
    info->collapsedWidth = caption + 8;
    info->fullWidth = std::max(caption, row->layout()->sizeHint().width() + 16);
    row->setVisible(pinnedCount > 0);
    if (pinnedCount == 0) {
      widget->setMinimumWidth(info->collapsedWidth);
    }
  }
  return widget;
}

QSize Ribbon::sizeHint() const {
  const QSize content = QWidget::sizeHint();
  if (m_presentation == Presentation::Mac) {
    return content;
  }
  // The tabs' width at least; the toolbar gives it the rest (Expanding).
  return QSize(std::max(m_tabs->sizeHint().width() + 8, 240), content.height());
}

QSize Ribbon::minimumSizeHint() const {
  const QSize content = QWidget::minimumSizeHint();
  if (m_presentation == Presentation::Mac) {
    return content;
  }
  return QSize(std::max(m_tabs->sizeHint().width() + 8, 160), content.height());
}

void Ribbon::resizeEvent(QResizeEvent* event) {
  QWidget::resizeEvent(event);
  fitGroups();
  updateGroupMask();
}

void Ribbon::showEvent(QShowEvent* event) {
  QWidget::showEvent(event);
  fitGroups();
}

void Ribbon::fitGroups() {
  if (m_presentation != Presentation::Mac) {
    return;
  }
  // The page's margins (none as group cards: the layout keeps the margin).
  const int pageMargins = m_groupCards ? 0 : 24;
  constexpr int kSpacing = 8;
  for (auto it = m_groupsOfPage.begin(); it != m_groupsOfPage.end(); ++it) {
    QList<GroupInfo>& infos = it.value();
    int total = pageMargins + kSpacing * std::max<int>(0, static_cast<int>(infos.size()) - 1);
    for (const GroupInfo& info : infos) {
      total += info.collapsible ? info.fullWidth : info.collapsedWidth;
    }
    // Collapse from the right while the groups do not fit.
    std::vector<bool> collapsed(static_cast<std::size_t>(infos.size()), false);
    for (int i = static_cast<int>(infos.size()) - 1; i >= 0 && total > width(); --i) {
      if (infos[i].collapsible) {
        total -= infos[i].fullWidth - infos[i].collapsedWidth;
        collapsed[static_cast<std::size_t>(i)] = true;
      }
    }
    for (int i = 0; i < infos.size(); ++i) {
      const GroupInfo& info = infos[i];
      if (info.collapsible) {
        info.buttons->setVisible(!collapsed[static_cast<std::size_t>(i)]);
      }
    }
  }
}

void Ribbon::setGroupCards(bool cards) {
  if (m_presentation != Presentation::Mac || cards == m_groupCards) {
    return;
  }
  m_groupCards = cards;
  // The cards start at the ribbon's edges (the layout keeps the margin to
  // the view's), without the hairline's row. The ribbon is placed, not laid
  // out: its layout does not make it as wide as all the groups, which clip
  // and collapse at its right instead (fitGroups).
  layout()->setContentsMargins(0, 0, 0, cards ? 0 : 1);
  layout()->setSizeConstraint(cards ? QLayout::SetNoConstraint : QLayout::SetDefaultConstraint);
  if (cards) {
    setMinimumSize(0, 0);
  }
  for (auto it = m_groupsOfPage.begin(); it != m_groupsOfPage.end(); ++it) {
    if (QLayout* row = it.key()->layout()) {
      row->setContentsMargins(cards ? QMargins(0, 0, 0, 0) : QMargins(12, 4, 12, 6));
    }
    for (const GroupInfo& info : std::as_const(it.value())) {
      if (auto* frame = qobject_cast<CapsuleFrame*>(info.capsule)) {
        frame->setCardMaterial(cards);
      }
      if (cards) {
        info.capsule->installEventFilter(this);
      } else {
        info.capsule->removeEventFilter(this);
      }
    }
  }
  if (cards) {
    updateGroupMask();
  } else {
    clearMask();
  }
  fitGroups();
  update();
}

std::vector<std::pair<QString, QRect>> Ribbon::groupCards() const {
  std::vector<std::pair<QString, QRect>> cards;
  QWidget* page = m_pages->currentWidget();
  const auto infos = m_groupsOfPage.constFind(page);
  if (page == nullptr || infos == m_groupsOfPage.cend()) {
    return cards;
  }
  for (const GroupInfo& info : *infos) {
    if (info.capsule == nullptr || info.capsule->isHidden()) {
      continue;
    }
    const QRect rect = QRect(info.capsule->mapTo(this, QPoint(0, 0)), info.capsule->size()) & this->rect();
    if (rect.isEmpty()) {
      continue;
    }
    QString name = info.capsule->objectName();
    name.replace(QLatin1Char(' '), QLatin1Char('_'));
    cards.emplace_back(name, rect);
  }
  return cards;
}

void Ribbon::updateGroupMask() {
  if (!m_groupCards) {
    return;
  }
  QRegion region;
  for (const auto& card : groupCards()) {
    region += card.second;
  }
  if (region.isEmpty()) {
    // An empty mask is none at all: the whole ribbon would take the mouse.
    region = QRegion(0, 0, 1, 1);
  }
  if (region != mask()) {
    setMask(region);
    emit groupCardsChanged();
  }
}

bool Ribbon::eventFilter(QObject* watched, QEvent* event) {
  if (m_groupCards) {
    switch (event->type()) {
    case QEvent::Move:
    case QEvent::Resize:
    case QEvent::Show:
    case QEvent::Hide:
      updateGroupMask();
      break;
    default:
      break;
    }
  }
  return QWidget::eventFilter(watched, event);
}

void Ribbon::changeEvent(QEvent* event) {
  QWidget::changeEvent(event);
  if (event->type() == QEvent::ApplicationPaletteChange && m_tabs != nullptr) {
    if (const int sketch = tabOfPage(kSketchPage); sketch >= 0) {
      m_tabs->setTabTextColor(sketch, accentColor(palette()));
    }
  }
}

void Ribbon::paintEvent(QPaintEvent* event) {
  if (m_presentation != Presentation::Mac) {
    QWidget::paintEvent(event);
    return;
  }
  if (m_groupCards) {
    return; // the cards paint themselves; the view shows between them
  }
  // The hairline between the toolbar and the view.
  QPainter painter(this);
  painter.fillRect(QRect(0, height() - 1, width(), 1), cardBorderColor(palette()));
}

void Ribbon::setSketchMode(bool sketching) {
  const int shown = tabOfPage(kSketchPage);
  if (sketching && shown < 0) {
    // Back to the tab the sketch started from afterwards (SOLID or BUILD).
    m_pageBeforeSketch = m_tabs->tabData(m_tabs->currentIndex()).toInt();
    const int index = m_tabs->addTab(kSketch);
    m_tabs->setTabData(index, kSketchPage);
    m_tabs->setTabTextColor(index, accentColor(palette()));
    m_tabs->setTabToolTip(index, tr("Sketch: the tools of the sketch being edited"));
    m_tabs->setCurrentIndex(index);
    emit sketchModeChanged(true);
  } else if (!sketching && shown >= 0) {
    m_tabs->removeTab(shown);
    const int back = tabOfPage(m_pageBeforeSketch);
    m_tabs->setCurrentIndex(back >= 0 ? back : tabOfPage(kSolidPage));
    emit sketchModeChanged(false);
  }
}

QStringList Ribbon::layoutLines() const {
  QStringList lines;
  // The workspace tabs (mitcad#375: a click on BUILD), "Ribbon tab BUILD at
  // x,y", wherever the tab bar is (the ribbon's, or the title bar's).
  if (m_tabs->isVisible()) {
    for (int i = 0; i < m_tabs->count(); ++i) {
      const QPoint center = m_tabs->mapTo(m_tabs->window(), m_tabs->tabRect(i).center());
      lines << QStringLiteral("Ribbon tab %1 at %2,%3").arg(m_tabs->tabText(i)).arg(center.x()).arg(center.y());
    }
  }
  for (auto it = m_groupButtons.cbegin(); it != m_groupButtons.cend(); ++it) {
    const QWidget* button = it.value();
    if (!button->isVisible()) {
      continue;
    }
    const QPoint center = button->mapTo(window(), button->rect().center());
    lines << QStringLiteral("Ribbon %1 at %2,%3").arg(it.key()).arg(center.x()).arg(center.y());
  }
  // The split buttons' two parts (mitcad#222): "Ribbon split
  // SKETCH/CREATE/Circles at x,y" runs the default, "... arrow at x,y"
  // opens the submenu.
  for (auto it = m_splitButtons.cbegin(); it != m_splitButtons.cend(); ++it) {
    const QToolButton* button = it.value();
    if (!button->isVisible()) {
      continue;
    }
    const int y = button->height() / 2;
    const QPoint direct = button->mapTo(window(), QPoint((button->width() - kSplitArrowWidth) / 2, y));
    const QPoint arrow = button->mapTo(window(), QPoint(button->width() - kSplitArrowWidth / 2 - 1, y));
    lines << QStringLiteral("Ribbon split %1 at %2,%3").arg(it.key()).arg(direct.x()).arg(direct.y());
    lines << QStringLiteral("Ribbon split %1 arrow at %2,%3").arg(it.key()).arg(arrow.x()).arg(arrow.y());
  }
  return lines;
}

void Ribbon::logLayout() const {
  m_loggedLayout = layoutLines();
  for (const QString& line : std::as_const(m_loggedLayout)) {
    qDebug().noquote() << line;
  }
}

void Ribbon::logLayoutIfChanged() const {
  if (layoutLines() != m_loggedLayout) {
    logLayout();
  }
}

} // namespace mitcad
