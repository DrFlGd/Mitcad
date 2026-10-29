// SPDX-License-Identifier: MIT
#pragma once

#include <utility>
#include <vector>

#include <QHash>
#include <QList>
#include <QRect>
#include <QString>
#include <QStringList>
#include <QToolButton>
#include <QWidget>

class QAction;
class QEvent;
class QMenu;
class QMouseEvent;
class QPaintEvent;
class QResizeEvent;
class QShowEvent;
class QStackedWidget;
class QTabBar;

namespace mitcad {

class CommandRegistry;

// The toolbar: workspace tabs (SOLID, BUILD, and SKETCH while sketching),
// each with groups (CREATE, MODIFY, ...) of command buttons; the group's
// name opens a menu of all its commands, a subgroup's (CommandDef::subgroup)
// as a submenu whose pinned command is a split button (mitcad#222), with
// separators between its sections (CommandDef::section, mitcad#579).
//
// Presentation::Mac is the Floating chrome's: the tabs are a segmented control
// that the title bar hosts (tabBar()), the groups are rounded capsules and the
// buttons are painted without bevels. The objects, their names and the logged
// positions are the same in both.
class Ribbon : public QWidget {
  Q_OBJECT

public:
  enum class Presentation { Classic, Mac };

  explicit Ribbon(CommandRegistry& registry, Presentation presentation = Presentation::Classic,
                  QWidget* parent = nullptr);

  // Entries that are not commands (the SELECT group's filters): `menu`
  // entries go after the group's commands, `pinned` ones are buttons.
  void addGroupExtras(const QString& tab, const QString& group, const QList<QAction*>& menu,
                      const QList<QAction*>& pinned = {});
  // Builds the tabs from the registered commands.
  void build();
  // Shows the SKETCH tab and switches to it, or hides it.
  void setSketchMode(bool sketching);
  // Logs where the group menus are, for UI tests.
  void logLayout() const;
  // As logLayout, only when the places changed since they were logged
  // (the ribbon or its tab bar shown, moved, resized).
  void logLayoutIfChanged() const;

  static const QStringList& groups(const QString& tab);
  // The same full/collapsed group geometry for other native presentations.
  std::pair<int, int> groupWidths(const QString& tab, const QString& group) const;

  Presentation presentation() const { return m_presentation; }
  // The workspace tabs. In the Mac presentation the bar is not part of the
  // ribbon's layout: whoever hosts it (the title bar) places it.
  QTabBar* tabBar() const { return m_tabs; }

  // The Floating layout's group cards (mitcad#425, floatingTopCards(): Windows
  // and Linux): the ribbon floats over the 3D view, each group a card of its
  // own in the cards' material (CapsuleFrame::setCardMaterial). The ribbon
  // paints nothing else and takes the mouse only on the shown page's cards
  // (a mask), so that a click, drag or wheel between and beside them reaches
  // the view. Mac presentation only; call it after build().
  void setGroupCards(bool cards);
  bool hasGroupCards() const { return m_groupCards; }
  // The shown page's group cards in sight: their names
  // ("ribbonGroup_SOLID/SHEET_METAL": spaces as underscores) and rectangles
  // in the ribbon's coordinates, left to right (clipped to the ribbon).
  std::vector<std::pair<QString, QRect>> groupCards() const;

signals:
  // The SKETCH tab was shown (true) or hidden.
  void sketchModeChanged(bool sketching);
  // setGroupCards: the shown group cards moved, resized, came or went.
  void groupCardsChanged();

protected:
  // The SKETCH tab's colour follows the palette.
  void changeEvent(QEvent* event) override;
  void paintEvent(QPaintEvent* event) override;
  void resizeEvent(QResizeEvent* event) override;
  // The group cards' places, for the mask (setGroupCards).
  bool eventFilter(QObject* watched, QEvent* event) override;

public:
  // Classic (Docked): a width that always fits the toolbar (it expands to
  // what is there); the height of its content.
  QSize sizeHint() const override;
  QSize minimumSizeHint() const override;

protected:
  void showEvent(QShowEvent* event) override;

private:
  // A group and the widths needed to collapse its complete pinned row.
  struct GroupInfo {
    QWidget* capsule = nullptr;
    QWidget* buttons = nullptr; // the row of pinned commands
    int fullWidth = 0;
    int collapsedWidth = 0;
    bool collapsible = false;   // it has pinned commands to hide
  };
  struct Extras {
    QList<QAction*> menu;
    QList<QAction*> pinned;
  };

  QWidget* createPage(const QString& tab);
  QWidget* createGroup(const QString& tab, const QString& group, GroupInfo* info);
  // The tab that shows a page (SOLID 0, SKETCH 1, BUILD 2), or -1.
  int tabOfPage(int page) const;
  // The places logLayout logs: the tabs, the group menus, the split buttons.
  QStringList layoutLines() const;
  // When groups do not fit, collapse pinned rows from the right. Complete
  // captions still open the same menus; Classic can scroll those captions.
  void fitGroups();
  // setGroupCards: the mask of the shown page's group cards.
  void updateGroupMask();

  CommandRegistry& m_registry;
  Presentation m_presentation;
  QTabBar* m_tabs = nullptr;
  QStackedWidget* m_pages = nullptr;
  QWidget* m_sketchPage = nullptr;
  int m_pageBeforeSketch = 0; // the page Finish Sketch goes back to
  mutable QStringList m_loggedLayout; // the places logged last
  QHash<QString, Extras> m_extras;   // by "tab/group"
  QHash<QString, QWidget*> m_groupButtons; // by "tab/group", for logLayout
  // The subgroups' split buttons (mitcad#222), by "tab/group/subgroup".
  QHash<QString, QToolButton*> m_splitButtons;
  QHash<QWidget*, QList<GroupInfo>> m_groupsOfPage;
  QHash<QWidget*, QWidget*> m_classicPages;
  bool m_groupCards = false; // setGroupCards
};

// A command button of the Mac presentation (also the title bar's Undo and
// Redo): no bevel, a rounded highlight on hover, a darker one pressed, the
// accent tint when checked. The default action gives icon, tooltip and state.
// With QToolButton::MenuButtonPopup it is a split button (mitcad#222): a
// chevron part at the right opens its menu.
class RibbonButton : public QToolButton {
  Q_OBJECT

public:
  explicit RibbonButton(QAction* action, int iconSize, QWidget* parent = nullptr);

  QSize sizeHint() const override;

protected:
  void paintEvent(QPaintEvent* event) override;
  void changeEvent(QEvent* event) override;
  void mousePressEvent(QMouseEvent* event) override;

private:
  int m_iconSize;
};

// A rounded panel in the palette's material (the group capsules, the Undo and
// Redo pair): fill, hairline border and a faint shadow below.
class CapsuleFrame : public QWidget {
  Q_OBJECT

public:
  // A radius below zero is half the height.
  explicit CapsuleFrame(int radius = 12, QWidget* parent = nullptr);

  // The Floating chrome's card material (cardFillColor, a hairline of
  // cardBorderColor), for a group that is a card of its own over the 3D view
  // (Ribbon::setGroupCards, mitcad#425).
  void setCardMaterial(bool card);

protected:
  void paintEvent(QPaintEvent* event) override;

private:
  int m_radius;
  bool m_cardMaterial = false;
};

// "CREATE" -> "Create", "FINISH SKETCH" -> "Finish Sketch": the names every
// presentation shows. The internal names stay in capitals.
QString displayName(const QString& name);

} // namespace mitcad
