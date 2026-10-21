// SPDX-License-Identifier: MIT
#pragma once

// The part picker's stars (mitcad#418): the user's favourite parts and
// groups on this machine, and the order the picker shows parts and groups
// in. A part's star is keyed by its library and component id, and a tile
// of one configuration row by its row too (mitcad#491: every row is a tile
// of its own since mitcad#484), so it survives library updates; a part
// gone from the library is dropped from Favourites by the picker, its star
// kept.

#include <QList>
#include <QObject>
#include <QSet>
#include <QString>
#include <QStringList>

namespace mitcad {

// A starred part: its library, its component and, for a tile of one
// configuration row (mitcad#491), the row.
struct PartStar {
  QString library;
  QString component;
  QString row;  // empty: the part without a row

  bool operator==(const PartStar& other) const {
    return library == other.library && component == other.component && row == other.row;
  }
  bool operator!=(const PartStar& other) const { return !(*this == other); }
};

// The user's favourite parts and groups (mitcad#418), kept in the app's
// settings (QSettings group "partPicker"), keyed by library, component and
// configuration row.
class PartFavourites : public QObject {
  Q_OBJECT

public:
  static PartFavourites& instance();
  // Reads the stars from the settings; the picker uses instance(), tests
  // read the settings again with one of their own.
  explicit PartFavourites(QObject* parent = nullptr);

  // A part without configuration rows.
  bool starred(const QString& library, const QString& component) const;
  void setStarred(const QString& library, const QString& component, bool on);
  // A tile of one configuration row (`row`; empty: as above). A star kept
  // without a row (starred before mitcad#491, or on a tile without rows)
  // marks only the component's default row (`defaultRow`: `row` is it), so
  // a star of a whole family stars one size; turning that row's star off
  // turns it off too.
  bool starred(const QString& library, const QString& component, const QString& row, bool defaultRow) const;
  void setStarred(const QString& library, const QString& component, const QString& row, bool defaultRow,
                  bool on);
  bool groupStarred(const QString& group) const;
  void setGroupStarred(const QString& group, bool on);
  // Every star, in the order they were starred.
  QList<PartStar> starredParts() const;

signals:
  void changed();

private:
  void save() const;
  bool contains(const PartStar& star) const;

  // In the order they were starred.
  QList<PartStar> m_parts;
  QStringList m_groups;
  // The stars' keys (starKey in the .cpp).
  QSet<QString> m_partSet;
};

// One picker item for ordering.
struct OrderedPart {
  QString library;
  QString component;
  QString group;
  QString row;              // the configuration row, empty for none (mitcad#491)
  bool defaultRow = false;  // `row` is its component's default row
};

// The order the picker shows (stable: the given order within each class):
// starred parts first, then the rest. With inFavourites, the parts of
// starred groups come first among the starred ones.
QList<int> pickerOrder(const QList<OrderedPart>& parts, bool inFavourites);
// Groups in the order shown: starred groups first, then the rest, each in
// the given order.
QStringList groupOrder(const QStringList& groups);

} // namespace mitcad
