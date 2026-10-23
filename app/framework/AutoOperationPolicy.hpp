// SPDX-License-Identifier: MIT
#pragma once

#include <QJsonArray>
#include <QJsonObject>
#include <QSet>
#include <QString>

namespace mitcad {

// The baseline is immutable throughout an edit's initial asynchronous probe.
// Cancellation leaves it pending; a manual choice prevents a late reply from
// enabling automatic operation again (mitcad#575).
class AutoOperationPolicy {
public:
  void start(bool enabled, const QJsonObject& storedDefinition) {
    m_active = enabled && storedDefinition.isEmpty();
    m_stored = enabled ? storedDefinition : QJsonObject();
  }
  bool active() const { return m_active; }
  bool pending() const { return !m_stored.isEmpty(); }
  const QJsonObject& stored() const { return m_stored; }
  void disable() { m_active = false; m_stored = {}; }

  // No result may replace the decision once resolved or manually disabled.
  bool resolve(const QJsonObject& report) {
    if (!pending()) return false;
    m_active = matches(m_stored, report);
    m_stored = {};
    return true;
  }

private:
  static QSet<QString> bodySet(const QJsonArray& bodies) {
    QSet<QString> result;
    for (const QJsonValue& body : bodies) result.insert(body.toString());
    return result;
  }

  static bool matches(const QJsonObject& stored, const QJsonObject& report) {
    const QJsonObject automatic = report.value(QStringLiteral("auto_operation")).toObject();
    const QString operation = automatic.value(QStringLiteral("operation")).toString();
    if (report.value(QStringLiteral("status")).toString() != QStringLiteral("ok") || operation.isEmpty() ||
        operation != stored.value(QStringLiteral("operation")).toString()) return false;
    QSet<QString> selected = bodySet(stored.value(QStringLiteral("participants")).toArray());
    if (operation == QStringLiteral("cut") && !stored.value(QStringLiteral("cut_scope")).toObject().isEmpty()) {
      // The hidden Objects input does not represent a modern Cut. Compare
      // the actual selected intersections, including foreign selections and
      // body exclusions, with the model's local automatic target bodies.
      const QJsonObject targets = report.value(QStringLiteral("cut_targets")).toObject();
      if (!targets.contains(QStringLiteral("rows"))) return false;
      selected.clear();
      for (const QJsonValue& row : targets.value(QStringLiteral("rows")).toArray()) {
        for (const QJsonValue& body : row.toObject().value(QStringLiteral("bodies")).toArray()) {
          const QJsonObject target = body.toObject();
          if (target.value(QStringLiteral("checked")).toBool()) selected.insert(target.value(QStringLiteral("body")).toString());
        }
      }
    }
    return selected == bodySet(automatic.value(QStringLiteral("objects")).toArray());
  }

  bool m_active = false;
  QJsonObject m_stored;
};

} // namespace mitcad
