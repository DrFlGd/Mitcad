// SPDX-License-Identifier: MIT
#include "MountFrames.hpp"

#include <QSet>
#include <QStringList>

namespace mitcad {

QVector<MountFrameInfo> mountFramesOf(const QJsonValue& mounts) {
  QVector<MountFrameInfo> frames;
  QSet<QString> seen;
  for (const QJsonValue& value : mounts.toArray()) {
    const QJsonObject entry = value.toObject();
    MountFrameInfo frame;
    frame.name = entry.value(QStringLiteral("name")).toString();
    if (frame.name.isEmpty() || seen.contains(frame.name)) {
      continue;
    }
    seen.insert(frame.name);
    const QString kind = entry.value(QStringLiteral("kind")).toString();
    if (!kind.isEmpty()) {
      frame.kind = kind;
    }
    const QString joint = entry.value(QStringLiteral("joint")).toString();
    if (!joint.isEmpty()) {
      frame.joint = joint;
    }
    frame.raw = entry;
    frames.append(frame);
  }
  return frames;
}

QString mountJointText(const QString& joint) {
  if (joint == QStringLiteral("planar")) {
    return QStringLiteral("free on the face");
  }
  if (joint == QStringLiteral("revolute")) {
    return QStringLiteral("turns");
  }
  if (joint == QStringLiteral("cylindrical")) {
    return QStringLiteral("turns and slides");
  }
  if (joint == QStringLiteral("slider")) {
    return QStringLiteral("slides");
  }
  return QStringLiteral("fixed");
}

QString mountKindText(const QString& kind) {
  if (kind == QStringLiteral("shaft")) {
    return QStringLiteral("into a hole");
  }
  if (kind == QStringLiteral("hole")) {
    return QStringLiteral("onto a shaft");
  }
  if (kind == QStringLiteral("grid")) {
    return QStringLiteral("a grid");
  }
  if (kind == QStringLiteral("foot")) {
    return QStringLiteral("on a grid");
  }
  return QStringLiteral("on a face");
}

QString mountFrameLabel(const MountFrameInfo& frame) {
  return QStringLiteral("%1 (%2, %3)").arg(frame.name, mountKindText(frame.kind), mountJointText(frame.joint));
}

bool mountTakesAxes(const QString& kind) { return kind == QStringLiteral("hole") || kind == QStringLiteral("shaft"); }

QString mountCellsText(const QJsonArray& cells) {
  QStringList parts;
  for (const QJsonValue& value : cells) {
    const QJsonArray cell = value.toArray();
    if (cell.size() == 2) {
      parts << QStringLiteral("(%1, %2)").arg(cell[0].toInt()).arg(cell[1].toInt());
    }
  }
  if (parts.size() < 2) {
    return parts.join(QString());
  }
  const QString last = parts.takeLast();
  return parts.join(QStringLiteral(", ")) + QStringLiteral(" and ") + last;
}

} // namespace mitcad
