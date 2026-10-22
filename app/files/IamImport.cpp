// SPDX-License-Identifier: MIT
#include "IamImport.hpp"

#include <initializer_list>

#include <QCoreApplication>
#include <QRegularExpression>
#include <QStringList>

namespace mitcad {
namespace {

const QString kPartsHistory = QStringLiteral("import/iamPartsHistory");

QString tr(const char* text) { return QCoreApplication::translate("IamImport", text); }

} // namespace

bool iamPartsHistory(const QSettings& settings) { return settings.value(kPartsHistory, true).toBool(); }

void rememberIamPartsHistory(QSettings& settings, bool history) { settings.setValue(kPartsHistory, history); }

std::optional<IamPartProgress> parseIamPartProgress(const QString& line) {
  static const QRegularExpression pattern(QStringLiteral(
      R"(^import: \[[0-9.]+\] part ([0-9]+) of ([0-9]+): (.+?)(?:: (replayed|bodies) in ([0-9.]+) s)?$)"));
  const QRegularExpressionMatch match = pattern.match(line);
  if (!match.hasMatch()) {
    return std::nullopt;
  }
  IamPartProgress progress;
  progress.part = match.captured(1).toInt();
  progress.parts = match.captured(2).toInt();
  progress.file = match.captured(3);
  progress.outcome = match.captured(4);
  progress.seconds = match.captured(5).toDouble();
  return progress;
}

QString iamPartOutcome(const QJsonObject& part) {
  const QString outcome = part.value(QStringLiteral("outcome")).toString();
  if (outcome == QLatin1String("replayed")) {
    const QJsonObject counts = part.value(QStringLiteral("counts")).toObject();
    QStringList parts;
    for (const char* kind : {"parametric", "partial", "fallback", "skipped"}) {
      parts << QStringLiteral("%1 %2").arg(counts.value(QLatin1String(kind)).toInt()).arg(QLatin1String(kind));
    }
    return tr("replayed, %1 items (%2), %3 verified, %4 parameters")
        .arg(part.value(QStringLiteral("items")).toInt())
        .arg(parts.join(QStringLiteral(", ")))
        .arg(part.value(QStringLiteral("verified")).toInt())
        .arg(part.value(QStringLiteral("parameters")).toInt());
  }
  if (outcome == QLatin1String("bodies")) {
    return tr("stored bodies: %1").arg(part.value(QStringLiteral("reason")).toString());
  }
  return QString();
}

} // namespace mitcad
