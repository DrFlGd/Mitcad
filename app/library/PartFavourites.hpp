// SPDX-License-Identifier: MIT
#pragma once

// The part picker's stars (mitcad#418): the user's favourite parts and
// groups on this machine, and the order the picker shows parts and groups
// in. A part's star is keyed by its library and component id, so it
// survives library updates; a part gone from the library is dropped from
// Favourites by the picker, its star kept.

#include <QList>
#include <QObject>
#include <QPair>
#include <QSet>
#include <QString>
#include <QStringList>

namespace mitcad {

// The user's favourite parts and groups (mitcad#418), kept in the app's
// settings (QSettings group "partPicker"), keyed by library and component id.
class PartFavourites : public QObject {
  Q_OBJECT

public:
  static PartFavourites& instance();
  // Reads the stars from the settings; the picker uses instance(), tests
  // read the settings again with one of their own.
  explicit PartFavourites(QObject* parent = nullptr);

  bool starred(const QString& library, const QString& component) const;
  void setStarred(const QString& library, const QString& component, bool on);
  bool groupStarred(const QString& group) const;
  void setGroupStarred(const QString& group, bool on);
  // Every starred part as {library, component}.
  QList<QPair<QString, QString>> starredParts() const;

signals:
  void changed();

private:
  void save() const;

  // In the order they were starred.
  QList<QPair<QString, QString>> m_parts;
  QStringList m_groups;
  QSet<QPair<QString, QString>> m_partSet;
};

// One picker item for ordering.
struct OrderedPart {
  QString library;
  QString component;
  QString group;
};

// The order the picker shows (stable: the given order within each class):
// starred parts first, then the rest. With inFavourites, the parts of
// starred groups come first among the starred ones.
QList<int> pickerOrder(const QList<OrderedPart>& parts, bool inFavourites);
// Groups in the order shown: starred groups first, then the rest, each in
// the given order.
QStringList groupOrder(const QStringList& groups);

} // namespace mitcad
