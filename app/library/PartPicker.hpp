// SPDX-License-Identifier: MIT
#pragma once

// The part picker (mitcad#418; app/COMMANDS.md, "Part picker"): the
// library's parts as a grid of pictures, each tile with the part's
// preview (`library_preview`, read as tiles come into sight), its name
// and main sizes, a star for the user's favourites and, for the parts the
// open design uses, how many times. On the left the groups: Favourites,
// In this design (when it has parts), All parts, then the library's
// categories (starred groups first); above the grid a search, the slot
// system (BUILD parts) and the tiles' size. Hovering a tile shows a larger
// picture with the supplier, the part number, the library and the licence.
//
// - PartGrid: the groups, filters and grid as a widget; Insert from
//   Library's dialog (LibraryBrowser) has one in place of its list.
// - PartPicker: a floating, non-modal window with a PartGrid that the
//   part-choosing commands open (BUILD's Profile, Panel, Linear Part and
//   Connector): a click on a tile chooses the part in the running command;
//   the window closes with the command unless it is pinned, and a pinned
//   one follows the next command that chooses parts. Its place, size and
//   tile size are kept in the settings (group "partPicker").
//
// The favourites and the order are PartFavourites' (PartFavourites.hpp),
// the design's parts usedLibraryParts' (UsedParts.hpp).

#include <functional>

#include <QAbstractListModel>
#include <QDialog>
#include <QHash>
#include <QImage>
#include <QJsonObject>
#include <QList>
#include <QPixmap>
#include <QPointer>
#include <QSet>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QVector>
#include <QWidget>

#include "UsedParts.hpp"

class QComboBox;
class QFrame;
class QLabel;
class QLineEdit;
class QListView;
class QListWidget;
class QListWidgetItem;
class QSlider;
class QToolButton;

namespace mitcad {

namespace build {
struct Part;
}

// A part the picker shows.
struct PickerPart {
  QString key;          // the caller's value (BUILD: build::Part::key); unique in a list
  QString library;      // the library's id
  QString libraryName;  // for people
  QString url;          // where the library was fetched from (empty: the id is enough)
  QString rev;          // the library's version the part comes from (empty: the newest)
  QString version;      // "v1.0.0 (3f9a2c1)"
  QString component;    // the component's id in the library
  QString config;       // a configuration row, empty for none
  QString name;
  QString category;     // the library's category: the group
  QString standard;
  QString description;
  QString designation;
  QStringList keywords;
  QString supplier;
  QString partNumber;
  QString license;      // SPDX id, empty for none
  QString sizes;        // the main sizes under the name ("30x30", "M5x16")
  QString system;       // the slot system(s) of a BUILD part ("B8 30")
  bool preview = true;  // the library has a picture of it
  bool rowPreview = false; // `config` has a picture of its own (library_preview with it, mitcad#484)
  bool defaultRow = false; // `config` is its component's default row (an old star marks it, mitcad#491)
  QString refused;      // not choosable: why (greyed, the reason in the tooltip)
  // An index's library that is not fetched (Insert from Library, Community
  // Library): a tile of its own in the group "Libraries to get".
  bool libraryEntry = false;
};

// A component of `library_search` or `library_build_candidates` (with
// `build`: its section and system as the sizes); key "<library>|<component>".
PickerPart pickerPart(const QJsonObject& component);
// An answer of `library_search` or `library_build_candidates`: the
// components, then the refused ones (`refused`: greyed with their reason).
QList<PickerPart> pickerParts(const QJsonObject& answer);
// A BUILD picker's part (one row of a component), keyed by its key.
PickerPart pickerPart(const build::Part& part);
// BUILD pickers' parts with their categories, licences and pictures from
// one `library_search`.
QList<PickerPart> pickerParts(const QVector<build::Part>& parts);
// `library_build_candidates` with these fields (`role` with `system`, or
// `junction`; commands.md, "Library commands"), as the picker's parts.
QList<PickerPart> buildCandidates(const QJsonObject& fields);

// The grid's tiles: the parts the group and filters leave, in order.
class PartGridModel : public QAbstractListModel {
  Q_OBJECT

public:
  static constexpr int kKeyRole = Qt::UserRole;
  static constexpr int kImageRole = Qt::UserRole + 1;
  static constexpr int kCountRole = Qt::UserRole + 2;
  static constexpr int kStarRole = Qt::UserRole + 3;
  static constexpr int kRefusedRole = Qt::UserRole + 4;
  static constexpr int kSizesRole = Qt::UserRole + 5;

