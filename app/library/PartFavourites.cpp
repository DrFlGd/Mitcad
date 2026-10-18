// SPDX-License-Identifier: MIT
#include "PartFavourites.hpp"

#include <array>

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QSettings>

namespace mitcad {
namespace {

// The settings: JSON arrays, as the libraries' sources (files/Libraries.cpp),
// so that any id keeps as it is in the registry and in INI files.
const QString kPartsKey = QStringLiteral("partPicker/starredParts");
const QString kGroupsKey = QStringLiteral("partPicker/starredGroups");

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
    const QPair<QString, QString> key(part.value(QStringLiteral("library")).toString(),
                                      part.value(QStringLiteral("component")).toString());
    if (!key.first.isEmpty() && !key.second.isEmpty() && !m_partSet.contains(key)) {
      m_parts.append(key);
      m_partSet.insert(key);
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

bool PartFavourites::starred(const QString& library, const QString& component) const {
  return m_partSet.contains(qMakePair(library, component));
}

void PartFavourites::setStarred(const QString& library, const QString& component, bool on) {
  const QPair<QString, QString> key(library, component);
  if (library.isEmpty() || component.isEmpty() || m_partSet.contains(key) == on) {
    return;
  }
  if (on) {
    m_parts.append(key);
    m_partSet.insert(key);
  } else {
    m_parts.removeAll(key);
    m_partSet.remove(key);
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

QList<QPair<QString, QString>> PartFavourites::starredParts() const { return m_parts; }

void PartFavourites::save() const {
  QJsonArray parts;
  for (const QPair<QString, QString>& part : m_parts) {
    parts.append(QJsonObject{{QStringLiteral("library"), part.first}, {QStringLiteral("component"), part.second}});
  }
  QSettings store;
  store.setValue(kPartsKey, QJsonDocument(parts).toJson(QJsonDocument::Compact));
  store.setValue(kGroupsKey, QJsonDocument(QJsonArray::fromStringList(m_groups)).toJson(QJsonDocument::Compact));
}

QList<int> pickerOrder(const QList<OrderedPart>& parts, bool inFavourites) {
  const PartFavourites& favourites = PartFavourites::instance();
  // Classes in the order shown: starred parts of starred groups (in
  // Favourites only), the other starred parts, the rest.
  std::array<QList<int>, 3> classes;
  for (int i = 0; i < parts.size(); ++i) {
    const OrderedPart& part = parts.at(i);
    int rank = 2;
    if (favourites.starred(part.library, part.component)) {
      rank = inFavourites && favourites.groupStarred(part.group) ? 0 : 1;
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
