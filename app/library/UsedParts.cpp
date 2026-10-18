// SPDX-License-Identifier: MIT
#include "UsedParts.hpp"

#include <exception>

#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QtLogging>

#include "../framework/Command.hpp"

namespace mitcad {

QList<UsedPart> usedLibraryParts(const CommandContext& context) {
  QList<UsedPart> parts;
  QJsonObject answer;
  try {
    answer = context.query(QJsonObject{{QStringLiteral("query"), QStringLiteral("library_usage")}}).toObject();
  } catch (const std::exception& error) {
    qWarning().noquote() << QStringLiteral("Part picker: the design's library parts not read: %1")
                                .arg(QString::fromUtf8(error.what()));
    return parts;
  }
  for (const QJsonValue& value : answer.value(QStringLiteral("parts")).toArray()) {
    const QJsonObject entry = value.toObject();
    UsedPart part;
    part.library = entry.value(QStringLiteral("library")).toString();
    part.url = entry.value(QStringLiteral("url")).toString();
    part.component = entry.value(QStringLiteral("component")).toString();
    part.count = entry.value(QStringLiteral("count")).toInt();
    if (part.library.isEmpty() || part.component.isEmpty() || part.count <= 0) {
      continue;
    }
    parts.append(part);
  }
  return parts;
}

} // namespace mitcad
