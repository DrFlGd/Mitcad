// SPDX-License-Identifier: MIT
#include "PreviewBodies.hpp"

#include <cstddef>

#include <QHash>
#include <QJsonValue>
#include <QSet>

namespace mitcad {
namespace {

// An occurrence/body pair as one key.
QString pairKey(const QString& body, const QString& occurrence) {
  return body + QLatin1Char('@') + occurrence;
}

} // namespace

std::vector<PreviewBody> previewBodies(const QJsonObject& report, const std::vector<ShownBody>& shown) {
  std::vector<PreviewBody> bodies;
  if (report.value(QStringLiteral("status")).toString() != QStringLiteral("ok")) {
    return bodies;
  }
  // Every body after the command: whether the command made or changed it.
  QHash<QString, bool> after;
  for (const QJsonValue& value : report.value(QStringLiteral("bodies")).toArray()) {
    const QJsonObject body = value.toObject();
    after.insert(body.value(QStringLiteral("uid")).toString(), body.value(QStringLiteral("changed")).toBool());
  }
  QSet<QString> removed;
  for (const QJsonValue& value : report.value(QStringLiteral("removed_instances")).toArray()) {
    const QJsonObject pair = value.toObject();
    removed.insert(pairKey(pair.value(QStringLiteral("body")).toString(),
                           pair.value(QStringLiteral("occurrence")).toString()));
  }
  // Occurrences the command places elsewhere (joints, moves of occurrences;
  // mitcad#55), by their paths of uids.
  QHash<QString, QJsonArray> moved;
  for (const QJsonValue& value : report.value(QStringLiteral("placements")).toArray()) {
    const QJsonObject placement = value.toObject();
    moved.insert(placement.value(QStringLiteral("path")).toString(),
                 placement.value(QStringLiteral("transform")).toArray());
  }

  // The bodies shown now, where they are shown (or moved to).
  QSet<QString> drawn;
  for (std::size_t i = 0; i < shown.size(); ++i) {
    const ShownBody& body = shown[i];
    const auto state = after.constFind(body.uid);
    const QString key = pairKey(body.uid, body.occurrence);
    if (state == after.cend() || removed.contains(key) || drawn.contains(key)) {
      continue;
    }
    PreviewBody entry;
    entry.uid = body.uid;
    entry.occurrence = body.occurrence;
    entry.shown = static_cast<int>(i);
    entry.previewShape = state.value();
    const auto placedAt = moved.constFind(body.occurrence);
    entry.placed = placedAt != moved.cend();
    entry.transform = entry.placed ? placedAt.value() : body.transform;
    drawn.insert(key);
    bodies.push_back(entry);
  }

  // What the command makes: each new occurrence/body pair at the placement
  // the model gives. A component's occurrences share its bodies, so a new
  // occurrence takes the look of the body where it is shown already.
  for (const QJsonValue& value : report.value(QStringLiteral("instances")).toArray()) {
    const QJsonObject pair = value.toObject();
    if (!pair.value(QStringLiteral("visible")).toBool(true)) {
      continue;
    }
    PreviewBody entry;
    entry.uid = pair.value(QStringLiteral("body")).toString();
    entry.occurrence = pair.value(QStringLiteral("occurrence")).toString();
    const QString key = pairKey(entry.uid, entry.occurrence);
    if (!after.contains(entry.uid) || drawn.contains(key)) {
      continue;
    }
    for (std::size_t i = 0; i < shown.size(); ++i) {
      if (shown[i].uid == entry.uid) {
        entry.shown = static_cast<int>(i);
        break;
      }
    }
    entry.previewShape = true;
    entry.made = true;
    entry.placed = true;
    entry.transform = pair.value(QStringLiteral("transform")).toArray();
    drawn.insert(key);
    bodies.push_back(entry);
  }
  // Nothing else (mitcad#266): a body of the report neither shown nor in a
  // new visible pair is hidden in the normal view, or has no placement at
  // the marker. Drawing it in its component's own coordinates would put it
  // at the design's origin, visible.
  return bodies;
}

} // namespace mitcad
