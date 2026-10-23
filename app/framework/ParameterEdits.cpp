// SPDX-License-Identifier: MIT
#include "ParameterEdits.hpp"

#include <QChar>
#include <QObject>

namespace mitcad {
namespace {

QString commandOf(const QJsonObject& command) { return command.value(QStringLiteral("cmd")).toString(); }

// A character of a parameter's name: letters, digits and the underscore.
bool nameCharacter(QChar c) { return c.isLetterOrNumber() || c == QLatin1Char('_'); }

} // namespace

bool ParameterEditScope::operator==(const ParameterEditScope& other) const {
  return sketch == other.sketch && command == other.command && editedFeature == other.editedFeature &&
         editedName == other.editedName;
}

QString ParameterEditScope::describe() const {
  if (isEmpty()) {
    return QStringLiteral("no edit");
  }
  if (command.isEmpty()) {
    return QStringLiteral("sketch %1").arg(sketch);
  }
  QString text = QStringLiteral("command %1").arg(command);
  if (!editedFeature.isEmpty()) {
    text += QStringLiteral(" editing %1").arg(editedName.isEmpty() ? editedFeature : editedName);
  }
  if (!sketch.isEmpty()) {
    text += QStringLiteral(" in sketch %1").arg(sketch);
  }
  return text;
}

QString ParameterEditScope::note() const {
  if (isEmpty()) {
    return QString();
  }
  if (command.isEmpty()) {
    return QObject::tr("%1 stays open. Its dimensions follow the parameters they use. "
                       "Undo takes a change back. OK returns to the sketch.")
        .arg(sketch);
  }
  QString text = editedFeature.isEmpty()
                     ? QObject::tr("%1 stays open. Its preview follows each change.").arg(command)
                     : QObject::tr("The edit of %1 stays open. Its preview follows each change.")
                           .arg(editedName.isEmpty() ? editedFeature : editedName);
  text += QLatin1Char(' ');
  text += QObject::tr("A change stays when you cancel the panel. OK returns to the panel.");
  if (!editedFeature.isEmpty()) {
    text += QLatin1Char(' ');
    text += QObject::tr("Change the names and expressions of %1 in its panel.")
                .arg(editedName.isEmpty() ? editedFeature : editedName);
  }
  return text;
}

bool ParameterEditScope::holds(const QJsonObject& parameter) const {
  return !editedFeature.isEmpty() &&
         parameter.value(QStringLiteral("kind")).toString() != QStringLiteral("user") &&
         parameter.value(QStringLiteral("owner")).toString() == editedFeature;
}

QString ParameterEditScope::holdReason(const QString& name) const {
  return QObject::tr("%1 is a value of %2, whose panel is open: change it there.")
      .arg(name, editedName.isEmpty() ? editedFeature : editedName);
}

bool ParameterEditScope::refuses(const QJsonObject& modelCommand, const QString& owner, QString* reason) const {
  if (editedFeature.isEmpty() || owner != editedFeature) {
    return false;
  }
  const QString cmd = commandOf(modelCommand);
  bool refused = cmd == QStringLiteral("rename_parameter") || cmd == QStringLiteral("delete_parameter");
  if (cmd == QStringLiteral("set_parameter")) {
    refused = modelCommand.contains(QStringLiteral("expression")) || modelCommand.contains(QStringLiteral("unit")) ||
              modelCommand.contains(QStringLiteral("value"));
  }
  if (refused && reason != nullptr) {
    *reason = holdReason(modelCommand.value(QStringLiteral("name")).toString());
  }
  return refused;
}

bool parameterCommandChangesValues(const QJsonObject& command) {
  const QString cmd = commandOf(command);
  if (cmd == QStringLiteral("add_parameter") || cmd == QStringLiteral("delete_parameter") ||
      cmd == QStringLiteral("rename_parameter")) {
    return true;
  }
  if (cmd == QStringLiteral("set_parameter")) {
    return command.contains(QStringLiteral("expression")) || command.contains(QStringLiteral("unit")) ||
           command.contains(QStringLiteral("value"));
  }
  return false;
}

QString renameParameterIn(const QString& text, const QString& from, const QString& to) {
  if (from.isEmpty() || from == to) {
    return text;
  }
  QString result;
  result.reserve(text.size());
  qsizetype i = 0;
  while (i < text.size()) {
    if (!nameCharacter(text.at(i))) {
      result += text.at(i);
      ++i;
      continue;
    }
    // A whole run of name characters: a name, a number or a unit.
    qsizetype end = i;
    while (end < text.size() && nameCharacter(text.at(end))) {
      ++end;
    }
    const QString word = text.mid(i, end - i);
    result += word == from ? to : word;
    i = end;
  }
  return result;
}

} // namespace mitcad
