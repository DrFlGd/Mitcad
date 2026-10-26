// SPDX-License-Identifier: MIT
#include "PrintItemsHost.hpp"

#include <exception>
#include <string>

#include <QJsonDocument>
#include <QJsonValue>

#include <TopoDS_Shape.hxx>

#include "../browser/DocumentHost.hpp"
#include "../framework/Command.hpp"
#include "../framework/Json.hpp"
#include "mitcad_bridge/lib.h"
#include "rust/cxx.h"

namespace mitcad {

const CommandContext& DesignItemsHost::model() const { return m_host.model(); }

bool DesignItemsHost::runModelCommand(const QJsonObject& command, QJsonObject* result) {
  return m_host.runModelCommand(command, result);
}

bool DesignItemsHost::runModelCommands(const QJsonArray& commands, const QString& label) {
  return m_host.runModelCommands(commands, label);
}

QString DesignItemsHost::lastError() const { return m_host.lastError(); }

bool DesignItemsHost::lastCancelled() const { return m_host.lastCancelled(); }

bool DesignItemsHost::canChangePrintSettings() const { return m_host.canChangePrintSettings(); }

QString DesignItemsHost::changeBlockedReason() const { return m_host.changeBlockedReason(); }

void DesignItemsHost::commandInputsChanged() { m_host.commandInputsChanged(); }

QString DesignItemsHost::projectFolder() const { return m_host.projectFolder(); }

// My Library's own model: an empty design and what the panels query of it.
struct LibraryItemsHost::Model final : public CommandContext {
  Model() : document(new_design_document()) {}

  QJsonValue query(const QJsonObject& request) const override {
    const QJsonDocument answer = parseJson(document->query(rustStr(compactJson(request))));
    return answer.isArray() ? QJsonValue(answer.array()) : QJsonValue(answer.object());
  }
  std::shared_ptr<geometry::Shape> bodyShape(const QString& uid) const override {
    return document->body_shape(uid.toStdString());
  }
  TopoDS_Shape itemShape(const SelectionItem& /*item*/) const override { return TopoDS_Shape(); }
  TopoDS_Shape analysisShape(const QJsonObject& /*request*/) const override { return TopoDS_Shape(); }

  rust::Box<Document> document;
};

LibraryItemsHost::LibraryItemsHost() : m_model(std::make_unique<Model>()) {}

LibraryItemsHost::~LibraryItemsHost() = default;

const CommandContext& LibraryItemsHost::model() const { return *m_model; }

bool LibraryItemsHost::runModelCommand(const QJsonObject& command, QJsonObject* result) {
  // Files only, quick: on this thread, no progress.
  try {
    const QJsonDocument answer = parseJson(m_model->document->command(rustStr(compactJson(command))));
    if (result != nullptr) {
      *result = answer.object();
    }
    m_lastError.clear();
    return true;
  } catch (const std::exception& error) {
    // The caller says it in plain words and logs it.
    m_lastError = errorText(error);
    return false;
  }
}

bool LibraryItemsHost::runModelCommands(const QJsonArray& commands, const QString& /*label*/) {
  try {
    m_model->document->command(rustStr(QJsonDocument(commands).toJson(QJsonDocument::Compact)));
    m_lastError.clear();
    return true;
  } catch (const std::exception& error) {
    m_lastError = errorText(error);
    return false;
  }
}

} // namespace mitcad
