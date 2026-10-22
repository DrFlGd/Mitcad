// SPDX-License-Identifier: MIT
#include "PartFavourites.hpp"

#include <algorithm>
#include <array>

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QSettings>

namespace mitcad {
namespace {

// The settings: JSON arrays, as the libraries' sources (files/Libraries.cpp),
// so that any id keeps as it is in the registry and in INI files. A part's
// entry is {library, component} and, for a configuration row (mitcad#491),
// `row`.
const QString kPartsKey = QStringLiteral("partPicker/starredParts");
const QString kGroupsKey = QStringLiteral("partPicker/starredGroups");

// "<library>|<component>", "<library>|<component>|<row>" for a row: a
// star's key (the parts of a key kept apart by a character no id has).
QString starKey(const PartStar& star) {
  QString key = star.library + QChar(0x1f) + star.component;
  if (!star.row.isEmpty()) {
    key += QChar(0x1f) + star.row;
  }
  return key;
}

} // namespace

PartFavourites& PartFavourites::instance() {
  static PartFavourites favourites;
  return favourites;
}

PartFavourites::PartFavourites(QObject* parent) : QObject(parent) {
  const QSettings store;
  const QJsonArray parts = QJsonDocument::fromJson(store.value(kPartsKey).toByteArray()).array();
  for (const QJsonValue& value : parts) {
    const QJsonObject part = value.toObject();
    const PartStar star{part.value(QStringLiteral("library")).toString(),
                        part.value(QStringLiteral("component")).toString(),
                        part.value(QStringLiteral("row")).toString()};
    if (!star.library.isEmpty() && !star.component.isEmpty() && !contains(star)) {
      m_parts.append(star);
      m_partSet.insert(starKey(star));
    }
  }
  const QJsonArray groups = QJsonDocument::fromJson(store.value(kGroupsKey).toByteArray()).array();
  for (const QJsonValue& value : groups) {
    const QString group = value.toString();
    if (!group.isEmpty() && !m_groups.contains(group)) {
      m_groups.append(group);
    }
  }
}

bool PartFavourites::contains(const PartStar& star) const { return m_partSet.contains(starKey(star)); }

bool PartFavourites::starred(const QString& library, const QString& component) const {
  return starred(library, component, QString(), false);
}

void PartFavourites::setStarred(const QString& library, const QString& component, bool on) {
  setStarred(library, component, QString(), false, on);
}

bool PartFavourites::starred(const QString& library, const QString& component, const QString& row,
                             bool defaultRow) const {
  if (library.isEmpty() || component.isEmpty()) {
    return false;
  }
  if (contains({library, component, row})) {
    return true;
  }
  // A star without a row marks the default row only (mitcad#491).
  return !row.isEmpty() && defaultRow && contains({library, component, QString()});
}

void PartFavourites::setStarred(const QString& library, const QString& component, const QString& row,
                                bool defaultRow, bool on) {
  if (library.isEmpty() || component.isEmpty() || starred(library, component, row, defaultRow) == on) {
    return;
  }
  const PartStar star{library, component, row};
  if (on) {
    m_parts.append(star);
    m_partSet.insert(starKey(star));
  } else {
    m_parts.removeAll(star);
    m_partSet.remove(starKey(star));
    if (!row.isEmpty() && defaultRow) {
      // The star without a row that marked it.
      const PartStar whole{library, component, QString()};
      m_parts.removeAll(whole);
      m_partSet.remove(starKey(whole));
    }
  }
  save();
  emit changed();
}

bool PartFavourites::groupStarred(const QString& group) const { return m_groups.contains(group); }

void PartFavourites::setGroupStarred(const QString& group, bool on) {
  if (group.isEmpty() || m_groups.contains(group) == on) {
    return;
  }
  if (on) {
    m_groups.append(group);
  } else {
    m_groups.removeAll(group);
  }
  save();
  emit changed();
}

void PartFavourites::setGroupsStarred(const QStringList& groups, bool on) {
  bool changes = false;
  for (const QString& group : groups) {
    if (group.isEmpty() || m_groups.contains(group) == on) {
      continue;
    }
    if (on) {
      m_groups.append(group);
    } else {
      m_groups.removeAll(group);
    }
    changes = true;
  }
  if (changes) {
    save();
    emit changed();
  }
}

QList<PartStar> PartFavourites::starredParts() const { return m_parts; }

void PartFavourites::save() const {
  QJsonArray parts;
  for (const PartStar& part : m_parts) {
    QJsonObject entry{{QStringLiteral("library"), part.library}, {QStringLiteral("component"), part.component}};
    if (!part.row.isEmpty()) {
      entry.insert(QStringLiteral("row"), part.row);
    }
    parts.append(entry);
  }
  QSettings store;
  store.setValue(kPartsKey, QJsonDocument(parts).toJson(QJsonDocument::Compact));
  store.setValue(kGroupsKey, QJsonDocument(QJsonArray::fromStringList(m_groups)).toJson(QJsonDocument::Compact));
}

QList<int> pickerOrder(const QList<OrderedPart>& parts, bool inFavourites) {
  return pickerOrder(parts, inFavourites, {});
}

QList<int> pickerOrder(const QList<OrderedPart>& parts, bool inFavourites, const QList<QStringList>& groups) {
  const PartFavourites& favourites = PartFavourites::instance();
  // Classes in the order shown: starred parts of starred groups (in
  // Favourites only), the other starred parts, the rest.
  std::array<QList<int>, 3> classes;
  for (int i = 0; i < parts.size(); ++i) {
    const OrderedPart& part = parts.at(i);
    int rank = 2;
    if (favourites.starred(part.library, part.component, part.row, part.defaultRow)) {
      const QStringList others = i < groups.size() ? groups.at(i) : QStringList();
      const bool starredGroup =
          favourites.groupStarred(part.group) ||
          std::any_of(others.begin(), others.end(),
                      [&favourites](const QString& group) { return favourites.groupStarred(group); });
      rank = inFavourites && starredGroup ? 0 : 1;
    }
    classes[static_cast<size_t>(rank)].append(i);
  }
  QList<int> order;
  order.reserve(parts.size());
  for (const QList<int>& indices : classes) {
    order.append(indices);
  }
  return order;
}

QStringList groupOrder(const QStringList& groups) {
  const PartFavourites& favourites = PartFavourites::instance();
  QStringList starred;
  QStringList rest;
  for (const QString& group : groups) {
    (favourites.groupStarred(group) ? starred : rest).append(group);
  }
  return starred + rest;
}

} // namespace mitcad
