// SPDX-License-Identifier: MIT
#pragma once

#include <optional>

#include <QString>
#include <QStringList>

namespace mitcad {

// The application's general settings (Preferences, General), kept in the
// user's settings (autosave/enabled, autosave/minutes). Minutes, like every
// unit the application chooses, are metric; there is no unit to pick.
struct GeneralSettings {
  static constexpr int kMinAutosaveMinutes = 1;
  static constexpr int kMaxAutosaveMinutes = 60;

  // Autosave (P8): a modified document is written to the recovery folder
  // this often (app/files/Autosave.hpp).
  bool autosave = true;
  int autosaveMinutes = 5;

  // The .f3d import's geometric tolerance (mm, mitcad#224; the setting
  // import/geometricTolerance): how far the replay's geometry may be from
  // the file's where it is not exact (the model's import option
  // geometric_tolerance).
  static constexpr double kMinImportTolerance = 0.001;
  static constexpr double kMaxImportTolerance = 1.0;
  double importTolerance = 0.01;

  static GeneralSettings load();
  void save() const;
};

// The default author of versions (Preferences, Cloud; the settings
// versions/name, versions/email): a project's versions are recorded by the
// author in its repository's configuration (New Project and Project
// Settings set it), else git's configured user (user.name and user.email),
// else this name and email (mitcad#89).
struct VersionSettings {
  QString name;
  QString email;

  // Both a name and an email address are given.
  bool complete() const;
  // "Name <email>", the form the version history takes.
  QString author() const;

  static VersionSettings load();
  void save() const;
};

// Where File > New Project puts projects by default: "Mitcad" in the
// user's documents (MITCAD_PROJECTS_DIR for tests).
QString projectsDirectory();

// Cloud projects (P12 remote, mitcad#89; Preferences, Cloud; the settings
// remote/git, remote/checkMinutes, remote/autoPush, remote/allowLive,
// remote/defaultBroker): the git program that does the network work
// (empty: found on PATH, on Windows also where Git for Windows installs
// itself), how often the remote of the current project is checked for
// newer versions (when a design opens and then every so many minutes; 0:
// never on its own) and whether each version saved is sent to the remote
// at once, both the defaults of projects that do not set them (Project
// Settings); whether live updates (MQTT) are allowed at all, and the
// broker New Project offers.
struct RemoteSettings {
  static constexpr int kMaxCheckMinutes = 240;

  QString git;
  int checkMinutes = 10;
  bool autoPush = true;
  bool allowLive = true;
  QString defaultBroker;

  static RemoteSettings load();
  void save() const;
  // Gives the remote work (core/vcs) the git program: MITCAD_GIT set to
  // the one chosen, or as the environment had it when none is. Only while
  // no remote work runs (the variable is read on its thread).
  void apply() const;
};

// The recovery folder of autosave: "autosave" in the user's local
// application data (~/.local/share/Mitcad/Mitcad/autosave on Linux,
// %LOCALAPPDATA%\Mitcad\Mitcad\autosave on Windows), or MITCAD_AUTOSAVE_DIR
// (tests). Never a project's folder; empty when the system gives no place.
QString recoveryDirectory();

// The Projects panel (mitcad#145; the settings projects/known,
// projects/hidden, projects/panel): the projects made, opened or cloned on
// this computer (their folders, newest first), the projects folder's
// projects that Remove from List took out of the panel, and whether the
// panel is shown. Nothing here deletes a project.
struct ProjectsSettings {
  static constexpr int kMaxKnown = 100;

  QStringList known;
  QStringList hidden;
  // The user's last show or hide of the panel; none before the first
  // (mitcad#151: the panel then shows when a project is configured).
  std::optional<bool> panelShown;

  static ProjectsSettings load();
  void save() const;
  // A project made, opened or cloned: first in the list (and in the panel
  // again when it was removed from it).
  static void remember(const QString& root);
  // Remove from List: the project leaves the list, and the panel no longer
  // shows it (also when it is in the projects folder).
  static void forget(const QString& root);
};

} // namespace mitcad