  explicit PartGridModel(QObject* parent = nullptr);

  void setShown(const QList<PickerPart>& shownParts, const QHash<QString, int>& counts);
  const PickerPart* part(int row) const;
  int rowOf(const QString& key) const;
  int rowCount(const QModelIndex& parent = QModelIndex()) const override;
  QVariant data(const QModelIndex& index, int role) const override;
  // The part's picture, read now when it is not read yet (the hover's).
  QImage image(const PickerPart& shownPart) const;
  // The tiles' size: the pictures are scaled to it.
  void setTileSize(int size);

private:
  void loadSome();

  QList<PickerPart> m_shown;
  QHash<QString, int> m_counts;
  // The parts whose pictures are wanted, in the order the view asked.
  mutable QStringList m_wanted;
  mutable QHash<QString, PickerPart> m_waiting;
  mutable QHash<QString, QPixmap> m_scaled;
  int m_tileSize = 96;
  mutable QTimer m_loader;
};

class PartGrid : public QWidget {
  Q_OBJECT

public:
  // The groups that are not the library's categories.
  static const QString kFavourites;
  static const QString kInDesign;
  static const QString kAll;
  static const QString kLibraries;
  static const QString kOther;

  explicit PartGrid(QWidget* parent = nullptr);

  // The parts to choose from (the group, the filters and the current part
  // are kept while they are still there). Signals nothing.
  void setParts(const QList<PickerPart>& parts);
  // The parts of another command (a pinned picker follows it): as setParts,
  // but a group none of them is in goes back to All (within one list, a
  // search or a filter that empties the group keeps it).
  void setOtherParts(const QList<PickerPart>& parts);
  const QList<PickerPart>& allParts() const { return m_parts; }
  // The design's library parts (In this design, the tiles' xN).
  void setUsedParts(const QList<UsedPart>& used);
  // The tile of `key` current (empty: none); `notify`: currentChanged.
  void setCurrent(const QString& key, bool notify = false);
  // The tile of `key` current and scrolled into sight (mitcad#452): when
  // the group or the search leaves it out, All parts without the search.
  // Signals nothing; false when no tile shows it.
  bool reveal(const QString& key);
  // The first tile shown current (none when nothing is shown).
  void selectFirst(bool notify = false);
  QString currentKey() const { return m_current; }
  const PickerPart* currentPart() const;
  // The group shown (kFavourites, kInDesign, kAll, kLibraries, kOther or a
  // category id); an unknown one shows All parts.
  void showGroup(const QString& group);
  QString groupShown() const { return m_group; }
  void setSystem(const QString& system);
  // External: the search text is the caller's to search with
  // (searchChanged), the grid does not filter by it; else the grid keeps
  // the parts whose fields have every word.
  void setExternalSearch(bool external) { m_externalSearch = external; }
  QLineEdit* searchField() const { return m_search; }
  // A widget beside the search (the browser's licence filter).
  void addFilter(QWidget* widget);
  QListView* view() const { return m_view; }
  // The keys of the tiles shown, in order.
  QStringList shownKeys() const;
  // Stars the current part, or turns its star off.
  void toggleStar();
  // Stars the current group (a category), or turns its star off.
  void toggleGroupStar();
  // Logs where the search, Star, the groups and the tiles in sight are,
  // for UI tests: "Part picker place search at <at(widget, point)>",
  // "Part picker group row <group> at ...", "Part picker tile
  // <library>/<component> at ..." (PartPicker gives `at`).
  void logPlaces(const std::function<QString(const QWidget*, const QPoint&)>& at) const;

signals:
  // The tiles were made again (a group, a search, the system, a star).
  void tilesChanged();
  void searchChanged(const QString& text);
  // Another tile became current (a click, the arrow keys).
  void currentChanged(const QString& key);
  // A tile clicked (not on its star).
  void clicked(const QString& key);
  // A tile double-clicked, or Return on it.
  void activated(const QString& key);

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void hideEvent(QHideEvent* event) override;

private:
  void fillGroups();
  void fillSystems();
  // The tiles again from the parts, the group and the filters (logged).
  // The current part stays when it is still shown; else, with `notify`,
  // the first tile becomes current (currentChanged), without it none.
  void refill(bool notify);
  void star(const QString& key);
  void starGroup(const QString& group);
  void step(int by);
  void setTileSize(int size);
  void showHover(const QModelIndex& index, const QPoint& global);
  void hideHover();
  QString groupName(const QString& group) const;
  bool passes(const PickerPart& part) const;
  QString countKey(const PickerPart& part) const;

