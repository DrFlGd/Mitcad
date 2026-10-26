// SPDX-License-Identifier: MIT
#pragma once

#include <memory>

#include <QJsonArray>
#include <QJsonObject>
#include <QString>

namespace mitcad {

class CommandContext;
class DocumentHost;

// What the panels of materials, printers and processes ask of the model
// they work through (mitcad#704): the open design's (Materials, Printers
// and Processes) or a model of their own that holds no design (My
// Library), whose commands change only the libraries' files.
class PrintItemsHost {
public:
  virtual ~PrintItemsHost() = default;

  virtual const CommandContext& model() const = 0;
  // Runs a model command; false, with lastError(), when it was refused or
  // cancelled.
  virtual bool runModelCommand(const QJsonObject& command, QJsonObject* result = nullptr) = 0;
  // Runs commands as one undo step named `label`.
  virtual bool runModelCommands(const QJsonArray& commands, const QString& label) = 0;
  virtual QString lastError() const = 0;
  virtual bool lastCancelled() const = 0;
  // Whether the model can change now, and why not.
  virtual bool canChangePrintSettings() const = 0;
  virtual QString changeBlockedReason() const = 0;
  // A command changed the model: a running command's panel follows it.
  virtual void commandInputsChanged() {}
  // The folder of the design's project (its library); empty outside one.
  virtual QString projectFolder() const = 0;
  // Whether a design stands behind the model: its bodies, its choices,
  // Assign, Save to and Use in this design. False for My Library.
  virtual bool hasDesign() const { return true; }
};

// The open design's model, through the main window (DocumentHost).
class DesignItemsHost final : public PrintItemsHost {
public:
  explicit DesignItemsHost(DocumentHost& host) : m_host(host) {}

  const CommandContext& model() const override;
  bool runModelCommand(const QJsonObject& command, QJsonObject* result = nullptr) override;
  bool runModelCommands(const QJsonArray& commands, const QString& label) override;
  QString lastError() const override;
  bool lastCancelled() const override;
  bool canChangePrintSettings() const override;
  QString changeBlockedReason() const override;
  void commandInputsChanged() override;
  QString projectFolder() const override;

private:
  DocumentHost& m_host;
};

// A model of its own for My Library (mitcad#704): an empty design that
// is never shown or saved. The library commands (`create_material`,
// `edit_print_profile`, ... with `library`, `import_print_profile` into a
// library) read and write the libraries' files and leave the open design
// alone, which learns of the changes as of another design's.
class LibraryItemsHost final : public PrintItemsHost {
public:
  LibraryItemsHost();
  ~LibraryItemsHost() override;

  const CommandContext& model() const override;
  bool runModelCommand(const QJsonObject& command, QJsonObject* result = nullptr) override;
  bool runModelCommands(const QJsonArray& commands, const QString& label) override;
  QString lastError() const override { return m_lastError; }
  bool lastCancelled() const override { return false; }
  bool canChangePrintSettings() const override { return true; }
  QString changeBlockedReason() const override { return {}; }
  QString projectFolder() const override { return m_project; }
  bool hasDesign() const override { return false; }

  // The open design's project, whose library is the second place.
  void setProjectFolder(const QString& folder) { m_project = folder; }

private:
  struct Model;
  std::unique_ptr<Model> m_model;
  QString m_project;
  QString m_lastError;
};

} // namespace mitcad
