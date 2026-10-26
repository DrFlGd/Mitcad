// SPDX-License-Identifier: MIT
#include "DrawingTemplates.hpp"

#include <QCoreApplication>
#include <QJsonArray>

namespace mitcad::drawings {

QList<TemplateChoice> templateChoices(const QJsonObject& answer) {
  QList<TemplateChoice> out;
  for (const QJsonValue& entry : answer.value(QStringLiteral("templates")).toArray()) {
    QJsonObject data = entry.toObject();
    const QString choice = data.value(QStringLiteral("choice")).toString();
    const QString library = data.value(QStringLiteral("library")).toString();
    const QString name = data.value(QStringLiteral("name")).toString();
    if (choice.isEmpty() || name.isEmpty()) {
      continue;
    }
    TemplateChoice item;
    item.choice = choice;
    item.size = data.value(QStringLiteral("size")).toString();
    item.projection = data.value(QStringLiteral("projection")).toString();
    if (library == QStringLiteral("user")) {
      item.label = QCoreApplication::translate("DrawingTemplates", "%1 (User)").arg(name);
    } else if (library == QStringLiteral("project")) {
      item.label = QCoreApplication::translate("DrawingTemplates", "%1 (Project)").arg(name);
    } else {
      item.label = name;
    }
    if (library == QStringLiteral("user") || library == QStringLiteral("project")) {
      // A library's template goes to the model as its data: the sheet keeps
      // a copy of it.
      data.remove(QStringLiteral("choice"));
      item.value = data;
    } else {
      item.value = data.value(QStringLiteral("id")).toString();
    }
    out.append(item);
  }
  TemplateChoice simple;
  simple.label = QCoreApplication::translate("DrawingTemplates", "Simple Title Block");
  simple.value = QString();
  out.append(simple);
  return out;
}

int templateIndex(const QList<TemplateChoice>& choices, const QString& choice) {
  for (int i = 0; i < choices.size(); ++i) {
    if (choices.at(i).choice == choice) {
      return i;
    }
  }
  return -1;
}

QString defaultTemplate(const QString& size) {
  return size.startsWith(QStringLiteral("ANSI")) ? QStringLiteral("ansi") : QStringLiteral("iso");
}

} // namespace mitcad::drawings