  QList<PickerPart> m_parts;
  QHash<QString, int> m_counts;  // "<library>|<component>" -> uses
  QString m_group;
  QString m_current;
  bool m_externalSearch = false;
  bool m_filling = false;
  // A click on a tile's star: not a click on the tile.
  bool m_starHit = false;
  int m_tile = 96;
  QListWidget* m_groups = nullptr;
  QLineEdit* m_search = nullptr;
  QComboBox* m_system = nullptr;
  QLabel* m_systemLabel = nullptr;
  QSlider* m_size = nullptr;
  QWidget* m_filters = nullptr;
  QListView* m_view = nullptr;
  PartGridModel* m_model = nullptr;
  QToolButton* m_star = nullptr;
  QFrame* m_hover = nullptr;
  QLabel* m_hoverImage = nullptr;
  QLabel* m_hoverText = nullptr;
  QString m_hoverKey;
};

// What a part-choosing command asks the picker for.
struct PickerRequest {
  QString command;           // the command's id ("build.profile"): the picker closes with it
  QString title;             // what is chosen, in the window's title ("Profile")
  QList<PickerPart> parts;   // what the command takes; refused ones greyed with their reason
  QString current;           // the key chosen now
  QString system;            // the system filter to start with (empty: all)
  // The command's input the choice goes into, when not its part (a Timber
  // frame's fastener, mitcad#497); empty for the part.
  QString input;
};

class PartPicker : public QDialog {
  Q_OBJECT

public:
  // The picker of a main window, made the first time it is asked for.
  static PartPicker* of(QWidget* window);
  explicit PartPicker(QWidget* parent = nullptr);
  ~PartPicker() override;

  // Shows the request's parts, without taking the focus from the view. A
  // pinned picker that is open follows the new command.
  void ask(const PickerRequest& request);
  // New parts for the same command (after a placement, another system).
  void setParts(const QList<PickerPart>& parts, const QString& current = QString());
  // The part chosen in the command changed elsewhere (its panel).
  void setCurrent(const QString& key);
  // The command ended: the picker closes unless it is pinned.
  void commandEnded(const QString& command);
  QString command() const { return m_request.command; }
  // The input the choice goes into (PickerRequest::input; empty: the part).
  QString input() const { return m_request.input; }
  bool isPinned() const;
  void setPinned(bool on);
  // How In this design is read: when the picker shows and on
  // designChanged() while it is open.
  void setDesignParts(std::function<QList<UsedPart>()> read);
  // The design changed (a part placed or removed): In this design again.
  void designChanged();
  PartGrid* grid() const { return m_grid; }

signals:
  // A part chosen with a click (or Return) for `command`.
  void chosen(const PickerPart& part, const QString& command);

protected:
  void showEvent(QShowEvent* event) override;
  void hideEvent(QHideEvent* event) override;
  void moveEvent(QMoveEvent* event) override;
  void resizeEvent(QResizeEvent* event) override;

private:
  void choose(const QString& key);
  void readDesign();
  void saveGeometryLater();
  // Logs where the pin and the grid's places are once laid out, for UI
  // tests (mitcad#418): in the main window's coordinates, as the panel's
  // places, and in the picker's own after "; in the picker".
  void logPlacesLater();
  void logPlaces() const;

  PartGrid* m_grid = nullptr;
  QToolButton* m_pin = nullptr;
  PickerRequest m_request;
  std::function<QList<UsedPart>()> m_readDesign;
  bool m_designStale = true;
  bool m_restored = false;
  QTimer m_saveTimer;
  QTimer m_placesTimer;
};

} // namespace mitcad
