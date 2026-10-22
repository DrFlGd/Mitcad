// SPDX-License-Identifier: MIT
#pragma once

// The dialogs of Local and Cloud projects (mitcad#89): New Project, Open
// from Cloud, and Project Settings' Change... of the address. Each runs
// its network work on a thread of its own
// (RemoteTask) with its progress and Cancel in the dialog; a cancel or a
// failure leaves the folder as it was. New Project and Create Project Here
// make an empty project (or one of the designs its folder has); the
// Projects panel (projects/ProjectsPanel.hpp, mitcad#145) adds designs.

#include <optional>

#include <QJsonObject>
#include <QString>
#include <QStringList>

class QWidget;

namespace mitcad {

// File > New Project (also Open Project of a folder that is no project yet,
// and Save to Project's new project).
struct NewProjectOptions {
  QString title;          // empty: "New Project"
  QString folder;         // suggested; empty: Documents/Mitcad/Project<n>
  bool folderFixed = false; // Open Project: the folder chosen there
  bool cloud = false;     // Storage preset to Cloud
  QString url;            // the address preset
};

struct NewProjectResult {
  enum class Kind {
    Created,       // the project is made (answer: create_project's)
    OpenProject,   // Open It: the folder is a project (or inside one)
    OpenFromCloud, // Open It Instead: the remote holds a project
  };
  Kind kind = Kind::Created;
  QString folder;      // the project's folder (absolute)
  QString url;         // Cloud: the address
  QJsonObject answer;  // Created: create_project's answer
  QString broker;     // Created: live updates through this broker, or empty
};

std::optional<NewProjectResult> askNewProject(QWidget* parent, const NewProjectOptions& options);

// File > Open from Cloud: the address and a new or empty folder; clones a
// project, makes one in an empty remote (Create Project Here) or of a
// remote's files (Make It a Project), or opens a clone that is there (Open
// It).
struct OpenFromCloudOptions {
  QString url;
  QString folder;
};

struct OpenFromCloudResult {
  QString root;       // the project's folder
  QString url;
  QString action;     // "clone", "create", "adopt" or "open"
  QJsonObject answer; // the command's answer (none for "open")
};

std::optional<OpenFromCloudResult> askOpenFromCloud(QWidget* parent, const OpenFromCloudOptions& options);

// Project Settings' Change...: a new address of the project's repository
// (moved or renamed), accepted only when it shares the project's history
// (remote_check's `related`).
std::optional<QString> askChangeAddress(QWidget* parent, const QString& root, const QString& current);

} // namespace mitcad
