// SPDX-License-Identifier: MIT
// CommandSession: a new feature's operation and objects chosen by the model
// from each preview's tool until the user picks them (mitcad#289:
// Extrude's automatic operation; CommandDef::autoOperation). The rest of
// the session is in CommandSession.cpp.
#include <QDebug>
#include <QJsonArray>
#include <QJsonObject>
#include <QStringList>

#include "CommandSession.hpp"

namespace mitcad {
namespace {

const QString kOperation = QStringLiteral("operation");
const QString kObjects = QStringLiteral("objects");

QStringList ownersOf(const Selection& items) {
  QStringList owners;
  for (const SelectionItem& item : items) {
    owners << item.owner;
  }
  return owners;
}

} // namespace

void CommandSession::startAutoOperation() {
  m_autoOperation = m_def.autoOperation && !isEditing() && m_analysis.isEmpty() && slotOf(kOperation).has_value();
}

void CommandSession::addAutoOperation(QJsonObject& command) const {
  if (m_autoOperation) {
    command.insert(QStringLiteral("auto_operation"), true);
  }
}

bool CommandSession::takeAutoOperation(const QJsonObject& report) {
  if (!m_autoOperation || m_done) {
    return false;
  }
  const QJsonObject choice = report.value(QStringLiteral("auto_operation")).toObject();
  const QString operation = choice.value(kOperation).toString();
  const auto slot = slotOf(kOperation);
  if (operation.isEmpty() || !slot) {
    return false;
  }
  // Only what the input offers (an operation it lacks leaves it as it is).
  QString label;
  for (const auto& [value, text] : slot->def->choicesIn(m_state)) {
    if (value == operation) {
      label = text;
    }
  }
  if (label.isEmpty()) {
    return false;
  }
  // The bodies in the feature's component, placed where its first input's
  // items are (a sketch in a component placed off the origin).
  QString occurrence;
  if (const InputDef* primary = m_def.primarySelection()) {
    const Selection& first = m_state.items(primary->id);
    if (!first.isEmpty()) {
      occurrence = first.first().occurrence;
    }
  }
  Selection objects;
  for (const QJsonValue& uid : choice.value(kObjects).toArray()) {
    objects.append({SelectKind::Body, uid.toString(), QString(), QString(), occurrence});
  }
  const bool hasObjects = slotOf(kObjects).has_value();
  if (m_state.choice(kOperation) == operation &&
      (!hasObjects || ownersOf(m_state.items(kObjects)) == ownersOf(objects))) {
    return false;
  }
  m_state.setChoice(kOperation, operation);
  // What the UI tests read: the operation the model chose.
  qDebug().noquote() << QStringLiteral("%1: automatic %2 = %3").arg(m_def.name, slot->def->label, label);
  if (hasObjects) {
    m_state.setItems(kObjects, objects);
    logSelection(kObjects);
  }
  if (m_presenter) {
    m_presenter->refresh(m_state);
  }
  updateHighlights();
  return true;
}

void CommandSession::autoOperationOverruled(const QString& key) {
  if (!m_autoOperation || (key != kOperation && key != kObjects)) {
    return;
  }
  // The user's own choice stays for the rest of the command.
  m_autoOperation = false;
  qDebug().noquote() << QStringLiteral("%1: operation chosen by hand").arg(m_def.name);
}

} // namespace mitcad
