// SPDX-License-Identifier: MIT
// CommandSession: a feature's operation and objects chosen by the model
// from each preview's tool until the user picks them (mitcad#289:
// Extrude's automatic operation; CommandDef::autoOperation). The rest of
// the session is in CommandSession.cpp.
#include <QDebug>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStringList>

#include "CommandSession.hpp"
#include "ModelWorker.hpp"

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

void CommandSession::startAutoOperation(const QJsonObject& storedDefinition) {
  m_autoOperation.start(m_def.autoOperation && m_analysis.isEmpty() && slotOf(kOperation).has_value(),
                        isEditing() ? storedDefinition : QJsonObject());
}

void CommandSession::resolveAutoOperationProbe(const QJsonObject& report) {
  if (!m_autoOperation.resolve(report)) return;
  const bool verified = report.value(QStringLiteral("status")).toString() == QStringLiteral("ok") &&
                        report.value(QStringLiteral("auto_operation")).toObject().contains(kOperation);
  qDebug().noquote() << QStringLiteral("%1: %2").arg(m_def.name,
      m_autoOperation.active() ? QStringLiteral("stored operation matches automatic choice; editing follows geometry")
      : verified ? QStringLiteral("stored operation differs from automatic choice; editing keeps it")
                 : QStringLiteral("stored operation could not be verified; editing keeps it"));
}

bool CommandSession::probeAutoOperation() {
  if (!m_autoOperation.pending()) return false;
  const QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("edit_feature")},
                            {QStringLiteral("uid"), m_editUid},
                            {QStringLiteral("def"), m_autoOperation.stored()},
                            {QStringLiteral("auto_operation"), true}};
  const QByteArray key = QJsonDocument(command).toJson(QJsonDocument::Compact);
  const quint64 generation = m_generation;
  m_presenter->setMessage(tr("Computing preview…"), false);
  m_presenter->setOkEnabled(false);
  if (m_host.asynchronousCommands()) {
    m_asyncPending = true;
    const bool started = m_host.modelPreviewAsync(key, m_def.name, this,
        [this, key, generation](const CommandHost::AsyncResult& result) {
          if (!asyncPresenterAvailable()) return;
          m_asyncPending = false;
          if (m_cancelRequested) { finishAsyncCancel(); return; }
          if (result.cancelled) {
            // An input cancels old work before it is applied. Retry the
            // same stored baseline after that input, never its new geometry.
            if (generation != m_generation) schedulePreview();
            else showCancelledPreview(key);
            return;
          }
          resolveAutoOperationProbe(result.succeeded() ? result.answer : QJsonObject());
          // A changed input may still be queued behind the model job. The
          // normal preview follows after it and alone updates the controls.
          schedulePreview();
        });
    if (!started) { m_asyncPending = false; schedulePreview(); }
    return true;
  }
  try {
    const QJsonObject report = m_host.modelPreview(key, m_def.name);
    if (!m_done && m_presenter) resolveAutoOperationProbe(report);
  } catch (const ComputationCancelled&) {
    if (!m_done && m_presenter) {
      if (generation != m_generation) schedulePreview();
      else showCancelledPreview(key);
    }
    return true;
  } catch (const std::exception&) {
    if (!m_done && m_presenter) resolveAutoOperationProbe({});
  }
  if (!m_done && m_presenter) schedulePreview();
  return true;
}

void CommandSession::addAutoOperation(QJsonObject& command) const {
  if (m_autoOperation.active()) {
    command.insert(QStringLiteral("auto_operation"), true);
  }
}

bool CommandSession::takeAutoOperation(const QJsonObject& report) {
  if (!m_autoOperation.active() || m_done) {
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
  const bool changedOperation = m_state.choice(kOperation) != operation;
  m_state.setChoice(kOperation, operation);
  // What the UI tests read: the operation the model chose.
  qDebug().noquote() << QStringLiteral("%1: automatic %2 = %3").arg(m_def.name, slot->def->label, label);
  if (hasObjects) {
    m_state.setItems(kObjects, objects);
    logSelection(kObjects);
  }
  if (changedOperation && m_def.autoOperationApplied) {
    m_def.autoOperationApplied(m_state, m_host);
  }
  if (m_presenter) {
    m_presenter->refresh(m_state);
  }
  updateHighlights();
  return true;
}

void CommandSession::autoOperationOverruled(const QString& key) {
  const auto slot = slotOf(key);
  const bool targets = slot && slot->def->type == InputDef::Type::CutTargets;
  if ((!m_autoOperation.active() && !m_autoOperation.pending()) ||
      (key != kOperation && key != kObjects && !targets)) {
    return;
  }
  // The user's own choice stays for the rest of the command.
  m_autoOperation.disable();
  qDebug().noquote() << QStringLiteral("%1: operation chosen by hand").arg(m_def.name);
}

} // namespace mitcad
